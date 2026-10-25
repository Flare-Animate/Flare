// Golden harness for the `tiio_jpg_exif.cpp` port.
// Original build: the extracted tiio_jpg_exif.cpp is #included after its header. CHAIN build:
// the live inline header over the Rust library. Each record replays what the JPEG reader does:
// `process_EXIF(payload - 2, payload_len)` on a fresh reader.
//   in  <hex payload>
//   out <x bits> <y bits> <unit> <contains> <hex of std::cout text>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>

#define class struct  // JpgExifReader members default to private
#ifdef CHAIN
#include "tiio_jpg_exif.h"
#else
#include "tiio_jpg_exif.h"
#include "tiio_jpg_exif.cpp"
#endif
#undef class

static uint32_t st;
static uint32_t lcg() { return st = st * 1664525u + 1013904223u; }
static uint32_t rnd(uint32_t n) { return (lcg() >> 8) % n; }

static void put16(std::vector<uint8_t> &v, bool mot, unsigned x) {
  if (mot) { v.push_back(x >> 8); v.push_back(x & 255); } else { v.push_back(x & 255); v.push_back(x >> 8); }
}
static void put32(std::vector<uint8_t> &v, bool mot, uint32_t x) {
  if (mot) { for (int i = 3; i >= 0; --i) v.push_back((x >> (8 * i)) & 255); }
  else { for (int i = 0; i < 4; ++i) v.push_back((x >> (8 * i)) & 255); }
}

static std::string hex(const uint8_t *p, size_t n) {
  std::string s;
  char b[3];
  for (size_t i = 0; i < n; ++i) { snprintf(b, 3, "%02x", p[i]); s += b; }
  return s.empty() ? "-" : s;
}

static void run_case(const std::vector<uint8_t> &payload) {
  // two bytes before the payload (the JPEG reader passes marker->data - 2) and padding after
  std::vector<uint8_t> buf(2 + payload.size() + 64, 0);
  memcpy(buf.data() + 2, payload.data(), payload.size());
  std::ostringstream cap;
  std::streambuf *old = std::cout.rdbuf(cap.rdbuf());
  JpgExifReader r;
  r.ImageInfo.ResolutionUnit = 0;
  r.process_EXIF(buf.data(), (unsigned)payload.size());
  std::cout.rdbuf(old);
  float x = r.ImageInfo.xResolution, y = r.ImageInfo.yResolution;
  uint32_t xb, yb;
  memcpy(&xb, &x, 4); memcpy(&yb, &y, 4);
  std::string text = cap.str();
  printf("in %s\n", hex(payload.data(), payload.size()).c_str());
  printf("out %08x %08x %d %d %s\n", xb, yb, r.ImageInfo.ResolutionUnit, r.containsResolution() ? 1 : 0,
         hex((const uint8_t *)text.data(), text.size()).c_str());
}

int main() {
  st = 99;
  for (int n = 0; n < 700; ++n) {
    bool mot = rnd(2);
    int entries = rnd(7);
    std::vector<uint8_t> v = {'E', 'x', 'i', 'f', 0, 0, (uint8_t)(mot ? 'M' : 'I'), (uint8_t)(mot ? 'M' : 'I')};
    put16(v, mot, 0x2a);
    put32(v, mot, 8);
    put16(v, mot, entries);
    size_t ifd_end = 6 + 8 + 2 + 12 * entries + 4;
    size_t tail_len = 16 + rnd(48);
    for (int e = 0; e < entries; ++e) {
      static const unsigned tags[] = {0x011A, 0x011B, 0x0128, 0x011A, 0x011B, 0x0128, 0x0100, 0x8769};
      unsigned tag = tags[rnd(8)];
      unsigned fmt = rnd(10) == 0 ? rnd(16) : 1 + rnd(12);
      unsigned comps = rnd(8) == 0 ? (unsigned[]){0, 2, 3, 5, 0x10001, 0xfffffff0u}[rnd(6)] : 1;
      put16(v, mot, tag);
      put16(v, mot, fmt);
      put32(v, mot, comps);
      unsigned bpf[] = {0, 1, 1, 2, 4, 8, 1, 1, 2, 4, 8, 4, 8};
      unsigned bc = fmt < 13 ? bpf[fmt] * comps : 0;
      if (bc > 4 && rnd(6) != 0) {
        // point into the tail (TIFF-relative offset)
        put32(v, mot, (uint32_t)(ifd_end - 6 + rnd((unsigned)tail_len)));
      } else {
        for (int k = 0; k < 4; ++k) v.push_back(rnd(256));
      }
    }
    put32(v, mot, 0);  // next IFD
    for (size_t k = 0; k < tail_len; ++k) v.push_back(rnd(4) == 0 ? rnd(256) : (rnd(2) ? 0 : 1 + rnd(3)));
    if (n % 9 == 0 && v.size() > 20) v.resize(v.size() - rnd(12));
    run_case(v);
  }
  // fixed negative cases
  run_case({'E', 'x', 'i', 'f', 0, 0});
  run_case({'E', 'x', 'i', 'f', 0, 1, 'I', 'I', 0x2a, 0, 8, 0, 0, 0, 0, 0});
  run_case({'E', 'x', 'i', 'f', 0, 0, 'X', 'X', 0x2a, 0, 8, 0, 0, 0, 0, 0});
  run_case({'E', 'x', 'i', 'f', 0, 0, 'I', 'I', 0x2b, 0, 8, 0, 0, 0, 0, 0});
  run_case({'E', 'x', 'i', 'f', 0, 0, 'I', 'I', 0x2a, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0});
  run_case({'E', 'x', 'i', 'f', 0, 0, 'M', 'M', 0, 0x2a, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0});
  return 0;
}
