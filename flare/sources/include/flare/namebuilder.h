#pragma once

#ifndef NAMEBUILDER_INCLUDED
#define NAMEBUILDER_INCLUDED

#include "tcommon.h"

#include <cstddef>
#include <cstdint>
#include <stdexcept>
#include <string>
#include <vector>

// Implemented in flarelib/namebuilder.rs (flare_formats).
extern "C" {
int flare_namecreator_next(int *state, size_t n, size_t cap, size_t *new_n,
                           unsigned char *name, size_t name_cap,
                           size_t *name_len, int skip_invalid);
int flare_namemodifier_parse(const uint32_t *name, size_t len,
                             size_t *base_len, int *index);
}

//-------------------------------------------------------------------

#undef DVAPI
#undef DVVAR
#ifdef FLARELIB_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

//-------------------------------------------------------------------

class DVAPI NameBuilder {
public:
  virtual ~NameBuilder() {}
  virtual std::wstring getNext() = 0;

  static NameBuilder *getBuilder(std::wstring levelName = L"");

  // NameBuilder::getBuilder() restituisce un NameCreator
  // NameBuilder::getBuilder("pippo") restituisce un NameModifier
};

//-------------------------------------------------------------------

// NameCreator genera la sequenza 'A', 'B', ...
// inherited by FlexibleNameCreator
class DVAPI NameCreator : public NameBuilder {
protected:
  std::vector<int> m_s;

public:
  NameCreator() {}
  std::wstring getNext() override {
    const size_t n = m_s.size();
    m_s.resize(n + 1);
    std::vector<unsigned char> buf(n + 2);
    size_t newN = 0, len = 0;
#ifdef _WIN32
    const int skipInvalid = 1;  // AUX, COM, CON, LPT, NUL, PRN are device names
#else
    const int skipInvalid = 0;
#endif
    int rc = flare_namecreator_next(m_s.data(), n, n + 1, &newN, buf.data(),
                                    buf.size(), &len, skipInvalid);
    if (rc != 0) {
      m_s.resize(n);
      throw std::invalid_argument("NameCreator::getNext");
    }
    m_s.resize(newN);
    return std::wstring(buf.begin(), buf.begin() + len);
  }
};

//-------------------------------------------------------------------

class DVAPI NameModifier final : public NameBuilder {
  std::wstring m_nameBase;
  int m_index;

public:
  NameModifier(std::wstring name) : m_nameBase(name), m_index(0) {
    std::vector<uint32_t> u(name.begin(), name.end());
    size_t baseLen = name.length();
    int index      = 0;
    int rc = flare_namemodifier_parse(u.data(), u.size(), &baseLen, &index);
    if (rc == 2) throw std::out_of_range("stoi");  // as std::stoi did
    if (rc != 0) throw std::invalid_argument("NameModifier");
    m_index    = index;
    m_nameBase = name.substr(0, baseLen);
  }
  std::wstring getNext() override {
    int index = m_index++;
    if (index < 1)
      return m_nameBase;
    else
      return m_nameBase + L"_" + std::to_wstring(index);
  }
};

inline NameBuilder *NameBuilder::getBuilder(std::wstring levelName) {
  if (levelName == L"")
    return new NameCreator();
  else
    return new NameModifier(levelName);
}

//-------------------------------------------------------------------

#endif
