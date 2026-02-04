# IDE Support

Mockserver provides first-class IDE support through [LuaLS (Lua Language Server)](https://github.com/LuaLS/lua-language-server) type definitions. When `mockserver init` runs, it generates EmmyLua annotation files that enable:

- **Autocomplete** for request/response objects and host-provided modules
- **Type checking** to catch errors before running
- **Inline documentation** on hover
- **Go to definition** for module functions

## Generated Files

```
.mockserver/mocks/
    _types/                       # Type definitions (generated, commit to VCS)
        types.lua                 # Request, Response, handle() types
        json.lua                  # json.encode(), json.decode()
        log.lua                   # log.debug(), log.info(), etc.
        delay.lua                 # delay.sleep()
        state.lua                 # state.get(), state.set()
        uuid.lua                  # uuid.v4()
        time.lua                  # time.now(), time.format()
        fs.lua                    # fs.read(), fs.exists()
    .luarc.json                   # LuaLS workspace configuration
```

## Type Definition Files

### types.lua - Core Types

```lua
-- _types/types.lua
-- EmmyLua type definitions for mockserver

---@meta

---The request object passed to handle()
---@class Request
---@field method string HTTP method ("GET", "POST", "PUT", "DELETE", etc.)
---@field path string Request path (e.g., "/api/users/123")
---@field query table<string, string> Query parameters as strings (e.g., {page = "1", limit = "10"}). Use tonumber() to convert numeric values.
---@field headers table<string, string> Request headers (keys are lowercase)
---@field body string Raw request body as string
---@field domain string The domain this request was routed to

---The response object returned from handle()
---@class Response
---@field status? integer HTTP status code (e.g., 200, 404, 500). Defaults to 200 if omitted.
---@field headers? table<string, string> Response headers (optional, preserved as-is without normalization)
---@field body? string Response body (optional)

---Handle an incoming HTTP request
---@param request Request The incoming request
---@return Response response The response to send
function handle(request) end
```

### json.lua - JSON Module

```lua
-- _types/json.lua
-- JSON encoding/decoding module

---@meta

---@class json
---JSON encoding and decoding utilities
local json = {}

---Encode a Lua value to a JSON string
---@param value any The value to encode (table, string, number, boolean, or nil)
---@return string json The JSON-encoded string
---@nodiscard
function json.encode(value) end

---Decode a JSON string to a Lua value
---@param str string The JSON string to decode
---@return any value The decoded Lua value
---@nodiscard
function json.decode(str) end

return json
```

### log.lua - Logging Module

```lua
-- _types/log.lua
-- Logging module for debug output

---@meta

---@class log
---Logging functions that output to the server console
local log = {}

---Log a debug message (only visible with -v flag)
---@param message string The message to log
function log.debug(message) end

---Log an info message
---@param message string The message to log
function log.info(message) end

---Log a warning message
---@param message string The message to log
function log.warn(message) end

---Log an error message
---@param message string The message to log
function log.error(message) end

return log
```

### delay.lua - Delay Module

```lua
-- _types/delay.lua
-- Non-blocking delay functions

---@meta

---@class delay
---Functions for adding delays to responses (useful for testing timeouts)
local delay = {}

---Sleep for the specified number of milliseconds
---This is non-blocking and won't affect other requests
---@param ms integer Milliseconds to sleep
---@async
function delay.sleep(ms) end

return delay
```

### state.lua - State Module

```lua
-- _types/state.lua
-- Persistent key-value storage (per-domain)

---@meta

---@class state
---Per-domain persistent key-value storage.
---Values persist across requests but are isolated per domain.
---
---**Note:** State is stored in memory and will be lost when:
---  - The server restarts
---  - The domain is flushed due to idle timeout (--idle-timeout)
---  - The domain's Lua files are modified (hot reload)
---
---For data that must survive restarts, use the `fs` module to read/write
---JSON files, or store critical state externally.
local state = {}

---Get a value from the state store
---@param key string The key to look up
---@return any|nil value The stored value, or nil if not found
---@nodiscard
function state.get(key) end

---Set a value in the state store
---@param key string The key to store under
---@param value any The value to store (must be JSON-serializable: tables, strings, numbers, booleans, nil). Errors if value contains functions or userdata.
function state.set(key, value) end

---Delete a value from the state store
---@param key string The key to delete
function state.delete(key) end

---Clear all values from the state store
function state.clear() end

return state
```

### uuid.lua - UUID Module

```lua
-- _types/uuid.lua
-- UUID generation

---@meta

---@class uuid
---UUID generation utilities
local uuid = {}

---Generate a new UUID v4 (random)
---@return string uuid The generated UUID (e.g., "550e8400-e29b-41d4-a716-446655440000")
---@nodiscard
function uuid.v4() end

return uuid
```

### time.lua - Time Module

```lua
-- _types/time.lua
-- Time and date utilities

---@meta

---@class time
---Time and date utilities
local time = {}

---Get the current Unix timestamp in seconds
---@return integer timestamp Unix timestamp
---@nodiscard
function time.now() end

---Get the current Unix timestamp in milliseconds
---@return integer timestamp Unix timestamp in milliseconds
---@nodiscard
function time.now_ms() end

---Get the current time as an ISO 8601 string
---@return string formatted ISO 8601 formatted string (e.g., "2025-01-30T10:30:00.123456+00:00")
---@nodiscard
function time.iso8601() end

---Format a timestamp using a custom format string
---@param format string Format string (uses strftime syntax)
---@param timestamp? integer Unix timestamp (defaults to current time)
---@return string formatted Formatted time string
---@nodiscard
function time.format(format, timestamp) end

return time
```

### fs.lua - File System Module

```lua
-- _types/fs.lua
-- Sandboxed file system access (read-only, domain-scoped)

---@meta

---@class fs
---Read-only file system access, sandboxed to the domain folder
local fs = {}

---Read a file's contents as a string
---Path is relative to the domain folder; cannot escape via ../
---@param path string Relative path to the file (e.g., "fixtures/users.json")
---@return string contents The file contents
---@nodiscard
function fs.read(path) end

---Check if a file exists
---@param path string Relative path to check
---@return boolean exists True if the file exists
---@nodiscard
function fs.exists(path) end

return fs
```

## LuaLS Configuration

The `.luarc.json` file configures Lua Language Server for the mocks workspace:

```json
{
  "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
  "workspace": {
    "library": ["_types"],
    "checkThirdParty": false
  },
  "runtime": {
    "version": "Lua 5.5"
  },
  "diagnostics": {
    "globals": ["handle"]
  },
  "hint": {
    "enable": true,
    "setType": true,
    "paramName": "All"
  },
  "type": {
    "castNumberToInteger": true
  }
}
```

**Configuration explained:**

| Setting | Purpose |
|---------|---------|
| `workspace.library` | Points to `_types/` for type definitions |
| `runtime.version` | Enables Lua 5.5 syntax |
| `diagnostics.globals` | Prevents "undefined global" warning for `handle` |
| `hint.enable` | Shows inline type hints |
| `type.castNumberToInteger` | Allows numbers where integers are expected |

## IDE Setup

### Visual Studio Code

1. Install the [Lua extension by sumneko](https://marketplace.visualstudio.com/items?itemName=sumneko.lua)
2. Open the folder containing `.mockserver/mocks/`
3. The `.luarc.json` is automatically detected

**Result:** Full autocomplete, hover documentation, and type checking.

### Neovim

With [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig):

```lua
-- In your Neovim config
require('lspconfig').lua_ls.setup {
  -- LuaLS will automatically detect .luarc.json in the workspace
}
```

Or with [lazy.nvim](https://github.com/folke/lazy.nvim) and [mason.nvim](https://github.com/williamboman/mason.nvim):

```lua
{
  "neovim/nvim-lspconfig",
  dependencies = { "williamboman/mason-lspconfig.nvim" },
  config = function()
    require("mason-lspconfig").setup {
      ensure_installed = { "lua_ls" }
    }
    require("lspconfig").lua_ls.setup {}
  end
}
```

### JetBrains IDEs (IntelliJ, WebStorm, etc.)

1. Install the [EmmyLua plugin](https://plugins.jetbrains.com/plugin/9768-emmylua)
2. Open the folder containing `.mockserver/mocks/`
3. The plugin reads EmmyLua annotations from `_types/`

## Autocomplete in Action

With the type definitions in place, your IDE provides:

**Request object autocomplete:**
```lua
function handle(request)
    request.  -- Shows: method, path, query, headers, body, domain
end
```

**Module autocomplete:**
```lua
local json = require("json")
json.  -- Shows: encode(value), decode(str)
```

**Hover documentation:**
```lua
-- Hovering over json.encode shows:
-- Encode a Lua value to a JSON string
-- @param value any The value to encode
-- @return string json The JSON-encoded string
```

**Type checking:**
```lua
function handle(request)
    return {
        status = "200",  -- Warning: expected integer, got string
        body = 123       -- Warning: expected string|nil, got number
    }
end
```

## Version Control

**The `_types/` folder and `.luarc.json` should be committed to version control.** This ensures:

1. All team members get IDE support immediately
2. CI/CD can run Lua type checking (via `lua-language-server --check=./mocks --checklevel=Warning`)
3. Type definitions stay in sync with the mockserver version

**Recommended `.gitignore`:**
```gitignore
# Mockserver data (not the mocks or type definitions)
.mockserver/data/
*.db

# Don't ignore .mockserver/mocks/ - it contains your mocks and type definitions
# Don't ignore .luarc.json - it configures the IDE
```

## Updating Type Definitions

When mockserver is updated and APIs change, run:

```bash
mockserver init --force
```

This regenerates the `_types/` type definitions while preserving your mock scripts.

Alternatively, mockserver could check version compatibility on startup:

```
$ mockserver serve
Warning: _types/ type definitions are outdated (v1.0.0 vs v1.2.0)
Run 'mockserver init --force' to update IDE support
```

## CI Type Checking

You can run LuaLS in CI to catch type errors before deployment:

```bash
# Install lua-language-server
# Then run type checking on the mocks directory
lua-language-server --check=./.mockserver/mocks --checklevel=Warning
```

This will fail if there are any type errors in your mock scripts.

## Related Documentation

- **[Lua Scripting](./LUA_SCRIPTING.md)** - Writing mock handlers
- **[CLI](./CLI.md)** - The `init` command that generates type definitions
- **[Roadmap](./ROADMAP.md)** - Future `mockserver check` command for CI
