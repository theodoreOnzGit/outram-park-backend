//! The physics both panes run: the `triso` rung's 2D TRISO cell, transported
//! by surface tracking and by delta (Woodcock) tracking, both in
//! outram-mc-libs, unmodified (gh:#784).
//!
//! Nothing here transports a neutron. Every flight, every surface, every
//! accept/reject is the library's:
//!
//! - **surface tracking**: [`trace_csg_history`] for one neutron step by step,
//!   [`CsgPowerIteration::step_traced`] for "Run many"; the CSG geometry is the
//!   `triso` rung's own ([`model::build_geometry`]);
//! - **delta tracking**: [`trace_delta_history`] and
//!   [`DeltaPowerIteration::step_traced`], on a reflective cube
//!   ([`DeltaDomain::Cube`]) whose only geometry question is
//!   [`Geometry::locate`] on the SAME CSG model, at each tentative site;
//! - **the majorant**: [`Majorant::bounding`] over every material (the
//!   construction of gh:#585, tabulated on every nuclide breakpoint), and
//!   [`Majorant::scaled`] for the "majorant too low" ablation.
//!
//! The steps are reported by the library's observer hook
//! ([`TraceEvent`], `outram_mc_libs::physics::tracking_trace`), which draws
//! no random number: what the panes show is the history the library ran.
//!
//! # What is the same in the two panes, and what is not
//!
//! The same birth point, direction and energy, and the same RNG seed. Both
//! trackers draw their first variate from that seed for the first flight
//! (surface tracking scales it by `Σ_t` of the kernel, delta tracking by
//! `Σ_maj`), so the first flight distances differ by exactly the ratio
//! `Σ_maj/Σ_t`. After that they consume the stream differently (delta
//! tracking spends a variate on every accept/reject), so the two neutrons
//! take different random walks. They are two samples of the same physics,
//! not one path drawn twice.
//!
//! # The geometry is the same problem, bounded two ways
//!
//! The CSG cell is reflective in x and y and in z at ±50 cm; the delta
//! domain is a reflective cube of the cell's half-pitch. The model is
//! invariant in z, so both are the same infinite lattice of the same
//! infinitely long rods, and the eigenvalue is the same `k∞`. Helium is void
//! in the CSG model; for delta tracking it is an empty material
//! ([`VOID`], `Σ_t = 0`), so a tentative site in helium is always virtual,
//! which is exactly what void means.

// The browser build does not call the headless measurement helpers.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use crate::model::{self, NuclearData, N_U235};
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::keff_delta::{trace_delta_history, DeltaDomain, DeltaPowerIteration};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::tracking_trace::{TraceCounts, TraceEvent};
use outram_mc_libs::physics::transport_csg::{trace_csg_history, CsgPowerIteration, SourceBox};
use outram_mc_libs::rng::distributions::isotropic_direction;

/// Index of the helium "material" in [`Physics::materials`]: no components,
/// so `Σ_t = 0` and every delta-tracking site there is virtual.
pub const VOID: usize = 7;

/// The majorant's energy span and construction (`Majorant::bounding`), the
/// arguments `examples/common/htr10_fuel_zone.rs` uses for rung 5's record.
pub const MAJ_E_MIN: f64 = 1.0e-5;
pub const MAJ_E_MAX: f64 = 2.0e7;
pub const MAJ_BINS: usize = 4096;
pub const MAJ_SUBSAMPLES: usize = 32;
pub const MAJ_MARGIN: f64 = 0.1;

/// Events kept per pane for the step-by-step view. A thermal neutron in
/// graphite takes a few hundred collisions; the counts are always complete,
/// only the drawable list stops here (and says so).
pub const MAX_EVENTS: usize = 60_000;

/// Incident energy for the birth spectrum: thermal fission, eV.
const BIRTH_INCIDENT_EV: f64 = 0.0253;

/// The processed model, ready to transport.
pub struct Physics {
    pub geometry: Geometry,
    /// The `triso` rung's seven materials, plus [`VOID`].
    pub materials: Vec<Material>,
    pub nuclides: Vec<Nuclide>,
    pub centres: Vec<(f64, f64)>,
    /// `Σ_maj(E)`, bounding every material.
    pub majorant: Majorant,
    /// Seconds [`Majorant::bounding`] took here.
    pub majorant_secs: f64,
}

