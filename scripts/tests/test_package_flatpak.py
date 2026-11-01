"""The Flatpak manifest and the files it installs are named as Flatpak requires."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import package_flatpak as flatpak  # noqa: E402


class Flatpak(unittest.TestCase):
    def staged(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        binary = Path(directory.name) / "flare-desktop-built"
        binary.write_bytes(b"program")
        stage = Path(directory.name) / "stage"
        flatpak.write_stage(stage, binary)
        return stage

    def test_exported_files_are_named_by_the_app_id(self):
        stage = self.staged()
        for name in (f"{flatpak.APP_ID}.desktop", f"{flatpak.APP_ID}.metainfo.xml",
                     f"{flatpak.APP_ID}.xml", f"{flatpak.APP_ID}.png", "flare-desktop"):
            self.assertTrue((stage / name).is_file(), name)
        desktop = (stage / f"{flatpak.APP_ID}.desktop").read_text()
        self.assertIn(f"Icon={flatpak.APP_ID}", desktop)
        self.assertIn("Exec=flare-desktop", desktop)
        metadata = (stage / f"{flatpak.APP_ID}.metainfo.xml").read_text()
        self.assertIn(f'<launchable type="desktop-id">{flatpak.APP_ID}.desktop</launchable>', metadata)

    def test_the_manifest_installs_what_the_stage_holds_and_runs_the_desktop_program(self):
        stage = self.staged()
        manifest = flatpak.manifest(stage)
        self.assertEqual(manifest["app-id"], flatpak.APP_ID)
        self.assertEqual(manifest["command"], "flare-desktop")
        commands = "\n".join(manifest["modules"][0]["build-commands"])
        for installed in sorted(p.name for p in stage.iterdir() if p.name.startswith(flatpak.APP_ID)):
            self.assertIn(installed, commands)
        self.assertEqual(manifest["modules"][0]["sources"][0]["path"], str(stage))

    def test_the_sandbox_has_a_window_sound_and_files_but_no_network(self):
        args = flatpak.FINISH_ARGS
        self.assertIn("--device=dri", args)
        self.assertIn("--socket=pulseaudio", args)
        self.assertTrue(any(a.startswith("--socket=wayland") for a in args))
        self.assertFalse(any(a.startswith("--share=network") for a in args))
        self.assertFalse(any("--filesystem=host" in a for a in args))

    def test_only_the_two_linux_targets_are_built(self):
        self.assertEqual(set(flatpak.ARCHITECTURES), {"x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"})


if __name__ == "__main__":
    unittest.main()
