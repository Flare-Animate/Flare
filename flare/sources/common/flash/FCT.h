// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/09/1999.

/****************************************************************************************

				File Summary: FCT.h

  This header-file contains the declarations of the low-level Control-Tag related classes:
  
	 1)	class FCT;
	 2)	all the derived classes of class FCT:
  
		class FCTDoAction;
		class FCTFrameLabel;
		class FCTPlaceObject;
		class FCTPlaceObject2;
		class FCTProtect;
		class FCTRemoveObject;
		class FCTRemoveObject2;
		class FCTSetBackgroundColor;
		class FCTShowFrame;
		class FCTStartSound;


****************************************************************************************/

#ifdef WIN32 // added from DV
#pragma warning(push)
#pragma warning(disable : 4786)
#pragma warning(disable : 4251)

#endif

#include "tcommon.h"

#undef DVAPI
#undef DVVAR
#ifdef TNZCORE_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif
#ifndef _F_C_T_H_
#define _F_C_T_H_

#include <string.h>
#include <stdexcept>

#include "Macromedia.h"
#include "FObj.h"
#include "FAction.h"
#include "FPrimitive.h"
#include "FSWFStream.h"
#include "FDTShapes.h"
#include "FDTSounds.h"

// FCT.cpp was ported to FCT.rs: the tags that are plain data and the fixed heads/trailers of
// the others are written there (flare_fct_*); the classes, which own and call into virtual
// C++ sub-objects (FMatrix/FCXForm/FClipAction/FSoundInfo/FActionRecord), stay inline here.
extern "C" {
int flare_fct_protect(void *out);
int flare_fct_show_frame(void *out);
int flare_fct_remove_object(void *out, unsigned short character_id, unsigned short depth);
int flare_fct_remove_object2(void *out, unsigned short depth);
int flare_fct_frame_label(void *out, const unsigned char *text, size_t len);
int flare_fct_set_background_color(void *out, unsigned int red, unsigned int green, unsigned int blue, unsigned int alpha, int alpha_t);
int flare_fct_unparsed_tag(void *out, unsigned short tag_id, const unsigned char *data, unsigned int length);
int flare_fct_place_object_head(void *body, unsigned short character_id, unsigned short depth);
int flare_fct_place_object2_head(void *body, unsigned int has_clip_action, unsigned int has_clip_depth, unsigned int has_name,
								 unsigned int has_ratio, unsigned int has_color_transform, unsigned int has_matrix,
								 unsigned int has_char_id, unsigned int has_move, unsigned int depth);
int flare_fct_start_sound_head(void *body, unsigned short sound_id);
int flare_fct_finish_tag(void *out, unsigned short tag_id, void *body, int trailing_zero_byte, int flush_bits);
}

inline void flare_fct_check(int status)
{
	if (status)
		throw std::invalid_argument("FCT: invalid argument");
}

class FCXForm;
class FCXFormWAlpha;

class FSoundInfo;

// A "control tag" type of flash object

class DVAPI FCT : public FObj
{
public:
	virtual ~FCT() {}
	virtual void WriteToSWFStream(FSWFStream * /*_SWFStream*/) {}
};

// flash object that directs the flash player to complete a given set of actions at frame completion

class DVAPI FCTDoAction : public FCT
{
public:
	FCTDoAction() {}
	virtual ~FCTDoAction()
	{
		while (!actionRecordList.empty()) {
			delete actionRecordList.front();

			actionRecordList.pop_front();
		}
	}

	void AddAction(FActionRecord *_actionRecord) { actionRecordList.push_back(_actionRecord); }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		FSWFStream body;

		std::list<FActionRecord *>::iterator cursor;
		for (cursor = actionRecordList.begin(); cursor != actionRecordList.end(); cursor++) {
			(*cursor)->WriteToSWFStream(&body);
		}

		// the terminating zero byte and the tag are written in Rust
		flare_fct_check(flare_fct_finish_tag(_SWFStream->Handle(), stagDoAction, body.Handle(), 1, 0));
	}

