#!/usr/bin/env python3
"""
Flare upstream sync agent.

Requires Python 3.11+.

Pulls improvements from one or more upstream repos (opentoonz/opentoonz and
tahoma2d/tahoma2d) and applies them to Flare, automatically rebranding all
upstream-specific names to Flare.

Design principles
-----------------
* Cherry-pick commit-by-commit — Flare changes are never bulk-overwritten.
* Per-upstream state          — each source tracks its own last-synced SHA.
* Deduplication               — commits whose diff is identical to an already-
                                applied patch are skipped automatically.
* Smart conflict resolution   — branding-only conflicts keep Flare; functional
                                conflicts take upstream code and then rebrand.
* Protected paths             — Flare-only files/dirs are never touched.
"""

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from dataclasses import dataclass, field

if sys.version_info < (3, 11):
    sys.exit("upstream_agent.py requires Python 3.11+")

# ── repo root ─────────────────────────────────────────────────────────────────
REPO_ROOT  = Path(__file__).resolve().parents[2]
STATE_FILE = REPO_ROOT / ".github" / "upstream_sync_state.json"

# ── upstream source definitions ───────────────────────────────────────────────
@dataclass
class UpstreamSource:
    key:     str          # short id used in state file
    remote:  str          # git remote name
    url:     str          # clone URL
    branch:  str = "master"
    # Extra rebrand rules specific to this upstream (applied on top of shared ones)
    extra_rules: list[tuple[str, str, int]] = field(default_factory=list)


UPSTREAMS: list[UpstreamSource] = [
    UpstreamSource(
        key    = "opentoonz",
        remote = "upstream-opentoonz",
        url    = "https://github.com/opentoonz/opentoonz.git",
        extra_rules = [
            # opentoonz-specific version-header class name (already in shared rules)
        ],
    ),
    UpstreamSource(
        key    = "tahoma2d",
        remote = "upstream-tahoma2d",
        url    = "https://github.com/tahoma2d/tahoma2d.git",
        extra_rules = [
            # Tahoma2D brand names → Flare
            (r"\bTahoma2D\b",                "Flare",   0),
            (r"\btahoma2d\b",                "flare",   0),
            (r"\bTAHOMA2D\b",                "FLARE",   0),
            (r"\bTahoma\b",                  "Flare",   0),
            (r"\btahoma\b",                  "flare",   0),
            (r"\bTAHOMA\b",                  "FLARE",   0),
            # Tahoma env-var prefix (used like TAHOMA_ROOT, etc.)
            (r"\bTAHOMARROOT\b",             "FLAREROOT",          0),
            (r"\bTAHOMAPROJECTS\b",          "FLAREPROJECTS",      0),
            (r"\bTAHOMACACHEROOT\b",         "FLARECACHEROOT",     0),
            (r"\bTAHOMAPROFILES\b",          "FLAREPROFILES",      0),
            # GitHub URLs
            (r"github\.com/tahoma2d/tahoma2d", "github.com/Flare-Animate/Flare", 0),
            (r"tahoma2d\.github\.io",          "flare-animate.github.io",        0),
            # Install paths
            (r"/opt/tahoma2d",               "/opt/flare",  0),
            (r"lib/tahoma2d",                "lib/flare",   0),
            (r"/share/tahoma2d",             "/share/flare", 0),
            # Tahoma-specific shortcut preset filename
            (r"\bdeftahoma2d\.ini\b",        "defflare.ini", 0),
        ],
    ),
]

