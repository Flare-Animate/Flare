"""Builds the AppImage: a plain directory of the program and its desktop files, packed with a pinned
appimagetool and a pinned AppImage runtime (static, so no FUSE library is needed to run it).

It carries none of the system's libraries. A tool that gathers a program's libraries into an AppImage
also gathered the Wayland, xcb, X11 and xkbcommon ones of the build machine, which are older than the
ones a newer system's graphics driver is built against, and the window then failed to open there.
"""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import urllib.request

import package_release as release

APP_ID = "io.github.Flare_Animate.Flare"
TOOL_VERSION = "1.9.1"
RUNTIME_VERSION = "20251108"
# Digests of the release files, from https://github.com/AppImage/appimagetool/releases/tag/1.9.1 and
# https://github.com/AppImage/type2-runtime/releases/tag/20251108, checked against the downloads.
PINS = {
    "x86_64": {
        "tool": "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0",
        "runtime": "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d",
    },
    "aarch64": {
        "tool": "f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158",
        "runtime": "00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444",
    },
}
TOOL_URL = "https://github.com/AppImage/appimagetool/releases/download/{version}/appimagetool-{arch}.AppImage"
RUNTIME_URL = "https://github.com/AppImage/type2-runtime/releases/download/{version}/runtime-{arch}"
NOTICES = ("FONT_LICENSES.txt", "MODEL_LICENSES.txt")


def architecture(target):
    """The AppImage name of a Rust target: `x86_64` or `aarch64`."""
    arch = target.split("-")[0]
    if arch not in PINS:
        raise ValueError(f"Unsupported AppImage architecture: {target}")
    return arch


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def fetch_verified(url, destination, expected):
    """Downloads `url` to `destination` unless a file with the digest `expected` is already there.
    A download with another digest is deleted and refused: a tool that is run is not trusted on
    the strength of where it came from."""
    destination = Path(destination)
    if destination.exists() and sha256(destination) == expected:
        return destination
    destination.parent.mkdir(parents=True, exist_ok=True)
    partial = destination.with_name(destination.name + ".part")
    with urllib.request.urlopen(url) as response, open(partial, "wb") as out:
        shutil.copyfileobj(response, out)
    found = sha256(partial)
    if found != expected:
        partial.unlink()
        raise ValueError(f"{url} has the digest {found}, not the pinned {expected}")
    partial.replace(destination)
    return destination


def build_appdir(appdir, binary, root=release.ROOT):
    """The files the AppImage holds: the program as `usr/bin/flare-desktop` (the command line owns
    the name `flare`), what a desktop looks for under `usr/share`, and the top-level files the
    format asks for."""
    appdir = Path(appdir)
    (appdir / "usr/bin").mkdir(parents=True)
    shutil.copy2(binary, appdir / "usr/bin/flare-desktop")
    (appdir / "usr/bin/flare-desktop").chmod(0o755)
    share = appdir / "usr/share"
    desktop = (root / "packaging/flare.desktop").read_text().replace("Icon=flare", f"Icon={APP_ID}")
    icon = root / "assets/flare-icon-256.png"
    (share / "applications").mkdir(parents=True)
    (share / "applications" / f"{APP_ID}.desktop").write_text(desktop, encoding="utf-8")
    (share / "icons/hicolor/256x256/apps").mkdir(parents=True)
    shutil.copy2(icon, share / f"icons/hicolor/256x256/apps/{APP_ID}.png")
    (share / "metainfo").mkdir()
    metainfo = (root / "packaging/flare.metainfo.xml").read_text()
    (share / f"metainfo/{APP_ID}.metainfo.xml").write_text(
        metainfo.replace("flare.desktop", f"{APP_ID}.desktop"), encoding="utf-8")
    (share / "mime/packages").mkdir(parents=True)
    shutil.copy2(root / "packaging/flare-mime.xml", share / "mime/packages/flare.xml")
    (share / "licenses/flare").mkdir(parents=True)
    shutil.copy2(root / "LICENSE", share / "licenses/flare/LICENSE")
    for name in NOTICES:
        shutil.copy2(root / "packaging" / name, share / "licenses/flare" / name)
    shutil.copy2(root / "packaging/AppRun", appdir / "AppRun")
    (appdir / "AppRun").chmod(0o755)
    (appdir / f"{APP_ID}.desktop").write_text(desktop, encoding="utf-8")
    shutil.copy2(icon, appdir / f"{APP_ID}.png")
    (appdir / ".DirIcon").symlink_to(f"{APP_ID}.png")
    return appdir


def build(version, target, binary, archive, work, tools=None):
    """Packs the AppImage to `archive`, using `work` for the directory it is made from."""
    arch = architecture(target)
    tools = Path(tools) if tools else release.ROOT / "target/appimage-tools"
    tool = fetch_verified(TOOL_URL.format(version=TOOL_VERSION, arch=arch),
                          tools / f"appimagetool-{TOOL_VERSION}-{arch}.AppImage", PINS[arch]["tool"])
    runtime = fetch_verified(RUNTIME_URL.format(version=RUNTIME_VERSION, arch=arch),
                             tools / f"type2-runtime-{RUNTIME_VERSION}-{arch}", PINS[arch]["runtime"])
    tool.chmod(0o755)
    appdir = build_appdir(Path(work) / "Flare.AppDir", binary)
    # The tool is itself an AppImage: it is run unpacked, which needs no FUSE (containers, CI).
    env = dict(os.environ, ARCH=arch, APPIMAGE_EXTRACT_AND_RUN="1")
    subprocess.run([str(tool), "--no-appstream", "--runtime-file", str(runtime),
                    str(appdir), str(archive)], check=True, env=env)
    archive.chmod(0o755)
