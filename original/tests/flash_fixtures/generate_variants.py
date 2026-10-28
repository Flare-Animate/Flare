"""Build a spread of synthetic containers and report what each reader makes of them.

The committed fixtures cover one shape of each format. Real files turn up variants
those do not: a CWS with a zlib body, a ZWS with LZMA, a DefineShape3 or 4, a SWF
carrying only sound, a Moho project with a BOM, one behind leading whitespace, one
with no layers, one that is not JSON at all, a plain-text file that mentions the
.anme header in passing.

A test can only assert what somebody thought to assert, so a variant nobody
imagined is simply absent. This generates them and prints what the shipped readers
report, so an unhandled shape shows up as a wrong answer rather than as silence.

usage: python tests/flash_fixtures/generate_variants.py <outdir>
       then:  probe_samples <outdir>
"""
import io
import json
import os
import struct
import sys
import zipfile
import zlib

ZIP_EPOCH = (1980, 1, 1, 0, 0, 0)


def zinfo(name):
    zi = zipfile.ZipInfo(name, date_time=ZIP_EPOCH)
    zi.compress_type = zipfile.ZIP_DEFLATED
    zi.external_attr = 0o644 << 16
    return zi


def zip_bytes(members):
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in members:
            z.writestr(zinfo(name), data)
    return buf.getvalue()


# ------------------------------------------------------------------- SWF ----
def swf(version, body, signature=b"FWS"):
    """A SWF with a one-byte RECT (nbits = 0), so the tag offset is predictable."""
    out = bytearray()
    out += signature
    out += bytes([version])
    out += struct.pack("<I", 8 + 1 + 4 + len(body))
    out += b"\x00"                                  # RECT: nbits = 0
    out += struct.pack("<H", 0x0100)                # frame rate 1.00
    out += struct.pack("<H", 1)                     # frame count
    return bytes(out) + body


def tag(code, data=b""):
    if len(data) < 0x3F:
        return struct.pack("<H", (code << 6) | len(data)) + data
    return (struct.pack("<H", (code << 6) | 0x3F)
            + struct.pack("<I", len(data)) + data)


SHAPE2 = tag(2, b"\x00" * 20)      # DefineShape
SHAPE3 = tag(32, b"\x00" * 10)     # DefineShape3
SHAPE4 = tag(83, b"\x00" * 10)     # DefineShape4
MORPH = tag(46, b"\x00" * 10)      # DefineMorphShape
FONT = tag(10, b"\x00" * 8)        # DefineFont
VIDEO = tag(60, b"\x00" * 12)      # DefineVideoStream
SOUND = tag(14, b"\x00" * 20)      # DefineSound
SPRITE = tag(39, struct.pack("<H", 1) + struct.pack("<H", 1) + tag(0))
END = tag(0)

# width/height/frameRate are ATTRIBUTES on <DOMDocument>. Verified against a real
# Adobe export -- Grandfather Clock and Metronome.fla carries
#   xmlns="http://ns.adobe.com/xfl/2008/" backgroundColor="#BCC0C4" width="640"
#   height="360"
# -- and against tests/flash_fixtures/sample.fla. There is no <stage> element in
# the format, and an earlier version of this generator invented one, then read the
# reader's correct refusal as a bug.
MINIMAL_DOM = (
    '<?xml version="1.0" encoding="UTF-8"?>\n'
    '<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/" version="4.0" '
    'width="640" height="480" frameRate="30" backgroundColor="#3D9F3D">\n'
    '  <symbols/>\n'
    '  <timelines><DOMTimeline name="Scene 1"><layers>'
    '<DOMLayer name="Layer 1"><frames>'
    '<DOMFrame index="0" duration="1"/></frames>'
    '</DOMLayer></layers></DOMTimeline></timelines>\n'
    '</DOMDocument>\n'
)

# A document that states its size in a shape the reader does not know. Kept
# deliberately: it should report the defaults rather than invent a number, and the
# case is worth having because those defaults are indistinguishable from a real
# 550x400 stage.
STAGE_ELEMENT_DOM = (
    '<?xml version="1.0" encoding="UTF-8"?>\n'
    '<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/" version="4.0">\n'
    '  <properties><document><properties><stage>'
    '<Stage color="#3D9F3D"/><width>640</width><height>480</height>'
    '</stage></properties></document></properties>\n'
    '</DOMDocument>\n'
)


