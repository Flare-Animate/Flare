#!/usr/bin/env python3
"""Build x64 and ARM64 MSI installers using the same WiX 5 template."""
import argparse
from pathlib import Path
import subprocess

import package_release as release

ARCHITECTURES = {"x86_64-pc-windows-msvc": "x64", "aarch64-pc-windows-msvc": "arm64"}


def package(version, target, binary, output):
    release.validate_package_input(version, target, binary)
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported MSI architecture: {target}")
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    installer = output / f"flare-{version}-{release.TARGETS[target]}.msi"
    numeric = version.split("-")[0].split("+")[0]
    subprocess.run(["wix", "build", "-arch", ARCHITECTURES[target], "-pdbtype", "none",
                    "-d", f"Version={numeric}", "-d", f"Binary={binary.resolve()}",
                    "-d", f"Icon={release.ROOT / 'assets/flare-icon.ico'}",
                    "-d", f"License={release.ROOT / 'LICENSE'}",
                    "-d", f"FontLicense={release.ROOT / 'packaging/FONT_LICENSES.txt'}",
                    "-o", str(installer), str(release.ROOT / "packaging/windows.wxs")], check=True)
    release.write_checksum(installer)
    print(installer)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=ARCHITECTURES, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.binary, args.output)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Windows packaging failed: {error}\n")


if __name__ == "__main__":
    main()
