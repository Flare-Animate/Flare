#pragma once

// naru_graph.h: Boykov-Kolmogorov max-flow / min-cut graph used by
// naru_lazybrush.cpp.
//
// Implemented in naru_maxflow.rs (flare_formats): the C++ Graph held node and
// arc arrays and linked them with raw pointers; the implementation now lives in
// Rust behind an opaque handle and the class below is a thin forwarder, so
// naru_lazybrush.cpp is unchanged. See docs/PORTING.md.

#include "stdfx.h"
#include <stdexcept>

extern "C" {
struct FlareGraph;
FlareGraph *flare_graph_new(int maxNodeNum);
void flare_graph_free(FlareGraph *g);
// 0 ok, -1 edge capacity exceeded, -2 node index out of range.
int flare_graph_add_edge(FlareGraph *g, int from, int to, int cap, int revCap);
void flare_graph_add_terminal(FlareGraph *g, int nodeIndex, int tCap);
int flare_graph_get_segment(FlareGraph *g, int nodeIndex, int defaultType);
void flare_graph_mincut(FlareGraph *g);
}

class Graph {
public:
  typedef enum { SOURCE = 0, SINK = 1 } terminalType;

  Graph(int maxNodeNum) : m_g(flare_graph_new(maxNodeNum)) {}
  ~Graph() { flare_graph_free(m_g); }

  void addEdge(int from, int to, int cap, int revCap) {
    int rc = flare_graph_add_edge(m_g, from, to, cap, revCap);
    if (rc == -1) throw std::runtime_error("Maximum number of edges exceeded");
    if (rc == -2) throw std::out_of_range("Graph::addEdge: node index out of range");
  }

  void addTerminal(int nodeIndex, int tCap) {
    flare_graph_add_terminal(m_g, nodeIndex, tCap);
  }

  terminalType getSegment(int nodeIndex, terminalType defaultType) {
    return (terminalType)flare_graph_get_segment(m_g, nodeIndex, (int)defaultType);
  }

  void mincut() { flare_graph_mincut(m_g); }

private:
  Graph(const Graph &);
  Graph &operator=(const Graph &);
  FlareGraph *m_g;
};
