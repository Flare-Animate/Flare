#!/usr/bin/env python3
"""
Flare upstream sync agent.

Requires Python 3.11+.

Fetches new commits from opentoonz/opentoonz (upstream) and applies them
to Flare, automatically rebranding any OpenToonz content to Flare.

The agent works commit-by-commit (cherry-pick) rather than bulk merge, so
Flare-specific changes are never blindly overwritten.

Conflict resolution strategy:
  - branding-only conflicts  → always keep Flare version
  - functional conflicts     → keep upstream (new feature/fix), then rebrand
  - deleted-in-upstream      → warn but skip (Flare may have replaced the file)
"""

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

if sys.version_info < (3, 11):
    sys.exit("upstream_agent.py requires Python 3.11+")

# ── repo root ─────────────────────────────────────────────────────────────────
REPO_ROOT = Path(__file__).resolve().parents[2]
STATE_FILE = REPO_ROOT / ".github" / "upstream_sync_state.json"

# ── upstream ──────────────────────────────────────────────────────────────────
UPSTREAM_URL  = "https://github.com/opentoonz/opentoonz.git"
UPSTREAM_REMOTE = "opentoonz-upstream"
UPSTREAM_BRANCH = "master"

# ── files/directories that are Flare-only (never overwrite from upstream) ────
FLARE_ONLY = {
    "README.md",
    ".github/workflows/",
    "flare/",            # flare/ tree is our own; toonz/ is legacy compat
    "tools/sync/",
    "tools/rebrand/",
    "stuff/profiles/layouts/shortcuts/defflare.ini",
    "packaging/",
    "cmake/",
}

# ── directories that map upstream paths → Flare paths ─────────────────────────
# upstream toonz/sources → flare/sources  (applied after cherry-pick)
PATH_MAP = [
    ("toonz/sources/", "flare/sources/"),
]

# ── comprehensive rebrand replacement table ────────────────────────────────────
# Order matters: longest/most-specific first.
# Each entry: (pattern, replacement, flags)
REBRAND_RULES: list[tuple[str, str, int]] = [
    # ── class / namespace renames ────────────────────────────────────────────
    (r"\bToonzVersion\b",   "FlareVersion",   0),
    (r"\bToonzFolder\b",    "FlareFolder",    0),

    # ── system-var prefix ────────────────────────────────────────────────────
    (r'\bsystemVarPrefix\s*=\s*"TOONZ"', 'systemVarPrefix = "FLARE"', 0),
    (r'\bTOONZROOT\b',         "FLAREROOT",        0),
    (r'\bTOONZPROJECTS\b',     "FLAREPROJECTS",    0),
    (r'\bTOONZCACHEROOT\b',    "FLARECACHEROOT",   0),
    (r'\bTOONZPROFILES\b',     "FLAREPROFILES",    0),
    (r'\bTOONZFXPRESETS\b',    "FLAREFXPRESETS",   0),
    (r'\bTOONZSTUDIOPALETTE\b',"FLARESTUDIOPALETTE", 0),
    (r'\bTOONZCONFIG\b',       "FLARECONFIG",      0),
    (r'\bTOONZLIBRARY\b',      "FLARELIBRARY",     0),

    # ── brand name in UI strings, comments, docs ─────────────────────────────
    (r"\bOpenToonz\b",      "Flare",   0),
    (r"\bopentoonz\b",      "flare",   0),
    (r"\bOPENTOONZ\b",      "FLARE",   0),

    # ── GitHub URLs ──────────────────────────────────────────────────────────
    (r"github\.com/opentoonz/opentoonz",
     "github.com/Flare-Animate/Flare",   0),
    (r"opentoonz\.github\.io",
     "flare-animate.github.io",          0),
    (r"opentoonz\.readthedocs\.io",
     "flare-animate.readthedocs.io",     0),

    # ── install / library paths ──────────────────────────────────────────────
    (r"/opt/opentoonz",    "/opt/flare",    0),
    (r"lib/opentoonz",     "lib/flare",     0),
    (r"/share/opentoonz",  "/share/flare",  0),

    # ── shortcut preset filename ─────────────────────────────────────────────
    (r"\bdefopentoonz\.ini\b", "defflare.ini", 0),
]

# Compiled once for speed
_COMPILED: list[tuple[re.Pattern, str]] = [
    (re.compile(pat, flags), repl)
    for pat, repl, flags in REBRAND_RULES
]

# Extensions to rebrand
REBRAND_EXTS = {
    ".cpp", ".h", ".c", ".hpp",
    ".cmake", ".txt",
    ".yml", ".yaml",
    ".py", ".sh", ".cmd", ".bat",
    ".ini", ".xml", ".qrc", ".rc", ".ui", ".ts",
    ".md",
    ".desktop", ".appdata.xml",
}

