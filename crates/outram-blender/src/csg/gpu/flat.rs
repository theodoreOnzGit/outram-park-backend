// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. GPL-3.0-only; see LICENSE.
//
// NEW WORK (gh:#587), no upstream counterpart: a flat, GPU-uploadable copy of
// an assembled [`Geometry`]. The surface tag order and coefficient layout are
// those of `outram-mc-libs`' `gpu::surface_distance::encode_surfaces`
// (op-9s8.2), so the two GPU encodings of one surface agree; that crate cannot
// be a dependency of this one (it depends on us), so the layout is restated,
// not shared.

//! **The assembled CSG geometry as one `u32` buffer**, for the GPU ray tracer
//! in [`super::render`].
//!
//! Everything [`Geometry::locate`] and [`Geometry::distance_to_boundary`]
//! read is copied, nothing is derived: surfaces (tag + coefficients), cells
//! (region tokens in RPN, fill, translation), universes (cell lists, in search
//! order), rectangular and hexagonal lattices (their universe maps and
//! `outer`), and the root universe. The GPU therefore walks the solver's own
//! geometry, which is what the crate's drawing rule asks of a picture.
//!
//! Floats are stored as `f32` bit patterns (`f32::to_bits`), so the GPU path
//! is an `f32` accelerator of the trusted `f64` CPU plotter
//! ([`crate::csg::plot`]), never the reference.
//!
//! # Layout (all offsets in `u32` words from the start of the buffer)
//!
//! | words | content |
//! |---|---|
//! | header, [`HEADER_WORDS`] | see the `H_*` constants |
//! | surfaces, [`SURF_WORDS`] each | `[tag, c0 .. c10]` |
//! | cells, [`CELL_WORDS`] each | `[tok_start, tok_len, fill_kind, fill_idx, tx, ty, tz, flags]` |
//! | universes, 4 each | `[cellidx_start, n_cells, hint_start, n_hints]` |
//! | lattices, [`LAT_WORDS`] each | rect: `[0, nx, ny, nz, llx, lly, llz, px, py, pz, outer, map_start]`; hex: `[1, orient, n_rings, n_axial, cx, cy, cz, pitch_r, pitch_z, 0, outer, map_start]` |
//! | region tokens | half-space `(surface << 1) \| outside`, or [`TOK_INTERSECTION`] / [`TOK_UNION`] / [`TOK_COMPLEMENT`] |
//! | cell indices | the universes' cell lists |
//! | lattice maps | universe per tile (`i32`, `-1` = none) |
//! | crossing hints | per universe, sorted `[half-space, cand_start, n_cand]` triples, then the candidate cell lists |
//!
//! **Crossing hints** are OpenMC's neighbour lists in a static form: after a
//! ray leaves a cell across surface `s` onto side `σ`, the cell it enters
//! is, in a geometry whose cells do not overlap, one of the universe's cells
//! whose region names the half-space `(s, σ)`; the shader tries those first
//! and scans the whole universe only if none holds the point. Same answer as
//! the full scan for non-overlapping cells (which `Universe::find_cell`
//! assumes anyway: it returns the first match), at a fraction of the cost in
//! HTR-10's 128-cell root universe.
//!
//! `outer` and empty tiles are `-1` (as `u32`, [`NONE`]) when absent.

use crate::csg::cell::{CellFill, HalfSpaceSense, RegionToken};
use crate::csg::geometry::Geometry;
use crate::csg::lattice::{HexOrientation, Lattice};
use crate::csg::surface::SurfaceKind;

/// Words of header at the start of [`FlatGeometry::words`].
pub const HEADER_WORDS: usize = 16;
/// Words per surface: a tag and 11 coefficients.
pub const SURF_WORDS: usize = 12;
/// Words per cell.
pub const CELL_WORDS: usize = 8;
/// Words per universe.
pub const UNI_WORDS: usize = 4;
/// Words per lattice.
pub const LAT_WORDS: usize = 12;
/// Deepest nesting the GPU tracer holds (`MAX_DEPTH` in the shader): root
/// universe plus nine nested levels. HTR-10's explicit bed uses four.
pub const MAX_DEPTH: usize = 10;
/// Deepest boolean stack a non-trivial region may need (the shader keeps it
/// in the bits of one `u32`).
pub const MAX_REGION_STACK: usize = 32;

