#include "flashimport.h"
#include <QThread>
#include <thread>
#include <chrono>
#include <atomic>
#include "flashpreflight.h"
#include "flashbundle.h"
#include "flashfieldbindings.h"
#include "flashloadingrange.h"
#include "../flash/fieldaccounting.h"

#include "flare/levelset.h"
#include "flare/txsheet.h"
#include "flare/txshsimplelevel.h"
#include "flare/txshsoundcolumn.h"
#include "flare/txshsoundlevel.h"
#include "tlevel_io.h"
#include "texception.h"
#include "tenv.h"
#include "trasterimage.h"
#include "tvectorimage.h"
#include "tsound.h"
#include "tsound_io.h"
#include "../flash/media.h"
#include "../flash/video.h"
#include "thirdparty.h"

#include <QCoreApplication>
#include <QDateTime>
#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QSaveFile>
#include <QTemporaryDir>
#include <algorithm>
#include <cmath>
#include <limits>

#ifdef Q_OS_WIN
#include <qt_windows.h>
#include <io.h>
#else
#include <sys/stat.h>
#include <unistd.h>
#endif

namespace FlashNative {
namespace {

// Writes a whole file atomically; throws `failure` and the reason otherwise.
void saveFile(const QString &path, const QByteArray &bytes,
              const QString &failure) {
  QSaveFile file(path);
  if (!file.open(QIODevice::WriteOnly) || file.write(bytes) != bytes.size() ||
      !file.commit())
    throw Flash::Error(failure + file.errorString());
}

TFilePath nativePath(const QString &path) {
  return TFilePath(path.toStdWString());
}

// Identify the directory itself rather than following a replaced staging
// symlink. Cleanup is allowed only while this identity still belongs to us.
struct DirectoryIdentity {
  quint64 device = 0, file = 0;
  bool valid = false;

  static DirectoryIdentity read(const QString &path) {
    DirectoryIdentity result;
#ifdef Q_OS_WIN
    HANDLE handle = CreateFileW(
        reinterpret_cast<LPCWSTR>(path.utf16()), FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
    if (handle == INVALID_HANDLE_VALUE) return result;
    BY_HANDLE_FILE_INFORMATION info;
    if (GetFileInformationByHandle(handle, &info) &&
        (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) &&
        !(info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)) {
      result.device = info.dwVolumeSerialNumber;
      result.file   = (quint64(info.nFileIndexHigh) << 32) | info.nFileIndexLow;
      result.valid  = true;
    }
    CloseHandle(handle);
#else
    struct stat info;
    if (::lstat(QFile::encodeName(path).constData(), &info) == 0 &&
        S_ISDIR(info.st_mode)) {
      result.device = quint64(info.st_dev);
      result.file   = quint64(info.st_ino);
      result.valid  = true;
    }
#endif
    return result;
  }

  bool operator==(const DirectoryIdentity &other) const {
    return valid && other.valid && device == other.device && file == other.file;
  }
};

struct SourceFileIdentity {
  quint64 device = 0, file = 0;
  bool valid = false;

#ifdef Q_OS_WIN
  static SourceFileIdentity fromHandle(HANDLE handle) {
    SourceFileIdentity result;
    BY_HANDLE_FILE_INFORMATION info;
    if (handle != INVALID_HANDLE_VALUE &&
        GetFileInformationByHandle(handle, &info) &&
        !(info.dwFileAttributes &
          (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT))) {
      result.device = info.dwVolumeSerialNumber;
      result.file   = (quint64(info.nFileIndexHigh) << 32) | info.nFileIndexLow;
      result.valid  = true;
    }
    return result;
  }
#else
  static SourceFileIdentity fromStat(const struct stat &info) {
    SourceFileIdentity result;
    if (S_ISREG(info.st_mode)) {
      result.device = quint64(info.st_dev);
      result.file = quint64(info.st_ino);
      result.valid = true;
    }
    return result;
  }
#endif

  static SourceFileIdentity read(const QString &path) {
#ifdef Q_OS_WIN
    HANDLE handle = CreateFileW(
        reinterpret_cast<LPCWSTR>(path.utf16()), FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
        OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
    const auto result = fromHandle(handle);
    if (handle != INVALID_HANDLE_VALUE) CloseHandle(handle);
    return result;
#else
    struct stat info;
    return ::lstat(QFile::encodeName(path).constData(), &info) == 0
               ? fromStat(info)
               : SourceFileIdentity();
#endif
  }

  static SourceFileIdentity opened(const QFile &file) {
#ifdef Q_OS_WIN
    return fromHandle(
        reinterpret_cast<HANDLE>(::_get_osfhandle(file.handle())));
#else
    struct stat info;
    return ::fstat(file.handle(), &info) == 0 ? fromStat(info)
                                              : SourceFileIdentity();
#endif
  }

  bool operator==(const SourceFileIdentity &other) const {
    return valid && other.valid && device == other.device && file == other.file;
  }
};

class StagingDirectory {
  QTemporaryDir m_directory;
  DirectoryIdentity m_identity;
  bool m_owned = true;

public:
  explicit StagingDirectory(const QString &parent)
      : m_directory(QDir(parent).filePath(".flare-flash-import-XXXXXX")) {
    if (!m_directory.isValid())
      throw Flash::Error("Cannot create native import staging directory: " +
                         m_directory.errorString());
    m_identity = DirectoryIdentity::read(m_directory.path());
    if (!m_identity.valid)
      throw Flash::Error("Cannot identify native import staging directory.");
    // A failed constructor never runs our destructor. Keep Qt's owned-folder
    // cleanup until identity acquisition succeeds, then use guarded cleanup.
    m_directory.setAutoRemove(false);
#ifdef Q_OS_WIN
    // Shell views (including a file dialog already loaded in this process)
    // skip hidden folders instead of probing files while they are written.
    const std::wstring native =
        QDir::toNativeSeparators(m_directory.path()).toStdWString();
    const DWORD attributes = GetFileAttributesW(native.c_str());
    if (attributes != INVALID_FILE_ATTRIBUTES)
      SetFileAttributesW(native.c_str(), attributes | FILE_ATTRIBUTE_HIDDEN);
#endif
  }

  ~StagingDirectory() {
    if (m_owned && m_identity == DirectoryIdentity::read(path()))
      QDir(path()).removeRecursively();
  }

  QString path() const { return m_directory.path(); }

  void check() const {
    if (!(m_identity == DirectoryIdentity::read(path())))
      throw Flash::Error("Native import staging directory changed.");
  }

