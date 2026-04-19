// flashimport.cpp - Native built-in Flash format import (FLA, XFL, SWF, SWC, FLV, F4V, AS)
// No external tools or third-party processes required.
//
// FLA  = ZIP archive containing an XFL document (DOMDocument.xml + assets)
// XFL  = Unzipped FLA; a directory or ZIP containing DOMDocument.xml
// SWF  = Compiled Flash binary (header + tag stream)
// SWC  = ZIP archive containing library.swf and assets (Apache Flex SDK format)
// FLV  = Flash Video container
// F4V  = Flash H.264 video (ISO BMFF / MPEG-4 Part 12)
// AS   = ActionScript source file (imported as plain text for reference)

#include "flare/menubarcommandids.h"
#include "flare/menubar.h"
#include "flare/ocaio.h"
#include "flare/tproject.h"
#include "flare/preferences.h"
#include "flare/tapp.h"
#include "flare/tscenehandle.h"
#include "flare/txsheethandle.h"
#include "flare/toonzfolders.h"
#include "flare/toonzscene.h"
#include "flare/txsheet.h"
#include "flare/txshcell.h"
#include "flare/txshsimplelevel.h"
#include "flare/txshlevelcolumn.h"
#include "flare/tstageobject.h"

#include "flareqt/gutil.h"
#include "flareqt/dvdialog.h"
#include "flare/filebrowserpopup.h"
#include "iocommand.h"

#include "tsystem.h"
#include "tfilepath.h"

// Native flash infrastructure (include_directories contains ../common/flash)
#include "XFLReader.h"
#include "FSWFStream.h"
#include "Macromedia.h"

#include <QFile>
#include <QDir>
#include <QDirIterator>
#include <QDateTime>
#include <QFileInfo>
#include <QDesktopServices>
#include <QUrl>
#include <QDebug>
#include <QFileDialog>

// Minizip for ZIP/FLA/SWC extraction (include_directories contains minizip path)
#include "unzip.h"

#include <fstream>
#include <cstring>

using namespace DVGui;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

