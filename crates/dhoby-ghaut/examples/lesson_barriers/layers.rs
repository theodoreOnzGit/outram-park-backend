//! The calls behind chapter 4's migration pages (Fission products, lessons
//! 29-32): per-layer diffusion coefficients from boon-lay's Lagrangian
//! walker (`try_get_diffusion_coeff_jiang`, Jiang et al. 2023,
//! INL/EXT-21-63549-Rev001, as boon-lay cites it), TRISO-ATOPS's graphite and
//! SiC-Ag correlations, and the HTR-10 TRISO layer thicknesses from
//! outram-mc-libs' `TrisoSpec::HTR10_LI2014`. Public API only. Shared by
//! `main.rs` and `tests/lesson_barriers.rs` through `#[path]`.
//!
//! **No call here can panic.** The walker's coefficient table has `todo!()`
//! arms (matrix and structural graphite for every element, and krypton in
//! the buffer: gh:#541). [`has_walker_coefficient`] encodes exactly which
//! (layer, element) pairs are filled, read from
//! `crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/temperature_dependent_collisions/diffusion_coeffs/mod.rs`
//! on 2026-10-22, and [`walker_d`] refuses the others instead of calling
//! them. The gap is reported on the page as a gap.
#![allow(dead_code)]

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{
    try_get_diffusion_coeff_jiang, TrisoPebbleLayerMaterial,
};
use boon_lay::Nuclide;
use outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::degree_celsius;

/// The layers the walker carries coefficients for, in particle order. The
/// walker maps both IPyC and OPyC to `PyC`.
pub fn layers() -> Vec<(&'static str, TrisoPebbleLayerMaterial)> {
    vec![
        ("kernel", TrisoPebbleLayerMaterial::KernelUO2),
        ("buffer", TrisoPebbleLayerMaterial::Buffer),
        ("PyC", TrisoPebbleLayerMaterial::PyC),
        ("SiC", TrisoPebbleLayerMaterial::SiC),
    ]
}

/// The four elements with their own coefficients in the walker; every other
/// element falls through to silver's (gh:#541).
pub fn walker_elements() -> [(&'static str, Nuclide); 4] {
    [("Ag", Nuclide::Ag110m), ("Cs", Nuclide::Cs137), ("Sr", Nuclide::Sr90), ("Kr", Nuclide::Kr85)]
}

/// Whether boon-lay's walker has a filled (non-`todo!()`) coefficient for
/// `nuclide` in `layer`, and if not, why. Read from the source (see the
/// module docs); `tests/lesson_barriers.rs` calls every pair this says is
/// filled, so a pair wrongly marked filled would panic there, not on a page.
pub fn has_walker_coefficient(layer: TrisoPebbleLayerMaterial, nuclide: Nuclide) -> Result<(), &'static str> {
    use TrisoPebbleLayerMaterial::*;
    match layer {
        MatrixGraphite | StructuralGraphite => Err("graphite is todo!() in boon-lay's walker (gh:#541)"),
        Buffer if nuclide.get_z_a().0 == 36 => Err("krypton in the buffer is todo!() in boon-lay's walker (gh:#541)"),
        _ => Ok(()),
    }
}

/// The walker's D [m^2/s] for `nuclide` in `layer` at `t_c` °C, no fluence;
/// `None` where boon-lay has no value. Never calls a `todo!()` arm.
pub fn walker_d(layer: TrisoPebbleLayerMaterial, nuclide: Nuclide, t_c: f64) -> Option<f64> {
    has_walker_coefficient(layer, nuclide).ok()?;
    try_get_diffusion_coeff_jiang(layer, nuclide, ThermodynamicTemperature::new::<degree_celsius>(t_c), None)
        .map(|d| d.get::<square_meter_per_second>())
}

/// HTR-10 TRISO layer thicknesses [m], from the cumulative radii of
/// `TrisoSpec::HTR10_LI2014` (cm): kernel radius, buffer, IPyC, SiC, OPyC.
pub fn htr10_thicknesses() -> [(&'static str, f64); 5] {
    let s = TrisoSpec::HTR10_LI2014;
    let m = |cm: f64| cm / 100.0;
    [
        ("kernel", m(s.kernel)),
        ("buffer", m(s.buffer - s.kernel)),
        ("IPyC", m(s.ipyc - s.buffer)),
        ("SiC", m(s.sic - s.ipyc)),
        ("OPyC", m(s.opyc - s.sic)),
    ]
}

/// HTR-10 TRISO outer radii [m] of kernel, buffer, IPyC, SiC, OPyC.
pub fn htr10_radii() -> [f64; 5] {
    let s = TrisoSpec::HTR10_LI2014;
    [s.kernel / 100.0, s.buffer / 100.0, s.ipyc / 100.0, s.sic / 100.0, s.opyc / 100.0]
}

/// The diffusion time scale across a layer of thickness `l_m` [s]: l^2 / D.
pub fn crossing_time(l_m: f64, d: f64) -> f64 {
    l_m * l_m / d
}
