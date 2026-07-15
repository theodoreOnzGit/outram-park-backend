// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Unresolved-resonance-range (URR) self-shielding + infinite-medium weighting
//! flux — the Bondarenko narrow-resonance flux and the `getunr` self-shielded
//! cross-section table.
//!
//! # What this computes
//!
//! When GROUPR runs with more than one background dilution (`nsigz > 1`) the
//! group constants are *self-shielded*: each group average is weighted by a flux
//! that dips inside resonances. Two pieces produce that:
//!
//! 1. The **weighting flux** `phi(E; sigma_0)`. In the narrow-resonance
//!    (Bondarenko) model used above the resolved range,
//!
//!    ```text
//!    phi(E; sigma_0) = C(E) * (sigma_0 + sigma_pot) / (sigma_t(E) + sigma_0)
//!    ```
//!
//!    where `C(E)` is the smooth asymptotic weight, `sigma_t(E)` the total cross
//!    section \[barn\], `sigma_0` the background dilution \[barn\], and
//!    `sigma_pot` the potential-scattering cross section \[barn\] (the
//!    high-energy asymptote of `sigma_t`). This is exactly the P0 flux GROUPR's
//!    `genflx` writes for the Bondarenko model (`groupr.f90:5650,5653`):
//!    `fac = (sigpot+sigz)/(tt+sigz)`, `fout = wtf*fac`. The per-dilution
//!    normalization `(sigma_0 + sigma_pot)` cancels in any flux-weighted group
//!    average, so it does not change a group constant; it is kept so the
//!    `sigma_0 -> inf` limit equals `C(E)` exactly (see the tests).
//!
//! 2. The URR **self-shielded cross sections** themselves, tabulated by
//!    (temperature, dilution, energy) on the PENDF MF=2/MT=152 record and
//!    retrieved with sigma-zero + energy interpolation.
//!
//! # Fortran routine -> Rust item map (NJOY2016 `groupr.f90`)
//!
//! | Fortran | lines | Rust |
//! |---|---|---|
//! | `genflx` (Bondarenko P0 flux) | 5309-5684 | [`bondarenko_flux_value`], [`genflx_bondarenko`] |
//! | `stounr` (store URR table) | 6802-6894 | [`UnresolvedTable::store`] |
//! | `getunr` (retrieve + self-shield) | 6896-6994 | [`UnresolvedTable::shield`] |
//! | `terpu` (sigma-zero interpolation) | 6996-7036 | [`terpu`] (internal) |
//!
//! # Scope / gaps (honest — AI draft, untrusted until reviewed)
//!
//! - **DONE:** the Bondarenko flux formula + its energy-grid handling
//!   ([`genflx_bondarenko`]); the `getunr` sigma-zero/energy interpolation
//!   ([`UnresolvedTable::shield`], [`terpu`]) with both the additive (`lssf==0`)
//!   and multiplicative (`lssf>0`) self-shielding conventions.
//! - **NOT PORTED — PENDF URR-tape reading.** `stounr`'s `findf`/`contio`/
//!   `listio` navigation of the MF=2/MT=152 record (`groupr.f90:6822-6875`) is a
//!   separate ENDF-feeder concern. [`UnresolvedTable::store`] accepts an
//!   *already-parsed* set of points; [`read_urr_from_pendf`] is the documented
//!   boundary marker returning [`NjoyError::NotPorted`].
//! - **NOT PORTED — the slowing-down flux branch of `genflx`** (`nflmax > 0`,
//!   `groupr.f90:5396-5620`): the iterative integral-slowing-down solve with
//!   heterogeneity/admixed-moderator terms (`alpha`, `beta`, `sam`, `gamma`).
//!   Only the Bondarenko narrow-resonance branch is ported.
//! - **NOT PORTED — resolved/unresolved overlap (`iovl`, `xtot`)**
//!   (`groupr.f90:6961-6989`): the negative-energy overlap flag and the
//!   cross-reaction `xtot` background addition use module-global state carried
//!   between reaction calls. [`UnresolvedTable`] records the per-energy overlap
//!   flag but [`UnresolvedTable::shield`] does **not** apply the `xtot`
//!   correction — it evaluates the `iovl == 0` path. Flagged as a follow-up.

use std::sync::Arc;

use crate::endf::interp::{terp1, IntLaw};
use crate::groupr::panel::{GroupFlux, PointwiseXs};
use crate::NjoyError;

// ===========================================================================
// genflx — Bondarenko narrow-resonance weighting flux
// ===========================================================================

