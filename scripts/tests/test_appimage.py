"""The AppImage is built from pinned tools and holds what a desktop looks for, and none of the
system's libraries."""
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import appimage  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]


class Pins(unittest.TestCase):
    def test_every_architecture_pins_both_tools_by_a_sha256_digest(self):
        for arch, pins in appimage.PINS.items():
            for name in ("tool", "runtime"):
                self.assertRegex(pins[name], r"^[0-9a-f]{64}$", f"{arch} {name}")

    def test_a_target_is_named_the_way_the_tools_name_it_and_others_are_refused(self):
        self.assertEqual(appimage.architecture("x86_64-unknown-linux-gnu"), "x86_64")
        self.assertEqual(appimage.architecture("aarch64-unknown-linux-gnu"), "aarch64")
        with self.assertRaises(ValueError):
            appimage.architecture("riscv64gc-unknown-linux-gnu")


class Download(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.base = Path(directory.name)
        self.source = self.base / "source"
        self.source.write_bytes(b"a tool")
        self.digest = hashlib.sha256(b"a tool").hexdigest()

    def test_a_download_with_the_pinned_digest_is_kept(self):
        got = appimage.fetch_verified(self.source.as_uri(), self.base / "out/tool", self.digest)
        self.assertEqual(got.read_bytes(), b"a tool")

    def test_a_download_with_another_digest_is_refused_and_not_left_behind(self):
        with self.assertRaisesRegex(ValueError, "not the pinned"):
            appimage.fetch_verified(self.source.as_uri(), self.base / "out/tool", "0" * 64)
        self.assertEqual(list((self.base / "out").iterdir()), [])

    def test_a_file_already_there_with_the_digest_is_not_downloaded_again(self):
        target = self.base / "tool"
        target.write_bytes(b"a tool")
        got = appimage.fetch_verified("file:///no/such/place", target, self.digest)
        self.assertEqual(got, target)


class Directory(unittest.TestCase):
    def test_it_holds_the_program_and_what_a_desktop_looks_for_and_no_libraries(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "built"
            binary.write_bytes(b"program")
            appdir = appimage.build_appdir(Path(directory) / "Flare.AppDir", binary)
            names = {p.relative_to(appdir).as_posix() for p in appdir.rglob("*")}
            for wanted in ("AppRun", ".DirIcon", f"{appimage.APP_ID}.desktop", f"{appimage.APP_ID}.png",
                           "usr/bin/flare-desktop",
                           f"usr/share/applications/{appimage.APP_ID}.desktop",
                           f"usr/share/icons/hicolor/256x256/apps/{appimage.APP_ID}.png",
                           f"usr/share/metainfo/{appimage.APP_ID}.metainfo.xml",
                           "usr/share/mime/packages/flare.xml"):
                self.assertIn(wanted, names)
            self.assertFalse([n for n in names if "/lib" in n or n.endswith(".so") or ".so." in n])
            self.assertEqual((appdir / "usr/bin/flare-desktop").read_bytes(), b"program")
            self.assertTrue((appdir / "AppRun").stat().st_mode & 0o111)
            desktop = (appdir / f"{appimage.APP_ID}.desktop").read_text()
            self.assertIn(f"Icon={appimage.APP_ID}", desktop)
            self.assertIn("Exec=flare-desktop", desktop)
            metainfo = (appdir / f"usr/share/metainfo/{appimage.APP_ID}.metainfo.xml").read_text()
            self.assertIn(f"{appimage.APP_ID}.desktop", metainfo)

    def test_the_old_launcher_that_gathered_the_systems_libraries_is_gone(self):
        self.assertFalse((ROOT / "packaging/appimage-launcher.sh").exists())


if __name__ == "__main__":
    unittest.main()
