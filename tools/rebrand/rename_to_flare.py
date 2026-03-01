#!/usr/bin/env python3
"""
Comprehensive rebrand: Replace all 'Flare' and user-visible 'Toonz'
references with 'Flare' across the entire codebase.

Safe replacements (text content only, no directory/file renames):
- "Flare" -> "Flare"  
- "opentoonz" -> "flare" (in URLs, paths)
- "FLARE" -> "FLARE"
- User-visible "Toonz" -> "Flare" (menus, dialogs, splash, docs)
- GitHub URLs: opentoonz/opentoonz -> Flare-Animate/Flare
- env vars: FLAREROOT -> FLAREROOT, TOONZ prefix -> FLARE prefix

Skipped:
- build/ directory
- thirdparty/ directory  
- Binary files (.exe, .dll, .lib, .obj, .pdb, .png, .ico, .icns)
- Internal C++ identifiers (class names like TToonzImage, ToonzScene - too risky)
- Internal library names (toonzlib, toonzqt - require directory renames)
"""

import os
import re
import sys
from pathlib import Path

REPO_ROOT = Path(r"C:\Users\charl\Documents\Flare")

# Directories to skip entirely
SKIP_DIRS = {
    'build', '.git', 'thirdparty', 'node_modules', '__pycache__',
}

# File extensions to skip (binary files)
SKIP_EXTS = {
    '.exe', '.dll', '.lib', '.obj', '.pdb', '.png', '.jpg', '.jpeg',
    '.gif', '.bmp', '.ico', '.icns', '.tif', '.tiff', '.wav', '.mp3',
    '.zip', '.gz', '.tar', '.7z', '.rar', '.pdf', '.doc', '.docx',
    '.a', '.o', '.so', '.dylib', '.class', '.pyc', '.pyo',
    '.sln', '.vcxproj', '.filters',  # generated build files
}

# Specific files to skip (license/attribution files that should keep historical refs)
SKIP_FILES = {
    'LICENSE_dwango.txt',  # Legal attribution must stay as-is
}

# Track changes
changes_made = []

def should_skip(filepath: Path) -> bool:
    """Check if file should be skipped."""
    # Skip directories
    for part in filepath.parts:
        if part in SKIP_DIRS:
            return True
    # Skip binary extensions
    if filepath.suffix.lower() in SKIP_EXTS:
        return True
    # Skip specific files
    if filepath.name in SKIP_FILES:
        return True
    return False


def replace_in_file(filepath: Path, replacements: list) -> int:
    """Apply a list of (pattern, replacement) to a file. Returns count of changes."""
    try:
        with open(filepath, 'rb') as f:
            raw = f.read()
        # Skip binary files
        if b'\x00' in raw[:8192]:
            return 0
        content = raw.decode('utf-8', errors='replace')
    except (UnicodeDecodeError, PermissionError, OSError):
        return 0
    
    original = content
    count = 0
    for pattern, replacement in replacements:
        new_content = re.sub(pattern, replacement, content)
        if new_content != content:
            count += len(re.findall(pattern, content))
            content = new_content
    
    if content != original:
        try:
            filepath.write_text(content, encoding='utf-8')
            changes_made.append((str(filepath.relative_to(REPO_ROOT)), count))
        except (PermissionError, OSError) as e:
            print(f"  ERROR writing {filepath}: {e}")
            return 0
    
    return count


