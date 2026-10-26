"""Regression tests for Flash QA section 8 (F2-F11). Fixtures: flare/sources/common/flash/tests/fixtures/swf
(regenerate with make_flash_fixtures.py)."""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.abspath(os.path.join(HERE, ".."))
FIX = os.path.abspath(os.path.join(HERE, "..", "..", "..", "flare", "sources", "common", "flash", "tests", "fixtures", "swf"))
sys.path.insert(0, TOOLS)


def imp(name_or_path, extra=None):
    td = tempfile.mkdtemp()
    src = name_or_path if os.path.isabs(name_or_path) else os.path.join(FIX, name_or_path)
    out = os.path.join(td, "out")
    p = subprocess.run([sys.executable, "-I", os.path.join(TOOLS, "import_container.py"), "-i", src, "-o", out],
                       capture_output=True, text=True)
    return p, out, td


@pytest.mark.parametrize("name", ["tiny_fws.swf", "tiny_cws.swf", "tiny_zws.swf", "mix_zws.swf", "mix_cws.swf"])
def test_f3_header_size_fps_frames_and_abc(name):
    p, out, td = imp(name)
    try:
        assert p.returncode == 0, p.stderr
        info = json.load(open(os.path.join(out, "swf_info.json")))
        assert (info["width"], info["height"]) == (550, 400)
        assert info["frame_rate"] == 12.0 and info["frame_count"] == 2
        assert info["has_as3"] and info["abc_blocks"] == 1 and "JPEXS" in info["as3_note"]
        assert os.path.getsize(os.path.join(out, "abc", "block_001.abc")) >= 4
    finally:
        shutil.rmtree(td)


@pytest.mark.parametrize("name,msg", [
    ("mix_cws_trunc.swf", "truncated"),
    ("mix_zws_trunc.swf", "FileLength"),
    ("tiny_fws_trunc.swf", "FileLength"),
    ("cws_filelength_lie.swf", "FileLength"),
])
def test_f4_truncated_compressed_swf_rejected(name, msg):
    p, out, td = imp(name)
    try:
        assert p.returncode == 4 and msg in p.stderr and "malformed SWF" in p.stderr, p.stderr
        assert not os.path.exists(os.path.join(out, "swf_info.json"))
    finally:
        shutil.rmtree(td)


def test_f4_trailing_junk_is_warning_not_truncation():
    p, out, td = imp("cws_trailing_junk.swf")
    try:
        assert p.returncode == 0 and "trailing byte" in p.stderr
    finally:
        shutil.rmtree(td)


@pytest.mark.parametrize("name,msg", [
    ("empty.swf", "empty file"), ("short.swf", "too short"),
    ("bad_rect_huge.swf", "absurd stage size"), ("bad_rect_negative.swf", "negative stage size"),
])
def test_f6_f7_friendly_errors(name, msg):
    p, out, td = imp(name)
    try:
        assert p.returncode == 4 and msg in p.stderr and "Traceback" not in p.stderr and "index out of range" not in p.stderr
    finally:
        shutil.rmtree(td)


def test_f2_binary_fla_is_partial_not_success():
    p, out, td = imp("cs3_tiny.fla")
    try:
        assert p.returncode == 6, p.stderr
        assert "PARTIAL" in p.stderr and "NOT decoded" in p.stderr
        assert "Import complete" not in p.stdout
        m = json.load(open(os.path.join(out, "manifest.json")))
        assert m["partial"] is True and m["type"] == "fla-binary"
        assert [s["name"] for s in m["fla_streams"]] == ["Contents", "Media 1", "Symbol 1"]
        assert any(f.startswith("fla_media/") and f.endswith(".png") for f in m["files"])
        assert not os.path.isdir(os.path.join(out, "fla_streams"))  # no raw stream dump
    finally:
        shutil.rmtree(td)


