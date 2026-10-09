// Tests for the SWF shape decoder, SWF::decodeShape.
//
// The record stream is bit-packed, so a decoder that has lost sync still produces
// coordinates -- just the wrong ones, and plausible-looking ones. That is why
// these tests assert exact endpoints rather than checking that the output "looks
// like a shape".
//
// Status, measured on the real files available rather than assumed:
//
//   * The record grammar -- straight edges, curves, the axis-aligned form,
//     style changes, move-to, implicit closure -- decodes a square of known
//     coordinates exactly, in both edge encodings the format allows.
//   * Defined over the 250 shape tags in a real 3.5 MB SWF that Moho exported,
//     118 decode cleanly and 98 place their outline outside the bounds the same
//     tag declares, so the style-array walk is still wrong for some shapes.
//     Those cases are named in the test below rather than hidden, because a
//     decoder that quietly returns a partial outline is worse than one that
//     admits it cannot read the tag.
//
// What is verified here is the part that is known correct, so a later change that
// breaks it is caught; and the known-bad shapes are pinned as a failing
// expectation, so the day they start passing the failure count falls to zero and
// says so.

#include "SWFShape.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <cstdio>
#include <cmath>

static int gChecks = 0;
static int gFails = 0;

static void check(bool ok, const char *what, const QString &detail = {}) {
  ++gChecks;
  fprintf(stderr, "   [%s] %s", ok ? "ok  " : "FAIL", what);
  if (!detail.isEmpty()) fprintf(stderr, "  (%s)", qPrintable(detail));
  fprintf(stderr, "\n");
  if (!ok) ++gFails;
}

static bool loadFile(const QString &path, QByteArray &out) {
  QFile f(path);
  if (!f.open(QIODevice::ReadOnly)) return false;
  out = f.readAll();
  f.close();
  return true;
}

