#include "archive.h"

#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QVector>
#include <algorithm>
#include <limits>
#include <zlib.h>

namespace Flash {
namespace {

constexpr quint64 initialReserveBytes = 4ull * 1024 * 1024;

quint16 u16(const QByteArray &bytes, int pos) {
  return quint8(bytes[pos]) | (quint16(quint8(bytes[pos + 1])) << 8);
}

quint32 u32(const QByteArray &bytes, int pos) {
  return u16(bytes, pos) | (quint32(u16(bytes, pos + 2)) << 16);
}

bool safeName(const QString &name) {
  if (name.isEmpty() || name.contains(QChar(0)) || name.contains('\\') ||
      name.contains(':') || name.startsWith('/'))
    return false;
  const QStringList parts = name.split('/');
  for (int i = 0; i < parts.size(); ++i)
    if (parts[i] == "." || parts[i] == ".." ||
        (parts[i].isEmpty() && i != parts.size() - 1))
      return false;
  return true;
}

QByteArray exact(QFile &file, qint64 size) {
  QByteArray bytes = file.read(size);
  if (bytes.size() != size) throw Error("Truncated archive record.");
  return bytes;
}

void seek(QFile &file, quint64 offset) {
  if (offset > quint64(file.size()) || !file.seek(qint64(offset)))
    throw Error("Archive offset is outside the file.");
}

QString decodeName(const QByteArray &bytes, quint16 flags) {
  QString name;
  if (flags & 0x800) {
    name = QString::fromUtf8(bytes);
    if (name.toUtf8() != bytes) throw Error("Invalid UTF-8 archive filename.");
  } else {
    // CP437's non-ASCII characters require an explicit codec rather than
    // guessing a platform code page. Adobe XFL filenames use UTF-8.
    for (char c : bytes)
      if (quint8(c) >= 128)
        throw Error("Non-UTF-8 archive filenames are not supported.");
    name = QString::fromLatin1(bytes);
  }
  // Adobe's Windows writers use backslashes in some media member names.
  // Normalize before checking traversal and detecting name collisions.
  name.replace('\\', '/');
  if (!safeName(name)) throw Error("Unsafe archive filename: " + name);
  return name;
}

bool looksLikeXflProject(const QString &directory) {
  const QDir dir(directory);
  // LIBRARY/ holds symbols and bin/ holds embedded media; either one marks
  // a project folder whose members the document may reference.
  for (const char *folder : {"LIBRARY", "bin"}) {
    const QFileInfo member(dir.filePath(folder));
    if (member.isDir() && !member.isSymLink()) return true;
  }
  // Name filters are case-insensitive unless QDir::CaseSensitive is given.
  return !dir.entryList({"*.xfl"}, QDir::Files | QDir::NoDotAndDotDot)
              .isEmpty();
}

}  // namespace

Package::Package(const QString &path, Limits limits) : m_limits(limits) {
  checkCancelled();
  const QFileInfo info(path);
  if (!info.exists()) throw Error("Document does not exist: " + path);
  if (!info.isDir()) {
    if (!info.isFile()) throw Error("Document is not a regular file: " + path);
    m_file.reset(new QFile(path));
    if (!m_file->open(QIODevice::ReadOnly))
      throw Error("Cannot open document: " + m_file->errorString());
    // Historical interchange files can be ZIP packages with an .xfl suffix.
    // Only the small uncompressed-project marker resolves to a sibling folder;
    // a malformed archive must never fall back to an unrelated DOMDocument.xml.
    const qint64 fileSize = m_file->size();
    if (fileSize < 0) throw Error("Cannot determine document size: " + path);
    const QByteArray header = exact(*m_file, std::min<qint64>(fileSize, 64));
    checkCancelled();
    const bool archiveSignature =
        header.startsWith(QByteArray::fromHex("504b0304")) ||
        header.startsWith(QByteArray::fromHex("504b0506")) ||
        header.startsWith(QByteArray::fromHex("504b0708")) ||
        header.startsWith(QByteArray::fromHex("d0cf11e0a1b11ae1"));
    const bool marker =
        info.suffix().compare("xfl", Qt::CaseInsensitive) == 0 &&
        fileSize <= 64 && header.trimmed() == "PROXY-CS5";
    if (archiveSignature || (!marker && info.fileName() != "DOMDocument.xml")) {
      openArchive();
      return;
    }
    m_file.reset();
  }
  m_directory =
      info.isDir() ? info.canonicalFilePath() : info.dir().canonicalPath();
  if (!QFileInfo::exists(m_directory + "/DOMDocument.xml"))
    throw Error("The XFL project is missing DOMDocument.xml.");
  // A folder or .xfl marker names a project explicitly. A bare DOMDocument.xml
  // only implies one when its folder has the XFL layout; otherwise do not walk
  // an arbitrary parent tree (e.g. a downloads folder) and expose only the
  // document itself.
  if (!info.isDir() && info.suffix().compare("xfl", Qt::CaseInsensitive) != 0 &&
      !looksLikeXflProject(m_directory)) {
    const QFileInfo member(m_directory + "/DOMDocument.xml");
    if (member.isSymLink() || !member.isFile())
      throw Error("Unsafe XFL project member: DOMDocument.xml");
    Entry entry;
    entry.size = quint64(member.size());
    m_entries.insert("DOMDocument.xml", entry);
    return;
  }
  QDirIterator it(m_directory, QDir::Files | QDir::NoDotAndDotDot,
                  QDirIterator::Subdirectories);
  while (it.hasNext()) {
    checkCancelled();
    const QFileInfo member(it.next());
    const QString name = QDir(m_directory).relativeFilePath(member.filePath());
    if (!safeName(name) || member.isSymLink())
      throw Error("Unsafe XFL project member: " + name);
    Entry entry;
    entry.size = quint64(member.size());
    m_entries.insert(name, entry);
    if (quint32(m_entries.size()) > m_limits.maxEntries)
      throw Error("The project has too many files.");
  }
}

Package::~Package() = default;

void Package::checkCancelled() const {
  if (m_limits.cancelled && m_limits.cancelled())
    throw Error("Document reading cancelled.");
}

void Package::openArchive() {
  checkCancelled();
  if (m_file->size() < 22) throw Error("The FLA archive is too short.");
  const qint64 tailSize = std::min<qint64>(m_file->size(), 65557);
  seek(*m_file, quint64(m_file->size() - tailSize));
  const QByteArray tail = exact(*m_file, tailSize);
  int end               = -1;
  for (int pos = tail.size() - 22; pos >= 0; --pos) {
    if (u32(tail, pos) == 0x06054b50 &&
        pos + 22 + u16(tail, pos + 20) == tail.size()) {
      end = pos;
      break;
    }
  }
  if (end < 0) {
    seek(*m_file, 0);
    const QByteArray magic = exact(*m_file, 8);
    if (magic == QByteArray::fromHex("d0cf11e0a1b11ae1"))
      throw Error("Legacy binary FLA is not supported by this reader.");
    throw Error("Missing ZIP end record; the document may be truncated.");
  }
  const quint64 endOffset    = quint64(m_file->size() - tailSize + end);
  const quint32 count        = u16(tail, end + 10);
  const quint32 declaredSize = u32(tail, end + 12);
  m_directoryOffset          = u32(tail, end + 16);
  if (u16(tail, end + 4) || u16(tail, end + 6) || u16(tail, end + 8) != count)
    throw Error("Multi-volume FLA archives are not supported.");
  if (count == 65535 || declaredSize == 0xffffffff ||
      m_directoryOffset == 0xffffffff)
    throw Error("ZIP64 FLA archives are not supported.");
  if (!count || count > m_limits.maxEntries || m_directoryOffset >= endOffset)
    throw Error("Invalid ZIP directory bounds.");
  seek(*m_file, m_directoryOffset);
  QVector<QPair<quint64, quint64>> ranges;
  for (quint32 index = 0; index < count; ++index) {
    checkCancelled();
    if (quint64(m_file->pos()) + 46 > endOffset)
      throw Error("Truncated ZIP directory.");
    const QByteArray record = exact(*m_file, 46);
    if (u32(record, 0) != 0x02014b50)
      throw Error("Invalid ZIP directory record.");
    Entry entry;
    entry.flags          = u16(record, 8);
    entry.method         = u16(record, 10);
    entry.crc            = u32(record, 16);
    entry.compressedSize = u32(record, 20);
    entry.size           = u32(record, 24);
    entry.offset         = u32(record, 42);
    if (u16(record, 34) || entry.compressedSize == 0xffffffff ||
        entry.size == 0xffffffff || entry.offset == 0xffffffff)
      throw Error("ZIP64 or multi-volume entry is not supported.");
    if (entry.flags & (1 | 0x40 | 0x2000))
      throw Error("Encrypted FLA entries are not supported.");
    if (entry.method != 0 && entry.method != 8)
      throw Error("Unsupported ZIP compression method.");
    const quint32 nameLength    = u16(record, 28);
    const quint32 extraLength   = u16(record, 30);
    const quint32 commentLength = u16(record, 32);
    if (quint64(m_file->pos()) + nameLength + extraLength + commentLength >
        endOffset)
      throw Error("ZIP filename extends beyond the directory.");
    entry.encodedName  = exact(*m_file, nameLength);
    const QString name = decodeName(entry.encodedName, entry.flags);
    exact(*m_file, extraLength + commentLength);
    if (m_entries.contains(name)) throw Error("Duplicate ZIP entry: " + name);
    if (entry.offset + 30 + entry.compressedSize > m_directoryOffset)
      throw Error("ZIP entry overlaps the directory: " + name);
    const qint64 returnPosition = m_file->pos();
    seek(*m_file, entry.offset);
    const QByteArray local = exact(*m_file, 30);
    if (u32(local, 0) != 0x04034b50 || u16(local, 6) != entry.flags ||
        u16(local, 8) != entry.method)
      throw Error("ZIP local header disagrees with the directory: " + name);
    if (exact(*m_file, u16(local, 26)) != entry.encodedName)
      throw Error("ZIP local filename disagrees with the directory: " + name);
    const quint64 dataOffset = quint64(m_file->pos()) + u16(local, 28);
    if (dataOffset + entry.compressedSize > m_directoryOffset)
      throw Error("ZIP payload overlaps the directory: " + name);
    if (!(entry.flags & 8) && (u32(local, 14) != entry.crc ||
                               u32(local, 18) != entry.compressedSize ||
                               u32(local, 22) != entry.size))
      throw Error("ZIP local sizes or checksum disagree: " + name);
    ranges.push_back(
        qMakePair(entry.offset, dataOffset + entry.compressedSize));
    seek(*m_file, quint64(returnPosition));
    m_entries.insert(name, entry);
  }
  if (quint64(m_file->pos()) != endOffset)
    throw Error("Unexpected records after the ZIP directory.");
  std::sort(ranges.begin(), ranges.end());
  for (int i = 1; i < ranges.size(); ++i)
    if (ranges[i].first < ranges[i - 1].second)
      throw Error("Overlapping ZIP members.");
  const quint64 actualSize = endOffset - m_directoryOffset;
  if (declaredSize != actualSize) {
    // Some Adobe FLA writers overstate directory size. Accept a validated
    // contiguous directory only when the package identifies itself as XFL.
    // The offset/count/layout remain authoritative; no fixed correction.
    if (declaredSize < actualSize || declaredSize - actualSize > 65535 ||
        !contains("DOMDocument.xml") || !contains("mimetype") ||
        read("mimetype").trimmed() != "application/vnd.adobe.xfl")
      throw Error("Invalid ZIP directory size.");
    m_directoryCompatibility = true;
    m_directoryDelta         = declaredSize - actualSize;
  }
  if (!contains("DOMDocument.xml"))
    throw Error("This archive does not contain an XFL document.");
}

QStringList Package::names() const { return m_entries.keys(); }

bool Package::contains(const QString &name) const {
  return safeName(name) && m_entries.contains(name);
}

QByteArray Package::read(const QString &name, quint64 maxBytes) {
  checkCancelled();
  if (!safeName(name) || !m_entries.contains(name) || name.endsWith('/'))
    throw Error("Missing project member: " + name);
  const Entry &entry = m_entries[name];
  if (entry.size > maxBytes || entry.size > m_limits.maxMemberBytes ||
      entry.size > quint64(std::numeric_limits<int>::max()) ||
      entry.size >
          m_limits.maxReadBytes - std::min(m_readBytes, m_limits.maxReadBytes))
    throw Error("Project decompression budget exceeded: " + name);
  // Charge repeated reads too, including cancelled/failed attempts.
  m_readBytes += entry.size;
  if (m_file) return readArchive(name, entry);
  const QFileInfo fileInfo(m_directory + '/' + name);
  const QString canonical = fileInfo.canonicalFilePath();
  if (canonical.isEmpty() || fileInfo.isSymLink() ||
      !canonical.startsWith(m_directory + '/', Qt::CaseSensitive))
    throw Error("XFL member is outside the project: " + name);
  QFile file(canonical);
  if (!file.open(QIODevice::ReadOnly) || quint64(file.size()) != entry.size)
    throw Error("XFL member changed or cannot be read: " + name);
  return exact(file, qint64(entry.size));
}

QByteArray Package::readArchive(const QString &name, const Entry &entry) {
  seek(*m_file, entry.offset);
  const QByteArray local = exact(*m_file, 30);
  seek(*m_file, entry.offset + 30 + u16(local, 26) + u16(local, 28));
  QByteArray output;
  // The declared size is untrusted until the CRC/size check below; reserve a
  // bounded amount and let appends grow the buffer as data actually arrives.
  output.reserve(int(std::min<quint64>(entry.size, initialReserveBytes)));
  quint64 remaining = entry.compressedSize;
  uLong crc         = crc32(0, Z_NULL, 0);
  if (entry.method == 0) {
    if (entry.compressedSize != entry.size)
      throw Error("Stored ZIP member has inconsistent sizes: " + name);
    while (remaining) {
      checkCancelled();
      const QByteArray bytes =
          exact(*m_file, qint64(std::min<quint64>(remaining, 65536)));
      remaining -= bytes.size();
      crc = crc32(crc, reinterpret_cast<const Bytef *>(bytes.constData()),
                  bytes.size());
      output.append(bytes);
    }
  } else {
    z_stream stream = {};
    if (inflateInit2(&stream, -MAX_WBITS) != Z_OK)
      throw Error("Cannot initialize ZIP decompression.");
    struct EndInflate {
      z_stream *stream;
      ~EndInflate() { inflateEnd(stream); }
    } guard{&stream};
    QByteArray input;
    char buffer[65536];
    int status = Z_OK;
    while (status != Z_STREAM_END) {
      checkCancelled();
      if (!stream.avail_in && remaining) {
        input = exact(*m_file, qint64(std::min<quint64>(remaining, 65536)));
        remaining -= input.size();
        stream.next_in  = reinterpret_cast<Bytef *>(input.data());
        stream.avail_in = uInt(input.size());
      }
      stream.next_out  = reinterpret_cast<Bytef *>(buffer);
      stream.avail_out = sizeof(buffer);
      status           = inflate(&stream, Z_NO_FLUSH);
      if (status != Z_OK && status != Z_STREAM_END)
        throw Error("Invalid compressed member: " + name);
      const int produced = int(sizeof(buffer) - stream.avail_out);
      if (quint64(output.size()) + produced > entry.size)
        throw Error("ZIP member expands beyond its declared size: " + name);
      output.append(buffer, produced);
      crc = crc32(crc, reinterpret_cast<const Bytef *>(buffer), produced);
      if (!produced && !stream.avail_in && !remaining && status != Z_STREAM_END)
        throw Error("Incomplete compressed member: " + name);
    }
    if (remaining || stream.avail_in)
      throw Error("Trailing bytes in compressed member: " + name);
  }
  if (quint64(output.size()) != entry.size || quint32(crc) != entry.crc)
    throw Error("ZIP member checksum or size mismatch: " + name);
  return output;
}

}  // namespace Flash
