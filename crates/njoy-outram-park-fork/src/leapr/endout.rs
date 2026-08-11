// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `endout` — write the LEAPR results as an ENDF-6 MF=7 thermal-scattering tape.
//!
//! Ported from NJOY2016 `leapr.f90::endout` (lines 2972–3623). Turns the LEAPR
//! `S(alpha, beta)` arrays and coherent-elastic Bragg edges into in-memory ENDF
//! sections, laid out exactly as THERMR's MF=7 reader expects
//! ([`crate::thermr::mf7`]):
//!
//! - **MT=2** thermal elastic — coherent (`LTHR=1`, Bragg `S(E)`) or incoherent
//!   (`LTHR=2`, Debye-Waller `W'(T)`).
//! - **MT=4** incoherent inelastic — the `B` constants, a TAB2 over `beta`, one
//!   `S(alpha)` TAB1 per `beta` and temperature, and the trailing effective-
//!   temperature TAB1.
//!
//! The result is a [`Tape`] whose MF=7 sections round-trip back through
//! [`crate::thermr::mf7::parse_mf7`] — this is the module's primary verification
//! gate (see the tests).
//!
//! ## Scope / honest gaps
//!
//! - **File 1 (MF=1/MT=451) is not written.** The Fortran `endout` emits a
//!   descriptive MF=1 header and Hollerith comment cards (3052–3156); those carry
//!   no `S(alpha,beta)` physics and the crate's `[f64; 6]` row model cannot store
//!   Hollerith text (same limitation MIXR documents). MF=7 is complete; MF=1 is
//!   omitted.
//! - **Mixed-moderator merge** (`nss != 0`, 3017–3030 and the secondary `ssp`
//!   plumbing) is **not** ported — single principal scatterer only.
//! - The `sigfig`-rounding and `smin` flooring of every `S` value **are** ported
//!   (they match the Fortran bit-for-bit through [`crate::mixr::mix::sigfig`]).
//!
//! **Untrusted AI draft.** Verified by a LEAPR→THERMR round-trip on synthetic and
//! `coher`-derived data; **not** validated against a reference NJOY MF=7 tape.

use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::leapr::coher::BraggEdges;
use crate::leapr::SabMatrix;
use crate::mixr::mix::sigfig;

/// 0.0253 eV thermal reference used as the `LAT=1` scaling energy (`therm`).
const THERM: f64 = 0.0253;
/// Boltzmann constant \[eV/K\] (`physics::bk`).
const BK: f64 = crate::common::phys::BK_EV_PER_K;

/// The elastic part to emit in MF=7/MT=2.
#[derive(Debug, Clone)]
pub enum ElasticOutput {
    /// No elastic section.
    None,
    /// Coherent (Bragg) elastic, `LTHR=1`: the edges from [`crate::leapr::coher`].
    Coherent(BraggEdges),
    /// Incoherent elastic, `LTHR=2`: a bound cross section `sb * npr` \[barn\]
    /// (the `SB` C1 field); the `W'(T)` table comes from the temperatures +
    /// Debye-Waller integrals in [`LeaprOutput`].
    Incoherent { sb_npr: f64 },
}

