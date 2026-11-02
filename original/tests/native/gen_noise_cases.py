"""Re-derive the recorded goldens of the noise ports from the ORIGINAL C++.

Covers iwa_noise1234, iwa_simplexnoise, igs_hsv_noise_in_camera and
igs_hls_noise_in_camera (plus the already retired igs_color_rgb_hsv/hls and
igs_math_random they link against). All of them are deleted, so the originals are
rebuilt from git (REF) with g++ and `-ffp-contract=off` and run on inputs made
here (an LCG), through tests/native/noise_ref.cpp. Floating point arguments are
passed as bit patterns. The code paths use only + - * / sqrt floor (IEEE exact),
never libm transcendentals, so the recorded results are bit-exact and platform
independent. The in-camera cases depend on the width of `unsigned long`
(igs_math_random wraps at the word size), so they are recorded for both: the default
`--rng-bits 64` (LP64: macOS, Linux) and `--rng-bits 32` (LLP64 Windows, i686 Linux ->
`*_cases_32.txt`). The real igs_math_random.cpp is replaced by a generated stub of the
requested width (same recurrence); the Rust test picks the golden by `size_of::<c_ulong>()`.

The goldens are written to flare/sources/stdfx/<name>_cases.txt.

Usage: python tests/native/gen_noise_cases.py [--rng-bits {32,64}] [--check]
Needs g++. Not part of the test run; the goldens are committed.
"""
import os, struct, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
STDFX = os.path.join(REPO, "flare", "sources", "stdfx")
REF = os.environ.get("PORT_C_REF", "c1ec31c")

FILES = [
    "iwa_noise1234.cpp", "iwa_noise1234.h", "iwa_simplexnoise.cpp", "iwa_simplexnoise.h",
    "igs_hsv_noise_in_camera.cpp", "igs_hsv_noise_in_camera.h",
    "igs_hls_noise_in_camera.cpp", "igs_hls_noise_in_camera.h",
    "igs_color_rgb_hsv.cpp", "igs_color_rgb_hsv.h", "igs_color_rgb_hls.cpp", "igs_color_rgb_hls.h",
    "igs_math_random.cpp", "igs_math_random.h", "igs_ifx_common.h",
]


class Lcg:
    def __init__(self, s): self.s = s
    def __call__(self, n=1 << 31):
        self.s = (self.s * 6364136223846793005 + 1442695040888963407) & (2**64 - 1)
        return (self.s >> 33) % n
    def uni(self): return self(1 << 30) / float(1 << 30)


def b32(x): return "%08x" % struct.unpack("<I", struct.pack("<f", x))[0]
def b64(x): return "%016x" % struct.unpack("<Q", struct.pack("<d", x))[0]


def rand_f(r, scale):
    k = r(12)
    if k == 0: return 0.0
    if k == 1: return float(r(41) - 20)           # whole numbers, incl. negative (FASTFLOOR quirk)
    if k == 2: return (r(81) - 40) * 0.25
    if k == 3: return (r.uni() - 0.5) * 2e6        # large, wraps the 256 table many times
    return (r.uni() - 0.5) * scale


def make_noise_cases():
    r = Lcg(20261021)
    L = []
    for _ in range(250):
        L.append("N1 " + b32(rand_f(r, 40)))
        L.append("N2 " + " ".join(b32(rand_f(r, 40)) for _ in range(2)))
        L.append("N3 " + " ".join(b32(rand_f(r, 40)) for _ in range(3)))
        L.append("N4 " + " ".join(b32(rand_f(r, 40)) for _ in range(4)))
    for _ in range(200):
        def per():
            p = r(300) + 1
            return -p if r(8) == 0 else p
        L.append("P1 %s %d" % (b32(rand_f(r, 40)), per()))
        L.append("P2 %s %s %d %d" % (b32(rand_f(r, 40)), b32(rand_f(r, 40)), per(), per()))
        L.append("P3 %s %s %s %d %d %d" % (b32(rand_f(r, 40)), b32(rand_f(r, 40)), b32(rand_f(r, 40)), per(), per(), per()))
        L.append("P4 %s %s %s %s %d %d %d %d" % tuple([b32(rand_f(r, 40)) for _ in range(4)] + [per() for _ in range(4)]))
    return L


def make_simplex_cases():
    r = Lcg(777)
    L = []
    for _ in range(300):
        L.append("S2 " + " ".join(b64(rand_f(r, 30)) for _ in range(2)))
        L.append("S3 " + " ".join(b64(rand_f(r, 30)) for _ in range(3)))
        L.append("S4 " + " ".join(b64(rand_f(r, 30)) for _ in range(4)))
        L.append("C " + " ".join(b64(rand_f(r, 30)) for _ in range(3)))
    # exact ties between the coordinates (x0 == y0 etc.) decide the tetrahedron / rank order
    for a in (0.0, 0.25, 0.5, 1.0, -1.0):
        for b in (0.0, 0.25, 0.5, 1.0, -1.0):
            L.append("S2 %s %s" % (b64(a), b64(b)))
            L.append("S3 %s %s %s" % (b64(a), b64(b), b64(a)))
            L.append("C %s %s %s" % (b64(a), b64(b), b64(b)))
            L.append("S4 %s %s %s %s" % (b64(a), b64(b), b64(a), b64(b)))
    return L


