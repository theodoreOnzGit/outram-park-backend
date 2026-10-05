//! The model: geometry, materials and the nuclear data they are built from.
//!
//! Everything in this file runs identically natively and on `wasm32` — no
//! threads, no clock, no filesystem. The data arrive as bytes; where they came
//! from (a file on disk, or an HTTP fetch in the browser) is the caller's
//! business.
//!
//! # Provenance of every number
//!
//! - **Dimensions, densities, atom densities, particle count**: IAEA-TECDOC-1382
//!   (HTR-10 benchmark), Tables 4-2 and 4-38, as already mirrored and cited in
//!   `outram-mc-libs/examples/htr10_fuel_zone_kinf.rs` and typed in
//!   `outram-park-digital-twin-engine/src/htr10/{neutronics,design}.rs`.
//!   Nothing is fitted or tuned here.
//! - **Natural isotopic abundances**: `openmc.data.NATURAL_ABUNDANCE`
//!   (`openmc/data/data.py:30-128`), as ported in
//!   `outram-mc-libs/src/plotter/endf_tables.rs:616-634` — mirrored because that
//!   table is `pub(crate)`.
//! - **Atomic masses** for the coating atom densities: the AWR on each nuclide's
//!   own ENDF/B-VIII.0 tape, times `AMASSN_AMU` and `AMU_G` from the NJOY port's
//!   `common::phys`. So a coating's atom density is consistent with the very
//!   tape the transport uses.
//! - **Cross sections**: ENDF/B-VIII.0 tapes committed under
//!   `reference-data/endf/`, processed by this workspace's own NJOY port.
//!
//! # What "2D" means here, and what it does NOT preserve
//!
//! The geometry is invariant in `z` and bounded by reflective planes, so it is
//! exactly an infinite array of infinitely long objects: the TRISO particles are
//! **rods, not spheres**. The rod count is chosen so the particles' AREA
//! fraction of the fuelled zone equals the real pebble's VOLUME fraction
//! (5.03 %), and the cell pitch so the pebble's area fraction equals the core's
//! 0.61 ball filling fraction. That keeps the fuel-to-moderator inventory ratio
//! of the real pebble. It does **not** keep the self-shielding: an infinite rod
//! has mean chord 2r against a sphere's 4r/3, so resonance absorption in the
//! kernels is not the 3D pebble's. Treat this as a picture of how neutrons move
//! in a TRISO pebble, not a model of HTR-10.

