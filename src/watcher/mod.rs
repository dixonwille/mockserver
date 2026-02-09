// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! File watcher module
//!
//! Watches the mocks directory for changes and triggers reloads.
//! Also provides idle domain flushing and retention cleanup background tasks.

use crate::db::cleanup_old_requests;
use crate::lua::ScriptManager;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_rusqlite::Connection;
use tracing::{debug, info, warn};

/// Debounce duration for file changes (100ms)
const DEBOUNCE_DURATION: Duration = Duration::from_millis(100);

/// Start watching the mocks directory for changes
///
/// Returns a handle that can be used to stop the watcher.
pub async fn start_watcher(
    mocks_dir: PathBuf,
    scripts: Arc<ScriptManager>,
) -> notify::Result<WatcherHandle> {
    let (tx, rx) = mpsc::channel(100);

    // Create the file watcher
    let watcher_tx = tx.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Only care about modifications, creates, and deletes
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        let _ = watcher_tx.blocking_send(event);
                    }
                    _ => {}
                }
            }
        },
        Config::default(),
    )?;

    // Start watching the mocks directory
    watcher.watch(&mocks_dir, RecursiveMode::Recursive)?;

    info!(path = %mocks_dir.display(), "Started file watcher");

    // Spawn the debounce processor
    let mocks_dir_clone = mocks_dir.clone();
    let handle = tokio::spawn(async move {
        process_events(rx, mocks_dir_clone, scripts).await;
    });

    Ok(WatcherHandle {
        _watcher: watcher,
        _task: handle,
    })
}

/// Handle to the file watcher
///
/// The watcher stops when this handle is dropped.
pub struct WatcherHandle {
    _watcher: RecommendedWatcher,
    _task: tokio::task::JoinHandle<()>,
}

/// Process file events with debouncing
async fn process_events(
    mut rx: mpsc::Receiver<Event>,
    mocks_dir: PathBuf,
    scripts: Arc<ScriptManager>,
) {
    let mut pending_domains: HashSet<String> = HashSet::new();
    let mut last_event_time = Instant::now();

    loop {
        // Wait for events or debounce timeout
        let timeout = if pending_domains.is_empty() {
            // No pending domains, wait indefinitely for next event
            Duration::from_secs(3600)
        } else {
            // Calculate remaining debounce time
            let elapsed = last_event_time.elapsed();
            if elapsed >= DEBOUNCE_DURATION {
                Duration::ZERO
            } else {
                DEBOUNCE_DURATION - elapsed
            }
        };

        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(event) => {
                        // Process the event and collect affected domains
                        for path in &event.paths {
                            if let Some(domain) = get_domain_from_path(path, &mocks_dir) {
                                // Only watch .lua files
                                if is_lua_file(path) || is_init_lua_event(&event, path) {
                                    debug!(domain = %domain, path = %path.display(), "File change detected");
                                    pending_domains.insert(domain);
                                    last_event_time = Instant::now();
                                }
                            }
                        }
                    }
                    None => {
                        // Channel closed, exit
                        info!("File watcher channel closed");
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(timeout), if !pending_domains.is_empty() => {
                // Debounce timeout reached, process pending domains
                process_pending_reloads(&pending_domains, &scripts, &mocks_dir).await;
                pending_domains.clear();
            }
        }
    }
}

/// Check if the path is a .lua file
fn is_lua_file(path: &Path) -> bool {
    path.extension().map(|e| e == "lua").unwrap_or(false)
}

/// Check if this is an event related to init.lua (for domain add/remove)
fn is_init_lua_event(event: &Event, path: &Path) -> bool {
    if let Some(name) = path.file_name()
        && name == "init.lua"
    {
        return matches!(event.kind, EventKind::Create(_) | EventKind::Remove(_));
    }
    false
}

/// Extract the domain name from a file path
///
/// Given a path like `/mocks/api.example.com/helpers.lua`, extracts `api.example.com`
fn get_domain_from_path(path: &Path, mocks_dir: &Path) -> Option<String> {
    // Get the path relative to mocks_dir
    let relative = path.strip_prefix(mocks_dir).ok()?;

    // Get the first component (the domain folder)
    let domain = relative.components().next()?;

    let domain_str = domain.as_os_str().to_str()?;

    // Skip hidden directories (e.g., .mockserver, .git)
    if domain_str.starts_with('.') {
        return None;
    }

    Some(domain_str.to_string())
}

