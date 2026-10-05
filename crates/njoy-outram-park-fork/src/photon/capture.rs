//! Radiative capture's contribution to MT=442 when its photons are in MF=12:
//! HEATR's **energy balance**, not a sum over the photon lines.
//!
//! `gheat`'s MT=102 branch (`heatr.f90:5276-5300`, `5326-5333`) does not add
//! `y·σ·E_γ` for capture. For each photon subsection `k` it subtracts the
//! recoil the nucleus takes from emitting that photon, and after the last
//! subsection it adds the whole available energy:
//!
//! ```text
//!   MT442_cap(E) = σ(E)·(E + Q − E/(A+1)) − Σ_k y_k(E)·σ(E)·E_R,k(E)
//! ```
//!
//! - `Q` is MF=3/MT=102's `QI` (`c2h`, `:5230`).
//! - A discrete line's recoil is `E_R = E_γ²/(2·tm)`, `tm = m_n c²·(A+1)`
//!   (`disgam`, `:5671-5687`, applied to the tabulated `EG`, `:5280`).
//! - A continuum's recoil is the spectrum average of `E'²/(2·tm)` (`tabsqr`,
//!   `:5610-5669`: four-point Gauss–Legendre per panel, normalised by the same
//!   quadrature of the spectrum), at each MF=15 incident energy, lin-lin
//!   between them (`gambar`, `:5582-5585`), 0 below the first and above
//!   `1.000001×` the last.
//!
//! So capture's MT=442 is the energy left for photons once the recoil is
//! paid, whatever the photon lines add up to. Where an evaluation's lines do
//! not sum to `Q` (Fe-58's fall 208 keV short), the two definitions differ,
//! and this one is NJOY's.
//!
//! The whole reaction contributes only within the energy range where its
//! leading yield TAB1 is non-zero (`elow`/`ehigh`, `:5194-5210`, checked at `:5262`).

use crate::common::phys::{AMASSN_AMU, AMU_G, CLIGHT_CM_S, EV_ERG};
use crate::endf::interp::{terp1, terpa, IntLaw};
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Tape;

/// `tm = m_n c²·(A+1)` \[eV\], HEATR's recoil mass energy (`heatr.f90:324-326`).
pub(crate) fn recoil_mass_energy(awr: f64) -> f64 {
    AMASSN_AMU * AMU_G * CLIGHT_CM_S * CLIGHT_CM_S / EV_ERG * (awr + 1.0)
}

/// The recoil energy model of one capture photon subsection.
#[derive(Debug, Clone)]
pub(crate) enum Recoil {
    /// A discrete line: `EG²/(2·tm)` \[eV\].
    Discrete(f64),
    /// A continuum: `(E_in, <E'²>/(2·tm))` at each MF=15 incident energy.
    Continuum(Vec<(f64, f64)>),
}

impl Recoil {
    fn at(&self, e: f64) -> f64 {
        match self {
            Recoil::Discrete(r) => *r,
            Recoil::Continuum(c) => gambar_interp(c, e),
        }
    }
}

/// `gambar`'s lookup of a per-incident-energy quantity: 0 below the first
/// tabulated energy (`:5588-5592`), lin-lin inside (`:5583`), the last value
/// up to `1.000001×` the last energy and 0 beyond (`:5597-5605`).
fn gambar_interp(c: &[(f64, f64)], e: f64) -> f64 {
    let n = c.len();
    if n == 0 || e < c[0].0 * (1.0 - 1.0e-10) {
        return 0.0;
    }
    if e >= c[n - 1].0 * (1.0 - 1.0e-10) {
        return if e >= 1.000_001 * c[n - 1].0 { 0.0 } else { c[n - 1].1 };
    }
    let k = c.iter().position(|&(x, _)| e < x * (1.0 - 1.0e-10)).unwrap_or(n - 1).max(1);
    let (x1, y1) = c[k - 1];
    let (x2, y2) = c[k];
    terp1(x1, y1, x2, y2, e, IntLaw::LinLin).unwrap_or(0.0)
}

/// `tabsqr`'s spectrum average of `E'²/(2·tm)` for one MF=15 TAB1 `(E', g)`.
fn tabsqr(g: &Tab1, tm: f64) -> f64 {
    const QP: [f64; 4] = [-0.86114, -0.33998, 0.33998, 0.86114];
    const QW: [f64; 4] = [0.34785, 0.65215, 0.65215, 0.34785];
    let rein = 1.0 / (2.0 * tm);
    let (mut num, mut s) = (0.0f64, 0.0f64);
    let p = &g.pairs;
    let mut region = 0usize;
    for i in 1..p.len() {
        while region + 1 < g.interp.len() && i + 1 > g.interp[region].0 as usize {
            region += 1;
        }
        let law = IntLaw::from_code(g.interp.get(region).map_or(2, |r| r.1));
        let ((xl, yl), (xh, yh)) = (p[i - 1], p[i]);
        if xl == xh {
            continue;
        }
        let dx = xh - xl;
        for j in 0..4 {
            let x = xl + (1.0 + QP[j]) * dx / 2.0;
            let y = terp1(xl, yl, xh, yh, x, law).unwrap_or(0.0);
            num += QW[j] * x * x * rein * dx * y;
            s += QW[j] * dx * y;
        }
    }
    if s > 0.0 {
        num / s
    } else {
        0.0
    }
}

