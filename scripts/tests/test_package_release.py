import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import package_release as release  # noqa: E402


class ReleaseContract(unittest.TestCase):
    def test_the_workspace_version_is_valid(self):
        self.assertRegex(release.workspace_version(), r"^\d+\.\d+\.\d+")

    def test_a_tag_must_match_the_version(self):
        with self.assertRaises(ValueError):
            release.validate("v9.9.9")

    def test_release_notes_need_a_written_summary(self):
        version = release.workspace_version()
        self.assertIn("CHANGELOG.md", release.release_notes(version))
        with self.assertRaises(ValueError):
            release.release_notes("9.9.9")

    def test_every_target_has_its_installers(self):
        assets = release.expected_assets("1.2.3")
        for name in (
            "flare-1.2.3-macos-arm64.dmg",
            "flare-1.2.3-windows-x64.msi",
            "flare-1.2.3-linux-x64.deb",
            "flare-1.2.3-linux-arm64.AppImage",
            "flare-cli-1.2.3-linux-x64.tar.gz",
            "flare-cli-1.2.3-windows-arm64.zip",
        ):
            self.assertIn(name, assets)

    def test_a_binary_for_the_wrong_platform_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            fake = Path(directory) / "flare"
            fake.write_bytes(b"not an executable" * 8)
            with self.assertRaises(ValueError):
                release.validate_binary("x86_64-unknown-linux-gnu", fake)
            with self.assertRaises(ValueError):
                release.validate_binary("aarch64-apple-darwin", fake)

    def test_a_release_with_a_missing_asset_does_not_verify(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(ValueError):
                release.verify_assets(release.workspace_version(), Path(directory))

    def test_the_manifest_is_not_a_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "elsewhere"
            target.write_bytes(b"x")
            (Path(directory) / "asset").write_bytes(b"data")
            link = Path(directory) / "asset.sha256"
            link.symlink_to(target)
            with self.assertRaises(ValueError):
                release.write_checksum(Path(directory) / "asset")


if __name__ == "__main__":
    unittest.main()
