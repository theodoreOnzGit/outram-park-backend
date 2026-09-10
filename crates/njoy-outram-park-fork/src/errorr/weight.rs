// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine egnwtf`, l.9809-10021 (`ErrorrWeight::from_iwt`, the w1/w2/w8/w9 tables).
//   - `subroutine egtwtf`, l.10023-10224 (`WeightSampler::get`).
//   - `subroutine egtflx`, l.10226-10286, infinite-dilution branch (`WeightSampler::flux`).
// and from `src/endf.f90`:
//   - `subroutine terpa`, l.1729-1818 (`terpa`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ERRORR's weighting flux — `egnwtf`/`egtwtf`/`egtflx` (`errorr.f90`).
//!
//! ERRORR carries its own copy of GROUPR's weight-function menu (`iwt`
//! 1–12). Besides the weight *value*, `egtwtf` returns `enext`, the next
//! energy at which the panel integrator ([`super::grpav`]) must stop: `1.01
//! e` on the smooth analytic branches, tighter near the fusion peak or a
//! break point, and a per-panel geometric step for tabulated weights. Those
//! stops are what make the group integrals reproducible against the
//! Fortran, so they are ported literally, including the final
//! `sigfig(enext, 7)` rounding.
//!
//! The tabulated tables (`w1`/`w2` EPRI-CELL, `w8` fast reactor, `w9` CLAW)
//! are ERRORR's own (`errorr.f90:9838-9928`) — the CLAW table in
//! particular is **not** the one in `groupr.f90` (102 vs 106 words), so
//! they are embedded here rather than reused from
//! [`crate::groupr::weights`].
//!
//! `egtflx` (`errorr.f90:10226-10286`) reduces to `egtwtf` in this port's
//! scope: `grpav` runs at infinite dilution only (`sigz(1) = 1e10`,
//! `nscr2 = 0`, `errorr.f90:8907-8910`), so no total-cross-section
//! self-shielding factor is applied.

use crate::endf::interp::{terp1, IntLaw};
use crate::groupr::weights::BOLTZMANN_EV_PER_K;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// A weight-function selection (`egnwtf`, `errorr.f90:9809-10021`).
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorrWeight {
    /// `iwt = 1/5/8/9` — a TAB1 record in ENDF flat layout
    /// (`c1,c2,l1,l2,nr,np`, `nr` `(nbt,int)` pairs, `np` `(x,y)` pairs).
    Tabulated(Vec<f64>),
    /// `iwt = 2` — constant.
    Constant,
    /// `iwt = 3` — `1/E`.
    OneOverE,
    /// `iwt = 4` — thermal Maxwellian + `1/E` + fission; `wght(1..6) =
    /// eb, tb, ab, ec, tc, ac` (`errorr.f90:9967-9988`).
    ThermalOneOverEFission {
        /// Thermal break \[eV\].
        eb: f64,
        /// Thermal temperature \[eV\].
        tb: f64,
        /// Thermal amplitude (derived).
        ab: f64,
        /// Fission break \[eV\].
        ec: f64,
        /// Fission temperature \[eV\].
        tc: f64,
        /// Fission amplitude (derived).
        ac: f64,
    },
    /// `iwt = 6/7` — (thermal) -- (1/E) -- (fission + fusion), optionally
    /// with a temperature-dependent thermal part.
    ThermalFissionFusion {
        /// `iwt = 7`.
        t_dependent: bool,
    },
    /// `iwt = 10` — CLAW with a temperature-dependent thermal part below
    /// `0.15 T/300` eV, the CLAW table above.
    ClawTDependent(Vec<f64>),
    /// `iwt = 11/12` — VITAMIN-E (ORNL-5505), optionally T-dependent.
    VitaminE {
        /// `iwt = 12`.
        t_dependent: bool,
    },
}

