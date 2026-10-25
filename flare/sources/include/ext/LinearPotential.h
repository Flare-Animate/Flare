#pragma once

#ifndef LINEAR_POTENTIAL_H
#define LINEAR_POTENTIAL_H

/**
 * @author  Fabrizio Morciano <fabrizio.morciano@gmail.com>
 */

//#include "tcommon.h"
//#include "tstroke.h"

#include "ext/Potential.h"
#include "tstroke.h"

extern "C" {
int flare_linear_potential_params(double stroke_length, double length_at_param,
                                  double *left, double *right, double *range);
double flare_linear_potential_shape(double x_len, double action_length,
                                    double length_at_param, double range);
double flare_linear_potential_value(double x_len, double par,
                                    double action_length, double stroke_length,
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
class DVAPI LinearPotential final : public Potential {
public:
  virtual ~LinearPotential() {}


  // ---- inline definitions (formerly LinearPotential.cpp; the arithmetic is in LinearPotential.rs) ----
  void setParameters_(const TStroke *ref, double par, double al) override {
  ref_          = ref;
  par_          = par;
  actionLength_ = al;
  strokeLength_  = ref->getLength();
  lengthAtParam_ = ref->getLength(par);
  flare_linear_potential_params(strokeLength_, lengthAtParam_, &leftFactor_,
                                &rightFactor_, &range_);
  }
  double value_(double w) const override {
    assert(0.0 <= w && w <= 1.0);
    return compute_value(w);
  }
  double compute_shape(double v) const {
    return flare_linear_potential_shape(ref_->getLength(v), actionLength_, lengthAtParam_, range_);
  }
  double compute_value(double v) const {
    return flare_linear_potential_value(ref_->getLength(v), par_, actionLength_, strokeLength_, lengthAtParam_, leftFactor_, rightFactor_);
  }
  Potential *clone() override { return new LinearPotential; }

private:
  const TStroke *ref_;
  double
      /**
* @brief Range of mapping
*
* Only an interval of potential function will be used to
* map the transformation.
*/
      range_,
      par_,            //! Parameter of selection.
      actionLength_,   //! Action length, how many units to move.
      strokeLength_,   //! Stroke Length.
      lengthAtParam_,  //! Length at <code>par_</code>
      leftFactor_,     //! How many units to move on the left of curve.
      rightFactor_;    //! How many units to move on the right of curve.
};
}
#endif /* LINEAR_POTENTIAL_H */
//-----------------------------------------------------------------------------
//  End Of File
//-----------------------------------------------------------------------------
