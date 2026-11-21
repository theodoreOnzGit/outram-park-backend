// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): this module is a port of
// OpenMC's photon interaction data and samplers.
//   Upstream project: OpenMC, https://github.com/openmc-dev/openmc
//   Upstream files:   src/photon.cpp, include/openmc/photon.h
//   Upstream commit:  8f79d9d0 (the commit this workspace's OpenMC ports use)
//   Upstream licence: MIT, Copyright (c) 2011-2025 Massachusetts Institute of
//                     Technology, UChicago Argonne LLC, and OpenMC contributors
//   This port:        AGPL-3.0-only, with the MIT notice preserved here.
//! **OpenMC photon interaction samplers** (gh:#975 / #1683).
//!
//! **Status: first port, not differentially tested (gh:#1683).** Nothing here
//! has been compared with a compiled OpenMC; that is phase 2.
//!
//! # What is ported (upstream `src/photon.cpp` at `8f79d9d0`)
//!
//! | upstream | here |
//! |---|---|
//! | `PhotonInteraction::calc_max_stack_size`, `calc_helper` (`:417-463`) | [`PhotonInteraction::calc_max_stack_size`] |
//! | `PhotonInteraction::compton_scatter` (`:465-498`) | [`PhotonInteraction::compton_scatter`] |
//! | `compton_profile_cdf`, `invert_compton_profile_cdf` (`:500-559`) | [`PhotonInteraction::compton_profile_cdf`], [`PhotonInteraction::invert_compton_profile_cdf`] |
//! | `compton_shell_kinematics`, `sample_compton_momentum` (`:561-623`) | private, same names |
//! | `compton_doppler_conditional`, `compton_doppler` (`:625-709`) | private, same names |
//! | `calculate_xs` (`:711-767`) | [`PhotonInteraction::calculate_xs`] |
//! | `rayleigh_scatter` (`:769-796`) | [`PhotonInteraction::rayleigh_scatter`] |
//! | `pair_production` (`:798-922`) | [`PhotonInteraction::pair_production`] |
//! | `atomic_relaxation` (`:924-997`) | [`PhotonInteraction::atomic_relaxation`] |
//! | `detail::compton_profile_tail_integral`, `invert_compton_profile_tail`, `compton_energy_ratio` (`:999-1066`) | [`detail`] |
//! | `klein_nishina` (`:1068-1123`) | [`klein_nishina`] |
//! | `Tabulated1D::operator()` (`src/endf.cpp:243-299`) | [`Tabulated1D::eval`] |
//! | `lower_bound_index`, `upper_bound_index` (`include/openmc/search.h`) | [`lower_bound_index`], [`upper_bound_index`] |
//!
//! # What is NOT here
//!
//! - **The HDF5 constructor** (`PhotonInteraction::PhotonInteraction(hid_t)`,
//!   `:46-411`). Data is passed in by value as a [`PhotonInteraction`]; a
//!   reader belongs in `njoy-outram-park-fork` (brief, "Data files").
//! - **`Particle` coupling.** Upstream's samplers read and write a `Particle`
//!   (`p.E()`, `p.photon_xs`, `p.create_secondary`). Here they take scalars and
//!   a random-number seed and return values. Atomic relaxation returns the
//!   secondaries it would have created as a list ([`RelaxationSecondary`]) and
//!   the caller decides what to do with each: that list is the **seam** at
//!   `process_charged_secondary` where a charged particle could be handed to
//!   `punggol::egsnrc` (see [`crate::openmc_photon::reaction`]).
//! - The `data::` globals: the Compton-profile momentum grid is held per
//!   element as an `Arc<[f64]>` ([`PhotonInteraction::profile_pz`]).
//!
//! # Findings from reading upstream
//!
//! - **`pair_production` indexes the screening-radius table `r[]` by `Z_`,**
//!   but the table's first entry (122.81) is hydrogen's value
//!   (PENELOPE-2011, Z = 1), so `r[Z_]` reads the radius of element `Z + 1`,
//!   and `Z = 99` reads one past the end. Ported as upstream has it
//!   ([`SCREENING_RADIUS`] is indexed by `Z`, with `Z = 99` clamped to the last
//!   entry instead of reading out of bounds). For phase 2 to confirm against
//!   the PENELOPE table.
//! - Maths goes through `petir` (`RealMath`, `Real`): never the platform libm.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use alloc::sync::Arc;
use alloc::vec::Vec;
use petir::mathf::RealMath;
use petir::real::Real;
use petir::rng::lcg::prn;

pub mod bremsstrahlung;
pub mod ttb_tables;
#[cfg(test)]
mod tests;
pub mod reaction;

/// Electron rest energy, eV (`include/openmc/constants.h:113`).
pub const MASS_ELECTRON_EV: f64 = 0.51099895000e6;
/// Inverse fine-structure constant (`constants.h:115`).
pub const FINE_STRUCTURE: f64 = 137.035999084;
/// Planck's constant times c, eV-Angstrom (`constants.h:117`).
pub const PLANCK_C: f64 = 1.2398419839593942e4;
/// Number of electron subshells upstream knows (`SUBSHELLS`, `constants.h:125`).
pub const N_SUBSHELLS: usize = 39;
/// Maximum possible size of the atomic-relaxation vacancy stack
/// (`PhotonInteraction::MAX_STACK_SIZE`, `photon.h`).
pub const MAX_STACK_SIZE: usize = 7;

// ── search.h ────────────────────────────────────────────────────────────────

/// OpenMC's `lower_bound_index` (`include/openmc/search.h:15-22`): `0` when
/// `value` equals the first element, else `lower_bound(value) - 1`. Upstream
/// returns a signed difference and a value below the first element indexes
/// `-1` (undefined behaviour there); here that case saturates to `0`.
pub fn lower_bound_index(slice: &[f64], value: f64) -> usize {
    if slice.first() == Some(&value) {
        return 0;
    }
    slice.partition_point(|&e| e < value).saturating_sub(1)
}

