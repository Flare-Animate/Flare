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

#ifndef QGALLERYFILTER_H
#define QGALLERYFILTER_H

#include "qgalleryglobal.h"

#include <QtCore/qset.h>
#include <QtCore/qshareddata.h>
#include <QtCore/qvariant.h>
#include <QtCore/qvector.h>

QT_BEGIN_NAMESPACE
class QDebug;
QT_END_NAMESPACE

QT_BEGIN_NAMESPACE_DOCGALLERY

class QGalleryFilter;
class QGalleryIntersectionFilter;
class QGalleryMetaDataFilter;
class QGalleryUnionFilter;
class QGalleryFilterPrivate;

class Q_GALLERY_EXPORT QGalleryFilter
{
public:
    enum Type
    {
        Invalid,
        Intersection,
        Union,
        MetaData
    };

    enum Comparator
    {
        Equals,
        LessThan,
        GreaterThan,
        LessThanEquals,
        GreaterThanEquals,
        Contains,
        StartsWith,
        EndsWith,
        Wildcard,
        RegExp
    };

    QGalleryFilter();
    QGalleryFilter(const QGalleryFilter &filter);
    QGalleryFilter(const QGalleryIntersectionFilter &filter);
    QGalleryFilter(const QGalleryUnionFilter &filter);
    QGalleryFilter(const QGalleryMetaDataFilter &filter);
    ~QGalleryFilter();

    QGalleryFilter &operator =(const QGalleryFilter &filter);

    Type type() const;
    bool isValid() const;

    QGalleryIntersectionFilter toIntersectionFilter() const;
    QGalleryUnionFilter toUnionFilter() const;
    QGalleryMetaDataFilter toMetaDataFilter() const;

private:
    QSharedDataPointer<QGalleryFilterPrivate> d;

    friend Q_GALLERY_EXPORT bool operator ==(
            const QGalleryFilter &filter1, const QGalleryFilter &filter2);
    friend Q_GALLERY_EXPORT bool operator !=(
            const QGalleryFilter &filter1, const QGalleryFilter &filter2);

#ifndef QT_NO_DEBUG_STREAM
    friend Q_GALLERY_EXPORT QDebug operator <<(QDebug debug, const QGalleryFilter &filter);
#endif
};

Q_GALLERY_EXPORT bool operator ==(const QGalleryFilter &filter1, const QGalleryFilter &filter2);
Q_GALLERY_EXPORT bool operator !=(const QGalleryFilter &filter1, const QGalleryFilter &filter2);

#ifndef QT_NO_DEBUG_STREAM
Q_GALLERY_EXPORT QDebug operator <<(QDebug debug, const QGalleryFilter &filter);
#endif

class QGalleryIntersectionFilterPrivate;

class Q_GALLERY_EXPORT QGalleryIntersectionFilter
{
public:
    QGalleryIntersectionFilter();
    QGalleryIntersectionFilter(const QGalleryMetaDataFilter &filter);
    QGalleryIntersectionFilter(const QGalleryUnionFilter &filter);
    QGalleryIntersectionFilter(const QGalleryIntersectionFilter &filter);
    ~QGalleryIntersectionFilter();

    QGalleryIntersectionFilter &operator =(const QGalleryIntersectionFilter &filter);

    bool isValid() const;

    int filterCount() const;
    bool isEmpty() const;

    QList<QGalleryFilter> filters() const;

    void append(const QGalleryMetaDataFilter &filter);
    void append(const QGalleryUnionFilter &filter);
    void append(const QGalleryIntersectionFilter &filter);

    void prepend(const QGalleryMetaDataFilter &filter);
    void prepend(const QGalleryUnionFilter &filter);
    void prepend(const QGalleryIntersectionFilter &filter);

    void insert(int index, const QGalleryMetaDataFilter &filter);
    void insert(int index, const QGalleryUnionFilter &filter);
    void insert(int index, const QGalleryIntersectionFilter &filter);

