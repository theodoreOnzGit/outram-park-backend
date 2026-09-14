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
//! Swap `DhTreatment::DeltaTracking` for any other [`DhTreatment`] variant and
//! nothing else changes.
//!
//! # Building the material table
//!
//! A pebble needs **seven** materials, in this order — the five TRISO layers
//! outward from the centre, then the fuel-zone matrix, then the fuel-free outer
//! shell — or **eight** with a coolant shell ([`PebbleParams::with_coolant`],
//! [`PebbleParams::fhr_unit_cell`]). The count and order are **checked**: a
//! short or misordered table is a [`DhError::Materials`], not a silent wrong
//! answer.
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
//! | Variant | Geometry stored | Exact? | Needs fitting | Geometry-only speed |
//! |---|---|---|---|---|
//! | [`DhTreatment::DeltaTracking`] | every particle | **yes** | no | 1x (reference) |
//! | [`DhTreatment::ChordLength`] | none | no | no | ~2.5-3x faster |
//! | [`DhTreatment::Scls`] | a retention window | no | no | ~1.5-2x faster |
//! | [`DhTreatment::Homogenised`] | none (smeared) | no | no | ~8x faster |
//! | [`DhTreatment::RingRpt`] | none (fitted annulus) | no | **yes** | ~8x faster |
//!
//! Measured eigenvalues and their biases live in `examples/dh_keff_vv.rs`,
//! which runs every arm on one pebble and prints the table. They are **not**
//! duplicated here, because a number copied into two places drifts — an earlier
//! revision of this file advertised biases from a superseded run for exactly
//! that reason.
//!
//! **Read a geometry-only speedup with suspicion, but not with the suspicion
//! an earlier revision of this file recommended.** That revision reported every
//! approximate treatment as *slower* than exact delta tracking in a real
//! eigenvalue calculation. It was wrong: [`DhUniverse::keff`] was bounding its
//! majorant over the whole material table, including the undiluted kernel no
//! approximate geometry can return, which multiplied their virtual-collision
//! counts by roughly 25x at the U-238 resonances. Bounding over reachable
//! materials only (2026-09-14) put all four arms at **1.7-2.2x faster** than
//! delta tracking. The geometry-only benchmark's 19-69x still does not
//! transfer — the eigenvalue speedups are far smaller — but the sign was this
//! crate's bug, not a property of the methods. Measure on your own case.
//!
//! # Ring-RPT needs a fitted radius, and there is an API for that
//!
//! [`DhTreatment::RingRpt`] carries an `inner_radius` that is a fitted
//! equivalence, not a dimension you can look up. Published values are fitted
//! against one fuel, one library and one code and do not transfer.
//! [`fit_ring_rpt_inner_radius`] runs the fit for your own pebble — it solves
//! the explicit pebble once for a target, then bisects the radius until
//! ring-RPT matches — so the parameter is measured rather than borrowed.
//!
//! # Haiku dogfood record — 2026-09-14
//!
//! Per the workspace "dogfood the API on a small model" hard rule: a Haiku agent
//! with **documentation only** — no repository access, no source, no compiler —
//! was asked to build this pebble and solve its eigenvalue under every
//! treatment.
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
//! **What the dogfood did NOT catch, and could not have.** The agent had only
//! the documentation, and the documentation was *wrong*: the variant then named
//! `RingRpt` did naive full-zone homogenisation while its rustdoc described the
//! fitted-annulus method. Haiku wrote code that called it correctly and got a
//! number that was not ring-RPT. A usability dogfood tests whether an API can be
//! found and called; it cannot test whether the API does what it says. That
//! needed a V&V run against a published reference, which is what eventually
//! caught it — see the correction on [`DhTreatment`].
//!
//! Still open, deliberately: the agent could not find the field lists for
//! `KeffResult`, `KeffSettings`, `Material` or `Nuclide`. Those are other
//! modules' documentation debt, not this one's, and are not papered over here.
//!
//! # Scope
//!
//! Mono-material-per-region: each treatment answers "which material index is at
//! this point?", and the caller supplies the material table. That is the seam
//! every k-eff driver in this crate already consumes, which is why every
//! treatment shares one transport path rather than each needing its own.

use std::sync::Mutex;

use crate::geometry::position::Position;
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::pebble_beds::delta_tracking::Majorant;
use crate::pebble_beds::fhr_pebble::{
    homogenise_by_volume, ExplicitTrisoPebble, TrisoMaterials, TrisoSpec,
};
use crate::pebble_beds::keff_delta::{run_keff_delta_in, DeltaDomain, MaterialQuery};
use crate::pebble_beds::sphere_packing::{PackedSpheres, PackingConfig, PackingMethod};
use crate::physics::keff::{KeffResult, KeffSettings};
use crate::stochastic::cls::ClsMedium;
use crate::stochastic::scls::SclsMedium;
use crate::stochastic::medium::MaterialId;

/// How the double heterogeneity is resolved.
///
/// The four variants sit on one axis — how much particle geometry survives —
/// and each gives something up in exchange for time.
///
/// # A correction, 2026-09-14
///
/// Until this revision there were three variants and the one named `RingRpt`
/// did **naive full-fuel-zone homogenisation**: one smeared material filling
/// `r < fuel_zone_radius`, with no inner ball and no fitted shell. Its own
/// rustdoc described the fitted-shell method, so the name and the prose both
/// promised something the code did not do, and a k-eff V&V run against it
/// reported ring-RPT as costing **-5151 pcm** when the method it was named
/// after costs about **-30 pcm**.
///
/// The naive smear is still here and still useful — it is the "all double
/// heterogeneity removed" upper bound on speedup — but it is now called
/// [`Self::Homogenised`], which is what it is. [`Self::RingRpt`] is the real
/// method and carries the fitted radius it cannot work without.
#[derive(Debug, Clone, Copy, PartialEq)]
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

    /// **Semi-implicit chord-length sampling (SCLS).** CLS, but with bounded
    /// geometric memory: inclusions the neutron has already met are remembered
    /// inside a moving sphere of radius `transport_mfp + inclusion_radius`, so a
    /// back-scattered neutron re-meets the particle it just left instead of a
    /// freshly sampled one.
    ///
    /// This attacks exactly the weakness named on [`Self::ChordLength`], at the
    /// cost of carrying a retention window. Whether it is *closer* to exact is
    /// regime-dependent and this crate has measured it going the wrong way:
    /// `src/stochastic/benchmark.rs` finds CLS nearer an RSA reference than
    /// SCLS at pf 0.2, with SCLS over-correcting past it. Measure on your own
    /// problem; do not assume the more elaborate method wins.
    ///
    /// # Wiring, and a correction — 2026-09-14
    ///
    /// SCLS has two kinds of state and they are reset differently. The
    /// in-progress **flight** is per-history and is discarded at each history
    /// boundary through
    /// [`MaterialQuery::begin_history`](crate::pebble_beds::keff_delta::MaterialQuery::begin_history).
    /// The **retained inclusions** are not per-history at all — they are a
    /// progressively reconstructed model of the packing, and
    /// [`SclsMedium::begin_flight`](crate::stochastic::scls::SclsMedium::begin_flight)
    /// deliberately keeps them so a later history in the same neighbourhood
    /// still benefits.
    ///
    /// **Two defects here made this arm CLS in all but name, and both are
    /// fixed.** `begin_history` restored a pristine medium, wiping the retained
    /// geometry every history; and the retention window was seeded from
    /// `mean_chord_matrix()`, a *geometric* inter-inclusion distance, where the
    /// rule calls for a *transport mean free path* — 0.132 cm against ~2.6 cm
    /// on the FHR pebble, about **15x too small**. Both pushed SCLS towards CLS,
    /// and the V&V duly measured the two within statistics of each other. Any
    /// SCLS number recorded before 2026-09-14 is a measurement of that wiring,
    /// not of the method.
    ///
    /// # SCLS is the WRONG METHOD for a graphite pebble, and fixing the wiring is what showed it
    ///
    /// The retention window is `lambda_transport-mfp + R_largest`. In a
    /// graphite-moderated pebble at thermal energy that is about **2.68 cm**,
    /// against a fuel zone of radius **1.9 cm** — the window is larger than the
    /// region it is supposed to be a *local* view of.
    ///
    /// When the window exceeds the domain nothing is ever culled, so the
    /// retained set grows to every inclusion the neutron has ever met, and
    /// [`SclsMedium::material_at`](crate::stochastic::scls::SclsMedium::material_at)
    /// scans it **linearly on every query**. Bounded memory — the entire premise
    /// of the method — does not hold, and the result is slower than explicit
    /// delta tracking, which answers the same question from an O(1) grid.
    ///
    /// So SCLS pays off only where `lambda_tr` is genuinely *small* against the
    /// domain: optically thick media, strong absorbers, larger geometries. On
    /// this problem it is dominated by delta tracking on both accuracy and cost,
    /// and the honest recommendation is not to use it here. The window is capped
    /// at the domain (beyond it there is no geometry to retain) and a warning is
    /// logged, but a cap cannot rescue the premise.
    ///
    /// This was invisible while the window was 15x too small: the wiring bug was
    /// hiding a methodological mismatch behind an accidental speed-up.
    Scls,

    /// **Naive homogenisation.** One smeared material — TRISO particles and
    /// the matrix they sit in, mixed at the packing fraction — fills the whole
    /// fuel zone. No double heterogeneity is left at all.
    ///
    /// Gives up *self-shielding entirely*: a neutron sees absorber spread
    /// everywhere at reduced density instead of concentrated in kernels it
    /// could have missed, so U-238's resonances lose the spatial shielding that
    /// protected them and reactivity drops hard. Measured at **-5151 pcm** on
    /// the bare reference pebble.
    ///
    /// Worth keeping despite that, because it is the **upper bound on what any
    /// DH treatment can save**: with the geometry gone completely, nothing
    /// cheaper is possible. Read its speed as a ceiling and its eigenvalue as a
    /// warning.
    ///
    /// The mixing conserves inventory. Filling the zone with pure particle
    /// material instead would give `1/pf` times the heavy metal and delete the
    /// matrix graphite that moderates the explicit pebble from the inside —
    /// that is a different reactor, not a homogenisation of this one.
    Homogenised,

    /// **Ring-RPT.** The particles are dissolved into an equivalent *annulus*
    /// of homogenised material, wrapped around an inner ball of matrix
    /// graphite.
    ///
    /// This is the method [`Self::Homogenised`] is often confused with, and the
    /// difference is the whole point. Concentrating the smeared fuel into a
    /// shell at the right radius keeps much of the *radial* self-shielding that
    /// full homogenisation throws away, which is why it lands within tens of
    /// pcm of explicit TRISO where the naive smear is thousands out.
    ///
    /// `inner_radius` \[cm\] is the single fitted knob; the outer radius follows
    /// from conserving particle volume,
    /// `r_outer^3 = inner_radius^3 + pf * fuel_zone_radius^3`
    /// ([`rpt_fuel_outer_radius`]). It is **fuel- and code-specific** — the
    /// reference value [`Self::FHR_REFERENCE_RPT_INNER`] was fitted against
    /// OpenMC for this crate's FHR pebble, and refitting it for another fuel,
    /// another data library, or another code is part of using the method, not
    /// an optional refinement. Passing a radius fitted elsewhere is the usual
    /// way to get a bad ring-RPT answer.
    ///
    /// # Errors
    ///
    /// [`DhUniverse::pebble`] returns [`DhError::Geometry`] if the conserved
    /// outer radius would fall outside the fuel zone, which happens when
    /// `inner_radius` is too large for the packing fraction.
    RingRpt {
        /// Inner radius of the homogenised fuel annulus \[cm\]; the fitted knob.
        inner_radius: f64,
    },
}

