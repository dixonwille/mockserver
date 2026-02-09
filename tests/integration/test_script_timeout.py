"""Tests for --script-timeout enforcement."""

import pytest


def _write_domain(server, domain_name, lua_code):
    """Write a custom init.lua into a domain folder."""
    domain_dir = server.mocks_dir / domain_name
    domain_dir.mkdir(exist_ok=True)
    (domain_dir / "init.lua").write_text(lua_code)


@pytest.mark.slow
def test_cpu_loop_returns_504(start_server):
    """A CPU-bound infinite loop should be killed by the instruction hook."""
    server = start_server([], extra_args=["--script-timeout", "2"])
    _write_domain(
        server,
        "cpuloop.test",
        "function handle(request)\n  while true do local x = 1 end\nend\n",
    )
    r = server.request("GET", "/", domain="cpuloop.test")
    assert r.status_code == 504
    body = r.json()
    assert "timeout" in body["message"].lower()


@pytest.mark.slow
def test_long_sleep_returns_504(start_server):
    """A long async delay.sleep should be killed by the tokio timeout."""
    server = start_server([], extra_args=["--script-timeout", "2"])
    _write_domain(
        server,
        "longsleep.test",
        (
            'local delay = require("delay")\n'
            "function handle(request)\n"
            "  delay.sleep(60000)\n"
            '  return { status = 200, body = "done" }\n'
            "end\n"
        ),
    )
    r = server.request("GET", "/", domain="longsleep.test", timeout=10)
    assert r.status_code == 504
    body = r.json()
    assert "timeout" in body["message"].lower()


def test_normal_script_within_timeout(start_server):
    """A fast script completes well within the timeout."""
    server = start_server([], extra_args=["--script-timeout", "5"])
    _write_domain(
        server,
        "fast.test",
        'function handle(request)\n  return { status = 200, body = "ok" }\nend\n',
    )
    r = server.request("GET", "/", domain="fast.test")
    assert r.status_code == 200
    assert r.text == "ok"


@pytest.mark.slow
def test_short_delay_within_timeout(start_server):
    """The mock-slow-upstream example (2s delay) succeeds with a 10s timeout."""
    server = start_server(["mock-slow-upstream"], extra_args=["--script-timeout", "10"])
    r = server.request("POST", "/v1/process", domain="mock-slow-upstream")
    assert r.status_code == 200
    body = r.json()
    assert body["status"] == "completed"
