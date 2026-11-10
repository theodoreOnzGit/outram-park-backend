//! The calls behind "Walking on spheres" (Fission products, chapter 2):
//! boon-lay's legacy Gaussian step (`central_limit_theorem::
//! sample_dimensioned_gaussian_vector`), its Walk-on-Spheres engine
//! (`first_passage::walk_on_spheres::WoSWalker`, `sphere_fpt`), the CRP-6
//! geometry as the solver sees it (`TrisoCell::new_crp6_geometry`,
//! `get_triso_region`), the CRP-6 Case 1 Monte Carlo
//! (`mc_kernel_release_fraction`) and Crank's series
//! (`calculate_analytical_fraction_released`). Public API only. Shared by
//! `main.rs` and `tests/lesson_decay_walk.rs` through `#[path]`.
#![allow(dead_code)]

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::sample_dimensioned_gaussian_vector;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::sphere_fpt::sample_dimensionless_exit_time;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{sample_uniform_in_ball, HopOutcome, WoSWalker};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::{TrisoCell, TrisoRegion};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::{kernel_diffusion_coefficient, mc_kernel_release_fraction};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{try_get_diffusion_coeff_jiang, TrisoPebbleLayerMaterial};
use boon_lay::Nuclide;
use uom::si::area::square_meter;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{Area, ArealNumberDensity, DiffusionCoefficient, Length, ThermodynamicTemperature, Time};
use uom::si::length::{meter, micrometer};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::{hour, second};
use uom::ConstZero;

/// The CRP-6 Case 1 seed of `monte_carlo_test.rs` and the V&V record.
pub const CASE1_SEED: u64 = 0x0C1A_5EED;

pub fn celsius(c: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(c)
}
fn um(x: f64) -> Length {
    Length::new::<micrometer>(x)
}

/// The CRP-6 particle (`TrisoCell::new_crp6_geometry`, Hales et al. 2013)
/// at a uniform temperature.
pub fn crp6_cell(temp_c: f64) -> TrisoCell {
    let mut c = TrisoCell::new_crp6_geometry();
    c.set_uniform_temperature(celsius(temp_c));
    c
}

/// The five outer radii of the assembled cell, µm (kernel to OPyC).
pub fn radii_um(c: &TrisoCell) -> [f64; 5] {
    [c.get_fuel_radius(), c.get_buffer_radius(), c.get_ipyc_radius(), c.get_sic_radius(), c.get_opyc_radius()].map(|r| r.get::<micrometer>())
}

/// Region index 0 kernel .. 4 OPyC, 5 outside, as `get_triso_region` says.
pub fn region_index(r: TrisoRegion) -> usize {
    match r {
        TrisoRegion::Fuel => 0,
        TrisoRegion::Buffer => 1,
        TrisoRegion::IPyC => 2,
        TrisoRegion::SiC => 3,
        TrisoRegion::OPyC => 4,
        TrisoRegion::Outside => 5,
    }
}

/// Cs in the buffer at `temp_c`, zero fluence (Jiang et al. 2023, as
/// `try_get_diffusion_coeff_jiang` carries it), m²/s.
pub fn buffer_d(temp_c: f64) -> f64 {
    try_get_diffusion_coeff_jiang(TrisoPebbleLayerMaterial::Buffer, Nuclide::Cs137, celsius(temp_c), Some(ArealNumberDensity::ZERO))
        .expect("Cs in the buffer")
        .get::<square_meter_per_second>()
}

/// Cs in the kernel at `temp_c`, the D of `mc_kernel_release_fraction`.
pub fn kernel_d(temp_c: f64) -> f64 {
    kernel_diffusion_coefficient(Nuclide::Cs137, celsius(temp_c)).get::<square_meter_per_second>()
}

/// One Gaussian step: start and landing (µm, x y z) and the landing region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaussStep {
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub region: usize,
}

