// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine rpxsamm`, l.3252-3732 — resolved resonances with the SAMMY
//     method (`LRF=7`, `LCOMP=1/2`): the parameter covariance readers, the
//     group-wise sensitivities from the SAMM derivatives, and the `crr` fold.
//   - `subroutine errorr`, l.796-808 — `mmtres` from `s2sammy` with the
//     `103..107 -> 600..800` remap.
//   - `samm.f90` `rdsammy`, l.1151-1176 and `orders`, l.1368-1412 — the
//     resonance node list `enode`.
//   - `endf.f90` `intgio`, l.721-836 — the INTG record formats.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `rpxsamm` — the MF=32 branch for an `LRF=7` (R-matrix limited) range.
//!
//! Unlike the ERRORJ branch ([`super::resolved`]), which differences the
//! MLBW cross sections numerically, this one takes the analytic
//! resonance-parameter derivatives from [`crate::samm::derivs`]
//! (`Want_Partial_Derivs`) and integrates them over each user group with
//! the `egtwtf` weight on an adaptive panel grid seeded by the resonance
//! nodes (`enode`: every `E_λ` and `E_λ ± Γ/2` inside the range). The
//! group sensitivities are then rescaled to the group cross sections
//! `covout` will divide by (`cflx·csig/sigs`) and folded with the
//! parameter covariance into `crr(ig, ig2, mt, mt1)`, which
//! [`super::ResonanceCovariance::rescon`] adds to *every* output block
//! (the SAMMY branch of `rescon`, `errorr.f90:8528, 8800-8806`).
//!
//! Upstream idioms kept because the oracle carries them: the panel
//! search's `enext` is `egtwtf`'s next stop from the *previous* panel,
//! bounded by `(1+eps)·ee` and the next node; the midpoint convergence
//! test uses `eps = 0.01` relative plus `epm = 1e-7` absolute on every
//! `sigp` slot including the always-zero fission slot; a range's
//! contribution above `enode(nodes)` (the shaded `eh`) is zeroed; the
//! total's sensitivity comes only through `akxy` (a directly-evaluated
//! `MT=1` therefore gets none); `MT=18` in `mmtres(3..)` selects the empty
//! slot `2+i` rather than the fission slot (`k2` is assigned twice).
//!
//! **Validated** on ENDF/B-VII.1 Cl-35 (`LCOMP=2`, 1088 parameters, 3093
//! INTG lines) against NJOY2016 — `tests/errorr_mf32_cl35_rml_golden.rs`.
//! The `LCOMP=1` reader (`errorr.f90:3389-3417`) is ported alongside but
//! has no oracle tape.

use crate::endf::records::SectionCursor;
use crate::mixr::mix::sigfig;
use crate::samm::mf2::RmlSection;
use crate::samm::setup::{setup_with_derivs, SammSetup};
use crate::samm::xsformula::cssammy::cssammy_with_derivs;
use crate::NjoyError;

use super::super::covout::CoarseGroupXs;
use super::super::gridd::DerivedCoefficients;
use super::super::weight::{ErrorrWeight, WeightSampler};
use super::mf2::{Mf2Range, Mf2Resonances};
use super::{RangeParams, ResonanceCovariance};

/// What `rpxsamm` needs from the MF=33 side of the run.
pub struct SammyContext<'a> {
    /// `mts(1:nmt)`.
    pub mts: &'a [i32],
    /// `akxy` / `ek`.
    pub derived: &'a DerivedCoefficients,
    /// `cflx` / `csig`.
    pub coarse: &'a CoarseGroupXs,
}

/// `mmtres(1:nmtres)` for a material whose MF=2 carries an `LRF=7` range
/// (`s2sammy`, `samm.f90:504-512`, then `errorr.f90:801-806`): `2`,
/// `102`, then every further particle pair's `MT` with `103..107` mapped
/// to `600/650/700/750/800`. `None` when no range is `LRF=7` (`nmtres = 0`).
///
/// # Errors
/// `NotPorted` for `NIS > 1` with an `LRF=7` range (upstream: "multiple
/// isotopes do not work with sammy method").
pub fn mmtres_of(mf2: &Mf2Resonances) -> Result<Option<Vec<i32>>, NjoyError> {
    let Some(r) = mf2.ranges.iter().find(|r| r.rml.is_some()) else {
        return Ok(None);
    };
    if mf2.nis > 1 {
        return Err(NjoyError::NotPorted(
            "errorr::s2sammy: multiple isotopes do not work with sammy method",
        ));
    }
    let section = r.rml.as_ref().unwrap();
    let mut mm: Vec<i32> = section.particle_pairs.iter().map(|p| p.mt).collect();
    if mm.len() >= 2 {
        mm[0] = 2;
        mm[1] = 102;
    }
    for m in mm.iter_mut() {
        *m = match *m {
            103 => 600,
            104 => 650,
            105 => 700,
            106 => 750,
            107 => 800,
            x => x,
        };
    }
    Ok(Some(mm))
}

