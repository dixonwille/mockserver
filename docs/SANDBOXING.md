# Sandboxing

Lua scripts execute in a restricted environment. This document covers what is allowed, what is blocked, and what the sandbox is (and isn't) designed for.

## Overview

The sandbox provides defense-in-depth for mock scripts:

- Prevents accidental file system or network access
- Isolates domains from each other
- Limits resource consumption (memory, CPU time)

It does **not** provide cryptographic isolation or protection against sophisticated sandbox escapes. See [Intended Use Cases](#intended-use-cases).

## Disabled Functions and Modules

These are completely removed from the Lua environment:

| Removed | Reason |
|---------|--------|
| `io` module | Prevents arbitrary file I/O |
| `loadfile()` | Prevents loading arbitrary Lua files |
| `dofile()` | Prevents executing arbitrary Lua files |
| `load()` | Prevents loading bytecode (potential escape) |
| `debug` module | Can be used to escape the sandbox |
| `package.loadlib` | Prevents loading C modules |

## Restricted os Module

Only safe time functions are available:

| Available | Not Available |
|-----------|---------------|
| `os.date(format, time)` | `os.execute()` |
| `os.time(table)` | `os.exit()` |
| `os.difftime(t2, t1)` | `os.getenv()` |
| | `os.remove()` |
| | `os.rename()` |
| | `os.setlocale()` |
| | `os.tmpname()` |

## Domain Isolation

Each domain gets its own pool of isolated Lua runtimes:

- **Separate Lua states** -- Domain A cannot access Domain B's environment
- **Separate state storage** -- `state.get()`/`state.set()` is scoped per-domain
- **Separate package.path** -- Each domain can only `require()` its own modules
- **Separate fs access** -- `fs.read()` is sandboxed to the domain folder

## Filesystem Restrictions

The `fs` module provides read-only access scoped to the domain folder:

| Function | Behavior |
|----------|----------|
| `fs.read(path)` | Reads file contents; errors on missing file or access violation |
| `fs.exists(path)` | Returns `false` for paths outside the sandbox (does not error) |

Path traversal prevention:

1. Absolute paths rejected (`/` or `\` prefix)
2. `..` sequences rejected
3. Symlinks resolved via canonicalization
4. Final path verified to be within domain folder

## Resource Limits

| Resource | Flag | Default |
|----------|------|---------|
| Memory per domain | `--lua-memory` | 64 MB |
| Script execution time | `--script-timeout` | 30 s |
| Idle domain lifetime | `--idle-timeout` | 30 min |

See [CLI.md](./CLI.md) for all flags and environment variables.

## Intended Use Cases

**Designed for:**

- Local development mocking
- Integration test environments
- CI/CD pipeline testing
- Internal staging environments

**Not designed for:**

- Running untrusted third-party Lua code
- Multi-tenant public services
- Production API traffic
- Cryptographic isolation between tenants

Only run Lua scripts you control or trust.

## Related Documentation

- [Lua Scripting](./LUA_SCRIPTING.md) -- Module APIs and domain isolation details
- [CLI](./CLI.md) -- `--lua-memory`, `--script-timeout`, `--idle-timeout`
