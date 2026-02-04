# Testing with mockserver

This document covers integration patterns, CI/CD configuration, and framework-specific examples for using mockserver in your test suites.

## Integration Patterns

### Starting mockserver for Tests

**Basic startup:**

```bash
# Initialize mocks directory (one-time setup)
mockserver init ./test/mocks

# Start the server
mockserver serve --port 3000 --dir ./test/mocks --data-dir ./test/data
```

**Key command-line options:**

| Option | Description | Default |
|--------|-------------|---------|
| `--port` | Mock server port | 3000 |
| `--api-port` | Admin API port | 3001 |
| `--dir` | Lua mocks directory | ./.mockserver/mocks |
| `--data-dir` | SQLite database directory | ./.mockserver/data |
| `--host` | Bind address | 127.0.0.1 |

**Using separate ports for test isolation:**

Run each test suite on different ports to enable parallel execution:

```bash
# Test suite A
mockserver serve --port 4000 --api-port 4001 --data-dir ./data-a

# Test suite B
mockserver serve --port 5000 --api-port 5001 --data-dir ./data-b
```

**Data directory isolation:**

Each mockserver instance maintains its own SQLite database for request recording. Use separate `--data-dir` paths to isolate test data:

```bash
mockserver serve --data-dir /tmp/mockserver-$TEST_RUN_ID
```

### Configuring Applications to Use Mock Endpoints

Your application under test must be configured to send requests to the mock server. The mock server routes requests based on the `Host` header.

**Setting base URLs:**

```bash
# Application configuration
export API_BASE_URL=http://localhost:3000

# Your app should then make requests like:
# GET http://localhost:3000/users
# with Host header: api.example.com
```

**Host header requirements:**

The mock server uses the `Host` header to determine which domain's Lua scripts to execute. Ensure your HTTP client preserves or sets the correct Host header:

```bash
# Using curl with explicit Host header
curl -H "Host: api.example.com" http://localhost:3000/users

# The mock server will look for scripts in:
# ./mocks/api.example.com/init.lua
```

If no matching domain folder exists, the request falls through to `_default/init.lua`.

### Verifying Requests Were Received

The Admin API provides endpoints to query recorded requests. By default, the Admin API runs on port 3001.

**GET /api/requests - List all requests:**

```bash
curl http://localhost:3001/api/requests
```

Response:
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

**Filtering requests:**

| Parameter | Description |
|-----------|-------------|
| `domain` | Filter by domain (exact match) |
| `path` | Filter by path (prefix match) |
| `method` | Filter by HTTP method |
| `limit` | Results per page (default 50, max 500) |
| `offset` | Pagination offset |

```bash
# Filter by domain
curl "http://localhost:3001/api/requests?domain=api.example.com"

# Filter by method and path
curl "http://localhost:3001/api/requests?method=POST&path=/users"

# Paginate results
curl "http://localhost:3001/api/requests?limit=10&offset=20"
```

**GET /api/requests/{id} - Get request details:**

```bash
curl http://localhost:3001/api/requests/550e8400-e29b-41d4-a716-446655440000
```

