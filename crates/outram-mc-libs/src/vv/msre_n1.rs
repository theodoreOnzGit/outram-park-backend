//! **MSRE first criticality, U-235 (MSRE N-1, gh:#1128): the full-core
//! model.**
//!
//! > Research, education and V&V only. AI draft (Claude Opus 5.5 in Claude
//! > Code), unreviewed.
//!
//! The model, its sources and its simplifications were posted on gh:#1128
//! before any run. Page numbers below are report pages of the scans in the
//! MSRE repository's `literature/`.
//!
//! # Reference: ORNL-4233 (Prince et al. 1968), §2, Table 1, p. 11
//!
//! The state is 1181 °F, pump off, with rod poisoning of 0.08 % δk/k. The
//! critical ²³⁵U concentration was **32.8 ± 0.3 g/L** (book mass fraction
//! corrected for flush-salt dilution) or 32.4 ± 0.3 g/L (analytical). This
//! model is built at 32.78 g/L, and its rods-out k is compared against
//! `1/(1 − 0.0008)`: [`REFERENCE_K`].
//!
//! # Geometry (ORNL-TM-0728)
//!
//! - **Core**, Table 5.4: a lattice of 2 in graphite stringers, clipped to a
//!   cylinder 55¼ in in diameter and 64 in high, with a stringer on the axis
//!   (Fig. 5.7).
//!   - Each stringer face carries half of a **stadium** fuel passage,
//!     1.2 × 0.4 in with 0.2 in rounded ends (Fig. 5.7). That gives a fuel
//!     fraction of 0.2228, against Table 5.4's 0.224.
//!   - [`STRINGER_FUEL_FRACTION`] is the analytic value;
//!     `tests::the_stringer_holds_the_tabulated_fuel_fraction` measures it
//!     on the assembled geometry.
//! - **Centre**, Fig. 5.7 and §5.3.5.2: three control-rod thimbles at
//!   (±2, ±2) in.
//!   - Each thimble is a 2 in OD × 0.065 in INOR-8 tube, empty with the rods
//!     withdrawn.
//!   - Each sits in a graphite opening 2.5 in across. **That diameter was
//!     read from Fig. 5.7 by scale (±0.1 in); the text does not give it.**
//!   - The fourth position holds the sample baskets (§5.3.6), homogenised
//!     over the same opening ([`basket_mix`]).
//! - **Radially, outward** (Table 5.2): the salt gap to the core can (55½ in
//!   ID, ¼ in INOR-8), the downcomer (56 to 58 in), and the vessel (58 in ID,
//!   59⅛ in OD).
//! - **Axially:** the heads are not dimensioned in the report. Each is a salt
//!   plenum of the vessel's inner radius holding Table 5.4's volume (upper
//!   10.5 ft³, lower 10.0 ft³), closed by a 1 in INOR-8 plate (Table 5.2's
//!   head thickness).
//!   - Vacuum lies outside the vessel; the thermal shield is not modelled by
//!     default ([`Shield::Tm0728Cylinder`] adds its cylinder, an ablation).
//!   - Also not modelled: the ~5 ft³ of salt in the graphite's end regions
//!     (Table 5.4's 25 ft³ "between the extreme top and bottom of graphite"
//!     against 20 ft³ in the lattice), and the distributor and volute.
//! - **Cold dimensions throughout.**
//!
//! # Materials ([`materials`])
//!
//! Everything is at [`TEMPERATURE_K`] (1181 °F).
//!
//! - **Fuel salt**, built from ORNL-4233's inventory (p. 3–4, 9–10):
//!   - the carrier 65LiF-30BeF₂-5ZrF₄;
//!   - a 73LiF-27UF₄ eutectic with 150 kg of depleted U;
//!   - 93 % enriched U, 69.6 kg of ²³⁵U at criticality;
//!   - 20 kg of 66LiF-34BeF₂ flush salt;
//!   - density 145.3 lb/ft³.
//!
//!   This gives 1.4083 wt % ²³⁵U (ORNL: 1.408) and 32.78 g/L, at
//!   65.24-29.14-4.83-0.79 mol %. The depleted-U assay is not given: 0.2 % is
//!   assumed, and 0.7 % would move ²³⁸U by 0.5 %. The enriched lot's
//!   ²³⁴U/²³⁶U are counted as ²³⁸U, and Li is 99.99 % ⁷Li. The script is
//!   `verification_and_validation/msre_n1_first_criticality/scripts/salt.py`.
//! - **Graphite:** CGB as in N-C1 (gh:#1126). The ENDF/B-VIII.0 reactor
//!   graphite 30P S(α,β) is applied by the caller.
//! - **INOR-8**, TM-0728 Table 2.2: 0.317 lb/in³. The composition is Ni 71,
//!   Mo 16.5, Cr 7 and Fe 4 wt %; the ~1.5 % of minor elements is omitted.
//!   ~~**Mo is a data gap** (only a beta ⁹⁵Mo tape is in `reference-data/endf/`),
//!   handled by [`MoTreatment`].~~ **CLOSED 2026-11-09:** natural Mo is read
//!   from the seven ENDF/B-VIII.0 tapes in `reference-data/ace`
//!   ([`MoTreatment::Natural`]); the other arms are kept as ablations. The script is `scripts/inor8.py` beside
//!   `salt.py`.

use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use crate::geometry::geometry::Geometry;
use crate::geometry::lattice::{Lattice, RectLattice};
use crate::geometry::position::Position;
use crate::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane};
use crate::geometry::universe::Universe;
use crate::physics::transport_csg::SourceBox;

/// One inch \[cm\].
pub const INCH: f64 = 2.54;
/// 1181 °F, the state of ORNL-4233 Table 1 \[K\].
pub const TEMPERATURE_K: f64 = (1181.0 - 32.0) * 5.0 / 9.0 + 273.15;
/// The k this rods-out model should give: the real state carried 0.08 %
/// δk/k of rod poisoning (ORNL-4233 Table 1).
pub const REFERENCE_K: f64 = 1.0 / (1.0 - 0.0008);
/// Stringer pitch, 2 in \[cm\].
pub const PITCH: f64 = 2.0 * INCH;
/// Graphite core radius, 55¼ in / 2 (TM-0728 Table 5.4) \[cm\].
pub const R_GRAPHITE: f64 = 55.25 / 2.0 * INCH;
/// Core height, 64 in (Table 5.4) \[cm\].
pub const H_CORE: f64 = 64.0 * INCH;
/// Core can inner radius, 55½ in / 2 (§5.3.1) \[cm\].
pub const R_CAN_IN: f64 = 55.5 / 2.0 * INCH;
/// Core can outer radius, 56 in / 2 (Table 5.2) \[cm\].
pub const R_CAN_OUT: f64 = 56.0 / 2.0 * INCH;
/// Vessel inner radius, 58 in / 2 (Table 5.2) \[cm\].
pub const R_VESSEL_IN: f64 = 58.0 / 2.0 * INCH;
/// Vessel outer radius, 59⅛ in / 2 (Table 5.2) \[cm\].
pub const R_VESSEL_OUT: f64 = 59.125 / 2.0 * INCH;
/// Head thickness, 1 in (Table 5.2) \[cm\].
pub const T_HEAD: f64 = INCH;
/// One cubic foot \[cm³\].
pub const FT3: f64 = 12.0 * 12.0 * 12.0 * INCH * INCH * INCH;
/// Upper-head salt volume, 10.5 ft³ (Table 5.4) \[cm³\].
pub const V_UPPER_HEAD: f64 = 10.5 * FT3;
/// Lower-head salt volume, 10.0 ft³ (Table 5.4) \[cm³\].
pub const V_LOWER_HEAD: f64 = 10.0 * FT3;
/// Half-depth of a fuel passage in one stringer face, 0.2 in \[cm\].
pub const PASSAGE_HALF_DEPTH: f64 = 0.2 * INCH;
/// Half the straight length of a passage, (1.2 − 0.4)/2 in \[cm\].
pub const PASSAGE_HALF_FLAT: f64 = 0.4 * INCH;
/// Radius of a passage's rounded ends, 0.2 in \[cm\].
pub const PASSAGE_END_RADIUS: f64 = 0.2 * INCH;
/// Distance of each centre opening from either axis, 2 in (Fig. 5.7) \[cm\].
pub const OPENING_OFFSET: f64 = 2.0 * INCH;
/// Radius of each centre opening, 2.5 in / 2, read from Fig. 5.7 \[cm\].
pub const R_OPENING: f64 = 1.25 * INCH;
/// Thimble outer radius, 2 in OD / 2 (§5.3.5.2) \[cm\].
pub const R_THIMBLE_OUT: f64 = INCH;
/// Thimble inner radius, outer less the 0.065 in wall \[cm\].
pub const R_THIMBLE_IN: f64 = INCH - 0.065 * INCH;

