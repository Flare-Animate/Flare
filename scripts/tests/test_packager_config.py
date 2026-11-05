"""What the Linux packages are told to carry."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class PackagerConfig(unittest.TestCase):
    def test_the_appimage_is_not_told_to_bundle_the_systems_graphics_libraries(self):
        # A tester on a newer system got "Failed to create surface for any enabled backend: {}"
        # from the AppImage, while the tarball, which uses the system's libraries, ran: the
        # AppImage's older copies of Wayland, xcb, X11 and xkbcommon came first on the library
        # path and the system's graphics driver could not load against them.
        config = json.loads((ROOT / "packaging/cargo-packager.json").read_text())
        libs = config.get("appimage", {}).get("libs", [])
        graphics = ("wayland", "xcb", "X11", "Xcursor", "Xi", "Xrandr", "xkbcommon", "EGL", "GL", "vulkan")
        bundled = [lib for lib in libs if any(name in lib for name in graphics)]
        self.assertEqual(bundled, [], "the AppImage must use the system's copies")

    def test_the_release_workflow_fails_an_appimage_that_carries_them_anyway(self):
        workflow = (ROOT / ".github/workflows/platforms.yml").read_text()
        self.assertIn("bundles libraries of the system's graphics stack", workflow)
        for name in ("libwayland", "libxcb", "libX11", "libxkbcommon", "libffi"):
            self.assertIn(f"-name '{name}*'", workflow)

    def test_the_release_workflow_starts_the_packed_programs_and_asks_their_version(self):
        workflow = (ROOT / ".github/workflows/platforms.yml").read_text()
        self.assertIn("squashfs-root/AppRun --version", workflow)
        self.assertIn("--command=flare-desktop io.github.Flare_Animate.Flare --version", workflow)


if __name__ == "__main__":
    unittest.main()