/// Bondarenko narrow-resonance weighting flux `phi(E; sigma_0)` at one energy,
/// in the same (arbitrary, shape-only) units as the weight `C(E)`.
///
/// Mirrors the P0 Bondarenko flux `genflx` stores (`groupr.f90:5650,5653`):
/// `phi = C(E) * (sigma_0 + sigma_pot) / (sigma_t(E) + sigma_0)`.
///
/// # Parameters
/// - `sigma_t` — total cross section `sigma_t(E)` \[barn\], `>= 0`.
/// - `weight_c` — smooth asymptotic weight `C(E)` (arbitrary units).
/// - `sigma_pot` — potential-scattering cross section \[barn\] (the high-energy
///   asymptote of `sigma_t`); makes the flux tend to `C(E)` at high dilution.
/// - `sigma_0` — background dilution \[barn\], `>= 0`. Pass `f64::INFINITY` for
///   the infinite-dilution (unshielded) limit.
///
/// # Limits (both exercised by the tests)
/// - `sigma_0 -> inf`: `phi -> C(E)` (flat asymptotic weight, unshielded).
/// - `sigma_0 -> 0`:   `phi -> C(E) * sigma_pot / sigma_t(E)`, i.e. the
///   narrow-resonance `1 / sigma_t` shape.
///
/// Returns `weight_c` for the degenerate `sigma_t + sigma_0 == 0` case rather
/// than a `NaN`.
pub fn bondarenko_flux_value(sigma_t: f64, weight_c: f64, sigma_pot: f64, sigma_0: f64) -> f64 {
    if sigma_0.is_infinite() {
        return weight_c;
    }
    let denom = sigma_t + sigma_0;
    if denom <= 0.0 {
        return weight_c;
    }
    weight_c * (sigma_0 + sigma_pot) / denom
}

/// A set of self-shielded weighting fluxes, one per background dilution — the
/// output of the Bondarenko branch of `genflx`.
///
/// Each flux is a [`GroupFlux::Tabulated`] the panel integrator
/// ([`crate::groupr::panel::group_integral`]) can weight a cross section with.
/// `dilutions[i]` is the background `sigma_0` \[barn\] that produced
/// `fluxes[i]`.
#[derive(Debug, Clone)]
pub struct SelfShieldedFluxSet {
    /// Background dilutions `sigma_0` \[barn\], in the order supplied.
    pub dilutions: Vec<f64>,
    /// Parallel weighting fluxes `phi(E; sigma_0)` (shape-only, arbitrary units).
    pub fluxes: Vec<GroupFlux>,
}

impl SelfShieldedFluxSet {
    /// The weighting flux for the `i`-th dilution, or `None` if out of range.
    pub fn flux(&self, i: usize) -> Option<&GroupFlux> {
        self.fluxes.get(i)
    }
}

/// Build the Bondarenko self-shielded weighting fluxes on an explicit energy
/// grid — the narrow-resonance branch of `genflx` (`groupr.f90:5623-5665`).
///
/// For each dilution `sigma_0` this tabulates
/// `phi(E; sigma_0) = C(E) * (sigma_0 + sigma_pot) / (sigma_t(E) + sigma_0)`
/// at every grid energy and wraps it as a [`GroupFlux::Tabulated`]. Put the
/// grid on the `sigma_t` break points (the resonance structure) so the tabulated
/// flux resolves the resonance dips; the panel integrator then marches those
/// break points exactly.
///
/// # Parameters
/// - `sigma_t` — pointwise total cross section \[barn vs eV\].
/// - `weight` — smooth asymptotic weight `C(E)` (any [`GroupFlux`]; a
///   [`GroupFlux::Flat`] gives `C(E) = 1`).
/// - `sigma_pot` — potential-scattering cross section \[barn\].
/// - `dilutions` — background `sigma_0` values \[barn\] (`>= 0`;
///   `f64::INFINITY` allowed for infinite dilution).
/// - `energy_grid` — **ascending** energies \[eV\], at least 2 points, spanning
///   the group(s) the fluxes will weight.
///
/// # Errors
/// [`NjoyError::EndfParse`] if the grid has fewer than 2 points or is not
/// strictly ascending.
pub fn genflx_bondarenko(
    sigma_t: &PointwiseXs,
    weight: &GroupFlux,
    sigma_pot: f64,
    dilutions: &[f64],
    energy_grid: &[f64],
) -> Result<SelfShieldedFluxSet, NjoyError> {
    if energy_grid.len() < 2 {
        return Err(NjoyError::EndfParse(
            "genflx_bondarenko: energy grid needs >= 2 points".into(),
        ));
    }
    for w in energy_grid.windows(2) {
        if !(w[1] > w[0]) {
            return Err(NjoyError::EndfParse(
                "genflx_bondarenko: energy grid must be strictly ascending".into(),
            ));
        }
    }

    let mut fluxes = Vec::with_capacity(dilutions.len());
    for &s0 in dilutions {
        let mut tab = Vec::with_capacity(energy_grid.len());
        for &e in energy_grid {
            let st = sigma_t.value(e);
            let c = weight.value(e);
            tab.push((e, bondarenko_flux_value(st, c, sigma_pot, s0)));
        }
        fluxes.push(GroupFlux::Tabulated(Arc::new(tab)));
    }

    Ok(SelfShieldedFluxSet {
        dilutions: dilutions.to_vec(),
        fluxes,
    })
}

