# Command-Line Interface

All CLI flags, environment variables, and defaults.

## Command Structure

```
mockserver <COMMAND>

Commands:
  serve    Start the mock server
  init     Initialize a new mocks directory
  new      Create a new domain mock folder
  check    Validate domains and check for Lua syntax errors
  help     Print help information

Global Options:
  -v, --verbose    Increase logging verbosity (can be repeated: -vvv)
  -q, --quiet      Suppress non-error output
  --version        Print version information
  -h, --help       Print help
```

## serve

Start the mock server.

```bash
mockserver serve [OPTIONS]
```

### Options

| Flag | Env Var | Default | Description |
|------|---------|---------|-------------|
| `-p, --port` | `MOCKSERVER_PORT` | `3000` | Mock server port |
| `-d, --dir` | `MOCKSERVER_DIR` | `./.mockserver/mocks` | Lua scripts directory |
| `--data-dir` | `MOCKSERVER_DATA_DIR` | `./.mockserver/data` | SQLite database directory |
| `--host` | `MOCKSERVER_HOST` | `127.0.0.1` | Bind address |
| `--api-port` | `MOCKSERVER_API_PORT` | `3001` | Admin API port (default routing mode) |
| `--api-prefix` | `MOCKSERVER_API_PREFIX` | -- | Serve Admin API at path prefix (disables `--api-port`) |
| `--api-domain` | `MOCKSERVER_API_DOMAIN` | -- | Serve Admin API at domain (disables `--api-port`). **Note:** not fully implemented |
| `--retention` | `MOCKSERVER_RETENTION` | `7` | Days to retain request history |
| `--max-body` | `MOCKSERVER_MAX_BODY` | `10485760` | Max request body size in bytes (10 MB) |
| `--script-timeout` | `MOCKSERVER_SCRIPT_TIMEOUT` | `30` | Lua script execution timeout in seconds |
| `--idle-timeout` | `MOCKSERVER_IDLE_TIMEOUT` | `30` | Flush idle domain states after N minutes (0 = disabled) |
| `--lua-memory` | `MOCKSERVER_LUA_MEMORY` | `64` | Memory limit per Lua state in MB |
| `--db-cache` | `MOCKSERVER_DB_CACHE` | `64` | SQLite page cache size in MB |
| `--no-watch` | -- | `false` | Disable hot-reload of Lua files |

CLI flags take precedence over environment variables, which take precedence over defaults.

### Examples

```bash
# Local development — just works
mockserver serve

# Custom port
mockserver serve --port 8080

# Single port with path prefix for API
mockserver serve --api-prefix /_api

# Bind to all interfaces (Docker/remote)
mockserver serve --host 0.0.0.0
```

## init

Initialize a new mocks directory with type definitions and a default handler.

```bash
mockserver init [OPTIONS] [PATH]

Arguments:
  [PATH]    Directory to initialize [default: ./.mockserver/mocks]

Options:
  -f, --force    Regenerate _types/ and .luarc.json (preserves _default/)
```

Creates:

```
<PATH>/
  _types/          Type definitions for IDE support (one .lua per module)
  .luarc.json      LuaLS workspace configuration
  _default/
    init.lua       Fallback handler for unmatched domains
```

Run `mockserver init --force` to update type definitions after upgrading mockserver.

## new

Create a new domain mock folder.

```bash
mockserver new <DOMAIN> [OPTIONS]

Arguments:
  <DOMAIN>    Domain name (e.g., api.example.com)

Options:
  -d, --dir <DIR>         Mocks directory [default: ./.mockserver/mocks]
  -t, --template <TYPE>   Template: basic, rest, graphql [default: basic]
  -f, --force             Overwrite existing folder
```

All templates create a single `init.lua` file with template-specific content.

## check

Validate domains and check for Lua syntax errors. Performs syntax validation only — does not execute scripts or call `handle()`.

```bash
mockserver check [OPTIONS] [DOMAIN]

Arguments:
  [DOMAIN]    Check only a specific domain (optional)

Options:
  -d, --dir <DIR>    Mocks directory [default: ./.mockserver/mocks]
      --json         Output as JSON
  -b, --brief        Brief output (no detailed error messages)
```

Exit codes: `0` = all valid, `1` = one or more errors.

### Text output

```
$ mockserver check
Checking .mockserver/mocks

  ✓ _default
  ✓ api.example.com
  ✗ broken.local - syntax error:
      [string "broken.local/init.lua"]:5: unexpected symbol near 'end'

Checked 3 domain(s): 2 ok, 1 with errors
```

### JSON output

The `--json` flag outputs a flat array:

```json
[
  { "name": "_default", "has_init": true, "status": "ok" },
  { "name": "api.example.com", "has_init": true, "status": "ok" },
  { "name": "broken.local", "has_init": true, "status": "error", "error": "syntax error at line 5" }
]
```

Possible `status` values: `ok`, `missing_init`, `missing_handle`, `error`.

## Sensible Defaults

| Setting | Default | Rationale |
|---------|---------|-----------|
| Port | 3000 | Common development port |
| API Port | 3001 | Adjacent to mock port |
| Mocks Dir | `./.mockserver/mocks` | Hidden folder, out of the way |
| Data Dir | `./.mockserver/data` | Keeps DB alongside mocks |
| Host | `127.0.0.1` | Secure default, localhost only |
| Retention | 7 days | Reasonable for development |
| Max Body | 10 MB | Covers most API payloads |
| Script Timeout | 30 s | Generous for debugging |
| Idle Timeout | 30 min | Balances memory vs reload latency |
| Lua Memory | 64 MB | Sufficient for most mocks |
| DB Cache | 64 MB | Good performance vs memory balance |

## Configuration Philosophy

**No config file required.** CLI flags + env vars cover all server settings. Lua scripts _are_ the mock configuration. If you need a config file, it could be added as a future enhancement.

## Related Documentation

- [Admin API](./API.md) -- Endpoints and routing modes
- [Lua Scripting](./LUA_SCRIPTING.md) -- Writing mock handlers
- [Architecture](./ARCHITECTURE.md) -- Design rationale
