// Tests for the native format readers in tnzcore.
//
// These call the real entry points. The Moho reader is resolved from the built
// DLL at run time so this binary carries no link-time dependency on the module
// under test, which also means a rebuilt tnzcore.dll is picked up without
// relinking.
//
// usage: flash_reader_tests <fixtureDir>
// No <windows.h>: nothing here needs it, and including it before the Flare
// headers defines min/max as macros, which collides with the std::min and
// std::max in tcommon.h under /permissive-.

#include "As3Bridge.h"
#include "jpeg3_fixture.h"
#include "SWFAssets.h"
#include "ZipArchive.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QImage>
#include <QSet>
#include <QTemporaryDir>
#include <cstdio>
#include <string>

static int gFail = 0;
static int gChecks = 0;

static void check(bool ok, const char *what, const QString &detail = {}) {
    ++gChecks;
    fprintf(stderr, "   [%s] %s", ok ? "ok  " : "FAIL", what);
    if (!detail.isEmpty()) fprintf(stderr, "  (%s)", qPrintable(detail));
    fprintf(stderr, "\n");
    if (!ok) ++gFail;
}

static QString fx(const QString &dir, const QString &name) {
    return QDir(dir).filePath(name);
}

// For labels built at run time.
static void checkQ(bool ok, const QString &what, const QString &detail = {}) {
    check(ok, qPrintable(what), detail);
}

// ---------------------------------------------------------------------------
// Format sniffing: identified from leading bytes, not the extension.
// ---------------------------------------------------------------------------
static void test_format_detection(const QString &dir) {
    fprintf(stderr, "\n-- format detection --\n");

    check(FlashAssets::detectFormat(fx(dir, "sample.swf")) ==
              FlashAssets::Format::Swf,
          "an uncompressed SWF is detected");

    check(FlashAssets::detectFormat(fx(dir, "sample.fla")) ==
              FlashAssets::Format::Zip,
          "an FLA is detected as a ZIP container");

    check(FlashAssets::detectFormat(fx(dir, "sample.swc")) ==
              FlashAssets::Format::Zip,
          "an SWC is detected as a ZIP container");

    const FlashAssets::FlvInfo flv = FlashAssets::readFlvHeader(fx(dir, "sample.flv"));
    check(flv.valid, "an FLV header is parsed");

    check(FlashAssets::detectFormat(fx(dir, "sample.f4v")) ==
              FlashAssets::Format::IsoBmff,
          "an F4v is detected as ISO BMFF");

    // The whole point of sniffing: a SWF under a name that means nothing.
    const QString misnamed = QDir::temp().filePath("flare_misnamed.qqq");
    QFile::remove(misnamed);
    QFile::copy(fx(dir, "sample.swf"), misnamed);
    check(FlashAssets::detectFormat(misnamed) ==
              FlashAssets::Format::Swf,
          "a SWF named .qqq is still detected as a SWF");
    QFile::remove(misnamed);

    check(FlashAssets::detectFormat("no_such_file_at_all.swf") ==
              FlashAssets::Format::Unknown,
          "a missing file is Unknown");

    // Plain text is not a recognised container and must not be guessed at.
    const QString text = QDir::temp().filePath("flare_notabinary.txt");
    {
        QFile f(text);
        f.open(QIODevice::WriteOnly);
        f.write("this is not a swf, it is just some text\n");
    }
    check(FlashAssets::detectFormat(text) ==
              FlashAssets::Format::Unknown,
          "plain text is not mistaken for a container");
    QFile::remove(text);
}

// ---------------------------------------------------------------------------
// SWF header metadata
// ---------------------------------------------------------------------------
static void test_swf_header(const QString &dir) {
    fprintf(stderr, "\n-- SWF header --\n");
    const FlashAssets::SwfInfo info =
        FlashAssets::readSwfHeader(fx(dir, "sample.swf"));
    check(info.valid, "SWF header parsed");
    if (!info.valid) return;
    check(info.width == 550, "width read", QString::number(info.width));
    check(info.height == 400, "height read", QString::number(info.height));
    check(info.frameRate == 24, "frame rate read", QString::number(info.frameRate));
    check(info.version > 0, "version read", QString::number(info.version));
}

