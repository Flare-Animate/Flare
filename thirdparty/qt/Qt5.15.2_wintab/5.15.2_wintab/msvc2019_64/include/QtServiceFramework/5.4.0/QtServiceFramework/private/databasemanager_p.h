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

#ifndef DATABASEMANAGER_H_
#define DATABASEMANAGER_H_

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
#include "servicedatabase_p.h"
#include <QObject>

#ifdef QT_SFW_SERVICEDATABASE_GENERATE
#undef Q_AUTOTEST_EXPORT
#define Q_AUTOTEST_EXPORT
#endif

QT_BEGIN_NAMESPACE

class DatabaseFileWatcher;
class Q_AUTOTEST_EXPORT DatabaseManager : public QObject
{
    Q_OBJECT

    public:
        enum DbScope{UserScope, SystemScope, UserOnlyScope};
        DatabaseManager(void);
        virtual ~DatabaseManager();

        bool registerService(ServiceMetaDataResults &service, DbScope scope);
        bool unregisterService(const QString &serviceName, DbScope scope);
        bool serviceInitialized(const QString &serviceName, DbScope scope);

        QList<QServiceInterfaceDescriptor> getInterfaces(const QServiceFilter &filter, DbScope scope);
        QStringList getServiceNames(const QString &interfaceName, DbScope scope);

        QServiceInterfaceDescriptor interfaceDefault(const QString &interfaceName, DbScope scope);
        bool setInterfaceDefault(const QString &serviceName, const QString &interfaceName, DbScope scope);
        bool setInterfaceDefault(const QServiceInterfaceDescriptor &serviceInterface, DbScope scope);

        DBError lastError(){ return m_lastError;}

        void setChangeNotificationsEnabled(DbScope scope, bool enabled);

    Q_SIGNALS:
        void serviceAdded(const QString &serviceName, DatabaseManager::DbScope scope);
        void serviceRemoved(const QString &serviceName, DatabaseManager::DbScope scope);

    private:
        void initDbPath(DbScope scope);
        bool openDb(DbScope scope);

        ServiceDatabase *m_userDb;
        ServiceDatabase *m_systemDb;
        DBError m_lastError;

        friend class DatabaseFileWatcher;
        DatabaseFileWatcher *m_fileWatcher;
        QServiceInterfaceDescriptor latestDescriptor(const QList<QServiceInterfaceDescriptor> &descriptors);

        bool m_hasAccessedUserDb;
        bool m_alreadyWarnedOpenError;
};


class Q_AUTOTEST_EXPORT DatabaseFileWatcher : public QObject
{
    Q_OBJECT
public:
    DatabaseFileWatcher(DatabaseManager *parent = 0);

    void setEnabled(ServiceDatabase *database, bool enabled);

private Q_SLOTS:
    void databaseChanged(const QString &path);
    void databaseDirectoryChanged(const QString &path);

private:
    void notifyChanges(ServiceDatabase *database, DatabaseManager::DbScope scope);
    QString closestExistingParent(const QString &path);
    void restartDirMonitoring(const QString &dbPath, const QString &previousDirPath);

    DatabaseManager *m_manager;
    QFileSystemWatcher *m_watcher;
    QHash<QString, QStringList> m_knownServices;
    QStringList m_monitoredDbPaths;
};

QT_END_NAMESPACE

#endif