/// Region-token encoding of [`RegionToken::Intersection`].
pub const TOK_INTERSECTION: u32 = 0xFFFF_FFF0;
/// Region-token encoding of [`RegionToken::Union`].
pub const TOK_UNION: u32 = 0xFFFF_FFF1;
/// Region-token encoding of [`RegionToken::Complement`].
pub const TOK_COMPLEMENT: u32 = 0xFFFF_FFF2;
/// "Absent" (`-1` as `u32`): no outer universe, an empty tile, no lattice.
pub const NONE: u32 = u32::MAX;

/// Cell flag: the region is a well-formed AND of half-spaces only, so the
/// shader may test it with an early-out loop instead of the RPN stack.
pub const CELL_SIMPLE: u32 = 1;

/// Header word: format version ([`FORMAT_VERSION`]).
pub const H_VERSION: usize = 0;
/// Header word: root universe index.
pub const H_ROOT: usize = 1;
/// Header words: counts and offsets of each section.
pub const H_N_SURF: usize = 2;
/// See [`H_N_SURF`].
pub const H_SURF: usize = 3;
/// See [`H_N_SURF`].
pub const H_N_CELL: usize = 4;
/// See [`H_N_SURF`].
pub const H_CELL: usize = 5;
/// See [`H_N_SURF`].
pub const H_N_UNI: usize = 6;
/// See [`H_N_SURF`].
pub const H_UNI: usize = 7;
/// See [`H_N_SURF`].
pub const H_N_LAT: usize = 8;
/// See [`H_N_SURF`].
pub const H_LAT: usize = 9;
/// See [`H_N_SURF`].
pub const H_TOK: usize = 10;
/// See [`H_N_SURF`].
pub const H_CELLIDX: usize = 11;
/// See [`H_N_SURF`].
pub const H_LATMAP: usize = 12;
/// Header word: deepest nesting found (levels, root = 1).
pub const H_DEPTH: usize = 13;
/// Header word: material table length ([`crate::csg::plot::material_count`]).
pub const H_N_MAT: usize = 14;
/// Header word: offset of the crossing-hint section.
pub const H_HINTS: usize = 15;

/// Version of this layout; the shader is written against it.
pub const FORMAT_VERSION: u32 = 2;

/// Why a geometry cannot be flattened. Every case means "draw it on the CPU".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlattenError {
    /// A torus (`XTorus`/`YTorus`/`ZTorus`) at this surface index: the GPU
    /// tracer has no quartic solver. The CPU plotter draws tori.
    TorusUnsupported(usize),
    /// A cell's region needs a deeper boolean stack than [`MAX_REGION_STACK`].
    RegionTooDeep {
        /// Cell index.
        cell: usize,
    },
    /// Universes nest deeper than [`MAX_DEPTH`] levels (or a fill refers to
    /// itself, which would nest without end).
    TooDeep(usize),
    /// An index does not fit the 32-bit encoding.
    TooLarge,
}

impl std::fmt::Display for FlattenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TorusUnsupported(i) => write!(f, "surface {i} is a torus (no GPU solver)"),
            Self::RegionTooDeep { cell } => write!(f, "cell {cell}'s region is too deep"),
            Self::TooDeep(d) => write!(f, "geometry nests {d} levels (GPU limit {MAX_DEPTH})"),
            Self::TooLarge => write!(f, "geometry too large for 32-bit indices"),
        }
    }
}

impl std::error::Error for FlattenError {}

