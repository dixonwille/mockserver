// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Build script for computing version at build time from git state.
//!
//! Version detection logic (in order):
//! 1. If `MOCKSERVER_VERSION` env var is set (e.g., by Nix), use it directly
//! 2. Try `git describe --tags --exact-match HEAD`
//!    - If succeeds and matches `vX.Y.Z`: use tag version (strip `v` prefix)
//! 3. If no tag, compute dev version:
//!    - Get SHA from `git rev-parse --short HEAD`
//!    - Parse Cargo.toml version `X.Y.Z`, increment patch to `Z+1`
//!    - Format as `X.Y.(Z+1)-dev+<sha>` or `X.Y.(Z+1)-dev` if no SHA

use std::process::Command;

fn main() {
    // Tell Cargo to rerun this script if git state changes
    println!("cargo::rerun-if-changed=.git/HEAD");
    println!("cargo::rerun-if-changed=.git/refs/tags");
    println!("cargo::rerun-if-env-changed=MOCKSERVER_VERSION");

    // If MOCKSERVER_VERSION is already set (e.g., by Nix), use it directly
    let version = match std::env::var("MOCKSERVER_VERSION") {
        Ok(v) if !v.is_empty() => v,
        _ => compute_version(),
    };
    println!("cargo::rustc-env=MOCKSERVER_VERSION={version}");
}

fn compute_version() -> String {
    // Try to get exact tag from git
    if let Some(tag) = get_git_exact_tag()
        && let Some(version) = parse_tag_version(&tag)
    {
        return version;
    }

    // No tag found, compute dev version
    compute_dev_version()
}

/// Try to get exact tag from git describe
fn get_git_exact_tag() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--exact-match", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !tag.is_empty() {
            return Some(tag);
        }
    }
    None
}

/// Parse a tag like "v1.2.3" into version "1.2.3"
fn parse_tag_version(tag: &str) -> Option<String> {
    let tag = tag.trim();
    if let Some(version) = tag.strip_prefix('v') {
        // Validate it looks like a semver
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() >= 3 && parts[0].parse::<u32>().is_ok() && parts[1].parse::<u32>().is_ok() {
            // Third part might have pre-release suffix (e.g., "0-rc1")
            let patch_part = parts[2].split('-').next().unwrap_or(parts[2]);
            if patch_part.parse::<u32>().is_ok() {
                return Some(version.to_string());
            }
        }
    }
    None
}

/// Compute a dev version like "0.1.1-dev+abc1234"
fn compute_dev_version() -> String {
    let base_version = get_cargo_version();
    let (major, minor, patch) = parse_semver(&base_version);
    let next_patch = patch + 1;

    match get_git_short_sha() {
        Some(sha) => format!("{major}.{minor}.{next_patch}-dev+{sha}"),
        None => format!("{major}.{minor}.{next_patch}-dev"),
    }
}

/// Get the version from Cargo.toml
fn get_cargo_version() -> String {
    // CARGO_PKG_VERSION is set by Cargo during build
    std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string())
}

/// Parse semver string into (major, minor, patch)
fn parse_semver(version: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = version.split('.').collect();
    let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    // Patch might have pre-release suffix
    let patch_str = parts.get(2).unwrap_or(&"0");
    let patch = patch_str
        .split('-')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (major, minor, patch)
}

/// Try to get short SHA from git
fn get_git_short_sha() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !sha.is_empty() {
            return Some(sha);
        }
    }
    None
}
