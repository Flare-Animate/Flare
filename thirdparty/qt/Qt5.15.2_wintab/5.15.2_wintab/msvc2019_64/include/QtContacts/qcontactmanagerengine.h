/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtContacts module of the Qt Toolkit.
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

#ifndef QCONTACTMANAGERENGINE_H
#define QCONTACTMANAGERENGINE_H

#include <QtCore/qdatetime.h>
#include <QtCore/qmap.h>
#include <QtCore/qobject.h>
#include <QtCore/qstringlist.h>
#include <QtCore/qvariant.h>

#include <QtContacts/qcontact.h>
#include <QtContacts/qcontactabstractrequest.h>
#include <QtContacts/qcontactfetchhint.h>
#include <QtContacts/qcontactfilter.h>
#include <QtContacts/qcontactid.h>
#include <QtContacts/qcontactmanager.h>
#include <QtContacts/qcontactrequests.h>
#include <QtContacts/qcontactsortorder.h>
#include <QtContacts/qcontactcollectionfetchrequest.h>
#include <QtContacts/qcontactcollectionsaverequest.h>
#include <QtContacts/qcontactcollectionremoverequest.h>

QT_BEGIN_NAMESPACE_CONTACTS

class Q_CONTACTS_EXPORT QContactManagerEngine : public QObject
{
    Q_OBJECT

public:
    QContactManagerEngine() {}

    /* URI reporting */
    virtual QString managerName() const = 0;                           // e.g. "memory"
    virtual QMap<QString, QString> managerParameters() const;          // e.g. "filename=private.db,cachesize=1024"
    virtual QMap<QString, QString> idInterpretationParameters() const; // e.g. "filename=private.db"
    virtual int managerVersion() const = 0;

    inline QString managerUri() const
    { if (m_uri.isNull()) m_uri = QContactManager::buildUri(managerName(), idInterpretationParameters()); return m_uri; }

    inline QContactId contactId(const QByteArray &localId) const
    { return QContactId(managerUri(), localId); }
    inline QContactCollectionId collectionId(const QByteArray &localId) const
    { return QContactCollectionId(managerUri(), localId); }

    /* Filtering */
    virtual QList<QContactId> contactIds(const QContactFilter &filter, const QList<QContactSortOrder> &sortOrders, QContactManager::Error *error) const;
    virtual QList<QContact> contacts(const QContactFilter &filter, const QList<QContactSortOrder>& sortOrders, const QContactFetchHint &fetchHint, QContactManager::Error *error) const;
    virtual QList<QContact> contacts(const QList<QContactId> &contactIds, const QContactFetchHint& fetchHint, QMap<int, QContactManager::Error> *errorMap, QContactManager::Error *error) const;
    virtual QContact contact(const QContactId &contactId, const QContactFetchHint &fetchHint, QContactManager::Error *error) const;

    virtual bool saveContact(QContact *contact, QContactManager::Error *error);
    virtual bool removeContact(const QContactId &contactId, QContactManager::Error *error);
    virtual bool saveRelationship(QContactRelationship *relationship, QContactManager::Error *error);
    virtual bool removeRelationship(const QContactRelationship &relationship, QContactManager::Error *error);

    virtual bool saveContacts(QList<QContact> *contacts, QMap<int, QContactManager::Error> *errorMap, QContactManager::Error *error);
    virtual bool saveContacts(QList<QContact> *contacts,  const QList<QContactDetail::DetailType> &typeMask, QMap<int, QContactManager::Error> *errorMap, QContactManager::Error *error);
    virtual bool removeContacts(const QList<QContactId> &contactIds, QMap<int, QContactManager::Error> *errorMap, QContactManager::Error *error);

    /* "Self" contact id (MyCard) */
    virtual bool setSelfContactId(const QContactId &contactId, QContactManager::Error *error);
    virtual QContactId selfContactId(QContactManager::Error *error) const;

    /* Relationships between contacts */
    virtual QList<QContactRelationship> relationships(const QString &relationshipType, const QContactId& participantId, QContactRelationship::Role role, QContactManager::Error *error) const;
    virtual bool saveRelationships(QList<QContactRelationship> *relationships, QMap<int, QContactManager::Error>* errorMap, QContactManager::Error *error);
    virtual bool removeRelationships(const QList<QContactRelationship> &relationships, QMap<int, QContactManager::Error> *errorMap, QContactManager::Error *error);

    // collections
    virtual QContactCollectionId defaultCollectionId() const;
    virtual QContactCollection collection(const QContactCollectionId &collectionId, QContactManager::Error *error);
    virtual QList<QContactCollection> collections(QContactManager::Error *error);
    virtual bool saveCollection(QContactCollection *collection, QContactManager::Error *error);
    virtual bool removeCollection(const QContactCollectionId &collectionId, QContactManager::Error *error);

    /* Validation for saving */
    virtual bool validateContact(const QContact &contact, QContactManager::Error *error) const;

    /* Asynchronous Request Support */
    virtual void requestDestroyed(QContactAbstractRequest *req);
    virtual bool startRequest(QContactAbstractRequest *req);
    virtual bool cancelRequest(QContactAbstractRequest *req);
    virtual bool waitForRequestFinished(QContactAbstractRequest *req, int msecs);