/// OpenMC's `upper_bound_index` (`search.h:24-28`): `upper_bound(value) - 1`.
pub fn upper_bound_index(slice: &[f64], value: f64) -> usize {
    slice.partition_point(|&e| e <= value).saturating_sub(1)
}

/// `uniform_distribution(a, b, seed)` (`src/random_dist.cpp:10-13`).
fn uniform_distribution(a: f64, b: f64, seed: &mut u64) -> f64 {
    a + (b - a) * prn(seed)
}

/// `isotropic_direction` as photon.cpp calls it (`src/distribution_multi.cpp`,
/// azimuth drawn first, then the cosine): see
/// [`outram_mc_libs::rng::distributions::isotropic_direction_openmc`].
fn isotropic_direction(seed: &mut u64) -> (f64, f64, f64) {
    outram_mc_libs::rng::distributions::isotropic_direction_openmc(seed)
}

// ── Tabulated1D (src/endf.cpp:243-299) ──────────────────────────────────────

/// ENDF interpolation law of one region (`enum class Interpolation`,
/// `constants.h:366`). Quadratic (7) and cubic (8) are refused by
/// [`Tabulated1D::new`]; upstream throws "Invalid interpolation scheme" when it
/// evaluates one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    /// ENDF INT = 1.
    Histogram,
    /// INT = 2.
    LinLin,
    /// INT = 3.
    LinLog,
    /// INT = 4.
    LogLin,
    /// INT = 5.
    LogLog,
}

impl Interpolation {
    /// The ENDF code, `None` for an unsupported one (`int2interp`).
    pub fn from_endf(code: i32) -> Option<Self> {
        Some(match code {
            1 => Self::Histogram,
            2 => Self::LinLin,
            3 => Self::LinLog,
            4 => Self::LogLin,
            5 => Self::LogLog,
            _ => return None,
        })
    }
}

/// OpenMC's `Tabulated1D`: piecewise `y(x)` with per-region interpolation,
/// held constant outside its range.
#[derive(Debug, Clone, PartialEq)]
pub struct Tabulated1D {
    x: Vec<f64>,
    y: Vec<f64>,
    /// Region upper breakpoints as 0-based point indices (upstream subtracts 1
    /// from the file's 1-based `breakpoints`).
    nbt: Vec<usize>,
    interp: Vec<Interpolation>,
}

impl Tabulated1D {
    /// A table with the given regions (`nbt` already 0-based, `interp` per
    /// region). An empty `nbt` means one lin-lin region, as upstream.
    ///
    /// # Errors
    ///
    /// Fewer than two points, `x` and `y` of different length, or `nbt` and
    /// `interp` of different length.
    pub fn new(x: Vec<f64>, y: Vec<f64>, nbt: Vec<usize>, interp: Vec<Interpolation>) -> Result<Self, &'static str> {
        if x.len() < 2 || x.len() != y.len() {
            return Err("Tabulated1D needs at least two points and equal x and y lengths");
        }
        if nbt.len() != interp.len() {
            return Err("Tabulated1D needs one interpolation law per breakpoint");
        }
        Ok(Self { x, y, nbt, interp })
    }

    /// A lin-lin table.
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub fn lin_lin(x: Vec<f64>, y: Vec<f64>) -> Result<Self, &'static str> {
        Self::new(x, y, Vec::new(), Vec::new())
    }

    /// The abscissae.
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// The ordinates.
    pub fn y(&self) -> &[f64] {
        &self.y
    }

    /// `Tabulated1D::operator()` (`src/endf.cpp:243-299`).
    pub fn eval(&self, x: f64) -> f64 {
        let n = self.x.len();
        if x < self.x[0] {
            return self.y[0];
        } else if x > self.x[n - 1] {
            return self.y[n - 1];
        }
        let i = lower_bound_index(&self.x, x);

        let mut interp = Interpolation::LinLin;
        if !self.nbt.is_empty() {
            interp = self.interp[0];
            for j in 0..self.nbt.len() {
                if i < self.nbt[j] {
                    interp = self.interp[j];
                    break;
                }
            }
        }
        if interp == Interpolation::Histogram {
            return self.y[i];
        }
        let (x0, x1, y0, y1) = (self.x[i], self.x[i + 1], self.y[i], self.y[i + 1]);
        match interp {
            Interpolation::LinLin => {
                let r = (x - x0) / (x1 - x0);
                y0 + r * (y1 - y0)
            }
            Interpolation::LinLog => {
                let r = RealMath::r_ln(x / x0) / RealMath::r_ln(x1 / x0);
                y0 + r * (y1 - y0)
            }
            Interpolation::LogLin => {
                let r = (x - x0) / (x1 - x0);
                y0 * RealMath::r_exp(r * RealMath::r_ln(y1 / y0))
            }
            Interpolation::LogLog => {
                let r = RealMath::r_ln(x / x0) / RealMath::r_ln(x1 / x0);
                y0 * RealMath::r_exp(r * RealMath::r_ln(y1 / y0))
            }
            Interpolation::Histogram => y0,
        }
    }
}

// ── data ────────────────────────────────────────────────────────────────────

/// A row-major matrix (`tensor::Tensor<double>` with two dimensions).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Matrix {
    data: Vec<f64>,
    cols: usize,
}

