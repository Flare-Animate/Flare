#include "tflash.h"
//#include "tstroke.h"
#include "tcurves.h"
#include "tregion.h"
#include "tstrokeprop.h"
#include "tregionprop.h"

#include "tpalette.h"
#include "tvectorimage.h"
#include "tmachine.h"
#include "trasterimage.h"
#include "tsimplecolorstyles.h"
#include "tcolorfunctions.h"
#include "F3SDK.h"
#include "FFixed.h"
#include "tsop.h"
#include "tropcm.h"
#include "tsweepboundary.h"
#include "tiio_jpg_util.h"
#include "zlib.h"
//#include "trop.h"
#include "tflareimage.h"
#include "tconvert.h"
#include "timage_io.h"
#include "tsystem.h"
#include <stack>
#include <fstream>
#include <algorithm>

// Macros for switch-case syntax
#define CASE case
#define __OR case
#define DEFAULT default

#if !defined(TNZ_LITTLE_ENDIAN)
TNZ_LITTLE_ENDIAN undefined !!
#endif

	int Tw = 0;

bool areTwEqual(double x, double y)
{
	assert(Tw != 0);

	return (int)(Tw * x) == (int)(Tw * y);
}

bool areTwEqual(TPointD p0, TPointD p1)
{
	assert(Tw != 0);

	return areTwEqual(p0.x, p1.x) && areTwEqual(p0.y, p1.y);
}
//-------------------------------------------------------------------

const std::wstring TFlash::ConstantLines = L"Low: Constant Thickness";
const std::wstring TFlash::MixedLines = L"Medium: Mixed Thickness";
const std::wstring TFlash::VariableLines = L"High: Variable Thickness";

Tiio::SwfWriterProperties::SwfWriterProperties()
	: m_lineQuality("Curve Quality"), m_isCompressed("File Compression", true), m_autoplay("Autoplay", true), m_looping("Looping", true), m_jpgQuality("Jpg Quality", 0, 100, 90), m_url("URL", std::wstring()), m_preloader("Insert Preloader", false)
{
	m_lineQuality.addValue(TFlash::MixedLines);
	m_lineQuality.addValue(TFlash::ConstantLines);
	m_lineQuality.addValue(TFlash::VariableLines);

	bind(m_lineQuality);
	bind(m_isCompressed);
	bind(m_autoplay);
	bind(m_looping);
	bind(m_jpgQuality);
	bind(m_url);
	bind(m_preloader);

	TEnumProperty::Range range = m_lineQuality.getRange();
}

//-------------------------------------------------------------------

enum PolyType { None,
			Centerline,
			Solid,
			Texture,
			LinearGradient,
			RadialGradient };

class PolyStyle
{
public:
	PolyType m_type;
	TPixel32 m_color1;	//only if type!=Texture
	TPixel32 m_color2;	//only if type==LinearGradient || type==RadialGradient
	double m_smooth;	  //only if type==RadialGradient
	double m_thickness;   //only if type==Centerline
	TAffine m_matrix;	 //only if type==Texture
	TRaster32P m_texture; //only if type==Texture
	//bool m_isRegion;            //only if type!=Centerline
	//bool m_isHole;              //only if type!=Centerline && m_isRegion==true
	PolyStyle() : m_type(None), m_color1(), m_color2(), m_smooth(0), m_thickness(0), m_matrix(), m_texture() /*, m_isRegion(false), m_isHole(false)*/ {}
	bool operator==(const PolyStyle &p) const;
	bool operator<(const PolyStyle &p) const;
};

class FlashPolyline
{
public:
	UINT m_depth;
	bool m_skip;
	bool m_toBeDeleted;
	bool m_isPoint;
	std::vector<TQuadratic *> m_quads;
	PolyStyle m_fillStyle1;
	PolyStyle m_fillStyle2;
	PolyStyle m_lineStyle;
	//PolyStyle m_bgStyle;
	FlashPolyline() : m_depth(0), m_skip(false), m_toBeDeleted(false), m_isPoint(false), m_fillStyle1(), m_fillStyle2(), m_lineStyle() {}
	bool operator<(const FlashPolyline &p) const { return m_depth < p.m_depth; }
};

class biPoint
{
public:
	TPointD p0, p1;

