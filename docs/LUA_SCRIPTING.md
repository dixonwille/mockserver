# Lua Scripting Guide

This document covers everything about writing Lua mock handlers for the mock server.

## Directory Structure

Each domain gets its own folder with an `init.lua` entry point. This enables modular organization with helper files, templates, and route-specific handlers.

```
./mocks/                              # Root mocks directory
    .mockserver/                      # IDE type definitions (EXCLUDED from routing/watching)
        types.lua
        json.lua
        ...
    .luarc.json                       # LuaLS config (EXCLUDED from watching)
    _default/                         # Fallback for unmatched domains
        init.lua                      # Entry point (required)
    api.example.com/                  # Routable domain folder
        init.lua                      # Entry point with handle() function
        helpers.lua                   # Shared helper functions
        templates/                    # Reusable response templates
            error.lua
            pagination.lua
    integrator.example.com/           # Another routable domain
        init.lua                      # Entry point
        routes/                       # Route-specific handlers
            users.lua
            orders.lua
            webhooks.lua
        fixtures/                     # Test data
            users.json
./data/                               # Data directory
    mockserver.db                     # SQLite database
```

**Key conventions:**
- Each domain is a **folder**, not a single file
- The folder name is the domain (e.g., `api.example.com/`)
- Each domain folder **must** contain `init.lua` as the entry point
- `init.lua` must define the `handle(request)` function
- Additional `.lua` files can be `require()`d from `init.lua`
- Non-Lua files (JSON fixtures, etc.) can be read via provided APIs
- **Folders starting with `.` (dot) are NEVER routable domains** (e.g., `.mockserver/`, `.git/`)

## Domain Resolution Logic

```
1. Check X-Original-Host header (highest priority - custom proxy header)
2. Check X-Forwarded-Host header (standard proxy header)
3. Check Host header (direct connection)
4. Extract hostname (strip port if present)
5. Validate hostname:
   - Must NOT start with "." (excludes .mockserver, .git, etc.)
   - Must NOT contain path traversal sequences
6. Look for {hostname}/ folder with init.lua in mocks directory
7. Fall back to _default/init.lua if not found
```

**Security note:** Hostnames starting with `.` are always rejected. This prevents:
- `.mockserver` being treated as a routable domain
- `.git`, `.svn`, or other VCS folders being accessible
- Editor/IDE config folders (`.vscode`, `.idea`) being exposed

## Lua Script Interface

Each domain's `init.lua` receives a request object and must return a response object. The script can require other Lua files within its domain folder.

### Request Object

| Field | Type | Description |
|-------|------|-------------|
| `method` | string | HTTP method ("GET", "POST", etc.) |
| `path` | string | Request path (e.g., "/api/users") |
| `query` | table | Query parameters as strings (e.g., `{page = "1", limit = "10"}`) |
| `headers` | table | Request headers (keys are lowercase) |
| `body` | string | Raw request body |
| `domain` | string | The domain this request was routed to |

### Response Object

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `status` | integer | 200 | HTTP status code |
| `headers` | table | (none) | Response headers as key-value pairs |
| `body` | string | (none) | Response body |

## Basic Example

```lua
-- api.example.com/init.lua

-- Require modules from the same domain folder
local helpers = require("helpers")           -- loads ./helpers.lua
local templates = require("templates.error") -- loads ./templates/error.lua

-- Host-provided modules (always available)
local json = require("json")

-- Handler function (required, runs per request)
function handle(request)
    -- request.method      : string ("GET", "POST", etc.)
    -- request.path        : string ("/api/users")
    -- request.query       : table  ({page = "1", limit = "10"})
    -- request.headers     : table  ({["Content-Type"] = "application/json"})
    -- request.body        : string (raw body)
    -- request.domain      : string ("api.example.com")

    if request.path == "/api/users" and request.method == "GET" then
        return {
            status = 200,
            headers = {
                ["Content-Type"] = "application/json"
            },
            body = json.encode({
                users = {
                    {id = 1, name = "Alice"},
                    {id = 2, name = "Bob"}
                }
            })
        }
    end

    -- Use helper from another file
    return templates.not_found(request.path)
end
```

## Modular Example with Route Handlers

```lua
-- integrator.example.com/init.lua

local users = require("routes.users")    -- loads ./routes/users.lua
local orders = require("routes.orders")  -- loads ./routes/orders.lua

function handle(request)
    -- Route to appropriate handler based on path prefix
    if request.path:match("^/users") then
        return users.handle(request)
    elseif request.path:match("^/orders") then
        return orders.handle(request)
    end

    return { status = 404, body = "Not Found" }
end
```

```lua
-- integrator.example.com/routes/users.lua

local json = require("json")
local helpers = require("helpers")  -- Still relative to domain root

local M = {}

function M.handle(request)
    if request.method == "GET" and request.path == "/users" then
        return {
            status = 200,
            headers = helpers.json_headers(),
            body = json.encode({ users = {} })
        }
    end

    -- ... more routes
end

return M
```

## Helper Module Example

```lua
-- api.example.com/helpers.lua

local M = {}

function M.json_headers()
    return { ["Content-Type"] = "application/json" }
end

function M.json_response(status, data)
    local json = require("json")
    return {
        status = status,
        headers = M.json_headers(),
        body = json.encode(data)
    }
end

return M
```

## Template Example

