//! Lesson 32's reference: boon-lay's own multilayer Walk-on-Spheres walker
//! (`WoSWalker::walk_until_released`, the engine of
//! `first_passage::walk_on_spheres`) carrying atoms from the kernel through
//! buffer, IPyC, SiC and OPyC of the HTR-10 particle, at one uniform
//! temperature, with no decay. Public API only. Shared by `main.rs` and
//! `tests/lesson_barriers.rs` through `#[path]`.
//!
//! - Geometry: `TrisoCell::new` on `TrisoSpec::HTR10_LI2014`'s radii.
//! - Birth: uniform in the kernel (`sample_uniform_in_ball`).
//! - Interfaces: boon-lay's rule, `p = K D2 / (D1 + K D2)`, `K = 1`.
//! - Elements: Cs-137, Sr-90 and Ag-110m only. Kr-85 is never walked: its
//!   buffer coefficient is `todo!()` (gh:#541) and a walker reaching the
//!   buffer would panic.
//!
//! **Finding, 2026-10-22: the walker cannot carry an atom out of the whole
//! HTR-10 particle in practice.** The buffer's D (1e-8 m^2/s) is 10^5-10^7
//! times the PyC's, so at the buffer/IPyC interface the walker transmits
//! with p = D_PyC / (D_buffer + D_PyC) ~ 1e-6 and bounces back ~1e6 times
//! per entry, each bounce costing several hops; the IPyC then returns it to
//! the buffer thousands of times per arrival at the SiC. [`probe`] measures
//! this (steps spent per layer and IPyC entries in a fixed budget) instead
//! of claiming a release time it cannot reach. The lesson's check is
//! therefore [`exact_mean_exit`], an analytic mean exit time.
#![allow(dead_code)]

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
    sample_uniform_in_ball, WalkParams, WoSWalker,
};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;
use boon_lay::Nuclide;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::{meter, nanometer};
use uom::si::thermodynamic_temperature::degree_celsius;

/// The HTR-10 particle as boon-lay's walker sees it, at `t_c` °C throughout.
pub fn htr10_cell(t_c: f64) -> TrisoCell {
    let r = super::layers::htr10_radii();
    let m = |x: f64| Length::new::<meter>(x);
    let mut cell = TrisoCell::new(m(r[0]), m(r[1]), m(r[2]), m(r[3]), m(r[4]));
    cell.set_uniform_temperature(ThermodynamicTemperature::new::<degree_celsius>(t_c));
    cell
}

/// The walk parameters: boon-lay's CRP-6 defaults (`capture_eps` 10 nm,
/// reinsert at 2 eps, K = 1) with the step cap `max_steps`.
pub fn params(max_steps: u64) -> WalkParams {
    WalkParams { max_steps, ..WalkParams::crp6_default() }
}

/// A cost probe: one walk of `steps` steps (no release expected), reporting
/// the largest radius reached [m] and the steps spent in each of the five
/// layers (kernel, buffer, IPyC, SiC, OPyC), and the number of times the
/// walker entered the IPyC from the buffer.
pub fn probe(nuclide: Nuclide, t_c: f64, seed: u64, steps: u64) -> (f64, [u64; 5], u64, bool) {
    use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{radial_distance, HopOutcome};
    assert!(nuclide.get_z_a().0 != 36, "Kr is never walked (gh:#541)");
    let cell = htr10_cell(t_c);
    let p = params(steps);
    let radii = super::layers::htr10_radii();
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let pos = sample_uniform_in_ball(&mut s, cell.get_fuel_radius());
    let mut w = WoSWalker::new(pos, nuclide, OoRng64::from_u64(seed));
    let (mut rmax, mut per, mut entries, mut released) = (0.0f64, [0u64; 5], 0u64, false);
    let mut prev = 0usize;
    for _ in 0..steps {
        if w.step_multilayer(&cell, &p) == HopOutcome::Released {
            released = true;
            break;
        }
        let rr = radial_distance(w.position).get::<meter>();
        rmax = rmax.max(rr);
        let k = radii.iter().position(|&ro| rr <= ro).unwrap_or(4);
        per[k] += 1;
        if prev == 1 && k == 2 {
            entries += 1;
        }
        prev = k;
    }
    (rmax, per, entries, released)
}

/// The EXACT mean time to leave the particle [s] for atoms born uniformly in
/// the kernel, from the continuum diffusion equation with concentration and
/// flux continuous at every interface (boon-lay's K = 1) and an absorbing
/// outer surface. The mean exit time T(r) obeys div(D grad T) = -1; in a
/// sphere flux continuity gives D(r) T'(r) = -r/3 everywhere, so
///
///   T(r) = integral from r to R of s / (3 D(s)) ds,
///
/// and the volume average over the kernel (radius a) is
///
///   <T> = a^2 / (15 D_kernel) + sum over shells (r_out^2 - r_in^2) / (6 D_i).
///
/// An analytic solution written for this lesson as the check on its walk;
/// it is not a solver. `None` if any layer has no coefficient. Returns
/// (total, the five per-layer terms).
pub fn exact_mean_exit(nuclide: Nuclide, t_c: f64) -> Option<(f64, [f64; 5])> {
    use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial as M;
    let r = super::layers::htr10_radii();
    let mats = [M::KernelUO2, M::Buffer, M::PyC, M::SiC, M::PyC];
    let mut terms = [0.0; 5];
    for k in 0..5 {
        let d = super::layers::walker_d(mats[k], nuclide, t_c)?;
        terms[k] = if k == 0 { r[0] * r[0] / (15.0 * d) } else { (r[k] * r[k] - r[k - 1] * r[k - 1]) / (6.0 * d) };
    }
    Some((terms.iter().sum(), terms))
}

/// The walker's capture distance, for the page's note.
pub fn capture_eps_nm() -> f64 {
    WalkParams::crp6_default().capture_eps.get::<nanometer>()
}
