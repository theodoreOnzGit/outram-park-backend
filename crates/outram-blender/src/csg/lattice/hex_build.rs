// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/lattice.cpp, include/openmc/lattice.h
// Hexagonal lattice: ring-order construction.
// Moved here unchanged in substance from `outram-mc-libs`
// (`src/geometry/`) on 2026-10-02, GitHub issue #486: outram-blender owns the
// CSG description and its pure navigation kernel, outram-mc-libs keeps the
// transport-state work and re-exports these items under its old paths.
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

//! Hexagonal lattice construction from OpenMC's ring order
//! (`HexLattice.universes`, `fill_lattice_x` / `fill_lattice_y`); split out of
//! the lattice module, 2026-10-02.

#[allow(unused_imports)]
use crate::csg::position::{Direction, Position};
use super::hex::*;

impl HexLattice {
    /// Build a 2-D hexagonal lattice from the user-facing **ring** description,
    /// mirroring the OpenMC Python `HexLattice.universes = [[ring], …]` setter.
    ///
    /// `rings` is ordered **outermost ring first**, and within each ring the
    /// elements are listed clockwise starting at the "top" (see the notebook's
    /// `show_indices`). The outer ring has `6*(n_rings-1)` elements, the next
    /// `6*(n_rings-2)`, …, and the innermost "ring" is the single central tile.
    /// Each entry is a universe index. This routine walks the same skewed index
    /// path OpenMC's `fill_lattice_y` / `fill_lattice_x`
    /// (`src/lattice.cpp:546,598`) uses so the stored array matches OpenMC
    /// exactly. `outer` fills everything outside the hexagon.
    ///
    /// Panics if the ring sizes are inconsistent with a hexagon of
    /// `rings.len()` rings.
    pub fn from_rings(
        id: i32,
        orientation: HexOrientation,
        center: Position,
        radial_pitch: f64,
        rings: &[Vec<usize>],
        outer: Option<usize>,
    ) -> Self {
        let n_rings = rings.len();
        assert!(n_rings >= 1, "a hex lattice needs at least one ring");
        // Validate ring sizes: outer→inner is 6(n-1), 6(n-2), …, 1.
        for (k, ring) in rings.iter().enumerate() {
            let expected = if k == n_rings - 1 {
                1
            } else {
                6 * (n_rings - 1 - k)
            };
            assert_eq!(
                ring.len(),
                expected,
                "ring {k} (outer-first) has {} elements, expected {expected} for a {n_rings}-ring hex lattice",
                ring.len()
            );
        }
        let n_side = 2 * n_rings - 1;
        let mut universes = vec![HEX_NONE; n_side * n_side];
        // ~~Geometry-derived placement (op-6tz.38 fix)~~ CORRECTED 2026-09-26:
        // [`fill_level`] converts the rings to upstream's Python row order and
        // then runs the ported `fill_lattice_x/y`. (The op-6tz.38 walk
        // mis-placed ring-order input because it skipped that conversion; its
        // angular replacement round-tripped but started at +x and ran
        // anticlockwise, where OpenMC starts at the top and runs clockwise.)
        fill_level(n_rings, orientation, rings, &mut universes);

        HexLattice {
            id,
            orientation,
            n_rings,
            n_axial: 1,
            center,
            pitch: [radial_pitch, 0.0],
            universes,
            outer,
        }
    }

