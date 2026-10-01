// SPDX-License-Identifier: GPL-3.0-only
//! **The HTR-10 material set, in one place.**
//!
//! # Why this module exists
//!
//! These materials (eleven until 2026-09-25, `mat::COUNT` since the explicit
//! reflector) were assembled inline in
//! `examples/htr10_rmc_keff.rs`. That was fine while the example was the only
//! consumer, and stopped being fine the moment a second one appeared
//! (`examples/htr10_geometry_export.rs`, which writes the model specification
//! for the double-heterogeneity manuscript). Two copies of a material set is
//! exactly the drift this workspace forbids: the exported table would go on
//! describing the model the eigenvalue *used to* be computed with.
//!
//! So the assembly lives here and both callers use it. The **environment
//! knobs stay in the example** — this function takes an explicit
//! [`Htr10MaterialConfig`] instead, so a caller that wants the default model
//! asks for the default and gets exactly what the benchmark runs.
//!
//! # The indices are load-bearing
//!
//! The returned `Vec` is indexed by [`super::core_model::mat`], and the
//! geometry refers to materials by that index. Reordering it silently
//! repoints every cell in the core at the wrong material, which is not a
//! failure that announces itself — `k_eff` simply comes out wrong. The
//! length is asserted against `mat::COUNT` for that reason.
//!
//! # Provenance
//!
//! Compositions: Li et al. (2014) Table 2 for the pebble (via
//! [`outram_mc_libs::pebble_beds::htr10::fuel_pebble_materials`]);
//! IAEA-TECDOC-1382 Table 4-3 for every reflector zone, with its p. 242
//! corrections; TECDOC ~~§ 4.1.2~~ **§ 4.1.1.5 "Control of HTR-10" (printed
//! pp. 235-236; CORRECTED 2026-10-01, gh:#428 — § 4.1.2 is the benchmark
//! problem descriptions)** for the control-rod B4C density, steel and iron.
//! That section gives no B4C isotopics: natural boron is this model's reading,
//! consistent with MIT's homogenised rod in TECDOC Table 4-36 but not stated in
//! the specification. IUPAC/CIAAW for atomic weights and isotopic compositions.
//!
//! # The nuclide slots come from a layout (2026-10-01)
//!
//! ~~[`htr10_material_set`] takes an `Htr10Nuclides` and a
//! [`RodMetalNuclides`].~~ **CHANGED 2026-10-01:** it takes an
//! [`Htr10NuclideLayout`] ([`super::data`]), the one object that says which
//! nuclide sits in which slot. Three maintainer decisions of that date are
//! carried through it:
//! - **natural carbon** in every carbon-bearing material, C-12 / C-13 at
//!   98.93 / 1.07 at.% on ENDF/B-VIII.0, elemental C-nat on VII.0 (gh:#425);
//! - **helium coolant** in [`mat::HELIUM`], not vacuum (gh:#426);
//! - **real nickel and iron** in the rod steel, or the simplified Ni -> Fe,
//!   Fe-57 -> Fe-56 mapping when asked for (gh:#329, gh:#339).

use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading};
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

use super::data::{CoolantNuclides, DataDir, Htr10NuclideLayout, Tape};

use super::core_model::{mat, HTR10_BORED_BORON, HTR10_BORED_CARBON, PAPER_FILLING_FRACTION};
use super::reflector::zone_composition;

/// Natural boron is 19.9 at.% B-10; the other 80.1 at.% is B-11, a
/// non-absorber that is placed anyway (gh:#311) because the reference states
/// natural boron.
pub const B10_OF_NATURAL: f64 = 0.199;

