//! Axisymmetric (r, z) subvolume/zone map of the **simplified HTR-10
//! benchmark model** -- reflector, coolant channels, boronated shielding and
//! the pebble-bed core cavity, as an explicit list of dimensioned regions.
//!
//! This is geometry data, not a fidelity tier: it does not appear in
//! [`super::ReactorModelKind`] and nothing in this simulator's thermal solve
//! reads it yet. It exists so a future consumer (a schematic cross-section,
//! a mesh generator, a reflector/shield thermal model) has one place to pull
//! the benchmark's reflector geometry from, instead of re-deriving it. The
//! *bed's own* lumped dimensions (`core_diameter`, `average_core_height`) stay
//! in [`super::one_node`] -- that module explicitly does not model the
//! reflector, core barrel or vessel at all, which is exactly the geometry
//! this module fills in.
//!
//! ## Provenance -- read this before trusting the numbers
//!
//! Transcribed directly from `generate_htr10_geometry.py`, a script the
//! maintainer had generated (GitHub Copilot) from their own manual
//! transcription of **Fig. 2** of Terry, W. K. et al. (2005), *Evaluation of
//! the HTR-10 Reactor as a Benchmark for Physics Code QA*, INL/CON-05-00852
//! (preprint) -- attached to
//! [GitHub issue #23](https://github.com/theodoreOnzGit/outram-park-backend/issues/23).
//! The same figure, same hand-reading, is independently recorded with its own
//! corroboration against the paper's Table 2 in
//! `crates/kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md` --
//! that earlier note only reached two of the sixteen axial rows before the
//! script above completed the rest, so **the two documents currently
//! disagree in one place**: the earlier note's axial boundary list includes
//! `114.7` where this script's `z_ticks` does not. That is not resolved here;
//! it is a hand-reading question for the maintainer/Yan Ren to settle against
//! the figure, not something this module should silently pick a side on.
//!
//! Citation caution carries over unchanged: the preprint "should not be cited
//! or reproduced without permission of the author" -- for publication, cite
//! the IAEA TECDOCs or the IRPhEP handbook (NEA/NSC/DOC(2006)1), not the
//! preprint. See the derived-data doc above for the open-provenance path.
//!
//! ## Status: NOT VALIDATED
//!
//! Per the workspace V&V rule (`RESPONSIBLE_USE.md`, `VERIFICATION_AND_VALIDATION.md`):
//! this is a **hand-transcribed reconstruction of a raster figure**, not a
//! digitiser run (no calibration record, no stated uncertainty) and not yet
//! checked against any transport or mesh calculation. Treat every number here
//! as illustrative until a human reviews it against the source figure a
//! second time. Do not describe anything built on top of this module as
//! validated.
//!
//! ## Units and orientation
//!
//! All dimensions are **centimetres, exactly as read from the figure** -- no
//! conversion has been applied, so [`Htr10RzZone::vertices_cm`] stays
//! byte-comparable against the script and the figure for anyone re-checking.
//! [`Htr10RzZone::vertices_m`] converts to [`Length`] (`uom`) at the API
//! boundary for callers that want SI. **Z increases downward**: `z = 0` is
//! the top of the model (the upper reflector face) and `z = 610` cm is the
//! bottom (the base of the discharge-tube reflector). This matches the
//! figure's own orientation, confirmed by the maintainer against Table 2 of
//! the same paper (see the derived-data doc's "Orientation" section).
//!
//! The model is **radially symmetric** -- an r-z partition, not a full 3-D
//! one. Borings (control rods, coolant channels) are homogenised into the
//! ring they sit in; they are not resolved as discrete holes.
//!
//! ## No consumer yet
//!
//! Nothing in `htgr_sim_v1` reads this module yet -- it lands ahead of a
//! schematic cross-section or mesh-generation consumer, not after one. The
//! `dead_code` lint is silenced module-wide for exactly that reason rather
//! than per-item, since every item here is equally unread for the same
//! reason. Wiring a consumer in is tracked as follow-up work, not deferred
//! silently.

