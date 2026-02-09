"""Tests for --max-body enforcement."""


def test_oversized_body_returns_400(start_server):
    """A request body exceeding --max-body should be rejected with 400."""
    server = start_server(["debug-echo"], extra_args=["--max-body", "100"])
    big_body = "x" * 200
    r = server.request("POST", "/", domain="debug-echo", content=big_body)
    assert r.status_code == 400
    body = r.json()
    assert body["error"] == "Bad Request"


def test_body_within_limit_succeeds(start_server):
    """A request body under --max-body should be accepted normally."""
    server = start_server(["debug-echo"], extra_args=["--max-body", "100"])
    small_body = "x" * 50
    r = server.request("POST", "/", domain="debug-echo", content=small_body)
    assert r.status_code == 200
    body = r.json()
    assert body["body"] == small_body


def test_exact_limit_succeeds(start_server):
    """A request body exactly at --max-body should be accepted."""
    server = start_server(["debug-echo"], extra_args=["--max-body", "100"])
    exact_body = "x" * 100
    r = server.request("POST", "/", domain="debug-echo", content=exact_body)
    assert r.status_code == 200
    body = r.json()
    assert body["body"] == exact_body


def test_one_over_limit_rejected(start_server):
    """A request body one byte over --max-body should be rejected."""
    server = start_server(["debug-echo"], extra_args=["--max-body", "100"])
    over_body = "x" * 101
    r = server.request("POST", "/", domain="debug-echo", content=over_body)
    assert r.status_code == 400
    body = r.json()
    assert body["error"] == "Bad Request"


def test_get_without_body_succeeds(start_server):
    """A GET request with no body should succeed regardless of --max-body."""
    server = start_server(["debug-echo"], extra_args=["--max-body", "100"])
    r = server.request("GET", "/", domain="debug-echo")
    assert r.status_code == 200


def test_rejection_does_not_store_request(start_server):
    """A rejected oversized body should not appear in /api/requests."""
    server = start_server(["debug-echo"], extra_args=["--max-body", "100"])
    # Clean any prior records
    server.cleanup()

    big_body = "x" * 200
    r = server.request("POST", "/", domain="debug-echo", content=big_body)
    assert r.status_code == 400

    records = server.get_requests(domain="debug-echo")
    assert records.status_code == 200
    data = records.json()
    assert data["total"] == 0
    assert len(data["requests"]) == 0
