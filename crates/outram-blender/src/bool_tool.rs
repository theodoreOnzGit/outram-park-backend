// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// The Bool Tool: a managed, non-destructive stack of "brush" cutter meshes.
// Blender analogue (architecture only): the bundled `object_boolean_tools`
// add-on — non-destructive brush booleans (difference / union / intersect /
// slice) with an auto-boolean bake and a fast carve mode. No upstream source
// copied; wraps this crate's `boolean` CSG entry point.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! **Bool Tool** (`op-hzs.54.45`, GH issue #37 §I) — a non-destructive stack of
//! brush cutters over a base mesh.
//!
//! - [`BrushOp`] — Difference / Union / Intersect / Slice.
//! - [`BoolBrush`] — one cutter: an `Arc<Mesh>`, its op, and an `enabled`
//!   toggle. Nothing is applied until [`BoolStack::bake`].
//! - [`BoolStack`] — the ordered stack. [`BoolStack::bake`] folds every enabled
//!   brush into the base; [`BoolStack::slice_pieces`] returns the inside piece
//!   each `Slice` brush carves off as a separate mesh.
//! - **Carve mode** ([`BoolStack::carve`]) — a fast, best-effort bake that
//!   skips a brush the CSG cannot resolve instead of failing the whole stack.
//!
//! The base and brushes are unchanged by any call here — that is what makes
//! the stack "non-destructive"; `bake` returns a fresh [`Mesh`].

use std::sync::Arc;

use crate::boolean::{boolean, BooleanError};
use crate::mesh::Mesh;
use crate::ops::BooleanMode;

/// What a brush does to the base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushOp {
    /// Subtract the brush volume (`base \ brush`).
    Difference,
    /// Add the brush volume (`base ∪ brush`).
    Union,
    /// Keep only the shared volume (`base ∩ brush`).
    Intersect,
    /// Keep the base outside the brush, and carve the inside off as a
    /// separate piece (see [`BoolStack::slice_pieces`]).
    Slice,
}

/// One non-destructive cutter.
#[derive(Debug, Clone)]
pub struct BoolBrush {
    /// The cutter geometry (shared, never mutated).
    pub mesh: Arc<Mesh>,
    /// What it does.
    pub op: BrushOp,
    /// Skipped by [`BoolStack::bake`] when `false`.
    pub enabled: bool,
}

impl BoolBrush {
    /// A new enabled brush.
    pub fn new(mesh: Arc<Mesh>, op: BrushOp) -> Self {
        BoolBrush {
            mesh,
            op,
            enabled: true,
        }
    }
}

/// An ordered stack of brushes over a base mesh.
#[derive(Debug, Clone, Default)]
pub struct BoolStack {
    /// The brushes, applied in order by [`BoolStack::bake`].
    pub brushes: Vec<BoolBrush>,
}

impl BoolStack {
    /// An empty stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a brush and return `self` (builder style).
    pub fn with(mut self, brush: BoolBrush) -> Self {
        self.brushes.push(brush);
        self
    }

    /// Add a brush in place.
    pub fn push(&mut self, brush: BoolBrush) {
        self.brushes.push(brush);
    }

    /// Fold every **enabled** brush into `base`, in stack order, and return the
    /// result. A `Slice` brush contributes its *outside* part here (the inside
    /// piece is available from [`BoolStack::slice_pieces`]).
    ///
    /// Errors on the first brush the CSG cannot resolve — use
    /// [`BoolStack::carve`] to skip such a brush instead.
    pub fn bake(&self, base: &Mesh) -> Result<Mesh, BooleanError> {
        let mut acc = base.clone();
        for b in self.brushes.iter().filter(|b| b.enabled) {
            acc = apply(&acc, &b.mesh, b.op)?;
        }
        Ok(acc)
    }

    /// Like [`BoolStack::bake`], but a brush whose boolean fails is **skipped**
    /// (fast carve mode). Returns the baked mesh plus the indices of the
    /// brushes that were skipped.
    pub fn carve(&self, base: &Mesh) -> (Mesh, Vec<usize>) {
        let mut acc = base.clone();
        let mut skipped = Vec::new();
        for (i, b) in self.brushes.iter().enumerate() {
            if !b.enabled {
                continue;
            }
            match apply(&acc, &b.mesh, b.op) {
                Ok(next) => acc = next,
                Err(_) => skipped.push(i),
            }
        }
        (acc, skipped)
    }

