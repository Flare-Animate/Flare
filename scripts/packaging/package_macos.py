#!/usr/bin/env python3
"""Build a macOS bundle; installation and registration are explicit options."""
import argparse
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]
TEMPLATE = ROOT / "apps/flare-desktop/resources/macos/Info.plist.in"
ICON = ROOT / "assets/flare-icon.icns"
BUNDLE_ID = "io.github.Flare-Animate.Flare"
TARGETS = ("aarch64-apple-darwin", "x86_64-apple-darwin")
LSREGISTER = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"


def workspace_version():
    return tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]


def metadata(version):
    numeric = version.split("-")[0].split("+")[0]
    return plistlib.loads(TEMPLATE.read_text().replace("@VERSION@", numeric).encode())


def populate_bundle(binary, destination, version):
    contents = Path(destination) / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    (contents / "Resources").mkdir()
    executable = contents / "MacOS/flare"
    shutil.copy2(binary, executable)
    executable.chmod(executable.stat().st_mode | 0o111)
    shutil.copy2(ICON, contents / "Resources" / ICON.name)
    shutil.copy2(ROOT / "LICENSE", contents / "Resources/LICENSE")
    shutil.copy2(ROOT / "packaging/FONT_LICENSES.txt", contents / "Resources/FONT_LICENSES.txt")
    with (contents / "Info.plist").open("wb") as stream:
        plistlib.dump(metadata(version), stream, sort_keys=False)
    (contents / "PkgInfo").write_bytes(b"APPL????")


def check_destination(destination, replace):
    if destination.is_symlink():
        raise ValueError(f"Refusing symlink destination: {destination}")
    if not destination.exists():
        return
    if not replace:
        raise ValueError(f"Already exists: {destination}; use --replace to update Flare")
    try:
        with (destination / "Contents/Info.plist").open("rb") as stream:
            identifier = plistlib.load(stream)["CFBundleIdentifier"]
    except (OSError, KeyError, ValueError, plistlib.InvalidFileException) as error:
        raise ValueError(f"Refusing to replace an unrecognized bundle: {destination}") from error
    if identifier != BUNDLE_ID:
        raise ValueError(f"Refusing to replace another application's bundle: {destination}")


def publish_bundle(destination, prepare, replace=False):
    """Prepare privately, then replace only a recognized Flare bundle."""
    destination = Path(destination).absolute()
    if destination.suffix != ".app":
        raise ValueError("Bundle destination must end in .app")
    check_destination(destination, replace)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".flare-package-", dir=destination.parent) as temporary:
        stage = Path(temporary) / "Flare.app"
        prepare(stage)
        check_destination(destination, replace)
        previous = Path(temporary) / "previous.app"
        had_previous = destination.exists()
        if had_previous:
            destination.rename(previous)
        try:
            stage.rename(destination)
        except OSError:
            if had_previous:
                previous.rename(destination)
            raise
    return destination


def built_binary(debug, target):
    command = ["cargo", "build", "--locked", "-p", "flare-desktop", "--message-format=json-render-diagnostics"]
    if target:
        command += ["--target", target]
    if not debug:
        command.append("--release")
    process = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
    sys.stderr.write(process.stderr)
    if process.returncode:
        raise subprocess.CalledProcessError(process.returncode, command)
    executable = None
    for line in process.stdout.splitlines():
        item = json.loads(line)
        if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "flare-desktop":
            executable = item.get("executable") or executable
    if executable is None:
        raise ValueError("Cargo did not report a Flare executable")
    return Path(executable)


def sign_bundle(bundle):
    """Ad-hoc signature, so the bundle runs locally; release signing needs a Developer ID."""
    subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", str(bundle)], check=True)
    subprocess.run(["/usr/bin/codesign", "--verify", "--deep", "--strict", str(bundle)], check=True)


def release_archives(binary, output, version, target, name):
    """The DMG and updater ZIP carry the exact same signed app bundle."""
    from package_release import write_checksum, write_update_archive
    if sys.platform != "darwin":
        raise ValueError("DMG packaging requires macOS")
    with tempfile.TemporaryDirectory(prefix="flare-dmg-") as directory:
        payload = Path(directory) / "volume"
        payload.mkdir()
        app = payload / "Flare.app"
        populate_bundle(binary, app, version)
        sign_bundle(app)
        archive = output / (name + ".dmg")
        build_disk_image(app, archive)
        write_checksum(archive)
        write_update_archive(app, output / (name + "-update.zip"))
    print(archive)


def build_disk_image(app, archive):
    try:
        import dmgbuild
    except ImportError as error:
        raise ValueError("macOS packaging requires python -m pip install dmgbuild==1.6.7") from error
    dmgbuild.build_dmg(str(archive), "Flare", settings={
        "format": "UDZO",
        "files": [str(app)],
        "symlinks": {"Applications": "/Applications"},
        "window_rect": ((200, 200), (640, 270)),
        "default_view": "icon-view",
        "icon_size": 128,
        "text_size": 14,
        "icon_locations": {"Flare.app": (150, 70), "Applications": (490, 70)},
    })
    subprocess.run(["/usr/bin/hdiutil", "verify", str(archive)], check=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, help="Pin the Rust target; default is the host")
    parser.add_argument("--debug", action="store_true", help="build the debug profile instead of release")
    parser.add_argument("--binary", type=Path, help="package an existing executable instead of building")
    parser.add_argument("--output", type=Path, default=ROOT / "target/macos/Flare.app")
    parser.add_argument("--replace", action="store_true", help="update an existing recognized Flare bundle")
    parser.add_argument("--install", action="store_true", help="also copy to ~/Applications/Flare.app")
    parser.add_argument("--register", action="store_true", help="register the bundle with Launch Services")
    args = parser.parse_args(argv)
    if sys.platform != "darwin":
        parser.error("macOS packaging must run on macOS")
    try:
        check_destination(args.output.absolute(), args.replace)
        install = Path.home() / "Applications/Flare.app"
        if args.install:
            check_destination(install, args.replace)
        version = workspace_version()
        binary = args.binary.absolute() if args.binary else built_binary(args.debug, args.target)
        if not binary.is_file():
            raise ValueError(f"Executable does not exist: {binary}")

        def prepare(stage):
            populate_bundle(binary, stage, version)
            sign_bundle(stage)
        bundle = publish_bundle(args.output, prepare, args.replace)
        if args.install and install.absolute() != bundle:
            bundle = publish_bundle(install, lambda stage: shutil.copytree(bundle, stage), args.replace)
        if args.register:
            subprocess.run([LSREGISTER, "-f", str(bundle)], check=True)
        print(bundle)
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Packaging failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
