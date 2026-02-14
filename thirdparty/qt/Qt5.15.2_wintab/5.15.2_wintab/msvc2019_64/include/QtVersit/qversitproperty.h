/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtVersit module of the Qt Toolkit.
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

#ifndef QVERSITPROPERTY_H
#define QVERSITPROPERTY_H

#include <QtCore/qhash.h>
#include <QtCore/qshareddata.h>
#include <QtCore/qstringlist.h>
#include <QtCore/qvariant.h>

#include <QtVersit/qversitglobal.h>

QT_BEGIN_NAMESPACE_VERSIT

class QVersitPropertyPrivate;
class Q_VERSIT_EXPORT QVersitProperty
{
public:
    enum ValueType {
        PlainType,
        CompoundType,
        ListType,
        BinaryType,
        VersitDocumentType,
        PreformattedType
    };

    QVersitProperty();
    QVersitProperty(const QVersitProperty& other);
    ~QVersitProperty();

    QVersitProperty& operator=(const QVersitProperty& other);
    bool operator==(const QVersitProperty& other) const;
    bool operator!=(const QVersitProperty& other) const;

    void setGroups(const QStringList& groups);
    QStringList groups() const;

    void setName(const QString& name);
    QString name() const;

    void insertParameter(const QString& name, const QString& value);
    void removeParameter(const QString& name, const QString& value);
    void removeParameters(const QString& name);

    void setParameters(const QMultiHash<QString,QString>& parameters);
    QMultiHash<QString,QString> parameters() const;

    void setValue(const QVariant& value);
    QVariant variantValue() const;
    template <typename T> T value() const
    {
        return variantValue().value<T>();
    }
    QString value() const;

    void setValueType(ValueType type);
    ValueType valueType() const;

    bool isEmpty() const;
    void clear();

private:

    QSharedDataPointer<QVersitPropertyPrivate> d;
};

Q_VERSIT_EXPORT uint qHash(const QVersitProperty& key);
#ifndef QT_NO_DEBUG_STREAM
Q_VERSIT_EXPORT QDebug operator<<(QDebug dbg, const QVersitProperty& property);
#endif

QT_END_NAMESPACE_VERSIT

#endif // QVERSITPROPERTY_H
