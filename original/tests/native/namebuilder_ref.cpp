// Golden harness for the `namebuilder.cpp` port.
// Original build: the extracted namebuilder.cpp is #included (its header and a stub
// tconvert.h come first on the include path). CHAIN build: the live inline header over the
// Rust library. Output, one record per line (replayed by `namebuilder::tests`):
//   cn <i> <name>                    the i-th NameCreator::getNext() (non-Windows branch)
//   mod <input> <g0> <g1> <g2> <g3>  NameModifier(input).getNext() x4, or  mod <input> throw
#include <cstdio>
#include <iostream>
#include <memory>
#include <string>

#ifdef CHAIN
#include "flare/namebuilder.h"
#else
#include "namebuilder.cpp"
#endif

static std::string narrow(const std::wstring &w) {
  std::string s;
  for (wchar_t c : w) s += (c < 128) ? std::string(1, (char)c) : "?";
  return s;
}

int main() {
  NameCreator c;
  for (int i = 0; i < 1500; ++i) printf("cn %d %s\n", i, narrow(c.getNext()).c_str());
  const wchar_t *inputs[] = {L"pippo",       L"pippo_3",   L"pippo_",     L"pippo3",     L"a_0012",
                             L"_5",          L"_",         L"x_y_7",      L"x__8",       L"123",
                             L"a_2147483647", L"a_2147483648", L"a_99999999999", L"ab_000", L"A_1"};
  for (const wchar_t *in : inputs) {
    try {
      std::unique_ptr<NameBuilder> b(NameBuilder::getBuilder(in));
      printf("mod %s", narrow(in).c_str());
      for (int k = 0; k < 4; ++k) printf(" %s", narrow(b->getNext()).c_str());
      printf("\n");
    } catch (const std::exception &) {
      printf("mod %s throw\n", narrow(in).c_str());
    }
  }
  return 0;
}
