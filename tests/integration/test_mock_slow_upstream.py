"""Tests for the mock-slow-upstream example."""

import time

import pytest


@pytest.mark.slow
def test_process_endpoint_has_delay(start_server):
    server = start_server(["mock-slow-upstream"])
    t0 = time.monotonic()
    r = server.request("POST", "/v1/process", domain="mock-slow-upstream")
    elapsed = time.monotonic() - t0
    assert r.status_code == 200
    body = r.json()
    assert body["status"] == "completed"
    assert "processed_at" in body
    assert elapsed >= 1.5, f"Expected >=1.5s delay, got {elapsed:.2f}s"


def test_status_endpoint_returns_immediately(start_server):
    server = start_server(["mock-slow-upstream"])
    r = server.request("GET", "/v1/status/job-123", domain="mock-slow-upstream")
    assert r.status_code == 200
    body = r.json()
    assert body["job_id"] == "job-123"
    assert body["status"] == "processing"


def test_health_endpoint(start_server):
    server = start_server(["mock-slow-upstream"])
    r = server.request("GET", "/v1/health", domain="mock-slow-upstream")
    assert r.status_code == 200
    assert r.json()["status"] == "healthy"


def test_unknown_path_returns_404(start_server):
    server = start_server(["mock-slow-upstream"])
    r = server.request("GET", "/unknown", domain="mock-slow-upstream")
    assert r.status_code == 404
