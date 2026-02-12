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

#ifndef QVERSITREADER_H
#define QVERSITREADER_H

#include <QtCore/qlist.h>
#include <QtCore/qobject.h>

#include <QtVersit/qversitdocument.h>

QT_FORWARD_DECLARE_CLASS(QByteArray)
QT_FORWARD_DECLARE_CLASS(QIODevice)
QT_FORWARD_DECLARE_CLASS(QTextCodec)

QT_BEGIN_NAMESPACE_VERSIT

class QVersitReaderPrivate;

// reads a QVersitDocument from i/o device
class Q_VERSIT_EXPORT QVersitReader : public QObject
{
    Q_OBJECT
public:
    enum Error {
        NoError = 0,
        UnspecifiedError,
        IOError,
        OutOfMemoryError,
        NotReadyError,
        ParseError
    };

    enum State {
        InactiveState = 0,
        ActiveState,
        CanceledState,
        FinishedState
    };

    QVersitReader();
    QVersitReader(QIODevice* inputDevice);
    QVersitReader(const QByteArray& inputData);
    ~QVersitReader();

    // input:
    void setDevice(QIODevice* inputDevice);
    QIODevice* device() const;
    void setData(const QByteArray& inputData);

    void setDefaultCodec(QTextCodec* codec);
    QTextCodec* defaultCodec() const;

    // output:
    QList<QVersitDocument> results() const;

    State state() const;
    Error error() const;

    // reading:
public Q_SLOTS:
    bool startReading();
    void cancel();
public:
    Q_INVOKABLE bool waitForFinished(int msec = -1);

Q_SIGNALS:
    void stateChanged(QVersitReader::State state);
    void resultsAvailable();

private: // data
    QVersitReaderPrivate* d;
};

QT_END_NAMESPACE_VERSIT

Q_DECLARE_METATYPE(QTVERSIT_PREPEND_NAMESPACE(QVersitReader::State))

#endif // QVERSITREADER_H
