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

#ifndef QGALLERYRESULTSET_H
#define QGALLERYRESULTSET_H

#include <qgalleryabstractresponse.h>
#include <qgalleryproperty.h>

#include <QtCore/qmap.h>
#include <QtCore/qobject.h>
#include <QtCore/qurl.h>
#include <QtCore/qvariant.h>

QT_BEGIN_NAMESPACE_DOCGALLERY

class QGalleryResource;

class QGalleryResultSetPrivate;

class Q_GALLERY_EXPORT QGalleryResultSet : public QGalleryAbstractResponse
{
    Q_OBJECT
    Q_DECLARE_PRIVATE(QGalleryResultSet)
public:
    QGalleryResultSet(QObject *parent = Q_NULLPTR);
    ~QGalleryResultSet();

    virtual int propertyKey(const QString &property) const = 0;
    virtual QGalleryProperty::Attributes propertyAttributes(int key) const = 0;
    virtual QVariant::Type propertyType(int key) const = 0;

    virtual int itemCount() const = 0;

    virtual bool isValid() const;

    virtual QVariant itemId() const = 0;
    virtual QUrl itemUrl() const = 0;
    virtual QString itemType() const = 0;
    virtual QList<QGalleryResource> resources() const;

    virtual QVariant metaData(int key) const = 0;
    virtual bool setMetaData(int key, const QVariant &value) = 0;

    virtual int currentIndex() const = 0;
    virtual bool fetch(int index) = 0;
    virtual bool fetchNext();
    virtual bool fetchPrevious();
    virtual bool fetchFirst();
    virtual bool fetchLast();

Q_SIGNALS:
    void currentItemChanged();
    void currentIndexChanged(int index);
    void itemsInserted(int index, int count);
    void itemsRemoved(int index, int count);
    void itemsMoved(int from, int to, int count);

    void metaDataChanged(int index, int count, const QList<int> &keys);

protected:
    QGalleryResultSet(QGalleryResultSetPrivate &dd, QObject *parent);
};

QT_END_NAMESPACE_DOCGALLERY

#endif
