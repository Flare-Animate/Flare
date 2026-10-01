// ZipArchive.cpp - ZIP container handling for the Flash importers
// Copyright (c) 2026 Flare Project
//
// See ZipArchive.h for why this exists.  The salvage strategy is deliberately
// conservative: we only ever rewrite the 22-byte end-of-central-directory
// record, and we only accept a recovered central directory after cross-checking
// every record against the local file headers it points back to.

#include "ZipArchive.h"

#include "tsystem.h"

#include <QFile>
#include <QByteArray>
#include <QDir>
#include <QFileInfo>
#include <QDateTime>
#include <QString>
#include <QDebug>

#include "../../../../thirdparty/zlib-1.2.8/contrib/minizip/unzip.h"

#include <algorithm>
#include <cstdio>
#include <cstring>

namespace FlareZip {

namespace {

const quint32 kEocdSignature    = 0x06054b50;  // "PK\x05\x06"
const quint32 kCdSignature      = 0x02014b50;  // "PK\x01\x02"
const quint32 kLfhSignature     = 0x04034b50;  // "PK\x03\x04"
const quint32 kZip64EocdLocSig  = 0x07064b50;  // "PK\x06\x07"

const int kEocdSize         = 22;
const int kCdRecordSize     = 46;
const int kMaxCommentLen    = 0xFFFF;
const int kTailProbeSize    = kMaxCommentLen + kEocdSize;   // 65557

// Little-endian scalar reads. The QByteArray helpers take a char*, and we are
// handling possibly unaligned offsets, so go through quint8 explicitly.
inline quint16 rd16(const QByteArray &b, int o) {
    return static_cast<quint16>(static_cast<quint8>(b[o])) |
           static_cast<quint16>(static_cast<quint8>(b[o + 1])) << 8;
}
inline quint32 rd32(const QByteArray &b, int o) {
    return static_cast<quint32>(static_cast<quint8>(b[o])) |
           (static_cast<quint32>(static_cast<quint8>(b[o + 1])) << 8) |
           (static_cast<quint32>(static_cast<quint8>(b[o + 2])) << 16) |
           (static_cast<quint32>(static_cast<quint8>(b[o + 3])) << 24);
}
inline quint32 sigAt(const QByteArray &b, int o) {
    return static_cast<quint32>(static_cast<quint8>(b[o])) |
           (static_cast<quint32>(static_cast<quint8>(b[o + 1])) << 8) |
           (static_cast<quint32>(static_cast<quint8>(b[o + 2])) << 16) |
           (static_cast<quint32>(static_cast<quint8>(b[o + 3])) << 24);
}

void append16(QByteArray &out, quint16 v) {
    out.append(static_cast<char>(v & 0xFF));
    out.append(static_cast<char>((v >> 8) & 0xFF));
}
void append32(QByteArray &out, quint32 v) {
    out.append(static_cast<char>(v & 0xFF));
    out.append(static_cast<char>((v >> 8) & 0xFF));
    out.append(static_cast<char>((v >> 16) & 0xFF));
    out.append(static_cast<char>((v >> 24) & 0xFF));
}

// Read `n` bytes at absolute `offset`. Returns an empty array on any failure.
QByteArray readAt(QFile &f, qint64 offset, qint64 n) {
    if (offset < 0 || n <= 0) return QByteArray();
    if (!f.seek(offset)) return QByteArray();
    return f.read(n);
}

// One central directory record's decoded, validated form.
struct CdRecord {
    quint32 localHeaderOffset = 0;
    int     nameLen = 0;
    int     nextOffset = 0;  // absolute offset of the following record
};

// Decode the central directory record at `offset`. `limit` bounds the read so a
// corrupt length field can never walk us off the end of the file. Returns false
// when the record is not usable.
bool decodeCdRecord(const QByteArray &head, int recordStart, qint64 fileSize,
                    CdRecord &out) {
    if (head.size() < kCdRecordSize) return false;
    if (sigAt(head, 0) != kCdSignature) return false;

    const quint16 nameLen  = rd16(head, 28);
    const quint16 extraLen = rd16(head, 30);
    const quint16 cmtLen   = rd16(head, 32);
    out.localHeaderOffset  = rd32(head, 42);

    const qint64 next = static_cast<qint64>(recordStart) + kCdRecordSize +
                        nameLen + extraLen + cmtLen;
    if (next > fileSize) return false;

    out.nameLen = nameLen;
    out.nextOffset = static_cast<int>(next);
    return true;
}

// Walk `count` central directory records starting at absolute `start`.
// Returns the offset just past the last record, or -1 if the walk breaks.
int walkCentralDirectory(QFile &f, qint64 start, int count, qint64 fileSize,
                         qint64 endLimit) {
    qint64 off = start;
    for (int i = 0; i < count; ++i) {
        if (off < 0 || off + kCdRecordSize > fileSize) return -1;
        QByteArray head = readAt(f, off, kCdRecordSize);
        CdRecord rec;
        if (!decodeCdRecord(head, static_cast<int>(off), fileSize, rec)) return -1;
        if (rec.nextOffset > endLimit) return -1;
        off = rec.nextOffset;
    }
    return (off <= fileSize) ? static_cast<int>(off) : -1;
}

// Walk central directory records from `start` until we run out of valid
// records. Returns the count and the end offset; stops when the next offset
// does not carry a central directory signature.
int countCentralDirectory(QFile &f, qint64 start, qint64 fileSize,
                          qint64 &endOut) {
    qint64 off = start;
    int count = 0;
    // Hard ceiling so a pathological file cannot spin here; the number of
    // entries is a 16-bit field in practice, so this is generous.
    while (count < 0x10000) {
        if (off < 0 || off + kCdRecordSize > fileSize) break;
        QByteArray head = readAt(f, off, kCdRecordSize);
        if (sigAt(head, 0) != kCdSignature) break;
        CdRecord rec;
        if (!decodeCdRecord(head, static_cast<int>(off), fileSize, rec)) break;
        off = rec.nextOffset;
        ++count;
    }
    endOut = off;
    return count;
}

// Verify that every record in [start, end) points back at a real local file
// header. This is what separates a genuine central directory from a coincidental
// "PK\x01\x02" byte sequence inside compressed data.
bool backPointersResolve(QFile &f, qint64 start, qint64 end, qint64 fileSize) {
    qint64 off = start;
    while (off < end) {
        QByteArray head = readAt(f, off, kCdRecordSize);
        CdRecord rec;
        if (!decodeCdRecord(head, static_cast<int>(off), fileSize, rec)) return false;
        if (static_cast<qint64>(rec.localHeaderOffset) >= start) return false;
        QByteArray lfh = readAt(f, rec.localHeaderOffset, 4);
        if (lfh.size() < 4 || sigAt(lfh, 0) != kLfhSignature) return false;
        off = rec.nextOffset;
    }
    return off == end;
}

}  // namespace

// ---------------------------------------------------------------------------

Status normalize(const TFilePath &zipPath, const TFilePath &outPath,
                 std::string &detail) {
    detail.clear();

    QFile f(zipPath.getQString());
    if (!f.open(QIODevice::ReadOnly)) {
        detail = "cannot open " + zipPath.getQString().toStdString();
        return Status::NotAZip;
    }
    const qint64 fileSize = f.size();
    if (fileSize < kEocdSize) {
        detail = "file too small to be a ZIP";
        return Status::NotAZip;
    }

    // ---- locate the end-of-central-directory record -------------------------
    // It is the last record in the file; its comment length is what tells us
    // where it starts, so we scan backwards for a signature that makes the
    // record terminate exactly at EOF.  (A bare rfind() can latch onto a
    // "PK\x05\x06" that lives inside the payload.)
    const qint64 tailLen = std::min(fileSize, static_cast<qint64>(kTailProbeSize));
    const QByteArray tail = readAt(f, fileSize - tailLen, tailLen);
    if (tail.size() != tailLen) {
        detail = "short read while probing the ZIP trailer";
        return Status::NotAZip;
    }

    int eocdRel = -1;
    for (int i = tailLen - kEocdSize; i >= 0; --i) {
        if (sigAt(tail, i) != kEocdSignature) continue;
        const int commentLen = rd16(tail, i + 20);
        if (i + kEocdSize + commentLen == tailLen) { eocdRel = i; break; }
    }
    if (eocdRel < 0) {
        detail = "no end-of-central-directory record";
        return Status::NotAZip;
    }
    const qint64 eocdOffset = fileSize - tailLen + eocdRel;

    // ZIP64 archives keep the authoritative 64-bit values in a separate record.
    // minizip already understands those, and rewriting a classic trailer would
    // not fix them, so stay out of the way.
    {
        // Look for the ZIP64 EOCD locator, which always precedes the EOCD.
        const qint64 probeStart = std::max<qint64>(0, eocdOffset - 1024);
        const QByteArray before = readAt(f, probeStart, eocdOffset - probeStart);
        for (int i = before.size() - 4; i >= 0; --i) {
            if (sigAt(before, i) == kZip64EocdLocSig) {
                detail = "ZIP64 archive left untouched";
                return Status::Zip64;
            }
        }
    }

    const quint16 entriesOnDisk = rd16(tail, eocdRel + 8);
    const quint16 entriesTotal  = rd16(tail, eocdRel + 10);
    const quint32 cdSize        = rd32(tail, eocdRel + 12);
    const quint32 cdOffset      = rd32(tail, eocdRel + 16);
    const int     commentLen    = rd16(tail, eocdRel + 20);
    const QByteArray comment(tail.constData() + eocdRel + kEocdSize, commentLen);

    const bool singleDisk =
        (rd16(tail, eocdRel + 4) == 0) && (rd16(tail, eocdRel + 6) == 0);

    // ---- fast path: already consistent --------------------------------------
    if (singleDisk && entriesOnDisk == entriesTotal && entriesTotal > 0 &&
        static_cast<qint64>(cdOffset) + cdSize <= eocdOffset &&
        walkCentralDirectory(f, cdOffset, entriesTotal, fileSize,
                             eocdOffset) == static_cast<qint64>(cdOffset) + cdSize) {
        detail = "end-of-central-directory record is consistent";
        return Status::Ok;
    }

    // ---- recovery: find the central directory that ends at the EOCD ---------
    // The central directory always terminates immediately before the EOCD, so
    // scanning a window that grows backwards from the EOCD always finds it.
    qint64 cdStart = -1;
    int    cdCount = 0;

    const quint32 declaredCount = entriesTotal ? entriesTotal : entriesOnDisk;
    // Grow the scan window backwards from the EOCD. Termination is decided by
    // the window reaching the start of the file, not by the window size: an
    // archive smaller than the first window (a small FLA, a SWC) must still be
    // scanned, so the size of the window must never gate the loop.
    for (qint64 window = 64 * 1024;; window *= 8) {
        const qint64 winStart = std::max<qint64>(0, eocdOffset - window);
        const QByteArray buf = readAt(f, winStart, eocdOffset - winStart);
        if (buf.isEmpty()) break;

        for (int i = 0; i + kCdRecordSize <= buf.size(); ++i) {
            if (sigAt(buf, i) != kCdSignature) continue;
            const qint64 candidate = winStart + i;

            // (a) Trust the declared entry count when it walks cleanly.
            if (declaredCount > 0) {
                const int end = walkCentralDirectory(f, candidate,
                                                     declaredCount, fileSize,
                                                     eocdOffset);
                if (end == eocdOffset &&
                    backPointersResolve(f, candidate, eocdOffset, fileSize)) {
                    cdStart = candidate;
                    cdCount = declaredCount;
                    break;
                }
            }
            // (b) The count itself may be wrong: walk until the records stop.
            if (cdStart < 0) {
                qint64 end = 0;
                const int n = countCentralDirectory(f, candidate, fileSize, end);
                if (n > 0 && end == eocdOffset &&
                    backPointersResolve(f, candidate, eocdOffset, fileSize)) {
                    cdStart = candidate;
                    cdCount = n;
                    break;
                }
            }
        }
        if (cdStart >= 0) break;
        if (winStart == 0) break;
    }

    if (cdStart < 0 || cdCount <= 0) {
        detail = "central directory could not be recovered";
        return Status::Unrepairable;
    }

    // ---- emit the corrected archive ----------------------------------------
    // Payload bytes are copied verbatim; only the trailer is rewritten.
    const quint32 newCdSize =
        static_cast<quint32>(eocdOffset - cdStart);
    const quint32 newCdOffset = static_cast<quint32>(cdStart);

    QFile out(outPath.getQString());
    if (!out.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        detail = "cannot write " + outPath.getQString().toStdString();
        return Status::Unrepairable;
    }

    f.seek(0);
    qint64 remaining = eocdOffset;
    QByteArray chunk(256 * 1024, Qt::Uninitialized);
    while (remaining > 0) {
        const qint64 want = std::min<qint64>(remaining, chunk.size());
        const qint64 got = f.read(chunk.data(), want);
        if (got <= 0) break;
        out.write(chunk.constData(), got);
        remaining -= got;
    }
    if (remaining != 0) {
        out.close();
        out.remove();
        detail = "short copy while rewriting the ZIP trailer";
        return Status::Unrepairable;
    }

    QByteArray trailer;
    append32(trailer, kEocdSignature);
    append16(trailer, 0);                            // number of this disk
    append16(trailer, 0);                            // disk with the CD
    append16(trailer, static_cast<quint16>(cdCount));
    append16(trailer, static_cast<quint16>(cdCount));
    append32(trailer, newCdSize);
    append32(trailer, newCdOffset);
    append16(trailer, static_cast<quint16>(commentLen));
    out.write(trailer);
    out.write(comment);
    out.close();

    detail = "repaired ZIP trailer: central directory " +
             std::to_string(cdOffset) + "+" + std::to_string(cdSize) +
             " -> " + std::to_string(newCdOffset) + "+" +
             std::to_string(newCdSize) + " (" + std::to_string(cdCount) +
             " entries)";
    return Status::Repaired;
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

namespace {

// Confirm a resolved member path really lands under `baseDir`.
// The textual checks in memberNameIsSafe() are the primary defence; this is the
// belt-and-braces check that survives symlinked or oddly-normalised names.
bool isPathUnderDir(const QString &baseDir, const QString &candidate) {
    const QDir dir(baseDir);
    QString prefix = dir.canonicalPath();
    if (prefix.isEmpty()) prefix = dir.absolutePath();
    if (prefix.isEmpty()) return false;
    if (!prefix.endsWith('/')) prefix += '/';

    // canonicalFilePath() is empty for a path that does not exist yet, and
    // absoluteFilePath() does not follow symlinks -- so a member written into a
    // symlinked subdirectory would pass a purely textual comparison. Walk up to
    // the deepest ancestor that does exist, canonicalise that, then re-append
    // the part that is still to be created.
    QFileInfo info(QDir::cleanPath(QFileInfo(candidate).absoluteFilePath()));
    QStringList pending;
    while (!info.exists()) {
        const QString parent = info.path();
        if (parent.isEmpty() || parent == info.filePath()) return false;
        pending.prepend(info.fileName());
        info.setFile(parent);
    }
    QString resolved = info.canonicalFilePath();
    if (resolved.isEmpty()) return false;
    if (!pending.isEmpty()) {
        resolved += QLatin1Char('/');
        resolved += pending.join(QLatin1Char('/'));
    }
    return resolved.startsWith(prefix);
}

// A scratch file that removes itself when it goes out of scope, so the
// trailer-repair step never leaks a temp file — even on the error paths.
class ScratchFile {
public:
    explicit ScratchFile(const QString &path) : m_path(path) {}
    ~ScratchFile() { QFile::remove(m_path); }
    ScratchFile(const ScratchFile &) = delete;
    ScratchFile &operator=(const ScratchFile &) = delete;
    const QString &path() const { return m_path; }

private:
    QString m_path;
};

// Reject member names that would escape `outDir`.
// Deliberately strict: a well-formed FLA/XFL/SWC never contains an absolute
// member or a ".." component, so encountering one means the archive is
// malformed or hostile, and skipping the entry is the safe response.
bool memberNameIsSafe(const QString &raw) {
    QString s = raw;
    s.replace('\\', '/');
    while (s.startsWith("./")) s = s.mid(2);
    if (s.isEmpty()) return false;
    if (s.startsWith('/') || s.startsWith('\\')) return false;
    if (s.length() >= 2 && s[1] == ':') return false;          // "C:\..."
    if (s == "..") return false;
    if (s.contains("../") || s.contains("..\\")) return false;
    return true;
}

}  // namespace

bool extract(const TFilePath &zipPath, const TFilePath &outDir,
             std::string &detail) {
    detail.clear();
    if (!TSystem::doesExistFileOrLevel(zipPath)) {
        detail = "archive does not exist";
        return false;
    }
    if (!TSystem::doesExistFileOrLevel(outDir)) TSystem::mkDir(outDir);

    // Salvage the trailer when it disagrees with the file. Well-formed
    // archives cost one 64 KiB tail read and produce no scratch file.
    const QString scratchPath =
        QDir(TSystem::getTempDir().getQString())
            .filePath("flare_ziprepair_" +
                      QString::number(QDateTime::currentMSecsSinceEpoch()) +
                      ".zip");
    ScratchFile scratch(scratchPath);

    const Status status = normalize(zipPath, TFilePath(scratchPath), detail);
    QString usable = zipPath.getQString();
    if (status == Status::Repaired) {
        usable = scratchPath;
        qDebug() << "[ZipArchive] repaired archive trailer:"
                 << zipPath.getQString() << QString::fromStdString(detail);
    } else if (status == Status::Unrepairable || status == Status::NotAZip) {
        return false;
    }

    unzFile uf = unzOpen64(usable.toUtf8().constData());
    if (!uf) {
        if (detail.empty()) detail = "archive could not be opened";
        return false;
    }

    const QString outDirPath = QDir(outDir.getQString()).absolutePath();
    QByteArray name(4096, Qt::Uninitialized);
    QByteArray buf(64 * 1024, Qt::Uninitialized);
    int written  = 0;
    int skipped  = 0;   // rejected by the traversal checks
    int failed   = 0;   // present in the archive but unreadable
    int dirCount = 0;

    // Drive iteration from the archive itself rather than from the entry count
    // in the trailer. That count is the field most likely to be wrong (see the
    // file header), and a truncated count would silently stop extraction short.
    for (int next = unzGoToFirstFile(uf); next == UNZ_OK;
         next = unzGoToNextFile(uf)) {
        unz_file_info64 fi;
        if (unzGetCurrentFileInfo64(uf, &fi, name.data(), name.size(), nullptr, 0,
                                    nullptr, 0) != UNZ_OK) {
            ++failed;
            continue;
        }
        name[name.size() - 1] = '\0';  // guarantee termination
        const QString rawName =
            QString::fromUtf8(name.constData(),
                              static_cast<int>(qstrlen(name.constData())));
        if (rawName.isEmpty()) continue;

        QString rel = rawName;
        rel.replace('\\', '/');
        while (rel.startsWith("./")) rel = rel.mid(2);
        const bool isDir = rel.endsWith('/');

        if (!isDir && !memberNameIsSafe(rel)) {
            ++skipped;
            continue;
        }

        const QString fullOut = outDirPath + "/" + rel;

        // Second line of defence: confirm the resolved path really is inside
        // outDir even if the name slipped past the textual checks.
        if (!isPathUnderDir(outDirPath, fullOut)) {
            ++skipped;
            continue;
        }

        if (isDir) {
            QDir().mkpath(fullOut);
            ++dirCount;
            continue;
        }

        QDir().mkpath(QFileInfo(fullOut).absolutePath());
        if (unzOpenCurrentFile(uf) != UNZ_OK) {
            ++failed;
            continue;
        }

        bool writeOk = true;
        QFile outFile(fullOut);
        if (!outFile.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            unzCloseCurrentFile(uf);
            ++failed;
            continue;
        }
        int n = 0;
        while ((n = unzReadCurrentFile(uf, buf.data(),
                                       static_cast<unsigned>(buf.size()))) > 0) {
            if (outFile.write(buf.constData(), n) != n) { writeOk = false; break; }
        }
        // A negative return is an inflate error, not end-of-entry: whatever we
        // wrote is incomplete.
        if (n < 0) writeOk = false;
        outFile.close();

        // minizip validates the entry's CRC in unzCloseCurrentFile and reports
        // a mismatch as UNZ_CRCERROR. A read can reach EOF cleanly and still
        // fail here, so a silently corrupt entry only shows up in this return
        // value - do not count it as extracted.
        if (unzCloseCurrentFile(uf) != UNZ_OK) writeOk = false;

        if (writeOk) {
            ++written;
        } else {
            outFile.remove();
            ++failed;
        }
    }
    unzClose(uf);

    if (skipped > 0)
        qDebug() << "[ZipArchive] skipped" << skipped
                 << "unsafe member(s) in" << zipPath.getQString();
    if (failed > 0)
        qDebug() << "[ZipArchive]" << failed << "member(s) of"
                 << zipPath.getQString() << "could not be read and were skipped";

    if (written == 0 && failed > 0 && dirCount == 0) return false;
    // An archive of nothing but directory entries is still a successful read.
    return written > 0 || (dirCount > 0 && failed == 0);
}

}  // namespace FlareZip
