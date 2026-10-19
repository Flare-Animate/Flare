// Reference driver for gen_psdutils_cases.py: links the ORIGINAL psdutils.cpp
// (rebuilt from git, real zlib) and prints one result line per input line.
//   U <outlen> <hexin|-> -> U <outlen> <hexin|-> <ret> <hexout|->
//   Z <dst_len> <hexsrc|-> -> Z <dst_len> <hexsrc|-> <ret> <hexdst|->
//   P <dst_len> <row> <depth> <hexsrc|-> -> P ... <ret> <hexdst|->
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
#include "orig_psdutils.cpp"

static std::vector<unsigned char> unhex(const std::string &s) {
  std::vector<unsigned char> v;
  if (s == "-") return v;
  for (size_t i = 0; i + 1 < s.size(); i += 2) v.push_back((unsigned char)std::stoi(s.substr(i, 2), 0, 16));
  return v;
}
static std::string hex(const unsigned char *p, size_t n) {
  if (!n) return "-";
  std::string s;
  char b[3];
  for (size_t i = 0; i < n; i++) { snprintf(b, 3, "%02x", p[i]); s += b; }
  return s;
}

int main() {
  char kind;
  while (scanf(" %c", &kind) == 1) {
    char buf[1 << 16];
    if (kind == 'U') {
      long outlen;
      scanf("%ld %s", &outlen, buf);
      std::vector<unsigned char> in = unhex(buf);
      long inlen = (long)in.size();
      in.resize(in.size() + 256, 0);  // zeros after the input: the C++ may read past inlen
      std::vector<unsigned char> out(outlen + 1, 0xA5);
      int r = unpackrow(out.data(), in.data(), outlen, inlen);
      printf("U %ld %s %d %s\n", outlen, buf, r, hex(out.data(), outlen).c_str());
    } else {
      int dst_len, row = 0, depth = 0;
      if (kind == 'P') scanf("%d %d %d %s", &dst_len, &row, &depth, buf);
      else scanf("%d %s", &dst_len, buf);
      std::vector<unsigned char> src = unhex(buf);
      std::vector<unsigned char> dst(dst_len > 0 ? dst_len : 1, 0xCC);
      int r = kind == 'P'
                  ? psdUnzipWithPrediction(src.empty() ? nullptr : src.data(), (int)src.size(), dst.data(), dst_len, row, depth)
                  : psdUnzipWithoutPrediction(src.empty() ? nullptr : src.data(), (int)src.size(), dst.data(), dst_len);
      if (kind == 'P') printf("P %d %d %d %s %d %s\n", dst_len, row, depth, buf, r, hex(dst.data(), dst_len).c_str());
      else printf("Z %d %s %d %s\n", dst_len, buf, r, hex(dst.data(), dst_len).c_str());
    }
  }
}
