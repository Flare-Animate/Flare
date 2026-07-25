"""SWF file reader and ABC block extraction.

Provides three levels of API:

  read_abc_blocks(path)   – Memory-efficient: streams tags, keeps only ABC data.
                            Use this for large SWFs (100+ MB).
  iter_tags(path)         – Generator yielding (tag_type, body) one at a time.
                            Never holds more than one tag body in memory.
  read_swf(path)          – Legacy: loads all tags into a list.
                            Convenient for small files, but O(filesize) memory.
"""

from __future__ import annotations

import io
import logging
import struct
import zlib
from typing import Generator, List, Tuple

log = logging.getLogger(__name__)

__all__ = [
    'read_swf', 'iter_tags', 'read_abc_blocks',
    'extract_abc_blocks', 'TAG_DOABC', 'TAG_DOABC2',
]

# ── SWF spec constants ────────────────────────────────────────────────
SIG_FWS = b'FWS'          # Uncompressed SWF signature
SIG_CWS = b'CWS'          # zlib-compressed SWF signature
SIG_ZWS = b'ZWS'          # LZMA-compressed SWF signature

_SWF_HEADER_SIZE      = 8   # Signature (3) + version (1) + file length (4)
_RECT_NBITS_SHIFT     = 3   # RECT Nbits is top 5 bits of byte (>> 3)
_RECT_NBITS_MASK      = 0x1F  # 5-bit field
_RECT_FIELD_COUNT     = 4   # Xmin, Xmax, Ymin, Ymax
_FRAME_INFO_SIZE      = 4   # frame_rate (u16) + frame_count (u16)

_TAG_CODE_SHIFT       = 6   # tag_type is upper 10 bits of tag_code_and_length
_TAG_SHORT_LENGTH_MASK = 0x3F  # lower 6 bits = short length
_TAG_LONG_LENGTH_FLAG  = 0x3F  # length == 0x3F → read 4-byte extended length
_TAG_END              = 0   # End tag type

TAG_DOABC  = 72
TAG_DOABC2 = 82

# Default read chunk size for streaming decompression (256 KB)
_STREAM_CHUNK_SIZE = 256 * 1024


# ═══════════════════════════════════════════════════════════════════════════
#  Low-level streaming helpers
# ═══════════════════════════════════════════════════════════════════════════

class _StreamReader:
    """Buffered reader over a binary stream.

    Provides exact-byte reads by buffering internally so that callers
    don't have to worry about short reads from decompression streams.
    """

    def __init__(self, stream: io.RawIOBase | io.BufferedIOBase):
        self._stream = stream
        self._buf = bytearray()

    def read(self, n: int) -> bytes:
        """Read exactly *n* bytes, or raise EOFError."""
        while len(self._buf) < n:
            chunk = self._stream.read(_STREAM_CHUNK_SIZE)
            if not chunk:
                break
            self._buf.extend(chunk)
        if len(self._buf) < n:
            raise EOFError(f"Expected {n} bytes, got {len(self._buf)}")
        result = bytes(self._buf[:n])
        del self._buf[:n]
        return result

    def read_available(self, n: int) -> bytes:
        """Read up to *n* bytes (may return fewer at EOF, never raises)."""
        while len(self._buf) < n:
            chunk = self._stream.read(_STREAM_CHUNK_SIZE)
            if not chunk:
                break
            self._buf.extend(chunk)
        take = min(n, len(self._buf))
        result = bytes(self._buf[:take])
        del self._buf[:take]
        return result


