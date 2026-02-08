# Troubleshooting

## Domain Not Found

**Symptom:** Requests return 404 with `"Lua script not found for domain: example.com"`.

**Check these:**

1. **Host header** -- The mock server routes by Host header. Use `curl -H "Host: api.example.com" http://localhost:3000/path`. Behind a proxy, set `X-Forwarded-Host` or `X-Original-Host`.

2. **Folder name matches** -- The domain folder must match the Host header exactly. Domain names are lowercased internally, so `API.Example.com` maps to `api.example.com/`.

3. **init.lua exists** -- Each domain folder must contain `init.lua`.

4. **_default exists** -- If no domain-specific folder matches, requests fall through to `_default/init.lua`. Run `mockserver init` to create it.

5. **Valid hostname** -- Domain headers must contain only alphanumeric characters, dots, and hyphens. Leading dots, underscores, and paths are rejected.

## Lua Syntax Errors

**Symptom:** Domain fails to load or requests return 500.

**Validate before deploying:**

```bash
mockserver check
```

Exit code `0` means all domains are valid. See [CLI.md](./CLI.md) for `--json` and `--brief` options.

**Common check statuses:**

| Status | Meaning |
|--------|---------|
| `ok` | Domain is valid |
| `missing_init` | No `init.lua` in domain folder |
| `missing_handle` | `init.lua` exists but no `handle()` function |
| `error` | Lua syntax error (details in error message) |

Error messages include file and line number:

```
[string "api.example.com/init.lua"]:15: attempt to call a nil value
```

## Hot Reload Not Working

**Check these:**

1. **File is saved** -- Your editor must write the file to disk (not just buffer it).
2. **File is .lua** -- Only `.lua` file changes trigger reloads. JSON fixtures do not.
3. **--no-watch** -- Check that hot reload isn't disabled in your startup command.
4. **Syntax error** -- If the new script has an error, the reload fails silently and the old version continues. Check logs for `WARN Failed to reload domain`.
5. **Debounce** -- Changes are batched with a 100 ms debounce window.
6. **Idle flush** -- Idle domains are unloaded after `--idle-timeout` minutes (default 30). They reload on the next request.

## Memory Limits

**Symptom:** Requests fail with `not enough memory` or scripts abort.

Increase the per-domain memory limit:

```bash
mockserver serve --lua-memory 128
```

Or via environment variable: `MOCKSERVER_LUA_MEMORY=128`.

**Avoid unbounded growth** in your scripts. Use `state.clear()` periodically if you accumulate data.

## Script Timeout

**Symptom:** Requests return 504 or take too long.

Increase the script timeout:

```bash
mockserver serve --script-timeout 60
```

Or via environment variable: `MOCKSERVER_SCRIPT_TIMEOUT=60`.

**Common causes:** Large JSON parsing, recursive algorithms, excessive `delay.sleep()` calls.

## Debugging Tips

1. **Use the log module** -- `log.debug("msg")` appears with `-v`, `log.info("msg")` at default verbosity.

2. **Increase verbosity** -- `mockserver serve -v` (debug), `-vv` (trace), `-q` (quiet).

3. **Query the Admin API** -- Inspect recorded requests:

   ```bash
   curl "http://localhost:3001/api/requests?domain=api.example.com&limit=5"
   ```

   See [API.md](./API.md) for all query parameters and endpoints.

4. **Check health** -- `curl http://localhost:3001/api/healthz` confirms the server is running.

5. **Force reload** -- `curl -X POST http://localhost:3001/api/config/reload` reloads all domains from disk.

## Related Documentation

- [CLI](./CLI.md) -- All flags and defaults
- [Admin API](./API.md) -- Request inspection endpoints
- [Lua Scripting](./LUA_SCRIPTING.md) -- Module APIs and hot reload behavior
