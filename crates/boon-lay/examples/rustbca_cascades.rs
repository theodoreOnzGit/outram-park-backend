//! # RustBCA cascades in an HTR-10 UO2 kernel and its TRISO coatings — headless cost probe
//!
//! **What this measures.** The wall-clock cost of ONE full binary-collision
//! cascade (the primary plus every recoil it knocks on above the target's cutoff
//! energy, `track_recoils = true`) for the charged particles boon-lay is to
//! carry (gh:#876 item 7, maintainer answer 4: "try first via an example
//! headless"): fission fragments, the B-10(n,α) alpha, and C / O neutron-recoil
//! knock-ons, each in the layer of an HTR-10 TRISO particle it is born in. That
//! number sets what a browser demo can afford to draw. It also reports each
//! primary's range, as a sanity check on the physics and as the first,
//! UNVALIDATED look at fragment ranges.
//!
//! Run: `cargo run --release -j 6 -p boon-lay --example rustbca_cascades`
//! (`RUSTBCA_CASCADES_SCALE=0.25` runs a quarter of the cascades). Single
//! thread, by design: a per-cascade time is the quantity.
//! Tests of the helpers: `cargo test --release -j 6 -p boon-lay --example rustbca_cascades`.
//!
//! **Fragment results are ILLUSTRATIVE.** RustBCA's high-energy electronic
//! stopping is Bethe with the bare projectile charge `Za²`
//! (`material::bethe_bloch_stopping_power_cross_section`), no effective
//! charge, and fragments at 0.5–1 MeV/u are exactly where a heavy ion's charge
//! state matters. Nothing here is compared with a measured range, and nothing
//! is tuned: `INTERPOLATEDPLUS`'s `ci` is not used.
//!
//! ## Method
//!
//! * **Code:** boon-lay's copy of RustBCA v3.0.0 (`a356280d`), `no_std` core,
//!   `rustbca::bca::single_ion_bca` one primary at a time.
//! * **Geometry:** RustBCA's `Mesh1D`, the TRISO coatings UNROLLED into planar
//!   layers: x = 0 is the OPyC outer surface (vacuum beyond; the matrix
//!   graphite is not modelled), then OPyC 40, SiC 35, IPyC 40, buffer 90 µm and
//!   the kernel's 250 µm radius, so x = 455 µm is the kernel centre. Radii from
//!   `outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec::HTR10_LI2014`
//!   (IAEA-TECDOC-1382 Table 4-2 and Li, Yu & Wei 2014 Table 2, which agree).
//!   Curvature is ignored: every range here is under 10 µm against a 250 µm
//!   kernel radius. RustBCA has no layered sphere; a `Geometry` impl for one is
//!   the next step if curvature matters. Drawn from the assembled `Mesh1D`
//!   (a density lookup per 0.5 µm) to
//!   `verification_and_validation/rustbca_cascades/triso_stack_mesh1d.svg`,
//!   which also marks every birth point.
//! * **Composition:** the HTR-10 fuel-pebble materials of
//!   `outram_mc_libs::pebble_beds::htr10::fuel_pebble_materials`
//!   (`Htr10Nuclides::ELEMENTAL_CARBON`, `BoronReading::Natural`): UO2 10.4
//!   g/cm³ at 17 wt% U-235 with 4 atom-ppm natural B, buffer 1.1, PyC 1.9 and
//!   SiC 3.18 g/cm³, carbon with 1.3 atom-ppm natural B. Atom densities in
//!   atoms/(b·cm) are atoms/Å³, which is what `Mesh1D` is given.
//! * **Target-atom parameters:** surface binding `Es`, bulk binding `Eb` and
//!   cutoff `Ec` per element from upstream RustBCA's own table
//!   (`scripts/materials.py` at `a356280d`): U 5.19 / 0 / 3 eV, O 2.58 / 2.58
//!   / 2 eV, C 7.37 / 0 / 1 eV, Si 4.72 / 0 / 1.5 eV, B 5.77 / 0 / 5 eV. A
//!   recoil is followed while its energy exceeds its element's `Ec`. Masses:
//!   isotopic (AME2020) except carbon, natural 12.011 as in the composition.
//! * **Projectiles:** cutoff 1 eV and `Es` 0, as upstream's table gives its
//!   projectile-only entries (He, Xe); knock-ons take their element's values.
//! * **Physics options:** `Options::default_options(true)` (Kr-C potential,
//!   Mendenhall-Weller scattering integral, Newton root-finder, liquid
//!   (amorphous) mean free path, no weak collisions) with
//!   `electronic_stopping_mode = INTERPOLATED`, the only mode valid at
//!   fragment and alpha energies (the low-energy modes stop at ~25 keV/u), and
//!   used for the knock-ons too for one consistent model. Correction factor
//!   `ck = 1`. Fragments are also run in a second arm with
//!   `high_energy_free_flight_paths = true`, upstream's own option for fast
//!   ions, to price it; the default arm is the one to quote.
//! * **Birth energies, derived from evaluated data (ENDF/B-VIII.0 tapes in
//!   `reference-data/endf/`):**
//!   - **Fragments.** U-235 MF1/MT458 `EFR` = 169.13 MeV (E = 0 coefficient),
//!     split between a light A = 95 (Sr-95, Z = 38) and heavy A = 139 (Xe-139,
//!     Z = 54) fragment by momentum conservation, `E_L = EFR·A_H/(A_L+A_H)`:
//!     **100.47 MeV** light, **68.66 MeV** heavy. A = 95 / 139 are the usual
//!     peak masses of the U-235 thermal mass-yield curve and Z = 38 / 54 the
//!     pair summing to 92; the masses are taken as A u (0.1 % approximation).
//!   - **Alpha.** B-10 MF3/MT801 `QI` = 2.311472 MeV (the Li-7* branch), shared
//!     with a thermal neutron's momentum neglected: `E_α = QI·M_Li7/(M_Li7+M_α)`
//!     = **1.4718 MeV**.
//!   - **Knock-ons.** The mean elastic recoil `<E_R> = E_n·2A/(A+1)²` (isotropic
//!     centre of mass, an assumption: MeV scattering on C and O is forward
//!     peaked, which lowers the true mean) at `E_n` = the U-235 mean prompt
//!     neutron energy, MT458 `ENP` / MT456 ν̄_p(thermal) = 4.827645 MeV / 2.414
//!     = 1.99985 MeV: **C-12 286.1 keV**, **O-16 223.2 keV**.
//! * **Birth points** (unrolled x): kernel middle 330 µm; 2 µm inside the
//!   kernel surface, 207 µm (light fragments only, to see the buffer catch
//!   them); buffer middle 160 µm; IPyC middle 95 µm; SiC middle 57.5 µm.
//!   Directions isotropic.
//! * **Statistics:** each cascade has its own ChaCha8 stream (seed 876,
//!   stream = case·10⁶ + index); the direction is drawn from it first. Range =
//!   net distance from birth to rest; projected range = along the birth
//!   direction. Mean ± standard error over the cascades; time per cascade is
//!   `std::time::Instant` around `single_ion_bca`, mean and standard deviation.
//!   The first cascade of every case is re-run with its seed and must come back
//!   bit-identical, or the program panics.
//!
//! ## Predictions, written 2026-10-16 BEFORE the first run
//!
//! Sign and magnitude, from first-principles estimates (not from any run):
//!
//! | case | range | cost per cascade | tracked recoils |
//! |---|---|---|---|
//! | light fragment, kernel | 4–9 µm, central 6 µm | 0.1–2 s | 10³–10⁵ |
//! | heavy fragment, kernel | 3–8 µm, central 5 µm, shorter than light | 0.1–2 s | 10³–10⁵ |
//! | light fragment, 2 µm from kernel surface | as above | as above | — |
//! | alpha, kernel (UO2) | 2–4 µm | 5–50 ms | 1–100 |
//! | alpha, buffer (1.1 g/cm³ C) | 5–9 µm | 5–50 ms | 1–100 |
//! | O-16 knock-on, kernel | 0.1–0.4 µm | 1–20 ms | 10–300 |
//! | C-12 knock-on, buffer / IPyC / SiC | 0.6–1.5 / 0.4–0.9 / 0.3–0.7 µm | 1–20 ms | 10–300 |
//!
//! * **Fragment range sign:** of order µm. The commonly quoted fission-fragment
//!   range in UO2 is "about 10 µm" (an order-of-magnitude recollection, NOT
//!   from a source in the workspace corpus and not checked). The bare-`Za²`
//!   Bethe term over-states electronic stopping at high velocity (a light
//!   fragment's equilibrium charge is roughly half its Z), so the prediction
//!   is that RustBCA comes out SHORT of that, by up to a factor of ~2.
//! * **Cost:** the default (no free-flight) model takes one collision per
//!   ~2.4 Å of UO2 (`n^(-1/3)`), so ~3×10⁴ primary steps for a fragment, and a
//!   fragment knocks most uranium atoms it passes by a few eV, so 10³–10⁵
//!   recoils. At ~1 µs per collision step: 0.1–2 s. The free-flight arm should
//!   be ≥ 3× cheaper with ranges agreeing to ~10 %.
//! * **Surface-born light fragments:** about half head outward and, with a
//!   ~6 µm range, those within ~70° of the normal clear 2 µm: 25–40 % come to
//!   rest outside the kernel (buffer).
//!
//! ## Results, 2026-10-16 (written after the run; the predictions above are unedited)
//!
//! **Hardware:** Intel Core i9-13900K (32 logical CPUs; the process saw 16
//! through `available_parallelism`), **1 thread used**, 62.5 GiB RAM, Arch
//! Linux 7.2.9, rustc 1.98.0, release profile, CPU only (an RTX A5000 is
//! present and unused). **The machine was shared:** load average 6.5 at the
//! start of the run from other builds, so absolute times are upper-side;
//! compare cases with each other, not with another host. Full run (scale 1)
//! 220.8 s. Every case's first cascade re-ran bit-identically.
//!
//! | case | n | range µm (mean ± sem) | projected µm | s per cascade (mean, sd) | tracked recoils (mean, sd) | ended outside birth layer |
//! |---|---|---|---|---|---|---|
//! | light fragment Sr-95, kernel middle (**ILLUSTRATIVE**) | 16 | 10.69 ± 0.17 | 10.53 ± 0.20 | 1.47, 0.37 | 317 562, 81 974 | 0.00 |
//! | heavy fragment Xe-139, kernel middle (**ILLUSTRATIVE**) | 16 | 7.41 ± 0.12 | 7.32 ± 0.13 | 2.00, 0.24 | 454 318, 56 603 | 0.00 |
//! | light fragment, 2 µm inside kernel surface (**ILLUSTRATIVE**) | 16 | 16.8 ± 2.2 | 16.6 ± 2.2 | 2.36, 1.4 | 839 512, 738 744 | 0.38 |
//! | light fragment, free-flight arm (**ILLUSTRATIVE**) | 16 | 10.59 ± 0.13 | 10.35 ± 0.19 | 2.27, 0.68 | 334 531, 101 391 | 0.00 |
//! | heavy fragment, free-flight arm (**ILLUSTRATIVE**) | 16 | 7.34 ± 0.09 | 7.17 ± 0.10 | 3.09, 0.49 | 456 875, 70 458 | 0.00 |
//! | alpha 1.472 MeV, kernel | 64 | 3.907 ± 0.033 | 3.780 ± 0.041 | 0.0283, 0.0024 | 1 452, 566 | 0.00 |
//! | alpha 1.472 MeV, buffer | 64 | 9.430 ± 0.012 | 9.413 ± 0.013 | 0.0635, 0.0049 | 9 598, 2 393 | 0.00 |
//! | O-16 223 keV, kernel | 64 | 0.320 ± 0.009 | 0.244 ± 0.013 | 0.0287, 0.0048 | 6 218, 1 155 | 0.00 |
//! | C-12 286 keV, buffer | 64 | 1.261 ± 0.017 | 1.235 ± 0.018 | 0.0925, 0.017 | 40 968, 7 949 | 0.00 |
//! | C-12 286 keV, IPyC | 64 | 0.728 ± 0.009 | 0.712 ± 0.010 | 0.109, 0.022 | 54 422, 10 932 | 0.00 |
//! | C-12 286 keV, SiC | 64 | 0.570 ± 0.009 | 0.546 ± 0.010 | 0.0832, 0.020 | 36 810, 8 571 | 0.00 |
//!
//! **Against the predictions:**
//!
//! * **Ranges.** Heavy fragment 7.4 µm, inside 3–8 µm. **Light fragment
//!   10.7 µm, OUTSIDE the predicted 4–9 µm**, and the predicted sign
//!   ("short of ~10 µm because the bare-`Za²` Bethe term over-stops") is
//!   **not supported**. Why is not known: the high-energy term is combined
//!   harmonically with Lindhard-Scharff, so which term governs at 1 MeV/u
//!   was not checked. Light > heavy, as predicted (ratio 1.44). Alpha in UO2
//!   3.9 µm (in 2–4); alpha in the buffer 9.4 µm, **just above** 5–9; O and
//!   C knock-ons all inside their bands. None of these is a validation:
//!   nothing is compared with measured or tabulated ranges yet.
//! * **Surface-born light fragments:** 38 % came to rest outside the kernel,
//!   inside the predicted 25–40 %. Their mean range, 16.8 µm, is longer
//!   because the 1.1 g/cm³ buffer stops less per µm than UO2.
//! * **Cost: the prediction was too low, for one reason.** Tracked recoils
//!   are **10–100× more** than predicted in every case (3–5 × 10⁵ per
//!   fragment, 10³–10⁴ per alpha, 10⁴–10⁵ per C knock-on). The upstream
//!   cutoffs `Ec` (1 eV for C, 3 eV for U) are far below displacement
//!   energies, so nearly every collision that moves a target atom by a few eV
//!   starts a followed recoil. Fragments cost 1.5–2.0 s per cascade (the
//!   0.1–2 s band, at its top); alphas 28–64 ms and knock-ons 29–109 ms, above
//!   the 5–50 and 1–20 ms bands. `Ec` was not changed: it is upstream's
//!   per-element value and the cost knob, and changing it is a choice for the
//!   maintainer, not a tuning of this run.
//! * **Free-flight arm: the cost prediction is REFUTED.** It was predicted
//!   ≥ 3× cheaper; it is **1.5× MORE expensive** (2.27 vs 1.47 s light, 3.09
//!   vs 2.00 s heavy) with about as many recoils. Ranges agree with the
//!   default arm (light −0.10 ± 0.22 µm, heavy −0.07 ± 0.15 µm), so it buys
//!   nothing here. A hypothesis, not measured: the option applies to every
//!   recoil too, and for each step it first builds a `Vec` of per-species
//!   stopping powers before falling back to the mean free path.
//! * **For the browser demo:** a full fragment cascade is seconds of CPU on
//!   this desktop, so a demo cannot redo one per fission event on the UI
//!   thread. Primary tracks alone (`track_recoils = false`), a higher `Ec`
//!   (the maintainer's call), or precomputed cascades are the options.
//!   Not measured here: primary-only cost, and anything in a browser.

