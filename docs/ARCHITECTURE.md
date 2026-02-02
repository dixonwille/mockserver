# Architecture

Design rationale and technology choices for the mock server.

## Goals

- Accept HTTP requests on multiple domains and route to appropriate Lua scripts
- Hot-reload Lua scripts without server restart
- Store requests/responses in SQLite for inspection
- Simple deployment via single binary or Docker
- Good developer experience with IDE support

## Non-Goals

- High performance (flexibility over speed)
- Horizontal scaling (single instance is sufficient)
- Production-grade reliability
- Complex authentication

## Why Rust?

1. **Static binaries** - Single binary deployment, no runtime dependencies
2. **Memory efficiency** - Low baseline resource usage
3. **Type safety** - Catches errors at compile time
4. **Ecosystem** - Axum, tokio, and mlua are mature and well-documented

## Why Lua?

1. **Familiar syntax** - Easy to learn, similar to JavaScript/Python
2. **mlua integration** - Native async/await with Rust
3. **Sandboxing** - Can restrict what scripts access
4. **Hot reload** - Scripts reload without recompiling

Lua 5.5 was chosen over LuaJIT for modern features (integers, const locals, generational GC).

## Key Design Decisions

### Domain-based Routing

Each domain gets its own folder with an `init.lua` entry point. This provides:
- Clear organization (one folder per API)
- Isolation between domains
- Easy to add/remove mocks

### Per-domain Lua Pools

Each domain has a pool of Lua runtimes. This provides:
- Concurrent request handling
- Memory limits per state
- Automatic idle cleanup

### SQLite for Storage

WAL mode SQLite provides:
- No external dependencies
- Sufficient write throughput for dev/test use
- Simple backup (copy the file)

### Hot Reload

File watcher monitors `.lua` files with 100ms debounce. On change:
1. Create fresh Lua state for the domain
2. Load and validate `init.lua`
3. Atomically swap the domain pool
4. Old states drain naturally (in-flight requests complete)

### Filesystem Sandboxing

Lua scripts can only access files within their domain folder:
- `require()` path scoped to domain directory
- `fs.read()` rejects path traversal
- Hidden folders (`.git`, `.mockserver`) never routable

## Component Overview

```
HTTP Request
    |
    v
Domain Extraction (Host header)
    |
    v
Lua Pool Manager --> [Domain Pools]
    |                      |
    v                      v
Execute handle()     SQLite Storage
    |
    v
HTTP Response
```

## Related Documentation

- [Lua Scripting](./LUA_SCRIPTING.md) - Writing mock handlers
- [CLI](./CLI.md) - Command-line options
- [API](./API.md) - Admin API reference