namespace {

// Create a unique timestamped output directory under the system temp dir.
static TFilePath makeTempImportDir(const QString &prefix) {
    QString name = prefix + QString::number(QDateTime::currentMSecsSinceEpoch());
    TFilePath dir = TSystem::getTempDir() + TFilePath(name.toStdString());
    try { TSystem::mkDir(dir); } catch (...) {}
    return dir;
}

// Asset file filters for auto-import scan
static const QStringList kAssetFilters = {
    "*.png", "*.jpg", "*.jpeg", "*.svg", "*.xml", "*.as"
};

// Validate that a resolved path stays under the intended directory (Zip Slip guard).
static bool isPathUnderDir(const QString &dir, const QString &candidate) {
    QDir base(dir);
    QString canonical = QFileInfo(candidate).canonicalFilePath();
    // If the file doesn't exist yet, canonicalFilePath returns empty; fall back
    if (canonical.isEmpty())
        canonical = QFileInfo(candidate).absoluteFilePath();
    QString baseCanonical = base.absolutePath();
    if (!baseCanonical.endsWith('/')) baseCanonical += '/';
    return canonical.startsWith(baseCanonical);
}

// Extract every entry of a ZIP archive to outDir using the bundled minizip.
static bool extractZip(const QString &zipPath, const QString &outDir) {
    unzFile uf = unzOpen(zipPath.toUtf8().constData());
    if (!uf) return false;

    unz_global_info gi;
    if (unzGetGlobalInfo(uf, &gi) != UNZ_OK) { unzClose(uf); return false; }

    char entryName[1024];   // larger buffer for deeply-nested paths
    char buf[16384];

    for (uLong i = 0; i < gi.number_entry; i++) {
        unz_file_info fi;
        if (unzGetCurrentFileInfo(uf, &fi, entryName, sizeof(entryName),
                                  nullptr, 0, nullptr, 0) != UNZ_OK)
            break;

        size_t nameLen = strlen(entryName);
        if (nameLen == 0) {
            if (i + 1 < gi.number_entry) unzGoToNextFile(uf);
            continue;
        }

        QString entryStr = QString::fromUtf8(entryName);

        // Normalize path separators and strip leading ./ (common in ZIP entries)
        entryStr.replace('\\', '/');
        while (entryStr.startsWith("./"))
            entryStr = entryStr.mid(2);
        while (entryStr.startsWith('/'))
            entryStr = entryStr.mid(1);

        // Zip Slip protection: reject absolute paths and path traversal
        if (entryStr.startsWith('/') || entryStr.startsWith('\\') ||
            entryStr.contains("../") || entryStr.contains("..\\") ||
            entryStr.endsWith("..") ||
            (entryStr.length() >= 2 && entryStr[1] == ':')) {
            if (i + 1 < gi.number_entry) unzGoToNextFile(uf);
            continue;
        }

        QString fullOut = outDir + "/" + entryStr;
        if (!isPathUnderDir(outDir, fullOut)) {
            if (i + 1 < gi.number_entry) unzGoToNextFile(uf);
            continue;
        }

        QFileInfo info(fullOut);

        if (entryName[nameLen - 1] == '/') {
            // Directory entry
            QDir().mkpath(fullOut);
        } else {
            QDir().mkpath(info.absolutePath());
            if (unzOpenCurrentFile(uf) == UNZ_OK) {
                QFile outFile(fullOut);
                if (outFile.open(QIODevice::WriteOnly)) {
                    int n;
                    while ((n = unzReadCurrentFile(uf, buf, sizeof(buf))) > 0)
                        outFile.write(buf, n);
                    outFile.close();
                }
                unzCloseCurrentFile(uf);
            }
        }

        if (i + 1 < gi.number_entry && unzGoToNextFile(uf) != UNZ_OK) break;
    }
    unzClose(uf);
    return true;
}

// Read the SWF file header and extract basic metadata.
struct SwfInfo {
    bool valid = false;
    bool compressed = false;  // zlib-compressed (SWF6+) or LZMA (SWF13+)
    int  version   = 0;
    int  width     = 0;
    int  height    = 0;
    int  frameRate = 0;
    int  frameCount = 0;
};

static SwfInfo readSwfHeader(const QString &path) {
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
struct F4vInfo {
    bool    valid       = false;
    QString majorBrand;
    QString compatBrands;
};

static F4vInfo readF4vHeader(const QString &path) {
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
static QByteArray decompressCwsSwf(const QByteArray &swfData) {
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

// Build a plain-text manifest listing imported files in outDir.
static void writeManifest(const QString &outDir, const QStringList &files,
                          const QString &sourceFile) {
    QFile mf(outDir + "/manifest.txt");
    if (!mf.open(QIODevice::WriteOnly | QIODevice::Text)) return;
    mf.write(QByteArray("Source: ") + sourceFile.toUtf8() + "\n");
    mf.write("Exported files:\n");
    for (const auto &f : files) mf.write(QByteArray("  ") + f.toUtf8() + "\n");
    mf.close();
}

// FLA/XFL binary media detection — FLA archives store bitmap media in the
// bin/ directory as .dat files.  These are raw JPEG, PNG, or GIF data with
// no wrapper.  Detect the image type by magic bytes and copy to outDir with
// the correct extension so Flare's level loader can open them.
// Reference: Adobe XFL spec; open-flash/swf-bitmap AGPL-3.0 approach.
static QStringList extractFLABinaryMedia(const QString &outDir) {
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
// SWF bitmap extractor
//
// Tag codes (from Ruffle swf/src/tag_code.rs, MIT/Apache-2.0):
//   DefineBits         = 6   (JPEG with separate JPEGTables tag)
//   JpegTables         = 8   (global JPEG header)
//   DefineBitsLossless = 20  (zlib-compressed palettized / RGB)
//   DefineBitsJpeg2    = 21  (self-contained JPEG)
//   DefineBitsJpeg3    = 35  (JPEG + separate alpha channel)
//   DefineBitsLossless2= 36  (zlib-compressed RGBA)
//   DefineBitsJpeg4    = 90  (JPEG with deblocking parameter)
//
// Tag record format (little-endian):
//   Short record: 2-byte word (high 10 bits = tag code, low 6 bits = length)
//   Long record:  2-byte word with length=63, followed by 4-byte signed length
//
// Bitmap format constants (DefineBitsLossless format byte):
//   3 = 8-bit palettized,  4 = 15-bit RGB555,  5 = 24-bit RGB/32-bit ARGB
// ---------------------------------------------------------------------------
static QStringList extractSwfBitmaps(const QByteArray &swfData, const QString &outDir) {
    QStringList extracted;
    if (swfData.size() < 8) return extracted;

    const unsigned char *d = reinterpret_cast<const unsigned char *>(swfData.constData());
    int size = swfData.size();

    // Skip fixed header (8 bytes) + RECT (variable) + frame_rate (2) + frame_count (2)
    // We parse the RECT to find where tags begin.
    int pos = 8;  // after sig(3) + version(1) + fileLen(4)
    if (pos >= size) return extracted;

    int nbits = (d[pos] >> 3) & 0x1F;
    int rectBits = 5 + 4 * nbits;
    pos += (rectBits + 7) / 8;  // skip RECT
    pos += 4;                    // skip frame_rate (2) + frame_count (2)

    int bitmapIndex = 0;
    QByteArray jpegTables;  // from JpegTables tag (tag 8)

    while (pos + 2 <= size) {
        // Read tag record header
        quint16 tagAndLen = static_cast<quint16>(d[pos]) | (static_cast<quint16>(d[pos+1]) << 8);
        pos += 2;

        int tagCode = (tagAndLen >> 6) & 0x3FF;
        int tagLen  = tagAndLen & 0x3F;

        if (tagLen == 63) {
            // Long record: read 4-byte unsigned length
            if (pos + 4 > size) break;
            quint32 longLen = static_cast<quint32>(d[pos])
                   | (static_cast<quint32>(d[pos+1]) << 8)
                   | (static_cast<quint32>(d[pos+2]) << 16)
                   | (static_cast<quint32>(d[pos+3]) << 24);
            pos += 4;
            // Validate: reject absurd lengths that would exceed remaining data
            int remaining = (pos < size) ? (size - pos) : 0;
            if (longLen > static_cast<quint32>(remaining)) {
                tagLen = remaining;  // clamp to remaining
            } else {
                tagLen = static_cast<int>(longLen);
            }
        }

        if (tagCode == 0) break;  // End tag

        // Clamp to available data
        int dataStart = pos;
        int dataEnd   = qMin(pos + tagLen, size);
        pos           = dataEnd;

        if (tagLen < 2) continue;

        // ---- JpegTables (tag 8): save for use with DefineBits ----
        if (tagCode == 8) {
            jpegTables = QByteArray(reinterpret_cast<const char *>(d + dataStart),
                                    dataEnd - dataStart);
            continue;
        }

        // ---- DefineBitsJpeg2 (21), DefineBitsJpeg4 (90): self-contained JPEG ----
        // Format: CharacterID (2 bytes) + raw JPEG data
        if (tagCode == 21 || tagCode == 90) {
            int skip = (tagCode == 90) ? 4 : 2;  // Jpeg4 has extra deblocking u16
            if (dataStart + skip >= dataEnd) continue;

            QByteArray jpeg(reinterpret_cast<const char *>(d + dataStart + skip),
                            dataEnd - dataStart - skip);

            // Some SWF authoring tools write a broken JPEG header (0xFF 0xD9 0xFF 0xD8)
            // before the actual image data. Strip it (known Ruffle workaround).
            if (jpeg.size() >= 4 &&
                (unsigned char)jpeg[0] == 0xFF && (unsigned char)jpeg[1] == 0xD9 &&
                (unsigned char)jpeg[2] == 0xFF && (unsigned char)jpeg[3] == 0xD8)
                jpeg = jpeg.mid(4);

            QString fname = QString("bitmap_%1.jpg").arg(bitmapIndex++, 4, 10, QChar('0'));
            QFile jf(outDir + "/" + fname);
            if (jf.open(QIODevice::WriteOnly)) { jf.write(jpeg); jf.close(); }
            extracted << fname;
            continue;
        }

        // ---- DefineBits (6): JPEG data using the global JpegTables ----
        if (tagCode == 6 && !jpegTables.isEmpty()) {
            if (dataStart + 2 >= dataEnd) continue;
            QByteArray jpeg = jpegTables +
                QByteArray(reinterpret_cast<const char *>(d + dataStart + 2),
                           dataEnd - dataStart - 2);
            QString fname = QString("bitmap_%1.jpg").arg(bitmapIndex++, 4, 10, QChar('0'));
            QFile jf(outDir + "/" + fname);
            if (jf.open(QIODevice::WriteOnly)) { jf.write(jpeg); jf.close(); }
            extracted << fname;
            continue;
        }

        // ---- DefineBitsJpeg3 (35): JPEG + separate zlib alpha channel ----
        // Format: CharID(2) + alphaDataOffset(4) + JPEG data + zlib alpha
        if (tagCode == 35) {
            const int jpegDataStart = dataStart + 6;
            const int jpegMaxLen    = dataEnd - jpegDataStart;
            if (jpegMaxLen <= 0) continue;
            quint32 alphaOffset = static_cast<quint32>(d[dataStart+2])
                                | (static_cast<quint32>(d[dataStart+3]) << 8)
                                | (static_cast<quint32>(d[dataStart+4]) << 16)
                                | (static_cast<quint32>(d[dataStart+5]) << 24);
            if (alphaOffset > static_cast<quint32>(jpegMaxLen))
                alphaOffset = static_cast<quint32>(jpegMaxLen);
            QByteArray jpeg(reinterpret_cast<const char *>(d + jpegDataStart),
                            static_cast<int>(alphaOffset));
            // We save only the JPEG data (alpha channel would require compositing)
            QString fname = QString("bitmap_%1.jpg").arg(bitmapIndex++, 4, 10, QChar('0'));
            QFile jf(outDir + "/" + fname);
            if (jf.open(QIODevice::WriteOnly)) { jf.write(jpeg); jf.close(); }
            extracted << fname;
            continue;
        }

        // ---- DefineBitsLossless2 (36): zlib-compressed ARGB bitmap ----
        // Format: CharID(2) + BitmapFormat(1) + width(2) + height(2)
        //         [+ ColorTableSize(1) if format==3] + zlib(pixel data)
        if (tagCode == 36) {
            if (dataStart + 7 >= dataEnd) continue;
            // int charId   = ... (unused)
            int fmt      = d[dataStart + 2];
            int bmpW     = d[dataStart + 3] | (d[dataStart + 4] << 8);
            int bmpH     = d[dataStart + 5] | (d[dataStart + 6] << 8);
            int zlibOff  = dataStart + 7;
            if (fmt == 3) zlibOff++;  // skip ColorTableSize byte

            if (bmpW <= 0 || bmpH <= 0 || zlibOff >= dataEnd) continue;

            // Sanity limits on bitmap dimensions to avoid huge allocations
            const int kMaxBitmapDim = 16384;
            if (bmpW > kMaxBitmapDim || bmpH > kMaxBitmapDim) continue;

            // Decompress zlib pixel data using Qt
            QByteArray compressed(reinterpret_cast<const char *>(d + zlibOff),
                                  dataEnd - zlibOff);
            if (compressed.isEmpty()) continue;

            // Compute uncompressed length in 64-bit to avoid overflow
            static constexpr qint64 kMaxUncompressedLen = 256LL * 1024 * 1024;
            qint64 uncompLen64 = static_cast<qint64>(bmpW) * static_cast<qint64>(bmpH) * 4;
            if (uncompLen64 <= 0 || uncompLen64 > kMaxUncompressedLen) continue;
            int uncompLen = static_cast<int>(uncompLen64);

            // Prepend a 4-byte big-endian uncompressed length for qUncompress
            QByteArray prefixed(4 + compressed.size(), 0);
            prefixed[0] = (uncompLen >> 24) & 0xFF;
            prefixed[1] = (uncompLen >> 16) & 0xFF;
            prefixed[2] = (uncompLen >> 8)  & 0xFF;
            prefixed[3] =  uncompLen        & 0xFF;
            memcpy(prefixed.data() + 4, compressed.constData(), compressed.size());

            QByteArray pixels = qUncompress(prefixed);
            if (pixels.size() < uncompLen) continue;  // decompression failed

            // SWF lossless2 stores 32-bit ARGB (premultiplied alpha).
            // Convert to QImage ARGB32_Premultiplied and save as PNG.
            QImage img(reinterpret_cast<const uchar *>(pixels.constData()),
                       bmpW, bmpH, bmpW * 4, QImage::Format_ARGB32_Premultiplied);
            QString fname = QString("bitmap_%1.png").arg(bitmapIndex++, 4, 10, QChar('0'));
            img.save(outDir + "/" + fname, "PNG");
            extracted << fname;
            continue;
        }
    }

    return extracted;
}

// Open the output folder in the system file manager.
static void openFolder(const QString &path) {
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}

// ---------------------------------------------------------------------------
// Copy a single file to outDir and add its filename to the exported list.
// Used by text/binary format handlers (AS, ASC, MXML, LWF, RSL, AFL).
// ---------------------------------------------------------------------------
static void copyFileForReference(const QString &srcPath, const QString &outDir,
                                 QStringList &exported, const QString &infoMsg = {}) {
    QString fname = QFileInfo(srcPath).fileName();
    QString dst   = outDir + "/" + fname;
    QFile::copy(srcPath, dst);
    exported << fname;
    (void)infoMsg;
}

// ---------------------------------------------------------------------------
// ANE / AIR / OAM — ZIP-based Adobe packaging formats.
//
// ANE  (Adobe Native Extension) — ZIP; contains META-INF/ANE/extension.xml
// AIR  (Adobe AIR application)  — ZIP; contains META-INF/AIR/application.xml
// OAM  (Open Architecture Mod.) — ZIP; contains OAMMetadata.xml or META-INF/OAM/metadata.xml
//
// All three use the same ZIP extraction pipeline as FLA/SWC.
// Format knowledge: Adobe AIR SDK Reference, Adobe Animate OAM spec.
// ---------------------------------------------------------------------------
static QString extractAdobeZipPackage(const QString &srcPath, const QString &outDir,
                                      const QString &ext) {
    if (!extractZip(srcPath, outDir)) return {};

    // Find and report the manifest/metadata XML specific to each format.
    QStringList candidates;
    if (ext == "ane")
        candidates = {"META-INF/ANE/extension.xml", "META-INF/extension.xml"};
    else if (ext == "air")
        candidates = {"META-INF/AIR/application.xml", "META-INF/MANIFEST.MF"};
    else if (ext == "oam")
        candidates = {"OAMMetadata.xml", "META-INF/OAM/metadata.xml", "metadata.xml"};

    for (const QString &rel : candidates) {
        QFile f(outDir + "/" + rel);
        if (f.exists()) {
            return rel;  // return the found manifest path
        }
    }
    return {};
}

// ---------------------------------------------------------------------------
// Native FLA/XFL scene import
//
// After XFLReader has fully parsed the document (including timelines),
// this function populates the active Flare xsheet:
//   - each DOMLayer  → TXshLevelColumn
//   - each DOMBitmapItem → PNG loaded via IoCmd::loadResources
//   - each DOMFrame span → cells set via TXsheet::setCell
//
// Approach:
//   1. Map libraryItemName → TFilePath from parsed BitmapItem list
//   2. Load each bitmap once using IoCmd::loadResources (expose=false)
//   3. Iterate layers (topmost XFL layer = leftmost Flare column)
//   4. For each keyframe with a BITMAP_INSTANCE element, set cells for its duration
//
// References:
//   jpexs-decompiler XFLConverter (GPL-3.0) — layer/frame/element model (ideas only)
//   fla-viewer (MIT) — attribute interpretation (ideas only)
//   OCA import (ocaio.cpp) — xsheet insertion pattern
// ---------------------------------------------------------------------------
static void importXFLScene(ToonzScene *scene, TXsheet *xsheet,
                            const XFL::Document &doc,
                            const TFilePath &xflBaseDir) {
    if (doc.timelines.empty()) return;

    // Build libraryItemName → absolute file path
    QMap<QString, TFilePath> bitmapPaths;
    for (const XFL::BitmapItem &bi : doc.bitmaps) {
        QString href = QString::fromStdString(bi.href);
        href.replace('\\', '/');
        TFilePath fp = xflBaseDir + TFilePath(href.toStdString());
        if (TSystem::doesExistFileOrLevel(fp))
            bitmapPaths[QString::fromStdString(bi.name)] = fp;
    }
    if (bitmapPaths.isEmpty()) return;

    // Load all referenced bitmaps; collect TXshSimpleLevel* per name.
    // IoCmd::loadResources(expose=false) → levels added to scene but no column auto-inserted.
    QMap<QString, TXshSimpleLevel *> bitmapLevels;
    for (auto it = bitmapPaths.constBegin(); it != bitmapPaths.constEnd(); ++it) {
        IoCmd::LoadResourceArguments args;
        args.expose = false;
        args.resourceDatas.emplace_back(it.value());
        IoCmd::loadResources(args);
        if (!args.loadedLevels.empty()) {
            TXshLevel *lv = *args.loadedLevels.begin();
            if (TXshSimpleLevel *sl = lv ? dynamic_cast<TXshSimpleLevel *>(lv) : nullptr)
                bitmapLevels[it.key()] = sl;
        }
    }
    if (bitmapLevels.isEmpty()) return;

    const XFL::XFLTimeline &tl = doc.timelines[0];

    // Iterate layers: XFL layers[0] = topmost visual layer → col 0 in Flare.
    for (const XFL::XFLLayer &layer : tl.layers) {
        if (layer.layerType == "guide" || layer.layerType == "folder") continue;

        // Skip layers with no bitmap elements
        bool hasCells = false;
        for (const XFL::XFLFrame &fr : layer.frames)
            if (!fr.elements.empty()) { hasCells = true; break; }
        if (!hasCells) continue;

        int col = xsheet->getFirstFreeColumnIndex();
        TXshLevelColumn *column = new TXshLevelColumn();
        xsheet->insertColumn(col, column);

        if (!layer.name.empty()) {
            TStageObject *obj = xsheet->getStageObject(TStageObjectId::ColumnId(col));
            if (obj) obj->setName(layer.name);
        }

        for (const XFL::XFLFrame &frame : layer.frames) {
            // Use only the first bitmap element per frame span
            for (const XFL::FrameElement &el : frame.elements) {
                if (el.type != XFL::FrameElement::BITMAP_INSTANCE) continue;
                TXshSimpleLevel *sl = bitmapLevels.value(
                    QString::fromStdString(el.libraryItemName), nullptr);
                if (!sl) continue;
                TXshCell cell(sl, TFrameId(1));  // PNG = single frame, id=1
                for (int r = frame.index; r < frame.index + frame.duration; ++r)
                    xsheet->setCell(r, col, cell);
                break;
            }
        }
    }

    xsheet->updateFrameCount();
    TApp::instance()->getCurrentLevel()->notifyLevelChange();
    TApp::instance()->getCurrentScene()->notifyCastChange();
    TApp::instance()->getCurrentXsheet()->notifyXsheetChanged();
}

} // anonymous namespace

// ---------------------------------------------------------------------------
// Command: Import Flash (FLA / XFL / SWC / SWF / AS) — fully native
// ---------------------------------------------------------------------------

class ImportFlashVectorCommand final : public MenuItemHandler {
public:
    ImportFlashVectorCommand() : MenuItemHandler(MI_ImportFlashVector) {}
    void execute() override;
} g_importFlashVectorCommand;

void ImportFlashVectorCommand::execute() {
    TApp *app = TApp::instance();
    TSceneHandle *sceneHandle = app->getCurrentScene();
    ToonzScene *scene = sceneHandle->getScene();

    static GenericLoadFilePopup *loadPopup = nullptr;
    if (!loadPopup) {
        loadPopup = new GenericLoadFilePopup(
            QObject::tr("Import Flash / Animate File"));
        // Core Flash / Animate project formats
        loadPopup->addFilterType("fla");
        loadPopup->addFilterType("xfl");
        loadPopup->addFilterType("swf");
        loadPopup->addFilterType("swc");
        // ActionScript source
        loadPopup->addFilterType("as");
        loadPopup->addFilterType("asc");
        loadPopup->addFilterType("mxml");
        // Video
        loadPopup->addFilterType("flv");
        loadPopup->addFilterType("f4v");
        // AIR / ANE / OAM packaging
        loadPopup->addFilterType("air");
        loadPopup->addFilterType("ane");
        loadPopup->addFilterType("oam");
        // Other Flash-ecosystem formats
        loadPopup->addFilterType("lwf");
        loadPopup->addFilterType("rsl");
        loadPopup->addFilterType("afl");
    }

    if (!scene->isUntitled())
        loadPopup->setFolder(scene->getScenePath().getParentDir());
    else
        loadPopup->setFolder(
            TProjectManager::instance()->getCurrentProject()->getScenesPath());

    TFilePath fp = loadPopup->getPath();
    if (fp.isEmpty()) return;

    TFilePath outDir = makeTempImportDir("flare_flash_import_");
    QString   outPath = outDir.getQString();
    QString   srcPath = fp.getQString();
    QString   ext     = QString::fromStdString(fp.getType()).toLower();
    QStringList exported;
    QString info;

    // ---- FLA / XFL / SWC : ZIP-based container ----
    if (ext == "fla" || ext == "swc" || (ext == "xfl" && XFL::isFLAZipBased(fp))) {

        if (!extractZip(srcPath, outPath)) {
            DVGui::error(QObject::tr("Failed to extract archive (invalid/corrupt ZIP): %1").arg(srcPath));
            return;
        }

        if (ext == "swc") {
            // SWC (Flex/Flash component library) format (Apache Flex SDK reference,
            // Apache License 2.0): ZIP containing catalog.xml + library.swf.
            //
            // catalog.xml structure:
            //   <swc xmlns="http://www.adobe.com/flash/swccatalog/9">
            //     <components>
            //       <component className="..." name="..." uri="..."/>
            //     </components>
            //   </swc>
            QFile catFile(outPath + "/catalog.xml");
            if (catFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
                QString catXml = QString::fromUtf8(catFile.readAll());
                catFile.close();
                int compCount = 0;
                QStringList compNames;
                int searchPos = 0;
                while (true) {
                    int idx = catXml.indexOf("<component ", searchPos, Qt::CaseInsensitive);
                    if (idx < 0) break;
                    compCount++;
                    int nameIdx = catXml.indexOf("name=\"", idx);
                    if (nameIdx >= 0 && nameIdx < idx + 200) {
                        int nameStart = nameIdx + 6;
                        int nameEnd   = catXml.indexOf('"', nameStart);
                        if (nameEnd > nameStart)
                            compNames << catXml.mid(nameStart, nameEnd - nameStart);
                    }
                    searchPos = idx + 1;
                }
                info = QObject::tr("SWC: %1 component(s) exported").arg(compCount);
                if (!compNames.isEmpty())
                    info += "\n  " + compNames.join(", ");
            }

            // Extract bitmaps from the embedded library.swf
            QFile libSwf(outPath + "/library.swf");
            if (libSwf.open(QIODevice::ReadOnly)) {
                QByteArray swfData = libSwf.readAll();
                libSwf.close();
                // Use shared decompression helper with size cap
                QByteArray decompressed = decompressCwsSwf(swfData);
                const QByteArray &src = decompressed.isEmpty() ? swfData : decompressed;
                QStringList bitmaps = extractSwfBitmaps(src, outPath);
                exported += bitmaps;
                if (!bitmaps.isEmpty())
                    info += QObject::tr("\n  %1 bitmap(s) extracted from library.swf")
                            .arg(bitmaps.size());
            }

            // Also include any other extracted files (scripts, assets)
            {
                QDirIterator it(outPath, kAssetFilters,
                                QDir::Files | QDir::NoDotAndDotDot,
                                QDirIterator::Subdirectories);
                QDir base(outPath);
                while (it.hasNext()) {
                    it.next();
                    QString rel = base.relativeFilePath(it.filePath());
                    if (!exported.contains(rel)) exported << rel;
                }
            }
        } else {
            // For XFL/FLA: parse document structure and report
            TFilePath extractedXfl = outDir;
            if (!XFL::isXFLDirectory(extractedXfl)) {
                try {
                    TFilePathSet entries = TSystem::readDirectory(outDir, false, false, true);
                    for (const auto &e : entries)
                        if (XFL::isXFLDirectory(e)) { extractedXfl = e; break; }
                } catch (...) {}
            }
            if (XFL::isXFLDirectory(extractedXfl)) {
                XFL::Reader r2(extractedXfl);
                if (r2.read()) {
                    const XFL::Document &doc = r2.getDocument();
                    int tlCount = static_cast<int>(doc.timelines.size());
                    int bmCount = static_cast<int>(doc.bitmaps.size());
                    info = QObject::tr(
                        "Document: %1 × %2 px  |  %3 fps  |  %4 symbol(s)  |  %5 bitmap(s)  |  %6 timeline(s)")
                        .arg(doc.width).arg(doc.height)
                        .arg(doc.frameRate, 0, 'f', 1)
                        .arg(static_cast<int>(doc.symbols.size()))
                        .arg(bmCount)
                        .arg(tlCount);
                    // Native scene import: map FLA layers → Flare columns
                    if (tlCount > 0) {
                        TXsheet *xsheet = TApp::instance()->getCurrentXsheet()->getXsheet();
                        importXFLScene(scene, xsheet, doc, extractedXfl);
                    }
                } else {
                    QString err = QString::fromStdString(r2.getError());
                    info = QObject::tr("Failed to parse XFL metadata: %1").arg(err);
                    DVGui::warning(info);
                }
            }
            // Extract binary media from FLA's bin/ directory (.dat → PNG/JPG)
            QStringList binMedia = extractFLABinaryMedia(outPath);
            exported += binMedia;
            if (!binMedia.isEmpty())
                info += QObject::tr("\n  %1 bitmap(s) extracted from FLA binary media")
                        .arg(binMedia.size());
            // Also look in subdirectories if the XFL was nested
            if (extractedXfl != outDir) {
                QStringList nestedMedia = extractFLABinaryMedia(extractedXfl.getQString());
                for (const QString &m : nestedMedia) {
                    if (!exported.contains(m)) {
                        // Copy to output root for auto-load
                        QString src2 = extractedXfl.getQString() + "/" + m;
                        QString dst2 = outPath + "/" + m;
                        if (!QFile::exists(dst2)) QFile::copy(src2, dst2);
                        exported << m;
                    }
                }
            }
            {
                QDirIterator it(outPath, kAssetFilters,
                                QDir::Files | QDir::NoDotAndDotDot,
                                QDirIterator::Subdirectories);
                QDir base(outPath);
                while (it.hasNext()) { it.next(); exported << base.relativeFilePath(it.filePath()); }
            }
        }

    // ---- XFL directory ----
    } else if ((ext == "xfl" && QFileInfo(srcPath).isDir()) || QFileInfo(srcPath).isDir()) {
        if (!QFileInfo(srcPath).exists()) {
            DVGui::error(QObject::tr("XFL directory does not exist: %1").arg(srcPath));
            return;
        }
        XFL::Reader reader(fp);
        if (!reader.read()) {
            DVGui::error(QObject::tr("Failed to read XFL: %1").arg(reader.getError().c_str()));
            return;
        }
        const XFL::Document &doc = reader.getDocument();
        info = QObject::tr(
            "Document: %1 × %2 px  |  %3 fps  |  %4 symbol(s)  |  %5 bitmap(s)  |  %6 timeline(s)")
            .arg(doc.width).arg(doc.height)
            .arg(doc.frameRate, 0, 'f', 1)
            .arg(static_cast<int>(doc.symbols.size()))
            .arg(static_cast<int>(doc.bitmaps.size()))
            .arg(static_cast<int>(doc.timelines.size()));

        // Native scene import for XFL directory
        if (!doc.timelines.empty()) {
            TXsheet *xsheet = TApp::instance()->getCurrentXsheet()->getXsheet();
            importXFLScene(scene, xsheet, doc, fp);
        }

        // Copy assets to output dir using recursive iteration
        {
            QDirIterator it(srcPath, kAssetFilters,
                            QDir::Files | QDir::NoDotAndDotDot,
                            QDirIterator::Subdirectories);
            QDir base(srcPath);
            while (it.hasNext()) {
                it.next();
                QString rel = base.relativeFilePath(it.filePath());
                QString dst = outPath + "/" + rel;
                QDir().mkpath(QFileInfo(dst).absolutePath());
                if (!QFile::copy(it.filePath(), dst)) continue;
                exported << rel;
            }
        }

    // ---- SWF binary: read header + extract embedded bitmaps ----
    } else if (ext == "swf") {
        SwfInfo swf = readSwfHeader(srcPath);
        if (!swf.valid) {
            DVGui::error(QObject::tr("Not a valid SWF file: %1").arg(srcPath));
            return;
        }
        info = QObject::tr(
            "SWF v%1  |  %2 × %3 px  |  %4 fps  |  %5 frame(s)%6")
            .arg(swf.version)
            .arg(swf.width).arg(swf.height)
            .arg(swf.frameRate)
            .arg(swf.frameCount)
            .arg(swf.compressed ? QObject::tr("  [compressed]") : QString());

        // Read entire SWF and extract embedded bitmaps via tag scan.
        // For zlib-compressed SWF (CWS, version 6+), decompress body first.
        // Uses shared decompressCwsSwf() helper with size cap
        // (approach consistent with lightspark and open-flash/swf-bitmap).
        QFile swfFile(srcPath);
        if (swfFile.open(QIODevice::ReadOnly)) {
            QByteArray swfData = swfFile.readAll();
            swfFile.close();
            QByteArray decompressed = decompressCwsSwf(swfData);
            const QByteArray &src2 = decompressed.isEmpty() ? swfData : decompressed;
            QStringList bitmaps = extractSwfBitmaps(src2, outPath);
            exported += bitmaps;
            if (!bitmaps.isEmpty())
                info += QObject::tr("\n  %1 embedded bitmap(s) extracted").arg(bitmaps.size());
        }

        // Always copy the SWF itself to output for reference
        QString dstSwf = outPath + "/" + QFileInfo(srcPath).fileName();
        if (QFile::copy(srcPath, dstSwf)) {
            QString fileName = QFileInfo(srcPath).fileName();
            if (!exported.contains(fileName))
                exported << fileName;
        } else {
            // If we can't copy SWF, continue with the rest of import rather than failing.
            qDebug() << "Warning: failed to copy SWF to" << dstSwf;
        }

    // ---- ActionScript source (.as) ----
    } else if (ext == "as") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("ActionScript source copied for reference.");

    // ---- ActionScript command script (.asc) ----
    } else if (ext == "asc") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("ActionScript command script (.asc) copied for reference.");

    // ---- MXML (Apache Flex / Royale UI definition) ----
    } else if (ext == "mxml") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("MXML (Flex UI) file copied for reference.");

    // ---- FLV (Flash Video) ----
    } else if (ext == "flv") {
        FlvInfo flv = readFlvHeader(srcPath);
        if (!flv.valid) {
            DVGui::error(QObject::tr("Not a valid FLV file: %1").arg(srcPath));
            return;
        }
        info = QObject::tr("FLV v%1  |  %2%3")
            .arg(flv.version)
            .arg(flv.hasVideo ? QObject::tr("video") : QString())
            .arg(flv.hasAudio ? QObject::tr(flv.hasVideo ? " + audio" : "audio") : QString());
        copyFileForReference(srcPath, outPath, exported);

    // ---- F4V (Flash H.264, ISO BMFF container) ----
    } else if (ext == "f4v") {
        F4vInfo f4v = readF4vHeader(srcPath);
        if (!f4v.valid) {
            DVGui::error(QObject::tr("Not a valid F4V/ISOBMFF file: %1").arg(srcPath));
            return;
        }
        info = QObject::tr("F4V  |  brand: %1").arg(f4v.majorBrand);
        if (!f4v.compatBrands.isEmpty())
            info += QObject::tr("  |  compatible: %1").arg(f4v.compatBrands);
        copyFileForReference(srcPath, outPath, exported);

    // ---- ANE (Adobe Native Extension) — ZIP ----
    } else if (ext == "ane") {
        QString manifest = extractAdobeZipPackage(srcPath, outPath, "ane");
        info = manifest.isEmpty()
            ? QObject::tr("ANE: extracted (no extension.xml found)")
            : QObject::tr("ANE: extension manifest at %1").arg(manifest);
        QDirIterator it(outPath, kAssetFilters, QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) { it.next(); exported << base.relativeFilePath(it.filePath()); }

    // ---- AIR (Adobe AIR application package) — ZIP ----
    } else if (ext == "air") {
        QString manifest = extractAdobeZipPackage(srcPath, outPath, "air");
        info = manifest.isEmpty()
            ? QObject::tr("AIR: extracted (no application.xml found)")
            : QObject::tr("AIR: application manifest at %1").arg(manifest);
        QDirIterator it(outPath, kAssetFilters, QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) { it.next(); exported << base.relativeFilePath(it.filePath()); }

    // ---- OAM (Open Architecture Module) — ZIP ----
    } else if (ext == "oam") {
        QString manifest = extractAdobeZipPackage(srcPath, outPath, "oam");
        info = manifest.isEmpty()
            ? QObject::tr("OAM: extracted (no OAMMetadata.xml found)")
            : QObject::tr("OAM: metadata at %1").arg(manifest);
        QDirIterator it(outPath, kAssetFilters, QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) { it.next(); exported << base.relativeFilePath(it.filePath()); }

    // ---- LWF (Lightweight SWF alternative) / RSL (Runtime Shared Library) /
    //      AFL (ActionScript Library, legacy) — copy for reference ----
    } else if (ext == "lwf" || ext == "rsl" || ext == "afl") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("%1 file copied for reference.")
                   .arg(ext.toUpper());

    } else {
        DVGui::warning(QObject::tr("Unsupported Flash format: .%1").arg(ext));
        return;
    }

