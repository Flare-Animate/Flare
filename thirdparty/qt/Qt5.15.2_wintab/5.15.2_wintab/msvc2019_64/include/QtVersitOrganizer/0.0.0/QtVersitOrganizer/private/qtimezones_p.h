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

#ifndef QTIMEZONES_P_H
#define QTIMEZONES_P_H

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

#include <QtCore/qdatetime.h>
#include <QtCore/qlist.h>
#include <QtCore/qhash.h>

#include <QtOrganizer/qorganizerrecurrencerule.h>

#include <QtVersitOrganizer/qversitorganizerglobal.h>

QT_BEGIN_NAMESPACE_ORGANIZER
class QOrganizerManager;
QT_END_NAMESPACE_ORGANIZER

QTORGANIZER_USE_NAMESPACE

QT_BEGIN_NAMESPACE_VERSITORGANIZER

class TimeZonePhase {
    public:
        TimeZonePhase() : mStandard(true), mUtcOffset(100000) {} // invalid offset
        void setStandard(bool standard) { mStandard = standard; }
        bool isStandard() const { return mStandard; }
        void setUtcOffset(int offset) { mUtcOffset = offset; }
        int utcOffset() const { return mUtcOffset; }
        void setStartDateTime(const QDateTime& dateTime) { mStartDateTime = dateTime; }
        QDateTime startDateTime() const { return mStartDateTime; }
        void setRecurrenceRule(const QOrganizerRecurrenceRule& rrule) { mRecurrenceRule = rrule; }
        QOrganizerRecurrenceRule recurrenceRule() const { return mRecurrenceRule; }
        void setRecurrenceDates(const QSet<QDate>& rdates) { mRecurrenceDates = rdates; }
        QSet<QDate> recurrenceDates() const { return mRecurrenceDates; }
        bool isValid() const {
            // offset must be within -24 hours to +24 hours
            return mStartDateTime.isValid() && mUtcOffset < 86400 && mUtcOffset > -86400;
        }
    private:
        bool mStandard; // true for STANDARD, false for DAYLIGHT
        int mUtcOffset; // in seconds, the offset to apply after mStartDateTime
        QDateTime mStartDateTime; // local time, when the phase comes into effect
        QOrganizerRecurrenceRule mRecurrenceRule;
        QSet<QDate> mRecurrenceDates;
};

class TimeZone {
    public:
        QDateTime convert(const QDateTime& dateTime) const;
        void setTzid(const QString& tzid) { mTzid = tzid; }
        QString tzid() const { return mTzid; }
        void addPhase(const TimeZonePhase& phase) { mPhases.append(phase); }
        bool isValid() const {
            foreach (const TimeZonePhase& phase, mPhases) {
                if (!phase.isValid()) return false;
            }
            return !mPhases.isEmpty();
        }

    private:
        static QOrganizerManager* getManager();
        QString mTzid;
        QList<TimeZonePhase> mPhases;
};

class TimeZones {
    public:
        QDateTime convert(const QDateTime& dateTime, const QString& tzid) const;
        void addTimeZone(const TimeZone& timezone) {
            if (!timezone.tzid().isEmpty())
                    mTimeZones.insert(timezone.tzid(), timezone);
        }
    private:
        QHash<QString, TimeZone> mTimeZones;
};

QT_END_NAMESPACE_VERSITORGANIZER

#endif // QTIMEZONES_P_H
