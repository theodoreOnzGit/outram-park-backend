//! `PURR` — unresolved-resonance probability tables for Monte Carlo.
//!
//! Produces probability tables for the unresolved resonance range (URR),
//! suitable for continuous-energy Monte Carlo self-shielding (MCNP/OpenMC),
//! where the Bondarenko method used by UNRESR/GROUPR is not applicable. Builds
//! the tables by sampling explicit **resonance ladders** from the ENDF average
//! parameters and binning the resulting cross sections into probability bins per
//! energy.
//!
//! Ported from `purr.f90` (2919 lines):
//!
//! - **ENDF File 2/3 reading** — PURR's `rdf2un`/`rdf3un`/`unfac2`/`intrf2`/
//!   `intr2` are structurally identical to UNRESR's `rdunf2`/`rdunf3`/
//!   `uunfac`/`intrf`/`intr` (same ENDF LRU=2 Case A/B/C layout, same
//!   formulas) — this module reuses [`crate::unresr::mf2`] and
//!   [`crate::unresr::penetrability_factor`] directly rather than duplicating
//!   them.
//! - [`wfun`] — `uw2`, PURR's own complex-probability-integral evaluator
//!   (algorithmically identical to [`crate::unresr::wfun::uw`] but for one
//!   small genuine difference — see its doc comment).
//! - [`Rng`] — `rann`, NJOY's shuffled-LCG pseudo-random generator.
//! - [`generate_ladder`] — `ladr2`, sampling one resonance ladder (Wigner
//!   spacing, χ² / Porter-Thomas widths) for one sequence.
//! - [`infinite_dilution_reference`] (+ private `gnrx`) — `unresx`, the
//!   analytic infinite-dilution cross-section reference used as a
//!   convergence check on the Monte Carlo tables.
//! - [`read_heating_cross_sections`] — `rdheat`, reading HEATR's partial
//!   heating cross sections (MT=301/302/318/402) from the PENDF tape.
//!
//! **Deferred: `unrest`** (`purr.f90:1789-2543`, ~750 lines) — the actual
//! Monte Carlo probability-table binning core. It combines Monte Carlo energy
//! sampling, per-resonance Doppler line-shape evaluation across **six**
//! precision regimes (asymptotic → two rational-approximation tiers → two
//! table-lookup variants depending on Doppler width → back down, needing its
//! own two-table `w(z)` lookup scheme, `uwtab2`, not ported), a dynamic
//! non-uniform histogram bin-edge scheme, and simultaneous probability-table +
//! Bondarenko-moment accumulation. This is a substantially larger and more
//! numerically delicate routine than anything else in this crate's URR work —
//! comparable in kind (regime-boundary Doppler-shape branching) to BROADR's
//! still-open wing-fidelity issue, but larger in scope. Deferred as an
//! explicit TODO (see `README.md`) rather than risking a rushed, hard-to-verify
//! port of the single highest-risk routine in the NJOY porting effort so far —
//! matching the precedent set by deferring HEATR's H6.
//!
//! **Status:** the PENDF MT=152 (Bondarenko table) / MT=153 (probability
//! table) output-tape bookkeeping is not ported either, for the same reason as
//! every other module's driver split: this crate has no established PENDF
//! output-section-writer concept yet, and it is pure tape plumbing, not
//! physics. `run()` remains [`crate::NjoyError::NotPorted`].

pub mod wfun;

use crate::unresr::mf2::UnresolvedRange;
use crate::unresr::wfun::{qp_node, qw_weight};
use crate::unresr::{channel_radius_urr, penetrability_factor, range_sequences};
use crate::NjoyError;

/// NJOY's pseudo-random number generator — a shuffled linear-congruential
/// generator (Numerical-Recipes-`ran1`-style), ported from `rann`
/// (`purr.f90:2877-2917`). Used by [`generate_ladder`] to sample resonance
/// spacings and widths.
pub struct Rng {
    idum: i32,
    seeded: bool,
    iy: i32,
    ir: [i32; 97],
}

impl Rng {
    const M: i32 = 714_025;
    const IA: i32 = 1366;
    const IC: i32 = 150_889;

