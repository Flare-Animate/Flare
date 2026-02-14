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

#ifndef QUDEVWRAPPER_P_H
#define QUDEVWRAPPER_P_H

#include <QtCore/QObject>

#include <libudev.h>

#if !defined(QT_NO_UDEV)

QT_BEGIN_NAMESPACE

class QSocketNotifier;

class QUDevWrapper: public QObject
{
    Q_OBJECT

public:
    QUDevWrapper(QObject *parent = 0);

Q_SIGNALS:
    void driveChanged();
    void batteryDataChanged(int battery, const QByteArray &attribute, const QByteArray &value);
    void chargerTypeChanged(const QByteArray &value, bool enabled);

protected:
    void connectNotify(const QMetaMethod &signal);
    void disconnectNotify(const QMetaMethod &signal);

private:
    Q_DISABLE_COPY(QUDevWrapper)

    struct udev *udev;
    struct udev_monitor *udevMonitor;
    int udevFd;
    QSocketNotifier *notifier;
    bool watcherEnabled;
    bool watchPowerSupply;
    bool watchDrives;

    bool addUDevWatcher(const QByteArray &subsystem);
    bool removeAllUDevWatcher();

private Q_SLOTS:
    void onUDevChanges();
};

QT_END_NAMESPACE

#endif // QT_NO_UDEV

#endif /* QUDEVWRAPPER_P_H */
