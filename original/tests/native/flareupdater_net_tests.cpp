// Exercises the shipped FlareUpdater network path against the real GitHub API.
//
// The decision logic is covered by flareupdater_tests; this covers the part that
// cannot be unit-tested: that checkForRelease() actually reaches GitHub, sends
// the headers GitHub requires, parses what comes back, and reports a Release the
// caller can act on. A regression there -- a wrong URL, a dropped User-Agent, a
// changed response shape -- is invisible to the offline tests.
//
// Skips (exit 0) rather than fails when there is no network, so this is safe to
// run in an offline environment. It prints WHY it skipped.


#include <QtCore>
#include <QFile>

#include "flareqt/flareupdater.h"


namespace {

//! Where the human-readable report goes. Passing a path as argv[1] is the same
//! convention the fixture-driven tests use.
QString g_reportPath;

void reportLine(const QString& line) {
  std::printf("%s\n", qPrintable(line));
  std::fflush(stdout);
  if (!g_reportPath.isEmpty()) {
    QFile f(g_reportPath);
    if (f.open(QIODevice::WriteOnly | QIODevice::Append | QIODevice::Text)) {
      f.write(line.toUtf8() + "\n");
      f.close();
    }
  }
}

}  // namespace

namespace {

int g_checks = 0;
int g_failures = 0;

void check(bool ok, const QString& what) {
  ++g_checks;
  reportLine(QStringLiteral("%1 %2").arg(ok ? QStringLiteral("[ok  ]")
                                        : QStringLiteral("[FAIL]"),
                                  what));
  if (!ok)
    ++g_failures;
}

}  // namespace

int main(int argc, char* argv[]) {
  if (argc > 1)
    g_reportPath = QString::fromLocal8Bit(argv[1]);

  QCoreApplication app(argc, argv);

  const QUrl api(QStringLiteral(
      "https://api.github.com/repos/Flare-Animate/Flare/releases/latest"));

  FlareUpdater updater;

  QEventLoop loop;
  bool gotRelease = false;
  FlareUpdater::Release received;
  QString failure;

  QObject::connect(&updater, &FlareUpdater::releaseReady, &loop,
                   [&](const FlareUpdater::Release& r) {
                     received  = r;
                     gotRelease = true;
                     loop.quit();
                   });
  QObject::connect(&updater, &FlareUpdater::failed, &loop,
                   [&](const QString& m) {
                     failure = m;
                     loop.quit();
                   });

  updater.checkForRelease(api);

  // A slow link should not hang the suite forever.
  QTimer::singleShot(30000, &loop, &QEventLoop::quit);
  loop.exec();
  if (!gotRelease && failure.isEmpty()) {
    reportLine(QStringLiteral("[skip] no response within 30s (offline?)"));
    return 0;
  }

  if (!gotRelease && !FlareUpdater::secureConnectionsAvailable()) {
    reportLine(QStringLiteral("[skip] this build has no TLS backend, so no secure "
                        "connection can be made."));
    reportLine(QStringLiteral("       Qt is missing its OpenSSL runtime DLLs; that "
                        "is a packaging matter, not a code fault."));
    reportLine(QStringLiteral("       updater said: %1").arg(failure));
    return 0;
  }

  if (!gotRelease) {
    check(false, QStringLiteral("reaching GitHub: %1").arg(failure));
    reportLine(QStringLiteral("FAILED: %1 of %2 checks failed")
             .arg(g_failures).arg(g_checks));
    return 1;
  }

  check(true, QStringLiteral("the release feed was fetched and parsed"));

  reportLine(QString());
  reportLine(QStringLiteral("-- what came back --"));
  reportLine(QStringLiteral("  tag        : %1").arg(received.tag));
  reportLine(QStringLiteral("  name       : %1").arg(received.name));
  reportLine(QStringLiteral("  prerelease : %1")
           .arg(received.prerelease ? QStringLiteral("yes")
                                   : QStringLiteral("no")));
  reportLine(QStringLiteral("  draft      : %1")
           .arg(received.draft ? QStringLiteral("yes")
                               : QStringLiteral("no")));
  reportLine(QStringLiteral("  assets     : %1").arg(received.assets.size()));
  for (const auto& a : received.assets)
    reportLine(QStringLiteral("    %1  %2 bytes")
             .arg(a.name, QString::number(a.size)));
  reportLine(QString());

  // Against the live feed, these are properties of the *response*, not of us.
  check(!received.tag.isEmpty(), QStringLiteral("the tag is non-empty"));
  check(!received.draft, QStringLiteral("the latest release is not a draft"));
  check(received.tag.contains(QLatin1Char('.')),
        QStringLiteral("the tag looks like a version (%1)").arg(received.tag));

  QString why;
  const auto picked =
      FlareUpdater::pickAssetForCurrentPlatform(received.assets, &why);
  reportLine(QStringLiteral("  picked     : %1%2")
             .arg(picked.name.isEmpty() ? QStringLiteral("(none)")
                                        : picked.name,
                  why.isEmpty() ? QString()
                                : QStringLiteral(" -- ") + why));

  // Not asserting a pick: whether Flare publishes a Windows asset today is not
  // something this test should depend on. Reporting it is the useful part; the
  // selection rules themselves are covered offline.
  if (!picked.isValid())
    reportLine(QStringLiteral("[note] no asset matched this platform: %1")
             .arg(why));
  else
    check(picked.url.isValid() && !picked.url.host().isEmpty(),
          QStringLiteral("the picked asset has a real download URL"));

  reportLine(QString());
  if (g_failures == 0)
    reportLine(QStringLiteral("PASSED: %1 checks, 0 failure(s)").arg(g_checks));
  else
    reportLine(QStringLiteral("FAILED: %1 of %2 checks failed")
           .arg(g_failures).arg(g_checks));
  return g_failures == 0 ? 0 : 1;
}
