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
use crate::NjoyError;
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

/// Upper energy bound of the general-purpose ENDF/B neutron sublibrary \[eV\].
///
/// 20 MeV — the ceiling for the standard ENDF/B-VII/VIII neutron evaluations, and
/// therefore the top of the fast MGXS range. A handful of special high-energy
/// evaluations extend to 150/200 MeV; pass an explicit `e_hi` to
/// [`Mgxs::fast_group_bounds`] for those.
pub const ENDF_MAX_ENERGY_EV: f64 = 2.0e7;

/// Number of energy groups in the standard fast MGXS structure.
///
/// Ten log-spaced groups span each nuclide's WMP `e_max` up to
/// [`ENDF_MAX_ENERGY_EV`]. The fast range is smooth, so ten groups is plenty for
/// the low-fidelity fallback.
pub const FAST_GROUP_COUNT: usize = 10;

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
    /// The standard fast-range group boundaries: [`FAST_GROUP_COUNT`] log-spaced
    /// groups from a nuclide's WMP `e_max` up to `e_hi` \[eV\].
    ///
    /// Returns `FAST_GROUP_COUNT + 1` boundaries, ascending, log-uniform:
    /// `bound[i] = e_lo · (e_hi / e_lo)^(i / n)`. Pass a WMP evaluation's
    /// [`crate::wmp::WindowedMultipole::e_max`] as `e_lo` so the MGXS set begins
    /// exactly where the multipole form ends, and [`ENDF_MAX_ENERGY_EV`] (20 MeV)
    /// as `e_hi` for the standard neutron sublibrary ceiling.
    ///
    /// # Panics
    /// Debug-asserts `0 < e_lo < e_hi` — a log grid needs a positive, ordered span.
    pub fn fast_group_bounds(e_lo: f64, e_hi: f64) -> Vec<f64> {
        debug_assert!(
            e_lo > 0.0 && e_hi > e_lo,
            "fast_group_bounds needs 0 < e_lo ({e_lo}) < e_hi ({e_hi})"
        );
        let n = FAST_GROUP_COUNT;
        let ratio = e_hi / e_lo;
        (0..=n)
            .map(|i| e_lo * ratio.powf(i as f64 / n as f64))
            .collect()
    }

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

    /// Build the standard fast MGXS set directly from **RECONR pointwise output** —
    /// the in-crate bake path for the high-energy fallback.
    ///
    /// Uses the reconstructed lin-lin σ(E) grid ([`ReconrResult`]) as the fine
    /// input, [`FAST_GROUP_COUNT`] log-spaced groups from `e_lo` (the nuclide's WMP
    /// `e_max`) to [`ENDF_MAX_ENERGY_EV`] (20 MeV), and a Watt weight
    /// ([`Self::collapse_watt`]). Channels are taken from the standard MTs:
    /// total = MT 1 (or elastic+fission+capture if MT 1 is absent), elastic = MT 2,
    /// fission = MT 18, capture = MT 102; ν·σ_f folds in `nu` at each energy.
    ///
    /// The fine grid is the union of every section's native energies within
    /// `[e_lo, 20 MeV]` (so σ kinks are represented exactly) plus the group
    /// boundaries (so no group is empty). Faithful to RECONR's pointwise data — no
    /// re-reconstruction, just group collapse.
    pub fn collapse_from_reconr(
        result: &crate::reconr::ReconrResult,
        name: impl Into<String>,
        e_lo: f64,
        nu: &NuBar,
        spectrum: &FissionSpectrum,
    ) -> Self {
        use crate::endf::MtReaction;

        // A non-resonant nuclide's WMP can already span the full sublibrary
        // (`e_max` = 20 MeV, e.g. H-1). There is then no fast gap to fill — return
        // an empty set rather than a degenerate zero-width group grid.
        if e_lo >= ENDF_MAX_ENERGY_EV {
            return Mgxs {
                name: name.into(),
                ..Default::default()
            };
        }

        let bounds = Self::fast_group_bounds(e_lo, ENDF_MAX_ENERGY_EV);
        let e_hi = *bounds.last().unwrap();

        // Fine grid: every RECONR energy in range + the group boundaries.
        let mut grid: Vec<f64> = result
            .sections
            .iter()
            .flat_map(|s| s.pairs.iter().map(|&(e, _)| e))
            .filter(|&e| e >= e_lo && e <= e_hi)
            .collect();
        grid.extend_from_slice(&bounds);
        grid.sort_by(|a, b| a.partial_cmp(b).unwrap());
        grid.dedup();

        let has_total = result
            .sections
            .iter()
            .any(|s| s.mt == MtReaction::Mt1Total);

        let sample = |mt: MtReaction| -> Vec<f64> {
            grid.iter().map(|&e| result.eval_mt(mt, e)).collect()
        };
        let elastic = sample(MtReaction::Mt2Elastic);
        let fission = sample(MtReaction::Mt18Fission);
        let capture = sample(MtReaction::Mt102Capture);
        let total: Vec<f64> = if has_total {
            sample(MtReaction::Mt1Total)
        } else {
            // Synthesize a total when the evaluation omits MT 1.
            (0..grid.len())
                .map(|i| elastic[i] + fission[i] + capture[i])
                .collect()
        };
        let nu_fission: Vec<f64> = grid
            .iter()
            .zip(&fission)
            .map(|(&e, &sf)| sf * nu.at(e))
            .collect();

        Self::collapse_watt(
            name, &grid, &total, &elastic, &fission, &capture, &nu_fission, &bounds,
            spectrum,
        )
    }
}

