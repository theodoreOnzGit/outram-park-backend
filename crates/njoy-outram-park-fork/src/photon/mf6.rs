//! MF=6 photon products (`ZAP = 0`): their contribution to MT=442, as
//! HEATR's `nheat` makes it.
//!
//! For every reaction with MF=6 data, `nheat` calls `sixbar` once per
//! subsection. For a photon subsection it adds `ebar·yld·σ_mt(E)` to MT=442
//! (`heatr.f90:1447-1454`), where:
//!
//! - `yld` is the subsection's yield TAB1 at `E` (`terpa`, `:2973`);
//! - `ebar` is the mean photon energy, computed by `getsix` at each tabulated
//!   incident energy and interpolated **lin-lin** between them (`terp1`,
//!   `:3048`). Upstream reads the TAB2's law and then overwrites it with
//!   `intl=2` (`:2882`), so the tape's own law is not used, here or
//!   there. Above the last incident energy `ebar` is 0 (`:3066-3071`).
//!
//! `getsix` values a LAW=1 photon spectrum in the laboratory (`:3243-3305`,
//! the `irec = 0` branch) as
//!
//! ```text
//!   ebar = Σ_{i ≤ ND} |E'_i|·b0_i                      (discrete lines)
//!        + Σ_panels (E'_i − E'_{i−1})·(E'·b0 terms)/2   (continuum)
//! ```
//!
//! with the continuum panel term `E'_i·b0_{i−1} + E'_{i−1}·b0_{i−1}` for
//! `LEP = 1` (histogram, so the first moment is exact) and
//! `E'_i·b0_i + E'_{i−1}·b0_{i−1}` for `LEP = 2` (trapezoid). The discrete
//! energy is `|E'|`: a primary photon (negative `E'`) is valued at its
//! tabulated magnitude, not shifted by the incident energy. That is what
//! upstream does, so it is what this port does.
//!
//! Other photon laws are not handled here: a photon with `LAW ≠ 1`
//! contributes 0 and is recorded in [`Mf6Photons::unhandled`], and a photon
//! with `AWP > 0` on MT=102 (upstream's `disc102` relativistic branch,
//! `hgam102`) likewise. Neither occurs in the evaluations this was gated on.

use crate::acer::energy::mf6::skip_mf6_subsection;
use crate::endf::interp::{terp1, terpa, IntLaw};
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Tape;

/// One MF=6 photon subsection, reduced to what MT=442 needs.
#[derive(Debug, Clone)]
pub(crate) struct Mf6PhotonSub {
    /// Yield TAB1 vs incident energy.
    yield_tab: Tab1,
    /// `(E_in, ebar)` at each tabulated incident energy \[eV, eV\].
    means: Vec<(f64, f64)>,
}

impl Mf6PhotonSub {
    /// `sixbar`'s `ebar` at incident energy `e`: `terp1` across the panel
    /// holding `e`, 0 above the last tabulated energy.
    fn mean(&self, e: f64) -> f64 {
        let m = &self.means;
        match m.len() {
            0 => 0.0,
            1 => {
                if e <= m[0].0 * (1.0 + 1.0e-10) {
                    m[0].1
                } else {
                    0.0
                }
            }
            n => {
                if e > m[n - 1].0 * (1.0 + 1.0e-10) {
                    return 0.0;
                }
                // Panel: the first k with e < E_k (k >= 1), as `sixbar`
                // slides panels upward; below the first energy it uses the
                // first panel, which `terp1` extrapolates, as upstream.
                let k = m.iter().position(|&(x, _)| e < x * (1.0 - 1.0e-10)).unwrap_or(n - 1).max(1);
                let (x1, y1) = m[k - 1];
                let (x2, y2) = m[k];
                terp1(x1, y1, x2, y2, e, IntLaw::LinLin).unwrap_or(0.0)
            }
        }
    }

    fn production(&self, e: f64) -> f64 {
        terpa(&self.yield_tab.interp, &self.yield_tab.pairs, e).0 * self.mean(e)
    }
}

/// `getsix`'s LAW=1 laboratory mean photon energy for one incident-energy
/// LIST: `nd` discrete lines, rows of `na + 2` words `(E', b0, …)`.
fn law1_mean(data: &[f64], nd: usize, na: usize, nep: usize, lep: i32) -> f64 {
    let ncyc = na + 2;
    let (mut h, mut xl, mut yl, mut el) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 1..=nep {
        let l = ncyc * (i - 1);
        let (Some(&ep), Some(&yy)) = (data.get(l), data.get(l + 1)) else { break };
        let xx = ep.abs();
        let en = yy * xx;
        if i <= nd {
            h += en;
        } else if i == nd + 1 {
            (xl, yl, el) = (xx, yy, en);
        } else {
            let t1 = if lep == 1 { xx * yl + el } else { en + el };
            h += (xx - xl) * t1 / 2.0;
            (xl, yl, el) = (xx, yy, en);
        }
    }
    h
}