    void replace(int index, const QGalleryMetaDataFilter &filter);
    void replace(int index, const QGalleryUnionFilter &filter);

    void remove(int index);
    void clear();

    QGalleryIntersectionFilter &operator <<(const QGalleryIntersectionFilter &filter);

private:
    explicit inline QGalleryIntersectionFilter(QGalleryFilterPrivate *d);
    explicit inline QGalleryIntersectionFilter(QGalleryFilter::Type type);

    QSharedDataPointer<QGalleryIntersectionFilterPrivate> d;

    friend class QGalleryFilter;
};

QGalleryIntersectionFilter Q_GALLERY_EXPORT operator &&(
        const QGalleryIntersectionFilter &filter1, const QGalleryIntersectionFilter &filter2);

class QGalleryUnionFilterPrivate;

class Q_GALLERY_EXPORT QGalleryUnionFilter
{
public:
    QGalleryUnionFilter();
    QGalleryUnionFilter(const QGalleryMetaDataFilter &filter);
    QGalleryUnionFilter(const QGalleryIntersectionFilter &filter);
    QGalleryUnionFilter(const QGalleryUnionFilter &other);
    ~QGalleryUnionFilter();

    QGalleryUnionFilter &operator =(const QGalleryUnionFilter &filter);

    bool isValid() const;

    int filterCount() const;
    bool isEmpty() const;

    QList<QGalleryFilter> filters() const;

    void append(const QGalleryMetaDataFilter &filter);
    void append(const QGalleryIntersectionFilter &filter);
    void append(const QGalleryUnionFilter &filter);

    void prepend(const QGalleryMetaDataFilter &filter);
    void prepend(const QGalleryIntersectionFilter &filter);
    void prepend(const QGalleryUnionFilter &filter);

    void insert(int index, const QGalleryMetaDataFilter &filter);
    void insert(int index, const QGalleryIntersectionFilter &filter);
    void insert(int index, const QGalleryUnionFilter &filter);

    void replace(int index, const QGalleryMetaDataFilter &filter);
    void replace(int index, const QGalleryIntersectionFilter &filter);

    void remove(int index);
    void clear();

    QGalleryUnionFilter &operator <<(const QGalleryUnionFilter &filter);

private:
    explicit inline QGalleryUnionFilter(QGalleryFilterPrivate *d);
    explicit inline QGalleryUnionFilter(QGalleryFilter::Type type);

    QSharedDataPointer<QGalleryUnionFilterPrivate> d;

    friend class QGalleryFilter;
};

QGalleryUnionFilter Q_GALLERY_EXPORT operator ||(
        const QGalleryUnionFilter &filter1, const QGalleryUnionFilter &filter2);

class QGalleryMetaDataFilterPrivate;

class Q_GALLERY_EXPORT QGalleryMetaDataFilter
{
public:
    QGalleryMetaDataFilter();
    QGalleryMetaDataFilter(
            const QString &propertyName,
            const QVariant &value,
            QGalleryFilter::Comparator comparator = QGalleryFilter::Equals);
    QGalleryMetaDataFilter(const QGalleryMetaDataFilter &other);
    ~QGalleryMetaDataFilter();

    QGalleryMetaDataFilter &operator =(const QGalleryMetaDataFilter &filter);

    bool isValid() const;

    QString propertyName() const;
    void setPropertyName(const QString &name);

    QVariant value() const;
    void setValue(const QVariant &value);

    QGalleryFilter::Comparator comparator() const;
    void setComparator(QGalleryFilter::Comparator comparator);

    bool isNegated() const;
    void setNegated(bool inverted);

    QGalleryMetaDataFilter operator !() const;

private:
    explicit inline QGalleryMetaDataFilter(QGalleryFilterPrivate *d);
    explicit inline QGalleryMetaDataFilter(QGalleryFilter::Type type);

    QSharedDataPointer<QGalleryMetaDataFilterPrivate> d;

    friend class QGalleryFilter;
};

QT_END_NAMESPACE_DOCGALLERY

#endif
