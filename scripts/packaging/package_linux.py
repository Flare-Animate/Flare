#!/usr/bin/env python3
"""Build DEB, RPM and AppImage packages on each native Linux architecture. The AppImage is made by
`appimage.py`; the DEB by cargo-packager and the RPM by cargo-generate-rpm."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

import appimage
import package_release as release

APP_ID = appimage.APP_ID
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
    # cargo-packager names the installed program after the file, so it is staged as `flare-desktop`:
    # `flare` is the command line, and the two must not share a name on PATH.
    staged = temporary / "flare-desktop"
    shutil.copy2(binary, staged)
    staged.chmod(0o755)
    config = configuration(version, target, staged, output, format_name)
    subprocess.run(["cargo", "packager", "--config", json.dumps(config)],
                   cwd=release.ROOT, env=dict(os.environ), check=True)
    artifacts = list(output.glob("*.deb"))
    if len(artifacts) != 1:
        raise ValueError(f"Expected exactly one deb, found {len(artifacts)}")
    shutil.copy2(artifacts[0], archive)


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
            elif format_name == "appimage":
                appimage.build(version, target, binary, archive, Path(directory))
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