def get_replacements_for_file(filepath: Path) -> list:
    """Get the list of replacements appropriate for this file type."""
    rel = str(filepath.relative_to(REPO_ROOT)).replace('\\', '/')
    name = filepath.name
    ext = filepath.suffix.lower()
    
    replacements = []
    
    # === GITHUB URLs ===
    # opentoonz/opentoonz -> Flare-Animate/Flare
    replacements.append((
        r'github\.com/opentoonz/opentoonz',
        'github.com/Flare-Animate/Flare'
    ))
    # flare-animate.github.io -> flare-animate.github.io (or remove update check)
    replacements.append((
        r'opentoonz\.github\.io',
        'flare-animate.github.io'
    ))
    # flare-animate.readthedocs.io -> flare-animate.readthedocs.io
    replacements.append((
        r'opentoonz\.readthedocs\.io',
        'flare-animate.readthedocs.io'
    ))
    # Google groups forum
    replacements.append((
        r'groups\.google\.com/forum/#!forum/opentoonz_en',
        'github.com/Flare-Animate/Flare/discussions'
    ))
    # andeon/opentoonz-thirdparty-libs (keep as-is - it's an external repo)
    # But update references to the opentoonz upstream in sync workflow
    replacements.append((
        r'opentoonz/opentoonz\.git',
        'opentoonz/opentoonz.git'  # Keep upstream as-is (it IS opentoonz)
    ))
    
    # === PRODUCT NAME ===
    # "Flare" -> "Flare" (case-sensitive, whole word in most contexts)
    replacements.append((r'\bOpenToonz\b', 'Flare'))
    # "FLARE" -> "FLARE"  
    replacements.append((r'\bOPENTOONZ\b', 'FLARE'))
    
    # === USER-VISIBLE "Toonz" TEXT ===
    # These are carefully targeted to only change user-visible strings
    
    # Menu items and dialog text
    if ext in ('.cpp', '.h', '.hpp', '.py', '.md', '.txt', '.xml', '.html',
               '.yml', '.yaml', '.bat', '.cmd', '.sh', '.ini', '.qss',
               '.less', '.ts', '.lua', '.nuspec', '.desktop', '.myb',
               '.appdata.xml'):
        
        # "Flare Raster" -> "Flare Raster" (in user-visible menu text)
        replacements.append((r'Flare Raster', 'Flare Raster'))
        # "Flare Vector" -> "Flare Vector"
        replacements.append((r'Flare Vector', 'Flare Vector'))
        # "Flare Lip Sync" -> "Flare Lip Sync"
        replacements.append((r'Flare Lip Sync', 'Flare Lip Sync'))
        
    # === ENVIRONMENT VARIABLES ===
    if ext in ('.cpp', '.h', '.bat', '.cmd', '.sh', '.ini', '.md', '.yml',
               '.py', '.ps1'):
        # FLAREROOT -> FLAREROOT
        replacements.append((r'\bTOONZROOT\b', 'FLAREROOT'))
        # FLAREPROJECTS -> FLAREPROJECTS
        replacements.append((r'\bTOONZPROJECTS\b', 'FLAREPROJECTS'))
        # FLARECACHEROOT -> FLARECACHEROOT  
        replacements.append((r'\bTOONZCACHEROOT\b', 'FLARECACHEROOT'))
        # FLAREPROFILES -> FLAREPROFILES
        replacements.append((r'\bTOONZPROFILES\b', 'FLAREPROFILES'))
        # FLAREFXPRESETS -> FLAREFXPRESETS
        replacements.append((r'\bTOONZFXPRESETS\b', 'FLAREFXPRESETS'))
        # FLARESTUDIOPALETTE -> FLARESTUDIOPALETTE
        replacements.append((r'\bTOONZSTUDIOPALETTE\b', 'FLARESTUDIOPALETTE'))
        # FLARECONFIG -> FLARECONFIG
        replacements.append((r'\bTOONZCONFIG\b', 'FLARECONFIG'))
        # Generic "TOONZ" as env prefix (only in main.cpp context)
        # Be very careful - only match the specific env var prefix pattern

    # === SPLASH SCREEN / ERROR MESSAGES ===
    if name == 'main.cpp' and 'flare/sources' in rel:
        replacements.append((
            r'Initializing Toonz environment',
            'Initializing Flare environment'
        ))
        replacements.append((
            r'restart Toonz!',
            'restart Flare!'
        ))
        replacements.append((
            r'"TOONZ"',
            '"FLARE"'
        ))
        
    if name == 'mainwindow.cpp' and 'flare/sources' in rel:
        replacements.append((
            r'Re-installing Toonz will',
            'Re-installing Flare will'
        ))
    
    # === CI PATHS ===
    if ext in ('.yml', '.yaml'):
        replacements.append((r'/opt/opentoonz', '/opt/flare'))
        replacements.append((r'lib/opentoonz', 'lib/flare'))
    
    # === XDG DATA ===
    if name.endswith('.desktop') or name.endswith('.appdata.xml'):
        replacements.append((
            r'based on the software "Toonz"',
            'based on the software "Toonz" (now called Flare)'
        ))
    
    # === SHORTCUT CONFIG ===
    if name == 'defopentoonz.ini':
        pass  # File will be renamed separately
    
    # === MyPaint brush descriptions ===
    if ext == '.myb':
        replacements.append((
            r'optimized for Flare',
            'optimized for Flare'
        ))
        replacements.append((
            r'Brush package optimized for Flare',
            'Brush package optimized for Flare'
        ))
    
    # === DOCUMENTATION ===
    if ext == '.md':
        replacements.append((
            r'Toonz environment',
            'Flare environment'
        ))
    
    return replacements


def main():
    print("=" * 60)
    print("Flare Rebrand Script")
    print("=" * 60)
    
    total_files = 0
    total_changes = 0
    
    for root, dirs, files in os.walk(REPO_ROOT):
        # Remove skip dirs from traversal
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        
        for filename in files:
            filepath = Path(root) / filename
            
            if should_skip(filepath):
                continue
            
            replacements = get_replacements_for_file(filepath)
            if not replacements:
                continue
            
            count = replace_in_file(filepath, replacements)
            if count > 0:
                total_files += 1
                total_changes += count
    
    print(f"\nTotal files modified: {total_files}")
    print(f"Total replacements: {total_changes}")
    print("\nModified files:")
    for fpath, count in sorted(changes_made):
        print(f"  {fpath}: {count} changes")
    
    return 0


if __name__ == '__main__':
    sys.exit(main())
