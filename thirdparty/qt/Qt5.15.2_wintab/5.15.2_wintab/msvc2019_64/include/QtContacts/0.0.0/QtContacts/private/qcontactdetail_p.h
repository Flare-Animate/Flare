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

#ifndef QCONTACTDETAIL_P_H
#define QCONTACTDETAIL_P_H

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

#include <QMap>
#include <QList>
#include <QString>
#include <QVariant>
#include <QDateTime>
#include <QAtomicInt>
#include <QSharedData>
#include <QStringList>

#include "qcontactdetail.h"

QT_BEGIN_NAMESPACE_CONTACTS

class QContactDetailPrivate : public QSharedData
{
public:
    static QAtomicInt lastDetailKey;

    // all details have these fields
    QList<int> m_contexts;

    // detail metadata
    QString m_provenance;
    QContactDetail::DetailType m_type;
    QContactDetail::AccessConstraints m_access;
    int m_detailId;
    int m_hasValueBitfield; // subclass types must set the hasValue bit for any field value which isn't stored in m_extraData.

    // extra field data
    QMap<int, QVariant> m_extraData;

    QContactDetailPrivate()
        : m_type(QContactDetail::TypeUndefined)
        , m_access(QContactDetail::NoConstraint)
        , m_detailId(lastDetailKey.fetchAndAddOrdered(1))
        , m_hasValueBitfield(0) {}
    QContactDetailPrivate(QContactDetail::DetailType detailType)
        : m_type(detailType)
        , m_access(QContactDetail::NoConstraint)
        , m_detailId(lastDetailKey.fetchAndAddOrdered(1))
        , m_hasValueBitfield(0) {}
    QContactDetailPrivate(const QContactDetailPrivate& other)
        : QSharedData(other)
        , m_contexts(other.m_contexts)
        , m_provenance(other.m_provenance)
        , m_type(other.m_type)
        , m_access(other.m_access)
        , m_detailId(other.m_detailId)
        , m_hasValueBitfield(other.m_hasValueBitfield)
        , m_extraData(other.m_extraData) {}
    virtual ~QContactDetailPrivate() {}

    virtual QContactDetailPrivate *clone() {
        // NOTE: this one must be called for extension (non-built-in) types ONLY
        // otherwise slicing will occur on detach, leading to crashes!
        Q_ASSERT(this->m_type == QContactDetail::TypeUndefined || this->m_type > QContactDetail::TypeVersion); // TypeVersion is the builtin type with largest type value.
        return new QContactDetailPrivate(*this);
    }

    virtual bool operator==(const QContactDetailPrivate& other) const { // doesn't check detailId or provenance
        if (m_type != other.m_type
                || !bitfieldsEqual(m_hasValueBitfield, other.m_hasValueBitfield, FieldProvenanceBit)
                || m_contexts != other.m_contexts
                || m_access != other.m_access
                || m_extraData.size() != other.m_extraData.size()) {
            return false;
        }

        const QMap<int, QVariant> &thisValues(values());
        const QMap<int, QVariant> &otherValues(other.values());
        int thisSize = thisValues.contains(QContactDetail::FieldProvenance) ? thisValues.size() - 1 : thisValues.size();
        int otherSize = otherValues.contains(QContactDetail::FieldProvenance) ? otherValues.size() - 1 : otherValues.size();
        if (thisSize != otherSize)
            return false;

        QMap<int, QVariant>::const_iterator it = thisValues.constBegin(), end = thisValues.constEnd();
        QMap<int, QVariant>::const_iterator otherIt;
        for ( ; it != end; ++it) {
            if (it.key() == QContactDetail::FieldProvenance)
                continue;
            otherIt = otherValues.constFind(it.key());
            if (otherIt == otherValues.constEnd())
                return false;
            if (it.value().canConvert<QList<int> >()) {
                // QList<int> values must be compared as QList<int> not as QVariant<QList<int> >...
                if (it.value().value<QList<int> >() != otherIt.value().value<QList<int> >())
                    return false;
            } else if (it.value() != otherIt.value()) {
                // Everything else can be compared directly by value.
                return false;
            }
        }
        return true;
    }
    bool operator!=(const QContactDetailPrivate& other) const {return !(other == *this);}

    static void setAccessConstraints(QContactDetail *d, QContactDetail::AccessConstraints constraint)
    {
        d->d->m_access = constraint;
    }

    static void setProvenance(QContactDetail *d, const QString &newProvenance)
    {
        d->d->m_provenance = newProvenance;
        d->d->setHasValueBitfieldBit(!newProvenance.isEmpty(), FieldProvenanceBit);
    }

