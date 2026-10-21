// Differential reference harness for the SWF writer port (common/flash/FSWFStream, FPrimitive,
// FObj, FSound, FCT, FDTBitmaps -> *.rs).
//
// Compiled twice by gen_swfwriter_cases.py:
//   * against the ORIGINAL six .cpp files (+ their headers) rebuilt from git -> the golden;
//   * (--verify-chain) against the NEW inline headers + libflare_formats.a -> must print the
//     same bytes (this proves the C++ shells marshal like the old members did).
// FDTShapes.cpp, FAction.cpp and FDTSounds.cpp are not ported; they are linked from the same
// tree in both builds and supply FCXForm, FClipAction and FSoundInfo.
//
// Only the public API is used. Output is one case per line, fields separated by '|', byte
// strings in hex ("-" when empty). zlib streams are never printed raw (the Rust port uses
// miniz, so deflate bytes differ): they are inflated here and the inflated bytes are printed.
//
// Cases that the C++ would answer with undefined behaviour (uninitialised matrix fields,
// reads past the end of a sound buffer, abs(INT_MIN), 8-bit stereo ADPCM, products of
// 16.16 values that overflow int) are avoided on purpose.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <initializer_list>
#include <string>
#include <vector>
#include <zlib.h>

#include "FSWFStream.h"
#include "FObj.h"
#include "FPrimitive.h"
#include "FSound.h"
#include "FCT.h"
#include "FDTBitmaps.h"
#include "FDTShapes.h"
#include "FDTSounds.h"
#include "FAction.h"

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

// Draw several values in a fixed order (function arguments are evaluated in an unspecified
// order, which would make the recorded inputs depend on the compiler's inlining). A bound of 0
// means a full 32-bit value.
static std::vector<uint32_t> draw(Lcg &r, std::initializer_list<uint32_t> bounds)
{
	std::vector<uint32_t> v;
	for (uint32_t b : bounds)
		v.push_back(b ? r(b) : r.r32());
	return v;
}

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

static std::vector<uint8_t> fileBytes(FILE *f)
{
	std::fflush(f);
	long n = std::ftell(f);
	std::rewind(f);
	std::vector<uint8_t> v(n > 0 ? n : 1);
	size_t got = n > 0 ? std::fread(v.data(), 1, n, f) : 0;
	v.resize(got);
	return v;
}

static bool inflateAll(const uint8_t *p, size_t n, std::vector<uint8_t> &out)
{
	z_stream z;
	std::memset(&z, 0, sizeof z);
	if (inflateInit(&z) != Z_OK)
		return false;
	z.next_in = (Bytef *)p;
	z.avail_in = (uInt)n;
	uint8_t buf[65536];
	int r;
	do {
		z.next_out = buf;
		z.avail_out = sizeof buf;
		r = inflate(&z, Z_NO_FLUSH);
		if (r != Z_OK && r != Z_STREAM_END) {
			inflateEnd(&z);
			return false;
		}
		out.insert(out.end(), buf, buf + (sizeof buf - z.avail_out));
	} while (r != Z_STREAM_END);
	inflateEnd(&z);
	return true;
}

static void resetIds()
{
	FObjCollection c;
	FSWFStream tmp;
	c.CreateSprite(&tmp, FRect());
}

// ---------------------------------------------------------------------------------------
// FSWFStream: random op sequences

struct Op {
	char k;
	uint32_t a, b;
	std::vector<Op> sub;
};

static std::string opsText(const std::vector<Op> &ops)
{
	std::string t;
	for (const Op &o : ops) {
		if (!t.empty())
			t += ' ';
		switch (o.k) {
		case 'b': t += "b:" + num(o.a) + ":" + num(o.b); break;
		case 'd': t += "d:" + num(o.a); break;
		case 'w': t += "w:" + num(o.a); break;
		case 'y': t += "y:" + num(o.a); break;
		case 'f': t += "f"; break;
		case 'L': t += "L:" + num(o.a) + ":" + num(o.b); break;
		case 'N': t += "N:" + num(o.a) + ":" + num(o.b); break;
		case 'A': t += "A[ " + opsText(o.sub) + " ]"; break;
		case 'T': t += "T:" + num(o.a) + "[ " + opsText(o.sub) + " ]"; break;
		}
	}
	return t.empty() ? "-" : t;
}