#![allow(dead_code)] // data module ahead of its first consumer -- see "No consumer yet" above

use uom::si::f64::Length;
use uom::si::length::centimeter;

/// Material/zone classification for an [`Htr10RzZone`], matching the
/// legend of the source reconstruction (`generate_htr10_geometry.py`'s
/// `STYLES` map). Homogenised region identity, not a cross-section library
/// key -- a caller mapping this to nuclear data still has to choose the
/// actual composition each variant stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneMaterial {
    /// Boronated carbon brick shielding (outermost shell and top/bottom caps).
    Boronated,
    /// Plain carbon brick (bottommost row, outside the discharge tube).
    Carbon,
    /// Graphite reflector.
    Graphite,
    /// Bottom reflector block (below the core cavity, around the conus).
    Bottom,
    /// Volume 31 -- control-rod-channel reflector column.
    Control,
    /// Cold coolant flow channel (volumes 58-63 and their row-siblings).
    ColdChannel,
    /// Volume 83 -- hot-coolant-side reflector at the core/conus transition.
    Hot,
    /// Volume 3 -- cold-coolant collection chamber above the core cavity.
    ColdChamber,
    /// Volume 5 -- the open core cavity above the settled pebble bed.
    Cavity,
    /// Volume 99 -- mixed fuel/dummy pebble region (the settled bed at the
    /// benchmark's critical loading).
    Mixed,
    /// Dummy-pebble-only region (conus and discharge tube).
    Dummy,
    /// Material not yet assigned by the hand-reading (Z = 510-540 cm row);
    /// do not treat as a real material, only as "known unknown".
    Unknown,
}

/// One subvolume of the HTR-10 R-Z benchmark zone map.
///
/// `volume` is the benchmark's own zone number (Terry 2005 Fig. 2 numbering,
/// as carried through the reconstruction script) and is **not unique across
/// [`htr10_rz_zones`]** -- the same physical zone can appear as more than one
/// entry when the source script drew it across more than one axial sub-band
/// (e.g. volume `12` appears three times, at consecutive z-spans that are
/// contiguous in the real geometry). Merge on `volume` first if a caller
/// needs one region per physical zone rather than one entry per drawn band.
#[derive(Clone, Debug)]
pub struct Htr10RzZone {
    /// Benchmark zone number, Terry 2005 Fig. 2 numbering.
    pub volume: u32,
    /// Homogenised material/zone classification.
    pub material: ZoneMaterial,
    /// Ordered polygon vertices `(r_cm, z_cm)`, implicitly closed (the last
    /// vertex connects back to the first). Rectangles are stored as their
    /// four corners, `(r_min, z_top) -> (r_max, z_top) -> (r_max, z_bottom)
    /// -> (r_min, z_bottom)`. Centimetres, exactly as read -- see the module
    /// doc comment.
    pub vertices_cm: Vec<(f64, f64)>,
}

impl Htr10RzZone {
    /// [`Self::vertices_cm`] converted to [`Length`] pairs, for callers that
    /// want SI rather than the raw centimetre figures this data was recorded
    /// in.
    pub fn vertices_m(&self) -> Vec<(Length, Length)> {
        self.vertices_cm
            .iter()
            .map(|&(r_cm, z_cm)| {
                (
                    Length::new::<centimeter>(r_cm),
                    Length::new::<centimeter>(z_cm),
                )
            })
            .collect()
    }
}

fn rectangle(
    volume: u32,
    r_min: f64,
    r_max: f64,
    z_top: f64,
    z_bottom: f64,
    material: ZoneMaterial,
) -> Htr10RzZone {
    Htr10RzZone {
        volume,
        material,
        vertices_cm: vec![
            (r_min, z_top),
            (r_max, z_top),
            (r_max, z_bottom),
            (r_min, z_bottom),
        ],
    }
}

