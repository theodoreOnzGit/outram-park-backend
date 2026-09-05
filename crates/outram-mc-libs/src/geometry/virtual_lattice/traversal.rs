//! Ray traversal through a [`VirtualLattice`] — the 3-D DDA voxel walk.
//!
//! Ported from `CSGCell::distance_in_virtual_lattice` (`src/cell.cpp:665` in
//! the vendored `liangjg/openmc` `virtual_lattice` fork, commit `be04e28`).
//! See the module-level docs in [`super`] for provenance and the MIT notice.

use super::VirtualLattice;
use crate::geometry::position::{Direction, Position};
use crate::geometry::surface::SurfaceKind;

/// Floating-point precision floor for "already on the edge" tests. Mirrors
/// `FP_PRECISION` (`include/openmc/constants.h:53`), matching the value used
/// by [`crate::geometry::lattice`].
const FP_PRECISION: f64 = 1.0e-14;

impl VirtualLattice {
    /// Distance to the nearest registered surface along the ray `(r, u)`,
    /// searching only the voxels the ray actually crosses.
    ///
    /// This is the accelerated counterpart of
    /// [`crate::geometry::cell::Cell::distance_to_boundary`]: same contract,
    /// but O(spheres per voxel x voxels crossed) instead of O(all surfaces).
    ///
    /// # Arguments
    ///
    /// - `r` — ray origin, cm. Need not lie inside the grid; a ray starting
    ///   outside walks in along its first boundary index (see below).
    /// - `u` — ray direction. Normalised internally, mirroring upstream, which
    ///   carries the comment *"don't know if u has been normalized"*.
    /// - `on_surface` — global index of the surface the particle currently sits
    ///   on, or `usize::MAX` if none. That surface is queried with the
    ///   `coincident` flag so round-off cannot re-report a zero crossing.
    /// - `max_distance` — stop walking once the voxel exit distance exceeds
    ///   this. Upstream passes the sampled collision distance
    ///   (`p->collision_distance()`): there is no point tracking surfaces
    ///   beyond the next collision. Pass `f64::INFINITY` for no cutoff.
    ///
    /// # Returns
    ///
    /// `(distance, surface_idx)`, with `surface_idx == usize::MAX` and
    /// `distance == f64::INFINITY` when no registered surface is hit.
    ///
    /// # Early-exit condition
    ///
    /// The walk stops as soon as the best hit so far is closer than the exit
    /// point of the current voxel — at that point no later voxel can contain a
    /// nearer surface. This is what makes the traversal sublinear.
    ///
    /// # Caveat inherited from upstream
    ///
    /// `max_distance` is checked *after* stepping, against the distance to the
    /// voxel boundary just crossed, so the walk may enter one voxel beyond the
    /// cutoff before stopping. Faithful to `src/cell.cpp:665`; harmless
    /// (it can only find a *nearer* surface), but it is not a tight bound.
    pub fn distance(
        &self,
        r: Position,
        u: Direction,
        surfaces: &[SurfaceKind],
        on_surface: usize,
        max_distance: f64,
    ) -> (f64, usize) {
        let mut min_dist = f64::INFINITY;
        let mut i_surf = usize::MAX;

        // Upstream normalises defensively; do the same so a caller passing a
        // non-unit direction gets the same answer it would from OpenMC.
        let norm = (u.u * u.u + u.v * u.v + u.w * u.w).sqrt();
        if norm == 0.0 || !norm.is_finite() {
            return (min_dist, i_surf);
        }
        let dir = [u.u / norm, u.v / norm, u.w / norm];
        let origin = [r.x, r.y, r.z];

        // Starting voxel. The two corrections pull a ray that starts exactly on
        // the far/near face back onto the grid when it is heading inward.
        let mut ijk = self.indices_at(r);
        for d in 0..3 {
            if ijk[d] == self.shape[d] as i64 && dir[d] < 0.0 {
                ijk[d] = self.shape[d] as i64 - 1;
            }
            if ijk[d] == -1 && dir[d] > 0.0 {
                ijk[d] = 0;
            }
        }

        // Distance from the origin to the next voxel boundary on each axis, and
        // the constant increment between successive boundaries on that axis.
        // An axis with dir == 0 never crosses a boundary, so both stay
        // infinite. (Upstream leaves the increment uninitialised in that case;
        // initialising it here is safe because the axis can never be selected.)
        let mut dist_to_bound = [f64::INFINITY; 3];
        let mut dist_to_bound_step = [f64::INFINITY; 3];
        for d in 0..3 {
            if dir[d] > 0.0 {
                dist_to_bound[d] =
                    (((ijk[d] + 1) as f64) * self.pitch[d] + self.lower_left[d] - origin[d]).abs()
                        / dir[d].abs();
                dist_to_bound_step[d] = self.pitch[d] / dir[d].abs();
            } else if dir[d] < 0.0 {
                dist_to_bound[d] =
                    ((ijk[d] as f64) * self.pitch[d] + self.lower_left[d] - origin[d]).abs()
                        / dir[d].abs();
                dist_to_bound_step[d] = self.pitch[d] / dir[d].abs();
            }
        }

        loop {
            if !self.contains_indices(ijk) {
                break;
            }

            let voxel = [ijk[0] as usize, ijk[1] as usize, ijk[2] as usize];
            for &si in self.surfaces_in_voxel(voxel) {
                let coincident = si == on_surface;
                let d = surfaces[si].distance(r, u, coincident);
                if d < min_dist && (min_dist - d) >= FP_PRECISION * min_dist {
                    min_dist = d;
                    i_surf = si;
                }
            }

            // Axis whose boundary the ray reaches first.
            let mut axis = 0usize;
            if dist_to_bound[1] < dist_to_bound[0] {
                axis = 1;
            }
            if dist_to_bound[2] < dist_to_bound[axis] {
                axis = 2;
            }
            let exit_dist = dist_to_bound[axis];

            // Nothing further along the ray can beat a hit inside this voxel.
            if min_dist < exit_dist {
                break;
            }

            if dir[axis] > 0.0 {
                ijk[axis] += 1;
            } else {
                ijk[axis] -= 1;
            }
            dist_to_bound[axis] += dist_to_bound_step[axis];

            if exit_dist > max_distance {
                break;
            }
        }

        (min_dist, i_surf)
    }

    /// Brute-force reference: nearest surface over *every* index in
    /// `surface_indices`, ignoring the grid entirely.
    ///
    /// Not used in transport — it exists so tests can assert that
    /// [`VirtualLattice::distance`] returns the same answer as the
    /// unaccelerated scan, which is the correctness property that matters.
    /// Same `(distance, surface_idx)` contract.
    pub fn distance_brute_force(
        r: Position,
        u: Direction,
        surfaces: &[SurfaceKind],
        surface_indices: &[usize],
        on_surface: usize,
    ) -> (f64, usize) {
        let mut min_dist = f64::INFINITY;
        let mut i_surf = usize::MAX;
        for &si in surface_indices {
            let coincident = si == on_surface;
            let d = surfaces[si].distance(r, u, coincident);
            if d < min_dist && (min_dist - d) >= FP_PRECISION * min_dist {
                min_dist = d;
                i_surf = si;
            }
        }
        (min_dist, i_surf)
    }
}
