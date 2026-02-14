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

#ifndef QCONTACTACTIONFACTORY_H
#define QCONTACTACTIONFACTORY_H

#include <QtCore/qlist.h>
#include <QtCore/qmap.h>
#include <QtCore/qobject.h>
#include <QtCore/qset.h>
#include <QtCore/qvariant.h>

#include <QtContacts/qcontactactiontarget.h>
#include <QtContacts/qcontactactiondescriptor.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContact;
class QContactAction;
class QContactFilter;

class Q_CONTACTS_EXPORT QContactActionFactory : public QObject
{
    Q_OBJECT

public:
    QContactActionFactory(QObject *parent_ = Q_NULLPTR) : QObject(parent_) {}
    virtual ~QContactActionFactory();
    virtual QList<QContactActionDescriptor> actionDescriptors() const = 0;
    virtual QContactAction* create(const QContactActionDescriptor& which) const = 0;

    // Backend functionality for QCAD
    virtual QSet<QContactActionTarget> supportedTargets(const QContact& contact, const QContactActionDescriptor& which) const = 0;
    virtual QContactFilter contactFilter(const QContactActionDescriptor& which) const = 0;
    virtual QVariant metaData(const QString& key, const QList<QContactActionTarget>& targets, const QVariantMap& parameters, const QContactActionDescriptor& which) const = 0;

    virtual bool supportsContact(const QContact& contact, const QContactActionDescriptor& which) const; // virtual but not pure virtual; default impl. calls supportedTargets.isEmpty().

    inline static const QString InterfaceName() {return QStringLiteral("org.qt-project.Qt.QContactActionFactory");};

protected:
    QContactActionDescriptor createDescriptor(const QString& actionName, const QString& serviceName, const QString& actionIdentifier, int implementationVersion) const;
};

QT_END_NAMESPACE_CONTACTS

#endif // QCONTACTACTIONFACTORY_H
