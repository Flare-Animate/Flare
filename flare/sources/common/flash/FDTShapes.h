// Copyright © 1999 Middlesoft, Inc. All rights reserved.
// First Created By Lee Thomason.
// First Created On 09/08/1999.
// Last Modified On 11/09/1999.

/****************************************************************************************

				File Summary: FDTShapes.h

	This header-file contains the declarations of all low-level shape-related classes. 
	Their parent classes are in the parentheses:

		class FShape;
		class FACXForm;
		class FALineStyle;
		class FAGradRecord;
		class FCXForm;  (public FACXForm)
		class FCXFormWAlpha; (public FACXForm)
		class FDTDefineMorphShape; (public FDT)
		class FDTDefineShape; (public FDT)
		class FDTDefineShape2; (public FDT)
		class FDTDefineShape3; (public FDT)
		class FAFillStyle;
		class FFillStyle; (public FAFillStyle)
		class FFillStyleArray;
		class FFillStyleBitmap; (public FFillStyle)
		class FFillStyleGradient; (public FFillStyle)
		class FFillStyleSolid; (public FFillStyle)
		class FGradient;
		class FGradRecord; (public FAGradRecord)
		class FLineStyle; (public FALineStyle)
		class FLineStyleArray;
		class FMorphFillStyle; (public FAFillStyle)
		class FMorphFillStyleBitmap; (public FMorphFillStyle)
		class FMorphFillStyleGradient; (public FMorphFillStyle)
		class FMorphFillStyleSolid; (public FMorphFillStyle)
		class FMorphGradRecord; (public FAGradRecord)
		class FMorphLineStyle; (public FALineStyle)
		class FShapeRec;
		class FShapeRecChange; (public FShapeRec)
		class FShapeRecEdgeStraight; (public FShapeRec)
		class FShapeRecEdgeCurved; (public FShapeRec)
		class FShapeRecEnd; (public FShapeRec)
		class FShapeWStyle;

****************************************************************************************/

#ifndef _FDT_SHAPES_H_
#define _FDT_SHAPES_H_

#include "Macromedia.h"
#include "FDT.h"
#include "FSWFStream.h"
#include "FPrimitive.h"
#include "tpixel.h"

#include <assert.h>
#include <list>
#include <map>
#include <stdexcept>
#include <vector>

// FDTShapes.cpp was ported to FDTShapes.rs: the record layouts (colour transforms, style array
// counts, fill / line style heads, shape records, the DefineShape / DefineMorphShape tag assembly)
// are written there. The classes (the owning pointer graph of styles and shape records, virtual
// WriteToSWFStream / IncludeNFillBitInfo, FColor / FMatrix / FRect sub-objects) stay in this
// header with inline member definitions at the end of the file.
extern "C" {
unsigned int flare_fcxform_min_bits(int rm, int gm, int bm, int ra, int ga, int ba);
unsigned int flare_fcxform_alpha_min_bits(int rm, int gm, int bm, int am, int ra, int ga, int ba, int aa);
int flare_fcxform_write(void *out, unsigned int has_add, unsigned int has_mult, unsigned int n_bits, int rm, int gm, int bm,
						int ra, int ga, int ba);
int flare_fcxform_alpha_write(void *out, unsigned int has_add, unsigned int has_mult, unsigned int n_bits, int rm, int gm,
							  int bm, int am, int ra, int ga, int ba, int aa);
int flare_fstyle_array_count(void *out, unsigned int size);
int flare_ffill_style_head(void *out, int kind, int flag, unsigned short bitmap_id);
int flare_fgradient_head(void *out, unsigned int count);
int flare_fgrad_ratio(void *out, unsigned int ratio);
int flare_fline_style_head(void *out, unsigned int width);
int flare_fmorph_line_style_head(void *out, unsigned int width1, unsigned int width2);
int flare_fshape_head(void *out, unsigned int n_fill_bits, unsigned int n_line_bits);
unsigned int flare_fshaperec_change_min_bits(int dx, int dy);
int flare_fshaperec_change_head(void *out, unsigned int state_new_styles, unsigned int state_line_style,
								unsigned int state_fill_style1, unsigned int state_fill_style0, unsigned int state_move_to,
								unsigned int n_move_bits, int move_dx, int move_dy, unsigned int fill0_style,
								unsigned int fill1_style, unsigned int line_style, unsigned int n_fill_bits,
								unsigned int n_line_bits);
int flare_fshaperec_change_new_styles(void *out, unsigned int fill_count, unsigned int line_count, unsigned int *n_fill_bits,
									  unsigned int *n_line_bits);
// layout shared with StraightEdge in FDTShapes.rs
struct FlareStraightEdge {
	unsigned int general_line_flag;
	int general_delta_x;
	int general_delta_y;
	unsigned int vertical_line_flag;
	int horizontal_delta_x;
	int vertical_delta_y;
	unsigned int n_bits;
};
int flare_fedge_straight_new(int dx, int dy, FlareStraightEdge *edge);
int flare_fedge_straight_write(void *out, const FlareStraightEdge *edge);
unsigned int flare_fedge_curved_min_bits(int control_dx, int control_dy, int anchor_dx, int anchor_dy);
int flare_fedge_curved_write(void *out, unsigned int n_bits, int control_dx, int control_dy, int anchor_dx, int anchor_dy);
int flare_fshaperec_end_write(void *out);
int flare_fdt_shape_head(void *body, unsigned short character_id);
int flare_fdt_shape_finish(void *out, int kind, void *body);
int flare_fdt_morph_join(void *body, void *temp);
}
enum {
	FLARE_FILL_KIND_SOLID = 0,
	FLARE_FILL_KIND_BITMAP = 1,
	FLARE_FILL_KIND_GRADIENT = 2
};
enum {
	FLARE_SHAPE_TAG_SHAPE = 0,
	FLARE_SHAPE_TAG_SHAPE2 = 1,
	FLARE_SHAPE_TAG_SHAPE3 = 2,
	FLARE_SHAPE_TAG_MORPH = 3
};
inline void flare_fshapes_check(int status)
{
	if (status)
		throw std::invalid_argument("FDTShapes: invalid argument");
}

class FMorphFillStyle;
class FMorphLineStyle;
class FShapeRec;
class FFillStyle;
class FLineStyle;
class FFillStyleArray;
class FLineStyleArray;
class FShapeWStyle;
class FGradient;

#ifdef WIN32 // added from DV
#pragma warning(push)
#pragma warning(disable : 4786)
#pragma warning(disable : 4251)

#endif

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

//! Holds an array of ShapeRec's
/*!	The ShapeRecs are  written to a SWFStream in 
	the order they are added to the array. Normally used to store Font 
	Glyphs in a Definefont tag. An array of ShapeRec'sThis should probably 
	be called FShapeArray or such. FShapeWStyle is an expanded version.
	\sa FShapeRec, FShapeRecChange,
*/
class DVAPI FShape
{
public:
	FShape();
	virtual ~FShape();

	//!	Sets the number of bits necessary to index a Fill Style array into the SWF field nFillBits.
	/*! The SWF SHAPE tag needs the number of bits necessary to index a 
	fillstyle array However, what Fill Style Array is it referring to ??? 
	Put 0 for now.
	\param _nFillBits
	*/
	void SetFillBits(U32 _nFillBits);
	//!	Sets the number of bits necessary to index a Line Style array into theSWF field nFillBits.
	/*! The SWF SHAPE tag also needs the number of bits necessary to index a 
	linestyle array. Again, what Line Style Array is it referring to ??? 
	Put 0 for now.
	\param _nLineBits
	*/
	void SetLineBits(U32 _nLineBits);

	//!	Add a ShapeRec (of type curve, line, or ChangeRec) to this Shape
	/*!	Add a ShapeRec which could be a ChangeRec, or an EdgeRec (curve or line)
		to this Shape.  The first is always a ChangeRec to set drawing position.
	
	  \param	shapeRec
	 */
	void AddShapeRec(FShapeRec *shapeRec);

	//!	Stream the ShapeRecs out to the temporary buffer
	/*!	For each ShapeRec, tell it the # of fill bits, line bits, 
		and tell it to write itself to the temp stream.
	
	  \param	_SWFStream: the temporary output stream
	 */
	virtual void WriteToSWFStream(FSWFStream *_SWFStream); //not Complete, see comments

private:
	U32 nFillBits;
	U32 nLineBits;
	std::vector<FShapeRec *> shapeRecs;
};

//!	Abstract Base class of FCXForm and FCXFormWAlpha
/*!	Specifies a color transform for certain objects

  \sa FCXForm, 	FCXFormWAlpha
 */
class DVAPI FACXForm
{
public:
	//!	Virtual method. Write the color transform out to stream.
	/*!	Implemented by FCXForm, FCXFormWAlpha
	
	  \param	_SWFStream
	 */
	virtual ~FACXForm() {}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream) = 0;
};

//!	Abstract Base class of FLineStyle, FMorphLineStyle
/*!	All line styles fall into this category (morph or regular)

  \sa	FLineStyle, FMorphLineStyle
 */
class DVAPI FALineStyle
{
public:
	virtual ~FALineStyle() {}
	//!	Virtual Method for writing LineStyles out to a stream
	/*!	Virtual Method for writing LineStyles out to a stream
	
	  \param	_SWFStream	
	 */
	virtual void WriteToSWFStream(FSWFStream *_SWFStream) = 0;
};

//

//!	Virtual Base Class of FGradRecord,FMorphGradRecord
/*!	All grad records fall into this category

  \sa	FGradRecord,FMorphGradRecord
 */
class DVAPI FAGradRecord
{
public:
	virtual ~FAGradRecord() {}

	//!	Virtual Method for writing a Gradient out to a stream

	/*!
	  \param	_SWFStream	
	 */
	virtual void WriteToSWFStream(FSWFStream *_SWFStream) = 0;
};

//

//!	Color Transform
/*!	Specifies a color transformation for some object without transparency information

  \sa	
 */
class DVAPI FCXForm : public FACXForm
{
public:
	//!	Set the values of the transform - no alpha channel
	/*!	Do we need the _has parameters?
		Is 0 a valid value for each of these mult or add params?
	
	  \param	U32 _hasAdd:  Has color addition values if equal to 1
	  \param	U32 _hasMult:  Has color multipy values if equal to 1
	  \param	S32 _redMultTerm: Red multiply value
	  \param	S32 _greenMultTerm: Green multiply value
	  \param	S32 _blueMultiTerm: Blue multiply value
	  \param	S32 _redAddTerm: Red addition value
	  \param	S32 _greenAddTerm: Green addition value
	  \param	S32 _blueAddTerm: Blue addition value

	 */
	FCXForm(
		U32 _hasAdd, U32 _hasMult, S32 _redMultTerm, S32 _greenMultTerm,
		S32 _blueMultiTerm, S32 _redAddTerm, S32 _greenAddTerm, S32 _blueAddTerm);

	//!	Write a SWF Color Transform without Alpha to Stream
	/*!	Writes the _has bits, and depending on their values, the multiply and add
		color values. Computes the SWF nBits field from color values passed.
	
	  \param	_SWFStream	
	 */
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

protected:
	//flags
	U32 hasAdd;
	U32 hasMult;

	// bits in each term field
	U32 nBits;

	S32 redMultTerm;
	S32 greenMultTerm;
	S32 blueMultTerm;
	S32 redAddTerm;
	S32 greenAddTerm;
	S32 blueAddTerm;