use std::time::Instant;

use boon_lay::rustbca::{
    bca, geometry, material, particle, ElectronicStoppingMode, Geometry, Mesh1D, Options, AMU,
    ANGSTROM, EV, MICRON,
};
use outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};
use outram_mc_libs::perf_report::HardwareInfo;
use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

// ---------------------------------------------------------------------------
// Evaluated-data inputs (ENDF/B-VIII.0, reference-data/endf/).
// ---------------------------------------------------------------------------

/// U-235 (MAT 9228) MF1/MT458 `EFR`, fragment kinetic energy, E = 0 term [eV].
const U235_EFR_EV: f64 = 1.691300e8;
/// U-235 MF1/MT458 `ENP`, prompt-neutron kinetic energy per fission [eV].
const U235_ENP_EV: f64 = 4.827645e6;
/// U-235 MF1/MT456 prompt ν̄ at 1e-5 eV.
const U235_NUBAR_PROMPT_THERMAL: f64 = 2.414;
/// B-10 (MAT 525) MF3/MT801 `QI`, (n,α) to Li-7 first excited state [eV].
const B10_MT801_QI_EV: f64 = 2.311472e6;

/// Atomic masses [u], AME2020.
const M_NEUTRON: f64 = 1.008_664_92;
const M_HE4: f64 = 4.002_603_25;
const M_LI7: f64 = 7.016_003_44;
const M_C12: f64 = 12.0;
const M_O16: f64 = 15.994_914_62;

