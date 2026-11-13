//! V&V: the `(p, T)` flash returns the STABLE root, and the other defects the CoolProp
//! differential (gh:#1349) found and this change fixed.
//!
//! ## Methodology
//!
//! Reference values are CoolProp's own output at `0e67fe74b30a2fe9526af9bc64ea026a96f56ebf`
//! (built from source, g++ 13.3, glibc 2.39, no `-ffast-math`, no FMA), printed with `%.17g`
//! by a one-off driver (`PT_INPUTS`, `QT_INPUTS`), 2026-11-13. The inputs are round numbers
//! chosen before the run, not tuned.
//!
//! ## Results (2026-11-13)
//!
//! Before the change `density_pt` started every Newton solve from the ideal-gas density and so
//! returned the vapour root for compressed liquids: 3398 of 3680 liquid-phase grid points of
//! the differential were wrong (gh:#917: Water at 2 MPa, 350 K came back as vapour). After:
//! the cases below agree with CoolProp to the asserted tolerances (measured values in the
//! comments), and 12 of 3680 liquid points still differ (Propylene glycol at 2 Pa, see
//! `verification_and_validation/coolprop_differential/RESULTS.md`).

use outram_park_fork_coolprop::{
    conductivity, density_pt, saturation_at_temperature, viscosity, Fluid,
};

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / a.abs().max(b.abs())
}

/// `(fluid, T [K], p [Pa], CoolProp rho [kg/m3], CoolProp phase)`
const CASES: [(Fluid, f64, f64, f64, &str); 7] = [
    (Fluid::Water, 350.0, 2e6, 974.57327918265173, "liquid, gh:#917"),
    (Fluid::Water, 300.0, 1e5, 996.55634038891594, "liquid"),
    (Fluid::Water, 400.0, 1e5, 0.54760541522594286, "gas: must be unchanged"),
    (Fluid::Acetone, 250.0, 1e6, 837.64749227497293, "liquid"),
    (Fluid::R134a, 280.0, 5e6, 1290.6560052059392, "supercritical liquid (p > pc)"),
    (Fluid::Nitrogen, 70.0, 1e6, 840.49497413378378, "liquid"),
    (Fluid::Water, 700.0, 3e7, 184.23678566068949, "supercritical"),
];

#[test]
fn density_pt_returns_the_stable_root() {
    for (fluid, t, p, coolprop, what) in CASES {
        let rho = density_pt(fluid, t, p).expect("converges");
        let d = rel(rho, coolprop);
        println!("{:?} T={t} p={p}: rust {rho:.12} coolprop {coolprop:.12} rel {d:.2e} ({what})", fluid);
        // Measured 2026-11-13: worst 2.6e-14 (Water, 300 K).
        assert!(d < 1e-11, "{fluid:?} {what}: rel {d:.3e}");
    }
}

/// CoolProp default configuration (superancillaries on), Cyclopentane at
/// `T = 511.61015944850499 K` (the last point of the saturation grid): the Maxwell pair is
/// `(rho' = 291.8621941, rho'' = 258.0645436)`. The Newton solve used to converge with the two
/// labels swapped, returning `rho_liquid < rho_vapour`.
#[test]
fn saturation_labels_are_ordered_near_the_critical_point() {
    let s = saturation_at_temperature(Fluid::Cyclopentane, 511.61015944850499).expect("solves");
    assert!(s.rho_liquid > s.rho_vapour, "{s:?}");
    // CoolProp (SATS row of the reference): 291.86219414161997 / 258.0645435893633, p 4575583.569887.
    assert!(rel(s.rho_liquid, 291.86219414161997) < 1e-8, "{s:?}");
    assert!(rel(s.rho_vapour, 258.0645435893633) < 1e-8, "{s:?}");
    assert!(rel(s.pressure, 4575583.569887) < 1e-9, "{s:?}");
}

/// R152A, R1234yf, R1234ze(E) (rhosr-CS viscosity) and EthylBenzene (ECS viscosity) have no
/// viscosity model here, and their conductivity needs it for the Olchowy-Sengers critical term.
/// They used to return a conductivity silently missing that term; now `None`.
#[test]
fn conductivity_without_its_critical_term_is_none() {
    for fluid in [Fluid::R152A, Fluid::R1234yf, Fluid::R1234zeE, Fluid::EthylBenzene] {
        let eos = fluid.eos();
        let (t, rho) = (1.05 * eos.t_critical, eos.rho_critical * eos.molar_mass);
        assert!(viscosity(fluid, t, rho).is_none(), "{fluid:?} has a viscosity now: revisit");
        assert!(conductivity(fluid, t, rho).is_none(), "{fluid:?}: partial conductivity returned");
    }
    // A fluid with both is untouched.
    let e = Fluid::Ethane.eos();
    assert!(conductivity(Fluid::Ethane, 1.05 * e.t_critical, e.rho_critical * e.molar_mass).is_some());
}
