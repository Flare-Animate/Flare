// Differential reference harness for the SWF shape / action / sprite / sound port
// (common/flash FDTShapes, FAction, FDTSprite, FDTSounds -> *.rs).
//
// Compiled twice by gen_swfshapes_cases.py:
//   * against the ORIGINAL ten .cpp files (the six SWF stream writers and the four classes
//     ported here) plus their headers, rebuilt from git -> the golden;
//   * (--verify-chain) against the NEW inline headers + libflare_formats.a -> must print the
//     same bytes (this proves the C++ shells marshal like the old members did).
//
// Only the public API is used. One case per line, fields separated by '|', byte strings in hex
// ("-" when empty). Lines whose first field is cx, cxa, straight, curved, change, shapeend,
// stylecount, unparsed, geturl2, gotoframe, gotoframe2, if, jump, waitframe, waitframe2, push,
// condbit, condkey, cliprec, snddef, sndblock, sndhead, sndhead2, sndinfo and spritebody are
// replayed by the Rust unit tests; every other line (whole tags built from the C++ object
// graph: shapes, morphs, sprites, conditions, clip actions, ...) is compared by --verify-chain.
//
// Deliberate omissions (undefined behaviour in the C++): FActionPush type 6 prints its eight
// address-derived operand bytes zeroed (see FAction.rs); FSndEnv is built over pre-filled memory
// because its constructor never sets its members (see FDTSounds.rs; the harness is built with
// -fno-lifetime-dse so the pre-fill survives); dx/dy beyond 16 bits (nBits >= 18) and 8-bit
// stereo ADPCM are not generated.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <new>
#include <string>
#include <vector>

#include "FSWFStream.h"
#include "FObj.h"
#include "FPrimitive.h"
#include "FSound.h"
#include "FCT.h"
#include "FDTBitmaps.h"
#include "FDTShapes.h"
#include "FDTSounds.h"
#include "FDTSprite.h"
#include "FAction.h"

// (tpixel.cpp is not linked)
const int TPixelRGBM32::maxChannelValue = 0xff;

struct Lcg {
	uint64_t s;
	explicit Lcg(uint64_t seed) : s(seed) {}
	uint32_t next()
	{
		s = s * 6364136223846793005ULL + 1442695040888963407ULL;
		return (uint32_t)(s >> 33);
	}
	uint32_t operator()(uint32_t n) { return next() % n; }
	uint32_t r32() { return (next() << 16) ^ (next() << 1) ^ (next() >> 7); }
};

static std::string hex(const uint8_t *p, size_t n)
{
	if (!n)
		return "-";
	static const char *d = "0123456789abcdef";
	std::string s;
	for (size_t i = 0; i < n; i++) {
		s += d[p[i] >> 4];
		s += d[p[i] & 15];
	}
	return s;
}
static std::string hex(const std::vector<uint8_t> &v) { return hex(v.data(), v.size()); }
static std::string num(long long v) { return std::to_string(v); }

static std::vector<uint8_t> bytesOf(FSWFStream &s)
{
	U32 n = s.FullSize();
	std::vector<uint8_t> v(n ? n : 1);
	s.WriteToMemory(v.data());
	v.resize(n);
	return v;
}
static std::string dump(FSWFStream &s) { return hex(bytesOf(s)); }
// partial bits left in the stream are kicked out first
static std::string dumpFlushed(FSWFStream &s)
{
	s.FlushBits();
	return dump(s);
}

static void resetIds()
{
	FObjCollection c;
	FSWFStream tmp;
	c.CreateSprite(&tmp, FRect());
}

// p one-bits to put the stream at every bit alignment before the record
static void prefix(FSWFStream &s, int p)
{
	if (p > 0)
		s.WriteBits((1u << p) - 1, p);
}

// a signed value in [-limit, limit], zero and the extremes over-represented
static int sval(Lcg &r, int limit)
{
	switch (r(8)) {
	case 0:
		return 0;
	case 1:
		return limit;
	case 2:
		return -limit;
	default:
		return (int)r(2 * limit + 1) - limit;
	}
}
// random clip event flags; bit 17 is ClipEventKeyPress
static U32 r32_flags(Lcg &r)
{
	U32 f = r.r32() & 0x0007ffff;
	if (r(2))
		f |= ClipEventKeyPress;
	else
		f &= ~(U32)ClipEventKeyPress;
	return f;
}
static int magnitude(Lcg &r) { return (1 << r(15)) - 1 + (int)r(2); }

