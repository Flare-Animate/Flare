// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/09/1999.

/****************************************************************************************

				File Summary: FDTButtons.h

	This header-file contains the declarations of all low-level button-related classes. 
	Their parent classes are in the parentheses:

		class FAButtonRecord;
		class FButtonRecord1; (public FAButtonRecord)
		class FButtonRecord2; (public FAButtonRecord)
		class FButtonRecordList;
		class FDTDefineButton; (public FDT)
		class FDTDefineButton2; (public FDT)
		class FDTDefineButtonCXForm; (public FDT)
		class FDTDefineButtonSound. (public FDT)

****************************************************************************************/

#ifndef _F_DTBUTTONS_H_
#define _F_DTBUTTONS_H_

#include "FDT.h"

#ifdef WIN32 // added from DV
#pragma warning(push)
#pragma warning(disable : 4786)
#pragma warning(disable : 4251)

#endif

#include <list>

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

#include "FPrimitive.h"
#include "FAction.h"
#include "FDTShapes.h"
#include "FDTSounds.h"

// FDTButtons.cpp was ported to FDTButtons.rs: the record heads, the DefineButton2 record block
// (terminator, action offset) and the tag selection are written there; the classes (matrix /
// colour transform / action / sound objects and their virtual writers, character IDs) stay
// inline.
extern "C" {
int flare_fdt_button_record1_head(void *h, unsigned int hit, unsigned int down, unsigned int over, unsigned int up,
								  unsigned int character_id, unsigned int layer);
int flare_fdt_button_record2_head(void *h, unsigned int hit, unsigned int down, unsigned int over, unsigned int up,
								  unsigned int character_id, unsigned int layer);
int flare_fdt_button_head(void *body, unsigned int id, int menu_flag);
int flare_fdt_button2_records(void *body, void *records, int has_conditions);
int flare_fdt_button_finish(void *out, int kind, void *body);
}
enum { FLARE_BUTTON_KIND_BUTTON = 1, FLARE_BUTTON_KIND_BUTTON2 = 2, FLARE_BUTTON_KIND_CXFORM = 3, FLARE_BUTTON_KIND_SOUND = 4 };

// Specifies appearance aspects for a button definition

class DVAPI FAButtonRecord
{

public:
	virtual void WriteToSWFStream(FSWFStream *_SWFStream) = 0;

	virtual ~FAButtonRecord() {}
};

// Specifies appearance aspects for a button definition (flash 1.0)

class DVAPI FButtonRecord1 : public FAButtonRecord
{

public:
	FButtonRecord1(U8 _hit, U8 _down, U8 _over, U8 _up, U16 _layer, FMatrix *_matrix)
	{
		hit = _hit;
		down = _down;
		over = _over;
		up = _up;
		layer = _layer;
		matrix = _matrix;
		characterID = FObjCollection::Increment();
	}
	virtual ~FButtonRecord1() { delete matrix; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fdt_check(flare_fdt_button_record1_head(_SWFStream->Handle(), hit, down, over, up, characterID, layer));
		matrix->WriteToSWFStream(_SWFStream);
	}

private:
	U8 hit;
	U8 down;
	U8 over;
	U8 up;
	U16 layer;
	FMatrix *matrix;
	U16 characterID;
};

// Specifies appearance aspects for a button definition (flash 3.0)

class DVAPI FButtonRecord2 : public FAButtonRecord
{

public:
	FButtonRecord2(U8 _hit, U8 _down, U8 _over, U8 _up, U16 _characterID, U16 _layer, FMatrix *_matrix, FACXForm *_colorTransform)
	{
		hit = _hit;
		down = _down;
		over = _over;
		up = _up;
		characterID = _characterID;
		layer = _layer;
		matrix = _matrix;
		colorTransform = _colorTransform;
	}
	virtual ~FButtonRecord2()
	{
		delete matrix;
		delete colorTransform;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fdt_check(flare_fdt_button_record2_head(_SWFStream->Handle(), hit, down, over, up, characterID, layer));
		matrix->WriteToSWFStream(_SWFStream);
		_SWFStream->FlushBits();
		colorTransform->WriteToSWFStream(_SWFStream);
	}

private:
	U8 hit;
	U8 down;
	U8 over;
	U8 up;
	U16 layer;
	FMatrix *matrix;
	FACXForm *colorTransform;
	U16 characterID;
};

