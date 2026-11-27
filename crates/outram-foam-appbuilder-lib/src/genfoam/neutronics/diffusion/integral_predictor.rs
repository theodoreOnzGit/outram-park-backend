// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/include/integralPredictor.H (included by solveNeutronics.H when
//                    `eigenvalueNeutronics false` and `neutronTransport.integralPredictor true`), with
//                    src/classes/neutronics/include/{initializeNeutroSource,calcScatteringSource}.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `integralPredictor.H` of the diffusion neutronics (gh:#1919, README round 60).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Before the neutron iteration of a transient step, upstream balances the INTEGRAL flux: the one-group integral of the star fluxes
//! (`integralFlux`), the integral of the old-time star fluxes (`integralFluxOld`), and the volume integrals of the temporal, leakage
//! (`fvc::laplacian(D/discFactor, fluxStar)`, `Gauss linear corrected`), removal, source, scattering-source terms and of the delayed
//! neutron source give
//!
//! `newIntegralFlux = (precursorTerm + integralFluxOld temporalTerm) / (temporalTerm - leakageTerm + removalTerm - sourceTerm - scatteringSourceTerm)`
//!
//! (each `*Term` is the corresponding integral divided by `integralFlux`), every star flux is multiplied by
//! `newIntegralFlux/integralFluxOld` and `flux_ = fluxStar_/discFactor`. The log's "Old integral flux" is `integralFlux` (the CURRENT
//! one; upstream's label), "New predicted integral flux" is `newIntegralFlux`.
//!
//! Every sum is the sequential `sum_c V_c x_c` of `gSum(V*field)` (`fvc::domainIntegrate`); the leakage is `fvc::div` of the face flux
//! `(gamma_f snGrad_f) |Sf|` (`gaussLaplacianScheme::fvcLaplacian`, scalar gamma), `V (surfaceIntegrate/V)`.
//!
//! Not ported: `implicitPredictor.H` (refused by name by the caller's reader) and `aitkenAcceleration.H` (its three lists are
//! uninitialised in upstream and the real log of this case never reaches it: one neutron iteration per step).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use outram_foam_basic_lib::fv_operators::les_filter::surface_integrate;
use outram_foam_basic_lib::prelude::{fvc, NonOrthoScheme, VolScalarField};

use super::DiffusionNeutronics;

/// What `integralPredictor.H` printed and computed (the log's two `integral flux` lines and the six integrals).
#[derive(Debug, Clone, PartialEq)]
pub struct IntegralPredictorReport {
    /// `integralFlux` (printed as "Old integral flux").
    pub integral_flux: f64,
    /// `integralFluxOld` (from `fluxStar_.oldTime()`).
    pub integral_flux_old: f64,
    /// `newIntegralFlux` ("New predicted integral flux").
    pub new_integral_flux: f64,
    /// `temporalTermInt`, `leakageTermInt`, `removalTermInt`, `sourceTermInt`, `scatteringSourceTermInt`, `precursorTermInt`.
    pub integrals: [f64; 6],
}

/// `fvc::domainIntegrate`: `gSum(V*field)`, summed in cell order.
fn domain_integrate(volumes: &[f64], field: &[f64]) -> f64 {
    let mut s = 0.0;
    for c in 0..volumes.len() {
        s += volumes[c] * field[c];
    }
    s
}

impl DiffusionNeutronics {
    /// Switch `neutronTransport.integralPredictor` on (`true`) or off for [`Self::upstream_stationary_step`].
    #[must_use]
    pub fn with_integral_predictor(mut self, on: bool) -> Self {
        self.integral_predictor = on;
        self
    }

    /// Whether the transient step runs `integralPredictor.H`.
    #[must_use]
    pub fn integral_predictor_enabled(&self) -> bool {
        self.integral_predictor
    }