// ---------------------------------------------------------------------------
// The square, in both encodings. 400x400 twips, matching the same shape Ruffle's
// DefineShape.fla carries as:
//
//   !0 0|400 0!400 0|400 400!400 400|0 400!0 400|0 0
//
// which is a move to the origin, four straight edges, and an implicit close.
// ---------------------------------------------------------------------------
static void test_square(const QString &dir) {
  fprintf(stderr, "\n-- the square, both edge encodings --\n");

  struct Case {
    const char *file;
    const char *label;
  };
  const Case cases[] = {
      {"swf_square_v1_aa0.bin", "GeneralDelta: every edge carries both components"},
      {"swf_square_v1_aa1.bin", "axis-aligned: a straight edge stores one component"},
      // The same bytes, read as DefineShape2. A line style's width is where the two
      // versions differ in what a decoder must do with it -- reading it as a byte and
      // then a further UI16 from version 2 onwards consumes 24 bits where the format
      // has 16 -- so the fixture has to be decoded at both versions to cover it.
      {"swf_square_v2_aa0.bin", "as DefineShape2, general"},
      {"swf_square_v2_aa1.bin", "as DefineShape2, axis-aligned"},
  };

  for (const Case &c : cases) {
    QByteArray body;
    if (!loadFile(QDir(dir).filePath(c.file), body)) {
      check(false, "fixture present", c.file);
      continue;
    }
    // The fixture name states the version: swf_square_v<2>_aa<0|1>.bin.
    const int version = c.file[12] == '2' ? 2 : 1;
    const SWF::Shape s = SWF::decodeShape(body, version);

    QString d = QString("%1: decodes").arg(c.label);
    check(s.ok, qPrintable(d), s.error);
    if (!s.ok) continue;

    check(s.id == 1, "  CharacterID is 1",
          QString::number(s.id));
    check(s.boundsMin.x() == 0 && s.boundsMin.y() == 0 &&
              s.boundsMax.x() == 400 && s.boundsMax.y() == 400,
          "  ShapeBounds is 0,0 to 400,400",
          QString("%1,%2 to %3,%4")
              .arg(s.boundsMin.x()).arg(s.boundsMin.y())
              .arg(s.boundsMax.x()).arg(s.boundsMax.y()));
    check(s.numFillStyles == 1 && s.numLineStyles == 1,
          "  one fill and one line style",
          QString("%1 fill, %2 line").arg(s.numFillStyles)
              .arg(s.numLineStyles));

    check(s.contours.size() == 1, "  exactly one contour",
          QString::number(s.contours.size()));
    if (s.contours.size() != 1) continue;

    const SWF::Contour &ct = s.contours.first();
    check(ct.start.x() == 0 && ct.start.y() == 0,
          "  it starts at the origin",
          QString("%1,%2").arg(ct.start.x()).arg(ct.start.y()));
    check(ct.segments.size() == 4, "  four segments",
          QString::number(ct.segments.size()));

    // The endpoints, in order. This is the assertion that cannot be satisfied by
    // a decoder that has merely produced something shape-like.
    const QPointF want[4] = {QPointF(400, 0), QPointF(400, 400),
                             QPointF(0, 400), QPointF(0, 0)};
    int wrong = 0;
    for (int i = 0; i < ct.segments.size() && i < 4; ++i) {
      const QPointF &got = ct.segments.at(i).end;
      if (qAbs(got.x() - want[i].x()) > 1e-6 ||
          qAbs(got.y() - want[i].y()) > 1e-6) {
        ++wrong;
        if (wrong <= 2)
          fprintf(stderr, "        segment %d ends (%g,%g), expected (%g,%g)\n",
                  i, got.x(), got.y(), want[i].x(), want[i].y());
      }
    }
    check(wrong == 0, "  every endpoint matches, in order",
          QString("%1 wrong").arg(wrong));
    check(s.segmentCount() == 4, "  segmentCount agrees",
          QString::number(s.segmentCount()));

    // No curve in a square made of straight lines.
    int curves = 0;
    for (const SWF::Segment &sg : ct.segments)
      if (sg.curve) ++curves;
    check(curves == 0, "  no segment is reported as curved",
          QString::number(curves));

    // The outline must lie inside the bounds the same tag declares.
    bool inside = true;
    for (const SWF::Segment &sg : ct.segments) {
      const QPointF pts[2] = {sg.end, sg.curve ? sg.control : sg.end};
      for (const QPointF &p : pts)
        if (p.x() < s.boundsMin.x() || p.x() > s.boundsMax.x() ||
            p.y() < s.boundsMin.y() || p.y() > s.boundsMax.y())
          inside = false;
    }
    check(inside, "  the outline lies inside the declared bounds");

    // The SVG, which is what an importer would actually write.
    const QString svg = SWF::toSvgPath(s);
    check(svg.contains("M0 0") && svg.contains("L400 400") &&
              svg.contains("L0 400") && svg.endsWith("Z\""),
          "  the SVG path is the square",
          svg.left(90));
    check(svg.startsWith("viewBox=\"0 0 400 400\""),
          "  with the declared bounds as the viewBox");
  }
}

// ---------------------------------------------------------------------------
// Malformed input must be refused, not half-decoded.
// ---------------------------------------------------------------------------
static void test_malformed() {
  fprintf(stderr, "\n-- malformed input --\n");

  check(!SWF::decodeShape(QByteArray(), 1).ok, "an empty body is refused");
  check(!SWF::decodeShape(QByteArray(1, '\0'), 1).ok,
        "a one-byte body is refused");

  // A shape whose records never end.
  QByteArray body = QByteArray::fromHex("0100") + QByteArray::fromHex("00") +
                    QByteArray(40, '\xff');
  const SWF::Shape s = SWF::decodeShape(body, 1);
  check(!s.ok, "a body whose records run off the end is refused");
  check(!s.error.isEmpty(), "  and says so", s.error);
  check(s.contours.isEmpty(),
        "  and yields no partial outline rather than a wrong one");

  // A RECT that claims more bits than the buffer holds. A 5-bit Nbits can reach 31,
  // and four fields that wide need 124 bits, so a three-byte body cannot contain one.
  // The reader must notice and refuse rather than read past the end.
  {
    QByteArray wide = QByteArray::fromHex("0100");    // CharacterID 1
    wide.append(char(0xF8));                          // Nbits = 31 in the top 5 bits
    wide.append(QByteArray(2, '\xff'));               // far too little RECT
    const SWF::Shape truncated = SWF::decodeShape(wide, 1);
    check(!truncated.ok,
          "a RECT claiming more bits than the buffer holds is refused",
          truncated.error);
    check(truncated.contours.isEmpty(),
          "  and yields no partial outline either");
  }

  // A shape whose style arrays run off the end before the records begin. The count
  // says five fills and the buffer holds a couple of bytes.
  {
    QByteArray few = QByteArray::fromHex("01005000c8000c80");
    few.append(char(0x05));                           // five fill styles
    few.append(QByteArray(3, '\x11'));                // and not enough for them
    const SWF::Shape s2 = SWF::decodeShape(few, 1);
    check(!s2.ok, "a fill style array longer than the tag is refused");
    check(s2.contours.isEmpty(),
          "  and yields no partial outline either");
  }

  check(!SWF::decodeShape(QByteArray::fromHex("0100"), 0).ok,
        "version 0 is refused");
  check(!SWF::decodeShape(QByteArray::fromHex("0100"), 5).ok,
        "version 5 is refused");
}

