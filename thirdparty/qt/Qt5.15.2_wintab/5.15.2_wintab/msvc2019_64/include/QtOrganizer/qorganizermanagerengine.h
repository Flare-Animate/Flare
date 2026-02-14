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

#ifndef QORGANIZERMANAGERENGINE_H
#define QORGANIZERMANAGERENGINE_H

#include <QtOrganizer/qorganizermanager.h>
#include <QtOrganizer/qorganizerabstractrequest.h>
#include <QtOrganizer/qorganizerrecurrencerule.h>

QT_BEGIN_NAMESPACE_ORGANIZER

class QOrganizerCollectionFetchRequest;
class QOrganizerCollectionRemoveRequest;
class QOrganizerCollectionSaveRequest;
class QOrganizerItemIdFetchRequest;
class QOrganizerItemFetchByIdRequest;
class QOrganizerItemFetchRequest;
class QOrganizerItemOccurrenceFetchRequest;
class QOrganizerItemRemoveRequest;
class QOrganizerItemRemoveByIdRequest;
class QOrganizerItemSaveRequest;
class QOrganizerItemFetchForExportRequest;

class Q_ORGANIZER_EXPORT QOrganizerManagerEngine : public QObject
{
    Q_OBJECT

public:
    QOrganizerManagerEngine(QObject *parent = Q_NULLPTR);

    virtual QString managerName() const;
    virtual QMap<QString, QString> managerParameters() const;
    virtual QMap<QString, QString> idInterpretationParameters() const;

    inline QString managerUri() const
    { if (m_uri.isNull()) m_uri = QOrganizerManager::buildUri(managerName(), idInterpretationParameters()); return m_uri; }

    inline QOrganizerItemId itemId(const QByteArray &localId) const
    { return QOrganizerItemId(managerUri(), localId); }
    inline QOrganizerCollectionId collectionId(const QByteArray &localId) const
    { return QOrganizerCollectionId(managerUri(), localId); }

    // items
    virtual QList<QOrganizerItem> items(const QList<QOrganizerItemId> &itemIds, const QOrganizerItemFetchHint &fetchHint,
                                        QMap<int, QOrganizerManager::Error> *errorMap, QOrganizerManager::Error *error);

    virtual QList<QOrganizerItem> items(const QOrganizerItemFilter &filter, const QDateTime &startDateTime,
                                        const QDateTime &endDateTime, int maxCount,
                                        const QList<QOrganizerItemSortOrder> &sortOrders,
                                        const QOrganizerItemFetchHint &fetchHint, QOrganizerManager::Error *error);

    virtual QList<QOrganizerItemId> itemIds(const QOrganizerItemFilter &filter, const QDateTime &startDateTime,
                                            const QDateTime &endDateTime, const QList<QOrganizerItemSortOrder> &sortOrders,
                                            QOrganizerManager::Error *error);

    virtual QList<QOrganizerItem> itemOccurrences(const QOrganizerItem &parentItem, const QDateTime &startDateTime,
                                                  const QDateTime &endDateTime, int maxCount,
                                                  const QOrganizerItemFetchHint &fetchHint, QOrganizerManager::Error *error);

    virtual QList<QOrganizerItem> itemsForExport(const QDateTime &startDateTime, const QDateTime &endDateTime,
                                                 const QOrganizerItemFilter &filter,
                                                 const QList<QOrganizerItemSortOrder> &sortOrders,
                                                 const QOrganizerItemFetchHint &fetchHint, QOrganizerManager::Error *error);

    virtual bool saveItems(QList<QOrganizerItem> *items, const QList<QOrganizerItemDetail::DetailType> &detailMask,
                           QMap<int, QOrganizerManager::Error> *errorMap, QOrganizerManager::Error *error);

    virtual bool removeItems(const QList<QOrganizerItemId> &itemIds, QMap<int, QOrganizerManager::Error> *errorMap,
                             QOrganizerManager::Error *error);

    virtual bool removeItems(const QList<QOrganizerItem> *items, QMap<int, QOrganizerManager::Error> *errorMap, QOrganizerManager::Error *error);

    // collections
    virtual QOrganizerCollectionId defaultCollectionId() const;
    virtual QOrganizerCollection collection(const QOrganizerCollectionId &collectionId, QOrganizerManager::Error *error);
    virtual QList<QOrganizerCollection> collections(QOrganizerManager::Error *error);
    virtual bool saveCollection(QOrganizerCollection *collection, QOrganizerManager::Error *error);
    virtual bool removeCollection(const QOrganizerCollectionId &collectionId, QOrganizerManager::Error *error);

    // asynchronous request support
    virtual void requestDestroyed(QOrganizerAbstractRequest *request);
    virtual bool startRequest(QOrganizerAbstractRequest *request);
    virtual bool cancelRequest(QOrganizerAbstractRequest *request);
    virtual bool waitForRequestFinished(QOrganizerAbstractRequest *request, int msecs);

    static void updateRequestState(QOrganizerAbstractRequest *request, QOrganizerAbstractRequest::State state);

    static void updateItemOccurrenceFetchRequest(QOrganizerItemOccurrenceFetchRequest *request, const QList<QOrganizerItem> &result,
                                                 QOrganizerManager::Error error, QOrganizerAbstractRequest::State newState);

    static void updateItemIdFetchRequest(QOrganizerItemIdFetchRequest *request, const QList<QOrganizerItemId> &result,
                                         QOrganizerManager::Error error, QOrganizerAbstractRequest::State newState);

    static void updateItemFetchByIdRequest(QOrganizerItemFetchByIdRequest *request, const QList<QOrganizerItem> &result,
                                           QOrganizerManager::Error error, const QMap<int, QOrganizerManager::Error> &errorMap,
                                           QOrganizerAbstractRequest::State);