/// Fuel fraction of one stringer cell, analytic: four half-stadia of
/// `0.4 × 0.2 in² + 2 × π (0.2 in)²/4` each, over `(2 in)²`.
pub const STRINGER_FUEL_FRACTION: f64 =
    4.0 * (2.0 * 0.4 * 0.2 + 2.0 * core::f64::consts::PI * 0.2 * 0.2 / 4.0) / 4.0;

/// Height of the lower salt plenum, holding [`V_LOWER_HEAD`] \[cm\].
pub fn lower_plenum_height() -> f64 {
    V_LOWER_HEAD / (core::f64::consts::PI * R_VESSEL_IN * R_VESSEL_IN)
}

/// Height of the upper salt plenum, holding [`V_UPPER_HEAD`] \[cm\].
pub fn upper_plenum_height() -> f64 {
    V_UPPER_HEAD / (core::f64::consts::PI * R_VESSEL_IN * R_VESSEL_IN)
}

/// How INOR-8's molybdenum is represented. ~~The gap in `reference-data/endf/`
/// (only `n-042_Mo_095-ENDF8.0-beta.endf`), bounded by two arms.~~
/// **UPDATED 2026-11-09:** [`MoTreatment::Natural`] is the model (N-1:
/// +801 ± 22 pcm under the RVS default, +772 ± 20 with DBRC, six seeds
/// each; see the V&V README); `Omitted` and `Mo95Equivalent` are ablations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoTreatment {
    /// Arm A: molybdenum left out of INOR-8 (its mass is removed).
    Omitted,
    /// Arm B: natural Mo replaced by ⁹⁵Mo at equal 2200 m/s absorption,
    /// `N(⁹⁵Mo) = N(Mo) × 2.48 / 13.4` (σ_a quoted from memory, not
    /// re-checked; see gh:#1128).
    Mo95Equivalent,
    /// Arm C (2026-11-09, once the tapes were added to `reference-data/ace`,
    /// `ba58c2e`): natural Mo, all seven stable isotopes from the
    /// ENDF/B-VIII.0 release tapes at IUPAC natural abundance, at the same
    /// element density as the other arms. **This is INOR-8 as specified.**
    Natural,
}

/// Natural Mo abundances, atom fraction (IUPAC 2013 isotopic compositions).
pub const MO_NATURAL_ABUNDANCE: [(&str, f64); 7] = [
    ("Mo92", 0.1453),
    ("Mo94", 0.0915),
    ("Mo95", 0.1584),
    ("Mo96", 0.1667),
    ("Mo97", 0.0960),
    ("Mo98", 0.2439),
    ("Mo100", 0.0982),
];

/// INOR-8's Mo element density (atoms/b-cm), `scripts/inor8.py`: 16.5 wt %
/// of 8.775 g/cm³ at an atomic weight of 95.95.
pub const INOR8_MO_ELEMENT: f64 = 9.086858e-03;

/// The Mo tapes an arm reads, `(nuclide, path relative to reference-data/endf/)`.
pub fn mo_tapes(mo: MoTreatment) -> Vec<(&'static str, &'static str)> {
    match mo {
        MoTreatment::Omitted => vec![],
        MoTreatment::Mo95Equivalent => vec![MO95_TAPE],
        MoTreatment::Natural => vec![
            ("Mo92", "../ace/endf/endf-b-viii.0/n-042_Mo_092-ENDF8.0.endf"),
            ("Mo94", "../ace/endf/endf-b-viii.0/n-042_Mo_094-ENDF8.0.endf"),
            ("Mo95", "../ace/endf/endf-b-viii.0/n-042_Mo_095-ENDF8.0.endf"),
            ("Mo96", "../ace/endf/endf-b-viii.0/n-042_Mo_096-ENDF8.0.endf"),
            ("Mo97", "../ace/endf/endf-b-viii.0/n-042_Mo_097-ENDF8.0.endf"),
            ("Mo98", "../ace/endf/endf-b-viii.0/n-042_Mo_098-ENDF8.0.endf"),
            ("Mo100", "../ace/endf/endf-b-viii.0/n-042_Mo_100-ENDF8.0.endf"),
        ],
    }
}

