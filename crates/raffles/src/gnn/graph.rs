//! Graph topology: the mesh or particle connectivity a message-passing network
//! runs on, and the reach questions that can be asked of it.
//!
//! Nothing here involves machine learning or `burn`. A [`Graph`] is a plain
//! adjacency structure, and every function in this file is an ordinary
//! combinatorial computation — which is deliberate, because the physics-guided
//! bound in [`super::bound`] is *about* this topology and a caller who only
//! wants that bound should not have to pull in a tensor library to get it.

use crate::{RafflesError, Result};

/// An undirected graph over `n` nodes, stored as a directed edge list with
/// both directions present.
///
/// # Why both directions
///
/// Message passing is directional: a message flows from a sender to a
/// receiver. An undirected mesh edge is therefore two messages, and storing it
/// as two directed edges means the aggregation step needs no special case.
/// [`Graph::from_undirected_edges`] does the doubling; [`Graph::new`] takes the
/// directed list as given, for a genuinely directed problem.
///
/// # Invariants
///
/// Every endpoint index is below [`node_count`](Self::node_count). A graph that
/// exists has no dangling edge, so the message-passing loop needs no bounds
/// check in its inner loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph {
    node_count: usize,
    /// Sender index of each directed edge.
    senders: Vec<usize>,
    /// Receiver index of each directed edge.
    receivers: Vec<usize>,
}