/// Representative fission-fragment pair (peak masses; Z summing to 92).
const LIGHT_A: f64 = 95.0;
const LIGHT_Z: f64 = 38.0;
const HEAVY_A: f64 = 139.0;
const HEAVY_Z: f64 = 54.0;

/// Light and heavy fragment kinetic energies [eV] from `EFR` by momentum
/// conservation between the two fragments.
fn fragment_energies_ev() -> (f64, f64) {
    let total = LIGHT_A + HEAVY_A;
    (U235_EFR_EV * HEAVY_A / total, U235_EFR_EV * LIGHT_A / total)
}

/// B-10(n,α)Li-7* alpha energy [eV] for a thermal neutron (momentum neglected).
fn alpha_energy_ev() -> f64 {
    B10_MT801_QI_EV * M_LI7 / (M_LI7 + M_HE4)
}

/// Mean prompt fission-neutron energy of U-235 [eV], `ENP / ν̄_p`.
fn mean_prompt_neutron_energy_ev() -> f64 {
    U235_ENP_EV / U235_NUBAR_PROMPT_THERMAL
}

/// Mean elastic-recoil energy [eV] of a nucleus of mass `m_u` struck by a
/// neutron of energy `e_n_ev`, isotropic in the centre of mass.
fn mean_elastic_recoil_ev(e_n_ev: f64, m_u: f64) -> f64 {
    let a = m_u / M_NEUTRON;
    e_n_ev * 2.0 * a / ((a + 1.0) * (a + 1.0))
}

