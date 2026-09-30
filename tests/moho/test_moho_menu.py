"""Static checks that the Moho import command is actually reachable.

A menu command that compiles but is never added to a menu is invisible, and no
compiler will say so. These assertions cover the wiring, which is the part
that silently breaks when someone reorganises the app's sources.
"""
import io
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

checks = []


def chk(name, ok, detail=""):
    checks.append((name, ok, detail))


def read(rel):
    with io.open(os.path.join(ROOT, rel), encoding="utf-8", errors="replace") as f:
        return f.read()


def test_id_defined_in_every_command_header():
    """Both the current and the legacy app have their own command-id header."""
    for h in ("flare/sources/flare/menubarcommandids.h",
              "flare/sources/flare_legacy/menubarcommandids.h"):
        s = read(h)
        chk(f"{os.path.basename(os.path.dirname(h))}: command id defined",
            '#define MI_ImportMohoProject "MI_ImportMohoProject"' in s)


def test_action_created_where_flash_action_is():
    """Each room layout builds its own File menu, so the action has to be
    created in each one. Creating it in only some of them leaves the item
    missing from the other layouts."""
    for mw in ("flare/sources/flare/mainwindow.cpp",
               "flare/sources/flare_legacy/mainwindow.cpp"):
        s = read(mw)
        n_flash = len(re.findall(r"createMenuFileAction\(MI_ImportFlashVector", s))
        n_moho = len(re.findall(r"createMenuFileAction\(MI_ImportMohoProject", s))
        chk(f"{os.path.basename(os.path.dirname(mw))}: action created once per layout",
            n_moho == n_flash and n_moho > 0, f"flash={n_flash} moho={n_moho}")


def test_added_to_the_import_menu():
    chk("menubar: added to the Import menu",
        "addMenuItem(importMenu, MI_ImportMohoProject);" in read(
            "flare/sources/flare/menubar.cpp"))


def test_command_self_registers():
    """A file-scope instance is what registers a command with the command
    manager; a class that is merely declared is dead code."""
    s = read("flare/sources/flare/mohoimport.cpp")
    chk("command derives from MenuItemHandler",
        "class ImportMohoProjectCommand final : public MenuItemHandler" in s)
    chk("command binds its id", "MenuItemHandler(MI_ImportMohoProject)" in s)
    chk("command instance exists at file scope",
        "} g_importMohoProjectCommand;" in s)


def test_sources_are_in_the_build():
    cml = read("flare/sources/flare_legacy/CMakeLists.txt")
    chk("mohoimport.cpp compiled", "mohoimport.cpp" in cml)
    chk("reader include dir present", "common/moho" in cml)
    tnz = read("flare/sources/tnzcore/CMakeLists.txt")
    chk("MohoReader.cpp compiled into tnzcore",
        "../common/moho/MohoReader.cpp" in tnz)
    chk("MohoReader.h listed in tnzcore headers",
        "../common/moho/MohoReader.h" in tnz)
    chk("tnzcore defines the export guard the reader headers use",
        "TFLASH_EXPORTS" in tnz)


def test_popup_offers_only_extensions_the_reader_handles():
    """Every extension in the file dialog must be one the reader can
    classify, or the dialog offers a file it will then reject."""
    s = read("flare/sources/flare/mohoimport.cpp")
    exts = set(re.findall(r'"(moho\w*|anime\w*|anme)"', s))
    known = {"moho", "mohoproj", "anime", "animeproj", "anme"}
    chk("popup extensions are all recognised", exts and exts <= known,
        ", ".join(sorted(exts)))
    chk("all five container extensions offered", len(exts) == 5,
        f"{len(exts)} of 5")


def main():
    for fn in sorted(
            (v for k, v in globals().items() if k.startswith("test_")),
            key=lambda f: f.__code__.co_firstlineno):
        fn()
    fails = 0
    for name, ok, detail in checks:
        print(f"   [{'ok  ' if ok else 'FAIL'}] {name}"
              + (f"  ({detail})" if detail else ""))
        if not ok:
            fails += 1
    print(f"\n{'FAILED' if fails else 'PASSED'}: {len(checks)} checks, "
          f"{fails} failure(s)")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
