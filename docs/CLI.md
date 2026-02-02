# Command-Line Interface

This document covers the mock server CLI commands, options, and configuration.

## Configuration Philosophy

**No config file required.** Unlike tools that require YAML/TOML configuration:

1. **CLI flags + env vars are sufficient** for all server settings
2. **Lua scripts ARE the configuration** for mock behavior
3. **Server settings are static** - they don't change at runtime, so a config file adds complexity without benefit
4. **Docker/K8s deployments** pass configuration via env vars anyway

If users strongly prefer a config file, it could be added as a future enhancement, but the MVP focuses on CLI-first design.

## Command Structure

```
mockserver <COMMAND>

Commands:
  serve    Start the mock server
  init     Initialize a new mocks directory with example files
  new      Create a new domain mock file
  check    Validate domains and check for Lua syntax errors
  help     Print help information

Global Options:
  -v, --verbose    Increase logging verbosity (can be repeated: -vvv)
  -q, --quiet      Suppress non-error output
  --version        Print version information
  -h, --help       Print help
```

## Subcommand: serve

Start the mock server with sensible defaults.

```
mockserver serve [OPTIONS]

Options:
  -p, --port <PORT>           Port for mock server [default: 3000]
  -d, --dir <DIR>             Directory containing Lua mock files [default: ./mocks]
      --data-dir <DIR>        Directory for SQLite database [default: ./data]

  API Routing (mutually exclusive):
      --api-port <PORT>       Serve Admin API on separate port [default: 3001]
      --api-prefix <PREFIX>   Serve Admin API at path prefix (disables --api-port)
      --api-domain <DOMAIN>   Serve Admin API at specific domain (disables --api-port)

  Storage:
      --retention <DAYS>      Days to retain request history [default: 7]
      --max-body <BYTES>      Maximum request body size [default: 10485760]
                              Requests exceeding this limit receive 413 Payload Too Large

  Lua:
      --script-timeout <SECS> Lua script execution timeout [default: 30]
      --idle-timeout <MINS>   Flush idle domain Lua states after N minutes [default: 30]
      --lua-memory <MB>       Memory limit per Lua domain state [default: 64]

  Database:
      --db-cache <MB>         SQLite page cache size [default: 64]

  Other:
      --no-watch              Disable hot-reload of Lua files
      --host <HOST>           Bind address [default: 127.0.0.1]
```

**Examples:**

```bash
# Simplest usage - just works for local development
mockserver serve

# Custom port
mockserver serve --port 8080

# Watch a different directory
mockserver serve --dir ./test/mocks

# Single port with path prefix for API
mockserver serve --port 3000 --api-prefix /_api

# Single port with domain-based API routing
mockserver serve --port 3000 --api-domain mock-admin.local

# Bind to all interfaces (for Docker/remote access)
mockserver serve --host 0.0.0.0
```

## Subcommand: init

Initialize a new mocks directory with the folder structure and example files.

```
mockserver init [OPTIONS] [PATH]

Arguments:
  [PATH]    Directory to initialize [default: ./mocks]

Options:
  -f, --force    Update .mockserver/ type definitions and .luarc.json (preserves _default/)
```

**What it creates:**

```
./mocks/
    .mockserver/              # IDE support files (LuaLS type definitions)
        types.lua             # Request, Response type definitions
        json.lua              # json module definition
        log.lua               # log module definition
        delay.lua             # delay module definition
        state.lua             # state module definition
        uuid.lua              # uuid module definition
        time.lua              # time module definition
        fs.lua                # fs module definition
    .luarc.json               # LuaLS workspace configuration
    _default/
        init.lua              # Fallback handler with helpful example
```

The `.mockserver/` folder contains EmmyLua type definitions that enable IDE support (autocomplete, type checking, documentation). See [IDE Support](./IDE_SUPPORT.md) for details.

**Example _default/init.lua:**

```lua
-- _default/init.lua
-- This handler is used when no domain-specific mock is found.

local json = require("json")
local helpers = require("helpers")

function handle(request)
    -- Log the request for debugging
    log.info("Received: " .. request.method .. " " .. request.path)

    -- Return a helpful message
    return helpers.json_response(200, {
        message = "Mock server is running!",
        request = {
            method = request.method,
            path = request.path,
            domain = request.domain
        },
        hint = "Run 'mockserver new " .. request.domain .. "' to create a mock for this domain"
    })
end
```

**Example _default/helpers.lua:**

```lua
-- _default/helpers.lua
-- Common helper functions

local json = require("json")

local M = {}

function M.json_response(status, data)
    return {
        status = status,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode(data)
    }
end

return M
```

## Subcommand: new

Create a new domain mock folder with init.lua and helper files.

```
mockserver new <DOMAIN> [OPTIONS]

Arguments:
  <DOMAIN>    Domain name (e.g., api.example.com)

Options:
  -d, --dir <DIR>      Mocks directory [default: ./mocks]
  -t, --template <T>   Template type: basic, rest, graphql [default: basic]
  -f, --force          Overwrite existing folder
```

**What it creates (all templates):**

```
./mocks/api.example.com/
    init.lua          # Entry point with handle() function
```

The `new` command creates a single `init.lua` file with template-specific content:
- **basic** - Simple handler with JSON response
- **rest** - RESTful routing patterns with CRUD examples
- **graphql** - GraphQL query/mutation handling

**Examples:**

