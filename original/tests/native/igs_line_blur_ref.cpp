// Golden harness for the `igs_line_blur.cpp` port.
//
// Compiled against the *original* igs_line_blur.cpp (recovered from git history by
// `gen_igs_line_blur_cases.py` and #included so the harness reaches its anonymous
// namespace: the thinning class, the geometry helpers, the pipeline stages).
//
// Sections (one record per line, replayed by `igs_line_blur::tests`):
//   case   <params>            then   img <fnv64 of the whole output buffer> | throw <text>
//                              and (original only)  pipe <lines> <points> <bbox hex> <fnv of coordinates>
//   tcase  <w h art xd yd>     then   thin <xs> <ys> <fnv of the thinned plane>   (original only)
//   rad / rot / ratio / acc    libm-dependent values as raw IEEE hex (original only), 4 ULP in Rust
//   vcase  <action>            then   log/err lines (timestamps and addresses normalised)
//                              and    file <name> <len> <fnv>   for the debug_save files
// Built with -ffp-contract=off (no FMA contraction), like the other igs_* goldens.
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

#ifdef CHAIN
#include "igs_line_blur.h"  // the live inline forwarder over the Rust port
#else
#define private public
#include "igs_line_blur.cpp"
#undef private
#endif

static uint64_t fnv_bytes(const unsigned char *p, size_t n, uint64_t h = 0xcbf29ce484222325ULL) {
  for (size_t i = 0; i < n; ++i) {
    h ^= p[i];
    h *= 0x100000001b3ULL;
  }
  return h;
}

static uint64_t bits64(double d) {
  uint64_t u;
  std::memcpy(&u, &d, 8);
  return u;
}

struct Cfg {
  int w, h, art, cc, bits, ref, padin, padout;
  int blur_count;
  double power;
  int subpixel, near_ref, near_len, thick;
  double remain;
  int retry, v_ref, v_len, action;
};

// line art level (0..255) of pixel (x, y); integer arithmetic only
static int art_level(int art, int x, int y, int w, int h) {
  const int cx = w / 2, cy = h / 2;
  bool on = false;
  switch (art) {
  case 0:
    on = (y >= 3 && y <= h - 4 && (x == cx - 1 || x == cx)) ||
         (x >= 3 && x <= w - 4 && (y == cy - 1 || y == cy));
    break;
  case 1: {
    const int t = y * (w - 1) / (h - 1);
    on = y >= 2 && y <= h - 3 && (x == t || x == t + 1);
    break;
  }
  case 2: {
    const int r = (w < h ? w : h) / 3;
    const int d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy);
    const int dd = d2 - r * r;
    on = (dd < 0 ? -dd : dd) <= r && !(x > cx && std::abs(y - cy) < 2);
    break;
  }
  case 3: {
    const int tri = std::abs((x % 16) - 8) - 4;
    const int yy = cy + tri;
    on = x >= 2 && x <= w - 3 && (y == yy || y == yy + 1);
    break;
  }
  case 4:
    on = (x == cx && y >= cy && y <= h - 3) ||
         (y < cy && y >= 3 && (x == cx - (cy - y) || x == cx + (cy - y)));
    break;
  case 5:
    on = (y == h / 3 && x >= 2 && x <= w / 2 - 4) ||
         (y == h / 3 + 1 && x >= w / 2 + 2 && x <= w - 3);
    break;
  case 6:
    on = (y == h / 3 || y == h / 3 + 1 || y == 2 * h / 3 || y == 2 * h / 3 + 1) && x >= 2 &&
         x <= w - 3;
    break;
  case 7:
    on = x >= 4 && x <= w - 5 && y >= cy - 2 && y <= cy + 2;
    break;
  case 8:
    on = x >= 2 && x <= w - 3 && y >= 2 && y <= h - 3;
    break;
  default:
    break;
  }
  if (on) return 255;
  return ((x * 7 + y * 13) % 97 == 0) ? 40 : 0;
}

static unsigned other_val(int bits, int x, int y, int z) {
  if (bits == 8) return (unsigned)((x * 37 + y * 91 + z * 53 + (x / 3) * 11) % 256);
  return (unsigned)((x * 2579 + y * 97 + z * 7919 + 13) % 65536);
}

