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

#ifndef QSERVICEMANAGER_H
#define QSERVICEMANAGER_H

#include "qserviceframeworkglobal.h"

#include "qservice.h"
#include "qserviceinterfacedescriptor.h"
#include "qservicefilter.h"

#include <QObject>
#include <QList>
#include <QStringList>
#include <QDebug>

QT_BEGIN_NAMESPACE

class QServiceContext;
class QServiceFilter;
class QServiceManagerPrivate;
class QServiceReply;
class Q_SERVICEFW_EXPORT QServiceManager : public QObject
{
    Q_OBJECT

public:

   enum Error {
        NoError,
        StorageAccessError,
        InvalidServiceLocation,
        InvalidServiceXml,
        InvalidServiceInterfaceDescriptor,
        ServiceAlreadyExists,
        ImplementationAlreadyExists,
        PluginLoadingFailed,
        ComponentNotFound,
        ServiceCapabilityDenied,
        UnknownError = 100
    };

    explicit QServiceManager(QObject *parent = Q_NULLPTR);
    explicit QServiceManager(QService::Scope scope, QObject *parent = Q_NULLPTR);
    ~QServiceManager();

    QService::Scope scope() const;

    QStringList findServices(const QString& interfaceName = QString()) const;
    QList<QServiceInterfaceDescriptor> findInterfaces(const QServiceFilter& filter = QServiceFilter()) const;
    QList<QServiceInterfaceDescriptor> findInterfaces(const QString& serviceName) const;

    QObject* loadInterface(const QString& interfaceName);
    QObject* loadInterface(const QServiceInterfaceDescriptor& descriptor);

    bool isInterfaceRunning(const QString& interfaceName);
    bool isInterfaceRunning(const QServiceInterfaceDescriptor& descriptor);

    template <class T>
    T* loadLocalTypedInterface(const QString& interfaceName);

    template <class T>
    T* loadLocalTypedInterface(const QServiceInterfaceDescriptor& descriptor);

    QServiceReply *loadInterfaceRequest(const QString& interfaceName);
    QServiceReply *loadInterfaceRequest(const QServiceInterfaceDescriptor& descriptor);

    bool addService(const QString& xmlFilePath);
    bool addService(QIODevice* xmlDevice);
    bool removeService(const QString& serviceName);

    bool setInterfaceDefault(const QString &service, const QString &interfaceName);
    bool setInterfaceDefault(const QServiceInterfaceDescriptor& descriptor);

    QServiceInterfaceDescriptor interfaceDefault(const QString& interfaceName) const;

    QServiceManager::Error error() const;

    bool event(QEvent *);

protected:
    void connectNotify(const QMetaMethod &signal);
    void disconnectNotify(const QMetaMethod &signal);

Q_SIGNALS:
    void serviceAdded(const QString& serviceName, QService::Scope scope);
    void serviceRemoved(const QString& serviceName, QService::Scope scope);
    void errorChanged();

private:
    QObject *loadInterProcessService(const QServiceInterfaceDescriptor &descriptor,
                                          const QString &location) const;
    QObject *loadInProcessService(const QServiceInterfaceDescriptor &descriptor,
                                  const QString &serviceFilePath) const;

    static QString resolveLibraryPath(const QString &libNameOrPath);

    friend class QServiceManagerPrivate;
    friend class QServiceOperationProcessor;
    QServiceManagerPrivate* d;
};

template <class T>
Q_INLINE_TEMPLATE T* QServiceManager::loadLocalTypedInterface(const QString& interfaceName)
{
    return loadLocalTypedInterface<T>(interfaceDefault(interfaceName));
}

template <class T>
Q_OUTOFLINE_TEMPLATE T* QServiceManager::loadLocalTypedInterface(const QServiceInterfaceDescriptor& descriptor)
{
    T* instance = Q_NULLPTR;
    if (descriptor.isValid()) {
        QObject* obj = loadInterface(descriptor);
        if (!obj) return Q_NULLPTR;

        //TODO this should really be
        //instance = qobject_cast<T *>(loadInterface(descriptor, context, session));
        //check why qobject_cast fails
        const char* templateClassName = T::staticMetaObject.className();
        const QMetaObject* source = obj->metaObject();
        do {
            if (strcmp(templateClassName,source->className())==0) {
                instance = static_cast<T *>(obj);
                break;
            }
            source = source->superClass();
        } while (source);
        if (!instance)
            delete obj;
    }
    return instance;
}

QT_END_NAMESPACE

#endif
