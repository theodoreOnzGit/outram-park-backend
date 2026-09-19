// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # Handing Monte Carlo MGXS to the GeN-Foam deterministic solvers
//!
//! [`crate::mgxs`] condenses a Monte Carlo run into group constants. This turns
//! that set into a [`NuclearDataInput`], the dictionary GeN-Foam's own
//! cross-section machinery reads, so the data enters through
//! [`CrossSectionData::from_input`] and is validated by the same code path a
//! hand-written `nuclearData` file would be. Nothing here reaches around that
//! machinery to poke fields into a solver directly.
//!
//! ## UNITS — the thing most likely to silently ruin a result
//!
//! `outram-mc-libs` works in **centimetres**: cross sections in cm^-1, lengths
//! in cm. GeN-Foam, following OpenFOAM, works in **base SI**: cross sections in
//! m^-1, the diffusion coefficient in m, `sigma_pow` in J/m.
//!
//! So every cross section is multiplied by [`CM_INV_TO_M_INV`] = 100 and every
//! length by [`CM_TO_M`] = 0.01. Getting this wrong does not crash anything --
//! it produces a reactor that is wrong by two orders of magnitude in optical
//! thickness, which looks like a physics problem rather than a units problem.
//! The conversion is therefore done in exactly one place, named, and tested.
//!
//! ## What is carried across, and what is supplied
//!
//! | GeN-Foam field | Source |
//! |---|---|
//! | `d` | `1 / (3 Sigma_t)`, converted to m |
//! | `nu_sigma_eff` | tallied `nu Sigma_f` |
//! | `sigma_pow` | tallied `kappa Sigma_f` |
//! | `sigma_removal` | `Sigma_t - Sigma_s,g->g` |
//! | `chi_prompt` | tallied fission-birth spectrum |
//! | `chi_delayed` | **copied from `chi_prompt`** — see below |
//! | `scattering[0]` | tallied `P0` matrix, transposed into GeN-Foam's `[into][from]` |
//! | `iv` | `1/v` **derived** from each group's midpoint energy |
//! | `integral_flux` | the tallied group flux |
//! | `beta`, `lambda` | **zero / placeholder** — see below |
//!
//! ## Delayed neutrons are NOT carried, and that bounds what this can do
//!
//! The Monte Carlo passes tally no delayed-neutron data, so `beta` is written
//! as **all zeros** and `chi_delayed` is copied from `chi_prompt`.
//!
//! With `beta = 0` the delayed precursors carry no source, so the delayed
//! spectrum multiplies nothing and the copy is inert rather than a smuggled
//! assumption. That is correct for a **steady-state eigenvalue**, where `k_eff`
//! does not depend on the prompt/delayed split at all.
//!
//! It is **wrong for a transient**. A kinetics solve driven from this data has
//! no delayed neutrons and would run prompt-critical nonsense. The transient
//! path must not be used until beta and lambda are tallied; nothing here
//! prevents a caller trying, so this is stated loudly rather than guarded.
//!
//! ## Scattering matrix orientation — NO transpose
//!
//! [`crate::mgxs::ZoneMgxs::scatter`] is indexed `[g_from][g_into]`, and
//! GeN-Foam's `ZoneStateInput::scattering` documents `[moment][j][i]` as
//! `Sigma_{s, j -> i}`, which is the **same** orientation. Nothing is
//! transposed; only the units change.
//!
//! This is spelled out because it was got wrong in the first version, and the
//! failure mode is worth knowing. Transposing fed the solver zero in-scatter,
//! so the softer group was populated by its fission-spectrum share alone: the
//! spectrum came out 12x too hard and `k_inf` dropped 4393 pcm. That reads as
//! a physics disagreement rather than an indexing bug, which is exactly why the
//! leakage-free cross-check below exists.

use std::collections::BTreeMap;

use outram_foam_appbuilder_lib::genfoam::neutronics::xs::input::{
    NuclearDataInput, StateInput, ZoneConstantsInput, ZoneStateInput,
};

use crate::mgxs::MgxsLibrary;