/// The MF=6 photon products of one material.
#[derive(Debug, Clone, Default)]
pub(crate) struct Mf6Photons {
    /// `(MT, photon subsections)` for every reaction with a photon product.
    pub(crate) reactions: Vec<(i32, Vec<Mf6PhotonSub>)>,
    /// Every MT with at least one `ZAP = 0` subsection, of any law: HEATR's
    /// `mt6yp`, the list whose MF=12/MF=13 `gheat` skips (`:5148-5160`).
    pub(crate) photon_mts: Vec<i32>,
    /// `(MT, LAW)` of photon subsections this port does not value.
    pub(crate) unhandled: Vec<(i32, i32)>,
}

impl Mf6Photons {
    /// Read every MF=6 section of `mat`. MT=18 is left out when its MF=6
    /// head has `JP ≠ 0`, as `hinit` deletes it (`:794-806`).
    pub(crate) fn from_endf(tape: &Tape, mat: i32) -> Self {
        let mut out = Mf6Photons::default();
        for sec in tape.sections() {
            if sec.key.mat != mat || sec.key.mf != 6 {
                continue;
            }
            let mt = sec.key.mt;
            let mut cur = SectionCursor::new(&sec.rows);
            let Ok(head) = cur.read_cont() else { continue };
            if mt == 18 && head.l1 != 0 {
                continue;
            }
            let mut subs = Vec::new();
            let mut any_photon = false;
            for _ in 0..head.n1 {
                let Ok(y) = cur.read_tab1() else { break };
                let (zap, awp, law) = (y.head.c1, y.head.c2, y.head.l2);
                if zap != 0.0 {
                    if skip_mf6_subsection(&mut cur, law).is_err() {
                        break;
                    }
                    continue;
                }
                any_photon = true;
                if law != 1 || (mt == 102 && awp > 0.0) {
                    out.unhandled.push((mt, law));
                    if skip_mf6_subsection(&mut cur, law).is_err() {
                        break;
                    }
                    continue;
                }
                let Ok(t2) = cur.read_tab2() else { break };
                let lep = t2.head.l2;
                let mut means = Vec::with_capacity(t2.head.n2.max(0) as usize);
                let mut ok = true;
                for _ in 0..t2.head.n2 {
                    let Ok(list) = cur.read_list() else {
                        ok = false;
                        break;
                    };
                    let (nd, na, nep) = (list.head.l1.max(0) as usize, list.head.l2.max(0) as usize, list.head.n2.max(0) as usize);
                    means.push((list.head.c2, law1_mean(&list.data, nd, na, nep, lep)));
                }
                if !ok {
                    break;
                }
                subs.push(Mf6PhotonSub { yield_tab: y, means });
            }
            if any_photon {
                out.photon_mts.push(mt);
            }
            if !subs.is_empty() {
                out.reactions.push((mt, subs));
            }
        }
        out
    }

    /// `Σ_subsections yld·ebar` of reaction `mt` at `e` \[eV per reaction\];
    /// the caller multiplies by `σ_mt(E)`.
    pub(crate) fn energy_per_reaction(subs: &[Mf6PhotonSub], e: f64) -> f64 {
        subs.iter().map(|s| s.production(e)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn law1_mean_discrete_lines_use_the_magnitude() {
        // Two discrete lines, one primary (negative E'), and no continuum.
        let data = [1.0e6, 0.5, -2.0e6, 0.5];
        assert_eq!(law1_mean(&data, 2, 0, 2, 2), 1.5e6);
    }

    #[test]
    fn law1_mean_histogram_and_trapezoid_continua() {
        // Continuum on [1, 3] eV with density 0.5 then 0: histogram LEP=1
        // gives the exact first moment of a flat density, 0.5·(9-1)/2 = 2.
        let data = [1.0, 0.5, 3.0, 0.0];
        assert_eq!(law1_mean(&data, 0, 0, 2, 1), 2.0);
        // LEP=2 trapezoid of E'·f: (3-1)·(3·0 + 1·0.5)/2 = 0.5.
        assert_eq!(law1_mean(&data, 0, 0, 2, 2), 0.5);
    }
}
