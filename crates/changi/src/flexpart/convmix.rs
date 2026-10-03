// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/calcmatrix.f90, src/redist.f90, src/convmix.f90,
//                    src/conv_mod.f90 (the state they share)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
//                    (P. Seibert, B. C. Krueger, A. Frank, C. Forster, A. Stohl,
//                    M. Harustak)
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.
//
// NOT translated: src/sort2.f90 (Numerical Recipes quicksort; licence not
// GPL-compatible) and src/random_mod.f90 (Numerical Recipes ran3). See the
// module doc for what replaces each.

//! FLEXPART's convective mixing of particles: [`calcmatrix`] turns Emanuel's
//! mass-flux diagnosis ([`super::convection::convect`]) for one grid column
//! into a **redistribution matrix** `fmassfrac`; [`redist`] moves one particle
//! with it (a random destination level, or compensating subsidence); and
//! [`convmix`] drives both over every grid column that holds particles, on the
//! mother grid and the nests.
//!
//! # State
//!
//! Upstream couples the three routines through module state: `conv_mod`
//! (column profiles, `fmass`, `fmassfrac`, `sub`, `nconvtop`), `com_mod`
//! (particles, fields, `ldirect`, `lsynctime`, `height`) and one `SAVE`d
//! array in `redist` (`uvzlev`). The port makes that state explicit:
//! [`ConvMod`] mirrors `conv_mod` plus `redist`'s `uvzlev`; everything
//! `com_mod` supplies is an argument. Persistent state is exactly what upstream
//! keeps between calls, so stale-value behaviour is reproduced rather than
//! guessed (see the quirks below).
//!
//! # Random numbers
//!
//! `redist` draws one uniform number per redistributed particle with
//! Numerical Recipes' `ran3`. That generator is **not ported and not
//! reimplemented** (licence; maintainer decision gh:#410). The port takes the
//! draws as an input iterator, consumed in the same order and at the same point
//! (only for particles inside the convective domain) as upstream consumes
//! `ran3`.
//!
//! # Sorting
//!
//! `convmix` visits particles grouped by grid column, sorting with
//! Numerical Recipes' `sort2` (a median-of-three quicksort with insertion sort
//! below 7 elements). It is **not translated**. The port uses Rust's stable
//! `sort_by_key`: the sorted key sequence is identical for any correct sort,
//! so columns are visited in the same order and each column's particles are
//! the same set. Only the order of particles **within one column** can
//! differ: the stable sort keeps ascending particle index, `sort2` is unstable
//! for more than 7 particles. That order matters for exactly one thing — which
//! particle receives which `ran3` draw. The code-to-code test measures the
//! permutation against upstream's compiled `sort2` and records where it
//! differs (see `dev/flexpart_reference_convection.f90`).
//!
//! # Upstream quirks, reproduced and documented
//!
//! 1. **`calcmatrix` restores the old cloud-base mass flux whenever `convect`
//!    reports no convection** (`calcmatrix.f90:110–113`). `convect` sets
//!    `CBMF = 0` on its `IFLAG` 0/2/3 returns, but `calcmatrix` overwrites it
//!    with `cbmfold`, so a column's `cbaseflux` never relaxes to 0 when it
//!    stops convecting. Emanuel's interface comment (`convect43c.f90:144–147`)
//!    asks the caller to keep the value `CONVECT` returns. Reproduced.
//! 2. **GFS `phconv` at the top reads `pconv(nuvz)`, which nothing assigns**
//!    (`calcmatrix.f90:76` with `kuvz = nuvz`; `convmix.f90:181–188` fills
//!    `pconv(1..nuvz-1)`). In a fresh process that element is the static
//!    zero (when `nuvz < nuvzmax`; at `nuvz = nuvzmax` it is out of bounds).
//!    [`ConvMod`] keeps `pconv[nuvz]` at its initial `0.0` and never writes
//!    it, which is what upstream does when `nuvz < nuvzmax`.
//! 3. **GFS nests use the previous column's pressures.** The nest loop of
//!    `convmix` (`convmix.f90:260–265`) has no GFS branch: it fills `tconv`/
//!    `qconv` from `tthn`/`qvhn` at `kz+1` and never sets `pconv`, so a GFS
//!    run's nested columns are computed with the `pconv` left by the last
//!    mother-grid column. Reproduced through the persistent [`ConvMod`].
//! 4. **`redist`'s top half-level height reads unassigned levels.** The
//!    `uvzlev` integration runs to `nconvtop + 1`, which can be `nuvz`; there
//!    it reads `tconv(nuvz)`, `qconv(nuvz)` and (ECMWF) `pconv(nuvz)`, none
//!    of which `convmix`/`calcmatrix` assign. The value is used only if a
//!    particle is sent to level `nconvtop`, whose matrix entries are zero by
//!    construction (`nconvtop` is one above the highest non-zero `fmass`
//!    index), so it does not reach a particle position. [`ConvMod`] keeps
//!    those elements at their static `0.0`, as upstream.
//! 5. **`nconvlev = nuvz - 1` would read unassigned levels in `convect`.**
//!    `gridcheck_ecmwf.f90:535–539` leaves `nconvlev = nuvz - 1` when no
//!    level lies above 50 hPa at standard surface pressure (the `DO` index
//!    after a completed loop); `convect` then reads `TCONV(nuvz)` etc., which
//!    `convmix` never fills. The port refuses that configuration
//!    ([`ConvMod::new`] requires `nconvlev <= nuvz - 2`) rather than guess.
//! 6. **`convect`'s `NCONVTOP` on early return is stale** (see
//!    [`super::convection`]). `calcmatrix` only reads it after `IFLAG` ∈ {1,4}
//!    and a non-zero flux, when it has been assigned; [`ConvMod::nconvtop`] is
//!    left untouched otherwise, as upstream leaves it.
//! 7. **`redist` recomputes `uvzlev` only when `ktop <= 1`**, and its comment
//!    says "when ktop.eq.1"; `convmix` resets `ktop = 0` at each new column
//!    and `redist` sets it to 2. Reproduced.
//! 8. **`redist`'s subsidence step uses the signed `lsynctime`** while the
//!    matrix uses `abs(lsynctime)` (`convmix.f90:67`): in a backward run the
//!    subsidence displacement is reversed and the matrix transposed.
//! 9. **`redist` reflects negative heights** (`ztra1 = -ztra1`) and caps every
//!    particle it is called for — including one above the convective domain
//!    (`goto 90`) — at `height(nz) - 0.5`.
//! 10. **Gross-flux accounting (`iflux = 1`, `calcfluxes`) is not ported**:
//!     it belongs to FLEXPART's flux output, not to particle transport. The
//!     port is upstream with `iflux = 0`.
//! 11. **A draw of exactly 0 relocates every particle to level 1.** `redist`
//!     accepts the first `k` with `rn <= ffraction`; `ffraction` starts at 0
//!     and `fmassfrac(levold, 1)` is 0 for any level that sends no mass to
//!     level 1, so `rn = 0` gives `levnew = 1` and, through the
//!     `ffraction > 1e-20` guard, `dlevfrac = 0.5`: the particle is placed at
//!     the log-pressure centre of level 1 whatever the matrix says. Measured in
//!     the fixture: every particle between 150 m and 15.5 km on the deep test
//!     column goes to 66.71 m at `rn = 0`, while at `rn = 1e-6` one of them
//!     moves. Whether `ran3` can return exactly 0 was not examined (its source
//!     is not read beyond its interface). Reproduced.
//!
//! # Verification
//!
//! Code-to-code against upstream compiled from source at both precisions
//! (`dev/flexpart_reference_convection.f90`,
//! `tests/flexpart_convection_code_to_code.rs`), 2026-10-02, **bit-exact
//! against the real(8) build** on every output: `calcmatrix` 21 calls
//! (4 200 outputs incl. `fmassfrac`), `redist` 736 single-particle calls
//! (2 944 outputs), `convmix` four successive calls on a mother grid and a
//! nest with 60 particles (424 outputs). `nest` coverage uses a `par_mod.f90`
//! copy with `maxnests=1, nxmaxn=12, nymaxn=12`. Mutation-tested: 20
//! mutations of this file, all killed. Stability of the sort: the test
//! measures that the stable sort and `sort2` agree on the sorted keys always,
//! and on the permutation whenever keys are distinct or `n <= 7`; on the
//! `convmix` calls they differ at 89-97 of 120 positions, and the test
//! re-pairs draws to particles to compare end to end.
//!
//! # Units
//!
//! Bare `f64` in FLEXPART's units: pressures Pa in `pconv`/`phconv`/`dpr`
//! (hPa in `*_hpa`), temperature K, specific humidity kg/kg, heights m above
//! ground, times s, mass fluxes kg m⁻² s⁻¹, `fmassfrac` kg/m².