/// `enode` — the range bounds (shaded inward) plus every resonance's
/// centre and half-height energies inside `(el, eh)` (`rdsammy`,
/// `samm.f90:1151-1176`), sorted with near-duplicates removed (`orders`).
fn resonance_nodes(section: &RmlSection, el: f64, eh: f64) -> Vec<f64> {
    let mut nodes = vec![sigfig(el, 7, 1), sigfig(eh, 7, -1)];
    for g in &section.spin_groups {
        for r in &g.resonances {
            // hw = res(jj+1)/2 + sum |res(jj+1+j)|/2: the first raw width is
            // not abs'd upstream; with the eliminated channel first (every
            // committed LRF=7 evaluation) that is Gamma_gamma.
            let mut hw = r.gamma_gamma / 2.0;
            for w in &r.channel_widths {
                hw += w.abs() / 2.0;
            }
            let mut ndig = 5i32;
            if r.energy > 0.0 {
                let q = r.energy / (hw / 10.0);
                ndig = if q.is_finite() && q > 0.0 {
                    2 + q.log10().round() as i32
                } else {
                    9
                };
            }
            ndig = ndig.clamp(5, 9);
            let e = r.energy;
            if e > el && e < eh {
                nodes.push(sigfig(e, ndig, 0));
            }
            if e + hw > el && e + hw < eh {
                nodes.push(sigfig(e + hw, ndig, 0));
            }
            if e - hw > el && e - hw < eh {
                nodes.push(sigfig(e - hw, ndig, 0));
            }
        }
    }
    orders(&mut nodes);
    nodes
}

/// `orders` (`samm.f90:1368-1412`): ascending, dropping an element within
/// `1e-10` (relative) of its predecessor.
fn orders(x: &mut Vec<f64>) {
    if x.len() <= 2 {
        return;
    }
    x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut out: Vec<f64> = Vec::with_capacity(x.len());
    for &v in x.iter() {
        if let Some(&last) = out.last() {
            if (v - last).abs() <= 1.0e-10 * v {
                continue;
            }
        }
        out.push(v);
    }
    *x = out;
}

/// One INTG line `(ii, jj, kij...)` in `intgio`'s formats
/// (`endf.f90:748-761`): `2i5,1x` then `18i3` / `13i4` / `11i5` / `9i6` /
/// `8i7` for `NDIGIT = 2..6`.
fn intg_line(line: &str, ndigit: i32) -> Result<(i64, i64, Vec<i64>), NjoyError> {
    let (w, n) = match ndigit {
        2 => (3usize, 18usize),
        3 => (4, 13),
        4 => (5, 11),
        5 => (6, 9),
        6 => (7, 8),
        _ => {
            return Err(NjoyError::EndfParse(format!(
                "errorr::rpxsamm: invalid INTG ndigit={ndigit}"
            )))
        }
    };
    let field = |s: &str, a: usize, b: usize| -> Result<i64, NjoyError> {
        let sub = if a >= s.len() {
            ""
        } else {
            &s[a..b.min(s.len())]
        };
        let t = sub.trim();
        if t.is_empty() {
            Ok(0)
        } else {
            t.parse::<i64>().map_err(|_| {
                NjoyError::EndfParse(format!("errorr::rpxsamm: bad INTG field {sub:?}"))
            })
        }
    };
    let ii = field(line, 0, 5)?;
    let jj = field(line, 5, 10)?;
    let mut k = Vec::with_capacity(n);
    for m in 0..n {
        let a = 11 + m * w;
        k.push(field(line, a, a + w)?);
    }
    Ok((ii, jj, k))
}

