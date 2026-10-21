// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/09/1999.

/****************************************************************************************

				File Summary: FDTSounds.h

	This header-file contains the declarations of all low-level sound-related classes. 
	Their parent classes are in the parentheses:

		class FDTDefineSound; (public FDT)
		class FDTDefineSoundADPCM; (public FDTDefineSound)
		class FDTDefineSoundWAV; (public FDTDefineSound)
		class FDTSoundStreamBlock; (public FDT)
		class FDTSoundStreamHead; (public FDT)
		class FDTSoundStreamHead2; (public FDT)
		class FSndEnv;
		class FSoundInfo;

****************************************************************************************/

#ifndef _F_DEFINE_SOUNDS_H_
#define _F_DEFINE_SOUNDS_H_

#include "Macromedia.h"
#include "FDT.h"
#include "FSound.h"
#include "FSWFStream.h"

#include <assert.h>
#include <stdexcept>
#include <stdio.h>
#include <string.h>
#include <vector>

// FDTSounds.cpp was ported to FDTSounds.rs: the tag bodies and the sound info / envelope records
// are written there (flare_fdt_sound_*, flare_fsndenv_write, flare_fsoundinfo_write); the classes
// (sample buffers, the FSoundComp compression, the WAV reader on a caller-owned FILE*) stay inline.
extern "C" {
int flare_fdt_define_sound(void *out, unsigned short sound_id, unsigned char format, unsigned char rate, unsigned char size,
						   unsigned char sound_type, unsigned int sample_count, const unsigned char *sound, unsigned int sound_size);
int flare_fdt_sound_stream_block(void *out, const unsigned char *data, unsigned int size);
int flare_fdt_sound_stream_head(void *out, unsigned char mix_format, unsigned char sound_type, unsigned short count);
int flare_fdt_sound_stream_head2(void *out, unsigned char mix_format, unsigned char compression, unsigned char rate,
								 unsigned char size, unsigned char sound_type, unsigned short count);
int flare_fsndenv_write(void *out, unsigned int mark44, unsigned short level0, unsigned short level1);
int flare_fsoundinfo_write(void *out, unsigned char sync_flags, unsigned char has_envelope, unsigned char has_loops,
						   unsigned char has_out_point, unsigned char has_in_point, unsigned int in_point, unsigned int out_point,
						   unsigned short loop_count, unsigned char n_points);
}

inline void flare_fdt_sounds_check(int status)
{
	if (status)
		throw std::invalid_argument("FDTSounds: invalid argument");
}

// A flash object that defines a sound

class FDTDefineSound : public FDT
{
public:
	// Compression Type
	enum {
		NO_COMPRESSION,
		PCM
	};

	// Compression Level
	enum {
		Comp2 = 2,
		Comp3,
		Comp4,
		Comp5
	};

	// Sound Rate
	enum {
		Snd5k,
		Snd11k,
		Snd22k,
		Snd44k
	};

	// Sound Size
	enum {
		Snd8Bit,
		Snd16Bit
	};

	// Sound Type
	enum {
		Snd16Mono,
		Snd16Stereo
	};

	FDTDefineSound()
	{
		soundID = FObjCollection::Increment();
		memset(&soundData, 0, sizeof(soundData));
	}
	virtual ~FDTDefineSound() {}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fdt_sounds_check(flare_fdt_define_sound(_SWFStream->Handle(), soundID, soundData.format, soundData.rate,
													  soundData.size, soundData.type, soundData.sampleCount,
													  soundData.sound, soundData.soundSize));
	}

	U16 ID() { return soundID; }
	int NumSamples() { return (U8)soundData.sampleCount; }
	int Format() { return 4 * (soundData.rate) + (soundData.size) * 2 + (soundData.type); }
	int SoundType() { return soundData.type; }

protected:
	U16 soundID;
	SSound soundData;
	std::vector<U8> pcmData;
};

// A flash object that defines a sound