impl Matrix {
    /// A matrix of `rows` rows of `cols` columns from row-major `data`.
    ///
    /// # Errors
    ///
    /// `data.len()` is not a multiple of `cols`.
    pub fn new(data: Vec<f64>, cols: usize) -> Result<Self, &'static str> {
        if cols == 0 {
            if data.is_empty() {
                return Ok(Self { data, cols });
            }
            return Err("a matrix with data needs at least one column");
        }
        if data.len() % cols != 0 {
            return Err("matrix data is not a whole number of rows");
        }
        Ok(Self { data, cols })
    }

    /// Row `i` (`tensor::slice(i)`).
    pub fn row(&self, i: usize) -> &[f64] {
        &self.data[i * self.cols..(i + 1) * self.cols]
    }

    /// Element `(i, j)`.
    pub fn at(&self, i: usize, j: usize) -> f64 {
        self.data[i * self.cols + j]
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        if self.cols == 0 { 0 } else { self.data.len() / self.cols }
    }
}

/// One atomic-relaxation transition (`ElectronSubshell::Transition`).
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// Index in `shells` of the originating subshell.
    pub primary_subshell: i32,
    /// Index in `shells` of the Auger electron's subshell, `-1` for a radiative
    /// transition.
    pub secondary_subshell: i32,
    /// Transition energy, eV.
    pub energy: f64,
    /// Probability of this transition.
    pub probability: f64,
}

/// `ElectronSubshell` (`photon.h`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ElectronSubshell {
    /// Index in `SUBSHELLS`.
    pub index_subshell: i32,
    /// Photoionisation threshold index.
    pub threshold: i32,
    /// Binding energy, eV.
    pub binding_energy: f64,
    /// Transitions that fill a vacancy here.
    pub transitions: Vec<Transition>,
}

/// Photon interaction data for one element (`PhotonInteraction`,
/// `photon.h:40-120`). Fields are public: this is a data record and the
/// loader (HDF5) is not part of this module.
#[derive(Debug, Clone, Default)]
pub struct PhotonInteraction {
    /// Element name, e.g. "Zr".
    pub name: String,
    /// Atomic number.
    pub z: usize,
    /// Index in the global element list.
    pub index: usize,

    /// Natural log of the energy grid, ln(eV) (`energy_`).
    pub energy: Vec<f64>,
    /// ln of the coherent cross section, ln(b).
    pub coherent: Vec<f64>,
    /// ln of the incoherent cross section.
    pub incoherent: Vec<f64>,
    /// ln of the total photoelectric cross section.
    pub photoelectric_total: Vec<f64>,
    /// ln of the pair-production total.
    pub pair_production_total: Vec<f64>,
    /// ln of the pair production in the electron field.
    pub pair_production_electron: Vec<f64>,
    /// ln of the pair production in the nuclear field.
    pub pair_production_nuclear: Vec<f64>,
    /// Heating cross section.
    pub heating: Vec<f64>,

    /// Incoherent scattering function S(x, Z).
    pub incoherent_form_factor: Option<Tabulated1D>,
    /// Integrated coherent form factor.
    pub coherent_int_form_factor: Option<Tabulated1D>,
    /// Real anomalous scattering factor.
    pub coherent_anomalous_real: Option<Tabulated1D>,
    /// Imaginary anomalous scattering factor.
    pub coherent_anomalous_imag: Option<Tabulated1D>,

    /// Subshells with atomic relaxation data.
    pub shells: Vec<ElectronSubshell>,
    /// ln subshell photoionisation cross sections, row per energy-grid point,
    /// column per subshell (`cross_sections_`).
    pub cross_sections: Matrix,

    /// Compton-profile momentum grid (`data::compton_profile_pz`, shared by every
    /// element upstream).
    pub profile_pz: Arc<[f64]>,
    /// Compton profile pdf, row per shell, column per momentum point.
    pub profile_pdf: Matrix,
    /// Compton profile cdf, same shape.
    pub profile_cdf: Matrix,
    /// Exponential tail slope per shell.
    pub profile_tail_slope: Vec<f64>,
    /// Profile mass from -1/alpha to 0 per shell.
    pub profile_negative_mass: Vec<f64>,
    /// Binding energy per Compton-profile shell, eV.
    pub binding_energy: Vec<f64>,
    /// Electron occupancy (pdf over shells).
    pub electron_pdf: Vec<f64>,
    /// Map from Compton-profile shells to ENDF relaxation shells.
    pub subshell_map: Vec<i32>,

    /// Mean excitation energy, eV.
    pub i_mean: f64,
    /// Electrons per subshell.
    pub n_electrons: Vec<i32>,
    /// Ionisation energy per subshell.
    pub ionization_energy: Vec<f64>,
    /// Radiative stopping power.
    pub stopping_power_radiative: Vec<f64>,
    /// Bremsstrahlung scaled DCS.
    pub dcs: Matrix,
    /// Whether atomic relaxation data is present.
    pub has_atomic_relaxation: bool,
}

/// Microscopic photon cross sections at one energy (`Particle::photon_xs`
/// element, `particle_data.h`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhotonMicroXs {
    /// Index of the lower grid point.
    pub index_grid: usize,
    /// Interpolation factor in ln E.
    pub interp_factor: f64,
    /// Coherent, b.
    pub coherent: f64,
    /// Incoherent, b.
    pub incoherent: f64,
    /// Photoelectric, b.
    pub photoelectric: f64,
    /// Pair production, b.
    pub pair_production: f64,
    /// Total, b.
    pub total: f64,
    /// The energy these were evaluated at, eV.
    pub last_e: f64,
}

/// What atomic relaxation would have created (the seam, see the module docs).
#[derive(Debug, Clone, PartialEq)]
pub enum RelaxationSecondary {
    /// A fluorescent photon (`p.create_secondary(p.wgt(), u, E, photon)`).
    Photon {
        /// Direction cosines.
        u: (f64, f64, f64),
        /// Energy, eV.
        energy: f64,
    },
    /// An Auger or Coster-Kronig electron
    /// (`process_charged_secondary(p, u, E, electron)`).
    Electron {
        /// Direction cosines.
        u: (f64, f64, f64),
        /// Energy, eV.
        energy: f64,
    },
}

