#!/usr/bin/env python3
"""XFL (XML Flash) format reader and writer.

XFL is an XML-based format used by Adobe Animate for FLA project files.
Modern .fla files are actually ZIP archives containing XFL structure.

This module provides basic XFL parsing and generation capabilities
based on the format used by Adobe Animate and documented in the
Adobe XFL specification.

References:
- Adobe XFL format specification
- jpexs-decompiler XFLConverter implementation
- openfl/swf library structures
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from typing import Dict, List, Optional
import zipfile

# Use defusedxml when available to guard against XML bomb / XXE attacks in
# untrusted FLA/XFL files.  Falls back to the stdlib implementation if the
# package is not installed (e.g. minimal CI environments).
try:
    import defusedxml.ElementTree as ET
except ImportError:
    import xml.etree.ElementTree as ET  # type: ignore[no-redef]  # noqa: S405


@dataclass
class XFLFilter:
    """Represents a visual filter applied to a symbol instance (discussion #26)."""
    filter_type: str                # e.g. "DropShadowFilter", "BlurFilter", "GlowFilter"
    params: dict = None

    def __post_init__(self):
        if self.params is None:
            self.params = {}


@dataclass
class XFLSymbol:
    """Represents a symbol in the XFL library."""
    name: str
    item_id: str
    symbol_type: str  # "graphic", "movie clip", "button"
    linkage_class: Optional[str] = None
    linkage_export: bool = False
    # Blend mode support (discussion #26): graphic symbols can now carry blend
    # mode metadata, matching how movie clip instances work in Adobe Animate.
    blend_mode: Optional[str] = None   # e.g. "normal", "multiply", "screen", "overlay"
    filters: List[XFLFilter] = None

    def __post_init__(self):
        if self.filters is None:
            self.filters = []


@dataclass
class XFLDocument:
    """Represents the main XFL document structure."""
    width: int = 550
    height: int = 400
    frame_rate: float = 24.0
    background_color: str = "#FFFFFF"
    symbols: List[XFLSymbol] = None

    def __post_init__(self):
        if self.symbols is None:
            self.symbols = []


class XFLReader:
    """Read and parse XFL format files."""
    
    def __init__(self, xfl_path: str):
        """Initialize reader with path to an XFL/FLA file or directory.

        Args:
            xfl_path: One of
              * a ZIP-based ``.fla``/``.xfl`` archive,
              * a directory-based XFL project folder, or
              * the tiny ``<name>.xfl`` *marker* file that sits inside such a
                folder (Adobe Animate "Save as XFL" writes one). The marker is
                NOT a container — the real project root is its parent directory.
        """
        self.xfl_path = xfl_path
        self.document = XFLDocument()

        # Detect a real ZIP archive by its magic bytes, not just the extension —
        # a directory-based ``.xfl`` marker is a plain text file, not a ZIP.
        self.is_zip = self._looks_like_zip(xfl_path)

        # If handed the ``.xfl`` marker file of a directory-based project, use the
        # folder it lives in as the project root.
        if (not self.is_zip and os.path.isfile(xfl_path)
                and xfl_path.lower().endswith('.xfl')):
            self.xfl_path = os.path.dirname(os.path.abspath(xfl_path))

    # Valid ZIP magic numbers: local file header (normal case), end-of-central-
    # directory record (a valid but empty archive), and the spanned/split
    # archive marker. Checking only PK\x03\x04 would misclassify the latter two
    # as "not zip" and send a legitimate ZIP-based FLA/XFL down the wrong path.
    _ZIP_SIGNATURES = (b'PK\x03\x04', b'PK\x05\x06', b'PK\x07\x08')

    @staticmethod
    def _looks_like_zip(path: str) -> bool:
        """Return True if *path* is a file beginning with a real ZIP signature.

        Checks the full 4-byte signature rather than just the 2-byte 'PK'
        prefix, so a plain-text XFL marker file that happens to start with
        those two characters isn't misclassified as a ZIP and sent down the
        _read_from_zip() path.
        """
        if not os.path.isfile(path):
            return False
        try:
            with open(path, 'rb') as f:
                header = f.read(4)
                return header in XFLReader._ZIP_SIGNATURES
        except OSError:
            return False
        
    def read(self) -> XFLDocument:
        """Read and parse the XFL structure.
        
        Returns:
            XFLDocument containing parsed structure
        """
        if self.is_zip:
            return self._read_from_zip()
        else:
            return self._read_from_directory()
    
    def _read_from_zip(self) -> XFLDocument:
        """Read XFL from a ZIP archive (modern FLA format)."""
        with zipfile.ZipFile(self.xfl_path, 'r') as zf:
            # Look for DOMDocument.xml (main document)
            if 'DOMDocument.xml' in zf.namelist():
                with zf.open('DOMDocument.xml') as f:
                    self._parse_document(f)
            
            # Parse library symbols
            for name in zf.namelist():
                if name.startswith('LIBRARY/') and name.endswith('.xml'):
                    with zf.open(name) as f:
                        self._parse_symbol(f, name)
        
        return self.document
    
    def _read_from_directory(self) -> XFLDocument:
        """Read XFL from an uncompressed directory."""
        base = self.xfl_path
        # If we still hold a file path (e.g. a ``.xfl`` marker), the project root
        # is the directory that contains it.
        if os.path.isfile(base):
            base = os.path.dirname(os.path.abspath(base))
        doc_path = os.path.join(base, 'DOMDocument.xml')
        if os.path.exists(doc_path):
            with open(doc_path, 'rb') as f:
                self._parse_document(f)
        
        # Parse library
        lib_dir = os.path.join(base, 'LIBRARY')
        if os.path.isdir(lib_dir):
            for root, _, files in os.walk(lib_dir):
                for fname in files:
                    if fname.endswith('.xml'):
                        fpath = os.path.join(root, fname)
                        with open(fpath, 'rb') as f:
                            self._parse_symbol(f, fname)
        
        return self.document
    
    @staticmethod
    def _local_tag(tag: str) -> str:
        """Strip the XML namespace prefix from a tag name.

        Converts ``{http://ns.adobe.com/xfl/2008/}DOMDocument`` to
        ``DOMDocument`` so comparisons work for both namespaced and
        non-namespaced XFL files.
        """
        return tag.split("}")[-1] if "}" in tag else tag

    def _parse_document(self, file_obj):
        """Parse the main DOMDocument.xml file.

        Handles both plain-tag format (DOMDocument) and namespace-qualified
        format ({http://ns.adobe.com/xfl/2008/}DOMDocument) that Adobe Animate
        writes into modern .fla files.
        """
        try:
            tree = ET.parse(file_obj)  # nosec B314 — defusedxml used when available
            root = tree.getroot()

            if self._local_tag(root.tag) == 'DOMDocument':
                self.document.width = int(root.get('width', 550))
                self.document.height = int(root.get('height', 400))
                self.document.frame_rate = float(root.get('frameRate', 24.0))
                self.document.background_color = root.get('backgroundColor', '#FFFFFF')
        except ET.ParseError as e:
            print(f"Warning: Failed to parse DOMDocument.xml: {e}")

    def _parse_symbol(self, file_obj, symbol_name: str):
        """Parse a library symbol XML file.

        Handles both plain-tag and namespace-qualified formats.
        Extracts blend mode and filter data for graphic and movie clip symbols
        (discussion #26: graphic symbols should support blend mode and filters).
        """
        try:
            tree = ET.parse(file_obj)  # nosec B314 — defusedxml used when available
            root = tree.getroot()

            if self._local_tag(root.tag) == 'DOMSymbolItem':
                symbol = XFLSymbol(
                    name=root.get('name', symbol_name),
                    item_id=root.get('itemID', ''),
                    symbol_type=root.get('symbolType', 'graphic')
                )

                # Check for linkage (ActionScript export)
                if root.get('linkageClassName'):
                    symbol.linkage_class = root.get('linkageClassName')
                    symbol.linkage_export = root.get('linkageExportForAS', 'false') == 'true'

                # Extract blend mode and filters from the symbol's first instance
                # (discussion #26 — graphic symbols should carry blend mode/filter metadata
                # just like movie clip instances in Adobe Animate)
                self._extract_symbol_fx(root, symbol)

                self.document.symbols.append(symbol)
        except ET.ParseError:
            pass  # Silently skip invalid symbol files

    def _extract_symbol_fx(self, root: ET.Element, symbol: "XFLSymbol") -> None:
        """Extract blend mode and filter metadata from a symbol's timeline frames.

        Walks the symbol's layer/frame/element tree and captures the blend mode
        and filter list from the *first* DOMSymbolInstance or DOMShape element
        found.  This mirrors how Adobe Animate stores per-instance effects inside
        a graphic symbol's timeline (discussion #26).
        """
        ns_prefix = ""
        if "}" in root.tag:
            ns_prefix = root.tag.split("}")[0] + "}"

        _instance_tags = {"DOMSymbolInstance", "DOMShape",
                          "DOMBitmapInstance", "DOMStaticText"}

        # Walk timeline → layers → frames → (elements container) → instances
        for timeline in root.iter(f"{ns_prefix}DOMTimeline"):
            for layer in timeline.iter(f"{ns_prefix}DOMLayer"):
                for frame in layer.iter(f"{ns_prefix}DOMFrame"):
                    # Iterate all descendants of the frame to find instances
                    for elem in frame.iter():
                        if elem is frame:
                            continue
                        local = self._local_tag(elem.tag)
                        if local in _instance_tags:
                            # Blend mode
                            if elem.get("blendMode"):
                                symbol.blend_mode = elem.get("blendMode")

                            # Filters (DOMDropShadowFilter, DOMBlurFilter, etc.)
                            for child in elem:
                                if self._local_tag(child.tag) == "filters":
                                    for flt in child:
                                        ftype = self._local_tag(flt.tag)
                                        params = dict(flt.attrib)
                                        symbol.filters.append(XFLFilter(
                                            filter_type=ftype, params=params
                                        ))
                            return  # Use first element only


class XFLWriter:
    """Write XFL format files."""
    
    def __init__(self, output_path: str, document: XFLDocument):
        """Initialize writer.
        
        Args:
            output_path: Path where XFL should be written (.xfl for ZIP, directory for uncompressed)
            document: XFLDocument to write
        """
        self.output_path = output_path
        self.document = document
        self.is_zip = output_path.lower().endswith('.xfl') or output_path.lower().endswith('.fla')
    
    def write(self):
        """Write the XFL structure to disk."""
        if self.is_zip:
            self._write_as_zip()
        else:
            self._write_as_directory()
    
    def _write_as_zip(self):
        """Write as a ZIP-based XFL/FLA file."""
        with zipfile.ZipFile(self.output_path, 'w', zipfile.ZIP_DEFLATED) as zf:
            # Write main document
            doc_xml = self._generate_document_xml()
            zf.writestr('DOMDocument.xml', doc_xml)
            
            # Write PublishSettings
            pub_xml = self._generate_publish_settings()
            zf.writestr('PublishSettings.xml', pub_xml)
            
            # Write symbols
            for symbol in self.document.symbols:
                symbol_xml = self._generate_symbol_xml(symbol)
                symbol_path = f'LIBRARY/{symbol.name}.xml'
                zf.writestr(symbol_path, symbol_xml)
    
    def _write_as_directory(self):
        """Write as an uncompressed XFL directory."""
        os.makedirs(self.output_path, exist_ok=True)
        
        # Write main document
        doc_path = os.path.join(self.output_path, 'DOMDocument.xml')
        with open(doc_path, 'w', encoding='utf-8') as f:
            f.write(self._generate_document_xml())
        
        # Write PublishSettings
        pub_path = os.path.join(self.output_path, 'PublishSettings.xml')
        with open(pub_path, 'w', encoding='utf-8') as f:
            f.write(self._generate_publish_settings())
        
        # Write library
        lib_dir = os.path.join(self.output_path, 'LIBRARY')
        os.makedirs(lib_dir, exist_ok=True)
        
        for symbol in self.document.symbols:
            symbol_path = os.path.join(lib_dir, f'{symbol.name}.xml')
            with open(symbol_path, 'w', encoding='utf-8') as f:
                f.write(self._generate_symbol_xml(symbol))
    
    def _generate_document_xml(self) -> str:
        """Generate the main DOMDocument.xml content."""
        xml = (
            '<?xml version="1.0" encoding="utf-8"?>\n'
            '<DOMDocument'
            ' xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"'
            ' xmlns="http://ns.adobe.com/xfl/2008/"'
            f' width="{self.document.width}"'
            f' height="{self.document.height}"'
            f' frameRate="{self.document.frame_rate}"'
            f' backgroundColor="{self.document.background_color}"'
            ' currentTimeline="1" xflVersion="2.97" creatorInfo="Flare"'
            ' platform="Macintosh" versionInfo="Saved by Flare"'
            ' majorVersion="1" minorVersion="0" buildNumber="0"'
            ' nextSceneIdentifier="2">\n'
            '  <symbols/>\n'
            '  <timelines>\n'
            '    <DOMTimeline name="Scene 1" currentFrame="0">\n'
            '      <layers>\n'
            '        <DOMLayer name="Layer 1" color="#4FFF4F"'
            ' current="true" isSelected="true">\n'
            '          <frames>\n'
            '            <DOMFrame index="0" duration="1"'
            ' tweenType="none">\n'
            '              <elements/>\n'
            '            </DOMFrame>\n'
            '          </frames>\n'
            '        </DOMLayer>\n'
            '      </layers>\n'
            '    </DOMTimeline>\n'
            '  </timelines>\n'
            '</DOMDocument>\n'
        )
        return xml
    
    def _generate_publish_settings(self) -> str:
        """Generate PublishSettings.xml content."""
        xml = '''<?xml version="1.0" encoding="utf-8"?>
<publishSettings xmlns="http://ns.adobe.com/xfl/2008/">
  <PublishFormatProperties enabled="true">
    <defaultNames>1</defaultNames>
    <flashDefaultName>1</flashDefaultName>
    <projectorWinDefaultName>1</projectorWinDefaultName>
    <projectorMacDefaultName>1</projectorMacDefaultName>
    <htmlDefaultName>1</htmlDefaultName>
    <gifDefaultName>1</gifDefaultName>
    <jpegDefaultName>1</jpegDefaultName>
    <pngDefaultName>1</pngDefaultName>
    <qtDefaultName>1</qtDefaultName>
    <rnwkDefaultName>1</rnwkDefaultName>
  </PublishFormatProperties>
</publishSettings>'''
        return xml
    
    def _generate_symbol_xml(self, symbol: XFLSymbol) -> str:
        """Generate XML for a library symbol."""
        linkage_attrs = ''
        if symbol.linkage_export and symbol.linkage_class:
            linkage_attrs = f' linkageExportForAS="true" linkageClassName="{symbol.linkage_class}"'
        
        xml = f'''<?xml version="1.0" encoding="utf-8"?>
<DOMSymbolItem xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" name="{symbol.name}" itemID="{symbol.item_id}" symbolType="{symbol.symbol_type}"{linkage_attrs}>
  <timeline>
    <DOMTimeline name="{symbol.name}">
      <layers>
        <DOMLayer name="Layer 1" color="#4FFF4F">
          <frames>
            <DOMFrame index="0" duration="1">
              <elements/>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timeline>
</DOMSymbolItem>'''
        return xml


def read_swf_header(swf_path: str) -> Optional[Dict]:
    """Read a SWF file header.

    Returns a dict with ``version``, ``compression`` ('none' | 'zlib' | 'lzma'),
    ``width``/``height`` in pixels, ``frame_rate`` and ``frame_count``, or None
    if the file is not a SWF. ``background_color`` comes from the
    SetBackgroundColor tag when one is present, else Flash's white default.

    The stage RECT is a bit-packed FIXED-length field in twips (1/20 px):
    5 bits giving the width of each of the following four fields, then xmin,
    xmax, ymin, ymax.  Frame rate is an 8.8 fixed-point value.
    """
    try:
        with open(swf_path, 'rb') as f:
            head = f.read(8)
            if len(head) < 8:
                return None
            signature = head[:3]
            if signature not in (b'FWS', b'CWS', b'ZWS'):
                return None
            version = head[3]
            body = f.read()
    except OSError:
        return None

    if signature == b'CWS':
        compression = 'zlib'
        try:
            import zlib
            body = zlib.decompress(body)
        except Exception:
            return None
    elif signature == b'ZWS':
        # LZMA-compressed SWF uses a non-standard container; callers decide how
        # to handle it rather than guessing at a decode here.
        return {'version': version, 'compression': 'lzma', 'width': 0,
                'height': 0, 'frame_rate': 0.0, 'frame_count': 0,
                'background_color': '#FFFFFF'}
    else:
        compression = 'none'

    if not body:
        return None

    nbits = body[0] >> 3
    total_bits = 5 + nbits * 4
    rect_len = (total_bits + 7) // 8
    if len(body) < rect_len + 4:
        return None

    bits = ''.join(f'{byte:08b}' for byte in body[:rect_len])

    def _signed(field: str) -> int:
        """Decode one SB[nbits] field: two's complement, most significant bit
        is the sign. SWF stage bounds are signed, and xmin/ymin are legitimately
        negative in files whose stage origin is not the top-left corner."""
        if not field:
            return 0
        value = int(field, 2)
        if field[0] == '1':
            value -= 1 << len(field)
        return value

    fields = [_signed(bits[5 + i * nbits:5 + (i + 1) * nbits])
              for i in range(4)]
    xmin, xmax, ymin, ymax = fields
    width = round((xmax - xmin) / 20)
    height = round((ymax - ymin) / 20)

    frame_rate = body[rect_len + 1] + body[rect_len] / 256.0
    frame_count = int.from_bytes(body[rect_len + 2:rect_len + 4], 'little')

    background_color = '#FFFFFF'
    pos = rect_len + 4
    while pos + 2 <= len(body):
        code_and_length = int.from_bytes(body[pos:pos + 2], 'little')
        tag_type = code_and_length >> 6
        tag_length = code_and_length & 0x3F
        pos += 2
        if tag_length == 0x3F:
            if pos + 4 > len(body):
                break
            tag_length = int.from_bytes(body[pos:pos + 4], 'little')
            pos += 4
        if tag_type == 0:  # End
            break
        if tag_type == 9 and tag_length >= 3:  # SetBackgroundColor
            r, g, b = body[pos], body[pos + 1], body[pos + 2]
            background_color = f'#{r:02X}{g:02X}{b:02X}'
            break
        pos += tag_length

    return {'version': version, 'compression': compression, 'width': width,
            'height': height, 'frame_rate': round(frame_rate, 3),
            'frame_count': frame_count, 'background_color': background_color}


def convert_swf_to_xfl(swf_path: str, xfl_output: str, use_jpexs: bool = True) -> bool:
    """Convert a SWF file to XFL format.
    
    This is a high-level function that can use either:
    1. JPEXS decompiler (if available and use_jpexs=True)
    2. Basic conversion using openfl/swf-like parsing (future enhancement)
    
    Args:
        swf_path: Path to input SWF file
        xfl_output: Path for output XFL (directory or .xfl file)
        use_jpexs: Whether to attempt using JPEXS decompiler
    
    Returns:
        True if conversion succeeded, False otherwise
    """
    if use_jpexs:
        # Try to use JPEXS if available
        try:
            import subprocess
            # Check if jpexs/ffdec is available
            result = subprocess.run(['ffdec', '-help'], capture_output=True, timeout=5)
            if result.returncode == 0:
                # Use JPEXS to export to XFL
                cmd = ['ffdec', '-export', 'fla', xfl_output, swf_path]
                result = subprocess.run(cmd, capture_output=True, timeout=60)
                return result.returncode == 0
        except (FileNotFoundError, subprocess.TimeoutExpired):
            pass
    
    # Fallback: no decompiler available. Rebuilding shapes, symbols and
    # timelines from SWF tags is JPEXS' job; what we can always do without it is
    # read the SWF header and emit a valid, openable XFL whose document
    # properties (stage size, frame rate, background colour) match the source,
    # so the artwork can be re-imported into a correctly configured stage.
    header = read_swf_header(swf_path)
    if header is None:
        print(f"Error: {swf_path} is not a readable SWF file")
        return False
    if header['compression'] == 'lzma':
        print("Error: LZMA-compressed SWF (ZWS) is not supported by this "
              "fallback; install JPEXS/ffdec to convert it")
        return False

    document = XFLDocument(width=header['width'], height=header['height'],
                           frame_rate=header['frame_rate'],
                           background_color=header['background_color'])
    XFLWriter(xfl_output, document).write()
    print(f"Note: wrote stage-only XFL ({header['width']}x{header['height']} @ "
          f"{header['frame_rate']}fps, {header['frame_count']} frame(s)). "
          "Install JPEXS/ffdec to also convert shapes, symbols and timelines.")
    return True


if __name__ == '__main__':
    import argparse
    
    parser = argparse.ArgumentParser(description='XFL format reader/writer utility')
    parser.add_argument('--read', help='Read and display XFL structure')
    parser.add_argument('--write', help='Create a basic XFL structure')
    parser.add_argument('--convert-swf', help='Convert SWF to XFL (requires JPEXS)')
    parser.add_argument('--output', help='Output path for write/convert operations')
    
    args = parser.parse_args()
    
    if args.read:
        reader = XFLReader(args.read)
        doc = reader.read()
        print(f"XFL Document: {doc.width}x{doc.height} @ {doc.frame_rate}fps")
        print(f"Background: {doc.background_color}")
        print(f"Symbols: {len(doc.symbols)}")
        for sym in doc.symbols:
            print(f"  - {sym.name} ({sym.symbol_type})")
            if sym.linkage_export:
                print(f"    Exported as: {sym.linkage_class}")
    
    elif args.write and args.output:
        doc = XFLDocument()
        writer = XFLWriter(args.output, doc)
        writer.write()
        print(f"Created XFL structure at: {args.output}")
    
    elif args.convert_swf and args.output:
        success = convert_swf_to_xfl(args.convert_swf, args.output)
        if success:
            print(f"Converted {args.convert_swf} to {args.output}")
        else:
            print("Conversion failed - ensure JPEXS (ffdec) is installed")
    
    else:
        parser.print_help()
