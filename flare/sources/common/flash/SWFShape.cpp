#include "SWFShape.h"

#include <QObject>
#include <QStringList>
#include <cmath>

namespace SWF {

namespace {

// A big-endian bit reader, MSB first, which is the order the SWF specification
// states: "the bit order within bytes is big-endian: the most significant bit is
// stored first".
class BitReader {
public:
  BitReader(const unsigned char *d, int size) : m_d(d), m_size(size) {}

  bool eof() const { return m_bit >= m_size * 8; }
  int bitPos() const { return m_bit; }
  int bytePos() const { return m_bit / 8; }
  int bitsLeft() const { return m_size * 8 - m_bit; }

  bool bit(bool &out) {
    if (eof()) return false;
    const int byte = m_bit >> 3;
    const int off = 7 - (m_bit & 7);
    out = ((m_d[byte] >> off) & 1) != 0;
    ++m_bit;
    return true;
  }

  bool bits(int n, int &out) {
    out = 0;
    for (int i = 0; i < n; ++i) {
      bool b;
      if (!bit(b)) return false;
      out = (out << 1) | (b ? 1 : 0);
    }
    return true;
  }

  // A two's-complement signed value of n bits.
  bool sbits(int n, int &out) {
    if (n <= 0) { out = 0; return true; }
    if (n > 32) return false;
    int v;
    if (!bits(n, v)) return false;
    const int signBit = 1 << (n - 1);
    out = (v & signBit) ? (v - (1 << n)) : v;
    return true;
  }

  // An unsigned byte-aligned value, for the parts of the format that are not bit
  // packed: the style arrays and the header fields ahead of the records.
  // Only usable where the format really is byte aligned. Anything past the
  // RECT is bit-packed and must go through bit()/bits(), because reading it a
  // byte at a time is what produced a style array that parsed "successfully"
  // while consuming the wrong bits.
  bool ub(int &out) {
    if (m_bit % 8 != 0) return false;
    const int byte = m_bit >> 3;
    if (byte >= m_size) return false;
    out = m_d[byte];
    m_bit += 8;
    return true;
  }

  // Little-endian, as every integer in the format is: "the least significant
  // byte is stored first". Only the bit order *within* a byte is big-endian, and
  // reading a CharacterID the other way round turns 1 into 256 -- a value that
  // looks entirely reasonable and is wrong.
  bool ub16(int &out) {
    int lo, hi;
    if (!ub(lo) || !ub(hi)) return false;
    out = (hi << 8) | lo;
    return true;
  }

  bool sbits_fixed(int n, double &out) {
    int v;
    if (!sbits(n, v)) return false;
    out = v;
    return true;
  }

  // RECT: Nbits(5) then four signed fields of Nbits, XMin XMax YMin YMax.
  bool rect(QPointF &mn, QPointF &mx) {
    int nbits;
    if (!bits(5, nbits)) return false;
    if (nbits == 0) {                       // an empty rect is legal
      mn = QPointF(0, 0);
      mx = QPointF(0, 0);
      return true;
    }
    if (bitsLeft() < nbits * 4) return false;
    double xmin, xmax, ymin, ymax;
    if (!sbits_fixed(nbits, xmin)) return false;
    if (!sbits_fixed(nbits, xmax)) return false;
    if (!sbits_fixed(nbits, ymin)) return false;
    if (!sbits_fixed(nbits, ymax)) return false;
    mn = QPointF(xmin, ymin);
    mx = QPointF(xmax, ymax);
    return true;
  }

