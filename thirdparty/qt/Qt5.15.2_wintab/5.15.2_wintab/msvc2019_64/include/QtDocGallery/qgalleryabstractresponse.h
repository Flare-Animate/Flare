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

#ifndef QGALLERYABSTRACTRESPONSE_H
#define QGALLERYABSTRACTRESPONSE_H

#include <qgalleryabstractrequest.h>

#include <QtCore/qstringlist.h>

QT_BEGIN_NAMESPACE_DOCGALLERY

class QGalleryAbstractResponsePrivate;

class Q_GALLERY_EXPORT QGalleryAbstractResponse : public QObject
{
    Q_OBJECT
    Q_DECLARE_PRIVATE(QGalleryAbstractResponse)
public:
    QGalleryAbstractResponse(
            int error, const QString &errorString = QString(), QObject *parent = Q_NULLPTR);
    ~QGalleryAbstractResponse();

    int error() const;
    QString errorString() const;

    bool isActive() const;
    bool isIdle() const;

    virtual void cancel();

    virtual bool waitForFinished(int msecs);

Q_SIGNALS:
    void finished();
    void resumed();
    void canceled();
    void progressChanged(int current, int maximum);

protected:
    QGalleryAbstractResponse(QObject *parent = Q_NULLPTR);
    QGalleryAbstractResponse(QGalleryAbstractResponsePrivate &dd, QObject *parent);

    void finish(bool idle = false);
    void resume();
    void error(int error, const QString &errorString = QString());

    QScopedPointer<QGalleryAbstractResponsePrivate> d_ptr;
};

QT_END_NAMESPACE_DOCGALLERY

#endif