impl DhTreatment {
    /// Ring-RPT inner radius \[cm\] fitted against OpenMC for this crate's FHR
    /// reference pebble (19.9 % HALEU UCO TRISO, 30 % packing, 1.9 cm fuel
    /// zone).
    ///
    /// **This number does not transfer.** It is a fitted equivalence, and what
    /// it was fitted against — that fuel, that geometry, that code, that data
    /// library — is part of its definition. `examples/fhr_ring_rpt_endf.rs`
    /// measures how far this crate's own best-fit radius sits from it; the
    /// answer is not zero.
    pub const FHR_REFERENCE_RPT_INNER: f64 = 1.493_359_375;

    /// Every variant, for sweeping all five in a comparison.
    ///
    /// [`Self::RingRpt`] appears at [`Self::FHR_REFERENCE_RPT_INNER`], which is
    /// only meaningful on the FHR reference pebble. Sweeping `ALL` on some
    /// other fuel will build a ring-RPT arm at a radius fitted for something
    /// else — construct that variant yourself with your own fitted radius
    /// rather than reading a number out of this array.
    pub const ALL: [Self; 5] = [
        Self::DeltaTracking,
        Self::ChordLength,
        Self::Scls,
        Self::Homogenised,
        Self::RingRpt { inner_radius: Self::FHR_REFERENCE_RPT_INNER },
    ];

    /// Short human-readable name, e.g. for table rows.
    pub fn name(self) -> &'static str {
        match self {
            Self::DeltaTracking => "delta tracking",
            Self::ChordLength => "chord-length sampling (CLS)",
            Self::Scls => "semi-implicit CLS (SCLS)",
            Self::Homogenised => "naive homogenisation",
            Self::RingRpt { .. } => "ring-RPT (fitted annulus)",
        }
    }

    /// Whether this treatment resolves the particle geometry exactly.
    ///
    /// Only [`Self::DeltaTracking`] does. The other three are approximations
    /// and their eigenvalues must be read as such.
    pub fn is_exact(self) -> bool {
        matches!(self, Self::DeltaTracking)
    }

    /// Whether this treatment needs a parameter fitted against a reference
    /// calculation before it means anything.
    ///
    /// Only [`Self::RingRpt`] does — see [`fit_ring_rpt_inner_radius`], which
    /// finds that parameter for you rather than leaving you to guess it.
    pub fn needs_fitting(self) -> bool {
        matches!(self, Self::RingRpt { .. })
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
    /// Outer radius of a coolant shell around the pebble \[cm\], which becomes
    /// the reflective boundary. `None` puts the boundary at the pebble surface
    /// and models the pebble alone. Set it with [`Self::with_coolant`].
    pub coolant_radius: Option<f64>,
    /// Material table. Indices are the convention documented on
    /// [`DhUniverse::material_at`].
    pub materials: Vec<Material>,
    /// Packing seed — fixes the particle positions, so a fixed seed gives a
    /// reproducible universe.
    pub seed: u64,
}

impl PebbleParams {
    /// The FHR reference pebble **on its own**: 19.9 % HALEU UCO TRISO at 30 %
    /// packing, 1.9 cm fuel zone inside a 2.0 cm pebble, reflective at the
    /// pebble surface. No coolant.
    ///
    /// This is a bare pebble's own k-infinity and is **not** the system the
    /// published OpenMC numbers were computed on — strip the coolant off an FHR
    /// pebble and k falls by over 10 000 pcm. Use [`Self::fhr_unit_cell`] when
    /// you want a number comparable to those, and this one when you want the
    /// pebble isolated.
    ///
    /// `materials` is left empty and **must be filled in** before use — the
    /// geometry is reference data, the material composition is yours.
    pub fn fhr_reference() -> Self {
        Self {
            spec: TrisoSpec::FHR_HALEU_UCO,
            fuel_zone_radius: 1.9,
            pebble_radius: 2.0,
            coolant_radius: None,
            materials: Vec::new(),
            seed: 0x0DDF_1234_5678_9ABC,
        }
    }

    /// The FHR reference **unit cell**: [`Self::fhr_reference`]'s pebble with a
    /// 1 cm coolant shell around it, reflective at r = 3.0 cm.
    ///
    /// This is the geometry `examples/fhr_ring_rpt_endf.rs` and the OpenMC deck
    /// it is checked against both use, so eigenvalues computed here are
    /// comparable to the published `k = 1.36510 ± 0.00063` (explicit TRISO) and
    /// `k = 1.36479 ± 0.00067` (ring-RPT).
    ///
    /// Needs **eight** materials — the seven of [`DhUniverse::pebble`] plus the
    /// coolant at index 7. Supply FLiBe at the temperature the rest of the
    /// table uses; the deck it matches runs at 600 K.
    pub fn fhr_unit_cell() -> Self {
        Self { coolant_radius: Some(3.0), ..Self::fhr_reference() }
    }

