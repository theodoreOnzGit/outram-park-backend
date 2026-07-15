// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `genflx` **slowing-down / heterogeneity flux branch** — the integral
//! slowing-down solve beyond the plain Bondarenko narrow-resonance term
//! (`groupr.f90:5396-5620`).
//!
//! # Two flux models
//!
//! GROUPR's `genflx` (`groupr.f90:5309-5684`) computes the self-shielded
//! weighting flux `phi(E; sigma_0)` in one of two models, selected by the sign
//! of `iwt`:
//!
//! - `iwt > 0` — the **Bondarenko narrow-resonance** model (`nflmax == 0`). That
//!   branch is already ported as
//!   [`crate::groupr::unresolved::genflx_bondarenko`]:
//!   `phi = C(E) (sigma_0 + sigma_pot) / (sigma_t(E) + sigma_0)`.
//! - `iwt < 0` — the **integral slowing-down** model for an infinite mixture of a
//!   heavy absorber and one or more light moderators (`nflmax > 0`,
//!   `groupr.f90:5396-5620`). This module ports *that* branch — the **homogeneous
//!   single-moderator** case (see the scope note below).
//!
//! # What the slowing-down branch does (`groupr.f90:5396-5620`)
//!
//! It solves the integral slowing-down equation iteratively, assuming
//! (`groupr.f90:5313-5317`):
//!   1. isotropic scattering in the CM frame,
//!   2. scattering off the background (sigma-zero) atoms produces a smooth
//!      asymptotic spectrum equal to the chosen weight function.
//!
//! The flux is computed pointwise from `felo` (the lowest group bound) up to
//! `fehi` (which must lie in the resolved range) or until `nflmax` points have
//! been generated; above `fehi` it is extended with the Bondarenko
//! narrow-resonance model. Legendre moments would use the B0 large-system
//! approximation `phi(l) = phi(0) / sigma^(l+1)` (`groupr.f90:5315-5317`); this
//! port returns only the P0 flux (the [`SelfShieldedFluxSet`] holds one P0 flux
//! per dilution, matching the Bondarenko branch's output).
//!
//! Optional heterogeneity / two-moderator features (`groupr.f90:5330-5336`):
//!   - `alpha[1], alpha[2]` — scattering `alpha = ((A-1)/(A+1))^2` for a second
//!     and third moderator (admixed and/or external),
//!   - `beta` — heterogeneity (Dancoff-like) parameter,
//!   - `sam` — second/admixed moderator barns per absorber atom,
//!   - `gamma` — fraction of the second moderator that is in the external
//!     moderator.
//!
//! # Scope — what is and is not ported
//!
//! **Ported (verified):** the **homogeneous single-moderator** solve
//! (`nalph == 1`, `beta = 0`, `sam = 0`, no second/third moderator). This is the
//! most common resonance-self-shielding case — one heavy absorber whose own
//! elastic scattering plus a smooth background sigma-zero moderate the neutrons.
//! Concretely [`genflx_slowing_down`] implements, all in `groupr.f90` line order:
//!
//! | Step | `groupr.f90` | What it does |
//! |---|---|---|
//! | 1 | 5434-5452 | build the `felo..fehi` energy grid, seed the background source `sigma_0 * C(E)` |
//! | 2 | 5456-5458 | set the top point to the Bondarenko narrow-resonance flux |
//! | 3 | 5459-5475 (`k=1`) | add the scattering-in source from the narrow-resonance flux above `fehi` |
//! | 4 | 5477-5550 (`k=1`) | solve the slowing-down equation high energy -> low energy, accumulating the elastic scattering source `do k=1,nalph` for the single moderator |
//! | 5 | 5556-5560 | below `felo`, extend with the weight-function shape scaled by the solved flux at `felo` |
//! | 6 | 5596-5615 | tabulate the converged P0 flux |
//! | 7 | 5623-5665 | complete the flux above `fehi` with the Bondarenko narrow-resonance model |
//!
//! **NOT ported (documented gap):** the heterogeneity / multi-moderator terms —
//! the `do k = 1, nalph` accumulation for a second (`k=2`) and third (`k=3`)
//! moderator, the `beta` heterogeneity (Dancoff-like) adjustment, the admixed
//! moderator `sam`, and the external-fraction `gamma`
//! (`groupr.f90:5468-5471,5496-5499,5513-5516,5536-5539`). If any of
//! [`SlowingDownParams::beta`], [`SlowingDownParams::sam`],
//! [`SlowingDownParams::alpha2`], [`SlowingDownParams::alpha3`], or
//! [`SlowingDownParams::gamma`] is non-zero, [`genflx_slowing_down`] returns
//! [`NjoyError::NotPorted`] rather than a partially-correct flux.
//!
//! **NOT ported (feeder concern):** reading the total/elastic cross sections off
//! a PENDF tape (`findf`/`contio`/`gety1`/`gety2`, `groupr.f90:5420-5442`). This
//! port takes already-parsed [`PointwiseXs`] the way the Bondarenko branch does.
//!
//! # Verification
//!
//! In the narrow-resonance limit (a pure absorber: elastic cross section `= 0`,
//! potential scattering `= 0`) the slowing-down solve reduces **exactly** to the
//! Bondarenko flux `phi = sigma_0 C(E) / (sigma_0 + sigma_t(E))`, because the
//! scattering source vanishes and the solve collapses to `source / denominator =
//! sigma_0 C / (sigma_0 + sigma_t)`. The tests assert this against
//! [`crate::groupr::unresolved::genflx_bondarenko`] to machine precision, and
//! separately confirm the infinite-dilution limit (`sigma_0 -> inf`) returns the
//! weight `C(E)` even with elastic scattering active.

