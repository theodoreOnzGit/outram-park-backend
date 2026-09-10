// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Two-body feed function — `getdis` (`groupr.f90:9340-9677`) for neutron
//! elastic and discrete-level scattering described by File 4.
//!
//! At an incident energy `E` the feed function `ff(il, ig)` is the fraction
//! of the reaction's outgoing neutrons that land in final group `ig`,
//! weighted by the Legendre polynomial `P_il` of the **lab** scattering
//! cosine. `getdis` walks the final-group boundaries `egn(iglo+ig)` upward,
//! maps each boundary to a centre-of-mass cosine through two-body
//! kinematics (`:9480`), and Gauss-Legendre integrates the CM angular
//! distribution `sum fle(l) P_l(w) (2l-1)/2` (`getfle` coefficients, see
//! [`crate::groupr::file4`]) between consecutive cosines, projecting onto
//! `P_il(w_lab)` (`:9552-9558`). Quadrature order follows the band width
//! (`nqp = int(npo*2*b)`, `:9491-9498`) with NJOY's own 4/8/12/20-point
//! tables, and the result is rounded to seven decimals (`:9573-9580`).
//!
//! The routine also keeps `panel` informed of the next *critical energy*
//! (`:9603-9677`): the incident energy at which either `E` itself (`ecl`)
//! or the lowest reachable outgoing energy `alpha*E` (`ech`) crosses a group
//! boundary. Those are discontinuities of `ff`, so `panel` ends a
//! sub-panel there and, when the two indices coincide (no boundary inside
//! the down-scatter band), drops to two-point Lobatto (`nq = 0`, `:9675`).
//!
//! # Scope
//! Neutron-in / neutron-out (`izap = 1`, `aprime = 1`) two-body reactions
//! from File 4 in the CM frame: elastic (`MT = 2`) and the discrete levels
//! (`MT = 51-90`, through `q`/`thresh`). Not ported here: the charged-particle
//! Coulomb term (`:9529-9548`), `MT = 251-253` (`mubar` etc.), the `law = 4`
//! sign flip of ENDF-5 lab-frame data, and File-6 two-body data (`mft = 8`).

use crate::groupr::file4::{File4Angular, NLD};
use crate::groupr::kinematics::legndr;
use crate::NjoyError;

/// `emax` (`:9403`).
const EMAX: f64 = 1.0e10;
/// `small` (`:9404`).
const SMALL: f64 = 1.0e-10;
/// `shade` (`:9406`): boundaries are treated as reached this close below.
const SHADE: f64 = 0.999_999_95;