use super::boundary_layer::f_qvsat;
use super::constants::{GA, R_AIR};
use super::convection::{convect, ConvectFlag, ConvectSounding};
use super::thermo::ew_kelvin;

/// Which meteorological product the columns come from (upstream's
/// `metdata_format`, `GRIBFILE_CENTRE_ECMWF` or not).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvMetFormat {
    /// ECMWF: pressures from the hybrid coefficients, convection on the
    /// original model levels (`tth`, `qvh` at `kz+1`).
    Ecmwf,
    /// NCEP GFS: pressures `pplev` supplied per level, convection on
    /// FLEXPART's levels (`tt`, `qv` at `kz`).
    Gfs,
}

/// Hybrid vertical coefficients from `com_mod` (`akz`, `bkz` at the model
/// levels, `akm`, `bkm` at the half levels), level `k` at index `k-1`. Pa and
/// dimensionless. Read only for [`ConvMetFormat::Ecmwf`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HybridCoefficients {
    /// `akz`, Pa.
    pub akz: Vec<f64>,
    /// `bkz`.
    pub bkz: Vec<f64>,
    /// `akm`, Pa.
    pub akm: Vec<f64>,
    /// `bkm`.
    pub bkm: Vec<f64>,
}

/// `conv_mod`, plus `redist`'s `SAVE`d `uvzlev`. All vectors are **1-based**
/// (index 0 unused) so subscripts read like upstream; matrices are
/// `(i, j)` → `i * (dim) + j` via [`ConvMod::fmassfrac`].
///
/// Every element starts at `0.0`, as `conv_mod`'s static arrays do, and is
/// written only where upstream writes it.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvMod {
    /// `nuvz`: number of model levels.
    pub nuvz: usize,
    /// `nconvlev`: `NL` passed to `convect`.
    pub nconvlev: usize,
    /// Side of the 1-based arrays (`nuvz + 2`).
    pub dim: usize,
    /// `pconv`, Pa.
    pub pconv: Vec<f64>,
    /// `phconv`, Pa.
    pub phconv: Vec<f64>,
    /// `dpr`, Pa.
    pub dpr: Vec<f64>,
    /// `pconv_hpa`, hPa.
    pub pconv_hpa: Vec<f64>,
    /// `phconv_hpa`, hPa.
    pub phconv_hpa: Vec<f64>,
    /// `ft`, K/s (written by `convect`).
    pub ft: Vec<f64>,
    /// `fq`, (kg/kg)/s.
    pub fq: Vec<f64>,
    /// `sub`, kg m⁻² s⁻¹.
    pub sub: Vec<f64>,
    /// `tconv`, K.
    pub tconv: Vec<f64>,
    /// `qconv`, kg/kg.
    pub qconv: Vec<f64>,
    /// `qsconv`, kg/kg.
    pub qsconv: Vec<f64>,
    /// `fmass`, kg m⁻² s⁻¹, `dim × dim`.
    pub fmass: Vec<f64>,
    /// `fmassfrac`, kg/m², `dim × dim`.
    pub fmassfrac: Vec<f64>,
    /// `psconv`, Pa.
    pub psconv: f64,
    /// `tt2conv`, K.
    pub tt2conv: f64,
    /// `td2conv`, K.
    pub td2conv: f64,
    /// `nconvtop`.
    pub nconvtop: usize,
    /// `redist`'s `SAVE`d `uvzlev`, m.
    pub uvzlev: Vec<f64>,
}

