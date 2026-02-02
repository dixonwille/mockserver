# Operations Guide

This document covers operational concerns for running mockserver in production environments.

## Configuration Reference

### CLI Flags

| Flag | Environment Variable | Default | Description |
|------|---------------------|---------|-------------|
| `-p, --port` | `MOCKSERVER_PORT` | `3000` | Port for mock server |
| `-d, --dir` | `MOCKSERVER_DIR` | `./mocks` | Directory containing Lua mock files |
| `--data-dir` | `MOCKSERVER_DATA_DIR` | `./data` | Directory for SQLite database |
| `--host` | `MOCKSERVER_HOST` | `127.0.0.1` | Bind address |
| `--api-port` | `MOCKSERVER_API_PORT` | `3001` | Port for Admin API (default mode) |
| `--api-prefix` | `MOCKSERVER_API_PREFIX` | - | Serve Admin API at path prefix (disables --api-port) |
| `--api-domain` | `MOCKSERVER_API_DOMAIN` | - | Serve Admin API at specific domain (disables --api-port) |
| `--retention` | `MOCKSERVER_RETENTION` | `7` | Days to retain request history |
| `--max-body` | `MOCKSERVER_MAX_BODY` | `10485760` | Maximum request body size in bytes (10MB) |
| `--script-timeout` | `MOCKSERVER_SCRIPT_TIMEOUT` | `30` | Lua script execution timeout in seconds |
| `--idle-timeout` | `MOCKSERVER_IDLE_TIMEOUT` | `30` | Flush idle domain Lua states after N minutes (0 to disable) |
| `--lua-memory` | `MOCKSERVER_LUA_MEMORY` | `64` | Memory limit per Lua domain state in MB |
| `--db-cache` | `MOCKSERVER_DB_CACHE` | `64` | SQLite page cache size in MB |
| `--no-watch` | - | `false` | Disable hot-reload of Lua files |

### Environment Variable Configuration

All configuration can be set via environment variables, which is useful for containerized deployments:

```bash
export MOCKSERVER_PORT=8080
export MOCKSERVER_HOST=0.0.0.0
export MOCKSERVER_DIR=/app/mocks
export MOCKSERVER_DATA_DIR=/data
export MOCKSERVER_RETENTION=14
export MOCKSERVER_LUA_MEMORY=128
export MOCKSERVER_DB_CACHE=128

mockserver serve
```

### Configuration Precedence

1. CLI flags (highest priority)
2. Environment variables
3. Default values (lowest priority)

---

## Monitoring and Logging

### Log Levels

Mockserver uses the `tracing` crate for structured logging. Control verbosity with the `RUST_LOG` environment variable:

```bash
# Default (info level)
RUST_LOG=info mockserver serve

# Debug level (includes file watcher events, request details)
RUST_LOG=debug mockserver serve

# Trace level (maximum verbosity)
RUST_LOG=trace mockserver serve

# Module-specific logging
RUST_LOG=mockserver=debug,mockserver::lua=trace mockserver serve

# Quiet mode (errors only)
RUST_LOG=error mockserver serve
```

### Log Format

Logs are output in a human-readable format by default. Key fields include:

- Timestamp
- Log level
- Target module
- Structured fields (domain, request_id, duration_ms, etc.)

Example log output:

```
2026-01-30T10:30:00.123Z  INFO mockserver::serve: Starting mockserver v1.0.0...
2026-01-30T10:30:00.124Z  INFO mockserver::serve: Database: ./data/mockserver.db
2026-01-30T10:30:00.150Z  INFO mockserver::serve: Loaded 3 domain(s): ["_default", "api.example.com", "auth.example.com"]
2026-01-30T10:30:00.151Z  INFO mockserver::serve: Hot reload enabled
2026-01-30T10:30:00.152Z  INFO mockserver::serve: Mock server listening on http://127.0.0.1:3000
2026-01-30T10:30:00.152Z  INFO mockserver::serve: Admin API listening on http://127.0.0.1:3001/api
```

### Key Log Events to Watch

| Event | Level | Description |
|-------|-------|-------------|
| `Starting mockserver` | INFO | Server startup with version |
| `Database initialized` | INFO | Schema version after migrations |
| `Loaded N domain(s)` | INFO | Domains loaded at startup |
| `Mock server listening` | INFO | Server ready to accept requests |
| `Reloaded domain` | INFO | Hot reload triggered for a domain |
| `Failed to reload domain` | WARN | Lua syntax error on reload |
| `Flushed idle domains` | DEBUG | Idle domains removed from memory |
| `Cleaned up old requests` | INFO | Retention cleanup completed |

### Health Check Endpoint

The `/api/health` endpoint provides operational status:

```bash
curl http://localhost:3001/api/health
```

Response:

