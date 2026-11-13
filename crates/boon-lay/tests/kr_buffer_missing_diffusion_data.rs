// SPDX-License-Identifier: AGPL-3.0-only
//
// gh:#638: a krypton walker reaching the buffer used to panic with
// `not yet implemented` (`get_d1_for_kr(Buffer) => todo!()`).

//! # Krypton in the buffer: ~~a typed stop, not a panic~~ a cited value (gh:#638)
//!
//! The file keeps its first name so the history of gh:#638 stays in one
//! place.
//!
//! ## History
//!
//! - **Before 2026-11-10** (develop `a836e2f07`): `walk_until_released` for a
//!   Kr-85 walker born at the kernel centre panicked at the kernel/buffer
//!   interface (`get_d1_for_kr(Buffer) => todo!()`).
//! - **First fix, 2026-11-10** (branch `fix/1317-panics`): ~~Jiang et al.
//!   (2023) tabulate Kr for the kernel, PyC and SiC only, and the `1e-8 m^2/s`
//!   buffer value of Ag, Cs and Sr was taken to be a CRP-6 convention with no
//!   source applying it to Kr, so the lookup returned a typed
//!   `MissingDiffusionData` and the walker stopped at the buffer.~~
//! - **CORRECTED 2026-11-10** (maintainer: "Find a cited value"): a source
//!   applies it to Kr. IAEA-TECDOC-978 (1997) App. A, section A.1.2, printed
//!   p. 440: the buffer has "no significant retention capability for fission
//!   products", and "in FRG calculations, a value of the diffusion
//!   coefficient of 10^-8 m^2/s with no temperature dependence (activation
//!   energy equal to zero) is used"; the same appendix's FRG tables carry Kr
//!   in the kernel (Table A-1) and PyC (Table A-2). IAEA-TECDOC-1674 (2012)
//!   section 10.2.2, printed p. 538: CRP-6 computes "caesium, strontium,
//!   silver, fission gases" with the "Germany" set of TECDOC-978, and its
//!   Table 10.5 asks for Kr-85 through whole particles (cases 3e, 4d). So
//!   Kr in the buffer is `D = 1e-8 m^2/s`, `Q = 0`
//!   (`diffusion_coeffs::KR_BUFFER_D_M2_PER_S`). A convention, not a
//!   measurement: the US value in the same sentence is `1e-10 m^2/s`.
//!   Graphite still has no Kr value in the German set, so the typed error
//!   stays for `MatrixGraphite` and `StructuralGraphite`.
//!
//! ## Methodology
//!
//! The lookup is checked against the cited numbers directly (exact equality:
//! `1e-8 * exp(0) + 0 * exp(0)` is `1e-8` in floating point). The walker is
//! checked on `TrisoCell::new_crp6_geometry()` (kernel radius 212.5 um, buffer
//! 100 um, all layers at 1600 °C, zero fluence) with
//! `WalkParams::crp6_default()` but a bounded step budget, because the PyC
//! barrier makes a full release run impractical (below).
//!
//! **Predictions, written before the first run (2026-11-10):**
//!
//! - P1. Kr-85 in the buffer: `D = 1e-8 m^2/s` exactly at 600, 1000, 1600 and
//!   2000 °C (no temperature dependence); the four public getters give
//!   `D1 = 1e-8`, `Q1 = 0`, `D2 = 0`, `Q2 = 0`; graphite stays an error.
//! - P2. The Kr-85 walker no longer stops. At 1600 °C the kernel D is
//!   `8.8e-15 exp(-54/RT) + 0.6 exp(-480/RT) ~ 2.5e-14 m^2/s`, so the
//!   kernel-to-buffer transmission probability `D_buf/(D_ker + D_buf)` is
//!   `1 - 2.5e-6`: the walker is in the buffer within the first 10 steps
//!   (one hop from the centre to the interface, then the crossing).
//! - P3. In 100 000 steps it never reaches the IPyC: Kr in PyC at 1600 °C is
//!   `~2.2e-16 m^2/s`, so each buffer/IPyC arrival transmits with
//!   probability `~2e-8`. It is not released, so the release fraction of a
//!   bounded run stays 0, as it was when the walker stopped (no movement in
//!   any recorded number; no V&V case walks Kr).
//!
//! **Results (2026-11-10, x86_64-linux, release):** all four tests pass;
//! P1, P2 and P3 HIT (numbers in each test's doc comment).

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
    HopOutcome, WalkParams, WoSWalker,
};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::diffusion_coeffs::{
    get_d1_for_kr, get_d2_for_kr, get_q1_for_kr, get_q2_for_kr, KR_BUFFER_D_M2_PER_S,
};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{
    get_diffusion_coeff_jiang, krypton_has_cited_coefficients, try_get_diffusion_coeff_jiang,
    MissingDiffusionData, TrisoPebbleLayerMaterial,
};
use boon_lay::Nuclide;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::length::meter;
use uom::si::molar_energy::kilojoule_per_mole;
use uom::si::thermodynamic_temperature::degree_celsius;

fn celsius(c: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(c)
}

fn bounded(max_steps: u64) -> WalkParams {
    WalkParams {
        max_steps,
        ..WalkParams::crp6_default()
    }
}

