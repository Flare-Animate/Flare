"""What the Windows installer is told to do, read from its source, since WiX is not here to build it."""
from pathlib import Path
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
WIX = "{http://wixtoolset.org/schemas/v4/wxs}"


class WindowsInstaller(unittest.TestCase):
    def setUp(self):
        self.tree = ET.parse(ROOT / "packaging/windows.wxs").getroot()

    def test_it_installs_for_the_current_user_so_it_asks_for_no_administrator(self):
        package = self.tree.find(f"{WIX}Package")
        self.assertEqual(package.get("Scope"), "perUser")

    def test_every_registration_goes_to_the_users_own_hive_in_a_per_user_install(self):
        roots = {value.get("Root") for value in self.tree.iter(f"{WIX}RegistryValue")}
        self.assertEqual(roots, {"HKMU"}, "HKLM needs an administrator; HKMU is HKCU here")

    def test_it_installs_flare_desktop_and_a_start_menu_shortcut_to_it(self):
        files = {f.get("Name"): f for f in self.tree.iter(f"{WIX}File")}
        program = files["flare-desktop.exe"]
        shortcut = program.find(f"{WIX}Shortcut")
        self.assertIsNotNone(shortcut)
        self.assertEqual(shortcut.get("Directory"), "ProgramMenuFolder")

    def test_the_file_types_are_registered_with_the_program_that_installed_them(self):
        text = (ROOT / "packaging/windows.wxs").read_text()
        for extension in (".flare", ".fla", ".xfl", ".swf"):
            self.assertIn(f"Software\\Classes\\{extension}", text)
        self.assertIn("[#FlareExe]", text)


if __name__ == "__main__":
    unittest.main()