/// Everything `endout` needs from a completed LEAPR run for a single principal
/// scatterer.
///
/// Field units and meaning follow the LEAPR globals of the same name. `ssm[t]`
/// is the negative-beta asymmetric law at temperature `t`; `ssp[t]` (cold H/D
/// only) is the positive-beta law, required when `isym` is odd.
#[derive(Debug, Clone)]
pub struct LeaprOutput {
    /// ENDF MAT number.
    pub mat: i32,
    /// ZA = 1000*Z + A of the principal scatterer.
    pub za: f64,
    /// Atomic weight ratio of the principal scatterer.
    pub awr: f64,
    /// `LAT`: `1` if `alpha`/`beta` are scaled to 0.0253 eV, else `0`.
    pub lat: i32,
    /// `LASYM`/`isym`: `0` symmetric `S`; `1` symmetric `+/-beta`; `2` asymmetric
    /// `Ss` for `-beta`; `3` asymmetric `Ss` for `+/-beta`.
    pub isym: i32,
    /// `ilog`: if `true`, store `log10 S` instead of `S`.
    pub ilog: bool,
    /// Minimum stored `S` (values below are floored to `0` when `ilog=false`).
    pub smin: f64,
    /// Momentum-transfer grid `alpha` (dimensionless), ascending.
    pub alpha: Vec<f64>,
    /// Energy-transfer grid `beta` (dimensionless), ascending.
    pub beta: Vec<f64>,
    /// Temperatures \[K\], one per `ssm` entry.
    pub temperatures_k: Vec<f64>,
    /// Debye-Waller integral `W'(T)` \[1/eV\] per temperature (LEAPR `dwpix`
    /// after its `/(awr*T*bk)` conversion, 3035).
    pub dwpix: Vec<f64>,
    /// Effective (SCT) temperature `T_eff` \[K\] per temperature (LEAPR `tempf`).
    pub tempf: Vec<f64>,
    /// Negative-beta asymmetric law `Ss(alpha,-beta)` per temperature.
    pub ssm: Vec<SabMatrix>,
    /// Positive-beta asymmetric law `Ss(alpha,+beta)` per temperature (cold H/D;
    /// required iff `isym` is odd).
    pub ssp: Option<Vec<SabMatrix>>,
    /// Number of principal scattering atoms (`npr`).
    pub npr: i32,
    /// Free-atom cross section of the principal scatterer (`spr`) \[barn\].
    pub spr: f64,
    /// The elastic section to emit.
    pub elastic: ElasticOutput,
}

// ── ENDF record row-packing helpers ─────────────────────────────────────────
//
// Each pushes rows in the exact 6-field-per-line layout that
// `endf::records::SectionCursor` reads back.

/// Push a CONT record (one row).
fn push_cont(rows: &mut Vec<[f64; 6]>, c1: f64, c2: f64, l1: i32, l2: i32, n1: i32, n2: i32) {
    rows.push([c1, c2, l1 as f64, l2 as f64, n1 as f64, n2 as f64]);
}

/// Push `values` packed six per row (the payload of LIST / TAB1 / TAB2 bodies),
/// zero-padding the final short row.
fn push_packed(rows: &mut Vec<[f64; 6]>, values: &[f64]) {
    for chunk in values.chunks(6) {
        let mut row = [0.0_f64; 6];
        row[..chunk.len()].copy_from_slice(chunk);
        rows.push(row);
    }
}

/// Push a LIST record: CONT head (`N1 = data.len()`) then the packed data.
fn push_list(rows: &mut Vec<[f64; 6]>, c1: f64, c2: f64, l1: i32, l2: i32, n2: i32, data: &[f64]) {
    push_cont(rows, c1, c2, l1, l2, data.len() as i32, n2);
    push_packed(rows, data);
}

/// Push a TAB1 record: CONT head (`N1 = NR`, `N2 = NP`), the interpolation table,
/// then the `(x, y)` pairs — each body packed separately six per row.
fn push_tab1(
    rows: &mut Vec<[f64; 6]>,
    c1: f64,
    c2: f64,
    l1: i32,
    l2: i32,
    interp: &[(i32, i32)],
    pairs: &[(f64, f64)],
) {
    push_cont(rows, c1, c2, l1, l2, interp.len() as i32, pairs.len() as i32);
    let iflat: Vec<f64> = interp.iter().flat_map(|&(a, b)| [a as f64, b as f64]).collect();
    push_packed(rows, &iflat);
    let pflat: Vec<f64> = pairs.iter().flat_map(|&(a, b)| [a, b]).collect();
    push_packed(rows, &pflat);
}

/// Push a TAB2 record: CONT head (`N1 = NR`, `N2 = NZ`) then the interpolation
/// table (no data pairs).
fn push_tab2(
    rows: &mut Vec<[f64; 6]>,
    c1: f64,
    c2: f64,
    l1: i32,
    l2: i32,
    nz: i32,
    interp: &[(i32, i32)],
) {
    push_cont(rows, c1, c2, l1, l2, interp.len() as i32, nz);
    let iflat: Vec<f64> = interp.iter().flat_map(|&(a, b)| [a as f64, b as f64]).collect();
    push_packed(rows, &iflat);
}

