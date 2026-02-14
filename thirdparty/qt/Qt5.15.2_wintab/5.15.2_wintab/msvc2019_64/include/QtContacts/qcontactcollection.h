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

#ifndef QCONTACTCOLLECTION_H
#define QCONTACTCOLLECTION_H

#include <QtCore/qmap.h>
#include <QtCore/qshareddata.h>
#include <QtCore/qvariant.h>

#include <QtContacts/qcontactcollectionid.h>
#include <QtContacts/qcontactid.h>
#include <QtContacts/qcontactdetail.h>
#include <QtContacts/qcontacttype.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContactManagerEngine;

class QContactCollectionData;
class Q_CONTACTS_EXPORT QContactCollection
{
public:
    enum MetaDataKey {
        KeyName = 0,
        KeyDescription,
        KeyColor,
        KeySecondaryColor,
        KeyImage,
        KeyExtended
    };

    QContactCollection();
    ~QContactCollection();

    QContactCollection(const QContactCollection &other);
    QContactCollection &operator=(const QContactCollection &other);

    bool operator==(const QContactCollection &other) const;
    bool operator!=(const QContactCollection &other) const {return !(other == *this);}

    QContactCollectionId id() const;
    void setId(const QContactCollectionId &id);

    void setMetaData(MetaDataKey key, const QVariant &value);
    QVariant metaData(MetaDataKey key) const;

    void setMetaData(const QMap<QContactCollection::MetaDataKey, QVariant> &metaData);
    QMap<QContactCollection::MetaDataKey, QVariant> metaData() const;

    void setExtendedMetaData(const QString &key, const QVariant &value);
    QVariant extendedMetaData(const QString &key) const;

private:
    friend Q_CONTACTS_EXPORT uint qHash(const QContactCollection &key);
    friend class QContactManagerEngine;
    QSharedDataPointer<QContactCollectionData> d;
};

Q_CONTACTS_EXPORT uint qHash(const QContactCollection &key);

#ifndef QT_NO_DEBUG_STREAM
Q_CONTACTS_EXPORT QDebug operator<<(QDebug dbg, const QContactCollection &collection);
#endif // QT_NO_DEBUG_STREAM

#ifndef QT_NO_DATASTREAM
Q_CONTACTS_EXPORT QDataStream &operator<<(QDataStream &out, const QContactCollection &collection);
Q_CONTACTS_EXPORT QDataStream &operator>>(QDataStream &in, QContactCollection &collection);
#endif // QT_NO_DATASTREAM

QT_END_NAMESPACE_CONTACTS

QT_BEGIN_NAMESPACE
Q_DECLARE_TYPEINFO(QTCONTACTS_PREPEND_NAMESPACE(QContactCollection), Q_MOVABLE_TYPE);
QT_END_NAMESPACE

#endif // QCONTACTCOLLECTION_H
