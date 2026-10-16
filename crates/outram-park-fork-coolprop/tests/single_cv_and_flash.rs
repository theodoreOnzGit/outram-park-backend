//! V&V — the single-phase flashes ([`crate::flash`]) and the `uom`-typed
//! [`OPCPFluidSingleCV`] control volume.
//!
//! ## Methodology
//!
//! The flashes invert the EOS-native `(T, ρ)` evaluation, so the cleanest check
//! is a **round trip**: pick a well-conditioned single-phase state, evaluate it
//! forward with `state_trho` to get `(p, h, s)`, then confirm each flash
//! (`state_pt`, `state_ph`, `state_ps`) recovers the original `(T, ρ)`. Several
//! fluids and phases are exercised: superheated steam, a supercritical state,
//! and a low-pressure gas.
//!
//! The control-volume test then builds `OPCPFluidSingleCV` through each
//! constructor and checks the `uom` getters agree with the underlying state and
//! that `m = ρ·V`.
//!
//! ## Results (2026-07-08, this port)
//!
//! All round trips recover `T` and `ρ` to ≤1e-6 relative; the CV getters and the
//! mass identity hold to machine precision.

use outram_park_fork_coolprop::{
    density_pt, state_ph, state_ps, state_pt, state_trho, Fluid, OPCPFluidSingleCV,
};
use uom::si::f64::*;
use uom::si::{
    available_energy::joule_per_kilogram, mass_density::kilogram_per_cubic_meter, pressure::pascal,
    specific_heat_capacity::joule_per_kilogram_kelvin, thermodynamic_temperature::kelvin,
    volume::cubic_meter,
};

/// (fluid, T [K], ρ [kg/m³]) — all single-phase (superheated / supercritical / gas).
const CASES: &[(Fluid, f64, f64)] = &[
    (Fluid::Water, 600.0, 2.0),           // superheated steam, ~0.5 MPa
    (Fluid::Water, 800.0, 30.0),          // hotter, denser vapour
    (Fluid::CarbonDioxide, 400.0, 150.0), // supercritical CO2 (well above Tc≈304 K)
    (Fluid::Nitrogen, 300.0, 20.0),       // gas
    (Fluid::Methanol, 600.0, 5.0),        // superheated (exercises the newer terms)
];

#[test]
fn density_pt_round_trip() {
    for &(f, t, rho) in CASES {
        let p = state_trho(f, t, rho).pressure;
        let rho_back = density_pt(f, t, p).expect("density solve converges");
        let rel = (rho_back - rho).abs() / rho;
        assert!(
            rel < 1e-6,
            "{}: ρ {rho} -> p {p} -> ρ {rho_back} (rel {rel:.2e})",
            f.name()
        );
    }
}

#[test]
fn state_pt_ph_ps_round_trip() {
    for &(f, t, rho) in CASES {
        let s0 = state_trho(f, t, rho);

        // (p, T) -> (T, ρ)
        let s_pt = state_pt(f, t, s0.pressure).expect("pt flash");
        assert!(
            (s_pt.density - rho).abs() / rho < 1e-6,
            "{}: pt ρ",
            f.name()
        );

        // (p, h) -> T
        let s_ph = state_ph(f, s0.pressure, s0.enthalpy).expect("ph flash");
        assert!(
            (s_ph.temperature - t).abs() / t < 1e-6,
            "{}: ph T ({} vs {t})",
            f.name(),
            s_ph.temperature
        );
        assert!(
            (s_ph.density - rho).abs() / rho < 1e-6,
            "{}: ph ρ",
            f.name()
        );

        // (p, s) -> T
        let s_ps = state_ps(f, s0.pressure, s0.entropy).expect("ps flash");
        assert!(
            (s_ps.temperature - t).abs() / t < 1e-6,
            "{}: ps T ({} vs {t})",
            f.name(),
            s_ps.temperature
        );
        assert!(
            (s_ps.density - rho).abs() / rho < 1e-6,
            "{}: ps ρ",
            f.name()
        );
    }
}

#[test]
fn flash_rejects_non_physical_input() {
    assert!(state_pt(Fluid::Water, -1.0, 1e5).is_err());
    assert!(state_pt(Fluid::Water, 300.0, -1.0).is_err());
    assert!(state_ph(Fluid::Water, -1.0, 1e6).is_err());
}

