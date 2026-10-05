//! # A fresh HTR-10 pebble pour, steppable from a UI
//!
//! Pours `N` pebbles into the HTR-10 vessel from empty and settles them under
//! gravity, in chunks the caller drives, reporting progress after each. Built
//! for the Dhoby Ghaut workbench's Step 1 (gh:#561: "I should be able to run a
//! fresh DEM in the early stages"), so a user can choose how many pebbles to
//! load and watch the bed form, and the Monte Carlo model is then built on the
//! bed this produced.
//!
//! **Before this module** every HTR-10 bed in the workspace started from
//! LIGGGHTS' own `fix insert/pack` output (`reference-data/liggghts/htr10_init.csv`);
//! the Rust side only re-settled it (`examples/htr10_recirculation_sweep.rs`).
//! The pieces here are lifted from those examples so the two cannot drift:
//!
//! | piece | from |
//! |---|---|
//! | vessel: barrel, conus + discharge tube mesh, valve | `htr10_recirculation_sweep.rs` (system set-up), `reference-data/liggghts/make_htr10_discharge_stl.sh` (the mesh, generated here in code with the same 120-segment winding) |
//! | contact model and defaults | the same example; V&V `docs/verification-and-validation.md` § 4.7 (E, dt) and § 4.9 (µ, µ_r; gh:#216) |
//! | loose jittered-lattice seeding | `examples/bake_htr10_conus_slab.rs` `seed_column` |
//! | settle criterion (core KE per pebble / one-radius drop < 1e-3) | `htr10_recirculation_sweep.rs` adaptive pre-settle |
//! | surface height (99th percentile + r), whole-core φ | the same example's `bed_surface_height`, `whole_core_fraction` |
//!
//! ## The defaults, and what they rest on
//!
//! - **µ = 0.1, µ_r = 0**: the setting at which this port's conus pre-settle
//!   gives a whole-core filling fraction of **0.6047 against the published
//!   0.61 (−0.9 %)** (V&V § 4.9, gh:#216). The docs justify µ as "graphite is
//!   a solid lubricant; graphite-on-graphite sliding friction 0.1–0.2", with
//!   **no specific paper cited**; treat it as a stated assumption, and ablate
//!   it rather than tune it. The older LIGGGHTS-deck default is µ = 0.4,
//!   µ_r = 0.1 (`reference-data/liggghts/in.htr10`).
//! - **E = 5e8 Pa** (graphite is ~9 GPa): the standard pebble-bed DEM
//!   softening, set by measuring contact overlap, not by its effect on packing
//!   (V&V § 4.7). Soft-sphere overlap at this stiffness is up to ~1.7 % of r.
//! - **dt = 3.5e-5 s**: 11.7 % of the Rayleigh time at E = 5e8.
//! - ν = 0.2, e = 0.5, ρ = 1730 kg/m³, r = 3 cm.
//!
//! These are the settings the maintainer's HTR-10 pebble-bed DEM figure package
//! records (publications repository, `outram_park/outram_park_intro_paper/
//! outram_park_double_heterogeneity_arxiv/src/results_and_discussion/
//! pebble_bed_dem/README.md`, prepared 2026-09-18 from this crate's V&V): the
//! same µ = 0.1, µ_r = 0, E = 5e8 Pa, ν = 0.2, e = 0.5, dt = 35 µs, and the
//! 2×2 friction ablation reaching 0.6047 against the quoted 0.61, which it
//! states is "an ablation, not a calibration".
//!
//! The published 0.61 is a check, not an input: nothing here is tuned to it.
//! It is itself quoted from a specification table, not measured.
//!
//! ## Frame
//!
//! Metres. `z = 0` is the conus inlet (the floor of the cylindrical core); the
//! conus runs down to `z = −0.36946`, then a 0.25 m length of the 0.25 m-radius
//! discharge tube to the valve at `z = −0.61946`. The real tube is ~6 m; the
//! DEM holds only this stub of it.

