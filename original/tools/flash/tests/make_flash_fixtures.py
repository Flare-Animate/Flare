#!/usr/bin/env python3
"""Regenerate the tiny SWF / CFBF fixtures in flare/sources/common/flash/tests/fixtures/swf.

Deterministic (seeded). Run:  python3 -I tools/flash/tests/make_flash_fixtures.py
The files are committed; Rust (ac_swf tests) and Python (test_flash_import_fixes.py) read them.
"""
import lzma
import os
import random
import struct
import zlib

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..",
                   "flare", "sources", "common", "flash", "tests", "fixtures", "swf")


def rect(xmin, xmax, ymin, ymax):
    vals = [xmin, xmax, ymin, ymax]
    n = max(len(format(v if v >= 0 else (-v - 1), "b")) + 1 for v in vals)
    bits = format(n, "05b")
    for v in vals:
        bits += format(v & ((1 << n) - 1), "0%db" % n)
    bits += "0" * (-len(bits) % 8)
    return bytes(int(bits[i:i + 8], 2) for i in range(0, len(bits), 8))


def tag(code, body=b""):
    if len(body) < 63:
        return struct.pack("<H", (code << 6) | len(body)) + body
    return struct.pack("<HI", (code << 6) | 63, len(body)) + body


def body(stage=(0, 11000, 0, 8000), frames=2, extra=b""):
    b = rect(*stage) + struct.pack("<BBH", 0, 12, frames)
    b += tag(9, b"\x10\x20\x30")                       # SetBackgroundColor
    b += tag(82, struct.pack("<I", 0) + b"Main\0" + b"\x10\x00\x2e\x00" + b"\xAA" * 8)  # DoABC2
    b += extra
    b += tag(1) + tag(1) + tag(0)
    return b


def swf(sig, bd, version=9, declared=None):
    n = 8 + len(bd) if declared is None else declared
    head = sig + bytes([version]) + struct.pack("<I", n)
    if sig == b"FWS":
        return head + bd
    if sig == b"CWS":
        return head + zlib.compress(bd, 9)
    c = lzma.LZMACompressor(lzma.FORMAT_ALONE, filters=[{"id": lzma.FILTER_LZMA1, "dict_size": 1 << 16, "lc": 3, "lp": 0, "pb": 2}])
    raw = c.compress(bd) + c.flush()
    props, stream = raw[:5], raw[13:]  # drop the 8-byte size field of .lzma
    return head + struct.pack("<I", len(stream)) + props + stream


