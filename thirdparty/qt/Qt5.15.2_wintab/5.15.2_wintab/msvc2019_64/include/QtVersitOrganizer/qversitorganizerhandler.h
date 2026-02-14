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

#ifndef QVERSITORGANIZERHANDLER_H
#define QVERSITORGANIZERHANDLER_H

#include <QtVersitOrganizer/qversitorganizerimporter.h>
#include <QtVersitOrganizer/qversitorganizerexporter.h>

QT_BEGIN_NAMESPACE_VERSITORGANIZER

// qdoc seems to not find QVersitOrganizerHandler if it is declared first, hence this forward
// declaration
class QVersitOrganizerHandler;

class Q_VERSIT_ORGANIZER_EXPORT QVersitOrganizerHandlerFactory
{
public:
    virtual ~QVersitOrganizerHandlerFactory() {}
    virtual QSet<QString> profiles() const { return QSet<QString>(); }
    virtual QString name() const = 0;
    virtual int index() const { return 0; }
    virtual QVersitOrganizerHandler* createHandler() const = 0;

    inline static const QString ProfileSync() {return QStringLiteral("Sync");};
    inline static const QString ProfileBackup() {return QStringLiteral("Backup");};
};

class Q_VERSIT_ORGANIZER_EXPORT QVersitOrganizerHandler : public QVersitOrganizerImporterPropertyHandler,
                                                public QVersitOrganizerExporterDetailHandler
{
public:
    virtual ~QVersitOrganizerHandler() {}
};

QT_END_NAMESPACE_VERSITORGANIZER

#define QT_VERSIT_ORGANIZER_HANDLER_INTERFACE "org.qt-project.Qt.QVersitOrganizerHandlerFactory"
QT_BEGIN_NAMESPACE
Q_DECLARE_INTERFACE(QtVersitOrganizer::QVersitOrganizerHandlerFactory, QT_VERSIT_ORGANIZER_HANDLER_INTERFACE)
QT_END_NAMESPACE

#endif // QVERSITORGANIZERHANDLER_H
