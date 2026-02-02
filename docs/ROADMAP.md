# Roadmap

Future features and open questions.

## Planned Features

### v2.0

- Binary response support (base64 or file references)
- Direct TLS support (`--tls-cert`, `--tls-key`)
- Web UI for request inspection
- Proxy mode (record and replay)

### Future Considerations

- WebSocket mocking
- OpenAPI spec import
- Response templating
- Request matching by JSONPath/regex
- Shared `_shared/` folder for cross-domain utilities
- `http` module for outbound requests from Lua

## Open Questions

1. **Config file support?** Currently CLI-only. TOML config could be added if requested.

2. **Luau option?** mlua supports Luau (Roblox's Lua) with built-in sandboxing.

3. **Request replay?** Replay stored requests against real backends for comparison.

## Version History

### v1.0.0

- Domain-based routing with folder-per-domain
- Hot reload of Lua scripts
- Request/response storage in SQLite
- Admin API for querying data
- IDE support via LuaLS type definitions
- Standard library: json, delay, log, state, uuid, time, fs