  // Round up to the next byte boundary. This is what the specification means by
  // "all integer types must be byte-aligned", and it applies after each RECT --
  // the RECT is the one bit-packed part of the header.
  //
  // Settled against JPEXS's parse of Ruffle's own DefineShape.swf: the RECT ends
  // at bit 61, and the fill-count byte at bit 64 reads 1 solid red fill, which is
  // what that shape is. Read straddling at bit 61 the same byte reads 0 fills and
  // the next reads 32 line styles, impossible in a 26-byte tag.
  //
  // An earlier version of this reader did no alignment at all. That was wrong, and
  // it looked right because the *flag word* was also being misread at the time, so
  // the count being examined was not the count the shape declares.
  // A style's length is not knowable to a bit reader. Everything of fixed width is
  // read a byte at a time, but a MATRIX and a GRADIENTRECORD are bit-packed and can
  // leave the stream mid-byte -- and the next style starts from wherever the style
  // array left off, not from the next boundary.
  //
  // Ruffle's structure explains what happens, because its reader is two-layered:
  // read_shape_styles reads through a byte `Reader`, while a matrix is read through
  // a temporary `BitReader` built over the same slice. That bit reader consumes the
  // matrix and advances the shared slice by *whole bytes*, so the partial byte is
  // gone by the time the next style is read. Rounding up here reproduces that.
  //
  // Measured over the 250 DefineShape tags in mario.ssf and checked field by field
  // against JPEXS's parse: without this the style arrays come out 2 to 6 bits short
  // on every shape whose fill carries a matrix, and 64 of 250 disagree. It is not a
  // property of the matrix -- two shapes with identical matrices can differ -- it is
  // simply the padding from wherever the previous style ended. Example: in one real
  // DefineShape the first bitmap fill ends on a byte boundary and needs no padding,
  // while the second ends 2 bits short and needs 2; in another the single fill ends
  // 4 bits short. Both then read the NumFillBits/NumLineBits byte at exactly the bit
  // JPEXS reads it.
  //
  // Called at the end of every fill and line style. For a solid fill it is a no-op:
  // a type byte plus three or four colour bytes always lands on a boundary, which is
  // why the 132 shapes whose only fill is solid already agreed.
  void alignToByte() {
    if (m_bit % 8) m_bit += 8 - (m_bit % 8);
  }

