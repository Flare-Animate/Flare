/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtOrganizer module of the Qt Toolkit.
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

#ifndef QORGANIZERRECURRENCERULE_H
#define QORGANIZERRECURRENCERULE_H

#include <QtCore/qdatetime.h>
#include <QtCore/qmetatype.h>
#include <QtCore/qset.h>
#include <QtCore/qshareddata.h>

#include <QtOrganizer/qorganizerglobal.h>

QT_BEGIN_NAMESPACE_ORGANIZER

class QOrganizerRecurrenceRulePrivate;
class Q_ORGANIZER_EXPORT QOrganizerRecurrenceRule
{
public:
    enum Frequency {
        Invalid = 0,
        Daily,
        Weekly,
        Monthly,
        Yearly
    };

    enum Month {
        January = 1,
        February,
        March,
        April,
        May,
        June,
        July,
        August,
        September,
        October,
        November,
        December
    };

    enum LimitType {
        NoLimit = 0,
        CountLimit,
        DateLimit
    };

    QOrganizerRecurrenceRule();
    QOrganizerRecurrenceRule(const QOrganizerRecurrenceRule &other);
    ~QOrganizerRecurrenceRule();

    QOrganizerRecurrenceRule &operator=(const QOrganizerRecurrenceRule &other);

    bool operator==(const QOrganizerRecurrenceRule &other) const;
    inline bool operator!=(const QOrganizerRecurrenceRule &other) const { return !operator==(other); }

    void setFrequency(Frequency freq);
    Frequency frequency() const;

    void setLimit(int count);
    void setLimit(const QDate &date);
    void clearLimit();

    LimitType limitType() const;
    int limitCount() const;
    QDate limitDate() const;

    void setInterval(int interval);
    int interval() const;

    void setDaysOfWeek(const QSet<Qt::DayOfWeek> &days);
    QSet<Qt::DayOfWeek> daysOfWeek() const;

    void setDaysOfMonth(const QSet<int> &days);
    QSet<int> daysOfMonth() const;

    void setDaysOfYear(const QSet<int> &days);
    QSet<int> daysOfYear() const;

    void setMonthsOfYear(const QSet<Month> &months);
    QSet<Month> monthsOfYear() const;

    void setWeeksOfYear(const QSet<int> &weeks);
    QSet<int> weeksOfYear() const;

    void setFirstDayOfWeek(Qt::DayOfWeek day);
    Qt::DayOfWeek firstDayOfWeek() const;

    void setPositions(const QSet<int> &pos);
    QSet<int> positions() const;

private:
    QSharedDataPointer<QOrganizerRecurrenceRulePrivate> d;
};

//hash functions
Q_ORGANIZER_EXPORT uint qHash(const QOrganizerRecurrenceRule &rule);

#ifndef QT_NO_DEBUG_STREAM
Q_ORGANIZER_EXPORT QDebug operator<<(QDebug dbg, const QOrganizerRecurrenceRule &rule);
#endif // QT_NO_DEBUG_STREAM

inline uint qHash(QOrganizerRecurrenceRule::Month month)
{
    return static_cast<uint>(month);
}

QT_END_NAMESPACE_ORGANIZER

QT_BEGIN_NAMESPACE
Q_DECLARE_TYPEINFO(QTORGANIZER_PREPEND_NAMESPACE(QOrganizerRecurrenceRule), Q_MOVABLE_TYPE);
QT_END_NAMESPACE

Q_DECLARE_METATYPE(QTORGANIZER_PREPEND_NAMESPACE(QOrganizerRecurrenceRule))
Q_DECLARE_METATYPE(QSet<QTORGANIZER_PREPEND_NAMESPACE(QOrganizerRecurrenceRule)>)
Q_DECLARE_METATYPE(QSet<QDate>)

#endif // QORGANIZERRECURRENCERULE_H
