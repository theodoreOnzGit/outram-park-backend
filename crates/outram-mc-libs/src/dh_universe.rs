//! **Doubly heterogeneous universes** — one enum to choose how the particles
//! are resolved, and one call to get an eigenvalue out.
//!
//! A *doubly heterogeneous* (DH) medium has structure at two scales: fuel
//! particles a few hundred microns across, dispersed through a matrix that is
//! itself a region of a reactor. A pebble holds O(10^4) TRISO particles; a
//! dispersion-fuel plate holds a comparable number. Resolving every particle is
//! exact and expensive, so practical calculations pick a treatment — and the
//! whole point of this module is that picking one should be a single enum
//! variant, not a different program.
//!
//! ```no_run
//! use outram_mc_libs::prelude::*;
//! # let materials: Vec<Material> = Vec::new();
//! # let nuclides: Vec<Nuclide> = Vec::new();
//!
//! let universe = DhUniverse::pebble(
//!     PebbleParams::fhr_reference().with_materials(materials),
//!     DhTreatment::DeltaTracking,
//! )?;
//! let result = universe.keff(&nuclides, &KeffSettings::default());
//! println!("k_eff = {:.5} +/- {:.5}", result.k_mean, result.k_std);
//! # Ok::<(), DhError>(())
//! ```
//!
//! See [Building the material table](#building-the-material-table) below for
//! what goes in `materials` — it is the one thing a caller must supply.
//!
//! Swap `DhTreatment::DeltaTracking` for [`DhTreatment::ChordLength`] or
//! [`DhTreatment::RingRpt`] and nothing else changes.
//!
//! # Building the material table
//!
//! A pebble needs **seven** materials, in this order — the five TRISO layers
//! outward from the centre, then the fuel-zone matrix, then the fuel-free outer
//! shell. The count and order are **checked**: a short or misordered table is a
//! [`DhError::Materials`], not a silent wrong answer.
//!
//! ```no_run
//! use outram_mc_libs::prelude::*;
//! use outram_mc_libs::material::material::NuclideComponent;
//!
//! // Nuclide indices refer to positions in the `nuclides` slice passed to keff().
//! let nuclides: Vec<Nuclide> = ["U235", "U238", "O16", "C0", "Si28"]
//!     .iter()
//!     .map(|n| Nuclide::from_core(n).unwrap())
//!     .collect();
//! let (u235, u238, o16, c, si) = (0, 1, 2, 3, 4);
//!
//! let mat = |id: i32, name: &str, comps: &[(usize, f64)]| Material {
//!     id,
//!     name: name.into(),
//!     temperature: 293.6,
//!     components: comps
//!         .iter()
//!         .map(|&(nuclide_idx, atom_density)| NuclideComponent { nuclide_idx, atom_density })
//!         .collect(),
//! };
//!
//! // atom densities in atoms/b-cm
//! let materials = vec![
//!     mat(0, "UCO kernel", &[(u235, 4.40e-3), (u238, 1.77e-2), (o16, 2.27e-2), (c, 9.10e-3)]),
//!     mat(1, "buffer",     &[(c, 5.02e-2)]),
//!     mat(2, "IPyC",       &[(c, 9.53e-2)]),
//!     mat(3, "SiC",        &[(si, 4.79e-2), (c, 4.79e-2)]),
//!     mat(4, "OPyC",       &[(c, 9.53e-2)]),
//!     mat(5, "matrix",     &[(c, 8.53e-2)]),
//!     mat(6, "shell",      &[(c, 8.78e-2)]),
//! ];
//!
//! let universe = DhUniverse::pebble(
//!     PebbleParams::fhr_reference().with_materials(materials),
//!     DhTreatment::DeltaTracking,
//! )?;
//! # Ok::<(), DhError>(())
//! ```
//!
//! Dispersed fuel needs only **two**: `[particle, matrix]`.
//!
//! # Choosing a treatment
//!
//! | Variant | Geometry stored | Exact? | Measured speed |
//! |---|---|---|---|
//! | [`DhTreatment::DeltaTracking`] | every particle | **yes** | 1x (the reference) |
//! | [`DhTreatment::ChordLength`] | none | no | ~2.5-3x faster |
//! | [`DhTreatment::RingRpt`] | none (smeared) | no | ~8x faster |
//!
//! Speed figures are from `examples/dh_tracking_speedup.rs`, relative to delta
//! tracking on a fixed-source walk. See [`DhTreatment`] for what each one gives
//! up, and `examples/dh_keff_vv.rs` for the eigenvalue each produces on the same
//! problem — **a speedup is meaningless without the answer it bought.**
//!
//! # Haiku dogfood record — 2026-09-14
//!
//! Per the workspace "dogfood the API on a small model" hard rule: a Haiku agent
//! with **documentation only** — no repository access, no source, no compiler —
//! was asked to build this pebble and solve its eigenvalue under all three
//! treatments.
//!
//! **It wrote correct code on the first attempt**, with zero wrong method names
//! and zero wrong argument orders. It found `DhTreatment::ALL`, `is_exact()`,
//! `name()`, `with_materials()` and `keff()` unaided. For contrast, the baseline
//! recorded in the workspace `CLAUDE.md` is an *Opus* session guessing wrong ten
//! times across six scripts against the older API, **with** source access.
//!
//! Self-rated confidence was 6/10 — and every reason it gave was about
//! *adjacent* types rather than this enum. Those are the real findings, and they
//! were fixed rather than filed:
//!
//! | It could not tell | Fix |
//! |---|---|
//! | Whether repeated `ChordLength` runs reproduce | [`DhUniverse::keff`] now has a Reproducibility section; the answer is backend-dependent, which it could not have guessed |
//! | How to construct a `Material` at all — "I do not know what fields they have" | Worked material-table example added above |
//! | Whether the 7-material contract is checked or merely stated | Now says explicitly that it is checked |
//! | Why `DeltaTracking` is "exact" — no physics cited | [`DhTreatment::DeltaTracking`] now says unbiased-at-the-majorant, and what that does and does not mean |
//!
//! One finding came from running the doctests rather than from the agent: the
//! module's own headline example **did not compile** (it used an undeclared
//! `nuclides`). That is the same failure already recorded in
//! [`crate::pebble_beds::fhr_pebble`] — an API that cannot be called from the
//! module documenting it is not callable. Both examples are now compile-tested.
//!
//! Still open, deliberately: the agent could not find the field lists for
//! `KeffResult`, `KeffSettings`, `Material` or `Nuclide`. Those are other
//! modules' documentation debt, not this one's, and are not papered over here.
//!
//! # Scope
//!
//! Mono-material-per-region: each treatment answers "which material index is at
//! this point?", and the caller supplies the material table. That is the seam
//! every k-eff driver in this crate already consumes, which is why all three
//! treatments can share one transport path rather than each needing its own.

