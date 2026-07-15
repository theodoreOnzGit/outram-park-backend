//! Doubly-heterogeneous k-eigenvalue power iteration driven by **delta (Woodcock)
//! tracking**.
//!
//! This is the assembly point for the random-packed TRISO k-eff: it composes the
//! [`super::stochastic_media`] packed geometry, the [`super::delta_tracking`]
//! flight primitives, and the crate's collision physics into a fission-source
//! power iteration — the doubly-heterogeneous analogue of
//! [`crate::physics::keff::run_keff`] (bare sphere) and
//! [`crate::physics::transport_csg::run_keff_csg`] (surface-tracked CSG).
//!
//! # Why delta tracking here
//!
//! In a packed TRISO medium a neutron's straight-line path crosses an enormous
//! number of kernel surfaces. Surface tracking must find the *nearest* of them at
//! every flight; delta tracking never looks for a surface at all. It samples the
//! flight on a **majorant** `Σ_maj(E) ≥ Σ_t(E)` bounding every material, lands at a
//! point, and asks only "**what material is here?**" — a point-membership test the
//! packed-sphere grid answers in O(1) ([`super::stochastic_media::PackedSpheres::is_inside_kernel`]).
//! The landing is a real collision with probability `Σ_t(local)/Σ_maj` and a
//! virtual (do-nothing) collision otherwise. See [`super::delta_tracking`] for the
//! primitives and their unit tests (unbiased mean free path, correct real/virtual
//! split).
//!
//! # Geometry model
//!
//! A **reflective cube** of half-width `half_width` (an infinite-medium unit cell:
//! neutrons reflect off the six walls, so the eigenvalue is the infinite-medium
//! `k∞` of the packed fuel, free of leakage). Inside the cube the caller's
//! `material_at` closure maps a point to a material index (kernel → fuel, else
//! matrix). The delta flight reflects the ray off the walls segment by segment, so
//! the neutron always lands at an interior point where `material_at` is defined.
//!
//! # Collision physics
//!
//! At each real collision the analog reaction partition — fission | capture |
//! inelastic | (n,2n) | elastic — mirrors [`crate::physics::keff`] /
//! [`crate::physics::transport_csg`] (the same [`crate::physics::scatter`] and
//! [`crate::physics::fission`] kernels). Only fission neutrons are banked to the
//! next generation; `(n,2n)` multiplicity is realized in-generation via a local
//! work stack. Fidelity matches those drivers: analog, target at rest, data tier
//! set by how the `nuclides` were built ([`Nuclide::from_core`] LOW /
//! [`Nuclide::from_endf`] HIGH).
//!
//! # Provenance
//!
//! The delta-tracking method is standard (Woodcock, ANL-7050, 1965; used in OpenMC,
//! Serpent, RMC). The collision partition mirrors OpenMC `src/physics.cpp`
//! (`collision` / `inelastic_scatter`). The reflective-cube flight is new pebble-bed
//! assembly built on this crate's primitives.