/// Kaltiaisenaho shell kinematics (`ShellKinematics`, `photon.h:122-126`).
#[derive(Debug, Clone, Copy, Default)]
struct ShellKinematics {
    pz_max: f64,
    c_limit: f64,
    profile_mass: f64,
}

/// PENELOPE-2011 reduced screening radii (`pair_production`, `photon.cpp:799`),
/// ported verbatim; see the module docs for the indexing finding.
const SCREENING_RADIUS: [f64; 99] = [
    122.81, 73.167, 69.228, 67.301, 64.696, 61.228,
    57.524, 54.033, 50.787, 47.851, 46.373, 45.401,
    44.503, 43.815, 43.074, 42.321, 41.586, 40.953,
    40.524, 40.256, 39.756, 39.144, 38.462, 37.778,
    37.174, 36.663, 35.986, 35.317, 34.688, 34.197,
    33.786, 33.422, 33.068, 32.740, 32.438, 32.143,
    31.884, 31.622, 31.438, 31.142, 30.950, 30.758,
    30.561, 30.285, 30.097, 29.832, 29.581, 29.411,
    29.247, 29.085, 28.930, 28.721, 28.580, 28.442,
    28.312, 28.139, 27.973, 27.819, 27.675, 27.496,
    27.285, 27.093, 26.911, 26.705, 26.516, 26.304,
    26.108, 25.929, 25.730, 25.577, 25.403, 25.245,
    25.100, 24.941, 24.790, 24.655, 24.506, 24.391,
    24.262, 24.145, 24.039, 23.922, 23.813, 23.712,
    23.621, 23.523, 23.430, 23.331, 23.238, 23.139,
    23.048, 22.967, 22.833, 22.694, 22.624, 22.545,
    22.446, 22.358, 22.264,
];

impl PhotonInteraction {
    fn profile_n(&self) -> usize {
        self.profile_pz.len()
    }

    /// `calc_max_stack_size` (`photon.cpp:417-430`): the largest vacancy stack
    /// atomic relaxation can need for this element.
    pub fn calc_max_stack_size(&self) -> i32 {
        let mut visited: Vec<Option<i32>> = alloc::vec![None; self.shells.len()];
        let mut max_size = 0;
        for i_shell in 0..self.shells.len() {
            max_size = max_size.max(self.calc_helper(&mut visited, i_shell));
        }
        max_size
    }

    /// `calc_helper` (`photon.cpp:432-463`); `visited` replaces the
    /// `unordered_map<int, int>`.
    fn calc_helper(&self, visited: &mut Vec<Option<i32>>, i_shell: usize) -> i32 {
        let shell = &self.shells[i_shell];
        if shell.transitions.is_empty() {
            return 1;
        }
        if let Some(v) = visited[i_shell] {
            return v;
        }
        let mut max_size = 0;
        for transition in &shell.transitions {
            let mut size = 0;
            if transition.secondary_subshell != -1 {
                size = self.calc_helper(visited, transition.secondary_subshell as usize) + 1;
            }
            size = size.max(self.calc_helper(visited, transition.primary_subshell as usize));
            max_size = max_size.max(size);
        }
        visited[i_shell] = Some(max_size);
        max_size
    }

    /// `compton_scatter` (`photon.cpp:465-498`): `(alpha_out, mu, i_shell)`,
    /// with `i_shell = -1` when Doppler broadening is off.
    ///
    /// # Panics
    ///
    /// If `incoherent_form_factor` is missing (data not loaded).
    pub fn compton_scatter(&self, alpha: f64, doppler: bool, seed: &mut u64) -> (f64, f64, i32) {
        let ff = self.incoherent_form_factor.as_ref().expect("incoherent form factor loaded");
        let mut form_factor_xmax = 0.0;
        loop {
            let (mut alpha_out, mu) = klein_nishina(alpha, seed);
            let x = MASS_ELECTRON_EV / PLANCK_C * alpha * Real::sqrt(0.5 * (1.0 - mu));
            let form_factor_x = ff.eval(x);
            if form_factor_xmax == 0.0 {
                form_factor_xmax = ff.eval(MASS_ELECTRON_EV / PLANCK_C * alpha);
            }
            if prn(seed) < form_factor_x / form_factor_xmax {
                let i_shell;
                if doppler {
                    let (e_out, shell) = self.compton_doppler(alpha, mu, seed);
                    i_shell = shell;
                    alpha_out = e_out / MASS_ELECTRON_EV;
                } else {
                    i_shell = -1;
                }
                return (alpha_out, mu, i_shell);
            }
        }
    }

    /// `compton_profile_cdf` (`photon.cpp:500-525`): `K_i(pz)`, capped at 1/2.
    pub fn compton_profile_cdf(&self, i_shell: usize, pz: f64) -> f64 {
        if pz <= 0.0 {
            return 0.0;
        }
        let n = self.profile_n();
        let pz_last = self.profile_pz[n - 1];
        let c;
        if pz >= pz_last {
            c = self.profile_cdf.at(i_shell, n - 1)
                + detail::compton_profile_tail_integral(
                    pz,
                    pz_last,
                    self.profile_pdf.at(i_shell, n - 1),
                    self.profile_tail_slope[i_shell],
                );
        } else {
            let i = lower_bound_index(&self.profile_pz, pz);
            let pz_l = self.profile_pz[i];
            let pz_r = self.profile_pz[i + 1];
            let p_l = self.profile_pdf.at(i_shell, i);
            let p_r = self.profile_pdf.at(i_shell, i + 1);
            let c_l = self.profile_cdf.at(i_shell, i);
            let slope = (p_r - p_l) / (pz_r - pz_l);
            let delta = pz - pz_l;
            c = c_l + p_l * delta + 0.5 * slope * delta * delta;
        }
        c.min(0.5)
    }

