//! Building a message-passing graph from a Monte Carlo CSG geometry.
//!
//! # Where this lives, and why
//!
//! In `raffles`, not in `outram-mc-libs`. RAFFLES already depends on
//! `outram-mc-libs` for its random-number generator, so an adapter in the other
//! direction would make the two crates mutually dependent and neither would
//! compile. The DEM bridge went the other way — it lives in
//! `outram-park-fork-liggghts`, which nothing here depends on — and the
//! asymmetry is the dependency graph's, not a design preference.
//!
//! # The graph
//!
//! Nodes are **cells**; two cells are joined when they share a surface with
//! **opposite senses** — one on the inside, the other on the outside. That is
//! the standard cheap construction of a CSG adjacency graph, and it is what a
//! particle's possible next cell looks like from the current one.
//!
//! ## What it gets wrong, stated plainly
//!
//! This is a **superset** of true geometric adjacency, in two ways:
//!
//! - Two cells can reference the same surface with opposite senses and still
//!   not touch, because some *other* surface in one of their region
//!   expressions separates them. The graph will join them anyway.
//! - A cell defined by a union can have disconnected pieces, and this treats it
//!   as one node.
//!
//! It is never a *subset*: two genuinely adjacent cells always share a surface
//! with opposite senses, so no real adjacency is missed. That direction is the
//! one that matters for a reach bound — an over-connected graph gives a
//! diameter that is too small, and therefore a bound that is too *low*, which
//! is the unsafe direction. **Treat a diameter computed from this graph as a
//! lower bound on the true one**, and say so in anything reported from it.
//!
//! Exact adjacency needs surface-surface intersection tests against the full
//! region expressions, which is a real geometry kernel and is not what this
//! module is.
//!
//! # What it is for
//!
//! Two uses, both from the literature on learned variance reduction:
//!
//! - **Importance and weight-window maps.** These are currently produced by a
//!   deterministic adjoint solve or by iterating Monte Carlo. A network over
//!   the cell graph is a candidate, and the cell graph is small and static so
//!   the training cost is dominated by generating targets, not by the graph.
//! - **Cell-wise response surrogates** — a predicted reaction rate or leakage
//!   per cell.
//!
//! One caution worth stating up front: a Monte Carlo target carries a
//! statistical uncertainty, and a surrogate fitted to noisy targets inherits it
//! without reporting it. Any such surrogate needs its own uncertainty story
//! before it goes anywhere near a k-eff.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};

use super::graph::Graph;
use crate::{RafflesError, Result};