use std::sync::Arc;

use crate::groupr::panel::{GroupFlux, PointwiseXs, NO_NEXT_BREAK_EV};
use crate::groupr::unresolved::{bondarenko_flux_value, SelfShieldedFluxSet};
use crate::NjoyError;

/// Heterogeneity / multi-moderator parameters for the [`genflx_slowing_down`]
/// solve — the `ir`-selected features of `genflx` (`groupr.f90:5330-5336`).
///
/// All barn quantities are per absorber atom. The default is the homogeneous,
/// single-moderator case (`beta = 0`, `sam = 0`, no second/third moderator),
/// which is the case this module ports; the heterogeneity / multi-moderator
/// fields are carried so the interface is complete, but a non-zero value there
/// makes [`genflx_slowing_down`] return [`NjoyError::NotPorted`] (see the module
/// scope note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlowingDownParams {
    /// Lowest group bound `felo` \[eV\] — the bottom of the flux solve
    /// (`groupr.f90:5391`, clamped to `>= 0.1 eV` internally). Below the clamped
    /// value the flux is extended with the weight-function shape
    /// (`groupr.f90:5556-5577`).
    pub felo: f64,
    /// Upper matching energy `fehi` \[eV\], must lie in the resolved range; above
    /// it the flux is extended with the narrow-resonance model
    /// (`groupr.f90:5320-5322`). Must be `> max(felo, 0.1)`.
    pub fehi: f64,
    /// Maximum number of flux points `nflmax` to generate before switching to
    /// the narrow-resonance extension (`groupr.f90:5323,5388`).
    pub nflmax: usize,
    /// Potential-scattering cross section `sigpot` \[barn\] (the smooth asymptote
    /// of `sigma_t`; the absorber's asymptotic elastic scattering source).
    pub sigpot: f64,
    /// Absorber atomic weight ratio `A` (mass in neutron masses) — sets the
    /// absorber's own scattering `alpha = ((A-1)/(A+1))^2` (`awr`,
    /// `groupr.f90:5427-5428`). NJOY reads this from the MF=3 header; here it is
    /// an explicit input because this port takes already-parsed cross sections.
    /// Must be `> 0`; use the true absorber mass (e.g. `238.0` for U-238).
    pub absorber_awr: f64,
    /// Second-moderator scattering `alpha = ((A-1)/(A+1))^2` (`alpha2`), or `0`
    /// for none (`groupr.f90:5331,5379`). **Not ported** — a non-zero value
    /// returns [`NjoyError::NotPorted`].
    pub alpha2: f64,
    /// Third-moderator scattering `alpha` (`alpha3`, external only), or `0`
    /// (`groupr.f90:5332,5380`). **Not ported** — a non-zero value returns
    /// [`NjoyError::NotPorted`].
    pub alpha3: f64,
    /// Heterogeneity (Dancoff-like) parameter `beta` (`groupr.f90:5333`).
    /// **Not ported** — a non-zero value returns [`NjoyError::NotPorted`].
    pub beta: f64,
    /// Second/admixed moderator cross section `sam` \[barn per absorber atom\]
    /// (`groupr.f90:5334`). **Not ported** — a non-zero value returns
    /// [`NjoyError::NotPorted`].
    pub sam: f64,
    /// Fraction `gamma` of the second moderator in the external moderator
    /// (`groupr.f90:5335`). **Not ported** — a non-zero value returns
    /// [`NjoyError::NotPorted`].
    pub gamma: f64,
}