```lua
-- api.example.com/templates/error.lua

local json = require("json")

local M = {}

function M.not_found(path)
    return {
        status = 404,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({
            error = "Not Found",
            message = "No handler for path: " .. path
        })
    }
end

function M.bad_request(message)
    return {
        status = 400,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({
            error = "Bad Request",
            message = message
        })
    }
end

return M
```

## Standard Library Modules

The following modules are provided to Lua scripts:

| Module | Purpose |
|--------|---------|
| `json` | JSON encode/decode (via serde_json) |
| `delay` | Add response delays (async-aware, non-blocking) |
| `log` | Logging to server console (via tracing) |
| `state` | Persistent key-value storage (across requests) |
| `uuid` | Generate UUIDs |
| `time` | Current time, formatting |
| `fs` | Read files from domain folder (read-only, sandboxed) |

### json Module

```lua
local json = require("json")

-- Encode Lua value to JSON string
local str = json.encode({ name = "Alice", age = 30 })

-- Decode JSON string to Lua value
local data = json.decode('{"name": "Bob"}')
```

### delay Module

```lua
local delay = require("delay")

function handle(request)
    -- Non-blocking delay (won't block other requests)
    delay.sleep(1000)  -- Wait 1 second (milliseconds)

    return {
        status = 200,
        body = "Delayed response"
    }
end
```

### log Module

```lua
local log = require("log")

log.debug("Debug message")   -- Only visible with -v flag
log.info("Info message")
log.warn("Warning message")
log.error("Error message")
```

### state Module

Per-domain persistent key-value storage. Values persist across requests but are isolated per domain.

**Note:** State is stored in memory and will be lost when:
- The server restarts
- The domain is flushed due to idle timeout (`--idle-timeout`)
- The domain's Lua files are modified (hot reload)

```lua
local state = require("state")

-- Store a value
state.set("counter", 0)

-- Get a value (returns nil if not found)
local counter = state.get("counter")

-- Delete a value
state.delete("counter")

-- Clear all state
state.clear()
```

### uuid Module

```lua
local uuid = require("uuid")

-- Generate a UUID v4 (random)
local id = uuid.v4()  -- e.g., "550e8400-e29b-41d4-a716-446655440000"
```

### time Module

```lua
local time = require("time")

-- Current Unix timestamp (seconds)
local ts = time.now()

-- Current Unix timestamp (milliseconds)
local ts_ms = time.now_ms()

-- Format as ISO 8601
local iso = time.iso8601()  -- e.g., "2025-01-30T10:30:00.123456+00:00"

-- Custom format (strftime syntax)
local formatted = time.format("%Y-%m-%d", ts)
```

### fs Module

Read-only file system access, sandboxed to the domain folder.

```lua
local fs = require("fs")

-- Read a file's contents
local data = fs.read("fixtures/users.json")  -- Relative to domain folder

-- Check if file exists
if fs.exists("fixtures/users.json") then
    -- ...
end
```

**Security:**
- Only allows reading files within the domain folder
- Blocks path traversal (`../`, absolute paths)
- `fs.read()` returns file contents as a string; throws an error if file not found or access denied
- `fs.exists()` returns `false` for paths outside the sandbox (does not throw)

## Module Loading and Isolation

Each domain has an isolated Lua environment with `package.path` scoped to its folder. This prevents cross-domain requires and provides clean encapsulation.

### How require() Works

When a domain's `init.lua` calls `require("helpers")`:

1. Lua looks for `helpers.lua` in the domain folder
2. For nested modules like `require("routes.users")`, Lua looks for `routes/users.lua`
3. Host-provided modules (`json`, `log`, etc.) are always available
4. Attempting to require files outside the domain folder fails

### Package Path Configuration

```rust
// When loading a domain, set package.path to only include the domain folder
fn configure_package_path(lua: &Lua, domain_dir: &Path) -> LuaResult<()> {
    let package: Table = lua.globals().get("package")?;

    // Only allow requires from the domain folder
    // ./?.lua      - for require("helpers")
    // ./?/init.lua - for require("routes") loading routes/init.lua
    let path = format!(
        "{0}/?.lua;{0}/?/init.lua",
        domain_dir.display()
    );
    package.set("path", path)?;

    // Disable C module loading entirely
    package.set("cpath", "")?;

    Ok(())
}
```

### Domain Isolation

Each domain's Lua state is completely isolated:

| What | Isolated? | Notes |
|------|-----------|-------|
| `require()` path | Yes | Can only load from own domain folder |
| Global variables | Yes | Each domain has its own Lua state |
| `state` module | Yes | Key-value storage is per-domain |
| File system access | Yes | `fs.read()` only works within domain folder |

**Example of isolation:**

```lua
-- In api.example.com/init.lua
require("helpers")           -- OK: loads api.example.com/helpers.lua
require("routes.users")      -- OK: loads api.example.com/routes/users.lua
require("../other.com/init") -- FAILS: path traversal blocked
require("/etc/passwd")       -- FAILS: absolute paths blocked
```

## Reading Non-Lua Files

For fixtures and test data, use the sandboxed `fs` module:

```lua
-- api.example.com/init.lua
local fs = require("fs")

function handle(request)
    if request.path == "/users" then
        -- Read JSON fixture from domain folder
        local data = fs.read("fixtures/users.json")  -- Reads ./fixtures/users.json
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = data
        }
    end
end
```

## Related Documentation

- **[Architecture](./ARCHITECTURE.md)** - Design rationale
- **[IDE Support](./IDE_SUPPORT.md)** - Setting up autocomplete and type checking
- **[Examples](./EXAMPLES.md)** - Comprehensive patterns and recipes
