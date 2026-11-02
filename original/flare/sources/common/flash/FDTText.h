// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/09/1999.

/****************************************************************************************

				File Summary: FDTText.h

	This header-file contains the declarations of low-level text-related classes.
	All parent classes are in the parentheses:

		class FTextRecord;
		class FDTDefineEditText; (public FDT)
		class FDTDefineText; (public FDT)
		class FDTDefineText2; (public FDT)
		class FTextChangeRecord; (public FTextRecord)
		class FTextGlyphRecord; (public FTextRecord)

****************************************************************************************/

#ifndef _F_DEFINE_TEXT_H_
#define _F_DEFINE_TEXT_H_

#ifdef WIN32 // added from DV
#pragma warning(push)
#pragma warning(disable : 4786)
#pragma warning(disable : 4251)

#endif

#include <algorithm>
#include <list>
#include <string>
#include <vector>

#include "tcommon.h"
#include "Macromedia.h"
#include "FDT.h"
#include "FSWFStream.h"
#include "FPrimitive.h"

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

class FGlyphEntry;

// FDTText.cpp was ported to FDTText.rs: DefineEditText is written there in one piece from this
// argument block (same layout as EditTextArgs in the Rust file), the text change and glyph
// records are written/measured there too. The classes (bounds/matrix/colour/string ownership,
// character IDs, the virtual FTextRecord list) stay inline.
struct FlareEditTextArgs {
	int xmin, xmax, ymin, ymax;
	unsigned int hasFont, hasMaxLength, hasTextColor, readOnly, password, multiline, wordWrap, hasText, useOutlines,
		border, noSelect, hasLayout;
	unsigned int fontID, fontHeight, maxLength, alignment, leftMargin, rightMargin, indent, leading;
	unsigned int colorRed, colorGreen, colorBlue, colorAlpha;
	const unsigned char *variableName;
	size_t variableNameLen;
	const unsigned char *initialText;
	size_t initialTextLen;
};

extern "C" {
int flare_fdt_edit_text_write(void *out, unsigned int character_id, const FlareEditTextArgs *args);
int flare_fdt_text_change_write(void *h, unsigned short has_font, unsigned short has_color, unsigned short has_x_offset,
								unsigned short has_y_offset, unsigned short font_id, unsigned short font_height,
								unsigned int red, unsigned int green, unsigned int blue, unsigned int alpha,
								short x_offset, short y_offset);
int flare_fdt_text_glyph_bits(const unsigned short *entries, size_t n, unsigned int *code_bits, unsigned int *advance_bits);
int flare_fdt_text_glyph_write(void *h, const unsigned short *entries, size_t n, int code_bits, int advance_bits);
}

class DVAPI FTextRecord
{
public:
	virtual U32 MinAdvanceBits() = 0; // the computed minimum # of bits to record the advance values
	virtual U32 MinCodeBits() = 0;	// the computed minimum # of bits to record the character code values
	virtual ~FTextRecord() {}

	// Because of multiple text records, the DefineText will specify the code and advance bits
	// when it writes.
	virtual void WriteToSWFStream(FSWFStream *_SWFStream, int codeBits, int advanceBits) = 0;
};

// A flash object that defines a font's appearance