    /// Replace the material table, returning `self` so constructors can chain.
    pub fn with_materials(mut self, materials: Vec<Material>) -> Self {
        self.materials = materials;
        self
    }

    /// Wrap the pebble in a coolant shell out to `outer_radius` \[cm\], which
    /// becomes the reflective boundary.
    ///
    /// Requires an eighth material (index 7) for the coolant. Pass a radius
    /// greater than `pebble_radius`, or [`DhUniverse::pebble`] returns
    /// [`DhError::Geometry`].
    pub fn with_coolant(mut self, outer_radius: f64) -> Self {
        self.coolant_radius = Some(outer_radius);
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
    /// Explicit particles in a pebble (fuel zone + shell + optional coolant).
    Pebble(ExplicitTrisoPebble),
    /// Explicit particles in a cube.
    Dispersed {
        packing: PackedSpheres,
        particle_material: usize,
        matrix_material: usize,
    },
    /// Chord-length sampling — no stored particles. The `Mutex` is what lets a
    /// stateful sampler satisfy the `Sync` point-query seam the k-eff drivers
    /// expect; on the single-threaded reference path it is uncontended.
    Cls {
        medium: Mutex<(ClsMedium, u64)>,
        particle_material: usize,
        matrix_material: usize,
        outer: OuterShells,
    },
    /// Semi-implicit chord-length sampling — CLS plus a retention window whose
    /// *flight* is reset at each history boundary through
    /// [`MaterialQuery::begin_history`](crate::pebble_beds::keff_delta::MaterialQuery::begin_history),
    /// while the retained inclusions persist as reconstructed geometry.
    ///
    /// `window_set` guards the one-time sizing of the retention radius, which
    /// needs cross sections and so cannot happen at construction.
    Scls {
        medium: Mutex<(SclsMedium, u64)>,
        window_set: std::sync::atomic::AtomicBool,
        particle_material: usize,
        matrix_material: usize,
        outer: OuterShells,
    },
    /// Naive homogenisation — one smeared material filling the whole fuel zone.
    Homogenised { homogenised: usize, outer: OuterShells },
    /// Ring-RPT — matrix ball inside `inner_radius`, homogenised particle
    /// material in the annulus out to `fuel_outer_radius`, matrix again from
    /// there to the fuel-zone boundary.
    RingRpt {
        inner_radius: f64,
        fuel_outer_radius: f64,
        homogenised: usize,
        matrix_material: usize,
        outer: OuterShells,
    },
}

/// The fuel-free regions every pebble treatment shares: graphite shell, then an
/// optional coolant shell out to the reflective boundary.
///
/// Factored out because getting it wrong is silent and expensive — the crate's
/// own ring-RPT deck records a cube-corner coolant over-count worth thousands
/// of pcm — and because four treatments must answer it identically or their
/// eigenvalues are not comparable.
#[derive(Debug, Clone, Copy)]
struct OuterShells {
    fuel_zone_radius: f64,
    pebble_radius: f64,
    shell_material: usize,
    coolant_material: Option<usize>,
}

impl OuterShells {
    /// No shells at all — dispersed fuel in a cube, where the medium fills the
    /// whole domain and every point is the treatment's own business.
    #[inline]
    fn none() -> Self {
        Self {
            fuel_zone_radius: f64::INFINITY,
            pebble_radius: f64::INFINITY,
            shell_material: 0,
            coolant_material: None,
        }
    }

    /// Material at radius `r`, or `None` while `r` is still inside the fuel
    /// zone and the treatment must answer for itself.
    /// Every material index this shell set can return, for the majorant bound.
    fn reachable(&self) -> Vec<usize> {
        let mut v = vec![self.shell_material];
        if let Some(c) = self.coolant_material {
            v.push(c);
        }
        v
    }

    #[inline]
    fn at(&self, r: f64) -> Option<usize> {
        if r < self.fuel_zone_radius {
            None
        } else if r < self.pebble_radius {
            Some(self.shell_material)
        } else {
            // With no coolant the domain stops at the pebble surface, so this
            // arm is only reached by rounding at the boundary itself.
            Some(self.coolant_material.unwrap_or(self.shell_material))
        }
    }
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
    packing_fraction: f64,
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
        const MATRIX_IDX: usize = 5;
        const SHELL_IDX: usize = 6;
        const COOLANT_IDX: usize = 7;

        // The coolant shell, if any, moves the reflective boundary outward and
        // costs an eighth material. Both are checked here rather than surfacing
        // later as a wrong eigenvalue.
        let coolant_material = match params.coolant_radius {
            None => None,
            Some(r_cool) => {
                if r_cool <= params.pebble_radius {
                    return Err(DhError::Geometry(format!(
                        "coolant_radius {r_cool} must be > pebble_radius {}",
                        params.pebble_radius
                    )));
                }
                Some(COOLANT_IDX)
            }
        };
        let needed = if coolant_material.is_some() { 8 } else { 7 };
        if params.materials.len() < needed {
            return Err(DhError::Materials(format!(
                "pebble needs {needed} materials [kernel, buffer, ipyc, sic, opyc, matrix, \
                 shell{}], got {}",
                if needed == 8 { ", coolant" } else { "" },
                params.materials.len()
            )));
        }

        let pf = params.spec.packing_fraction;
        let r_particle = params.spec.opyc;
        // What the explicit packing actually realises inside the fuel zone. The
        // approximate treatments do not pack, so they smear at the requested
        // `pf`; `pack_in_ball` is responsible for making that the same number.
        // Kept as a field so a caller can check rather than assume.
        //
        // Only the delta-tracking arm falls through to the final constructor;
        // every approximate arm returns early with `pf`, which is exactly the
        // fraction it smears at. So this is assigned on the one path that reads
        // it, and the compiler checks that rather than a default hiding a gap.
        let achieved_pf: f64;
        let r_boundary = params.coolant_radius.unwrap_or(params.pebble_radius);
        let domain = DeltaDomain::Sphere { radius: r_boundary };
        let outer = OuterShells {
            fuel_zone_radius: params.fuel_zone_radius,
            pebble_radius: params.pebble_radius,
            shell_material: SHELL_IDX,
            coolant_material,
        };

