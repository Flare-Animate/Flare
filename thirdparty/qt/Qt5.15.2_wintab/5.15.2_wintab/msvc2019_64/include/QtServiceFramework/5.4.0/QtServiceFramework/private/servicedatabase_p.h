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

#ifndef QSERVICEDATABASE_H_
#define QSERVICEDATABASE_H_

//
//  W A R N I N G
//  -------------
//
// This file is not part of the Qt API. It exists purely as an
// implementation detail. This header file may change from version to
// version without notice, or even be removed.
//
// We mean it.
//

#include "qserviceframeworkglobal.h"
#include <QtSql>
#include <QList>
#include "servicemetadata_p.h"
#include "qservicefilter.h"
#include "dberror_p.h"

QT_BEGIN_NAMESPACE

class QServiceInterfaceDescriptor;

class Q_AUTOTEST_EXPORT ServiceDatabase : public QObject
{
    Q_OBJECT

    public:
        ServiceDatabase(void);

        virtual ~ServiceDatabase();

        bool open();
        bool close();

        bool isOpen() const;
        void setDatabasePath(const QString &databasePath);
        QString databasePath() const;

        bool registerService(const ServiceMetaDataResults &service, const QString &securityToken = QString());
        bool unregisterService(const QString &serviceName, const QString &securityToken = QString());
        bool serviceInitialized(const QString &serviceName, const QString &securityToken = QString());

        QList<QServiceInterfaceDescriptor> getInterfaces(const QServiceFilter &filter);
        QServiceInterfaceDescriptor getInterface(const QString &interfaceID);
        QString getInterfaceID(const QServiceInterfaceDescriptor &serviceInterface);
        QStringList getServiceNames(const QString &interfaceName);

        QServiceInterfaceDescriptor interfaceDefault(const QString &interfaceName,
                                    QString *interfaceID = 0, bool inTransaction = false);
        bool setInterfaceDefault(const QServiceInterfaceDescriptor &serviceInterface,
                                const QString &externalInterfaceID = QString());
        QList<QPair<QString,QString> > externalDefaultsInfo();
        bool removeExternalDefaultServiceInterface(const QString &interfaceID);

        DBError lastError() const { return m_lastError; }

Q_SIGNALS:
        void serviceAdded(const QString& serviceName);
        void serviceRemoved(const QString& serviceName);

#ifdef QT_BUILD_INTERNAL
    public:
#else
    private:
#endif
        enum TransactionType{Read, Write};

        bool createTables();
        bool dropTables();
        bool checkTables();

        bool checkConnection();

        bool executeQuery(QSqlQuery *query, const QString &statement, const QList<QVariant> &bindValues = QList<QVariant>());
        QString getInterfaceID(QSqlQuery *query, const QServiceInterfaceDescriptor &serviceInterface);
        bool insertInterfaceData(QSqlQuery *query, const QServiceInterfaceDescriptor &anInterface, const QString &serviceID);

        bool beginTransaction(QSqlQuery *query, TransactionType);
        bool commitTransaction(QSqlQuery *query);
        bool rollbackTransaction(QSqlQuery *query);

        bool populateInterfaceProperties(QServiceInterfaceDescriptor *descriptor, const QString &interfaceID);
        bool populateServiceProperties(QServiceInterfaceDescriptor *descriptor, const QString &serviceID);

        QString m_databasePath;
        QString m_connectionName;
        bool m_isDatabaseOpen;
        bool m_inTransaction;
        DBError m_lastError;
};

QT_END_NAMESPACE

#endif /*QSERVICEDATABASE_H_*/