class DVAPI FDTDefineEditText : public FDT
{

public:
	FDTDefineEditText(FRect *_bounds, U8 _hasFont, U8 _hasMaxLength, U8 _hasTextColor,
					  U8 _readOnly, U8 _password, U8 _multiline, U8 _wordWrap, U8 _hasText,
					  U8 _useOutlines, U8 _border, U8 _noSelect, U8 _hasLayout, U16 _fontID,
					  U16 _fontHeight, FColor *_textColor, U16 _maxLength, U8 _alignment,
					  U16 _leftMargin, U16 _rightMargin, U16 _indent, U16 _leading,
					  FString *_variableName, FString *_initialText)
		: textColor(_textColor)
	{
		bounds = _bounds;
		hasFont = _hasFont;
		hasMaxLength = _hasMaxLength;
		hasTextColor = _hasTextColor;
		readOnly = _readOnly;
		password = _password;
		multiline = _multiline;
		wordWrap = _wordWrap;
		hasText = _hasText;
		useOutlines = _useOutlines;
		border = _border;
		noSelect = _noSelect;
		hasLayout = _hasLayout;
		fontID = _fontID;
		fontHeight = _fontHeight;
		textColor = _textColor;
		maxLength = _maxLength;
		alignment = _alignment;
		leftMargin = _leftMargin;
		rightMargin = _rightMargin;
		indent = _indent;
		leading = _leading;
		variableName = _variableName;
		initialText = _initialText;

		characterID = FObjCollection::Increment();
	}
	//	deletes the FRECT and 2 FString's if they exist.
	virtual ~FDTDefineEditText(void)
	{
		delete bounds;
		delete variableName;
		delete initialText;
		delete textColor;
	}
	U16 ID(void) { return characterID; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		// (a missing bounds / name / text / colour used to be a null dereference; they are
		// written as an empty rectangle, empty strings and black)
		FRect empty;
		FRect *r = bounds ? bounds : &empty;
		std::string name = variableName ? variableName->GetString() : std::string();
		std::string text = initialText ? initialText->GetString() : std::string();

		FlareEditTextArgs a;
		a.xmin = r->Xmin();
		a.xmax = r->Xmax();
		a.ymin = r->Ymin();
		a.ymax = r->Ymax();
		a.hasFont = hasFont;
		a.hasMaxLength = hasMaxLength;
		a.hasTextColor = hasTextColor;
		a.readOnly = readOnly;
		a.password = password;
		a.multiline = multiline;
		a.wordWrap = wordWrap;
		a.hasText = hasText;
		a.useOutlines = useOutlines;
		a.border = border;
		a.noSelect = noSelect;
		a.hasLayout = hasLayout;
		a.fontID = fontID;
		a.fontHeight = fontHeight;
		a.maxLength = maxLength;
		a.alignment = alignment;
		a.leftMargin = leftMargin;
		a.rightMargin = rightMargin;
		a.indent = indent;
		a.leading = leading;
		a.colorRed = a.colorGreen = a.colorBlue = a.colorAlpha = 0;
		if (hasTextColor && textColor) {
			textColor->AlphaChannel(true); // (the original switched the owned colour to RGBA here)
			a.colorRed = textColor->Red();
			a.colorGreen = textColor->Green();
			a.colorBlue = textColor->Blue();
			a.colorAlpha = textColor->Alpha();
		}
		a.variableName = (const unsigned char *)name.data();
		a.variableNameLen = name.length();
		a.initialText = (const unsigned char *)text.data();
		a.initialTextLen = text.length();
		flare_fdt_check(flare_fdt_edit_text_write(_SWFStream->Handle(), characterID, &a));
	}

private:
	FRect *bounds;
	U8 hasFont;
	U8 hasMaxLength;
	U8 hasTextColor;
	U8 readOnly;
	U8 password;
	U8 multiline;
	U8 wordWrap;
	U8 hasText;
	U8 useOutlines;
	U8 border;
	U8 noSelect;
	U8 hasLayout;
	U16 fontID;
	U16 fontHeight;
	FColor *textColor;
	U16 maxLength;
	U8 alignment;
	U16 leftMargin;
	U16 rightMargin;
	U16 indent;
	U16 leading;
	FString *variableName;
	FString *initialText;
	U16 characterID;
};

// A flash object that defines the font and formating of text characters in the record (flash 1.0)
// takes only RGB color values

