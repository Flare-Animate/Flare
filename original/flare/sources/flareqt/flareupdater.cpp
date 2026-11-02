#include "flareqt/flareupdater.h"

#include <QCoreApplication>
#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QProcess>
#include <QSslSocket>
#include <QStringList>

#include <algorithm>

#ifdef Q_OS_WIN
#include <windows.h>
#endif

namespace {

//! Splits "v1.8.0-rc1" into its numeric head [1,8,0] and a pre-release marker.
//! \a hadDigits reports whether any digit was present at all, which is how the
//! caller tells a real version from unparseable junk.
void splitVersion(const QString& raw, QVector<int>* numbers, QString* suffix,
                  bool* hadDigits = nullptr) {
  QString v = raw.trimmed();
  if (v.startsWith(QLatin1Char('v'), Qt::CaseInsensitive))
    v.remove(0, 1);

  // A pre-release suffix starts at the first '-' or at a '+' (build metadata,
  // which does not affect precedence and is dropped along with the rest).
  //
  // QString::indexOf returns -1 when absent, so these cannot be combined with
  // std::min: min(5, -1) is -1, which silently discarded the '-' marker and made
  // "1.8.0-rc1" parse as the final 1.8.0. Compare the two hits explicitly.
  int cut = v.indexOf(QLatin1Char('-'));
  const int plus = v.indexOf(QLatin1Char('+'));
  if (cut < 0 || (plus >= 0 && plus < cut))
    cut = plus;

  if (cut >= 0) {
    *suffix = v.mid(cut + 1).trimmed().toLower();
    v       = v.left(cut);
  } else {
    suffix->clear();
  }

  numbers->clear();
  const QStringList parts = v.split(QLatin1Char('.'), Qt::SkipEmptyParts);
  for (const QString& p : parts) {
    bool ok     = false;
    const int n = p.toInt(&ok);
    if (ok && hadDigits)
      *hadDigits = true;
    (*numbers) << (ok ? n : 0);
  }
}

/*! Turns a network failure into something a user can act on.

    Qt reports every HTTPS problem as "TLS initialization failed" when it is
    actually the common case of the OpenSSL runtime DLLs not being deployed
    alongside the app -- a packaging fault, not a network fault, and not
    something retrying will fix. Naming it saves the user from wondering whether
    they are offline. */
QString describeNetworkFailure(QNetworkReply::NetworkError code,
                               const QString& qtMessage) {
  // Match on the message as well as the code: Qt reports a failure to bring the
  // TLS layer up at all with QNetworkReply::UnknownNetworkError, not one of the
  // SSL-specific codes, so checking codes alone missed exactly the case this is
  // meant to explain.
  const bool tls = (code == QNetworkReply::SslHandshakeFailedError ||
                    code == QNetworkReply::ProxyConnectionRefusedError ||
                    qtMessage.contains(QLatin1String("TLS initialization"),
                                       Qt::CaseInsensitive));

  if (tls && !QSslSocket::supportsSsl()) {
    return QStringLiteral(
        "This build cannot make secure connections because its encryption "
        "library is missing, so the update check cannot run. Reinstalling Flare "
        "from the official download restores it. (%1)")
        .arg(qtMessage);
  }
  if (code == QNetworkReply::HostNotFoundError ||
      code == QNetworkReply::RemoteHostClosedError) {
    return QStringLiteral("Could not reach the update server. (%1)")
        .arg(qtMessage);
  }
  return QStringLiteral("Could not reach the update server: %1").arg(qtMessage);
}

//! The file name to save a download as, taken from its URL path.
QString assetNameFromUrl(const QUrl& url) {
  const QString name = QFileInfo(url.path()).fileName();
  // A redirect to a CDN sometimes lands on an opaque path; fall back to
  // something identifiable rather than writing to an empty name.
  return name.isEmpty() ? QStringLiteral("flare-update.bin") : name;
}

}  // namespace

//==============================================================================
int FlareUpdater::compareVersions(const QString& a, const QString& b) {
  QVector<int> na, nb;
  QString sa, sb;
  bool aHasDigits = false, bHasDigits = false;
  splitVersion(a, &na, &sa, &aHasDigits);
  splitVersion(b, &nb, &sb, &bHasDigits);

  // Neither side contains a single digit: both are unparseable. Report them equal
  // rather than falling through to a suffix comparison, because falling through
  // made "not-a-version" look newer than "also-not" -- and a garbage tag must
  // never be the reason the user is told an update exists.
  if (!aHasDigits && !bHasDigits)
    return 0;

  const int n = qMax(na.size(), nb.size());
  for (int i = 0; i < n; ++i) {
    // A missing field is zero, so "1.8" and "1.8.0" compare equal.
    const int x = i < na.size() ? na.at(i) : 0;
    const int y = i < nb.size() ? nb.at(i) : 0;
    if (x != y)
      return x > y ? 1 : -1;
  }

  // Same numbers: an absent suffix is the final release and outranks any
  // pre-release of the same version.
  if (sa.isEmpty() && sb.isEmpty())
    return 0;
  if (sa.isEmpty())
    return 1;
  if (sb.isEmpty())
    return -1;

  // Both pre-releases: fall back to a plain string compare so rc2 > rc1, and
  // an arbitrary but stable answer when they are equal.
  if (sa == sb)
    return 0;
  return sa > sb ? 1 : -1;
}