/// Gauss-Legendre 4-point nodes/weights (`qp4`/`qw4`, `:9360-9364`).
const QP4: [f64; 4] = [
    -0.861_136_311_6,
    -0.339_981_043_6,
    0.339_981_043_6,
    0.861_136_311_6,
];
const QW4: [f64; 4] = [
    0.347_854_845_1,
    0.652_145_154_9,
    0.652_145_154_9,
    0.347_854_845_1,
];
/// 8-point (`:9365-9372`). NJOY's last weight is `.1012285363`, not
/// `.1012285362` — kept verbatim.
const QP8: [f64; 8] = [
    -0.960_289_856_5,
    -0.796_666_477_4,
    -0.525_532_409_9,
    -0.183_434_642_5,
    0.183_434_642_5,
    0.525_532_409_9,
    0.796_666_477_4,
    0.960_289_856_5,
];
const QW8: [f64; 8] = [
    0.101_228_536_2,
    0.222_381_034_5,
    0.313_706_645_9,
    0.362_683_783_4,
    0.362_683_783_4,
    0.313_706_645_9,
    0.222_381_034_5,
    0.101_228_536_3,
];
/// 12-point (`:9373-9382`).
const QP12: [f64; 12] = [
    -0.981_560_634_2,
    -0.904_117_256_4,
    -0.769_902_674_2,
    -0.587_317_954_3,
    -0.367_831_499_0,
    -0.125_233_408_5,
    0.125_233_408_5,
    0.367_831_499_0,
    0.587_317_954_3,
    0.769_902_674_2,
    0.904_117_256_4,
    0.981_560_634_2,
];
const QW12: [f64; 12] = [
    0.047_175_336_4,
    0.106_939_326_0,
    0.160_078_328_5,
    0.203_167_426_7,
    0.233_492_536_5,
    0.249_147_045_8,
    0.249_147_045_8,
    0.233_492_536_5,
    0.203_167_426_7,
    0.160_078_328_5,
    0.106_939_326_0,
    0.047_175_336_4,
];
/// 20-point, NJOY's seven-figure table (`:9383-9400`).
const QP20: [f64; 20] = [
    -0.993_128_6,
    -0.963_971_9,
    -0.912_234_4,
    -0.839_117,
    -0.746_331_9,
    -0.636_053_7,
    -0.510_867,
    -0.373_706_1,
    -0.227_785_9,
    -0.076_526_5,
    0.076_526_5,
    0.227_785_9,
    0.373_706_1,
    0.510_867_0,
    0.636_053_7,
    0.746_331_9,
    0.839_117_0,
    0.912_234_4,
    0.963_971_9,
    0.993_128_6,
];
const QW20: [f64; 20] = [
    0.017_614_0,
    0.040_601_4,
    0.062_672_0,
    0.083_276_7,
    0.101_930_1,
    0.118_194_5,
    0.131_688_6,
    0.142_096_1,
    0.149_173_0,
    0.152_753_4,
    0.152_753_4,
    0.149_173_0,
    0.142_096_1,
    0.131_688_6,
    0.118_194_5,
    0.101_930_1,
    0.083_276_7,
    0.062_672_0,
    0.040_601_4,
    0.017_614_0,
];

fn quadrature(nqp: usize) -> (&'static [f64], &'static [f64]) {
    match nqp {
        4 => (&QP4, &QW4),
        8 => (&QP8, &QW8),
        12 => (&QP12, &QW12),
        _ => (&QP20, &QW20),
    }
}

/// `getdis`'s answer at one incident energy.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedAt {
    /// `ff(il, ig)` as `ff[il][ig]`, `ig = 0..ng` counting final groups
    /// upward from `iglo`.
    pub ff: Vec<Vec<f64>>,
    /// `ng` — number of columns filled (some may be all zero).
    pub ng: usize,
    /// `iglo` — 1-based final group of column 0.
    pub iglo: usize,
    /// `nq` — quadrature order hint for `panel` (`nqp0`, or 0 when no group
    /// boundary lies inside the down-scatter band).
    pub nq: usize,
    /// `enext` — next energy at which `ff` may change or is discontinuous.
    pub enext: f64,
    /// `idisc` — `enext` is a discontinuity.
    pub idisc: bool,
}

/// The `getdis` state for one reaction: the File-4 data, the kinematics
/// constants `getsig`/`getflx` set up (`awr`, `q`, `thresh`, `alpha`), the
/// group structure, and the saved critical-energy trackers.
#[derive(Debug, Clone)]
pub struct TwoBodyFeed {
    angular: File4Angular,
    egn: Vec<f64>,
    awr: f64,
    aprime: f64,
    q: f64,
    thresh: f64,
    alpha: f64,
    yld: f64,
    // `save iecl,iech,ecl,ech,ecn,nqp0` (`:9409`).
    iecl: usize,
    iech: usize,
    ecl: f64,
    ech: f64,
    ecn: f64,
    nqp0: usize,
}