/// The `LRF=7` parameter covariance, `cov(nresp × nresp)` row-major
/// (entries beyond what the file supplies stay zero, as upstream's
/// zero-initialised allocation).
fn read_covariance(
    cur: &mut SectionCursor,
    raw: Option<&[String]>,
    rp: &RangeParams,
    nresp: usize,
) -> Result<Vec<f64>, NjoyError> {
    let mut cov = vec![0.0f64; nresp * nresp];
    match rp.lcomp {
        // errorr.f90:3389-3417 -- lrf=7 and lcomp=1: full covariance LIST
        1 => {
            let _srs = cur.read_cont()?; // nsrs, nlrs
            let _njsx = cur.read_cont()?;
            for _ in 0..rp.nls {
                let _ = cur.read_list()?;
            }
            let l = cur.read_list()?;
            let nparb = l.head.n2.max(0) as usize;
            if nparb > nresp {
                return Err(NjoyError::EndfParse(format!(
                    "errorr::rpxsamm: MF=32 has {nparb} parameters, MF=2 {nresp}"
                )));
            }
            let mut l2 = 0usize;
            for i1 in 0..nparb {
                for i2 in i1..nparb {
                    let v = l.data.get(l2).copied().unwrap_or(0.0);
                    l2 += 1;
                    cov[i1 * nresp + i2] = v;
                    if i1 != i2 {
                        cov[i2 * nresp + i1] = v;
                    }
                }
            }
        }
        // errorr.f90:3420-3475 -- lrf=7 and lcomp=2: uncertainties + INTG correlations
        2 => {
            let _pp = cur.read_list()?;
            let mut sdev: Vec<f64> = Vec::new();
            for _ in 0..rp.nls {
                let ch = cur.read_list()?;
                let nch = ch.head.n2.max(0) as usize;
                let res = cur.read_list()?;
                let nx = res.head.n2.max(0) as usize;
                for i1 in 0..nx {
                    for i2 in 0..=nch {
                        sdev.push(res.data.get(i1 * 12 + 6 + i2).copied().unwrap_or(0.0));
                    }
                }
            }
            let npar = sdev.len();
            if npar > nresp {
                return Err(NjoyError::EndfParse(format!(
                    "errorr::rpxsamm: MF=32 has {npar} parameters, MF=2 {nresp}"
                )));
            }
            let c = cur.read_cont()?;
            let mut nx = c.l1;
            if nx == 0 {
                nx = 2;
            }
            let nm = c.n1.max(0) as usize;
            for i in 0..npar {
                cov[i * nresp + i] = 1.0;
            }
            if nm > 0 {
                let raw = raw.ok_or_else(|| {
                    NjoyError::EndfParse(
                        "errorr::rpxsamm: INTG records need the tape's raw MF=32 text".into(),
                    )
                })?;
                let fact = 2.0 * 10f64.powi(nx);
                let start = cur.position();
                cur.skip_rows(nm)?;
                for line in &raw[start..start + nm] {
                    let (nn1, nn2, kij) = intg_line(line, nx)?;
                    let mut nn2p = nn2 - 1;
                    for &kv in &kij {
                        nn2p += 1;
                        if nn2p >= nn1 {
                            break;
                        }
                        let (a, b) = ((nn2p - 1) as usize, (nn1 - 1) as usize);
                        if a >= nresp || b >= nresp {
                            return Err(NjoyError::EndfParse(format!(
                                "errorr::rpxsamm: INTG index ({nn2p},{nn1}) beyond {nresp} parameters"
                            )));
                        }
                        let v = if kv > 0 {
                            (2.0 * kv as f64 + 1.0) / fact
                        } else if kv < 0 {
                            -((-2.0 * kv as f64 + 1.0) / fact)
                        } else {
                            continue;
                        };
                        cov[a * nresp + b] = v;
                        cov[b * nresp + a] = v;
                    }
                }
            }
            for i in 0..npar {
                for j in 0..npar {
                    cov[i * nresp + j] *= sdev[i] * sdev[j];
                }
            }
        }
        other => {
            return Err(NjoyError::NotPorted(if other == 0 {
                "errorr::rpxsamm: lrf=7 with lcomp=0 has no upstream coding"
            } else {
                "errorr::rpxsamm: illegal lcomp"
            }))
        }
    }
    Ok(cov)
}

/// `sigp(1:nmtres+2)` and `sigx(nresp, nmtres)` at one energy
/// (`cssammy`'s slot mapping, `samm.f90:102-121`, `152-164`).
struct SammyPoint {
    sigp: Vec<f64>,
    /// `sigx[ipar * nmtres + slot]`.
    sigx: Vec<f64>,
}