    /// `fvc::domainIntegrate(fvc::laplacian(D/discFactor, fluxStar_[g]))` with `Gauss linear corrected`: the face flux
    /// `(gamma_f snGrad_f) |Sf|` summed by `surfaceIntegrate`, divided by `V` and re-multiplied by it in the integral.
    fn leakage_integral(&self, g: usize, star_field: &VolScalarField, volumes: &[f64]) -> f64 {
        let mesh = self.state.mesh().clone();
        let gamma = self.d_over_df_face(g, star_field);
        let sn = fvc::sn_grad_corrected(star_field, NonOrthoScheme::Corrected);
        let internal: Vec<f64> =
            (0..mesh.n_internal_faces).map(|f| (gamma.internal[f] * sn.internal[f]) * mesh.face_areas[f]).collect();
        let boundary: Vec<Vec<f64>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| (gamma.boundary[pi].values[fi] * sn.boundary[pi].values[fi]) * mesh.face_areas[p.start + fi])
                    .collect()
            })
            .collect();
        let lap = surface_integrate(&mesh, &internal, &boundary);
        domain_integrate(volumes, &lap)
    }

    /// `integralPredictor.H`. `star` is `fluxStar_` (scaled in place), `star_fields` the same as fields with their boundary
    /// conditions, `flux_old` is `fluxStar_.oldTime()`, `neutro_source` is `neutroSource_` (as `initializeNeutroSource.H` left it, replaced
    /// by the re-initialised one on return), `delayed` is `delayedNeutroSource_`, `dt` the `deltaT`, `keff` `keff_`.
    ///
    /// On return the state's flux is `fluxStar_/discFactor` of the scaled star fluxes.
    pub(super) fn run_integral_predictor(
        &mut self,
        star: &mut [Vec<f64>],
        star_fields: &[VolScalarField],
        flux_old: &[Vec<f64>],
        neutro_source: &mut Vec<f64>,
        delayed: &[f64],
        dt: f64,
        keff: f64,
    ) -> IntegralPredictorReport {
        let n = self.state.mesh().n_cells;
        let groups = self.energy_groups();
        let volumes = self.state.mesh().cell_volumes.clone();
        // oneGroupFlux_ = 0; += fluxStar_[g]   and the same for the old time
        let mut one_group = vec![0.0; n];
        let mut one_group_old = vec![0.0; n];
        for g in 0..groups {
            for c in 0..n {
                one_group[c] += star[g][c];
                one_group_old[c] += flux_old[g][c];
            }
        }
        let integral_flux = domain_integrate(&volumes, &one_group);
        let integral_flux_old = domain_integrate(&volumes, &one_group_old);

        let (mut temporal, mut leakage, mut removal, mut scattering_src) = (0.0, 0.0, 0.0, 0.0);
        for g in 0..groups {
            let iv = self.xs.inv_velocity[g].internal.as_slice();
            let df = self.xs.disc_factor[g].internal.as_slice();
            let rem = self.xs.sigma_removal[g].internal.as_slice();
            // IV * fluxStar / discFactor / deltaT
            let t: Vec<f64> = (0..n).map(|c| ((iv[c] * star[g][c]) / df[c]) / dt).collect();
            temporal += domain_integrate(&volumes, &t);
            leakage += self.leakage_integral(g, &star_fields[g], &volumes);
            // sigmaRemoval * fluxStar / discFactor
            let r: Vec<f64> = (0..n).map(|c| (rem[c] * star[g][c]) / df[c]).collect();
            removal += domain_integrate(&volumes, &r);
            // calcScatteringSource.H with the current flux_
            let mut scatter = vec![0.0; n];
            for j in 0..groups {
                if j != g {
                    let sig = self.xs.scattering[j][g].internal.as_slice();
                    let ph = self.state.flux()[j].internal.as_slice();
                    for c in 0..n {
                        scatter[c] += sig[c] * ph[c];
                    }
                }
            }
            scattering_src += domain_integrate(&volumes, &scatter);
        }
        let beta_tot = self.xs.beta_tot.internal.as_slice();
        let source_field: Vec<f64> = (0..n).map(|c| (neutro_source[c] / keff) * (1.0 - beta_tot[c])).collect();
        let source = domain_integrate(&volumes, &source_field);
        let precursor = domain_integrate(&volumes, delayed);

        let temporal_term = temporal / integral_flux;
        let leakage_term = leakage / integral_flux;
        let removal_term = removal / integral_flux;
        let source_term = source / integral_flux;
        let scattering_term = scattering_src / integral_flux;
        let precursor_term = precursor;

        let new_integral_flux = (precursor_term + integral_flux_old * temporal_term)
            / (((((temporal_term - leakage_term) + removal_term) - source_term) - scattering_term));
        let factor = new_integral_flux / integral_flux_old;
        for g in 0..groups {
            for v in star[g].iter_mut() {
                *v *= factor;
            }
        }
        self.write_flux_from_star_values(star);
        // initializeNeutroSource.H
        neutro_source.iter_mut().for_each(|x| *x = 0.0);
        self.accumulate_neutro_source(neutro_source);
        IntegralPredictorReport {
            integral_flux,
            integral_flux_old,
            new_integral_flux,
            integrals: [temporal, leakage, removal, source, scattering_src, precursor],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_integrate_is_the_sequential_volume_weighted_sum() {
        assert_eq!(domain_integrate(&[2.0, 0.5], &[1.0, 3.0]), 3.5);
        assert_eq!(domain_integrate(&[], &[]), 0.0);
    }
}