static std::vector<unsigned char> make_in(const Cfg &c) {
  const int bytes = c.bits / 8;
  std::vector<unsigned char> buf((size_t)c.h * (c.w + c.padin) * c.cc * bytes, 0x5c);
  for (int y = 0; y < c.h; ++y) {
    for (int x = 0; x < c.w; ++x) {
      for (int z = 0; z < c.cc; ++z) {
        unsigned v;
        if (z == c.ref) {
          v = (unsigned)art_level(c.art, x, y, c.w, c.h);
          if (c.bits == 16) v *= 257;
        } else {
          v = other_val(c.bits, x, y, z);
        }
        const size_t i = ((size_t)y * (c.w + c.padin) * c.cc + (size_t)x * c.cc + z) * bytes;
        buf[i] = (unsigned char)(v & 0xff);
        if (bytes == 2) buf[i + 1] = (unsigned char)(v >> 8);
      }
    }
  }
  return buf;
}

static void run_convert(const Cfg &c, const unsigned char *in, unsigned char *out, bool mv, bool pv,
                        bool cv, bool dbg) {
  igs::line_blur::convert(in, out, c.h, c.w, c.w + c.padin, c.w + c.padout, c.cc, c.bits,
                          c.blur_count, c.power, c.subpixel, c.near_ref, c.near_len, c.thick,
                          c.remain, c.retry, c.v_ref, c.v_len, mv, pv, cv, (long)c.ref, dbg,
                          c.action);
}

#ifndef CHAIN
// the stages of convert(), to expose the vector structure
static void pipe_summary(const Cfg &c, const unsigned char *in) {
  thinnest_ui16_image th;
  pixel_point_root pp;
  pixel_line_root lr;
  pixel_select_same_way_root sw;
  th.set_i32_xs(c.w);
  th.set_i32_ys(c.h);
  lr.set_i32_smooth_retry(c.retry);
  sw.set_i32_count_max(c.v_ref);
  sw.set_d_length_max(c.v_len);
  if (th.mem_alloc() != OK) return;
  igs_line_blur_image_get_(false, false, c.ref, th, in, c.h, c.w, c.w + c.padin, c.cc, c.bits);
  th.exec01_fill_noise_pixel();
  th.exec02_scale_add_edge_pixel();
  th.exec03_scale_liner();
  th.exec04_bw();
  if (th.exec05_thin() != OK) return;
  if (pp.alloc_mem_and_list_node(th.get_i32_xs(), th.get_i32_ys(), th.get_ui16p_src_channel()) !=
      OK)
    return;
  if (lr.exec01020304(&pp) != OK) return;
  lr.exec05_set_middle();
  lr.exec06_int2double_body();
  lr.exec07_smooth_body();
  if (lr.exec08_expand_lines(&pp) != OK) return;
  lr.exec09_same_way_expand(&sw);
  lr.exec10_smooth_expand();
  lr.exec11_set_bbox();
  uint64_t h = 0xcbf29ce484222325ULL;
  int32_t n = 0;
  for (pixel_point_node *p = (pixel_point_node *)pp.get_clp_first(); p;
       p = (pixel_point_node *)p->get_clp_next(), ++n) {
    int32_t xy[2] = {p->get_i32_xp(), p->get_i32_yp()};
    uint64_t t[2] = {bits64(p->get_d_xp_tgt()), bits64(p->get_d_yp_tgt())};
    h = fnv_bytes((const unsigned char *)xy, sizeof xy, h);
    h = fnv_bytes((const unsigned char *)t, sizeof t, h);
  }
  std::printf("pipe %d %d %016llx %016llx %016llx %016llx %016llx\n", lr.get_i32_count(), n,
              (unsigned long long)bits64(lr.get_d_bbox_x_min()),
              (unsigned long long)bits64(lr.get_d_bbox_x_max()),
              (unsigned long long)bits64(lr.get_d_bbox_y_min()),
              (unsigned long long)bits64(lr.get_d_bbox_y_max()), (unsigned long long)h);
}
#endif

static void print_case(const Cfg &c) {
  std::printf("case %d %d %d %d %d %d %d %d %d %016llx %d %d %d %d %016llx %d %d %d %d\n", c.w, c.h,
              c.art, c.cc, c.bits, c.ref, c.padin, c.padout, c.blur_count,
              (unsigned long long)bits64(c.power), c.subpixel, c.near_ref, c.near_len, c.thick,
              (unsigned long long)bits64(c.remain), c.retry, c.v_ref, c.v_len, c.action);
}