/// A [`Geometry`] flattened for the GPU ray tracer. Built by [`flatten`].
#[derive(Debug, Clone, PartialEq)]
pub struct FlatGeometry {
    /// The buffer (see the module docs for the layout).
    pub words: Vec<u32>,
    /// Materials the cells fill with (`material_count`): the colour table
    /// the renderer needs.
    pub n_materials: usize,
    /// Deepest nesting found, in levels (root = 1).
    pub depth: usize,
    /// What each universe can show: the materials of its own cells, and the
    /// universes and lattices its cells are filled with.
    pub universe_contents: Vec<UniverseContents>,
    /// The distinct universes each lattice places (tiles and `outer`).
    pub lattice_universes: Vec<Vec<usize>>,
}

/// One universe's direct contents (see [`FlatGeometry::universe_contents`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UniverseContents {
    /// Material indices of its material cells.
    pub materials: Vec<usize>,
    /// Universes its cells are filled with.
    pub universes: Vec<usize>,
    /// Lattices its cells are filled with.
    pub lattices: Vec<usize>,
}

impl FlatGeometry {
    /// **Which universes and lattices hold anything drawn**, given which
    /// materials are drawn: the words the GPU tracer uses to skip a whole
    /// subtree of hidden materials (a pebble whose TRISO particles, matrix
    /// and shell are all hidden is crossed as one cell, not as hundreds of
    /// lattice tiles). Universes first, then lattices; 1 = something shown.
    #[must_use]
    pub fn subtree_shown(&self, shown: &[bool]) -> Vec<u32> {
        let nu = self.universe_contents.len();
        let mut memo: Vec<Option<bool>> = vec![None; nu];
        fn uni(f: &FlatGeometry, u: usize, shown: &[bool], memo: &mut Vec<Option<bool>>) -> bool {
            if let Some(v) = memo[u] {
                return v;
            }
            // Cycles are refused by `flatten`; mark before descending anyway.
            memo[u] = Some(false);
            let c = &f.universe_contents[u];
            let v = c
                .materials
                .iter()
                .any(|&m| shown.get(m).copied().unwrap_or(false))
                || c.universes.iter().any(|&v| uni(f, v, shown, memo))
                || c.lattices.iter().any(|&l| {
                    f.lattice_universes[l]
                        .iter()
                        .any(|&v| uni(f, v, shown, memo))
                });
            memo[u] = Some(v);
            v
        }
        let mut out: Vec<u32> = (0..nu)
            .map(|u| u32::from(uni(self, u, shown, &mut memo)))
            .collect();
        for l in &self.lattice_universes {
            out.push(u32::from(l.iter().any(|&v| uni(self, v, shown, &mut memo))));
        }
        out
    }

    /// Size of the buffer in bytes.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.words.len() * 4
    }

    /// The buffer as little-endian bytes, for upload.
    #[must_use]
    pub fn to_le_bytes(&self) -> Vec<u8> {
        self.words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }
}

fn f(x: f64) -> u32 {
    (x as f32).to_bits()
}

fn idx(i: usize) -> Result<u32, FlattenError> {
    u32::try_from(i)
        .ok()
        .filter(|&v| v < TOK_INTERSECTION >> 1)
        .ok_or(FlattenError::TooLarge)
}

fn opt(i: Option<usize>) -> Result<u32, FlattenError> {
    i.map_or(Ok(NONE), idx)
}