def cfbf(streams):
    """Minimal CFBF v3 (512-byte sectors, all streams >= 4096 bytes so no mini stream)."""
    SS = 512
    ENDOFCHAIN, FATSECT, FREESECT = 0xFFFFFFFE, 0xFFFFFFFD, 0xFFFFFFFF
    data_secs, entries = [], []
    for name, d in streams:
        d = d + b"\0" * max(0, 4096 - len(d))
        start = len(data_secs)
        for i in range(0, len(d), SS):
            data_secs.append(d[i:i + SS].ljust(SS, b"\0"))
        entries.append((name, start, len(d), len(data_secs) - start))
    ndir = (len(entries) + 1 + 3) // 4
    first_dir = len(data_secs)
    nsec = len(data_secs) + ndir
    nfat = 1
    while nfat * 128 < nsec + nfat:
        nfat += 1
    fat = [FREESECT] * (nfat * 128)
    for _, start, _, cnt in entries:
        for k in range(cnt):
            fat[start + k] = start + k + 1 if k + 1 < cnt else ENDOFCHAIN
    for k in range(ndir):
        fat[first_dir + k] = first_dir + k + 1 if k + 1 < ndir else ENDOFCHAIN
    fat_start = nsec
    for k in range(nfat):
        fat[fat_start + k] = FATSECT

    def dent(name, typ, left, right, child, start, size):
        nm = (name + "\0").encode("utf-16-le")
        e = nm.ljust(64, b"\0") + struct.pack("<HBBIII", len(nm), typ, 1, left, right, child)
        e += b"\0" * 16 + b"\0" * 4 + b"\0" * 16 + struct.pack("<IQ", start, size)[0:4] + struct.pack("<Q", size)[0:8]
        return e.ljust(128, b"\0")[:128]

    # directory: root, then streams as a right-leaning chain under the root
    d = [dent("Root Entry", 5, 0xFFFFFFFF, 0xFFFFFFFF, 1 if entries else 0xFFFFFFFF, ENDOFCHAIN, 0)]
    for i, (name, start, size, _) in enumerate(entries):
        right = i + 2 if i + 1 < len(entries) else 0xFFFFFFFF
        d.append(dent(name, 2, 0xFFFFFFFF, right, 0xFFFFFFFF, start, size))
    while len(d) % 4:
        d.append(b"\0" * 128)
    dir_bytes = b"".join(d)
    hdr = bytearray(512)
    hdr[0:8] = bytes.fromhex("d0cf11e0a1b11ae1")
    struct.pack_into("<HHHHH", hdr, 24, 0x3E, 3, 0xFFFE, 9, 6)
    struct.pack_into("<IIIIIIIII", hdr, 40, 0, nfat, first_dir, 0, 4096, ENDOFCHAIN, 0, ENDOFCHAIN, 0)
    difat = [fat_start + k for k in range(nfat)] + [FREESECT] * (109 - nfat)
    struct.pack_into("<109I", hdr, 76, *difat)
    fat_bytes = struct.pack("<%dI" % len(fat), *fat)
    return bytes(hdr) + b"".join(data_secs) + dir_bytes + fat_bytes


def main():
    os.makedirs(OUT, exist_ok=True)
    random.seed(7)
    words = [b"alpha", b"beta", b"gamma", b"delta", b"frame", b"stage", b"movie"]
    mix = b" ".join(random.choice(words) for _ in range(900)) + bytes(random.randrange(256) for _ in range(300))
    ok = body()
    big = body(extra=tag(87, struct.pack("<HI", 3, 0) + mix))
    w = {
        "tiny_fws.swf": swf(b"FWS", ok),
        "tiny_cws.swf": swf(b"CWS", ok),
        "tiny_zws.swf": swf(b"ZWS", ok),
        "mix_zws.swf": swf(b"ZWS", big),
        "mix_cws.swf": swf(b"CWS", big),
        "empty.swf": b"",
        "short.swf": b"FWS\x09\x00",
        # bad RECT: 2^29 twips wide (26843 px), FileLength correct
        "bad_rect_huge.swf": swf(b"FWS", body(stage=(0, 1 << 29, 0, 8000))),
        # inverted RECT (xmax < xmin)
        "bad_rect_negative.swf": swf(b"FWS", body(stage=(5000, 100, 0, 8000))),
    }
    w["mix_cws_trunc.swf"] = w["mix_cws.swf"][: len(w["mix_cws.swf"]) // 2]
    w["mix_zws_trunc.swf"] = w["mix_zws.swf"][: len(w["mix_zws.swf"]) // 2]
    w["tiny_fws_trunc.swf"] = w["tiny_fws.swf"][:-6]
    # CWS whose zlib stream is complete but FileLength claims more
    w["cws_filelength_lie.swf"] = swf(b"CWS", ok, declared=8 + len(ok) + 40)
    # complete CWS followed by junk (QA's "truncated" Prep 3 sample was this shape)
    w["cws_trailing_junk.swf"] = w["tiny_cws.swf"] + b"\xEE" * 64
    png = bytes.fromhex("89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c63f8cfc0f01f0005000201a4eb5d5b0000000049454e44ae426082")
    w["cs3_tiny.fla"] = cfbf([("Contents", b"\x01\x02Contents" * 3), ("Media 1", b"hdr" + png + b"tail"), ("Symbol 1", b"sym")])
    for k, v in w.items():
        with open(os.path.join(OUT, k), "wb") as f:
            f.write(v)
    print("wrote", len(w), "fixtures to", os.path.normpath(OUT))


if __name__ == "__main__":
    main()
