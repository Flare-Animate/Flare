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

#ifndef QGALLERYTRACKERRESULTSET_P_H
#define QGALLERYTRACKERRESULTSET_P_H

#include <qgalleryresultset.h>

#include "qgallerydbusinterface_p.h"
#include "qgallerytrackerlistcolumn_p.h"

class QDBusPendingCallWatcher;

typedef struct _TrackerSparqlConnection TrackerSparqlConnection;

QT_BEGIN_NAMESPACE_DOCGALLERY

class QGalleryDBusInterfaceFactory;
class QGalleryTrackerImageColumn;
class QGalleryTrackerSchema;

class QGalleryTrackerResultSetPrivate;

struct QGalleryTrackerResultSetArguments
{
    QGalleryTrackerResultSetArguments()
        : idColumn(0)
        , urlColumn(0)
        , typeColumn(0)
        , updateMask(0)
        , identityWidth(0)
        , tableWidth(0)
        , valueOffset(0)
        , compositeOffset(0)
    {
    }

    ~QGalleryTrackerResultSetArguments()
    {
        qDeleteAll(valueColumns);
        qDeleteAll(compositeColumns);
    }

    void clear()
    {
        valueColumns.clear();
        compositeColumns.clear();
    }

    QScopedPointer<QGalleryTrackerCompositeColumn> idColumn;
    QScopedPointer<QGalleryTrackerCompositeColumn> urlColumn;
    QScopedPointer<QGalleryTrackerCompositeColumn> typeColumn;
    int updateMask;
    int identityWidth;
    int tableWidth;
    int valueOffset;
    int compositeOffset;
    QString sparql;
    QStringList propertyNames;
    QStringList fieldNames;
    QVector<QGalleryProperty::Attributes> propertyAttributes;
    QVector<QVariant::Type> propertyTypes;
    QVector<QGalleryTrackerValueColumn *> valueColumns;
    QVector<QGalleryTrackerCompositeColumn *> compositeColumns;
    QVector<int> aliasColumns;
    QVector<int> resourceKeys;
    QString service;
};

class QGalleryTrackerResultSet : public QGalleryResultSet
{
    Q_OBJECT
public:
    QGalleryTrackerResultSet(
            TrackerSparqlConnection *connection,
            QGalleryTrackerResultSetArguments *arguments,
            bool autoUpdate,
            QObject *parent = Q_NULLPTR);
    ~QGalleryTrackerResultSet();

    QStringList propertyNames() const;
    int propertyKey(const QString &property) const;
    QGalleryProperty::Attributes propertyAttributes(int key) const;
    QVariant::Type propertyType(int key) const;

    int itemCount() const;

    int currentIndex() const;
    bool fetch(int index);

    QVariant itemId() const;
    QUrl itemUrl() const;
    QString itemType() const;
    QList<QGalleryResource> resources() const;

    QVariant metaData(int key) const;
    bool setMetaData(int key, const QVariant &value);

    void cancel();

    bool waitForFinished(int msecs);

    bool event(QEvent *event);

public Q_SLOTS:
    void refresh(int serviceId = -1);

Q_SIGNALS:
    void itemEdited(const QString &service);

protected:
    QGalleryTrackerResultSet(QGalleryTrackerResultSetPrivate &dd, QObject *parent);

    void timerEvent(QTimerEvent *event);

private:
    Q_DECLARE_PRIVATE(QGalleryTrackerResultSet)
    Q_PRIVATE_SLOT(d_func(), void _q_parseFinished())
};

QT_END_NAMESPACE_DOCGALLERY

#endif
