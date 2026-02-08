# Lua Scripting

Writing mock handlers, module APIs, and runtime behavior.

## Directory Structure

```
.mockserver/mocks/
  _types/                    IDE type definitions (not routable)
  .luarc.json                LuaLS config
  _default/
    init.lua                 Fallback handler for unmatched domains
  api.example.com/
    init.lua                 Entry point — must define handle()
    helpers.lua              Optional helper modules
    fixtures/
      users.json             Non-Lua files readable via fs module
```

**Conventions:**

- Each domain is a **folder** whose name matches the Host header
- Every domain folder **must** contain `init.lua` defining `handle(request)`
- Additional `.lua` files can be loaded with `require()`
- Folders starting with `_` or `.` are never routable

## The handle() Function

Every `init.lua` must define a global `handle` function:

```lua
---@param request Request
---@return Response
function handle(request)
    return { status = 200, body = "OK" }
end
```

### Request Object

| Field | Type | Description |
|-------|------|-------------|
| `method` | string | `"GET"`, `"POST"`, etc. |
| `path` | string | e.g., `"/api/users"` |
| `query` | table\<string, string\> | Query parameters (values are strings) |
| `headers` | table\<string, string\> | Request headers (keys are lowercase) |
| `body` | string | Raw request body |
| `domain` | string | Domain this request was routed to |

### Response Object

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `status` | integer | `200` | HTTP status code |
| `headers` | table\<string, string\> | (none) | Response headers |
| `body` | string | (none) | Response body |

## Standard Library Modules

All modules are available via `require()`. See [examples/](../examples/) for runnable patterns.

### json

| Function | Args | Returns | Description |
|----------|------|---------|-------------|
| `json.encode(value)` | any | string | Encode Lua value to JSON |
| `json.decode(str)` | string | any | Decode JSON string to Lua value |

### log

All functions take a single string argument.

| Function | Description |
|----------|-------------|
| `log.debug(msg)` | Only visible with `-v` flag |
| `log.info(msg)` | Standard log level |
| `log.warn(msg)` | Warning |
| `log.error(msg)` | Error |

### delay

| Function | Args | Description |
|----------|------|-------------|
| `delay.sleep(ms)` | integer (milliseconds) | Non-blocking sleep; does not block other requests |

### state

Per-domain key-value storage. Values persist across requests but are cleared when:

- The server restarts
- The domain's Lua files change (hot reload)
- A reload is triggered via `POST /api/config/reload`
- The domain is unloaded due to `--idle-timeout`

| Function | Args | Returns | Description |
|----------|------|---------|-------------|
| `state.get(key)` | string | any\|nil | Get a stored value |
| `state.set(key, value)` | string, any | -- | Store a value (must be JSON-serializable) |
| `state.delete(key)` | string | -- | Delete a value |
| `state.clear()` | -- | -- | Clear all state for this domain |

### uuid

| Function | Returns | Description |
|----------|---------|-------------|
| `uuid.v4()` | string | Random UUID v4 (e.g., `"550e8400-..."`) |

### time

| Function | Args | Returns | Description |
|----------|------|---------|-------------|
| `time.now()` | -- | integer | Unix timestamp (seconds) |
| `time.now_ms()` | -- | integer | Unix timestamp (milliseconds) |
| `time.iso8601()` | -- | string | Current time as ISO 8601 |
| `time.format(fmt, ts?)` | string, integer? | string | Custom strftime format |

### fs

Read-only file access, sandboxed to the domain folder.

| Function | Args | Returns | Description |
|----------|------|---------|-------------|
| `fs.read(path)` | string | string | Read file contents (relative to domain folder) |
| `fs.exists(path)` | string | boolean | Check if file exists |

Path traversal (`../`) and absolute paths are blocked. See [SANDBOXING.md](./SANDBOXING.md).

## Module Loading (require)

Each domain has its own isolated `package.path` scoped to its folder:

```lua
require("helpers")          -- loads <domain>/helpers.lua
require("routes.users")     -- loads <domain>/routes/users.lua
require("routes")           -- loads <domain>/routes/init.lua
```

Host-provided modules (`json`, `log`, `delay`, etc.) are always available and take priority. Cross-domain requires are blocked. C module loading is disabled.

## Domain Isolation

Each domain gets a separate Lua state:

| What | Isolated? |
|------|-----------|
| `require()` path | Yes -- can only load from own folder |
| Global variables | Yes -- each domain has its own Lua state |
| `state` module | Yes -- key-value storage is per-domain |
| `fs` module | Yes -- can only read from own folder |

## Hot Reload

When a `.lua` file changes inside the mocks directory:

1. A fresh Lua state is created for the affected domain
2. The new `init.lua` is loaded and validated
3. The domain pool is atomically swapped
4. In-flight requests on the old state drain naturally

Changes are debounced (100 ms). Non-Lua files (JSON, etc.) do not trigger reloads. Disable with `--no-watch` (see [CLI.md](./CLI.md)).

If the new script has a syntax error, the reload fails and the old version continues running. Check logs for `WARN Failed to reload domain`.

## Domain Resolution

The mock server extracts the domain from HTTP headers (priority order):

1. `X-Original-Host`
2. `X-Forwarded-Host`
3. `Host`

The port is stripped (`example.com:8080` -> `example.com`). If no matching domain folder exists, the request falls through to `_default/init.lua`.

## Related Documentation

- [Sandboxing](./SANDBOXING.md) -- Sandbox restrictions and domain isolation details
- [IDE Support](./IDE_SUPPORT.md) -- Autocomplete for module APIs
- [CLI](./CLI.md) -- `--script-timeout`, `--lua-memory`, `--idle-timeout`
- [examples/](../examples/) -- Runnable Lua patterns
