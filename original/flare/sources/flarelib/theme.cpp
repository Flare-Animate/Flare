#include "flare/theme.h"

#include "flare/toonzfolders.h"
#include "tenv.h"
#include "tfilepath.h"
#include "tsystem.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QMap>
#include <QRegularExpression>
#include <QTextStream>

namespace {

// Icon set per theme. Explicit rather than inferred: the old inference searched
// the stylesheet text for "Light"/"Neutral"/"imgs/black", so Animate, Clay, Blue
// and Synthwave -- all dark -- were given the light icon set.
//
// A name absent from this table falls back to Dark, which is right for every
// theme currently shipped; add an entry when adding a light theme.
const QMap<QString, bool>& lightThemeTable() {
  static const QMap<QString, bool> table = {
      {"Light", true},
      {"Neutral", true},
      {"Default", false},
      {"Default-Green", false},
      {"Dark", false},
      // Named for its appearance. This theme used to be called "Animate",
      // which collided with the "Animate" Layout shown in the same tab -- one
      // name, two meanings. scripts/check_theme_layout_separation.py keeps the
      // two namespaces disjoint.
      {"AnimateDark", false},
      {"Blue", false},
      {"Clay", false},
      {"LiquidGlass", false},
      {"Synthwave", false},
  };
  return table;
}

/// Directory holding the themes, without a trailing separator.
QString themeDir() {
  return TFilePath(TEnv::getConfigDir() + "qss").getQString();
}

/// Directory holding the layouts (rooms), without a trailing separator.
QString layoutDir() {
  return TFilePath(FlareFolder::getRoomsDir()).getQString();
}

QStringList subdirectoryNames(const QString& root) {
  QStringList out;
  QDir dir(root);
  if (!dir.exists()) return out;
  for (const QFileInfo& fi : dir.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot)) {
    out << fi.fileName();
  }
  out.sort();
  return out;
}

/// Recursively inline @import, so callers get one self-contained stylesheet.
/// Preferences::getCurrentStyleSheet() also rewrites the specifiers to absolute
/// paths before they reach here; this resolves them relative to the importing
/// file and guards against an import cycle.
QString resolveImports(const QString& text, const QString& baseDir, int depth) {
  if (depth > 8) return text;  // cycle guard
  static const QRegularExpression re(
      QStringLiteral("@import\\s+(['\"])([^'\"]+)\\1"));
  QString out;
  int last = 0;
  for (auto it = re.globalMatch(text); it.hasNext();) {
    const QRegularExpressionMatch m = it.next();
    out += text.mid(last, m.capturedStart() - last);
    QString spec = m.captured(2);
    QFileInfo fi(spec);
    const QString abs =
        fi.isAbsolute() ? spec : QDir(baseDir).absoluteFilePath(spec);
    QFile f(abs);
    if (f.open(QFile::ReadOnly | QFile::Text)) {
      QTextStream ts(&f);
      out += resolveImports(ts.readAll(), QFileInfo(abs).absolutePath(), depth + 1);
    }
    last = m.capturedEnd();
  }
  out += text.mid(last);
  return out;
}

}  // namespace

namespace FlareTheme {

QStringList themeNames() { return subdirectoryNames(themeDir()); }

QStringList layoutNames() { return subdirectoryNames(layoutDir()); }

QString stylesheet(const QString& name) {
  if (name.isEmpty()) return QString();
  const QString dir = themeDir();
  QFile f(QDir(dir).absoluteFilePath(name + "/" + name + ".qss"));
  if (!f.open(QFile::ReadOnly | QFile::Text)) return QString();
  QTextStream ts(&f);
  return resolveImports(ts.readAll(), dir, 0);
}

IconSet iconSetFor(const QString& name) {
  return lightThemeTable().value(name, false) ? IconSet::Light : IconSet::Dark;
}

bool isLightTheme(const QString& name) {
  return iconSetFor(name) == IconSet::Light;
}

bool isLayoutName(const QString& name) {
  return !name.isEmpty() && layoutNames().contains(name);
}

bool isThemeName(const QString& name) {
  return !name.isEmpty() && themeNames().contains(name);
}

}  // namespace FlareTheme