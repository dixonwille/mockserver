"""Core fixtures for mockserver integration tests."""

from __future__ import annotations

import os
import shutil
import socket
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import httpx
import pytest


# ---------------------------------------------------------------------------
# Binary fixture
# ---------------------------------------------------------------------------

@pytest.fixture(scope="session")
def mockserver_bin() -> Path:
    """Return the path to the mockserver binary.

    If MOCKSERVER_BIN is set, use that.  Otherwise, build via cargo.
    """
    env_bin = os.environ.get("MOCKSERVER_BIN")
    if env_bin:
        p = Path(env_bin).resolve()
        if not p.exists():
            pytest.fail(f"MOCKSERVER_BIN={env_bin!r} does not exist")
        return p

    # Build release binary
    result = subprocess.run(
        ["cargo", "build", "--release"],
        capture_output=True,
        text=True,
        timeout=600,
    )
    if result.returncode != 0:
        pytest.fail(f"cargo build failed:\n{result.stderr}")

    # Find it relative to the workspace root
    workspace = Path(__file__).resolve().parents[2]
    binary = workspace / "target" / "release" / "mockserver"
    if not binary.exists():
        pytest.fail(f"Binary not found at {binary}")
    return binary


# ---------------------------------------------------------------------------
# Free-port helper
# ---------------------------------------------------------------------------

def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


# ---------------------------------------------------------------------------
# MockServer helper class
# ---------------------------------------------------------------------------

@dataclass
class MockServer:
    """Convenience wrapper around a running mockserver process."""

    mock_port: int
    api_port: int
    mocks_dir: Path
    data_dir: Path
    process: subprocess.Popen
    stdout_log: Path
    stderr_log: Path
    cmd: list[str] = field(default_factory=list)
    examples_loaded: list[str] = field(default_factory=list)

    @property
    def mock_url(self) -> str:
        return f"http://127.0.0.1:{self.mock_port}"

    @property
    def api_url(self) -> str:
        return f"http://127.0.0.1:{self.api_port}"

    # -- Mock-port helpers --------------------------------------------------

    def request(
        self,
        method: str,
        path: str,
        *,
        domain: str | None = None,
        **kwargs: Any,
    ) -> httpx.Response:
        headers = dict(kwargs.pop("headers", {}))
        if domain:
            headers["Host"] = domain
        return httpx.request(
            method,
            f"{self.mock_url}{path}",
            headers=headers,
            **kwargs,
        )

    # -- Admin API helpers --------------------------------------------------

    def api_get(self, path: str, **kwargs: Any) -> httpx.Response:
        return httpx.get(f"{self.api_url}{path}", **kwargs)

    def api_post(self, path: str, **kwargs: Any) -> httpx.Response:
        return httpx.post(f"{self.api_url}{path}", **kwargs)

    def api_delete(self, path: str, **kwargs: Any) -> httpx.Response:
        return httpx.delete(f"{self.api_url}{path}", **kwargs)

    def get_requests(self, **filters: Any) -> httpx.Response:
        return self.api_get("/api/requests", params=filters)

    def get_request(self, request_id: str) -> httpx.Response:
        return self.api_get(f"/api/requests/{request_id}")

    def get_response(self, request_id: str) -> httpx.Response:
        return self.api_get(f"/api/requests/{request_id}/response")

    def clear_requests(self) -> httpx.Response:
        return self.api_delete("/api/requests")

    def reload(self) -> httpx.Response:
        return self.api_post("/api/config/reload")

    def cleanup(self) -> httpx.Response:
        return self.api_post("/api/cleanup")

    def healthz(self) -> httpx.Response:
        return self.api_get("/api/healthz")


# ---------------------------------------------------------------------------
# Server-lifecycle fixture (factory)
# ---------------------------------------------------------------------------