    static void updateItemFetchRequest(QOrganizerItemFetchRequest *request, const QList<QOrganizerItem> &result,
                                       QOrganizerManager::Error error, QOrganizerAbstractRequest::State newState);

    static void updateItemFetchForExportRequest(QOrganizerItemFetchForExportRequest *request, const QList<QOrganizerItem> &result,
                                                QOrganizerManager::Error error, QOrganizerAbstractRequest::State newState);

    static void updateItemRemoveRequest(QOrganizerItemRemoveRequest *request, QOrganizerManager::Error error,
                                        const QMap<int, QOrganizerManager::Error> &errorMap, QOrganizerAbstractRequest::State newState);

    static void updateItemRemoveByIdRequest(QOrganizerItemRemoveByIdRequest *request, QOrganizerManager::Error error,
                                            const QMap<int, QOrganizerManager::Error> &errorMap, QOrganizerAbstractRequest::State newState);

    static void updateItemSaveRequest(QOrganizerItemSaveRequest *request, const QList<QOrganizerItem> &result, QOrganizerManager::Error error,
                                      const QMap<int, QOrganizerManager::Error> &errorMap, QOrganizerAbstractRequest::State newState);

    static void updateCollectionFetchRequest(QOrganizerCollectionFetchRequest *request, const QList<QOrganizerCollection> &result,
                                             QOrganizerManager::Error error, QOrganizerAbstractRequest::State newState);

    static void updateCollectionRemoveRequest(QOrganizerCollectionRemoveRequest *request, QOrganizerManager::Error error,
                                              const QMap<int, QOrganizerManager::Error> &errorMap, QOrganizerAbstractRequest::State newState);

    static void updateCollectionSaveRequest(QOrganizerCollectionSaveRequest *request, const QList<QOrganizerCollection> &result,
                                            QOrganizerManager::Error error, const QMap<int, QOrganizerManager::Error> &errorMap,
                                            QOrganizerAbstractRequest::State newState);

    // functionality reporting
    virtual QList<QOrganizerItemFilter::FilterType> supportedFilters() const;
    virtual QList<QOrganizerItemDetail::DetailType> supportedItemDetails(QOrganizerItemType::ItemType itemType) const;
    virtual QList<QOrganizerItemType::ItemType> supportedItemTypes() const;

    // helper
    static int addSorted(QList<QOrganizerItem> *sorted, const QOrganizerItem &toAdd, const QList<QOrganizerItemSortOrder> &sortOrders);
    static bool addDefaultSorted(QMultiMap<QDateTime, QOrganizerItem> *defaultSorted, const QOrganizerItem &toAdd);
    static int compareItem(const QOrganizerItem &a, const QOrganizerItem &b, const QList<QOrganizerItemSortOrder> &sortOrders);
    static int compareVariant(const QVariant &a, const QVariant &b, Qt::CaseSensitivity sensitivity);
    static bool isItemBetweenDates(const QOrganizerItem &item, const QDateTime &startPeriod, const QDateTime &endPeriod);
    static bool itemLessThan(const QOrganizerItem &a, const QOrganizerItem &b);
    static bool testFilter(const QOrganizerItemFilter &filter, const QOrganizerItem &item);
    static QOrganizerItemFilter canonicalizedFilter(const QOrganizerItemFilter &filter);

    // recurrence help
    static QOrganizerItem generateOccurrence(const QOrganizerItem &parentItem, const QDateTime &rdate);
    static QList<QDateTime> generateDateTimes(const QDateTime &initialDateTime, QOrganizerRecurrenceRule rrule, const QDateTime &periodStart, const QDateTime &periodEnd, int maxCount);
    static void inferMissingCriteria(QOrganizerRecurrenceRule *rrule, const QDate &initialDate);
    static bool inMultipleOfInterval(const QDate &date, const QDate &initialDate, QOrganizerRecurrenceRule::Frequency frequency, int interval, Qt::DayOfWeek firstDayOfWeek);
    static QDate firstDateInPeriod(const QDate &date, QOrganizerRecurrenceRule::Frequency frequency, Qt::DayOfWeek firstDayOfWeek);
    static QDate firstDateInNextPeriod(const QDate &date, QOrganizerRecurrenceRule::Frequency frequency, Qt::DayOfWeek firstDayOfWeek);
    static QList<QDate> matchingDates(const QDate &periodStart, const QDate &periodEnd, const QOrganizerRecurrenceRule &rrule);
    static QList<QDate> filterByPosition(const QList<QDate> &dates, const QSet<int> positions);
    static bool itemHasReccurence(const QOrganizerItem &oi);

Q_SIGNALS:
    void dataChanged();
    void itemsAdded(const QList<QOrganizerItemId> &itemIds);
    void itemsChanged(const QList<QOrganizerItemId> &itemIds, const QList<QOrganizerItemDetail::DetailType> &typesChanged);
    void itemsRemoved(const QList<QOrganizerItemId> &itemIds);
    void itemsModified(const QList<QPair<QOrganizerItemId, QOrganizerManager::Operation> > &itemIds);
    void collectionsAdded(const QList<QOrganizerCollectionId> &collectionIds);
    void collectionsChanged(const QList<QOrganizerCollectionId> &collectionIds);
    void collectionsRemoved(const QList<QOrganizerCollectionId> &collectionIds);
    void collectionsModified(const QList<QPair<QOrganizerCollectionId, QOrganizerManager::Operation> > &collectionIds);

private:
    Q_DISABLE_COPY(QOrganizerManagerEngine)
    friend class QOrganizerItemChangeSet;
    friend class QOrganizerCollectionChangeSet;

    mutable QString m_uri;
};

QT_END_NAMESPACE_ORGANIZER

#endif // QORGANIZERMANAGERENGINE_H
