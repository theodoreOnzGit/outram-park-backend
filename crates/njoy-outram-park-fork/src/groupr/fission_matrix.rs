// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **Group fission spectrum (Chi) + fission group-to-group matrix** — the MF=5
//! fission-emission feed of GROUPR (`getff`/`getmf6` for MT=18,
//! `groupr.f90` fission-matrix path).
//!
//! # What this computes
//!
//! GROUPR treats fission as a full group-to-group matrix `sigma_f[g -> g']`: the
//! rate at which a neutron in incident group `g` induces fission producing a
//! neutron in outgoing (secondary) group `g'`. Two quantities come out of it:
//!
//! 1. **Group Chi** `chi_g'` — the multigroup fission emission spectrum: the
//!    fraction of fission neutrons born into outgoing group `g'`,
//!    `chi_g' = integral_{g'} chi(E') dE'`, normalized so `sum_g' chi_g' = 1`.
//!    This is the group-collapse of the (normalized) fission spectrum
//!    `chi(E')` supplied by [`FissionSpectrum`].
//! 2. **Fission matrix** `sigma_f[g -> g']` — in the common **separable**
//!    approximation `chi(E' | E) ~= chi(E')` (the outgoing spectrum is
//!    independent of the incident energy), this factorizes:
//!
//!    ```text
//!    sigma_f[g -> g'] = ( flux-weighted group nu*sigma_f in incident group g )
//!                       * chi_g'
//!    ```
//!
//! # Fidelity note (honest)
//!
//! The separable form is exact only when `chi` does not depend on the incident
//! energy. A fully incident-dependent χ(E'|E) matrix needs the MF=5 incident-
//! energy axis ([`crate::nuclear_data::secondary`]'s `ChiTabular::incident`),
//! which GROUPR reads through `getmf6`/`getdis`. This module scaffolds the group
//! **Chi** collapse and the **separable** matrix (both buildable from the
//! ported panel average + [`FissionSpectrum`]); the incident-dependent matrix is
//! flagged as the remaining gap.
//!
//! # Scope
//!
//! - **LIVE:** [`fission_group_chi`] collapses `spectrum.weight(E')` over each
//!   outgoing group with [`group_integral`] (flat weight = pure spectrum
//!   integral) and normalizes to sum 1.
//! - **LIVE:** [`separable_fission_matrix`] outer-products the vector group
//!   nu*sigma_f (via [`group_average_vector`]) with the group Chi; each row sums
//!   to the incident-group nu*sigma_f and is proportional to the shared group Chi.
//! - **Remaining gap:** the fully incident-dependent χ(E'|E) matrix (MF=5
//!   incident-energy axis) is still unported — see the fidelity note above.

use std::sync::Arc;

use crate::groupr::panel::{group_average_vector, group_integral, GroupFlux, PointwiseXs};
use crate::nuclear_data::secondary::FissionSpectrum;
use crate::NjoyError;

/// Validate a group-boundary array: `>= 2` entries, strictly ascending, finite.
///
/// Mirrors the panel-matrix `validate_bounds` guard: a group structure needs at
/// least one group (two edges) and must be strictly ascending so every group has
/// positive width. Returns [`NjoyError::EndfParse`] naming the offending array.
fn validate_bounds(bounds: &[f64], name: &'static str) -> Result<(), NjoyError> {
    if bounds.len() < 2 {
        return Err(NjoyError::EndfParse(format!(
            "fission_matrix: {name} needs >= 2 ascending entries"
        )));
    }
    if !bounds.windows(2).all(|w| w[1] > w[0] && w[0].is_finite()) {
        return Err(NjoyError::EndfParse(format!(
            "fission_matrix: {name} must be strictly ascending and finite"
        )));
    }
    Ok(())
}

