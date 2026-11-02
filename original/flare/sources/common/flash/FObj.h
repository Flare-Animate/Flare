// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/09/1999.
// Last Modified On 18/06/2002  by DV for Fixes.
/****************************************************************************************

				File Summary: FObj.h

	This header-file contains the declarations of low-level FObj-related classes.
	All parent classes are in the parentheses:

		class FObj;
		class FFragment; (public FObj)
		class FObjCollection;

****************************************************************************************/

#ifndef _FOBJ_H_
#define _FOBJ_H_

#include "tcommon.h"
#include <assert.h>
#include <list>

#ifdef WIN32 // added from DV
#pragma warning(push)
#pragma warning(disable : 4786)
#pragma warning(disable : 4251)

#endif

#include "Macromedia.h"
#include "FPrimitive.h"
#include "FSWFStream.h"
#include <stdio.h>

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

//All Flash tagged data block objects fall into this category

class DVAPI FObj
{
public:
	virtual ~FObj() {} // Doesn't do anything, just makes all the other destructors virtual

	virtual void WriteToSWFStream(FSWFStream *_SWFStream) = 0;
	virtual U32 IsShowFrame() { return false; }
	virtual U32 IsPlaceObject() { return false; }
	virtual bool IsDefineShape() { return false; }
	virtual bool IsSprite() { return false; }

	virtual void SetId(U16 id) { FLASHASSERT(0); }
	virtual void SetMatrix(FMatrix *matrix) { FLASHASSERT(0); }
};

// FObj.cpp was ported to FObj.rs (the 21 byte file header and the character-ID counter);
// the collection of FObj pointers and the virtual-call orchestration stay here, inline.
extern "C" {
int flare_fobj_file_header(unsigned char *target, size_t target_len, unsigned int file_length_no_header,
						   int xmin, int xmax, int ymin, int ymax, unsigned int frame_rate,
						   unsigned char version, unsigned int num_frames);
unsigned short flare_fobj_next_character_id();
void flare_fobj_reset_character_id();
}

/*! A class for writing an arbitrary block of data (a SWF fragment, perhaps) to
 *	the SWFStream. The data is assumed to be large, so it is not changed.
 */
class FFragment : public FObj
{
public:
	FFragment(const void *_data, int _size)
	{
		data = _data;
		size = _size;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		_SWFStream->WriteLargeData((U8 *)data, size);
	}

private:
	const void *data;
	int size;
};

//	Holds a collection of FObj's so that they can be dumped in a SWF format
//Writes tags in the same order as they appear in the collection

//using namespace std;

class DVAPI FObjCollection
{

public:
	//	When constructor is called, the empty FObjList is automatically created.
	FObjCollection(void) { numOfFrames = 0; }

	//	Removes and deletes every element in the list.
	~FObjCollection(void)
	{
		while (!FObjList.empty()) {
			delete FObjList.front();
			FObjList.pop_front();
		}
	}

	//	The given FObj is added to the end of FObjList
	void AddFObj(FObj *fobj)
	{
		if (fobj->IsShowFrame()) //increment numFrames then store the tag
			numOfFrames++;

		FObjList.push_back(fobj);
	}

	void InsertFObj(int beforeTag, FObj *fobj)
	{
		if (fobj->IsShowFrame()) //increment numFrames then store the tag
			numOfFrames++;

		std::list<FObj *>::iterator it = FObjList.begin();
		std::advance(it, beforeTag);
		FObjList.insert(it, fobj);
	}

	void EraseFObj(int tagindex)
	{
		std::list<FObj *>::iterator it = FObjList.begin();
		std::advance(it, tagindex);
		FObjList.erase(it);
	}

	//	Uses a cursor to loop through the entire list and write all of the
	//  list's FObjs to the given SWFStream
	void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		std::list<FObj *>::iterator cursor;

		for (cursor = FObjList.begin(); cursor != FObjList.end(); cursor++) {
			(*cursor)->WriteToSWFStream(_SWFStream);
		}
	}

	//void CreateCompressedMovie( const TFilePath &_fileName, FRect cameraBox, U32 _frameRate = 12 );
	//void CreateMovie( const TFilePath &_fileName, FRect cameraBox, U32 _frameRate = 12, U8 _version = 6);
	void CreateCompressedMovie(FILE *fp, FRect cameraBox, U32 _frameRate = 12)
	{
		if (fp) {
			FSWFStream stream;

			CreateMovie(&stream, cameraBox, _frameRate);
			stream.WriteToFileVersion6(fp);
		}
	}

	void CreateMovie(FILE *fp, FRect cameraBox, U32 _frameRate = 12, U8 _version = 6)
	{
		if (fp) {
			FSWFStream stream;

			CreateMovie(&stream, cameraBox, _frameRate, _version);
			stream.WriteToFile(fp);
		}
	}

	// creates a Flash Movie
	void CreateMovie(FSWFStream *swfStream, FRect cameraBox, U32 _frameRate = 12, U8 _version = 6)
	{
		// Carve out space for the header -- always 21 bytes
		for (int i = 0; i < HEADER_SIZE; i++)
			swfStream->WriteByte(0);

		// This FObjCollection is dumped into swfFileBody
		WriteToSWFStream(swfStream);

		//swfFileBody Terminated with a FCTEnd tag
		WriteEndTag(swfStream);

		// write file header into file
		WriteFileHeader(swfStream->Memory(), swfStream->Size(), cameraBox, _frameRate, _version);

		flare_fobj_reset_character_id();
	}

	// creates a Flash Sprite
	void CreateSprite(FSWFStream *swfStream, FRect cameraBox, U32 /*_frameRate*/ = 12)
	{
		// Sprites do not have headers.

		// This FObjCollection is dumped into swfFileBody
		WriteToSWFStream(swfStream);

		//swfFileBody Terminated with a FCTEnd tag
		WriteEndTag(swfStream);

		flare_fobj_reset_character_id();
	}

	// increments the number of flash objects created
	static U16 Increment(void)
	{
		U16 id = flare_fobj_next_character_id();
		assert(id != 0); // the counter wrapped: characterID was 0xffff
		return id;
	}

	int GetFObjCount() const { return FObjList.size(); }

	FObj *GetFObj(int i) const
	{
		if (i >= (int)FObjList.size())
			return 0;
		std::list<FObj *>::const_iterator it = FObjList.begin();
		std::advance(it, i);

		return *it;
	}

private:
	enum {
		HEADER_SIZE = 21
	};

	U32 numOfFrames;
	std::list<FObj *> FObjList;

	// This works differently than the other calls, because it hacks the header information back
	// into the beginning of an existing SWF stream.
	void WriteFileHeader(U8 *target, U32 _fileLengthNoHeader, FRect cameraBox,
						 U32 _frameRate, U8 _version)
	{
		flare_fobj_file_header(target, HEADER_SIZE, _fileLengthNoHeader, cameraBox.Xmin(), cameraBox.Xmax(),
							   cameraBox.Ymin(), cameraBox.Ymax(), _frameRate, _version, numOfFrames);
	}

	void WriteEndTag(FSWFStream *_SWFStream) { _SWFStream->AppendTag(stagEnd, 0, 0); }
};

#ifdef WIN32 // added from DV
#pragma warning(pop)
#endif

#endif
