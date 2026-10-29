#!/usr/bin/env python3
"""Require successful builds from the tagged source before reusing release artifacts."""
import argparse
import json
import subprocess

import package_release as release


def verify(run, jobs, tag_sha):
    if run.get("status") != "completed" or run.get("head_sha") != tag_sha:
        raise ValueError("Artifact source must be a completed run of the exact tagged commit")
    if run.get("path") != ".github/workflows/release.yml":
        raise ValueError("Artifacts must come from the release workflow")
    if run.get("event") not in ("push", "workflow_dispatch"):
        raise ValueError("Pull-request artifacts cannot be published")
    for target in release.TARGETS:
        name = f"Build {target}"
        matches = [job for job in jobs if job.get("name") == name]
        if (len(matches) != 1 or matches[0].get("status") != "completed"
                or matches[0].get("conclusion") != "success"):
            raise ValueError(f"Required release build did not succeed: {name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--run", required=True)
    parser.add_argument("--tag", required=True)
    args = parser.parse_args()
    if not args.run.isascii() or not args.run.isdecimal() or int(args.run) <= 0:
        parser.error("--run must be a positive workflow run ID")
    release.validate(args.tag)
    tag_sha = subprocess.check_output(
        ["git", "rev-parse", "--verify", f"refs/tags/{args.tag}^{{commit}}"], text=True).strip()
    endpoint = f"repos/{args.repo}/actions/runs/{args.run}"
    run = json.loads(subprocess.check_output(["gh", "api", endpoint], text=True))
    pages = json.loads(subprocess.check_output(
        ["gh", "api", endpoint + "/jobs?per_page=100", "--paginate", "--slurp"], text=True))
    verify(run, [job for page in pages for job in page["jobs"]], tag_sha)
    print(f"Verified all {len(release.TARGETS)} required builds from {tag_sha}")


if __name__ == "__main__":
    main()