// A SWF tag record: the 16-bit header (code in the top 10 bits, length in the
// low 6) followed, when the length does not fit, by a 32-bit length. TagWalker
// reads the 32-bit form when the low 6 bits are 63.
//
// The length form matters as much as the code. These bodies are ~350 bytes, so
// they cannot use the short form: writing body.size() & 0x3F truncates to 27,
// the walker reads a 27-byte tag, the extractor's own bounds check bails, and
// the test fails for a reason that has nothing to do with the tag code.
static void appendTagRecord(QByteArray &swf, int code, const QByteArray &body) {
    Q_ASSERT(body.size() < 0x3FFFFFFF);
    if (body.size() < 0x3F) {
        const quint16 record = static_cast<quint16>(
            (static_cast<quint32>(code & 0x3FF) << 6) |
            static_cast<quint32>(body.size()));
        swf.append(reinterpret_cast<const char *>(&record), 2);
    } else {
        const quint16 record =
            static_cast<quint16>((static_cast<quint32>(code & 0x3FF) << 6) | 0x3F);
        swf.append(reinterpret_cast<const char *>(&record), 2);
        const quint32 len = static_cast<quint32>(body.size());
        swf.append(reinterpret_cast<const char *>(&len), 4);
    }
    swf.append(body);
}

// ---------------------------------------------------------------------------
// Tag-code dispatch. The census reports what a SWF holds so content that cannot
// be converted is *named* rather than silently dropped, which means the
// numbers are a user-facing claim about the file and a wrong code makes the
// claim false.
//
// The bug this pins: the bitmap family and the JPEG/lossless families had been
// transposed. 22/23 were dispatched as DefineBitsJPEG3/4 and 35/90 as the
// lossless variants. Per the SWF specification, and per this repository's own
// flare/sources/common/flash/Macromedia.h, 22 is DefineShape2, 23 is
// DefineButtonCxform, 35 is DefineBitsJPEG3, 36 is DefineBitsLossless2 and 90
// is DefineBitsJPEG4. So shapes and button colour transforms were being counted
// as images, and every real alpha JPEG fell through to the lossless branch.
//
// It survived three review rounds because no fixture contained a tag 22 and
// because the wrong branch papered over itself: a "format byte outside 3/4/5"
// was taken as a sign that a lossless tag was really a JPEG, which made the
// misparse produce plausible output rather than an error.
//
// So this builds a SWF from tags whose codes are known, one tag per code, and
// asserts where each lands. Synthetic rather than a binary fixture: the point is
// the mapping, and a generated tag stream states its own intent in the test.
// ---------------------------------------------------------------------------
static void test_census_tag_codes() {
    fprintf(stderr, "\n-- census tag codes --\n");

    // SWF_CWS: "CWS", version, fileLength, then an uncompressed body. The census
    // does not decompress, so the tag stream is stored as-is and the caller
    // hands it the same bytes it wrote.
    // swfBodyOffset() reads: signature(3) version(1) fileLength(4), then a RECT
    // whose bit-packed header byte has nbits = (byte >> 3) & 0x1F, occupying
    // (5 + 4*nbits + 7) / 8 bytes, then frameRate(2) and frameCount(2). The tag
    // stream starts there. nbits = 0 gives a one-byte RECT, the smallest legal.
    //
    // Each tag is a 16-bit record: code in the top 10 bits, length in the low 6.
    // A length of 63 means a 32-bit length follows. TagWalker returns false when
    // a tag occupies no bytes, so the bodies here carry a filler byte rather
    // than being empty -- the census reads only the code, and a non-empty body
    // is what keeps the walker advancing.
    const auto swfWithTags = [](const QList<quint16> &codes) {
        QByteArray body;
        for (const quint16 code : codes)
            appendTagRecord(body, code, QByteArray(1, static_cast<char>(0x00)));
        QByteArray out;
        out.append("FWS");
        out.append(static_cast<char>(0x06));              // version 6
        for (int i = 0; i < 4; ++i) out.append(static_cast<char>(0x00));  // length
        out.append(static_cast<char>(0x00));              // RECT: nbits = 0
        const quint16 rate = 0x0100, count = 1;
        out.append(reinterpret_cast<const char *>(&rate), 2);
        out.append(reinterpret_cast<const char *>(&count), 2);
        out.append(body);
        return out;
    };

    struct Expect {
        quint16 code;
        const char *name;
        int bitmaps;
        int shapes;
        int fonts;
        int video;
    };
    // One tag at a time, so a code that lands in two tallies cannot hide behind
    // another tag's contribution. Codes verified against the SWF specification's
    // tag table and against flare/sources/common/flash/Macromedia.h.
    const Expect cases[] = {
        {2, "DefineShape", 0, 1, 0, 0},
        {22, "DefineShape2", 0, 1, 0, 0},
        {32, "DefineShape3", 0, 1, 0, 0},
        {83, "DefineShape4", 0, 1, 0, 0},
        {46, "DefineMorphShape", 0, 1, 0, 0},
        {20, "DefineBitsLossless", 1, 0, 0, 0},
        {36, "DefineBitsLossless2", 1, 0, 0, 0},
        {21, "DefineBitsJPEG2", 1, 0, 0, 0},
        {35, "DefineBitsJPEG3", 1, 0, 0, 0},
        {90, "DefineBitsJPEG4", 1, 0, 0, 0},
        {6, "DefineBits", 1, 0, 0, 0},
        {24, "Protect", 0, 0, 0, 0},
        {48, "DefineFont2", 0, 0, 1, 0},
        {75, "DefineFont3", 0, 0, 1, 0},
        {10, "DefineFont", 0, 0, 1, 0},
        // Video is 60 and 62. 81 and 93 are DefineSceneAndFrameLabelData and
        // DefineScalingGrid -- the previous tally used those, so it was never
        // reachable, and a real video clip went uncounted.
        {60, "DefineVideoStream", 0, 0, 0, 1},
        {62, "DefineVideoStream2", 0, 0, 0, 1},
        // 23 is DefineButtonCxform: not a bitmap, not a shape, not a font. It
        // used to be dispatched as DefineBitsJPEG4, so this is the case that
        // fails on the old mapping.
        {23, "DefineButtonCxform", 0, 0, 0, 0},
        // 8 is JPEGTables: a header, not an image. Deliberately uncounted.
        {8, "JPEGTables", 0, 0, 0, 0},
    };

    int wrong = 0;
    for (const Expect &e : cases) {
        const QByteArray swf = swfWithTags({e.code});
        const FlashAssets::SwfContent c = FlashAssets::censusSwf(swf);
        const bool ok = (c.bitmaps == e.bitmaps && c.shapes == e.shapes &&
                         c.fonts == e.fonts && c.video == e.video);
        checkQ(ok, QString("tag %1 (%2) lands in bitmaps=%3 shapes=%4 fonts=%5 "
                           "video=%6")
                       .arg(e.code)
                       .arg(QString::fromLatin1(e.name))
                       .arg(c.bitmaps)
                       .arg(c.shapes)
                       .arg(c.fonts)
                       .arg(c.video));
        if (!ok) ++wrong;
    }
    check(wrong == 0, "every tag code lands in exactly the right tally",
          QString("%1 mis-dispatched").arg(wrong));

    // A tag the census does not know must not be counted at all, and must not
    // stop the walk: an unknown tag followed by a known one has to find it.
    {
        const QByteArray swf = swfWithTags({0x7fff, 2, 0x7ffe});
        const FlashAssets::SwfContent c = FlashAssets::censusSwf(swf);
        check(c.shapes == 1,
              "an unknown tag does not stop the walk", QString::number(c.shapes));
        check(c.bitmaps == 0, "and is not counted as an image");
    }

    // Several shapes in one movie: the tally has to accumulate, and a code that
    // reached two branches would show up as an inflated count.
    {
        const QByteArray swf = swfWithTags({2, 22, 32, 83, 46, 20, 36, 21, 35, 90});
        const FlashAssets::SwfContent c = FlashAssets::censusSwf(swf);
        check(c.shapes == 5, "five shape tags counted", QString::number(c.shapes));
        check(c.bitmaps == 5, "five bitmap tags counted", QString::number(c.bitmaps));
    }
}

