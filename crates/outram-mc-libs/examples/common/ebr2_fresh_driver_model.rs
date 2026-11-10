//! # A fresh EBR-II MARK-II driver subassembly, for k-infinity (gh #1329)
//!
//! Shared by `examples/ebr2_fresh_driver_kinf.rs` (the run and the pictures)
//! and `tests/model_ebr2_fresh_driver.rs` (geometry, composition and the
//! ARC-228 Table 3.3 volume fractions). Epic #1312, brief #1317 item 7,
//! builder #1327.
//!
//! > Research, education and V&V only. Not for reactor operation,
//! > licensing or any safety decision (`RESPONSIBLE_USE.md`). AI draft,
//! > unreviewed.
//!
//! **Every number below is an AI transcription, not yet reviewed by a
//! human.** Each carries its document, table and printed page; check the
//! page before relying on it. The EBR-II reports are held in the
//! maintainer's private corpus and cited under the data-policy exception for
//! published EBR-II benchmark numbers (`reference-data/ebr2/README.md`).
//!
//! ## The model
//!
//! One fresh MARK-II driver subassembly ([`HexSubassembly`]): 91 sodium-bonded
//! U-5Fs pins (6 rings) in 316SS cladding, a 316SS helical wire wrap smeared
//! into the sodium coolant, a 316SS hex duct, and the sodium between ducts
//! (a hexagon at the subassembly pitch, [`lattice_cell`]). Every face is
//! reflective, so it is an infinite lattice of identical subassemblies,
//! axially infinite: a k-infinity. Isothermal **20 °C (293.15 K)**: ARC-228
//! Tables 3.1.1 (p.17), 3.1.2 (p.18) and 3.3 (pp.22-24) give their
//! dimensions "at 20 C", so the geometry, the densities and the nuclear data
//! are all taken at that one temperature. At 20 °C the sodium is solid.
//!
//! | input | value | source |
//! |---|---|---|
//! | pins per subassembly | 91 | ARC-228 Table 3.1.1, p.17 |
//! | fuel-slug diameter | 3.3655 mm | ARC-228 Table 3.1.1, p.17 |
//! | cladding OD | 4.4196 mm | ARC-228 Table 3.1.1, p.17 |
//! | cladding wall | **0.305 mm** | ARC-228 Table 3.1.1, p.17 (see below) |
//! | spacer-wire diameter | 1.24 mm | ARC-228 Table 3.1.1, p.17 |
//! | spacer-wire pitch (lead) | 0.1524 m | ARC-226 Table 4.1.1 footnote, p.49 |
//! | pin pitch | 5.6596 mm, **assumed** `D + s` | not printed; see below |
//! | duct flat-to-flat inside / outside | 56.134 / 58.166 mm | ARC-228 Table 3.1.2, p.18 |
//! | subassembly pitch | 58.929 mm | ARC-228 Table 3.1.2, p.18 |
//! | fuel-slug length (the drawn height only) | 0.3429 m | ARC-228 Table 3.1.1, p.17 |
//! | U-5Fs composition | U 95.0, Zr 0.10, Mo 2.46, Ru 1.96, Rh 0.28, Pd 0.19, Nb 0.01 wt% | ARC-228 Table 3.2.2, p.21 |
//! | enrichment | 67 wt% U-235 | ARC-228 Table 3.1.1, p.17 |
//! | U-5Fs density | 18.2 g/cm³ at 300 K | ARC-226 §4.2.1, p.100 |
//! | 316SS | PNNL-15870 Rev. 1, material 300 | see [`steel_316_weight_percent`] |
//! | sodium | 0.971 g/cm³ | PNNL-15870 Rev. 1, material 287 |
//!
//! **Cladding wall, 0.305 vs 0.315 mm.** ARC-226 Table 4.1.1 (p.49) prints
//! 0.315 mm; ARC-228 Table 3.1.1 prints 0.305. ARC-228 is the neutronics
//! specification, and its own Table 3.3 (p.23) gives cladding ID 0.3810 cm
//! and OD 0.4420 cm, a 0.305 mm wall. So 0.305 is the value consistent
//! within the specification this model follows.
//!
//! **Pin pitch.** Neither ARC-228 nor ARC-226 prints the pin pitch. It is
//! taken as `D + s` (each wire touching its neighbour's cladding, the
//! usual wire-wrap design): 4.4196 + 1.24 = 5.6596 mm, P/D = 1.2806. That
//! agrees with the "P/D=1.28" IAEA-TECDOC-1819 uses for the EBR-II driver
//! (TABLE 34 footnote, p.249). The volume fractions do not depend on it.
//!
//! **Steel.** Neither EBR-II specification gives a steel composition (#1313,
//! A6 GAP). 316SS is PNNL-15870 Rev. 1 (R. J. McConn Jr. et al., *Compendium
//! of Material Composition Data for Radiation Transport Modeling*, Pacific
//! Northwest National Laboratory, 2011), material 300 "Steel, Stainless
//! 316", 8.00 g/cm³. **Read from the machine-readable copy of the compendium
//! that PyNE distributes** (`pyne/dbgen/materials_compendium.csv`,
//! github.com/pyne/pyne, branch `develop`, fetched 2026-11-10), not from the
//! PDF, which could not be reached from this session: no page is cited.
//! Sodium is material 287 of the same copy, 0.971 g/cm³.
//!
//! **Not modelled.** U-234 and U-236 (no fresh-fuel isotopics are given;
//! ARC-228 Table 3.2.1 is a depleted subassembly); the plenum, the
//! restrainer, the end fittings and the axial reflectors (axially infinite);
//! thermal expansion and the sodium phase (everything at 20 °C).
//!
//! ## Nuclear-data gaps and their treatments
//!
//! ENDF/B-VIII.0 tapes come from two folders already in `reference-data/`:
//! `reference-data/endf/` and `reference-data/ace/endf/endf-b-viii.0/` (the
//! ACE submodule; Ni, Zr and all seven Mo isotopes, added 2026-10-01 for
//! HTR-10 and 2026-11-09 for MSRE). **No tape exists anywhere in
//! `reference-data/` for Ru, Rh, Pd or Nb.** [`GapTreatment`] names each
//! treatment; [`GapTreatment::arms`] lists the ablation arms.