// ---------------------------------------------------------------------------------------
// colour transforms

static void cxCases()
{
	Lcg r(0x5EED1001);
	for (int i = 0; i < 120; i++) {
		int p = i % 8;
		int lim[8];
		for (int k = 0; k < 8; k++)
			lim[k] = magnitude(r);
		U32 add = r(2), mult = r(2);
		int v[6];
		for (int k = 0; k < 6; k++)
			v[k] = sval(r, lim[k]);
		FCXForm cx(add, mult, v[0], v[1], v[2], v[3], v[4], v[5]);
		FSWFStream s;
		prefix(s, p);
		cx.WriteToSWFStream(&s);
		std::printf("cx|%d|%u,%u,%d,%d,%d,%d,%d,%d|%s\n", p, add, mult, v[0], v[1], v[2], v[3], v[4], v[5], dump(s).c_str());
	}
	for (int i = 0; i < 120; i++) {
		int p = i % 8;
		int lim[8];
		for (int k = 0; k < 8; k++)
			lim[k] = magnitude(r);
		U32 add = r(2), mult = r(2);
		int v[8];
		for (int k = 0; k < 8; k++)
			v[k] = sval(r, lim[k]);
		FCXFormWAlpha cx(add, mult, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
		FSWFStream s;
		prefix(s, p);
		cx.WriteToSWFStream(&s);
		std::printf("cxa|%d|%u,%u,%d,%d,%d,%d,%d,%d,%d,%d|%s\n", p, add, mult, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], dumpFlushed(s).c_str());
	}
}

// ---------------------------------------------------------------------------------------
// shape records

static void recordCases()
{
	Lcg r(0x5EED1002);
	for (int i = 0; i < 160; i++) {
		int p = i % 8;
		int dx, dy;
		int kind = r(5);
		int lim = 1 << r(16);
		if (lim > 65535)
			lim = 65535;
		dx = kind == 0 ? 0 : sval(r, lim);
		dy = kind == 1 ? 0 : sval(r, lim);
		if (kind == 4)
			dx = dy = 0;
		FShapeRecEdgeStraight e(dx, dy);
		FSWFStream s;
		prefix(s, p);
		e.WriteToSWFStream(&s);
		std::printf("straight|%d|%d,%d|%s\n", p, dx, dy, dumpFlushed(s).c_str());
	}
	for (int i = 0; i < 160; i++) {
		int p = i % 8;
		int lim = 1 << r(16);
		if (lim > 65535)
			lim = 65535;
		int d[4];
		for (int k = 0; k < 4; k++)
			d[k] = sval(r, lim);
		FShapeRecEdgeCurved e(d[0], d[1], d[2], d[3]);
		FSWFStream s;
		prefix(s, p);
		e.WriteToSWFStream(&s);
		std::printf("curved|%d|%d,%d,%d,%d|%s\n", p, d[0], d[1], d[2], d[3], dumpFlushed(s).c_str());
	}
	// change records without new styles (the head only)
	for (int i = 0; i < 200; i++) {
		int p = i % 8;
		U32 line = r(2), f1 = r(2), f0 = r(2), mv = r(2);
		int lim = 1 << r(16);
		if (lim > 65535)
			lim = 65535;
		int dx = sval(r, lim), dy = sval(r, lim);
		U32 nf = r(12), nl = r(12);
		U32 s0 = r(1u << nf), s1 = r(1u << nf), sl = r(1u << nl);
		FShapeRecChange c(0, line, f1, f0, mv, dx, dy, s0, s1, sl, 0, 0);
		c.IncludeNFillBitInfo(nf);
		c.IncludeNLineBitInfo(nl);
		FSWFStream s;
		prefix(s, p);
		c.WriteToSWFStream(&s);
		std::printf("change|%d|0,%u,%u,%u,%u,%d,%d,%u,%u,%u,%u,%u|%s\n", p, line, f1, f0, mv, dx, dy, s0, s1, sl, nf, nl, dumpFlushed(s).c_str());
	}
	for (int p = 0; p < 8; p++) {
		FShapeRecEnd e;
		FSWFStream s;
		prefix(s, p);
		e.WriteToSWFStream(&s);
		std::printf("shapeend|%d|%s\n", p, dump(s).c_str());
	}
	static const int counts[] = {0, 1, 2, 3, 100, 254, 255, 256, 300, 65535};
	for (int n : counts) {
		FFillStyleArray fa;
		FLineStyleArray la;
		for (int k = 0; k < n; k++) {
			fa.Add(new FFillStyleSolid(new FColor(k & 255, (k >> 1) & 255, 7)));
			la.Add(new FLineStyle(k, new FColor(1, 2, 3)));
		}
		FSWFStream s, t;
		fa.WriteToSWFStream(&s);
		la.WriteToSWFStream(&t);
		std::vector<uint8_t> a = bytesOf(s), b = bytesOf(t);
		size_t head = n >= 255 ? 3 : 1;
		std::printf("stylecount|%d|%s\n", n, hex(a.data(), head).c_str());
		std::printf("fillarr|%d|%s\n", n, hex(a).c_str());
		std::printf("linearr|%d|%s|%s\n", n, hex(b.data(), head).c_str(), hex(b).c_str());
	}
}