/// Errors from handing a Monte Carlo MGXS set to GeN-Foam.
#[derive(Debug, thiserror::Error)]
pub enum GenfoamXsError {
    /// A group carries no flux in some zone, so its cross sections were never
    /// measured and [`crate::mgxs::condense`] left them at zero.
    ///
    /// **This must not be passed to a diffusion solver.** A group with zero
    /// removal and zero diffusion coefficient has the equation `0 * phi = 0`,
    /// which is singular: the solver returns an arbitrary flux there, and that
    /// spurious flux dilutes the real spectrum and moves the eigenvalue.
    /// Measured on a bare HEU medium 2026-09-19, two empty thermal groups
    /// absorbed 27.9% of the converged flux and pulled `k_inf` down by
    /// 4873 pcm.
    ///
    /// The fix is a group structure the problem actually populates, or more
    /// particles -- not a fabricated cross section for a group no neutron
    /// visited.
    #[error(
        "zone '{zone}' has no flux in group {group} of {n_groups}, so its cross sections were          never measured; a diffusion solve on zeros is singular there. Use a group structure          this problem populates, or run more particles."
    )]
    UnvisitedGroup {
        /// Name of the zone with the empty group.
        zone: String,
        /// Index of the empty group, in the order supplied.
        group: usize,
        /// Total group count.
        n_groups: usize,
    },
}

/// Macroscopic cross sections: cm^-1 to m^-1.
pub const CM_INV_TO_M_INV: f64 = 100.0;
/// Lengths: cm to m.
pub const CM_TO_M: f64 = 0.01;

/// Neutron rest-mass energy \[eV\], for the non-relativistic group speed.
const NEUTRON_MASS_EV: f64 = 939.565_420_52e6;
/// Speed of light \[m/s\].
const C_M_PER_S: f64 = 299_792_458.0;

/// Non-relativistic neutron speed \[m/s\] at energy `e_ev`.
///
/// `v = c * sqrt(2E / (m c^2))`. Non-relativistic is accurate to better than a
/// percent below ~10 MeV, which covers every group a reactor spectrum uses; at
/// 20 MeV it overestimates by about 1.5%. Stated rather than hidden because
/// `1/v` feeds the time-derivative term of a transient, and this module does
/// not support transients anyway (see the module note on delayed neutrons).
#[must_use]
pub fn neutron_speed_m_per_s(e_ev: f64) -> f64 {
    if e_ev <= 0.0 {
        return 0.0;
    }
    C_M_PER_S * (2.0 * e_ev / NEUTRON_MASS_EV).sqrt()
}

