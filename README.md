# mockserver

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Build Status](https://img.shields.io/github/actions/workflow/status/user/mockserver/ci.yml?branch=main)](https://github.com/user/mockserver/actions)

A Lua-powered HTTP mock server for API development and testing.

## Features

- **Lua 5.5 Scripting** - Write mock handlers in Lua with full async support
- **Hot Reload** - Scripts reload automatically when files change
- **Per-Domain Isolation** - Each domain gets its own Lua state with sandboxed modules
- **Request Recording** - All requests and responses stored in SQLite for inspection
- **Admin API** - Query recorded requests, trigger reloads, and monitor health
- **IDE Support** - Type definitions for autocomplete in VS Code, Neovim, and other editors

## Installation

### From Cargo

```bash
cargo install mockserver
```

### From Source

```bash
git clone https://github.com/user/mockserver
cd mockserver
cargo build --release
```

The binary will be at `target/release/mockserver`.

### Nix

```bash
nix run github:user/mockserver
```

Or add to your flake inputs:

```nix
{
  inputs.mockserver.url = "github:user/mockserver";
}
```

## Quick Start

### 1. Initialize the mocks directory

```bash
mockserver init
```

This creates the `.mockserver/mocks/` directory with:
- `_types/` - Type definitions for IDE support
- `.luarc.json` - Lua language server configuration
- `_default/init.lua` - Fallback handler for unmatched domains

### 2. Create a domain mock

```bash
mockserver new api.example.com
```

This creates `.mockserver/mocks/api.example.com/init.lua` with a basic handler:

```lua
local json = require("json")

---@param request Request
---@return Response
function handle(request)
    return {
        status = 200,
        headers = {
            ["Content-Type"] = "application/json"
        },
        body = json.encode({
            message = "Hello from api.example.com",
            method = request.method,
            path = request.path
        })
    }
end
```

### 3. Start the server

```bash
mockserver serve
```

The mock server starts on port 3000 (default) and the Admin API on port 3001.

### 4. Test your mock

```bash
curl -H "Host: api.example.com" http://localhost:3000/hello
```

Response:
```json
{
  "message": "Hello from api.example.com",
  "method": "GET",
  "path": "/hello"
}
```

### 5. View recorded requests

```bash
curl http://localhost:3001/api/requests
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `mockserver init [path]` | Initialize a new mocks directory |
| `mockserver new <domain>` | Create a new domain mock folder |
| `mockserver serve` | Start the mock server (uses `.mockserver/mocks` and `.mockserver/data`) |
| `mockserver check` | Validate Lua scripts for syntax errors |

### Serve Options

```bash
mockserver serve [OPTIONS]

Options:
  -p, --port <PORT>              Mock server port [default: 3000]
  -d, --dir <DIR>                Mocks directory [default: ./.mockserver/mocks]
      --api-port <PORT>          Admin API port [default: 3001]
      --api-prefix <PREFIX>      Serve Admin API at path prefix (disables --api-port)
      --api-domain <DOMAIN>      Serve Admin API at domain (disables --api-port)
      --host <HOST>              Bind address [default: 127.0.0.1]
      --retention <DAYS>         Days to retain request history [default: 7]
      --script-timeout <SECS>    Lua script timeout in seconds [default: 30]
      --no-watch                 Disable hot-reload of Lua files
  -v, --verbose                  Increase logging verbosity
  -q, --quiet                    Suppress non-error output
```

## Available Lua Modules

Scripts have access to these built-in modules:

| Module | Description |
|--------|-------------|
| `json` | JSON encoding/decoding |
| `log` | Structured logging |
| `delay` | Async delays for simulating latency |
| `state` | Per-domain persistent state |
| `uuid` | UUID generation |
| `time` | Time and date utilities |
| `fs` | Sandboxed file system access |

Example using multiple modules:

```lua
local json = require("json")
local log = require("log")
local delay = require("delay")
local state = require("state")
local uuid = require("uuid")

function handle(request)
    log.info("Received request", { path = request.path })

    -- Simulate network latency
    delay.ms(100)

    -- Track request count
    local count = state.get("request_count") or 0
    state.set("request_count", count + 1)

    return {
        status = 200,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({
            id = uuid.v4(),
            request_number = count + 1
        })
    }
end
```

## Documentation

**User Guides:**
- [Lua Scripting](docs/LUA_SCRIPTING.md) - Writing mock handlers
- [CLI Reference](docs/CLI.md) - Command-line options
- [Admin API](docs/API.md) - REST API for request inspection
- [Examples](docs/EXAMPLES.md) - Comprehensive patterns and recipes
- [Troubleshooting](docs/TROUBLESHOOTING.md) - Common issues and debugging
- [Testing](docs/TESTING.md) - Integration with test frameworks
- [IDE Support](docs/IDE_SUPPORT.md) - Setting up autocomplete

**Operations:**
- [Deployment](docs/DEPLOYMENT.md) - Docker and production setup
- [Operations](docs/OPERATIONS.md) - Monitoring and maintenance
- [Security](docs/SECURITY.md) - Sandboxing and hardening

**Development:**
- [Contributing](CONTRIBUTING.md) - Development setup and guidelines
- [Architecture](docs/ARCHITECTURE.md) - Design rationale
- [Roadmap](docs/ROADMAP.md) - Planned features

## Templates

When creating a new domain, choose from available templates:

```bash
mockserver new api.example.com --template basic    # Simple echo handler (default)
mockserver new api.example.com --template rest     # RESTful CRUD patterns
mockserver new api.example.com --template graphql  # GraphQL endpoint
```

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSES/AGPL-3.0-only.txt).