/// Process pending domain reloads
async fn process_pending_reloads(
    domains: &HashSet<String>,
    scripts: &Arc<ScriptManager>,
    mocks_dir: &Path,
) {
    for domain in domains {
        let init_path = mocks_dir.join(domain).join("init.lua");

        if init_path.exists() {
            // Domain exists, reload it
            match scripts.load_domain(domain).await {
                Ok(_) => {
                    info!(domain = %domain, "Reloaded domain");
                }
                Err(e) => {
                    warn!(domain = %domain, error = %e, "Failed to reload domain (syntax error?)");
                }
            }
        } else {
            // Domain was removed, unload it
            scripts.unload_domain(domain).await;
            info!(domain = %domain, "Unloaded removed domain");
        }
    }
}

/// Start the idle domain flusher background task
///
/// Periodically checks for domains that haven't been accessed within the idle timeout
/// and unloads them to free memory. Also shrinks idle runtime pools to reduce memory usage.
/// Domains will be lazily reloaded on next request.
///
/// Returns a handle that stops the flusher when dropped.
pub fn start_idle_flusher(
    scripts: Arc<ScriptManager>,
    idle_timeout: Duration,
) -> IdleFlusherHandle {
    // Check interval is half the timeout, minimum 30 seconds
    let check_interval = (idle_timeout / 2).max(Duration::from_secs(30));

    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(check_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            // Shrink idle runtime pools first
            scripts.shrink_idle_pools();

            // Then flush completely cold domains
            let flushed = scripts.flush_idle_domains(idle_timeout);
            if !flushed.is_empty() {
                debug!(
                    domains = ?flushed,
                    "Flushed idle domains"
                );
            }
        }
    });

    IdleFlusherHandle { _task: handle }
}

/// Handle to the idle flusher background task
///
/// The flusher stops when this handle is dropped.
pub struct IdleFlusherHandle {
    _task: tokio::task::JoinHandle<()>,
}

/// Check interval for retention cleanup (1 hour)
const RETENTION_CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Start the retention cleanup background task
///
/// Periodically deletes requests older than the configured retention period.
/// Returns a handle that stops the worker when dropped.
pub fn start_retention_cleanup(db: Arc<Connection>, retention_days: u32) -> RetentionCleanupHandle {
    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(RETENTION_CHECK_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            match cleanup_old_requests(&db, retention_days).await {
                Ok(deleted) if deleted > 0 => {
                    info!(deleted, retention_days, "Retention cleanup completed");
                }
                Err(e) => {
                    warn!(error = %e, "Retention cleanup failed");
                }
                _ => {}
            }
        }
    });
    RetentionCleanupHandle { _task: handle }
}