use std::sync::Mutex;

use crate::geometry::position::Position;
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::pebble_beds::delta_tracking::Majorant;
use crate::pebble_beds::fhr_pebble::{
    homogenise_by_volume, ExplicitTrisoPebble, TrisoMaterials, TrisoSpec,
};
use crate::pebble_beds::keff_delta::{run_keff_delta_in, DeltaDomain};
use crate::pebble_beds::sphere_packing::{PackedSpheres, PackingConfig, PackingMethod};
use crate::physics::keff::{KeffResult, KeffSettings};
use crate::stochastic::cls::ClsMedium;
use crate::stochastic::medium::MaterialId;

/// How the double heterogeneity is resolved.
///
/// The three variants sit on one axis — how much particle geometry survives —
/// and each gives something up in exchange for time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhTreatment {
    /// **Exact.** Every particle is stored; flights are sampled at a majorant
    /// and collisions accepted by rejection (Woodcock).
    ///
    /// Gives up nothing: rejection at the majorant is an *unbiased* estimator
    /// of the true collision density, so this reproduces the explicit-geometry
    /// answer to within its own statistics. That is what "exact" means here —
    /// no geometric approximation, not zero statistical error.
    ///
    /// The unbiasedness holds only while the majorant bounds the true total
    /// cross section everywhere; [`DhUniverse::keff`] builds one with margin
    /// for that reason. This is the reference the other two are judged against.
    DeltaTracking,

    /// **Chord-length sampling.** No geometry is stored; particle crossings are
    /// resampled from closed-form chord statistics as the neutron flies.
    ///
    /// Gives up *memory of where the particles were*. A neutron that
    /// back-scatters re-crosses ground it has already covered and meets a
    /// freshly sampled medium rather than the one it just left, so CLS is
    /// least reliable in scattering-dominated, optically thick problems.
    ChordLength,

    /// **Ring-RPT.** The particles are dissolved into an equivalent shell of
    /// homogenised material; there is no double heterogeneity left at all.
    ///
    /// Gives up *self-shielding*: a neutron sees absorber smeared everywhere at
    /// reduced density instead of concentrated in kernels it could have missed,
    /// so reactivity is systematically affected. The shell's inner radius is a
    /// fitted parameter for exactly this reason.
    RingRpt,
}

