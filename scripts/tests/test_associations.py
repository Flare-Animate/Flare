"""The file types Flare registers with each operating system are the ones the app opens.

The list lives in `crates/presentation/flare-animate/src/workspace/open_any.rs`; the macOS
bundle, the Linux desktop entry and MIME types and the Windows installer must name exactly
those extensions, or a file would be offered to Flare that Flare then refuses.
"""

import plistlib
import re
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OPEN_ANY = ROOT / "crates/presentation/flare-animate/src/workspace/open_any.rs"


def rust_extensions(name: str) -> set[str]:
    text = OPEN_ANY.read_text()
    match = re.search(rf"pub const {name}: \[&str; \d+\] = \[([^\]]*)\]", text)
    assert match, name
    return set(re.findall(r'"([^"]+)"', match.group(1)))


PROJECT = rust_extensions("PROJECT_EXTENSIONS")
IMPORT = rust_extensions("IMPORT_EXTENSIONS")


class Associations(unittest.TestCase):
    def test_the_app_knows_the_formats_this_file_checks(self):
        self.assertEqual(PROJECT, {"flare"})
        self.assertEqual(IMPORT, {"fla", "xfl"})

    def test_the_macos_bundle_declares_each_format(self):
        template = (ROOT / "apps/flare-desktop/resources/macos/Info.plist.in").read_text()
        info = plistlib.loads(template.replace("@VERSION@", "1.0.0").encode())
        declared = {
            ext
            for kind in info["UTExportedTypeDeclarations"]
            for ext in kind["UTTypeTagSpecification"]["public.filename-extension"]
        }
        by_extension = {
            ext
            for kind in info["CFBundleDocumentTypes"]
            for ext in kind.get("CFBundleTypeExtensions", [])
        }
        self.assertEqual(declared, PROJECT)
        self.assertEqual(by_extension, IMPORT)
        for kind in info["CFBundleDocumentTypes"]:
            if "CFBundleTypeExtensions" in kind:
                self.assertEqual(kind["LSHandlerRank"], "Alternate", "Flare is not their default")

    def test_the_linux_mime_types_and_desktop_entry_cover_each_format(self):
        namespace = "{http://www.freedesktop.org/standards/shared-mime-info}"
        tree = ET.parse(ROOT / "packaging/flare-mime.xml")
        globs = {
            g.get("pattern").removeprefix("*.")
            for g in tree.iter(f"{namespace}glob")
        }
        self.assertEqual(globs, PROJECT | IMPORT)
        types = {t.get("type") for t in tree.iter(f"{namespace}mime-type")}
        desktop = (ROOT / "packaging/flare.desktop").read_text()
        listed = set(re.search(r"^MimeType=(.*)$", desktop, re.M).group(1).strip(";").split(";"))
        self.assertEqual(listed, types)
        self.assertIn("%F", desktop, "several files can be opened at once")
        metainfo = (ROOT / "packaging/flare.metainfo.xml").read_text()
        for mime in types:
            self.assertIn(f"<mediatype>{mime}</mediatype>", metainfo)

    def test_the_windows_installer_registers_each_format(self):
        wxs = (ROOT / "packaging/windows.wxs").read_text()
        for extension in PROJECT | IMPORT:
            self.assertIn(f'Software\\Classes\\.{extension}\\OpenWithProgids', wxs, extension)
            self.assertIn(f'Name=".{extension}"', wxs, extension)
        # Animate's formats are only offered; the default is never taken.
        for extension in IMPORT:
            self.assertNotRegex(
                wxs, rf'Key="Software\\Classes\\\.{extension}"\s'
            )


if __name__ == "__main__":
    unittest.main()