impl ErrorrWeight {
    /// Build the weight for `iwt` (`egnwtf`).
    ///
    /// `tab1` supplies the read-in TAB1 for `iwt = 1` (card 13); `analytic`
    /// supplies `(eb, tb, ec, tc)` for `iwt = 4` (card 13b). Both are
    /// ignored for the other options.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] for an illegal `iwt`, or a missing `tab1` /
    /// `analytic` when the option needs one.
    pub fn from_iwt(
        iwt: i32,
        tab1: Option<Vec<f64>>,
        analytic: Option<(f64, f64, f64, f64)>,
    ) -> Result<Self, NjoyError> {
        match iwt.abs() {
            1 => tab1
                .map(ErrorrWeight::Tabulated)
                .ok_or_else(|| NjoyError::EndfParse("errorr: iwt=1 needs a TAB1 weight".into())),
            2 => Ok(ErrorrWeight::Constant),
            3 => Ok(ErrorrWeight::OneOverE),
            4 => {
                let (eb, tb, ec, tc) = analytic.ok_or_else(|| {
                    NjoyError::EndfParse("errorr: iwt=4 needs eb,tb,ec,tc".into())
                })?;
                // errorr.f90:9968-9975
                let (ab, ac) = if eb > 50.0 * tb {
                    (1.0, 0.0)
                } else {
                    (
                        1.0 / ((-eb / tb).exp() * eb.powi(2)),
                        1.0 / ((-ec / tc).exp() * ec.powf(1.5)),
                    )
                };
                Ok(ErrorrWeight::ThermalOneOverEFission {
                    eb,
                    tb,
                    ab,
                    ec,
                    tc,
                    ac,
                })
            }
            5 => {
                let mut v = Vec::with_capacity(184);
                v.extend_from_slice(&W1);
                v.extend_from_slice(&W2);
                Ok(ErrorrWeight::Tabulated(v))
            }
            6 => Ok(ErrorrWeight::ThermalFissionFusion { t_dependent: false }),
            7 => Ok(ErrorrWeight::ThermalFissionFusion { t_dependent: true }),
            8 => Ok(ErrorrWeight::Tabulated(W8.to_vec())),
            9 => Ok(ErrorrWeight::Tabulated(W9.to_vec())),
            10 => Ok(ErrorrWeight::ClawTDependent(W9.to_vec())),
            11 => Ok(ErrorrWeight::VitaminE { t_dependent: false }),
            12 => Ok(ErrorrWeight::VitaminE { t_dependent: true }),
            _ => Err(NjoyError::EndfParse(format!(
                "errorr::egnwtf: illegal weight function requested (iwt={iwt})"
            ))),
        }
    }
}

/// Interpolate `y(x)` in a TAB1 packed in `a` (`subroutine terpa`,
/// `endf.f90:1729-1818`).
///
/// `ip`/`ir` are the running estimates of the first data point above `x`
/// and its interpolation range (initialise to `2`/`1`). Returns
/// `(y, xnext, idis)`: `xnext` is the next grid point above `x` (`1e12`
/// past the table), `idis` flags a discontinuity (histogram law, a doubled
/// grid point, or `x` below the first point).
pub fn terpa(
    a: &[f64],
    x: f64,
    ip: &mut usize,
    ir: &mut usize,
) -> Result<(f64, f64, i32), NjoyError> {
    const SHADE: f64 = 1.00001;
    const XBIG: f64 = 1.0e12;
    let av = |i: usize| a[i - 1]; // Fortran 1-based a(i)
    let nr = av(5).round() as usize;
    let np = av(6).round() as usize;
    if *ir > nr {
        *ir = nr;
    }
    if *ip > np {
        *ip = np;
    }
    let mut jr = 5 + 2 * *ir;
    let mut jp = 5 + 2 * nr + 2 * *ip;
    loop {
        // label 110
        if x < av(jp) {
            // label 120
            if x > av(jp - 2) {
                // label 130
                let law = av(jr + 1).round() as u32;
                let y = terp1(
                    av(jp - 2),
                    av(jp - 1),
                    av(jp),
                    av(jp + 1),
                    x,
                    IntLaw::from_code(law),
                )?;
                let xnext = av(jp);
                let mut idis = i32::from(law == 1);
                if *ip != np && av(jp + 2) == xnext {
                    idis = 1;
                }
                return Ok((y, xnext, idis));
            }
            if x == av(jp - 2) {
                // label 140
                let y = av(jp - 1);
                let law = av(jr + 1).round() as u32;
                let xnext = av(jp);
                let mut idis = i32::from(law == 1);
                if *ip != np && av(jp + 2) == xnext {
                    idis = 1;
                }
                return Ok((y, xnext, idis));
            }
            if *ip == 2 {
                // label 170: below the first point
                return Ok((0.0, av(jp - 2), 1));
            }
            // move down
            jp -= 2;
            *ip -= 1;
            if *ir == 1 {
                continue;
            }
            let it = av(jr - 2).round() as usize;
            if *ip > it {
                continue;
            }
            jr -= 2;
            *ir -= 1;
            continue;
        }
        if *ip == np {
            // label 150: last point and above
            if x < SHADE * av(jp) {
                let y = av(jp + 1);
                let xnext = if y > 0.0 {
                    SHADE * SHADE * av(jp)
                } else {
                    XBIG
                };
                return Ok((y, xnext, 0));
            }
            return Ok((0.0, XBIG, 0));
        }
        // move up
        jp += 2;
        *ip += 1;
        let it = av(jr).round() as usize;
        if *ip <= it {
            continue;
        }
        jr += 2;
        *ir += 1;
    }
}

