//! Locate the first and last fuelled node along each grid line.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `geometry_ends3d.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

use crate::handle3dcoords::handle3dcoords;
use crate::matlab::{Array2, Array3};
use crate::types::{Geometry, Params};

/// Scan one grid line for the first and last node carrying material.
///
/// Returns `(low, high)` as **0-based** node indices. Defaults are the full
/// span `(0, extent - 1)`, matching the reference's pre-fill.
fn scan_line(extent: usize, occupied: impl Fn(usize) -> bool) -> (usize, usize) {
    let mut low_index = 0;
    let mut high_index = extent - 1;
    let mut found_low = false;

    for i in 0..extent {
        if !found_low && occupied(i) {
            found_low = true;
            low_index = i;
        } else if found_low && !occupied(i) {
            // `i` is at least 1 here: `found_low` can only be true after an
            // earlier iteration set it.
            high_index = i - 1;
            break;
        }
    }

    (low_index, high_index)
}

/// `geometry = geometry_ends3d(params, geometry, whichsigma)`.
///
/// For every grid line in each of the three directions, records the index of
/// the first node with material present and the index of the last one. The
/// nodal solvers use these to apply the outer boundary condition at the real
/// edge of the reactor rather than at the edge of the bounding box.
///
/// Six fields are written onto `geometry`: `xlows`/`xhis` indexed `(iy, iz)`,
/// `ylows`/`yhis` indexed `(ix, iz)`, and `zlows`/`zhis` indexed `(ix, iy)`.
/// All stored values are **0-based node indices**.
///
/// # Arguments
///
/// - `params` — supplies the extents.
/// - `geometry` — modified in place, gaining the six bound arrays.
/// - `whichsigma` — material index per node, `0` meaning no material.
///
/// # Defaults when a line is empty or full
///
/// The reference pre-fills `lows` with the first index and `his` with the last,
/// then overwrites. A grid line with **no** material therefore reports the full
/// span rather than an empty range — the caller cannot distinguish "entirely
/// fuelled" from "entirely empty" from these arrays alone.
///
/// # Reference limitation — a single contiguous run per line
///
/// The scan stops at the first empty node after material is found (`break`). A
/// grid line with material, then a gap, then material again — an internal void
/// or a re-entrant boundary — has everything past the gap silently excluded,
/// with no warning. The benchmark geometries in this snapshot are convex, so
/// the case does not arise there, but the limitation is real and is translated
/// as written rather than generalised.
pub fn geometry_ends3d(params: &Params, geometry: &mut Geometry, whichsigma: &Array3<usize>) {
    let (maxix, maxiy, maxiz) = handle3dcoords(params);

    // --- x extents, per (iy, iz) line ------------------------------------
    let mut xlows = Array2::<usize>::zeros(maxiy, maxiz);
    let mut xhis = Array2::<usize>::zeros(maxiy, maxiz);
    for iy in 0..maxiy {
        for iz in 0..maxiz {
            let (low, high) = scan_line(maxix, |ix| whichsigma.get(ix, iy, iz) != 0);
            xlows.set(iy, iz, low);
            xhis.set(iy, iz, high);
        }
    }

    // --- y extents, per (ix, iz) line ------------------------------------
    let mut ylows = Array2::<usize>::zeros(maxix, maxiz);
    let mut yhis = Array2::<usize>::zeros(maxix, maxiz);
    for ix in 0..maxix {
        for iz in 0..maxiz {
            let (low, high) = scan_line(maxiy, |iy| whichsigma.get(ix, iy, iz) != 0);
            ylows.set(ix, iz, low);
            yhis.set(ix, iz, high);
        }
    }

    // --- z extents, per (ix, iy) line ------------------------------------
    let mut zlows = Array2::<usize>::zeros(maxix, maxiy);
    let mut zhis = Array2::<usize>::zeros(maxix, maxiy);
    for ix in 0..maxix {
        for iy in 0..maxiy {
            let (low, high) = scan_line(maxiz, |iz| whichsigma.get(ix, iy, iz) != 0);
            zlows.set(ix, iy, low);
            zhis.set(ix, iy, high);
        }
    }

    geometry.xlows = Some(xlows);
    geometry.xhis = Some(xhis);
    geometry.ylows = Some(ylows);
    geometry.yhis = Some(yhis);
    geometry.zlows = Some(zlows);
    geometry.zhis = Some(zhis);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_params(maxix: usize) -> Params {
        Params {
            maxix: Some(maxix),
            maxiy: Some(1),
            maxiz: Some(1),
            ..Default::default()
        }
    }

    #[test]
    fn bounds_track_a_contiguous_run() {
        let params = line_params(4);
        // Material occupies ix = 1 and 2 (0-based).
        let mut whichsigma = Array3::<usize>::zeros(4, 1, 1);
        whichsigma.set(1, 0, 0, 1);
        whichsigma.set(2, 0, 0, 1);

        let mut geometry = Geometry::default();
        geometry_ends3d(&params, &mut geometry, &whichsigma);

        assert_eq!(geometry.xlows.as_ref().unwrap().get(0, 0), 1);
        assert_eq!(geometry.xhis.as_ref().unwrap().get(0, 0), 2);
    }

    /// A fully fuelled line keeps the pre-filled full span, because the scan
    /// never meets the empty node that would set `xhis`.
    #[test]
    fn a_full_line_reports_the_whole_span() {
        let params = line_params(3);
        let mut whichsigma = Array3::<usize>::zeros(3, 1, 1);
        for ix in 0..3 {
            whichsigma.set(ix, 0, 0, 1);
        }
        let mut geometry = Geometry::default();
        geometry_ends3d(&params, &mut geometry, &whichsigma);
        assert_eq!(geometry.xlows.as_ref().unwrap().get(0, 0), 0);
        assert_eq!(geometry.xhis.as_ref().unwrap().get(0, 0), 2);
    }

    /// Pins the documented limitation: material after an internal gap is
    /// excluded.
    #[test]
    fn material_after_a_gap_is_excluded() {
        let params = line_params(5);
        // Material at ix = 0, gap at 1, material again at 2..=4.
        let mut whichsigma = Array3::<usize>::zeros(5, 1, 1);
        whichsigma.set(0, 0, 0, 1);
        whichsigma.set(2, 0, 0, 1);
        whichsigma.set(3, 0, 0, 1);
        whichsigma.set(4, 0, 0, 1);

        let mut geometry = Geometry::default();
        geometry_ends3d(&params, &mut geometry, &whichsigma);

        assert_eq!(geometry.xlows.as_ref().unwrap().get(0, 0), 0);
        assert_eq!(geometry.xhis.as_ref().unwrap().get(0, 0), 0);
    }

    /// An empty line is indistinguishable from a full one.
    #[test]
    fn an_empty_line_is_reported_as_the_full_span() {
        let params = line_params(3);
        let whichsigma = Array3::<usize>::zeros(3, 1, 1);
        let mut geometry = Geometry::default();
        geometry_ends3d(&params, &mut geometry, &whichsigma);
        assert_eq!(geometry.xlows.as_ref().unwrap().get(0, 0), 0);
        assert_eq!(geometry.xhis.as_ref().unwrap().get(0, 0), 2);
    }
}
