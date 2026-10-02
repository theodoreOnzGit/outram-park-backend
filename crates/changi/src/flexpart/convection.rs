// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/convect43c.f90 (subroutines CONVECT and TLIFT)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
//                    (convect43c.f90: Kerry Emanuel, version 4.3c, 20 May 2002;
//                    FLEXPART integration by C. Forster, 2003-2004)
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Emanuel's moist-convection scheme, version 4.3c (Emanuel 1991,
//! *J. Atmos. Sci.* 48, 2313–2335; Emanuel & Živković-Rothman 1999,
//! *J. Atmos. Sci.* 56, 1766–1782), as FLEXPART v10.4 carries it in
//! `convect43c.f90`: the buoyancy-sorting scheme that diagnoses, for one grid
//! column, the cloud-base mass flux, the entrainment/detrainment mass fluxes
//! between every pair of levels, and from them the **mass displacement matrix
//! `FMASS`** and the **compensating subsidence `SUB`** that FLEXPART's
//! `calcmatrix`/`redist` turn into particle redistribution probabilities.
//!
//! # What is ported
//!
//! [`convect`] is `SUBROUTINE CONVECT` and [`tlift`] is `SUBROUTINE TLIFT`,
//! translated line for line in `f64`, keeping Fortran's left-to-right operation
//! order (so a `real(8)` build of upstream and this port agree to the last bit
//! or two). Fortran's 1-based arrays are kept 1-based internally (index 0
//! unused) so every subscript can be read against the upstream line.
//!
//! The commented-out dry-adiabatic adjustment (`IPBL /= 0`, upstream lines
//! 314–389) is dead code upstream (`IPBL = 0` is a parameter) and is not
//! ported. The potential temperature `TH(I)` that upstream computes at lines
//! 303–307 is read only by that dead block, so it is not computed here; it
//! cannot affect any output.
//!
//! # Inputs and outputs
//!
//! Upstream reads the column from `conv_mod` (`TCONV`, `QCONV`, `QSCONV`,
//! `PCONV_HPA`, `PHCONV_HPA`) and writes `FT`, `FQ`, `FMASS`, `SUB`,
//! `NCONVTOP` back into it. Here the column is the [`ConvectSounding`]
//! argument and the results come back in [`ConvectOutput`].
//!
//! Units (FLEXPART's, bare `f64`): temperature K, specific humidity kg/kg,
//! pressure **hPa** (the scheme's own convention), `delt` s, cloud-base mass
//! flux kg m⁻² s⁻¹, tendencies K/s and (kg/kg)/s, `FMASS`/`SUB` kg m⁻² s⁻¹,
//! precipitation mm/day, `WD` m/s, `TPRIME` K, `QPRIME` kg/kg.
//!
//! # Upstream quirks, reproduced and documented
//!
//! 1. **`NCONVTOP` is not assigned on any early return.** The five early
//!    `RETURN`s (lines 444, 456, 470, 489, 617) leave `NCONVTOP` holding
//!    whatever the previous call left in `conv_mod`. The port returns
//!    `nconvtop: None` there rather than inventing a value. FLEXPART's only
//!    caller, `calcmatrix`, never reads it on those paths (it requires
//!    `IFLAG` ∈ {1, 4} *and* a non-zero mass flux), which the code-to-code test
//!    confirms against the stale value upstream actually printed.
//! 2. **`CBMF` is in/out state.** It must be "remembered by the calling
//!    program" (upstream's own comment); FLEXPART keeps it per grid column in
//!    `cbaseflux`. The `IFLAG = 0` return at line 489 (parcel stable at cloud
//!    base, no convection last step) returns `CBMF` unmodified (it is `0`).
//!    The return at line 617 (`CBMF` and `CBMFOLD` both `0` after the update)
//!    leaves `IFLAG = 1` with no mass fluxes computed.
//! 3. **`IF(IFLAG.NE.4)IFLAG=1`** (line 494) is always taken, because `IFLAG`
//!    was set to `0` at line 312 and nothing sets it to `4` before then.
//! 4. **`SIJ(I,I)=1.0` inside the `J` loop** (line 648): at `J = I` the
//!    just-computed `ANUM/DEI` is overwritten before use; the port does the
//!    same assignment in the same place.
//! 5. **`TVAPLCL`** (line 595) extrapolates the *environmental* virtual
//!    temperature to the LCL using the *parcel* gradient
//!    `TVP(ICB)-TVP(ICB+1)`. Kept as written.
//! 6. **Geopotential** (line 404) divides the layer thickness by the
//!    half-level pressure `PHCONV_HPA(I)`, not the mid-level one. Kept.
//! 7. **The CBMF relaxation does not depend on the time step**: Forster's
//!    change sets `DELT0 = DELT/3` and `DAMPS = DAMP*DELT/DELT0`, i.e.
//!    `DAMPS ≈ 0.3` for any `DELT` (exactly what the floating-point division
//!    gives, which the port reproduces rather than writing `0.3`).
//! 8. **Gotos.** `GOTO 405` (skip the precipitating downdraft when
//!    `EP(INB) < 1e-4`), `GOTO 360` (no downdraft mass flux at level 1) and
//!    `GOTO 400` (no `QP` update at `INB`) are reproduced as structured
//!    control flow with identical semantics.
//! 9. **Local arrays are not stale.** The `NA×NA` locals (`MENT`, `QENT`,
//!    `ELIJ`, `SIJ`; 76 KB at `NA = 138`) exceed gfortran's documented default
//!    `-fmax-stack-var-size` (64 KiB), so they would live in static memory and
//!    persist between calls (not inspected in the binary). Every element the
//!    routine *reads* is re-initialised earlier in the same call (checked index
//!    by index during the port: all reads stay within `1..NL+1`, which lines
//!    290–302 and 533–549 initialise). Evidence: the reference driver runs all
//!    73 calls in one process, carrying whatever upstream leaves behind, while
//!    the port starts every call fresh, and the two agree bit for bit. No
//!    stale numerical state reaches any output except `NCONVTOP` (quirk 1).
//! 10. **Two guards are unreachable or dead.** `FRAC = MIN(FRAC, 1.0)`
//!     (line 576) can never bind: `FRAC = -(CAPEM+BYP)/MAX(-BYP, 0.001)` with
//!     `CAPEM >= 0` is at most 1. `TG = MAX(TG, 35.0)` in `TLIFT` needs a
//!     lifted parcel colder than 35 K. Both are ported as written.
//!
//! # Verification
//!
//! Code-to-code against upstream compiled from source at both precisions
//! (`dev/flexpart_reference_convection.f90`,
//! `tests/flexpart_convection_code_to_code.rs`), 2026-10-02: on 73 soundings
//! straddling every branch above, **bit-exact against the real(8) build on
//! all 15 220 outputs** (`IFLAG`, `CBMF`, `PRECIP`, `WD`, `TPRIME`,
//! `QPRIME`, `NCONVTOP`, `FT`, `FQ`, `SUB`, `FMASS`). Against the shipped
//! real(4) build the residual (up to 2.7e-3 in `FMASS`) is upstream's own
//! single precision, measured on identical inputs; see the test's doc.
//! Mutation-tested: 22 mutations of this file, 17 killed; the 5 survivors
//! are `MIN(FRAC,1)` and the 35 K floor (quirk 10), the `|DEI| < 0.01`
//! guard (needs a J/kg-scale coincidence), dropping `GOTO 405` (on the sweep
//! the skipped downdraft computes exact zeros, since `EP` grows with height),
//! and `DAMP*DELT/DELT0 -> DAMP*3` (`1-DAMPS` is the same double either
//! way).
//!
//! # Not established
//!
//! Verification only: the port computes what upstream computes. Nothing here
//! says the scheme represents real convection, and per the crate's scope limit
//! nothing here supports operational or emergency use.