//==============================================================================
bool FlareUpdater::isPreReleaseTag(const QString& tag) {
  QString suffix;
  QVector<int> numbers;
  splitVersion(tag, &numbers, &suffix);
  return !suffix.isEmpty();
}

//==============================================================================
FlareUpdater::Release FlareUpdater::parseReleaseJson(const QByteArray& json,
                                                     QString* error) {
  auto fail = [error](const char* why) {
    if (error)
      *error = QString::fromLatin1(why);
    return Release();
  };

  QJsonParseError parseError{};
  const QJsonDocument doc =
      QJsonDocument::fromJson(json, &parseError);
  if (parseError.error != QJsonParseError::NoError)
    return fail("release feed was not valid JSON");
  QJsonObject obj;
  if (doc.isArray()) {  // /releases list (nightly channel): newest non-draft
    for (const QJsonValue& v : doc.array())
      if (v.isObject() && !v.toObject().value(QStringLiteral("draft")).toBool()) {
        obj = v.toObject();
        break;
      }
  } else if (doc.isObject())
    obj = doc.object();
  else
    return fail("release feed was not a JSON object");

  Release release;
  release.tag       = obj.value(QStringLiteral("tag_name")).toString().trimmed();
  release.name      = obj.value(QStringLiteral("name")).toString().trimmed();
  release.notes     = obj.value(QStringLiteral("body")).toString();
  release.prerelease = obj.value(QStringLiteral("prerelease")).toBool(false);
  release.draft      = obj.value(QStringLiteral("draft")).toBool(false);

  if (release.tag.isEmpty())
    return fail("release has no tag_name");

  const QJsonArray assets = obj.value(QStringLiteral("assets")).toArray();
  for (const QJsonValue& v : assets) {
    const QJsonObject a = v.toObject();
    Asset asset;
    asset.name = a.value(QStringLiteral("name")).toString();
    asset.url  = QUrl(a.value(QStringLiteral("browser_download_url")).toString());
    const QString digest = a.value(QStringLiteral("digest")).toString();
    if (digest.startsWith(QStringLiteral("sha256:")))
      asset.sha256 = digest.mid(7).toLower();
    asset.size = static_cast<qint64>(
        a.value(QStringLiteral("size")).toDouble(0.0));
    if (asset.isValid())
      release.assets.append(asset);
  }

  if (error)
    error->clear();
  return release;
}

//==============================================================================
FlareUpdater::Asset FlareUpdater::pickAssetForCurrentPlatform(
    const QList<Asset>& assets, QString* why) {
  auto none = [why](const QString& reason) {
    if (why)
      *why = reason;
    return Asset();
  };

  if (assets.isEmpty())
    return none(QStringLiteral("the release has no downloadable files"));

  // Pass 1 requires the asset name to mention this platform, in a ranked order
  // so that a 64-bit build beats a 32-bit one when a release ships both. Pass 2
  // relaxes that (below) so a release shipping a single unlabelled binary still
  // installs instead of dead-ending the user with "no download for your
  // platform".
  QVector<QString> wanted;
#if defined(Q_OS_WIN)
  wanted << QStringLiteral("win64") << QStringLiteral("windows-x64")
         << QStringLiteral("win-x64") << QStringLiteral("win32")
         << QStringLiteral("windows");
  const QStringList extensions{QStringLiteral(".exe"), QStringLiteral(".msi")};
#elif defined(Q_OS_MAC)
  wanted << QStringLiteral("macos") << QStringLiteral("osx")
         << QStringLiteral("darwin") << QStringLiteral("mac");
  const QStringList extensions{QStringLiteral(".dmg")};
#else
  wanted << QStringLiteral("appimage") << QStringLiteral("x86_64")
         << QStringLiteral("amd64") << QStringLiteral("linux");
  const QStringList extensions{QStringLiteral(".appimage")};
#endif

  for (const QString& w : wanted) {
    for (const Asset& a : assets) {
      if (a.name.toLower().contains(w)) {
        if (why)
          why->clear();
        return a;
      }
    }
  }

  // No platform token anywhere: accept a single obvious binary for our
  // extension, but never guess between several.
  QList<Asset> byExtension;
  for (const Asset& a : assets) {
    const QString lower = a.name.toLower();
    for (const QString& ext : extensions)
      if (lower.endsWith(ext))
        byExtension.append(a);
  }
  if (byExtension.size() == 1) {
    if (why)
      why->clear();
    return byExtension.first();
  }

  if (why) {
    *why = byExtension.isEmpty()
               ? QStringLiteral(
                     "the release publishes no build for this platform")
               : QStringLiteral(
                     "the release publishes several builds for this platform "
                     "and none is labelled clearly enough to choose safely");
  }
  return Asset();
}

