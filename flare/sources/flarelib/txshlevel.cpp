

#include "flare/txshlevel.h"
#include "flare/txshleveltypes.h"
#include "flare/hook.h"
#include <QUuid>

//-----------------------------------------------------------------------------

TXshLevel::TXshLevel(ClassCode code, std::wstring name)
    : TSmartObject(code)
    , m_name(name)
    , m_type(UNKNOWN_XSHLEVEL)
    , m_hookSet(new HookSet())
    , m_scene(0) {
  updateShortName();
}

//-----------------------------------------------------------------------------

TXshLevel::~TXshLevel() { delete m_hookSet; }

//-----------------------------------------------------------------------------

void TXshLevel::setScene(ToonzScene *scene) { m_scene = scene; }

//-----------------------------------------------------------------------------

void TXshLevel::setName(std::wstring name) {
  m_name = name;
  updateShortName();
}

bool TXshLevel::setFlashNativeId(const QString &id) {
  if (!id.isEmpty()) {
    const QUuid uuid(id);
    if (id.size() != 36 || uuid.isNull() ||
        uuid.toString(QUuid::WithoutBraces) != id)
      return false;
  }
  m_flashNativeId = id;
  return true;
}

QString TXshLevel::ensureFlashNativeId() {
  if (m_flashNativeId.isEmpty())
    m_flashNativeId = QUuid::createUuid().toString(QUuid::WithoutBraces);
  return m_flashNativeId;
}

//-----------------------------------------------------------------------------

void TXshLevel::updateShortName() {
  if (m_name.size() < 5)
    m_shortName = m_name;
  else
    m_shortName = m_name.substr(0, 4) + L"~";
}

