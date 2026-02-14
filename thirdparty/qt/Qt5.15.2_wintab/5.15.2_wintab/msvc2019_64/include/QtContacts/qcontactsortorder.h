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

#ifndef QCONTACTSORTORDER_H
#define QCONTACTSORTORDER_H

#include <QtCore/qlist.h>
#include <QtCore/qshareddata.h>

#include <QtContacts/qcontactdetail.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContactSortOrderPrivate;
class Q_CONTACTS_EXPORT QContactSortOrder
{
public:
    QContactSortOrder();
    ~QContactSortOrder();

    QContactSortOrder(const QContactSortOrder& other);
    QContactSortOrder& operator=(const QContactSortOrder& other);

    enum BlankPolicy {
        BlanksFirst,
        BlanksLast
    };

    /* Mutators */
    void setDetailType(QContactDetail::DetailType type, int field = -1);
    void setBlankPolicy(BlankPolicy blankPolicy);
    void setDirection(Qt::SortOrder direction);
    void setCaseSensitivity(Qt::CaseSensitivity sensitivity);

    /* Accessors */
    QContactDetail::DetailType detailType() const;
    int detailField() const;
    BlankPolicy blankPolicy() const;
    Qt::SortOrder direction() const;
    Qt::CaseSensitivity caseSensitivity() const;

    bool isValid() const;

    bool operator==(const QContactSortOrder& other) const;
    bool operator!=(const QContactSortOrder& other) const {return !operator==(other);}

    /* Convenience cast */
    operator QList<QContactSortOrder>() const {return QList<QContactSortOrder>() << *this;}

private:
    QSharedDataPointer<QContactSortOrderPrivate> d;
};

#ifndef QT_NO_DATASTREAM
Q_CONTACTS_EXPORT QDataStream& operator<<(QDataStream& out, const QContactSortOrder& sortOrder);
Q_CONTACTS_EXPORT QDataStream& operator>>(QDataStream& in, QContactSortOrder& sortOrder);
#endif

#ifndef QT_NO_DEBUG_STREAM
Q_CONTACTS_EXPORT QDebug operator<<(QDebug dbg, const QContactSortOrder& sortOrder);
#endif

QT_END_NAMESPACE_CONTACTS

QT_BEGIN_NAMESPACE
Q_DECLARE_TYPEINFO(QTCONTACTS_PREPEND_NAMESPACE(QContactSortOrder), Q_MOVABLE_TYPE);
QT_END_NAMESPACE

#endif // QCONTACTSORTORDER_H