// ── Section builders ────────────────────────────────────────────────────────

/// Build the MF=7/MT=2 **coherent** elastic section (`LTHR=1`,
/// `leapr.f90:3192–3289`).
///
/// Forms the cumulative, Debye-Waller-weighted structure factor
/// `S(E) = sum_{edges<=E} exp(-4 W' E_edge) f_edge` at the base temperature, with
/// the high-energy 1/E thinning (`jmax`) NJOY applies, and one LIST per extra
/// temperature.
fn build_coherent_elastic(out: &LeaprOutput, edges: &[(f64, f64)]) -> Vec<[f64; 6]> {
    const TOL: f64 = 0.9e-7;
    let nedge = edges.len();
    let ntempr = out.temperatures_k.len();
    let mut rows = Vec::new();

    // HEAD: ZA, AWR, LTHR=1, 0, 0, 0
    push_cont(&mut rows, out.za, out.awr, 1, 0, 0, 0);

    // thin out the 1/E tail using the first temperature's W' (3204–3216).
    let w0 = out.dwpix[0];
    let mut sum = 0.0;
    let mut suml = 0.0;
    let mut jmax = 0usize;
    for (j, &(e, f)) in edges.iter().enumerate() {
        sum += (-4.0 * w0 * e).exp() * f;
        if sum - suml > TOL * sum {
            jmax = j + 1; // 1-based index of the last retained edge
            suml = sum;
        }
    }
    if jmax == 0 {
        jmax = nedge.max(1);
    }

    // temperature 0: TAB1 with the (E, cumulative S) pairs (3219–3255).
    let w = out.dwpix[0];
    let mut pairs = vec![(0.0_f64, 0.0_f64); jmax];
    let mut sum = 0.0;
    for (j, &(e, f)) in edges.iter().enumerate() {
        let p = (j + 1).min(jmax); // 1-based pair slot; tail collapses onto jmax
        sum += (-4.0 * w * e).exp() * f;
        pairs[p - 1] = (sigfig(e, 7, 0), sigfig(sum, 7, 0));
    }
    push_tab1(
        &mut rows,
        out.temperatures_k[0],
        0.0,
        (ntempr - 1) as i32, // LT = number of extra temperatures
        0,
        &[(jmax as i32, 1)], // histogram interpolation
        &pairs,
    );

    // extra temperatures: one LIST of jmax S values each (3256–3286).
    for t in 1..ntempr {
        let w = out.dwpix[t];
        let mut svals = vec![0.0_f64; jmax];
        let mut sum = 0.0;
        for j in 0..nedge {
            // jj tracks the retained edge index; the tail reuses edge jmax (a
            // documented NJOY quirk — the temp-0 path above uses the real tail
            // energies instead).
            let jj = (j + 1).min(jmax);
            let (e, f) = edges[jj - 1];
            let e = sigfig(e, 7, 0);
            sum += (-4.0 * w * e).exp() * f;
            svals[jj - 1] = sigfig(sum, 7, 0);
        }
        push_list(&mut rows, out.temperatures_k[t], 0.0, 2, 0, 0, &svals);
    }

    rows
}

/// Build the MF=7/MT=2 **incoherent** elastic section (`LTHR=2`,
/// `leapr.f90:3158–3190`): one `W'(T)` TAB1 with `SB = sb_npr`.
fn build_incoherent_elastic(out: &LeaprOutput, sb_npr: f64) -> Vec<[f64; 6]> {
    let ntempr = out.temperatures_k.len();
    let mut rows = Vec::new();
    // HEAD: ZA, AWR, LTHR=2, 0, 0, 0
    push_cont(&mut rows, out.za, out.awr, 2, 0, 0, 0);
    // W'(T) TAB1: C1=SB, L1=0, one interp region (ndw, 2), (T, W') pairs.
    // NJOY pads to ndw>=2 when only one temperature is present (3174–3187).
    let ndw = ntempr.max(2);
    let mut pairs = Vec::with_capacity(ndw);
    for i in 0..ndw {
        if i < ntempr {
            pairs.push((out.temperatures_k[i], sigfig(out.dwpix[i], 7, 0)));
        } else {
            let prev = pairs[i - 1];
            pairs.push(prev);
        }
    }
    push_tab1(&mut rows, sb_npr, 0.0, 0, 0, &[(ndw as i32, 2)], &pairs);
    rows
}