    /// Build a **3-D** hexagonal lattice: `levels.len()` axially-stacked copies
    /// of a hexagonal ring fill, one full ring description per axial level.
    ///
    /// Mirrors OpenMC's 3-D `HexLattice` input, where `universes` is nested one
    /// level deeper than the 2-D case — `[axial_level][ring][element]` — and the
    /// C++ `fill_lattice_x`/`fill_lattice_y` (`src/lattice.cpp:546,598`) wrap the
    /// 2-D ring walk in an outer axial `m` loop. Each axial level is an
    /// independent 2-D hexagonal fill written into its own `(2*n_rings-1)^2`
    /// slice of [`Self::universes`] at flat offset `(2*n_rings-1)^2 * iz` (see
    /// [`Self::flat_index`]). The 2-D [`Self::from_rings`] is the special case
    /// `levels.len() == 1`; a single-level call here reproduces its planar layout
    /// (only `n_axial`/`pitch[1]` differ).
    ///
    /// # Parameters
    /// - `id`, `orientation`, `center`, `outer` — as [`Self::from_rings`];
    ///   `center.z` is the axial centre of the whole stack, in cm.
    /// - `radial_pitch` — tile flat-to-flat pitch in cm (`pitch[0]`).
    /// - `axial_pitch` — height of one axial level in cm (`pitch[1]`).
    /// - `levels` — one entry per axial level. **`levels[0]` is the bottom level**
    ///   (`iz = 0`, lowest z; its centre sits at `center.z - (n_axial-1)/2 ·
    ///   axial_pitch`), matching the internal `iz` convention of
    ///   [`Self::get_indices`]/[`Self::center_offset`]/[`Self::distance`]. This is
    ///   the **reverse** of OpenMC's Python display (top-first) — flip the outer
    ///   list when porting a Python case. Every level must have the same ring
    ///   count; within a level the ring/element order is that of
    ///   [`Self::from_rings`] (outermost ring first, single central tile last).
    ///
    /// Panics if `levels` is empty, if the levels disagree on ring count, or if
    /// any ring size is inconsistent with a hexagon of that ring count.
    pub fn from_rings_3d(
        id: i32,
        orientation: HexOrientation,
        center: Position,
        radial_pitch: f64,
        axial_pitch: f64,
        levels: &[Vec<Vec<usize>>],
        outer: Option<usize>,
    ) -> Self {
        let n_axial = levels.len();
        assert!(n_axial >= 1, "a hex lattice needs at least one axial level");
        let n_rings = levels[0].len();
        assert!(n_rings >= 1, "a hex lattice needs at least one ring");

        let n_side = 2 * n_rings - 1;
        let block = n_side * n_side;
        let mut universes = vec![HEX_NONE; block * n_axial];

        for (iz, rings) in levels.iter().enumerate() {
            assert_eq!(
                rings.len(),
                n_rings,
                "axial level {iz} has {} rings, expected {n_rings} (all levels must have the same ring count)",
                rings.len()
            );
            // Validate ring sizes: outer→inner is 6(n-1), 6(n-2), …, 1.
            for (k, ring) in rings.iter().enumerate() {
                let expected = if k == n_rings - 1 {
                    1
                } else {
                    6 * (n_rings - 1 - k)
                };
                assert_eq!(
                    ring.len(),
                    expected,
                    "axial level {iz} ring {k} (outer-first) has {} elements, expected {expected} for a {n_rings}-ring hex lattice",
                    ring.len()
                );
            }
            let slice = &mut universes[iz * block..(iz + 1) * block];
            // Upstream's Python-rows-then-fill_lattice path; see from_rings.
            fill_level(n_rings, orientation, rings, slice);
        }

        HexLattice {
            id,
            orientation,
            n_rings,
            n_axial,
            center,
            pitch: [radial_pitch, axial_pitch],
            universes,
            outer,
        }
    }
}

/// Write one axial level's ring-nested universes into its `(2*n_rings-1)^2`
/// skewed slice, by upstream's Python-rows-then-`fill_lattice` path.
/// ~~using the geometry-derived `ring_slots` placement~~ (removed 2026-09-26,
/// see the correction in the body).
///
/// `rings` is outer-ring-first (as accepted by [`HexLattice::from_rings`]); the
/// outer ring has hex-radius `n_rings-1` and the innermost (single-tile) ring
/// radius `0`. `out` is a single already-`HEX_NONE`-filled block. The caller
/// (`from_rings`/`from_rings_3d`) validates ring sizes inline before calling.
pub(crate) fn fill_level(
    n_rings: usize,
    orientation: HexOrientation,
    rings: &[Vec<usize>],
    out: &mut [i32],
) {
    // CORRECTED 2026-09-26: upstream's own two-step path, not a geometric
    // guess. OpenMC's Python turns the ring lists into the XML row text
    // (`HexLattice._repr_axial_slice_{x,y}`, `openmc/lattice.py:1625-1835`)
    // and the C++ reads those words back with `fill_lattice_{x,y}`
    // (`src/lattice.cpp:555-660`). The earlier `ring_slots` placement
    // (op-6tz.38) put element 0 of each ring at the first tile anticlockwise
    // from +x and walked anticlockwise; upstream starts at the top and walks
    // clockwise. Found by the pixel comparison against OpenMC's `Model.plot`
    // (`verification_and_validation/python_plotting_parity/`).
    let words = python_hex_words(n_rings, orientation, rings);
    match orientation {
        HexOrientation::Y => fill_lattice_y(n_rings, &words, out),
        HexOrientation::X => fill_lattice_x(n_rings, &words, out),
    }
}