private:
	std::list<FActionRecord *> actionRecordList;
};

//  Associates a label with the frame.  This label can then be used
//	in the Go to Label Action.

class DVAPI FCTFrameLabel : public FCT
{

public:
	FCTFrameLabel(FString *_frameName) { frameName = _frameName; }
	virtual ~FCTFrameLabel() { delete frameName; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		std::string name = frameName->GetString();
		flare_fct_check(flare_fct_frame_label(_SWFStream->Handle(), (const unsigned char *)name.data(), name.length()));
	}

private:
	FString *frameName;
};

// a flash object that directs the player to add an object to the display list (flash 1.0)

class DVAPI FCTPlaceObject : public FCT
{
public:
	// if no color transform then _colorTransform argument must be null
	FCTPlaceObject(U16 _characterID,
				   U16 _depth,
				   FMatrix *_matrix,		  // always present
				   FCXForm *_colorTransform) // NULL if there is not a color transform
	{
		characterID = _characterID;
		depth = _depth;
		matrix = _matrix;
		colorTransform = _colorTransform;

		FLASHASSERT(_colorTransform);
	}
	virtual ~FCTPlaceObject()
	{
		delete matrix;
		delete colorTransform;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		FSWFStream body;

		flare_fct_check(flare_fct_place_object_head(body.Handle(), characterID, depth));

		matrix->WriteToSWFStream(&body);

		if (colorTransform) {
			colorTransform->WriteToSWFStream(&body);
		}

		flare_fct_check(flare_fct_finish_tag(_SWFStream->Handle(), stagPlaceObject, body.Handle(), 0, 0));
	}

private:
	U16 characterID;
	U16 depth;
	FMatrix *matrix;
	FCXForm *colorTransform;
};

class DVAPI FCTPlaceObject2 : public FCT
{
public:
	// if a certain flag is not true, you must provide a NULL value for its associated argument(s)
	FCTPlaceObject2(U16 _hasClipDepth, /*DM*/
					U16 _hasRatio,
					U16 _hasCharID,
					U16 _hasMove,
					U16 _depth,
					U16 _characterID,
					FMatrix *_matrix,				// NULL if the object does not have a matrix
					FCXFormWAlpha *_colorTransform, // NULL if there is no color transform
					U16 _ratio,
					FString *_name,  // NULL if there is no name
					U16 _clipDepth) /*DM*/
	{
		SetParameters(_hasClipDepth, _hasRatio, _hasCharID, _hasMove, _depth, _characterID,
					  _matrix, _colorTransform, _ratio, _name, _clipDepth);
	}

	//now is possible to give a clip action
	// if a certain flag is not true, you must provide a NULL value for its associated argument(s)
	FCTPlaceObject2(U16 _hasClipAction,
					U16 _hasClipDepth, /*DM*/
					U16 _hasRatio,
					U16 _hasCharID,
					U16 _hasMove,
					U16 _depth,
					U16 _characterID,
					FMatrix *_matrix,				// NULL if the object does not have a matrix
					FCXFormWAlpha *_colorTransform, // NULL if there is no color transform
					U16 _ratio,
					FString *_name, // NULL if there is no name
					U16 _clipDepth,
					FClipAction *_clipAction) /*DM*/
	{
		hasClipAction = _hasClipAction;
		hasClipDepth = _hasClipDepth;
		hasRatio = _hasRatio;
		hasCharID = _hasCharID;
		hasMove = _hasMove;
		depth = _depth;
		characterID = _characterID;
		matrix = _matrix;
		colorTransform = _colorTransform;
		ratio = _ratio;
		name = _name;
		clipDepth = _clipDepth;
		clipAction = _clipAction;
	}

	FCTPlaceObject2(const FCTPlaceObject2 &obj)
	{
		SetParameters(obj.hasClipDepth, obj.hasRatio, obj.hasCharID, obj.hasMove,
					  obj.depth, obj.characterID, 0, obj.colorTransform,
					  obj.ratio, 0, obj.clipDepth);
		matrix = obj.matrix ? new FMatrix(*(obj.matrix)) : 0;
		name = obj.name ? new FString(*(obj.name)) : 0;
	}