/// `n` atoms born uniformly in the buffer shell, each moved by ONE
/// Gaussian step of per-component variance 2 D dt
/// (`sample_dimensioned_gaussian_vector`, the legacy engine's step), and
/// where each lands, asked of the assembled cell. Each birth is put in the
/// z = 0 plane at a random angle (the landing region depends only on the
/// birth radius and the displacement, so this is the uniform shell
/// statistically, and it can be drawn on a slice).
pub fn gaussian_steps(cell: &TrisoCell, d: f64, dt: f64, n: usize, seed: u64) -> Vec<GaussStep> {
    let mut rng = OoRng64::from_u64(seed);
    let (r0, r1) = (cell.get_fuel_radius().get::<meter>(), cell.get_buffer_radius().get::<meter>());
    let var = Area::new::<square_meter>(2.0 * d * dt);
    (0..n)
        .map(|_| {
            let u = rng.rand_float();
            let r = (r0.powi(3) + u * (r1.powi(3) - r0.powi(3))).cbrt();
            let phi = 2.0 * std::f64::consts::PI * rng.rand_float();
            let from = [r * phi.cos(), r * phi.sin(), 0.0];
            let step = sample_dimensioned_gaussian_vector(&mut rng.0, var);
            let to = [from[0] + step[0].get::<meter>(), from[1] + step[1].get::<meter>(), step[2].get::<meter>()];
            let region = region_index(cell.get_triso_region(to.map(|x| Length::new::<meter>(x))));
            GaussStep { from: from.map(|x| x * 1e6), to: to.map(|x| x * 1e6), region }
        })
        .collect()
}

/// Fraction of steps landing in each region (0 kernel .. 5 outside).
pub fn region_fractions(steps: &[GaussStep]) -> [f64; 6] {
    let mut f = [0.0; 6];
    for s in steps {
        f[s.region] += 1.0 / steps.len() as f64;
    }
    f
}

/// One walker's hops in the CRP-6 kernel at `temp_c`, by `WoSWalker::hop`
/// in the assembled cell with `capture_eps` = 1e-3 R (as
/// `mc_kernel_release_fraction`): rows `[x, y, z, hop radius, t]` (µm, s),
/// the first row the birth, each next row where the hop landed and the
/// radius of the sphere it hopped on. Births and child generators as
/// `mc_kernel_release_fraction` makes them, from `seed`.
pub fn wos_paths(temp_c: f64, n: usize, seed: u64) -> Vec<Vec<[f64; 5]>> {
    let cell = crp6_cell(temp_c);
    let radius = cell.get_fuel_radius();
    let eps = radius * 1e-3;
    let mut master = OoRng64::from_u64(seed);
    (0..n)
        .map(|_| {
            let start = sample_uniform_in_ball(&mut master.0, radius);
            let mut w = WoSWalker::new(start, Nuclide::Cs137, OoRng64::from_u64(master.next_u64()));
            let row = |w: &WoSWalker, h: f64| [w.position[0].get::<micrometer>(), w.position[1].get::<micrometer>(), w.position[2].get::<micrometer>(), h, w.time.get::<second>()];
            let mut path = vec![row(&w, 0.0)];
            for _ in 0..100_000 {
                let h = (radius - w.radius()).get::<micrometer>();
                match w.hop(&cell, eps) {
                    HopOutcome::Stepped => path.push(row(&w, h)),
                    HopOutcome::ReachedInterface | HopOutcome::Released | HopOutcome::NoDiffusionData => break,
                }
            }
            path
        })
        .collect()
}