	// finds minimum number of bits needed to represent the largest term
	U32 MinBits(void);
};

// Specifies a color transformation for some object with transparency information

//!	Set the values of the transform - same as FCXForm with alpha channel
/*!	Do we need the _has parameters?
		Is 0 a valid value for each of these mult or add params?
	
	  \param	U32 _hasAdd:  Has color addition values if equal to 1
	  \param	U32 _hasMult:  Has color multipy values if equal to 1
	  \param	S32 _redMultTerm: Red multiply value
	  \param	S32 _greenMultTerm: Green multiply value
	  \param	S32 _blueMultiTerm: Blue multiply value
	  \param	S32 _alphaMultTerm: Alpha transparency multiply value
	  \param	S32 _redAddTerm: Red addition value
	  \param	S32 _greenAddTerm: Green addition value
	  \param	S32 _blueAddTerm: Blue addition value
	  \param	S32 _alphaAddTerm: Alpha transparency addition value

	 */
class DVAPI FCXFormWAlpha : public FCXForm
{

public:
	FCXFormWAlpha(U32 _hasAdd, U32 _hasMult, S32 _redMultTerm, S32 _greenMultTerm, S32 _blueMultTerm, S32 _alphaMultTerm,
				  S32 _redAddTerm, S32 _greenAddTerm, S32 _blueAddTerm, S32 _alphaAddTerm);
	//!	Write a SWF Color Transform with Alpha to Stream
	/*!	Writes the _has bits, and depending on their values, the multiply and add
			color values. Computes the SWF nBits field from color values passed.
	
			  \param	_SWFStream	
		*/
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	//flags
	//	U32 hasAdd;
	//	U32 hasMult;

	//bits in each term field
	U32 nBits;

	//	S32 redMultTerm;
	//	S32 greenMultTerm;
	//	S32 blueMultTerm;
	S32 alphaMultTerm;

	//	S32 redAddTerm;
	//	S32 greenAddTerm;
	//	S32 blueAddTerm;
	S32 alphaAddTerm;

	//returns minimum number of bits needed to represent largest term value
	U32 MinBits(void);
};

//

//!	a flash object which defines a morphing shape.  It contains appearance information about an original shape and a shape being morphed into
/*!	Note: this is the SWF info and should be rewritten. I would have expected that a
	morph would take two character IDs instead of defining everything here. ???
	Defines the metamorphosis of one shape (Shape1) into another (Shape2). 
	The ShapeBounds1 specifies the boundaries of the original shape, 
	while ShapeBounds2 specifies the boundaries of the shape into which 
	Shape1 changes. The data specified in MorphFillStyles and MorphLineStyles 
	are for both shapes. For example, the fill style for Shape1 is specified, 
	then the corresponding fill style for Shape2 is specified. Edges1 
	specifies the edges for Shape1 as well as the morph style data for 
	both shapes. Edges2 specifies the edges for Shape2, and contains no 
	style data. The number of edges specified in Edges1 must equal the 
	number of edges in Edges2. 

	Should point to example code here. 	
 */

class DVAPI FDTDefineMorphShape : public FDT
{

public:
	//!	Constructor just takes the before and after bounds RECTs
	/*!	
		 \param	_rect1, _rect2 rects of before and after shapes
		*/
	FDTDefineMorphShape(FRect *_rect1, FRect *_rect2);
	virtual ~FDTDefineMorphShape();
	//!	Add Fill style to array and record its index
	/*!	 You should have used FMorphFillStyles (solid, gradient, bitmap)
			to create a morph fill style 
				\param	fillStyle
			*/
	U32 AddFillStyle(FMorphFillStyle *fillStyle);

	// here changed.
	//!	Add line style to array and record its index.
	/*!	 You need a pair of line styles (lineWidth + lineColor),
		     which contains both "morph from" and "morph to" line style information.
			\param	startLineWidth	Line thickness of starting shape in twips.
			\param	startLineColor	Line color of starting shape. (should be RGBA type).
			\param	finalLineWidth	Line thickness of ending shape in twips.
			\param	finalLineColor	Line color of ending shape. (should be RGBA type).
			\return	the position in the array.
			*/
	U32 AddLineStyle(U32 startLineWidth, FColor *startLineColor,
					 U32 finalLineWidth, FColor *finalLineColor);

	//!	Cleans up the internal representation of the fill and line style arrays.
	void FinishStyleArrays(void);

	//!	Add a Shape Record to the first morph object definition
	/*!	The AddShapeRec_1 method takes style information for both shapes and edge 
	information for the "morph from" shape
	  \param	_shapeRec the change, straight, curve record, or end record
	 */
	void AddShapeRec_1(FShapeRec *_shapeRec);
	//!	Add a Shape Record to the second morph object
	/*!	The AddEdgeRec_2 method takes edge information for just the "morph to" 
	  \param	_shapeRec the change, straight, curve record, or end record
	 */
	void AddEdgeRec_2(FShapeRec *_shapeRec);

	//!	Record morphing objects's ID for later reference
	U16 ID(void);

	//!	Called by the FObj to write to the output stream
	/*!		  \param	_SWFStream
	 */
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	U16 characterID;
	FRect *shapeBounds_1; // 1 subscript denotes original shape
	FRect *shapeBounds_2; // and 2 subscript denotes shape being morphed to
	FShape shape1;
	FShape shape2;
	U32 offset;
	FFillStyleArray *morphFillStyleArray;
	FLineStyleArray *morphLineStyleArray;
	U32 nFillBits;
	U32 nLineBits;
	U8 styleArraysFinished;
};

//!	Create a DefineShape tag
/*!	Creates a Flash 1.0 Define Shape tag. You should use DefineShape3.
	All the DefineShape tags only differ in the fill/line style arrays
	and fill/line style records 

  \sa	FFillStyle, FLineStyle, FShapeRec, FShapeWStyle, 
	see also FExampleRectangle.cpp	
 */
class DVAPI FDTDefineShape : public FDT
{

public:
	//!	Create the basic empty shell of a DefineShape tag
	/*!	Only the bounds rect is passed. Fills, Lines, and 
				Shapes must be added.
	
				\param	_rect the bounds rectangle
			*/
	FDTDefineShape(FRect *_rect);
	virtual ~FDTDefineShape();

	//!	Add a Shape record to the internal ShapeRec array
	/*!	Use FShapeRecChange, FShapeRecEdgeStraight, FShapeRecEdgeStraight
				to create edges to add to this shape
			\param	_shapeRec
			*/
	void AddShapeRec(FShapeRec *_shapeRec);

	//!	Cleans up the internal representation of the fill and line style arrays.
	void FinishStyleArrays(void);
	//!	Add fill style to shape,
	/*!	Remember to store the position of the fill style. Use this when 
				creating the shape change record later, to indicate which style
				to use for the fill.
				\param	fillStyle
				\return the position in the array.  when creating the shape change record later.
			*/
	U32 AddFillStyle(FFillStyle *fillStyle);

	// changed here.
	//!	Add solid fill style to shape,
	/*!	Remember to store the position of the fill style. Use this when 
				creating the shape change record later, to indicate which style
				to use for the fill.
				\param	fillColor
				\return the position in the array.  when creating the shape change record later.
			*/
	U32 AddSolidFillStyle(FColor *fillColor);

	// here changed.
	//!	Add line style to array and record its index.
	/*!	Remember to store the position of the fill style. Use this 
			when creating the shape change record later, to indicate which style
			to use for the next edges.
			\param	lineWidth	Line thickness in twips.
			\param	lineColor	Line color. (should be RGB type).
			\return the position in the array. 
		*/
	U32 AddLineStyle(U32 lineWidth, FColor *lineColor);

	//!	Record objects's ID for when you do a PlaceObject
	U16 ID(void);
	//!	Called by the FObj to write to the output stream
	/*!		  \param	_SWFStream
	 */
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);
	virtual void SetId(U16 id) { characterID = id; }
	bool IsDefineShape();

private:
	U16 characterID;
	FRect *shapeBounds;
	FShapeWStyle *shapeWithStyle;
	FFillStyleArray *fillStyleArray;
	FLineStyleArray *lineStyleArray;
	//U32 nFillBits;
	//U32 nLineBits;
	U8 styleArraysFinished;
};

//!	Create a DefineShape2 tag
/*!	Creates a Flash 2.0 Define Shape tag.  You should use DefineShape3.
	All the DefineShape tags only differ in the fill/line style arrays
	and fill/line style records 

  \sa	FFillStyle, FLineStyle, FShapeRec, FShapeWStyle, 
	see also FExampleRectangle.cpp	
 */
class DVAPI FDTDefineShape2 : public FDT
{

public:
	//!	Create the basic empty shell of a DefineShape tag
	/*!	Only the bounds rect is passed. Fills, Lines, and 
				Shapes must be added.
	
				\param	_rect the bounds rectangle
			*/
	FDTDefineShape2(FRect *_rect);
	virtual ~FDTDefineShape2();
	//!	Add a Shape record to the internal ShapeRec array
	/*!	Use FShapeRecChange, FShapeRecEdgeStraight, FShapeRecEdgeStraight
				to create edges to add to this shape
			\param	_shapeRec
			*/
	void AddShapeRec(FShapeRec *_shapeRec);

	//!	Cleans up the internal representation of the fill and line style arrays.
	void FinishStyleArrays(void);

	//!	Add fill style to shape,
	/*!	Remember to store the position of the fill style. Use this when 
			creating the shape change record later, to indicate which style
			to use for the fill. Remember, DefineShape2s don't support Alpha.
			\param	fillStyle
			\return the position in the array.  When creating the shape change record later.
		*/
	U32 AddFillStyle(FFillStyle *fillStyle);

	// here changed.
	//!	Add solid fill style to shape,
	/*!	Remember to store the position of the fill style. Use this when 
			creating the shape change record later, to indicate which style
			to use for the fill. Remember, DefineShape2s don't support Alpha.
			\param	fillColor
			\return the position in the array.  When creating the shape change record later.
		*/
	U32 AddSolidFillStyle(FColor *fillColor);

	// here changed.
	//!	Add line style to rectangle,
	/*!	Remember to store the position of the fill style. Use this 
			when creating the shape change record later, to indicate which style
			to use for the next edges.
			\param	lineWidth	Line thickness in twips.
			\param	lineColor	Line color. (should be RGB type).
			\return the position in the array. 
		*/
	U32 AddLineStyle(U32 lineWidth, FColor *lineColor);

	//!	Record objects's ID for when you do a PlaceObject
	U16 ID(void);
	//!	Called by the FObj to write to the output stream
	/*!		  \param	_SWFStream
	 */
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);
	virtual void SetId(U16 id) { characterID = id; }
	bool IsDefineShape();

private:
	U16 characterID;
	FRect *shapeBounds;
	FShapeWStyle *shapeWithStyle;
	FFillStyleArray *fillStyleArray;
	FLineStyleArray *lineStyleArray;
	U32 nFillBits;
	U32 nLineBits;
	U8 styleArraysFinished;
};

//!	Create a DefineShape3 tag
/*!	Creates a Flash 3.0 Define Shape tag. 
	All the DefineShape tags only differ in the fill/line style arrays
	and fill/line style records (accepts alpha color values).

  \sa	FFillStyle, FLineStyle, FShapeRec, FShapeWStyle, 
	see also FExampleRectangle.cpp	
 */