    writeManifest(outPath, exported, srcPath);

    // SWF/FLV/F4V are never directly loadable as Flare levels — Flare has no
    // native level reader for these binary Flash formats.  Only the extracted
    // image assets (PNG, JPG, SVG) can be auto-loaded into the scene.
    // Future support roadmap:
    //  - SWF vector DefineShape rendering
    //  - timeline/tween reconstruction from FLA/XFL
    //  - ActionScript execution (insecure sandboxed runtime)
    //  - SWF sprite/movieclip playback timeline
    //  - sound extraction from SWF/SWC
    if (ext == "swf")
        info += QObject::tr("\n  Embedded bitmaps extracted from SWF for import.");
    else if (ext == "flv")
        info += QObject::tr("\n  FLV copied for reference (no native FLV level reader).");
    else if (ext == "f4v")
        info += QObject::tr("\n  F4V copied for reference (no native F4V level reader).");

    if (ext == "fla" || ext == "xfl") {
      info += QObject::tr("\n  Note: FLA/XFL import currently extracts bitmap media; advanced timeline/vector/actionscript support is experimental.");
    }

    // Auto-load only image assets that Flare can natively handle as levels.
    int imported = 0;
    {
        IoCmd::LoadResourceArguments args;
        for (const QString &rel : exported) {
            QString full = outPath + "/" + rel;
            QString e    = QFileInfo(full).suffix().toLower();
            // Only load image formats that Flare supports as levels
            if (e == "png" || e == "jpg" || e == "jpeg" || e == "svg") {
                args.resourceDatas.push_back(
                    IoCmd::LoadResourceArguments::ResourceData(TFilePath(full.toStdWString())));
            }
        }
        if (!args.resourceDatas.empty()) {
            imported = IoCmd::loadResources(args);
        }
    }

