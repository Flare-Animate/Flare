//! Port of `stdfx/naru_maxflow.cpp` and the `Graph` internals of
//! `stdfx/naru_graph.h`: the Boykov-Kolmogorov max-flow / min-cut used by
//! `naru_lazybrush.cpp`.
//!
//! `naru_graph.h` keeps a thin `Graph` class whose methods forward to the C ABI
//! below over an opaque handle, so `naru_lazybrush.cpp` is unchanged.
//!
//! The C++ stores nodes and arcs in flat arrays and links them with raw
//! pointers, using `(arc*)1` (`TERMINAL`) and `(arc*)2` (`ORPHAN`) as sentinels.
//! Here the links are indices and the sentinels are the negative constants
//! [`TERMINAL`] / [`ORPHAN`]; `NONE` marks a null link. Deviations from the C++,
//! only where the original was undefined behaviour:
//! - `addEdge` with a node index outside `[0, maxNodeNum)` reports status `-2`
//!   instead of writing past the `nodes` array.
//! - While descending a parent chain, a null parent is treated as "no terminal
//!   found" (the C++ dereferenced the null sentinel).

use std::collections::VecDeque;

/// No link (the C++ `nullptr`).
const NONE: i32 = -1;
/// Node's parent arc is the source/sink terminal (the C++ `(Graph::arc*)1`).
const TERMINAL: i32 = -2;
/// Node is an orphan (the C++ `(Graph::arc*)2`).
const ORPHAN: i32 = -3;

/// `Graph::node`.
#[derive(Clone, Copy)]
struct Node {
    first_arc: i32,
    parent_arc: i32,
    is_sink: bool,
    t_cap: i32,
}

/// `Graph::arc`.
#[derive(Clone, Copy)]
struct Arc {
    head_node: i32,
    next_arc: i32,
    rev_arc: i32,
    r_cap: i32,
}

/// `Graph` (see `stdfx/naru_graph.h`).
pub struct Graph {
    nodes: Vec<Node>,
    arcs: Vec<Arc>,
    arcs_num: usize,
    active: VecDeque<i32>,
    orphan: VecDeque<i32>,
    // The C++ kept `flow` private and never read it; kept for fidelity and
    // exposed through [`Graph::flow`] for tests.
    flow: i32,
}

impl Graph {
    /// `Graph::Graph(int maxNodeNum)`.
    pub fn new(max_node_num: i32) -> Graph {
        let n = if max_node_num > 0 { max_node_num as usize } else { 0 };
        Graph {
            nodes: vec![
                Node {
                    first_arc: NONE,
                    parent_arc: NONE,
                    is_sink: false,
                    t_cap: 0,
                };
                n
            ],
            arcs: Vec::with_capacity(n.saturating_mul(4)),
            arcs_num: n.saturating_mul(4),
            active: VecDeque::new(),
            orphan: VecDeque::new(),
            flow: 0,
        }
    }

    /// `Graph::addEdge`. Returns 0 on success, -1 if `arcsNum` would overflow
    /// (the C++ `std::runtime_error`), -2 if a node index is out of range.
    pub fn add_edge(&mut self, from: i32, to: i32, cap: i32, rev_cap: i32) -> i32 {
        if self.arcs.len() + 2 >= self.arcs_num {
            return -1;
        }
        if from < 0 || to < 0 || from as usize >= self.nodes.len() || to as usize >= self.nodes.len()
        {
            return -2;
        }
        let from_i = from as usize;
        let to_i = to as usize;
        let from_first = self.nodes[from_i].first_arc;
        let to_first = self.nodes[to_i].first_arc;
        let new_arc = self.arcs.len() as i32;
        let rev_arc = new_arc + 1;
        self.arcs.push(Arc {
            head_node: to,
            next_arc: from_first,
            rev_arc,
            r_cap: cap,
        });
        self.arcs.push(Arc {
            head_node: from,
            next_arc: to_first,
            rev_arc: new_arc,
            r_cap: rev_cap,
        });
        self.nodes[from_i].first_arc = new_arc;
        self.nodes[to_i].first_arc = rev_arc;
        0
    }

    /// `Graph::addTerminal`.
    pub fn add_terminal(&mut self, node_index: i32, t_cap: i32) {
        if node_index >= 0 && (node_index as usize) < self.nodes.len() {
            self.nodes[node_index as usize].t_cap = t_cap;
        }
    }

    /// `Graph::getSegment`: SOURCE (0) / SINK (1), or `default_type` when the
    /// node has no parent yet.
    pub fn get_segment(&self, node_index: i32, default_type: i32) -> i32 {
        let i = node_index as usize;
        if node_index >= 0 && i < self.nodes.len() && self.nodes[i].parent_arc != NONE {
            if self.nodes[i].is_sink {
                1
            } else {
                0
            }
        } else {
            default_type
        }
    }