/// Graphite thermal scattering law, S(α,β), applied to every graphite region
/// on the ENDF/B-VIII.0 path (VII.0 has one graphite law only).
///
/// **Default: [`GraphiteLaw::Reactor30P`]** (maintainer decision 2026-09-27).
/// The choice rests on density and was not made to match k. HTR-10 graphite is
/// porous. The reflector is 1.76 g/cm³ ([`super::reflector::REFLECTOR_GRAPHITE_DENSITY`],
/// TECDOC-1382), against a crystal density of about 2.25 g/cm³, so its
/// porosity is about **22 %**. Hawari's reactor-graphite laws (Hawari &
/// Gillette, NDS 118 (2014) 176; ENDF/B-VIII.0) are tabulated at 10 % and 30 %
/// porosity only, and 30 % is the nearer. Neither is exact, and no law at 22 %
/// exists to interpolate to.
///
/// The "porosity" in these laws is vacancy disorder in the molecular-dynamics
/// phonon spectrum (atoms removed at random; the coherent elastic part is
/// kept crystalline). It is **not** a bulk-density scaling. The carbon atom
/// density of each material is set separately and does not change with this
/// choice.
///
/// **Comparison caveat.** Li, Yu & Wei (2014) used ENDF/B-VII.0, whose only
/// graphite law is crystalline. Against RMC this default therefore carries a
/// TSL term as well as the VII-vs-VIII library term. The fast single-seed
/// worth at n = 25 was +947 ± 483 pcm against crystalline
/// (`outram-mc-libs/verification_and_validation/htr10_rmc/fast_ablation_2026_09_26.md`).
/// ~~A pooled re-measurement is in progress.~~ **CORRECTED 2026-10-01
/// (gh:#428):** the pooled re-measurement is complete and recorded in the same
/// file: **30P − crystalline = +705 ± 86 pcm** (8.2σ; 4 seeds × 1.0 M active
/// histories per arm, two-ball bed). [`GraphiteLaw::Crystalline`] is the
/// explicit ablation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphiteLaw {
    /// Ideal crystalline graphite, VIII.0 MAT 30. The like-for-like law for a
    /// VII.0 reference. Ablation only.
    Crystalline,
    /// Hawari reactor graphite, 10 % porosity, VIII.0 MAT 31.
    Reactor10P,
    /// Hawari reactor graphite, 30 % porosity, VIII.0 MAT 32. **Default.**
    #[default]
    Reactor30P,
}

impl GraphiteLaw {
    /// Parse `crystalline`, `10P` or `30P`, the values accepted by
    /// `OUTRAM_HTR10_GRAPHITE_TSL`.
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "crystalline" => Some(Self::Crystalline),
            "10P" => Some(Self::Reactor10P),
            "30P" => Some(Self::Reactor30P),
            _ => None,
        }
    }

    /// Tape file name in `reference-data/endf/`.
    pub fn tape(self) -> &'static str {
        match self {
            Self::Crystalline => "tsl-crystalline-graphite.endf",
            Self::Reactor10P => "tsl-reactor-graphite-10P.endf",
            Self::Reactor30P => "tsl-reactor-graphite-30P.endf",
        }
    }

    /// MAT number as it appears in the tape's control columns. The 10P and
    /// 30P tapes' header text says MAT 35 and 36, but the records carry 31
    /// and 32, and these are the values the loader matches.
    pub fn mat(self) -> i32 {
        match self {
            Self::Crystalline => 30,
            Self::Reactor10P => 31,
            Self::Reactor30P => 32,
        }
    }
}

/// Everything the material set depends on, stated rather than read from the
/// environment — see the module docs.
#[derive(Debug, Clone, Copy)]
pub struct Htr10MaterialConfig {
    /// Temperature \[K\] every material is built at.
    pub temperature_k: f64,
    /// How the two "ppm natural boron" rows of Table 2 are read.
    pub boron: BoronReading,
    /// Which TECDOC Table 4-3 zone's composition fills material slot
    /// [`mat::REFLECTOR`].
    ///
    /// ~~Stands in for the whole reflector; 22 is the default and the
    /// OPTIMISTIC bound (cleanest graphite).~~ **CORRECTED 2026-10-01
    /// (gh:#428):** since the explicit reflector and the Fig. 4.10 zone map,
    /// `mat::REFLECTOR` fills only the zones TECDOC p. 242 says take zone 22's
    /// density once the borings are explicit (the zone-22 list of
    /// [`mat::for_zone_mc`]). Every other
    /// zone has its own slot. (Under the `OUTRAM_HTR10_NO_ZONE_MAP` ablation it
    /// fills every zone but the boronated bricks.) 22 is therefore the model,
    /// not a bound; any other value is a sensitivity on those zones only.
    pub reflector_zone: usize,
    /// Multiplier on the reflector's carbon density. `1.0` is unmodified.
    /// A sensitivity bound, not a model.
    pub reflector_carbon_scale: f64,
}

