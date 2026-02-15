/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtVersitOrganizer module of the Qt Toolkit.
**
** $QT_BEGIN_LICENSE:LGPL21$
** Commercial License Usage
** Licensees holding valid commercial Qt licenses may use this file in
** accordance with the commercial license agreement provided with the
** Software or, alternatively, in accordance with the terms contained in
** a written agreement between you and The Qt Company. For licensing terms
** and conditions see http://www.qt.io/terms-conditions. For further
** information use the contact form at http://www.qt.io/contact-us.
**
** GNU Lesser General Public License Usage
** Alternatively, this file may be used under the terms of the GNU Lesser
** General Public License version 2.1 or version 3 as published by the Free
** Software Foundation and appearing in the file LICENSE.LGPLv21 and
** LICENSE.LGPLv3 included in the packaging of this file. Please review the
** following information to ensure the GNU Lesser General Public License
** requirements will be met: https://www.gnu.org/licenses/lgpl.html and
** http://www.gnu.org/licenses/old-licenses/lgpl-2.1.html.
**
** As a special exception, The Qt Company gives you certain additional
** rights. These rights are described in The Qt Company LGPL Exception
** version 1.1, included in the file LGPL_EXCEPTION.txt in this package.
**
** $QT_END_LICENSE$
**
****************************************************************************/

#ifndef QVERSITORGANIZEREXPORTER_P_H
#define QVERSITORGANIZEREXPORTER_P_H

//
//  W A R N I N G
//  -------------
//
// This file is not part of the Qt API.  It exists purely as an
// implementation detail.  This header file may change from version to
// version without notice, or even be removed.
//
// We mean it.
//

#include <QtCore/qpair.h>

#include <QtOrganizer/qorganizeritemdetail.h>

#include <QtVersit/qversitdocument.h>
#include <QtVersit/qversitproperty.h>

#include <QtVersitOrganizer/qversitorganizerexporter.h>
#include <QtVersitOrganizer/qversitorganizerhandler.h>
#include <QtVersitOrganizer/qversittimezonehandler.h>

QT_BEGIN_NAMESPACE_ORGANIZER
class QOrganizerItem;
class QOrganizerItemReminder;
class QOrganizerRecurrenceRule;
QT_END_NAMESPACE_ORGANIZER

QTORGANIZER_USE_NAMESPACE
QTVERSIT_USE_NAMESPACE

QT_BEGIN_NAMESPACE_VERSITORGANIZER

class QVersitOrganizerExporterPrivate
{
public:
    QVersitOrganizerExporterPrivate(const QString& profile = QString());
    ~QVersitOrganizerExporterPrivate();

    bool exportItem(const QOrganizerItem& item,
                    QVersitDocument* document,
                    QVersitOrganizerExporter::Error* error);

    QVersitDocument mResult;
    QMap<int, QVersitOrganizerExporter::Error> mErrors;
    QVersitOrganizerExporterDetailHandler* mDetailHandler;
    QList<QVersitOrganizerHandler*> mPluginDetailHandlers;
    QVersitTimeZoneHandler* mTimeZoneHandler;

private:
    void exportDetail(
            const QOrganizerItem& item,
            const QOrganizerItemDetail& detail,
            QVersitDocument* document);
    void encodeEventTimeRange(
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeTodoTimeRange(
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeJournalTimeRange(
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeTimestamp(
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeVersion(
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeRecurrence(
            const QOrganizerItem& item,
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeRecurRule(
            const QString& propertyName,
            const QOrganizerRecurrenceRule& rule,
            QList<QVersitProperty>* generatedProperties);
    void appendInts(QString* str, const QSet<int>& ints);
    QString weekString(Qt::DayOfWeek day);
    void encodeRecurDates(
            const QString& propertyName,
            const QOrganizerItem& item,
            const QSet<QDate>& dates,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties);
    void encodePriority(
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeInstanceOrigin(
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeTodoProgress(
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeComment(
            const QOrganizerItemDetail& detail,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    void encodeAudibleReminder(
            const QOrganizerItem &item,
            const QOrganizerItemDetail &detail,
            QList<QVersitProperty> *generatedProperties,
            QSet<int> *processedFields);
    void encodeEmailReminder(
            const QOrganizerItem &item,
            const QOrganizerItemDetail &detail,
            QList<QVersitProperty> *generatedProperties,
            QSet<int> *processedFields);
    void encodeVisualReminder(
            const QOrganizerItem &item,
            const QOrganizerItemDetail &detail,
            QList<QVersitProperty> *generatedProperties,
            QSet<int> *processedFields);
    QVersitDocument encodeItemReminderCommonFields(
                const QOrganizerItem &item,
                const QOrganizerItemReminder &reminder,
                QSet<int> *processedFields);
    void encodeExtendedDetail(
            const QOrganizerItemDetail &detail,
            QList<QVersitProperty> *generatedProperties,
            QSet<int> *processedFields);
    void encodeSimpleProperty(
            const QOrganizerItemDetail& detail,
            const QVersitDocument& document,
            QList<QVersitProperty>* removedProperties,
            QList<QVersitProperty>* generatedProperties,
            QSet<int>* processedFields);
    QString encodeDateTime(const QDateTime& dateTime);

    bool documentContainsUidAndRecurrenceId(const QVersitDocument& document);
    QVersitProperty takeProperty(
            const QVersitDocument& document,
            const QString& propertyName,
            QList<QVersitProperty>* toBeRemoved);

    // definition name -> <field name, versit property name>:
    QMap<QOrganizerItemDetail::DetailType, QPair<int, QString> > mPropertyMappings;
};

QT_END_NAMESPACE_VERSITORGANIZER

#endif // QVERSITORGANIZEREXPORTER_P_H
