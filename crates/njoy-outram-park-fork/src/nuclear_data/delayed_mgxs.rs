//! Delayed-group multigroup collapse: condense the delayed-neutron data
//! ([`super::delayed`]) onto an energy-group structure, one set per precursor
//! (delayed) group.
//!
//! This is the njoy side of the openmc-notebooks **`mdgxs-part-ii`** operations
//! `mgxs.DelayedNuFissionXS`, `mgxs.Beta`, `mgxs.ChiDelayed`, and
//! `mgxs.DecayRate` (bead op-6tz.6.4). The delayed *data* itself — the `NNF`
//! (usually 6) precursor decay constants λ, the total delayed ν̄_d(E), the
//! per-group emission fractions p_k(E), and the per-group outgoing spectra
//! χ_k(E') — is read off the ENDF tape in [`super::delayed`]. This module adds
//! the **group condensation**: a flux-weighted group average (the same
//! midpoint-rule collapse [`super::Mgxs::collapse`] uses for the prompt/total
//! channels) applied per delayed group.
//!
//! # What is produced
//!
//! For an ascending energy-group structure `group_bounds` (`n_g` groups) and
//! `n_d` precursor groups, [`DelayedMgxs::collapse`] yields:
//!
//! - **`decay_rate[k]`** — λ_k \[s⁻¹\], the group's precursor decay constant
//!   (energy-independent for the ENDF/B-VII/VIII actinides; carried through
//!   verbatim). Exposed dimensionally via [`DelayedMgxs::decay_constant`].
//! - **`delayed_nu_fission[g][k]`** — the group-averaged delayed
//!   fission-production cross section ν_{d,k}·σ_f \[barn\], where
//!   ν_{d,k}(E) = ν̄_d(E)·p_k(E).
//! - **`beta[g][k]`** — the group delayed-neutron fraction
//!   β_{g,k} = (delayed ν·σ_f)_{g,k} / (total ν·σ_f)_g. This is the ENDF
//!   *nominal* β, **not** the transport-adjoint effective β_eff (which needs an
//!   importance weight from a transport solve — that belongs to
//!   `outram-mc-libs`).
//! - **`chi_delayed[k][g']`** — the delayed emission spectrum of precursor group
//!   `k` integrated over outgoing energy group `g'`, normalized so
//!   Σ_{g'} χ_delayed[k][g'] = 1.
//!
//! # Weighting and fidelity
//!
//! The collapse takes a caller-supplied fine pointwise fission cross section
//! σ_f(E) and total ν̄(E), plus a [`WeightingSpectrum`] φ(E), exactly like
//! [`super::Mgxs::collapse`]. It is a **fixed-spectrum** condensation: there is
//! no Boltzmann/self-shielding solve. A flux-solved, self-shielded delayed MGXS
//! (and the precursor *concentrations*, which need transport tallies) is the
//! `outram-mc-libs` transport-side responsibility, per the notebook split.
//!
//! No trait objects, no `Box`, no lifetimes (workspace rules); storage is plain
//! `f64` in documented units with a [`DecayConstant`] `uom` alias on the
//! accessor boundary.

use uom::si::f64::Frequency;
use uom::si::frequency::hertz;

use super::delayed::{DecayConstant, DelayedChi, DelayedNuBar};
use super::weighting::WeightingSpectrum;

/// Delayed-neutron data condensed onto an energy-group structure (see the module
/// doc). Row index `g` is the incident energy group; the inner index `k` is the
/// precursor (delayed) group in ENDF tape order.
#[derive(Debug, Clone, Default)]
pub struct DelayedMgxs {
    /// Nuclide name (e.g. `"U235"`).
    pub name: String,
    /// Incident-energy group boundaries \[eV\], ascending, length `n_groups + 1`.
    pub group_bounds: Vec<f64>,
    /// Precursor decay constants λ_k \[s⁻¹\], one per delayed group, tape order.
    pub decay_rate: Vec<f64>,
    /// Group-averaged delayed fission production ν_{d,k}·σ_f \[barn\], indexed
    /// `[energy group g][delayed group k]`.
    pub delayed_nu_fission: Vec<Vec<f64>>,
    /// Group delayed-neutron fraction β_{g,k} (dimensionless), indexed
    /// `[energy group g][delayed group k]`.
    pub beta: Vec<Vec<f64>>,
    /// Delayed emission spectrum integrated over outgoing energy groups,
    /// normalized to sum to 1 per delayed group; indexed
    /// `[delayed group k][outgoing energy group g']`.
    pub chi_delayed: Vec<Vec<f64>>,
}

