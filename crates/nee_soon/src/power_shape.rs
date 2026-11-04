// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! # `power_shape` — where the fission power goes, from Monte Carlo to a TH mesh
//!
//! gh:#1184 (epic gh:#1183). A closed set of **power shapes** (analytic,
//! tabulated or reconstructed from a Monte Carlo tally), and the one operation
//! every coupling needs: **put a total power onto a set of thermal-hydraulic
//! cells so that the cells' powers add up to that total, to round-off.**
//!
//! ## The shortest thing that works
//!
//! ```
//! use nee_soon::power_shape::{CellGeometry, Frame, FrameMap, PowerCells, PowerShape};
//! use uom::si::f64::{Length, Power};
//! use uom::si::length::centimeter;
//! use uom::si::power::megawatt;
//! use uom::si::volumetric_power_density::watt_per_cubic_meter;
//!
//! let cm = Length::new::<centimeter>;
//! // A chopped cosine over a 180 cm bed (MC frame: z = 0 at bed mid-height).
//! let shape = PowerShape::ChoppedCosine { centre: cm(0.0), extrapolated_height: cm(220.0) };
//! // Two R-Z ring cells, in the Monte Carlo frame.
//! let cells = PowerCells::new(
//!     Frame::Mc,
//!     FrameMap::new(cm(90.0)),
//!     vec![
//!         CellGeometry::Annulus { r: [cm(0.0), cm(90.0)], z: [cm(-90.0), cm(0.0)] },
//!         CellGeometry::Annulus { r: [cm(0.0), cm(90.0)], z: [cm(0.0), cm(90.0)] },
//!     ],
//! );
//! let q = shape.cell_power_density(&cells, Power::new::<megawatt>(10.0)).unwrap();
//! // Symmetric shape, symmetric cells: equal densities, 5 MW each.
//! assert!((q[0].get::<watt_per_cubic_meter>() / q[1].get::<watt_per_cubic_meter>() - 1.0).abs() < 1e-12);
//! ```
//!
//! ## Module map
//!
//! | item | what |
//! |---|---|
//! | [`PowerShape`] | the closed set of shapes: uniform, region lumps, axial cosine/sine, `J0 x cos`, Legendre-z, Zernike(-Legendre), spatial spherical harmonics, an R-Z table |
//! | [`PowerShape::cell_power_density`] | total power -> W/m^3 per cell, conserving the total to round-off |
//! | [`PowerShape::from_tally`], [`PowerShape::zernike_legendre_from_tallies`] | the shape a Monte Carlo `KappaFission` tally measured, with the expansion normalisations |
//! | [`McPowerTally`] | builders for those tallies (`rz_mesh`, `legendre_z`, `zernike`) |
//! | [`zernike_to_bessel`], [`BesselRadial`], [`BesselBoundary`] | a Zernike (`m = 0`) shape re-expressed in Fourier-Bessel `J0` modes (Neumann/Dini by default, Dirichlet on request); [`PowerShape::bessel_legendre`] makes the R-Z product |
//! | [`Frame`], [`FrameMap`] | the three coordinate frames of the HTR-10 chain: MC (cm, z up, 0 at bed mid-height), TECDOC-1694 (cm, z down, 0 at the full-core bed top) and GeN-Foam meshes (metres, TECDOC orientation) |
//! | [`CellGeometry`], [`PowerCells`] | the cells power is put on: exact rings (R-Z meshes), boxes, or centroid points (a general `FvMesh`) |
//! | [`PowerShape::write_rz_png`] | an R-Z picture of a shape, for a human to check |
//!
//! ## How the cell integrals are done (and how exact they are)
//!
//! A cell receives `total * I_c / sum_k I_k / V_c`, with `I_c` the integral of
//! the shape over the cell. **The total is therefore conserved by
//! construction, to round-off, whatever the shape or the mesh.** The cell
//! integrals themselves are:
//!
//! - **rings** ([`CellGeometry::Annulus`], what an R-Z mesh has): composite
//!   Gauss-Legendre in `r` (with the `r dr` weight) and `z`, split at every
//!   discontinuity the shape has in `r` and `z` (table and lump edges, the
//!   support of a chopped cosine or of an expansion), so piecewise-constant and
//!   polynomial shapes are integrated **exactly**; smooth ones (`J0 x cos`) are
//!   refined by doubling until two estimates agree to `1e-12` relative (at
//!   most 16 subdivisions per piece).
//!   An axisymmetric shape is integrated in azimuth exactly (`2 pi f`); a
//!   non-axisymmetric one (Zernike `m != 0`, spherical harmonics) by the
//!   periodic trapezoid rule, refined with the rest;
//! - **boxes**: the same composite Gauss in `x, y, z`, refined by doubling to
//!   `1e-10` or 4 subdivisions per axis (a cylindrical discontinuity inside a
//!   box converges only at first order);
//! - **points** (centroid and volume of a general cell): the midpoint rule.
//!   Not exact; use it only where the cell geometry is not known.
//!
//! **Power outside every cell is not lost silently**:
//! [`PowerShape::captured_fraction`] reports what share of a shape's integral
//! over a support the cells hold, for shapes whose integral is known.
//!
//! ## What does not belong here
//!
//! No transport and no thermal-hydraulics: this module turns a tally into a
//! shape and a shape into cell power densities. The Picard loops that use it
//! are [`crate::htr10_coupling`] (Monte Carlo against the TH) and
//! [`crate::direct_coupling`].
//!
//! Status: untrusted AI-generated draft (2026-11-03), research, education and
//! V&V only (`RESPONSIBLE_USE.md`).

use std::f64::consts::PI;

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solid_conduction::RzMesh;
use outram_foam_basic_lib::prelude::FvMesh;
use outram_mc_libs::geometry::plot::{annotate_image, ImageData, LegendEntry, Rgb};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::tally::filter::{
    legendre_pn, real_spherical_harmonics, zernike_zn, FilterKind, LegendreAxis, MeshFilter,
    SpatialLegendreFilter, ZernikeFilter,
};
use outram_mc_libs::tally::mesh::{CylindricalMesh, MeshKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};
use petir::integration::gauss_legendre_tables::gauss_legendre_table;
use petir::mathf::RealMath;
use petir::specfunc::bessel::{bessel_j0, bessel_j1};
use uom::si::f64::{Length, Power, Volume, VolumetricPowerDensity};
use uom::si::length::{centimeter, meter};
use uom::si::power::watt;
use uom::si::volume::cubic_centimeter;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

/// Power per unit volume, W/m^3 (`uom`'s `VolumetricPowerDensity`): what a
/// thermal-hydraulic cell receives (GeN-Foam's `powerDensityNeutronics`).
pub type PowerDensity = VolumetricPowerDensity;

/// The first zero of `J0`, `2.404 825 557 695 773` (Abramowitz & Stegun
/// Tab 9.5): the radial buckling root of a bare cylinder.
pub const J0_FIRST_ZERO: f64 = 2.404_825_557_695_773;

/// TECDOC-1694 `z` of the bed floor (the top of the discharge cone), cm:
/// the full-core bed occupies `0 <= z <= 180` cm, `z` downward
/// (`outram_foam_appbuilder_lib::htr10_rz::geometry`, the `BED` rectangle
/// `0..180`). This is fixed hardware: the Monte Carlo model's bed floor is at
/// the same place for every loading, and its bed top moves with the loading.
/// Cross-check: the Monte Carlo core cavity is 221.818 cm above the floor
/// (`htr10_rmc::core_model::HTR10_CORE_CAVITY_CM`), TECDOC-1694 puts the top of
/// the core-entry plenum at `z = -40`, i.e. 220 cm above it (-0.8 %).
pub const TECDOC_BED_FLOOR_Z_CM: f64 = 180.0;

/// Why a shape could not be built or put on cells.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PowerShapeError {
    /// The shape integrates to zero (or less) over the cells, so there is no
    /// power distribution to normalise.
    #[error("the shape integrates to {0} over the cells: nothing to normalise")]
    NoPowerInCells(f64),
    /// A tally this module cannot read as a power shape.
    #[error("tally not readable as a power shape: {0}")]
    UnsupportedTally(String),
    /// An argument out of range.
    #[error("power shape input: {0}")]
    Input(String),
}

// ─────────────────────────────────────────────────────────────────────────
// Frames
// ─────────────────────────────────────────────────────────────────────────

/// A coordinate frame of the HTR-10 chain. Every frame shares the core axis
/// (`x = y = 0`, `r` from the axis); they differ in `z`.
///
/// - **`Mc`**: the Monte Carlo model (`htr10_rmc`): `z` **up**, `z = 0` at the
///   bed mid-height of the loading built (so it moves with the loading);
/// - **`Tecdoc`**: IAEA-TECDOC-1694's R-Z frame (`htr10_rz`): `z` **down**,
///   `z = 0` at the full-core bed top, fixed hardware.
///
/// A GeN-Foam mesh built by this crate (an `htr10_rz` `RzMesh`) is in the
/// `Tecdoc` frame in metres: see [`FrameMap::genfoam_m_to_mc_cm`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// Monte Carlo: `z` up, `0` at the loading's bed mid-height.
    Mc,
    /// TECDOC-1694: `z` down, `0` at the full-core bed top.
    Tecdoc,
}

