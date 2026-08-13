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
//! - [`line_shape`] + [`wfun::DopplerTable`] — the Doppler line-shape
//!   evaluator across four precision tiers (asymptotic → 2-term rational →
//!   3-term rational → table lookup), and [`probability_table`] — `unrest`,
//!   the Monte Carlo probability-table binning core itself.
//!
//! **`unrest`'s index-range bookkeeping is not replicated literally.** Upstream
//! finds, for each resonance, which of its sample points need which precision
//! tier via a chain of binary searches (`fsrch`) on sub-ranges of the sorted
//! energy array, each search boundary reused as the next search's starting
//! point — a performance optimisation for repeatedly narrowing a large sorted
//! array. Because the Doppler-scaled offset `xs(ie) = ctx·(es(ie)−E_r)` is a
//! monotonic function of the (already sorted) `es(ie)`, this index-chain always
//! assigns the *same* tier to the *same* point as a direct classification of
//! that point's own `(x, y)` against the four tier thresholds would (verified
//! by tracing every threshold and fall-through in the upstream source). This
//! port therefore classifies each point directly ([`line_shape`]) rather than
//! replicating the binary-search chain — identical results, far less
//! reused-index bookkeeping to get right in a language without `GOTO`.
//!
//! **Status:** the PENDF MT=152 (Bondarenko table) / MT=153 (probability
//! table) output-tape bookkeeping is not ported, for the same reason as every
//! other module's driver split: this crate has no established PENDF
//! output-section-writer concept yet, and it is pure tape plumbing, not
//! physics. `run()` remains [`crate::NjoyError::NotPorted`]. See `README.md`
//! for the full status, the tier-classification equivalence argument in more
//! detail, and caveats.

pub mod wfun;

