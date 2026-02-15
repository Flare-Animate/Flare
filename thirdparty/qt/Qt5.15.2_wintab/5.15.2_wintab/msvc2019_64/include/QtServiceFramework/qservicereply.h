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

#ifndef QSERVICEREPLY_H
#define QSERVICEREPLY_H

#include "qserviceframeworkglobal.h"
#include "qservicemanager.h"

#include <QObject>
#include <QMetaObject>

QT_BEGIN_NAMESPACE

class QServiceReplyPrivate;

class Q_SERVICEFW_EXPORT QServiceReplyBase : public QObject
{
    Q_OBJECT
public:
    explicit QServiceReplyBase(QObject *parent = Q_NULLPTR);
    virtual ~QServiceReplyBase();

    bool isFinished() const;
    bool isRunning() const;
    QServiceManager::Error error() const;

    QString request() const;
    void setRequest(const QString &request);

Q_SIGNALS:
    void started();
    void finished();
    void errorChanged();

protected:

private Q_SLOTS:
    void start();
    void finish();
    void setError(QServiceManager::Error error);
    virtual void setProxyObject(QObject *proxyObject) = 0;

private:
    friend class QServiceOperationProcessor;

    Q_DISABLE_COPY(QServiceReplyBase)

    QServiceReplyPrivate *d;
};

class Q_SERVICEFW_EXPORT QServiceReply : public QServiceReplyBase
{
    Q_OBJECT
    public:
    explicit QServiceReply(QObject *p = Q_NULLPTR)
        : QServiceReplyBase(p),
          m_proxyObject(Q_NULLPTR)
    {
        // nothing to do
    }

    ~QServiceReply()
    {
        // nothing to do
    }

    QObject *proxyObject() const
    {
        return m_proxyObject;
    }

    void setProxyObject(QObject *newProxyObject)
    {
        m_proxyObject = newProxyObject;
    }

private:
    QObject *m_proxyObject;
};

QT_END_NAMESPACE

#endif // QSERVICEREPLY_H