class DVAPI FDTDefineText : public FDT
{

public:
	// glyhpsInFont: how many characters are in the font?
	FDTDefineText(FRect *_textBounds, FMatrix *_textMatrix, int glyhpsInFont)
	{
		characterID = FObjCollection::Increment();
		textBounds = _textBounds;
		textMatrix = _textMatrix;
		nIndexBits = (U8) FSWFStream::MinBits(glyhpsInFont, 0);
	}
	virtual ~FDTDefineText()
	{
		delete textBounds;

		delete textMatrix;

		while (!textRecords.empty()) {
			delete textRecords.front();

			textRecords.pop_front();
		}
	}
	void AddTextRecord(FTextRecord *_textRec) { textRecords.push_back(_textRec); }
	U16 ID(void) { return (U16)characterID; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		U16 nAdvanceBits = 0;
		U16 nCodeBits = 0;
		FSWFStream body;

		body.WriteWord((U32)characterID);

		textBounds->WriteToSWFStream(&body);
		textMatrix->WriteToSWFStream(&body);

		// Get the bits needed to write the advance and character codes
		std::list<FTextRecord *>::iterator cursor1;
		for (cursor1 = textRecords.begin(); cursor1 != textRecords.end(); cursor1++) {
			nAdvanceBits = (U16)std::max((*cursor1)->MinAdvanceBits(), (U32)nAdvanceBits);
			nCodeBits = (U16)std::max((*cursor1)->MinCodeBits(), (U32)nCodeBits);
		}
		body.WriteByte((U32)nCodeBits);
		body.WriteByte((U32)nAdvanceBits);

		std::list<FTextRecord *>::iterator cursor;
		for (cursor = textRecords.begin(); cursor != textRecords.end(); cursor++) {
			(*cursor)->WriteToSWFStream(&body, nCodeBits, nAdvanceBits);
		}

		// flush the bit writer, terminate the record list (a zero byte) and wrap the tag
		flare_fdt_check(flare_fdt_finish_tag(_SWFStream->Handle(), stagDefineText, body.Handle(), 1, 1));
	}

private:
	U32 characterID;
	FRect *textBounds;
	FMatrix *textMatrix;
	std::list<FTextRecord *> textRecords;
	U16 nIndexBits;
};

// A flash object that defines the font and formating of text characters in the record (flash 1.0)
// takes RGBA color values

class DVAPI FDTDefineText2 : public FDT
{

public:
	FDTDefineText2(FRect *_textBounds, FMatrix *_textMatrix, int glyhpsInFont)
	{
		characterID = FObjCollection::Increment();
		textBounds = _textBounds;
		textMatrix = _textMatrix;
		nIndexBits = (int) FSWFStream::MinBits(glyhpsInFont, 0);
	}
	virtual ~FDTDefineText2()
	{
		delete textBounds;

		delete textMatrix;

		while (!textRecords.empty()) {
			delete textRecords.front();

			textRecords.pop_front();
		}
	}
	void AddTextRecord(FTextRecord *_textRec) { textRecords.push_back(_textRec); }
	U16 ID(void) { return (U16)characterID; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		U16 nAdvanceBits = 0;
		U16 nCodeBits = 0;
		FSWFStream body;

		body.WriteWord((U32)characterID);

		textBounds->WriteToSWFStream(&body);
		textMatrix->WriteToSWFStream(&body);

		// Get the bits needed to write the advance and character codes
		std::list<FTextRecord *>::iterator cursor1;
		for (cursor1 = textRecords.begin(); cursor1 != textRecords.end(); cursor1++) {
			nAdvanceBits = (U16)std::max((*cursor1)->MinAdvanceBits(), (U32)nAdvanceBits);
			nCodeBits = (U16)std::max((*cursor1)->MinCodeBits(), (U32)nCodeBits);
		}
		body.WriteByte((U32)nCodeBits);
		body.WriteByte((U32)nAdvanceBits);

		std::list<FTextRecord *>::iterator cursor;
		for (cursor = textRecords.begin(); cursor != textRecords.end(); cursor++) {
			(*cursor)->WriteToSWFStream(&body, nCodeBits, nAdvanceBits);
		}

		// flush the bit writer, terminate the record list (a zero byte) and wrap the tag
		flare_fdt_check(flare_fdt_finish_tag(_SWFStream->Handle(), stagDefineText2, body.Handle(), 1, 1));
	}

private:
	U32 characterID;
	FRect *textBounds;
	FMatrix *textMatrix;
	std::list<FTextRecord *> textRecords;
	U16 nIndexBits;
};