static std::string capture_fd(int fd, const std::function<void()> &f) {
  std::fflush(stdout);
  std::fflush(stderr);
  char tmpl[] = "/tmp/igslbXXXXXX";
  const int tf = mkstemp(tmpl);
  const int saved = dup(fd);
  dup2(tf, fd);
  f();
  std::fflush(stdout);
  std::fflush(stderr);
  dup2(saved, fd);
  close(saved);
  std::string s;
  lseek(tf, 0, SEEK_SET);
  char buf[4096];
  ssize_t n;
  while ((n = read(tf, buf, sizeof buf)) > 0) s.append(buf, (size_t)n);
  close(tf);
  unlink(tmpl);
  return s;
}

// timestamps and addresses -> fixed text (the Rust test applies the same rules)
static std::string norm_line(std::string l) {
  if (!l.empty() && l[0] == '\a') l = "<BEL>" + l.substr(1);
  const size_t off = (l.compare(0, 5, "<BEL>") == 0) ? 5 : 0;
  if (l.compare(off, 3, "#  ") == 0 && l.size() >= off + 29 && l[off + 16] == ':' &&
      l[off + 27] == ' ' && l[off + 28] == ' ')
    l.replace(off + 3, 24, "<TS>");
  if (l.find("thinnest_ui16_image::mem_free() <") != std::string::npos ||
      l.find("thinnest_ui16_image::mem_alloc() memory <") != std::string::npos) {
    const size_t a = l.find('<', l.find("ui16_image::mem_"));
    const size_t b = l.find('>', a);
    if (a != std::string::npos && b != std::string::npos) l.replace(a, b - a + 1, "<PTR>");
  }
  return l;
}

static void print_log(const char *tag, const std::string &text) {
  size_t pos = 0;
  while (pos <= text.size()) {
    size_t nl = text.find('\n', pos);
    if (nl == std::string::npos) {
      if (pos < text.size()) std::printf("%s %s|\n", tag, norm_line(text.substr(pos)).c_str());
      break;
    }
    std::printf("%s %s\n", tag, norm_line(text.substr(pos, nl - pos)).c_str());
    pos = nl + 1;
  }
}

static std::string slurp(const char *name) {
  std::string s;
  FILE *fp = std::fopen(name, "rb");
  if (!fp) return s;
  char buf[4096];
  size_t n;
  while ((n = std::fread(buf, 1, sizeof buf, fp)) > 0) s.append(buf, n);
  std::fclose(fp);
  return s;
}