static void genOps(Lcg &r, std::vector<Op> &ops, int n, int depth)
{
	for (int i = 0; i < n; i++) {
		Op o;
		o.k = 'b';
		o.a = o.b = 0;
		uint32_t c = r(100);
		if (c < 40) {
			o.k = 'b';
			uint32_t m = r(100);
			o.b = m < 5 ? 0 : m < 65 ? 1 + r(8) : m < 85 ? 9 + r(8) : m < 95 ? 17 + r(15) : 32;
			o.a = r.r32();
		} else if (c < 46) {
			o.k = 'd';
			o.a = r.r32();
		} else if (c < 52) {
			o.k = 'w';
			o.a = r.r32();
		} else if (c < 60) {
			o.k = 'y';
			o.a = r.r32();
		} else if (c < 68) {
			o.k = 'f';
		} else if (c < 76) {
			o.k = 'L';
			o.a = 1 + r(40);
			o.b = r(256);
		} else if (c < 90 && depth < 2) {
			o.k = 'A';
			genOps(r, o.sub, r(6), depth + 1);
		} else if (c < 97 && depth < 2) {
			static const unsigned ids[] = {1, 2, 6, 8, 20, 26, 32, 36, 1023};
			o.k = 'T';
			o.a = ids[r(9)];
			genOps(r, o.sub, r(8), depth + 1);
		} else {
			o.k = 'N';
			o.a = r(1024);
			o.b = r(100);
		}
		ops.push_back(o);
	}
}

static void exec(FSWFStream &s, const std::vector<Op> &ops, std::deque<std::vector<uint8_t> > &blobs, std::string *trace)
{
	for (const Op &o : ops) {
		switch (o.k) {
		case 'b': s.WriteBits(o.a, o.b); break;
		case 'd': s.WriteDWord(o.a); break;
		case 'w': s.WriteWord(o.a); break;
		case 'y': s.WriteByte(o.a); break;
		case 'f': s.FlushBits(); break;
		case 'L': {
			blobs.emplace_back(o.a);
			for (uint32_t i = 0; i < o.a; i++)
				blobs.back()[i] = (uint8_t)(o.b + i * 7);
			s.WriteLargeData(blobs.back().data(), o.a);
			break;
		}
		case 'A': {
			FSWFStream sub;
			exec(sub, o.sub, blobs, 0);
			s.Append(&sub);
			if (trace)
				*trace += "s" + num(sub.Size()) + ";";
			break;
		}
		case 'T': {
			FSWFStream body;
			exec(body, o.sub, blobs, 0);
			s.AppendTag((U16)o.a, body.Size(), &body);
			break;
		}
		case 'N': s.AppendTag((U16)o.a, o.b, 0); break;
		}
		if (trace)
			*trace += num(s.Size()) + ",";
	}
}

static void streamCases()
{
	Lcg r(0x5EED0001);
	std::vector<std::vector<Op> > cases;
	for (int i = 0; i < 160; i++) {
		std::vector<Op> ops;
		genOps(r, ops, 1 + r(24), 0);
		cases.push_back(ops);
	}
	// fixed edge cases: empty, every bit width, tag length special cases
	cases.push_back({});
	for (uint32_t sz = 0; sz <= 32; sz++)
		cases.push_back({Op{'b', 0xFFFFFFFFu, sz, {}}, Op{'b', 0x89ABCDEFu, sz, {}}, Op{'f', 0, 0, {}}});
	for (auto spec : std::vector<std::pair<unsigned, unsigned> >{{6, 62}, {6, 63}, {6, 64}, {32, 29}, {2, 28}, {26, 19}, {1, 0}, {1023, 1000}}) {
		Op t{'T', spec.first, 0, {}};
		for (unsigned i = 0; i < spec.second; i++)
			t.sub.push_back(Op{'y', i * 3u, 0, {}});
		cases.push_back({t});
		cases.push_back({Op{'N', spec.first, spec.second, {}}});
	}
	for (auto &ops : cases) {
		std::deque<std::vector<uint8_t> > blobs;
		FSWFStream s;
		std::string trace;
		exec(s, ops, blobs, &trace);
		U32 size = s.Size(), full = s.FullSize();
		std::string h = dump(s);
		// the same sequence through WriteToFile must give the same bytes
		FSWFStream s2;
		exec(s2, ops, blobs, 0);
		FILE *f = tmpfile();
		s2.WriteToFile(f);
		bool same = hex(fileBytes(f)) == h;
		std::fclose(f);
		std::printf("stream|%s|%s|%u|%u|%s|%d\n", opsText(ops).c_str(), trace.c_str(), size, full, h.c_str(), same ? 1 : 0);
	}
	// copy construction / assignment: independent copies
	for (int i = 0; i < 12; i++) {
		std::vector<Op> a, b;
		genOps(r, a, 1 + r(10), 1);
		genOps(r, b, 1 + r(10), 1);
		std::deque<std::vector<uint8_t> > blobs;
		FSWFStream s;
		exec(s, a, blobs, 0);
		FSWFStream c(s);
		FSWFStream d;
		d.WriteByte(9);
		d = s;
		exec(s, b, blobs, 0);
		std::string hs = dump(s), hc = dump(c), hd = dump(d);
		std::printf("streamcopy|%s|%s|%s|%s|%s\n", opsText(a).c_str(), opsText(b).c_str(), hs.c_str(), hc.c_str(), hd.c_str());
	}
	// MinBits / MaxNum
	static const uint32_t nums[] = {0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 127, 128, 255, 256, 1000, 65535, 65536, 1u << 20, (1u << 24) - 1, 1u << 24, 0x7FFFFFFFu, 0x80000000u, 0xFFFFFFFFu};
	for (uint32_t n : nums)
		for (unsigned sg = 0; sg < 2; sg++) {
			if (n == 0x80000000u || n == 0xFFFFFFFFu)
				continue; // MinBits would ask WriteBits for > 32 bits
			std::printf("minbits|%u|%u|%u\n", n, sg, FSWFStream::MinBits(n, sg));
		}
	for (int i = 0; i < 200; i++) {
		int32_t v[4];
		for (int k = 0; k < 4; k++) {
			uint32_t sh = r(31);
			uint32_t mag = r.r32() >> (31 - sh);
			uint32_t neg = r(2);
			v[k] = (int32_t)mag * (neg ? 1 : -1);
			if (v[k] == INT32_MIN)
				v[k] = 0;
		}
		std::printf("maxnum|%d|%d|%d|%d|%u\n", v[0], v[1], v[2], v[3], FSWFStream::MaxNum(v[0], v[1], v[2], v[3]));
	}
	// WriteToFileVersion6 on assembled images, normalised (see header comment)
	for (int i = 0; i < 40; i++) {
		FSWFStream s;
		uint32_t bodyLen = i < 4 ? i * 3 : r(700);
		uint32_t total = 8 + bodyLen;
		s.WriteByte('F');
		s.WriteByte('W');
		s.WriteByte('S');
		s.WriteByte(4);
		s.WriteDWord(total);
		uint32_t kind = r(3); // 0 zeros, 1 text-ish repeats, 2 noise
		for (uint32_t k = 0; k < bodyLen; k++)
			s.WriteByte(kind == 0 ? 0 : kind == 1 ? "flash movie "[k % 12] : r(256));
		std::string image = dump(s); // no large data: the stream stays intact
		FILE *f = tmpfile();
		s.WriteToFileVersion6(f);
		std::vector<uint8_t> file = fileBytes(f);
		std::fclose(f);
		bool comp = !file.empty() && file[0] == 'C';
		std::vector<uint8_t> payload;
		if (comp)
			inflateAll(file.data() + 8, file.size() - 8, payload);
		else
			payload.assign(file.begin() + (file.size() >= 8 ? 8 : file.size()), file.end());
		std::printf("v6|%u|%u|%s|%d|%s|%s\n", kind, bodyLen, image.c_str(), comp ? 1 : 0, hex(file.data(), file.size() >= 8 ? 8 : file.size()).c_str(), hex(payload).c_str());
	}
}

