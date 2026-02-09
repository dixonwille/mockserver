"""Tests for the mock-user-directory example."""


def test_list_seeded_users(start_server):
    server = start_server(["mock-user-directory"])
    r = server.request("GET", "/v1/users", domain="mock-user-directory")
    assert r.status_code == 200
    body = r.json()
    assert body["total"] == 2


def test_get_user_by_id(start_server):
    server = start_server(["mock-user-directory"])
    r = server.request("GET", "/v1/users/seed-1", domain="mock-user-directory")
    assert r.status_code == 200
    body = r.json()
    assert body["id"] == "seed-1"
    assert body["name"] == "Admin User"
    assert body["role"] == "admin"


def test_get_missing_user_returns_404(start_server):
    server = start_server(["mock-user-directory"])
    r = server.request("GET", "/v1/users/nonexistent", domain="mock-user-directory")
    assert r.status_code == 404


def test_create_user(start_server):
    server = start_server(["mock-user-directory"])
    r = server.request(
        "POST",
        "/v1/users",
        domain="mock-user-directory",
        content='{"name":"Alice","email":"alice@example.com"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 201
    body = r.json()
    assert body["name"] == "Alice"
    assert body["email"] == "alice@example.com"
    assert body["role"] == "viewer"  # default role
    assert len(body["id"]) > 0


def test_delete_user(start_server):
    server = start_server(["mock-user-directory"])
    r = server.request("DELETE", "/v1/users/seed-2", domain="mock-user-directory")
    assert r.status_code == 204

    # Verify deleted
    r2 = server.request("GET", "/v1/users/seed-2", domain="mock-user-directory")
    assert r2.status_code == 404


def test_delete_missing_user_returns_404(start_server):
    server = start_server(["mock-user-directory"])
    r = server.request(
        "DELETE", "/v1/users/nonexistent", domain="mock-user-directory"
    )
    assert r.status_code == 404


def test_validate_valid_token(start_server):
    server = start_server(["mock-user-directory"])
    r = server.request(
        "POST",
        "/v1/auth/validate",
        domain="mock-user-directory",
        content='{"token":"tok_admin"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 200
    body = r.json()
    assert body["valid"] is True
    assert body["role"] == "admin"


def test_validate_invalid_token(start_server):
    server = start_server(["mock-user-directory"])
    r = server.request(
        "POST",
        "/v1/auth/validate",
        domain="mock-user-directory",
        content='{"token":"invalid"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 401
    assert r.json()["error"] == "Invalid token"
