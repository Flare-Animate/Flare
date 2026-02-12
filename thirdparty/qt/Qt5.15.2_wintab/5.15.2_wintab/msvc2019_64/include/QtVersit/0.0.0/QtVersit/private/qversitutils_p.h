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

#ifndef QVERSITUTILS_P_H
#define QVERSITUTILS_P_H

//
//  W A R N I N G
//  -------------
//
// This file is not part of the Qt API.  It exists purely as an
// implementation detail.  This header file may change from version to
// version without notice, or even be removed.
//
// We mean it.
//

#include <QtCore/qbytearray.h>
#include <QtCore/qbytearraymatcher.h>
#include <QtCore/qlist.h>
#include <QtCore/QMutex>

#include <QtVersit/qversitproperty.h>

QT_FORWARD_DECLARE_CLASS(QString)
QT_FORWARD_DECLARE_CLASS(QTextCodec)
QT_FORWARD_DECLARE_CLASS(QVariant)

QT_BEGIN_NAMESPACE_VERSIT

class QVersitDocument;

class Q_VERSIT_EXPORT VersitUtils
{
public:
    static QByteArray encode(const QByteArray& ba, QTextCodec* codec);
    static QByteArray encode(char ch, QTextCodec* codec);
    static QList<QByteArrayMatcher>* newlineList(QTextCodec* codec);
    static void changeCodec(QTextCodec* codec);
    static QVersitProperty takeProperty(const QVersitDocument& document,
                                        const QString& propertyName,
                                        QList<QVersitProperty>* toBeRemoved);
    static bool isValidUtf8(const QByteArray& bytes);
    static bool convertToJson(const QVariant &data, QString *json);
    static bool convertFromJson(const QString &json, QVariant *data);

private:
    // These are caches for performance:
    // The previous codec that encode(char, QTextCodec) was called with
    static QTextCodec* m_previousCodec;
    // The QByteArray corresponding to each char from 0-255, encoded with m_previousCodec
    static QByteArray m_encodingMap[256];
    // List of different newline delimeters, encoded with m_previousCodec
    static QList<QByteArrayMatcher>* m_newlineList;
    static QBasicMutex m_staticLock;
};

QT_END_NAMESPACE_VERSIT

#endif // QVERSITUTILS_P_H