// ===========================================================================
// stounr / getunr — URR self-shielded cross-section table
// ===========================================================================

/// The self-shielding-flag `lssf` (`stounr` reads it from `l1h`,
/// `groupr.f90:6849`) that decides how a stored URR cross section is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LssfFlag {
    /// `lssf == 0`: the stored table holds **partial cross sections**. The
    /// smooth-file background is corrected additively:
    /// `sig <- sig + s - sinf` (`groupr.f90:6967,6988`).
    Additive,
    /// `lssf > 0`: the stored table holds **self-shielding factors**. The
    /// smooth-file background is scaled: `sig <- sig * s / sinf`
    /// (`groupr.f90:6968,6989`). No change when `sinf == 0`.
    Multiplicative,
}

/// A URR reaction channel, mapping an ENDF MT to the `getunr` column index `ix`
/// (`groupr.f90:6934-6940`).
///
/// The stored table keeps `n_reactions` columns; a reaction whose index exceeds
/// the stored count is treated as "not present".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrrReaction {
    /// MT=1, total (`ix = 1`).
    Total,
    /// MT=2, elastic (`ix = 2`).
    Elastic,
    /// MT=18/19, fission (`ix = 3`).
    Fission,
    /// MT=102, radiative capture (`ix = 4`).
    Capture,
    /// MT=261, current-weighted / transport (`ix = 5`).
    Current,
}

impl UrrReaction {
    /// Map an ENDF MT number to a URR reaction, or `None` for an MT with no URR
    /// column (mirrors the `ix == 0` early return in `getunr`,
    /// `groupr.f90:6940`).
    pub fn from_mt(mt: i32) -> Option<Self> {
        match mt {
            1 => Some(UrrReaction::Total),
            2 => Some(UrrReaction::Elastic),
            18 | 19 => Some(UrrReaction::Fission),
            102 => Some(UrrReaction::Capture),
            261 => Some(UrrReaction::Current),
            _ => None,
        }
    }

    /// Zero-based column index into a stored energy point's reaction table
    /// (`ix - 1` in `getunr`).
    pub fn column(self) -> usize {
        match self {
            UrrReaction::Total => 0,
            UrrReaction::Elastic => 1,
            UrrReaction::Fission => 2,
            UrrReaction::Capture => 3,
            UrrReaction::Current => 4,
        }
    }
}

/// One stored URR energy point: the self-shielded cross sections at a single
/// energy for every reaction and every stored dilution.
///
/// `xs[ix][is]` is the cross section \[barn\] for reaction column `ix` at stored
/// dilution `is` (index into [`UnresolvedTable::sigma0_grid`]). This is the
/// already-parsed layout `stounr` would fill from the PENDF LIST body; parsing
/// that tape is out of scope (see [`read_urr_from_pendf`]).
#[derive(Debug, Clone)]
pub struct UrrEnergyPoint {
    /// Energy \[eV\] of this URR point (must be positive; ascending across the
    /// table).
    pub energy_ev: f64,
    /// Resolved/unresolved overlap flag (the negative-energy marker in NJOY,
    /// `groupr.f90:6961`). Recorded but not applied — see the module gap note.
    pub overlap: bool,
    /// `xs[reaction_column][dilution_index]` \[barn\].
    pub xs: Vec<Vec<f64>>,
}

/// A stored URR self-shielded cross-section table, indexed by
/// (dilution, energy, reaction) — the `unr(*)` array `stounr` builds and
/// `getunr` reads.
///
/// Read-only after construction; the heavy grids are shared via [`Arc`].
#[derive(Debug, Clone)]
pub struct UnresolvedTable {
    /// Temperature \[K\] this table was stored at (`temz`, `groupr.f90:6851`).
    pub temperature_k: f64,
    /// How stored cross sections are applied (`lssf`).
    pub lssf: LssfFlag,
    /// Energy interpolation law between URR points (`intunr`,
    /// `groupr.f90:6848`).
    pub interp: IntLaw,
    /// Stored dilution grid `sigma_0` \[barn\] (`sigs`, `unr(7..)`). Typically
    /// leads with a very large value for infinite dilution.
    sigma0_grid: Arc<Vec<f64>>,
    /// URR energy points, ascending in energy.
    points: Arc<Vec<UrrEnergyPoint>>,
    /// Number of reaction columns stored (`nx`).
    n_reactions: usize,
}

/// The result of a `getunr` self-shielding lookup.
#[derive(Debug, Clone, PartialEq)]
pub struct UnrShielded {
    /// Self-shielded cross section \[barn\], one entry per requested run
    /// dilution (the modified `sig(*)`).
    pub sig: Vec<f64>,
    /// Next URR energy \[eV\] the caller should step to (`getunr` `enext`), or
    /// `None` when `e` is outside the URR range (`enext = -1`,
    /// `groupr.f90:6919`).
    pub next_energy: Option<f64>,
}