Response includes full request details:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "domain": "api.example.com",
  "method": "POST",
  "path": "/users",
  "query_string": null,
  "headers": {
    "content-type": "application/json",
    "host": "api.example.com"
  },
  "body": "{\"name\": \"Alice\"}",
  "received_at": "2025-01-30T10:30:00Z"
}
```

**GET /api/requests/{id}/response - Get the response:**

```bash
curl http://localhost:3001/api/requests/550e8400-e29b-41d4-a716-446655440000/response
```

Response:
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

### Cleaning Up Between Tests

**DELETE /api/requests - Clear all recorded requests:**

```bash
curl -X DELETE http://localhost:3001/api/requests
```

Response:
```json
{
  "deleted": 42
}
```

Call this endpoint in your test setup/teardown to ensure a clean state.

**POST /api/cleanup - Run retention cleanup:**

```bash
curl -X POST http://localhost:3001/api/cleanup
```

This removes requests older than the configured retention period (default 7 days). Useful for long-running test environments.

**POST /api/config/reload - Reload Lua scripts:**

```bash
curl -X POST http://localhost:3001/api/config/reload
```

Forces a reload of all Lua scripts. Useful if you modify mocks during test execution.

---

## CI/CD Examples

### GitHub Actions Workflow

```yaml
name: Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      # Install mockserver
      - name: Install mockserver
        run: |
          curl -L https://github.com/your-org/mockserver/releases/latest/download/mockserver-linux-amd64 -o mockserver
          chmod +x mockserver
          sudo mv mockserver /usr/local/bin/

      # Initialize mocks (if not committed to repo)
      - name: Initialize mocks
        run: |
          mockserver init ./mocks

      # Start mockserver in background
      - name: Start mockserver
        run: |
          mockserver serve \
            --port 3000 \
            --api-port 3001 \
            --dir ./mocks \
            --data-dir /tmp/mockserver-data \
            --host 0.0.0.0 &

          # Wait for server to be ready
          for i in {1..30}; do
            if curl -s http://localhost:3001/api/healthz > /dev/null; then
              echo "mockserver is ready"
              break
            fi
            echo "Waiting for mockserver..."
            sleep 1
          done

      # Run your tests
      - name: Run tests
        run: |
          npm test  # or: cargo test, pytest, go test, etc.
        env:
          API_BASE_URL: http://localhost:3000

      # Optional: Upload request logs on failure
      - name: Upload request logs
        if: failure()
        run: |
          curl http://localhost:3001/api/requests > requests.json

      - uses: actions/upload-artifact@v4
        if: failure()
        with:
          name: mockserver-requests
          path: requests.json
```

### Docker Compose for Test Environments

**docker-compose.test.yml:**

```yaml
version: "3.8"

services:
  mockserver:
    image: your-org/mockserver:latest
    ports:
      - "3000:3000"  # Mock server
      - "3001:3001"  # Admin API
    volumes:
      - ./mocks:/mocks:ro
      - mockserver-data:/data
    environment:
      MOCKSERVER_HOST: "0.0.0.0"
      MOCKSERVER_PORT: "3000"
      MOCKSERVER_API_PORT: "3001"
      MOCKSERVER_DIR: "/mocks"
      MOCKSERVER_DATA_DIR: "/data"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3001/api/healthz"]
      interval: 5s
      timeout: 3s
      retries: 5

  app:
    build: .
    depends_on:
      mockserver:
        condition: service_healthy
    environment:
      API_BASE_URL: http://mockserver:3000
    # Your app configuration here

  tests:
    build:
      context: .
      dockerfile: Dockerfile.test
    depends_on:
      mockserver:
        condition: service_healthy
      app:
        condition: service_started
    environment:
      APP_URL: http://app:8080
      MOCKSERVER_API_URL: http://mockserver:3001

volumes:
  mockserver-data:
```

**Running tests:**

```bash
# Start services and run tests
docker-compose -f docker-compose.test.yml up --abort-on-container-exit --exit-code-from tests

# Clean up
docker-compose -f docker-compose.test.yml down -v
```

### Parallel Test Isolation Strategies

**Strategy 1: Port allocation**

Assign unique ports to each parallel test worker:

```bash
# Worker 0
MOCKSERVER_PORT=3000 MOCKSERVER_API_PORT=3001 mockserver serve --data-dir /tmp/worker-0

# Worker 1
MOCKSERVER_PORT=3002 MOCKSERVER_API_PORT=3003 mockserver serve --data-dir /tmp/worker-1

# Worker 2
MOCKSERVER_PORT=3004 MOCKSERVER_API_PORT=3005 mockserver serve --data-dir /tmp/worker-2
```

**Strategy 2: Domain-based isolation**

Use different Host headers per test to isolate request verification:

```bash
# Test A uses: Host: test-a.api.example.com
# Test B uses: Host: test-b.api.example.com

