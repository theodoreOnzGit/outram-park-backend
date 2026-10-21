//! Two-body mean outgoing neutron energy, as HEATR's `disbar` computes it
//! for the energy-balance heating (GitHub #535, phase H6b).
//!
//! `nheat` deposits `E + q0 − Ē'·yld` per reaction (`heatr.f90:1440-1446`).
//! For elastic scattering and the discrete inelastic levels without MF=6 data
//! (`icon = 1`, `:1176-1190`) `Ē'` comes from `disbar` (`:1829-2013`):
//!
//! ```text
//!   Ē'(E) = E·c(E),   c = (1 + 2·b·w̄ + b²)·awp/(A+1)²,   b = r·A,
//!   r = 1 (elastic) or √(1 − E_th/E),   E_th = −Q·(A+1)/A,
//! ```
//!
//! with `w̄` the centre-of-mass mean cosine, the first Legendre coefficient
//! of MF=4 at `E` (`hgtfle`/`hgetco`, here [`File4Angular`] with
//! [`File4Options::HEATR`]), capped at the largest 64-point Gauss node.
//!
//! **`c` is evaluated at nodes, not at every energy.** `disbar` walks the
//! ascending energies it is asked for, places a new node at `1.1×` the last
//! one (`step`), pulled down to the next MF=4 energy and pushed up to the
//! requested one, and interpolates `c` lin-lin between nodes (`:1950-1958`,
//! `:2007-2008`). The same rule makes the MT=445 damage chord the 2026-10-05
//! diagnosis found. Reproducing NJOY means walking the grid in the same
//! order from the same first energy, which [`DisbarWalker::ebar`] does.
//!
//! With `w̄ = 0` this is the isotropic two-body result the kinematic H1/H3
//! models use, except for the node interpolation of `c`, which matters near
//! a level's threshold where `r` varies fastest.
//!
//! A charged-particle level (MT=600-849) has `awp ≠ 1`, for which `disbar`
//! leaves `c = 0` (`:1966-1968`, the jump to label 120): all of `E + q0` is
//! deposited. That case needs no walker.

use crate::endf::interp::{terp1, IntLaw};
use crate::endf::tape::Tape;
use crate::groupr::file4::{File4Angular, File4Options};

/// `disbar`'s node step (`heatr.f90:1890`).
const STEP: f64 = 1.1;
/// `small` (`:1891`).
const SMALL: f64 = 1.0e-10;
/// The largest node of NJOY's 64-point Gauss–Legendre set, `qp(64)`: the cap
/// on `w̄` (`:1972`).
const QP64_MAX: f64 = 9.993_050_42e-1;
/// Coefficients asked of `hgtfle` (`nld = 60`, `:1913`).
const NLD: usize = 60;

/// `disbar`'s state for one reaction, walked over ascending energies.
#[derive(Debug, Clone)]
pub(crate) struct DisbarWalker {
    /// MF=4 coefficients, or `None` for a reaction with no MF=4 (`miss4`,
    /// isotropy assumed, `:1905-1918`).
    file4: Option<File4Angular>,
    awr: f64,
    /// Kinematic threshold `−Q·(A+1)/A` (`:1920`).
    thresh: f64,
    elastic: bool,
    /// `etop`: the next-energy answer for isotropic data (`:4316`).
    etop: f64,
    en: f64,
    cn: f64,
    el: f64,
    cl: f64,
    enext: f64,
}

impl DisbarWalker {
    /// `disbar`'s `e = 0` initialisation for reaction `mt` with Q-value `q`
    /// (`QI`), target mass ratio `awr` and `etop` (`max(2e7, EMAX)`).
    pub(crate) fn new(tape: &Tape, mat: i32, mt: i32, awr: f64, q: f64, etop: f64) -> Self {
        let opts = File4Options { lab_as_cm: (51..=90).contains(&mt) && awr >= 10.0, ..File4Options::HEATR };
        let file4 = tape
            .section(mat, 4, mt)
            .and_then(|_| File4Angular::from_tape_with(tape, mat, mt, NLD, &opts).ok());
        let enext = match &file4 {
            Some(f) if !f.isotropic => f.first_energy(),
            _ => etop,
        };
        DisbarWalker {
            file4,
            awr,
            thresh: ((awr + 1.0) / awr) * (-q),
            elastic: mt == 2,
            etop,
            en: 0.0,
            cn: 0.0,
            el: 0.0,
            cl: 0.0,
            enext,
        }
    }

