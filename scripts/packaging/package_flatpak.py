#!/usr/bin/env python3
"""Build a Flatpak bundle of the desktop app on a native Linux architecture.

The bundle holds the program the release already built (nothing is compiled inside the sandbox),
the desktop entry, the AppStream metadata, the MIME types and the icon, each named by the app ID
as Flatpak requires. It is a file to install with `flatpak install --user flare-<v>-linux-<arch>.flatpak`;
publishing it on Flathub is separate.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

import package_release as release

APP_ID = "io.github.Flare_Animate.Flare"
RUNTIME = "org.freedesktop.Platform"
SDK = "org.freedesktop.Sdk"
RUNTIME_VERSION = "24.08"
# Flatpak's names for the two architectures.
ARCHITECTURES = {"x86_64-unknown-linux-gnu": "x86_64", "aarch64-unknown-linux-gnu": "aarch64"}

# What the sandbox is let reach: a window (Wayland, falling back to X11), the GPU, sound, and the
# person's files, which projects are opened from and saved to. There is no network access.
FINISH_ARGS = [
    "--share=ipc",
    "--socket=wayland",
    "--socket=fallback-x11",
    "--device=dri",
    "--socket=pulseaudio",
    "--filesystem=home",
]


def write_stage(stage, binary):
    """Lays out the files the manifest installs, named as Flatpak wants them."""
    stage.mkdir(parents=True, exist_ok=True)
    program = stage / "flare-desktop"
    shutil.copy2(binary, program)
    program.chmod(0o755)
    root = release.ROOT
    (stage / f"{APP_ID}.desktop").write_text(
        (root / "packaging/flare.desktop").read_text().replace("Icon=flare", f"Icon={APP_ID}"),
        encoding="utf-8")
    (stage / f"{APP_ID}.metainfo.xml").write_text(
        (root / "packaging/flare.metainfo.xml").read_text().replace("flare.desktop", f"{APP_ID}.desktop"),
        encoding="utf-8")
    shutil.copy2(root / "packaging/flare-mime.xml", stage / f"{APP_ID}.xml")
    shutil.copy2(root / "assets/flare-icon-256.png", stage / f"{APP_ID}.png")
    shutil.copy2(root / "LICENSE", stage / "LICENSE")
    shutil.copy2(root / "packaging/FONT_LICENSES.txt", stage / "FONT_LICENSES.txt")


def manifest(stage):
    """The flatpak-builder manifest for the files in `stage`."""
    return {
        "app-id": APP_ID,
        "runtime": RUNTIME,
        "runtime-version": RUNTIME_VERSION,
        "sdk": SDK,
        "command": "flare-desktop",
        "finish-args": FINISH_ARGS,
        "modules": [{
            "name": "flare",
            "buildsystem": "simple",
            "build-commands": [
                "install -Dm755 flare-desktop /app/bin/flare-desktop",
                f"install -Dm644 {APP_ID}.desktop /app/share/applications/{APP_ID}.desktop",
                f"install -Dm644 {APP_ID}.metainfo.xml /app/share/metainfo/{APP_ID}.metainfo.xml",
                f"install -Dm644 {APP_ID}.xml /app/share/mime/packages/{APP_ID}.xml",
                f"install -Dm644 {APP_ID}.png /app/share/icons/hicolor/256x256/apps/{APP_ID}.png",
                "install -Dm644 LICENSE /app/share/licenses/flare/LICENSE",
                "install -Dm644 FONT_LICENSES.txt /app/share/licenses/flare/FONT_LICENSES.txt",
            ],
            "sources": [{"type": "dir", "path": str(stage)}],
        }],
    }


# What flatpak-builder runs. `eu-strip` is elfutils', which it calls to split the debug symbols
# out; without it the build fails late, after the sandbox has been made, with a bare "No such
# file or directory".
REQUIRED_TOOLS = ("flatpak", "flatpak-builder", "eu-strip")


def require_tools():
    missing = [tool for tool in REQUIRED_TOOLS if shutil.which(tool) is None]
    if missing:
        raise ValueError("Flatpak packaging needs " + ", ".join(missing)
                         + " installed (apt: flatpak flatpak-builder elfutils)")


def package(version, target, binary, output):
    release.validate_package_input(version, target, binary)
    require_tools()
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported Linux target: {target}")
    arch = ARCHITECTURES[target]
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"flare-{version}-{release.TARGETS[target]}.flatpak"
    with tempfile.TemporaryDirectory(prefix="flare-flatpak-") as directory:
        work = Path(directory)
        stage = work / "stage"
        write_stage(stage, binary.resolve())
        manifest_file = work / f"{APP_ID}.json"
        manifest_file.write_text(json.dumps(manifest(stage), indent=2), encoding="utf-8")
        repo, build = work / "repo", work / "build"
        subprocess.run(["flatpak-builder", "--user", "--force-clean", "--disable-rofiles-fuse",
                        "--disable-cache", f"--arch={arch}", f"--repo={repo}", str(build),
                        str(manifest_file)], check=True)
        subprocess.run(["flatpak", "build-bundle", f"--arch={arch}", str(repo), str(archive),
                        APP_ID], check=True)
    release.write_checksum(archive)
    print(archive)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=sorted(ARCHITECTURES), required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.binary, args.output)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Flatpak packaging failed: {error}\n")


if __name__ == "__main__":
    main()
