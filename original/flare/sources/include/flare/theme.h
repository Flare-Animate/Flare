//
// Flare appearance theming.
//
// This header separates two concepts that are easy to conflate, and which used
// to be conflated in the UI:
//
//   * Theme  - the colour and shape of every panel, button and field. One
//              stylesheet (a .qss under stuff/config/qss/<Name>/<Name>.qss).
//   * Layout - where those panels go: which docks are open, their sizes and
//              contents. One room directory under
//              stuff/profiles/layouts/rooms/<Name>/.
//
// They are independent and freely combinable: any Layout can be used with any
// Theme. "Animate", for instance, is a Layout; "LiquidGlass" is a Theme.
// (Both also existed as the *other* kind at one point -- a Theme named "Animate"
// alongside the Layout named "Animate" -- which is the confusion this file
// exists to remove. scripts/check_theme_layout_separation.py guards against a
// name appearing in both namespaces again.)
//
// Themes may @import another theme to build on it; see
// Preferences::getCurrentStyleSheet() for path resolution.
//

#ifndef FLARE_THEME_H_INCLUDED
#define FLARE_THEME_H_INCLUDED

#include <QString>
#include <QStringList>

class TFilePath;

namespace FlareTheme {

/// Which icon set suits a theme. Icons are not recoloured at runtime, so this
/// is a property of the theme rather than something derived by inspecting the
/// stylesheet text.
enum class IconSet {
  Dark,   // light icons, for a dark theme
  Light,  // dark icons, for a light theme
};

/// Every theme available, as bare names (the directory names under
/// stuff/config/qss). Sorted.
QStringList themeNames();

/// The themesheet for `name`, with @import resolved, or an empty string when
/// there is no such theme.
QString stylesheet(const QString& name);

/// Whether `name` wants the dark or light icon set.
///
/// This is a lookup, not a guess. It used to be inferred by searching the
/// stylesheet text for "Light", "Neutral" or "imgs/black", which mis-classified
/// every theme not named after one of those two words -- including Animate,
/// Clay, Blue and Synthwave, all of which are dark but were being given light
/// icons.
IconSet iconSetFor(const QString& name);

/// True when the theme uses dark icons (i.e. it is a light theme).
bool isLightTheme(const QString& name);

// ── Layouts ────────────────────────────────────────────────────────────────────

/// Every layout available, as bare names (the directory names under
/// stuff/profiles/layouts/rooms). Sorted.
QStringList layoutNames();

/// True when `name` is a known Layout and not a Theme.
bool isLayoutName(const QString& name);

/// True when `name` is a known Theme and not a Layout.
bool isThemeName(const QString& name);

}  // namespace FlareTheme

#endif