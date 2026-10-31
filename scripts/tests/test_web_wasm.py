"""The browser build's `.flare` bridge, end to end: this repository's Rust compiled to WebAssembly,
run under node with SQLite and zstd as WebAssembly (apps/flare-web/tests/flare_file.rs), reading a
project `flare-cli` made and writing one back that the desktop's own code then reads.

Needs node, `npm ci` in apps/flare-web, the wasm32-unknown-unknown target, `wasm-bindgen-test-runner`
and a built flare-cli; skipped without them.
Run with `python3 -m unittest discover -s scripts/tests -p 'test_web_wasm.py'`.
"""
import functools
import http.server
import os
import shutil
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WEB = ROOT / "apps/flare-web"
CLI = Path(os.environ.get("FLARE_CLI", ROOT / "target/debug/flare-cli"))


def have_wasm_target():
    done = subprocess.run(["rustup", "target", "list", "--installed"], capture_output=True, text=True)
    return "wasm32-unknown-unknown" in done.stdout


READY = (
    shutil.which("node") is not None
    and shutil.which("wasm-bindgen-test-runner") is not None
    and (WEB / "node_modules/@sqlite.org/sqlite-wasm").is_dir()
    and CLI.is_file()
    and have_wasm_target()
)


@unittest.skipUnless(READY, "needs node, npm ci, wasm32 target, wasm-bindgen-test-runner, flare-cli")
class WebWasm(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.dir = Path(self.directory.name)

    def cli(self, *args):
        done = subprocess.run([str(CLI), *map(str, args)], capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)
        return done.stdout

    def test_the_page_reads_what_the_desktop_wrote_and_the_desktop_reads_what_the_page_wrote(self):
        native = self.dir / "native.flare"
        self.cli("new", native)
        self.cli("add-layer", native, "Second")
        self.cli("draw-rect", native, "--rect", 10, 10, 120, 80, "--color", "cc3333")
        self.cli("add-symbol", native, "Art")
        written = self.dir / "from-wasm.flare"
        env = dict(
            os.environ,
            CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="wasm-bindgen-test-runner",
            FLARE_WEB_DIR=str(WEB),
            FLARE_WEB_FIXTURE=str(native),
            FLARE_WEB_OUT=str(written),
        )
        done = subprocess.run(
            ["cargo", "test", "-p", "flare-web", "--target", "wasm32-unknown-unknown", "--test", "flare_file"],
            cwd=ROOT, env=env, capture_output=True, text=True,
        )
        self.assertEqual(done.returncode, 0, done.stdout[-3000:] + done.stderr[-3000:])
        self.assertIn("2 passed", done.stdout)
        self.assertTrue(written.is_file())
        # The desktop's own code finds the same project in what the page wrote, and draws it alike.
        self.assertEqual(self.cli("info", written), self.cli("info", native))
        a, b = self.dir / "a.png", self.dir / "b.png"
        self.cli("export", native, a)
        self.cli("export", written, b)
        self.assertEqual(a.read_bytes(), b.read_bytes())


@unittest.skipUnless(shutil.which("node") and (WEB / "node_modules/@sqlite.org/sqlite-wasm").is_dir(),
                     "needs node and `npm ci` in apps/flare-web")
class Vendoring(unittest.TestCase):
    def test_the_libraries_are_copied_with_specifiers_a_browser_can_load_and_zstd_still_works(self):
        with tempfile.TemporaryDirectory() as out:
            done = subprocess.run(["node", "scripts/vendor.mjs", out], cwd=WEB, capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            out = Path(out)
            for name in ["sqlite/index.mjs", "sqlite/sqlite3.wasm", "zstd/index.web.js", "zstd/module.js",
                         "zstd/zstd.wasm", "LICENSES.txt"]:
                self.assertTrue((out / name).is_file(), name)
            # No relative import is left without its extension.
            import re
            for js in (out / "zstd").rglob("*.js"):
                for spec in re.findall(r"(?:from|import)\s*\(?\s*['\"](\.{1,2}/[^'\"]+)['\"]", js.read_text()):
                    self.assertRegex(spec, r"\.(js|mjs|wasm)$", f"{js.name}: {spec}")
            notice = (out / "LICENSES.txt").read_text()
            self.assertIn("Apache-2.0", notice)
            self.assertIn("MIT", notice)
            # The vendored files, served over HTTP like a page's, load and work: both wasm files are
            # fetched by URL, as a browser does, and a .flare the desktop wrote is read with them.
            native = out / "native.flare"
            done = subprocess.run([str(CLI), "new", str(native)], capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)
            class Quiet(http.server.SimpleHTTPRequestHandler):
                def log_message(self, *args):
                    pass

            handler = functools.partial(Quiet, directory=str(out))
            server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.server_close)
            self.addCleanup(server.shutdown)
            base = f"http://127.0.0.1:{server.server_port}"
            probe = f"""
              const here = {str(out)!r};
              const url = (p) => 'file://' + here + '/' + p;
              const {{ default: sqlite3InitModule }} = await import(url('sqlite/index.mjs'));
              const zstd = await import(url('zstd/index.web.js'));
              const sqlite3 = await sqlite3InitModule({{
                locateFile: (name) => '{base}/sqlite/' + name, print: () => {{}}, printErr: () => {{}},
              }});
              await zstd.init('{base}/zstd/zstd.wasm');
              const raw = Uint8Array.from({{ length: 5000 }}, (_, i) => i % 7);
              const back = zstd.decompress(zstd.compress(raw, 3));
              if (back.length !== raw.length || back.some((v, i) => v !== raw[i])) process.exit(3);
              const {{ createFlareFile }} = await import('file://{WEB}/src/lib/flare-file.js');
              const file = createFlareFile({{ sqlite3, zstd }});
              const parts = file.read(new Uint8Array(await (await fetch('{base}/native.flare')).arrayBuffer()));
              if (!parts.records.some((r) => r.kind === 'scene')) process.exit(4);
              console.log('vendored ok');
            """
            run = subprocess.run(["node", "--input-type=module", "-e", probe], capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            self.assertIn("vendored ok", run.stdout)


if __name__ == "__main__":
    unittest.main()
