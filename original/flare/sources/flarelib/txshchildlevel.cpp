

#include "flare/txshchildlevel.h"
#include "flare/txshleveltypes.h"
#include "flare/txsheet.h"
#include "flare/imagemanager.h"
#include "flare/toonzscene.h"
#include "flare/txshcolumn.h"
#include "tconvert.h"
#include "trasterimage.h"
#include "timagecache.h"
#include "tstream.h"

//-----------------------------------------------------------------------------

DEFINE_CLASS_CODE(TXshChildLevel, 50)

PERSIST_IDENTIFIER(TXshChildLevel, "childLevel")

//=============================================================================
// TXshChildLevel

TXshChildLevel::TXshChildLevel(std::wstring name)
    : TXshLevel(m_classCode, name), m_xsheet(new TXsheet()), m_iconId() {
  m_xsheet->addRef();
  m_type = CHILD_XSHLEVEL;
}

//-----------------------------------------------------------------------------

TXshChildLevel::~TXshChildLevel() {
  m_xsheet->release();
  if (m_iconId != "") {
    ImageManager::instance()->bind(m_iconId, 0);
    TImageCache::instance()->remove(m_iconId);
  }
}

//-----------------------------------------------------------------------------

void TXshChildLevel::loadData(TIStream &is) {
  setFlashNativeId({});
  if (m_xsheet) m_xsheet->release();
  m_xsheet = 0;
  is >> m_xsheet;
  m_xsheet->addRef();
  std::string tagName;
  bool readFlashNativeId = false;
  while (is.matchTag(tagName)) {
    if (tagName == "name") {
      std::wstring name;
      is >> name;
      setName(name);
    } else if (tagName == "flashNativeId") {
      if (readFlashNativeId)
        throw TException("Repeated Flash native level identifier.");
      std::string id;
      is >> id;
      if (!setFlashNativeId(QString::fromStdString(id)))
        throw TException("Invalid Flash native level identifier.");
      readFlashNativeId = true;
    } else {
      // Tolerate unknown tags, as before.
      is.skipCurrentTag();
      continue;
    }
    is.closeChild();
  }
}

//-----------------------------------------------------------------------------

void TXshChildLevel::saveData(TOStream &os) {
  os << m_xsheet;
  os.child("name") << getName();
  if (!flashNativeId().isEmpty())
    os.child("flashNativeId") << flashNativeId().toStdString();
}

//-----------------------------------------------------------------------------

void TXshChildLevel::setXsheet(TXsheet *xsheet) {
  xsheet->addRef();
  m_xsheet->release();
  m_xsheet = xsheet;
}

//-----------------------------------------------------------------------------

void TXshChildLevel::setScene(ToonzScene *scene) {
  TXshLevel::setScene(scene);
  assert(m_xsheet);
  if (!m_xsheet) return;
  m_xsheet->setScene(scene);
  int i, columnCount = m_xsheet->getColumnCount();
  for (i = 0; i < columnCount; i++)
    if (m_xsheet->getColumn(i)) m_xsheet->getColumn(i)->setXsheet(m_xsheet);
}

//-----------------------------------------------------------------------------

int TXshChildLevel::getFrameCount() const {
  if (m_xsheet)
    return m_xsheet->getFrameCount();
  else
    return 0;
}

//-----------------------------------------------------------------------------

void TXshChildLevel::getFids(std::vector<TFrameId> &fids) const {
  int i;
  for (i = 1; i <= getFrameCount(); i++) fids.push_back(TFrameId(i));
}

