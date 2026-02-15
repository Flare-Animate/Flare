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

#ifndef QCONTACTINTERSECTIONFILTER_P_H
#define QCONTACTINTERSECTIONFILTER_P_H

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

#include <QtContacts/qcontactintersectionfilter.h>
#include <QtContacts/private/qcontactfilter_p.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContactIntersectionFilterPrivate : public QContactFilterPrivate
{
public:
    QContactIntersectionFilterPrivate()
       : QContactFilterPrivate()
    {

    }

    QContactIntersectionFilterPrivate(const QContactIntersectionFilterPrivate& other)
       : QContactFilterPrivate(other),
       m_filters(other.m_filters)
    {

    }

    bool compare(const QContactFilterPrivate* other) const
    {
        const QContactIntersectionFilterPrivate *od = static_cast<const QContactIntersectionFilterPrivate*>(other);
        if (m_filters != od->m_filters)
            return false;
        return true;
    }

    QDataStream& outputToStream(QDataStream& stream, quint8 formatVersion) const
    {
        if (formatVersion == 1) {
            stream << m_filters;
        }
        return stream;
    }

    QDataStream& inputFromStream(QDataStream& stream, quint8 formatVersion)
    {
        if (formatVersion == 1) {
            stream >> m_filters;
        }
        return stream;
    }

#ifndef QT_NO_DEBUG_STREAM
    QDebug& debugStreamOut(QDebug& dbg) const
    {
        dbg.nospace() << "QContactIntersectionFilter(";
        dbg.nospace() << "filters=" << m_filters;
        dbg.nospace() << ")";
        return dbg.maybeSpace();
    }
#endif

    Q_IMPLEMENT_CONTACTFILTER_VIRTUALCTORS(QContactIntersectionFilter, QContactFilter::IntersectionFilter)

    QList<QContactFilter> m_filters;
};

QT_END_NAMESPACE_CONTACTS

#endif // QCONTACTINTERSECTIONFILTER_P_H