// Headless output and data preparation are native-only; the browser build
// does not call them.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use njoy_outram_park_fork::common::phys::{AMASSN_AMU, AMU_G};
use crate::tapes::read_tape;
use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{
    BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;

// ─── HTR-10 specification (IAEA-TECDOC-1382) ─────────────────────────────────

/// UO2 kernel radius, cm (Table 4-2: radius 0.25 mm).
pub const KERNEL_R: f64 = 0.025;
/// Coating thicknesses, cm (Table 4-2: 0.09 / 0.04 / 0.035 / 0.04 mm).
pub const BUFFER_T: f64 = 0.009;
pub const IPYC_T: f64 = 0.004;
pub const SIC_T: f64 = 0.0035;
pub const OPYC_T: f64 = 0.004;
/// Coating densities, g/cm^3 (Table 4-2).
pub const BUFFER_RHO: f64 = 1.1;
pub const PYC_RHO: f64 = 1.9;
pub const SIC_RHO: f64 = 3.18;
/// Fuelled-zone and pebble radii, cm (5 cm and 6 cm diameters).
pub const FUEL_ZONE_R: f64 = 2.5;
pub const PEBBLE_R: f64 = 3.0;
/// Coated particles per pebble (TECDOC-1382 Monte Carlo modelling notes).
pub const PARTICLES_PER_PEBBLE_3D: f64 = 8335.0;
/// Volumetric filling fraction of balls in the core (`htr10/design.rs`).
pub const CORE_FILLING_FRACTION: f64 = 0.61;

/// Kernel atom densities, atoms/(b cm) (Table 4-38).
const KERNEL_U235: f64 = 3.992067e-3;
const KERNEL_U238: f64 = 1.924449e-2;
const KERNEL_O16: f64 = 4.647329e-2;
const KERNEL_B10: f64 = 1.849637e-8;
const KERNEL_B11: f64 = 7.445022e-8;
/// Matrix and shell graphite, atoms/(b cm) (Table 4-38; 1.73 g/cm^3 graphite).
/// The shell uses the matrix composition: `neutronics.rs:482` gives one
/// density for "the graphite in the matrix and outer shell".
const MATRIX_C: f64 = 8.674169e-2;
const MATRIX_B10: f64 = 2.244010e-8;
const MATRIX_B11: f64 = 9.032424e-8;

/// Natural abundances (atom fraction), `openmc.data.NATURAL_ABUNDANCE`.
const AB_C12: f64 = 0.988922;
const AB_C13: f64 = 0.011078;
const AB_SI28: f64 = 0.9222968;
const AB_SI29: f64 = 0.0468316;
const AB_SI30: f64 = 0.0308716;

// ─── Run choices (NOT specification) ─────────────────────────────────────────

/// Temperature of every material and every cross section, K.
///
/// 296 K, not the HTR-10 B1 benchmark's 293.15 K: the graphite S(alpha,beta)
/// tape tabulates 296 K as its lowest temperature, and running there avoids
/// regenerating the law in the browser. A 3 K offset is immaterial here.
pub const TEMPERATURE_K: f64 = 296.0;

/// Reconstruction tier. `VeryFast` reconstructs and Doppler-broadens at
/// tolerance 0.01 instead of NJOY's 0.001 — an APPROXIMATION, chosen for this
/// demo by the maintainer (2026-10-03) to make in-browser processing tolerable.
/// Measured natively: U-235 19.1 s vs 46.2 s, U-238 13.4 s vs 36.3 s.
pub const SPEED: SpeedTier = SpeedTier::VeryFast;

/// Half-length of the reflective slab in z, cm. The geometry is z-invariant,
/// so any value gives the same physics; it only bounds void streaming.
const Z_HALF: f64 = 50.0;
/// Clearance kept between particles, and from the fuel-zone edge, cm — so no
/// two cylinders are tangent (a tangency is a degenerate crossing).
const CLEARANCE: f64 = 1.0e-4;

// ─── Derived geometry ────────────────────────────────────────────────────────

pub fn buffer_r() -> f64 { KERNEL_R + BUFFER_T }
pub fn ipyc_r() -> f64 { buffer_r() + IPYC_T }
pub fn sic_r() -> f64 { ipyc_r() + SIC_T }
/// Outer radius of a coated particle, cm (0.0455).
pub fn particle_r() -> f64 { sic_r() + OPYC_T }

/// 2D particle count: area fraction in 2D = volume fraction in 3D.
///
/// `n3 (r/R)^3 = n2 (r/R)^2`, so `n2 = n3 r / R` = 8335 x 0.0455 / 2.5 = 151.7.
pub fn particles_2d() -> usize {
    (PARTICLES_PER_PEBBLE_3D * particle_r() / FUEL_ZONE_R).round() as usize
}

/// Half-pitch of the square cell: `pi R^2 / (2P)^2 = 0.61`.
pub fn half_pitch() -> f64 {
    PEBBLE_R * (std::f64::consts::PI / (4.0 * CORE_FILLING_FRACTION)).sqrt()
}

/// SplitMix64 — a seedable, platform-independent generator for layout and
/// births (the transport has its own RNG inside outram-mc-libs).
pub fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform in [0, 1).
pub fn uniform(state: &mut u64) -> f64 {
    (splitmix(state) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Particle centres, placed by random sequential addition in the fuelled zone.
///
/// Deterministic: a fixed seed gives a fixed layout, which the committed
/// geometry images and headless fixture depend on.
pub fn particle_centres(seed: u64) -> Vec<(f64, f64)> {
    let n = particles_2d();
    let rp = particle_r();
    let r_max = FUEL_ZONE_R - rp - CLEARANCE;
    let min_d2 = (2.0 * rp + CLEARANCE).powi(2);
    let mut s = seed;
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(n);
    let mut attempts = 0usize;
    while out.len() < n {
        attempts += 1;
        assert!(attempts < 10_000_000, "particle placement did not converge");
        // Uniform in the disc of radius r_max.
        let (r, t) = (r_max * uniform(&mut s).sqrt(), std::f64::consts::TAU * uniform(&mut s));
        let (x, y) = (r * t.cos(), r * t.sin());
        if out.iter().all(|&(a, b)| (a - x).powi(2) + (b - y).powi(2) >= min_d2) {
            out.push((x, y));
        }
    }
    out
}

/// Layout seed for the particle placement.
pub const LAYOUT_SEED: u64 = 20_261_003;

// ─── Material indices ────────────────────────────────────────────────────────

pub const MAT_KERNEL: usize = 0;
pub const MAT_BUFFER: usize = 1;
pub const MAT_IPYC: usize = 2;
pub const MAT_SIC: usize = 3;
pub const MAT_OPYC: usize = 4;
pub const MAT_MATRIX: usize = 5;
pub const MAT_SHELL: usize = 6;
pub const MATERIAL_NAMES: [&str; 7] = [
    "UO2 kernel", "Porous C buffer", "Inner PyC", "SiC", "Outer PyC",
    "Matrix graphite", "Shell graphite",
];

// ─── Geometry ────────────────────────────────────────────────────────────────

fn hs(surface_idx: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx, sense }
}

/// `a AND b AND ...` in this crate's postfix region notation.
fn all_of(tokens: &[RegionToken]) -> Vec<RegionToken> {
    let mut out = Vec::with_capacity(tokens.len() * 2);
    for (i, t) in tokens.iter().enumerate() {
        out.push(t.clone());
        if i > 0 {
            out.push(RegionToken::Intersection);
        }
    }
    out
}

/// Surface indices fixed by [`build_geometry`].
pub const S_X_LO: usize = 0;
pub const S_X_HI: usize = 1;
pub const S_Y_LO: usize = 2;
pub const S_Y_HI: usize = 3;
pub const S_Z_LO: usize = 4;
pub const S_Z_HI: usize = 5;
pub const S_PEBBLE: usize = 6;
pub const S_FUEL_ZONE: usize = 7;
/// First of five concentric cylinders for particle `i`.
pub fn s_particle(i: usize) -> usize { 8 + 5 * i }

/// Build the pebble cell. The helium coolant is modelled as void (its density
/// is negligible next to graphite) and is said to be void in the UI.
pub fn build_geometry(centres: &[(f64, f64)]) -> Geometry {
    use HalfSpaceSense::{Inside, Outside};
    let p = half_pitch();
    let refl = BoundaryType::Reflective;
    let mut surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -p, bc: refl }),
        SurfaceKind::XPlane(XPlane { x0: p, bc: refl }),
        SurfaceKind::YPlane(YPlane { y0: -p, bc: refl }),
        SurfaceKind::YPlane(YPlane { y0: p, bc: refl }),
        SurfaceKind::ZPlane(ZPlane { z0: -Z_HALF, bc: refl }),
        SurfaceKind::ZPlane(ZPlane { z0: Z_HALF, bc: refl }),
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: PEBBLE_R, bc: BoundaryType::Transmissive }),
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: FUEL_ZONE_R, bc: BoundaryType::Transmissive }),
    ];
    let radii = [KERNEL_R, buffer_r(), ipyc_r(), sic_r(), particle_r()];
    for &(x0, y0) in centres {
        for r in radii {
            surfaces.push(SurfaceKind::ZCylinder(ZCylinder { x0, y0, r, bc: BoundaryType::Transmissive }));
        }
    }

    let slab = [hs(S_Z_LO, Outside), hs(S_Z_HI, Inside)];
    let t = TEMPERATURE_K;
    let mut cells = Vec::new();
    let mut id = 1;
    let mut push = |cells: &mut Vec<Cell>, region: Vec<RegionToken>, mat: Option<usize>| {
        cells.push(match mat {
            Some(m) => Cell::material(id, region, m, t),
            None => Cell::fill(id, region, CellFill::Void, Position::ZERO),
        });
        id += 1;
    };

    // Ordered by how often a neutron is there: graphite first.
    let mut matrix = vec![slab[0].clone(), slab[1].clone(), hs(S_FUEL_ZONE, Inside)];
    for i in 0..centres.len() {
        matrix.push(hs(s_particle(i) + 4, Outside));
    }
    push(&mut cells, all_of(&matrix), Some(MAT_MATRIX));
    push(&mut cells, all_of(&[slab[0].clone(), slab[1].clone(), hs(S_PEBBLE, Inside), hs(S_FUEL_ZONE, Outside)]), Some(MAT_SHELL));
    push(
        &mut cells,
        all_of(&[
            slab[0].clone(), slab[1].clone(),
            hs(S_X_LO, Outside), hs(S_X_HI, Inside), hs(S_Y_LO, Outside), hs(S_Y_HI, Inside),
            hs(S_PEBBLE, Outside),
        ]),
        None, // helium: void
    );
    let layer_mats = [MAT_KERNEL, MAT_BUFFER, MAT_IPYC, MAT_SIC, MAT_OPYC];
    for i in 0..centres.len() {
        let s0 = s_particle(i);
        for (k, &m) in layer_mats.iter().enumerate() {
            let mut reg = vec![slab[0].clone(), slab[1].clone(), hs(s0 + k, Inside)];
            if k > 0 {
                reg.push(hs(s0 + k - 1, Outside));
            }
            push(&mut cells, all_of(&reg), Some(m));
        }
    }

    let n_cells = cells.len();
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe { id: 0, cell_indices: (0..n_cells).collect() }],
        lattices: vec![],
        root_universe: 0,
    }
}