// ---------------------------------------------------------------------------
// Target species and the unrolled TRISO stack.
// ---------------------------------------------------------------------------

/// A RustBCA target species: atomic number, mass [u], and upstream's
/// `scripts/materials.py` surface binding `Es`, bulk binding `Eb` and cutoff
/// `Ec` [eV] for its element.
#[derive(Clone, Copy, Debug)]
struct Species {
    name: &'static str,
    z: f64,
    m_u: f64,
    es: f64,
    eb: f64,
    ec: f64,
}

const U_ES: f64 = 5.19;
const U_EC: f64 = 3.0;
const O_ES: f64 = 2.58;
const O_EB: f64 = 2.58;
const O_EC: f64 = 2.0;
const C_ES: f64 = 7.37;
const C_EC: f64 = 1.0;
const SI_ES: f64 = 4.72;
const SI_EC: f64 = 1.5;
const B_ES: f64 = 5.77;
const B_EC: f64 = 5.0;

/// The species of `fuel_pebble_materials(Htr10Nuclides::ELEMENTAL_CARBON, ..)`,
/// indexed by its nuclide slot (slot 3, free carbon, is not used by the fuel
/// materials and is mapped to carbon for completeness).
fn species_for_slot(slot: usize) -> Species {
    let n = Htr10Nuclides::ELEMENTAL_CARBON;
    let carbon = Species { name: "C", z: 6.0, m_u: 12.011, es: C_ES, eb: 0.0, ec: C_EC };
    match slot {
        s if s == n.u235 => Species { name: "U-235", z: 92.0, m_u: 235.043_930, es: U_ES, eb: 0.0, ec: U_EC },
        s if s == n.u238 => Species { name: "U-238", z: 92.0, m_u: 238.050_788, es: U_ES, eb: 0.0, ec: U_EC },
        s if s == n.o16 => Species { name: "O-16", z: 8.0, m_u: M_O16, es: O_ES, eb: O_EB, ec: O_EC },
        s if n.c_graphite.contains(s) || n.c_sic.contains(s) || n.c_free.contains(s) => carbon,
        s if s == n.si28 => Species { name: "Si-28", z: 14.0, m_u: 27.976_926_5, es: SI_ES, eb: 0.0, ec: SI_EC },
        s if s == n.si29 => Species { name: "Si-29", z: 14.0, m_u: 28.976_494_7, es: SI_ES, eb: 0.0, ec: SI_EC },
        s if s == n.si30 => Species { name: "Si-30", z: 14.0, m_u: 29.973_770_1, es: SI_ES, eb: 0.0, ec: SI_EC },
        s if s == n.b10 => Species { name: "B-10", z: 5.0, m_u: 10.012_937, es: B_ES, eb: 0.0, ec: B_EC },
        s if s == n.b11 => Species { name: "B-11", z: 5.0, m_u: 11.009_305, es: B_ES, eb: 0.0, ec: B_EC },
        other => panic!("no RustBCA species for HTR-10 nuclide slot {other}"),
    }
}

/// Number of nuclide slots in `Htr10Nuclides::ELEMENTAL_CARBON` (0..=10).
const N_SLOTS: usize = 11;

/// One planar layer of the unrolled stack: name, thickness [µm] and atom
/// density per nuclide slot [atoms/Å³ = atoms/(b·cm)].
#[derive(Clone, Debug)]
struct Layer {
    name: &'static str,
    /// What the solver can tell apart: IPyC and OPyC are the same material.
    composition: &'static str,
    thickness_um: f64,
    densities: Vec<f64>,
}

/// The TRISO layers in UNROLLED order, outermost first (x = 0 is the OPyC
/// surface): OPyC, SiC, IPyC, buffer, kernel (its radius). Thicknesses from
/// `TrisoSpec::HTR10_LI2014`, compositions from `fuel_pebble_materials`.
fn unrolled_triso_stack() -> Vec<Layer> {
    let spec = TrisoSpec::HTR10_LI2014;
    let um = |cm: f64| cm * 1.0e4;
    let mats = fuel_pebble_materials(Htr10Nuclides::ELEMENTAL_CARBON, BoronReading::Natural, 300.15);
    let densities = |i: usize| {
        let mut d = vec![0.0; N_SLOTS];
        for c in &mats[i].components {
            d[c.nuclide_idx] += c.atom_density;
        }
        d
    };
    // fuel_pebble_materials order: 0 kernel, 1 buffer, 2 IPyC, 3 SiC, 4 OPyC.
    vec![
        Layer { name: "OPyC", composition: "PyC 1.9 g/cm3", thickness_um: um(spec.opyc - spec.sic), densities: densities(4) },
        Layer { name: "SiC", composition: "SiC 3.18 g/cm3", thickness_um: um(spec.sic - spec.ipyc), densities: densities(3) },
        Layer { name: "IPyC", composition: "PyC 1.9 g/cm3", thickness_um: um(spec.ipyc - spec.buffer), densities: densities(2) },
        Layer { name: "buffer", composition: "buffer PyC 1.1 g/cm3", thickness_um: um(spec.buffer - spec.kernel), densities: densities(1) },
        Layer { name: "UO2 kernel", composition: "UO2 10.4 g/cm3", thickness_um: um(spec.kernel), densities: densities(0) },
    ]
}