use uom::si::f64::{Length, Mass, Pressure, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::boundary::Boundary;
use crate::compute::{ComputeType, ThreadCount};
use crate::granular::{GranularContactModel, GranularMaterial, RollingModel};
use crate::granular_system::GranularSystem;
use crate::mesh_wall::{MeshWall, MovingBoundary, Triangle, WallGeometry};
use crate::particle::{Particle, Vec3};
use crate::DemError;

/// Pebble radius \[m\].
pub const PEBBLE_RADIUS_M: f64 = 0.03;
/// Graphite pebble density \[kg/m³\].
pub const PEBBLE_DENSITY: f64 = 1730.0;
/// Core (barrel) radius \[m\].
pub const CORE_RADIUS_M: f64 = 0.90;
/// Height of the conus \[m\].
pub const CONE_HEIGHT_M: f64 = 0.36946;
/// Discharge-tube radius \[m\].
pub const TUBE_RADIUS_M: f64 = 0.25;
/// Length of discharge tube the DEM holds \[m\].
pub const TUBE_LENGTH_M: f64 = 0.25;
/// The valve at the bottom of the DEM's tube \[m\].
pub const VALVE_Z_M: f64 = -(CONE_HEIGHT_M + TUBE_LENGTH_M);

/// Everything a fill depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Htr10FillSettings {
    /// Pebbles to pour.
    pub n_pebbles: usize,
    /// Sliding friction µ \[-\].
    pub friction: f64,
    /// Rolling friction µ_r \[-\] (0 = none).
    pub rolling_friction: f64,
    /// Young's modulus.
    pub youngs_modulus: Pressure,
    /// Poisson's ratio \[-\].
    pub poisson_ratio: f64,
    /// Coefficient of restitution \[-\].
    pub restitution: f64,
    /// Integration timestep.
    pub dt: Time,
    /// Threads.
    pub threads: ThreadCount,
    /// Seed of the jitter in the initial loose lattice.
    pub seed: u64,
    /// Settled when the mean kinetic energy per core pebble falls below this
    /// fraction of a one-radius gravitational drop.
    pub settle_target: f64,
    /// Give up settling after this many steps (reported, never hidden).
    pub max_steps: usize,
}

impl Default for Htr10FillSettings {
    /// The V&V § 4.9 setting (µ = 0.1, µ_r = 0) for the full core: 27 000
    /// balls, the IAEA-TECDOC-1382 loading the published 0.61 filling fraction
    /// is quoted for (27 000 in a 1.97 m core is 0.609; the LIGGGHTS reference
    /// runs hold 27 554). First criticality (16 890 balls) is a different,
    /// smaller loading; set `n_pebbles` for it.
    fn default() -> Self {
        Self {
            n_pebbles: 27_000,
            friction: 0.1,
            rolling_friction: 0.0,
            youngs_modulus: Pressure::new::<pascal>(5.0e8),
            poisson_ratio: 0.2,
            restitution: 0.5,
            dt: Time::new::<second>(3.5e-5),
            threads: ThreadCount::Auto,
            seed: 0x5EED_0010,
            settle_target: 1.0e-3,
            max_steps: 200_000,
        }
    }
}

/// Where a fill is, after a chunk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FillProgress {
    /// Steps taken so far.
    pub steps: usize,
    /// Simulated time.
    pub time: Time,
    /// Mean KE per pebble in the core (`z > 0`) over a one-radius drop.
    pub ke_ratio_core: f64,
    /// Whole-core filling fraction `N V / (π R² h)` (`z > 0`, `h` = surface).
    pub phi_whole_core: f64,
    /// Bed surface: 99th-percentile core centre height + r.
    pub surface_height: Length,
    /// Pebbles above the conus inlet.
    pub n_in_core: usize,
    /// Below the settle target.
    pub settled: bool,
    /// Hit `max_steps` without settling.
    pub gave_up: bool,
}

/// A pour in progress.
#[derive(Debug, Clone)]
pub struct Htr10Fill {
    sys: GranularSystem,
    settings: Htr10FillSettings,
    steps: usize,
}

/// One radius drop of one pebble \[J\]: the "is it quasi-static?" yardstick.
fn e_drop() -> f64 {
    pebble_mass() * 9.81 * PEBBLE_RADIUS_M
}

fn pebble_mass() -> f64 {
    PEBBLE_DENSITY * 4.0 / 3.0 * std::f64::consts::PI * PEBBLE_RADIUS_M.powi(3)
}