impl Physics {
    pub fn new(data: NuclearData) -> Self {
        let centres = model::particle_centres(model::LAYOUT_SEED);
        let geometry = model::build_geometry(&centres);
        let mut materials = data.materials;
        materials.push(Material {
            id: VOID as i32 + 1,
            name: "Helium coolant (void)".into(),
            components: Vec::new(),
            temperature: model::TEMPERATURE_K,
        });
        let t = dhoby_ghaut::web_demo::platform::now_s();
        let majorant = Majorant::bounding(
            &materials,
            &data.nuclides,
            MAJ_E_MIN,
            MAJ_E_MAX,
            MAJ_BINS,
            MAJ_SUBSAMPLES,
            MAJ_MARGIN,
        );
        let majorant_secs = dhoby_ghaut::web_demo::platform::now_s() - t;
        Self {
            geometry,
            materials,
            nuclides: data.nuclides,
            centres,
            majorant,
            majorant_secs,
        }
    }

    /// The delta-tracking domain: the cell, as a reflective cube.
    pub fn domain() -> DeltaDomain {
        DeltaDomain::Cube {
            half: model::half_pitch(),
        }
    }

    /// Delta tracking's one geometry question: what is at `p`? A
    /// [`Geometry::locate`] on the same CSG model; helium (a void cell) is
    /// [`VOID`].
    pub fn material_at(&self, p: Position) -> Option<usize> {
        self.geometry
            .locate(p, Direction::new(1.0, 0.0, 0.0), SurfaceToken::NONE)
            .map(|path| path.material.unwrap_or(VOID))
    }
}

/// Where a neutron starts: a point in a random kernel, an isotropic
/// direction, an energy from U-235's ENDF/B-VIII.0 fission spectrum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Birth {
    pub r: Position,
    pub u: Direction,
    pub e: f64,
    /// The seed both trackers start their RNG stream from.
    pub seed: u64,
}

/// The `n`-th neutron of the sequence a `seed` starts (deterministic).
pub fn birth(phys: &Physics, seed: u64) -> Birth {
    let mut s = seed;
    let c = &phys.centres;
    let i = ((model::uniform(&mut s) * c.len() as f64) as usize).min(c.len() - 1);
    let (cx, cy) = c[i];
    let rr = model::KERNEL_R * (1.0 - 1.0e-9) * model::uniform(&mut s).sqrt();
    let t = std::f64::consts::TAU * model::uniform(&mut s);
    let r = Position::new(cx + rr * t.cos(), cy + rr * t.sin(), 0.0);
    let mut dir_seed = model::splitmix(&mut s);
    let (u, v, w) = isotropic_direction(&mut dir_seed);
    let mut e_seed = model::splitmix(&mut s);
    let e = phys.nuclides[N_U235].sample_fission_energy(BIRTH_INCIDENT_EV, &mut e_seed);
    Birth {
        r,
        u: Direction::new(u, v, w),
        e,
        seed: model::splitmix(&mut s),
    }
}

/// One neutron's steps in one pane.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    /// The steps, in order (at most [`MAX_EVENTS`]).
    pub events: Vec<TraceEvent>,
    /// Every step counted, including any not kept.
    pub counts: TraceCounts,
    /// Steps not kept.
    pub dropped: usize,
}

impl Trace {
    fn observe(&mut self, ev: TraceEvent) {
        self.counts.add(&ev);
        if self.events.len() < MAX_EVENTS {
            self.events.push(ev);
        } else {
            self.dropped += 1;
        }
    }
}

/// The neutron, surface-tracked ([`trace_csg_history`]).
pub fn trace_surface(phys: &Physics, b: Birth) -> Trace {
    let mut t = Trace::default();
    let mut seed = b.seed;
    trace_csg_history(
        b.r,
        b.u,
        b.e,
        &phys.geometry,
        &phys.materials,
        &phys.nuclides,
        &mut seed,
        |ev| t.observe(ev),
    );
    t
}

/// The same neutron, delta-tracked ([`trace_delta_history`]) on `majorant`.
pub fn trace_delta(phys: &Physics, b: Birth, majorant: &Majorant) -> Trace {
    let mut t = Trace::default();
    let mut seed = b.seed;
    let lookup = |p: Position| phys.material_at(p);
    trace_delta_history(
        b.r,
        b.u,
        b.e,
        Physics::domain(),
        &phys.materials,
        &phys.nuclides,
        majorant,
        &lookup,
        &mut seed,
        |ev| t.observe(ev),
    );
    t
}

// ─── Run many: k∞ by both methods ────────────────────────────────────────────

/// Which tracker a "Run many" generation used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Surface,
    Delta,
    /// Delta tracking on the majorant times [`RunConfig::low_factor`].
    DeltaLow,
}