impl TwoBodyFeed {
    /// Set up the feed for a neutron two-body reaction.
    ///
    /// - `angular` — the File-4 section (CM frame) for the reaction;
    /// - `egn` — the `ngn + 1` ascending group boundaries \[eV\];
    /// - `awr` — atomic-weight ratio from the MF=3 HEAD (`getsig:6717`);
    /// - `q` — the MF=3 `QI` (`c2h`, `:6748`), 0 for elastic;
    /// - `lrflag` — MF=3 `LR` (`:6749`), sets the multiplicity `yld`
    ///   (`:9418-9421`).
    ///
    /// The state starts as after `getdis(e = 0)` (`:9603-9609`): the
    /// critical-energy trackers reset, ready for ascending calls.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] for fewer than two boundaries, a non-CM
    /// distribution, or `awr <= 0`.
    pub fn new(
        angular: File4Angular,
        egn: &[f64],
        awr: f64,
        q: f64,
        lrflag: i32,
    ) -> Result<Self, NjoyError> {
        if egn.len() < 2 {
            return Err(NjoyError::EndfParse(
                "getdis: group structure needs >= 2 boundaries".into(),
            ));
        }
        if angular.lct != 2 {
            return Err(NjoyError::EndfParse(format!(
                "getdis: File 4 must be in the CM frame (LCT = {}), lab data is not coded",
                angular.lct
            )));
        }
        if awr.is_nan() || awr <= 0.0 {
            return Err(NjoyError::EndfParse(format!(
                "getdis: awr = {awr} must be > 0"
            )));
        }
        let aprime = 1.0;
        // `getsig:6751-6752`.
        let thresh = (awr + 1.0) * (-q) / awr;
        let alpha = (awr - 1.0).powi(2) / (awr + 1.0).powi(2);
        // `:9418-9421`.
        let yld = match lrflag {
            16 | 21 | 24 | 26 | 30 => 2.0,
            17 | 25 | 38 => 3.0,
            37 => 4.0,
            _ => 1.0,
        };
        let mut feed = TwoBodyFeed {
            angular,
            egn: egn.to_vec(),
            awr,
            aprime,
            q,
            thresh,
            alpha,
            yld,
            iecl: 0,
            iech: 0,
            ecl: 0.0,
            ech: 0.0,
            ecn: 0.0,
            nqp0: 0,
        };
        feed.reset();
        Ok(feed)
    }

    /// Number of groups `ngn`.
    pub fn ngn(&self) -> usize {
        self.egn.len() - 1
    }

    /// Group boundaries.
    pub fn egn(&self) -> &[f64] {
        &self.egn
    }

    /// `alpha = ((awr-1)/(awr+1))^2`.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// `getdis(e = 0)` (`:9603-9609` then `:610-680` at `e = 0`): reset the
    /// critical-energy trackers so the next call may be at any energy.
    pub fn reset(&mut self) {
        self.iecl = 0;
        self.iech = 0;
        self.ecl = 0.0;
        self.ech = 0.0;
        self.ecn = 0.0;
        self.nqp0 = 0;
        let mut enext = EMAX;
        let mut idisc = false;
        self.track_critical(0.0, &mut enext, &mut idisc);
    }

