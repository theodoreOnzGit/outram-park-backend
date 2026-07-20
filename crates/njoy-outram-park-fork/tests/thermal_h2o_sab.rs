//! V&V: H-in-H₂O incoherent-inelastic thermal scattering, S(α,β) → σ(E).
//!
//! # What this validates
//!
//! The THERMR incoherent-inelastic path for **H in light water** at 293.6 K,
//! generated from the ENDF/B-VIII.0 thermal sublibrary evaluation
//! `tsl-HinH2O.endf` (MAT = 1), through the
//! [`IncoherentInelasticScattering`](njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering)
//! consumer surface. The four checks are analytic/limiting — no ACE oracle is
//! required, so they are reproducible on any machine that has the (open-source)
//! ENDF file.
//!
//! ## Data / provenance
//!
//! - **Evaluation:** ENDF/B-VIII.0 thermal scattering law, `tsl-HinH2O.endf`,
//!   MAT 1, LAT = 1, LASYM = 0, `B(1) = 40.872`, `A = B(3) = 0.99917`,
//!   `natom = B(6) = 2`. Open-source (ENDF/B-VIII.0, 2018), per `DATA_POLICY.md`.
//! - **Kernel:** port of NJOY2016 (release 2016.79, commit `ac5adf5f`)
//!   `thermr.f90` — `sig`/`calcem` double-differential + SCT tail.
//! - The 17.4 MB ENDF file is **not** checked in; the test reads it from
//!   `HINH2O_TSL` (env override) or the default absolute path below, and
//!   **skips** (prints a note, passes) when the file is absent so CI stays green.
//!
//! # Methodology and measured results (2026-07-15, ENDF/B-VIII.0)
//!
//! Temperature selection: request 293.6 K → the evaluation tabulates S(α,β) at
//! exactly 293.6 K (base T₀ is 283.6 K), and the parser selects it. **Measured:
//! selected T = 293.600 K.**
//!
//! 1. **Free-atom high-E limit.** As E rises above the ~0.5 eV phonon spectrum,
//!    the bound σ_inel(E) must approach the free-atom cross section
//!    σ_free = B(1)/natom = 40.872/2 = **20.436 b/H**, from the SCT tail (a pure
//!    tabulated-S kernel falls to ~2 b at 8 eV — wrong). Pass: within 5 % at
//!    8 eV and monotone-decreasing 1→8 eV. **Measured: σ_inel = 20.707 b/H at
//!    8 eV (+1.33 %); 21.713 → 20.951 → 20.707 b decreasing over 1→4→8 eV.**
//!
//! 2. **Thermal cross section.** σ_inel(0.0253 eV) per H × 2 H = the bound H₂O
//!    scattering cross section per molecule. Literature: the bound thermal
//!    scattering cross section of light water at 2200 m/s (0.0253 eV) is
//!    ≈ 103 b/molecule (2·σ_bound(H) dominated; H free 20.4 b → bound ~81.8 b,
//!    plus Maxwellian upscatter). Pass: 90–115 b/molecule. **Measured:
//!    52.10 b/H → 104.2 b/molecule (+1.2 % vs 103 b).**
//!
//! 3. **Detailed balance of the double-differential kernel.** For any μ and any
//!    two energies E, E' inside the tabulated grid,
//!    `d²σ(E→E',μ) / d²σ(E'→E,μ) = (E'/E)·exp(−(E'−E)/kT)` (the exp(−β/2)
//!    asymmetry factor + the √(E'/E) kinematic factor). Pass: relative error
//!    < 1 %. **Measured (E=0.05, E'=0.03 eV, μ=0.3): ratio 1.322697 vs
//!    analytic 1.322697, rel. err 1.7e-16 — machine-exact.**
//!
//! 4. **Effective temperature.** T_eff(293.6 K) from the MF=7/MT=4 table must
//!    exceed the physical T (bound-atom zero-point motion) and sit in the known
//!    H-in-H₂O band (~1100–1300 K). Pass: 293.6 < T_eff < 2000 K. **Measured:
//!    T_eff = 1194.3 K.**
//!
//! # Interpretation
//!
//! The free-gas limit (1) and detailed balance (3) together exercise the full
//! S(α,β) integration — the top human-review ask. Limit (1) is the decisive
//! evidence that the SCT tail is wired correctly: without it σ(E) collapses at
//! high E. The thermal value (2) agrees with the literature bound cross section
//! to ~1 %. These are *limiting/analytic* checks, not a full pointwise
//! comparison against an NJOY ACE oracle — see the REVIEW_MANIFEST for the
//! human-verify list.

use njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering;
use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
use uom::si::{area::barn, energy::electronvolt, thermodynamic_temperature::kelvin};

const DEFAULT_PATH: &str = "/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt/tsl-HinH2O.endf";
const HINH2O_MAT: i32 = 1;
const BK_EV_PER_K: f64 = 8.617_333_262e-5;

/// Resolve the ENDF file path (env override → default) and load at 293.6 K, or
/// return `None` (with a printed skip note) when the file is not present.
fn load() -> Option<IncoherentInelasticScattering> {
    let path = std::env::var("HINH2O_TSL").unwrap_or_else(|_| DEFAULT_PATH.to_string());
    if !std::path::Path::new(&path).exists() {
        eprintln!(
            "SKIP thermal_h2o_sab: tsl-HinH2O.endf not found at {path} \
             (set HINH2O_TSL to the ENDF/B-VIII.0 file to run this V&V)"
        );
        return None;
    }
    let t = Temperature::new::<kelvin>(293.6);
    Some(
        IncoherentInelasticScattering::from_endf_file(&path, HINH2O_MAT, t)
            .expect("parse tsl-HinH2O.endf MF=7/MT=4"),
    )
}

#[test]
fn selects_the_293k_scattering_law() {
    let Some(sab) = load() else { return };
    let sel = sab.selected_temperature().get::<kelvin>();
    assert!((sel - 293.6).abs() < 0.1, "nearest tabulated T should be 293.6 K, got {sel}");
    assert_eq!(sab.principal_atom_count(), 2.0, "H₂O has 2 principal H atoms (B(6))");
    assert!((sab.mass_ratio() - 0.99917).abs() < 1e-3, "A = B(3) ≈ 0.99917 for H");
}

#[test]
fn approaches_free_atom_cross_section_at_high_energy() {
    let Some(sab) = load() else { return };
    let sigma_free = sab.free_cross_section().get::<barn>();
    assert!((sigma_free - 20.436).abs() < 0.01, "σ_free = B(1)/2 = 20.436 b/H, got {sigma_free}");

    let xs = |e_ev: f64| {
        sab.inelastic_xs(NeutronEnergy::new::<electronvolt>(e_ev)).get::<barn>()
    };
    let (s1, s4, s8) = (xs(1.0), xs(4.0), xs(8.0));
    // Monotone decreasing toward the free-atom limit from above.
    assert!(s1 > s4 && s4 > s8, "σ_inel decreases 1→4→8 eV toward σ_free: {s1} {s4} {s8}");
    let rel = (s8 - sigma_free).abs() / sigma_free;
    assert!(rel < 0.05, "σ_inel(8 eV)={s8} within 5% of σ_free={sigma_free} (rel {rel:.4})");
}

#[test]
fn thermal_cross_section_matches_bound_water() {
    let Some(sab) = load() else { return };
    let per_h = sab.inelastic_xs(NeutronEnergy::new::<electronvolt>(0.0253)).get::<barn>();
    let per_molecule = 2.0 * per_h; // 2 H per H₂O
    // Literature bound scattering cross section of light water at 0.0253 eV
    // ≈ 103 b/molecule.
    assert!(
        (90.0..=115.0).contains(&per_molecule),
        "σ_s(H₂O, 0.0253 eV) = {per_molecule} b/molecule (per-H {per_h}); expect ≈103 b"
    );
}

#[test]
fn double_differential_obeys_detailed_balance() {
    let Some(sab) = load() else { return };
    let ii = sab.kernel();
    let t = sab.selected_temperature().get::<kelvin>();
    let kt = BK_EV_PER_K * t;
    // Two energies well inside the tabulated (α,β) grid, and a mid cosine.
    let (e, ep, mu) = (0.05, 0.03, 0.3);
    let fwd = ii.double_differential(e, ep, mu, t, 2.0);
    let rev = ii.double_differential(ep, e, mu, t, 2.0);
    assert!(fwd > 0.0 && rev > 0.0, "both directions must be tabulated, got {fwd} {rev}");
    let ratio = fwd / rev;
    let analytic = (ep / e) * (-(ep - e) / kt).exp();
    let rel = (ratio - analytic).abs() / analytic;
    assert!(rel < 0.01, "detailed balance: ratio {ratio} vs analytic {analytic} (rel {rel:.2e})");
}

#[test]
fn effective_temperature_is_physical() {
    let Some(sab) = load() else { return };
    let teff = sab.effective_temperature().get::<kelvin>();
    assert!(
        (293.6 < teff) && (teff < 2000.0),
        "T_eff(293.6 K) must exceed physical T and sit in the H-in-H₂O band (~1194 K), got {teff}"
    );
}
