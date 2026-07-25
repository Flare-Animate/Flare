"""SWF patcher — read a SWF, replace specific DoABC tags, write a new SWF.

This module supports the "compile edited class" workflow:
  1. read_swf_full()   — parse SWF header + all tags, preserving raw bytes
  2. recompile_class() — high-level: decompile block, replace edited source,
                         compile with mxmlc, patch tag, write new SWF

Only the ABC block(s) containing edited classes are recompiled.
All other tags (shapes, bitmaps, sounds, sprites) are preserved byte-for-byte.
"""

from __future__ import annotations

import logging
import os
import struct
import subprocess
import sys
import tempfile
import threading
import time
import zlib
from typing import Dict, List, Optional, Tuple

log = logging.getLogger(__name__)

# On Windows, suppress console windows for subprocess calls
_NO_WINDOW = {}
if sys.platform == 'win32':
    _NO_WINDOW = {'creationflags': subprocess.CREATE_NO_WINDOW}

# Reuse existing modules
from as3_decompiler.swf_reader import (
    TAG_DOABC, TAG_DOABC2,
    _open_swf_stream, _StreamReader,
)
from as3_decompiler.abc_parser import ABCFile
from as3_decompiler.class_decompiler import AS3Decompiler
from as3_decompiler.abc_patcher import transplant_class, _detect_changed_methods

__all__ = ['read_swf_full', 'write_swf_from_tags',
           'recompile_class', 'recompile_classes']

# SWF spec constants
_SWF_HEADER_SIZE = 8
_RECT_NBITS_SHIFT = 3
_RECT_NBITS_MASK = 0x1F
_RECT_FIELD_COUNT = 4
_FRAME_INFO_SIZE = 4
_TAG_CODE_SHIFT = 6
_TAG_SHORT_LENGTH_MASK = 0x3F
_TAG_LONG_LENGTH_FLAG = 0x3F

# SDK search paths (same as compile_n2d.py)
SDK_SEARCH_PATHS = [
    r"C:\AIRSDK_Compiler",
    r"C:\aflex_sdk",
    r"C:\flex_sdk",
    r"C:\apache-flex-sdk",
]


def find_sdk() -> Optional[str]:
    """Locate a Flex/AIR SDK installation."""
    env = os.environ.get("FLEX_HOME") or os.environ.get("AIR_SDK_HOME")
    if env and os.path.isdir(env):
        return env
    for p in SDK_SEARCH_PATHS:
        if os.path.isdir(p):
            return p
    return None


def find_mxmlc(sdk_path: str) -> str:
    """Find the mxmlc compiler binary inside the SDK."""
    for name in ("mxmlc.bat", "mxmlc"):
        path = os.path.join(sdk_path, "bin", name)
        if os.path.isfile(path):
            return path
    raise FileNotFoundError(f"mxmlc not found in {sdk_path}/bin/")


# ═══════════════════════════════════════════════════════════════════════════
#  SWF reading — preserves header fields + raw tag bodies
# ═══════════════════════════════════════════════════════════════════════════

def read_swf_full(path: str) -> dict:
    """Read a SWF file, returning everything needed to reconstruct it.

    Returns dict with keys:
        version      : int
        rect_bytes   : bytes   (raw RECT field, preserved exactly)
        frame_rate   : int     (raw 16-bit value, 8.8 fixed-point)
        frame_count  : int
        compressed   : bool    (original compression state)
        tags         : list of (tag_type: int, body: bytes)
    """
    log.debug("read_swf_full: %s", path)
    with open(path, 'rb') as f:
        header = f.read(_SWF_HEADER_SIZE)
    sig = header[:3]
    version = header[3]
    compressed = sig in (b'CWS', b'ZWS')

    # Re-open using the streaming reader to handle decompression
    _, stream = _open_swf_stream(path)
    try:
        reader = _StreamReader(stream)

        # Read RECT (variable length)
        first = reader.read(1)
        nbits = (first[0] >> _RECT_NBITS_SHIFT) & _RECT_NBITS_MASK
        total_bits = 5 + nbits * _RECT_FIELD_COUNT
        rect_byte_count = (total_bits + 7) // 8
        if rect_byte_count > 1:
            rect_rest = reader.read(rect_byte_count - 1)
        else:
            rect_rest = b''
        rect_bytes = bytes([first[0]]) + rect_rest

        # Frame info
        frame_info = reader.read(_FRAME_INFO_SIZE)
        frame_rate = struct.unpack('<H', frame_info[:2])[0]
        frame_count = struct.unpack('<H', frame_info[2:4])[0]

        # Read all tags
        tags: List[Tuple[int, bytes]] = []
        while True:
            hdr = reader.read_available(2)
            if len(hdr) < 2:
                break
            tag_code_and_length = struct.unpack('<H', hdr)[0]
            tag_type = tag_code_and_length >> _TAG_CODE_SHIFT
            tag_length = tag_code_and_length & _TAG_SHORT_LENGTH_MASK
            if tag_length == _TAG_LONG_LENGTH_FLAG:
                ext = reader.read_available(4)
                if len(ext) < 4:
                    break
                tag_length = struct.unpack('<I', ext)[0]
            if tag_type == 0:  # End tag
                break
            body = reader.read_available(tag_length)
            tags.append((tag_type, body))
    finally:
        stream.close()

    return {
        'version': version,
        'rect_bytes': rect_bytes,
        'frame_rate': frame_rate,
        'frame_count': frame_count,
        'compressed': compressed,
        'tags': tags,
    }


# ═══════════════════════════════════════════════════════════════════════════
#  SWF writing — from raw tags
# ═══════════════════════════════════════════════════════════════════════════

def _build_tag(tag_id: int, data: bytes, force_long: bool = False) -> bytes:
    """Encode a single SWF tag (header + body)."""
    length = len(data)
    if length < 0x3F and not force_long:
        code = (tag_id << 6) | length
        return struct.pack('<H', code) + data
    else:
        code = (tag_id << 6) | 0x3F
        return struct.pack('<HI', code, length) + data


