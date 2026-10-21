// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/18/1999.
// Mostly about FRGB, FRGBA, and FColor.
// Additionally, there is some brain-dump on how color should be implemented in low and
// high level manager.

/****************************************************************************************

				File Summary: FPrmitive.h

	This header-file contains the declarations of low-level prmitive-related structs and
	classes.
	
		struct FRGB;
		struct FRGBA;

		class FColor;
		class FMatrix;
		class FRect;
		class FString;

****************************************************************************************/

#ifndef FPRIMITIVE_INCLUDED
#define FPRIMITIVE_INCLUDED
#include "tcommon.h"
#include "Macromedia.h"
#include <string>
#include "FSWFStream.h"

// FPrimitive.cpp was ported to FPrimitive.rs: the record writers below take the fields and
// the FSWFStream handle (FSWFStream::Handle()); constructors stay inline here.
extern "C" {
int flare_fcolor_write(void *h, unsigned int red, unsigned int green, unsigned int blue, unsigned int alpha, int alpha_t);
unsigned int flare_fmatrix_min_bits(int x, int y);
int flare_fmatrix_write(void *h, unsigned short identity, unsigned int has_scale, unsigned int n_scale_bits,
						int scale_x, int scale_y, unsigned int has_rotate, unsigned int n_rotate_bits,
						int rotate_skew0, int rotate_skew1, unsigned int n_translate_bits,
						int translate_x, int translate_y);
unsigned int flare_frect_min_bits(int xmin, int xmax, int ymin, int ymax);
int flare_frect_write(void *h, int xmin, int xmax, int ymin, int ymax);
int flare_fstring_write(void *h, const unsigned char *text, size_t len, int null_end);
}

#ifdef WIN32 // added from DV
#pragma warning(push)
#pragma warning(disable : 4786)
#pragma warning(disable : 4251)

#endif

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

// color structures

struct FRGB {
	U8 red;
	U8 green;
	U8 blue;
};

struct FRGBA {
	// 	FRGBA( U8 r, U8 g, U8 b, U8 a )		{ red = r; green = g; blue = b; alpha = a; }
	// 	FRGBA()								{ red = 0; green = 0; blue = 0; alpha = 255; }
	// 	void Set( U8 r, U8 g, U8 b, U8 a )	{ red = r; green = g; blue = b; alpha = a; }
	//
	U8 red;
	U8 green;
	U8 blue;
	U8 alpha;
};

//! This object specifies a color object.
/*! FColor is used (copied) as a structure. Do not add dynamic memory or virtual functions.

	There are two types of FColor, i.e. one with alpha channel on and the other with alpha channel.off.
	When you construct an instance of FColor, if you provide three parameters, the alpha channel is off;
	if you provide four, the alpha channel is on.

	Each color-related SWF tag requires the correct color type, either RGB or RGBA. For example,
	DefineShape, DefineShape2, SetBackgroundColor need RGB (alpha off);
	DefineShape3, DefineMorphShape, DefineEditText need RGBA (alpha on).

	The low-level manager can correct some errors of wrong color type, but not all. So don't rely on it.
	If the wrong one got over low-level manager, the flash play might crash on that.

	We recommend you use RGBA whenever reasonable. The reason? Using RGBA is very consistent
	and worth the sacrifice of some file space (in SWF, it's one more byte for each occurance of color info, and some
	additional for color transformation.)

	Actually, we only provide RGBA interface for colors in high-level manager because this solution is much more elegant
	in terms of consistency and compatibility.
*/
class DVAPI FColor
{
public:
	//! Default FColor is white (0xff, 0xff, 0xff), construct a FColor with Alpha channel off.
	FColor(U32 _red = 0xff, U32 _green = 0xff, U32 _blue = 0xff)
		: red(_red), green(_green), blue(_blue), alpha(0xff), alphaT(false) {}

	//! Construct a FColor with Alpha channel on.
	FColor(U32 _red, U32 _green, U32 _blue, U32 _alpha)
		: red(_red), green(_green), blue(_blue), alpha(_alpha), alphaT(true) {}

	//! Construct a FColor with Alpha channel off.
	FColor(FRGB rgb)
		: red(rgb.red), green(rgb.green), blue(rgb.blue), alpha(0xff), alphaT(false) {}

	//! Construct a FColor with Alpha channel on.
	FColor(FRGBA rgba)
		: red(rgba.red), green(rgba.green), blue(rgba.blue), alpha(rgba.alpha), alphaT(true) {}

	//! Returns red component.
	int Red() { return (int)red; }

	//! Returns green component.
	int Green() { return (int)green; }

	//! Returns blue component.
	int Blue() { return (int)blue; }

	//! Returns alpha value.
	int Alpha() { return (int)alpha; }

	bool HasAlpha() { return alphaT; }

	//! Alpha switch: turn on/off alpha channel.
	void AlphaChannel(bool on) { alphaT = on; }

	bool operator==(const FColor &color) { return (red == color.red &&
												   green == color.green &&
												   blue == color.blue &&
												   alpha == color.alpha); }

	/*
	enum {
		WRITE_SMALLEST,
		WRITE_3BYTE,
		WRITE_4BYTE
	};
*/
	// Method for internal use.
	void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fcolor_write(_SWFStream->Handle(), red, green, blue, alpha, alphaT ? 1 : 0);
	}