@pytest.fixture()
def start_server(mockserver_bin: Path, tmp_path: Path):
    """Factory fixture — returns a callable that starts a mockserver."""

    servers: list[MockServer] = []
    examples_root = Path(__file__).resolve().parents[2] / "examples"

    def _start(
        examples: list[str],
        *,
        extra_args: list[str] | None = None,
        watch: bool = False,
        api_port: int | None = None,
        separate_api: bool = True,
        healthz_path: str = "/api/healthz",
    ) -> MockServer:
        mocks_dir = tmp_path / "mocks"
        data_dir = tmp_path / "data"
        mocks_dir.mkdir(exist_ok=True)
        data_dir.mkdir(exist_ok=True)

        # 1. Run `mockserver init`
        init_result = subprocess.run(
            [str(mockserver_bin), "init", str(mocks_dir)],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if init_result.returncode != 0:
            pytest.fail(f"mockserver init failed:\n{init_result.stderr}")

        # 2. Copy requested examples into mocks/
        for example in examples:
            src = examples_root / example
            if not src.exists():
                pytest.fail(f"Example {example!r} not found at {src}")
            dst = mocks_dir / example
            if dst.exists():
                shutil.rmtree(dst)
            shutil.copytree(src, dst)

        # 3. Run `mockserver check`
        check_result = subprocess.run(
            [str(mockserver_bin), "check", "--dir", str(mocks_dir)],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if check_result.returncode != 0:
            pytest.fail(
                f"mockserver check failed:\n{check_result.stdout}\n{check_result.stderr}"
            )

        # 4. Find free ports
        mock_port = _free_port()
        if separate_api:
            actual_api_port = api_port if api_port is not None else _free_port()
        else:
            actual_api_port = mock_port

        # 5. Build command
        cmd = [
            str(mockserver_bin),
            "serve",
            "--port", str(mock_port),
            "--dir", str(mocks_dir),
            "--data-dir", str(data_dir),
            "--idle-timeout", "0",
        ]

        if separate_api:
            cmd += ["--api-port", str(actual_api_port)]

        if not watch:
            cmd.append("--no-watch")

        if extra_args:
            cmd.extend(extra_args)

        # 6. Start server
        stdout_log = tmp_path / "server_stdout.log"
        stderr_log = tmp_path / "server_stderr.log"
        stdout_fh = open(stdout_log, "w")
        stderr_fh = open(stderr_log, "w")

        proc = subprocess.Popen(
            cmd,
            stdout=stdout_fh,
            stderr=stderr_fh,
        )

        server = MockServer(
            mock_port=mock_port,
            api_port=actual_api_port,
            mocks_dir=mocks_dir,
            data_dir=data_dir,
            process=proc,
            stdout_log=stdout_log,
            stderr_log=stderr_log,
            cmd=cmd,
            examples_loaded=list(examples),
        )
        servers.append(server)

        # 7. Wait for healthy
        healthz_url = f"http://127.0.0.1:{actual_api_port}{healthz_path}"
        deadline = time.monotonic() + 15
        last_err = None
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                stdout_fh.close()
                stderr_fh.close()
                pytest.fail(
                    f"Server exited early (rc={proc.returncode}).\n"
                    f"cmd: {' '.join(cmd)}\n"
                    f"stdout: {stdout_log.read_text()}\n"
                    f"stderr: {stderr_log.read_text()}"
                )
            try:
                r = httpx.get(healthz_url, timeout=2)
                if r.status_code == 200:
                    return server
            except Exception as exc:
                last_err = exc
            time.sleep(0.15)

        stdout_fh.close()
        stderr_fh.close()
        pytest.fail(
            f"Server did not become healthy within 15s.\n"
            f"Last error: {last_err}\n"
            f"cmd: {' '.join(cmd)}\n"
            f"stdout: {stdout_log.read_text()}\n"
            f"stderr: {stderr_log.read_text()}"
        )

    yield _start

    # Teardown: kill all servers
    for srv in servers:
        srv.process.terminate()
        try:
            srv.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            srv.process.kill()
            srv.process.wait(timeout=5)


# ---------------------------------------------------------------------------
# Failure diagnostics hook
# ---------------------------------------------------------------------------

@pytest.hookimpl(tryfirst=True, hookwrapper=True)
def pytest_runtest_makereport(item, call):
    """On test failure, dump server logs for diagnostics."""
    outcome = yield
    report = outcome.get_result()

    if report.when == "call" and report.failed:
        # Find MockServer instances attached to the test
        for name, val in item.funcargs.items():
            if isinstance(val, MockServer):
                _dump_server_logs(val, item.nodeid)


def _dump_server_logs(server: MockServer, node_id: str) -> None:
    sep = "=" * 60
    print(f"\n{sep}")
    print(f"SERVER DIAGNOSTICS for {node_id}")
    print(f"cmd: {' '.join(server.cmd)}")
    print(f"examples: {server.examples_loaded}")
    print(f"{sep}")
    if server.stdout_log.exists():
        print(f"--- stdout ({server.stdout_log}) ---")
        print(server.stdout_log.read_text()[-5000:])
    if server.stderr_log.exists():
        print(f"--- stderr ({server.stderr_log}) ---")
        print(server.stderr_log.read_text()[-5000:])
    print(sep)