def write_swf_from_tags(swf_info: dict, output_path: str) -> None:
    """Write a SWF file from the parsed structure (as returned by read_swf_full).

    Preserves the original RECT, frame rate, frame count, and version.
    """
    log.info("write_swf_from_tags: %s (v%d, %d tags)", output_path, swf_info['version'], len(swf_info['tags']))
    version = swf_info['version']
    rect_bytes = swf_info['rect_bytes']
    frame_rate = swf_info['frame_rate']
    frame_count = swf_info['frame_count']
    compressed = swf_info['compressed']
    tags = swf_info['tags']

    # Concatenate all tags + End tag
    tag_data = bytearray()
    for tag_type, body in tags:
        tag_data.extend(_build_tag(tag_type, body))
    tag_data.extend(_build_tag(0, b''))  # End tag

    # Build body: RECT + frame_info + tags
    frame_info = struct.pack('<HH', frame_rate, frame_count)
    body = rect_bytes + frame_info + bytes(tag_data)

    file_length = _SWF_HEADER_SIZE + len(body)

    if compressed:
        sig = b'CWS'
        compressed_body = zlib.compress(body, 9)
        result = sig + struct.pack('<BI', version, file_length) + compressed_body
    else:
        sig = b'FWS'
        result = sig + struct.pack('<BI', version, file_length) + body

    with open(output_path, 'wb') as f:
        f.write(result)


# ═══════════════════════════════════════════════════════════════════════════
#  DoABC tag helpers
# ═══════════════════════════════════════════════════════════════════════════

def _get_abc_block_indices(tags: List[Tuple[int, bytes]]) -> List[int]:
    """Return tag-list indices of DoABC/DoABC2 tags."""
    indices = []
    for i, (tag_type, _) in enumerate(tags):
        if tag_type in (TAG_DOABC, TAG_DOABC2):
            indices.append(i)
    return indices


def _extract_abc_data_from_tag(tag_type: int, body: bytes) -> Tuple[str, bytes]:
    """Extract (name, raw_abc_bytes) from a DoABC/DoABC2 tag body."""
    if tag_type == TAG_DOABC2:
        # flags (4 bytes) + null-terminated name + abc data
        if len(body) < 5:
            raise ValueError("DoABC2 tag body too short")
        null_pos = body.find(b'\x00', 4)
        if null_pos < 0:
            raise ValueError("DoABC2 tag name not null-terminated")
        name = body[4:null_pos].decode('utf-8', errors='replace')
        abc_data = body[null_pos + 1:]
        return (name or '(unnamed)', abc_data)
    else:
        # DoABC — raw ABC data, no flags/name
        return ('DoABC', body)


def _build_doabc2_tag_body(name: str, abc_data: bytes, flags: int = 1) -> bytes:
    """Build a DoABC2 tag body from name + abc data."""
    return struct.pack('<I', flags) + name.encode('utf-8') + b'\x00' + abc_data


def _is_air_sdk(sdk_path: str) -> bool:
    """Check whether the given SDK is an AIR SDK (has airglobal.swc)."""
    air_global = os.path.join(sdk_path, "frameworks", "libs", "air", "airglobal.swc")
    return os.path.isfile(air_global)


# SWF-version → Flash Player version mapping (from Adobe docs)
_SWF_VERSION_TO_PLAYER = {
    9: "9.0", 10: "10.0", 11: "10.1", 12: "10.2", 13: "10.3",
    14: "11.0", 15: "11.1", 16: "11.2", 17: "11.3", 18: "11.4",
    19: "11.5", 20: "11.6", 21: "11.7", 22: "11.8", 23: "11.9",
    24: "12.0", 25: "13.0", 26: "14.0", 27: "15.0", 28: "16.0",
    29: "17.0", 30: "18.0", 31: "19.0", 32: "20.0", 33: "21.0",
    34: "22.0", 35: "23.0", 36: "24.0", 37: "25.0", 38: "26.0",
    39: "27.0", 40: "28.0", 41: "29.0", 42: "30.0", 43: "32.0",
    44: "33.0",
}


def _player_version_for_swf(swf_version: int) -> str:
    """Map SWF file version to the nearest target-player value."""
    if swf_version in _SWF_VERSION_TO_PLAYER:
        return _SWF_VERSION_TO_PLAYER[swf_version]
    # If higher than known, use highest known
    if swf_version > max(_SWF_VERSION_TO_PLAYER):
        return _SWF_VERSION_TO_PLAYER[max(_SWF_VERSION_TO_PLAYER)]
    return "32.0"  # safe default


# ═══════════════════════════════════════════════════════════════════════════
#  mxmlc compilation helpers
# ═══════════════════════════════════════════════════════════════════════════

def _compile_with_mxmlc(
    source_dirs: List[str],
    main_class: str,
    sdk_path: str,
    output_swf: str,
    swf_version: int = 43,
    extra_args: Optional[List[str]] = None,
) -> Tuple[bool, str, str]:
    """Compile AS3 source with mxmlc.

    Returns (success: bool, stdout: str, stderr: str).
    """
    mxmlc = find_mxmlc(sdk_path)
    is_air = _is_air_sdk(sdk_path)
    player_ver = _player_version_for_swf(swf_version)

    cmd = [mxmlc]
    for d in source_dirs:
        cmd.append(f"-source-path+={d}")
    cmd.extend([
        f"-target-player={player_ver}",
        "-static-link-runtime-shared-libraries",
        "-strict=false",
        "-warnings=false",
        "-debug=false",
        f"-output={output_swf}",
    ])

    # AIR SDK: use air config and add airglobal.swc as external library
    if is_air:
        cmd.append("+configname=air")
        air_global = os.path.join(sdk_path, "frameworks", "libs", "air", "airglobal.swc")
        cmd.append(f"-external-library-path+={air_global}")

    if extra_args:
        cmd.extend(extra_args)
    cmd.append(main_class)

    env = os.environ.copy()
    env["FLEX_HOME"] = sdk_path
    if is_air:
        env["AIR_HOME"] = sdk_path
    player_home = os.path.join(sdk_path, "frameworks", "libs", "player")
    env["PLAYERGLOBAL_HOME"] = player_home

    result = subprocess.run(
        cmd, capture_output=True, text=True, env=env,
        timeout=120, **_NO_WINDOW,
    )

    return (result.returncode == 0, result.stdout, result.stderr)


# ═══════════════════════════════════════════════════════════════════════════
#  MxmlcShell — persistent JVM wrapper for fast recompiles (~4-5s warm)
# ═══════════════════════════════════════════════════════════════════════════

# Search paths for a Flex SDK that has mxmlc.jar (needed by MxmlcShell)
_FLEX_SDK_SEARCH = [
    r"C:\aflex_sdk",
    r"C:\flex_sdk",
    r"C:\apache-flex-sdk",
]


