#!/usr/bin/env python3
"""Build named release archives and verify the complete native release set."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[2]
TARGETS = {
    "x86_64-pc-windows-msvc": "windows-x64",
    "aarch64-pc-windows-msvc": "windows-arm64",
    "aarch64-apple-darwin": "macos-arm64",
    "x86_64-apple-darwin": "macos-x64",
    "x86_64-unknown-linux-gnu": "linux-x64",
    "aarch64-unknown-linux-gnu": "linux-arm64",
}
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
                     r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
                     r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?")


def workspace_version():
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    match = VERSION.fullmatch(version)
    if not match or any(int(n) > limit for n, limit in zip(match.groups(), (255, 255, 65535))):
        raise ValueError(f"Version must be semver within MSI limits: {version}")
    return version


def validate(tag):
    version = workspace_version()
    if tag and tag != f"v{version}":
        raise ValueError(f"Tag {tag!r} must match workspace version v{version}")
    if tag:
        notes = ROOT / "docs/releases" / f"v{version}.md"
        if not notes.is_file() or not notes.read_text(encoding="utf-8").strip():
            raise ValueError(f"Write user-facing release notes before tagging: {notes}")
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8", newline="\n") as stream:
            stream.write(f"version={version}\n")
    print(version)


def release_notes(version):
    if version != workspace_version():
        raise ValueError("Release notes version must match the workspace manifest")
    summary = (ROOT / "docs/releases" / f"v{version}.md").read_text(encoding="utf-8").strip()
    if not summary:
        raise ValueError("Release summary must not be empty")
    return (summary + "\n\n[Show full changelog]"
            "(https://github.com/Flare-Animate/Flare/blob/main/CHANGELOG.md)\n")


def validate_package_input(version, target, binary):
    if version != workspace_version() or target not in TARGETS:
        raise ValueError("Package version or target does not match the release contract")
    validate_binary(target, binary)


def validate_binary(target, binary):
    if target not in TARGETS:
        raise ValueError("Unknown native executable target")
    if not binary.is_file() or binary.is_symlink():
        raise ValueError(f"Release binary must be a regular file: {binary}")
    with binary.open("rb") as stream:
        header = stream.read(64)
        if "apple" in target:
            cpu = 0x0100000C if target.startswith("aarch64") else 0x01000007
            valid = header[:8] == struct.pack("<II", 0xFEEDFACF, cpu)
        elif "windows" in target:
            valid = len(header) == 64 and header[:2] == b"MZ"
            if valid:
                stream.seek(struct.unpack_from("<I", header, 60)[0])
                cpu = 0xAA64 if target.startswith("aarch64") else 0x8664
                valid = stream.read(6) == b"PE\0\0" + struct.pack("<H", cpu)
        else:
            cpu = 183 if target.startswith("aarch64") else 62
            valid = (len(header) == 64 and header[:6] == b"\x7fELF\x02\x01"
                     and struct.unpack_from("<H", header, 18)[0] == cpu
                     and header[7] in (0, 3))
        if not valid:
            raise ValueError(f"Executable header does not match {target}: {binary}")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def checksum_line(path):
    return f"{digest(path)}  {path.name}\n".encode("ascii")


def write_checksum(path):
    if not path.is_file() or path.is_symlink() or path.stat().st_size == 0:
        raise ValueError(f"Missing, empty or symlinked asset: {path}")
    sidecar = path.with_name(path.name + ".sha256")
    if sidecar.is_symlink():
        raise ValueError(f"Refusing symlink checksum: {sidecar}")
    sidecar.write_bytes(checksum_line(path))


def write_update_archive(app, archive):
    # Preserve Unix executable bits and forward-slash paths, including the signed
    # helper application. There are no aliases to source files outside the bundle.
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as stream:
        stream.write(app, app.name)
        for entry in sorted(app.rglob("*")):
            if entry.is_symlink():
                raise ValueError(f"Unexpected symlink in updater bundle: {entry}")
            stream.write(entry, entry.relative_to(app.parent).as_posix())
    write_checksum(archive)


def portable_tar_entry(info):
    # Windows hosts do not expose Unix executable bits. Set the archive mode
    # explicitly, independent of where the packager is tested.
    info.mode = 0o755 if info.name in ("flare", "flare-cli", "flare-mcp") else 0o644
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    return info


def package(version, target, binary, output):
    validate_package_input(version, target, binary)
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    name = f"flare-{version}-{TARGETS[target]}"
    if "apple" in target:
        from package_macos import release_archives
        return release_archives(binary.resolve(), output, version, target, name)
    extension = "-portable.zip" if "windows" in target else ".tar.gz"
    archive = output / (name + extension)
    with tempfile.TemporaryDirectory(prefix="flare-archive-") as temporary:
        payload = Path(temporary)
        executable = payload / ("flare.exe" if "windows" in target else "flare")
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        shutil.copy2(ROOT / "LICENSE", payload / "LICENSE")
        shutil.copy2(ROOT / "packaging/FONT_LICENSES.txt", payload / "FONT_LICENSES.txt")
        shutil.copy2(ROOT / "CHANGELOG.md", payload / "CHANGELOG.md")
        if "windows" in target:
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as stream:
                for entry in sorted(payload.iterdir()):
                    stream.write(entry, entry.name)
            standalone = output / (name + ".exe")
            shutil.copy2(binary, standalone)
            write_checksum(standalone)
        else:
            with tarfile.open(archive, "w:gz") as stream:
                for entry in sorted(payload.iterdir()):
                    stream.add(entry, arcname=entry.name, filter=portable_tar_entry)
    write_checksum(archive)
    print(archive)


def expected_assets(version):
    names = set()
    for target, suffix in TARGETS.items():
        name = f"flare-{version}-{suffix}"
        names.add(f"flare-cli-{version}-{suffix}" +
                  (".zip" if "windows" in target else ".tar.gz"))
        if "apple" in target:
            names.update((name + ".dmg", name + "-update.zip"))
        elif "windows" in target:
            names.update(name + ext for ext in ("-portable.zip", ".exe", ".msi"))
        elif "linux" in target:
            names.update(name + ext for ext in (".tar.gz", ".deb", ".rpm", ".AppImage"))
        else:
            names.add(name + ".tar.gz")
    return names


def verify_assets(version, output, finalize=False):
    if version != workspace_version():
        raise ValueError("Asset version must match the workspace manifest")
    assets = expected_assets(version)
    manifest = output / "SHA256SUMS.txt"
    if manifest.is_symlink():
        raise ValueError("Refusing symlink SHA256SUMS.txt")
    required = assets | ({name + ".sha256" for name in assets} if finalize else {manifest.name})
    actual = {p.name for p in output.iterdir()}
    if finalize:
        actual.discard(manifest.name)
    if actual != required:
        raise ValueError(f"Release mismatch: missing={sorted(required - actual)}, unexpected={sorted(actual - required)}")
    lines = []
    for name in sorted(assets):
        path = output / name
        if not path.is_file() or path.is_symlink() or path.stat().st_size == 0:
            raise ValueError(f"Invalid release asset: {name}")
        line = checksum_line(path)
        if finalize:
            sidecar = output / (name + ".sha256")
            if sidecar.is_symlink() or sidecar.read_bytes() != line:
                raise ValueError(f"Checksum verification failed: {name}")
        lines.append(line)
    expected = b"".join(lines)
    if finalize:
        manifest.write_bytes(expected)
        for name in assets:
            (output / (name + ".sha256")).unlink()
    elif manifest.is_symlink() or manifest.read_bytes() != expected:
        raise ValueError("SHA256SUMS.txt does not match the complete release")
    print(f"Verified all {len(assets)} release assets and SHA-256 checksums")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    version = commands.add_parser("validate")
    version.add_argument("--tag", default="")
    notes = commands.add_parser("notes")
    notes.add_argument("--version", required=True)
    notes.add_argument("--output", type=Path, required=True)
    archive = commands.add_parser("package")
    archive.add_argument("--version", required=True)
    archive.add_argument("--target", choices=TARGETS, required=True)
    archive.add_argument("--binary", type=Path, required=True)
    archive.add_argument("--output", type=Path, required=True)
    for command in ("finalize", "verify"):
        verify = commands.add_parser(command)
        verify.add_argument("--version", required=True)
        verify.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "validate":
            validate(args.tag)
        elif args.command == "notes":
            args.output.write_text(release_notes(args.version), encoding="utf-8")
        elif args.command in ("verify", "finalize"):
            verify_assets(args.version, args.output, args.command == "finalize")
        else:
            package(args.version, args.target, args.binary, args.output)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release packaging failed: {error}\n")


if __name__ == "__main__":
    main()