struct Evaluator<'a> {
    section: &'a RmlSection,
    st: &'a SammSetup,
    ds: &'a crate::samm::derivs::DerivSetup,
    mmtres: &'a [i32],
    calls: std::cell::Cell<usize>,
}

impl Evaluator<'_> {
    fn eval(&self, e: f64) -> SammyPoint {
        self.calls.set(self.calls.get() + 1);
        let (r, sigd) = cssammy_with_derivs(
            self.section,
            &self.st.kinematics,
            &self.st.amplitudes,
            &self.st.quantum_info,
            self.ds,
            e,
        );
        let nmtres = self.mmtres.len();
        let mut sigp = vec![0.0f64; nmtres + 2];
        sigp[0] = r.total;
        sigp[1] = r.elastic;
        sigp[2] = r.fission;
        sigp[3] = r.capture;
        // slots 2+i (1-based) for pairs i >= 3; a fission pair fills slot 3
        // instead (samm.f90:113-119)
        let mut other = r.other.iter();
        for i in 3..=nmtres {
            let mt = self.section.particle_pairs[i - 1].mt;
            if mt != 18 {
                if let Some((_, v)) = other.next() {
                    sigp[2 + i - 1] = *v;
                }
            }
        }
        SammyPoint { sigp, sigx: sigd }
    }

    fn zero(&self, p: &mut SammyPoint) {
        for v in p.sigp.iter_mut() {
            *v = 0.0;
        }
        for v in p.sigx.iter_mut() {
            *v = 0.0;
        }
    }
}

