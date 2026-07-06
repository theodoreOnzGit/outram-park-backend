//! `UNRESR` — effective self-shielded cross sections in the unresolved range.
//!
//! Produces effective self-shielded cross sections in the unresolved
//! resonance range (URR), where ENDF gives only average widths/spacings plus
//! distribution functions rather than individual resonances. Uses the
//! **ETOX/MC2-2 method** (Bondarenko-style narrow-resonance flux weighting,
//! with proper Porter-Thomas width-fluctuation quadrature — see
//! `unresx.tex`'s theory section) to tabulate σ(E, T, σ₀) versus background
//! ("dilution") cross section σ₀ — the precursor data GROUPR needs and,
//! historically, PURR's ancestor.
//!
//! Ported from `unresr.f90` (1665 lines). Layout:
//!
//! - [`mf2`] — ENDF File 2 LRU=2 parameter reader (`rdunf2`) and File 3
//!   background cross sections (`rdunf3`).
//! - [`wfun`] — the complex probability integral `w(z)` and the J/K
//!   width-fluctuation integrals (`uw`/`uwtab`/`quikw`/`ajku`).
//! - This module — the per-energy self-shielded cross-section calculator
//!   ([`unresolved_cross_sections`], ported from `unresl`) and its
//!   penetrability/interpolation helpers (`penetrability_factor`/`intrf`/`intr`).
//!
//! **Status:** the physics kernel is ported (ENDF parsing, the W-function
//! library, and `unresl` itself); the NJOY card-input driver (`run()`) and
//! the PENDF MT=152 output-tape bookkeeping remain
//! [`crate::NjoyError::NotPorted`] — this crate has no established "write an
//! unresolved-region PENDF section" concept yet (unlike ACE, which does), and
//! that bookkeeping is pure tape plumbing, not physics; see `README.md` for
//! the full status and caveats.

pub mod mf2;
pub mod wfun;

use crate::common::phys::{BK_EV_PER_K, PI};
use crate::endf::interp::{terp1, IntLaw};
use crate::reconr::slbw::{channel_radius, phase_shift, WAVE_K};
use crate::unresr::mf2::UnresolvedCase;
use crate::unresr::wfun::{ajk, qp_node, qw_weight, WTable};
use crate::NjoyError;

/// Penetrability factor `V_l(ρ)` and phase shift `φ_l(ρ_c)` for the
/// unresolved region — ported from `uunfac` (`unresr.f90:1213-1237`).
///
/// **Not the same formula as [`crate::reconr::slbw::shift_and_penetrability`]**:
/// the unresolved-region convention normalises out one extra factor of `ρ`
/// (`V_l = P_l(ρ)/ρ`, so that `Γn(E) = GNO·√E·V_l(E)` matches the reduced
/// average neutron width convention `unresx.tex` uses), whereas RECONR's
/// resolved-region `P_l` is the raw penetrability. The phase-shift formula
/// *is* identical between the two regions for `l=0,1,2` — reused directly
/// via [`phase_shift`] rather than re-derived.
///
/// `l` — orbital angular momentum (0, 1, or 2; NJOY's unresolved region never
/// needs `l≥3`). `rho` — `k·a` (channel radius). `rho_c` — `k·AP` (scattering
/// radius). Returns `(V_l, φ_l)` (`unresl` always calls `uunfac` with its
/// `amun` argument fixed at `1.0` — `unresl:1021` — and applies the real
/// `AMUN`-derived scaling separately when it forms `Γn(E)`; this port keeps
/// that same separation, so `penetrability_factor` takes no `amun` argument).
pub fn penetrability_factor(l: i32, rho: f64, rho_c: f64) -> (f64, f64) {
    let r2 = rho * rho;
    match l {
        0 => (1.0, rho_c),
        1 => (r2 / (1.0 + r2), phase_shift(1, rho_c)),
        _ => {
            let r4 = r2 * r2;
            (r4 / (9.0 + 3.0 * r2 + r4), phase_shift(2, rho_c))
        }
    }
}