/// Build a fine, log-spaced sampling grid spanning `[group_bounds[0],
/// group_bounds[last]]` with the group edges merged in.
///
/// The fission spectrum `chi(E')` is a smooth analytic shape with no intrinsic
/// break points, so [`group_integral`] (which marches on the cross-section grid)
/// needs a supplied grid to resolve the trapezoids. We use ~100 points per decade
/// (dense enough that the group-fraction distribution is grid-insensitive to
/// several significant figures) and splice in the exact group boundaries so each
/// group is integrated between exact edge samples. Ascending, deduplicated.
fn fine_emission_grid(group_bounds: &[f64]) -> Vec<f64> {
    let lo = group_bounds[0].max(1.0e-5);
    let hi = group_bounds[group_bounds.len() - 1];
    let decades = (hi / lo).log10().max(1.0);
    let n = ((decades * 100.0).ceil() as usize).max(2);
    let ratio = hi / lo;
    let mut grid: Vec<f64> = (0..=n)
        .map(|i| lo * ratio.powf(i as f64 / n as f64))
        .collect();
    grid.extend_from_slice(group_bounds);
    grid.sort_by(|a, b| a.partial_cmp(b).expect("finite grid energies"));
    grid.dedup();
    grid
}

/// A multigroup fission emission spectrum `chi_g'` — the group-collapsed,
/// normalized fission spectrum.
///
/// `chi[g]` is the fraction of fission neutrons born into outgoing group `g`
/// (0-based, ascending). Normalized so `sum_g chi[g] == 1` (within rounding).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupChi {
    /// Group boundaries \[eV\], ascending (length = number of groups + 1).
    pub group_bounds: Vec<f64>,
    /// `chi[g]` — fraction of fission neutrons born into group `g`; sums to 1.
    pub chi: Vec<f64>,
}

impl GroupChi {
    /// Fraction born into group `g`; `0.0` for an out-of-range index.
    pub fn get(&self, g: usize) -> f64 {
        self.chi.get(g).copied().unwrap_or(0.0)
    }
}

/// Collapse a fission spectrum `chi(E')` to the multigroup emission spectrum
/// `chi_g'`, normalized to sum 1.
///
/// # Parameters
/// - `spectrum` — the (shape-only) fission emission spectrum `chi(E')`
///   ([`FissionSpectrum`]); `spectrum.weight(E')` supplies the density.
/// - `group_bounds` — group boundaries \[eV\], **ascending**, `>= 2` entries.
///
/// # Method
/// Sample the (shape-only) emission density `spectrum.weight(E')` on a fine
/// log-spaced grid spanning the group structure (with the group edges spliced
/// in), wrap it as a [`PointwiseXs::LinLin`] "cross section", and integrate each
/// group with a **flat** weight via [`group_integral`]: the `rate` accumulator is
/// then `integral_{g} chi(E') dE'` (the numerator `integral sigma * phi` with
/// `phi == 1` reduces to `integral chi`). Summing the group integrals gives the
/// spectrum's total mass over the group structure; dividing each group by that
/// total yields `chi_g'` with `sum_g chi_g' == 1` by construction. All physical
/// fission spectra are non-negative, so every `chi_g'` is non-negative.
///
/// # Errors
/// [`NjoyError::EndfParse`] if `group_bounds` is not `>= 2` strictly-ascending
/// finite entries, or if the spectrum integrates to a non-positive total over the
/// group structure (a degenerate / empty spectrum, which cannot be normalized).
pub fn fission_group_chi(
    spectrum: &FissionSpectrum,
    group_bounds: &[f64],
) -> Result<GroupChi, NjoyError> {
    validate_bounds(group_bounds, "group_bounds")?;

    // Sample chi(E') on a fine grid and present it to the panel integrator as a
    // pointwise "cross section"; a flat weight then makes group_integral's `rate`
    // the pure spectrum integral over each group.
    let grid = fine_emission_grid(group_bounds);
    let samples: Vec<(f64, f64)> = grid.iter().map(|&e| (e, spectrum.weight(e))).collect();
    let chi_xs = PointwiseXs::LinLin(Arc::new(samples));

    let n_groups = group_bounds.len() - 1;
    let mut chi = vec![0.0_f64; n_groups];
    for (g, w) in group_bounds.windows(2).enumerate() {
        // rate = integral_{e_lo}^{e_hi} chi(E') * 1 dE'  (flat weight).
        chi[g] = group_integral(&chi_xs, &GroupFlux::Flat, w[0], w[1]).rate;
    }

    let total: f64 = chi.iter().sum();
    if !(total > 0.0) {
        return Err(NjoyError::EndfParse(format!(
            "fission_group_chi: spectrum integrates to non-positive total {total} \
             over the group structure; cannot normalize"
        )));
    }
    for c in &mut chi {
        *c /= total;
    }
    // A physical fission spectrum is non-negative everywhere, so each group
    // fraction must be non-negative; a negative here signals a corrupt weight.
    debug_assert!(
        chi.iter().all(|&c| c >= 0.0),
        "fission_group_chi produced a negative group fraction: {chi:?}"
    );

    Ok(GroupChi {
        group_bounds: group_bounds.to_vec(),
        chi,
    })
}

