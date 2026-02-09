"""Tests for the debug-echo example."""


def test_get_root_returns_echo(start_server):
    server = start_server(["debug-echo"])
    r = server.request("GET", "/", domain="debug-echo")
    assert r.status_code == 200
    body = r.json()
    assert body["method"] == "GET"
    assert body["path"] == "/"
    assert body["domain"] == "debug-echo"


def test_post_echoes_body(start_server):
    server = start_server(["debug-echo"])
    r = server.request(
        "POST", "/anything", domain="debug-echo", content="hello world"
    )
    assert r.status_code == 200
    body = r.json()
    assert body["method"] == "POST"
    assert body["body"] == "hello world"


def test_query_params_included(start_server):
    server = start_server(["debug-echo"])
    r = server.request("GET", "/search?foo=bar&baz=qux", domain="debug-echo")
    assert r.status_code == 200
    body = r.json()
    assert body["query"]["foo"] == "bar"
    assert body["query"]["baz"] == "qux"


def test_content_type_is_json(start_server):
    server = start_server(["debug-echo"])
    r = server.request("GET", "/", domain="debug-echo")
    assert "application/json" in r.headers["content-type"]


def test_domain_matches_host_header(start_server):
    server = start_server(["debug-echo"])
    r = server.request("GET", "/", domain="debug-echo")
    body = r.json()
    assert body["domain"] == "debug-echo"


def test_received_at_is_present(start_server):
    server = start_server(["debug-echo"])
    r = server.request("GET", "/", domain="debug-echo")
    body = r.json()
    assert "received_at" in body
    assert len(body["received_at"]) > 0