# Query requests for specific test:
curl "http://localhost:3001/api/requests?domain=test-a.api.example.com"
```

Create domain-specific mock folders or use `_default` to handle all:

```
.mockserver/mocks/
    _default/init.lua          # Handles all domains
    test-a.api.example.com/    # Optional: test-specific behavior
    test-b.api.example.com/
```

**Strategy 3: Data directory separation**

Run isolated instances with separate databases:

```bash
# Each test run gets its own data directory
TEST_ID=$(uuidgen)
mockserver serve --data-dir /tmp/mockserver-$TEST_ID

# Clean up after tests
rm -rf /tmp/mockserver-$TEST_ID
```

---

## Framework Examples

### JavaScript/TypeScript (Jest, Vitest)

**Setup and teardown:**

```typescript
// test/setup.ts
import { spawn, ChildProcess } from "child_process";

let mockserver: ChildProcess;

export async function startMockServer(): Promise<void> {
  mockserver = spawn("mockserver", [
    "serve",
    "--port", "3000",
    "--api-port", "3001",
    "--dir", "./test/mocks",
    "--data-dir", "/tmp/mockserver-test",
  ]);

  // Wait for server to be ready
  await waitForServer("http://localhost:3001/api/healthz");
}

export async function stopMockServer(): Promise<void> {
  mockserver?.kill();
}

async function waitForServer(url: string, timeout = 30000): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // Server not ready yet
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error(`Server at ${url} did not become ready`);
}

export async function clearRequests(): Promise<void> {
  await fetch("http://localhost:3001/api/requests", { method: "DELETE" });
}

export async function getRequests(filters?: {
  domain?: string;
  method?: string;
  path?: string;
}): Promise<any[]> {
  const params = new URLSearchParams(filters as Record<string, string>);
  const response = await fetch(`http://localhost:3001/api/requests?${params}`);
  const data = await response.json();
  return data.requests;
}
```

**Jest configuration:**

```typescript
// jest.config.ts
export default {
  globalSetup: "./test/globalSetup.ts",
  globalTeardown: "./test/globalTeardown.ts",
  setupFilesAfterEnv: ["./test/setup.ts"],
};

// test/globalSetup.ts
import { startMockServer } from "./setup";
export default startMockServer;

// test/globalTeardown.ts
import { stopMockServer } from "./setup";
export default stopMockServer;
```

**Example test:**

```typescript
// test/api.test.ts
import { clearRequests, getRequests } from "./setup";

describe("User API", () => {
  beforeEach(async () => {
    await clearRequests();
  });

  it("should create a user", async () => {
    // Make request through your application
    const response = await fetch("http://localhost:3000/users", {
      method: "POST",
      headers: {
        "Host": "api.example.com",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ name: "Alice" }),
    });

    expect(response.status).toBe(201);

    // Verify the request was recorded
    const requests = await getRequests({ method: "POST", path: "/users" });
    expect(requests).toHaveLength(1);
    expect(requests[0].domain).toBe("api.example.com");
  });

  it("should verify request body", async () => {
    await fetch("http://localhost:3000/orders", {
      method: "POST",
      headers: {
        "Host": "api.example.com",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ items: [{ id: 1, quantity: 2 }] }),
    });

    const requests = await getRequests({ path: "/orders" });
    const request = requests[0];

    // Get full request details including body
    const detailResponse = await fetch(
      `http://localhost:3001/api/requests/${request.id}`
    );
    const detail = await detailResponse.json();

    const body = JSON.parse(detail.body);
    expect(body.items).toHaveLength(1);
    expect(body.items[0].quantity).toBe(2);
  });
});
```

### Python (pytest)

**Fixtures for server management:**

```python
# conftest.py
import subprocess
import time
import pytest
import requests

