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

#ifndef QGALLERYITEMREQUEST_H
#define QGALLERYITEMREQUEST_H

#include <qgalleryabstractrequest.h>
#include <qgalleryproperty.h>
#include <qgalleryresource.h>

#include <QtCore/qurl.h>

QT_BEGIN_NAMESPACE_DOCGALLERY

class QGalleryFilter;
class QGalleryResultSet;

class QGalleryItemRequestPrivate;

class Q_GALLERY_EXPORT QGalleryItemRequest : public QGalleryAbstractRequest
{
    Q_OBJECT

    Q_PROPERTY(QStringList propertyNames READ propertyNames WRITE setPropertyNames NOTIFY propertyNamesChanged)
    Q_PROPERTY(bool autoUpdate READ autoUpdate WRITE setAutoUpdate NOTIFY autoUpdateChanged)
    Q_PROPERTY(QVariant itemId READ itemId WRITE setItemId NOTIFY itemIdChanged)
    Q_PROPERTY(bool valid READ isValid NOTIFY itemChanged)
    Q_PROPERTY(QString itemType READ itemType NOTIFY itemChanged)
    Q_PROPERTY(QUrl itemUrl READ itemUrl NOTIFY itemChanged)
    Q_PROPERTY(QList<QGalleryResource> resources READ resources NOTIFY itemChanged)

public:
    explicit QGalleryItemRequest(QObject *parent = Q_NULLPTR);
    explicit QGalleryItemRequest(QAbstractGallery *gallery, QObject *parent = Q_NULLPTR);
    ~QGalleryItemRequest();

    QStringList propertyNames() const;
    void setPropertyNames(const QStringList &names);

    bool autoUpdate() const;
    void setAutoUpdate(bool enabled);

    QVariant itemId() const;
    void setItemId(const QVariant &itemId);

    QGalleryResultSet *resultSet() const;

    int propertyKey(const QString &property) const;
    QGalleryProperty::Attributes propertyAttributes(int key) const;
    QVariant::Type propertyType(int key) const;

    bool isValid() const;

    QUrl itemUrl() const;
    QString itemType() const;
    QList<QGalleryResource> resources() const;

    QVariant metaData(int key) const;
    bool setMetaData(int key, const QVariant &value);

    QVariant metaData(const QString &property) const;
    bool setMetaData(const QString &property, const QVariant &value);

Q_SIGNALS:
    void propertyNamesChanged();
    void autoUpdateChanged();
    void itemIdChanged();
    void resultSetChanged(QGalleryResultSet *resultSet);
    void itemChanged();
    void metaDataChanged(const QList<int> &keys);

protected:
    void setResponse(QGalleryAbstractResponse *response);

private:
    Q_DECLARE_PRIVATE(QGalleryItemRequest)
    Q_PRIVATE_SLOT(d_func(), void _q_itemsInserted(int, int))
    Q_PRIVATE_SLOT(d_func(), void _q_itemsRemoved(int, int))
    Q_PRIVATE_SLOT(d_func(), void _q_itemsMoved(int, int, int))
    Q_PRIVATE_SLOT(d_func(), void _q_currentItemChanged())
    Q_PRIVATE_SLOT(d_func(), void _q_metaDataChanged(int, int, const QList<int> &))
};

QT_END_NAMESPACE_DOCGALLERY

#endif