    /// `invert_compton_profile_cdf` (`photon.cpp:527-559`).
    pub fn invert_compton_profile_cdf(&self, i_shell: usize, c: f64) -> f64 {
        let n = self.profile_n();
        let integral = c;
        let c_last = self.profile_cdf.at(i_shell, n - 1);
        if integral >= c_last {
            return detail::invert_compton_profile_tail(
                integral - c_last,
                self.profile_pz[n - 1],
                self.profile_pdf.at(i_shell, n - 1),
                self.profile_tail_slope[i_shell],
            );
        }
        let cdf_shell = self.profile_cdf.row(i_shell);
        let i = lower_bound_index(cdf_shell, integral);
        let pz_l = self.profile_pz[i];
        let pz_r = self.profile_pz[i + 1];
        let p_l = self.profile_pdf.at(i_shell, i);
        let p_r = self.profile_pdf.at(i_shell, i + 1);
        let c_l = self.profile_cdf.at(i_shell, i);
        if p_l == p_r {
            return pz_l + (integral - c_l) / p_l;
        }
        let slope = (p_r - p_l) / (pz_r - pz_l);
        let delta_c = integral - c_l;
        let discriminant = p_l * p_l + 2.0 * slope * delta_c;
        let denominator = p_l + Real::sqrt(discriminant.max(0.0));
        pz_l + 2.0 * delta_c / denominator
    }

    /// `compton_shell_kinematics` (`photon.cpp:561-584`).
    fn compton_shell_kinematics(&self, alpha: f64, mu: f64, e: f64, i_shell: usize) -> ShellKinematics {
        let mut k = ShellKinematics::default();
        let e_b = self.binding_energy[i_shell];
        if e <= e_b {
            return k;
        }
        k.pz_max = -FINE_STRUCTURE * (e_b - (e - e_b) * alpha * (1.0 - mu))
            / Real::sqrt(2.0 * e * (e - e_b) * (1.0 - mu) + e_b * e_b);
        if k.pz_max <= -FINE_STRUCTURE {
            return k;
        }
        let c_negative = self.profile_negative_mass[i_shell];
        k.c_limit = self.compton_profile_cdf(i_shell, k.pz_max.abs());
        k.profile_mass = c_negative + k.c_limit.copysign(k.pz_max);
        k
    }

    /// `sample_compton_momentum` (`photon.cpp:586-623`): `Some(E_out)` when the
    /// sample is accepted.
    fn sample_compton_momentum(
        &self,
        alpha: f64,
        mu: f64,
        e: f64,
        i_shell: usize,
        k: &ShellKinematics,
        seed: &mut u64,
    ) -> Option<f64> {
        let c_negative = self.profile_negative_mass[i_shell];
        let pz;
        if k.pz_max < 0.0 {
            let c = k.c_limit + prn(seed) * k.profile_mass;
            pz = -self.invert_compton_profile_cdf(i_shell, c);
        } else {
            let c = prn(seed) * k.profile_mass;
            if c < c_negative {
                pz = -self.invert_compton_profile_cdf(i_shell, c_negative - c);
            } else {
                pz = self.invert_compton_profile_cdf(i_shell, c - c_negative);
            }
        }
        let mut energy_ratio = detail::compton_energy_ratio(alpha, mu, pz);
        let max_energy_ratio = 1.0 - self.binding_energy[i_shell] / e;
        if !energy_ratio.is_finite() || energy_ratio <= 0.0 {
            return None;
        }
        let energy_tolerance = 16.0 * f64::EPSILON * 1.0_f64.max(max_energy_ratio);
        if energy_ratio > max_energy_ratio + energy_tolerance {
            return None;
        }
        energy_ratio = energy_ratio.min(max_energy_ratio);
        let e_out = energy_ratio * e;
        if prn(seed) <= energy_ratio {
            Some(e_out)
        } else {
            None
        }
    }

    /// `compton_doppler_conditional` (`photon.cpp:625-661`): `(E_out, shell)`.
    fn compton_doppler_conditional(&self, alpha: f64, mu: f64, e: f64, seed: &mut u64) -> Option<(f64, i32)> {
        let n = self.electron_pdf.len();
        let mut shell_data = [ShellKinematics::default(); N_SUBSHELLS];
        let mut shell_cdf = [0.0_f64; N_SUBSHELLS];
        let mut shell_pmf_norm = 0.0;
        for i in 0..n {
            shell_data[i] = self.compton_shell_kinematics(alpha, mu, e, i);
            shell_pmf_norm += self.electron_pdf[i] * shell_data[i].profile_mass;
            shell_cdf[i] = shell_pmf_norm;
        }
        if shell_pmf_norm == 0.0 {
            return None;
        }
        const MAX_SAMPLES: i32 = 100000;
        for _ in 0..MAX_SAMPLES {
            let rn = prn(seed) * shell_pmf_norm;
            let mut shell = 0;
            while shell < n {
                if rn < shell_cdf[shell] {
                    break;
                }
                shell += 1;
            }
            // Upstream does not bound `shell` here; roundoff can leave it at
            // `n`. Keep it in range rather than index past the arrays.
            let shell_i = shell.min(n.saturating_sub(1));
            if let Some(e_out) = self.sample_compton_momentum(alpha, mu, e, shell_i, &shell_data[shell_i], seed) {
                return Some((e_out, shell_i as i32));
            }
        }
        None
    }

