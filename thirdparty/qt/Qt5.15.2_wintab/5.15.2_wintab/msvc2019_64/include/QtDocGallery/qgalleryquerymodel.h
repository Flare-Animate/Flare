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

#ifndef QGALLERYQUERYMODEL_H
#define QGALLERYQUERYMODEL_H

#include <qgalleryqueryrequest.h>

#include <QtCore/qabstractitemmodel.h>

QT_BEGIN_NAMESPACE_DOCGALLERY

class QGalleryFilter;
class QGalleryResultSet;

class QGalleryQueryModelPrivate;

class Q_GALLERY_EXPORT QGalleryQueryModel : public QAbstractItemModel
{
    Q_OBJECT
    Q_PROPERTY(QAbstractGallery *gallery READ gallery WRITE setGallery NOTIFY galleryChanged)
    Q_PROPERTY(QStringList sortPropertyNames READ sortPropertyNames WRITE setSortPropertyNames NOTIFY sortPropertyNamesChanged)
    Q_PROPERTY(bool autoUpdate READ autoUpdate WRITE setAutoUpdate NOTIFY autoUpdateChanged)
    Q_PROPERTY(int offset READ offset WRITE setOffset NOTIFY offsetChanged)
    Q_PROPERTY(int limit READ limit WRITE setLimit NOTIFY limitChanged)
    Q_PROPERTY(QString rootType READ rootType WRITE setRootType NOTIFY rootTypeChanged)
    Q_PROPERTY(QVariant rootItem READ rootItem WRITE setRootItem NOTIFY rootItemChanged)
    Q_PROPERTY(QGalleryQueryRequest::Scope scope READ scope WRITE setScope NOTIFY scopeChanged)
    Q_PROPERTY(QGalleryFilter filter READ filter WRITE setFilter NOTIFY filterChanged)
    Q_PROPERTY(int error READ error NOTIFY errorChanged)
    Q_PROPERTY(QString errorString READ errorString NOTIFY errorChanged)
    Q_PROPERTY(QGalleryAbstractRequest::State state READ state NOTIFY stateChanged)
public:
    QGalleryQueryModel(QObject *parent = Q_NULLPTR);
    QGalleryQueryModel(QAbstractGallery *gallery, QObject *parent = Q_NULLPTR);
    ~QGalleryQueryModel();

    QAbstractGallery *gallery() const;
    void setGallery(QAbstractGallery *gallery);

    QHash<int, QString> roleProperties(int column) const;
    void setRoleProperties(int column, const QHash<int, QString> &properties);

    void addColumn(const QHash<int, QString> &properties);
    void addColumn(const QString &property, int role = Qt::DisplayRole);
    void insertColumn(int index, const QHash<int, QString> &properties);
    void insertColumn(int index, const QString &property, int role = Qt::DisplayRole);
    void removeColumn(int index);

    QStringList sortPropertyNames() const;
    void setSortPropertyNames(const QStringList &names);

    bool autoUpdate() const;
    void setAutoUpdate(bool enabled);

    int offset() const;
    void setOffset(int offset);

    int limit() const;
    void setLimit(int limit);

    QString rootType() const;
    void setRootType(const QString &itemType);

    QVariant rootItem() const;
    void setRootItem(const QVariant &itemId);

    QGalleryQueryRequest::Scope scope() const;
    void setScope(QGalleryQueryRequest::Scope scope);

    QGalleryFilter filter() const;
    void setFilter(const QGalleryFilter &filter);

    void execute();
    void cancel();
    void clear();

    int error() const;
    QString errorString() const;

    QGalleryAbstractRequest::State state() const;

    QModelIndex index(int row, int column, const QModelIndex &parent = QModelIndex()) const;
    QModelIndex parent(const QModelIndex &index) const;

    int rowCount(const QModelIndex &parent = QModelIndex()) const;
    int columnCount(const QModelIndex &parent = QModelIndex()) const;

    QVariant data(const QModelIndex &index, int role = Qt::DisplayRole) const;
    bool setData(const QModelIndex &index, const QVariant &value, int role = Qt::EditRole);

    QVariant itemId(const QModelIndex &index) const;
    QUrl itemUrl(const QModelIndex &index) const;
    QString itemType(const QModelIndex &index) const;

    QVariant headerData(int section, Qt::Orientation orientation, int role = Qt::DisplayRole) const;
    bool setHeaderData(
            int section,
            Qt::Orientation orientation,
            const QVariant &value,
            int role = Qt::EditRole);

    Qt::ItemFlags flags(const QModelIndex &index) const;

Q_SIGNALS:
    void galleryChanged();
    void sortPropertyNamesChanged();
    void autoUpdateChanged();
    void offsetChanged();
    void limitChanged();
    void rootTypeChanged();
    void rootItemChanged();
    void scopeChanged();
    void filterChanged();
    void finished();
    void canceled();
    void error(int error, const QString &errorString);
    void errorChanged();
    void stateChanged(QGalleryAbstractRequest::State state);

private:
    QScopedPointer<QGalleryQueryModelPrivate> d_ptr;

    Q_DECLARE_PRIVATE(QGalleryQueryModel)
    Q_PRIVATE_SLOT(d_func(), void _q_resultSetChanged(QGalleryResultSet*))
    Q_PRIVATE_SLOT(d_func(), void _q_itemsInserted(int, int))
    Q_PRIVATE_SLOT(d_func(), void _q_itemsRemoved(int, int))
    Q_PRIVATE_SLOT(d_func(), void _q_itemsMoved(int, int, int))
    Q_PRIVATE_SLOT(d_func(), void _q_metaDataChanged(int, int, const QList<int> &))
};

QT_END_NAMESPACE_DOCGALLERY

#endif