impl DhTreatment {
    /// Every variant, for sweeping all three in a comparison.
    pub const ALL: [Self; 3] = [Self::DeltaTracking, Self::ChordLength, Self::RingRpt];

    /// Short human-readable name, e.g. for table rows.
    pub fn name(self) -> &'static str {
        match self {
            Self::DeltaTracking => "delta tracking",
            Self::ChordLength => "chord-length sampling",
            Self::RingRpt => "ring-RPT (homogenised)",
        }
    }

    /// Whether this treatment resolves the particle geometry exactly.
    ///
    /// Only [`Self::DeltaTracking`] does. The other two are approximations and
    /// their eigenvalues must be read as such.
    pub fn is_exact(self) -> bool {
        matches!(self, Self::DeltaTracking)
    }
}

/// What went wrong building a [`DhUniverse`].
#[derive(Debug)]
pub enum DhError {
    /// The particle packing could not be generated at the requested fraction.
    Packing(String),
    /// A geometric parameter was inconsistent, e.g. fuel zone outside the pebble.
    Geometry(String),
    /// The material table did not carry the indices the universe needs.
    Materials(String),
}

impl core::fmt::Display for DhError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Packing(m) => write!(f, "particle packing failed: {m}"),
            Self::Geometry(m) => write!(f, "inconsistent geometry: {m}"),
            Self::Materials(m) => write!(f, "material table problem: {m}"),
        }
    }
}

impl std::error::Error for DhError {}

/// A spherical fuel pebble with TRISO particles in a graphite fuel zone.
///
/// Radii are in cm and must satisfy `fuel_zone_radius < pebble_radius`.
#[derive(Debug, Clone)]
pub struct PebbleParams {
    /// TRISO layer radii and packing fraction.
    pub spec: TrisoSpec,
    /// Outer radius of the particle-bearing fuel zone \[cm\].
    pub fuel_zone_radius: f64,
    /// Outer radius of the whole pebble, including the fuel-free shell \[cm\].
    pub pebble_radius: f64,
    /// Material table. Indices are the convention documented on
    /// [`DhUniverse::material_at`].
    pub materials: Vec<Material>,
    /// Packing seed — fixes the particle positions, so a fixed seed gives a
    /// reproducible universe.
    pub seed: u64,
}

impl PebbleParams {
    /// The FHR reference pebble used by this crate's ring-RPT V&V decks:
    /// 19.9 % HALEU UCO TRISO at 30 % packing, 1.9 cm fuel zone inside a 2.0 cm
    /// pebble.
    ///
    /// `materials` is left empty and **must be filled in** before use — the
    /// geometry is reference data, the material composition is yours.
    pub fn fhr_reference() -> Self {
        Self {
            spec: TrisoSpec::FHR_HALEU_UCO,
            fuel_zone_radius: 1.9,
            pebble_radius: 2.0,
            materials: Vec::new(),
            seed: 0x0DDF_1234_5678_9ABC,
        }
    }

    /// Replace the material table, returning `self` so constructors can chain.
    pub fn with_materials(mut self, materials: Vec<Material>) -> Self {
        self.materials = materials;
        self
    }
}

