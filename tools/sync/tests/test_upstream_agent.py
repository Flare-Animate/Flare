"""Tests for tools/sync/upstream_agent.py.

Covers the protected-path guarantee the sync agent advertises: an upstream
commit must never bring its own .github/ (or any other Flare-owned) file into
the sync branch, whether or not that file conflicts.
"""
import json
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

    write(root, "toonz/sources/toonz/foo.cpp", "// base\n")
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


def staged_renames(repo):
    """Staged paths with rename detection, i.e. one name per change."""
    out = git(repo, "diff", "--cached", "--name-only", "-M").stdout
    return sorted(f.strip() for f in out.splitlines() if f.strip())


def test_added_workflow_is_dropped_but_source_is_kept(repo):
    """The exact failure that broke every scheduled sync run.

    Upstream adds a workflow of its own and touches a source file in the same
    commit. The workflow must not survive the cherry-pick (GITHUB_TOKEN cannot
    push .github/workflows changes), the source file must.
    """
    write(repo, ".github/workflows/linux_build.yml", "name: OpenToonz Linux\n")
    write(repo, "toonz/sources/toonz/foo.cpp", "// OpenToonz feature\n")
    sha = upstream_commit(repo)

    assert agent.apply_commit(sha, agent.compile_rules([]))

    # The source file lands in Flare's tree, not upstream's: only flare/ is
    # compiled, so a synced file left at toonz/ would never build.
    assert not (repo / ".github/workflows/linux_build.yml").exists()
    assert (repo / "flare/sources/flare/foo.cpp").read_text() == "// Flare feature\n"
    assert "toonz/" not in "".join(staged_renames(repo)).replace(
        "toonz/sources/toonz/foo.cpp", "")


def test_modified_flare_owned_file_is_restored_to_head(repo):
    """A Flare-owned file that already exists keeps Flare's content."""
    write(repo, ".github/workflows/ci.yml", "name: OpenToonz CI\njobs: {}\n")
    write(repo, "toonz/sources/toonz/foo.cpp", "// other\n")
    sha = upstream_commit(repo)

    assert agent.apply_commit(sha, agent.compile_rules([]))

    assert (repo / "flare/sources/flare/foo.cpp").exists()
    assert (repo / ".github/workflows/ci.yml").read_text() == "name: Flare CI\n"


def test_readme_is_protected_but_the_source_tree_is_not(repo):
    """Flare's own docs stay protected; its source tree is a sync target.

    "flare/" used to be listed as Flare-only, which meant that after the path
    mapping produced flare/sources/flare/foo.cpp the file was dropped again as
    "Flare-owned" - so the agent could report a successful sync having applied
    nothing at all. Only paths upstream never writes should be protected.
    """
    write(repo, "README.md", "OpenToonz\n")
    write(repo, "toonz/sources/toonz/main.cpp", "// upstream main\n")
    write(repo, "toonz/sources/toonz/foo.cpp", "// other\n")
    sha = upstream_commit(repo)

    assert agent.apply_commit(sha, agent.compile_rules([]))

    # Both source files reach Flare's compiled tree. Neither carries a brand
    # string, so the rebrand pass leaves both alone.
    assert (repo / "flare/sources/flare/foo.cpp").read_text() == "// other\n"
    assert (repo / "flare/sources/flare/main.cpp").read_text() == "// upstream main\n"
    # Flare's own README is still protected.
    assert not (repo / "README.md").exists() or \
        (repo / "README.md").read_text() != "OpenToonz\n"


