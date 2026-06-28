#!/usr/bin/env python3
"""Generate minimal-but-valid sample files for every Flash/Animate format Flare
imports. These are deterministic test fixtures for the Flash import pipeline
(flashimport.cpp / XFLReader). Run: python generate_fixtures.py

Formats covered: FLA, XFL, SWF (uncompressed FWS), SWC, FLV, F4V, AS.
"""
import os, struct, zipfile, shutil

HERE = os.path.dirname(os.path.abspath(__file__))


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
        z.writestr('DOMDocument.xml', DOMDOCUMENT)
        z.writestr('LIBRARY/', '')

    # SWC: ZIP with catalog.xml + library.swf
    with zipfile.ZipFile(os.path.join(HERE, 'sample.swc'), 'w', zipfile.ZIP_DEFLATED) as z:
        z.writestr('catalog.xml', CATALOG_XML)
        z.writestr('library.swf', swf)

    print('Generated fixtures in', HERE)
    for name in sorted(os.listdir(HERE)):
        p = os.path.join(HERE, name)
        kind = 'dir ' if os.path.isdir(p) else 'file'
        size = '' if os.path.isdir(p) else f'{os.path.getsize(p)} bytes'
        print(f'  [{kind}] {name} {size}')


if __name__ == '__main__':
    main()
