// Verify the SWF shape *extraction*, not just the decoder.
//
// The decoder has its own tests; this checks the step after it, because that is
// where a movie's art actually reaches disk. A decoder can be perfect and the
// extractor still drop every shape: it has to find the tags, descend into sprites,
// name the files, and write an SVG a level loader will accept.
//
// usage: swfshape_extract_tests <fixture dir>

#include "SWFAssets.h"
#include "SWFShape.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QSet>
#include <QTemporaryDir>
#include <QXmlStreamReader>

#include <cstdio>

static int gChecks = 0;
static int gFails  = 0;

static void check(bool ok, const QString &what, const QString &detail = {}) {
  ++gChecks;
  const QString line = detail.isEmpty()
                           ? QString("  [%1] %2").arg(ok ? "ok  " : "FAIL", what)
                           : QString("  [%1] %2  (%3)")
                                 .arg(ok ? "ok  " : "FAIL", what, detail);
  fprintf(stderr, "%s\n", qPrintable(line));
  if (!ok) ++gFails;
}

static QString readAll(const QString &path) {
  QFile f(path);
  if (!f.open(QIODevice::ReadOnly)) return QString();
  return QString::fromUtf8(f.readAll());
}

// A whole SWF carrying the given tags, so the extractor is exercised over a real
// tag stream rather than handed bodies directly.
//
// The RECT is 15 bits wide and nine bytes long, and the tag stream starts at byte 21.
// Getting that wrong is invisible from here: both the census and the extractor then
// agree that the movie has no tags at all, which looks like agreement rather than
// like a header nobody parsed.
// A whole SWF carrying the given tags, so the extractor is exercised over a real
// tag stream rather than handed bodies directly.
//
//   bytes 0-2   "FWS"
//   byte  3     version
//   bytes 4-7   FileLength, UI32
//   bytes 8-16  RECT, 9 bytes: 15 bits of width then four 15-bit fields
//   bytes 17-18 FrameRate, UI16 little-endian
//   bytes 19-20 FrameCount, UI16 little-endian
//   byte  21    first tag
//
// The RECT is nine bytes and the stream starts at 21; getting that wrong is invisible
// from here, because the census and the extractor then agree that a movie with no
// readable tags has none. That is what happened the first time this was written, and
// the check below is what now catches it.
static QByteArray wrapSwf(const QByteArray &tags) {
  QByteArray s("FWS");
  s.append(char(14));                                // version
  s.append(QByteArray(4, '\0'));                     // FileLength
  s.append(QByteArray::fromHex("7800064000000e1000"));  // RECT, 9 bytes
  s.append(QByteArray::fromHex("001e"));             // FrameRate 30, UI16 LE
  s.append(QByteArray::fromHex("0200"));             // FrameCount 2, UI16 LE
  s.append(tags);
  s.append(QByteArray::fromHex("0000"));             // End tag
  s[4] = char((s.size() >> 16) & 0xFF);
  s[5] = char((s.size() >> 8) & 0xFF);
  s[6] = char(s.size() & 0xFF);
  return s;
}

// One tag, code and length. The length is a *single* byte in the short form, not a
// little-endian UI16: the whole record is one UI16 and the reader assembles it.
//
// Writing the length as two little-endian bytes -- which the first version of this
// did, because every other field here that is a number is little-endian -- puts the
// high length byte in the code's low bits, and a DefineShape becomes a DefineBits.
// Nothing then fails: the census reports no shapes and the extractor writes no files,
// which reads as agreement rather than as a malformed header.
static QByteArray tag(int code, const QByteArray &body) {
  QByteArray t;
  if (body.size() < 0x3F) {
    const int word = (code << 6) | body.size();
    t.append(char(word & 0xFF));
    t.append(char((word >> 8) & 0xFF));
  } else {
    const int word = (code << 6) | 0x3F;
    t.append(char(word & 0xFF));
    t.append(char((word >> 8) & 0xFF));
    const quint32 len = static_cast<quint32>(body.size());
    t.append(char(len & 0xFF));
    t.append(char((len >> 8) & 0xFF));
    t.append(char((len >> 16) & 0xFF));
    t.append(char((len >> 24) & 0xFF));
  }
  t.append(body);
  return t;
}

// The four corners of the square fixture, whose ShapeBounds are 0,0 to 400,400.
static bool mentionsAllCorners(const QString &svg) {
  return svg.contains("400 0") && svg.contains("400 400") &&
         svg.contains("0 400");
}