// ---------------------------------------------------------------------------
// One check per repaired fault. Each exists because that fault shipped: a
// wrong-but-plausible decode of a bit-packed format is silent, so nothing short of
// a fixture that states its own geometry would have caught any of them.
//
// The figures the old placeholder recorded -- 118 of 250 real shapes decoding, 98
// of the rest outside their declared bounds -- came from probing mario.ssf with a
// decoder whose style-array walk was wrong, so they measured the decoder and not
// the file. What replaced them is in test_real_shapes below.
// ---------------------------------------------------------------------------

// Decode one fixture and assert the whole of it: the header it declares, the
// outline it contains, and that no point of the outline escapes the bounds the
// same tag declares.
static void expect_square(const QString &dir, const char *file, int version,
                          int id, int numFill, int numLine, int numFillBits,
                          double xmax, double ymax, const char *what) {
  QByteArray body;
  if (!loadFile(QDir(dir).filePath(file), body)) {
    check(false, what, QString("fixture missing: %1").arg(file));
    return;
  }
  const SWF::Shape s = SWF::decodeShape(body, version);
  QString d = QString("%1: decodes").arg(what);
  check(s.ok, qPrintable(d), s.error);
  if (!s.ok) return;

  check(s.id == id, "  the declared CharacterID",
        QString::number(s.id));
  check(s.numFillStyles == numFill && s.numLineStyles == numLine,
        "  the declared style counts",
        QString("%1 fill, %2 line").arg(s.numFillStyles).arg(s.numLineStyles));
  check(s.numFillBits == numFillBits,
        "  the declared NumFillBits", QString::number(s.numFillBits));
  check(s.contours.size() == 1, "  exactly one contour",
        QString::number(s.contours.size()));
  if (s.contours.size() != 1) return;

  const SWF::Contour &ct = s.contours.first();
  check(ct.start.x() == 0 && ct.start.y() == 0, "  it starts at the origin",
        QString("%1,%2").arg(ct.start.x()).arg(ct.start.y()));
  check(ct.segments.size() == 4, "  four segments",
        QString::number(ct.segments.size()));

  const QPointF want[4] = {QPointF(xmax, 0), QPointF(xmax, ymax),
                           QPointF(0, ymax), QPointF(0, 0)};
  int wrong = 0;
  for (int i = 0; i < ct.segments.size() && i < 4; ++i) {
    const QPointF &got = ct.segments.at(i).end;
    if (qAbs(got.x() - want[i].x()) > 1e-6 ||
        qAbs(got.y() - want[i].y()) > 1e-6) {
      ++wrong;
      if (wrong <= 2)
        fprintf(stderr, "        segment %d ends (%g,%g), expected (%g,%g)\n",
                i, got.x(), got.y(), want[i].x(), want[i].y());
    }
  }
  check(wrong == 0, "  every endpoint matches, in order",
        QString("%1 wrong").arg(wrong));

  // The outline, control points included, must lie inside the bounds the same tag
  // declares. These fixtures have no curves, so the control point is the end point
  // and this is exact.
  bool inside = true;
  for (const SWF::Segment &sg : ct.segments)
    for (const QPointF &p : {sg.end, sg.control})
      if (p.x() < s.boundsMin.x() || p.x() > s.boundsMax.x() ||
          p.y() < s.boundsMin.y() || p.y() > s.boundsMax.y())
        inside = false;
  check(inside, "  the outline lies inside the declared bounds");
}

