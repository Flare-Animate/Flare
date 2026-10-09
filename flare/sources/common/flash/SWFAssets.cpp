// SWFAssets.cpp - binary Flash container readers and asset extractors
// Copyright (c) 2026 Flare Project
//
// See SWFAssets.h. The bodies below were moved verbatim out of
// flare/sources/flare/flashimport.cpp; they never depended on the scene layer.

#include "SWFAssets.h"
#include "SWFShape.h"
#include "tsystem.h"

#include <QDir>
#include <QDebug>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QImage>
#include <QSet>
#include <QVector>

#include <algorithm>
#include <climits>
#include <cstring>

namespace FlashAssets {

// Read the SWF file header and extract basic metadata.
DVAPI SwfInfo readSwfHeader(const QString &path) {
    SwfInfo info;
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return info;

    QByteArray data = f.read(9);  // minimum SWF header size before RECT
    if (data.size() < 4) return info;

    unsigned char sig0 = data[0], sig1 = data[1], sig2 = data[2];
    // Signature: "FWS" (uncompressed), "CWS" (zlib), "ZWS" (LZMA)
    if ((sig0 != 'F' && sig0 != 'C' && sig0 != 'Z') ||
         sig1 != 'W' || sig2 != 'S')
        return info;

    info.valid      = true;
    info.compressed = (sig0 == 'C' || sig0 == 'Z');
    info.version    = static_cast<unsigned char>(data[3]);

    // For uncompressed files we can read frame rect right away.
    // For compressed, we at least have version & file length.
    if (!info.compressed && data.size() >= 9) {
        // After the 8-byte fixed header comes the RECT record (variable bits).
        // Minimum: 1 byte for Nbits, then 4 x Nbits bits.
        // We skip Twips→pixel conversion and just report what we can.
        QByteArray rest = f.read(256);
        data += rest;

        int offset = 8;
        if (offset < data.size()) {
            unsigned char first = static_cast<unsigned char>(data[offset]);
            int nbits = first >> 3;  // high 5 bits = Nbits
            int totalBits = 5 + 4 * nbits;
            int bytesNeeded = (totalBits + 7) / 8;
            if (offset + bytesNeeded + 4 <= data.size()) {
                // Decode RECT via bit stream
                int bitPos = offset * 8 + 5;  // skip Nbits field
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
                // RECT: Xmin, Xmax, Ymin, Ymax in twips (1/20 pixel)
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

                // After RECT: 2-byte frame rate (8.8 fixed), 2-byte frame count
                int afterRect = (bitPos + 7) / 8;
                if (afterRect + 4 <= data.size()) {
                    info.frameRate =
                        static_cast<unsigned char>(data[afterRect + 1]);  // integer part
                    info.frameCount =
                        static_cast<unsigned char>(data[afterRect + 2]) |
                        (static_cast<unsigned char>(data[afterRect + 3]) << 8);
                }
            }
        }
    }

    return info;
}

// ---------------------------------------------------------------------------

// FLV (Flash Video) header reader
//
// Binary format (big-endian, public spec / Ruffle flv crate):
//   Bytes 0-2:  "FLV" signature
//   Byte  3:    version (always 1 for standard FLV)
//   Byte  4:    type flags — bit 0 = has video, bit 2 = has audio
//   Bytes 5-8:  header size (big-endian uint32, standard = 9)
// ---------------------------------------------------------------------------
DVAPI FlvInfo readFlvHeader(const QString &path) {
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

// ---------------------------------------------------------------------------

// F4V (Flash H.264 video, ISO BMFF / MPEG-4 Part 12) header reader
//
// ISO BMFF "ftyp" box (big-endian):
//   Bytes 0-3:  box size (uint32)
//   Bytes 4-7:  box type "ftyp"
//   Bytes 8-11: major brand (4 ASCII chars, e.g. "f4v ", "mp42", "isom")
//   Bytes 12-15: minor version (uint32)
//   Bytes 16+:  compatible brands (4 bytes each)
// ---------------------------------------------------------------------------
DVAPI F4vInfo readF4vHeader(const QString &path) {
    F4vInfo info;
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return info;
    QByteArray hdr = f.read(32);
    if (hdr.size() < 12) return info;
    // box type must be "ftyp"
    if (hdr[4] != 'f' || hdr[5] != 't' || hdr[6] != 'y' || hdr[7] != 'p')
        return info;
    info.valid      = true;
    info.majorBrand = QString::fromLatin1(hdr.mid(8, 4)).trimmed();
    // Collect compatible brands
    QStringList brands;
    for (int off = 16; off + 4 <= hdr.size(); off += 4) {
        QString b = QString::fromLatin1(hdr.mid(off, 4)).trimmed();
        if (!b.isEmpty()) brands << b;
    }
    info.compatBrands = brands.join(", ");
    return info;
}

// Decompress a zlib-compressed SWF body (CWS signature).
// Returns the decompressed full SWF (header patched to FWS), or empty on failure.
DVAPI QByteArray decompressCwsSwf(const QByteArray &swfData) {
    if (swfData.size() < 9 ||
        static_cast<unsigned char>(swfData[0]) != 'C' ||
        static_cast<unsigned char>(swfData[1]) != 'W' ||
        static_cast<unsigned char>(swfData[2]) != 'S')
        return {};

    quint32 uncompLen =
        (quint8)swfData[4]        | ((quint8)swfData[5] << 8) |
        ((quint8)swfData[6] << 16)| ((quint8)swfData[7] << 24);

    // Sanity-cap: reject malformed headers claiming > 100 MB uncompressed
    static constexpr quint32 kMaxSwfUncompressed = 100 * 1024 * 1024u;
    if (uncompLen > kMaxSwfUncompressed) return {};

    QByteArray body = swfData.mid(8);
    QByteArray prefixed(4 + body.size(), '\0');
    prefixed[0] = (uncompLen >> 24) & 0xFF; prefixed[1] = (uncompLen >> 16) & 0xFF;
    prefixed[2] = (uncompLen >> 8)  & 0xFF; prefixed[3] =  uncompLen        & 0xFF;
    memcpy(prefixed.data() + 4, body.constData(), body.size());

    QByteArray inflated = qUncompress(prefixed);
    if (inflated.isEmpty()) return {};

    QByteArray result = swfData.left(8) + inflated;
    result[0] = 'F';  // mark as uncompressed
    return result;
}

// FLA/XFL binary media detection — FLA archives store bitmap media in the
// bin/ directory as .dat files.  These are raw JPEG, PNG, or GIF data with
// no wrapper.  Detect the image type by magic bytes and copy to outDir with
// the correct extension so Flare's level loader can open them.
// Reference: Adobe XFL spec; open-flash/swf-bitmap AGPL-3.0 approach.
DVAPI QStringList extractFLABinaryMedia(const QString &outDir) {
    QStringList extracted;
    QString binDir = outDir + "/bin";
    QDir bin(binDir);
    if (!bin.exists()) return extracted;

    int idx = 0;
    QDirIterator it(binDir, {"*.dat"}, QDir::Files);
    while (it.hasNext()) {
        it.next();
        QFile f(it.filePath());
        if (!f.open(QIODevice::ReadOnly)) continue;
        QByteArray header = f.read(8);
        f.close();
        if (header.size() < 4) continue;

        const unsigned char *h = reinterpret_cast<const unsigned char *>(header.constData());
        QString ext;
        // JPEG: FF D8 FF
        if (h[0] == 0xFF && h[1] == 0xD8 && h[2] == 0xFF)
            ext = "jpg";
        // PNG: 89 50 4E 47
        else if (h[0] == 0x89 && h[1] == 0x50 && h[2] == 0x4E && h[3] == 0x47)
            ext = "png";
        // GIF: 47 49 46 38
        else if (h[0] == 0x47 && h[1] == 0x49 && h[2] == 0x46 && h[3] == 0x38)
            ext = "gif";
        else
            continue;  // unknown binary format

        QString fname = QString("media_%1.%2").arg(idx++, 4, 10, QChar('0')).arg(ext);
        QString dst = outDir + "/" + fname;
        if (!QFile::copy(it.filePath(), dst)) continue;
        extracted << fname;
    }
    return extracted;
}

// ---------------------------------------------------------------------------

// Legacy binary FLA support (Flash CS4 and earlier)
//
// Pre-CS5 .fla files are not ZIP/XFL archives — they are OLE2 / Compound File
// Binary Format (CFBF) documents, identified by the 8-byte magic
// D0 CF 11 E0 A1 B1 1A E1. Flare's ZIP-based importer cannot open them, which
// previously surfaced as a misleading "invalid/corrupt ZIP" error (issue #47).
//
// Full timeline/symbol reconstruction from the binary format is a large effort
// (tracked with the Next2Flash merge). As a first step we (a) detect the format
// and tell the user exactly what it is and how to convert it, and (b) recover
// whatever embedded bitmaps we can, validating each candidate with QImage so a
// false-positive marker in entropy data never produces a broken image.
// ---------------------------------------------------------------------------
DVAPI bool isOle2CompoundFile(const QString &path) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return false;
    QByteArray magic = f.read(8);
    f.close();
    static const unsigned char kOle2[8] = {0xD0, 0xCF, 0x11, 0xE0,
                                           0xA1, 0xB1, 0x1A, 0xE1};
    return magic.size() == 8 &&
           std::memcmp(magic.constData(), kOle2, 8) == 0;
}

// Minimal OLE2 / Compound File Binary Format (CFBF) reader — just enough to walk
// a legacy binary FLA's directory and return each stream's reassembled bytes.
// Legacy FLA bitmaps live in per-symbol streams; reading streams individually
// avoids the cross-stream fragmentation that defeats a whole-file byte scan.
// Reference: [MS-CFB] / the olefile documentation.
namespace {
class CfbfReader {
public:
    explicit CfbfReader(const QByteArray &data) : m_d(data) { m_ok = parse(); }
    bool ok() const { return m_ok; }