//==============================================================================
QString FlareUpdater::runningExecutableName() {
  const QString path =
      QCoreApplication::applicationFilePath();
  return QFileInfo(path).fileName();
}

//==============================================================================
bool FlareUpdater::secureConnectionsAvailable() {
  return QSslSocket::supportsSsl();
}

//==============================================================================
FlareUpdater::FlareUpdater(QObject* parent)
    : QObject(parent)
    , m_manager(new QNetworkAccessManager(this)) {}

FlareUpdater::~FlareUpdater() {
  // QObject parentage deletes the replies, but abort them first so a reply
  // still in flight cannot fire into a half-destroyed object.
  if (m_releaseReply)
    m_releaseReply->abort();
  if (m_downloadReply)
    m_downloadReply->abort();
}

//==============================================================================
void FlareUpdater::checkForRelease(const QUrl& releasesApiUrl) {
  if (m_releaseReply) {
    m_releaseReply->abort();
    m_releaseReply->deleteLater();
    m_releaseReply = nullptr;
  }

  QNetworkRequest request(releasesApiUrl);
  // GitHub rejects requests without a User-Agent, and an unauthenticated agent
  // is rate limited far more aggressively.
  request.setRawHeader("User-Agent", "Flare-Updater");
  request.setRawHeader("Accept", "application/vnd.github+json");
  request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
                       QNetworkRequest::NoLessSafeRedirectPolicy);

  m_releaseReply = m_manager->get(request);
  connect(m_releaseReply, &QNetworkReply::finished, this,
          &FlareUpdater::onReleaseFinished);
}

void FlareUpdater::onReleaseFinished() {
  QNetworkReply* reply = m_releaseReply;
  m_releaseReply       = nullptr;
  if (!reply)
    return;
  reply->deleteLater();

  if (reply->error() != QNetworkReply::NoError) {
    emit failed(describeNetworkFailure(reply->error(), reply->errorString()));
    return;
  }

  QString error;
  const Release release = parseReleaseJson(reply->readAll(), &error);
  if (!error.isEmpty()) {
    emit failed(error);
    return;
  }
  if (!release.isValid()) {
    emit failed(QStringLiteral("the release server sent no usable release"));
    return;
  }

  emit releaseReady(release);
}

//==============================================================================
void FlareUpdater::download(const Asset& asset, const QUrl& destinationDir) {
  if (!asset.isValid()) {
    emit failed(QStringLiteral("there is nothing to download"));
    return;
  }
  if (m_downloadReply) {
    m_downloadReply->abort();
    m_downloadReply->deleteLater();
    m_downloadReply = nullptr;
  }

  m_destinationDir = destinationDir;
  m_expectedSha256 = asset.sha256;

  QNetworkRequest request(asset.url);
  request.setRawHeader("User-Agent", "Flare-Updater");
  request.setRawHeader("Accept", "application/octet-stream");
  request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
                       QNetworkRequest::NoLessSafeRedirectPolicy);

  m_downloadReply = m_manager->get(request);
  connect(m_downloadReply, &QNetworkReply::downloadProgress, this,
          &FlareUpdater::downloadProgress);
  connect(m_downloadReply, &QNetworkReply::finished, this,
          &FlareUpdater::onDownloadFinished);
}

void FlareUpdater::onDownloadFinished() {
  QNetworkReply* reply = m_downloadReply;
  m_downloadReply       = nullptr;
  if (!reply)
    return;
  reply->deleteLater();

  if (reply->error() != QNetworkReply::NoError) {
    emit failed(QStringLiteral("the download failed: %1")
                    .arg(reply->errorString()));
    return;
  }

  const QString dir = m_destinationDir.isLocalFile()
                          ? m_destinationDir.toLocalFile()
                          : m_destinationDir.toString();
  if (!QDir().mkpath(dir)) {
    emit failed(QStringLiteral("could not create a place to save the update"));
    return;
  }

  // Write to a .part file and rename on success, so an interrupted download can
  // never be mistaken for a complete installer on the next run.
  const QString finalPath = QDir(dir).filePath(assetNameFromUrl(reply->url()));
  const QString partPath  = finalPath + QStringLiteral(".part");

  QFile file(partPath);
  if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
    emit failed(QStringLiteral("could not write the update to %1").arg(dir));
    return;
  }
  const QByteArray payload = reply->readAll();
  if (file.write(payload) != payload.size()) {
    file.close();
    QFile::remove(partPath);
    emit failed(QStringLiteral("the update download was incomplete"));
    return;
  }
  file.close();

  // Verify against GitHub's published digest before anything can run it.
  if (!m_expectedSha256.isEmpty() &&
      QCryptographicHash::hash(payload, QCryptographicHash::Sha256).toHex() !=
          m_expectedSha256.toLatin1()) {
    QFile::remove(partPath);
    emit failed(QStringLiteral("the update failed its checksum verification"));
    return;
  }

  QFile::remove(finalPath);
  if (!QFile::rename(partPath, finalPath)) {
    QFile::remove(partPath);
    emit failed(QStringLiteral("could not finish saving the update"));
    return;
  }

  emit downloadFinished(finalPath);
}

