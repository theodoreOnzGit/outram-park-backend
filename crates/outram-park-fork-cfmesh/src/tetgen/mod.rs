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
//! | 3 | gh:#1792 | PLC input (`.poly`/`.smesh`/`.edge`) and surface meshing (`meshsurface`) | **done: [`plc`], [`surface`]** |
//! | 4 | gh:#1793 | boundary recovery (`recoverboundary`) | **done: [`flip`], [`recover`], [`steiner`]** |
//! | 5 | gh:#1794 | the rest of `-p`: `carveholes`, `suppresssteinerpoints`, `recoverdelaunay`, `smooth_vertices`, `improve_mesh`, `jettisonnodes`, the `.node`/`.ele`/`.face`/`.edge` output, [`to_volume_mesh`] | **done: [`carve`], [`optimize`], [`output`], [`tetrahedralize`]** |
//! | 6 | gh:#1795 | Delaunay refinement (`-q`, `-a`); ~~mesh improvement~~ (ported in stage 5: plain `-p` runs it) | **done: [`refine`], [`quality`]** |
//! | 7 | gh:#1796 | TetGen as the tet source for cfMesh's tet → dual → layers, and in Mesh Studio | **done: [`crate::pipeline::TetSource::TetGen`]** (`pipeline/tet_source.rs`; the crate default stays the carved route) |
//!
//! Stage 2 adds the mesh object ([`mesh::TetgenMesh`]: pools, records,
//! `triface` and every orientation table, with indices for pointers) and
//! [`delaunay_tetrahedralize`], whose `.node`/`.ele` output is identical to
//! `tetgen -zQ` on twelve cases (`verification_and_validation/tetgen_delaunay_gh1791/`).
//!
//! Stage 3 adds the PLC input ([`PlcInput`]: points, facets as polygon lists
//! with markers and hole points, volume holes, regions, input edges;
//! [`PlcInput::read_poly`], [`PlcInput::read_smesh`] and
//! [`PlcInput::read_edge`] follow `tetgenio::load_poly` / `load_edge` token
//! for token; [`PlcInput::from_triangle_soup`] builds a PLC from a
//! triangulated surface with one marker per triangle, the route later stages
//! take for cfMesh and Blender surfaces), the subface and subsegment pools
//! ([`mesh::ShellRecord`], [`mesh::Face`]) and [`plc_meshsurface`]: the `-p`
//! Delaunay of the PLC's vertices, then `meshsurface` (each facet a 2D
//! constrained Delaunay triangulation; coplanar facets of one marker merged).
//! Its state after `meshsurface` (every subface, subsegment, tetrahedron and
//! point slot with its links, the dead-item stacks and the random state)
//! equals an instrumented upstream's on thirteen cases
//! (`verification_and_validation/tetgen_meshsurface_gh1792/`).
//!
//! Stage 4 adds boundary recovery ([`tetrahedralize_plc`]: `tetgen -p` up to
//! and including `recoverboundary`): the tetrahedron-subface and
//! tetrahedron-segment links with their `tet2subpool` / `tet2segpool`, the 3D
//! flips (`flip23`, `flip32`, `flip41`, `flipnm`, `flipnm_post`,
//! `removeedgebyflips`, `removefacebyflips`, [`flip`]), segment and facet
//! recovery by flips (`recoveredgebyflips`, `recoverfacebyflips`,
//! `recoversegments`, `recoversubfaces`, [`recover`]) and the Steiner-point
//! fallbacks (`insertpoint`, `add_steinerpt_*`, `removevertexbyflips`,
//! `sremovevertex`, `recoverdelaunay`, [`steiner`]). Its state after
//! recovery equals an instrumented upstream's on 19 cases under `-p` and
//! `-pY` (`verification_and_validation/tetgen_recoverboundary_gh1793/`). The
//! CDT routines (`-D`: `delaunizesegments`, `formcavity`, `fillcavity`, ...)
//! are not reached by `recoverboundary` and are not ported.
//!
//! Stage 5 completes `tetgen -p` ([`tetrahedralize_plc`] now runs the whole
//! route; [`Switches`] builds `-p`, `-z`, `-Y`, `-A`, ...): `carveholes`
//! (exterior and volume holes removed, region attributes with `-A`, hull
//! slivers peeled), Steiner-point suppression, `recoverdelaunay`,
//! `smooth_vertices`, `improve_mesh` (**plain `-p` runs it**:
//! `tetrahedralize()` calls it under `b->plc`, and without `-q` its dihedral
//! bound is 179.9 degrees), `jettisonnodes` and the outputs
//! ([`TetgenOutput`], rendered as upstream's files). Its `.node`, `.ele`,
//! `.face` and `.edge` files equal the unmodified upstream binary's byte for
//! byte on 48 runs (21 cases x `-pz`, `-pzY`, and `-pzA`, `-pzAY` on three)
//! (`verification_and_validation/tetgen_plc_gh1794/`). [`to_volume_mesh`]
//! gives this crate's `VolumeMesh`, one patch per facet marker.
//!
//! Stage 6 adds Delaunay refinement ([`refine`]: `delaunayrefinement`,
//! `repairencsegs`, `repairencfacs`, `repairbadtets`, the encroachment
//! tests, `split_segment` / `split_subface` / `split_tetrahedron`, the
//! facet and segment maps; `insertpoint`'s `rejflag`, insertion radii and
//! `chkencflag` queues; [`Switches::quality`], [`Switches::max_volume`])
//! and the `-V` statistics ([`quality`]). Under `-pq`, `-pq<r>/<a>`,
//! `-pqa<V>` and `-pqA` its `.node`, `.ele`, `.face` and `.edge` files
//! equal the unmodified upstream binary's byte for byte on 41 runs, with
//! 64 reach counters equal (`verification_and_validation/tetgen_quality_gh1795/`).
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

pub mod carve;
pub mod delaunay;
pub mod flip;
pub mod mesh;
pub mod optimize;
pub mod output;
pub mod plc;
pub mod predicates;
pub mod recover;
pub mod quality;
pub mod refine;
pub mod steiner;
pub mod surface;
pub mod tetrahedralize;

pub use delaunay::{
    delaunay_tetrahedralize, delaunay_tetrahedralize_with, TetgenError, Tetrahedralization,
};
pub use mesh::{Behavior, TetgenMesh};
pub use plc::{PlcFacet, PlcInput, PlcReadError};
pub use recover::{tetrahedralize_plc, PlcTetrahedralization};
pub use steiner::InsertPointFlags;
pub use output::{fmt_g17, to_volume_mesh, TetgenOutput};
pub use surface::{plc_meshsurface, plc_meshsurface_with, PlcSurface};
pub use tetrahedralize::Switches;