    /* Capabilities reporting */
    virtual bool isRelationshipTypeSupported(const QString &relationshipType, QContactType::TypeValues contactType) const;
    virtual bool isFilterSupported(const QContactFilter &filter) const;
    virtual QList<QVariant::Type> supportedDataTypes() const;
    virtual QList<QContactType::TypeValues> supportedContactTypes() const;
    virtual QList<QContactDetail::DetailType> supportedContactDetailTypes() const;

Q_SIGNALS:
    void dataChanged();
    void contactsAdded(const QList<QContactId> &contactIds);
    void contactsChanged(const QList<QContactId> &contactIds, const QList<QContactDetail::DetailType> &typesChanged);
    void contactsRemoved(const QList<QContactId> &contactIds);
    void relationshipsAdded(const QList<QContactId> &affectedContactIds);
    void relationshipsRemoved(const QList<QContactId> &affectedContactIds);
    void selfContactIdChanged(const QContactId &oldId, const QContactId &newId);
    void collectionsAdded(const QList<QContactCollectionId> &collectionIds);
    void collectionsChanged(const QList<QContactCollectionId> &collectionIds);
    void collectionsRemoved(const QList<QContactCollectionId> &collectionIds);
    void collectionsModified(const QList<QPair<QContactCollectionId, QContactManager::Operation> > &collectionIds);

public:
    // Async update functions
    static void updateRequestState(QContactAbstractRequest *req, QContactAbstractRequest::State state);

    static void updateContactIdFetchRequest(QContactIdFetchRequest *req, const QList<QContactId>& result, QContactManager::Error error, QContactAbstractRequest::State);
    static void updateContactFetchRequest(QContactFetchRequest *req, const QList<QContact> &result, QContactManager::Error error, QContactAbstractRequest::State);
    static void updateContactFetchByIdRequest(QContactFetchByIdRequest *req, const QList<QContact>& result, QContactManager::Error error, const QMap<int, QContactManager::Error> &errorMap, QContactAbstractRequest::State);
    static void updateContactRemoveRequest(QContactRemoveRequest *req, QContactManager::Error error, const QMap<int, QContactManager::Error> &errorMap, QContactAbstractRequest::State);
    static void updateContactSaveRequest(QContactSaveRequest *req, const QList<QContact> &result, QContactManager::Error error, const QMap<int, QContactManager::Error> &errorMap, QContactAbstractRequest::State);
    static void updateRelationshipSaveRequest(QContactRelationshipSaveRequest *req, const QList<QContactRelationship> &result, QContactManager::Error error, const QMap<int, QContactManager::Error> &errorMap, QContactAbstractRequest::State);
    static void updateRelationshipRemoveRequest(QContactRelationshipRemoveRequest *req, QContactManager::Error error, const QMap<int, QContactManager::Error> &errorMap, QContactAbstractRequest::State);
    static void updateRelationshipFetchRequest(QContactRelationshipFetchRequest *req, const QList<QContactRelationship> &result, QContactManager::Error error, QContactAbstractRequest::State);

    // collections
    static void updateCollectionFetchRequest(QContactCollectionFetchRequest *request, const QList<QContactCollection> &result, QContactManager::Error error, QContactAbstractRequest::State newState);
    static void updateCollectionRemoveRequest(QContactCollectionRemoveRequest *request, QContactManager::Error error, const QMap<int, QContactManager::Error> &errorMap, QContactAbstractRequest::State newState);
    static void updateCollectionSaveRequest(QContactCollectionSaveRequest *request, const QList<QContactCollection> &result, QContactManager::Error error, const QMap<int, QContactManager::Error> &errorMap, QContactAbstractRequest::State newState);

    // Other protected area update functions
    static void setDetailAccessConstraints(QContactDetail *detail, QContactDetail::AccessConstraints constraints);
    static void setContactRelationships(QContact *contact, const QList<QContactRelationship> &relationships);
    static void setDetailProvenance(QContactDetail *detail, const QString &provenance);

    /* Helper functions */
    static int compareContact(const QContact &a, const QContact &b, const QList<QContactSortOrder> &sortOrders);
    static void addSorted(QList<QContact>* sorted, const QContact &toAdd, const QList<QContactSortOrder> &sortOrders);
    static int compareVariant(const QVariant &first, const QVariant &second, Qt::CaseSensitivity sensitivity);
    static bool testFilter(const QContactFilter& filter, const QContact &contact);
    static QList<QContactId> sortContacts(const QList<QContact> &contacts, const QList<QContactSortOrder> &sortOrders);

    static QContactFilter canonicalizedFilter(const QContactFilter &filter);

private:
    /* QContactChangeSet is a utility class used to emit the appropriate signals */
    friend class QContactChangeSet;

    mutable QString m_uri;
};

QT_END_NAMESPACE_CONTACTS

QT_BEGIN_NAMESPACE
Q_DECLARE_INTERFACE(QtContacts::QContactManagerEngine, "org.qt-project.Qt.QContactManagerEngine")
QT_END_NAMESPACE

#endif // QCONTACTMANAGERENGINE_H