/// The map between the frames for one Monte Carlo loading.
///
/// The bed floor is common hardware ([`TECDOC_BED_FLOOR_Z_CM`]); the Monte
/// Carlo bed spans `-h <= z <= h` with `h` its half-height, so
/// `z_tecdoc = 180 cm - (z_mc + h)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameMap {
    /// Half-height of the Monte Carlo bed's cylindrical part (cm in the model;
    /// `AssembledCore::bed_half_height`). Range: positive, at most 90 cm (the
    /// full core).
    pub bed_half_height: Length,
}

impl FrameMap {
    /// The map for a bed of half-height `bed_half_height`.
    #[must_use]
    pub fn new(bed_half_height: Length) -> Self {
        Self { bed_half_height }
    }

    /// The map for an assembled Monte Carlo core.
    #[must_use]
    pub fn from_core(core: &crate::htr10_rmc::core_model::AssembledCore) -> Self {
        Self::new(Length::new::<centimeter>(core.bed_half_height))
    }

    fn h_cm(&self) -> f64 {
        self.bed_half_height.get::<centimeter>()
    }

    /// `z` in the Monte Carlo frame of a `z` given in `frame`.
    #[must_use]
    pub fn z_to_mc(&self, frame: Frame, z: Length) -> Length {
        match frame {
            Frame::Mc => z,
            Frame::Tecdoc => Length::new::<centimeter>(
                TECDOC_BED_FLOOR_Z_CM - self.h_cm() - z.get::<centimeter>(),
            ),
        }
    }

    /// `z` in `frame` of a Monte Carlo `z`.
    #[must_use]
    pub fn z_from_mc(&self, frame: Frame, z_mc: Length) -> Length {
        // The map is its own inverse (a reflection about a fixed plane).
        self.z_to_mc(frame, z_mc)
    }

    /// A point given in `frame`, in the Monte Carlo frame.
    #[must_use]
    pub fn to_mc(&self, frame: Frame, p: [Length; 3]) -> [Length; 3] {
        [p[0], p[1], self.z_to_mc(frame, p[2])]
    }

    /// A Monte Carlo point, in `frame`.
    #[must_use]
    pub fn from_mc(&self, frame: Frame, p: [Length; 3]) -> [Length; 3] {
        [p[0], p[1], self.z_from_mc(frame, p[2])]
    }

    /// A GeN-Foam mesh point (metres, TECDOC orientation: `z` down, `0` at
    /// the full-core bed top, as an `htr10_rz` `RzMesh`'s cell centres) in
    /// the Monte Carlo frame, cm.
    #[must_use]
    pub fn genfoam_m_to_mc_cm(&self, p: [f64; 3]) -> [f64; 3] {
        [
            p[0] * 100.0,
            p[1] * 100.0,
            TECDOC_BED_FLOOR_Z_CM - self.h_cm() - p[2] * 100.0,
        ]
    }

    /// A Monte Carlo point (cm) as GeN-Foam mesh coordinates (metres, TECDOC
    /// orientation).
    #[must_use]
    pub fn mc_cm_to_genfoam_m(&self, p: [f64; 3]) -> [f64; 3] {
        [
            p[0] / 100.0,
            p[1] / 100.0,
            (TECDOC_BED_FLOOR_Z_CM - self.h_cm() - p[2]) / 100.0,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Cells
// ─────────────────────────────────────────────────────────────────────────

/// One cell that receives power.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellGeometry {
    /// A full ring about the core axis: `r[0] <= r <= r[1]`, `z[0] <= z <=
    /// z[1]` (`z` in the cells' frame; either order). Integrated exactly for
    /// piecewise-polynomial shapes.
    Annulus {
        /// Inner and outer radius (`0 <= r[0] < r[1]`).
        r: [Length; 2],
        /// The two axial faces.
        z: [Length; 2],
    },
    /// An axis-aligned box.
    Box {
        /// Lower corner.
        min: [Length; 3],
        /// Upper corner.
        max: [Length; 3],
    },
    /// A cell known only by its centroid and volume (a general polyhedral
    /// `FvMesh` cell): the midpoint rule, not exact.
    Point {
        /// Centroid.
        centre: [Length; 3],
        /// Volume (positive).
        volume: Volume,
    },
}

impl CellGeometry {
    /// The cell's volume.
    #[must_use]
    pub fn volume(&self) -> Volume {
        let v_cm3 = match *self {
            Self::Annulus { r, z } => {
                let (a, b) = (r[0].get::<centimeter>(), r[1].get::<centimeter>());
                PI * (b * b - a * a) * (z[1] - z[0]).get::<centimeter>().abs()
            }
            Self::Box { min, max } => (0..3)
                .map(|k| (max[k] - min[k]).get::<centimeter>().abs())
                .product(),
            Self::Point { volume, .. } => volume.get::<cubic_centimeter>(),
        };
        Volume::new::<cubic_centimeter>(v_cm3)
    }
}

/// The cells a power is put on, in one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerCells {
    /// The frame the cells' coordinates are in.
    pub frame: Frame,
    /// The map to the Monte Carlo frame (where shapes live).
    pub map: FrameMap,
    /// The cells.
    pub cells: Vec<CellGeometry>,
    /// The mesh cell index of each entry of [`Self::cells`] (its position in
    /// the mesh it came from; `0..n` when built by hand).
    pub mesh_cell: Vec<usize>,
}

impl PowerCells {
    /// Cells built by hand; `mesh_cell` is `0..cells.len()`.
    #[must_use]
    pub fn new(frame: Frame, map: FrameMap, cells: Vec<CellGeometry>) -> Self {
        let mesh_cell = (0..cells.len()).collect();
        Self {
            frame,
            map,
            cells,
            mesh_cell,
        }
    }

    /// The rings of an `htr10_rz` [`RzMesh`] (TECDOC frame), keeping only
    /// the cells `keep(c)` accepts (e.g. the heated bed cells). Exact ring
    /// geometry from the mesh's own edges.
    #[must_use]
    pub fn from_rz_mesh(mesh: &RzMesh, map: FrameMap, keep: impl Fn(usize) -> bool) -> Self {
        let (re, ze) = (mesh.r_edges(), mesh.z_edges());
        let nr = re.n_cells();
        let mut cells = Vec::new();
        let mut ids = Vec::new();
        for c in 0..mesh.n_cells() {
            if !keep(c) {
                continue;
            }
            let (i, j) = (c % nr, c / nr);
            cells.push(CellGeometry::Annulus {
                r: [re.edge(i), re.edge(i + 1)],
                z: [ze.edge(j), ze.edge(j + 1)],
            });
            ids.push(c);
        }
        Self {
            frame: Frame::Tecdoc,
            map,
            cells,
            mesh_cell: ids,
        }
    }