/// `(nuclide, ENDF/B-VIII.0 tape)`, paths relative to `reference-data/endf/`.
/// Ni and Zr live in the `reference-data/ace` submodule's `endf/` (the
/// maintainer's rule for new nuclear data, 2026-11-09).
pub const NUCLIDES: [(&str, &str); 28] = [
    ("Li6", "n-003_Li_006-ENDF8.0.endf"),
    ("Li7", "n-003_Li_007-ENDF8.0.endf"),
    ("Be9", "n-004_Be_009-ENDF8.0.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
    ("B11", "n-005_B_011-ENDF8.0.endf"),
    ("C12", "n-006_C_012-ENDF8.0.endf"),
    ("C13", "n-006_C_013-ENDF8.0.endf"),
    ("F19", "n-009_F_019-ENDF8.0.endf"),
    ("Cr50", "n-024_Cr_050-ENDF8.0.endf"),
    ("Cr52", "n-024_Cr_052-ENDF8.0.endf"),
    ("Cr53", "n-024_Cr_053-ENDF8.0.endf"),
    ("Cr54", "n-024_Cr_054-ENDF8.0.endf"),
    ("Fe54", "n-026_Fe_054-ENDF8.0.endf"),
    ("Fe56", "n-026_Fe_056-ENDF8.0.endf"),
    ("Fe57", "n-026_Fe_057-ENDF8.0.endf"),
    ("Fe58", "n-026_Fe_058-ENDF8.0.endf"),
    ("Ni58", "../ace/endf/endf-b-viii.0/n-028_Ni_058-ENDF8.0.endf"),
    ("Ni60", "../ace/endf/endf-b-viii.0/n-028_Ni_060-ENDF8.0.endf"),
    ("Ni61", "../ace/endf/endf-b-viii.0/n-028_Ni_061-ENDF8.0.endf"),
    ("Ni62", "../ace/endf/endf-b-viii.0/n-028_Ni_062-ENDF8.0.endf"),
    ("Ni64", "../ace/endf/endf-b-viii.0/n-028_Ni_064-ENDF8.0.endf"),
    ("Zr90", "../ace/endf/endf-b-viii.0/n-040_Zr_090-ENDF8.0.endf"),
    ("Zr91", "../ace/endf/endf-b-viii.0/n-040_Zr_091-ENDF8.0.endf"),
    ("Zr92", "../ace/endf/endf-b-viii.0/n-040_Zr_092-ENDF8.0.endf"),
    ("Zr94", "../ace/endf/endf-b-viii.0/n-040_Zr_094-ENDF8.0.endf"),
    ("Zr96", "../ace/endf/endf-b-viii.0/n-040_Zr_096-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
];

/// The beta ⁹⁵Mo tape, used only by [`MoTreatment::Mo95Equivalent`].
pub const MO95_TAPE: (&str, &str) = ("Mo95", "n-042_Mo_095-ENDF8.0-beta.endf");

/// Fuel salt at first criticality (atoms/b-cm), from `scripts/salt.py`.
pub const SALT: [(&str, f64); 11] = [
    ("Li7", 2.218270e-02),
    ("Li6", 2.218492e-06),
    ("Be9", 9.910143e-03),
    ("U235", 8.398016e-05),
    ("U238", 1.845625e-04),
    ("F19", 4.964736e-02),
    ("Zr90", 8.448066e-04),
    ("Zr91", 1.842319e-04),
    ("Zr92", 2.816022e-04),
    ("Zr94", 2.853788e-04),
    ("Zr96", 4.597587e-05),
];

/// CGB graphite, 1.87 g/cm³ with 0.8 wt ppm natural boron (TM-0728 §5.3.2,
/// Table 5.3), the N-C1 numbers.
pub const GRAPHITE: [(&str, f64); 4] = [
    ("C12", 9.271946e-02),
    ("C13", 1.038652e-03),
    ("B10", 1.651533e-08),
    ("B11", 6.681124e-08),
];

/// INOR-8 without molybdenum (atoms/b-cm), from `scripts/inor8.py`.
pub const INOR8_WITHOUT_MO: [(&str, f64); 13] = [
    ("Ni58", 4.351546e-02),
    ("Ni60", 1.676208e-02),
    ("Ni61", 7.286360e-04),
    ("Ni62", 2.323210e-03),
    ("Ni64", 5.916532e-04),
    ("Cr50", 3.090949e-04),
    ("Cr52", 5.960587e-03),
    ("Cr53", 6.758827e-04),
    ("Cr54", 1.682415e-04),
    ("Fe54", 2.212254e-04),
    ("Fe56", 3.472766e-03),
    ("Fe57", 8.020130e-05),
    ("Fe58", 1.067332e-05),
];

/// ⁹⁵Mo standing in for INOR-8's natural Mo at equal thermal absorption
/// (atoms/b-cm): `9.086858e-03 × 2.48 / 13.4`.
pub const INOR8_MO95_EQUIVALENT: f64 = 1.681747e-03;

/// Volume fractions in the homogenised sample-basket opening: 12 INOR-8 rods
/// of 0.25 in diameter and 15 graphite bars of 0.25 × 0.47 in (§5.3.6), over a
/// 2.5 in circle; the rest is salt. Returns `(INOR-8, graphite, salt)`.
pub fn basket_fractions() -> (f64, f64, f64) {
    let opening = core::f64::consts::PI * 1.25 * 1.25;
    let inor = 12.0 * core::f64::consts::PI * 0.125 * 0.125 / opening;
    let graphite = 15.0 * 0.25 * 0.47 / opening;
    (inor, graphite, 1.0 - inor - graphite)
}

/// INOR-8 under `mo`.
pub fn inor8(mo: MoTreatment) -> Vec<(&'static str, f64)> {
    let mut v: Vec<(&'static str, f64)> = INOR8_WITHOUT_MO.to_vec();
    match mo {
        MoTreatment::Omitted => {}
        MoTreatment::Mo95Equivalent => v.push(("Mo95", INOR8_MO95_EQUIVALENT)),
        MoTreatment::Natural => {
            for (n, a) in MO_NATURAL_ABUNDANCE {
                v.push((n, INOR8_MO_ELEMENT * a));
            }
        }
    }
    v
}

/// The sample-basket opening, homogenised: [`basket_fractions`] of INOR-8,
/// graphite and salt, summed nuclide by nuclide.
pub fn basket_mix(mo: MoTreatment) -> Vec<(&'static str, f64)> {
    materials_at(mo, REFERENCE_TEMPERATURE_F).swap_remove(BASKET_IDX).1
}

/// Nuclides that live only in graphite (and the basket's graphite share):
/// these take the graphite temperature in a fuel/graphite split.
pub const GRAPHITE_NUCLIDES: [&str; 4] = ["C12", "C13", "B10", "B11"];

/// Material indices of [`materials`].
pub const SALT_IDX: usize = 0;
/// Graphite's index.
pub const GRAPHITE_IDX: usize = 1;
/// INOR-8's index.
pub const INOR8_IDX: usize = 2;
/// The homogenised sample basket's index.
pub const BASKET_IDX: usize = 3;

/// The four materials at the reference state, in index order: salt,
/// graphite, INOR-8, basket.
pub fn materials(mo: MoTreatment) -> Vec<(&'static str, Vec<(&'static str, f64)>)> {
    materials_at(mo, REFERENCE_TEMPERATURE_F)
}

/// Fuel-salt fractional density change per °F, **−1.18 × 10⁻⁴**: the
/// expansion coefficient ORNL-4233 §4.1 (p. 38) says its calculated
/// coefficients were based on. The same section reports −1.09 and
/// −1.15 × 10⁻⁴ from pump-bowl level measurements; the fuel part of the
/// temperature coefficient scales nearly in proportion (ORNL-4233 p. 38).
pub const SALT_EXPANSION_PER_F: f64 = -1.18e-4;

/// The first-criticality temperature, 1181 °F (ORNL-4233 Table 1).
pub const REFERENCE_TEMPERATURE_F: f64 = 1181.0;

/// Kelvin from °F.
pub fn kelvin_from_f(t_f: f64) -> f64 {
    (t_f - 32.0) * 5.0 / 9.0 + 273.15
}

/// The materials at `t_f` °F, for the isothermal coefficient (MSRE N-3,
/// gh:#1129). Only the salt's density moves, by [`SALT_EXPANSION_PER_F`]
/// from the reference state, in the salt and in the basket's salt share.
/// Graphite and INOR-8 densities and every dimension stay at their cold
/// values (graphite expands by ~1.5e-6 /°F linear, TM-0728 §5.3.1: left
/// out).
pub fn materials_at(mo: MoTreatment, t_f: f64) -> Vec<(&'static str, Vec<(&'static str, f64)>)> {
    let f = 1.0 + SALT_EXPANSION_PER_F * (t_f - REFERENCE_TEMPERATURE_F);
    let salt: Vec<(&'static str, f64)> = SALT.iter().map(|&(n, d)| (n, d * f)).collect();
    let (fi, fg, fs) = basket_fractions();
    let mut basket: Vec<(&'static str, f64)> = Vec::new();
    let parts: [(f64, Vec<(&'static str, f64)>); 3] =
        [(fs, salt.clone()), (fg, GRAPHITE.to_vec()), (fi, inor8(mo))];
    for (w, comps) in parts {
        for (n, d) in comps {
            match basket.iter_mut().find(|(m, _)| *m == n) {
                Some(e) => e.1 += w * d,
                None => basket.push((n, w * d)),
            }
        }
    }
    vec![
        ("MSRE fuel salt, first criticality", salt),
        ("MSRE CGB graphite", GRAPHITE.to_vec()),
        ("INOR-8", inor8(mo)),
        ("Sample baskets (homogenised)", basket),
    ]
}

fn hs(surface_idx: usize, inside: bool) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx,
        sense: if inside { HalfSpaceSense::Inside } else { HalfSpaceSense::Outside },
    }
}

/// Intersection of the given half-spaces, in RPN.
fn all_of(tokens: &[RegionToken]) -> Vec<RegionToken> {
    let mut v = vec![tokens[0].clone()];
    for t in &tokens[1..] {
        v.push(t.clone());
        v.push(RegionToken::Intersection);
    }
    v
}

/// Union of RPN regions.
fn any_of(regions: Vec<Vec<RegionToken>>) -> Vec<RegionToken> {
    let mut it = regions.into_iter();
    let mut v = it.next().expect("at least one region");
    for r in it {
        v.extend(r);
        v.push(RegionToken::Union);
    }
    v
}

fn zcyl(x0: f64, y0: f64, r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::ZCylinder(ZCylinder { x0, y0, r, bc })
}

/// Linear scale factors applied to the cold (as-drawn) dimensions: one for
/// everything graphite, one for everything INOR-8. Both 1.0 is the cold
/// geometry every N-1 and N-3 record used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dims {
    /// Graphite: stringer pitch and passages, core radius and height, the
    /// openings' offsets and radius.
    pub graphite: f64,
    /// INOR-8: core can, vessel, heads, thimbles; the head volumes scale with
    /// its cube.
    pub inor8: f64,
}

/// Graphite's mean expansion coefficient, the middle of TM-0728 §5.3's
/// "1.3 to 1.7 × 10⁻⁶ in./in.-°F" \[1/°F\].
pub const GRAPHITE_EXPANSION_PER_F: f64 = 1.5e-6;
/// INOR-8's, "7.8 × 10⁻⁶ in./in.-°F (in the 70-1200°F range)", the same
/// passage of TM-0728 §5.3 (Table 2.2 gives 8.0 × 10⁻⁶ over 70-1300 °F)
/// \[1/°F\].
pub const INOR8_EXPANSION_PER_F: f64 = 7.8e-6;
/// The temperature the drawings' dimensions hold at \[°F\].
pub const COLD_TEMPERATURE_F: f64 = 70.0;

impl Dims {
    /// The cold, as-drawn dimensions.
    pub const COLD: Dims = Dims { graphite: 1.0, inor8: 1.0 };

    /// Dimensions at `t_f` (°F) with the given expansion coefficients
    /// (1/°F), from [`COLD_TEMPERATURE_F`].
    pub fn hot(t_f: f64, graphite_per_f: f64, inor8_per_f: f64) -> Dims {
        let dt = t_f - COLD_TEMPERATURE_F;
        Dims { graphite: 1.0 + graphite_per_f * dt, inor8: 1.0 + inor8_per_f * dt }
    }
}

/// [`materials_at`] with the solid densities thinned to match `d`:
/// graphite by `1/d.graphite³`, INOR-8 by `1/d.inor8³` (mass conserved as the
/// parts expand). The homogenised sample basket is left as it is (its volume
/// is a few hundred cm³).
pub fn materials_with_dims(mo: MoTreatment, t_f: f64, d: Dims) -> Vec<(&'static str, Vec<(&'static str, f64)>)> {
    let mut m = materials_at(mo, t_f);
    for (idx, scale) in [(GRAPHITE_IDX, d.graphite), (INOR8_IDX, d.inor8)] {
        let f = 1.0 / (scale * scale * scale);
        for e in m[idx].1.iter_mut() {
            e.1 *= f;
        }
    }
    m
}

/// How the regions above and below the graphite are represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ends {
    /// Every record so far: each head is a pure-salt plenum of the vessel's
    /// inner radius holding TM-0728 Table 5.4's volume.
    VolumeEquivalent,
    /// ORNL's own axial end regions, ORNL-TM-0730 Table 3.1 (the 20-region
    /// R-Z model, PDF p. 26): homogenised layers above the graphite (H, G, E,
    /// then the salt head D, 10.33 in in all) and below it (O, the support
    /// grid, then the bottom head P, 9.14 in in all), with Table 3.1's fuel /
    /// graphite / INOR-8 volume fractions and thicknesses, anchored at this
    /// model's graphite faces. Radially each layer fills the vessel; Table
    /// 3.1's r < 3 in sub-regions are not represented.
    Tm0730,
}

/// TM-0730 Table 3.1's axial end layers: `(name, thickness in., fuel,
/// graphite, INOR-8)` volume fractions. Above the graphite, upward: H, G, E;
/// the head D (pure salt) fills the rest to 10.33 in. Below it, downward:
/// O, then the bottom head P.
pub const TM0730_TOP: [(&str, f64, f64, f64, f64); 3] = [
    ("H", 0.94, 0.633, 0.365, 0.002),
    ("G", 0.69, 0.946, 0.054, 0.0),
    ("E", 1.25, 0.937, 0.035, 0.028),
];
/// Height of TM-0730's salt above the graphite top, 74.92 - 64.59 in.
pub const TM0730_TOP_HEIGHT_IN: f64 = 74.92 - 64.59;
/// TM-0730's layers below the graphite, downward: O (support grid), P (bottom head).
pub const TM0730_BOTTOM: [(&str, f64, f64, f64, f64); 2] = [
    ("O", 1.41, 0.669, 0.153, 0.178),
    ("P", 7.73, 0.908, 0.0, 0.092),
];
/// Material indices of the TM-0730 end mixes in [`materials_with_ends`]:
/// H, G, E, O, P, after the four standard materials.
pub const END_MIX_IDX: [usize; 5] = [4, 5, 6, 7, 8];

/// [`materials_with_dims`] plus the five TM-0730 end mixes (H, G, E, O, P),
/// each a volume-fraction blend of the salt, graphite and INOR-8 above.
pub fn materials_with_ends(mo: MoTreatment, t_f: f64, d: Dims) -> Vec<(&'static str, Vec<(&'static str, f64)>)> {
    let mut m = materials_with_dims(mo, t_f, d);
    let (salt, graphite, inor) = (m[SALT_IDX].1.clone(), m[GRAPHITE_IDX].1.clone(), m[INOR8_IDX].1.clone());
    let names = ["TM-0730 end H", "TM-0730 end G", "TM-0730 end E", "TM-0730 end O", "TM-0730 end P"];
    let fracs: Vec<(f64, f64, f64)> =
        TM0730_TOP.iter().chain(TM0730_BOTTOM.iter()).map(|&(_, _, f, g, i)| (f, g, i)).collect();
    for (name, (ff, fg, fi)) in names.into_iter().zip(fracs) {
        let mut mix: Vec<(&'static str, f64)> = Vec::new();
        for (w, comps) in [(ff, &salt), (fg, &graphite), (fi, &inor)] {
            for &(n, dens) in comps.iter() {
                if w == 0.0 {
                    continue;
                }
                match mix.iter_mut().find(|(k, _)| *k == n) {
                    Some(e) => e.1 += w * dens,
                    None => mix.push((n, w * dens)),
                }
            }
        }
        m.push((name, mix));
    }
    m
}

/// The stringer universe's cells, appended to `surfaces` and `cells`: four
/// half-stadium passages (salt) and the graphite around them. Returns the
/// cell indices.
fn stringer_cells(surfaces: &mut Vec<SurfaceKind>, cells: &mut Vec<Cell>, t_salt_k: f64, t_graphite_k: f64, d: Dims) -> Vec<usize> {
    let t = BoundaryType::Transmissive;
    let g = d.graphite;
    let h = PITCH * g / 2.0;
    let floor = h - PASSAGE_HALF_DEPTH * g;
    let a = PASSAGE_HALF_FLAT * g;
    let r = PASSAGE_END_RADIUS * g;
    let mut add = |s: SurfaceKind| {
        surfaces.push(s);
        surfaces.len() - 1
    };
    let xf_p = add(SurfaceKind::XPlane(XPlane { x0: floor, bc: t }));
    let xf_m = add(SurfaceKind::XPlane(XPlane { x0: -floor, bc: t }));
    let yf_p = add(SurfaceKind::YPlane(YPlane { y0: floor, bc: t }));
    let yf_m = add(SurfaceKind::YPlane(YPlane { y0: -floor, bc: t }));
    let ya_p = add(SurfaceKind::YPlane(YPlane { y0: a, bc: t }));
    let ya_m = add(SurfaceKind::YPlane(YPlane { y0: -a, bc: t }));
    let xa_p = add(SurfaceKind::XPlane(XPlane { x0: a, bc: t }));
    let xa_m = add(SurfaceKind::XPlane(XPlane { x0: -a, bc: t }));
    let mut passages = Vec::new();
    // ±x faces: a slab beyond the floor, |y| < a, and two end circles on the face.
    for (floor_idx, beyond, sx) in [(xf_p, false, 1.0), (xf_m, true, -1.0)] {
        let slab = all_of(&[hs(floor_idx, beyond), hs(ya_m, false), hs(ya_p, true)]);
        let c1 = add(zcyl(sx * h, a, r, t));
        let c2 = add(zcyl(sx * h, -a, r, t));
        passages.push(any_of(vec![slab, vec![hs(c1, true)], vec![hs(c2, true)]]));
    }
    for (floor_idx, beyond, sy) in [(yf_p, false, 1.0), (yf_m, true, -1.0)] {
        let slab = all_of(&[hs(floor_idx, beyond), hs(xa_m, false), hs(xa_p, true)]);
        let c1 = add(zcyl(a, sy * h, r, t));
        let c2 = add(zcyl(-a, sy * h, r, t));
        passages.push(any_of(vec![slab, vec![hs(c1, true)], vec![hs(c2, true)]]));
    }
    let fuel = any_of(passages);
    let mut graphite = fuel.clone();
    graphite.push(RegionToken::Complement);
    let i0 = cells.len();
    cells.push(Cell::material(101, fuel, SALT_IDX, t_salt_k));
    cells.push(Cell::material(102, graphite, GRAPHITE_IDX, t_graphite_k));
    vec![i0, i0 + 1]
}

/// Centres of the four openings: three thimbles, then the sample baskets
/// (Fig. 5.7: thimbles upper-left, upper-right, lower-left).
pub const OPENINGS: [(f64, f64); 4] = [
    (-OPENING_OFFSET, OPENING_OFFSET),
    (OPENING_OFFSET, OPENING_OFFSET),
    (-OPENING_OFFSET, -OPENING_OFFSET),
    (OPENING_OFFSET, -OPENING_OFFSET),
];

/// The assembled full-core geometry at the reference temperature. `z = 0`
/// is the bottom of the graphite.
pub fn geometry() -> Geometry {
    geometry_at(TEMPERATURE_K)
}

/// The same geometry with every cell at `t_k` kelvin (dimensions stay cold).
pub fn geometry_at(t_k: f64) -> Geometry {
    geometry_at_split(t_k, t_k)
}

/// The geometry with the stringer graphite at `t_graphite_k` and every other
/// cell (salt, INOR-8, the basket) at `t_salt_k`: the fuel/graphite split
/// of the temperature coefficient (MSRE N-3, gh:#1129).
pub fn geometry_at_split(t_salt_k: f64, t_graphite_k: f64) -> Geometry {
    geometry_with_dims(t_salt_k, t_graphite_k, Dims::COLD)
}

/// [`geometry_at_split`] with the dimensions scaled by `d` (N-1 residual,
/// hot dimensions, gh:#1128). `Dims::COLD` builds the cold geometry
/// unchanged, which `tests::cold_dims_build_the_recorded_geometry` pins.
pub fn geometry_with_dims(t_salt_k: f64, t_graphite_k: f64, d: Dims) -> Geometry {
    geometry_with_ends(t_salt_k, t_graphite_k, d, Ends::VolumeEquivalent)
}

/// [`geometry_with_dims`] with a choice of end regions ([`Ends`]).
/// `Ends::Tm0730` needs [`materials_with_ends`] (its mixes are materials
/// 4-8). `Ends::VolumeEquivalent` builds exactly [`geometry_with_dims`].
pub fn geometry_with_ends(t_salt_k: f64, t_graphite_k: f64, d: Dims, ends: Ends) -> Geometry {
    let t_k = t_salt_k;
    let (g, i8) = (d.graphite, d.inor8);
    let t = BoundaryType::Transmissive;
    let v = BoundaryType::Vacuum;
    let mut surfaces: Vec<SurfaceKind> = Vec::new();
    let mut cells: Vec<Cell> = Vec::new();
    let stringer = stringer_cells(&mut surfaces, &mut cells, t_salt_k, t_graphite_k, d);

    let mut add = |s: SurfaceKind| {
        surfaces.push(s);
        surfaces.len() - 1
    };
    let r_vessel_in = R_VESSEL_IN * i8;
    let plenum = |v_head: f64| (v_head * i8 * i8 * i8) / (core::f64::consts::PI * r_vessel_in * r_vessel_in);
    let h_core = H_CORE * g;
    let (z_lp, z_up) = match ends {
        Ends::VolumeEquivalent => (-plenum(V_LOWER_HEAD), h_core + plenum(V_UPPER_HEAD)),
        Ends::Tm0730 => {
            let bottom: f64 = TM0730_BOTTOM.iter().map(|x| x.1).sum();
            (-bottom * INCH * i8, h_core + TM0730_TOP_HEIGHT_IN * INCH * i8)
        }
    };
    let r_g = add(zcyl(0.0, 0.0, R_GRAPHITE * g, t));
    let r_ci = add(zcyl(0.0, 0.0, R_CAN_IN * i8, t));
    let r_co = add(zcyl(0.0, 0.0, R_CAN_OUT * i8, t));
    let r_vi = add(zcyl(0.0, 0.0, r_vessel_in, t));
    let r_vo = add(zcyl(0.0, 0.0, R_VESSEL_OUT * i8, v));
    let z0 = add(SurfaceKind::ZPlane(ZPlane { z0: 0.0, bc: t }));
    let zt = add(SurfaceKind::ZPlane(ZPlane { z0: h_core, bc: t }));
    let zl = add(SurfaceKind::ZPlane(ZPlane { z0: z_lp, bc: t }));
    let zu = add(SurfaceKind::ZPlane(ZPlane { z0: z_up, bc: t }));
    let zlb = add(SurfaceKind::ZPlane(ZPlane { z0: z_lp - T_HEAD * i8, bc: v }));
    let zut = add(SurfaceKind::ZPlane(ZPlane { z0: z_up + T_HEAD * i8, bc: v }));
    let openings: Vec<usize> = OPENINGS.iter().map(|&(x, y)| add(zcyl(x * g, y * g, R_OPENING * g, t))).collect();
    let thimbles_out: Vec<usize> =
        OPENINGS[..3].iter().map(|&(x, y)| add(zcyl(x * g, y * g, R_THIMBLE_OUT * i8, t))).collect();
    let thimbles_in: Vec<usize> =
        OPENINGS[..3].iter().map(|&(x, y)| add(zcyl(x * g, y * g, R_THIMBLE_IN * i8, t))).collect();

    let core_z = [hs(z0, false), hs(zt, true)];
    let plenum_up_z = [hs(zt, false), hs(zu, true)];
    let thimble_z = [hs(z0, false), hs(zu, true)];
    let mut root: Vec<Cell> = Vec::new();
    let mat = |id: i32, region: Vec<RegionToken>, m: usize| Cell::material(id, region, m, t_k);

    // The lattice-filled core, outside the four openings.
    let mut core = vec![hs(r_g, true)];
    core.extend(core_z.iter().cloned());
    core.extend(openings.iter().map(|&o| hs(o, false)));
    root.push(Cell::fill(1, all_of(&core), CellFill::Lattice(0), Position::ZERO));
    // Thimbles: wall and void through the core and the upper plenum; salt
    // between the wall and the opening, in the core.
    for i in 0..3 {
        let mut wall = vec![hs(thimbles_out[i], true), hs(thimbles_in[i], false)];
        wall.extend(thimble_z.iter().cloned());
        root.push(mat(10 + i as i32, all_of(&wall), INOR8_IDX));
        let mut inside = vec![hs(thimbles_in[i], true)];
        inside.extend(thimble_z.iter().cloned());
        root.push(Cell::fill(20 + i as i32, all_of(&inside), CellFill::Void, Position::ZERO));
        let mut annulus = vec![hs(openings[i], true), hs(thimbles_out[i], false)];
        annulus.extend(core_z.iter().cloned());
        root.push(mat(30 + i as i32, all_of(&annulus), SALT_IDX));
    }
    let mut basket = vec![hs(openings[3], true)];
    basket.extend(core_z.iter().cloned());
    root.push(mat(40, all_of(&basket), BASKET_IDX));
    // Radial layers beside the core.
    for (id, inner, outer, m) in [(50, r_g, r_ci, SALT_IDX), (51, r_ci, r_co, INOR8_IDX), (52, r_co, r_vi, SALT_IDX)] {
        let mut c = vec![hs(inner, false), hs(outer, true)];
        c.extend(core_z.iter().cloned());
        root.push(mat(id, all_of(&c), m));
    }
    // Plenums.
    match ends {
        Ends::VolumeEquivalent => {
            root.push(mat(60, all_of(&[hs(r_vi, true), hs(zl, false), hs(z0, true)]), SALT_IDX));
            let mut upper = vec![hs(r_vi, true)];
            upper.extend(plenum_up_z.iter().cloned());
            upper.extend(thimbles_out.iter().map(|&o| hs(o, false)));
            root.push(mat(61, all_of(&upper), SALT_IDX));
        }
        Ends::Tm0730 => {
            // Below: P from the head plate up to O, O up to the graphite.
            let z_o = -TM0730_BOTTOM[0].1 * INCH * i8;
            let zo = add(SurfaceKind::ZPlane(ZPlane { z0: z_o, bc: t }));
            root.push(mat(60, all_of(&[hs(r_vi, true), hs(zl, false), hs(zo, true)]), END_MIX_IDX[4]));
            root.push(mat(62, all_of(&[hs(r_vi, true), hs(zo, false), hs(z0, true)]), END_MIX_IDX[3]));
            // Above: H, G, E, then the salt head D, each around the thimbles.
            let mut lo = zt;
            let mut z = h_core;
            for (k, layer) in TM0730_TOP.iter().enumerate() {
                z += layer.1 * INCH * i8;
                let hi = add(SurfaceKind::ZPlane(ZPlane { z0: z, bc: t }));
                let mut c = vec![hs(r_vi, true), hs(lo, false), hs(hi, true)];
                c.extend(thimbles_out.iter().map(|&o| hs(o, false)));
                root.push(mat(63 + k as i32, all_of(&c), END_MIX_IDX[k]));
                lo = hi;
            }
            let mut c = vec![hs(r_vi, true), hs(lo, false), hs(zu, true)];
            c.extend(thimbles_out.iter().map(|&o| hs(o, false)));
            root.push(mat(61, all_of(&c), SALT_IDX));
        }
    }
    // Vessel wall and heads.
    root.push(mat(70, all_of(&[hs(r_vi, false), hs(r_vo, true), hs(zl, false), hs(zu, true)]), INOR8_IDX));
    root.push(mat(71, all_of(&[hs(r_vo, true), hs(zlb, false), hs(zl, true)]), INOR8_IDX));
    root.push(mat(72, all_of(&[hs(r_vo, true), hs(zu, false), hs(zut, true)]), INOR8_IDX));

    let root_start = cells.len();
    let root_count = root.len();
    cells.extend(root);
    let pitch = PITCH * g;
    let n = (2.0 * r_vessel_in / pitch).ceil() as usize | 1; // odd: a stringer on the axis
    let half = n as f64 * pitch / 2.0;
    let lattice = RectLattice {
        id: 1,
        n: [n, n, 1],
        lower_left: Position::new(-half, -half, 0.0),
        pitch: [pitch, pitch, 1.0],
        universes: vec![1; n * n],
        outer: None,
    };
    Geometry {
        surfaces,
        cells,
        universes: vec![
            Universe { id: 0, cell_indices: (root_start..root_start + root_count).collect() },
            Universe { id: 1, cell_indices: stringer },
        ],
        lattices: vec![Lattice::Rect(lattice)],
        root_universe: 0,
    }
}

/// Whether the thermal shield is modelled (N-1 residual ablation, gh:#1128).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shield {
    /// Vacuum outside the vessel: every recorded N-1 and N-3 result.
    #[default]
    Omitted,
    /// The shield's cylindrical section, ORNL-TM-0728 §5.3.10.1 (report
    /// p. 128): 151 in high, 125 in OD and 93 in ID, two 1 in Type 304 walls
    /// around a 14 in annulus of 1 in carbon-steel balls in cooling water.
    ///
    /// - The 6 in lining of insulation (§5.3.1) and the furnace annulus
    ///   (§5.3.11) between the vessel and the shield are void.
    /// - The base (an 18 in tank), the 16 in top cover and the three plugs are
    ///   **not modelled**. Their placement against the vessel is not given in
    ///   the text, and the cover has a 22⅝ in central hole. The model is the
    ///   cylinder alone, open above and below.
    /// - **The cylinder's height relative to the vessel is not given either.**
    ///   It is centred on the vessel's mid-height here; at 151 in it overlaps
    ///   the whole vessel (about 80 in) either way.
    Tm0728Cylinder,
}