#[test]
fn cv_constructors_and_getters_agree() {
    let vol = Volume::new::<cubic_meter>(2.0);
    let p = Pressure::new::<pascal>(5.0e5);
    let t = ThermodynamicTemperature::new::<kelvin>(600.0);

    // Reference state from the native (p, T) flash.
    let ref_state = state_pt(Fluid::Water, 600.0, 5.0e5).unwrap();

    // (p, T) constructor
    let cv = OPCPFluidSingleCV::try_new_from_pt(Fluid::Water, p, t, vol).unwrap();
    assert!((cv.get_temperature().get::<kelvin>() - 600.0).abs() < 1e-6);
    assert!((cv.get_pressure().get::<pascal>() - 5.0e5).abs() / 5.0e5 < 1e-9);
    assert!(
        (cv.get_density().get::<kilogram_per_cubic_meter>() - ref_state.density).abs()
            / ref_state.density
            < 1e-9
    );
    // specific volume = 1/ρ
    let v = cv.get_specific_volume().value;
    assert!((v - 1.0 / ref_state.density).abs() / v < 1e-12);
    // mass = ρ V
    let m = cv.get_mass().value;
    assert!((m - ref_state.density * 2.0).abs() / m < 1e-12);
    // γ = cp/cv
    assert!(cv.get_specific_heat_ratio().value > 1.0);
    assert!(cv.get_speed_of_sound().value > 0.0);

    // (p, h) and (p, s) constructors reproduce the same CV.
    let h = AvailableEnergy::new::<joule_per_kilogram>(ref_state.enthalpy);
    let cv_h = OPCPFluidSingleCV::try_new_from_ph(Fluid::Water, p, h, vol).unwrap();
    assert!((cv_h.get_temperature().get::<kelvin>() - 600.0).abs() < 1e-4);

    let s = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(ref_state.entropy);
    let cv_s = OPCPFluidSingleCV::try_new_from_ps(Fluid::Water, p, s, vol).unwrap();
    assert!((cv_s.get_temperature().get::<kelvin>() - 600.0).abs() < 1e-4);

    // native (T, ρ) constructor
    let rho = MassDensity::new::<kilogram_per_cubic_meter>(ref_state.density);
    let cv_trho = OPCPFluidSingleCV::new_from_trho(Fluid::Water, t, rho, vol);
    assert!((cv_trho.get_pressure().get::<pascal>() - 5.0e5).abs() / 5.0e5 < 1e-9);
    assert_eq!(cv_trho.get_fluid(), Fluid::Water);
}

/// V&V — `OPCPFluidSingleCV::get_equilibrium_speed_of_sound` (gh #901, epic
/// #896 step M2) at water-steam dome states against Kieffer (1977).
///
/// ## Methodology
///
/// Reference: Kieffer, S. W. (1977), "Sound speed in liquid-gas mixtures:
/// water-air and water-steam", *J. Geophys. Res.* 82(20), 2895–2904, Fig. 9
/// (equilibrium water-steam sound speed at 1 bar against quality), the same
/// digitised points `tampines-steam-tables` checks its IF97 Kieffer closure
/// against (`region_4_vap_liq_equilibrium/tests.rs`). States: water at 1 bar,
/// `h = (1−x)h' + x h''` from this crate's own saturation properties, at
/// `x = 0.1214` (Fig. 9: 128.6 m/s) and `x = 0.3527` (248.8 m/s). Pass
/// criterion: `|log10(c/c_ref)| < 0.1` (about ±26 %), the tolerance of the
/// IF97 check, which allows for the digitisation of a log-scale figure. Also
/// checked: the CV is two-phase, its frozen `get_speed_of_sound` is `NaN`
/// there, and the equilibrium speed is finite and positive.
///
/// ## Results (2026-10-16, `cargo test --release -p outram-park-fork-coolprop
/// --test single_cv_and_flash equilibrium -- --nocapture`)
///
/// x = 0.1214 → **128.46 m/s** (−0.1 % vs 128.59); x = 0.3527 →
/// **247.74 m/s** (−0.4 % vs 248.83). Both well inside the gate. The figure
/// is digitised from a log-scale plot, so this shows agreement to within that
/// reading accuracy (a few percent), not a verification of the
/// finite-difference closure to the 0.1 % level the numbers suggest.
#[test]
fn equilibrium_speed_of_sound_matches_kieffer_fig9_at_1_bar() {
    use outram_park_fork_coolprop::vle::{saturation_at_temperature, saturation_temperature};
    use uom::si::velocity::meter_per_second;

    let p = 1.0e5;
    let t_sat = saturation_temperature(Fluid::Water, p).expect("Tsat at 1 bar");
    let sat = saturation_at_temperature(Fluid::Water, t_sat).expect("VLE at 1 bar");
    let h_l = state_trho(Fluid::Water, t_sat, sat.rho_liquid).enthalpy;
    let h_v = state_trho(Fluid::Water, t_sat, sat.rho_vapour).enthalpy;

    for &(x, c_ref) in &[
        (0.12139772_f64, 128.59231566_f64),
        (0.35266992, 248.83097247),
    ] {
        let h = (1.0 - x) * h_l + x * h_v;
        let cv = OPCPFluidSingleCV::try_new_from_ph(
            Fluid::Water,
            Pressure::new::<pascal>(p),
            AvailableEnergy::new::<joule_per_kilogram>(h),
            Volume::new::<cubic_meter>(1.0),
        )
        .expect("two-phase (p, h) state");
        assert!(cv.is_two_phase(), "x = {x} must classify two-phase");
        assert!(
            cv.get_speed_of_sound().value.is_nan(),
            "frozen speed is NaN in the dome"
        );
        let c = cv
            .get_equilibrium_speed_of_sound()
            .get::<meter_per_second>();
        println!(
            "MEASURE CoolProp HEM 1 bar x = {x:.4}: {c:.2} m/s vs Kieffer Fig. 9 {c_ref:.2} m/s ({:+.1} %)",
            100.0 * (c / c_ref - 1.0)
        );
        assert!(c.is_finite() && c > 0.0);
        assert!(
            (c / c_ref).log10().abs() < 0.1,
            "x = {x}: {c} m/s vs {c_ref} m/s"
        );
    }
}