def _open_swf_stream(path: str) -> Tuple[int, io.IOBase]:
    """Open a SWF file and return (version, decompressed_body_stream).

    The returned stream starts right after the 8-byte header (i.e. at the
    RECT field).  Callers must close it when done.
    """
    f = open(path, 'rb')
    try:
        header = f.read(_SWF_HEADER_SIZE)
        if len(header) < _SWF_HEADER_SIZE:
            raise ValueError("File too short to be a SWF")
        sig = header[:3]
        version = header[3]
        file_length = struct.unpack_from('<I', header, 4)[0]
        body_length = file_length - _SWF_HEADER_SIZE  # uncompressed body size

        if sig == SIG_FWS:
            # Uncompressed — the file *is* the stream.  Wrap the remainder in
            # a sub-stream that stops at file_length.
            return version, _LimitedFileReader(f, body_length)

        elif sig == SIG_CWS:
            # zlib compressed — use incremental decompressor
            return version, _ZlibStream(f)

        elif sig == SIG_ZWS:
            # LZMA compressed
            try:
                import lzma as _lzma
            except ImportError:
                raise RuntimeError("LZMA SWF requires the lzma module")
            # SWF LZMA layout after the 8-byte header:
            #   4 bytes compressed size, 5 bytes LZMA props, rest is data
            meta = f.read(9)  # 4 (compressed_size) + 5 (lzma_props)
            if len(meta) < 9:
                raise ValueError("Truncated LZMA SWF header")
            lzma_props = meta[4:9]
            # Parse LZMA properties and use FORMAT_RAW with explicit
            # FILTER_LZMA1.  FORMAT_ALONE with a known uncompressed size
            # fails on LZMA streams that contain an end-of-stream marker
            # (such as those produced by AIR SDK's mxmlc).
            prop_byte = lzma_props[0]
            pb = prop_byte // 45
            leftover = prop_byte - pb * 45
            lp = leftover // 9
            lc = leftover - lp * 9
            dict_size = struct.unpack_from('<I', lzma_props, 1)[0]
            return version, _LzmaStream(f, lc=lc, lp=lp, pb=pb, dict_size=dict_size)

        else:
            f.close()
            raise ValueError(f"Not a SWF file: signature {sig!r}")

    except Exception:
        f.close()
        raise


class _LimitedFileReader(io.RawIOBase):
    """Wraps a file object and limits reads to *limit* remaining bytes."""

    def __init__(self, f, limit: int):
        self._f = f
        self._remaining = limit

    def readable(self) -> bool:
        return True

    def read(self, n: int = -1) -> bytes:
        if self._remaining <= 0:
            return b''
        if n < 0:
            n = self._remaining
        n = min(n, self._remaining)
        data = self._f.read(n)
        self._remaining -= len(data)
        return data

    def close(self):
        self._f.close()
        super().close()


class _ZlibStream(io.RawIOBase):
    """Wraps a file containing zlib-compressed data and yields decompressed bytes."""

    def __init__(self, f):
        self._f = f
        self._dec = zlib.decompressobj()
        self._buf = b''
        self._eof = False

    def readable(self) -> bool:
        return True

    def read(self, n: int = -1) -> bytes:
        if n < 0:
            # Read everything remaining
            chunks = [self._buf] if self._buf else []
            while not self._eof:
                compressed = self._f.read(_STREAM_CHUNK_SIZE)
                if not compressed:
                    try:
                        chunks.append(self._dec.flush())
                    except Exception:
                        pass  # truncated/incomplete stream at EOF - keep what decompressed so far
                    self._eof = True
                    break
                chunks.append(self._dec.decompress(compressed))
            self._buf = b''
            return b''.join(chunks)

        while len(self._buf) < n and not self._eof:
            compressed = self._f.read(_STREAM_CHUNK_SIZE)
            if not compressed:
                try:
                    self._buf += self._dec.flush()
                except Exception:
                    pass  # truncated/incomplete stream at EOF - keep what decompressed so far
                self._eof = True
                break
            self._buf += self._dec.decompress(compressed)

        result = self._buf[:n]
        self._buf = self._buf[n:]
        return result

    def close(self):
        self._f.close()
        super().close()