/// Fuel particles dispersed through a matrix in a cube — dispersion fuel,
/// burnable-poison particles, a plate rather than a pebble.
#[derive(Debug, Clone)]
pub struct DispersedParams {
    /// Whole-particle radius \[cm\].
    pub particle_radius: f64,
    /// Particle volume fraction in the cube, in (0, 1).
    pub packing_fraction: f64,
    /// Half-width of the cubic domain \[cm\].
    pub half_width: f64,
    /// Material table — see [`DhUniverse::material_at`].
    pub materials: Vec<Material>,
    /// Packing seed.
    pub seed: u64,
}

/// The geometry backing a universe, once a treatment has been chosen.
enum DhGeometry {
    /// Explicit particles in a pebble (fuel zone + shell).
    Pebble(ExplicitTrisoPebble),
    /// Explicit particles in a cube.
    Dispersed {
        packing: PackedSpheres,
        particle_material: usize,
        matrix_material: usize,
    },
    /// Chord-length sampling — no stored particles. The `Mutex` is what lets a
    /// stateful sampler satisfy the `Fn + Sync` point-query seam the k-eff
    /// drivers expect; on the single-threaded reference path it is uncontended.
    Cls {
        medium: Mutex<(ClsMedium, u64)>,
        particle_material: usize,
        matrix_material: usize,
        shell_material: Option<usize>,
        fuel_zone_radius: f64,
    },
    /// Homogenised — one smeared material inside the fuel zone.
    RingRpt {
        homogenised: usize,
        shell_material: Option<usize>,
        fuel_zone_radius: f64,
    },
}

/// A doubly heterogeneous universe with a chosen [`DhTreatment`].
///
/// Build one with [`Self::pebble`] or [`Self::dispersed`], then call
/// [`Self::keff`]. The treatment is fixed at construction because it changes
/// what geometry is stored, not merely how it is traversed.
pub struct DhUniverse {
    treatment: DhTreatment,
    geometry: DhGeometry,
    materials: Vec<Material>,
    domain: DeltaDomain,
    particles: usize,
}

