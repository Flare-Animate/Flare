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

#ifndef QSERVICEREPLY_P_H
#define QSERVICEREPLY_P_H

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
#include "qservice.h"
#include "qservicemanager.h"


#include <QAtomicInt>
#include <QAtomicPointer>
#include <QObject>
#include <QList>
#include <QPair>

QT_BEGIN_NAMESPACE

class QObject;
class QServiceManager;
class QServiceReplyPrivate
{
public:
    QServiceReplyPrivate()
        : running(false)
        , finished(false)
        , manager(0)
        , proxyObject(0)
        , error(QServiceManager::NoError)
    {
    }

    ~QServiceReplyPrivate()
    {
        // nothing to do here
    }

    bool running;
    bool finished;
    QServiceManager *manager;
    QObject *proxyObject;
    QServiceManager::Error error;
    QString request;
};

class QServiceRequest;
class QServiceOperationProcessor : public QObject
{
    Q_OBJECT
public:
    explicit QServiceOperationProcessor(QObject *parent = 0);
    ~QServiceOperationProcessor();

public Q_SLOTS:
    void handleRequest(const QServiceRequest &inrequest);

private:
    bool inRequest;
    QList<QServiceRequest> pendingList;
};

QT_END_NAMESPACE

#endif // QSERVICEREPLY_P_H
