"""Tests for the mock-weather-service example."""


def test_current_weather_known_city(start_server):
    server = start_server(["mock-weather-service"])
    r = server.request(
        "GET", "/v1/current?city=london", domain="mock-weather-service"
    )
    assert r.status_code == 200
    body = r.json()
    assert body["city"] == "london"
    assert "temp_c" in body
    assert "condition" in body


def test_current_weather_unknown_city(start_server):
    server = start_server(["mock-weather-service"])
    r = server.request(
        "GET", "/v1/current?city=atlantis", domain="mock-weather-service"
    )
    assert r.status_code == 404
    assert "City not found" in r.json()["error"]


def test_current_weather_missing_city_param(start_server):
    server = start_server(["mock-weather-service"])
    r = server.request("GET", "/v1/current", domain="mock-weather-service")
    assert r.status_code == 400


def test_forecast_returns_correct_days(start_server):
    server = start_server(["mock-weather-service"])
    r = server.request(
        "GET", "/v1/forecast?city=tokyo&days=3", domain="mock-weather-service"
    )
    assert r.status_code == 200
    body = r.json()
    assert body["city"] == "tokyo"
    assert body["days"] == 3
    assert len(body["forecast"]) == 3


def test_forecast_caps_at_7_days(start_server):
    server = start_server(["mock-weather-service"])
    r = server.request(
        "GET", "/v1/forecast?city=london&days=30", domain="mock-weather-service"
    )
    assert r.status_code == 200
    body = r.json()
    assert body["days"] == 7
    assert len(body["forecast"]) == 7


def test_forecast_unknown_city(start_server):
    server = start_server(["mock-weather-service"])
    r = server.request(
        "GET", "/v1/forecast?city=mars", domain="mock-weather-service"
    )
    assert r.status_code == 404