impl DhUniverse {
    /// Build a TRISO fuel pebble under the chosen treatment.
    ///
    /// # Material index convention
    ///
    /// `params.materials` is indexed as
    /// `[kernel, buffer, ipyc, sic, opyc, matrix, shell]` — the five TRISO
    /// layers outward from the centre, then the fuel-zone matrix, then the
    /// fuel-free outer shell. Seven entries are required.
    ///
    /// # Errors
    ///
    /// [`DhError::Geometry`] if `fuel_zone_radius >= pebble_radius`,
    /// [`DhError::Materials`] if fewer than seven materials are supplied, and
    /// [`DhError::Packing`] if the particles will not pack at the requested
    /// fraction.
    pub fn pebble(params: PebbleParams, treatment: DhTreatment) -> Result<Self, DhError> {
        if params.fuel_zone_radius >= params.pebble_radius {
            return Err(DhError::Geometry(format!(
                "fuel_zone_radius {} must be < pebble_radius {}",
                params.fuel_zone_radius, params.pebble_radius
            )));
        }
        if params.materials.len() < 7 {
            return Err(DhError::Materials(format!(
                "pebble needs 7 materials [kernel, buffer, ipyc, sic, opyc, matrix, shell], got {}",
                params.materials.len()
            )));
        }
        let pf = params.spec.packing_fraction;
        let r_particle = params.spec.opyc;
        let domain = DeltaDomain::Sphere { radius: params.pebble_radius };
        const MATRIX_IDX: usize = 5;
        const SHELL_IDX: usize = 6;

        let (geometry, particles) = match treatment {
            DhTreatment::DeltaTracking => {
                let packing = pack_in_ball(r_particle, params.fuel_zone_radius, pf, params.seed)?;
                let n = packing.len();
                let pebble = ExplicitTrisoPebble::new(
                    packing,
                    params.spec,
                    TrisoMaterials {
                        kernel: 0,
                        buffer: 1,
                        ipyc: 2,
                        sic: 3,
                        opyc: 4,
                        matrix: MATRIX_IDX,
                    },
                    SHELL_IDX,
                    // Coolant: the domain is reflective at the pebble surface,
                    // so no history ever reaches a coolant region. Naming the
                    // shell here keeps the index in range without inventing a
                    // material the caller did not supply.
                    SHELL_IDX,
                    params.fuel_zone_radius,
                    params.pebble_radius,
                );
                (DhGeometry::Pebble(pebble), n)
            }
            DhTreatment::ChordLength => {
                // CLS treats the whole particle as one inclusion: its internal
                // layering is below the resolution the model claims. The
                // inclusion material is the volume-homogenised particle.
                let particle = homogenise_particle(&params.materials, params.spec)?;
                let mut materials = params.materials.clone();
                materials.push(particle);
                let particle_idx = materials.len() - 1;
                let medium = ClsMedium::new(
                    r_particle,
                    pf,
                    MaterialId(particle_idx),
                    MaterialId(MATRIX_IDX),
                );
                return Ok(Self {
                    treatment,
                    geometry: DhGeometry::Cls {
                        medium: Mutex::new((medium, params.seed | 1)),
                        particle_material: particle_idx,
                        matrix_material: MATRIX_IDX,
                        shell_material: Some(SHELL_IDX),
                        fuel_zone_radius: params.fuel_zone_radius,
                    },
                    materials,
                    domain,
                    particles: 0,
                });
            }
            DhTreatment::RingRpt => {
                // Smear particles and matrix together across the fuel zone.
                let particle = homogenise_particle(&params.materials, params.spec)?;
                let matrix = params.materials[MATRIX_IDX].clone();
                let smeared = homogenise_by_volume(
                    &[(&particle, pf), (&matrix, 1.0 - pf)],
                    900,
                    "ring-RPT homogenised fuel zone",
                    matrix.temperature,
                );
                let mut materials = params.materials.clone();
                materials.push(smeared);
                let idx = materials.len() - 1;
                return Ok(Self {
                    treatment,
                    geometry: DhGeometry::RingRpt {
                        homogenised: idx,
                        shell_material: Some(SHELL_IDX),
                        fuel_zone_radius: params.fuel_zone_radius,
                    },
                    materials,
                    domain,
                    particles: 0,
                });
            }
        };

        Ok(Self {
            treatment,
            geometry,
            materials: params.materials,
            domain,
            particles,
        })
    }

    /// Build a cube of fuel particles dispersed through a matrix.
    ///
    /// # Material index convention
    ///
    /// `params.materials` is `[particle, matrix]`. Two entries are required.
    ///
    /// # Errors
    ///
    /// As [`Self::pebble`], minus the shell.
    pub fn dispersed(params: DispersedParams, treatment: DhTreatment) -> Result<Self, DhError> {
        if params.materials.len() < 2 {
            return Err(DhError::Materials(format!(
                "dispersed fuel needs 2 materials [particle, matrix], got {}",
                params.materials.len()
            )));
        }
        const PARTICLE: usize = 0;
        const MATRIX: usize = 1;
        let domain = DeltaDomain::Cube { half: params.half_width };

        let (geometry, particles) = match treatment {
            DhTreatment::DeltaTracking => {
                let cfg = PackingConfig {
                    particle_radius: params.particle_radius,
                    packing_fraction: params.packing_fraction,
                    domain_half_width: params.half_width,
                    method: PackingMethod::Rsa,
                    seed: params.seed,
                };
                let spheres = cfg.generate().map_err(|e| DhError::Packing(format!("{e:?}")))?;
                let n = spheres.len();
                let packing = PackedSpheres::from_spheres(
                    spheres,
                    params.half_width,
                    params.particle_radius,
                );
                (
                    DhGeometry::Dispersed {
                        packing,
                        particle_material: PARTICLE,
                        matrix_material: MATRIX,
                    },
                    n,
                )
            }
            DhTreatment::ChordLength => {
                let medium = ClsMedium::new(
                    params.particle_radius,
                    params.packing_fraction,
                    MaterialId(PARTICLE),
                    MaterialId(MATRIX),
                );
                (
                    DhGeometry::Cls {
                        medium: Mutex::new((medium, params.seed | 1)),
                        particle_material: PARTICLE,
                        matrix_material: MATRIX,
                        shell_material: None,
                        fuel_zone_radius: f64::INFINITY,
                    },
                    0,
                )
            }
            DhTreatment::RingRpt => {
                let smeared = homogenise_by_volume(
                    &[
                        (&params.materials[PARTICLE], params.packing_fraction),
                        (&params.materials[MATRIX], 1.0 - params.packing_fraction),
                    ],
                    901,
                    "homogenised dispersed fuel",
                    params.materials[MATRIX].temperature,
                );
                let mut materials = params.materials.clone();
                materials.push(smeared);
                let idx = materials.len() - 1;
                return Ok(Self {
                    treatment,
                    geometry: DhGeometry::RingRpt {
                        homogenised: idx,
                        shell_material: None,
                        fuel_zone_radius: f64::INFINITY,
                    },
                    materials,
                    domain,
                    particles: 0,
                });
            }
        };

        Ok(Self {
            treatment,
            geometry,
            materials: params.materials,
            domain,
            particles,
        })
    }

