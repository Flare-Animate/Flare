#pragma once

#include "../flash/archive.h"
#include "tcommon.h"
#include <QJsonArray>
#include <QJsonObject>
#include <QString>

class ToonzScene;

#undef DVAPI
#ifdef TOONZLIB_EXPORTS
#define DVAPI DV_EXPORT_API
#else
#define DVAPI DV_IMPORT_API
#endif

namespace FlashNative {

// A bounded, import-time proof for explicitly authored graphic-instance and
// passive frame-label fields. This is separate from the all-field inventory:
// unsupported fields remain unclassified there. Native edits can change the
// current values without changing the immutable import-time proof.
QJsonObject verifiedGraphicFieldBindings(Flash::Package &package,
                                         const QJsonObject &accounting,
                                         const QString &accountingSha256,
                                         const ToonzScene &reopened,
                                         const QJsonArray &nativeSourceMap);

// Hash-check the immutable source bundle, then compare current linked native
// instance and frame-label values with the values proven at import. Equal
// values say nothing about edit history or unsupported Flash behavior.
DVAPI QJsonObject inspectCurrentGraphicFieldBindings(const ToonzScene &scene,
                                                     int maxDiagnostics = 1000);

}  // namespace FlashNative