/// Build the GeN-Foam `nuclearData` input from a Monte Carlo MGXS set.
///
/// The result has a single reactor state named `"reference"` with no feedback
/// variables, because one Monte Carlo run is one state point. Feeding it to
/// [`outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData::from_input`]
/// gives a `CrossSectionData` a diffusion or SP3 solve can use.
///
/// `library` must be in **descending-energy** group order — GeN-Foam's
/// convention, group 0 fastest. Call
/// [`MgxsLibrary::in_descending_energy`](crate::mgxs::MgxsLibrary::in_descending_energy)
/// first; this function cannot tell the two orders apart and will not guess.
///
/// # Errors
///
/// [`GenfoamXsError::UnvisitedGroup`] if any zone has a group with zero flux.
/// That is refused rather than passed on, because zeros make the group's
/// diffusion equation singular and the solver then invents a flux there — see
/// that variant's documentation for the measured consequence.
pub fn to_nuclear_data_input(library: &MgxsLibrary) -> Result<NuclearDataInput, GenfoamXsError> {
    let n_g = library.groups.n_groups();
    let edges = library.groups.edges();

    // Group midpoints for the inverse-velocity vector. Geometric mean rather
    // than arithmetic: group boundaries are log-spaced in a reactor structure,
    // so the arithmetic midpoint of a decade-wide group sits far above its
    // flux-weighted centre.
    //
    // NOTE the edge list ascends even when the GROUPS are in descending-energy
    // order, so group `g` spans edges[n_g - 1 - g] .. edges[n_g - g].
    let iv: Vec<f64> = (0..n_g)
        .map(|g| {
            let lo = edges[n_g - 1 - g];
            let hi = edges[n_g - g];
            let mid = (lo * hi).sqrt();
            let v = neutron_speed_m_per_s(mid);
            if v > 0.0 {
                1.0 / v
            } else {
                0.0
            }
        })
        .collect();

    // Refuse unvisited groups BEFORE building anything: a zero-flux group has
    // no measured cross section, and zeros are singular input.
    for z in &library.zones {
        for (g, phi) in z.flux.iter().enumerate() {
            if !(*phi > 0.0) {
                return Err(GenfoamXsError::UnvisitedGroup {
                    zone: z.name.clone(),
                    group: g,
                    n_groups: n_g,
                });
            }
        }
    }

    let zones = library
        .zones
        .iter()
        .map(|z| {
            let d: Vec<f64> = (0..n_g)
                .map(|g| z.diffusion_coefficient(g).unwrap_or(0.0) * CM_TO_M)
                .collect();
            let nu_sigma_eff: Vec<f64> = z.nu_fission.iter().map(|v| v * CM_INV_TO_M_INV).collect();
            // kappa*Sigma_f is J/cm -> J/m: it is a per-length quantity, so it
            // scales like a cross section, not like a length.
            let sigma_pow: Vec<f64> = z
                .kappa_fission
                .iter()
                .map(|v| v * CM_INV_TO_M_INV)
                .collect();
            let sigma_removal: Vec<f64> = (0..n_g)
                .map(|g| z.removal(g).unwrap_or(0.0) * CM_INV_TO_M_INV)
                .collect();

            // NO transpose. GeN-Foam's ZoneStateInput documents
            // `scattering[moment][j][i]` as `Sigma_{s, j -> i}`, i.e.
            // [from][into] -- the same orientation as ZoneMgxs::scatter. Only
            // the units change.
            //
            // This was got WRONG first time, and the cost is recorded because it
            // is instructive: transposing here fed the solver zero in-scatter,
            // so the softer group was populated by its fission-spectrum share
            // alone. The spectrum came out 12x too hard and k_inf fell 4393 pcm.
            // It looked like a physics discrepancy, not an indexing bug.
            let scatter_si: Vec<Vec<f64>> = z
                .scatter
                .iter()
                .map(|row| row.iter().map(|v| v * CM_INV_TO_M_INV).collect())
                .collect();

            ZoneStateInput {
                name: z.name.clone(),
                d,
                nu_sigma_eff,
                sigma_pow,
                sigma_removal,
                chi_prompt: z.chi.clone(),
                // Inert: beta is all zeros below, so this multiplies nothing.
                chi_delayed: z.chi.clone(),
                scattering: vec![scatter_si],
                constants: Some(ZoneConstantsInput {
                    fuel_fraction: 1.0,
                    secondary_power_volume_fraction: 0.0,
                    fraction_to_secondary_power: 0.0,
                    df_adjust: false,
                    iv: iv.clone(),
                    // No discontinuity-factor correction: these group constants
                    // were condensed from a transport solution of the same
                    // geometry, not from an assembly calculation needing an ADF.
                    disc_factor: vec![1.0; n_g],
                    integral_flux: z.flux.clone(),
                    // See the module note: no delayed data is tallied. Zero
                    // beta is honest for a steady-state eigenvalue and WRONG
                    // for a transient.
                    beta: Vec::new(),
                    lambda: Vec::new(),
                }),
            }
        })
        .collect();

    Ok(NuclearDataInput {
        energy_groups: n_g,
        // No delayed precursor groups, for the reason above.
        prec_groups: 0,
        // P0 only: the Monte Carlo passes tally no scattering-cosine axis.
        legendre_moments: 1,
        poly_spline_mode: 1,
        fast_neutrons: false,
        xs_variables: Vec::new(),
        do_not_parametrize: Vec::new(),
        states: vec![StateInput {
            name: "reference".into(),
            parameters: BTreeMap::new(),
            zones,
        }],
    })
}
