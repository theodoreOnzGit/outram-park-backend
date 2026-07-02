//! Nuclear-data **provider** surface — what transport codes pull from this crate.
//!
//! Per the OUTRAM PARK architecture (`docs/architecture.md`), *all* nuclear-data
//! representation lives in `njoy-outram-park-fork`. Downstream transport crates
//! (`openmc-libs` for Monte Carlo, later a deterministic solver) do **not** own
//! cross sections — they call into this module for the microscopic cross sections
//! and secondary distributions a history needs.
//!
//! This module is the *thin, human-readable* boundary over the heavier machinery:
//! - [`crate::wmp`] — windowed-multipole cross sections + analytic Doppler.
//! - [`crate::ace`] — the ACE writer / (future) lean-ACE pointwise tables.
//! - [`secondary`] — ν̄(E) and χ(E), which WMP does not carry.
//!
//! # Dispatch is an enum, not a trait object
//!
//! Following the workspace rule (no `dyn`), a nuclide's cross-section *source* is
//! the [`XsProvider`] enum. A consumer holds one and calls [`XsProvider::micro`];
//! adding a new representation is a new variant that every `match` must handle.

pub mod secondary;

use crate::wmp::WindowedMultipole;
use secondary::{FissionSpectrum, NuBar};

/// Microscopic neutron cross sections at one energy/temperature \[barn\].
///
/// The common currency between the data crate and any transport kernel. All
/// channels are microscopic (per target atom); the transport code multiplies by
/// atom density to get macroscopic Σ \[cm⁻¹\].
#[derive(Debug, Clone, Copy, Default)]
pub struct MicroXs {
    /// Total σ_t \[barn\].
    pub total: f64,
    /// Elastic scattering σ_s \[barn\].
    pub elastic: f64,
    /// Fission σ_f \[barn\].
    pub fission: f64,
    /// Radiative capture σ_γ \[barn\].
    pub capture: f64,
    /// Fission production ν·σ_f \[barn\] (ν̄ folded in for the fission source).
    pub nu_fission: f64,
}

/// The cross-section *representation* backing one nuclide.
///
/// - `Multipole` — WMP σ + ν̄ + χ; the compact, in-crate, analytically-broadenable
///   path (the default OUTRAM PARK ships). Accurate over thermal + resonance.
/// - `Mgxs` — a coarse **multigroup** set for the fast range above WMP's `e_max`.
///   Group constants are Watt-spectrum-weighted, **not** self-shielded (no
///   Boltzmann solve): a deliberately low-fidelity / high-speed fallback. See
///   [`Mgxs`].
pub enum XsProvider {
    Multipole {
        wmp: WindowedMultipole,
        nu: NuBar,
        chi: FissionSpectrum,
    },
    Mgxs(Mgxs),
}

impl XsProvider {
    /// Microscopic cross sections at incident energy `e` \[eV\] and temperature
    /// `temp_k` \[K\]. Dispatches over the representation.
    pub fn micro(&self, e: f64, temp_k: f64) -> MicroXs {
        match self {
            XsProvider::Multipole { wmp, nu, .. } => {
                let x = wmp.evaluate(e, temp_k);
                MicroXs {
                    total: x.total(),
                    elastic: x.scatter,
                    fission: x.fission,
                    capture: x.capture(),
                    nu_fission: x.fission * nu.at(e),
                }
            }
            // Multigroup is temperature-independent by construction — the fast
            // range is smooth and the collapse is done once, offline.
            XsProvider::Mgxs(mg) => mg.micro(e),
        }
    }
}

/// A coarse **multigroup** cross-section set for the fast range above WMP's
/// `e_max` — the "ACE-lite" high-energy fallback.
///
/// # Fidelity contract
///
/// This is the **low-fidelity, high-speed** path, on purpose:
/// - Group constants are collapsed *once, offline* with a **Watt fission-spectrum
///   weight** ([`Mgxs::collapse_watt`]): σ_g = ∫σ(E)χ(E)dE / ∫χ(E)dE over each
///   group. That is the only weighting — there is **no Boltzmann self-shielding
///   solve, no Bondarenko dilution, no URR treatment**.
/// - Runtime lookup ([`Mgxs::micro`]) is a piecewise-constant group index —
///   O(log n) — and **temperature-independent**. The fast range where a bare
///   sphere lives is smooth, so a hard-spectrum-weighted constant is adequate and
///   fast. Callers wanting Doppler accuracy stay on the WMP ([`XsProvider::Multipole`])
///   path below `e_max`.
///
/// # Layout
///
/// `group_bounds` holds `n_groups + 1` energy boundaries \[eV\] in **ascending**
/// order; each σ column has `n_groups` entries, where entry `g` is the constant
/// for the group `[group_bounds[g], group_bounds[g + 1])`.
#[derive(Debug, Clone, Default)]
pub struct Mgxs {
    /// Nuclide name (e.g. `"U238"`).
    pub name: String,
    /// Group boundaries \[eV\], ascending, length `n_groups + 1`.
    pub group_bounds: Vec<f64>,
    /// Total σ_t per group \[barn\].
    pub total: Vec<f64>,
    /// Elastic scattering σ_s per group \[barn\].
    pub elastic: Vec<f64>,
    /// Fission σ_f per group \[barn\].
    pub fission: Vec<f64>,
    /// Radiative capture σ_γ per group \[barn\].
    pub capture: Vec<f64>,
    /// Fission production ν·σ_f per group \[barn\].
    pub nu_fission: Vec<f64>,
}

