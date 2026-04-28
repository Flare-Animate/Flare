// test_flashimport.cpp — unit tests for Flash header parsing logic
// Compile standalone (no modification to flashimport.cpp needed):
//   g++ -std=c++17 test_flashimport.cpp -o test_flash \
//       $(pkg-config --cflags --libs Qt5Core) && ./test_flash

#include <QByteArray>
#include <QDir>
#include <QFile>
#include <QRegularExpression>
#include <QString>
#include <QStringList>
#include <QXmlStreamReader>
#include <cstdio>
#include <cstring>

// ---------------------------------------------------------------------------
// Re-implemented structs and parsers (copied from anonymous namespace in
// flashimport.cpp — tests cannot reach the originals directly)
// ---------------------------------------------------------------------------

struct SwfInfo {
    bool valid      = false;
    bool compressed = false;
    int  version    = 0;
    int  width      = 0;
    int  height     = 0;
    int  frameRate  = 0;
    int  frameCount = 0;
};

static SwfInfo readSwfHeader(const QString &path) {
    SwfInfo info;
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return info;
    QByteArray data = f.read(9);
    if (data.size() < 4) return info;
    unsigned char sig0 = data[0], sig1 = data[1], sig2 = data[2];
    if ((sig0 != 'F' && sig0 != 'C' && sig0 != 'Z') ||
         sig1 != 'W' || sig2 != 'S')
        return info;
    info.valid      = true;
    info.compressed = (sig0 == 'C' || sig0 == 'Z');
    info.version    = static_cast<unsigned char>(data[3]);
    if (!info.compressed && data.size() >= 9) {
        QByteArray rest = f.read(256);
        data += rest;
        int offset = 8;
        if (offset < data.size()) {
            unsigned char first = static_cast<unsigned char>(data[offset]);
            int nbits = first >> 3;
            int totalBits = 5 + 4 * nbits;
            int bytesNeeded = (totalBits + 7) / 8;
            if (offset + bytesNeeded + 4 <= data.size()) {
                int bitPos = offset * 8 + 5;
                auto readBits = [&](int n) -> int {
                    int val = 0;
                    for (int b = 0; b < n; b++) {
                        int byteIdx = bitPos / 8;
                        int bitIdx  = 7 - (bitPos % 8);
                        if (byteIdx < data.size())
                            val = (val << 1) | ((static_cast<unsigned char>(data[byteIdx]) >> bitIdx) & 1);
                        else
                            val <<= 1;
                        bitPos++;
                    }
                    return val;
                };
                auto readSBits = [&](int n) -> int {
                    int val = readBits(n);
                    if (val & (1 << (n - 1))) val -= (1 << n);
                    return val;
                };
                int xmin = readSBits(nbits);
                int xmax = readSBits(nbits);
                int ymin = readSBits(nbits);
                int ymax = readSBits(nbits);
                info.width  = (xmax - xmin) / 20;
                info.height = (ymax - ymin) / 20;
                int afterRect = (bitPos + 7) / 8;
                if (afterRect + 4 <= data.size()) {
                    info.frameRate =
                        static_cast<unsigned char>(data[afterRect + 1]);
                    info.frameCount =
                        static_cast<unsigned char>(data[afterRect + 2]) |
                        (static_cast<unsigned char>(data[afterRect + 3]) << 8);
                }
            }
        }
    }
    return info;
}

struct FlvInfo {
    bool valid    = false;
    int  version  = 0;
    bool hasVideo = false;
    bool hasAudio = false;
};

static FlvInfo readFlvHeader(const QString &path) {
    FlvInfo info;
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return info;
    QByteArray hdr = f.read(9);
    if (hdr.size() < 9) return info;
    if ((unsigned char)hdr[0] != 'F' ||
        (unsigned char)hdr[1] != 'L' ||
        (unsigned char)hdr[2] != 'V')
        return info;
    info.valid    = true;
    info.version  = (unsigned char)hdr[3];
    unsigned char flags = (unsigned char)hdr[4];
    info.hasVideo = (flags & 0x01) != 0;
    info.hasAudio = (flags & 0x04) != 0;
    return info;
}