/// A fission group-to-group matrix `sigma_f[g -> g']` \[barn\].
///
/// `matrix[g][gp]` is the transfer from incident group `g` to outgoing group
/// `gp`. In the separable approximation each row `g` equals the incident-group
/// `nu*sigma_f` times the shared group Chi.
#[derive(Debug, Clone, PartialEq)]
pub struct FissionMatrix {
    /// Incident group boundaries \[eV\], ascending.
    pub incident_bounds: Vec<f64>,
    /// Outgoing (secondary) group boundaries \[eV\], ascending.
    pub secondary_bounds: Vec<f64>,
    /// `matrix[g][gp]` — fission transfer \[barn\], incident group `g`, outgoing
    /// group `gp`.
    pub matrix: Vec<Vec<f64>>,
    /// The shared group Chi used to build each row.
    pub group_chi: GroupChi,
}

impl FissionMatrix {
    /// Transfer element from incident group `g` to outgoing group `gp` \[barn\];
    /// `0.0` for out-of-range indices.
    pub fn element(&self, g: usize, gp: usize) -> f64 {
        self.matrix
            .get(g)
            .and_then(|r| r.get(gp))
            .copied()
            .unwrap_or(0.0)
    }

    /// Row sum `sum_gp sigma_f[g -> gp]` — the incident-group `nu*sigma_f`
    /// \[barn\] (since the group Chi sums to 1).
    pub fn row_sum(&self, g: usize) -> f64 {
        self.matrix.get(g).map(|r| r.iter().sum()).unwrap_or(0.0)
    }
}

