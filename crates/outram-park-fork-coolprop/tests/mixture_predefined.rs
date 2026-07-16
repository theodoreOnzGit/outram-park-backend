//! V&V — predefined mixtures (bead op-kbc.23): the ready-made
//! [`Mixture::air`] / [`Mixture::r410a`] constructors and the composition
//! hygiene added to [`Mixture::new`].
//!
//! These exercise the **direct `(T, ρ_molar, x)` evaluation path only**
//! ([`Mixture::state_trho_molar`]); no flash / VLE is attempted (that is
//! op-kbc.22). The multi-fluid residual surface itself is validated separately
//! against the Nitrogen–Oxygen reference pair (`mixture_nitrogen_oxygen.rs`).

use outram_park_fork_coolprop::mixtures::{Mixture, MixtureError};
use outram_park_fork_coolprop::Fluid;

/// Universal gas constant used by the mixture EOS (GERG-2008 convention),
/// mirrored here to compute the ideal-gas pressure for the compressibility
/// (`Z`) check. \[J/(mol·K)\].
const R: f64 = 8.314472;

/// METHODOLOGY — verify the predefined dry-air composition is exactly the
/// CoolProp `Air.mix` set (N₂/O₂/Ar = 0.7812/0.2096/0.0092, from
/// `upstream_source/CoolProp/dev/mixtures/predefined_mixtures.json`), that the
/// components and fractions are stored in that order, that the fractions sum to
/// 1, and that the resulting molar mass matches the accepted dry-air value.
/// Reference: standard dry-air mean molar mass ≈ 28.96 g/mol (CODATA/CIPM
/// 28.9647 g/mol; this N₂/O₂/Ar-only composition omits the ~0.04 % CO₂, so it is
/// marginally lighter). Pass criterion: exact composition match, `Σx = 1`
/// exactly, molar mass within 0.1 % of 28.96 g/mol.
///
/// RESULTS (2026-07-16, this port): components = [Nitrogen, Oxygen, Argon];
/// mole fractions = [0.7812, 0.2096, 0.0092], `Σx = 1.0000` exactly; mean molar
/// mass = 28.9586 g/mol, i.e. 0.021 % below the 28.9647 g/mol CIPM dry-air
/// value (the expected CO₂-omission bias). Interpretation: the constructor
/// reproduces the reference composition to full tabulated precision.
#[test]
fn air_composition_matches_coolprop_reference() {
    let air = Mixture::air().expect("predefined air composition is valid");

    assert_eq!(
        air.components,
        vec![Fluid::Nitrogen, Fluid::Oxygen, Fluid::Argon]
    );
    assert_eq!(air.mole_fractions, vec![0.7812, 0.2096, 0.0092]);

    let sum: f64 = air.mole_fractions.iter().sum();
    assert!((sum - 1.0).abs() < 1e-12, "Sum x = {sum}, expected 1");

    let mbar_kg: f64 = air
        .components
        .iter()
        .zip(&air.mole_fractions)
        .map(|(f, x)| x * f.eos().molar_mass)
        .sum();
    let mbar_g = mbar_kg * 1000.0;
    println!("dry-air mean molar mass = {mbar_g:.4} g/mol (ref ~28.96 g/mol)");
    assert!(
        (mbar_g - 28.96).abs() / 28.96 < 1e-3,
        "molar mass {mbar_g} g/mol off reference"
    );
}

/// METHODOLOGY — evaluate the predefined dry air at `T = 300 K`,
/// `ρ_molar = 40.09 mol/m³` (chosen so the state sits at ≈ 1 atm) through the
/// three-component multi-fluid surface [`Mixture::state_trho_molar`], and check
/// two independent, widely-cited properties of dry air:
///
/// 1. **Speed of sound.** Reference `c = 331.3·√(T/273.15) = 347.2 m/s` at
///    300 K (the standard temperature scaling of the 0 °C value). Pass: within
///    0.6 %.
/// 2. **Compressibility factor** `Z = p / (ρ R T)`. Reference: air is very
///    nearly ideal at 1 atm / 300 K, `Z ≈ 0.9997`. Pass: `|Z − 1| < 0.001` and
///    `Z < 1` (attraction-dominated), plus `cp > cv`.
///
/// RESULTS (2026-07-16, this port): `p = 99 970 Pa` (0.9866 atm),
/// `w = 346.68 m/s` vs the 347.2 m/s reference → 0.15 % low; `Z = 0.99972`;
/// `cp = 29.442 J/(mol·K) > cv = 21.089 J/(mol·K)`. Interpretation: the
/// predefined three-component air reproduces the speed of sound of real dry air
/// to better than 0.2 % and shows the correct slight (< 0.03 %) real-gas
/// under-compression — the constructor and the evaluation path are consistent
/// with reference air properties, not just internally.
#[test]
fn air_speed_of_sound_and_compressibility_match_reference() {
    let air = Mixture::air().expect("predefined air composition is valid");
    let t = 300.0;
    let rho = 40.09;
    let s = air.state_trho_molar(t, rho).expect("valid air state");

    let z = s.pressure / (rho * R * t);
    println!(
        "air(300 K, 40.09 mol/m^3): p={:.1} Pa, w={:.3} m/s, Z={z:.5}, cp={:.4}, cv={:.4}",
        s.pressure, s.speed_of_sound, s.cp, s.cv
    );

    // Speed of sound in dry air at 300 K: 331.3*sqrt(300/273.15) = 347.2 m/s.
    let w_ref = 331.3 * (t / 273.15_f64).sqrt();
    assert!(
        (s.speed_of_sound - w_ref).abs() / w_ref < 6e-3,
        "w = {} m/s vs ref {w_ref} m/s",
        s.speed_of_sound
    );

    // Air is very nearly ideal at 1 atm / 300 K, Z ~ 0.9997 (< 1).
    assert!(
        (z - 1.0).abs() < 1e-3 && z < 1.0,
        "Z = {z}, expected ~0.9997"
    );
    assert!(s.cp > s.cv, "cp {} must exceed cv {}", s.cp, s.cv);
    assert!(
        s.pressure > 0.9 * 101_325.0 && s.pressure < 1.05 * 101_325.0,
        "p = {} Pa, expected near 1 atm",
        s.pressure
    );
}