/// The shield cylinder's inner radius (93 in ID), TM-0728 p. 128.
pub const SHIELD_R_IN: f64 = 93.0 / 2.0 * INCH;
/// The shield cylinder's outer radius (125 in OD), TM-0728 p. 128.
pub const SHIELD_R_OUT: f64 = 125.0 / 2.0 * INCH;
/// The shield cylinder's height, TM-0728 p. 128.
pub const SHIELD_HEIGHT: f64 = 151.0 * INCH;
/// The shield's 304 stainless wall plates, TM-0728 p. 128.
pub const SHIELD_PLATE: f64 = INCH;
/// The shield's iron fraction, homogenised over its 16 in, as TM-0728 states
/// it twice (p. 68 and §5.3.10.4): "50% of which is Fe and 50% is H2O".
/// With two solid 1 in plates that fixes the ball annulus at 6/14 steel.
/// TM-0728 also gives 70 tons of balls in the annulus (p. 128), which would
/// imply about 2/3 steel. The text's own 50 % is used here, and the
/// difference is left as an input uncertainty.
pub const SHIELD_IRON_FRACTION: f64 = 0.5;
/// The shield's temperature. The cooling water is not given a temperature
/// in TM-0728; 293.6 K is the nearest tabulated temperature of
/// `tsl-HinH2O` to room temperature.
pub const SHIELD_TEMPERATURE_K: f64 = 293.6;