fn polygon(volume: u32, vertices: &[(f64, f64)], material: ZoneMaterial) -> Htr10RzZone {
    Htr10RzZone {
        volume,
        material,
        vertices_cm: vertices.to_vec(),
    }
}

/// The full HTR-10 R-Z benchmark zone list, port-for-port from
/// `generate_htr10_geometry.py`'s `regions` construction (same blocks, same
/// order, same loops) so it can be diffed against the source script
/// line-by-line. See the module doc comment for provenance and the
/// NOT-VALIDATED caveat.
pub fn htr10_rz_zones() -> Vec<Htr10RzZone> {
    use ZoneMaterial::*;
    let mut zones = Vec::new();

    // Top layer, Z = 0 to 40 cm.
    for (volume, r_min, r_max) in [
        (1, 0.0, 90.0),
        (19, 90.0, 95.6),
        (27, 95.6, 108.6),
        (74, 108.6, 167.793),
        (75, 167.793, 190.0),
    ] {
        zones.push(rectangle(volume, r_min, r_max, 0.0, 40.0, Boronated));
    }

    // Central upper stack.
    zones.push(rectangle(2, 0.0, 90.0, 40.0, 95.0, Graphite));
    zones.push(rectangle(3, 0.0, 90.0, 95.0, 105.0, ColdChamber));
    zones.push(rectangle(4, 0.0, 90.0, 105.0, 130.0, Graphite));
    zones.push(rectangle(5, 0.0, 90.0, 130.0, 228.758, Cavity));
    zones.push(rectangle(99, 0.0, 90.0, 228.758, 351.818, Mixed));

    // R = 90.0 to 95.6 cm column.
    zones.push(rectangle(20, 90.0, 95.6, 40.0, 95.0, Graphite));
    zones.push(rectangle(21, 90.0, 95.6, 95.0, 105.0, Graphite));
    zones.push(rectangle(22, 90.0, 95.6, 105.0, 388.764, Graphite));

    // R = 95.6 to 108.6 cm column.
    zones.push(rectangle(28, 95.6, 108.6, 40.0, 95.0, Graphite));
    zones.push(rectangle(29, 95.6, 108.6, 95.0, 130.0, Graphite));
    zones.push(rectangle(31, 95.6, 108.6, 130.0, 388.764, Control));

    // Upper reflector and coolant-channel regions.
    zones.push(rectangle(66, 108.6, 140.6, 95.0, 105.0, Graphite));
    zones.push(rectangle(57, 140.6, 148.6, 95.0, 105.0, Graphite));
    zones.push(rectangle(49, 108.6, 140.6, 105.0, 388.764, Graphite));
    zones.push(rectangle(58, 140.6, 148.6, 105.0, 388.764, ColdChannel));

    // Volume 48: continuous L-shaped graphite-reflector region directly below 74.
    zones.push(polygon(
        48,
        &[
            (108.6, 40.0),
            (167.793, 40.0),
            (167.793, 388.764),
            (148.6, 388.764),
            (148.6, 95.0),
            (108.6, 95.0),
        ],
        Graphite,
    ));

    // Continuous outer boronated-carbon shell.
    zones.push(rectangle(76, 167.793, 190.0, 40.0, 465.0, Boronated));

    // Special diagonal transition region.
    zones.push(polygon(
        91,
        &[
            (0.0, 351.818),
            (90.0, 351.818),
            (25.0, 388.764),
            (0.0, 388.764),
        ],
        Dummy,
    ));
    zones.push(polygon(
        83,
        &[(90.0, 351.818), (90.0, 388.764), (25.0, 388.764)],
        Hot,
    ));

    // Lower geometry, Z = 388.764 to 430 cm.
    zones.push(rectangle(6, 0.0, 25.0, 388.764, 495.0, Dummy));
    zones.push(rectangle(8, 25.0, 90.0, 388.764, 402.0, Bottom));
    zones.push(rectangle(9, 25.0, 90.0, 402.0, 430.0, Bottom));
    for (volume, r_min, r_max, material) in [
        (23, 90.0, 95.6, Graphite),
        (41, 95.6, 108.6, Graphite),
        (50, 108.6, 140.6, Graphite),
        (59, 140.6, 148.6, ColdChannel),
        (67, 148.6, 167.793, Graphite),
    ] {
        zones.push(rectangle(volume, r_min, r_max, 388.764, 430.0, material));
    }

    // Z = 430 to 450 cm.
    for (volume, r_min, r_max, material) in [
        (10, 25.0, 41.75, Bottom),
        (11, 41.75, 90.0, Bottom),
        (24, 90.0, 95.6, Graphite),
        (42, 95.6, 108.6, Graphite),
        (51, 108.6, 140.6, Graphite),
        (60, 140.6, 148.6, ColdChannel),
        (68, 148.6, 167.793, Graphite),
    ] {
        zones.push(rectangle(volume, r_min, r_max, 430.0, 450.0, material));
    }

    // Z = 450 to 465 cm.
    for (volume, r_min, r_max, material) in [
        (12, 25.0, 41.75, Bottom),
        (13, 41.75, 90.0, Bottom),
        (25, 90.0, 95.6, Graphite),
        (43, 95.6, 108.6, Graphite),
        (52, 108.6, 140.6, Graphite),
        (61, 140.6, 148.6, ColdChannel),
        (69, 148.6, 167.793, Graphite),
    ] {
        zones.push(rectangle(volume, r_min, r_max, 450.0, 465.0, material));
    }

    // Z = 465 to 495 cm.
    for (volume, r_min, r_max, material) in [
        (12, 25.0, 41.75, Bottom),
        (14, 41.75, 70.75, Bottom),
        (15, 70.75, 90.0, Bottom),
        (26, 90.0, 95.6, Graphite),
        (44, 95.6, 108.6, Graphite),
        (53, 108.6, 140.6, Graphite),
        (62, 140.6, 148.6, ColdChannel),
        (70, 148.6, 167.793, Graphite),
        (77, 167.793, 190.0, Boronated),
    ] {
        zones.push(rectangle(volume, r_min, r_max, 465.0, 495.0, material));
    }

    // Z = 495 to 510 cm.
    for (volume, r_min, r_max, material) in [
        (7, 0.0, 25.0, Dummy),
        (12, 25.0, 41.75, Bottom),
        (16, 41.75, 90.0, Bottom),
        (80, 90.0, 95.6, Graphite),
        (45, 95.6, 108.6, Graphite),
        (54, 108.6, 140.6, Graphite),
        (63, 140.6, 148.6, ColdChannel),
        (71, 148.6, 167.793, Graphite),
        (78, 167.793, 190.0, Boronated),
    ] {
        zones.push(rectangle(volume, r_min, r_max, 495.0, 510.0, material));
    }

    // Z = 510 to 540 cm. Detailed material assignments remain pending.
    zones.push(rectangle(7, 0.0, 25.0, 510.0, 540.0, Dummy));
    for (volume, r_min, r_max) in [
        (17, 25.0, 95.6),
        (46, 95.6, 108.6),
        (55, 108.6, 140.6),
        (64, 140.6, 148.6),
        (72, 148.6, 167.793),
    ] {
        zones.push(rectangle(volume, r_min, r_max, 510.0, 540.0, Unknown));
    }
    zones.push(rectangle(78, 167.793, 190.0, 510.0, 540.0, Boronated));

    // Bottommost row, Z = 540 to 610 cm.
    zones.push(rectangle(81, 0.0, 25.0, 540.0, 610.0, Dummy));
    for (volume, r_min, r_max) in [
        (18, 25.0, 95.6),
        (47, 95.6, 108.6),
        (56, 108.6, 140.6),
        (65, 140.6, 148.6),
        (73, 148.6, 167.793),
    ] {
        zones.push(rectangle(volume, r_min, r_max, 540.0, 610.0, Carbon));
    }
    zones.push(rectangle(79, 167.793, 190.0, 540.0, 610.0, Boronated));

    zones
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: every axis-aligned rectangle zone must have a positive
    /// footprint (`r_min < r_max`, `z_top < z_bottom`) -- a degenerate or
    /// inverted rectangle would be a transcription error, not a real
    /// benchmark feature. Checked over all rectangle-shaped zones (four
    /// vertices in the `rectangle()` winding order); the three polygon zones
    /// (48, 83, 91) are excluded since they are not rectangles.
    ///
    /// Result (2026-08-17): all rectangle zones pass.
    #[test]
    fn every_rectangle_zone_has_a_positive_footprint() {
        for zone in htr10_rz_zones() {
            if zone.vertices_cm.len() != 4 {
                continue; // polygon zone (48, 83, 91), not a rectangle
            }
            let (r_min, z_top) = zone.vertices_cm[0];
            let (r_max, _) = zone.vertices_cm[1];
            let (_, z_bottom) = zone.vertices_cm[2];
            assert!(
                r_min < r_max,
                "zone {}: r_min {r_min} >= r_max {r_max}",
                zone.volume
            );
            assert!(
                z_top < z_bottom,
                "zone {}: z_top {z_top} >= z_bottom {z_bottom}",
                zone.volume
            );
        }
    }

    /// Methodology: this transcription's own internal self-consistency check
    /// -- the set of distinct radial and axial boundaries implied by every
    /// rectangle zone's corners must equal `r_ticks`/`z_ticks` as literally
    /// written in `generate_htr10_geometry.py` (reproduced below). This
    /// catches a mistyped boundary value in the port; it does **not**
    /// validate the transcription against the source figure itself, which is
    /// a human's job (see the module doc comment's NOT-VALIDATED status).
    ///
    /// Result (2026-08-17): both sets match exactly.
    #[test]
    fn rectangle_boundaries_match_the_source_scripts_axis_ticks() {
        let expected_r_ticks = [
            0.0, 25.0, 41.75, 70.75, 90.0, 95.6, 108.6, 140.6, 148.6, 167.793, 190.0,
        ];
        let expected_z_ticks = [
            0.0, 40.0, 95.0, 105.0, 130.0, 228.758, 351.818, 388.764, 402.0, 430.0, 450.0, 465.0,
            495.0, 510.0, 540.0, 610.0,
        ];

        let mut r_values: Vec<f64> = Vec::new();
        let mut z_values: Vec<f64> = Vec::new();
        for zone in htr10_rz_zones() {
            if zone.vertices_cm.len() != 4 {
                continue; // polygon zone -- not part of the tensor-grid ticks
            }
            let (r_min, z_top) = zone.vertices_cm[0];
            let (r_max, _) = zone.vertices_cm[1];
            let (_, z_bottom) = zone.vertices_cm[2];
            r_values.push(r_min);
            r_values.push(r_max);
            z_values.push(z_top);
            z_values.push(z_bottom);
        }
        r_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        r_values.dedup();
        z_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        z_values.dedup();

        assert_eq!(r_values, expected_r_ticks, "radial boundary set diverged from the source script's r_ticks");
        assert_eq!(z_values, expected_z_ticks, "axial boundary set diverged from the source script's z_ticks");
    }

    /// Methodology: `vertices_m` must be `vertices_cm` converted 1:1, no more
    /// and no fewer points. Checked against zone 1's rectangle.
    ///
    /// Result (2026-08-17): passes.
    #[test]
    fn vertices_m_is_a_faithful_unit_conversion_of_vertices_cm() {
        let zone = htr10_rz_zones().into_iter().next().unwrap();
        let converted = zone.vertices_m();
        assert_eq!(converted.len(), zone.vertices_cm.len());
        for ((r_cm, z_cm), (r_m, z_m)) in zone.vertices_cm.iter().zip(converted.iter()) {
            assert!((r_m.get::<centimeter>() - r_cm).abs() < 1e-9);
            assert!((z_m.get::<centimeter>() - z_cm).abs() < 1e-9);
        }
    }
}