class DVAPI FDTDefineShape3 : public FDT
{

public:
	//!	Create the basic empty shell of a DefineShape tag
	/*!	Only the bounds rect is passed. Fills, Lines, and 
				Shapes must be added.
	
				\param	_rect the bounds rectangle
			*/
	FDTDefineShape3();
	FDTDefineShape3(FRect *_rect);
	void setBounds(FRect *_rect);
	virtual ~FDTDefineShape3();
	//!	Add a Shape record to the internal ShapeRec array
	/*!	Use FShapeRecChange, FShapeRecEdgeStraight, FShapeRecEdgeStraight
				to create edges to add to this shape
			\param	_shapeRec
			*/
	void AddShapeRec(FShapeRec *_shapeRec);
	//!	Cleans up the internal representation of the fill and line style arrays.
	void FinishStyleArrays(void);

	//!	Add fill style to shape,
	/*!	Remember to store the position of the fill style. Use this when 
			creating the shape change record later, to indicate which style
			to use for the fill. 
			\param	fillStyle
			\return the position in the array.  When creating the shape change record later.
		*/
	U32 AddFillStyle(FFillStyle *fillStyle);

	//!	Add solid fill style to shape,
	/*!	Remember to store the position of the fill style. Use this when 
			creating the shape change record later, to indicate which style
			to use for the fill. 
			\param	fillColor
			\return the position in the array.  When creating the shape change record later.
		*/
	U32 AddSolidFillStyle(FColor *fillColor);

	//!	Add line style to array and record its index.
	/*!	Remember to store the position of the fill style. Use this 
			when creating the shape change record later, to indicate which style
			to use for the next edges.
			\param	lineWidth	Line thickness in twips.
			\param	lineColor	Line color. (should be RGBA type).
			\return the position in the array. 
		*/
	U32 AddLineStyle(U32 lineWidth, FColor *lineColor);

	//!	Record objects's ID for when you do a PlaceObject
	U16 ID(void);
	//!	Called by the FObj to write to the output stream
	/*!		  \param	_SWFStream
	 */
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);
	virtual void SetId(U16 id) { characterID = id; }
	bool IsDefineShape();
	void changeColor(const FColor &oldColor, const FColor &newColor);
	void changeColor(const std::map<TPixel, TPixel> &table);

	FRect GetBBox() const { return *shapeBounds; }
private:
	U16 characterID;
	FRect *shapeBounds;
	FShapeWStyle *shapeWithStyle;
	FFillStyleArray *fillStyleArray;
	FLineStyleArray *lineStyleArray;
	U32 nFillBits;
	U32 nLineBits;
	U8 styleArraysFinished;
};

//!	Abstract Base Class. All edge records fall into this category
/*!	The fill classes from the solid, gradient, bitmap are children.

  \sa	FFillStyleGradient, FFillStyleBitmap, FFillStyleSolid
 */
class DVAPI FAFillStyle
{
public:
	virtual bool IsSolidStyle() { return false; };

	virtual ~FAFillStyle() {}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream) = 0;
};

//The fill style used by regular non-morph shapes

//!	Why does this exist?
/*!	It is used as a parameter to other functions but I don't know why.

  \sa	FFillStyleGradient, FFillStyleBitmap, FFillStyleSolid
 */
class DVAPI FFillStyle : public FAFillStyle
{
public:
	virtual ~FFillStyle() {}
};

//!An array of fill styles.
/*!	

  \sa	FFillStyleGradient, FFillStyleBitmap, FFillStyleSolid, FAFillStyle
*/
class DVAPI FFillStyleArray
{

public:
	FFillStyleArray(void) {}
	virtual ~FFillStyleArray(void);
	//!	The given fill style is added to the end of the fill style array.
	/*!	The position of the added fill style is returned so that the fill style can later be referenced.

			\param	fillStyle the style to add
			*/

	U32 Add(FAFillStyle *fillStyle);
	//!	Returns the size of the fill style list.
	U32 Size();
	FAFillStyle *get(unsigned int index);

	//!	Writes the array to the stream
	/*!	Travels through all of the nodes in the array and writing
		their fill styles.  First has to write the count of fill style arrays.  See's if count
		is small enough to fit in to an 8 bit field, and either writes the count into an 8 bit
		field, or writes all 1's into an 8 bit field and writes the real count into a 16 bit 
		field.  		
	
			\param	_SWFStream
		*/
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	//the list which contains all of the fill styles
	std::vector<FAFillStyle *> fillStyleArray;
};

//!	The Bitmap fill style used by regular non-morph shapes
/*!	Can be tiled or clipped, depending on the flag.

  \sa	FFillStyleArray, FFillStyleGradient, FFillStyleBitmap, FFillStyleSolid, FAFillStyle
 */
//
//

class DVAPI FFillStyleBitmap : public FFillStyle
{

public:
	/*!
	
			\param	tiled indicates if the Bitmap fill style is tiled (tiledFlag==1) or 
				clipped (tiledFlag==0).
			\param	matrix translation matrix for offsetting, rotating, scaleing etc. the bitmap
		*/
	FFillStyleBitmap(int tiled, U16 ID, FMatrix *matrix);
	virtual ~FFillStyleBitmap(void);
	//!	Writes the bitmap fill style to the given FSWFStream.
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	int tiledFlag;
	U16 bitmapID;
	FMatrix *bitmapMatrix;
};

//!	The Gradient fill style used by regular non-morph shapes
/*!	Can be linear or radial, depending on the flag.

  \sa	FFillStyleArray, FGradient
 */
class DVAPI FFillStyleGradient : public FFillStyle
{

public:
	//!	Create a Gradient fill style
	/*!	
			\param linear indicates if the gradient fill style is linear (linearFlag==1) or 
	radial (linearFlag==0).	
			\param	matrix matrix translation matrix for placing the gradient in the fill area
				Not sure what each of the matrix fields do???
			\param gradient a pre-build gradient object. See FGradient
		*/
	FFillStyleGradient(int linear, FMatrix *matrix, FGradient *gradient);
	virtual ~FFillStyleGradient(void);
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	int linearFlag;
	FMatrix *gradFillMatrix;
	FGradient *gradFill;
};

//The solid fill style used by regular non-morph shapes

//!	Creates a Solid Fill object
/*!	Basically just contains a color

  \sa FFillStyleArray, FFillStyleGradient, FFillStyleBitmap	
 */
class DVAPI FFillStyleSolid : public FFillStyle
{
public:
	//!	Create a style object
	/*!	Whether or not FColor contains alpha information is not 
				important until the color is written out.
	
			\param	Fcolor which can contain alpha or not
			*/
	FFillStyleSolid(FColor *_color);

	virtual ~FFillStyleSolid();

	//!	Write the color to the file stream
	/*!	When this object is serialized, it automatically handles the case of
				including the alpha information if its there.
			\param	*_SWFStream the output stream
			*/
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);
	bool IsSolidStyle() { return true; };
	void setColor(FColor *_color)
	{
		delete color;
		color = _color;
	}
	FColor *getColor() { return color; }

private:
	FColor *color;
};

//!	Gradient information is stored in an array of Gradient Records
/*!	This is that array. This style of gradient records is used with 
	non-morphing objects.

  \sa	FGradRecord
 */
class DVAPI FGradient
{

public:
	FGradient(void);
	~FGradient(void);
	//!	Add a Gradient Record to the internal list
	/*!	Up to 8 gradient records may be added to this list
				The ratio field within each GradientRec determines what percentage
				of a gradient is a particular color.
			\param	FAGradRecord*  pointer to a gradient record
			*/
	void Add(FAGradRecord *g);
	void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	//the list which contains all of the grad records
	std::list<FAGradRecord *> gradRecords;
};

//	used by non-morph shapes

//!	A non morph gradient record.
/*!	Note that ostensibly the ratio parameter is the percentage complete aTLong 
	the gradient that this color should be but it's really not known what this
	field is.
  \sa	FGradient, FFillStyleGradient
 */
class DVAPI FGradRecord : public FAGradRecord
{
public:
	//!Create the Gradient record with the mysterious ration parm.
	/*!	
		
		  \param U_ratio know one knows for sure
		  \param FColor one of the colors to shift through
		 */
	FGradRecord(U32 _ratio, FColor *color);

	virtual ~FGradRecord();

	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	U32 ratio;
	FColor *color;
};

//

//!	The line style used by regular non-morph shapes
/*!	LineStyles are held in arrays of LineStyles which are held
	in DefineShape Tags.   

  \sa	FExampleRectangle.cpp, FLineStyleArray
 */
class DVAPI FLineStyle : public FALineStyle
{

public:
	//!Line Style is just a thickness and color
	/*!	
		
		  \param	width U32
		  \param	_color FColor	
		 */
	FLineStyle(U32 _width, FColor *_color);
	virtual ~FLineStyle();
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	U32 width;
	FColor *color;
};

//
//!	An array of line styles.
/*!	LineStyles are held in arrays of LineStyles which are held
	in DefineShape Tags.   

  \sa	FLineStyle, FExampleRectangle.cpp
*/
class DVAPI FLineStyleArray
{

public:
	//!Create an empty initialized array
	FLineStyleArray(void);
	~FLineStyleArray(void);
	//!Add the line style to the array.
	/*!	Automatically handles the case of packing the corresponding SWF object
			properly when you go over 255 styles.  Not that we can see why you'd do
			this.
		
		  \param		lineStyle the style object to add to the internal array
		 */
	U32 Add(FALineStyle *lineStyle);
	//! The number of items in the list
	/*! This is needed when a containing DefineShape needs to know how how
	many styles there are and therefore how many bits are needed for an index.
		The given line style is added to the end of the line style array.  The position of 
	the added line style is returned so that the line style can later be referenced.
	*/
	U32 Size(void);
	//! Write the array to the output stream.
	/*!		Writes to the stream by travelling through all of the nodes in 
		the array and writing their line styles.  First has to write the count 
		of line style arrays.  Sees if count is small enough to fit in to an 
		8 bit field, and either writes the count into an 8 bit field, or writes 
		all 1's into an 8 bit field and writes the real count into a 16 bit field.
		
		  \param	*_SWFStream	
		 */
	void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	//the list which contains all of the line styles
	std::list<FALineStyle *> lineStyleArray;
};

//!	The Base class of fill style objects used by morph shapes
/*!	

  \sa	FMorphFillStyleGradient, FMorphFillStyleBitmap, FMorphFillStyleSolid
 */
class DVAPI FMorphFillStyle : public FAFillStyle
{
public:
	virtual ~FMorphFillStyle() {}
	virtual void WriteToSWFStream(FSWFStream * /* _SWFStream */) {}
};

//
//

//!The Bitmap fill style used by morph shapes
/*!	Note that all Morph fill styles are documented in SWF as a single overloaded
		data structure.  It's simplier to have three separate routiens.
		
		This can be tiled or clipped, depending on the flag.
	
	\sa	FMorphFillStyleGradient, FMorphFillStyleBitmap, FMorphFillStyleSolid, FMatrix
	 */
class DVAPI FMorphFillStyleBitmap : public FMorphFillStyle
{

public:
	//	The tiledFlag indicates if the Bitmap fill style is tiled (tiledFlag==1) or
	//	clipped (tiledFlag==0).

	//! Bitmap fill style for a morph
	/*!	
		
		  \param tiled	The tiledFlag indicates if the Bitmap fill style is tiled (tiledFlag==1) or 
	clipped (tiledFlag==0)	
		\param ID Character ID of bitmap
		\param matrix1 rotation/translation Matrix for first state
		\param matrix2 for the second state
		 */
	FMorphFillStyleBitmap(int tiled, U16 ID, FMatrix *matrix1, FMatrix *matrix2);
	virtual ~FMorphFillStyleBitmap(void);
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	int tiledFlag;
	U16 bitmapID;
	FMatrix *bitmapMatrix1;
	FMatrix *bitmapMatrix2;
};

