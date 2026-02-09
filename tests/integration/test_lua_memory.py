"""Tests for --lua-memory enforcement."""

import pytest


def _write_domain(server, domain_name, lua_code):
    """Write a custom init.lua into a domain folder."""
    domain_dir = server.mocks_dir / domain_name
    domain_dir.mkdir(exist_ok=True)
    (domain_dir / "init.lua").write_text(lua_code)


@pytest.mark.slow
def test_memory_exceeded_returns_500(start_server):
    """A script that allocates excessive memory should return 500."""
    server = start_server([], extra_args=["--lua-memory", "1"])
    _write_domain(
        server,
        "oom.test",
        (
            "function handle(request)\n"
            "  local t = {}\n"
            "  for i = 1, 10000000 do\n"
            '    t[i] = string.rep("x", 1024)\n'
            "  end\n"
            '  return { status = 200, body = "done" }\n'
            "end\n"
        ),
    )
    r = server.request("GET", "/", domain="oom.test")
    assert r.status_code == 500
    body = r.json()
    assert body["error"] == "Handler Error"
    assert "memory" in body["message"].lower()


def test_normal_script_within_memory_limit(start_server):
    """A lightweight script completes within the memory limit."""
    server = start_server([], extra_args=["--lua-memory", "1"])
    _write_domain(
        server,
        "small.test",
        'function handle(request)\n  return { status = 200, body = "ok" }\nend\n',
    )
    r = server.request("GET", "/", domain="small.test")
    assert r.status_code == 200
    assert r.text == "ok"


@pytest.mark.slow
def test_memory_exceeded_does_not_crash_server(start_server):
    """After a memory error the server remains healthy for other domains."""
    server = start_server([], extra_args=["--lua-memory", "1"])
    _write_domain(
        server,
        "oom2.test",
        (
            "function handle(request)\n"
            "  local t = {}\n"
            "  for i = 1, 10000000 do\n"
            '    t[i] = string.rep("x", 1024)\n'
            "  end\n"
            '  return { status = 200, body = "done" }\n'
            "end\n"
        ),
    )
    _write_domain(
        server,
        "lite.test",
        'function handle(request)\n  return { status = 200, body = "hi" }\nend\n',
    )

    # Hit the memory-exceeding domain
    r1 = server.request("GET", "/", domain="oom2.test")
    assert r1.status_code == 500

    # Hit the lightweight domain — should still work
    r2 = server.request("GET", "/", domain="lite.test")
    assert r2.status_code == 200
    assert r2.text == "hi"

    # Healthz should confirm the server is alive
    r3 = server.healthz()
    assert r3.status_code == 200
