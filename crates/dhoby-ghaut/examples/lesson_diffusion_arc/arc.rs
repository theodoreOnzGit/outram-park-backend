//! The calls behind the diffusion arc's two lessons (gh:#1209): one sphere
//! three ways (Crank's series, finite volumes, the walk), and TRISO-ATOPS's
//! analytical release formulas measured against boon-lay's multilayer
//! finite-volume solution (`triso_atops_extensions::multilayer_diffusion`,
//! NOT a port) on the HTR-10 particle. Public API only; shared by `main.rs`
//! and `tests/lesson_diffusion_arc.rs` through `#[path]`.
//!
//! **Which D, and why.** A formula is judged against a numerical solution of
//! the same equation with the SAME coefficients, so that any difference is
//! the formula's assumptions and nothing else:
//!
//! - kernel: TRISO-ATOPS's own (`triso_atops_fork::diffusion::diffusion_coefficient`);
//! - silver in SiC: TRISO-ATOPS's own (`diffusion_coefficient_sic_ag`), the D
//!   its breakthrough formula uses;
//! - every other coating: boon-lay's walker table (`try_get_diffusion_coeff_jiang`,
//!   Jiang et al. 2023 as boon-lay cites it), because TRISO-ATOPS has none:
//!   it never diffuses Cs or Sr through a coating. No fluence.
//!
//! Krypton is not compared: TRISO-ATOPS's noble-gas R/B is an empirical fit,
//! not a diffusion solution~~, and boon-lay has no Kr D in the buffer
//! (gh:#541)~~ (**CORRECTED 2026-11-10, gh:#638:** boon-lay now has one, the
//! German `1e-8 m^2/s` of IAEA-TECDOC-978; the empirical fit is the reason).
#![allow(dead_code)]

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::kernel_diffusion_coefficient;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{
    try_get_diffusion_coeff_jiang, TrisoPebbleLayerMaterial,
};
use boon_lay::triso_atops_extensions::multilayer_diffusion::{LayeredDiffusion, LayeredSphere, SphereLayer};
use boon_lay::triso_atops_fork::diffusion::{diffusion_coefficient, diffusion_coefficient_sic_ag};
use boon_lay::triso_atops_fork::release_models::steady_state::booth_longlived;
use boon_lay::triso_atops_fork::release_models::transient::{booth_transient, breakthrough_model_transient};
use boon_lay::Nuclide;
use outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec;
use uom::si::area::square_meter;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{Area, DiffusionCoefficient, Frequency, Length, Ratio, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::{hour, second};

/// A D that makes a region well mixed next to every real one here (m²/s).
pub const WELL_MIXED_D: f64 = 1e-6;

pub fn dco(x: f64) -> DiffusionCoefficient {
    DiffusionCoefficient::new::<square_meter_per_second>(x)
}
pub fn celsius(c: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(c)
}
pub fn metres(x: f64) -> Length {
    Length::new::<meter>(x)
}

// ---------------------------------------------------------------- part A

/// CRP-6 Case 1's kernel radius, m.
pub const CRP6_KERNEL_M: f64 = 212.5e-6;

/// Cs-137 in the CRP-6 kernel at `t_c` (the D of the Case 1 check and of
/// the lesson "Walking on spheres").
pub fn crp6_d(t_c: f64) -> f64 {
    kernel_diffusion_coefficient(Nuclide::Cs137, celsius(t_c)).get::<square_meter_per_second>()
}

/// Crank's series (boon-lay's ported `booth_longlived`) for a sphere of
/// radius `a` with `D`, at each of `hours`.
pub fn crank_curve(d: f64, a: f64, hours: &[f64]) -> Vec<f64> {
    hours
        .iter()
        .map(|h| if *h == 0.0 { 0.0 } else { booth_longlived(dco(d), Time::new::<hour>(*h), metres(a)).get::<ratio>() })
        .collect()
}

/// A finite-volume run in a layered sphere, uniform start in layer 0, one D
/// per layer held constant, no decay, `steps` equal steps to the last of
/// `hours` (which must all be multiples of one step). Released fraction at
/// each of `hours`.
pub fn fv_curve(layers: &[(f64, usize)], d: &[f64], start_layers: usize, steps: usize, hours: &[f64]) -> Vec<f64> {
    let sphere = LayeredSphere::new(
        &layers.iter().map(|(r, n)| SphereLayer { outer_radius: metres(*r), cells: *n }).collect::<Vec<_>>(),
    )
    .expect("valid layers");
    let mut run = LayeredDiffusion::uniform_in_first_layers(sphere, start_layers);
    let t_end = hours.last().copied().unwrap_or(0.0);
    let dt_h = t_end / steps as f64;
    let dd: Vec<_> = d.iter().map(|x| dco(*x)).collect();
    let mut out = Vec::with_capacity(hours.len());
    let mut k = 0usize;
    for h in hours {
        let target = (h / dt_h).round() as usize;
        while k < target {
            run.step(Time::new::<hour>(dt_h), &dd, Frequency::new::<hertz>(0.0)).expect("step");
            k += 1;
        }
        out.push(run.released_fraction().get::<ratio>());
    }
    out
}

/// The exact concentration in a sphere (uniform start `c0 = 1`, `c = 0` at
/// the surface) at `x = r/a` and `tau = Dt/a²`: Crank (1975) eq. 6.18,
/// `c = (2/(π x)) Σ (−1)^{n+1}/n sin(nπx) exp(−n²π²τ)`, `terms` terms.
/// At `x = 0` the limit `2 Σ (−1)^{n+1} exp(−n²π²τ)` (the first draft had
/// an extra factor n here; the test caught it, and no page data uses x = 0).
pub fn exact_profile(x: f64, tau: f64, terms: usize) -> f64 {
    use std::f64::consts::PI;
    let mut s = 0.0;
    for n in 1..=terms {
        let nf = n as f64;
        let sign = if n % 2 == 1 { 1.0 } else { -1.0 };
        let decay = (-(nf * PI).powi(2) * tau).exp();
        s += if x == 0.0 { sign * decay } else { sign / nf * (nf * PI * x).sin() * decay };
    }
    if x == 0.0 { 2.0 * s } else { 2.0 / (PI * x) * s }
}

/// The finite-volume concentration profile (cell centres as `r/a`, cell
/// concentration over the starting one) in one sphere after `tau = Dt/a²`,
/// with `cells` cells and `steps` steps.
pub fn fv_profile(cells: usize, tau: f64, steps: usize) -> (Vec<f64>, Vec<f64>) {
    let a = 1e-4;
    let d = 1e-12;
    let sphere = LayeredSphere::new(&[SphereLayer { outer_radius: metres(a), cells }]).expect("valid");
    let centres: Vec<f64> = sphere.centres_m().iter().map(|r| r / a).collect();
    let faces: Vec<f64> = (0..=cells).map(|i| a * i as f64 / cells as f64).collect();
    let mut run = LayeredDiffusion::uniform_in_kernel(sphere);
    let t = tau * a * a / d;
    for _ in 0..steps {
        run.step(Time::new::<second>(t / steps as f64), &[dco(d)], Frequency::new::<hertz>(0.0)).expect("step");
    }
    // Atoms per cell over the cell's share of the starting uniform inventory.
    let atoms = run.cell_atoms();
    let vol_total = a.powi(3);
    let c: Vec<f64> = atoms
        .iter()
        .enumerate()
        .map(|(i, n)| n / ((faces[i + 1].powi(3) - faces[i].powi(3)) / vol_total))
        .collect();
    (centres, c)
}

// ---------------------------------------------------------------- part B

/// The three compared elements: name, Z, the walker's nuclide.
pub fn elements() -> [(&'static str, u32, Nuclide); 3] {
    [("Cs", 55, Nuclide::Cs137), ("Sr", 38, Nuclide::Sr90), ("Ag", 47, Nuclide::Ag110m)]
}

/// HTR-10 TRISO outer radii, m (kernel, buffer, IPyC, SiC, OPyC), from
/// outram-mc-libs' `TrisoSpec::HTR10_LI2014` (as chapter 4's lessons use).
pub fn htr10_radii() -> [f64; 5] {
    let s = TrisoSpec::HTR10_LI2014;
    [s.kernel / 100.0, s.buffer / 100.0, s.ipyc / 100.0, s.sic / 100.0, s.opyc / 100.0]
}

/// Cells per layer of the base mesh (kernel core, buffer, IPyC, SiC, OPyC).
/// First run 2026-11-04: [100, 20, 10, 20, 10], refined after the first run
/// (see `main.rs`).
pub const BASE_CELLS: [usize; 5] = [100, 20, 20, 80, 20];

/// Cells in the kernel's outer shell (see [`kernel_shell_m`]).
pub const SHELL_CELLS: usize = 200;

/// Thickness of the finely meshed outer shell of the kernel: ten diffusion
/// lengths √(D t_end), at most half the kernel radius.
pub fn kernel_shell_m(d_kernel: f64, t_end_h: f64, kernel_radius: f64) -> f64 {
    (10.0 * (d_kernel * t_end_h * 3600.0).sqrt()).min(0.5 * kernel_radius)
}

/// D per layer [m²/s] for element `z` at `t_c`, by the rule in the module
/// doc: kernel from TRISO-ATOPS, Ag in SiC from TRISO-ATOPS, the rest from
/// boon-lay's walker table (PyC for both IPyC and OPyC).
pub fn d_layers(z: u32, nuclide: Nuclide, t_c: f64) -> [f64; 5] {
    let t = celsius(t_c);
    let kernel = diffusion_coefficient(z, t, t).kernel.get::<square_meter_per_second>();
    let walker = |m| {
        try_get_diffusion_coeff_jiang(m, nuclide, t, None).expect("filled for Cs, Sr, Ag").get::<square_meter_per_second>()
    };
    let sic = if z == 47 {
        diffusion_coefficient_sic_ag(t).get::<square_meter_per_second>()
    } else {
        walker(TrisoPebbleLayerMaterial::SiC)
    };
    let pyc = walker(TrisoPebbleLayerMaterial::PyC);
    [kernel, walker(TrisoPebbleLayerMaterial::Buffer), pyc, sic, pyc]
}

/// TRISO-ATOPS's failed-particle kernel release for Cs or Sr after `hours`
/// at constant `t_c`: `booth_transient(∫D dt / a²)` on the kernel radius,
/// exactly what its accident path calls (`release_fraction_transient`, kernel
/// material, upstream readings; ∫D dt is D t at constant temperature).
pub fn atops_failed_kernel(z: u32, t_c: f64, hours: f64) -> f64 {
    let a = htr10_radii()[0];
    let d = diffusion_coefficient(z, celsius(t_c), celsius(t_c)).kernel.get::<square_meter_per_second>();
    let int = d * hours * 3600.0;
    booth_transient(Ratio::new::<ratio>(int / (a * a))).get::<ratio>()
}

/// TRISO-ATOPS's silver release from EVERY particle after `hours` at
/// `t_c`: `breakthrough_model_transient` through the SiC thickness with the
/// kernel radius, as its accident path calls it (upstream readings).
pub fn atops_silver(t_c: f64, hours: f64) -> f64 {
    let r = htr10_radii();
    let a_sic = r[3] - r[2];
    let d = diffusion_coefficient_sic_ag(celsius(t_c)).get::<square_meter_per_second>();
    let int = d * hours * 3600.0;
    if int == 0.0 {
        return 0.0;
    }
    breakthrough_model_transient(
        Ratio::new::<ratio>(int / (a_sic * a_sic)),
        Area::new::<square_meter>(int),
        metres(a_sic),
        metres(r[0]),
    )
    .get::<ratio>()
}

/// Which particle the finite volumes describe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Particle {
    /// A failed particle as TRISO-ATOPS treats it: the bare kernel.
    BareKernel,
    /// The intact particle, all five layers with their own D.
    Intact,
    /// Silver's breakthrough assumptions put on a mesh: kernel, buffer and
    /// IPyC well mixed (D = [`WELL_MIXED_D`]), then the SiC, sink at its outer
    /// surface (no OPyC).
    WellMixedInsideSic,
}

/// Mesh (outer radius, cells), D per layer and the number of leading layers
/// the inventory starts in, for `particle`, element `z` at `t_c` over
/// `t_end_h`, the base cells times `refine`. The kernel is a core and a fine
/// outer shell with the same D ([`kernel_shell_m`]).
pub fn setup(particle: Particle, z: u32, nuclide: Nuclide, t_c: f64, t_end_h: f64, refine: usize) -> (Vec<(f64, usize)>, Vec<f64>, usize) {
    let r = htr10_radii();
    let d = d_layers(z, nuclide, t_c);
    let c: Vec<usize> = BASE_CELLS.iter().map(|n| n * refine).collect();
    let shell = kernel_shell_m(d[0], t_end_h, r[0]);
    let kernel = vec![(r[0] - shell, c[0]), (r[0], SHELL_CELLS * refine)];
    match particle {
        Particle::BareKernel => (kernel, vec![d[0], d[0]], 2),
        Particle::Intact => {
            let mut layers = kernel;
            layers.extend((1..5).map(|i| (r[i], c[i])));
            (layers, vec![d[0], d[0], d[1], d[2], d[3], d[4]], 2)
        }
        Particle::WellMixedInsideSic => (
            (0..4).map(|i| (r[i], c[i])).collect(),
            vec![WELL_MIXED_D, WELL_MIXED_D, WELL_MIXED_D, d[3]],
            1,
        ),
    }
}
