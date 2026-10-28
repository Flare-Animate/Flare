#!/usr/bin/env python3
"""Verify every Flash fixture is a valid input for Flare's importer, checking the
exact contract flashimport.cpp / XFLReader rely on for each format.
Run: python verify_fixtures.py  (exit 0 = all pass)
"""
import os, struct, zipfile, sys
import xml.etree.ElementTree as ET

HERE = os.path.dirname(os.path.abspath(__file__))
passed = failed = 0


def check(name, cond, detail=''):
    global passed, failed
    if cond:
        passed += 1; print(f'  PASS  {name}  {detail}')
    else:
        failed += 1; print(f'  FAIL  {name}  {detail}')


def read_swf_dims(data):
    """Mirror readSwfHeader(): FWS/CWS/ZWS sig, RECT decode for uncompressed."""
    if len(data) < 9 or data[0:1] not in (b'F', b'C', b'Z') or data[1:3] != b'WS':
        return None
    if data[0:1] != b'F':
        return ('compressed', data[3])
    first = data[8]; nbits = first >> 3
    bitpos = 8 * 8 + 5
    def rd(n):
        nonlocal bitpos
        v = 0
        for _ in range(n):
            v = (v << 1) | ((data[bitpos // 8] >> (7 - bitpos % 8)) & 1); bitpos += 1
        return v
    def rds(n):
        v = rd(n)
        return v - (1 << n) if v & (1 << (n - 1)) else v
    xmin, xmax, ymin, ymax = rds(nbits), rds(nbits), rds(nbits), rds(nbits)
    return (data[3], (xmax - xmin) // 20, (ymax - ymin) // 20)


def main():
    # SWF
    swf = open(os.path.join(HERE, 'sample.swf'), 'rb').read()
    dims = read_swf_dims(swf)
    check('SWF FWS header + RECT', dims == (5, 550, 400), f'ver/w/h={dims}')

    # FLV
    flv = open(os.path.join(HERE, 'sample.flv'), 'rb').read()
    check('FLV magic + flags', flv[0:3] == b'FLV' and (flv[4] & 0x05) == 0x05,
          f'flags=0x{flv[4]:02x}')

    # F4V
    f4v = open(os.path.join(HERE, 'sample.f4v'), 'rb').read()
    check('F4V ftyp box', f4v[4:8] == b'ftyp' and f4v[8:12] == b'f4v ',
          f'brand={f4v[8:12]!r}')

    # AS
    src = open(os.path.join(HERE, 'sample.as'), encoding='utf-8').read()
    check('AS source readable', 'class Main' in src and 'trace' in src)

    # XFL directory
    dom = os.path.join(HERE, 'sample_xfl', 'DOMDocument.xml')
    check('XFL has DOMDocument.xml', os.path.isfile(dom))
    root = ET.parse(dom).getroot()
    check('XFL DOMDocument attrs', root.get('width') == '550' and
          root.get('height') == '400' and root.get('frameRate') == '24',
          f"{root.get('width')}x{root.get('height')}@{root.get('frameRate')}")

    # FLA (ZIP)
    with zipfile.ZipFile(os.path.join(HERE, 'sample.fla')) as z:
        names = z.namelist()
        check('FLA is ZIP w/ DOMDocument.xml', 'DOMDocument.xml' in names, str(names))
        d2 = ET.fromstring(z.read('DOMDocument.xml'))
        check('FLA DOMDocument parses', d2.get('width') == '550')

    # SWC (ZIP)
    with zipfile.ZipFile(os.path.join(HERE, 'sample.swc')) as z:
        names = z.namelist()
        check('SWC has catalog.xml + library.swf',
              'catalog.xml' in names and 'library.swf' in names, str(names))
        cat = ET.fromstring(z.read('catalog.xml'))
        comps = [e for e in cat.iter() if e.tag.endswith('component')]
        check('SWC catalog lists component(s)', len(comps) >= 1, f'{len(comps)} comp')
        lib = read_swf_dims(z.read('library.swf'))
        check('SWC library.swf is valid SWF', lib == (5, 550, 400), f'{lib}')

    print(f'\nResults: {passed} passed, {failed} failed')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