use std::path::{Path, PathBuf};

use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::{
    AssembledModel, AtomicMasses, HexPrism, HexSubassembly, Isotope, PinDesign, Shape,
    SubassemblyMaterials, WireWrap, AVOGADRO_PER_MOL,
};
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::position::{Direction, Position};
use uom::si::f64::{Length, MassDensity, Ratio, ThermodynamicTemperature};
use uom::si::length::millimeter;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::ratio::{percent, ratio};
use uom::si::thermodynamic_temperature::kelvin;

/// The model temperature \[K\]: 20 °C, the temperature ARC-228 Tables
/// 3.1.1, 3.1.2 and 3.3 state their dimensions at.
pub const TEMPERATURE_K: f64 = 293.15;
/// Subassembly pitch \[mm\], ARC-228 Table 3.1.2, p.18.
pub const SA_PITCH_MM: f64 = 58.929;
/// Duct flat-to-flat inside / outside \[mm\], ARC-228 Table 3.1.2, p.18.
pub const DUCT_INSIDE_MM: f64 = 56.134;
/// See [`DUCT_INSIDE_MM`].
pub const DUCT_OUTSIDE_MM: f64 = 58.166;
/// Fuel-slug diameter \[mm\], ARC-228 Table 3.1.1, p.17.
pub const SLUG_DIAMETER_MM: f64 = 3.3655;
/// Cladding OD \[mm\], ARC-228 Table 3.1.1, p.17.
pub const CLAD_OD_MM: f64 = 4.4196;
/// Cladding wall \[mm\], ARC-228 Table 3.1.1, p.17 (module docs: why not
/// ARC-226's 0.315).
pub const CLAD_WALL_MM: f64 = 0.305;
/// Spacer-wire diameter \[mm\], ARC-228 Table 3.1.1, p.17.
pub const WIRE_DIAMETER_MM: f64 = 1.24;
/// Spacer-wire pitch (helix lead) \[mm\], ARC-226 Table 4.1.1 footnote,
/// p.49 ("0.1524 m").
pub const WIRE_LEAD_MM: f64 = 152.4;
/// Fuel-slug length \[mm\], ARC-228 Table 3.1.1, p.17: the height drawn;
/// with reflective ends it does not enter k-infinity.
pub const SLUG_LENGTH_MM: f64 = 342.9;
/// Rings of pins: 6 rings is `3*6*5 + 1 = 91` pins (ARC-228 Table 3.1.1).
pub const RINGS: usize = 6;
/// U-5Fs density \[g/cm³\], ARC-226 §4.2.1, p.100 ("18.2 x 10^3 kg/m3 at
/// 300 K").
pub const FUEL_DENSITY_G_CC: f64 = 18.2;
/// U-235 weight fraction of the uranium, ARC-228 Table 3.1.1, p.17.
pub const ENRICHMENT_WT: f64 = 0.67;
/// 316SS density \[g/cm³\], PNNL-15870 Rev. 1 material 300.
pub const STEEL_DENSITY_G_CC: f64 = 8.00;
/// Sodium density \[g/cm³\], PNNL-15870 Rev. 1 material 287.
pub const SODIUM_DENSITY_G_CC: f64 = 0.971;