/// The universe words of one axial level in the order OpenMC's Python writes
/// them to `geometry.xml`: a port of `HexLattice._repr_axial_slice_y` /
/// `_repr_axial_slice_x` (`openmc/lattice.py:1625-1835`, commit d7d3284a1)
/// without the padding, which the C++ reader skips anyway.
pub(crate) fn python_hex_words(
    n_rings: usize,
    orientation: HexOrientation,
    rings: &[Vec<usize>],
) -> Vec<i32> {
    let n = n_rings;
    let u = |r_prime: usize, theta: usize| rings[r_prime][theta] as i32;
    let rows: Vec<Vec<i32>> = match orientation {
        HexOrientation::Y => {
            let mut rows: Vec<Vec<i32>> = vec![Vec::new(); 1 + 4 * (n - 1)];
            let middle = 2 * (n - 1);
            rows[middle] = vec![u(n - 1, 0)];
            for r in 1..n {
                let r_prime = n - 1 - r;
                let mut theta = 0;
                let mut y = middle + 2 * r;
                for _ in 0..r {
                    rows[y].push(u(r_prime, theta));
                    y -= 1;
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].push(u(r_prime, theta));
                    y -= 2;
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].push(u(r_prime, theta));
                    y -= 1;
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].insert(0, u(r_prime, theta));
                    y += 1;
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].insert(0, u(r_prime, theta));
                    y += 2;
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].insert(0, u(r_prime, theta));
                    y += 1;
                    theta += 1;
                }
            }
            // "Flip the rows" (`rows[::-1]`).
            rows.reverse();
            rows
        }
        HexOrientation::X => {
            let mut rows: Vec<Vec<i32>> = vec![Vec::new(); 2 * n - 1];
            let middle = n - 1;
            rows[middle] = vec![u(n - 1, 0)];
            for r in 1..n {
                let r_prime = n - 1 - r;
                let mut theta = 0;
                let mut y = middle;
                for _ in 0..r {
                    rows[y].push(u(r_prime, theta));
                    y += 1;
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].insert(0, u(r_prime, theta));
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].insert(0, u(r_prime, theta));
                    y -= 1;
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].insert(0, u(r_prime, theta));
                    y -= 1;
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].push(u(r_prime, theta));
                    theta += 1;
                }
                for _ in 0..r {
                    rows[y].push(u(r_prime, theta));
                    y += 1;
                    theta += 1;
                }
            }
            rows
        }
    };
    rows.into_iter().flatten().collect()
}

/// Fill the skewed universe array for a `'y'`-orientation hex lattice from the
/// flattened ring-order input. Ported from `HexLattice::fill_lattice_y`
/// (`src/lattice.cpp:598`), for a single axial level (2-D).
///
/// ~~No longer used by `from_rings`/`from_rings_3d` (op-6tz.38 replaced the
/// row-order walk with the geometry-derived [`fill_level`]); retained for
/// reference against the C++ source.~~ **CORRECTED 2026-09-26:** used again,
/// fed by [`python_hex_words`] — the row order upstream's Python writes, which
/// is what op-6tz.38's original row-order walk was missing.
pub(crate) fn fill_lattice_y(n_rings: usize, univ: &[i32], out: &mut [i32]) {
    let nr = n_rings as i32;
    let n_side = (2 * nr - 1) as usize;
    let mut input_index = 0usize;
    let mut i_x: i32 = 1;
    let mut i_a: i32 = nr - 1;

    // Upper triangular region (first n_rings-1 rows of input).
    for k in 0..(nr - 1) {
        i_x -= 1;
        for _ in 0..(k + 1) {
            let indx = n_side * (i_a + nr - 1) as usize + (i_x + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_x += 2;
            i_a -= 1;
        }
        i_x -= 2 * (k + 1);
        i_a += k + 1;
    }

    // Middle square region (next 2*n_rings-1 rows).
    for k in 0..(2 * nr - 1) {
        if k % 2 == 0 {
            i_x -= 1;
        } else {
            i_x += 1;
            i_a -= 1;
        }
        for _ in 0..(nr - (k % 2)) {
            let indx = n_side * (i_a + nr - 1) as usize + (i_x + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_x += 2;
            i_a -= 1;
        }
        i_x -= 2 * (nr - (k % 2));
        i_a += nr - (k % 2);
    }

    // Lower triangular region.
    for k in 0..(nr - 1) {
        i_x += 1;
        i_a -= 1;
        for _ in 0..(nr - k - 1) {
            let indx = n_side * (i_a + nr - 1) as usize + (i_x + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_x += 2;
            i_a -= 1;
        }
        i_x -= 2 * (nr - k - 1);
        i_a += nr - k - 1;
    }
}

/// Fill the skewed universe array for an `'x'`-orientation hex lattice. Ported
/// from `HexLattice::fill_lattice_x` (`src/lattice.cpp:546`), single axial level.
///
/// ~~No longer used by `from_rings`/`from_rings_3d`~~ **CORRECTED 2026-09-26:**
/// used again, fed by [`python_hex_words`] (see [`fill_lattice_y`]).
pub(crate) fn fill_lattice_x(n_rings: usize, univ: &[i32], out: &mut [i32]) {
    let nr = n_rings as i32;
    let n_side = (2 * nr - 1) as usize;
    let mut input_index = 0usize;
    let mut i_a: i32 = -(nr - 1);
    let mut i_y: i32 = nr - 1;

    // Upper region (first n_rings-1 rows).
    for k in 0..(nr - 1) {
        for _ in 0..(k + nr) {
            let indx = n_side * (i_y + nr - 1) as usize + (i_a + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_a += 1;
        }
        i_a = -(nr - 1);
        i_y -= 1;
    }

    // Lower region (centerline downward).
    for k in 0..nr {
        i_a = -(nr - 1) + k;
        for _ in 0..(2 * nr - k - 1) {
            let indx = n_side * (i_y + nr - 1) as usize + (i_a + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_a += 1;
        }
        i_y -= 1;
    }
}