@pytest.fixture(scope="session")
def mockserver():
    """Start mockserver for the test session."""
    proc = subprocess.Popen([
        "mockserver", "serve",
        "--port", "3000",
        "--api-port", "3001",
        "--dir", "./test/mocks",
        "--data-dir", "/tmp/mockserver-pytest",
    ])

    # Wait for server to be ready
    for _ in range(30):
        try:
            resp = requests.get("http://localhost:3001/api/healthz")
            if resp.ok:
                break
        except requests.ConnectionError:
            pass
        time.sleep(1)
    else:
        proc.kill()
        raise RuntimeError("mockserver failed to start")

    yield proc

    proc.terminate()
    proc.wait()


@pytest.fixture
def clear_requests(mockserver):
    """Clear all recorded requests before each test."""
    requests.delete("http://localhost:3001/api/requests")
    yield


def get_requests(domain=None, method=None, path=None):
    """Helper to query recorded requests."""
    params = {}
    if domain:
        params["domain"] = domain
    if method:
        params["method"] = method
    if path:
        params["path"] = path

    resp = requests.get("http://localhost:3001/api/requests", params=params)
    return resp.json()["requests"]


def get_request_detail(request_id):
    """Get full details for a specific request."""
    resp = requests.get(f"http://localhost:3001/api/requests/{request_id}")
    return resp.json()
```

**Example test:**

```python
# test_api.py
import json
import requests
from conftest import get_requests, get_request_detail


class TestUserAPI:
    def test_create_user(self, clear_requests):
        # Make request to mock server
        response = requests.post(
            "http://localhost:3000/users",
            headers={"Host": "api.example.com"},
            json={"name": "Alice", "email": "alice@example.com"},
        )

        assert response.status_code == 201

        # Verify request was recorded
        recorded = get_requests(method="POST", path="/users")
        assert len(recorded) == 1
        assert recorded[0]["domain"] == "api.example.com"

    def test_request_body_verification(self, clear_requests):
        payload = {"items": [{"id": 1, "qty": 5}], "total": 99.99}

        requests.post(
            "http://localhost:3000/orders",
            headers={"Host": "api.example.com"},
            json=payload,
        )

        recorded = get_requests(path="/orders")
        detail = get_request_detail(recorded[0]["id"])

        body = json.loads(detail["body"])
        assert body["items"][0]["qty"] == 5
        assert body["total"] == 99.99

    def test_multiple_requests(self, clear_requests):
        # Make several requests
        for i in range(5):
            requests.get(
                f"http://localhost:3000/users/{i}",
                headers={"Host": "api.example.com"},
            )

        recorded = get_requests(domain="api.example.com")
        assert len(recorded) == 5
```

### Go (testing package)

**Test helpers:**

```go
// testutil/mockserver.go
package testutil

import (
    "encoding/json"
    "fmt"
    "net/http"
    "net/url"
    "os"
    "os/exec"
    "testing"
    "time"
)

const (
    MockServerURL = "http://localhost:3000"
    AdminAPIURL   = "http://localhost:3001"
)

type MockServer struct {
    cmd *exec.Cmd
}

func StartMockServer(t *testing.T) *MockServer {
    t.Helper()

    cmd := exec.Command("mockserver", "serve",
        "--port", "3000",
        "--api-port", "3001",
        "--dir", "./testdata/mocks",
        "--data-dir", "/tmp/mockserver-go-test",
    )
    cmd.Stdout = os.Stdout
    cmd.Stderr = os.Stderr

    if err := cmd.Start(); err != nil {
        t.Fatalf("Failed to start mockserver: %v", err)
    }

    // Wait for server to be ready
    deadline := time.Now().Add(30 * time.Second)
    for time.Now().Before(deadline) {
        resp, err := http.Get(AdminAPIURL + "/api/healthz")
        if err == nil && resp.StatusCode == 200 {
            resp.Body.Close()
            return &MockServer{cmd: cmd}
        }
        time.Sleep(100 * time.Millisecond)
    }

    cmd.Process.Kill()
    t.Fatal("mockserver failed to start within timeout")
    return nil
}