/// **ARC-228 Table 3.3, p.23**: the driver's volume fractions, fuel alloy /
/// 316SS / sodium, as fractions (26.92 / 22.29 / 50.79 %). They are over the
/// lattice cell at the subassembly pitch (checked: they reproduce only with
/// the sodium between ducts counted).
pub const TABLE_3_3_FRACTIONS: [f64; 3] = [0.2692, 0.2229, 0.5079];

/// The pin pitch \[mm\]: `D + s`, assumed (module docs).
pub fn pin_pitch_mm() -> f64 {
    CLAD_OD_MM + WIRE_DIAMETER_MM
}

/// **The driver subassembly's geometry**, from the cited dimensions.
pub fn driver_subassembly() -> HexSubassembly {
    let mm = Length::new::<millimeter>;
    let pin = PinDesign {
        fuel_radius: mm(0.5 * SLUG_DIAMETER_MM),
        clad_inner_radius: mm(0.5 * CLAD_OD_MM - CLAD_WALL_MM),
        clad_outer_radius: mm(0.5 * CLAD_OD_MM),
    };
    HexSubassembly::new(
        mm(DUCT_INSIDE_MM),
        mm(0.5 * (DUCT_OUTSIDE_MM - DUCT_INSIDE_MM)),
        mm(SLUG_LENGTH_MM),
        mm(pin_pitch_mm()),
        RINGS,
        pin,
    )
    .wire_wrap(WireWrap {
        diameter: mm(WIRE_DIAMETER_MM),
        lead: mm(WIRE_LEAD_MM),
    })
}

/// **The lattice cell**: the subassembly inside a sodium hexagon at the
/// subassembly pitch, reflective on every face (an infinite lattice).
pub fn lattice_cell(subassembly: Shape, gap_sodium: Material) -> Shape {
    let mm = Length::new::<millimeter>;
    HexPrism::flat_to_flat(mm(SA_PITCH_MM), mm(SLUG_LENGTH_MM))
        .filled_with(gap_sodium)
        .containing(subassembly)
        .boundary(Boundary::Reflective)
}

