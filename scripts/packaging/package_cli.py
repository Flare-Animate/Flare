#!/usr/bin/env python3
"""Package the command line tool and the MCP server separately from the desktop installers."""
import argparse
from pathlib import Path
import tarfile
import zipfile

import package_release as release


def package(version, target, cli, mcp, output):
    release.validate_package_input(version, target, cli)
    release.validate_binary(target, mcp)
    output.mkdir(parents=True, exist_ok=True)
    windows = "windows" in target
    suffix = ".exe" if windows else ""
    archive = output / (f"flare-cli-{version}-{release.TARGETS[target]}" + (".zip" if windows else ".tar.gz"))
    files = [(cli, "flare-cli" + suffix), (mcp, "flare-mcp" + suffix),
             (release.ROOT / "LICENSE", "LICENSE")]
    if windows:
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as stream:
            for source, destination in files:
                stream.write(source, destination)
    else:
        with tarfile.open(archive, "w:gz") as stream:
            for source, destination in files:
                info = stream.gettarinfo(str(source), arcname=destination)
                info.mode = 0o644 if destination == "LICENSE" else 0o755
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                with source.open("rb") as contents:
                    stream.addfile(info, contents)
    release.write_checksum(archive)
    return archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=release.TARGETS, required=True)
    parser.add_argument("--binary", type=Path, required=True, help="flare-cli")
    parser.add_argument("--mcp-binary", type=Path, required=True, help="flare-mcp")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        print(package(args.version, args.target, args.binary, args.mcp_binary, args.output))
    except (OSError, ValueError) as error:
        parser.exit(1, f"CLI packaging failed: {error}\n")