func (m *MockServer) Stop() {
    if m.cmd != nil && m.cmd.Process != nil {
        m.cmd.Process.Kill()
        m.cmd.Wait()
    }
}

func ClearRequests(t *testing.T) {
    t.Helper()

    req, _ := http.NewRequest(http.MethodDelete, AdminAPIURL+"/api/requests", nil)
    resp, err := http.DefaultClient.Do(req)
    if err != nil {
        t.Fatalf("Failed to clear requests: %v", err)
    }
    resp.Body.Close()
}

type RequestSummary struct {
    ID         string `json:"id"`
    Domain     string `json:"domain"`
    Method     string `json:"method"`
    Path       string `json:"path"`
    Status     *int   `json:"status"`
    ReceivedAt string `json:"received_at"`
}

type ListResponse struct {
    Requests []RequestSummary `json:"requests"`
    Total    int              `json:"total"`
}

func GetRequests(t *testing.T, domain, method, path string) []RequestSummary {
    t.Helper()

    params := url.Values{}
    if domain != "" {
        params.Set("domain", domain)
    }
    if method != "" {
        params.Set("method", method)
    }
    if path != "" {
        params.Set("path", path)
    }

    resp, err := http.Get(AdminAPIURL + "/api/requests?" + params.Encode())
    if err != nil {
        t.Fatalf("Failed to get requests: %v", err)
    }
    defer resp.Body.Close()

    var result ListResponse
    if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
        t.Fatalf("Failed to decode response: %v", err)
    }

    return result.Requests
}
```

**Example test:**

```go
// api_test.go
package myapp_test

import (
    "bytes"
    "encoding/json"
    "net/http"
    "os"
    "testing"

    "myapp/testutil"
)

var mockServer *testutil.MockServer

func TestMain(m *testing.M) {
    // Start mockserver once for all tests
    mockServer = testutil.StartMockServer(&testing.T{})
    code := m.Run()
    mockServer.Stop()
    os.Exit(code)
}

func TestCreateUser(t *testing.T) {
    testutil.ClearRequests(t)

    // Make request to mock server
    payload := map[string]string{"name": "Alice"}
    body, _ := json.Marshal(payload)

    req, _ := http.NewRequest(http.MethodPost, testutil.MockServerURL+"/users", bytes.NewReader(body))
    req.Header.Set("Host", "api.example.com")
    req.Header.Set("Content-Type", "application/json")

    resp, err := http.DefaultClient.Do(req)
    if err != nil {
        t.Fatalf("Request failed: %v", err)
    }
    defer resp.Body.Close()

    if resp.StatusCode != http.StatusCreated {
        t.Errorf("Expected 201, got %d", resp.StatusCode)
    }

    // Verify request was recorded
    requests := testutil.GetRequests(t, "api.example.com", "POST", "/users")
    if len(requests) != 1 {
        t.Errorf("Expected 1 request, got %d", len(requests))
    }
}

func TestMultipleRequests(t *testing.T) {
    testutil.ClearRequests(t)

    // Make several requests
    for i := 0; i < 5; i++ {
        req, _ := http.NewRequest(http.MethodGet,
            testutil.MockServerURL+"/users/"+string(rune('0'+i)), nil)
        req.Header.Set("Host", "api.example.com")

        resp, _ := http.DefaultClient.Do(req)
        resp.Body.Close()
    }

    requests := testutil.GetRequests(t, "api.example.com", "GET", "")
    if len(requests) != 5 {
        t.Errorf("Expected 5 requests, got %d", len(requests))
    }
}
```

### Rust (cargo test)

**Integration test setup:**

```rust
// tests/common/mod.rs
use reqwest::Client;
use serde::Deserialize;
use std::process::{Child, Command};
use std::time::Duration;