/// Stateful `egtwtf` evaluator — one per group-averaging pass.
///
/// Holds the `save`d `ip`/`ir`/`ipl`/`step` of the Fortran routine
/// (`errorr.f90:10087`), reset by [`WeightSampler::new`] exactly as the
/// `e == 0` initialisation call does (`errorr.f90:10091-10098`).
#[derive(Debug, Clone)]
pub struct WeightSampler {
    weight: ErrorrWeight,
    tempin: f64,
    ip: usize,
    ir: usize,
    ipl: usize,
    step: f64,
    /// `mwtf` — the zero-weight warning has fired.
    pub warned_zero: bool,
}

impl WeightSampler {
    /// Initialise (`egtwtf(e = 0)`): `ip = 2, ir = 1, ipl = 0, step = 1.10`.
    pub fn new(weight: ErrorrWeight, tempin: f64) -> Self {
        WeightSampler {
            weight,
            tempin,
            ip: 2,
            ir: 1,
            ipl: 0,
            step: 1.10,
            warned_zero: false,
        }
    }

    /// The weight at `e` \[eV\] and the next stop: `(wtf, enext, idis)`
    /// (`egtwtf`, `errorr.f90:10100-10223`). `e` must be positive.
    pub fn get(&mut self, e: f64) -> Result<(f64, f64, i32), NjoyError> {
        const S110: f64 = 1.10;
        const S101: f64 = 1.01;
        const S1002: f64 = 1.002;
        const S1005: f64 = 1.005;
        const S10001: f64 = 1.0001;
        const TENTH: f64 = 0.1;
        const EMAX: f64 = 1.0e10;
        // iwt=6/7 constants (errorr.f90:10064-10073)
        const WT6A: f64 = 0.054;
        const WT6B: f64 = 1.57855e-3;
        const WT6C: f64 = 2.1e6;
        const WT6D: f64 = 2.32472e-12;
        const WT6E: f64 = 1.4e6;
        const WT6F: f64 = 2.5e4;
        const WT6G: f64 = 1.407e7;
        const WT6H: f64 = 2.51697e-11;
        const WT6I: f64 = 1.6e6;
        const WT6J: f64 = 3.3e5;
        // iwt=10 (errorr.f90:10074-10076)
        const WT10A: f64 = 0.15;
        const WT10B: f64 = 300.0;
        const WT10C: f64 = 1.15e6;
        // iwt=11/12 (errorr.f90:10041-10058)
        const CON1: f64 = 7.45824e7;
        const CON2: f64 = 1.0;
        const CON3: f64 = 1.44934e-9;
        const CON4: f64 = 3.90797e-2;
        const CON5: f64 = 2.64052e-5;
        const CON6: f64 = 6.76517e-2;
        const EN1: f64 = 0.414;
        const EN2: f64 = 2.12e6;
        const EN3: f64 = 1.0e7;
        const EN4: f64 = 1.252e7;
        const EN5: f64 = 1.568e7;
        const THERM: f64 = 0.0253;
        const THETA: f64 = 1.415e6;
        const FUSION: f64 = 2.5e4;
        const EP: f64 = 1.407e7;
        const VEB: f64 = 5.0e5;
        const EXMIN: f64 = -89.0;
        let _ = S110;

        let mut idis = 0;
        let wtf;
        let mut enext;
        match &self.weight {
            ErrorrWeight::Tabulated(a) => {
                let (w, en, id) = terpa(a, e, &mut self.ip, &mut self.ir)?;
                wtf = w;
                enext = en;
                idis = id;
                if wtf != 0.0 {
                    if self.ip != self.ipl {
                        self.step = S10001 * (enext / e).powf(TENTH);
                        self.ipl = self.ip;
                    }
                    let mut enxt = self.step * e;
                    if enxt > S101 * e {
                        enxt = S101 * e;
                    }
                    if enext > enxt {
                        idis = 0;
                        enext = enxt;
                    }
                }
            }
            ErrorrWeight::Constant => {
                wtf = 1.0;
                enext = S101 * e;
            }
            ErrorrWeight::OneOverE => {
                wtf = 1.0 / e;
                enext = S101 * e;
            }
            ErrorrWeight::ThermalOneOverEFission {
                eb,
                tb,
                ab,
                ec,
                tc,
                ac,
            } => {
                if e <= *eb {
                    wtf = ab * e * (-e / tb).exp();
                    enext = S101 * e;
                    if e < *eb && enext > *eb {
                        enext = *eb;
                    }
                } else if e <= *ec {
                    wtf = 1.0 / e;
                    enext = S101 * e;
                    if e < *ec && enext > *ec {
                        enext = *ec;
                    }
                } else {
                    wtf = ac * e.sqrt() * (-e / tc).exp();
                    enext = S101 * e;
                }
            }
            ErrorrWeight::ThermalFissionFusion { t_dependent } => {
                let tt = if *t_dependent {
                    self.tempin * BOLTZMANN_EV_PER_K
                } else {
                    WT6A
                };
                let bb = 2.0 * tt;
                let cc = if *t_dependent {
                    WT6B * (2.0f64).exp() / bb.powi(2)
                } else {
                    1.0
                };
                if e <= bb {
                    wtf = cc * e * (-e / tt).exp();
                    enext = S101 * e;
                } else if e <= WT6C {
                    wtf = WT6B / e;
                    enext = S101 * e;
                } else {
                    let mut w = WT6D * e.sqrt() * (-e / WT6E).exp();
                    let pow = -((e / WT6F).sqrt() - (WT6G / WT6F).sqrt()).powi(2) / 2.0;
                    if pow > EXMIN {
                        w += WT6H * pow.exp();
                    }
                    wtf = w;
                    enext = S101 * e;
                    if (e - WT6G).abs() <= WT6I {
                        enext = S1005 * e;
                    }
                    if (e - WT6G).abs() <= WT6J {
                        enext = S1002 * e;
                    }
                }
            }
            ErrorrWeight::ClawTDependent(a) => {
                let ea = BOLTZMANN_EV_PER_K * self.tempin;
                let eb = WT10A * self.tempin / WT10B;
                if e < eb {
                    wtf = WT10C * (e / eb.powi(2)) * (-(e - eb) / ea).exp();
                    enext = S101 * e;
                    if enext > eb {
                        enext = eb;
                    }
                } else {
                    let (w, en, id) = terpa(a, e, &mut self.ip, &mut self.ir)?;
                    wtf = w;
                    enext = en;
                    idis = id;
                    if wtf == 0.0 {
                        enext = EMAX;
                    } else {
                        if self.ip != self.ipl {
                            self.step = S10001 * (enext / e).powf(TENTH);
                            self.ipl = self.ip;
                        }
                        let mut enxt = self.step * e;
                        if enxt > S101 * e {
                            enxt = S101 * e;
                        }
                        if enext > enxt {
                            idis = 0;
                            enext = enxt;
                        }
                    }
                }
            }
            ErrorrWeight::VitaminE { t_dependent } => {
                enext = S101 * e;
                if e < EN1 {
                    let tt = if *t_dependent {
                        self.tempin * BOLTZMANN_EV_PER_K
                    } else {
                        THERM
                    };
                    let cc = if *t_dependent {
                        CON2 * (EN1 / tt).exp() / EN1.powi(2)
                    } else {
                        CON1
                    };
                    wtf = cc * e * (-e / tt).exp();
                    if enext > EN1 {
                        enext = EN1;
                    }
                } else if e < EN2 {
                    wtf = CON2 / e;
                    if enext > EN2 {
                        enext = EN2;
                    }
                } else if e < EN3 {
                    wtf = CON3 * e.sqrt() * (-e / THETA).exp();
                    if enext > EN3 {
                        enext = EN3;
                    }
                } else if e < EN4 {
                    wtf = CON4 / e;
                    if enext > EN4 {
                        enext = EN4;
                    }
                } else if e < EN5 {
                    wtf = CON5 * (-5.0 * (e.sqrt() - EP.sqrt()).powi(2) / FUSION).exp();
                    if (e - EP).abs() <= VEB {
                        enext = S1002 * e;
                    }
                    if enext > EN5 {
                        enext = EN5;
                    }
                } else {
                    wtf = CON6 / e;
                }
            }
        }
        if wtf == 0.0 && !self.warned_zero {
            self.warned_zero = true; // upstream: mess('egtwtf', 'xs energy range exceeds weight function range', ...)
        }
        Ok((wtf, sigfig(enext, 7, 0), idis))
    }