        let (geometry, particles) = match treatment {
            DhTreatment::DeltaTracking => {
                let (packing, realised) =
                    pack_in_ball(r_particle, params.fuel_zone_radius, pf, params.seed)?;
                achieved_pf = realised;
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
                    // With no coolant the domain stops at the pebble surface, so
                    // no history reaches a coolant region; naming the shell keeps
                    // the index in range without inventing a material.
                    coolant_material.unwrap_or(SHELL_IDX),
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
                let medium =
                    ClsMedium::new(r_particle, pf, MaterialId(particle_idx), MaterialId(MATRIX_IDX));
                return Ok(Self {
                    treatment,
                    geometry: DhGeometry::Cls {
                        medium: Mutex::new((medium, params.seed | 1)),
                        particle_material: particle_idx,
                        matrix_material: MATRIX_IDX,
                        outer,
                    },
                    materials,
                    domain,
                    particles: 0,
                    packing_fraction: pf,
                });
            }
            DhTreatment::Scls => {
                let particle = homogenise_particle(&params.materials, params.spec)?;
                let mut materials = params.materials.clone();
                materials.push(particle);
                let particle_idx = materials.len() - 1;
                let cls =
                    ClsMedium::new(r_particle, pf, MaterialId(particle_idx), MaterialId(MATRIX_IDX));
                // Retention window radius. SCLS remembers inclusions within one
                // transport mean free path; the matrix mean chord is the length
                // scale the sampler itself works in, so it is the natural stand-in
                // for a spectrum-averaged mfp we do not have at construction time.
                let window = cls.mean_chord_matrix();
                let medium = SclsMedium::new(cls, Position::new(0.0, 0.0, 0.0), window);
                return Ok(Self {
                    treatment,
                    geometry: DhGeometry::Scls {
                        medium: Mutex::new((medium, params.seed | 1)),
                        window_set: std::sync::atomic::AtomicBool::new(false),
                        particle_material: particle_idx,
                        matrix_material: MATRIX_IDX,
                        outer,
                    },
                    materials,
                    domain,
                    particles: 0,
                    packing_fraction: pf,
                });
            }
            DhTreatment::Homogenised => {
                // Particles AND the matrix they sit in, smeared over the whole
                // fuel zone. Mixing at the packing fraction is what conserves
                // inventory: filling the zone with pure particle material would
                // give 1/pf times the heavy metal and delete the matrix graphite
                // that moderates the explicit pebble from the inside.
                let particle = homogenise_particle(&params.materials, params.spec)?;
                let matrix = params.materials[MATRIX_IDX].clone();
                let smeared = homogenise_by_volume(
                    &[(&particle, pf), (&matrix, 1.0 - pf)],
                    900,
                    "naive-homogenised fuel zone (TRISO + matrix)",
                    matrix.temperature,
                );
                let mut materials = params.materials.clone();
                materials.push(smeared);
                let idx = materials.len() - 1;
                return Ok(Self {
                    treatment,
                    geometry: DhGeometry::Homogenised { homogenised: idx, outer },
                    materials,
                    domain,
                    particles: 0,
                    packing_fraction: pf,
                });
            }
            DhTreatment::RingRpt { inner_radius } => {
                // Unlike the naive case, the annulus is sized to the particle
                // volume alone, so the material that fills it is the homogenised
                // PARTICLE with no matrix mixed in. Mixing matrix in here would
                // double-count it — the matrix is already represented by the
                // inner ball and the outer remainder of the fuel zone.
                if !(inner_radius >= 0.0) || inner_radius >= params.fuel_zone_radius {
                    return Err(DhError::Geometry(format!(
                        "ring-RPT inner_radius {inner_radius} must be in [0, fuel_zone_radius {})",
                        params.fuel_zone_radius
                    )));
                }
                let r_outer3 =
                    inner_radius.powi(3) + pf * params.fuel_zone_radius.powi(3);
                let fuel_outer_radius = r_outer3.cbrt();
                if fuel_outer_radius > params.fuel_zone_radius {
                    return Err(DhError::Geometry(format!(
                        "ring-RPT annulus outer radius {fuel_outer_radius:.4} exceeds the fuel \
                         zone {:.4} — inner_radius {inner_radius} is too large for packing \
                         fraction {pf}",
                        params.fuel_zone_radius
                    )));
                }
                let particle = homogenise_particle(&params.materials, params.spec)?;
                let mut materials = params.materials.clone();
                materials.push(particle);
                let idx = materials.len() - 1;
                return Ok(Self {
                    treatment,
                    geometry: DhGeometry::RingRpt {
                        inner_radius,
                        fuel_outer_radius,
                        homogenised: idx,
                        matrix_material: MATRIX_IDX,
                        outer,
                    },
                    materials,
                    domain,
                    particles: 0,
                    packing_fraction: pf,
                });
            }
        };

        Ok(Self {
            treatment,
            geometry,
            materials: params.materials,
            domain,
            particles,
            packing_fraction: achieved_pf,
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
                        outer: OuterShells::none(),
                    },
                    0,
                )
            }
            DhTreatment::Scls => {
                let cls = ClsMedium::new(
                    params.particle_radius,
                    params.packing_fraction,
                    MaterialId(PARTICLE),
                    MaterialId(MATRIX),
                );
                let window = cls.mean_chord_matrix();
                let medium = SclsMedium::new(cls, Position::new(0.0, 0.0, 0.0), window);
                (
                    DhGeometry::Scls {
                        medium: Mutex::new((medium, params.seed | 1)),
                        window_set: std::sync::atomic::AtomicBool::new(false),
                        particle_material: PARTICLE,
                        matrix_material: MATRIX,
                        outer: OuterShells::none(),
                    },
                    0,
                )
            }
            DhTreatment::Homogenised => {
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
                    geometry: DhGeometry::Homogenised {
                        homogenised: idx,
                        outer: OuterShells::none(),
                    },
                    materials,
                    domain,
                    particles: 0,
                    packing_fraction: params.packing_fraction,
                });
            }
            DhTreatment::RingRpt { .. } => {
                // Ring-RPT concentrates the smeared fuel into a spherical
                // annulus at a fitted radius. A cube of uniformly dispersed
                // particles has no radial structure for that annulus to sit in,
                // so there is nothing to fit and no shell to place. Refusing is
                // the honest answer; silently falling back to Homogenised would
                // report a ring-RPT number that was never computed.
                return Err(DhError::Geometry(
                    "ring-RPT is a spherical construction and does not apply to dispersed fuel \
                     in a cube — there is no radial structure to fit an annulus to. Use \
                     DhTreatment::Homogenised for the smeared case, or DhUniverse::pebble if \
                     the geometry really is a pebble."
                        .into(),
                ));
            }
        };

        Ok(Self {
            treatment,
            geometry,
            materials: params.materials,
            domain,
            particles,
            packing_fraction: params.packing_fraction,
        })
    }

    /// The particle volume fraction this universe **actually** models.
    ///
    /// For [`DhTreatment::DeltaTracking`] this is measured from the packing that
    /// was generated, not the number that was requested — those differ, because
    /// the packer targets a cube while the fuel zone is the inscribed ball. For
    /// every approximate treatment it is the requested fraction, since they
    /// smear rather than pack.
    ///
    /// **Check it against what you asked for.** Until 2026-09-14 the explicit
    /// arm silently realised 0.2894 against a requested 0.30 while the smeared
    /// arms mixed at 0.30 exactly, so a comparison of *treatments* was also a
    /// comparison of two fuel loadings 3.7 % apart — and the extra fuel
    /// flattered every smeared arm. `pack_in_ball` now iterates onto the
    /// requested fraction, and this accessor exists so the assumption is
    /// checkable rather than implicit.
    pub fn packing_fraction(&self) -> f64 {
        self.packing_fraction
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
                outer,
            } => {
                if let Some(m) = outer.at(p.norm()) {
                    return Some(m);
                }
                let mut guard = medium.lock().ok()?;
                let (cls, seed) = &mut *guard;
                match cls.material_at(p, seed) {
                    Ok(id) if id.index() == *particle_material => Some(*particle_material),
                    _ => Some(*matrix_material),
                }
            }
            DhGeometry::Scls {
                medium,
                particle_material,
                matrix_material,
                outer,
                ..
            } => {
                if let Some(m) = outer.at(p.norm()) {
                    return Some(m);
                }
                let mut guard = medium.lock().ok()?;
                let (scls, seed) = &mut *guard;
                match scls.material_at(p, seed) {
                    Ok(id) if id.index() == *particle_material => Some(*particle_material),
                    _ => Some(*matrix_material),
                }
            }
            DhGeometry::Homogenised { homogenised, outer } => {
                outer.at(p.norm()).or(Some(*homogenised))
            }
            DhGeometry::RingRpt {
                inner_radius,
                fuel_outer_radius,
                homogenised,
                matrix_material,
                outer,
            } => {
                let r = p.norm();
                if let Some(m) = outer.at(r) {
                    return Some(m);
                }
                // Inside the fuel zone: matrix ball, then the homogenised
                // annulus that carries all the particle material, then matrix
                // again out to the fuel-zone boundary.
                Some(if r < *inner_radius {
                    *matrix_material
                } else if r < *fuel_outer_radius {
                    *homogenised
                } else {
                    *matrix_material
                })
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
        // majorant is a SILENT bias in delta tracking, not a crash — which is
        // why `reachable_materials` must return every index `material_at` can
        // produce, and is derived from the geometry rather than guessed.
        let reachable = self.reachable_materials();
        let majorant = Majorant::bounding(&reachable, nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.3);
        self.size_scls_window(nuclides);
        run_keff_delta_in(
            self.domain,
            &self.materials,
            nuclides,
            &majorant,
            self,
            settings,
        )
    }
}

impl DhUniverse {
    /// The materials [`Self::material_at`] can actually return — which for every
    /// approximate treatment is a **strict subset** of [`Self::materials`].
    ///
    /// # Why this exists, and why getting it wrong is expensive in both directions
    ///
    /// An approximate treatment synthesises a smeared material and stops
    /// returning the ones it was smeared from, but those originals stay in the
    /// table so that indices do not shift. Bounding the delta-tracking majorant
    /// over the whole table therefore bounds materials **no history can ever
    /// reach** — and on a TRISO pebble the unreachable one is the undiluted
    /// kernel, whose U-238 resonance peaks tower over everything the smeared
    /// geometry actually contains.
    ///
    /// That costs time quadratically in a sense that matters: delta tracking
    /// accepts a collision with probability `sigma_t / sigma_maj`, so inflating
    /// the majorant by a factor N multiplies the virtual-collision count — and
    /// hence the point queries, and hence the whole run — by roughly N.
    ///
    /// It does **not** bias the eigenvalue. An over-bound majorant is wasteful;
    /// only an *under*-bound one is wrong, and silently so. That asymmetry is
    /// why this function is derived from the geometry arm by arm rather than
    /// filtered by a heuristic: a missed index would trade a performance bug for
    /// a correctness bug.
    fn reachable_materials(&self) -> Vec<Material> {
        self.reachable_material_indices()
            .into_iter()
            .map(|i| self.materials[i].clone())
            .collect()
    }