/// **The driver's volume fractions over the lattice cell**, analytic:
/// fuel / steel (clad + wire + duct) / sodium (bond + coolant + the gap
/// between ducts), from the builder's own fractions (which are over the
/// duct's outside) rescaled to the cell at the subassembly pitch.
pub fn analytic_cell_fractions(sa: &HexSubassembly) -> [f64; 3] {
    let v = sa.volume_fractions();
    let r = DUCT_OUTSIDE_MM / SA_PITCH_MM;
    let duct_share = r * r;
    let g = |r: Ratio| r.get::<ratio>() * duct_share;
    let fuel = g(v.fuel);
    let steel = g(v.clad) + g(v.wire) + g(v.duct);
    [fuel, steel, 1.0 - fuel - steel]
}

/// **316SS, PNNL-15870 Rev. 1 material 300**, weight percent (module docs).
pub fn steel_316_weight_percent() -> [(Element, f64); 9] {
    [
        (Element::Carbon, 0.041),
        (Element::Silicon, 0.507),
        (Element::Phosphorus, 0.023),
        (Element::Sulfur, 0.015),
        (Element::Chromium, 17.0),
        (Element::Manganese, 1.014),
        (Element::Iron, 66.9),
        (Element::Nickel, 12.0),
        (Element::Molybdenum, 2.5),
    ]
}

/// **U-5Fs, ARC-228 Table 3.2.2 (p.21)**, weight percent of the alloy, the
/// uranium first. Ru, Rh, Pd and Nb have no tape in `reference-data/`.
pub fn u5fs_weight_percent() -> [(Element, f64); 7] {
    [
        (Element::Uranium, 95.0),
        (Element::Zirconium, 0.10),
        (Element::Molybdenum, 2.46),
        (Element::Ruthenium, 1.96),
        (Element::Rhodium, 0.28),
        (Element::Palladium, 0.19),
        (Element::Niobium, 0.01),
    ]
}

/// **Standard atomic weights** \[g/mol\] of the four fissium elements with no
/// tape (so the library cannot give their masses): IUPAC/CIAAW 2021
/// (T. Prohaska et al., *Pure Appl. Chem.* 94 (2022) 573). Used only to
/// count their atoms for the [`FissionProductProxy::NaturalMolybdenum`]
/// substitution.
pub fn standard_atomic_weight(e: Element) -> Option<f64> {
    match e {
        Element::Ruthenium => Some(101.07),
        Element::Rhodium => Some(102.905_49),
        Element::Palladium => Some(106.42),
        Element::Niobium => Some(92.906_37),
        _ => None,
    }
}

/// Keep an element, or leave its atoms out (no renormalisation: the other
/// elements keep their physical densities).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// Its natural isotopes, from the tapes.
    Natural,
    /// Left out.
    Omit,
}

/// How molybdenum (steel and fuel) is represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MolybdenumTreatment {
    /// All seven natural isotopes.
    Natural,
    /// **Substitution**: every Mo atom is Mo-95 (atom for atom).
    AllMo95,
    /// Left out.
    Omit,
}

/// How the fissium elements with no tape (Ru, Rh, Pd, Nb) are represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FissionProductProxy {
    /// Left out (the only choice the data allow without a substitution).
    Omit,
    /// **Neighbour substitution**: their atoms, counted with IUPAC standard
    /// atomic weights, become natural molybdenum atoms (Z = 42, next to
    /// Nb 41 and Ru 44). Mo captures less than Ru, Rh and Pd, so this is a
    /// lower bound on what they absorb, not an estimate of it.
    NaturalMolybdenum,
}

/// **A nuclear-data gap treatment**: one choice per gap element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GapTreatment {
    /// Short name of the arm (`A` ... `G`).
    pub arm: &'static str,
    /// What it is, in words.
    pub description: &'static str,
    /// Nickel (316SS, 12 wt%).
    pub nickel: Keep,
    /// Molybdenum (316SS 2.5 wt%, U-5Fs 2.46 wt%).
    pub molybdenum: MolybdenumTreatment,
    /// Zirconium (U-5Fs, 0.10 wt%).
    pub zirconium: Keep,
    /// Ru, Rh, Pd, Nb (U-5Fs, 2.44 wt% together).
    pub ru_rh_pd_nb: FissionProductProxy,
}