/// Interpolate the Case-B energy-dependent fission width `GF(E)` from the
/// shared fission-energy grid — ported from `intrf` (`unresr.f90:1239-1259`).
/// `unresl` always forces linear-linear interpolation here (`intl=2`,
/// `unresl:1051`) regardless of what ENDF's own `INT` field for this range
/// says; ported as the same hard-coded override, not a "fix".
pub(crate) fn interp_case_b_fission_width(e: f64, energies: &[f64], widths: &[f64]) -> Result<f64, NjoyError> {
    for i in 1..energies.len() {
        if e >= energies[i - 1] && e <= energies[i] {
            return terp1(energies[i - 1], widths[i - 1], energies[i], widths[i], e, IntLaw::LinLin);
        }
    }
    Ok(*widths.last().unwrap_or(&0.0))
}

/// Interpolate all five Case-C energy-dependent parameters (`D`, `GX`,
/// `GNO`, `GG`, `GF`) at energy `e` — ported from `intr`
/// (`unresr.f90:1261-1289`). Also forces linear-linear interpolation
/// (`int=2`, `unresl:1058`) regardless of the stored `INT`.
///
/// Returns `(d, gx, gno, gg, gf)`, with `gx`/`gf` floored to `0.0` below
/// `1e-8` (`unresr.f90:1286-1287`: negligible competitive/fission width is
/// treated as exactly absent, not left as quadrature-polluting noise).
pub(crate) fn interp_case_c(
    e: f64,
    points: &[mf2::UnresolvedPointC],
) -> Result<(f64, f64, f64, f64, f64), NjoyError> {
    const SMALL: f64 = 1e-8;
    let mut d = 0.0;
    let mut gx = 0.0;
    let mut gno = 0.0;
    let mut gg = 0.0;
    let mut gf = 0.0;
    for i in 1..points.len() {
        let (p0, p1) = (points[i - 1], points[i]);
        if e >= p0.e && e <= p1.e {
            d = terp1(p0.e, p0.d, p1.e, p1.d, e, IntLaw::LinLin)?;
            gx = terp1(p0.e, p0.gx, p1.e, p1.gx, e, IntLaw::LinLin)?;
            gno = terp1(p0.e, p0.gno, p1.e, p1.gno, e, IntLaw::LinLin)?;
            gg = terp1(p0.e, p0.gg, p1.e, p1.gg, e, IntLaw::LinLin)?;
            gf = terp1(p0.e, p0.gf, p1.e, p1.gf, e, IntLaw::LinLin)?;
        }
    }
    if gx < SMALL {
        gx = 0.0;
    }
    if gf < SMALL {
        gf = 0.0;
    }
    Ok((d, gx, gno, gg, gf))
}

/// Channel radius `a` \[10⁻¹² cm\] per the `NAPS` convention — ported from
/// `unresl:995-1005`. Reuses [`crate::reconr::slbw::channel_radius`] for the
/// `NAPS=0`/`NAPS=1` cases (the identical formula); `NAPS=2` (energy-dependent
/// scattering radius via `NRO=1`) is rejected earlier, in
/// [`mf2::parse_lru2_ranges`], so it never reaches here.
pub(crate) fn channel_radius_urr(awri: f64, naps: i32, ap: f64) -> Result<f64, NjoyError> {
    if naps == 0 || naps == 1 {
        Ok(channel_radius(awri, naps, ap))
    } else {
        Err(NjoyError::EndfParse(format!("unresr: unsupported naps={naps}")))
    }
}

/// One resonance sequence's parameters at the energy being evaluated,
/// flattened out of whichever [`UnresolvedCase`] variant it came from — the
/// common shape `unresl`'s per-(l,j) inner loop body needs, regardless of
/// which ENDF representation (A/B/C) supplied it.
pub(crate) struct SequenceParams {
    pub(crate) l: i32,
    pub(crate) abn: f64,
    pub(crate) d: f64,
    pub(crate) aj: f64,
    pub(crate) amun: f64,
    pub(crate) gno: f64,
    pub(crate) gg: f64,
    pub(crate) gf: f64,
    pub(crate) amuf: f64,
    pub(crate) gx: f64,
    pub(crate) amux: f64,
    pub(crate) awri: f64,
    pub(crate) ap: f64,
    pub(crate) naps: i32,
    pub(crate) spi: f64,
}

