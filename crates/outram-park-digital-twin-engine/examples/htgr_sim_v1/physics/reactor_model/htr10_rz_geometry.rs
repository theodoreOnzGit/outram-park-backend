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
//! script above completed the rest.
//!
//! **Correction (maintainer, 2026-08-17), resolving a discrepancy this doc
//! comment used to flag:** the script's single volume 4 over z = [105, 130]
//! was missing a split at `z = 114.7` -- volume 82 sits above it
//! (105-114.7), volume 30 below (114.7-130). This is the reason the derived
//! doc's axial list already had `114.7` where the unmodified script's
//! `z_ticks` did not: the derived doc was right and the script had the gap.
//! `htr10_rz_zones()` below carries the correction; `generate_htr10_geometry.py`
//! itself (the GitHub attachment, not part of this repo's tree) still does
//! not, so a diff against it will show this one place, deliberately. The
//! material assigned to both 82 and 30 (`Graphite`) is *assumed*, inherited
//! from the pre-split volume 4 -- not independently confirmed, since only
//! the z-split itself was given.
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
//! ## Consumers
//!
//! [`pebble_bed_helium_volume`] has a real consumer:
//! [`super::one_node::PebbleBedPorousMediaNode`] carries it as a field, set
//! once at construction. Everything else here -- the zone list
//! itself, [`top_cavity_helium_volume`] and the four
//! `dummy_pebble_helium_volume_*` functions -- has no consumer yet; it lands
//! ahead of a schematic cross-section or mesh-generation use, not after one.
//! The `dead_code` lint stays silenced module-wide rather than per-item,
//! since most items here are still equally unread for that reason. Wiring
//! the rest in is tracked as follow-up work (`op-853i`), not deferred
//! silently.

#![allow(dead_code)] // data module ahead of its first consumer -- see "No consumer yet" above

use uom::si::f64::{Length, Ratio, Volume};
use uom::si::length::centimeter;
use uom::si::volume::cubic_centimeter;

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

    /// Solid-of-revolution volume of this zone's `(r, z)` cross-section,
    /// swept 360 degrees about the z-axis (`r = 0`) -- the physically
    /// correct volume for this axisymmetric benchmark, and valid for both
    /// rectangle and polygon zones alike.
    ///
    /// Computed directly by Pappus's centroid theorem in its line-integral
    /// form, `V = pi/3 * sum((z_{i+1} - z_i)(r_i^2 + r_i r_{i+1} +
    /// r_{i+1}^2))` around the closed vertex loop -- each term is exactly
    /// the signed volume of the frustum (or cylinder, if `r_i == r_{i+1}`)
    /// swept by one polygon edge, so the sum nets out to the enclosed solid
    /// exactly as the shoelace formula nets out to enclosed area. Taking the
    /// absolute value at the end makes it independent of vertex winding
    /// direction. For the four-vertex rectangle winding order this module's
    /// `rectangle()` helper uses, this reduces algebraically to the plain
    /// annulus formula `pi (r_max^2 - r_min^2) * height` -- checked directly
    /// in `tests::volume_of_revolution_matches_the_annulus_formula_for_a_rectangle`.
    pub fn volume_of_revolution(&self) -> Volume {
        let n = self.vertices_cm.len();
        let mut sum_cm3 = 0.0;
        for i in 0..n {
            let (r_i, z_i) = self.vertices_cm[i];
            let (r_next, z_next) = self.vertices_cm[(i + 1) % n];
            sum_cm3 += (z_next - z_i) * (r_i * r_i + r_i * r_next + r_next * r_next);
        }
        let volume_cm3 = (std::f64::consts::PI / 3.0) * sum_cm3.abs();
        Volume::new::<cubic_centimeter>(volume_cm3)
    }
}

/// Total geometric volume of every zone whose benchmark `volume` number
/// matches `target`, summed via [`Htr10RzZone::volume_of_revolution`].
/// Several benchmark zones are drawn as more than one rectangle across
/// adjacent axial bands (see [`Htr10RzZone`]'s doc comment) -- summing
/// recovers the one physical region's true volume.
fn zone_geometric_volume(target: u32) -> Volume {
    htr10_rz_zones()
        .into_iter()
        .filter(|zone| zone.volume == target)
        .map(|zone| zone.volume_of_revolution())
        .fold(Volume::new::<cubic_centimeter>(0.0), |acc, v| acc + v)
}