def _find_compiler_jar() -> Optional[str]:
    """Locate mxmlc.jar (Flex SDK) or compiler.jar with flex2.tools.Mxmlc.

    MxmlcShell needs a JAR containing ``flex2.tools.Mxmlc`` — this is
    ``mxmlc.jar`` in a Flex SDK, or ``compiler.jar`` in some SDKs.
    The AIR SDK's compiler.jar does NOT have this class, so we check
    Flex SDK paths first.
    """
    # Check Flex SDK paths first (guaranteed to have flex2.tools.Mxmlc)
    for p in _FLEX_SDK_SEARCH:
        jar = os.path.join(p, "lib", "mxmlc.jar")
        if os.path.isfile(jar):
            return jar

    # Check primary SDK
    primary = find_sdk()
    if primary:
        # Flex SDK uses mxmlc.jar, some SDKs have compiler.jar with the class
        for name in ("mxmlc.jar", "compiler.jar"):
            jar = os.path.join(primary, "lib", name)
            if os.path.isfile(jar):
                return jar

    return None


def _ensure_mxmlcshell_class(compiler_jar: str) -> Optional[str]:
    """Compile MxmlcShell.java → .class if not already done.

    Returns path to the directory containing MxmlcShell.class, or None.
    In bundled deployments, MxmlcShell.class is pre-compiled and lives
    next to MxmlcShell.java; in dev mode we compile on-the-fly.
    """
    src_dir = os.path.dirname(os.path.abspath(__file__))
    java_src = os.path.join(src_dir, "MxmlcShell.java")

    # Check for pre-compiled .class next to .java (bundled deployment)
    class_beside = os.path.join(src_dir, "MxmlcShell.class")
    if os.path.isfile(class_beside):
        return src_dir

    # Dev mode: compile to temp dir
    out_dir = os.path.join(tempfile.gettempdir(), "mxmlcshell_class")
    os.makedirs(out_dir, exist_ok=True)
    class_file = os.path.join(out_dir, "MxmlcShell.class")

    if os.path.isfile(class_file):
        if os.path.getmtime(java_src) <= os.path.getmtime(class_file):
            return out_dir

    r = subprocess.run(
        ['javac', '-cp', compiler_jar, '-d', out_dir, java_src],
        capture_output=True, text=True, timeout=30,
        cwd=os.path.dirname(compiler_jar), **_NO_WINDOW,
    )
    if r.returncode != 0:
        return None
    return out_dir


class MxmlcShellManager:
    """Persistent MxmlcShell process manager.

    Keeps a JVM alive running our ``MxmlcShell.java`` wrapper.  The
    first compile (prewarm) takes ~10s, but subsequent compiles reuse
    the warm JVM and finish in ~4-5s.

    Thread-safe: uses a lock for all state mutations.
    """

    _instance: Optional['MxmlcShellManager'] = None

    def __init__(self):
        self._proc: Optional[subprocess.Popen] = None
        self._ready = False
        self._lock = threading.Lock()
        self._source_dir: Optional[str] = None
        self._out_swf: Optional[str] = None
        self._tmp_dir: Optional[str] = None
        self._args_file: Optional[str] = None
        self._log_lines: List[str] = []

    @classmethod
    def get(cls) -> 'MxmlcShellManager':
        if cls._instance is None:
            cls._instance = MxmlcShellManager()
        return cls._instance

    def _log(self, msg: str):
        self._log_lines.append(msg)

    def _read_until_sentinel(self, timeout: float = 180) -> Tuple[bool, str]:
        """Read stdout lines until ``__DONE__`` or ``__ERROR__``.

        Returns ``(success, all_output)``.
        """
        lines: List[str] = []
        deadline = time.time() + timeout
        while time.time() < deadline:
            line = self._proc.stdout.readline()
            if not line:
                break
            line = line.rstrip('\n\r')
            lines.append(line)
            if line.startswith('__DONE__'):
                return (True, '\n'.join(lines))
            if line.startswith('__ERROR__'):
                return (False, '\n'.join(lines))
        return (False, '\n'.join(lines))

    def prewarm(
        self,
        source_dir: str,
        main_file: str,
        out_swf: str,
        cfg_path: str,
        sdk_path: str,
        swf_version: int = 43,
    ) -> bool:
        """Start MxmlcShell JVM and do an initial compile.

        This is expensive (~10s) but runs in a background thread.
        After prewarm, :meth:`compile` finishes in ~4-5s.
        """
        with self._lock:
            self._kill_unlocked()
            self._log_lines.clear()

            compiler_jar = _find_compiler_jar()
            if not compiler_jar:
                self._log("MxmlcShell: no compiler jar found")
                return False

            shell_dir = _ensure_mxmlcshell_class(compiler_jar)
            if not shell_dir:
                self._log("MxmlcShell: failed to compile MxmlcShell.java")
                return False

            # Determine the SDK root from compiler_jar path
            jar_sdk = os.path.dirname(os.path.dirname(compiler_jar))

            self._log(f"MxmlcShell: starting (jar={compiler_jar})")

            # Build classpath: all JARs in SDK lib + shell dir
            jar_dir = os.path.join(jar_sdk, "lib")
            all_jars = [os.path.join(jar_dir, f) for f in os.listdir(jar_dir)
                        if f.endswith('.jar')]
            # Ensure compiler_jar is first
            if compiler_jar in all_jars:
                all_jars.remove(compiler_jar)
            all_jars.insert(0, compiler_jar)
            cp = ';'.join(all_jars + [shell_dir])

            try:
                cmd = [
                    'java',
                    '-Dsun.io.useCanonCaches=false',
                    '-Xms32m', '-Xmx512m',
                    f'-Dflexlib={jar_sdk}\\frameworks',
                    f'-Dapplication.home={jar_sdk}',
                    '-cp', cp,
                    'MxmlcShell',
                ]
                self._proc = subprocess.Popen(
                    cmd,
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    bufsize=1,
                    **_NO_WINDOW,
                )
            except Exception as e:
                self._log(f"MxmlcShell: failed to start: {e}")
                return False

            # Wait for READY
            line = self._proc.stdout.readline().strip()
            if line != 'READY':
                self._log(f"MxmlcShell: unexpected startup: {line}")
                self._kill_unlocked()
                return False

            self._log("MxmlcShell: JVM started")

            # Build args file
            tmp_dir = os.path.dirname(out_swf)
            args_file = os.path.join(tmp_dir, "mxmlc_args.txt")
            player_ver = _player_version_for_swf(swf_version)
            air_global = os.path.join(sdk_path, "frameworks", "libs", "air", "airglobal.swc")

            args_lines = [
                f'-source-path+={source_dir}',
                f'-target-player={player_ver}',
                '-static-link-runtime-shared-libraries',
                '-strict=false',
                '-warnings=false',
                '-debug=false',
                f'-output={out_swf}',
            ]
            if os.path.isfile(air_global):
                args_lines.append(f'-external-library-path+={air_global}')
            args_lines.extend([
                f'-load-config+={cfg_path}',
                '-incremental=true',
                main_file,
            ])
            with open(args_file, 'w', encoding='utf-8') as f:
                f.write('\n'.join(args_lines) + '\n')

            self._args_file = args_file
            self._tmp_dir = tmp_dir
            self._source_dir = source_dir
            self._out_swf = out_swf

            # Delete old output if exists
            if os.path.isfile(out_swf):
                os.unlink(out_swf)

            # Initial compile (warm up JVM + compiler)
            t0 = time.time()
            self._proc.stdin.write(args_file + '\n')
            self._proc.stdin.flush()
            ok, out = self._read_until_sentinel(timeout=180)

            compile_time = time.time() - t0
            has_swf = os.path.isfile(out_swf)
            self._log(f"MxmlcShell: initial compile {compile_time:.1f}s, "
                       f"swf={'OK' if has_swf else 'MISSING'}, "
                       f"result={'OK' if ok else 'FAIL'}")
            if not has_swf:
                # Log last few output lines for diagnostics
                for line in out.split('\n')[-10:]:
                    if line.strip():
                        self._log(f"  | {line.strip()[:200]}")

            self._ready = has_swf and ok
            return self._ready

    def compile(self) -> Optional[Tuple[bool, str, str]]:
        """Recompile using warm JVM (~4-5s).

        Returns ``(success, stdout, stderr)`` or ``None`` if not ready.
        """
        with self._lock:
            if not self._ready:
                return None
            if not self._proc or self._proc.poll() is not None:
                self._ready = False
                return None

            # Delete old output
            if self._out_swf and os.path.isfile(self._out_swf):
                os.unlink(self._out_swf)

            t0 = time.time()
            try:
                self._proc.stdin.write(self._args_file + '\n')
                self._proc.stdin.flush()
                ok, out = self._read_until_sentinel(timeout=120)
            except (BrokenPipeError, OSError):
                self._ready = False
                return None

            elapsed = time.time() - t0
            has_swf = os.path.isfile(self._out_swf) if self._out_swf else False
            success = has_swf and ok

            self._log(f"MxmlcShell: compile {elapsed:.1f}s, "
                       f"{'OK' if success else 'FAIL'}")
            return (success, out, '')

    @property
    def out_swf(self) -> Optional[str]:
        return self._out_swf

    def is_ready(self) -> bool:
        with self._lock:
            return (self._ready
                    and self._proc is not None
                    and self._proc.poll() is None)

    def kill(self):
        with self._lock:
            self._kill_unlocked()

    def _kill_unlocked(self):
        if self._proc:
            try:
                self._proc.stdin.write('QUIT\n')
                self._proc.stdin.flush()
                self._proc.wait(timeout=3)
            except Exception:
                try:
                    self._proc.kill()
                except Exception:
                    pass
            self._proc = None
            self._ready = False