// a list of button records

class DVAPI FButtonRecordList
{

public:
	FButtonRecordList() {}
	virtual ~FButtonRecordList()
	{
		while (!listOfButtonRecords.empty()) {
			delete listOfButtonRecords.front();

			listOfButtonRecords.pop_front();
		}
	}
	void AddRecord(FAButtonRecord *_buttonRecord) { listOfButtonRecords.push_back(_buttonRecord); }
	int Size() { return listOfButtonRecords.size(); }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		std::list<FAButtonRecord *>::iterator cursor;

		for (cursor = listOfButtonRecords.begin(); cursor != listOfButtonRecords.end(); cursor++) {
			(*cursor)->WriteToSWFStream(_SWFStream);
		}
	}

private:
	std::list<FAButtonRecord *> listOfButtonRecords;
};

// a flash object that defines a button in a SWF movie (flash 1.0)

class DVAPI FDTDefineButton : public FDT
{

public:
	FDTDefineButton(void) { characterID = FObjCollection::Increment(); }
	virtual ~FDTDefineButton()
	{
		// delete all entries in action record list
		while (!listOfActionRecords.empty()) {
			delete listOfActionRecords.front();
			listOfActionRecords.pop_front();
		}

		//delete all entries in button record list
		while (!listOfButtonRecords.empty()) {
			delete listOfButtonRecords.front();

			listOfButtonRecords.pop_front();
		}
	}
	U16 ID(void) { return characterID; }
	void AddButtonRecord(FButtonRecord1 *_buttonRecord) { listOfButtonRecords.push_back(_buttonRecord); }
	void AddActionRecord(FActionRecord *_actionRecord) { listOfActionRecords.push_back(_actionRecord); }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		FSWFStream body;
		flare_fdt_check(flare_fdt_button_head(body.Handle(), characterID, -1));

		//write the button record list to the body
		std::list<FButtonRecord1 *>::iterator cursor;

		for (cursor = listOfButtonRecords.begin(); cursor != listOfButtonRecords.end(); cursor++) {
			(*cursor)->WriteToSWFStream(&body);
		}

		//write the end of button record flag
		body.WriteByte(0);

		//write the action record list to the body
		std::list<FActionRecord *>::iterator cursor2;
		for (cursor2 = listOfActionRecords.begin(); cursor2 != listOfActionRecords.end(); cursor2++) {
			(*cursor2)->WriteToSWFStream(&body);
		}

		// the action end flag is written by the finish; the tag is stagDefineButton2 (sic, as in
		// the original)
		flare_fdt_check(flare_fdt_button_finish(_SWFStream->Handle(), FLARE_BUTTON_KIND_BUTTON, body.Handle()));
	}

private:
	U16 characterID;
	std::list<FActionRecord *> listOfActionRecords;
	std::list<FButtonRecord1 *> listOfButtonRecords;
};

// a flash object that defines a button in a SWF movie (flash 3.0)

class DVAPI FDTDefineButton2 : public FDT
{

public:
	FDTDefineButton2(U8 _menuFlag)
	{
		characterID = FObjCollection::Increment();
		menuFlag = _menuFlag;
	}
	virtual ~FDTDefineButton2(void)
	{
		//delete all entries in conditionsList
		while (!conditionList.empty()) {
			delete conditionList.front();

			conditionList.pop_front();
		}

		//delete all entries in listOfButtonRecords
		while (!listOfButtonRecords.empty()) {
			delete listOfButtonRecords.front();

			listOfButtonRecords.pop_front();
		}
	}
	U16 ID(void) { return characterID; }
	void AddButtonRecord(FButtonRecord2 *_buttonRecord) { listOfButtonRecords.push_back(_buttonRecord); }
	void AddActionCondition(FActionCondition *_actionCondition) { conditionList.push_back(_actionCondition); }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		FSWFStream body;
		flare_fdt_check(flare_fdt_button_head(body.Handle(), characterID, menuFlag));