impl GapTreatment {
    /// **Arm A, the reference arm**: every element with a tape in
    /// `reference-data/` at its natural composition; Ru, Rh, Pd, Nb omitted.
    pub fn on_hand() -> Self {
        Self {
            arm: "A",
            description: "all on-hand data (Ni, Mo, Zr natural); Ru, Rh, Pd, Nb omitted",
            nickel: Keep::Natural,
            molybdenum: MolybdenumTreatment::Natural,
            zirconium: Keep::Natural,
            ru_rh_pd_nb: FissionProductProxy::Omit,
        }
    }

    /// **The ablation arms**, A first. B and C are the brief's treatments
    /// (a) and (b) on `reference-data/endf/` alone; D-G change one gap of A
    /// at a time.
    pub fn arms() -> Vec<Self> {
        let a = Self::on_hand();
        vec![
            a,
            Self {
                arm: "B",
                description: "brief (a), reference-data/endf only: Ni, Mo, Zr, Ru, Rh, Pd, Nb omitted",
                nickel: Keep::Omit,
                molybdenum: MolybdenumTreatment::Omit,
                zirconium: Keep::Omit,
                ..a
            },
            Self {
                arm: "C",
                description: "brief (b), reference-data/endf only: Mo-95 for all Mo; Ni, Zr, Ru, Rh, Pd, Nb omitted",
                nickel: Keep::Omit,
                molybdenum: MolybdenumTreatment::AllMo95,
                zirconium: Keep::Omit,
                ..a
            },
            Self {
                arm: "D",
                description: "A with Ni omitted",
                nickel: Keep::Omit,
                ..a
            },
            Self {
                arm: "E",
                description: "A with Mo-95 substituted for all Mo",
                molybdenum: MolybdenumTreatment::AllMo95,
                ..a
            },
            Self {
                arm: "F",
                description: "A with Mo omitted",
                molybdenum: MolybdenumTreatment::Omit,
                ..a
            },
            Self {
                arm: "G",
                description: "A with Ru, Rh, Pd, Nb replaced atom for atom by natural Mo",
                ru_rh_pd_nb: FissionProductProxy::NaturalMolybdenum,
                ..a
            },
        ]
    }
}

/// **The tapes the model reads**, one per nuclide, chosen by name:
/// ENDF/B-VIII.0 from `reference-data/endf/` (`endf_dir`) and, for Ni, Zr
/// and Mo, from `reference-data/ace/endf/endf-b-viii.0/` (`ace_endf_dir`).
/// The release Mo-95 tape (`ace_endf_dir`) is used, not the beta one in
/// `endf_dir`, so a substitution arm differs from arm A only in the
/// substitution.
pub fn tape_paths(endf_dir: &Path, ace_endf_dir: &Path) -> Vec<PathBuf> {
    let endf = [
        "n-092_U_235-ENDF8.0.endf",
        "n-092_U_238.endf",
        "n-011_Na_023-ENDF8.0.endf",
        "n-026_Fe_054-ENDF8.0.endf",
        "n-026_Fe_056-ENDF8.0.endf",
        "n-026_Fe_057-ENDF8.0.endf",
        "n-026_Fe_058-ENDF8.0.endf",
        "n-024_Cr_050-ENDF8.0.endf",
        "n-024_Cr_052-ENDF8.0.endf",
        "n-024_Cr_053-ENDF8.0.endf",
        "n-024_Cr_054-ENDF8.0.endf",
        "n-025_Mn_055-ENDF8.0.endf",
        "n-014_Si_028-ENDF8.0.endf",
        "n-014_Si_029-ENDF8.0.endf",
        "n-014_Si_030-ENDF8.0.endf",
        "n-006_C_012-ENDF8.0.endf",
        "n-006_C_013-ENDF8.0.endf",
        "n-015_P_031-ENDF8.0.endf",
        "n-016_S_032-ENDF8.0.endf",
        "n-016_S_033-ENDF8.0.endf",
        "n-016_S_034-ENDF8.0.endf",
        "n-016_S_036-ENDF8.0.endf",
    ];
    let ace = [
        "n-028_Ni_058-ENDF8.0.endf",
        "n-028_Ni_060-ENDF8.0.endf",
        "n-028_Ni_061-ENDF8.0.endf",
        "n-028_Ni_062-ENDF8.0.endf",
        "n-028_Ni_064-ENDF8.0.endf",
        "n-040_Zr_090-ENDF8.0.endf",
        "n-040_Zr_091-ENDF8.0.endf",
        "n-040_Zr_092-ENDF8.0.endf",
        "n-040_Zr_094-ENDF8.0.endf",
        "n-040_Zr_096-ENDF8.0.endf",
        "n-042_Mo_092-ENDF8.0.endf",
        "n-042_Mo_094-ENDF8.0.endf",
        "n-042_Mo_095-ENDF8.0.endf",
        "n-042_Mo_096-ENDF8.0.endf",
        "n-042_Mo_097-ENDF8.0.endf",
        "n-042_Mo_098-ENDF8.0.endf",
        "n-042_Mo_100-ENDF8.0.endf",
    ];
    endf.iter()
        .map(|f| endf_dir.join(f))
        .chain(ace.iter().map(|f| ace_endf_dir.join(f)))
        .collect()
}

