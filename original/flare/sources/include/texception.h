#pragma once

#ifndef T_EXCEPTION_INCLUDED
#define T_EXCEPTION_INCLUDED

#include "tcommon.h"

#undef DVAPI
#undef DVVAR
#ifdef TNZCORE_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

// Implemented in texception.rs (flare_formats): converts a UTF-8 std::string to a
// platform-native std::wstring. Returns a heap-allocated, NUL-terminated buffer that
// the caller copies into m_msg and frees with flare_to_wstring_free.
extern "C" {
DVAPI wchar_t *flare_to_wstring(const char *s);
DVAPI void flare_to_wstring_free(wchar_t *p);
}

class DVAPI TException {
  TString m_msg;

public:
  explicit TException(const std::string &msg = "Toonz Exception") {
    wchar_t *p = flare_to_wstring(msg.c_str());
    if (p) {
      m_msg = p;
      flare_to_wstring_free(p);
    }
  }
  explicit TException(const std::wstring &msg) : m_msg(msg) {}
  virtual ~TException() {}
  virtual TString getMessage() const { return m_msg; }
};

// DVAPI ostream& operator<<(ostream &out, const TException &e);

#endif  // T_EXCEPTION_INCLUDED