    /// Construct with the given seed. Any seed works, but a **negative**
    /// value matches `rann`'s own documented convention ("set idum negative
    /// to reset the seed") and is what `purr.f90`'s driver uses (`kk=-101`).
    pub fn new(seed: i32) -> Self {
        Rng { idum: seed, seeded: false, iy: 0, ir: [0; 97] }
    }

    /// Draw the next uniform value in `(0, 1)` (never exactly `0.0` —
    /// `rann`'s own `if (rann.eq.zero) go to 100` retry). Ported from `rann`
    /// (`purr.f90:2877-2917`); `rem_euclid` stands in for Fortran's `mod`,
    /// which coincide for every operand sign this recurrence actually
    /// produces (see the doc note on [`Self`]).
    pub fn next(&mut self) -> f64 {
        loop {
            if self.idum < 0 || !self.seeded {
                self.seeded = true;
                self.idum = (Self::IC - self.idum).rem_euclid(Self::M);
                for j in 0..97 {
                    self.idum = (Self::IA * self.idum + Self::IC).rem_euclid(Self::M);
                    self.ir[j] = self.idum;
                }
                self.idum = (Self::IA * self.idum + Self::IC).rem_euclid(Self::M);
                self.iy = self.idum;
            }
            let j = (1 + (97 * self.iy) / Self::M).clamp(1, 97) as usize;
            let idx = j - 1;
            self.iy = self.ir[idx];
            let r = self.iy as f64 / Self::M as f64;
            self.idum = (Self::IA * self.idum + Self::IC).rem_euclid(Self::M);
            self.ir[idx] = self.idum;
            if r != 0.0 {
                return r;
            }
        }
    }
}

/// 20-quantile-bin × 4-degrees-of-freedom inverse-CDF table for sampling a
/// χ² (Porter-Thomas) distributed width multiplier — ported from `chisq`
/// (`purr.f90:1701-1721`). `CHISQ[bin][dof-1]` is the multiplier for
/// quantile bin `bin` (0-indexed, 20 equiprobable bins) and `dof` degrees of
/// freedom (1..=4).
const CHISQ: [[f64; 4]; 20] = [
    [1.31003e-3, 0.0508548, 0.206832, 0.459462],
    [9.19501e-3, 0.156167, 0.470719, 0.893735],
    [0.0250905, 0.267335, 0.691933, 1.21753],
    [0.049254, 0.38505, 0.901674, 1.50872],
    [0.0820892, 0.510131, 1.10868, 1.78605],
    [0.124169, 0.643564, 1.31765, 2.05854],
    [0.176268, 0.786543, 1.53193, 2.33194],
    [0.239417, 0.940541, 1.75444, 2.61069],
    [0.314977, 1.1074, 1.98812, 2.89878],
    [0.404749, 1.28947, 2.23621, 3.20032],
    [0.511145, 1.48981, 2.50257, 3.51995],
    [0.637461, 1.71249, 2.79213, 3.86331],
    [0.788315, 1.96314, 3.11143, 4.23776],
    [0.970419, 2.24984, 3.46967, 4.65345],
    [1.194, 2.58473, 3.88053, 5.12533],
    [1.47573, 2.98744, 4.36586, 5.67712],
    [1.84547, 3.49278, 4.96417, 6.35044],
    [2.36522, 4.17238, 5.75423, 7.22996],
    [3.20371, 5.21888, 6.94646, 8.541],
    [5.58201, 7.99146, 10.0048, 11.8359],
];

/// Sample a quantile-bin index (0-indexed, `0..20`) from `1 + scale·U` with
/// `U` uniform on `(0,1)` — the common shape of `ladr2`'s three width-sampling
/// draws (`n=int(1+start*rann(kk))`, `purr.f90:1756`/`1764`/`1774`), each
/// with its own `scale` (see [`generate_ladder`]'s two named constants).
fn chisq_quantile_index(rng: &mut Rng, scale: f64) -> usize {
    let n = (1.0 + scale * rng.next()) as i32;
    (n - 1).clamp(0, 19) as usize
}

