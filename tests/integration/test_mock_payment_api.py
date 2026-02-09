"""Tests for the mock-payment-api example."""


def test_create_charge(start_server):
    server = start_server(["mock-payment-api"])
    r = server.request(
        "POST",
        "/v1/charges",
        domain="mock-payment-api",
        content='{"amount":2000,"currency":"usd"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 201
    body = r.json()
    assert body["amount"] == 2000
    assert body["currency"] == "usd"
    assert body["status"] == "succeeded"
    assert body["id"].startswith("ch_")
    assert "created_at" in body


def test_get_charge_by_id(start_server):
    server = start_server(["mock-payment-api"])
    # Create a charge first
    create_r = server.request(
        "POST",
        "/v1/charges",
        domain="mock-payment-api",
        content='{"amount":1000,"currency":"eur"}',
        headers={"Content-Type": "application/json"},
    )
    charge_id = create_r.json()["id"]

    # Retrieve it
    r = server.request("GET", f"/v1/charges/{charge_id}", domain="mock-payment-api")
    assert r.status_code == 200
    assert r.json()["id"] == charge_id
    assert r.json()["amount"] == 1000


def test_get_missing_charge_returns_404(start_server):
    server = start_server(["mock-payment-api"])
    r = server.request("GET", "/v1/charges/ch_nonexistent", domain="mock-payment-api")
    assert r.status_code == 404


def test_list_charges(start_server):
    server = start_server(["mock-payment-api"])
    # Create two charges
    for amount in [500, 1500]:
        server.request(
            "POST",
            "/v1/charges",
            domain="mock-payment-api",
            content=f'{{"amount":{amount}}}',
            headers={"Content-Type": "application/json"},
        )

    r = server.request("GET", "/v1/charges", domain="mock-payment-api")
    assert r.status_code == 200
    body = r.json()
    assert body["total"] == 2
    assert len(body["charges"]) == 2


def test_refund_charge(start_server):
    server = start_server(["mock-payment-api"])
    # Create a charge
    create_r = server.request(
        "POST",
        "/v1/charges",
        domain="mock-payment-api",
        content='{"amount":3000}',
        headers={"Content-Type": "application/json"},
    )
    charge_id = create_r.json()["id"]

    # Refund it
    r = server.request(
        "POST",
        "/v1/refunds",
        domain="mock-payment-api",
        content=f'{{"charge":"{charge_id}"}}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 200
    assert r.json()["charge"] == charge_id
    assert r.json()["id"].startswith("re_")

    # Verify charge status updated
    r2 = server.request("GET", f"/v1/charges/{charge_id}", domain="mock-payment-api")
    assert r2.json()["status"] == "refunded"


def test_refund_nonexistent_charge_returns_404(start_server):
    server = start_server(["mock-payment-api"])
    r = server.request(
        "POST",
        "/v1/refunds",
        domain="mock-payment-api",
        content='{"charge":"ch_fake"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 404
