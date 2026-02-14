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

#ifndef QORGANIZERCOLLECTIONCHANGESET_H
#define QORGANIZERCOLLECTIONCHANGESET_H

#include <QtCore/qlist.h>
#include <QtCore/qset.h>
#include <QtCore/qshareddata.h>

#include <QtOrganizer/qorganizercollectionid.h>
#include <QtOrganizer/qorganizermanager.h>

QT_BEGIN_NAMESPACE_ORGANIZER

class QOrganizerManagerEngine;

class QOrganizerCollectionChangeSetData;
class Q_ORGANIZER_EXPORT QOrganizerCollectionChangeSet
{
public:
    QOrganizerCollectionChangeSet();
    QOrganizerCollectionChangeSet(const QOrganizerCollectionChangeSet &other);
    ~QOrganizerCollectionChangeSet();

    QOrganizerCollectionChangeSet &operator=(const QOrganizerCollectionChangeSet &other);

    void setDataChanged(bool dataChanged);
    bool dataChanged() const;

    QSet<QOrganizerCollectionId> addedCollections() const;
    void insertAddedCollection(const QOrganizerCollectionId &collectionId);
    void insertAddedCollections(const QList<QOrganizerCollectionId> &collectionIds);
    void clearAddedCollections();

    QSet<QOrganizerCollectionId> changedCollections() const;
    void insertChangedCollection(const QOrganizerCollectionId &collectionId);
    void insertChangedCollections(const QList<QOrganizerCollectionId> &collectionIds);
    void clearChangedCollections();

    QSet<QOrganizerCollectionId> removedCollections() const;
    void insertRemovedCollection(const QOrganizerCollectionId &collectionId);
    void insertRemovedCollections(const QList<QOrganizerCollectionId> &collectionIds);
    void clearRemovedCollections();

    QList<QPair<QOrganizerCollectionId, QOrganizerManager::Operation> > modifiedCollections() const;
    void clearModifiedCollections();

    void clearAll();

    void emitSignals(QOrganizerManagerEngine *engine) const;

private:
    QSharedDataPointer<QOrganizerCollectionChangeSetData> d;
};

QT_END_NAMESPACE_ORGANIZER

#endif // QORGANIZERCOLLECTIONCHANGESET_H