// ---------------------------------------------------------------------------------------
// whole shape tags from the object graph

static FColor *mkColor(Lcg &r, bool alpha)
{
	U32 a = r(256), b = r(256), c = r(256), d = r(256);
	return alpha ? new FColor(a, b, c, d) : new FColor(a, b, c);
}
static FMatrix *mkMatrix(Lcg &r)
{
	U32 d0 = r(2), d1 = r(2000), d2 = r(2000), d3 = r(2), d4 = r(2000), d5 = r(2000), d6 = r(9000), d7 = r(9000);
	return new FMatrix(d0, (int)d1 - 1000, (int)d2 - 1000, d3, (int)d4 - 1000, (int)d5 - 1000, (int)d6 - 4500, (int)d7 - 4500);
}
static FGradient *mkGradient(Lcg &r, bool morph, bool alpha)
{
	FGradient *g = new FGradient();
	int n = 1 + r(5);
	for (int i = 0; i < n; i++) {
		U32 ratio = r(256);
		if (morph) {
			U32 ratio2 = r(256);
			FColor *c1 = mkColor(r, true);
			FColor *c2 = mkColor(r, true);
			g->Add(new FMorphGradRecord(ratio, c1, ratio2, c2));
		} else
			g->Add(new FGradRecord(ratio, mkColor(r, alpha)));
	}
	return g;
}
static FFillStyle *mkFill(Lcg &r, bool alpha, bool solidOnly)
{
	switch (solidOnly ? 0 : r(3)) {
	case 0:
		return new FFillStyleSolid(mkColor(r, alpha));
	case 1: {
		int tiled = r(2);
		U16 id = r(65536);
		return new FFillStyleBitmap(tiled, id, mkMatrix(r));
	}
	default: {
		int linear = r(2);
		FMatrix *m = mkMatrix(r);
		return new FFillStyleGradient(linear, m, mkGradient(r, false, alpha));
	}
	}
}

static void addEdges(Lcg &r, int n, void (*add)(void *, FShapeRec *), void *target, int nf, int nl)
{
	for (int i = 0; i < n; i++) {
		FShapeRec *rec;
		switch (r(4)) {
		case 0:
			rec = new FShapeRecEdgeStraight(sval(r, 3000), sval(r, 3000));
			break;
		case 1:
			rec = new FShapeRecEdgeCurved(sval(r, 3000), sval(r, 3000), sval(r, 3000), sval(r, 3000));
			break;
		default: {
			U32 f0 = nf ? 1 + r(nf) : 0, f1 = nf ? 1 + r(nf) : 0, l = nl ? 1 + r(nl) : 0;
			rec = new FShapeRecChange(0, r(2), r(2), r(2), r(2), sval(r, 5000), sval(r, 5000), f0, f1, l, 0, 0);
			break;
		}
		}
		add(target, rec);
	}
}
template <class T>
static void addRec(void *t, FShapeRec *rec)
{
	((T *)t)->AddShapeRec(rec);
}