def make_camera_cases(kind):
    r = Lcg(4242 if kind == "H" else 2424)
    L = []
    ranges = [0.0, 0.0, 0.05, 0.1, 0.3, 0.5, 1.0]
    effs = [0.0, 0.0, 0.3, 1.0]
    ctrs = [0.5, 0.2, 0.8, 0.0, 1.0]
    blurs = [0.0, 0.25, 0.5, 1.0]
    for n in range(220):
        bits = 8 if n % 2 == 0 else 16
        ch = (1, 3, 4)[r(3)]
        w, h = r(9) + 1, r(7) + 1
        cx, cy = r(7) - 3, r(7) - 3
        cw, chh = r(w + 3) + 1, r(h + 3) + 1
        rg = [ranges[r(len(ranges))] for _ in range(4)]
        if n % 17 == 0: rg = [0.0, 0.0, 0.0, 0.0]
        if n % 5 == 0 and ch == 1: rg[1 if kind == "L" else 2] = 0.3   # the grayscale plane
        terms = []
        for _ in range(3):
            terms += [b64(effs[r(len(effs))]), b64(ctrs[r(len(ctrs))]), str(r(5))]
        mx = 255 if bits == 8 else 65535
        px = []
        for i in range(w * h * ch):
            k = r(10)
            px.append(0 if k == 0 else mx if k == 1 else r(mx + 1))
        if ch == 4 and n % 3 == 0:                       # exercise alpha 0 / full and grey pixels
            for i in range(0, len(px), 4):
                px[i + 3] = (0, mx, r(mx + 1))[r(3)]
        d = 2 if bits == 8 else 4
        hx = "".join(("%0*x" % (d, v)) for v in px)
        L.append(" ".join([kind, str(bits), str(ch), str(w), str(h), str(cx), str(cy), str(cw), str(chh)]
                          + [b64(x) for x in rg] + [str(r(1 << 20)), b64(blurs[r(len(blurs))])] + terms + [hx]))
    return L


RNG_STUB = """\
#pragma once
#ifndef igs_math_random_h
#define igs_math_random_h
// Generated by gen_noise_cases.py -- igs::math::random at a chosen `unsigned long` width.
#include <cstdint>
using rng_word = std::uint{bits}_t;
static constexpr rng_word RNG_MAX = {rng_max};
namespace igs {{
namespace math {{
class random {{
public:
  random() : seed_(1) {{}}
  void seed(unsigned long s) {{ seed_ = (rng_word)s; }}
  unsigned long seed() const {{ return (unsigned long)seed_; }}
  long next() {{
    seed_ = (rng_word)(seed_ * (rng_word)1103515245u + (rng_word)12345u);
    return (long)(seed_ % ((rng_word)RNG_MAX + (rng_word)1));
  }}
  double next_d() {{ return (double)next() / (double)RNG_MAX; }}
private:
  rng_word seed_;
}};
}}  // namespace math
}}  // namespace igs
#endif /* !igs_math_random_h */
"""


def build_ref(t, bits):
    for n in FILES:
        d = subprocess.check_output(["git", "-C", REPO, "show", f"{REF}:flare/sources/stdfx/{n}"])
        open(os.path.join(t, n), "wb").write(d)
    exe = os.path.join(t, "ref")
    with open(os.path.join(t, "igs_math_random.h"), "w") as fh:
        fh.write(RNG_STUB.format(bits=bits, rng_max=hex((1 << (bits - 1)) - 1)))
    cpps = [os.path.join(t, n) for n in FILES if n.endswith(".cpp") and n != "igs_math_random.cpp"]
    subprocess.check_call(["g++", "-std=c++17", "-O1", "-ffp-contract=off", f"-I{t}",
                           os.path.join(HERE, "noise_ref.cpp")] + cpps + ["-o", exe])
    return exe


def run(exe, lines):
    out = subprocess.run([exe], input=("\n".join(lines) + "\n").encode(), check=True, stdout=subprocess.PIPE).stdout.decode()
    return out.splitlines()


def main():
    check = "--check" in sys.argv
    bits = 32 if "--rng-bits" in sys.argv and sys.argv[sys.argv.index("--rng-bits") + 1] == "32" else 64
    sfx = "_32" if bits == 32 else ""
    jobs = {
        f"igs_hsv_noise_in_camera_cases{sfx}.txt": make_camera_cases("H"),
        f"igs_hls_noise_in_camera_cases{sfx}.txt": make_camera_cases("L"),
    }
    if bits == 64:  # width independent: recorded once
        jobs["iwa_noise1234_cases.txt"] = make_noise_cases()
        jobs["iwa_simplexnoise_cases.txt"] = make_simplex_cases()
    bad = False
    with tempfile.TemporaryDirectory() as t:
        exe = build_ref(t, bits)
        for name, lines in jobs.items():
            out = run(exe, lines)
            assert len(out) == len(lines), name
            assert not any(l.endswith(" ERR") for l in out), name
            text = "\n".join(out) + "\n"
            path = os.path.join(STDFX, name)
            if check:
                if not os.path.exists(path) or open(path).read() != text:
                    print("golden drifted:", name)
                    bad = True
            else:
                open(path, "w").write(text)
            print(f"{name}: {len(out)} cases")
    if bad:
        sys.exit(1)


if __name__ == "__main__":
    main()