/// One resonance sequence's staged average parameters, ready for repeated
/// ladder sampling at a fixed energy — the useful subset of what `unresx`
/// stores into its per-sequence module globals (`purr.f90:17-30`) that
/// [`generate_ladder`] needs. (The additional fields `unresx` stages —
/// `csz`/`cth`/`cc2p`/`cs2p` — are only consumed by the not-yet-ported
/// `unrest`, so they are not represented here.)
#[derive(Debug, Clone, Copy)]
pub struct SequenceLadderParams {
    /// Average level spacing `D` \[eV\].
    pub dbar: f64,
    /// Mean energy-scaled neutron width `Γn(E) = GNO·√E·V_l(E)` \[eV\].
    pub gn_mean: f64,
    /// Mean fission width \[eV\] (`0.0` if this sequence has no fission
    /// channel).
    pub gf_mean: f64,
    /// Mean radiative capture width \[eV\] (never fluctuated — no degrees of
    /// freedom).
    pub gg_mean: f64,
    /// Mean competitive-reaction width \[eV\] (`0.0` if absent).
    pub gx_mean: f64,
    /// Degrees of freedom for the neutron-width distribution.
    pub ndf_n: i32,
    /// Degrees of freedom for the fission-width distribution.
    pub ndf_f: i32,
    /// Degrees of freedom for the competitive-width distribution.
    pub ndf_x: i32,
}

/// One sampled resonance within a [`generate_ladder`] ladder.
#[derive(Debug, Clone, Copy)]
pub struct LadderResonance {
    /// Resonance energy \[eV\].
    pub energy: f64,
    /// Fractional neutron width (`Γn/Γ_total`, dimensionless).
    pub gn_frac: f64,
    /// Fractional fission width.
    pub gf_frac: f64,
    /// Fractional capture width.
    pub gg_frac: f64,
    /// Fractional competitive width.
    pub gx_frac: f64,
    /// Sampled total width `Γ_total` \[eV\].
    pub total_width: f64,
}

/// Generate one resonance ladder spanning `[elow, ehigh]` for one sequence —
/// ported from `ladr2` (`purr.f90:1687-1787`).
///
/// Resonance spacing is drawn from a **Wigner** distribution
/// (`E_r ← E_{r-1} + D·√(4/π)·√(−ln U)`, the standard Wigner-surmise sampler);
/// the first resonance's position is uniform in `[elow, elow + D√(4/π))`.
/// Each width is drawn as `(mean/dof)·χ²_dof`, with `χ²_dof` looked up from
/// [`CHISQ`] at a uniformly-sampled quantile bin. Two distinct (numerically
/// near-identical, but genuinely different in the source) quantile-bin scales
/// are used: `19.9999` for neutron and fission widths, `19.998` for the
/// competitive width (`purr.f90:1756`/`1764`/`1772-1774`) — ported as the same
/// two constants, not unified, since the difference is real (if immaterial)
/// upstream.
///
/// Returns every resonance up to and including the first one whose energy
/// exceeds `ehigh`.
pub fn generate_ladder(seq: &SequenceLadderParams, elow: f64, ehigh: f64, rng: &mut Rng) -> Vec<LadderResonance> {
    const NEUTRON_FISSION_SCALE: f64 = 19.9999;
    // purr.f90:1772-1773 — `dn=20; dn=dn-dn/10000` = 19.998.
    const COMPETITIVE_SCALE: f64 = 20.0 - 20.0 / 10000.0;

    let dcon = seq.dbar * (4.0 / std::f64::consts::PI).sqrt();
    let mut resonances: Vec<LadderResonance> = Vec::new();

    loop {
        let energy = match resonances.last() {
            Some(prev) => prev.energy + dcon * (-rng.next().ln()).sqrt(),
            None => elow + dcon * rng.next(),
        };

        let mut gn = seq.gn_mean;
        if seq.ndf_n > 0 {
            gn /= seq.ndf_n as f64;
        }
        let idx = chisq_quantile_index(rng, NEUTRON_FISSION_SCALE);
        gn *= CHISQ[idx][(seq.ndf_n - 1).clamp(0, 3) as usize];

        let mut gf = seq.gf_mean;
        if seq.gf_mean != 0.0 && seq.ndf_f != 0 {
            if seq.ndf_f > 0 {
                gf /= seq.ndf_f as f64;
            }
            let idx = chisq_quantile_index(rng, NEUTRON_FISSION_SCALE);
            gf *= CHISQ[idx][(seq.ndf_f - 1).clamp(0, 3) as usize];
        }

        let mut gx = seq.gx_mean;
        if seq.gx_mean != 0.0 && seq.ndf_x != 0 {
            if seq.ndf_x > 0 {
                gx /= seq.ndf_x as f64;
            }
            let idx = chisq_quantile_index(rng, COMPETITIVE_SCALE);
            gx *= CHISQ[idx][(seq.ndf_x - 1).clamp(0, 3) as usize];
        }

        let gg = seq.gg_mean;
        let gt = gn + gf + gg + gx;
        let done = energy > ehigh;
        resonances.push(LadderResonance {
            energy,
            gn_frac: gn / gt,
            gf_frac: gf / gt,
            gg_frac: gg / gt,
            gx_frac: gx / gt,
            total_width: gt,
        });
        if done {
            return resonances;
        }
    }
}