struct F4vInfo {
    bool    valid = false;
    QString majorBrand;
    QString compatBrands;
};

static F4vInfo readF4vHeader(const QString &path) {
    F4vInfo info;
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return info;
    QByteArray hdr = f.read(32);
    if (hdr.size() < 12) return info;
    if (hdr[4] != 'f' || hdr[5] != 't' || hdr[6] != 'y' || hdr[7] != 'p')
        return info;
    info.valid      = true;
    info.majorBrand = QString::fromLatin1(hdr.mid(8, 4)).trimmed();
    QStringList brands;
    for (int off = 16; off + 4 <= hdr.size(); off += 4) {
        QString b = QString::fromLatin1(hdr.mid(off, 4)).trimmed();
        if (!b.isEmpty()) brands << b;
    }
    info.compatBrands = brands.join(", ");
    return info;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

static bool writeTmp(const QString &path, const QByteArray &data) {
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly)) return false;
    f.write(data);
    return true;
}

static int g_pass = 0, g_fail = 0;

#define ASSERT_TRUE(expr) do { \
    if (!(expr)) { \
        printf("  FAIL: %s  (line %d)\n", #expr, __LINE__); \
        ++g_fail; return; \
    } \
} while(0)

#define ASSERT_EQ(a, b) do { \
    if ((a) != (b)) { \
        printf("  FAIL: %s == %s  (%d != %d, line %d)\n", #a, #b, (int)(a), (int)(b), __LINE__); \
        ++g_fail; return; \
    } \
} while(0)

#define ASSERT_STR_EQ(a, b) do { \
    if ((a) != QString(b)) { \
        printf("  FAIL: %s == \"%s\"  (got \"%s\", line %d)\n", #a, b, (a).toUtf8().constData(), __LINE__); \
        ++g_fail; return; \
    } \
} while(0)

static void beginTest(const char *name) { printf("[ RUN ] %s\n", name); }
static void endTest(const char *name) {
    // if no extra FAILs were printed, count as pass
    (void)name;
    ++g_pass;
    printf("[ OK  ]\n");
}

