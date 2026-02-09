"""Tests for hot reload functionality."""

import time


def test_manual_reload_changes_behavior(start_server):
    """POST /api/config/reload triggers reload without file watcher."""
    server = start_server(["debug-echo"])

    # Confirm debug-echo works
    r = server.request("GET", "/", domain="debug-echo")
    assert r.status_code == 200
    assert r.json()["method"] == "GET"

    # Overwrite init.lua with different behavior
    init_lua = server.mocks_dir / "debug-echo" / "init.lua"
    init_lua.write_text(
        'local json = require("json")\n'
        "function handle(request)\n"
        "  return {\n"
        "    status = 200,\n"
        '    headers = { ["Content-Type"] = "application/json" },\n'
        '    body = json.encode({ reloaded = true }),\n'
        "  }\n"
        "end\n"
    )

    # Trigger reload
    server.reload()

    # Verify new behavior
    r = server.request("GET", "/", domain="debug-echo")
    assert r.status_code == 200
    assert r.json()["reloaded"] is True


def test_file_watcher_detects_changes(start_server):
    """With watch enabled, file changes are picked up automatically."""
    server = start_server(["debug-echo"], watch=True)

    # Confirm debug-echo works
    r = server.request("GET", "/", domain="debug-echo")
    assert r.status_code == 200
    assert "method" in r.json()

    # Overwrite init.lua with different behavior
    init_lua = server.mocks_dir / "debug-echo" / "init.lua"
    init_lua.write_text(
        'local json = require("json")\n'
        "function handle(request)\n"
        "  return {\n"
        "    status = 200,\n"
        '    headers = { ["Content-Type"] = "application/json" },\n'
        '    body = json.encode({ watched = true }),\n'
        "  }\n"
        "end\n"
    )

    # Poll until response changes (5s timeout)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        r = server.request("GET", "/", domain="debug-echo")
        if r.status_code == 200:
            body = r.json()
            if body.get("watched") is True:
                return  # Success
        time.sleep(0.2)

    # If we get here, the watcher didn't pick up the change
    assert False, "File watcher did not pick up init.lua change within 5s"


def test_new_domain_picked_up(start_server):
    """New domain folder picked up via lazy loading (get_or_create_pool)."""
    server = start_server(["debug-echo"])

    # Create new domain
    new_domain = server.mocks_dir / "new-domain.test"
    new_domain.mkdir()
    (new_domain / "init.lua").write_text(
        'local json = require("json")\n'
        "function handle(request)\n"
        "  return {\n"
        "    status = 200,\n"
        '    headers = { ["Content-Type"] = "application/json" },\n'
        '    body = json.encode({ new_domain = true }),\n'
        "  }\n"
        "end\n"
    )

    # The domain should be lazily loaded on first request
    r = server.request("GET", "/", domain="new-domain.test")
    assert r.status_code == 200
    assert r.json()["new_domain"] is True