/// Bed void fraction (porosity) this module borrows rather than re-derives:
/// [`super::one_node::bed_porosity`], 0.39, the complement of the published
/// 0.61 pebble filling fraction. The R-Z zone map has no packing information
/// of its own -- only geometric extent -- so the porosity has to come from
/// somewhere that does. Do not add a second copy of this constant here.
fn bed_void_fraction() -> Ratio {
    super::one_node::bed_porosity()
}

/// Helium gas volume in the **top cavity** (benchmark zone 5) -- the open
/// void space between the upper reflector and the settled pebble bed, `r =
/// [0, 90]` cm, `z = [130, 228.758]` cm (98.758 cm tall). A plain cylinder,
/// since `r` starts at the axis. This is the zone's whole geometric volume:
/// the cavity has nothing packed in it to subtract a porosity for.
///
/// **NOT VALIDATED** -- see the module doc comment.
pub fn top_cavity_helium_volume() -> Volume {
    zone_geometric_volume(5)
}

/// Helium gas volume **within the pebble bed** (benchmark zone 99, the
/// mixed fuel/dummy-pebble region at the benchmark's critical loading,
/// `r = [0, 90]` cm, `z = [228.758, 351.818]` cm, 123.06 cm tall) -- the void
/// space between packed pebbles, not the bed's total geometric volume.
///
/// This is a **different quantity** from [`super::one_node::bed_void_volume`],
/// which uses the *published operational mean* bed height (197 cm, averaged
/// over the fuel cycle as pebbles are added and discharged) rather than this
/// benchmark's *critical-loading* height (123.06 cm) -- the two numbers are
/// not expected to agree and neither supersedes the other; they describe
/// different states of the same core.
///
/// **NOT VALIDATED** -- see the module doc comment.
pub fn pebble_bed_helium_volume() -> Volume {
    zone_geometric_volume(99) * bed_void_fraction()
}

/// Helium gas volume within the dummy-pebble packing of the **conus**
/// (benchmark zone 91, the diagonal transition region immediately below the
/// pebble bed). Assumes dummy pebbles pack at the same void fraction as the
/// fuelled bed -- same pebble size and shape, same random packing -- which
/// is physically reasonable but not independently confirmed.
///
/// **NOT VALIDATED** -- see the module doc comment.
pub fn dummy_pebble_helium_volume_conus() -> Volume {
    zone_geometric_volume(91) * bed_void_fraction()
}

/// Helium gas volume within the dummy-pebble packing of the **upper
/// discharge-tube region** (benchmark zone 6, `r = [0, 25]` cm, `z =
/// [388.764, 495.0]` cm, directly below the conus). Same porosity assumption
/// as [`dummy_pebble_helium_volume_conus`].
///
/// **NOT VALIDATED** -- see the module doc comment.
pub fn dummy_pebble_helium_volume_upper_discharge_tube() -> Volume {
    zone_geometric_volume(6) * bed_void_fraction()
}

/// Helium gas volume within the dummy-pebble packing of the **middle
/// discharge-tube region** (benchmark zone 7, `r = [0, 25]` cm -- drawn as
/// two adjacent rectangles at `z = [495, 510]` and `z = [510, 540]` cm in the
/// source script, merged here since they are the same physical zone). Same
/// porosity assumption as [`dummy_pebble_helium_volume_conus`].
///
/// **NOT VALIDATED** -- see the module doc comment.
pub fn dummy_pebble_helium_volume_middle_discharge_tube() -> Volume {
    zone_geometric_volume(7) * bed_void_fraction()
}

