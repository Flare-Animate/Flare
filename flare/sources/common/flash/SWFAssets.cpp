// SWFAssets.cpp - binary Flash container readers and asset extractors
// Copyright (c) 2026 Flare Project
//
// See SWFAssets.h. The bodies below were moved verbatim out of
// flare/sources/flare/flashimport.cpp; they never depended on the scene layer.

#include "SWFAssets.h"
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

// Format detection by content rather than by file extension.
//
// Flash files in the wild routinely carry the wrong extension: ".ssf" and
// ".dat" are used for plain SWFs, and users rename or re-zip FLAs. Dispatching
// purely on the extension meant those files were rejected as "Unsupported Flash
// format" even though the content was perfectly readable.
//
// Everything here looks at the leading bytes only, which is unambiguous: the
// formats we accept all have a distinct magic number.
// ---------------------------------------------------------------------------
DVAPI Format detectFormat(const QString &path) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return Format::Unknown;
    const QByteArray magic = f.read(12);
    f.close();
    if (magic.size() < 4) return Format::Unknown;

    const unsigned char *h = reinterpret_cast<const unsigned char *>(magic.constData());

    // OLE2 / Compound File Binary: legacy binary .fla
    static const unsigned char kOle2[8] = {0xD0, 0xCF, 0x11, 0xE0,
                                           0xA1, 0xB1, 0x1A, 0xE1};
    if (magic.size() >= 8 && std::memcmp(h, kOle2, 8) == 0)
        return Format::Ole2Fla;

    // SWF: "FWS" uncompressed, "CWS" zlib, "ZWS" LZMA
    if (h[0] == 'F' || h[0] == 'C' || h[0] == 'Z')
        if (h[1] == 'W' && h[2] == 'S') return Format::Swf;

    // ZIP: all four valid local-file-header / end-of-directory signatures
    if (h[0] == 'P' && h[1] == 'K' &&
        (h[2] == 0x03 || h[2] == 0x05 || h[2] == 0x07))
        return Format::Zip;

    // ISO base media file format box (f4v / mp4 family)
    if (magic.size() >= 8 && std::memcmp(h + 4, "ftyp", 4) == 0)
        return Format::IsoBmff;

    return Format::Unknown;
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
//   22  DefineBitsJPEG3       JPEG + separate zlib alpha channel
//   23  DefineBitsJPEG4       JPEG + deblocking u16 + zlib alpha
//   24  DefineBitsJPEG5       JPEG + zlib alpha (SWF 13)
//   35  DefineBitsLossless2   zlib; 3 = 8-bit palettized ARGB,
//                             5 = 32-bit premultiplied ARGB
//   36  DefineBitsLossless3   same layout as 35 (SWF 13)
//   90  DefineBitsLossless4   same layout as 35 (SWF 16)
//
// Tag record format (little-endian):
//   Short record: 2-byte word (high 10 bits = tag code, low 6 bits = length)
//   Long record:  2-byte word with length=63, followed by 4-byte signed length
//
// Note the previous revision of this function mis-assigned 35/90 as the JPEG
// variants. 35/36/90 are the zlib lossless family and 22/23/24 are the JPEG
// ones, so real-world SWFs lost every palettized image and every alpha JPEG.
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
    if (alpha.isEmpty()) return false;

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
                           static_cast<unsigned char>(alpha.at(y * width + x)));
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

        // ---- DefineBitsJPEG3 (22) / JPEG4 (23) / JPEG5 (24) ----------------
        // 22: CharacterID(2) + AlphaDataOffset(4) + JPEG + zlib alpha
        // 23: CharacterID(2) + AlphaDataOffset(4) + DeblockParam(2) + JPEG + zlib alpha
        // 24: as 22, with the alpha length stored as a zlib u16 prefix
        if (tagCode == 22 || tagCode == 23 || tagCode == 24) {
            const int headerLen = (tagCode == 23) ? 8 : 6;
            if (dataStart + headerLen >= dataEnd) continue;

            quint32 alphaOffset = static_cast<quint32>(d[dataStart + 2])
                               | (static_cast<quint32>(d[dataStart + 3]) << 8)
                               | (static_cast<quint32>(d[dataStart + 4]) << 16)
                               | (static_cast<quint32>(d[dataStart + 5]) << 24);
            const int maxAlpha = dataEnd - (dataStart + headerLen);
            if (alphaOffset > static_cast<quint32>(maxAlpha))
                alphaOffset = static_cast<quint32>(maxAlpha);

            // Layout: CharacterID(2) AlphaDataOffset(4) JPEG alpha. The JPEG
            // starts at +6; AlphaDataOffset is measured from there.
            QByteArray jpeg = payload(6, static_cast<int>(alphaOffset));
            QByteArray alphaZlib =
                payload(6 + static_cast<int>(alphaOffset),
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

        // ---- DefineBitsLossless (20) and (35) / (36) / (90) ----------------
        // 20 : 3 = 8-bit indexed, 4 = 15-bit RGB555, 5 = 24-bit RGB
        // 35/36/90: 3 = 8-bit palettized premultiplied ARGB, 5 = 32-bit ARGB
        if (tagCode == 20 || tagCode == 35 || tagCode == 36 || tagCode == 90) {
            if (dataStart + 6 >= dataEnd) continue;
            const bool premultiplied = (tagCode != 20);

            const int fmt  = d[dataStart + 2];

            // Tag 35 is DefineBitsLossless2 in the specification, but a fair
            // amount of SWF in the wild stores DefineBitsJPEG3 (CharacterID,
            // AlphaDataOffset, JPEG, zlib alpha) under that code. A format byte
            // outside 3/4/5 is the giveaway, so retry as a JPEG before giving up.
            if (fmt < 3 || fmt > 5) {
                if (tagCode == 35 && dataStart + 6 <= dataEnd) {
                    quint32 off = static_cast<quint32>(d[dataStart + 2])
                               | (static_cast<quint32>(d[dataStart + 3]) << 8)
                               | (static_cast<quint32>(d[dataStart + 4]) << 16)
                               | (static_cast<quint32>(d[dataStart + 5]) << 24);
                    const int avail = dataEnd - (dataStart + 6);
                    if (off > static_cast<quint32>(avail))
                        off = static_cast<quint32>(avail);
                    QByteArray jpeg = payload(6, static_cast<int>(off));
                    QByteArray alpha =
                        payload(6 + static_cast<int>(off), avail - static_cast<int>(off));
                    QImage probe;
                    if (!jpeg.isEmpty() && !alpha.isEmpty() &&
                        probe.loadFromData(jpeg, "JPG") && !probe.isNull()) {
                        if (!saveJpegWithAlpha(jpeg, alpha, probe.width(),
                                               probe.height(), outDir,
                                               bitmapIndex, extracted)) {
                            const QString fname = QString("bitmap_%1.jpg")
                                .arg(bitmapIndex++, 4, 10, QChar('0'));
                            QFile jf(outDir + "/" + fname);
                            if (jf.open(QIODevice::WriteOnly)) {
                                jf.write(jpeg); jf.close();
                            }
                            extracted << fname;
                        }
                    }
                }
                continue;
            }

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

}  // namespace FlashAssets
