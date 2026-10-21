// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/09/1999.

/****************************************************************************************

				File Summary: FDTFonts.h

  	This header-file contains the declarations of all low-level font-related classes. 
	Their parent classes are in the parentheses:

		class FKerningRec;
		class FDTDefineFont; (public FDT)
		class FDTDefineFont2; (public FDT)
		class FDTDefineFontInfo; (public FDT)
//		class FGlyphEntry;

  	Note: Class FGlyphEntry has been commented out. Need to fix.

****************************************************************************************/

#ifndef _FDT_FONTS_H_
#define _FDT_FONTS_H_

#ifdef WIN32 // added from DV
#pragma warning(push)
#pragma warning(disable : 4786)
#pragma warning(disable : 4251)

#endif

#include <list>
#include <string>
#include <vector>

#include "tcommon.h"
#include "FDT.h"
#include "FPrimitive.h"
#include "FDTShapes.h"

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

// FDTFonts.cpp was ported to FDTFonts.rs: the tag bodies (flags, offset tables, codes, layout,
// the font info code table, kerning records) are written there; the classes (glyph/bounds/name
// ownership, character IDs, the calls into the virtual FShape / FKerningRec writers) stay inline.
extern "C" {
int flare_fdt_define_font_write(void *out, unsigned int character_id, const unsigned int *offsets, size_t n, void *shapes);
int flare_fdt_define_font2_body(void *body, unsigned int font_id, int has_layout, unsigned short encode_type, int italic,
								int bold, const unsigned char *name, size_t name_len, size_t glyph_count,
								const short *offsets, const unsigned short *codes, const short *advances,
								const int *bounds, void *shapes, short ascender, short descender, short leading,
								size_t kerning_count);
int flare_fdt_define_font_info_write(void *out, unsigned int font_id, const unsigned char *name, size_t name_len,
									 unsigned short encode_type, unsigned int italic, unsigned int bold,
									 const unsigned short *codes, size_t n);
int flare_fkerning_write(void *h, unsigned short wide, unsigned short code1, unsigned short code2, short adjust);
}

//	A kerning record

class FKerningRec
{

public:
	FKerningRec(U16 cd1, U16 cd2, S16 krnAdj)
	{
		wideCodesFlag = 0; // default not wide
		code1 = cd1;
		code2 = cd2;
		kerningAdjust = krnAdj;
	}
	void CodesWide(U16 _flag) { wideCodesFlag = _flag ? 1 : 0; }
	virtual void WriteToSWFStream(FSWFStream *b)
	{
		flare_fdt_check(flare_fkerning_write(b->Handle(), wideCodesFlag, code1, code2, kerningAdjust));
	}

	virtual ~FKerningRec() {}

private:
	U16 wideCodesFlag;
	U16 code1;
	U16 code2;
	S16 kerningAdjust;
};

// A flash object that defines a font's appearance

class DVAPI FDTDefineFont : public FDT
{

public:
	FDTDefineFont(void)
	{
		characterID = FObjCollection::Increment();
		nFillBits = 1;
		nLineBits = 0;
	}
	virtual ~FDTDefineFont()
	{
		while (!shapeGlyphs.empty()) {
			delete shapeGlyphs.front();
			shapeGlyphs.pop_front();
		}
	}

	U16 ID(void) { return (U8)characterID; } // (sic: the low byte only, as in the original)
	void AddShapeGlyph(FShape *_shape)
	{
		_shape->SetFillBits(nFillBits);
		_shape->SetLineBits(nLineBits);
		shapeGlyphs.push_back(_shape);
	}
	int NumberOfGlyphs() { return shapeGlyphs.size(); }

	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		U32 offsetsBufferSize = shapeGlyphs.size() * 2;
		FSWFStream shapeBuffer;

