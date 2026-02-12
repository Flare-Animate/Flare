/****************************************************************************
**
** Copyright (C) 2012 Digia Plc and/or its subsidiary(-ies).
** Contact: http://www.qt-project.org/legal
**
** This file is part of the QtDocGallery module of the Qt Toolkit.
**
** $QT_BEGIN_LICENSE:LGPL$
** Commercial License Usage
** Licensees holding valid commercial Qt licenses may use this file in
** accordance with the commercial license agreement provided with the
** Software or, alternatively, in accordance with the terms contained in
** a written agreement between you and Digia.  For licensing terms and
** conditions see http://qt.digia.com/licensing.  For further information
** use the contact form at http://qt.digia.com/contact-us.
**
** GNU Lesser General Public License Usage
** Alternatively, this file may be used under the terms of the GNU Lesser
** General Public License version 2.1 as published by the Free Software
** Foundation and appearing in the file LICENSE.LGPL included in the
** packaging of this file.  Please review the following information to
** ensure the GNU Lesser General Public License version 2.1 requirements
** will be met: http://www.gnu.org/licenses/old-licenses/lgpl-2.1.html.
**
** In addition, as a special exception, Digia gives you certain additional
** rights.  These rights are described in the Digia Qt LGPL Exception
** version 1.1, included in the file LGPL_EXCEPTION.txt in this package.
**
** GNU General Public License Usage
** Alternatively, this file may be used under the terms of the GNU
** General Public License version 3.0 as published by the Free Software
** Foundation and appearing in the file LICENSE.GPL included in the
** packaging of this file.  Please review the following information to
** ensure the GNU General Public License version 3.0 requirements will be
** met: http://www.gnu.org/copyleft/gpl.html.
**
**
** $QT_END_LICENSE$
**
****************************************************************************/

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

#ifndef QGALLERYTRACKERSCHEMA_P_H
#define QGALLERYTRACKERSCHEMA_P_H

#include "qgalleryqueryrequest.h"
#include "qgalleryfilter.h"
#include "qgalleryproperty.h"

#include "qdocumentgallery.h"

#include <QtCore/qstringlist.h>

QT_BEGIN_NAMESPACE_DOCGALLERY


class QGalleryDBusInterfaceFactory;
class QGalleryTrackerImageColumn;
class QGalleryTrackerValueColumn;

struct QGalleryTrackerResultSetArguments;

class Q_GALLERY_EXPORT QGalleryTrackerSchema
{
public:
    QGalleryTrackerSchema(const QString &itemType);
    ~QGalleryTrackerSchema();

    static QGalleryTrackerSchema fromItemId(const QString &itemId);

    bool isValid() const { return m_itemIndex >= 0; }

    QString itemType() const;

    static int serviceUpdateId(const QString &service);
    static QString serviceForType( const QString &galleryType );

    QStringList supportedPropertyNames() const;
    QGalleryProperty::Attributes propertyAttributes(const QString &propertyName) const;

    QDocumentGallery::Error prepareItemResponse(
            QGalleryTrackerResultSetArguments *arguments,
            const QString &itemId,
            const QStringList &propertyNames) const;

    QDocumentGallery::Error prepareQueryResponse(
            QGalleryTrackerResultSetArguments *arguments,
            QGalleryQueryRequest::Scope scope,
            const QString &rootItem,
            const QGalleryFilter &filter,
            const QStringList &propertyNames,
            const QStringList &sortPropertyNames,
            int offset,
            int limit) const;

    QDocumentGallery::Error prepareTypeResponse(
            QGalleryTrackerResultSetArguments *arguments) const;

private:
    QGalleryTrackerSchema(int itemIndex) : m_itemIndex(itemIndex) {}

    QDocumentGallery::Error buildFilterQuery(
            QString *query,
            QString *join,
            QString *optionalJoin,
            QGalleryQueryRequest::Scope scope,
            const QString &scopeItemId,
            const QGalleryFilter &filter) const;

    void populateItemArguments(
            QGalleryTrackerResultSetArguments *arguments,
            const QString &query,
            const QString &join,
            const QString &optionalJoin,
            const QStringList &propertyNames,
            const QStringList &sortPropertyNames,
            int offset,
            int limit) const;

    const int m_itemIndex;
};

QT_END_NAMESPACE_DOCGALLERY

#endif