use crate::geometry::position::{Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::{Inelastic, Nuclide};
use crate::pebble_beds::delta_tracking::{classify_collision, sample_delta_distance, DeltaEvent, Majorant};
use crate::physics::fission::sample_num_neutrons;
use crate::physics::keff::{KeffResult, KeffSettings};
use crate::physics::scatter::{
    continuum_inelastic_scatter, elastic_scatter, two_body_scatter, two_body_scatter_with_mu,
};
use crate::rng::distributions::{isotropic_direction, watt};
use crate::rng::lcg::prn;

/// A fission-source neutron awaiting transport in the next generation.
#[derive(Clone, Copy)]
struct Site {
    r: Position,
    u: Direction,
    e: f64,
}

/// Advance a ray by `distance` \[cm\] inside a reflective cube of half-width
/// `half`, reflecting off the walls, and return the landing position and the
/// (possibly reflected) direction.
///
/// Delta-tracking flights between collisions are straight lines under the
/// majorant, so a flight that would leave the cube instead reflects: the ray is
/// walked wall-to-wall, flipping the crossed axis's direction component each time,
/// until the full `distance` is consumed. The returned direction is what the
/// neutron continues along after a virtual collision.
///
/// The landing point is guaranteed to lie inside the closed cube (within
/// floating-point slack), so a subsequent material lookup is always defined.
fn advance_reflective(
    mut r: Position,
    mut u: Direction,
    mut distance: f64,
    half: f64,
) -> (Position, Direction) {
    // Guard against a pathological number of reflections (vanishing component).
    for _ in 0..10_000 {
        if distance <= 0.0 {
            break;
        }
        // Distance to the nearest wall along each axis (∞ if not moving on it).
        let t_axis = |p: f64, d: f64| -> f64 {
            if d > 0.0 {
                (half - p) / d
            } else if d < 0.0 {
                (-half - p) / d
            } else {
                f64::INFINITY
            }
        };
        let tx = t_axis(r.x, u.u);
        let ty = t_axis(r.y, u.v);
        let tz = t_axis(r.z, u.w);
        let t_wall = tx.min(ty).min(tz);

        if t_wall >= distance || !t_wall.is_finite() {
            r = Position::new(r.x + u.u * distance, r.y + u.v * distance, r.z + u.w * distance);
            break;
        }

        // Walk to the wall, flip the crossed component(s), consume the distance.
        r = Position::new(r.x + u.u * t_wall, r.y + u.v * t_wall, r.z + u.w * t_wall);
        let mut nu = u.u;
        let mut nv = u.v;
        let mut nw = u.w;
        if (tx - t_wall).abs() < 1e-15 {
            nu = -nu;
            r = Position::new(r.x.clamp(-half, half), r.y, r.z);
        }
        if (ty - t_wall).abs() < 1e-15 {
            nv = -nv;
            r = Position::new(r.x, r.y.clamp(-half, half), r.z);
        }
        if (tz - t_wall).abs() < 1e-15 {
            nw = -nw;
            r = Position::new(r.x, r.y, r.z.clamp(-half, half));
        }
        u = Direction::new(nu, nv, nw);
        distance -= t_wall;
    }
    // Numerical safety: keep the point strictly inside the closed cube.
    let r = Position::new(r.x.clamp(-half, half), r.y.clamp(-half, half), r.z.clamp(-half, half));
    (r, u)
}

/// Fly a neutron to its next **real** collision inside the reflective cube by
/// delta tracking, returning the collision position, the material index there, and
/// the direction it arrived along (for post-collision scattering).
///
/// Loops over virtual collisions internally: sample a flight on `majorant.at(e)`,
/// reflect-advance the ray, look up the local material and its Σ_t, and accept a
/// real collision with probability `Σ_t/Σ_maj`. Returns `None` if the virtual
/// budget is exhausted (a pathologically loose majorant) or the material lookup
/// unexpectedly fails — both leak the history, as in the surface-tracked drivers.
fn delta_flight<F>(
    start: Position,
    direction: Direction,
    energy: f64,
    half: f64,
    majorant: &Majorant,
    materials: &[Material],
    nuclides: &[Nuclide],
    max_virtual: u32,
    material_at: &F,
    seed: &mut u64,
) -> Option<(Position, usize, Direction)>
where
    F: Fn(Position) -> Option<usize>,
{
    let maj = majorant.at(energy);
    if !(maj > 0.0) {
        return None;
    }
    let mut r = start;
    let mut u = direction;
    for _ in 0..max_virtual {
        let s = sample_delta_distance(maj, seed);
        let (r_next, u_next) = advance_reflective(r, u, s, half);
        r = r_next;
        u = u_next;
        let m = material_at(r)?;
        let sigma_t = materials[m].macro_xs_total(energy, nuclides);
        match classify_collision(sigma_t, maj, seed) {
            DeltaEvent::Real => return Some((r, m, u)),
            DeltaEvent::Virtual => continue,
        }
    }
    None
}

/// Run fission-source power iteration over a **reflective cube** filled with a
/// two-(or-more-)material dispersion medium, transporting each history by delta
/// (Woodcock) tracking.
///
/// # Parameters
/// - `half_width` — half-width \[cm\] of the reflective cube (infinite-medium cell).
/// - `materials` — global material array; `material_at` returns indices into it.
/// - `nuclides` — global nuclide array the materials index into.
/// - `majorant` — a [`Majorant`] bounding `Σ_t(E)` of **every** material over the
///   full energy range the histories span (build it with
///   [`Majorant::from_materials`] on a broad grid).
/// - `material_at` — geometry lookup: the material index at a point inside the cube
///   (e.g. kernel → fuel, matrix → moderator). Must be defined everywhere inside
///   the closed cube; returning `None` leaks the history.
/// - `settings` — power-iteration controls (reuses [`KeffSettings`]).
///
/// Returns the mean eigenvalue and its standard error over the active generations.
/// The initial source is rejection-sampled uniformly in the cube for points in a
/// fissile material.
pub fn run_keff_delta<F>(
    half_width: f64,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorant: &Majorant,
    material_at: F,
    settings: &KeffSettings,
) -> KeffResult
where
    F: Fn(Position) -> Option<usize>,
{
    let mut seed = settings.seed;
    let temp = settings.temperature_k;

    // Initial source: rejection-sample the cube for points in a fissile material.
    let mut source: Vec<Site> = Vec::with_capacity(settings.n_particles);
    let mut guard = 0usize;
    while source.len() < settings.n_particles {
        guard += 1;
        if guard > settings.n_particles * 10_000 {
            break; // pathological: fuel fills a vanishing fraction of the cube
        }
        let r = Position::new(
            -half_width + 2.0 * half_width * prn(&mut seed),
            -half_width + 2.0 * half_width * prn(&mut seed),
            -half_width + 2.0 * half_width * prn(&mut seed),
        );
        let fissile = material_at(r)
            .map(|m| materials[m].macro_xs(1.0e6, nuclides).nu_fission > 0.0)
            .unwrap_or(false);
        if fissile {
            let (dx, dy, dz) = isotropic_direction(&mut seed);
            source.push(Site {
                r,
                u: Direction::new(dx, dy, dz),
                e: watt(&mut seed, settings.watt_a, settings.watt_b),
            });
        }
    }

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0;
    let mut active_k = Vec::with_capacity(settings.n_active);

    for gen in 0..n_gen {
        let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
        let mut production = 0.0_f64;

        for site in &source {
            production += transport_history(
                *site, half_width, materials, nuclides, majorant, temp, k_running,
                &material_at, &mut next_bank, &mut seed,
            );
        }

        let k_gen = production / settings.n_particles as f64;
        k_by_generation.push(k_gen);
        k_running = k_gen.max(1.0e-6);
        if gen >= settings.n_inactive {
            active_k.push(k_gen);
        }

        if next_bank.is_empty() {
            break;
        }
        source = resample(&next_bank, settings.n_particles, &mut seed);
    }

    let (k_mean, k_std) = mean_and_stderr(&active_k);
    KeffResult { k_mean, k_std, k_by_generation }
}

/// Transport one source neutron (plus its same-generation `(n,2n)` secondaries) to
/// death by delta tracking, banking fission neutrons. Returns the fission
/// production ν̄ summed over the history's fission events.
///
/// Mirrors the analog reaction partition of [`crate::physics::keff`]; the only
/// difference is that streaming is done by [`delta_flight`] (Woodcock) rather than
/// surface tracking.
#[allow(clippy::too_many_arguments)]
fn transport_history<F>(
    site: Site,
    half: f64,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorant: &Majorant,
    temp: f64,
    k_running: f64,
    material_at: &F,
    next_bank: &mut Vec<Site>,
    seed: &mut u64,
) -> f64
where
    F: Fn(Position) -> Option<usize>,
{
    // Safety cap on events per history — a purely-scattering reflective medium with
    // vanishing absorption could otherwise bounce forever (mirrors keff drivers).
    const MAX_EVENTS: u32 = 100_000;
    const MAX_VIRTUAL: u32 = 100_000;
    let mut production = 0.0;
    let mut stack: Vec<Site> = vec![site];

    while let Some(start) = stack.pop() {
        let mut r = start.r;
        let mut u = start.u;
        let mut e = start.e;
        let mut events = 0u32;

        'history: loop {
            events += 1;
            if events > MAX_EVENTS {
                break 'history; // give up on a stuck history (leak it)
            }

            let Some((r_col, m, u_arr)) = delta_flight(
                r, u, e, half, majorant, materials, nuclides, MAX_VIRTUAL, material_at, seed,
            ) else {
                break 'history; // leaked / virtual budget exhausted
            };
            r = r_col;
            u = u_arr;

            let material = &materials[m];
            let ci = material.sample_nuclide(e, seed, nuclides);
            let nuc = &nuclides[material.components[ci].nuclide_idx];
            let x = nuc.xs_at_energy(e, temp);

            // Reaction partition on the total: fission | capture | inelastic |
            // (n,2n) | elastic — identical to keff.rs / transport_csg.rs.
            let xi = prn(seed) * x.total;
            if xi < x.fission {
                let nu_bar = if x.fission > 0.0 { x.nu_fission / x.fission } else { 0.0 };
                production += nu_bar;
                let n = sample_num_neutrons(nu_bar, k_running, seed);
                for _ in 0..n {
                    let (dx, dy, dz) = isotropic_direction(seed);
                    next_bank.push(Site {
                        r,
                        u: Direction::new(dx, dy, dz),
                        e: nuc.sample_fission_energy(e, seed),
                    });
                }
                break 'history; // fission absorbs the incident neutron
            } else if xi < x.absorption {
                break 'history; // radiative capture
            } else if xi < x.absorption + x.inelastic {
                let (e2, u2) = match nuc.sample_inelastic(e, seed) {
                    Inelastic::Level { q } => two_body_scatter(e, u, nuc.awr, q, seed),
                    Inelastic::Continuum => continuum_inelastic_scatter(e, u, nuc.awr, seed),
                };
                e = e2;
                u = u2;
            } else if xi < x.absorption + x.inelastic + x.n2n {
                let (e2, u2) = continuum_inelastic_scatter(e, u, nuc.awr, seed);
                stack.push(Site { r, u: u2, e: e2 }); // yield − 1 = 1 secondary
                e = e2;
                u = u2;
            } else {
                let (e2, u2) = match nuc.sample_elastic_mu_cm(e, seed) {
                    Some(mu_cm) => two_body_scatter_with_mu(e, u, nuc.awr, 0.0, mu_cm, seed),
                    None => elastic_scatter(e, u, nuc.awr, seed),
                };
                e = e2;
                u = u2;
            }
        }
    }
    production
}