/// The RustBCA material: the unrolled stack as a `Mesh1D` over all species.
fn build_material(stack: &[Layer]) -> material::Material<Mesh1D> {
    let species: Vec<Species> = (0..N_SLOTS).map(species_for_slot).collect();
    let material_parameters = material::MaterialParameters {
        energy_unit: "EV".to_string(),
        mass_unit: "AMU".to_string(),
        Eb: species.iter().map(|s| s.eb).collect(),
        Es: species.iter().map(|s| s.es).collect(),
        Ec: species.iter().map(|s| s.ec).collect(),
        Ed: vec![0.0; N_SLOTS],
        Z: species.iter().map(|s| s.z).collect(),
        m: species.iter().map(|s| s.m_u).collect(),
        interaction_index: vec![0; N_SLOTS],
        surface_binding_model: boon_lay::rustbca::SurfaceBindingModel::AVERAGE,
        bulk_binding_model: boon_lay::rustbca::BulkBindingModel::INDIVIDUAL,
    };
    let geometry_input = geometry::Mesh1DInput {
        length_unit: "ANGSTROM".to_string(),
        layer_thicknesses: stack.iter().map(|l| l.thickness_um * MICRON / ANGSTROM).collect(),
        densities: stack.iter().map(|l| l.densities.clone()).collect(),
        electronic_stopping_correction_factors: vec![1.0; stack.len()],
    };
    material::Material::<Mesh1D>::new(&material_parameters, &geometry_input)
}

/// Which composition the ASSEMBLED geometry puts at depth `x_um`, as the index
/// of the FIRST stack layer whose density vector matches what `Mesh1D` returns
/// there (IPyC and OPyC share one composition, so IPyC reads as index 0), or
/// `None` outside the stack.
fn layer_at(material: &material::Material<Mesh1D>, stack: &[Layer], x_um: f64) -> Option<usize> {
    let x = x_um * MICRON;
    if !material.geometry.inside(x, 0.0, 0.0) {
        return None;
    }
    // Mesh1D stores densities in 1/m³; the stack holds 1/Å³.
    let got = material.geometry.get_densities(x, 0.0, 0.0);
    stack.iter().position(|l| {
        l.densities.iter().zip(got).all(|(&want, &have)| (want / (ANGSTROM * ANGSTROM * ANGSTROM) - have).abs() <= 1e-12 * have.abs().max(1.0))
    })
}

/// Interfaces [µm] recovered from the assembled geometry by scanning
/// `layer_at` every `step_um`: the first depth at which each new layer is seen.
fn recovered_interfaces_um(material: &material::Material<Mesh1D>, stack: &[Layer], step_um: f64) -> Vec<(f64, Option<usize>)> {
    let total: f64 = stack.iter().map(|l| l.thickness_um).sum();
    let mut out = Vec::new();
    let mut last: Option<Option<usize>> = None;
    let mut x = -5.0;
    while x <= total + 5.0 {
        let here = layer_at(material, stack, x);
        if last != Some(here) {
            out.push((x, here));
            last = Some(here);
        }
        x += step_um;
    }
    out
}

// ---------------------------------------------------------------------------
// Cases and the measurement.
// ---------------------------------------------------------------------------

/// One measured case: a projectile born at depth `x_um` in the unrolled stack.
#[derive(Clone, Debug)]
struct Case {
    label: &'static str,
    z: f64,
    m_u: f64,
    energy_ev: f64,
    ec_ev: f64,
    es_ev: f64,
    x_um: f64,
    n: usize,
    free_flight: bool,
    illustrative: bool,
}

/// What one case produced.
#[derive(Clone, Debug)]
struct CaseResult {
    label: &'static str,
    n: usize,
    range_um: (f64, f64),
    projected_um: (f64, f64),
    time_s: (f64, f64),
    recoils: (f64, f64),
    /// Fraction of primaries that came to rest shallower than their birth
    /// layer's outer face (for the surface-born fragments: left the kernel).
    left_birth_layer: f64,
    deterministic: bool,
}

/// Mean and sample standard deviation.
fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    let var = if v.len() > 1 { v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0) } else { 0.0 };
    (mean, var.sqrt())
}

/// An isotropic unit direction from two uniforms.
fn isotropic_direction(rng: &mut ChaCha8Rng) -> (f64, f64, f64) {
    let mu = 2.0 * rng.random::<f64>() - 1.0;
    let phi = 2.0 * core::f64::consts::PI * rng.random::<f64>();
    let s = (1.0 - mu * mu).max(0.0).sqrt();
    (mu, s * petir::real::cos(phi), s * petir::real::sin(phi))
}

const BASE_SEED: u64 = 876;

/// Runs cascade `index` of `case`: returns (final primary, number of tracked
/// recoils, birth direction, all output particles' state bits).
fn run_one(case: &Case, case_id: u64, index: usize, material: &material::Material<Mesh1D>)
    -> (particle::Particle, usize, (f64, f64, f64), Vec<[u64; 6]>, f64) {
    let mut options = Options::default_options(true);
    options.electronic_stopping_mode = ElectronicStoppingMode::INTERPOLATED;
    options.high_energy_free_flight_paths = case.free_flight;
    let mut rng = ChaCha8Rng::seed_from_u64(BASE_SEED);
    rng.set_stream(case_id * 1_000_000 + index as u64);
    let dir = isotropic_direction(&mut rng);
    let primary = particle::Particle::new(case.m_u * AMU, case.z, case.energy_ev * EV, case.ec_ev * EV,
        case.es_ev * EV, 0.0, case.x_um * MICRON, 0.0, 0.0, dir.0, dir.1, dir.2, true, false, 0);
    let t0 = Instant::now();
    let out = bca::single_ion_bca(primary, material, &options, &mut rng);
    let dt = t0.elapsed().as_secs_f64();
    let bits = out.iter().map(|p| [p.E.to_bits(), p.pos.x.to_bits(), p.pos.y.to_bits(), p.pos.z.to_bits(),
        p.Z.to_bits(), p.m.to_bits()]).collect();
    // single_ion_bca pushes each particle when it finishes; the primary is
    // the only `incident` one.
    let primary_out = out.iter().find(|p| p.incident).expect("primary missing from output").clone();
    let recoils = out.len() - 1;
    (primary_out, recoils, dir, bits, dt)
}