// ─── Nuclear data ────────────────────────────────────────────────────────────

/// One unit of data processing — the granularity of the loading progress bar.
#[derive(Debug, Clone, Copy)]
pub struct Job {
    pub label: &'static str,
    pub tape: &'static str,
    pub kind: JobKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Nuclide,
    GraphiteSab,
}

/// Every tape this demo reads, in processing order. All ENDF/B-VIII.0
/// (`reference-data/endf/README.md` provenance table; the un-suffixed U-238
/// file is ENDF/B-VIII.0, MAT 9237).
pub const JOBS: [Job; 11] = [
    Job { label: "U-235", tape: "n-092_U_235-ENDF8.0.endf", kind: JobKind::Nuclide },
    Job { label: "U-238", tape: "n-092_U_238.endf", kind: JobKind::Nuclide },
    Job { label: "O-16", tape: "n-008_O_016-ENDF8.0.endf", kind: JobKind::Nuclide },
    Job { label: "B-10", tape: "n-005_B_010-ENDF8.0.endf", kind: JobKind::Nuclide },
    Job { label: "B-11", tape: "n-005_B_011-ENDF8.0.endf", kind: JobKind::Nuclide },
    Job { label: "C-12", tape: "n-006_C_012-ENDF8.0.endf", kind: JobKind::Nuclide },
    Job { label: "C-13", tape: "n-006_C_013-ENDF8.0.endf", kind: JobKind::Nuclide },
    Job { label: "Si-28", tape: "n-014_Si_028-ENDF8.0.endf", kind: JobKind::Nuclide },
    Job { label: "Si-29", tape: "n-014_Si_029-ENDF8.0.endf", kind: JobKind::Nuclide },
    Job { label: "Si-30", tape: "n-014_Si_030-ENDF8.0.endf", kind: JobKind::Nuclide },
    Job { label: "graphite S(a,b)", tape: "tsl-crystalline-graphite.endf", kind: JobKind::GraphiteSab },
];