/// Release times (s) of `n` Cs-137 atoms born uniformly in the CRP-6
/// kernel: exactly the loop of `mc_kernel_release_fraction` (same births,
/// child generators and `walk_to_absorbing_sphere`), keeping each time so a
/// whole release curve comes from one set of walkers.
pub fn release_times(temp_c: f64, n: usize, seed: u64) -> Vec<f64> {
    let radius = um(212.5);
    let d = kernel_diffusion_coefficient(Nuclide::Cs137, celsius(temp_c));
    let mut master = OoRng64::from_u64(seed);
    (0..n)
        .map(|_| {
            let start = sample_uniform_in_ball(&mut master.0, radius);
            let mut w = WoSWalker::new(start, Nuclide::Cs137, OoRng64::from_u64(master.next_u64()));
            w.walk_to_absorbing_sphere(radius, d, radius * 1e-3).get::<second>()
        })
        .collect()
}

/// Fraction of `times` at or below each of `hours`.
pub fn fraction_by(times: &[f64], hours: &[f64]) -> Vec<f64> {
    hours.iter().map(|h| times.iter().filter(|&&t| t <= h * 3600.0).count() as f64 / times.len() as f64).collect()
}

/// Crank's series (200 terms) for Cs-137 in the 212.5 µm kernel.
pub fn crank(temp_c: f64, hours: f64) -> f64 {
    calculate_analytical_fraction_released(kernel_diffusion_coefficient(Nuclide::Cs137, celsius(temp_c)), um(212.5), Time::new::<hour>(hours), 200)
}

/// The CRP-6 Case 1 check itself: `mc_kernel_release_fraction` at 200 h,
/// 40 000 histories, the record's seed.
pub fn case1(temp_c: f64) -> f64 {
    mc_kernel_release_fraction(Nuclide::Cs137, um(212.5), celsius(temp_c), Time::new::<hour>(200.0), 40_000, CASE1_SEED)
}

/// A FIXED-STEP Gaussian walk out of the same kernel: per-component
/// sigma = `sigma_over_r` R per step (dt = sigma² / 2D), release counted
/// when an atom is outside R at the END of a step (the legacy scheme's
/// check). Returns the release time of each atom (s; infinite if still in
/// after `t_max_h`).
pub fn gaussian_release_times(temp_c: f64, sigma_over_r: f64, n: usize, seed: u64, t_max_h: f64) -> Vec<f64> {
    let r = 212.5e-6;
    let d = kernel_d(temp_c);
    let sigma = sigma_over_r * r;
    let dt = sigma * sigma / (2.0 * d);
    let steps = (t_max_h * 3600.0 / dt).ceil() as usize;
    let var = Area::new::<square_meter>(sigma * sigma);
    let mut master = OoRng64::from_u64(seed);
    (0..n)
        .map(|_| {
            let p0 = sample_uniform_in_ball(&mut master.0, um(212.5));
            let mut p = p0.map(|x| x.get::<meter>());
            for k in 1..=steps {
                let s = sample_dimensioned_gaussian_vector(&mut master.0, var);
                for (pi, si) in p.iter_mut().zip(s) {
                    *pi += si.get::<meter>();
                }
                if p.iter().map(|x| x * x).sum::<f64>() >= r * r {
                    return k as f64 * dt;
                }
            }
            f64::INFINITY
        })
        .collect()
}

/// Mean and standard error of `n` dimensionless hop times
/// `theta = D tau / R²` (`sample_dimensionless_exit_time`); exact mean 1/6.
pub fn theta_mean(n: usize, seed: u64) -> (f64, f64) {
    let mut s = OoRng64::from_u64(seed);
    let v: Vec<f64> = (0..n).map(|_| sample_dimensionless_exit_time(&mut s.0)).collect();
    let m = v.iter().sum::<f64>() / n as f64;
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n as f64 - 1.0);
    (m, (var / n as f64).sqrt())
}

/// The D a walker in the kernel is given by the assembled cell (the lookup
/// `hop` makes), m²/s.
pub fn cell_kernel_d(temp_c: f64) -> f64 {
    let c = crp6_cell(temp_c);
    let d: DiffusionCoefficient = c.try_get_diffusion_coefficient([Length::ZERO; 3], Nuclide::Cs137).expect("kernel D");
    d.get::<square_meter_per_second>()
}