// ---------------------------------------------------------------------------------------
// FPrimitive

static void prefixBits(FSWFStream &s, unsigned p)
{
	if (p)
		s.WriteBits((1u << p) - 1, p);
}

static FMatrix randMatrix(Lcg &r, int range)
{
	std::vector<uint32_t> d = draw(r, {2, 2u * range + 1, 2u * range + 1, 2, 2u * range + 1, 2u * range + 1, 8u * range + 1, 8u * range + 1});
	return FMatrix(d[0], (int)d[1] - range, (int)d[2] - range, d[3], (int)d[4] - range, (int)d[5] - range, (int)d[6] - range * 4, (int)d[7] - range * 4);
}

static std::string matFields(const FMatrix &m)
{
	return num(m.hasScale) + "," + num(m.scaleX) + "," + num(m.scaleY) + "," + num(m.hasRotate) + "," + num(m.rotateSkew0) + "," + num(m.rotateSkew1) + "," + num(m.translateX) + "," + num(m.translateY) + "," + num(m.identity);
}

static void primitiveCases()
{
	Lcg r(0x5EED0002);
	static const unsigned prefixes[] = {0, 3, 7};
	for (int i = 0; i < 40; i++) {
		unsigned kind = i % 4, p = prefixes[i % 3];
		U32 rr = r(256), g = r(256), b = r(256), a = r(256);
		FSWFStream s;
		prefixBits(s, p);
		FColor c = kind == 0 ? FColor(rr, g, b) : kind == 1 ? FColor(rr, g, b, a) : kind == 2 ? FColor(FRGB{(U8)rr, (U8)g, (U8)b}) : FColor(FRGBA{(U8)rr, (U8)g, (U8)b, (U8)a});
		if (i % 7 == 6)
			c.AlphaChannel(!c.HasAlpha());
		c.WriteToSWFStream(&s);
		std::printf("color|%u|%u|%u|%u|%u|%u|%d|%s\n", kind, rr, g, b, a, p, c.HasAlpha() ? 1 : 0, dump(s).c_str());
	}
	for (int i = 0; i < 160; i++) {
		int range = i < 80 ? 40 : i < 140 ? 3000 : 1 << 14;
		unsigned p = prefixes[i % 3];
		FMatrix m = randMatrix(r, range);
		if (i % 17 == 0)
			m = FMatrix(0, 0, 0, 0, 0, 0, 0, 0);
		FSWFStream s;
		prefixBits(s, p);
		m.WriteToSWFStream(&s);
		std::printf("matrix|%d|%d|%d|%d|%d|%d|%d|%d|%u|%u,%u,%u|%s\n", (int)m.hasScale, m.scaleX, m.scaleY, (int)m.hasRotate, m.rotateSkew0, m.rotateSkew1, m.translateX, m.translateY, p, m.nScaleBits, m.nRotateBits, m.nTranslateBits, dump(s).c_str());
	}
	for (unsigned p : prefixes) {
		FMatrix m;
		FSWFStream s;
		prefixBits(s, p);
		m.WriteToSWFStream(&s);
		std::printf("matrixid|%u|%d,%d,%d,%d,%d|%s\n", p, (int)m.hasScale, m.scaleX, m.scaleY, (int)m.hasRotate, m.identity, dump(s).c_str());
	}
	for (int i = 0; i < 60; i++) {
		FMatrix a = randMatrix(r, 90), b = randMatrix(r, 90);
		FMatrix c = a * b;
		FSWFStream s;
		c.WriteToSWFStream(&s);
		std::printf("matmul|%s|%s|%s|%s\n", matFields(a).c_str(), matFields(b).c_str(), matFields(c).c_str(), dump(s).c_str());
	}
	FMatrix id;
	static const int mm[][2] = {{0, 0}, {1, 0}, {0, 1}, {-1, 0}, {5, -9}, {-9, 5}, {255, 256}, {-65536, 3}, {100000, -100001}, {2147483647, 0}, {0, -2147483647}};
	for (auto &q : mm)
		std::printf("fmmin|%d|%d|%u\n", q[0], q[1], id.MinBits(q[0], q[1]));
	for (int i = 0; i < 80; i++) {
		int rg = i < 20 ? 4 : i < 50 ? 300 : i < 70 ? 40000 : 1 << 28;
		auto sv = [&]() { return (int)r(2 * rg + 1) - rg; };
		int x0 = sv(), x1 = sv(), y0 = sv(), y1 = sv();
		if (i % 13 == 0)
			x0 = x1 = y0 = y1 = 0;
		unsigned p = prefixes[i % 3];
		FRect rc(x0, y0, x1, y1);
		FSWFStream s;
		prefixBits(s, p);
		rc.WriteToSWFStream(&s);
		std::printf("rect|%d|%d|%d|%d|%u|%s\n", x0, x1, y0, y1, p, dump(s).c_str());
	}
	for (int i = 0; i < 30; i++) {
		std::vector<uint8_t> t(r(14));
		for (auto &c : t)
			c = 1 + r(255);
		t.push_back(0);
		FString fs((const char *)t.data());
		bool nul = i & 1;
		unsigned p = prefixes[i % 3];
		FSWFStream s;
		prefixBits(s, p);
		fs.WriteToSWFStream(&s, nul);
		t.pop_back();
		std::printf("string|%s|%d|%u|%u|%s\n", hex(t).c_str(), nul ? 1 : 0, p, (unsigned)fs.Length(), dump(s).c_str());
	}
}