    /// The cells of a general GeN-Foam [`FvMesh`] (coordinates in metres, in
    /// `frame`'s orientation), as centroid points: the midpoint rule.
    #[must_use]
    pub fn from_fv_mesh(mesh: &FvMesh, frame: Frame, map: FrameMap) -> Self {
        let m = Length::new::<meter>;
        let cells = mesh
            .cell_centres
            .iter()
            .zip(mesh.cell_volumes.iter())
            .map(|(p, v)| CellGeometry::Point {
                centre: [m(p.x), m(p.y), m(p.z)],
                volume: Volume::new::<uom::si::volume::cubic_meter>(*v),
            })
            .collect();
        Self::new(frame, map, cells)
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Shapes
// ─────────────────────────────────────────────────────────────────────────

/// One region lump of [`PowerShape::Zones`]: a ring `r[0]..r[1]`,
/// `z[0]..z[1]` (Monte Carlo frame) holding `fraction` of the power,
/// uniformly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneLump {
    /// Inner and outer radius.
    pub r: [Length; 2],
    /// Bottom and top, Monte Carlo frame.
    pub z: [Length; 2],
    /// Share of the power, `>= 0` (shares are renormalised over the cells).
    pub fraction: f64,
}

/// The radial profile of [`PowerShape::SphericalHarmonics`], as a function of
/// `rho = |p - centre| / radius` in `[0, 1]`.
#[derive(Debug, Clone, PartialEq)]
pub enum RadialProfile {
    /// Constant in `rho`.
    Uniform,
    /// `sum_k a_k rho^k`.
    Polynomial(Vec<f64>),
    /// `sin(pi rho) / (pi rho)`: the bare sphere's fundamental mode.
    BareSphere,
}

impl RadialProfile {
    fn eval(&self, rho: f64) -> f64 {
        match self {
            Self::Uniform => 1.0,
            Self::Polynomial(a) => a.iter().rev().fold(0.0, |acc, c| acc * rho + c),
            Self::BareSphere => {
                if rho < 1e-12 {
                    1.0
                } else {
                    (PI * rho).r_sin() / (PI * rho)
                }
            }
        }
    }
}

/// **Where the power goes**: a closed set of shapes, all in the Monte Carlo
/// frame ([`Frame::Mc`]: cm, `z` up, `0` at the bed mid-height).
///
/// A shape is a *relative* power density: only its variation matters, because
/// [`Self::cell_power_density`] normalises it to the total power over the
/// cells it is given. Where a shape has a support (a chopped cosine, an
/// expansion interval, a sphere), it is zero outside it.
#[derive(Debug, Clone, PartialEq)]
pub enum PowerShape {
    /// Uniform over whatever cells are given.
    Uniform,
    /// Region lumps: each [`ZoneLump`] holds its share uniformly (the
    /// per-vessel / per-region lumped description).
    Zones(Vec<ZoneLump>),
    /// `cos(pi (z - centre) / H_e)` on `|z - centre| < H_e / 2`, radially
    /// uniform: the chopped cosine of a reflected core.
    ChoppedCosine {
        /// Axial centre of the cosine.
        centre: Length,
        /// Extrapolated height `H_e` (positive).
        extrapolated_height: Length,
    },
    /// `sin(pi (z - bottom) / height)` on `bottom < z < bottom + height`,
    /// radially uniform: the bare slab's fundamental mode.
    Sine {
        /// Where the sine starts.
        bottom: Length,
        /// Its length (positive).
        height: Length,
    },
    /// `J0(2.405 r / R_e) cos(pi (z - centre) / H_e)` inside `r < R_e`,
    /// `|z - centre| < H_e / 2`: the bare finite cylinder's fundamental mode.
    J0Cosine {
        /// Axial centre.
        centre: Length,
        /// Extrapolated radius `R_e` (positive).
        extrapolated_radius: Length,
        /// Extrapolated height `H_e` (positive).
        extrapolated_height: Length,
    },
    /// `sum_n a_n P_n(xi)`, `xi = 2 (z - z_min)/(z_max - z_min) - 1`, on
    /// `z_min <= z <= z_max`, radially uniform. `coefficients[n]` is `a_n` in
    /// power per unit length (any unit; relative).
    LegendreZ {
        /// Lower end of the expansion interval.
        z_min: Length,
        /// Upper end (must exceed `z_min`).
        z_max: Length,
        /// `a_0 ... a_N`.
        coefficients: Vec<f64>,
    },
    /// `sum_j c_j Z_j(rho, theta)` on the disc `rho = |(x, y) - centre| /
    /// radius <= 1`, axially uniform, in Monte Carlo filter order (`n =
    /// 0..`, `m = -n, -n+2, .., n`; [`zernike_zn`]).
    Zernike {
        /// Disc centre `(x, y)`.
        centre: [Length; 2],
        /// Disc radius (positive).
        radius: Length,
        /// `c_j` per unit area (relative).
        coefficients: Vec<f64>,
    },
    /// `sum_n sum_j c[n][j] P_n(xi(z)) Z_j(rho, theta)`: Legendre in `z`
    /// times Zernike in `(r, theta)`, on the cylinder.
    ZernikeLegendre {
        /// Disc centre `(x, y)`.
        centre: [Length; 2],
        /// Disc radius.
        radius: Length,
        /// Lower end of the axial interval.
        z_min: Length,
        /// Upper end.
        z_max: Length,
        /// `c[n][j]`.
        coefficients: Vec<Vec<f64>>,
    },
    /// **Fourier-Bessel in `r` times Legendre in `z`**:
    /// `sum_n sum_k c[n][k] P_n(xi(z)) J0(alpha_k r / radius)` on `r <=
    /// radius`, `z_min <= z <= z_max`, axisymmetric about the core axis.
    /// The radial modes come from [`zernike_to_bessel`] (a Zernike tally
    /// re-expressed in Bessel modes); [`Self::bessel_legendre`] builds the
    /// product with a Legendre-z axial shape.
    BesselLegendre {
        /// The Bessel radius `R_b` (support and argument scale).
        radius: Length,
        /// `alpha_k` (zeros of `J0` for Dirichlet, `0` and the zeros of `J1`
        /// for Neumann).
        alphas: Vec<f64>,
        /// Lower end of the axial interval.
        z_min: Length,
        /// Upper end.
        z_max: Length,
        /// `c[n][k]`.
        coefficients: Vec<Vec<f64>>,
    },
    /// **Spatial** real spherical harmonics on a sphere:
    /// `R(rho) sum_{l,m} c_lm Y_l^m(theta, phi)` for `rho = |p - centre| /
    /// radius <= 1`, `theta` the polar angle from `+z`. `coefficients` in
    /// `(l, m)` order, `m = -l..=l` ([`real_spherical_harmonics`]'s order,
    /// orthonormal over the unit sphere).
    SphericalHarmonics {
        /// Sphere centre.
        centre: [Length; 3],
        /// Sphere radius (positive).
        radius: Length,
        /// `c_lm`, `(order + 1)^2` of them.
        coefficients: Vec<f64>,
        /// The radial factor.
        radial: RadialProfile,
    },
    /// A piecewise-constant R-Z table: `values[i + n_r k]` on `r_edges[i]..
    /// r_edges[i+1]`, `z_edges[k]..z_edges[k+1]` (Monte Carlo frame, both
    /// ascending), zero outside.
    RzTable {
        /// Ascending radial edges.
        r_edges: Vec<Length>,
        /// Ascending axial edges.
        z_edges: Vec<Length>,
        /// Relative density per cell, `r` fastest.
        values: Vec<f64>,
    },
}

/// Highest Zernike radial order with `n_terms` moments (`(N+1)(N+2)/2`).
fn zernike_order(n_terms: usize) -> Option<usize> {
    (0..64).find(|n| (n + 1) * (n + 2) / 2 == n_terms)
}

/// Highest spherical-harmonic degree with `n_terms` moments (`(L+1)^2`).
fn sh_order(n_terms: usize) -> Option<usize> {
    (0..64).find(|l| (l + 1) * (l + 1) == n_terms)
}

/// Orthogonality norm of [`zernike_zn`]'s moment `j` over the unit disc:
/// `int Z_j^2 dA = pi/(n+1)` for `m = 0`, `pi/(2(n+1))` otherwise (the
/// filter's polynomials are unnormalised, `R_n^m(1) = 1`).
fn zernike_norms(order: usize) -> Vec<f64> {
    let mut out = Vec::new();
    for n in 0..=order {
        let mut m = -(n as i32);
        while m <= n as i32 {
            let base = PI / (n as f64 + 1.0);
            out.push(if m == 0 { base } else { 0.5 * base });
            m += 2;
        }
    }
    out
}

fn cm(x: Length) -> f64 {
    x.get::<centimeter>()
}

impl PowerShape {
    /// The relative power density at Monte Carlo point `p` (any unit; only
    /// ratios matter).
    #[must_use]
    pub fn relative_density(&self, p: [Length; 3]) -> f64 {
        self.eval(cm(p[0]), cm(p[1]), cm(p[2]))
    }

    /// True when the shape does not depend on the azimuth about the core axis.
    #[must_use]
    pub fn is_axisymmetric(&self) -> bool {
        match self {
            Self::Uniform
            | Self::Zones(_)
            | Self::ChoppedCosine { .. }
            | Self::Sine { .. }
            | Self::J0Cosine { .. }
            | Self::LegendreZ { .. }
            | Self::RzTable { .. }
            | Self::BesselLegendre { .. } => true,
            Self::Zernike {
                centre,
                coefficients,
                ..
            } => centred(centre) && azimuthal_free(coefficients.len(), |j| coefficients[j] == 0.0),
            Self::ZernikeLegendre {
                centre,
                coefficients,
                ..
            } => {
                centred(centre)
                    && coefficients.iter().all(|row| {
                        azimuthal_free(row.len(), |j| row[j] == 0.0)
                    })
            }
            Self::SphericalHarmonics {
                centre,
                coefficients,
                ..
            } => {
                cm(centre[0]) == 0.0
                    && cm(centre[1]) == 0.0
                    && {
                        let mut ok = true;
                        let mut k = 0;
                        let order = sh_order(coefficients.len()).unwrap_or(0);
                        for l in 0..=order {
                            for m in -(l as i32)..=(l as i32) {
                                if m != 0 && coefficients.get(k).copied().unwrap_or(0.0) != 0.0 {
                                    ok = false;
                                }
                                k += 1;
                            }
                        }
                        ok
                    }
            }
        }
    }