use crate::unresr::mf2::UnresolvedRange;
use crate::unresr::wfun::{qp_node, qw_weight};
use crate::unresr::{channel_radius_urr, penetrability_factor, range_sequences};
use crate::NjoyError;
use wfun::DopplerTable;

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
        Rng {
            idum: seed,
            seeded: false,
            iy: 0,
            ir: [0; 97],
        }
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
    /// `csz = abn·(4π/k²)·g_J` \[b\] — the overall elastic/capture/fission
    /// line-shape scale factor for this sequence (`unresx`'s `csz`,
    /// `purr.f90:1455`). Needed by [`line_shape_contribution`].
    pub csz: f64,
    /// The **inverse Doppler width** `1/Δ` \[eV⁻¹\] at the reference
    /// temperature `T_ref = 300 K` (`unresx`'s `cth`, `purr.f90:1456-1458`,
    /// `Δ = 2√E·√(kT/A)`). `unrest` rescales this to the actual temperature
    /// via `ctx = cth_ref·√(T_ref/T)` (`purr.f90:1921`) rather than
    /// recomputing the square root from scratch per temperature — ported as
    /// the same rescaling, not "simplified" to a direct per-temperature
    /// computation, since NJOY's own `con1` constant (`31.83`, `purr.f90:1835`
    /// — a *different*, unrelated `con1` than `unresx`'s Doppler-width one)
    /// depends on this exact staged value being reused.
    pub cth_ref: f64,
    /// `cos(2φ_l)` at this sequence's phase shift (`unresx`'s `cc2p`,
    /// `purr.f90:1459`).
    pub cc2p: f64,
    /// `sin(2φ_l)` at this sequence's phase shift (`unresx`'s `cs2p`,
    /// `purr.f90:1460`).
    pub cs2p: f64,
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
pub fn generate_ladder(
    seq: &SequenceLadderParams,
    elow: f64,
    ehigh: f64,
    rng: &mut Rng,
) -> Vec<LadderResonance> {
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
fn gnrx(
    galpha: f64,
    gbeta: f64,
    gamma: f64,
    mu: i32,
    nu: i32,
    lamda: i32,
    df: f64,
    id: i32,
) -> f64 {
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
                        1 => {
                            s += wj * wk * wl * xj * xj
                                / (galpha * xj + gbeta * xk + gamma + df * xl)
                        }
                        2 => s += wj * wk * wl * xj / (galpha * xj + gbeta * xk + gamma + df * xl),
                        3 => {
                            s += wj * wk * wl * xj * xk
                                / (galpha * xj + gbeta * xk + gamma + df * xl)
                        }
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
pub fn infinite_dilution_reference(
    ranges: &[UnresolvedRange],
    e: f64,
) -> Result<InfiniteDilutionResult, NjoyError> {
    // purr.f90:1310 — unresx's own local `con1` (Doppler-width related,
    // `1/(4·k_B)`), a *different* constant from `unrest`'s unrelated
    // `con1=31.83` (a resonance significant-range cutoff). NJOY reuses the
    // same identifier in two subroutine-local scopes with different values;
    // ported as two separate named constants to avoid that collision.
    const CON1_DOPPLER: f64 = 2901.34;
    // purr.f90:1839 / driver's `call unresx(ez,tref)` — `unresx` (and hence
    // `cth_ref`) is always evaluated at this fixed reference temperature;
    // `unrest` rescales to the real temperature afterward (`purr.f90:1921`).
    const T_REF: f64 = 300.0;

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
            let ndf_f = if seq.amuf <= 0.0 {
                0
            } else {
                seq.amuf.round() as i32
            };
            let ndf_x = if seq.amux <= 0.0 {
                0
            } else {
                seq.amux.round() as i32
            };
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
                csz: seq.abn * ab * gj,
                cth_ref: (CON1_DOPPLER * seq.awri / (e * T_REF)).sqrt(),
                cc2p: (2.0 * ps).cos(),
                cs2p: (2.0 * ps).sin(),
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
        let Some((_, interp, xy)) = find(mt) else {
            continue;
        };
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

/// Evaluate the Doppler line shape `(Re w, Im w)` at Doppler-scaled
/// coordinates `(x, y)` (`y > 0`, `yy = y·y` precomputed by the caller since
/// it is constant across every point sharing one resonance) — ported from
/// `unrest`'s combined four-tier regime selection (`purr.f90:1950-2204`). See
/// the module docs for why this is a direct per-point classification rather
/// than upstream's binary-search index-range chains.
///
/// Tier boundaries (checked in order, each an `OR` across `|x|` and `y` so a
/// resonance whose `y` alone exceeds a tier's threshold routes *every* point
/// past that tier, matching upstream's resonance-level shortcuts):
/// 1. `|x| > 100` or `y > 100` — leading asymptotic term of `erfc`.
/// 2. `|x| > 6` or `y > 6` — 2-term rational (Humlicek-style) approximation.
/// 3. `|x| > 3.9` or `y > 3.0` — 3-term rational approximation (note the
///    genuinely asymmetric thresholds, `3.9` for `x` vs `3.0` for `y`, ported
///    as-is).
/// 4. Otherwise — table lookup via [`DopplerTable`], coarse grid for
///    `y ≥ 0.5`, fine grid for `y < 0.5`.
fn line_shape(x: f64, y: f64, yy: f64, table: &DopplerTable) -> (f64, f64) {
    const C1: f64 = 0.5641895835;
    const C2: f64 = 0.2752551;
    const C3: f64 = 2.724745;
    const C4: f64 = 0.5124242;
    const C5: f64 = 0.05176536;
    const D1: f64 = 0.4613135;
    const D2: f64 = 0.1901635;
    const D3: f64 = 0.09999216;
    const D4: f64 = 1.7844927;
    const D5: f64 = 0.002883894;
    const D6: f64 = 5.5253437;

    let ax = x.abs();

    // purr.f90:2192-2203 (label 340) — leading asymptotic term.
    if ax > 100.0 || y > 100.0 {
        let test = x * x + yy;
        let a1 = C1 / test;
        return (y * a1, x * a1);
    }

    // purr.f90:2163-2188 (label 330) — 2-term rational approximation.
    if ax > 6.0 || y > 6.0 {
        let a1 = x * x - yy;
        let a2 = 2.0 * x * y;
        let a3 = a2 * a2;
        let temp1 = a2 * x;
        let temp2 = a2 * y;
        let a4 = a1 - C2;
        let a5 = a1 - C3;
        let f1 = C4 / (a4 * a4 + a3);
        let f2 = C5 / (a5 * a5 + a3);
        return (
            f1 * (temp1 - a4 * y) + f2 * (temp1 - a5 * y),
            f1 * (a4 * x + temp2) + f2 * (a5 * x + temp2),
        );
    }

    // purr.f90:2132-2159 (label 320) — 3-term rational approximation.
    if ax > 3.9 || y > 3.0 {
        let a1 = x * x - yy;
        let a2 = 2.0 * x * y;
        let a3 = a2 * a2;
        let temp1 = a2 * x;
        let temp2 = a2 * y;
        let a4 = a1 - D2;
        let a5 = a1 - D4;
        let a6 = a1 - D6;
        let e1 = D1 / (a4 * a4 + a3);
        let e2 = D3 / (a5 * a5 + a3);
        let e3 = D5 / (a6 * a6 + a3);
        return (
            e1 * (temp1 - a4 * y) + e2 * (temp1 - a5 * y) + e3 * (temp1 - a6 * y),
            e1 * (a4 * x + temp2) + e2 * (a5 * x + temp2) + e3 * (a6 * x + temp2),
        );
    }

    // purr.f90:2040-2128 (labels 260/310) — table lookup, tier chosen by y.
    if y >= 0.5 {
        table.lookup_coarse(x, y)
    } else {
        table.lookup_fine(x, y)
    }
}

/// Per-temperature probability-table output: bin edges, per-bin
/// probability/average cross sections, and Bondarenko moments — ported from
/// `unrest`'s `tval`/`tabl`/`bval` outputs (`purr.f90:1789-2543`).
#[derive(Debug, Clone)]
pub struct ProbabilityTable {
    /// Upper energy edge of each bin \[b\] (`tval`, length `nbin`; the last
    /// entry is always `1e6` b, a catch-all "infinity" edge —
    /// `purr.f90:2320`/`2426`).
    pub bin_upper_edge: Vec<f64>,
    /// Probability of landing in each bin (`tabl[.., 1]`, sums to `1.0`).
    pub bin_probability: Vec<f64>,
    /// Bin-average `[total, elastic, fission, capture]` cross sections \[b\]
    /// (`tabl[.., 2..5]`), renormalized so their probability-weighted mean
    /// matches [`InfiniteDilutionResult`]'s reference values
    /// (`purr.f90:2509-2519`).
    pub bin_xs: Vec<[f64; 4]>,
    /// Bondarenko `[total, elastic, fission, capture, current-weighted total]`
    /// cross sections \[b\] per dilution value in `sig0`, computed from the
    /// (normalized) probability table and renormalized to the
    /// infinite-dilution reference (`purr.f90:2468-2530`) — the value
    /// `unrest`'s own `sigf` output argument actually holds when the
    /// subroutine returns.
    pub bondarenko: Vec<[f64; 5]>,
    /// The same 5 Bondarenko quantities computed **directly** from the Monte
    /// Carlo samples rather than from the binned table (`purr.f90:2395-2411`)
    /// — an intermediate value upstream only ever prints (`iprint>0`) before
    /// overwriting it with [`Self::bondarenko`]. Exposed here (rather than
    /// discarded, as it would be if this port only returned the final value)
    /// because it is a second, independent reference number the same
    /// Fortran run would have printed — useful for the verification pass to
    /// check against directly, without needing `iprint=1` output from a
    /// separate oracle run. **Not renormalized** to the infinite-dilution
    /// reference, unlike [`Self::bondarenko`].
    pub bondarenko_direct_sampling: Vec<[f64; 5]>,
}

/// Running-average convergence diagnostics across ladders — ported from
/// `unrest`'s `tav`/`tvar`/... block (`purr.f90:2218-2387`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ConvergenceStats {
    /// Mean total cross section \[b\] over all ladders (temperature index 0
    /// / first requested temperature only, matching upstream).
    pub mean_total: f64,
    /// Percent standard deviation of the total cross section across ladders.
    pub pct_std_total: f64,
    pub mean_elastic: f64,
    pub pct_std_elastic: f64,
    pub mean_fission: f64,
    pub pct_std_fission: f64,
    pub mean_capture: f64,
    pub pct_std_capture: f64,
}

/// The full result of one [`probability_table`] call: one [`ProbabilityTable`]
/// per requested temperature, plus convergence diagnostics.
#[derive(Debug, Clone)]
pub struct ProbabilityTableResult {
    pub tables: Vec<ProbabilityTable>,
    pub convergence: ConvergenceStats,
}

/// Compute probability tables and Bondarenko moments at one energy by Monte
/// Carlo ladder sampling — ported from `unrest` (`purr.f90:1789-2543`).
///
/// - `sequences` / `inf_dilution` — from [`infinite_dilution_reference`] at
///   the same energy.
/// - `bkg` — File-3 background `[total, elastic, fission, capture]` \[b\]
///   (matching [`crate::unresr::unresolved_cross_sections`]'s convention).
/// - `sig0` — dilution values to tabulate Bondarenko moments for.
/// - `temp` — temperatures \[K\] to tabulate (a separate [`ProbabilityTable`]
///   per entry, but energy sampling and ladders are shared across all of
///   them within one call — matching upstream's "all temperatures computed
///   simultaneously to preserve temperature correlations").
/// - `nbin` — number of probability-table bins (upstream requires `≥ 15`).
/// - `nladr` — number of Monte Carlo ladders to sample.
/// - `nsamp` — number of random energy points sampled per ladder
///   (upstream's default is `10000`).
/// - `rng` / `table` — shared, reusable state (build once, pass to every
///   energy-point call): [`Rng`] for reproducibility across calls with the
///   same seed, [`DopplerTable`] because building it is comparatively
///   expensive and it does not depend on energy.
///
/// # Errors
///
/// Returns [`NjoyError::EndfParse`] if `nbin < 15`, or if the derived energy
/// span for ladder generation is invalid (`purr.f90:1878-1880`'s
/// `nres≤0 or emax<emin`, typically meaning the minimum sequence spacing
/// `dbar` is too large for `nermax=1000` resonances to span a usable range —
/// upstream's fix is "increase dmin", i.e. this indicates unusual input data,
/// not a bug).
#[allow(clippy::too_many_arguments)]
pub fn probability_table(
    sequences: &[SequenceLadderParams],
    inf_dilution: &InfiniteDilutionResult,
    bkg: [f64; 4],
    sig0: &[f64],
    temp: &[f64],
    nbin: usize,
    nladr: usize,
    nsamp: usize,
    rng: &mut Rng,
    table: &DopplerTable,
) -> Result<ProbabilityTableResult, NjoyError> {
    // purr.f90:1835-1836 — unrest's OWN local `con1`/`con2` (a resonance
    // significant-range cutoff), unrelated to unresx's Doppler-width `con1`
    // (see the doc note in `infinite_dilution_reference`).
    const CON1_RANGE: f64 = 31.83;
    const CON2_RANGE: f64 = 20.0;
    const T_REF: f64 = 300.0;
    const ELOW: f64 = 10.0;
    const NAVOID: f64 = 300.0;
    const NERMAX: f64 = 1000.0;
    const BIG: f64 = 1.0e6;

    if nbin < 15 {
        return Err(NjoyError::EndfParse(
            "purr: nbin should be 15 or more".to_string(),
        ));
    }

    let ntemp = temp.len();
    let nsig0 = sig0.len();
    let ns = sequences.len();
    let rpi = std::f64::consts::PI.sqrt();

    // purr.f90:1867-1880 — ladder energy span, sized off the smallest
    // sequence spacing so `nermax` resonances comfortably span it.
    let dmin = sequences
        .iter()
        .map(|s| s.dbar)
        .fold(f64::INFINITY, f64::min);
    let erange = 9.0 * NERMAX * dmin / 10.0;
    let dbarin = inf_dilution.mean_inverse_spacing;
    let nres = erange * dbarin;
    let ehigh = ELOW + erange;
    let emin = ELOW + NAVOID / dbarin;
    let emax = ehigh - NAVOID / dbarin;
    let espan = emax - emin;
    if nres <= 0.0 || emax < emin {
        return Err(NjoyError::EndfParse(
            "purr: bad value for nres or emin>emax, increase dmin".to_string(),
        ));
    }

    let spot = inf_dilution.potential_scattering;

    // Per-temperature Monte Carlo accumulators.
    let mut cap = vec![vec![0.0f64; nsamp]; ntemp];
    let mut fis = vec![vec![0.0f64; nsamp]; ntemp];
    let mut els = vec![vec![0.0f64; nsamp]; ntemp];
    let mut es = vec![0.0f64; nsamp];

    let mut bval = vec![vec![[0.0f64; 7]; nsig0]; ntemp];
    let mut tval: Vec<Vec<f64>> = vec![vec![0.0; nbin]; ntemp];
    let mut tabl: Vec<Vec<[f64; 5]>> = vec![vec![[0.0; 5]; nbin]; ntemp];
    let mut tmin = vec![f64::INFINITY; ntemp];
    let mut tmax = vec![f64::NEG_INFINITY; ntemp];
    let mut tsum = vec![0.0f64; ntemp];

    let (mut tav, mut tvar) = (0.0, 0.0);
    let (mut eav, mut evar) = (0.0, 0.0);
    let (mut fav, mut fvar) = (0.0, 0.0);
    let (mut cav, mut cvar) = (0.0, 0.0);

    // --- Main ladder loop ----------------------------------------------
    for iladr in 0..nladr {
        // purr.f90:1904-1913 — random energy grid, sorted.
        for ie in 0..nsamp {
            es[ie] = emin + espan * rng.next();
            for itemp in 0..ntemp {
                cap[itemp][ie] = 0.0;
                fis[itemp][ie] = 0.0;
                els[itemp][ie] = spot;
            }
        }
        es.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // purr.f90:1916-2209 — loop over sequences, temperatures, resonances.
        for seq in sequences {
            let ladder = generate_ladder(seq, ELOW, ehigh, rng);
            for (itemp, &t) in temp.iter().enumerate() {
                let ctx = seq.cth_ref * (T_REF / t).sqrt();
                for res in &ladder {
                    // purr.f90:1925-1934 — significant energy range + index
                    // window (direct binary search on the sorted `es`,
                    // rather than upstream's reused-boundary search chain —
                    // this one boundary computation is not part of the
                    // per-point tier classification the module docs discuss,
                    // it is just "which points are close enough to this
                    // resonance to matter at all").
                    let chek1 = CON1_RANGE * res.total_width;
                    let chek2 = CON2_RANGE / ctx;
                    let chekn = chek1.max(chek2);
                    let delr = chek1 + chekn;
                    let elo = res.energy - delr;
                    let ehi_res = res.energy + delr;
                    let i0 = es.partition_point(|&e| e < elo);
                    let i7 = es.partition_point(|&e| e < ehi_res);
                    if i0 >= i7 {
                        continue;
                    }

                    let y = ctx * res.total_width / 2.0;
                    let yy = y * y;
                    let szy = seq.csz * res.gn_frac * rpi * y;
                    let cc2 = szy * (seq.cc2p - 1.0 + res.gn_frac);
                    let cs2 = szy * seq.cs2p;
                    let ccg = szy * res.gg_frac;
                    let ccf = szy * res.gf_frac;

                    for ie in i0..i7 {
                        let x = ctx * (es[ie] - res.energy);
                        let (rew, aimw) = line_shape(x, y, yy, table);
                        cap[itemp][ie] += ccg * rew;
                        fis[itemp][ie] += ccf * rew;
                        els[itemp][ie] += cc2 * rew + cs2 * aimw;
                    }
                }
            }
        }

        // purr.f90:2211-2216 — clamp negative elastic cross sections.
        for itemp in 0..ntemp {
            for ie in 0..nsamp {
                if els[itemp][ie] < -bkg[1] {
                    els[itemp][ie] = -bkg[1] + 1.0 / BIG;
                }
            }
        }

        // purr.f90:2218-2250 — running infinite-dilution average
        // (temperature index 0 only, matching upstream's `els(1,ie)` etc).
        let (mut totf, mut elsf, mut capf, mut fisf) = (0.0, 0.0, 0.0, 0.0);
        for ie in 0..nsamp {
            totf += els[0][ie] + fis[0][ie] + cap[0][ie] + bkg[0];
            elsf += els[0][ie] + bkg[1];
            capf += cap[0][ie] + bkg[3];
            fisf += fis[0][ie] + bkg[2];
        }
        totf /= nsamp as f64;
        elsf /= nsamp as f64;
        capf /= nsamp as f64;
        fisf /= nsamp as f64;
        tav += totf;
        tvar += totf * totf;
        eav += elsf;
        evar += elsf * elsf;
        fav += fisf;
        fvar += fisf * fisf;
        cav += capf;
        cvar += capf * capf;

        // purr.f90:2256-2268's `nmode==1` renormalization branch is not
        // ported: `nmode` is initialised to `0` in the driver and never set
        // to `1` anywhere else in `purr.f90` — an always-inactive code path
        // upstream (no card or feature sets it), so there is nothing to
        // reproduce.

        // purr.f90:2270-2356 — per-temperature bin construction (first
        // ladder only) and histogram/Bondarenko accumulation (every ladder).
        for itemp in 0..ntemp {
            if iladr == 0 {
                let mut es_tot: Vec<f64> = (0..nsamp)
                    .map(|ie| els[itemp][ie] + fis[itemp][ie] + cap[itemp][ie] + bkg[0])
                    .collect();
                es_tot.sort_by(|a, b| a.partial_cmp(b).unwrap());
                tmin[itemp] = es_tot[0];
                tmax[itemp] = es_tot[nsamp - 1];

                // purr.f90:2283-2319 — dynamic non-uniform bin-edge schedule:
                // bins near the extremes of the distribution are narrower
                // (finer resolution in the tails) than bins in the middle.
                let nebin = (nsamp as f64 / (nbin as f64 - 10.0 + 1.76)) as i64;
                let mut ibin = (nebin / 200).max(1);
                for i in 0..nbin - 1 {
                    let sample_idx = (ibin as usize).clamp(1, nsamp) - 1;
                    tval[itemp][i] = es_tot[sample_idx];
                    if i > 0 && tval[itemp][i] <= tval[itemp][i - 1] {
                        tval[itemp][i] = tval[itemp][i - 1] + tval[itemp][i - 1] / 20.0;
                    }
                    if i == 0 {
                        ibin += nebin / 40;
                    } else if i == 1 {
                        ibin += nebin / 10;
                    } else if i == 2 {
                        ibin += nebin / 4;
                    } else if i == 3 {
                        ibin += nebin / 2;
                    } else if i > 3 && i < nbin - 6 {
                        ibin += nebin;
                    } else if i == nbin - 6 {
                        ibin += nebin / 2;
                    } else if i == nbin - 5 {
                        ibin += nebin / 4;
                    } else if i == nbin - 4 {
                        ibin += nebin / 10;
                    } else if i == nbin - 3 {
                        ibin += nebin / 40;
                    } else if i == nbin - 2 {
                        ibin += nebin / 200;
                    }
                }
                tval[itemp][nbin - 1] = BIG;
                tabl[itemp] = vec![[0.0; 5]; nbin];
                tsum[itemp] = 0.0;
            }

            // purr.f90:2329-2353 — accumulate this ladder's samples into the
            // histogram and the Bondarenko moments.
            for ie in 0..nsamp {
                let tot = els[itemp][ie] + fis[itemp][ie] + cap[itemp][ie] + bkg[0];
                if tot < tmin[itemp] {
                    tmin[itemp] = tot;
                }
                if tot > tmax[itemp] {
                    tmax[itemp] = tot;
                }
                let ii = tval[itemp].partition_point(|&v| v < tot).min(nbin - 1);
                tsum[itemp] += 1.0;
                tabl[itemp][ii][0] += 1.0;
                tabl[itemp][ii][1] += tot;
                tabl[itemp][ii][2] += els[itemp][ie] + bkg[1];
                tabl[itemp][ii][3] += fis[itemp][ie] + bkg[2];
                tabl[itemp][ii][4] += cap[itemp][ie] + bkg[3];
                for (isig0, &s0) in sig0.iter().enumerate() {
                    let tem = s0 / (s0 + tot);
                    let b = &mut bval[itemp][isig0];
                    b[0] += tot * tem;
                    b[1] += (els[itemp][ie] + bkg[1]) * tem;
                    b[2] += (fis[itemp][ie] + bkg[2]) * tem;
                    b[3] += (cap[itemp][ie] + bkg[3]) * tem;
                    b[4] += tot * tem * tem;
                    b[5] += tem;
                    b[6] += tem * tem;
                }
            }
        }
    }
    let _ = ns;

    // --- Post-processing -------------------------------------------------
    // purr.f90:2361-2387 — overall convergence diagnostics.
    let nladr_f = nladr as f64;
    let mean = |sum: f64| sum / nladr_f;
    let pct_std = |sumsq: f64, mean_val: f64| {
        let var = (sumsq / nladr_f - mean_val * mean_val).max(0.0);
        if mean_val != 0.0 {
            100.0 * var.sqrt() / mean_val
        } else {
            0.0
        }
    };
    let (m_tot, m_els, m_fis, m_cap) = (mean(tav), mean(eav), mean(fav), mean(cav));
    let convergence = ConvergenceStats {
        mean_total: m_tot,
        pct_std_total: pct_std(tvar, m_tot),
        mean_elastic: m_els,
        pct_std_elastic: pct_std(evar, m_els),
        mean_fission: m_fis,
        pct_std_fission: pct_std(fvar, m_fis),
        mean_capture: m_cap,
        pct_std_capture: pct_std(cvar, m_cap),
    };

    let sigi_total = inf_dilution.sigma_elastic_inf
        + inf_dilution.sigma_fission_inf
        + inf_dilution.sigma_capture_inf
        + spot
        + bkg[0];
    let sigi = [
        sigi_total,
        inf_dilution.sigma_elastic_inf + spot + bkg[1],
        inf_dilution.sigma_fission_inf + bkg[2],
        inf_dilution.sigma_capture_inf + bkg[3],
    ];

    let mut tables = Vec::with_capacity(ntemp);
    for itemp in 0..ntemp {
        // purr.f90:2395-2411 — Bondarenko cross sections from direct
        // sampling.
        let mut bondarenko_direct = vec![[0.0f64; 5]; nsig0];
        for isig0 in 0..nsig0 {
            let b = &bval[itemp][isig0];
            bondarenko_direct[isig0] = [
                b[0] / b[5],
                b[1] / b[5],
                b[2] / b[5],
                b[3] / b[5],
                b[4] / b[6],
            ];
        }

        // purr.f90:2413-2444 — normalize the probability table (bin
        // probability and bin-average cross sections).
        let mut bin_probability = vec![0.0f64; nbin];
        let mut bin_xs = vec![[0.0f64; 4]; nbin];
        for i in 0..nbin {
            let denom = if tabl[itemp][i][0] == 0.0 {
                1.0
            } else {
                tabl[itemp][i][0]
            };
            bin_probability[i] = tabl[itemp][i][0] / tsum[itemp];
            bin_xs[i] = [
                tabl[itemp][i][1] / denom,
                tabl[itemp][i][2] / denom,
                tabl[itemp][i][3] / denom,
                tabl[itemp][i][4] / denom,
            ];
        }

        // purr.f90:2468-2507 — Bondarenko cross sections computed from the
        // (now-normalized) probability table, as an independent cross-check
        // on the direct-sampling values above.
        let mut bondarenko_from_table = vec![[0.0f64; 5]; nsig0];
        for isig0 in 0..nsig0 {
            let mut acc = [0.0f64; 7];
            for i in 0..nbin {
                if bin_probability[i] == 0.0 {
                    continue;
                }
                let den = sig0[isig0] / (sig0[isig0] + bin_xs[i][0]);
                let ttt = bin_probability[i];
                acc[0] += ttt * bin_xs[i][0] * den;
                acc[1] += ttt * bin_xs[i][1] * den;
                acc[2] += ttt * bin_xs[i][2] * den;
                acc[3] += ttt * bin_xs[i][3] * den;
                acc[4] += ttt * bin_xs[i][0] * den * den;
                acc[5] += ttt * den;
                acc[6] += ttt * den * den;
            }
            bondarenko_from_table[isig0] = [
                acc[0] / acc[5],
                acc[1] / acc[5],
                acc[2] / acc[5],
                acc[3] / acc[5],
                acc[4] / acc[6],
            ];
        }

        // purr.f90:2509-2530 — renormalize the probability table and the
        // table-derived Bondarenko cross sections to the infinite-dilution
        // reference. The anchor for *each* reaction component is that
        // component's own most-dilute (`isig0=0`) Bondarenko-from-table value
        // (`sigf(j,1,itemp)` upstream — every component divides by its own
        // anchor, not a shared one); the *multiplier* is the matching
        // `sigi` reference, except component 4 (current-weighted "transport"
        // total, `tabl` has no such column so only `bondarenko` carries it),
        // which is scaled toward `sigi[0]` (total) — `purr.f90:2521-2523`'s
        // `k=j; if (j.eq.5) k=1`.
        let anchor = bondarenko_from_table[0];
        for row in &mut bin_xs {
            for (k, comp) in row.iter_mut().enumerate() {
                if anchor[k] != 0.0 {
                    *comp *= sigi[k] / anchor[k];
                }
            }
        }
        let mut bondarenko = bondarenko_from_table.clone();
        for row in &mut bondarenko {
            for k in 0..5 {
                if anchor[k] != 0.0 {
                    let sigi_k = if k == 4 { sigi[0] } else { sigi[k] };
                    row[k] *= sigi_k / anchor[k];
                }
            }
        }

        tables.push(ProbabilityTable {
            bin_upper_edge: tval[itemp].clone(),
            bin_probability,
            bin_xs,
            bondarenko,
            bondarenko_direct_sampling: bondarenko_direct,
        });
    }

    Ok(ProbabilityTableResult {
        tables,
        convergence,
    })
}

/// Run the PURR card-input driver. Placeholder — the ported pieces (ENDF
/// parsing via [`crate::unresr::mf2`], [`generate_ladder`],
/// [`infinite_dilution_reference`], [`read_heating_cross_sections`],
/// [`wfun::uw2`], [`probability_table`]) are reached directly; the PENDF
/// MT=152/MT=153 output-tape bookkeeping is not ported — see the module docs.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "purr driver (PENDF MT=152/153 tape writer not ported — see module docs)",
    ))
}