    /// `egtflx` at infinite dilution (`errorr.f90:10265-10285`, `nscr2 = 0`):
    /// the flux is the bare weight and the stop is `egtwtf`'s.
    pub fn flux(&mut self, e: f64) -> Result<(f64, f64, i32), NjoyError> {
        self.get(e)
    }
}

// ---------------------------------------------------------------------------
// ERRORR's own weight tables (errorr.f90:9838-9928), verbatim.
// ---------------------------------------------------------------------------

/// `w1` (`errorr.f90:9838-9856`): first half of the EPRI-CELL LWR TAB1.
const W1: [f64; 92] = [
    0.0, 0.0, 0.0, 0.0, 1.0, 88.0, 88.0, 5.0, 1.0e-5, 5.25e-4, 0.009, 0.355, 0.016, 0.552, 0.024,
    0.712, 0.029, 0.785, 0.033, 0.829, 0.043, 0.898, 0.05, 0.918, 0.054, 0.921, 0.059, 0.918, 0.07,
    0.892, 0.09, 0.799, 0.112, 0.686, 0.14, 0.52, 0.17, 0.383, 0.21, 0.252, 0.3, 0.108, 0.4,
    0.0687, 0.49, 0.051, 0.57, 0.0437, 0.6, 0.0413, 1.0, 0.024914, 1.01e3, 3.7829e-5, 2.0e4,
    2.2257e-6, 3.07e4, 1.5571e-6, 6.07e4, 9.1595e-7, 1.2e5, 5.7934e-7, 2.01e5, 4.3645e-7, 2.83e5,
    3.8309e-7, 3.56e5, 3.6926e-7, 3.77e5, 3.4027e-7, 3.99e5, 2.7387e-7, 4.42e5, 1.0075e-7, 4.74e5,
    2.1754e-7, 5.02e5, 2.6333e-7, 5.4e5, 3.0501e-7, 6.5e5, 2.9493e-7, 7.7e5, 2.5005e-7, 9.0e5,
    2.1479e-7, 9.41e5, 1.7861e-7, 1.0e6, 9.1595e-8, 1.05e6, 1.1518e-7,
];