	biPoint(TPointD _p0, TPointD _p1) : p0(_p0), p1(_p1) {}
	biPoint() {}
	bool operator<(const biPoint &b) const
	{
		biPoint aux;
		aux.p0.x = areTwEqual(p0.x, b.p0.x) ? p0.x : b.p0.x;
		aux.p0.y = areTwEqual(p0.y, b.p0.y) ? p0.y : b.p0.y;
		aux.p1.x = areTwEqual(p1.x, b.p1.x) ? p1.x : b.p1.x;
		aux.p1.y = areTwEqual(p1.y, b.p1.y) ? p1.y : b.p1.y;

		return (p0.x == aux.p0.x) ? ((p0.y == aux.p0.y) ? ((p1.x == aux.p1.x) ? (p1.y < aux.p1.y) : (p1.x < aux.p1.x)) : (p0.y < aux.p0.y)) : p0.x < aux.p0.x;
	}
	void revert() { std::swap(p0, p1); }
};

class wChunk
{
public:
	double w0, w1;
	wChunk(double _w0, double _w1) : w0(_w0), w1(_w1) {}
	bool operator<(const wChunk &b) const { return (w1 < b.w0); }
};

//-------------------------------------------------------------------

const int c_soundRate = 5512; //  5512; //11025
const int c_soundBps = 16;
const bool c_soundIsSigned = false;
const int c_soundChannelNum = 1;
const int c_soundCompression = 3; //per compatibilita' con MAC!!!

//-------------------------------------------------------------------

class FlashImageData
{
public:
	FlashImageData(TAffine aff, TImageP img, const TColorFunction *cf, bool isMask, bool isMasked)
		: m_aff(aff), m_img(img), m_cf(cf), m_isMask(isMask), m_isMasked(isMasked)
	{
		assert(!isMask || !isMasked);
	}
	TAffine m_aff;
	const TColorFunction *m_cf;
	bool m_isMask, m_isMasked;
	TImageP m_img;
};

class FlashColorStyle
{
public:
	TPixel m_color;
	double m_thickness;
	U32 m_id;
	FlashColorStyle(TPixel color, double thickness, U32 id)
		: m_color(color), m_thickness(thickness), m_id(id) {}
};

class TFlash::Imp
{
public:
	//double m_totMem;
	bool m_supportAlpha;
	int m_tw;
	UCHAR m_version;
	SCOORD m_sCoord1;
	bool m_loaderAdded;
	TAffine m_globalScale;
	//typedef triple FlashImageData;
	typedef std::vector<FlashImageData> FrameData;
	FObjCollection m_tags;
	FDTSprite *m_currSprite;
	int m_currDepth;
	int m_frameRate;
	int m_currFrameIndex;
	int m_lx, m_ly;
	//ouble cameradpix, cameradpiy, inchFactor;
	const TPalette *m_currPalette;
	int m_soundRate;
	Tiio::SwfWriterProperties m_properties;

	bool m_maskEnabled;
	bool m_isMask;

	bool m_keepImages;

	std::list<FlashPolyline> m_polylinesArray;
	//std::set<Polyline> m_currentEdgeArray;

	TPixel32 m_lineColor;
	double m_thickness;

	PolyStyle m_polyData;
	//std::vector<PolyStyle> m_currentBgStyle;
	int m_regionDepth;
	int m_strokeCount;
	/*TPixel32 m_fillColor;
  TAffine m_fillMatrix;
  TRaster32P m_texture;
  GradientType m_gradientType;
  TPixel32 m_gradientColor1, m_gradientColor2;*/
	//std::ofstream m_of;

	TAffine m_affine;
	std::vector<TAffine> m_matrixStack;
	std::map<const TImage *, USHORT> m_imagesMap;
	std::map<const TImage *, double> m_imagesScaleMap;
	std::map<TEdge, FlashPolyline *> m_edgeMap;
	std::map<const void *, USHORT> m_texturesMap;
	std::map<biPoint, FlashPolyline *> m_autocloseMap;
	std::map<const TStroke *, std::set<wChunk>> m_strokeMap;

	std::vector<TStroke *> m_outlines;
	TPixel m_currStrokeColor;
	//std::set<TPixel> m_outlineColors;

	FrameData *m_frameData;
	FrameData *m_oldFrameData;
	//bool m_notClipped;
	std::vector<TSoundTrackP> m_sound;
	int m_soundSize;
	std::vector<UCHAR *> m_soundBuffer;
	int m_soundOffset;
	TVectorImageP m_currMask;