// ---------------------------------------------------------------------------
// Bitmap *extraction*, as opposed to the census above. The census and the
// extractor dispatch on the tag codes separately, so passing the census says
// nothing about extraction: a mutation that moves the JPEG3/JPEG4 branch onto
// the lossless codes leaves every tally above correct.
//
// It also fails silently, which is why no test caught it. The JPEG branch probes
// the payload with QImage and `continue`s when it does not decode, and the
// lossless branch rejects a format byte outside 3/4/5 -- so a wrong code writes
// no file and reports no error. The user sees an import that silently lost an
// image, which is the failure mode this whole census exists to make impossible.
//
// So this builds a real DefineBitsJPEG3 -- a genuine JPEG, a zlib alpha channel
// and the 6-byte fixed header -- and asserts a decodable file comes out. The
// payload comes from tests/native/make_jpeg3_fixture.py, which encodes it with a
// real encoder and verifies it decodes before writing it, because hand-written
// JPEG bytes do not.
// ---------------------------------------------------------------------------
static void test_jpeg3_extraction_by_tag_code() {
    fprintf(stderr, "\n-- JPEG3 extraction --\n");

    // The fixture's own precondition. Asserted rather than assumed: an
    // undecodable fixture makes every check below pass vacuously, which is
    // exactly what happened when this payload was a hand-written byte array.
    {
        QImage probe;
        const bool ok =
            probe.loadFromData(QByteArray(reinterpret_cast<const char *>(
                                              jpeg3_fixture::jpeg),
                                          static_cast<int>(
                                              sizeof(jpeg3_fixture::jpeg))),
                               "JPG") &&
            !probe.isNull();
        check(ok, "the fixture JPEG decodes, so this test is not vacuous");
        check(probe.width() == 2 && probe.height() == 2,
              "and is 2x2, as the fixture assumes",
              QString("%1x%2").arg(probe.width()).arg(probe.height()));
        if (!ok) return;
    }

    // DefineBitsJPEG3 body: CharacterID(2), AlphaDataOffset(4), then ImageData
    // -- the JPEG, then the zlib alpha. AlphaDataOffset counts the bytes of
    // ImageData, so it is measured from the end of the 6-byte header and is
    // therefore the JPEG's length.
    const quint32 alphaOffset =
        static_cast<quint32>(sizeof(jpeg3_fixture::jpeg));
    QByteArray body;
    const quint16 charId = 1;
    body.append(reinterpret_cast<const char *>(&charId), 2);
    body.append(reinterpret_cast<const char *>(&alphaOffset), 4);
    body.append(reinterpret_cast<const char *>(jpeg3_fixture::jpeg),
                sizeof(jpeg3_fixture::jpeg));
    body.append(reinterpret_cast<const char *>(jpeg3_fixture::alpha),
                sizeof(jpeg3_fixture::alpha));

    // Wrap in a minimal SWF carrying just this one tag, then extract. The tag
    // code is the point: 35 is DefineBitsJPEG3, and before this test nothing
    // checked that a 35 reached the JPEG branch at all.
    QByteArray swf;
    swf.append("FWS");
    swf.append(static_cast<char>(0x06));
    for (int i = 0; i < 4; ++i) swf.append(static_cast<char>(0x00));  // length
    swf.append(static_cast<char>(0x00));                              // RECT nbits=0
    const quint16 rate = 0x0100, count = 1;
    swf.append(reinterpret_cast<const char *>(&rate), 2);
    swf.append(reinterpret_cast<const char *>(&count), 2);
    appendTagRecord(swf, 35, body);

    QTemporaryDir dir;
    check(dir.isValid(), "temp directory available");
    if (!dir.isValid()) return;

    const QStringList written = FlashAssets::extractSwfBitmaps(swf, dir.path());
    check(written.size() == 1, "a DefineBitsJPEG3 tag yields exactly one file",
          QString("%1 written").arg(written.size()));
    if (written.size() != 1) return;

    const QString p = QDir(dir.path()).filePath(written.first());
    check(QFile::exists(p), "the reported file exists on disk", p);
    // A usable alpha channel becomes a .png carrying it; otherwise the JPEG is
    // written on its own as a .jpg. Either is a success, but the file has to
    // decode -- a file that exists and will not open is worse than none,
    // because the user believes the image was imported.
    QImage got(p);
    check(!got.isNull(), "the extracted file decodes as an image", p);
    check(got.width() == 2 && got.height() == 2,
          "and has the dimensions the embedded JPEG declared",
          QString("%1x%2").arg(got.width()).arg(got.height()));

    // A JPEG4 tag carries a 2-byte DeblockParam, so its header is 8 bytes and
    // AlphaDataOffset is measured from there. Off-by-two here shifts the JPEG
    // and alpha against each other and the branch falls back to writing a
    // truncated file, so the second code gets its own case.
    QByteArray body4;
    const quint16 charId4 = 2;
    body4.append(reinterpret_cast<const char *>(&charId4), 2);
    body4.append(reinterpret_cast<const char *>(&alphaOffset), 4);
    const quint16 deblock = 0x0000;
    body4.append(reinterpret_cast<const char *>(&deblock), 2);
    body4.append(reinterpret_cast<const char *>(jpeg3_fixture::jpeg),
                 sizeof(jpeg3_fixture::jpeg));
    body4.append(reinterpret_cast<const char *>(jpeg3_fixture::alpha),
                 sizeof(jpeg3_fixture::alpha));

    QByteArray swf4;
    swf4.append("FWS");
    swf4.append(static_cast<char>(0x06));
    for (int i = 0; i < 4; ++i) swf4.append(static_cast<char>(0x00));
    swf4.append(static_cast<char>(0x00));
    swf4.append(reinterpret_cast<const char *>(&rate), 2);
    swf4.append(reinterpret_cast<const char *>(&count), 2);
    appendTagRecord(swf4, 90, body4);

    QTemporaryDir dir4;
    check(dir4.isValid(), "second temp directory available");
    if (!dir4.isValid()) return;
    const QStringList written4 = FlashAssets::extractSwfBitmaps(swf4, dir4.path());
    check(written4.size() == 1, "a DefineBitsJPEG4 tag yields exactly one file",
          QString("%1 written").arg(written4.size()));
    if (written4.size() == 1) {
        QImage got4(QDir(dir4.path()).filePath(written4.first()));
        check(!got4.isNull(), "the JPEG4 file decodes too");
        check(got4.width() == 2 && got4.height() == 2,
              "and keeps the right dimensions -- the 8-byte header is handled");
    }
}

