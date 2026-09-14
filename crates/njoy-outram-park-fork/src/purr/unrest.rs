//! `unrest` — the Monte Carlo probability-table core (`purr.f90:1950-2204`
//! and the binning/Bondarenko-moment pipeline around it), plus the four-tier
//! Doppler line-shape evaluator it drives.
//!
//! Split out of `purr/mod.rs` (see the module doc there for the port map,
//! the tier-classification equivalence argument, and status); this is the
//! "Monte Carlo core" half of the plan-named boundary, ladder generation
//! lives in [`super::ladder`].

use super::ladder::{generate_ladder, InfiniteDilutionResult, Rng, SequenceLadderParams};
use super::wfun::DopplerTable;
use crate::NjoyError;

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
pub(super) fn line_shape(x: f64, y: f64, yy: f64, table: &DopplerTable) -> (f64, f64) {
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
/// - `sequences` / `inf_dilution` — from [`super::ladder::infinite_dilution_reference`] at
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
