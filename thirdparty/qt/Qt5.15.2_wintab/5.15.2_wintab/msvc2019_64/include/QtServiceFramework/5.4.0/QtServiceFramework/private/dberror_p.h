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

#ifndef DBERROR_H
#define DBERROR_H

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
#include <QString>

QT_BEGIN_NAMESPACE

class Q_AUTOTEST_EXPORT DBError
{
    public:
        enum ErrorCode {
            NoError,
            DatabaseNotOpen = -2000,    //A connection with the database has not been opened
                                        //  database needs to be opened before any operations take place
            InvalidDatabaseConnection,  //The database connection does not have a valid driver
            LocationAlreadyRegistered,  //A service location has already been registered.
            IfaceImplAlreadyRegistered, //An interface implementation by a given service is already registered to that service
            NotFound,
            SqlError,               //An Sql error occurred.
            IfaceIDNotExternal,     //InterfaceID does not refer to an external interface implementation
            CannotCreateDbDir,      //Directory to contain database could not be created(usu a permissions issue)
            CannotOpenServiceDb,     //service database cannot be opened(usually a permissions issue)
            ExternalIfaceIDFound,   //Notification for interfaceDefault() on a user scope database
                                    //  to indicate that a default refers to an interface implementation in the
                                    //  system scope database
            InvalidDescriptorScope, //Notification for setInterfaceDefault() on a system scope database
                                    //  to indicate that a user scope descriptor cannot be used
                                    //  with a system scope database.
            InvalidDatabaseFile,    //database file is corrupted or not a valid database
            NoWritePermissions,     //trying to perform a write operation without sufficient permissions
            UnknownError
        };
        DBError();
        void setError(ErrorCode error, const QString &errorText = QString());
        void setSQLError(const QString &errorText) {
            m_error = SqlError;
            m_text = errorText;
        }
        void setNotFoundError(const QString &errorText) {
            m_error = NotFound;
            m_text = errorText;
        }
        QString text() const { return m_text; }
        ErrorCode code() const { return m_error; }
    private:
        QString m_text;
        ErrorCode m_error;
};
QT_END_NAMESPACE

#endif  //DBERROR_H
