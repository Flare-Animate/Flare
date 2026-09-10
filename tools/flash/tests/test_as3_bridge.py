"""Tests for tools/flash/next2flash/flare_as3_bridge.py.

Covers the JSON CLI contract (status/decompile/compile/patch) and the AS3
constant-string patch pass, which is pure Python and needs no Flex SDK.
"""
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

BRIDGE = os.path.abspath(os.path.join(
    os.path.dirname(__file__), "..", "next2flash", "flare_as3_bridge.py"))
PY = sys.executable


def run_bridge(args):
    proc = subprocess.run([PY, BRIDGE] + args,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return proc, json.loads(proc.stdout.decode("utf-8"))


# ---------------------------------------------------------------------------
# Minimal ABC / SWF builders
# ---------------------------------------------------------------------------

def _u30(value):
    """Encode an unsigned 30-bit variable-length integer (AVM2 u30)."""
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def _abc_with_strings(strings):
    """Build a minimal but structurally valid ABC block with a string pool.

    `strings` are the pool entries from index 1 onward; index 0 is the implicit
    empty-string sentinel and is not stored in the file.
    """
    cpool = bytearray()
    cpool += _u30(1)  # int_count      (1 == only the implicit zero entry)
    cpool += _u30(1)  # uint_count
    cpool += _u30(1)  # double_count
    cpool += _u30(len(strings) + 1)
    for s in strings:
        raw = s.encode("utf-8")
        cpool += _u30(len(raw)) + raw
    cpool += _u30(1)  # namespace_count
    cpool += _u30(1)  # ns_set_count
    cpool += _u30(1)  # multiname_count

    body = bytearray()
    body += struct.pack("<HH", 16, 46)  # minor 16, major 46 (AVM2)
    body += cpool
    body += _u30(0)  # method_count
    body += _u30(0)  # metadata_count
    body += _u30(0)  # class_count
    body += _u30(0)  # script_count
    body += _u30(0)  # method_body_count
    return bytes(body)


def _build_tag(tag_id, data):
    if len(data) < 0x3F:
        return struct.pack("<H", (tag_id << 6) | len(data)) + data
    return struct.pack("<HI", (tag_id << 6) | 0x3F, len(data)) + data


def _swf_with_abc(path, strings, doabc2=True, name="patchtest",
                  compressed=False, extra_tag=True):
    """Write an uncompressed/compressed SWF carrying one DoABC/DoABC2 tag."""
    abc = _abc_with_strings(strings)
    if doabc2:
        tag_body = struct.pack("<I", 1) + name.encode("utf-8") + b"\x00" + abc
        tags = _build_tag(82, tag_body)
    else:
        tags = _build_tag(72, abc)
    if extra_tag:
        # SetBackgroundColor — a non-ABC tag that must survive untouched.
        tags += _build_tag(9, b"\x12\x34\x56")
    tags += _build_tag(0, b"")

    rect = b"\x00"  # nbits=0 -> 5 bits of zeroes, one byte
    frame_info = struct.pack("<HH", 24 << 8, 1)
    payload = rect + frame_info + tags
    header_size = 8
    length = header_size + len(payload)
    if compressed:
        blob = b"CWS" + struct.pack("<BI", 9, length) + zlib.compress(payload, 9)
    else:
        blob = b"FWS" + struct.pack("<BI", 9, length) + payload
    with open(path, "wb") as f:
        f.write(blob)
    return path


def _read_pool(swf_path):
    """Return (string pool, non-ABC tags) of the first ABC block in a SWF."""
    sys.path.insert(0, os.path.abspath(os.path.join(
        os.path.dirname(BRIDGE), "vendor")))
    from as3_decompiler import abc_editor, swf_patcher
    swf = swf_patcher.read_swf_full(swf_path)
    pool = None
    others = []
    for tag_type, body in swf["tags"]:
        if tag_type == swf_patcher.TAG_DOABC2:
            abc = body[body.find(b"\x00", 4) + 1:]
        elif tag_type == swf_patcher.TAG_DOABC:
            abc = body
        else:
            others.append((tag_type, body))
            continue
        if pool is None:
            pool = list(abc_editor.ABCEditor(abc).abc.strings)
    return pool, others, swf


# ---------------------------------------------------------------------------
# CLI contract
# ---------------------------------------------------------------------------

def test_status_reports_vendored_decompiler():
    proc, result = run_bridge(["status"])
    assert set(result) == {"available", "version"}
    assert result["available"] is True, f"vendor not importable: {proc.stderr}"
    assert result["version"]


def test_unknown_command_is_reported_not_raised():
    proc, result = run_bridge(["frobnicate"])
    assert result["ok"] is False
    assert "unknown command" in result["error"]
    assert proc.returncode == 1


def test_compile_reports_itself_unported():
    _, result = run_bridge(["compile", "src", "out.swf"])
    assert result["ok"] is False
    assert "not yet ported" in result["error"]


def test_decompile_swf_without_abc_is_success_with_no_classes():
    td = tempfile.mkdtemp()
    try:
        swf = _swf_with_abc(os.path.join(td, "in.swf"), [])
        # Strip the ABC tag: a SWF with no AS3 at all is not an error.
        plain = os.path.join(td, "plain.swf")
        payload = b"\x00" + struct.pack("<HH", 24 << 8, 1) + _build_tag(0, b"")
        with open(plain, "wb") as f:
            f.write(b"FWS" + struct.pack("<BI", 9, 8 + len(payload)) + payload)
        _, result = run_bridge(["decompile", plain, os.path.join(td, "out")])
        assert result["ok"] is True, result
        assert result["classes"] == []
        assert result["error"] is None
        assert os.path.exists(swf)
    finally:
        shutil.rmtree(td)


# ---------------------------------------------------------------------------
# patch
# ---------------------------------------------------------------------------

def _write_patch(td, mapping):
    p = os.path.join(td, "patch.json")
    with open(p, "w", encoding="utf-8") as f:
        json.dump({"strings": mapping}, f)
    return p


def test_patch_replaces_constant_strings_and_preserves_other_tags():
    td = tempfile.mkdtemp()
    try:
        src = _swf_with_abc(os.path.join(td, "in.swf"),
                            ["oldLabel", "keepMe", "alsoOld"])
        _, before_others, before_swf = _read_pool(src)
        out = os.path.join(td, "out.swf")
        patch = _write_patch(td, {"oldLabel": "newLabel", "alsoOld": "alsoNew"})

        _, result = run_bridge(["patch", src, patch, out])
        assert result["ok"] is True, result
        assert result["replaced"] == 2
        assert result["blocks"] == 1

        pool, others, swf = _read_pool(out)
        assert "newLabel" in pool and "alsoNew" in pool
        assert "oldLabel" not in pool and "alsoOld" not in pool
        assert "keepMe" in pool, "unmatched pool entries must be left alone"
        assert pool[0] == "", "the index-0 sentinel must stay the empty string"
        # Everything that is not an ABC block is copied through untouched.
        assert others == before_others
        assert swf["frame_rate"] == before_swf["frame_rate"]
        assert swf["frame_count"] == before_swf["frame_count"]
        assert swf["rect_bytes"] == before_swf["rect_bytes"]
    finally:
        shutil.rmtree(td)


def test_patch_handles_doabc_and_compressed_swf():
    td = tempfile.mkdtemp()
    try:
        src = _swf_with_abc(os.path.join(td, "in.swf"), ["oldLabel"],
                            doabc2=False, compressed=True)
        out = os.path.join(td, "out.swf")
        patch = _write_patch(td, {"oldLabel": "newLabel"})
        _, result = run_bridge(["patch", src, patch, out])
        assert result["ok"] is True, result
        assert result["replaced"] == 1
        with open(out, "rb") as f:
            assert f.read(3) == b"CWS", "compression state must round-trip"
        pool, _, _ = _read_pool(out)
        assert "newLabel" in pool and "oldLabel" not in pool
    finally:
        shutil.rmtree(td)


def test_patch_preserves_doabc2_flags_and_name():
    td = tempfile.mkdtemp()
    try:
        src = _swf_with_abc(os.path.join(td, "in.swf"), ["oldLabel"],
                            name="frame1")
        out = os.path.join(td, "out.swf")
        patch = _write_patch(td, {"oldLabel": "newLabel"})
        _, result = run_bridge(["patch", src, patch, out])
        assert result["ok"] is True, result

        sys.path.insert(0, os.path.abspath(os.path.join(
            os.path.dirname(BRIDGE), "vendor")))
        from as3_decompiler import swf_patcher
        for tag_type, body in swf_patcher.read_swf_full(out)["tags"]:
            if tag_type == swf_patcher.TAG_DOABC2:
                assert struct.unpack("<I", body[:4])[0] == 1
                assert body[4:body.find(b"\x00", 4)] == b"frame1"
                break
        else:
            raise AssertionError("DoABC2 tag missing from patched SWF")
    finally:
        shutil.rmtree(td)


def test_patch_reports_no_match_instead_of_writing_a_file():
    td = tempfile.mkdtemp()
    try:
        src = _swf_with_abc(os.path.join(td, "in.swf"), ["somethingElse"])
        out = os.path.join(td, "out.swf")
        patch = _write_patch(td, {"notPresent": "x"})
        _, result = run_bridge(["patch", src, patch, out])
        assert result["ok"] is False
        assert "no matching" in result["error"]
        assert not os.path.exists(out), "a failed patch must not leave output"
    finally:
        shutil.rmtree(td)


def test_patch_rejects_malformed_patch_files():
    td = tempfile.mkdtemp()
    try:
        src = _swf_with_abc(os.path.join(td, "in.swf"), ["a"])
        out = os.path.join(td, "out.swf")

        missing = os.path.join(td, "nope.json")
        _, result = run_bridge(["patch", src, missing, out])
        assert result["ok"] is False and "patch file" in result["error"]

        empty = _write_patch(td, {})
        _, result = run_bridge(["patch", src, empty, out])
        assert result["ok"] is False and "no \"strings\"" in result["error"]

        wrong = os.path.join(td, "wrong.json")
        with open(wrong, "w", encoding="utf-8") as f:
            json.dump({"strings": ["not", "an", "object"]}, f)
        _, result = run_bridge(["patch", src, wrong, out])
        assert result["ok"] is False and "must be an object" in result["error"]
    finally:
        shutil.rmtree(td)
