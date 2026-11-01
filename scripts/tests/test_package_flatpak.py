"""The Flatpak manifest and the files it installs are named as Flatpak requires."""
import sys
import tempfile
import unittest
from unittest import mock
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

    def test_the_app_id_is_a_valid_name_to_flatpak_and_the_other_linux_packages_use_it(self):
        # flatpak-builder refuses a hyphen anywhere but the last segment; the first release
        # build failed on io.github.Flare-Animate.Flare.
        segments = flatpak.APP_ID.split(".")
        self.assertGreaterEqual(len(segments), 3)
        for segment in segments[:-1]:
            self.assertRegex(segment, r"^[A-Za-z_][A-Za-z0-9_]*$")
        root = Path(__file__).resolve().parents[2]
        for path in ("packaging/flare.metainfo.xml", "packaging/cargo-packager.json",
                     "scripts/packaging/package_linux.py", "apps/flare-desktop/Cargo.toml"):
            self.assertIn(flatpak.APP_ID, (root / path).read_text(), path)

    def test_a_missing_build_tool_is_named_before_anything_is_built(self):
        found = {"flatpak": "/usr/bin/flatpak"}
        with mock.patch.object(flatpak.shutil, "which", side_effect=found.get):
            with self.assertRaisesRegex(ValueError, "dbus-run-session"):
                flatpak.require_tools()
        with mock.patch.object(flatpak.shutil, "which", return_value="/usr/bin/tool"):
            with mock.patch.object(flatpak.subprocess, "run",
                                   return_value=mock.Mock(returncode=1)):
                with self.assertRaisesRegex(ValueError, "org.flatpak.Builder"):
                    flatpak.require_tools()
            with mock.patch.object(flatpak.subprocess, "run",
                                   return_value=mock.Mock(returncode=0)):
                flatpak.require_tools()

    def test_the_builder_runs_as_a_flatpak_inside_a_d_bus_session_for_the_target_architecture(self):
        command = flatpak.builder_command("aarch64", "/r", "/b", "/m.json")
        self.assertEqual(command[:5], ["dbus-run-session", "--", "flatpak", "run", "org.flatpak.Builder"])
        self.assertIn("--arch=aarch64", command)
        self.assertEqual(command[-2:], ["/b", "/m.json"])

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