/// Encode one surface as `[tag, c0 .. c10]`; tags follow [`SurfaceKind`]'s
/// variant order (as in `outram-mc-libs`' `encode_surfaces`).
fn encode_surface(i: usize, s: &SurfaceKind) -> Result<[u32; SURF_WORDS], FlattenError> {
    let mut w = [0u32; SURF_WORDS];
    let (tag, c): (u32, Vec<f64>) = match s {
        SurfaceKind::XPlane(p) => (0, vec![p.x0]),
        SurfaceKind::YPlane(p) => (1, vec![p.y0]),
        SurfaceKind::ZPlane(p) => (2, vec![p.z0]),
        SurfaceKind::Plane(p) => (3, vec![p.a, p.b, p.c, p.d]),
        SurfaceKind::Sphere(p) => (4, vec![p.x0, p.y0, p.z0, p.r]),
        SurfaceKind::XCylinder(p) => (5, vec![p.y0, p.z0, p.r]),
        SurfaceKind::YCylinder(p) => (6, vec![p.x0, p.z0, p.r]),
        SurfaceKind::ZCylinder(p) => (7, vec![p.x0, p.y0, p.r]),
        SurfaceKind::XCone(p) => (8, vec![p.x0, p.y0, p.z0, p.r_sq]),
        SurfaceKind::YCone(p) => (9, vec![p.x0, p.y0, p.z0, p.r_sq]),
        SurfaceKind::ZCone(p) => (10, vec![p.x0, p.y0, p.z0, p.r_sq]),
        SurfaceKind::Quadric(p) => (11, vec![p.a, p.b, p.c, p.d, p.e, p.f, p.g, p.h, p.j, p.k]),
        SurfaceKind::XTorus(_) | SurfaceKind::YTorus(_) | SurfaceKind::ZTorus(_) => {
            return Err(FlattenError::TorusUnsupported(i))
        }
    };
    w[0] = tag;
    for (k, v) in c.iter().enumerate() {
        w[1 + k] = f(*v);
    }
    Ok(w)
}

/// Encode a region; return the tokens and whether it is a plain AND of
/// half-spaces (the shader's fast path). Mirrors the stack discipline of
/// [`crate::csg::cell::Cell::contains`]: a region that underflows, or ends
/// with more than one operand, is not "simple" and goes through the stack.
fn encode_region(cell: usize, region: &[RegionToken]) -> Result<(Vec<u32>, bool), FlattenError> {
    let mut out = Vec::with_capacity(region.len());
    let mut depth: i64 = 0;
    let mut max_depth: i64 = 0;
    let mut only_and = true;
    let mut underflow = false;
    for t in region {
        match *t {
            RegionToken::HalfSpace { surface_idx, sense } => {
                let s = idx(surface_idx)?;
                out.push((s << 1) | u32::from(sense == HalfSpaceSense::Outside));
                depth += 1;
            }
            RegionToken::Intersection => {
                out.push(TOK_INTERSECTION);
                underflow |= depth < 2;
                depth -= 1;
            }
            RegionToken::Union => {
                out.push(TOK_UNION);
                only_and = false;
                underflow |= depth < 2;
                depth -= 1;
            }
            RegionToken::Complement => {
                out.push(TOK_COMPLEMENT);
                only_and = false;
                underflow |= depth < 1;
            }
        }
        max_depth = max_depth.max(depth);
    }
    if max_depth > MAX_REGION_STACK as i64 {
        return Err(FlattenError::RegionTooDeep { cell });
    }
    let simple = only_and && !underflow && (region.is_empty() || depth == 1);
    Ok((out, simple))
}

/// Deepest nesting below universe `u`, in levels (a universe of material
/// cells only is 1). `memo` caches it per universe; `on_stack` catches a fill
/// that contains itself.
fn universe_depth(
    g: &Geometry,
    u: usize,
    memo: &mut Vec<Option<usize>>,
    on_stack: &mut Vec<bool>,
) -> Result<usize, FlattenError> {
    if let Some(d) = memo[u] {
        return Ok(d);
    }
    if on_stack[u] {
        return Err(FlattenError::TooDeep(usize::MAX));
    }
    on_stack[u] = true;
    let mut deepest = 1;
    for &c in &g.universes[u].cell_indices {
        let below = match g.cells[c].fill {
            CellFill::Material(_) | CellFill::Void => 0,
            CellFill::Universe(v) => universe_depth(g, v, memo, on_stack)?,
            CellFill::Lattice(l) => {
                let (map, outer): (Vec<usize>, Option<usize>) = match &g.lattices[l] {
                    Lattice::Rect(r) => (r.universes.clone(), r.outer),
                    Lattice::Hex(h) => (
                        h.universes
                            .iter()
                            .filter(|&&v| v >= 0)
                            .map(|&v| v as usize)
                            .collect(),
                        h.outer,
                    ),
                };
                let mut seen = std::collections::BTreeSet::new();
                let mut d = 0;
                for v in map.into_iter().chain(outer) {
                    if seen.insert(v) {
                        d = d.max(universe_depth(g, v, memo, on_stack)?);
                    }
                }
                d
            }
        };
        deepest = deepest.max(1 + below);
    }
    on_stack[u] = false;
    memo[u] = Some(deepest);
    Ok(deepest)
}

