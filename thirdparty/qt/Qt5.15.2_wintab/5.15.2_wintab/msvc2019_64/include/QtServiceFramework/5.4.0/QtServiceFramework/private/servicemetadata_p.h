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

#ifndef SERVICEMETADATA_H
#define SERVICEMETADATA_H

//
//  W A R N I N G
//  -------------
//
// This file is not part of the Qt API. It exists purely as an
// implementation detail. This header file may change from version to
// version without notice, or even be removed.
//
// We mean it.
//

#include "qserviceframeworkglobal.h"
#include <QXmlStreamReader>
#include <QStringList>
#include <QList>
#include <QSet>
#include "qserviceinterfacedescriptor.h"

#if defined(IGNORE_SERVICEMETADATA_EXPORT) || defined(SERVICE_XML_GENERATOR)
#  define SERVICEMETADATA_EXPORT
#else
#  define SERVICEMETADATA_EXPORT Q_AUTOTEST_EXPORT
#endif

QT_BEGIN_NAMESPACE
class QIODevice;

// FORWARD DECLARATIONS
class QServiceInterfaceDescriptor;

class SERVICEMETADATA_EXPORT ServiceMetaDataResults
{
public:
    ServiceMetaDataResults() : type(0) {}

    ServiceMetaDataResults(const ServiceMetaDataResults& other)
    {
        type = other.type;
        location = other.location;
        name = other.name;
        description = other.description;
        interfaces = other.interfaces;
        latestInterfaces = other.latestInterfaces;
    }

    int type;
    QString location;
    QString name;
    QString description;
    QList<QServiceInterfaceDescriptor> interfaces;
    QList<QServiceInterfaceDescriptor> latestInterfaces;
};

#ifndef QT_NO_DATASTREAM
SERVICEMETADATA_EXPORT QDataStream &operator<<(QDataStream &, const ServiceMetaDataResults &);
SERVICEMETADATA_EXPORT QDataStream &operator>>(QDataStream &, ServiceMetaDataResults &);
#endif

class SERVICEMETADATA_EXPORT ServiceMetaData
{
public:

    //! ServiceMetaData::ServiceMetadataErr
    /*!
     This enum describes the errors that may be returned by the Service metadata parser.
     */
    enum ServiceMetadataErr {
        SFW_ERROR_NO_SERVICE = 0,                           /* Can not find service root node in XML file*/
        SFW_ERROR_NO_SERVICE_NAME,                          /* Can not find service name in XML file */
        SFW_ERROR_NO_SERVICE_PATH,                          /* Can not find service filepath in XML file */
        SFW_ERROR_NO_SERVICE_INTERFACE,                     /* No interface for the service in XML file*/
        SFW_ERROR_NO_INTERFACE_VERSION,                     /* Can not find interface version in XML file */
        SFW_ERROR_NO_INTERFACE_NAME,                        /* Can not find interface name in XML file*/
        SFW_ERROR_UNABLE_TO_OPEN_FILE,                      /* Error opening XML file*/
        SFW_ERROR_INVALID_XML_FILE,                         /* Not a valid XML file*/
        SFW_ERROR_PARSE_SERVICE,                            /* Error parsing service node */
        SFW_ERROR_PARSE_INTERFACE,                          /* Error parsing interface node */
        SFW_ERROR_DUPLICATED_INTERFACE,                     /* The same interface is defined twice */
        SFW_ERROR_INVALID_VERSION,
        SFW_ERROR_DUPLICATED_TAG,                           /* The tag appears twice */
        SFW_ERROR_INVALID_CUSTOM_TAG,                       /* The customproperty tag is not corectly formatted or otherwise incorrect*/
        SFW_ERROR_DUPLICATED_CUSTOM_KEY,                    /* The customproperty appears twice*/
        SFW_ERROR_MULTIPLE_SERVICE_TYPES,                   /* Both filepath and ipcaddress found in the XML file */
        SFW_ERROR_INVALID_FILEPATH,                         /* Service path cannot contain IPC prefix */
        SFW_ERROR_INVALID_XML_VERSION,                      /* Error parsing serficefw version node */
        SFW_ERROR_UNSUPPORTED_IPC,                          /* Servicefw version doesn't support IPC */
        SFW_ERROR_UNSUPPORTED_XML_VERSION                   /* Unsupported servicefw version supplied */
    };

public:

    ServiceMetaData(const QString &aXmlFilePath);

    ServiceMetaData(QIODevice *device);

    ~ServiceMetaData();

    void setDevice(QIODevice *device);

    QIODevice *device() const;

    bool extractMetadata();

    int getLatestError() const;

    ServiceMetaDataResults parseResults() const;

private:
    QList<QServiceInterfaceDescriptor> latestInterfaces() const;
    QServiceInterfaceDescriptor latestInterfaceVersion(const QString &interfaceName);
    bool processVersionElement(QXmlStreamReader &aXMLReader);
    bool processServiceElement(QXmlStreamReader &aXMLReader);
    bool processInterfaceElement(QXmlStreamReader &aXMLReader);
    void clearMetadata();

    Q_DISABLE_COPY(ServiceMetaData);

private:
    bool lessThan(const QServiceInterfaceDescriptor &d1,
                    const QServiceInterfaceDescriptor &d2) const;
    bool greaterThan(const QString &v1, const QString &v2) const;
    bool checkVersion(const QString &version) const;
    void transformVersion(const QString &version, int *major, int *minor) const;

    QIODevice *xmlDevice;
    bool ownsXmlDevice;
    QString xmlVersion;
    QString serviceName;
    QString serviceLocation;
    QString serviceDescription;
    QService::Type serviceType;
    QList<QServiceInterfaceDescriptor> serviceInterfaces;
    QSet<QString> duplicates;
    int latestError;
    QHash<QString, int> m_latestIndex;
};

QT_END_NAMESPACE

#endif // SERVICEMETADATA_H