// ---------------------------------------------------------------------------------------
// FObj / FObjCollection

struct RawObj : FObj {
	std::vector<uint8_t> b;
	bool show;
	RawObj(const std::vector<uint8_t> &v, bool sh) : b(v), show(sh) {}
	virtual void WriteToSWFStream(FSWFStream *s)
	{
		for (uint8_t x : b)
			s->WriteByte(x);
	}
	virtual U32 IsShowFrame() { return show; }
};

static void objCases()
{
	Lcg r(0x5EED0003);
	std::deque<std::vector<uint8_t> > keep;
	for (int i = 0; i < 40; i++) {
		FObjCollection coll;
		int n = r(6);
		std::vector<RawObj *> objs;
		std::string script;
		unsigned frames = 0;
		for (int k = 0; k < n; k++) {
			std::vector<uint8_t> v(r(9));
			for (auto &x : v)
				x = r(256);
			bool sh = r(2);
			RawObj *o = new RawObj(v, sh);
			if (k > 0 && r(3) == 0) {
				int at = r(objs.size() + 1);
				coll.InsertFObj(at, o);
				objs.insert(objs.begin() + at, o);
				script += "i" + num(at) + ":";
			} else {
				coll.AddFObj(o);
				objs.push_back(o);
				script += "a:";
			}
			script += (sh ? "1:" : "0:") + hex(v) + " ";
			frames += sh;
		}
		bool ok = coll.GetFObjCount() == (int)objs.size() && coll.GetFObj(objs.size()) == 0;
		for (size_t k = 0; k < objs.size(); k++)
			ok = ok && coll.GetFObj((int)k) == objs[k];
		std::vector<uint32_t> cd = draw(r, {30000, 30000, 30000, 30000});
		FRect cam(-(int)cd[0], -(int)cd[1], (int)cd[2], (int)cd[3]);
		if (i % 5 == 0)
			cam = FRect(0, 0, 640 * 20, 480 * 20);
		U32 rate = r(256);
		U8 ver = i % 3 == 0 ? 6 : (U8)r(256);

		FSWFStream movie;
		coll.CreateMovie(&movie, cam, rate, ver);
		std::string ids;
		for (int k = 0; k < 3; k++)
			ids += num(FObjCollection::Increment()) + ",";
		FSWFStream sprite;
		coll.CreateSprite(&sprite, cam, rate);
		ids += "/" + num(FObjCollection::Increment());
		FILE *f = tmpfile();
		coll.CreateMovie(f, cam, rate, ver);
		std::vector<uint8_t> mv = bytesOf(movie);
		std::vector<uint8_t> fl = fileBytes(f);
		std::fclose(f);
		f = tmpfile();
		coll.CreateCompressedMovie(f, cam, rate);
		std::vector<uint8_t> cm = fileBytes(f);
		std::fclose(f);
		bool comp = !cm.empty() && cm[0] == 'C';
		std::vector<uint8_t> payload;
		if (comp)
			inflateAll(cm.data() + 8, cm.size() - 8, payload);
		else
			payload.assign(cm.begin() + (cm.size() >= 8 ? 8 : cm.size()), cm.end());
		std::vector<uint8_t> body(mv.begin() + 21, mv.end() - 2);
		std::printf("fobjmovie|%d|%d|%d|%d|%u|%u|%u|%s|%s\n", cam.Xmin(), cam.Xmax(), cam.Ymin(), cam.Ymax(), rate, (unsigned)ver, frames, hex(body).c_str(), hex(mv).c_str());
		std::printf("fobjmisc|%s|%d|%d|%s|%s|%d|%s|%s\n", script.c_str(), ok ? 1 : 0, (int)(fl == mv), hex(bytesOf(sprite)).c_str(), ids.c_str(), comp ? 1 : 0, hex(cm.data(), cm.size() >= 8 ? 8 : cm.size()).c_str(), hex(payload).c_str());
		// EraseFObj drops the entry (it does not delete it; the objects are leaked on purpose)
		if (!objs.empty()) {
			coll.EraseFObj(0);
			std::printf("fobjerase|%d\n", coll.GetFObjCount());
		}
	}
	// FFragment writes its block as large data
	std::vector<uint8_t> blob = {1, 2, 3, 4, 5};
	FFragment frag(blob.data(), (int)blob.size());
	FSWFStream s;
	s.WriteByte(7);
	frag.WriteToSWFStream(&s);
	s.WriteByte(8);
	std::printf("fragment|%u|%s\n", s.Size(), dump(s).c_str());
}