    // Bytes of every stream (type == 2) in the compound file.
    QList<QByteArray> streams() const {
        QList<QByteArray> out;
        for (int i = 0; i + 128 <= m_dir.size(); i += 128) {
            if (static_cast<quint8>(m_dir[i + 66]) != 2) continue;  // 2 = stream
            quint32 start = u32(m_dir, i + 116);
            quint64 size  = static_cast<quint64>(u32(m_dir, i + 120)) |
                            (static_cast<quint64>(u32(m_dir, i + 124)) << 32);
            QByteArray blob;
            if (size < m_miniCutoff) {
                blob = readMini(start, static_cast<quint32>(size));
            } else {
                blob = readChain(start);
                // Clamp against the actual chain length rather than casting a
                // 64-bit size to int directly: a corrupt/malicious stream size
                // field could exceed INT_MAX and wrap negative, which would
                // make left() return the wrong (or an empty) blob.
                qint64 wanted = static_cast<qint64>(std::min<quint64>(
                    size, static_cast<quint64>(blob.size())));
                blob = blob.left(static_cast<int>(wanted));
            }
            if (!blob.isEmpty()) out.append(blob);
        }
        return out;
    }

private:
    static quint32 u32(const QByteArray &b, int o) {
        return static_cast<quint32>(static_cast<quint8>(b[o])) |
               (static_cast<quint32>(static_cast<quint8>(b[o + 1])) << 8) |
               (static_cast<quint32>(static_cast<quint8>(b[o + 2])) << 16) |
               (static_cast<quint32>(static_cast<quint8>(b[o + 3])) << 24);
    }
    QByteArray sector(quint32 i) const {
        qint64 off = 512 + static_cast<qint64>(i) * m_secSize;
        if (off < 0 || off + m_secSize > m_d.size()) return QByteArray();
        return m_d.mid(static_cast<int>(off), static_cast<int>(m_secSize));
    }
    QVector<quint32> chainSectors(quint32 start) const {
        QVector<quint32> out; QSet<quint32> seen; quint32 s = start;
        while (s < 0xFFFFFFFE && s < static_cast<quint32>(m_fat.size()) &&
               !seen.contains(s)) {
            seen.insert(s); out.append(s); s = m_fat[static_cast<int>(s)];
        }
        return out;
    }
    QByteArray readChain(quint32 start) const {
        const QVector<quint32> secs = chainSectors(start);
        QByteArray out;
        // Reserve up front: appending sector-by-sector without this causes
        // repeated reallocation/copy (quadratic) for large multi-sector streams.
        out.reserve(static_cast<int>(std::min<qint64>(
            static_cast<qint64>(secs.size()) * m_secSize, INT_MAX)));
        for (quint32 s : secs) out += sector(s);
        return out;
    }
    QByteArray readMini(quint32 start, quint32 size) const {
        QByteArray out; QSet<quint32> seen; quint32 s = start;
        while (s < 0xFFFFFFFE && s < static_cast<quint32>(m_miniFat.size()) &&
               !seen.contains(s)) {
            seen.insert(s);
            out += m_miniStream.mid(static_cast<int>(s) * static_cast<int>(m_miniSize),
                                    static_cast<int>(m_miniSize));
            s = m_miniFat[static_cast<int>(s)];
        }
        return out.left(static_cast<int>(size));
    }
    bool parse() {
        if (m_d.size() < 512) return false;
        static const unsigned char magic[8] = {0xD0, 0xCF, 0x11, 0xE0,
                                               0xA1, 0xB1, 0x1A, 0xE1};
        if (std::memcmp(m_d.constData(), magic, 8) != 0) return false;
        quint16 secShift  = static_cast<quint8>(m_d[30]) | (static_cast<quint8>(m_d[31]) << 8);
        quint16 miniShift = static_cast<quint8>(m_d[32]) | (static_cast<quint8>(m_d[33]) << 8);
        if (secShift < 7 || secShift > 20 || miniShift < 1 || miniShift > 12) return false;
        m_secSize  = 1u << secShift;
        m_miniSize = 1u << miniShift;
        quint32 dirStart     = u32(m_d, 48);
        m_miniCutoff         = u32(m_d, 56);
        quint32 miniFatStart = u32(m_d, 60);
        quint32 difatStart   = u32(m_d, 68);

        QVector<quint32> difat;
        for (int i = 0; i < 109; ++i) difat.append(u32(m_d, 76 + i * 4));
        quint32 nxt = difatStart; int guard = 0;
        while (nxt < 0xFFFFFFFE && guard++ < 1000000) {
            QByteArray sec = sector(nxt);
            if (sec.size() < static_cast<int>(m_secSize)) break;
            int cnt = static_cast<int>(m_secSize) / 4;
            for (int i = 0; i < cnt - 1; ++i) difat.append(u32(sec, i * 4));
            nxt = u32(sec, (cnt - 1) * 4);
        }
        for (quint32 fs : difat) {
            if (fs >= 0xFFFFFFFE) continue;
            QByteArray sec = sector(fs);
            if (sec.size() < static_cast<int>(m_secSize)) continue;
            for (int i = 0; i < static_cast<int>(m_secSize) / 4; ++i)
                m_fat.append(u32(sec, i * 4));
        }
        if (m_fat.isEmpty()) return false;
        m_dir = readChain(dirStart);
        const QVector<quint32> mfSecs = chainSectors(miniFatStart);
        for (quint32 s : mfSecs) {
            QByteArray sec = sector(s);
            for (int i = 0; i < static_cast<int>(m_secSize) / 4; ++i)
                m_miniFat.append(u32(sec, i * 4));
        }
        if (m_dir.size() < 128) return false;
        m_miniStream = readChain(u32(m_dir, 116));  // root entry start = mini stream
        return true;
    }