class FDTDefineSoundADPCM : public FDTDefineSound
{
public:
	FDTDefineSoundADPCM(U8 rate, U8 size, U8 type, U32 sampleCount,
						const U8 *wavData, U32 wavDataSize, int compression)
	{
		// clear the vector to write to
		pcmData.clear();

		soundData.format = 1; //a 1 indicates ADPCM
		soundData.rate = rate;
		soundData.size = size;
		soundData.type = type;
		soundData.sampleCount = sampleCount;

		// Construct a FSound object
		FSound sound;
		sound.format = Format();

		sound.nSamples = (S16)soundData.sampleCount;
		sound.samples = const_cast<U8 *>(wavData); // it won't change it - so we cast (lee)
		sound.dataLen = (S16)wavDataSize;
		sound.delay = 0;

		FSoundComp compress(&sound, compression);

		compress.Compress(const_cast<U8 *>(wavData), sound.nSamples, &pcmData); // it won't change it - so we cast (lee)
		compress.Flush(&pcmData);

		// store the compressed data to the sound structure
		soundData.sound = &pcmData[0];
		soundData.soundSize = pcmData.size();
	}
};

// A flash object that defines a sound
class FDTDefineSoundWAV : public FDTDefineSound
{

public:
	FDTDefineSoundWAV(FILE *fp, int comp)
	{
		U8 *wavData;	 // memory where the WAV file is stored
		int wavDataSize; // the number of bytes in the WAV file

		// clear the vector to write to
		pcmData.clear();

		// 	fp = fopen( waveFile, "rb" );
		if (!fp) {
			return;
		}

		if (loadWavFile(fp, &soundData, &wavData, &wavDataSize) == false) {
			// there was an error
			return;
		}

		// Construct a FSound object
		FSound sound;
		sound.format = Format();

		sound.nSamples = (S16)soundData.sampleCount;
		sound.samples = wavData;
		sound.dataLen = wavDataSize;
		sound.delay = 0;

		FSoundComp compress(&sound, comp);

		compress.Compress(wavData, sound.nSamples, &pcmData);
		compress.Flush(&pcmData);

		// 	fclose( fp );

		// store the compressed data to the sound structure
		soundData.sound = &pcmData[0];
		soundData.soundSize = pcmData.size();

		// delete the wav data
		delete[] wavData;
	}

private:
	// (stays C++: a bare fread/fseek/ftell walk over the caller's FILE*)
	bool loadWavFile(FILE *fp, SSound *soundData, U8 **wavData, int *wavDataSize)
	{
		TUINT32 nextseek;
		TUINT32 filesize;
		TUINT32 fileTag;
		TUINT32 typeTag;
		TUINT32 chunkSize;
		TUINT32 chunkId;
		TUINT32 formatTag = 0;

		assert(fp);
		memset(soundData, 0, sizeof(SSound));

		soundData->format = 1; // ADPCM compressed

		fileTag = read32(fp);
		if (fileTag != 0x46464952) {
			FLASHOUTPUT("Not a RIFF format file\n");
			return false;
		}

		filesize = read32(fp);
		filesize += ftell(fp);

		typeTag = read32(fp);
		if (typeTag != 0x45564157) {
			FLASHOUTPUT("Not a WAVE file\n");
			return false;
		}

		nextseek = ftell(fp);
		while (nextseek < filesize) {
			fseek(fp, (TINT32)nextseek, 0);
			chunkId = read32(fp);
			chunkSize = read32(fp);
			nextseek = chunkSize + ftell(fp);

	#ifndef __LP64__
			FLASHOUTPUT("\n----- Chunk ID %d, Size %d -----\n", chunkId, chunkSize);
	#else
			FLASHOUTPUT("\n----- Chunk ID %d, Size %d -----\n", chunkId, chunkSize);
	#endif

			switch (chunkId) {
			case 0x20746D66: //Format Chunk
			{
				FLASHOUTPUT("Format Chunk\n");
				FLASHOUTPUT("Common Fields:\n");

				formatTag = read16(fp);
	#ifndef __LP64__
				FLASHOUTPUT("  Format Tag %04lXh:  ", (long unsigned int)formatTag);
	#else
				FLASHOUTPUT("  Format Tag %04Xh:  ", formatTag);
	#endif
				switch (formatTag) {
				default:
				case 0x0000:
					FLASHOUTPUT("WAVE_FORMAT_UNKNOWN");
					break;
				case 0x0001:
					FLASHOUTPUT("WAVE_FORMAT_PCM\n");
					break;
				case 0x0002:
					FLASHOUTPUT("WAVE_FORMAT_ADPCM\n");
					break;
				case 0x0005:
					FLASHOUTPUT("WAVE_FORMAT_IBM_CVSD\n");
					break;
				case 0x0006:
					FLASHOUTPUT("WAVE_FORMAT_ALAW\n");
					break;
				case 0x0007:
					FLASHOUTPUT("WAVE_FORMAT_MULAW\n");
					break;
				case 0x0010:
					FLASHOUTPUT("WAVE_FORMAT_OKI_ADPCM");
					break;
				case 0x0011:
					FLASHOUTPUT("WAVE_FORMAT_DVI_ADPCM or WAVE_FORMAT_IMA_ADPCM\n");
					break;
				case 0x0015:
					FLASHOUTPUT("WAVE_FORMAT_DIGISTD\n");
					break;
				case 0x0016:
					FLASHOUTPUT("WAVE_FORMAT_DIGIFIX\n");
					break;
				case 0x0020:
					FLASHOUTPUT("WAVE_FORMAT_YAMAHA_ADPCM\n");
					break;
				case 0x0021:
					FLASHOUTPUT("WAVE_FORMAT_SONARC\n");
					break;
				case 0x0022:
					FLASHOUTPUT("WAVE_FORMAT_DSPGROUP_TRUESPEECH\n");
					break;
				case 0x0023:
					FLASHOUTPUT("WAVE_FORMAT_ECHOSC1\n");
					break;
				case 0x0024:
					FLASHOUTPUT("WAVE_FORMAT_AUDIOFILE_AF18\n");
					break;
				case 0x0101:
					FLASHOUTPUT("IBM_FORMAT_MULAW\n");
					break;
				case 0x0102:
					FLASHOUTPUT("IBM_FORMAT_ALAW\n");
					break;
				case 0x0103:
					FLASHOUTPUT("IBM_FORMAT_ADPCM\n");
					break;
				case 0x0200:
					FLASHOUTPUT("WAVE_FORMAT_CREATIVE_ADPCM\n");
					break;
				}
				int channels = (int)read16(fp);
				soundData->type = channels - 1;
				FLASHOUTPUT("  %u Channels\n", channels);

				int samplesPerSec = (int)read32(fp);
				FLASHOUTPUT("  %u Samples Per Second\n", samplesPerSec);
				switch (samplesPerSec / 5000) {
				case 1: // 5k
					soundData->rate = Snd5k;
					break;
				case 2: // 11k
					soundData->rate = Snd11k;
					break;
				case 4: // 22k
					soundData->rate = Snd22k;
					break;
				case 8: // 44k
					soundData->rate = Snd44k;
					break;
				default:
					FLASHOUTPUT("Sample rate %d unsupported\n", samplesPerSec);
					return false;
				}

				int bytesPerSec = (int)read32(fp);
				FLASHOUTPUT("  %u Average Bytes Per Second\n", bytesPerSec);

				int blockAlign = read16(fp);
				FLASHOUTPUT("  Block Align %u\n", blockAlign);

				FLASHOUTPUT("Format Specific Fields:\n");
				int bits = read16(fp);
				FLASHOUTPUT("  %u Bits Per Sample\n", bits);

				//store the size of the chunks of data (8v16 bit)
				if (bits == 8)
					soundData->size = 0;
				else if (bits == 16)
					soundData->size = 1;
				else
					FLASHASSERT(0);

				if (formatTag != 0x0001) {
					int extra = read16(fp);
					FLASHOUTPUT("  %d Bytes of extra information\n", extra);
					FLASHOUTPUT("  NOT YET SUPPORTED\n");
					return false;
				}
			} break;

			case 0x61746164: //Data Chunk
			{
				FLASHOUTPUT("Data Chunk\n");
				if (!formatTag) {
					FLASHOUTPUT("Warning Format Chunk not defined before Data Chunk\n");
					return false;
				}
				*wavDataSize = (int)chunkSize;
				soundData->sampleCount = chunkSize / (soundData->size + 1);
				*wavData = new U8[chunkSize];
				int read = fread(*wavData, 1, *wavDataSize, fp);

				if (read != (*wavDataSize)) {
					FLASHOUTPUT("Warning Read %d of %d bytes\n", read, (*wavDataSize));
					return false;
				}
				return true;
			}

			default:
				FLASHOUTPUT("Unknown Chunk\n");
				return false;
			}
		}
		return false;
	}
	U32 read32(FILE *fp)
	{
		U32 val;
		fread(&val, 1, 4, fp);
		return val;
	}
	U16 read16(FILE *fp)
	{
		U16 val;
		fread(&val, 1, 2, fp);
		return val;
	}
};