template <class T>
static void shapeCases(const char *name, bool alpha)
{
	Lcg r(0x5EED1003 + alpha + (name[5] == '2'));
	for (int i = 0; i < 24; i++) {
		resetIds();
		FRect *bounds = new FRect(sval(r, 4000), sval(r, 4000), sval(r, 8000), sval(r, 8000));
		T shape(bounds);
		int nfill = i < 2 ? i : r(i == 23 ? 300 : 5), nline = i < 2 ? i : r(i == 22 ? 300 : 4);
		for (int k = 0; k < nfill; k++) {
			if (r(3) == 0)
				shape.AddSolidFillStyle(mkColor(r, alpha));
			else
				shape.AddFillStyle(mkFill(r, alpha, false));
		}
		for (int k = 0; k < nline; k++)
			shape.AddLineStyle(r(500), mkColor(r, alpha));
		shape.FinishStyleArrays();
		// new style blocks are only legal in change records created with fill/line arrays
		FFillStyleArray *nfa = new FFillStyleArray();
		FLineStyleArray *nla = new FLineStyleArray();
		int mf = r(4), ml = r(3);
		for (int k = 0; k < mf; k++)
			nfa->Add(mkFill(r, alpha, false));
		for (int k = 0; k < ml; k++)
			nla->Add(new FLineStyle(r(50), mkColor(r, alpha)));
		shape.AddShapeRec(new FShapeRecChange(1, 1, 1, 1, 1, sval(r, 2000), sval(r, 2000), nfill ? 1 + r(nfill) : 0, nfill ? 1 + r(nfill) : 0, nline ? 1 + r(nline) : 0, nfa, nla));
		addEdges(r, 1 + r(12), addRec<T>, &shape, nfill, nline);
		shape.AddShapeRec(new FShapeRecEnd());
		FSWFStream s;
		shape.WriteToSWFStream(&s);
		std::printf("%s|%d|%d|%u|%s\n", name, nfill, nline, (unsigned)shape.ID(), dump(s).c_str());
	}
}

static void shape3ColorCases()
{
	Lcg r(0x5EED1007);
	for (int i = 0; i < 10; i++) {
		resetIds();
		FDTDefineShape3 shape;
		shape.setBounds(new FRect(0, 0, 100, 100));
		std::vector<FColor> cols;
		int n = 1 + r(4);
		for (int k = 0; k < n; k++) {
			FColor *c = new FColor(r(4) * 60, r(4) * 60, r(4) * 60, 255);
			cols.push_back(*c);
			shape.AddSolidFillStyle(c);
		}
		shape.AddLineStyle(20, new FColor(1, 2, 3, 255));
		shape.FinishStyleArrays();
		FFillStyleArray *nfa = new FFillStyleArray();
		FColor *c2 = new FColor(cols[0]);
		nfa->Add(new FFillStyleSolid(c2));
		shape.AddShapeRec(new FShapeRecChange(1, 0, 1, 0, 1, 10, 10, 0, 1, 0, nfa, new FLineStyleArray()));
		shape.AddShapeRec(new FShapeRecEdgeStraight(10, 20));
		shape.AddShapeRec(new FShapeRecEnd());
		if (i % 2) {
			std::map<TPixel, TPixel> table;
			table[TPixel(cols[0].Red(), cols[0].Green(), cols[0].Blue(), cols[0].Alpha())] = TPixel(9, 8, 7, 255);
			shape.changeColor(table);
		} else {
			shape.changeColor(cols[0], FColor(200, 100, 50, 255));
		}
		FSWFStream s;
		shape.WriteToSWFStream(&s);
		FRect bb = shape.GetBBox();
		std::printf("shape3color|%d|%d,%d|%s\n", i, bb.Xmax(), bb.Ymax(), dump(s).c_str());
	}
}

static void morphCases()
{
	Lcg r(0x5EED1008);
	for (int i = 0; i < 16; i++) {
		resetIds();
		FDTDefineMorphShape m(new FRect(0, 0, sval(r, 900), sval(r, 900)), new FRect(5, 5, sval(r, 900), sval(r, 900)));
		int nf = i < 2 ? i : r(5), nl = i < 2 ? i : r(4);
		for (int k = 0; k < nf; k++) {
			switch (r(3)) {
			case 0:
				m.AddFillStyle(new FMorphFillStyleSolid(mkColor(r, true), mkColor(r, true)));
				break;
			case 1: {
				int tiled = r(2);
				U16 id = r(65536);
				FMatrix *a = mkMatrix(r), *b = mkMatrix(r);
				m.AddFillStyle(new FMorphFillStyleBitmap(tiled, id, a, b));
				break;
			}
			default: {
				int linear = r(2);
				FMatrix *a = mkMatrix(r), *b = mkMatrix(r);
				m.AddFillStyle(new FMorphFillStyleGradient(linear, a, b, mkGradient(r, true, true)));
			}
			}
		}
		for (int k = 0; k < nl; k++) {
			U32 w1 = r(400), w2 = r(400);
			FColor *c1 = mkColor(r, true);
			FColor *c2 = mkColor(r, true);
			m.AddLineStyle(w1, c1, w2, c2);
		}
		m.FinishStyleArrays();
		int edges = 1 + r(6);
		m.AddShapeRec_1(new FShapeRecChange(0, 1, 1, 1, 1, sval(r, 200), sval(r, 200), nf ? 1 + r(nf) : 0, nf ? 1 + r(nf) : 0, nl ? 1 + r(nl) : 0, 0, 0));
		m.AddEdgeRec_2(new FShapeRecChange(0, 0, 0, 0, 1, sval(r, 200), sval(r, 200), 0, 0, 0, 0, 0));
		for (int k = 0; k < edges; k++) {
			int dx = sval(r, 300), dy = sval(r, 300);
			m.AddShapeRec_1(new FShapeRecEdgeStraight(dx, dy));
			m.AddEdgeRec_2(new FShapeRecEdgeStraight(dy, dx));
		}
		m.AddShapeRec_1(new FShapeRecEnd());
		m.AddEdgeRec_2(new FShapeRecEnd());
		FSWFStream s;
		m.WriteToSWFStream(&s);
		std::printf("morph|%d|%d|%u|%s\n", nf, nl, (unsigned)m.ID(), dump(s).c_str());
	}
}