# ═══════════════════════════════════════════════════════════════════════════
#  Compiler helpers
# ═══════════════════════════════════════════════════════════════════════════

def prewarm_compiler(
    swf_path: str,
    swf_version: int,
    class_info_list: list,
    cached_sources: Dict[int, str],
    sdk_path: Optional[str] = None,
):
    """Write all source files and start FCSH for fast incremental compiles.

    This should be called in a background thread right after indexing.
    It writes all class sources to the persistent work directory, then
    starts the FCSH process and does an initial full compile (~15s).
    After this, :func:`recompile_classes` uses FCSH for ~1-2s compiles.

    Parameters
    ----------
    swf_path : str
        Path to the original SWF.
    swf_version : int
        SWF file version (for target-player mapping).
    class_info_list : list
        List of class info dicts (as returned by ``AS3Decompiler.list_classes``).
    cached_sources : dict
        ``{local_class_index: source_text}`` for all classes.
    sdk_path : str, optional
        Path to AIR/Flex SDK.  Auto-detected if omitted.
    """
    import hashlib

    if not sdk_path:
        sdk_path = find_sdk()
    if not sdk_path:
        return

    # Persistent work directory
    swf_hash = hashlib.md5(os.path.abspath(swf_path).encode()).hexdigest()[:12]
    tmp_dir = os.path.join(tempfile.gettempdir(), f"swf_recompile_{swf_hash}")
    os.makedirs(tmp_dir, exist_ok=True)
    source_dir = os.path.join(tmp_dir, "src")
    os.makedirs(source_dir, exist_ok=True)

    # Write all source files
    main_file = None
    for ci, cls_info in enumerate(class_info_list):
        pkg = cls_info['package'] if isinstance(cls_info, dict) else cls_info.get('package', '')
        name = cls_info['name'] if isinstance(cls_info, dict) else cls_info.get('name', '')

        if pkg:
            pkg_dir = os.path.join(source_dir, pkg.replace('.', os.sep))
            os.makedirs(pkg_dir, exist_ok=True)
            as_path = os.path.join(pkg_dir, f"{name}.as")
        else:
            as_path = os.path.join(source_dir, f"{name}.as")

        source = cached_sources.get(ci, '')
        if not source:
            # Write a stub
            super_cls = cls_info.get('super', 'Object')
            super_short = super_cls.split('.')[-1] if super_cls else 'Object'
            is_iface = cls_info.get('is_interface', False)
            if is_iface:
                source = f"package {pkg} {{\n    public interface {name} {{}}\n}}\n"
            else:
                source = (
                    f"package {pkg} {{\n"
                    f"    public class {name} extends {super_short} {{\n"
                    f"        public function {name}() {{ super(); }}\n"
                    f"    }}\n"
                    f"}}\n"
                )

        if main_file is None:
            main_file = as_path

        # Only write if different (preserve timestamps for incremental)
        need_write = True
        if os.path.isfile(as_path):
            try:
                with open(as_path, 'r', encoding='utf-8') as f:
                    if f.read() == source:
                        need_write = False
            except Exception:
                pass
        if need_write:
            with open(as_path, 'w', encoding='utf-8') as f:
                f.write(source)

    if not main_file:
        return

    # Fix ambiguous names
    _fix_ambiguous_names(source_dir, class_info_list, lambda msg: None)

    # Write includes.cfg with first class
    full_name = class_info_list[0].get('full_name', class_info_list[0].get('name', ''))
    cfg_path = os.path.join(tmp_dir, "includes.cfg")
    with open(cfg_path, 'w', encoding='utf-8') as cfg:
        cfg.write('<flex-config>\n')
        cfg.write('  <includes append="true">\n')
        cfg.write(f'    <symbol>{full_name}</symbol>\n')
        cfg.write('  </includes>\n')
        cfg.write('</flex-config>\n')

    out_swf = os.path.join(tmp_dir, "compiled.swf")

    # Start MxmlcShell and do initial compile (warm JVM)
    mgr = MxmlcShellManager.get()
    mgr.prewarm(
        source_dir=source_dir,
        main_file=main_file,
        out_swf=out_swf,
        cfg_path=cfg_path,
        sdk_path=sdk_path,
        swf_version=swf_version,
    )


