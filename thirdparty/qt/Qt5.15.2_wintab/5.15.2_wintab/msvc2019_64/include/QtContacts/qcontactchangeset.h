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

#ifndef QCONTACTCHANGESET_H
#define QCONTACTCHANGESET_H

#include <QtCore/qlist.h>
#include <QtCore/qpair.h>
#include <QtCore/qset.h>
#include <QtCore/qshareddata.h>

#include <QtContacts/qcontactid.h>
#include <QtContacts/qcontactdetail.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContactManagerEngine;

class QContactChangeSetData;
class Q_CONTACTS_EXPORT QContactChangeSet
{
public:
    typedef QPair<QList<QContactDetail::DetailType>, QList<QContactId> > ContactChangeList;

    QContactChangeSet();
    QContactChangeSet(const QContactChangeSet& other);
    ~QContactChangeSet();

    QContactChangeSet& operator=(const QContactChangeSet& other);

    void setDataChanged(bool dataChanged);
    bool dataChanged() const;

    QSet<QContactId> addedContacts() const;
    void insertAddedContact(QContactId addedContactId);
    void insertAddedContacts(const QList<QContactId>& addedContactIds);
    void clearAddedContacts();

    QList<ContactChangeList> changedContacts() const;
    void insertChangedContact(QContactId addedContactId, const QList<QContactDetail::DetailType> &typesChanged);
    void insertChangedContacts(const QList<QContactId>& addedContactIds, const QList<QContactDetail::DetailType> &typesChanged);
    void clearChangedContacts();

    QSet<QContactId> removedContacts() const;
    void insertRemovedContact(QContactId addedContactId);
    void insertRemovedContacts(const QList<QContactId>& addedContactIds);
    void clearRemovedContacts();

    QSet<QContactId> addedRelationshipsContacts() const;
    void insertAddedRelationshipsContact(QContactId affectedContactId);
    void insertAddedRelationshipsContacts(const QList<QContactId>& affectedContactIds);
    void clearAddedRelationshipsContacts();

    QSet<QContactId> removedRelationshipsContacts() const;
    void insertRemovedRelationshipsContact(QContactId affectedContactId);
    void insertRemovedRelationshipsContacts(const QList<QContactId>& affectedContactIds);
    void clearRemovedRelationshipsContacts();

    void setOldAndNewSelfContactId(const QPair<QContactId, QContactId>& oldAndNewContactId);
    QPair<QContactId, QContactId> oldAndNewSelfContactId() const;

    void clearAll();

    void emitSignals(QContactManagerEngine *engine) const;

private:
    QSharedDataPointer<QContactChangeSetData> d;
};

QT_END_NAMESPACE_CONTACTS

#endif // QCONTACTCHANGESET_H