# ── upstream path -> Flare path ──────────────────────────────────────────────
# Upstream OpenToonz keeps its entire C++ tree under "toonz/". Flare renamed
# that directory to "flare/" but left every other path alone, so the two repos
# share no file paths for the code that matters.
#
# Without this mapping the agent wrote upstream's commits out at their original
# "toonz/sources/..." paths. Nothing in the build references "toonz/", so the
# functional fixes landed in files the compiler never sees while "flare/" was
# simultaneously protected by FLARE_ONLY_PREFIXES and kept at its old
# contents. That is why seven sync branches piled up unmerged, and why the two
# newest ones carry 24k lines of dead shadow tree under toonz/.
#
# Paths are rewritten in the index after a cherry-pick and before rebranding.
# Longest prefix wins, so more specific entries must come first.
UPSTREAM_PATH_MAP: tuple[tuple[str, str], ...] = (
    # toonz/sources/<module>/...  ->  flare/sources/<module>/...
    ("toonz/sources/toonz/",    "flare/sources/flare/"),
    ("toonz/sources/toonzqt/",  "flare/sources/flareqt/"),
    ("toonz/sources/toonzlib/", "flare/sources/flarelib/"),
    ("toonz/sources/toonzfarm/", "flare/sources/flarefarm/"),
    # Sub-tree moves inside the toonz/ root itself.
    ("toonz/installer/",        "packaging/"),
    ("toonz/cmake/",            "cmake/"),
    ("toonz/README.md",         "doc/UPSTREAM_README_opentoonz.md"),
    # Any other toonz/sources/<module>/ (common, include, tnz*, image, sound,
    # stopmotion, ...) keeps its module name; only the root moves.
    ("toonz/sources/",          "flare/sources/"),
    # The top-level build file and everything else.
    ("toonz/CMakeLists.txt",    "CMakeLists.txt"),
    ("toonz/",                  ""),
    # Tahoma2D is a hard fork that already uses the Flare layout.
    ("tahoma2d/sources/",       "flare/sources/"),
    ("tahoma2d/",               ""),
)


def map_upstream_path(rel_path: str) -> str:
    """Translate one upstream-relative path into its Flare equivalent.

    Returns "" when the path has no counterpart and should be dropped.
    """
    for src, dst in UPSTREAM_PATH_MAP:
        if rel_path.startswith(src):
            return dst + rel_path[len(src):]
    return rel_path


# ── files/directories Flare owns (never overwrite from any upstream) ──────────
# NOTE: "flare/" is deliberately absent, even though it is Flare's own source
# tree. It is also the image of upstream's "toonz/" tree once UPSTREAM_PATH_MAP
# has been applied, so protecting it wholesale threw away precisely the commits
# we are trying to sync: every remapped file was immediately dropped again as
# "Flare-owned", and the sync agent reported success while applying nothing.
#
# Flare-authored files are still safe. Upstream has no "flare/" directory, so a
# synced commit can only ever mention "flare/..." as the mapped image of a
# "toonz/..." path it actually changed. Where both sides edited the same lines,
# resolve_conflicts() already decides per hunk: upstream wins on functional
# changes, Flare on branding-only ones.
FLARE_ONLY_PREFIXES = (
    "README.md",
    ".github/",
    "tools/sync/",
    "tools/rebrand/",
    "stuff/profiles/layouts/shortcuts/defflare.ini",
    "packaging/",
    "cmake/",
)