// specifies text format changes in a flash DefineText object

class DVAPI FTextChangeRecord : public FTextRecord
{

public:
	FTextChangeRecord(U16 _hasFontFlag, U16 _hasColorFlag,
					  U16 _hasXOffsetFlag, U16 _hasYOffsetFlag,
					  U16 _fontID, U16 _fontHeight, FColor *_fontColor,
					  S16 _xOffset, S16 _yOffset)
	{
		hasFontFlag = _hasFontFlag;
		hasColorFlag = _hasColorFlag;
		hasXOffsetFlag = _hasXOffsetFlag;
		hasYOffsetFlag = _hasYOffsetFlag;
		fontID = _fontID;
		fontColor = _fontColor;
		xOffset = _xOffset;
		yOffset = _yOffset;
		fontHeight = _fontHeight;
	}
	virtual ~FTextChangeRecord() { delete fontColor; }

	virtual U32 MinAdvanceBits() { return 0; }
	virtual U32 MinCodeBits() { return 0; }

	virtual void WriteToSWFStream(FSWFStream *_SWFStream, int /*codeBits*/, int /*advanceBits */)
	{
		unsigned int r = 0, g = 0, b = 0, a = 0;
		if (hasColorFlag && fontColor) { // (a missing colour used to be a null dereference)
			fontColor->AlphaChannel(true);	// (the original switched the owned colour to RGBA here)
			r = fontColor->Red();
			g = fontColor->Green();
			b = fontColor->Blue();
			a = fontColor->Alpha();
		}
		flare_fdt_check(flare_fdt_text_change_write(_SWFStream->Handle(), hasFontFlag, hasColorFlag, hasXOffsetFlag,
													hasYOffsetFlag, fontID, fontHeight, r, g, b, a, xOffset, yOffset));
	}

private:
	U16 reserved;
	U16 hasFontFlag;
	U16 hasColorFlag;
	U16 hasYOffsetFlag;
	U16 hasXOffsetFlag;
	U16 fontID;
	FColor *fontColor;
	S16 xOffset;
	S16 yOffset;
	U16 fontHeight;
};

class DVAPI FTextGlyphRecord : public FTextRecord
{

public:
	FTextGlyphRecord() {}
	virtual ~FTextGlyphRecord()
	{
		while (!glyphEntries.empty()) {
			glyphEntries.pop_front();
		}
	}

	void AddGlyphEntry(U16 code, U16 advance)
	{
		GlyphEntry glyph = {code, advance};

		glyphEntries.push_back(glyph);
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream, int codeBits, int advanceBits)
	{
		// Bug: The number of glyphs used to limit the the max code size, but this changes with
		// DefineFont2, where we could use glyph 38 as our sole glyph. So the re-write begins....
		std::vector<U16> e = entries();
		flare_fdt_check(flare_fdt_text_glyph_write(_SWFStream->Handle(), e.data(), glyphEntries.size(), codeBits, advanceBits));
	}

	virtual U32 MinAdvanceBits() // number of bits needed to write the advance data
	{
		U32 code, advance;
		std::vector<U16> e = entries();
		flare_fdt_check(flare_fdt_text_glyph_bits(e.data(), glyphEntries.size(), &code, &advance));
		return advance;
	}
	virtual U32 MinCodeBits() // number of bits needed to write the character code data
	{
		U32 code, advance;
		std::vector<U16> e = entries();
		flare_fdt_check(flare_fdt_text_glyph_bits(e.data(), glyphEntries.size(), &code, &advance));
		return code;
	}

private:
	struct GlyphEntry {
		U16 code;
		U16 advance;
	};
	std::list<GlyphEntry> glyphEntries;

	// (code, advance) pairs, as flare_fdt_text_glyph_* take them
	std::vector<U16> entries() const
	{
		std::vector<U16> e;
		for (std::list<GlyphEntry>::const_iterator it = glyphEntries.begin(); it != glyphEntries.end(); ++it) {
			e.push_back(it->code);
			e.push_back(it->advance);
		}
		return e;
	}
};

#endif
