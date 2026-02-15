/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd and/or its subsidiary(-ies).
** Contact: http://www.qt-project.org/legal
**
** This file is part of the QtSystems module of the Qt Toolkit.
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

#ifndef GCONFLAYER_P_H
#define GCONFLAYER_P_H

#if !defined(QT_NO_GCONFLAYER)

#include <qvaluespacepublisher.h>

#include "gconfitem_p.h"
#include "qvaluespace_p.h"

#include <QtCore/qmutex.h>
#include <QtCore/qset.h>

QT_BEGIN_NAMESPACE

class GConfLayer : public QAbstractValueSpaceLayer
{
    Q_OBJECT

public:
    GConfLayer();
    virtual ~GConfLayer();

    static GConfLayer *instance();

protected:
    bool value(Handle handle, QVariant *data);
    bool value(Handle handle, const QString &subPath, QVariant *data);
    void removeHandle(Handle handle);
    void setProperty(Handle handle, Properties properties);
    Handle item(Handle parent, const QString &subPath);
    QSet<QString> children(Handle handle);
    QUuid id();
    QValueSpace::LayerOptions layerOptions() const;

    // QValueSpaceSubscriber functions
    bool notifyInterest(Handle handle, bool interested);
    bool supportsInterestNotification() const;

    // QValueSpacePublisher functions
    void addWatch(QValueSpacePublisher *creator, Handle handle);
    bool removeSubTree(QValueSpacePublisher *creator, Handle handle);
    void removeWatches(QValueSpacePublisher *creator, Handle parent);
    bool removeValue(QValueSpacePublisher *creator, Handle handle, const QString &subPath);
    bool setValue(QValueSpacePublisher *creator, Handle handle, const QString &subPath, const QVariant &value);
    void sync();

private Q_SLOTS:
    void notifyChanged(const QString &key, const QVariant &value);

private:
    struct GConfHandle
    {
        GConfHandle(const QString &p)
            : path(p), refCount(1)
        {
        }

        QString path;
        unsigned int refCount;
    };

    QHash<QString, GConfHandle *> m_handles;

    GConfHandle *gConfHandle(Handle handle)
    {
        if (handle == InvalidHandle)
            return 0;

        GConfHandle *h = reinterpret_cast<GConfHandle *>(handle);
        if (m_handles.values().contains(h))
            return h;

        return 0;
    }

    //private methods not locking a mutex
    bool getValue(Handle handle, const QString &subPath, QVariant *data);
    void doRemoveHandle(Handle handle);
    Handle getItem(Handle parent, const QString &subPath);

private:
    QMap<QString, GConfItem *> m_monitoringItems;
    QMutex m_mutex;
    QSet<GConfHandle *> m_monitoringHandles;
};

QT_END_NAMESPACE

#endif // QT_NO_GCONFLAYER

#endif //GCONFLAYER_P_H
