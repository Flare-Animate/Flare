

"flare/txshlevelhandle.h"

"flare/txshlevel.h"
"flare/txshsimplelevel.h"
"flare/txsheet.h"
"flare/txshcell.h"

//=============================================================================
// TXshLevelHandle
//-----------------------------------------------------------------------------

TXshLevelHandle::TXshLevelHandle() : m_level(0) {}

//-----------------------------------------------------------------------------

TXshLevelHandle::~TXshLevelHandle() {
  if (m_level) m_level->release();
}

//-----------------------------------------------------------------------------

TXshLevel *TXshLevelHandle::getLevel() const { return m_level; }

//-----------------------------------------------------------------------------

TXshSimpleLevel *TXshLevelHandle::getSimpleLevel() const {
  if (!m_level)
    return 0;
  else
    return m_level->getSimpleLevel();
}

//-----------------------------------------------------------------------------

void TXshLevelHandle::setLevel(TXshLevel *level) {
  if (m_level == level) return;

  TXshLevel *oldLevel = m_level;
  m_level             = level;
  if (level) level->addRef();
  bool levelExists = oldLevel ? oldLevel->getRefCount() > 1 : false;
  if (oldLevel) oldLevel->release();
  emit xshLevelSwitched(levelExists ? oldLevel : 0);
}

//-----------------------------------------------------------------------------