/// Relative tolerances: `eps` for the last-point energy nudge and `del` for the
/// energy-range guards in `getunr` (`groupr.f90:6914-6915`).
const EPS: f64 = 1.0e-4;
const DEL: f64 = 1.0e-10;

impl UnresolvedTable {
    /// Store an already-parsed URR self-shielded cross-section table — the
    /// data-holding half of `stounr` (`groupr.f90:6846-6875`) without the PENDF
    /// tape navigation (see [`read_urr_from_pendf`]).
    ///
    /// # Parameters
    /// - `temperature_k` — temperature \[K\] of the stored data.
    /// - `lssf` — self-shielding flag (see [`LssfFlag`]).
    /// - `interp` — energy interpolation law (`intunr`).
    /// - `sigma0_grid` — stored dilution grid \[barn\] (at least 1 value).
    /// - `points` — URR energy points, **ascending** in energy; every point
    ///   must carry the same number of reaction columns, each with one value
    ///   per stored dilution.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if the grids are empty, the energies are not
    /// ascending/positive, or a point's shape does not match the dilution grid.
    pub fn store(
        temperature_k: f64,
        lssf: LssfFlag,
        interp: IntLaw,
        sigma0_grid: Vec<f64>,
        points: Vec<UrrEnergyPoint>,
    ) -> Result<Self, NjoyError> {
        if sigma0_grid.is_empty() {
            return Err(NjoyError::EndfParse(
                "stounr: sigma-zero grid is empty".into(),
            ));
        }
        if points.is_empty() {
            return Err(NjoyError::EndfParse("stounr: no URR energy points".into()));
        }
        let ns = sigma0_grid.len();
        let n_reactions = points[0].xs.len();
        if n_reactions == 0 {
            return Err(NjoyError::EndfParse(
                "stounr: first point has no reaction columns".into(),
            ));
        }
        let mut last_e = f64::NEG_INFINITY;
        for (ip, p) in points.iter().enumerate() {
            if !(p.energy_ev > 0.0) {
                return Err(NjoyError::EndfParse(format!(
                    "stounr: point {ip} energy {} is not positive",
                    p.energy_ev
                )));
            }
            if p.energy_ev <= last_e {
                return Err(NjoyError::EndfParse(format!(
                    "stounr: energies must ascend (point {ip} = {} <= {last_e})",
                    p.energy_ev
                )));
            }
            last_e = p.energy_ev;
            if p.xs.len() != n_reactions {
                return Err(NjoyError::EndfParse(format!(
                    "stounr: point {ip} has {} reaction columns, expected {n_reactions}",
                    p.xs.len()
                )));
            }
            for (ix, col) in p.xs.iter().enumerate() {
                if col.len() != ns {
                    return Err(NjoyError::EndfParse(format!(
                        "stounr: point {ip} reaction {ix} has {} dilutions, expected {ns}",
                        col.len()
                    )));
                }
            }
        }

        Ok(UnresolvedTable {
            temperature_k,
            lssf,
            interp,
            sigma0_grid: Arc::new(sigma0_grid),
            points: Arc::new(points),
            n_reactions,
        })
    }

    /// The stored dilution grid `sigma_0` \[barn\].
    pub fn sigma0_grid(&self) -> &[f64] {
        &self.sigma0_grid
    }

    /// Number of stored URR energy points (`nunr`).
    pub fn n_points(&self) -> usize {
        self.points.len()
    }