// ---------------------------------------------------------------------------
// Build a minimal uncompressed SWF byte sequence for 550x400 @ rate=24 fps
//
// SWF header (8 bytes): sig[3] + version[1] + fileLength[4 LE]
// RECT (bit-packed):
//   nbits for 11000 twips (550*20) = need 14 bits, so nbits=14
//   RECT: Nbits(5) Xmin(14) Xmax(14) Ymin(14) Ymax(14) = 61 bits → 8 bytes
// Frame rate: 2 bytes LE (8.8 fixed) → {0, 24}
// Frame count: 2 bytes LE → {1, 0}
// ---------------------------------------------------------------------------
static QByteArray buildUncompressedSwf(int version = 5,
                                       int widthPx = 550,
                                       int heightPx = 400,
                                       int fps = 24,
                                       int frameCount = 1) {
    int xmax = widthPx  * 20;   // twips
    int ymax = heightPx * 20;   // twips

    // determine nbits needed for signed representation of xmax
    int nbits = 1;
    while ((1 << (nbits - 1)) <= xmax) ++nbits;  // signed, so need one extra bit

    // Encode RECT into a bit buffer
    // fields: Xmin=0, Xmax=xmax, Ymin=0, Ymax=ymax
    int totalBits = 5 + 4 * nbits;
    int rectBytes = (totalBits + 7) / 8;
    QByteArray rect(rectBytes, '\0');

    int bitPos = 0;
    auto writeBits = [&](int val, int n) {
        for (int b = n - 1; b >= 0; b--) {
            int byteIdx = bitPos / 8;
            int bitIdx  = 7 - (bitPos % 8);
            if ((val >> b) & 1)
                rect[byteIdx] = rect[byteIdx] | (1 << bitIdx);
            bitPos++;
        }
    };
    writeBits(nbits, 5);
    writeBits(0,    nbits);  // Xmin
    writeBits(xmax, nbits);  // Xmax
    writeBits(0,    nbits);  // Ymin
    writeBits(ymax, nbits);  // Ymax

    // frame rate (8.8 LE) and frame count (uint16 LE)
    QByteArray tail(4, '\0');
    tail[0] = 0;           // fractional fps
    tail[1] = (char)fps;   // integer fps
    tail[2] = (char)(frameCount & 0xFF);
    tail[3] = (char)((frameCount >> 8) & 0xFF);

    int fileLen = 8 + rect.size() + tail.size();
    QByteArray hdr(8, '\0');
    hdr[0] = 'F'; hdr[1] = 'W'; hdr[2] = 'S';
    hdr[3] = (char)version;
    hdr[4] = (char)(fileLen & 0xFF);
    hdr[5] = (char)((fileLen >> 8) & 0xFF);
    hdr[6] = (char)((fileLen >> 16) & 0xFF);
    hdr[7] = (char)((fileLen >> 24) & 0xFF);

    return hdr + rect + tail;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

void testSwfHeaderUncompressed() {
    beginTest("testSwfHeaderUncompressed");
    QString path = QDir::tempPath() + "/test_flash_fws.swf";
    QByteArray data = buildUncompressedSwf(5, 550, 400, 24, 1);
    ASSERT_TRUE(writeTmp(path, data));
    SwfInfo info = readSwfHeader(path);
    QFile::remove(path);
    ASSERT_TRUE(info.valid);
    ASSERT_TRUE(!info.compressed);
    ASSERT_EQ(info.version, 5);
    ASSERT_EQ(info.width,   550);
    ASSERT_EQ(info.height,  400);
    ASSERT_EQ(info.frameRate, 24);
    ASSERT_EQ(info.frameCount, 1);
    endTest("testSwfHeaderUncompressed");
}

void testSwfHeaderCompressed() {
    beginTest("testSwfHeaderCompressed");
    QString path = QDir::tempPath() + "/test_flash_cws.swf";
    // CWS header: just need the first 8 bytes to detect compression
    QByteArray data(9, '\0');
    data[0] = 'C'; data[1] = 'W'; data[2] = 'S';
    data[3] = 6;  // version 6+
    // file length (4 bytes LE) — arbitrary
    data[4] = 20; data[5] = 0; data[6] = 0; data[7] = 0;
    data[8] = 0;
    ASSERT_TRUE(writeTmp(path, data));
    SwfInfo info = readSwfHeader(path);
    QFile::remove(path);
    ASSERT_TRUE(info.valid);
    ASSERT_TRUE(info.compressed);
    ASSERT_EQ(info.version, 6);
    endTest("testSwfHeaderCompressed");
}

void testSwfHeaderInvalid() {
    beginTest("testSwfHeaderInvalid");
    QString path = QDir::tempPath() + "/test_flash_bad.swf";
    QByteArray data("XYZGARBAGE1234567890", 20);
    ASSERT_TRUE(writeTmp(path, data));
    SwfInfo info = readSwfHeader(path);
    QFile::remove(path);
    ASSERT_TRUE(!info.valid);
    endTest("testSwfHeaderInvalid");
}

void testFlvHeaderValid() {
    beginTest("testFlvHeaderValid");
    QString path = QDir::tempPath() + "/test_flash_valid.flv";
    // FLV\x01\x05\x00\x00\x00\x09  — version=1, flags=0x05 (video+audio)
    QByteArray data("\x46\x4C\x56\x01\x05\x00\x00\x00\x09", 9);
    ASSERT_TRUE(writeTmp(path, data));
    FlvInfo info = readFlvHeader(path);
    QFile::remove(path);
    ASSERT_TRUE(info.valid);
    ASSERT_EQ(info.version, 1);
    ASSERT_TRUE(info.hasVideo);
    ASSERT_TRUE(info.hasAudio);
    endTest("testFlvHeaderValid");
}

void testFlvHeaderInvalid() {
    beginTest("testFlvHeaderInvalid");
    QString path = QDir::tempPath() + "/test_flash_bad.flv";
    QByteArray data("\x58\x4C\x56\x01\x05\x00\x00\x00\x09", 9);  // 'X' not 'F'
    ASSERT_TRUE(writeTmp(path, data));
    FlvInfo info = readFlvHeader(path);
    QFile::remove(path);
    ASSERT_TRUE(!info.valid);
    endTest("testFlvHeaderInvalid");
}

void testF4vHeaderValid() {
    beginTest("testF4vHeaderValid");
    QString path = QDir::tempPath() + "/test_flash_valid.f4v";
    // ftyp box: size(4) + "ftyp"(4) + majorBrand "f4v "(4) + minorVer(4) + compat(4)
    QByteArray data(20, '\0');
    // box size = 20
    data[0]=0; data[1]=0; data[2]=0; data[3]=20;
    data[4]='f'; data[5]='t'; data[6]='y'; data[7]='p';
    data[8]='f'; data[9]='4'; data[10]='v'; data[11]=' ';
    // minor version = 0
    data[12]=0; data[13]=0; data[14]=0; data[15]=0;
    // compat brand "isom"
    data[16]='i'; data[17]='s'; data[18]='o'; data[19]='m';
    ASSERT_TRUE(writeTmp(path, data));
    F4vInfo info = readF4vHeader(path);
    QFile::remove(path);
    ASSERT_TRUE(info.valid);
    ASSERT_STR_EQ(info.majorBrand, "f4v");
    endTest("testF4vHeaderValid");
}

void testF4vHeaderInvalid() {
    beginTest("testF4vHeaderInvalid");
    QString path = QDir::tempPath() + "/test_flash_bad.f4v";
    QByteArray data(20, '\0');
    data[0]=0; data[1]=0; data[2]=0; data[3]=20;
    // box type "XMP " instead of "ftyp"
    data[4]='X'; data[5]='M'; data[6]='P'; data[7]=' ';
    data[8]='f'; data[9]='4'; data[10]='v'; data[11]=' ';
    ASSERT_TRUE(writeTmp(path, data));
    F4vInfo info = readF4vHeader(path);
    QFile::remove(path);
    ASSERT_TRUE(!info.valid);
    endTest("testF4vHeaderInvalid");
}

// ---------------------------------------------------------------------------
// XFL / DOMDocument attribute parsing
//
// The XFLDocument attribute parser used in XFLReader.cpp extracts width,
// height, frameRate, and backgroundColor from a DOMDocument.xml root element.
// These tests exercise that logic independently using QXmlStreamReader on a
// minimal in-memory XML string (matching exactly what XFLReader::parseDOMDocument
// does), so they compile and run with Qt only — no tnzcore linkage needed.
// ---------------------------------------------------------------------------

static bool parseXFLDocumentAttributes(const QByteArray &xmlData,
                                        int &outWidth, int &outHeight,
                                        double &outFrameRate,
                                        QString &outBgColor) {
    // Mirror of XFLReader::parseDOMDocument attribute extraction.
    QXmlStreamReader xml(xmlData);
    while (!xml.atEnd()) {
        xml.readNext();
        if (xml.isStartElement()) {
            // Strip namespace prefix (Adobe FLA files use xmlns="http://ns.adobe.com/xfl/2008/")
            QString localName = xml.name().toString();
            if (localName == "DOMDocument") {
                const QXmlStreamAttributes a = xml.attributes();
                if (a.hasAttribute("width"))     outWidth     = a.value("width").toInt();
                if (a.hasAttribute("height"))    outHeight    = a.value("height").toInt();
                if (a.hasAttribute("frameRate")) outFrameRate = a.value("frameRate").toDouble();
                if (a.hasAttribute("backgroundColor")) outBgColor = a.value("backgroundColor").toString();
                return !xml.hasError();
            }
        }
    }
    return false;
}

static QByteArray buildMinimalDOMDocument(int w, int h, double fps, const char *bg) {
    // A minimal DOMDocument.xml matching the format written by XFLWriter / Adobe Animate.
    QString xml = QString(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n"
        "<DOMDocument xmlns=\"http://ns.adobe.com/xfl/2008/\""
        " width=\"%1\" height=\"%2\" frameRate=\"%3\" backgroundColor=\"%4\">"
        "<symbols/><timelines/></DOMDocument>\n"
    ).arg(w).arg(h).arg(fps, 0, 'f', 1).arg(bg);
    return xml.toUtf8();
}

void testXFLDocumentBasicAttributes() {
    beginTest("testXFLDocumentBasicAttributes");
    QByteArray xml = buildMinimalDOMDocument(1280, 720, 30.0, "#FF8800");
    int w = 0, h = 0; double fps = 0; QString bg;
    ASSERT_TRUE(parseXFLDocumentAttributes(xml, w, h, fps, bg));
    ASSERT_EQ(w, 1280);
    ASSERT_EQ(h, 720);
    ASSERT_EQ((int)(fps + 0.5), 30);
    ASSERT_STR_EQ(bg, "#FF8800");
    endTest("testXFLDocumentBasicAttributes");
}

void testXFLDocumentDefaultDimensions() {
    beginTest("testXFLDocumentDefaultDimensions");
    // Standard Adobe Animate default: 550×400, 24fps, white background
    QByteArray xml = buildMinimalDOMDocument(550, 400, 24.0, "#FFFFFF");
    int w = 0, h = 0; double fps = 0; QString bg;
    ASSERT_TRUE(parseXFLDocumentAttributes(xml, w, h, fps, bg));
    ASSERT_EQ(w, 550);
    ASSERT_EQ(h, 400);
    ASSERT_EQ((int)(fps + 0.5), 24);
    ASSERT_STR_EQ(bg, "#FFFFFF");
    endTest("testXFLDocumentDefaultDimensions");
}

void testXFLDocumentHighResolution() {
    beginTest("testXFLDocumentHighResolution");
    // 4K UHD canvas
    QByteArray xml = buildMinimalDOMDocument(3840, 2160, 60.0, "#000000");
    int w = 0, h = 0; double fps = 0; QString bg;
    ASSERT_TRUE(parseXFLDocumentAttributes(xml, w, h, fps, bg));
    ASSERT_EQ(w, 3840);
    ASSERT_EQ(h, 2160);
    ASSERT_EQ((int)(fps + 0.5), 60);
    endTest("testXFLDocumentHighResolution");
}

void testXFLDocumentNoNamespace() {
    beginTest("testXFLDocumentNoNamespace");
    // Some older or third-party tools omit the XFL namespace.
    QByteArray xml =
        "<?xml version=\"1.0\"?>\n"
        "<DOMDocument width=\"800\" height=\"600\" frameRate=\"12.0\" backgroundColor=\"#336699\">"
        "</DOMDocument>\n";
    int w = 0, h = 0; double fps = 0; QString bg;
    ASSERT_TRUE(parseXFLDocumentAttributes(xml, w, h, fps, bg));
    ASSERT_EQ(w, 800);
    ASSERT_EQ(h, 600);
    ASSERT_EQ((int)(fps + 0.5), 12);
    endTest("testXFLDocumentNoNamespace");
}

void testXFLDocumentMalformedXML() {
    beginTest("testXFLDocumentMalformedXML");
    // Malformed XML: parser should not crash; parseXFLDocumentAttributes may
    // return false (no valid DOMDocument root found) but must not throw.
    QByteArray xml = "<DOMDocument width='400' height='300'><unclosed>\n";
    int w = 0, h = 0; double fps = 0; QString bg;
    // Tolerate either true (partial parse) or false (no complete element found)
    (void)parseXFLDocumentAttributes(xml, w, h, fps, bg);
    // The important thing is we reach here without a crash
    endTest("testXFLDocumentMalformedXML");
}

void testXFLFrameRateFractional() {
    beginTest("testXFLFrameRateFractional");
    // 23.976 fps (film framerate)
    QByteArray xml = buildMinimalDOMDocument(1920, 1080, 23.976, "#FFFFFF");
    int w = 0, h = 0; double fps = 0; QString bg;
    ASSERT_TRUE(parseXFLDocumentAttributes(xml, w, h, fps, bg));
    ASSERT_EQ(w, 1920);
    ASSERT_EQ(h, 1080);
    // Allow small floating-point tolerance
    ASSERT_TRUE(fps > 23.9 && fps < 24.1);
    endTest("testXFLFrameRateFractional");
}

// ---------------------------------------------------------------------------
// FLA binary media extraction helper tests
//
// The JPEG / PNG / GIF magic-byte detection used by extractFLABinaryMedia() is
// re-implemented here to verify the magic-byte constants are correct.
// ---------------------------------------------------------------------------

struct BinaryMediaResult { bool valid; QString ext; };

static BinaryMediaResult detectBinaryMediaType(const QByteArray &hdr) {
    BinaryMediaResult r { false, {} };
    if (hdr.size() < 4) return r;
    const auto *h = reinterpret_cast<const unsigned char *>(hdr.constData());
    if (h[0]==0xFF && h[1]==0xD8 && h[2]==0xFF)              { r.valid=true; r.ext="jpg"; }
    else if (h[0]==0x89 && h[1]==0x50 && h[2]==0x4E && h[3]==0x47) { r.valid=true; r.ext="png"; }
    else if (h[0]==0x47 && h[1]==0x49 && h[2]==0x46 && h[3]==0x38) { r.valid=true; r.ext="gif"; }
    return r;
}

void testBinaryMediaJPEG() {
    beginTest("testBinaryMediaJPEG");
    QByteArray hdr = QByteArray("\xFF\xD8\xFF\xE0\x00\x10JFIF", 10);
    auto r = detectBinaryMediaType(hdr);
    ASSERT_TRUE(r.valid);
    ASSERT_STR_EQ(r.ext, "jpg");
    endTest("testBinaryMediaJPEG");
}

void testBinaryMediaPNG() {
    beginTest("testBinaryMediaPNG");
    QByteArray hdr = QByteArray("\x89PNG\r\n\x1A\n\x00\x00", 10);
    auto r = detectBinaryMediaType(hdr);
    ASSERT_TRUE(r.valid);
    ASSERT_STR_EQ(r.ext, "png");
    endTest("testBinaryMediaPNG");
}

void testBinaryMediaGIF() {
    beginTest("testBinaryMediaGIF");
    QByteArray hdr = QByteArray("GIF89a\x01\x00\x01\x00", 10);
    auto r = detectBinaryMediaType(hdr);
    ASSERT_TRUE(r.valid);
    ASSERT_STR_EQ(r.ext, "gif");
    endTest("testBinaryMediaGIF");
}

void testBinaryMediaUnknown() {
    beginTest("testBinaryMediaUnknown");
    QByteArray hdr = QByteArray("RIFF\x00\x00\x00\x00WAVE", 12);
    auto r = detectBinaryMediaType(hdr);
    ASSERT_TRUE(!r.valid);
    endTest("testBinaryMediaUnknown");
}

void testBinaryMediaTooShort() {
    beginTest("testBinaryMediaTooShort");
    QByteArray hdr = QByteArray("\xFF\xD8", 2);   // JPEG but only 2 bytes
    auto r = detectBinaryMediaType(hdr);
    ASSERT_TRUE(!r.valid);
    endTest("testBinaryMediaTooShort");
}

// ---------------------------------------------------------------------------
// Zip Slip protection logic tests
//
// The isPathUnderDir() / path-traversal guard logic is re-implemented here
// so these tests run without the full flashimport.cpp dependency.
// ---------------------------------------------------------------------------

static bool isPathSafeForExtraction(const QString &outDir, const QString &entryName) {
    // Mirror the logic in extractZip() and _safe_extract_zip (Python equivalent).
    // Returns false if the entry would escape outDir.
    QString entry = entryName;
    entry.replace('\\', '/');
    while (entry.startsWith("./")) entry = entry.mid(2);
    while (entry.startsWith('/'))  entry = entry.mid(1);
    if (entry.isEmpty()) return false;
    if (entry.startsWith('/')) return false;
    // Reject any remaining backslash after normalisation
    if (entry.contains('\\')) return false;
    // Reject '..' path traversal segments (including trailing '..')
    for (const QString &seg : entry.split('/')) {
        if (seg == "..") return false;
    }
    if (entry.size() >= 2 && entry[1] == ':') return false;  // Windows drive path
    return true;
}

void testZipSlipTraversalRejected() {
    beginTest("testZipSlipTraversalRejected");
    ASSERT_TRUE(!isPathSafeForExtraction("/tmp/out", "../../etc/passwd"));
    ASSERT_TRUE(!isPathSafeForExtraction("/tmp/out", "../escape.txt"));
    ASSERT_TRUE(!isPathSafeForExtraction("/tmp/out", "/absolute/path.txt"));
    ASSERT_TRUE(!isPathSafeForExtraction("/tmp/out", "C:/Windows/System32/evil.dll"));
    endTest("testZipSlipTraversalRejected");
}

void testZipSlipSafePathsAccepted() {
    beginTest("testZipSlipSafePathsAccepted");
    ASSERT_TRUE(isPathSafeForExtraction("/tmp/out", "DOMDocument.xml"));
    ASSERT_TRUE(isPathSafeForExtraction("/tmp/out", "LIBRARY/Symbol1.xml"));
    ASSERT_TRUE(isPathSafeForExtraction("/tmp/out", "bin/bitmap0001.dat"));
    ASSERT_TRUE(isPathSafeForExtraction("/tmp/out", "./nested/path/file.png"));  // ./ prefix stripped
    endTest("testZipSlipSafePathsAccepted");
}

// ---------------------------------------------------------------------------
// JSFL / ActionScript content detection tests (issue #52, #11)
//
// These test the regex-level function-name extraction logic used by
// import_container.py's extract_jsfl_functions() — re-implemented here in
// C++ for the C++ test suite completeness.
// ---------------------------------------------------------------------------

static QStringList extractJSFLFunctions(const QString &src) {
    QStringList names;
    QRegularExpression re(R"(\bfunction\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\()");
    QRegularExpressionMatchIterator it = re.globalMatch(src);
    while (it.hasNext()) names << it.next().captured(1);
    return names;
}

void testJSFLFunctionExtraction() {
    beginTest("testJSFLFunctionExtraction");
    QString src =
        "// JSFL toolbar script\n"
        "function incrementInstances(startFrame) {\n"
        "  fl.trace('hi');\n"
        "}\n"
        "function addLayer() { }\n"
        "var anon = function() {};\n"  // anonymous – should not be extracted
        "function resetTimeline(doc) {}\n";
    QStringList fns = extractJSFLFunctions(src);
    ASSERT_TRUE(fns.contains("incrementInstances"));
    ASSERT_TRUE(fns.contains("addLayer"));
    ASSERT_TRUE(fns.contains("resetTimeline"));
    ASSERT_EQ(fns.size(), 3);
    endTest("testJSFLFunctionExtraction");
}

void testJSFLEmptySource() {
    beginTest("testJSFLEmptySource");
    ASSERT_EQ(extractJSFLFunctions("").size(), 0);
    ASSERT_EQ(extractJSFLFunctions("// no functions here\nvar x = 1;").size(), 0);
    endTest("testJSFLEmptySource");
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

int main(int argc, char **argv) {
    (void)argc; (void)argv;
    // QCoreApplication not strictly required for QFile/QDir/QString/QRegExp
    printf("=== Flash header parsing unit tests ===\n");
    testSwfHeaderUncompressed();
    testSwfHeaderCompressed();
    testSwfHeaderInvalid();
    testFlvHeaderValid();
    testFlvHeaderInvalid();
    testF4vHeaderValid();
    testF4vHeaderInvalid();
    printf("\n=== XFL / DOMDocument attribute tests ===\n");
    testXFLDocumentBasicAttributes();
    testXFLDocumentDefaultDimensions();
    testXFLDocumentHighResolution();
    testXFLDocumentNoNamespace();
    testXFLDocumentMalformedXML();
    testXFLFrameRateFractional();
    printf("\n=== Binary media magic-byte detection tests ===\n");
    testBinaryMediaJPEG();
    testBinaryMediaPNG();
    testBinaryMediaGIF();
    testBinaryMediaUnknown();
    testBinaryMediaTooShort();
    printf("\n=== Zip Slip / path traversal protection tests ===\n");
    testZipSlipTraversalRejected();
    testZipSlipSafePathsAccepted();
    printf("\n=== JSFL function extraction tests ===\n");
    testJSFLFunctionExtraction();
    testJSFLEmptySource();
    printf("=======================================\n");
    printf("Results: %d passed, %d failed\n", g_pass, g_fail);
    return g_fail == 0 ? 0 : 1;
}
