#!/usr/bin/env python3
"""Build a Flatpak bundle of the desktop app on a native Linux architecture.

The bundle holds the program the release already built (nothing is compiled inside the sandbox),
the desktop entry, the AppStream metadata, the MIME types and the icon, each named by the app ID
as Flatpak requires. It is a file to install with `flatpak install --user flare-<v>-linux-<arch>.flatpak`;
publishing it on Flathub is separate.
"""
import argparse
import json
import os
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


# The builder is the `org.flatpak.Builder` Flatpak, not the distribution's flatpak-builder: the
# one in Ubuntu 22.04 is 1.2.2, which runs `appstream-compose` and `eu-strip` inside the SDK, and
# the 24.08 SDK has neither. The Flatpak brings its own, and reaches the host's runtimes through
# a D-Bus session.
BUILDER = "org.flatpak.Builder"
REQUIRED_TOOLS = ("flatpak", "dbus-run-session")


def require_tools():
    missing = [tool for tool in REQUIRED_TOOLS if shutil.which(tool) is None]
    if missing:
        raise ValueError("Flatpak packaging needs " + ", ".join(missing) + " installed")
    installed = subprocess.run(["flatpak", "info", "--user", BUILDER],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    if not installed:
        raise ValueError(f"Flatpak packaging needs {BUILDER}: flatpak install --user flathub {BUILDER}")


def builder_command(arch, repo, build, manifest_file):
    return ["dbus-run-session", "--", "flatpak", "run", BUILDER, "--user", "--force-clean",
            "--disable-rofiles-fuse", "--disable-cache", f"--arch={arch}", f"--repo={repo}",
            str(build), str(manifest_file)]


def package(version, target, binary, output):
    release.validate_package_input(version, target, binary)
    require_tools()
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported Linux target: {target}")
    arch = ARCHITECTURES[target]
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"flare-{version}-{release.TARGETS[target]}.flatpak"
    # Not the system temporary folder: the builder is a Flatpak, and its sandbox has a /tmp of its
    # own, so it cannot see the host's. The output folder is in the workspace, which it can.
    with tempfile.TemporaryDirectory(prefix=".flare-flatpak-", dir=output) as directory:
        work = Path(directory)
        stage = work / "stage"
        write_stage(stage, binary.resolve())
        manifest_file = work / f"{APP_ID}.json"
        manifest_file.write_text(json.dumps(manifest(stage), indent=2), encoding="utf-8")
        repo, build = work / "repo", work / "build"
        subprocess.run(builder_command(arch, repo, build, manifest_file), check=True,
                       env=dict(os.environ, GSETTINGS_BACKEND="memory"))
        # The runtime repository lets `flatpak install` of the bundle fetch the runtime.
        subprocess.run(["flatpak", "build-bundle", f"--arch={arch}",
                        "--runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo",
                        str(repo), str(archive), APP_ID], check=True)
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
