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

#ifndef QCONTACTFILTER_P_H
#define QCONTACTFILTER_P_H

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

#ifndef QT_NO_DATASTREAM
#include <QtCore/qdatastream.h>
#endif
#ifndef QT_NO_DEBUG_STREAM
#include <QtCore/qdebug.h>
#endif
#include <QtCore/qlist.h>
#include <QtCore/qshareddata.h>

#include <QtContacts/qcontactfilter.h>

/* Boiler plate code */
#define Q_IMPLEMENT_CONTACTFILTER_PRIVATE(Class) \
    Class##Private* Class::d_func() { return reinterpret_cast<Class##Private *>(d_ptr.data()); } \
    const Class##Private* Class::d_func() const { return reinterpret_cast<const Class##Private *>(d_ptr.constData()); } \
    Class::Class(const QContactFilter& other) : QContactFilter() { Class##Private::copyIfPossible(d_ptr, other); }

#define Q_IMPLEMENT_CONTACTFILTER_VIRTUALCTORS(Class, Type) \
    QContactFilterPrivate* clone() const { return new Class##Private(*this); } \
    virtual QContactFilter::FilterType type() const {return Type;} \
    static void copyIfPossible(QSharedDataPointer<QContactFilterPrivate>& d_ptr, const QContactFilter& other) \
    { \
        if (other.type() == Type) \
            d_ptr = extract_d(other); \
        else \
            d_ptr = new Class##Private; \
    }

QT_BEGIN_NAMESPACE_CONTACTS
class QContactFilterPrivate : public QSharedData
{
public:
    QContactFilterPrivate()
    {
    }

    virtual ~QContactFilterPrivate()
    {
    }

    virtual bool compare(const QContactFilterPrivate* other) const = 0;
    virtual QDataStream& outputToStream(QDataStream& stream, quint8 formatVersion) const = 0;
    virtual QDataStream& inputFromStream(QDataStream& stream, quint8 formatVersion) = 0;
#ifndef QT_NO_DEBUG_STREAM
    // NOTE: on platforms where Qt is built without debug streams enabled, vtable will differ!
    virtual QDebug& debugStreamOut(QDebug& dbg) const = 0;
#endif
    virtual QContactFilterPrivate* clone() const = 0;
    virtual QContactFilter::FilterType type() const = 0;

    /* Helper functions for C++ protection rules */
    static const QSharedDataPointer<QContactFilterPrivate>& extract_d(const QContactFilter& other) {return other.d_ptr;}
};
QT_END_NAMESPACE_CONTACTS

QT_BEGIN_NAMESPACE
#if defined(Q_CC_MWERKS)
// This results in multiple symbol definition errors on all other compilers
// but not having a definition here results in an attempt to use the unspecialized
// clone (which fails because of the pure virtuals above)
template<> QTCONTACTS_PREPEND_NAMESPACE(QContactFilterPrivate) *QSharedDataPointer<QTCONTACTS_PREPEND_NAMESPACE(QContactFilterPrivate)>::clone()
{
    return d->clone();
}
#else
template<> QTCONTACTS_PREPEND_NAMESPACE(QContactFilterPrivate) *QSharedDataPointer<QTCONTACTS_PREPEND_NAMESPACE(QContactFilterPrivate)>::clone();
#endif
QT_END_NAMESPACE

#endif // QCONTACTFILTER_P_H