```bash
# Create a basic mock for api.github.com
mockserver new api.github.com
# Creates: ./mocks/api.github.com/init.lua

# Create a REST-style mock with route structure
mockserver new api.stripe.com --template rest
# Creates: ./mocks/api.stripe.com/{init.lua, routes/, templates/}

# Specify custom directory
mockserver new payments.local --dir ./test/mocks
```

**Generated init.lua (basic template):**

```lua
-- api.github.com/init.lua

local json = require("json")
local helpers = require("helpers")

function handle(request)
    log.debug("Handling: " .. request.method .. " " .. request.path)

    -- Add your route handlers here
    if request.path == "/" then
        return helpers.json_response(200, {
            message = "api.github.com mock is working!"
        })
    end

    return helpers.json_response(404, {
        error = "Not Found",
        path = request.path
    })
end
```

## Subcommand: check

Validate domains and check for Lua syntax errors.

```
mockserver check [OPTIONS] [DOMAIN]

Arguments:
  [DOMAIN]    Check only a specific domain (optional)

Options:
  -d, --dir <DIR>    Mocks directory [default: ./mocks]
      --json         Output as JSON
  -b, --brief        Brief output (no detailed error messages)
```

**Note:** The `check` command performs syntax validation only (parses Lua files). It does not execute scripts or call `handle()`.

**Example output:**

```
$ mockserver check
Mocks directory: ./mocks

  Domain                  Status
  ----------------------  -------
  _default                OK
  api.example.com         OK
  api.github.com          OK
  broken.local            ERROR: init.lua syntax error at line 15

3 domains OK, 1 with errors
```

```
$ mockserver check api.example.com
Domain: api.example.com
Status: OK
```

```
$ mockserver check --json
{
  "mocks_dir": "./mocks",
  "domains": [
    {
      "name": "_default",
      "status": "ok"
    },
    {
      "name": "api.example.com",
      "status": "ok"
    },
    {
      "name": "broken.local",
      "status": "error",
      "error": "init.lua: syntax error at line 15"
    }
  ]
}
```

## Sensible Defaults

The following defaults are chosen to minimize configuration for local development:

| Setting | Default | Rationale |
|---------|---------|-----------|
| Port | 3000 | Common development port, unlikely to conflict |
| API Port | 3001 | Adjacent to mock port |
| Mocks Directory | ./mocks | Project-local, obvious location |
| Data Directory | ./data | Project-local, gitignore-friendly |
| Host | 127.0.0.1 | Secure default, localhost only |
| Retention | 7 days | Reasonable for development |
| Max Body | 10MB | Covers most API payloads |
| Script Timeout | 30s | Generous for debugging |
| Idle Timeout | 30 min | Balances memory savings with reload latency |
| Lua Memory | 64MB | Sufficient for most mocks, prevents runaway scripts |
| DB Cache | 64MB | Good balance of performance vs memory usage |

## Environment Variables

All CLI options can also be set via environment variables with a `MOCKSERVER_` prefix:

| CLI Flag | Environment Variable |
|----------|---------------------|
| --port | MOCKSERVER_PORT |
| --api-port | MOCKSERVER_API_PORT |
| --api-prefix | MOCKSERVER_API_PREFIX |
| --api-domain | MOCKSERVER_API_DOMAIN |
| --dir | MOCKSERVER_DIR |
| --data-dir | MOCKSERVER_DATA_DIR |
| --host | MOCKSERVER_HOST |
| --retention | MOCKSERVER_RETENTION |
| --max-body | MOCKSERVER_MAX_BODY |
| --script-timeout | MOCKSERVER_SCRIPT_TIMEOUT |
| --idle-timeout | MOCKSERVER_IDLE_TIMEOUT |
| --lua-memory | MOCKSERVER_LUA_MEMORY |
| --db-cache | MOCKSERVER_DB_CACHE |

**CLI flags take precedence over environment variables.**

### Full Environment Variable Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `MOCKSERVER_PORT` | 3000 | Mock server port |
| `MOCKSERVER_API_PORT` | 3001 | Admin API port |
| `MOCKSERVER_API_PREFIX` | (none) | API path prefix (disables separate port) |
| `MOCKSERVER_API_DOMAIN` | (none) | API domain (disables separate port) |
| `MOCKSERVER_DIR` | `./mocks` | Lua scripts directory |
| `MOCKSERVER_DATA_DIR` | `./data` | SQLite database directory |
| `MOCKSERVER_HOST` | `127.0.0.1` | Bind address |
| `MOCKSERVER_RETENTION` | 7 | Days to keep request history |
| `MOCKSERVER_MAX_BODY` | 10485760 | Maximum request body size (bytes) |
| `MOCKSERVER_SCRIPT_TIMEOUT` | 30 | Lua script timeout (seconds) |
| `MOCKSERVER_IDLE_TIMEOUT` | 30 | Flush idle domains after N minutes (0 = disabled) |
| `MOCKSERVER_LUA_MEMORY` | 64 | Memory limit per Lua domain state (MB) |
| `MOCKSERVER_DB_CACHE` | 64 | SQLite page cache size (MB) |
| `RUST_LOG` | `info` | Log level (tracing format) |

## Related Documentation

- **[Architecture Overview](./ARCHITECTURE.md)** - System design and technology choices
- **[Admin API](./API.md)** - REST API for querying requests
- **[Deployment](./DEPLOYMENT.md)** - Docker and production configuration
