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

#ifndef QORGANIZERCOLLECTION_H
#define QORGANIZERCOLLECTION_H

#include <QtCore/qmap.h>
#include <QtCore/qshareddata.h>
#include <QtCore/qvariant.h>

#include <QtOrganizer/qorganizercollectionid.h>
#include <QtOrganizer/qorganizeritemid.h>
#include <QtOrganizer/qorganizeritemdetail.h>
#include <QtOrganizer/qorganizeritemtype.h>

QT_BEGIN_NAMESPACE_ORGANIZER

class QOrganizerManagerEngine;

class QOrganizerCollectionData;
class Q_ORGANIZER_EXPORT QOrganizerCollection
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

    QOrganizerCollection();
    ~QOrganizerCollection();

    QOrganizerCollection(const QOrganizerCollection &other);
    QOrganizerCollection &operator=(const QOrganizerCollection &other);

    bool operator==(const QOrganizerCollection &other) const;
    bool operator!=(const QOrganizerCollection &other) const {return !(other == *this);}

    QOrganizerCollectionId id() const;
    void setId(const QOrganizerCollectionId &id);

    void setMetaData(MetaDataKey key, const QVariant &value);
    QVariant metaData(MetaDataKey key) const;

    void setMetaData(const QMap<QOrganizerCollection::MetaDataKey, QVariant> &metaData);
    QMap<QOrganizerCollection::MetaDataKey, QVariant> metaData() const;

    void setExtendedMetaData(const QString &key, const QVariant &value);
    QVariant extendedMetaData(const QString &key) const;

private:
    friend Q_ORGANIZER_EXPORT uint qHash(const QOrganizerCollection &key);
    friend class QOrganizerManagerEngine;
    QSharedDataPointer<QOrganizerCollectionData> d;
};

Q_ORGANIZER_EXPORT uint qHash(const QOrganizerCollection &key);

#ifndef QT_NO_DEBUG_STREAM
Q_ORGANIZER_EXPORT QDebug operator<<(QDebug dbg, const QOrganizerCollection &collection);
#endif // QT_NO_DEBUG_STREAM

#ifndef QT_NO_DATASTREAM
Q_ORGANIZER_EXPORT QDataStream &operator<<(QDataStream &out, const QOrganizerCollection &collection);
Q_ORGANIZER_EXPORT QDataStream &operator>>(QDataStream &in, QOrganizerCollection &collection);
#endif // QT_NO_DATASTREAM

QT_END_NAMESPACE_ORGANIZER

QT_BEGIN_NAMESPACE
Q_DECLARE_TYPEINFO(QTORGANIZER_PREPEND_NAMESPACE(QOrganizerCollection), Q_MOVABLE_TYPE);
QT_END_NAMESPACE

#endif // QORGANIZERCOLLECTION_H
