"""The CI monitor reads checks, classifies them and schedules its polls without touching gh.

The parts that call `gh` are not exercised here; everything below is a function over data,
so a wrong verdict or an unbounded poll interval is caught without a network.
"""
import contextlib
import io
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_monitor  # noqa: E402


def run(name, conclusion, status="completed", run_id=1):
    return {
        "id": run_id,
        "name": name,
        "status": status,
        "conclusion": conclusion,
        "workflow_run": {"id": run_id},
    }


class Classify(unittest.TestCase):
    def test_no_checks_yet_is_pending(self):
        self.assertEqual(ci_monitor.classify([]), (ci_monitor.PENDING, []))

    def test_a_running_check_keeps_the_whole_set_pending(self):
        runs = [run("Windows", "success", 1), run("Linux", None, "in_progress", 2)]
        state, pending = ci_monitor.classify(runs)
        self.assertEqual(state, ci_monitor.PENDING)
        self.assertEqual(pending, ["Linux"])

    def test_finished_with_failures_reports_only_the_failures(self):
        runs = [run("Windows", "success", 1), run("Linux", "failure", 2)]
        state, problems = ci_monitor.classify(runs)
        self.assertEqual(state, ci_monitor.DONE)
        self.assertEqual([problem["name"] for problem in problems], ["Linux"])

    def test_finished_clean_has_no_problems(self):
        runs = [run("Windows", "success", 1), run("Linux", "success", 2)]
        self.assertEqual(ci_monitor.classify(runs), (ci_monitor.DONE, []))


class Summary(unittest.TestCase):
    def test_every_check_gets_a_row_and_a_verdict(self):
        runs = [run("macOS", "success", 1), run("Linux", "failure", 2)]
        text = ci_monitor.summary(runs, "0123456789abcdef", "main")
        lines = text.splitlines()
        self.assertEqual(lines[0], "CI on 0123456 (main):")
        self.assertEqual(lines[1], "  Linux: failure")
        self.assertEqual(lines[2], "  macOS: success")
        self.assertEqual(lines[3], "  -> FAILED: Linux")

    def test_pending_checks_are_named(self):
        runs = [run("Linux", None, "queued", 1)]
        self.assertIn("still running: Linux", ci_monitor.summary(runs, "abc", "x"))

    def test_no_runs_says_so_instead_of_passing(self):
        self.assertIn("no checks have started yet",
                      ci_monitor.summary([], "abc", "x"))


class Interval(unittest.TestCase):
    def test_first_wait_is_the_default(self):
        self.assertEqual(ci_monitor.next_interval(0), ci_monitor.DEFAULT_INTERVAL)

    def test_the_wait_doubles(self):
        self.assertEqual(ci_monitor.next_interval(30), 60)

    def test_the_wait_is_capped(self):
        self.assertEqual(ci_monitor.next_interval(120), ci_monitor.MAX_INTERVAL)
        self.assertEqual(ci_monitor.next_interval(1e9), ci_monitor.MAX_INTERVAL)


class Watch(unittest.TestCase):
    def watch(self, runs, rerun=False, max_wait=60):
        log = io.StringIO()
        original_runs = ci_monitor.check_runs
        original_sleep = ci_monitor.time.sleep
        ci_monitor.check_runs = lambda repo, sha: runs
        ci_monitor.time.sleep = lambda seconds: None
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                return ci_monitor.watch("o/r", "abc", "main", rerun, max_wait, log), log
        finally:
            ci_monitor.check_runs = original_runs
            ci_monitor.time.sleep = original_sleep

    def test_a_finished_clean_set_returns_zero_and_writes_the_report(self):
        code, log = self.watch([run("Linux", "success", 1)])
        self.assertEqual(code, 0)
        self.assertIn("every check passed", log.getvalue())

    def test_a_failed_set_returns_nonzero(self):
        code, _ = self.watch([run("Linux", "failure", 1)])
        self.assertEqual(code, 1)

    def test_a_set_that_never_finishes_gives_up(self):
        code, _ = self.watch([run("Linux", None, "in_progress", 1)], max_wait=0)
        self.assertEqual(code, 2)

    def test_rerun_asks_for_the_failed_runs_only(self):
        calls = []
        original_gh = ci_monitor.subprocess.run
        ci_monitor.subprocess.run = lambda argv, check: calls.append(argv)
        try:
            code, _ = self.watch(
                [run("Linux", "failure", 1), run("Windows", "success", 2)], rerun=True)
        finally:
            ci_monitor.subprocess.run = original_gh
        self.assertEqual(code, 1)
        self.assertEqual(calls, [
            ["gh", "run", "rerun", "1", "--repo", "o/r", "--failed"]])


class CheckRuns(unittest.TestCase):
    def test_pages_are_merged_and_deduplicated(self):
        payload = (
            '[{"check_runs": [{"id": 2, "name": "b", "status": "completed",'
            ' "conclusion": "success", "workflow_run": {"id": 2}}]},'
            ' {"check_runs": [{"id": 1, "name": "a", "status": "completed",'
            ' "conclusion": "success", "workflow_run": {"id": 1}},'
            ' {"id": 2, "name": "b", "status": "completed",'
            ' "conclusion": "success", "workflow_run": {"id": 2}}]}]'
        )
        original = ci_monitor.subprocess.check_output
        ci_monitor.subprocess.check_output = lambda argv, text: payload
        try:
            runs = ci_monitor.check_runs("o/r", "abc")
        finally:
            ci_monitor.subprocess.check_output = original
        self.assertEqual([item["id"] for item in runs], [1, 2])


if __name__ == "__main__":
    unittest.main()