// A minimal SWF holding one DefineSprite whose body is a CharacterID and a tag
// stream. Written out longhand because the sprite's tag stream starts four bytes in,
// past its own ID -- an offset a helper that appends tags verbatim would get wrong,
// and again nothing would fail: the movie would simply have no shapes in it.
// A DefineSprite holding the given tags, with two FRAMETEST records in front of
// them. Each record is a UI16 frame count followed by that many UI16s, so the two
// together put the tags eight bytes past the sprite's CharacterID.
static QByteArray spriteTag(const QByteArray &inner) {
  QByteArray body;
  body.append(QByteArray(2, '\0'));         // the sprite's CharacterID, UI16
  body.append(QByteArray::fromHex("0200")); // 2 FRAMETEST records
  body.append(QByteArray::fromHex("0100")); //   record 0: 1 frame
  body.append(QByteArray::fromHex("0100")); //     frame 0
  body.append(QByteArray::fromHex("0200")); //   record 1: 2 frames
  body.append(QByteArray::fromHex("0100")); //     frame 1
  body.append(QByteArray::fromHex("0100")); //     frame 2
  body.append(inner);
  body.append(QByteArray::fromHex("0000")); // the sprite's End tag
  return tag(39, body);
}

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  if (argc < 2) {
    fprintf(stderr, "usage: swfshape_extract_tests <fixture dir>\n");
    return 2;
  }
  const QString dir = QString::fromLocal8Bit(argv[1]);

  fprintf(stderr, "\n-- the extractor over a real tag stream --\n");

  QByteArray square;
  {
    QFile f(QDir(dir).filePath("swf_square_v1_aa1.bin"));
    if (!f.open(QIODevice::ReadOnly)) {
      fprintf(stderr, "  cannot read the square fixture\n");
      return 1;
    }
    square = f.readAll();
  }

  QTemporaryDir tmp;
  const QString out = tmp.path();

  // One DefineShape in one movie.
  //
  // Confirm the movie is readable before blaming the extractor. If the header here
  // is wrong, the census and the extractor both report no tags, which looks like
  // agreement rather than like a header nobody parsed -- and that is exactly what
  // happened the first time this test was written.
  {
    const QByteArray bare = wrapSwf(tag(2, square));
    check(bare.size() > 21, QStringLiteral("the hand-built movie is long enough "
                                           "to hold a tag"),
          QString("%1 bytes").arg(bare.size()));
    const int counted = FlashAssets::censusSwf(bare).shapes;
    check(counted == 1,
          QStringLiteral("and the census can read a tag out of it"),
          QString("%1 shape(s) counted, %2 in the stream")
              .arg(counted).arg(1));
  }

  int skipped = -1;
  QStringList names =
      FlashAssets::extractSwfShapes(wrapSwf(tag(2, square)), out, &skipped);
  check(names.size() == 1, QStringLiteral("one shape in, one file out"),
        QString("%1 written").arg(names.size()));
  check(skipped == 0, QStringLiteral("nothing refused"), QString::number(skipped));

  if (!names.isEmpty()) {
    const QString svg = readAll(QDir(out).filePath(names.first()));
    check(!svg.isEmpty(), QStringLiteral("the file has content"),
          QString("%1 bytes").arg(svg.size()));
    check(svg.contains("<svg") && svg.contains("</svg>"),
          QStringLiteral("it is an SVG document"));
    check(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""),
          QStringLiteral("with the SVG namespace, which a loader needs"));
    check(svg.contains("viewBox="), QStringLiteral("and a viewBox"));
    check(svg.contains("width=") && svg.contains("height="),
          QStringLiteral("and explicit width and height, so a loader ignoring "
                         "the viewBox still gets the right size"));
    check(mentionsAllCorners(svg), QStringLiteral("carrying the square's four corners"));
    // 400 twips is 20 px at 1/20 px per twip.
    check(svg.contains("width=\"20.000\""),
          QStringLiteral("20 px wide, at 1/20 px per twip"));

    // It must parse as XML. A truncated or mis-escaped path is the failure mode
    // that a string comparison would not notice.
    QXmlStreamReader xml(svg);
    QString rootTag;
    while (!xml.atEnd()) {
      xml.readNext();
      if (xml.isStartElement() && rootTag.isEmpty())
        rootTag = xml.name().toString();
    }
    check(!xml.hasError() && rootTag == "svg",
          QStringLiteral("it parses as XML with <svg> at the root"),
          xml.hasError() ? xml.errorString() : rootTag);
  }

  // Several shapes, of different tag versions, must all come out.
  {
    const QDir d(dir);
    QByteArray bodies;
    int expect = 0;
    struct { const char *file; int code; } cases[] = {
        {"swf_square_v1_aa0.bin", 2},
        {"swf_square_v1_aa1.bin", 2},
        {"swf_square_v2_aa0.bin", 22},
        {"swf_bitmapfill_v1.bin", 2},
        {"swf_rotatedfill_v1.bin", 2},
        {"swf_gradient_v3.bin", 32},
        {"swf_twofillindices_v1.bin", 2},
        {"swf_ruffle_square_v1.bin", 2},
    };
    for (const auto &c : cases) {
      QFile f(d.filePath(QString::fromLatin1(c.file)));
      if (!f.open(QIODevice::ReadOnly)) continue;
      bodies.append(tag(c.code, f.readAll()));
      ++expect;
    }
    QStringList n = FlashAssets::extractSwfShapes(wrapSwf(bodies), out, nullptr);
    check(n.size() == expect,
          QString("%1 shapes of four tag versions, %2 written")
              .arg(expect).arg(n.size()));
    check(n.size() == n.toSet().size(), QStringLiteral("and every file name is distinct"));
  }

  // A shape inside a DefineSprite, which is where nested art actually lives.
  //
  // A DefineSprite's body is a CharacterID, then FRAMETEST records, then the tags.
  // Assuming the tags start straight after the ID -- which is true of DefineSprite2
  // and not of DefineSprite -- lands inside a frame test and finds nothing, so the
  // sprite's art is dropped without a word. Two frame tests here, so the tags start
  // eight bytes in, not two.
  {
    const QStringList n = FlashAssets::extractSwfShapes(
        wrapSwf(spriteTag(tag(2, square))), out, nullptr);
    check(n.size() == 1, QStringLiteral("a shape inside a DefineSprite is found"),
          QString("%1 written").arg(n.size()));
    if (!n.isEmpty())
      check(n.first().contains("sprite"),
            QStringLiteral("and named so the nesting is visible"), n.first());
  }

  // A tag the decoder refuses must be counted, not written as a partial outline.
  {
    QByteArray junk = QByteArray::fromHex("0100") + QByteArray(40, '\xff');
    int bad = 0;
    const QStringList n =
        FlashAssets::extractSwfShapes(wrapSwf(tag(2, junk)), out, &bad);
    check(n.isEmpty(), QStringLiteral("an undecodable shape writes no file"),
          QString("%1 written").arg(n.size()));
    check(bad == 1, QStringLiteral("and is counted as refused"), QString::number(bad));
  }

  // Nothing in, nothing out, and no directory required to exist first.
  {
    int bad = -1;
    const QStringList n =
        FlashAssets::extractSwfShapes(QByteArray("FWS"), out, &bad);
    check(n.isEmpty() && bad == 0, QStringLiteral("an empty movie yields nothing and no errors"));
    check(FlashAssets::extractSwfShapes(QByteArray(), out, nullptr).isEmpty(),
          "and so does an empty buffer");
  }

  // A tag with no body must not end the walk.
  //
  // A real 3.5 MB movie has an empty DefineShape4 in the middle of its tag stream, and
  // because an empty tag was reported as "nothing here" the walk stopped there: the
  // 2312 bytes after it held four more shapes, one of them a 60-byte DefineShape4 the
  // census counted, and the import reported 249 of 250 with nothing said about the
  // last. Both the census and the extractor were reading a truncated stream and
  // neither mentioned it.
  {
    const QStringList n = FlashAssets::extractSwfShapes(
        wrapSwf(tag(83, QByteArray()) + tag(2, square) + tag(2, square)), out,
        nullptr);
    check(n.size() == 2,
          QStringLiteral("an empty DefineShape4 does not end the tag walk"),
          QString("%1 written: the empty tag has no outline, the two after it "
                  "do").arg(n.size()));
  }

  // The stream really ending must still stop the walk, or the bytes after the End tag
  // would be read as tags.
  {
    QByteArray past = wrapSwf(tag(2, square));
    past.append(QByteArray::fromHex("a2000100"));   // a shape after the End tag
    const QStringList n = FlashAssets::extractSwfShapes(past, out, nullptr);
    check(n.size() == 1, QStringLiteral("the End tag does stop the walk"),
          QString("%1 written").arg(n.size()));
  }

  // A tag claiming more bytes than the file holds must be clamped, not trusted.
  {
    QByteArray lying;
    lying.append(char((2 << 6) & 0xFF));
    lying.append(char(((2 << 6) >> 8) & 0xFF));
    lying.append(QByteArray::fromHex("40420f00"));  // a 1,000,000-byte body
    lying.append(QByteArray::fromHex("0100"));
    const QStringList n = FlashAssets::extractSwfShapes(wrapSwf(lying), out,
                                                        nullptr);
    check(n.isEmpty(),
          QStringLiteral("a tag longer than the file is clamped and refused"),
          QString("%1 written").arg(n.size()));
  }

  // The census and the extractor must agree about how many shapes there are, or the
  // summary line and the files on disk tell different stories.
  {
    QByteArray bodies = tag(2, square) + tag(2, square) + tag(2, square);
    const QByteArray swf = wrapSwf(bodies);
    const int counted = FlashAssets::censusSwf(swf).shapes;
    const int written =
        FlashAssets::extractSwfShapes(swf, out, nullptr).size();
    check(counted == 3 && written == 3,
          QStringLiteral("the census and the extractor agree on the shape count"),
          QString("%1 counted, %2 written").arg(counted).arg(written));
  }

  fprintf(stderr, "\n");
  if (gFails)
    fprintf(stderr, "FAILED: %d of %d checks\n", gFails, gChecks);
  else
    fprintf(stderr, "PASSED: %d checks, 0 failure(s)\n", gChecks);
  return gFails ? 1 : 0;
}