    /// Total flow pushed so far (used by the differential test).
    pub fn flow(&self) -> i32 {
        self.flow
    }

    fn set_active(&mut self, n: i32) {
        self.active.push_back(n);
    }

    fn set_orphan(&mut self, n: i32) {
        self.nodes[n as usize].parent_arc = ORPHAN;
        self.orphan.push_back(n);
    }

    fn next_active(&mut self) -> Option<i32> {
        while let Some(n) = self.active.pop_front() {
            if self.nodes[n as usize].parent_arc == ORPHAN {
                continue;
            }
            return Some(n);
        }
        None
    }

    fn next_orphan(&mut self) -> Option<i32> {
        self.orphan.pop_front()
    }

    /// `Graph::augment`.
    fn augment(&mut self, mid_arc: i32) {
        let mut bottleneck = self.arcs[mid_arc as usize].r_cap;
        // source tree
        {
            let mut n = self.arcs[self.arcs[mid_arc as usize].rev_arc as usize].head_node;
            loop {
                let arc = self.nodes[n as usize].parent_arc;
                // TERMINAL ends the chain; ORPHAN/NONE cannot occur on a valid
                // augment path (the C++ dereferenced them).
                if arc < 0 {
                    break;
                }
                let rc = self.arcs[self.arcs[arc as usize].rev_arc as usize].r_cap;
                if bottleneck > rc {
                    bottleneck = rc;
                }
                n = self.arcs[arc as usize].head_node;
            }
            if bottleneck > self.nodes[n as usize].t_cap {
                bottleneck = self.nodes[n as usize].t_cap;
            }
        }
        // sink tree
        {
            let mut n = self.arcs[mid_arc as usize].head_node;
            loop {
                let arc = self.nodes[n as usize].parent_arc;
                if arc < 0 {
                    break;
                }
                let rc = self.arcs[arc as usize].r_cap;
                if bottleneck > rc {
                    bottleneck = rc;
                }
                n = self.arcs[arc as usize].head_node;
            }
            if bottleneck > -self.nodes[n as usize].t_cap {
                bottleneck = -self.nodes[n as usize].t_cap;
            }
        }

        // augment flow
        // source tree
        {
            let mut n = self.arcs[self.arcs[mid_arc as usize].rev_arc as usize].head_node;
            loop {
                let arc = self.nodes[n as usize].parent_arc;
                if arc < 0 {
                    break;
                }
                self.arcs[arc as usize].r_cap += bottleneck;
                let rev = self.arcs[arc as usize].rev_arc;
                self.arcs[rev as usize].r_cap -= bottleneck;
                if self.arcs[rev as usize].r_cap == 0 {
                    self.set_orphan(n);
                }
                n = self.arcs[arc as usize].head_node;
            }
            self.nodes[n as usize].t_cap -= bottleneck;
            if self.nodes[n as usize].t_cap == 0 {
                self.set_orphan(n);
            }
        }
        // sink tree
        {
            let mut n = self.arcs[mid_arc as usize].head_node;
            loop {
                let arc = self.nodes[n as usize].parent_arc;
                if arc < 0 {
                    break;
                }
                self.arcs[arc as usize].r_cap -= bottleneck;
                let rev = self.arcs[arc as usize].rev_arc;
                self.arcs[rev as usize].r_cap += bottleneck;
                if self.arcs[arc as usize].r_cap == 0 {
                    self.set_orphan(n);
                }
                n = self.arcs[arc as usize].head_node;
            }
            self.nodes[n as usize].t_cap += bottleneck;
            if self.nodes[n as usize].t_cap == 0 {
                self.set_orphan(n);
            }
        }
        // mid arc
        let rev = self.arcs[mid_arc as usize].rev_arc;
        self.arcs[rev as usize].r_cap += bottleneck;
        self.arcs[mid_arc as usize].r_cap -= bottleneck;

        self.flow += bottleneck;
    }

