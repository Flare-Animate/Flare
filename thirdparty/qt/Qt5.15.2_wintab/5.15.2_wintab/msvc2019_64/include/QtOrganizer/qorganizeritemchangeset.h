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

#ifndef QORGANIZERITEMCHANGESET_H
#define QORGANIZERITEMCHANGESET_H

#include <QtCore/qlist.h>
#include <QtCore/qset.h>
#include <QtCore/qshareddata.h>

#include <QtOrganizer/qorganizeritemdetail.h>
#include <QtOrganizer/qorganizeritemid.h>
#include <QtOrganizer/qorganizermanager.h>

QT_BEGIN_NAMESPACE_ORGANIZER

class QOrganizerManagerEngine;

class QOrganizerItemChangeSetData;
class Q_ORGANIZER_EXPORT QOrganizerItemChangeSet
{
public:
    typedef QPair<QList<QOrganizerItemDetail::DetailType>, QList<QOrganizerItemId> > ItemChangeList;

    QOrganizerItemChangeSet();
    QOrganizerItemChangeSet(const QOrganizerItemChangeSet &other);
    ~QOrganizerItemChangeSet();

    QOrganizerItemChangeSet &operator=(const QOrganizerItemChangeSet &other);

    void setDataChanged(bool dataChanged);
    bool dataChanged() const;

    QSet<QOrganizerItemId> addedItems() const;
    void insertAddedItem(const QOrganizerItemId &itemId);
    void insertAddedItems(const QList<QOrganizerItemId> &itemIds);
    void clearAddedItems();

    QList<ItemChangeList> changedItems() const;
    void insertChangedItem(const QOrganizerItemId &itemId, const QList<QOrganizerItemDetail::DetailType> &typesChanged);
    void insertChangedItems(const QList<QOrganizerItemId> &itemIds, const QList<QOrganizerItemDetail::DetailType> &typesChanged);
    void clearChangedItems();

    QSet<QOrganizerItemId> removedItems() const;
    void insertRemovedItem(const QOrganizerItemId &itemId);
    void insertRemovedItems(const QList<QOrganizerItemId> &itemIds);
    void clearRemovedItems();

    QList<QPair<QOrganizerItemId, QOrganizerManager::Operation> > modifiedItems() const;
    void clearModifiedItems();

    void clearAll();

    void emitSignals(QOrganizerManagerEngine *engine) const;

private:
    QSharedDataPointer<QOrganizerItemChangeSetData> d;
};

QT_END_NAMESPACE_ORGANIZER

#endif // QORGANIZERITEMCHANGESET_H
