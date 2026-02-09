"""Tests for the mock-feature-flags example."""


def test_list_flags(start_server):
    server = start_server(["mock-feature-flags"])
    r = server.request("GET", "/v1/flags", domain="mock-feature-flags")
    assert r.status_code == 200
    flags = r.json()["flags"]
    assert flags["dark-mode"] is False
    assert flags["new-checkout"] is True


def test_get_single_flag(start_server):
    server = start_server(["mock-feature-flags"])
    r = server.request("GET", "/v1/flags/dark-mode", domain="mock-feature-flags")
    assert r.status_code == 200
    body = r.json()
    assert body["key"] == "dark-mode"
    assert body["value"] is False


def test_get_missing_flag_returns_404(start_server):
    server = start_server(["mock-feature-flags"])
    r = server.request("GET", "/v1/flags/nonexistent", domain="mock-feature-flags")
    assert r.status_code == 404


def test_override_flag(start_server):
    server = start_server(["mock-feature-flags"])
    # Override dark-mode to true
    r = server.request(
        "PUT",
        "/v1/flags/dark-mode",
        domain="mock-feature-flags",
        content='{"enabled":true}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 200
    assert r.json()["value"] is True

    # Verify it persisted
    r2 = server.request("GET", "/v1/flags/dark-mode", domain="mock-feature-flags")
    assert r2.json()["value"] is True


def test_reset_flags(start_server):
    server = start_server(["mock-feature-flags"])
    # Override a flag
    server.request(
        "PUT",
        "/v1/flags/dark-mode",
        domain="mock-feature-flags",
        content='{"enabled":true}',
        headers={"Content-Type": "application/json"},
    )

    # Reset to defaults
    r = server.request("POST", "/v1/flags/reset", domain="mock-feature-flags")
    assert r.status_code == 200
    assert r.json()["reset"] is True

    # Verify defaults restored
    r2 = server.request("GET", "/v1/flags/dark-mode", domain="mock-feature-flags")
    assert r2.json()["value"] is False