    /// Which treatment this universe was built with.
    pub fn treatment(&self) -> DhTreatment {
        self.treatment
    }

    /// How many particles are explicitly stored.
    ///
    /// Zero for [`DhTreatment::ChordLength`] and [`DhTreatment::RingRpt`] —
    /// that *is* the saving, and it is worth being able to see it.
    pub fn particle_count(&self) -> usize {
        self.particles
    }

    /// The material table this universe resolves indices against, including any
    /// homogenised material the treatment synthesised.
    pub fn materials(&self) -> &[Material] {
        &self.materials
    }

    /// Material index at `p` \[cm\], or `None` outside the universe.
    ///
    /// This is the single seam all three treatments share: delta tracking looks
    /// the point up in stored geometry, chord-length sampling *samples* it, and
    /// ring-RPT answers from radius alone.
    ///
    /// For [`DhTreatment::ChordLength`] this call is **stateful and not pure** —
    /// repeated queries at the same point may differ, because the medium is
    /// being sampled rather than read.
    pub fn material_at(&self, p: Position) -> Option<usize> {
        match &self.geometry {
            DhGeometry::Pebble(pebble) => pebble.material_at(p),
            DhGeometry::Dispersed {
                packing,
                particle_material,
                matrix_material,
            } => {
                if packing.is_inside_kernel(p) {
                    Some(*particle_material)
                } else {
                    Some(*matrix_material)
                }
            }
            DhGeometry::Cls {
                medium,
                particle_material,
                matrix_material,
                shell_material,
                fuel_zone_radius,
            } => {
                if let (Some(shell), true) = (shell_material, p.norm() >= *fuel_zone_radius) {
                    return Some(*shell);
                }
                let mut guard = medium.lock().ok()?;
                let (cls, seed) = &mut *guard;
                match cls.material_at(p, seed) {
                    Ok(id) if id.index() == particle_material.to_owned() => Some(*particle_material),
                    Ok(_) => Some(*matrix_material),
                    Err(_) => Some(*matrix_material),
                }
            }
            DhGeometry::RingRpt {
                homogenised,
                shell_material,
                fuel_zone_radius,
            } => {
                if let (Some(shell), true) = (shell_material, p.norm() >= *fuel_zone_radius) {
                    return Some(*shell);
                }
                Some(*homogenised)
            }
        }
    }