    /// Retrieve the self-shielded cross section at energy `e` and correct the
    /// per-dilution background `sig` — the port of `getunr`
    /// (`groupr.f90:6896-6994`), `iovl == 0` path.
    ///
    /// For each requested run dilution `run_dilutions[k]` the stored table is
    /// interpolated over sigma-zero (via [`terpu`]) at the two bracketing URR
    /// energies, then interpolated in energy (via [`terp1`] under
    /// [`Self::interp`]). The result `s` and the infinite-dilution reference
    /// `sinf` (the first stored dilution column) correct the incoming background
    /// per [`LssfFlag`].
    ///
    /// # Parameters
    /// - `reaction` — which reaction column to read.
    /// - `e` — energy \[eV\].
    /// - `background_sig` — incoming smooth-file cross section \[barn\], one per
    ///   run dilution (the `sig(*)` `getunr` modifies).
    /// - `run_dilutions` — the run's dilution grid `sigz(*)` \[barn\]
    ///   (same length as `background_sig`).
    ///
    /// # Returns
    /// [`UnrShielded`] with the corrected `sig` and the next URR energy. If `e`
    /// is outside the URR range, the reaction has no stored column, or the
    /// dilution grid is trivial (`<= 1`, matching `nsig0 <= 1`,
    /// `groupr.f90:6924`), `sig` is returned unchanged with `next_energy = None`.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if `background_sig` and `run_dilutions` differ in
    /// length, or an interpolation fails.
    pub fn shield(
        &self,
        reaction: UrrReaction,
        e: f64,
        background_sig: &[f64],
        run_dilutions: &[f64],
    ) -> Result<UnrShielded, NjoyError> {
        if background_sig.len() != run_dilutions.len() {
            return Err(NjoyError::EndfParse(
                "getunr: background_sig and run_dilutions differ in length".into(),
            ));
        }
        let unchanged = || UnrShielded {
            sig: background_sig.to_vec(),
            next_energy: None,
        };

        // nsig0 <= 1: no sigma-zero data to interpolate (groupr.f90:6924).
        if self.sigma0_grid.len() <= 1 {
            return Ok(unchanged());
        }
        // Reaction column present?
        let ix = reaction.column();
        if ix >= self.n_reactions {
            return Ok(unchanged());
        }
        // Energy inside the URR range? (groupr.f90:6927-6930)
        let e1 = self.points[0].energy_ev;
        let enunr = self.points[self.points.len() - 1].energy_ev;
        if e < e1 - DEL * e1 || e > enunr + DEL * enunr {
            return Ok(unchanged());
        }

        let sigs = &self.sigma0_grid;
        let combine = |bg: f64, s: f64, sinf: f64| -> f64 {
            match self.lssf {
                LssfFlag::Additive => bg + s - sinf,
                LssfFlag::Multiplicative => {
                    if sinf != 0.0 {
                        bg * s / sinf
                    } else {
                        bg
                    }
                }
            }
        };

        // Find the interpolation interval [i, i+1] with elo < e < ehi
        // (groupr.f90:6944-6956).
        let mut interval = None;
        for i in 0..self.points.len() - 1 {
            let elo = self.points[i].energy_ev;
            let elo = elo - DEL * elo;
            let ehi = self.points[i + 1].energy_ev;
            if e > elo && e < ehi {
                interval = Some(i);
                break;
            }
        }

        match interval {
            None => {
                // Special exit for the last point (groupr.f90:6958-6970).
                let last = &self.points[self.points.len() - 1];
                let col = &last.xs[ix];
                let sinf = col[0];
                let mut sig = Vec::with_capacity(run_dilutions.len());
                for (k, &sigb) in run_dilutions.iter().enumerate() {
                    let s = terpu(sigb, col, sigs);
                    sig.push(combine(background_sig[k], s, sinf));
                }
                Ok(UnrShielded {
                    sig,
                    next_energy: Some(enunr * (1.0 + EPS)),
                })
            }
            Some(i) => {
                // Interpolate for cross sections requested (groupr.f90:6973-6991).
                let plo = &self.points[i];
                let phi = &self.points[i + 1];
                let el = plo.energy_ev;
                let en = phi.energy_ev;
                let col_lo = &plo.xs[ix];
                let col_hi = &phi.xs[ix];
                // sinf: energy interpolation of the infinite-dilution (first) column.
                let sinf = terp1(el, col_lo[0], en, col_hi[0], e, self.interp)?;
                let mut sig = Vec::with_capacity(run_dilutions.len());
                for (k, &sigb) in run_dilutions.iter().enumerate() {
                    let sl = terpu(sigb, col_lo, sigs);
                    let sn = terpu(sigb, col_hi, sigs);
                    let s = terp1(el, sl, en, sn, e, self.interp)?;
                    sig.push(combine(background_sig[k], s, sinf));
                }
                Ok(UnrShielded {
                    sig,
                    next_energy: Some(en),
                })
            }
        }
    }
}

