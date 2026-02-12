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

#ifndef QSERVICE_INSTANCE_MANAGER_P_H
#define QSERVICE_INSTANCE_MANAGER_P_H

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

#include <qserviceframeworkglobal.h>
#include "qremoteserviceregister.h"
#include "qremoteserviceregisterentry_p.h"
#include <private/qserviceclientcredentials_p.h>
#include <QHash>
#include <QMutexLocker>
#include <QMetaObject>
#include <QMetaClassInfo>
#include <QUuid>
#include <QDebug>

QT_BEGIN_NAMESPACE

struct ServiceIdentDescriptor
{
    ServiceIdentDescriptor() : globalInstance(0), globalRefCount(0)
    {
    }

    QExplicitlySharedDataPointer<QRemoteServiceRegisterEntryPrivate> entryData;

    QHash<QUuid, QObject*> individualInstances;
    QObject* globalInstance;
    QUuid globalId;
    int globalRefCount;
};

class Q_AUTOTEST_EXPORT InstanceManager : public QObject
{
    Q_OBJECT
public:
    InstanceManager(QObject *parent = 0);
    ~InstanceManager();

    bool addType(const QRemoteServiceRegister::Entry& entry);

    const QMetaObject* metaObject(const QRemoteServiceRegister::Entry& ident) const;
    QList<QRemoteServiceRegister::Entry> allEntries() const;

    int totalInstances() const;

    QObject* createObjectInstance(const QRemoteServiceRegister::Entry& entry, QUuid& instanceId, QServiceClientCredentials& creds);
    void removeObjectInstance(const QRemoteServiceRegister::Entry& entry, const QUuid& instanceId);

    static InstanceManager* instance();

Q_SIGNALS:
    void allInstancesClosed();
    void instanceClosed(const QRemoteServiceRegister::Entry&);
    void instanceClosed(const QRemoteServiceRegister::Entry&, const QUuid&);

private:
    mutable QMutex lock;
    QHash<QRemoteServiceRegister::Entry, ServiceIdentDescriptor> metaMap;
};



QT_END_NAMESPACE


#endif //QSERVICE_INSTANCE_MANAGER_P_H
