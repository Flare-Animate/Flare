"""The nightly build is a rolling pre-release, never a tag release: it cannot start the release
workflow, it writes only to its own `nightly` tag and release, and the packages come from the same
reusable workflow a release uses.

Run with `python3 -m unittest discover -s scripts/tests -p 'test_nightly_workflow.py'`.
"""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github/workflows"


def read(name):
    return (WORKFLOWS / name).read_text(encoding="utf-8")


def trigger_block(text):
    """The lines of the top-level `on:` key, up to the next top-level key."""
    match = re.search(r"^on:\n(.*?)(?=^\S)", text, re.S | re.M)
    return match.group(1) if match else ""


class NightlyWorkflow(unittest.TestCase):
    def test_nightly_runs_on_a_schedule_and_by_hand_and_never_on_a_tag(self):
        trigger = trigger_block(read("nightly.yml"))
        self.assertIn("schedule:", trigger)
        self.assertIn("workflow_dispatch:", trigger)
        self.assertNotIn("tags", trigger)
        self.assertNotIn("push:", trigger)

    def test_nightly_publishes_only_to_its_own_tag_and_release(self):
        text = read("nightly.yml")
        pushes = re.findall(r"git push[^\n]*", text)
        self.assertEqual(len(pushes), 1, pushes)
        self.assertIn("refs/tags/nightly", pushes[0])
        self.assertIn("--force", pushes[0])
        for line in text.splitlines():
            if "gh release " in line and " nightly" not in line and "release view" not in line:
                self.assertIn("nightly", line, line)
        self.assertNotRegex(text, r"refs/tags/v|gh release (create|edit|upload|delete) v")
        self.assertNotIn("--cleanup-tag", text)

    def test_only_the_publish_job_can_write_to_the_repository(self):
        text = read("nightly.yml")
        self.assertEqual(text.count("contents: write"), 1)
        self.assertLess(text.index("publish:"), text.index("contents: write"))
        self.assertIn("permissions:\n  contents: read", text)

    def test_a_new_run_waits_for_a_publish_in_progress(self):
        self.assertIn("cancel-in-progress: false", read("nightly.yml"))

    def test_nightly_skips_when_no_commit_is_new(self):
        text = read("nightly.yml")
        self.assertIn('if [ "$previous" = "$sha" ]', text)
        self.assertIn("build=false", text)

    def test_nightly_builds_the_packages_with_the_release_workflow_steps(self):
        text = read("nightly.yml")
        self.assertIn("uses: ./.github/workflows/platforms.yml", text)
        self.assertIn("uses: ./.github/workflows/android.yml", text)
        self.assertNotIn("secrets:", text)
        self.assertNotIn("uses: ./.github/workflows/release.yml", text)

    def test_the_platform_workflow_is_only_called_and_the_release_still_triggers_on_tags(self):
        platforms = read("platforms.yml")
        self.assertIn("workflow_call:", trigger_block(platforms))
        self.assertNotIn("push:", trigger_block(platforms))
        release = read("release.yml")
        self.assertIn("tags: ['v*']", release)
        self.assertIn("uses: ./.github/workflows/platforms.yml", release)

    def test_the_nightly_version_has_the_shape_the_packagers_accept(self):
        text = read("nightly.yml")
        self.assertIn("-nightly.$(date -u +%Y%m%d).${sha:0:7}", text)


if __name__ == "__main__":
    unittest.main()