impl ConvMod {
    /// Fresh `conv_mod` (all zeros) for `nuvz` levels and `nconvlev`.
    ///
    /// # Panics
    /// Unless `3 <= nconvlev <= nuvz - 2` (quirk 5 of the module doc).
    #[must_use]
    pub fn new(nuvz: usize, nconvlev: usize) -> Self {
        assert!(
            nconvlev >= 3 && nconvlev + 2 <= nuvz,
            "ConvMod: need 3 <= nconvlev <= nuvz-2 (got nconvlev {nconvlev}, nuvz {nuvz}); \
             upstream with nconvlev = nuvz-1 reads unassigned levels"
        );
        let dim = nuvz + 2;
        let v = || vec![0.0; dim];
        Self {
            nuvz,
            nconvlev,
            dim,
            pconv: v(),
            phconv: v(),
            dpr: v(),
            pconv_hpa: v(),
            phconv_hpa: v(),
            ft: v(),
            fq: v(),
            sub: v(),
            tconv: v(),
            qconv: v(),
            qsconv: v(),
            fmass: vec![0.0; dim * dim],
            fmassfrac: vec![0.0; dim * dim],
            psconv: 0.0,
            tt2conv: 0.0,
            td2conv: 0.0,
            nconvtop: 0,
            uvzlev: v(),
        }
    }

    /// `fmassfrac(i, j)`, 1-based.
    #[must_use]
    pub fn fmassfrac(&self, i: usize, j: usize) -> f64 {
        self.fmassfrac[i * self.dim + j]
    }

    /// `fmass(i, j)`, 1-based.
    #[must_use]
    pub fn fmass(&self, i: usize, j: usize) -> f64 {
        self.fmass[i * self.dim + j]
    }

    fn set_fmassfrac(&mut self, i: usize, j: usize, x: f64) {
        let d = self.dim;
        self.fmassfrac[i * d + j] = x;
    }
}

/// What [`calcmatrix`] reports besides the state it leaves in [`ConvMod`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalcMatrixResult {
    /// `lconv`: the column convects and `fmassfrac` was updated.
    pub lconv: bool,
    /// `IFLAG` from `convect`.
    pub iflag: ConvectFlag,
}