	void SetParameters(U16 _hasClipDepth, /*DM*/
					   U16 _hasRatio,
					   U16 _hasCharID,
					   U16 _hasMove,
					   U16 _depth,
					   U16 _characterID,
					   FMatrix *_matrix,			   // NULL if the object does not have a matrix
					   FCXFormWAlpha *_colorTransform, // NULL if there is no color transform
					   U16 _ratio,
					   FString *_name,  // NULL if there is no name
					   U16 _clipDepth) /*DM*/
	{
		hasClipAction = 0;
		hasClipDepth = _hasClipDepth;
		hasRatio = _hasRatio;
		hasCharID = _hasCharID;
		hasMove = _hasMove;
		depth = _depth;
		characterID = _characterID;
		matrix = _matrix;
		colorTransform = _colorTransform;
		ratio = _ratio;
		name = _name;
		clipDepth = _clipDepth;
		clipAction = NULL;
	}

	// Used to fix the high level manager depth sorting problem.
	int GetDepth() { return depth; }
	void SetDepth(int d) { depth = d; }

	virtual ~FCTPlaceObject2()
	{
		delete name;
		delete matrix;
		delete colorTransform;
		delete clipAction;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		FSWFStream body;

		flare_fct_check(flare_fct_place_object2_head(body.Handle(), hasClipAction, hasClipDepth, name != 0, hasRatio,
													 colorTransform != 0, matrix != 0, hasCharID, hasMove, depth));

		if (hasCharID)
			body.WriteWord((U32)characterID);
		if (matrix)
			matrix->WriteToSWFStream(&body);
		if (colorTransform)
			colorTransform->WriteToSWFStream(&body);
		if (hasRatio)
			body.WriteWord((U32)ratio);
		if (name)
			name->WriteToSWFStream(&body, true);
		if (hasClipDepth)					/*DM*/
			body.WriteWord((U32)clipDepth); /*DM*/
		if (hasClipAction)
			clipAction->WriteToSWFStream(&body);

		// FlushBits() of the body, then the tag
		flare_fct_check(flare_fct_finish_tag(_SWFStream->Handle(), stagPlaceObject2, body.Handle(), 0, 1));
	}
	U32 IsPlaceObject() { return 1; }
	int GetPlacedId() { return hasCharID ? characterID : -1; }
	std::string GetName() { return name ? name->GetString() : std::string(); }
	void SetId(U16 id)
	{
		characterID = id;
		hasMove = 0;
		hasCharID = 1;
	}
	void ChangePlacedId(U16 id)
	{
		if (hasCharID) {
			characterID = id;
			//hasMove=0;
		}
	}

	void SetMatrix(FMatrix *_matrix) { matrix = _matrix; }
	void ApplyMatrix(const FMatrix &_matrix)
	{
		if (matrix)
			(*matrix) = (*matrix) * _matrix;
		else
			matrix = new FMatrix(_matrix);
	}

private:
	//flags
	U16 hasClipAction;
	U16 hasClipDepth; /*DM*/
	U16 hasRatio;
	U16 hasCharID;
	U16 hasMove;

	U16 depth;
	U16 characterID;
	FMatrix *matrix;
	FCXFormWAlpha *colorTransform;
	U16 ratio;
	FString *name;
	U16 clipDepth; /*DM*/
	FClipAction *clipAction;
};

// a flash control tag object which marks a SWF movie as uneditable

class DVAPI FCTProtect : public FCT
{
public:
	FCTProtect() {}

	virtual void WriteToSWFStream(FSWFStream *_SWFStream) { flare_fct_check(flare_fct_protect(_SWFStream->Handle())); }
};

// a Flash control tag object which indicates to the flash player to remove an object from the display list (flash 1.0)

class DVAPI FCTRemoveObject : public FCT
{

public:
	FCTRemoveObject(U16 _characterID, U16 _depth)
	{
		characterID = _characterID;
		depth = _depth;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fct_check(flare_fct_remove_object(_SWFStream->Handle(), characterID, depth));
	}