    /// `w̄` and the next MF=4 energy at node energy `e` (`hgtfle`).
    fn mean_cosine(&self, e: f64) -> (f64, f64) {
        match &self.file4 {
            Some(f) if !f.isotropic => match f.coefficients_at(e, NLD) {
                Ok(la) => (if la.nle > 1 { la.fle[1] } else { 0.0 }, la.enext),
                Err(_) => (0.0, self.etop),
            },
            _ => (0.0, self.etop),
        }
    }

    /// `Ē'` \[eV\] at `ee` \[eV\]. Must be called with non-decreasing `ee`,
    /// starting at the reaction's first grid energy, as `nheat` calls it.
    pub(crate) fn ebar(&mut self, ee: f64) -> f64 {
        if ee > self.en * (1.0 + SMALL) {
            self.el = self.en;
            self.cl = self.cn;
            let mut e = STEP * self.el;
            if self.enext < e * (1.0 - SMALL) {
                e = self.enext;
            }
            if ee > e * (1.0 + SMALL) {
                e = ee;
            }
            let r = if self.elastic {
                1.0
            } else if self.thresh >= e * (1.0 - SMALL) {
                0.0
            } else {
                (1.0 - self.thresh / e).sqrt()
            };
            let (wbar, enext) = self.mean_cosine(e);
            self.enext = enext;
            self.en = e;
            let afact = 1.0 / ((self.awr + 1.0) * (self.awr + 1.0));
            // `b = r*sqrt(awr/arat)`, `arat = awp/(awr+1-awp) = 1/awr`.
            let b = r * self.awr;
            let wbar = wbar.min(QP64_MAX);
            self.cn = (1.0 + 2.0 * b * wbar + b * b) * afact;
        }
        let ce = terp1(self.el, self.cl, self.en, self.cn, ee, IntLaw::LinLin).unwrap_or(self.cn);
        ee * ce
    }
}

/// `nheat`'s neutron yield for a discrete level with breakup flag `LR`
/// (`heatr.f90:1184-1186`).
pub(crate) fn level_yield(lr: i32) -> f64 {
    match lr {
        16 | 21 | 24 | 26 | 30 => 2.0,
        17 | 25 | 38 => 3.0,
        37 => 4.0,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walker(awr: f64, q: f64, elastic: bool) -> DisbarWalker {
        DisbarWalker {
            file4: None,
            awr,
            thresh: ((awr + 1.0) / awr) * (-q),
            elastic,
            etop: 2.0e7,
            en: 0.0,
            cn: 0.0,
            el: 0.0,
            cl: 0.0,
            enext: 2.0e7,
        }
    }

    #[test]
    fn isotropic_elastic_is_the_two_body_mean() {
        let a = 56.0;
        let mut w = walker(a, 0.0, true);
        for &e in &[1.0, 10.0, 1.0e3, 1.0e6] {
            let want = e * (1.0 + a * a) / ((a + 1.0) * (a + 1.0));
            let got = w.ebar(e);
            assert!((got - want).abs() <= 1.0e-12 * want, "{e}: {got} vs {want}");
        }
    }

    #[test]
    fn a_level_is_interpolated_between_nodes() {
        // At a node the isotropic level formula holds exactly; between nodes
        // `c = Ē'/E` is the lin-lin chord of its node values (`:2007-2008`).
        // `c` is linear in 1/E, not in E, so near threshold the chord and the
        // curve differ by percent: here 0.8 % at the interval's midpoint.
        let (a, q) = (56.0, -8.0e5);
        let thresh = -q * (a + 1.0) / a;
        let mut w = walker(a, q, false);
        let c = |e: f64| (1.0 + a * a * (1.0 - thresh / e)) / ((a + 1.0) * (a + 1.0));
        let e0 = 1.0e6;
        assert!((w.ebar(e0) - e0 * c(e0)).abs() <= 1.0e-12 * e0 * c(e0));
        let (mid, e1) = (1.05e6, 1.1e6); // next node at STEP × 1e6
        let chord = c(e0) + (c(e1) - c(e0)) * (mid - e0) / (e1 - e0);
        let got = w.ebar(mid);
        assert!((got - mid * chord).abs() <= 1.0e-12 * mid * chord, "{got} vs {}", mid * chord);
        let off = (got - mid * c(mid)).abs() / (mid * c(mid));
        assert!(off > 1.0e-3, "the chord should not be the curve: {off:.3e}");
    }

    #[test]
    fn breakup_yields_follow_nheat() {
        assert_eq!(level_yield(0), 1.0);
        assert_eq!(level_yield(16), 2.0);
        assert_eq!(level_yield(17), 3.0);
        assert_eq!(level_yield(37), 4.0);
    }
}
