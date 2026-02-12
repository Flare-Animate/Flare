/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
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

#ifndef QCONTACTDETAIL_H
#define QCONTACTDETAIL_H

#include <QtCore/qmap.h>
#include <QtCore/qshareddata.h>
#include <QtCore/qstringlist.h>
#include <QtCore/qvariant.h>

#include <QtContacts/qcontactsglobal.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContact;
class QContactActionDescriptor;
class QContactDetailPrivate;
class Q_CONTACTS_EXPORT QContactDetail
{
public:
    enum DetailType {
        TypeUndefined = 0,
        TypeAddress,
        TypeAnniversary,
        TypeAvatar,
        TypeBirthday,
        TypeDisplayLabel,
        TypeEmailAddress,
        TypeExtendedDetail,
        TypeFamily,
        TypeFavorite,
        TypeGender,
        TypeGeoLocation,
        TypeGlobalPresence,
        TypeGuid,
        TypeHobby,
        TypeName,
        TypeNickname,
        TypeNote,
        TypeOnlineAccount,
        TypeOrganization,
        TypePhoneNumber,
        TypePresence,
        TypeRingtone,
        TypeSyncTarget,
        TypeTag,
        TypeTimestamp,
        TypeType,
        TypeUrl,
        TypeVersion
    };

    enum DetailContext {
        ContextHome = 0,
        ContextWork,
        ContextOther
    };

    enum DetailField {
        FieldContext = 5000, //to avoid clashing with other detail field values from leaf classes
        FieldDetailUri,
        FieldLinkedDetailUris,
        FieldProvenance,
        FieldMaximumUserVisible = 10000 // keys above this will not be reported to clients via values() etc accessors.
    };

    enum AccessConstraint {
        NoConstraint = 0,
        ReadOnly = 0x01,
        Irremovable = 0x02
    };
    Q_DECLARE_FLAGS(AccessConstraints, AccessConstraint)

protected:
    QSharedDataPointer<QContactDetailPrivate> d;

public:
    QContactDetail();
    explicit QContactDetail(DetailType _type);
    QContactDetail(const QContactDetail &other);
    ~QContactDetail();
    QContactDetail& operator=(const QContactDetail& other);

    bool operator==(const QContactDetail& other) const;
    bool operator!=(const QContactDetail& other) const { return !(other == *this); }

    DetailType type() const;

    int key() const;
    void resetKey();

    AccessConstraints accessConstraints() const;

    bool isEmpty() const;
    bool setValue(int field, const QVariant& value);
    bool removeValue(int field);
    bool hasValue(int field) const;

    void setValues(const QMap<int, QVariant> &newValues);
    QMap<int, QVariant> values() const;
    QVariant value(int field) const;
    template <typename T> T value(int field) const {
        return value(field).value<T>();
    }

    void setContexts(int newContext)
    {
        QList<int> newContexts;
        newContexts << newContext;
        setValue(FieldContext, QVariant::fromValue(newContexts));
    }

    void setContexts(const QList<int>& newContexts)
    {
        setValue(FieldContext, QVariant::fromValue(newContexts));
    }

    QList<int> contexts() const
    {
        return value< QList<int> >(FieldContext);
    }

    void setDetailUri(const QString& newDetailUri)
    {
        setValue(FieldDetailUri, newDetailUri);
    }

    QString detailUri() const
    {
        return value(FieldDetailUri).toString();
    }

    void setLinkedDetailUris(const QStringList& newLinkedDetailUris)
    {
        setValue(FieldLinkedDetailUris, newLinkedDetailUris);
    }

    void setLinkedDetailUris(const QString& linkedDetailUri)
    {
        setValue(FieldLinkedDetailUris, QStringList(linkedDetailUri));
    }

    QStringList linkedDetailUris() const
    {
        return value<QStringList>(FieldLinkedDetailUris);
    }

    QString provenance() const
    {
        return value(FieldProvenance).toString();
    }

protected:
    QContactDetail(const QContactDetail &other, DetailType expectedType);
    QContactDetail& assign(const QContactDetail &other, DetailType expectedType);

private:
    friend class QContactDetailPrivate;
    friend class QContact;
    friend class QContactManagerEngine;
#ifndef QT_NO_DATASTREAM
    Q_CONTACTS_EXPORT friend QDataStream& operator>>(QDataStream& in, QContactDetail& detail);
#endif
};

Q_CONTACTS_EXPORT uint qHash(const QContactDetail& key);
#ifndef QT_NO_DEBUG_STREAM
Q_CONTACTS_EXPORT QDebug operator<<(QDebug dbg, const QContactDetail& detail);
#endif
#ifndef QT_NO_DATASTREAM
Q_CONTACTS_EXPORT QDataStream& operator<<(QDataStream& out, const QContactDetail& detail);
Q_CONTACTS_EXPORT QDataStream& operator>>(QDataStream& in, QContactDetail& detail);
#endif

Q_DECLARE_OPERATORS_FOR_FLAGS(QContactDetail::AccessConstraints)

#define Q_DECLARE_CUSTOM_CONTACT_DETAIL(className) \
    className() : QContactDetail(Type) {} \
    className(const QContactDetail &field) : QContactDetail(field, Type) {} \
    className& operator=(const QContactDetail &other) {assign(other, Type); return *this;} \
    static const DetailType Type;

QT_END_NAMESPACE_CONTACTS

QT_BEGIN_NAMESPACE
Q_DECLARE_TYPEINFO(QTCONTACTS_PREPEND_NAMESPACE(QContactDetail), Q_MOVABLE_TYPE);
QT_END_NAMESPACE

#endif // QCONTACTDETAIL_H