impl DelayedMgxs {
    /// Number of incident energy groups.
    pub fn n_groups(&self) -> usize {
        self.group_bounds.len().saturating_sub(1)
    }

    /// Number of precursor (delayed) groups (`NNF`).
    pub fn n_delayed_groups(&self) -> usize {
        self.decay_rate.len()
    }

    /// The decay constant of precursor group `k` as a dimensioned
    /// [`DecayConstant`] (a [`Frequency`]; 1 Hz ≡ 1 s⁻¹). `None` if out of range.
    pub fn decay_constant(&self, k: usize) -> Option<DecayConstant> {
        self.decay_rate.get(k).map(|&l| Frequency::new::<hertz>(l))
    }

    /// Total (summed over precursor groups) delayed fraction of energy group `g`,
    /// β_g = Σ_k β_{g,k}. Returns `0.0` if `g` is out of range.
    pub fn total_beta(&self, g: usize) -> f64 {
        self.beta.get(g).map(|row| row.iter().sum()).unwrap_or(0.0)
    }

    /// Condense the delayed-neutron data onto `group_bounds`.
    ///
    /// # Parameters
    /// - `name` — nuclide label carried onto the result.
    /// - `energy` — fine incident-energy grid \[eV\], **ascending**.
    /// - `fission` — pointwise fission σ_f(E) \[barn\], aligned with `energy`.
    /// - `nu_total` — pointwise total ν̄(E) (prompt + delayed), aligned with
    ///   `energy`; used to form the total ν·σ_f that `beta` divides by.
    /// - `delayed` — the parsed MF=1/455 data (λ_k and ν̄_d(E)).
    /// - `chi` — the parsed MF=5/455 data (per-group p_k(E) and χ_k(E')).
    /// - `group_bounds` — target energy-group boundaries \[eV\], ascending,
    ///   `≥ 2` entries. The **same** structure is used for both the incident
    ///   condensation and the outgoing χ integration.
    /// - `spectrum` — the weighting φ(E) (e.g. [`WeightingSpectrum::default`]).
    ///
    /// The number of precursor groups is taken from `chi.groups` (the MF=5/455
    /// subsection count); a precursor group with no `delayed.lambda[k]` gets a
    /// `0.0` decay rate. A group with zero collapsed weight keeps `0.0` constants.
    #[allow(clippy::too_many_arguments)]
    pub fn collapse(
        name: impl Into<String>,
        energy: &[f64],
        fission: &[f64],
        nu_total: &[f64],
        delayed: &DelayedNuBar,
        chi: &DelayedChi,
        group_bounds: &[f64],
        spectrum: &WeightingSpectrum,
    ) -> Self {
        let n_g = group_bounds.len().saturating_sub(1);
        let n_d = chi.groups.len();

        // Numerators/denominators for the flux-weighted incident-energy collapse.
        // den[g] = ∫_g φ; num_total[g] = ∫_g ν_total·σ_f·φ;
        // num_del[g][k] = ∫_g ν_d·p_k·σ_f·φ.
        let mut den = vec![0.0f64; n_g.max(1)];
        let mut num_total = vec![0.0f64; n_g.max(1)];
        let mut num_del = vec![vec![0.0f64; n_d]; n_g.max(1)];

        let n = energy.len();
        for i in 0..n.saturating_sub(1) {
            let (e0, e1) = (energy[i], energy[i + 1]);
            let mid = 0.5 * (e0 + e1);
            let d_e = e1 - e0;
            if d_e <= 0.0 || mid < group_bounds[0] || mid >= group_bounds[n_g] {
                continue;
            }
            let phi = spectrum.weight(mid) * d_e;
            if phi <= 0.0 {
                continue;
            }
            let g = group_bounds
                .partition_point(|&b| b <= mid)
                .saturating_sub(1)
                .min(n_g - 1);
            let avg = |v: &[f64]| 0.5 * (v[i] + v[i + 1]);
            let sf = avg(fission);
            let nu_t = avg(nu_total);
            let nu_d = delayed.nu_delayed_at(mid);

            den[g] += phi;
            num_total[g] += nu_t * sf * phi;
            for (k, grp) in chi.groups.iter().enumerate() {
                let p_k = interp_pairs_clamped(&grp.fraction, mid);
                num_del[g][k] += nu_d * p_k * sf * phi;
            }
        }

        let mut delayed_nu_fission = vec![vec![0.0f64; n_d]; n_g];
        let mut beta = vec![vec![0.0f64; n_d]; n_g];
        for g in 0..n_g {
            let d = den[g];
            let nf_total = if d > 0.0 { num_total[g] / d } else { 0.0 };
            for k in 0..n_d {
                let dnf = if d > 0.0 { num_del[g][k] / d } else { 0.0 };
                delayed_nu_fission[g][k] = dnf;
                beta[g][k] = if nf_total > 0.0 { dnf / nf_total } else { 0.0 };
            }
        }

        // Outgoing χ: integrate each precursor spectrum over the energy groups,
        // then normalize each precursor group to sum to 1.
        let chi_delayed = chi
            .groups
            .iter()
            .map(|grp| group_integrate_spectrum(&grp.spectrum, group_bounds))
            .collect();

        let decay_rate = (0..n_d)
            .map(|k| delayed.lambda.get(k).copied().unwrap_or(0.0))
            .collect();

        DelayedMgxs {
            name: name.into(),
            group_bounds: group_bounds.to_vec(),
            decay_rate,
            delayed_nu_fission,
            beta,
            chi_delayed,
        }
    }
}

