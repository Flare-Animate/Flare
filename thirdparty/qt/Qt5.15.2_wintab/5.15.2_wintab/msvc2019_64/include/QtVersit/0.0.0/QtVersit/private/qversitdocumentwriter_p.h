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

#ifndef QVERSITDOCUMENTWRITER_H
#define QVERSITDOCUMENTWRITER_H

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

#include <QtCore/qhash.h>
#include <QtCore/qstringlist.h>

#include <QtVersit/qversitdocument.h>

QT_FORWARD_DECLARE_CLASS(QByteArray)
QT_FORWARD_DECLARE_CLASS(QIODevice)
QT_FORWARD_DECLARE_CLASS(QTextCodec)
QT_FORWARD_DECLARE_CLASS(QTextEncoder)

QT_BEGIN_NAMESPACE_VERSIT

class QVersitProperty;

class Q_VERSIT_EXPORT QVersitDocumentWriter
{
public:
    QVersitDocumentWriter(QVersitDocument::VersitType type);
    virtual ~QVersitDocumentWriter();

    void setCodec(QTextCodec* codec);
    void setAsciiCodec();
    void setDevice(QIODevice* device);

    virtual void encodeVersitProperty(const QVersitProperty& property) = 0;
    virtual void encodeParameters(const QMultiHash<QString,QString>& parameters) = 0;
    bool encodeVersitDocument(const QVersitDocument& document, bool encodeVersion = true);
    void encodeGroupsAndName(const QVersitProperty& property);

    void writeBytes(const QByteArray& value);
    void writeString(const QString& value);
    void writeStringQp(const QString& value);
    void writeCrlf();

protected:
    QVersitDocument::VersitType mType;
    QIODevice* mDevice;
    QTextCodec* mCodec;
    bool mCodecIsAscii;
    bool mCodecIsAsciiCompatible;
    QTextEncoder* mEncoder;
    bool mSuccessful;
    int mCurrentLineLength;
};

QT_END_NAMESPACE_VERSIT

#endif // QVERSITDOCUMENTWRITER_H