def test_protected_only_run_still_records_progress(repo):
    """A whole run whose commits touch only Flare-owned paths must still
    persist last_synced_sha, or every later run re-scans the same commits.

    Drives the real sync() against a local upstream remote, so the ordering of
    stage_state_file() and the has-anything-to-commit test is what is checked.
    """
    base = git(repo, "rev-parse", "HEAD").stdout.strip()
    write(repo, ".github/workflows/linux_build.yml", "name: OpenToonz Linux\n")
    sha = upstream_commit(repo)

    agent.save_state({"upstreams": {"ot": {"last_synced_sha": base}}})
    src = agent.UpstreamSource(key="ot", remote="up", url=str(repo),
                               branch="upstream")
    assert agent.sync([src], max_commits=10, dry_run=False) == 0

    state = json.loads(agent.STATE_FILE.read_text())
    assert state["upstreams"]["ot"]["last_synced_sha"] == sha

    committed = git(repo, "show", "--name-only", "--format=", "HEAD").stdout
    assert ".github/state.json" in committed
    assert "linux_build.yml" not in committed


def test_state_file_is_staged_for_the_sync_commit(repo):
    """Without this the last-synced SHAs never reach master."""
    assert agent.stage_state_file() is False        # nothing written yet

    agent.save_state({"upstreams": {"opentoonz": {"last_synced_sha": "abc123"}}})
    assert agent.stage_state_file() is True
    assert staged(repo) == [".github/state.json"]


# --- upstream path -> Flare path ------------------------------------------------

def test_upstream_toonz_paths_are_mapped_into_flare_tree():
    """Upstream's C++ lives under toonz/; Flare's lives under flare/.

    Without this mapping every synced commit lands in a dead shadow tree that no
    CMakeLists references, which is why the scheduled sync never produced a
    mergeable change.
    """
    cases = {
        "toonz/sources/toonz/flashimport.cpp": "flare/sources/flare/flashimport.cpp",
        "toonz/sources/toonzqt/styleeditor.cpp": "flare/sources/flareqt/styleeditor.cpp",
        "toonz/sources/toonzlib/preferences.cpp": "flare/sources/flarelib/preferences.cpp",
        "toonz/sources/common/timage/timage.cpp": "flare/sources/common/timage/timage.cpp",
        "toonz/sources/include/traster.h": "flare/sources/include/traster.h",
        "toonz/sources/stopmotion/stopmotion.cpp":
            "flare/sources/stopmotion/stopmotion.cpp",
        "toonz/CMakeLists.txt": "CMakeLists.txt",
        "toonz/cmake/FindSuperLU.cmake": "cmake/FindSuperLU.cmake",
        "toonz/installer/README.md": "packaging/README.md",
    }
    for src, want in cases.items():
        assert agent.map_upstream_path(src) == want, src


def test_paths_outside_the_renamed_tree_are_left_alone():
    """Only the renamed directory moves; doc/, thirdparty/ and friends stay."""
    for p in ("doc/architecture.rst", "thirdparty/zlib/zlib.h",
              "ci-scripts/linux/build.sh", "flare/sources/flare/flashimport.cpp",
              "plugins/example/CMakeLists.txt"):
        assert agent.map_upstream_path(p) == p, p


def test_tahoma2d_sources_map_into_flare():
    """Tahoma2D is a hard fork that already uses the flare/ layout."""
    assert (agent.map_upstream_path("tahoma2d/sources/toonz/x.cpp")
            == "flare/sources/toonz/x.cpp")


def test_a_touched_toonz_source_lands_under_flare(repo):
    """End to end: a cherry-picked edit ends up in the tree the build compiles."""
    write(repo, "toonz/sources/toonz/aboutpopup.cpp", "// upstream change\n")
    sha = upstream_commit(repo)

    assert agent.apply_commit(sha, agent.compile_rules([]))

    assert staged(repo) == ["flare/sources/flare/aboutpopup.cpp"]
    assert (repo / "flare/sources/flare/aboutpopup.cpp").exists()
    # The upstream path must no longer be staged, or the file is synced twice
    # under two names. (The directory itself still exists: the fixture seeds
    # toonz/sources/toonz/foo.cpp in the base commit, so it is tracked in HEAD.)
    assert "toonz/sources/toonz/aboutpopup.cpp" not in staged(repo)