/// The shield's nuclides at [`SHIELD_TEMPERATURE_K`], under their own names
/// (suffix `s`) so they are processed apart from the hot INOR-8's.
/// `(name, ENDF/B-VIII.0 tape)`; H1 takes `tsl-HinH2O` as its bound law.
pub const SHIELD_NUCLIDES: [(&str, &str); 15] = [
    ("Fe54s", "n-026_Fe_054-ENDF8.0.endf"),
    ("Fe56s", "n-026_Fe_056-ENDF8.0.endf"),
    ("Fe57s", "n-026_Fe_057-ENDF8.0.endf"),
    ("Fe58s", "n-026_Fe_058-ENDF8.0.endf"),
    ("Cr50s", "n-024_Cr_050-ENDF8.0.endf"),
    ("Cr52s", "n-024_Cr_052-ENDF8.0.endf"),
    ("Cr53s", "n-024_Cr_053-ENDF8.0.endf"),
    ("Cr54s", "n-024_Cr_054-ENDF8.0.endf"),
    ("Ni58s", "../ace/endf/endf-b-viii.0/n-028_Ni_058-ENDF8.0.endf"),
    ("Ni60s", "../ace/endf/endf-b-viii.0/n-028_Ni_060-ENDF8.0.endf"),
    ("Ni61s", "../ace/endf/endf-b-viii.0/n-028_Ni_061-ENDF8.0.endf"),
    ("Ni62s", "../ace/endf/endf-b-viii.0/n-028_Ni_062-ENDF8.0.endf"),
    ("Ni64s", "../ace/endf/endf-b-viii.0/n-028_Ni_064-ENDF8.0.endf"),
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
];