    /// The index set behind [`Self::reachable_materials`], separated so a test
    /// can check it against what [`Self::material_at`] actually returns.
    fn reachable_material_indices(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = match &self.geometry {
            // Explicit geometry reaches every material the caller supplied.
            DhGeometry::Pebble(_) => (0..self.materials.len()).collect(),
            DhGeometry::Dispersed { particle_material, matrix_material, .. } => {
                vec![*particle_material, *matrix_material]
            }
            DhGeometry::Cls { particle_material, matrix_material, outer, .. }
            | DhGeometry::Scls { particle_material, matrix_material, outer, .. } => {
                let mut v = vec![*particle_material, *matrix_material];
                v.extend(outer.reachable());
                v
            }
            DhGeometry::Homogenised { homogenised, outer } => {
                let mut v = vec![*homogenised];
                v.extend(outer.reachable());
                v
            }
            DhGeometry::RingRpt { homogenised, matrix_material, outer, .. } => {
                let mut v = vec![*homogenised, *matrix_material];
                v.extend(outer.reachable());
                v
            }
        };
        idx.sort_unstable();
        idx.dedup();
        idx
    }
}

/// A summary rather than a dump: the treatment, how many particles are stored,
/// how many materials the table ended up with, and the reflective boundary.
/// Printing the packing itself would be tens of thousands of spheres.
impl core::fmt::Debug for DhUniverse {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DhUniverse")
            .field("treatment", &self.treatment)
            .field("particles", &self.particles)
            .field("materials", &self.materials.len())
            .field("domain", &self.domain)
            .finish()
    }
}

impl DhUniverse {
    /// Size the SCLS retention window from real cross sections, once.
    ///
    /// SCLS's rule is `R = lambda_transport-mfp + R_largest`. The justification
    /// is physical: one transport mean free path is the distance over which the
    /// neutron's direction decorrelates, so geometry further away is unlikely to
    /// be revisited before it would have been forgotten anyway. `R_largest`
    /// guarantees an inclusion whose *body* still overlaps the neighbourhood is
    /// retained even when its centre has just left.
    ///
    /// `lambda_transport-mfp` is a property of the **material and energy**, not
    /// of the geometry, so it cannot be known when [`Self::pebble`] runs — there
    /// are no nuclides yet. It is therefore set here, on the first
    /// [`Self::keff`], and guarded so repeated calls do not re-cull.
    ///
    /// # The bug this replaces
    ///
    /// Construction seeded the window with `ClsMedium::mean_chord_matrix()`,
    /// which is a **geometric** length — the mean distance *between* inclusions
    /// — and not a transport mean free path, which is an *interaction* length.
    /// On the FHR pebble that is 0.132 cm against roughly 2.6 cm: the window was
    /// about **15x too small**, so nearly every remembered inclusion was culled
    /// almost immediately and SCLS behaved like CLS. Combined with a
    /// `begin_history` that also wiped the memory outright, that is why the two
    /// measured within statistics of each other.
    ///
    /// # Approximations, stated
    ///
    /// - **One fixed radius for all energies.** The transport mfp varies by
    ///   orders of magnitude across the spectrum; this evaluates it at thermal
    ///   (0.0253 eV), which is where a graphite-moderated pebble does most of
    ///   its scattering. A variable-radius SCLS is the design's own answer to
    ///   this ([`SclsMedium::adapt_radius`], bead `op-eby.6`) and is not wired
    ///   up here.
    /// - **`lambda_tr` approximated by `1 / Sigma_t`**, i.e. the anisotropy
    ///   correction `1 / (1 - mu_bar)` is dropped. For carbon `mu_bar = 2/(3A)`
    ///   is 0.056, so this understates the window by ~6 % — three orders of
    ///   magnitude less wrong than what it replaces, and in the conservative
    ///   direction (a slightly smaller window retains slightly less).
    fn size_scls_window(&self, nuclides: &[Nuclide]) {
        use std::sync::atomic::Ordering;
        let DhGeometry::Scls { medium, window_set, matrix_material, .. } = &self.geometry else {
            return;
        };
        if window_set.swap(true, Ordering::SeqCst) {
            return;
        }
        const THERMAL_EV: f64 = 0.0253;
        let sigma_t = self.materials[*matrix_material].macro_xs_total(THERMAL_EV, nuclides);
        if !(sigma_t > 0.0) {
            return; // a void matrix has no transport length to speak of
        }
        let Ok(mut guard) = medium.lock() else { return };
        let r_largest = guard.0.cls().inclusion_radius();
        let window = 1.0 / sigma_t + r_largest;

        // Beyond the domain there is no geometry to retain, so a larger window
        // buys nothing and only slows the retained-inclusion scan.
        let cap = self.domain.bounding_half();
        if window >= cap {
            log::warn!(
                "SCLS retention window {window:.2} cm reaches or exceeds the domain \
                 ({cap:.2} cm): nothing will ever be culled, so SCLS degenerates to \
                 remember-everything with a linear scan and will be SLOWER than explicit \
                 delta tracking, which has an O(1) grid. See DhTreatment::Scls."
            );
        }
        guard.0.set_sphere_radius(window.min(cap));
    }
}

/// Lets a [`DhUniverse`] be handed straight to the delta-tracked k-eff drivers,
/// and — the reason this is a trait impl rather than a closure — lets
/// [`DhTreatment::Scls`] throw away its retention window at each history
/// boundary, which is what makes it SCLS rather than CLS with a leak.
impl MaterialQuery for &DhUniverse {
    #[inline]
    fn material_at(&self, p: Position) -> Option<usize> {
        DhUniverse::material_at(self, p)
    }

    fn begin_history(&self) {
        // Only SCLS carries per-history state, and the distinction between the
        // two kinds of state is the whole method.
        //
        // `begin_flight` discards the in-progress FLIGHT — the phase the
        // reconstruction is currently in — and deliberately KEEPS the retained
        // inclusions, because those are *geometry*: a progressively
        // reconstructed model of the packing that a later history in the same
        // neighbourhood should still benefit from. Throwing them away every
        // history is what degrades SCLS back into CLS, since each history would
        // then start with no memory at all and could only accumulate within its
        // own flight.
        //
        // This function did exactly that until 2026-09-14 (it restored a
        // pristine clone), which is why SCLS and CLS measured within statistics
        // of each other -- see the correction on [`DhTreatment::Scls`].
        if let DhGeometry::Scls { medium, .. } = &self.geometry {
            if let Ok(mut guard) = medium.lock() {
                guard.0.begin_flight();
            }
        }
    }
}