/// Nuclide indices in the assembled list.
pub const N_U235: usize = 0;
pub const N_U238: usize = 1;
const N_O16: usize = 2;
const N_B10: usize = 3;
const N_B11: usize = 4;
/// Carbon bound in graphite — carries the crystalline-graphite S(alpha,beta).
pub const N_C12_GR: usize = 5;
const N_C13_GR: usize = 6;
/// Carbon in SiC — free gas (see [`NuclearData::assemble`]).
const N_C12_FREE: usize = 7;
const N_C13_FREE: usize = 8;
const N_SI28: usize = 9;
const N_SI29: usize = 10;
const N_SI30: usize = 11;

pub fn nuclide_from_bytes(bytes: &[u8], name: &str) -> Result<Nuclide, String> {
    let (tape, mat) = read_tape(bytes, name)?;
    Nuclide::from_tape_with_speed(&tape, mat, name, TEMPERATURE_K, SPEED).map_err(|e| format!("{name}: {e}"))
}

pub fn graphite_sab_from_bytes(bytes: &[u8]) -> Result<ThermalScattering, String> {
    let (tape, mat) = read_tape(bytes, "graphite S(a,b)")?;
    ThermalScattering::from_tape(&tape, mat, TEMPERATURE_K, "c_Graphite").map_err(|e| format!("graphite S(a,b): {e}"))
}

/// Incremental processing, one [`Job`] per call, so a UI can redraw between
/// the (long, blocking) jobs.
#[derive(Default)]
pub struct DataBuilder {
    done: usize,
    nuclides: Vec<Nuclide>,
    sab: Option<ThermalScattering>,
}

impl DataBuilder {
    pub fn next_job(&self) -> Option<Job> {
        JOBS.get(self.done).copied()
    }
    /// Process the next job from its (covariance-stripped) tape bytes.
    pub fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        let job = self.next_job().ok_or("no job left")?;
        match job.kind {
            JobKind::Nuclide => self.nuclides.push(nuclide_from_bytes(bytes, job.label)?),
            JobKind::GraphiteSab => self.sab = Some(graphite_sab_from_bytes(bytes)?),
        }
        self.done += 1;
        Ok(())
    }
    pub fn finish(self) -> Result<NuclearData, String> {
        if self.done != JOBS.len() {
            return Err(format!("{} of {} jobs done", self.done, JOBS.len()));
        }
        NuclearData::assemble(self.nuclides, self.sab.ok_or("no S(a,b)")?)
    }
}