/// `subroutine calcmatrix(lconv, delt, cbmf, metdata_format)`.
///
/// Reads `psconv`, `tconv(1..nuvz-1)`, `qconv(1..nuvz-1)` and (GFS)
/// `pconv(1..nuvz-1)` from `cm`, as `convmix` leaves them; writes `pconv`
/// (ECMWF), `phconv`, `dpr`, `qsconv`, `*_hpa`, `ft`, `fq`, `fmass`, `sub`,
/// `nconvtop` and `fmassfrac` exactly where upstream writes them.
///
/// - `delt` — convection time step, s (`abs(lsynctime)`).
/// - `cbmf` — the column's cloud-base mass flux, kg m⁻² s⁻¹, in/out
///   (`cbaseflux(ix,jy)`), see quirk 1.
pub fn calcmatrix(
    cm: &mut ConvMod,
    delt: f64,
    cbmf: &mut f64,
    format: ConvMetFormat,
    coeffs: &HybridCoefficients,
) -> CalcMatrixResult {
    let nuvz = cm.nuvz;
    let nconvlev = cm.nconvlev;

    cm.phconv[1] = cm.psconv;
    for kuvz in 2..=nuvz {
        let k = kuvz - 1;
        match format {
            ConvMetFormat::Ecmwf => {
                cm.pconv[k] = coeffs.akz[kuvz - 1] + coeffs.bkz[kuvz - 1] * cm.psconv;
                cm.phconv[kuvz] = coeffs.akm[kuvz - 1] + coeffs.bkm[kuvz - 1] * cm.psconv;
            }
            ConvMetFormat::Gfs => {
                cm.phconv[kuvz] = 0.5 * (cm.pconv[kuvz] + cm.pconv[k]);
            }
        }
        cm.dpr[k] = cm.phconv[k] - cm.phconv[kuvz];
        cm.qsconv[k] = f_qvsat(cm.pconv[k], cm.tconv[k]);
        // initialize mass fractions
        for kk in 1..=nconvlev {
            cm.set_fmassfrac(k, kk, 0.0);
        }
    }

    let cbmfold = *cbmf;
    for k in 1..=nconvlev + 1 {
        cm.pconv_hpa[k] = cm.pconv[k] / 100.0;
        cm.phconv_hpa[k] = cm.phconv[k] / 100.0;
    }
    cm.phconv_hpa[nconvlev + 1] = cm.phconv[nconvlev + 1] / 100.0;

    let n1 = nconvlev + 1;
    let sounding = ConvectSounding {
        t: cm.tconv[1..=n1].to_vec(),
        q: cm.qconv[1..=n1].to_vec(),
        qs: cm.qsconv[1..=n1].to_vec(),
        p_hpa: cm.pconv_hpa[1..=n1].to_vec(),
        ph_hpa: cm.phconv_hpa[1..=n1].to_vec(),
    };
    let out = convect(&sounding, nconvlev, delt, *cbmf);
    *cbmf = out.cbmf;
    // convect zeroes FT, FQ, SUB, FMASS over 1..NL+1 and writes inside it.
    for i in 1..=n1 {
        cm.ft[i] = out.ft[i - 1];
        cm.fq[i] = out.fq[i - 1];
        cm.sub[i] = out.sub[i - 1];
        for j in 1..=n1 {
            let d = cm.dim;
            cm.fmass[i * d + j] = out.fmass(i, j);
        }
    }
    if let Some(top) = out.nconvtop {
        cm.nconvtop = top;
    }
    let iflag = out.iflag;

    // do not update fmassfrac and cloudbase massflux if no convection takes
    // place or if a CFL criterion is violated in convect43c.f
    if iflag != ConvectFlag::Convection && iflag != ConvectFlag::CflViolated {
        *cbmf = cbmfold;
        return CalcMatrixResult {
            lconv: false,
            iflag,
        };
    }
    // ... or if the old and the new cloud base mass fluxes are zero
    if *cbmf <= 0.0 && cbmfold <= 0.0 {
        *cbmf = cbmfold;
        return CalcMatrixResult {
            lconv: false,
            iflag,
        };
    }

    // Update fmassfrac; account for mass displaced from level k to level k
    for k in 1..=cm.nconvtop {
        let rlevmass = cm.dpr[k] / GA;
        let mut summe = 0.0;
        for kk in 1..=cm.nconvtop {
            let v = delt * cm.fmass(k, kk);
            cm.set_fmassfrac(k, kk, v);
            summe += v;
        }
        let v = cm.fmassfrac(k, k) + rlevmass - summe;
        cm.set_fmassfrac(k, k, v);
    }
    CalcMatrixResult { lconv: true, iflag }
}

/// `redist`'s `const = r_air/ga`.
const REDIST_CONST: f64 = R_AIR / GA;

