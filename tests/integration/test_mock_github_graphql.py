"""Tests for the mock-github-graphql example."""


def test_viewer_query(start_server):
    server = start_server(["mock-github-graphql"])
    r = server.request(
        "POST",
        "/graphql",
        domain="mock-github-graphql",
        content='{"query":"{ viewer { login name } }"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 200
    body = r.json()
    assert body["data"]["viewer"]["login"] == "octocat"


def test_pull_requests_query(start_server):
    server = start_server(["mock-github-graphql"])
    r = server.request(
        "POST",
        "/graphql",
        domain="mock-github-graphql",
        content='{"query":"{ repository { pullRequests { nodes { number title } } } }"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 200
    body = r.json()
    prs = body["data"]["repository"]["pullRequests"]["nodes"]
    assert len(prs) == 2
    assert prs[0]["number"] == 42


def test_issues_query(start_server):
    server = start_server(["mock-github-graphql"])
    r = server.request(
        "POST",
        "/graphql",
        domain="mock-github-graphql",
        content='{"query":"{ repository { issues { nodes { number title } } } }"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 200
    body = r.json()
    issues = body["data"]["repository"]["issues"]["nodes"]
    assert len(issues) == 2


def test_create_issue_mutation(start_server):
    server = start_server(["mock-github-graphql"])
    r = server.request(
        "POST",
        "/graphql",
        domain="mock-github-graphql",
        content='{"query":"mutation { createIssue(input: {}) { issue { number } } }","variables":{"title":"Test issue"}}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 200
    body = r.json()
    assert body["data"]["createIssue"]["issue"]["number"] == 99


def test_unknown_query_returns_error(start_server):
    server = start_server(["mock-github-graphql"])
    r = server.request(
        "POST",
        "/graphql",
        domain="mock-github-graphql",
        content='{"query":"{ unknown { field } }"}',
        headers={"Content-Type": "application/json"},
    )
    assert r.status_code == 200
    body = r.json()
    assert "errors" in body
    assert body["errors"][0]["message"] == "Unknown query"


def test_non_post_returns_405(start_server):
    server = start_server(["mock-github-graphql"])
    r = server.request("GET", "/graphql", domain="mock-github-graphql")
    assert r.status_code == 405
