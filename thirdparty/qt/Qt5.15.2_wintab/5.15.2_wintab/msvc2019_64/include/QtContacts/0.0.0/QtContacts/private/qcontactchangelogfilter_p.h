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

#ifndef QCONTACTCHANGELOGFILTER_P_H
#define QCONTACTCHANGELOGFILTER_P_H

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

#include <QtContacts/qcontactchangelogfilter.h>
#include <QtContacts/private/qcontactfilter_p.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContactChangeLogFilterPrivate : public QContactFilterPrivate
{
public:
    QContactChangeLogFilterPrivate(QContactChangeLogFilter::EventType type = QContactChangeLogFilter::EventAdded)
        : QContactFilterPrivate()
        , m_eventType(type)
    {

    }

    QContactChangeLogFilterPrivate(const QContactChangeLogFilterPrivate& other)
        : QContactFilterPrivate(other)
        , m_eventType(other.m_eventType)
        , m_since(other.m_since)
    {

    }

    bool compare(const QContactFilterPrivate* other) const
    {
        const QContactChangeLogFilterPrivate *od = static_cast<const QContactChangeLogFilterPrivate*>(other);
        if (m_eventType != od->m_eventType)
            return false;
        if (m_since != od->m_since)
            return false;
        return true;
    }

    QDataStream& outputToStream(QDataStream& stream, quint8 formatVersion) const
    {
        if (formatVersion == 1) {
            stream << static_cast<quint32>(m_eventType) << m_since;
        }
        return stream;
    }

    QDataStream& inputFromStream(QDataStream& stream, quint8 formatVersion)
    {
        if (formatVersion == 1) {
            quint32 eventType;
            stream >> eventType >> m_since;
            m_eventType = (QContactChangeLogFilter::EventType)eventType;
        }
        return stream;
    }

#ifndef QT_NO_DEBUG_STREAM
    QDebug& debugStreamOut(QDebug& dbg) const
    {
        dbg.nospace() << "QContactChangeLogFilter(";
        dbg.nospace() << "eventType=" << static_cast<quint32>(m_eventType) << ",since=" << m_since;
        dbg.nospace() << ")";
        return dbg.maybeSpace();
    }
#endif

    Q_IMPLEMENT_CONTACTFILTER_VIRTUALCTORS(QContactChangeLogFilter, QContactFilter::ChangeLogFilter)

    QContactChangeLogFilter::EventType m_eventType;
    QDateTime m_since;
};

QT_END_NAMESPACE_CONTACTS

#endif // QCONTACTCHANGELOGFILTER_P_H
