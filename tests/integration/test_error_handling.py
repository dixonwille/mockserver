"""Tests for error handling with custom Lua scripts."""


def _write_domain(server, domain_name, lua_code):
    """Write a custom init.lua into a domain folder."""
    domain_dir = server.mocks_dir / domain_name
    domain_dir.mkdir(exist_ok=True)
    (domain_dir / "init.lua").write_text(lua_code)


def test_missing_handle_falls_back_to_default(start_server):
    """Domain without handle() can't load; falls back to _default (404)."""
    server = start_server([])
    _write_domain(server, "nohandle.test", "-- no handle function\nlocal x = 1\n")
    r = server.request("GET", "/", domain="nohandle.test")
    assert r.status_code == 404
    body = r.json()
    assert "nohandle.test" in body["message"]


def test_lua_syntax_error_falls_back_to_default(start_server):
    """Domain with syntax error can't load; falls back to _default (404)."""
    server = start_server([])
    _write_domain(server, "syntax.test", "function handle(request\nend\n")
    r = server.request("GET", "/", domain="syntax.test")
    assert r.status_code == 404
    body = r.json()
    assert "syntax.test" in body["message"]


def test_runtime_error_returns_500(start_server):
    """Domain loads but handle() throws at runtime → 500."""
    server = start_server([])
    _write_domain(
        server,
        "runtime.test",
        'function handle(request)\n  error("intentional error")\nend\n',
    )
    r = server.request("GET", "/", domain="runtime.test")
    assert r.status_code == 500
    body = r.json()
    assert body["error"] == "Handler Error"
    assert "intentional error" in body["message"]


def test_unknown_domain_falls_back_to_default(start_server):
    """Domain with no folder at all falls back to _default (404 JSON)."""
    server = start_server([])
    r = server.request("GET", "/", domain="nonexistent.test")
    assert r.status_code == 404
    body = r.json()
    assert body["error"] == "Not Found"
    assert "nonexistent.test" in body["message"]


def test_handle_returning_nil_produces_200(start_server):
    server = start_server([])
    _write_domain(
        server,
        "nilreturn.test",
        "function handle(request)\n  return nil\nend\n",
    )
    r = server.request("GET", "/", domain="nilreturn.test")
    assert r.status_code == 200
    assert len(r.content) == 0