/// `ELCRIT`: autoconversion threshold water content, g/g.
const ELCRIT: f64 = 0.0011;
/// `TLCRIT`: temperature (°C) below which the autoconversion threshold is zero.
const TLCRIT: f64 = -55.0;
/// `ENTP`: coefficient of mixing in the entrainment formulation.
const ENTP: f64 = 1.5;
/// `SIGD`: fractional area covered by the unsaturated downdraft.
const SIGD: f64 = 0.05;
/// `SIGS`: fraction of precipitation falling outside the cloud.
const SIGS: f64 = 0.12;
/// `OMTRAIN`: assumed fall speed of rain (P/s).
const OMTRAIN: f64 = 50.0;
/// `OMTSNOW`: assumed fall speed of snow (P/s).
const OMTSNOW: f64 = 5.5;
/// `COEFFR`: rain evaporation coefficient.
const COEFFR: f64 = 1.0;
/// `COEFFS`: snow evaporation coefficient.
const COEFFS: f64 = 0.8;
/// `BETA`: downdraft velocity-scale coefficient.
const BETA: f64 = 10.0;
/// `DTMAX`: maximum negative parcel temperature perturbation below the LFC, K.
const DTMAX: f64 = 0.9;
/// `ALPHA`: quasi-equilibrium approach rate (FLEXPART: `0.025`, "original 0.2").
const ALPHA: f64 = 0.025;
/// `DAMP`: quasi-equilibrium damping.
const DAMP: f64 = 0.1;

/// `CPD`, J kg⁻¹ K⁻¹ (the scheme's own constants, also in `TLIFT`).
const CPD: f64 = 1005.7;
/// `CPV`, J kg⁻¹ K⁻¹.
const CPV: f64 = 1870.0;
/// `CL`, J kg⁻¹ K⁻¹.
const CL: f64 = 2500.0;
/// `RV`, J kg⁻¹ K⁻¹.
const RV: f64 = 461.5;
/// `RD`, J kg⁻¹ K⁻¹.
const RD: f64 = 287.04;
/// `LV0`, J/kg.
const LV0: f64 = 2.501e6;
/// `G`, m s⁻².
const G: f64 = 9.81;
/// `ROWL`, kg m⁻³.
const ROWL: f64 = 1000.0;
/// `CPVMCL = CL - CPV`.
const CPVMCL: f64 = CL - CPV;
/// `EPS0 = RD/RV`.
const EPS0: f64 = RD / RV;
/// `EPSI = 1/EPS0`.
const EPSI: f64 = 1.0 / EPS0;
/// `GINV = 1/G`.
const GINV: f64 = 1.0 / G;
/// `EPSILON`: smallest mass flux counted when diagnosing `NCONVTOP`.
const EPSILON: f64 = 1.0e-20;
/// `MINORIG`: lowest level from which convection may originate.
const MINORIG: usize = 1;

/// One grid column as `convect` reads it from `conv_mod`. Every vector is
/// 0-based in Rust and holds Fortran level `k` at index `k-1`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvectSounding {
    /// `TCONV`: temperature, K, levels `1..=NL+1`.
    pub t: Vec<f64>,
    /// `QCONV`: specific humidity, kg/kg, levels `1..=NL+1`.
    pub q: Vec<f64>,
    /// `QSCONV`: saturation specific humidity, kg/kg, levels `1..=NL+1`.
    pub qs: Vec<f64>,
    /// `PCONV_HPA`: mid-level pressure, hPa, levels `1..=NL+1`.
    pub p_hpa: Vec<f64>,
    /// `PHCONV_HPA`: half-level pressure, hPa; `PHCONV_HPA(k)` is the bottom
    /// interface of level `k`. Levels `1..=NL+1`.
    pub ph_hpa: Vec<f64>,
}

/// `IFLAG` as upstream defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvectFlag {
    /// `0`: no moist convection (stable, parcel too cold or dry, or stable at
    /// cloud base with no convection at the previous step).
    NoConvection,
    /// `1`: moist convection occurs.
    Convection,
    /// `2`: LCL above 200 hPa (or the LCL formula gives ≥ 2000 hPa).
    LclTooHigh,
    /// `3`: cloud base at or above level `NL-1`.
    CloudBaseTooHigh,
    /// `4`: convection occurs but the subsidence CFL condition is violated.
    CflViolated,
}

impl ConvectFlag {
    /// Upstream's integer code.
    #[must_use]
    pub fn code(self) -> i32 {
        match self {
            ConvectFlag::NoConvection => 0,
            ConvectFlag::Convection => 1,
            ConvectFlag::LclTooHigh => 2,
            ConvectFlag::CloudBaseTooHigh => 3,
            ConvectFlag::CflViolated => 4,
        }
    }
}

