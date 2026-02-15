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

#ifndef QSERVICEFILTER_H
#define QSERVICEFILTER_H

#include "qserviceframeworkglobal.h"
#include <QStringList>

QT_BEGIN_NAMESPACE
class QDataStream;

#ifdef QT_SFW_SERVICEDATABASE_GENERATE
#undef Q_SERVICEFW_EXPORT
#define Q_SERVICEFW_EXPORT
#endif

class QServiceFilterPrivate;
class Q_SERVICEFW_EXPORT QServiceFilter
{
public:
    enum VersionMatchRule {
        ExactVersionMatch = 0,
        MinimumVersionMatch
    };

    enum CapabilityMatchRule {
        MatchMinimum = 0,
        MatchLoadable
    };

    QServiceFilter();
    ~QServiceFilter();
    QServiceFilter(const QServiceFilter& other);
    explicit QServiceFilter(const QString& interfaceName,
                   const QString& version = QString(),
                   QServiceFilter::VersionMatchRule rule = QServiceFilter::MinimumVersionMatch);

    QServiceFilter& operator=(const QServiceFilter& other);

    void setInterface(const QString& interfaceName, const QString& version = QString(),
            QServiceFilter::VersionMatchRule rule = QServiceFilter::MinimumVersionMatch);
    void setServiceName(const QString& serviceName);


    QString serviceName() const;
    QString interfaceName() const;
    int majorVersion() const;
    int minorVersion() const;
    VersionMatchRule versionMatchRule() const;

    QStringList customAttributes() const;
    QString customAttribute(const QString& which) const;
    void setCustomAttribute(const QString& key, const QString& value);
    void clearCustomAttribute(const QString &key = QString());

    void setCapabilities(QServiceFilter::CapabilityMatchRule, const QStringList& capabilities = QStringList() );
    QStringList capabilities() const;
    CapabilityMatchRule capabilityMatchRule() const;

private:
    QServiceFilterPrivate *d;
    friend class QServiceManager;
    //friend class ServiceDatabase;
#ifndef QT_NO_DATASTREAM
    friend Q_SERVICEFW_EXPORT QDataStream &operator<<(QDataStream &, const QServiceFilter &);
    friend Q_SERVICEFW_EXPORT QDataStream &operator>>(QDataStream &, QServiceFilter &);
#endif
};

#ifndef QT_NO_DATASTREAM
Q_SERVICEFW_EXPORT QDataStream &operator<<(QDataStream &, const QServiceFilter &);
Q_SERVICEFW_EXPORT QDataStream &operator>>(QDataStream &, QServiceFilter &);
#endif

QT_END_NAMESPACE

#endif //QSERVICEFILTER_H