//!The Gradient fill style used by morph shapes
/*!	Note that all Morph fill styles are documented in SWF as a single overloaded
		data structure.  It's simplier to have three separate routines.
		
		Can be linear or radial, depending on the flag, depending on the flag.
	
	\sa	FMorphFillStyleGradient, FMorphFillStyleBitmap, FMorphFillStyleSolid, FGradient, FMatrix, FFillStyleArray
	 */
class DVAPI FMorphFillStyleGradient : public FMorphFillStyle
{

public:
	//!Construct a Gradient Fill Style
	/*!	Ultimatly to go in a MorphFillStyles Array and then get 
			added to a DefineMorphShape
			
			  \param		linear The linearFlag indicates if the gradient fill 
			  style is linear (linearFlag==1) or radial (linearFlag==0)
			  \param matrix1
			  \param matrix2 translation matrix for the gradient before and after.
			  \param gradient an FGradient fill
			*/
	FMorphFillStyleGradient(int linear, FMatrix *matrix1, FMatrix *matrix2, FGradient *gradient);
	virtual ~FMorphFillStyleGradient(void);
	//!Writes the Gradient fill style to the given FSWFStream.
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	int linearFlag;
	FMatrix *gradFillMatrix1;
	FMatrix *gradFillMatrix2;
	FGradient *gradFill;
};

//!The solid fill style used by morph shapes
/*!	Each Fill style has two colors for the two objects being morphed.  The FillStyles are
in a FFillStyle array.  In this case the fill styles object 1 and 2 are interleaved. The
FillStyleArray is then stored in a DefineMorphShape.

  \sa FMorphFillStyleGradient, FMorphFillStyleBitmap, FMorphFillStyleSolid, FGradient, FMatrix,FFillStyleArray 	
 */
class DVAPI FMorphFillStyleSolid : public FMorphFillStyle
{

public:
	/*!	
		  \param _color1 Solid fill color with transparency information for first shape	
		  \param _color2 Same for second
		 */
	FMorphFillStyleSolid(FColor *_color1, FColor *_color2);
	virtual ~FMorphFillStyleSolid();
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	FColor *color1;
	FColor *color2;
};

//!The grad record used by morph shapes
/*!	The Morph Gradient store an array of these records.  Each is like a double
of the regular Gradient fill record.  Each Morph Gradient Record has two colors 
and two ratio numbers for the two objects being morphed.  Theses Gradient Records 
possibly 8 of them, are stored in an FAGradient array, the base of all gradient records. 
Normally the Colors in a gradient array store the colors aTLong the gradient.  These
store pairs of colors, one for each of the two objects aTLong with the mysterious ratio number.
 The FMorphFillStyleGradient is then stored in a DefineMorphShape.

  \sa FMorphFillStyleGradient, FMorphFillStyleBitmap, FMorphFillStyleSolid, FAGradient, FMatrix,FFillStyleArray 	
 */
class DVAPI FMorphGradRecord : public FAGradRecord
{

public:
	//!Create a Morph Gradient record
	/*!	
		  \param	_ratio1 for the first object
		  \param	_color1
		  \param	_ratio2 for the second object
		  \param	_color2
		 */
	FMorphGradRecord(U32 _ratio1, FColor *_color1, U32 _ratio2, FColor *_color2);
	virtual ~FMorphGradRecord();

	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	U32 ratio1;
	U32 ratio2;

	FColor *color1;
	FColor *color2;
};

//!	Morph line styles to go in the DefineMorph Object
/*!	MorphLineStyle is an FALineStyle.  These get put into 
	FLineStyleArrays which in turn are put in the Define Morph Object.
	These are just line regular line styles, except that they 
	carry a pair of line styles, one for the morph from object
	and the corresponding one in the Morphe to object.

  \sa	FLineStyleArray
 */
class FMorphLineStyle : public FALineStyle
{

public:
	//!create the pair of styles, width/color
	/*!	
		
		  \param width1
		  \param width2
		  \param _color1
		  \param _color2

		 */
	FMorphLineStyle(U32 _width1, U32 _width2, FColor *_color1, FColor *_color2);
	virtual ~FMorphLineStyle();
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);

private:
	FColor *color1;
	FColor *color2;
	U32 width1;
	U32 width2;
};

//!	Virtual Base class for the change, straight, and curved Shape records
/*!	

  \sa 	FShapeRecChange, FShapeRecEdgeStraight, FShapeRecEdgeCurved
 */
class DVAPI FShapeRec
{

public:
	virtual ~FShapeRec() {}
	virtual bool isFShapeRecChange() { return false; }
	virtual void WriteToSWFStream(FSWFStream *_SWFStream) = 0;
	virtual void IncludeNFillBitInfo(U32 _nFillBits) = 0;
	virtual void IncludeNLineBitInfo(U32 _nLineBits) = 0;
	virtual bool IsRecChange() { return false; }
};

//	shape record that defines changes in line style, fill style, position, or a new set of styles

class DVAPI FShapeRecChange : public FShapeRec
{

public:
	FShapeRecChange(U32 _stateNewStyles,
					U32 _stateLineStyle,
					U32 _stateFillStyle1,
					U32 _stateFillStyle0,
					U32 _stateMoveTo,
					S32 _moveDeltaX,
					S32 _moveDeltaY,
					U32 _fill0Style,
					U32 _fill1Style,
					U32 _lineStyle,
					FFillStyleArray *_fillStyles,
					FLineStyleArray *_lineStyles);

	virtual ~FShapeRecChange(void);

	virtual bool isFShapeRecChange() { return true; }
	void getFillLineBits(U32 &_nFillBits, U32 &_nLineBits)
	{
		_nFillBits = nFillBits;
		_nLineBits = nLineBits;
	}
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);
	virtual void IncludeNFillBitInfo(U32 _nFillBits);
	virtual void IncludeNLineBitInfo(U32 _nLineBits);
	bool IsRecChange() { return true; }
	FFillStyleArray *GetFillStyles();

private:
	U32 stateNewStyles;
	U32 stateLineStyle;
	U32 stateFillStyle0;
	U32 stateFillStyle1;
	U32 stateMoveTo;
	U32 nMoveBits;
	U32 nFillBits; // these two values are stored in the Shape field
	U32 nLineBits; // they are passed to each ShapeRec
	S32 moveDeltaX;
	S32 moveDeltaY;
	U32 fill0Style;
	U32 fill1Style;
	U32 lineStyle;
	FFillStyleArray *fillStyles;
	FLineStyleArray *lineStyles;
	U32 MinBits(void);
};

//!	Create a straight edge
/*!	The XY values are passed as delta values from the previous XY points.
	Creating and adding several straight edges in a row will create a set
	of connected edges.

	This may change to absolution positions in the future. ???

	See also FExampleRectangle.cpp	
	
 */
class DVAPI FShapeRecEdgeStraight : public FShapeRec
{
public:
	//!	Create a straight edge from the previous point
	/*!	This is the equivalent of a "LineTo" command
	
			\param	generalDX
			\param generalDY
			*/
	FShapeRecEdgeStraight(S32 dx, S32 dy);
	virtual ~FShapeRecEdgeStraight(){};

	//!Write out as Vertical, Horizontal or general line SWF Record automatically
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);
	//!dummy in this object.  This call only makes sense for a Change Rec, here for historical
	virtual void IncludeNFillBitInfo(U32 _nFillBits);
	//!dummy in this object.  This call only makes sense for a Change Rec, here for historical
	virtual void IncludeNLineBitInfo(U32 _nLineBits);

private:
	FlareStraightEdge edge; // flags, deltas and nBits (computed by flare_fedge_straight_new)
	U32 MinBits(void);
};

//!	Create a bezier edge
/*!	The control points are passed as delta values from the previous control points.
	This may change to absolution positions in the future. ???

	See also [example file], [bezier info file]
	
 */
class DVAPI FShapeRecEdgeCurved : public FShapeRec
{
public:
	//!	Create a bezier
	/*!	All values expressed in twips ???
			\param	controlDX delta from the last X control value
			\param  controlDY delta from the last Y control value 
			\param  anchorDX delta from the last X anchor value
			\param  anchorDY delta from the last Y anchor value
			*/
	FShapeRecEdgeCurved(S32 controlDX, S32 controlDY, S32 anchorDX, S32 anchorDY);
	virtual ~FShapeRecEdgeCurved(void) {}
	//!	Called by containing Shape when file is serialized
	/*!	Writes the anchor, control, pts, as well as the minBits and edgeFlag	  
	\param	_SWFStream
	 */
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);
	//!dummy in this object.  This call only makes sense for a Change Rec, here for historical
	virtual void IncludeNFillBitInfo(U32 _nFillBits);
	//!dummy in this object.  This call only makes sense for a Change Rec, here for historical
	virtual void IncludeNLineBitInfo(U32 _nLineBits);

private:
	U32 nBits;
	S32 controlDeltaX;
	S32 controlDeltaY;
	S32 anchorDeltaX;
	S32 anchorDeltaY;
	U32 MinBits(void);
};

//The shape record which signifies the end of a shape record array.

class DVAPI FShapeRecEnd : public FShapeRec
{

public:
	FShapeRecEnd(void);
	//!Write an EndRecord.  Signifies the last ShapeRecord in the ShapeRecArray
	virtual void WriteToSWFStream(FSWFStream *_SWFStream);
	//!dummy in this object.  This call only makes sense for a Change Rec, here for historical
	virtual void IncludeNFillBitInfo(U32 _nFillBits);
	//!dummy in this object.  This call only makes sense for a Change Rec, here for historical
	virtual void IncludeNLineBitInfo(U32 _nLineBits);
};

class DVAPI FShapeWStyle
{

public:
	FShapeWStyle(FFillStyleArray *_fillStyles, FLineStyleArray *_lineStyles);
	~FShapeWStyle(void);
	void AddShapeRec(FShapeRec *shapeRec);
	void WriteToSWFStream(FSWFStream *_SWFStream);
	U32 NumFillBits();
	U32 NumLineBits();
	FFillStyleArray *GetFillStyles() { return fillStyles; }
	void changeColor(const FColor &oldColor, const FColor &newColor);
	void changeColor(const std::map<TPixel, TPixel> &table);

private:
	FFillStyleArray *fillStyles;
	FLineStyleArray *lineStyles;
	U32 nFillBits;
	U32 nLineBits;
	std::vector<FShapeRec *> shapeRecs; //see comments, will need to be changed
};


//=============================================================================
//   Member definitions (FDTShapes.cpp was ported to FDTShapes.rs)
//=============================================================================

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FCXForm ----------------------------------------------------------------

inline FCXForm::FCXForm(U32 _hasAdd, U32 _hasMult, S32 _redMultTerm, S32 _greenMultTerm, S32 _blueMultTerm, S32 _redAddTerm, S32 _greenAddTerm, S32 _blueAddTerm)
{
	hasAdd = _hasAdd;
	hasMult = _hasMult;
	redMultTerm = _redMultTerm;
	greenMultTerm = _greenMultTerm;
	blueMultTerm = _blueMultTerm;
	redAddTerm = _redAddTerm;
	greenAddTerm = _greenAddTerm;
	blueAddTerm = _blueAddTerm;
	nBits = MinBits();
}