  void release() { m_owned = false; }
};

bool occupied(const QString &path) {
  const QFileInfo info(path);
  // exists() alone ignores a dangling symlink.
  return info.exists() || info.isSymLink();
}

void renameWithoutReplace(const StagingDirectory &staging,
                          const QString &destination) {
  // Do not fall back to POSIX/Qt rename: it can replace an empty directory.
  try {
    commitDirectoryNoReplaceWithRetry(
        staging.path(), destination, "native import", [&] { staging.check(); });
  } catch (const TException &error) {
    throw Flash::Error(QString::fromStdWString(error.getMessage()));
  }
}

void progress(const ImportOptions &options, const QString &stage, int value = 0,
              int maximum = 0) {
  if (options.limits.cancelled && options.limits.cancelled())
    throw Flash::Error("Flash import cancelled.");
  if (options.progress) options.progress(stage, value, maximum);
  if (options.limits.cancelled && options.limits.cancelled())
    throw Flash::Error("Flash import cancelled.");
}

void checkCancelled(const ImportOptions &options) {
  if (options.limits.cancelled && options.limits.cancelled())
    throw Flash::Error("Flash import cancelled.");
}

struct SnapshotFile {
  QString originalPath;
  QString originalMember;
  QString relativePath;
  quint64 bytes = 0;
  QByteArray sha256;
  SourceFileIdentity identity;
};

struct SourceSnapshot {
  QString originalInput;
  QString originalRoot;
  QString entry;
  QStringList members;
  QVector<SnapshotFile> files;
  QJsonObject manifest;
  QByteArray manifestSha256;
  DirectoryIdentity originalRootIdentity;
  bool uncompressed = false;
};

bool safeSnapshotMember(const QString &name) {
  if (name.isEmpty() || name.contains(QChar(0)) || name.contains('\\') ||
      name.contains(':') || QDir::isAbsolutePath(name))
    return false;
  for (const auto &part : name.split('/'))
    if (part.isEmpty() || part == "." || part == "..") return false;
  return true;
}

void requireSourceFile(const QString &path, const QString &root = {}) {
  const QFileInfo file(path);
  const auto canonical = file.canonicalFilePath();
  if (!file.isFile() || file.isSymLink() || canonical.isEmpty() ||
      (!root.isEmpty() && !canonical.startsWith(root + '/')))
    throw Flash::Error("Source snapshot file is unsafe or changed: " + path);
  if (!root.isEmpty()) {
    const QString relative = QDir(root).relativeFilePath(file.absolutePath());
    QString ancestor       = root;
    const auto parts       = relative.split('/');
    for (const auto &part : parts) {
      if (part == ".") continue;
      ancestor = QDir(ancestor).filePath(part);
      if (!DirectoryIdentity::read(ancestor).valid)
        throw Flash::Error("Source snapshot directory is unsafe: " + ancestor);
    }
  }
}

QByteArray hashFile(const QString &path, quint64 expectedBytes,
                    const ImportOptions &options,
                    const std::function<void()> &check,
                    const SourceFileIdentity *expectedIdentity   = nullptr,
                    const std::function<void(quint64)> &advanced = {}) {
  requireSourceFile(path);
  const auto identity = SourceFileIdentity::read(path);
  QFile file(path);
  if (!file.open(QIODevice::ReadOnly) || file.size() < 0 ||
      quint64(file.size()) != expectedBytes ||
      !(identity == SourceFileIdentity::opened(file)) ||
      (expectedIdentity && !(identity == *expectedIdentity)))
    throw Flash::Error("Source snapshot file size changed: " + path);
  QCryptographicHash hash(QCryptographicHash::Sha256);
  quint64 bytes = 0;
  while (!file.atEnd()) {
    check();
    const auto block = file.read(1024 * 1024);
    if (block.isEmpty() && (file.error() != QFile::NoError || !file.atEnd()))
      throw Flash::Error("Cannot verify source snapshot bytes: " + path);
    bytes += quint64(block.size());
    if (bytes > expectedBytes)
      throw Flash::Error("Source snapshot file grew while reading: " + path);
    hash.addData(block);
    if (advanced) advanced(bytes);
    checkCancelled(options);
  }
  if (bytes != expectedBytes || file.error() != QFile::NoError ||
      !(identity == SourceFileIdentity::read(path)))
    throw Flash::Error("Source snapshot file changed while reading: " + path);
  return hash.result().toHex();
}

void snapshotDirectory(const QString &staging, const QString &relative,
                       QMap<QString, DirectoryIdentity> &directories,
                       const std::function<void()> &check) {
  QString current = staging;
  for (const auto &part : relative.split('/')) {
    if (part.isEmpty() || part == "." || part == ".." || part.contains(':') ||
        part.contains('\\'))
      throw Flash::Error("Unsafe source snapshot directory component.");
    check();
    current = QDir(current).filePath(part);
    if (!occupied(current) && !QDir().mkdir(current))
      throw Flash::Error("Cannot create source snapshot directory.");
    const auto identity = DirectoryIdentity::read(current);
    if (!identity.valid ||
        (directories.contains(current) && !(directories[current] == identity)))
      throw Flash::Error("Source snapshot directory changed.");
    directories.insert(current, identity);
  }
}

SourceSnapshot planSourceSnapshot(Flash::Package &package,
                                  const QString &sourcePath,
                                  const ImportOptions &options,
                                  const std::function<void()> &check) {
  SourceSnapshot result;
  result.originalInput = QFileInfo(sourcePath).absoluteFilePath();
  result.uncompressed  = package.isUncompressed();
  result.members       = package.names();
  const QFileInfo input(sourcePath);
  if (input.isSymLink())
    throw Flash::Error("A source snapshot cannot read a symbolic-link input.");
  if (!result.uncompressed) {
    requireSourceFile(result.originalInput);
    result.entry = "source/archive.fla";
    SnapshotFile file;
    file.originalPath = result.originalInput;
    file.relativePath = result.entry;
    result.files.push_back(file);
  } else {
    result.originalRoot =
        input.isDir() ? input.canonicalFilePath() : input.dir().canonicalPath();
    result.originalRootIdentity = DirectoryIdentity::read(result.originalRoot);
    if (!result.originalRootIdentity.valid)
      throw Flash::Error("Cannot identify the uncompressed source directory.");
    result.entry = "source/xfl/DOMDocument.xml";
    for (const auto &member : result.members) {
      if (!safeSnapshotMember(member))
        throw Flash::Error("Unsafe uncompressed source snapshot member.");
      SnapshotFile file;
      file.originalMember = member;
      file.originalPath   = QDir(result.originalRoot).filePath(member);
      file.relativePath   = "source/xfl/" + member;
      requireSourceFile(file.originalPath, result.originalRoot);
      result.files.push_back(file);
    }
  }
  quint64 total = 0;
  for (auto &file : result.files) {
    file.identity   = SourceFileIdentity::read(file.originalPath);
    const auto size = QFileInfo(file.originalPath).size();
    if (!file.identity.valid || size < 0 ||
        quint64(size) > options.maxSourceSnapshotBytes - total ||
        (result.uncompressed && quint64(size) > options.limits.maxMemberBytes))
      throw Flash::Error("Original source snapshot exceeds its byte budget.");
    file.bytes = quint64(size);
    total += file.bytes;
  }
  quint64 hashed = 0;
  progress(options, "Hashing Flash source", 0, 1000);
  for (auto &file : result.files) {
    file.sha256 = hashFile(
        file.originalPath, file.bytes, options, check, &file.identity,
        [&](quint64 current) {
          const int value =
              total ? int(1000.0 * double(hashed + current) / double(total))
                    : 0;
          progress(options, "Hashing Flash source", std::min(1000, value),
                   1000);
        });
    hashed += file.bytes;
  }
  return result;
}

void preserveSource(SourceSnapshot &result, const QString &staging,
                    const ImportOptions &options,
                    QMap<QString, DirectoryIdentity> &directories,
                    const std::function<void()> &check) {
  quint64 total = 0, copied = 0;
  for (const auto &file : result.files) total += file.bytes;
  snapshotDirectory(staging, "source", directories, check);
  for (auto &file : result.files) {
    check();
    requireSourceFile(file.originalPath, result.originalRoot);
    const QString output = QDir(staging).filePath(file.relativePath);
    const QString parent = QFileInfo(file.relativePath).path();
    snapshotDirectory(staging, parent, directories, check);
    if (occupied(output))
      throw Flash::Error("Source snapshot destination already exists.");
    QFile inputFile(file.originalPath);
    QSaveFile outputFile(output);
    if (!inputFile.open(QIODevice::ReadOnly) || inputFile.size() < 0 ||
        quint64(inputFile.size()) != file.bytes ||
        !(file.identity == SourceFileIdentity::opened(inputFile)) ||
        !(file.identity == SourceFileIdentity::read(file.originalPath)) ||
        !outputFile.open(QIODevice::WriteOnly))
      throw Flash::Error("Cannot open original source snapshot files.");
    requireSourceFile(file.originalPath, result.originalRoot);
    QCryptographicHash hash(QCryptographicHash::Sha256);
    quint64 bytes = 0;
    while (!inputFile.atEnd()) {
      check();
      const auto block = inputFile.read(1024 * 1024);
      if (block.isEmpty() &&
          (inputFile.error() != QFile::NoError || !inputFile.atEnd()))
        throw Flash::Error("Cannot read original source snapshot bytes.");
      bytes += quint64(block.size());
      if (bytes > file.bytes || outputFile.write(block) != block.size())
        throw Flash::Error("Cannot save complete original source snapshot.");
      hash.addData(block);
      copied += quint64(block.size());
      const int value =
          total ? int(1000.0 * double(copied) / double(total)) : 0;
      progress(options, "Preserving Flash source", std::min(1000, value), 1000);
      check();
    }
    if (bytes != file.bytes || inputFile.error() != QFile::NoError ||
        !(file.identity == SourceFileIdentity::read(file.originalPath)) ||
        hash.result().toHex() != file.sha256 || !outputFile.commit())
      throw Flash::Error("Original source changed or snapshot saving failed.");
  }
  QJsonArray files;
  for (const auto &file : result.files)
    files.append(QJsonObject{{"originalMember", file.originalMember},
                             {"path", file.relativePath},
                             {"bytes", double(file.bytes)},
                             {"sha256", QString::fromLatin1(file.sha256)}});
  result.manifest = QJsonObject{
      {"schemaVersion", 1},
      {"kind", result.uncompressed ? "uncompressed-xfl" : "exact-archive"},
      {"hashAlgorithm", "SHA-256"},
      {"originalInputName", QFileInfo(result.originalInput).fileName()},
      {"entry", result.entry},
      {"totalBytes", double(total)},
      {"files", files},
      {"packageMembers", QJsonArray::fromStringList(result.members)},
      {"memberNameRepresentation", "canonical-package-paths"},
      {"originalArchiveBytesPreserved", !result.uncompressed},
      {"nativeFeatureCompletenessVerified", false}};
  const auto manifest =
      QJsonDocument(result.manifest).toJson(QJsonDocument::Indented);
  QSaveFile manifestFile(QDir(staging).filePath("source/manifest.json"));
  check();
  if (!manifestFile.open(QIODevice::WriteOnly) ||
      manifestFile.write(manifest) != manifest.size() || !manifestFile.commit())
    throw Flash::Error("Cannot write the original source snapshot manifest.");
  result.manifestSha256 =
      QCryptographicHash::hash(manifest, QCryptographicHash::Sha256).toHex();
}

void verifySourceSnapshot(const SourceSnapshot &snapshot,
                          const QString &staging, const ImportOptions &options,
                          const std::function<void()> &check) {
  check();
  if (QFileInfo(snapshot.originalInput).isSymLink() ||
      (snapshot.uncompressed &&
       !(snapshot.originalRootIdentity ==
         DirectoryIdentity::read(snapshot.originalRoot))))
    throw Flash::Error("Original source input directory or link changed.");
  Flash::Package original(snapshot.originalInput, options.limits);
  if (original.isUncompressed() != snapshot.uncompressed)
    throw Flash::Error("Original source package membership changed.");
  // Element-wise compare: QStringList::operator== does not compile on MSVC
  // STLs that dropped stdext::make_checked_array_iterator.
  const QStringList originalNames = original.names();
  if (!std::equal(originalNames.cbegin(), originalNames.cend(),
                  snapshot.members.cbegin(), snapshot.members.cend()))
    throw Flash::Error("Original source package membership changed.");
  for (const auto &file : snapshot.files) {
    requireSourceFile(file.originalPath, snapshot.originalRoot);
    if (hashFile(file.originalPath, file.bytes, options, check,
                 &file.identity) != file.sha256 ||
        hashFile(QDir(staging).filePath(file.relativePath), file.bytes, options,
                 check) != file.sha256)
      throw Flash::Error("Original source or preserved snapshot hash changed.");
  }
  const auto bytes =
      QJsonDocument(snapshot.manifest).toJson(QJsonDocument::Indented);
  if (hashFile(QDir(staging).filePath("source/manifest.json"),
               quint64(bytes.size()), options,
               check) != snapshot.manifestSha256)
    throw Flash::Error("Original source snapshot manifest changed.");
}

QString destinationPath(const ImportOptions &options) {
  if (options.destinationDirectory.contains(QChar(0)) ||
      !QDir::isAbsolutePath(options.destinationDirectory) ||
      !std::isfinite(options.dpi) || options.dpi <= 0 ||
      options.maxSourceSnapshotBytes == 0 ||
      options.maxSourceSnapshotBytes > 9007199254740991ull)
    throw Flash::Error("Invalid native import destination or DPI.");
  const QFileInfo requested(QDir::cleanPath(options.destinationDirectory));
  const QFileInfo parent(requested.absolutePath());
  if (requested.fileName().isEmpty() || requested.fileName() == "." ||
      requested.fileName() == ".." || !parent.isDir())
    throw Flash::Error(
        "The native import destination needs an existing parent "
        "directory.");
  // Resolve existing parent aliases once; the new destination must itself be
  // absent, including any dangling link.
  const QString destination =
      QDir(parent.canonicalFilePath()).filePath(requested.fileName());
  if (occupied(destination))
    throw Flash::Error("Native import destination already exists: " +
                       destination);
  return destination;
}

void checkSourceDestination(const QString &sourcePath,
                            const QString &destination, bool uncompressed) {
  if (!uncompressed) return;
  const QFileInfo source(sourcePath);
  const QString root     = source.isDir() ? source.canonicalFilePath()
                                          : source.dir().canonicalPath();
  const QString relative = QDir(root).relativeFilePath(destination);
  if (relative == "." ||
      (!relative.startsWith("../") && !QDir::isAbsolutePath(relative)))
    throw Flash::Error(
        "The native bundle must be outside the original XFL project.");
}

void requireCompatible(const QJsonObject &preflight) {
  if (preflight.value("canImport").toBool()) return;
  QString message =
      "Flash compatibility check found " +
      QString::number(preflight.value("errorCount").toDouble(), 'f', 0) +
      " unsupported or invalid source feature(s).";
  for (const auto &value : preflight.value("issues").toArray()) {
    const auto issue = value.toObject();
    if (issue.value("severity") != "error") continue;
    message += "\n" + issue.value("sourceId").toString() + ": " +
               issue.value("message").toString();
    break;
  }
  throw Flash::Error(message);
}

// Reopens one saved level and checks it against the in-memory originals.
// Runs on worker threads: it only uses its arguments and the file.
struct LevelCheck {
  TFilePath path;
  std::vector<TFrameId> frames;
  bool raster = false;
  QString name;
  QMap<int, TVectorImageP> originals;
  QMap<int, VectorFrameAudit> audits;
  // Frames [begin, end) are reopened and checked; large levels are split into
  // several checks so they spread across cores.
  int begin = 0, end = 0;
  // Results
  QJsonArray differences;
  QString error;
};

void verifyLevel(LevelCheck &check, const std::function<void()> &cancelled,
                 bool bestEffort) {
  const TFilePath &path                          = check.path;
  const std::vector<TFrameId> &frames            = check.frames;
  const QMap<int, VectorFrameAudit> &frameAudits = check.audits;
  cancelled();
  const bool raster = check.raster;
  TLevelReaderP reader(path);
  const auto level       = reader->loadInfo();
  const QString filename = QString::fromStdWString(path.getWideString());
  if (!level || level->isPartialLoad() ||
      level->getFrameCount() != int(frames.size()))
    throw Flash::Error("Native drawing was not saved completely: " + filename);
  if (!raster && !level->getPalette())
    throw Flash::Error("Native drawing palette could not be reopened: " +
                       filename);
  auto saved = level->begin();
  for (int ordinal = 0; ordinal < int(frames.size()); ++ordinal) {
    const TFrameId &frame = frames[ordinal];
    cancelled();
    if (saved == level->end() || saved->first != frame)
      throw Flash::Error("Native drawing frame IDs changed while saving.");
    if (ordinal < check.begin || ordinal >= check.end) {
      ++saved;
      continue;
    }
    const TImageP image = reader->getFrameReader(frame)->load();
    if (!image || (raster ? !TRasterImageP(image) : !TVectorImageP(image)))
      throw Flash::Error(
          "Native drawing frame could not be reopened: " + filename +
          " (frame " + QString::number(frame.getNumber()) + ").");
    // PLI stores its shared palette on the loaded TLevel. Frame readers return
    // geometry; native simple-level loading attaches that palette separately.
    if (!raster) {
      image->setPalette(level->getPalette());
      const auto original = check.originals.value(frame.getNumber());
      const auto audit    = frameAudits.constFind(frame.getNumber());
      const QString location =
          audit == frameAudits.cend() || audit->sourceId.isEmpty()
              ? check.name
              : audit->sourceId;
      // Point-sampled fallback fills carry no exact topology certificate.
      if (audit != frameAudits.cend() && audit->approximatedFills) {
        if (!original || original->getStrokeCount() !=
                             TVectorImageP(image)->getStrokeCount())
          throw Flash::Error("Native drawing lost strokes while saving: " +
                             location);
        ++saved;
        continue;
      }
      if (!original ||
          (original->getStrokeCount() && audit == frameAudits.cend()))
        throw Flash::Error(
            QString("Native vector audit is missing at %1, frame %2.")
                .arg(location)
                .arg(frame.getNumber()));
      try {
        const VectorFrameAudit emptyAudit;
        verifyVectorFramePersistence(
            original, TVectorImageP(image),
            audit == frameAudits.cend() ? emptyAudit : audit.value(),
            cancelled);
      } catch (const Flash::Error &error) {
        const QString message =
            QString("Native PLI changed source artwork at %1, frame %2: %3")
                .arg(location)
                .arg(frame.getNumber())
                .arg(QString::fromUtf8(error.what()));
        cancelled();
        // Best-effort imports keep the saved drawing (the difference is
        // usually one fill region) and report it instead of failing.
        if (!bestEffort) throw Flash::Error(message);
        check.differences.append(
            QJsonObject{{"code", "pli-persistence-difference"},
                        {"sourceId", location},
                        {"message", message},
                        {"handling", "approximated"}});
      }
    }
    ++saved;
  }
}

// Writes an imported sound level as a canonical PCM WAV file and reads it
// back: the bundle must hold exactly the samples of the native track.
QJsonObject saveSound(TXshSoundLevel *sound, const TFilePath &path,
                      const QString &relative) {
  const TSoundTrackP track = sound->getSoundTrack();
  if (!track || track->getSampleCount() <= 0)
    throw Flash::Error("Native imported sound has no samples: " + relative);
  Flash::SoundFormat format;
  format.sampleRate = int(track->getSampleRate());
  format.bits       = track->getBitPerSample();
  format.channels   = track->getChannelCount();
  // WAV PCM stores 8-bit samples unsigned and 16-bit samples signed.
  if (track->getSampleType() == TSound::FLOAT ||
      (format.channels != 1 && format.channels != 2) ||
      !((format.bits == 16 && track->isSampleSigned()) ||
        (format.bits == 8 && !track->isSampleSigned())))
    throw Flash::Error("Native imported sound has no WAV layout: " + relative);
  const QByteArray pcm(
      reinterpret_cast<const char *>(track->getRawData()),
      int(qint64(track->getSampleCount()) * format.frameBytes()));
  const QByteArray bytes = Flash::waveFile(format, pcm);
  const QString filename = QString::fromStdWString(path.getWideString());
  if (occupied(filename))
    throw Flash::Error("Native sound file already exists: " + relative);
  saveFile(filename, bytes, "Cannot save native sound " + relative + ": ");
  QFile saved(filename);
  Flash::SoundFormat savedFormat;
  QByteArray savedPcm;
  if (!saved.open(QIODevice::ReadOnly) ||
      !Flash::readWaveFile(saved.readAll(), savedFormat, savedPcm) ||
      savedFormat.sampleRate != format.sampleRate ||
      savedFormat.bits != format.bits ||
      savedFormat.channels != format.channels || savedPcm != pcm)
    throw Flash::Error("Native sound was not saved completely: " + relative);
  return QJsonObject{
      {"path", relative},
      {"kind", "sound"},
      {"frames", sound->getFrameCount()},
      {"samples", double(track->getSampleCount())},
      {"sampleRate", format.sampleRate},
      {"channels", format.channels},
      {"bitsPerSample", format.bits},
      {"sha256", QString::fromLatin1(
                     QCryptographicHash::hash(bytes, QCryptographicHash::Sha256)
                         .toHex())}};
}

// Checks the reopened scene's sounds: portable WAV paths and, when a WAV
// reader is installed (the application always installs one), that every
// sound level loads the saved samples and every sound column keeps its rows.
void verifySounds(ToonzScene &verified, ToonzScene &original,
                  const ImportOptions &options) {
  QMap<QString, TXshSoundLevel *> originals;
  for (int index = 0; index < original.getLevelSet()->getLevelCount(); ++index)
    if (auto *sound = original.getLevelSet()->getLevel(index)->getSoundLevel())
      originals.insert(QString::fromStdWString(sound->getName()), sound);
  QStringList readers;
  TSoundTrackReader::getSupportedFormats(readers);
  const bool load = readers.contains("wav");
  int count       = 0;
  for (int index = 0; index < verified.getLevelSet()->getLevelCount();
       ++index) {
    checkCancelled(options);
    auto *sound = verified.getLevelSet()->getLevel(index)->getSoundLevel();
    if (!sound) continue;
    ++count;
    QString path = QString::fromStdWString(sound->getPath().getWideString());
    path.replace('\\', '/');
    const QString name     = QString::fromStdWString(sound->getName());
    TXshSoundLevel *source = originals.value(name, nullptr);
    if (!source || !path.startsWith("$scenefolder/levels/") ||
        !path.endsWith(".wav", Qt::CaseInsensitive))
      throw Flash::Error("Native scene contains a nonportable sound path.");
    if (!load) continue;
    sound->load();
    const TSoundTrackP track    = sound->getSoundTrack();
    const TSoundTrackP expected = source->getSoundTrack();
    if (!track || !expected ||
        track->getSampleCount() != expected->getSampleCount() ||
        track->getSampleRate() != expected->getSampleRate() ||
        track->getChannelCount() != expected->getChannelCount() ||
        track->getBitPerSample() != expected->getBitPerSample() ||
        sound->getFrameCount() != source->getFrameCount() ||
        QString::fromStdWString(sound->getName()) != name)
      throw Flash::Error("Native sound could not be reopened: " + path);
  }
  if (count != originals.size())
    throw Flash::Error("Native scene lost a sound level when reopened.");
  if (!load) return;
  TXsheet *sheet = verified.getTopXsheet(), *built = original.getTopXsheet();
  for (int index = 0; index < built->getColumnCount(); ++index) {
    auto *column = built->getColumn(index)
                       ? built->getColumn(index)->getSoundColumn()
                       : nullptr;
    if (!column) continue;
    auto *reopened = index < sheet->getColumnCount() && sheet->getColumn(index)
                         ? sheet->getColumn(index)->getSoundColumn()
                         : nullptr;
    int first = 0, last = -1, reopenedFirst = 0, reopenedLast = -1;
    column->getRange(first, last);
    if (reopened) reopened->getRange(reopenedFirst, reopenedLast);
    bool same = reopened && first == reopenedFirst && last == reopenedLast &&
                std::abs(reopened->getVolume() - column->getVolume()) < 1e-4;
    for (int row = first; same && row <= last; ++row) {
      if ((row & 1023) == 0) checkCancelled(options);
      const TXshCell a = column->getSoundCell(row);
      const TXshCell b = reopened->getSoundCell(row);
      same = a.isEmpty() == b.isEmpty() && a.getFrameId() == b.getFrameId() &&
             (a.isEmpty() || a.m_level->getName() == b.m_level->getName());
    }
    if (!same)
      throw Flash::Error("Native sound column " + QString::number(index + 1) +
                         " changed when reopened.");
  }
}

// The FFmpeg Flare is set up with (Preferences > Import/Export), else
// one found on the PATH or beside the application. Empty when there is none.
QString ffmpegExecutable() {
  QString folder = ThirdParty::getFFmpegDir();
  if (!ThirdParty::findFFmpeg(folder)) folder = ThirdParty::autodetectFFmpeg();
  if (!ThirdParty::findFFmpeg(folder)) return QString();
  if (folder.isEmpty() || folder.at(0) == '.')
    folder = QCoreApplication::applicationDirPath() + "/" + folder;
#ifdef Q_OS_WIN
  return QDir(folder).filePath("ffmpeg.exe");
#else
  return QDir(folder).filePath("ffmpeg");
#endif
}

}  // namespace

void attachMediaDecoders(BuildOptions &build, const Flash::DocumentInfo &info,
                         const ImportOptions &options) {
  auto has = [&info](const char *type) {
    return std::any_of(info.assets.constBegin(), info.assets.constEnd(),
                       [type](const QJsonValue &item) {
                         return item.toObject().value("type") == type;
                       });
  };
  // Embedded video is decoded by FFmpeg into PNG frames in levels/.
  build.decodeVideo = options.decodeVideo;
  // MP3 sounds are decoded by FFmpeg too (SWF ADPCM and PCM need none).
  build.decodeSound    = options.decodeSound;
  const bool needVideo = !build.decodeVideo && has("DOMVideoItem");
  const bool needSound = !build.decodeSound && has("DOMSoundItem");
  if (!needVideo && !needSound) return;
  const QString ffmpeg = ffmpegExecutable();
  if (ffmpeg.isEmpty()) return;
  if (needVideo)
    build.decodeVideo = [ffmpeg](const Flash::VideoPayload &video,
                                 const QString &directory,
                                 const QString &baseName,
                                 const std::function<void(int)> &advanced) {
      Flash::decodeVideoFrames(video, ffmpeg, directory, baseName, advanced);
    };
  if (needSound) {
    const auto cancelled = options.limits.cancelled;
    build.decodeSound    = [ffmpeg, cancelled](Flash::SoundPayload &sound) {
      Flash::decodeMp3Sound(sound, ffmpeg, [cancelled] {
        if (cancelled && cancelled())
          throw Flash::Error("Flash import cancelled.");
      });
    };
  }
}

double importProgressFraction(const QString &stage, int current, int total) {
  struct Span {
    const char *stage;
    double begin, end;
  };
  static const Span spans[] = {
      {"Inspecting Flash document", 0.00, 0.01},
      {"Hashing Flash source", 0.01, 0.03},
      {"Checking Flash compatibility", 0.03, 0.08},
      {"Preserving Flash source", 0.08, 0.10},
      {"Verifying preserved Flash source", 0.10, 0.17},
      {"Building editable scene", 0.17, 0.24},
      {"Decoding video", 0.17, 0.24},  // during scene building
      {"Converting drawings", 0.24, 0.46},
      {"Saving native drawings", 0.46, 0.54},
      {"Verifying native drawings", 0.54, 0.68},
      {"Saving native scene", 0.68, 0.70},
      {"Verifying native scene", 0.70, 0.80},
      {"Writing import report", 0.80, 0.94},
      {"Finishing import", 0.94, 1.00}};
  for (const auto &span : spans)
    if (stage == QLatin1String(span.stage)) {
      const double within =
          total > 0 ? std::max(0.0, std::min(1.0, double(current) / total))
                    : 0.0;
      return span.begin + (span.end - span.begin) * within;
    }
  return 0.0;
}

ImportResult importDocument(const QString &sourcePath,
                            const ImportOptions &options) {
  detail::LoadingRangeGuard loadingRange;
  checkCancelled(options);
  const QString absoluteSource       = QFileInfo(sourcePath).absoluteFilePath();
  const auto sourceFileIdentity      = SourceFileIdentity::read(absoluteSource);
  const auto sourceDirectoryIdentity = DirectoryIdentity::read(absoluteSource);
  if (!sourceFileIdentity.valid && !sourceDirectoryIdentity.valid)
    throw Flash::Error("A source snapshot cannot read a symbolic-link input.");
  const QString destination = destinationPath(options);
  const QString parent      = QFileInfo(destination).absolutePath();
  const auto parentIdentity = DirectoryIdentity::read(parent);
  if (!parentIdentity.valid)
    throw Flash::Error("Cannot identify the native import parent directory.");
  const auto checkSourceInput = [&] {
    checkCancelled(options);
    if (!(parentIdentity == DirectoryIdentity::read(parent)) ||
        (sourceFileIdentity.valid
             ? !(sourceFileIdentity == SourceFileIdentity::read(absoluteSource))
             : !(sourceDirectoryIdentity ==
                 DirectoryIdentity::read(absoluteSource))))
      throw Flash::Error(
          "Original source input or destination parent changed.");
  };
  progress(options, "Inspecting Flash document");
  checkSourceInput();
  Flash::Package originalPackage(sourcePath, options.limits);
  checkSourceDestination(sourcePath, destination,
                         originalPackage.isUncompressed());
  auto snapshot = planSourceSnapshot(originalPackage, sourcePath, options,
                                     checkSourceInput);
  auto info     = Flash::inspect(originalPackage);
  checkCancelled(options);
  BuildOptions buildOptions;
  buildOptions.assetDirectory       = QDir(destination).filePath("levels");
  buildOptions.project              = options.project;
  buildOptions.dpi                  = options.dpi;
  buildOptions.includeUnusedSymbols = options.includeUnusedSymbols;
  buildOptions.bestEffort           = options.bestEffort;
  buildOptions.showSymbolGuides     = options.showSymbolGuides;
  buildOptions.progress             = [&options](int done, int total) {
    progress(options, "Converting drawings", done, total);
  };
  attachMediaDecoders(buildOptions, info, options);
  buildOptions.videoProgress = [&options](int done, int total) {
    progress(options, "Decoding video", done, total);
  };
  progress(options, "Checking Flash compatibility");
  auto preflight = preflightScene(originalPackage, info, buildOptions);
  requireCompatible(preflight);
  checkCancelled(options);
  StagingDirectory staging(parent);
  const QString levelsDirectory = QDir(staging.path()).filePath("levels");
  if (!QDir(staging.path()).mkdir("levels"))
    throw Flash::Error("Cannot create the native import levels directory.");
  const auto levelsIdentity = DirectoryIdentity::read(levelsDirectory);
  QMap<QString, DirectoryIdentity> sourceDirectories;
  auto checkStaging = [&] {
    checkSourceInput();
    staging.check();
    if (!(parentIdentity == DirectoryIdentity::read(parent)) ||
        !(levelsIdentity == DirectoryIdentity::read(levelsDirectory)))
      throw Flash::Error("Native import destination directories changed.");
    for (auto directory = sourceDirectories.cbegin();
         directory != sourceDirectories.cend(); ++directory)
      if (!(directory.value() == DirectoryIdentity::read(directory.key())))
        throw Flash::Error("Source snapshot directory changed.");
  };
  buildOptions.assetDirectory = levelsDirectory;
  progress(options, "Preserving Flash source", 0, 1000);
  preserveSource(snapshot, staging.path(), options, sourceDirectories,
                 checkStaging);
  progress(options, "Verifying preserved Flash source");
  verifySourceSnapshot(snapshot, staging.path(), options, checkStaging);
  // Account for every XML member in the verified immutable copy. Keep the
  // bounded inventory outside the already large native import report.
  QJsonObject fieldAccounting;
  {
    Flash::Package package(QDir(staging.path()).filePath(snapshot.entry),
                           options.limits);
    fieldAccounting = Flash::accountXmlFields(package);
  }
  const QByteArray fieldAccountingBytes =
      QJsonDocument(fieldAccounting).toJson(QJsonDocument::Compact);
  const QByteArray fieldAccountingSha256 =
      QCryptographicHash::hash(fieldAccountingBytes, QCryptographicHash::Sha256)
          .toHex();
  const QString fieldAccountingPath =
      QDir(staging.path()).filePath("field-accounting.json");
  checkStaging();
  saveFile(fieldAccountingPath, fieldAccountingBytes,
           "Cannot save Flash XML field accounting: ");
  // Native construction and its compatibility report use the same immutable
  // bytes retained in the bundle, rather than reopening mutable source files.
  auto imported = [&] {
    Flash::Package package(QDir(staging.path()).filePath(snapshot.entry),
                           options.limits);
    // The snapshot was verified byte-identical to the checked original, so
    // the preflight above applies; the builder repeats its own guards.
    info = Flash::inspect(package);
    progress(options, "Building editable scene");
    checkStaging();
    return buildScene(package, info, buildOptions);
  }();
  // Close the retained archive handle before renaming its parent directory;
  // Windows sharing modes must not prevent committing the complete bundle.
  const TFilePath stagedScene =
      nativePath(QDir(staging.path()).filePath("scene.tnz"));
  imported.scene->setScenePath(stagedScene);
  auto *levelSet = imported.scene->getLevelSet();
  QVector<TXshSimpleLevel *> drawings;
  QVector<TXshSoundLevel *> sounds;
  quint64 frameCount = 0;
  for (int index = 0; index < levelSet->getLevelCount(); ++index) {
    if (auto *sound = levelSet->getLevel(index)->getSoundLevel()) {
      // Sounds are written beside the drawings, as WAV files.
      const QString filename =
          QFileInfo(QString::fromStdWString(sound->getPath().getWideString()))
              .fileName();
      sound->setPath(nativePath("$scenefolder/levels/" + filename));
      sounds.push_back(sound);
      continue;
    }
    auto *drawing = levelSet->getLevel(index)->getSimpleLevel();
    if (!drawing) continue;
    const TFilePath absolute = drawing->getPath();
    const QString filename =
        QFileInfo(QString::fromStdWString(absolute.getWideString())).fileName();
    // Changing only the persisted alias must retain the newly built frames.
    // The default setPath() clears them and attempts to load the future file.
    drawing->setPath(nativePath("$scenefolder/levels/" + filename), true);
    drawings.push_back(drawing);
    std::vector<TFrameId> frames;
    drawing->getFids(frames);
    if (frames.empty())
      throw Flash::Error("Native imported drawing has no frames to save.");
    // Decoded video frames are already written; they are only verified.
    if (!imported.videoLevels.contains(drawing)) frameCount += frames.size();
  }
  if (frameCount > quint64(std::numeric_limits<int>::max()))
    throw Flash::Error("Native drawing frame count exceeds progress limits.");
  int savedFrames = 0;
  QJsonArray resources;
  QJsonArray persistenceDifferences;
  std::vector<LevelCheck> levelChecks;
  for (auto *drawing : drawings) {
    checkStaging();
    std::vector<TFrameId> frames;
    drawing->getFids(frames);
    const auto path = imported.scene->decodeFilePath(drawing->getPath());
    // Reuse the native writer, with cancellation between frames. PLI writers
    // flush on destruction; verifyLevel also catches their swallowed errors.
    const bool video = imported.videoLevels.contains(drawing);
    if (!video) {
      TLevelWriterP writer(path);
      writer->setCreator(
          QString::fromStdString(TEnv::getApplicationFullName()));
      if (drawing->getType() == PLI_XSHLEVEL)
        writer->setPalette(drawing->getPalette());
      writer->setFrameRate(info.frameRate);
      for (const auto &frame : frames) {
        progress(options, "Saving native drawings", savedFrames,
                 int(frameCount));
        checkStaging();
        const TImageP image = drawing->getFrame(frame, true);
        if (drawing->getType() == PLI_XSHLEVEL ? !TVectorImageP(image)
                                               : !TRasterImageP(image))
          throw Flash::Error("Native imported drawing frame is missing.");
        writer->getFrameWriter(frame)->save(image);
        ++savedFrames;
      }
    }
    LevelCheck levelCheck;
    levelCheck.path     = path;
    levelCheck.frames   = frames;
    levelCheck.raster   = drawing->getType() == OVL_XSHLEVEL;
    levelCheck.name     = QString::fromStdWString(drawing->getName());
    const auto auditSet = imported.vectorFrameAudits.constFind(drawing);
    if (auditSet != imported.vectorFrameAudits.cend())
      levelCheck.audits = auditSet.value();
    if (!levelCheck.raster)
      for (const auto &frame : frames)
        levelCheck.originals.insert(
            frame.getNumber(), TVectorImageP(drawing->getFrame(frame, true)));
    // About eight frames per check keeps every core busy on big levels.
    constexpr int framesPerCheck = 8;
    for (int first = 0; first < int(frames.size()); first += framesPerCheck) {
      LevelCheck chunk = levelCheck;
      chunk.begin      = first;
      chunk.end        = std::min(int(frames.size()), first + framesPerCheck);
      levelChecks.push_back(std::move(chunk));
    }
    QJsonObject resource;
    resource["path"] =
        QDir(staging.path())
            .relativeFilePath(QString::fromStdWString(path.getWideString()));
    resource["frames"] = int(frames.size());
    if (video) resource["kind"] = "video";
    resources.append(resource);
  }
  for (auto *sound : sounds) {
    checkStaging();
    const auto path = imported.scene->decodeFilePath(sound->getPath());
    resources.append(saveSound(
        sound, path,
        QDir(staging.path())
            .relativeFilePath(QString::fromStdWString(path.getWideString()))));
  }
  progress(options, "Saving native drawings", savedFrames, int(frameCount));
  {
    // Reopen and check every saved level on all cores. Only this thread
    // calls back into the application (progress and cancellation).
    const int total = int(levelChecks.size());
    std::atomic<int> next{0}, done{0};
    std::atomic<bool> stop{false};
    const auto cancelled = [&stop] {
      if (stop) throw Flash::Error("Flash import cancelled.");
    };
    std::vector<std::thread> threads;
    const int workers =
        std::max(1, std::min({QThread::idealThreadCount(), 16, total}));
    for (int index = 0; index < workers && total; ++index)
      threads.emplace_back([&] {
        for (int job; (job = next++) < total && !stop;) {
          try {
            verifyLevel(levelChecks[job], cancelled, options.bestEffort);
          } catch (const std::exception &error) {
            levelChecks[job].error = QString::fromUtf8(error.what());
          } catch (...) {
            levelChecks[job].error = "Unexpected drawing verification failure.";
          }
          ++done;
        }
      });
    try {
      while (done < total) {
        progress(options, "Verifying native drawings", done, total);
        checkStaging();
        std::this_thread::sleep_for(std::chrono::milliseconds(30));
      }
    } catch (...) {
      stop = true;
      for (auto &thread : threads) thread.join();
      throw;
    }
    for (auto &thread : threads) thread.join();
    progress(options, "Verifying native drawings", total, total);
    for (const auto &levelCheck : levelChecks) {
      if (!levelCheck.error.isEmpty()) throw Flash::Error(levelCheck.error);
      for (const auto &difference : levelCheck.differences)
        persistenceDifferences.append(difference);
    }
    levelChecks.clear();
  }
  progress(options, "Saving native scene");
  checkStaging();
  imported.scene->save(stagedScene, nullptr, false);
  progress(options, "Verifying native scene");
  checkStaging();
  QJsonObject fieldBindings;
  {
    ToonzScene verified;
    verified.setScenePath(stagedScene);
    verified.loadTnzFile(stagedScene);
    if (verified.getLevelSet()->getLevelCount() != levelSet->getLevelCount() ||
        verified.getTopXsheet()->getFrameCount() !=
            imported.scene->getTopXsheet()->getFrameCount())
      throw Flash::Error("Native scene could not be reopened completely.");
    for (int index = 0; index < verified.getLevelSet()->getLevelCount();
         ++index) {
      checkCancelled(options);
      if (auto *drawing =
              verified.getLevelSet()->getLevel(index)->getSimpleLevel()) {
        const auto path =
            QString::fromStdWString(drawing->getPath().getWideString());
        if (!path.startsWith("$scenefolder/levels/") &&
            !path.startsWith("$scenefolder\\levels\\"))
          throw Flash::Error(
              "Native scene contains a nonportable drawing path.");
      }
    }
    verifySounds(verified, *imported.scene, options);
    Flash::Package package(QDir(staging.path()).filePath(snapshot.entry),
                           options.limits);
    fieldBindings = verifiedGraphicFieldBindings(
        package, fieldAccounting, QString::fromLatin1(fieldAccountingSha256),
        verified, imported.nativeSourceMap);
  }
  const QByteArray fieldBindingsBytes =
      QJsonDocument(fieldBindings).toJson(QJsonDocument::Compact);
  const QByteArray fieldBindingsSha256 =
      QCryptographicHash::hash(fieldBindingsBytes, QCryptographicHash::Sha256)
          .toHex();
  const QString fieldBindingsPath =
      QDir(staging.path()).filePath("field-bindings.json");
  checkStaging();
  saveFile(fieldBindingsPath, fieldBindingsBytes,
           "Cannot save Flash field bindings: ");
  // The raw builder snapshot retains every structural issue. Persist only the
  // bounded source view from preflight, alongside its full issue counts.
  QJsonObject report            = preflight.value("source").toObject();
  report["nativeCells"]         = double(imported.cells);
  report["nativeColumns"]       = imported.columns;
  report["nativeDrawingLevels"] = imported.drawingLevels;
  report["nativeChildLevels"]   = imported.childLevels;
  for (const char *key :
       {"nativeSoundLevels", "nativeSoundColumns", "nativeSounds",
        "nativeMovieClipSounds", "nativeBakedSoundEnvelopes",
        "nativeVideoLevels", "nativeVideoFrames", "nativeShapeTweens"})
    report[key] = imported.sourceReport.value(key);
  report["nativePaintWitnesses"]  = double(imported.paintWitnesses);
  report["nativeSourceMap"]       = imported.nativeSourceMap;
  report["nativeSourceMapCount"]  = imported.nativeSourceMap.size();
  report["sourceIssueCount"]      = preflight.value("sourceIssueCount");
  report["sourceIssuesTruncated"] = preflight.value("sourceIssuesTruncated");
  report["schemaVersion"]         = 1;
  report["preflight"]             = preflight;
  report["bestEffort"]            = options.bestEffort;
  report["buildProfileMs"]        = imported.profileMs;
  report["omittedCount"]          = double(imported.omitted);
  {
    // Save/reopen differences are approximations found after conversion.
    QJsonArray degradations = imported.degradations;
    QJsonObject counts      = imported.degradationCounts;
    for (int index = 0; index < std::min(100, persistenceDifferences.size());
         ++index)
      degradations.append(persistenceDifferences[index]);
    if (!persistenceDifferences.isEmpty())
      counts["pli-persistence-difference"] =
          double(persistenceDifferences.size());
    report["degradations"]      = degradations;
    report["degradationCounts"] = counts;
    report["approximatedCount"] =
        double(imported.approximated + persistenceDifferences.size());
  }
  report["importedAtUtc"] =
      QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs);
  report["sourcePath"] = QFileInfo(sourcePath).canonicalFilePath();
  report["sourceKind"] = snapshot.uncompressed ? "uncompressed-xfl" : "fla-zip";
  report["sourceSnapshot"] = QJsonObject{
      {"schemaVersion", 1},
      {"manifest", "source/manifest.json"},
      {"manifestSha256", QString::fromLatin1(snapshot.manifestSha256)},
      {"entry", snapshot.entry},
      {"kind", snapshot.manifest.value("kind")},
      {"totalBytes", snapshot.manifest.value("totalBytes")},
      {"payloadFileCount", snapshot.files.size()},
      {"packageMemberCount", snapshot.members.size()}};
  report["fieldAccounting"] = QJsonObject{
      {"schemaVersion", 2},
      {"path", "field-accounting.json"},
      {"sha256", QString::fromLatin1(fieldAccountingSha256)},
      {"bytes", fieldAccountingBytes.size()},
      {"xmlMemberCount", fieldAccounting.value("xmlMemberCount")},
      {"elementCount", fieldAccounting.value("elementCount")},
      {"attributeCount", fieldAccounting.value("attributeCount")},
      {"characterRunCount", fieldAccounting.value("characterRunCount")},
      {"cdataRunCount", fieldAccounting.value("cdataRunCount")},
      {"commentCount", fieldAccounting.value("commentCount")},
      {"processingInstructionCount",
       fieldAccounting.value("processingInstructionCount")},
      {"namespaceDeclarationCount",
       fieldAccounting.value("namespaceDeclarationCount")}};
  report["fieldBindings"] = QJsonObject{
      {"schemaVersion", 2},
      {"path", "field-bindings.json"},
      {"sha256", QString::fromLatin1(fieldBindingsSha256)},
      {"bytes", fieldBindingsBytes.size()},
      {"bindingCount", fieldBindings.value("bindingCount")},
      {"graphicInstanceCount", fieldBindings.value("graphicInstanceCount")},
      {"frameLabelCount", fieldBindings.value("frameLabelCount")},
      {"truncated", fieldBindings.value("truncated")}};
  report["scene"]                = "scene.tnz";
  report["resources"]            = resources;
  report["dpi"]                  = options.dpi;
  report["includeUnusedSymbols"] = options.includeUnusedSymbols;
  // The final scene save adds the versioned source-bundle reference.
  report["nativeSceneVersion"] = "71.4";
  progress(options, "Writing import report");
  checkStaging();
  const auto reportBytes =
      QJsonDocument(report).toJson(QJsonDocument::Indented);
  saveFile(QDir(staging.path()).filePath("import-report.json"), reportBytes,
           "Cannot save native import report: ");
  // The first scene save was reopened and verified above. Once the immutable
  // report exists, persist its exact digest in the scene and verify reopen.
  ToonzScene::FlashSourceBundle sourceReference;
  sourceReference.reportPath = "import-report.json";
  sourceReference.reportSha256 =
      QCryptographicHash::hash(reportBytes, QCryptographicHash::Sha256)
          .toHex()
          .toStdString();
  sourceReference.manifestSha256 = snapshot.manifestSha256.toStdString();
  imported.scene->setFlashSourceBundle(sourceReference);
  checkStaging();
  if (!QFile::remove(stagedScene.getQString()))
    throw Flash::Error(
        "Cannot replace staged scene with its source reference.");
  imported.scene->save(stagedScene, nullptr, false);
  {
    ToonzScene verified;
    verified.loadTnzFile(stagedScene);
    if (verified.flashSourceBundle().reportSha256 !=
            sourceReference.reportSha256 ||
        verified.flashSourceBundle().manifestSha256 !=
            sourceReference.manifestSha256)
      throw Flash::Error("Native scene lost its source bundle reference.");
    // Only the source-bundle reference changed since the bindings above were
    // verified on the first saved scene.
  }
  // Allocate the complete result before commit: no later error/cancellation
  // should turn a successfully installed bundle into a reported failed import.
  ImportResult result;
  result.scenePath  = nativePath(QDir(destination).filePath("scene.tnz"));
  result.reportPath = QDir(destination).filePath("import-report.json");
  result.report     = report;
  progress(options, "Finishing import");
  checkStaging();
  // This follows the final UI callback so source/snapshot tampering during
  // any precommit progress stage cannot install a mismatched provenance copy.
  verifySourceSnapshot(snapshot, staging.path(), options, checkStaging);
  if (hashFile(fieldAccountingPath, quint64(fieldAccountingBytes.size()),
               options, checkStaging) != fieldAccountingSha256)
    throw Flash::Error("Flash XML field accounting changed before commit.");
  if (hashFile(fieldBindingsPath, quint64(fieldBindingsBytes.size()), options,
               checkStaging) != fieldBindingsSha256)
    throw Flash::Error("Flash field bindings changed before commit.");
  if (occupied(destination))
    throw Flash::Error("Native import destination appeared before commit: " +
                       destination);
  // Release the in-memory native scene first: nothing of it may keep a file
  // inside the staging folder open during the rename.
  imported = BuildResult();
  renameWithoutReplace(staging, destination);
  staging.release();
  return result;
}

}  // namespace FlashNative
