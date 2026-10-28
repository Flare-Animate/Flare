// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/09/1999.

/****************************************************************************************

				File Summary: FSWFStream.h

	This header-file contains the declarations of low-level SWF-stream class, i.e.
	
		class FSWFStream;

****************************************************************************************/

#ifndef SWFSTREAM_INCLUDED
#define SWFSTREAM_INCLUDED

#ifdef WIN32 // added from DV
#pragma warning(push)
#pragma warning(disable : 4786)
#pragma warning(disable : 4251)

#endif

#include "Macromedia.h"

// #include <stdio.h>
// #include <fstream.h>
#include <list>
#include <vector>
#include <new>
#include <stdexcept>
#include <stdio.h>
#include <assert.h>

#include "tcommon.h"

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

// FSWFStream.cpp was ported to FSWFStream.rs: the byte/bit stream state lives behind an
// opaque handle in Rust (flare_swfs_*) and this class is a thin shell over it. Statuses:
// 0 ok, 1 null/invalid argument, 2 panic caught, 3 buffer too small.
extern "C" {
void *flare_swfs_new();
void flare_swfs_free(void *h);
void *flare_swfs_clone(const void *h);
int flare_swfs_write_bits(void *h, unsigned int data, unsigned int size);
int flare_swfs_write_large_data(void *h, const unsigned char *data, unsigned int size);
int flare_swfs_flush_bits(void *h);
int flare_swfs_write_dword(void *h, unsigned int data);
int flare_swfs_write_word(void *h, unsigned int data);
int flare_swfs_write_byte(void *h, unsigned int data);
unsigned int flare_swfs_size(void *h);
unsigned int flare_swfs_full_size(void *h);
unsigned char *flare_swfs_memory(void *h);
int flare_swfs_append(void *h, void *add);
int flare_swfs_append_tag(void *h, unsigned short tag_id, unsigned int length, void *body);
int flare_swfs_write_to_memory(void *h, unsigned char *mem, unsigned int cap);
unsigned int flare_swfs_min_bits(unsigned int number, unsigned short sign);
unsigned int flare_swfs_max_num(int a, int b, int c, int d);
int flare_swfs_v6_pack(const unsigned char *buf, unsigned int size, unsigned char *out, unsigned int out_cap, unsigned int *out_len);
}

// class used to store data before it is written to a .swf file

class DVAPI FSWFStream
{
public:
	FSWFStream() : m_h(flare_swfs_new())
	{
		if (!m_h)
			throw std::bad_alloc();
	}
	FSWFStream(const FSWFStream &o) : m_h(flare_swfs_clone(o.m_h))
	{
		if (!m_h)
			throw std::bad_alloc();
	}
	FSWFStream &operator=(const FSWFStream &o)
	{
		if (this != &o) {
			void *h = flare_swfs_clone(o.m_h);
			if (!h)
				throw std::bad_alloc();
			flare_swfs_free(m_h);
			m_h = h;
		}
		return *this;
	}
	~FSWFStream() { flare_swfs_free(m_h); }

	// The Rust-owned stream, for the other ported writers (FPrimitive.rs, FCT.rs, ...).
	void *Handle() { return m_h; }

	U32 Size() { return flare_swfs_size(m_h); }
	U32 FullSize() { return flare_swfs_full_size(m_h); }

	U8 *Memory() { return flare_swfs_memory(m_h); } // Useful to get the origin of the stream memory.
													// Used by CreateMovie to write the header information.

	void WriteBits(U32 data, U32 size) { check(flare_swfs_write_bits(m_h, data, size)); } // Writes # of bits size, that has value data
	void WriteLargeData(const U8 *data, U32 size)										   // Stores the pointer to a large block of data
	{																					   // to be written. Ownership of the data is NOT passed
		check(flare_swfs_write_large_data(m_h, data, size));							   // to this stream.
	}

	//	NOTE:  These functions all take in U32's even though in some situations a smaller data
	//	type can suffice. This is for the sake of simplicity.
	void WriteDWord(U32 data) { check(flare_swfs_write_dword(m_h, data)); }
	void WriteWord(U32 data) { check(flare_swfs_write_word(m_h, data)); }
	void WriteByte(U32 data) { check(flare_swfs_write_byte(m_h, data)); }

	void FlushBits() { check(flare_swfs_flush_bits(m_h)); } //	Kick out the current partially filled byte to the stream.

	// Copies the stream, adding tag id and length information.
	void AppendTag(U16 _tagID, U32 _length, FSWFStream *_tagBody)
	{
		check(flare_swfs_append_tag(m_h, _tagID, _length, _tagBody ? _tagBody->m_h : 0));
	}
	// Copies another stream to the end of this without any changes. The stream
	// will be empty but still needs to be deleted by the caller.
	void Append(FSWFStream *_SWFStream) { check(flare_swfs_append(m_h, _SWFStream ? _SWFStream->m_h : 0)); }

	// Utility functions
	void WriteToFile(FILE *out) // writes this data to file
	{
		U32 n = FullSize();
		std::vector<U8> buf(n ? n : 1);
		WriteToMemory(buf.data());
		if (n)
			fwrite(buf.data(), 1, n, out);
	}
	void WriteToFileVersion6(FILE *out) // writes this data to file, compressing with zlib(flash 6)
	{
		U32 size = FullSize();
		std::vector<U8> buf(size ? size : 1);
		WriteToMemory(buf.data());

		assert(buf[0] == 'F');
		assert(buf[1] == 'W');
		assert(buf[2] == 'S');
		//assert(buf[3]==4);
		assert(((U32)(buf[4] + (buf[5] << 8) + (buf[6] << 16) + (buf[7] << 24))) == size);

		std::vector<U8> bufOut(size ? size : 1);
		U32 outLen = 0;
		// buf[3] = 6 (version 6), then 'C' + deflate when that is smaller (done in Rust)
		int st = flare_swfs_v6_pack(buf.data(), size, bufOut.data(), size, &outLen);
		if (st > 1)
			throw std::runtime_error("FSWFStream::WriteToFileVersion6 failed");
		if (outLen)
			fwrite(bufOut.data(), 1, outLen, out);
	}
	void WriteToMemory(U8 *memory) // copy this data to a memory buffer (use Size() to make sure it is large enough)
	{
		check(flare_swfs_write_to_memory(m_h, memory, FullSize()));
	}

	//functions used to calculate optimal size for fields
	static U32 MinBits(U32 number, U16 sign) { return flare_swfs_min_bits(number, sign); }
	static U32 MaxNum(S32 a, S32 b, S32 c, S32 d) { return flare_swfs_max_num(a, b, c, d); }

private:
	static void check(int status)
	{
		if (status == 3)
			throw std::length_error("FSWFStream: output buffer too small");
		if (status)
			throw std::invalid_argument("FSWFStream: invalid argument");
	}

	void *m_h; // Rust-owned stream (FSWFStream.rs)
};

#endif
