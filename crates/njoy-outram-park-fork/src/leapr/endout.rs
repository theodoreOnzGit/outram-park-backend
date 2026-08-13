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
use crate::leapr::vintage::PhysicalConstants;
use crate::mixr::mix::sigfig;

/// 0.0253 eV thermal reference used as the `LAT=1` scaling energy (`therm`).
const THERM: f64 = 0.0253;

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
    /// The physical-constant set the run used — specifically the `k_B` that
    /// defines `tev = k_B T` in the `LAT = 1` detailed-balance factor
    /// `0.0253 / (k_B T)` applied to every stored `S` (leapr.f90:3338-3346).
    ///
    /// **Must match the [`crate::leapr::input::LeaprInput::constants`] the
    /// `ssm` arrays were generated with.** Defaults to
    /// [`PhysicalConstants::Codata2018`], the crate constant, so an output
    /// built by hand behaves exactly as it did before this field existed.
    pub constants: PhysicalConstants,
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
    push_cont(
        rows,
        c1,
        c2,
        l1,
        l2,
        interp.len() as i32,
        pairs.len() as i32,
    );
    let iflat: Vec<f64> = interp
        .iter()
        .flat_map(|&(a, b)| [a as f64, b as f64])
        .collect();
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
    let iflat: Vec<f64> = interp
        .iter()
        .flat_map(|&(a, b)| [a as f64, b as f64])
        .collect();
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
            let ssp = out
                .ssp
                .as_ref()
                .expect("odd isym requires ssp")
                .get(nt)
                .unwrap();
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
        let v = if raw >= 1e-9 {
            sigfig(raw, 7, 0)
        } else {
            sigfig(raw, 6, 0)
        };
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
        (out.npr as f64) * out.spr, // B(1) bound-ish xsec factor > 0
        out.beta[nbeta - 1],        // B(2)
        out.awr,                    // B(3) = A
        sigfig(THERM * out.beta[nbeta - 1], 7, 0), // B(4)
        0.0,                        // B(5)
        out.npr as f64,             // B(6)
    ];
    let l1 = if out.ilog { 1 } else { 0 };
    push_list(&mut rows, 0.0, 0.0, l1, 0, 0, &b);

    // TAB2 over beta (3324–3334). nbt doubles for odd isym (+/- beta).
    let nbt = if out.isym == 1 || out.isym == 3 {
        2 * nbeta - 1
    } else {
        nbeta
    };
    push_tab2(&mut rows, 0.0, 0.0, 0, 0, nbt as i32, &[(nbt as i32, 4)]);

    // For each output beta, one S(alpha) TAB1 at temp 0 + a LIST per extra temp.
    for i in 0..nbt {
        for nt in 0..ntempr {
            let sc = if out.lat == 1 {
                THERM / (out.constants.bk_ev_per_k() * out.temperatures_k[nt])
            } else {
                1.0
            };
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
                let svals: Vec<f64> = (0..nalpha).map(|j| stored_s(out, i, j, nt, be)).collect();
                push_list(&mut rows, out.temperatures_k[nt], beta_i, 4, 0, 0, &svals);
            }
        }
    }

    // Trailing effective-temperature TAB1 (3599–3617): (T, T_eff) pairs.
    let teff: Vec<(f64, f64)> = (0..ntempr)
        .map(|i| {
            (
                sigfig(out.temperatures_k[i], 7, 0),
                sigfig(out.tempf[i], 7, 0),
            )
        })
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
        sections.push(Section {
            key: EndfKey {
                mat: out.mat,
                mf: 7,
                mt: 2,
            },
            rows,
        });
    }

    let inel_rows = build_inelastic(out);
    sections.push(Section {
        key: EndfKey {
            mat: out.mat,
            mf: 7,
            mt: 4,
        },
        rows: inel_rows,
    });

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
            constants: PhysicalConstants::default(),
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
        assert_eq!(
            ii.s_tables.len(),
            out.beta.len(),
            "one S(alpha) table per beta"
        );
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
        assert!(
            (ii.teff_table[0].0 - 296.0).abs() < 1e-3,
            "T_eff table T = 296 K"
        );
        assert!(
            (ii.teff_table[0].1 - 430.0).abs() < 1e-1,
            "T_eff value ~430 K"
        );
    }

    #[test]
    fn coherent_elastic_roundtrips_and_is_monotone() {
        // real Bragg edges from graphite, wrapped in a LEAPR output
        let br = coher(CoherentLattice::Graphite, 1, 5.0);
        let out = tiny_output(ElasticOutput::Coherent(br.clone()));
        let tape = endout(&out);
        let mf7 = parse_mf7(&tape, MAT).unwrap();
        let ce = mf7.coherent_elastic.expect("MT=2 coherent present");
        assert!(
            (ce.base_temperature_k() - 296.0).abs() < 1e-3,
            "base temperature 296 K"
        );
        assert!(
            ce.bragg_energies_ev.len() > 5,
            "several Bragg edges retained"
        );
        // E ascending; cumulative S(E) non-decreasing (it only steps up).
        assert!(
            ce.bragg_energies_ev.windows(2).all(|w| w[1] >= w[0]),
            "E ascending"
        );
        assert!(
            ce.s_tables[0].windows(2).all(|w| w[1] >= w[0] - 1e-9),
            "cumulative S(E) non-decreasing"
        );
        assert!(ce.s_tables[0].iter().all(|&s| s >= 0.0), "S(E) >= 0");
        assert_eq!(
            ce.temperatures_k.len(),
            1,
            "single temperature -> no extra LISTs"
        );
    }

    /// The ten temperatures \[K\] the ENDF/B-VIII.0 graphite thermal evaluation
    /// tabulates — i.e. an MF=7/MT=2 section with `LT = 9` extra temperatures
    /// beyond the 296 K base. Used to exercise the multi-temperature branch of
    /// [`build_coherent_elastic`].
    const GRAPHITE_TEMPS_K: [f64; 10] = [
        296.0, 400.0, 500.0, 600.0, 700.0, 800.0, 1000.0, 1200.0, 1600.0, 2000.0,
    ];

    /// [`tiny_output`] widened to the ten [`GRAPHITE_TEMPS_K`] temperatures, so
    /// `endout` takes the `LT > 0` extra-temperature path.
    ///
    /// `dwpix_296` is the Debye-Waller integral `W'(296 K)` \[1/eV\]; the other
    /// temperatures scale it linearly (`W' ∝ T`, the classical high-temperature
    /// limit). That scaling is a **synthetic** stand-in, chosen only so `S(E,T)`
    /// is monotonically suppressed with temperature — it is *not* the graphite
    /// evaluation's real `W'(T)`, and nothing here is validated against one.
    /// `S(alpha,beta)` and `T_eff` are duplicated across temperatures purely to
    /// keep the MT=4 section well-formed; these tests inspect MT=2 only.
    fn ten_temperature_output(elastic: ElasticOutput, dwpix_296: f64) -> LeaprOutput {
        let mut out = tiny_output(elastic);
        out.temperatures_k = GRAPHITE_TEMPS_K.to_vec();
        out.dwpix = GRAPHITE_TEMPS_K
            .iter()
            .map(|&t| dwpix_296 * t / 296.0)
            .collect();
        out.tempf = GRAPHITE_TEMPS_K.iter().map(|&t| t + 134.0).collect();
        let base = out.ssm[0].clone();
        out.ssm = vec![base; GRAPHITE_TEMPS_K.len()];
        out
    }

    /// Recompute, from the Bragg edges and `W'` alone, the base-temperature
    /// `(E, S)` TAB1 pairs `build_coherent_elastic` must write: `S` is the
    /// running Debye-Waller-weighted structure factor `sum exp(-4 W' E_j) f_j`,
    /// rounded to 7 significant figures, and every edge past `jmax` collapses
    /// onto the final slot (last write wins) — so the last pair carries the
    /// **real** tail energy `E_nedge` and the total sum.
    fn expected_base_row(edges: &[(f64, f64)], w: f64, jmax: usize) -> Vec<(f64, f64)> {
        let mut pairs = vec![(0.0_f64, 0.0_f64); jmax];
        let mut sum = 0.0;
        for (j, &(e, f)) in edges.iter().enumerate() {
            let slot = (j + 1).min(jmax) - 1;
            sum += (-4.0 * w * e).exp() * f;
            pairs[slot] = (sigfig(e, 7, 0), sigfig(sum, 7, 0));
        }
        pairs
    }

    /// Recompute one extra-temperature LIST of `S` values, **including the NJOY
    /// tail quirk this port deliberately reproduces** (see
    /// [`build_coherent_elastic`], `endout.rs` "documented NJOY quirk"): once the
    /// running index passes `jmax` the loop re-reads `edges[jmax-1]` instead of
    /// the real tail edges, so an extra-temperature row is accumulated over a
    /// *different energy set* than the base TAB1 above, and the retained edge is
    /// counted `nedge - jmax + 1` times. The extra-temperature path also feeds
    /// the 7-sigfig-rounded energy into the exponential where the base path uses
    /// the raw energy.
    ///
    /// This helper **pins** that behaviour rather than correcting it: it matches
    /// the Fortran, and `coherent_elastic_extra_temperature_tail_collapse_is_pinned`
    /// measures how little it costs. Do not "fix" either side.
    fn expected_extra_row(edges: &[(f64, f64)], w: f64, jmax: usize) -> Vec<f64> {
        let mut svals = vec![0.0_f64; jmax];
        let mut sum = 0.0;
        for j in 0..edges.len() {
            let slot = (j + 1).min(jmax) - 1;
            let (e, f) = edges[slot];
            sum += (-4.0 * w * sigfig(e, 7, 0)).exp() * f;
            svals[slot] = sigfig(sum, 7, 0);
        }
        svals
    }

    /// V&V — the multi-temperature (`LT = 9`) coherent-elastic write/read path.
    ///
    /// # Methodology
    ///
    /// The `LT > 0` extra-temperature LIST branch of [`build_coherent_elastic`]
    /// had no coverage before 2026-08-13 (the sibling
    /// `coherent_elastic_roundtrips_and_is_monotone` pins `LT = 0`, and
    /// `incoherent_elastic_roundtrips` exercises two temperatures on the
    /// unrelated `LTHR = 2` branch). Graphite — the motivating case — is a
    /// ten-temperature `LT = 9` evaluation, so this test drives exactly that
    /// shape: real Bragg edges from `coher(Graphite, 1, 5.0)` wrapped in a
    /// [`ten_temperature_output`] fixture at the ten [`GRAPHITE_TEMPS_K`],
    /// written with [`endout`] and read back with
    /// [`crate::thermr::mf7::parse_mf7`].
    ///
    /// **Reference:** none external — this is a *verification* (self-consistency)
    /// gate, not validation. The oracle for the `S` values is an independent
    /// recomputation from the edge list and `W'` ([`expected_base_row`] /
    /// [`expected_extra_row`]); the oracle for the physics is the Debye-Waller
    /// law, `S(E, T)` falling with `T` at fixed `E`. No claim is made about
    /// agreement with the ENDF/B-VIII.0 graphite tape — the `W'(T)` used here is
    /// synthetic (see [`ten_temperature_output`]).
    ///
    /// **Pass criteria:** all ten temperatures survive in order and exactly; ten
    /// `S` rows, each the length of the shared Bragg grid; every row matches its
    /// recomputation to `< 1e-12` relative; `LI` codes survive with length
    /// `ntemp - 1`; `S(E, T)` is non-increasing in `T` everywhere and strictly
    /// decreasing wherever it is positive; and
    /// [`CoherentElastic::cross_section`](crate::thermr::mf7::CoherentElastic::cross_section)
    /// is finite, non-negative, and zero below the first edge at all ten
    /// temperatures.
    ///
    /// # Results (measured 2026-08-13, this run)
    ///
    /// The `LT = 9` path round-trips. `coher(Graphite, 1, 5.0)` yields **345**
    /// Bragg edges and, at this fixture's `W'(296 K) = 8.0e-3 /eV`, the `1/E`
    /// thinning retains **all 345** (`jmax = nedge`), so the tail-collapse quirk
    /// does not trigger *in this fixture* — it is pinned separately by
    /// `coherent_elastic_extra_temperature_tail_collapse_is_pinned`. Ten `S`
    /// rows of 345 values each come back, `temp_interp = [2; 9]` (lin-lin).
    ///
    /// # This fixture's `W'` is synthetic and roughly 350x too small — corrected 2026-08-13
    ///
    /// **Do not read the "quirk does not trigger" note above as a statement
    /// about real graphite.** It is a statement about this fixture only.
    ///
    /// `W'(296 K) = 8.0e-3 /eV` here is a synthetic `W' ∝ T` stand-in. The
    /// **real** value, recovered from the ENDF/B-VIII.0 tape itself by
    /// `tests/leapr_graphite_deck_parity.rs`, is **`W'(296 K) ≈ 2.86 /eV`** —
    /// about 350x larger. At that magnitude the `1/E` thinning is genuinely
    /// active: the official tape retains **221 of the 345 edges** with **zero**
    /// trailing flat points, so **the tail-collapse path IS exercised by the
    /// real evaluation.**
    ///
    /// The consequence for anyone reading this test as evidence: it verifies
    /// the `LT = 9` *record round trip*, and nothing about whether the real
    /// graphite tape takes the collapsed-tail branch. It does. That branch is
    /// covered by the separate pinning test, and the real-`W'` behaviour is
    /// covered by the parity check against the tape.
    ///
    /// Debye-Waller suppression, `S(E, T)` \[eV·b\] at fixed `E`, 296 K → 2000 K:
    ///
    /// | `E` \[eV\] | 296 K | 800 K | 2000 K | change |
    /// |---|---|---|---|---|
    /// | 1.822326e-3 (edge 1) | 0.01369261 | 0.01369125 | 0.01368801 | −0.034 % |
    /// | 6.236910e-1 (edge 172) | 3.550155 | 3.488917 | 3.348682 | −5.68 % |
    /// | 5.000000 (edge 344, top) | 25.44551 | 22.40989 | 16.95397 | −33.4 % |
    ///
    /// Interpretation: the suppression is strongest at high `E` (the exponent is
    /// `-4 W' E`), exactly as the Debye-Waller factor requires, and is monotone
    /// in `T` at every edge. `S(E_0) = 0` at all ten temperatures — graphite's
    /// first tabulated reflection carries a zero structure factor — which is why
    /// the strict-decrease criterion is applied only where `S > 0`.
    #[test]
    fn coherent_elastic_ten_temperatures_roundtrip() {
        let br = coher(CoherentLattice::Graphite, 1, 5.0);
        let out = ten_temperature_output(ElasticOutput::Coherent(br.clone()), 8.0e-3);
        let tape = endout(&out);
        let mf7 = parse_mf7(&tape, MAT).unwrap();
        let ce = mf7.coherent_elastic.expect("MT=2 coherent present");
        let ntemp = GRAPHITE_TEMPS_K.len();

        // (1) every temperature survives, in order and exactly.
        assert_eq!(ce.temperatures_k.len(), ntemp, "LT=9 -> ten temperatures");
        assert_eq!(
            ce.temperatures_k,
            GRAPHITE_TEMPS_K.to_vec(),
            "temperatures round-trip exactly and in order"
        );
        assert!(
            (ce.base_temperature_k() - 296.0).abs() < 1e-12,
            "base temperature is the first tabulated one"
        );

        // (2) one S row per temperature, all on the shared Bragg-edge grid.
        let npts = ce.bragg_energies_ev.len();
        assert_eq!(ce.s_tables.len(), ntemp, "one S table per temperature");
        for (j, row) in ce.s_tables.iter().enumerate() {
            assert_eq!(
                row.len(),
                npts,
                "S row {j} has one value per Bragg energy ({npts})"
            );
        }
        assert!(
            ce.bragg_energies_ev.windows(2).all(|w| w[1] >= w[0]),
            "E ascending"
        );
        // At this W' the 1/E thinning keeps every edge (345 of 345 on
        // 2026-08-13), so `jmax == nedge` below. The thinned regime is covered
        // by the tail-collapse test; the value oracles handle either case.
        let jmax = npts;
        assert_eq!(
            jmax,
            br.edges.len(),
            "no 1/E thinning at W'(296 K) = 8.0e-3 /eV"
        );

        // (3) every temperature's S values match an independent recomputation —
        // not just the base row.
        let expect_base = expected_base_row(&br.edges, out.dwpix[0], jmax);
        for (i, &(e, s)) in expect_base.iter().enumerate() {
            let rel_e = (ce.bragg_energies_ev[i] - e).abs() / e.abs().max(1e-30);
            assert!(rel_e < 1e-12, "Bragg energy {i}: rel err {rel_e:e}");
            let rel_s = (ce.s_tables[0][i] - s).abs() / s.abs().max(1e-30);
            assert!(rel_s < 1e-12, "S(E_{i}, T_0): rel err {rel_s:e}");
        }
        for t in 1..ntemp {
            let expect = expected_extra_row(&br.edges, out.dwpix[t], jmax);
            for (i, &s) in expect.iter().enumerate() {
                let rel = (ce.s_tables[t][i] - s).abs() / s.abs().max(1e-30);
                assert!(
                    rel < 1e-12,
                    "S(E_{i}, T_{t} = {} K): got {}, expected {s}, rel err {rel:e}",
                    ce.temperatures_k[t],
                    ce.s_tables[t][i]
                );
            }
        }

        // (4) the ENDF LI temperature-interpolation codes survive.
        assert_eq!(
            ce.temp_interp.len(),
            ce.temperatures_k.len() - 1,
            "one LI code per temperature interval"
        );
        assert!(
            ce.temp_interp.iter().all(|&li| li == 2),
            "endout writes LI=2 (lin-lin) on every extra-temperature LIST, got {:?}",
            ce.temp_interp
        );

        // (5a) physics: Debye-Waller suppression — S falls with T at fixed E.
        for i in 0..npts {
            let col: Vec<f64> = ce.s_tables.iter().map(|r| r[i]).collect();
            assert!(
                col.windows(2).all(|w| w[1] <= w[0]),
                "S(E_{i}) must not rise with T: {col:?}"
            );
            if col[0] > 0.0 {
                assert!(
                    col.windows(2).all(|w| w[1] < w[0]),
                    "S(E_{i}) must fall strictly with T where positive: {col:?}"
                );
            }
        }
        // cumulative in E at every temperature, too.
        for (t, row) in ce.s_tables.iter().enumerate() {
            assert!(
                row.windows(2).all(|w| w[1] >= w[0] - 1e-9),
                "cumulative S(E) non-decreasing at T index {t}"
            );
            assert!(row.iter().all(|&s| s >= 0.0), "S >= 0 at T index {t}");
        }

        // (5b) physics: the cross section is usable at every tabulated
        // temperature — finite, non-negative, and zero below the first edge.
        let first_edge = ce.bragg_energies_ev[0];
        let top_edge = *ce.bragg_energies_ev.last().unwrap();
        for (t, &temp_k) in ce.temperatures_k.iter().enumerate() {
            assert_eq!(
                ce.cross_section(0.5 * first_edge, temp_k).unwrap(),
                0.0,
                "sigma = 0 below the first Bragg edge at {temp_k} K"
            );
            for k in 0..40 {
                // 1e-5 .. 10 eV, log-spaced
                let e = 1e-5 * 10_f64.powf(6.0 * (k as f64) / 39.0);
                let xs = ce.cross_section(e, temp_k).unwrap();
                assert!(
                    xs.is_finite() && xs >= 0.0,
                    "sigma({e:e} eV, {temp_k} K) = {xs} must be finite and >= 0"
                );
            }
            // above the top edge sigma*E is the whole cumulative S of that row.
            let e = 1.5 * top_edge;
            let total = *ce.s_tables[t].last().unwrap();
            let rel = (ce.cross_section(e, temp_k).unwrap() * e - total).abs() / total;
            assert!(rel < 1e-12, "sigma*E = S_total at {temp_k} K, rel {rel:e}");
        }
        // and the suppression is visible in the cross section itself.
        let e = 1.5 * top_edge;
        assert!(
            ce.cross_section(e, 2000.0).unwrap() < ce.cross_section(e, 296.0).unwrap(),
            "sigma(2000 K) < sigma(296 K) at fixed E"
        );
    }

    /// V&V — pins the NJOY tail-collapse quirk on the extra-temperature path.
    ///
    /// # Methodology
    ///
    /// When the `1/E` thinning in [`build_coherent_elastic`] drops the tail
    /// (`jmax < nedge`), the base-temperature TAB1 and the extra-temperature
    /// LISTs are built from **different energy sets**: the base path keeps
    /// writing the real edge energies into the collapsed final slot (last write
    /// wins), while the extra-temperature loop re-reads `edges[jmax-1]` for every
    /// tail term. That is a faithful reproduction of the Fortran and is marked
    /// deliberate in the source, so this test **pins it** — it does not correct
    /// it. If this assertion ever fails, the code changed, not the expectation.
    ///
    /// The fixture is [`ten_temperature_output`] with a deliberately exaggerated
    /// `W'(296 K) = 1.0 /eV` (≈125× the value used in the round-trip test above,
    /// and not physical for graphite) — chosen purely because it is what makes
    /// the thinning criterion bite. **Pass criteria:** the grid is thinned; the
    /// final grid energy is the *real* tail edge, not the retained one; every
    /// extra-temperature row matches [`expected_extra_row`] (the quirk model) to
    /// `< 1e-12` relative; and the quirk's cost — the difference against a
    /// hypothetical "honest tail" sum over the real tail edges — stays inside the
    /// `TOL = 0.9e-7` relative tolerance the thinning criterion itself allows.
    ///
    /// # Results (measured 2026-08-13, this run)
    ///
    /// At `W' (296 K) = 1.0 /eV` the thinning keeps **316 of 345** edges. The
    /// final grid point is `E = 5.0 eV` (the real last edge) while the extra
    /// temperatures accumulated their final `S` at `E = 3.940324 eV` (the
    /// retained edge `edges[315]`), counted `345 − 316 + 1 = 30` times. Every
    /// extra-temperature row reproduces the quirk model exactly.
    ///
    /// The quirk's numerical cost at the final slot, `|S_quirk − S_honest| /
    /// S_honest`: **1.68e-8** at 400 K, 1.03e-10 at 500 K, 6.1e-13 at 600 K,
    /// 5.5e-15 at 700 K, and identically **0** at 800 K and above (the tail terms
    /// underflow the 7-significant-figure rounding). Interpretation: the quirk is
    /// real and it is a genuine inconsistency between the two paths, but it is
    /// bounded by the same `0.9e-7` tolerance that authorised dropping those
    /// edges in the first place, and it shrinks as temperature rises. It is not
    /// worth "fixing" against the Fortran, and doing so would break bit-parity
    /// with NJOY output.
    #[test]
    fn coherent_elastic_extra_temperature_tail_collapse_is_pinned() {
        const TOL: f64 = 0.9e-7; // the thinning tolerance in build_coherent_elastic
        let br = coher(CoherentLattice::Graphite, 1, 5.0);
        let out = ten_temperature_output(ElasticOutput::Coherent(br.clone()), 1.0);
        let tape = endout(&out);
        let mf7 = parse_mf7(&tape, MAT).unwrap();
        let ce = mf7.coherent_elastic.expect("MT=2 coherent present");

        let jmax = ce.bragg_energies_ev.len();
        assert!(
            jmax < br.edges.len(),
            "the exaggerated W' must trigger 1/E thinning (got {jmax} of {} edges)",
            br.edges.len()
        );
        assert_eq!(ce.s_tables.len(), GRAPHITE_TEMPS_K.len());

        // The quirk's visible signature: the last grid energy is the real tail
        // edge, but the extra-temperature S values there were accumulated at the
        // retained edge instead.
        let real_tail_e = sigfig(br.edges.last().unwrap().0, 7, 0);
        let retained_e = sigfig(br.edges[jmax - 1].0, 7, 0);
        assert!(
            (ce.bragg_energies_ev[jmax - 1] - real_tail_e).abs() < 1e-9,
            "final grid energy is the real tail edge"
        );
        assert!(
            retained_e < real_tail_e,
            "and it is NOT the retained edge (pinned quirk: {retained_e} < {real_tail_e})"
        );

        // Every extra-temperature row reproduces the quirk model exactly.
        for t in 1..GRAPHITE_TEMPS_K.len() {
            let expect = expected_extra_row(&br.edges, out.dwpix[t], jmax);
            for (i, &s) in expect.iter().enumerate() {
                let rel = (ce.s_tables[t][i] - s).abs() / s.abs().max(1e-30);
                assert!(
                    rel < 1e-12,
                    "quirk model mismatch at (E_{i}, T_{t}): {rel:e}"
                );
            }

            // The cost of the quirk: compare against an "honest tail" sum over
            // the real tail edges. It must stay inside the thinning tolerance
            // that justified dropping them.
            let w = out.dwpix[t];
            let quirk: f64 = expect[jmax - 1];
            let honest = sigfig(
                br.edges.iter().fold(0.0, |acc, &(e, f)| {
                    acc + (-4.0 * w * sigfig(e, 7, 0)).exp() * f
                }),
                7,
                0,
            );
            let rel = (quirk - honest).abs() / honest;
            assert!(
                rel < TOL,
                "collapsed tail at T_{t} = {} K differs from the real tail by {rel:e}, \
                 outside the {TOL:e} thinning tolerance",
                ce.temperatures_k[t]
            );
        }

        // Physics still holds in the thinned regime.
        for i in 0..jmax {
            let col: Vec<f64> = ce.s_tables.iter().map(|r| r[i]).collect();
            assert!(
                col.windows(2).all(|w| w[1] <= w[0]),
                "S(E_{i}) must not rise with T even with a collapsed tail: {col:?}"
            );
        }
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
        assert!(
            ie.wp_of_t.windows(2).all(|w| w[1].1 >= w[0].1),
            "W'(T) non-decreasing"
        );
    }
}
