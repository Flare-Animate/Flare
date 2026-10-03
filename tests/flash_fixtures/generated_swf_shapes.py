"""Generate synthetic DefineShape bodies whose geometry is known exactly.

The SWF shape record stream is bit-packed, so a test over real files can only say
whether the result looks plausible -- and a plausible wrong answer is exactly
what this format produces when the bit stream is misread. These fixtures state
their own coordinates, so agreement is evidence rather than a judgement call.

The square is 400x400 twips, matching the same shape Ruffle's DefineShape.fla
carries as:

    !0 0|400 0!400 0|400 400!400 400|0 400!0 400|0 0

Two encodings are produced, because the format allows both and a decoder that
only handles one is wrong:

  aa0  every edge carries both components (GeneralDelta set)
  aa1  the axis-aligned form, where a horizontal or vertical edge stores only
       the component that changes

usage: python generated_swf_shapes.py <outdir>
"""

import os
import sys


class BW:
    def __init__(self):
        self.bits = []

    def u8(self, v):
        self.put(v, 8)

    def u16le(self, v):
        self.put(v & 0xFF, 8)
        self.put((v >> 8) & 0xFF, 8)

    def put(self, v, n):
        for k in range(n - 1, -1, -1):
            self.bits.append((v >> k) & 1)

    def s(self, v, n):
        self.put(v & ((1 << n) - 1), n)

    def bytes(self):
        out = bytearray((len(self.bits) + 7) // 8)
        for i, b in enumerate(self.bits):
            if b:
                out[i >> 3] |= 1 << (7 - (i & 7))
        return bytes(out)


class BR:
    def __init__(self, d):
        self.d = d
        self.p = 0

    def bit(self):
        v = (self.d[self.p >> 3] >> (7 - (self.p & 7))) & 1
        self.p += 1
        return v

    def bits(self, n):
        v = 0
        for _ in range(n):
            v = (v << 1) | self.bit()
        return v

    def s(self, n):
        v = self.bits(n)
        return v - (1 << n) if v & (1 << (n - 1)) else v

    def u16le(self):
        lo = self.bits(8)
        hi = self.bits(8)
        return (hi << 8) | lo


def build(axis_aligned=False):
    w = BW()
    w.u16le(1)                       # CharacterID
    nb = 10
    w.put(nb, 5)
    for v in (0, 400, 0, 400):
        w.s(v, nb)

    w.u8(1)                          # FillStyleArray count
    w.u8(0)                          # type 0 = solid
    w.u8(0xFF); w.u8(0x00); w.u8(0x00)
    w.u8(1)                          # LineStyleArray count
    # Width is UI16 in every shape version. The first version of this wrote a
    # single byte, which is what the decoder also expected -- so the fixture
    # encoded the reader's own bug and would have kept passing after a fix. A
    # fixture built from the same reading as the code cannot catch that code
    # being wrong, which is the whole reason the square is checked against Ruffle's
    # .fla oracle as well.
    w.u8(20); w.u8(0x00)             # width, UI16 little-endian
    w.u8(0x00); w.u8(0x00); w.u8(0x00)         # RGB -- and nothing else:
    # a LineStyle1 has no matrix, which is why the first version of this wrote
    # 7 bits the format does not contain and the decoder read the NumFillBits
    # byte out of the middle of the record stream.
    w.u8(0x00)                       # NumFillBits=0, NumLineBits=0

    w.put(0, 1)                      # not an edge
    w.put(0x08, 5)                   # StateMoveTo
    w.put(0, 6)                      # reserved
    w.put(10, 5)                     # MoveBits
    w.s(0, 10); w.s(0, 10)

    for dx, dy in ((400, 0), (0, 400), (-400, 0), (0, -400)):
        w.put(1, 1)                  # edge
        w.put(1, 1)                  # straight
        w.put(8, 4)                  # NumBits - 2, so 10 bits
        if axis_aligned:
            # Axis-aligned: GeneralDeltaFlag is 0, then VerticalFlag. The flag
            # names the edge's orientation and the component stored is the one
            # that changes -- a *vertical* edge (VerticalFlag 1) moves along y and
            # stores DeltaY, a horizontal one stores DeltaX.
            #
            # The first version of this writer paired VerticalFlag=1 with dx, the
            # opposite, so the fixture encoded a shape that was not the square
            # and the decoder correctly decoded the wrong thing. That is why the
            # expected endpoints are stated here and asserted again in
            # swfshape_tests: a fixture that does not say what it should be is
            # worse than no fixture at all.
            w.put(0, 1)              # not general -> axis aligned
            if dy == 0:
                w.put(0, 1)          # horizontal: store DeltaX
                w.s(dx, 10)
            else:
                w.put(1, 1)          # vertical: store DeltaY
                w.s(dy, 10)
        else:
            w.put(1, 1)              # GeneralDelta
            w.s(dx, 10)
            w.s(dy, 10)

    w.put(0, 1)
    w.put(0, 5)
    w.put(0, 6)
    return w.bytes()




def build_square(axis_aligned=False):
    w = BW()
    w.u16le(1)                       # CharacterID, UI16 little-endian
    nb = 10
    w.put(nb, 5)
    for v in (0, 400, 0, 400):
        w.s(v, nb)

    w.u8(1)                          # FillStyleArray count
    w.u8(0)                          # type 0 = solid
    w.u8(0xFF); w.u8(0x00); w.u8(0x00)
    w.u8(1)                          # LineStyleArray count
    # Width is UI16 in every version. This second copy of the writer
    # is the one main() uses; the first was fixed and left behind.
    w.u8(20); w.u8(0x00)             # width, UI16 little-endian
    w.u8(0x00); w.u8(0x00); w.u8(0x00)
    # and nothing else -- a LineStyle1 has no matrix.
    w.u8(0x00)                       # NumFillBits=0, NumLineBits=0

    w.put(0, 1)                      # not an edge
    w.put(0x08, 5)                   # StateMoveTo
    w.put(0, 6)                      # reserved
    w.put(10, 5)                     # MoveBits
    w.s(0, 10); w.s(0, 10)

    for dx, dy in ((400, 0), (0, 400), (-400, 0), (0, -400)):
        w.put(1, 1)                  # edge
        w.put(1, 1)                  # straight
        w.put(8, 4)                  # NumBits - 2, so 10 bits
        if axis_aligned:
            # GeneralDeltaFlag is 0 for an axis-aligned edge, then
            # VerticalFlag. The flag names the orientation and the stored
            # component is the one that changes: a vertical edge moves along y
            # and stores DeltaY, a horizontal one stores DeltaX.
            #
            # The first version paired VerticalFlag=1 with dx, the opposite, so
            # the fixture was not the square and the decoder correctly decoded
            # the wrong thing. The expected endpoints are asserted in
            # swfshape_tests for exactly that reason.
            w.put(0, 1)              # not general -> axis aligned
            if dy == 0:
                w.put(0, 1)          # horizontal: store DeltaX
                w.s(dx, 10)
            else:
                w.put(1, 1)          # vertical: store DeltaY
                w.s(dy, 10)
        else:
            w.put(1, 1)              # GeneralDelta
            w.s(dx, 10)
            w.s(dy, 10)

    w.put(0, 1)                      # end of shape
    w.put(0, 5)
    w.put(0, 6)
    return w.bytes()


# The geometry each encoding must produce.
SQUARE = [(400, 0), (400, 400), (0, 400), (0, 0)]


def main():
    outdir = sys.argv[1] if len(sys.argv) > 1 else "."
    os.makedirs(outdir, exist_ok=True)
    for aa in (False, True):
        data = build_square(aa)
        name = f"swf_square_v1_aa{int(aa)}.bin"
        with open(os.path.join(outdir, name), "wb") as f:
            f.write(data)
        print(f"wrote {name}  {len(data)} bytes  {data.hex(' ')}")
    print(f"expected endpoints: {SQUARE}")


if __name__ == "__main__":
    main()