    /// The inside piece (`base ∩ brush`) that each enabled `Slice` brush carves
    /// off, in stack order. Each is evaluated against the base as baked by the
    /// brushes *before* it, matching [`BoolStack::bake`].
    pub fn slice_pieces(&self, base: &Mesh) -> Vec<Result<Mesh, BooleanError>> {
        let mut acc = base.clone();
        let mut pieces = Vec::new();
        for b in self.brushes.iter().filter(|b| b.enabled) {
            match b.op {
                BrushOp::Slice => {
                    pieces.push(boolean(&acc, &b.mesh, BooleanMode::Intersect));
                    // The running base keeps only the outside part.
                    if let Ok(outside) = boolean(&acc, &b.mesh, BooleanMode::Difference) {
                        acc = outside;
                    }
                }
                other => {
                    if let Ok(next) = apply(&acc, &b.mesh, other) {
                        acc = next;
                    }
                }
            }
        }
        pieces
    }
}

fn apply(base: &Mesh, brush: &Mesh, op: BrushOp) -> Result<Mesh, BooleanError> {
    match op {
        BrushOp::Difference => boolean(base, brush, BooleanMode::Difference),
        BrushOp::Union => boolean(base, brush, BooleanMode::Union),
        BrushOp::Intersect => boolean(base, brush, BooleanMode::Intersect),
        // Slice, when baked into the base, leaves the outside part.
        BrushOp::Slice => boolean(base, brush, BooleanMode::Difference),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec3;
    use crate::primitives;

    /// A cube of `size` translated by `t`.
    fn cube_at(size: f64, t: Vec3) -> Mesh {
        let m = primitives::cube(size);
        let positions: Vec<Vec3> = m.positions().iter().map(|p| p.add(t)).collect();
        let faces: Vec<Vec<usize>> = m
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        Mesh::from_polygons(&positions, &faces)
    }

    #[test]
    fn difference_brush_removes_volume() {
        let base = primitives::cube(2.0);
        let brush = Arc::new(cube_at(1.0, Vec3::new(1.0, 1.0, 1.0)));
        let stack = BoolStack::new().with(BoolBrush::new(brush, BrushOp::Difference));
        let out = stack.bake(&base).unwrap();
        assert!(out.face_count() > 6, "corner was carved, adding faces");
        assert_eq!(out.euler_characteristic(), 2);
    }

    #[test]
    fn disabled_brush_is_a_no_op() {
        let base = primitives::cube(2.0);
        let mut brush = BoolBrush::new(
            Arc::new(cube_at(1.0, Vec3::new(1.0, 1.0, 1.0))),
            BrushOp::Difference,
        );
        brush.enabled = false;
        let out = BoolStack::new().with(brush).bake(&base).unwrap();
        assert_eq!(out.face_count(), base.face_count());
    }

    #[test]
    fn two_brushes_fold_in_order() {
        let base = primitives::cube(4.0);
        let stack = BoolStack::new()
            .with(BoolBrush::new(
                Arc::new(cube_at(1.5, Vec3::new(2.0, 2.0, 2.0))),
                BrushOp::Difference,
            ))
            .with(BoolBrush::new(
                Arc::new(cube_at(1.5, Vec3::new(-2.0, -2.0, -2.0))),
                BrushOp::Difference,
            ));
        let out = stack.bake(&base).unwrap();
        assert_eq!(out.euler_characteristic(), 2);
        assert!(out.face_count() > base.face_count());
    }

    #[test]
    fn carve_mode_skips_the_unresolvable_brush() {
        let base = primitives::cube(2.0);
        // A coplanar-face cutter the CSG rejects.
        let coplanar = Arc::new(cube_at(2.0, Vec3::new(2.0, 0.0, 0.0)));
        let good = Arc::new(cube_at(1.0, Vec3::new(1.0, 1.0, 1.0)));
        let stack = BoolStack::new()
            .with(BoolBrush::new(coplanar, BrushOp::Difference))
            .with(BoolBrush::new(good, BrushOp::Difference));
        assert!(
            stack.bake(&base).is_err(),
            "strict bake fails on the coplanar brush"
        );
        let (out, skipped) = stack.carve(&base);
        assert_eq!(skipped, vec![0]);
        assert!(
            out.face_count() > base.face_count(),
            "the good brush still applied"
        );
    }

    #[test]
    fn slice_returns_the_inside_piece_separately() {
        let base = primitives::cube(3.0);
        let brush = Arc::new(cube_at(1.2, Vec3::new(1.5, 1.5, 1.5)));
        let stack = BoolStack::new().with(BoolBrush::new(brush, BrushOp::Slice));

        let baked = stack.bake(&base).unwrap();
        assert!(
            baked.face_count() > base.face_count(),
            "base keeps the outside, cut"
        );

        let pieces = stack.slice_pieces(&base);
        assert_eq!(pieces.len(), 1);
        let inside = pieces[0].as_ref().unwrap();
        assert_eq!(inside.euler_characteristic(), 2);
        // The inside piece is smaller than the base.
        let (lo, hi) = crate::measure::bounding_box(inside);
        assert!(hi.x - lo.x < 3.0 - 1e-6);
    }
}
