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

#ifndef QREMOTESERVICEREGISTER_DBUS_P_H
#define QREMOTESERVICEREGISTER_DBUS_P_H

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

#include <QUuid>
#include <QtDBus/QtDBus>

#include "qremoteserviceregister.h"
#include "instancemanager_p.h"
#include "qserviceinterfacedescriptor.h"
#include "ipcendpoint_p.h"
#include "objectendpoint_dbus_p.h"

QT_BEGIN_NAMESPACE

#define SERVER 0
#define CLIENT 1

class ObjectEndPoint;

class DBusSession: public QObject, protected QDBusContext
{
    Q_OBJECT
public:
    DBusSession(QObject* parent = 0)
        : QObject(parent)
    {
        m_accept = true;
    }
    ~DBusSession() {}

public Q_SLOTS:
    QByteArray writePackage(const QByteArray &package, int type, const QString &id) {

        QDataStream data(package);
        QServicePackage pack;
        data >> pack;

        if (type == CLIENT && pack.d->packageType == 0) {
            // Use the client DBus connection as the Id
            QDBusReply<QString> reply =
                connection().interface()->serviceOwner(message().service());
            QString clientId = reply.value();
            pack.d->payload = QVariant(clientId);

            QByteArray block;
            QDataStream out(&block, QIODevice::WriteOnly);
            out.setVersion(QDataStream::Qt_4_6);
            out << pack;

            emit packageReceived(block, type, id, -1, -1);
            return block;

        }

        if (!m_accept)
            return QByteArray();

        int pid = connection().interface()->servicePid(message().service());
        int uid = connection().interface()->serviceUid(message().service());

        emit packageReceived(package, type, id, pid, uid);
        return package;
    }

    bool processIncoming() {
        int pid = connection().interface()->servicePid(message().service());
        int uid = connection().interface()->serviceUid(message().service());
        emit newConnection(pid, uid);
        return m_accept;
    }

    void acceptIncoming(bool accept) {
        m_accept = accept;
    }

    void closeIncoming(const QString& instanceId) {
        QDBusReply<QString> reply =
            connection().interface()->serviceOwner(message().service());
        const QString& clientId = reply.value();
        emit closeConnection(clientId, instanceId);
    }

Q_SIGNALS:
    void packageReceived(const QByteArray &package, int type, const QString &id, int pid, int uid);
    void newConnection(int pid, int uid);
    void closeConnection(const QString& clientId, const QString& instanceId);

private:
    bool m_accept;
};


class QRemoteServiceRegisterDBusPrivate: public QRemoteServiceRegisterPrivate
{
    Q_OBJECT
public:
    QRemoteServiceRegisterDBusPrivate(QObject* parent);
    ~QRemoteServiceRegisterDBusPrivate();
    void publishServices(const QString& ident );

public Q_SLOTS:
    void processIncoming(int pid, int uid);

private:
    bool createServiceEndPoint(const QString& ident);

    QList<ObjectEndPoint*> pendingConnections;
    QDBusInterface *iface;
    DBusSession *session;
};

QT_END_NAMESPACE

#endif //QREMOTESERVICEREGISTER_DBUS_P_H