impl Mgxs {
    /// Number of energy groups.
    pub fn n_groups(&self) -> usize {
        self.group_bounds.len().saturating_sub(1)
    }

    /// Index of the group containing incident energy `e` \[eV\].
    ///
    /// Energies below the first boundary clamp to group 0; energies at or above
    /// the last boundary clamp to the top group. The fast MGXS set only claims the
    /// range above WMP's `e_max`; clamping keeps a stray lookup finite rather than
    /// panicking.
    fn group_index(&self, e: f64) -> usize {
        let n = self.n_groups();
        if n == 0 {
            return 0;
        }
        // partition_point returns the count of bounds <= e; subtract 1 for the
        // group index, clamped into [0, n-1].
        let p = self.group_bounds.partition_point(|&b| b <= e);
        p.saturating_sub(1).min(n - 1)
    }

    /// Microscopic cross sections at incident energy `e` \[eV\] — a constant-per-
    /// group lookup (temperature-independent; see the type-level fidelity note).
    pub fn micro(&self, e: f64) -> MicroXs {
        if self.n_groups() == 0 {
            return MicroXs::default();
        }
        let g = self.group_index(e);
        MicroXs {
            total: self.total[g],
            elastic: self.elastic[g],
            fission: self.fission[g],
            capture: self.capture[g],
            nu_fission: self.nu_fission[g],
        }
    }

