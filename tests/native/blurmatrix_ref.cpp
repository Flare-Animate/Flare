// Differential reference harness for the `CBlurMatrix` port (flarelib/sandor_fxs).
//
// Compiled against the *original* C++ with MSVC, this prints one block per case:
//
//   E d=<16 hex digits of the IEEE-754 bits> nb=<n> ns=<sections> np=<points>
//   S <rs> <npts> <x> <y> <16 hex digits> ...
//   <blank line>
//
// `E` is createEqual, `P` is createEqual followed by addPath, `R` is createRandom with
// is_sac (random sampling *and* the path extension). Doubles go out as raw bit patterns
// rather than %a, because Rust's `{:x}` and C's `%a` disagree on zero and subnormals.
//
// gen_blurmatrix_cases.py runs this and writes the result to
// flare/sources/flarelib/sandor_fxs/BlurMatrix_cases.txt, which the Rust test
// blurmatrix::tests::matches_the_cpp_reference_bit_for_bit checks the port against.
//
// The random cases call srand() with a fixed seed: createRandom draws from the C runtime's
// rand(), and the Rust test reproduces that same MSVC generator, so both see one sequence.
#include <cstdio>
#include <cstdint>
#include <cstring>
#include "BlurMatrix.h"

static void dump(char kind, const CBlurMatrix& m, double d, int nb) {
  uint64_t dbits;
  std::memcpy(&dbits, &d, sizeof(dbits));

  int ns = 0, np = 0;
  for (int i = 0; i < NBRS; i++)
    for (size_t s = 0; s < m.m_m[i].size(); s++) {
      ns++;
      np += (int)m.m_m[i][s].size();
    }
  printf("%c d=%016llx nb=%d ns=%d np=%d\n", kind, (unsigned long long)dbits, nb, ns, np);
  for (int i = 0; i < NBRS; i++)
    for (size_t s = 0; s < m.m_m[i].size(); s++) {
      const BLURSECTION& b = m.m_m[i][s];
      printf("S %d %d", i, (int)b.size());
      for (size_t k = 0; k < b.size(); k++) {
        uint64_t bits;
        std::memcpy(&bits, &b[k].d, sizeof(bits));
        printf(" %d %d %016llx", b[k].x, b[k].y, (unsigned long long)bits);
      }
      printf("\n");
    }
  printf("\n");
}

int main() {
  // (d, nb) pairs that cover: below the d < 1.0 cutoff, nb <= 0, the nb <= 0 / d <= 0.01
  // cutoffs in createRandom, a disc too small to hold nb samples, and ordinary sizes.
  const double ds[]  = {0.0, 0.5, 1.0, 2.0, 3.0, 4.5, 7.0};
  const int    nbs[] = {-1, 0, 1, 2, 5, 12};

  for (double d : ds) {
    for (int nb : nbs) {
      {
        CBlurMatrix m(d, nb, false, false);
        dump('E', m, d, nb);
      }
      {
        CBlurMatrix m(d, nb, true, false);
        dump('P', m, d, nb);
      }
      {
        std::srand(12345);
        CBlurMatrix m(d, nb, true, true);
        dump('R', m, d, nb);
      }
    }
  }
  return 0;
}