impl Htr10MaterialConfig {
    /// The configuration the reported benchmark results are computed with.
    #[must_use]
    pub fn benchmark_default(temperature_k: f64) -> Self {
        Self {
            temperature_k,
            boron: BoronReading::Natural,
            reflector_zone: 22,
            reflector_carbon_scale: 1.0,
        }
    }
}

/// Indices, into the caller's nuclide array, of the nuclides the withdrawn
/// control rods need beyond the pebble's `Htr10Nuclides`: the sleeve steel
/// and the iron joints.
///
/// Silicon appears here AGAIN, as free gas: the pebble's silicon is bound in
/// SiC with its own S(alpha, beta), which is wrong for silicon dissolved in
/// steel. Carbon in steel and in B4C uses the pebble table's free carbon
/// (`Htr10Nuclides::c_free`, natural C-12 / C-13 since 2026-10-01) for the
/// same reason.
///
/// Under the simplified rod-metal case the Ni (and Fe-57) fields point at Fe
/// slots; see [`super::data::RodMetalTreatment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct RodMetalNuclides {
    pub fe54: usize,
    pub fe56: usize,
    pub fe57: usize,
    pub fe58: usize,
    pub cr50: usize,
    pub cr52: usize,
    pub cr53: usize,
    pub cr54: usize,
    pub ni58: usize,
    pub ni60: usize,
    pub ni61: usize,
    pub ni62: usize,
    pub ni64: usize,
    pub mn55: usize,
    pub ti46: usize,
    pub ti47: usize,
    pub ti48: usize,
    pub ti49: usize,
    pub ti50: usize,
    /// Free-gas Si-28 (NOT the SiC-bound slot).
    pub si28: usize,
    /// Free-gas Si-29.
    pub si29: usize,
    /// Free-gas Si-30.
    pub si30: usize,
}

impl RodMetalNuclides {
    /// Number of nuclide slots this table names.
    pub const COUNT: usize = 22;

    /// The slots laid out consecutively from `first`, in field order
    /// (Fe-54..58, Cr-50..54, Ni-58..64, Mn-55, Ti-46..50, free Si-28..30) --
    /// the order a caller appends them to its nuclide array in.
    #[must_use]
    pub const fn contiguous(first: usize) -> Self {
        let f = first;
        Self {
            fe54: f,
            fe56: f + 1,
            fe57: f + 2,
            fe58: f + 3,
            cr50: f + 4,
            cr52: f + 5,
            cr53: f + 6,
            cr54: f + 7,
            ni58: f + 8,
            ni60: f + 9,
            ni61: f + 10,
            ni62: f + 11,
            ni64: f + 12,
            mn55: f + 13,
            ti46: f + 14,
            ti47: f + 15,
            ti48: f + 16,
            ti49: f + 17,
            ti50: f + 18,
            si28: f + 19,
            si29: f + 20,
            si30: f + 21,
        }
    }
}