// ---------------------------------------------------------------------------------------
// FSound / FSoundComp

static void soundCases()
{
	Lcg r(0x5EED0004);
	for (int fmt = 0; fmt < 16; fmt++) {
		FSound s;
		s.Init();
		s.format = ((fmt & 1) ? sndStereo : sndMono) | ((fmt & 2) ? snd16Bit : snd8Bit) | ((fmt >> 2) << 2);
		WaveFormat w;
		std::memset(&w, 0, sizeof w);
		s.Set(&w);
		s.nSamples = 1000;
		std::printf("fsound|%d|%d,%d,%d,%d|%d,%d,%d,%d,%d,%d|%d,%d,%d|%d|%d\n", s.format, (int)s.Rate(), s.RateShift(), s.Stereo(), s.Is8Bit(), (int)w.wFormatTag, (int)w.nChannels, (int)w.nSamplesPerSec, (int)w.nAvgBytesPerSec, (int)w.nBlockAlign, (int)w.wBitsPerSample, s.NChannels(), s.BitsPerSample(), s.BytesPerSample(), (int)s.GetDuration44(), (int)s.SizeBytes());
	}
	for (int i = 0; i < 48; i++) {
		bool stereo = i % 2, is8 = (i / 2) % 2;
		if (is8 && stereo)
			continue; // reads uninitialised stack in the C++
		int nbits = 2 + (i / 4) % 4;
		// total frames: include header boundaries (4096), tiny, and long runs
		static const int totals[] = {2, 3, 17, 300, 4095, 4096, 4097, 4098, 8192, 9000, 12289};
		int total = totals[(i * 5 + (i >> 3)) % 11];
		// cut into pieces (each >= 2 so the 8 bit lookahead stays inside the chunk buffer)
		std::vector<int> pieces;
		int left = total;
		while (left > 0) {
			int p = (i % 3 == 0) ? left : 2 + (int)r(2 + left / 2);
			if (p > left || left - p == 1)
				p = left;
			pieces.push_back(p);
			left -= p;
		}
		int sn = stereo ? 2 : 1;
		size_t elems = (size_t)total * sn;
		std::vector<uint8_t> data(elems * (is8 ? 1 : 2) + 64, 0);
		int kind = r(3);
		for (size_t k = 0; k < elems; k++) {
			int v = kind == 0 ? (int)(r(65536)) - 32768 : kind == 1 ? (int)(12000.0 * ((k / sn) % 40 < 20 ? 1 : -1)) + (int)r(200) : (int)(k * 37 % 65536) - 32768;
			if (v > 32767)
				v = 32767;
			if (v < -32768)
				v = -32768;
			if (is8) {
				data[k] = (uint8_t)(v >> 8) + 128;
			} else {
				data[2 * k] = v & 0xff;
				data[2 * k + 1] = (v >> 8) & 0xff;
			}
		}
		FSound snd;
		snd.Init();
		snd.format = (stereo ? sndStereo : sndMono) | (is8 ? snd8Bit : snd16Bit) | snd22K;
		FSoundComp comp(&snd, nbits);
		std::vector<U8> out;
		size_t off = 0;
		std::string ps;
		for (int p : pieces) {
			comp.Compress(data.data() + off, p, &out);
			off += (size_t)p * sn * (is8 ? 1 : 2);
			ps += num(p) + ",";
		}
		comp.Flush(&out);
		std::printf("sound|%d|%d|%d|%s|%s|%s\n", stereo, is8, nbits, ps.c_str(), hex(data.data(), elems * (is8 ? 1 : 2)).c_str(), hex(out).c_str());
	}
}