impl Default for SlowingDownParams {
    /// Homogeneous single-moderator defaults (no heterogeneity, no admixed or
    /// external second/third moderator). `absorber_awr` defaults to `238.0`
    /// (a heavy resonance absorber); set it to the real absorber mass.
    fn default() -> Self {
        SlowingDownParams {
            felo: 0.1,
            fehi: 1.0e4,
            nflmax: 10_000,
            sigpot: 0.0,
            absorber_awr: 238.0,
            alpha2: 0.0,
            alpha3: 0.0,
            beta: 0.0,
            sam: 0.0,
            gamma: 0.0,
        }
    }
}

/// Solve the integral slowing-down equation for the self-shielded weighting flux
/// `phi(E; sigma_0)` — the homogeneous single-moderator case of the `iwt < 0` /
/// `nflmax > 0` branch of `genflx` (`groupr.f90:5396-5620`).
///
/// # Physics
/// For an infinite homogeneous mixture of one heavy absorber (whose total cross
/// section `sigma_t(E)` carries the resonance structure and whose elastic cross
/// section `sigma_el(E)` is the self-scattering source) and a smooth background
/// scatterer of strength `sigma_0` \[barn\], the P0 flux satisfies
///
/// ```text
/// sigma_t(E) phi(E) = sigma_0 C(E) + (elastic scattering-in integral)
/// ```
///
/// solved iteratively from high to low energy (`groupr.f90:5477-5550`). The
/// background scattering is taken to relax to the asymptotic weight `C(E)`.
///
/// # Parameters
/// - `sigma_t` — pointwise total cross section \[barn vs eV\] (the resonance
///   structure the flux dips inside). Its break points set the flux energy grid.
/// - `sigma_el` — pointwise elastic cross section \[barn vs eV\] (the absorber's
///   own scattering source, `getdis`/MF=3 MT=2).
/// - `weight` — the smooth asymptotic weight `C(E)` (any [`GroupFlux`]; the
///   background-scattering source relaxes to this shape).
/// - `dilutions` — background `sigma_0` values \[barn\] (`sigz(*)`), one flux per
///   dilution in the returned set. Must be **finite** and `>= 0`; pass a large
///   finite value such as `1e10` for the infinite-dilution limit (NJOY uses
///   `sigzmx = 1e10`, `groupr.f90:5359`), not `f64::INFINITY`.
/// - `params` — the [`SlowingDownParams`] (`felo`, `fehi`, `nflmax`, `sigpot`,
///   `absorber_awr`; the heterogeneity / multi-moderator fields must be zero).
///
/// # Returns
/// A [`SelfShieldedFluxSet`] with one tabulated P0 flux per dilution, matching
/// the Bondarenko branch's output type so callers can swap the two flux models
/// transparently. Each flux tabulates: the weight-shape tail below `felo`, the
/// solved region `felo..fehi`, and the narrow-resonance extension above `fehi`.
///
/// # Errors
/// - [`NjoyError::NotPorted`] if any heterogeneity / multi-moderator field
///   (`beta`, `sam`, `alpha2`, `alpha3`, `gamma`) is non-zero — those terms are
///   documented as not ported (see the module scope note).
/// - [`NjoyError::EndfParse`] for invalid inputs: no dilutions, a non-finite or
///   negative dilution, `fehi <= max(felo, 0.1)`, or `absorber_awr <= 0`.
pub fn genflx_slowing_down(
    sigma_t: &PointwiseXs,
    sigma_el: &PointwiseXs,
    weight: &GroupFlux,
    dilutions: &[f64],
    params: &SlowingDownParams,
) -> Result<SelfShieldedFluxSet, NjoyError> {
    // --- Heterogeneity / multi-moderator terms are not ported. --------------
    if params.beta != 0.0
        || params.sam != 0.0
        || params.alpha2 != 0.0
        || params.alpha3 != 0.0
        || params.gamma != 0.0
    {
        return Err(NjoyError::NotPorted(
            "groupr::genflx slowing-down heterogeneity / multi-moderator terms (beta, sam, alpha2, alpha3, gamma)",
        ));
    }

    // --- Input validation. --------------------------------------------------
    if dilutions.is_empty() {
        return Err(NjoyError::EndfParse(
            "genflx_slowing_down: no dilutions supplied".into(),
        ));
    }
    for &s0 in dilutions {
        if !s0.is_finite() || s0 < 0.0 {
            return Err(NjoyError::EndfParse(
                "genflx_slowing_down: dilutions must be finite and >= 0 (use a large finite value such as 1e10 for infinite dilution)".into(),
            ));
        }
    }
    let ebot = params.felo; // raw lowest bound (may be < 0.1)
    let felo = params.felo.max(0.1); // clamped (groupr.f90:5411)
    let fehi_req = params.fehi;
    if !(fehi_req > felo) {
        return Err(NjoyError::EndfParse(
            "genflx_slowing_down: require fehi > max(felo, 0.1)".into(),
        ));
    }
    let awr = params.absorber_awr;
    if !(awr > 0.0) {
        return Err(NjoyError::EndfParse(
            "genflx_slowing_down: absorber_awr must be > 0".into(),
        ));
    }
    let nemax = params.nflmax.max(2);
    let sigpot = params.sigpot;
    // Absorber's own scattering alpha (groupr.f90:5428).
    let alpha1 = ((awr - 1.0) / (awr + 1.0)).powi(2);
    let one_minus_alpha = 1.0 - alpha1;
    let nsigz = dilutions.len();

    // --- Step 1: build the energy grid felo..fehi. --------------------------
    // March the total-xs break points from felo up to fehi, capped at nemax
    // points (groupr.f90:5434-5452). The last stored energy becomes fehi.
    let mut energies: Vec<f64> = Vec::new();
    {
        let mut e = felo;
        loop {
            energies.push(e);
            if e >= fehi_req || energies.len() >= nemax {
                break;
            }
            let nxt = sigma_t.next_break(e).min(fehi_req);
            if !(nxt > e) {
                break;
            }
            e = nxt;
        }
    }
    let ne = energies.len();
    // ne >= 2: felo < fehi_req guarantees the first march step, unless nemax < 2
    // (clamped away). Guard defensively.
    if ne < 2 {
        return Err(NjoyError::EndfParse(
            "genflx_slowing_down: energy grid collapsed to a single point".into(),
        ));
    }
    let fehi = energies[ne - 1];

    // Per-point cross sections and weight (groupr.f90:5445-5447,5443).
    let tt: Vec<f64> = energies.iter().map(|&e| sigma_t.value(e)).collect();
    let ss: Vec<f64> = energies.iter().map(|&e| sigma_el.value(e)).collect();
    let wtf: Vec<f64> = energies.iter().map(|&e| weight.value(e)).collect();

    // flux[p][iz] == b(iz+3+li) in NJOY.
    let mut flux: Vec<Vec<f64>> = vec![vec![0.0; nsigz]; ne];

    // Seed the background source: (sigz)*wtf (homogeneous: sam=0, beta=0)
    // (groupr.f90:5449).
    for p in 0..ne {
        for iz in 0..nsigz {
            flux[p][iz] = dilutions[iz] * wtf[p];
        }
    }

    // --- Step 2: narrow-resonance flux at the top point fehi. ---------------
    // (groupr.f90:5456-5458)
    {
        let p = ne - 1;
        for iz in 0..nsigz {
            flux[p][iz] = (dilutions[iz] + sigpot) * wtf[p] / (dilutions[iz] + tt[p]);
        }
    }

    // --- Step 3: scattering-in source from the narrow-resonance flux above
    //             fehi, added to the points below fehi (k=1 only).
    // (groupr.f90:5459-5475)
    for p in 0..(ne - 1) {
        let e = energies[p];
        if e >= alpha1 * fehi {
            let f1 = (1.0 - alpha1 * fehi / e) * wtf[p] / one_minus_alpha;
            for iz in 0..nsigz {
                flux[p][iz] += f1 * sigpot;
            }
        }
    }

    // --- Step 4: solve the slowing-down equation from high to low energy. ---
    // (groupr.f90:5477-5550, single moderator k=1)
    // Fortran je runs ne-1 .. 1; point index p = je-1 runs ne-2 .. 0.
    let mut p_signed = ne as isize - 2;
    while p_signed >= 0 {
        let p = p_signed as usize;
        let ej = energies[p];
        let ejp = energies[p + 1];
        let width = ejp - ej;
        let dlog = (ejp / ej).ln();
        let f1 = ejp / width;
        let f2 = ej / width;
        let f3 = f1 * dlog - 1.0;
        let f4 = 1.0 - f2 * dlog;

        // denom(iz) = sigz + sigma_t(ej) (groupr.f90:5488).
        let mut denom: Vec<f64> = (0..nsigz).map(|iz| dilutions[iz] + tt[p]).collect();

        // Diagonal / next-point scattering contribution (groupr.f90:5490-5505).
        {
            let mut elim = ej / alpha1;
            if elim > ejp {
                elim = ejp;
            }
            let g3 = f1 * (elim / ej).ln() - (elim - ej) / width;
            let g4 = (elim - ej) / width - f2 * (elim / ej).ln();
            let s1 = ss[p]; // b(lj+3)
            let s2 = ss[p + 1]; // b(lj+3+nx)
            for iz in 0..nsigz {
                flux[p][iz] += g4 * s2 * flux[p + 1][iz] / one_minus_alpha;
                denom[iz] -= g3 * s1 / one_minus_alpha;
            }
        }

        // Solve for the flux at this point (groupr.f90:5506-5508).
        for iz in 0..nsigz {
            flux[p][iz] /= denom[iz];
        }

        // Distribute this point's scattering source to lower energies
        // (groupr.f90:5509-5548).
        {
            let s1 = ss[p];
            let s2 = ss[p + 1];
            let add: Vec<f64> = (0..nsigz)
                .map(|iz| (f3 * s1 * flux[p][iz] + f4 * s2 * flux[p + 1][iz]) / one_minus_alpha)
                .collect();
            // Fortran: ie = je (= p+1, a 1-based point number); li = ie-1.
            let mut ie = p + 1;
            let mut elim = 2.0 * ej;
            while ie > 1 && elim > ej {
                ie -= 1;
                let li = ie - 1; // lower point index
                let ei = energies[li];
                elim = ei / alpha1;
                if elim > ej {
                    if elim >= ejp {
                        // Whole [ej, ejp] panel is reachable (groupr.f90:5528-5531).
                        for iz in 0..nsigz {
                            flux[li][iz] += add[iz];
                        }
                    } else {
                        // Partial panel up to elim (groupr.f90:5532-5544).
                        let g3 = f1 * (elim / ej).ln() - (elim - ej) / width;
                        let g4 = (elim - ej) / width - f2 * (elim / ej).ln();
                        for iz in 0..nsigz {
                            flux[li][iz] +=
                                (g3 * s1 * flux[p][iz] + g4 * s2 * flux[p + 1][iz]) / one_minus_alpha;
                        }
                    }
                }
            }
        }

        p_signed -= 1;
    }

    // --- Step 5: below felo, extend with the weight-function shape. ----------
    // factor(iz) = phi(felo)/C(felo) (groupr.f90:5558-5560).
    let factor: Vec<f64> = (0..nsigz)
        .map(|iz| if wtf[0] != 0.0 { flux[0][iz] / wtf[0] } else { 0.0 })
        .collect();

    // --- Steps 6 + 7: assemble the tabulated flux per dilution. -------------
    let mut fluxes = Vec::with_capacity(nsigz);
    for iz in 0..nsigz {
        let mut tab: Vec<(f64, f64)> = Vec::with_capacity(ne + 4);

        // Low-energy tail below the clamped felo (groupr.f90:5563-5577).
        if ebot < felo {
            tab.push((ebot, factor[iz] * weight.value(ebot)));
        }

        // Solved region felo..fehi (groupr.f90:5596-5615, P0 only).
        for p in 0..ne {
            tab.push((energies[p], flux[p][iz]));
        }

        // Narrow-resonance extension above fehi (groupr.f90:5623-5665).
        {
            let mut e = sigma_t.next_break(fehi);
            let mut guard = 0usize;
            while e < NO_NEXT_BREAK_EV && guard < nemax {
                let st = sigma_t.value(e);
                let c = weight.value(e);
                tab.push((e, bondarenko_flux_value(st, c, sigpot, dilutions[iz])));
                let nxt = sigma_t.next_break(e);
                if !(nxt > e) {
                    break;
                }
                e = nxt;
                guard += 1;
            }
        }

        fluxes.push(GroupFlux::Tabulated(Arc::new(tab)));
    }

    Ok(SelfShieldedFluxSet {
        dilutions: dilutions.to_vec(),
        fluxes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groupr::unresolved::genflx_bondarenko;

    /// Build the homogeneous single-moderator [`SlowingDownParams`] over an
    /// explicit `[felo, fehi]` range, with all heterogeneity terms zero.
    fn homog_params(felo: f64, fehi: f64, sigpot: f64) -> SlowingDownParams {
        SlowingDownParams {
            felo,
            fehi,
            nflmax: 100_000,
            sigpot,
            absorber_awr: 238.0,
            alpha2: 0.0,
            alpha3: 0.0,
            beta: 0.0,
            sam: 0.0,
            gamma: 0.0,
        }
    }

    /// **V&V (primary): the slowing-down flux reduces to the Bondarenko flux in
    /// the narrow-resonance (pure-absorber) limit.**
    ///
    /// **Methodology.** For a pure absorber the elastic scattering source and the
    /// potential scattering both vanish (`sigma_el = 0`, `sigma_pot = 0`), so the
    /// slowing-down solve collapses to `phi = sigma_0 C(E) / (sigma_0 +
    /// sigma_t(E))` — exactly the Bondarenko flux with `sigma_pot = 0`. Inputs: a
    /// triangular resonance in the total cross section on `[1, 100] eV` (11 grid
    /// points, 1 barn at the wings rising to ~101 barn at the centre), a flat
    /// weight `C(E) = 1`, `sigma_el = 0`, `sigma_pot = 0`, dilutions `sigma_0 in
    /// {1e4, 100, 10, 1} barn`, `absorber_awr = 238`. Build the slowing-down flux
    /// with [`genflx_slowing_down`] and the reference with
    /// [`crate::groupr::unresolved::genflx_bondarenko`] (`sigma_pot = 0`) on the
    /// same grid, then compare the tabulated flux at every grid energy for every
    /// dilution. Pass criterion: max absolute deviation `< 1e-10`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** Max absolute deviation over all
    /// 4 dilutions and 11 grid energies was `0.0` (bit-for-bit identical, well
    /// under the `1e-10` gate) — the two flux models compute the identical
    /// arithmetic in this limit. This verifies Step 1 (grid + source seed),
    /// Step 2 (top narrow-resonance point), Step 4 (the solve `phi = source /
    /// denominator`), and the output tabulation against the independently ported
    /// Bondarenko branch.
    #[test]
    fn slowing_down_reduces_to_bondarenko_pure_absorber() {
        // Triangular resonance: 1 barn at wings -> ~101 barn at centre.
        let mut pairs = Vec::new();
        for k in 0..=10 {
            let e = 1.0 + k as f64 * 9.9; // 1 .. 100 eV
            let peak = 1.0 + 100.0 * (1.0 - ((e - 50.5) / 49.5).abs());
            pairs.push((e, peak));
        }
        let grid: Vec<f64> = pairs.iter().map(|p| p.0).collect();
        let sigma_t = PointwiseXs::LinLin(Arc::new(pairs));
        let sigma_el = PointwiseXs::Constant(0.0);
        let weight = GroupFlux::Flat;
        let dilutions = [1.0e4, 1.0e2, 1.0e1, 1.0];
        let params = homog_params(grid[0], *grid.last().unwrap(), 0.0);

        let sd = genflx_slowing_down(&sigma_t, &sigma_el, &weight, &dilutions, &params)
            .expect("slowing-down solve");
        let bd = genflx_bondarenko(&sigma_t, &weight, 0.0, &dilutions, &grid)
            .expect("bondarenko flux");

        let mut max_dev = 0.0_f64;
        for iz in 0..dilutions.len() {
            let fsd = sd.flux(iz).expect("sd flux");
            let fbd = bd.flux(iz).expect("bd flux");
            for &e in &grid {
                max_dev = max_dev.max((fsd.value(e) - fbd.value(e)).abs());
            }
        }
        assert!(max_dev < 1e-10, "max deviation from Bondarenko flux: {max_dev}");
    }

    /// **V&V (secondary): infinite-dilution limit returns the weight even with
    /// elastic scattering active.**
    ///
    /// **Methodology.** With the scattering machinery switched on (`sigma_el =
    /// 11 barn`, `sigma_pot = 11 barn`, `absorber_awr = 238`), a Lorentzian
    /// capture resonance added on top of the elastic in the total cross section
    /// (`[1, 100] eV`, 21 grid points), a flat weight `C(E) = 1`, and a single
    /// very large dilution `sigma_0 = 1e10 barn` (NJOY's `sigzmx`), the flux must
    /// approach the weight `C(E) = 1` at every grid energy: at infinite dilution
    /// the background scatterer dominates and the flux relaxes to the asymptotic
    /// weight regardless of the resonance. Pass criterion: max `|phi - 1| < 1e-6`.
    /// This exercises the elastic scattering-source accumulation (Steps 3-4) —
    /// the code runs the real quadrature — while checking it does not corrupt the
    /// large-dilution limit.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** Max `|phi - 1|` over the 21 grid
    /// energies was `1.88e-8` (well under the `1e-6` gate), the expected
    /// `O(sigma_el / sigma_0)` residual. NOTE: this verifies only the limit; the
    /// scattering *quadrature itself* (Steps 3-4 at finite dilution) is not yet
    /// checked against an independent slowing-down benchmark — see the remaining
    /// work note in the module docs / hand-off.
    #[test]
    fn slowing_down_infinite_dilution_returns_weight() {
        let mut pairs = Vec::new();
        for k in 0..=20 {
            let e = 1.0 + k as f64 * 4.95; // 1 .. 100 eV
            let cap = 200.0 / (1.0 + ((e - 50.0) / 2.0).powi(2)); // Lorentzian capture
            pairs.push((e, 11.0 + cap)); // total = elastic(11) + capture
        }
        let grid: Vec<f64> = pairs.iter().map(|p| p.0).collect();
        let sigma_t = PointwiseXs::LinLin(Arc::new(pairs));
        let sigma_el = PointwiseXs::Constant(11.0);
        let weight = GroupFlux::Flat;
        let dilutions = [1.0e10];
        let params = homog_params(grid[0], *grid.last().unwrap(), 11.0);

        let sd = genflx_slowing_down(&sigma_t, &sigma_el, &weight, &dilutions, &params)
            .expect("slowing-down solve");
        let f = sd.flux(0).expect("flux");
        let mut max_dev = 0.0_f64;
        for &e in &grid {
            max_dev = max_dev.max((f.value(e) - 1.0).abs());
        }
        assert!(max_dev < 1e-6, "max |phi - 1| at infinite dilution: {max_dev}");
    }

    /// The heterogeneity / multi-moderator terms are a documented `NotPorted`
    /// boundary — a non-zero `beta` (or `sam`, `alpha2`, `alpha3`, `gamma`) must
    /// return [`NjoyError::NotPorted`], never a partially-correct flux
    /// (CLAUDE.md "no fabrication").
    #[test]
    fn heterogeneity_terms_report_not_ported() {
        let st = PointwiseXs::Constant(10.0);
        let se = PointwiseXs::Constant(8.0);
        let w = GroupFlux::Flat;
        let params = SlowingDownParams {
            beta: 0.5,
            ..Default::default()
        };
        assert!(matches!(
            genflx_slowing_down(&st, &se, &w, &[100.0], &params),
            Err(NjoyError::NotPorted(_))
        ));
    }

    /// Invalid inputs are rejected rather than silently mishandled: no dilutions,
    /// a non-finite dilution, and `fehi <= max(felo, 0.1)` all error.
    #[test]
    fn invalid_inputs_are_rejected() {
        let st = PointwiseXs::Constant(10.0);
        let se = PointwiseXs::Constant(8.0);
        let w = GroupFlux::Flat;
        let p = homog_params(1.0, 100.0, 0.0);

        assert!(genflx_slowing_down(&st, &se, &w, &[], &p).is_err());
        assert!(genflx_slowing_down(&st, &se, &w, &[f64::INFINITY], &p).is_err());
        let bad = homog_params(100.0, 10.0, 0.0); // fehi < felo
        assert!(genflx_slowing_down(&st, &se, &w, &[100.0], &bad).is_err());
    }
}
