// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/09/1999.

/****************************************************************************************

				File Summary: FDTSprite.h

	This header-file contains the declarations of low-level sprite-related class.
	Its parent class is in the parentheses:

		class FDTSprite; (public FDT)

****************************************************************************************/

#ifndef _SPRITE_H_
#define _SPRITE_H_

#include <list>
#include <stdexcept>
#include "Macromedia.h"
#include "FDT.h"
#include "FSWFStream.h"

// FDTSprite.cpp was ported to FDTSprite.rs: the DefineSprite tag layout is written there; the
// class (the owned FObj list, character ID, frame count) stays inline.
extern "C" {
int flare_fdt_sprite_head(void *body, unsigned short character_id, unsigned int num_of_frames);
int flare_fdt_sprite_finish(void *out, void *body);
}

//! Defines a low-level sprite object.
/*! A sprite is a flash object that acts as a "movie within a movie".
	\sa FDT
*/
class FDTSprite : public FDT
{
public:
	//! Construct a low-level sprite object.
	/*! */
	FDTSprite()
	{
		characterID = FObjCollection::Increment();
		numOfFrames = 0;
	}

	//! Destruct a low-level sprite object.
	/*! */
	virtual ~FDTSprite()
	{
		while (!objectList.empty()) {

			delete objectList.front();
			objectList.pop_front();
		}
	}

	// Method for internal use.
	void AddFObj(FObj *_object)
	{
		if (_object->IsShowFrame())
			numOfFrames++;

		objectList.push_back(_object);
	}

	// Method for internal use.
	U16 ID() { return characterID; }

	// Method for internal use.
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		FSWFStream body;

		if (flare_fdt_sprite_head(body.Handle(), characterID, numOfFrames))
			throw std::invalid_argument("FDTSprite: invalid argument");

		std::list<FObj *>::iterator cursor;
		for (cursor = objectList.begin(); cursor != objectList.end(); cursor++) {

			(*cursor)->WriteToSWFStream(&body);
		}

		// the end tag, and the temporary buffer into the main stream
		if (flare_fdt_sprite_finish(_SWFStream->Handle(), body.Handle()))
			throw std::invalid_argument("FDTSprite: invalid argument");
	}

private:
	U16 characterID;
	std::list<FObj *> objectList;
	U32 numOfFrames;
};

#endif