/// **The nuclear-data library**: exactly the tapes of [`tape_paths`], read
/// into memory (one tape per nuclide, so no tape choice is left to the
/// library's own rules).
///
/// # Errors
/// A tape cannot be read, or none is an incident-neutron tape.
pub fn library(endf_dir: &Path, ace_endf_dir: &Path) -> Result<NuclearData, String> {
    let mut tapes = Vec::new();
    for p in tape_paths(endf_dir, ace_endf_dir) {
        let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        tapes.push((name, bytes));
    }
    NuclearData::from_endf_bytes("EBR-II fresh driver tapes (ENDF/B-VIII.0)", tapes)
        .map_err(|e| e.to_string())
}

/// `(isotope, atoms/b-cm)` of `species` present at partial density
/// `g_cc` (its mass per unit volume of the mixture), with the library's
/// masses (the model's own resolver).
fn partial(
    masses: &AtomicMasses,
    species: impl Into<Species>,
    g_cc: f64,
) -> Result<Vec<(Isotope, f64)>, String> {
    let m = Material::from_fractions(Basis::Weight, [(species.into(), Ratio::new::<percent>(100.0))])
        .density(MassDensity::new::<gram_per_cubic_centimeter>(g_cc));
    let r = m.resolve(masses).map_err(|e| e.to_string())?;
    Ok(r
        .nuclides
        .iter()
        .map(|(i, d)| (*i, to_atoms_per_barn_cm(*d)))
        .collect())
}

/// `element`'s partial density `g_cc` under `t`'s treatment of it: natural,
/// omitted, or (molybdenum) all Mo-95 atom for atom. Elements that are not
/// a gap are always natural.
fn element_atoms(
    masses: &AtomicMasses,
    element: Element,
    g_cc: f64,
    t: &GapTreatment,
) -> Result<Vec<(Isotope, f64)>, String> {
    let keep = match element {
        Element::Nickel => t.nickel,
        Element::Zirconium => t.zirconium,
        _ => Keep::Natural,
    };
    if keep == Keep::Omit {
        return Ok(Vec::new());
    }
    if element != Element::Molybdenum {
        return partial(masses, element.natural(), g_cc);
    }
    let natural = partial(masses, element.natural(), g_cc)?;
    Ok(match t.molybdenum {
        MolybdenumTreatment::Natural => natural,
        MolybdenumTreatment::Omit => Vec::new(),
        MolybdenumTreatment::AllMo95 => {
            let total: f64 = natural.iter().map(|(_, n)| n).sum();
            vec![(mo95()?, total)]
        }
    })
}

/// Mo-95.
fn mo95() -> Result<Isotope, String> {
    Isotope::new(42, 95).map_err(|e| e.to_string())
}

