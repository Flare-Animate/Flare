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

#ifndef QSERVICE_PACKAGE_H
#define QSERVICE_PACKAGE_H

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

#include "qserviceframeworkglobal.h"
#include <QExplicitlySharedDataPointer>
#include <QSharedData>
#include <QUuid>
#include <QVariant>
#include "qserviceclientcredentials_p.h"
#include "qremoteserviceregister.h"

QT_BEGIN_NAMESPACE

class QDataStream;
class QDebug;

class QServicePackagePrivate;
class Q_AUTOTEST_EXPORT QServicePackage
{
    Q_GADGET
public:
    QServicePackage();
    QServicePackage(const QServicePackage& other);
    QServicePackage& operator=(const QServicePackage& other);
    ~QServicePackage();

    enum Type {
        ObjectCreation = 0,
        MethodCall,
        PropertyCall
    };
    Q_ENUMS(Type)

    enum ResponseType {
        NotAResponse = 0,
        Success,
        Failed
    };
    Q_ENUMS(ResponseType)

    QServicePackage createResponse() const;

    bool isValid() const;

    QExplicitlySharedDataPointer<QServicePackagePrivate> d;

#ifndef QT_NO_DATASTREAM
    friend QDataStream &operator<<(QDataStream &, const QServicePackage&);
    friend QDataStream &operator>>(QDataStream &, QServicePackage&);
#endif
};

#ifndef QT_NO_DATASTREAM
QDataStream &operator<<(QDataStream &, const QServicePackage&);
QDataStream &operator>>(QDataStream &, QServicePackage&);
#endif

#ifndef QT_NO_DEBUG_STREAM
QDebug operator<<(QDebug, const QServicePackage&);
#endif

class QServicePackagePrivate : public QSharedData
{
public:
    QServicePackagePrivate()
        :   packageType(QServicePackage::ObjectCreation),
            entry(QRemoteServiceRegister::Entry()), payload(QVariant()),
            messageId(QUuid()), instanceId(QUuid()), responseType(QServicePackage::NotAResponse)
    {
    }

    QServicePackage::Type packageType;
    QRemoteServiceRegister::Entry entry;
    QVariant payload;
    QUuid messageId;
    QUuid instanceId;
    QServicePackage::ResponseType responseType;

    void clean()
    {
        packageType = QServicePackage::ObjectCreation;
        messageId = QUuid();
        instanceId = QUuid();
        payload = QVariant();
        entry = QRemoteServiceRegister::Entry();
        responseType = QServicePackage::NotAResponse;
    }
};

QT_END_NAMESPACE

#endif //QSERVICE_PACKAGE_H
