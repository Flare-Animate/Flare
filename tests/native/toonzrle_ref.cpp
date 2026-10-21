// Reference driver for gen_toonzrle_cases.py: links the ORIGINAL cm16/cm24 decode
// kernels of toonzrle.cpp (extracted verbatim from git into orig_kernels.inc) and
// prints one result line per input line.
//   A <tone> <coff> <cbits> <poff> <pbits> <omask> <inlen> <outcap> <hexin|->
//   B <inlen> <outcap> <hexin|->
// Output: the input line, then "=> <dec> <enc> <hexout|->" with the pixels as big-endian
// hex. The original has no extents; it runs on an input padded with 0xFE
// terminators and an output buffer behind a guard page. A run that reads more than
// <inlen> bytes or writes more than <outcap> pixels is recorded as the error result
// "0 0 -" (that is what the Rust port reports for an extent overrun); a run that
// crashes on the guard page prints "SKIP" and is dropped by the generator.
#include <csetjmp>
#include <csignal>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <sys/mman.h>
#include <assert.h>

typedef uint32_t TUINT32;
#define UCHAR unsigned char
#define USHORT unsigned short
#define UINT TUINT32

extern "C" {
#include "orig_kernels.inc"
}

static sigjmp_buf jb;
static void onsegv(int) { siglongjmp(jb, 1); }

static std::vector<unsigned char> unhex(const std::string &s) {
  std::vector<unsigned char> v;
  if (s == "-") return v;
  for (size_t i = 0; i + 1 < s.size(); i += 2) v.push_back((unsigned char)std::stoi(s.substr(i, 2), 0, 16));
  return v;
}
static std::string hexbe(const unsigned char *p, size_t n) {
  if (!n) return "-";
  std::string s;
  char b[3];
  for (size_t i = 0; i < n; i++) { snprintf(b, 3, "%02x", p[i]); s += b; }
  return s;
}

int main() {
  signal(SIGSEGV, onsegv);
  const size_t PAGE = 4096, OUT_ELEMS = 1 << 20;
  size_t outbytes = OUT_ELEMS * 4;
  unsigned char *map = (unsigned char *)mmap(0, outbytes + PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  mprotect(map + outbytes, PAGE, PROT_NONE);
  char kind;
  static char hexbuf[1 << 16];
  while (scanf(" %c", &kind) == 1) {
    int tone = 0, coff = 0, cbits = 0, poff = 0, pbits = 0, omask = 0;
    long inlen, outcap;
    if (kind == 'A') {
      scanf("%d %d %d %d %d %d %ld %ld %s", &tone, &coff, &cbits, &poff, &pbits, &omask, &inlen, &outcap, hexbuf);
    } else {
      scanf("%ld %ld %s", &inlen, &outcap, hexbuf);
    }
    std::vector<unsigned char> in = unhex(hexbuf);
    in.resize(in.size() + 64, 0xFE);
    if (kind == 'A') printf("A %d %d %d %d %d %d %ld %ld %s", tone, coff, cbits, poff, pbits, omask, inlen, outcap, hexbuf);
    else printf("B %ld %ld %s", inlen, outcap, hexbuf);
    int dec = 0, enc = 0;
    volatile int crashed = 0;
    memset(map, 0xA5, outbytes);
    if (sigsetjmp(jb, 1) == 0) {
      if (kind == 'A') {
        USHORT *o = (USHORT *)(map + outbytes - OUT_ELEMS * 2);  // guard page right after the last element
        dec = tif_toonz1_decode_cm16(in.data(), &enc, o, tone, coff, cbits, poff, pbits, (USHORT)omask);
        if (enc > inlen || dec > outcap) { dec = 0; enc = 0; }
        std::string h = hexbe(0, 0);
        std::vector<unsigned char> be;
        for (int i = 0; i < dec; i++) { be.push_back(o[i] >> 8); be.push_back(o[i] & 255); }
        printf(" => %d %d %s\n", dec, enc, hexbe(be.data(), be.size()).c_str());
      } else {
        TUINT32 *o = (TUINT32 *)(map + outbytes - OUT_ELEMS * 4);
        dec = tif_toonz1_decode_cm24(in.data(), &enc, o);
        if (enc > inlen || dec > outcap) { dec = 0; enc = 0; }
        std::vector<unsigned char> be;
        for (int i = 0; i < dec; i++) { be.push_back(o[i] >> 24); be.push_back(o[i] >> 16); be.push_back(o[i] >> 8); be.push_back(o[i]); }
        printf(" => %d %d %s\n", dec, enc, hexbe(be.data(), be.size()).c_str());
      }
    } else {
      printf(" => SKIP\n");
      sigset_t s; sigemptyset(&s); sigaddset(&s, SIGSEGV); sigprocmask(SIG_UNBLOCK, &s, 0);
    }
    fflush(stdout);
  }
}