    /// `compton_doppler` (`photon.cpp:663-709`): `(E_out, i_shell)`.
    fn compton_doppler(&self, alpha: f64, mu: f64, seed: &mut u64) -> (f64, i32) {
        const N_FAST_SAMPLES: i32 = 2;
        let n = self.electron_pdf.len();
        let e = alpha * MASS_ELECTRON_EV;
        let mut shell = 0usize;
        for _ in 0..N_FAST_SAMPLES {
            let rn = prn(seed);
            let mut c = 0.0;
            shell = 0;
            while shell < n {
                c += self.electron_pdf[shell];
                if rn < c {
                    break;
                }
                shell += 1;
            }
            let shell_i = shell.min(n.saturating_sub(1));
            let k = self.compton_shell_kinematics(alpha, mu, e, shell_i);
            if k.profile_mass <= 0.0 {
                continue;
            }
            if prn(seed) >= k.profile_mass {
                continue;
            }
            if let Some(e_out) = self.sample_compton_momentum(alpha, mu, e, shell_i, &k, seed) {
                return (e_out, shell_i as i32);
            }
        }
        let mut i_shell = shell as i32;
        if let Some((e_out, s)) = self.compton_doppler_conditional(alpha, mu, e, seed) {
            i_shell = s;
            return (e_out, i_shell);
        }
        (alpha / (1.0 + alpha * (1.0 - mu)) * MASS_ELECTRON_EV, i_shell)
    }

    /// `calculate_xs` (`photon.cpp:711-767`) for a photon of energy `e` (eV).
    pub fn calculate_xs(&self, e: f64) -> PhotonMicroXs {
        let n_grid = self.energy.len();
        let log_e = RealMath::r_ln(e);
        let mut i_grid;
        if log_e <= self.energy[0] {
            i_grid = 0;
        } else if log_e > self.energy[n_grid - 1] {
            i_grid = n_grid - 2;
        } else {
            i_grid = upper_bound_index(&self.energy, log_e);
        }
        if self.energy[i_grid] == self.energy[i_grid + 1] {
            i_grid += 1;
        }
        let f = (log_e - self.energy[i_grid]) / (self.energy[i_grid + 1] - self.energy[i_grid]);

        let mut xs = PhotonMicroXs { index_grid: i_grid, interp_factor: f, ..Default::default() };
        xs.coherent = RealMath::r_exp(self.coherent[i_grid] + f * (self.coherent[i_grid + 1] - self.coherent[i_grid]));
        xs.incoherent =
            RealMath::r_exp(self.incoherent[i_grid] + f * (self.incoherent[i_grid + 1] - self.incoherent[i_grid]));
        xs.photoelectric = 0.0;
        if self.cross_sections.cols() > 0 {
            let lower = self.cross_sections.row(i_grid);
            let upper = self.cross_sections.row(i_grid + 1);
            for i in 0..upper.len() {
                if lower[i] != 0.0 {
                    xs.photoelectric += RealMath::r_exp(lower[i] + f * (upper[i] - lower[i]));
                }
            }
        }
        xs.pair_production = RealMath::r_exp(
            self.pair_production_total[i_grid]
                + f * (self.pair_production_total[i_grid + 1] - self.pair_production_total[i_grid]),
        );
        xs.total = xs.coherent + xs.incoherent + xs.photoelectric + xs.pair_production;
        xs.last_e = e;
        xs
    }

    /// `rayleigh_scatter` (`photon.cpp:769-796`): the scattering cosine.
    ///
    /// # Panics
    ///
    /// If `coherent_int_form_factor` is missing (data not loaded).
    pub fn rayleigh_scatter(&self, alpha: f64, seed: &mut u64) -> f64 {
        let ff = self.coherent_int_form_factor.as_ref().expect("coherent form factor loaded");
        loop {
            let x2_max = RealMath::r_powf(MASS_ELECTRON_EV / PLANCK_C * alpha, 2.0);
            let f_max = ff.eval(x2_max);
            let f = prn(seed) * f_max;
            let x = ff.x();
            let y = ff.y();
            let i = lower_bound_index(y, f);
            let r = (f - y[i]) / (y[i + 1] - y[i]);
            let x2 = x[i] + r * (x[i + 1] - x[i]);
            let mu = 1.0 - 2.0 * x2 / x2_max;
            if prn(seed) < 0.5 * (1.0 + mu * mu) {
                return mu;
            }
        }
    }