/// `rpxsamm` (`errorr.f90:3252-3732`) for one `LRF=7` range. Adds the
/// range's `crr` to `rc` (upstream allocates `crr` per range; a second
/// `LRF=7` range would have aborted there, the port accumulates).
///
/// # Errors
/// `NotPorted` for an MF=2 range that is not `LRF=7` in a material that
/// has one (upstream routes every resolved range through `rpxsamm` once
/// `nmtres > 0`), for `LCOMP=0`, or for a `KRM≠3`/`IFG≠0` section
/// ([`crate::samm::mf2`]'s own limits); `EndfParse` for malformed
/// covariance records.
#[allow(clippy::needless_range_loop, clippy::too_many_arguments)]
pub fn rpxsamm(
    cur: &mut SectionCursor,
    raw: Option<&[String]>,
    rp: &RangeParams,
    mf2: &Mf2Resonances,
    mf2r: &Mf2Range,
    mmtres: &[i32],
    egn: &[f64],
    weight: &ErrorrWeight,
    tempin: f64,
    ctx: &SammyContext,
    rc: &mut ResonanceCovariance,
) -> Result<(), NjoyError> {
    const EPS: f64 = 0.01;
    const EPM: f64 = 1.0e-7;

    let Some(rml) = &mf2r.rml else {
        return Err(NjoyError::NotPorted(
            "errorr::rpxsamm: only an lrf=7 MF=2 range is handled by the SAMM path (lrf=3 with nmtres>0 not ported)",
        ));
    };
    if rp.lrf != 7 {
        return Err(NjoyError::NotPorted(
            "errorr::rpxsamm: MF=32 lrf != 7 in a material routed through the SAMM path",
        ));
    }
    let el = mf2r.range.c1;
    let eh = mf2r.range.c2;
    let enode = resonance_nodes(rml, el, eh);
    let nodes = enode.len();
    let enode_top = enode[nodes - 1];

    let mut section = rml.clone();
    let (st, ds) = setup_with_derivs(&mut section, mf2.awr)?;
    let nresp = ds.npar.max(1);
    let nmtres = mmtres.len();

    let cov = read_covariance(cur, raw, rp, nresp)?;

    // --- group-wise sensitivities (errorr.f90:3524-3688)
    let ngn = egn.len() - 1;
    let nmt = ctx.mts.len();
    let ev = Evaluator {
        section: &section,
        st: &st,
        ds: &ds,
        mmtres,
        calls: std::cell::Cell::new(0),
    };
    let mut panels = 0usize;
    let mut sens = vec![vec![vec![0.0f64; nresp]; ngn]; nmt];
    let mut sigs = vec![vec![0.0f64; nmt]; ngn];
    let mut sflx = vec![0.0f64; ngn];
    let mut sampler = WeightSampler::new(weight.clone(), tempin);
    // k2 per reaction: which sigp slot (1-based) / mmtres index feeds it
    let slot_of: Vec<usize> = ctx
        .mts
        .iter()
        .map(|&mt| {
            let mut k2 = 0usize;
            if mt == 2 {
                k2 = 2;
            }
            if mt == 18 {
                k2 = 3;
            }
            if mt == 102 {
                k2 = 4;
            }
            for i in 3..=nmtres {
                if mt == mmtres[i - 1] {
                    k2 = i + 2;
                }
            }
            k2
        })
        .collect();
    let mmt_of: Vec<usize> = ctx
        .mts
        .iter()
        .map(|&mt| {
            let mut k2 = 0usize;
            for i in 1..=nmtres {
                if mt == mmtres[i - 1] {
                    k2 = i;
                }
            }
            k2
        })
        .collect();
    let ek = &ctx.derived.ek;
    let nek = ctx.derived.nek();

    for i1 in 0..ngn {
        let mut frac = 0.0f64;
        let elo = egn[i1];
        let ehi = egn[i1 + 1];
        let mut e = elo;
        let mut pl = ev.eval(e);
        if e > enode_top {
            ev.zero(&mut pl);
        }
        let (mut wtl, _, _) = sampler.get(e)?;
        let mut ee = elo;
        let mut eel = ee;
        let mut enext = ehi;
        loop {
            // label 100
            if ee > enode_top {
                enext = ehi;
            } else if ee == enode_top {
                enext = sigfig(enode_top, 7, 1);
            } else {
                if (1.0 + EPS) * ee < enext {
                    enext = (1.0 + EPS) * ee;
                }
                let mut i2 = 1usize;
                while enode[i2 - 1] <= ee && i2 < nodes {
                    i2 += 1;
                }
                if i2 <= nodes && enode[i2 - 1] < enext {
                    enext = enode[i2 - 1];
                }
            }
            ee = enext;
            let (p, wt);
            loop {
                // label 110
                let mut pp = ev.eval(ee);
                if ee > enode_top {
                    ev.zero(&mut pp);
                }
                let (w, en, _) = sampler.get(ee)?;
                enext = en;
                e = (ee + eel) / 2.0;
                let mut pn = ev.eval(e);
                if e > enode_top {
                    ev.zero(&mut pn);
                }
                if eel < e {
                    let mut redo = false;
                    for n1 in 0..nmtres + 2 {
                        if ee >= enode_top {
                            break;
                        }
                        if (pn.sigp[n1] - (pp.sigp[n1] + pl.sigp[n1]) / 2.0).abs()
                            > EPS * pn.sigp[n1] + EPM
                        {
                            ee = e;
                            redo = true;
                            break;
                        }
                    }
                    if redo {
                        continue;
                    }
                } else {
                    rc.messages.push(format!(
                        "rpxsamm: convergence issue for e={eel:.3e} — check reconstructed xs for steep increase"
                    ));
                }
                p = pp;
                wt = w;
                break;
            }
            // derivation range k (1-based; 0 = none)
            let mut k = 0usize;
            for i in 1..=nek {
                if ee >= ek[i - 1] && ee < ek[i] {
                    k = i;
                }
            }
            let de = ee - eel;
            if k > 0 {
                for n1 in 0..nmt {
                    for n2 in 0..nmt {
                        let k2 = slot_of[n2];
                        if k2 != 0 {
                            let a = ctx.derived.get(n2, n1, k - 1);
                            if a != 0.0 {
                                sigs[i1][n1] += a * de
                                    * (p.sigp[k2 - 1] * wt + pl.sigp[k2 - 1] * wtl)
                                    / 2.0;
                            }
                        }
                    }
                }
                for n1 in 0..nmt {
                    for n2 in 0..nmt {
                        let k2 = mmt_of[n2];
                        if k2 != 0 {
                            let a = ctx.derived.get(n2, n1, k - 1);
                            if a != 0.0 {
                                let s = &mut sens[n1][i1];
                                for n3 in 0..nresp {
                                    let x = p.sigx[n3 * nmtres + k2 - 1];
                                    let xl = pl.sigx[n3 * nmtres + k2 - 1];
                                    s[n3] += a * de * (x * wt + xl * wtl) / 2.0;
                                }
                            }
                        }
                    }
                }
            } else {
                rc.messages.push(format!(
                    "rpxsamm: panel at {ee:.6e} eV outside the covariance grid — no akxy applied"
                ));
            }
            panels += 1;
            sflx[i1] += de * (wt + wtl) / 2.0;
            if p.sigp[0] != 0.0 {
                frac += de * (wt + wtl) / 2.0;
            }
            pl = p;
            eel = ee;
            wtl = wt;
            if ee >= ehi {
                break;
            }
        }
        // adjust to groupr cross sections (errorr.f90:3675-3684)
        frac /= sflx[i1];
        for n1 in 0..nmt {
            let mut rat = 1.0;
            if sigs[i1][n1] != 0.0 {
                rat = ctx.coarse.cflx[i1] * ctx.coarse.csig[n1][i1] / sigs[i1][n1];
            }
            for v in sens[n1][i1].iter_mut() {
                *v *= rat * frac;
            }
        }
    }

    rc.messages.push(format!(
        "rpxsamm: {} panels, {} cssammy evaluations, {} parameters, {} nodes",
        panels,
        ev.calls.get(),
        nresp,
        nodes
    ));

    // --- fold sensitivities with covariances (errorr.f90:3697-3722)
    // a[n1][ig][j] = sum_i cov(i,j) sens(n1,ig,i) over the non-zero cov
    // entries, then crr = sum_j a sens(n2,ig2,j).
    let nonzero: Vec<(usize, usize, f64)> = (0..nresp)
        .flat_map(|i| (0..nresp).map(move |j| (i, j)))
        .filter_map(|(i, j)| {
            let v = cov[i * nresp + j];
            (v != 0.0).then_some((i, j, v))
        })
        .collect();
    let mut a = vec![vec![vec![0.0f64; nresp]; ngn]; nmt];
    for n1 in 0..nmt {
        for ig in 0..ngn {
            let s = &sens[n1][ig];
            let row = &mut a[n1][ig];
            for &(i, j, v) in &nonzero {
                row[j] += v * s[i];
            }
        }
    }
    if rc.crr.is_empty() {
        rc.crr = vec![0.0f64; ngn * ngn * nmt * nmt];
        rc.nmt = nmt;
    }
    rc.sammy = true;
    for ig in 0..ngn {
        for ig2 in 0..ngn {
            for n1 in 0..nmt {
                for n2 in 0..nmt {
                    let mut t = 0.0f64;
                    let ar = &a[n1][ig];
                    let sr = &sens[n2][ig2];
                    for j in 0..nresp {
                        t += ar[j] * sr[j];
                    }
                    rc.crr[((ig * ngn + ig2) * nmt + n1) * nmt + n2] += t;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `intgio`'s `NDIGIT = 2` layout: `2i5,1x,18i3`. A blank field is 0.
    #[test]
    fn intg_line_ndigit2() {
        let line = "   12    1  1 -3  0 12          ";
        let (ii, jj, k) = intg_line(line, 2).unwrap();
        assert_eq!((ii, jj), (12, 1));
        assert_eq!(&k[..5], &[1, -3, 0, 12, 0]);
        assert_eq!(k.len(), 18);
    }

    /// `orders`: sorted, near-duplicates (1e-10 relative) dropped.
    #[test]
    fn orders_sorts_and_dedups() {
        let mut v = vec![5.0, 1.0, 3.0, 3.0 * (1.0 + 1e-12), 2.0];
        orders(&mut v);
        assert_eq!(v, vec![1.0, 2.0, 3.0, 5.0]);
    }

    /// The `mmtres` remap `103..107 -> 600..800` (`errorr.f90:801-806`).
    #[test]
    fn mmtres_remap() {
        use crate::endf::records::Cont;
        use crate::samm::mf2::ParticlePair;
        let pp = |mt| ParticlePair {
            mass_a: 1.0,
            mass_b: 1.0,
            z_a: 0,
            z_b: 0,
            spin_a: 0.0,
            spin_b: 0.0,
            q_value: 0.0,
            penetrability_flag: 0,
            shift_flag: 0,
            mt,
            parity_a: 0.0,
            parity_b: 0.0,
        };
        let mf2 = Mf2Resonances {
            awr: 1.0,
            nis: 1,
            ranges: vec![Mf2Range {
                range: Cont::from_fields(&[0.0, 1.0, 1.0, 7.0, 0.0, 0.0]),
                lfw: 0,
                nls: 0,
                spi_cont: None,
                lists: Vec::new(),
                rml: Some(RmlSection {
                    particle_pairs: vec![pp(102), pp(2), pp(103), pp(107)],
                    spin_groups: Vec::new(),
                }),
            }],
            amur: Vec::new(),
        };
        assert_eq!(mmtres_of(&mf2).unwrap(), Some(vec![2, 102, 600, 800]));
    }
}
