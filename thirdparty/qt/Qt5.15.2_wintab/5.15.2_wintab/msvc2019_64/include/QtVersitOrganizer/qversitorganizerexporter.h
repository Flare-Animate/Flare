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

#ifndef QVERSITORGANIZEREXPORTER_H
#define QVERSITORGANIZEREXPORTER_H

#include <QtCore/qlist.h>
#include <QtCore/qmap.h>
#include <QtCore/qset.h>

#include <QtOrganizer/qorganizeritem.h>

#include <QtVersit/qversitdocument.h>
#include <QtVersit/qversitproperty.h>

#include <QtVersitOrganizer/qversitorganizerglobal.h>

QT_BEGIN_NAMESPACE_ORGANIZER
class QOrganizerItemDetail;
QT_END_NAMESPACE_ORGANIZER

QTORGANIZER_USE_NAMESPACE
QTVERSIT_USE_NAMESPACE

QT_BEGIN_NAMESPACE_VERSITORGANIZER

class QVersitOrganizerExporterPrivate;

class Q_VERSIT_ORGANIZER_EXPORT QVersitOrganizerExporterDetailHandler
{
public:
    virtual ~QVersitOrganizerExporterDetailHandler() {}
    virtual void detailProcessed(const QOrganizerItem& item,
                                 const QOrganizerItemDetail& detail,
                                 const QVersitDocument& document,
                                 QSet<int>* processedFields,
                                 QList<QVersitProperty>* toBeRemoved,
                                 QList<QVersitProperty>* toBeAdded) = 0;
    virtual void itemProcessed(const QOrganizerItem& item,
                               QVersitDocument* document) = 0;
};

class Q_VERSIT_ORGANIZER_EXPORT QVersitOrganizerExporter
{
public:
    enum Error {
        NoError = 0,
        EmptyOrganizerError,
        UnknownComponentTypeError,
        UnderspecifiedOccurrenceError
    };

    QVersitOrganizerExporter();
    explicit QVersitOrganizerExporter(const QString& profile);
    ~QVersitOrganizerExporter();

    bool exportItems(const QList<QOrganizerItem>& items,
            QVersitDocument::VersitType versitType = QVersitDocument::ICalendar20Type);
    QVersitDocument document() const;
    QMap<int, Error> errorMap() const;

    void setDetailHandler(QVersitOrganizerExporterDetailHandler* handler);

private:
    QVersitOrganizerExporterPrivate* d;
};

QT_END_NAMESPACE_VERSITORGANIZER

#endif // QVERSITORGANIZEREXPORTER_H