    /// `pair_production` (`photon.cpp:798-922`):
    /// `(E_electron, E_positron, mu_electron, mu_positron)`.
    pub fn pair_production(&self, alpha: f64, seed: &mut u64) -> (f64, f64, f64, f64) {
        let z = self.z;
        let r_z = SCREENING_RADIUS[z.min(SCREENING_RADIUS.len() - 1)];
        let zf = z as f64;

        // High-energy Coulomb correction.
        let a = zf / FINE_STRUCTURE;
        let a2 = a * a;
        let c = a2
            * (1.0 / (1.0 + a2)
                + 0.202059
                + a2 * (-0.03693
                    + a2 * (0.00835 + a2 * (-0.00201 + a2 * (0.00049 + a2 * (-0.00012 + a2 * 0.00003))))));

        // Low-energy correction factor.
        let q = Real::sqrt(2.0 / alpha);
        let f = q * (-0.1774 - 12.10 * a + 11.18 * a * a)
            + q * q * (8.523 + 73.26 * a - 44.41 * a * a)
            + q * q * q * (-13.52 - 121.1 * a + 96.41 * a * a)
            + q * q * q * q * (8.946 + 62.05 * a - 63.41 * a * a);

        let mut b = 2.0 * r_z / alpha;
        let mut t1 = 2.0 * RealMath::r_ln(1.0 + b * b);
        let mut t2 = b * RealMath::r_atan(1.0 / b);
        let mut t3 = b * b * (4.0 - 4.0 * t2 - 3.0 * RealMath::r_ln(1.0 + 1.0 / (b * b)));
        let t4 = 4.0 * RealMath::r_ln(r_z) - 4.0 * c + f;
        let phi1_max = 7.0 / 3.0 - t1 - 6.0 * t2 - t3 + t4;
        let phi2_max = 11.0 / 6.0 - t1 - 3.0 * t2 + 0.5 * t3 + t4;

        let u1 = 2.0 / 3.0 * RealMath::r_powf(0.5 - 1.0 / alpha, 2.0) * phi1_max;
        let u2 = phi2_max;
        let mut e;
        loop {
            let rn = prn(seed);
            let i;
            if prn(seed) < u1 / (u1 + u2) {
                i = 1;
                e = if rn >= 0.5 {
                    0.5 + (0.5 - 1.0 / alpha) * RealMath::r_powf(2.0 * rn - 1.0, 1.0 / 3.0)
                } else {
                    0.5 - (0.5 - 1.0 / alpha) * RealMath::r_powf(1.0 - 2.0 * rn, 1.0 / 3.0)
                };
            } else {
                i = 2;
                e = 1.0 / alpha + (0.5 - 1.0 / alpha) * 2.0 * rn;
            }
            b = r_z / (2.0 * alpha * e * (1.0 - e));
            t1 = 2.0 * RealMath::r_ln(1.0 + b * b);
            t2 = b * RealMath::r_atan(1.0 / b);
            t3 = b * b * (4.0 - 4.0 * t2 - 3.0 * RealMath::r_ln(1.0 + 1.0 / (b * b)));
            if i == 1 {
                let phi1 = 7.0 / 3.0 - t1 - 6.0 * t2 - t3 + t4;
                if prn(seed) <= phi1 / phi1_max {
                    break;
                }
            } else {
                let phi2 = 11.0 / 6.0 - t1 - 3.0 * t2 + 0.5 * t3 + t4;
                if prn(seed) <= phi2 / phi2_max {
                    break;
                }
            }
        }

        let e_electron = (alpha * e - 1.0) * MASS_ELECTRON_EV;
        let e_positron = (alpha * (1.0 - e) - 1.0) * MASS_ELECTRON_EV;

        let mut beta = Real::sqrt(e_electron * (e_electron + 2.0 * MASS_ELECTRON_EV)) / (e_electron + MASS_ELECTRON_EV);
        let mut rn = uniform_distribution(-1.0, 1.0, seed);
        let mu_electron = (rn + beta) / (rn * beta + 1.0);

        beta = Real::sqrt(e_positron * (e_positron + 2.0 * MASS_ELECTRON_EV)) / (e_positron + MASS_ELECTRON_EV);
        rn = uniform_distribution(-1.0, 1.0, seed);
        let mu_positron = (rn + beta) / (rn * beta + 1.0);
        (e_electron, e_positron, mu_electron, mu_positron)
    }

    /// `atomic_relaxation` (`photon.cpp:924-997`).
    ///
    /// Upstream reads the photon cutoff from `settings::energy_cutoff` and the
    /// particle energy from `Particle`; both are parameters here
    /// (`photon_energy_cutoff`, `particle_energy`, eV). The secondaries
    /// upstream creates are returned in order.
    pub fn atomic_relaxation(
        &self,
        i_shell: usize,
        particle_energy: f64,
        photon_energy_cutoff: f64,
        seed: &mut u64,
    ) -> Vec<RelaxationSecondary> {
        let mut out = Vec::new();
        self.atomic_relaxation_with(i_shell, particle_energy, photon_energy_cutoff, seed, |s, _| out.push(s));
        out
    }

    /// [`atomic_relaxation`](Self::atomic_relaxation) with each secondary handed
    /// to `on_secondary` **at the point upstream creates it**, together with the
    /// seed. This matters: upstream processes an Auger electron
    /// (`process_charged_secondary`, which may draw random numbers for
    /// thick-target bremsstrahlung) in the middle of the cascade, so those
    /// draws interleave with the cascade's own. Collecting the list first and
    /// processing it afterwards (what `atomic_relaxation` does) reorders the
    /// stream; found by the differential test against compiled OpenMC
    /// (gh:#1690, 2026-11-21).
    pub fn atomic_relaxation_with<F: FnMut(RelaxationSecondary, &mut u64)>(
        &self,
        i_shell: usize,
        particle_energy: f64,
        photon_energy_cutoff: f64,
        seed: &mut u64,
        mut on_secondary: F,
    ) {
        if !self.has_atomic_relaxation || self.shells[i_shell].binding_energy > particle_energy {
            return;
        }
        let mut n_holes = 0usize;
        let mut holes = [0usize; MAX_STACK_SIZE];
        holes[n_holes] = i_shell;
        n_holes += 1;

        while n_holes > 0 {
            n_holes -= 1;
            let i_hole = holes[n_holes];
            let shell = &self.shells[i_hole];
            if shell.binding_energy < photon_energy_cutoff {
                continue;
            }
            if shell.transitions.is_empty() {
                let u = isotropic_direction(seed);
                on_secondary(RelaxationSecondary::Photon { u, energy: shell.binding_energy }, seed);
                continue;
            }
            let mut c = -prn(seed);
            let mut i_trans = 0usize;
            while i_trans < shell.transitions.len() {
                c += shell.transitions[i_trans].probability;
                if c > 0.0 {
                    break;
                }
                i_trans += 1;
            }
            // Upstream does not guard i_trans == len (probabilities summing
            // below the draw); keep it in range.
            let transition = &shell.transitions[i_trans.min(shell.transitions.len() - 1)];

            holes[n_holes] = transition.primary_subshell as usize;
            n_holes += 1;
            let auger = transition.secondary_subshell != -1;
            if auger {
                holes[n_holes] = transition.secondary_subshell as usize;
                n_holes += 1;
            }
            if transition.energy < photon_energy_cutoff {
                continue;
            }
            let u = isotropic_direction(seed);
            if auger {
                on_secondary(RelaxationSecondary::Electron { u, energy: transition.energy }, seed);
            } else {
                on_secondary(RelaxationSecondary::Photon { u, energy: transition.energy }, seed);
            }
        }
    }
}