	std::vector<std::vector<UCHAR> *> m_toBeDeleted;
	std::vector<TQuadratic *> m_quadsToBeDeleted;
	std::vector<TStroke *> m_strokesToBeDeleted;
	void drawPolygon(const std::vector<TQuadratic *> &poly, bool isOutline);
	int setFill(FDTDefineShape3 *shape);
	inline FMatrix *affine2Matrix(const TAffine &aff);
	void drawHangedObjects();
	void setStyles(const std::list<FlashPolyline> &polylines,
			   std::vector<U32> &lineStyleID, std::vector<U32> &fillStyle1ID, std::vector<U32> &fillStyle2ID,
			   FDTDefineShape3 *polygon);

	U32 findStyle(const PolyStyle &p, std::map<PolyStyle, U32> &idMap, FDTDefineShape3 *polygon);
	void addEdge(const TEdge &e, TPointD &p0, TPointD &p1);
	void addNewEdge(const TEdge &e);
	//void closeRegion(int numEdges);

	void drawHangedOutlines();

	void addAutoclose(biPoint &bp, int edgeIndex);

	inline TPoint toTwips(const TPointD &p) { return TPoint((int)(m_tw * p.x), (int)(m_tw * (-p.y))); }

	~Imp()
	{
		clearPointerContainer(m_toBeDeleted);
		clearPointerContainer(m_quadsToBeDeleted);
		clearPointerContainer(m_strokesToBeDeleted);
		if (m_oldFrameData)
			delete m_oldFrameData;

		while (!m_soundBuffer.empty()) {
			delete[] * m_soundBuffer.rbegin();
			m_soundBuffer.pop_back();
		}
	}

	//===================================================================

	/*
  l'inizializzazione di m_currDepth e' 3 poiche' si riservanola depth 1
  per la clipcamera e la depth 2 per l'eventuale bottone (non visibile)
  del play non automatico
*/
	Imp(int lx, int ly, int frameCount, int frameRate, TPropertyGroup *properties, bool keepImages)
		: m_version(4), m_tags(), m_currSprite(0), m_currDepth(3), m_frameRate(frameRate), m_currFrameIndex(-1), m_lx(lx), m_ly(ly), m_currPalette(0), m_maskEnabled(false), m_isMask(false), m_polylinesArray(), m_lineColor(TPixel32::Black), m_thickness(0), m_polyData(), m_regionDepth(0), m_strokeCount(0), m_affine(), m_matrixStack(), m_imagesMap(), m_imagesScaleMap(), m_edgeMap(), m_texturesMap(), m_autocloseMap(), m_strokeMap(), m_outlines(), m_currStrokeColor(0, 0, 0, 0), m_frameData(0), m_oldFrameData(0)
		  //, m_notClipped(true)
		  ,
		  m_sound(), m_soundSize(0), m_soundBuffer(), m_currMask(), m_toBeDeleted(), m_quadsToBeDeleted(), m_strokesToBeDeleted(), m_soundOffset(0), m_loaderAdded(false), m_globalScale(), m_keepImages(keepImages), m_supportAlpha(true), m_soundRate(c_soundRate)
	//, m_totMem(0)

	//, m_of("c:\\temp\\boh.txt")

	{
		m_tags.AddFObj(new FCTFrameLabel(new FString("DigitalVideoRm")));

		if (properties)
			m_properties.setProperties(properties);
		//m_currentBgStyle.push_back(PolyStyle());

		m_tw = 16384 / std::max(m_lx, m_ly);
		if (m_tw > 20)
			m_tw = 20;
		Tw = m_tw;
		m_sCoord1 = m_tw;
		//addCameraClip();
		if (!m_properties.m_autoplay.getValue() && !m_properties.m_preloader.getValue())
			addPause();
	}

	void drawSubregions(TFlash *tf, const TRegion *r, const TPalette *palette);
	void doDrawPolygon(std::list<FlashPolyline> &polylines, int clippedShapes = 0);
	int drawSegments(const std::vector<TSegment> segmentArray, bool isGradientColor);
	int drawquads(const std::vector<TQuadratic> quadsArray);
	int drawRectangle(const TRectD &rect);
	int drawPolyline(std::vector<TPointD> &poly);
	int drawEllipse(const TPointD &center, double radiusX, double radiusY);
	void drawDot(const TPointD &center, double radius);

	void buildRegion(TFlash *tf, const TVectorImageP &vi, int regionIndex);
	void buildStroke(TFlash *tf, const TVectorImageP &vi, int strokeIndex);

	//void addCameraClip(int index);
	void writeFrame(TFlash *tf, bool isLast, int frameCountLoader, bool lastScene);
	U16 getTexture(const PolyStyle &p, int &lx, int &ly);

	void addSoundToFrame(bool isLast);