class _LzmaStream(io.RawIOBase):
    """Wraps a file containing LZMA-compressed data and yields decompressed bytes.

    Uses FORMAT_RAW with an explicit FILTER_LZMA1 filter, which correctly
    handles LZMA streams both with and without end-of-stream markers.
    """

    def __init__(self, f, *, lc: int, lp: int, pb: int, dict_size: int):
        import lzma as _lzma
        self._f = f
        filters = [{
            'id': _lzma.FILTER_LZMA1,
            'lc': lc, 'lp': lp, 'pb': pb, 'dict_size': dict_size,
        }]
        self._dec = _lzma.LZMADecompressor(format=_lzma.FORMAT_RAW, filters=filters)
        self._buf = b''
        self._eof = False

    def readable(self) -> bool:
        return True

    def read(self, n: int = -1) -> bytes:
        if n < 0:
            chunks = [self._buf] if self._buf else []
            while not self._eof:
                compressed = self._f.read(_STREAM_CHUNK_SIZE)
                if not compressed:
                    self._eof = True
                    break
                try:
                    chunks.append(self._dec.decompress(compressed))
                except EOFError:
                    self._eof = True
                    break
            self._buf = b''
            return b''.join(chunks)

        while len(self._buf) < n and not self._eof:
            compressed = self._f.read(_STREAM_CHUNK_SIZE)
            if not compressed:
                self._eof = True
                break
            try:
                self._buf += self._dec.decompress(compressed)
            except EOFError:
                self._eof = True
                break

        result = self._buf[:n]
        self._buf = self._buf[n:]
        return result

    def close(self):
        self._f.close()
        super().close()


def _skip_swf_preamble(reader: _StreamReader) -> None:
    """Advance *reader* past the RECT and frame-info fields."""
    first = reader.read(1)
    nbits = (first[0] >> _RECT_NBITS_SHIFT) & _RECT_NBITS_MASK
    total_bits = 5 + nbits * _RECT_FIELD_COUNT
    rect_bytes = (total_bits + 7) // 8
    # We already consumed 1 byte of the RECT
    if rect_bytes > 1:
        reader.read(rect_bytes - 1)
    reader.read(_FRAME_INFO_SIZE)  # frame_rate + frame_count


# ═══════════════════════════════════════════════════════════════════════════
#  Public API — streaming
# ═══════════════════════════════════════════════════════════════════════════

def iter_tags(path: str) -> Generator[Tuple[int, bytes], None, None]:
    """Lazily yield (tag_type, tag_body) from a SWF file.

    Only one tag body is in memory at a time, making this suitable for
    arbitrarily large SWFs.  The SWF version is not returned; use
    ``read_swf_header()`` if you need it.

    Usage::

        for tag_type, body in iter_tags('huge.swf'):
            if tag_type == TAG_DOABC2:
                process(body)
    """
    log.debug("iter_tags: %s", path)
    version, stream = _open_swf_stream(path)
    try:
        reader = _StreamReader(stream)
        _skip_swf_preamble(reader)

        while True:
            header_bytes = reader.read_available(2)
            if len(header_bytes) < 2:
                break
            tag_code_and_length = struct.unpack('<H', header_bytes)[0]
            tag_type = tag_code_and_length >> _TAG_CODE_SHIFT
            tag_length = tag_code_and_length & _TAG_SHORT_LENGTH_MASK
            if tag_length == _TAG_LONG_LENGTH_FLAG:
                ext = reader.read_available(4)
                if len(ext) < 4:
                    break
                tag_length = struct.unpack('<I', ext)[0]
            if tag_type == _TAG_END:
                break
            body = reader.read_available(tag_length)
            yield tag_type, body
    finally:
        stream.close()