//
inline U32 FCXForm::MinBits()
{

	return flare_fcxform_min_bits(redMultTerm, greenMultTerm, blueMultTerm, redAddTerm, greenAddTerm, blueAddTerm);
}

//
inline void FCXForm::WriteToSWFStream(FSWFStream *_SWFStream)
{

	flare_fshapes_check(flare_fcxform_write(_SWFStream->Handle(), hasAdd, hasMult, nBits, redMultTerm, greenMultTerm,
											blueMultTerm, redAddTerm, greenAddTerm, blueAddTerm));
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FCXFormWAlpha ----------------------------------------------------------

inline FCXFormWAlpha::FCXFormWAlpha(U32 _hasAdd, U32 _hasMult, S32 _redMultTerm, S32 _greenMultTerm, S32 _blueMultTerm, S32 _alphaMultTerm,
							 S32 _redAddTerm, S32 _greenAddTerm, S32 _blueAddTerm, S32 _alphaAddTerm)
	: FCXForm(_hasAdd, _hasMult, _redMultTerm, _greenMultTerm, _blueMultTerm,
			  _redAddTerm, _greenAddTerm, _blueAddTerm)
{
	alphaMultTerm = _alphaMultTerm;

	alphaAddTerm = _alphaAddTerm;

	nBits = MinBits();
}

inline U32 FCXFormWAlpha::MinBits()
{

	return flare_fcxform_alpha_min_bits(redMultTerm, greenMultTerm, blueMultTerm, alphaMultTerm,
										redAddTerm, greenAddTerm, blueAddTerm, alphaAddTerm);
}

inline void FCXFormWAlpha::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//fill? I think so (the flush comes first here, not last as in FCXForm)
	flare_fshapes_check(flare_fcxform_alpha_write(_SWFStream->Handle(), hasAdd, hasMult, nBits, redMultTerm, greenMultTerm,
												  blueMultTerm, alphaMultTerm, redAddTerm, greenAddTerm, blueAddTerm,
												  alphaAddTerm));
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FDTDefineMorphShape ----------------------------------------------------

inline FDTDefineMorphShape::FDTDefineMorphShape(FRect *_rect1, FRect *_rect2)
{

	characterID = FObjCollection::Increment();
	shapeBounds_1 = _rect1;
	shapeBounds_2 = _rect2;
	morphFillStyleArray = new FFillStyleArray();
	morphLineStyleArray = new FLineStyleArray();
	styleArraysFinished = false;
}

inline FDTDefineMorphShape::~FDTDefineMorphShape()
{

	delete shapeBounds_1;
	delete shapeBounds_2;
	delete morphFillStyleArray;
	delete morphLineStyleArray;
}

inline U32 FDTDefineMorphShape::AddFillStyle(FMorphFillStyle *fillStyle)
{

	assert(!styleArraysFinished);
	return morphFillStyleArray->Add(fillStyle);
}

inline U32 FDTDefineMorphShape::AddLineStyle(U32 startLineWidth, FColor *startLineColor,
									  U32 finalLineWidth, FColor *finalLineColor)
{
	assert(!styleArraysFinished);

	// here changed.
	// morph shape always has RGBA color.
	startLineColor->AlphaChannel(true);
	finalLineColor->AlphaChannel(true);

	// Create a morph line style (one which contains both "morph from" and "morph to" line style information
	FMorphLineStyle *startToFinalLine = new FMorphLineStyle(startLineWidth, finalLineWidth,
															startLineColor, finalLineColor);
	return morphLineStyleArray->Add(startToFinalLine);
}

inline void FDTDefineMorphShape::FinishStyleArrays(void)
{

	styleArraysFinished = true;
	nFillBits = FSWFStream::MinBits(morphFillStyleArray->Size(), 0);
	nLineBits = FSWFStream::MinBits(morphLineStyleArray->Size(), 0);

	shape1.SetFillBits(nFillBits);
	shape1.SetLineBits(nLineBits);

	// asumption: shape2 will never contain fill or line syle info and this means no need
	// for fill or line style bits
}

inline void FDTDefineMorphShape::AddShapeRec_1(FShapeRec *_shapeRec)
{

	assert(styleArraysFinished);
	shape1.AddShapeRec(_shapeRec);
}

inline void FDTDefineMorphShape::AddEdgeRec_2(FShapeRec *_shapeRec)
{

	assert(styleArraysFinished);
	_shapeRec->IncludeNFillBitInfo(0); // Always Zero
	_shapeRec->IncludeNLineBitInfo(0);
	shape2.AddShapeRec(_shapeRec);
}

inline U16 FDTDefineMorphShape::ID(void)
{

	return characterID;
}

inline void FDTDefineMorphShape::WriteToSWFStream(FSWFStream *_SWFStream)
{
	FSWFStream body;

	flare_fshapes_check(flare_fdt_shape_head(body.Handle(), characterID));

	shapeBounds_1->WriteToSWFStream(&body);

	shapeBounds_2->WriteToSWFStream(&body);

	FSWFStream temp;

	morphFillStyleArray->WriteToSWFStream(&temp);
	morphLineStyleArray->WriteToSWFStream(&temp);

	shape1.WriteToSWFStream(&temp);

	flare_fshapes_check(flare_fdt_morph_join(body.Handle(), temp.Handle()));

	shape2.WriteToSWFStream(&body);

	flare_fshapes_check(flare_fdt_shape_finish(_SWFStream->Handle(), FLARE_SHAPE_TAG_MORPH, body.Handle()));
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FDTDefineShape ---------------------------------------------------------

inline FDTDefineShape::FDTDefineShape(FRect *_rect)
{

	characterID = FObjCollection::Increment();
	shapeBounds = _rect;
	fillStyleArray = new FFillStyleArray();
	lineStyleArray = new FLineStyleArray();
	shapeWithStyle = NULL;
	styleArraysFinished = false;
}

inline FDTDefineShape::~FDTDefineShape()
{

	delete fillStyleArray;
	delete lineStyleArray;
	delete shapeBounds;
	delete shapeWithStyle;
}

inline void FDTDefineShape::AddShapeRec(FShapeRec *_shapeRec)
{

	//you must be done creating the fill style and line style arrays
	//before you can begin adding shape records
	assert(styleArraysFinished);

	//Change rec doesn't know how many bits for saving the FillStyle0
	//and FillStyle1.  It gets this from the ShapeWithStyle which contains
	//it.  What happens when the change Rec also contains Fill and Line
	//Syles???  The other types of ShapeRecs just use the "current style"
	//and don't need this info so their methods don't actually do anything.
	//Why not just go to the container when writing?
	_shapeRec->IncludeNFillBitInfo(shapeWithStyle->NumFillBits());
	_shapeRec->IncludeNLineBitInfo(shapeWithStyle->NumLineBits());
	//now have the shapeWithStyle add the ShapeRec
	shapeWithStyle->AddShapeRec(_shapeRec);
}

//This shapes internal fillStyleArray adds a fillStyle to itself.
inline U32 FDTDefineShape::AddFillStyle(FFillStyle *fillStyle)
{

	assert(!styleArraysFinished); //once complete, cannot change
	return fillStyleArray->Add(fillStyle);
}
// here changed.
inline U32 FDTDefineShape::AddSolidFillStyle(FColor *fillColor)
{
	fillColor->AlphaChannel(false);
	FFillStyle *fillStyle = new FFillStyleSolid(fillColor);

	assert(!styleArraysFinished); //once complete, cannot change
	return fillStyleArray->Add(fillStyle);
}

inline U32 FDTDefineShape::AddLineStyle(U32 lineWidth, FColor *lineColor)
{

	assert(!styleArraysFinished); //once complete, cannot change

	// here changed.
	lineColor->AlphaChannel(false);
	FLineStyle *lineStyle = new FLineStyle(lineWidth, lineColor);

	//add line style to rectangle, remembering to store the position of the line style
	//just as in the fill style
	return lineStyleArray->Add(lineStyle);
}

/*!	Creates the shape with style.  Can only do this once the fill style
	and line style arrays are complete because the shapeWithStyle constructor
	takes in complete fill and line style arrays. 
*/
inline void FDTDefineShape::FinishStyleArrays(void)
{

	styleArraysFinished = true;
	shapeWithStyle = new FShapeWStyle(fillStyleArray, lineStyleArray);
	fillStyleArray = NULL; /*AMM*/
	lineStyleArray = NULL;
}

inline U16 FDTDefineShape::ID(void)
{

	return characterID;
}

//Called by the FObjCollection when writing to the stream
//In turn, it calls the WriteToSWFStream methods of its embedded objects.

inline void FDTDefineShape::WriteToSWFStream(FSWFStream *_SWFStream)
{
	FSWFStream tempBuffer;

	flare_fshapes_check(flare_fdt_shape_head(tempBuffer.Handle(), characterID));
	shapeBounds->WriteToSWFStream(&tempBuffer);
	shapeWithStyle->WriteToSWFStream(&tempBuffer);

	flare_fshapes_check(flare_fdt_shape_finish(_SWFStream->Handle(), FLARE_SHAPE_TAG_SHAPE, tempBuffer.Handle()));
}

inline bool FDTDefineShape::IsDefineShape()
{
	return true;
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FDTDefineShape2 --------------------------------------------------------
inline FDTDefineShape2::FDTDefineShape2(FRect *_rect)
{

	characterID = FObjCollection::Increment();
	shapeBounds = _rect;
	fillStyleArray = new FFillStyleArray();
	lineStyleArray = new FLineStyleArray();
	shapeWithStyle = NULL;
	styleArraysFinished = false;
}

inline FDTDefineShape2::~FDTDefineShape2()
{

	delete fillStyleArray;
	delete lineStyleArray;
	delete shapeBounds;
	delete shapeWithStyle;
}

inline void FDTDefineShape2::AddShapeRec(FShapeRec *_shapeRec)
{

	//you must be done creating the fill style and line style arrays
	//before you can begin adding shape records
	assert(styleArraysFinished);

	_shapeRec->IncludeNFillBitInfo(shapeWithStyle->NumFillBits());
	_shapeRec->IncludeNLineBitInfo(shapeWithStyle->NumLineBits());
	shapeWithStyle->AddShapeRec(_shapeRec);
}

inline U32 FDTDefineShape2::AddFillStyle(FFillStyle *fillStyle)
{

	assert(!styleArraysFinished); //once complete, cannot change
	return fillStyleArray->Add(fillStyle);
}
// here changed.
inline U32 FDTDefineShape2::AddSolidFillStyle(FColor *fillColor)
{
	fillColor->AlphaChannel(false);
	FFillStyle *fillStyle = new FFillStyleSolid(fillColor);

	assert(!styleArraysFinished); //once complete, cannot change
	return fillStyleArray->Add(fillStyle);
}

inline U32 FDTDefineShape2::AddLineStyle(U32 lineWidth, FColor *lineColor)
{

	assert(!styleArraysFinished); //once complete, cannot change

	// here changed.
	lineColor->AlphaChannel(false);
	FLineStyle *lineStyle = new FLineStyle(lineWidth, lineColor);

	//add line style to rectangle, remembering to store the position of the line style
	//just as in the fill style
	return lineStyleArray->Add(lineStyle);
}

//	Creates the shape with style.  Can only do this once the fill style
//	and line style arrays are complete because the shapeWithStyle constructor
//	takes in complete fill and line style arrays.

inline void FDTDefineShape2::FinishStyleArrays(void)
{

	styleArraysFinished = true;
	shapeWithStyle = new FShapeWStyle(fillStyleArray, lineStyleArray);
	fillStyleArray = NULL; /*AMM*/
	lineStyleArray = NULL;
}

inline U16 FDTDefineShape2::ID(void)
{

	return characterID;
}

inline bool FDTDefineShape2::IsDefineShape()
{
	assert(false);
	return true;
}

inline void FDTDefineShape2::WriteToSWFStream(FSWFStream *_SWFStream)
{
	FSWFStream body;

	flare_fshapes_check(flare_fdt_shape_head(body.Handle(), characterID));
	shapeBounds->WriteToSWFStream(&body);
	shapeWithStyle->WriteToSWFStream(&body);

	flare_fshapes_check(flare_fdt_shape_finish(_SWFStream->Handle(), FLARE_SHAPE_TAG_SHAPE2, body.Handle()));
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FDTDefineShape3 --------------------------------------------------------
inline FDTDefineShape3::FDTDefineShape3(FRect *_rect)
{
	characterID = FObjCollection::Increment();
	shapeBounds = _rect;
	fillStyleArray = new FFillStyleArray();
	lineStyleArray = new FLineStyleArray();
	shapeWithStyle = NULL;
	styleArraysFinished = false;
}

//////////////////////////////////////////////////////////////////////////////////////

inline FDTDefineShape3::FDTDefineShape3()
{

	characterID = FObjCollection::Increment();
	shapeBounds = 0;
	fillStyleArray = new FFillStyleArray();
	lineStyleArray = new FLineStyleArray();
	shapeWithStyle = NULL;
	styleArraysFinished = false;
}

inline FDTDefineShape3::~FDTDefineShape3()
{

	delete fillStyleArray;
	delete lineStyleArray;
	delete shapeBounds;
	delete shapeWithStyle;
}

inline void FDTDefineShape3::setBounds(FRect *_rect)
{
	if (shapeBounds)
		delete shapeBounds;
	shapeBounds = _rect;
}

//////////////////////////////////////////////////////////////////////////////////////

inline bool FDTDefineShape3::IsDefineShape()
{
	return true;
}

//////////////////////////////////////////////////////////////////////////////////////

inline FColor tpixel2fcolor(const TPixel &color)
{
	return FColor(color.r, color.g, color.b, color.m);
}

//////////////////////////////////////////////////////////////////////////////////////

inline TPixel fcolor2tpixel(FColor &color)
{
	return TPixel(color.Red(), color.Green(), color.Blue(), color.Alpha());
}

//////////////////////////////////////////////////////////////////////////////////////

inline void FDTDefineShape3::changeColor(const std::map<TPixel, TPixel> &table)
{
	unsigned int i;
	if (fillStyleArray)
		for (i = 0; i < fillStyleArray->Size(); i++) {
			FAFillStyle *style = fillStyleArray->get(i);
			if (style->IsSolidStyle()) {
				TPixel oldColor = fcolor2tpixel(*(((FFillStyleSolid *)style)->getColor()));
				std::map<TPixel, TPixel>::const_iterator it = table.find(oldColor);
				if (it != table.end())
					((FFillStyleSolid *)style)->setColor(new FColor(tpixel2fcolor(it->second)));
			}
		}

	shapeWithStyle->changeColor(table);
}

//////////////////////////////////////////////////////////////////////////////////////

inline void FDTDefineShape3::changeColor(const FColor &oldColor, const FColor &newColor)
{
	unsigned int i;
	if (fillStyleArray)
		for (i = 0; i < fillStyleArray->Size(); i++) {
			FAFillStyle *style = fillStyleArray->get(i);
			if (style->IsSolidStyle() && (*((FFillStyleSolid *)style)->getColor()) == oldColor)
				((FFillStyleSolid *)style)->setColor(new FColor(newColor));
		}

	shapeWithStyle->changeColor(oldColor, newColor);
}

//////////////////////////////////////////////////////////////////////////////////////

inline void FDTDefineShape3::AddShapeRec(FShapeRec *_shapeRec)
{

	//you must be done creating the fill style and line style arrays
	//before you can begin adding shape records
	assert(styleArraysFinished);

	_shapeRec->IncludeNFillBitInfo(shapeWithStyle->NumFillBits());
	_shapeRec->IncludeNLineBitInfo(shapeWithStyle->NumLineBits());
	shapeWithStyle->AddShapeRec(_shapeRec);
}

inline U32 FDTDefineShape3::AddFillStyle(FFillStyle *fillStyle)
{

	assert(!styleArraysFinished); //once complete, cannot change
	return fillStyleArray->Add(fillStyle);
}

// here changed.
inline U32 FDTDefineShape3::AddSolidFillStyle(FColor *fillColor)
{
	fillColor->AlphaChannel(true);
	FFillStyle *fillStyle = new FFillStyleSolid(fillColor);

	assert(!styleArraysFinished); //once complete, cannot change
	return fillStyleArray->Add(fillStyle);
}

inline U32 FDTDefineShape3::AddLineStyle(U32 lineWidth, FColor *lineColor)
{

	assert(!styleArraysFinished); //once complete, cannot change

	// here changed.
	lineColor->AlphaChannel(true);
	FLineStyle *lineStyle = new FLineStyle(lineWidth, lineColor);

	//add line style to rectangle, remembering to store the position of the line style
	//just as in the fill style
	return lineStyleArray->Add(lineStyle);
}

//	Creates the shape with style.  Can only do this once the fill style
//	and line style arrays are complete because the shapeWithStyle constructor
//	takes in complete fill and line style arrays.

inline void FDTDefineShape3::FinishStyleArrays(void)
{

	styleArraysFinished = true;
	shapeWithStyle = new FShapeWStyle(fillStyleArray, lineStyleArray);
	fillStyleArray = NULL;
	lineStyleArray = NULL;
}

inline U16 FDTDefineShape3::ID(void)
{

	return characterID;
}

inline void FDTDefineShape3::WriteToSWFStream(FSWFStream *_SWFStream)
{
	FSWFStream body;

	flare_fshapes_check(flare_fdt_shape_head(body.Handle(), characterID));
	shapeBounds->WriteToSWFStream(&body);
	shapeWithStyle->WriteToSWFStream(&body);

	flare_fshapes_check(flare_fdt_shape_finish(_SWFStream->Handle(), FLARE_SHAPE_TAG_SHAPE3, body.Handle()));
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FFillStyleArray --------------------------------------------------------

inline FFillStyleArray::~FFillStyleArray()
{
	while (!fillStyleArray.empty()) {
		delete fillStyleArray.back();
		fillStyleArray.pop_back();
	}
}

//	Returns the size of the fill style list.
inline U32 FFillStyleArray::Size(void)
{
	return (U32)fillStyleArray.size();
}

//	The given fill style is added to the end of the fill style array.  The position of
//  the added fill style is returned so that the fill style can later be referenced.

inline U32 FFillStyleArray::Add(FAFillStyle *fillStyle)
{
	FLASHASSERT(fillStyle);

	fillStyleArray.push_back(fillStyle);
	return ((U32)fillStyleArray.size());
}

//////////////////////////////////////////////////////////////////////////////////////

inline FAFillStyle *FFillStyleArray::get(unsigned int index)
{
	return fillStyleArray[index];
}

//	Writes to the stream by travelling through all of the nodes in the array and writing
//  their fill styles.  First has to write the count of fill style arrays.  See's if count
//  is small enough to fit in to an 8 bit field, and either writes the count into an 8 bit
//  field, or writes all 1's into an 8 bit field and writes the real count into a 16 bit
//  field.

inline void FFillStyleArray::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//write the size
	U32 size = fillStyleArray.size();

	flare_fshapes_check(flare_fstyle_array_count(_SWFStream->Handle(), size));

	//write the fill styles
	std::vector<FAFillStyle *>::iterator cursor;

	for (cursor = fillStyleArray.begin(); cursor != fillStyleArray.end(); cursor++) {
		(*cursor)->WriteToSWFStream(_SWFStream);
	}
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FFillStyleBitmap -------------------------------------------------------

//	The tiledFlag indicates if the Bitmap fill style is tiled (tiledFlag==1) or
//	clipped (tiledFlag==0).
inline FFillStyleBitmap::FFillStyleBitmap(int tiled, U16 ID, FMatrix *matrix)
{

	tiledFlag = tiled;
	bitmapID = ID;
	bitmapMatrix = matrix;
}

//	Deletes the matrix.

inline FFillStyleBitmap::~FFillStyleBitmap(void)
{

	delete bitmapMatrix;
	bitmapMatrix = NULL;
}

//	Writes the bitmap fill style to the given FSWFStream.

inline void FFillStyleBitmap::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//write the type and the bitmap id
	flare_fshapes_check(flare_ffill_style_head(_SWFStream->Handle(), FLARE_FILL_KIND_BITMAP, tiledFlag, bitmapID));

	//write the matrix
	bitmapMatrix->WriteToSWFStream(_SWFStream);
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FFillStyleGradient -----------------------------------------------------

//	The linearFlag indicates if the gradient fill style is linear (linearFlag==1) or
//	radial (linearFlag==0).
inline FFillStyleGradient::FFillStyleGradient(int linear, FMatrix *matrix, FGradient *gradient)
{
	linearFlag = linear;
	gradFillMatrix = matrix;
	gradFill = gradient;
}

//	Deletes the matrix and gradient.

inline FFillStyleGradient::~FFillStyleGradient(void)
{
	delete gradFillMatrix;
	delete gradFill;
}

//	Writes the Gradient fill style to the given FSWFStream.

inline void FFillStyleGradient::WriteToSWFStream(FSWFStream *_SWFStream)
{
	//write the type
	flare_fshapes_check(flare_ffill_style_head(_SWFStream->Handle(), FLARE_FILL_KIND_GRADIENT, linearFlag, 0));

	//write the matrix
	gradFillMatrix->WriteToSWFStream(_SWFStream);

	//write the gradient
	gradFill->WriteToSWFStream(_SWFStream);
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FFillStyleSolid --------------------------------------------------------

//	Stores the color of the solid fill style.
inline FFillStyleSolid::FFillStyleSolid(FColor *_color) : color(_color) {}

inline FFillStyleSolid::~FFillStyleSolid()
{
	delete color;
}

//	Writes the solid fill style to the given FSWFStream.

inline void FFillStyleSolid::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//write the type
	flare_fshapes_check(flare_ffill_style_head(_SWFStream->Handle(), FLARE_FILL_KIND_SOLID, 0, 0));

	//write the color
	color->WriteToSWFStream(_SWFStream); //, FColor::WRITE_SMALLEST ); changed here.
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FGradient --------------------------------------------------------------

//	Constructor.  The grad record list is automatically constructed.
inline FGradient::FGradient(void) {}

//	Removes and deletes all the grad records from the grad record list.

inline FGradient::~FGradient(void)
{

	while (!gradRecords.empty()) {

		delete gradRecords.front();

		gradRecords.pop_front();
	}
}

//	Adds the given grad record to the end of the grad record list.

inline void FGradient::Add(FAGradRecord *gradRec)
{

	gradRecords.push_back(gradRec);
}

//	Writes the grad records to the given _SWFStream.

inline void FGradient::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//write the size

	flare_fshapes_check(flare_fgradient_head(_SWFStream->Handle(), (U32)gradRecords.size()));

	//write the grad records

	std::list<FAGradRecord *>::iterator cursor;

	for (cursor = gradRecords.begin(); cursor != gradRecords.end(); cursor++) {

		(*cursor)->WriteToSWFStream(_SWFStream);
	}
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FGradientRecord --------------------------------------------------------

//	FGradRecord class constructor.
inline FGradRecord::FGradRecord(U32 _ratio, FColor *_color)
{
	color = _color;
	ratio = _ratio;
}

inline FGradRecord::~FGradRecord()
{
	delete color;
}

//	Writes the grad record to the given buffer.

inline void FGradRecord::WriteToSWFStream(FSWFStream *_SWFStream)
{

	flare_fshapes_check(flare_fgrad_ratio(_SWFStream->Handle(), ratio));

	color->WriteToSWFStream(_SWFStream); //, FColor::WRITE_SMALLEST ); changed here.
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FLineStyle -------------------------------------------------------------

//	LineStyle class constructor.
inline FLineStyle::FLineStyle(U32 _width, FColor *_color)
{
	color = _color;
	width = _width; //in TWIPS
}

inline FLineStyle::~FLineStyle()
{
	delete color;
}

//	Writes the object to the given _SWFStream.

inline void FLineStyle::WriteToSWFStream(FSWFStream *_SWFStream)
{

	flare_fshapes_check(flare_fline_style_head(_SWFStream->Handle(), width));
	color->WriteToSWFStream(_SWFStream); //, FColor::WRITE_SMALLEST ); changed here.
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FLineStyleArray --------------------------------------------------------

//	The line style array is automatically constructed
inline FLineStyleArray::FLineStyleArray(void) {}

//	Removes and deletes every element in the list.

inline FLineStyleArray::~FLineStyleArray(void)
{

	while (!lineStyleArray.empty()) {

		delete lineStyleArray.front();
		lineStyleArray.pop_front();
	}
}

//  Returns the size of the line style list.

inline U32 FLineStyleArray::Size(void)
{

	return (U32)lineStyleArray.size();
}

//	The given line style is added to the end of the line style array.  The position of
//	the added line style is returned so that the line style can later be referenced.

inline U32 FLineStyleArray::Add(FALineStyle *lineStyle)
{

	lineStyleArray.push_back(lineStyle);
	return ((U32)lineStyleArray.size());
}

//	Writes to the stream by travelling through all of the nodes in the array and writing
//  their line styles.  First has to write the count of line style arrays.  See's if count
//  is small enough to fit in to an 8 bit field, and either writes the count into an 8 bit
//  field, or writes all 1's into an 8 bit field and writes the real count into a 16 bit
//  field.

inline void FLineStyleArray::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//write the size
	U32 size = lineStyleArray.size();

	flare_fshapes_check(flare_fstyle_array_count(_SWFStream->Handle(), size));

	//write the line styles
	std::list<FALineStyle *>::iterator cursor;

	for (cursor = lineStyleArray.begin(); cursor != lineStyleArray.end(); cursor++) {

		(*cursor)->WriteToSWFStream(_SWFStream);
	}
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FMorphFillStyleBitmap --------------------------------------------------

//	The tiledFlag indicates if the Bitmap fill style is tiled (tiledFlag==1) or
//	clipped (tiledFlag==0).
inline FMorphFillStyleBitmap::FMorphFillStyleBitmap(int tiled, U16 ID,
											 FMatrix *matrix1, FMatrix *matrix2)
{

	tiledFlag = tiled;
	bitmapID = ID;
	bitmapMatrix1 = matrix1;
	bitmapMatrix2 = matrix2;
}

//	Deletes the matrices.

inline FMorphFillStyleBitmap::~FMorphFillStyleBitmap(void)
{

	delete bitmapMatrix1;
	bitmapMatrix1 = NULL;

	delete bitmapMatrix2;
	bitmapMatrix2 = NULL;
}

//	Writes the bitmap fill style to the given FSWFStream.

inline void FMorphFillStyleBitmap::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//write the type and the bitmap id
	flare_fshapes_check(flare_ffill_style_head(_SWFStream->Handle(), FLARE_FILL_KIND_BITMAP, tiledFlag, bitmapID));

	//write the matrices
	bitmapMatrix1->WriteToSWFStream(_SWFStream);
	bitmapMatrix2->WriteToSWFStream(_SWFStream);
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FMorphFillStyleGradient ------------------------------------------------

//	The linearFlag indicates if the gradient fill style is linear (linearFlag==1) or
//	radial (linearFlag==0).
inline FMorphFillStyleGradient::FMorphFillStyleGradient(int linear, FMatrix *matrix1, FMatrix *matrix2,
												 FGradient *gradient)
{

	linearFlag = linear;
	gradFillMatrix1 = matrix1;
	gradFillMatrix2 = matrix2;
	gradFill = gradient;
}

//	Deletes the matrices and gradient.

inline FMorphFillStyleGradient::~FMorphFillStyleGradient(void)
{

	delete gradFillMatrix1;
	gradFillMatrix1 = NULL;
	delete gradFillMatrix2;
	gradFillMatrix2 = NULL;
	delete gradFill;
	gradFill = NULL;
}

//	Writes the Gradient fill style to the given FSWFStream.

inline void FMorphFillStyleGradient::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//write the type
	flare_fshapes_check(flare_ffill_style_head(_SWFStream->Handle(), FLARE_FILL_KIND_GRADIENT, linearFlag, 0));

	//write the matrices
	gradFillMatrix1->WriteToSWFStream(_SWFStream);
	gradFillMatrix2->WriteToSWFStream(_SWFStream);

	//write the gradient
	gradFill->WriteToSWFStream(_SWFStream);
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FMorphFillStyleSolid ---------------------------------------------------

//	Stores the colors of the solid fill style.
inline FMorphFillStyleSolid::FMorphFillStyleSolid(FColor *_color1, FColor *_color2)
{
	color1 = _color1;
	color2 = _color2;
}

inline FMorphFillStyleSolid::~FMorphFillStyleSolid()
{
	delete color1;
	delete color2;
}

//	Writes the solid fill style to the given FSWFStream.

inline void FMorphFillStyleSolid::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//write the type
	flare_fshapes_check(flare_ffill_style_head(_SWFStream->Handle(), FLARE_FILL_KIND_SOLID, 0, 0));

	//write the colors
	color1->WriteToSWFStream(_SWFStream); //, FColor::WRITE_SMALLEST ); changed here.
	color2->WriteToSWFStream(_SWFStream); //, FColor::WRITE_SMALLEST ); changed here.
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FMorphGradRecord -------------------------------------------------------

//	Constructor.
inline FMorphGradRecord::FMorphGradRecord(U32 _ratio1, FColor *_color1, U32 _ratio2, FColor *_color2)
{

	ratio1 = _ratio1;
	ratio2 = _ratio2;

	color1 = _color1;
	color2 = _color2;
}

inline FMorphGradRecord::~FMorphGradRecord()
{
	delete color1;
	delete color2;
}

//  Writes to the given _SWFStream.

inline void FMorphGradRecord::WriteToSWFStream(FSWFStream *_SWFStream)
{

	flare_fshapes_check(flare_fgrad_ratio(_SWFStream->Handle(), ratio1));
	color1->WriteToSWFStream(_SWFStream); //, FColor::WRITE_SMALLEST ); changed here.

	flare_fshapes_check(flare_fgrad_ratio(_SWFStream->Handle(), ratio2));
	color2->WriteToSWFStream(_SWFStream); //, FColor::WRITE_SMALLEST ); changed here.
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FMorphLineStyle --------------------------------------------------------

//The line style used by morph shapes
inline FMorphLineStyle::FMorphLineStyle(U32 _width1, U32 _width2, FColor *_color1,
								 FColor *_color2) : color1(_color1), color2(_color2)
{
	width1 = _width1;
	width2 = _width2;

	color1 = _color1;
	color2 = _color2;
}

inline FMorphLineStyle::~FMorphLineStyle()
{
	delete color1;
	delete color2;
}

//	Writes to the given _SWFStream.

inline void FMorphLineStyle::WriteToSWFStream(FSWFStream *_SWFStream)
{
	//write the widths
	flare_fshapes_check(flare_fmorph_line_style_head(_SWFStream->Handle(), width1, width2));

	//write the colors
	color1->WriteToSWFStream(_SWFStream); //, FColor::WRITE_SMALLEST ); changed here.
	color2->WriteToSWFStream(_SWFStream); //, FColor::WRITE_SMALLEST ); changed here.
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FShape -----------------------------------------------------------------

//	Shape class constructor.  The shape record list is automatically constructed
inline FShape::FShape(void)
{

	nFillBits = 0;
	nLineBits = 0;
}

//	Removes and deletes every element in the list.

inline FShape::~FShape(void)
{

	while (!shapeRecs.empty()) {

		delete shapeRecs.back();
		shapeRecs.pop_back();
	}
}

//	Sets the nFillBits field.

inline void FShape::SetFillBits(U32 _nFillBits)
{

	nFillBits = _nFillBits;
}

//	Sets the nLineBits field.

inline void FShape::SetLineBits(U32 _nLineBits)
{

	nLineBits = _nLineBits;
}

//	Adds a shape record to the end of the shape record list.

inline void FShape::AddShapeRec(FShapeRec *shapeRec)
{

	shapeRecs.push_back(shapeRec);
}

//	Writes the object to the given buffer.

inline void FShape::WriteToSWFStream(FSWFStream *_SWFStream)
{

	flare_fshapes_check(flare_fshape_head(_SWFStream->Handle(), nFillBits, nLineBits));

	std::vector<FShapeRec *>::iterator cursor;

	for (cursor = shapeRecs.begin(); cursor != shapeRecs.end(); cursor++) {
		//the shape rec might be referring to a fillor line style arry
		//external (ShapesWStyle) and will need the # of fillBits.
		(*cursor)->IncludeNFillBitInfo(nFillBits); //enter fillbits
		(*cursor)->IncludeNLineBitInfo(nLineBits); //enter linebits
		(*cursor)->WriteToSWFStream(_SWFStream);
	}
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FShapeRecChange --------------------------------------------------------

//	FShapeRecChange class constructor.  It is passed the nFillBits and nLineBits values.
inline FShapeRecChange::FShapeRecChange(U32 _stateNewStyles,
								 U32 _stateLineStyle,
								 U32 _stateFillStyle1,
								 U32 _stateFillStyle0,
								 U32 _stateMoveTo,
								 S32 _moveDeltaX,
								 S32 _moveDeltaY,
								 U32 _fill0Style,
								 U32 _fill1Style,
								 U32 _lineStyle,
								 FFillStyleArray *_fillStyles,
								 FLineStyleArray *_lineStyles)
{
	stateNewStyles = _stateNewStyles;
	stateLineStyle = _stateLineStyle;
	stateFillStyle1 = _stateFillStyle1;
	stateFillStyle0 = _stateFillStyle0;
	stateMoveTo = _stateMoveTo;

	moveDeltaX = _moveDeltaX;
	moveDeltaY = _moveDeltaY;
	fill0Style = _fill0Style;
	fill1Style = _fill1Style;
	lineStyle = _lineStyle;
	fillStyles = _fillStyles;
	lineStyles = _lineStyles;
	nMoveBits = MinBits();
}

inline FFillStyleArray *FShapeRecChange::GetFillStyles()
{
	return fillStyles;
}

//	Deletes fillStyles and lineStyles if they exist.

inline FShapeRecChange::~FShapeRecChange(void)
{

	if (fillStyles) { //if fillStyles isn't NULL

		delete fillStyles;
		fillStyles = NULL;
	}

	if (lineStyles) { //if lineStyles isn't NULL

		delete lineStyles;
		lineStyles = NULL;
	}
}

//!Change Rec needs to know how many bits to write for the Fill and Line Styles
/*!Change Rec doesn't actually store the nLineBits or nFillBits, but needs to know
	them when it writes the active style fields so that it knows how many bits
	to write
	\param	_nFillBits the size in bits of the index into the FillStyleArray
*/
inline void FShapeRecChange::IncludeNFillBitInfo(U32 _nFillBits)
{

	nFillBits = _nFillBits;
}

//!Change Rec needs to know how many bits to write for the Fill and Line Styles
/*!Change Rec doesn't actually store the nLineBits or nFillBits, but needs to know
	them when it writes the active style fields so that it knows how many bits
	to write
	\param	_nLineBits the size in bits of the index into the LineStyleArray
*/
inline void FShapeRecChange::IncludeNLineBitInfo(U32 _nLineBits)
{

	nLineBits = _nLineBits;
}

//	Calculates nMoveBits by returning the number of bits needed to store the larger of
//	moveDeltaX and moveDeltaY.

inline U32 FShapeRecChange::MinBits(void)
{

	return flare_fshaperec_change_min_bits(moveDeltaX, moveDeltaY);
}

//	Writes the shape record to the given _SWFStream.

inline void FShapeRecChange::WriteToSWFStream(FSWFStream *_SWFStream)
{

	// the non-edge record flag, the five state flags, the move delta and the active styles
	flare_fshapes_check(flare_fshaperec_change_head(_SWFStream->Handle(), stateNewStyles, stateLineStyle, stateFillStyle1,
													stateFillStyle0, stateMoveTo, nMoveBits, moveDeltaX, moveDeltaY,
													fill0Style, fill1Style, lineStyle, nFillBits, nLineBits));

	if (stateNewStyles) {

		fillStyles->WriteToSWFStream(_SWFStream);
		lineStyles->WriteToSWFStream(_SWFStream);

		// the widths of the new arrays, written (4 + 4 bits) and kept as the current ones
		// ("dimenticavano" di mettere questi...)
		flare_fshapes_check(flare_fshaperec_change_new_styles(_SWFStream->Handle(), fillStyles->Size(), lineStyles->Size(),
															  &nFillBits, &nLineBits));
	}

	//NO FILLING IN BETWEEN SHAPE RECORDS
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FShapeRecEdgeStraight --------------------------------------------------

inline FShapeRecEdgeStraight::FShapeRecEdgeStraight(S32 dx, S32 dy)
{
	// classifies the line (general / vertical / horizontal) and derives nBits
	flare_fshapes_check(flare_fedge_straight_new(dx, dy, &edge));
	assert(edge.n_bits < 16 + 2); //vincenzo: se no non entra nei quattro bits riservati per scrivere questo valore...
}

inline U32 FShapeRecEdgeStraight::MinBits(void)
{

	return FSWFStream::MinBits(FSWFStream::MaxNum(edge.general_delta_x, edge.general_delta_y,
												  edge.horizontal_delta_x, edge.vertical_delta_y), 1);
}

inline void FShapeRecEdgeStraight::IncludeNFillBitInfo(U32 /*_nFillBits*/)
{
}

inline void FShapeRecEdgeStraight::IncludeNLineBitInfo(U32 /*_nLineBits*/)
{
}

inline void FShapeRecEdgeStraight::WriteToSWFStream(FSWFStream *_SWFStream)
{
	// edge record, straight edge, nBits - 2, then a general / vertical / horizontal line
	// (verticalFlag is supposed to be signed but don't think it matters)
	flare_fshapes_check(flare_fedge_straight_write(_SWFStream->Handle(), &edge));

	//NO FILLING IN BETWEEN SHAPE RECORDS
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FShapeRecEdgeCurved  ---------------------------------------------------

//	FShapeRecEdge class constructor.
inline FShapeRecEdgeCurved::FShapeRecEdgeCurved(S32 controlDX, S32 controlDY, S32 anchorDX, S32 anchorDY)
{
	controlDeltaX = controlDX;
	controlDeltaY = controlDY;
	anchorDeltaX = anchorDX;
	anchorDeltaY = anchorDY;

	nBits = MinBits();
}
//	Finds the min bits necessary to represent the 4 fields, by seeing how many bits are
//	necessary to represent the largest field of the four.

inline U32 FShapeRecEdgeCurved::MinBits(void)
{

	return flare_fedge_curved_min_bits(controlDeltaX, controlDeltaY, anchorDeltaX, anchorDeltaY);
}

inline void FShapeRecEdgeCurved::IncludeNFillBitInfo(U32 /*_nFillBits*/)
{
}

inline void FShapeRecEdgeCurved::IncludeNLineBitInfo(U32 /*_nLineBits*/)
{
}

//	Writes the shape record to the given _SWFStream.

inline void FShapeRecEdgeCurved::WriteToSWFStream(FSWFStream *_SWFStream)
{
	// edge record, curved edge, nBits - 2, then control and anchor deltas
	flare_fshapes_check(flare_fedge_curved_write(_SWFStream->Handle(), nBits, controlDeltaX, controlDeltaY, anchorDeltaX,
												 anchorDeltaY));

	//NO FILLING IN BETWEEN SHAPE RECORDS
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FShapeRecEnd  ----------------------------------------------------------

//	ShapeRecEnd class constructor.  Doesn't take in anything because the
//	object serves as an end tag and has no details.
inline FShapeRecEnd::FShapeRecEnd(void) {}

inline void FShapeRecEnd::IncludeNFillBitInfo(U32 /*_nFillBits*/)
{
	// does nothing
	//virtual method needed for shape rec change
}

inline void FShapeRecEnd::IncludeNLineBitInfo(U32 /*_nLineBits*/)
{
	//same deal
}

//	Writes the object to the given buffer.

inline void FShapeRecEnd::WriteToSWFStream(FSWFStream *_SWFStream)
{

	//stream of 0's signifies the end; filled to end the shape rec array structure.
	flare_fshapes_check(flare_fshaperec_end_write(_SWFStream->Handle()));
}

//////////////////////////////////////////////////////////////////////////////////////
//  --------  FShapeWStyle  ----------------------------------------------------------

//	FShapeWStyle class constructor.  The shape record list is automatically constructed.
inline FShapeWStyle::FShapeWStyle(FFillStyleArray *_fillStyles, FLineStyleArray *_lineStyles)
{

	fillStyles = _fillStyles;
	lineStyles = _lineStyles;

	nFillBits = FSWFStream::MinBits(fillStyles->Size(), 0);
	nLineBits = FSWFStream::MinBits(lineStyles->Size(), 0);
}

//	Deleted the fill style array, line style array, and shape records.

inline FShapeWStyle::~FShapeWStyle(void)
{

	delete fillStyles;
	fillStyles = NULL;

	delete lineStyles;
	lineStyles = NULL;

	while (!shapeRecs.empty()) {

		delete shapeRecs.back();
		shapeRecs.pop_back();
	}
}

//	Adds a shape record to the end of the shape record list.

inline void FShapeWStyle::AddShapeRec(FShapeRec *shapeRec)
{

	shapeRecs.push_back(shapeRec);
}

inline U32 FShapeWStyle::NumFillBits()
{

	return nFillBits;
}

inline U32 FShapeWStyle::NumLineBits()
{

	return nLineBits;
}

//////////////////////////////////////////////////////////////////////

inline void FShapeWStyle::changeColor(const std::map<TPixel, TPixel> &table)
{
	unsigned int i, j;
	if (fillStyles)
		for (i = 0; i < fillStyles->Size(); i++) {
			FAFillStyle *style = fillStyles->get(i);
			//FColor* color = ((FFillStyleSolid*)style)->getColor();
			if (style->IsSolidStyle()) {
				TPixel oldColor = fcolor2tpixel(*(((FFillStyleSolid *)style)->getColor()));
				std::map<TPixel, TPixel>::const_iterator it = table.find(oldColor);
				if (it != table.end())
					((FFillStyleSolid *)style)->setColor(new FColor(tpixel2fcolor(it->second)));
			}
		}

	for (i = 0; i < shapeRecs.size(); i++) {
		FShapeRec *rec = shapeRecs[i];
		if (rec->isFShapeRecChange() && ((FShapeRecChange *)rec)->GetFillStyles())
			for (j = 0; j < ((FShapeRecChange *)rec)->GetFillStyles()->Size(); j++) {
				FAFillStyle *style = ((FShapeRecChange *)rec)->GetFillStyles()->get(j);
				//FColor* color = ((FFillStyleSolid*)style)->getColor();
				if (style->IsSolidStyle()) {
					TPixel oldColor = fcolor2tpixel(*(((FFillStyleSolid *)style)->getColor()));
					std::map<TPixel, TPixel>::const_iterator it = table.find(oldColor);
					if (it != table.end())
						((FFillStyleSolid *)style)->setColor(new FColor(tpixel2fcolor(it->second)));
				}
			}
	}
}

/////////////////////////////////////////////////////////////////////

inline void FShapeWStyle::changeColor(const FColor &oldColor, const FColor &newColor)
{
	unsigned int i, j;
	if (fillStyles)
		for (i = 0; i < fillStyles->Size(); i++) {
			FAFillStyle *style = fillStyles->get(i);
			FColor *color = ((FFillStyleSolid *)style)->getColor();
			if (style->IsSolidStyle() && *color == oldColor)
				((FFillStyleSolid *)style)->setColor(new FColor(newColor));
			color = ((FFillStyleSolid *)style)->getColor();
		}

	for (i = 0; i < shapeRecs.size(); i++) {
		FShapeRec *rec = shapeRecs[i];
		if (rec->isFShapeRecChange() && ((FShapeRecChange *)rec)->GetFillStyles())
			for (j = 0; j < ((FShapeRecChange *)rec)->GetFillStyles()->Size(); j++) {
				FAFillStyle *style = ((FShapeRecChange *)rec)->GetFillStyles()->get(j);
				FColor *color = ((FFillStyleSolid *)style)->getColor();
				if (style->IsSolidStyle() && *color == oldColor)
					((FFillStyleSolid *)style)->setColor(new FColor(newColor));
				color = ((FFillStyleSolid *)style)->getColor();
			}
	}
}

/////////////////////////////////////////////////////////////////////

//	Writes the shape with style to the given _SWFStream.

inline void FShapeWStyle::WriteToSWFStream(FSWFStream *_SWFStream)
{

	fillStyles->WriteToSWFStream(_SWFStream);
	lineStyles->WriteToSWFStream(_SWFStream);

	flare_fshapes_check(flare_fshape_head(_SWFStream->Handle(), nFillBits, nLineBits));

	//write the shape records
	std::vector<FShapeRec *>::iterator cursor;

	for (cursor = shapeRecs.begin(); cursor != shapeRecs.end(); cursor++) {
		(*cursor)->IncludeNFillBitInfo(nFillBits); //vincenzo
		(*cursor)->IncludeNLineBitInfo(nLineBits); //vincenzo

		(*cursor)->WriteToSWFStream(_SWFStream);
		if ((*cursor)->isFShapeRecChange())										   //vincenzo
			((FShapeRecChange *)(*cursor))->getFillLineBits(nFillBits, nLineBits); //vincenzo
	}
	_SWFStream->FlushBits();
}

#ifdef WIN32 // added from DV
#pragma warning(pop)
#endif
#endif
