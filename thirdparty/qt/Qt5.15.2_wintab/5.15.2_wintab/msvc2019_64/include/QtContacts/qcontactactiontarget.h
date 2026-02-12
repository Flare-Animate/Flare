/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtContacts module of the Qt Toolkit.
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

#ifndef QCONTACTACTIONTARGET_H
#define QCONTACTACTIONTARGET_H

#include <QtCore/qlist.h>
#include <QtCore/qshareddata.h>

#include <QtContacts/qcontact.h>
#include <QtContacts/qcontactdetail.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContact;

class QContactActionTargetPrivate;
class Q_CONTACTS_EXPORT QContactActionTarget
{
public:
    explicit QContactActionTarget(const QContact& contact = QContact(), const QList<QContactDetail>& details = QList<QContactDetail>());
    QContactActionTarget(const QContact& contact, const QContactDetail& detail);

    QContactActionTarget(const QContactActionTarget& other);

    QContactActionTarget& operator=(const QContactActionTarget& other);
    ~QContactActionTarget();

    enum Type {
        Invalid, // no contact
        WholeContact, // no details
        SingleDetail, // a single detail
        MultipleDetails
    };

    Type type() const; // No setter
    bool isValid() const;
    bool operator==(const QContactActionTarget& other) const;
    bool operator!=(const QContactActionTarget& other) const;

    void setContact(const QContact& contact);
    void setDetails(const QList<QContactDetail>& details);

    QContact contact() const;
    QList<QContactDetail> details() const;

private:
#ifndef QT_NO_DATASTREAM
    Q_CONTACTS_EXPORT friend QDataStream& operator<<(QDataStream& out, const QContactActionTarget& target);
    Q_CONTACTS_EXPORT friend QDataStream& operator>>(QDataStream& in, QContactActionTarget& target);
#endif
    QSharedDataPointer<QContactActionTargetPrivate> d;
};

Q_CONTACTS_EXPORT uint qHash(const QContactActionTarget& key);
#ifndef QT_NO_DATASTREAM
Q_CONTACTS_EXPORT QDataStream& operator<<(QDataStream& out, const QContactActionTarget& target);
Q_CONTACTS_EXPORT QDataStream& operator>>(QDataStream& in, QContactActionTarget& target);
#endif
#ifndef QT_NO_DEBUG_STREAM
Q_CONTACTS_EXPORT QDebug& operator<<(QDebug dbg, const QContactActionTarget& target);
#endif

QT_END_NAMESPACE_CONTACTS

QT_BEGIN_NAMESPACE
Q_DECLARE_TYPEINFO(QTCONTACTS_PREPEND_NAMESPACE(QContactActionTarget), Q_MOVABLE_TYPE);
QT_END_NAMESPACE

#endif // QCONTACTACTIONTARGET_H
