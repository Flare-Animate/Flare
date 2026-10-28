#pragma once

#include <QByteArray>
#include <QMap>
#include <QString>
#include <QStringList>
#include <functional>
#include <memory>
#include <limits>
#include <stdexcept>

class QFile;

namespace Flash {

class Error : public std::runtime_error {
public:
  explicit Error(const QString &message)
      : std::runtime_error(message.toUtf8().constData()) {}
};

struct Limits {
  quint64 maxMemberBytes      = 256ull * 1024 * 1024;
  quint64 maxReadBytes        = 2ull * 1024 * 1024 * 1024;
  quint32 maxEntries          = 100000;
  quint64 maxXmlBytes         = 64ull * 1024 * 1024;
  quint64 maxDocumentXmlBytes = 512ull * 1024 * 1024;
  quint64 maxXmlElements      = 4000000;
  quint64 maxGeometrySegments = 4000000;
  int maxXmlDepth             = 128;
  // Longest chain of nested library symbols (a symbol without symbol
  // children has depth 1). Scene builders must enforce it too.
  int maxSymbolDepth = 256;
  std::function<bool()> cancelled;
};

// Reads FLA/XFL archives or uncompressed XFL projects without extracting files.
// File content distinguishes compressed .xfl packages from project markers.
// Instances are single-threaded; cancellation is checked during inflation.
class Package {
public:
  explicit Package(const QString &path, Limits limits = Limits());
  ~Package();
  QStringList names() const;
  bool contains(const QString &name) const;
  QByteArray read(const QString &name,
                  quint64 maxBytes = std::numeric_limits<quint64>::max());
  bool usedDirectoryCompatibility() const { return m_directoryCompatibility; }
  quint64 declaredDirectorySizeDifference() const { return m_directoryDelta; }
  bool isUncompressed() const { return !m_file; }
  const Limits &limits() const { return m_limits; }
  void checkCancelled() const;

private:
  struct Entry {
    quint16 flags          = 0;
    quint16 method         = 0;
    quint32 crc            = 0;
    quint64 compressedSize = 0;
    quint64 size           = 0;
    quint64 offset         = 0;
    QByteArray encodedName;
  };
  Limits m_limits;
  std::unique_ptr<QFile> m_file;
  QString m_directory;
  QMap<QString, Entry> m_entries;
  quint64 m_directoryOffset     = 0;
  quint64 m_readBytes           = 0;
  bool m_directoryCompatibility = false;
  quint64 m_directoryDelta      = 0;

  void openArchive();
  QByteArray readArchive(const QString &name, const Entry &entry);
};

}  // namespace Flash