# ── shared rebrand rules (applied for ALL upstreams) ─────────────────────────
# Order matters: most-specific first.
SHARED_REBRAND_RULES: list[tuple[str, str, int]] = [
    # Quoted include paths. The rename moved toonz/sources/<module>/x.h to
    # flare/sources/<module>/x.h, and the path map fixes the *file locations* --
    # but nothing rewrote the #include lines pointing at them, so the synced tree
    # compiled against paths that do not exist:
    #
    #   fatal error C1083: Cannot open include file: 'toonz/levelproperties.h'
    #
    # 37 files were left that way. The rules are ordered before the branding ones
    # because they are unambiguous: inside a quoted include, "toonz/" is always
    # the old layout.
    (r'(#include\s*[<"])toonz/',        r'\1flare/',            0),
    (r'(#include\s*[<"])(toonz4\.6)/',   r'\1flare/',            0),

    # Internal C++ names already rebranded in Flare
    (r"\bToonzVersion\b",              "FlareVersion",         0),
    (r"\bToonzFolder\b",               "FlareFolder",          0),

    # Include guards and paths that embed the module directory name.
    (r'(#include\s*[<"])toonzqt/',      r'\1flareqt/',          0),
    (r'(#include\s*[<"])toonzlib/',     r'\1flarelib/',         0),

    # System-var prefix
    (r'systemVarPrefix\s*=\s*"TOONZ"', 'systemVarPrefix = "FLARE"', 0),
    (r"\bTOONZROOT\b",                 "FLAREROOT",            0),
    (r"\bTOONZPROJECTS\b",             "FLAREPROJECTS",        0),
    (r"\bTOONZCACHEROOT\b",            "FLARECACHEROOT",       0),
    (r"\bTOONZPROFILES\b",             "FLAREPROFILES",        0),
    (r"\bTOONZFXPRESETS\b",            "FLAREFXPRESETS",       0),
    (r"\bTOONZSTUDIOPALETTE\b",        "FLARESTUDIOPALETTE",   0),
    (r"\bTOONZCONFIG\b",               "FLARECONFIG",          0),
    (r"\bTOONZLIBRARY\b",              "FLARELIBRARY",         0),

    # Brand name in UI strings, comments, docs
    (r"\bOpenToonz\b",                 "Flare",                0),
    (r"\bopentoonz\b",                 "flare",                0),
    (r"\bOPENTOONZ\b",                 "FLARE",                0),

    # GitHub / web URLs
    (r"github\.com/opentoonz/opentoonz",
     "github.com/Flare-Animate/Flare",                         0),
    (r"opentoonz\.github\.io",         "flare-animate.github.io",   0),
    (r"opentoonz\.readthedocs\.io",    "flare-animate.readthedocs.io", 0),

    # Install / library paths
    (r"/opt/opentoonz",                "/opt/flare",           0),
    (r"lib/opentoonz",                 "lib/flare",            0),
    (r"/share/opentoonz",              "/share/flare",         0),

    # Shortcut preset filename
    (r"\bdefopentoonz\.ini\b",         "defflare.ini",         0),
]

# Extensions eligible for text rebranding
REBRAND_EXTS = {
    ".cpp", ".h", ".c", ".hpp",
    ".cmake", ".txt",
    ".yml", ".yaml",
    ".py", ".sh", ".cmd", ".bat",
    ".ini", ".xml", ".qrc", ".rc", ".ui", ".ts",
    ".md",
    ".desktop", ".appdata.xml",
}

# Pattern for detecting branding-only conflict lines
_BRANDING_PAT = re.compile(
    r"OpenToonz|opentoonz|ToonzVersion|ToonzFolder|TOONZROOT|"
    r"systemVarPrefix.*TOONZ|defopentoonz|"
    r"Tahoma2D|tahoma2d|TAHOMA",
    re.IGNORECASE,
)


# ── helpers ────────────────────────────────────────────────────────────────────

def run(cmd: list[str], check=True, capture=False,
        cwd: Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd, check=check, capture_output=capture,
        text=True, cwd=cwd or REPO_ROOT,
    )

def git(args: list[str], check=True, capture=False) -> subprocess.CompletedProcess:
    return run(["git"] + args, check=check, capture=capture)


def load_state() -> dict:
    if STATE_FILE.exists():
        return json.loads(STATE_FILE.read_text())
    return {"upstreams": {}}

def save_state(state: dict) -> None:
    STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
    STATE_FILE.write_text(json.dumps(state, indent=2))

def stage_state_file() -> bool:
    """Stage the sync state so the last-synced SHAs travel with the PR.

    save_state() only writes the file. Nothing ever staged it, so the state
    never reached master and every scheduled run re-scanned the same ~20
    upstream commits instead of resuming where the previous sync stopped.
    """
    if not STATE_FILE.exists():
        return False
    git(["add", "--", str(STATE_FILE.relative_to(REPO_ROOT))], check=False)
    return True


