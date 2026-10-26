"""Dependency-free reader for OLE2 / CFBF compound files ([MS-CFB]).

Legacy binary .fla files (Flash 8 / CS3 / CS4 and earlier) are CFBF containers.
This replaces the optional `olefile` dependency of import_container.py (F12).
It is hardened for untrusted input: every sector index and chain is bounds- and
loop-checked, and the total stream bytes are capped.
"""
from __future__ import annotations

import struct

MAGIC = bytes.fromhex("d0cf11e0a1b11ae1")
MAX_FREE = 0xFFFFFFFA          # sector ids at or above this are special
ENDOFCHAIN = 0xFFFFFFFE
MAX_TOTAL = 1 << 30            # 1 GiB of reassembled stream data


class CfbfError(ValueError):
    pass


def is_cfbf(head: bytes) -> bool:
    return head[:8] == MAGIC


def read_streams(buf: bytes) -> list:
    """Return [(path, bytes)] for every stream; path joins storages with '/'."""
    if len(buf) < 512 or buf[:8] != MAGIC:
        raise CfbfError("not an OLE2/CFBF compound file")
    major, = struct.unpack_from("<H", buf, 26)
    sshift, mshift = struct.unpack_from("<HH", buf, 30)
    if major not in (3, 4) or not (7 <= sshift <= 16) or not (1 <= mshift <= sshift):
        raise CfbfError("unsupported CFBF header (version %d, sector shift %d)" % (major, sshift))
    ss, ms = 1 << sshift, 1 << mshift
    first_dir, = struct.unpack_from("<I", buf, 48)
    cutoff, first_mfat, nmfat, first_difat, ndifat = struct.unpack_from("<IIIII", buf, 56)
    nsect = (len(buf) - 512) // ss + 1

    def sector(i: int) -> bytes:
        if i >= nsect or i >= MAX_FREE:
            raise CfbfError("sector %d outside the file (truncated or corrupt)" % i)
        o = 512 + i * ss
        s = buf[o:o + ss]
        if len(s) < ss:
            s = s.ljust(ss, b"\0")  # last sector may be short
        return s

    difat = list(struct.unpack_from("<109I", buf, 76))
    nxt, guard = first_difat, 0
    while nxt < MAX_FREE and guard < min(ndifat, 100000) + 1:
        s = sector(nxt)
        vals = struct.unpack("<%dI" % (ss // 4), s)
        difat += vals[:-1]
        nxt = vals[-1]
        guard += 1
    fat = []
    for f in difat:
        if f < MAX_FREE:
            fat += struct.unpack("<%dI" % (ss // 4), sector(f))
    if not fat:
        raise CfbfError("no FAT sectors")

    total = [0]

    def chain(start: int, fat_tbl) -> list:
        out, s = [], start
        while s < MAX_FREE:
            if len(out) > len(fat_tbl):
                raise CfbfError("FAT chain loops")
            out.append(s)
            if s >= len(fat_tbl):
                raise CfbfError("FAT chain leaves the table")
            s = fat_tbl[s]
        return out

    def read_chain(start: int) -> bytes:
        secs = chain(start, fat)
        total[0] += len(secs) * ss
        if total[0] > MAX_TOTAL:
            raise CfbfError("compound file expands past %d MiB" % (MAX_TOTAL >> 20))
        return b"".join(sector(i) for i in secs)

    dirb = read_chain(first_dir)
    ents = []
    for i in range(0, len(dirb) - 127, 128):
        e = dirb[i:i + 128]
        nl, typ = struct.unpack_from("<HB", e, 64)
        name = e[:max(0, min(nl, 64) - 2)].decode("utf-16-le", "replace")
        left, right, child = struct.unpack_from("<III", e, 68)
        start, size = struct.unpack_from("<IQ", e, 116)
        ents.append((name, typ, left, right, child, start, size))
    if not ents or ents[0][1] != 5:
        raise CfbfError("missing root entry")
    mfat = []
    if first_mfat < MAX_FREE:
        mraw = read_chain(first_mfat)
        mfat = list(struct.unpack("<%dI" % (len(mraw) // 4), mraw))
    ministream = read_chain(ents[0][5]) if ents[0][5] < MAX_FREE else b""

    def read_entry(start: int, size: int) -> bytes:
        if size < cutoff:
            secs = chain(start, mfat) if start < MAX_FREE else []
            data = b"".join(ministream[s * ms:(s + 1) * ms] for s in secs)
        else:
            data = read_chain(start)
        return data[:size]

    out, seen = [], set()

    def walk(idx: int, prefix: str, depth: int = 0):
        # in-order walk of the red-black sibling tree
        stack = []
        cur = idx
        while stack or cur != 0xFFFFFFFF:
            while cur != 0xFFFFFFFF:
                if cur >= len(ents) or cur in seen or depth > 64:
                    raise CfbfError("directory tree is corrupt")
                seen.add(cur)
                stack.append(cur)
                cur = ents[cur][2]
            cur = stack.pop()
            name, typ, _, right, child, start, size = ents[cur]
            path = prefix + name
            if typ == 2:
                out.append((path, read_entry(start, size)))
            elif typ == 1 and child != 0xFFFFFFFF:
                walk(child, path + "/", depth + 1)
            cur = right

    walk(ents[0][4], "")
    return out


def carve_images(data: bytes, limit: int = 4096) -> list:
    """Embedded complete JPEG/PNG images in a stream: [(ext, bytes)].

    A candidate runs from its signature to the next signature; PNGs are cut at
    their IEND chunk and JPEGs at the last EOI marker, and candidates without
    that terminator (random hits inside compressed data) are dropped.
    """
    sigs = ((b"\xff\xd8\xff", ".jpg"), (b"\x89PNG\r\n\x1a\n", ".png"))
    hits = []
    for sig, ext in sigs:
        p = data.find(sig)
        while p != -1 and len(hits) < limit:
            hits.append((p, ext))
            p = data.find(sig, p + 1)
    hits.sort()
    res = []
    for k, (p, ext) in enumerate(hits):
        win = data[p:hits[k + 1][0] if k + 1 < len(hits) else len(data)]
        if ext == ".png":
            i = win.find(b"IEND")
            if i < 0 or len(win) < 33:
                continue
            win = win[:i + 8]
        else:
            i = win.rfind(b"\xff\xd9")
            if i < 0 or i + 2 < 128:
                continue
            win = win[:i + 2]
        res.append((ext, win))
    return res
