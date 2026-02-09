"""Tests for the Admin API endpoints."""

import uuid


def test_healthz(start_server):
    server = start_server(["debug-echo"])
    r = server.healthz()
    assert r.status_code == 200
    body = r.json()
    assert body["status"] == "ok"
    assert "version" in body


def test_about(start_server):
    server = start_server(["debug-echo"])
    r = server.api_get("/api/about")
    assert r.status_code == 200
    body = r.json()
    assert body["name"] == "mockserver"
    assert body["license"] == "AGPL-3.0-only"
    assert "source" in body


def test_requests_initially_empty(start_server):
    server = start_server(["debug-echo"])
    r = server.get_requests()
    assert r.status_code == 200
    body = r.json()
    assert body["total"] == 0
    assert body["requests"] == []


def test_requests_recorded_after_mock_call(start_server):
    server = start_server(["debug-echo"])
    server.request("GET", "/hello", domain="debug-echo")

    r = server.get_requests()
    assert r.status_code == 200
    body = r.json()
    assert body["total"] >= 1


def test_filter_by_domain(start_server):
    server = start_server(["debug-echo"])
    server.request("GET", "/a", domain="debug-echo")

    r = server.get_requests(domain="debug-echo")
    body = r.json()
    assert body["total"] >= 1
    for req in body["requests"]:
        assert req["domain"] == "debug-echo"


def test_filter_by_method(start_server):
    server = start_server(["debug-echo"])
    server.request("POST", "/data", domain="debug-echo", content="x")
    server.request("GET", "/data", domain="debug-echo")

    r = server.get_requests(method="POST")
    body = r.json()
    assert body["total"] >= 1
    for req in body["requests"]:
        assert req["method"] == "POST"


def test_filter_by_path(start_server):
    server = start_server(["debug-echo"])
    server.request("GET", "/findme", domain="debug-echo")
    server.request("GET", "/other", domain="debug-echo")

    r = server.get_requests(path="findme")
    body = r.json()
    assert body["total"] >= 1
    for req in body["requests"]:
        assert "findme" in req["path"]


def test_pagination_limit_offset(start_server):
    server = start_server(["debug-echo"])
    for i in range(5):
        server.request("GET", f"/p{i}", domain="debug-echo")

    r = server.get_requests(limit=2, offset=0)
    body = r.json()
    assert len(body["requests"]) == 2
    assert body["limit"] == 2
    assert body["offset"] == 0


def test_get_request_by_id(start_server):
    server = start_server(["debug-echo"])
    server.request("GET", "/trackme", domain="debug-echo")

    listing = server.get_requests(path="trackme").json()
    assert listing["total"] >= 1
    req_id = listing["requests"][0]["id"]

    r = server.get_request(req_id)
    assert r.status_code == 200
    body = r.json()
    assert body["id"] == req_id
    assert body["domain"] == "debug-echo"
    assert body["method"] == "GET"


def test_get_request_invalid_uuid_returns_400(start_server):
    server = start_server(["debug-echo"])
    r = server.get_request("not-a-uuid")
    assert r.status_code == 400


def test_get_request_missing_uuid_returns_404(start_server):
    server = start_server(["debug-echo"])
    r = server.get_request(str(uuid.uuid4()))
    assert r.status_code == 404


def test_get_response_by_request_id(start_server):
    server = start_server(["debug-echo"])
    server.request("GET", "/resptest", domain="debug-echo")

    listing = server.get_requests(path="resptest").json()
    req_id = listing["requests"][0]["id"]

    r = server.get_response(req_id)
    assert r.status_code == 200
    body = r.json()
    assert body["status_code"] == 200
    assert "duration_ms" in body


def test_full_round_trip(start_server):
    server = start_server(["debug-echo"])
    # Send a request with known body
    mock_r = server.request(
        "POST",
        "/roundtrip",
        domain="debug-echo",
        content="test-body",
        headers={"X-Test": "value"},
    )
    assert mock_r.status_code == 200

    # Find it via API
    listing = server.get_requests(path="roundtrip").json()
    assert listing["total"] >= 1
    req_id = listing["requests"][0]["id"]

    # Inspect the request record
    req = server.get_request(req_id).json()
    assert req["method"] == "POST"
    assert req["path"] == "/roundtrip"

    # Inspect the response record
    resp = server.get_response(req_id).json()
    assert resp["status_code"] == 200


def test_delete_requests(start_server):
    server = start_server(["debug-echo"])
    server.request("GET", "/del1", domain="debug-echo")
    server.request("GET", "/del2", domain="debug-echo")

    r = server.clear_requests()
    assert r.status_code == 200
    assert r.json()["deleted"] >= 2

    # Verify empty
    r2 = server.get_requests()
    assert r2.json()["total"] == 0


def test_reload_config(start_server):
    server = start_server(["debug-echo"])
    r = server.reload()
    assert r.status_code == 200
    body = r.json()
    assert "reloaded" in body
    assert isinstance(body["reloaded"], list)


def test_cleanup(start_server):
    server = start_server(["debug-echo"])
    r = server.cleanup()
    assert r.status_code == 200
    assert "deleted" in r.json()


def test_api_prefix_mode(start_server):
    """Admin API served under a path prefix on the mock port."""
    import httpx

    server = start_server(
        ["debug-echo"],
        extra_args=["--api-prefix", "/_admin"],
        separate_api=False,
        healthz_path="/_admin/api/healthz",
    )
    prefix_url = f"http://127.0.0.1:{server.mock_port}"

    r = httpx.get(f"{prefix_url}/_admin/api/healthz", timeout=5)
    assert r.status_code == 200
    assert r.json()["status"] == "ok"

    r2 = httpx.get(f"{prefix_url}/_admin/api/about", timeout=5)
    assert r2.status_code == 200
    assert r2.json()["name"] == "mockserver"