// ── MGXS container (MGXL v1) ──────────────────────────────────────────────────

const MGXL_MAGIC: [u8; 4] = *b"MGXL";
const MGXL_VERSION: u8 = 1;

/// A bundle of fast-range [`Mgxs`] sets for many nuclides — the embedded
/// high-energy companion to [`crate::wmp::WmpLibrary`].
///
/// Baked offline by the `bake_mgxs` example from the local ENDF/B-VIII.0 tapes
/// (RECONR background → Watt collapse) and shipped in-crate via [`MgxsLibrary::core`].
/// The blob is small (10 groups × 5 channels × a few hundred nuclides ≈ tens of
/// KB), so the **MGXL v1** format is a flat, uncompressed little-endian dump:
///
/// ```text
/// magic "MGXL" | version u8 | reserved [u8;3] | n_nuclides u32
/// per nuclide: name_len u32 | name UTF-8 | n_groups u32
///              (n_groups+1) f64 group_bounds
///              n_groups f64 ×5 columns (total, elastic, fission, capture, nu_fission)
/// ```
#[derive(Debug, Clone, Default)]
pub struct MgxsLibrary {
    nuclides: Vec<Mgxs>,
}

impl MgxsLibrary {
    /// Serialize a set of fast-MGXS nuclides to the MGXL v1 blob (the bake step).
    pub fn pack(nuclides: &[Mgxs]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&MGXL_MAGIC);
        out.push(MGXL_VERSION);
        out.extend_from_slice(&[0u8; 3]); // reserved
        out.extend_from_slice(&(nuclides.len() as u32).to_le_bytes());
        for mg in nuclides {
            let name = mg.name.as_bytes();
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name);
            let ng = mg.n_groups() as u32;
            out.extend_from_slice(&ng.to_le_bytes());
            let put = |out: &mut Vec<u8>, col: &[f64]| {
                for &v in col {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            };
            put(&mut out, &mg.group_bounds);
            put(&mut out, &mg.total);
            put(&mut out, &mg.elastic);
            put(&mut out, &mg.fission);
            put(&mut out, &mg.capture);
            put(&mut out, &mg.nu_fission);
        }
        out
    }

    /// Parse an MGXL v1 blob (owns the decoded nuclides). Hardened: every length is
    /// bounds-checked against the remaining bytes, so a malformed blob fails
    /// cleanly rather than over-reading or runaway-allocating.
    pub fn from_blob(bytes: &[u8]) -> Result<Self, NjoyError> {
        let bad = || NjoyError::WmpData("malformed MGXL blob".into());
        if bytes.len() < 12 || bytes[0..4] != MGXL_MAGIC || bytes[4] != MGXL_VERSION {
            return Err(bad());
        }
        let mut p = 8usize;
        let rd_u32 = |bytes: &[u8], p: &mut usize| -> Result<u32, NjoyError> {
            let end = p.checked_add(4).ok_or_else(bad)?;
            if end > bytes.len() {
                return Err(bad());
            }
            let v = u32::from_le_bytes(bytes[*p..end].try_into().unwrap());
            *p = end;
            Ok(v)
        };
        let rd_f64s = |bytes: &[u8], p: &mut usize, n: usize| -> Result<Vec<f64>, NjoyError> {
            let span = n.checked_mul(8).ok_or_else(bad)?;
            let end = p.checked_add(span).ok_or_else(bad)?;
            if end > bytes.len() {
                return Err(bad());
            }
            let v = bytes[*p..end]
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                .collect();
            *p = end;
            Ok(v)
        };

        let n_nuc = rd_u32(bytes, &mut p)? as usize;
        let mut nuclides = Vec::with_capacity(n_nuc.min(4096));
        for _ in 0..n_nuc {
            let name_len = rd_u32(bytes, &mut p)? as usize;
            let end = p.checked_add(name_len).ok_or_else(bad)?;
            if end > bytes.len() {
                return Err(bad());
            }
            let name = String::from_utf8(bytes[p..end].to_vec()).map_err(|_| bad())?;
            p = end;
            let ng = rd_u32(bytes, &mut p)? as usize;
            let group_bounds = rd_f64s(bytes, &mut p, ng + 1)?;
            let total = rd_f64s(bytes, &mut p, ng)?;
            let elastic = rd_f64s(bytes, &mut p, ng)?;
            let fission = rd_f64s(bytes, &mut p, ng)?;
            let capture = rd_f64s(bytes, &mut p, ng)?;
            let nu_fission = rd_f64s(bytes, &mut p, ng)?;
            nuclides.push(Mgxs {
                name,
                group_bounds,
                total,
                elastic,
                fission,
                capture,
                nu_fission,
            });
        }
        Ok(MgxsLibrary { nuclides })
    }

    /// Number of nuclides.
    pub fn len(&self) -> usize {
        self.nuclides.len()
    }

    /// Whether the library holds no nuclides.
    pub fn is_empty(&self) -> bool {
        self.nuclides.is_empty()
    }

    /// Names of all nuclides, in packed order (e.g. `"U238"`).
    pub fn names(&self) -> Vec<&str> {
        self.nuclides.iter().map(|m| m.name.as_str()).collect()
    }

    /// Whether a nuclide with this name is present.
    pub fn contains(&self, name: &str) -> bool {
        self.nuclides.iter().any(|m| m.name == name)
    }

    /// The fast-MGXS set for `name`, if present.
    pub fn get(&self, name: &str) -> Option<&Mgxs> {
        self.nuclides.iter().find(|m| m.name == name)
    }

    /// The embedded CORE fast-range MGXS library (ENDF/B-VIII.0 MF=3 background,
    /// Watt-collapsed to 10 groups per nuclide from each WMP `e_max` up to 20 MeV).
    ///
    /// Baked by the `bake_mgxs` example and **always embedded** (no feature gate) —
    /// the high-energy companion to [`crate::wmp::WmpLibrary::core`]. The blob is
    /// tens of KB and parsed once, then shared. Look up a nuclide with
    /// [`Self::get`] (e.g. `.get("U238")`).
    pub fn core() -> &'static MgxsLibrary {
        static CORE: std::sync::OnceLock<MgxsLibrary> = std::sync::OnceLock::new();
        CORE.get_or_init(|| {
            MgxsLibrary::from_blob(include_bytes!("../data/mgxs_core.mgxl"))
                .expect("embedded CORE MGXL blob is valid")
        })
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

    /// 10 log-spaced groups from `e_max` to 20 MeV: 11 ascending bounds, endpoints
    /// exact, and log-uniform (constant ratio between successive bounds).
    #[test]
    fn fast_group_bounds_are_log_spaced() {
        let b = Mgxs::fast_group_bounds(2.0e4, ENDF_MAX_ENERGY_EV);
        assert_eq!(b.len(), FAST_GROUP_COUNT + 1);
        assert!((b[0] - 2.0e4).abs() < 1e-6);
        assert!((b[FAST_GROUP_COUNT] - 2.0e7).abs() < 1.0);
        let r0 = b[1] / b[0];
        for i in 1..FAST_GROUP_COUNT {
            assert!((b[i + 1] / b[i] - r0).abs() < 1e-9, "ratio drift at {i}");
        }
    }

    /// The MGXL container is a lossless round trip and rejects a clobbered magic.
    #[test]
    fn mgxl_container_round_trips() {
        let a = Mgxs {
            name: "U238".into(),
            group_bounds: vec![2.0e4, 6.0e5, 2.0e7],
            total: vec![10.0, 7.0],
            elastic: vec![9.0, 5.0],
            fission: vec![0.5, 1.0],
            capture: vec![0.5, 1.0],
            nu_fission: vec![1.2, 2.6],
        };
        let b = Mgxs {
            name: "H1".into(),
            group_bounds: vec![1.0e3, 2.0e7],
            total: vec![1.0],
            elastic: vec![1.0],
            fission: vec![0.0],
            capture: vec![0.0],
            nu_fission: vec![0.0],
        };
        let image = MgxsLibrary::pack(&[a.clone(), b.clone()]);
        let lib = MgxsLibrary::from_blob(&image).unwrap();
        assert_eq!(lib.len(), 2);
        assert!(lib.contains("U238") && lib.contains("H1"));
        let u = lib.get("U238").unwrap();
        assert_eq!(u.group_bounds, a.group_bounds);
        assert_eq!(u.nu_fission, a.nu_fission);
        let mut bad = image.clone();
        bad[0] = b'Z';
        assert!(MgxsLibrary::from_blob(&bad).is_err());
    }

    /// The embedded CORE fast-MGXS library loads and is physically sane: it holds
    /// the nuclides with a real fast gap above their WMP `e_max`, a fissile nuclide
    /// carries ν·σ_f + elastic, and a nuclide whose WMP already spans 20 MeV
    /// (H-1, non-resonant) is *absent* because it has no fast range to fill.
    #[test]
    fn embedded_core_mgxs_loads_and_is_sane() {
        let lib = MgxsLibrary::core();
        assert!(!lib.is_empty(), "CORE fast-MGXS set is non-empty");
        assert!(lib.len() <= 125, "at most the CORE set");

        // U-238: WMP e_max ≈ 20 keV, so a real fast range up to 20 MeV.
        let u = lib.get("U238").expect("U238 has a fast range");
        assert_eq!(u.n_groups(), FAST_GROUP_COUNT);
        assert!((*u.group_bounds.last().unwrap() - 2.0e7).abs() < 1.0);
        assert!(u.elastic.iter().any(|&x| x > 0.0), "U238 fast elastic present");
        // Fast fission opens above ~1 MeV → some group carries ν·σ_f.
        assert!(u.nu_fission.iter().any(|&x| x > 0.0), "U238 fast ν·σ_f present");

        // H-1 is non-resonant; its WMP covers the whole sublibrary, so it is not in
        // the fast-MGXS container.
        assert!(!lib.contains("H1"), "H1 has no fast range (WMP covers it)");
    }
}