/// Helium gas volume within the dummy-pebble packing of the **lower
/// discharge-tube region** (benchmark zone 81, `r = [0, 25]` cm, `z = [540,
/// 610.0]` cm, the bottommost row). Same porosity assumption as
/// [`dummy_pebble_helium_volume_conus`].
///
/// **NOT VALIDATED** -- see the module doc comment.
pub fn dummy_pebble_helium_volume_lower_discharge_tube() -> Volume {
    zone_geometric_volume(81) * bed_void_fraction()
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
    // CORRECTION (maintainer, 2026-08-17): the script's single volume 4 over
    // z = [105, 130] was missing a split at z = 114.7 -- volume 82 sits
    // above it (105-114.7), volume 30 below (114.7-130). This is the fix for
    // the z_ticks discrepancy the module doc comment used to flag against
    // crates/kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md's
    // axial list, which already had 114.7. Material assumed Graphite for
    // both, matching volume 4's original assignment -- not independently
    // confirmed by the maintainer, since only the z-split was given.
    zones.push(rectangle(82, 0.0, 90.0, 105.0, 114.7, Graphite));
    zones.push(rectangle(30, 0.0, 90.0, 114.7, 130.0, Graphite));
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
    /// written in `generate_htr10_geometry.py`, **except** for the single
    /// `114.7` axial value introduced by the maintainer's 2026-08-17
    /// volume-82/30 correction (see the module doc comment) -- the
    /// unmodified script does not have it. This catches a mistyped boundary
    /// value in the port; it does **not** validate the transcription against
    /// the source figure itself, which is a human's job (see the module doc
    /// comment's NOT-VALIDATED status).
    ///
    /// Result (2026-08-17): both sets match, `114.7` included as expected.
    #[test]
    fn rectangle_boundaries_match_the_source_scripts_axis_ticks() {
        let expected_r_ticks = [
            0.0, 25.0, 41.75, 70.75, 90.0, 95.6, 108.6, 140.6, 148.6, 167.793, 190.0,
        ];
        // z_ticks per generate_htr10_geometry.py, plus 114.7 -- see the
        // "except" note in this test's doc comment above.
        let expected_z_ticks = [
            0.0, 40.0, 95.0, 105.0, 114.7, 130.0, 228.758, 351.818, 388.764, 402.0, 430.0, 450.0,
            465.0, 495.0, 510.0, 540.0, 610.0,
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

    /// Methodology: build a synthetic rectangle zone with `rectangle()` and
    /// compare [`Htr10RzZone::volume_of_revolution`] against the plain
    /// annulus formula `pi (r_max^2 - r_min^2) * height`, computed
    /// independently in this test. This is the algebraic identity the
    /// method's doc comment claims for the rectangle winding order
    /// `rectangle()` uses; if the two formulas ever diverge, the
    /// line-integral implementation has a sign or ordering bug.
    ///
    /// Result (2026-08-17): agree to within 1e-6 cm^3 on a 40 x 26.6 cm
    /// synthetic annulus (r = 10-50, z = 3-30 cm).
    #[test]
    fn volume_of_revolution_matches_the_annulus_formula_for_a_rectangle() {
        let (r_min, r_max, z_top, z_bottom) = (10.0_f64, 50.0_f64, 3.0_f64, 30.0_f64);
        let zone = rectangle(0, r_min, r_max, z_top, z_bottom, ZoneMaterial::Unknown);
        let expected_cm3 =
            std::f64::consts::PI * (r_max * r_max - r_min * r_min) * (z_bottom - z_top);
        let got_cm3 = zone.volume_of_revolution().get::<cubic_centimeter>();
        assert!(
            (got_cm3 - expected_cm3).abs() < 1e-6,
            "got {got_cm3}, expected {expected_cm3}"
        );
    }

    /// Methodology: the top cavity (zone 5) is a plain cylinder, `r = [0,
    /// 90]` cm, `z = [130, 228.758]` cm -- compare
    /// [`top_cavity_helium_volume`] against `pi r^2 h` computed independently
    /// in this test.
    ///
    /// Result (2026-08-17): agrees to within 1e-6 cm^3.
    #[test]
    fn top_cavity_volume_matches_a_plain_cylinder_formula() {
        let radius_cm = 90.0_f64;
        let height_cm = 228.758 - 130.0;
        let expected_cm3 = std::f64::consts::PI * radius_cm * radius_cm * height_cm;
        let got_cm3 = top_cavity_helium_volume().get::<cubic_centimeter>();
        assert!(
            (got_cm3 - expected_cm3).abs() < 1e-6,
            "got {got_cm3}, expected {expected_cm3}"
        );
    }

    /// Methodology: [`pebble_bed_helium_volume`] must equal zone 99's raw
    /// geometric volume times [`super::super::one_node::bed_porosity`] --
    /// checked by recomputing both sides independently in this test, so a
    /// future edit that changes one but not the other is caught.
    ///
    /// Result (2026-08-17): agrees exactly (same porosity value on both
    /// sides, by construction).
    #[test]
    fn pebble_bed_helium_volume_is_the_bed_geometric_volume_times_porosity() {
        let radius_cm = 90.0_f64;
        let height_cm = 351.818 - 228.758;
        let bed_geometric_volume_cm3 = std::f64::consts::PI * radius_cm * radius_cm * height_cm;
        let porosity = super::super::one_node::bed_porosity().get::<uom::si::ratio::ratio>();
        let expected_cm3 = bed_geometric_volume_cm3 * porosity;
        let got_cm3 = pebble_bed_helium_volume().get::<cubic_centimeter>();
        assert!(
            (got_cm3 - expected_cm3).abs() < 1e-6,
            "got {got_cm3}, expected {expected_cm3}"
        );
    }

    /// Methodology: benchmark zone 7 is drawn as two adjacent rectangles
    /// (`z = [495, 510]` and `z = [510, 540]`, same `r = [0, 25]`) --
    /// [`dummy_pebble_helium_volume_middle_discharge_tube`] must equal the
    /// sum of the two sub-band cylinder volumes computed independently here,
    /// confirming the merge-by-`volume`-number logic in
    /// [`zone_geometric_volume`] adds rather than overwrites.
    ///
    /// Result (2026-08-17): agrees to within 1e-6 cm^3.
    #[test]
    fn middle_discharge_tube_volume_is_the_sum_of_its_two_drawn_sub_bands() {
        let radius_cm = 25.0_f64;
        let band_1_cm3 = std::f64::consts::PI * radius_cm * radius_cm * (510.0 - 495.0);
        let band_2_cm3 = std::f64::consts::PI * radius_cm * radius_cm * (540.0 - 510.0);
        let porosity = super::super::one_node::bed_porosity().get::<uom::si::ratio::ratio>();
        let expected_cm3 = (band_1_cm3 + band_2_cm3) * porosity;
        let got_cm3 = dummy_pebble_helium_volume_middle_discharge_tube().get::<cubic_centimeter>();
        assert!(
            (got_cm3 - expected_cm3).abs() < 1e-6,
            "got {got_cm3}, expected {expected_cm3}"
        );
    }

    /// Methodology: every helium-volume function defined here must return a
    /// strictly positive, finite volume -- a zero or negative result would
    /// mean a wrong zone number (no zones matched) or a sign error, and a
    /// non-finite result would mean a NaN slipped through the porosity or
    /// geometry arithmetic.
    ///
    /// Result (2026-08-17): all six pass.
    #[test]
    fn every_helium_volume_function_is_positive_and_finite() {
        let volumes_cm3 = [
            top_cavity_helium_volume().get::<cubic_centimeter>(),
            pebble_bed_helium_volume().get::<cubic_centimeter>(),
            dummy_pebble_helium_volume_conus().get::<cubic_centimeter>(),
            dummy_pebble_helium_volume_upper_discharge_tube().get::<cubic_centimeter>(),
            dummy_pebble_helium_volume_middle_discharge_tube().get::<cubic_centimeter>(),
            dummy_pebble_helium_volume_lower_discharge_tube().get::<cubic_centimeter>(),
        ];
        for v in volumes_cm3 {
            assert!(v.is_finite(), "non-finite helium volume: {v}");
            assert!(v > 0.0, "non-positive helium volume: {v}");
        }
    }
}