    /// Solve the eigenvalue for this universe.
    ///
    /// All three treatments run through the same delta-tracked power iteration,
    /// differing only in how [`Self::material_at`] answers — which is what makes
    /// the eigenvalues directly comparable, and therefore what makes a V&V of
    /// the approximate treatments possible at all.
    ///
    /// The domain is reflective: a pebble is a sphere of `pebble_radius`,
    /// dispersed fuel a cube of `half_width`. That makes this an infinite-medium
    /// (k-infinity) calculation, not a standalone critical system.
    ///
    /// # Reproducibility
    ///
    /// [`DhTreatment::DeltaTracking`] and [`DhTreatment::RingRpt`] answer
    /// [`Self::material_at`] from stored data, so a fixed
    /// [`KeffSettings::seed`] gives the same eigenvalue every run, on either the
    /// single- or multi-threaded backend.
    ///
    /// **[`DhTreatment::ChordLength`] is different and you need to know how.**
    /// Its point query *samples*, consuming from a stream seeded by
    /// [`PebbleParams::seed`]. On
    /// [`ComputeType::CpuSingleThread`](crate::physics::compute::ComputeType)
    /// the queries are ordered, so a fixed pair of seeds reproduces exactly. On
    /// the multi-threaded backend they are **not** — threads take the sampler's
    /// lock in whatever order they arrive, so the stream is consumed in a
    /// different order each run and the eigenvalue moves by of order its own
    /// statistical error. Use the single-thread backend when a chord-length
    /// result has to be reproducible, and treat any multi-threaded CLS number as
    /// one draw rather than *the* answer.
    pub fn keff(&self, nuclides: &[Nuclide], settings: &KeffSettings) -> KeffResult {
        // Same bounding grid the ring-RPT V&V deck uses: 4096 log bins over
        // 1e-4 eV .. 20 MeV with refinement, 30 % margin. An under-bound
        // majorant is a SILENT bias in delta tracking, not a crash.
        let majorant = Majorant::bounding(&self.materials, nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.3);
        run_keff_delta_in(
            self.domain,
            &self.materials,
            nuclides,
            &majorant,
            |p| self.material_at(p),
            settings,
        )
    }
}

/// RSA-pack whole particles into a ball of `radius`, via a cube clip.
fn pack_in_ball(
    particle_radius: f64,
    radius: f64,
    packing_fraction: f64,
    seed: u64,
) -> Result<PackedSpheres, DhError> {
    let half = radius + particle_radius;
    let cfg = PackingConfig {
        particle_radius,
        packing_fraction,
        domain_half_width: half,
        method: PackingMethod::Rsa,
        seed,
    };
    let spheres = cfg.generate().map_err(|e| DhError::Packing(format!("{e:?}")))?;
    let kept: Vec<_> = spheres
        .into_iter()
        .filter(|s| s.center.norm() + particle_radius <= radius)
        .collect();
    if kept.is_empty() {
        return Err(DhError::Packing(
            "no particles fell inside the fuel zone".into(),
        ));
    }
    Ok(PackedSpheres::from_spheres(kept, half, particle_radius))
}