/// Atom densities (atoms/b-cm) of an element of mass density `rho` (g/cm³),
/// weight fraction `w` and atomic weight `a`, split by atom abundances.
fn element(rho: f64, w: f64, a: f64, isotopes: &[(&'static str, f64)]) -> Vec<(&'static str, f64)> {
    let n = rho * w * 0.602214076 / a;
    isotopes.iter().map(|&(k, f)| (k, n * f)).collect()
}

/// Type 304 stainless steel, nominal: 8.00 g/cm³, 19 wt % Cr, 9.5 wt % Ni,
/// the balance (Mn and Si folded in) Fe. TM-0728 names the grade only; the
/// composition is the grade's nominal, not a value from the report.
/// Natural abundances (IUPAC 2013).
pub fn ss304() -> Vec<(&'static str, f64)> {
    let mut v = element(8.00, 0.715, 55.845, &FE_NATURAL_SHIELD);
    v.extend(element(8.00, 0.19, 51.996, &[("Cr50s", 0.04345), ("Cr52s", 0.83789), ("Cr53s", 0.09501), ("Cr54s", 0.02365)]));
    v.extend(element(8.00, 0.095, 58.693, &[("Ni58s", 0.68077), ("Ni60s", 0.26223), ("Ni61s", 0.011399), ("Ni62s", 0.036346), ("Ni64s", 0.009255)]));
    v
}

const FE_NATURAL_SHIELD: [(&str, f64); 4] = [("Fe54s", 0.05845), ("Fe56s", 0.91754), ("Fe57s", 0.02119), ("Fe58s", 0.00282)];

/// The ball annulus homogenised: carbon-steel balls (as iron, 7.87 g/cm³)
/// at the steel fraction that makes the whole 16 in [`SHIELD_IRON_FRACTION`]
/// iron, in water at 0.9982 g/cm³ (293.6 K).
pub fn shield_ball_zone() -> Vec<(&'static str, f64)> {
    let total = SHIELD_R_OUT - SHIELD_R_IN;
    let annulus = total - 2.0 * SHIELD_PLATE;
    let f = (SHIELD_IRON_FRACTION * total - 2.0 * SHIELD_PLATE) / annulus;
    let mut v = element(7.87 * f, 1.0, 55.845, &FE_NATURAL_SHIELD);
    let molecules = 0.9982 * (1.0 - f) * 0.602214076 / 18.0153;
    v.push(("H1", 2.0 * molecules));
    v.push(("O16", molecules));
    v
}

/// The two shield materials, to append after the model's own; the first of
/// them is the index [`geometry_with_shield`] takes.
pub fn shield_materials() -> Vec<(&'static str, Vec<(&'static str, f64)>)> {
    vec![("Thermal shield 304 plate", ss304()), ("Thermal shield ball annulus", shield_ball_zone())]
}

/// The ball annulus's steel volume fraction at random close packing of
/// equal spheres, 0.64 (Scott & Kilgour 1969, J. Phys. D 2, 863). TM-0728's
/// 70 tons of balls (p. 128) in the 14 in annulus give about 0.68 for short
/// tons (11.9 m³ of annulus, 7870 kg/m³), which equal balls cannot reach; so
/// the packing limit bounds the steel the text's tonnage implies. An input
/// sensitivity against [`SHIELD_IRON_FRACTION`]'s 6/14 (gh:#1128).
pub const SHIELD_BALL_FRACTION_PACKED: f64 = 0.64;

/// [`shield_ball_zone`] with the balls at volume fraction `f_ball` of the
/// annulus and water in the rest. `shield_ball_zone()` is this at 6/14.
pub fn shield_ball_zone_with_fraction(f_ball: f64) -> Vec<(&'static str, f64)> {
    let mut v = element(7.87 * f_ball, 1.0, 55.845, &FE_NATURAL_SHIELD);
    let molecules = 0.9982 * (1.0 - f_ball) * 0.602214076 / 18.0153;
    v.push(("H1", 2.0 * molecules));
    v.push(("O16", molecules));
    v
}

/// [`shield_materials`] with the ball annulus at random close packing
/// ([`SHIELD_BALL_FRACTION_PACKED`]); the plates are unchanged.
pub fn shield_materials_packed() -> Vec<(&'static str, Vec<(&'static str, f64)>)> {
    vec![
        ("Thermal shield 304 plate", ss304()),
        ("Thermal shield ball annulus, close-packed", shield_ball_zone_with_fraction(SHIELD_BALL_FRACTION_PACKED)),
    ]
}

/// [`shield_ball_zone`] with its water removed: the same balls, the space
/// between them void. An ablation for splitting the shield's worth between
/// its steel and its water (gh:#1128); the real shield is water-filled
/// (TM-0728 §5.3.10.1).
pub fn shield_ball_zone_drained() -> Vec<(&'static str, f64)> {
    shield_ball_zone().into_iter().filter(|e| e.0.starts_with("Fe")).collect()
}

/// [`shield_materials`] with the ball annulus drained
/// ([`shield_ball_zone_drained`]); the plates are unchanged. Same order and
/// count, so [`geometry_with_shield`] takes it unchanged.
pub fn shield_materials_drained() -> Vec<(&'static str, Vec<(&'static str, f64)>)> {
    vec![("Thermal shield 304 plate", ss304()), ("Thermal shield ball annulus, drained", shield_ball_zone_drained())]
}

