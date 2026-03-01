#pragma once

#ifndef IWA_PAINTBRUSHFX_H
#define IWA_PAINTBRUSHFX_H

#include "stdfx.h"
#include "tfxparam.h"
#include "tparamset.h"

#include <QVector>
#include <QMap>

struct double2 {
  double x, y;
};

struct colorRGBA {
  double r, g, b, a;
};

struct BrushStroke {
  QVector<TPointD> centerPos;
  TPointD originPos;
  colorRGBA color;
  double length;
  double widthHalf;
  double angle;
  int textureId;
  bool invert;
  double randomVal;
  double stack;  // ����p�̒l
};

struct BrushVertex {
  double pos[2];
  double texCoord[2];
  BrushVertex(const TPointD _pos, double u, double v) {
    pos[0]      = _pos.x;
    pos[1]      = _pos.y;
    texCoord[0] = u;
    texCoord[1] = v;
  }
  BrushVertex() : BrushVertex(TPointD(), 0, 0) {}
};

struct FlowPaintBrushFxParam {
  TDimensionI dim;
  TPointD origin_pos;
  TPointD horiz_pos;
  TPointD vert_pos;
  TRectD bbox;
  int fill_gap_size;

  double h_density;
  double v_density;
  double pos_randomness;
  double pos_wobble;

  int random_seed;
  DoublePair tipLength;
  DoublePair tipWidth;
  DoublePair tipAlpha;
  double width_random;
  double length_random;
  double angle_random;

  int reso;
  bool anti_jaggy;

  TPointD hVec;
  TPointD vVec;
  double2 vVec_unit;

  TAffine brushAff;

  int lastFrame;
};

class Iwa_FlowPaintBrushFx final : public TStandardRasterFx {
  FX_PLUGIN_DECLARATION(Iwa_FlowPaintBrushFx)
public:
  enum StackMode {
    NoSort = 0,
    Smaller,
    Larger,
    Darker,
    Brighter,
    Random
    //,TestModeArea
  };

private:
  TRasterFxPort m_brush;
  TRasterFxPort m_flow;
  TRasterFxPort m_area;
  TRasterFxPort m_color;

  // ���x
  TDoubleParamP m_h_density;
  TDoubleParamP m_v_density;
  // �ʒu�̃����_�����i0�F�i�q�� 1�F��l�Ƀ����_�� <1: �΂�������j
  TDoubleParamP m_pos_randomness;
  TDoubleParamP m_pos_wobble;
  // �^�b�`�̃T�C�Y�iArea�̒l�ɂ���ĕω��j
  TRangeParamP m_tip_width;
  TRangeParamP m_tip_length;
  // �^�b�`�̕s�����x
  TRangeParamP m_tip_alpha;
  TIntParamP m_tip_joints;
  TBoolParamP m_bidirectional;

  // �΂��
  TDoubleParamP m_width_randomness;
  TDoubleParamP m_length_randomness;
  TDoubleParamP m_angle_randomness;  // degree��

  TDoubleParamP m_sustain_width_to_skew;
  TBoolParamP m_anti_jaggy;

  // �����͈�
  TPointParamP m_origin_pos;
  TPointParamP m_horizontal_pos;
  TPointParamP m_vertical_pos;
  TPointParamP m_curve_point;
  TDoubleParamP m_fill_gap_size;

  // ��t���[��
  TDoubleParamP m_reference_frame;
  // ��t���[�����g���Đ�������^�b�`�̊���
  TDoubleParamP m_reference_prevalence;

  // �����_���V�[�h
  TIntParamP m_random_seed;
  // ���בւ�
  TIntEnumParamP m_sortBy;

  // �u���V�^�b�`�̃��X�^�[�f�[�^���擾
  void getBrushRasters(std::vector<TRasterP> &brushRasters, TDimension &b_size,
                       int &lastFrame, TTile &tile, const TRenderSettings &ri);

  template <typename RASTER, typename PIXEL>
  void setFlowTileToBuffer(const RASTER flowRas, double2 *buf);
  template <typename RASTER, typename PIXEL>
  void setAreaTileToBuffer(const RASTER areaRas, double *buf);
  template <typename RASTER, typename PIXEL>
  void setColorTileToBuffer(const RASTER colorRas, colorRGBA *buf);

  // ���߂���
  template <typename RASTER, typename PIXEL>
  void setOutRaster(const RASTER outRas, double *buf);

  void fillGapByDilateAndErode(double *buf, const TDimension &dim,
                               const int fill_gap_size);

  void computeBrushVertices(QVector<BrushVertex> &brushVertices,
                            QList<BrushStroke> &brushStrokes,
                            FlowPaintBrushFxParam &p, TTile &tile, double frame,
                            const TRenderSettings &ri);

  double getSizePixelAmount(const double val, const TAffine affine);
  FlowPaintBrushFxParam getParam(TTile &tile, double frame,
                                 const TRenderSettings &ri);

public:
  Iwa_FlowPaintBrushFx();

  bool canHandle(const TRenderSettings &info, double frame) override {
    return true;
  }
  bool doGetBBox(double frame, TRectD &bBox,
                 const TRenderSettings &info) override;
  void doCompute(TTile &tile, double frame, const TRenderSettings &ri) override;
  void getParamUIs(TParamUIConcept *&concepts, int &length) override;

  std::string getAlias(double frame,
                       const TRenderSettings &info) const override;
};

#endif