		FSWFStream buttonRecordStream;

		//write the list of button records to button record stream
		std::list<FButtonRecord2 *>::iterator cursor;

		for (cursor = listOfButtonRecords.begin(); cursor != listOfButtonRecords.end(); cursor++) {
			(*cursor)->WriteToSWFStream(&buttonRecordStream);
		}

		// terminator, offset to the actions (have to count the action offset also) and the records
		flare_fdt_check(flare_fdt_button2_records(body.Handle(), buttonRecordStream.Handle(), !conditionList.empty()));

		//write the list of action records
		if (!conditionList.empty()) {
			std::list<FActionCondition *>::iterator cursor1;
			std::list<FActionCondition *>::iterator cursor2;
			cursor2 = (conditionList.end());
			cursor2--;

			for (cursor1 = conditionList.begin(); cursor1 != cursor2; cursor1++) {
				(*cursor1)->WriteToSWFStream(&body);
			}

			(*cursor1)->WriteToSWFStream(&body, 1); //flag indicating it is the last action condition
		}

		flare_fdt_check(flare_fdt_button_finish(_SWFStream->Handle(), FLARE_BUTTON_KIND_BUTTON2, body.Handle()));
	}
	virtual void SetID(U16 id) { characterID = id; }

private:
	U16 characterID;
	U8 menuFlag;
	std::list<FActionCondition *> conditionList;
	std::list<FButtonRecord2 *> listOfButtonRecords;
};

//A flash object that defines a color transformation on a button

class DVAPI FDTDefineButtonCXForm : public FDT
{

public:
	FDTDefineButtonCXForm(U16 _characterID, FCXForm *_colorTransform)
	{
		characterID = _characterID;
		colorTransform = _colorTransform;
	}
	virtual ~FDTDefineButtonCXForm() { delete colorTransform; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		FSWFStream body;

		flare_fdt_check(flare_fdt_button_head(body.Handle(), characterID, -1));
		colorTransform->WriteToSWFStream(&body);

		flare_fdt_check(flare_fdt_button_finish(_SWFStream->Handle(), FLARE_BUTTON_KIND_CXFORM, body.Handle()));
	}

private:
	U16 characterID;
	FCXForm *colorTransform;
};

class DVAPI FDTDefineButtonSound : public FDT
{

public:
	FDTDefineButtonSound(U16 _buttonID, U16 _soundID0, FSoundInfo *_soundInfo0,
						 U16 _soundID1, FSoundInfo *_soundInfo1,
						 U16 _soundID2, FSoundInfo *_soundInfo2,
						 U16 _soundID3, FSoundInfo *_soundInfo3)
	{
		buttonID = _buttonID;
		soundID0 = _soundID0;
		soundInfo0 = _soundInfo0;
		soundID1 = _soundID1;
		soundInfo1 = _soundInfo1;
		soundID2 = _soundID2;
		soundInfo2 = _soundInfo2;
		soundID3 = _soundID3;
		soundInfo3 = _soundInfo3;
	}
	virtual ~FDTDefineButtonSound()
	{
		delete soundInfo0;
		delete soundInfo1;
		delete soundInfo2;
		delete soundInfo3;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		FSWFStream body;

		flare_fdt_check(flare_fdt_button_head(body.Handle(), buttonID, -1));

		body.WriteWord((U32)soundID0);
		soundInfo0->WriteToSWFStream(&body);

		body.WriteWord((U32)soundID1);
		soundInfo1->WriteToSWFStream(&body);

		body.WriteWord((U32)soundID2);
		soundInfo2->WriteToSWFStream(&body);

		body.WriteWord((U32)soundID3);
		soundInfo3->WriteToSWFStream(&body);

		flare_fdt_check(flare_fdt_button_finish(_SWFStream->Handle(), FLARE_BUTTON_KIND_SOUND, body.Handle()));
	}

private:
	U16 buttonID;
	U16 soundID0;
	U16 soundID1;
	U16 soundID2;
	U16 soundID3;
	FSoundInfo *soundInfo0;
	FSoundInfo *soundInfo1;
	FSoundInfo *soundInfo2;
	FSoundInfo *soundInfo3;
};

#endif