    /// `getdis` at `e > 0` for `nl` Legendre orders (`:9412-9601`, then the
    /// critical-point update `:9610-9677`). Calls must be in ascending `e`
    /// (the trackers only move up), as `panel` guarantees.
    ///
    /// # Errors
    /// Propagates [`File4Angular::coefficients_at`] failures; `nl == 0` or
    /// `e <= 0` is an [`NjoyError::EndfParse`].
    pub fn feed(&mut self, e: f64, nl: usize) -> Result<FeedAt, NjoyError> {
        if nl == 0 || nl > NLD {
            return Err(NjoyError::EndfParse(format!(
                "getdis: nl = {nl} must be in 1..={NLD}"
            )));
        }
        if e.is_nan() || e <= 0.0 {
            return Err(NjoyError::EndfParse(format!("getdis: e = {e} must be > 0")));
        }
        let nld = NLD;
        let ngn = self.ngn();
        let ngn1 = ngn + 1;
        let mut enext = EMAX;
        let mut idisc = false;
        let at = self.angular.coefficients_at(e, nld)?;
        if at.enext < enext * (1.0 - SMALL) {
            idisc = at.idis;
            enext = at.enext;
        }
        let fle = &at.fle;
        let awr = self.awr;
        let aprime = self.aprime;
        let awr2 = awr * (awr + 1.0 - aprime) / aprime;
        let ld = nld - 1;
        // `npo = ld + nl + int(log(300/awr))` (`:9426`); Fortran `int` truncates.
        let npo = (ld as i64 + nl as i64 + (300.0 / awr).ln().trunc() as i64).max(0) as usize;
        let mut nqp0 = 4;
        if npo > 8 {
            nqp0 = 8;
        }
        if npo > 12 {
            nqp0 = 12;
        }
        if npo > 16 {
            nqp0 = 20;
        }
        self.nqp0 = nqp0;
        let mut ast = 0.0;
        if e > self.thresh {
            ast = awr2.sqrt() * (1.0 - self.thresh / e).sqrt();
        }
        let mut ng = 0usize;
        let mut ig = 0usize;
        let mut ngnow: usize;
        let mut ignow = 0usize;
        let iglo = self.iech.saturating_sub(1).max(1);
        let wmin = -1.0f64;
        let wmax = 1.0f64;
        let mut whi = wmin;
        let mut ff = vec![vec![0.0f64; ngn1 + 1]; nl];
        let mut flt = vec![0.0f64; nl];
        // Labels 410/425: one final-group band per iteration (`mfd = 6`
        // always returns to 410 after a band, `:9567`).
        loop {
            ng += 1;
            if ng > ngn1 {
                ng = ngn1;
                ig = ignow;
                for f in ff.iter_mut() {
                    f[ng - 1] = 0.0;
                }
                break;
            }
            ngnow = ng;
            for f in ff.iter_mut() {
                f[ng - 1] = 0.0;
            }
            if ast == 0.0 {
                break;
            }
            let wlo = whi;
            ig += 1;
            if iglo + ig > ngn1 {
                ng = ngnow;
                ig -= 1;
                break;
            }
            ignow = ig;
            let ep = self.egn[iglo + ig - 1];
            whi = ((ep / e) * (1.0 + awr).powi(2) / aprime - (1.0 + ast * ast)) / (2.0 * ast);
            if whi < SHADE * wmin {
                whi = wmin;
            }
            if whi > SHADE * wmax {
                whi = wmax;
            }
            if wlo == whi {
                continue;
            }
            // Gauss quadrature between wlo and whi (`:9489-9563`).
            let aa = (whi + wlo) / 2.0;
            let b = (whi - wlo) / 2.0;
            let mut nqp = (npo as f64 * 2.0 * b).trunc() as i64;
            if nqp > npo as i64 {
                nqp = npo as i64;
            }
            let nqp = if nqp <= 8 {
                4
            } else if nqp <= 12 {
                8
            } else if nqp <= 16 {
                12
            } else {
                20
            };
            flt.iter_mut().for_each(|x| *x = 0.0);
            let (qp, qw) = quadrature(nqp);
            for iqp in 0..nqp {
                let wqp = aa + b * qp[iqp];
                let wqw = b * qw[iqp];
                let p = legndr(wqp, ld);
                let mut prob = 0.0;
                for il in 1..=nld {
                    prob += fle[il - 1] * p[il - 1] * ((2 * il - 1) as f64) / 2.0;
                }
                let wlab = (1.0 + ast * wqp) / (1.0 + ast * ast + 2.0 * ast * wqp).sqrt();
                let pl = legndr(wlab, nl - 1);
                // `prob = yld*wqw*prob` (`:9555`); the product is commutative.
                prob *= self.yld * wqw;
                for il in 0..nl {
                    flt[il] += prob * pl[il];
                }
            }
            for il in 0..nl {
                ff[il][ng - 1] += flt[il];
            }
            if whi >= wmax {
                break;
            }
            if iglo + ig > ngn {
                break;
            }
        }
        let _ = ig;
        // Label 465: seven-decimal rounding (`:9573-9580`).
        let fact = 1.0e7f64;
        let nudge = 1.0e-4f64;
        for f in ff.iter_mut() {
            for v in f.iter_mut().take(ng) {
                let iii = (fact * *v + nudge).round();
                *v = iii / fact;
            }
            f.truncate(ng);
        }
        // Labels 610-680.
        self.track_critical(e, &mut enext, &mut idisc);
        let mut nq = self.nqp0;
        if self.iecl == self.iech {
            nq = 0;
        }
        Ok(FeedAt {
            ff,
            ng,
            iglo,
            nq,
            enext,
            idisc,
        })
    }

