/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Copyright (C) 2015 Canonical Ltd
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtContacts module of the Qt Toolkit.
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

#ifndef QCONTACTCOLLECTIONREMOVEREQUEST_H
#define QCONTACTCOLLECTIONREMOVEREQUEST_H

#include <QtCore/qlist.h>
#include <QtCore/qmap.h>

#include <QtContacts/qcontactabstractrequest.h>
#include <QtContacts/qcontactcollectionid.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContactCollectionRemoveRequestPrivate;

/* Leaf class */

class Q_CONTACTS_EXPORT QContactCollectionRemoveRequest : public QContactAbstractRequest
{
    Q_OBJECT

public:
    QContactCollectionRemoveRequest(QObject *parent = Q_NULLPTR);
    ~QContactCollectionRemoveRequest();

    void setCollectionId(const QContactCollectionId &collectionId);
    void setCollectionIds(const QList<QContactCollectionId> &collectionIds);
    QList<QContactCollectionId> collectionIds() const;

    QMap<int, QContactManager::Error> errorMap() const;

private:
    Q_DISABLE_COPY(QContactCollectionRemoveRequest)
    friend class QtContactManagerEngine;
    Q_DECLARE_PRIVATE_D(d_ptr, QContactCollectionRemoveRequest)
};

QT_END_NAMESPACE_CONTACTS

#endif // QCONTACTCOLLECTIONREMOVEREQUEST_H