pub const MOCK_SERVER_URL: &str = "http://localhost:3000";
pub const ADMIN_API_URL: &str = "http://localhost:3001";

pub struct MockServer {
    process: Child,
    client: Client,
}

impl MockServer {
    pub async fn start() -> Self {
        let process = Command::new("mockserver")
            .args([
                "serve",
                "--port", "3000",
                "--api-port", "3001",
                "--dir", "./tests/mocks",
                "--data-dir", "/tmp/mockserver-rust-test",
            ])
            .spawn()
            .expect("Failed to start mockserver");

        let client = Client::new();

        // Wait for server to be ready
        for _ in 0..30 {
            if let Ok(resp) = client
                .get(format!("{}/api/healthz", ADMIN_API_URL))
                .send()
                .await
            {
                if resp.status().is_success() {
                    return Self { process, client };
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        panic!("mockserver failed to start");
    }

    pub async fn clear_requests(&self) {
        self.client
            .delete(format!("{}/api/requests", ADMIN_API_URL))
            .send()
            .await
            .expect("Failed to clear requests");
    }

    pub async fn get_requests(
        &self,
        domain: Option<&str>,
        method: Option<&str>,
        path: Option<&str>,
    ) -> Vec<RequestSummary> {
        let mut url = format!("{}/api/requests", ADMIN_API_URL);
        let mut params = vec![];

        if let Some(d) = domain {
            params.push(format!("domain={}", d));
        }
        if let Some(m) = method {
            params.push(format!("method={}", m));
        }
        if let Some(p) = path {
            params.push(format!("path={}", p));
        }

        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }

        let resp: ListResponse = self.client
            .get(&url)
            .send()
            .await
            .expect("Failed to get requests")
            .json()
            .await
            .expect("Failed to parse response");

        resp.requests
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

#[derive(Debug, Deserialize)]
pub struct RequestSummary {
    pub id: String,
    pub domain: String,
    pub method: String,
    pub path: String,
    pub status: Option<u16>,
    pub received_at: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    requests: Vec<RequestSummary>,
    total: usize,
}
```

**Example test:**

```rust
// tests/api_test.rs
mod common;

use common::{MockServer, MOCK_SERVER_URL};
use reqwest::Client;
use serde_json::json;

#[tokio::test]
async fn test_create_user() {
    let server = MockServer::start().await;
    server.clear_requests().await;

    let client = Client::new();

    // Make request to mock server
    let response = client
        .post(format!("{}/users", MOCK_SERVER_URL))
        .header("Host", "api.example.com")
        .json(&json!({"name": "Alice"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    // Verify request was recorded
    let requests = server
        .get_requests(Some("api.example.com"), Some("POST"), Some("/users"))
        .await;

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/users");
}

#[tokio::test]
async fn test_request_isolation() {
    let server = MockServer::start().await;
    server.clear_requests().await;

    let client = Client::new();

    // Requests to different domains
    client
        .get(format!("{}/data", MOCK_SERVER_URL))
        .header("Host", "service-a.example.com")
        .send()
        .await
        .unwrap();

    client
        .get(format!("{}/data", MOCK_SERVER_URL))
        .header("Host", "service-b.example.com")
        .send()
        .await
        .unwrap();

    // Query only service-a
    let requests = server
        .get_requests(Some("service-a.example.com"), None, None)
        .await;

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].domain, "service-a.example.com");
}
```

**Cargo.toml for integration tests:**

```toml
[dev-dependencies]
tokio = { version = "1", features = ["full", "test-util"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

## Related Documentation

- **[CLI](./CLI.md)** - Full command-line reference
- **[Admin API](./API.md)** - Complete API documentation
- **[Deployment](./DEPLOYMENT.md)** - Production configuration
- **[Lua Scripting](./LUA_SCRIPTING.md)** - Writing mock handlers