/// Find the ring-RPT inner radius that reproduces the explicit-TRISO
/// eigenvalue for a given pebble — the fit the method cannot be used without.
///
/// [`DhTreatment::RingRpt`] takes an `inner_radius` that is a **fitted
/// equivalence**, not a physical dimension. Published values are fitted for one
/// fuel, one data library and one code, and do not transfer. This runs the fit
/// for *your* pebble with *your* data, so you do not have to guess or borrow a
/// number: it solves the explicit delta-tracked pebble once for a target, then
/// bisects on the radius until ring-RPT matches it.
///
/// ```no_run
/// use outram_mc_libs::prelude::*;
/// # let materials: Vec<Material> = Vec::new();
/// # let nuclides: Vec<Nuclide> = Vec::new();
/// let params = PebbleParams::fhr_unit_cell().with_materials(materials);
/// let settings = KeffSettings::default();
///
/// let fit = fit_ring_rpt_inner_radius(&params, &nuclides, &settings, None)?;
/// println!("fitted inner radius = {:.4} cm (target k = {:.5})", fit.inner_radius, fit.target_k);
///
/// // Then use it.
/// let universe = DhUniverse::pebble(params, fit.treatment())?;
/// # Ok::<(), DhError>(())
/// ```
///
/// # Cost
///
/// One explicit eigenvalue solve for the target, then one ring-RPT solve per
/// bisection step (up to `max_iterations`, default 12). Budget roughly a dozen
/// times a single `keff()` call. Raise `settings.n_particles` before trusting a
/// tight fit — bisecting on a noisy residual converges to noise, and the
/// returned `target_std` tells you how noisy the target itself was.
///
/// # What the fit does and does not mean
///
/// A fitted radius makes ring-RPT reproduce the explicit **eigenvalue**. It
/// does not make the smeared pebble a good model of anything else about the
/// explicit one — the flux shape inside the fuel zone is not the same, and a
/// tally that resolves radius will not agree. Fit for the quantity you care
/// about, and say which one it was.
///
/// # Errors
///
/// [`DhError::Geometry`] if the bracket does not straddle the target (widen it,
/// or check that ring-RPT can reach the explicit k at all for this fuel), and
/// whatever [`DhUniverse::pebble`] returns if the pebble will not build.
pub fn fit_ring_rpt_inner_radius(
    params: &PebbleParams,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
    bracket: Option<(f64, f64)>,
) -> Result<RingRptFit, DhError> {
    let explicit = DhUniverse::pebble(params.clone(), DhTreatment::DeltaTracking)?;
    let target = explicit.keff(nuclides, settings);

    // Default bracket: from a solid smear (inner radius 0, i.e. all the
    // particle material in a central ball) out to most of the fuel zone. The
    // upper end is capped by conservation — the annulus cannot leave the zone.
    let max_inner = (params.fuel_zone_radius.powi(3)
        * (1.0 - params.spec.packing_fraction))
        .cbrt();
    let (lo, hi) = bracket.unwrap_or((0.05 * params.fuel_zone_radius, 0.985 * max_inner));

    let k_at = |r: f64| -> Result<KeffResult, DhError> {
        let u = DhUniverse::pebble(params.clone(), DhTreatment::RingRpt { inner_radius: r })?;
        Ok(u.keff(nuclides, settings))
    };

    let (mut lo, mut hi) = (lo.min(hi), lo.max(hi));
    let mut k_lo = k_at(lo)?.k_mean;
    let mut k_hi = k_at(hi)?.k_mean;
    let (g_lo, g_hi) = (k_lo - target.k_mean, k_hi - target.k_mean);
    if g_lo * g_hi > 0.0 {
        return Err(DhError::Geometry(format!(
            "ring-RPT fit bracket [{lo:.4}, {hi:.4}] cm does not straddle the explicit k \
             {:.5}: k(lo) = {k_lo:.5}, k(hi) = {k_hi:.5}. Both residuals have the same sign, \
             so either widen the bracket or accept that ring-RPT cannot reach the explicit \
             eigenvalue for this fuel.",
            target.k_mean
        )));
    }

    let max_iterations = 12;
    let mut evaluations = 2;
    let mut mid = 0.5 * (lo + hi);
    let mut k_mid = target.k_mean;
    for _ in 0..max_iterations {
        mid = 0.5 * (lo + hi);
        let r = k_at(mid)?;
        evaluations += 1;
        k_mid = r.k_mean;
        let g_mid = k_mid - target.k_mean;
        // Stop once the residual is inside the target's own statistics: past
        // that point bisection is chasing Monte Carlo noise, not the radius.
        if g_mid.abs() <= target.k_std {
            break;
        }
        if (k_lo - target.k_mean) * g_mid < 0.0 {
            hi = mid;
            k_hi = k_mid;
        } else {
            lo = mid;
            k_lo = k_mid;
        }
    }
    let _ = (k_hi, k_lo);

    Ok(RingRptFit {
        inner_radius: mid,
        fitted_k: k_mid,
        target_k: target.k_mean,
        target_std: target.k_std,
        residual_pcm: (k_mid - target.k_mean) * 1.0e5,
        evaluations,
    })
}

/// What [`fit_ring_rpt_inner_radius`] found.
#[derive(Debug, Clone, Copy)]
pub struct RingRptFit {
    /// The fitted ring-RPT inner radius \[cm\]. Feed it to [`Self::treatment`].
    pub inner_radius: f64,
    /// Eigenvalue ring-RPT gives at that radius.
    pub fitted_k: f64,
    /// Eigenvalue of the explicit delta-tracked pebble — what was fitted to.
    pub target_k: f64,
    /// Statistical standard deviation on `target_k`. **The fit is meaningless
    /// below this**; bisection stops once the residual is inside it.
    pub target_std: f64,
    /// `fitted_k - target_k` in pcm. Compare against `target_std * 1e5`.
    pub residual_pcm: f64,
    /// Eigenvalue solves spent, target included.
    pub evaluations: usize,
}

impl RingRptFit {
    /// The treatment this fit produced, ready to pass to
    /// [`DhUniverse::pebble`].
    pub fn treatment(&self) -> DhTreatment {
        DhTreatment::RingRpt { inner_radius: self.inner_radius }
    }

    /// Whether the fit converged to inside the target's own statistics.
    ///
    /// `false` means the bisection ran out of iterations while the residual was
    /// still resolvable — the radius returned is the best bracket midpoint, not
    /// a converged answer.
    pub fn converged(&self) -> bool {
        self.residual_pcm.abs() <= self.target_std * 1.0e5
    }
}