/// ENDF/B-VIII.0 tapes for the rod-metal nuclides, in
/// [`RodMetalNuclides::contiguous`] order, as `(name, tape)`.
///
/// Every tape is in `reference-data/endf/` except the five **nickel** tapes,
/// which are in the `reference-data/ace` submodule
/// (`ace/endf/endf-b-viii.0/`, listed in its `MANIFEST.tsv`) since 2026-10-01:
/// the maintainer decided on 2026-09-25 that no Ni data is committed to this
/// repository (gh:#329, ~48 MB). ~~(in `reference-data/endf/`)~~
///
/// The silicon tapes are the same files as the SiC slots'; they are loaded a
/// second time WITHOUT a thermal law. There is no ENDF/B-VII.0 counterpart in
/// the checkout for the metals, so a VII.0 run takes these VIII.0 tapes for
/// the rod metal only, and must say so.
pub const ROD_METAL_TAPES_ENDF8: [(&str, Tape); RodMetalNuclides::COUNT] = [
    ("Fe54", Tape::endf("n-026_Fe_054-ENDF8.0.endf")),
    ("Fe56", Tape::endf("n-026_Fe_056-ENDF8.0.endf")),
    ("Fe57", Tape::endf("n-026_Fe_057-ENDF8.0.endf")),
    ("Fe58", Tape::endf("n-026_Fe_058-ENDF8.0.endf")),
    ("Cr50", Tape::endf("n-024_Cr_050-ENDF8.0.endf")),
    ("Cr52", Tape::endf("n-024_Cr_052-ENDF8.0.endf")),
    ("Cr53", Tape::endf("n-024_Cr_053-ENDF8.0.endf")),
    ("Cr54", Tape::endf("n-024_Cr_054-ENDF8.0.endf")),
    ("Ni58", NI_TAPE[0]),
    ("Ni60", NI_TAPE[1]),
    ("Ni61", NI_TAPE[2]),
    ("Ni62", NI_TAPE[3]),
    ("Ni64", NI_TAPE[4]),
    ("Mn55", Tape::endf("n-025_Mn_055-ENDF8.0.endf")),
    ("Ti46", Tape::endf("n-022_Ti_046-ENDF8.0.endf")),
    ("Ti47", Tape::endf("n-022_Ti_047-ENDF8.0.endf")),
    ("Ti48", Tape::endf("n-022_Ti_048-ENDF8.0.endf")),
    ("Ti49", Tape::endf("n-022_Ti_049-ENDF8.0.endf")),
    ("Ti50", Tape::endf("n-022_Ti_050-ENDF8.0.endf")),
    ("Si28", Tape::endf("n-014_Si_028-ENDF8.0.endf")),
    ("Si29", Tape::endf("n-014_Si_029-ENDF8.0.endf")),
    ("Si30", Tape::endf("n-014_Si_030-ENDF8.0.endf")),
];

/// The five nickel tapes, ENDF/B-VIII.0, in the ACE submodule.
const NI_TAPE: [Tape; 5] = [
    Tape { dir: DataDir::AceSubmoduleEndfB8, file: "n-028_Ni_058-ENDF8.0.endf" },
    Tape { dir: DataDir::AceSubmoduleEndfB8, file: "n-028_Ni_060-ENDF8.0.endf" },
    Tape { dir: DataDir::AceSubmoduleEndfB8, file: "n-028_Ni_061-ENDF8.0.endf" },
    Tape { dir: DataDir::AceSubmoduleEndfB8, file: "n-028_Ni_062-ENDF8.0.endf" },
    Tape { dir: DataDir::AceSubmoduleEndfB8, file: "n-028_Ni_064-ENDF8.0.endf" },
];

/// He-3 atom fraction of natural (atmospheric) helium, 1.343e-6.
///
/// Source: IUPAC/CIAAW representative isotopic composition of helium,
/// 0.000 001 343(13) He-3 / 0.999 998 657(13) He-4 (Meija et al., *Pure Appl.
/// Chem.* 88 (2016) 293-306, Table 1). He-3's 5333 b thermal (n,p) makes it
/// the only part of the coolant that absorbs at all; at this fraction it is
/// ~3e-11 atoms/(b cm).
pub const HE3_ATOM_FRACTION_OF_NATURAL_HE: f64 = 1.343e-6;

/// Helium coolant pressure \[kPa\]: **101.33 kPa, an ASSUMPTION** (gh:#426).
///
/// Şeker & Çolak (2003), NED 222:263, p.267 states atmospheric pressure,
/// 101.33 kPa, for its air case and gives no other pressure; the HTR-10
/// first-criticality loading was at room temperature (27 °C, as Li and Şeker
/// state), and neither paper states the helium pressure. The helium case is
/// therefore taken as atmospheric, and this is stated rather than implied.
pub const HELIUM_PRESSURE_KPA: f64 = 101.33;

/// Boltzmann constant \[J/K\] (SI 2019, exact).
const BOLTZMANN: f64 = 1.380_649e-23;

/// Atom density of an ideal gas \[atoms/(b cm)\]: `N = p / (k_B T)`.
///
/// At 300.15 K and 101.33 kPa this is **2.4452e-5 atoms/(b cm)**. Helium at
/// one atmosphere is ideal to ~5e-4 (second virial coefficient ~12 cm3/mol),
/// far below anything an eigenvalue sees.
#[must_use]
pub fn ideal_gas_atom_density(temperature: ThermodynamicTemperature, pressure: Pressure) -> f64 {
    // atoms/m3 -> atoms/(b cm): 1 b cm = 1e-24 cm3 = 1e-30 m3.
    pressure.get::<pascal>() / (BOLTZMANN * temperature.get::<kelvin>()) * 1.0e-30
}