    static const QContactDetailPrivate* detailPrivate(const QContactDetail& detail)
    {
        return detail.d.constData();
    }

    // built-in fields hasValue bitfield bit
    enum BuiltinFieldBitfieldBits {
        FieldContextBit = 0,
        FieldDetailUriBit,
        FieldLinkedDetailUrisBit,
        FieldProvenanceBit
    };
    // custom types can define more (for fields which are a complicated view of Field>FieldMaximumUserVisible data)
    inline bool hasValueBitfieldBitSet(unsigned int whichBit) const {
        unsigned int mask = 1 << whichBit;
        return m_hasValueBitfield & mask;
    }
    inline void setHasValueBitfieldBit(bool _value, unsigned int whichBit) {
        unsigned int mask = 1 << whichBit;
        if (_value) {
            m_hasValueBitfield |= mask;  // set
        } else {
            m_hasValueBitfield &= ~mask; // clear
        }
    }
    static inline bool bitfieldsEqual(int first, int second, unsigned int ignore) {
        unsigned int mask = 1 << ignore;
        return (first & ~mask) == (second & ~mask); // clear the ignored bit in both
    }

    virtual bool setValue(int field, const QVariant &_value) {
        switch (field) {
            case QContactDetail::FieldContext: {
                if (_value.userType() == QMetaType::Int || _value.userType() == QMetaType::UInt) {
                    m_contexts = QList<int>() << _value.toInt();
                } else {
                    m_contexts = _value.value<QList<int> >();
                }
                setHasValueBitfieldBit(true, FieldContextBit);
                return true;
            }
            case QContactDetail::FieldProvenance: {
                m_provenance = _value.toString();
                setHasValueBitfieldBit(!m_provenance.isEmpty(), FieldProvenanceBit);
                return true;
            }
            default: {
                // add the data as an extraData field
                m_extraData.insert(field, _value);
                // don't need to set hasValueBitfield bit for fields stored in extra data.
                return true;
            }
        }
    }

    virtual bool removeValue(int field) {
        switch (field) {
            case QContactDetail::FieldContext: {
                m_contexts = QList<int>();
                setHasValueBitfieldBit(false, FieldContextBit);
                return true;
            }
            case QContactDetail::FieldProvenance: {
                m_provenance = QString();
                setHasValueBitfieldBit(false, FieldProvenanceBit);
                return true;
            }
            default: {
                return m_extraData.remove(field);
                // don't need to clear hasValueBitfield bit for fields stored in extra data.
            }
        }
    }

    virtual bool hasValue(int field) const {
        switch (field) {
            case QContactDetail::FieldContext: return hasValueBitfieldBitSet(FieldContextBit);
            case QContactDetail::FieldProvenance: return hasValueBitfieldBitSet(FieldProvenanceBit);
            default: return m_extraData.contains(field);
        }
    }

    virtual bool isEmpty() const {
        return m_hasValueBitfield == 0 && m_extraData.isEmpty();
    }

    virtual QMap<int, QVariant> values() const {
        QMap<int, QVariant> retn;
        if (hasValueBitfieldBitSet(FieldContextBit)) {
            retn.insert(QContactDetail::FieldContext, QVariant::fromValue<QList<int> >(m_contexts));
        }
        if (hasValueBitfieldBitSet(FieldProvenanceBit)) {
            retn.insert(QContactDetail::FieldProvenance, QVariant::fromValue<QString>(m_provenance));
        }
        QMap<int, QVariant>::const_iterator it = m_extraData.constBegin(), end = m_extraData.constEnd();
        for ( ; it != end; ++it) {
            if (it.key() <= QContactDetail::FieldMaximumUserVisible) {
                retn.insert(it.key(), it.value());
            }
        }
        return retn;
    }

    virtual QVariant value(int field) const {
        switch (field) {
            case QContactDetail::FieldContext:          return QVariant::fromValue<QList<int> >(m_contexts);
            case QContactDetail::FieldProvenance:       return QVariant::fromValue<QString>(m_provenance);
            default:                                    return m_extraData.value(field);
        }
    }

    static QContactDetailPrivate *construct(QContactDetail::DetailType detailType);
};

class QContactDetailBuiltinPrivateBase : public QContactDetailPrivate
{
public:
    enum MemberType {
        Bool,
        Double,
        Int,
        IntList,
        String,
        StringList,
        Date,
        DateTime,
        Url,
        ByteArray,
        Variant,
    };

    struct Member {
        MemberType type;
        size_t offset;
    };

    enum { BaseFieldOffset = 4 };

