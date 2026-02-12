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

#ifndef QREMOTESERVICEREGISTER_P_H
#define QREMOTESERVICEREGISTER_P_H

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

#include "qremoteserviceregister.h"
#include "instancemanager_p.h"
#include "qserviceinterfacedescriptor.h"

QT_BEGIN_NAMESPACE

class ObjectEndPoint;
class QRemoteServiceRegisterPrivate: public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool quitOnLastInstanceClosed READ quitOnLastInstanceClosed WRITE setQuitOnLastInstanceClosed)
public:
    QRemoteServiceRegisterPrivate(QObject* parent);
    virtual ~QRemoteServiceRegisterPrivate();

    virtual void publishServices(const QString& ident ) = 0;

    virtual bool quitOnLastInstanceClosed() const;
    virtual void setQuitOnLastInstanceClosed(const bool quit);

    virtual QRemoteServiceRegister::SecurityFilter setSecurityFilter(QRemoteServiceRegister::SecurityFilter filter);

    void setBaseUserIdentifier(qintptr uid);
    qintptr getBaseUserIdentifier() const;
    bool isBaseUserIdentifierSet() const;

    void setBaseGroupIdentifier(qintptr gid);
    qintptr getBaseGroupIdentifier() const;
    bool isBaseGroupIdentifierSet() const;

    void setSecurityOptions(QRemoteServiceRegister::SecurityAccessOptions options);


public Q_SLOTS:
    // Must be implemented in the subclass
    //void processIncoming();

protected:
    virtual QRemoteServiceRegister::SecurityFilter getSecurityFilter();
    QRemoteServiceRegister::SecurityAccessOptions getSecurityOptions() const;

private:
    bool m_quit;
    QRemoteServiceRegister::SecurityFilter iFilter;
    QRemoteServiceRegister::SecurityAccessOptions securityOptions;
    qintptr userIdentifier;
    bool userIdentifierSet;
    qintptr groupIdentifier;
    bool groupIdentifierSet;

public:
    // These methods apply to IPC services only
    static QObject* proxyForService(const QRemoteServiceRegister::Entry& entry, const QString& location);
    static QRemoteServiceRegisterPrivate* constructPrivateObject(QObject *parent);
    static bool isServiceRunning(const QRemoteServiceRegister::Entry&, const QString& location);
    // Create a private object based on the service type
    static QRemoteServiceRegisterPrivate* constructPrivateObject(QService::Type serviceType, QObject *parent);
};

QT_END_NAMESPACE

#endif //QREMOTESERVICEREGISTER_P_H
