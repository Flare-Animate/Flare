// QA automation hook (scripts/dev/qa). Active only when FLARE_QA_HOOK=<dir> is set.
//
// The harness writes command lines to <dir>/cmd (atomically, via rename); the app executes
// them from the GUI thread and writes one result line per command to <dir>/result, then
// removes <dir>/cmd. This gives xdotool-based tests a reliable way to do what is fragile
// through file dialogs (save/load a .tnz, export) and to ask where widgets are on screen
// (geometry), without any hard-coded pixel positions.
//
// Commands:  save <path> | load <path> | new | frame <n> | cmd <MI_/T_ id> | shortcut <id> | stats | rooms | room <n>
//            geom <QtClassName> | prefs <n> | prefscount | export xfl|html5 <path> | moho <path> | button <text> | quit
#include "qahook.h"

#include "tapp.h"
#include "iocommand.h"
#include "mainwindow.h"
#include "flareqt/menubarcommand.h"
#include "flareqt/gutil.h"
#include "flare/toonzscene.h"
#include "flare/txsheet.h"
#include "flare/tscenehandle.h"
#include "flare/txsheethandle.h"
#include "flare/tframehandle.h"
#include "tools/tool.h"
#include "tools/toolhandle.h"
#include "tfilepath.h"

#include <QAbstractButton>
#include <QApplication>
#include <QDialog>
#include <QDir>
#include <QFile>
#include <QListWidget>
#include <QTabBar>
#include <QTextStream>
#include <QTimer>
#include <QWidget>

bool flareQaExportTo(bool html5, const QString &path);  // flashexport.cpp
int flareImportMoho(const TFilePath &fp, bool quiet);   // mohoimport.cpp