// ---------------------------------------------------------------------------------------
// FCT

struct ByteAction : FActionRecord {
	std::vector<uint8_t> b;
	explicit ByteAction(const std::vector<uint8_t> &v) : b(v) {}
	virtual void WriteToSWFStream(FSWFStream *s)
	{
		for (uint8_t x : b)
			s->WriteByte(x);
	}
};

static FMatrix *mkMatrix(Lcg &r)
{
	std::vector<uint32_t> d = draw(r, {2, 2000, 2000, 2, 2000, 2000, 9000, 9000});
	return new FMatrix(d[0], (int)d[1] - 1000, (int)d[2] - 1000, d[3], (int)d[4] - 1000, (int)d[5] - 1000, (int)d[6] - 4500, (int)d[7] - 4500);
}
static FCXForm *mkCx(Lcg &r)
{
	std::vector<uint32_t> d = draw(r, {2, 2, 512, 512, 512, 512, 512, 512});
	return new FCXForm(d[0], d[1], (int)d[2] - 256, (int)d[3] - 256, (int)d[4] - 256, (int)d[5] - 256, (int)d[6] - 256, (int)d[7] - 256);
}
static FCXFormWAlpha *mkCxA(Lcg &r)
{
	std::vector<uint32_t> d = draw(r, {2, 2, 512, 512, 512, 512, 512, 512, 512, 512});
	return new FCXFormWAlpha(d[0], d[1], (int)d[2] - 256, (int)d[3] - 256, (int)d[4] - 256, (int)d[5] - 256, (int)d[6] - 256, (int)d[7] - 256, (int)d[8] - 256, (int)d[9] - 256);
}

static std::string mcx(FMatrix *m)
{
	return m ? "m" : "-";
}

