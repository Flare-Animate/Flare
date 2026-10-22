// Differential reference harness for the tcolor port (tpixel / tcolorfunctions / tpixelutils
// -> *.rs). Built by gen_tcolor_cases.py against the ORIGINAL sources (golden) and, with
// -DCHAIN, against the NEW inline headers + libflare_formats.a (must print the same lines).
// One case per line: "tag in... | out...". Integers decimal, doubles as 16 hex digits of their
// bits, floats as 8. Only defined inputs (no NaN / out-of-int-range conversions).
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

#include "tcolorfunctions.h"
#include "tpixelgr.h"
#include "tpixelutils.h"

struct Lcg {
	uint64_t s;
	explicit Lcg(uint64_t seed) : s(seed) {}
	uint32_t next()
	{
		s = s * 6364136223846793005ULL + 1442695040888963407ULL;
		return (uint32_t)(s >> 33);
	}
	uint32_t operator()(uint32_t n) { return next() % n; }
	double unit() { return next() / 2147483648.0; } // [0, 1)
};

static uint64_t db(double d) { uint64_t u; std::memcpy(&u, &d, 8); return u; }
static uint32_t fb(float f) { uint32_t u; std::memcpy(&u, &f, 4); return u; }
#define D(x) (unsigned long long)db(x)
#define F(x) (unsigned)fb(x)