// ---------------------------------------------------------------------------------------
// sprites

static void spriteCases()
{
	Lcg r(0x5EED1009);
	for (int i = 0; i < 12; i++) {
		resetIds();
		FDTSprite sp;
		std::vector<uint8_t> body;
		U32 frames = 0;
		int n = i < 3 ? i : r(8);
		for (int k = 0; k < n; k++) {
			FSWFStream one;
			FObj *o;
			switch (r(3)) {
			case 0:
				o = new FCTShowFrame();
				frames++;
				break;
			case 1:
				o = new FCTProtect();
				break;
			default:
				o = new FCTRemoveObject2((U16)r(500));
			}
			o->WriteToSWFStream(&one);
			std::vector<uint8_t> b = bytesOf(one);
			body.insert(body.end(), b.begin(), b.end());
			sp.AddFObj(o);
		}
		FSWFStream s;
		sp.WriteToSWFStream(&s);
		std::printf("spritebody|%u|%u|%s|%s\n", (unsigned)sp.ID(), frames, hex(body).c_str(), dump(s).c_str());
	}
	// a sprite holding shapes (nested orchestration, compared by --verify-chain)
	resetIds();
	FDTSprite sp;
	FDTDefineShape3 *sh = new FDTDefineShape3(new FRect(0, 0, 50, 50));
	sh->AddSolidFillStyle(new FColor(1, 2, 3, 4));
	sh->FinishStyleArrays();
	sh->AddShapeRec(new FShapeRecEdgeStraight(5, 5));
	sh->AddShapeRec(new FShapeRecEnd());
	sp.AddFObj(sh);
	sp.AddFObj(new FCTShowFrame());
	FSWFStream s;
	sp.WriteToSWFStream(&s);
	std::printf("spriteshape|%u|%s\n", (unsigned)sp.ID(), dump(s).c_str());
}

// ---------------------------------------------------------------------------------------
// actions

struct ByteAction : FActionRecord {
	std::vector<uint8_t> b;
	explicit ByteAction(const std::vector<uint8_t> &v) : b(v) {}
	virtual void WriteToSWFStream(FSWFStream *s)
	{
		for (uint8_t x : b)
			s->WriteByte(x);
	}
};

static std::vector<uint8_t> randBytes(Lcg &r, size_t n)
{
	std::vector<uint8_t> v(n);
	for (auto &x : v)
		x = r(256);
	return v;
}
static std::string randText(Lcg &r, size_t n)
{
	std::string t;
	for (size_t i = 0; i < n; i++)
		t += (char)(1 + r(255));
	return t;
}
static std::string textHex(const std::string &t) { return hex((const uint8_t *)t.data(), t.size()); }

static void recordAction(const char *name, const std::string &params, FActionRecord &a)
{
	FSWFStream s;
	a.WriteToSWFStream(&s);
	std::printf("%s|%s|%s\n", name, params.c_str(), dump(s).c_str());
}