def compile_rules(extra: list[tuple[str, str, int]]) -> list[tuple[re.Pattern, str]]:
    rules = SHARED_REBRAND_RULES + extra
    return [(re.compile(pat, flags), repl) for pat, repl, flags in rules]


def rebrand_text(text: str,
                 compiled: list[tuple[re.Pattern, str]]) -> str:
    for pattern, repl in compiled:
        text = pattern.sub(repl, text)
    return text


def rebrand_file(path: Path,
                 compiled: list[tuple[re.Pattern, str]]) -> bool:
    if path.suffix.lower() not in REBRAND_EXTS:
        return False
    try:
        original = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return False
    rebranded = rebrand_text(original, compiled)
    if rebranded != original:
        path.write_text(rebranded, encoding="utf-8")
        return True
    return False


def rebrand_staged(compiled: list[tuple[re.Pattern, str]]) -> int:
    result = git(["diff", "--cached", "--name-only"], capture=True)
    files = [f.strip() for f in result.stdout.splitlines() if f.strip()]
    changed = 0
    for rel in files:
        p = REPO_ROOT / rel
        if p.exists() and rebrand_file(p, compiled):
            git(["add", rel], check=False)
            changed += 1
    return changed


def is_flare_only(rel_path: str) -> bool:
    return rel_path.startswith(FLARE_ONLY_PREFIXES)


def drop_flare_only_from_index() -> list[str]:
    """Undo staged changes to paths Flare owns, restoring them to HEAD.

    resolve_conflicts() already honours FLARE_ONLY_PREFIXES, but only for files
    git reported as conflicted. An upstream commit that *adds* a file under one
    of those prefixes cherry-picks cleanly and so never reaches that check — it
    just lands in the sync commit. That is how upstream's .github/workflows/
    files ended up in every sync branch, and GITHUB_TOKEN may not push workflow
    changes at all, so the push (and the whole run) failed.
    """
    staged = git(["diff", "--cached", "--name-only"], capture=True).stdout
    dropped = [f.strip() for f in staged.splitlines()
               if f.strip() and is_flare_only(f.strip())]
    for rel in dropped:
        # capture=True only to swallow cat-file's "exists on disk, but not in
        # 'HEAD'" note — that is the expected answer for an upstream addition.
        in_head = git(["cat-file", "-e", f"HEAD:{rel}"],
                      check=False, capture=True).returncode == 0
        if in_head:
            git(["checkout", "HEAD", "--", rel], check=False)
        else:
            # Added by upstream and absent from Flare — drop it entirely.
            git(["rm", "-f", "-q", "--", rel], check=False)
    return dropped


def patch_fingerprint(sha: str) -> str:
    """Hash of the diff produced by a commit (used for deduplication)."""
    result = subprocess.run(
        ["git", "diff-tree", "--no-commit-id", "-p", sha],
        check=False, capture_output=True,  # bytes — avoids UnicodeDecodeError on binary diffs
        cwd=REPO_ROOT,
    )
    return hashlib.sha256(result.stdout).hexdigest()


def resolve_conflicts(compiled: list[tuple[re.Pattern, str]]) -> list[str]:
    result = git(["diff", "--name-only", "--diff-filter=U"], capture=True)
    conflicts = [f.strip() for f in result.stdout.splitlines() if f.strip()]
    unresolved = []
    for rel in conflicts:
        if is_flare_only(rel):
            git(["checkout", "--ours",   rel], check=False)
            git(["add", rel], check=False)
            continue
        # Count branding vs functional lines in the conflict
        p = REPO_ROOT / rel
        try:
            content = p.read_text(encoding="utf-8", errors="replace")
        except OSError:
            git(["checkout", "--ours", rel], check=False)
            git(["add", rel], check=False)
            continue
        brand_lines = func_lines = 0
        in_block = False
        for line in content.splitlines():
            if line.startswith("<<<<<<<"):
                in_block = True
            elif line.startswith(">>>>>>>"):
                in_block = False
            elif in_block and not line.startswith("======="):
                if _BRANDING_PAT.search(line):
                    brand_lines += 1
                else:
                    func_lines += 1
        if func_lines == 0:
            # Purely branding conflict → keep Flare
            git(["checkout", "--ours",   rel], check=False)
            git(["add", rel], check=False)
        else:
            # Functional change → take upstream, then rebrand
            git(["checkout", "--theirs", rel], check=False)
            rebrand_file(REPO_ROOT / rel, compiled)
            git(["add", rel], check=False)
    return unresolved