static void fctCases()
{
	Lcg r(0x5EED0005);
	auto one = [&](const char *name, const std::string &params, FObj &o) {
		FSWFStream s;
		o.WriteToSWFStream(&s);
		std::printf("fct|%s|%s|%s|%u|%u\n", name, params.c_str(), dump(s).c_str(), (unsigned)o.IsShowFrame(), (unsigned)o.IsPlaceObject());
	};
	{
		FCTProtect p;
		one("protect", "-", p);
		FCTShowFrame sf;
		one("showframe", "-", sf);
	}
	for (int i = 0; i < 20; i++) {
		U16 ch = r.r32(), d = r.r32();
		FCTRemoveObject ro(ch, d);
		one("remove", num(ch) + "," + num(d), ro);
		FCTRemoveObject2 ro2(d);
		one("remove2", num(d), ro2);
	}
	for (int i = 0; i < 10; i++) {
		std::vector<uint8_t> t(r(40));
		for (auto &c : t)
			c = 1 + r(255);
		std::string h = hex(t);
		t.push_back(0);
		FCTFrameLabel fl(new FString((const char *)t.data()));
		one("framelabel", h, fl);
	}
	for (int i = 0; i < 12; i++) {
		U32 cr = r(256), cg = r(256), cb = r(256), ca = r(256);
		FColor *c = i % 2 ? new FColor(cr, cg, cb, ca) : new FColor(cr, cg, cb);
		FCTSetBackgroundColor bg(c);
		one("bgcolor", num(cr) + "," + num(cg) + "," + num(cb) + "," + num(ca) + "," + num(i % 2), bg);
		c->AlphaChannel(i % 3 == 0); // changed through the owner pointer after construction
		one("bgcolor2", num(c->HasAlpha()), bg);
	}
	for (int i = 0; i < 24; i++) {
		static const unsigned ids[] = {1, 6, 7, 11, 12, 14, 20, 22, 26, 35, 36, 39, 46, 999};
		unsigned id = ids[i % 14];
		std::vector<U8> data(i < 4 ? 0 : (i < 8 ? 1 : r(140)));
		for (auto &x : data)
			x = r(256);
		FCTUnparsedTag t((U16)id, (U32)data.size(), data.data());
		FSWFStream s;
		t.WriteToSWFStream(&s);
		std::string d1 = dump(s); // large data is read when written out, i.e. before SetID below
		unsigned got = 0;
		if (data.size() >= 2) {
			got = t.GetID();
			t.SetID(0xA1B2);
		}
		FSWFStream s2;
		t.WriteToSWFStream(&s2);
		std::printf("unparsed|%u|%s|%u|%s|%s\n", id, hex(data).c_str(), (unsigned)got, d1.c_str(), dump(s2).c_str());
	}
	for (int i = 0; i < 10; i++) {
		FCTDoAction da;
		std::string desc;
		int n = r(5);
		for (int k = 0; k < n; k++) {
			std::vector<uint8_t> v(r(10));
			for (auto &x : v)
				x = r(256);
			da.AddAction(new ByteAction(v));
			desc += hex(v) + ";";
		}
		FSWFStream s;
		da.WriteToSWFStream(&s);
		std::printf("doaction|%s|%s\n", desc.c_str(), dump(s).c_str());
	}
	for (int i = 0; i < 12; i++) {
		U16 ch = r(3000), d = r(3000);
		FMatrix *m = mkMatrix(r);
		FCXForm *cx = mkCx(r);
		FCTPlaceObject po(ch, d, m, cx);
		one("place", num(ch) + "," + num(d) + "," + matFields(*m) + ",cx", po);
	}
	for (int i = 0; i < 48; i++) {
		U16 hasClipDepth = r(2), hasRatio = r(2), hasChar = r(2), hasMove = r(2);
		U16 depth = r(3000), ch = r(3000), ratio = r(3000), clipDepth = r(3000);
		bool hasM = r(2), hasCx = r(2), hasName = r(2), hasClip = i >= 24 && r(2);
		FMatrix *m = hasM ? mkMatrix(r) : 0;
		FCXFormWAlpha *cx = hasCx ? mkCxA(r) : 0;
		FString *name = hasName ? new FString("layer") : 0;
		FClipAction *clip = 0;
		if (hasClip) {
			clip = new FClipAction();
			clip->AddFlags(r(1 << 16));
			FClipActionRecord *rec = new FClipActionRecord();
			rec->AddFlags(r(1 << 16));
			rec->AddKeyCode(r(256));
			clip->AddClipActionRecord(rec);
		}
		FCTPlaceObject2 *po = i < 24 ? new FCTPlaceObject2(hasClipDepth, hasRatio, hasChar, hasMove, depth, ch, m, cx, ratio, name, clipDepth)
									 : new FCTPlaceObject2(hasClip, hasClipDepth, hasRatio, hasChar, hasMove, depth, ch, m, cx, ratio, name, clipDepth, clip);
		std::string params = num(hasClip) + "," + num(hasClipDepth) + "," + num(hasRatio) + "," + num(hasChar) + "," + num(hasMove) + "," + num(depth) + "," + num(ch) + "," + (m ? matFields(*m) : "-") + "," + (hasCx ? "cx" : "-") + "," + num(ratio) + "," + (hasName ? "name" : "-") + "," + num(clipDepth);
		FSWFStream s;
		po->WriteToSWFStream(&s);
		std::string accessors = num(po->GetDepth()) + "," + num(po->GetPlacedId()) + "," + po->GetName() + "," + num(po->IsPlaceObject());
		// mutators
		po->SetDepth(depth + 1);
		po->ChangePlacedId(ch + 1);
		po->SetId(ch + 2);
		po->ApplyMatrix(FMatrix(1, 3, 4, 1, 5, 6, 7, 8));
		FSWFStream s2;
		po->WriteToSWFStream(&s2);
		std::printf("place2|%s|%s|%s|%s|%s\n", params.c_str(), accessors.c_str(), dump(s).c_str(), dump(s2).c_str(), mcx(m).c_str());
		if (!hasCx) {
			// the copy constructor deep-copies matrix and name only (it shares colorTransform and drops clipAction)
			FCTPlaceObject2 cp(*po);
			FSWFStream s3;
			cp.WriteToSWFStream(&s3);
			std::printf("place2copy|%s|%s\n", params.c_str(), dump(s3).c_str());
		}
		delete po;
	}
	for (int i = 0; i < 12; i++) {
		U16 sid = r(3000);
		// FSndEnv's constructor (FDTSounds.cpp, not ported) assigns its parameters to themselves and
		// leaves the members uninitialised, so no envelope is recorded.
		bool env = false;
		std::vector<uint32_t> ed = draw(r, {0, 65536, 65536});
		FSndEnv *e = env ? new FSndEnv(ed[0], ed[1], ed[2]) : 0;
		std::vector<uint32_t> id = draw(r, {16, 2, 2, 2, 0, 0, 65536});
		FSoundInfo *info = new FSoundInfo(id[0], env, id[1], id[2], id[3], id[4], id[5], id[6], 1, e);
		FCTStartSound ss(sid, info);
		one("startsound", num(sid) + "," + num(env), ss);
	}
}