/// P1: the cited buffer value, through the lookup and the public getters.
///
/// Result 2026-11-10: P1 HIT, `1e-8` exactly at all four temperatures.
#[test]
fn kr_buffer_is_the_cited_german_value() {
    assert_eq!(KR_BUFFER_D_M2_PER_S, 1e-8);
    assert!(krypton_has_cited_coefficients(TrisoPebbleLayerMaterial::Buffer));
    for c in [600.0, 1000.0, 1600.0, 2000.0] {
        let t = celsius(c);
        let b = TrisoPebbleLayerMaterial::Buffer;
        let d = get_diffusion_coeff_jiang(b, Nuclide::Kr85, t, None).expect("Kr buffer is cited");
        assert_eq!(d.get::<square_meter_per_second>(), 1e-8, "at {c} °C");
        assert_eq!(get_d1_for_kr(b, t).get::<square_meter_per_second>(), 1e-8);
        assert_eq!(get_q1_for_kr(b, t).get::<kilojoule_per_mole>(), 0.0);
        assert_eq!(get_d2_for_kr(b, t).get::<square_meter_per_second>(), 0.0);
        assert_eq!(get_q2_for_kr(b, t).get::<kilojoule_per_mole>(), 0.0);
        // Same value as Ag, Cs and Sr, which carry the same convention.
        for n in [Nuclide::Ag110m, Nuclide::Cs137, Nuclide::Sr90] {
            let other = get_diffusion_coeff_jiang(b, n, t, None).expect("buffer convention present");
            assert_eq!(other, d);
        }
    }
}

/// The typed error is kept where the German set has no Kr value: the two
/// graphite variants. Every TRISO layer gives a value, and `try_get` agrees.
///
/// Result 2026-11-10: pass.
#[test]
fn kr_lookup_is_typed_error_only_for_graphite() {
    let t = celsius(1000.0);
    for layer in [
        TrisoPebbleLayerMaterial::MatrixGraphite,
        TrisoPebbleLayerMaterial::StructuralGraphite,
    ] {
        assert!(!krypton_has_cited_coefficients(layer));
        let r = get_diffusion_coeff_jiang(layer, Nuclide::Kr85, t, None);
        let expected = MissingDiffusionData {
            element_z: 36,
            layer,
        };
        assert_eq!(r, Err(expected));
        assert!(expected.to_string().contains("Z = 36"));
        assert!(try_get_diffusion_coeff_jiang(layer, Nuclide::Kr85, t, None).is_none());
    }
    for layer in [
        TrisoPebbleLayerMaterial::KernelUO2,
        TrisoPebbleLayerMaterial::Buffer,
        TrisoPebbleLayerMaterial::PyC,
        TrisoPebbleLayerMaterial::SiC,
        TrisoPebbleLayerMaterial::CrackedMaterial,
    ] {
        assert!(krypton_has_cited_coefficients(layer));
        let d = get_diffusion_coeff_jiang(layer, Nuclide::Kr85, t, None)
            .expect("Kr is cited in this layer");
        assert_eq!(Some(d), try_get_diffusion_coeff_jiang(layer, Nuclide::Kr85, t, None));
    }
}

/// The issue's own probe, bounded: before the first fix this panicked.
///
/// Result 2026-11-10: pass; no panic, no stop, not released in 200 000 steps.
#[test]
fn kr85_walk_does_not_panic() {
    let cell = TrisoCell::new_crp6_geometry();
    let mut w = WoSWalker::new_at_center(Nuclide::Kr85, OoRng64::from_u64(1));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let r = w.try_walk_until_released(&cell, &bounded(200_000));
        (r, w.missing_diffusion_data)
    }));
    let (r, missing) = result.expect("a Kr-85 walker must not panic at the buffer");
    // P3: the step cap is hit, not a missing coefficient.
    assert_eq!(r, Ok(None));
    assert_eq!(missing, None);
}

/// P2 and P3: the walker crosses into the buffer and stays there.
///
/// Result 2026-11-10 (`cargo test --release -p boon-lay --test
/// kr_buffer_missing_diffusion_data -- --nocapture`): P2 HIT, in the buffer
/// from step 1 (index 1, the second step); P3 HIT, 99 999 of 100 000 steps
/// in the buffer, `r_max` 312.500 um against the buffer's 312.5 um (never
/// into the IPyC), 3.824e5 s of simulated time, never released.
#[test]
fn kr85_walker_proceeds_through_the_buffer() {
    let cell = TrisoCell::new_crp6_geometry();
    let params = bounded(100_000);
    let r_kernel = cell.get_fuel_radius().get::<meter>();
    let r_buffer = cell.get_buffer_radius().get::<meter>();
    let mut w = WoSWalker::new_at_center(Nuclide::Kr85, OoRng64::from_u64(1));
    let mut first_in_buffer: Option<u64> = None;
    let mut steps_in_buffer: u64 = 0;
    let mut r_max: f64 = 0.0;
    for step in 0..params.max_steps {
        let outcome = w.step_multilayer(&cell, &params);
        assert_ne!(outcome, HopOutcome::NoDiffusionData, "step {step}");
        assert_ne!(outcome, HopOutcome::Released, "step {step}");
        let r = w.radius().get::<meter>();
        r_max = r_max.max(r);
        if r > r_kernel && r < r_buffer {
            steps_in_buffer += 1;
            first_in_buffer.get_or_insert(step);
        }
    }
    println!(
        "Kr-85, 1600 C, {} steps: first in buffer at step {:?}, {} steps in buffer, \
         r_max {:.3} um (kernel {:.1}, buffer {:.1}), simulated time {:.3e} s",
        params.max_steps,
        first_in_buffer,
        steps_in_buffer,
        r_max * 1e6,
        r_kernel * 1e6,
        r_buffer * 1e6,
        w.time.get::<uom::si::time::second>()
    );
    assert_eq!(w.missing_diffusion_data, None);
    // P2: in the buffer within the first 10 steps.
    assert!(first_in_buffer.expect("the walker entered the buffer") < 10);
    // P3: never into the IPyC (r_max stays within the buffer plus the
    // reinsertion distance on the buffer side).
    assert!(r_max < r_buffer, "r_max {r_max} reached the IPyC");
}