/// What `convect` returns or writes into `conv_mod`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvectOutput {
    /// `IFLAG`.
    pub iflag: ConvectFlag,
    /// `CBMF` on return: the updated cloud-base mass flux, kg m⁻² s⁻¹.
    pub cbmf: f64,
    /// `PRECIP`, mm/day.
    pub precip: f64,
    /// `WD`, downdraft velocity scale, m/s.
    pub wd: f64,
    /// `TPRIME`, K.
    pub tprime: f64,
    /// `QPRIME`, kg/kg.
    pub qprime: f64,
    /// `FT(1..=NL+1)`, K/s, at index `k-1`.
    pub ft: Vec<f64>,
    /// `FQ(1..=NL+1)`, (kg/kg)/s.
    pub fq: Vec<f64>,
    /// `SUB(1..=NL+1)`, kg m⁻² s⁻¹, positive downwards.
    pub sub: Vec<f64>,
    /// `FMASS(1..=NL+1, 1..=NL+1)`, kg m⁻² s⁻¹, row-major: `FMASS(i,j)` at
    /// `(i-1)*(NL+1) + (j-1)`. Use [`ConvectOutput::fmass`].
    pub fmass: Vec<f64>,
    /// `NL + 1`, the side of `fmass`.
    pub n: usize,
    /// `NCONVTOP`; `None` where upstream returns early without assigning it
    /// (quirk 1 of the module doc).
    pub nconvtop: Option<usize>,
}

impl ConvectOutput {
    /// `FMASS(i, j)`, 1-based like upstream.
    #[must_use]
    pub fn fmass(&self, i: usize, j: usize) -> f64 {
        self.fmass[(i - 1) * self.n + (j - 1)]
    }
}

/// 1-based square matrix, `NA × NA` locals of upstream.
struct Mat {
    n: usize,
    v: Vec<f64>,
}

impl Mat {
    fn new(n: usize) -> Self {
        Mat {
            n,
            v: vec![0.0; (n + 1) * (n + 1)],
        }
    }
    fn get(&self, i: usize, j: usize) -> f64 {
        self.v[i * (self.n + 1) + j]
    }
    fn set(&mut self, i: usize, j: usize, x: f64) {
        self.v[i * (self.n + 1) + j] = x;
    }
}

/// 1-based accessors (Fortran level `k`), as `TLIFT` and `CONVECT` read the
/// column.
impl ConvectSounding {
    #[inline]
    fn tk(&self, k: usize) -> f64 {
        self.t[k - 1]
    }
    #[inline]
    fn qk(&self, k: usize) -> f64 {
        self.q[k - 1]
    }
    #[inline]
    fn qsk(&self, k: usize) -> f64 {
        self.qs[k - 1]
    }
    #[inline]
    fn pk(&self, k: usize) -> f64 {
        self.p_hpa[k - 1]
    }
    #[inline]
    fn phk(&self, k: usize) -> f64 {
        self.ph_hpa[k - 1]
    }
}

/// `SUBROUTINE TLIFT(GZ,ICB,NK,TVP,TPK,CLW,ND,NL,KK)`: lifted-parcel
/// temperature `TPK`, virtual temperature `TVP` and adiabatic liquid water
/// `CLW`. `kk = 1` does the sub-cloud levels and cloud base, `kk = 2` the
/// levels `ICB+1..NL`. Two fixed-point iterations of the saturation
/// adjustment, with Bolton's `es` over water above 0 °C and upstream's
/// `exp(23.33086 - 6111.72784/T + 0.15215 ln T)` below; `TG` is floored at
/// 35 K. Arrays are 1-based.
#[allow(clippy::too_many_arguments)]
fn tlift_impl(
    c: &ConvectSounding,
    gz: &[f64],
    icb: usize,
    nk: usize,
    tvp: &mut [f64],
    tpk: &mut [f64],
    clw: &mut [f64],
    nl: usize,
    kk: i32,
) {
    let qnk = c.qk(nk);
    let tnk = c.tk(nk);
    let ah0 = (CPD * (1.0 - qnk) + CL * qnk) * tnk + qnk * (LV0 - CPVMCL * (tnk - 273.15)) + gz[nk];
    let cpp = CPD * (1.0 - qnk) + qnk * CPV;
    let cpinv = 1.0 / cpp;

    if kk == 1 {
        for clw_i in clw.iter_mut().take(icb).skip(1) {
            *clw_i = 0.0;
        }
        for i in nk..icb {
            tpk[i] = tnk - (gz[i] - gz[nk]) * cpinv;
            tvp[i] = tpk[i] * (1.0 + qnk * EPSI);
        }
    }

    let (nsb, nst) = if kk == 2 { (icb + 1, nl) } else { (icb, icb) };
    for i in nsb..=nst {
        let ti = c.tk(i);
        let mut tg = ti;
        let mut qg = c.qsk(i);
        let alv = LV0 - CPVMCL * (ti - 273.15);
        for _ in 1..=2 {
            let mut s = CPD + alv * alv * qg / (RV * ti * ti);
            s = 1.0 / s;
            let ahg = CPD * tg + (CL - CPD) * qnk * ti + alv * qg + gz[i];
            tg += s * (ah0 - ahg);
            tg = tg.max(35.0);
            let tc = tg - 273.15;
            let denom = 243.5 + tc;
            let es = if tc >= 0.0 {
                6.112 * (17.67 * tc / denom).exp()
            } else {
                (23.33086 - 6111.72784 / tg + 0.15215 * tg.ln()).exp()
            };
            qg = EPS0 * es / (c.pk(i) - es * (1.0 - EPS0));
        }
        let alv = LV0 - CPVMCL * (ti - 273.15);
        tpk[i] = (ah0 - (CL - CPD) * qnk * ti - gz[i] - alv * qg) / CPD;
        clw[i] = qnk - qg;
        clw[i] = clw[i].max(0.0);
        let rg = qg / (1.0 - qnk);
        tvp[i] = tpk[i] * (1.0 + rg * EPSI);
    }
}

/// Lifted-parcel quantities from [`tlift`].
#[derive(Debug, Clone, PartialEq)]
pub struct LiftedParcel {
    /// `TVP`, K, 0-based (Fortran level `k` at `k-1`); untouched levels are 0.
    pub tvp: Vec<f64>,
    /// `TPK`, K.
    pub tp: Vec<f64>,
    /// `CLW`, kg/kg.
    pub clw: Vec<f64>,
}