/// `(E_in, average recoil)` over MF=15/MT=102's first partial distribution.
fn mf15_recoil_curve(tape: &Tape, mat: i32, tm: f64) -> Option<Vec<(f64, f64)>> {
    let sec = tape.section(mat, 15, 102)?;
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont().ok()?;
    let p = cur.read_tab1().ok()?;
    if p.head.l2 != 1 {
        return None;
    }
    let t2 = cur.read_tab2().ok()?;
    let mut out = Vec::with_capacity(t2.head.n2.max(0) as usize);
    for _ in 0..t2.head.n2 {
        let g = cur.read_tab1().ok()?;
        out.push((g.head.c2, tabsqr(&g, tm)));
    }
    Some(out)
}

/// Capture's MT=442 term, from MF=12/MT=102 LO=1.
#[derive(Debug, Clone)]
pub(crate) struct CaptureBalance {
    /// MF=3/MT=102 `QI` \[eV\].
    q: f64,
    /// `1/(A+1)`.
    aw1fac: f64,
    /// `(yield TAB1, recoil model)` per photon subsection.
    subs: Vec<(Tab1, Recoil)>,
    /// `[elow, ehigh]`, the non-zero range of the leading yield TAB1.
    range: (f64, f64),
}

impl CaptureBalance {
    /// Build from MF=12/MT=102 (`LO = 1`) of `mat`; `None` if absent or not
    /// LO=1, or if MF=3/MT=102 is missing.
    pub(crate) fn from_endf(tape: &Tape, mat: i32, awr: f64) -> Option<Self> {
        let sec = tape.section(mat, 12, 102)?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont().ok()?;
        if head.l1 != 1 {
            return None;
        }
        let q = {
            let s3 = tape.section(mat, 3, 102)?;
            let mut c3 = SectionCursor::new(&s3.rows);
            c3.read_cont().ok()?;
            c3.read_tab1().ok()?.head.c2
        };
        let tm = recoil_mass_energy(awr);
        let nk = head.n1.max(1);
        let lead = if nk > 1 { Some(cur.read_tab1().ok()?) } else { None };
        let mut subs = Vec::with_capacity(nk as usize);
        let mut continuum: Option<Vec<(f64, f64)>> = None;
        for _ in 0..nk {
            let tab = cur.read_tab1().ok()?;
            let eg = tab.head.c1;
            let recoil = if eg != 0.0 {
                Recoil::Discrete(eg * eg / (2.0 * tm))
            } else {
                if continuum.is_none() {
                    continuum = mf15_recoil_curve(tape, mat, tm);
                }
                Recoil::Continuum(continuum.clone().unwrap_or_default())
            };
            subs.push((tab, recoil));
        }
        let range = nonzero_range(lead.as_ref().unwrap_or(&subs.first()?.0));
        Some(CaptureBalance { q, aw1fac: 1.0 / (awr + 1.0), subs, range })
    }

    /// MT=442 from capture at incident energy `e` \[eV·barn\], given the
    /// capture cross section `sigma` \[barn\] there.
    pub(crate) fn eval(&self, e: f64, sigma: f64) -> f64 {
        if sigma == 0.0 || !in_range(self.range, e) {
            return 0.0;
        }
        let recoil: f64 = self
            .subs
            .iter()
            .map(|(tab, r)| terpa(&tab.interp, &tab.pairs, e).0 * r.at(e))
            .sum();
        sigma * (e + self.q - e * self.aw1fac) - sigma * recoil
    }
}

/// `gheat`'s `[elow, ehigh]` for a reaction (`heatr.f90:5194-5210`): from the
/// first point with a positive value to the first zero after the last
/// positive one (or the last point).
pub(crate) fn nonzero_range(t: &Tab1) -> (f64, f64) {
    let p = &t.pairs;
    let lo = p.iter().find(|q| q.1 > 0.0).map_or(f64::INFINITY, |q| q.0);
    let last_pos = p.iter().rposition(|q| q.1 > 0.0);
    let hi = match last_pos {
        Some(i) if i + 1 < p.len() => p[i + 1].0,
        Some(i) => p[i].0,
        None => 0.0,
    };
    (lo, hi)
}

/// Whether `e` is inside `[elow, ehigh]` with `gheat`'s `1e-10` slack
/// (`:5262`).
pub(crate) fn in_range(r: (f64, f64), e: f64) -> bool {
    e >= r.0 * (1.0 - 1.0e-10) && e <= r.1 * (1.0 + 1.0e-10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::records::Cont;

    #[test]
    fn tabsqr_of_a_flat_spectrum_is_the_mean_square_over_2tm() {
        // Flat g on [0, 2]: <E'^2> = 4/3, so the result is (4/3)/(2 tm).
        let g = Tab1 {
            head: Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: 2 },
            interp: vec![(2, 2)],
            pairs: vec![(0.0, 0.5), (2.0, 0.5)],
        };
        let tm = 10.0;
        let r = tabsqr(&g, tm);
        assert!((r - (4.0 / 3.0) / (2.0 * tm)).abs() < 1.0e-5 * r, "{r}");
    }

    #[test]
    fn gambar_lookup_is_zero_outside_and_linear_inside() {
        let c = [(1.0, 10.0), (3.0, 30.0)];
        assert_eq!(gambar_interp(&c, 0.5), 0.0);
        assert_eq!(gambar_interp(&c, 2.0), 20.0);
        assert_eq!(gambar_interp(&c, 3.0), 30.0);
        assert_eq!(gambar_interp(&c, 3.1), 0.0);
    }
}