/// Sum equal isotopes, keeping first-seen order.
fn merge(parts: Vec<(Isotope, f64)>) -> Vec<(Isotope, f64)> {
    let mut out: Vec<(Isotope, f64)> = Vec::new();
    for (i, n) in parts {
        match out.iter_mut().find(|(j, _)| *j == i) {
            Some(e) => e.1 += n,
            None => out.push((i, n)),
        }
    }
    out
}

/// **316SS atom densities** \[atoms/b-cm\] under treatment `t`: each element
/// at its PNNL-15870 weight fraction times 8.00 g/cm³, gaps treated by `t`,
/// nothing renormalised.
///
/// # Errors
/// A nuclide the library does not hold.
pub fn steel_316_atoms(
    masses: &AtomicMasses,
    t: &GapTreatment,
) -> Result<Vec<(Isotope, f64)>, String> {
    let mut all = Vec::new();
    for (e, wt) in steel_316_weight_percent() {
        all.extend(element_atoms(masses, e, STEEL_DENSITY_G_CC * wt / 100.0, t)?);
    }
    Ok(merge(all))
}

/// **U-5Fs atom densities** \[atoms/b-cm\] under treatment `t`: U-235 and
/// U-238 at 67/33 wt% of the uranium, each fissium element at its ARC-228
/// weight fraction times 18.2 g/cm³, gaps treated by `t`, nothing
/// renormalised.
///
/// # Errors
/// A nuclide the library does not hold.
pub fn u5fs_atoms(masses: &AtomicMasses, t: &GapTreatment) -> Result<Vec<(Isotope, f64)>, String> {
    let rho = FUEL_DENSITY_G_CC;
    let mut all = Vec::new();
    let mut proxy_mo_atoms = 0.0;
    for (e, wt) in u5fs_weight_percent() {
        let g_cc = rho * wt / 100.0;
        if e == Element::Uranium {
            all.extend(partial(masses, U235, g_cc * ENRICHMENT_WT)?);
            all.extend(partial(masses, U238, g_cc * (1.0 - ENRICHMENT_WT))?);
        } else if let Some(a) = standard_atomic_weight(e) {
            if t.ru_rh_pd_nb == FissionProductProxy::NaturalMolybdenum {
                proxy_mo_atoms += g_cc * AVOGADRO_PER_MOL / a * 1e-24;
            }
        } else {
            all.extend(element_atoms(masses, e, g_cc, t)?);
        }
    }
    if proxy_mo_atoms > 0.0 {
        for (i, x) in Element::Molybdenum.natural_abundance() {
            all.push((i, x * proxy_mo_atoms));
        }
    }
    Ok(merge(all))
}

/// **Sodium** at 0.971 g/cm³, as atom densities, named `name`.
///
/// # Errors
/// The library has no Na-23.
pub fn sodium(masses: &AtomicMasses, name: &str) -> Result<Material, String> {
    Ok(atoms_material(partial(masses, Na23, SODIUM_DENSITY_G_CC)?, name))
}

/// A material from `(isotope, atoms/b-cm)`.
pub fn atoms_material(atoms: Vec<(Isotope, f64)>, name: &str) -> Material {
    Material::from_atom_densities(atoms.into_iter().map(|(i, n)| (i, atoms_per_barn_cm(n))))
        .named(name)
}

/// The material names the model uses (the sampled-fraction test reads
/// them back).
pub mod names {
    /// The fuel slugs.
    pub const FUEL: &str = "U-5Fs";
    /// The bond annuli.
    pub const BOND: &str = "Na bond";
    /// The cladding.
    pub const CLAD: &str = "316SS clad";
    /// The duct.
    pub const DUCT: &str = "316SS duct";
    /// The coolant before the wire is smeared in.
    pub const COOLANT: &str = "Na coolant";
    /// The wire.
    pub const WIRE: &str = "316SS wire";
    /// The coolant with the wire smeared in (the builder's name).
    pub const SMEARED: &str = "Na coolant + 316SS wire (smeared)";
    /// The sodium between ducts.
    pub const GAP: &str = "Na between ducts";
}

