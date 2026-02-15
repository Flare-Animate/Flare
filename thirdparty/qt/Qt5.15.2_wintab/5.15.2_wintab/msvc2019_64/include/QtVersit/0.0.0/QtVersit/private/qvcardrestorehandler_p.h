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

#ifndef QVCARDRESTOREHANDLER_P_H
#define QVCARDRESTOREHANDLER_P_H

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

#include <QtCore/qhash.h>
#include <QtCore/qlist.h>

#include <QtVersit/qversitglobal.h>

#include <QtContacts/qcontactdetail.h>

QTCONTACTS_USE_NAMESPACE

QT_BEGIN_NAMESPACE_VERSIT

class QVersitProperty;

// This is a map from Versit group names to the details that were generated from properties with the
// said groups.  Multiple details can be associated with a single group.
class DetailGroupMap
{
public:
    QList<QContactDetail> detailsInGroup(const QString& groupName) const;
    void insert(const QString& groupName, const QContactDetail& detail);
    void update(const QContactDetail& detail);
    void clear();

private:
    QHash<int, QString> mDetailGroupName; // detailid -> group name
    QHash<int, QContactDetail> mDetailById; // detailid -> detail
};

class QVCardRestoreHandler
{
public:
    bool propertyProcessed(const QVersitProperty& property,
                           QList<QContactDetail>* updatedDetails);
    void documentProcessed();

    const static QString PropertyName;
    const static QString DetailTypeParameter;
    const static QString FieldParameter;
    const static QString DatatypeParameter;
    const static QString DatatypeParameterVariant;
    const static QString DatatypeParameterDate;
    const static QString DatatypeParameterDateTime;
    const static QString DatatypeParameterTime;
    const static QString DatatypeParameterBool;
    const static QString DatatypeParameterInt;
    const static QString DatatypeParameterUInt;
    const static QString DatatypeParameterUrl;
    const static QString GroupPrefix;

private:
    static QVariant deserializeValue(const QVersitProperty& property);
    DetailGroupMap mDetailGroupMap; // remembers which details came from which groups
};

QT_END_NAMESPACE_VERSIT

#endif // QVCARDRESTOREHANDLER_P_H