/// `w2` (`errorr.f90:9857-9876`): second half of the EPRI-CELL LWR TAB1.
const W2: [f64; 92] = [
    1.12e6, 1.3648e-7, 1.19e6, 1.5479e-7, 1.21e6, 1.5022e-7, 1.31e6, 6.8696e-8, 1.4e6, 1.2182e-7,
    2.22e6, 5.9033e-8, 2.35e6, 9.1595e-8, 2.63e6, 3.9981e-8, 3.0e6, 3.1142e-8, 4.0e6, 1.7073e-8,
    5.0e6, 9.0679e-9, 6.0e6, 4.7153e-9, 8.0e6, 1.2276e-9, 1.0e7, 3.0953e-10, 1.257e7, 2.4619e-10,
    1.26e7, 3.4731e-10, 1.27e7, 1.0357e-9, 1.28e7, 2.8436e-9, 1.29e7, 7.191e-9, 1.3e7, 1.6776e-8,
    1.31e7, 3.6122e-8, 1.32e7, 7.1864e-8, 1.33e7, 1.3222e-7, 1.34e7, 2.2511e-7, 1.35e7, 3.5512e-7,
    1.36e7, 5.1946e-7, 1.37e7, 7.0478e-7, 1.38e7, 8.8825e-7, 1.39e7, 1.0408e-6, 1.407e7, 1.154e-6,
    1.42e7, 1.087e-6, 1.43e7, 9.5757e-7, 1.44e7, 7.7804e-7, 1.45e7, 6.0403e-7, 1.46e7, 4.3317e-7,
    1.47e7, 2.9041e-7, 1.48e7, 1.8213e-7, 1.49e7, 1.0699e-7, 1.5e7, 5.8832e-8, 1.51e7, 3.0354e-8,
    1.52e7, 1.4687e-8, 1.53e7, 6.6688e-9, 1.54e7, 2.845e-9, 1.55e7, 1.1406e-9, 1.5676e7, 1.978e-10,
    2.0e7, 1.5477e-10,
];