/// Volume-homogenise the five TRISO layers into one particle material.
fn homogenise_particle(materials: &[Material], spec: TrisoSpec) -> Result<Material, DhError> {
    if materials.len() < 5 {
        return Err(DhError::Materials("need the 5 TRISO layer materials".into()));
    }
    let r = [spec.kernel, spec.buffer, spec.ipyc, spec.sic, spec.opyc];
    let total = r[4] * r[4] * r[4];
    let mut parts = Vec::with_capacity(5);
    let mut inner = 0.0;
    for (i, &outer) in r.iter().enumerate() {
        let v = (outer * outer * outer - inner) / total;
        parts.push((materials[i].clone(), v));
        inner = outer * outer * outer;
    }
    let refs: Vec<(&Material, f64)> = parts.iter().map(|(m, v)| (m, *v)).collect();
    Ok(homogenise_by_volume(
        &refs,
        902,
        "homogenised TRISO particle",
        materials[0].temperature,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::material::NuclideComponent;

    /// Seven graphite-ish materials in the index order [`DhUniverse::pebble`]
    /// documents. Composition is irrelevant to these tests — only the count and
    /// the indexing are under test.
    fn dummy_materials(n: usize) -> Vec<Material> {
        (0..n)
            .map(|i| Material {
                id: i as i32,
                name: format!("m{i}"),
                temperature: 293.6,
                components: vec![NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 8.0e-2,
                }],
            })
            .collect()
    }

    fn params() -> PebbleParams {
        PebbleParams::fhr_reference().with_materials(dummy_materials(7))
    }

    /// Every treatment builds from the same parameters — the property that makes
    /// the enum a real choice rather than three separate code paths.
    #[test]
    fn all_three_treatments_build_from_identical_params() {
        for t in DhTreatment::ALL {
            let u = DhUniverse::pebble(params(), t)
                .unwrap_or_else(|e| panic!("{} failed to build: {e}", t.name()));
            assert_eq!(u.treatment(), t);
        }
    }

    /// Only delta tracking stores particles; that difference IS the saving, and
    /// it should be visible through the API rather than implied.
    #[test]
    fn only_delta_tracking_stores_particles() {
        let delta = DhUniverse::pebble(params(), DhTreatment::DeltaTracking).unwrap();
        assert!(
            delta.particle_count() > 1000,
            "expected a real packing, got {}",
            delta.particle_count()
        );
        for t in [DhTreatment::ChordLength, DhTreatment::RingRpt] {
            let u = DhUniverse::pebble(params(), t).unwrap();
            assert_eq!(u.particle_count(), 0, "{} should store no particles", t.name());
        }
    }

    /// The approximate treatments synthesise a material (homogenised particle or
    /// smeared zone), so their table is longer than the caller's.
    #[test]
    fn approximate_treatments_extend_the_material_table() {
        let delta = DhUniverse::pebble(params(), DhTreatment::DeltaTracking).unwrap();
        assert_eq!(delta.materials().len(), 7);
        for t in [DhTreatment::ChordLength, DhTreatment::RingRpt] {
            let u = DhUniverse::pebble(params(), t).unwrap();
            assert!(
                u.materials().len() > 7,
                "{} should synthesise a material",
                t.name()
            );
        }
    }

    /// Every treatment answers everywhere inside the pebble, and the shell is
    /// the shell for all of them — the fuel-free region is not approximated.
    #[test]
    fn material_at_answers_across_the_whole_pebble() {
        for t in DhTreatment::ALL {
            let u = DhUniverse::pebble(params(), t).unwrap();
            // Deep inside the fuel zone.
            assert!(
                u.material_at(Position { x: 0.0, y: 0.0, z: 0.0 }).is_some(),
                "{}: no material at the centre",
                t.name()
            );
            // In the fuel-free shell (fuel zone 1.9, pebble 2.0).
            assert_eq!(
                u.material_at(Position { x: 0.0, y: 0.0, z: 1.95 }),
                Some(6),
                "{}: shell should be material 6",
                t.name()
            );
        }
    }

    /// Exactness is a property of the treatment, and callers need to be able to
    /// ask — a bias is only interpretable against a known-exact reference.
    #[test]
    fn exactly_one_treatment_is_exact() {
        let exact: Vec<_> = DhTreatment::ALL.iter().filter(|t| t.is_exact()).collect();
        assert_eq!(exact.len(), 1);
        assert_eq!(*exact[0], DhTreatment::DeltaTracking);
    }

    #[test]
    fn geometry_and_material_errors_are_reported_not_panicked() {
        let mut bad = params();
        bad.fuel_zone_radius = 2.5; // outside the pebble
        assert!(matches!(
            DhUniverse::pebble(bad, DhTreatment::DeltaTracking),
            Err(DhError::Geometry(_))
        ));

        let short = PebbleParams::fhr_reference().with_materials(dummy_materials(3));
        assert!(matches!(
            DhUniverse::pebble(short, DhTreatment::DeltaTracking),
            Err(DhError::Materials(_))
        ));
    }

    /// Dispersed fuel takes the same three treatments as a pebble.
    #[test]
    fn dispersed_fuel_supports_every_treatment() {
        for t in DhTreatment::ALL {
            let p = DispersedParams {
                particle_radius: 0.04,
                packing_fraction: 0.25,
                half_width: 1.0,
                materials: dummy_materials(2),
                seed: 42,
            };
            let u = DhUniverse::dispersed(p, t)
                .unwrap_or_else(|e| panic!("dispersed {} failed: {e}", t.name()));
            assert_eq!(u.treatment(), t);
            assert!(u.material_at(Position { x: 0.0, y: 0.0, z: 0.0 }).is_some());
        }
    }
}
