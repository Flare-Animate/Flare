// Tests for FlareUpdater's decision logic.
//
// This links the shipped flareqt DLL and calls the shipped statics. A test that
// re-implements the comparison under test would pass against a broken updater,
// so nothing here duplicates production logic -- it feeds real inputs and
// asserts on real outputs.
//
// The three things worth getting right, and the ones most likely to break
// silently in the field:
//
//   * version comparison, including "1.10.0" > "1.9.0" and release-candidate
//     ordering. Getting this wrong either never offers an update or offers one
//     that is not newer.
//   * release-feed parsing, including refusing malformed input instead of
//     returning a half-filled struct that would be treated as "update exists".
//   * picking the right download for the running platform, and *refusing* to
//     guess when several equally plausible candidates exist.

#include <QtCore>

#include "flareqt/flareupdater.h"

namespace {

int g_checks = 0;
int g_failures = 0;

void check(bool ok, const QString& what) {
  ++g_checks;
  if (ok)
    std::printf("[ok  ] %s\n", qPrintable(what));
  else {
    ++g_failures;
    std::printf("[FAIL] %s\n", qPrintable(what));
  }
}

template <typename T>
void checkEq(const T& got, const T& want, const QString& what) {
  ++g_checks;
  if (got == want) {
    std::printf("[ok  ] %s\n", qPrintable(what));
    return;
  }
  ++g_failures;
  std::printf("[FAIL] %s\n", qPrintable(what));
}

void checkStr(const QString& got, const QString& want, const QString& what) {
  ++g_checks;
  if (got == want) {
    std::printf("[ok  ] %s\n", qPrintable(what));
    return;
  }
  ++g_failures;
  std::printf("[FAIL] %s\n         got  %s\n         want %s\n",
              qPrintable(what), qPrintable(got), qPrintable(want));
}

void checkCount(qint64 got, qint64 want, const QString& what) {
  ++g_checks;
  if (got == want) {
    std::printf("[ok  ] %s\n", qPrintable(what));
    return;
  }
  ++g_failures;
  std::printf("[FAIL] %s\n         got  %lld\n         want %lld\n",
              qPrintable(what), static_cast<long long>(got),
              static_cast<long long>(want));
}

//------------------------------------------------------------------------------

void testVersionComparison() {
  std::printf("-- version comparison\n");

  checkCount(FlareUpdater::compareVersions(QStringLiteral("1.8.0"),
          QStringLiteral("1.7.1")),
          1, QStringLiteral("1.8.0 is newer than 1.7.1"));
  checkCount(FlareUpdater::compareVersions(QStringLiteral("1.7.1"),
          QStringLiteral("1.8.0")),
          -1, QStringLiteral("1.7.1 is older than 1.8.0"));
  checkCount(FlareUpdater::compareVersions(QStringLiteral("1.8.0"),
          QStringLiteral("1.8.0")),
          0, QStringLiteral("equal versions compare equal"));

  // The classic bug: a string compare says "1.9.0" > "1.10.0".
  check(FlareUpdater::compareVersions(QStringLiteral("1.10.0"),
                                      QStringLiteral("1.9.0")) > 0,
        QStringLiteral("1.10.0 is newer than 1.9.0 (numeric, not lexical)"));
  check(FlareUpdater::compareVersions(QStringLiteral("1.9.0"),
                                      QStringLiteral("1.10.0")) < 0,
        QStringLiteral("1.9.0 is older than 1.10.0"));

  // Tags carry a leading "v" from the git tag.
  checkCount(FlareUpdater::compareVersions(QStringLiteral("v1.8.0"),
          QStringLiteral("1.8.0")),
          0, QStringLiteral("a leading 'v' is ignored"));
  check(FlareUpdater::compareVersions(QStringLiteral("v2.0.0"),
                                      QStringLiteral("v1.9.9")) > 0,
        QStringLiteral("'v' prefix does not defeat comparison"));

  // Ragged versions: a missing field is zero.
  checkCount(FlareUpdater::compareVersions(QStringLiteral("1.8"),
          QStringLiteral("1.8.0")),
          0, QStringLiteral("1.8 equals 1.8.0"));
  check(FlareUpdater::compareVersions(QStringLiteral("1.8.1"),
                                      QStringLiteral("1.8")) > 0,
        QStringLiteral("1.8.1 is newer than 1.8"));

  // Release candidates must not be offered as "newer" than the final release.
  check(FlareUpdater::compareVersions(QStringLiteral("1.8.0"),
                                      QStringLiteral("1.8.0-rc1")) > 0,
        QStringLiteral("a final release outranks its own rc"));
  check(FlareUpdater::compareVersions(QStringLiteral("1.8.0-rc2"),
                                      QStringLiteral("1.8.0-rc1")) > 0,
        QStringLiteral("rc2 outranks rc1"));
  checkCount(FlareUpdater::compareVersions(QStringLiteral("1.8.0-rc1"),
          QStringLiteral("v1.8.0-rc1")),
          0, QStringLiteral("pre-release tags compare consistently"));

  // Junk must not crash and must not fabricate an update.
  checkCount(FlareUpdater::compareVersions(QString(),
          QStringLiteral("1.0.0")), -1,
          QStringLiteral("an empty version is older than 1.0.0"));
  checkCount(FlareUpdater::compareVersions(QStringLiteral("not-a-version"),
          QStringLiteral("also-not")),
          0, QStringLiteral("two junk versions compare equal, not randomly"));
  check(FlareUpdater::compareVersions(QStringLiteral("garbage"),
                                      QStringLiteral("1.0.0")) < 0,
        QStringLiteral("junk never claims to be newer than a real version"));

  check(FlareUpdater::isPreReleaseTag(QStringLiteral("1.8.0-rc1")),
        QStringLiteral("1.8.0-rc1 is detected as a pre-release"));
  check(FlareUpdater::isPreReleaseTag(QStringLiteral("v2.0.0-beta")),
        QStringLiteral("2.0.0-beta is detected as a pre-release"));
  check(!FlareUpdater::isPreReleaseTag(QStringLiteral("1.8.0")),
        QStringLiteral("1.8.0 is not a pre-release"));
  check(!FlareUpdater::isPreReleaseTag(QStringLiteral("v1.7.1")),
        QStringLiteral("1.7.1 is not a pre-release"));
}

//------------------------------------------------------------------------------

QByteArray releaseJson(const char* tag, const char* body,
                       const QList<QPair<QString, QString>>& assets,
                       bool prerelease = false) {
  QJsonArray arr;
  for (const auto& kv : assets) {
    QJsonObject a;
    a.insert(QStringLiteral("name"), kv.first);
    a.insert(QStringLiteral("browser_download_url"), kv.second);
    a.insert(QStringLiteral("size"), 1234);
    arr.append(a);
  }
  QJsonObject root;
  root.insert(QStringLiteral("tag_name"), QString::fromLatin1(tag));
  root.insert(QStringLiteral("name"), QString::fromLatin1(tag));
  root.insert(QStringLiteral("body"), QString::fromUtf8(body));
  root.insert(QStringLiteral("prerelease"), prerelease);
  root.insert(QStringLiteral("draft"), false);
  root.insert(QStringLiteral("assets"), arr);
  return QJsonDocument(root).toJson(QJsonDocument::Compact);
}

void testReleaseParsing() {
  std::printf("-- release feed parsing\n");

  QString error;
  const auto assets = QList<QPair<QString, QString>>{
      {QStringLiteral("Flare-1.8.0-win64.exe"),
       QStringLiteral("https://example.invalid/a.exe")},
      {QStringLiteral("Flare-1.8.0-linux.AppImage"),
       QStringLiteral("https://example.invalid/a.AppImage")},
  };
  const auto release = FlareUpdater::parseReleaseJson(
      releaseJson("v1.8.0", "notes here", assets), &error);

  check(error.isEmpty(), QStringLiteral("a well-formed feed reports no error"));
  check(release.isValid(), QStringLiteral("a well-formed feed yields a release"));
  checkStr(release.tag, QStringLiteral("v1.8.0"),
          QStringLiteral("tag_name is read"));
  checkStr(release.notes, QStringLiteral("notes here"),
          QStringLiteral("release notes are read"));
  check(!release.prerelease, QStringLiteral("prerelease flag is read"));
  checkCount(release.assets.size(), 2, QStringLiteral("both assets are read"));

  checkStr(release.assets.at(0).name, QStringLiteral("Flare-1.8.0-win64.exe"),
          QStringLiteral("asset name is read"));
  check(release.assets.at(0).url.toString() ==
            QStringLiteral("https://example.invalid/a.exe"),
        QStringLiteral("asset download URL is read"));
  checkCount(release.assets.at(0).size, static_cast<qint64>(1234),
             QStringLiteral("asset size is read"));

  // A flagged pre-release must survive parsing, because the caller uses it to
  // decide whether to offer the update at all.
  const auto pre = FlareUpdater::parseReleaseJson(
      releaseJson("v2.0.0-rc1", "", {}, /*prerelease=*/true), &error);
  check(error.isEmpty(), QStringLiteral("an rc feed parses cleanly"));
  check(pre.prerelease, QStringLiteral("the prerelease flag survives parsing"));
  check(FlareUpdater::isPreReleaseTag(pre.tag),
        QStringLiteral("the rc tag is recognised as a pre-release"));

  // Malformed input must be refused, never half-parsed.
  struct Case {
    const char* json;
    const char* why;
  };
  const Case bad[] = {
      {"", QStringLiteral("empty body is rejected").toLatin1().constData()},
      {"not json at all", "non-JSON is rejected"},
      {"[1,2,3]", "a JSON array is rejected"},
      {"{}", "a feed with no tag_name is rejected"},
      {R"({"tag_name":"v1.8.0","assets":[{"name":"a.exe"}]})",
       "an asset with no download URL is dropped"},
  };

  for (const Case& c : bad) {
    QString err;
    const auto r = FlareUpdater::parseReleaseJson(QByteArray(c.json), &err);
    if (QByteArray(c.why) == "an asset with no download URL is dropped") {
      // Not an error -- just no usable assets.
      check(r.tag == QStringLiteral("v1.8.0") && r.assets.isEmpty(),
            QStringLiteral("an asset with no download URL is dropped"));
      continue;
    }
    check(!err.isEmpty(),
          QStringLiteral("%1").arg(QString::fromLatin1(c.why)));
  }

  // A feed whose only assets are malformed yields a release with no assets, so
  // the caller can say "nothing to download" rather than crashing.
  QString err2;
  const auto noAssets = FlareUpdater::parseReleaseJson(
      QByteArray(R"({"tag_name":"v1.8.0","assets":[]})"), &err2);
  check(err2.isEmpty() && noAssets.isValid() && noAssets.assets.isEmpty(),
        QStringLiteral("a release with an empty asset list is valid but empty"));
}

//------------------------------------------------------------------------------

FlareUpdater::Asset asset(const char* name) {
  FlareUpdater::Asset a;
  a.name = QString::fromLatin1(name);
  a.url  = QUrl(QStringLiteral("https://example.invalid/") + a.name);
  a.size = 100;
  return a;
}

void testAssetSelection() {
  std::printf("-- platform asset selection\n");

  QString why;
  const auto onThisHost = []() {
    // Mirror the platform tokens the shipped picker ranks, so the test asserts
    // the intended behaviour rather than hard-coding one host.
#if defined(Q_OS_WIN)
    return QStringLiteral("win64");
#elif defined(Q_OS_MAC)
    return QStringLiteral("macos");
#else
    return QStringLiteral("appimage");
#endif
  }();

  const auto picked = FlareUpdater::pickAssetForCurrentPlatform(
      {asset("Flare-1.8.0-linux.AppImage"), asset("Flare-1.8.0-win64.exe"),
       asset("Flare-1.8.0-macos.dmg")},
      &why);

  check(picked.isValid(),
        QStringLiteral("a multi-platform release yields a download (%1)")
            .arg(onThisHost));
  check(picked.name.contains(onThisHost, Qt::CaseInsensitive),
        QStringLiteral("the download chosen is the one for this platform (%1)")
            .arg(picked.name));
  check(why.isEmpty(), QStringLiteral("a successful pick explains nothing"));

  // Prefer the 64-bit build when both are published.
  const auto both64 = FlareUpdater::pickAssetForCurrentPlatform(
      {asset("Flare-1.8.0-win32.exe"), asset("Flare-1.8.0-win64.exe")}, &why);
#if defined(Q_OS_WIN)
  check(both64.name.contains(QStringLiteral("win64")),
        QStringLiteral("a 64-bit build is preferred over a 32-bit one"));
#else
  Q_UNUSED(both64);
#endif

  // Nothing for this platform: say so, do not download something random.
  QString why2;
  const auto foreign = FlareUpdater::pickAssetForCurrentPlatform(
      {asset("Flare-1.8.0-solaris-sparc.pkg")}, &why2);
  check(!foreign.isValid(),
        QStringLiteral("a release with no build for this platform is refused"));
  check(!why2.isEmpty(), QStringLiteral("refusing explains why"));

  // Two equally plausible candidates: refuse rather than guess wrong.
  // These have to be files this platform would actually accept (an .exe on
  // Windows) so the ambiguity branch is reached -- two unrecognised files are a
  // different case, covered below.
  QString why3;
  const auto ambiguous = FlareUpdater::pickAssetForCurrentPlatform(
      {asset("Flare-1.8.0-portable.exe"), asset("Flare-1.8.0-full.exe")},
      &why3);
  check(!ambiguous.isValid(),
        QStringLiteral("several equally plausible downloads are refused"));
  check(why3.contains(QStringLiteral("several")),
        QStringLiteral("the ambiguity is reported as such"));

  // Files we do not recognise for this platform at all: refused, and the
  // message says so rather than blaming ambiguity.
  QString why3b;
  const auto unrecognised = FlareUpdater::pickAssetForCurrentPlatform(
      {asset("Flare-1.8.0-portable.zip"), asset("Flare-1.8.0-full.zip")},
      &why3b);
  check(!unrecognised.isValid(),
        QStringLiteral("unrecognised file types are refused"));
  check(!why3b.isEmpty() && !why3b.contains(QStringLiteral("several")),
        QStringLiteral("unrecognised files are not reported as ambiguous"));

  // A single unlabelled binary is unambiguous enough to install.
  QString why4;
  const auto single = FlareUpdater::pickAssetForCurrentPlatform(
      {asset("Flare-1.8.0.exe")}, &why4);
#if defined(Q_OS_WIN)
  check(single.isValid(),
        QStringLiteral("a lone .exe is accepted on Windows"));
#else
  check(!single.isValid(),
        QStringLiteral("a lone .exe is not offered off Windows"));
#endif

  QString why5;
  check(!FlareUpdater::pickAssetForCurrentPlatform({}, &why5).isValid(),
        QStringLiteral("an empty asset list is refused"));
  check(!why5.isEmpty(), QStringLiteral("an empty list explains why"));

  // The running executable always has a name; used to keep the downloaded file
  // from clobbering the current one.
  check(!FlareUpdater::runningExecutableName().isEmpty(),
        QStringLiteral("the running executable has a name"));
}

}  // namespace

int main(int argc, char* argv[]) {
  QCoreApplication app(argc, argv);

  std::printf("FlareUpdater decision-logic tests\n");
  testVersionComparison();
  testReleaseParsing();
  testAssetSelection();

  std::printf("\n");
  if (g_failures == 0)
    std::printf("PASSED: %d checks, 0 failure(s)\n", g_checks);
  else
    std::printf("FAILED: %d of %d checks failed\n", g_failures, g_checks);
  return g_failures == 0 ? 0 : 1;
}