/// Infinite-dilution fluctuation integral for one width-channel combination —
/// ported from `gnrx` (`purr.f90:1573-1685`). Used only by
/// [`infinite_dilution_reference`]; unlike [`crate::unresr::wfun::ajk`], this
/// needs no complex probability integral — at infinite dilution there is no
/// resonance-resonance overlap correction, so the average is a direct
/// multi-fold quadrature over the raw widths.
///
/// - `galpha` / `mu` — the width (and its degrees of freedom) that appears as
///   `x_j²` (`id=1`) or `x_j` (`id=2`,`id=3`) in the numerator.
/// - `gbeta` / `nu` — a second fluctuating width (fission, when present;
///   `gbeta<=0.0` means absent).
/// - `gamma` — a non-fluctuating width (capture; always additive, never
///   sampled).
/// - `df` / `lamda` — a third fluctuating width (competitive), or `<=0.0` if
///   absent.
/// - `id` — `1`: elastic-type; `2`: capture-type; `3`: fission-type (needs
///   `gbeta>0.0`).
fn gnrx(galpha: f64, gbeta: f64, gamma: f64, mu: i32, nu: i32, lamda: i32, df: f64, id: i32) -> f64 {
    if galpha <= 0.0 || gamma <= 0.0 || gbeta < 0.0 {
        return 0.0;
    }
    if gbeta <= 0.0 && df < 0.0 {
        return 0.0;
    }

    let mut s = 0.0;
    if gbeta <= 0.0 {
        if df <= 0.0 {
            for j in 0..10 {
                let (xj, wj) = (qp_node(j, mu), qw_weight(j, mu));
                match id {
                    1 => s += wj * xj * xj / (galpha * xj + gamma),
                    2 => s += wj * xj / (galpha * xj + gamma),
                    _ => {}
                }
            }
        } else {
            for j in 0..10 {
                let (xj, wj) = (qp_node(j, mu), qw_weight(j, mu));
                for k in 0..10 {
                    let (xk, wk) = (qp_node(k, lamda), qw_weight(k, lamda));
                    match id {
                        1 => s += wj * wk * xj * xj / (galpha * xj + gamma + df * xk),
                        2 => s += wj * wk * xj / (galpha * xj + gamma + df * xk),
                        _ => {}
                    }
                }
            }
        }
    } else if df <= 0.0 {
        // purr.f90:1642-1643's redundant `if (df.le.zero) then if
        // (df.ge.zero)` — only fires for df exactly 0.0; a strictly negative
        // df here leaves `s` at 0.0, matching upstream's fall-through.
        if df >= 0.0 {
            for j in 0..10 {
                let (xj, wj) = (qp_node(j, mu), qw_weight(j, mu));
                for k in 0..10 {
                    let (xk, wk) = (qp_node(k, nu), qw_weight(k, nu));
                    match id {
                        1 => s += wj * wk * xj * xj / (galpha * xj + gbeta * xk + gamma),
                        2 => s += wj * wk * xj / (galpha * xj + gbeta * xk + gamma),
                        3 => s += wj * wk * xj * xk / (galpha * xj + gbeta * xk + gamma),
                        _ => {}
                    }
                }
            }
        }
    } else {
        for j in 0..10 {
            let (xj, wj) = (qp_node(j, mu), qw_weight(j, mu));
            for k in 0..10 {
                let (xk, wk) = (qp_node(k, nu), qw_weight(k, nu));
                for l in 0..10 {
                    let (xl, wl) = (qp_node(l, lamda), qw_weight(l, lamda));
                    match id {
                        1 => s += wj * wk * wl * xj * xj / (galpha * xj + gbeta * xk + gamma + df * xl),
                        2 => s += wj * wk * wl * xj / (galpha * xj + gbeta * xk + gamma + df * xl),
                        3 => s += wj * wk * wl * xj * xk / (galpha * xj + gbeta * xk + gamma + df * xl),
                        _ => {}
                    }
                }
            }
        }
    }
    s
}

