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

#ifndef QORGANIZERITEMCHANGESET_P_H
#define QORGANIZERITEMCHANGESET_P_H

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

#include <QtCore/qlist.h>
#include <QtCore/qset.h>
#include <QtCore/qshareddata.h>

#include <QtOrganizer/qorganizeritemchangeset.h>
#include <QtOrganizer/qorganizeritemid.h>

QT_BEGIN_NAMESPACE_ORGANIZER

class QOrganizerItemChangeSetData : public QSharedData
{
public:
    QOrganizerItemChangeSetData()
        : QSharedData(), m_dataChanged(false)
    {
    }

    QOrganizerItemChangeSetData(const QOrganizerItemChangeSetData& other)
        : QSharedData(other),
          m_dataChanged(other.m_dataChanged),
          m_addedItems(other.m_addedItems),
          m_changedItems(other.m_changedItems),
          m_removedItems(other.m_removedItems),
          m_modifiedItems(other.m_modifiedItems)
    {
    }

    ~QOrganizerItemChangeSetData()
    {
    }

    bool m_dataChanged;
    QSet<QOrganizerItemId> m_addedItems;
    QList<QOrganizerItemChangeSet::ItemChangeList> m_changedItems;
    QSet<QOrganizerItemId> m_removedItems;
    QList<QPair<QOrganizerItemId, QOrganizerManager::Operation> > m_modifiedItems;
};

QT_END_NAMESPACE_ORGANIZER

#endif // QORGANIZERITEMCHANGESET_P_H