/// What [`redist`] did to one particle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RedistResult {
    /// New `ztra1`, m.
    pub z: f64,
    /// `ipconv`: `-1` if the particle was moved by the matrix, else `1`.
    pub ipconv: i32,
    /// Whether a random number was drawn (the particle was inside the
    /// convective domain).
    pub drew: bool,
}

/// `subroutine redist(ipart, ktop, ipconv)` for one particle at height `z`
/// (`ztra1(abs(ipart))`, m).
///
/// - `ktop` — in/out; `<= 1` makes `redist` (re)compute `uvzlev` for this
///   column and set it to 2 (quirk 7).
/// - `ldirect` — `1` forward (matrix rows), otherwise backward (columns).
/// - `lsynctime` — signed synchronisation step, s (quirk 8).
/// - `height_nz` — `height(nz)`, the top FLEXPART level, m.
/// - `draws` — the uniform numbers upstream takes from `ran3`; one is
///   consumed iff the particle is inside the convective domain.
///
/// # Panics
/// If a draw is needed and `draws` is exhausted.
// `-1.0 * x` is upstream's spelling (`-1.*x`); multiplying by -1 is exact,
// so it is bit-identical to negation and kept for side-by-side reading.
#[allow(clippy::neg_multiply, clippy::assign_op_pattern)]
pub fn redist<I: Iterator<Item = f64>>(
    cm: &mut ConvMod,
    z: f64,
    ktop: &mut i32,
    ldirect: i32,
    lsynctime: i64,
    height_nz: f64,
    draws: &mut I,
) -> RedistResult {
    let mut ipconv = 1;
    let mut ztra1 = z;
    let mut drew = false;

    if *ktop <= 1 {
        let mut tvold = cm.tt2conv * (1.0 + 0.378 * ew_kelvin(cm.td2conv) / cm.psconv);
        let mut pold = cm.psconv;
        cm.uvzlev[1] = 0.0;

        let mut pint = cm.phconv[2];
        let mut tv1 = cm.tconv[1] * (1.0 + 0.608 * cm.qconv[1]);
        let tv2 = cm.tconv[2] * (1.0 + 0.608 * cm.qconv[2]);
        let tv = tv1 + (tv2 - tv1) * (cm.pconv[1] - cm.phconv[2]) / (cm.pconv[1] - cm.pconv[2]);
        if (tv - tvold).abs() > 0.2 {
            cm.uvzlev[2] =
                cm.uvzlev[1] + REDIST_CONST * (pold / pint).ln() * (tv - tvold) / (tv / tvold).ln();
        } else {
            cm.uvzlev[2] = cm.uvzlev[1] + REDIST_CONST * (pold / pint).ln() * tv;
        }
        tvold = tv;
        tv1 = tv2;
        pold = pint;

        for kz in 3..=cm.nconvtop + 1 {
            pint = cm.phconv[kz];
            let tv2 = cm.tconv[kz] * (1.0 + 0.608 * cm.qconv[kz]);
            let tv = tv1
                + (tv2 - tv1) * (cm.pconv[kz - 1] - cm.phconv[kz])
                    / (cm.pconv[kz - 1] - cm.pconv[kz]);
            if (tv - tvold).abs() > 0.2 {
                cm.uvzlev[kz] = cm.uvzlev[kz - 1]
                    + REDIST_CONST * (pold / pint).ln() * (tv - tvold) / (tv / tvold).ln();
            } else {
                cm.uvzlev[kz] = cm.uvzlev[kz - 1] + REDIST_CONST * (pold / pint).ln() * tv;
            }
            tvold = tv;
            tv1 = tv2;
            pold = pint;
        }
        *ktop = 2;
    }

    // determine vertical grid position of particle in the eta system
    let ztold = ztra1;
    let mut levold = None;
    for kz in 2..=cm.nconvtop {
        if cm.uvzlev[kz] >= ztold {
            levold = Some(kz - 1);
            break;
        }
    }

    if let Some(levold) = levold {
        // now redistribute particles
        let rn = draws
            .next()
            .expect("redist: ran out of random draws (one per particle in the convective domain)");
        drew = true;

        let mut levnew = levold;
        let mut dlevfrac = f64::NAN; // assigned whenever levnew != levold
        let mut ffraction = 0.0;
        let totlevmass = cm.dpr[levold] / GA;
        for k in 1..=cm.nconvtop {
            // for backward runs use the transposed matrix
            let f = if ldirect == 1 {
                cm.fmassfrac(levold, k)
            } else {
                cm.fmassfrac(k, levold)
            };
            ffraction += f / totlevmass;
            if rn <= ffraction {
                levnew = k;
                // avoid division by zero or a too small number
                if ffraction > 1.0e-20 {
                    dlevfrac = (ffraction - rn) / f * totlevmass;
                } else {
                    dlevfrac = 0.5;
                }
                break;
            }
        }

        // now assign new position to particle
        if levnew <= cm.nconvtop {
            if levnew == levold {
                ztra1 = ztold;
            } else {
                let lp0 = cm.phconv[levnew].ln();
                let lp1 = cm.phconv[levnew + 1].ln();
                let dlogp = (1.0 - dlevfrac) * (lp1 - lp0);
                let pint = lp0 + dlogp;
                let dz1 = pint - lp0;
                let dz2 = lp1 - pint;
                let dz = dz1 + dz2;
                ztra1 = (cm.uvzlev[levnew] * dz2 + cm.uvzlev[levnew + 1] * dz1) / dz;
                if ztra1 < 0.0 {
                    ztra1 = -ztra1;
                }
                if ipconv > 0 {
                    ipconv = -1;
                }
            }
        }

        // displace particle according to compensating subsidence
        if levnew <= cm.nconvtop && levnew == levold {
            let ztold = ztra1;
            let wsub_lo = if levold > 1 {
                let temp_levold = cm.tconv[levold - 1]
                    + (cm.tconv[levold] - cm.tconv[levold - 1])
                        * (cm.pconv[levold - 1] - cm.phconv[levold])
                        / (cm.pconv[levold - 1] - cm.pconv[levold]);
                let sub_levold = cm.sub[levold] / (1.0 - cm.sub[levold] / cm.dpr[levold] * GA);
                -1.0 * sub_levold * R_AIR * temp_levold / cm.phconv[levold]
            } else {
                0.0
            };
            let temp_levold1 = cm.tconv[levold]
                + (cm.tconv[levold + 1] - cm.tconv[levold])
                    * (cm.pconv[levold] - cm.phconv[levold + 1])
                    / (cm.pconv[levold] - cm.pconv[levold + 1]);
            let sub_levold1 =
                cm.sub[levold + 1] / (1.0 - cm.sub[levold + 1] / cm.dpr[levold + 1] * GA);
            let wsub_hi = -1.0 * sub_levold1 * R_AIR * temp_levold1 / cm.phconv[levold + 1];

            // interpolate wsub to the vertical particle position
            let dz1 = ztold - cm.uvzlev[levold];
            let dz2 = cm.uvzlev[levold + 1] - ztold;
            let dz = dz1 + dz2;
            let wsubpart = (dz2 * wsub_lo + dz1 * wsub_hi) / dz;
            ztra1 = ztold + wsubpart * (lsynctime as f64);
            if ztra1 < 0.0 {
                ztra1 = -1.0 * ztra1;
            }
        }
    }

    // 90: Maximum altitude .5 meter below uppermost model level
    if ztra1 > height_nz - 0.5 {
        ztra1 = height_nz - 0.5;
    }
    RedistResult {
        z: ztra1,
        ipconv,
        drew,
    }
}