/// `detail::` helpers of `photon.cpp:999-1066`.
pub mod detail {
    use super::{FINE_STRUCTURE, Real, RealMath};

    /// `compton_profile_tail_integral` (`:999-1003`).
    pub fn compton_profile_tail_integral(pz: f64, pz_last: f64, profile_last: f64, slope: f64) -> f64 {
        profile_last * RealMath::r_exp_m1(slope * (pz - pz_last)) / slope
    }

    /// `invert_compton_profile_tail` (`:1005-1009`).
    pub fn invert_compton_profile_tail(integral: f64, pz_last: f64, profile_last: f64, slope: f64) -> f64 {
        pz_last + RealMath::r_ln_1p(slope * integral / profile_last) / slope
    }

    /// `compton_energy_ratio` (`:1011-1066`): outgoing-to-incident energy ratio
    /// for signed electron momentum `pz`; NaN where upstream returns it.
    pub fn compton_energy_ratio(alpha: f64, mu: f64, pz: f64) -> f64 {
        if pz == 0.0 {
            return 1.0 / (1.0 + alpha * (1.0 - mu));
        }
        let momentum = pz / FINE_STRUCTURE;
        let momentum_sq = momentum * momentum;
        let f = 1.0 + alpha * (1.0 - mu);
        let a = momentum_sq - f * f;
        let b = 2.0 * (f - momentum_sq * mu);
        let c = momentum_sq - 1.0;
        let mut discriminant = b * b - 4.0 * a * c;
        let discriminant_tolerance = 16.0 * f64::EPSILON * (b * b + (4.0 * a * c).abs());
        if discriminant < -discriminant_tolerance {
            return f64::NAN;
        }
        discriminant = discriminant.max(0.0);

        let root1;
        let root2;
        if a.abs() < 1.0e-14 * (b.abs() + c.abs()) {
            if b == 0.0 {
                return f64::NAN;
            }
            root1 = -c / b;
            root2 = root1;
        } else {
            let sqrt_discriminant = Real::sqrt(discriminant);
            let q = -0.5 * (b + sqrt_discriminant.copysign(b));
            root1 = q / a;
            root2 = if q == 0.0 { (-b + sqrt_discriminant) / (2.0 * a) } else { c / q };
        }

        let mut root_min = f64::INFINITY;
        let mut root_max = f64::NEG_INFINITY;
        if root1.is_finite() && root1 > 0.0 {
            root_min = root1;
            root_max = root1;
        }
        if root2.is_finite() && root2 > 0.0 {
            root_min = root_min.min(root2);
            root_max = root_max.max(root2);
        }
        if !root_min.is_finite() {
            return f64::NAN;
        }

        let energy_ratio = if pz < 0.0 { root_min } else { root_max };
        let free_electron_ratio = 1.0 / f;
        let tolerance = 16.0 * f64::EPSILON * 1.0_f64.max(free_electron_ratio);
        if (pz < 0.0 && energy_ratio > free_electron_ratio + tolerance)
            || (pz > 0.0 && energy_ratio < free_electron_ratio - tolerance)
        {
            return f64::NAN;
        }
        energy_ratio
    }
}

/// `klein_nishina` (`photon.cpp:1068-1123`): `(alpha_out, mu)`.
pub fn klein_nishina(alpha: f64, seed: &mut u64) -> (f64, f64) {
    let alpha_out;
    let mu;
    let beta = 1.0 + 2.0 * alpha;
    if alpha < 3.0 {
        // Kahn's rejection method.
        let t = beta / (beta + 8.0);
        let mut x;
        let mut m;
        loop {
            if prn(seed) < t {
                let r = uniform_distribution(0.0, 2.0, seed);
                x = 1.0 + alpha * r;
                if prn(seed) < 4.0 / x * (1.0 - 1.0 / x) {
                    m = 1.0 - r;
                    break;
                }
            } else {
                x = beta / (1.0 + 2.0 * alpha * prn(seed));
                m = 1.0 + (1.0 - x) / alpha;
                if prn(seed) < 0.5 * (m * m + 1.0 / x) {
                    break;
                }
            }
        }
        mu = m;
        alpha_out = alpha / x;
    } else {
        // Koblinger's direct method.
        let gamma = 1.0 - RealMath::r_powi(beta, -2);
        let s = prn(seed)
            * (4.0 / alpha + 0.5 * gamma + (1.0 - (1.0 + beta) / (alpha * alpha)) * RealMath::r_ln(beta));
        let ao;
        if s <= 2.0 / alpha {
            ao = alpha / (1.0 + 2.0 * alpha * prn(seed));
        } else if s <= 4.0 / alpha {
            ao = alpha * (1.0 + 2.0 * alpha * prn(seed)) / beta;
        } else if s <= 4.0 / alpha + 0.5 * gamma {
            ao = alpha * Real::sqrt(1.0 - gamma * prn(seed));
        } else {
            ao = alpha / RealMath::r_powf(beta, prn(seed));
        }
        alpha_out = ao;
        mu = 1.0 + 1.0 / alpha - 1.0 / alpha_out;
    }
    (alpha_out, mu)
}