/// Build the **separable** fission group-to-group matrix
/// `sigma_f[g -> g'] = (group nu*sigma_f)_g * chi_g'`.
///
/// # Parameters
/// - `nu_sigma_f` — pointwise `nu(E) * sigma_f(E)` \[barn vs eV\] (production
///   cross section).
/// - `spectrum` — the fission emission spectrum `chi(E')` ([`FissionSpectrum`]).
/// - `flux` — the incident-energy weighting flux ([`GroupFlux`]).
/// - `incident_bounds` / `secondary_bounds` — group boundaries \[eV\], ascending.
///
/// # Method
/// Compute the incident-group production vector
/// `nu_sf_g = group_average_vector(nu_sigma_f, flux, incident_bounds)` (barn) and
/// the shared group Chi `chi = fission_group_chi(spectrum, secondary_bounds)`,
/// then fill `matrix[g][gp] = nu_sf_g[g] * chi.chi[gp]`. Because `sum_gp chi_gp ==
/// 1`, `row_sum(g) == nu_sf_g[g]`, and every row is (by construction) exactly
/// proportional to the shared Chi — the separable-χ signature.
///
/// # Errors
/// [`NjoyError::EndfParse`] if either bounds array is not `>= 2` strictly-
/// ascending finite entries, or if the spectrum cannot be normalized (propagated
/// from [`fission_group_chi`]).
pub fn separable_fission_matrix(
    nu_sigma_f: &PointwiseXs,
    spectrum: &FissionSpectrum,
    flux: &GroupFlux,
    incident_bounds: &[f64],
    secondary_bounds: &[f64],
) -> Result<FissionMatrix, NjoyError> {
    validate_bounds(incident_bounds, "incident_bounds")?;
    validate_bounds(secondary_bounds, "secondary_bounds")?;

    let nu_sf_g = group_average_vector(nu_sigma_f, flux, incident_bounds);
    let group_chi = fission_group_chi(spectrum, secondary_bounds)?;

    // Separable outer product: each incident row is the incident-group production
    // times the shared outgoing Chi.
    let matrix: Vec<Vec<f64>> = nu_sf_g
        .iter()
        .map(|&nu_sf| group_chi.chi.iter().map(|&c| nu_sf * c).collect())
        .collect();

    Ok(FissionMatrix {
        incident_bounds: incident_bounds.to_vec(),
        secondary_bounds: secondary_bounds.to_vec(),
        matrix,
        group_chi,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collapsing a U-235 Watt spectrum to a coarse group structure yields a
    /// normalized group Chi whose mass concentrates in the fast groups.
    ///
    /// **Methodology.** Collapse the default [`FissionSpectrum`] (U-235 Watt,
    /// `a = 0.988e6 eV`, `b = 2.249e-6 /eV`; the Watt shape peaks near
    /// `0.7 MeV`) over the 4-group structure
    /// `[1e-3, 1e3, 1e5, 1e6, 2e7] eV` with [`fission_group_chi`]. Assert:
    /// (a) `sum_g chi_g == 1` to `< 1e-10` (normalization, exact by construction);
    /// (b) every `chi_g >= 0` (a physical emission spectrum is non-negative);
    /// (c) the bulk lands in the two fast groups `g2 = [1e5, 1e6)` and
    /// `g3 = [1e6, 2e7)` — i.e. `chi[2] + chi[3] > 0.95` — consistent with a
    /// ~2 MeV mean fission-neutron energy. The reference is the analytic Watt
    /// shape itself (the collapse must reproduce where its mass sits), not an
    /// external benchmark.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** With a ~100-points-per-decade
    /// sampling grid the group Chi is
    /// `chi = [1.38921e-5, 0.0133624, 0.287804, 0.698819]`
    /// (groups low→high). `sum = 1.0` (deviation `< 1e-16`), all non-negative,
    /// and `chi[2] + chi[3] = 0.986624` — the two fast groups carry essentially
    /// all of the fission emission, as expected for a Watt spectrum. The peak
    /// group `g2 = [1e5, 1e6)` (containing the ~0.7 MeV Watt peak) holds 0.2878
    /// and the `g3 = [1e6, 2e7)` group 0.6988; the two lowest groups are nearly
    /// empty (`~0.0134` combined). Interpretation: the separable group-Chi
    /// collapse places fission-neutron birth in the fast spectrum correctly.
    #[test]
    fn watt_group_chi_normalizes_and_is_fast() {
        let spectrum = FissionSpectrum::default();
        let bounds = [1.0e-3, 1.0e3, 1.0e5, 1.0e6, 2.0e7];
        let gc = fission_group_chi(&spectrum, &bounds).expect("Watt collapse");

        let sum: f64 = gc.chi.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10, "sum chi_g = {sum}");
        assert!(
            gc.chi.iter().all(|&c| c >= 0.0),
            "negative chi: {:?}",
            gc.chi
        );
        assert_eq!(gc.chi.len(), 4);

        let fast = gc.chi[2] + gc.chi[3];
        assert!(
            fast > 0.95,
            "fast-group fraction {fast} too low; chi = {:?}",
            gc.chi
        );
        // The single highest group carries the majority (Watt mean ~2 MeV).
        assert!(gc.chi[3] > 0.5, "top group {} should dominate", gc.chi[3]);
    }

    /// The separable fission matrix has row sums equal to the incident-group
    /// `nu*sigma_f`, and every row is exactly proportional to the shared group Chi.
    ///
    /// **Methodology.** Use a linear production cross section
    /// `nu*sigma_f(E) = 1 + 1e-6 * E` \[barn\] (so each incident group averages to
    /// a distinct value under a flat weight) and the default U-235 Watt spectrum,
    /// over incident = secondary bounds `[1e-3, 1e3, 1e5, 1e6, 2e7] eV`. Build the
    /// matrix with [`separable_fission_matrix`]. Independently compute
    /// `nu_sf_g = group_average_vector(nu_sigma_f, Flat, bounds)`. Assert:
    /// (a) `FissionMatrix::row_sum(g) == nu_sf_g[g]` to `< 1e-9` for every `g`
    /// (since the shared Chi sums to 1);
    /// (b) for each incident group `g`, the ratio `matrix[g][gp] / chi[gp]` is
    /// constant across all outgoing groups `gp` with `chi[gp] > 1e-12`, and equals
    /// `nu_sf_g[g]` — the separable-χ signature.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** The flat-weight group averages are
    /// `nu_sf_g = [1.0005000005, 1.0505, 1.55, 11.5]` barn (midpoint values of the
    /// linear cross section over each group, `(sigma(E_lo)+sigma(E_hi))/2`). Every
    /// `row_sum(g)` matches its `nu_sf_g[g]` to `< 1e-12` (measured deviations
    /// `<= 3e-16`), and within each row the `matrix[g][gp]/chi[gp]` ratio is
    /// constant to `< 1e-9` across the populated outgoing groups. Interpretation:
    /// the matrix factorizes exactly as `nu_sf_g outer chi_gp`, confirming the
    /// separable approximation is implemented without cross-term leakage.
    #[test]
    fn separable_matrix_row_sums_and_proportionality() {
        let bounds = [1.0e-3, 1.0e3, 1.0e5, 1.0e6, 2.0e7];
        // Linear nu*sigma_f so incident groups get distinct flat-weight averages.
        let table: Vec<(f64, f64)> = bounds.iter().map(|&e| (e, 1.0 + 1.0e-6 * e)).collect();
        let nu_sf = PointwiseXs::LinLin(Arc::new(table));
        let spectrum = FissionSpectrum::default();

        let fm = separable_fission_matrix(&nu_sf, &spectrum, &GroupFlux::Flat, &bounds, &bounds)
            .expect("separable matrix");
        let nu_sf_g = group_average_vector(&nu_sf, &GroupFlux::Flat, &bounds);

        assert_eq!(fm.matrix.len(), 4);
        for g in 0..4 {
            // (a) row sum == incident-group nu*sigma_f.
            assert!(
                (fm.row_sum(g) - nu_sf_g[g]).abs() < 1e-9,
                "row_sum({g}) = {} vs nu_sf_g = {}",
                fm.row_sum(g),
                nu_sf_g[g]
            );
            // (b) each populated column has the same ratio == nu_sf_g[g].
            for gp in 0..4 {
                let chi = fm.group_chi.chi[gp];
                if chi > 1e-12 {
                    let ratio = fm.element(g, gp) / chi;
                    assert!(
                        (ratio - nu_sf_g[g]).abs() < 1e-9,
                        "ratio matrix[{g}][{gp}]/chi = {ratio} != nu_sf_g = {}",
                        nu_sf_g[g]
                    );
                }
            }
        }
    }

    /// Degenerate inputs are rejected rather than silently normalized: a too-short
    /// group structure and a zero-mass spectrum both return [`NjoyError::EndfParse`].
    #[test]
    fn invalid_inputs_rejected() {
        let spectrum = FissionSpectrum::default();
        // Too-short bounds.
        assert!(matches!(
            fission_group_chi(&spectrum, &[1.0e3]),
            Err(NjoyError::EndfParse(_))
        ));
        // A tabulated spectrum with zero mass over the group structure.
        let empty = FissionSpectrum::Tabulated {
            e_out: vec![1.0, 2.0],
            pdf: vec![0.0, 0.0],
        };
        assert!(matches!(
            fission_group_chi(&empty, &[1.0e3, 1.0e5, 2.0e7]),
            Err(NjoyError::EndfParse(_))
        ));
    }
}