def moho_doc(version=1021, main="m", layers=None):
    # The key is "version", not "moho_version" -- verified against a real bare
    # document (Check-Line.mohoproj carries "version": 1045 and no
    # "moho_version"), and against MohoReader.cpp, which reads root.value("version").
    # An earlier version of this generator used "moho_version", so every variant
    # it built reported version 0 and that read as the reader ignoring the field.
    #
    # The mime_type likewise has to be right, because the reader checks it:
    # application/x-vnd.lm_mohodoc, from the same two real documents. A guessed
    # "application/x-moho" was rejected outright, which looked like a reader bug
    # until the real value was checked.
    return json.dumps({
        "version": version,
        "major_version": 0,
        "mime_type": "application/x-vnd.lm_mohodoc",
        "main": main,
        "fps": 24,
        "range": [1, 24],
        "width": 480, "height": 360,
        "layers": layers if layers is not None else [{
            "type": "BoneLayer", "name": "root", "uuid": "l0",
            "parent": -1, "visible": True,
            "bones": [{"name": "b0", "pos": [0, 0], "rot": 0,
                       "scale": [1, 1]}],
        }],
    })


def build(outdir):
    v = {}

    # --- SWF: one tag of each interesting code ---------------------------
    v["plain_fws.swf"] = swf(10, SHAPE2 + FONT + VIDEO + END)
    v["shapes34.swf"] = swf(11, SHAPE3 + SHAPE4 + MORPH + END)
    v["sound_only.swf"] = swf(9, SOUND + END)
    v["sprite_nested.swf"] = swf(10, SHAPE2 + SPRITE + END)

    # zlib body (CWS): every byte after offset 8 is deflated. The reader must
    # decompress before walking tags, or it reads tag codes out of zlib data and
    # reports an empty movie.
    payload = (b"\x00" + struct.pack("<H", 0x0100) + struct.pack("<H", 1)
               + SHAPE2 + FONT + VIDEO + END)
    cws = bytearray(swf(12, b"", b"CWS")[:8])
    cws += zlib.compress(payload, 9)
    cws[0:3] = b"CWS"
    struct.pack_into("<I", cws, 4, len(cws))
    v["compressed_cws.swf"] = bytes(cws)

    # --- FLA / XFL -------------------------------------------------------
    v["xfl_minimal.fla"] = zip_bytes([
        ("mimetype", b"application/x-shockwave-flash"),
        ("DOMDocument.xml", MINIMAL_DOM.encode()),
        ("PublishSettings.xml", b"<publishSettings/>"),
    ])
    v["xfl_stage_element.fla"] = zip_bytes([
        ("mimetype", b"application/x-shockwave-flash"),
        ("DOMDocument.xml", STAGE_ELEMENT_DOM.encode()),
    ])
    v["xfl_no_root_attrs.fla"] = zip_bytes([
        ("DOMDocument.xml",
         b'<?xml version="1.0"?><DOMDocument '
         b'xmlns="http://ns.adobe.com/xfl/2008/"><symbols/></DOMDocument>'),
    ])

    # --- Moho ------------------------------------------------------------
    v["plain.mohoproj"] = moho_doc().encode()
    v["bom.mohoproj"] = b"\xef\xbb\xbf" + moho_doc().encode()
    v["ws.mohoproj"] = b"\n\n  \t " + moho_doc().encode()
    v["newver1045.mohoproj"] = moho_doc(version=1045).encode()
    v["nolayers.mohoproj"] = moho_doc(layers=[]).encode()
    v["notjson.mohoproj"] = b"this is not a project at all\n"
    v["empty.mohoproj"] = b""
    v["real.anme"] = b"Anime Studio Project\r\n  version 9\r\n{\r\n"
    # The header phrase present but not at the start: must NOT be Legacy, or the
    # signature check is matching anywhere in the file rather than at the front.
    v["looks_anme.later"] = b"hello\nAnime Studio Project\n"
    v["binary.bin"] = b"\x7fELF\x02\x01\x01" + bytes(range(256)) * 4
    v["zipped.moho"] = zip_bytes([("Project.mohoproj",
                                   moho_doc().encode())])

    os.makedirs(outdir, exist_ok=True)
    written = []
    for name, data in v.items():
        p = os.path.join(outdir, name)
        with open(p, "wb") as f:
            f.write(data)
        written.append(p)
    return written


def main():
    outdir = sys.argv[1] if len(sys.argv) > 1 else "variants"
    written = build(outdir)
    print(f"wrote {len(written)} variant(s) to {outdir}")
    for p in written:
        print(f"   {os.path.basename(p)}  {os.path.getsize(p)} bytes")
    print()
    print("Now run the shipped readers over them:")
    print(f"   ./build/RelWithDebInfo/probe_samples {outdir}")


if __name__ == "__main__":
    main()