    const QByteArray &m_d;
    bool m_ok = false;
    quint32 m_secSize = 512, m_miniSize = 64, m_miniCutoff = 4096;
    QVector<quint32> m_fat, m_miniFat;
    QByteArray m_dir, m_miniStream;
};
}  // namespace

DVAPI QStringList extractLegacyFlaBitmaps(const QByteArray &data,
                                           const QString &outDir) {
    QStringList extracted;
    int idx = 0;

    // Carve every embedded image out of one blob: for each signature, take the
    // window up to the next signature and let QImage decode it (QImage stops at
    // the real end of the image and rejects false positives in entropy data).
    auto carveBlob = [&](const QByteArray &blob) {
        auto scan = [&](const QByteArray &sig, const char *qtFormat) {
            int pos = 0, found = 0;
            while (pos < blob.size() && found < 4096) {
                int start = blob.indexOf(sig, pos);
                if (start < 0) break;
                int next = blob.indexOf(sig, start + sig.size());
                int end  = (next < 0) ? blob.size() : next;
                pos = start + sig.size();
                ++found;
                QImage img;
                if (img.loadFromData(blob.mid(start, end - start), qtFormat) &&
                    !img.isNull() && img.width() >= 2 && img.height() >= 2) {
                    QString fname =
                        QString("media_%1.png").arg(idx++, 4, 10, QChar('0'));
                    if (img.save(outDir + "/" + fname, "PNG")) extracted << fname;
                }
            }
        };
        scan(QByteArray("\xFF\xD8\xFF", 3), "JPG");
        scan(QByteArray("\x89PNG\r\n\x1A\n", 8), "PNG");
    };

    // Preferred path: parse the OLE2 compound file and carve each stream on its
    // own (bitmaps are stored per-symbol, so this recovers far more than a
    // whole-file scan and never splices two streams together).
    CfbfReader ole(data);
    if (ole.ok()) {
        const QList<QByteArray> streams = ole.streams();
        for (const QByteArray &s : streams) carveBlob(s);
    }

    // Fallback: if CFBF parsing failed or found nothing, scan the whole file.
    if (extracted.isEmpty()) carveBlob(data);

    return extracted;
}

// ---------------------------------------------------------------------------

// SWF bitmap extractor
//
// Tag codes (Adobe SWF specification; cross-checked against Ruffle's
// swf/src/tag_code.rs, MIT/Apache-2.0):
//    6  DefineBits            JPEG using the global JPEGTables tag
//    8  JPEGTables            global JPEG header table
//   20  DefineBitsLossless    zlib; 3 = 8-bit indexed, 4 = 15-bit RGB555,
//                             5 = 24-bit RGB (NOT premultiplied)
//   21  DefineBitsJPEG2       self-contained JPEG
//   35  DefineBitsJPEG3       JPEG + separate zlib alpha channel
//   36  DefineBitsLossless2   zlib; 3 = 8-bit palettized ARGB,
//                             5 = 32-bit premultiplied ARGB
//   90  DefineBitsJPEG4       JPEG + deblocking u16 + zlib alpha
//
// Not image tags, and all three have been dispatched as one at some point in
// this file's history:
//   22  DefineShape2          a shape, same family as 2 / 32 / 83
//   23  DefineButtonCxform    a button colour transform
//   24  DefineFont2           a font table
// The specification defines no DefineBitsLossless3/4 and no DefineBitsJPEG5;
// the lossless family is 20 and 36 only.
//
// Tag record format (little-endian):
//   Short record: 2-byte word (high 10 bits = tag code, low 6 bits = length)
//   Long record:  2-byte word with length=63, followed by 4-byte signed length
// ---------------------------------------------------------------------------

// Rows of a DefineBitsLossless payload are padded out to a multiple of 4
// *bytes*, so the stride depends on the pixel size, not on the width alone:
// 1 byte/pixel and 3 byte/pixel rows need padding, 2 and 4 byte/pixel rows are
// already 4-byte aligned. Getting this wrong silently truncates every image
// whose width is not a multiple of 4.
static inline int losslessRowStride(int width, int bytesPerPixel) {
    const int raw = width * bytesPerPixel;
    return ((raw + 3) / 4) * 4;
}

// Cap dimensions so a corrupt header cannot ask for an absurd allocation.
static const int kMaxBitmapDim = 16384;

// Inflate a zlib stream. Qt's qUncompress insists on being told the exact
// output size up front, so `expected` must be right or the call fails.
static QByteArray inflateExact(const unsigned char *d, int len, qint64 expected) {
    static constexpr qint64 kMaxUncompressed = 256LL * 1024 * 1024;
    if (expected <= 0 || expected > kMaxUncompressed || len <= 0) return {};

    QByteArray prefixed(4 + len, '\0');
    prefixed[0] = static_cast<char>((expected >> 24) & 0xFF);
    prefixed[1] = static_cast<char>((expected >> 16) & 0xFF);
    prefixed[2] = static_cast<char>((expected >> 8) & 0xFF);
    prefixed[3] = static_cast<char>(expected & 0xFF);
    memcpy(prefixed.data() + 4, d, static_cast<size_t>(len));

    QByteArray out = qUncompress(prefixed);
    return (out.size() == expected) ? out : QByteArray();
}

// Compose a JPEG with its zlib-compressed 8-bit alpha channel into a PNG.
static bool saveJpegWithAlpha(const QByteArray &jpeg, const QByteArray &alphaZlib,
                              int width, int height, const QString &outDir,
                              int &index, QStringList &extracted) {
    if (jpeg.size() < 4 || alphaZlib.isEmpty()) return false;
    if (width <= 0 || height <= 0 || width > kMaxBitmapDim || height > kMaxBitmapDim)
        return false;

    // The alpha channel is one byte per pixel, no row padding.
    const qint64 alphaLen = static_cast<qint64>(width) * height;
    QByteArray alpha = inflateExact(
        reinterpret_cast<const unsigned char *>(alphaZlib.constData()),
        alphaZlib.size(), alphaLen);
    // inflateExact() already returns either exactly alphaLen bytes or nothing;
    // re-stating it here is what makes the unchecked indexing below safe to read.
    if (alpha.size() != alphaLen) return false;

    QImage rgb;
    if (!rgb.loadFromData(jpeg, "JPG") || rgb.isNull()) return false;
    rgb = rgb.convertToFormat(QImage::Format_RGB888);

    QImage out(width, height, QImage::Format_ARGB32);
    for (int y = 0; y < height; ++y) {
        QRgb *dst = reinterpret_cast<QRgb *>(out.scanLine(y));
        for (int x = 0; x < width; ++x) {
            const QColor c = rgb.pixelColor(std::min(x, rgb.width() - 1),
                                            std::min(y, rgb.height() - 1));
            dst[x] = qRgba(c.red(), c.green(), c.blue(),
                           static_cast<unsigned char>(
                               alpha.at(static_cast<int>(
                                   static_cast<qint64>(y) * width + x))));
        }
    }

    const QString fname =
        QString("bitmap_%1.png").arg(index++, 4, 10, QChar('0'));
    if (!out.save(outDir + "/" + fname, "PNG")) return false;
    extracted << fname;
    return true;
}

DVAPI QStringList extractSwfBitmaps(const QByteArray &swfData, const QString &outDir) {
    QStringList extracted;
    if (swfData.size() < 8) return extracted;

    const unsigned char *d = reinterpret_cast<const unsigned char *>(swfData.constData());
    const int size = swfData.size();

    // Skip fixed header (8 bytes) + RECT (variable) + frame_rate (2) + frame_count (2).
    // We parse the RECT to find where the tag stream begins.
    int pos = 8;
    if (pos >= size) return extracted;

    const int nbits = (d[pos] >> 3) & 0x1F;
    const int rectBits = 5 + 4 * nbits;
    pos += (rectBits + 7) / 8;  // skip RECT
    pos += 4;                    // skip frame_rate (2) + frame_count (2)

    int bitmapIndex = 0;
    QByteArray jpegTables;  // from the JPEGTables tag (tag 8)

    while (pos + 2 <= size) {
        const quint16 tagAndLen =
            static_cast<quint16>(d[pos]) | (static_cast<quint16>(d[pos + 1]) << 8);
        pos += 2;

        const int tagCode = (tagAndLen >> 6) & 0x3FF;
        int tagLen = tagAndLen & 0x3F;

        if (tagLen == 63) {
            // Long record: read the 4-byte length.
            if (pos + 4 > size) break;
            const quint32 longLen = static_cast<quint32>(d[pos])
                   | (static_cast<quint32>(d[pos + 1]) << 8)
                   | (static_cast<quint32>(d[pos + 2]) << 16)
                   | (static_cast<quint32>(d[pos + 3]) << 24);
            pos += 4;
            const int remaining = (pos < size) ? (size - pos) : 0;
            // Clamp: a corrupt length must not walk us past the buffer.
            tagLen = (longLen > static_cast<quint32>(remaining)) ? remaining
                                                                : static_cast<int>(longLen);
        }

        if (tagCode == 0) break;  // End tag

        const int dataStart = pos;
        const int dataEnd   = qMin(pos + tagLen, size);
        pos = dataEnd;

        if (tagLen < 2) continue;

        // Bytes [dataStart+off, dataStart+off+len), clamped to the tag body.
        const auto payload = [&](int off, int len) {
            const int avail = dataEnd - dataStart;
            if (off < 0 || len <= 0 || off >= avail) return QByteArray();
            len = qMin(len, avail - off);
            if (len <= 0) return QByteArray();
            return QByteArray(reinterpret_cast<const char *>(d + dataStart + off), len);
        };

        // ---- JPEGTables (8): global header for DefineBits (6) ---------------
        if (tagCode == 8) {
            jpegTables = payload(0, dataEnd - dataStart);
            continue;
        }

        // ---- DefineBitsJPEG2 (21): self-contained JPEG --------------------
        if (tagCode == 21) {
            if (dataStart + 2 >= dataEnd) continue;
            QByteArray jpeg = payload(2, dataEnd - dataStart - 2);
            // Some authoring tools emit a broken "FFD9FFD8" marker run before
            // the real image; strip it (known Ruffle workaround).
            if (jpeg.size() >= 4 &&
                static_cast<unsigned char>(jpeg[0]) == 0xFF &&
                static_cast<unsigned char>(jpeg[1]) == 0xD9 &&
                static_cast<unsigned char>(jpeg[2]) == 0xFF &&
                static_cast<unsigned char>(jpeg[3]) == 0xD8)
                jpeg = jpeg.mid(4);

            const QString fname =
                QString("bitmap_%1.jpg").arg(bitmapIndex++, 4, 10, QChar('0'));
            QFile jf(outDir + "/" + fname);
            if (jf.open(QIODevice::WriteOnly)) { jf.write(jpeg); jf.close(); }
            extracted << fname;
            continue;
        }

        // ---- DefineBitsJPEG3 (35) / DefineBitsJPEG4 (90) -------------------
        // 35: CharacterID(2) + AlphaDataOffset(4) + JPEG + zlib alpha
        // 90: CharacterID(2) + AlphaDataOffset(4) + DeblockParam(2) + JPEG
        //     + zlib alpha
        //
        // These are the codes the SWF specification assigns, and the ones
        // Macromedia.h in this very directory already lists: 22 is
        // stagDefineShape2 and 23 is stagDefineButtonCxform, so dispatching
        // either one here parsed a shape or a button colour transform as image
        // data while real alpha JPEGs fell through to the lossless branch.
        if (tagCode == 35 || tagCode == 90) {
            const int headerLen = (tagCode == 90) ? 8 : 6;
            if (dataStart + headerLen >= dataEnd) continue;

            quint32 alphaOffset = static_cast<quint32>(d[dataStart + 2])
                               | (static_cast<quint32>(d[dataStart + 3]) << 8)
                               | (static_cast<quint32>(d[dataStart + 4]) << 16)
                               | (static_cast<quint32>(d[dataStart + 5]) << 24);
            const int maxAlpha = dataEnd - (dataStart + headerLen);
            if (alphaOffset > static_cast<quint32>(maxAlpha))
                alphaOffset = static_cast<quint32>(maxAlpha);

            // AlphaDataOffset counts the bytes of ImageData, so it is measured
            // from the end of the fixed header -- which JPEG4's DeblockParam
            // makes two bytes longer than JPEG3's.
            QByteArray jpeg = payload(headerLen, static_cast<int>(alphaOffset));
            QByteArray alphaZlib =
                payload(headerLen + static_cast<int>(alphaOffset),
                        maxAlpha - static_cast<int>(alphaOffset));
            if (jpeg.isEmpty() || alphaZlib.isEmpty()) continue;

            // Dimensions are not in the tag; take them from the JPEG itself.
            // If it does not decode, this tag is not a JPEG at all - some
            // authoring tools reuse these codes for other payloads - so drop it
            // rather than writing a file the user cannot open.
            QImage probe;
            if (!probe.loadFromData(jpeg, "JPG") || probe.isNull()) continue;
            if (!saveJpegWithAlpha(jpeg, alphaZlib, probe.width(), probe.height(),
                                   outDir, bitmapIndex, extracted)) {
                // Alpha unusable: keep the JPEG on its own.
                const QString fname =
                    QString("bitmap_%1.jpg").arg(bitmapIndex++, 4, 10, QChar('0'));
                QFile jf(outDir + "/" + fname);
                if (jf.open(QIODevice::WriteOnly)) { jf.write(jpeg); jf.close(); }
                extracted << fname;
            }
            continue;
        }

        // ---- DefineBits (6): JPEG sharing the global JPEGTables ------------
        if (tagCode == 6 && !jpegTables.isEmpty()) {
            if (dataStart + 2 >= dataEnd) continue;
            QByteArray jpeg = jpegTables + payload(2, dataEnd - dataStart - 2);
            const QString fname =
                QString("bitmap_%1.jpg").arg(bitmapIndex++, 4, 10, QChar('0'));
            QFile jf(outDir + "/" + fname);
            if (jf.open(QIODevice::WriteOnly)) { jf.write(jpeg); jf.close(); }
            extracted << fname;
            continue;
        }

        // ---- DefineBitsLossless (20) / DefineBitsLossless2 (36) ------------
        // 20: 3 = 8-bit indexed, 4 = 15-bit RGB555, 5 = 24-bit RGB
        // 36: 3 = 8-bit palettized premultiplied ARGB, 5 = 32-bit ARGB
        if (tagCode == 20 || tagCode == 36) {
            if (dataStart + 6 >= dataEnd) continue;
            const bool premultiplied = (tagCode != 20);

            const int fmt = d[dataStart + 2];
            if (fmt < 3 || fmt > 5) continue;

            const int bmpW = d[dataStart + 3] | (d[dataStart + 4] << 8);
            const int bmpH = d[dataStart + 5] | (d[dataStart + 6] << 8);
            int zlibOff = dataStart + 7;
            int nColors = 0;
            if (fmt == 3) {
                nColors = d[dataStart + 7] + 1;  // stored as (count - 1)
                zlibOff = dataStart + 8;
            }
            if (bmpW <= 0 || bmpH <= 0 || bmpW > kMaxBitmapDim || bmpH > kMaxBitmapDim)
                continue;
            if (zlibOff >= dataEnd) continue;

            const int bytesPerPixel =
                (fmt == 3) ? 1 : (fmt == 4) ? 2 : (fmt == 5) ? (premultiplied ? 4 : 3) : 0;
            if (bytesPerPixel == 0) continue;
            if (bmpW > kMaxBitmapDim / bytesPerPixel) continue;

            // The palettized forms are preceded by a 4-bytes-per-entry RGBA
            // colour table, then stride-padded rows of pixel data.
            const int stride = losslessRowStride(bmpW, bytesPerPixel);
            const qint64 pixelBytes = static_cast<qint64>(stride) * bmpH;
            const qint64 paletteBytes = (fmt == 3) ? 4LL * nColors : 0;

            QByteArray raw = inflateExact(
                d + zlibOff, dataEnd - zlibOff, paletteBytes + pixelBytes);
            if (raw.isEmpty()) continue;

            const int width  = bmpW;
            const int height = bmpH;
            QImage img(width, height, QImage::Format_ARGB32);
            if (img.isNull()) continue;

            const unsigned char *px =
                reinterpret_cast<const unsigned char *>(raw.constData()) +
                static_cast<size_t>(paletteBytes);

            if (fmt == 3) {
                // 4-byte RGBA palette, then one index byte per pixel.
                const unsigned char *pal =
                    reinterpret_cast<const unsigned char *>(raw.constData());
                bool badIndex = false;
                for (int y = 0; y < height && !badIndex; ++y) {
                    QRgb *dst = reinterpret_cast<QRgb *>(img.scanLine(y));
                    const unsigned char *row = px + static_cast<qint64>(y) * stride;
                    for (int x = 0; x < width; ++x) {
                        const int idx = row[x];
                        if (idx < 0 || idx >= nColors) { badIndex = true; break; }
                        const unsigned char *c = pal + static_cast<qint64>(idx) * 4;
                        if (premultiplied) {
                            dst[x] = qRgba(c[0], c[1], c[2], c[3]);
                        } else {
                            dst[x] = qRgba(c[0], c[1], c[2], c[3]);
                        }
                    }
                }
                if (badIndex) continue;
            } else if (fmt == 4) {
                // 15-bit RGB555, padded rows.
                for (int y = 0; y < height; ++y) {
                    QRgb *dst = reinterpret_cast<QRgb *>(img.scanLine(y));
                    const unsigned char *row = px + static_cast<qint64>(y) * stride;
                    for (int x = 0; x < width; ++x) {
                        const int off = x * 2;
                        if (off + 1 >= stride) break;
                        const int v = row[off] | (row[off + 1] << 8);
                        const int r = ((v >> 10) & 0x1F) << 3;
                        const int g = ((v >> 5) & 0x1F) << 3;
                        const int b = (v & 0x1F) << 3;
                        dst[x] = qRgba(r, g, b, 255);
                    }
                }
            } else if (fmt == 5) {
                const int bpp = premultiplied ? 4 : 3;
                for (int y = 0; y < height; ++y) {
                    QRgb *dst = reinterpret_cast<QRgb *>(img.scanLine(y));
                    const unsigned char *row = px + static_cast<qint64>(y) * stride;
                    for (int x = 0; x < width; ++x) {
                        const int off = x * bpp;
                        if (off + bpp > stride) break;
                        if (premultiplied) {
                            // SWF stores premultiplied ARGB; QImage's
                            // ARGB32_Premultiplied matches directly.
                            dst[x] = qRgba(row[off + 1], row[off + 2], row[off + 3],
                                           row[off]);
                        } else {
                            dst[x] = qRgba(row[off], row[off + 1], row[off + 2], 255);
                        }
                    }
                }
            } else {
                continue;  // unknown format byte
            }

            const QString fname =
                QString("bitmap_%1.png").arg(bitmapIndex++, 4, 10, QChar('0'));
            if (!img.save(outDir + "/" + fname, "PNG")) continue;
            extracted << fname;
            continue;
        }
    }

    return extracted;
}


// ---------------------------------------------------------------------------
// Content census and audio extraction
//
// One tag walker, shared so the census and the extractor can never disagree
// about where the tag stream actually is.
//
// Tag codes follow the Adobe SWF specification: the JPEG family is 21 (JPEG2),
// 35 (JPEG3) and 90 (JPEG4), and the lossless family is 20 and 36. Codes 22,
// 23 and 24 are DefineShape2, DefineButtonCxform and DefineFont2 -- not
// bitmaps, whatever earlier revisions of this file assumed.
// ---------------------------------------------------------------------------
namespace {

struct SwfTag {
    int code  = 0;
    int start = 0;
    int end   = 0;
    int len() const { return end - start; }
};

// Walks a SWF tag stream starting from a byte offset.
class TagWalker {
public:
    TagWalker(const unsigned char *d, int size, int pos)
        : m_d(d), m_size(size), m_pos(pos) {}