/// The analytic infinite-dilution reference calculation, plus every
/// contributing sequence's staged ladder-sampling parameters — ported from
/// `unresx` (`purr.f90:1291-1485`).
#[derive(Debug, Clone)]
pub struct InfiniteDilutionResult {
    /// Potential-scattering cross section \[b\] (`spot`).
    pub potential_scattering: f64,
    /// Σ 1/D over every contributing sequence — the mean inverse resonance
    /// spacing (`dbarin`), used by the caller to size the Monte Carlo energy
    /// span.
    pub mean_inverse_spacing: f64,
    /// Infinite-dilution elastic cross section \[b\] (resonance part only —
    /// excludes potential scattering and File-3 background; `sigi(2)` before
    /// the driver adds `spot + bkg(2)`).
    pub sigma_elastic_inf: f64,
    /// Infinite-dilution fission cross section \[b\] (resonance part only).
    pub sigma_fission_inf: f64,
    /// Infinite-dilution capture cross section \[b\] (resonance part only).
    pub sigma_capture_inf: f64,
    /// Every contributing sequence's staged parameters, ready for
    /// [`generate_ladder`].
    pub sequences: Vec<SequenceLadderParams>,
}

/// Compute the infinite-dilution reference cross sections and stage
/// per-sequence ladder-sampling parameters at energy `e` — ported from
/// `unresx` (`purr.f90:1291-1485`).
///
/// `ranges` is every parsed [`UnresolvedRange`] for the material (from
/// [`crate::unresr::mf2::parse_lru2_ranges`]); only ranges whose `[el, eh]`
/// brackets `e` contribute. `e` must be positive (see
/// [`crate::unresr::unresolved_cross_sections`]'s identical convention).
pub fn infinite_dilution_reference(ranges: &[UnresolvedRange], e: f64) -> Result<InfiniteDilutionResult, NjoyError> {
    let e2 = e.sqrt();

    let mut spot = 0.0;
    let mut dbarin = 0.0;
    let (mut sigi_elastic, mut sigi_fission, mut sigi_capture) = (0.0, 0.0, 0.0);
    let mut sequences = Vec::new();

    let mut last_l: Option<i32> = None;
    for range in ranges {
        if e < range.el || e > range.eh {
            continue;
        }
        for seq in range_sequences(range, e) {
            let aa = channel_radius_urr(seq.awri, seq.naps, seq.ap)?;
            let rat = seq.awri / (seq.awri + 1.0);
            let k = crate::reconr::slbw::WAVE_K * rat * e2;
            let ab = 4.0 * std::f64::consts::PI / (k * k);
            let rho = k * aa;
            let rho_c = k * seq.ap;
            let (vl, ps) = penetrability_factor(seq.l, rho, rho_c);

            let gj = (2.0 * seq.aj + 1.0) / (4.0 * seq.spi + 2.0);
            let gnx_full = seq.gno * vl * e2 * seq.amun;

            // unresx:1440-1442 — potential scattering added once per L (its
            // first J-state), matching unresl's identical convention.
            if last_l != Some(seq.l) {
                spot += seq.abn * ab * (2.0 * seq.l as f64 + 1.0) * ps.sin().powi(2);
                last_l = Some(seq.l);
            }

            dbarin += 1.0 / seq.d;

            let ndf_n = seq.amun.round() as i32;
            let ndf_f = if seq.amuf <= 0.0 { 0 } else { seq.amuf.round() as i32 };
            let ndf_x = if seq.amux <= 0.0 { 0 } else { seq.amux.round() as i32 };
            let gnx = gnx_full / ndf_n.max(1) as f64;
            let gfx = if ndf_f <= 0 { 0.0 } else { seq.gf };
            let gxx = if ndf_x <= 0 { 0.0 } else { seq.gx };

            sequences.push(SequenceLadderParams {
                dbar: seq.d,
                gn_mean: gnx,
                gf_mean: gfx,
                gg_mean: seq.gg,
                gx_mean: gxx,
                ndf_n,
                ndf_f,
                ndf_x,
            });

            let gs = gnrx(gnx, gfx, seq.gg, ndf_n, ndf_f, ndf_x, gxx, 1);
            let gc = gnrx(gnx, gfx, seq.gg, ndf_n, ndf_f, ndf_x, gxx, 2);
            let gf_avg = gnrx(gnx, gfx, seq.gg, ndf_n, ndf_f, ndf_x, gxx, 3);
            let temp = seq.abn * std::f64::consts::PI * ab * gj * gnx / (2.0 * seq.d);
            sigi_elastic += temp * (gs * gnx - 2.0 * ps.sin().powi(2));
            sigi_fission += temp * gf_avg * gfx;
            sigi_capture += temp * gc * seq.gg;
        }
    }

    Ok(InfiniteDilutionResult {
        potential_scattering: spot,
        mean_inverse_spacing: dbarin,
        sigma_elastic_inf: sigi_elastic,
        sigma_fission_inf: sigi_fission,
        sigma_capture_inf: sigi_capture,
        sequences,
    })
}

