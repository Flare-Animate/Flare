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

#ifndef QCONTACTACTIONDESCRIPTOR_H
#define QCONTACTACTIONDESCRIPTOR_H

#include <QtCore/qmap.h>
#include <QtCore/qset.h>
#include <QtCore/qshareddata.h>
#include <QtCore/qstringlist.h>
#include <QtCore/qvariant.h>

#include <QtContacts/qcontactactiontarget.h>
#include <QtContacts/qcontactdetail.h>
#include <QtContacts/qcontactfilter.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContact;
class QContactActionFactory;

class QContactActionDescriptorPrivate;
class Q_CONTACTS_EXPORT QContactActionDescriptor
{
public:
    QContactActionDescriptor();
    QContactActionDescriptor(const QContactActionDescriptor& other);
    QContactActionDescriptor& operator=(const QContactActionDescriptor& other);
    ~QContactActionDescriptor();

    bool isValid() const;
    bool operator==(const QContactActionDescriptor& other) const;
    bool operator!=(const QContactActionDescriptor& other) const;

    QString actionName() const;
    QString serviceName() const;
    QString actionIdentifier() const;
    int implementationVersion() const;

    /* The descriptor provides the client with all information required in UI. */
    QSet<QContactActionTarget> supportedTargets(const QContact& contact) const;
    QContactFilter contactFilter() const;
    bool supportsContact(const QContact& contact) const;
    QVariant metaData(const QString& key, const QList<QContactActionTarget>& targets, const QVariantMap& parameters = QVariantMap()) const;

    /* Convenience meta data accessors */
    QVariant metaData(const QString& key, const QVariantMap& parameters = QVariantMap()) const;
    QVariant metaData(const QString& key, const QContactActionTarget& target, const QVariantMap& parameters = QVariantMap()) const;
    QVariant metaData(const QString& key, const QContact& contact, const QContactDetail& detail = QContactDetail(), const QVariantMap& parameters = QVariantMap()) const;

    // default meta-data keys
    static const QString MetaDataIcon;
    static const QString MetaDataLabel;
    static const QString MetaDataSecondLabel;
    static const QString MetaDataOptionalParameterKeys;
    static const QString MetaDataMandatoryParameterKeys;

private:
    QSharedDataPointer<QContactActionDescriptorPrivate> d;
    friend class QContactActionFactory;
    friend class QContactActionServiceManager;
#ifndef QT_NO_DEBUG_STREAM
    Q_CONTACTS_EXPORT friend QDebug& operator<<(QDebug dbg, const QContactActionDescriptor& descriptor);
#endif
    Q_CONTACTS_EXPORT friend uint qHash(const QContactActionDescriptor& key);
};

Q_CONTACTS_EXPORT uint qHash(const QContactActionDescriptor& key);
#ifndef QT_NO_DEBUG_STREAM
Q_CONTACTS_EXPORT QDebug& operator<<(QDebug dbg, const QContactActionDescriptor& descriptor);
#endif

QT_END_NAMESPACE_CONTACTS

QT_BEGIN_NAMESPACE
Q_DECLARE_TYPEINFO(QTCONTACTS_PREPEND_NAMESPACE(QContactActionDescriptor), Q_MOVABLE_TYPE);
QT_END_NAMESPACE

#endif // QCONTACTACTIONDESCRIPTOR_H