  // A byte-valued field at the current position. Byte *width*, not byte
  // *alignment*: a field following a RECT sits on the boundary alignToByte just
  // put it on, but a field following a matrix or a gradient is wherever that left
  // the stream.
  bool ub8(int &out) { return bits(8, out); }

private:
  const unsigned char *m_d;
  int m_size;
  int m_bit = 0;
};

// Skip one fill style. The geometry decoder needs to walk past these to reach the
// records, and a wrong stride here puts every subsequent record at the wrong bit
// while still producing plausible coordinates -- which is why this follows the
// specification field by field rather than estimating.
// FillStyleArray and LineStyleArray sit inside the bit stream, after a bit-packed
// RECT, so every field here is read through the bit reader. Reading them as
// bytes is the mistake this file made first: the array "parsed", consumed the
// wrong number of bits, and every record after it was read at the wrong offset --
// producing coordinates that look like geometry.
// Defined below: a DefineShape4 line style may carry a fill style.
bool skipFillStyle(BitReader &r, int version);

bool skipFillStyle(BitReader &r, int version) {
  int type;
  if (!r.ub8(type)) return false;

  const auto rgb = [&r]() {
    int a, b, c;
    return r.ub8(a) && r.ub8(b) && r.ub8(c);
  };
  const auto rgba = [&r]() {
    int a, b, c, d;
    return r.ub8(a) && r.ub8(b) && r.ub8(c) && r.ub8(d);
  };
  const auto matrix = [&r]() {
    // A MATRIX: an optional scale, an optional rotation, then a translation. Each
    // optional part carries its own bit width and *two* signed values -- the two
    // components. This read only one of the scale pair, which put everything after
    // a scaled fill or gradient a whole nScaleBits out of step. That is why 111 of
    // 250 real shapes stopped in the style arrays: this decoder read the same fill
    // count as JPEXS, then could not walk the fills it had been given.
    bool hasScale;
    if (!r.bit(hasScale)) return false;
    if (hasScale) {
      int n;
      if (!r.bits(5, n)) return false;
      for (int i = 0; i < 2; ++i) {
        int v;
        if (!r.sbits(n, v)) return false;
      }
    }
    bool hasRotate;
    if (!r.bit(hasRotate)) return false;
    if (hasRotate) {
      int n;
      if (!r.bits(5, n)) return false;
      for (int i = 0; i < 2; ++i) {
        int v;
        if (!r.sbits(n, v)) return false;
      }
    }
    int n;
    if (!r.bits(5, n)) return false;
    for (int i = 0; i < 2; ++i) {
      int v;
      if (!r.sbits(n, v)) return false;
    }
    // The matrix is the last bit-packed field in a style, and what follows it is
    // read a byte at a time -- so the padding belongs here, not at the end of the
    // style. Reading the next byte straddling instead of from the boundary gives a
    // different *value*, not merely a different position: on a real DefineShape3
    // whose matrix ends at bit 130, the gradient's flags byte read there is 0x00,
    // saying a gradient with no records, while read at the boundary (136) it is
    // 0x04, saying the four records the gradient actually has.
    r.alignToByte();
    return true;
  };

  // A gradient. The matrix comes first, then a single flags byte, then the
  // records.
  //
  // Three things here were wrong, and the file settles all of them. The order was
  // gradient-then-matrix, which is what an earlier draft assumed and what neither
  // Ruffle nor JPEXS reads: with that order 25 of the 30 gradient fills here read a
  // line-style count in the hundreds. The flags byte holds spread, interpolation
  // and the record count together -- there is no separate two-bit and four-bit
  // field after it. And each record is one ratio byte plus the colour, not two
  // ratio bytes and an interpolation table; the table was invented here and cost
  // four bits plus an entry per record.
  //
  // A gradient with zero records is real -- 4 records, 2 records and none all occur
  // in this one file -- and costs only its flags byte. Ruffle notes the same for
  // malformed files and reads nothing further.
  const auto gradient = [&r, version](bool focal) {
    int flags;
    if (!r.ub8(flags)) return false;
    const int numRecords = flags & 0x0F;
    if (numRecords == 0) return true;
    const int colourBytes = version >= 3 ? 4 : 3;
    for (int i = 0; i < numRecords; ++i) {
      int ratio;
      if (!r.ub8(ratio)) return false;
      for (int c = 0; c < colourBytes; ++c) {
        int v;
        if (!r.ub8(v)) return false;
      }
    }
    return true;
  };

  switch (type) {
    case 0x00:
      if (!(version >= 3 ? rgba() : rgb())) return false;
      r.alignToByte();
      return true;
    case 0x10:                                        // linear
    case 0x12:                                        // radial
      if (!matrix()) return false;
      if (!gradient(false)) return false;
      r.alignToByte();
      return true;
    case 0x13: {                                      // focal
      if (!matrix()) return false;
      if (!gradient(true)) return false;
      int fx, fy;                                    // FocalPoint, FIXED8
      if (!r.ub8(fx) || !r.ub8(fy)) return false;
      r.alignToByte();
      return true;
    }
    // Clipped and tiled bitmap fills: a CharacterID then a matrix.
    case 0x40: case 0x41: case 0x42: case 0x43: {
      int idHi, idLo;
      if (!r.ub8(idHi) || !r.ub8(idLo)) return false;
      if (!matrix()) return false;
      r.alignToByte();
      return true;
    }
    default:
      // An unknown type cannot be skipped, because its length is not knowable.
      // Continuing would resume mid-field and produce a shape that looks
      // drawn but is not, so the tag is refused instead.
      return false;
  }
}

bool skipLineStyle(BitReader &r, int version) {
  // Width is UI16 for every shape version. The earlier version of this read a
  // leading byte and then a further UI16 for version >= 2, consuming 24 bits
  // where the format has 16 -- so every shape with a line style desynchronised
  // from that point on and produced coordinates that were plausible and wrong.
  // That is the whole of why 118 of 250 real shapes decoded while 98 of the
  // rest placed their outline outside the bounds the same tag declared.
  int lo, hi;
  if (!r.ub8(lo) || !r.ub8(hi)) return false;

  if (version < 4) {
    // LineStyle1: RGB, or RGBA from DefineShape3. Five or six whole bytes, so it
    // always lands on a boundary and there is nothing to pad.
    if (version >= 3) {
      int a, b, c, d;
      if (!r.ub8(a) || !r.ub8(b) || !r.ub8(c) || !r.ub8(d)) return false;
    } else {
      int a, b, c;
      if (!r.ub8(a) || !r.ub8(b) || !r.ub8(c)) return false;
    }
    return true;
  }

  // DefineShape4's LineStyle2: a UI16 of flags, an optional miter limit, and
  // a paint that is either a fill style or a plain colour. Skipping a fixed
  // number of bytes is not enough -- the optional fields make the stride depend
  // on the flags, so the fields are walked rather than assumed.
  int fhi, flo;
  if (!r.ub8(fhi) || !r.ub8(flo)) return false;
  const int flags = (fhi << 8) | flo;
  if (flags & 0x08) {                 // miter limit, for the miter join style
    int ml, mh;
    if (!r.ub8(ml) || !r.ub8(mh)) return false;
  }
  if (flags & 0x10) return skipFillStyle(r, version);  // a fill, not a colour
  int a, b, c, d;
  if (!r.ub8(a) || !r.ub8(b) || !r.ub8(c) || !r.ub8(d)) return false;
  r.alignToByte();
  return true;
}


// The style arrays, and the two bit widths the records use. Read even though the
// geometry does not need the styles: they are ahead of the records, and skipping
// them by estimate would shift every record.
bool readStyles(BitReader &r, int version, int &numFill, int &numLine,
                int &numFillBits, int &numLineBits) {
  int n;
  if (!r.ub8(n)) return false;
  if (n == 0xFF && version >= 2) {
    int hi, lo;
    if (!r.bits(8, lo) || !r.bits(8, hi)) return false;   // UI16, little-endian
    n = (hi << 8) | lo;
  }
  numFill = n;
  for (int i = 0; i < numFill; ++i)
    if (!skipFillStyle(r, version)) return false;

  if (!r.ub8(n)) return false;
  if (n == 0xFF && version >= 2) {
    int hi, lo;
    if (!r.ub8(lo) || !r.ub8(hi)) return false;
    n = (hi << 8) | lo;
  }
  numLine = n;
  for (int i = 0; i < numLine; ++i)
    if (!skipLineStyle(r, version)) return false;

  // One byte holding both widths: the high nibble the fill bits, the low nibble
  // the line bits.
  int both;
  if (!r.ub8(both)) return false;
  numFillBits = both >> 4;
  numLineBits = both & 0x0F;
  return true;
}

// Format a twip coordinate: integral values plainly, fractions to at most three
// decimals with no trailing zeros.
QString num(double v) {
  const double r = std::round(v * 1000.0) / 1000.0;
  if (std::fabs(r - std::round(r)) < 1e-9)
    return QString::number(static_cast<qlonglong>(std::round(r)));
  return QString::number(r, 'f', 3);
}

}  // namespace

int Shape::segmentCount() const {
  int n = 0;
  for (const Contour &c : contours) n += c.segments.size();
  return n;
}

Shape decodeShape(const unsigned char *data, int size, int version) {
  Shape out;
  out.version = version;
  if (!data || size <= 0) {
    out.error = QObject::tr("empty shape tag");
    return out;
  }
  if (version < 1 || version > 4) {
    out.error = QObject::tr("unsupported shape version %1").arg(version);
    return out;
  }

  BitReader r(data, size);
  // Byte aligned: nothing precedes it in the tag. Still read through the bit
  // reader so there is one code path, and so the little-endian order is applied
  // in one place.
  if (!r.ub16(out.id)) {
    out.error = QObject::tr("truncated: no CharacterID");
    return out;
  }
  if (!r.rect(out.boundsMin, out.boundsMax)) {
    out.error = QObject::tr("truncated: no ShapeBounds");
    return out;
  }
  out.edgeMin = out.boundsMin;
  out.edgeMax = out.boundsMax;
  r.alignToByte();

  // DefineShape4 carries EdgeBounds separately, and a flag byte after it.
  if (version >= 4) {
    if (!r.rect(out.edgeMin, out.edgeMax)) {
      out.error = QObject::tr("truncated: no EdgeBounds");
      return out;
    }
    out.hasEdgeBounds = true;
    r.alignToByte();
    int flags;
    if (!r.ub8(flags)) {
      out.error = QObject::tr("truncated: no Shape4 flag byte");
      return out;
    }
  }

  if (!readStyles(r, version, out.numFillStyles, out.numLineStyles,
                  out.numFillBits, out.numLineBits)) {
    out.error = QObject::tr("truncated in the style arrays");
    return out;
  }
  // The bit widths change mid-shape when a new style array arrives, so these are
  // read from the Shape rather than copied, and kept in step below.
  int numFillBits = out.numFillBits, numLineBits = out.numLineBits;

  // ---- the record stream --------------------------------------------------
  //
  // The current point, and the contour being built. A move starts a new one; an
  // edge extends the current one. Closure is implicit -- the format has no closing
  // segment and Flash closes every contour it fills.
  QPointF cur;
  Contour contour;
  bool haveContour = false;

  const auto flush = [&]() {
    if (haveContour && !contour.segments.isEmpty())
      out.contours.push_back(contour);
    contour = Contour();
    haveContour = false;
  };

  while (true) {
    if (r.bitsLeft() < 1) {
      // Ran out mid-stream. What has been read so far is a partial outline, and
      // drawing it would put a shape on screen that is not the one in the file,
      // so it is discarded rather than reported as a decode with a warning.
      out.error = QObject::tr("truncated in the shape records at bit %1")
                      .arg(r.bitPos());
      return out;
    }

    bool isEdge;
    if (!r.bit(isEdge)) {
      out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
      return out;
    }

    if (isEdge) {
      bool straight;
      if (!r.bit(straight)) {
        out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
        return out;
      }
      int numBitsRaw;
      if (!r.bits(4, numBitsRaw)) {
        out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
        return out;
      }
      const int numBits = numBitsRaw + 2;

      Segment seg;
      if (straight) {
        // The bit is GeneralDeltaFlag, and it means the opposite of what the
        // local name suggested: 1 means *general* (both components present), 0
        // means axis-aligned (only one present). Reading it as "axisAligned"
        // without the negation swapped the two, so every ordinary edge was
        // decoded as if it were axis-aligned -- which produced a segment at
        // (-224, 0) where the square wanted (400, 0), and still terminated
        // cleanly at the end-of-shape record.
        bool generalDelta;
        if (!r.bit(generalDelta)) {
          out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
          return out;
        }
        const bool axisAligned = !generalDelta;
        // Axis-aligned edges store only the component that changes. The other is
        // zero -- *not* "the same as the previous edge". Reading it as anything
        // else yields plausible-looking coordinates, which is how a decoder of
        // this format goes wrong quietly.
        bool vertical = false;
        if (axisAligned) {
          if (!r.bit(vertical)) {
            out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
            return out;
          }
        }
        int dx = 0, dy = 0;
        if (!axisAligned || !vertical) {
          if (!r.sbits(numBits, dx)) {
            out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
            return out;
          }
        }
        if (!axisAligned || vertical) {
          if (!r.sbits(numBits, dy)) {
            out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
            return out;
          }
        }
        seg.curve = false;
        seg.end = cur + QPointF(dx, dy);
      } else {
        // A quadratic segment: a control point and an anchor, both relative to
        // the current point.
        int cx, cy, ax, ay;
        if (!r.sbits(numBits, cx) || !r.sbits(numBits, cy) ||
            !r.sbits(numBits, ax) || !r.sbits(numBits, ay)) {
          out.error = QObject::tr("truncated curve at bit %1").arg(r.bitPos());
          return out;
        }
        seg.curve = true;
        seg.control = cur + QPointF(cx, cy);
        seg.end = cur + QPointF(ax, ay);
      }

      if (!haveContour) {
        // An edge with no preceding move: the contour starts at the current
        // point, which the specification initialises to (0, 0) and which a
        // well-formed file never relies on.
        contour.start = cur;
        haveContour = true;
      }
      cur = seg.end;
      contour.segments.push_back(seg);
      ++out.records;
      continue;
    }

    // A style change record: five state bits and then the payload -- no reserved
    // field. Counting the records JPEXS reports for Ruffle's DefineShape.swf: with
    // six reserved bits they need 97 and only 88 bits remain, so the walk runs off
    // the end; with none they need 85 and fit. Ruffle's read_shape_record likewise
    // reads five bits and goes straight to MoveBits.
    //
    // The five bits are a little-endian bitfield of the value read MSB-first, so
    // MoveTo is the *first* bit and NewStyles the last. Reading them as bits 3 to 7
    // of the value -- which is what an earlier version here did -- left three of
    // the five permanently false, because a five-bit field cannot hold bits 5, 6 or
    // 7. FillStyle1, LineStyle and NewStyles were dead code and MoveTo fired
    // exactly when a line style was selected, so no real move was ever seen.
    //
    // Checked against Ruffle's ShapeRecordFlag, and then measured: the outline of
    // a DefineShape must lie inside the bounds that same tag declares, which is
    // strong enough to settle a bit layout without trusting either.
    int flagsRaw;
    if (!r.bits(5, flagsRaw)) {
      out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
      return out;
    }
    if (flagsRaw == 0) break;          // all five clear: end of shape

    const bool stateMoveTo     = flagsRaw & 0x01;
    const bool stateFillStyle0 = flagsRaw & 0x02;
    const bool stateFillStyle1 = flagsRaw & 0x04;
    const bool stateLineStyle  = flagsRaw & 0x08;
    const bool stateNewStyles  = flagsRaw & 0x10;

    if (stateMoveTo) {
      int moveBits;
      if (!r.bits(5, moveBits)) {
        out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
        return out;
      }
      int mx = 0, my = 0;
      if (moveBits > 0) {
        if (!r.sbits(moveBits, mx) || !r.sbits(moveBits, my)) {
          out.error = QObject::tr("truncated move at bit %1").arg(r.bitPos());
          return out;
        }
      }
      // A move is absolute, not relative to the current point.
      flush();
      cur = QPointF(mx, my);
      contour.start = cur;
      haveContour = true;
    }

    // The fill and line indices are read and discarded: they index the style
    // arrays, and this decoder produces geometry only. They still have to be
    // consumed or the next record starts at the wrong bit.
    // One index per flag that is set, FillStyle0's first. Reading only one when
    // both are set left the next record starting in the middle of the second
    // index, which is the same silent desynchronisation as any other miscount.
    if (stateFillStyle0 || stateFillStyle1) {
      const int wanted = (stateFillStyle0 ? 1 : 0) + (stateFillStyle1 ? 1 : 0);
      for (int k = 0; k < wanted; ++k) {
        int idx;
        if (!r.bits(numFillBits, idx)) {
          out.error =
              QObject::tr("truncated fill index at bit %1").arg(r.bitPos());
          return out;
        }
      }
    }
    if (stateLineStyle) {
      int idx;
      if (!r.bits(numLineBits, idx)) {
        out.error = QObject::tr("truncated line index at bit %1").arg(r.bitPos());
        return out;
      }
    }
    if (stateNewStyles) {
      // A new style array, and new bit widths with it. Byte aligned, for the same
      // reason as the first one: Ruffle builds a fresh byte reader over the bit
      // reader's position here, which drops the bits left over from the record.
      r.alignToByte();
      int nf = 0, nl = 0;
      if (!readStyles(r, version, nf, nl, out.numFillBits, out.numLineBits)) {
        out.error = QObject::tr("truncated in a mid-shape style array at "
                                "bit %1")
                        .arg(r.bitPos());
        return out;
      }
      numFillBits = out.numFillBits;
      numLineBits = out.numLineBits;
    }

    ++out.records;
  }

  flush();
  out.ok = true;
  return out;
}

Shape decodeShape(const QByteArray &data, int version) {
  return decodeShape(reinterpret_cast<const unsigned char *>(data.constData()),
                     static_cast<int>(data.size()), version);
}

QString toSvgPath(const Shape &shape) {
  if (!shape.ok || shape.contours.isEmpty()) return QString();

  // The viewBox is the shape's own bounds. EdgeBounds is the outline's extent
  // and ShapeBounds includes the stroke width, so EdgeBounds is the tighter fit
  // where DefineShape4 provides it; for the earlier versions they are one value.
  const QPointF mn = shape.hasEdgeBounds ? shape.edgeMin : shape.boundsMin;
  const QPointF mx = shape.hasEdgeBounds ? shape.edgeMax : shape.boundsMax;

  QStringList d;
  for (const Contour &c : shape.contours) {
    d << QStringLiteral("M") + num(c.start.x()) + QLatin1Char(' ') +
           num(c.start.y());
    for (const Segment &s : c.segments) {
      if (s.curve) {
        d << QStringLiteral("Q") + num(s.control.x()) + QLatin1Char(' ') +
               num(s.control.y()) + QLatin1Char(' ') + num(s.end.x()) +
               QLatin1Char(' ') + num(s.end.y());
      } else {
        d << QStringLiteral("L") + num(s.end.x()) + QLatin1Char(' ') +
               num(s.end.y());
      }
    }
    // Z, not an explicit line back to the start: the format stores no closing
    // segment, so emitting one as a line would make the segment count disagree
    // with the record count that produced it.
    d << QStringLiteral("Z");
  }

  double w = mx.x() - mn.x(), h = mx.y() - mn.y();
  if (w <= 0) w = 1;
  if (h <= 0) h = 1;

  return QStringLiteral("viewBox=\"%1 %2 %3 %4\"")
             .arg(num(mn.x()), num(mn.y()), num(w), num(h)) +
         QStringLiteral(" d=\"") + d.join(QLatin1Char(' ')) +
         QStringLiteral("\"");
}

QString toSvgDocument(const Shape &shape) {
  if (!shape.ok || shape.contours.isEmpty()) return QString();
  const QString path = toSvgPath(shape);
  if (path.isEmpty()) return QString();

  // Stroked, not filled: this decoder reads outlines, not styles, so asserting a
  // fill would claim a paint it never looked at.
  //
  // The d= attribute appears once inside toSvgPath(); recover it for the <path>
  // body rather than printing the viewBox a second time as a path.
  const int dAt = path.indexOf(QLatin1String(" d=\""));
  const QString dAttr = dAt >= 0 ? path.mid(dAt + 4, path.size() - dAt - 5)
                                 : QString();

  return QStringLiteral(
             "<svg xmlns=\"http://www.w3.org/2000/svg\" %1 "
             "fill=\"none\" stroke=\"#000\" stroke-width=\"1\">"
             "<path d=\"%2\"/></svg>")
      .arg(path, dAttr);
}

}  // namespace SWF