/// Lin-lin interpolate a `(x, y)` table at `x`, clamped to the endpoint values
/// outside the tabulated range. Returns `0.0` for an empty table. (Mirrors the
/// clamped interpolation used by [`super::delayed`]; kept local so the pair-typed
/// `fraction`/spectrum tables can be interpolated without a slice split.)
fn interp_pairs_clamped(pairs: &[(f64, f64)], x: f64) -> f64 {
    match pairs.first() {
        None => 0.0,
        Some(&(x0, y0)) if x <= x0 => y0,
        Some(_) => {
            let &(xn, yn) = pairs.last().unwrap();
            if x >= xn {
                return yn;
            }
            let hi = pairs.iter().position(|&(xv, _)| xv >= x).unwrap();
            let (xa, ya) = pairs[hi - 1];
            let (xb, yb) = pairs[hi];
            ya + (yb - ya) * (x - xa) / (xb - xa)
        }
    }
}

/// Integrate an outgoing spectrum `(E' [eV], density [eV⁻¹])` over the energy
/// groups `bounds`, returning one integral per group normalized so the vector
/// sums to 1. Each spectrum trapezoid contributes to the group its midpoint falls
/// in (consistent with the midpoint binning of [`super::Mgxs::collapse`]). A
/// spectrum with no positive integral yields an all-zero vector.
fn group_integrate_spectrum(spectrum: &[(f64, f64)], bounds: &[f64]) -> Vec<f64> {
    let n_g = bounds.len().saturating_sub(1);
    let mut acc = vec![0.0f64; n_g];
    if n_g == 0 {
        return acc;
    }
    for w in spectrum.windows(2) {
        let (ea, ya) = w[0];
        let (eb, yb) = w[1];
        let de = eb - ea;
        if de <= 0.0 {
            continue;
        }
        let mid = 0.5 * (ea + eb);
        if mid < bounds[0] || mid >= bounds[n_g] {
            continue;
        }
        let area = 0.5 * (ya + yb) * de;
        let g = bounds
            .partition_point(|&b| b <= mid)
            .saturating_sub(1)
            .min(n_g - 1);
        acc[g] += area;
    }
    let total: f64 = acc.iter().sum();
    if total > 0.0 {
        for v in &mut acc {
            *v /= total;
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nuclear_data::delayed::DelayedChiGroup;

    /// A two-precursor, two-energy-group synthetic case with hand-computable
    /// answers: flat σ_f, flat ν, constant per-group fractions. β must equal the
    /// analytic ν_d·p_k / ν_total, χ must group-normalize to 1, and the decay
    /// rates pass through.
    #[test]
    fn collapse_matches_analytic_two_group() {
        // Fine grid spanning both groups; flat cross sections and ν.
        let energy: Vec<f64> = (0..=100).map(|i| 1.0 + i as f64 * 1.0e5).collect();
        let fission = vec![2.0; energy.len()];
        let nu_total = vec![2.5; energy.len()];

        // Delayed data: λ = [0.02, 0.5]; ν_d = 0.02 flat over all E.
        let delayed = DelayedNuBar {
            lambda: vec![0.02, 0.5],
            energy: vec![1.0e-5, 2.0e7],
            nu_delayed: vec![0.02, 0.02],
            ldg1_energy_dependent: false,
        };
        // χ: two groups, constant fractions p = [0.3, 0.7]; each with a triangular
        // outgoing spectrum on [0, 2 MeV] (area 1).
        let g_spec = vec![(0.0, 0.0), (1.0e6, 1.0e-6), (2.0e6, 0.0)];
        let chi = DelayedChi {
            groups: vec![
                DelayedChiGroup {
                    fraction: vec![(1.0e-5, 0.3), (2.0e7, 0.3)],
                    lf: 5,
                    spectrum: g_spec.clone(),
                },
                DelayedChiGroup {
                    fraction: vec![(1.0e-5, 0.7), (2.0e7, 0.7)],
                    lf: 5,
                    spectrum: g_spec,
                },
            ],
        };

        let bounds = vec![1.0, 5.0e6, 1.0e7];
        let dm = DelayedMgxs::collapse(
            "SYNTH",
            &energy,
            &fission,
            &nu_total,
            &delayed,
            &chi,
            &bounds,
            &WeightingSpectrum::OneOverE,
        );

        assert_eq!(dm.n_groups(), 2);
        assert_eq!(dm.n_delayed_groups(), 2);
        assert_eq!(dm.decay_rate, vec![0.02, 0.5]);

        // Analytic β_{g,k} = (ν_d·p_k·σ_f) / (ν_total·σ_f) = ν_d·p_k / ν_total.
        let beta_expect = |p: f64| 0.02 * p / 2.5;
        for g in 0..2 {
            assert!((dm.beta[g][0] - beta_expect(0.3)).abs() < 1e-9, "β[{g}][0]");
            assert!((dm.beta[g][1] - beta_expect(0.7)).abs() < 1e-9, "β[{g}][1]");
            // delayed ν·σ_f = ν_d·p_k·σ_f.
            assert!((dm.delayed_nu_fission[g][0] - 0.02 * 0.3 * 2.0).abs() < 1e-9);
            assert!((dm.delayed_nu_fission[g][1] - 0.02 * 0.7 * 2.0).abs() < 1e-9);
        }
        // Total β per group = ν_d/ν_total (p sums to 1).
        for g in 0..2 {
            assert!((dm.total_beta(g) - 0.02 / 2.5).abs() < 1e-9, "Σβ[{g}]");
        }
        // χ normalizes to 1 per precursor group.
        for k in 0..2 {
            let s: f64 = dm.chi_delayed[k].iter().sum();
            assert!((s - 1.0).abs() < 1e-9, "χ_delayed[{k}] Σ = {s}");
            assert!(dm.chi_delayed[k].iter().all(|&v| v >= 0.0));
        }
        // uom accessor round-trips.
        assert!((dm.decay_constant(1).unwrap().get::<hertz>() - 0.5).abs() < 1e-12);
        assert!(dm.decay_constant(2).is_none());
    }
}