    bool next(SwfTag &t) {
        if (m_pos + 2 > m_size) return false;
        const unsigned raw = static_cast<unsigned>(m_d[m_pos]) |
                             (static_cast<unsigned>(m_d[m_pos + 1]) << 8);
        m_pos += 2;
        t.code = static_cast<int>((raw >> 6) & 0x3FF);
        int len = static_cast<int>(raw & 0x3F);
        if (len == 63) {
            if (m_pos + 4 > m_size) return false;
            const quint64 v =
                static_cast<quint64>(m_d[m_pos]) |
                (static_cast<quint64>(m_d[m_pos + 1]) << 8) |
                (static_cast<quint64>(m_d[m_pos + 2]) << 16) |
                (static_cast<quint64>(m_d[m_pos + 3]) << 24);
            m_pos += 4;
            const int remaining = (m_pos < m_size) ? (m_size - m_pos) : 0;
            // A corrupt length must not walk us past the buffer.
            len = (v > static_cast<quint64>(remaining)) ? remaining
                                                        : static_cast<int>(v);
        }
        if (t.code == 0) return false;   // End tag
        t.start = m_pos;
        t.end   = qMin(m_pos + len, m_size);
        m_pos   = t.end;
        // A zero-length tag body is legal for some codes but not for a shape, and
        // treating it as "nothing here" makes the walk stop: every later tag in the
        // movie is then skipped, silently. The real movie this was found on has one --
        // an empty DefineShape4 with a 63-byte length that reads as 0 once clamped --
        // and the 2312 bytes after it contain the last four shapes, one of which is a
        // 60-byte DefineShape4 the census counts and the extractor never saw.
        //
        // So: only stop when the tag has no room left in the buffer at all. A tag that
        // is present but empty is reported, and the caller's own length check decides
        // what to make of it.
        return t.end > t.start || len == 0;
    }

private:
    const unsigned char *m_d;
    int m_size;
    int m_pos;
};

// Offset just past the SWF fixed header + RECT + frameRate + frameCount.
int swfBodyOffset(const unsigned char *d, int size) {
    int pos = 8;
    if (pos >= size) return -1;
    const int nbits = (d[pos] >> 3) & 0x1F;
    pos += (5 + 4 * nbits + 7) / 8;
    pos += 4;
    return (pos <= size) ? pos : -1;
}

const int kSwfRates[4] = {5512, 11025, 22050, 44100};

// Audio codec id from the low 4 bits of the SoundFormat byte.
enum SwfSoundFormat {
    kSndUncompressed = 0, kSndADPCM = 1, kSndMP3 = 2, kSndRawPCM = 3,
    kSndNelly8 = 5, kSndNelly = 6, kSndSpeex = 9, kSndAAC = 10
};

struct SwfAudioFormat {
    int codec = kSndADPCM;
    int rate  = 5512;
    QString extension;
};

// Decode the SoundFormat/SoundRate/SoundSize pair that DefineSound and
// SoundStreamHead both carry.
SwfAudioFormat readAudioHeader(const unsigned char *p) {
    SwfAudioFormat a;
    a.codec = p[0] & 0x0F;
    a.rate  = kSwfRates[(p[1] >> 2) & 0x03];
    switch (a.codec) {
    case kSndMP3:    a.extension = "mp3";   break;
    case kSndRawPCM: a.extension = "wav";   break;  // little-endian 16-bit stereo
    case kSndSpeex:
    case kSndAAC:    a.extension = "raw";   break;
    case kSndADPCM:  a.extension = "adpcm"; break;
    default:         a.extension = "raw";   break;  // Nellymoser family
    }
    return a;
}

// First MPEG frame sync, so a DefineSound body can be written as a playable
// .mp3: some encoders prepend a few bytes of padding before the frames.
int mpegSyncOffset(const unsigned char *d, int len) {
    for (int i = 0; i + 1 < len && i < 64; ++i) {
        if (d[i] == 0xFF && (d[i + 1] & 0xE0) == 0xE0) return i;
    }
    return -1;
}

// Wrap 16-bit little-endian PCM in a canonical 44-byte RIFF/WAVE header.
QByteArray pcmToWav(const unsigned char *d, int len, int channels, int rate) {
    if (len < 0) len = 0;
    QByteArray out(44 + len, '\0');
    unsigned char *o = reinterpret_cast<unsigned char *>(out.data());
    const quint32 dataSize = static_cast<quint32>(len);
    const quint32 byteRate = static_cast<quint32>(rate * channels * 2);
    const quint16 blockAlign = static_cast<quint16>(channels * 2);
    auto le16 = [&](int at, quint16 v) {
        o[at] = static_cast<unsigned char>(v & 0xFF);
        o[at + 1] = static_cast<unsigned char>((v >> 8) & 0xFF);
    };
    auto le32 = [&](int at, quint32 v) {
        o[at] = static_cast<unsigned char>(v & 0xFF);
        o[at + 1] = static_cast<unsigned char>((v >> 8) & 0xFF);
        o[at + 2] = static_cast<unsigned char>((v >> 16) & 0xFF);
        o[at + 3] = static_cast<unsigned char>((v >> 24) & 0xFF);
    };
    std::memcpy(o, "RIFF", 4);
    le32(4, 36 + dataSize);
    std::memcpy(o + 8, "WAVEfmt ", 8);
    le32(16, 16);                 // fmt chunk size
    le16(20, 1);                  // PCM
    le16(22, static_cast<quint16>(channels));
    le32(24, static_cast<quint32>(rate));
    le32(28, byteRate);
    le16(32, blockAlign);
    le16(34, 16);                 // bits per sample
    std::memcpy(o + 36, "data", 4);
    le32(40, dataSize);
    if (len > 0) std::memcpy(o + 44, d, static_cast<size_t>(len));
    return out;
}

bool writeAsset(const QString &outDir, const QString &name, const QByteArray &data) {
    QFile f(outDir + "/" + name);
    if (!f.open(QIODevice::WriteOnly)) return false;
    f.write(data);
    f.close();
    return true;
}

void censusInto(const unsigned char *d, int size, int pos, SwfContent &c,
                int depth) {
    TagWalker w(d, size, pos);
    SwfTag t;
    while (w.next(t)) {
        switch (t.code) {
        // Bitmaps. JPEGTables (8) carries a header, not an image, so it is
        // deliberately absent.
        case 6: case 20: case 21: case 35: case 36: case 90:
            ++c.bitmaps; break;
        case 14:  ++c.audio;  break;              // DefineSound
        case 18: case 45: case 89: ++c.streams; break;   // SoundStreamHead/2
        // Shape2 (22) belongs here, not with the bitmaps.
        case 2: case 22: case 32: case 46: case 83: ++c.shapes; break;
        case 11: case 33: ++c.texts; break;        // DefineText / Text2
        // 48 is DefineFont2, not 24. 24 is Protect, which carries no content;
        // counting it as a font made every FLA-exported SWF report a font table
        // it does not have, while missing the font tag it does have. 75 is
        // DefineFont3, written by editors only.
        case 10: case 48: case 75: ++c.fonts; break;  // Font / Font2 / Font3
        case 12: case 59: ++c.actions; break;       // DoAction / DoInitAction
        case 72: case 82: ++c.abc;     break;       // DoABC / DoABCDefine2
        // DefineVideoStream(2) are 60 and 62. 81 and 93 are DefineSceneAndFrame-
        // LabelData and DefineScalingGrid, so this tally was never reachable.
        case 60: case 62: ++c.video;   break;
        // VideoFrame (61) is a frame of the stream above, counted separately so a
        // movie with video can say how much of it there is rather than only that a
        // stream exists.
        case 61: ++c.videoFrames; break;
        case 87: ++c.binary; break;                 // DefineBinaryData
        // Buttons (3, 34) are art a user can see and click; DefineEditText (37) is
        // a text field they can type into. Neither was counted, so a movie built
        // from buttons reported no vector art and no text at all.
        case 3: case 34: ++c.buttons; break;        // DefineButton / DefineButton2
        case 37: ++c.fields;  break;                // DefineEditText
        // SymbolClass (76): the name of every display object in the movie.
        case 76: ++c.symbols; break;

        // Deliberately not counted, because they are timeline structure rather than
        // content, and counting them would make the census useless: in a real 3.5 MB
        // SWF, PlaceObject2 (26), RemoveObject2 (28) and PlaceObject3 (70) are 9,515
        // of 14,882 tags -- 64% -- in a movie with 111 shapes. Reporting "14,882
        // items" for 111 shapes is the opposite of naming what a file holds.
        // FrameLabel (43) is structural for the same reason, as are ExportAssets
        // (56), ImportAssets (57), SetTabIndex (66), FileAttributes (69),
        // CSMTextSettings (74), DefineFontAlignZones (73), DefineScalingGrid (78),
        // DefineSceneAndFrameLabelData (86), DefineFontName (88), Protect (24),
        // JPEGTables (8) and DefineButtonSound (4/7/17).
        // Only codes that appear nowhere else in this switch, because a repeated
        // case label does not compile. 24 is already above, as the negative case
        // that caught the font transposition; 5, 8, 13 and 23 likewise.
        case 26: case 28: case 70: case 43: case 66: case 69: case 73:
        case 74: case 78: case 86: case 88: case 56: case 57:
        case 9: case 77: case 15: case 19: case 58: case 4: case 7: case 17:
            break;                                   // structure, not content
        case 39: {                                  // DefineSprite
            ++c.sprites;
            // The sprite body is its own tag stream, after CharacterID(2) and
            // FrameCount(2). Recurse so nested art is counted too.
            if (depth < 4 && t.len() > 4)
                censusInto(d, size, t.start + 4, c, depth + 1);
            break;
        }
        default: break;
        }
    }
}

}  // namespace

SwfContent censusSwf(const QByteArray &swfData) {
    SwfContent c;
    if (swfData.size() < 9) return c;
    const unsigned char *d =
        reinterpret_cast<const unsigned char *>(swfData.constData());
    const int body = swfBodyOffset(d, swfData.size());
    if (body < 0) return c;
    censusInto(d, swfData.size(), body, c, 0);
    return c;
}

// ---------------------------------------------------------------------------
// Audio extraction
// ---------------------------------------------------------------------------
namespace {

QStringList extractAudioFromRange(const unsigned char *d, int size, int pos,
                                  const QString &outDir, const QString &prefix) {
    QStringList out;
    TagWalker w(d, size, pos);
    SwfTag t;
    int counter = 0;

    // Streaming-audio state, scoped to this call rather than static: the helper
    // is used for both a directly-imported SWF and a SWC's library.swf.
    bool inStream = false;
    QByteArray streamData;
    SwfAudioFormat streamFmt;

    auto flushStream = [&]() {
        if (!inStream || streamData.isEmpty()) { inStream = false; return; }
        const QString ext = streamFmt.extension.isEmpty() ? QString("raw")
                                                         : streamFmt.extension;
        const QString name = QString("%1stream_%2.%3")
                                 .arg(prefix)
                                 .arg(counter, 4, 10, QChar('0'))
                                 .arg(ext);
        bool ok = false;
        if (streamFmt.codec == kSndMP3) {
            const int skip = mpegSyncOffset(
                reinterpret_cast<const unsigned char *>(streamData.constData()),
                streamData.size());
            ok = writeAsset(outDir, name,
                            skip > 0 ? streamData.mid(skip) : streamData);
        } else if (streamFmt.codec == kSndRawPCM) {
            ok = writeAsset(outDir, name,
                            pcmToWav(
                                reinterpret_cast<const unsigned char *>(
                                    streamData.constData()),
                                streamData.size(), 2, streamFmt.rate));
        } else {
            ok = writeAsset(outDir, name, streamData);
        }
        if (ok) out << name;
        streamData.clear();
        inStream = false;
    };

    while (w.next(t)) {
        const unsigned char *p = d + t.start;
        const int len = t.len();

        // ---- DefineSound (14): one self-contained clip ---------------------
        if (t.code == 14 && len > 8) {
            // p[0..1] SoundId, then the SoundFormat/SoundRate pair at p[2..3].
            const SwfAudioFormat fmt = readAudioHeader(p + 2);
            // Both the ADPCM/Nellymoser family and MP3/PCM carry a 16-bit
            // field between the sample count and the data.
            int dataOff = 8;
            if (fmt.codec == kSndMP3) {
                // MP3: UI16 fv where the low nibble is the block size.
                dataOff = 8;
            }
            if (dataOff >= len) continue;

            QByteArray body(reinterpret_cast<const char *>(p + dataOff),
                            len - dataOff);
            if (body.isEmpty()) continue;

            const QString stem = QString("%1sound_%2").arg(prefix).arg(
                counter, 4, 10, QChar('0'));
            bool ok = false;
            if (fmt.codec == kSndMP3) {
                const int skip = mpegSyncOffset(p + dataOff, len - dataOff);
                ok = writeAsset(outDir, stem + ".mp3",
                                skip > 0 ? body.mid(skip) : body);
            } else if (fmt.codec == kSndRawPCM) {
                ok = writeAsset(outDir, stem + ".wav",
                                pcmToWav(p + dataOff, len - dataOff, 2, fmt.rate));
            } else {
                // ADPCM / Nellymoser / Speex / AAC have no container we can
                // write without a codec we do not ship. Keep the bytes under an
                // honest extension so the asset is not silently lost.
                ok = writeAsset(outDir, stem + "." + fmt.extension, body);
            }
            if (ok) out << stem + (fmt.codec == kSndMP3   ? ".mp3"
                                   : fmt.codec == kSndRawPCM ? ".wav"
                                                            : "." + fmt.extension);
            ++counter;
            continue;
        }

        // ---- SoundStreamHead (18 / 45 / 89) opens a stream -----------------
        if (t.code == 18 || t.code == 45 || t.code == 89) {
            flushStream();
            inStream = true;
            streamData.clear();
            streamFmt = (len > 4) ? readAudioHeader(p + 2) : SwfAudioFormat();
            continue;
        }

        // ---- SoundStreamBlock (19 / 60) appends ---------------------------
        if ((t.code == 19 || t.code == 60) && inStream) {
            streamData.append(reinterpret_cast<const char *>(p), len);
            continue;
        }

        // Any other tag closes the stream.
        flushStream();
    }
    flushStream();
    return out;
}

}  // namespace

QStringList extractSwfAudio(const QByteArray &swfData, const QString &outDir) {
    if (swfData.size() < 9) return QStringList();
    const unsigned char *d =
        reinterpret_cast<const unsigned char *>(swfData.constData());
    const int body = swfBodyOffset(d, swfData.size());
    if (body < 0) return QStringList();
    return extractAudioFromRange(d, swfData.size(), body, outDir, QString());
}

// ---------------------------------------------------------------------------
// Vector shape extraction
//
// A SWF's vector art is the one kind of content a movie can be built entirely from,
// and until now it was only counted: the import reported "258 vector shape(s) not
// converted" and wrote nothing. SWFShape decodes all four DefineShape tags, so each
// one becomes an SVG file here, on the same terms as a bitmap or a sound.
//
// What the SVG is and is not: the outline only. A shape's fill and line styles are
// indices into arrays this does not read, so the geometry comes out unpainted. That
// is stated in the file's comment rather than left for the user to discover, and it
// is why the importer says so in its summary.
//
// Morph shapes (DefineMorphShape, 46) are counted but not decoded: their records
// carry a start and an end shape per step and need both at once, which is a
// different decoder rather than an extension of this one.
// ---------------------------------------------------------------------------
namespace {

// A whole SVG document for one shape: the path at 1/20 px per twip, which is the
// same scale XFLShape uses, so a shape and an FLA of the same art come out the same
// size. Width and height are written explicitly as well as the viewBox, because a
// level loader that ignores the viewBox would otherwise get a unit-sized image.
QString shapeToSvgDocument(const SWF::Shape &s, double &outW, double &outH) {
    const QString d = SWF::toSvgPath(s);
    if (d.isEmpty()) return QString();

    // EdgeBounds is the outline's own extent and excludes any stroke; ShapeBounds
    // includes it. Prefer the tighter one so the viewBox is the artwork, not the
    // pen width around it. A degenerate extent still has to produce a valid file.
    QPointF lo = s.hasEdgeBounds ? s.edgeMin : s.boundsMin;
    QPointF hi = s.hasEdgeBounds ? s.edgeMax : s.boundsMax;
    const double w = qMax(hi.x() - lo.x(), 1.0);
    const double h = qMax(hi.y() - lo.y(), 1.0);
    outW = w / 20.0;
    outH = h / 20.0;

    // The path has to be escaped. toSvgPath formats coordinates numerically, so on
    // today's output nothing in it needs escaping -- but a decimal point written as a
    // comma under a locale-aware format, or a stray NaN from a degenerate bounds
    // computation, produces a file that no XML parser will read, and the symptom is a
    // shape that silently fails to load. Escape it rather than assume.
    QString path = d;
    path.replace(QLatin1Char('&'), QLatin1String("&amp;"));
    path.replace(QLatin1Char('<'), QLatin1String("&lt;"));
    path.replace(QLatin1Char('>'), QLatin1String("&gt;"));
    path.replace(QLatin1Char('"'), QLatin1String("&quot;"));

    return QStringLiteral(
               "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
               "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\"\n"
               "     width=\"%1\" height=\"%2\"\n"
               "     viewBox=\"%3 %4 %5 %6\">\n"
               "  <path d=\"%7\" fill=\"#000000\" fill-rule=\"nonzero\"/>\n"
               "</svg>\n")
        .arg(outW, 0, 'f', 3)
        .arg(outH, 0, 'f', 3)
        .arg(lo.x(), 0, 'f', 3)
        .arg(lo.y(), 0, 'f', 3)
        .arg(w, 0, 'f', 3)
        .arg(h, 0, 'f', 3)
        .arg(path);
}

void extractShapesFromRange(const unsigned char *d, int size, int pos,
                            const QString &outDir, const QString &prefix,
                            QStringList &out, int &skipped, int depth) {
    if (depth > 6) return;      // sprites nest; do not follow a cycle
    TagWalker w(d, size, pos);
    SwfTag t;
    int index = 0;
    while (w.next(t)) {
        // Both sprite tags open with a CharacterID. DefineSprite (39) follows it with
        // FRAMETEST records -- a UI16 count then, per record, a UI16 frame count --
        // before the tags start; DefineSprite2 (90) follows it with the tags
        // directly. Treating 39 as though it were 90 lands four bytes into a frame
        // test and finds nothing, which is why a sprite's art was silently dropped
        // rather than reported.
        if ((t.code == 39 || t.code == 90) && t.len() > 2) {
            int inner = t.start + 2;          // past the CharacterID
            if (t.code == 39) {
                if (inner + 2 > t.end) { ++index; continue; }
                const int frameTests =
                    static_cast<int>(d[inner]) | (static_cast<int>(d[inner + 1]) << 8);
                inner += 2;
                for (int f = 0; f < frameTests && inner + 2 <= t.end; ++f) {
                    const int frames =
                        static_cast<int>(d[inner]) | (static_cast<int>(d[inner + 1]) << 8);
                    inner += 2 + 2 * frames;   // UI16 count plus one UI16 per frame
                }
            }
            if (inner < t.end)
                extractShapesFromRange(d, size, inner, outDir,
                                       prefix + QString("sprite%1_")
                                           .arg(index, 3, 10, QChar('0')),
                                       out, skipped, depth + 1);
            ++index;
            continue;
        }
        int version = 0;
        switch (t.code) {
            case 2:  version = 1; break;   // DefineShape
            case 22: version = 2; break;   // DefineShape2
            case 32: version = 3; break;   // DefineShape3
            case 83: version = 4; break;   // DefineShape4
            default: break;
        }
        if (version == 0) continue;

        const QByteArray body(reinterpret_cast<const char *>(d + t.start),
                              t.len());
        const SWF::Shape s = SWF::decodeShape(body, version);
        double pw = 0, ph = 0;
        const QString svg = s.ok ? shapeToSvgDocument(s, pw, ph) : QString();
        if (svg.isEmpty()) {
            // The decoder refuses a tag it cannot read rather than returning a
            // partial outline, so there is nothing to write. Counted so the summary
            // can say how many were left out instead of quietly writing fewer files.
            ++skipped;
            continue;
        }
        const QString name =
            QString("%1shape%2_id%3.svg")
                .arg(prefix)
                .arg(index, 4, 10, QChar('0'))
                .arg(s.id);
        QFile f(QDir(outDir).filePath(name));
        if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate)) continue;
        f.write(svg.toUtf8());
        f.close();
        out << name;
        ++index;
    }
}

}  // namespace

