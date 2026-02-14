/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtContacts module of the Qt Toolkit.
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

#ifndef QCONTACTGEOLOCATION_H
#define QCONTACTGEOLOCATION_H

#include <QtCore/qdatetime.h>

#include <QtContacts/qcontactdetail.h>

QT_BEGIN_NAMESPACE_CONTACTS

/* Leaf class */
// replaces the below
class Q_CONTACTS_EXPORT QContactGeoLocation : public QContactDetail
{
public:
#ifndef Q_QDOC
    Q_DECLARE_CUSTOM_CONTACT_DETAIL(QContactGeoLocation)
#else
    static const DetailType Type;
#endif

    enum GeoLocationField {
        FieldLabel = 0,
        FieldLatitude,
        FieldLongitude,
        FieldAccuracy,
        FieldAltitude,
        FieldAltitudeAccuracy,
        FieldHeading,
        FieldSpeed,
        FieldTimestamp
    };

    void setLabel(const QString& _label);
    QString label() const;
    void setLatitude(double _latitude);
    double latitude() const;
    void setLongitude(double _longitude);
    double longitude() const;
    void setAccuracy(double _accuracy);
    double accuracy() const;
    void setAltitude(double _altitude);
    double altitude() const;
    void setAltitudeAccuracy(double _altitudeAccuracy);
    double altitudeAccuracy() const;
    void setHeading(double _heading);
    double heading() const;
    void setSpeed(double _speed);
    double speed() const;
    void setTimestamp(const QDateTime& _timestamp);
    QDateTime timestamp() const;
};

QT_END_NAMESPACE_CONTACTS

#endif // QCONTACTGEOLOCATION_H