		// the offsets of the glyphs (the first one is the size of the offset table itself);
		// the glyph shapes are rendered into the shape buffer. (An empty font used to
		// dereference the end of the list; it now gets just the first offset.)
		std::vector<U32> offsets;
		offsets.push_back(offsetsBufferSize);
		size_t i = 0, n = shapeGlyphs.size();
		for (std::list<FShape *>::iterator cursor = shapeGlyphs.begin(); cursor != shapeGlyphs.end(); cursor++, i++) {
			(*cursor)->WriteToSWFStream(&shapeBuffer);
			if (i + 1 != n)
				offsets.push_back(offsetsBufferSize + shapeBuffer.Size());
		}
		flare_fdt_check(flare_fdt_define_font_write(_SWFStream->Handle(), characterID, offsets.data(), offsets.size(),
													shapeBuffer.Handle()));
	}

private:
	U32 characterID;
	std::list<FShape *> shapeGlyphs;
	U32 nFillBits;
	U32 nLineBits;
};

// A flash object that defines a font's appearance (flash 3.0)

class DVAPI FDTDefineFont2 : public FDT
{
public:
	FDTDefineFont2(const char *_fontName,
				   U16 _encodeType, //  ShiftJIS, Unicode, ANSI
				   U16 _italicFlag,
				   U16 _boldFlag)
	{
		fontID = FObjCollection::Increment();
		hasLayoutFlag = 1;
		encodeType = _encodeType;
		italicFlag = _italicFlag;
		boldFlag = _boldFlag;
		fontName = new FString((U8 *)_fontName);
		nFillBits = 1;
		nLineBits = 0;
		ascenderHeight = 0;
		descenderHeight = 0;
		leadingHeight = 0;
	}

	FDTDefineFont2(const char *_fontName,
				   U16 _encodeType,
				   U16 _italicFlag,
				   U16 _boldFlag,
				   S16 _ascenderHeight,
				   S16 _descenderHeight,
				   S16 _leadingHeight)
	{
		fontID = FObjCollection::Increment();
		hasLayoutFlag = 1;
		encodeType = _encodeType;
		italicFlag = _italicFlag;
		boldFlag = _boldFlag;
		fontName = new FString((U8 *)_fontName);

		ascenderHeight = _ascenderHeight;
		descenderHeight = _descenderHeight;
		leadingHeight = _leadingHeight;

		nFillBits = 1;
		nLineBits = 0;
	}
	virtual ~FDTDefineFont2()
	{
		delete fontName;

		while (!glyphs.empty()) {
			delete glyphs.front().shape;
			delete glyphs.front().bounds;
			glyphs.pop_front();
		}

		while (!kerningTable.empty()) {
			delete kerningTable.front();
			kerningTable.pop_front();
		}
	}

	void AddShapeGlyph(FShape *_shape, U16 _shapeCode, S16 _shapeAdvance = 0,
					   FRect *_shapeBounds = 0)
	{
		// FIXME, don't know what nFillBits and nLineBits are
		_shape->SetFillBits(nFillBits);
		_shape->SetLineBits(nLineBits);
		Glyph g;
		g.advance = _shapeAdvance;
		g.bounds = _shapeBounds;
		g.code = _shapeCode;
		g.shape = _shape;
		glyphs.push_back(g);
	}
	void AddKerningRec(FKerningRec *_kerningRecord)
	{
		if (hasLayoutFlag)
			kerningTable.push_back(_kerningRecord);
	}
	U16 nIndexBits() { return (U16)FSWFStream::MinBits(glyphs.size() - 1, false); }
	U16 ID(void) { return fontID; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		// FIXME: add wide offset later
		FSWFStream body;
		FSWFStream shapeBuffer;

		// per glyph: the offset (truncated to a signed byte, as in the original), the code, the
		// advance and the bounds (a glyph added without bounds used to be a null dereference;
		// it gets an empty rectangle), while the shapes are rendered into the shape buffer
		std::vector<S16> offsetTable;
		std::vector<U16> codes;
		std::vector<S16> advances;
		std::vector<S32> bounds;
		for (std::list<Glyph>::iterator glyphCursor = glyphs.begin(); glyphCursor != glyphs.end(); glyphCursor++) {
			offsetTable.push_back((S8)shapeBuffer.Size());
			glyphCursor->shape->WriteToSWFStream(&shapeBuffer);
			codes.push_back(glyphCursor->code);
			advances.push_back(glyphCursor->advance);
			FRect empty;
			FRect *r = glyphCursor->bounds ? glyphCursor->bounds : &empty;
			bounds.push_back(r->Xmin());
			bounds.push_back(r->Xmax());
			bounds.push_back(r->Ymin());
			bounds.push_back(r->Ymax());
		}

		std::string name = fontName->GetString();
		flare_fdt_check(flare_fdt_define_font2_body(body.Handle(), fontID, hasLayoutFlag, encodeType, italicFlag, boldFlag,
													(const unsigned char *)name.data(), name.length(), glyphs.size(),
													offsetTable.data(), codes.data(), advances.data(), bounds.data(),
													shapeBuffer.Handle(), ascenderHeight, descenderHeight, leadingHeight,
													kerningTable.size()));

		// the kerning records write themselves (virtual)
		if (hasLayoutFlag) {
			for (std::list<FKerningRec *>::iterator kernCursor = kerningTable.begin(); kernCursor != kerningTable.end(); kernCursor++) {
				(*kernCursor)->CodesWide(0); // 0 is narraw offset flag
				(*kernCursor)->WriteToSWFStream(&body);
			}
		}

		// create entire tag with record header
		flare_fdt_check(flare_fdt_finish_tag(_SWFStream->Handle(), stagDefineFont2, body.Handle(), 0, 0));
	}

private:
	U16 fontID;
	int hasLayoutFlag;
	U16 encodeType;
	int italicFlag;
	int boldFlag;