/// Public form of upstream `TLIFT`, both passes (`KK = 1` then `KK = 2`) on a
/// zeroed parcel, exactly as `CONVECT` drives it but without the virtual
/// temperature correction `CONVECT` applies between the passes. `gz` is the
/// geopotential, 0-based, levels `1..=nl+1`. Exposed for inspection; the
/// code-to-code test checks it through [`convect`].
#[must_use]
pub fn tlift(
    sounding: &ConvectSounding,
    gz: &[f64],
    icb: usize,
    nk: usize,
    nl: usize,
) -> LiftedParcel {
    let c = sounding;
    let mut gz1 = vec![0.0; nl + 3];
    gz1[1..=nl + 1].copy_from_slice(&gz[..=nl]);
    let mut tvp = vec![0.0; nl + 3];
    let mut tp = vec![0.0; nl + 3];
    let mut clw = vec![0.0; nl + 3];
    tlift_impl(c, &gz1, icb, nk, &mut tvp, &mut tp, &mut clw, nl, 1);
    tlift_impl(c, &gz1, icb, nk, &mut tvp, &mut tp, &mut clw, nl, 2);
    LiftedParcel {
        tvp: tvp[1..=nl + 1].to_vec(),
        tp: tp[1..=nl + 1].to_vec(),
        clw: clw[1..=nl + 1].to_vec(),
    }
}