    /// The density at MC `(x, y, z)` cm.
    fn eval(&self, x: f64, y: f64, z: f64) -> f64 {
        match self {
            Self::Uniform => 1.0,
            Self::Zones(lumps) => {
                let r = x.r_hypot(y);
                lumps
                    .iter()
                    .filter(|l| {
                        let (r0, r1) = (cm(l.r[0]), cm(l.r[1]));
                        let (z0, z1) = (cm(l.z[0]).min(cm(l.z[1])), cm(l.z[0]).max(cm(l.z[1])));
                        r >= r0 && r <= r1 && z >= z0 && z <= z1
                    })
                    .map(|l| {
                        let (r0, r1) = (cm(l.r[0]), cm(l.r[1]));
                        let v = PI * (r1 * r1 - r0 * r0) * (cm(l.z[1]) - cm(l.z[0])).abs();
                        if v > 0.0 {
                            l.fraction / v
                        } else {
                            0.0
                        }
                    })
                    .sum()
            }
            Self::ChoppedCosine {
                centre,
                extrapolated_height,
            } => {
                let u = (z - cm(*centre)) / cm(*extrapolated_height);
                if u.abs() < 0.5 {
                    (PI * u).r_cos()
                } else {
                    0.0
                }
            }
            Self::Sine { bottom, height } => {
                let u = (z - cm(*bottom)) / cm(*height);
                if u > 0.0 && u < 1.0 {
                    (PI * u).r_sin()
                } else {
                    0.0
                }
            }
            Self::J0Cosine {
                centre,
                extrapolated_radius,
                extrapolated_height,
            } => {
                let r = x.r_hypot(y);
                let re = cm(*extrapolated_radius);
                let u = (z - cm(*centre)) / cm(*extrapolated_height);
                if r < re && u.abs() < 0.5 {
                    bessel_j0(J0_FIRST_ZERO * r / re) * (PI * u).r_cos()
                } else {
                    0.0
                }
            }
            Self::LegendreZ {
                z_min,
                z_max,
                coefficients,
            } => {
                let (a, b) = (cm(*z_min), cm(*z_max));
                if z < a || z > b || coefficients.is_empty() {
                    return 0.0;
                }
                let xi = 2.0 * (z - a) / (b - a) - 1.0;
                let p = legendre_pn(coefficients.len() - 1, xi);
                coefficients.iter().zip(&p).map(|(c, p)| c * p).sum()
            }
            Self::Zernike {
                centre,
                radius,
                coefficients,
            } => zernike_eval(centre, *radius, coefficients, x, y),
            Self::ZernikeLegendre {
                centre,
                radius,
                z_min,
                z_max,
                coefficients,
            } => {
                let (a, b) = (cm(*z_min), cm(*z_max));
                if z < a || z > b || coefficients.is_empty() {
                    return 0.0;
                }
                let xi = 2.0 * (z - a) / (b - a) - 1.0;
                let p = legendre_pn(coefficients.len() - 1, xi);
                coefficients
                    .iter()
                    .zip(&p)
                    .map(|(row, pn)| pn * zernike_eval(centre, *radius, row, x, y))
                    .sum()
            }
            Self::BesselLegendre {
                radius,
                alphas,
                z_min,
                z_max,
                coefficients,
            } => {
                let (a, b, rb) = (cm(*z_min), cm(*z_max), cm(*radius));
                let r = x.r_hypot(y);
                if z < a || z > b || r > rb || coefficients.is_empty() {
                    return 0.0;
                }
                let xi = 2.0 * (z - a) / (b - a) - 1.0;
                let p = legendre_pn(coefficients.len() - 1, xi);
                let j: Vec<f64> = alphas.iter().map(|al| bessel_j0(al * r / rb)).collect();
                coefficients
                    .iter()
                    .zip(&p)
                    .map(|(row, pn)| pn * row.iter().zip(&j).map(|(c, j)| c * j).sum::<f64>())
                    .sum()
            }
            Self::SphericalHarmonics {
                centre,
                radius,
                coefficients,
                radial,
            } => {
                let (dx, dy, dz) = (x - cm(centre[0]), y - cm(centre[1]), z - cm(centre[2]));
                let d = (dx * dx + dy * dy + dz * dz).sqrt();
                let rad = cm(*radius);
                if rad <= 0.0 || d > rad {
                    return 0.0;
                }
                let order = sh_order(coefficients.len()).unwrap_or(0);
                let (mu, phi) = if d < 1e-300 { (1.0, 0.0) } else { (dz / d, dy.r_atan2(dx)) };
                let y = real_spherical_harmonics(order, mu, phi);
                radial.eval(d / rad) * coefficients.iter().zip(&y).map(|(c, y)| c * y).sum::<f64>()
            }
            Self::RzTable {
                r_edges,
                z_edges,
                values,
            } => {
                let r = x.r_hypot(y);
                let re: Vec<f64> = r_edges.iter().map(|v| cm(*v)).collect();
                let ze: Vec<f64> = z_edges.iter().map(|v| cm(*v)).collect();
                match (bin_of(&re, r), bin_of(&ze, z)) {
                    (Some(i), Some(k)) => values.get(i + (re.len() - 1) * k).copied().unwrap_or(0.0),
                    _ => 0.0,
                }
            }
        }
    }

    /// Discontinuities in `r` (cm) the integrator must split at.
    fn r_breaks(&self) -> Vec<f64> {
        match self {
            Self::Zones(l) => l.iter().flat_map(|l| [cm(l.r[0]), cm(l.r[1])]).collect(),
            Self::J0Cosine {
                extrapolated_radius,
                ..
            } => vec![cm(*extrapolated_radius)],
            Self::Zernike { centre, radius, .. } | Self::ZernikeLegendre { centre, radius, .. }
                if centred(centre) =>
            {
                vec![cm(*radius)]
            }
            Self::RzTable { r_edges, .. } => r_edges.iter().map(|v| cm(*v)).collect(),
            Self::BesselLegendre { radius, .. } => vec![cm(*radius)],
            _ => Vec::new(),
        }
    }

    /// Discontinuities in `z` (cm).
    fn z_breaks(&self) -> Vec<f64> {
        match self {
            Self::Zones(l) => l.iter().flat_map(|l| [cm(l.z[0]), cm(l.z[1])]).collect(),
            Self::ChoppedCosine {
                centre,
                extrapolated_height,
            }
            | Self::J0Cosine {
                centre,
                extrapolated_height,
                ..
            } => {
                let (c, h) = (cm(*centre), cm(*extrapolated_height));
                vec![c - 0.5 * h, c + 0.5 * h]
            }
            Self::Sine { bottom, height } => vec![cm(*bottom), cm(*bottom) + cm(*height)],
            Self::LegendreZ { z_min, z_max, .. }
            | Self::ZernikeLegendre { z_min, z_max, .. }
            | Self::BesselLegendre { z_min, z_max, .. } => {
                vec![cm(*z_min), cm(*z_max)]
            }
            Self::SphericalHarmonics { centre, radius, .. } => {
                vec![cm(centre[2]) - cm(*radius), cm(centre[2]) + cm(*radius)]
            }
            Self::RzTable { z_edges, .. } => z_edges.iter().map(|v| cm(*v)).collect(),
            _ => Vec::new(),
        }
    }

    /// The integral of the shape over one cell, in (relative units) x cm^3.
    ///
    /// Rings and boxes by composite Gauss-Legendre split at the shape's
    /// discontinuities (module docs); points by the midpoint rule.
    #[must_use]
    pub fn cell_integral(&self, cell: &CellGeometry, frame: Frame, map: &FrameMap) -> f64 {
        match *cell {
            CellGeometry::Point { centre, volume } => {
                let p = map.to_mc(frame, centre);
                self.relative_density(p) * volume.get::<cubic_centimeter>()
            }
            CellGeometry::Annulus { r, z } => {
                let (z0, z1) = (cm(map.z_to_mc(frame, z[0])), cm(map.z_to_mc(frame, z[1])));
                let (z0, z1) = (z0.min(z1), z0.max(z1));
                self.integrate_ring([cm(r[0]), cm(r[1])], [z0, z1])
            }
            CellGeometry::Box { min, max } => {
                let (a, b) = (map.to_mc(frame, min), map.to_mc(frame, max));
                let lo = [cm(a[0]).min(cm(b[0])), cm(a[1]).min(cm(b[1])), cm(a[2]).min(cm(b[2]))];
                let hi = [cm(a[0]).max(cm(b[0])), cm(a[1]).max(cm(b[1])), cm(a[2]).max(cm(b[2]))];
                self.integrate_box(lo, hi)
            }
        }
    }

    fn integrate_ring(&self, r: [f64; 2], z: [f64; 2]) -> f64 {
        let rs = split(r, &self.r_breaks());
        let zs = split(z, &self.z_breaks());
        let axisym = self.is_axisymmetric();
        let (nodes, weights) = gauss_legendre_table(8).expect("8-point rule tabulated");
        let estimate = |s: usize| -> f64 {
            let n_theta = 16 * s;
            let mut acc = 0.0;
            for rp in &rs {
                for zp in &zs {
                    for (ra, rb) in pieces(*rp, s) {
                        for (za, zb) in pieces(*zp, s) {
                            let (hr, cr) = (0.5 * (rb - ra), 0.5 * (rb + ra));
                            let (hz, cz) = (0.5 * (zb - za), 0.5 * (zb + za));
                            for (xr, wr) in nodes.iter().zip(weights) {
                                let rr = cr + hr * xr;
                                for (xz, wz) in nodes.iter().zip(weights) {
                                    let zz = cz + hz * xz;
                                    let ang = if axisym {
                                        2.0 * PI * self.eval(rr, 0.0, zz)
                                    } else {
                                        let dt = 2.0 * PI / n_theta as f64;
                                        (0..n_theta)
                                            .map(|k| {
                                                let (st, ct) = ((k as f64 + 0.5) * dt).r_sin_cos();
                                                self.eval(rr * ct, rr * st, zz)
                                            })
                                            .sum::<f64>()
                                            * dt
                                    };
                                    acc += wr * wz * hr * hz * rr * ang;
                                }
                            }
                        }
                    }
                }
            }
            acc
        };
        converge(estimate, 1e-12, 16)
    }