pub struct NuclearData {
    pub nuclides: Vec<Nuclide>,
    pub materials: Vec<Material>,
}

/// Atoms per (b cm) of an element at mass density `rho` (g/cm^3), from the
/// AWRs of its isotopes on their own tapes.
fn element_atom_density(rho: f64, isotopes: &[(&Nuclide, f64)]) -> f64 {
    let mass_amu: f64 = isotopes.iter().map(|(n, ab)| ab * n.awr * AMASSN_AMU).sum();
    rho / (mass_amu * AMU_G) * 1.0e-24
}

impl NuclearData {
    /// Build the nuclide list and the seven materials.
    ///
    /// `loaded` is U-235, U-238, O-16, B-10, B-11, C-12, C-13, Si-28, Si-29,
    /// Si-30 in [`JOBS`] order. The carbon nuclides are cloned so graphite
    /// carbon can carry the crystalline-graphite S(alpha,beta) while SiC carbon
    /// stays free gas: there is a C-in-SiC law on disk, but the SiC layer is
    /// 35 um thick and adding two more tapes to a browser load is not worth it
    /// for a demo. That is an approximation and is listed as one.
    pub fn assemble(loaded: Vec<Nuclide>, graphite: ThermalScattering) -> Result<Self, String> {
        if loaded.len() != 10 {
            return Err(format!("expected 10 nuclides, got {}", loaded.len()));
        }
        let mut it = loaded.into_iter();
        let mut take = || it.next().expect("length checked");
        let (u235, u238, o16, b10, b11, c12, c13, si28, si29, si30) =
            (take(), take(), take(), take(), take(), take(), take(), take(), take(), take());
        let c12_gr = c12.clone().with_thermal_scattering(graphite.clone());
        let c13_gr = c13.clone().with_thermal_scattering(graphite);
        let nuclides = vec![u235, u238, o16, b10, b11, c12_gr, c13_gr, c12, c13, si28, si29, si30];

        let carbon = [(&nuclides[N_C12_FREE], AB_C12), (&nuclides[N_C13_FREE], AB_C13)];
        let silicon = [(&nuclides[N_SI28], AB_SI28), (&nuclides[N_SI29], AB_SI29), (&nuclides[N_SI30], AB_SI30)];
        let n_buffer = element_atom_density(BUFFER_RHO, &carbon);
        let n_pyc = element_atom_density(PYC_RHO, &carbon);
        // SiC molecules per (b cm): rho / (m_Si + m_C).
        let m_si: f64 = silicon.iter().map(|(n, ab)| ab * n.awr * AMASSN_AMU).sum();
        let m_c: f64 = carbon.iter().map(|(n, ab)| ab * n.awr * AMASSN_AMU).sum();
        let n_sic = SIC_RHO / ((m_si + m_c) * AMU_G) * 1.0e-24;

        let c = |i: usize, d: f64| NuclideComponent { nuclide_idx: i, atom_density: d };
        let graphite = |n_c: f64| vec![c(N_C12_GR, n_c * AB_C12), c(N_C13_GR, n_c * AB_C13)];
        let mat = |id: usize, comps: Vec<NuclideComponent>| Material {
            id: id as i32 + 1,
            name: MATERIAL_NAMES[id].into(),
            components: comps,
            temperature: TEMPERATURE_K,
        };
        let mut matrix = graphite(MATRIX_C);
        matrix.extend([c(N_B10, MATRIX_B10), c(N_B11, MATRIX_B11)]);
        let materials = vec![
            mat(MAT_KERNEL, vec![
                c(N_U235, KERNEL_U235), c(N_U238, KERNEL_U238), c(N_O16, KERNEL_O16),
                c(N_B10, KERNEL_B10), c(N_B11, KERNEL_B11),
            ]),
            mat(MAT_BUFFER, graphite(n_buffer)),
            mat(MAT_IPYC, graphite(n_pyc)),
            mat(MAT_SIC, vec![
                c(N_SI28, n_sic * AB_SI28), c(N_SI29, n_sic * AB_SI29), c(N_SI30, n_sic * AB_SI30),
                c(N_C12_FREE, n_sic * AB_C12), c(N_C13_FREE, n_sic * AB_C13),
            ]),
            mat(MAT_OPYC, graphite(n_pyc)),
            mat(MAT_MATRIX, matrix.clone()),
            mat(MAT_SHELL, matrix),
        ];
        Ok(Self { nuclides, materials })
    }
}
