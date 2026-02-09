"""Tests for the mock-notification-service example."""


def test_send_message(start_server):
    server = start_server(["mock-notification-service"])
    r = server.request(
        "POST",
        "/v1/messages",
        domain="mock-notification-service",
        content='{"to":"user@example.com","subject":"Hello","body":"Hi there","channel":"email"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 201
    body = r.json()
    assert body["to"] == "user@example.com"
    assert body["subject"] == "Hello"
    assert body["channel"] == "email"
    assert body["id"].startswith("msg_")
    assert "sent_at" in body


def test_list_messages(start_server):
    server = start_server(["mock-notification-service"])
    # Send two messages
    for to in ["a@test.com", "b@test.com"]:
        server.request(
            "POST",
            "/v1/messages",
            domain="mock-notification-service",
            content=f'{{"to":"{to}","subject":"Test","body":"Hi"}}',
            headers={"Content-Type": "application/json"},
        )

    r = server.request("GET", "/v1/messages", domain="mock-notification-service")
    assert r.status_code == 200
    body = r.json()
    assert body["total"] == 2


def test_get_message_by_id(start_server):
    server = start_server(["mock-notification-service"])
    create_r = server.request(
        "POST",
        "/v1/messages",
        domain="mock-notification-service",
        content='{"to":"test@test.com","subject":"Sub","body":"Body"}',
        headers={"Content-Type": "application/json"},
    )
    msg_id = create_r.json()["id"]

    r = server.request(
        "GET", f"/v1/messages/{msg_id}", domain="mock-notification-service"
    )
    assert r.status_code == 200
    assert r.json()["id"] == msg_id


def test_get_missing_message_returns_404(start_server):
    server = start_server(["mock-notification-service"])
    r = server.request(
        "GET", "/v1/messages/msg_nonexistent", domain="mock-notification-service"
    )
    assert r.status_code == 404


def test_clear_messages(start_server):
    server = start_server(["mock-notification-service"])
    # Send a message
    server.request(
        "POST",
        "/v1/messages",
        domain="mock-notification-service",
        content='{"to":"x@x.com","subject":"X","body":"X"}',
        headers={"Content-Type": "application/json"},
    )

    # Clear
    r = server.request("DELETE", "/v1/messages", domain="mock-notification-service")
    assert r.status_code == 200
    assert r.json()["cleared"] is True

    # Verify empty
    r2 = server.request("GET", "/v1/messages", domain="mock-notification-service")
    assert r2.json()["total"] == 0