/// Handle to the retention cleanup background task
///
/// The worker stops when this handle is dropped.
pub struct RetentionCleanupHandle {
    _task: tokio::task::JoinHandle<()>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};
    use std::path::PathBuf;

    // ==================== get_domain_from_path tests ====================

    #[test]
    fn test_get_domain_from_path() {
        let mocks_dir = PathBuf::from("/mocks");

        // Normal domain file
        assert_eq!(
            get_domain_from_path(
                &PathBuf::from("/mocks/api.example.com/init.lua"),
                &mocks_dir
            ),
            Some("api.example.com".to_string())
        );

        // Nested file
        assert_eq!(
            get_domain_from_path(
                &PathBuf::from("/mocks/api.example.com/routes/users.lua"),
                &mocks_dir
            ),
            Some("api.example.com".to_string())
        );

        // Hidden directory (should be ignored)
        assert_eq!(
            get_domain_from_path(&PathBuf::from("/mocks/.mockserver/types.lua"), &mocks_dir),
            None
        );

        // _default domain
        assert_eq!(
            get_domain_from_path(&PathBuf::from("/mocks/_default/init.lua"), &mocks_dir),
            Some("_default".to_string())
        );
    }

    #[test]
    fn test_get_domain_from_path_git_directory() {
        let mocks_dir = PathBuf::from("/mocks");

        assert_eq!(
            get_domain_from_path(&PathBuf::from("/mocks/.git/config"), &mocks_dir),
            None
        );
    }

    #[test]
    fn test_get_domain_from_path_deeply_nested() {
        let mocks_dir = PathBuf::from("/mocks");

        assert_eq!(
            get_domain_from_path(
                &PathBuf::from("/mocks/api.example.com/routes/v1/users/handlers.lua"),
                &mocks_dir
            ),
            Some("api.example.com".to_string())
        );
    }

    #[test]
    fn test_get_domain_from_path_outside_mocks() {
        let mocks_dir = PathBuf::from("/mocks");

        assert_eq!(
            get_domain_from_path(&PathBuf::from("/other/dir/file.lua"), &mocks_dir),
            None
        );
    }

    #[test]
    fn test_get_domain_from_path_mocks_dir_itself() {
        let mocks_dir = PathBuf::from("/mocks");

        // The mocks directory itself shouldn't return a domain
        assert_eq!(
            get_domain_from_path(&PathBuf::from("/mocks"), &mocks_dir),
            None
        );
    }

    #[test]
    fn test_get_domain_from_path_with_hyphen() {
        let mocks_dir = PathBuf::from("/mocks");

        assert_eq!(
            get_domain_from_path(
                &PathBuf::from("/mocks/my-api.example.com/init.lua"),
                &mocks_dir
            ),
            Some("my-api.example.com".to_string())
        );
    }

    #[test]
    fn test_get_domain_from_path_localhost() {
        let mocks_dir = PathBuf::from("/mocks");

        assert_eq!(
            get_domain_from_path(&PathBuf::from("/mocks/localhost/init.lua"), &mocks_dir),
            Some("localhost".to_string())
        );
    }

    // ==================== is_lua_file tests ====================

    #[test]
    fn test_is_lua_file() {
        assert!(is_lua_file(Path::new("/mocks/test.lua")));
        assert!(is_lua_file(Path::new("init.lua")));
        assert!(!is_lua_file(Path::new("/mocks/data.json")));
        assert!(!is_lua_file(Path::new("/mocks/readme.md")));
    }

    #[test]
    fn test_is_lua_file_uppercase() {
        // .LUA extension should not match (case sensitive)
        assert!(!is_lua_file(Path::new("test.LUA")));
    }

    #[test]
    fn test_is_lua_file_no_extension() {
        assert!(!is_lua_file(Path::new("Makefile")));
        assert!(!is_lua_file(Path::new("README")));
    }

    #[test]
    fn test_is_lua_file_hidden() {
        assert!(is_lua_file(Path::new(".hidden.lua")));
    }

    #[test]
    fn test_is_lua_file_double_extension() {
        assert!(is_lua_file(Path::new("test.spec.lua")));
        assert!(!is_lua_file(Path::new("test.lua.bak")));
    }

    // ==================== is_init_lua_event tests ====================

    fn make_event(kind: EventKind, path: &str) -> Event {
        Event {
            kind,
            paths: vec![PathBuf::from(path)],
            attrs: Default::default(),
        }
    }

    #[test]
    fn test_is_init_lua_event_create() {
        let event = make_event(
            EventKind::Create(CreateKind::File),
            "/mocks/test.com/init.lua",
        );
        assert!(is_init_lua_event(
            &event,
            Path::new("/mocks/test.com/init.lua")
        ));
    }

    #[test]
    fn test_is_init_lua_event_remove() {
        let event = make_event(
            EventKind::Remove(RemoveKind::File),
            "/mocks/test.com/init.lua",
        );
        assert!(is_init_lua_event(
            &event,
            Path::new("/mocks/test.com/init.lua")
        ));
    }

    #[test]
    fn test_is_init_lua_event_modify_returns_false() {
        let event = make_event(
            EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            "/mocks/test.com/init.lua",
        );
        // Modify events for init.lua should return false (we use is_lua_file for those)
        assert!(!is_init_lua_event(
            &event,
            Path::new("/mocks/test.com/init.lua")
        ));
    }

    #[test]
    fn test_is_init_lua_event_other_lua_file() {
        let event = make_event(
            EventKind::Create(CreateKind::File),
            "/mocks/test.com/helpers.lua",
        );
        assert!(!is_init_lua_event(
            &event,
            Path::new("/mocks/test.com/helpers.lua")
        ));
    }

    #[test]
    fn test_is_init_lua_event_non_lua_file() {
        let event = make_event(
            EventKind::Create(CreateKind::File),
            "/mocks/test.com/data.json",
        );
        assert!(!is_init_lua_event(
            &event,
            Path::new("/mocks/test.com/data.json")
        ));
    }

    #[test]
    fn test_is_init_lua_event_init_in_subdirectory() {
        // init.lua in a subdirectory should still match
        let event = make_event(
            EventKind::Create(CreateKind::File),
            "/mocks/test.com/routes/init.lua",
        );
        assert!(is_init_lua_event(
            &event,
            Path::new("/mocks/test.com/routes/init.lua")
        ));
    }

    // ==================== DEBOUNCE_DURATION constant test ====================

    #[test]
    fn test_debounce_duration_value() {
        assert_eq!(DEBOUNCE_DURATION, Duration::from_millis(100));
    }

    // ==================== start_idle_flusher interval calculation ====================

    #[test]
    fn test_idle_flusher_interval_half_timeout() {
        // The check interval should be half the idle timeout
        let idle_timeout = Duration::from_secs(120);
        let expected_interval = Duration::from_secs(60);

        // Calculate what start_idle_flusher would use
        let check_interval = (idle_timeout / 2).max(Duration::from_secs(30));
        assert_eq!(check_interval, expected_interval);
    }

    #[test]
    fn test_idle_flusher_interval_minimum_30_seconds() {
        // If half the timeout is less than 30 seconds, use 30 seconds
        let idle_timeout = Duration::from_secs(40);
        let check_interval = (idle_timeout / 2).max(Duration::from_secs(30));
        assert_eq!(check_interval, Duration::from_secs(30));
    }

    #[test]
    fn test_idle_flusher_interval_exactly_60_seconds() {
        // If timeout is exactly 60 seconds, interval should be 30 seconds
        let idle_timeout = Duration::from_secs(60);
        let check_interval = (idle_timeout / 2).max(Duration::from_secs(30));
        assert_eq!(check_interval, Duration::from_secs(30));
    }

    #[test]
    fn test_idle_flusher_interval_large_timeout() {
        // Large timeout should result in half
        let idle_timeout = Duration::from_secs(3600); // 1 hour
        let check_interval = (idle_timeout / 2).max(Duration::from_secs(30));
        assert_eq!(check_interval, Duration::from_secs(1800)); // 30 minutes
    }

    // ==================== retention cleanup interval test ====================

    #[test]
    fn test_retention_check_interval_is_one_hour() {
        assert_eq!(RETENTION_CHECK_INTERVAL, Duration::from_secs(3600));
    }

    #[tokio::test(start_paused = true)]
    async fn test_start_retention_cleanup_runs() {
        use crate::db::RequestRecord;
        use crate::db::{init_database, store_request};
        use chrono::{Duration as ChronoDuration, Utc};
        use serde_json::json;
        use uuid::Uuid;

        let conn = init_database(":memory:", 2).await.unwrap();
        let conn = Arc::new(conn);

        // Store a request backdated to 10 days ago
        let old_request = RequestRecord {
            id: Uuid::new_v4(),
            domain: "example.com".to_string(),
            method: "GET".to_string(),
            path: "/old".to_string(),
            query_string: None,
            headers: json!({}),
            body: None,
            received_at: Utc::now() - ChronoDuration::days(10),
        };
        store_request(&conn, &old_request).await.unwrap();

        // Store a recent request
        let new_request = RequestRecord {
            id: Uuid::new_v4(),
            domain: "example.com".to_string(),
            method: "GET".to_string(),
            path: "/new".to_string(),
            query_string: None,
            headers: json!({}),
            body: None,
            received_at: Utc::now(),
        };
        store_request(&conn, &new_request).await.unwrap();

        // Start retention cleanup with 7-day retention
        let _handle = start_retention_cleanup(conn.clone(), 7);

        // Advance time past the check interval (1 hour)
        tokio::time::advance(Duration::from_secs(3601)).await;
        // Yield to let the spawned task run
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // Verify old request was deleted
        let remaining = crate::db::get_requests(&conn, crate::db::RequestQuery::default())
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].path, "/new");
    }

    #[tokio::test]
    async fn test_start_idle_flusher_flushes_domains() {
        use crate::lua::ScriptManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let domain_dir = temp_dir.path().join("idle-test.example.com");
        std::fs::create_dir_all(&domain_dir).unwrap();
        std::fs::write(
            domain_dir.join("init.lua"),
            "function handle(r) return {status=200} end",
        )
        .unwrap();

        let manager = Arc::new(ScriptManager::new(
            temp_dir.path().to_path_buf(),
            64,
            Duration::from_secs(30),
        ));
        manager.load_domain("idle-test.example.com").await.unwrap();
        assert!(manager.is_loaded("idle-test.example.com"));

        // Directly test the flush logic with Duration::ZERO (flushes everything idle)
        let flushed = manager.flush_idle_domains(Duration::ZERO);
        assert!(
            flushed.contains(&"idle-test.example.com".to_string()),
            "Domain should be flushed"
        );
        assert!(
            !manager.is_loaded("idle-test.example.com"),
            "Idle domain should have been flushed"
        );
    }
}