/// **Flatten an assembled geometry for the GPU tracer.**
///
/// # Errors
/// [`FlattenError`] when the geometry uses something the GPU tracer does not
/// carry (a torus, a region deeper than [`MAX_REGION_STACK`], nesting deeper
/// than [`MAX_DEPTH`]) or does not fit 32-bit indices. The caller draws on
/// the CPU instead.
pub fn flatten(g: &Geometry) -> Result<FlatGeometry, FlattenError> {
    let mut memo = vec![None; g.universes.len()];
    let mut on_stack = vec![false; g.universes.len()];
    let depth = universe_depth(g, g.root_universe, &mut memo, &mut on_stack)?;
    if depth > MAX_DEPTH {
        return Err(FlattenError::TooDeep(depth));
    }

    let mut surf = Vec::with_capacity(g.surfaces.len() * SURF_WORDS);
    for (i, s) in g.surfaces.iter().enumerate() {
        surf.extend_from_slice(&encode_surface(i, s)?);
    }

    let mut toks: Vec<u32> = Vec::new();
    let mut cells = Vec::with_capacity(g.cells.len() * CELL_WORDS);
    for (i, c) in g.cells.iter().enumerate() {
        let (t, simple) = encode_region(i, &c.region)?;
        let (kind, fill) = match c.fill {
            CellFill::Material(m) => (0u32, idx(m)?),
            CellFill::Universe(u) => (1, idx(u)?),
            CellFill::Lattice(l) => (2, idx(l)?),
            CellFill::Void => (3, NONE),
        };
        cells.extend_from_slice(&[
            idx(toks.len())?,
            idx(t.len())?,
            kind,
            fill,
            f(c.translation.x),
            f(c.translation.y),
            f(c.translation.z),
            if simple { CELL_SIMPLE } else { 0 },
        ]);
        toks.extend(t);
    }

    let mut cellidx: Vec<u32> = Vec::new();
    let mut unis = Vec::with_capacity(g.universes.len() * UNI_WORDS);
    // Per universe: half-space key (surface << 1 | outside) -> the cells, in
    // search order, whose region names it.
    let mut hint_keys: Vec<u32> = Vec::new();
    let mut hint_cands: Vec<Vec<u32>> = Vec::new();
    for u in &g.universes {
        let mut by_key: std::collections::BTreeMap<u32, Vec<u32>> = Default::default();
        for &c in &u.cell_indices {
            for t in &g.cells[c].region {
                if let RegionToken::HalfSpace { surface_idx, sense } = *t {
                    let key =
                        (idx(surface_idx)? << 1) | u32::from(sense == HalfSpaceSense::Outside);
                    let list = by_key.entry(key).or_default();
                    if list.last() != Some(&idx(c)?) {
                        list.push(idx(c)?);
                    }
                }
            }
        }
        unis.extend_from_slice(&[
            idx(cellidx.len())?,
            idx(u.cell_indices.len())?,
            idx(hint_keys.len() / 3)?,
            idx(by_key.len())?,
        ]);
        for &c in &u.cell_indices {
            cellidx.push(idx(c)?);
        }
        for (key, list) in by_key {
            hint_keys.extend_from_slice(&[key, 0, idx(list.len())?]);
            hint_cands.push(list);
        }
    }
    // Candidate lists follow the key triples; fix up their starts.
    let mut hints = hint_keys;
    for (k, list) in hint_cands.into_iter().enumerate() {
        hints[3 * k + 1] = idx(hints.len())?;
        hints.extend(list);
    }

    let mut latmap: Vec<u32> = Vec::new();
    let mut lats = Vec::with_capacity(g.lattices.len() * LAT_WORDS);
    for l in &g.lattices {
        let start = idx(latmap.len())?;
        match l {
            Lattice::Rect(r) => {
                lats.extend_from_slice(&[
                    0,
                    idx(r.n[0])?,
                    idx(r.n[1])?,
                    idx(r.n[2])?,
                    f(r.lower_left.x),
                    f(r.lower_left.y),
                    f(r.lower_left.z),
                    f(r.pitch[0]),
                    f(r.pitch[1]),
                    f(r.pitch[2]),
                    opt(r.outer)?,
                    start,
                ]);
                for &u in &r.universes {
                    latmap.push(idx(u)?);
                }
            }
            Lattice::Hex(h) => {
                lats.extend_from_slice(&[
                    1,
                    u32::from(h.orientation == HexOrientation::X),
                    idx(h.n_rings)?,
                    idx(h.n_axial)?,
                    f(h.center.x),
                    f(h.center.y),
                    f(h.center.z),
                    f(h.pitch[0]),
                    f(h.pitch[1]),
                    0,
                    opt(h.outer)?,
                    start,
                ]);
                for &u in &h.universes {
                    latmap.push(if u < 0 { NONE } else { idx(u as usize)? });
                }
            }
        }
    }

    let n_materials = crate::csg::plot::material_count(g);
    let mut words = vec![0u32; HEADER_WORDS];
    let section = |words: &mut Vec<u32>, data: &[u32]| -> Result<u32, FlattenError> {
        let at = idx(words.len())?;
        words.extend_from_slice(data);
        Ok(at)
    };
    let surf_at = section(&mut words, &surf)?;
    let cell_at = section(&mut words, &cells)?;
    let uni_at = section(&mut words, &unis)?;
    let lat_at = section(&mut words, &lats)?;
    let tok_at = section(&mut words, &toks)?;
    let cellidx_at = section(&mut words, &cellidx)?;
    let latmap_at = section(&mut words, &latmap)?;
    let hints_at = section(&mut words, &hints)?;
    words[H_VERSION] = FORMAT_VERSION;
    words[H_ROOT] = idx(g.root_universe)?;
    words[H_N_SURF] = idx(g.surfaces.len())?;
    words[H_SURF] = surf_at;
    words[H_N_CELL] = idx(g.cells.len())?;
    words[H_CELL] = cell_at;
    words[H_N_UNI] = idx(g.universes.len())?;
    words[H_UNI] = uni_at;
    words[H_N_LAT] = idx(g.lattices.len())?;
    words[H_LAT] = lat_at;
    words[H_TOK] = tok_at;
    words[H_CELLIDX] = cellidx_at;
    words[H_LATMAP] = latmap_at;
    words[H_DEPTH] = idx(depth)?;
    words[H_N_MAT] = idx(n_materials)?;
    words[H_HINTS] = hints_at;
    let universe_contents = g
        .universes
        .iter()
        .map(|u| {
            let mut c = UniverseContents::default();
            for &i in &u.cell_indices {
                match g.cells[i].fill {
                    CellFill::Material(m) => c.materials.push(m),
                    CellFill::Universe(v) => c.universes.push(v),
                    CellFill::Lattice(l) => c.lattices.push(l),
                    CellFill::Void => {}
                }
            }
            for v in [&mut c.materials, &mut c.universes, &mut c.lattices] {
                v.sort_unstable();
                v.dedup();
            }
            c
        })
        .collect();
    let lattice_universes = g
        .lattices
        .iter()
        .map(|l| {
            let mut v: Vec<usize> = match l {
                Lattice::Rect(r) => r.universes.iter().copied().chain(r.outer).collect(),
                Lattice::Hex(h) => h
                    .universes
                    .iter()
                    .filter(|&&u| u >= 0)
                    .map(|&u| u as usize)
                    .chain(h.outer)
                    .collect(),
            };
            v.sort_unstable();
            v.dedup();
            v
        })
        .collect();
    Ok(FlatGeometry {
        words,
        n_materials,
        depth,
        universe_contents,
        lattice_universes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::cell::Cell;
    use crate::csg::position::Position;
    use crate::csg::surface::{BoundaryType, Sphere, XTorus};
    use crate::csg::universe::Universe;

    fn ball() -> Geometry {
        let s = SurfaceKind::Sphere(Sphere {
            x0: 1.0,
            y0: 2.0,
            z0: 3.0,
            r: 4.0,
            bc: BoundaryType::Vacuum,
        });
        let inside = RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        };
        Geometry {
            surfaces: vec![s],
            cells: vec![Cell::material(1, vec![inside], 2, 293.6)],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0],
            }],
            lattices: vec![],
            root_universe: 0,
        }
    }

    /// The header points at each section and the sphere's coefficients land
    /// where the shader reads them.
    #[test]
    fn a_sphere_cell_round_trips() {
        let flat = flatten(&ball()).unwrap();
        let w = &flat.words;
        assert_eq!(w[H_VERSION], FORMAT_VERSION);
        assert_eq!(w[H_N_SURF], 1);
        let s = w[H_SURF] as usize;
        assert_eq!(w[s], 4, "sphere tag");
        assert_eq!(f32::from_bits(w[s + 4]), 4.0);
        let c = w[H_CELL] as usize;
        assert_eq!(w[c + 2], 0, "material fill");
        assert_eq!(w[c + 3], 2, "material index");
        assert_eq!(w[c + 7], CELL_SIMPLE);
        let t = w[H_TOK] as usize + w[c] as usize;
        assert_eq!(w[t], 0, "half-space of surface 0, inside");
        assert_eq!(flat.n_materials, 3);
        assert_eq!(flat.depth, 1);
    }

    /// A union is not the AND fast path; an underflowing region is not either.
    #[test]
    fn only_well_formed_ands_are_simple() {
        let hs = |i, sense| RegionToken::HalfSpace {
            surface_idx: i,
            sense,
        };
        let and = [
            hs(0, HalfSpaceSense::Inside),
            hs(1, HalfSpaceSense::Outside),
            RegionToken::Intersection,
        ];
        assert!(encode_region(0, &and).unwrap().1);
        let or = [
            hs(0, HalfSpaceSense::Inside),
            hs(1, HalfSpaceSense::Outside),
            RegionToken::Union,
        ];
        assert!(!encode_region(0, &or).unwrap().1);
        let two_left = [hs(0, HalfSpaceSense::Inside), hs(1, HalfSpaceSense::Inside)];
        assert!(!encode_region(0, &two_left).unwrap().1);
        assert!(!encode_region(0, &[RegionToken::Intersection]).unwrap().1);
    }

    /// A torus is refused (the CPU plotter draws it), not drawn wrongly.
    #[test]
    fn a_torus_is_refused() {
        let mut g = ball();
        g.surfaces.push(SurfaceKind::XTorus(XTorus {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            a: 2.0,
            b: 1.0,
            c: 1.0,
            bc: BoundaryType::Transmissive,
        }));
        assert_eq!(flatten(&g), Err(FlattenError::TorusUnsupported(1)));
    }

    /// A universe that fills itself would nest forever: refused.
    #[test]
    fn a_self_fill_is_refused() {
        let mut g = ball();
        g.cells
            .push(Cell::fill(2, vec![], CellFill::Universe(0), Position::ZERO));
        g.universes[0].cell_indices.push(1);
        assert!(matches!(flatten(&g), Err(FlattenError::TooDeep(_))));
    }
}
