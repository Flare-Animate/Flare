#pragma once

#ifndef XSHHANDLEMANAGER_INCLUDED
#define XSHHANDLEMANAGER_INCLUDED

#include "flare/tstageobjecttree.h"

//===============================================================

//    Forward declarations

class TXsheet;
class flareScene;

//===============================================================

//*********************************************************************************
//    XshHandleManager  declaration
//*********************************************************************************

class XshHandleManager final : public HandleManager {
  TXsheet *m_xsh;

public:
  XshHandleManager(TXsheet *xsh) : m_xsh(xsh) {}

  TPointD getHandlePos(const TStageObjectId &id, const std::string &handle,
                       int row) const override;
};

#endif