/// Compute the stored `S` value for `(beta index i, alpha index j, temperature
/// nt)` under the `isym`/`ilog` conventions (`leapr.f90:3354–3451`,
/// `3471–3567`). `i` and `nbeta` are 0-based / lengths; `be` is the (scaled)
/// energy transfer already multiplied by the `LAT` factor.
fn stored_s(out: &LeaprOutput, i: usize, j: usize, nt: usize, be: f64) -> f64 {
    let nbeta = out.beta.len();
    let ssm = &out.ssm[nt];
    // The base asymmetric value, before any detailed-balance factor. For odd
    // `isym` the (+/-)-beta halves draw from ssm (i < nbeta) and ssp (i >= nbeta).
    let base = match out.isym {
        0 | 2 => ssm.get(i, j),
        1 | 3 => {
            let ssp = out.ssp.as_ref().expect("odd isym requires ssp").get(nt).unwrap();
            if i + 1 < nbeta {
                ssm.get(nbeta - 1 - i, j)
            } else {
                ssp.get(i + 1 - nbeta, j)
            }
        }
        _ => ssm.get(i, j),
    };
    // The detailed-balance factor (`isym` 0/1) is a multiply in the linear path
    // (`S*exp(-/+be/2)`) and an additive log-shift in the log path
    // (`log(S) -/+ be/2`) — `leapr.f90:3356-3450`.
    if out.ilog {
        if base > 0.0 {
            let shift = match out.isym {
                0 => -be / 2.0,
                1 => be / 2.0,
                _ => 0.0,
            };
            sigfig(base.ln() + shift, 7, 0)
        } else {
            -999.0
        }
    } else {
        let factor = match out.isym {
            0 => (-be / 2.0).exp(),
            1 => (be / 2.0).exp(),
            _ => 1.0,
        };
        let raw = base * factor;
        let v = if raw >= 1e-9 { sigfig(raw, 7, 0) } else { sigfig(raw, 6, 0) };
        if v < out.smin {
            0.0
        } else {
            v
        }
    }
}

/// The signed `beta` written for output row `i` (0-based) under `isym`
/// (`leapr.f90:3342–3346`).
fn output_beta(out: &LeaprOutput, i: usize) -> f64 {
    let nbeta = out.beta.len();
    if out.isym % 2 == 0 {
        out.beta[i]
    } else if i + 1 < nbeta {
        -out.beta[nbeta - 1 - i]
    } else {
        out.beta[i + 1 - nbeta]
    }
}

/// Build the MF=7/MT=4 incoherent-inelastic section (`leapr.f90:3291–3618`).
fn build_inelastic(out: &LeaprOutput) -> Vec<[f64; 6]> {
    let nalpha = out.alpha.len();
    let nbeta = out.beta.len();
    let ntempr = out.temperatures_k.len();
    let mut rows = Vec::new();

    // HEAD: ZA, AWR, 0, LAT, LASYM=isym, 0
    push_cont(&mut rows, out.za, out.awr, 0, out.lat, out.isym, 0);

    // B-constants LIST (3301–3323). NI=6 for a single scatterer; NS=0.
    let b = [
        (out.npr as f64) * out.spr,        // B(1) bound-ish xsec factor > 0
        out.beta[nbeta - 1],               // B(2)
        out.awr,                           // B(3) = A
        sigfig(THERM * out.beta[nbeta - 1], 7, 0), // B(4)
        0.0,                               // B(5)
        out.npr as f64,                    // B(6)
    ];
    let l1 = if out.ilog { 1 } else { 0 };
    push_list(&mut rows, 0.0, 0.0, l1, 0, 0, &b);

    // TAB2 over beta (3324–3334). nbt doubles for odd isym (+/- beta).
    let nbt = if out.isym == 1 || out.isym == 3 { 2 * nbeta - 1 } else { nbeta };
    push_tab2(&mut rows, 0.0, 0.0, 0, 0, nbt as i32, &[(nbt as i32, 4)]);

    // For each output beta, one S(alpha) TAB1 at temp 0 + a LIST per extra temp.
    for i in 0..nbt {
        for nt in 0..ntempr {
            let sc = if out.lat == 1 { THERM / (BK * out.temperatures_k[nt]) } else { 1.0 };
            let beta_i = output_beta(out, i);
            let be = beta_i * sc;
            if nt == 0 {
                let pairs: Vec<(f64, f64)> = (0..nalpha)
                    .map(|j| (out.alpha[j], stored_s(out, i, j, nt, be)))
                    .collect();
                push_tab1(
                    &mut rows,
                    out.temperatures_k[nt],
                    beta_i,
                    (ntempr - 1) as i32, // LT
                    0,
                    &[(nalpha as i32, 4)],
                    &pairs,
                );
            } else {
                let svals: Vec<f64> =
                    (0..nalpha).map(|j| stored_s(out, i, j, nt, be)).collect();
                push_list(&mut rows, out.temperatures_k[nt], beta_i, 4, 0, 0, &svals);
            }
        }
    }

    // Trailing effective-temperature TAB1 (3599–3617): (T, T_eff) pairs.
    let teff: Vec<(f64, f64)> = (0..ntempr)
        .map(|i| (sigfig(out.temperatures_k[i], 7, 0), sigfig(out.tempf[i], 7, 0)))
        .collect();
    push_tab1(&mut rows, 0.0, 0.0, 0, 0, &[(ntempr as i32, 2)], &teff);

    rows
}