```json
{
  "status": "healthy",
  "version": "1.0.0",
  "uptime_seconds": 3600,
  "domains_loaded": 5,
  "requests_stored": 1234
}
```

---

## SQLite Maintenance

### Database Location

The database file is stored at `{data_dir}/mockserver.db`:

```bash
# Default location
./data/mockserver.db

# Custom location via CLI
mockserver serve --data-dir /var/lib/mockserver

# The database file will be at:
/var/lib/mockserver/mockserver.db
```

### Schema Overview

The database contains two tables:

**requests** - Stores incoming HTTP requests:

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT | UUID primary key |
| `domain` | TEXT | Target domain |
| `method` | TEXT | HTTP method |
| `path` | TEXT | Request path |
| `query_string` | TEXT | Query parameters (nullable) |
| `headers` | TEXT | JSON object of headers |
| `body` | BLOB | Request body (nullable) |
| `received_at` | TEXT | ISO8601 timestamp |

**responses** - Stores generated responses:

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT | UUID primary key |
| `request_id` | TEXT | Foreign key to requests |
| `status_code` | INTEGER | HTTP status code |
| `headers` | TEXT | JSON object of headers |
| `body` | BLOB | Response body (nullable) |
| `lua_script` | TEXT | Script that generated response |
| `duration_ms` | INTEGER | Execution time |
| `error` | TEXT | Error message if failed |

**Indexes:**

- `idx_requests_domain_received` - For filtering by domain with time ordering
- `idx_requests_received` - For global time-based queries
- `idx_responses_request_id` - For looking up responses by request

### Retention Settings

Request history is automatically cleaned up based on the `--retention` flag:

```bash
# Keep 7 days of history (default)
mockserver serve --retention 7

# Keep 30 days for debugging
mockserver serve --retention 30

# Keep 1 day for high-volume environments
mockserver serve --retention 1
```

### Manual Cleanup

Trigger cleanup manually via the Admin API:

```bash
# Run retention cleanup
curl -X POST http://localhost:3001/api/cleanup
```

Response:

```json
{
  "deleted": 1234
}
```

Clear all recorded data:

```bash
# Delete all requests (cascades to responses)
curl -X DELETE http://localhost:3001/api/requests
```

### Backup Considerations

The SQLite database uses WAL (Write-Ahead Logging) mode. To safely backup:

**Option 1: Use SQLite backup API**

```bash
sqlite3 ./data/mockserver.db ".backup './backup/mockserver.db'"
```

**Option 2: Checkpoint and copy**

```bash
# Force WAL checkpoint
sqlite3 ./data/mockserver.db "PRAGMA wal_checkpoint(TRUNCATE);"

# Copy the main database file
cp ./data/mockserver.db ./backup/
```

**Option 3: Use the Admin API to clear, then backup**

For non-critical data, simply clear and backup periodically:

```bash
curl -X POST http://localhost:3001/api/cleanup
sqlite3 ./data/mockserver.db ".backup './backup/mockserver.db'"
```

### WAL Mode Notes

The database is configured with:

```sql
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA busy_timeout=5000;
PRAGMA foreign_keys=ON;
```

WAL mode provides:

- Better concurrent read/write performance
- Crash recovery (transactions are atomic)
- Readers do not block writers

Files created:

- `mockserver.db` - Main database file
- `mockserver.db-wal` - Write-ahead log (may be large during heavy writes)
- `mockserver.db-shm` - Shared memory file

**Note:** All three files must be present for proper operation. Do not delete the `-wal` or `-shm` files while the server is running.

---

## Resource Tuning

### Memory Limits (--lua-memory)

Each domain gets its own isolated Lua runtime with a memory limit:

```bash
# Default: 64MB per domain
mockserver serve --lua-memory 64

# Increase for complex scripts with large state
mockserver serve --lua-memory 256

# Reduce for many domains with simple scripts
mockserver serve --lua-memory 32
```

**Memory usage estimation:**

```
Total Lua memory = (number of active domains) * (--lua-memory)
```

If you have 10 active domains with 64MB limit each, expect up to 640MB for Lua runtimes.

### Connection Limits (--db-cache)

SQLite page cache improves query performance:

```bash
# Default: 64MB cache
mockserver serve --db-cache 64

# Increase for high query volume
mockserver serve --db-cache 256

# Reduce for memory-constrained environments
mockserver serve --db-cache 16
```

The cache is specified in megabytes and is converted to SQLite's negative KB format internally.

### Timeout Settings

**Script execution timeout:**

```bash
# Default: 30 seconds
mockserver serve --script-timeout 30

# Shorter timeout for fast responses
mockserver serve --script-timeout 5

# Longer timeout for slow external dependencies
mockserver serve --script-timeout 60
```

Scripts exceeding this timeout will be terminated and return a 500 error.

**Maximum request body size:**

