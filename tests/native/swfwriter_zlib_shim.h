// Force-included when the ORIGINAL FSWFStream.cpp / FDTBitmaps.cpp are rebuilt on an LP64 host.
//
// Both files call compress()/compress2() with `(uLongf *)&u32Variable`. uLongf is 32 bits on
// Windows (where the code was written) but 64 bits on LP64, so on Linux/macOS the original
// writes 8 bytes into a 4 byte local (`FSWFStream::WriteToFileVersion6`: stack smashing, the
// process aborts). These wrappers give the call the 32-bit semantics it was written for, so
// the reference build can run; the Rust port has no such width mismatch.
#pragma once
#include <stdint.h>
#include <zlib.h>

static inline int swf_compress32(Bytef *dest, uLongf *destLen, const Bytef *source, uLong sourceLen)
{
	uint32_t *p = (uint32_t *)destLen;
	uLongf n = *p;
	int r = compress(dest, &n, source, sourceLen);
	*p = (uint32_t)n;
	return r;
}
static inline int swf_compress2_32(Bytef *dest, uLongf *destLen, const Bytef *source, uLong sourceLen, int level)
{
	uint32_t *p = (uint32_t *)destLen;
	uLongf n = *p;
	int r = compress2(dest, &n, source, sourceLen, level);
	*p = (uint32_t)n;
	return r;
}
#define compress swf_compress32
#define compress2 swf_compress2_32