    QString msg = QObject::tr("Flash import complete.\n");
    if (!info.isEmpty()) msg += info + "\n";
    msg += QObject::tr("\n%1 file(s) exported to:\n%2").arg(exported.size()).arg(outPath);
    if (imported > 0)
        msg += QObject::tr("\n%1 asset(s) added to the scene.").arg(imported);

    std::vector<QString> btns = {QObject::tr("Open folder"), QObject::tr("Save as FLA"), QObject::tr("OK")};
    int ret = DVGui::MsgBox(DVGui::INFORMATION, msg, btns);
    if (ret == 1) {
      openFolder(outPath);
    } else if (ret == 2 && (ext == "fla" || ext == "xfl" || ext == "swc")) {
      QString savePath = QFileDialog::getSaveFileName(nullptr,
          QObject::tr("Save as FLA"), outDir.getQString(),
          QObject::tr("Adobe FLA files (*.fla)"));
      if (!savePath.isEmpty()) {
        TFilePath sourceDir;
        if (ext == "xfl" && QFileInfo(srcPath).isDir())
          sourceDir = fp;
        else
          sourceDir = outDir;

        if (XFL::writeFLA(sourceDir, TFilePath(savePath.toStdWString()))) {
          DVGui::info(QObject::tr("Saved FLA successfully: %1").arg(savePath));
        } else {
          DVGui::error(QObject::tr("Failed to save FLA: %1").arg(savePath));
        }
      }
    }
}