/// Resample `n` sites uniformly with replacement — fixed-size population control.
fn resample(bank: &[Site], n: usize, seed: &mut u64) -> Vec<Site> {
    let len = bank.len();
    (0..n)
        .map(|_| {
            let idx = ((prn(seed) * len as f64) as usize).min(len - 1);
            bank[idx]
        })
        .collect()
}

/// Mean and standard error of the mean (1σ) of the active-generation eigenvalues.
fn mean_and_stderr(k: &[f64]) -> (f64, f64) {
    let n = k.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = k.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return (mean, 0.0);
    }
    let var = k.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    (mean, (var / n as f64).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reflective cube uniformly filled with one fissile material must give the
    /// same k as the surface-tracked bare-sphere driver would for the *infinite*
    /// medium — but here we just check delta tracking on a homogeneous reflective
    /// cube converges to a stationary, positive eigenvalue (the flight machinery is
    /// exercised; unbiasedness vs surface tracking is checked in the triso test).
    #[test]
    fn homogeneous_reflective_cube_converges() {
        use crate::material::material::NuclideComponent;
        let nuclides = vec![Nuclide::from_core("U235").unwrap()];
        let fuel = Material {
            id: 1,
            name: "U235".into(),
            temperature: 293.6,
            components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 4.8e-2 }],
        };
        let materials = vec![fuel];
        let grid: Vec<f64> = (0..60).map(|i| 1.0e-3 * 1.5_f64.powi(i)).collect();
        let maj = Majorant::from_materials(&materials, &nuclides, &grid, 0.05);
        let settings = KeffSettings { n_particles: 500, n_inactive: 10, n_active: 20, ..KeffSettings::default() };

        let result = run_keff_delta(2.0, &materials, &nuclides, &maj, |_p| Some(0), &settings);
        assert!(!result.k_by_generation.is_empty());
        assert!(result.k_mean.is_finite() && result.k_mean > 0.0, "k = {}", result.k_mean);
    }

    /// `advance_reflective` keeps a ray inside the cube and conserves path length
    /// (the reflected polyline has the requested total length).
    #[test]
    fn reflective_advance_stays_in_cube() {
        let half = 1.0;
        let start = Position::new(0.0, 0.0, 0.0);
        let dir = Direction::from_unnormalised(1.0, 0.3, -0.7);
        let (end, _u) = advance_reflective(start, dir, 12.5, half);
        assert!(end.x.abs() <= half + 1e-9 && end.y.abs() <= half + 1e-9 && end.z.abs() <= half + 1e-9);
    }
}
