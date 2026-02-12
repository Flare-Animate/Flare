/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Copyright (C) 2015 Canonical Ltd
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

#ifndef QCONTACTCOLLECTIONCHANGESET_H
#define QCONTACTCOLLECTIONCHANGESET_H

#include <QtCore/qlist.h>
#include <QtCore/qset.h>
#include <QtCore/qshareddata.h>

#include <QtContacts/qcontactcollectionid.h>
#include <QtContacts/qcontactmanager.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContactManagerEngine;

class QContactCollectionChangeSetData;
class Q_CONTACTS_EXPORT QContactCollectionChangeSet
{
public:
    QContactCollectionChangeSet();
    QContactCollectionChangeSet(const QContactCollectionChangeSet &other);
    ~QContactCollectionChangeSet();

    QContactCollectionChangeSet &operator=(const QContactCollectionChangeSet &other);

    void setDataChanged(bool dataChanged);
    bool dataChanged() const;

    QSet<QContactCollectionId> addedCollections() const;
    void insertAddedCollection(const QContactCollectionId &collectionId);
    void insertAddedCollections(const QList<QContactCollectionId> &collectionIds);
    void clearAddedCollections();

    QSet<QContactCollectionId> changedCollections() const;
    void insertChangedCollection(const QContactCollectionId &collectionId);
    void insertChangedCollections(const QList<QContactCollectionId> &collectionIds);
    void clearChangedCollections();

    QSet<QContactCollectionId> removedCollections() const;
    void insertRemovedCollection(const QContactCollectionId &collectionId);
    void insertRemovedCollections(const QList<QContactCollectionId> &collectionIds);
    void clearRemovedCollections();

    QList<QPair<QContactCollectionId, QContactManager::Operation> > modifiedCollections() const;
    void clearModifiedCollections();

    void clearAll();

    void emitSignals(QContactManagerEngine *engine) const;

private:
    QSharedDataPointer<QContactCollectionChangeSetData> d;
};

QT_END_NAMESPACE_CONTACTS

#endif // QCONTACTCOLLECTIONCHANGESET_H
