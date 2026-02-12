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

#ifndef QVERSITORGANIZERIMPORTER_P_H
#define QVERSITORGANIZERIMPORTER_P_H

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
#include <QtCore/qset.h>

#include <QtOrganizer/qorganizeritem.h>
#include <QtOrganizer/qorganizeritemdetail.h>

#include <QtVersitOrganizer/qversitorganizerimporter.h>
#include <QtVersitOrganizer/private/qtimezones_p.h>

QT_BEGIN_NAMESPACE_ORGANIZER
class QOrganizerRecurrenceRule;
QT_END_NAMESPACE_ORGANIZER

QTORGANIZER_USE_NAMESPACE

QT_BEGIN_NAMESPACE_VERSITORGANIZER

class QVersitOrganizerHandler;
class QVersitTimeZoneHandler;

class Duration
{
    public:
        Duration() :
            mNegative(false), mWeeks(0), mDays(0), mHours(0), mMinutes(0), mSeconds(0), mValid(true)
            {}
        static Duration invalidDuration() { Duration d; d.setValid(false); return d; }
        static Duration parseDuration(QString str);
        static void parseDurationTime(QString* str, Duration* dur);
        static void parseDurationMinutes(QString* str, Duration* dur);
        static void parseDurationSeconds(QString* str, Duration* dur);
        static QString nextToken(QString* str);

        void setNegative(bool neg) { mNegative = neg; }
        void setWeeks(int weeks) { mWeeks = weeks; }
        void setDays(int days) { mDays = days; }
        void setHours(int hours) { mHours = hours; }
        void setMinutes(int minutes) { mMinutes = minutes; }
        void setSeconds(int seconds) { mSeconds = seconds; }
        void setValid(bool val) { mValid = val; }
        int weeks() { return mNegative ? -mWeeks : mWeeks; }
        int days() { return mNegative ? -mDays : mDays; }
        int hours() { return mNegative ? -mHours : mHours; }
        int minutes() { return mNegative ? -mMinutes : mMinutes; }
        int seconds() { return mNegative ? -mSeconds : mSeconds; }
        bool isValid() { return mValid; }
        int toSeconds() { return (seconds()
                                  + 60*minutes()
                                  + 3600*hours()
                                  + 86400*days()
                                  + 604800*weeks());
                        }

    private:
        bool mNegative;
        int mWeeks;
        int mDays;
        int mHours;
        int mMinutes;
        int mSeconds;
        bool mValid;
};

class QVersitOrganizerImporterPrivate
{
public:
    QVersitOrganizerImporterPrivate(const QString& profile = QString());
    ~QVersitOrganizerImporterPrivate();
    bool importDocument(const QVersitDocument& topLevel,
                        const QVersitDocument& subDocument,
                        QOrganizerItem* item,
                        QVersitOrganizerImporter::Error* error);
    void importProperty(const QVersitDocument& document,
                        const QVersitProperty& property,
                        QOrganizerItem* item);

    QList<QOrganizerItem> mItems;
    QMap<int, QVersitOrganizerImporter::Error> mErrors;
    QVersitOrganizerImporterPropertyHandler* mPropertyHandler;
    QList<QVersitOrganizerHandler*> mPluginPropertyHandlers;
    QVersitTimeZoneHandler* mTimeZoneHandler;
    bool mDurationSpecified; // true iff a valid DURATION property has been seen in the current
                             // document with no subsequent DTEND property

private:
    bool createSimpleDetail(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createTimestampCreated(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createTimestampModified(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createVersion(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createPriority(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createComment(
            const QVersitProperty& property,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createItemReminder(
            const QVersitDocument &valarmDocument,
            QOrganizerItem *item,
            QList<QOrganizerItemDetail> *updatedDetails);
    int triggerToSecondsBeforeStart(const QVersitProperty& triggerProperty, const QOrganizerItem &item);

    bool createExtendedDetail(
            const QVersitProperty &property,
            QList<QOrganizerItemDetail> *updatedDetails);
    bool createRecurrenceId(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createStartDateTime(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createEndDateTime(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createDuration(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createTodoStartDateTime(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createDueDateTime(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createJournalEntryDateTime(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    QDateTime parseDateTime(const QVersitProperty& property, bool* hasTime = 0) const;
    QDateTime parseDateTime(QString str) const;

    bool createRecurrenceRule(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool parseRecurRule(const QString& str, QOrganizerRecurrenceRule* rule) const;
    void parseRecurFragment(const QString& key, const QString& value,
                            QOrganizerRecurrenceRule* rule) const;
    QSet<int> parseInts(const QString& str, int min, int max) const;
    int parseDayOfWeek(const QString& str) const;

    bool createRecurrenceDates(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool parseDates(const QString& str, QSet<QDate>* dates) const;
    QDate parseDate(QString str) const;

    bool createStatus(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createPercentageComplete(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);
    bool createFinishedDateTime(
            const QVersitProperty& property,
            QOrganizerItem* item,
            QList<QOrganizerItemDetail>* updatedDetails);

    TimeZone importTimeZone(const QVersitDocument& document) const;
    TimeZonePhase importTimeZonePhase(const QVersitDocument& document) const;

    QString takeFirst(QList<QString>& list) const;

    // versit property name -> <definition name, field name>:
    QMap<QString, QPair<QOrganizerItemDetail::DetailType, int> > mPropertyMappings;
    TimeZones mTimeZones;
};

QT_END_NAMESPACE_VERSITORGANIZER

#endif // QVERSITORGANIZERIMPORTER_P_H