impl Graph {
    /// Builds a graph from a directed edge list.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if `senders` and `receivers` differ
    /// in length. [`RafflesError::InvalidParameter`] if `node_count` is zero or
    /// any endpoint index is out of range.
    pub fn new(node_count: usize, senders: Vec<usize>, receivers: Vec<usize>) -> Result<Self> {
        if senders.len() != receivers.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: senders.len(),
                found: receivers.len(),
            });
        }
        if node_count == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "node_count".to_string(),
                value: 0.0,
                reason: "a graph needs at least one node".to_string(),
            });
        }
        for index in senders.iter().chain(receivers.iter()) {
            if *index >= node_count {
                return Err(RafflesError::InvalidParameter {
                    parameter: "edge endpoint".to_string(),
                    value: *index as f64,
                    reason: format!("node index is outside 0..{node_count}"),
                });
            }
        }
        Ok(Self {
            node_count,
            senders,
            receivers,
        })
    }

    /// Builds a graph from undirected edges, storing each one in both
    /// directions.
    pub fn from_undirected_edges(node_count: usize, edges: &[(usize, usize)]) -> Result<Self> {
        let mut senders = Vec::with_capacity(edges.len() * 2);
        let mut receivers = Vec::with_capacity(edges.len() * 2);
        for (a, b) in edges {
            senders.push(*a);
            receivers.push(*b);
            senders.push(*b);
            receivers.push(*a);
        }
        Self::new(node_count, senders, receivers)
    }

    /// Builds a **radius graph**: every pair of points closer than `radius` is
    /// connected.
    ///
    /// This is how a mesh or a particle cloud becomes a message-passing graph,
    /// and `radius` is the parameter the physics-guided bound is most sensitive
    /// to: it sets how far one message hop travels in physical space, and
    /// therefore how many hops are needed to cross the domain.
    ///
    /// `points` are rows of coordinates, all of the same dimension. The
    /// implementation is the honest `O(n^2)` pairwise scan — a spatial index
    /// would be faster, and would be worth adding when a caller brings a
    /// hundred thousand nodes, but the constant-factor win is not worth the
    /// index's own correctness risk at the sizes this is used at today.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `points` is empty, if a
    /// coordinate is not finite, or if `radius` is not strictly positive.
    /// [`RafflesError::DimensionMismatch`] if the rows are ragged.
    pub fn radius_graph(points: &[Vec<f64>], radius: f64) -> Result<Self> {
        if points.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "points".to_string(),
                value: 0.0,
                reason: "a radius graph needs at least one point".to_string(),
            });
        }
        if !(radius > 0.0) {
            return Err(RafflesError::InvalidParameter {
                parameter: "radius".to_string(),
                value: radius,
                reason: "the connectivity radius must be strictly positive".to_string(),
            });
        }
        let d = points[0].len();
        for row in points {
            if row.len() != d {
                return Err(RafflesError::DimensionMismatch {
                    expected: d,
                    found: row.len(),
                });
            }
            for value in row {
                if !value.is_finite() {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "point coordinate".to_string(),
                        value: *value,
                        reason: "coordinates must be finite".to_string(),
                    });
                }
            }
        }

        let mut edges = Vec::new();
        let radius_squared = radius * radius;
        for i in 0..points.len() {
            for j in (i + 1)..points.len() {
                let mut distance_squared = 0.0;
                for k in 0..d {
                    let delta = points[i][k] - points[j][k];
                    distance_squared += delta * delta;
                }
                if distance_squared <= radius_squared {
                    edges.push((i, j));
                }
            }
        }
        Self::from_undirected_edges(points.len(), &edges)
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Number of directed edges — twice the undirected count for a graph built
    /// by [`from_undirected_edges`](Self::from_undirected_edges).
    pub fn edge_count(&self) -> usize {
        self.senders.len()
    }

    /// Sender index of each directed edge.
    pub fn senders(&self) -> &[usize] {
        &self.senders
    }

    /// Receiver index of each directed edge.
    pub fn receivers(&self) -> &[usize] {
        &self.receivers
    }

    /// Adjacency lists, one per node, listing the nodes it can send to.
    pub fn adjacency(&self) -> Vec<Vec<usize>> {
        let mut adjacency = vec![Vec::new(); self.node_count];
        for (sender, receiver) in self.senders.iter().zip(self.receivers.iter()) {
            adjacency[*sender].push(*receiver);
        }
        adjacency
    }

    /// Hop distance from `source` to every node, with [`usize::MAX`] for nodes
    /// it cannot reach.
    ///
    /// Plain breadth-first search. Unreachable nodes are marked rather than
    /// omitted, because "this node is unreachable" is exactly the diagnosis the
    /// under-reaching analysis needs and silently dropping it would hide a
    /// disconnected mesh.
    pub fn hop_distances(&self, source: usize) -> Result<Vec<usize>> {
        if source >= self.node_count {
            return Err(RafflesError::InvalidParameter {
                parameter: "source".to_string(),
                value: source as f64,
                reason: format!("node index is outside 0..{}", self.node_count),
            });
        }
        let adjacency = self.adjacency();
        let mut distance = vec![usize::MAX; self.node_count];
        distance[source] = 0;
        let mut frontier = vec![source];
        let mut next = Vec::new();
        let mut depth = 0usize;
        while !frontier.is_empty() {
            depth += 1;
            for node in frontier.drain(..) {
                for neighbour in &adjacency[node] {
                    if distance[*neighbour] == usize::MAX {
                        distance[*neighbour] = depth;
                        next.push(*neighbour);
                    }
                }
            }
            core::mem::swap(&mut frontier, &mut next);
        }
        Ok(distance)
    }

    /// Greatest hop distance from `source` to any node it can reach.
    pub fn eccentricity(&self, source: usize) -> Result<usize> {
        Ok(self
            .hop_distances(source)?
            .into_iter()
            .filter(|d| *d != usize::MAX)
            .max()
            .unwrap_or(0))
    }

    /// Whether every node can reach every other.
    ///
    /// A disconnected message-passing graph is almost always a modelling
    /// mistake — it means part of the domain can never influence another part,
    /// at any number of iterations — so this is worth checking before trusting
    /// a bound computed from a diameter.
    pub fn is_connected(&self) -> bool {
        match self.hop_distances(0) {
            Ok(distances) => distances.iter().all(|d| *d != usize::MAX),
            Err(_) => false,
        }
    }

    /// The graph's **diameter** in hops: the greatest hop distance between any
    /// two connected nodes.
    ///
    /// This is the quantity the elliptic and parabolic bounds are built on: it
    /// is exactly the number of message-passing iterations needed for
    /// information to cross the whole domain once.
    ///
    /// Computed by a breadth-first search from every node, which is `O(n * e)`.
    /// For the mesh sizes involved in choosing a hyperparameter that is
    /// immaterial; for a very large graph, use
    /// [`diameter_estimate`](Self::diameter_estimate), which is honest about
    /// being a lower bound.
    pub fn diameter(&self) -> usize {
        (0..self.node_count)
            .filter_map(|source| self.eccentricity(source).ok())
            .max()
            .unwrap_or(0)
    }

    /// A cheap **lower bound** on the diameter, by double sweep.
    ///
    /// Two breadth-first searches: from an arbitrary node to its furthest, then
    /// from there. Exact on trees and usually exact or near-exact on meshes,
    /// but it is a *lower* bound in general — which matters here, because using
    /// it in place of [`diameter`](Self::diameter) can only ever *understate*
    /// the required number of message-passing iterations. The method name says
    /// "estimate" for that reason; do not substitute it silently.
    pub fn diameter_estimate(&self) -> usize {
        let first = match self.hop_distances(0) {
            Ok(d) => d,
            Err(_) => return 0,
        };
        let furthest = first
            .iter()
            .enumerate()
            .filter(|(_, d)| **d != usize::MAX)
            .max_by_key(|(_, d)| **d)
            .map(|(index, _)| index)
            .unwrap_or(0);
        self.eccentricity(furthest).unwrap_or(0)
    }

    /// A **contact graph** over spheres: two spheres are connected when the gap
    /// between their surfaces is at most `skin`.
    ///
    /// The difference from [`radius_graph`](Self::radius_graph) is that each
    /// sphere carries its own radius, so the connection test is
    /// `|x_i - x_j| <= r_i + r_j + skin` rather than a single global distance.
    /// That is the right test for a polydisperse packing — a bed of 1 mm and
    /// 3 mm pebbles has no single radius that is correct for both — and it is
    /// the graph a granular DEM code's force network actually lives on.
    ///
    /// `skin` is the usual neighbour-list margin: zero connects only spheres
    /// that are already touching or overlapping, and a positive value includes
    /// pairs that are close enough to come into contact within the next few
    /// steps. Passing zero to analyse a static packing is correct; passing zero
    /// to build a graph that will be reused across timesteps is not.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if `centres` and `radii` differ in
    /// length or the centre rows are ragged.
    /// [`RafflesError::InvalidParameter`] if there are no spheres, if a value
    /// is not finite, if a radius is not strictly positive, or if `skin` is
    /// negative.
    pub fn contact_graph(centres: &[Vec<f64>], radii: &[f64], skin: f64) -> Result<Self> {
        if centres.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "centres".to_string(),
                value: 0.0,
                reason: "a contact graph needs at least one sphere".to_string(),
            });
        }
        if centres.len() != radii.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: centres.len(),
                found: radii.len(),
            });
        }
        if skin < 0.0 || !skin.is_finite() {
            return Err(RafflesError::InvalidParameter {
                parameter: "skin".to_string(),
                value: skin,
                reason: "the neighbour-list skin must be finite and non-negative".to_string(),
            });
        }
        let d = centres[0].len();
        for (row, radius) in centres.iter().zip(radii.iter()) {
            if row.len() != d {
                return Err(RafflesError::DimensionMismatch {
                    expected: d,
                    found: row.len(),
                });
            }
            if !(*radius > 0.0) || !radius.is_finite() {
                return Err(RafflesError::InvalidParameter {
                    parameter: "radius".to_string(),
                    value: *radius,
                    reason: "a sphere radius must be finite and strictly positive".to_string(),
                });
            }
            for value in row {
                if !value.is_finite() {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "centre coordinate".to_string(),
                        value: *value,
                        reason: "coordinates must be finite".to_string(),
                    });
                }
            }
        }

        let mut edges = Vec::new();
        for i in 0..centres.len() {
            for j in (i + 1)..centres.len() {
                let mut distance_squared = 0.0;
                for k in 0..d {
                    let delta = centres[i][k] - centres[j][k];
                    distance_squared += delta * delta;
                }
                let reach = radii[i] + radii[j] + skin;
                if distance_squared <= reach * reach {
                    edges.push((i, j));
                }
            }
        }
        Self::from_undirected_edges(centres.len(), &edges)
    }

    /// `copies` disjoint copies of this graph, as one graph.
    ///
    /// The standard way to train a graph network on a batch of samples that
    /// share a topology: the copies never exchange messages, because no edge
    /// crosses between them, so one forward pass over the union is exactly
    /// `copies` independent forward passes — and is far faster than running
    /// them one at a time.
    ///
    /// Node `i` of copy `c` is node `c * node_count + i` in the result, which
    /// is the layout [`crate::gnn::training`] relies on when it stacks sample
    /// features.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `copies` is zero.
    pub fn repeat(&self, copies: usize) -> Result<Self> {
        if copies == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "copies".to_string(),
                value: 0.0,
                reason: "a batched graph needs at least one copy".to_string(),
            });
        }
        let mut senders = Vec::with_capacity(self.senders.len() * copies);
        let mut receivers = Vec::with_capacity(self.receivers.len() * copies);
        for copy in 0..copies {
            let offset = copy * self.node_count;
            for (s, r) in self.senders.iter().zip(self.receivers.iter()) {
                senders.push(s + offset);
                receivers.push(r + offset);
            }
        }
        Self::new(self.node_count * copies, senders, receivers)
    }

    /// The **receptive field** of a node after `iterations` message-passing
    /// steps: the set of nodes whose information can have reached it.
    ///
    /// One message-passing iteration moves information one hop, so this is the
    /// hop-ball of that radius. Counting it is the most direct way to see
    /// under-reaching: if the receptive field after the model's chosen number
    /// of iterations does not cover the nodes that physically influence the
    /// answer, no amount of training will fix the prediction.
    pub fn receptive_field(&self, node: usize, iterations: usize) -> Result<Vec<usize>> {
        let distances = self.hop_distances(node)?;
        Ok(distances
            .into_iter()
            .enumerate()
            .filter(|(_, d)| *d != usize::MAX && *d <= iterations)
            .map(|(index, _)| index)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path graph `0 - 1 - 2 - ... - (n-1)`.
    fn path(n: usize) -> Graph {
        let edges: Vec<(usize, usize)> = (0..n - 1).map(|i| (i, i + 1)).collect();
        Graph::from_undirected_edges(n, &edges).unwrap()
    }

    /// **Methodology.** A path of `n` nodes has diameter `n - 1` by
    /// construction, and every node's eccentricity is its distance to the
    /// furthest end. Checked for `n = 7`.
    ///
    /// **Result** (2026-09-16): diameter 6; eccentricity 6 at both ends and 3
    /// in the middle; the double-sweep estimate also gives 6.
    #[test]
    fn a_path_has_the_diameter_it_should() {
        let graph = path(7);
        assert_eq!(graph.diameter(), 6);
        assert_eq!(graph.eccentricity(0).unwrap(), 6);
        assert_eq!(graph.eccentricity(6).unwrap(), 6);
        assert_eq!(graph.eccentricity(3).unwrap(), 3);
        assert_eq!(graph.diameter_estimate(), 6);
        assert!(graph.is_connected());
    }

    /// **Methodology.** A radius graph over a regular 5x5 lattice of spacing
    /// 0.25 with a radius just above the spacing must connect exactly the
    /// 4-neighbours, giving 40 undirected edges (2 * 5 * 4) and therefore 80
    /// directed ones, and a diameter of 8 hops (4 across plus 4 down).
    ///
    /// **Result** (2026-09-16): 80 directed edges, diameter 8 — both exact.
    #[test]
    fn a_lattice_radius_graph_has_the_expected_connectivity() {
        let mut points = Vec::new();
        for i in 0..5 {
            for j in 0..5 {
                points.push(vec![i as f64 * 0.25, j as f64 * 0.25]);
            }
        }
        let graph = Graph::radius_graph(&points, 0.26).unwrap();
        assert_eq!(graph.node_count(), 25);
        assert_eq!(graph.edge_count(), 80);
        assert_eq!(graph.diameter(), 8);
    }

    /// **Methodology — the quantity the whole bound rests on.** Widening the
    /// connectivity radius must shrink the diameter, because each hop covers
    /// more ground. The same 5x5 lattice at radius 0.26, 0.36 (adds diagonals)
    /// and 0.51 (adds the second ring).
    ///
    /// **Result** (2026-09-16): printed under `--nocapture`; the diameter falls
    /// monotonically.
    #[test]
    fn a_wider_radius_shrinks_the_diameter() {
        let mut points = Vec::new();
        for i in 0..5 {
            for j in 0..5 {
                points.push(vec![i as f64 * 0.25, j as f64 * 0.25]);
            }
        }
        let mut previous = usize::MAX;
        let mut row = Vec::new();
        for radius in [0.26, 0.36, 0.51] {
            let diameter = Graph::radius_graph(&points, radius).unwrap().diameter();
            row.push((radius, diameter));
            assert!(diameter <= previous, "diameter grew at radius {radius}");
            previous = diameter;
        }
        println!("5x5 lattice diameter against connectivity radius: {row:?}");
    }

    /// **Methodology.** A disconnected graph must be reported as such, and its
    /// unreachable nodes must come back marked rather than omitted — a
    /// disconnected mesh is a modelling error that must not be silently
    /// smoothed over.
    ///
    /// **Result** (2026-09-16): `is_connected` false; the far component's
    /// distance is `usize::MAX`.
    #[test]
    fn a_disconnected_graph_is_reported_not_hidden() {
        let graph = Graph::from_undirected_edges(4, &[(0, 1), (2, 3)]).unwrap();
        assert!(!graph.is_connected());
        let distances = graph.hop_distances(0).unwrap();
        assert_eq!(distances[1], 1);
        assert_eq!(distances[2], usize::MAX);
    }

    /// **Methodology.** The receptive field after `k` iterations must be the
    /// hop-ball of radius `k`. On a 7-node path from the middle node: 1 node at
    /// `k = 0`, 3 at `k = 1`, 5 at `k = 2`, all 7 at `k >= 3`.
    ///
    /// **Result** (2026-09-16): exactly those counts.
    #[test]
    fn the_receptive_field_is_the_hop_ball() {
        let graph = path(7);
        for (iterations, expected) in [(0, 1), (1, 3), (2, 5), (3, 7), (10, 7)] {
            assert_eq!(
                graph.receptive_field(3, iterations).unwrap().len(),
                expected,
                "iterations {iterations}"
            );
        }
    }

    /// **Methodology.** Malformed graphs must be refused: mismatched edge
    /// arrays, zero nodes, an out-of-range endpoint, an empty point set, a
    /// non-positive radius, and ragged points.
    ///
    /// **Result.** All six rejected (2026-09-16).
    #[test]
    fn malformed_graphs_are_refused() {
        assert!(Graph::new(3, vec![0, 1], vec![1]).is_err());
        assert!(Graph::new(0, vec![], vec![]).is_err());
        assert!(Graph::new(2, vec![0], vec![5]).is_err());
        assert!(Graph::radius_graph(&[], 0.1).is_err());
        assert!(Graph::radius_graph(&[vec![0.0]], 0.0).is_err());
        assert!(Graph::radius_graph(&[vec![0.0], vec![0.0, 1.0]], 0.1).is_err());
    }
}
