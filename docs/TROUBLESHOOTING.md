# Troubleshooting Guide

This guide covers common issues, debugging techniques, and performance tuning for mockserver.

## Common Issues

### 1. "Domain not found" Errors

**Symptom:** Requests return 404 with `{"error": "Handler Error", "message": "Lua script not found for domain: example.com"}`

**Causes and Solutions:**

#### Host Header Configuration

The mockserver extracts the domain from HTTP headers in this priority order:

1. `X-Original-Host` (highest priority)
2. `X-Forwarded-Host`
3. `Host` (standard header)

The port is automatically stripped (e.g., `example.com:8080` becomes `example.com`).

```bash
# Explicit domain header
curl -H "Host: api.example.com" http://localhost:3000/users

# Behind a proxy, use X-Forwarded-Host
curl -H "X-Forwarded-Host: api.example.com" http://localhost:3000/users
```

#### Domain Folder Does Not Exist

Check that a folder exists in your mocks directory matching the domain name:

```
.mockserver/mocks/
  _default/
    init.lua
  api.example.com/      <-- Must match Host header exactly
    init.lua
```

Domain names are case-insensitive (lowercased internally), so `API.Example.com` maps to `api.example.com/`.

#### Invalid Domain Characters

Domain headers are validated and must only contain:
- Alphanumeric characters (a-z, 0-9)
- Dots (`.`)
- Hyphens (`-`)

Invalid patterns that will be rejected:
- Leading dot: `.example.com`
- Leading hyphen: `-example.com`
- Double dots: `example..com`
- Underscores: `test_server.local`
- Paths: `example.com/path`
- Longer than 253 characters

#### Fallback to _default

If no domain-specific folder exists, mockserver falls back to `_default/init.lua`. Ensure this exists:

```bash
mockserver init
```

---

### 2. Lua Syntax Errors

**Symptom:** Domain fails to load or requests return 500 errors.

#### Using `mockserver check`

Validate all domains before deploying:

```bash
# Check all domains
mockserver check --dir ./mocks

# Check a specific domain
mockserver check --dir ./mocks api.example.com

# Brief output (one line per domain)
mockserver check --dir ./mocks --brief

# JSON output for CI pipelines
mockserver check --dir ./mocks --json
```

Example output:

```
✓ _default
✓ api.example.com
✗ broken.example.com - syntax error:
    [string "broken.example.com/init.lua"]:5: unexpected symbol near 'end'

Checked 3 domain(s): 2 ok, 1 with errors
```

Exit codes:
- `0` - All domains valid
- `1` - One or more domains have errors

#### Common Validation Errors

| Status | Meaning |
|--------|---------|
| `ok` | Domain is valid |
| `missing_init` | No `init.lua` file in domain folder |
| `missing_handle` | `init.lua` exists but no `handle()` function defined |
| `error` | Lua syntax error (details in error message) |

#### Reading Error Messages

Lua error messages include the file and line number:

```
[string "api.example.com/init.lua"]:15: attempt to call a nil value (global 'undefined_function')
```

This indicates line 15 in `api.example.com/init.lua` is calling a function that does not exist.

---

### 3. Hot Reload Not Working

**Symptom:** Changes to Lua files are not reflected without restart.

#### File Watcher Issues

The file watcher only monitors `.lua` files. Check that:

1. Your editor saves files (not just buffers)
2. The file has a `.lua` extension
3. You are editing files within the mocks directory

The watcher uses a 100ms debounce, so rapid saves are batched.

#### --no-watch Flag

Hot reload can be disabled:

```bash
mockserver serve --no-watch
```

Check your startup command or environment variables:
- Command line: `--no-watch` flag
- No environment variable equivalent (must be command line)

#### Idle Timeout Flushing

Idle domains are unloaded after `--idle-timeout` minutes (default: 30). This is normal behavior - domains reload on next request.

If hot reload seems slow, the domain pool may have been flushed. The first request after flush takes slightly longer.

#### Syntax Errors Prevent Reload

If your Lua file has a syntax error, the reload will fail and the old version continues running. Check logs for:

```
WARN Failed to reload domain (syntax error?)
```

Run `mockserver check` to validate your changes.

---

### 4. Memory Limits Exceeded

**Symptom:** Requests fail with `not enough memory` or Lua scripts abort unexpectedly.

#### --lua-memory-mb Configuration

Each Lua domain state has a memory limit (default: 64 MB):

```bash
# Increase to 128 MB per domain
mockserver serve --lua-memory-mb 128
```

Or via environment variable:

```bash
export MOCKSERVER_LUA_MEMORY=128
mockserver serve
```

#### State Management Best Practices

Avoid unbounded state growth in your Lua scripts:

```lua
-- BAD: Unbounded growth
local all_requests = {}
function handle(request)
    table.insert(all_requests, request)  -- Grows forever!
    return { status = 200 }
end

-- GOOD: Bounded cache with eviction
local cache = {}
local max_cache_size = 100

function handle(request)
    if #cache >= max_cache_size then
        table.remove(cache, 1)  -- Remove oldest
    end
    table.insert(cache, request.path)
    return { status = 200 }
end
```

Use the `state` module for persistent storage with explicit management:

```lua
local state = require("state")

-- Clear state periodically or on specific conditions
if state.get("request_count") > 1000 then
    state.clear()
end
```

---

### 5. Request Timeout

**Symptom:** Requests return 504 Gateway Timeout or take too long.

#### --script-timeout Configuration

The default script timeout is 30 seconds:

```bash
# Increase to 60 seconds
mockserver serve --script-timeout 60
```

Or via environment variable:

```bash
export MOCKSERVER_SCRIPT_TIMEOUT=60
mockserver serve
```

#### Identifying Slow Operations

Use logging to identify bottlenecks:

```lua
local log = require("log")
local time = require("time")

function handle(request)
    local start = time.now()

    -- Your logic here
    local result = expensive_operation()

    log.debug("expensive_operation took " .. (time.now() - start) .. "ms")

    return { status = 200, body = result }
end
```

Common causes of slow scripts:
- Large JSON parsing (`json.decode` on big payloads)
- Complex string operations in loops
- Recursive algorithms without bounds
- Excessive `delay.sleep()` calls

---

## Debugging Techniques

### 1. Using log.info/debug/warn/error

The `log` module provides leveled logging:

```lua
local log = require("log")

function handle(request)
    log.debug("Processing request: " .. request.path)
    log.info("User-Agent: " .. (request.headers["user-agent"] or "unknown"))
    log.warn("Deprecated endpoint called")
    log.error("Something went wrong!")

    return { status = 200 }
end
```

#### Verbosity Flags

Control log output with verbosity flags:

```bash
# Normal output (info and above)
mockserver serve

# Verbose output (debug and above)
mockserver serve -v

# Very verbose (trace and above)
mockserver serve -vv

# Quiet mode (warnings and errors only)
mockserver serve -q
```

#### Log Output Format

Logs include timestamp, level, domain context, and message:

```
2026-02-02T10:30:45.123Z INFO  api.example.com Processing request: /users
2026-02-02T10:30:45.125Z DEBUG api.example.com Cache hit for user 123
```

---

### 2. Checking Stored Requests via Admin API

All requests are recorded and can be queried through the Admin API.

#### GET /api/requests

List recent requests with optional filtering:

```bash
# List all requests
curl http://localhost:3001/api/requests

# Filter by domain
curl "http://localhost:3001/api/requests?domain=api.example.com"

# Filter by method
curl "http://localhost:3001/api/requests?method=POST"

# Filter by path (exact match)
curl "http://localhost:3001/api/requests?path=/api/users"

# Pagination
curl "http://localhost:3001/api/requests?limit=10&offset=20"

# Combine filters
curl "http://localhost:3001/api/requests?domain=api.example.com&method=GET&limit=5"
```

Response format:

```json
{
  "requests": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "domain": "api.example.com",
      "method": "GET",
      "path": "/users",
      "status": 200,
      "received_at": "2026-02-02T10:30:45.123Z",
      "duration_ms": 5
    }
  ],
  "total": 1,
  "limit": 100,
  "offset": 0
}
```