# ── per-upstream sync ──────────────────────────────────────────────────────────

def setup_remote(src: UpstreamSource) -> None:
    remotes = git(["remote"], capture=True).stdout.split()
    if src.remote not in remotes:
        git(["remote", "add", src.remote, src.url])
        print(f"  Added remote {src.remote} → {src.url}")
    git(["fetch", src.remote, "--no-tags"])
    print(f"  Fetched {src.remote}/{src.branch}")


def new_commits(src: UpstreamSource, last_sha: str | None,
                seen_fps: set[str]) -> list[dict]:
    ref = f"{src.remote}/{src.branch}"
    rng = f"{last_sha}..{ref}" if last_sha else f"{ref}~20..{ref}"
    result = git(
        ["log", "--reverse", "--format=%H\t%s\t%ae", rng],
        capture=True, check=False,
    )
    commits = []
    for line in result.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        parts = line.split("\t", 2)
        sha = parts[0]
        fp  = patch_fingerprint(sha)
        if fp in seen_fps:
            continue            # already applied from another upstream
        commits.append({
            "sha":     sha,
            "subject": parts[1] if len(parts) > 1 else "",
            "author":  parts[2] if len(parts) > 2 else "",
            "fp":      fp,
        })
    return commits


def remap_staged_paths() -> list[tuple[str, str]]:
    """Rewrite staged paths from their upstream layout into Flare's.

    Runs immediately after the cherry-pick and before
    drop_flare_only_from_index(): a commit that adds toonz/sources/toonz/foo.cpp
    is still staged under the upstream name at that point, so it has to be moved
    to flare/sources/flare/foo.cpp before the Flare-only check can judge it.

    The staged blob is copied rather than the working-tree file so the move is
    independent of what happens to be on disk, and the upstream entry is
    explicitly removed from the index so a file cannot be synced twice under
    both names.
    """
    # Deletions (D) are in here deliberately: an upstream commit that removes
    # toonz/sources/toonz/foo.cpp has to remove Flare's flare/sources/flare/foo.cpp,
    # and a filter of ACMR left that file behind for good while staging a
    # deletion at a path Flare does not even have.
    staged = git(["diff", "--cached", "--name-status", "--diff-filter=ACMRD"],
                 check=False, capture=True).stdout.splitlines()
    moved: list[tuple[str, str]] = []
    for raw in staged:
        line = raw.decode("utf-8", "replace") if isinstance(raw, bytes) else raw
        if not line.strip():
            continue
        fields = line.split("\t")
        status = fields[0]
        # A rename or copy reports both paths; the destination is what is staged.
        old_path = fields[-1]
        new_path = map_upstream_path(old_path)
        if new_path == old_path:
            continue

        if status.startswith("D"):
            # The upstream file is gone, so Flare's mapped copy has to go too --
            # that is the whole point of seeing D here.
            if new_path:
                git(["rm", "-f", "-q", "--ignore-unmatch", "--", new_path],
                    check=False)
            # Then un-stage the upstream name, the same guarantee the add/modify
            # path gives: nothing is ever left staged under the upstream layout.
            # Restoring from HEAD covers index and working tree in one go, and
            # is a harmless no-op where Flare has no such path at all.
            git(["checkout", "-q", "HEAD", "--", old_path], check=False)
            moved.append((old_path, new_path))
            continue

        if new_path:
            # git() captures text; use a raw call so binary members survive.
            blob = subprocess.run(["git", "show", ":" + old_path],
                                  cwd=REPO_ROOT, capture_output=True, check=False)
            target = REPO_ROOT / new_path
            target.parent.mkdir(parents=True, exist_ok=True)
            if blob.returncode == 0:
                target.write_bytes(blob.stdout)
            else:
                # No staged content (a pure rename, say): fall back to whatever
                # is in the working tree.
                src_file = REPO_ROOT / old_path
                if not src_file.is_file():
                    moved.append((old_path, new_path))
                    continue
                target.write_bytes(src_file.read_bytes())
            git(["add", "--", new_path], check=False)

        # Retire the upstream entry: from the index and from the working tree.
        # -f is required because a file the cherry-pick just added is "modified"
        # relative to HEAD, and git rm refuses to touch those without it.
        git(["rm", "--cached", "-f", "-q", "--ignore-unmatch", "--", old_path],
            check=False)
        try:
            (REPO_ROOT / old_path).unlink()
        except OSError:
            pass
        # Prune directories the move just emptied, so a sync does not leave a
        # hollow toonz/ tree behind for the next run to trip over.
        pruned = (REPO_ROOT / old_path).parent
        while pruned != REPO_ROOT and pruned.is_dir():
            try:
                next(pruned.iterdir())
                break          # still holds something; leave it alone
            except StopIteration:
                try:
                    pruned.rmdir()
                except OSError:
                    break
                pruned = pruned.parent
        moved.append((old_path, new_path))
    return moved


