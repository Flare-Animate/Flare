"""End-to-end check of the command line tool: make a project, draw, export, round-trip it through
Animate's file format and publish it, on the real binary and the real fixtures.

Run after building: `cargo build -p flare-cli`, then
`python3 -m unittest discover -s scripts/tests -p 'test_cli_smoke.py'`.
Skipped when the binary has not been built.
"""
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CLI = Path(os.environ.get("FLARE_CLI", ROOT / "target/debug/flare-cli"))
FIXTURES = ROOT / "crates/io/flare-import/tests/fixtures"


@unittest.skipUnless(CLI.is_file(), f"{CLI} is not built")
class CommandLineSmoke(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.dir = Path(self.directory.name)

    def run_cli(self, *args, ok=True):
        done = subprocess.run([str(CLI), *map(str, args)], capture_output=True, text=True)
        if ok:
            self.assertEqual(done.returncode, 0, f"{args}: {done.stderr}")
        else:
            self.assertNotEqual(done.returncode, 0, f"{args} should have been refused")
        return done

    def info(self, path):
        return json.loads(self.run_cli("info", path).stdout)

    def drawn_project(self):
        project = self.dir / "a.flare"
        self.run_cli("new", project)
        self.run_cli("set-stage", project, "--width", 320, "--height", 180, "--background", "202020")
        self.run_cli("draw-rect", project, "--rect", 20, 20, 100, 60, "--color", "FF6A00",
                     "--stroke", "000000", "--duration", 12)
        self.run_cli("frame", "insert-keyframe", project, "--at", 6)
        return project

    def test_drawing_and_exporting_a_frame(self):
        project = self.drawn_project()
        png = self.dir / "out.png"
        self.run_cli("export", project, png, "--frame", 3)
        data = png.read_bytes()
        self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
        width, height = int.from_bytes(data[16:20], "big"), int.from_bytes(data[20:24], "big")
        self.assertEqual((width, height), (320, 180))

    def test_a_filled_outlined_shape_survives_a_round_trip_through_fla_as_one_layer(self):
        project = self.drawn_project()
        fla, back = self.dir / "out.fla", self.dir / "back.flare"
        self.run_cli("export-xfl", project, fla)
        self.run_cli("import", fla, back)
        layers = self.info(back)["scenes"][0]["layers"]
        self.assertEqual([(l["name"], l["spans"]) for l in layers], [("Layer 1", 2)])
        # Exporting never overwrites what is there.
        self.run_cli("export-xfl", project, fla, ok=False)

    def test_publishing_a_swf(self):
        project = self.drawn_project()
        swf = self.dir / "out.swf"
        self.run_cli("publish-swf", project, swf)
        self.assertIn(swf.read_bytes()[:3], (b"CWS", b"FWS", b"ZWS"))

    def test_publishing_an_html5_page_that_plays_when_its_player_is_run(self):
        project = self.drawn_project()
        page = self.dir / "out.html"
        published = json.loads(self.run_cli("publish-html5", project, page).stdout)
        self.assertEqual(published["frames"], 12)
        text = page.read_text()
        self.assertTrue(text.startswith("<!DOCTYPE html>"))
        self.run_cli("publish-html5", project, page, ok=False)  # never overwrites
        node = shutil.which("node")
        if node is None:
            self.skipTest("node is not installed, so the player was not run")
        harness = ROOT / "scripts/tests/html5_player_harness.js"
        ran = subprocess.run([node, str(harness), str(page)], capture_output=True, text=True)
        self.assertEqual(ran.returncode, 0, ran.stderr)
        report = json.loads(ran.stdout)
        # The 320 x 180 stage is drawn at its own size, and the first frame has the shape.
        self.assertEqual(report["size"], [320, 180])
        names = [call[0] for call in report["first"]]
        self.assertIn("fillRect", names)
        self.assertIn("fill", names)
        self.assertIn("stroke", names)
        fills = [c for c in report["first"] if c[0] == "set fillStyle"]
        self.assertIn("rgba(255,106,0,1)", [c[1] for c in fills])
        # It keeps playing, and a held drawing is drawn the same way each time.
        self.assertGreater(report["rafLeft"], 0)
        self.assertTrue(all("fill" in [c[0] for c in frame] for frame in report["later"]))
        self.assertEqual(report["frames"], 12)

    def test_a_shape_tween_with_an_ease_is_made_and_played_in_the_page(self):
        project = self.drawn_project()  # a rectangle held for 12 frames, a keyframe at 6
        self.run_cli("frame", "shape-tween", project, "--at", 0)
        self.run_cli("frame", "set-ease", project, "--at", 0, "--ease", -40)
        spans = self.info(project)["scenes"][0]["layers"][0]["spans"]
        self.assertEqual(spans, 2)
        # Only a shape can be shape-tweened, and a span must be there to tween.
        self.run_cli("frame", "motion-tween", project, "--at", 0, ok=False)
        self.run_cli("frame", "shape-tween", project, "--at", 0, ok=False)  # already set
        self.run_cli("frame", "set-ease", project, "--at", 0, ok=False)  # --ease is needed
        page = self.dir / "tween.html"
        self.run_cli("publish-html5", project, page)
        text = page.read_text()
        data = json.loads(text.split('type="application/json">')[1].split("</script>")[0])
        # Six tweened steps, then the second keyframe held for six frames: seven pictures.
        self.assertEqual(len(data["seq"]), 12)
        self.assertEqual(len(set(data["seq"])), 7)
        self.run_cli("frame", "remove-tween", project, "--at", 0)
        self.run_cli("frame", "remove-tween", project, "--at", 0, ok=False)  # nothing to remove

    def test_a_page_with_sounds_waits_for_a_click_and_plays_each_at_its_cue_every_pass(self):
        node = shutil.which("node")
        if node is None:
            self.skipTest("node is not installed, so the player was not run")
        project = self.drawn_project()
        page = self.dir / "sound.html"
        self.run_cli("publish-html5", project, page)
        text = page.read_text()
        marker = 'type="application/json">'
        head, rest = text.split(marker, 1)
        body, tail = rest.split("</script>", 1)
        data = json.loads(body)
        # Two sounds, cued at frames 2 and 8 of the 12.
        data["sounds"] = [
            {"name": "a", "mime": "audio/wav", "data": "QQ=="},
            {"name": "b", "mime": "audio/mpeg", "data": "Qg=="},
        ]
        data["cues"] = [[2, 0], [8, 1]]
        page.write_text(head + marker + json.dumps(data) + "</script>" + tail)
        harness = ROOT / "scripts/tests/html5_player_harness.js"
        ran = subprocess.run([node, str(harness), str(page)], capture_output=True, text=True)
        self.assertEqual(ran.returncode, 0, ran.stderr)
        report = json.loads(ran.stdout)
        self.assertEqual(report["startShown"], "flex", "the page asks for a click first")
        self.assertEqual(report["queuedBeforeClick"], 0, "and does not play before it")
        plays = [(e[1], e[2]) for e in report["audio"] if e[0] == "play"]
        # Sound 0 at frame 2 and sound 1 at frame 8, then again on the next pass (12 frames on).
        self.assertEqual(plays, [(0, 2), (1, 8), (0, 14), (1, 20)])
        pauses = [e[2] for e in report["audio"] if e[0] == "pause"]
        self.assertTrue(pauses and all(t >= 12 for t in pauses), "a new pass stops what was playing")
        sources = [e[2] for e in report["audio"] if e[0] == "new"]
        self.assertTrue(sources[0].startswith("data:audio/wav;base64,"))
        self.assertTrue(sources[1].startswith("data:audio/mpeg;base64"))

    def test_the_html5_options_reach_the_page(self):
        project = self.drawn_project()
        page = self.dir / "fixed.html"
        self.run_cli("publish-html5", project, page, "--no-loop", "--no-center", "--responsive")
        text = page.read_text()
        data = json.loads(text.split('type="application/json">')[1].split("</script>")[0])
        self.assertEqual((data["loop"], data["responsive"]), (False, True))
        self.assertNotIn("body{display:flex", text)

    def test_real_files_import_and_render(self):
        for name in ("sample.fla", "handmade.fla"):
            project = self.dir / (name + ".flare")
            self.run_cli("import", FIXTURES / name, project)
            self.run_cli("export", project, self.dir / (name + ".png"))
            self.assertGreater((self.dir / (name + ".png")).stat().st_size, 100)

    def test_bad_input_is_refused_not_a_crash(self):
        project = self.drawn_project()
        self.run_cli("info", self.dir / "missing.flare", ok=False)
        self.run_cli("draw-rect", project, "--rect", 0, 0, -5, 10, ok=False)
        self.run_cli("export", project, self.dir / "x.png", "--width", 99999999, ok=False)
        junk = self.dir / "junk.fla"
        junk.write_bytes(b"PK\x03\x04 not really a zip")
        refused = self.run_cli("import", junk, self.dir / "j.flare", ok=False)
        self.assertNotIn("panicked", refused.stderr)


if __name__ == "__main__":
    unittest.main()