    fn integrate_box(&self, lo: [f64; 3], hi: [f64; 3]) -> f64 {
        let zs = split([lo[2], hi[2]], &self.z_breaks());
        let (nodes, weights) = gauss_legendre_table(6).expect("6-point rule tabulated");
        let estimate = |s: usize| -> f64 {
            let mut acc = 0.0;
            for (xa, xb) in pieces([lo[0], hi[0]], s) {
                for (ya, yb) in pieces([lo[1], hi[1]], s) {
                    for zp in &zs {
                        for (za, zb) in pieces(*zp, s) {
                            let h = [0.5 * (xb - xa), 0.5 * (yb - ya), 0.5 * (zb - za)];
                            let c = [0.5 * (xb + xa), 0.5 * (yb + ya), 0.5 * (zb + za)];
                            for (u, wu) in nodes.iter().zip(weights) {
                                for (v, wv) in nodes.iter().zip(weights) {
                                    for (w, ww) in nodes.iter().zip(weights) {
                                        acc += wu * wv * ww * h[0] * h[1] * h[2]
                                            * self.eval(c[0] + h[0] * u, c[1] + h[1] * v, c[2] + h[2] * w);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            acc
        };
        converge(estimate, 1e-10, 4)
    }

    /// The shape's integral over each cell, (relative units) x cm^3.
    #[must_use]
    pub fn cell_integrals(&self, cells: &PowerCells) -> Vec<f64> {
        cells
            .cells
            .iter()
            .map(|c| self.cell_integral(c, cells.frame, &cells.map))
            .collect()
    }

    /// **Put `total` on `cells`**: each cell's power density, so that
    /// `sum_c q_c V_c = total` to round-off.
    ///
    /// `q_c = total I_c / (V_c sum_k I_k)`, `I_c` the cell integral
    /// ([`Self::cell_integral`]). The shape is renormalised over the cells:
    /// power the shape puts outside every cell is redistributed, not lost
    /// (check [`Self::captured_fraction`] when that matters).
    ///
    /// # Errors
    /// [`PowerShapeError::NoPowerInCells`] when the shape integrates to zero
    /// or less over the cells.
    pub fn cell_power_density(
        &self,
        cells: &PowerCells,
        total: Power,
    ) -> Result<Vec<PowerDensity>, PowerShapeError> {
        let ints = self.cell_integrals(cells);
        let sum: f64 = ints.iter().sum();
        if !(sum > 0.0) || !sum.is_finite() {
            return Err(PowerShapeError::NoPowerInCells(sum));
        }
        let p = total.get::<watt>();
        Ok(ints
            .iter()
            .zip(&cells.cells)
            .map(|(i, c)| {
                let v_m3 = c.volume().get::<uom::si::volume::cubic_meter>();
                let q = if v_m3 > 0.0 { p * i / sum / v_m3 } else { 0.0 };
                PowerDensity::new::<watt_per_cubic_meter>(q)
            })
            .collect())
    }

    /// Share of the shape's integral over the ring `r < r_max`, `z_lo < z <
    /// z_hi` (Monte Carlo frame) that `cells` hold: `sum_c I_c / I_support`.
    /// `1` means the cells capture all of it; less means power the shape puts
    /// in that support falls outside the cells (and is redistributed by
    /// [`Self::cell_power_density`]).
    #[must_use]
    pub fn captured_fraction(&self, cells: &PowerCells, r_max: Length, z: [Length; 2]) -> f64 {
        let whole = self.integrate_ring([0.0, cm(r_max)], [cm(z[0]).min(cm(z[1])), cm(z[0]).max(cm(z[1]))]);
        let held: f64 = self.cell_integrals(cells).iter().sum();
        held / whole
    }

    // ── From Monte Carlo ────────────────────────────────────────────────

    /// **The shape a Monte Carlo power tally measured** ([`McPowerTally`]):
    ///
    /// - a cylindrical mesh tally -> [`Self::RzTable`], each bin's mean over
    ///   its volume (`MeshKind::bin_volume`), so the table is a density;
    /// - a spatial Legendre (`z`) tally -> [`Self::LegendreZ`] with
    ///   `a_n = (2n+1)/(z_max - z_min) M_n`, `M_n` the raw moment (the filter
    ///   stores `int q P_n`, without OpenMC's `(2n+1)/2` reconstruction
    ///   factor, which is applied here);
    /// - a Zernike tally -> [`Self::Zernike`] with `c_j = M_j / (R^2 N_j)`,
    ///   `N_j = pi/(n+1)` (`m = 0`) or `pi/(2(n+1))` (the filter's
    ///   unnormalised polynomials' norms over the unit disc).
    ///
    /// `n_active` is the number of active generations (the tally's
    /// realisations). The first score of the tally is read.
    ///
    /// # Errors
    /// [`PowerShapeError::UnsupportedTally`] for any other tally (more than
    /// one filter, a non-cylindrical mesh, a Legendre filter on `x`/`y`).
    pub fn from_tally(tally: &Tally, n_active: usize) -> Result<Self, PowerShapeError> {
        let n = n_active as u64;
        if tally.filters.len() != 1 || tally.scores.is_empty() {
            return Err(PowerShapeError::UnsupportedTally(
                "one filter and at least one score expected".into(),
            ));
        }
        let ns = tally.scores.len();
        let mean = |b: usize| tally.bins.get(b * ns).map_or(0.0, |x| x.mean(n));
        match &tally.filters[0] {
            FilterKind::Mesh(MeshFilter {
                mesh: m @ MeshKind::Cylindrical(c),
            }) => {
                let d = c.dimension();
                if d[1] != 1 {
                    return Err(PowerShapeError::UnsupportedTally(
                        "an R-Z table needs one azimuthal bin".into(),
                    ));
                }
                let values = (0..d[0] * d[2])
                    .map(|b| {
                        let v = m.bin_volume(b).unwrap_or(0.0);
                        if v > 0.0 {
                            mean(b) / v
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let l = Length::new::<centimeter>;
                Ok(Self::RzTable {
                    r_edges: c.r_grid.iter().map(|v| l(*v)).collect(),
                    z_edges: c.z_grid.iter().map(|v| l(v + c.origin.z)).collect(),
                    values,
                })
            }
            FilterKind::SpatialLegendre(f) => {
                if f.axis != LegendreAxis::Z {
                    return Err(PowerShapeError::UnsupportedTally(
                        "only a Legendre expansion in z is an axial power shape".into(),
                    ));
                }
                let len = f.max - f.min;
                Ok(Self::LegendreZ {
                    z_min: Length::new::<centimeter>(f.min),
                    z_max: Length::new::<centimeter>(f.max),
                    coefficients: (0..=f.order)
                        .map(|k| (2.0 * k as f64 + 1.0) / len * mean(k))
                        .collect(),
                })
            }
            FilterKind::Zernike(f) => {
                let norms = zernike_norms(f.order);
                Ok(Self::Zernike {
                    centre: [Length::new::<centimeter>(f.x0), Length::new::<centimeter>(f.y0)],
                    radius: Length::new::<centimeter>(f.r),
                    coefficients: norms
                        .iter()
                        .enumerate()
                        .map(|(j, nj)| mean(j) / (f.r * f.r * nj))
                        .collect(),
                })
            }
            other => Err(PowerShapeError::UnsupportedTally(format!(
                "filter {} is not a power-shape filter",
                other.name()
            ))),
        }
    }

    /// **Zernike in `(r, theta)` times Legendre in `z`, from two tallies**
    /// (a Zernike and a Legendre-`z` [`McPowerTally`]; each must be a
    /// tally's only filter, so they are two tallies). The product assumes the
    /// power is **separable**, `q = sigma(r, theta) lambda(z) / P`: exact for
    /// a separable shape, an approximation otherwise.
    ///
    /// # Errors
    /// As [`Self::from_tally`], and either tally the wrong kind, or a zero
    /// total.
    pub fn zernike_legendre_from_tallies(
        zernike: &Tally,
        legendre: &Tally,
        n_active: usize,
    ) -> Result<Self, PowerShapeError> {
        let (Self::Zernike { centre, radius, coefficients: c }, Self::LegendreZ { z_min, z_max, coefficients: a }) =
            (Self::from_tally(zernike, n_active)?, Self::from_tally(legendre, n_active)?)
        else {
            return Err(PowerShapeError::UnsupportedTally(
                "a Zernike tally and a Legendre-z tally expected, in that order".into(),
            ));
        };
        // P = int lambda dz = a_0 (z_max - z_min).
        let total = a[0] * cm(z_max - z_min);
        if !(total.abs() > 0.0) {
            return Err(PowerShapeError::UnsupportedTally("the Legendre tally holds no power".into()));
        }
        Ok(Self::ZernikeLegendre {
            centre,
            radius,
            z_min,
            z_max,
            coefficients: a.iter().map(|an| c.iter().map(|cj| an * cj / total).collect()).collect(),
        })
    }

    /// **A Fourier-Bessel radial shape times a Legendre-z axial one**:
    /// `c[n][k] = a_n b_k`, with `radial` from [`zernike_to_bessel`] and
    /// `legendre` the `a_n` of a [`Self::LegendreZ`] on `[z_min, z_max]`.
    #[must_use]
    pub fn bessel_legendre(radial: &BesselRadial, legendre: &[f64], z_min: Length, z_max: Length) -> Self {
        Self::BesselLegendre {
            radius: radial.radius,
            alphas: radial.alphas.clone(),
            z_min,
            z_max,
            coefficients: legendre
                .iter()
                .map(|a| radial.coefficients.iter().map(|b| a * b).collect())
                .collect(),
        }
    }

    // ── Pictures ────────────────────────────────────────────────────────

    /// **Draw the shape in R-Z** (the `x >= 0, y = 0` half-plane, Monte
    /// Carlo frame) as a PNG with a colour key, `px_per_cm` pixels per cm.
    /// Colours run from white (0) through yellow and orange to dark red (the
    /// maximum); negative values (an expansion's undershoot) in blue.
    ///
    /// # Errors
    /// The PNG cannot be written.
    pub fn write_rz_png(
        &self,
        path: impl AsRef<std::path::Path>,
        title: &str,
        r_max: Length,
        z: [Length; 2],
        px_per_cm: f64,
    ) -> std::io::Result<()> {
        let (rm, z0, z1) = (cm(r_max), cm(z[0]).min(cm(z[1])), cm(z[0]).max(cm(z[1])));
        let w = ((rm * px_per_cm).round() as usize).max(2);
        let h = (((z1 - z0) * px_per_cm).round() as usize).max(2);
        let mut v = vec![0.0; w * h];
        for py in 0..h {
            let zz = z1 - (py as f64 + 0.5) / px_per_cm;
            for px in 0..w {
                v[py * w + px] = self.eval((px as f64 + 0.5) / px_per_cm, 0.0, zz);
            }
        }
        write_scalar_png(path.as_ref(), title, w, h, &v)
    }

    /// **Draw the shape on the plane `z` (Monte Carlo frame)**, `x, y` in
    /// `[-half_width, half_width]`, as [`Self::write_rz_png`] does in R-Z.
    ///
    /// # Errors
    /// The PNG cannot be written.
    pub fn write_xy_png(
        &self,
        path: impl AsRef<std::path::Path>,
        title: &str,
        z: Length,
        half_width: Length,
        px_per_cm: f64,
    ) -> std::io::Result<()> {
        let hw = cm(half_width);
        let n = ((2.0 * hw * px_per_cm).round() as usize).max(2);
        let mut v = vec![0.0; n * n];
        for py in 0..n {
            let y = hw - (py as f64 + 0.5) / px_per_cm;
            for px in 0..n {
                let x = -hw + (px as f64 + 0.5) / px_per_cm;
                v[py * n + px] = self.eval(x, y, cm(z));
            }
        }
        write_scalar_png(path.as_ref(), title, n, n, &v)
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Zernike -> Fourier-Bessel
// ─────────────────────────────────────────────────────────────────────────

/// Which Fourier-Bessel series a radial shape is re-expressed in.
///
/// **The default is [`Self::Neumann`]**, because a Zernike reconstruction of
/// a reflected core's power is generally **non-zero at the disc edge**: the
/// Dini (Neumann) modes `J0(alpha_k r/R)`, `J1(alpha_k) = 0`, with the
/// constant `k = 0` term, carry a non-zero edge value and converge fast. The
/// Dirichlet modes all vanish at `R_b`, so with `R_b = R` they can only
/// approach a non-zero edge value slowly and with Gibbs ringing; they are
/// the bare cylinder's modes, an explicit choice for a bare (or
/// extrapolated, `R_b > R`) description.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BesselBoundary {
    /// `alpha_k` the zeros of `J0` at the radius `R_b = extrapolated_radius
    /// >= R` (the disc radius): `f(R_b) = 0`.
    Dirichlet {
        /// `R_b`, at least the disc radius.
        extrapolated_radius: Length,
    },
    /// `alpha_0 = 0` and `alpha_k` the zeros of `J1` (`f'(R) = 0`), at the
    /// disc radius: the Dini series.
    #[default]
    Neumann,
}

/// A radial Fourier-Bessel series `f(r) = sum_k b_k J0(alpha_k r / R_b)` on
/// `r <= R_b`.
#[derive(Debug, Clone, PartialEq)]
pub struct BesselRadial {
    /// `R_b`.
    pub radius: Length,
    /// `alpha_k`.
    pub alphas: Vec<f64>,
    /// `b_k` (the units of the Zernike shape it came from).
    pub coefficients: Vec<f64>,
    /// The series' boundary condition.
    pub boundary: BesselBoundary,
}

impl BesselRadial {
    /// `f(r)` (zero beyond `R_b`).
    #[must_use]
    pub fn eval(&self, r: Length) -> f64 {
        let (r, rb) = (cm(r), cm(self.radius));
        if r > rb {
            return 0.0;
        }
        self.alphas.iter().zip(&self.coefficients).map(|(a, b)| b * bessel_j0(a * r / rb)).sum()
    }

    /// `int_0^{r_max} 2 pi r f(r) dr`, cm^2 times the shape's unit: exact
    /// per mode (`int_0^x t J0(t) dt = x J1(x)`, A&S 11.3.20).
    #[must_use]
    pub fn disc_integral(&self, r_max: Length) -> f64 {
        let rb = cm(self.radius);
        let x = cm(r_max).min(rb) / rb;
        self.alphas
            .iter()
            .zip(&self.coefficients)
            .map(|(a, b)| {
                let m = if *a == 0.0 { 0.5 * x * x } else { x * bessel_j1(a * x) / a };
                2.0 * PI * rb * rb * b * m
            })
            .sum()
    }
}

/// The first `n` positive zeros of `J0` (`order = 0`) or `J1` (`order = 1`),
/// by Newton from McMahon's asymptotic start `(k - 1/4) pi` / `(k + 1/4)
/// pi` (A&S 9.5.12), on petir's `J0`, `J1` (`J0' = -J1`, `J1' = J0 - J1/x`),
/// to round-off.
#[must_use]
pub fn bessel_zeros(order: usize, n: usize) -> Vec<f64> {
    (1..=n)
        .map(|k| {
            let mut x = if order == 0 { (k as f64 - 0.25) * PI } else { (k as f64 + 0.25) * PI };
            for _ in 0..50 {
                let (f, df) = if order == 0 {
                    (bessel_j0(x), -bessel_j1(x))
                } else {
                    (bessel_j1(x), bessel_j0(x) - bessel_j1(x) / x)
                };
                let dx = f / df;
                x -= dx;
                if dx.abs() <= 1e-15 * x {
                    break;
                }
            }
            x
        })
        .collect()
}

/// The radial profile `f(r)`, `r <= R`, of the `m = 0` moments of a Zernike
/// coefficient vector in Monte Carlo filter order (the `m != 0` moments are
/// dropped: `f` is the azimuthal average).
fn zernike_m0_profile(zernike: &[f64], rho: f64) -> f64 {
    let order = zernike_order(zernike.len()).unwrap_or(0);
    let z = zernike_zn(order, rho, 0.0);
    let mut j = 0;
    let mut f = 0.0;
    for n in 0..=order {
        let mut m = -(n as i32);
        while m <= n as i32 {
            if m == 0 {
                f += zernike[j] * z[j];
            }
            j += 1;
            m += 2;
        }
    }
    f
}

/// **Re-express a Zernike radial shape in Fourier-Bessel modes** (no Monte
/// Carlo Bessel filter: the tally stays Zernike, this is a projection).
///
/// `zernike` is a coefficient vector in filter order on a disc of
/// `disc_radius` ([`PowerShape::Zernike`]'s, e.g. from
/// [`PowerShape::from_tally`]); its `m = 0` part `f(r)` is projected on
/// `n_terms` modes with the Bessel orthogonality weight `r`:
///
/// - **Dirichlet** (`alpha_k` zeros of `J0`, `R_b >= R`; `f = 0` beyond
///   `R`): `b_k = 2 / (R_b^2 J1(alpha_k)^2) int_0^{R_b} f(r) J0(alpha_k r /
///   R_b) r dr` (Watson, *Treatise on the Theory of Bessel Functions*, 2nd
///   ed., Sec 18.24; the norm `int_0^1 x J0(alpha x)^2 dx = J1(alpha)^2/2`,
///   A&S 11.4.5);
/// - **Neumann / Dini** (`alpha_0 = 0`, `alpha_k` zeros of `J1`, `R_b =
///   R`): `b_0 = (2/R^2) int_0^R f r dr`, `b_k = 2 / (R^2 J0(alpha_k)^2)
///   int_0^R f J0(alpha_k r/R) r dr` (Watson Sec 18.33, the Dini series with
///   `H = 0`; the norm is `J0(alpha)^2/2` where `J1(alpha) = 0`).
///
/// The integrand is a polynomial times `J0`, integrated by composite
/// 30-point Gauss-Legendre refined until two estimates agree to 1e-14.
///
/// **Power**: with Neumann the disc integral is carried by `b_0` alone
/// (`int_0^R J0(alpha_k r/R) r dr = 0`), so it is conserved exactly at every
/// `n_terms >= 1`. With Dirichlet every mode carries some, and the
/// truncated series' integral converges to the Zernike one as `n_terms`
/// grows ([`BesselRadial::disc_integral`] measures it).
///
/// # Errors
/// `n_terms == 0`, a non-positive radius, a Dirichlet radius below the disc
/// radius, or a vector whose length is not a Zernike count.
pub fn zernike_to_bessel(
    zernike: &[f64],
    disc_radius: Length,
    n_terms: usize,
    boundary: BesselBoundary,
) -> Result<BesselRadial, PowerShapeError> {
    let r = cm(disc_radius);
    if n_terms == 0 || !(r > 0.0) || zernike_order(zernike.len()).is_none() {
        return Err(PowerShapeError::Input(
            "n_terms >= 1, a positive radius and a Zernike-length vector are needed".into(),
        ));
    }
    let (rb, alphas, norm): (f64, Vec<f64>, Vec<f64>) = match boundary {
        BesselBoundary::Dirichlet { extrapolated_radius } => {
            let rb = cm(extrapolated_radius);
            if rb < r {
                return Err(PowerShapeError::Input("the Dirichlet radius is below the disc radius".into()));
            }
            let a = bessel_zeros(0, n_terms);
            let n = a.iter().map(|a| 2.0 / (rb * rb * bessel_j1(*a).r_powi(2))).collect();
            (rb, a, n)
        }
        BesselBoundary::Neumann => {
            let mut a = vec![0.0];
            a.extend(bessel_zeros(1, n_terms - 1));
            let n = a
                .iter()
                .map(|a| if *a == 0.0 { 2.0 / (r * r) } else { 2.0 / (r * r * bessel_j0(*a).r_powi(2)) })
                .collect();
            (r, a, n)
        }
    };
    let (nodes, weights) = gauss_legendre_table(30).expect("30-point rule tabulated");
    let coefficients = alphas
        .iter()
        .zip(&norm)
        .map(|(a, nk)| {
            // f is zero beyond R: integrate on [0, R] only.
            let integral = converge(
                |s| {
                    pieces([0.0, r], s)
                        .map(|(lo, hi)| {
                            let (h, c) = (0.5 * (hi - lo), 0.5 * (hi + lo));
                            nodes
                                .iter()
                                .zip(weights)
                                .map(|(x, w)| {
                                    let rr = c + h * x;
                                    w * h * rr * zernike_m0_profile(zernike, rr / r) * bessel_j0(a * rr / rb)
                                })
                                .sum::<f64>()
                        })
                        .sum()
                },
                1e-14,
                64,
            );
            nk * integral
        })
        .collect();
    Ok(BesselRadial {
        radius: Length::new::<centimeter>(rb),
        alphas,
        coefficients,
        boundary,
    })
}

/// `int_0^R 2 pi r f(r) dr` of the `m = 0` Zernike profile (cm^2 times its
/// unit): `pi R^2 c_00`, the only moment with a disc integral.
#[must_use]
pub fn zernike_disc_integral(zernike: &[f64], disc_radius: Length) -> f64 {
    PI * cm(disc_radius).r_powi(2) * zernike.first().copied().unwrap_or(0.0)
}

/// **Picture: a Zernike radial profile against its Fourier-Bessel
/// reconstructions** with `n_terms` of each listed count, both boundaries.
///
/// # Errors
/// The conversion fails or the file cannot be written.
pub fn write_bessel_comparison_png(
    path: impl AsRef<std::path::Path>,
    zernike: &[f64],
    disc_radius: Length,
    dirichlet_radius: Length,
    n_terms: &[usize],
) -> Result<(), String> {
    let n = 200;
    let r: Vec<f64> = (0..=n).map(|k| cm(disc_radius) * k as f64 / n as f64).collect();
    let z: Vec<f64> = r.iter().map(|x| zernike_m0_profile(zernike, x / cm(disc_radius))).collect();
    let black = Rgb::new(0, 0, 0);
    let blues = [Rgb::new(150, 190, 240), Rgb::new(60, 120, 220), Rgb::new(10, 40, 140)];
    let reds = [Rgb::new(250, 170, 150), Rgb::new(230, 80, 50), Rgb::new(140, 20, 10)];
    let zernike_series = (black, r.clone(), z, vec![0.0; r.len()]);
    let mut series = Vec::new();
    let mut legend = vec![LegendEntry::new(black, "ZERNIKE (M = 0)")];
    for (k, &t) in n_terms.iter().enumerate() {
        for (b, cols, name) in [
            (BesselBoundary::Neumann, &blues, "NEUMANN"),
            (BesselBoundary::Dirichlet { extrapolated_radius: dirichlet_radius }, &reds, "DIRICHLET"),
        ] {
            let s = zernike_to_bessel(zernike, disc_radius, t, b).map_err(|e| e.to_string())?;
            let y: Vec<f64> = r.iter().map(|x| s.eval(Length::new::<centimeter>(*x))).collect();
            let c = cols[k.min(2)];
            series.push((c, r.clone(), y, vec![0.0; r.len()]));
            legend.push(LegendEntry::new(c, format!("{name} {t} TERMS")));
        }
    }
    legend.push(LegendEntry::new(black, format!("X: R 0-{:.0} CM, Y: RELATIVE POWER", cm(disc_radius))));
    series.push(zernike_series); // drawn last, on top
    let img = line_plot(&series, 520, 340);
    annotate_image(&img, "ZERNIKE -> FOURIER-BESSEL (RADIAL)", &legend)
        .write_png(path)
        .map_err(|e| e.to_string())
}

/// A minimal line plot: each series `(colour, x, y, y_error)` scaled into a
/// `w x h` box with a frame; error bars where `y_error > 0`.
pub(crate) fn line_plot(series: &[(Rgb, Vec<f64>, Vec<f64>, Vec<f64>)], w: usize, h: usize) -> ImageData {
    let mut img = ImageData::filled(w, h, Rgb::new(255, 255, 255));
    let pad = 12.0;
    let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for (_, xs, ys, es) in series {
        for ((x, y), e) in xs.iter().zip(ys).zip(es) {
            x0 = x0.min(*x);
            x1 = x1.max(*x);
            y0 = y0.min(y - e);
            y1 = y1.max(y + e);
        }
    }
    if !(x1 > x0) {
        x1 = x0 + 1.0;
    }
    if !(y1 > y0) {
        y1 = y0 + 1.0;
    }
    let dy = 0.05 * (y1 - y0);
    let (y0, y1) = (y0 - dy, y1 + dy);
    let px = |x: f64| pad + (x - x0) / (x1 - x0) * (w as f64 - 2.0 * pad);
    let py = |y: f64| h as f64 - pad - (y - y0) / (y1 - y0) * (h as f64 - 2.0 * pad);
    let put = |img: &mut ImageData, x: f64, y: f64, c: Rgb| {
        let (xi, yi) = (x.round() as i64, y.round() as i64);
        if xi >= 0 && yi >= 0 && (xi as usize) < w && (yi as usize) < h {
            img.set(xi as usize, yi as usize, c);
        }
    };
    let black = Rgb::new(0, 0, 0);
    for t in 0..w {
        put(&mut img, t as f64, pad, black);
        put(&mut img, t as f64, h as f64 - pad, black);
    }
    for t in 0..h {
        put(&mut img, pad, t as f64, black);
        put(&mut img, w as f64 - pad, t as f64, black);
    }
    if y0 < 0.0 && y1 > 0.0 {
        for t in 0..w {
            put(&mut img, t as f64, py(0.0), Rgb::new(180, 180, 180));
        }
    }
    for (c, xs, ys, es) in series {
        let dense = xs.len() > 50;
        for i in 0..xs.len() {
            let (cx, cy) = (px(xs[i]), py(ys[i]));
            if !dense {
                for d in -2i32..=2 {
                    for e in -2i32..=2 {
                        put(&mut img, cx + d as f64, cy + e as f64, *c);
                    }
                }
            }
            if es[i] > 0.0 {
                let (a, b) = (py(ys[i] - es[i]), py(ys[i] + es[i]));
                let mut t = b.min(a);
                while t <= a.max(b) {
                    put(&mut img, cx, t, *c);
                    t += 0.5;
                }
            }
            if i + 1 < xs.len() {
                let (nx, ny) = (px(xs[i + 1]), py(ys[i + 1]));
                let n = ((nx - cx).abs().max((ny - cy).abs()) * 2.0) as usize + 1;
                for s in 0..=n {
                    let f = s as f64 / n as f64;
                    put(&mut img, cx + f * (nx - cx), cy + f * (ny - cy), *c);
                }
            }
        }
    }
    img
}

fn centred(c: &[Length; 2]) -> bool {
    cm(c[0]) == 0.0 && cm(c[1]) == 0.0
}

/// True when every `m != 0` Zernike moment of `n_terms` is zero per `is_zero`.
fn azimuthal_free(n_terms: usize, is_zero: impl Fn(usize) -> bool) -> bool {
    let order = zernike_order(n_terms).unwrap_or(0);
    let mut j = 0;
    for n in 0..=order {
        let mut m = -(n as i32);
        while m <= n as i32 {
            if m != 0 && j < n_terms && !is_zero(j) {
                return false;
            }
            j += 1;
            m += 2;
        }
    }
    true
}

fn zernike_eval(centre: &[Length; 2], radius: Length, c: &[f64], x: f64, y: f64) -> f64 {
    let (dx, dy) = (x - cm(centre[0]), y - cm(centre[1]));
    let r = cm(radius);
    if r <= 0.0 || c.is_empty() {
        return 0.0;
    }
    let rho = (dx * dx + dy * dy).sqrt() / r;
    if rho > 1.0 {
        return 0.0;
    }
    let order = zernike_order(c.len()).unwrap_or(0);
    let z = zernike_zn(order, rho, dy.r_atan2(dx));
    c.iter().zip(&z).map(|(a, b)| a * b).sum()
}

/// Bin of `x` in ascending `edges` (last edge inclusive).
fn bin_of(edges: &[f64], x: f64) -> Option<usize> {
    let n = edges.len();
    if n < 2 || x < edges[0] || x > edges[n - 1] {
        return None;
    }
    Some(edges.partition_point(|e| *e <= x).saturating_sub(1).min(n - 2))
}

/// `[a, b]` split at every break strictly inside it.
fn split(ab: [f64; 2], breaks: &[f64]) -> Vec<[f64; 2]> {
    let mut pts: Vec<f64> = breaks
        .iter()
        .copied()
        .filter(|x| x.is_finite() && *x > ab[0] && *x < ab[1])
        .collect();
    pts.push(ab[0]);
    pts.push(ab[1]);
    pts.sort_by(f64::total_cmp);
    pts.dedup();
    pts.windows(2).map(|w| [w[0], w[1]]).collect()
}

/// `[a, b]` in `s` equal pieces.
fn pieces(ab: [f64; 2], s: usize) -> impl Iterator<Item = (f64, f64)> {
    let h = (ab[1] - ab[0]) / s as f64;
    (0..s).map(move |k| (ab[0] + k as f64 * h, if k + 1 == s { ab[1] } else { ab[0] + (k + 1) as f64 * h }))
}

/// Double the subdivision until two estimates agree to `rel` (or `max_s`).
fn converge(f: impl Fn(usize) -> f64, rel: f64, max_s: usize) -> f64 {
    let mut s = 1;
    let mut prev = f(s);
    while s < max_s {
        s *= 2;
        let next = f(s);
        let scale = next.abs().max(prev.abs());
        if (next - prev).abs() <= rel * scale || scale == 0.0 {
            return next;
        }
        prev = next;
    }
    prev
}

/// A scalar field as a PNG with a ten-step colour key.
fn write_scalar_png(path: &std::path::Path, title: &str, w: usize, h: usize, v: &[f64]) -> std::io::Result<()> {
    let vmax = v.iter().fold(0.0f64, |a, b| a.max(b.abs()));
    let colour = |x: f64| -> Rgb {
        if vmax <= 0.0 {
            return Rgb::new(255, 255, 255);
        }
        let t = (x / vmax).clamp(-1.0, 1.0);
        if t < 0.0 {
            let s = -t;
            return Rgb::new((255.0 * (1.0 - s)) as u8, (255.0 * (1.0 - 0.6 * s)) as u8, 255);
        }
        // white -> yellow -> orange -> dark red
        let stops = [(255.0, 255.0, 255.0), (255.0, 230.0, 80.0), (240.0, 120.0, 30.0), (120.0, 0.0, 0.0)];
        let u = t * 3.0;
        let k = (u.floor() as usize).min(2);
        let f = u - k as f64;
        let (a, b) = (stops[k], stops[k + 1]);
        Rgb::new(
            (a.0 + f * (b.0 - a.0)) as u8,
            (a.1 + f * (b.1 - a.1)) as u8,
            (a.2 + f * (b.2 - a.2)) as u8,
        )
    };
    let mut img = ImageData::filled(w, h, Rgb::new(255, 255, 255));
    for y in 0..h {
        for x in 0..w {
            img.set(x, y, colour(v[y * w + x]));
        }
    }
    let legend: Vec<LegendEntry> = (0..=5)
        .rev()
        .map(|k| {
            let t = k as f64 / 5.0;
            LegendEntry::new(colour(t * vmax), format!("{:.1} OF MAX", t))
        })
        .chain(std::iter::once(LegendEntry::new(colour(-vmax), "NEGATIVE")))
        .collect();
    annotate_image(&img, title, &legend).write_png(path)
}

// ─────────────────────────────────────────────────────────────────────────
// Monte Carlo tallies
// ─────────────────────────────────────────────────────────────────────────

/// Builders of the Monte Carlo **power tallies** whose result
/// [`PowerShape::from_tally`] reads: each scores
/// [`ScoreType::KappaFission`] (fission rate times the single recoverable
/// `Q`, `outram_mc_libs::tally::scoring::Q_FISSION_J`), J per source
/// particle, through one spatial filter. All coordinates in the Monte Carlo
/// frame (cm, `z` up).
///
/// In a delta-tracked region (the HTR-10 bed) the tally is scored at the
/// tentative-collision sites (gh:#598); elsewhere by track length at the
/// segment midpoint, which is exact for a mesh bin only while the bin is
/// large against a mean free path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McPowerTally;

impl McPowerTally {
    /// An R-Z mesh tally on rings `r_edges` x slabs `z_edges` (both
    /// ascending, Monte Carlo frame), one azimuthal bin.
    ///
    /// # Errors
    /// Fewer than two edges on an axis, or edges not ascending, or a negative
    /// radius.
    pub fn rz_mesh(r_edges: &[Length], z_edges: &[Length]) -> Result<Tally, PowerShapeError> {
        let r: Vec<f64> = r_edges.iter().map(|v| cm(*v)).collect();
        let z: Vec<f64> = z_edges.iter().map(|v| cm(*v)).collect();
        for (name, e) in [("r", &r), ("z", &z)] {
            if e.len() < 2 || e.windows(2).any(|w| !(w[1] > w[0])) {
                return Err(PowerShapeError::Input(format!("{name} edges must ascend, two or more")));
            }
        }
        if r[0] < 0.0 {
            return Err(PowerShapeError::Input("negative radius".into()));
        }
        let mesh = MeshKind::Cylindrical(CylindricalMesh {
            r_grid: r.clone(),
            phi_grid: vec![0.0, 2.0 * PI],
            z_grid: z.clone(),
            origin: Position::new(0.0, 0.0, 0.0),
        });
        Ok(power_tally(
            1184,
            "nee_soon power R-Z",
            FilterKind::Mesh(MeshFilter { mesh }),
            (r.len() - 1) * (z.len() - 1),
        ))
    }

    /// A Legendre expansion in `z` of order `order` over `[z_min, z_max]`.
    ///
    /// # Errors
    /// `z_max <= z_min`.
    pub fn legendre_z(order: usize, z_min: Length, z_max: Length) -> Result<Tally, PowerShapeError> {
        if !(cm(z_max) > cm(z_min)) {
            return Err(PowerShapeError::Input("z_max must exceed z_min".into()));
        }
        Ok(power_tally(
            1185,
            "nee_soon power Legendre-z",
            FilterKind::SpatialLegendre(SpatialLegendreFilter {
                order,
                axis: LegendreAxis::Z,
                min: cm(z_min),
                max: cm(z_max),
            }),
            order + 1,
        ))
    }

    /// A Zernike expansion of radial order `order` on the disc of `radius`
    /// about `centre` (`(x, y)`), all heights.
    ///
    /// # Errors
    /// A non-positive radius.
    pub fn zernike(order: usize, centre: [Length; 2], radius: Length) -> Result<Tally, PowerShapeError> {
        if !(cm(radius) > 0.0) {
            return Err(PowerShapeError::Input("radius must be positive".into()));
        }
        Ok(power_tally(
            1186,
            "nee_soon power Zernike",
            FilterKind::Zernike(ZernikeFilter {
                order,
                x0: cm(centre[0]),
                y0: cm(centre[1]),
                r: cm(radius),
            }),
            (order + 1) * (order + 2) / 2,
        ))
    }
}

fn power_tally(id: i32, name: &str, filter: FilterKind, n_bins: usize) -> Tally {
    Tally {
        id,
        name: name.into(),
        filters: vec![filter],
        scores: vec![ScoreType::KappaFission],
        bins: vec![TallyBin::default(); n_bins],
    }
}

#[cfg(test)]
#[path = "power_shape_tests.rs"]
mod tests;