/// METHODOLOGY — confirm the second predefined blend, R-410A
/// (R-32/R-125 = 0.697615/0.302385 molar, CoolProp `R410A.mix`), constructs
/// through the shared validation and evaluates to physically sane vapour
/// properties via [`Mixture::state_trho_molar`] at `T = 300 K`,
/// `ρ_molar = 100 mol/m³` (a dilute superheated-vapour state, no VLE needed).
/// No external reference-grade oracle is asserted here — reference-property /
/// VLE validation of refrigerant blends belongs to the flash work (op-kbc.22);
/// this is a construction-plus-finite-evaluation smoke. Pass: composition sums
/// to 1, and `p > 0`, `w` finite and positive, `cp > cv > 0`.
///
/// RESULTS (2026-07-16, this port): `Σx = 1.0000` (within 1e-12);
/// `p = 241 699 Pa`, `w = 195.07 m/s`, `cp = 64.309 > cv` — all finite and
/// positive, in the expected range for a superheated fluorocarbon-blend vapour.
/// Interpretation: the predefined R-410A constructor is wired correctly and the
/// evaluation path is stable for it; quantitative refrigerant validation is
/// deferred to op-kbc.22.
#[test]
fn r410a_predefined_constructs_and_evaluates() {
    let r = Mixture::r410a().expect("predefined R-410A composition is valid");
    assert_eq!(r.components, vec![Fluid::R32, Fluid::R125]);
    let sum: f64 = r.mole_fractions.iter().sum();
    assert!((sum - 1.0).abs() < 1e-12, "Sum x = {sum}, expected 1");

    let s = r
        .state_trho_molar(300.0, 100.0)
        .expect("valid R-410A state");
    println!(
        "R410A(300 K, 100 mol/m^3): p={:.1} Pa, w={:.3} m/s, cp={:.4}, cv={:.4}",
        s.pressure, s.speed_of_sound, s.cp, s.cv
    );
    assert!(s.pressure.is_finite() && s.pressure > 0.0);
    assert!(s.speed_of_sound.is_finite() && s.speed_of_sound > 0.0);
    assert!(s.cp.is_finite() && s.cv.is_finite() && s.cp > s.cv && s.cv > 0.0);
}

/// METHODOLOGY — verify the composition hygiene added to [`Mixture::new`]
/// (op-kbc.23): a mole-fraction vector that does not sum to 1 within
/// [`Mixture::SUM_TOLERANCE`] (1e-6) is rejected rather than silently trusted or
/// renormalised, a negative/non-finite fraction is rejected, and a correctly
/// normalised vector is accepted. Pass criterion: the specific `MixtureError`
/// variant for each malformed case and `Ok` for the normalised case.
///
/// RESULTS (2026-07-16, this port): `[0.5, 0.4]` (Σ = 0.9) →
/// `UnnormalizedComposition`; `[0.5, 0.6]` (Σ = 1.1) →
/// `UnnormalizedComposition`; `[1.2, -0.2]` → `InvalidMoleFraction`;
/// `[f64::NAN, 1.0]` → `InvalidMoleFraction`; `[0.5, 0.5]` → `Ok`. Boundary:
/// `[0.5, 0.5 + 5e-7]` (Σ − 1 = 5e-7 < tol) → `Ok`; `[0.5, 0.5 + 2e-6]`
/// (Σ − 1 = 2e-6 > tol) → `UnnormalizedComposition`. Interpretation: the
/// validation is active, tolerance-bounded, and propagates a distinct error per
/// failure mode — the previous `TODO` no longer trusts the caller blindly.
#[test]
fn composition_hygiene_rejects_unnormalized() {
    let n2o2 = || vec![Fluid::Nitrogen, Fluid::Oxygen];

    assert_eq!(
        Mixture::new(n2o2(), vec![0.5, 0.4]),
        Err(MixtureError::UnnormalizedComposition)
    );
    assert_eq!(
        Mixture::new(n2o2(), vec![0.5, 0.6]),
        Err(MixtureError::UnnormalizedComposition)
    );
    assert_eq!(
        Mixture::new(n2o2(), vec![1.2, -0.2]),
        Err(MixtureError::InvalidMoleFraction)
    );
    assert_eq!(
        Mixture::new(n2o2(), vec![f64::NAN, 1.0]),
        Err(MixtureError::InvalidMoleFraction)
    );

    assert!(Mixture::new(n2o2(), vec![0.5, 0.5]).is_ok());

    // Tolerance boundary (SUM_TOLERANCE = 1e-6).
    assert!(Mixture::new(n2o2(), vec![0.5, 0.5 + 5e-7]).is_ok());
    assert_eq!(
        Mixture::new(n2o2(), vec![0.5, 0.5 + 2e-6]),
        Err(MixtureError::UnnormalizedComposition)
    );
}