// ---------------------------------------------------------------------------
// The advertised extension list must contain no duplicates and no blanks:
// the file dialog iterates it, so a duplicate is a duplicated entry in the
// dialog and a blank is an entry that matches everything.
// ---------------------------------------------------------------------------
static void test_supported_extensions() {
    fprintf(stderr, "\n-- advertised extensions --\n");
    const QStringList exts = FlashAssets::supportedExtensions();
    check(!exts.isEmpty(), "the list is not empty");
    check(exts.size() == QSet<QString>(exts.begin(), exts.end()).size(),
          "no duplicates", QString::number(exts.size()));
    // Every entry, not just the first. The file dialog iterates the whole list,
    // so one bad entry is one bad row in the dialog. The previous form ran the
    // loop once and broke out, and the loop below only called check() on
    // failure -- so for a well-formed list the test asserted nothing at all.
    int malformed = 0;
    for (const QString &e : exts) {
        if (e.trimmed().isEmpty() || e != e.toLower() || e.contains('*') ||
            e.contains('?') || e.contains('.'))
            ++malformed;
    }
    check(malformed == 0, "every extension is bare, lower-case and non-blank",
          QString("%1 of %2 malformed").arg(malformed).arg(exts.size()));
    // The formats Flare claims must all be advertised.
    for (const char *want : {"fla", "xfl", "swf", "swc", "flv", "f4v", "as"}) {
        checkQ(exts.contains(QString::fromLatin1(want)),
               QString("advertises .%1").arg(QString::fromLatin1(want)));
    }
}