def _compile_fast(
    source_dirs: List[str],
    main_class: str,
    sdk_path: str,
    output_swf: str,
    swf_version: int = 43,
    extra_args: Optional[List[str]] = None,
) -> Tuple[bool, str, str]:
    """Compile AS3 source with mxmlc (direct invocation).

    Uses persistent work directories + ``-incremental=true`` so that
    subsequent compiles reuse mxmlc's type-check cache (~40 % faster).
    """
    return _compile_with_mxmlc(
        source_dirs, main_class, sdk_path, output_swf,
        swf_version, extra_args,
    )


def _extract_doabc_from_swf(swf_path: str) -> List[Tuple[int, bytes]]:
    """Extract all DoABC/DoABC2 tag bodies from a compiled SWF.

    Returns list of (tag_type, full_tag_body) tuples.
    """
    info = read_swf_full(swf_path)
    result = []
    for tag_type, body in info['tags']:
        if tag_type in (TAG_DOABC, TAG_DOABC2):
            result.append((tag_type, body))
    return result


# ═══════════════════════════════════════════════════════════════════════════
#  Ambiguous name resolution
# ═══════════════════════════════════════════════════════════════════════════

# Well-known flash.* simple names that often get shadowed by user classes.
# When a decompiled class in the same package has the same simple name,
# other files in that package can't use "Dictionary" unqualified.
_FLASH_BUILTINS = {
    'Dictionary':       'flash.utils.Dictionary',
    'Proxy':            'flash.utils.Proxy',
    'Timer':            'flash.utils.Timer',
    'ByteArray':        'flash.utils.ByteArray',
    'Endian':           'flash.utils.Endian',
    'IExternalizable':  'flash.utils.IExternalizable',
    'IDataInput':       'flash.utils.IDataInput',
    'IDataOutput':      'flash.utils.IDataOutput',
    'Sprite':           'flash.display.Sprite',
    'MovieClip':        'flash.display.MovieClip',
    'Bitmap':           'flash.display.Bitmap',
    'BitmapData':       'flash.display.BitmapData',
    'TextField':        'flash.text.TextField',
    'TextFormat':       'flash.text.TextFormat',
    'Event':            'flash.events.Event',
    'EventDispatcher':  'flash.events.EventDispatcher',
    'MouseEvent':       'flash.events.MouseEvent',
    'KeyboardEvent':    'flash.events.KeyboardEvent',
    'Sound':            'flash.media.Sound',
    'SoundChannel':     'flash.media.SoundChannel',
    'SoundTransform':   'flash.media.SoundTransform',
    'Matrix':           'flash.geom.Matrix',
    'Point':            'flash.geom.Point',
    'Rectangle':        'flash.geom.Rectangle',
    'ColorTransform':   'flash.geom.ColorTransform',
    'Loader':           'flash.display.Loader',
    'URLRequest':       'flash.net.URLRequest',
    'URLLoader':        'flash.net.URLLoader',
    'SharedObject':     'flash.net.SharedObject',
    'Socket':           'flash.net.Socket',
    'XML':              'XML',  # top-level, no import needed
    'XMLList':          'XMLList',
}


def _fix_ambiguous_names(source_dir: str, class_list: list, _log) -> None:
    """Post-process decompiled source to disambiguate shadowed names.

    If the decompiled classes include e.g. com.mcleodgaming.ssf2.util.Dictionary,
    any .as file in that same package that uses 'Dictionary' but means
    flash.utils.Dictionary will fail to compile.  Simply adding an import
    makes it WORSE (both local class and import are visible).

    Instead, this function replaces bare references to the shadowed name
    with the fully-qualified flash.* name in the code body, e.g.
    ``Dictionary`` → ``flash.utils.Dictionary``.  Import lines, comments,
    and the shadow class file itself are left untouched.
    """
    import re

    # Build set of (simple_name, package) for classes that shadow flash builtins
    shadows = {}  # simple_name -> (flash_fqn, shadow_pkg)
    for cls_info in class_list:
        simple = cls_info['name']
        pkg = cls_info['package']
        if simple in _FLASH_BUILTINS and _FLASH_BUILTINS[simple] not in ('XML', 'XMLList'):
            flash_fqn = _FLASH_BUILTINS[simple]
            shadows[simple] = (flash_fqn, pkg)

    if not shadows:
        return

    _log(f"Ambiguous names detected: {', '.join(shadows.keys())}")

    fixes = 0
    for simple_name, (flash_fqn, shadow_pkg) in shadows.items():
        pkg_dir = os.path.join(source_dir, shadow_pkg.replace('.', os.sep))
        if not os.path.isdir(pkg_dir):
            continue

        # Match bare name as a whole word, but NOT preceded by a dot
        # (avoids double-qualifying already-qualified refs like flash.utils.Dictionary)
        name_re = re.compile(r'(?<!\.)\b' + re.escape(simple_name) + r'\b')

        for fname in os.listdir(pkg_dir):
            if not fname.endswith('.as'):
                continue
            # Don't touch the shadow class itself
            if fname == f"{simple_name}.as":
                continue

            fpath = os.path.join(pkg_dir, fname)
            try:
                with open(fpath, 'r', encoding='utf-8') as f:
                    content = f.read()
            except Exception:
                continue

            # Quick check: does this file even use the name?
            if not name_re.search(content):
                continue

            # Process line by line – skip import / comment lines
            lines = content.split('\n')
            changed = False
            for i, line in enumerate(lines):
                stripped = line.strip()
                # Skip import statements (the name there is intentional)
                if stripped.startswith('import '):
                    continue
                # Skip full-line comments
                if stripped.startswith('//') or stripped.startswith('/*') or stripped.startswith('*'):
                    continue
                new_line = name_re.sub(flash_fqn, line)
                if new_line != line:
                    lines[i] = new_line
                    changed = True

            if changed:
                with open(fpath, 'w', encoding='utf-8') as f:
                    f.write('\n'.join(lines))
                fixes += 1

    if fixes:
        _log(f"Fixed {fixes} file(s) with fully-qualified name replacement")


