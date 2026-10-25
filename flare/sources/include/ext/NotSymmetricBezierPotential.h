#pragma once

#ifndef NOT_Symmetric_BEZIER_POTENTIAL_H
#define NOT_Symmetric_BEZIER_POTENTIAL_H

/**
 * @author  Fabrizio Morciano <fabrizio.morciano@gmail.com>
 */

//#include "tcommon.h"
//#include "tstroke.h"

#include "ext/Potential.h"
#include "tstroke.h"

extern "C" {
int flare_nsbezier_potential_params(double stroke_length, double length_at_param,
                                    double action_length, double *left,
                                    double *right);
double flare_nsbezier_potential_shape(double x_len, double action_length,
                                      double length_at_param);
double flare_nsbezier_potential_value(double x_len, double len_at_1,
                                      double len_at_0, double action_length,
                                      double stroke_length,
                                      double length_at_param, double left,
                                      double right);
}

#undef DVAPI
#undef DVVAR
#ifdef TNZEXT_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

namespace ToonzExt {
class DVAPI NotSymmetricBezierPotential final : public Potential {
public:
  virtual ~NotSymmetricBezierPotential() {}


  // ---- inline definitions (formerly NotSymmetricBezierPotential.cpp; the arithmetic is in NotSymmetricBezierPotential.rs) ----
  void setParameters_(const TStroke *ref, double par, double al) override {
  assert(ref);
  ref_          = ref;
  par_          = par;
  actionLength_ = al;
  strokeLength_  = ref->getLength();
  lengthAtParam_ = ref->getLength(par_);
  flare_nsbezier_potential_params(strokeLength_, lengthAtParam_, actionLength_,
                                  &leftFactor_, &rightFactor_);
  }
  double value_(double w) const override {
    assert(0.0 <= w && w <= 1.0);
    return compute_value(w);
  }
  double compute_shape(double v) const {
    return flare_nsbezier_potential_shape(ref_->getLength(v), actionLength_, lengthAtParam_);
  }
  double compute_value(double v) const {
    return flare_nsbezier_potential_value(ref_->getLength(v), ref_->getLength(1.0), ref_->getLength(0.0), actionLength_, strokeLength_, lengthAtParam_, leftFactor_, rightFactor_);
  }
  Potential *clone() override { return new NotSymmetricBezierPotential; }

private:
  const TStroke *ref_;
  double range_;  // range of mapping
  double par_;
  double actionLength_;   // lunghezza dell'azione
  double strokeLength_;   // lunghezza stroke
  double lengthAtParam_;  // lunghezza nel pto di movimento
  double leftFactor_;     // fattore di shape x la curva a sinistra
  double rightFactor_;    // fattore di shape x la curva a dx
};
}
#endif /* NOT_Symmetric_BEZIER_POTENTIAL_H */
//-----------------------------------------------------------------------------
//  End Of File
//-----------------------------------------------------------------------------