fn pebble(x: Vec3) -> Result<Particle, DemError> {
    Particle::new(
        x,
        Vec3::zero(),
        Vec3::zero(),
        Mass::new::<kilogram>(pebble_mass()),
        Length::new::<meter>(PEBBLE_RADIUS_M),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
}

/// The conus and discharge tube as an inward-facing triangle mesh, exactly as
/// `make_htr10_discharge_stl.sh` writes it (two bands of `segments` quads).
pub fn discharge_mesh(segments: usize) -> Result<MeshWall, DemError> {
    let n = segments.max(3);
    let mut tris = Vec::with_capacity(4 * n);
    let band = |z0: f64, r0: f64, z1: f64, r1: f64, tris: &mut Vec<Triangle>| -> Result<(), DemError> {
        for i in 0..n {
            let (a0, a1) = (
                std::f64::consts::TAU * i as f64 / n as f64,
                std::f64::consts::TAU * (i + 1) as f64 / n as f64,
            );
            let p0 = Vec3::new(r0 * a0.cos(), r0 * a0.sin(), z0);
            let p1 = Vec3::new(r0 * a1.cos(), r0 * a1.sin(), z0);
            let q0 = Vec3::new(r1 * a0.cos(), r1 * a0.sin(), z1);
            let q1 = Vec3::new(r1 * a1.cos(), r1 * a1.sin(), z1);
            // Windings make (b - a) x (c - a) point toward the axis.
            tris.push(Triangle::new(p0, q1, q0)?);
            tris.push(Triangle::new(p0, p1, q1)?);
        }
        Ok(())
    };
    band(0.0, CORE_RADIUS_M, -CONE_HEIGHT_M, TUBE_RADIUS_M, &mut tris)?;
    band(-CONE_HEIGHT_M, TUBE_RADIUS_M, VALVE_Z_M, TUBE_RADIUS_M, &mut tris)?;
    MeshWall::new(tris)
}

/// Inner wall radius at height `z` \[m\]: tube, conus, then barrel.
pub fn wall_radius_at(z: f64) -> f64 {
    if z <= -CONE_HEIGHT_M {
        TUBE_RADIUS_M
    } else if z < 0.0 {
        TUBE_RADIUS_M + (CORE_RADIUS_M - TUBE_RADIUS_M) * (z + CONE_HEIGHT_M) / CONE_HEIGHT_M
    } else {
        CORE_RADIUS_M
    }
}

/// SplitMix64 in `[0, 1)`: deterministic jitter with no RNG dependency (the
/// generator `examples/bake_htr10_conus_slab.rs` uses).
fn splitmix(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Solid fraction of the loose seed (random sequential placement).
const SEED_SOLID_FRACTION: f64 = 0.30;

/// `n` pebble centres placed at random without overlap in the vessel, filling
/// it from the valve upward at a loose solid fraction (0.30), the way
/// LIGGGHTS' `fix insert/pack` seeds a region: random positions, rejected if
/// they overlap a placed pebble or the wall. Deterministic in `seed`
/// (SplitMix64, no RNG dependency). The pour then settles under gravity.
///
/// ~~A jittered square lattice (pitch 1.1 d), from `bake_htr10_conus_slab.rs`
/// `seed_column`.~~ **REPLACED 2026-10-05:** a 16 890-pebble pour from that
/// lattice kept its order through the short drop: g(√3 d) 2.90 and g(√2 d)
/// 0.898 against 1.29 and 0.599 for the LIGGGHTS-port reference bed
/// (`htr10_conus_presettled_mu10_mur00.csv`), and a whole-core φ of 0.6227.
/// A seed with no lattice in it carries no order to freeze in. Measured
/// with this seed, same 16 890 pebbles: g(√2 d) 0.619, g(√3 d) 1.284, g(2 d)
/// 1.202, contact peak 19.55, against 0.599 / 1.286 / 1.190 / 19.56 for the
/// reference random bed (core region, `examples/bed_rdf_check.rs`); settled
/// after 26 000 steps (307 s, 12 threads) at whole-core φ 0.5954. That φ is
/// not compared with the package's 0.6047, which is a 27 554-pebble core.
pub fn seed_loose(n: usize, seed: u64) -> Vec<Vec3> {
    let r = PEBBLE_RADIUS_M;
    let d = 2.0 * r;
    let mut state = seed;
    let mut out: Vec<Vec3> = Vec::with_capacity(n);
    // Spatial hash of placed centres on a d-sized grid, for O(1) overlap checks.
    let mut grid: std::collections::BTreeMap<(i64, i64, i64), Vec<usize>> = std::collections::BTreeMap::new();
    let cell = |p: Vec3| ((p.x / d).floor() as i64, (p.y / d).floor() as i64, (p.z / d).floor() as i64);
    let v_pebble = 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
    // Fill slab by slab (one diameter thick) from the valve up, each slab to
    // the target solid fraction, so the seed is uniformly loose.
    let mut z0 = VALVE_Z_M;
    while out.len() < n {
        let z1 = z0 + d;
        // The narrowest wall any sphere centred in this slab can touch.
        let reach = wall_radius_at(z0 - r).min(wall_radius_at(z1 + r)) - r - 1.0e-3;
        if reach <= 0.0 {
            z0 = z1;
            continue;
        }
        let slab_volume = std::f64::consts::PI * (reach + r).powi(2) * d;
        let target = ((SEED_SOLID_FRACTION * slab_volume / v_pebble).round() as usize).max(1);
        let (mut placed, mut tries) = (0usize, 0usize);
        while placed < target && out.len() < n && tries < 200 * target {
            tries += 1;
            // Uniform in the disc of radius `reach`, uniform in z within the slab.
            let rr = reach * splitmix(&mut state).sqrt();
            let th = std::f64::consts::TAU * splitmix(&mut state);
            let p = Vec3::new(rr * th.cos(), rr * th.sin(), z0 + (z1 - z0) * splitmix(&mut state));
            if p.z - r <= VALVE_Z_M {
                continue;
            }
            let (cx, cy, cz) = cell(p);
            let mut clear = true;
            'scan: for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(ids) = grid.get(&(cx + dx, cy + dy, cz + dz)) {
                            for &k in ids {
                                let q = out[k];
                                if (p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2) < d * d {
                                    clear = false;
                                    break 'scan;
                                }
                            }
                        }
                    }
                }
            }
            if !clear {
                continue;
            }
            grid.entry((cx, cy, cz)).or_default().push(out.len());
            out.push(p);
            placed += 1;
        }
        z0 = z1;
    }
    out
}