	// Used to fix the high level manager depth sorting problem.
	int GetDepth() { return depth; }
	void SetDepth(int d) { depth = d; }

private:
	U16 characterID;
	U16 depth;
};

// a Flash control tag object which indicates to the flash player to remove an object from the display list (flash 1.0)

class DVAPI FCTRemoveObject2 : public FCT
{

public:
	FCTRemoveObject2(U16 _depth) { depth = _depth; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fct_check(flare_fct_remove_object2(_SWFStream->Handle(), depth));
	}

private:
	U16 depth;
};

// A flash object which sets a movie's background color

class DVAPI FCTSetBackgroundColor : public FCT
{
public:
	FCTSetBackgroundColor(FColor *_color)
	{
		color = _color;
		// here changed.
		// Background color is always opaque, i.e. no alpha channel.
		color->AlphaChannel(false);
	}
	virtual ~FCTSetBackgroundColor() { delete (color); }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fct_check(flare_fct_set_background_color(_SWFStream->Handle(), color->Red(), color->Green(), color->Blue(),
													   color->Alpha(), color->HasAlpha() ? 1 : 0));
	}

private:
	FColor *color;
};

class DVAPI FCTUnparsedTag : public FCT
{
public:
	FCTUnparsedTag(U16 _tagId, U32 _lenght, U8 *_data)
	{
		lenght = _lenght;
		if (lenght > 0) {
			data = new U8[lenght];
			memcpy(data, _data, lenght);
		} else
			data = 0;

		tagId = _tagId;
	}
	virtual ~FCTUnparsedTag() { delete[] data; } // the original used `delete` on the new[] block
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fct_check(flare_fct_unparsed_tag(_SWFStream->Handle(), tagId, data, lenght));
	}
	USHORT GetID()
	{
		if (tagId == stagDefineButton2 ||
			tagId == stagDefineButton ||
			tagId == stagDefineText ||
			tagId == stagDefineText2 ||
			tagId == stagDefineEditText ||
			tagId == stagDefineMorphShape ||
			tagId == stagDefineBitsLossless2 ||
			tagId == stagDefineBitsLossless ||
			tagId == stagDefineBitsJPEG3)
			return (USHORT)(data[0] | (data[1] << 8));
		return 0;
	}
	void SetID(USHORT id)
	{
		if (tagId == stagDefineButton2 ||
			tagId == stagDefineButton ||
			tagId == stagDefineText ||
			tagId == stagDefineText2 ||
			tagId == stagDefineEditText ||
			tagId == stagDefineMorphShape ||
			tagId == stagDefineBitsLossless2 ||
			tagId == stagDefineBitsLossless ||
			tagId == stagDefineBitsJPEG3) {
			data[0] = (id & 0xff);
			data[1] = (id >> 8);
		}
	}

private:
	U8 *data;
	U32 lenght;
	U16 tagId;
};

//a control tag that indicates end of the current frame

class DVAPI FCTShowFrame : public FCT
{

public:
	FCTShowFrame() {}
	U32 IsShowFrame() { return 1; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream) { flare_fct_check(flare_fct_show_frame(_SWFStream->Handle())); }
};

// A flash object that instructs flash player to start a sound

class DVAPI FCTStartSound : public FCT
{
public:
	FCTStartSound(U16 _soundID, FSoundInfo *_soundInfo)
	{
		soundID = _soundID;
		soundInfo = _soundInfo;
	}
	virtual ~FCTStartSound() { delete soundInfo; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		FSWFStream body;

		flare_fct_check(flare_fct_start_sound_head(body.Handle(), soundID));
		soundInfo->WriteToSWFStream(&body);

		flare_fct_check(flare_fct_finish_tag(_SWFStream->Handle(), stagStartSound, body.Handle(), 0, 0));
	}

private:
	U16 soundID;
	FSoundInfo *soundInfo;
};

#ifdef WIN32 // added from DV
#pragma warning(pop)
#endif

#endif