/// `w8` (`errorr.f90:9877-9893`): thermal--1/E--fast reactor--fission+fusion.
const W8: [f64; 66] = [
    0.0,
    0.0,
    0.0,
    0.0,
    1.0,
    29.0,
    29.0,
    5.0,
    0.139e-3,
    0.751516e-3,
    0.1e-1,
    0.497360e-1,
    0.2e-1,
    0.754488e-1,
    0.4e-1,
    0.107756,
    0.6e-1,
    0.110520,
    0.8e-1,
    0.101542,
    0.1,
    0.884511e-1,
    0.614e2,
    0.144057e-3,
    0.788930e2,
    0.217504e-3,
    0.312030e3,
    0.127278e-2,
    0.179560e4,
    0.236546e-2,
    0.804730e4,
    0.114311e-2,
    0.463090e5,
    0.387734e-3,
    0.161630e6,
    0.125319e-3,
    0.639280e6,
    0.207541e-4,
    0.286500e7,
    0.216111e-5,
    0.472370e7,
    0.748998e-6,
    0.1e8,
    0.573163e-7,
    0.1279e8,
    0.940528e-8,
    0.129e8,
    0.973648e-8,
    0.1355e8,
    0.985038e-7,
    0.1375e8,
    0.176388e-6,
    0.1395e8,
    0.239801e-6,
    0.1407e8,
    0.251963e-6,
    0.1419e8,
    0.239298e-6,
    0.1439e8,
    0.176226e-6,
    0.1459e8,
    0.992422e-7,
    0.1555e8,
    0.150737e-8,
    0.2e8,
    0.725e-10,
];