static void actionCases()
{
	Lcg r(0x5EED100A);
	static const unsigned codes[] = {0x00, 0x05, 0x7f, 0x80, 0x81, 0x96, 0xff};
	for (int i = 0; i < 28; i++) {
		unsigned code = codes[i % 7];
		size_t len = i < 7 ? 0 : (i < 14 ? 1 : r(40));
		std::vector<uint8_t> d = randBytes(r, len + 1);
		FActionUnparsed a((UCHAR)code, (USHORT)len, d.data());
		FSWFStream s;
		a.WriteToSWFStream(&s);
		d.resize(len);
		std::printf("unparsed|%u|%u|%s|%s\n", code, (unsigned)len, hex(d).c_str(), dump(s).c_str());
	}
	for (int i = 0; i < 12; i++) {
		U8 m = r(3);
		FActionGetURL2 a(m);
		recordAction("geturl2", num(m), a);
		U16 f = r(65536);
		FActionGotoFrame g(f);
		recordAction("gotoframe", num(f), g);
		U8 pl = r(256);
		FActionGotoFrame2 g2(pl);
		recordAction("gotoframe2", num(pl), g2);
		S16 off = (S16)r(65536);
		FActionIf fi(off);
		recordAction("if", num(off), fi);
		FActionJump j(off);
		recordAction("jump", num(off), j);
		U16 fr = r(65536), sk = r(65536);
		FActionWaitForFrame w(fr, sk);
		recordAction("waitframe", num(fr) + "," + num(sk), w);
		U8 sk2 = r(256);
		FActionWaitForFrame2 w2(sk2);
		recordAction("waitframe2", num(sk2), w2);
	}
	// push: kind|number bits|value|text|bytes
	auto pushLine = [&](int kind, FActionPush &a, float number, int value, const std::string &text) {
		FSWFStream s;
		a.WriteToSWFStream(&s);
		std::vector<uint8_t> b = bytesOf(s);
		if (kind == 6)
			for (size_t k = b.size() - 8; k < b.size(); k++)
				b[k] = 0; // address-derived bytes
		uint32_t bits;
		std::memcpy(&bits, &number, 4);
		std::printf("push|%d|%u|%d|%s|%s\n", kind, bits, value, textHex(text).c_str(), hex(b).c_str());
	};
	for (int i = 0; i < 12; i++) {
		std::string t = randText(r, i < 2 ? i : r(30));
		FActionPush a(new FString(t.c_str()));
		pushLine(0, a, 0.0f, 0, std::string(t.c_str()));
	}
	for (int i = 0; i < 40; i++) {
		float f;
		switch (i % 6) {
		case 0:
			f = (float)((int)r(2000) - 1000);
			break;
		case 1:
			f = (float)r(2000) / 7.0f;
			break;
		case 2:
			f = -(float)r(100000) / 3.0f;
			break;
		case 3:
			f = (float)r(2000000000u) * 1.9f; // up to ~3.8e9: above INT_MAX, below 2^32
			break;
		case 4:
			f = 0.0f;
			break;
		default:
			f = -(float)r(300);
		}
		int kind = i < 20 ? 1 : (i < 30 ? 7 : 2 + (i % 2)); // 2 and 3 are unknown types: size 0, type byte only
		if (kind == 1) {
			FActionPush a(f);
			pushLine(1, a, f, 0, "");
		} else {
			FActionPush a((U8)kind, f);
			pushLine(kind, a, f, 0, "");
		}
	}
	for (int i = 0; i < 12; i++) {
		static const int kinds[] = {4, 5, 8};
		int kind = kinds[i % 3];
		U8 v = r(256);
		FActionPush a((U8)kind, v);
		pushLine(kind, a, 0.0f, v, "");
	}
	for (int i = 0; i < 8; i++) {
		double d = i == 0 ? 0.0 : (double)((int)r(1000000) - 500000) / 13.0;
		FActionPush a(d);
		pushLine(6, a, 0.0f, 0, "");
	}

	// conditions: the flag bit of every condition, then the key code shift
	for (int c = 0; c <= 11; c++) {
		FActionCondition cond;
		cond.AddCondition((U16)c);
		FSWFStream s;
		cond.WriteToSWFStream(&s);
		std::vector<uint8_t> b = bytesOf(s);
		unsigned flags = b[2] | (b[3] << 8);
		std::printf("condbit|%d|%u\n", c, flags);
		for (int k = 0; k < 3; k++) {
			U32 key = k == 0 ? 0x41 : k == 1 ? 0x7f : r(1024);
			FActionCondition c2;
			c2.AddCondition((U16)c);
			c2.AddKeyCode(key);
			FSWFStream s2;
			c2.WriteToSWFStream(&s2);
			std::vector<uint8_t> b2 = bytesOf(s2);
			std::printf("condkey|%u|%u|%u\n", flags, key, (unsigned)(b2[2] | (b2[3] << 8)));
		}
	}
	// whole condition lists with actions (both write forms)
	for (int i = 0; i < 10; i++) {
		FActionCondition cond;
		cond.AddCondition((U16)(1 + r(9)));
		cond.AddCondition((U16)(1 + r(9)));
		if (i % 2)
			cond.AddKeyCode(r(128));
		std::string desc;
		int n = r(4);
		for (int k = 0; k < n; k++) {
			std::vector<uint8_t> v = randBytes(r, r(6));
			cond.AddActionRecord(new ByteAction(v));
			desc += hex(v) + ";";
		}
		FSWFStream a, b;
		cond.WriteToSWFStream(&a);
		cond.WriteToSWFStream(&b, 1);
		std::printf("condition|%s|%s|%s\n", desc.c_str(), dump(a).c_str(), dump(b).c_str());
	}

	// clip action records: key press events add the key code
	for (int i = 0; i < 24; i++) {
		FClipActionRecord rec;
		U32 flags = i % 3 == 0 ? 0 : r32_flags(r);
		std::string desc;
		int n = r(4);
		for (int k = 0; k < n; k++) {
			std::vector<uint8_t> v = randBytes(r, r(6));
			rec.AddActionRecord(new ByteAction(v));
			desc += v.empty() ? std::string() : hex(v);
		}
		U8 key = r(256);
		rec.AddFlags(flags);
		rec.AddKeyCode(key);
		FSWFStream s;
		rec.WriteToSWFStream(&s);
		std::printf("cliprec|%s|%u|%u|%s\n", desc.empty() ? "-" : desc.c_str(), flags, (unsigned)key, dump(s).c_str());
	}
	for (int i = 0; i < 8; i++) {
		FClipAction ca;
		ca.AddFlags(r32_flags(r));
		int nrec = r(4);
		std::string desc;
		for (int k = 0; k < nrec; k++) {
			FClipActionRecord *rec = new FClipActionRecord();
			rec->AddFlags(r32_flags(r));
			rec->AddKeyCode(r(256));
			std::vector<uint8_t> v = randBytes(r, r(6));
			rec->AddActionRecord(new ByteAction(v));
			ca.AddClipActionRecord(rec);
			desc += hex(v) + ";";
		}
		FSWFStream s;
		ca.WriteToSWFStream(&s);
		std::printf("clipaction|%s|%s\n", desc.c_str(), dump(s).c_str());
	}

	// string records and the constant pool
	for (int i = 0; i < 10; i++) {
		std::string u = randText(r, r(20)), w = randText(r, r(10));
		FActionGetURL g(new FString(u.c_str()), new FString(w.c_str()));
		recordAction("geturl", textHex(std::string(u.c_str())) + "," + textHex(std::string(w.c_str())), g);
		std::string l = randText(r, r(20));
		FActionGotoLabel gl(new FString(l.c_str()));
		recordAction("gotolabel", textHex(std::string(l.c_str())), gl);
		std::string t = randText(r, r(20));
		FActionSetTarget st(new FString(t.c_str()));
		recordAction("settarget", textHex(std::string(t.c_str())), st);
		std::string c1 = randText(r, r(10));
		FActionConstantPool cp(new FString(c1.c_str()));
		int more = r(4);
		std::string desc = textHex(std::string(c1.c_str()));
		for (int k = 0; k < more; k++) {
			std::string cn = randText(r, r(10));
			cp.addConstant(new FString(cn.c_str()));
			desc += "," + textHex(std::string(cn.c_str()));
		}
		recordAction("constpool", desc, cp);
	}
}