private:
	U32 red;	 // should be U8
	U32 green;   // should be U8
	U32 blue;	// should be U8
	U32 alpha;   // should be U8
	bool alphaT; //alpha type flag
};

class DVAPI FMatrix
{

public:
	//	Creates an identity matrix. (The fields the original left uninitialised are zeroed.)
	FMatrix(void)
	{
		identity = true;
		hasScale = false;
		nScaleBits = 0;
		scaleX = 1;
		scaleY = 1;

		hasRotate = false;
		nRotateBits = 0;
		rotateSkew0 = 0;
		rotateSkew1 = 0;

		nTranslateBits = 0;
		translateX = 0;
		translateY = 0;
	}

	//	The Matrix class constructor
	FMatrix(U32 _hasScale,
			SFIXED _scaleX,
			SFIXED _scaleY,
			U32 _hasRotate,
			SFIXED _rotateSkew0,
			SFIXED _rotateSkew1,
			SCOORD _translateX,
			SCOORD _translateY)
	{
		hasScale = _hasScale;

		scaleX = _scaleX;
		scaleY = _scaleY;

		hasRotate = _hasRotate;
		rotateSkew0 = _rotateSkew0;
		rotateSkew1 = _rotateSkew1;

		translateX = _translateX;
		translateY = _translateY;

		nScaleBits = MinBits(scaleX, scaleY);
		nRotateBits = MinBits(rotateSkew0, rotateSkew1);
		nTranslateBits = MinBits(translateX, translateY);

		identity = false;
	}

	//	Writes the Matrix to the given _SWFStream.
	void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_fmatrix_write(_SWFStream->Handle(), identity, hasScale, nScaleBits, scaleX, scaleY,
							hasRotate, nRotateBits, rotateSkew0, rotateSkew1,
							nTranslateBits, translateX, translateY);
	}

	FMatrix operator*(const FMatrix &b) const
	{
		SFIXED _scaleX = scaleX * b.scaleX + rotateSkew0 * b.rotateSkew1;
		SFIXED _scaleY = scaleY * b.scaleY + rotateSkew0 * b.rotateSkew1;
		SFIXED _rot0 = scaleX * b.rotateSkew0 + rotateSkew0 * b.scaleY;
		SFIXED _rot1 = rotateSkew1 * b.scaleX + scaleY * b.rotateSkew1;

		return FMatrix(
			_scaleX != 1 || scaleY != 1, _scaleX, _scaleY,
			_rot0 != 0 || _rot1 != 0, _rot0, _rot1,
			scaleX * b.translateX + rotateSkew0 * b.translateY + translateX,
			rotateSkew1 * b.translateX + scaleY * b.translateY + translateY);
	}

public:
	U32 hasScale;
	U32 nScaleBits;

	SFIXED scaleX;
	SFIXED scaleY;

	U32 hasRotate;
	U32 nRotateBits;
	SFIXED rotateSkew0;
	SFIXED rotateSkew1;

	U32 nTranslateBits;
	SCOORD translateX;
	SCOORD translateY;

	// Minimum number of bits for two signed numbers (the larger magnitude, signed).
	U32 MinBits(S32 x, S32 y) { return flare_fmatrix_min_bits(x, y); }
	U16 identity;
};

//rectangle class

class DVAPI FRect
{
public:
	FRect() { xmin = ymin = xmax = ymax = 0; }
	FRect(SCOORD _xmin, SCOORD _ymin, SCOORD _xmax, SCOORD _ymax)
	{
		xmin = _xmin;
		xmax = _xmax;
		ymin = _ymin;
		ymax = _ymax;
	}

	SCOORD Xmin() { return xmin; }
	SCOORD Ymin() { return ymin; }
	SCOORD Xmax() { return xmax; }
	SCOORD Ymax() { return ymax; }
	SCOORD Width() { return xmax - xmin + 1; }
	SCOORD Height() { return ymax - ymin + 1; }

	void SetXmin(SCOORD val) { xmin = val; }
	void SetYmin(SCOORD val) { ymin = val; }
	void SetXmax(SCOORD val) { xmax = val; }
	void SetYmax(SCOORD val) { ymax = val; }

	void WriteToSWFStream(FSWFStream *_SWFStream)
	{
		flare_frect_write(_SWFStream->Handle(), xmin, xmax, ymin, ymax);
	}

private:
	SCOORD xmin;
	SCOORD xmax;
	SCOORD ymin;
	SCOORD ymax;

	U32 MinBits(void) { return flare_frect_min_bits(xmin, xmax, ymin, ymax); }
};

// a flash string

class DVAPI FString
{

public:
	FString(const U8 *_string) { text = (const char *)_string; }
	FString(const char *_string) { text = _string; }
	std::string GetString() { return text; }

	void WriteToSWFStream(FSWFStream *_SWFStream, bool null_end)
	{
		flare_fstring_write(_SWFStream->Handle(), (const unsigned char *)text.data(), text.length(), null_end ? 1 : 0);
	}
	U16 Length() { return text.length(); }

private:
	std::string text;
};

#ifdef WIN32 // added from DV
#pragma warning(pop)
#endif
#endif
