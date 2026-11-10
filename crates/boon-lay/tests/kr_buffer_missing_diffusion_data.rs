// SPDX-License-Identifier: AGPL-3.0-only
//
// gh:#638: a krypton walker reaching the buffer used to panic with
// `not yet implemented` (`get_d1_for_kr(Buffer) => todo!()`).

//! # Krypton in the buffer: a typed stop, not a panic (gh:#638)
//!
//! **What is checked.** Jiang et al. (2023, INL/EXT-21-63549-Rev001) tabulate
//! krypton diffusion coefficients for the kernel, PyC and SiC only. The
//! `1e-8 m^2/s` buffer value used for Ag, Cs and Sr is the CRP-6 convention,
//! not a cited Kr value, so it is not applied to Kr. The lookup and the
//! Walk-on-Spheres engine must therefore report "no cited coefficient" as a
//! typed [`MissingDiffusionData`] the caller sees, and must not panic or
//! substitute the `1e-6 m^2/s` cracked-layer fallback.
//!
//! **Before the fix** (develop `a836e2f07`), `walk_until_released` for a
//! Kr-85 walker born at the kernel centre panicked at the kernel/buffer
//! interface: `kr85_walk_does_not_panic` failed there.
//!
//! **Results (2026-11-10, x86_64-linux, release):** all four tests pass; the
//! Kr-85 walk stops with `MissingDiffusionData { element_z: 36, layer: Buffer }`
//! at the kernel radius, and Ag, Cs, Sr and Kr's kernel, PyC and SiC values are
//! unchanged (the existing per-element tests in `diffusion_coeffs/` pin them).

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
    HopOutcome, WalkParams, WoSWalker,
};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{
    get_diffusion_coeff_jiang, try_get_diffusion_coeff_jiang, MissingDiffusionData,
    TrisoPebbleLayerMaterial,
};
use boon_lay::Nuclide;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::degree_celsius;

fn t_1000c() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(1000.0)
}

/// The issue's own probe: before the fix this panicked, so `catch_unwind`
/// returned `Err`.
#[test]
fn kr85_walk_does_not_panic() {
    let cell = TrisoCell::new_crp6_geometry();
    let mut w = WoSWalker::new_at_center(Nuclide::Kr85, OoRng64::from_u64(1));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        w.walk_until_released(&cell, &WalkParams::crp6_default())
    }));
    assert!(result.is_ok(), "a Kr-85 walker must not panic at the buffer");
    // No cited coefficient, so no release time either.
    assert_eq!(result.unwrap(), None);
}

/// The stop is typed and names the (element, layer) pair.
#[test]
fn kr85_walk_reports_missing_buffer_coefficient() {
    let cell = TrisoCell::new_crp6_geometry();
    let mut w = WoSWalker::new_at_center(Nuclide::Kr85, OoRng64::from_u64(1));
    let err = w
        .try_walk_until_released(&cell, &WalkParams::crp6_default())
        .expect_err("Kr has no cited buffer coefficient");
    let expected = MissingDiffusionData {
        element_z: 36,
        layer: TrisoPebbleLayerMaterial::Buffer,
    };
    assert_eq!(err, expected);
    assert_eq!(w.missing_diffusion_data, Some(expected));
    // The walker is still inside (it stopped at the kernel/buffer interface)
    // and stays stopped.
    assert!(w.radius() <= cell.get_buffer_radius());
    assert_eq!(
        w.step_multilayer(&cell, &WalkParams::crp6_default()),
        HopOutcome::NoDiffusionData
    );
    // The frame-stepping driver reports "still inside" and does not move it.
    let before = w.position;
    let until = w.time * 2.0;
    assert!(w.diffuse_until(&cell, &WalkParams::crp6_default(), until));
    assert_eq!(w.position, before);
    assert!(err.to_string().contains("Z = 36"));
}

/// The lookup: Kr in the buffer and graphite is an error, not a value; the
/// cited layers still give a value; `try_get` folds the error into `None`.
#[test]
fn kr_lookup_is_typed_error_only_where_uncited() {
    for layer in [
        TrisoPebbleLayerMaterial::Buffer,
        TrisoPebbleLayerMaterial::MatrixGraphite,
        TrisoPebbleLayerMaterial::StructuralGraphite,
    ] {
        let r = get_diffusion_coeff_jiang(layer, Nuclide::Kr85, t_1000c(), None);
        assert_eq!(
            r,
            Err(MissingDiffusionData {
                element_z: 36,
                layer
            })
        );
        assert!(try_get_diffusion_coeff_jiang(layer, Nuclide::Kr85, t_1000c(), None).is_none());
    }
    for layer in [
        TrisoPebbleLayerMaterial::KernelUO2,
        TrisoPebbleLayerMaterial::PyC,
        TrisoPebbleLayerMaterial::SiC,
        TrisoPebbleLayerMaterial::CrackedMaterial,
    ] {
        let d = get_diffusion_coeff_jiang(layer, Nuclide::Kr85, t_1000c(), None)
            .expect("Kr is cited in this layer");
        assert_eq!(
            Some(d),
            try_get_diffusion_coeff_jiang(layer, Nuclide::Kr85, t_1000c(), None)
        );
    }
}

/// Elements with a buffer convention keep it: the change touches Kr only.
#[test]
fn ag_cs_sr_buffer_values_unchanged() {
    use uom::si::diffusion_coefficient::square_meter_per_second;
    for n in [Nuclide::Ag110m, Nuclide::Cs137, Nuclide::Sr90] {
        let d = get_diffusion_coeff_jiang(TrisoPebbleLayerMaterial::Buffer, n, t_1000c(), None)
            .expect("buffer convention present");
        // D1 = 1e-8 with Q1 = 0 (and D2 = 0) gives exactly 1e-8.
        assert_eq!(d.get::<square_meter_per_second>(), 1e-8);
    }
}