    /// `Graph::adopt`.
    fn adopt(&mut self) {
        while let Some(n) = self.next_orphan() {
            let mut found = false;
            if !self.nodes[n as usize].is_sink {
                // source tree
                let mut arc = self.nodes[n as usize].first_arc;
                while arc != NONE {
                    let head = self.arcs[arc as usize].head_node;
                    let rev_cap = self.arcs[self.arcs[arc as usize].rev_arc as usize].r_cap;
                    if rev_cap != 0
                        && !self.nodes[head as usize].is_sink
                        && self.nodes[head as usize].parent_arc != NONE
                    {
                        let mut nn = head;
                        loop {
                            let parent = self.nodes[nn as usize].parent_arc;
                            if parent == TERMINAL {
                                self.nodes[n as usize].parent_arc = arc;
                                self.set_active(n);
                                found = true;
                                break;
                            } else if parent == ORPHAN || parent == NONE {
                                break;
                            }
                            if found {
                                break;
                            }
                            nn = self.arcs[parent as usize].head_node;
                        }
                    }
                    arc = self.arcs[arc as usize].next_arc;
                }
            } else {
                // sink tree
                let mut arc = self.nodes[n as usize].first_arc;
                while arc != NONE {
                    let head = self.arcs[arc as usize].head_node;
                    if self.arcs[arc as usize].r_cap != 0
                        && self.nodes[head as usize].is_sink
                        && self.nodes[head as usize].parent_arc != NONE
                    {
                        let mut nn = head;
                        loop {
                            let parent = self.nodes[nn as usize].parent_arc;
                            if parent == TERMINAL {
                                self.nodes[n as usize].parent_arc = arc;
                                self.set_active(n);
                                found = true;
                                break;
                            } else if parent == ORPHAN || parent == NONE {
                                break;
                            }
                            if found {
                                break;
                            }
                            nn = self.arcs[parent as usize].head_node;
                        }
                    }
                    arc = self.arcs[arc as usize].next_arc;
                }
            }

            // no origin found
            if !found {
                let mut arc = self.nodes[n as usize].first_arc;
                while arc != NONE {
                    let nn = self.arcs[arc as usize].head_node;
                    let parent2 = self.nodes[nn as usize].parent_arc;
                    if self.nodes[nn as usize].is_sink == self.nodes[n as usize].is_sink && parent2 != NONE
                    {
                        if parent2 != TERMINAL
                            && parent2 != ORPHAN
                            && self.arcs[parent2 as usize].head_node == n
                        {
                            self.set_orphan(nn);
                        }
                    }
                    arc = self.arcs[arc as usize].next_arc;
                }
            }
        }
    }

    /// `Graph::mincut`.
    pub fn mincut(&mut self) {
        // initialize active nodes
        for i in 0..self.nodes.len() as i32 {
            let t = self.nodes[i as usize].t_cap;
            if t > 0 {
                self.nodes[i as usize].is_sink = false;
                self.nodes[i as usize].parent_arc = TERMINAL;
                self.set_active(i);
            } else if t < 0 {
                self.nodes[i as usize].is_sink = true;
                self.nodes[i as usize].parent_arc = TERMINAL;
                self.set_active(i);
            }
        }

        while let Some(n) = self.next_active() {
            let mut found: i32 = NONE;
            if !self.nodes[n as usize].is_sink {
                // grow source node
                let mut arc = self.nodes[n as usize].first_arc;
                while arc != NONE {
                    if self.arcs[arc as usize].r_cap <= 0 {
                        arc = self.arcs[arc as usize].next_arc;
                        continue;
                    }
                    let nn = self.arcs[arc as usize].head_node;
                    let parent = self.nodes[nn as usize].parent_arc;
                    if parent == NONE || parent == ORPHAN {
                        self.nodes[nn as usize].is_sink = false;
                        self.nodes[nn as usize].parent_arc = self.arcs[arc as usize].rev_arc;
                        self.set_active(nn);
                    } else if self.nodes[nn as usize].is_sink {
                        found = arc;
                        break;
                    }
                    arc = self.arcs[arc as usize].next_arc;
                }
            } else {
                // grow sink node
                let mut arc = self.nodes[n as usize].first_arc;
                while arc != NONE {
                    let rev = self.arcs[arc as usize].rev_arc;
                    if self.arcs[rev as usize].r_cap <= 0 {
                        arc = self.arcs[arc as usize].next_arc;
                        continue;
                    }
                    let nn = self.arcs[arc as usize].head_node;
                    let parent = self.nodes[nn as usize].parent_arc;
                    if parent == NONE || parent == ORPHAN {
                        self.nodes[nn as usize].is_sink = true;
                        self.nodes[nn as usize].parent_arc = rev;
                        self.set_active(nn);
                    } else if !self.nodes[nn as usize].is_sink {
                        found = rev;
                        break;
                    }
                    arc = self.arcs[arc as usize].next_arc;
                }
            }

            // found path
            if found != NONE {
                self.augment(found);
                self.adopt();
            }
        }
    }
}

fn guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(default)
}

/// `Graph::Graph(int)`; free with `flare_graph_free`.
#[no_mangle]
pub extern "C" fn flare_graph_new(max_node_num: i32) -> *mut Graph {
    guard(std::ptr::null_mut(), || {
        Box::into_raw(Box::new(Graph::new(max_node_num)))
    })
}

/// `Graph::~Graph`.
#[no_mangle]
pub unsafe extern "C" fn flare_graph_free(g: *mut Graph) {
    if !g.is_null() {
        drop(Box::from_raw(g));
    }
}