/// Depth [µm] of the outer face of the layer containing `x_um`.
fn outer_face_um(stack: &[Layer], x_um: f64) -> f64 {
    let mut top = 0.0;
    for l in stack {
        if x_um < top + l.thickness_um {
            return top;
        }
        top += l.thickness_um;
    }
    top
}

/// Runs `case.n` cascades and summarises them.
fn run_case(case: &Case, case_id: u64, material: &material::Material<Mesh1D>, stack: &[Layer]) -> CaseResult {
    let face = outer_face_um(stack, case.x_um);
    let (mut ranges, mut projected, mut times, mut recoils) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut left = 0usize;
    let mut first_bits = Vec::new();
    for i in 0..case.n {
        let (p, nrec, dir, bits, dt) = run_one(case, case_id, i, material);
        if i == 0 {
            first_bits = bits;
        }
        let d = (p.pos.x - p.pos_origin.x, p.pos.y - p.pos_origin.y, p.pos.z - p.pos_origin.z);
        ranges.push((d.0 * d.0 + d.1 * d.1 + d.2 * d.2).sqrt() / MICRON);
        projected.push((d.0 * dir.0 + d.1 * dir.1 + d.2 * dir.2) / MICRON);
        times.push(dt);
        recoils.push(nrec as f64);
        if p.pos.x / MICRON < face {
            left += 1;
        }
    }
    let (_, _, _, again, _) = run_one(case, case_id, 0, material);
    let sem = |v: &[f64]| {
        let (m, s) = mean_sd(v);
        (m, s / (v.len() as f64).sqrt())
    };
    CaseResult {
        label: case.label,
        n: case.n,
        range_um: sem(&ranges),
        projected_um: sem(&projected),
        time_s: mean_sd(&times),
        recoils: mean_sd(&recoils),
        left_birth_layer: left as f64 / case.n as f64,
        deterministic: again == first_bits,
    }
}

/// The measured cases. `scale` multiplies every cascade count (at least 1).
fn cases(scale: f64) -> Vec<Case> {
    let (e_light, e_heavy) = fragment_energies_ev();
    let e_alpha = alpha_energy_ev();
    let e_n = mean_prompt_neutron_energy_ev();
    let e_c = mean_elastic_recoil_ev(e_n, M_C12);
    let e_o = mean_elastic_recoil_ev(e_n, M_O16);
    let n = |k: usize| ((k as f64 * scale).round() as usize).max(1);
    let frag = |label, z, m, e, x, ff| Case { label, z, m_u: m, energy_ev: e, ec_ev: 1.0, es_ev: 0.0, x_um: x, n: n(16), free_flight: ff, illustrative: true };
    vec![
        frag("light fragment Sr-95, kernel middle", LIGHT_Z, LIGHT_A, e_light, 330.0, false),
        frag("heavy fragment Xe-139, kernel middle", HEAVY_Z, HEAVY_A, e_heavy, 330.0, false),
        frag("light fragment Sr-95, 2 um inside kernel surface", LIGHT_Z, LIGHT_A, e_light, 207.0, false),
        frag("light fragment, kernel middle, FREE-FLIGHT arm", LIGHT_Z, LIGHT_A, e_light, 330.0, true),
        frag("heavy fragment, kernel middle, FREE-FLIGHT arm", HEAVY_Z, HEAVY_A, e_heavy, 330.0, true),
        Case { label: "alpha B-10(n,a), kernel middle", z: 2.0, m_u: M_HE4, energy_ev: e_alpha, ec_ev: 1.0, es_ev: 0.0, x_um: 330.0, n: n(64), free_flight: false, illustrative: false },
        Case { label: "alpha B-10(n,a), buffer middle", z: 2.0, m_u: M_HE4, energy_ev: e_alpha, ec_ev: 1.0, es_ev: 0.0, x_um: 160.0, n: n(64), free_flight: false, illustrative: false },
        Case { label: "O-16 knock-on, kernel middle", z: 8.0, m_u: M_O16, energy_ev: e_o, ec_ev: O_EC, es_ev: O_ES, x_um: 330.0, n: n(64), free_flight: false, illustrative: false },
        Case { label: "C-12 knock-on, buffer middle", z: 6.0, m_u: M_C12, energy_ev: e_c, ec_ev: C_EC, es_ev: C_ES, x_um: 160.0, n: n(64), free_flight: false, illustrative: false },
        Case { label: "C-12 knock-on, IPyC middle", z: 6.0, m_u: M_C12, energy_ev: e_c, ec_ev: C_EC, es_ev: C_ES, x_um: 95.0, n: n(64), free_flight: false, illustrative: false },
        Case { label: "C-12 knock-on, SiC middle", z: 6.0, m_u: M_C12, energy_ev: e_c, ec_ev: C_EC, es_ev: C_ES, x_um: 57.5, n: n(64), free_flight: false, illustrative: false },
    ]
}

// ---------------------------------------------------------------------------
// The geometry picture (drawn from the assembled Mesh1D).
// ---------------------------------------------------------------------------