int main() {
  // w h art cc bits ref padin padout | blur_count power subpixel near_ref near_len thick remain
  // retry v_ref v_len action
  const Cfg cfgs[] = {
      // curve blur, defaults-ish, every art
      {40, 32, 0, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 1, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {44, 36, 2, 4, 8, 3, 0, 0, 7, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {48, 30, 3, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 36, 4, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {48, 30, 5, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {48, 30, 6, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 30, 7, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      // curve blur variations
      {40, 32, 0, 4, 8, 3, 0, 0, 5, 2.0, 2, 2, 60, 7, 0.85, 10, 2, 80, 0},
      {40, 32, 1, 4, 8, 3, 0, 0, 15, 0.5, 2, 6, 160, 7, 0.85, 40, 6, 160, 0},
      {40, 32, 3, 4, 8, 3, 0, 0, 21, 3.0, 3, 4, 100, 7, 0.85, 100, 4, 160, 0},
      {40, 32, 4, 4, 8, 3, 0, 0, 12, 1.0, 1, 1, 160, 7, 0.85, 0, 0, 160, 0},
      {40, 32, 2, 4, 8, 3, 0, 0, 1, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 2, 4, 8, 3, 0, 0, 2, 1.0, 2, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 5, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 5, 0},
      {40, 32, 5, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 40, 0},
      {40, 32, 6, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 3, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 6, 4, 8, 3, 0, 0, 9, 1.0, 4, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 3, 4, 8, 3, 0, 0, 0, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      // channels / bits / reference / strides
      {40, 32, 3, 4, 16, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 0, 3, 8, 2, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 0, 3, 16, 0, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 1, 1, 8, 0, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 1, 2, 16, 1, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 4, 4, 8, 1, 3, 5, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 4, 4, 16, 3, 6, 2, 9, 1.0, 2, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {40, 32, 2, 3, 8, 0, 4, 4, 5, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      // smudge brush
      {40, 32, 0, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 5, 0.85, 20, 4, 160, 1},
      {40, 32, 1, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.5, 20, 4, 160, 1},
      {44, 36, 2, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 3, 0.85, 20, 4, 160, 1},
      {48, 30, 3, 4, 16, 3, 0, 0, 9, 1.0, 1, 4, 160, 6, 0.9, 20, 4, 160, 1},
      {40, 36, 4, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 8, 0.85, 20, 4, 160, 1},
      {40, 32, 5, 3, 8, 2, 2, 3, 9, 1.0, 1, 4, 160, 4, 0.85, 20, 4, 160, 1},
      {40, 32, 6, 1, 8, 0, 0, 0, 9, 1.0, 1, 4, 160, 1, 0.85, 20, 4, 160, 1},
      {40, 32, 0, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 0, 0.85, 20, 4, 160, 1},
      {40, 32, 7, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 5, 0.0, 20, 4, 160, 1},
      {40, 32, 7, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 5, 1.0, 20, 4, 160, 1},
      // neither brush (action 2): the image stays untouched
      {40, 32, 0, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 2},
      // degenerate sizes
      {1, 1, 0, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
      {3, 2, 8, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 1},
      {9, 7, 8, 4, 8, 3, 0, 0, 5, 1.0, 1, 4, 160, 3, 0.85, 20, 4, 160, 0},
      {16, 5, 7, 4, 16, 3, 0, 0, 5, 1.0, 1, 4, 160, 3, 0.85, 20, 4, 160, 0},
      // the thinning loop limit (a 215 x 215 block needs more than 100 rounds)
      {215, 215, 8, 4, 8, 3, 0, 0, 9, 1.0, 1, 4, 160, 7, 0.85, 20, 4, 160, 0},
  };
  for (const Cfg &c : cfgs) {
    print_case(c);
    const std::vector<unsigned char> in = make_in(c);
    std::vector<unsigned char> out((size_t)c.h * (c.w + c.padout) * c.cc * (c.bits / 8), 0xab);
    try {
      run_convert(c, in.data(), out.data(), false, false, false, false);
      std::printf("img %016llx\n", (unsigned long long)fnv_bytes(out.data(), out.size()));
#ifndef CHAIN
      pipe_summary(c, in.data());
#endif
    } catch (const std::domain_error &e) {
      std::printf("throw %s\n", e.what());
    }
  }

#ifndef CHAIN
  // thinning class on its own, including the (never used by convert) scale-up
  const int tcases[][5] = {
      {16, 12, 0, 1, 1}, {16, 12, 7, 1, 1}, {20, 14, 2, 1, 1}, {20, 14, 3, 1, 1},
      {18, 12, 0, 2, 2}, {18, 12, 7, 2, 2}, {20, 14, 2, 3, 2}, {20, 14, 4, 2, 3},
      {17, 11, 3, 3, 3}, {2, 2, 8, 2, 2},   {5, 1, 8, 1, 1},   {1, 5, 8, 1, 1},
  };
  for (const auto &t : tcases) {
    std::printf("tcase %d %d %d %d %d\n", t[0], t[1], t[2], t[3], t[4]);
    thinnest_ui16_image th;
    th.set_i32_xs(t[0]);
    th.set_i32_ys(t[1]);
    th.set_i32_xd(t[3]);
    th.set_i32_yd(t[4]);
    if (th.mem_alloc() != OK) continue;
    uint16_t *s = th.get_ui16p_src_channel();
    for (int y = 0; y < t[1]; ++y)
      for (int x = 0; x < t[0]; ++x) {
        int a = art_level(t[2], x, y, t[0], t[1]);
        s[y * t[0] + x] = (uint16_t)(a * 257);
      }
    // a few isolated hot pixels for the noise fill
    if (t[0] > 6 && t[1] > 6) s[2 * t[0] + 3] = 65535;
    th.exec01_fill_noise_pixel();
    th.exec02_scale_add_edge_pixel();
    th.exec03_scale_liner();
    th.exec04_bw();
    const int rc = th.exec05_thin();
    const uint16_t *r = th.get_ui16p_src_channel();
    std::printf("thin %d %d %d %016llx\n", rc, th.get_i32_xs(), th.get_i32_ys(),
                (unsigned long long)fnv_bytes((const unsigned char *)r,
                                              (size_t)th.get_i32_xs() * th.get_i32_ys() * 2));
  }

  // libm-dependent values as raw bits
  const double vecs[][2] = {{1, 0},   {0, 1},     {-1, 0},      {0, -1},     {3, 4},
                            {-3, 4},  {-3, -4},   {3, -4},      {1, 1e-9},   {-1e-9, 1},
                            {7.25, 0.5}, {-0.3, -9.1}, {123.0, 77.0}, {0.01, -0.02}};
  calculator_geometry geom;
  for (const auto &v : vecs) {
    std::printf("rad %016llx %016llx %016llx\n", (unsigned long long)bits64(v[0]),
                (unsigned long long)bits64(v[1]),
                (unsigned long long)bits64(geom.get_d_radian(v[0], v[1])));
    double rx, ry;
    geom.get_dd_rotate(v[0] + 2.5, v[1] - 1.5, v[0] * 0.37, &rx, &ry);
    std::printf("rot %016llx %016llx %016llx\n", (unsigned long long)bits64(v[0] * 0.37),
                (unsigned long long)bits64(rx), (unsigned long long)bits64(ry));
  }
  const int rcounts[] = {1, 2, 3, 9, 10, 51, 100};
  const double rpowers[] = {1.0, 0.1, 0.5, 2.0, 3.3, 10.0};
  for (int n : rcounts)
    for (double p : rpowers) {
      brush_curve_blur bb;
      bb.set_i32_count(n);
      bb.set_d_power(p);
      bb.mem_alloc();
      bb.init_ratio_array();
      uint64_t h = 0xcbf29ce484222325ULL;
      std::printf("ratio %d %016llx", n, (unsigned long long)bits64(p));
      for (int i = 0; i < n; ++i) {
        const uint64_t u = bits64(bb._dp_ratio[i]);
        std::printf(" %016llx", (unsigned long long)u);
        h ^= u;
      }
      std::printf("\n");
    }
  {
    pixel_select_curve_blur_root sel;
    pixel_line_node node;
    const int blurs[] = {1, 5, 9, 21, 50};
    const int counts[] = {30, 61, 200, 301};
    for (int bc : blurs)
      for (int cnt : counts) {
        node._i32_point_count = cnt;
        pixel_select_curve_blur_node sn;
        sn.clp_line = &node;
        for (int pos = bc / 2; pos < cnt - bc / 2; pos += 7) {
          sn.i32_near_point_pos = pos;
          std::printf("acc %d %d %d %016llx\n", bc, cnt, pos,
                      (unsigned long long)bits64(sel._get_line_accum_count(&sn, bc)));
        }
      }
  }
#endif

  // verbose output and debug files, normalised
  for (int action = 0; action < 2; ++action) {
    Cfg c = {30, 24, 4, 4, 8, 3, 1, 2, 5, 1.0, 1, 3, 160, 3, 0.85, 3, 4, 160, action};
    std::printf("vcase %d\n", action);
    const std::vector<unsigned char> in = make_in(c);
    std::vector<unsigned char> out((size_t)c.h * (c.w + c.padout) * c.cc, 0xab);
    char dtmpl[] = "/tmp/igslbdXXXXXX";
    char *d = mkdtemp(dtmpl);
    char cwd[4096];
    if (!getcwd(cwd, sizeof cwd) || !d || chdir(d) != 0) return 1;
    std::string err;
    const std::string so = capture_fd(1, [&] {
      err = capture_fd(2, [&] { run_convert(c, in.data(), out.data(), true, true, true, true); });
    });
    print_log("log", so);
    print_log("err", err);
    std::printf("vimg %016llx\n", (unsigned long long)fnv_bytes(out.data(), out.size()));
    const char *files[] = {"tmp08_jaggy_lines.txt",   "tmp09_one_point.txt",
                           "tmp10_another_point.txt", "tmp11_not_include.txt",
                           "tmp12_middle_point.txt",  "tmp13_expand_vector.txt",
                           "tmp14_same_way_vector.txt", "tmp15_expand_lines.txt",
                           "tmp16_one_expand_point.txt", "tmp17_another_expand_point.txt"};
    for (const char *f : files) {
      const std::string s = slurp(f);
      std::printf("file %s %zu %016llx\n", f, s.size(),
                  (unsigned long long)fnv_bytes((const unsigned char *)s.data(), s.size()));
      std::remove(f);
    }
    if (chdir(cwd) != 0) return 1;
    rmdir(d);
  }
  return 0;
}
