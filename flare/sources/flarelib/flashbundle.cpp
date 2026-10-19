#include "flashbundle.h"
#include "imagebuilders.h"

#include "flare/imagemanager.h"
#include "flare/levelset.h"
#include "flare/hook.h"
#include "flare/sceneproperties.h"
#include "flare/tlog.h"
#include "flare/txshlevel.h"
#include "flare/txshleveltypes.h"
#include "flare/txshsimplelevel.h"
#include "tpalette.h"
#include "tlevel_io.h"
#include "toutputproperties.h"
#include "texception.h"
#include "trasterimage.h"
#include "tvectorimage.h"
#include "tenv.h"

#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonParseError>
#include <QSaveFile>
#include <QSet>
#include <QTemporaryDir>
#include <QUuid>
#include <cerrno>
#include <exception>
#include <vector>
#include <QDirIterator>
#ifdef Q_OS_WIN
#include <qt_windows.h>
#include <restartmanager.h>
#else
#if defined(Q_OS_LINUX)
#include <fcntl.h>
#include <linux/fs.h>
#include <sys/syscall.h>
#include <unistd.h>
#elif defined(Q_OS_MACOS)
#include <stdio.h>
#endif
#endif

namespace FlashNative {
namespace {

constexpr qint64 maxReportBytes          = 256ll * 1024 * 1024;
constexpr qint64 maxManifestBytes        = 64ll * 1024 * 1024;
constexpr qint64 maxFieldAccountingBytes = 16ll * 1024 * 1024;
constexpr qint64 maxFieldBindingsBytes   = 16ll * 1024 * 1024;
// Match ImportOptions' upper bound; copying and hashing still use fixed-size
// blocks and the manifest's exact recorded byte count.
constexpr quint64 maxSnapshotBytes = 9007199254740991ull;
constexpr int maxSnapshotFiles     = 100000;

void fail(const char *message) { throw TException(message); }

bool sha256(const QString &value) {
  if (value.size() != 64) return false;
  for (const QChar c : value)
    if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'))) return false;
  return true;
}

bool safeRelative(const QString &path) {
  if (path.isEmpty() || path.size() > 4096 || path.contains('\\') ||
      path.contains(':') || path.contains(QChar(0)) ||
      QDir::isAbsolutePath(path))
    return false;
  for (const auto &part : path.split('/'))
    if (part.isEmpty() || part == "." || part == ".." || part.size() > 255)
      return false;
  return true;
}

bool linkedPath(const QFileInfo &info) {
  if (info.isSymLink()) return true;
#ifdef Q_OS_WIN
  const QString native = QDir::toNativeSeparators(info.absoluteFilePath());
  const DWORD attributes =
      GetFileAttributesW(reinterpret_cast<LPCWSTR>(native.utf16()));
  return attributes != INVALID_FILE_ATTRIBUTES &&
         (attributes & FILE_ATTRIBUTE_REPARSE_POINT);
#else
  return false;
#endif
}

QString checkedPath(const QString &root, const QString &relative) {
  if (!safeRelative(relative)) fail("Unsafe Flash source bundle path.");
  QFileInfo base(root);
  if (!base.isDir() || linkedPath(base))
    fail("Flash source bundle folder is missing or linked.");
  const QString canonicalRoot = base.canonicalFilePath();
  if (canonicalRoot.isEmpty())
    fail("Cannot resolve Flash source bundle folder.");
  QString current = canonicalRoot;
  for (const auto &part : relative.split('/')) {
    current = QDir(current).filePath(part);
    QFileInfo info(current);
    if (!info.exists() || linkedPath(info))
      fail("Flash source bundle member is missing or linked.");
  }
  const QString canonical = QFileInfo(current).canonicalFilePath();
  const QString within    = QDir(canonicalRoot).relativeFilePath(canonical);
  if (canonical.isEmpty() || within == ".." || within.startsWith("../") ||
      QDir::isAbsolutePath(within))
    fail("Flash source bundle member escapes its folder.");
  return current;
}

void checkedNativeDrawing(const QString &sceneDirectory,
                          const QString &relative, bool requireFile) {
  if (!safeRelative(relative) || !relative.startsWith("levels/"))
    fail("Flash Save As drawing is outside its native bundle.");
  const QFileInfo source(QDir(sceneDirectory).filePath(relative));
  if (source.exists()) {
    if (!source.isFile()) fail("Flash Save As drawing source is not a file.");
    checkedPath(sceneDirectory, relative);
  } else {
    if (requireFile || linkedPath(source))
      fail("Flash Save As drawing source is missing or linked.");
    checkedPath(sceneDirectory, QFileInfo(relative).path());
  }
}

QByteArray readBounded(const QString &path, qint64 limit) {
  QFile file(path);
  if (!file.open(QIODevice::ReadOnly) || file.size() < 0 || file.size() > limit)
    fail("Flash source bundle metadata exceeds its byte budget.");
  const QByteArray data = file.read(limit + 1);
  if (data.size() != file.size() || file.error() != QFile::NoError)
    fail("Cannot read complete Flash source bundle metadata.");
  return data;
}

QJsonObject jsonObject(const QByteArray &bytes) {
  QJsonParseError error;
  const auto document = QJsonDocument::fromJson(bytes, &error);
  if (error.error != QJsonParseError::NoError || !document.isObject())
    fail("Invalid Flash source bundle metadata.");
  return document.object();
}

struct FileRecord {
  QString path;
  quint64 bytes = 0;
  QString digest;
};

quint64 checkedBytes(const QJsonValue &value, quint64 limit) {
  const double number = value.toDouble(-1);
  if (number < 0 || number > double(limit) || double(quint64(number)) != number)
    fail("Invalid Flash source bundle byte count.");
  return quint64(number);
}

QVector<FileRecord> fileRecords(const QJsonObject &manifest) {
  if (manifest.value("schemaVersion") != 1 ||
      manifest.value("hashAlgorithm") != "SHA-256")
    fail("Unsupported Flash source manifest.");
  const auto values = manifest.value("files").toArray();
  if (values.isEmpty() || values.size() > maxSnapshotFiles)
    fail("Invalid Flash source manifest file count.");
  const quint64 expectedTotal =
      checkedBytes(manifest.value("totalBytes"), maxSnapshotBytes);
  QVector<FileRecord> files;
  QSet<QString> seen;
  quint64 total = 0;
  files.reserve(values.size());
  for (const auto &value : values) {
    const auto object = value.toObject();
    FileRecord item;
    item.path   = object.value("path").toString();
    item.digest = object.value("sha256").toString();
    item.bytes  = checkedBytes(object.value("bytes"), maxSnapshotBytes);
    if (!safeRelative(item.path) || !item.path.startsWith("source/") ||
        item.path == "source/manifest.json" || !sha256(item.digest) ||
        seen.contains(item.path) || item.bytes > maxSnapshotBytes - total)
      fail("Invalid Flash source manifest member.");
    total += item.bytes;
    seen.insert(item.path);
    files.push_back(item);
  }
  if (total != expectedTotal)
    fail("Flash source manifest total does not match.");
  return files;
}

QString fileHash(const QString &path, quint64 expectedBytes) {
  QFile file(path);
  if (!file.open(QIODevice::ReadOnly) || file.size() < 0 ||
      quint64(file.size()) != expectedBytes)
    fail("Flash source bundle member size changed.");
  QCryptographicHash hash(QCryptographicHash::Sha256);
  quint64 total = 0;
  while (!file.atEnd()) {
    const QByteArray block = file.read(1024 * 1024);
    if (block.isEmpty() && !file.atEnd())
      fail("Cannot read Flash source bundle member.");
    if (quint64(block.size()) > expectedBytes - total)
      fail("Flash source bundle member grew while reading.");
    total += quint64(block.size());
    hash.addData(block);
  }
  if (total != expectedBytes || file.error() != QFile::NoError ||
      file.size() < 0 || quint64(file.size()) != expectedBytes)
    fail("Flash source bundle member changed while reading.");
  return QString::fromLatin1(hash.result().toHex());
}

void copyVerified(const QString &input, const QString &output, quint64 bytes,
                  const QString &digest) {
  QFile source(input);
  if (!source.open(QIODevice::ReadOnly) || source.size() < 0 ||
      quint64(source.size()) != bytes)
    fail("Cannot reopen verified Flash source member.");
  if (!QDir().mkpath(QFileInfo(output).absolutePath()))
    fail("Cannot create Flash Save As bundle folder.");
  QSaveFile target(output);
  if (!target.open(QIODevice::WriteOnly))
    fail("Cannot write Flash Save As bundle member.");
  QCryptographicHash hash(QCryptographicHash::Sha256);
  quint64 copied = 0;
  while (!source.atEnd()) {
    const QByteArray block = source.read(1024 * 1024);
    if (block.isEmpty() && !source.atEnd())
      fail("Cannot read Flash Save As bundle member.");
    if (quint64(block.size()) > bytes - copied ||
        target.write(block) != block.size())
      fail("Cannot copy complete Flash Save As bundle member.");
    hash.addData(block);
    copied += quint64(block.size());
  }
  if (copied != bytes || source.error() != QFile::NoError ||
      QString::fromLatin1(hash.result().toHex()) != digest || !target.commit())
    fail("Flash source member changed during Save As.");
}

// Atomic directory commit with no destination replacement. A filesystem that
// cannot provide this primitive fails the operation.
bool atomicDirectoryRenameNoReplace(const QString &source,
                                    const QString &destination) {
#ifdef Q_OS_WIN
  return MoveFileExW(reinterpret_cast<LPCWSTR>(source.utf16()),
                     reinterpret_cast<LPCWSTR>(destination.utf16()),
                     MOVEFILE_WRITE_THROUGH) != 0;
#elif defined(Q_OS_LINUX) && defined(SYS_renameat2)
  return ::syscall(SYS_renameat2, AT_FDCWD,
                   QFile::encodeName(source).constData(), AT_FDCWD,
                   QFile::encodeName(destination).constData(),
                   RENAME_NOREPLACE) == 0;
#elif defined(Q_OS_MACOS) && defined(RENAME_EXCL)
  return ::renamex_np(QFile::encodeName(source).constData(),
                      QFile::encodeName(destination).constData(),
                      RENAME_EXCL) == 0;
#else
  return false;
#endif
}
}  // namespace

#ifdef Q_OS_WIN
namespace {
// Names the processes holding files under a directory, through the Restart
// Manager, so a denied rename reports its cause. Loaded dynamically to keep
// the link line unchanged. Directory-only handles are not visible to it.
QString processesHoldingFiles(const QString &directory) {
  using StartSession = DWORD(WINAPI *)(DWORD *, DWORD, WCHAR *);
  using EndSession   = DWORD(WINAPI *)(DWORD);
  using Register     = DWORD(WINAPI *)(DWORD, UINT, LPCWSTR *, UINT,
                                   RM_UNIQUE_PROCESS *, UINT, LPCWSTR *);
  using GetList =
      DWORD(WINAPI *)(DWORD, UINT *, UINT *, RM_PROCESS_INFO *, LPDWORD);
  HMODULE module = LoadLibraryW(L"rstrtmgr.dll");
  if (!module) return QString();
  const auto start = reinterpret_cast<StartSession>(
      reinterpret_cast<void *>(GetProcAddress(module, "RmStartSession")));
  const auto end = reinterpret_cast<EndSession>(
      reinterpret_cast<void *>(GetProcAddress(module, "RmEndSession")));
  const auto add = reinterpret_cast<Register>(
      reinterpret_cast<void *>(GetProcAddress(module, "RmRegisterResources")));
  const auto list = reinterpret_cast<GetList>(
      reinterpret_cast<void *>(GetProcAddress(module, "RmGetList")));
  QStringList holders;
  DWORD session                     = 0;
  WCHAR key[CCH_RM_SESSION_KEY + 1] = {};
  if (start && end && add && list && start(&session, 0, key) == ERROR_SUCCESS) {
    std::vector<std::wstring> paths;
    QDirIterator it(directory, QDir::Files | QDir::Hidden | QDir::System,
                    QDirIterator::Subdirectories);
    while (it.hasNext() && paths.size() < 512)
      paths.push_back(QDir::toNativeSeparators(it.next()).toStdWString());
    std::vector<LPCWSTR> names;
    for (const auto &path : paths) names.push_back(path.c_str());
    UINT needed = 0, count = 0;
    DWORD reasons = 0;
    if (!names.empty() &&
        add(session, UINT(names.size()), names.data(), 0, nullptr, 0,
            nullptr) == ERROR_SUCCESS &&
        list(session, &needed, &count, nullptr, &reasons) == ERROR_MORE_DATA) {
      std::vector<RM_PROCESS_INFO> processes(needed);
      count = needed;
      if (list(session, &needed, &count, processes.data(), &reasons) ==
          ERROR_SUCCESS)
        for (UINT index = 0; index < count; ++index)
          holders << QString("%1 (pid %2)")
                         .arg(QString::fromWCharArray(
                             processes[index].strAppName))
                         .arg(processes[index].Process.dwProcessId);
    }
    end(session);
  }
  FreeLibrary(module);
  return holders.join(", ");
}
}  // namespace
#endif

void commitDirectoryNoReplaceWithRetry(
    const QString &source, const QString &destination, const QString &kind,
    const std::function<void()> &beforeAttempt) {
#ifdef Q_OS_WIN
  // Antivirus and search indexers open newly written files briefly; with
  // hundreds of drawings that can block a directory rename for seconds.
  constexpr DWORD backoffMs[] = {15,  30,  60,  100,  150,  250,
                                 400, 600, 800, 1000, 1500, 2000};
  DWORD systemError           = 0;
#else
  int systemError = 0;
#endif
  int attempts = 0;
  for (;;) {
    if (beforeAttempt) beforeAttempt();
    // A retry may only rename the same verified directory into a still-empty
    // slot. Never treat a newly occupied destination as a transient denial.
    const QFileInfo sourceInfo(source);
    const QFileInfo destinationInfo(destination);
    if (!sourceInfo.isDir() || linkedPath(sourceInfo))
      fail("Flash staging directory is missing or linked.");
    if (destinationInfo.exists() || linkedPath(destinationInfo))
      fail("Flash directory destination already exists or is linked.");
    ++attempts;
    if (atomicDirectoryRenameNoReplace(source, destination)) {
#ifdef Q_OS_WIN
      // Staging folders are hidden from shell views until committed.
      const std::wstring committed =
          QDir::toNativeSeparators(destination).toStdWString();
      const DWORD attributes = GetFileAttributesW(committed.c_str());
      if (attributes != INVALID_FILE_ATTRIBUTES &&
          (attributes & FILE_ATTRIBUTE_HIDDEN))
        SetFileAttributesW(committed.c_str(),
                           attributes & ~FILE_ATTRIBUTE_HIDDEN);
#endif
      return;
    }
#ifdef Q_OS_WIN
    // Capture immediately: even QFileInfo can overwrite GetLastError.
    systemError          = GetLastError();
    const bool transient = systemError == ERROR_ACCESS_DENIED ||
                           systemError == ERROR_SHARING_VIOLATION;
    if (!transient || attempts > int(sizeof(backoffMs) / sizeof(backoffMs[0])))
      break;
    Sleep(backoffMs[attempts - 1]);
#else
    systemError = errno;
    break;
#endif
  }
  const QFileInfo sourceInfo(source);
  const QFileInfo destinationInfo(destination);
  QString holders;
#ifdef Q_OS_WIN
  holders = processesHoldingFiles(source);
#endif
  const QString message =
      QString(
          "Cannot commit Flash %1 directory (system error %2; "
          "attempts=%3; source exists=%4 linked=%5; "
          "destination exists=%6 linked=%7).")
          .arg(kind)
          .arg(qulonglong(systemError))
          .arg(attempts)
          .arg(int(sourceInfo.exists()))
          .arg(int(linkedPath(sourceInfo)))
          .arg(int(destinationInfo.exists()))
          .arg(int(linkedPath(destinationInfo))) +
      (holders.isEmpty()
           ? QString(" No process holds a file inside it; a folder handle "
                     "(for example a file browser view) may be open.")
           : QString(" Files are held open by: %1.").arg(holders));
  throw TException(message.toStdString());
}

bool validSourceBundleReference(
    const ToonzScene::FlashSourceBundle &reference) {
  return safeRelative(QString::fromStdString(reference.reportPath)) &&
         sha256(QString::fromStdString(reference.reportSha256)) &&
         sha256(QString::fromStdString(reference.manifestSha256));
}

namespace {
// hashPayloads=false still validates the scene reference, report, manifest
// and member layout, but leaves payload hashing to a caller that streams
// (and hashes) every member anyway, such as the Save As copy.
ResolvedSourceBundle resolveBundle(const ToonzScene &scene, bool hashPayloads) {
  const auto &reference = scene.flashSourceBundle();
  if (!validSourceBundleReference(reference))
    fail("Scene has no valid Flash source bundle reference.");
  const QString sceneDir =
      QFileInfo(scene.decodeFilePath(scene.getScenePath()).getQString())
          .absolutePath();
  const QString reportPath =
      checkedPath(sceneDir, QString::fromStdString(reference.reportPath));
  const QByteArray reportBytes = readBounded(reportPath, maxReportBytes);
  const QString reportHash     = QString::fromLatin1(
          QCryptographicHash::hash(reportBytes, QCryptographicHash::Sha256)
              .toHex());
  if (reportHash != QString::fromStdString(reference.reportSha256))
    fail("Flash import report hash does not match its scene reference.");
  const QJsonObject report   = jsonObject(reportBytes);
  const QJsonObject snapshot = report.value("sourceSnapshot").toObject();
  if (report.value("schemaVersion") != 1 ||
      snapshot.value("schemaVersion") != 1 ||
      snapshot.value("manifest") != "source/manifest.json" ||
      snapshot.value("manifestSha256") !=
          QString::fromStdString(reference.manifestSha256))
    fail("Flash import report has an invalid source manifest reference.");
  const QString bundleDir    = QFileInfo(reportPath).absolutePath();
  const QString manifestPath = checkedPath(bundleDir, "source/manifest.json");
  const QByteArray manifestBytes = readBounded(manifestPath, maxManifestBytes);
  const QString manifestHash     = QString::fromLatin1(
          QCryptographicHash::hash(manifestBytes, QCryptographicHash::Sha256)
              .toHex());
  if (manifestHash != QString::fromStdString(reference.manifestSha256))
    fail("Flash source manifest hash does not match its scene reference.");
  const auto manifest = jsonObject(manifestBytes);
  const auto files    = fileRecords(manifest);
  if (snapshot.value("totalBytes").toDouble(-1) !=
          manifest.value("totalBytes").toDouble(-2) ||
      snapshot.value("payloadFileCount").toInt(-1) != files.size())
    fail("Flash source snapshot report and manifest disagree.");
  QSet<QString> names;
  for (const auto &file : files) {
    const QString member = checkedPath(bundleDir, file.path);
    if (hashPayloads && fileHash(member, file.bytes) != file.digest)
      fail("Flash source snapshot payload hash does not match.");
    names.insert(file.path);
  }
  const QString entry = snapshot.value("entry").toString();
  if (!safeRelative(entry) || !names.contains(entry) ||
      manifest.value("entry") != entry)
    fail("Flash source snapshot entry is missing from its manifest.");
  QString fieldAccountingPath;
  QJsonObject fieldAccounting;
  if (report.contains("fieldAccounting")) {
    const QJsonObject reference = report.value("fieldAccounting").toObject();
    const int accountingVersion = reference.value("schemaVersion").toInt();
    if ((accountingVersion != 1 && accountingVersion != 2) ||
        reference.value("path") != "field-accounting.json" ||
        !sha256(reference.value("sha256").toString()))
      fail("Invalid Flash XML field-accounting reference.");
    const quint64 expectedBytes =
        checkedBytes(reference.value("bytes"), maxFieldAccountingBytes);
    fieldAccountingPath = checkedPath(bundleDir, "field-accounting.json");
    const QByteArray bytes =
        readBounded(fieldAccountingPath, maxFieldAccountingBytes);
    if (quint64(bytes.size()) != expectedBytes ||
        QString::fromLatin1(
            QCryptographicHash::hash(bytes, QCryptographicHash::Sha256)
                .toHex()) != reference.value("sha256").toString())
      fail("Flash XML field-accounting bytes changed.");
    fieldAccounting = jsonObject(bytes);
    if (fieldAccounting.value("schemaVersion") != accountingVersion ||
        fieldAccounting.value("classifierVersion") != accountingVersion ||
        fieldAccounting.value("scope") !=
            (accountingVersion == 1 ? "all-package-xml-elements-and-attributes"
                                    : "all-package-xml-parser-records") ||
        fieldAccounting.value("nativeFidelityVerified") != false ||
        fieldAccounting.value("xmlMemberCount") !=
            reference.value("xmlMemberCount") ||
        fieldAccounting.value("elementCount") !=
            reference.value("elementCount") ||
        fieldAccounting.value("attributeCount") !=
            reference.value("attributeCount"))
      fail("Flash XML field-accounting reference and content disagree.");
    if (accountingVersion == 2 &&
        (fieldAccounting.value("lookupValueRepresentation") !=
             "xml-parser-normalized" ||
         fieldAccounting.value("exactLexicalAuthority") !=
             "retained-source-member-bytes" ||
         fieldAccounting.value("characterRunCount") !=
             reference.value("characterRunCount") ||
         fieldAccounting.value("cdataRunCount") !=
             reference.value("cdataRunCount") ||
         fieldAccounting.value("commentCount") !=
             reference.value("commentCount") ||
         fieldAccounting.value("processingInstructionCount") !=
             reference.value("processingInstructionCount") ||
         fieldAccounting.value("namespaceDeclarationCount") !=
             reference.value("namespaceDeclarationCount")))
      fail("Flash XML content accounting reference and content disagree.");
  }
  QString fieldBindingsPath;
  QJsonObject fieldBindings;
  if (report.contains("fieldBindings")) {
    const QJsonObject reference = report.value("fieldBindings").toObject();
    const int bindingVersion    = reference.value("schemaVersion").toInt(-1);
    if ((bindingVersion != 1 && bindingVersion != 2) ||
        reference.value("path") != "field-bindings.json" ||
        !sha256(reference.value("sha256").toString()) ||
        fieldAccountingPath.isEmpty())
      fail("Invalid Flash field-binding reference.");
    const quint64 expectedBytes =
        checkedBytes(reference.value("bytes"), maxFieldBindingsBytes);
    fieldBindingsPath = checkedPath(bundleDir, "field-bindings.json");
    const QByteArray bytes =
        readBounded(fieldBindingsPath, maxFieldBindingsBytes);
    if (quint64(bytes.size()) != expectedBytes ||
        QString::fromLatin1(
            QCryptographicHash::hash(bytes, QCryptographicHash::Sha256)
                .toHex()) != reference.value("sha256").toString())
      fail("Flash field-binding bytes changed.");
    fieldBindings             = jsonObject(bytes);
    const QJsonArray bindings = fieldBindings.value("bindings").toArray();
    if (fieldBindings.value("schemaVersion") != bindingVersion ||
        fieldBindings.value("scope") !=
            (bindingVersion == 1 ? "explicit-graphic-instance-fields-at-import"
                                 : "explicit-native-fields-at-import") ||
        fieldBindings.value("sourceAccountingSha256") !=
            report.value("fieldAccounting").toObject().value("sha256") ||
        fieldBindings.value("verification") !=
            (bindingVersion == 1
                 ? "source-member-hash-and-reopened-linked-native-instance"
                 : "source-member-hash-and-reopened-linked-native-object") ||
        fieldBindings.value("currentNativeEditFidelityVerified") != false ||
        fieldBindings.value("omittedDefaultsAreSourceFields") != false ||
        fieldBindings.value("bindingCount") != bindings.size() ||
        fieldBindings.value("bindingCount") !=
            reference.value("bindingCount") ||
        fieldBindings.value("graphicInstanceCount") !=
            reference.value("graphicInstanceCount") ||
        (bindingVersion == 2 &&
         (fieldBindings.value("frameLabelCount") !=
              reference.value("frameLabelCount") ||
          fieldBindings.value("examinedFrameLabelCount").toInt(-1) < 0)) ||
        fieldBindings.value("truncated") != reference.value("truncated"))
      fail("Flash field-binding reference and content disagree.");
  }
  return {reportPath,
          manifestPath,
          checkedPath(bundleDir, entry),
          report,
          fieldAccountingPath,
          fieldAccounting,
          fieldBindingsPath,
          fieldBindings};
}
}  // namespace

ResolvedSourceBundle resolveSourceBundle(const ToonzScene &scene) {
  return resolveBundle(scene, true);
}

namespace {

QString exceptionMessage() {
  try {
    throw;
  } catch (const TException &error) {
    return QString::fromStdWString(error.getMessage());
  } catch (const std::exception &error) {
    return QString::fromUtf8(error.what());
  } catch (...) {
    return "Unknown error.";
  }
}

QString sceneDirectoryOf(const ToonzScene &scene, const TFilePath &scenePath) {
  return QFileInfo(scene.decodeFilePath(scenePath).getQString()).absolutePath();
}

bool sameDirectory(const QString &a, const QString &b) {
  const QString ca = QFileInfo(a).canonicalFilePath();
  const QString cb = QFileInfo(b).canonicalFilePath();
  if (!ca.isEmpty() && !cb.isEmpty()) return ca == cb;
  return QDir::cleanPath(a) == QDir::cleanPath(b);
}

// Copies the immutable source/report next to the new scene. Payload bytes are
// hashed while they stream, so each member is read exactly once.
ToonzScene::FlashSourceBundle copySourceBundle(const ToonzScene &scene,
                                               const QString &newDir,
                                               const TFilePath &newScenePath) {
  const auto original = scene.flashSourceBundle();
  const auto resolved = resolveBundle(scene, false);
  if (!QDir().mkpath(newDir)) fail("Cannot create Flash Save As folder.");
  const QFileInfo destinationParent(newDir);
  if (!destinationParent.isDir() || linkedPath(destinationParent))
    fail("Flash Save As folder is missing or linked.");
  const QString name = QFileInfo(newScenePath.getQString()).completeBaseName() +
                       ".flash-source-" +
                       QUuid::createUuid().toString(QUuid::WithoutBraces);
  if (!safeRelative(name)) fail("Unsafe Flash Save As bundle name.");
  const QString destination = QDir(newDir).filePath(name);
  if (QFileInfo(destination).exists() || QFileInfo(destination).isSymLink())
    fail("Flash Save As bundle destination already exists.");
  const auto manifest =
      jsonObject(readBounded(resolved.manifestPath, maxManifestBytes));
  const auto files = fileRecords(manifest);
  // Auto-removed on any failure, so an aborted copy leaves no directory.
  QTemporaryDir staging(QDir(newDir).filePath(".flash-source-XXXXXX"));
  if (!staging.isValid()) fail("Cannot stage Flash Save As source bundle.");
  const QString sourceRoot = QFileInfo(resolved.reportPath).absolutePath();
  copyVerified(resolved.reportPath,
               QDir(staging.path()).filePath("import-report.json"),
               quint64(QFileInfo(resolved.reportPath).size()),
               QString::fromStdString(original.reportSha256));
  if (!resolved.fieldAccountingPath.isEmpty()) {
    const QJsonObject reference =
        resolved.report.value("fieldAccounting").toObject();
    copyVerified(
        resolved.fieldAccountingPath,
        QDir(staging.path()).filePath("field-accounting.json"),
        checkedBytes(reference.value("bytes"), maxFieldAccountingBytes),
        reference.value("sha256").toString());
  }
  if (!resolved.fieldBindingsPath.isEmpty()) {
    const QJsonObject reference =
        resolved.report.value("fieldBindings").toObject();
    copyVerified(resolved.fieldBindingsPath,
                 QDir(staging.path()).filePath("field-bindings.json"),
                 checkedBytes(reference.value("bytes"), maxFieldBindingsBytes),
                 reference.value("sha256").toString());
  }
  copyVerified(resolved.manifestPath,
               QDir(staging.path()).filePath("source/manifest.json"),
               quint64(QFileInfo(resolved.manifestPath).size()),
               QString::fromStdString(original.manifestSha256));
  for (const auto &file : files)
    copyVerified(checkedPath(sourceRoot, file.path),
                 QDir(staging.path()).filePath(file.path), file.bytes,
                 file.digest);
  ToonzScene::FlashSourceBundle copied = original;
  copied.reportPath = (name + "/import-report.json").toStdString();
  commitDirectoryNoReplaceWithRetry(staging.path(), destination,
                                    "source bundle");
  staging.setAutoRemove(false);
  return copied;
}

}  // namespace

ToonzScene::FlashSourceBundle prepareSourceBundleSave(
    const ToonzScene &scene, const TFilePath &oldScenePath,
    const TFilePath &newScenePath, QString *warning) {
  const auto original = scene.flashSourceBundle();
  if (original.empty()) return original;
  QString message;
  try {
    if (!validSourceBundleReference(original)) {
      message = "The Flash source bundle reference is invalid.";
    } else {
      const QString oldDir = sceneDirectoryOf(scene, oldScenePath);
      const QString newDir = sceneDirectoryOf(scene, newScenePath);
      if (sameDirectory(oldDir, newDir)) {
        // An ordinary save never reads or hashes the bundle. Only a missing
        // report is noticed (one stat); the reference is then dropped.
        if (QFileInfo(QDir(oldDir).filePath(
                          QString::fromStdString(original.reportPath)))
                .isFile())
          return original;
        message = "The Flash source bundle is missing.";
      } else {
        return copySourceBundle(scene, newDir, newScenePath);
      }
    }
  } catch (...) {
    message = exceptionMessage();
  }
  message = QString(
                "Flash source bundle reference dropped while saving "
                "%1: %2")
                .arg(newScenePath.getQString(), message);
  TSysLog::warning(message.toStdString());
  *warning = message;
  return ToonzScene::FlashSourceBundle();
}

void discardSourceBundleCopy(const ToonzScene::FlashSourceBundle &original,
                             const ToonzScene::FlashSourceBundle &saved,
                             const QString &newSceneDirectory) {
  if (saved.empty() || saved.reportPath == original.reportPath) return;
  const QString report = QString::fromStdString(saved.reportPath);
  const QString folder = report.section('/', 0, 0);
  if (!safeRelative(report) || !folder.contains(".flash-source-") ||
      report != folder + "/import-report.json")
    return;
  const QFileInfo info(QDir(newSceneDirectory).filePath(folder));
  if (info.isDir() && !linkedPath(info))
    QDir(info.absoluteFilePath()).removeRecursively();
}

namespace {

struct PortableDrawing {
  TXshSimpleLevel *level;
  TFilePath original;
  TFilePath destination;
  TFilePath originalDecoded;
  TFilePath destinationDecoded;
  std::vector<TFrameId> frames;
  // Only edited frames are retained (they are resident in the image cache
  // anyway); clean frames are reloaded lazily from the written copy.
  std::vector<std::pair<size_t, TImageP>> edited;
};

// A drawing is relocated by the portable path only when it is a Flash-imported
// PLI/PNG level stored inside the old scene's levels folder. Everything else
// goes through the standard Save As handling untouched.
bool portableDrawing(const ToonzScene &scene, TXshLevel *level,
                     const QString &originalSceneDirectory,
                     PortableDrawing &record) {
  if (!level || level->getChildLevel() || level->flashNativeId().isEmpty())
    return false;
  auto *drawing = level->getSimpleLevel();
  if (!drawing) return false;
  const bool vector = drawing->getType() == PLI_XSHLEVEL &&
                      drawing->getPath().getType() == "pli";
  const bool bitmap = drawing->getType() == OVL_XSHLEVEL &&
                      drawing->getPath().getType() == "png";
  if ((!vector && !bitmap) || !drawing->getScannedPath().isEmpty() ||
      (vector && (!drawing->getPalette() ||
                  !drawing->getPalette()->getRefImgPath().isEmpty())) ||
      (drawing->getHookSet() && drawing->getHookSet()->getHookCount() > 0))
    return false;
  QString original = drawing->getPath().getQString();
  original.replace('\\', '/');
  if (!original.startsWith("$scenefolder/levels/")) return false;
  const QString relative = original.mid(QString("$scenefolder/").size());
  if (!safeRelative(relative)) return false;
  std::vector<TFrameId> frames;
  drawing->getFids(frames);
  if (frames.empty()) return false;
  try {
    checkedNativeDrawing(originalSceneDirectory, relative, vector);
    if (bitmap) {
      const TFilePath decoded = scene.decodeFilePath(drawing->getPath());
      for (const auto &frame : frames) {
        const QString numbered =
            QDir(originalSceneDirectory)
                .relativeFilePath(decoded.withFrame(frame).getQString());
        checkedNativeDrawing(
            originalSceneDirectory, numbered,
            !ImageManager::instance()->isModified(drawing->getImageId(frame)));
      }
    }
  } catch (...) {
    return false;
  }
  record.level           = drawing;
  record.original        = drawing->getPath();
  record.originalDecoded = scene.decodeFilePath(drawing->getPath());
  record.frames          = std::move(frames);
  return true;
}

std::vector<PortableDrawing> portableDrawings(const ToonzScene &scene) {
  const QString directory = sceneDirectoryOf(scene, scene.getScenePath());
  std::vector<TXshLevel *> levels;
  scene.getLevelSet()->listLevels(levels);
  std::vector<PortableDrawing> drawings;
  for (auto *level : levels) {
    PortableDrawing record{};
    if (portableDrawing(scene, level, directory, record))
      drawings.push_back(std::move(record));
  }
  return drawings;
}

}  // namespace

std::vector<TXshLevel *> portableSaveAsLevels(const ToonzScene &scene) {
  std::vector<TXshLevel *> levels;
  for (const auto &drawing : portableDrawings(scene))
    levels.push_back(drawing.level);
  return levels;
}

void saveImportedSceneAs(ToonzScene &scene, const TFilePath &newScenePath,
                         bool saveSceneIcon) {
  if (newScenePath.getType() != "tnz")
    fail("Portable Flash Save As requires a native scene path.");
  const QString originalSceneDirectory =
      sceneDirectoryOf(scene, scene.getScenePath());
  const QString sceneDirectory = sceneDirectoryOf(scene, newScenePath);
  std::vector<PortableDrawing> drawings;
  if (!sameDirectory(originalSceneDirectory, sceneDirectory))
    drawings = portableDrawings(scene);
  // Nothing Flash-specific to relocate: this is a standard Save As.
  if (drawings.empty()) {
    scene.save(newScenePath, nullptr, saveSceneIcon);
    return;
  }
  const QFileInfo sceneFolder(sceneDirectory);
  if (!sceneFolder.isDir() || linkedPath(sceneFolder))
    fail("Flash Save As destination folder is missing or linked.");
  const QString levelsDirectory = QDir(sceneDirectory).filePath("levels");
  const QFileInfo levelsFolder(levelsDirectory);
  if (levelsFolder.exists() &&
      (!levelsFolder.isDir() || linkedPath(levelsFolder)))
    fail("Flash Save As native levels folder is invalid or linked.");
  const bool createdLevels = !levelsFolder.exists();
  const QString folderName =
      QFileInfo(newScenePath.getQString()).completeBaseName() +
      ".flash-native-" + QUuid::createUuid().toString(QUuid::WithoutBraces);
  if (!safeRelative(folderName)) fail("Unsafe Flash Save As drawing folder.");
  const QString committed = QDir(levelsDirectory).filePath(folderName);
  bool folderCommitted    = false;
  // Leave neither the drawing folder nor a levels folder we created behind.
  auto removeOutput = [&] {
    if (folderCommitted) QDir(committed).removeRecursively();
    if (createdLevels) QDir().rmdir(levelsDirectory);
  };

  try {
    if (!QDir().mkpath(levelsDirectory))
      fail("Cannot prepare Flash Save As native levels folder.");
    if (QFileInfo(committed).exists() || QFileInfo(committed).isSymLink())
      fail("Flash Save As drawing destination already exists.");
    QTemporaryDir staging(
        QDir(levelsDirectory).filePath(".flash-native-XXXXXX"));
    if (!staging.isValid()) fail("Cannot stage Flash Save As native drawings.");

    for (size_t index = 0; index < drawings.size(); ++index) {
      auto &drawing     = drawings[index];
      const bool vector = drawing.level->getType() == PLI_XSHLEVEL;
      const QString file =
          QString("drawing-%1%2").arg(index + 1).arg(vector ? ".pli" : "..png");
      const QString staged = QDir(staging.path()).filePath(file);
      {
        TLevelWriterP writer(TFilePath(staged.toStdWString()));
        writer->setCreator(
            QString::fromStdString(TEnv::getApplicationFullName()));
        writer->setFrameRate(
            scene.getProperties()->getOutputProperties()->getFrameRate());
        if (vector) writer->setPalette(drawing.level->getPalette());
        for (size_t f = 0; f < drawing.frames.size(); ++f) {
          const auto &frame   = drawing.frames[f];
          const bool modified = ImageManager::instance()->isModified(
              drawing.level->getImageId(frame));
          // Clean frames are streamed without entering the image cache.
          const TImageP image =
              modified
                  ? drawing.level->getFrame(frame, false)
                  : drawing.level->getFrame(
                        frame, static_cast<UCHAR>(ImageManager::dontPutInCache),
                        0);
          if (vector ? !TVectorImageP(image) : !TRasterImageP(image))
            fail("Flash Save As native drawing frame is missing.");
          writer->getFrameWriter(frame)->save(image);
          if (modified) drawing.edited.emplace_back(f, image);
        }
      }
      // PLI writers finalize on destruction. Reopen the level info before
      // committing so a swallowed writer error cannot produce a valid TNZ.
      // Each frame is decoded and released one at a time.
      TLevelReaderP reader(TFilePath(staged.toStdWString()));
      const auto saved = reader->loadInfo();
      if (!saved || saved->isPartialLoad() ||
          (vector && !saved->getPalette()) ||
          saved->getFrameCount() != int(drawing.frames.size()))
        fail("Flash Save As native drawing could not be reopened.");
      auto cursor = saved->begin();
      for (const auto &frame : drawing.frames) {
        if (cursor == saved->end() || cursor->first != frame ||
            (vector ? !TVectorImageP(reader->getFrameReader(frame)->load())
                    : !TRasterImageP(reader->getFrameReader(frame)->load())))
          fail("Flash Save As native drawing frame could not be reopened.");
        ++cursor;
      }
      drawing.destination = TFilePath(
          ("$scenefolder/levels/" + folderName + "/" + file).toStdWString());
      drawing.destinationDecoded =
          TFilePath(QDir(committed).filePath(file).toStdWString());
    }
    commitDirectoryNoReplaceWithRetry(staging.path(), committed,
                                      "native drawings");
    staging.setAutoRemove(false);
    folderCommitted = true;
  } catch (...) {
    removeOutput();
    throw;
  }

  const auto originalScenePath = scene.getScenePath();
  const auto originalProject   = scene.getProject();
  auto rebind = [](const PortableDrawing &drawing, bool destination) {
    const TFilePath &path =
        destination ? drawing.destinationDecoded : drawing.originalDecoded;
    for (const auto &frame : drawing.frames)
      ImageManager::instance()->bind(drawing.level->getImageId(frame),
                                     new ImageLoader(path, frame));
    // Edited frames keep their in-memory image (and modified state).
    for (const auto &edited : drawing.edited)
      if (!ImageManager::instance()->setImage(
              drawing.level->getImageId(drawing.frames[edited.first]),
              edited.second))
        fail("Flash Save As could not retain an edited drawing frame.");
  };
  try {
    for (auto &drawing : drawings) {
      drawing.level->setPath(drawing.destination, true);
      rebind(drawing, true);
    }
    scene.save(newScenePath, nullptr, saveSceneIcon);
  } catch (...) {
    const auto saveFailure = std::current_exception();
    // Restore the scene's identity before any loader operation that might
    // itself fail; a failed rollback must never strand it at the new path.
    scene.setScenePath(originalScenePath, false);
    scene.setProject(originalProject);
    bool rollbackFailed = false;
    for (auto &drawing : drawings) {
      try {
        drawing.level->setPath(drawing.original, true);
        rebind(drawing, false);
      } catch (...) {
        rollbackFailed = true;
      }
    }
    // Loaders point at the original drawings again; the new copy is unused.
    if (!rollbackFailed) removeOutput();
    if (rollbackFailed)
      fail("Flash Save As failed and could not restore every drawing loader.");
    std::rethrow_exception(saveFailure);
  }
}

}  // namespace FlashNative