/// `SUBROUTINE CONVECT(ND, NL, DELT, IFLAG, PRECIP, WD, TPRIME, QPRIME, CBMF)`.
///
/// - `sounding` — the column (`conv_mod`'s `TCONV`, `QCONV`, `QSCONV`,
///   `PCONV_HPA`, `PHCONV_HPA`), each at least `nl + 1` levels.
/// - `nl` — `NL`, the highest level convection may reach plus one
///   (FLEXPART passes `nconvlev`). Must be at least 3.
/// - `delt` — `DELT`, s.
/// - `cbmf` — `CBMF` on entry: the cloud-base mass flux remembered from the
///   previous call for this column, kg m⁻² s⁻¹ (`0` at the first call).
///
/// `ND` (array dimension) has no role once the arrays are slices.
///
/// # Panics
/// If `nl < 3` or a sounding vector is shorter than `nl + 1`.
#[must_use]
// Index loops are kept index-for-index with upstream's DO loops on purpose.
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::needless_range_loop
)]
pub fn convect(sounding: &ConvectSounding, nl: usize, delt: f64, cbmf: f64) -> ConvectOutput {
    assert!(nl >= 3, "convect needs NL >= 3, got {nl}");
    for (name, v) in [
        ("t", &sounding.t),
        ("q", &sounding.q),
        ("qs", &sounding.qs),
        ("p_hpa", &sounding.p_hpa),
        ("ph_hpa", &sounding.ph_hpa),
    ] {
        assert!(
            v.len() > nl,
            "convect: `{name}` has {} levels, needs NL+1 = {}",
            v.len(),
            nl + 1
        );
    }
    let c = sounding;
    let n1 = nl + 1;
    let dim = nl + 3; // 1-based, room for NL+2

    let mut cbmf = cbmf;
    let delti = 1.0 / delt;

    // ***  INITIALIZE OUTPUT ARRAYS AND PARAMETERS  ***
    let mut ft = vec![0.0; dim];
    let mut fq = vec![0.0; dim];
    let mut fdown = vec![0.0; dim];
    let mut sub = vec![0.0; dim];
    let mut fup = vec![0.0; dim];
    let mut m = vec![0.0; dim];
    let mut mp = vec![0.0; dim];
    let mut fmass = Mat::new(dim);
    let mut ment = Mat::new(dim);
    // (TH(I) is computed here upstream but read only by the dead IPBL block.)
    let mut precip = 0.0;
    let mut wd = 0.0;
    let mut tprime = 0.0;
    let mut qprime = 0.0;
    let mut iflag = ConvectFlag::NoConvection;

    let finish = |iflag: ConvectFlag,
                  cbmf: f64,
                  precip: f64,
                  wd: f64,
                  tprime: f64,
                  qprime: f64,
                  ft: &[f64],
                  fq: &[f64],
                  sub: &[f64],
                  fmass: &Mat,
                  nconvtop: Option<usize>| {
        let mut fm = vec![0.0; n1 * n1];
        for i in 1..=n1 {
            for j in 1..=n1 {
                fm[(i - 1) * n1 + (j - 1)] = fmass.get(i, j);
            }
        }
        ConvectOutput {
            iflag,
            cbmf,
            precip,
            wd,
            tprime,
            qprime,
            ft: ft[1..=n1].to_vec(),
            fq: fq[1..=n1].to_vec(),
            sub: sub[1..=n1].to_vec(),
            fmass: fm,
            n: n1,
            nconvtop,
        }
    };

    // *** CALCULATE ARRAYS OF GEOPOTENTIAL, HEAT CAPACITY AND STATIC ENERGY
    let mut gz = vec![0.0; dim];
    let mut cpn = vec![0.0; dim];
    let mut h = vec![0.0; dim];
    let mut lv = vec![0.0; dim];
    let mut hm = vec![0.0; dim];
    let mut tv = vec![0.0; dim];
    gz[1] = 0.0;
    cpn[1] = CPD * (1.0 - c.qk(1)) + c.qk(1) * CPV;
    h[1] = c.tk(1) * cpn[1];
    lv[1] = LV0 - CPVMCL * (c.tk(1) - 273.15);
    hm[1] = lv[1] * c.qk(1);
    tv[1] = c.tk(1) * (1.0 + c.qk(1) * EPSI - c.qk(1));
    let mut ahmin = 1.0e12;
    let mut ihmin = nl;
    for i in 2..=n1 {
        let tvx = c.tk(i) * (1.0 + c.qk(i) * EPSI - c.qk(i));
        let tvy = c.tk(i - 1) * (1.0 + c.qk(i - 1) * EPSI - c.qk(i - 1));
        gz[i] = gz[i - 1] + 0.5 * RD * (tvx + tvy) * (c.pk(i - 1) - c.pk(i)) / c.phk(i);
        cpn[i] = CPD * (1.0 - c.qk(i)) + CPV * c.qk(i);
        h[i] = c.tk(i) * cpn[i] + gz[i];
        lv[i] = LV0 - CPVMCL * (c.tk(i) - 273.15);
        hm[i] =
            (CPD * (1.0 - c.qk(i)) + CL * c.qk(i)) * (c.tk(i) - c.tk(1)) + lv[i] * c.qk(i) + gz[i];
        tv[i] = c.tk(i) * (1.0 + c.qk(i) * EPSI - c.qk(i));
        // ***  Find level of minimum moist static energy    ***
        if i >= MINORIG && hm[i] < ahmin && hm[i] < hm[i - 1] {
            ahmin = hm[i];
            ihmin = i;
        }
    }
    ihmin = ihmin.min(nl - 1);

    // ***  Find that model level below the level of minimum moist static
    // ***  energy that has the maximum value of moist static energy
    let mut ahmax = 0.0;
    let mut nk = MINORIG; // HSO 2009 bug fix: NK initialised
    for i in MINORIG..=ihmin {
        if hm[i] > ahmax {
            nk = i;
            ahmax = hm[i];
        }
    }

    // ***  CHECK WHETHER PARCEL LEVEL TEMPERATURE AND SPECIFIC HUMIDITY ARE REASONABLE
    if c.tk(nk) < 250.0 || c.qk(nk) <= 0.0 || ihmin == nl - 1 {
        return finish(
            ConvectFlag::NoConvection,
            0.0,
            precip,
            wd,
            tprime,
            qprime,
            &ft,
            &fq,
            &sub,
            &fmass,
            None,
        );
    }

    // ***  CALCULATE LIFTED CONDENSATION LEVEL OF AIR AT PARCEL ORIGIN LEVEL
    let rh = c.qk(nk) / c.qsk(nk);
    let chi = c.tk(nk) / (1669.0 - 122.0 * rh - c.tk(nk));
    let plcl = c.pk(nk) * rh.powf(chi);
    // Fortran `.LT.`/`.GE.` with a NaN operand are false, as in Rust.
    if plcl < 200.0 || plcl >= 2000.0 {
        return finish(
            ConvectFlag::LclTooHigh,
            0.0,
            precip,
            wd,
            tprime,
            qprime,
            &ft,
            &fq,
            &sub,
            &fmass,
            None,
        );
    }

    // ***  CALCULATE FIRST LEVEL ABOVE LCL (=ICB)  ***
    let mut icb = nl - 1;
    for i in nk + 1..=nl {
        if c.pk(i) < plcl {
            icb = icb.min(i);
        }
    }
    if icb >= nl - 1 {
        return finish(
            ConvectFlag::CloudBaseTooHigh,
            0.0,
            precip,
            wd,
            tprime,
            qprime,
            &ft,
            &fq,
            &sub,
            &fmass,
            None,
        );
    }

    // *** FIND TEMPERATURE UP THROUGH ICB AND TEST FOR INSTABILITY
    let mut tvp = vec![0.0; dim];
    let mut tp = vec![0.0; dim];
    let mut clw = vec![0.0; dim];
    tlift_impl(c, &gz, icb, nk, &mut tvp, &mut tp, &mut clw, nl, 1);
    for i in nk..=icb {
        tvp[i] -= tp[i] * c.qk(nk);
    }

    // ***  If there was no convection at last time step and parcel
    // ***       is stable at ICB then skip rest of calculation
    if cbmf == 0.0 && tvp[icb] <= (tv[icb] - DTMAX) {
        return finish(
            ConvectFlag::NoConvection,
            cbmf,
            precip,
            wd,
            tprime,
            qprime,
            &ft,
            &fq,
            &sub,
            &fmass,
            None,
        );
    }

    // ***  IF THIS POINT IS REACHED, MOIST CONVECTIVE ADJUSTMENT IS NECESSARY
    if iflag != ConvectFlag::CflViolated {
        iflag = ConvectFlag::Convection;
    }

    // ***  FIND THE REST OF THE LIFTED PARCEL TEMPERATURES
    tlift_impl(c, &gz, icb, nk, &mut tvp, &mut tp, &mut clw, nl, 2);

    // ***  SET THE PRECIPITATION EFFICIENCIES AND THE FRACTION OF
    // ***          PRECIPITATION FALLING OUTSIDE OF CLOUD
    let mut ep = vec![0.0; dim];
    let mut sigp = vec![0.0; dim];
    for i in 1..=nk {
        ep[i] = 0.0;
        sigp[i] = SIGS;
    }
    for i in nk + 1..=nl {
        let tca = tp[i] - 273.15;
        let mut elacrit = if tca >= 0.0 {
            ELCRIT
        } else {
            ELCRIT * (1.0 - tca / TLCRIT)
        };
        elacrit = elacrit.max(0.0);
        let epmax = 0.999;
        ep[i] = epmax * (1.0 - elacrit / clw[i].max(1.0e-8));
        ep[i] = ep[i].max(0.0);
        ep[i] = ep[i].min(epmax);
        sigp[i] = SIGS;
    }

    // ***       CALCULATE VIRTUAL TEMPERATURE AND LIFTED PARCEL VIRTUAL TEMPERATURE
    for i in icb + 1..=nl {
        tvp[i] -= tp[i] * c.qk(nk);
    }
    tvp[nl + 1] = tvp[nl] - (gz[nl + 1] - gz[nl]) / CPD;

    // ***        NOW INITIALIZE VARIOUS ARRAYS USED IN THE COMPUTATIONS
    let mut hp = vec![0.0; dim];
    let mut nent = vec![0_i32; dim];
    let mut water = vec![0.0; dim];
    let mut evap = vec![0.0; dim];
    let mut wt = vec![0.0; dim];
    let mut lvcp = vec![0.0; dim];
    let mut qent = Mat::new(dim);
    let mut elij = Mat::new(dim);
    let mut sij = Mat::new(dim);
    for i in 1..=n1 {
        hp[i] = h[i];
        nent[i] = 0;
        water[i] = 0.0;
        evap[i] = 0.0;
        wt[i] = OMTSNOW;
        lvcp[i] = lv[i] / cpn[i];
        for j in 1..=n1 {
            qent.set(i, j, c.qk(j));
            elij.set(i, j, 0.0);
            sij.set(i, j, 0.0);
        }
    }
    let mut qp = vec![0.0; dim];
    qp[1] = c.qk(1);
    for i in 2..=n1 {
        qp[i] = c.qk(i - 1);
    }

    // ***  FIND THE FIRST MODEL LEVEL (INB1) ABOVE THE PARCEL'S HIGHEST LEVEL
    // ***  OF NEUTRAL BUOYANCY AND THE HIGHEST LEVEL OF POSITIVE CAPE (INB)
    let mut cape = 0.0;
    let mut capem = 0.0;
    let mut inb = icb + 1;
    let mut inb1 = inb;
    let mut byp = 0.0;
    for i in icb + 1..nl {
        let by = (tvp[i] - tv[i]) * (c.phk(i) - c.phk(i + 1)) / c.pk(i);
        cape += by;
        if by >= 0.0 {
            inb1 = i + 1;
        }
        if cape > 0.0 {
            inb = i + 1;
            byp = (tvp[i + 1] - tv[i + 1]) * (c.phk(i + 1) - c.phk(i + 2)) / c.pk(i + 1);
            capem = cape;
        }
    }
    inb = inb.max(inb1);
    cape = capem + byp;
    let mut defrac = capem - cape;
    defrac = defrac.max(0.001);
    let mut frac = -cape / defrac;
    frac = frac.min(1.0);
    frac = frac.max(0.0);

    // ***   CALCULATE LIQUID WATER STATIC ENERGY OF LIFTED PARCEL
    for i in icb..=inb {
        hp[i] = h[nk] + (lv[i] + (CPD - CPV) * c.tk(i)) * ep[i] * clw[i];
    }

    // ***  CALCULATE CLOUD BASE MASS FLUX AND RATES OF MIXING, M(I)
    let mut dbosum = 0.0;

    // ***     INTERPOLATE DIFFERENCE BETWEEN LIFTED PARCEL AND ENVIRONMENTAL
    // ***     TEMPERATURES TO LIFTED CONDENSATION LEVEL
    let tvpplcl =
        tvp[icb - 1] - RD * tvp[icb - 1] * (c.pk(icb - 1) - plcl) / (cpn[icb - 1] * c.pk(icb - 1));
    let tvaplcl =
        tv[icb] + (tvp[icb] - tvp[icb + 1]) * (plcl - c.pk(icb)) / (c.pk(icb) - c.pk(icb + 1));
    let mut dtpbl = 0.0;
    for i in nk..icb {
        dtpbl += (tvp[i] - tv[i]) * (c.phk(i) - c.phk(i + 1));
    }
    dtpbl /= c.phk(nk) - c.phk(icb);
    let dtmin = tvpplcl - tvaplcl + DTMAX + dtpbl;
    let dtma = dtmin;

    // ***  ADJUST CLOUD BASE MASS FLUX   ***
    let cbmfold = cbmf;
    // C. Forster: adjustment of CBMF is not allowed to depend on FLEXPART timestep
    let delt0 = delt / 3.0;
    let damps = DAMP * delt / delt0;
    cbmf = (1.0 - damps) * cbmf + 0.1 * ALPHA * dtma;
    cbmf = cbmf.max(0.0);

    // *** If cloud base mass flux is zero, skip rest of calculation
    if cbmf == 0.0 && cbmfold == 0.0 {
        return finish(
            iflag, cbmf, precip, wd, tprime, qprime, &ft, &fq, &sub, &fmass, None,
        );
    }

    // ***   CALCULATE RATES OF MIXING,  M(I)   ***
    m[icb] = 0.0;
    for i in icb + 1..=inb {
        let k = i.min(inb1);
        let dbo = (tv[k] - tvp[k]).abs() + ENTP * 0.02 * (c.phk(k) - c.phk(k + 1));
        dbosum += dbo;
        m[i] = cbmf * dbo;
    }
    for mi in m.iter_mut().take(inb + 1).skip(icb + 1) {
        *mi /= dbosum;
    }

    // ***  CALCULATE ENTRAINED AIR MASS FLUX (MENT), TOTAL WATER MIXING
    // ***  RATIO (QENT), TOTAL CONDENSED WATER (ELIJ), AND MIXING FRACTION (SIJ)
    for i in icb + 1..=inb {
        let qti = c.qk(nk) - ep[i] * clw[i];
        for j in icb..=inb {
            let bf2 = 1.0 + lv[j] * lv[j] * c.qsk(j) / (RV * c.tk(j) * c.tk(j) * CPD);
            let mut anum = h[j] - hp[i] + (CPV - CPD) * c.tk(j) * (qti - c.qk(j));
            let mut denom = h[i] - hp[i] + (CPD - CPV) * (c.qk(i) - qti) * c.tk(j);
            let mut dei = denom;
            if dei.abs() < 0.01 {
                dei = 0.01;
            }
            sij.set(i, j, anum / dei);
            sij.set(i, i, 1.0);
            let mut altem = sij.get(i, j) * c.qk(i) + (1.0 - sij.get(i, j)) * qti - c.qsk(j);
            altem /= bf2;
            let cwat = clw[j] * (1.0 - ep[j]);
            let stemp = sij.get(i, j);
            if (stemp < 0.0 || stemp > 1.0 || altem > cwat) && j > i {
                anum -= lv[j] * (qti - c.qsk(j) - cwat * bf2);
                denom += lv[j] * (c.qk(i) - qti);
                if denom.abs() < 0.01 {
                    denom = 0.01;
                }
                sij.set(i, j, anum / denom);
                altem = sij.get(i, j) * c.qk(i) + (1.0 - sij.get(i, j)) * qti - c.qsk(j);
                altem -= (bf2 - 1.0) * cwat;
            }
            if sij.get(i, j) > 0.0 && sij.get(i, j) < 0.9 {
                qent.set(i, j, sij.get(i, j) * c.qk(i) + (1.0 - sij.get(i, j)) * qti);
                elij.set(i, j, altem);
                elij.set(i, j, 0.0_f64.max(elij.get(i, j)));
                ment.set(i, j, m[i] / (1.0 - sij.get(i, j)));
                nent[i] += 1;
            }
            sij.set(i, j, 0.0_f64.max(sij.get(i, j)));
            sij.set(i, j, 1.0_f64.min(sij.get(i, j)));
        }
        // ***   IF NO AIR CAN ENTRAIN AT LEVEL I ASSUME THAT UPDRAFT DETRAINS
        if nent[i] == 0 {
            ment.set(i, i, m[i]);
            qent.set(i, i, c.qk(nk) - ep[i] * clw[i]);
            elij.set(i, i, clw[i]);
            sij.set(i, i, 1.0);
        }
    }
    sij.set(inb, inb, 1.0);

    // ***  NORMALIZE ENTRAINED AIR MASS FLUXES TO REPRESENT EQUAL PROBABILITIES OF MIXING
    for i in icb + 1..=inb {
        if nent[i] != 0 {
            let qp1 = c.qk(nk) - ep[i] * clw[i];
            let anum = h[i] - hp[i] - lv[i] * (qp1 - c.qsk(i));
            let mut denom = h[i] - hp[i] + lv[i] * (c.qk(i) - qp1);
            if denom.abs() < 0.01 {
                denom = 0.01;
            }
            let mut scrit = anum / denom;
            let alt = qp1 - c.qsk(i) + scrit * (c.qk(i) - qp1);
            if alt < 0.0 {
                scrit = 1.0;
            }
            scrit = scrit.max(0.0);
            let mut asij = 0.0;
            let mut smin = 1.0;
            for j in icb..=inb {
                if sij.get(i, j) > 0.0 && sij.get(i, j) < 0.9 {
                    let (smid, sjmax, sjmin);
                    if j > i {
                        let s = sij.get(i, j).min(scrit);
                        let mut sx = s;
                        let mut sn = s;
                        if s < smin && sij.get(i, j + 1) < s {
                            smin = s;
                            sx = sij.get(i, j + 1).min(sij.get(i, j)).min(scrit);
                            sn = sij.get(i, j - 1).max(sij.get(i, j));
                            sn = sn.min(scrit);
                        }
                        smid = s;
                        sjmax = sx;
                        sjmin = sn;
                    } else {
                        sjmax = sij.get(i, j + 1).max(scrit);
                        smid = sij.get(i, j).max(scrit);
                        let mut sn = 0.0;
                        if j > 1 {
                            sn = sij.get(i, j - 1);
                        }
                        sjmin = sn.max(scrit);
                    }
                    let delp = (sjmax - smid).abs();
                    let delm = (sjmin - smid).abs();
                    asij += (delp + delm) * (c.phk(j) - c.phk(j + 1));
                    ment.set(
                        i,
                        j,
                        ment.get(i, j) * (delp + delm) * (c.phk(j) - c.phk(j + 1)),
                    );
                }
            }
            asij = 1.0e-21_f64.max(asij);
            asij = 1.0 / asij;
            for j in icb..=inb {
                ment.set(i, j, ment.get(i, j) * asij);
            }
            let mut bsum = 0.0;
            for j in icb..=inb {
                bsum += ment.get(i, j);
            }
            if bsum < 1.0e-18 {
                nent[i] = 0;
                ment.set(i, i, m[i]);
                qent.set(i, i, c.qk(nk) - ep[i] * clw[i]);
                elij.set(i, i, clw[i]);
                sij.set(i, i, 1.0);
            }
        }
    }

    // ***  CHECK WHETHER EP(INB)=0, IF SO, SKIP PRECIPITATING DOWNDRAFT
    // ***  CALCULATION (upstream: IF(EP(INB).LT.0.0001)GOTO 405)
    if ep[inb] >= 0.0001 || ep[inb].is_nan() {
        // ***  INTEGRATE LIQUID WATER EQUATION TO FIND CONDENSED WATER
        // ***                AND CONDENSED WATER FLUX
        let mut jtt: usize = 2;
        // ***                    BEGIN DOWNDRAFT LOOP
        for i in (1..=inb).rev() {
            // ***              CALCULATE DETRAINED PRECIPITATION
            let mut wdtrain = G * ep[i] * m[i] * clw[i];
            if i > 1 {
                for j in 1..i {
                    let mut awat = elij.get(j, i) - (1.0 - ep[i]) * clw[i];
                    awat = awat.max(0.0);
                    wdtrain += G * awat * ment.get(j, i);
                }
            }
            // ***  Value of terminal velocity and coefficient of evaporation for snow
            let mut coeff = COEFFS;
            wt[i] = OMTSNOW;
            // ***  Value of terminal velocity and coefficient of evaporation for rain
            if c.tk(i) > 273.0 {
                coeff = COEFFR;
                wt[i] = OMTRAIN;
            }
            let qsm = 0.5 * (c.qk(i) + qp[i + 1]);
            let mut afac =
                coeff * c.phk(i) * (c.qsk(i) - qsm) / (1.0e4 + 2.0e3 * c.phk(i) * c.qsk(i));
            afac = afac.max(0.0);
            let mut sigt = sigp[i];
            sigt = 0.0_f64.max(sigt);
            sigt = 1.0_f64.min(sigt);
            let b6 = 100.0 * (c.phk(i) - c.phk(i + 1)) * sigt * afac / wt[i];
            let c6 = (water[i + 1] * wt[i + 1] + wdtrain / SIGD) / wt[i];
            let revap = 0.5 * (-b6 + (b6 * b6 + 4.0 * c6).sqrt());
            evap[i] = sigt * afac * revap;
            water[i] = revap * revap;

            // ***  CALCULATE PRECIPITATING DOWNDRAFT MASS FLUX UNDER
            // ***              HYDROSTATIC APPROXIMATION
            // (upstream: IF(I.EQ.1)GOTO 360)
            if i != 1 {
                let mut dhdp = (h[i] - h[i - 1]) / (c.pk(i - 1) - c.pk(i));
                dhdp = dhdp.max(10.0);
                mp[i] = 100.0 * GINV * lv[i] * SIGD * evap[i] / dhdp;
                mp[i] = mp[i].max(0.0);
                // ***   ADD SMALL AMOUNT OF INERTIA TO DOWNDRAFT
                let fac = 20.0 / (c.phk(i - 1) - c.phk(i));
                mp[i] = (fac * mp[i + 1] + mp[i]) / (1.0 + fac);
                // ***      FORCE MP TO DECREASE LINEARLY TO ZERO
                // ***      BETWEEN ABOUT 950 MB AND THE SURFACE
                if c.pk(i) > (0.949 * c.pk(1)) {
                    jtt = jtt.max(i);
                    mp[i] = mp[jtt] * (c.pk(1) - c.pk(i)) / (c.pk(1) - c.pk(jtt));
                }
            }
            // 360 CONTINUE
            // ***       FIND MIXING RATIO OF PRECIPITATING DOWNDRAFT
            // (upstream: IF(I.EQ.INB)GOTO 400)
            if i != inb {
                let qstm = if i == 1 { c.qsk(1) } else { c.qsk(i - 1) };
                if mp[i] > mp[i + 1] {
                    let rat = mp[i + 1] / mp[i];
                    qp[i] = qp[i + 1] * rat
                        + c.qk(i) * (1.0 - rat)
                        + 100.0 * GINV * SIGD * (c.phk(i) - c.phk(i + 1)) * (evap[i] / mp[i]);
                } else if mp[i + 1] > 0.0 {
                    qp[i] = (gz[i + 1] - gz[i]
                        + qp[i + 1] * (lv[i + 1] + c.tk(i + 1) * (CL - CPD))
                        + CPD * (c.tk(i + 1) - c.tk(i)))
                        / (lv[i] + c.tk(i) * (CL - CPD));
                }
                qp[i] = qp[i].min(qstm);
                qp[i] = qp[i].max(0.0);
            }
            // 400 CONTINUE
        }
        // ***  CALCULATE SURFACE PRECIPITATION IN MM/DAY
        precip += wt[1] * SIGD * water[1] * 3600.0 * 24000.0 / (ROWL * G);
    }
    // 405 CONTINUE

    // ***  CALCULATE DOWNDRAFT VELOCITY SCALE AND SURFACE TEMPERATURE AND
    // ***                    WATER VAPOR FLUCTUATIONS
    wd = BETA * mp[icb].abs() * 0.01 * RD * c.tk(icb) / (SIGD * c.pk(icb));
    qprime = 0.5 * (qp[1] - c.qk(1));
    tprime = LV0 * qprime / CPD;

    // ***  CALCULATE TENDENCIES OF LOWEST LEVEL POTENTIAL TEMPERATURE AND MIXING RATIO
    let mut dpinv = 0.01 / (c.phk(1) - c.phk(2));
    let mut am = 0.0;
    if nk == 1 {
        for mk in m.iter().take(inb + 1).skip(2) {
            am += mk;
        }
    }
    // save saturated upward mass flux for first level
    fup[1] = am;
    if (2.0 * G * dpinv * am) >= delti {
        iflag = ConvectFlag::CflViolated;
    }
    ft[1] += G * dpinv * am * (c.tk(2) - c.tk(1) + (gz[2] - gz[1]) / cpn[1]);
    ft[1] -= lvcp[1] * SIGD * evap[1];
    ft[1] += SIGD * wt[2] * (CL - CPD) * water[2] * (c.tk(2) - c.tk(1)) * dpinv / cpn[1];
    fq[1] = fq[1] + G * mp[2] * (qp[2] - c.qk(1)) * dpinv + SIGD * evap[1];
    fq[1] += G * am * (c.qk(2) - c.qk(1)) * dpinv;
    for j in 2..=inb {
        fq[1] += G * dpinv * ment.get(j, 1) * (qent.get(j, 1) - c.qk(1));
    }

    // ***  CALCULATE TENDENCIES OF POTENTIAL TEMPERATURE AND MIXING RATIO
    // ***               AT LEVELS ABOVE THE LOWEST LEVEL
    for i in 2..=inb {
        dpinv = 0.01 / (c.phk(i) - c.phk(i + 1));
        let cpinv = 1.0 / cpn[i];
        let mut amp1 = 0.0;
        let mut ad = 0.0;
        if i >= nk {
            for mk in m.iter().take(inb + 2).skip(i + 1) {
                amp1 += mk;
            }
        }
        for k in 1..=i {
            for j in i + 1..=inb + 1 {
                amp1 += ment.get(k, j);
            }
        }
        // save saturated upward mass flux
        fup[i] = amp1;
        if (2.0 * G * dpinv * amp1) >= delti {
            iflag = ConvectFlag::CflViolated;
        }
        for k in 1..i {
            for j in i..=inb {
                ad += ment.get(j, k);
            }
        }
        // save saturated downward mass flux
        fdown[i] = ad;
        ft[i] = ft[i]
            + G * dpinv
                * (amp1 * (c.tk(i + 1) - c.tk(i) + (gz[i + 1] - gz[i]) * cpinv)
                    - ad * (c.tk(i) - c.tk(i - 1) + (gz[i] - gz[i - 1]) * cpinv))
            - SIGD * lvcp[i] * evap[i];
        ft[i] += G
            * dpinv
            * ment.get(i, i)
            * (hp[i] - h[i] + c.tk(i) * (CPV - CPD) * (c.qk(i) - qent.get(i, i)))
            * cpinv;
        ft[i] +=
            SIGD * wt[i + 1] * (CL - CPD) * water[i + 1] * (c.tk(i + 1) - c.tk(i)) * dpinv * cpinv;
        fq[i] += G * dpinv * (amp1 * (c.qk(i + 1) - c.qk(i)) - ad * (c.qk(i) - c.qk(i - 1)));
        for k in 1..i {
            let mut awat = elij.get(k, i) - (1.0 - ep[i]) * clw[i];
            awat = awat.max(0.0);
            fq[i] += G * dpinv * ment.get(k, i) * (qent.get(k, i) - awat - c.qk(i));
        }
        for k in i..=inb {
            fq[i] += G * dpinv * ment.get(k, i) * (qent.get(k, i) - c.qk(i));
        }
        fq[i] = fq[i]
            + SIGD * evap[i]
            + G * (mp[i + 1] * (qp[i + 1] - c.qk(i)) - mp[i] * (qp[i] - c.qk(i - 1))) * dpinv;
    }

    // *** Adjust tendencies at top of convection layer to reflect
    // ***       actual position of the level zero CAPE
    let fqold = fq[inb];
    fq[inb] *= 1.0 - frac;
    fq[inb - 1] +=
        frac * fqold * ((c.phk(inb) - c.phk(inb + 1)) / (c.phk(inb - 1) - c.phk(inb))) * lv[inb]
            / lv[inb - 1];
    let ftold = ft[inb];
    ft[inb] *= 1.0 - frac;
    ft[inb - 1] +=
        frac * ftold * ((c.phk(inb) - c.phk(inb + 1)) / (c.phk(inb - 1) - c.phk(inb))) * cpn[inb]
            / cpn[inb - 1];

    // ***   Very slightly adjust tendencies to force exact
    // ***     enthalpy, momentum and tracer conservation
    let mut ents = 0.0;
    for i in 1..=inb {
        ents += (cpn[i] * ft[i] + lv[i] * fq[i]) * (c.phk(i) - c.phk(i + 1));
    }
    ents /= c.phk(1) - c.phk(inb + 1);
    for i in 1..=inb {
        ft[i] -= ents / cpn[i];
    }

    // **** DETERMINE MASS DISPLACEMENT MATRIX AND COMPENSATING SUBSIDENCE
    sub[1] = 0.0;
    let mut nconvtop = 1;
    for i in 1..=inb + 1 {
        for j in 1..=inb + 1 {
            if j == nk {
                fmass.set(j, i, fmass.get(j, i) + m[i]);
            }
            fmass.set(j, i, fmass.get(j, i) + ment.get(j, i));
            if fmass.get(j, i) > EPSILON {
                nconvtop = nconvtop.max(i).max(j);
            }
        }
        if i > 1 {
            sub[i] = fup[i - 1] - fdown[i];
        }
    }
    nconvtop += 1;

    finish(
        iflag,
        cbmf,
        precip,
        wd,
        tprime,
        qprime,
        &ft,
        &fq,
        &sub,
        &fmass,
        Some(nconvtop),
    )
}