/// Flatten one range's L/J-state tree into a plain list of
/// [`SequenceParams`], evaluating any Case-B/C energy-dependent parameters at
/// `e` via [`interp_case_b_fission_width`]/[`interp_case_c`] — this is the
/// data-gathering half of `unresl`'s combined walk-and-compute loop
/// (`unresl:954-1149`); walking the tree once here (rather than twice, as
/// two separate closures previously attempted) keeps the accumulation code
/// below a plain, borrow-checker-friendly loop over an owned `Vec`.
pub(crate) fn range_sequences(range: &mf2::UnresolvedRange, e: f64) -> Vec<SequenceParams> {
    let mut out = Vec::new();
    match &range.case_ {
        UnresolvedCase::CaseA { awri, ap, spi, l_states } => {
            for ls in l_states {
                for j in &ls.j_states {
                    out.push(SequenceParams {
                        l: ls.l,
                        abn: range.abn,
                        d: j.d,
                        aj: j.aj,
                        amun: j.amun,
                        gno: j.gno,
                        gg: j.gg,
                        gf: 0.0,
                        amuf: 0.0,
                        gx: 0.0,
                        amux: 0.0,
                        awri: *awri,
                        ap: *ap,
                        naps: range.naps,
                        spi: *spi,
                    });
                }
            }
        }
        UnresolvedCase::CaseB { awri, ap, spi, fission_energies, l_states } => {
            for ls in l_states {
                for j in &ls.j_states {
                    let gf = interp_case_b_fission_width(e, fission_energies, &j.gf).unwrap_or(0.0);
                    out.push(SequenceParams {
                        l: ls.l,
                        abn: range.abn,
                        d: j.d,
                        aj: j.aj,
                        amun: j.amun,
                        gno: j.gno,
                        gg: j.gg,
                        gf,
                        amuf: j.amuf,
                        gx: 0.0,
                        amux: 0.0,
                        awri: *awri,
                        ap: *ap,
                        naps: range.naps,
                        spi: *spi,
                    });
                }
            }
        }
        UnresolvedCase::CaseC { awri, ap, spi, l_states } => {
            for ls in l_states {
                for j in &ls.j_states {
                    if let Ok((d, gx, gno, gg, gf)) = interp_case_c(e, &j.points) {
                        out.push(SequenceParams {
                            l: ls.l,
                            abn: range.abn,
                            d,
                            aj: j.aj,
                            amun: j.amun,
                            gno,
                            gg,
                            gf,
                            amuf: j.amuf,
                            gx,
                            amux: j.amux,
                            awri: *awri,
                            ap: *ap,
                            naps: range.naps,
                            spi: *spi,
                        });
                    }
                }
            }
        }
    }
    out
}

/// Number of quadrature points for a width distribution with `mu` degrees of
/// freedom: `1` (fixed width, no quadrature) if `mu=0`, else the shared
/// 10-point rule — `unresl:1092-1097`'s `nqf`/`nqn`/`nqx` selection.
fn quadrature_points(mu: i32) -> usize {
    if mu <= 0 {
        1
    } else {
        10
    }
}

