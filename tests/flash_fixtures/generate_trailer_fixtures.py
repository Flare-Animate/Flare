#!/usr/bin/env python3
"""Generate regression fixtures for the ZIP trailer bug behind issue #70.

Real Adobe Animate FLAs in the wild carry an end-of-central-directory record
whose size/offset fields do not describe the central directory actually in the
file. Both minizip and Python's zipfile reject such an archive outright, which
produced the "invalid/corrupt ZIP" error on perfectly valid documents.

This builds the two shapes seen in the wild, deterministically and at a few
hundred bytes each:

  stale_trailer.fla   a normal FLA whose EOCD overstates cd_size by exactly one
                      extra central-directory record - the same 54-byte
                      discrepancy measured on a real 564 KB sample, where the
                      cause is a duplicate local `mimetype` header.

  orphan_local.fla    a FLA carrying an orphan duplicate local file header that
                      no central-directory record refers to, as the real sample
                      does. Its trailer is *correct*, so this is the negative
                      case: the repair must recognise it as already consistent
                      and leave the archive alone rather than second-guessing a
                      good record.

stale_trailer.fla is a valid XFL archive that neither minizip nor zipfile can
open until the trailer is repaired. orphan_local.fla opens as-is, and must keep
opening: it guards against the repair over-correcting.

Run: python generate_trailer_fixtures.py
"""
import os
import struct
import zipfile
import io

HERE = os.path.dirname(os.path.abspath(__file__))

# A fixed timestamp for every archive entry.
#
# zipfile.writestr() stamps each entry with the current time, so regenerating
# a fixture produced a byte-different file every time. That matters for
# committed fixtures: `git status` was permanently dirty after any
# regeneration, and a fixture's checksum could not be used to tell whether it
# had really changed. Fixing the timestamp makes generation reproducible, so a
# fixture differs only when its content differs.
_ZIP_EPOCH = (1980, 1, 1, 0, 0, 0)


def zinfo(name):
    """A ZipInfo with a fixed timestamp, so archives are reproducible."""
    zi = zipfile.ZipInfo(name, date_time=_ZIP_EPOCH)
    zi.compress_type = zipfile.ZIP_DEFLATED
    # 0o644: rw-r--r--, so the archive is usable on any host.
    zi.external_attr = (0o644 << 16)
    return zi


MIMETYPE = b"application/vnd.adobe.xfl"

DOM_DOCUMENT = b"""<?xml version="1.0" encoding="utf-8"?>
<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/" width="550" height="400"\
 frameRate="24" backgroundColor="#FFFFFF">
  <symbols/>
  <timelines>
    <DOMTimeline name="Scene 1" currentFrame="0">
      <layers>
        <DOMLayer name="Layer 1" color="#4FFF4F" current="true">
          <frames>
            <DOMFrame index="0" duration="1" tweenType="none">
              <elements/>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timelines>
</DOMDocument>
"""


def build_xfl_zip(extra_member: bytes = b"") -> bytes:
    """A well-formed XFL-in-ZIP archive, optionally with a trailing member."""
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        # Adobe writes an uncompressed, first-entry `mimetype`, ODF style.
        info = zipfile.ZipInfo("mimetype", date_time=(2020, 8, 19, 0, 0, 0))
        info.compress_type = zipfile.ZIP_STORED
        zf.writestr(info, MIMETYPE)
        zf.writestr(zinfo("DOMDocument.xml"), DOM_DOCUMENT)
        zf.writestr(zinfo("PublishSettings.xml"), b"<publishSettings/>")
        if extra_member:
            zf.writestr(zinfo("extra.txt"), extra_member)
    return buf.getvalue()


def find_eocd(data: bytes) -> int:
    i = data.rfind(b"PK\x05\x06")
    if i < 0:
        raise ValueError("no EOCD in generated archive")
    return i


def patch_cd_size(data: bytes, new_cd_size: int) -> bytes:
    """Rewrite the EOCD's cd_size field, leaving everything else alone."""
    eocd = find_eocd(data)
    out = bytearray(data)
    struct.pack_into("<I", out, eocd + 12, new_cd_size)
    return bytes(out)


def make_stale_trailer() -> bytes:
    """Overstate cd_size by exactly one 54-byte central-directory record.

    54 = 46 (fixed record) + 8 ("mimetype") + 0 (extra) + 0 (comment). This is
    the discrepancy measured on the real sample: the writer counted the
    `mimetype` record twice.
    """
    data = build_xfl_zip()
    eocd = find_eocd(data)
    _, _, _, _, _, cd_size, _, _ = struct.unpack("<4sHHHHIIH", data[eocd:eocd + 22])
    return patch_cd_size(data, cd_size + 54)