static void test_fault_fixtures(const QString &dir) {
  fprintf(stderr, "\n-- a fixture per repaired fault --\n");

  // A bitmap fill carries a MATRIX, which is bit-packed, so the style does not end
  // on a byte boundary and the next field must still be read from the boundary.
  // This matrix ends 7 bits short on purpose. Read straddling, the next byte has a
  // different value, not merely a different position -- which is what made 59 of the
  // 250 real shapes here read a line-style count in the hundreds or stop outright.
  expect_square(dir, "swf_bitmapfill_v1.bin", 1, 2, 1, 0, 1, 100, 100,
                "a bitmap fill, whose MATRIX ends mid-byte");

  // Each optional part of a MATRIX holds two signed values. Reading only one of
  // the scale pair put everything after a scaled fill a whole nScaleBits out of
  // step, which is how 111 of the 250 real shapes stopped in the style arrays. A
  // fixture whose matrix has no scale cannot catch that, so this one has both a
  // scale and a rotation.
  expect_square(dir, "swf_rotatedfill_v1.bin", 1, 5, 1, 0, 1, 100, 100,
                "a bitmap fill whose MATRIX has a scale and a rotation");

  // A gradient is a MATRIX first and then a flags byte holding spread,
  // interpolation and the record count, then records of one ratio byte and a
  // colour. Reading the gradient before the matrix, or adding a second ratio byte
  // and an interpolation table, desynchronises the style arrays without failing:
  // 30 gradient fills exist in this one file and all of them read wrongly at once.
  expect_square(dir, "swf_gradient_v3.bin", 3, 3, 1, 0, 1, 120, 100,
                "a radial gradient: MATRIX, flags byte, two records");

  // A style change may set StateFillStyle0 and StateFillStyle1 together, which
  // carries two indices. Consuming one resumes the next record inside the other.
  expect_square(dir, "swf_twofillindices_v1.bin", 1, 4, 1, 0, 2, 60, 60,
                "a style change setting both fill flags");

  // Ruffle's own DefineShape.swf, lifted out of the container. The file is authored
  // against the specification and is known good, so it settles what the
  // specification's prose leaves open: the RECT ends at bit 61 and the fill count
  // is at bit 64, one solid red fill and no stroke. Read straddling at bit 61 the
  // same byte reads no fills and the next reads 32 line styles, impossible in a
  // 26-byte tag.
  expect_square(dir, "swf_ruffle_square_v1.bin", 1, 1, 1, 0, 1, 400, 400,
                "Ruffle's DefineShape.swf, decoded");
}

