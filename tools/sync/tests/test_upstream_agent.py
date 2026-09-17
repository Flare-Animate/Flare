"""Tests for tools/sync/upstream_agent.py.

Covers the protected-path guarantee the sync agent advertises: an upstream
commit must never bring its own .github/ (or any other Flare-owned) file into
the sync branch, whether or not that file conflicts.
"""
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import upstream_agent as agent  # noqa: E402


def git(repo, *args, check=True):
    return subprocess.run(["git"] + list(args), cwd=repo, check=check,
                          capture_output=True, text=True)


def write(repo, rel, text):
    path = repo / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


@pytest.fixture
def repo(tmp_path, monkeypatch):
    """A git repo with a master commit and an 'upstream' branch off it."""
    root = tmp_path / "flare"
    root.mkdir()
    git(root, "init", "-q", "-b", "master")
    git(root, "config", "user.email", "t@example.com")
    git(root, "config", "user.name", "t")
    git(root, "config", "commit.gpgsign", "false")

    write(root, "toonz/sources/foo.cpp", "// base\n")
    write(root, ".github/workflows/ci.yml", "name: Flare CI\n")
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", "base")

    git(root, "checkout", "-q", "-b", "upstream")
    monkeypatch.setattr(agent, "REPO_ROOT", root)
    monkeypatch.setattr(agent, "STATE_FILE", root / ".github" / "state.json")
    yield root


def upstream_commit(repo, message="upstream change"):
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", message)
    sha = git(repo, "rev-parse", "HEAD").stdout.strip()
    git(repo, "checkout", "-q", "master")
    return sha


def staged(repo):
    out = git(repo, "diff", "--cached", "--name-only").stdout
    return sorted(f.strip() for f in out.splitlines() if f.strip())


def test_added_workflow_is_dropped_but_source_is_kept(repo):
    """The exact failure that broke every scheduled sync run.

    Upstream adds a workflow of its own and touches a source file in the same
    commit. The workflow must not survive the cherry-pick (GITHUB_TOKEN cannot
    push .github/workflows changes), the source file must.
    """
    write(repo, ".github/workflows/linux_build.yml", "name: OpenToonz Linux\n")
    write(repo, "toonz/sources/foo.cpp", "// OpenToonz feature\n")
    sha = upstream_commit(repo)

    assert agent.apply_commit(sha, agent.compile_rules([]))

    assert staged(repo) == ["toonz/sources/foo.cpp"]
    assert not (repo / ".github/workflows/linux_build.yml").exists()
    assert (repo / "toonz/sources/foo.cpp").read_text() == "// Flare feature\n"


def test_modified_flare_owned_file_is_restored_to_head(repo):
    """A Flare-owned file that already exists keeps Flare's content."""
    write(repo, ".github/workflows/ci.yml", "name: OpenToonz CI\njobs: {}\n")
    write(repo, "toonz/sources/foo.cpp", "// other\n")
    sha = upstream_commit(repo)

    assert agent.apply_commit(sha, agent.compile_rules([]))

    assert staged(repo) == ["toonz/sources/foo.cpp"]
    assert (repo / ".github/workflows/ci.yml").read_text() == "name: Flare CI\n"


def test_readme_and_flare_dir_are_protected(repo):
    """FLARE_ONLY_PREFIXES is honoured beyond .github/."""
    write(repo, "README.md", "OpenToonz\n")
    write(repo, "flare/sources/main.cpp", "// upstream main\n")
    write(repo, "toonz/sources/foo.cpp", "// other\n")
    sha = upstream_commit(repo)

    assert agent.apply_commit(sha, agent.compile_rules([]))

    assert staged(repo) == ["toonz/sources/foo.cpp"]
    assert not (repo / "flare/sources/main.cpp").exists()


def test_state_file_is_staged_for_the_sync_commit(repo):
    """Without this the last-synced SHAs never reach master."""
    assert agent.stage_state_file() is False        # nothing written yet

    agent.save_state({"upstreams": {"opentoonz": {"last_synced_sha": "abc123"}}})
    assert agent.stage_state_file() is True
    assert staged(repo) == [".github/state.json"]