// ---------------------------------------------------------------------------
// ZIP extraction, including the repair of a stale trailer and the refusal of
// member paths that escape the output directory.
// ---------------------------------------------------------------------------
static void test_zip_extraction(const QString &dir) {
    fprintf(stderr, "\n-- ZIP extraction --\n");

    const QString out = QDir::temp().filePath("flare_zip_test");
    QDir().mkpath(out);

    std::string detail;
    const bool ok = FlareZip::extract(
        TFilePath(fx(dir, "sample.fla").toStdString()),
        TFilePath(out.toStdString()), detail);
    check(ok, "a well-formed FLA extracts", QString::fromStdString(detail));
    check(QFile::exists(QDir(out).filePath("DOMDocument.xml")),
          "the extracted FLA contains DOMDocument.xml");

    // A stale trailer is a real failure mode in the wild (issue #70): the
    // archive is valid but the end-of-central-directory overstates the central
    // directory size, and minizip rejects it.
    const QString stale = fx(dir, "stale_trailer.fla");
    if (QFile::exists(stale)) {
        std::string d2;
        const QString out2 = QDir::temp().filePath("flare_zip_stale");
        QDir().mkpath(out2);
        const bool ok2 = FlareZip::extract(TFilePath(stale.toStdString()),
                                           TFilePath(out2.toStdString()), d2);
        check(ok2, "an archive with a stale trailer is repaired and extracted",
              QString::fromStdString(d2));
    } else {
        fprintf(stderr, "   (skipping the stale-trailer case: fixture absent)\n");
    }

    // A truncated archive must be refused outright. The previous version of this
    // check read `!ok3 || true`, which is unconditionally true, so the property
    // it claimed to test was never tested at all.
    const QString truncated = QDir::temp().filePath("flare_truncated.zip");
    {
        QFile f(truncated);
        f.open(QIODevice::WriteOnly);
        QByteArray zip(22, '\0');
        zip[0] = 'P'; zip[1] = 'K'; zip[2] = 0x05; zip[3] = 0x06;
        f.write(zip);
        f.close();
    }
    const QString out3 = QDir::temp().filePath("flare_truncated_out");
    QDir().mkpath(out3);
    std::string d3;
    check(!FlareZip::extract(TFilePath(truncated.toStdString()),
                             TFilePath(out3.toStdString()), d3),
          "a truncated archive is refused");
    check(!d3.empty(), "the refusal carries a reason",
          QString::fromStdString(d3));
    QFile::remove(truncated);
    QDir(out3).removeRecursively();

    // Zip-slip: a member whose path escapes the output directory must not be
    // written above it.
    //
    // The archive is a generated fixture rather than bytes built here, and that
    // matters: the hand-built version had a 49-byte central directory where the
    // spec says 46, so the extractor rejected it before it ever reached the
    // member name. The assertion passed because the archive was malformed, not
    // because the traversal guard worked. generate_trailer_fixtures.py now
    // builds it and asserts that a normal ZIP reader can open it, which is what
    // makes this test able to fail.
    const QString slip = fx(dir, "zipslip.zip");
    if (QFile::exists(slip)) {
        // Where a stray file would land if the guard failed.
        const QString above = QDir::temp().filePath("escape.txt");
        QFile::remove(above);
        const QString out4 = QDir::temp().filePath("flare_zipslip_out");
        QDir(out4).removeRecursively();
        QDir().mkpath(out4);

        std::string d4;
        FlareZip::extract(TFilePath(slip.toStdString()),
                          TFilePath(out4.toStdString()), d4);

        check(!QFile::exists(above),
              "a member named ../escape.txt is not written above the output dir");
        check(!QFile::exists(QDir(out4).filePath("escape.txt")),
              "and it is not silently written inside it either");
        // The archive is readable, so the extractor really did walk its members.
        // Without this the two checks above would also pass if the archive had
        // been rejected wholesale.
        check(QFile::exists(QDir(out4).filePath("benign.txt")),
              "the archive's ordinary members were extracted, so the traversal "
              "member was actually reached",
              QString::fromStdString(d4));
        QFile::remove(above);
        QDir(out4).removeRecursively();
    } else {
        fprintf(stderr, "   (skipping the Zip-slip case: fixture absent)\n");
    }
}