// A Flash object that defines sound data that is interleaved with frame data
//enables sound streaming

class FDTSoundStreamBlock : public FDT
{

public:
	FDTSoundStreamBlock(U32 _size, U8 *_data)
	{
		size = _size;
		data = _data;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fdt_sounds_check(flare_fdt_sound_stream_block(_SWFStream->Handle(), data, size));
	}

private:
	U32 size;
	U8 *data;
};

// A flash object that defines the start of sound data that will be interleaved within flash frames
// This is how sound streaming of networks is possible.
//	Doesn't support compressed sound (flash 2.0)

class FDTSoundStreamHead : public FDT
{

public:
	FDTSoundStreamHead(U8 _mixFormat, // mixer format (?) set to 6
					   U8 _soundType, // 0 mono, 1 stereo
					   U16 _count	 // number of sound samples
					   )
	{
		mixFormat = _mixFormat;
		soundType = _soundType;
		count = _count;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fdt_sounds_check(flare_fdt_sound_stream_head(_SWFStream->Handle(), mixFormat, soundType, count));
	}

private:
	U8 mixFormat;
	U8 soundType;
	U16 count;
};

class FDTSoundStreamHead2 : public FDT
{

public:
	FDTSoundStreamHead2(U8 _mixFormat, U8 _compression, U8 _size, U8 _soundType, U16 _count)
	{
		mixFormat = _mixFormat;
		compression = _compression;
		rate = 0;
		size = _size;
		soundType = _soundType;
		count = _count;
	}
	FDTSoundStreamHead2(U8 _mixFormat, U8 _compression, U8 _rate, U8 _size, U8 _soundType, U16 _count)
	{
		mixFormat = _mixFormat;
		compression = _compression;
		rate = _rate;
		size = _size;
		soundType = _soundType;
		count = _count;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fdt_sounds_check(flare_fdt_sound_stream_head2(_SWFStream->Handle(), mixFormat, compression, rate, size, soundType, count));
	}

private:
	U8 mixFormat;
	U8 compression;
	U8 rate;
	U8 size;
	U8 soundType;
	U16 count;
};

