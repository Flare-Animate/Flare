"""Generate DefineShape bodies whose geometry is known exactly, one per fault.

The SWF shape record stream is bit-packed, so a test over a real file can only say
whether the result looks plausible -- and a plausible wrong answer is exactly what
this format produces when the bit stream is misread. Every fixture here states the
coordinates it contains and the header fields it carries, so agreement is evidence
rather than a judgement call.

The square is 400x400 twips, matching the shape Ruffle's DefineShape.fla carries as

    !0 0|400 0!400 0|400 400!400 400|0 400!0 400|0 0

and ruffle_DefineShape.swf carries the same outline as a single solid red fill with
no stroke, so it is decoded too and checked against the same endpoints.

Each of the other fixtures exists because a real fault slipped through without one:

  bitmap fill   the MATRIX is bit-packed, so a style that carries one does not end
                on a byte boundary, and the next field must still be read from the
                boundary. The matrix here ends 7 bits short on purpose. Reading it
                straddling instead gives the following byte a different *value*.
  gradient      the MATRIX comes *before* the gradient records, and a gradient is a
                flags byte holding spread, interpolation and the record count,
                followed by records of one ratio byte and a colour. Getting the
                order or the record layout wrong desynchronises the style arrays
                without failing.
  two fills     a style change may set StateFillStyle0 and StateFillStyle1 together,
                which carries two indices. Consuming one resumes inside the other.

A fixture generator and the decoder written from one reading of the format will
agree with each other whatever that reading is, so where a field's width was in
doubt the value here was checked against JPEXS's parse of the same bytes.

usage: python generated_swf_shapes.py <outdir>
"""

import os
import struct
import sys
import zlib


