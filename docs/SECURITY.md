# Security Model

This document describes the security architecture and hardening recommendations for mockserver.

## Lua Sandboxing Model

Lua scripts execute in a sandboxed environment that restricts access to dangerous functionality.

### Disabled Functions and Modules

The following are completely removed from the Lua environment:

| Removed | Reason |
|---------|--------|
| `io` module | Prevents arbitrary file I/O |
| `loadfile()` | Prevents loading arbitrary Lua files |
| `dofile()` | Prevents executing arbitrary Lua files |
| `load()` | Prevents loading bytecode (potential sandbox escape) |
| `debug` module | Can be used to escape the sandbox |
| `package.loadlib` | Prevents loading C modules |

### Restricted `os` Module

The `os` module is replaced with a safe subset:

```lua
-- Available:
os.date(format, time)    -- Format timestamps
os.time(table)           -- Get current time / convert to timestamp
os.difftime(t2, t1)      -- Calculate time difference

-- NOT available:
os.execute()             -- Shell command execution
os.exit()                -- Process termination
os.getenv()              -- Environment variable access
os.remove()              -- File deletion
os.rename()              -- File renaming
os.setlocale()           -- Locale modification
os.tmpname()             -- Temp file creation
```

### Package Path Restriction

The `package.path` is restricted to only load modules from the domain's folder:

```lua
-- Allowed:
require("helpers")           -- Loads ./helpers.lua
require("routes/users")      -- Loads ./routes/users.lua
require("routes")            -- Loads ./routes/init.lua

-- NOT allowed:
require("../other-domain/helpers")  -- Blocked by path restriction
```

C module loading is completely disabled (`package.cpath = ""`).

### Memory Limits

Each Lua runtime has a configurable memory limit (default: 64 MB). When exceeded, the script terminates with an error.

Configure via CLI:
```bash
mockserver serve --lua-memory 128  # 128 MB per runtime
```

### Execution Timeout

Scripts have a configurable execution timeout (default: 30 seconds). Long-running scripts are terminated.

Configure via CLI:
```bash
mockserver serve --timeout 60  # 60 second timeout
```

## Domain Isolation

### Per-Domain Lua Runtimes

Each domain gets its own pool of isolated Lua runtimes:

- **Separate Lua states**: Domain A cannot access Domain B's Lua environment
- **Separate state storage**: `state.get()`/`state.set()` is scoped per-domain
- **Separate package.path**: Each domain can only `require()` its own modules

```
mocks/
  api.example.com/     <- Isolated runtime pool
    init.lua
    helpers.lua
  auth.example.com/    <- Separate isolated runtime pool
    init.lua
```

### State Isolation

The `state` module provides key-value storage that is:

- Shared across all runtimes within a single domain (for counters, caching)
- Completely isolated between domains
- Thread-safe via RwLock

```lua
-- In api.example.com/init.lua
state.set("counter", 1)  -- Only visible to api.example.com

-- In auth.example.com/init.lua
state.get("counter")     -- Returns nil (isolated)
```

### Filesystem Sandboxing

The `fs` module restricts file access to the domain's folder:

```lua
-- In api.example.com/init.lua
fs.read("data/users.json")       -- OK: reads ./mocks/api.example.com/data/users.json
fs.read("../auth.example.com/x") -- ERROR: path traversal blocked
fs.read("/etc/passwd")           -- ERROR: absolute paths blocked
```