def make_orphan_local_header() -> bytes:
    """Insert a duplicate local file header that no central record refers to.

    Inserted immediately before the central directory so the offsets already in
    the central directory stay valid - which is exactly the situation on the
    real sample, where the orphan sat just before the CD and every cd_off
    relative offset was still correct.
    """
    data = build_xfl_zip()
    eocd = find_eocd(data)
    _, _, _, _, _, _, cd_off, _ = struct.unpack("<4sHHHHIIH", data[eocd:eocd + 22])

    name = b"mimetype"
    payload = MIMETYPE
    # A complete, standalone local file header + stored data, referring to
    # nothing. Same size as the real sample's 63-byte orphan.
    orphan = (b"PK\x03\x04"
              + struct.pack("<HHHHHIIIHH", 20, 0, 0, 0, 0, 0, len(payload), len(payload),
                            len(name), 0)
              + name + payload)
    # Insert *before* the central directory, so every relative offset already
    # recorded in the CD stays correct - as on the real sample. The CD itself
    # therefore shifts by len(orphan) and the EOCD has to say so.
    cd_size = eocd - cd_off
    out = data[:cd_off] + orphan + data[cd_off:]
    return _repoint_cd(out, eocd + len(orphan), cd_off + len(orphan), cd_size)


def _repoint_cd(data: bytes, new_eocd: int, new_cd_off: int, cd_size: int) -> bytes:
    """Fix the EOCD after the central directory shifted.

    Only cd_offset moves; the size of the directory itself is unchanged.
    """
    out = bytearray(data)
    struct.pack_into("<I", out, new_eocd + 12, cd_size)     # cd_size
    struct.pack_into("<I", out, new_eocd + 16, new_cd_off)  # cd_offset
    return bytes(out)


def make_zipslip() -> bytes:
    """A valid archive with a member whose name escapes the output directory.

    Generated here rather than hand-built in the C++ test: a hand-built central
    directory is easy to get subtly wrong, and when it is wrong the extractor
    rejects the archive before it ever reaches the member name -- so the
    path-traversal assertion passes without the guard ever being exercised.
    Verified below that zipfile really can read it, which is what makes the test
    meaningful.
    """
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        # zinfo(), not writestr(name, ...): writestr stamps each entry with the
        # current time, which made this committed fixture differ on every
        # regeneration and left `git status` permanently dirty.
        zf.writestr(zinfo("benign.txt"), b"this one is fine")
        zf.writestr(zinfo("../escape.txt"), b"this one is not")
        zf.writestr(zinfo("nested/deep.txt"), b"and this one is fine too")
    return buf.getvalue()


def main() -> None:
    for name, blob in (("stale_trailer.fla", make_stale_trailer()),
                       ("orphan_local.fla", make_orphan_local_header())):
        path = os.path.join(HERE, name)
        with open(path, "wb") as f:
            f.write(blob)
        print(f"wrote {name} ({len(blob)} bytes)")

        # Sanity: a naive reader must fail, which is the whole point.
        try:
            with zipfile.ZipFile(path) as z:
                print(f"  !! unexpectedly opened by zipfile ({len(z.namelist())} entries)")
        except zipfile.BadZipFile as e:
            print(f"  confirmed: zipfile rejects it ({e})")

    # The Zip-slip fixture, which has to be a *readable* archive: a malformed one
    # is rejected before the extractor reaches the member name, so the test that
    # uses it would pass without the traversal guard ever running.
    path = os.path.join(HERE, "zipslip.zip")
    blob = make_zipslip()
    with open(path, "wb") as f:
        f.write(blob)
    print(f"wrote zipslip.zip ({len(blob)} bytes)")
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        print(f"  confirmed readable: {names}")
        if "../escape.txt" not in names:
            raise SystemExit("zipslip.zip does not contain the traversal member")
        if z.read("benign.txt") != b"this one is fine":
            raise SystemExit("zipslip.zip: benign member does not read back")
        if z.read("nested/deep.txt") != b"and this one is fine too":
            raise SystemExit("zipslip.zip: nested member does not read back")
    # Reproducible, like the other fixtures: regenerating must not dirty the
    # tree, or a diff on this file says nothing.
    if make_zipslip() != blob:
        raise SystemExit("zipslip.zip is not reproducible: "
                         "two generations differ")


if __name__ == "__main__":
    main()