    /// Labels 610-680 (`:9610-9677`): advance the critical energies past `e`
    /// and fold the next one into `enext`/`idisc`.
    fn track_critical(&mut self, e: f64, enext: &mut f64, idisc: &mut bool) {
        let ngn = self.ngn();
        let awr = self.awr;
        let aprime = self.aprime;
        let awr2 = awr * (awr + 1.0 - aprime) / aprime;
        if e >= SHADE * self.ecn {
            // Label 620: next boundary crossed by the incident energy.
            loop {
                if e < SHADE * self.ecl {
                    break;
                }
                self.ecl = EMAX;
                if self.iecl == ngn + 1 {
                    break;
                }
                self.iecl += 1;
                let eg = self.egn[self.iecl - 1];
                if self.thresh > 0.0 {
                    // Label 630.
                    let ef = (awr + 1.0) * eg / (awr + 1.0 - aprime);
                    let aa = 1.0 + ef / (-self.q);
                    let disc = (awr2 * aa - 1.0) * ef / (-self.q);
                    if disc < 0.0 {
                        continue;
                    }
                    let disc = disc.sqrt();
                    let af = (1.0 - disc) / aa;
                    self.ecl = if af * af == awr2 {
                        EMAX
                    } else {
                        self.thresh / (1.0 - af * af / awr2)
                    };
                } else if aprime > 1.0 {
                    self.ecl = eg * (awr + 1.0).powi(2) / (4.0 * awr);
                } else {
                    self.ecl = eg;
                }
            }
            // Label 640: next boundary crossed by the lowest outgoing energy.
            loop {
                if e < SHADE * self.ech {
                    break;
                }
                self.ech = EMAX;
                if self.iech == ngn + 1 {
                    break;
                }
                self.iech += 1;
                let eg = self.egn[self.iech - 1];
                if self.thresh > 0.0 {
                    // Label 650.
                    let ef = (awr + 1.0) * eg / (awr + 1.0 - aprime);
                    let aa = 1.0 + ef / (-self.q);
                    let disc = (awr2 * aa - 1.0) * ef / (-self.q);
                    if disc < 0.0 {
                        continue;
                    }
                    let disc = disc.sqrt();
                    let af = (1.0 + disc) / aa;
                    self.ech = if af * af == awr2 {
                        EMAX
                    } else {
                        self.thresh / (1.0 - af * af / awr2)
                    };
                } else if aprime > 1.0 {
                    self.ech = EMAX;
                } else if self.alpha < 0.1 {
                    // `test = 1/10; if (alpha.lt.test) go to 640` with ech = emax.
                    continue;
                } else {
                    self.ech = eg / self.alpha;
                }
            }
            // Label 670.
            self.ecn = self.ecl;
            if self.ech < self.ecn * (1.0 - SMALL) {
                self.ecn = self.ech;
            }
        }
        // Label 680.
        if self.ecn < *enext * (1.0 - SMALL) {
            *idisc = true;
            *enext = self.ecn;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::{Section, Tape};
    use crate::endf::EndfKey;

    /// MF=4/MT=2, LTT=1, CM, isotropic (a1 = 0) at 1e-5 eV and 2e7 eV.
    fn isotropic_tape() -> Tape {
        let rows = vec![
            [1001.0, 0.9992, 0.0, 1.0, 0.0, 0.0],
            [0.0, 0.9992, 0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 1.0, 2.0],
            [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1.0e-5, 0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 2.0e7, 0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        Tape::from_sections(
            String::new(),
            vec![Section {
                key: EndfKey {
                    mat: 125,
                    mf: 4,
                    mt: 2,
                },
                rows,
            }],
        )
    }

    /// Methodology: for an isotropic-CM elastic kernel the P0 feed into a
    /// final group is the fraction of `[alpha E, E]` it covers (`E'` is
    /// linear in the CM cosine, `:9480`). Heavy target (`awr = 100`,
    /// `alpha = 0.9606`), `E = 1000 eV`, boundaries 900/950/1000/2000 eV:
    /// the band `[959.6, 999]` lies entirely in group 2, so the P0 feed is
    /// `[0, 1]` over columns starting at `iglo = 1`. Prediction: sum of P0
    /// columns = 1 within 1e-6 (the seven-decimal rounding), next critical
    /// energy 1000 eV flagged as a discontinuity.
    #[test]
    fn isotropic_heavy_elastic_p0_fractions_sum_to_one() {
        let ang = File4Angular::from_tape(&isotropic_tape(), 125, 2, NLD).unwrap();
        let egn = [900.0, 950.0, 1000.0, 2000.0];
        let mut feed = TwoBodyFeed::new(ang, &egn, 100.0, 0.0, 0).unwrap();
        // Walk up so the critical trackers are current at 999 eV.
        let _ = feed.feed(901.0, 2).unwrap();
        let at = feed.feed(999.0, 2).unwrap();
        let sum: f64 = at.ff[0].iter().sum();
        assert!((sum - 1.0).abs() < 1e-6, "P0 sum {sum}");
        // `iglo = iech - 1` uses the tracker state *before* this call's
        // update (`:9443`), so it still points at group 1 here; the band
        // then lands entirely in column 2 = group 2 (950-1000 eV), which is
        // where alpha*999 = 959.6 eV and 999 eV both lie.
        assert_eq!(at.iglo, 1, "{at:?}");
        assert_eq!(at.ng, 2, "{at:?}");
        assert!(
            at.ff[0][0] == 0.0 && (at.ff[0][1] - 1.0).abs() < 1e-6,
            "{at:?}"
        );
        // Next critical energy: E crosses 1000 eV, before alpha*E crosses it.
        assert!((at.enext - 1000.0).abs() < 1e-9 && at.idisc, "{at:?}");
    }

    /// Methodology: `nq` is the full quadrature order only when a boundary
    /// lies inside `[alpha E, E]` (`iecl != iech`, `:9675`). For hydrogen
    /// (`alpha = 0`) every boundary below `E` is inside the band, so `nq`
    /// must be non-zero, and the P0 sum over all groups down to the bottom
    /// is 1. Result (2026-09-10): as predicted.
    #[test]
    fn hydrogen_band_spans_all_lower_groups() {
        let ang = File4Angular::from_tape(&isotropic_tape(), 125, 2, NLD).unwrap();
        let egn = [1.0e-5, 1.0, 10.0, 100.0, 1000.0, 1.0e4];
        let mut feed = TwoBodyFeed::new(ang, &egn, 0.9992, 0.0, 0).unwrap();
        let at = feed.feed(500.0, 1).unwrap();
        assert_eq!(at.iglo, 1);
        assert!(at.nq > 0, "{at:?}");
        let sum: f64 = at.ff[0].iter().sum();
        assert!((sum - 1.0).abs() < 1e-6, "{at:?}");
    }
}