namespace {

QString geomOf(const QString &cls) {
  QWidget *best = nullptr;
  qint64 bestArea = 0;
  for (QWidget *w : QApplication::allWidgets()) {
    if (!w->isVisible() || cls != QLatin1String(w->metaObject()->className()))
      continue;
    qint64 a = qint64(w->width()) * w->height();
    if (a > bestArea) best = w, bestArea = a;
  }
  if (!best) return "error no visible widget of class " + cls;
  QPoint p = best->mapToGlobal(QPoint(0, 0));
  return QString("ok %1 %2 %3 %4").arg(p.x()).arg(p.y()).arg(best->width()).arg(best->height());
}

QTabBar *roomTabs() {
  MainWindow *mw = qobject_cast<MainWindow *>(TApp::instance()->getMainWindow());
  if (!mw) return nullptr;
  for (QTabBar *tb : mw->findChildren<QTabBar *>())
    if (tb->isVisible()) return tb;
  return nullptr;
}

QListWidget *prefsList() {
  for (QWidget *w : QApplication::topLevelWidgets())
    if (w->isVisible() && QLatin1String(w->metaObject()->className()) == "PreferencesPopup")
      return w->findChild<QListWidget *>();
  return nullptr;
}

// "new"/"load" may raise a save-first question; while the command runs, click its Discard button.
void autoDiscard() {
  QTimer *t = new QTimer(qApp);
  int *n    = new int(0);
  QObject::connect(t, &QTimer::timeout, [t, n] {
    if (++*n > 50) { t->deleteLater(); delete n; return; }
    QWidget *m = QApplication::activeModalWidget();
    if (!m) return;
    for (QAbstractButton *b : m->findChildren<QAbstractButton *>()) {
      const QString txt = QString(b->text()).remove('&');
      if (b->isVisible() && (txt.startsWith("Discard") || txt.endsWith("Anyway"))) { b->click(); return; }
    }
  });
  t->start(200);
}

QString run(const QString &line) {
  const QString l = line.trimmed();
  const int sp = l.indexOf(' ');
  const QString c = sp < 0 ? l : l.left(sp), arg = sp < 0 ? QString() : l.mid(sp + 1).trimmed();
  TApp *app = TApp::instance();
  try {
    if (c == "save") {
      // scene file first (gives the untitled scene its path), then every modified level beside it
      if (!IoCmd::saveScene(TFilePath(arg.toStdWString()), IoCmd::SILENTLY_OVERWRITE)) return "error save failed";
      return IoCmd::saveAll(IoCmd::SILENTLY_OVERWRITE) ? "ok" : "error save levels failed";
    } else if (c == "load") {
      autoDiscard();
      return IoCmd::loadScene(TFilePath(arg.toStdWString()), false, false) ? "ok" : "error load failed";
    } else if (c == "new") {
      app->getCurrentScene()->setDirtyFlag(false);  // discard changes: no "save first?" prompt
      autoDiscard();                                // ...nor the "modified files" prompt
      IoCmd::newScene();
      return "ok";
    } else if (c == "cmd") {
      QByteArray id = arg.toLatin1();
      if (!CommandManager::instance()->getAction(id.constData())) return "error unknown command " + arg;
      CommandManager::instance()->execute(id.constData());
      return "ok";
    } else if (c == "stats") {
      TXsheet *x = app->getCurrentXsheet()->getXsheet();
      TTool *t   = app->getCurrentTool()->getTool();
      return QString("ok frames=%1 cols=%2 curframe=%3 tool=%4 scene=%5")
          .arg(x->getFrameCount()).arg(x->getColumnCount())
          .arg(app->getCurrentFrame()->getFrameIndex())
          .arg(t ? QString::fromStdString(t->getName()) : "none")
          .arg(QString::fromStdWString(app->getCurrentScene()->getScene()->getScenePath().getWideString()));
    } else if (c == "shortcut") {
      QAction *act = CommandManager::instance()->getAction(arg.toLatin1().constData());
      return act ? "ok " + act->shortcut().toString().replace(' ', '_') : "error unknown command " + arg;
    } else if (c == "frame") {
      app->getCurrentFrame()->setFrameIndex(arg.toInt());
      return "ok";
    } else if (c == "geom") {
      return geomOf(arg);
    } else if (c == "rooms") {
      MainWindow *mw = qobject_cast<MainWindow *>(app->getMainWindow());
      QTabBar *tb = roomTabs();
      if (!mw || !tb) return "error no room tabs";
      QStringList n;
      for (int i = 0; i < tb->count(); i++) n << tb->tabText(i).replace(' ', '_');
      return QString("ok %1 %2 current=%3").arg(tb->count()).arg(n.join(',')).arg(tb->currentIndex());
    } else if (c == "room") {
      QTabBar *tb = roomTabs();
      int i = arg.toInt();
      if (!tb || i < 0 || i >= tb->count()) return "error bad room";
      tb->setCurrentIndex(i);
      return "ok";
    } else if (c == "prefscount") {
      QListWidget *lw = prefsList();
      return lw ? QString("ok %1").arg(lw->count()) : "error no preferences popup";
    } else if (c == "prefs") {
      QListWidget *lw = prefsList();
      int i = arg.toInt();
      if (!lw || i < 0 || i >= lw->count()) return "error bad prefs page";
      lw->setCurrentRow(i);
      return "ok " + lw->item(i)->text().replace(' ', '_');
    } else if (c == "export") {
      const QStringList a = arg.split(' ', Qt::SkipEmptyParts);
      if (a.size() != 2) return "error usage: export xfl|html5 path";
      return flareQaExportTo(a[0] == "html5", a[1]) ? "ok" : "error export failed";
    } else if (c == "button") {
      // click the visible push button with this text in the active modal dialog (or any dialog)
      QWidget *top = QApplication::activeModalWidget();
      QList<QWidget *> tops;
      if (top) tops << top;
      for (QWidget *w : QApplication::topLevelWidgets())
        if (w->isVisible() && qobject_cast<QDialog *>(w) && !tops.contains(w)) tops << w;
      for (QWidget *t : tops)
        for (QAbstractButton *b : t->findChildren<QAbstractButton *>())
          if (b->isVisible() && b->isEnabled() && QString(b->text()).remove('&') == arg) {
            QTimer::singleShot(0, b, [b] { b->click(); });
            return "ok";
          }
      return "error no such button " + arg;
    } else if (c == "moho") {
      int cols = flareImportMoho(TFilePath(arg.toStdWString()), true);
      return cols < 0 ? "error moho read failed" : QString("ok columns=%1").arg(cols);
    } else if (c == "quit") {
      QTimer::singleShot(0, qApp, [] { _exit(0); });
      return "ok";
    }
  } catch (const std::exception &e) {
    return QString("error exception %1").arg(e.what());
  } catch (...) {
    return "error unknown exception";
  }
  return "error unknown hook command " + c;
}

}  // namespace

void installQaHook() {
  const QString dir = qEnvironmentVariable("FLARE_QA_HOOK");
  if (dir.isEmpty()) return;
  QDir().mkpath(dir);
  static bool busy = false;
  QTimer *t        = new QTimer(qApp);
  t->setInterval(250);
  QObject::connect(t, &QTimer::timeout, [dir] {
    if (busy) return;
    QFile f(dir + "/cmd");
    if (!f.exists() || !f.open(QIODevice::ReadOnly | QIODevice::Text)) return;
    const QStringList lines = QString::fromUtf8(f.readAll()).split('\n', Qt::SkipEmptyParts);
    f.close();
    f.remove();
    busy = true;
    QStringList out;
    for (const QString &l : lines) out << run(l);
    busy = false;
    QFile r(dir + "/result.tmp");
    if (r.open(QIODevice::WriteOnly | QIODevice::Text)) {
      r.write(out.join('\n').toUtf8() + "\n");
      r.close();
      QFile::remove(dir + "/result");
      r.rename(dir + "/result");
    }
  });
  t->start();
}
