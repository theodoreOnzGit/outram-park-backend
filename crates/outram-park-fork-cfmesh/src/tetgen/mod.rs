// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (module root):
//   TetGen — A Quality Tetrahedral Mesh Generator and 3D Delaunay Triangulator
//   https://github.com/libigl/tetgen, commit e05aca7df74e (TetGen 1.6.0)
//   Copyright (C) 2002-2020 Hang Si, Weierstrass Institute for Applied
//   Analysis and Stochastics (WIAS), Berlin.
//   Licence: GNU Affero General Public License v3.0 or later.
//
// This file is part of OUTRAM PARK. It is free software: you can redistribute
// it and/or modify it under the terms of the GNU Affero General Public License
// as published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version. It is distributed WITHOUT
// ANY WARRANTY; see the GNU Affero General Public License for details.

//! # `tetgen` — a port of TetGen 1.6.0 (epic gh:#1789)
//!
//! Hang Si's TetGen (constrained Delaunay tetrahedralisation and quality
//! refinement) is being ported here, as a module of this crate, from
//! `libigl/tetgen` at commit `e05aca7df74e` (TetGen 1.6.0, AGPL-3.0-or-later).
//! ~~TetGen is AGPL and off-limits (`op-hzs.32`)~~: the maintainer's
//! 2026-10-20 direction makes AGPL acceptable (root `CLAUDE.md`), so the
//! earlier avoidance recorded in [`crate::tet`] no longer applies.
//!
//! ## Stage plan (epic gh:#1789)
//!
//! Each stage is checked **bit-identically** against upstream (the upstream
//! binary or a C harness compiled against the upstream source) before the next
//! one builds on it.
//!
//! | Stage | Issue | Content | Status |
//! |---|---|---|---|
//! | 1 | gh:#1790 | exact geometric predicates (`predicates.cxx`) | **done: [`predicates`]** |
//! | 2 | gh:#1791 | point-set Delaunay (`transfernodes`, BRIO/Hilbert sort, `incrementaldelaunay`, `insert_vertex_bw`) and the mesh data structure | **done: [`mesh`], [`delaunay`]** |
//! | 3 | gh:#1792 | PLC surface meshing (`meshsurface`) | not started |
//! | 4 | gh:#1793 | boundary recovery (`recoverboundary`) | not started |
//! | 5 | gh:#1794 | `carveholes`, `suppresssteinerpoints`, `recoverdelaunay` | not started |
//! | 6 | gh:#1795 | Delaunay refinement and mesh improvement (`-q`, `-a`) | not started |
//! | 7 | gh:#1796 | TetGen as the tet source for cfMesh's tet → dual → layers, and in Mesh Studio | not started |
//!
//! Stage 2 adds the mesh object ([`mesh::TetgenMesh`]: pools, records,
//! `triface` and every orientation table, with indices for pointers) and
//! [`delaunay_tetrahedralize`], whose `.node`/`.ele` output is identical to
//! `tetgen -zQ` on twelve cases (`verification_and_validation/tetgen_delaunay_gh1791/`).
//!
//! **The pool order is part of the answer.** Upstream numbers its output by
//! walking its memory pools in allocation order and reuses freed records
//! last-in first-out; [`mesh::MemoryPool`] reproduces both, and anything that
//! creates or frees a tetrahedron or a point must go through it
//! ([`mesh::TetgenMesh::maketetrahedron2`], `tetrahedrondealloc`,
//! `makepoint`, `pointdealloc`), never a `Vec::push`. Likewise the C
//! library's `rand()` (glibc, [`mesh::GlibcRand`]) and TetGen's own
//! `randomnation` are part of the answer.
//!
//! ## Why the predicates are ported, not reused
//!
//! Two exact-predicate implementations already exist in the workspace and were
//! checked first (search-before-building rule):
//!
//! - `outram-blender/src/boolean_predicates.rs` returns signs only, through
//!   double-double arithmetic;
//! - [`crate::delaunay`]'s `orient3d` / `insphere` are plain `f64`.
//!
//! Neither can stand in: TetGen uses the predicates' **magnitudes** (the
//! returned determinant, e.g. as a volume), and its own static filter and
//! `-X` options decide which value is returned. Only a line-for-line port
//! reproduces those values to the bit.

pub mod delaunay;
pub mod mesh;
pub mod predicates;

pub use delaunay::{
    delaunay_tetrahedralize, delaunay_tetrahedralize_with, TetgenError, Tetrahedralization,
};
pub use mesh::{Behavior, TetgenMesh};
