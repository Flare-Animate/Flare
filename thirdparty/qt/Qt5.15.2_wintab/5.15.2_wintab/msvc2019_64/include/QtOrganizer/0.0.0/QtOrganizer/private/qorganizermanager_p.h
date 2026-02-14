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

#ifndef QCONTACTMANAGER_P_H
#define QCONTACTMANAGER_P_H

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

#include <QtCore/qhash.h>
#include <QtCore/qlist.h>
#include <QtCore/qmap.h>
#include <QtCore/qstringlist.h>

#include <QtOrganizer/qorganizermanager.h>
#include <QtOrganizer/qorganizermanagerengine.h>

QT_BEGIN_NAMESPACE_ORGANIZER

class QOrganizerItemObserver;
class QOrganizerManagerEngineFactory;

class QOrganizerManagerData
{
public:
    QOrganizerManagerData()
        : m_engine(0), m_lastError(QOrganizerManager::NoError)
    {
    }

    ~QOrganizerManagerData()
    {
        delete m_engine;
    }

    // helpers
    static bool parseUri(const QString &uriString, QString *managerName, QMap<QString, QString> *params, bool strict = true);
    static QString buildUri(const QString &managerName, const QMap<QString, QString> &params);

    static bool parseIdData(const QByteArray &idData, QString *managerName, QMap<QString, QString> *params, QString *managerUri = 0, QByteArray *localId = 0);

    static QByteArray buildIdData(const QString &managerUri, const QByteArray &localId = QByteArray());
    static QByteArray buildIdData(const QString &managerName, const QMap<QString, QString> &params, const QByteArray &localId = QByteArray());

    static QString cachedUri(const QString &managerUri);

    void createEngine(const QString &managerName, const QMap<QString, QString> &parameters);
    static QOrganizerManagerData *get(const QOrganizerManager *manager);
    static QOrganizerManagerEngine *engine(const QOrganizerManager *manager);

    QOrganizerManagerEngine *m_engine;
    QOrganizerManager::Error m_lastError;
    QMap<int, QOrganizerManager::Error> m_lastErrorMap;

    // manager plugins
    static QHash<QString, QOrganizerManagerEngineFactory *> m_engines;
    static bool m_discovered;
    static bool m_discoveredStatic;
    static QStringList m_pluginPaths;
    static void loadFactories();
    static void loadStaticFactories();

    // observer stuff
    void registerObserver(QOrganizerItemObserver *observer);
    void unregisterObserver(QOrganizerItemObserver *observer);
    void _q_itemsUpdated(const QList<QOrganizerItemId> &ids, const QList<QOrganizerItemDetail::DetailType> &typesChanged);
    void _q_itemsDeleted(const QList<QOrganizerItemId> &ids);
    QMultiHash<QOrganizerItemId, QOrganizerItemObserver *> m_observerForItem;

    // helpers
    static QOrganizerManagerData *managerData(const QOrganizerManager *m) { return m->d; }
};

/*!
    \internal

    Helper to hold the error state of a synchronous operation - when destructed, updates the
    manager's last error variables to the result of this operation.  This means that during
    callbacks the error state can't be modified behind the engines back. and it's more conceptually
    correct.
 */
class QOrganizerManagerSyncOpErrorHolder
{
public:
    QOrganizerManagerSyncOpErrorHolder(const QOrganizerManager *m, QMap<int, QOrganizerManager::Error> *pUserError = 0)
        : error(QOrganizerManager::NoError), data(QOrganizerManagerData::managerData(m)), userError(pUserError)
    {
    }

    ~QOrganizerManagerSyncOpErrorHolder()
    {
        data->m_lastError = error;
        data->m_lastErrorMap = errorMap;
        if (userError)
            *userError = errorMap;
    }

    QOrganizerManager::Error error;
    QOrganizerManagerData *data;
    QMap<int, QOrganizerManager::Error> errorMap;
    QMap<int, QOrganizerManager::Error> *userError;
};

QT_END_NAMESPACE_ORGANIZER

#endif // QCONTACTMANAGER_P_H