Path traversal prevention:
1. Absolute paths rejected (starts with `/` or `\`)
2. `..` sequences rejected
3. Symlink resolution with canonicalization
4. Final path verified to be within domain folder

## Admin API Access Patterns

### Default Binding

By default, mockserver binds to `127.0.0.1` (localhost only):

```bash
mockserver serve  # Binds to 127.0.0.1:3000 (mock) and 127.0.0.1:3001 (API)
```

This prevents external access by default. To expose to the network:

```bash
mockserver serve --host 0.0.0.0  # WARNING: Exposes to all interfaces
```

### API Routing Modes

The Admin API supports three routing modes:

#### 1. Separate Port (Default)

Admin API runs on a dedicated port:

```bash
mockserver serve --api-port 3001
```

- Mock server: `http://localhost:3000`
- Admin API: `http://localhost:3001/api/...`

Best for: Firewall-based access control.

#### 2. Path Prefix

Admin API shares the mock port under a path prefix:

```bash
mockserver serve --api-prefix /__mockserver
```

- Mock server: `http://localhost:3000/...`
- Admin API: `http://localhost:3000/__mockserver/api/...`

Best for: Single-port deployments, testing environments.

#### 3. Domain-Based

Admin API responds to a specific Host header:

```bash
mockserver serve --api-domain admin.mockserver.local
```

- Mock server: Any other Host header
- Admin API: `Host: admin.mockserver.local`

Best for: Reverse proxy deployments where you control DNS.

### When to Expose vs Protect

| Environment | Recommendation |
|-------------|----------------|
| Local development | Default settings are fine |
| CI/CD pipelines | Default settings, or path prefix mode |
| Shared test servers | Separate port + firewall, or VPN access only |
| Production (if used) | Separate port, firewalled, authenticated proxy |

## Filesystem Restrictions

### Read-Only Access

The `fs` module only provides read access:

```lua
fs.read(path)    -- Read file contents (string)
fs.exists(path)  -- Check if file exists (boolean)

-- No write/delete operations exist
```

### Path Resolution

All paths are resolved relative to the domain folder:

```
Domain: api.example.com
Base: /mocks/api.example.com/

fs.read("users.json")      -> /mocks/api.example.com/users.json
fs.read("data/list.json")  -> /mocks/api.example.com/data/list.json
```

### Path Traversal Prevention

Multiple layers of defense:

1. **Input validation**: Rejects paths starting with `/`, `\`, or containing `..`
2. **Canonicalization**: Resolves symlinks to actual paths
3. **Prefix check**: Verifies resolved path starts with domain folder

```lua
-- All of these are blocked:
fs.read("/etc/passwd")              -- Absolute path
fs.read("../other-domain/init.lua") -- Path traversal
fs.read("data/../../../etc/passwd") -- Hidden traversal
```

## Production Hardening Checklist

### Network Binding

```bash
# Development (default, safe)
mockserver serve

# Production - bind to specific interface
mockserver serve --host 10.0.0.5 --port 3000
```

Never use `--host 0.0.0.0` in production without additional access controls.

### Admin API Protection

Choose one:

1. **Firewall**: Block Admin API port (3001) from external access
2. **Reverse proxy auth**: Put nginx/Caddy with authentication in front
3. **Path prefix + WAF**: Block `/__mockserver` paths at the edge

Example nginx config:
```nginx
# Block admin API from external access
location /__mockserver {
    allow 10.0.0.0/8;
    deny all;
}
```

### Memory and Timeout Limits

Set conservative limits:

```bash
mockserver serve \
  --lua-memory 32 \      # 32 MB per Lua runtime
  --timeout 10 \         # 10 second script timeout
  --max-body-size 1048576  # 1 MB request body limit
```

### Log Verbosity

Production logging:

```bash
RUST_LOG=mockserver=info mockserver serve
```

Avoid `debug` or `trace` levels in production (may log sensitive data).

### Database Location

Default location is `./data/mockserver.db`. For production:

```bash
mockserver serve --data-dir /var/lib/mockserver
```

Ensure:
- Directory has appropriate permissions (700 or 750)
- Regular backups if request history is important
- Consider tmpfs for ephemeral deployments

### Reverse Proxy Considerations

If behind a reverse proxy:

1. **Trust proxy headers**: mockserver respects `X-Forwarded-Host` and `X-Original-Host`
2. **Validate at proxy**: Ensure proxy validates/sanitizes Host headers
3. **Rate limiting**: Implement at proxy layer
4. **TLS termination**: Proxy handles HTTPS, mockserver runs HTTP internally

Example Caddy config:
```caddy
mock.example.com {
    reverse_proxy localhost:3000 {
        header_up X-Original-Host {host}
    }
}
```

### Request History Retention

Limit stored request history:

```bash
mockserver serve --retention-days 1  # Keep only 1 day
```

Or run periodic cleanup:
```bash
curl -X POST http://localhost:3001/api/cleanup
```

## Security Considerations

### Not Designed for Untrusted Scripts

The Lua sandbox provides defense-in-depth but is **not designed** to run untrusted code from the internet. It is intended to:

- Prevent accidental security issues in mock scripts
- Isolate domains from each other
- Limit resource consumption

It is **not designed** to:

- Run arbitrary user-submitted Lua code safely
- Provide cryptographic isolation between tenants
- Defend against sophisticated sandbox escape attacks

### Intended Use Cases

mockserver is designed for:

- Local development mocking
- Integration test environments
- CI/CD pipeline testing
- Internal staging environments

It is **not** designed for:

- Production API traffic
- Multi-tenant public services
- Running untrusted third-party scripts

### What to Watch For

1. **Script source**: Only run Lua scripts you control or trust
2. **Network exposure**: Minimize external access to both mock and API ports
3. **Resource limits**: Set memory/timeout limits appropriate for your workload
4. **Request logging**: Request bodies are stored in SQLite - consider sensitivity
5. **Domain validation**: Host header is validated but trusted - proxy should validate upstream

### Reporting Security Issues

If you discover a security vulnerability, please report it responsibly by contacting the maintainer directly rather than opening a public issue.