// ---------------------------------------------------------------------------------------
// FDTBitmaps

static std::vector<uint8_t> randBytes(Lcg &r, size_t n, int kind)
{
	std::vector<uint8_t> v(n);
	for (size_t i = 0; i < n; i++)
		v[i] = kind == 0 ? 0 : kind == 1 ? (uint8_t)((i / 7) * 3) : (uint8_t)r(256);
	return v;
}

static void bitmapCases()
{
	Lcg r(0x5EED0006);
	for (int i = 0; i < 12; i++) {
		resetIds();
		size_t n = i < 3 ? i : r(60);
		std::vector<uint8_t> img = randBytes(r, n, 2);
		FDTDefineBits b((U32)n, img.data());
		FSWFStream s;
		b.WriteToSWFStream(&s);
		std::printf("defbits|%s|%u|%s\n", hex(img).c_str(), (unsigned)b.ID(), dump(s).c_str());
	}
	for (int i = 0; i < 12; i++) {
		resetIds();
		size_t n = i < 3 ? i : r(120);
		std::vector<uint8_t> img = randBytes(r, n, 2);
		std::string ih = hex(img);
		FDTDefineBitsJPEG2 b(img.data(), (U32)n);
		img.assign(n, 0xEE); // JPEG2 copied its input
		FSWFStream s;
		b.WriteToSWFStream(&s);
		std::printf("jpeg2|%s|%u|%s\n", ih.c_str(), (unsigned)b.ID(), dump(s).c_str());
	}
	for (int i = 0; i < 12; i++) {
		resetIds();
		size_t n = i < 3 ? i : r(120), na = i < 3 ? 0 : r(90);
		std::vector<uint8_t> img = randBytes(r, n, 2), al = randBytes(r, na, 1);
		FDTDefineBitsJPEG3 b(img.data(), (U32)n, al.data(), (U32)na);
		FSWFStream s;
		b.WriteToSWFStream(&s);
		std::printf("jpeg3|%s|%s|%u|%s\n", hex(img).c_str(), hex(al).c_str(), (unsigned)b.ID(), dump(s).c_str());
	}
	for (int i = 0; i < 6; i++) {
		size_t n = i == 0 ? 1 : r(200);
		std::vector<uint8_t> enc = randBytes(r, n, 2);
		FDTJPEGTables t((U32)n, enc.data());
		FSWFStream s;
		t.WriteToSWFStream(&s);
		std::printf("jpegtables|%s|%s\n", hex(enc).c_str(), dump(s).c_str());
	}
	// lossless: format 0..5, with and without alpha. Printed after inflating.
	for (int i = 0; i < 60; i++) {
		resetIds();
		int format = i % 6;
		bool alpha = (i / 6) % 2;
		int bits = 1 << format;
		int w = 1 + r(i < 40 ? 24 : 90), h = 1 + r(i < 40 ? 16 : 70);
		int count = format <= 3 ? 1 + r(format == 0 ? 2 : format == 1 ? 4 : format == 2 ? 16 : 256) : 0;
		int rgb = alpha ? 4 : 3;
		std::vector<uint8_t> table = randBytes(r, (size_t)count * rgb, r(3));
		int imageBytes = (w * h * bits + 7) / 8;
		std::vector<uint8_t> img = randBytes(r, imageBytes, r(3));
		FDTDefineBitsLosslessBase *o = 0;
		if (alpha)
			o = new FDTDefineBitsLossless2((U8)format, (U16)w, (U16)h, count, count ? (const FRGBA *)table.data() : 0, img.data());
		else
			o = new FDTDefineBitsLossless((U8)format, (U16)w, (U16)h, count, count ? (const FRGB *)table.data() : 0, img.data());
		FSWFStream s;
		o->WriteToSWFStream(&s);
		std::vector<uint8_t> t = bytesOf(s);
		// parse the tag header
		unsigned code = t[0] | (t[1] << 8);
		size_t hdr = 2, len = code & 0x3f;
		if (len == 0x3f) {
			len = t[2] | (t[3] << 8) | (t[4] << 16) | ((size_t)t[5] << 24);
			hdr = 6;
		}
		bool lenok = hdr + len == t.size();
		size_t prefix = 2 + 1 + 2 + 2 + (format <= 3 ? 1 : 0);
		std::vector<uint8_t> payload;
		bool infl = inflateAll(t.data() + hdr + prefix, t.size() - hdr - prefix, payload);
		std::printf("lossless|%d|%d|%d|%d|%d|%s|%s|%u|%u|%d|%d|%s|%s\n", alpha, format, w, h, count, hex(table).c_str(), hex(img).c_str(), (unsigned)o->ID(), code >> 6, lenok, infl, hex(t.data() + hdr, prefix).c_str(), hex(payload).c_str());
		delete o;
	}
}

int main()
{
	streamCases();
	primitiveCases();
	objCases();
	soundCases();
	fctCases();
	bitmapCases();
	return 0;
}
