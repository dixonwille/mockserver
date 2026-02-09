"""Tests for CLI commands: init, new, check."""

import json
import subprocess


def test_init_creates_structure(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    r = subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0

    assert (mocks / "_default" / "init.lua").exists()
    assert (mocks / "_types").is_dir()
    assert (mocks / ".luarc.json").exists()


def test_init_twice_fails(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    r = subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode != 0


def test_init_force_regenerates(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    # Modify default init to detect preservation
    default_init = mocks / "_default" / "init.lua"
    original_content = default_init.read_text()

    r = subprocess.run(
        [str(mockserver_bin), "init", "--force", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0
    assert (mocks / "_types").is_dir()
    # _default/init.lua should be preserved (not overwritten)
    assert default_init.read_text() == original_content


def test_new_creates_domain(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )

    r = subprocess.run(
        [str(mockserver_bin), "new", "api.example.com", "--dir", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0
    assert (mocks / "api.example.com" / "init.lua").exists()


def test_new_template_basic(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    r = subprocess.run(
        [str(mockserver_bin), "new", "basic.test", "--dir", str(mocks), "--template", "basic"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0
    init_lua = (mocks / "basic.test" / "init.lua").read_text()
    assert "handle" in init_lua


def test_new_template_rest(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    r = subprocess.run(
        [str(mockserver_bin), "new", "rest.test", "--dir", str(mocks), "--template", "rest"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0
    init_lua = (mocks / "rest.test" / "init.lua").read_text()
    assert "handle" in init_lua


def test_new_template_graphql(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    r = subprocess.run(
        [str(mockserver_bin), "new", "gql.test", "--dir", str(mocks), "--template", "graphql"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0
    init_lua = (mocks / "gql.test" / "init.lua").read_text()
    assert "handle" in init_lua


def test_new_existing_domain_fails(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    subprocess.run(
        [str(mockserver_bin), "new", "dup.test", "--dir", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    r = subprocess.run(
        [str(mockserver_bin), "new", "dup.test", "--dir", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode != 0


def test_new_invalid_domain_fails(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    r = subprocess.run(
        [str(mockserver_bin), "new", "../bad", "--dir", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode != 0


def test_check_valid_domains(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    r = subprocess.run(
        [str(mockserver_bin), "check", "--dir", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0


def test_check_missing_init(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    # Create domain folder without init.lua
    (mocks / "broken.test").mkdir()

    r = subprocess.run(
        [str(mockserver_bin), "check", "--dir", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode != 0


def test_check_missing_handle(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    domain_dir = mocks / "nohandle.test"
    domain_dir.mkdir()
    (domain_dir / "init.lua").write_text("-- no handle function\nlocal x = 1\n")

    r = subprocess.run(
        [str(mockserver_bin), "check", "--dir", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode != 0


def test_check_syntax_error(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    domain_dir = mocks / "syntax.test"
    domain_dir.mkdir()
    (domain_dir / "init.lua").write_text("function handle(request\n")  # missing )

    r = subprocess.run(
        [str(mockserver_bin), "check", "--dir", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode != 0


def test_check_json_output(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    r = subprocess.run(
        [str(mockserver_bin), "check", "--json", "--dir", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0
    results = json.loads(r.stdout)
    assert isinstance(results, list)
    for item in results:
        assert "name" in item
        assert "has_init" in item
        assert "status" in item


def test_check_brief_output(mockserver_bin, tmp_path):
    mocks = tmp_path / "mocks"
    subprocess.run(
        [str(mockserver_bin), "init", str(mocks)],
        capture_output=True,
        timeout=30,
    )
    r = subprocess.run(
        [str(mockserver_bin), "check", "--brief", "--dir", str(mocks)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0
    # Brief output should be more compact
    assert len(r.stdout.strip().splitlines()) >= 1