/// Computes effective self-shielded cross sections at one energy, using the
/// ETOX method — ported from `unresl` (`unresr.f90:842-1211`).
///
/// - `ranges` — every parsed [`mf2::UnresolvedRange`] for the material
///   (possibly spanning several isotopes, mirroring `rdunf2`'s
///   `nsect`-indexed storage); only ranges whose `[el, eh]` brackets `e`
///   contribute.
/// - `e` — energy \[eV\] (**must be positive** — the resolved/unresolved
///   overlap sign convention from [`mf2::ilist`]'s marking is the caller's
///   concern, not this function's; pass `e.abs()`).
/// - `t_kelvin` — temperature \[K\] (floored to 1 K, matching
///   `unresl:917-918`'s `if (tempu(1).lt.one) tempu(1)=one`).
/// - `sig0` — the dilution (`σ₀`) values to tabulate, one output row each.
/// - `sigbkg` — File-3 background cross sections `[total, elastic, fission,
///   capture]` \[b\] at this energy (zeroed by the caller when `LSSF>0` —
///   see the crate-level README; `unresl` itself does not consult `LSSF`).
/// - `table` — the precomputed [`WTable`] (build once, reuse across energies
///   — `unresl`'s Fortran equivalent is the module-level `tr`/`ti` populated
///   once by `uwtab`).
///
/// Returns one `[total, elastic, fission, capture, transport]` \[b\] row per
/// `sig0` entry (`unresl:1193-1200`'s `sigu(1..5, is)` output order).
pub fn unresolved_cross_sections(
    ranges: &[mf2::UnresolvedRange],
    e: f64,
    t_kelvin: f64,
    sig0: &[f64],
    sigbkg: [f64; 4],
    table: &WTable,
) -> Result<Vec<[f64; 5]>, NjoyError> {
    let temp = t_kelvin.max(1.0);
    let e2 = e.sqrt();
    let nsig0 = sig0.len();

    // Gather every contributing sequence across all ranges bracketing `e`,
    // with Case-B/C interpolation already resolved at this energy.
    let mut sequences: Vec<SequenceParams> = Vec::new();
    for range in ranges {
        if e < range.el || e > range.eh {
            continue;
        }
        sequences.extend(range_sequences(range, e));
    }

    // --- Pass 1: potential scattering + interference correction ------------
    // unresl:920-937, 954-1073 (ispot==0 branch) — accumulated across every
    // contributing sequence using only nuclide/geometry quantities, no
    // fluctuation-integral quadrature yet.
    let mut spot = 0.0;
    let mut sint = 0.0;
    {
        let mut last_l: Option<i32> = None;
        for seq in &sequences {
            let aa = channel_radius_urr(seq.awri, seq.naps, seq.ap)?;
            let rat = seq.awri / (seq.awri + 1.0);
            let k = WAVE_K * rat * e2;
            let ab = 4.0 * PI / (k * k);
            let rho = k * aa;
            let rho_c = k * seq.ap;
            let (vl, ps) = penetrability_factor(seq.l, rho, rho_c);

            let gj = (2.0 * seq.aj + 1.0) / (4.0 * seq.spi + 2.0);
            let gnx = seq.gno * vl * e2 * seq.amun;

            // unresl:1072 — the potential-scattering term is added once per
            // `L` (its first J-state, `j.eq.1`), not once per J-state.
            if last_l != Some(seq.l) {
                spot += seq.abn * ab * (2.0 * seq.l as f64 + 1.0) * ps.sin().powi(2);
                last_l = Some(seq.l);
            }
            sint -= seq.abn * ab * PI * gj * gnx * ps.sin().powi(2) / seq.d;
        }
    }

    let sigbt = sigbkg[0] + spot + sint;
    let sigm: Vec<f64> = sig0.iter().map(|s0| sigbt + s0).collect();
    for &s in &sigm {
        if s < 0.0 {
            log::warn!("unresr: negative background xs in urr may cause issues — check the evaluation");
        }
    }

    // --- Pass 2: fluctuation-integral quadrature per sequence ---------------
    // unresl:1076-1146 (ispot==1 branch). `tj[ks][is0]`/`tk[ks][is0]` are the
    // per-sequence total/current-weighted fluctuation sums;
    // `t[ks][is0][kx]` the four partial (fission/gamma/neutron/competitive)
    // sums feeding the cross-sequence overlap correction below.
    let ns = sequences.len();
    let mut t = vec![vec![[0.0f64; 4]; nsig0]; ns];
    let mut tj = vec![vec![0.0f64; nsig0]; ns];
    let mut tk = vec![vec![0.0f64; nsig0]; ns];

    for (ks, seq) in sequences.iter().enumerate() {
        let aa = channel_radius_urr(seq.awri, seq.naps, seq.ap)?;
        let rat = seq.awri / (seq.awri + 1.0);
        let k = WAVE_K * rat * e2;
        let ab = 4.0 * PI / (k * k);
        let rho = k * aa;
        let rho_c = k * seq.ap;
        let (vl, _ps) = penetrability_factor(seq.l, rho, rho_c);
        let del = 2.0 * e2 * (temp * BK_EV_PER_K / seq.awri).sqrt();

        let gj = (2.0 * seq.aj + 1.0) / (4.0 * seq.spi + 2.0);
        let gnx = seq.gno * vl * e2 * seq.amun;

        let mu_f = if seq.gf.abs() <= 1e-8 { 0 } else { seq.amuf.round() as i32 };
        let mu_n = seq.amun.round() as i32;
        let mu_x = if seq.gx.abs() < 1e-8 { 0 } else { seq.amux.round() as i32 };
        let (nqf, nqn, nqx) = (quadrature_points(mu_f), quadrature_points(mu_n), quadrature_points(mu_x));

        for kf in 0..nqf {
            let gf_node = if mu_f == 0 { seq.gf } else { qp_node(kf, mu_f) * seq.gf };
            for kn in 0..nqn {
                let gn_node = if mu_n == 0 { gnx } else { qp_node(kn, mu_n) * gnx };
                for kl in 0..nqx {
                    let gx_node = if mu_x == 0 { seq.gx } else { qp_node(kl, mu_x) * seq.gx };
                    let gg = [gf_node, seq.gg, gn_node, gx_node];
                    let g_total = gg[0] + gg[1] + gg[2] + gg[3];
                    if g_total <= 0.0 {
                        continue;
                    }
                    let s0u_denom = gg[2];
                    let sti = g_total / del;

                    for (is0, &sigm_val) in sigm.iter().enumerate() {
                        let s0u = ab * gj * s0u_denom / g_total;
                        let beta = sigm_val / s0u;
                        let (mut xj, mut xk) = ajk(table, beta, sti);
                        if mu_f > 0 {
                            xj *= qw_weight(kf, mu_f);
                            xk *= qw_weight(kf, mu_f);
                        }
                        if mu_n > 0 {
                            xj *= qw_weight(kn, mu_n);
                            xk *= qw_weight(kn, mu_n);
                        }
                        if mu_x > 0 {
                            xj *= qw_weight(kl, mu_x);
                            xk *= qw_weight(kl, mu_x);
                        }
                        for kx in 0..4 {
                            t[ks][is0][kx] += xj * gg[kx];
                        }
                        tk[ks][is0] += xk * g_total;
                    }
                }
            }
        }

        for is0 in 0..nsig0 {
            tk[ks][is0] /= seq.d;
            let mut ttj = 0.0;
            for kx in 0..4 {
                ttj += t[ks][is0][kx];
                t[ks][is0][kx] /= seq.d;
            }
            tj[ks][is0] = ttj / seq.d;
        }
    }

    // --- Sum over sequences with in-sequence / sequence-sequence overlap ---
    // corrections — unresl:1152-1191.
    let mut out = vec![[0.0f64; 5]; nsig0];
    for is0 in 0..nsig0 {
        let mut yy = [0.0f64; 3];
        let mut yj = 0.0;
        let mut yk = 0.0;
        for ks in 0..ns {
            let mut xj = 0.0;
            let mut xk = 0.0;
            for ksp in 0..ns {
                if ksp != ks {
                    xj += tj[ksp][is0] * sequences[ksp].abn;
                    xk += tk[ksp][is0] * sequences[ksp].abn;
                }
            }
            for kx in 0..3 {
                yy[kx] += t[ks][is0][kx] * (1.0 - xj) * sequences[ks].abn;
            }
            let ttt = tj[ks][is0] * (1.0 - xj) * sequences[ks].abn;
            yj += ttt;
            yk += (tk[ks][is0] - tj[ks][is0]) * (1.0 - xk) * sequences[ks].abn + ttt;
        }

        let mut sigf = [0.0f64; 4]; // [fission, gamma, neutron, sum(0..3)]
        for i in 0..3 {
            sigf[i] = sigm[is0] * yy[i] / (1.0 - yj);
            sigf[3] += sigf[i];
        }
        let term = (1.0 - yj) / (1.0 - yk);
        let term1 = sig0[is0] * (yk - yj) / (1.0 - yk);
        let sigtr = sigbt * term + term1;

        // unresl:1193-1200 — output order: total, elastic, fission, capture, transport.
        out[is0][0] = sigf[3] + sigbt;
        out[is0][1] = sigf[2] + sigbkg[1] + spot + sint;
        out[is0][2] = sigf[0] + sigbkg[2];
        out[is0][3] = sigf[1] + sigbkg[3];
        out[is0][4] = sigtr;
    }

    Ok(out)
}

/// Run the UNRESR card-input driver. Placeholder — the ENDF parsing and
/// physics kernel are reached directly through [`mf2::parse_lru2_ranges`] and
/// [`unresolved_cross_sections`]; the PENDF MT=152 output-tape bookkeeping is
/// not ported (see the module docs and `README.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("unresr driver (physics: crate::unresr::unresolved_cross_sections)"))
}