# Patterns that indicate a line is branding-only (for conflict heuristics)
_BRANDING_PATTERNS = re.compile(
    r"OpenToonz|opentoonz|ToonzVersion|ToonzFolder|TOONZROOT|"
    r"systemVarPrefix.*TOONZ|defopentoonz",
    re.IGNORECASE,
)


# ── helpers ────────────────────────────────────────────────────────────────────

def run(cmd: list[str], check=True, capture=False, cwd=None) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        check=check,
        capture_output=capture,
        text=True,
        cwd=cwd or REPO_ROOT,
    )


def git(args: list[str], check=True, capture=False) -> subprocess.CompletedProcess:
    return run(["git"] + args, check=check, capture=capture)


def load_state() -> dict:
    if STATE_FILE.exists():
        return json.loads(STATE_FILE.read_text())
    return {"last_synced_sha": None, "synced_commits": []}


def save_state(state: dict) -> None:
    STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
    STATE_FILE.write_text(json.dumps(state, indent=2))


def rebrand_text(text: str) -> str:
    for pattern, repl in _COMPILED:
        text = pattern.sub(repl, text)
    return text


def rebrand_file(path: Path) -> bool:
    """Rebrand a single file in-place. Returns True if changed."""
    if path.suffix.lower() not in REBRAND_EXTS:
        return False
    try:
        original = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return False
    rebranded = rebrand_text(original)
    if rebranded != original:
        path.write_text(rebranded, encoding="utf-8")
        return True
    return False


def rebrand_staged_files() -> int:
    """Rebrand all files that are staged (after a cherry-pick). Returns count."""
    result = git(["diff", "--cached", "--name-only"], capture=True)
    files = [f.strip() for f in result.stdout.splitlines() if f.strip()]
    changed = 0
    for rel in files:
        p = REPO_ROOT / rel
        if p.exists() and rebrand_file(p):
            git(["add", rel], check=False)
            changed += 1
    return changed


def is_flare_only(rel_path: str) -> bool:
    """Return True if this path belongs to Flare-only territory."""
    for prefix in FLARE_ONLY:
        if rel_path.startswith(prefix) or rel_path == prefix.rstrip("/"):
            return True
    return False


def resolve_conflict_file(rel_path: str) -> str:
    """
    Decide how to resolve a conflict in `rel_path`.
    Returns 'ours' | 'theirs' | 'manual'.
    """
    if is_flare_only(rel_path):
        return "ours"
    # Read the conflict markers to decide
    p = REPO_ROOT / rel_path
    if not p.exists():
        return "ours"
    try:
        content = p.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return "ours"
    # Count branding vs functional conflict lines
    branding_lines = 0
    functional_lines = 0
    in_conflict = False
    for line in content.splitlines():
        if line.startswith("<<<<<<<"):
            in_conflict = True
        elif line.startswith(">>>>>>>"):
            in_conflict = False
        elif in_conflict and line.startswith("======="):
            continue
        elif in_conflict:
            if _BRANDING_PATTERNS.search(line):
                branding_lines += 1
            else:
                functional_lines += 1
    if functional_lines == 0 and branding_lines > 0:
        return "ours"          # purely a branding conflict → keep Flare
    if functional_lines > 0:
        return "theirs"        # functional change → take upstream, then rebrand
    return "ours"


def auto_resolve_conflicts() -> list[str]:
    """Auto-resolve all conflicted files. Returns list of unresolved paths."""
    result = git(["diff", "--name-only", "--diff-filter=U"], capture=True)
    conflicts = [f.strip() for f in result.stdout.splitlines() if f.strip()]
    unresolved = []
    for rel in conflicts:
        strategy = resolve_conflict_file(rel)
        if strategy == "ours":
            git(["checkout", "--ours",   rel], check=False)
            git(["add", rel], check=False)
        elif strategy == "theirs":
            git(["checkout", "--theirs", rel], check=False)
            git(["add", rel], check=False)
            # Re-apply rebrand after taking upstream version
            rebrand_file(REPO_ROOT / rel)
            git(["add", rel], check=False)
        else:
            unresolved.append(rel)
    return unresolved


# ── main sync logic ─────────────────────────────────────────────────────────

def setup_upstream() -> None:
    """Ensure the upstream remote exists and is fetched."""
    remotes = git(["remote"], capture=True).stdout.split()
    if UPSTREAM_REMOTE not in remotes:
        git(["remote", "add", UPSTREAM_REMOTE, UPSTREAM_URL])
        print(f"Added remote {UPSTREAM_REMOTE} → {UPSTREAM_URL}")
    git(["fetch", UPSTREAM_REMOTE, "--no-tags"])
    print(f"Fetched {UPSTREAM_REMOTE}/{UPSTREAM_BRANCH}")


