# Admin API

This document covers the Admin API for querying stored requests and managing the mock server.

## API vs Mock Routing

When the mock server receives a request, it must determine whether to route it to the Admin API or to the Lua mock handler. Three strategies are supported:

### Option A: Separate Ports (Default)

Run the Admin API on a different port than the mock server.

```
Mock requests  -> :3000 -> Lua handler
API requests   -> :3001 -> Admin API (/api/* endpoints)
```

**CLI Usage:**
```bash
mockserver serve --port 3000 --api-port 3001
```

**Example requests:**
```bash
# Mock server (port 3000) - routed to Lua handlers
curl http://localhost:3000/users

# Admin API (port 3001) - all endpoints under /api/
curl http://localhost:3001/api/requests
curl http://localhost:3001/api/health

# Future: Web UI at root of admin port
curl http://localhost:3001/
```

**Pros:**
- Clear separation, no routing ambiguity
- API requests never interfere with mocks
- Simple mental model

**Cons:**
- Requires two ports to be available
- Firewall/proxy configuration needs both ports

**Best for:** Local development, simple deployments

---

### Option B: Path Prefix

Serve both on the same port, with Admin API requests distinguished by a reserved path prefix on the mock port.

```
Requests to /_mockserver/* -> Admin API (prefix stripped, then /api/* routing)
All other requests         -> Lua handler
```

**CLI Usage:**
```bash
mockserver serve --port 3000 --api-prefix /_mockserver
```

**How paths map with --api-prefix:**
```
# Request to mock port with prefix -> Admin API endpoint
GET /_mockserver/api/requests      -> GET /api/requests (list requests)
GET /_mockserver/api/health        -> GET /api/health   (health check)
GET /_mockserver/                  -> (Future) Web UI

# Request to mock port without prefix -> Lua handler
GET /api/users                     -> Lua handler for domain
```

**Pros:**
- Single port simplifies deployment
- Works well behind reverse proxies

**Cons:**
- Cannot mock paths starting with the prefix
- Slightly more complex routing logic

**Best for:** Containerized deployments, when ports are limited

---

### Option C: Domain-Based Routing

Serve both on the same port, with API requests routed based on a dedicated domain.

```
Requests to admin.mock.local      -> Admin API
Requests to *.mock.local          -> Lua handler (domain from subdomain)
Requests to any other domain      -> Lua handler (domain from Host header)
```

**CLI Usage:**
```bash
mockserver serve --port 3000 --api-domain admin.mock.local
```

**Pros:**
- Full path space available for mocking
- Clean separation by domain
- Natural fit for wildcard DNS setups

**Cons:**
- Requires DNS or /etc/hosts configuration
- More complex initial setup
- Domain must be configured in clients

**Best for:** Team environments with shared mock server, CI/CD pipelines with DNS

---

### Routing Decision Flow

```
Request received
    |
    v
Is --api-domain set AND Host matches api-domain?
    |-- Yes --> Admin API Server
    |-- No
        |
        v
    Is --api-prefix set AND path starts with prefix?
        |-- Yes --> Admin API Server (strip prefix first)
        |-- No
            |
            v
        Is this the API port (if separate)?
            |-- Yes --> Admin API Server
            |-- No --> Lua Mock Handler
```

**Admin API Server internal routing:**

```
Request routed to Admin API Server
    |
    v
Path starts with /api/?
    |-- Yes --> API handlers (/api/requests, /api/health, etc.)
    |-- No
        |
        v
    Path is / or /index.html?
        |-- Yes --> (Future) Web UI static files
        |-- No --> 404 Not Found
```

## API Endpoints

All Admin API endpoints are prefixed with `/api/`. This reserves the root path for a future web-based UI.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/requests` | List recorded requests (paginated) |
| GET | `/api/requests/{id}` | Get specific request details |
| GET | `/api/requests/{id}/response` | Get response for a request |
| DELETE | `/api/requests` | Clear all recorded requests |
| POST | `/api/config/reload` | Trigger config reload |
| POST | `/api/cleanup` | Run retention cleanup |
| GET | `/api/health` | Health check |

**Reserved paths (for future use):**

| Path | Purpose |
|------|---------|
| `/` | Future web UI (static files) |
| `/api/*` | Admin API (current) |

## Query Parameters

**For GET /api/requests:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `domain` | string | Filter by domain |
| `path` | string | Filter by path (contains match) |
| `method` | string | Filter by HTTP method |
| `limit` | integer | Results per page (default 50, max 500) |
| `offset` | integer | Pagination offset |

**Example:**
```bash
# Get last 10 requests to api.example.com
curl "http://localhost:3001/api/requests?domain=api.example.com&limit=10"

# Get POST requests
curl "http://localhost:3001/api/requests?method=POST"

# Filter by path containing "users"
curl "http://localhost:3001/api/requests?path=users"
```

## Data Models

### Rust Types

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRecord {
    pub id: Uuid,
    pub domain: String,
    pub method: String,
    pub path: String,
    pub query_string: Option<String>,
    pub headers: serde_json::Value,  // JSON object
    pub body: Option<Vec<u8>>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseRecord {
    pub id: Uuid,
    pub request_id: Uuid,
    pub status_code: u16,
    pub headers: serde_json::Value,  // JSON object
    pub body: Option<Vec<u8>>,
    pub lua_script: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}
```

### SQL Schema

```sql
CREATE TABLE requests (
    id TEXT PRIMARY KEY,           -- UUID
    domain TEXT NOT NULL,
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    query_string TEXT,
    headers TEXT NOT NULL,          -- JSON object
    body BLOB,
    received_at TEXT NOT NULL       -- ISO8601 timestamp
);

CREATE INDEX idx_requests_domain_received ON requests(domain, received_at DESC);
CREATE INDEX idx_requests_received ON requests(received_at DESC);

CREATE TABLE responses (
    id TEXT PRIMARY KEY,            -- UUID
    request_id TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    headers TEXT NOT NULL,          -- JSON object
    body BLOB,
    lua_script TEXT,                -- Which script generated this
    duration_ms INTEGER,            -- Execution time
    error TEXT,                     -- If script failed

    FOREIGN KEY (request_id) REFERENCES requests(id) ON DELETE CASCADE
);

CREATE INDEX idx_responses_request_id ON responses(request_id);
```

## Example API Responses

**GET /api/requests:**

Returns a summary of recorded requests (not the full request details):

```json
{
  "requests": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
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

**GET /api/requests/{id}/response:**
```json
{
  "id": "660e8400-e29b-41d4-a716-446655440001",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "status_code": 201,
  "headers": {
    "content-type": "application/json"
  },
  "body": "{\"id\": 1, \"name\": \"Alice\"}",
  "lua_script": "api.example.com/init.lua",
  "duration_ms": 12,
  "error": null
}
```

**GET /api/health:**
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

**POST /api/config/reload:**
```json
{
  "reloaded": ["api.example.com", "other.example.com"]
}
```

**DELETE /api/requests:**
```json
{
  "deleted": 42
}
```

**POST /api/cleanup:**
```json
{
  "deleted": 15
}
```

## Error Responses

All error responses follow this format:

```json
{
  "error": "<error type>",
  "message": "<detailed message>"
}
```

| HTTP Status | Error Type | When |
|-------------|------------|------|
| 400 | `Invalid UUID` | Invalid ID format for `/api/requests/{id}` |
| 404 | `Not found` | Request or response not found |
| 500 | `Database error` | Database operation failed |

## Related Documentation

- **[CLI](./CLI.md)** - Command-line options for API routing
- **[Architecture](./ARCHITECTURE.md)** - Design rationale