    QContactDetailBuiltinPrivateBase(QContactDetail::DetailType type)
        : QContactDetailPrivate(type)
    {
    }
    QContactDetailBuiltinPrivateBase(const QContactDetailBuiltinPrivateBase& other)
        : QContactDetailPrivate(other)
    {
    }

    template<typename Subclass>
    static const void* memberAddress(const Subclass *base, size_t offset)
    {
        return reinterpret_cast<const void*>(reinterpret_cast<const char *>(base) + offset);
    }
    template<typename Subclass>
    static void* memberAddress(Subclass *base, size_t offset)
    {
        return reinterpret_cast<void*>(reinterpret_cast<char *>(base) + offset);
    }

    template<typename T, typename Subclass>
    static const T* memberVariable(const Subclass *base, size_t offset)
    {
        return reinterpret_cast<const T*>(memberAddress(base, offset));
    }
    template<typename T, typename Subclass>
    static T* memberVariable(Subclass *base, size_t offset)
    {
        return reinterpret_cast<T*>(memberAddress(base, offset));
    }

    template<typename Subclass>
    void reinitialize(Subclass* us, const Member& member)
    {
        switch (member.type) {
            case Bool:       *memberVariable<bool>(us, member.offset) = false; return;
            case Double:     *memberVariable<double>(us, member.offset) = 0.0; return;
            case Int:        *memberVariable<int>(us, member.offset) = 0; return;
            case IntList:    reinitialize<QList<int> >(us, member); return;
            case String:     reinitialize<QString>(us, member); return;
            case StringList: reinitialize<QStringList>(us, member); return;
            case Date:       reinitialize<QDate>(us, member); return;
            case DateTime:   reinitialize<QDateTime>(us, member); return;
            case Url:        reinitialize<QUrl>(us, member); return;
            case ByteArray:  reinitialize<QByteArray>(us, member); return;
            case Variant:    reinitialize<QVariant>(us, member); return;
            default: qFatal("Unsupported field type");
        }
    }

    template<typename Subclass>
    static void setValueFromVariant(Subclass* us, const QVariant& value, const Member& member)
    {
        switch (member.type) {
            case Bool:       setValueFromVariant<bool>(us, value, member); return;
            case Double:     setValueFromVariant<double>(us, value, member); return;
            case Int:        setValueFromVariant<int>(us, value, member); return;
            case IntList:    setValueFromVariant<QList<int> >(us, value, member); return;
            case String:     setValueFromVariant<QString>(us, value, member); return;
            case StringList: setValueFromVariant<QStringList>(us, value, member); return;
            case Date:       *memberVariable<QDate>(us, member.offset) = value.userType() == QMetaType::QDateTime ? value.value<QDateTime>().date() : value.value<QDate>(); return;
            case DateTime:   setValueFromVariant<QDateTime>(us, value, member); return;
            case Url:        setValueFromVariant<QUrl>(us, value, member); return;
            case ByteArray:  *memberVariable<QByteArray>(us, member.offset) = value.userType() == QMetaType::QString ? value.value<QString>().toUtf8() : value.value<QByteArray>(); return;
            case Variant:    *memberVariable<QVariant>(us, member.offset) = value; return;
            default: qFatal("Unsupported field type");
        }
    }

    template<typename Subclass>
    static bool equal(const Subclass* us, const Subclass* them, const Member& member)
    {
        switch (member.type) {
            case Bool:       return equal<bool>(us, them, member);
            case Double:     return equal<double>(us, them, member);
            case Int:        return equal<int>(us, them, member);
            case IntList:    return equal<QList<int> >(us, them, member);
            case String:     return equal<QString>(us, them, member);
            case StringList: return equal<QStringList>(us, them, member);
            case Date:       return equal<QDate>(us, them, member);
            case DateTime:   return equal<QDateTime>(us, them, member);
            case Url:        return equal<QUrl>(us, them, member);
            case ByteArray:  return equal<QByteArray>(us, them, member);
            case Variant:    return equal<QVariant>(us, them, member);
            default: qFatal("Unsupported field type");
        }
    }