class FSndEnv
{

public:
	// BUG KEPT ON PURPOSE (it is in the original FDTSounds.cpp): the parameters shadow the members,
	// so these three statements assign each parameter to itself and the members are never set. The
	// bytes written by WriteToSWFStream are whatever the object's memory held. "Fixing" this would
	// change the bytes of every file that sets a sound envelope, so it is preserved and documented.
	FSndEnv(U32 mark44, U16 level0, U16 level1)
	{
		mark44 = mark44;
		level0 = level0;
		level1 = level1;
	}
	void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fdt_sounds_check(flare_fsndenv_write(_SWFStream->Handle(), this->mark44, this->level0, this->level1));
	}

private:
	U32 mark44;
	U16 level0;
	U16 level1;
};

// specifies how a sound character is player

class FSoundInfo
{

public:
	FSoundInfo(U8 _syncFlags, U8 _hasEnvelope, U8 _hasLoops, U8 _hasOutPoint,
			   U8 _hasInPoint, U32 _inPoint, U32 _outPoint, U16 _loopCount,
			   U8 _nPoints, FSndEnv *_soundEnvelope)
	{

		syncFlags = _syncFlags;
		hasEnvelope = _hasEnvelope;
		hasLoops = _hasLoops;
		hasOutPoint = _hasOutPoint;
		hasInPoint = _hasInPoint;
		inPoint = _inPoint;
		outPoint = _outPoint;
		loopCount = _loopCount;
		nPoints = _nPoints;
		soundEnvelope = _soundEnvelope;
	}
	~FSoundInfo()
	{

		delete soundEnvelope;
	}
	void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fdt_sounds_check(flare_fsoundinfo_write(_SWFStream->Handle(), syncFlags, hasEnvelope, hasLoops, hasOutPoint,
													  hasInPoint, inPoint, outPoint, loopCount, nPoints));
		// the envelope follows nPoints (a NULL envelope with hasEnvelope set crashed in the original too)
		if (hasEnvelope)
			soundEnvelope->WriteToSWFStream(_SWFStream);
	}

	enum {
		SyncNoMultiple = 0x01,
		SyncStor = 0x02
	};

private:
	U8 syncFlags;
	U8 hasEnvelope;
	U8 hasLoops;
	U8 hasOutPoint;
	U8 hasInPoint;
	U32 inPoint;
	U32 outPoint;
	U16 loopCount;
	U8 nPoints;
	FSndEnv *soundEnvelope;
};

#endif