/// [`geometry_with_ends`] inside the thermal shield's cylinder
/// ([`Shield::Tm0728Cylinder`]). `first_shield_material` is the index of
/// [`shield_materials`]'s first entry in the run's material list.
///
/// Built from `geometry_with_ends` unchanged, then extended: the three
/// vacuum surfaces that closed the vessel become transmissive, and the void
/// around the vessel, the two plates and the ball annulus are added to the
/// root universe, closed by vacuum at the shield's outer radius and ends.
/// With [`Shield::Omitted`] the caller uses `geometry_with_ends` itself.
pub fn geometry_with_shield(t_salt_k: f64, t_graphite_k: f64, d: Dims, ends: Ends, first_shield_material: usize) -> Geometry {
    let mut g = geometry_with_ends(t_salt_k, t_graphite_k, d, ends);
    let t = BoundaryType::Transmissive;
    let v = BoundaryType::Vacuum;
    let mut vac: Vec<usize> = Vec::new();
    for (i, s) in g.surfaces.iter_mut().enumerate() {
        match s {
            SurfaceKind::ZCylinder(c) if c.bc == v => {
                c.bc = t;
                vac.push(i);
            }
            SurfaceKind::ZPlane(p) if p.bc == v => {
                p.bc = t;
                vac.push(i);
            }
            _ => {}
        }
    }
    assert_eq!(vac.len(), 3, "the vessel is closed by one cylinder and two planes");
    let r_vo = vac[0];
    let (zlb, zut) = match (&g.surfaces[vac[1]], &g.surfaces[vac[2]]) {
        (SurfaceKind::ZPlane(a), SurfaceKind::ZPlane(b)) if a.z0 < b.z0 => (vac[1], vac[2]),
        _ => (vac[2], vac[1]),
    };
    let z_mid = match (&g.surfaces[zlb], &g.surfaces[zut]) {
        (SurfaceKind::ZPlane(a), SurfaceKind::ZPlane(b)) => (a.z0 + b.z0) / 2.0,
        _ => unreachable!("planes"),
    };
    let mut add = |s: SurfaceKind| {
        g.surfaces.push(s);
        g.surfaces.len() - 1
    };
    let r1 = add(zcyl(0.0, 0.0, SHIELD_R_IN, t));
    let r2 = add(zcyl(0.0, 0.0, SHIELD_R_IN + SHIELD_PLATE, t));
    let r3 = add(zcyl(0.0, 0.0, SHIELD_R_OUT - SHIELD_PLATE, t));
    let r4 = add(zcyl(0.0, 0.0, SHIELD_R_OUT, v));
    let zs_lo = add(SurfaceKind::ZPlane(ZPlane { z0: z_mid - SHIELD_HEIGHT / 2.0, bc: v }));
    let zs_hi = add(SurfaceKind::ZPlane(ZPlane { z0: z_mid + SHIELD_HEIGHT / 2.0, bc: v }));
    let (lo, hi) = (hs(zs_lo, false), hs(zs_hi, true));
    let tk = SHIELD_TEMPERATURE_K;
    let (plate, balls) = (first_shield_material, first_shield_material + 1);
    let new_cells = vec![
        // Void beside, below and above the vessel, inside the shield.
        Cell::fill(80, all_of(&[hs(r_vo, false), hs(r1, true), hs(zlb, false), hs(zut, true)]), CellFill::Void, Position::ZERO),
        Cell::fill(81, all_of(&[hs(r1, true), lo.clone(), hs(zlb, true)]), CellFill::Void, Position::ZERO),
        Cell::fill(82, all_of(&[hs(r1, true), hs(zut, false), hi.clone()]), CellFill::Void, Position::ZERO),
        Cell::material(83, all_of(&[hs(r1, false), hs(r2, true), lo.clone(), hi.clone()]), plate, tk),
        Cell::material(84, all_of(&[hs(r2, false), hs(r3, true), lo.clone(), hi.clone()]), balls, tk),
        Cell::material(85, all_of(&[hs(r3, false), hs(r4, true), lo, hi]), plate, tk),
    ];
    let root = g.root_universe;
    for c in new_cells {
        g.cells.push(c);
        let idx = g.cells.len() - 1;
        g.universes[root].cell_indices.push(idx);
    }
    g
}