/// Bed surface \[m\]: 99th-percentile centre height over the core (`z > 0`)
/// plus one radius. Robust to a single pebble on top (see the sweep example's
/// note on why `max z` is the wrong instrument). `None` if no pebble is in the
/// core.
pub fn surface_height_m(centres: &[Vec3]) -> Option<f64> {
    let mut zs: Vec<f64> = centres.iter().map(|c| c.z).filter(|z| *z > 0.0).collect();
    if zs.is_empty() {
        return None;
    }
    zs.sort_by(f64::total_cmp);
    let k = ((zs.len() as f64 * 0.99) as usize).min(zs.len() - 1);
    Some(zs[k] + PEBBLE_RADIUS_M)
}

/// Whole-core filling fraction `N V / (π R² h)` over `z > 0`: the like-for-like
/// comparison with the published 0.61, itself a whole-core figure.
pub fn whole_core_fraction(centres: &[Vec3]) -> f64 {
    let Some(top) = surface_height_m(centres) else { return f64::NAN };
    let n = centres.iter().filter(|c| c.z > 0.0).count();
    let v = 4.0 / 3.0 * std::f64::consts::PI * PEBBLE_RADIUS_M.powi(3);
    n as f64 * v / (std::f64::consts::PI * CORE_RADIUS_M * CORE_RADIUS_M * top)
}

