

#include "flare/ikengine.h"
#include "flare/ikjacobian.h"

#include <stdexcept>

enum Method { JACOB_TRANS, PURE_PSEUDO, DLS, SDLS, COMPARE };

IKEngine::IKEngine() {}

int IKEngine::addJoint(const TPointD &pos, int indexParent) {
  // TODO: evitare che si formino segmenti nulli
  // assert(m_joints.empty() || norm2(pos-m_joints.back())>0.000001);
  // assert(m_nodes[indexParent]);
  assert(m_skeleton.getNode(indexParent));
  m_skeleton.addNode(new IKNode());
  int index = m_skeleton.getNodeCount() - 1;
  m_skeleton.setNode(index, pos, IKNode::JOINT);
  m_skeleton.setParent(index, indexParent);
  return index;
}
// The root must be a pinned point!
void IKEngine::setRoot(const TPointD &pos) {
  m_skeleton.addNode(new IKNode());
  m_skeleton.setNode(0, pos, IKNode::JOINT);
  // m_skeleton.setParent(0,0);
  m_skeleton.setRoot(0);
}

void IKEngine::lock(int index) {
  assert(index > -1 && index < m_skeleton.getNodeCount());
  m_skeleton.setPurpose(index, IKNode::EFFECTOR);
  IKNode *n   = m_skeleton.getNode(index);
  TPointD pos = n->getPos();
  target.push_back(pos);
}
void IKEngine::unlock(int index) {
  assert(index > -1 && index < m_skeleton.getNodeCount());
  m_skeleton.setPurpose(index, IKNode::JOINT);
}

bool IKEngine::isLocked(int index) {
  assert(index > -1 && index < m_skeleton.getNodeCount());
  return m_skeleton.getNode(index)->IsEffector();
}

// The angle arithmetic (normalize, perpendicular, atan2) is in ikjacobian.rs.
extern "C" int flare_ikengine_joint_angle(double px, double py, int hasPrev, double prx,
                                          double pry, double nx, double ny, double *out);

double IKEngine::getJointAngle(int index) {
  assert(index > -1 && index < m_skeleton.getNodeCount());
  TPointD pos = m_skeleton.getNode(index)->getPos();
  TPointD prevPos;
  if (index != 0) {
    int parent = getJointParent(index);
    prevPos    = m_skeleton.getNode(parent)->getPos();
  }
  TPointD nextPos = m_skeleton.getNode(index + 1)->getPos();
  double theta    = 0.0;
  if (flare_ikengine_joint_angle(pos.x, pos.y, index != 0 ? 1 : 0, prevPos.x, prevPos.y,
                                 nextPos.x, nextPos.y, &theta) != 0)
    throw std::runtime_error("IKEngine::getJointAngle failed");
  return theta;
}

void IKEngine::drag(TPointD &pos) {
  // assert(index>-1 && index<m_skeleton.getNodeCount());
  // se lo scheletro è vuoto non succede nulla
  if (m_skeleton.getNodeCount() == 0) return;

  // Grab the last point of the chain
  int indexDrag = m_skeleton.getNodeCount() - 1;
  if (m_skeleton.getNode(indexDrag)->getParent()->IsEffector()) return;
  m_skeleton.setPurpose(indexDrag, IKNode::EFFECTOR);

  // assegno un indice alla sequenza dei giunti (nodi -end effectors)
  setSequenceJoints();

  target.push_back(pos);

  Jacobian jacobian(&m_skeleton, target);
  target.pop_back();
  for (int i = 0; i < 250; i++) doUpdateStep(jacobian);
}

void IKEngine::doUpdateStep(Jacobian &jacobian) {
  jacobian.computeJacobian();  // calcolo Jacobiano e il vettore deltaS
  int WhichMethod = DLS;

  bool clampingDetected = true;
  while (clampingDetected) {
    // Calcolo i cambiamenti dell'angolo
    switch (WhichMethod) {
    case JACOB_TRANS:
      jacobian.CalcDeltaThetasTranspose();  // Jacobian transpose method
      break;
    case DLS:
      jacobian.CalcDeltaThetasDLS();  // Damped least squares method
      break;
    case PURE_PSEUDO:
      jacobian.CalcDeltaThetasPseudoinverse();  // Pure pseudoinverse method
      break;
    case SDLS:
      jacobian
          .CalcDeltaThetasSDLS();  // Selectively damped least squares method
      break;
    default:
      jacobian.ZeroDeltaThetas();
      break;
    }

    jacobian.UpdateThetas();  // Aggiorno i valori di theta

    clampingDetected = jacobian.checkJointsLimit();
    // jacobian.UpdatedSClampValue();
  }
}

// Assegno gli indici nella sequenza dei giunti
void IKEngine::setSequenceJoints() {
  int indexJoint = 0;
  for (int i = 0; i < (int)m_skeleton.getNodeCount(); i++) {
    IKNode *n               = m_skeleton.getNode(i);
    IKNode::Purpose purpose = n->getPurpose();
    if (purpose != IKNode::EFFECTOR) {
      n->setSeqNumJoint(indexJoint);
      indexJoint++;
    }
  }
}

