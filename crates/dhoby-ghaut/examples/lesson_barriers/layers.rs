//! The calls behind the Fission products chapter "Barriers: from bare
//! kernel to TRISO": per-layer diffusion coefficients from boon-lay's
//! Lagrangian walker (`try_get_diffusion_coeff_jiang`, Jiang et al. 2023,
//! INL/EXT-21-63549-Rev001, as boon-lay cites it) and TRISO-ATOPS's
//! correlations, and the HTR-10 TRISO layer thicknesses from outram-mc-libs'
//! `TrisoSpec::HTR10_LI2014`. Public API only. Shared by `main.rs` and
//! `tests/lesson_barriers.rs` through `#[path]`.
#![allow(dead_code)]

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{
    try_get_diffusion_coeff_jiang, TrisoPebbleLayerMaterial,
};
use boon_lay::Nuclide;
use outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::degree_celsius;

/// The layers the walker carries coefficients for, in particle order.
pub fn layers() -> Vec<(&'static str, TrisoPebbleLayerMaterial)> {
    vec![
        ("kernel", TrisoPebbleLayerMaterial::KernelUO2),
        ("buffer", TrisoPebbleLayerMaterial::Buffer),
        ("PyC", TrisoPebbleLayerMaterial::PyC),
        ("SiC", TrisoPebbleLayerMaterial::SiC),
    ]
}

/// The walker's D [m^2/s] for `nuclide` in `layer` at `t_c` °C, no
/// fluence; `None` where boon-lay has no value (a `todo!()` there, such as
/// Kr in the buffer, or a `None` from the function). The panic of a
/// `todo!()` is caught (its message still prints) so the gap can be
/// reported rather than hidden.
pub fn walker_d(layer: TrisoPebbleLayerMaterial, nuclide: Nuclide, t_c: f64) -> Option<f64> {
    std::panic::catch_unwind(|| {
        try_get_diffusion_coeff_jiang(layer, nuclide, ThermodynamicTemperature::new::<degree_celsius>(t_c), None)
            .map(|d| d.get::<square_meter_per_second>())
    })
    .ok()
    .flatten()
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

/// The diffusion time scale across a layer of thickness `l_m` [s]: l^2 / D.
pub fn crossing_time(l_m: f64, d: f64) -> f64 {
    l_m * l_m / d
}
