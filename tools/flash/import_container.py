#!/usr/bin/env python3
"""Import various Flash container formats (FLA, XFL, SWF, SWC, AS).

This helper extracts assets into an output directory and tries to run
an external decompiler for embedded SWF content when available.

It is intentionally conservative: for FLA (binary) files it will attempt
to unzip (many modern FLA files are zip-backed XFL archives). If that fails
it will exit with a helpful message asking the user to export XFL from
Adobe Animate or to use the external decompiler toolchain.

The script writes a simple manifest.json listing the exported files.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile


def run_decompiler_on_swf(swf_path: str, outdir: str, decompiler: str | None = None) -> list:
    """Invoke the existing decompiler wrapper script (decompile_flash.py).

    Returns a list of exported file paths relative to outdir.
    """
    script_dir = os.path.dirname(os.path.abspath(__file__))
    decomp_script = os.path.join(script_dir, "decompile_flash.py")
    if not os.path.exists(decomp_script):
        raise RuntimeError("decompile_flash.py not found alongside import_container.py")

    cmd = [sys.executable, decomp_script, "--input", swf_path, "--output", outdir]
    if decompiler:
        cmd += ["--decompiler", decompiler]

    proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if proc.returncode != 0:
        raise RuntimeError(f"Decompiler failed: {proc.stderr.decode('utf-8', errors='ignore')}\n{proc.stdout.decode('utf-8', errors='ignore')}")

    # Read manifest emitted by decompile_flash.py (if present)
    mf = os.path.join(outdir, "manifest.json")
    files = []
    if os.path.exists(mf):
        try:
            with open(mf, "r", encoding="utf-8") as f:
                data = json.load(f)
                for p in data.get("files", []):
                    files.append(p)
        except Exception:
            pass
    return files


def copy_tree(src: str, dst: str, patterns=None) -> list:
    patterns = patterns or ("*",)
    copied = []
    for root, dirs, files in os.walk(src):
        rel_root = os.path.relpath(root, src)
        for fn in files:
            if any(fn.lower().endswith(p.lower().lstrip("*")) for p in patterns):
                srcpath = os.path.join(root, fn)
                tgt_dir = os.path.join(dst, rel_root) if rel_root != "." else dst
                os.makedirs(tgt_dir, exist_ok=True)
                tgt_path = os.path.join(tgt_dir, fn)
                shutil.copy2(srcpath, tgt_path)
                copied.append(os.path.relpath(tgt_path, dst))
    return copied


def extract_jsfl_functions(path: str) -> list:
    """Return a list of top-level function names declared in a JSFL/JS file.

    Uses a simple regex scan so it works without any JS parser installed.
    Each returned item is a string of the form 'functionName'.
    """
    import re
    functions = []
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            src = f.read()
        for m in re.finditer(r"\bfunction\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\(", src):
            functions.append(m.group(1))
    except OSError as e:
        print(f"Warning: could not read script file '{path}' for function extraction: {e}", file=sys.stderr)
    return functions


def detect_jsfl_api_calls(path: str) -> list:
    """Return a deduplicated list of top-level JSFL API objects referenced.

    Detects common JSFL globals: fl, doc, timeline, layer, item, etc.
    """
    import re
    apis = set()
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            src = f.read()
        # Match 'fl.someMethod(', 'doc.someMethod(' etc.
        for m in re.finditer(r"\b(fl|doc|timeline|layer|item|dom)\s*\.", src):
            apis.add(m.group(1))
    except OSError:
        # Best-effort scanner: unreadable/missing files are treated as having no detectable API calls.
        return []
    return sorted(apis)


def lint_script_file(path: str) -> list:
    """Return a list of problems found in a script file.

    Uses the `esprima` package if available to parse JS/JSFL; falls back to
    a naive bracket-counting check when esprima isn't installed.
    """
    problems = []
    try:
        import esprima  # type: ignore

        with open(path, "r", encoding="utf-8") as f:
            src = f.read()
        try:
            # esprima.parseScript raises a SyntaxError-like exception on problems
            esprima.parseScript(src)
        except Exception as e:
            problems.append({"type": "syntax", "message": str(e)})
    except Exception:
        # esprima not installed; do a best-effort parity check
        with open(path, "r", encoding="utf-8") as f:
            src = f.read()
        if src.count("{") != src.count("}") or src.count("(") != src.count(")") or src.count("[") != src.count("]"):
            problems.append({"type": "syntax", "message": "Mismatched parentheses/braces/brackets (esprima not installed)"})
    return problems


def handle_swc(path: str, outdir: str, decompiler: str | None) -> list:
    exported = []
    with zipfile.ZipFile(path, "r") as z:
        members = z.namelist()
        # Extract everything to a subdir
        extract_dir = os.path.join(outdir, "swc_extracted")
        os.makedirs(extract_dir, exist_ok=True)
        z.extractall(extract_dir)

    # Copy script files, resources, and Apache Flex MXML files
    exported += copy_tree(
        extract_dir, outdir,
        patterns=(".as", ".jsfl", ".js", ".mxml", ".png", ".jpg", ".jpeg",
                  ".svg", ".xml", ".txt", ".mp3"),
    )

    # Find embedded swf(s)
    for root, _, files in os.walk(extract_dir):
        for fn in files:
            if fn.lower().endswith(".swf"):
                swf = os.path.join(root, fn)
                swf_outdir = os.path.join(outdir, "swf_%s" % os.path.splitext(fn)[0])
                os.makedirs(swf_outdir, exist_ok=True)
                try:
                    files_from_sw = run_decompiler_on_swf(swf, swf_outdir, decompiler)
                    exported += [os.path.join(os.path.basename(swf_outdir), x) for x in files_from_sw]
                except Exception as e:
                    print(f"Warning: decompilation of {swf} failed: {e}", file=sys.stderr)
    return exported


def handle_xfl_dir(path: str, outdir: str) -> list:
    # XFL is a directory-based project; copy a safe set of asset types
    exported = []
    exported += copy_tree(
        path, outdir,
        patterns=(".svg", ".xml", ".png", ".jpg", ".jpeg", ".as", ".jsfl",
                  ".js", ".mxml", ".mp3", ".wav"),
    )
    return exported


def _safe_extract_zip(path: str, extract_dir: str) -> None:
    """Extract *path* (a ZIP file) to *extract_dir*, rejecting path-traversal entries.

    Entries whose normalised relative path would escape *extract_dir* (Zip Slip)
    or that carry absolute paths are silently skipped.
    """
    abs_extract = os.path.realpath(extract_dir)
    try:
        z = zipfile.ZipFile(path, "r")
    except zipfile.BadZipFile:
        # Damaged central directory (common in FLAs saved by crashed Animate):
        # recover entries from local file headers instead.
        for name, data in _scan_local_headers(path):
            entry = name.replace("\\", "/").lstrip("/")
            if not entry or ".." in entry.split("/") or (len(entry) >= 2 and entry[1] == ":"):
                continue
            target = os.path.realpath(os.path.join(abs_extract, entry))
            if not target.startswith(abs_extract + os.sep) or entry.endswith("/"):
                continue
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with open(target, "wb") as dst:
                dst.write(data)
        return
    with z:
        for member in z.infolist():
            # Normalise separator and strip leading ./ or /
            entry = member.filename.replace("\\", "/")
            while entry.startswith("./"):
                entry = entry[2:]
            entry = entry.lstrip("/")

            # Reject path traversal and absolute paths
            if not entry or ".." in entry.split("/") or entry.endswith("..") or os.path.isabs(entry):
                continue
            # Also reject Windows drive-letter paths (e.g. "C:/...")
            if len(entry) >= 2 and entry[1] == ":":
                continue

            target = os.path.realpath(os.path.join(abs_extract, entry))
            if not target.startswith(abs_extract + os.sep):
                continue  # would escape output directory

            if member.filename.endswith("/"):
                os.makedirs(target, exist_ok=True)
            else:
                os.makedirs(os.path.dirname(target), exist_ok=True)
                with z.open(member) as src, open(target, "wb") as dst:
                    shutil.copyfileobj(src, dst)


ZIP_LOCAL = bytes.fromhex("504b0304")


def _scan_local_headers(path: str):
    """Yield (name, bytes) from ZIP local headers, ignoring the central directory."""
    import struct
    import zlib
    with open(path, "rb") as f:
        buf = f.read()
    pos = buf.find(ZIP_LOCAL)
    while pos != -1 and pos + 30 <= len(buf):
        _, flag, method, _, _, _, csize, _, nlen, xlen = struct.unpack_from("<HHHHHIIIHH", buf, pos + 4)
        name = buf[pos + 30:pos + 30 + nlen].decode("utf-8" if flag & 0x800 else "cp437", "replace")
        start = pos + 30 + nlen + xlen
        data = None
        try:
            if method == 8:
                d = zlib.decompressobj(-15)
                data = d.decompress(buf[start:] if (flag & 8 or not csize) else buf[start:start + csize])
                end = len(buf) - len(d.unused_data) if (flag & 8 or not csize) else start + csize
            elif method == 0 and csize:
                data, end = buf[start:start + csize], start + csize
            else:
                end = start
        except zlib.error:
            end = start
        if data is not None:
            yield name, data
        pos = buf.find(ZIP_LOCAL, max(end, pos + 4))


class PartialImport(Exception):
    """Raised when only part of a file could be recovered; carries the exported files."""

    def __init__(self, message: str, files: list, extra: dict | None = None):
        super().__init__(message)
        self.files = files
        self.extra = extra or {}


def handle_cfb_fla(path: str, outdir: str) -> list:
    """Legacy (Flash 8 / CS3 / CS4) binary FLA = OLE2 compound document.

    Read natively (cfbf_reader.py, no olefile). The proprietary timeline /
    library records inside the streams are NOT decoded, so this never reports
    success: embedded JPEG/PNG bitmaps are recovered to fla_media/ and the
    import ends as PARTIAL (exit 6) with the stream inventory in the manifest.
    """
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import cfbf_reader
    name = os.path.basename(path)
    with open(path, "rb") as f:
        buf = f.read()
    try:
        streams = cfbf_reader.read_streams(buf)
    except cfbf_reader.CfbfError as e:
        raise RuntimeError(f"{name}: damaged binary FLA ({e})")
    media_dir = os.path.join(outdir, "fla_media")
    exported, n = [], 0
    for sname, data in streams:
        for ext, img in cfbf_reader.carve_images(data):
            if n >= 5000:
                break
            n += 1
            os.makedirs(media_dir, exist_ok=True)
            rel = "fla_media/%04d%s" % (n, ext)
            with open(os.path.join(outdir, rel), "wb") as f:
                f.write(img)
            exported.append(rel)
    inv = [{"name": sname, "size": len(data)} for sname, data in streams]
    msg = (f"{name}: legacy binary FLA (Flash CS4 or earlier, OLE2). Timeline, layers and library are NOT "
           f"decoded (format is proprietary; {len(streams)} streams found). Recovered {n} embedded bitmap(s). "
           "Open it in Adobe Animate and save as a CS5+ .fla or XFL to import everything.")
    raise PartialImport(msg, exported, {"fla_streams": inv, "partial": True})


def handle_fla(path: str, outdir: str, decompiler: str | None) -> list:
    # Try to unzip the FLA (some FLA files are zip archives containing XFL
    # structure). If unzipping works, treat as XFL. Otherwise give a helpful
    # message asking the user to export XFL or use the external decompiler.
    exported = []
    with open(path, "rb") as f:
        magic = f.read(8)
    if magic == bytes.fromhex("d0cf11e0a1b11ae1"):
        return handle_cfb_fla(path, outdir)
    if magic[:4] == ZIP_LOCAL or zipfile.is_zipfile(path):
        check_flash_zip(path)
        extract_dir = os.path.join(outdir, "fla_extracted")
        os.makedirs(extract_dir, exist_ok=True)
        _safe_extract_zip(path, extract_dir)
        exported += handle_xfl_dir(extract_dir, outdir)
    else:
        # As a fallback, try to run the decompiler on the FLA; some tools accept
        # SWF-like extraction from FLA. If that fails, provide a friendly error.
        tmp = os.path.join(outdir, "fla_attempt_decompile")
        os.makedirs(tmp, exist_ok=True)
        try:
            exported += run_decompiler_on_swf(path, tmp, decompiler)
        except Exception:
            raise RuntimeError(
                "Unable to parse binary FLA file. Export XFL from Adobe Animate or provide an XFL folder, or use an external decompiler to extract assets.")
    return exported


MAX_STAGE_PX = 16384
MAX_BODY = 512 * 1024 * 1024


def read_swf(raw: bytes, name: str) -> dict:
    """Validate a SWF and return header facts + decompressed body (F3/F4/F6/F7).

    Raises RuntimeError with a plain message for empty/short/corrupt/truncated
    input, FileLength mismatch (also after zlib/LZMA decompression), and
    inverted or absurd (> 16384 px) stage rectangles.
    """
    import struct
    import zlib
    if len(raw) == 0:
        raise RuntimeError(f"{name}: empty file (0 bytes), not a SWF")
    if len(raw) < 8:
        raise RuntimeError(f"{name}: too short to be a SWF ({len(raw)} bytes)")
    sig, ver = raw[:3], raw[3]
    declared = struct.unpack_from("<I", raw, 4)[0]
    want = max(declared - 8, 0)
    limit = min(want, MAX_BODY) + 1
    warnings = []
    try:
        if sig == b"FWS":
            body = raw[8:]
        elif sig == b"CWS":
            d = zlib.decompressobj()
            body = d.decompress(raw[8:], limit)
            if not d.eof and len(body) <= want:
                raise RuntimeError(f"{name}: malformed SWF: zlib stream truncated")
            if d.eof and d.unused_data:
                warnings.append(f"{len(d.unused_data)} trailing byte(s) after the compressed stream ignored")
        elif sig == b"ZWS":
            import lzma
            if len(raw) < 17:
                raise RuntimeError(f"{name}: malformed SWF: truncated LZMA header")
            flt = lzma._decode_filter_properties(lzma.FILTER_LZMA1, raw[12:17])
            dec = lzma.LZMADecompressor(lzma.FORMAT_RAW, filters=[flt])
            body = dec.decompress(raw[17:], limit)
        else:
            raise RuntimeError(f"{name}: not a SWF file (bad signature)")
    except (zlib.error, EOFError, OSError, ValueError) as e:
        raise RuntimeError(f"{name}: malformed SWF: compressed data damaged or truncated ({e})")
    if declared != 8 + len(body):
        raise RuntimeError(
            f"{name}: malformed SWF: header FileLength={declared} but "
            f"{'decompressed ' if sig != b'FWS' else ''}length is {8 + len(body)}")
    if not body:
        raise RuntimeError(f"{name}: malformed SWF: no data after header")
    nbits = body[0] >> 3
    rect_len = (5 + 4 * nbits + 7) // 8
    if rect_len + 4 > len(body):
        raise RuntimeError(f"{name}: malformed SWF: truncated header")
    bits = int.from_bytes(body[:rect_len], "big")
    total = rect_len * 8

    def sb(i):
        v = (bits >> (total - 5 - nbits * (i + 1))) & ((1 << nbits) - 1) if nbits else 0
        return v - (1 << nbits) if nbits and v >> (nbits - 1) else v
    xmin, xmax, ymin, ymax = (sb(i) for i in range(4))
    w, h = xmax - xmin, ymax - ymin
    if w < 0 or h < 0:
        raise RuntimeError(f"{name}: malformed SWF: negative stage size ({w}x{h} twips)")
    if w > MAX_STAGE_PX * 20 or h > MAX_STAGE_PX * 20:
        raise RuntimeError(f"{name}: malformed SWF: absurd stage size {w // 20}x{h // 20} px (limit {MAX_STAGE_PX} px)")
    rate = body[rect_len + 1] + body[rect_len] / 256.0
    count = struct.unpack_from("<H", body, rect_len + 2)[0]
    return {"sig": sig.decode(), "version": ver, "declared": declared, "body": body,
            "tags_at": rect_len + 4, "width": w / 20, "height": h / 20,
            "frame_rate": round(rate, 3), "frame_count": count, "warnings": warnings}


def builtin_swf_extract(path: str, outdir: str, hdr: dict | None = None) -> list:
    """Dependency-free SWF fallback (tag walk per SWF spec, as in ruffle/swf2js).

    Writes swf_info.json (size, fps, frames, tag histogram), dumps embedded
    JPEG/PNG/GIF from DefineBits* tags (6/21/35/90; JPEG3/4 alpha dropped) and
    each AS3 DoABC/DoABC2 block as raw abc/*.abc. ABC is NOT decompiled to
    .as source without JPEXS; the result says so.
    """
    import struct
    with open(path, "rb") as f:
        raw = f.read()
    name = os.path.basename(path)
    hdr = hdr or read_swf(raw, name)
    body, pos = hdr["body"], hdr["tags_at"]
    for w in hdr["warnings"]:
        print(f"Warning: {name}: {w}", file=sys.stderr)
    tags, exported = {}, []
    saw_end = False
    # validate the whole tag stream before writing anything
    p = pos
    while p + 2 <= len(body):
        cl = struct.unpack_from("<H", body, p)[0]
        c, n = cl >> 6, cl & 0x3F
        p += 2
        if n == 0x3F:
            if p + 4 > len(body):
                raise RuntimeError(f"{name}: malformed SWF: tag stream runs past end of file")
            n = struct.unpack_from("<I", body, p)[0]
            p += 4
        if p + n > len(body):
            raise RuntimeError(f"{name}: malformed SWF: tag {c} (length {n}) runs past end of file")
        p += n
        if c == 0:
            saw_end = True
            break
    if not saw_end:
        raise RuntimeError(f"{name}: malformed SWF: missing End tag")
    os.makedirs(outdir, exist_ok=True)
    abc_n = 0
    while pos + 2 <= len(body):
        code_len = struct.unpack_from("<H", body, pos)[0]
        code, ln = code_len >> 6, code_len & 0x3F
        pos += 2
        if ln == 0x3F:
            ln = struct.unpack_from("<I", body, pos)[0]
            pos += 4
        data = body[pos:pos + ln]
        pos += ln
        tags[code] = tags.get(code, 0) + 1
        if code == 0:
            break
        if code in (72, 82):
            blk = data
            if code == 82:  # DoABC2: flags u32, cstring name, then ABC
                z = data.find(b"\0", 4)
                blk = data[z + 1:] if z >= 0 else b""
            if len(blk) >= 4:
                abc_n += 1
                os.makedirs(os.path.join(outdir, "abc"), exist_ok=True)
                rel = "abc/block_%03d.abc" % abc_n
                with open(os.path.join(outdir, rel), "wb") as f:
                    f.write(blk)
                exported.append(rel)
        if code in (6, 21, 35, 90) and len(data) > 2:
            cid = struct.unpack_from("<H", data)[0]
            img = data[2:]
            if code in (35, 90):
                alen = struct.unpack_from("<I", img)[0]
                img = img[4 + (2 if code == 90 else 0):][:alen]
            img = img.replace(bytes.fromhex("ffd9ffd8"), b"", 1) if img[:4] == bytes.fromhex("ffd9ffd8") else img
            ext = ".png" if img[1:4] == b"PNG" else ".gif" if img[:3] == b"GIF" else ".jpg"
            iname = f"image_{cid}{ext}"
            with open(os.path.join(outdir, iname), "wb") as f:
                f.write(img)
            exported.append(iname)
    info = {"version": hdr["version"], "compression": {"FWS": "none", "CWS": "zlib", "ZWS": "lzma"}[hdr["sig"]],
            "width": hdr["width"], "height": hdr["height"], "frame_rate": hdr["frame_rate"],
            "frame_count": hdr["frame_count"], "has_as3": 82 in tags or 72 in tags,
            "abc_blocks": abc_n, "tags": tags}
    if abc_n:
        info["as3_note"] = ("ActionScript 3 bytecode exported as raw abc/*.abc; decompiling to .as source "
                            "needs JPEXS (ffdec) and is unsupported in the built-in extractor")
        print(f"Note: {name}: {abc_n} AS3 ABC block(s) saved as raw .abc; no .as source without JPEXS.", file=sys.stderr)
    with open(os.path.join(outdir, "swf_info.json"), "w", encoding="utf-8") as f:
        json.dump(info, f, indent=2)
    return exported + ["swf_info.json"]


def handle_swf(path: str, outdir: str, decompiler: str | None) -> list:
    exported = []
    with open(path, "rb") as f:
        raw = f.read()
    hdr = read_swf(raw, os.path.basename(path))  # reject empty/corrupt/truncated/absurd first
    tmp = os.path.join(outdir, "swf_decomp")
    os.makedirs(tmp, exist_ok=True)
    try:
        exported += run_decompiler_on_swf(path, tmp, decompiler)
    except RuntimeError as e:
        if decompiler:
            raise
        shutil.rmtree(tmp, ignore_errors=True)
        print(f"Warning: JPEXS unavailable, using built-in SWF extractor ({str(e).splitlines()[0]})", file=sys.stderr)
        return builtin_swf_extract(path, outdir, hdr)
    # include script files from decompiler output too
    exported += copy_tree(tmp, outdir, patterns=(".svg", ".png", ".jpg", ".jpeg", ".xml", ".as", ".jsfl", ".js"))
    return exported


def handle_as(path: str, outdir: str) -> list:
    os.makedirs(outdir, exist_ok=True)
    tgt = os.path.join(outdir, os.path.basename(path))
    shutil.copy2(path, tgt)
    return [os.path.basename(tgt)]


# Magic headers for media/Adobe formats we pass through (ideas from ruffle/jpexs sniffing).
MEDIA_MAGIC = {
    ".flv": (b"FLV",),
    ".f4v": (b"ftyp",),  # ISO BMFF: checked at offset 4
    ".psd": (b"8BPS",),
    ".ai": (b"%PDF", b"%!PS"),
    ".aep": (b"RIFX",),
}


def handle_media(path: str, outdir: str, ext: str) -> list:
    with open(path, "rb") as f:
        head = f.read(12)
    probe = head[4:8] if ext == ".f4v" else head
    if not any(probe.startswith(m) for m in MEDIA_MAGIC[ext]):
        raise RuntimeError(f"{os.path.basename(path)}: not a valid {ext} file (bad header)")
    return handle_as(path, outdir)


FLASH_EXT = (".swf", ".fla", ".xfl", ".swc", ".as", ".jsfl")
FLASH_ROOT_NAMES = ("domdocument.xml", "catalog.xml", "meta-inf/air/application.xml", "library.swf")
MAX_UNPACKED = 2 << 30


def check_flash_zip(path: str, need_dom: bool = False) -> None:
    """Cheap central-directory check (F9): reject non-Flash zips before extracting anything.

    A Flash package has a Flash marker at the archive root or one wrapper folder
    down (DOMDocument.xml, catalog.xml, AIR descriptor, or .swf/.fla/.xfl/.swc/.as
    files). Deeper-only content (e.g. a source tree with test SWFs) is rejected.
    """
    name = os.path.basename(path)
    try:
        z = zipfile.ZipFile(path, "r")
    except zipfile.BadZipFile:
        return  # damaged directory: _safe_extract_zip salvages via local headers
    with z:
        infos = z.infolist()
        if sum(i.file_size for i in infos) > MAX_UNPACKED:
            raise RuntimeError(f"{name}: archive expands past {MAX_UNPACKED >> 30} GiB; refusing to extract")
        found = False
        for i in infos:
            n = i.filename.replace("\\", "/").lstrip("./").lower()
            if n.endswith("/"):
                continue
            parts = n.split("/")
            if len(parts) > 2:
                continue
            if need_dom:
                found |= parts[-1] == "domdocument.xml"
            else:
                found |= n in FLASH_ROOT_NAMES or parts[-1] in FLASH_ROOT_NAMES or n.endswith(FLASH_EXT)
        if not found:
            what = "an XFL-based FLA (no DOMDocument.xml)" if need_dom else "a Flash package (no DOMDocument.xml, SWF, FLA, XFL, SWC or AS at its top level)"
            raise RuntimeError(f"{name}: zip is not {what}; {len(infos)} entries, nothing extracted")


def handle_air(path: str, outdir: str, decompiler: str | None) -> list:
    """AIR/ANE packages are ZIPs holding SWF/SWC/XML; extract then recurse."""
    if not zipfile.is_zipfile(path):
        raise RuntimeError(f"{os.path.basename(path)}: AIR/ANE package is not a ZIP archive")
    check_flash_zip(path)
    extract_dir = os.path.join(outdir, "pkg_extracted")
    os.makedirs(extract_dir, exist_ok=True)
    _safe_extract_zip(path, extract_dir)
    exported = []
    for root, _, names in os.walk(extract_dir):
        for n in names:
            full = os.path.join(root, n)
            rel = os.path.relpath(full, outdir).replace("\\", "/")
            exported.append(rel)
            low = n.lower()
            try:
                if low.endswith(".fla"):
                    exported += handle_fla(full, os.path.join(outdir, n + "_x"), decompiler)
                elif low.endswith(".swf"):
                    exported += handle_swf(full, os.path.join(outdir, n + "_x"), decompiler)
                elif low.endswith(".swc"):
                    exported += handle_swc(full, os.path.join(outdir, n + "_x"), decompiler)
            except Exception as e:  # nested decompile optional
                print(f"Warning: {rel}: {e}", file=sys.stderr)
    return exported


def main():
    parser = argparse.ArgumentParser(description="Import Flash container and extract assets to an output directory")
    parser.add_argument("--input", "-i", required=True, help="Input file or folder (FLA/XFL/SWF/SWC/AS/JSFL/AIR/ANE/FLV/F4V/PSD/AI/AEP)")
    parser.add_argument("--output", "-o", required=True, help="Output directory (will be created)")
    parser.add_argument("--decompiler", "-d", help="Optional path to an external Flash decompiler (JPEXS/ffdec)")
    parser.add_argument("--no-lint-scripts", dest="lint_scripts", action="store_false", help="Disable script linting (requires 'esprima' for best results)")
    parser.set_defaults(lint_scripts=True)

    args = parser.parse_args()

    inp = args.input
    outdir = os.path.abspath(args.output)
    decompiler = args.decompiler

    if not os.path.exists(inp):
        print(f"Input {inp} not found", file=sys.stderr)
        sys.exit(2)

    if not os.path.isdir(inp) and os.path.getsize(inp) == 0:
        print(f"{os.path.basename(inp)}: empty file (0 bytes), nothing to import", file=sys.stderr)
        sys.exit(4)
    os.makedirs(outdir, exist_ok=True)

    files = []
    partial_msg, manifest_extra = None, {}
    try:
        if os.path.isdir(inp):
            # Treat as XFL folder
            files += handle_xfl_dir(inp, outdir)
            container_type = "xfl"
        else:
            _, ext = os.path.splitext(inp)
            ext = ext.lower()
            with open(inp, "rb") as f:
                magic = f.read(3)
            if magic in (b"FWS", b"CWS", b"ZWS") and ext not in (".swc",):
                ext = ".swf"  # SWF under any extension (e.g. .ssf, .spl)
            if ext == ".zip":
                ext = ".air"  # generic archive: extract and recurse
            if ext == ".swf":
                files += handle_swf(inp, outdir, decompiler)
                container_type = "swf"
            elif ext == ".swc":
                files += handle_swc(inp, outdir, decompiler)
                container_type = "swc"
            elif ext == ".fla":
                files += handle_fla(inp, outdir, decompiler)
                container_type = "fla"
            elif ext == ".xfl":
                # Occasionally XFL projects are distributed as .xfl zip files
                if zipfile.is_zipfile(inp):
                    extract_dir = os.path.join(outdir, "xfl_extracted")
                    os.makedirs(extract_dir, exist_ok=True)
                    _safe_extract_zip(inp, extract_dir)
                    files += handle_xfl_dir(extract_dir, outdir)
                else:
                    files += handle_fla(inp, outdir, decompiler)
                container_type = "xfl"
            elif ext in (".air", ".ane"):
                files += handle_air(inp, outdir, decompiler)
                container_type = ext.lstrip('.')
            elif ext in MEDIA_MAGIC:
                files += handle_media(inp, outdir, ext)
                container_type = ext.lstrip('.')
            elif ext == ".as" or ext == ".jsfl":
                files += handle_as(inp, outdir)
                container_type = ext.lstrip('.')
            else:
                print(f"Unsupported file type: {ext}", file=sys.stderr)
                sys.exit(3)
    except PartialImport as e:
        files += e.files
        partial_msg, manifest_extra, container_type = str(e), e.extra, "fla-binary"
    except RuntimeError as e:
        print(str(e), file=sys.stderr)
        sys.exit(4)
    except Exception as e:
        print(f"Unexpected error: {e}", file=sys.stderr)
        sys.exit(5)

    # Perform optional script linting for extracted script files
    problems = {}
    jsfl_metadata: dict = {}  # per-file function/API info (issues #52, #11)
    if args.lint_scripts:
        for rel in list(files):
            if rel.lower().endswith((".jsfl", ".js", ".as")):
                full = os.path.join(outdir, rel)
                if os.path.exists(full):
                    probs = lint_script_file(full)
                    if probs:
                        problems[rel] = probs
                        for p in probs:
                            print(f"Script problem in {rel}: {p}", file=sys.stderr)
                    # Collect JSFL-specific metadata (issues #52, #11)
                    if rel.lower().endswith(".jsfl"):
                        fns = extract_jsfl_functions(full)
                        apis = detect_jsfl_api_calls(full)
                        if fns or apis:
                            jsfl_metadata[rel] = {}
                            if fns:
                                jsfl_metadata[rel]["functions"] = fns
                            if apis:
                                jsfl_metadata[rel]["jsfl_apis"] = apis

    # Write manifest
    manifest = {
        "input": os.path.abspath(inp),
        "output_dir": os.path.abspath(outdir),
        "files": files,
        "type": container_type,
        "problems": problems,
    }
    manifest.update(manifest_extra)
    if jsfl_metadata:
        manifest["jsfl_scripts"] = jsfl_metadata
    mf = os.path.join(outdir, "manifest.json")
    with open(mf, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)

    # Also write problems file for easy consumption
    if problems:
        pf = os.path.join(outdir, "script_problems.json")
        with open(pf, "w", encoding="utf-8") as f:
            json.dump(problems, f, indent=2)

    if partial_msg:
        print(f"Import PARTIAL: {partial_msg}", file=sys.stderr)
        sys.exit(6)
    print(f"Import complete. Exported {len(files)} files.")
    sys.exit(0)


if __name__ == "__main__":
    main()