# ═══════════════════════════════════════════════════════════════════════════
#  High-level: recompile edited class(es) → patched SWF
# ═══════════════════════════════════════════════════════════════════════════

def recompile_class(
    swf_path: str,
    block_index: int,
    local_index: int,
    edited_source: str,
    output_path: Optional[str] = None,
    sdk_path: Optional[str] = None,
) -> dict:
    """Recompile one edited class and produce a new SWF.

    Thin wrapper around :func:`recompile_classes` for backward compatibility.
    """
    log.info("recompile_class: %s block=%d local=%d", swf_path, block_index, local_index)
    return recompile_classes(
        swf_path=swf_path,
        block_index=block_index,
        edits={local_index: edited_source},
        output_path=output_path,
        sdk_path=sdk_path,
    )


def recompile_classes(
    swf_path: str,
    block_index: int,
    edits: Dict[int, str],
    output_path: Optional[str] = None,
    sdk_path: Optional[str] = None,
    cached_sources: Optional[Dict[int, str]] = None,
) -> dict:
    """Recompile one or more edited classes and produce a new SWF.

    Parameters
    ----------
    swf_path : str
        Path to the original SWF file.
    block_index : int
        Index of the ABC block within the SWF to modify.
    edits : dict
        Mapping of ``{local_class_index: edited_source_text}``.
        Multiple classes can be edited in a single pass.
    output_path : str, optional
        Output SWF path.  Defaults to ``<original>_modified.swf``.
    sdk_path : str, optional
        Path to Flex/AIR SDK.
    cached_sources : dict, optional
        Mapping of ``{local_class_index: decompiled_source}`` for classes
        whose source is already known.  Skips re-decompilation for these.

    Returns
    -------
    dict
        ``success``, ``message``, ``errors``, ``logs``, ``elapsed``,
        ``output_path``
    """
    log.info("recompile_classes: %s block=%d edits=%d", swf_path, block_index, len(edits))
    logs: List[str] = []
    t0 = time.time()

    def _log(msg: str):
        logs.append(f"[{time.time() - t0:.2f}s] {msg}")

    _log(f"Recompile: block={block_index}, classes={sorted(edits.keys())}")
    _log(f"SWF: {os.path.basename(swf_path)}")

    def _fail(message, errors=None):
        return {
            'success': False, 'message': message,
            'errors': errors or [], 'logs': logs,
            'elapsed': round(time.time() - t0, 2),
        }

    # Resolve SDK
    if not sdk_path:
        sdk_path = find_sdk()
    if not sdk_path:
        return _fail('Flex/AIR SDK not found. Install to C:\\aflex_sdk or set FLEX_HOME.',
                      ['No SDK found at known paths or FLEX_HOME/AIR_SDK_HOME env vars.'])

    _log(f"SDK: {sdk_path}")
    _log(f"AIR SDK: {'yes' if _is_air_sdk(sdk_path) else 'no'}")

    try:
        mxmlc_path = find_mxmlc(sdk_path)
        _log(f"mxmlc: {mxmlc_path}")
    except FileNotFoundError as e:
        return _fail(str(e), [str(e)])

    # Check playerglobal.swc exists
    player_home = os.path.join(sdk_path, "frameworks", "libs", "player")
    if os.path.isdir(player_home):
        versions = sorted(os.listdir(player_home), reverse=True)
        if versions:
            pg = os.path.join(player_home, versions[0], "playerglobal.swc")
            _log(f"playerglobal: {pg} ({'exists' if os.path.isfile(pg) else 'MISSING'})")
        else:
            _log("WARNING: No player versions found in SDK")
    else:
        _log(f"WARNING: Player home dir missing: {player_home}")

    # Default output path — overwrite the original file in-place
    if not output_path:
        output_path = swf_path

    # ── Step 1: Read original SWF ──────────────────────────────────────
    try:
        swf_info = read_swf_full(swf_path)
    except Exception as e:
        return _fail(f'Failed to read SWF: {e}', [str(e)])

    tags = swf_info['tags']
    abc_tag_indices = _get_abc_block_indices(tags)
    _log(f"SWF: v{swf_info['version']}, {len(tags)} tags, {len(abc_tag_indices)} ABC block(s)")

    if block_index >= len(abc_tag_indices):
        return _fail(f'ABC block {block_index} not found (SWF has {len(abc_tag_indices)} blocks)',
                      [f'Invalid block_index {block_index}'])

    target_tag_idx = abc_tag_indices[block_index]
    tag_type, tag_body = tags[target_tag_idx]
    _log(f"Target DoABC tag: index={target_tag_idx}, type={tag_type}, size={len(tag_body)} bytes")

    # ── Step 2: Parse the ABC block, decompile all classes ─────────────
    try:
        block_name, abc_data = _extract_abc_data_from_tag(tag_type, tag_body)
        abc = ABCFile(abc_data)
        decomp = AS3Decompiler(abc)
        class_list = decomp.list_classes()
    except Exception as e:
        return _fail(f'Failed to parse ABC block: {e}', [str(e)])

    _log(f"Block '{block_name}': {len(class_list)} classes")

    for local_index in edits:
        if local_index >= len(class_list):
            return _fail(
                f'Class index {local_index} not found in block {block_index}',
                [f'Block has {len(class_list)} classes'],
            )

    for local_index in sorted(edits):
        edited_cls = class_list[local_index]
        _log(f"Editing: {edited_cls['full_name']}")

    # ── Step 2b: Detect which methods actually changed per class ───
    # Maps local_index → (changed_methods, original_source, edited_source)
    per_class_info: Dict[int, dict] = {}
    for local_index, edited_source in edits.items():
        edited_cls = class_list[local_index]
        try:
            original_source = decomp.decompile_class(local_index)
        except Exception as e:
            original_source = None
            _log(f"WARNING: Could not decompile {edited_cls['full_name']} for diff: {e}")

        if original_source is not None:
            class_short_name = edited_cls['name']
            changed_methods = _detect_changed_methods(
                original_source, edited_source, class_short_name
            )
            if changed_methods:
                _log(f"  {edited_cls['name']}: changed methods ({len(changed_methods)}): {sorted(changed_methods)}")
            else:
                _log(f"  {edited_cls['name']}: no method-level changes (comments/whitespace only)")
        else:
            changed_methods = None

        per_class_info[local_index] = {
            'changed_methods': changed_methods,
            'original_source': original_source,
            'edited_source': edited_source,
            'class_info': edited_cls,
        }

    # ── Step 3: Write all class .as files to persistent work dir ─────────
    # Use a persistent directory (based on SWF path) so mxmlc incremental
    # compilation can reuse its type-check cache between recompile calls.
    import hashlib
    swf_hash = hashlib.md5(os.path.abspath(swf_path).encode()).hexdigest()[:12]
    tmp_dir = os.path.join(tempfile.gettempdir(), f"swf_recompile_{swf_hash}")
    os.makedirs(tmp_dir, exist_ok=True)
    source_dir = os.path.join(tmp_dir, "src")
    os.makedirs(source_dir, exist_ok=True)

    errors_decompiling = []
    stub_count = 0
    files_written = 0

    for ci, cls_info in enumerate(class_list):
        pkg = cls_info['package']
        name = cls_info['name']

        # Create package directory
        if pkg:
            pkg_dir = os.path.join(source_dir, pkg.replace('.', os.sep))
            os.makedirs(pkg_dir, exist_ok=True)
            as_path = os.path.join(pkg_dir, f"{name}.as")
        else:
            as_path = os.path.join(source_dir, f"{name}.as")

        if ci in edits:
            # Use the user's edited source
            source = edits[ci]
        elif cached_sources and ci in cached_sources:
            # Use cached source from frontend (skip decompilation)
            source = cached_sources[ci]
        else:
            # Decompile the original
            try:
                source = decomp.decompile_class(ci)
            except Exception as e:
                errors_decompiling.append(f"Stub: {cls_info['full_name']} ({e})")
                stub_count += 1
                # Write a minimal stub so mxmlc can resolve references
                super_cls = cls_info.get('super', 'Object')
                super_short = super_cls.split('.')[-1] if super_cls else 'Object'
                is_iface = cls_info.get('is_interface', False)
                if is_iface:
                    source = f"package {pkg} {{\n    public interface {name} {{}}\n}}\n"
                else:
                    source = (
                        f"package {pkg} {{\n"
                        f"    public class {name} extends {super_short} {{\n"
                        f"        public function {name}() {{ super(); }}\n"
                        f"    }}\n"
                        f"}}\n"
                    )

        # Only write if the file doesn't exist or content changed.
        # This avoids touching timestamps so mxmlc incremental can skip
        # re-checking unchanged files.
        need_write = True
        if os.path.isfile(as_path):
            try:
                with open(as_path, 'r', encoding='utf-8') as f:
                    existing = f.read()
                if existing == source:
                    need_write = False
            except Exception:
                pass

        if need_write:
            with open(as_path, 'w', encoding='utf-8') as f:
                f.write(source)
            files_written += 1

    _log(f"Source files: {len(class_list)} total, {files_written} written, "
         f"{len(class_list) - files_written} cached ({stub_count} stubs)")

    if len(class_list) == 0:
        return _fail('No classes found in ABC block')

    # ── Step 3b: Fix ambiguous names ──────────────────────────────
    # When decompiled classes shadow flash.* builtins (e.g.
    # com.mcleodgaming.ssf2.util.Dictionary vs flash.utils.Dictionary),
    # mxmlc can't resolve the unqualified name. Build a map of
    # "simple name → list of packages" and for each source file that
    # uses an ambiguous name, ensure it has an explicit import.
    _fix_ambiguous_names(source_dir, class_list, _log)

    # Build mxmlc includes config.  We only need to compile the edited
    # classes (plus whatever mxmlc pulls in as dependencies).  This is
    # dramatically faster than compiling all 500+ classes.
    #
    # When FCSH is active, DON'T rewrite the config — FCSH was prewarmed
    # with a stable config and any change triggers an expensive full rebuild.
    config_path = os.path.join(tmp_dir, "includes.cfg")
    temp_swf = os.path.join(tmp_dir, "compiled.swf")

    mgr = MxmlcShellManager.get()
    use_shell = mgr.is_ready() and mgr.out_swf == temp_swf

    if not use_shell:
        edited_syms = [class_list[ci]["full_name"] for ci in edits]
        with open(config_path, 'w', encoding='utf-8') as cfg:
            cfg.write('<flex-config>\n')
            cfg.write('  <includes append="true">\n')
            for sym in edited_syms:
                cfg.write(f'    <symbol>{sym}</symbol>\n')
            cfg.write('  </includes>\n')
            cfg.write('</flex-config>\n')
    else:
        _log("MxmlcShell active — using prewarmed config")

    # Use the first edited class as the main entry for mxmlc
    first_edited = sorted(edits.keys())[0]
    first_cls = class_list[first_edited]
    pkg = first_cls['package']
    name = first_cls['name']
    if pkg:
        main_class_path = os.path.join(source_dir, pkg.replace('.', os.sep), f"{name}.as")
    else:
        main_class_path = os.path.join(source_dir, f"{name}.as")

    # ── Step 4: Compile with mxmlc (via persistent JVM if available) ──
    compile_t0 = time.time()

    # Try MxmlcShell first (warm JVM compile ~4-5s)
    shell_result = None
    if use_shell:
        _log("Compiling via MxmlcShell (warm JVM)...")
        shell_result = mgr.compile()

    if shell_result is not None:
        success, stdout, stderr = shell_result
        compile_elapsed = time.time() - compile_t0
        _log(f"MxmlcShell compile: {compile_elapsed:.1f}s (exit={'OK' if success else 'FAIL'})")
    else:
        # Fall back to direct mxmlc (cold start ~10s)
        _log("Running mxmlc (direct, no warm JVM)...")
        success, stdout, stderr = _compile_fast(
            source_dirs=[source_dir],
            main_class=main_class_path,
            sdk_path=sdk_path,
            output_swf=temp_swf,
            swf_version=swf_info['version'],
            extra_args=[f"-load-config+={config_path}", "-incremental=true"],
        )
        compile_elapsed = time.time() - compile_t0
        _log(f"mxmlc finished in {compile_elapsed:.1f}s (exit={'OK' if success else 'FAIL'})")

    if not success:
        _log("mxmlc FAILED — full output below")

        # Collect ALL output for user (not just filtered "Error" lines)
        all_output = []
        combined = (stderr + '\n' + stdout).strip()
        if combined:
            # Clean up temp paths for readability
            combined = combined.replace(source_dir + os.sep, '')
            combined = combined.replace(source_dir.replace('\\', '/') + '/', '')
            combined = combined.replace(tmp_dir + os.sep, '')
            combined = combined.replace(tmp_dir.replace('\\', '/') + '/', '')
            all_output.append(combined)
        else:
            all_output.append('mxmlc produced no output — check SDK installation')

        if errors_decompiling:
            all_output.append('\n--- Decompile warnings ---')
            all_output.extend(errors_decompiling)

        return {
            'success': False,
            'message': f'mxmlc compilation failed ({compile_elapsed:.1f}s)',
            'errors': all_output,
            'logs': logs,
            'elapsed': round(time.time() - t0, 2),
        }

    if not os.path.isfile(temp_swf):
        return _fail('mxmlc did not produce output SWF', ['No output file generated'])

    compiled_size = os.path.getsize(temp_swf)
    _log(f"Compiled SWF: {compiled_size} bytes")

    # ── Step 5: Binary-patch the edited class(es) ─────────────
    #
    # Instead of replacing the entire ABC block with mxmlc's output
    # (which round-trips ALL classes and can break runtime behavior),
    # we surgically transplant only the edited classes' method bodies
    # from mxmlc's ABC into the original ABC.  This preserves all
    # other classes' bytecode byte-for-byte.
    #
    # For multi-class edits, we chain transplant_class() calls:
    # each call takes the previous output as its "original" input.
    try:
        new_abc_tags = _extract_doabc_from_swf(temp_swf)
    except Exception as e:
        return _fail(f'Failed to read compiled SWF: {e}', [str(e)])

    if not new_abc_tags:
        return _fail('No ABC data in compiled SWF', ['mxmlc output contained no DoABC tags'])

    # Extract the compiled ABC data
    new_compiled_tag_type, new_compiled_tag_body = new_abc_tags[0]
    _, compiled_abc_data = _extract_abc_data_from_tag(new_compiled_tag_type, new_compiled_tag_body)
    _log(f"Compiled ABC: {len(compiled_abc_data)} bytes")

    # Chain transplant for each edited class
    current_abc = abc_data  # start with original raw bytes
    any_transplanted = False

    for local_index in sorted(edits.keys()):
        info = per_class_info[local_index]
        edited_cls = info['class_info']
        changed_methods = info['changed_methods']
        original_source = info['original_source']
        edited_source = info['edited_source']
        class_full_name = edited_cls['full_name']

        if isinstance(changed_methods, set) and len(changed_methods) == 0:
            _log(f"  {edited_cls['name']}: no method body changes "
                 f"(running transplant for structural changes)")
            # Still call transplant_class — it handles structural
            # features (new slots, interface changes, signature updates)
            # even when no method bodies change.

        # Extract source-level string edits for inline constant patching
        source_string_edits = None
        if original_source is not None:
            from as3_decompiler.abc_patcher import _extract_string_edits_from_source
            source_string_edits = _extract_string_edits_from_source(
                original_source, edited_source,
            )
            if source_string_edits:
                _log(f"  {edited_cls['name']}: string edits: {source_string_edits}")

        try:
            current_abc = transplant_class(
                current_abc, compiled_abc_data, class_full_name,
                changed_methods=changed_methods,
                source_string_edits=source_string_edits,
            )
            any_transplanted = True
            _log(f"  {edited_cls['name']}: transplanted OK")
        except Exception as e:
            _log(f"  {edited_cls['name']}: transplant failed: {e}")
            if len(edits) == 1:
                _log("Falling back to full ABC replacement")
                current_abc = compiled_abc_data
                any_transplanted = True
            else:
                return _fail(
                    f"Transplant failed for {class_full_name}: {e}",
                    [str(e)],
                )

    if not any_transplanted:
        _log("No classes required transplant — re-serializing original ABC")
        from as3_decompiler.abc_patcher import serialize_abc
        current_abc = serialize_abc(ABCFile(abc_data))

    patched_abc = current_abc
    _log(f"Patched ABC: {len(patched_abc)} bytes "
         f"(delta: {len(patched_abc) - len(abc_data):+d} bytes)")

    # Rebuild the DoABC2 tag body with original block name and flags
    orig_flags = struct.unpack_from('<I', tag_body[:4])[0]
    new_tag_body = _build_doabc2_tag_body(block_name, patched_abc, flags=orig_flags)
    _log(f"Rebuilt DoABC2 tag: name='{block_name}', flags={orig_flags}, "
         f"body={len(new_tag_body)} bytes")

    # ── Step 6: Patch the original SWF's tag list ────────────────
    patched_tags = list(tags)
    patched_tags[target_tag_idx] = (tag_type, new_tag_body)

    patched_info = dict(swf_info)
    patched_info['tags'] = patched_tags

    # ── Step 7: Write the new SWF ────────────────────────────────
    try:
        write_swf_from_tags(patched_info, output_path)
    except Exception as e:
        return _fail(f'Failed to write SWF: {e}', [str(e)])

    out_size = os.path.getsize(output_path)
    _log(f"Wrote {os.path.basename(output_path)} ({out_size} bytes)")

    total = round(time.time() - t0, 2)
    edited_names = [per_class_info[i]['class_info']['full_name'] for i in sorted(edits.keys())]
    if len(edited_names) == 1:
        result_msg = (
            f"✓ Compiled {edited_names[0]} — "
            f"wrote {os.path.basename(output_path)} ({total}s)"
        )
    else:
        result_msg = (
            f"✓ Compiled {len(edited_names)} classes — "
            f"wrote {os.path.basename(output_path)} ({total}s)"
        )
    warnings = errors_decompiling if errors_decompiling else []

    return {
        'success': True,
        'message': result_msg,
        'errors': warnings,
        'logs': logs,
        'output_path': output_path,
        'elapsed': total,
    }