/// An SVG strip of the unrolled stack, coloured by the layer the assembled
/// `Mesh1D` reports every 0.5 µm, with interface depths, a legend and the
/// birth points of `cases`.
fn stack_svg(material: &material::Material<Mesh1D>, stack: &[Layer], cases: &[Case]) -> String {
    let colours = ["#9aa7b0", "#d9a441", "#6f8fb3", "#c9b28a", "#b5483b"];
    let total: f64 = stack.iter().map(|l| l.thickness_um).sum();
    let (x0, px_per_um, top, h) = (40.0, 2.0, 70.0, 60.0);
    let mut s = String::new();
    let width = x0 * 2.0 + (total + 10.0) * px_per_um + 160.0;
    s.push_str(&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"360\" font-family=\"sans-serif\" font-size=\"11\">\n"));
    s.push_str("<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n");
    s.push_str(&format!("<text x=\"{x0}\" y=\"20\" font-size=\"13\">HTR-10 TRISO, unrolled into RustBCA Mesh1D (drawn from density lookups every 0.5 um; x = 0 is the OPyC surface, x = {total:.0} um the kernel centre)</text>\n"));
    let step = 0.5;
    let mut x = -5.0;
    while x < total + 5.0 {
        let fill = match layer_at(material, stack, x + step / 2.0) {
            Some(i) => colours[i % colours.len()],
            None => "#ffffff",
        };
        let px = x0 + (x + 5.0) * px_per_um;
        s.push_str(&format!("<rect x=\"{px:.1}\" y=\"{top}\" width=\"{:.1}\" height=\"{h}\" fill=\"{fill}\"/>\n", step * px_per_um + 0.2));
        x += step;
    }
    s.push_str(&format!("<rect x=\"{x0}\" y=\"{top}\" width=\"{:.1}\" height=\"{h}\" fill=\"none\" stroke=\"black\"/>\n", (total + 10.0) * px_per_um));
    for (xi, layer) in recovered_interfaces_um(material, stack, 0.5).into_iter().skip(1) {
        let px = x0 + (xi + 5.0) * px_per_um;
        let name = layer.map_or("outside", |i| stack[i].composition);
        s.push_str(&format!("<line x1=\"{px:.1}\" y1=\"{}\" x2=\"{px:.1}\" y2=\"{}\" stroke=\"black\"/>\n", top + h, top + h + 8.0));
        s.push_str(&format!("<text x=\"{px:.1}\" y=\"{}\" transform=\"rotate(45 {px:.1} {})\">{xi:.1} um: {name}</text>\n", top + h + 18.0, top + h + 18.0));
    }
    for (k, c) in cases.iter().enumerate() {
        let px = x0 + (c.x_um + 5.0) * px_per_um;
        let y = top - 4.0;
        s.push_str(&format!("<path d=\"M {px:.1} {y:.1} l -3 -5 l 6 0 z\" fill=\"black\"><title>{}</title></path>\n", c.label));
        if k == 0 || cases[..k].iter().all(|o| o.x_um != c.x_um) {
            s.push_str(&format!("<text x=\"{px:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{} um</text>\n", y - 12.0, c.x_um));
        }
    }
    for (i, l) in stack.iter().enumerate() {
        let lx = x0 + i as f64 * 150.0;
        // The colour of the composition the solver sees for this layer.
        let ci = stack.iter().position(|o| o.densities == l.densities).unwrap_or(i);
        s.push_str(&format!("<rect x=\"{lx}\" y=\"315\" width=\"14\" height=\"14\" fill=\"{}\" stroke=\"black\"/>", colours[ci % colours.len()]));
        s.push_str(&format!("<text x=\"{}\" y=\"326\">{} ({:.0} um)</text>\n", lx + 18.0, l.name, l.thickness_um));
    }
    s.push_str(&format!("<text x=\"{x0}\" y=\"350\">Triangles above the strip: birth depths of the measured cases (hover for the label).</text>\n"));
    s.push_str("</svg>\n");
    s
}