#### GET /api/requests/{id}

Get full request details including headers and body:

```bash
curl http://localhost:3001/api/requests/550e8400-e29b-41d4-a716-446655440000
```

Response includes:
- Full request headers
- Request body (if any)
- Query string
- Timestamp

#### GET /api/requests/{id}/response

Get the response that was returned for a specific request:

```bash
curl http://localhost:3001/api/requests/550e8400-e29b-41d4-a716-446655440000/response
```

Response includes:
- Status code
- Response headers
- Response body
- Lua script path
- Execution duration
- Error message (if handler failed)

---

### 3. Validating with `mockserver check`

#### Syntax Validation

The `check` command validates Lua syntax without starting the server:

```bash
mockserver check --dir ./mocks
```

This:
1. Parses each `init.lua` file
2. Verifies the `handle()` function exists
3. Reports any syntax errors

#### JSON Output for CI

For CI/CD pipelines, use JSON output:

```bash
mockserver check --dir ./mocks --json
```

```json
[
  {
    "name": "_default",
    "has_init": true,
    "status": "ok"
  },
  {
    "name": "api.example.com",
    "has_init": true,
    "status": "error",
    "error": "[string \"api.example.com/init.lua\"]:10: syntax error"
  }
]
```

Example CI usage:

```yaml
# GitHub Actions
- name: Validate mocks
  run: mockserver check --dir ./mocks --json
```

---

## Performance Issues

### 1. Domain Pool Sizing

Each domain maintains a pool of Lua runtimes for concurrent request handling.

#### Pool Behavior

- **Max size:** Number of CPU cores (automatic)
- **Runtimes created on demand:** First request creates first runtime
- **Idle shrinking:** Unused runtimes are released after 30 seconds of inactivity

#### Idle Timeout Tuning

The `--idle-timeout` controls how long unused domain states are kept in memory:

```bash
# Keep domains loaded longer (60 minutes)
mockserver serve --idle-timeout 60

# Aggressive memory reclamation (5 minutes)
mockserver serve --idle-timeout 5

# Disable idle flushing (keep all domains loaded)
mockserver serve --idle-timeout 0
```

Trade-offs:
- **Longer timeout:** Faster response times, higher memory usage
- **Shorter timeout:** Lower memory usage, occasional cold-start latency

#### Memory Considerations

Memory usage scales with:
- Number of loaded domains
- `--lua-memory-mb` per domain
- State stored in each domain

Estimate: `loaded_domains * lua_memory_mb` = peak memory

Monitor with the health endpoint:

```bash
curl http://localhost:3001/api/health
```

---

### 2. SQLite Considerations

Request history is stored in SQLite for querying and debugging.

#### Retention Cleanup

Old requests are automatically cleaned up based on `--retention`:

```bash
# Keep 14 days of history (default: 7)
mockserver serve --retention 14

# Minimal history (1 day)
mockserver serve --retention 1
```

Manual cleanup via API:

```bash
curl -X POST http://localhost:3001/api/cleanup
```

Response:

```json
{
  "deleted": 1523
}
```

#### --db-cache Setting

SQLite page cache improves query performance:

```bash
# Increase cache to 128 MB (default: 64)
mockserver serve --db-cache 128
```

Higher values improve read performance but use more memory.

---

### 3. Request Retention Cleanup

#### --retention days Setting

Configure how long request history is kept:

```bash
# Environment variable
export MOCKSERVER_RETENTION=30
mockserver serve

# Command line
mockserver serve --retention 30
```

Requests older than this are eligible for cleanup.

#### Manual Cleanup via API

Trigger immediate cleanup:

```bash
# Clean up old requests
curl -X POST http://localhost:3001/api/cleanup

# Delete ALL requests (careful!)
curl -X DELETE http://localhost:3001/api/requests
```

#### Automatic Cleanup

Cleanup runs automatically on server startup and periodically during operation. For high-traffic deployments, consider:

1. Lower retention period
2. Schedule periodic `POST /api/cleanup` calls
3. Monitor database size in `./.mockserver/data/mockserver.db`