/// Read the total (MT=301) and partial (elastic MT=302, fission MT=318,
/// capture MT=402) HEATR-produced heating cross sections from the PENDF
/// tape's File 3, at the given energy grid — ported from `rdheat`
/// (`purr.f90:1237-1289`).
///
/// Reactions absent from `mt_xy` contribute `0.0` (matching upstream: `ihave`
/// only records whether MT=301 and/or the partials were *found* — it never
/// invents a value for a missing one). `mt_xy` supplies each heating
/// reaction's ENDF `TAB1` data (interpolation-law list + `(x,y)` pairs) keyed
/// by MT, from [`crate::endf::tape`].
///
/// Returns `(heat, have_total, have_partials)`, where `heat[energy_index]` is
/// `[total, elastic, fission, capture]` \[eV\] (HEATR heating is per-collision
/// average energy deposit, not a cross section, despite living in File 3).
pub fn read_heating_cross_sections(
    eunr: &[f64],
    mt_xy: &[(i32, Vec<(u32, u32)>, Vec<(f64, f64)>)],
) -> Result<(Vec<[f64; 4]>, bool, bool), NjoyError> {
    let find = |mt: i32| mt_xy.iter().find(|(m, _, _)| *m == mt);
    let mts = [301, 302, 318, 402];

    let mut heat = vec![[0.0f64; 4]; eunr.len()];
    let mut have_total = false;
    let mut have_partials = false;
    for (ix, &mt) in mts.iter().enumerate() {
        let Some((_, interp, xy)) = find(mt) else { continue };
        if ix == 0 {
            have_total = true;
        } else {
            have_partials = true;
        }
        for (ie, &e_signed) in eunr.iter().enumerate() {
            let e = e_signed.abs();
            heat[ie][ix] = crate::endf::interp::eval_tab1(e, interp, xy)?;
        }
    }
    Ok((heat, have_total, have_partials))
}

/// Run the PURR card-input driver. Placeholder — the ported pieces (ENDF
/// parsing via [`crate::unresr::mf2`], [`generate_ladder`],
/// [`infinite_dilution_reference`], [`read_heating_cross_sections`],
/// [`wfun::uw2`]) are reached directly; the Monte Carlo probability-table
/// core (`unrest`) is not ported — see the module docs.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("purr driver (unrest Monte Carlo core not yet ported — see module docs)"))
}
