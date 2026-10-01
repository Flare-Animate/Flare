// Tests for the native format readers in tnzcore.
//
// These call the real entry points. The Moho reader is resolved from the built
// DLL at run time so this binary carries no link-time dependency on the module
// under test, which also means a rebuilt tnzcore.dll is picked up without
// relinking.
//
// usage: flash_reader_tests <fixtureDir>
#include <windows.h>

#include "As3Bridge.h"
#include "SWFAssets.h"
#include "ZipArchive.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QSet>
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
    test_supported_extensions();
    test_zip_extraction(dir);
    test_malformed_inputs(dir);
    test_as3_bridge_degrades();

    fprintf(stderr, "\n%s: %d checks, %d failure(s)\n",
            gFail ? "FAILED" : "PASSED", gChecks, gFail);
    return gFail ? 1 : 0;
}