/// Stable replacement for Numerical Recipes' `sort2` (not translated): sort
/// `igrid` ascending, returning the sorted keys and the permutation `ipoint`
/// (0-based particle indices). Ties keep ascending particle index; see the
/// module doc for how that relates to `sort2`.
#[must_use]
pub fn sort_particles_by_column(igrid: &[i64]) -> (Vec<i64>, Vec<usize>) {
    let mut ipoint: Vec<usize> = (0..igrid.len()).collect();
    ipoint.sort_by_key(|&i| igrid[i]);
    let sorted = ipoint.iter().map(|&i| igrid[i]).collect();
    (sorted, ipoint)
}

/// Fields `convmix` interpolates for one grid (mother or nest), at FLEXPART's
/// two memory slots. 2-D fields are indexed `ix + nx*jy`; 3-D fields
/// `(ix + nx*jy)*nlev + (k-1)` for level `k`.
///
/// - ECMWF (and every nest): `t3`/`q3` are `tth`/`qvh` (`tthn`/`qvhn`), read
///   at `k = kz + 1`, `kz = 1..nuvz-1`; `p3` is unused.
/// - GFS mother grid: `t3`/`q3`/`p3` are `tt`/`qv`/`pplev`, read at `k = kz`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvMixMet {
    /// `nx` (`nxn` for a nest).
    pub nx: usize,
    /// `ny`.
    pub ny: usize,
    /// Levels stored per column in the 3-D fields.
    pub nlev: usize,
    /// `ps`, Pa, per slot.
    pub ps: [Vec<f64>; 2],
    /// `tt2`, K.
    pub tt2: [Vec<f64>; 2],
    /// `td2`, K.
    pub td2: [Vec<f64>; 2],
    /// Temperature, K.
    pub t3: [Vec<f64>; 2],
    /// Specific humidity, kg/kg.
    pub q3: [Vec<f64>; 2],
    /// Pressure, Pa (GFS mother grid only).
    pub p3: [Vec<f64>; 2],
    /// `cbaseflux` (`cbasefluxn`), kg m⁻² s⁻¹, in/out, `ix + nx*jy`.
    pub cbaseflux: Vec<f64>,
}

