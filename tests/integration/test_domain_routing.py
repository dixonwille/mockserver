"""Tests for domain routing resolution."""


def test_host_header_routes_to_correct_domain(start_server):
    server = start_server(["debug-echo", "mock-payment-api"])

    # debug-echo returns echo
    r1 = server.request("GET", "/", domain="debug-echo")
    assert r1.status_code == 200
    assert r1.json()["domain"] == "debug-echo"

    # mock-payment-api returns 404 for unknown paths
    r2 = server.request("GET", "/v1/charges", domain="mock-payment-api")
    assert r2.status_code == 200


def test_unknown_domain_falls_back_to_default(start_server):
    server = start_server(["debug-echo"])
    r = server.request("GET", "/", domain="unknown.test")
    # _default handler returns 404 with a JSON error message
    assert r.status_code == 404
    body = r.json()
    assert "unknown.test" in body["message"]


def test_x_forwarded_host_overrides_host(start_server):
    server = start_server(["debug-echo", "mock-payment-api"])
    r = server.request(
        "GET",
        "/",
        domain="mock-payment-api",
        headers={"X-Forwarded-Host": "debug-echo"},
    )
    assert r.status_code == 200
    body = r.json()
    # Should route to debug-echo because X-Forwarded-Host takes precedence
    assert body["domain"] == "debug-echo"


def test_x_original_host_highest_priority(start_server):
    server = start_server(["debug-echo", "mock-payment-api"])
    r = server.request(
        "GET",
        "/",
        domain="mock-payment-api",
        headers={
            "X-Forwarded-Host": "mock-payment-api",
            "X-Original-Host": "debug-echo",
        },
    )
    assert r.status_code == 200
    body = r.json()
    assert body["domain"] == "debug-echo"


def test_port_stripped_from_domain(start_server):
    server = start_server(["debug-echo"])
    r = server.request("GET", "/", domain="debug-echo:8080")
    assert r.status_code == 200
    body = r.json()
    assert body["domain"] == "debug-echo"


def test_case_insensitive_matching(start_server):
    server = start_server(["debug-echo"])
    r = server.request("GET", "/", domain="Debug-Echo")
    assert r.status_code == 200
    body = r.json()
    assert body["domain"] == "debug-echo"
