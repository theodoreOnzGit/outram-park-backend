//! The calls behind the Fission products chapter "TRISO kernel: how fission
//! products leave a bare UO2 sphere" (maintainer design, 2026-10-20). Public
//! API of boon-lay, tampines, nee_soon and outram-mc-libs only; nothing in
//! them is changed or copied. Shared by `main.rs` and
//! `tests/lesson_triso_kernel.rs` through `#[path]`.
//!
//! - **The code's lumping**, read from the code: boon-lay's
//!   `ElementGroup::from_atomic_number`, the classes of identical
//!   `diffusion_coefficient` output (kernel and graphite, on a temperature
//!   grid), and which groups plate out under TRISO-ATOPS's own routing
//!   (`RemovalRates::upstream`).
//! - **Booth**: `booth_longlived` and `booth_shortlived_fast_diffuse` on
//!   grids of the reduced variables, and boon-lay's CRP-6 kernel Monte Carlo
//!   (`mc_kernel_release_fraction`) against Booth at the same D.
//! - **The kernel's temperature**: the HTR-10 core-average volumetric power
//!   from cited constants (nee_soon `htr10_rmc::table1`, tampines
//!   `Pebble::htr10`, outram-mc-libs `TrisoSpec::HTR10_LI2014`), the radial
//!   conduction profile through tampines' fresh-UO2 conductivity, and
//!   tampines' own particle solution as the check.
#![allow(dead_code)]

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::{
    kernel_diffusion_coefficient, mc_kernel_release_fraction,
};
use boon_lay::prelude::decay_library::DecayLibrary;
use boon_lay::prelude::diffusion_coefficient;
use boon_lay::triso_atops_fork::release_models::steady_state::{booth_longlived, booth_shortlived_fast_diffuse};
use boon_lay::triso_atops_extensions::removal_rates::RemovalRates;
use boon_lay::triso_atops_fork::nuclide_model::ElementGroup;
use boon_lay::Nuclide;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{DiffusionCoefficient, Frequency, Length, Power, Ratio, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
use uom::si::time::second;

/// Element symbols by Z (index 0 unused), Z = 1..100.
pub const SYMBOLS: [&str; 101] = [
    "", "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl", "Ar", "K", "Ca",
    "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge", "As", "Se", "Br", "Kr", "Rb", "Sr", "Y",
    "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce",
    "Pr", "Nd", "Pm", "Sm", "Eu", "Gd", "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir",
    "Pt", "Au", "Hg", "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm",
    "Bk", "Cf", "Es", "Fm",
];

/// TRISO-ATOPS's transport group of element `z`, by its Debug name.
pub fn group_name(z: u32) -> String {
    format!("{:?}", ElementGroup::from_atomic_number(z))
}

/// Whether element `z` plates out under TRISO-ATOPS's own routing
/// (`RemovalRates::upstream` with non-zero rates: noble gases do not).
pub fn plates_out(z: u32) -> bool {
    let one = Frequency::new::<hertz>(1.0);
    RemovalRates::upstream(one, one).for_element(z).k_plate.get::<hertz>() > 0.0
}

/// A diffusion class: the elements whose kernel and graphite D are identical
/// at every grid temperature, and those two curves [m^2/s].
#[derive(Debug, Clone, PartialEq)]
pub struct DClass {
    pub members: Vec<u32>,
    pub kernel: Vec<f64>,
    pub graphite: Vec<f64>,
}

/// The kernel and graphite D of element `z` on the grid `t_c` [°C].
pub fn d_curves(z: u32, t_c: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let temp = |c: f64| ThermodynamicTemperature::new::<degree_celsius>(c);
    t_c.iter()
        .map(|&c| {
            let k = diffusion_coefficient(z, temp(c), temp(c));
            (k.kernel.get::<square_meter_per_second>(), k.graphite.get::<square_meter_per_second>())
        })
        .unzip()
}

/// Groups Z = 1..=`z_max` into classes of identical `diffusion_coefficient`
/// output: the code's own lumping, found without reading its match arms.
pub fn d_classes(t_c: &[f64], z_max: u32) -> Vec<DClass> {
    let mut out: Vec<DClass> = Vec::new();
    for z in 1..=z_max {
        let (k, g) = d_curves(z, t_c);
        match out.iter_mut().find(|c| c.kernel == k && c.graphite == g) {
            Some(c) => c.members.push(z),
            None => out.push(DClass { members: vec![z], kernel: k, graphite: g }),
        }
    }
    out
}

/// Booth's fractional release of a stable species, `booth_longlived`, at the
/// reduced time `x = D t / a^2` (a = 1 m, t = 1 s, D = x).
pub fn booth_fraction(x: f64) -> f64 {
    booth_longlived(
        DiffusionCoefficient::new::<square_meter_per_second>(x),
        Time::new::<second>(1.0),
        Length::new::<meter>(1.0),
    )
    .get::<ratio>()
}

/// Booth's steady release-to-birth ratio, `booth_shortlived_fast_diffuse`,
/// at `mu = sqrt(lambda a^2 / D)` (D = 1 m^2/s, a = 1 m, lambda = mu^2).
pub fn booth_rb(mu: f64) -> f64 {
    booth_shortlived_fast_diffuse(
        DiffusionCoefficient::new::<square_meter_per_second>(1.0),
        Frequency::new::<hertz>(mu * mu),
        Length::new::<meter>(1.0),
    )
    .get::<ratio>()
}

/// boon-lay's CRP-6 kernel Monte Carlo (Walk-on-Spheres, uniform birth,
/// perfect-sink surface) for `nuclide` in a kernel of radius `a_m` at
/// `t_c` °C after `time_s`, beside Booth's fraction at the same D:
/// (D [m^2/s], MC fraction, Booth fraction).
pub fn mc_vs_booth(nuclide: Nuclide, a_m: f64, t_c: f64, time_s: f64, n: usize, seed: u64) -> (f64, f64, f64) {
    let temp = ThermodynamicTemperature::new::<degree_celsius>(t_c);
    let d = kernel_diffusion_coefficient(nuclide, temp).get::<square_meter_per_second>();
    let mc = mc_kernel_release_fraction(nuclide, Length::new::<meter>(a_m), temp, Time::new::<second>(time_s), n, seed);
    (d, mc, booth_fraction(d * time_s / (a_m * a_m)))
}

/// HTR-10's core-average fission power per unit kernel volume [W/m^3]:
/// thermal power / fuel elements / particles per pebble / kernel volume, from
/// nee_soon's `htr10_rmc::table1` (the paper's Table 1 and body text),
/// tampines' `Pebble::htr10().particles_per_pebble` and outram-mc-libs'
/// `TrisoSpec::HTR10_LI2014.kernel`. Returns (q''' [W/m^3], power per
/// particle [W]).
pub fn htr10_kernel_power_density() -> (f64, f64) {
    use nee_soon::htr10_rmc::table1::{FUEL_ELEMENTS, THERMAL_POWER_MW};
    let per_pebble = tampines::pebble_bed::Pebble::htr10().particles_per_pebble;
    let p = THERMAL_POWER_MW * 1e6 / FUEL_ELEMENTS / per_pebble;
    let a = outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec::HTR10_LI2014.kernel / 100.0;
    (p / (4.0 / 3.0 * std::f64::consts::PI * a * a * a), p)
}

/// Fresh UO2 conductivity [W/(m K)] at `t_k`, tampines'
/// `uranium_dioxide_thermal_conductivity` (VTB HTR-PM deck, 300-2000 K).
pub fn k_uo2(t_k: f64) -> f64 {
    tampines::pebble_bed::triso::uranium_dioxide_thermal_conductivity(ThermodynamicTemperature::new::<kelvin>(t_k))
        .expect("inside 300-2000 K")
        .get::<watt_per_meter_kelvin>()
}

/// Steady radial temperature in a sphere of radius `a_m` with uniform heat
/// `q` [W/m^3], surface at `t_surf_k`, conductivity `k(T)`: integrates
/// dT/dr = -q r / (3 k(T)) inward from the surface in `n` steps (midpoint
/// rule). Returns (r [m], T [K]) from the centre out.
pub fn sphere_profile(q: f64, a_m: f64, t_surf_k: f64, n: usize, k: impl Fn(f64) -> f64) -> Vec<(f64, f64)> {
    let h = a_m / n as f64;
    let mut out = vec![(a_m, t_surf_k)];
    let mut t = t_surf_k;
    for i in (0..n).rev() {
        let r_hi = (i + 1) as f64 * h;
        let r_mid = r_hi - 0.5 * h;
        let t_mid = t + 0.5 * h * q * r_hi / (3.0 * k(t));
        t += h * q * r_mid / (3.0 * k(t_mid));
        out.push((i as f64 * h, t));
    }
    out.reverse();
    out
}

/// tampines' own HTR-10 particle solution at power `p_w` and OPyC surface
/// `t_surf_k` (no fluence): (kernel centre, kernel surface) [K].
pub fn tampines_particle(p_w: f64, t_surf_k: f64) -> (f64, f64) {
    let prof = tampines::pebble_bed::TrisoParticle::htr10()
        .steady_state_temperatures(Power::new::<watt>(p_w), ThermodynamicTemperature::new::<kelvin>(t_surf_k), Ratio::new::<ratio>(0.0))
        .expect("HTR-10 particle at core-average power");
    (prof.kernel_centre.get::<kelvin>(), prof.kernel_surface.get::<kelvin>())
}

/// Half-life [s] of `n` from boon-lay's decay library, if unstable.
pub fn half_life_s(lib: &DecayLibrary, n: Nuclide) -> Option<f64> {
    lib.try_match_nuclides_to_decay_data(n)?.try_get_half_life().map(|t| t.get::<second>())
}