	void addActionStop();
	void addLoader();
	void addSkipLoader(int jumpToFrame);
	void addPause();
	void beginMask();
	void endMask();
	void addUrlLink(std::string url);
	USHORT buildImage(const TImageP vi, TFlash *tf, double &scaleFactor, bool isMask);
	USHORT buildVectorImage(const TVectorImageP &img, TFlash *tf, double &scaleFactor, bool isMask);
	USHORT buildRasterImage(const TImageP rimg, TFlash *tf);
	bool drawOutline(TStroke *s, bool separeDifferentColors = true);
	inline void addEdgeStraightToShape(FDTDefineShape3 *shape, int x, int y);
	inline void addEdgeStraightToShape(FDTDefineShape3 *shape, const TPoint &p);
};

//===================================================================

void TFlash::setSoundRate(int soundrate)
{
	m_imp->m_soundRate = soundrate;
}

//===================================================================

void TFlash::enableAlphaChannelForRaster(bool supportAlpha)
{
	m_imp->m_supportAlpha = supportAlpha;
}

//===================================================================
namespace
{
inline void addShape(FDTDefineShape3 *polygon, bool newStyle, bool lStyle,
				 bool fillStyle1, bool fillStyle0, bool move, int x, int y,
				 int style0, int style1, int lineStyle)
{
	polygon->AddShapeRec(new FShapeRecChange(newStyle, lStyle, fillStyle1, fillStyle0, move,
						 x, y, style0, style1, lineStyle, 0, 0));
}

inline void addShape(FDTDefineShape3 *polygon, bool newStyle, bool lStyle,
				 bool fillStyle1, bool fillStyle0, bool move, TPoint *p,
				 int style0, int style1, int lineStyle)
{
	polygon->AddShapeRec(new FShapeRecChange(newStyle, lStyle, fillStyle1, fillStyle0, move,
						 p ? p->x : 0, p ? p->y : 0, style0, style1, lineStyle, 0, 0));
}
}

//===================================================================

inline void TFlash::Imp::addEdgeStraightToShape(FDTDefineShape3 *shape, int x, int y)
{
	if (x == 0 && y == 0)
		return;

	//m_of<< "ADD STRAIGHT LINE: "<<x<<", "<<y<<std::endl;

	if (abs(x) > 65535 || abs(y) > 65535) //flash non sa scrivere segmenti piu' lunghi di cosi', spezzo
	{
		shape->AddShapeRec(new FShapeRecEdgeStraight((x + 1) / 2, (y + 1) / 2));
		shape->AddShapeRec(new FShapeRecEdgeStraight(x / 2, y / 2));
	} else
		shape->AddShapeRec(new FShapeRecEdgeStraight(x, y));
}

inline void TFlash::Imp::addEdgeStraightToShape(FDTDefineShape3 *shape, const TPoint &p)
{
	addEdgeStraightToShape(shape, p.x, p.y);
}

//-------------------------------------------------------------------

double computeAverageThickness(const TStroke *s)
{
	int count = s->getControlPointCount();
	double resThick = 0;
	int i;

	for (i = 0; i < s->getControlPointCount(); i++) {
		double thick = s->getControlPoint(i).thick;
		if (i >= 2 && i < s->getControlPointCount() - 2)
			resThick += thick;
	}
	if (count < 6)
		return s->getControlPoint(count / 2 + 1).thick;

	return resThick / (s->getControlPointCount() - 4);
}

//-------------------------------------------------------------------

inline FMatrix *TFlash::Imp::affine2Matrix(const TAffine &aff)
{
	if (aff != TAffine()) {
		bool hasA11OrA22, hasA12OrA21;

		hasA11OrA22 = hasA12OrA21 = (aff.a12 != 0 || aff.a21 != 0);
		if (!hasA12OrA21)
			hasA11OrA22 = !areAlmostEqual(aff.det(), 1.0, 1e-3);
		return new FMatrix(hasA11OrA22, FloatToFixed(aff.a11), FloatToFixed(aff.a22),
					   hasA12OrA21, FloatToFixed(-aff.a21), FloatToFixed(-aff.a12),
					   (TINT32)tround(aff.a13 * m_tw), -(TINT32)tround(aff.a23 * m_tw));
	} else
		return 0;
}

TFlash::operator TVectorRenderData() const
{
    // Construct a TVectorRenderData using current affine and palette
    TAffine aff = m_imp->m_affine;
    const TPalette *palette = m_imp->m_currPalette;
    return TVectorRenderData(TVectorRenderData::ViewerSettings(), aff, TRect(), palette, nullptr);
}
