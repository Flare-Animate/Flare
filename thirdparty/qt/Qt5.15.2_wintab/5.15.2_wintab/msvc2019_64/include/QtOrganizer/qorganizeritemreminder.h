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

#ifndef QORGANIZERITEMREMINDER_H
#define QORGANIZERITEMREMINDER_H

#include <QtOrganizer/qorganizeritemdetail.h>

QT_BEGIN_NAMESPACE_ORGANIZER

/* Leaf class */

class Q_ORGANIZER_EXPORT QOrganizerItemReminder : public QOrganizerItemDetail
{
public:
#ifndef Q_QDOC
    Q_DECLARE_CUSTOM_ORGANIZER_DETAIL(QOrganizerItemReminder, QOrganizerItemDetail::TypeReminder)
#endif

    enum ReminderField {
        FieldSecondsBeforeStart = TypeReminder + 1,
        FieldRepetitionCount,
        FieldRepetitionDelay
    };

    enum ReminderType {
        NoReminder = 0,
        VisualReminder,
        AudibleReminder,
        EmailReminder

        // other types of reminders?
        //ProcedureReminder,
        //TactileReminder,
    };

    ReminderType reminderType() const;

    void setSecondsBeforeStart(int seconds);
    int secondsBeforeStart() const;

    void setRepetition(int count, int delaySeconds);
    int repetitionDelay() const;
    int repetitionCount() const;

protected:
    /*! \internal */
    QOrganizerItemReminder(DetailType detailType) : QOrganizerItemDetail(detailType) {}
    /*! \internal */
    QOrganizerItemReminder(const QOrganizerItemDetail &detail, DetailType detailType) : QOrganizerItemDetail(detail, detailType) {}
};

#define Q_DECLARE_CUSTOM_ORGANIZER_REMINDER_DETAIL(className, reminderType) \
    className() : QOrganizerItemReminder(reminderType) {} \
    className(const QOrganizerItemDetail &field) : QOrganizerItemReminder(field, reminderType) {} \
    className& operator=(const QOrganizerItemDetail &other) {assign(other, reminderType); return *this;}

QT_END_NAMESPACE_ORGANIZER

#endif // QORGANIZERITEMREMINDER_H
