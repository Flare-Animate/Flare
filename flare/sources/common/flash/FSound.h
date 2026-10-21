#ifndef SNDMIX_INCLUDED
#define SNDMIX_INCLUDED

// #include <windows.h>
// #include <mmreg.h>
#include <assert.h>
#include <stdio.h>
#include <stddef.h>
#include <stdexcept>
#include <vector>

#include "Macromedia.h"
#include "FFixed.h"

struct WaveFormat {
	U16 wFormatTag;		 /* format type */
	U16 nChannels;		 /* number of channels (i.e. mono, stereo...) */
	U32 nSamplesPerSec;  /* sample rate */
	U32 nAvgBytesPerSec; /* for buffer estimation */
	U16 nBlockAlign;	 /* block size of data */
	U16 wBitsPerSample;  /* number of bits per sample of mono data */
	U16 cbSize;			 /* the count in bytes of the size of */
};

// Our supported sample rates
// Note that sndRate5K is really 5512.5khz
// Use defines instead of an enum since these must be 32 bits on all platforms
#define sndRate5K 5512L
#define sndRate11K 11025L
#define sndRate22K 22050L
#define sndRate44K 44100L

const int sndMono = 0x0;
const int sndStereo = 0x1;

const int snd8Bit = 0x0;
const int snd16Bit = 0x2;

const int snd5K = 0 << 2;
const int snd11K = 1 << 2;
const int snd22K = 2 << 2;
const int snd44K = 3 << 2;

const int sndCompressNone = 0x00; // we could add 14 more compression types here...
const int sndCompressADPCM = 0x10;
const int sndCompressMP3 = 0x20;
const int sndCompressNoneI = 0x30; // save out in intel byte order
const int sndRateMask = 0x3 << 2;
const int sndCompressMask = 0xF0;

enum { // Sound format types
	snd5K8Mono = 0,
	snd5K8Stereo,
	snd5K16Mono,
	snd5K16Stereo,
	snd11K8Mono,
	snd11K8Stereo,
	snd11K16Mono,
	snd11K16Stereo,
	snd22K8Mono,
	snd22K8Stereo,
	snd22K16Mono,
	snd22K16Stereo,
	snd44K8Mono,
	snd44K8Stereo,
	snd44K16Mono,
	snd44K16Stereo
};

// This object defines a sound sample
class FSound
{
public:
	int format;
	S32 nSamples;  // the number of samples - duration = nSamples/Rate()
	void *samples; // this should probably be a handle on Mac
	S32 dataLen;   // length in bytes of samples - set only if needed
	S32 delay;	 // MP3 compression has a delay before the real sound data

	static constexpr S32 kRateTable[4] = {sndRate5K, sndRate11K, sndRate22K, sndRate44K};
	static constexpr int kRateShiftTable[4] = {3, 2, 1, 0};

public:
	void Init()
	{
		format = 0;
		nSamples = 0;
		samples = 0;
		dataLen = 0;
		delay = 0;
	}

	S32 Rate() { return kRateTable[(format >> 2) & 0x3]; }
	int RateShift() { return kRateShiftTable[(format >> 2) & 0x3]; }
	bool Stereo() { return (format & sndStereo) != 0; }
	int NChannels() { return (format & sndStereo) ? 2 : 1; }
	bool Is8Bit() { return (format & snd16Bit) == 0; }
	int BitsPerSample() { return (format & snd16Bit) ? 16 : 8; }
	int BytesPerSample() { return (format & snd16Bit) ? 2 : 1; }
	int CompressFormat() { return format & sndCompressMask; }
	bool Compressed() { return (format & sndCompressMask) != 0; }

	// Manage the duration in 44kHz units
	S32 GetDuration44() { return nSamples << RateShift(); }
	void SetDuration44(S32 d) { nSamples = d >> RateShift(); }

	int BytesPerBlock() { return BytesPerSample() * NChannels(); }
	S32 SizeBytes() { return nSamples * BytesPerBlock(); }

	void Set(WaveFormat *wfmt)
	{
		wfmt->wFormatTag = 1; //WAVE_FORMAT_PCM;
		wfmt->nSamplesPerSec = Rate();
		wfmt->nChannels = NChannels();
		wfmt->wBitsPerSample = BitsPerSample();
		wfmt->nBlockAlign = (wfmt->wBitsPerSample * wfmt->nChannels) / 8;
		wfmt->nAvgBytesPerSec = wfmt->nBlockAlign * wfmt->nSamplesPerSec;

		//wfmt->cbSize = 0;
	}
};

// FSound.cpp was ported to FSound.rs: the ADPCM compressor kernels take the plain state struct
// below and append to a scratch buffer that the shell copies onto the caller's std::vector.
extern "C" {
struct FlareSoundCompState {
	int isStereo;
	int is8Bit;
	int nBits; // number of bits in each sample
	int nSamples; // samples compressed so far
	int valpred[2]; // ADPCM state
	int index[2];
	int len; // default is to just calculate the size
	unsigned int bitBuf; // State for writing bits
	int bitPos;
};
size_t flare_soundcomp_max_out(const FlareSoundCompState *st, int n);
int flare_soundcomp_compress(FlareSoundCompState *st, const unsigned char *src, int n, unsigned char *out, size_t out_cap, size_t *out_len);
int flare_soundcomp_flush(FlareSoundCompState *st, unsigned char *out, size_t out_cap, size_t *out_len);
}

class FSoundComp
{
private:
	FlareSoundCompState s;

public:
	FSoundComp(FSound *snd, S32 nb) // numberBits from 2-5
	{
		// 	assert(snd->CompressFormat() == sndCompressADPCM);
		s.isStereo = snd->Stereo();
		s.is8Bit = snd->Is8Bit();

		s.nBits = nb;
		s.nSamples = 0;

		s.valpred[0] = s.valpred[1] = 0; // the original left these uninitialised
		s.index[0] = s.index[1] = 0;

		s.len = 0;

		s.bitBuf = 0;
		s.bitPos = 0;
	}
	~FSoundComp() { assert(s.bitPos == 0); }

	void Compress(void *src, S32 n, std::vector<U8> *stream)
	{
		if (!stream) { // Just counting...
			check(flare_soundcomp_compress(&s, (const unsigned char *)src, n, 0, 0, 0));
			return;
		}
		size_t cap = flare_soundcomp_max_out(&s, n), len = 0;
		std::vector<U8> tmp(cap ? cap : 1);
		check(flare_soundcomp_compress(&s, (const unsigned char *)src, n, tmp.data(), cap, &len));
		stream->insert(stream->end(), tmp.begin(), tmp.begin() + len);
	}
	void Flush(std::vector<U8> *stream)
	{
		if (!stream) {
			check(flare_soundcomp_flush(&s, 0, 0, 0));
			return;
		}
		size_t cap = flare_soundcomp_max_out(&s, 0), len = 0;
		std::vector<U8> tmp(cap ? cap : 1);
		check(flare_soundcomp_flush(&s, tmp.data(), cap, &len));
		stream->insert(stream->end(), tmp.begin(), tmp.begin() + len);
	}

private:
	static void check(int status)
	{
		if (status)
			throw std::invalid_argument("FSoundComp: invalid argument (numberBits must be 2..5)");
	}
};

#endif