impl ConvMixMet {
    fn i2(&self, ix: usize, jy: usize) -> usize {
        ix + self.nx * jy
    }
    fn i3(&self, ix: usize, jy: usize, k: usize) -> usize {
        (ix + self.nx * jy) * self.nlev + (k - 1)
    }
}

/// One nested grid: its fields and `xln`, `yln`, `xrn`, `yrn` (mother-grid
/// coordinates of its corners) and `xresoln`, `yresoln`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvMixNest {
    /// The nest's fields and `cbasefluxn`.
    pub met: ConvMixMet,
    /// `xln`.
    pub xln: f64,
    /// `yln`.
    pub yln: f64,
    /// `xrn`.
    pub xrn: f64,
    /// `yrn`.
    pub yrn: f64,
    /// `xresoln`.
    pub xresoln: f64,
    /// `yresoln`.
    pub yresoln: f64,
}

/// The particle arrays of `com_mod` that `convmix` reads and writes.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvParticles {
    /// `itra1`, s.
    pub itra1: Vec<i64>,
    /// `xtra1`, grid units (`real(kind=dp)` upstream).
    pub xtra1: Vec<f64>,
    /// `ytra1`, grid units.
    pub ytra1: Vec<f64>,
    /// `ztra1`, m above ground; updated.
    pub ztra1: Vec<f64>,
}

/// Scalars `convmix` takes from `com_mod`/`par_mod`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvMixParams {
    /// `itime`, s.
    pub itime: i64,
    /// `memtime(1..2)`, s.
    pub memtime: [i64; 2],
    /// `memind(1..2)`: the memory slot (1 or 2) of each time.
    pub memind: [usize; 2],
    /// `lsynctime`, s (signed).
    pub lsynctime: i64,
    /// `ldirect`.
    pub ldirect: i32,
    /// `metdata_format`.
    pub format: ConvMetFormat,
    /// `height(nz)`, m.
    pub height_nz: f64,
    /// `par_mod`'s `nxmax` (enters the nest-edge margin `eps = nxmax/3e5`).
    pub nxmax: usize,
    /// Hybrid coefficients (ECMWF).
    pub coeffs: HybridCoefficients,
}

/// What [`convmix`] did, for inspection: the particles in the order they were
/// visited (0-based index, grid: 0 mother, `n` nest `n`) and the number of
/// draws consumed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConvMixReport {
    /// `(particle, grid, drew)` in visiting order, for columns that convect;
    /// `drew` is whether `redist` consumed a random number for it.
    pub visited: Vec<(usize, usize, bool)>,
    /// Random numbers consumed.
    pub draws_used: usize,
    /// Columns for which `calcmatrix` was called: `(grid, igrid)`.
    pub columns: Vec<(usize, i64)>,
}

/// The pseudo grid numbers `convmix` sorts by (`convmix.f90:85–137`):
/// `igrid` for the mother grid and `igridn[n]` for nest `n+1`, `1 + jy*nx + ix`
/// of the particle's nearest grid point, or `-1` for a particle not on that
/// grid (not yet released, i.e. `itra1 != itime`, or in another domain). A
/// particle goes to the innermost nest that contains it — strictly, and for
/// ECMWF with a margin `eps = nxmax/3e5` grid units — and otherwise to the
/// mother grid. `nint` is Fortran's round-half-away-from-zero, i.e.
/// [`f64::round`].
#[must_use]
pub fn convmix_column_keys(
    mother: &ConvMixMet,
    nests: &[ConvMixNest],
    parts: &ConvParticles,
    prm: &ConvMixParams,
) -> (Vec<i64>, Vec<Vec<i64>>) {
    let numpart = parts.ztra1.len();
    let numbnests = nests.len();
    let eps = prm.nxmax as f64 / 3.0e5;
    let mut igrid = vec![-1_i64; numpart];
    let mut igridn = vec![vec![-1_i64; numpart]; numbnests];
    for ipart in 0..numpart {
        if parts.itra1[ipart] != prm.itime {
            continue;
        }
        let x = parts.xtra1[ipart];
        let y = parts.ytra1[ipart];
        // Determine which nesting level to be used
        let mut ngrid = 0;
        for j in (1..=numbnests).rev() {
            let n = &nests[j - 1];
            let inside = match prm.format {
                ConvMetFormat::Ecmwf => {
                    x > n.xln + eps && x < n.xrn - eps && y > n.yln + eps && y < n.yrn - eps
                }
                ConvMetFormat::Gfs => x > n.xln && x < n.xrn && y > n.yln && y < n.yrn,
            };
            if inside {
                ngrid = j;
                break;
            }
        }
        if ngrid > 0 {
            let n = &nests[ngrid - 1];
            let xtn = (x - n.xln) * n.xresoln;
            let ytn = (y - n.yln) * n.yresoln;
            let ix = xtn.round() as i64;
            let jy = ytn.round() as i64;
            igridn[ngrid - 1][ipart] = 1 + jy * n.met.nx as i64 + ix;
        } else {
            let ix = x.round() as i64;
            let jy = y.round() as i64;
            igrid[ipart] = 1 + jy * mother.nx as i64 + ix;
        }
    }
    (igrid, igridn)
}