    /// Collapse a fine pointwise cross-section set to group constants using a
    /// **Watt fission-spectrum weight** — the offline bake step.
    ///
    /// For each group `[E_lo, E_hi)` and each channel, computes the
    /// spectrum-weighted average σ_g = ∫σ(E)χ(E)dE / ∫χ(E)dE by the midpoint rule
    /// over the fine grid (each fine interval contributes to the group its
    /// midpoint falls in). This is intentionally crude — the fast range is smooth
    /// and this is the low-fidelity path.
    ///
    /// # Parameters
    /// - `energy` — fine incident-energy grid \[eV\], **ascending**.
    /// - `total`/`elastic`/`fission`/`capture`/`nu_fission` — σ columns \[barn\]
    ///   aligned with `energy` (each the same length as `energy`).
    /// - `group_bounds` — target group boundaries \[eV\], ascending, `≥ 2` entries.
    /// - `spectrum` — the weighting χ(E); pass a [`FissionSpectrum::Watt`].
    ///
    /// A group with no fine points (zero weight) keeps a `0.0` constant.
    #[allow(clippy::too_many_arguments)]
    pub fn collapse_watt(
        name: impl Into<String>,
        energy: &[f64],
        total: &[f64],
        elastic: &[f64],
        fission: &[f64],
        capture: &[f64],
        nu_fission: &[f64],
        group_bounds: &[f64],
        spectrum: &FissionSpectrum,
    ) -> Self {
        let n_g = group_bounds.len().saturating_sub(1);
        let mut num = [
            vec![0.0; n_g],
            vec![0.0; n_g],
            vec![0.0; n_g],
            vec![0.0; n_g],
            vec![0.0; n_g],
        ];
        let mut den = vec![0.0f64; n_g];

        // Midpoint rule over each fine interval; bin into the group of its midpoint.
        for i in 0..energy.len().saturating_sub(1) {
            let (e0, e1) = (energy[i], energy[i + 1]);
            let mid = 0.5 * (e0 + e1);
            let d_e = e1 - e0;
            if d_e <= 0.0 || mid < group_bounds[0] || mid >= group_bounds[n_g] {
                continue;
            }
            let w = spectrum.weight(mid) * d_e;
            if w <= 0.0 {
                continue;
            }
            let g = group_bounds.partition_point(|&b| b <= mid).saturating_sub(1);
            let g = g.min(n_g - 1);
            let avg = |v: &[f64]| 0.5 * (v[i] + v[i + 1]);
            num[0][g] += avg(total) * w;
            num[1][g] += avg(elastic) * w;
            num[2][g] += avg(fission) * w;
            num[3][g] += avg(capture) * w;
            num[4][g] += avg(nu_fission) * w;
            den[g] += w;
        }

        let finish = |col: &[f64]| -> Vec<f64> {
            col.iter()
                .zip(&den)
                .map(|(&s, &d)| if d > 0.0 { s / d } else { 0.0 })
                .collect()
        };

        Mgxs {
            name: name.into(),
            group_bounds: group_bounds.to_vec(),
            total: finish(&num[0]),
            elastic: finish(&num[1]),
            fission: finish(&num[2]),
            capture: finish(&num[3]),
            nu_fission: finish(&num[4]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat cross section collapses to itself regardless of the weight: if
    /// σ(E) ≡ c, then ∫σχ/∫χ = c in every group.
    #[test]
    fn flat_xs_collapses_to_constant() {
        let energy: Vec<f64> = (0..=1000).map(|i| 1.0e5 + i as f64 * 1.0e4).collect();
        let flat = vec![7.0; energy.len()];
        let zero = vec![0.0; energy.len()];
        let bounds = vec![1.0e5, 1.0e6, 5.0e6, 1.1e7];
        let mg = Mgxs::collapse_watt(
            "Test", &energy, &flat, &flat, &zero, &zero, &zero, &bounds,
            &FissionSpectrum::default(),
        );
        assert_eq!(mg.n_groups(), 3);
        for g in 0..mg.n_groups() {
            assert!((mg.total[g] - 7.0).abs() < 1e-9, "group {g} total");
            assert!((mg.elastic[g] - 7.0).abs() < 1e-9, "group {g} elastic");
        }
    }

    /// The group lookup is piecewise-constant and clamps outside the range.
    #[test]
    fn micro_is_piecewise_constant_and_clamped() {
        let mg = Mgxs {
            name: "X".into(),
            group_bounds: vec![1.0e6, 2.0e6, 1.0e7],
            total: vec![3.0, 5.0],
            elastic: vec![3.0, 5.0],
            fission: vec![0.0, 0.0],
            capture: vec![0.0, 0.0],
            nu_fission: vec![0.0, 0.0],
        };
        // Below range clamps to group 0; at/above top clamps to top group.
        assert_eq!(mg.micro(1.0e5).total, 3.0);
        assert_eq!(mg.micro(1.5e6).total, 3.0);
        assert_eq!(mg.micro(2.0e6).total, 5.0);
        assert_eq!(mg.micro(9.0e6).total, 5.0);
        assert_eq!(mg.micro(5.0e7).total, 5.0);
    }

    /// Watt-weighting biases a rising σ(E) toward the low end of a group, because
    /// the Watt spectrum peaks near ~0.7 MeV and decays: the weighted average of a
    /// monotonically increasing σ over a group sits below the unweighted midpoint.
    #[test]
    fn watt_weight_biases_toward_spectrum_peak() {
        let energy: Vec<f64> = (0..=2000).map(|i| 1.0e5 + i as f64 * 5.0e3).collect();
        // sigma rises linearly with E
        let sigma: Vec<f64> = energy.iter().map(|&e| e / 1.0e6).collect();
        let zero = vec![0.0; energy.len()];
        let bounds = vec![1.0e5, 1.01e7];
        let mg = Mgxs::collapse_watt(
            "R", &energy, &sigma, &sigma, &zero, &zero, &zero, &bounds,
            &FissionSpectrum::default(),
        );
        let unweighted_mid = 0.5 * (1.0e5 + 1.01e7) / 1.0e6;
        assert!(
            mg.total[0] < unweighted_mid,
            "watt-weighted avg {} should be below unweighted midpoint {}",
            mg.total[0], unweighted_mid
        );
    }

    /// The MGXS variant dispatches through `XsProvider::micro` (temperature is
    /// ignored, as documented).
    #[test]
    fn xsprovider_dispatches_to_mgxs() {
        let mg = Mgxs {
            name: "U238".into(),
            group_bounds: vec![1.0e6, 1.0e7],
            total: vec![7.5],
            elastic: vec![5.0],
            fission: vec![0.5],
            capture: vec![2.0],
            nu_fission: vec![1.3],
        };
        let p = XsProvider::Mgxs(mg);
        let x = p.micro(3.0e6, 900.0);
        assert_eq!(x.total, 7.5);
        assert_eq!(x.nu_fission, 1.3);
        // Temperature-independent.
        assert_eq!(p.micro(3.0e6, 300.0).total, x.total);
    }
}