// ---------------------------------------------------------------------------
// Malformed input must be rejected with a reason, not crash.
// ---------------------------------------------------------------------------
static void test_malformed_inputs(const QString &dir) {
    fprintf(stderr, "\n-- malformed input --\n");

    // A file that claims to be a ZIP but is not.
    const QString fake = QDir::temp().filePath("flare_fake.zip");
    {
        QFile f(fake);
        f.open(QIODevice::WriteOnly);
        f.write("PK\x03\x04 and then nothing valid at all");
    }
    std::string detail;
    const bool ok = FlareZip::extract(TFilePath(fake.toStdString()),
                                      TFilePath(QDir::tempPath().toStdString()),
                                      detail);
    check(!ok, "a corrupt archive is rejected");
    check(!detail.empty(), "the rejection carries a reason",
          QString::fromStdString(detail));
    QFile::remove(fake);

    // An empty file.
    const QString empty = QDir::temp().filePath("flare_empty.swf");
    QFile::remove(empty);
    QFile f2(empty);
    f2.open(QIODevice::WriteOnly);
    f2.close();
    check(!FlashAssets::readSwfHeader(empty).valid,
          "an empty file is not a SWF");
    QFile::remove(empty);
}

// ---------------------------------------------------------------------------
// The AS3 bridge must degrade gracefully when the helper is absent: no throw,
// no empty result presented as success.
// ---------------------------------------------------------------------------
static void test_as3_bridge_degrades() {
    fprintf(stderr, "\n-- AS3 bridge --\n");
    const As3Bridge::Result r =
        As3Bridge::decompile(TFilePath("nope.swf"), TFilePath("out"));
    check(!r.ok, "decompile on a missing file reports failure");
    check(!r.error.isEmpty(), "the failure explains itself", r.error);
    if (As3Bridge::isAvailable()) {
        check(!As3Bridge::version().isEmpty(), "version reported when available");
        check(As3Bridge::unavailableReason().isEmpty(),
              "no reason recorded when the helper is available");
    } else {
        check(!As3Bridge::unavailableReason().isEmpty(),
              "unavailableReason explains itself",
              As3Bridge::unavailableReason());
    }
}

int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);
    if (argc < 2) {
        fprintf(stderr, "   usage: flash_reader_tests <fixtureDir>\n");
        return 2;
    }
    const QString dir = QString::fromLocal8Bit(argv[1]);

    test_format_detection(dir);
    test_swf_header(dir);
    test_census_tag_codes();
    test_jpeg3_extraction_by_tag_code();
    test_supported_extensions();
    test_zip_extraction(dir);
    test_malformed_inputs(dir);
    test_as3_bridge_degrades();

    fprintf(stderr, "\n%s: %d checks, %d failure(s)\n",
            gFail ? "FAILED" : "PASSED", gChecks, gFail);
    return gFail ? 1 : 0;
}