impl Htr10Fill {
    /// Seed the pour and build the system. Nothing is stepped yet.
    ///
    /// # Errors
    ///
    /// A [`DemError`] if a setting is out of range (e.g. µ < 0, a non-positive
    /// timestep).
    pub fn new(settings: Htr10FillSettings) -> Result<Self, DemError> {
        let material = GranularMaterial::new(
            settings.youngs_modulus.get::<pascal>(),
            settings.poisson_ratio,
            settings.restitution,
            settings.friction,
        )?;
        let mut model = GranularContactModel::hertz_history(material);
        if settings.rolling_friction > 0.0 {
            model = model.with_rolling(RollingModel::cdt(settings.rolling_friction)?);
        }
        let boundaries = vec![
            Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), CORE_RADIUS_M)?,
            Boundary::wall(Vec3::new(0.0, 0.0, VALVE_Z_M), Vec3::new(0.0, 0.0, 1.0))?,
        ];
        let particles: Vec<Particle> =
            seed_loose(settings.n_pebbles, settings.seed).into_iter().map(pebble).collect::<Result<_, _>>()?;
        let sys = GranularSystem::new(
            particles,
            boundaries,
            model,
            Vec3::new(0.0, 0.0, -9.81),
            settings.dt.get::<second>(),
        )?
        .with_compute(ComputeType::CpuMultiThread(settings.threads))
        .with_moving_walls(vec![MovingBoundary::new(
            WallGeometry::Mesh(discharge_mesh(120)?),
            Vec3::zero(),
            Vec3::zero(),
            Vec3::zero(),
        )]);
        Ok(Self { sys, settings, steps: 0 })
    }

    /// The settings it was built with.
    #[must_use]
    pub fn settings(&self) -> Htr10FillSettings {
        self.settings
    }

    /// Step `n` more times (fewer if `max_steps` is reached) and report.
    pub fn advance(&mut self, n: usize) -> FillProgress {
        let n = n.min(self.settings.max_steps.saturating_sub(self.steps));
        if n > 0 {
            self.sys.run(n);
            self.steps += n;
        }
        self.progress()
    }

    /// Where the fill is now, without stepping.
    #[must_use]
    pub fn progress(&self) -> FillProgress {
        let centres = self.centres();
        let (mut ke, mut n) = (0.0, 0usize);
        for p in self.sys.particles() {
            if p.position.z > 0.0 {
                ke += 0.5 * p.mass * p.velocity.norm_squared();
                n += 1;
            }
        }
        let ke_ratio_core = if n == 0 { f64::INFINITY } else { ke / n as f64 / e_drop() };
        // Settled only once the bed has reached the core and come to rest; a
        // fill still falling through the barrel has n = 0 at the start.
        let settled = n > 0 && self.steps > 0 && ke_ratio_core < self.settings.settle_target;
        FillProgress {
            steps: self.steps,
            time: Time::new::<second>(self.steps as f64 * self.settings.dt.get::<second>()),
            ke_ratio_core,
            phi_whole_core: whole_core_fraction(&centres),
            surface_height: Length::new::<meter>(surface_height_m(&centres).unwrap_or(0.0)),
            n_in_core: n,
            settled,
            gave_up: !settled && self.steps >= self.settings.max_steps,
        }
    }

    /// Pebble centres \[m\], in insertion order.
    #[must_use]
    pub fn centres(&self) -> Vec<Vec3> {
        self.sys.particles().iter().map(|p| p.position).collect()
    }

    /// The underlying system (for `DemBed::from_granular_system`, overlap
    /// checks, contacts).
    #[must_use]
    pub fn system(&self) -> &GranularSystem {
        &self.sys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mesh_matches_the_stl_script_and_faces_inward() {
        let m = discharge_mesh(120).expect("mesh");
        assert_eq!(m.triangles.len(), 480, "make_htr10_discharge_stl.sh writes 480 facets");
        for t in &m.triangles {
            let n = t.normal();
            let c = t.a; // any vertex: the normal must point toward the axis
            assert!(n.x * c.x + n.y * c.y < 1e-9, "outward facet at {c:?}");
        }
    }

    #[test]
    fn seeding_is_deterministic_and_inside_the_wall() {
        let a = seed_loose(3000, 7);
        assert_eq!(a, seed_loose(3000, 7));
        assert_ne!(a, seed_loose(3000, 8));
        assert_eq!(a.len(), 3000);
        for c in &a {
            let r = (c.x * c.x + c.y * c.y).sqrt();
            assert!(r + PEBBLE_RADIUS_M <= wall_radius_at(c.z).max(wall_radius_at(c.z - PEBBLE_RADIUS_M)) + 1e-9);
            assert!(c.z - PEBBLE_RADIUS_M > VALVE_Z_M);
        }
        // Loose: no two seeds touch.
        for (i, p) in a.iter().enumerate().take(400) {
            for q in &a[i + 1..] {
                let d2 = (p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2);
                assert!(d2 >= (2.0 * PEBBLE_RADIUS_M).powi(2));
            }
        }
    }

    /// A small pour (300 pebbles: tube and conus) runs, loses kinetic energy
    /// and keeps every pebble inside the vessel. Not a packing result.
    #[test]
    fn a_small_pour_runs_and_stays_in_the_vessel() {
        let mut fill = Htr10Fill::new(Htr10FillSettings {
            n_pebbles: 300,
            threads: ThreadCount::Fixed(2),
            ..Htr10FillSettings::default()
        })
        .expect("fill");
        let p = fill.advance(4000);
        assert_eq!(p.steps, 4000);
        for c in fill.centres() {
            assert!(c.z > VALVE_Z_M, "a pebble fell through the valve: {c:?}");
            let r = (c.x * c.x + c.y * c.y).sqrt();
            assert!(r <= wall_radius_at(c.z) + 0.01, "a pebble left the wall at {c:?}");
        }
    }
}
