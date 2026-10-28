#pragma once

#include "archive.h"

#include <QJsonObject>
#include <QMap>
#include <QSet>
#include <QString>

namespace Flash {

// This inventory accounts for XML parser records in every package
// member whose name ends in .xml, including members the native builder never
// reads. It does not assign native behavior merely because parsing succeeded.
struct FieldAccountingLimits {
  quint64 maxXmlMemberBytes        = 64ull * 1024 * 1024;
  quint64 maxXmlTotalBytes         = 512ull * 1024 * 1024;
  quint64 maxElements              = 4000000;
  quint64 maxAttributes            = 12000000;
  quint64 maxContentRecords        = 4000000;
  quint64 maxNamespaceDeclarations = 4000000;
  int maxDepth                     = 128;
  int maxContexts                  = 100000;
  int maxReportBytes               = 16 * 1024 * 1024;
};

struct FieldLookup {
  QString sourceId;
  QString
      kind;  // element, attribute, characters, cdata, comment, PI, namespace
  QString expandedName;
  QString qualifiedName;
  QString value;  // XML parser value; exact lexical bytes live in Package.
  QString valueRepresentation;  // none or xml-parser-normalized
  QString semanticRole;
  QString nativeMappingStatus;
};

QJsonObject accountXmlFields(Package &package,
                             const FieldAccountingLimits &limits = {});

// Replays structural locators against a member whose SHA-256 must match the
// accounting report, with one bounded scan. No byte offset or XPath prefix
// spelling is inferred. Missing IDs are omitted from the result; callers must
// reject missing records when a native mapping requires them.
QMap<QString, FieldLookup> lookupXmlFields(
    Package &package, const QJsonObject &accounting, const QString &member,
    const QSet<QString> &sourceIds, const FieldAccountingLimits &limits = {});

}  // namespace Flash