/// `w9` (`errorr.f90:9894-9911`): ERRORR's extended CLAW weight (47 points —
/// **not** GROUPR's 49-point table).
const W9: [f64; 102] = [
    0.0, 0.0, 0.0, 0.0, 1.0, 47.0, 47.0, 5.0, 1.39e-4, 3.019e6, 5.0e-4, 1.07e7, 1.0e-3, 2.098e7,
    5.0e-3, 8.939e7, 1.0e-2, 1.4638e8, 2.5e-2, 2.008e8, 4.0e-2, 1.7635e8, 5.0e-2, 1.478e8, 1.0e-1,
    4.0e7, 1.4e-1, 1.13e7, 1.5e-1, 7.6e6, 4.14e-1, 2.79e6, 1.13, 1.02e6, 3.06, 3.77e5, 8.32,
    1.39e5, 2.26e1, 5.11e4, 6.14e1, 1.88e4, 1.67e2, 6.91e3, 4.54e2, 2.54e3, 1.235e3, 9.35e2,
    3.35e3, 3.45e2, 9.12e3, 1.266e2, 2.48e4, 4.65e1, 6.76e4, 1.71e1, 1.84e5, 6.27, 3.03e5, 3.88,
    5.0e5, 3.6, 8.23e5, 2.87, 1.353e6, 1.75, 1.738e6, 1.13, 2.232e6, 0.73, 2.865e6, 0.4, 3.68e6,
    2.05e-1, 6.07e6, 3.9e-2, 7.79e6, 1.63e-2, 1.0e7, 6.5e-3, 1.2e7, 7.6e-3, 1.3e7, 1.23e-2, 1.35e7,
    2.64e-2, 1.4e7, 1.14e-1, 1.41e7, 1.14e-1, 1.42e7, 1.01e-1, 1.43e7, 6.5e-2, 1.46e7, 1.49e-2,
    1.5e7, 4.0e-3, 1.6e7, 1.54e-3, 1.7e7, 0.85e-3,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// `terpa` on a two-region TAB1: lin-lin inside, zero + `xnext = x1`
    /// with `idis = 1` below the first point, last-point shading above.
    #[test]
    fn terpa_matches_fortran_branches() {
        // c1 c2 l1 l2 nr np | (nbt,int)=(4,2) | (1,10) (2,20) (3,30) (4,40)
        let a = vec![
            0.0, 0.0, 0.0, 0.0, 1.0, 4.0, 4.0, 2.0, 1.0, 10.0, 2.0, 20.0, 3.0, 30.0, 4.0, 40.0,
        ];
        let (mut ip, mut ir) = (2, 1);
        let (y, xn, idis) = terpa(&a, 2.5, &mut ip, &mut ir).unwrap();
        assert_eq!((y, xn, idis), (25.0, 3.0, 0));
        // exact grid point -> label 140
        let (y, xn, _) = terpa(&a, 3.0, &mut ip, &mut ir).unwrap();
        assert_eq!((y, xn), (30.0, 4.0));
        // below first point -> label 170 (move down first)
        let (y, xn, idis) = terpa(&a, 0.5, &mut ip, &mut ir).unwrap();
        assert_eq!((y, xn, idis), (0.0, 1.0, 1));
        // at/above the last point -> label 150/160
        let (y, xn, _) = terpa(&a, 4.0, &mut ip, &mut ir).unwrap();
        assert_eq!(y, 40.0);
        assert!((xn - 1.00001 * 1.00001 * 4.0).abs() < 1e-12);
        let (y, xn, _) = terpa(&a, 5.0, &mut ip, &mut ir).unwrap();
        assert_eq!((y, xn), (0.0, 1.0e12));
    }

    /// The `iwt = 6` shape reproduces `egtwtf`'s three regions with
    /// ERRORR's own constants — note `wt6b = 1.57855e-3` in `errorr.f90`
    /// versus `1.578551e-3` in `groupr.f90`, a genuine upstream
    /// module-to-module difference at the 6th figure — and the stop is
    /// `1.01 e` rounded to 7 figures away from the fusion peak, `1.005 e` /
    /// `1.002 e` near it.
    #[test]
    fn iwt6_matches_errorr_shape_and_stops() {
        let errorr_iwt6 = |e: f64| -> f64 {
            if e <= 0.108 {
                e * (-e / 0.054).exp()
            } else if e <= 2.1e6 {
                1.57855e-3 / e
            } else {
                let pow = -((e / 2.5e4).sqrt() - (1.407e7f64 / 2.5e4).sqrt()).powi(2) / 2.0;
                2.32472e-12 * e.sqrt() * (-e / 1.4e6).exp()
                    + if pow > -89.0 {
                        2.51697e-11 * pow.exp()
                    } else {
                        0.0
                    }
            }
        };
        let mut s = WeightSampler::new(ErrorrWeight::from_iwt(6, None, None).unwrap(), 293.6);
        for &e in &[1e-3, 0.05, 1.0, 1e3, 1e6, 3e6, 1.407e7, 1.9e7] {
            let (w, en, idis) = s.get(e).unwrap();
            let want = errorr_iwt6(e);
            assert!(
                (w - want).abs() <= 1e-12 * want.abs().max(1e-300),
                "e={e}: {w} vs {want}"
            );
            assert_eq!(idis, 0);
            let factor = if (e - 1.407e7).abs() <= 3.3e5 {
                1.002
            } else if (e - 1.407e7).abs() <= 1.6e6 {
                1.005
            } else {
                1.01
            };
            assert_eq!(en, sigfig(factor * e, 7, 0), "stop at e={e}");
        }
    }

    /// Tabulated weights (iwt=8) step geometrically with the `1.0001 *
    /// (enext/e)^0.1` panel step capped at `1.01 e`, and are zero above the
    /// table with the one-shot warning flag set.
    #[test]
    fn tabulated_weight_steps_and_warns() {
        let mut s = WeightSampler::new(ErrorrWeight::from_iwt(8, None, None).unwrap(), 300.0);
        let (w, en, _) = s.get(1.0).unwrap();
        assert!(w > 0.0);
        assert!(en > 1.0 && en <= sigfig(1.01, 7, 0) + 1e-12);
        let (w, _, _) = s.get(3.0e7).unwrap();
        assert_eq!(w, 0.0);
        assert!(s.warned_zero);
    }

    #[test]
    fn illegal_iwt_is_rejected() {
        assert!(ErrorrWeight::from_iwt(13, None, None).is_err());
        assert!(ErrorrWeight::from_iwt(1, None, None).is_err());
        assert!(ErrorrWeight::from_iwt(4, None, None).is_err());
    }
}
