#pragma once

#ifndef FLAREUPDATER_H
#define FLAREUPDATER_H

#include <QtGlobal>
#include <QObject>
#include <QByteArray>
#include <QList>
#include <QString>
#include <QUrl>
#include <QVector>

#include "tcommon.h"

class QNetworkAccessManager;
class QNetworkReply;
class QProgressDialog;

#undef DVAPI
#undef DVVAR
#ifdef FLAREQT_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

/*!
    \brief Checks a GitHub release feed and, when a newer one exists, downloads
    and installs it.

    The class is split deliberately. Everything that decides *what* should happen
    -- version comparison, parsing the release feed, choosing the right download
    for the running platform -- is a pure static function over plain data, with
    no network and no widgets. That is what tests/native/flareupdater_tests.cpp
    drives against the shipped DLL. Only download() and friends touch the
    network, and only applyUpdate() touches the filesystem.

    Existing installs already have a version check that just opens a web page
    (see UpdateChecker and MainWindow::checkForUpdates). This adds the part that
    was missing: actually fetching and installing the new build.
*/
class DVAPI FlareUpdater final : public QObject {
  Q_OBJECT

public:
  //! One downloadable file attached to a release.
  struct Asset {
    QString name;
    QUrl url;
    qint64 size = 0;

    bool isValid() const { return !name.isEmpty() && url.isValid(); }
  };

  //! The subset of a GitHub release we care about.
  struct Release {
    QString tag;
    QString name;
    QString notes;
    QList<Asset> assets;
    bool prerelease = false;
    bool draft      = false;

    bool isValid() const { return !tag.isEmpty(); }
  };

  //--------------------------------------------------------------------------
  // Pure logic. No network, no filesystem, no globals.
  //--------------------------------------------------------------------------

  /*! Compares two dotted version strings, tolerating a leading "v" and a
      trailing pre-release suffix.

      Returns >0 when \a a is newer than \a b, 0 when they are equal, <0 when
      \a a is older. Numeric fields compare numerically, so "1.10.0" is newer
      than "1.9.0" -- the whole point of not using QString::compare(). Missing
      fields count as zero, so "1.8" == "1.8.0". A version carrying a
      pre-release suffix ("1.8.0-rc1") sorts *below* the same version without
      one, which is what a user expects when offered a release candidate.

      Non-numeric junk in a field compares as zero rather than throwing, and two
      tags that contain no digits at all are reported equal -- so a malformed tag
      can never make the updater claim an update exists. */
  static int compareVersions(const QString& a, const QString& b);

  //! True when \a tag names a pre-release ("1.8.0-rc1", "v2.0.0-beta").
  static bool isPreReleaseTag(const QString& tag);

  /*! Parses the JSON body of GitHub's /releases/latest endpoint.

      Returns a default-constructed Release and sets \a error on anything it
      cannot make sense of, rather than throwing or returning a half-filled
      struct. An empty \a error means the payload was understood. */
  static Release parseReleaseJson(const QByteArray& json, QString* error);

  /*! Chooses the asset to download for the platform this build is running on.

      Returns an invalid Asset and sets \a why when nothing matches, which is
      the common case for a release that only ships one platform's binary.
      Preference order per platform is documented in flareupdater.cpp. */
  static Asset pickAssetForCurrentPlatform(const QList<Asset>& assets,
                                           QString* why = nullptr);

  //! The file name portion of the running executable, e.g. "Flare.exe".
  static QString runningExecutableName();

  /*! True when this build can make HTTPS connections at all.

      Qt needs its OpenSSL runtime DLLs deployed next to the binary; a portable
      build that omits them cannot open a secure connection even with a working
      network. Worth asking before blaming the user's connection, and it is
      exposed here because QSslSocket lives in Qt5::Network, which only
      flareqt links. */
  static bool secureConnectionsAvailable();

  //--------------------------------------------------------------------------
  // Network + filesystem.
  //--------------------------------------------------------------------------

  explicit FlareUpdater(QObject* parent = nullptr);
  ~FlareUpdater() override;

  //! Starts the release lookup. Results arrive via releaseReady()/failed().
  void checkForRelease(const QUrl& releasesApiUrl);

  //! Downloads \a asset into \a destinationDir, reporting via downloadProgress().
  void download(const Asset& asset, const QUrl& destinationDir);

  /*! Installs a previously downloaded \a archivePath and relaunches.

      Returns false and sets \a error when the platform is unsupported or the
      file cannot be replaced. On success the process is replaced and does not
      return, so callers should treat a true return as "shutting down". */
  static bool applyUpdate(const QString& archivePath, QString* error);

signals:
  void releaseReady(const FlareUpdater::Release& release);
  void failed(const QString& message);

  void downloadProgress(qint64 received, qint64 total);
  void downloadFinished(const QString& savedPath);

private:
  QNetworkAccessManager* m_manager = nullptr;
  QNetworkReply* m_releaseReply = nullptr;
  QNetworkReply* m_downloadReply = nullptr;
  QUrl m_destinationDir;

  void onReleaseFinished();
  void onDownloadFinished();
};

// No Q_DECLARE_METATYPE for Release/Asset: every connection here is a direct
// typed connect() on the same thread, so queued-argument marshalling is never
// involved, and declaring them drags in QMetaTypeId specialisations that Qt
// 5.15 does not provide for structs containing QList.

#endif  // FLAREUPDATER_H