def read_abc_blocks(path: str) -> Tuple[int, List[Tuple[str, bytes]]]:
    """Memory-efficient: stream-parse a SWF and return only ABC blocks.

    Returns (version, [(name, abc_data), ...]).

    Non-ABC tags are skipped without being retained in memory, so a 200 MB
    SWF with 1 MB of ABC data will use ~1 MB of heap rather than ~200 MB.
    """
    log.debug("read_abc_blocks: %s", path)
    version, stream = _open_swf_stream(path)
    result: List[Tuple[str, bytes]] = []
    try:
        reader = _StreamReader(stream)
        _skip_swf_preamble(reader)

        while True:
            header_bytes = reader.read_available(2)
            if len(header_bytes) < 2:
                break
            tag_code_and_length = struct.unpack('<H', header_bytes)[0]
            tag_type = tag_code_and_length >> _TAG_CODE_SHIFT
            tag_length = tag_code_and_length & _TAG_SHORT_LENGTH_MASK
            if tag_length == _TAG_LONG_LENGTH_FLAG:
                ext = reader.read_available(4)
                if len(ext) < 4:
                    break
                tag_length = struct.unpack('<I', ext)[0]
            if tag_type == _TAG_END:
                break

            if tag_type == TAG_DOABC2:
                body = reader.read_available(tag_length)
                if len(body) >= 5:
                    null_pos = body.find(b'\x00', 4)
                    if null_pos >= 0:
                        name = body[4:null_pos].decode('utf-8', errors='replace')
                        result.append((name or '(unnamed)', body[null_pos + 1:]))
            elif tag_type == TAG_DOABC:
                body = reader.read_available(tag_length)
                result.append(('DoABC', body))
            else:
                # Skip non-ABC tag body without allocating
                _skip_bytes(reader, tag_length)
    finally:
        stream.close()

    return version, result


def _skip_bytes(reader: _StreamReader, n: int) -> None:
    """Discard *n* bytes from the reader in chunks (avoids large allocation)."""
    remaining = n
    while remaining > 0:
        chunk = min(remaining, _STREAM_CHUNK_SIZE)
        reader.read_available(chunk)
        remaining -= chunk


# ═══════════════════════════════════════════════════════════════════════════
#  Public API — legacy (all-in-memory)
# ═══════════════════════════════════════════════════════════════════════════

def read_swf(path: str) -> Tuple[int, List[Tuple[int, bytes]]]:
    """Read a SWF file, decompress if needed, return (version, [(tag_type, body)]).

    Loads all tags into memory.  For large SWFs consider ``read_abc_blocks()``
    or ``iter_tags()`` instead.
    """
    log.debug("read_swf: %s", path)
    version, stream = _open_swf_stream(path)
    try:
        reader = _StreamReader(stream)
        _skip_swf_preamble(reader)

        tags: List[Tuple[int, bytes]] = []
        while True:
            header_bytes = reader.read_available(2)
            if len(header_bytes) < 2:
                break
            tag_code_and_length = struct.unpack('<H', header_bytes)[0]
            tag_type = tag_code_and_length >> _TAG_CODE_SHIFT
            tag_length = tag_code_and_length & _TAG_SHORT_LENGTH_MASK
            if tag_length == _TAG_LONG_LENGTH_FLAG:
                ext = reader.read_available(4)
                if len(ext) < 4:
                    break
                tag_length = struct.unpack('<I', ext)[0]
            if tag_type == _TAG_END:
                break
            body = reader.read_available(tag_length)
            tags.append((tag_type, body))
    finally:
        stream.close()

    return version, tags


# ═══════════════════════════════════════════════════════════════════════════
#  Extract ABC from tag list (legacy helper)
# ═══════════════════════════════════════════════════════════════════════════

def extract_abc_blocks(tags: List[Tuple[int, bytes]]) -> List[Tuple[str, bytes]]:
    """Extract (name, abc_data) pairs from an in-memory tag list.

    Prefer ``read_abc_blocks(path)`` for large files — it streams tags and
    never loads non-ABC data into memory.
    """
    log.debug("extract_abc_blocks: %d tags", len(tags))
    result = []
    for tag_type, body in tags:
        if tag_type == TAG_DOABC2:
            if len(body) < 5:
                continue
            null_pos = body.find(b'\x00', 4)
            if null_pos < 0:
                continue
            name = body[4:null_pos].decode('utf-8', errors='replace')
            abc_data = body[null_pos + 1:]
            result.append((name or '(unnamed)', abc_data))
        elif tag_type == TAG_DOABC:
            result.append(('DoABC', body))
    return result