/// Avogadro constant \[1/mol\] (CODATA 2018, exact).
const AVOGADRO: f64 = 6.022_140_76e23;

/// Standard atomic weights \[g/mol\], IUPAC/CIAAW (Meija et al., *Pure Appl.
/// Chem.* 88 (2016) 265-291, Table 1; conventional values where an interval is
/// given): the seven constituents of the rod steel.
pub mod atomic_weight {
    /// Chromium.
    pub const CR: f64 = 51.9961;
    /// Iron.
    pub const FE: f64 = 55.845;
    /// Nickel.
    pub const NI: f64 = 58.6934;
    /// Silicon (conventional value).
    pub const SI: f64 = 28.085;
    /// Manganese.
    pub const MN: f64 = 54.938_044;
    /// Carbon (conventional value).
    pub const C: f64 = 12.011;
    /// Titanium.
    pub const TI: f64 = 47.867;
    /// Boron (conventional value).
    pub const B: f64 = 10.811;
}

/// Representative natural isotopic compositions \[atom fraction\], IUPAC/CIAAW
/// (Meija et al., *Pure Appl. Chem.* 88 (2016) 293-306, Table 1).
pub mod abundance {
    /// Fe-54, Fe-56, Fe-57, Fe-58.
    pub const FE: [f64; 4] = [0.05845, 0.91754, 0.02119, 0.00282];
    /// Cr-50, Cr-52, Cr-53, Cr-54.
    pub const CR: [f64; 4] = [0.04345, 0.83789, 0.09501, 0.02365];
    /// Ni-58, Ni-60, Ni-61, Ni-62, Ni-64.
    pub const NI: [f64; 5] = [0.680_769, 0.262_231, 0.011_399, 0.036_345, 0.009_256];
    /// Ti-46, Ti-47, Ti-48, Ti-49, Ti-50.
    pub const TI: [f64; 5] = [0.0825, 0.0744, 0.7372, 0.0541, 0.0518];
}

/// Rod sleeve steel density \[g/cm³\], TECDOC ~~§ 4.1.2~~ § 4.1.1.5, pp. 235-236
/// (CORRECTED 2026-10-01, gh:#428; *"a density of 7.9g/cm3 is assumed"*).
pub const ROD_STEEL_DENSITY: f64 = 7.9;
/// Rod sleeve steel composition \[weight fraction\], TECDOC ~~§ 4.1.2~~
/// § 4.1.1.5, pp. 235-236 (CORRECTED 2026-10-01, gh:#428):
/// Cr 18, Fe 68.1, Ni 10, Si 1, Mn 2, C 0.1, Ti 0.8 (sums to 100 %).
pub const ROD_STEEL_WT: [(&str, f64); 7] = [
    ("Cr", 0.18),
    ("Fe", 0.681),
    ("Ni", 0.10),
    ("Si", 0.01),
    ("Mn", 0.02),
    ("C", 0.001),
    ("Ti", 0.008),
];
/// Iron atom density of the rod joints and ends \[atoms/(b cm)\], TECDOC
/// ~~§ 4.1.2~~ § 4.1.1.5, pp. 235-236 (CORRECTED 2026-10-01, gh:#428): iron alone,
/// filling 27.5 mm < R < 55 mm.
pub const ROD_JOINT_IRON_DENSITY: f64 = 0.04;

