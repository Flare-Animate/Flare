"""The page's Test Movie (apps/flare-web/src/lib/flare-ruffle.js) against a fake page, and its Rust
hand-off compiled to WebAssembly, and that the Ruffle package the page loads is vendored whole.

The overlay test needs `node`. The hand-off needs the wasm32-unknown-unknown target and
`wasm-bindgen-test-runner`. The vendor check needs `npm ci` in apps/flare-web. Each is skipped
without what it needs. Run with
`python3 -m unittest discover -s scripts/tests -p 'test_web_ruffle.py'`.
"""
import functools
import http.server
import json
import os
import re
import shutil
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WEB = ROOT / "apps/flare-web"
NODE = shutil.which("node") is not None
PACKAGE = WEB / "node_modules/@ruffle-rs/ruffle"
CLI = Path(os.environ.get("FLARE_CLI", ROOT / "target/debug/flare"))


def find_chrome():
    for candidate in (
        os.environ.get("CHROME"),
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        shutil.which("google-chrome"),
        shutil.which("chromium"),
        shutil.which("chromium-browser"),
    ):
        if candidate and Path(candidate).is_file():
            return candidate
    return None


CHROME = find_chrome()


@unittest.skipUnless(NODE, "needs node")
class Overlay(unittest.TestCase):
    def test_the_overlay_opens_replaces_closes_and_reports_failures(self):
        done = subprocess.run(
            ["node", "scripts/test_flare_ruffle.mjs"], cwd=WEB, capture_output=True, text=True
        )
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn("ok: flare-ruffle.js", done.stdout)


@unittest.skipUnless(
    NODE
    and shutil.which("wasm-bindgen-test-runner") is not None
    and shutil.which("rustup") is not None
    and "wasm32-unknown-unknown" in subprocess.run(
        ["rustup", "target", "list", "--installed"], capture_output=True, text=True
    ).stdout,
    "needs node, the wasm32 target and wasm-bindgen-test-runner",
)
class HandOff(unittest.TestCase):
    def test_the_movie_reaches_the_player_and_failures_come_back_as_messages(self):
        env = dict(
            os.environ,
            CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="wasm-bindgen-test-runner",
        )
        done = subprocess.run(
            ["cargo", "test", "-p", "flare-web", "--target", "wasm32-unknown-unknown", "--test", "ruffle"],
            cwd=ROOT, env=env, capture_output=True, text=True,
        )
        self.assertEqual(done.returncode, 0, done.stdout[-3000:] + done.stderr[-3000:])
        self.assertIn("3 passed", done.stdout + done.stderr)


@unittest.skipUnless(PACKAGE.is_dir(), "needs `npm ci` in apps/flare-web")
class Vendoring(unittest.TestCase):
    def test_the_script_its_chunks_wasm_and_licences_are_copied_and_maps_are_not(self):
        with tempfile.TemporaryDirectory() as out:
            done = subprocess.run(
                ["node", "scripts/vendor.mjs", out], cwd=WEB, capture_output=True, text=True
            )
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            names = sorted(p.name for p in (Path(out) / "ruffle").iterdir())
            self.assertIn("ruffle.js", names)
            self.assertIn("LICENSE_MIT", names)
            self.assertIn("LICENSE_APACHE", names)
            self.assertTrue(any(n.endswith(".wasm") for n in names))
            self.assertTrue(any(n.startswith("core.ruffle.") and n.endswith(".js") for n in names))
            self.assertFalse(any(n.endswith(".map") for n in names), names)
            # The script loads its core chunk by name, so a chunk it names must be there.
            script = (Path(out) / "ruffle/ruffle.js").read_text()
            self.assertTrue(
                any(n in script or n.split(".")[2] in script for n in names if n.startswith("core.ruffle.")),
                "ruffle.js names none of the chunks that were copied",
            )


@unittest.skipUnless(
    NODE and PACKAGE.is_dir() and CHROME and CLI.is_file(),
    "needs node, `npm ci` in apps/flare-web, Chrome (or CHROME) and a built flare-cli",
)
class RealBrowser(unittest.TestCase):
    """The page's own code and the vendored Ruffle, in a headless Chrome, on a movie we published."""

    def serve(self, site):
        class Quiet(http.server.SimpleHTTPRequestHandler):
            def log_message(self, *args):
                pass

        handler = functools.partial(Quiet, directory=str(site))
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return server.server_address[1]

    def result(self, swf_bytes):
        work = tempfile.TemporaryDirectory()
        self.addCleanup(work.cleanup)
        site = Path(work.name) / "site"
        site.mkdir()
        subprocess.run(["node", "scripts/vendor.mjs", str(site / "vendor")], cwd=WEB, check=True, capture_output=True)
        shutil.copy(WEB / "src/lib/flare-ruffle.js", site)
        shutil.copy(WEB / "tests/ruffle_page.html", site / "index.html")
        (site / "movie.swf").write_bytes(swf_bytes)
        port = self.serve(site)
        done = subprocess.run(
            [CHROME, "--headless=new", "--disable-gpu", "--no-sandbox",
             f"--user-data-dir={Path(work.name) / 'profile'}", "--virtual-time-budget=30000",
             "--dump-dom", f"http://127.0.0.1:{port}/index.html"],
            capture_output=True, text=True, timeout=120,
        )
        found = re.search(r'<pre id="result">([^<]*)</pre>', done.stdout)
        self.assertIsNotNone(found, done.stdout[-1500:] + done.stderr[-1500:])
        return found.group(1)

    def published(self):
        work = tempfile.TemporaryDirectory()
        self.addCleanup(work.cleanup)
        project, swf = Path(work.name) / "m.flare", Path(work.name) / "m.swf"
        for args in (["new", project], ["draw-rect", project, "--rect", 10, 10, 100, 60, "--color", "cc3333"], ["publish-swf", project, swf]):
            done = subprocess.run([str(CLI), *map(str, args)], capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)
        return swf.read_bytes()

    def test_ruffle_in_the_page_loads_a_movie_we_published(self):
        said = self.result(self.published())
        self.assertTrue(said.startswith("ok "), said)
        meta = json.loads(said[3:])
        self.assertEqual((meta["version"], meta["frames"]), (10, 1))
        self.assertEqual((meta["width"], meta["height"]), (1920, 1080))

    def test_a_file_that_is_not_a_movie_is_not_reported_as_one(self):
        # The check above can fail: Ruffle never reports a movie for bytes that are not one.
        said = self.result(b"this is not a SWF")
        self.assertTrue(said.startswith("FAIL"), said)


if __name__ == "__main__":
    unittest.main()
