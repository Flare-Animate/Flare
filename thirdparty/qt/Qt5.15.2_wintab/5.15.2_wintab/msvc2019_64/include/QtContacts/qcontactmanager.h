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

#ifndef QCONTACTMANAGER_H
#define QCONTACTMANAGER_H

#include <QtCore/qmap.h>
#include <QtCore/qobject.h>
#include <QtCore/qstringlist.h>

#include <QtContacts/qcontact.h>
#include <QtContacts/qcontactcollection.h>
#include <QtContacts/qcontactid.h>
#include <QtContacts/qcontactfetchhint.h>
#include <QtContacts/qcontactrelationship.h>
#include <QtContacts/qcontactsortorder.h>
#include <QtContacts/qcontacttype.h>

QT_BEGIN_NAMESPACE_CONTACTS

class QContactAction;
class QContactFilter;

class QContactManagerData;
class Q_CONTACTS_EXPORT QContactManager : public QObject
{
    Q_OBJECT

public:
#ifdef Q_QDOC // qdoc's parser fails to recognise the default map argument
    explicit QContactManager(const QString& managerName = QString(), const QMap<QString, QString>& parameters = 0, QObject* parent = Q_NULLPTR);
    QContactManager(const QString& managerName, int implementationVersion, const QMap<QString, QString>& parameters = 0, QObject* parent = Q_NULLPTR);
#else
    explicit QContactManager(const QString& managerName = QString(), const QMap<QString, QString>& parameters = (QMap<QString, QString>()), QObject* parent = Q_NULLPTR);
    QContactManager(const QString& managerName, int implementationVersion, const QMap<QString, QString>& parameters = (QMap<QString, QString>()), QObject* parent = Q_NULLPTR);
#endif
    explicit QContactManager(QObject* parent);

    inline static const QString ParameterSignalSources() {return QStringLiteral("SignalSources");};
    inline static const QString ParameterSignalDefinitions() {return QStringLiteral("SignalDefinitions");};
    inline static const QString ParameterValueOnlyOtherProcesses() {return QStringLiteral("OnlyOtherProcesses");};
    inline static const QString ParameterValueOnlyOtherManagers() {return QStringLiteral("OnlyOtherManagers");};

    static QContactManager* fromUri(const QString& uri, QObject* parent = Q_NULLPTR);
    ~QContactManager();                     // dtor

    QString managerName() const;                       // e.g. "memory"
    QMap<QString, QString> managerParameters() const;  // e.g. "filename=private.db"
    QString managerUri() const;                        // managerName + managerParameters
    int managerVersion() const;

    static bool parseUri(const QString &uri, QString *managerName, QMap<QString, QString> *params);
    static QString buildUri(const QString &managerName, const QMap<QString, QString> &params);
    static QString buildUri(const QString &managerName, const QMap<QString, QString> &params, int implementationVersion);

    enum Error {
        NoError = 0,
        DoesNotExistError,
        AlreadyExistsError,
        InvalidDetailError,
        InvalidRelationshipError,
        LockedError,
        DetailAccessError,
        PermissionsError,
        OutOfMemoryError,
        NotSupportedError,
        BadArgumentError,
        UnspecifiedError,
        VersionMismatchError,
        LimitReachedError,
        InvalidContactTypeError,
        TimeoutError,
        MissingPlatformRequirementsError
    };

    enum Operation {
        Add,
        Change,
        Remove
    };

    /* Error reporting */
    QContactManager::Error error() const;
    QMap<int, QContactManager::Error> errorMap() const;

    /* Contacts - Accessors and Mutators */
    QList<QContactId> contactIds(const QList<QContactSortOrder>& sortOrders = QList<QContactSortOrder>()) const;
    QList<QContactId> contactIds(const QContactFilter& filter, const QList<QContactSortOrder>& sortOrders = QList<QContactSortOrder>()) const;

    QList<QContact> contacts(const QList<QContactSortOrder>& sortOrders = QList<QContactSortOrder>(), const QContactFetchHint& fetchHint = QContactFetchHint()) const;
    QList<QContact> contacts(const QContactFilter& filter, const QList<QContactSortOrder>& sortOrders = QList<QContactSortOrder>(), const QContactFetchHint& fetchHint = QContactFetchHint()) const;
    QList<QContact> contacts(const QList<QContactId>& contactIds, const QContactFetchHint& fetchHint = QContactFetchHint(), QMap<int, QContactManager::Error>* errorMap = Q_NULLPTR) const;

