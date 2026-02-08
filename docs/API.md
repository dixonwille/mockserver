# Admin API

Endpoints for querying recorded requests, managing state, and monitoring health.

## Routing Modes

The Admin API can be reached three ways (mutually exclusive):

| Mode | Flag | How it works |
|------|------|-------------|
| Separate port (default) | `--api-port 3001` | API on its own port; mock server on `--port` |
| Path prefix | `--api-prefix /_api` | API under prefix on mock port (e.g., `/_api/api/requests`) |
| Domain | `--api-domain admin.local` | API when Host header matches; all other hosts go to mocks |

See [CLI.md](./CLI.md) for the full flag reference.

## Endpoints

All endpoints are prefixed with `/api/`.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/requests` | List recorded requests (paginated) |
| GET | `/api/requests/{id}` | Get full request details |
| GET | `/api/requests/{id}/response` | Get the response for a request |
| DELETE | `/api/requests` | Clear all recorded requests |
| POST | `/api/config/reload` | Trigger Lua script reload |
| POST | `/api/cleanup` | Run retention cleanup |
| GET | `/api/healthz` | Health check |
| GET | `/api/about` | License and source information |

## GET /api/requests

### Query Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `domain` | string | Filter by domain (exact match) |
| `path` | string | Filter by path (contains match) |
| `method` | string | Filter by HTTP method |
| `limit` | integer | Results per page (default 50, max 500) |
| `offset` | integer | Pagination offset |

### Response

```json
{
  "requests": [
    {
      "id": "550e8400-...",
      "domain": "api.example.com",
      "method": "POST",
      "path": "/users",
      "status": 201,
      "received_at": "2025-01-30T10:30:00Z",
      "duration_ms": 12
    }
  ],
  "total": 1,
  "limit": 50,
  "offset": 0
}
```

## GET /api/requests/{id}

Returns the full request record:

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | UUID |
| `domain` | string | Target domain |
| `method` | string | HTTP method |
| `path` | string | Request path |
| `query_string` | string? | Raw query string |
| `headers` | object | Request headers (JSON) |
| `body` | string? | Raw request body |
| `received_at` | string | ISO 8601 timestamp |

## GET /api/requests/{id}/response

Returns the response that was generated:

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | UUID |
| `request_id` | string | Linked request UUID |
| `status_code` | integer | HTTP status code |
| `headers` | object | Response headers (JSON) |
| `body` | string? | Response body |
| `lua_script` | string | Script that generated this response |
| `duration_ms` | integer | Execution time |
| `error` | string? | Error message if handler failed |

## DELETE /api/requests

Clears all recorded requests and responses.

```json
{ "deleted": 42 }
```

## POST /api/config/reload

Reloads all currently loaded Lua domains from disk.

```json
{ "reloaded": ["api.example.com", "other.example.com"] }
```

## POST /api/cleanup

Runs retention cleanup (deletes requests older than `--retention` days).

```json
{ "deleted": 15 }
```

## GET /api/healthz

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

## GET /api/about

```json
{
  "name": "mockserver",
  "version": "0.1.0",
  "license": "AGPL-3.0-only",
  "source": "https://github.com/dixonwille/mockserver"
}
```

## Error Format

All errors follow this shape:

```json
{
  "error": "<error type>",
  "message": "<detail>"
}
```

| Status | Error | When |
|--------|-------|------|
| 400 | `Invalid UUID` | Bad ID format in `/api/requests/{id}` |
| 404 | `Not found` | Request or response not found |
| 500 | `Database error` | Database operation failed |

## Related Documentation

- [CLI](./CLI.md) -- Flags for API routing modes
- [Troubleshooting](./TROUBLESHOOTING.md) -- Debugging with the Admin API