// `code` / `from_code` are the wire format (browser build and tests).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Method {
    pub const ALL: [Method; 3] = [Method::Surface, Method::Delta, Method::DeltaLow];
    pub fn code(self) -> f64 {
        match self {
            Method::Surface => 0.0,
            Method::Delta => 1.0,
            Method::DeltaLow => 2.0,
        }
    }
    pub fn from_code(c: f64) -> Result<Self, String> {
        match c as i64 {
            0 => Ok(Method::Surface),
            1 => Ok(Method::Delta),
            2 => Ok(Method::DeltaLow),
            other => Err(format!("unknown method {other}")),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Method::Surface => "surface tracking",
            Method::Delta => "delta tracking",
            Method::DeltaLow => "delta, majorant too low",
        }
    }
}

/// A "Run many" request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunConfig {
    pub n_particles: usize,
    pub n_inactive: usize,
    pub n_active: usize,
    pub seed: u64,
    /// The "majorant too low" factor; `None` runs no third arm.
    pub low_factor: Option<f64>,
}

/// One finished generation of one method.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenResult {
    pub method: Method,
    pub index: usize,
    pub active: bool,
    pub k: f64,
    /// Mean and standard error over the active generations so far.
    pub k_mean: Option<(f64, f64)>,
    pub counts: TraceCounts,
    /// Wall-clock seconds the generation took (transport only).
    pub secs: f64,
    pub n_particles: usize,
    /// Every generation of this method is done.
    pub last: bool,
}

/// The three power iterations of a "Run many", each stepped one generation
/// per request.
pub struct Run {
    pub cfg: RunConfig,
    surface: CsgPowerIteration,
    delta: DeltaPowerIteration,
    low: Option<(DeltaPowerIteration, Majorant)>,
}

impl Run {
    pub fn new(phys: &Physics, cfg: RunConfig) -> Self {
        let s = KeffSettings {
            n_particles: cfg.n_particles,
            n_inactive: cfg.n_inactive,
            n_active: cfg.n_active,
            temperature_k: model::TEMPERATURE_K,
            seed: cfg.seed,
            ..KeffSettings::default()
        };
        let p = model::half_pitch();
        let source = SourceBox {
            lower: Position::new(-p, -p, -p),
            upper: Position::new(p, p, p),
        };
        let surface =
            CsgPowerIteration::new(&phys.geometry, &phys.materials, &phys.nuclides, source, &s);
        let lookup = |q: Position| phys.material_at(q);
        let delta = DeltaPowerIteration::new(
            Physics::domain(),
            &phys.materials,
            &phys.nuclides,
            &lookup,
            &s,
        );
        let low = cfg
            .low_factor
            .map(|f| (delta.clone(), phys.majorant.scaled(f)));
        Self {
            cfg,
            surface,
            delta,
            low,
        }
    }

    /// Run `method`'s next generation; `None` once it is done (or it is the
    /// third arm and there is none).
    pub fn step(&mut self, phys: &Physics, method: Method) -> Option<GenResult> {
        let mut counts = TraceCounts::default();
        let lookup = |q: Position| phys.material_at(q);
        let t = dhoby_ghaut::web_demo::platform::now_s();
        let (index, active, k, k_mean, last) = match method {
            Method::Surface => {
                let g = self.surface.step_traced(
                    &phys.geometry,
                    &phys.materials,
                    &phys.nuclides,
                    |e| counts.add(&e),
                )?;
                (g.index, g.active, g.k, g.k_mean, self.surface.finished())
            }
            Method::Delta => {
                let g = self.delta.step_traced(
                    &phys.materials,
                    &phys.nuclides,
                    &phys.majorant,
                    &lookup,
                    |e| counts.add(&e),
                )?;
                (g.index, g.active, g.k, g.k_mean, self.delta.finished())
            }
            Method::DeltaLow => {
                let (it, maj) = self.low.as_mut()?;
                let g = it.step_traced(&phys.materials, &phys.nuclides, maj, &lookup, |e| {
                    counts.add(&e)
                })?;
                (g.index, g.active, g.k, g.k_mean, it.finished())
            }
        };
        let secs = dhoby_ghaut::web_demo::platform::now_s() - t;
        Some(GenResult {
            method,
            index,
            active,
            k,
            k_mean,
            counts,
            secs,
            n_particles: self.cfg.n_particles,
            last,
        })
    }

    /// The methods this run has.
    pub fn methods(&self) -> &'static [Method] {
        if self.low.is_some() {
            &Method::ALL
        } else {
            &Method::ALL[..2]
        }
    }
}

/// `(k_a − k_b) / σ_combined`.
pub fn z_score(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0) / (a.1 * a.1 + b.1 * b.1).sqrt()
}