/// `Graph::addEdge`: 0 ok, -1 capacity exceeded, -2 node index out of range.
#[no_mangle]
pub unsafe extern "C" fn flare_graph_add_edge(
    g: *mut Graph,
    from: i32,
    to: i32,
    cap: i32,
    rev_cap: i32,
) -> i32 {
    guard(-1, || {
        if g.is_null() {
            -1
        } else {
            (*g).add_edge(from, to, cap, rev_cap)
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn flare_graph_add_terminal(g: *mut Graph, node_index: i32, t_cap: i32) {
    let _ = guard((), || {
        if !g.is_null() {
            (*g).add_terminal(node_index, t_cap);
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn flare_graph_get_segment(
    g: *mut Graph,
    node_index: i32,
    default_type: i32,
) -> i32 {
    guard(default_type, || {
        if g.is_null() {
            default_type
        } else {
            (*g).get_segment(node_index, default_type)
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn flare_graph_mincut(g: *mut Graph) {
    let _ = guard((), || {
        if !g.is_null() {
            (*g).mincut();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_edge_is_capped() {
        // 0 --(cap 3)--> 1, source +10 at 0, sink -10 at 1: flow limited by 3.
        let mut g = Graph::new(2);
        assert_eq!(g.add_edge(0, 1, 3, 0), 0);
        g.add_terminal(0, 10);
        g.add_terminal(1, -10);
        g.mincut();
        assert_eq!(g.flow(), 3);
        assert_eq!(g.get_segment(0, 9), 0); // SOURCE
        assert_eq!(g.get_segment(1, 9), 1); // SINK
    }

    #[test]
    fn unreachable_node_keeps_default() {
        let mut g = Graph::new(3);
        assert_eq!(g.add_edge(0, 1, 2, 0), 0);
        g.add_terminal(0, 5);
        g.add_terminal(1, -5);
        g.mincut();
        assert_eq!(g.flow(), 2);
        assert_eq!(g.get_segment(2, 1), 1); // untouched -> default (SINK)
        assert_eq!(g.get_segment(2, 0), 0);
    }

    #[test]
    fn bottleneck_is_minimum_cut() {
        // 0 -> 1 (cap 4), 1 -> 2 (cap 2), 0 -> 2 (cap 1); min cut 3.
        let mut g = Graph::new(3);
        assert_eq!(g.add_edge(0, 1, 4, 0), 0);
        assert_eq!(g.add_edge(1, 2, 2, 0), 0);
        assert_eq!(g.add_edge(0, 2, 1, 0), 0);
        g.add_terminal(0, 100);
        g.add_terminal(2, -100);
        g.mincut();
        assert_eq!(g.flow(), 3);
        assert_eq!(g.get_segment(0, 9), 0);
        assert_eq!(g.get_segment(2, 9), 1);
    }

    #[test]
    fn edge_capacity_exceeded_and_bad_index() {
        let mut g = Graph::new(2);
        // arcsNum == 8; each edge takes 2 arcs, and count+2 >= 8 throws.
        assert_eq!(g.add_edge(0, 1, 1, 0), 0);
        assert_eq!(g.add_edge(0, 1, 1, 0), 0);
        assert_eq!(g.add_edge(0, 1, 1, 0), 0); // count 4, 4+2<8 ok
        assert_eq!(g.add_edge(0, 1, 1, 0), -1); // count 6, 6+2>=8
        assert_eq!(g.add_edge(0, 5, 1, 0), -1); // capacity first (count 6)
        let mut h = Graph::new(2);
        assert_eq!(h.add_edge(0, 5, 1, 0), -2);
        assert_eq!(h.add_edge(-1, 0, 1, 0), -2);
    }

    #[test]
    fn ffi_roundtrip() {
        unsafe {
            let g = flare_graph_new(2);
            assert!(!g.is_null());
            assert_eq!(flare_graph_add_edge(g, 0, 1, 5, 0), 0);
            flare_graph_add_terminal(g, 0, 5);
            flare_graph_add_terminal(g, 1, -5);
            flare_graph_mincut(g);
            assert_eq!(flare_graph_get_segment(g, 0, 9), 0);
            assert_eq!(flare_graph_get_segment(g, 1, 9), 1);
            flare_graph_free(g);
            // null handle is a no-op / returns the default
            flare_graph_mincut(std::ptr::null_mut());
            flare_graph_add_terminal(std::ptr::null_mut(), 0, 1);
            assert_eq!(flare_graph_get_segment(std::ptr::null_mut(), 0, 7), 7);
            assert_eq!(flare_graph_add_edge(std::ptr::null_mut(), 0, 1, 1, 0), -1);
            flare_graph_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn zero_and_negative_size() {
        let g = Graph::new(0);
        assert_eq!(g.flow(), 0);
        let g = Graph::new(-4);
        assert_eq!(g.get_segment(0, 3), 3);
    }
}