/// Assemble the full MF=7 tape from a completed LEAPR run.
///
/// Produces a [`Tape`] with the MF=7/MT=2 elastic section (if any) and the
/// MF=7/MT=4 inelastic section, in file order, ready to be read back by
/// [`crate::thermr::mf7::parse_mf7`]. MF=1 is intentionally omitted (see the
/// [module docs](self)).
pub fn endout(out: &LeaprOutput) -> Tape {
    let mut sections = Vec::new();

    let elastic_rows = match &out.elastic {
        ElasticOutput::None => None,
        ElasticOutput::Coherent(br) => Some(build_coherent_elastic(out, &br.edges)),
        ElasticOutput::Incoherent { sb_npr } => Some(build_incoherent_elastic(out, *sb_npr)),
    };
    if let Some(rows) = elastic_rows {
        sections.push(Section { key: EndfKey { mat: out.mat, mf: 7, mt: 2 }, rows });
    }

    let inel_rows = build_inelastic(out);
    sections.push(Section { key: EndfKey { mat: out.mat, mf: 7, mt: 4 }, rows: inel_rows });

    Tape::from_sections(" leapr MF=7 thermal scattering tape".to_string(), sections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leapr::coher::{coher, CoherentLattice};
    use crate::thermr::mf7::parse_mf7;

    const MAT: i32 = 37;

    /// A tiny symmetric LEAPR result: 3 alpha x 4 beta, one temperature.
    fn tiny_output(elastic: ElasticOutput) -> LeaprOutput {
        let alpha: Vec<f64> = vec![0.1, 0.5, 2.0];
        let beta: Vec<f64> = vec![0.0, 0.3, 1.0, 2.5];
        let mut ssm = SabMatrix::zeros(beta.len(), alpha.len());
        // fill with a smooth positive law S(a,b) = exp(-a) * exp(-b/3)
        for (ib, &b) in beta.iter().enumerate() {
            for (ia, &a) in alpha.iter().enumerate() {
                ssm.set(ib, ia, (-a).exp() * (-b / 3.0).exp());
            }
        }
        LeaprOutput {
            mat: MAT,
            za: 137.0,
            awr: 0.99917,
            lat: 0,
            isym: 0,
            ilog: false,
            smin: 1e-75,
            alpha,
            beta,
            temperatures_k: vec![296.0],
            dwpix: vec![8.0e-3],
            tempf: vec![430.0],
            ssm: vec![ssm],
            ssp: None,
            npr: 1,
            spr: 20.478,
            elastic,
        }
    }

    #[test]
    fn inelastic_only_roundtrips_through_thermr() {
        let out = tiny_output(ElasticOutput::None);
        let tape = endout(&out);
        let mf7 = parse_mf7(&tape, MAT).unwrap();
        assert!((mf7.awr - out.awr).abs() < 1e-6, "AWR survives");
        let ii = mf7.incoherent_inelastic.expect("MT=4 present");
        assert_eq!(ii.lat, out.lat, "LAT round-trips");
        assert_eq!(ii.lasym, out.isym, "LASYM round-trips");
        assert!(ii.b[0] > 0.0, "B(1) > 0 (npr*spr)");
        assert!((ii.b[2] - out.awr).abs() < 1e-6, "B(3) = A = AWR");
        // grid survives exactly (alpha/beta are written unrounded)
        assert_eq!(ii.beta, out.beta, "beta grid identical");
        assert_eq!(ii.s_tables.len(), out.beta.len(), "one S(alpha) table per beta");
        for (ib, tbl) in ii.s_tables.iter().enumerate() {
            assert_eq!(tbl.alpha, out.alpha, "alpha grid identical for beta {ib}");
            assert!(tbl.s.iter().all(|&s| s >= 0.0), "S >= 0");
            // values match S_sym = ssm*exp(-beta/2) to 7 sig figs
            for (ia, &a) in out.alpha.iter().enumerate() {
                let expect = (-a).exp() * (-out.beta[ib] / 3.0).exp() * (-out.beta[ib] / 2.0).exp();
                let rel = (tbl.s[ia] - expect).abs() / expect.max(1e-30);
                assert!(rel < 1e-6, "S({ia},{ib}) round-trip rel err {rel:e}");
            }
        }
        // effective-temperature TAB1 survives
        assert!(!ii.teff_table.is_empty(), "T_eff table present");
        assert!((ii.teff_table[0].0 - 296.0).abs() < 1e-3, "T_eff table T = 296 K");
        assert!((ii.teff_table[0].1 - 430.0).abs() < 1e-1, "T_eff value ~430 K");
    }

    #[test]
    fn coherent_elastic_roundtrips_and_is_monotone() {
        // real Bragg edges from graphite, wrapped in a LEAPR output
        let br = coher(CoherentLattice::Graphite, 1, 5.0);
        let out = tiny_output(ElasticOutput::Coherent(br.clone()));
        let tape = endout(&out);
        let mf7 = parse_mf7(&tape, MAT).unwrap();
        let ce = mf7.coherent_elastic.expect("MT=2 coherent present");
        assert!((ce.base_temperature_k() - 296.0).abs() < 1e-3, "base temperature 296 K");
        assert!(ce.bragg_energies_ev.len() > 5, "several Bragg edges retained");
        // E ascending; cumulative S(E) non-decreasing (it only steps up).
        assert!(ce.bragg_energies_ev.windows(2).all(|w| w[1] >= w[0]), "E ascending");
        assert!(
            ce.s_tables[0].windows(2).all(|w| w[1] >= w[0] - 1e-9),
            "cumulative S(E) non-decreasing"
        );
        assert!(ce.s_tables[0].iter().all(|&s| s >= 0.0), "S(E) >= 0");
        assert_eq!(ce.temperatures_k.len(), 1, "single temperature -> no extra LISTs");
    }

    #[test]
    fn incoherent_elastic_roundtrips() {
        let mut out = tiny_output(ElasticOutput::Incoherent { sb_npr: 80.4 });
        out.temperatures_k = vec![296.0, 400.0];
        out.dwpix = vec![8.0e-3, 1.1e-2];
        out.tempf = vec![430.0, 520.0];
        // add a second-temperature ssm so MT=4 stays well-formed
        let extra = out.ssm[0].clone();
        out.ssm.push(extra);
        let tape = endout(&out);
        let mf7 = parse_mf7(&tape, MAT).unwrap();
        let ie = mf7.incoherent_elastic.expect("MT=2 incoherent present");
        assert!((ie.sb - 80.4).abs() < 1e-4, "SB = sb_npr");
        assert_eq!(ie.wp_of_t.len(), 2, "two (T, W') points");
        assert!((ie.wp_of_t[0].1 - 8.0e-3).abs() < 1e-7, "W'(296)");
        assert!(ie.wp_of_t.windows(2).all(|w| w[1].1 >= w[0].1), "W'(T) non-decreasing");
    }
}
