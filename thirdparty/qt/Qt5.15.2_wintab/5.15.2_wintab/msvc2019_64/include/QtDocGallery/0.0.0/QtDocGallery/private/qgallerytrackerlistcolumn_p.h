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

#ifndef QGALLERYTRACKERLISTCOLUMN_P_H
#define QGALLERYTRACKERLISTCOLUMN_P_H

#include "qgalleryglobal.h"

#include <QtCore/qshareddata.h>
#include <QtCore/qvariant.h>
#include <QtCore/qvector.h>

typedef struct _TrackerSparqlCursor TrackerSparqlCursor;

QT_BEGIN_NAMESPACE_DOCGALLERY

class QGalleryTrackerValueColumn
{
public:
    QGalleryTrackerValueColumn() : m_warned(false) {}
    virtual ~QGalleryTrackerValueColumn() {}

    virtual QVariant toVariant(TrackerSparqlCursor *cursor, int index) const = 0;
    virtual QString toString(const QVariant &variant) const { return variant.toString(); }

protected:
    mutable bool m_warned;
};

class QGalleryTrackerCompositeColumn
{
public:
    virtual ~QGalleryTrackerCompositeColumn() {}

    virtual QVariant value(QVector<QVariant>::const_iterator row) const = 0;
};

class QGalleryTrackerStringColumn : public QGalleryTrackerValueColumn
{
public:
    QVariant toVariant(TrackerSparqlCursor *cursor, int index) const;
};

class QGalleryTrackerUrlColumn : public QGalleryTrackerValueColumn
{
public:
    QVariant toVariant(TrackerSparqlCursor *cursor, int index) const;
};

class QGalleryTrackerStringListColumn : public QGalleryTrackerValueColumn
{
public:
    QGalleryTrackerStringListColumn()
        : m_separatorChar(QLatin1Char('|')), m_separatorString(QLatin1String("|")) {}
    QVariant toVariant(TrackerSparqlCursor *cursor, int index) const;
    QString toString(const QVariant &variant) const;

private:
    const QChar m_separatorChar;
    const QString m_separatorString;
};

class QGalleryTrackerIntegerColumn : public QGalleryTrackerValueColumn
{
public:
    QVariant toVariant(TrackerSparqlCursor *cursor, int index) const;
};

class QGalleryTrackerDoubleColumn : public QGalleryTrackerValueColumn
{
public:
    QVariant toVariant(TrackerSparqlCursor *cursor, int index) const;
};

class QGalleryTrackerDateTimeColumn : public QGalleryTrackerValueColumn
{
public:
    QVariant toVariant(TrackerSparqlCursor *cursor, int index) const;
    QString toString(const QVariant &variant) const;
};

class QGalleryTrackerStaticColumn : public QGalleryTrackerCompositeColumn
{
public:
    QGalleryTrackerStaticColumn(const QVariant &value) : m_value(value) {}

    QVariant value(QVector<QVariant>::const_iterator row) const;

private:
    const QVariant m_value;
};

class QGalleryTrackerPrefixColumn : public QGalleryTrackerCompositeColumn
{
public:
    QGalleryTrackerPrefixColumn(int column, const QString &prefix)
        : m_column(column), m_prefix(prefix) {}

    QVariant value(QVector<QVariant>::const_iterator row) const;

private:
    const int m_column;
    const QString m_prefix;
};

class QGalleryTrackerCompositeIdColumn : public QGalleryTrackerCompositeColumn
{
public:
    QGalleryTrackerCompositeIdColumn(const QVector<int> columns, const QString &prefix)
        : m_columns(columns), m_prefix(prefix) {}

    QVariant value(QVector<QVariant>::const_iterator row) const;

private:
    const QVector<int> m_columns;
    const QString m_prefix;
};

#define QGALLERYTRACKERFILEURLCOLUMN_DEFAULT_COL 1

class QGalleryTrackerFileUrlColumn : public QGalleryTrackerCompositeColumn
{
public:
    QGalleryTrackerFileUrlColumn(int column) : m_column(column) {}

    QVariant value(QVector<QVariant>::const_iterator row) const;

    static QGalleryTrackerCompositeColumn *create(const QVector<int> &);

private:
    const int m_column;
};

class QGalleryTrackerFilePathColumn : public QGalleryTrackerCompositeColumn
{
public:
    QVariant value(QVector<QVariant>::const_iterator row) const;

    static QGalleryTrackerCompositeColumn *create(const QVector<int> &columns);
};

class QGalleryTrackerPathColumn : public QGalleryTrackerCompositeColumn
{
public:
    QVariant value(QVector<QVariant>::const_iterator row) const;

    static QGalleryTrackerCompositeColumn *create(const QVector<int> &columns);
};

class QGalleryTrackerFileExtensionColumn : public QGalleryTrackerCompositeColumn
{
public:
    QGalleryTrackerFileExtensionColumn(int column) : m_column(column) {}

    QVariant value(QVector<QVariant>::const_iterator row) const;

    static QGalleryTrackerCompositeColumn *create(const QVector<int> &);

private:
    const int m_column;
};


class QGalleryTrackerOrientationColumn : public QGalleryTrackerCompositeColumn
{
public:
    QGalleryTrackerOrientationColumn(int column)
        : m_column(column) {}

    QVariant value(QVector<QVariant>::const_iterator row) const;

    static QGalleryTrackerCompositeColumn *create(const QVector<int> &);

private:
    const int m_column;
};


QT_END_NAMESPACE_DOCGALLERY

#endif
