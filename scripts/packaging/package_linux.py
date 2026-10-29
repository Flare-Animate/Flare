#!/usr/bin/env python3
"""Build DEB, RPM and AppImage packages on each native Linux architecture."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

import package_release as release

APP_ID = "io.github.Flare-Animate.Flare"
ARCHITECTURES = ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")
NOTICES = ("FONT_LICENSES.txt",)


def desktop_files():
    return {
        str(release.ROOT / "LICENSE"): "usr/share/licenses/flare/LICENSE",
        **{str(release.ROOT / "packaging" / name): "usr/share/licenses/flare/" + name for name in NOTICES},
        str(release.ROOT / "packaging/flare.metainfo.xml"): f"usr/share/metainfo/{APP_ID}.metainfo.xml",
        str(release.ROOT / "packaging/flare-mime.xml"): "usr/share/mime/packages/flare.xml",
    }


def configuration(version, target, binary, output, format_name):
    config = json.loads((release.ROOT / "packaging/cargo-packager.json").read_text())
    config.update(version=version, targetTriple=target, outDir=str(output.resolve()),
                  binaries=[{"path": str(binary), "main": True}], formats=[format_name])
    config["icons"] = [str(release.ROOT / "assets/flare-icon-256.png")]
    config["deb"].update(desktopTemplate=str(release.ROOT / "packaging/flare.desktop"),
                         files=desktop_files())
    return config


def native_package(version, target, binary, archive, temporary, format_name):
    output = temporary / "output"
    output.mkdir()
    # cargo-packager names the installed program after the file, so it is staged as `flare`.
    staged = temporary / "flare"
    shutil.copy2(binary, staged)
    staged.chmod(0o755)
    config = configuration(version, target, staged, output, format_name)
    env = dict(os.environ)
    if format_name == "appimage":
        # Keep the dotted desktop ID intact through cargo-packager's file stem.
        launcher = temporary / f"{APP_ID}.sh"
        shutil.copy2(release.ROOT / "packaging/appimage-launcher.sh", launcher)
        launcher.chmod(0o755)
        config["binaries"] = [{"path": str(launcher), "main": True},
                              {"path": str(staged), "main": False}]
        desktop = temporary / f"{APP_ID}.desktop"
        desktop.write_text((release.ROOT / "packaging/flare.desktop").read_text()
                           .replace("Exec=flare", f"Exec={launcher.name}")
                           .replace("Icon=flare", f"Icon={APP_ID}"), encoding="utf-8")
        metadata = temporary / f"{APP_ID}.metainfo.xml"
        metadata.write_text((release.ROOT / "packaging/flare.metainfo.xml").read_text()
                            .replace("flare.desktop", desktop.name), encoding="utf-8")
        files = desktop_files()
        files.pop(str(release.ROOT / "packaging/flare.metainfo.xml"))
        files[str(metadata)] = f"usr/share/metainfo/{metadata.name}"
        config["deb"]["desktopTemplate"] = str(desktop)
        config.setdefault("appimage", {})["files"] = files
        env["APPIMAGE_EXTRACT_AND_RUN"] = "1"
    subprocess.run(["cargo", "packager", "--config", json.dumps(config)],
                   cwd=release.ROOT, env=env, check=True)
    extension = "AppImage" if format_name == "appimage" else "deb"
    artifacts = list(output.glob(f"*.{extension}"))
    if len(artifacts) != 1:
        raise ValueError(f"Expected exactly one {extension}, found {len(artifacts)}")
    shutil.copy2(artifacts[0], archive)
    if format_name == "appimage":
        archive.chmod(0o755)


def rpm(target, binary, archive):
    manifest = tomllib.loads((release.ROOT / "apps/flare-desktop/Cargo.toml").read_text())
    assets = manifest["package"]["metadata"]["generate-rpm"]["assets"]
    assets[0]["source"] = str(binary.resolve())
    entries = ["{ " + ", ".join(f"{key} = {json.dumps(value)}" for key, value in entry.items()) + " }"
               for entry in assets]
    subprocess.run(["cargo", "generate-rpm", "-p", "apps/flare-desktop", "--target", target,
                    "--output", str(archive), "--set-metadata", "assets = [" + ", ".join(entries) + "]"],
                   cwd=release.ROOT, check=True)


def package(version, target, binary, output, formats):
    release.validate_package_input(version, target, binary)
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported Linux target: {target}")
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    for format_name in formats:
        extension = "AppImage" if format_name == "appimage" else format_name
        archive = output / f"flare-{version}-{release.TARGETS[target]}.{extension}"
        with tempfile.TemporaryDirectory(prefix="flare-linux-") as directory:
            if format_name == "rpm":
                rpm(target, binary, archive)
            else:
                native_package(version, target, binary, archive, Path(directory), format_name)
        release.write_checksum(archive)
        print(archive)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=ARCHITECTURES, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--formats", nargs="+", choices=("deb", "rpm", "appimage"),
                        default=["deb", "rpm", "appimage"])
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.binary, args.output, args.formats)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Linux packaging failed: {error}\n")


if __name__ == "__main__":
    main()
