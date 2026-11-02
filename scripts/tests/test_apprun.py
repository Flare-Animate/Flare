"""The AppImage's entry point starts the program whatever happens to the desktop integration, and does
the integration right when it can."""
import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
ID = "io.github.Flare_Animate.Flare"


class AppRun(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.base = Path(directory.name)
        self.appdir = self.base / "app dir"  # a space, as a real path may have
        (self.appdir / "usr/bin").mkdir(parents=True)
        (self.appdir / "usr/share/applications").mkdir(parents=True)
        (self.appdir / "usr/share/icons/hicolor/256x256/apps").mkdir(parents=True)
        program = self.appdir / "usr/bin/flare-desktop"
        program.write_text('#!/bin/sh\nprintf "%s|" "$@" > "$RAN"\n')
        program.chmod(program.stat().st_mode | stat.S_IEXEC)
        (self.appdir / "usr/share/applications" / f"{ID}.desktop").write_text(
            f"[Desktop Entry]\nName=Flare\nExec=flare-desktop %F\nIcon={ID}\n")
        (self.appdir / f"usr/share/icons/hicolor/256x256/apps/{ID}.png").write_bytes(b"icon")
        run = self.appdir / "AppRun"
        run.write_text((ROOT / "packaging/AppRun").read_text())
        run.chmod(run.stat().st_mode | stat.S_IEXEC)
        self.home = self.base / "home"
        self.data = self.base / "data"
        self.home.mkdir()
        self.image = self.base / "Flare.AppImage"
        self.image.write_bytes(b"image")
        self.ran = self.base / "ran"

    def launch(self, *arguments, **env):
        environment = {"PATH": os.environ["PATH"], "HOME": str(self.home),
                       "XDG_DATA_HOME": str(self.data), "RAN": str(self.ran)}
        environment.update({k: v for k, v in env.items() if v is not None})
        for key in [k for k, v in env.items() if v is None]:
            environment.pop(key, None)
        return subprocess.run([str(self.appdir / "AppRun"), *arguments], env=environment,
                              capture_output=True, text=True)

    def entry(self):
        return self.data / "applications" / f"{ID}.desktop"

    def test_it_starts_the_program_with_the_arguments_it_was_given(self):
        result = self.launch("a b", "c.flare")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.ran.read_text(), "a b|c.flare|")

    def test_it_makes_a_menu_entry_for_the_appimage_and_installs_the_icon(self):
        self.launch(APPIMAGE=str(self.image))
        text = self.entry().read_text()
        self.assertIn(f'Exec="{self.image.resolve()}" %F', text)
        self.assertIn("Name=Flare", text)
        self.assertEqual((self.data / f"icons/hicolor/256x256/apps/{ID}.png").read_bytes(), b"icon")
        self.assertEqual(oct(self.entry().stat().st_mode & 0o777), "0o644")

    def test_a_second_launch_leaves_the_entry_as_it_was_and_a_moved_appimage_updates_it(self):
        self.launch(APPIMAGE=str(self.image))
        before = self.entry().stat().st_ino, self.entry().stat().st_mtime_ns
        self.launch(APPIMAGE=str(self.image))
        self.assertEqual((self.entry().stat().st_ino, self.entry().stat().st_mtime_ns), before)
        moved = self.base / "moved.AppImage"
        self.image.rename(moved)
        self.launch(APPIMAGE=str(moved))
        self.assertIn(f'Exec="{moved.resolve()}" %F', self.entry().read_text())
        leftovers = [p.name for p in (self.data / "applications").iterdir() if p.name != f"{ID}.desktop"]
        self.assertEqual(leftovers, [], "no temporary files are left behind")

    def test_it_can_be_turned_off_and_does_nothing_without_an_appimage_file(self):
        for env in ({"APPIMAGE": str(self.image), "FLARE_NO_DESKTOP_INTEGRATION": "1"},
                    {"APPIMAGE": None},
                    {"APPIMAGE": str(self.base / "gone.AppImage")}):
            result = self.launch(**env)
            self.assertEqual(result.returncode, 0)
            self.assertTrue(self.ran.exists(), "the program still starts")
            self.ran.unlink()
            self.assertFalse(self.data.exists(), env)

    def test_a_path_that_would_need_escaping_in_an_exec_line_gets_no_entry(self):
        for name in ('has"quote.AppImage', "has$dollar.AppImage", "has&amp.AppImage",
                     "has|bar.AppImage", "has%percent.AppImage", "has`tick.AppImage"):
            image = self.base / name
            image.write_bytes(b"image")
            result = self.launch(APPIMAGE=str(image))
            self.assertEqual(result.returncode, 0, name)
            self.assertFalse(self.entry().exists(), name)
            self.ran.unlink()

    def test_another_programs_entry_for_the_same_file_is_kept_and_ours_is_not_added(self):
        (self.data / "applications").mkdir(parents=True)
        theirs = self.data / "applications" / "appimagelauncher-flare.desktop"
        theirs.write_text(f'[Desktop Entry]\nName=Flare\nExec="{self.image.resolve()}" %U\n')
        self.launch(APPIMAGE=str(self.image))
        self.assertFalse(self.entry().exists())
        self.assertTrue(theirs.exists())
        # And a leftover entry of ours is removed rather than shown twice.
        self.entry().write_text("old")
        self.launch(APPIMAGE=str(self.image))
        self.assertFalse(self.entry().exists())

    def test_a_data_folder_it_cannot_write_to_does_not_stop_the_start(self):
        blocked = self.base / "blocked"
        blocked.write_text("a file, not a folder")
        result = self.launch(APPIMAGE=str(self.image), XDG_DATA_HOME=str(blocked))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.ran.exists())

    def test_it_sets_no_library_path_so_the_systems_graphics_libraries_are_used(self):
        text = (ROOT / "packaging/AppRun").read_text()
        code = [line for line in text.splitlines() if not line.lstrip().startswith("#")]
        self.assertFalse(any("LD_LIBRARY_PATH" in line for line in code))


if __name__ == "__main__":
    unittest.main()
