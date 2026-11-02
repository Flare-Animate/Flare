#!/usr/bin/env python3
"""Generate minimal-but-valid sample files for every Flash/Animate format Flare
imports. These are deterministic test fixtures for the Flash import pipeline
(flashimport.cpp / XFLReader). Run: python generate_fixtures.py

Formats covered: FLA, XFL, SWF (uncompressed FWS), SWC, FLV, F4V, AS.
"""
import os, struct, zipfile, shutil

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


def build_uncompressed_swf(version=5, w_px=550, h_px=400, fps=24, frames=1) -> bytes:
    """Minimal uncompressed FWS SWF: header + RECT + frameRate + frameCount.
    Matches the layout flashimport.cpp::readSwfHeader expects."""
    xmax, ymax = w_px * 20, h_px * 20  # twips
    nbits = 1
    while (1 << (nbits - 1)) <= xmax:
        nbits += 1
    total_bits = 5 + 4 * nbits
    rect = bytearray((total_bits + 7) // 8)
    bitpos = 0

    def wbits(val, n):
        nonlocal bitpos
        for b in range(n - 1, -1, -1):
            if (val >> b) & 1:
                rect[bitpos // 8] |= 1 << (7 - (bitpos % 8))
            bitpos += 1

    wbits(nbits, 5)
    wbits(0, nbits); wbits(xmax, nbits); wbits(0, nbits); wbits(ymax, nbits)
    tail = bytes([0, fps, frames & 0xFF, (frames >> 8) & 0xFF])
    body = bytes(rect) + tail
    file_len = 8 + len(body)
    hdr = b'FWS' + bytes([version]) + struct.pack('<I', file_len)
    return hdr + body


DOMDOCUMENT = (
    '<?xml version="1.0" encoding="utf-8"?>\n'
    '<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/" '
    'width="550" height="400" frameRate="24" backgroundColor="#FFFFFF">\n'
    '  <symbols/>\n'
    '  <timelines>\n'
    '    <DOMTimeline name="Scene 1">\n'
    '      <layers>\n'
    '        <DOMLayer name="Layer 1">\n'
    '          <frames><DOMFrame index="0" duration="1"/></frames>\n'
    '        </DOMLayer>\n'
    '      </layers>\n'
    '    </DOMTimeline>\n'
    '  </timelines>\n'
    '</DOMDocument>\n'
)

CATALOG_XML = (
    '<?xml version="1.0" encoding="utf-8"?>\n'
    '<swc xmlns="http://www.adobe.com/flash/swccatalog/9">\n'
    '  <components>\n'
    '    <component className="SampleButton" name="SampleButton" uri="assets"/>\n'
    '  </components>\n'
    '</swc>\n'
)

AS_SRC = (
    'package {\n'
    '    import flash.display.Sprite;\n'
    '    public class Main extends Sprite {\n'
    '        public function Main() { trace("Hello from Flare fixture"); }\n'
    '    }\n'
    '}\n'
)


def main():
    swf = build_uncompressed_swf()

    # SWF
    with open(os.path.join(HERE, 'sample.swf'), 'wb') as f:
        f.write(swf)

    # FLV: 9-byte header (version 1, video+audio) + PreviousTagSize0
    with open(os.path.join(HERE, 'sample.flv'), 'wb') as f:
        f.write(b'FLV' + bytes([1, 0x05, 0, 0, 0, 9]) + struct.pack('>I', 0))

    # F4V: ftyp box (major brand 'f4v ', compat 'isom')
    with open(os.path.join(HERE, 'sample.f4v'), 'wb') as f:
        f.write(struct.pack('>I', 20) + b'ftyp' + b'f4v ' + struct.pack('>I', 0) + b'isom')

    # AS
    with open(os.path.join(HERE, 'sample.as'), 'w', encoding='utf-8') as f:
        f.write(AS_SRC)

    # XFL directory: DOMDocument.xml + LIBRARY/
    xfl_dir = os.path.join(HERE, 'sample_xfl')
    if os.path.isdir(xfl_dir):
        shutil.rmtree(xfl_dir)
    os.makedirs(os.path.join(xfl_dir, 'LIBRARY'))
    with open(os.path.join(xfl_dir, 'DOMDocument.xml'), 'w', encoding='utf-8') as f:
        f.write(DOMDOCUMENT)
    # XFL marker file (Adobe writes a .xfl stub at the project root)
    with open(os.path.join(xfl_dir, 'sample.xfl'), 'w', encoding='utf-8') as f:
        f.write('PROXY-CS5\n')

    # FLA: ZIP archive containing the XFL document at the root
    with zipfile.ZipFile(os.path.join(HERE, 'sample.fla'), 'w', zipfile.ZIP_DEFLATED) as z:
        z.writestr(zinfo('DOMDocument.xml'), DOMDOCUMENT)
        z.writestr(zinfo('LIBRARY/'), '')

    # SWC: ZIP with catalog.xml + library.swf
    with zipfile.ZipFile(os.path.join(HERE, 'sample.swc'), 'w', zipfile.ZIP_DEFLATED) as z:
        z.writestr(zinfo('catalog.xml'), CATALOG_XML)
        z.writestr(zinfo('library.swf'), swf)

    # AIR / ANE: ZIP packages with their manifests
    with zipfile.ZipFile(os.path.join(HERE, 'sample.air'), 'w', zipfile.ZIP_DEFLATED) as z:
        z.writestr(zinfo('META-INF/AIR/application.xml'),
                   '<application xmlns="http://ns.adobe.com/air/application/33.0"><id>t</id></application>')
        z.writestr(zinfo('main.swf'), swf)
    with zipfile.ZipFile(os.path.join(HERE, 'sample.ane'), 'w', zipfile.ZIP_DEFLATED) as z:
        z.writestr(zinfo('META-INF/ANE/extension.xml'),
                   '<extension xmlns="http://ns.adobe.com/air/extension/33.0"><id>t</id></extension>')
        z.writestr(zinfo('library.swf'), swf)

    # JSFL
    with open(os.path.join(HERE, 'sample.jsfl'), 'w', encoding='utf-8') as f:
        f.write('fl.trace("hi");\n')

    # PSD: "8BPS" v1, 6 reserved, 3 channels, 1x1, 8bpc, RGB(3), 3 empty sections + raw image
    with open(os.path.join(HERE, 'sample.psd'), 'wb') as f:
        f.write(b'8BPS' + struct.pack('>H', 1) + bytes(6) + struct.pack('>HIIHH', 3, 1, 1, 8, 3)
                + struct.pack('>III', 0, 0, 0) + struct.pack('>H', 0) + bytes([255, 0, 0]))

    # AI: PDF-compatible Illustrator file
    with open(os.path.join(HERE, 'sample.ai'), 'wb') as f:
        f.write(b'%PDF-1.5\n%AIPrivateData\n1 0 obj<</Type/Catalog>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n')

    print('Generated fixtures in', HERE)
    for name in sorted(os.listdir(HERE)):
        p = os.path.join(HERE, name)
        kind = 'dir ' if os.path.isdir(p) else 'file'
        size = '' if os.path.isdir(p) else f'{os.path.getsize(p)} bytes'
        print(f'  [{kind}] {name} {size}')


if __name__ == '__main__':
    main()
