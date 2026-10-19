#pragma once

#ifndef IKNODE_H
#define IKNODE_H

#include <math.h>
#include <assert.h>
#include <vector>
#include "tgeometry.h"

// Implemented in flarelib/iknode.rs (flare_formats). Return 0 on success.
extern "C" int flare_iknode_rotate_point(double *x, double *y, double theta);
extern "C" int flare_iknode_compute_s(double rx, double ry, size_t n,
                                      const double *thetas, const double *rs,
                                      double *outS);

#undef DVAPI
#undef DVVAR
#ifdef FLARELIB_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

class DVAPI IKNode {
public:
  enum Purpose { JOINT, EFFECTOR };

  IKNode() : m_parent(0), m_pos() {
    r = TPointD(0.0, 1.0);  // r will be updated when the node is added
                            // in the skeleton
    theta    = 0.0;
    m_parent = 0;
  }

  void setParent(IKNode *parent) { m_parent = parent; }
  void setRealParent(IKNode *realParent) { m_parent = realParent; }

  void setTheta(double newTheta) { theta = newTheta; }
  void setTheta0(double newTheta) { theta0 = newTheta; }

  void setIndex(int index) { m_index = index; }
  int getIndex() const { return m_index; }

  IKNode *getParent() const { return m_parent; }

  Purpose getPurpose() { return m_purpose; }

  void setPos(const TPointD &pos) { m_pos = pos; }
  TPointD getPos() const { return m_pos; }
  void setPurpose(Purpose purpose) { m_purpose = purpose; }

  void setSeqNumJoint(int number) { m_seqNumJoint = number; }
  void setSeqNumEffector(int number) { m_seqNumEffector = number; }

  int getEffectorNum() const { return m_seqNumEffector; }
  int getJointNum() const { return m_seqNumJoint; }

  double getTheta() const { return theta; }
  double getTheta0() const { return theta0; }

  bool IsEffector() const { return m_purpose == EFFECTOR; }
  bool IsJoint() const { return m_purpose == JOINT; }

  const TPointD &GetS() const { return s; }

  void setR(TPointD pos) { r = pos; }
  void setS(TPointD pos) { s = pos; }

  // Global position of this node. The ancestor chain (nearest first) is
  // gathered here and the rotate/translate arithmetic runs in Rust.
  void computeS(void) {
    std::vector<double> thetas, rs;
    for (IKNode *y = getParent(); y; y = y->getParent()) {
      thetas.push_back(y->theta);
      rs.push_back(y->r.x);
      rs.push_back(y->r.y);
    }
    s = r;  // the original seeded s with r before walking the ancestors
    double out[2] = {r.x, r.y};
    if (flare_iknode_compute_s(r.x, r.y, thetas.size(), thetas.data(),
                               rs.data(), out) != 0)
      return;
    s = TPointD(out[0], out[1]);
    if (!thetas.empty()) m_pos = s;  // the original only set m_pos inside the walk
  }

  double AddToTheta(double delta) {
    theta += delta;
    return theta;
  }

  bool isFrozen() const { return freezed; }
  void freeze() { freezed = true; }  // keeps the theta angle constant
  void unFreeze() { freezed = false; }

private:
  int m_index;
  IKNode *m_parent;

  TPointD m_pos;
  Purpose m_purpose;
  int m_seqNumJoint;     // Joint index in the Joint sequence
  int m_seqNumEffector;  // index of the effector in the sequence of effectors

  TPointD r;  // Relative position
  TPointD s;  // Global position
  // TPointD w; // Global axis rotation

  double theta;      // joint angle(radians)
  double theta0;     // initial angle of the joint(radians)
  double minTheta;   // lower angle limit
  double maxTheta;   // high angle limit
  double restAngle;  

  bool freezed;  
};

// Free function of the original iknode.cpp: rotates `point` in place and returns it.
inline TPointD rotatePoint(TPointD &point, double theta) {
  flare_iknode_rotate_point(&point.x, &point.y, theta);
  return point;
}

#endif  // IKNODE_H
