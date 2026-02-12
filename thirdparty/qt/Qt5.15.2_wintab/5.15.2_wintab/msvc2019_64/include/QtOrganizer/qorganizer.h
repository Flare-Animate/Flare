/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtOrganizer module of the Qt Toolkit.
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

#ifndef QTORGANIZER_H
#define QTORGANIZER_H

// this file includes all of the public header files
// provided by the Qt Organizer API

#include <QtOrganizer/qorganizerglobal.h>                        // global exports

#include <QtOrganizer/qorganizerabstractrequest.h>                // asynchronous request
#include <QtOrganizer/qorganizeritemdetail.h>                     // organizer item detail
#include <QtOrganizer/qorganizeritemfetchhint.h>                  // fetch hint class
#include <QtOrganizer/qorganizeritemfilter.h>                     // organizer item filter
#include <QtOrganizer/qorganizeritem.h>                           // organizer item
#include <QtOrganizer/qorganizeritemid.h>                         // organizer item identifier
#include <QtOrganizer/qorganizeritemobserver.h>                   // organizer item observer
#include <QtOrganizer/qorganizermanager.h>                        // manager
#include <QtOrganizer/qorganizermanagerengine.h>                  // manager backend
#include <QtOrganizer/qorganizermanagerenginefactory.h>           // manage backend instantiator
#include <QtOrganizer/qorganizerrecurrencerule.h>                 // a single recurrence rule
#include <QtOrganizer/qorganizeritemsortorder.h>                  // organizer item sorting

#include <QtOrganizer/qorganizercollection.h>                     // collection of items
#include <QtOrganizer/qorganizercollectionid.h>                   // collection identifier

#include <QtOrganizer/qorganizercollectionchangeset.h>            // engine-specific collection changeset
#include <QtOrganizer/qorganizeritemchangeset.h>                  // engine-specific item changeset

#include <QtOrganizer/qorganizeritems.h>                          // item derived classes
#include <QtOrganizer/qorganizeritemdetails.h>                    // detail derived classes
#include <QtOrganizer/qorganizeritemrequests.h>                   // request derived classes
#include <QtOrganizer/qorganizeritemfilters.h>                    // detail derived classes

QT_BEGIN_NAMESPACE_ORGANIZER
QT_END_NAMESPACE_ORGANIZER

#endif // QTORGANIZER_H
