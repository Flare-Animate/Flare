"""Format coverage for import_container.py: synthetic fixtures + optional real samples."""
import glob
import os
import struct
import subprocess
import sys
import zipfile
import zlib

import pytest

SCRIPT = os.path.join(os.path.dirname(__file__), "..", "import_container.py")
SAMPLES = os.environ.get("FLARE_SAMPLES", os.path.expanduser("~/Downloads/Flare Samples"))


def run(inp, out):
    return subprocess.run([sys.executable, SCRIPT, "-i", str(inp), "-o", str(out)], capture_output=True, text=True)


def tiny_swf(sig=b"FWS"):
    # RECT nbits=0 (1 byte), rate, count, ShowFrame, End
    body = bytes([0]) + struct.pack("<HH", 30 << 8, 1) + struct.pack("<H", 1 << 6) + b"\0\0"
    payload = zlib.compress(body) if sig == b"CWS" else body
    return sig + bytes([10]) + struct.pack("<I", 8 + len(body)) + payload


@pytest.mark.parametrize("name,data", [
    ("a.swf", tiny_swf()), ("b.swf", tiny_swf(b"CWS")), ("c.ssf", tiny_swf()),
    ("v.flv", b"FLV\x01\x05" + b"\0" * 8), ("v.f4v", b"\0\0\0\x18ftypf4v " + b"\0" * 8),
    ("p.psd", b"8BPS" + b"\0" * 20), ("i.ai", b"%PDF-1.5\n"), ("e.aep", b"RIFX" + b"\0" * 8),
])
def test_formats(tmp_path, name, data):
    p = tmp_path / name
    p.write_bytes(data)
    r = run(p, tmp_path / "out")
    assert r.returncode == 0, r.stderr


def test_bad_media_header(tmp_path):
    p = tmp_path / "x.psd"
    p.write_bytes(b"nope")
    assert run(p, tmp_path / "o").returncode == 4


def test_air_package(tmp_path):
    p = tmp_path / "app.air"
    with zipfile.ZipFile(p, "w") as z:
        z.writestr("META-INF/AIR/application.xml", "<application/>")
        z.writestr("main.swf", tiny_swf())
    r = run(p, tmp_path / "o")
    assert r.returncode == 0, r.stderr
    assert (tmp_path / "o" / "main.swf_x" / "swf_info.json").exists()


def test_corrupt_central_directory_fla(tmp_path):
    p = tmp_path / "broken.fla"
    with zipfile.ZipFile(p, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("DOMDocument.xml", "<DOMDocument/>")
    data = p.read_bytes()
    cd = data.rfind(b"PK\x01\x02")
    p.write_bytes(data[:cd] + b"XX" + data[cd + 2:])
    r = run(p, tmp_path / "o")
    assert r.returncode == 0, r.stderr


SAMPLE_FILES = [f for f in glob.glob(os.path.join(SAMPLES, "*")) if not f.endswith(".lnk") and os.path.isfile(f)]


@pytest.mark.skipif(not SAMPLE_FILES, reason="no real samples")
@pytest.mark.parametrize("path", SAMPLE_FILES, ids=os.path.basename)
def test_real_samples(tmp_path, path):
    r = run(path, tmp_path / "o")
    assert r.returncode == 0, r.stderr
