// Reference driver for gen_noise_cases.py: runs the ORIGINAL (pre-port) iwa_noise1234,
// iwa_simplexnoise, igs_hsv_noise_in_camera and igs_hls_noise_in_camera, rebuilt from git.
// stdin: one case per line (see gen_noise_cases.py); stdout: the same line completed with the
// result. Arguments are passed as raw bit patterns so no text/libc conversion is involved.
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cstdint>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>
#include <stdexcept>
#include "iwa_noise1234.h"
#include "iwa_simplexnoise.h"
#include "igs_hsv_noise_in_camera.h"
#include "igs_hls_noise_in_camera.h"

static float f32(const std::string &s) {
  uint32_t b = (uint32_t)std::stoul(s, nullptr, 16);
  float f;
  std::memcpy(&f, &b, 4);
  return f;
}
static double f64(const std::string &s) {
  uint64_t b = std::stoull(s, nullptr, 16);
  double d;
  std::memcpy(&d, &b, 8);
  return d;
}
static std::string h32(float f) {
  uint32_t b;
  std::memcpy(&b, &f, 4);
  char t[16];
  std::snprintf(t, sizeof t, "%08x", b);
  return t;
}
static std::string h64(double d) {
  uint64_t b;
  std::memcpy(&b, &d, 8);
  char t[24];
  std::snprintf(t, sizeof t, "%016llx", (unsigned long long)b);
  return t;
}

int main() {
  std::string line;
  while (std::getline(std::cin, line)) {
    std::istringstream is(line);
    std::vector<std::string> t;
    std::string w;
    while (is >> w) t.push_back(w);
    if (t.empty()) continue;
    const std::string &k = t[0];
    std::string out = line;
    if (k == "N1") out += " " + h32(Noise1234::noise(f32(t[1])));
    else if (k == "N2") out += " " + h32(Noise1234::noise(f32(t[1]), f32(t[2])));
    else if (k == "N3") out += " " + h32(Noise1234::noise(f32(t[1]), f32(t[2]), f32(t[3])));
    else if (k == "N4") out += " " + h32(Noise1234::noise(f32(t[1]), f32(t[2]), f32(t[3]), f32(t[4])));
    else if (k == "P1") out += " " + h32(Noise1234::pnoise(f32(t[1]), std::stoi(t[2])));
    else if (k == "P2") out += " " + h32(Noise1234::pnoise(f32(t[1]), f32(t[2]), std::stoi(t[3]), std::stoi(t[4])));
    else if (k == "P3") out += " " + h32(Noise1234::pnoise(f32(t[1]), f32(t[2]), f32(t[3]), std::stoi(t[4]), std::stoi(t[5]), std::stoi(t[6])));
    else if (k == "P4") out += " " + h32(Noise1234::pnoise(f32(t[1]), f32(t[2]), f32(t[3]), f32(t[4]), std::stoi(t[5]), std::stoi(t[6]), std::stoi(t[7]), std::stoi(t[8])));
    else if (k == "S2") out += " " + h64(SimplexNoise::noise(f64(t[1]), f64(t[2])));
    else if (k == "S3") out += " " + h64(SimplexNoise::noise(f64(t[1]), f64(t[2]), f64(t[3])));
    else if (k == "S4") out += " " + h64(SimplexNoise::noise(f64(t[1]), f64(t[2]), f64(t[3]), f64(t[4])));
    else if (k == "C") {
      CellIds c = SimplexNoise::getCellIds(f64(t[1]), f64(t[2]), f64(t[3]));
      int v[9] = {c.i, c.j, c.k, c.i1, c.j1, c.k1, c.i2, c.j2, c.k2};
      for (int x : v) out += " " + std::to_string(x);
    } else if (k == "H" || k == "L") {
      // H|L bits ch w h cx cy cw chh r0 r1 r2 r3 seed blur e1 c1 t1 e2 c2 t2 e3 c3 t3 <in hex>
      int bits = std::stoi(t[1]), ch = std::stoi(t[2]), ww = std::stoi(t[3]), hh = std::stoi(t[4]);
      int cx = std::stoi(t[5]), cy = std::stoi(t[6]), cw = std::stoi(t[7]), chh = std::stoi(t[8]);
      double r0 = f64(t[9]), r1 = f64(t[10]), r2 = f64(t[11]), r3 = f64(t[12]);
      unsigned long seed = std::stoull(t[13]);
      double blur = f64(t[14]);
      double e1 = f64(t[15]), c1 = f64(t[16]);
      int t1 = std::stoi(t[17]);
      double e2 = f64(t[18]), c2 = f64(t[19]);
      int t2 = std::stoi(t[20]);
      double e3 = f64(t[21]), c3 = f64(t[22]);
      int t3 = std::stoi(t[23]);
      size_t n = (size_t)ww * hh * ch;
      int dig = bits == 8 ? 2 : 4;
      std::vector<unsigned short> v(n);
      std::vector<unsigned char> v8(n);
      for (size_t i = 0; i < n; ++i) {
        unsigned x = (unsigned)std::stoul(t[24].substr(i * dig, dig), nullptr, 16);
        v[i] = (unsigned short)x;
        v8[i] = (unsigned char)x;
      }
      void *p = bits == 8 ? (void *)v8.data() : (void *)v.data();
      try {
        if (k == "H")
          igs::hsv_noise_in_camera::change(p, hh, ww, ch, bits, cx, cy, cw, chh, r0, r1, r2, r3, seed, blur, e1, c1, t1, e2, c2, t2, e3, c3, t3);
        else
          igs::hls_noise_in_camera::change(p, hh, ww, ch, bits, cx, cy, cw, chh, r0, r1, r2, r3, seed, blur, e1, c1, t1, e2, c2, t2, e3, c3, t3);
      } catch (std::exception &) {
        out += " ERR";
        std::cout << out << "\n";
        continue;
      }
      out += " ";
      char b[8];
      for (size_t i = 0; i < n; ++i) {
        std::snprintf(b, sizeof b, bits == 8 ? "%02x" : "%04x", bits == 8 ? (unsigned)v8[i] : (unsigned)v[i]);
        out += b;
      }
    }
    std::cout << out << "\n";
  }
  return 0;
}