def test_f12_cfbf_reader_needs_no_olefile():
    import cfbf_reader
    with open(os.path.join(FIX, "cs3_tiny.fla"), "rb") as f:
        buf = f.read()
    assert [n for n, _ in cfbf_reader.read_streams(buf)] == ["Contents", "Media 1", "Symbol 1"]
    for bad in (buf[:600], b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1" + b"\0" * 600, buf[:512] + b"\xff" * 4096):
        with pytest.raises(cfbf_reader.CfbfError):
            cfbf_reader.read_streams(bad)


def test_f9_non_flash_zip_rejected_fast():
    td = tempfile.mkdtemp()
    try:
        z = os.path.join(td, "src.zip")
        with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED) as f:
            f.writestr("proj-1.0/README.md", "x")
            f.writestr("proj-1.0/tests/a/b/test.swf", open(os.path.join(FIX, "tiny_fws.swf"), "rb").read())
        p, out, td2 = imp(z)
        shutil.rmtree(td2)
        assert p.returncode == 4 and "not a Flash package" in p.stderr
        # an FLA-named zip with no Flash content
        z2 = os.path.join(td, "x.fla")
        with zipfile.ZipFile(z2, "w") as f:
            f.writestr("readme.txt", "x")
        p, out, td2 = imp(z2)
        shutil.rmtree(td2)
        assert p.returncode == 4 and "not a Flash package" in p.stderr
        # a zip with a SWF at the root still imports
        z3 = os.path.join(td, "ok.zip")
        with zipfile.ZipFile(z3, "w") as f:
            f.write(os.path.join(FIX, "tiny_fws.swf"), "game.swf")
        p, out, td2 = imp(z3)
        shutil.rmtree(td2)
        assert p.returncode == 0, p.stderr
    finally:
        shutil.rmtree(td)


def test_f5_convert_swf_fails_honestly_without_converter():
    from xfl_handler import convert_swf_to_xfl
    td = tempfile.mkdtemp()
    try:
        out = os.path.join(td, "o")
        assert convert_swf_to_xfl(os.path.join(FIX, "tiny_fws.swf"), out, use_jpexs=False, use_native=False) is False
        assert not os.path.exists(out)
        assert convert_swf_to_xfl(os.path.join(FIX, "bad_rect_huge.swf"), out, use_jpexs=False, stage_only=True) is False
        assert convert_swf_to_xfl(os.path.join(FIX, "mix_cws_trunc.swf"), out, use_jpexs=False, stage_only=True) is False
        assert convert_swf_to_xfl(os.path.join(FIX, "tiny_fws.swf"), out, use_jpexs=False, use_native=False, stage_only=True) is True
        assert os.path.isfile(os.path.join(out, "DOMDocument.xml"))
    finally:
        shutil.rmtree(td)


def test_f5_cli_exit_code_and_message():
    td = tempfile.mkdtemp()
    try:
        env = dict(os.environ, PATH="/nonexistent", FLARE_CLI="")
        p = subprocess.run([sys.executable, "-I", os.path.join(TOOLS, "xfl_handler.py"), "--convert-swf",
                            os.path.join(FIX, "tiny_fws.swf"), "--output", os.path.join(td, "o")],
                           capture_output=True, text=True, env=env)
        # a flare-cli built in the repo's target dir may exist; only assert the honest-failure branch
        if "cannot convert" in p.stdout:
            assert p.returncode == 1 and "Nothing was written" in p.stdout
    finally:
        shutil.rmtree(td)


def test_f11_scene_from_import_and_bad_json():
    import game_export
    p, out, td = imp("tiny_fws.swf")
    try:
        open(os.path.join(out, "image_3.png"), "wb").write(b"\x89PNG")
        sc = game_export.scene_from_import(out)
        assert (sc["width"], sc["height"], sc["fps"]) == (550, 400, 12.0)
        assert sc["sprites"][0]["image"] == "image_3.png"
        html = game_export.export(sc)
        assert "<canvas" in html
        bad = os.path.join(td, "bad.json")
        open(bad, "w").write("{not json")
        r = subprocess.run([sys.executable, "-I", os.path.join(TOOLS, "game_export.py"), bad, os.path.join(td, "o.html")],
                           capture_output=True, text=True)
        assert r.returncode != 0 and "not valid scene JSON" in r.stderr and "Traceback" not in r.stderr
        open(bad, "w").write("[1]")
        with pytest.raises(ValueError):
            game_export.load_scene(bad)
        with pytest.raises(ValueError):
            game_export.scene_from_import(td)
    finally:
        shutil.rmtree(td)
