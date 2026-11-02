#pragma once

#ifndef CASMFILEINFO_H
#define CASMFILEINFO_H

#include "tfilepath.h"
#include "tconvert.h"

#ifdef WIN32
#include <stdlib.h>
#define TOONZMAXPATHLEN _MAX_PATH
#else
#include <sys/param.h>
#define TOONZMAXPATHLEN MAXPATHLEN
#endif

// The CASM scan lives in casmfileinfo.rs (flare_formats). Returns the frame
// count (-1 on bad arguments); an unreadable file gives 0 and a 0..0 range.
extern "C" int flare_casm_range(const char *filename, int bufsize, int *start,
                                int *end, bool *interlaced);

//------------------------------------------------------------------------------

class CasmFileInfo {
public:
  CasmFileInfo(const TFilePath &fp) : m_fp(fp) {}
  void getFrameRange(int &startFrame, int &endFrame, bool &interlaced) {
    startFrame = endFrame = 0;
    interlaced            = false;
    flare_casm_range(toString(m_fp.getWideString()).c_str(), TOONZMAXPATHLEN,
                     &startFrame, &endFrame, &interlaced);
  }

private:
  TFilePath m_fp;
};

#endif