QStringList extractSwfShapes(const QByteArray &swfData, const QString &outDir,
                             int *skipped) {
    QStringList written;
    int bad = 0;
    if (skipped) *skipped = 0;
    if (swfData.size() < 9) return written;
    const unsigned char *d =
        reinterpret_cast<const unsigned char *>(swfData.constData());
    const int body = swfBodyOffset(d, swfData.size());
    if (body < 0) return written;
    extractShapesFromRange(d, swfData.size(), body, outDir, QString(), written,
                           bad, 0);
    if (skipped) *skipped = bad;
    return written;
}



// ---------------------------------------------------------------------------
// Container sniffing
//
// The bytes are authoritative; the extension is only a hint. Everything the
// importer accepts has a distinct magic number, so a SWF named ".fla", or an
// FLA re-zipped as ".zip", still resolves correctly.
// ---------------------------------------------------------------------------

Format detectFormat(const QString &path) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return Format::Unknown;
    const QByteArray magic = f.read(12);
    f.close();
    if (magic.size() < 4) return Format::Unknown;

    const unsigned char *h = reinterpret_cast<const unsigned char *>(magic.constData());

    // OLE2 / Compound File Binary: legacy binary .fla, and Flash Lite .fls.
    static const unsigned char kOle2[8] = {0xD0, 0xCF, 0x11, 0xE0,
                                           0xA1, 0xB1, 0x1A, 0xE1};
    if (magic.size() >= 8 && std::memcmp(h, kOle2, 8) == 0) return Format::Ole2Fla;

    // SWF: "FWS" uncompressed, "CWS" zlib, "ZWS" LZMA. The same header is used
    // by .swz (a SWF whose sounds ship pre-compressed) and by .ksk (a
    // keystroke-signed SWF), so both are recognised here rather than by name.
    if ((h[0] == 'F' || h[0] == 'C' || h[0] == 'Z') && h[1] == 'W' && h[2] == 'S')
        return Format::Swf;

    // ZIP: every valid local-header / end-of-directory signature.
    if (h[0] == 'P' && h[1] == 'K' &&
        (h[2] == 0x03 || h[2] == 0x05 || h[2] == 0x07))
        return Format::Zip;

    // ISO base media file format: .f4v, .m4v, .mp4.
    if (magic.size() >= 8 && std::memcmp(h + 4, "ftyp", 4) == 0)
        return Format::IsoBmff;

    // Photoshop and Illustrator: Adobe formats that travel with Flash assets.
    if (std::memcmp(h, "8BPS", 4) == 0) return Format::Psd;
    if (std::memcmp(h, "%PDF", 4) == 0) return Format::Pdf;

    return Format::Unknown;
}

QStringList supportedExtensions() {
    return {
        // Flash project, both generations
        "fla", "xfl", "fls",
        // Compiled Flash
        "swf", "swz", "swc", "sol", "ksk",
        // Mislabeled payloads: ".ssf" is a plain SWF in the wild, and users
        // re-zip FLAs because some hosts refuse ".fla" uploads.
        "ssf", "dat", "zip",
        // Video
        "flv", "f4v", "m4v",
        // ActionScript source
        "as", "asc", "mxml", "jsfl",
        // Packaging / extensions
        "zxp", "mxp", "ane", "air", "oam",
        // Copied for reference; not parsed
        "lwf", "rsl", "afl",
        // Photoshop / Illustrator
        "psd", "psb", "ai",
    };
}

}  // namespace FlashAssets
