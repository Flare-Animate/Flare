/****************************************************************************
**
** Copyright (C) 2016 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtFeedback module of the Qt Toolkit.
**
** $QT_BEGIN_LICENSE:LGPL3$
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
** General Public License version 3 as published by the Free Software
** Foundation and appearing in the file LICENSE.LGPLv3 included in the
** packaging of this file. Please review the following information to
** ensure the GNU Lesser General Public License version 3 requirements
** will be met: https://www.gnu.org/licenses/lgpl.html.
**
** GNU General Public License Usage
** Alternatively, this file may be used under the terms of the GNU
** General Public License version 2.0 or later as published by the Free
** Software Foundation and appearing in the file LICENSE.GPL included in
** the packaging of this file. Please review the following information to
** ensure the GNU General Public License version 2.0 requirements will be
** met: http://www.gnu.org/licenses/gpl-2.0.html.
**
** $QT_END_LICENSE$
**
****************************************************************************/

//
//  W A R N I N G
//  -------------
//
// This file is not part of the Qt API.  It exists for the convenience
// of Qt Feedback framework.  This header file may change from version
// to version without notice, or even be removed.
//
// We mean it.
//
//

#ifndef QFEEDBACKPLUGIN_P_H
#define QFEEDBACKPLUGIN_P_H

#include "qfeedbackactuator.h"
#include "qfeedbackeffect.h"
#include "qfeedbackplugininterfaces.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QVariant>

QT_BEGIN_NAMESPACE

class QDummyBackend : QObject, public QFeedbackHapticsInterface
{
public:
    QDummyBackend() : QObject(qApp) { pluginPriority(); }

    QList<QFeedbackActuator*> actuators() { return QList<QFeedbackActuator*>(); }

    void setActuatorProperty(const QFeedbackActuator &, ActuatorProperty, const QVariant &) { }
    QVariant actuatorProperty(const QFeedbackActuator &, ActuatorProperty) { return QVariant(); }
    bool isActuatorCapabilitySupported(const QFeedbackActuator &, QFeedbackActuator::Capability) { return false; }

    void updateEffectProperty(const QFeedbackHapticsEffect *, EffectProperty) { }
    void setEffectState(const QFeedbackHapticsEffect *, QFeedbackEffect::State) { }
    QFeedbackEffect::State effectState(const QFeedbackHapticsEffect *) { return QFeedbackEffect::Stopped; }

    virtual PluginPriority pluginPriority() { return PluginLowPriority; }
};

QT_END_NAMESPACE

#endif // QFEEDBACKPLUGIN_P_H
