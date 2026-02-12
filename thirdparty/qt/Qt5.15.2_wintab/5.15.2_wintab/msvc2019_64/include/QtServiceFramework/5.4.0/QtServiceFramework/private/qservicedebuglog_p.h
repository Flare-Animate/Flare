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
#ifndef QSERVICEDEBUGLOG_P_H
#define QSERVICEDEBUGLOG_P_H

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

#include <QString>
#include <QStringList>
#include <QBuffer>
#include <QDataStream>
#include <QSharedPointer>
#include <QMutex>
#include <QVector>

QT_BEGIN_NAMESPACE

class QServiceDebugMessage
{
public:
    enum DataType {
        Int32Type = 1,
        FloatType = 2,
        StringType = 3
    };

    QServiceDebugMessage();
    ~QServiceDebugMessage();

#ifdef QT_SFW_IPC_DEBUG
    QBuffer *buffer;
    QDataStream ds;
#endif
};

class QServiceDebugValue;
class QServiceDebugKey;
class QUdpSocket;

class QServiceDebugLog
{
public:
    QServiceDebugLog();

    static QServiceDebugLog* instance();

    QServiceDebugValue operator<<(const char *key);
    void logMessage(QServiceDebugMessage *msg);

private:
    QList<QBuffer *> queue;
    QVector<QUdpSocket *> sockets;
    void makeSockets();
    QMutex socketLock;
};

inline QServiceDebugLog &qServiceLog()
{
    return (*QServiceDebugLog::instance());
}

class QServiceDebugKey
{
public:
#ifdef QT_SFW_IPC_DEBUG
    inline QServiceDebugKey(const QSharedPointer<QServiceDebugMessage> &ptr)
        : ptr(ptr) {}

#else
    inline QServiceDebugKey() {}
#endif
    QServiceDebugValue operator<<(const char *key);
private:
#ifdef QT_SFW_IPC_DEBUG
    QSharedPointer<QServiceDebugMessage> ptr;
#endif
};

class QServiceDebugValue
{
public:
#ifdef QT_SFW_IPC_DEBUG
    inline QServiceDebugValue(const QSharedPointer<QServiceDebugMessage> &ptr)
        : ptr(ptr) {}
#else
    inline QServiceDebugValue() {}
#endif
    QServiceDebugKey operator<<(const qint32 &val);
    QServiceDebugKey operator<<(const float &val);
    QServiceDebugKey operator<<(const QString &val);
    QServiceDebugKey operator<<(const char *val);
private:
#ifdef QT_SFW_IPC_DEBUG
    QSharedPointer<QServiceDebugMessage> ptr;
#endif
};

inline QServiceDebugValue QServiceDebugKey::operator<<(const char *key)
{
#ifdef QT_SFW_IPC_DEBUG
    ptr->ds.writeBytes(key, ::strlen(key));
    return QServiceDebugValue(ptr);
#else
    Q_UNUSED(key)
    return QServiceDebugValue();
#endif
}

inline QServiceDebugKey QServiceDebugValue::operator<<(const qint32 &val)
{
#ifdef QT_SFW_IPC_DEBUG
    ptr->ds << (qint8)QServiceDebugMessage::Int32Type;
    ptr->ds << val;
    return QServiceDebugKey(ptr);
#else
    Q_UNUSED(val)
    return QServiceDebugKey();
#endif
}

inline QServiceDebugKey QServiceDebugValue::operator<<(const float &val)
{
#ifdef QT_SFW_IPC_DEBUG
    ptr->ds << (qint8)QServiceDebugMessage::FloatType;
    ptr->ds << val;
    return QServiceDebugKey(ptr);
#else
    Q_UNUSED(val)
    return QServiceDebugKey();
#endif
}

inline QServiceDebugKey QServiceDebugValue::operator<<(const QString &val)
{
#ifdef QT_SFW_IPC_DEBUG
    ptr->ds << (qint8)QServiceDebugMessage::StringType;
    QByteArray ba = val.toLatin1();
    ptr->ds.writeBytes(ba.constData(), ba.size());
    return QServiceDebugKey(ptr);
#else
    Q_UNUSED(val)
    return QServiceDebugKey();
#endif
}

inline QServiceDebugKey QServiceDebugValue::operator<<(const char *val)
{
#ifdef QT_SFW_IPC_DEBUG
    ptr->ds << (qint8)QServiceDebugMessage::StringType;
    ptr->ds.writeBytes(val, ::strlen(val));
    return QServiceDebugKey(ptr);
#else
    Q_UNUSED(val)
    return QServiceDebugKey();
#endif
}

inline QServiceDebugValue QServiceDebugLog::operator<<(const char *key)
{
#ifdef QT_SFW_IPC_DEBUG
    QSharedPointer<QServiceDebugMessage> msg(new QServiceDebugMessage());
    return (QServiceDebugKey(msg) << key);
#else
    Q_UNUSED(key)
    return QServiceDebugValue();
#endif
}

QT_END_NAMESPACE

#endif // QSERVICEDEBUGLOG_P_H
