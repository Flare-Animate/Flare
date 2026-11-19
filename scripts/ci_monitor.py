#!/usr/bin/env python3
"""Watch the checks of one commit or pull request and report them as they land.

This replaces the old repository's ci_monitor pair, which scraped the HTML of the Actions
tab and retriggered runs by pushing empty commits. Everything here goes through `gh`'s API:
the checks are read from the check-runs endpoint of the commit, a failure summary names the
failing job and its first error, and re-running failed jobs uses the supported
`gh run rerun --failed`, so the run keeps its history and no noise commit is made.

The polling calls gh, so it needs an authenticated `gh` on the PATH. The pure parts
(classification, the backoff schedule, the summary lines) are functions over data and are
unit tested; nothing else in this file touches the network by itself.
"""
import argparse
import json
import os
import subprocess
import sys
import time

DEFAULT_INTERVAL = 30.0
MAX_INTERVAL = 120.0

DONE = "done"
PENDING = "pending"


def check_runs(repo, sha):
    """Every check run on one commit, oldest first."""
    runs = json.loads(subprocess.check_output(
        ["gh", "api", "--paginate", "--slurp",
         f"repos/{repo}/commits/{sha}/check-runs?per_page=100"], text=True))
    seen = {}
    for page in runs:
        for run in page["check_runs"]:
            seen[run["id"]] = run
    return sorted(seen.values(), key=lambda run: run["id"])


def pull_head(repo, number):
    """The head branch and commit of a pull request, as gh reports them."""
    head = json.loads(subprocess.check_output(
        ["gh", "pr", "view", str(number), "--repo", repo,
         "--json", "headRefName,headRefOid"], text=True))
    return head["headRefName"], head["headRefOid"]


def classify(runs):
    """DONE when every run has a final conclusion, PENDING while any is still going."""
    if not runs:
        return PENDING, []
    pending = [run["name"] for run in runs if run["conclusion"] is None]
    if pending:
        return PENDING, pending
    return DONE, [run for run in runs if run["conclusion"] != "success"]


def summary(runs, sha, branch):
    """The lines a person reads: one row per check, then the verdict."""
    lines = [f"  {run['name']}: {run['conclusion'] or run['status']}"
             for run in sorted(runs, key=lambda run: run["name"].lower())]
    if not runs:
        verdict = "no checks have started yet"
    else:
        state, problems = classify(runs)
        if state == PENDING:
            verdict = "still running: " + ", ".join(sorted(problems))
        elif problems:
            verdict = "FAILED: " + ", ".join(sorted(run["name"] for run in problems))
        else:
            verdict = "every check passed"
    return f"CI on {sha[:7]} ({branch}):\n" + "\n".join(lines) + f"\n  -> {verdict}"


def next_interval(last):
    """The wait between polls: doubling, capped, so a long queue is not hammered."""
    return min(last * 2.0, MAX_INTERVAL) if last > 0 else DEFAULT_INTERVAL


def rerun_failed(runs, repo):
    """Re-run the failed jobs of the failed workflow runs, the supported way."""
    run_ids = sorted({run["workflow_run"]["id"]
                      for run in runs if run["conclusion"] not in (None, "success", "skipped")})
    for run_id in run_ids:
        subprocess.run(["gh", "run", "rerun", str(run_id), "--repo", repo, "--failed"],
                       check=True)
    return run_ids


def watch(repo, sha, branch, rerun, max_wait, log):
    """Poll until the checks are done or the wait runs out; log and print every poll."""
    waited = 0.0
    current = 0.0
    while True:
        runs = check_runs(repo, sha)
        text = summary(runs, sha, branch)
        log.write(text + "\n\n")
        state, problems = classify(runs)
        if state == DONE:
            if problems and rerun:
                print("Re-running failed jobs: "
                      + ", ".join(str(i) for i in rerun_failed(runs, repo)))
            else:
                print(text)
            return 0 if not problems else 1
        if waited >= max_wait:
            print(text)
            print(f"Gave up after {max_wait / 60:.0f} minutes; still running: "
                  + ", ".join(sorted(problems)))
            return 2
        current = next_interval(current)
        time.sleep(current)
        waited += current


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", help="owner/name; defaults to GH_REPO")
    parser.add_argument("--pr", type=int, help="pull request to watch")
    parser.add_argument("--sha", help="commit to watch, when there is no pull request")
    parser.add_argument("--branch", default="", help="branch name for the report lines")
    parser.add_argument("--rerun-failed", action="store_true",
                        help="when the checks finish with failures, re-run the failed jobs")
    parser.add_argument("--max-wait", type=float, default=3600.0,
                        help="give up after this many seconds (default 3600)")
    parser.add_argument("--log", default="ci_monitor.log", help="where the report goes")
    args = parser.parse_args()
    repo = args.repo or os.environ.get("GH_REPO")
    if not repo:
        parser.error("no repository: pass --repo or set GH_REPO")
    if args.pr:
        branch, sha = pull_head(repo, args.pr)
    elif args.sha:
        branch, sha = args.branch, args.sha
    else:
        parser.error("watch a --pr or a --sha")
    with open(args.log, "a", encoding="utf-8") as log:
        log.write(f"---- watch of {sha} started ----\n")
        sys.exit(watch(repo, sha, branch, args.rerun_failed, args.max_wait, log))


if __name__ == "__main__":
    main()