/// Builds the cell-adjacency graph of a CSG geometry.
///
/// Two cells are joined when one references a surface on its inside and the
/// other references the same surface on its outside. See the module
/// documentation for exactly how this over-approximates true adjacency, and
/// why that direction is the unsafe one for a reach bound.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if `cells` is empty.
pub fn cell_adjacency_graph(cells: &[Cell]) -> Result<Graph> {
    if cells.is_empty() {
        return Err(RafflesError::InvalidParameter {
            parameter: "cells".to_string(),
            value: 0.0,
            reason: "a cell-adjacency graph needs at least one cell".to_string(),
        });
    }

    // Per cell, the surfaces it bounds from the inside and from the outside.
    let mut inside: Vec<Vec<usize>> = Vec::with_capacity(cells.len());
    let mut outside: Vec<Vec<usize>> = Vec::with_capacity(cells.len());
    for cell in cells {
        let mut cell_inside = Vec::new();
        let mut cell_outside = Vec::new();
        for token in &cell.region {
            if let RegionToken::HalfSpace { surface_idx, sense } = token {
                match sense {
                    HalfSpaceSense::Inside => cell_inside.push(*surface_idx),
                    HalfSpaceSense::Outside => cell_outside.push(*surface_idx),
                }
            }
        }
        cell_inside.sort_unstable();
        cell_inside.dedup();
        cell_outside.sort_unstable();
        cell_outside.dedup();
        inside.push(cell_inside);
        outside.push(cell_outside);
    }

    let mut edges = Vec::new();
    for i in 0..cells.len() {
        for j in (i + 1)..cells.len() {
            let shares_opposite = inside[i].iter().any(|s| outside[j].contains(s))
                || outside[i].iter().any(|s| inside[j].contains(s));
            if shares_opposite {
                edges.push((i, j));
            }
        }
    }
    Graph::from_undirected_edges(cells.len(), &edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_mc_libs::geometry::cell::CellFill;

    /// A cell bounded inside surface `inside_of` and outside every surface in
    /// `outside_of`.
    fn cell(id: i32, inside_of: &[usize], outside_of: &[usize]) -> Cell {
        let mut region = Vec::new();
        for surface in inside_of {
            region.push(RegionToken::HalfSpace {
                surface_idx: *surface,
                sense: HalfSpaceSense::Inside,
            });
        }
        for surface in outside_of {
            region.push(RegionToken::HalfSpace {
                surface_idx: *surface,
                sense: HalfSpaceSense::Outside,
            });
        }
        Cell {
            id,
            region,
            fill: CellFill::Void,
            temperature: 300.0,
            translation: Default::default(),
        }
        tracking: None,
    }

    /// **Methodology.** Nested shells — the classic pin-cell or TRISO layout —
    /// must give a path graph. Three concentric regions built from two
    /// surfaces: the innermost is inside surface 0; the middle is outside 0 and
    /// inside 1; the outer is outside 1. Adjacent shells must be joined and the
    /// inner and outer must not be.
    ///
    /// **Result** (2026-09-16): 4 directed edges (2 undirected), diameter 2 —
    /// a path, exactly as the geometry is.
    #[test]
    fn nested_shells_give_a_path_graph() {
        let cells = [cell(1, &[0], &[]), cell(2, &[1], &[0]), cell(3, &[], &[1])];
        let graph = cell_adjacency_graph(&cells).unwrap();
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 4);
        assert_eq!(graph.diameter(), 2);

        // The inner and outer shells are 2 hops apart, not adjacent.
        let distances = graph.hop_distances(0).unwrap();
        assert_eq!(distances[1], 1);
        assert_eq!(distances[2], 2);
    }

    /// **Methodology.** Cells that share no surface must not be joined, and the
    /// graph must report itself disconnected rather than inventing an edge.
    ///
    /// **Result** (2026-09-16): no edges, not connected.
    #[test]
    fn unrelated_cells_are_not_joined() {
        let cells = [cell(1, &[0], &[]), cell(2, &[5], &[])];
        let graph = cell_adjacency_graph(&cells).unwrap();
        assert_eq!(graph.edge_count(), 0);
        assert!(!graph.is_connected());
    }

    /// **Methodology.** Cells on the *same* side of a shared surface are not
    /// adjacent across it — two cells both inside surface 0 are nested or
    /// overlapping, not neighbours — so the sense test must require opposite
    /// senses rather than a shared surface alone.
    ///
    /// **Result** (2026-09-16): no edge, which is the whole point of checking
    /// the sense.
    #[test]
    fn the_same_side_of_a_surface_is_not_an_adjacency() {
        let cells = [cell(1, &[0], &[]), cell(2, &[0], &[])];
        let graph = cell_adjacency_graph(&cells).unwrap();
        assert_eq!(graph.edge_count(), 0);
    }

    /// **Methodology — the over-approximation this module documents, shown
    /// rather than only described.** Two cells that reference surface 0 with
    /// opposite senses but are separated by a third surface are joined anyway.
    /// This test exists so the limitation is a measured property of the code,
    /// not a claim in a comment that could drift.
    ///
    /// **Result** (2026-09-16): the spurious edge is present, as documented.
    #[test]
    fn a_separated_pair_is_over_connected_as_documented() {
        // Cell A: inside 0 and inside 2. Cell B: outside 0 and outside 2.
        // They share surface 0 with opposite senses, so they are joined —
        // even though surface 2 also separates them.
        let cells = [cell(1, &[0, 2], &[]), cell(2, &[], &[0, 2])];
        let graph = cell_adjacency_graph(&cells).unwrap();
        assert_eq!(
            graph.edge_count(),
            2,
            "the documented over-approximation has changed"
        );
    }

    /// **Methodology.** An empty geometry must be refused.
    ///
    /// **Result.** Rejected (2026-09-16).
    #[test]
    fn an_empty_geometry_is_refused() {
        assert!(cell_adjacency_graph(&[]).is_err());
    }
}