    QContact contact(const QContactId& contactId, const QContactFetchHint& fetchHint = QContactFetchHint()) const;  // retrieve a contact

    bool saveContact(QContact* contact);                 // note: MODIFIES contact (sets the contactId)
    bool removeContact(const QContactId& contactId);      // remove the contact from the persistent store

    bool saveContacts(QList<QContact>* contacts, QMap<int, QContactManager::Error>* errorMap = Q_NULLPTR); // batch API - save.
    bool saveContacts(QList<QContact>* contacts, const QList<QContactDetail::DetailType>& typeMask, QMap<int, QContactManager::Error>* errorMap = Q_NULLPTR); // Partial save
    bool removeContacts(const QList<QContactId>& contactIds, QMap<int, QContactManager::Error>* errorMap = Q_NULLPTR); // batch API - remove.

    /* "Self" contact id (MyCard) */
    bool setSelfContactId(const QContactId& contactId);
    QContactId selfContactId() const;

    /* Relationships */
    QList<QContactRelationship> relationships(const QContactId& participantId, QContactRelationship::Role role = QContactRelationship::Either) const;
    QList<QContactRelationship> relationships(const QString& relationshipType = QString(), const QContactId& participantId = QContactId(), QContactRelationship::Role role = QContactRelationship::Either) const;
    bool saveRelationship(QContactRelationship* relationship);
    bool saveRelationships(QList<QContactRelationship>* relationships, QMap<int, QContactManager::Error>* errorMap = Q_NULLPTR);
    bool removeRelationship(const QContactRelationship& relationship);
    bool removeRelationships(const QList<QContactRelationship>& relationships, QMap<int, QContactManager::Error>* errorMap = Q_NULLPTR);

    /* Capabilities reporting */
    bool isRelationshipTypeSupported(const QString& relationshipType, QContactType::TypeValues  contactType = QContactType::TypeContact) const;
    QList<QVariant::Type> supportedDataTypes() const;
    bool isFilterSupported(const QContactFilter& filter) const;
    QList<QContactType::TypeValues> supportedContactTypes() const;
    QList<QContactDetail::DetailType> supportedContactDetailTypes() const;

    // collections
    QContactCollectionId defaultCollectionId() const;
    QContactCollection collection(const QContactCollectionId& collectionId);
    QList<QContactCollection> collections();
    bool saveCollection(QContactCollection* collection);
    bool removeCollection(const QContactCollectionId& collectionId);

    /* return a list of available backends for which a QContactManager can be constructed. */
    static QStringList availableManagers();

Q_SIGNALS:
    void dataChanged();
    void contactsAdded(const QList<QContactId>& contactIds);
    void contactsChanged(const QList<QContactId>& contactIds, const QList<QContactDetail::DetailType>& typesChanged);
    void contactsRemoved(const QList<QContactId>& contactIds);
    void relationshipsAdded(const QList<QContactId>& affectedContactIds);
    void relationshipsRemoved(const QList<QContactId>& affectedContactIds);
    void selfContactIdChanged(const QContactId& oldId, const QContactId& newId); // need both? or just new?
    void collectionsAdded(const QList<QContactCollectionId> &collectionIds);
    void collectionsChanged(const QList<QContactCollectionId> &collectionIds);
    void collectionsRemoved(const QList<QContactCollectionId> &collectionIds);
    void collectionsModified(const QList<QPair<QContactCollectionId, QContactManager::Operation> > &collectionIds);

protected:
    void connectNotify(const QMetaMethod &signal);
    void disconnectNotify(const QMetaMethod &signal);

private:
    friend class QContactManagerData;
    void createEngine(const QString& managerName, const QMap<QString, QString>& parameters);

    Q_DISABLE_COPY(QContactManager)

    Q_PRIVATE_SLOT(d, void _q_contactsUpdated(const QList<QContactId>& ids, const QList<QContactDetail::DetailType>& typesChanged))
    Q_PRIVATE_SLOT(d, void _q_contactsDeleted(const QList<QContactId>& ids))

    // private data pointer
    QContactManagerData* d;
};

QT_END_NAMESPACE_CONTACTS

#endif // QCONTACTMANAGER_H
