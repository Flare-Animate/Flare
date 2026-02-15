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

#ifndef QVALUESPACESUBSCRIBER_H
#define QVALUESPACESUBSCRIBER_H

#include <QtPublishSubscribe/qvaluespace.h>

#include <QtCore/qshareddata.h>
#include <QtCore/qvariant.h>

QT_BEGIN_NAMESPACE

class QValueSpaceSubscriberPrivate;

class Q_PUBLISHSUBSCRIBE_EXPORT QValueSpaceSubscriber : public QObject
{
    Q_OBJECT

    Q_PROPERTY(QString path READ path WRITE setPath)
    Q_PROPERTY(QVariant value READ valuex NOTIFY contentsChanged)

public:
    explicit QValueSpaceSubscriber(QObject *parent = Q_NULLPTR);
    explicit QValueSpaceSubscriber(const QString &path, QObject *parent = Q_NULLPTR);
    explicit QValueSpaceSubscriber(QValueSpace::LayerOptions filter, const QString &path, QObject *parent = Q_NULLPTR);
    explicit QValueSpaceSubscriber(const QUuid &uuid, const QString &path, QObject *parent = Q_NULLPTR);
    virtual ~QValueSpaceSubscriber();

    bool isConnected() const;
    void cd(const QString &path);
    void cdUp();
    void setPath(const QString &path);
    void setPath(QValueSpaceSubscriber *subscriber);
    QString path() const;
    QStringList subPaths() const;
    QVariant value(const QString &subPath = QString(), const QVariant &def = QVariant()) const;

Q_SIGNALS:
    void contentsChanged();

protected:
    virtual void connectNotify(const QMetaMethod &signal);
    virtual void disconnectNotify(const QMetaMethod &signal);

private:
    QVariant valuex(const QVariant &def = QVariant()) const;

private:
    Q_DISABLE_COPY(QValueSpaceSubscriber)
    QExplicitlySharedDataPointer<QValueSpaceSubscriberPrivate> d;
};

QT_END_NAMESPACE

#endif // QVALUESPACESUBSCRIBER_H