/// Interpolate a URR cross section to a desired sigma-zero — the port of
/// `terpu` (`groupr.f90:6996-7036`).
///
/// Given the stored dilution grid `sigs` \[barn\] and the cross sections `xs`
/// \[barn\] at each stored dilution, returns the cross section at `sigz`
/// \[barn\]. The scheme is a product (Lagrange-style) interpolation that switches
/// form by regime: quadratic in `sigma^2` for `sigz < sigs[ns-1]`, logarithmic
/// near `sigs[1]`, and `1/sigma` for the dilute tail. At a stored node
/// (`sigz == sigs[k]`) it reproduces `xs[k]` exactly.
///
/// `xs` and `sigs` must have equal length `>= 1`.
pub fn terpu(sigz: f64, xs: &[f64], sigs: &[f64]) -> f64 {
    // Fortran constants (groupr.f90:7007-7008).
    const RLO: f64 = 0.99;
    const RHI: f64 = 10.1;
    let ns = sigs.len();
    if ns == 0 {
        return 0.0;
    }
    if ns == 1 {
        return xs[0];
    }
    // Fortran is 1-based: sigs(2) -> sigs[1], sigs(ns) -> sigs[ns-1].
    let s_last = sigs[ns - 1];
    let s_two = sigs[1];

    let mut sig = 0.0;
    for is in 0..ns {
        let sm = sigs[is];
        // Skip conditions (groupr.f90:7014-7016).
        if sigz < s_last && sm > RHI * s_last {
            continue;
        }
        if sigz < s_two && sm > RHI * s_two {
            continue;
        }
        if sigz >= s_two && sm < RLO * s_two {
            continue;
        }
        let mut terp = 1.0_f64;
        for js in 0..ns {
            let st = sigs[js];
            if (sm - st).abs() < sm / 1000.0 {
                continue; // same node (groupr.f90:7019)
            }
            if sigz >= s_last {
                // label 130 (groupr.f90:7024-7028)
                if sigz >= s_two {
                    // label 140 (groupr.f90:7029-7031)
                    if st < RLO * s_two {
                        continue;
                    }
                    terp *= ((st / sigz) - 1.0) / ((st / sm) - 1.0);
                } else {
                    if st > RHI * s_two {
                        continue;
                    }
                    terp *= (sigz / st).ln() / (sm / st).ln();
                }
            } else {
                // path before label 130 (groupr.f90:7020-7023)
                if st > RHI * s_last {
                    continue;
                }
                terp *= (sigz * sigz - st * st) / (sm * sm - st * st);
            }
        }
        sig += terp * xs[is];
    }
    sig
}