/// Fission source box: the graphite core.
pub fn source_box() -> SourceBox {
    SourceBox {
        lower: Position::new(-R_GRAPHITE, -R_GRAPHITE, 0.0),
        upper: Position::new(R_GRAPHITE, R_GRAPHITE, H_CORE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::plot::{PlotBasis, SlicePlot, SliceHit};

    fn census(g: &Geometry, origin: Position, basis: PlotBasis, width: [f64; 2], px: usize) -> (Vec<usize>, usize, usize) {
        let plot = SlicePlot {
            origin,
            basis,
            width,
            pixels: [px, px],
            level: None,
            show_overlaps: true,
            meshlines: None,
        };
        let ids = plot.id_map(g);
        let mut by_mat = vec![0usize; 4];
        let (mut void, mut overlap, mut outside) = (0, 0, 0);
        for h in &ids.hits {
            match *h {
                SliceHit::Found { material: Some(m), .. } => by_mat[m] += 1,
                SliceHit::Found { material: None, .. } => void += 1,
                SliceHit::Overlap => overlap += 1,
                SliceHit::NotFound => outside += 1,
            }
        }
        assert_eq!(overlap, 0, "overlapping cells");
        (by_mat, void, outside)
    }

    /// One stringer, sampled on the assembled geometry (an 800 × 800 pixel
    /// slice of the cell at (−10.16, −10.16) cm, clear of the openings), holds
    /// the analytic fraction 0.2228 to the sampling's resolution, and
    /// Table 5.4's 0.224 to 0.6 %.
    #[test]
    fn the_stringer_holds_the_tabulated_fuel_fraction() {
        let g = geometry();
        let (m, void, other) = census(&g, Position::new(-4.0 * INCH, -4.0 * INCH, 80.0), PlotBasis::Xy, [PITCH, PITCH], 800);
        assert_eq!((void, other), (0, 0));
        let total: usize = m.iter().sum();
        let f = m[SALT_IDX] as f64 / total as f64;
        println!("stringer fuel fraction {f:.5} (analytic {STRINGER_FUEL_FRACTION:.5}, TM-0728 Table 5.4: 0.224)");
        assert!((f - STRINGER_FUEL_FRACTION).abs() < 2e-3, "{f}");
        assert!((STRINGER_FUEL_FRACTION / 0.224 - 1.0).abs() < 0.006);
    }

    /// The whole core section at mid-height: the salt and graphite areas in
    /// the graphite cylinder give Table 5.4's fuel and graphite volumes
    /// (20 and 69 ft³) within 2 %, and no pixel is lost or overlapped.
    #[test]
    fn the_core_holds_the_tabulated_salt_and_graphite_volumes() {
        let g = geometry();
        let w = 2.0 * R_GRAPHITE;
        let px = 1400;
        let (m, void, outside) = census(&g, Position::new(0.0, 0.0, H_CORE / 2.0), PlotBasis::Xy, [w, w], px);
        // The window's corners lie beyond the vessel: outside the model.
        println!("pixels outside the model (window corners): {outside}");
        let a_px = (w / px as f64).powi(2);
        // The square window clips the gap/can/downcomer rings; inside the
        // graphite radius only the lattice, openings and thimble voids lie.
        let graphite = m[GRAPHITE_IDX] as f64 * a_px * H_CORE / FT3;
        let salt_all = m[SALT_IDX] as f64 * a_px * H_CORE / FT3;
        println!("mid-plane census: graphite {graphite:.2} ft3, salt (window incl. gap) {salt_all:.2} ft3, void px {void}");
        assert!((graphite / 69.0 - 1.0).abs() < 0.02, "graphite {graphite}");
    }

    /// The plenums hold Table 5.4's head volumes, and the openings sit where
    /// Fig. 5.7 puts them (2.828 in from the axis, text: 2-53/64 in).
    /// The refactor onto [`Dims`] builds the cold geometry unchanged: the
    /// FNV-1a hash of its `Debug` text was printed from the code before
    /// [`geometry_with_dims`] existed (2026-11-09).
    #[test]
    fn cold_dims_build_the_recorded_geometry() {
        let s = format!("{:?}", geometry_at_split(911.0, 950.0));
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        assert_eq!((h, s.len()), (0x364611a9909b0605, 14517));
        assert_eq!(format!("{:?}", geometry_with_dims(911.0, 950.0, Dims::COLD)), s);
    }

    /// Hot dimensions reproduce TM-0728 §5.3's statement that heatup moves
    /// the core can 3/16 in radially away from the graphite.
    #[test]
    fn hot_dims_open_the_can_gap_by_three_sixteenths() {
        let d = Dims::hot(1200.0, GRAPHITE_EXPANSION_PER_F, INOR8_EXPANSION_PER_F);
        let growth = (R_CAN_IN * d.inor8 - R_GRAPHITE * d.graphite) - (R_CAN_IN - R_GRAPHITE);
        assert!((growth / INCH - 3.0 / 16.0).abs() < 0.02, "{}", growth / INCH);
        let gh = geometry_with_dims(TEMPERATURE_K, TEMPERATURE_K, Dims::hot(1181.0, GRAPHITE_EXPANSION_PER_F, INOR8_EXPANSION_PER_F));
        assert_ne!(format!("{gh:?}"), format!("{:?}", geometry()));
        // Densities thin by the cube of each scale; cold leaves them alone.
        let hot = materials_with_dims(MoTreatment::Omitted, 1181.0, d);
        let g3 = d.graphite * d.graphite * d.graphite;
        assert!((hot[GRAPHITE_IDX].1[0].1 * g3 / GRAPHITE[0].1 - 1.0).abs() < 1e-12);
        assert_eq!(materials_with_dims(MoTreatment::Omitted, 1181.0, Dims::COLD), materials_at(MoTreatment::Omitted, 1181.0));
    }

    /// The TM-0730 ends: the default build is untouched, the variant adds
    /// the five mixes, and its salt height above the graphite is Table 3.1's.
    #[test]
    fn tm0730_ends_build_beside_the_default() {
        assert_eq!(
            format!("{:?}", geometry_with_ends(911.0, 950.0, Dims::COLD, Ends::VolumeEquivalent)),
            format!("{:?}", geometry_at_split(911.0, 950.0))
        );
        let m = materials_with_ends(MoTreatment::Natural, 1181.0, Dims::COLD);
        assert_eq!(m.len(), 9);
        // H is 63.3 % salt: its Li-7 is 0.633 of the salt's.
        let li7 = |v: &Vec<(&str, f64)>| v.iter().find(|e| e.0 == "Li7").map(|e| e.1).unwrap_or(0.0);
        assert!((li7(&m[END_MIX_IDX[0]].1) / li7(&m[SALT_IDX].1) - 0.633).abs() < 1e-12);
        let g = geometry_with_ends(TEMPERATURE_K, TEMPERATURE_K, Dims::COLD, Ends::Tm0730);
        let n_mix = g.cells.iter().filter(|c| matches!(c.fill, CellFill::Material(i) if i >= 4)).count();
        assert_eq!(n_mix, 5);
    }

    /// The thermal shield: the vessel inside is the default build cell for
    /// cell, an x-z slice through the whole shield has no overlap and no
    /// pixel outside the model inside the shield's box, and the 16 in is half
    /// iron by volume as TM-0728 states.
    #[test]
    fn tm0728_shield_wraps_the_default_vessel() {
        let base = geometry_with_ends(TEMPERATURE_K, TEMPERATURE_K, Dims::COLD, Ends::VolumeEquivalent);
        let mats = materials_with_ends(MoTreatment::Natural, 1181.0, Dims::COLD);
        let g = geometry_with_shield(TEMPERATURE_K, TEMPERATURE_K, Dims::COLD, Ends::VolumeEquivalent, mats.len());
        assert_eq!(format!("{:?}", &g.cells[..base.cells.len()]), format!("{:?}", base.cells));
        assert_eq!(g.cells.len(), base.cells.len() + 6);
        assert_eq!(g.surfaces.len(), base.surfaces.len() + 6);
        let plot = SlicePlot {
            origin: Position::new(0.0, OPENING_OFFSET, H_CORE / 2.0),
            basis: PlotBasis::Xz,
            width: [2.0 * SHIELD_R_OUT - 1.0, SHIELD_HEIGHT - 1.0],
            pixels: [600, 600],
            level: None,
            show_overlaps: true,
            meshlines: None,
        };
        let ids = plot.id_map(&g);
        let (mut shield, mut overlap, mut outside) = (0usize, 0usize, 0usize);
        for h in &ids.hits {
            match *h {
                SliceHit::Found { material: Some(m), .. } if m >= mats.len() => shield += 1,
                SliceHit::Overlap => overlap += 1,
                SliceHit::NotFound => outside += 1,
                _ => {}
            }
        }
        assert_eq!((overlap, outside), (0, 0));
        assert!(shield > 0);
        // Iron over the 16 in: two plates at 71.5 wt % Fe of 8.00 g/cm³, and
        // the annulus's balls; as volume of iron-bearing steel, 0.5.
        let fe = |v: &Vec<(&str, f64)>| v.iter().filter(|e| e.0.starts_with("Fe")).map(|e| e.1).sum::<f64>();
        let n_fe_ball = 7.87 * 0.602214076 / 55.845;
        let f_ball = fe(&shield_ball_zone()) / n_fe_ball;
        let total = SHIELD_R_OUT - SHIELD_R_IN;
        let steel = (2.0 * SHIELD_PLATE + f_ball * (total - 2.0 * SHIELD_PLATE)) / total;
        assert!((steel - 0.5).abs() < 1e-12, "{steel}");
        assert!((f_ball - 6.0 / 14.0).abs() < 1e-12);
        // The fraction-parameterised zone reproduces the stated one at 6/14,
        // and the packed build changes only the annulus's fraction.
        let at = shield_ball_zone_with_fraction(f_ball);
        for (a, b) in at.iter().zip(shield_ball_zone().iter()) {
            assert_eq!(a.0, b.0);
            assert!((a.1 - b.1).abs() <= 1e-12 * b.1, "{} {} {}", a.0, a.1, b.1);
        }
        let packed = shield_materials_packed();
        assert_eq!(packed[0].1, ss304());
        assert!((fe(&packed[1].1) / n_fe_ball - SHIELD_BALL_FRACTION_PACKED).abs() < 1e-12);
        // The drained ablation keeps the plates and the balls, and only them.
        let drained = shield_materials_drained();
        assert_eq!(drained.len(), shield_materials().len());
        assert_eq!(drained[0].1, ss304());
        assert_eq!(fe(&drained[1].1), fe(&shield_ball_zone()));
        assert!(drained[1].1.iter().all(|e| e.0.starts_with("Fe")));
        let names: Vec<&str> = shield_materials().iter().flat_map(|m| m.1.iter().map(|e| e.0)).collect();
        for n in names {
            assert!(SHIELD_NUCLIDES.iter().any(|(k, _)| *k == n), "{n} has no tape");
        }
    }

    #[test]
    fn plenums_and_openings_match_their_sources() {
        let pi = core::f64::consts::PI;
        let v_lp = pi * R_VESSEL_IN * R_VESSEL_IN * lower_plenum_height();
        let v_up = pi * R_VESSEL_IN * R_VESSEL_IN * upper_plenum_height();
        assert!((v_lp / FT3 - 10.0).abs() < 1e-9 && (v_up / FT3 - 10.5).abs() < 1e-9);
        let d = (OPENINGS[0].0.powi(2) + OPENINGS[0].1.powi(2)).sqrt() / INCH;
        assert!((d - (2.0 + 53.0 / 64.0)).abs() < 0.01, "{d}");
        let (fi, fg, fs) = basket_fractions();
        assert!(fi > 0.0 && fg > 0.0 && fs > 0.0 && (fi + fg + fs - 1.0).abs() < 1e-12);
        assert_eq!(materials(MoTreatment::Omitted)[INOR8_IDX].1.len(), 13);
        assert_eq!(materials(MoTreatment::Mo95Equivalent)[INOR8_IDX].1.len(), 14);
        // The reference state is unchanged by the temperature path, and
        // 100 °F hotter thins the salt by 1.18 %.
        assert_eq!(materials(MoTreatment::Omitted), materials_at(MoTreatment::Omitted, REFERENCE_TEMPERATURE_F));
        let hot = materials_at(MoTreatment::Omitted, REFERENCE_TEMPERATURE_F + 100.0);
        assert!((hot[SALT_IDX].1[0].1 / SALT[0].1 - (1.0 - 0.0118)).abs() < 1e-12);
        assert!((kelvin_from_f(REFERENCE_TEMPERATURE_F) - TEMPERATURE_K).abs() < 1e-12);
        // The split geometry puts only the stringer graphite at its own
        // temperature.
        let g = geometry_at_split(900.0, 950.0);
        let hot: Vec<i32> = g.cells.iter().filter(|c| c.temperature == 950.0).map(|c| c.id).collect();
        assert_eq!(hot, vec![102]);
        assert_eq!(basket_mix(MoTreatment::Omitted), materials(MoTreatment::Omitted)[BASKET_IDX].1);
        // Every tape the model names exists (the first run died on a wrong
        // Ni path, 2026-11-09).
        let natural: f64 = MO_NATURAL_ABUNDANCE.iter().map(|x| x.1).sum();
        assert!((natural - 1.0).abs() < 1e-12, "{natural}");
        assert_eq!(materials(MoTreatment::Natural)[INOR8_IDX].1.len(), 13 + 7);
        let all_mo: Vec<_> = mo_tapes(MoTreatment::Natural).into_iter().chain(mo_tapes(MoTreatment::Mo95Equivalent)).collect();
        for (n, file) in NUCLIDES.iter().chain(all_mo.iter()).chain(SHIELD_NUCLIDES.iter()) {
            assert!(njoy_outram_park_fork::reference_data::reference_endf(file).is_some(), "{n}: {file}");
        }
    }
}