def apply_commit(sha: str,
                 compiled: list[tuple[re.Pattern, str]]) -> bool:
    result = git(["cherry-pick", "--no-commit", "-x", sha], check=False)
    if result.returncode != 0:
        unresolved = resolve_conflicts(compiled)
        if unresolved:
            print(f"    ⚠  unresolved: {unresolved}")
            git(["cherry-pick", "--abort"], check=False)
            return False
    remapped = remap_staged_paths()
    if remapped:
        print(f"    ⇄  remapped {len(remapped)} path(s) into flare/")
    dropped = drop_flare_only_from_index()
    if dropped:
        print(f"    ⊘  kept Flare's own: {', '.join(dropped[:5])}"
              + (f" (+{len(dropped) - 5} more)" if len(dropped) > 5 else ""))
    n = rebrand_staged(compiled)
    if n:
        print(f"    ✎  rebranded {n} file(s)")
    return True


def sync_source(src: UpstreamSource, state: dict,
                seen_fps: set[str], max_commits: int,
                dry_run: bool) -> tuple[list[dict], list[dict]]:
    """
    Sync one upstream source.
    Returns (applied, skipped) lists.
    """
    print(f"\n{'─'*60}")
    print(f"  Upstream: {src.key}  ({src.url})")
    print(f"{'─'*60}")

    setup_remote(src)

    src_state = state["upstreams"].setdefault(src.key, {"last_synced_sha": None})
    last_sha  = src_state.get("last_synced_sha")

    commits = new_commits(src, last_sha, seen_fps)
    if not commits:
        print(f"  ✅ Already up to date.")
        return [], []

    if len(commits) > max_commits:
        print(f"  ℹ  {len(commits)} new commits; capping at {max_commits}")
        commits = commits[:max_commits]

    print(f"  🔄 {len(commits)} commit(s) to apply\n")

    compiled = compile_rules(src.extra_rules)
    applied: list[dict]  = []
    skipped: list[dict]  = []

    for c in commits:
        sha, subj = c["sha"], c["subject"]
        print(f"  [{sha[:8]}] {subj[:72]}")
        if dry_run:
            print("    (dry-run)")
            applied.append(c)
            seen_fps.add(c["fp"])
            continue
        ok = apply_commit(sha, compiled)
        if ok:
            applied.append(c)
            seen_fps.add(c["fp"])
            src_state["last_synced_sha"] = sha
            src_state.setdefault("synced_commits", []).append(sha)
            save_state(state)
        else:
            skipped.append(c)
            print(f"    ✗  skipped (conflict)")

    return applied, skipped


