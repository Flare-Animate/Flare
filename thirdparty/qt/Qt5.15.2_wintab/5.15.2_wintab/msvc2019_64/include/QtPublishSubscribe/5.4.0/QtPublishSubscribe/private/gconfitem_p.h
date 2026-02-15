/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd and/or its subsidiary(-ies).
** Contact: http://www.qt-project.org/legal
**
** This file is part of the QtSystems module of the Qt Toolkit.
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

#ifndef GCONFITEM_P_H
#define GCONFITEM_P_H

#if !defined(QT_NO_GCONFLAYER)

#include <QtCore/qvariant.h>

QT_BEGIN_NAMESPACE

class GConfItem : public QObject
{
    Q_OBJECT

public:
    explicit GConfItem(const QString &key, bool monitor = false, QObject *parent = 0);
    virtual ~GConfItem();

    void recursiveUnset();
    void set(const QVariant &val);
    void unset();
    QList<QString> listDirs() const;
    QList<QString> listEntries() const;
    QString key() const;
    QVariant value() const;
    QVariant value(const QVariant &def) const;

Q_SIGNALS:
    void subtreeChanged(const QString &key, const QVariant &value);
    void valueChanged();

private:
    friend struct GConfItemPrivate;
    struct GConfItemPrivate *priv;

    void update_value(bool emit_signal, const QString &key, const QVariant &value);
};

QT_END_NAMESPACE

#endif // QT_NO_GCONFLAYER

#endif // GCONFITEM_P_H