/// RSA-pack whole particles into a ball of `radius`, **hitting the requested
/// packing fraction inside that ball** rather than inside the cube it was
/// generated in.
///
/// Returns the packing and the fraction it actually realised.
///
/// # Why this needs an iteration at all
///
/// [`PackingConfig`] targets its packing fraction over a **cube** of half-width
/// `radius + particle_radius`, and this function then keeps only the spheres
/// lying *wholly* inside the inscribed ball. Both steps move the fraction, in
/// opposite directions and by different amounts, so the in-ball result is not
/// the number that was asked for: on the FHR reference pebble a request of 0.30
/// realised **0.2894**, i.e. 3.7 % less heavy metal than specified.
///
/// That mattered because the approximate treatments do not pack — they smear at
/// `spec.packing_fraction` directly. An explicit arm at 0.2894 compared against
/// smeared arms at 0.3000 is not a comparison of *treatments*, it is a
/// comparison of two different fuel loadings, and the extra fuel flatters every
/// smeared arm. `examples/fhr_ring_rpt_endf.rs` already corrected for this with
/// a one-step rescale; this constructor did not.
///
/// # The measurement is exact, not sampled
///
/// Every kept sphere lies wholly inside the ball, so the realised fraction is
/// `n * (r / R)^3` in closed form — no Monte Carlo estimate and no sampling
/// error. (`PackedSpheres::volume_fraction_in_ball` samples, which is the right
/// tool when spheres may straddle the boundary; here none do.)
///
/// # Convergence
///
/// The kept count is very nearly linear in the requested fraction, so a
/// secant-style rescale `request *= target / realised` converges in two or
/// three steps. The loop is capped and returns the best attempt rather than
/// spinning.
///
/// # Cost
///
/// **This packs more than once**, where the previous version packed exactly
/// once — typically twice, since the first attempt is off by ~3.7 % and the
/// tolerance is 0.2 %. RSA-packing ~26 000 spheres is not free, so
/// constructing a [`DhTreatment::DeltaTracking`] universe now costs roughly
/// double what it did.
///
/// That is a deliberate trade: the alternative is an explicit arm modelling a
/// different fuel loading from every arm it is compared against, which is a
/// wrong answer rather than a slow one. Only the explicit arm pays it; the
/// approximate treatments never pack. If it ever matters, the correction
/// factor is deterministic in `(particle_radius, radius, packing_fraction,
/// seed)` and could be cached rather than re-derived.
fn pack_in_ball(
    particle_radius: f64,
    radius: f64,
    packing_fraction: f64,
    seed: u64,
) -> Result<(PackedSpheres, f64), DhError> {
    const TOLERANCE: f64 = 2.0e-3; // 0.2 % of the requested fraction
    const MAX_STEPS: usize = 6;

    let half = radius + particle_radius;
    let unit = (particle_radius / radius).powi(3); // one sphere's share of the ball

    let attempt = |request: f64| -> Result<(PackedSpheres, f64), DhError> {
        let cfg = PackingConfig {
            particle_radius,
            packing_fraction: request,
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
        let realised = kept.len() as f64 * unit;
        Ok((PackedSpheres::from_spheres(kept, half, particle_radius), realised))
    };

    let mut request = packing_fraction;
    let (mut packing, mut realised) = attempt(request)?;
    let mut best = (packing, realised);
    let mut best_err = (realised / packing_fraction - 1.0).abs();

    for _ in 0..MAX_STEPS {
        if best_err <= TOLERANCE {
            break;
        }
        // Secant rescale. Clamped below random close packing so a bad step
        // cannot ask the generator for something it must refuse.
        request = (request * packing_fraction / realised).clamp(1.0e-4, 0.63);
        let (p, r) = attempt(request)?;
        packing = p;
        realised = r;
        let err = (realised / packing_fraction - 1.0).abs();
        if err < best_err {
            best_err = err;
            best = (packing, realised);
        }
    }

    Ok(best)
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
        for t in [DhTreatment::ChordLength, DhTreatment::Scls, DhTreatment::Homogenised, DhTreatment::RingRpt { inner_radius: DhTreatment::FHR_REFERENCE_RPT_INNER }] {
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
        for t in [DhTreatment::ChordLength, DhTreatment::Scls, DhTreatment::Homogenised, DhTreatment::RingRpt { inner_radius: DhTreatment::FHR_REFERENCE_RPT_INNER }] {
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
        let p = || DispersedParams {
            particle_radius: 0.04,
            packing_fraction: 0.25,
            half_width: 1.0,
            materials: dummy_materials(2),
            seed: 42,
        };
        for t in [
            DhTreatment::DeltaTracking,
            DhTreatment::ChordLength,
            DhTreatment::Scls,
            DhTreatment::Homogenised,
        ] {
            let u = DhUniverse::dispersed(p(), t)
                .unwrap_or_else(|e| panic!("dispersed {} failed: {e}", t.name()));
            assert_eq!(u.treatment(), t);
            assert!(u.material_at(Position { x: 0.0, y: 0.0, z: 0.0 }).is_some());
        }
    }

    /// Ring-RPT is a spherical construction, so a cube of dispersed fuel must
    /// **refuse** it rather than quietly fall back to the naive smear. A silent
    /// fallback would report a ring-RPT number that was never computed — which
    /// is the exact failure mode this variant was split out to end.
    #[test]
    fn dispersed_fuel_refuses_ring_rpt_rather_than_falling_back() {
        let p = DispersedParams {
            particle_radius: 0.04,
            packing_fraction: 0.25,
            half_width: 1.0,
            materials: dummy_materials(2),
            seed: 42,
        };
        let e = DhUniverse::dispersed(
            p,
            DhTreatment::RingRpt { inner_radius: DhTreatment::FHR_REFERENCE_RPT_INNER },
        )
        .expect_err("ring-RPT on a cube should be refused");
        assert!(matches!(e, DhError::Geometry(_)), "wrong error kind: {e}");
        // The message must point at the treatment that IS right for a cube,
        // rather than only saying no.
        assert!(format!("{e}").contains("Homogenised"), "unhelpful message: {e}");
    }

    /// The two smearing treatments must be distinguishable through the API, or
    /// the rename accomplished nothing: ring-RPT puts the particle material in
    /// an annulus with matrix inside it, naive homogenisation fills the whole
    /// zone with one material.
    #[test]
    fn ring_rpt_is_radially_structured_and_naive_homogenisation_is_not() {
        let at = |t: DhTreatment, r: f64| {
            DhUniverse::pebble(params(), t)
                .unwrap()
                .material_at(Position { x: 0.0, y: 0.0, z: r })
        };
        let rpt = DhTreatment::RingRpt { inner_radius: DhTreatment::FHR_REFERENCE_RPT_INNER };

        // Naive: same material at the centre and near the fuel-zone edge.
        assert_eq!(at(DhTreatment::Homogenised, 0.0), at(DhTreatment::Homogenised, 1.8));

        // Ring-RPT: matrix in the middle, homogenised fuel in the annulus.
        // r_outer^3 = 1.493359375^3 + 0.30 * 1.9^3 -> r_outer ~ 1.746 cm.
        let centre = at(rpt, 0.0).expect("centre");
        let annulus = at(rpt, 1.6).expect("annulus");
        assert_ne!(centre, annulus, "ring-RPT annulus is not distinct from its inner ball");
        assert_eq!(centre, 5, "ring-RPT inner ball should be the matrix (index 5)");
        assert_eq!(at(rpt, 1.8), Some(5), "outside the annulus should be matrix again");
    }

    /// A ring-RPT radius too large for the packing fraction cannot conserve
    /// particle volume inside the fuel zone; that is reported, not panicked.
    #[test]
    fn ring_rpt_rejects_an_inner_radius_that_cannot_conserve_volume() {
        let e = DhUniverse::pebble(params(), DhTreatment::RingRpt { inner_radius: 1.88 })
            .expect_err("an annulus outside the fuel zone should be refused");
        assert!(matches!(e, DhError::Geometry(_)), "wrong error kind: {e}");
    }

    /// A coolant shell moves the reflective boundary and costs an eighth
    /// material. Both halves of that contract are checked.
    #[test]
    fn coolant_shell_requires_an_eighth_material() {
        let seven = PebbleParams::fhr_unit_cell().with_materials(dummy_materials(7));
        assert!(matches!(
            DhUniverse::pebble(seven, DhTreatment::DeltaTracking),
            Err(DhError::Materials(_))
        ));

        let eight = PebbleParams::fhr_unit_cell().with_materials(dummy_materials(8));
        let u = DhUniverse::pebble(eight, DhTreatment::Homogenised).expect("unit cell builds");
        // Coolant fills 2.0 < r < 3.0 for every treatment.
        assert_eq!(u.material_at(Position { x: 0.0, y: 0.0, z: 2.5 }), Some(7));
        assert_eq!(u.material_at(Position { x: 0.0, y: 0.0, z: 1.95 }), Some(6));
    }

    /// **The safety property behind the majorant.** `reachable_materials` bounds
    /// the delta-tracking majorant, and an under-bound majorant is a *silent*
    /// bias — the run completes and the answer is wrong. So the set it returns
    /// must contain every index `material_at` can produce, for every treatment.
    ///
    /// This samples the domain densely and checks exactly that. It is a
    /// necessary condition rather than a proof, but it is the condition that
    /// fails first if a future treatment forgets to declare one of its
    /// materials.
    #[test]
    fn every_material_the_geometry_returns_is_declared_reachable() {
        let mut seed = 0x5EED_1234_u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 11) as f64) / ((1u64 << 53) as f64)
        };

        for t in DhTreatment::ALL {
            let u = DhUniverse::pebble(
                PebbleParams::fhr_unit_cell().with_materials(dummy_materials(8)),
                t,
            )
            .unwrap_or_else(|e| panic!("{} failed to build: {e}", t.name()));

            let declared = u.reachable_material_indices();
            let r_max = 3.0;
            for _ in 0..200_000 {
                // Uniform in the ball by rejection — cheap and unbiased.
                let p = loop {
                    let c = Position {
                        x: r_max * (2.0 * next() - 1.0),
                        y: r_max * (2.0 * next() - 1.0),
                        z: r_max * (2.0 * next() - 1.0),
                    };
                    if c.norm() <= r_max {
                        break c;
                    }
                };
                if let Some(m) = u.material_at(p) {
                    assert!(
                        declared.contains(&m),
                        "{}: material_at returned index {m} at r = {:.4}, which is NOT in the \
                         declared reachable set {declared:?}. The majorant would not bound it, \
                         and an under-bound majorant is a silent bias.",
                        t.name(),
                        p.norm()
                    );
                }
            }
        }
    }

    /// The point of declaring reachability: every approximate treatment must
    /// bound a *strictly smaller* set than the explicit one, or the fix bought
    /// nothing. On a TRISO pebble the material dropped is the undiluted kernel,
    /// whose resonance peaks set the majorant for everyone else.
    #[test]
    fn approximate_treatments_bound_fewer_materials_than_delta_tracking() {
        let build = |t| {
            DhUniverse::pebble(
                PebbleParams::fhr_unit_cell().with_materials(dummy_materials(8)),
                t,
            )
            .unwrap()
        };
        let delta = build(DhTreatment::DeltaTracking);
        let n_delta = delta.reachable_material_indices().len();

        for t in DhTreatment::ALL.into_iter().filter(|t| !t.is_exact()) {
            let u = build(t);
            let idx = u.reachable_material_indices();
            assert!(
                idx.len() < n_delta,
                "{} bounds {} materials against delta tracking's {} — no saving",
                t.name(),
                idx.len(),
                n_delta
            );
            assert!(
                !idx.contains(&0),
                "{} declares the undiluted kernel (index 0) reachable, but no point in its \
                 geometry returns it — that is the inflated-majorant bug this guards",
                t.name()
            );
        }
    }

    /// **Every arm must model the same fuel inventory**, or a treatment
    /// comparison is also a comparison of two different reactors.
    ///
    /// The explicit arm packs; the approximate arms smear at
    /// `spec.packing_fraction`. Those agree only if the packing actually hits
    /// the fraction it was asked for, and before 2026-09-14 it did not — it
    /// realised 0.2894 against a requested 0.30, handing every smeared arm
    /// 3.7 % more heavy metal than the reference it was judged against.
    #[test]
    fn every_treatment_models_the_same_packing_fraction() {
        let target = TrisoSpec::FHR_HALEU_UCO.packing_fraction;
        let mut fractions = Vec::new();
        for t in DhTreatment::ALL {
            let u = DhUniverse::pebble(
                PebbleParams::fhr_unit_cell().with_materials(dummy_materials(8)),
                t,
            )
            .unwrap_or_else(|e| panic!("{} failed to build: {e}", t.name()));
            let pf = u.packing_fraction();
            assert!(
                (pf / target - 1.0).abs() <= 5.0e-3,
                "{} models pf {pf:.5} against a requested {target:.5} ({:+.2} %) — the arms \
                 are not comparable",
                t.name(),
                100.0 * (pf / target - 1.0)
            );
            fractions.push(pf);
        }
        let (lo, hi) = (
            fractions.iter().cloned().fold(f64::INFINITY, f64::min),
            fractions.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        assert!(
            (hi / lo - 1.0).abs() <= 5.0e-3,
            "arms span pf {lo:.5}..{hi:.5}, a {:+.2} % inventory difference between \
             treatments that are supposed to differ only in how geometry is resolved",
            100.0 * (hi / lo - 1.0)
        );
    }

    /// The realised fraction is exact, not sampled: every kept sphere lies
    /// wholly inside the ball, so it is `n * (r/R)^3` in closed form. Pinning
    /// that keeps the accessor honest if the clip convention ever changes.
    #[test]
    fn reported_packing_fraction_matches_the_stored_particles() {
        let u = DhUniverse::pebble(
            PebbleParams::fhr_reference().with_materials(dummy_materials(7)),
            DhTreatment::DeltaTracking,
        )
        .unwrap();
        let spec = TrisoSpec::FHR_HALEU_UCO;
        let expected = u.particle_count() as f64 * (spec.opyc / 1.9_f64).powi(3);
        assert!(
            (u.packing_fraction() - expected).abs() < 1.0e-12,
            "reported {:.6} but {} particles imply {expected:.6}",
            u.packing_fraction(),
            u.particle_count()
        );
    }

    /// **SCLS must remember geometry across history boundaries.** That is the
    /// entire difference between it and CLS.
    ///
    /// `begin_history` resets the in-progress *flight* but must keep the
    /// retained inclusions, which are a reconstructed model of the packing
    /// rather than per-history state. Until 2026-09-14 it restored a pristine
    /// medium instead, so every history began with no memory and SCLS decayed
    /// into CLS — measured, and indistinguishable from it.
    #[test]
    fn scls_retains_inclusions_across_history_boundaries() {
        let u = DhUniverse::pebble(
            PebbleParams::fhr_reference().with_materials(dummy_materials(7)),
            DhTreatment::Scls,
        )
        .expect("SCLS pebble builds");

        let retained = || match &u.geometry {
            DhGeometry::Scls { medium, .. } => medium.lock().unwrap().0.histories().len(),
            _ => unreachable!("built as SCLS"),
        };

        // Size the retention window exactly as `keff` does. Without this the
        // window is still the construction-time placeholder, and a walk longer
        // than it culls everything — which is precisely the second defect, and
        // is worth noting as the reason this line is not optional.
        let nucs = vec![Nuclide::from_core("C0").expect("embedded carbon")];
        u.size_scls_window(&nucs);

        // Walk a line through the fuel zone so the sampler meets inclusions.
        let walk = |z0: f64| {
            for i in 0..400 {
                let z = z0 + 0.002 * i as f64;
                let _ = u.material_at(Position { x: 0.0, y: 0.0, z });
            }
        };

        MaterialQuery::begin_history(&&u);
        walk(-1.0);
        let after_first = retained();
        assert!(
            after_first > 0,
            "SCLS remembered nothing at all during a history — the sampler is not \
             retaining inclusions"
        );

        // The boundary that used to wipe everything.
        MaterialQuery::begin_history(&&u);
        let after_boundary = retained();
        assert_eq!(
            after_boundary, after_first,
            "begin_history dropped {} of {after_first} retained inclusions; it must reset the \
             flight only, not the reconstructed geometry",
            after_first - after_boundary
        );

        // And a second history keeps accumulating rather than starting over.
        walk(-1.0);
        assert!(
            retained() >= after_first,
            "memory shrank across a second pass: {} < {after_first}",
            retained()
        );
    }

    /// The retention window must be a **transport** mean free path, not the
    /// geometric distance between inclusions. Seeding it from
    /// `mean_chord_matrix()` made it ~15x too small on the FHR pebble, so
    /// almost everything was culled immediately.
    ///
    /// This checks the sizing actually happens and lands on the right order of
    /// magnitude, using a matrix whose total cross section is known.
    #[test]
    fn scls_window_is_a_transport_mfp_not_a_chord_length() {
        let u = DhUniverse::pebble(
            PebbleParams::fhr_reference().with_materials(dummy_materials(7)),
            DhTreatment::Scls,
        )
        .unwrap();

        let radius = || match &u.geometry {
            DhGeometry::Scls { medium, .. } => medium.lock().unwrap().0.sphere().radius,
            _ => unreachable!(),
        };
        let before = radius();

        // dummy_materials uses 8.0e-2 atoms/b-cm of nuclide 0; from_core("C0")
        // gives a real carbon evaluation, so 1/Sigma_t is a genuine mfp.
        let nucs = vec![Nuclide::from_core("C0").expect("embedded carbon")];
        u.size_scls_window(&nucs);
        let after = radius();

        let sigma_t = u.materials[5].macro_xs_total(0.0253, &nucs);
        let uncapped = 1.0 / sigma_t + TrisoSpec::FHR_HALEU_UCO.opyc;
        // Capped at the domain: beyond it there is no geometry to retain.
        let expected = uncapped.min(2.0);
        assert!(
            (after - expected).abs() < 1.0e-9,
            "window {after:.4} cm, expected min(1/Sigma_t + r, domain) = {expected:.4} cm \
             (uncapped would be {uncapped:.4})"
        );
        // On THIS problem the cap binds, and that is the finding rather than an
        // implementation detail: a retention window larger than the region it is
        // meant to be a local view of means nothing is ever culled, so SCLS's
        // bounded-memory premise does not hold here at all.
        assert!(
            uncapped > 2.0,
            "expected the graphite transport mfp ({uncapped:.2} cm) to exceed the 2.0 cm \
             domain on this pebble -- if it no longer does, the degeneracy recorded on \
             DhTreatment::Scls needs re-checking"
        );
        assert!(
            after > 5.0 * before,
            "window barely moved ({before:.4} -> {after:.4} cm); the chord-length seed was \
             ~15x too small, so a correct sizing must be much larger"
        );
    }

    /// A coolant radius inside the pebble is a contradiction, not a clamp.
    #[test]
    fn coolant_radius_inside_the_pebble_is_rejected() {
        let bad = PebbleParams::fhr_reference()
            .with_coolant(1.5)
            .with_materials(dummy_materials(8));
        assert!(matches!(
            DhUniverse::pebble(bad, DhTreatment::DeltaTracking),
            Err(DhError::Geometry(_))
        ));
    }
}