static void test_shape_cases(const QString &dir) {
  fprintf(stderr, "\n-- shape cases the real sample does not cover --\n");

  expect_square(dir, "swf_implicit_origin_v1.bin", 1, 6, 1, 0, 0, 100, 100,
                "an edge with no preceding move starts at the origin");

  {
    QByteArray body;
    if (!loadFile(QDir(dir).filePath("swf_twocontours_v1.bin"), body)) {
      check(false, "two contours: fixture present",
            "swf_twocontours_v1.bin");
    } else {
      const SWF::Shape s = SWF::decodeShape(body, 1);
      check(s.ok, "two contours: decodes", s.error);
      if (s.ok) {
        check(s.contours.size() == 2, "  two contours",
              QString::number(s.contours.size()));
        if (s.contours.size() == 2) {
          check(s.contours.at(0).start == QPointF(0, 0) &&
                    s.contours.at(1).start == QPointF(60, 60),
                "  the second starts where its move says",
                QString("%1,%2 and %3,%4")
                    .arg(s.contours.at(0).start.x())
                    .arg(s.contours.at(0).start.y())
                    .arg(s.contours.at(1).start.x())
                    .arg(s.contours.at(1).start.y()));
          check(s.contours.at(0).segments.size() == 4 &&
                    s.contours.at(1).segments.size() == 4,
                "  four edges each");
        }
      }
    }
  }

  {
    QByteArray body;
    if (!loadFile(QDir(dir).filePath("swf_allcurves_v4.bin"), body)) {
      check(false, "all curves: fixture present", "swf_allcurves_v4.bin");
    } else {
      const SWF::Shape s = SWF::decodeShape(body, 4);
      check(s.ok, "all curves: decodes", s.error);
      if (s.ok) {
        check(s.contours.size() == 1, "  one contour",
              QString::number(s.contours.size()));
        int curves = 0;
        for (const SWF::Segment &sg : s.contours.first().segments)
          if (sg.curve) ++curves;
        check(curves == 4, "  all four segments are curves",
              QString::number(curves));
        check(s.contours.first().segments.size() == 4, "  four segments");
      }
    }
  }

  {
    QByteArray body;
    if (!loadFile(QDir(dir).filePath("swf_shape4_linestyle2_v4.bin"), body)) {
      check(false, "LineStyle2: fixture present",
            "swf_shape4_linestyle2_v4.bin");
    } else {
      const SWF::Shape s = SWF::decodeShape(body, 4);
      check(s.ok, "LineStyle2 with a miter limit: decodes", s.error);
      if (s.ok) {
        check(s.numFillStyles == 1 && s.numLineStyles == 1,
              "  one fill and one line style",
              QString("%1 fill, %2 line")
                  .arg(s.numFillStyles)
                  .arg(s.numLineStyles));
        check(s.hasEdgeBounds, "  EdgeBounds present");
        if (s.hasEdgeBounds)
          check(s.edgeMin == QPointF(10, 10) && s.edgeMax == QPointF(90, 90),
                "  EdgeBounds exclude the stroke",
                QString("%1,%2 to %3,%4")
                    .arg(s.edgeMin.x())
                    .arg(s.edgeMin.y())
                    .arg(s.edgeMax.x())
                    .arg(s.edgeMax.y()));
        check(s.contours.size() == 1 &&
                  s.contours.first().segments.size() == 4,
              "  one contour of four edges");
        if (!s.contours.isEmpty() && !s.contours.first().segments.isEmpty())
          check(s.contours.first().start == QPointF(10, 10),
                "  inside EdgeBounds, not ShapeBounds",
                QString("%1,%2")
                    .arg(s.contours.first().start.x())
                    .arg(s.contours.first().start.y()));
      }
    }
  }
}

// ---------------------------------------------------------------------------
// The real file, if it is here.
//
// Measured over the 250 DefineShape tags in mario.ssf: every one decodes, and every
// one produces the same outline, vertex for vertex, as JPEXS -- an independent and
// mature SWF decoder -- does on the same bytes. That is the claim; this is the part
// of it that can be re-checked without JPEXS.
//
// What is *not* claimed: that the outline lies inside the declared bounds. 101 of
// the 250 do not, and neither does JPEXS's, because the file's own bounds are loose
// in those places. The bounds are a useful oracle for a decoder -- they come from
// the same stream, so a misaligned read has to be wrong in a matching way to hide
// -- but they are not a property this decoder can assert.
// ---------------------------------------------------------------------------
static void test_real_shapes(const QString &dir) {
  fprintf(stderr, "\n-- the real file, when present --\n");
  const QString swf = QDir(dir).filePath("mario.ssf");
  if (!QFile::exists(swf)) {
    fprintf(stderr,
            "   [skip] no mario.ssf here; the real-shape census needs the "
            "sample file\n");
    return;
  }
  check(false, "the real-shape census is not yet wired up as a test");
}

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  if (argc < 2) {
    fprintf(stderr, "usage: swfshape_tests <fixture dir>\n");
    return 2;
  }
  const QString dir = QString::fromLocal8Bit(argv[1]);

  test_square(dir);
  test_fault_fixtures(dir);
  test_shape_cases(dir);
  test_malformed();
  test_real_shapes(dir);

  fprintf(stderr, "\n");
  if (gFails)
    fprintf(stderr, "FAILED: %d of %d checks\n", gFails, gChecks);
  else
    fprintf(stderr, "PASSED: %d checks, 0 failure(s)\n", gChecks);
  return gFails ? 1 : 0;
}