/// Read a URR self-shielded cross-section table from a PENDF tape (MF=2/MT=152)
/// — **not ported** (documented boundary).
///
/// Upstream `stounr` navigates the tape with `findf`/`contio`/`tosend`/`listio`
/// (`groupr.f90:6822-6875`) to locate MAT + temperature and read the LIST body.
/// That ENDF-record reading is a separate feeder concern; this port takes an
/// already-parsed table through [`UnresolvedTable::store`]. Returns
/// [`NjoyError::NotPorted`] tagged `"groupr::stounr (PENDF URR-tape reading)"`.
pub fn read_urr_from_pendf() -> Result<UnresolvedTable, NjoyError> {
    Err(NjoyError::NotPorted(
        "groupr::stounr (PENDF URR-tape reading)",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- genflx / Bondarenko flux ------------------------------------------

    /// **Flux limits.** The Bondarenko flux tends to the flat weight `C(E)` at
    /// high dilution and to the narrow-resonance `1/sigma_t` shape at zero
    /// dilution.
    ///
    /// **Methodology.** With `C = 1`, `sigma_pot = 5 barn`, hand-checkable
    /// `sigma_t in {2, 50} barn`:
    /// - `sigma_0 = inf` -> `phi = 1` exactly (assert both energies).
    /// - `sigma_0 = 1e12`  -> `phi -> 1` (assert |phi - 1| < 1e-9).
    /// - `sigma_0 = 0`   -> `phi = sigma_pot / sigma_t = 5/2 = 2.5` and
    ///   `5/50 = 0.1`; check `phi * sigma_t == sigma_pot` (the `1/sigma_t` law).
    ///
    /// **Result (2026-07-15, commit ac5adf5).** inf -> 1.0 (exact);
    /// 1e12 -> 1.0 within 1e-9; s0=0 -> 2.5 and 0.1 (phi*sigma_t = 5.0 both,
    /// deviation < 1e-13).
    #[test]
    fn bondarenko_flux_limits() {
        let spot = 5.0;
        for &st in &[2.0_f64, 50.0] {
            // infinite dilution -> flat weight C = 1
            assert_eq!(bondarenko_flux_value(st, 1.0, spot, f64::INFINITY), 1.0);
            // very large dilution -> approaches C = 1
            let big = bondarenko_flux_value(st, 1.0, spot, 1.0e12);
            assert!((big - 1.0).abs() < 1e-9, "sigma_t={st}: big-dilution phi={big}");
            // zero dilution -> C * sigma_pot / sigma_t (the 1/sigma_t NR shape)
            let nr = bondarenko_flux_value(st, 1.0, spot, 0.0);
            assert!((nr - spot / st).abs() < 1e-13, "sigma_t={st}: nr phi={nr}");
            assert!((nr * st - spot).abs() < 1e-13, "1/sigma_t law broken");
        }
    }

    /// **Self-shielding monotonicity.** The flux-weighted group average of a
    /// resonance-shaped `sigma_t` *decreases* as `sigma_0` decreases (more
    /// self-shielding depresses the flux inside the resonance peak, down-weighting
    /// the large cross section there).
    ///
    /// **Methodology.** `sigma_t(E)` is a triangular resonance on `[0, 100] eV`:
    /// 1 barn at the edges rising to 101 barn at 50 eV, tabulated on an 11-point
    /// grid. Weight `C = 1`, `sigma_pot = 1 barn`. Build the Bondarenko flux with
    /// [`genflx_bondarenko`] for dilutions
    /// `sigma_0 in {inf, 1e4, 100, 10, 1, 0.1} barn` and group-average `sigma_t`
    /// over the whole range with [`crate::groupr::panel::group_integral`].
    /// Assert the averages are strictly increasing in `sigma_0` and that the
    /// `sigma_0 = inf` average equals the unweighted trapezoid mean.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** measured group averages \[barn\]
    /// were strictly increasing with `sigma_0` (dilution -> avg): 0.1 < 1 < 10 <
    /// 100 < 1e4 < inf, and the infinite-dilution value matched the flat-weight
    /// trapezoid mean (51.0 barn) within 1e-6. Exact printed values are asserted
    /// by the ordering checks below rather than hard-coded.
    #[test]
    fn self_shielding_monotone_in_dilution() {
        // Triangular resonance: 1 barn -> 101 barn (at 50 eV) -> 1 barn.
        let mut pairs = Vec::new();
        for k in 0..=10 {
            let e = k as f64 * 10.0;
            let peak = 1.0 + 100.0 * (1.0 - ((e - 50.0) / 50.0).abs());
            pairs.push((e, peak));
        }
        let grid: Vec<f64> = pairs.iter().map(|p| p.0).collect();
        let sigma_t = PointwiseXs::LinLin(Arc::new(pairs));

        let dilutions = [f64::INFINITY, 1.0e4, 1.0e2, 1.0e1, 1.0, 0.1];
        let set = genflx_bondarenko(&sigma_t, &GroupFlux::Flat, 1.0, &dilutions, &grid)
            .expect("flux build");

        let avgs: Vec<f64> = set
            .fluxes
            .iter()
            .map(|f| crate::groupr::panel::group_integral(&sigma_t, f, 0.0, 100.0).average())
            .collect();

        // Strictly increasing with sigma_0 (dilutions listed descending).
        for w in avgs.windows(2) {
            assert!(w[0] > w[1] + 1e-6, "monotonicity broken: {avgs:?}");
        }
        // Infinite dilution -> flat-weight (unshielded) trapezoid mean.
        let flat =
            crate::groupr::panel::group_integral(&sigma_t, &GroupFlux::Flat, 0.0, 100.0).average();
        assert!(
            (avgs[0] - flat).abs() < 1e-6,
            "infinite-dilution avg {} != flat mean {flat}",
            avgs[0]
        );
    }

    // -- getunr / terpu ----------------------------------------------------

    /// **`terpu` reproduces stored sigma-zero nodes exactly.**
    ///
    /// **Methodology.** With a 4-node dilution grid `sigs = [1e10, 100, 10, 1]`
    /// and cross sections `xs = [1.0, 2.0, 5.0, 12.0]`, assert `terpu(sigs[k]) ==
    /// xs[k]` for every node, and that a value between two nodes lands between the
    /// neighbouring cross sections (monotone stored data).
    ///
    /// **Result (2026-07-15, commit ac5adf5).** all four nodes reproduced within
    /// 1e-10; `terpu(50)` (between sigs=100 and 10) lands inside (2.0, 5.0).
    #[test]
    fn terpu_reproduces_nodes_and_interpolates() {
        let sigs = [1.0e10, 100.0, 10.0, 1.0];
        let xs = [1.0, 2.0, 5.0, 12.0];
        for k in 0..sigs.len() {
            let v = terpu(sigs[k], &xs, &sigs);
            assert!((v - xs[k]).abs() < 1e-10, "node {k}: terpu={v}, xs={}", xs[k]);
        }
        let mid = terpu(50.0, &xs, &sigs); // between sigs=100 (2.0) and 10 (5.0)
        assert!((2.0..=5.0).contains(&mid), "terpu(50)={mid} out of (2,5)");
    }

    /// Build a small single-reaction URR table for the `shield` tests.
    ///
    /// sigma-zero grid `[1e10, 100, 10, 1]`; energies 10/20/30 eV; the total
    /// cross section grows with energy and with self-shielding (smaller sigma_0).
    fn sample_table(lssf: LssfFlag) -> UnresolvedTable {
        let sigma0 = vec![1.0e10, 100.0, 10.0, 1.0];
        // xs[reaction=0][dilution] per energy point.
        let points = vec![
            UrrEnergyPoint {
                energy_ev: 10.0,
                overlap: false,
                xs: vec![vec![10.0, 9.0, 7.0, 4.0]],
            },
            UrrEnergyPoint {
                energy_ev: 20.0,
                overlap: false,
                xs: vec![vec![20.0, 18.0, 14.0, 8.0]],
            },
            UrrEnergyPoint {
                energy_ev: 30.0,
                overlap: false,
                xs: vec![vec![30.0, 27.0, 21.0, 12.0]],
            },
        ];
        UnresolvedTable::store(300.0, lssf, IntLaw::LinLin, sigma0, points).expect("store")
    }

    /// **`getunr` reproduces stored table points exactly (additive `lssf==0`).**
    ///
    /// **Methodology.** With the additive convention `sig <- sig + s - sinf`,
    /// feed the infinite-dilution value as the incoming background so the
    /// correction returns the raw self-shielded value `s`. At a stored energy
    /// (20 eV) and a stored dilution (100 barn) the returned cross section must
    /// equal the stored `xs = 18.0`; at 10 eV and dilution 1 barn it must equal
    /// `4.0`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** e=20, sigma_0=100 -> 18.0
    /// (deviation < 1e-12); e=10, sigma_0=1 -> 4.0 (deviation < 1e-12).
    #[test]
    fn getunr_reproduces_stored_points_additive() {
        let t = sample_table(LssfFlag::Additive);
        // sinf at e=20 is the infinite-dilution (first) column = 20.0.
        let out = t
            .shield(UrrReaction::Total, 20.0, &[20.0], &[100.0])
            .expect("shield");
        assert!((out.sig[0] - 18.0).abs() < 1e-12, "sig={:?}", out.sig);

        // sinf at e=10 (first column) = 10.0; background = 10.0.
        let out2 = t
            .shield(UrrReaction::Total, 10.0, &[10.0], &[1.0])
            .expect("shield");
        assert!((out2.sig[0] - 4.0).abs() < 1e-12, "sig={:?}", out2.sig);
    }

    /// **`getunr` interpolates monotonically in energy between stored points.**
    ///
    /// **Methodology.** Additive convention, incoming background = 0 so the
    /// result is `s - sinf`; recover `s` by adding back `sinf` (which equals the
    /// energy in eV here, since the first column is `[10, 20, 30]`). Query at 10,
    /// 15, 20 eV for dilution 10 barn. Because the stored cross section increases
    /// with energy, `s(15)` must lie strictly between `s(10)` and `s(20)`, and be
    /// the exact lin-lin midpoint.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** s(10)=7.0, s(15)=10.5, s(20)=14.0
    /// — strictly increasing; 10.5 is the exact lin-lin midpoint of 7.0 and 14.0.
    #[test]
    fn getunr_energy_interpolation_monotone() {
        let t = sample_table(LssfFlag::Additive);
        let query = |e: f64| -> f64 {
            let out = t.shield(UrrReaction::Total, e, &[0.0], &[10.0]).unwrap();
            let sinf = e; // first column equals the energy in eV here
            out.sig[0] + sinf
        };
        let s10 = query(10.0);
        let s15 = query(15.0);
        let s20 = query(20.0);
        assert!((s10 - 7.0).abs() < 1e-12, "s10={s10}");
        assert!((s20 - 14.0).abs() < 1e-12, "s20={s20}");
        assert!(s10 < s15 && s15 < s20, "not monotone: {s10}, {s15}, {s20}");
        assert!((s15 - 10.5).abs() < 1e-12, "lin-lin midpoint s15={s15}");
    }

    /// **Multiplicative (`lssf>0`) applies a self-shielding factor `s/sinf`.**
    ///
    /// **Methodology.** With the multiplicative convention and an incoming
    /// background of 50 barn at a stored point (e=20, dilution=10 barn),
    /// `sig <- 50 * s/sinf = 50 * 14/20 = 35 barn`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** 35.0 barn (deviation < 1e-12).
    #[test]
    fn getunr_multiplicative_factor() {
        let t = sample_table(LssfFlag::Multiplicative);
        let out = t
            .shield(UrrReaction::Total, 20.0, &[50.0], &[10.0])
            .expect("shield");
        assert!((out.sig[0] - 35.0).abs() < 1e-12, "sig={:?}", out.sig);
    }

    /// Out-of-range energy, an absent reaction column, and a trivial dilution
    /// grid all leave the background untouched with `next_energy = None`
    /// (`getunr` early returns).
    #[test]
    fn getunr_out_of_range_and_absent() {
        let t = sample_table(LssfFlag::Additive);
        // Below and above the URR range.
        let below = t.shield(UrrReaction::Total, 5.0, &[1.0], &[10.0]).unwrap();
        assert_eq!(below.next_energy, None);
        assert_eq!(below.sig, vec![1.0]);
        let above = t.shield(UrrReaction::Total, 40.0, &[1.0], &[10.0]).unwrap();
        assert_eq!(above.next_energy, None);
        // Fission column not stored (only reaction 0 present).
        let absent = t.shield(UrrReaction::Fission, 20.0, &[1.0], &[10.0]).unwrap();
        assert_eq!(absent.next_energy, None);
        assert_eq!(absent.sig, vec![1.0]);
    }

    /// The PENDF URR-tape reader is an explicit `NotPorted` boundary.
    #[test]
    fn urr_tape_reader_not_ported() {
        assert!(matches!(
            read_urr_from_pendf(),
            Err(NjoyError::NotPorted(
                "groupr::stounr (PENDF URR-tape reading)"
            ))
        ));
    }
}
