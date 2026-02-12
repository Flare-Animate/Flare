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

#ifndef QNETWORKINFO_H
#define QNETWORKINFO_H

#include <QtSystemInfo/qsysteminfoglobal.h>
#include <QtCore/qobject.h>
#include <QtNetwork/qnetworkinterface.h>

QT_BEGIN_NAMESPACE

#if !defined(QT_SIMULATOR)
class QNetworkInfoPrivate;
#else
class QNetworkInfoSimulator;
#endif // QT_SIMULATOR

class Q_SYSTEMINFO_EXPORT QNetworkInfo : public QObject
{
    Q_OBJECT

public:
    enum CellDataTechnology {
        UnknownDataTechnology = 0,
        GprsDataTechnology,
        EdgeDataTechnology,
        UmtsDataTechnology,
        HspaDataTechnology
    };

    enum NetworkMode {
        UnknownMode = 0,
        GsmMode,
        CdmaMode,
        WcdmaMode,
        WlanMode,
        EthernetMode,
        BluetoothMode,
        WimaxMode,
        LteMode,
        TdscdmaMode
    };

    enum NetworkStatus {
        UnknownStatus = 0,
        NoNetworkAvailable,
        EmergencyOnly,
        Searching,
        Busy,
        Denied,
        HomeNetwork,
        Roaming
        // ,Connected //desktop
    };

    explicit QNetworkInfo(QObject *parent = Q_NULLPTR);
    virtual ~QNetworkInfo();

    int networkInterfaceCount(QNetworkInfo::NetworkMode mode) const;
    int networkSignalStrength(QNetworkInfo::NetworkMode mode, int interfaceDevice) const;
    QNetworkInfo::CellDataTechnology currentCellDataTechnology(int interfaceDevice) const;
    QNetworkInfo::NetworkMode currentNetworkMode() const;
    QNetworkInfo::NetworkStatus networkStatus(QNetworkInfo::NetworkMode mode, int interfaceDevice) const;
#ifndef QT_NO_NETWORKINTERFACE
    QNetworkInterface interfaceForMode(QNetworkInfo::NetworkMode mode, int interfaceDevice) const;
#endif // QT_NO_NETWORKINTERFACE
    QString cellId(int interfaceDevice) const;
    QString currentMobileCountryCode(int interfaceDevice) const;
    QString currentMobileNetworkCode(int interfaceDevice) const;
    QString homeMobileCountryCode(int interfaceDevice) const;
    QString homeMobileNetworkCode(int interfaceDevice) const;
    QString imsi(int interfaceDevice) const;
    QString locationAreaCode(int interfaceDevice) const;
    QString macAddress(QNetworkInfo::NetworkMode mode, int interfaceDevice) const;
    QString networkName(QNetworkInfo::NetworkMode mode, int interfaceDevice) const;

Q_SIGNALS:
    void cellIdChanged(int interfaceDevice, const QString &id);
    void currentCellDataTechnologyChanged(int interfaceDevice, QNetworkInfo::CellDataTechnology tech);
    void currentMobileCountryCodeChanged(int interfaceDevice, const QString &mcc);
    void currentMobileNetworkCodeChanged(int interfaceDevice, const QString &mnc);
    void currentNetworkModeChanged(QNetworkInfo::NetworkMode mode);
    void locationAreaCodeChanged(int interfaceDevice, const QString &lac);
    void networkInterfaceCountChanged(QNetworkInfo::NetworkMode mode, int count);
    void networkNameChanged(QNetworkInfo::NetworkMode mode, int interfaceDevice, const QString &name);
    void networkSignalStrengthChanged(QNetworkInfo::NetworkMode mode, int interfaceDevice, int strength);
    void networkStatusChanged(QNetworkInfo::NetworkMode mode, int interfaceDevice, QNetworkInfo::NetworkStatus status);

protected:
    void connectNotify(const QMetaMethod &signal);
    void disconnectNotify(const QMetaMethod &signal);

private:
    Q_DISABLE_COPY(QNetworkInfo)
#if !defined(QT_SIMULATOR)
    QNetworkInfoPrivate * const d_ptr;
    Q_DECLARE_PRIVATE(QNetworkInfo)
#else
    QNetworkInfoSimulator * const d_ptr;
#endif // QT_SIMULATOR
};

QT_END_NAMESPACE

#endif // QNETWORKINFO_H
