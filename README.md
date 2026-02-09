# mockserver

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Build Status](https://github.com/dixonwille/mockserver/actions/workflows/ci.yml/badge.svg)](https://github.com/dixonwille/mockserver/actions/workflows/ci.yml)

A Lua-powered HTTP mock server for API development and testing.

## Features

- **Lua 5.5 Scripting** -- Write mock handlers in Lua with full async support
- **Hot Reload** -- Scripts reload automatically when files change
- **Per-Domain Isolation** -- Each domain gets its own Lua state with sandboxed modules
- **Request Recording** -- All requests and responses stored in SQLite for inspection
- **Admin API** -- Query recorded requests, trigger reloads, and monitor health
- **IDE Support** -- Type definitions for autocomplete in VS Code, Neovim, and other editors

## Installation

**Nix (run without cloning):**

```bash
nix run github:dixonwille/mockserver
```

**From source:**

```bash
git clone https://github.com/dixonwille/mockserver
cd mockserver
cargo install --path .
```

**GitHub Releases:** Download a prebuilt binary from
[Releases](https://github.com/dixonwille/mockserver/releases).

**Container:** See the [CLI docs](docs/CLI.md) for environment variable configuration.

## Quick Start

```bash
# 1. Initialize the mocks directory (creates _types/, .luarc.json, _default/)
mockserver init

# 2. Create a domain mock
mockserver new api.example.com

# 3. Start the server (mock on :3000, Admin API on :3001)
mockserver serve

# 4. Test it
curl -H "Host: api.example.com" http://localhost:3000/hello
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `mockserver init [path]` | Initialize a new mocks directory |
| `mockserver new <domain>` | Create a new domain mock folder |
| `mockserver serve` | Start the mock server |
| `mockserver check` | Validate Lua scripts for syntax errors |

See [docs/CLI.md](docs/CLI.md) for all flags, environment variables, and defaults.

## Lua Modules

Scripts have access to these built-in modules:

| Module | Description |
|--------|-------------|
| `json` | JSON encoding/decoding |
| `log` | Leveled logging (debug, info, warn, error) |
| `delay` | Non-blocking sleep (`delay.sleep(ms)`) |
| `state` | Per-domain persistent key-value storage |
| `uuid` | UUID v4 generation |
| `time` | Timestamps, ISO 8601, custom formatting |
| `fs` | Sandboxed read-only file access |

See [docs/LUA_SCRIPTING.md](docs/LUA_SCRIPTING.md) for the full API reference and
[examples/](examples/) for runnable patterns.

## Documentation

- [CLI Reference](docs/CLI.md) -- Flags, env vars, defaults
- [Admin API](docs/API.md) -- Endpoints, routing modes, response shapes
- [Lua Scripting](docs/LUA_SCRIPTING.md) -- handle() contract, module APIs, hot reload
- [Sandboxing](docs/SANDBOXING.md) -- Sandbox restrictions, domain isolation
- [IDE Support](docs/IDE_SUPPORT.md) -- Autocomplete setup
- [Troubleshooting](docs/TROUBLESHOOTING.md) -- Common issues and debugging
- [Architecture](docs/ARCHITECTURE.md) -- Design rationale
- [Roadmap](docs/ROADMAP.md) -- Planned features
- [Contributing](CONTRIBUTING.md) -- Development setup and guidelines

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSES/AGPL-3.0-only.txt).