class BW:
    """A bit writer, MSB first, which is the order every field in this format uses."""

    def __init__(self):
        self.bits = []

    def u8(self, v):
        self.put(v, 8)

    def u16le(self, v):
        # UI16 is little-endian; a bit writer cannot express that, so the bytes go
        # out individually.
        self.put(v & 0xFF, 8)
        self.put((v >> 8) & 0xFF, 8)

    def put(self, v, n):
        for k in range(n - 1, -1, -1):
            self.bits.append((v >> k) & 1)

    def s(self, v, n):
        self.put(v & ((1 << n) - 1), n)

    def rect(self, xmin, xmax, ymin, ymax, nbits):
        self.put(nbits, 5)
        for v in (xmin, xmax, ymin, ymax):
            self.s(v, nbits)
        # The RECT is the one bit-packed part of the header, and what follows it is
        # read a byte at a time. Its unused low bits are zero padding.
        self.pad_to_byte()

    def pad_to_byte(self):
        """Zero-fill to the next byte boundary."""
        while len(self.bits) % 8:
            self.bits.append(0)

    def matrix(self, scale=None, rotate=None, translate=(0, 0),
               n_scale=0, n_rotate=0, n_translate=0):
        if scale is not None:
            self.put(1, 1)
            self.put(n_scale, 5)
            self.s(scale[0], n_scale)
            self.s(scale[1], n_scale)
        else:
            self.put(0, 1)
        if rotate is not None:
            self.put(1, 1)
            self.put(n_rotate, 5)
            self.s(rotate[0], n_rotate)
            self.s(rotate[1], n_rotate)
        else:
            self.put(0, 1)
        self.put(n_translate, 5)
        self.s(translate[0], n_translate)
        self.s(translate[1], n_translate)
        # A MATRIX is bit-packed and its width is not a multiple of eight, so a
        # style that carries one does not end on a byte boundary. The style arrays
        # that follow are read a byte at a time, from the boundary. On this matrix
        # that is 7 bits of padding, and reading the next field straddling instead
        # gives it a different value, not merely a different position.
        self.pad_to_byte()

    def bytes(self):
        out = bytearray((len(self.bits) + 7) // 8)
        for i, b in enumerate(self.bits):
            if b:
                out[i >> 3] |= 1 << (7 - (i & 7))
        return bytes(out)


# The state flags of a style change record, as a little-endian bitfield of the value
# read MSB-first -- so MoveTo is the *first* of the five bits and NewStyles the last.
MOVE_TO = 0x01
FILL_STYLE_0 = 0x02
FILL_STYLE_1 = 0x04
LINE_STYLE = 0x08
NEW_STYLES = 0x10


def rect_edges(w, dx, dy, nbits):
    """Four axis-aligned edges around a w-by-dy box, as straight edge records.

    numBits is stored as numBits-2, and a straight edge is
    type, straight, numBits-2, GeneralDeltaFlag, VerticalFlag, delta.
    """
    for ex, ey in ((dx, 0), (0, dy), (-dx, 0), (0, -dy)):
        w.put(1, 1)                       # an edge
        w.put(1, 1)                       # straight
        w.put(nbits - 2, 4)
        w.put(0, 1)                       # not general -> axis aligned
        if ey == 0:
            w.put(0, 1)                   # horizontal: stores DeltaX
            w.s(ex, nbits)
        else:
            w.put(1, 1)                   # vertical: stores DeltaY
            w.s(ey, nbits)


def end_of_shape(w):
    w.put(0, 1)                           # not an edge
    w.put(0, 5)                           # all five state bits clear
    # No reserved field follows. Counting the records JPEXS reports for Ruffle's
    # DefineShape.swf, with six reserved bits here they need 97 bits and only 88
    # remain, so the walk runs off the end; without them they need 85 and fit.


def solid_fill(w, rgb=(0xFF, 0x00, 0x00), version=1):
    w.u8(0x00)
    for c in rgb:
        w.u8(c)
    if version >= 3:
        w.u8(0xFF)          # alpha, from DefineShape3


def line_style_v1(w, width=20, rgb=(0, 0, 0), version=1):
    # Width is UI16 in every shape version. Reading it as a leading byte plus a
    # further UI16 for version >= 2 consumes 24 bits where the format has 16, and
    # every shape with a stroke desynchronises from that point on -- but only from
    # DefineShape2 onwards, which is why a version-1 fixture cannot catch it.
    w.u8(width & 0xFF)
    w.u8((width >> 8) & 0xFF)
    for c in rgb:
        w.u8(c)
    if version >= 3:
        w.u8(0xFF)
    # and nothing after: a LineStyle1 has no matrix.


# ---------------------------------------------------------------------------
# 1. The square, in both edge encodings, at two shape versions.
#
# The DefineShape and DefineShape2 bodies are byte-identical for this shape: the
# only differences between the two in a shape body are that the style counts may be
# written as 0xFF followed by a UI16, and that DefineShape3 adds an alpha byte to
# the colours. This square has one fill, one line style and no alpha, so both
# versions produce the same bytes.
#
# Both are emitted because the line style's width is where the versions differ in
# what a decoder must do with it, and a version-1 fixture cannot catch a fault that
# only shows at version >= 2.
# ---------------------------------------------------------------------------
def build_square(axis_aligned=False, version=1):
    w = BW()
    w.u16le(1)
    w.rect(0, 400, 0, 400, 10)
    w.u8(1)                          # FillStyleArray count
    solid_fill(w, version=version)
    w.u8(1)                          # LineStyleArray count
    line_style_v1(w, version=version)
    w.u8(0x00)                       # NumFillBits = 0, NumLineBits = 0

    w.put(0, 1)                      # not an edge
    w.put(MOVE_TO, 5)
    w.put(10, 5)                     # MoveBits
    w.s(0, 10); w.s(0, 10)

    for dx, dy in ((400, 0), (0, 400), (-400, 0), (0, -400)):
        w.put(1, 1)
        w.put(1, 1)
        w.put(8, 4)                  # numBits - 2, so 10 bits
        if axis_aligned:
            w.put(0, 1)              # not general -> axis aligned
            if dy == 0:
                w.put(0, 1)
                w.s(dx, 10)
            else:
                w.put(1, 1)          # vertical: stores DeltaY
                w.s(dy, 10)
        else:
            w.put(1, 1)              # GeneralDelta
            w.s(dx, 10)
            w.s(dy, 10)

    end_of_shape(w)
    return w.bytes()


# ---------------------------------------------------------------------------
# 2. A bitmap fill, whose MATRIX does not end on a byte boundary.
# ---------------------------------------------------------------------------
def build_bitmap_fill():
    w = BW()
    w.u16le(2)
    w.rect(0, 100, 0, 100, 8)
    w.u8(1)                          # FillStyleArray count
    w.u8(0x41)                       # clipped, smoothed, tiled bitmap
    w.u16le(7)                       # BitmapId
    # A MATRIX with no scale and no rotation: 1 + 1 + 5 + 16 = 23 bits, which is 7
    # bits short of a byte. The padding that follows is the point of this fixture.
    w.matrix(translate=(10, 20), n_translate=8)
    w.u8(0)                          # NumLineStyles
    w.u8(0x10)                       # NumFillBits = 1, NumLineBits = 0

    w.put(0, 1)
    w.put(MOVE_TO | FILL_STYLE_0, 5)
    w.put(8, 5)                      # MoveBits
    w.s(0, 8); w.s(0, 8)
    w.put(0, 1)                      # fill index 0, NumFillBits wide
    rect_edges(w, 100, 100, 8)
    end_of_shape(w)
    return w.bytes()


# ---------------------------------------------------------------------------
# 3. A radial gradient fill: matrix first, then a flags byte and two records.
# ---------------------------------------------------------------------------
def build_gradient():
    w = BW()
    w.u16le(3)
    w.rect(0, 120, 0, 100, 8)
    w.u8(1)                          # FillStyleArray count
    w.u8(0x12)                       # radial gradient
    w.matrix(translate=(20, 30), n_translate=10)
    w.u8(0x02)                       # spread 0, interpolation 0, 2 records
    for ratio, rgba in ((0x00, (0, 0, 255, 0)), (0xFF, (255, 242, 101, 255))):
        w.u8(ratio)                  # one ratio byte per record
        for c in rgba:               # RGBA from DefineShape3
            w.u8(c)
    # The records are whole bytes, so the style ends wherever the matrix padding
    # plus the records put it -- here still short of a boundary, since the matrix
    # padded to 104 and the records are 10 bytes.
    w.pad_to_byte()
    w.u8(0)                          # NumLineStyles
    w.u8(0x10)                       # NumFillBits = 1

    w.put(0, 1)
    w.put(MOVE_TO | FILL_STYLE_0, 5)
    w.put(8, 5)
    w.s(0, 8); w.s(0, 8)
    w.put(0, 1)
    rect_edges(w, 120, 100, 8)
    end_of_shape(w)
    return w.bytes()


# ---------------------------------------------------------------------------
# 3b. A bitmap fill whose MATRIX carries both a scale and a rotation.
#
# Each optional part of a MATRIX holds *two* signed values. Reading only one of the
# scale pair put everything after a scaled fill a whole nScaleBits out of step, which
# is why 111 of the 250 real shapes stopped in the style arrays: the fill count came
# out right and then the fills could not be walked. A fixture whose matrix has no
# scale cannot catch that, which is why this one exists separately from the plain
# bitmap fill above.
# ---------------------------------------------------------------------------
def build_rotated_fill():
    w = BW()
    w.u16le(5)
    w.rect(0, 100, 0, 100, 8)
    w.u8(1)                          # FillStyleArray count
    w.u8(0x40)                       # repeating bitmap
    w.u16le(9)                       # BitmapId
    # scale 1/4 in 20 bits (4 << 18), rotate 0/0 in 20 bits, translate 0/0 in 8.
    w.matrix(scale=(1 << 18, 1 << 18), n_scale=20,
             rotate=(0, 0), n_rotate=20,
             translate=(5, 5), n_translate=8)
    w.u8(0)                          # NumLineStyles
    w.u8(0x10)                       # NumFillBits = 1, NumLineBits = 0

    w.put(0, 1)
    w.put(MOVE_TO | FILL_STYLE_0, 5)
    w.put(8, 5)
    w.s(0, 8); w.s(0, 8)
    w.put(0, 1)
    rect_edges(w, 100, 100, 8)
    end_of_shape(w)
    return w.bytes()


# ---------------------------------------------------------------------------
# 4. A style change that sets both fill flags, so two indices must be consumed.
# ---------------------------------------------------------------------------
def build_two_fill_indices():
    w = BW()
    w.u16le(4)
    w.rect(0, 60, 0, 60, 7)
    w.u8(1)                          # FillStyleArray count
    solid_fill(w, (0x00, 0xFF, 0x00))
    w.u8(0)                          # NumLineStyles
    w.u8(0x20)                       # NumFillBits = 2, NumLineBits = 0

    w.put(0, 1)
    w.put(MOVE_TO | FILL_STYLE_0 | FILL_STYLE_1, 5)
    w.put(8, 5)
    w.s(0, 8); w.s(0, 8)
    w.put(0, 2)                      # fillStyle0 index, 2 bits
    w.put(0, 2)                      # fillStyle1 index, 2 bits -- both must be read
    rect_edges(w, 60, 60, 8)
    end_of_shape(w)
    return w.bytes()


# ---------------------------------------------------------------------------
# 5. The body of Ruffle's own DefineShape.swf, lifted out of the container.
#
# That file is authored against the specification and is known good, so it settles
# the questions the specification's prose leaves open. Its RECT ends at bit 61 and
# the fill count is at bit 64, reading 1 solid red fill; read straddling at bit 61
# the same byte reads 0 fills and the next reads 32 line styles, impossible in a
# 26-byte tag.
# ---------------------------------------------------------------------------
def build_from_ruffle_fixture(path):
    raw = open(path, "rb").read()
    d = (b"FWS" + raw[3:8] + zlib.decompress(raw[8:])
         if raw[:3] in (b"CWS", b"ZWS") else raw)
    p = 8
    n = (d[p] >> 3) & 0x1F
    off = p + ((5 + 4 * n + 7) // 8) + 4
    while off + 2 <= len(d):
        h = d[off] | (d[off + 1] << 8)
        code, ln = h >> 6, h & 0x3F
        off += 2
        if ln == 0x3F:
            ln = struct.unpack_from("<I", d, off)[0]
            off += 4
        if code == 0:
            return None
        if code == 2:
            return d[off:off + ln]
        off += ln
    return None


# What each fixture declares. The tests assert these, so a fixture that stops saying
# what it contains is caught rather than quietly agreeing with whatever the decoder
# does.
SQUARE = [(400, 0), (400, 400), (0, 400), (0, 0)]
BITMAP_SQUARE = [(100, 0), (100, 100), (0, 100), (0, 0)]
ROTATED_SQUARE = BITMAP_SQUARE
GRADIENT_SQUARE = [(120, 0), (120, 100), (0, 100), (0, 0)]
TWO_FILL_SQUARE = [(60, 0), (60, 60), (0, 60), (0, 0)]


def main():
    outdir = sys.argv[1] if len(sys.argv) > 1 else "."
    here = os.path.dirname(os.path.abspath(__file__))
    os.makedirs(outdir, exist_ok=True)

    written = []
    for aa in (False, True):
        for ver in (1, 2):
            data = build_square(aa, ver)
            name = f"swf_square_v{ver}_aa{int(aa)}.bin"
            open(os.path.join(outdir, name), "wb").write(data)
            written.append((name, len(data), data))

    for name, builder in (("swf_bitmapfill_v1.bin", build_bitmap_fill),
                          ("swf_rotatedfill_v1.bin", build_rotated_fill),
                          ("swf_gradient_v3.bin", build_gradient),
                          ("swf_twofillindices_v1.bin",
                           build_two_fill_indices)):
        data = builder()
        open(os.path.join(outdir, name), "wb").write(data)
        written.append((name, len(data), data))

    src = os.path.join(here, "ruffle_DefineShape.swf")
    if os.path.exists(src):
        body = build_from_ruffle_fixture(src)
        if body is not None:
            open(os.path.join(outdir, "swf_ruffle_square_v1.bin"),
                 "wb").write(body)
            written.append(("swf_ruffle_square_v1.bin", len(body), body))

    for name, size, data in written:
        print(f"wrote {name:<28} {size:>4} bytes  {data.hex(' ')}")
    print(f"expected endpoints: square {SQUARE}")
    print(f"                    bitmap {BITMAP_SQUARE}")
    print(f"                    scaled and rotated {ROTATED_SQUARE}")
    print(f"                    gradient {GRADIENT_SQUARE}")
    print(f"                    two fills {TWO_FILL_SQUARE}")
    print("                    ruffle fixture {SQUARE}")


if __name__ == "__main__":
    main()