int main()
{
	Lcg r(0x7C010001);
	// hsv2rgb into a pixel (7, 8, 9): hue sources up to 3 * max (some leave the pixel alone)
	for (int i = 0; i < 600; i++) {
		int mx = i % 3 == 0 ? 255 : i % 3 == 1 ? 100 : 360;
		int h = (int)r(mx * 3 + mx / 2) - (int)(mx / 4), s = (int)r(mx + mx / 4) - (int)(mx / 8), v = (int)r(mx + mx / 4) - (int)(mx / 8);
		if (i % 10 == 0) s = 0;
		int src[3] = {h, s, v};
		TPixel32 p(7, 8, 9, 10);
		hsv2rgb(p, src, mx);
		std::printf("hsv %d %d %d %d | %d %d %d %d\n", h, s, v, mx, p.r, p.g, p.b, p.m);
	}
	for (int i = 0; i < 600; i++) {
		double h = (r.unit() * 900.0) - 150.0, s = r.unit() * 1.4 - 0.2, v = r.unit() * 1.4 - 0.2;
		if (i % 10 == 0) s = 0;
		if (i % 17 == 0) h = 360.0;
		double a = 123.5, b = 456.5, c = 789.5;
		HSV2RGB(h, s, v, &a, &b, &c);
		std::printf("HSV2RGB %016llx %016llx %016llx | %016llx %016llx %016llx\n", D(h), D(s), D(v), D(a), D(b), D(c));
	}
	for (int i = 0; i < 600; i++) {
		double rr = r.unit(), g = r.unit(), b = r.unit();
		if (i % 5 == 0) g = rr;
		if (i % 7 == 0) b = g;
		if (i % 11 == 0) rr = g = b;
		if (i % 13 == 0) rr = b = 0;
		double h = 0, s = 0, v = 0;
		RGB2HSV(rr, g, b, &h, &s, &v);
		std::printf("RGB2HSV %016llx %016llx %016llx | %016llx %016llx %016llx\n", D(rr), D(g), D(b), D(h), D(s), D(v));
		double l = 0;
		double h2 = 0, s2 = 0;
		rgb2hls(rr, g, b, &h2, &l, &s2);
		std::printf("rgb2hls %016llx %016llx %016llx | %016llx %016llx %016llx\n", D(rr), D(g), D(b), D(h2), D(l), D(s2));
		double hh = r.unit() * 900.0 - 200.0, ll = r.unit(), ss = i % 9 == 0 ? 0.0 : r.unit();
		double o1, o2, o3;
		HLS2RGB(hh, ll, ss, &o1, &o2, &o3);
		std::printf("HLS2RGB %016llx %016llx %016llx | %016llx %016llx %016llx\n", D(hh), D(ll), D(ss), D(o1), D(o2), D(o3));
	}
	for (int i = 0; i < 600; i++) {
		int mx = i % 3 == 0 ? 255 : i % 3 == 1 ? 100 : 1000;
		TPixel32 p(r(256), r(256), r(256), 255);
		if (i % 7 == 0) p.g = p.r;
		if (i % 11 == 0) p.b = p.g = p.r;
		int d[3];
		rgb2hsv(d, p, mx);
		std::printf("rgb2hsv %d %d %d %d | %d %d %d\n", p.r, p.g, p.b, mx, d[0], d[1], d[2]);
	}
	// pixel type conversions
	for (int i = 0; i < 400; i++) {
		TPixel64 p64(r(65536), r(65536), r(65536), r(65536));
		TPixel32 p32(r(256), r(256), r(256), r(256));
		TPixelGR8 g8(r(256));
		double k = i % 4 == 0 ? 1.0 : 1.4;
		TPixelD pd(r.unit() * k - 0.2, r.unit() * k - 0.2, r.unit() * k - 0.2, r.unit() * k - 0.2);
		TPixelF pf((float)(r.unit() * k - 0.2), (float)(r.unit() * k - 0.2), (float)(r.unit() * k - 0.2), (float)(r.unit() * k - 0.2));
		TPixel32 a = toPixel32(p64), b = toPixel32(pd), c = toPixel32(g8), e = toPixel32(pf);
		std::printf("to32 %d %d %d %d | %d %d %d %d\n", p64.r, p64.g, p64.b, p64.m, a.r, a.g, a.b, a.m);
		std::printf("to32d %016llx %016llx %016llx %016llx | %d %d %d %d\n", D(pd.r), D(pd.g), D(pd.b), D(pd.m), b.r, b.g, b.b, b.m);
		std::printf("to32g %d | %d %d %d %d\n", g8.value, c.r, c.g, c.b, c.m);
		std::printf("to32f %08x %08x %08x %08x | %d %d %d %d\n", F(pf.r), F(pf.g), F(pf.b), F(pf.m), e.r, e.g, e.b, e.m);
		TPixel64 a6 = toPixel64(p32), b6 = toPixel64(pd), c6 = toPixel64(g8), e6 = toPixel64(pf);
		std::printf("to64 %d %d %d %d | %d %d %d %d\n", p32.r, p32.g, p32.b, p32.m, a6.r, a6.g, a6.b, a6.m);
		std::printf("to64d %016llx %016llx %016llx %016llx | %d %d %d %d\n", D(pd.r), D(pd.g), D(pd.b), D(pd.m), b6.r, b6.g, b6.b, b6.m);
		std::printf("to64g %d | %d %d %d %d\n", g8.value, c6.r, c6.g, c6.b, c6.m);
		std::printf("to64f %08x %08x %08x %08x | %d %d %d %d\n", F(pf.r), F(pf.g), F(pf.b), F(pf.m), e6.r, e6.g, e6.b, e6.m);
		TPixelD d1 = toPixelD(p32), d2 = toPixelD(p64), d3 = toPixelD(g8), d4 = toPixelD(pf);
		std::printf("toD32 %d %d %d %d | %016llx %016llx %016llx %016llx\n", p32.r, p32.g, p32.b, p32.m, D(d1.r), D(d1.g), D(d1.b), D(d1.m));
		std::printf("toD64 %d %d %d %d | %016llx %016llx %016llx %016llx\n", p64.r, p64.g, p64.b, p64.m, D(d2.r), D(d2.g), D(d2.b), D(d2.m));
		std::printf("toDg %d | %016llx %016llx %016llx %016llx\n", g8.value, D(d3.r), D(d3.g), D(d3.b), D(d3.m));
		std::printf("toDf %08x %08x %08x %08x | %016llx %016llx %016llx %016llx\n", F(pf.r), F(pf.g), F(pf.b), F(pf.m), D(d4.r), D(d4.g), D(d4.b), D(d4.m));
		TPixelF f1 = toPixelF(p32), f2 = toPixelF(pd), f3 = toPixelF(p64), f4 = toPixelF(g8);
		std::printf("toF32 %d %d %d %d | %08x %08x %08x %08x\n", p32.r, p32.g, p32.b, p32.m, F(f1.r), F(f1.g), F(f1.b), F(f1.m));
		std::printf("toFd %016llx %016llx %016llx %016llx | %08x %08x %08x %08x\n", D(pd.r), D(pd.g), D(pd.b), D(pd.m), F(f2.r), F(f2.g), F(f2.b), F(f2.m));
		std::printf("toF64 %d %d %d %d | %08x %08x %08x %08x\n", p64.r, p64.g, p64.b, p64.m, F(f3.r), F(f3.g), F(f3.b), F(f3.m));
		std::printf("toFg %d | %08x %08x %08x %08x\n", g8.value, F(f4.r), F(f4.g), F(f4.b), F(f4.m));
		// grey conversions
		TPixelGR8 q8 = TPixelGR8::from(p32);
		TPixelGR16 q16 = TPixelGR16::from(p64);
		TPixelGRF qf = TPixelGRF::from(pf);
		std::printf("gr %d %d %d %d %d %d | %d %d %08x\n", p32.r, p32.g, p32.b, p64.r, p64.g, p64.b, q8.value, q16.value, F(qf.value));
	}
	// toLinear (libm pow)
	for (int i = 0; i < 300; i++) {
		double gamma = 0.2 + r.unit() * 2.8;
		TPixel32 p(r(256), r(256), r(256), r(256));
		TPixel64 p64(r(65536), r(65536), r(65536), r(65536));
		TPixelD pd(r.unit(), r.unit(), r.unit(), r.unit());
		TPixelF pf((float)r.unit(), (float)r.unit(), (float)(r.unit() - 0.3), (float)r.unit());
		TPixelGR8 g(r(256));
		TPixel32 a = toLinear(p, gamma);
		TPixel64 b = toLinear(p64, gamma);
		TPixelD c = toLinear(pd, gamma);
		TPixelF d = toLinear(pf, gamma);
		TPixelGR8 e = toLinear(g, gamma);
		std::printf("lin32 %016llx %d %d %d %d | %d %d %d %d\n", D(gamma), p.r, p.g, p.b, p.m, a.r, a.g, a.b, a.m);
		std::printf("lin64 %016llx %d %d %d %d | %d %d %d %d\n", D(gamma), p64.r, p64.g, p64.b, p64.m, b.r, b.g, b.b, b.m);
		std::printf("linD %016llx %016llx %016llx %016llx %016llx | %016llx %016llx %016llx %016llx\n", D(gamma), D(pd.r), D(pd.g), D(pd.b), D(pd.m), D(c.r), D(c.g), D(c.b), D(c.m));
		std::printf("linF %016llx %08x %08x %08x %08x | %08x %08x %08x %08x\n", D(gamma), F(pf.r), F(pf.g), F(pf.b), F(pf.m), F(d.r), F(d.g), F(d.b), F(d.m));
		std::printf("lingr %016llx %d | %d\n", D(gamma), g.value, e.value);
	}
	// colour functions
	for (int i = 0; i < 300; i++) {
		TPixel32 c(r(256), r(256), r(256), i % 6 == 0 ? 0 : r(256)), fc(r(256), r(256), r(256), r(256));
		double fade = i % 8 == 0 ? 0.0 : i % 8 == 1 ? 1.0 : r.unit();
		TColorFader fd(fc, fade);
		TOnionFader of(fc, fade);
		TTranspFader tf(fade);
		TColumnColorFilterFunction cf(fc);
		double m[4] = {r.unit() * 2, r.unit() * 2, r.unit() * 2, r.unit() * 2}, k[4] = {r.unit() * 300 - 100, r.unit() * 300 - 100, r.unit() * 300 - 100, r.unit() * 300 - 100};
		TGenericColorFunction gf(m, k);
		TPixel32 o1 = fd(c), o2 = of(c), o3 = tf(c), o4 = cf(c), o5 = gf(c);
		TColorFunction::Parameters p1, p2, p5;
		fd.getParameters(p1);
		of.getParameters(p2);
		gf.getParameters(p5);
		std::printf("fn %d %d %d %d %d %d %d %d %016llx | %d %d %d %d | %d %d %d %d | %d %d %d %d | %d %d %d %d\n", c.r, c.g, c.b, c.m, fc.r, fc.g, fc.b, fc.m, D(fade), o1.r, o1.g, o1.b, o1.m, o2.r, o2.g, o2.b, o2.m, o3.r, o3.g, o3.b, o3.m, o4.r, o4.g, o4.b, o4.m);
		std::printf("fnp %d %d %d %d %016llx | %016llx %016llx %016llx %016llx %016llx %016llx %016llx %016llx | %016llx %016llx %016llx %016llx %016llx %016llx %016llx %016llx\n", fc.r, fc.g, fc.b, fc.m, D(fade),
					D(p1.m_mR), D(p1.m_mG), D(p1.m_mB), D(p1.m_mM), D(p1.m_cR), D(p1.m_cG), D(p1.m_cB), D(p1.m_cM), D(p2.m_mR), D(p2.m_mG), D(p2.m_mB), D(p2.m_mM), D(p2.m_cR), D(p2.m_cG), D(p2.m_cB), D(p2.m_cM));
		std::printf("fng %d %d %d %d %016llx %016llx %016llx %016llx %016llx %016llx %016llx %016llx | %d %d %d %d\n", c.r, c.g, c.b, c.m, D(m[0]), D(m[1]), D(m[2]), D(m[3]), D(k[0]), D(k[1]), D(k[2]), D(k[3]), o5.r, o5.g, o5.b, o5.m);
		(void)p5;
	}
	return 0;
}