//==============================================================================
bool FlareUpdater::applyUpdate(const QString& archivePath, QString* error) {
  auto fail = [error](const QString& why) {
    if (error)
      *error = why;
    return false;
  };

  if (!QFileInfo::exists(archivePath))
    return fail(QStringLiteral("the downloaded update is missing"));

  const QString exe = QCoreApplication::applicationFilePath();

#ifdef Q_OS_WIN
  // An installer (Setup.exe / .msi) is run, not swapped over the running exe.
  {
    const QString n = QFileInfo(archivePath).fileName().toLower();
    if (n.contains(QStringLiteral("setup")) ||
        n.contains(QStringLiteral("install")) ||
        n.endsWith(QStringLiteral(".msi"))) {
      const bool ok =
          n.endsWith(QStringLiteral(".msi"))
              ? QProcess::startDetached(
                    QStringLiteral("msiexec"),
                    QStringList{QStringLiteral("/i"),
                                QDir::toNativeSeparators(archivePath)})
              : QProcess::startDetached(QDir::toNativeSeparators(archivePath),
                                        QStringList());
      if (!ok) return fail(QStringLiteral("could not launch the installer"));
      QCoreApplication::quit();
      return true;
    }
  }
  // Cannot overwrite a running executable, so hand the replacement to a helper
  // that waits for this process to exit. Started detached and hidden so no
  // console flashes on the user's desktop.
  const QString helper = archivePath + QStringLiteral(".cmd");
  QFile script(helper);
  if (!script.open(QIODevice::WriteOnly | QIODevice::Text))
    return fail(QStringLiteral("could not write the update installer script"));
  script.write(QStringLiteral("@echo off\r\n"
                              "timeout /t 2 /nobreak >nul\r\n"
                              "move /y \"%1\" \"%2\"\r\n"
                              "start \"\" \"%2\"\r\n"
                              "del \"%3\"\r\n")
                   .arg(QDir::toNativeSeparators(exe),
                        QDir::toNativeSeparators(exe),
                        QDir::toNativeSeparators(helper))
                   .toLocal8Bit());
  script.close();

  if (!QProcess::startDetached(QStringLiteral("cmd.exe"),
                               QStringList{QStringLiteral("/c"),
                                           QStringLiteral("start"),
                                           QStringLiteral("/min"),
                                           QDir::toNativeSeparators(helper)}))
    return fail(QStringLiteral("could not launch the update installer"));

  QCoreApplication::quit();
  return true;

#elif defined(Q_OS_MAC)
  // Hand off to the installer image and let the user drag, which is what a
  // macOS update is expected to be.
  if (!QProcess::startDetached(QStringLiteral("open"),
                               QStringList{QStringLiteral("-a"),
                                           QStringLiteral("Finder"),
                                           archivePath}))
    return fail(QStringLiteral("could not open the downloaded update"));
  QCoreApplication::quit();
  return true;

#else
  // A self-contained AppImage can simply be replaced in place.
  // Inside an AppImage applicationFilePath() is the mounted copy; replace the
  // AppImage file itself.
  const QString appImage = QString::fromLocal8Bit(qgetenv("APPIMAGE"));
  const QString target   = appImage.isEmpty() ? exe : appImage;
  QFile::setPermissions(archivePath,
                        QFile::permissions(archivePath) | QFile::ExeOwner |
                            QFile::ExeGroup | QFile::ExeOther);
  const QString backup = target + QStringLiteral(".old");
  QFile::remove(backup);
  if (!QFile::rename(target, backup))
    return fail(QStringLiteral("could not move the running application aside"));
  if (!QFile::rename(archivePath, target)) {
    QFile::rename(backup, target);  // put it back rather than leave nothing
    return fail(QStringLiteral("could not install the update"));
  }
  QProcess::startDetached(target, QStringList());
  QCoreApplication::quit();
  return true;
#endif
}