    template<typename Subclass>
    static QVariant toVariant(const Subclass* us, const Member& member)
    {
        switch (member.type) {
            case Bool:       return QVariant(*memberVariable<bool>(us, member.offset));
            case Double:     return QVariant(*memberVariable<double>(us, member.offset));
            case Int:        return QVariant(*memberVariable<int>(us, member.offset));
            case IntList:    return QVariant::fromValue(*memberVariable<QList<int> >(us, member.offset));
            case String:     return QVariant(*memberVariable<QString>(us, member.offset));
            case StringList: return QVariant::fromValue(*memberVariable<QStringList>(us, member.offset));
            case Date:       return QVariant(*memberVariable<QDate>(us, member.offset));
            case DateTime: { // if the value was likely set as a QDate, then return it as a QDate.
                const QDateTime &dt(*memberVariable<QDateTime>(us, member.offset));
                return (dt.timeSpec() == Qt::LocalTime && dt.time() == QTime(0,0,0,0))
                        ? QVariant(dt.date())
                        : QVariant(dt);
                }
            case Url:        return QVariant(*memberVariable<QUrl>(us, member.offset));
            case ByteArray:  return QVariant(*memberVariable<QByteArray>(us, member.offset));
            case Variant:    return *memberVariable<QVariant>(us, member.offset);
            default: qFatal("Unsupported field type");
        }
    }

private:
    template<typename T, typename Subclass>
    void reinitialize(Subclass* us, const Member& member)
    {
        *memberVariable<T>(us, member.offset) = T();
    }

    template<typename T, typename Subclass>
    static void setValueFromVariant(Subclass* us, const QVariant& value, const Member& member)
    {
        *memberVariable<T>(us, member.offset) = value.value<T>();
    }

    template<typename T, typename Subclass>
    static bool equal(const Subclass* us, const Subclass* them, const Member& member)
    {
        return *memberVariable<T>(us, member.offset) == *memberVariable<int>(them, member.offset);
    }
};

template <typename Subclass>
class QContactDetailBuiltinPrivate : public QContactDetailBuiltinPrivateBase
{
public:
    static const Member s_members[];

    QContactDetailBuiltinPrivate(QContactDetail::DetailType type)
        : QContactDetailBuiltinPrivateBase(type)
    {
    }

    const Subclass *subclass() const
    {
        return static_cast<const Subclass *>(this);
    }
    Subclass *subclass()
    {
        return static_cast<Subclass *>(this);
    }

    QContactDetailPrivate *clone() {
        return new Subclass(*subclass());
    }

    bool operator==(const QContactDetailBuiltinPrivate& other) const
    {
        Subclass *us(subclass());
        const Subclass *them(other.subclass());
        for (int i = 0; i < Subclass::FieldCount; ++i) {
            if (!equal(us, them, s_members[i])) {
                return false;
            }
        }
        return QContactDetailPrivate::operator==(other);
    }

    template<typename T>
    const T& memberValue(int field) const
    {
        return *memberVariable<T>(subclass(), s_members[field].offset);
    }

    template<typename T>
    void setMemberValue(int field, const T& value)
    {
        *memberVariable<T>(subclass(), s_members[field].offset) = value;
        setHasValueBitfieldBit(true, field + BaseFieldOffset);
    }

    bool setValue(int field, const QVariant &value)
    {
        if (field < Subclass::FieldCount) {
            setValueFromVariant(subclass(), value, s_members[field]);
            setHasValueBitfieldBit(true, field + BaseFieldOffset);
            return true;
        }
        return QContactDetailPrivate::setValue(field, value);
    }

    bool removeValue(int field)
    {
        if (field < Subclass::FieldCount) {
            reinitialize(subclass(), s_members[field]);
            setHasValueBitfieldBit(false, field + BaseFieldOffset);
            return true;
        }
        return QContactDetailPrivate::removeValue(field);
    }

    bool hasValue(int field) const
    {
        if (field < Subclass::FieldCount) {
            return hasValueBitfieldBitSet(field + BaseFieldOffset);
        }
        return QContactDetailPrivate::hasValue(field);
    }

    QMap<int, QVariant> values() const
    {
        QMap<int, QVariant> retn = QContactDetailPrivate::values();
        for (int i = 0; i < Subclass::FieldCount; ++i) {
            if (hasValueBitfieldBitSet(i + BaseFieldOffset)) {
                retn.insert(i, value(i));
            }
        }
        return retn;
    }

    QVariant value(int field) const
    {
        if (field < Subclass::FieldCount) {
            return toVariant(subclass(), s_members[field]);
        }
        return QContactDetailPrivate::value(field);
    }
};

QT_END_NAMESPACE_CONTACTS

QT_BEGIN_NAMESPACE
// allow shared data / detach for specific detail types
template <> inline QTCONTACTS_PREPEND_NAMESPACE(QContactDetailPrivate) *QSharedDataPointer<QTCONTACTS_PREPEND_NAMESPACE(QContactDetailPrivate)>::clone()
{
    return d->clone();
}
QT_END_NAMESPACE

#endif // QCONTACTDETAIL_P_H
