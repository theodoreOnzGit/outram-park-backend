// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of OpenMC.
//   Upstream project: OpenMC, https://github.com/openmc-dev/openmc
//   Upstream files:   src/bremsstrahlung.cpp, include/openmc/bremsstrahlung.h
//   Upstream commit:  8f79d9d0
//   Upstream licence: MIT, Copyright (c) 2011-2025 Massachusetts Institute of
//                     Technology, UChicago Argonne LLC, and OpenMC contributors
//   This port:        AGPL-3.0-only, with the MIT notice preserved here.
//! **Thick-target bremsstrahlung (TTB)** from OpenMC (gh:#1683).
//!
//! **Status: first port, not differentially tested (gh:#1683).**
//!
//! Port of `thick_target_bremsstrahlung(Particle&, ParticleType, Direction,
//! double)` (`src/bremsstrahlung.cpp:34-119`). Upstream reads the material's
//! `ttb_` data and the global `data::ttb_e_grid`; here both are passed in
//! ([`BremsstrahlungData`], `e_grid`). Upstream calls `p.create_secondary`; here
//! the photons are returned as [`BremsPhoton`]s. The `p.material() ==
//! MATERIAL_VOID` early return is the caller's (`in_void`).
//!
//! No data is vendored: the tables come from OpenMC's `BREMX.DAT`-derived HDF5
//! files, read by a loader that is not part of this module.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::{Matrix, lower_bound_index};
use alloc::vec::Vec;
use petir::mathf::RealMath;
use petir::rng::lcg::prn;

/// `BremsstrahlungData` (`bremsstrahlung.h:14-18`).
#[derive(Debug, Clone, Default)]
pub struct BremsstrahlungData {
    /// Bremsstrahlung energy PDF, `[incident][photon]`.
    pub pdf: Matrix,
    /// Bremsstrahlung energy CDF.
    pub cdf: Matrix,
    /// ln photon yield per incident-energy grid point.
    pub yield_: Vec<f64>,
}

/// `Bremsstrahlung` (`bremsstrahlung.h:20-24`): per-material TTB data.
#[derive(Debug, Clone, Default)]
pub struct Bremsstrahlung {
    /// For electrons.
    pub electron: BremsstrahlungData,
    /// For positrons.
    pub positron: BremsstrahlungData,
}

/// One photon `thick_target_bremsstrahlung` creates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BremsPhoton {
    /// Energy, eV.
    pub energy: f64,
}

/// `thick_target_bremsstrahlung` (`bremsstrahlung.cpp:34-119`): the energies of
/// the photons emitted by a charged particle of kinetic energy `e` (eV).
/// Each photon is created with the charged particle's direction `u` and the
/// parent weight, which the caller applies.
///
/// `e_grid` is `data::ttb_e_grid` (ln eV), `data` is the material's table for
/// the particle's type, `photon_cutoff` is `settings::energy_cutoff[photon]`.
///
/// Where upstream would take the `ln` of a non-positive PDF ratio, the result is
/// NaN exactly as upstream's (no guard added).
pub fn thick_target_bremsstrahlung(
    e_grid: &[f64],
    data: &BremsstrahlungData,
    in_void: bool,
    photon_cutoff: f64,
    e: f64,
    seed: &mut u64,
) -> Vec<BremsPhoton> {
    let mut out = Vec::new();
    if in_void || e < photon_cutoff {
        return out;
    }
    let ln_e = RealMath::r_ln(e);
    let n_e = e_grid.len();

    let mut j = lower_bound_index(e_grid, ln_e);
    if j == n_e - 1 {
        j -= 1;
    }
    let e_l = e_grid[j];
    let e_r = e_grid[j + 1];
    let y_l = data.yield_[j];
    let y_r = data.yield_[j + 1];
    let f = (ln_e - e_l) / (e_r - e_l);
    let y = RealMath::r_exp(y_l + (y_r - y_l) * f);

    // `int n = y + prn(...)`: truncation toward zero.
    let n = (y + prn(seed)) as i32;
    if n == 0 {
        return out;
    }
    let mut e_radiated = 0.0;

    let c_max;
    let i_e;
    if prn(seed) <= f || j == 0 {
        i_e = j + 1;
        let p_l = data.pdf.at(i_e, i_e - 1);
        let p_r = data.pdf.at(i_e, i_e);
        let c_l = data.cdf.at(i_e, i_e - 1);
        let a = RealMath::r_ln(p_r / p_l) / (e_r - e_l) + 1.0;
        c_max = c_l + RealMath::r_exp(e_l) * p_l / a * (RealMath::r_exp(a * (ln_e - e_l)) - 1.0);
    } else {
        i_e = j;
        c_max = data.cdf.at(i_e, i_e);
    }

    for _ in 0..n {
        let c = prn(seed) * c_max;
        // lower_bound_index over cdf(i_e, 0 .. i_e)
        let i_w = lower_bound_index(&data.cdf.row(i_e)[..i_e], c);
        let w_l = e_grid[i_w];
        let w_r = e_grid[i_w + 1];
        let p_l = data.pdf.at(i_e, i_w);
        let p_r = data.pdf.at(i_e, i_w + 1);
        let c_l = data.cdf.at(i_e, i_w);
        let a = RealMath::r_ln(p_r / p_l) / (w_r - w_l) + 1.0;
        let mut w = RealMath::r_exp(w_l)
            * RealMath::r_powf(a * (c - c_l) / (RealMath::r_exp(w_l) * p_l) + 1.0, 1.0 / a);
        if w > photon_cutoff {
            if e_radiated + w > e {
                w = e - e_radiated;
            }
            out.push(BremsPhoton { energy: w });
            e_radiated += w;
        }
    }
    out
}