```bash
# Default: 10MB
mockserver serve --max-body 10485760

# Increase for large payloads (50MB)
mockserver serve --max-body 52428800

# Reduce for API-only mocking (1MB)
mockserver serve --max-body 1048576
```

### Idle Timeout for Domain Pools

Unused domain Lua runtimes are flushed to save memory:

```bash
# Default: 30 minutes
mockserver serve --idle-timeout 30

# Aggressive flushing (5 minutes)
mockserver serve --idle-timeout 5

# Disable idle flushing (keep all domains loaded)
mockserver serve --idle-timeout 0
```

The idle flusher runs at half the timeout interval (minimum 30 seconds) and:

1. Shrinks idle runtime pools
2. Completely unloads domains that have been cold for the full timeout period

Domains are lazily reloaded on the next request.

---

## Scaling Considerations

### Single Instance Limitations

Mockserver is designed as a single-instance application:

- Single SQLite database (not distributed)
- In-memory Lua state per domain
- File watcher tied to local filesystem

For most mock server use cases (development, testing, CI/CD), a single instance is sufficient.

### Filesystem as Source of Truth

The Lua mock files in the mocks directory are the source of truth:

- No database storage of mock definitions
- Hot reload watches the filesystem
- Changes take effect immediately (100ms debounce)

**Best practices:**

- Store mocks in version control
- Deploy by updating the mocks directory
- Use a shared volume for team environments

### Database Sizing

Estimate database size based on request volume:

| Requests/day | Body size (avg) | Retention | Estimated DB size |
|--------------|-----------------|-----------|-------------------|
| 1,000 | 1KB | 7 days | ~50MB |
| 10,000 | 1KB | 7 days | ~500MB |
| 100,000 | 1KB | 7 days | ~5GB |
| 10,000 | 10KB | 7 days | ~5GB |

**Factors affecting size:**

- Request/response body sizes (stored as BLOBs)
- Number of headers (stored as JSON)
- Retention period

**Mitigation strategies:**

- Reduce retention period
- Run periodic cleanup
- Exclude large bodies in Lua scripts (return minimal responses)

### Memory Planning

Total memory usage components:

| Component | Sizing |
|-----------|--------|
| Base process | ~20-50MB |
| SQLite cache | `--db-cache` value |
| Lua runtimes | `--lua-memory` * active domains |
| Request buffers | `--max-body` * concurrent requests |

**Example calculation for a busy server:**

```
Base:           50MB
DB cache:      128MB
Lua (10 domains * 64MB): 640MB
Buffers (100 concurrent * 1MB): 100MB
------------------------------
Total:         ~920MB
```

---

## Operational Tasks

### Starting/Stopping

**Start the server:**

```bash
# Foreground
mockserver serve

# Background (systemd recommended for production)
mockserver serve &

# With custom configuration
mockserver serve --port 8080 --host 0.0.0.0 --dir /app/mocks
```

**Stop the server:**

- Send `SIGINT` (Ctrl+C) or `SIGTERM` for graceful shutdown
- The server completes in-flight requests before exiting

### Graceful Shutdown

On receiving a termination signal:

1. Stop accepting new connections
2. Complete in-flight requests (with timeout)
3. Flush database writes
4. Exit cleanly

```bash
# Graceful stop
kill -TERM $(pgrep mockserver)

# Force kill (not recommended)
kill -9 $(pgrep mockserver)
```

### Hot Reload Behavior

With hot reload enabled (default), file changes trigger automatic reloading:

**What triggers a reload:**

- Creating, modifying, or deleting `.lua` files
- Creating or deleting `init.lua` (adds/removes a domain)

**What does NOT trigger a reload:**

- Non-Lua files (JSON, markdown, etc.)
- Hidden directories (`.git`, `.mockserver`)

**Debounce behavior:**

- Changes are debounced with a 100ms window
- Multiple rapid changes result in a single reload

**To disable hot reload:**

```bash
mockserver serve --no-watch
```

### Clearing Recorded Data

**Clear all requests and responses:**

```bash
curl -X DELETE http://localhost:3001/api/requests
```

**Run retention cleanup:**

```bash
curl -X POST http://localhost:3001/api/cleanup
```

### Forcing Config Reload

**Reload all loaded domains:**

```bash
curl -X POST http://localhost:3001/api/config/reload
```

Response:

```json
{
  "reloaded": ["_default", "api.example.com", "auth.example.com"]
}
```

This reloads all currently loaded domains from disk. Use this when:

- Hot reload is disabled
- You want to force a reload without file changes
- Debugging Lua loading issues

---

## Related Documentation

- **[CLI](./CLI.md)** - Full command-line reference
- **[API](./API.md)** - Admin API endpoints
- **[Architecture](./ARCHITECTURE.md)** - System design
- **[Lua Scripting](./LUA_SCRIPTING.md)** - Writing mock handlers