/// **The whole model under treatment `t`**: the lattice cell of
/// [`lattice_cell`] at [`TEMPERATURE_K`], on `data`.
///
/// # Errors
/// A nuclide the library does not hold, or the builder's checks.
pub fn build_model(data: NuclearData, t: &GapTreatment) -> Result<Model, String> {
    let masses = data.atomic_masses();
    let steel = steel_316_atoms(&masses, t)?;
    let materials = SubassemblyMaterials {
        fuel: atoms_material(u5fs_atoms(&masses, t)?, names::FUEL),
        bond: sodium(&masses, names::BOND)?,
        clad: atoms_material(steel.clone(), names::CLAD),
        coolant: sodium(&masses, names::COOLANT)?,
        duct: atoms_material(steel.clone(), names::DUCT),
        wire: Some(atoms_material(steel, names::WIRE)),
    };
    let sa = driver_subassembly()
        .build(materials)
        .map_err(|e| e.to_string())?;
    let mut model = Model::new(data);
    model.add(lattice_cell(sa, sodium(&masses, names::GAP)?));
    model.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(TEMPERATURE_K),
    ));
    Ok(model)
}

/// **Volume fractions of the ASSEMBLED geometry**, fuel / steel / sodium
/// over the lattice cell: cell lookup on an `n x n` grid at mid-height,
/// material by the cell found (each point offset within its grid square by
/// a row-dependent irrational fraction, so the grid does not alias on the
/// flats); the smeared coolant split by the builder's
/// wire share. Also the number of points inside the cell and the number
/// that located no material. Points within 1e-9 cm of the outer faces are
/// left out (lookup on a face is ambiguous).
pub fn sampled_cell_fractions(a: &AssembledModel, sa: &HexSubassembly, n: usize) -> ([f64; 3], usize, usize) {
    let f = SA_PITCH_MM / 10.0;
    let (hx, hy) = (f / 3.0_f64.sqrt(), 0.5 * f);
    let s = 0.5 * 3.0_f64.sqrt();
    let normals = [[0.0, 1.0], [s, 0.5], [-s, 0.5], [0.0, -1.0], [-s, -0.5], [s, -0.5]];
    let wfc = sa.wire_fraction_of_coolant();
    let mut sums = [0.0; 3];
    let (mut inside, mut lost) = (0usize, 0usize);
    for i in 0..n {
        for j in 0..n {
            // Offsets that shift row by row (golden-ratio and sqrt(2) - 1
            // sequences): a grid aligned with the duct's flats aliases on
            // them (one row of points in or out of a 0.1 cm wall).
            let ox = (j as f64 * 0.618_033_988_749_895).fract();
            let oy = (i as f64 * 0.414_213_562_373_095).fract();
            let x = -hx + (i as f64 + ox) * 2.0 * hx / n as f64;
            let y = -hy + (j as f64 + oy) * 2.0 * hy / n as f64;
            if !normals.iter().all(|m| m[0] * x + m[1] * y < 0.5 * f - 1e-9) {
                continue;
            }
            inside += 1;
            let found = a
                .geometry
                .locate(Position::new(x, y, 0.0), Direction::new(1.0, 0.0, 0.0), SurfaceToken::NONE)
                .and_then(|p| p.material);
            let Some(k) = found else {
                lost += 1;
                continue;
            };
            match a.resolved[k].name.as_str() {
                names::FUEL => sums[0] += 1.0,
                names::CLAD | names::DUCT => sums[1] += 1.0,
                names::SMEARED => {
                    sums[1] += wfc;
                    sums[2] += 1.0 - wfc;
                }
                _ => sums[2] += 1.0,
            }
        }
    }
    let d = inside.max(1) as f64;
    ([sums[0] / d, sums[1] / d, sums[2] / d], inside, lost)
}