// `main` is CLI glue (environment knob, printing, writing the picture); the
// work it calls is reached by the tests at the bottom of this file.
fn main() {
    let scale: f64 = std::env::var("RUSTBCA_CASCADES_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let stack = unrolled_triso_stack();
    let material = build_material(&stack);
    let cases = cases(scale);

    println!("Hardware: {}; 1 thread used.", HardwareInfo::detect().headline());
    println!("Target species (Es / Eb / Ec in eV, upstream scripts/materials.py):");
    for slot in 0..N_SLOTS {
        let s = species_for_slot(slot);
        println!("  slot {slot:2}: {:6} Z {:3} m {:9.5} u  {:.2} / {:.2} / {:.1}", s.name, s.z, s.m_u, s.es, s.eb, s.ec);
    }
    println!("Interfaces recovered from the assembled Mesh1D (0.5 um scan):");
    for (x, layer) in recovered_interfaces_um(&material, &stack, 0.5) {
        println!("  from {x:7.1} um: {}", layer.map_or("outside", |i| stack[i].composition));
    }
    let (el, eh) = fragment_energies_ev();
    println!("Birth energies: light fragment {:.2} MeV, heavy {:.2} MeV, alpha {:.4} MeV, C-12 {:.1} keV, O-16 {:.1} keV (E_n {:.5} MeV)",
        el / 1e6, eh / 1e6, alpha_energy_ev() / 1e6,
        mean_elastic_recoil_ev(mean_prompt_neutron_energy_ev(), M_C12) / 1e3,
        mean_elastic_recoil_ev(mean_prompt_neutron_energy_ev(), M_O16) / 1e3,
        mean_prompt_neutron_energy_ev() / 1e6);

    let svg_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/verification_and_validation/rustbca_cascades");
    let svg_path = format!("{svg_dir}/triso_stack_mesh1d.svg");
    match std::fs::create_dir_all(svg_dir).and_then(|_| std::fs::write(&svg_path, stack_svg(&material, &stack, &cases))) {
        Ok(()) => println!("Geometry picture: {svg_path}"),
        Err(e) => println!("Geometry picture NOT written ({e})"),
    }

    println!();
    println!("| case | n | range um (mean +- sem) | projected um | time per cascade s (mean, sd) | tracked recoils (mean, sd) | ended above birth layer | deterministic |");
    println!("|---|---|---|---|---|---|---|---|");
    let t_all = Instant::now();
    for (id, case) in cases.iter().enumerate() {
        let r = run_case(case, id as u64, &material, &stack);
        assert!(r.deterministic, "{}: re-running cascade 0 with its seed gave a different result", r.label);
        println!("| {}{} | {} | {:.3} +- {:.3} | {:.3} +- {:.3} | {:.3e}, {:.1e} | {:.0}, {:.0} | {:.2} | {} |",
            r.label, if case.illustrative { " (ILLUSTRATIVE)" } else { "" }, r.n,
            r.range_um.0, r.range_um.1, r.projected_um.0, r.projected_um.1,
            r.time_s.0, r.time_s.1, r.recoils.0, r.recoils.1, r.left_birth_layer, r.deterministic);
    }
    println!();
    println!("Total wall time {:.1} s.", t_all.elapsed().as_secs_f64());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The derived birth energies, against hand arithmetic on the cited values.
    #[test]
    fn birth_energies_follow_from_the_cited_data() {
        let (l, h) = fragment_energies_ev();
        assert!((l + h - U235_EFR_EV).abs() < 1e-6, "energies must sum to EFR");
        assert!((l / 1e6 - 100.4661).abs() < 1e-3, "light {l}");
        assert!((h / 1e6 - 68.6639).abs() < 1e-3, "heavy {h}");
        assert!((alpha_energy_ev() / 1e6 - 1.47183).abs() < 1e-4, "alpha {}", alpha_energy_ev());
        assert!((mean_prompt_neutron_energy_ev() / 1e6 - 1.99985).abs() < 1e-4);
        // Isotropic-CM mean recoil: E_n 2A/(A+1)^2.
        let c = mean_elastic_recoil_ev(2.0e6, M_C12);
        assert!((c / 1e3 - 286.13).abs() < 0.1, "C-12 {c}");
    }

    /// The cited ENDF values match the tapes, where the tapes are present.
    #[test]
    fn endf_values_match_the_tapes_when_present() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../reference-data/endf/");
        if let Ok(u235) = std::fs::read_to_string(format!("{root}n-092_U_235-ENDF8.0.endf")) {
            assert!(u235.lines().any(|l| l.starts_with(" 1.691300+8 0.000000+0 4.827645+6") && l.ends_with("9228 1458")));
            assert!(u235.lines().any(|l| l.starts_with(" 1.000000-5 2.414000+0") && l.ends_with("9228 1456")));
        }
        if let Ok(b10) = std::fs::read_to_string(format!("{root}n-005_B_010-ENDF8.0.endf")) {
            assert!(b10.lines().any(|l| l.starts_with(" 2.789323+6 2.311472+6") && l.contains(" 525 3801")));
        }
    }

    /// The assembled Mesh1D puts each layer where TrisoSpec says, to the scan
    /// step, and in the right order, with nothing outside 0..455 um.
    #[test]
    fn assembled_stack_reproduces_the_triso_radii() {
        let stack = unrolled_triso_stack();
        let material = build_material(&stack);
        let found = recovered_interfaces_um(&material, &stack, 0.5);
        let layers: Vec<Option<usize>> = found.iter().map(|f| f.1).collect();
        // IPyC is the same material as OPyC, so it reads as layer 0.
        assert_eq!(layers, vec![None, Some(0), Some(1), Some(0), Some(3), Some(4), None]);
        let expect = [0.0, 40.0, 75.0, 115.0, 205.0, 455.0];
        for (k, want) in expect.iter().enumerate() {
            let got = found[k + 1].0;
            assert!(got > *want - 1e-9 && got <= want + 0.5 + 1e-9, "interface {k}: {got} vs {want}");
        }
        assert!((outer_face_um(&stack, 207.0) - 205.0).abs() < 1e-9);
        assert!((outer_face_um(&stack, 57.5) - 40.0).abs() < 1e-9);
        // The kernel is UO2: U and O only (plus ppm boron), O/U = 2.
        let k = &stack[4].densities;
        let n = Htr10Nuclides::ELEMENTAL_CARBON;
        assert!(((k[n.o16] / (k[n.u235] + k[n.u238])) - 2.0).abs() < 1e-12);
        assert!(stack_svg(&material, &stack, &cases(0.01)).contains("UO2 kernel"));
    }

    /// Directions are unit vectors and a short alpha cascade runs, is
    /// deterministic, and stops inside its birth layer.
    #[test]
    fn a_small_case_runs_and_is_deterministic() {
        let stack = unrolled_triso_stack();
        let material = build_material(&stack);
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        for _ in 0..100 {
            let d = isotropic_direction(&mut rng);
            assert!((d.0 * d.0 + d.1 * d.1 + d.2 * d.2 - 1.0).abs() < 1e-12);
        }
        let case = Case { label: "test alpha", z: 2.0, m_u: M_HE4, energy_ev: 100.0e3, ec_ev: 1.0, es_ev: 0.0,
            x_um: 330.0, n: 3, free_flight: false, illustrative: false };
        let r = run_case(&case, 99, &material, &stack);
        assert!(r.deterministic);
        assert!(r.range_um.0 > 0.05 && r.range_um.0 < 2.0, "100 keV alpha range {} um", r.range_um.0);
        assert_eq!(r.left_birth_layer, 0.0);
        assert!(mean_sd(&[1.0, 3.0]) == (2.0, 2.0_f64.sqrt()));
        assert_eq!(cases(1.0).len(), 11);
    }
}