	FString *fontName;
	struct Glyph {
		FShape *shape;
		U16 code;
		S16 advance;
		FRect *bounds;
	};
	S16 ascenderHeight;
	S16 descenderHeight;
	S16 leadingHeight;

	std::list<Glyph> glyphs;

	std::list<FKerningRec *> kerningTable;
	U32 nFillBits;
	U32 nLineBits;
};

// A flash object that defines the mapping from a flash font object to a TrueType or ATM font so that a player can optionally use them

class DVAPI FDTDefineFontInfo : public FDT
{

public:
	FDTDefineFontInfo(const char *_fontName,
					  U16 _fontID,
					  U16 _encodeType,
					  U16 _italicFlag,
					  U16 _boldFlag)
	{
		characterID = FObjCollection::Increment();
		encodeType = _encodeType;
		italicFlag = _italicFlag;
		boldFlag = _boldFlag;

		fontID = _fontID;

		fontName = new FString((U8 *)_fontName);
	}
	virtual ~FDTDefineFontInfo() { delete fontName; }
	void AddCode(U16 _someCode) { codeTable.push_back(_someCode); }
	U16 ID(void) { return (U8)characterID; } // (sic: the low byte only, as in the original)
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		std::string name = fontName->GetString();
		std::vector<U16> codes(codeTable.begin(), codeTable.end());
		flare_fdt_check(flare_fdt_define_font_info_write(_SWFStream->Handle(), fontID, (const unsigned char *)name.data(),
														 name.length(), encodeType, italicFlag, boldFlag, codes.data(),
														 codes.size()));
		// the original consumed the code table while writing it
		codeTable.clear();
	}

private:
	U32 characterID;
	FString *fontName;
	U16 encodeType;
	U16 italicFlag;
	U16 boldFlag;
	std::list<U16> codeTable;
	U16 fontID;
};

//	Found in DefineText.  Used to describe the glyph index and X advance value to use for a
//	particular character in the currently selected font for the text record.

// class FGlyphEntry {
//
// public:
//
// 	FGlyphEntry (U16 index, S16 advance);
// 	S16 AdvanceValue();
// 	void IncludeNBitInfo(U16 _nIndexBits, U16 _nAdvanceBits);
// 	void WriteToSWFStream(FSWFStream *_SWFStream);
//
//
// private:
//
// 	U16 glyphIndex;
// 	S16 glyphAdvance;
// 	U16 nIndexBits;
// 	U16 nAdvanceBits;
//
// };

#endif