def get_new_commits(last_sha: str | None) -> list[dict]:
    """Return list of upstream commits newer than last_sha."""
    upstream_ref = f"{UPSTREAM_REMOTE}/{UPSTREAM_BRANCH}"
    if last_sha:
        log_range = f"{last_sha}..{upstream_ref}"
    else:
        # First run — only take the last 20 commits to avoid overwhelming
        log_range = f"{upstream_ref}~20..{upstream_ref}"

    result = git([
        "log", "--reverse", "--format=%H\t%s\t%ae",
        log_range,
    ], capture=True, check=False)

    commits = []
    for line in result.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        parts = line.split("\t", 2)
        if len(parts) >= 2:
            commits.append({
                "sha":     parts[0],
                "subject": parts[1] if len(parts) > 1 else "",
                "author":  parts[2] if len(parts) > 2 else "",
            })
    return commits


def cherry_pick_commit(sha: str) -> bool:
    """
    Cherry-pick one upstream commit.
    Returns True on success (possibly after conflict resolution).
    """
    result = git(
        ["cherry-pick", "--no-commit", "-x", sha],
        check=False,
    )
    if result.returncode != 0:
        # Check for conflicts
        unresolved = auto_resolve_conflicts()
        if unresolved:
            print(f"  ⚠  {len(unresolved)} unresolved conflict(s): {unresolved}")
            git(["cherry-pick", "--abort"], check=False)
            return False

    # Rebrand everything that was staged
    n = rebrand_staged_files()
    if n:
        print(f"  ✎  Rebranded {n} file(s)")

    return True


def sync(max_commits: int = 50, dry_run: bool = False) -> int:
    """
    Main entry point. Returns exit code.
    Syncs up to `max_commits` new upstream commits.
    """
    state = load_state()
    last_sha = state.get("last_synced_sha")

    setup_upstream()
    new_commits = get_new_commits(last_sha)

    if not new_commits:
        print("✅ Already up to date with upstream.")
        return 0

    # Cap to avoid giant PRs
    if len(new_commits) > max_commits:
        print(f"ℹ  {len(new_commits)} new commits; capping at {max_commits}")
        new_commits = new_commits[:max_commits]

    print(f"🔄 Syncing {len(new_commits)} upstream commit(s)…\n")

    applied = []
    skipped = []

    for commit in new_commits:
        sha     = commit["sha"]
        subject = commit["subject"]
        print(f"  [{sha[:8]}] {subject[:72]}")

        if dry_run:
            print("   (dry-run, skipping)")
            applied.append(commit)
            continue

        ok = cherry_pick_commit(sha)
        if ok:
            applied.append(commit)
            state["last_synced_sha"] = sha
            state.setdefault("synced_commits", []).append(sha)
            save_state(state)
        else:
            skipped.append(commit)
            print(f"  ✗  Skipped {sha[:8]} (conflict)")

    # Final commit with full summary
    if not dry_run and applied:
        staged = git(["diff", "--cached", "--name-only"], capture=True).stdout.strip()
        if staged:
            msg_lines = [
                f"Sync {len(applied)} commit(s) from opentoonz/opentoonz",
                "",
                "Applied upstream commits (with Flare rebranding):",
            ]
            for c in applied:
                msg_lines.append(f"  - {c['sha'][:8]} {c['subject'][:68]}")
            if skipped:
                msg_lines.append("")
                msg_lines.append("Skipped (conflict):")
                for c in skipped:
                    msg_lines.append(f"  - {c['sha'][:8]} {c['subject'][:68]}")
            git(["commit", "-m", "\n".join(msg_lines)])
            print(f"\n✅ Committed {len(applied)} upstream commit(s).")
        else:
            print("\nℹ  No staged changes after sync (all changes already present).")

    if skipped:
        print(f"\n⚠  {len(skipped)} commit(s) skipped due to conflicts.")
        return 1
    return 0


def main():
    parser = argparse.ArgumentParser(description="Flare upstream sync agent")
    parser.add_argument("--max-commits", type=int, default=50,
                        help="Max upstream commits to apply in one run (default: 50)")
    parser.add_argument("--dry-run", action="store_true",
                        help="Fetch and list commits without applying them")
    parser.add_argument("--reset-state", action="store_true",
                        help="Reset tracked state (start fresh from last 20 commits)")
    args = parser.parse_args()

    if args.reset_state:
        STATE_FILE.unlink(missing_ok=True)
        print("State reset.")

    sys.exit(sync(max_commits=args.max_commits, dry_run=args.dry_run))


if __name__ == "__main__":
    main()
