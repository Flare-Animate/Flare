/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtVersit module of the Qt Toolkit.
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

#ifndef QVERSITCONTACTEXPORTER_H
#define QVERSITCONTACTEXPORTER_H

#include <QtCore/qlist.h>
#include <QtCore/qmap.h>

#include <QtContacts/qcontact.h>

#include <QtVersit/qversitresourcehandler.h>
#include <QtVersit/qversitdocument.h>

QT_BEGIN_NAMESPACE_CONTACTS
class QContactDetail;
QT_END_NAMESPACE_CONTACTS

QTCONTACTS_USE_NAMESPACE

QT_BEGIN_NAMESPACE_VERSIT

class QVersitContactExporterPrivate;

class Q_VERSIT_EXPORT QVersitContactExporterDetailHandler
{
public:
    virtual ~QVersitContactExporterDetailHandler() {}
    virtual bool preProcessDetail(const QContact& contact,
                                  const QContactDetail& detail,
                                  QVersitDocument* document) = 0;
    virtual bool postProcessDetail(const QContact& contact,
                                   const QContactDetail& detail,
                                   bool alreadyProcessed,
                                   QVersitDocument* document) = 0;

};

class Q_VERSIT_EXPORT QVersitContactExporterDetailHandlerV2
{
public:
    virtual ~QVersitContactExporterDetailHandlerV2() {}

    virtual void detailProcessed(const QContact& contact,
                                 const QContactDetail& detail,
                                 const QVersitDocument& document,
                                 QSet<int>* processedFields,
                                 QList<QVersitProperty>* toBeRemoved,
                                 QList<QVersitProperty>* toBeAdded) = 0;
    virtual void contactProcessed(const QContact& contact,
                                  QVersitDocument* document) = 0;
};

class Q_VERSIT_EXPORT QVersitContactExporter
{
public:
    enum Error {
        NoError = 0,
        EmptyContactError,
        NoNameError
    };

    QVersitContactExporter();
    explicit QVersitContactExporter(const QString& profile);
    explicit QVersitContactExporter(const QStringList& profile);
    ~QVersitContactExporter();

    bool exportContacts(const QList<QTCONTACTS_PREPEND_NAMESPACE(QContact)>& contacts,
            QVersitDocument::VersitType versitType = QVersitDocument::VCard30Type);
    QList<QVersitDocument> documents() const;
    QMap<int, Error> errorMap() const;

    void setDetailHandler(QVersitContactExporterDetailHandlerV2* handler);

    void setResourceHandler(QVersitResourceHandler* handler);
    QVersitResourceHandler* resourceHandler() const;

    /* deprecated */
    QMap<int, Error> errors() const;
    void setDetailHandler(QVersitContactExporterDetailHandler* handler);
    QVersitContactExporterDetailHandler* detailHandler() const;

private:
    QVersitContactExporterPrivate* d;
};

QT_END_NAMESPACE_VERSIT

#endif // QVERSITCONTACTEXPORTER_H
