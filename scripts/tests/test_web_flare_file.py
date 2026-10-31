"""The page's .flare reader and writer (apps/flare-web/src/lib/flare-file.js) against files the
desktop code makes, run under Node with the same two WebAssembly libraries the page loads.

Needs `node`, `npm ci` in apps/flare-web, and `cargo build -p flare-cli`; skipped without them.
Run with `python3 -m unittest discover -s scripts/tests -p 'test_web_flare_file.py'`.
"""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WEB = ROOT / "apps/flare-web"
CLI = Path(os.environ.get("FLARE_CLI", ROOT / "target/debug/flare"))
READY = (
    shutil.which("node") is not None
    and (WEB / "node_modules/@sqlite.org/sqlite-wasm").is_dir()
    and (WEB / "node_modules/@bokuweb/zstd-wasm").is_dir()
    and CLI.is_file()
)


@unittest.skipUnless(READY, "needs node, `npm ci` in apps/flare-web and a built flare-cli")
class PageFlareFile(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.dir = Path(self.directory.name)

    def cli(self, *args):
        done = subprocess.run([str(CLI), *map(str, args)], capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)
        return done.stdout

    def project(self):
        path = self.dir / "native.flare"
        self.cli("new", path)
        self.cli("add-layer", path, "Second")
        self.cli("draw-rect", path, "--rect", 10, 10, 120, 80, "--color", "cc3333")
        self.cli("add-symbol", path, "Art")
        return path

    def node(self, script, *args):
        return subprocess.run(["node", script, *map(str, args)], cwd=WEB, capture_output=True, text=True)

    def test_a_file_the_desktop_wrote_is_read_and_what_the_page_writes_opens_on_the_desktop(self):
        native = self.project()
        done = self.node("scripts/test_flare_file.mjs", native, self.dir)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn("ok:", done.stdout)
        rewritten = self.dir / "rewritten.flare"
        self.assertTrue(rewritten.is_file())
        # The desktop's own code reads what the page wrote, and finds the same project.
        self.assertEqual(self.cli("info", rewritten), self.cli("info", native))
        # And the picture it draws is the same.
        a, b = self.dir / "a.png", self.dir / "b.png"
        self.cli("export", native, a)
        self.cli("export", rewritten, b)
        self.assertEqual(a.read_bytes(), b.read_bytes())


if __name__ == "__main__":
    unittest.main()
