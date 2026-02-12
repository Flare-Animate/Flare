/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtVersitOrganizer module of the Qt Toolkit.
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

#ifndef QVERSITORGANIZERIMPORTER_H
#define QVERSITORGANIZERIMPORTER_H

#include <QtCore/qlist.h>
#include <QtCore/qmap.h>

#include <QtOrganizer/qorganizeritem.h>
#include <QtOrganizer/qorganizeritemdetail.h>

#include <QtVersit/qversitglobal.h>

#include <QtVersitOrganizer/qversitorganizerglobal.h>

QT_BEGIN_NAMESPACE_VERSIT
class QVersitDocument;
class QVersitProperty;
QT_END_NAMESPACE_VERSIT

QTORGANIZER_USE_NAMESPACE
QTVERSIT_USE_NAMESPACE

QT_BEGIN_NAMESPACE_VERSITORGANIZER

class QVersitOrganizerImporterPrivate;

class Q_VERSIT_ORGANIZER_EXPORT QVersitOrganizerImporterPropertyHandler
{
public:
    virtual ~QVersitOrganizerImporterPropertyHandler() {}
    virtual void propertyProcessed(const QVersitDocument& document,
                                   const QVersitProperty& property,
                                   const QOrganizerItem& item,
                                   bool *alreadyProcessed,
                                   QList<QOrganizerItemDetail>* updatedDetails) = 0;
    virtual void subDocumentProcessed(const QVersitDocument& topLevel,
                                      const QVersitDocument& subDocument,
                                      QOrganizerItem* item) = 0;
};

class Q_VERSIT_ORGANIZER_EXPORT QVersitOrganizerImporter
{
public:
    enum Error {
        NoError = 0,
        InvalidDocumentError,
        EmptyDocumentError
    };

    QVersitOrganizerImporter();
    explicit QVersitOrganizerImporter(const QString& profile);
    ~QVersitOrganizerImporter();

    bool importDocument(const QVersitDocument& document);
    QList<QOrganizerItem> items() const;
    QMap<int, Error> errorMap() const;

    void setPropertyHandler(QVersitOrganizerImporterPropertyHandler* handler);

private:
    QVersitOrganizerImporterPrivate* d;
};

QT_END_NAMESPACE_VERSITORGANIZER

#endif // QVERSITORGANIZERIMPORTER_H