/// Build the material set, indexed by [`mat`] (`mat::COUNT` materials).
///
/// **Signature changed 2026-09-25** to take [`RodMetalNuclides`]: the ten
/// control rods are now explicit geometry at their withdrawn position, and
/// their steel sleeves and iron joints need nuclides the pebble set has no
/// slots for. **Changed again 2026-10-01** to take the whole
/// [`Htr10NuclideLayout`] (pebble, coolant and rod-metal slots in one), so
/// the carbon split, the coolant and the rod-metal treatment are decided once,
/// where the nuclides are.
///
/// # Panics
///
/// If `reflector_zone` is not listed in TECDOC Table 4-3, or if the assembled
/// length does not match the index table.
#[must_use]
pub fn htr10_material_set(layout: &Htr10NuclideLayout, cfg: Htr10MaterialConfig) -> Vec<Material> {
    let n = layout.pebble;
    let metal = layout.metal;
    let t = cfg.temperature_k;
    // Natural carbon at `total` atoms/(b cm) in graphite (C-12 + C-13 on
    // VIII.0, C-nat on VII.0; gh:#425).
    let graphite_c = |total: f64| n.c_graphite.components(total);
    let boron = cfg.boron;
    // B-10 density for a zone, honouring the `BoronReading::None` ablation.
    let b10 = |natural: f64| {
        if matches!(boron, BoronReading::None) {
            0.0
        } else {
            natural * B10_OF_NATURAL
        }
    };
    // B-11, the rest of the same natural boron (gh:#311). Until 2026-09-25 only
    // B-10 was placed, leaving the boronated brick ~3.5 % short on atoms.
    let b11 = |natural: f64| {
        if matches!(boron, BoronReading::None) {
            0.0
        } else {
            natural * (1.0 - B10_OF_NATURAL)
        }
    };

    let mut mats = fuel_pebble_materials(n, boron, t);
    // `fuel_pebble_materials` returns a 7th entry ("shell graphite") that is
    // identical to the matrix graphite at index 5; the core model uses one
    // index for both, so slot 6 is reused for helium.
    mats.truncate(6);

    // 6: the coolant, in every coolant region of the model (`mat::HELIUM`).
    // ~~helium -- deliberately near-void, as the reference's own model omits
    // it.~~ CORRECTED 2026-10-01 (gh:#426): that misread Li, who says
    // "Calculations are performed for vacuum and helium". The material was
    // exact vacuum. Since the maintainer's decision of 2026-10-01 it is natural
    // helium, an ideal gas at the material temperature and HELIUM_PRESSURE_KPA
    // (assumed atmospheric); vacuum is the `Coolant::Vacuum` ablation.
    let (coolant_name, coolant) = match layout.coolant {
        CoolantNuclides::Helium { he3, he4 } => {
            let n_he = ideal_gas_atom_density(
                ThermodynamicTemperature::new::<kelvin>(t),
                Pressure::new::<pascal>(HELIUM_PRESSURE_KPA * 1.0e3),
            );
            (
                format!("helium ({HELIUM_PRESSURE_KPA} kPa, {t:.2} K)"),
                vec![
                    NuclideComponent {
                        nuclide_idx: he3,
                        atom_density: n_he * HE3_ATOM_FRACTION_OF_NATURAL_HE,
                    },
                    NuclideComponent {
                        nuclide_idx: he4,
                        atom_density: n_he * (1.0 - HE3_ATOM_FRACTION_OF_NATURAL_HE),
                    },
                ],
            )
        }
        CoolantNuclides::Vacuum => ("vacuum (coolant ablation)".to_string(), vec![]),
    };
    mats.push(Material {
        id: 70,
        name: coolant_name,
        components: coolant,
        temperature: t,
    });

    // 7: reflector graphite, TECDOC Table 4-3.
    let z = zone_composition(cfg.reflector_zone).expect("reflector zone is listed in Table 4-3");
    mats.push(Material {
        id: 71,
        name: format!("reflector graphite (TECDOC zone {})", cfg.reflector_zone),
        components: graphite_c(z.carbon * cfg.reflector_carbon_scale)
            .into_iter()
            .chain([
                NuclideComponent {
                    nuclide_idx: n.b10,
                    atom_density: b10(z.natural_boron),
                },
                NuclideComponent {
                    nuclide_idx: n.b11,
                    atom_density: b11(z.natural_boron),
                },
            ])
            .collect(),
        temperature: t,
    });

    // 8: boronated carbon brick, the outermost reflector annulus. Zone 17 --
    // natural boron 3.4635e-3, ~7300x zone 22's.
    let zb = zone_composition(17).expect("zone 17 is listed");
    mats.push(Material {
        id: 72,
        name: "boronated carbon brick (TECDOC zone 17)".into(),
        components: graphite_c(zb.carbon)
            .into_iter()
            .chain([
                NuclideComponent {
                    nuclide_idx: n.b10,
                    atom_density: b10(zb.natural_boron),
                },
                NuclideComponent {
                    nuclide_idx: n.b11,
                    atom_density: b11(zb.natural_boron),
                },
            ])
            .collect(),
        temperature: t,
    });

    // 9: side reflector homogenised with its control-rod borings, TECDOC
    // zones 31-40 -- 28.1 % less carbon than zone 22. Built but NOT placed by
    // `assemble_explicit_triso` since 2026-09-25 (the borings are explicit);
    // the slot keeps later indices fixed (see `mat::BORED_GRAPHITE`).
    mats.push(Material {
        id: 73,
        name: "bored side reflector (TECDOC zones 31-40)".into(),
        components: graphite_c(HTR10_BORED_CARBON)
            .into_iter()
            .chain([
                NuclideComponent {
                    nuclide_idx: n.b10,
                    atom_density: b10(HTR10_BORED_BORON),
                },
                NuclideComponent {
                    nuclide_idx: n.b11,
                    atom_density: b11(HTR10_BORED_BORON),
                },
            ])
            .collect(),
        temperature: t,
    });

    // 10: homogenised dummy pebbles = pebble graphite scaled to the bed's
    // filling fraction. ~~What the discharge tube actually contains (Terry 2005
    // section 2), between the bounds of solid graphite and pure helium.~~
    // CORRECTED 2026-10-01 (gh:#428; already struck at `mat::HOMOG_DUMMY` on
    // 2026-09-25): the tube holds whole graphite balls, which the default model
    // places explicitly. This smear is only the `OUTRAM_HTR10_HOMOG_TUBE`
    // ablation.
    let dummy = mats[mat::GRAPHITE].clone();
    mats.push(Material {
        id: 74,
        name: "homogenised dummy pebbles (0.61 packing)".into(),
        components: dummy
            .components
            .iter()
            .map(|c| NuclideComponent {
                nuclide_idx: c.nuclide_idx,
                atom_density: c.atom_density * PAPER_FILLING_FRACTION,
            })
            .collect(),
        temperature: t,
    });

    // 11..: the Table 4-3 zones that keep a composition of their own in the
    // Monte Carlo model, with TECDOC p. 242's correction factors (see
    // `mat::TABLE_4_3_ZONES`). The factor scales carbon and boron alike: it
    // restores the graphite a homogenised boring void had diluted.
    assert_eq!(mats.len(), mat::ZONE_TABLE_FIRST);
    for (zone, factor) in mat::TABLE_4_3_ZONES {
        let z = zone_composition(zone).expect("every TABLE_4_3_ZONES entry is in Table 4-3");
        let mut components = graphite_c(z.carbon * factor);
        if z.natural_boron > 0.0 {
            components.push(NuclideComponent {
                nuclide_idx: n.b10,
                atom_density: b10(z.natural_boron * factor),
            });
            components.push(NuclideComponent {
                nuclide_idx: n.b11,
                atom_density: b11(z.natural_boron * factor),
            });
        }
        mats.push(Material {
            id: 100 + zone as i32,
            name: if factor == 1.0 {
                format!("TECDOC Table 4-3 zone {zone}")
            } else {
                format!("TECDOC Table 4-3 zone {zone} x {factor} (p. 242)")
            },
            components,
            temperature: t,
        });
    }

    // Control-rod B4C: 1.7 g/cm3 of B4C (TECDOC ~~§ 4.1.2~~ § 4.1.1.5, pp. 235-236;
    // CORRECTED 2026-10-01, gh:#428) with natural boron. ~~(TECDOC § 4.1.2)~~
    // The specification gives no isotopics: natural boron is this model's
    // reading, consistent with MIT's TECDOC Table 4-36, not stated by § 4.1.1.5.
    // Its carbon is NOT graphite, so it takes the free-gas carbon slot
    // (natural C-12 / C-13 on VIII.0 since 2026-10-01, gh:#425).
    let n_b4c = super::control_rod::b4c_molecular_density();
    mats.push(Material {
        id: 90,
        name: "control-rod B4C (1.7 g/cm3)".into(),
        components: [
            NuclideComponent {
                nuclide_idx: n.b10,
                atom_density: b10(4.0 * n_b4c),
            },
            NuclideComponent {
                nuclide_idx: n.b11,
                atom_density: b11(4.0 * n_b4c),
            },
        ]
        .into_iter()
        .chain(n.c_free.components(n_b4c))
        .collect(),
        temperature: t,
    });

    // Control-rod sleeve steel, 7.9 g/cm3 (TECDOC § 4.1.1.5, pp. 235-236; was
    // cited as § 4.1.2, CORRECTED 2026-10-01, gh:#428), split into
    // natural isotopes. N_e = rho w_e N_A / M_e.
    let n_elem = |w: f64, m: f64| ROD_STEEL_DENSITY * w * AVOGADRO / m * 1.0e-24;
    let wt = |e: &str| {
        ROD_STEEL_WT
            .iter()
            .find(|(s, _)| *s == e)
            .map(|(_, w)| *w)
            .expect("listed element")
    };
    let mut steel: Vec<NuclideComponent> = Vec::new();
    let mut split = |total: f64, idx: &[usize], frac: &[f64]| {
        for (&i, &f) in idx.iter().zip(frac) {
            steel.push(NuclideComponent {
                nuclide_idx: i,
                atom_density: total * f,
            });
        }
    };
    split(
        n_elem(wt("Fe"), atomic_weight::FE),
        &[metal.fe54, metal.fe56, metal.fe57, metal.fe58],
        &abundance::FE,
    );
    split(
        n_elem(wt("Cr"), atomic_weight::CR),
        &[metal.cr50, metal.cr52, metal.cr53, metal.cr54],
        &abundance::CR,
    );
    split(
        n_elem(wt("Ni"), atomic_weight::NI),
        &[metal.ni58, metal.ni60, metal.ni61, metal.ni62, metal.ni64],
        &abundance::NI,
    );
    split(
        n_elem(wt("Ti"), atomic_weight::TI),
        &[metal.ti46, metal.ti47, metal.ti48, metal.ti49, metal.ti50],
        &abundance::TI,
    );
    split(
        n_elem(wt("Si"), atomic_weight::SI),
        &[metal.si28, metal.si29, metal.si30],
        &[
            outram_mc_libs::pebble_beds::htr10::SI28_ATOM_FRACTION,
            outram_mc_libs::pebble_beds::htr10::SI29_ATOM_FRACTION,
            outram_mc_libs::pebble_beds::htr10::SI30_ATOM_FRACTION,
        ],
    );
    split(n_elem(wt("Mn"), atomic_weight::MN), &[metal.mn55], &[1.0]);
    // Steel carbon: natural carbon, free gas (gh:#425).
    for (i, d) in n.c_free.split(n_elem(wt("C"), atomic_weight::C)) {
        split(d, &[i], &[1.0]);
    }
    mats.push(Material {
        id: 91,
        name: "control-rod sleeve steel (7.9 g/cm3)".into(),
        components: steel,
        temperature: t,
    });

    // Control-rod joints and ends: iron only, 0.04 atoms/(b cm).
    mats.push(Material {
        id: 92,
        name: "control-rod joint iron (0.04 /b-cm)".into(),
        components: [metal.fe54, metal.fe56, metal.fe57, metal.fe58]
            .iter()
            .zip(abundance::FE)
            .map(|(&i, f)| NuclideComponent {
                nuclide_idx: i,
                atom_density: ROD_JOINT_IRON_DENSITY * f,
            })
            .collect(),
        temperature: t,
    });

    assert_eq!(
        mats.len(),
        mat::COUNT,
        "material set length must match the `mat` index table"
    );
    mats
}

/// Human-readable name for each nuclide slot, for reporting.
///
/// A material component carries an index and nothing else, so anything that
/// reports a composition needs this to turn the index back into a name.
/// ~~Took `Htr10Nuclides` and named only the eleven pebble slots.~~
/// **CHANGED 2026-10-01:** reads the layout, so it names every slot (C-13,
/// helium, rod metals) with the thermal law the layout binds to it.
#[must_use]
pub fn nuclide_name(layout: &Htr10NuclideLayout, idx: usize) -> String {
    layout.nuclide_name(idx)
}