// ---------------------------------------------------------------------------------------
// sounds

// FDTDefineSoundADPCM keeps the compressed block and the header fields protected
struct ExposedAdpcm : FDTDefineSoundADPCM {
	using FDTDefineSoundADPCM::FDTDefineSoundADPCM;
	std::vector<uint8_t> pcm() const { return std::vector<uint8_t>(pcmData.begin(), pcmData.end()); }
	const SSound &sd() const { return soundData; }
};

static void soundCases()
{
	Lcg r(0x5EED100B);
	for (int i = 0; i < 24; i++) {
		resetIds();
		U8 rate = r(4), size = r(2), type = r(2);
		if (!size && type)
			size = 1; // 8-bit stereo reads uninitialised memory in the C++
		U32 count = 2 + r(i < 20 ? 600 : 3000);
		size_t chunk = (size_t)count * (type + 1) * (size + 1);
		std::vector<uint8_t> data = randBytes(r, chunk + 64);
		int comp = 2 + r(4);
		ExposedAdpcm snd(rate, size, type, count, data.data(), (U32)chunk, comp);
		FSWFStream s;
		snd.WriteToSWFStream(&s);
		std::vector<uint8_t> pcm = snd.pcm();
		std::printf("snddef|%u|%u|%u|%u|%u|%u|%s|%s\n", (unsigned)snd.ID(), (unsigned)snd.sd().format, (unsigned)rate, (unsigned)size, (unsigned)type, (unsigned)snd.sd().sampleCount, hex(pcm).c_str(), dump(s).c_str());
		std::printf("sndmeta|%d|%d|%d|%d\n", snd.NumSamples(), snd.Format(), snd.SoundType(), (int)pcm.size());
	}
	for (int i = 0; i < 8; i++) {
		std::vector<uint8_t> d = randBytes(r, i == 0 ? 0 : r(200));
		FDTSoundStreamBlock b((U32)d.size(), d.data());
		FSWFStream s;
		b.WriteToSWFStream(&s);
		std::printf("sndblock|%s|%s\n", hex(d).c_str(), dump(s).c_str());
	}
	for (int i = 0; i < 12; i++) {
		U8 mix = r(256), st = r(2);
		U16 count = r(65536);
		FDTSoundStreamHead h(mix, st, count);
		FSWFStream s;
		h.WriteToSWFStream(&s);
		std::printf("sndhead|%u|%u|%u|%s\n", (unsigned)mix, (unsigned)st, (unsigned)count, dump(s).c_str());
	}
	for (int i = 0; i < 24; i++) {
		U8 mix = r(256), comp = r(16), rate = r(4), size = r(2), st = r(2);
		U16 count = r(65536);
		FSWFStream s;
		if (i % 2) {
			FDTSoundStreamHead2 h(mix, comp, rate, size, st, count);
			h.WriteToSWFStream(&s);
		} else {
			FDTSoundStreamHead2 h(mix, comp, size, st, count);
			h.WriteToSWFStream(&s);
			rate = 0; // the short constructor leaves the rate at 0
		}
		std::printf("sndhead2|%u|%u|%u|%u|%u|%u|%s\n", (unsigned)mix, (unsigned)comp, (unsigned)rate, (unsigned)size, (unsigned)st, (unsigned)count, dump(s).c_str());
	}
	// sound info; the envelope's members are never set by its constructor, so the bytes written
	// are the pre-filled memory (printed as the last-but-one field)
	for (int i = 0; i < 48; i++) {
		U8 sync = r(16), env = i % 2, loops = r(2), outp = r(2), inp = r(2);
		U32 inPoint = r.r32(), outPoint = r.r32();
		U16 loopCount = r(65536);
		U8 nPoints = r(256);
		alignas(FSndEnv) unsigned char mem[sizeof(FSndEnv) < 16 ? 16 : sizeof(FSndEnv)];
		std::vector<uint8_t> fill = randBytes(r, 8);
		std::memset(mem, 0, sizeof mem);
		std::memcpy(mem, fill.data(), 8);
		FSndEnv *e = 0;
		if (env)
			e = new (mem) FSndEnv(0x11111111, 0x2222, 0x3333); // leaves the memory untouched
		// (leaked on purpose: the destructor would delete memory that is not from operator new)
		FSoundInfo *info = new FSoundInfo(sync, env, loops, outp, inp, inPoint, outPoint, loopCount, nPoints, e);
		FSWFStream s;
		info->WriteToSWFStream(&s);
		std::printf("sndinfo|%u|%u|%u|%u|%u|%u|%u|%u|%u|%s|%s\n", (unsigned)sync, (unsigned)env, (unsigned)loops, (unsigned)outp, (unsigned)inp, inPoint, outPoint, (unsigned)loopCount, (unsigned)nPoints, hex(fill).c_str(), dump(s).c_str());
	}
}

int main()
{
	cxCases();
	recordCases();
	shapeCases<FDTDefineShape>("shape1", false);
	shapeCases<FDTDefineShape2>("shape2", false);
	shapeCases<FDTDefineShape3>("shape3", true);
	shape3ColorCases();
	morphCases();
	spriteCases();
	actionCases();
	soundCases();
	return 0;
}
