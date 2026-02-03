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
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from typing import Dict, List, Optional
import zipfile


@dataclass
class XFLSymbol:
    """Represents a symbol in the XFL library."""
    name: str
    item_id: str
    symbol_type: str  # "graphic", "movie clip", "button"
    linkage_class: Optional[str] = None
    linkage_export: bool = False


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
        """Initialize reader with path to XFL file or directory.
        
        Args:
            xfl_path: Path to .xfl file (ZIP) or uncompressed XFL directory
        """
        self.xfl_path = xfl_path
        self.is_zip = xfl_path.lower().endswith('.xfl') or xfl_path.lower().endswith('.fla')
        self.document = XFLDocument()
        
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
        doc_path = os.path.join(self.xfl_path, 'DOMDocument.xml')
        if os.path.exists(doc_path):
            with open(doc_path, 'rb') as f:
                self._parse_document(f)
        
        # Parse library
        lib_dir = os.path.join(self.xfl_path, 'LIBRARY')
        if os.path.isdir(lib_dir):
            for root, _, files in os.walk(lib_dir):
                for fname in files:
                    if fname.endswith('.xml'):
                        fpath = os.path.join(root, fname)
                        with open(fpath, 'rb') as f:
                            self._parse_symbol(f, fname)
        
        return self.document
    
    def _parse_document(self, file_obj):
        """Parse the main DOMDocument.xml file."""
        try:
            tree = ET.parse(file_obj)
            root = tree.getroot()
            
            # Extract document properties
            if root.tag == 'DOMDocument':
                self.document.width = int(root.get('width', 550))
                self.document.height = int(root.get('height', 400))
                self.document.frame_rate = float(root.get('frameRate', 24.0))
                self.document.background_color = root.get('backgroundColor', '#FFFFFF')
        except ET.ParseError as e:
            print(f"Warning: Failed to parse DOMDocument.xml: {e}")
    
    def _parse_symbol(self, file_obj, symbol_name: str):
        """Parse a library symbol XML file."""
        try:
            tree = ET.parse(file_obj)
            root = tree.getroot()
            
            # Extract symbol information
            if root.tag == 'DOMSymbolItem':
                symbol = XFLSymbol(
                    name=root.get('name', symbol_name),
                    item_id=root.get('itemID', ''),
                    symbol_type=root.get('symbolType', 'graphic')
                )
                
                # Check for linkage (ActionScript export)
                if root.get('linkageClassName'):
                    symbol.linkage_class = root.get('linkageClassName')
                    symbol.linkage_export = root.get('linkageExportForAS', 'false') == 'true'
                
                self.document.symbols.append(symbol)
        except ET.ParseError:
            pass  # Silently skip invalid symbol files


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
        xml = f'''<?xml version="1.0" encoding="utf-8"?>
<DOMDocument xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" currentTimeline="1" xflVersion="2.97" creatorInfo="Flare" platform="Macintosh" versionInfo="Saved by Flare" majorVersion="1" minorVersion="0" buildNumber="0" nextSceneIdentifier="2" playOptionsPlayLoop="false" playOptionsPlayPages="false" playOptionsPlayFrameActions="false" autoSaveHasPrompted="true">
  <symbols/>
  <timelines>
    <DOMTimeline name="Scene 1" currentFrame="0">
      <layers>
        <DOMLayer name="Layer 1" color="#4FFF4F" current="true" isSelected="true">
          <frames>
            <DOMFrame index="0" duration="1" tweenType="none" motionTweenSnap="true" motionTweenSync="false">
              <elements/>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timelines>
  <persistentData/>
  <printSettings/>
  <publishHistory/>
</DOMDocument>'''
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
    
    # Fallback: create basic XFL structure
    # This would require full SWF parsing - placeholder for now
    print("Note: Full SWF->XFL conversion requires JPEXS decompiler or advanced SWF parsing")
    return False


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