/// `subroutine convmix(itime, metdata_format)` with `iflux = 0` (quirk 10).
///
/// # Panics
/// If `draws` runs out, or a particle's column index falls outside its grid.
#[allow(clippy::too_many_lines)]
pub fn convmix<I: Iterator<Item = f64>>(
    cm: &mut ConvMod,
    mother: &mut ConvMixMet,
    nests: &mut [ConvMixNest],
    parts: &mut ConvParticles,
    prm: &ConvMixParams,
    draws: &mut I,
) -> ConvMixReport {
    let mut report = ConvMixReport::default();
    let numpart = parts.ztra1.len();
    let numbnests = nests.len();

    let dt1 = (prm.itime - prm.memtime[0]) as f64;
    let dt2 = (prm.memtime[1] - prm.itime) as f64;
    let dtt = 1.0 / (dt1 + dt2);
    let mind1 = prm.memind[0] - 1;
    let mind2 = prm.memind[1] - 1;
    let delt = prm.lsynctime.abs() as f64;

    if numpart == 0 {
        return report;
    }

    let (igrid, igridn) = convmix_column_keys(mother, nests, parts, prm);

    // 1. mother domain, then 2. the nests
    for grid in 0..=numbnests {
        let keys = if grid == 0 {
            igrid.clone()
        } else {
            igridn[grid - 1].clone()
        };
        let (sorted, ipoint) = sort_particles_by_column(&keys);
        let mut igrold = -1_i64;
        let mut ktop = 0_i32;
        let mut lconv = false;
        for kpart in 0..numpart {
            let igr = sorted[kpart];
            if igr == -1 {
                continue;
            }
            let ipart = ipoint[kpart];
            if igr != igrold {
                let met: &mut ConvMixMet = if grid == 0 {
                    &mut *mother
                } else {
                    &mut nests[grid - 1].met
                };
                let nx = met.nx as i64;
                let jy = (igr - 1) / nx;
                let ix = igr - jy * nx - 1;
                assert!(
                    ix >= 0 && jy >= 0 && (ix as usize) < met.nx && (jy as usize) < met.ny,
                    "convmix: column ({ix},{jy}) outside grid {grid}"
                );
                let (ix, jy) = (ix as usize, jy as usize);
                let c2 = met.i2(ix, jy);
                cm.psconv = (met.ps[mind1][c2] * dt2 + met.ps[mind2][c2] * dt1) * dtt;
                cm.tt2conv = (met.tt2[mind1][c2] * dt2 + met.tt2[mind2][c2] * dt1) * dtt;
                cm.td2conv = (met.td2[mind1][c2] * dt2 + met.td2[mind2][c2] * dt1) * dtt;
                let gfs_mother = grid == 0 && prm.format == ConvMetFormat::Gfs;
                for kz in 1..cm.nuvz {
                    if gfs_mother {
                        let c3 = met.i3(ix, jy, kz);
                        cm.pconv[kz] = (met.p3[mind1][c3] * dt2 + met.p3[mind2][c3] * dt1) * dtt;
                        cm.tconv[kz] = (met.t3[mind1][c3] * dt2 + met.t3[mind2][c3] * dt1) * dtt;
                        cm.qconv[kz] = (met.q3[mind1][c3] * dt2 + met.q3[mind2][c3] * dt1) * dtt;
                    } else {
                        let c3 = met.i3(ix, jy, kz + 1);
                        cm.tconv[kz] = (met.t3[mind1][c3] * dt2 + met.t3[mind2][c3] * dt1) * dtt;
                        cm.qconv[kz] = (met.q3[mind1][c3] * dt2 + met.q3[mind2][c3] * dt1) * dtt;
                    }
                }
                let mut cb = met.cbaseflux[c2];
                lconv = calcmatrix(cm, delt, &mut cb, prm.format, &prm.coeffs).lconv;
                met.cbaseflux[c2] = cb;
                report.columns.push((grid, igr));
                igrold = igr;
                ktop = 0;
            }
            if lconv {
                let r = redist(
                    cm,
                    parts.ztra1[ipart],
                    &mut ktop,
                    prm.ldirect,
                    prm.lsynctime,
                    prm.height_nz,
                    draws,
                );
                parts.ztra1[ipart] = r.z;
                if r.drew {
                    report.draws_used += 1;
                }
                report.visited.push((ipart, grid, r.drew));
            }
        }
    }
    report
}