# ── main ───────────────────────────────────────────────────────────────────────

def sync(sources: list[UpstreamSource], max_commits: int,
         dry_run: bool) -> int:
    state     = load_state()
    seen_fps: set[str] = set()          # deduplication across upstreams
    all_applied: list[tuple[str, dict]] = []
    all_skipped: list[tuple[str, dict]] = []

    for src in sources:
        applied, skipped = sync_source(
            src, state, seen_fps, max_commits, dry_run
        )
        all_applied.extend((src.key, c) for c in applied)
        all_skipped.extend((src.key, c) for c in skipped)

    # Final commit
    if not dry_run and all_applied:
        # Staged before the has-anything-to-commit test on purpose: when every
        # applied commit turned out to touch only Flare-owned paths there is no
        # content left, and skipping the commit would throw away the progress
        # those commits represent — the next run would re-scan them forever.
        stage_state_file()
        staged = git(["diff", "--cached", "--name-only"],
                     capture=True).stdout.strip()
        if staged:
            msg = [f"Sync {len(all_applied)} commit(s) from upstream", ""]
            for key, c in all_applied:
                msg.append(f"  [{key}] {c['sha'][:8]} {c['subject'][:60]}")
            if all_skipped:
                msg += ["", "Skipped (conflicts):"]
                for key, c in all_skipped:
                    msg.append(f"  [{key}] {c['sha'][:8]} {c['subject'][:60]}")
            git(["commit", "-m", "\n".join(msg)])
            print(f"\n✅ Committed {len(all_applied)} upstream commit(s).")
        else:
            print("\nℹ  No staged changes after sync.")

    if all_skipped:
        print(f"\n⚠  {len(all_skipped)} commit(s) skipped.")
        return 1
    return 0


def _make_console_safe() -> None:
    """Let the agent output reach a Windows console.

    The progress rules and status symbols are U+2500, U+2139, U+1F504 and
    friends, none of which exist in cp1252 -- the default for a Windows
    console. Printing one raised UnicodeEncodeError and killed the run before
    any work happened, which is why the upstream commits the orphaned
    sync-upstream branches carry were never landed: the tool that would land
    them failed to start.

    errors="replace" rather than "strict": an
    unencodable character becomes a question mark, so output can never abort a
    run whatever the console is.
    """
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            # Python < 3.7, or a stream with no reconfigure. A console that
            # genuinely cannot cope will still say so, which is better than
            # not running at all.
            pass


def main() -> None:
    _make_console_safe()
    parser = argparse.ArgumentParser(
        description="Flare multi-upstream sync agent",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python upstream_agent.py                     # sync both\n"
            "  python upstream_agent.py --source opentoonz  # one only\n"
            "  python upstream_agent.py --dry-run           # preview\n"
        ),
    )
    parser.add_argument(
        "--source",
        choices=[s.key for s in UPSTREAMS] + ["all"],
        default="all",
        help="Which upstream(s) to sync (default: all)",
    )
    parser.add_argument(
        "--max-commits", type=int, default=50,
        help="Max commits per upstream in one run (default: 50)",
    )
    parser.add_argument(
        "--dry-run", action="store_true",
        help="List commits without applying",
    )
    parser.add_argument(
        "--reset-state", action="store_true",
        help="Wipe tracked state (re-scans last ~20 commits per upstream)",
    )
    args = parser.parse_args()

    if args.reset_state:
        STATE_FILE.unlink(missing_ok=True)
        print("State reset.")

    sources = (
        UPSTREAMS if args.source == "all"
        else [s for s in UPSTREAMS if s.key == args.source]
    )
    sys.exit(sync(sources, args.max_commits, args.dry_run))


if __name__ == "__main__":
    main()
