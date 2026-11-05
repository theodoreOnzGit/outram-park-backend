// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// ---------------------------------------------------------------------------
// PORTED (NOT from GSL) from EGSnrc, National Research Council Canada (NRC):
//
//   HEN_HOUSE/egs++/egs_rndm.h    (class EGS_RandomGenerator: the array
//                                  buffer, getUniform, getAzimuth,
//                                  getGaussian, the counters, the state)
//   HEN_HOUSE/egs++/egs_rndm.cpp  (class EGS_Ranmar: seeding, fillArray,
//                                  high resolution, saveState/resetState,
//                                  getCopy, setState; defaultRNG)
//
// read at commit bd49a47aae3938f49f4100b8e018091861408b19 (2026-09-28) of
// https://github.com/nrc-cnrc/EGSnrc. Copyright lines as upstream states them:
//
//   egs_rndm.h:   Copyright (C) 2015 National Research Council Canada
//                 Author: Iwan Kawrakow, 2005. Contributors: Reid Townson
//   egs_rndm.cpp: Copyright (C) 2015 National Research Council Canada
//                 Author: Iwan Kawrakow, 2005. Contributors: Hubert Ho,
//                 Reid Townson
//
// Licence: EGSnrc is free software under the GNU Affero General Public
// License, version 3 or (at your option) any later version. This file is
// conveyed under AGPL-3.0-only, which that grant permits.
//
// The generator is "the Zaman & Marsaglia ranmar generator" (egs_rndm.cpp's
// class comment): G. Marsaglia, A. Zaman and W. W. Tsang, "Toward a universal
// random number generator", Statistics & Probability Letters 9 (1990) 35-39.
// See NOTICE. Issue: gh:#1289.
// ---------------------------------------------------------------------------

//! **RANMAR**, EGSnrc's egs++ random number generator (`EGS_Ranmar`), with
//! the array-buffered `getUniform` of its base class `EGS_RandomGenerator`.
//!
//! # What is ported, and what is not
//!
//! Ported, line for line: the seeding (`EGS_Ranmar::setState(int,int)`), the
//! batch refill (`fillArray`, including its **high-resolution** second pass),
//! `getUniform` with its array pointer, `getAzimuth`, `getGaussian`, the
//! counters (`numbersGenerated`, `numbersUsed`, `resetCounter`), the
//! in-memory state calls (`saveState`, `resetState`, `getCopy`,
//! `setState(EGS_RandomGenerator*)`), the content of `storeState` /
//! `setState(istream&)` / `addState` as a plain [`RanmarState`] value, and
//! `defaultRNG`.
//!
//! Not ported:
//! - `createRNG`, the `rng definition` input block and `describeRNG`: input
//!   and output plumbing, not numerics (the brief of gh:#1289).
//! - The **text** format of `storeState`. Upstream writes the buffered
//!   numbers with the stream's default precision (6 significant digits), so
//!   a text round trip changes the not-yet-used numbers: the differential
//!   harness measured 24 of the next 300 draws differing, by up to
//!   3.0e-6 relative (`verification_and_validation/egsnrc_rndm/README.md`).
//!   [`RanmarState`] carries the same fields exactly instead.
//! - `rngSize()`: the byte size of the C++ objects, meaningless here.
//!
//! # Verification
//!
//! `tests/egsnrc_rndm_vs_upstream.rs` replays streams made by upstream's own
//! `egs_rndm.cpp`, compiled unmodified by the harness in
//! `verification_and_validation/egsnrc_rndm/harness/`: 12 RANMAR streams of
//! 10^6 draws (standard and high resolution, array sizes 1, 97, 128, 1000),
//! plus `getGaussian` and `getAzimuth` streams, all bit-identical.

#[allow(unused_imports)]
use crate::real::Real;

/// `twom24` (`egs_rndm.cpp`): 2^-24.
const TWOM24: f64 = 1. / 16777216.;
/// `cd` (`egs_rndm.cpp`): the carry decrement.
const CD: i32 = 7654321;
/// `cm` (`egs_rndm.cpp`): the carry modulus.
const CM: i32 = 16777213;
/// The lag table length (`int u[97]`).
const U_LEN: usize = 97;
/// The default array size of `EGS_RandomGenerator(int n=128)`.
pub const DEFAULT_ARRAY_SIZE: usize = 128;
/// The default first seed of `EGS_Ranmar(int ixx=1802, ...)`.
pub const DEFAULT_IXX: i32 = 1802;
/// The default second seed of `EGS_Ranmar(..., int jxx=9373, ...)`.
pub const DEFAULT_JXX: i32 = 9373;

/// Why a [`Ranmar`] could not be built or restored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RanmarError {
    /// An array size below 1. Upstream: `egsFatal("Attempt to construct a
    /// RNG with n < 1")` in `EGS_RandomGenerator::allocate`.
    ArraySizeBelowOne,
    /// `defaultRNG(sequence)` with a sequence outside the allowed range.
    /// Upstream: `egsFatal("... sequence %d is outside of allowed range")`.
    SequenceOutOfRange,
    /// A [`RanmarState`] whose fields no RANMAR could reach (a pointer
    /// outside the lag table or the buffer, a lag-table entry or carry
    /// outside its range). **DEVIATION:** upstream's `setState(istream&)`
    /// reads the fields without checking them; PETIR refuses them, because
    /// out-of-range values would index outside the tables.
    InvalidState,
}

impl core::fmt::Display for RanmarError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RanmarError::ArraySizeBelowOne => write!(f, "RANMAR array size must be at least 1"),
            RanmarError::SequenceOutOfRange => write!(f, "RANMAR default sequence out of range"),
            RanmarError::InvalidState => write!(f, "RANMAR state is outside the reachable range"),
        }
    }
}

/// The complete state of a [`Ranmar`]: every field upstream's `storeState`
/// writes, in the same order, held exactly.
///
/// **Upstream:** `EGS_RandomGenerator::storeState` (`count`, `np`, `ip`,
/// `rarray`) followed by `EGS_Ranmar::storePrivateState` (`ix`, `jx`, `c`,
/// `iseed1`, `iseed2`, `high_res`, `u`). `np` is `rarray.len()`.
#[derive(Clone, Debug, PartialEq)]
pub struct RanmarState {
    /// Random numbers generated so far (`count`).
    pub count: i64,
    /// Index of the next unused element of `rarray` (`ip`).
    pub ip: usize,
    /// The buffered numbers (`rarray`, length `np`).
    pub rarray: alloc::vec::Vec<f64>,
    /// First lag pointer (`ix`).
    pub ix: i32,
    /// Second lag pointer (`jx`).
    pub jx: i32,
    /// The carry (`c`).
    pub c: i32,
    /// The first initial seed (`iseed1`).
    pub iseed1: i32,
    /// The second initial seed (`iseed2`).
    pub iseed2: i32,
    /// High-resolution mode (`high_res`).
    pub high_res: bool,
    /// The lag table (`u`).
    pub u: [i32; U_LEN],
}

/// The saved copy that `saveState` keeps and `resetState` restores: what
/// upstream's `EGS_Ranmar::set` copies (`copyBaseState`, then `ix`, `jx`,
/// `c`, `iseed1`, `iseed2`, `u`). Like upstream, it does not carry
/// `high_res`.
#[derive(Clone, Debug, PartialEq)]
struct Saved {
    count: i64,
    ip: usize,
    rarray: alloc::vec::Vec<f64>,
    have_x: bool,
    the_x: f64,
    ix: i32,
    jx: i32,
    c: i32,
    iseed1: i32,
    iseed2: i32,
    u: [i32; U_LEN],
}

/// EGSnrc's RANMAR generator with its array buffer (`EGS_Ranmar` and its
/// base `EGS_RandomGenerator`).
///
/// Draw with [`Ranmar::get_uniform`] (a number in `[0, 1)`, 24 bits, or 48
/// in high resolution), [`Ranmar::get_azimuth`] or [`Ranmar::get_gaussian`];
/// or through PETIR's [`Rng`](super::Rng) trait, where `random::<f64>()`
/// returns exactly `get_uniform()` (one draw per word; see [`top24`] for the
/// mapping and why `Normal` must not be fed this generator).
///
/// [`Clone`] copies everything, including a state saved by
/// [`Ranmar::save_state`]; [`Ranmar::get_copy`] is upstream's `getCopy`,
/// which does not.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranmar {
    /// `count`: random numbers generated so far.
    count: i64,
    /// `ip`: index of the next unused element of `rarray`.
    ip: usize,
    /// `rarray`: the buffer, of size `np`.
    rarray: alloc::vec::Vec<f64>,
    /// `have_x`: a second Gaussian is waiting.
    have_x: bool,
    /// `the_x`: that second Gaussian.
    the_x: f64,
    /// `ix`, `jx`: the lag pointers, each in `0..=96`.
    ix: i32,
    jx: i32,
    /// `c`: the carry, in `0..CM`.
    c: i32,
    /// `u`: the lag table, entries in `0..2^24`.
    u: [i32; U_LEN],
    /// `iseed1`, `iseed2`: the seeds actually used (after clamping).
    iseed1: i32,
    iseed2: i32,
    /// `high_res`: draw a second number per element (48 bits).
    high_res: bool,
    /// `copy`: the state kept by `saveState`.
    saved: Option<Saved>,
}

impl Default for Ranmar {
    /// `EGS_Ranmar()`: seeds 1802 and 9373, an array of 128.
    fn default() -> Self {
        Ranmar::new(DEFAULT_IXX, DEFAULT_JXX)
    }
}

impl Ranmar {
    /// `EGS_Ranmar(ixx, jxx)` with upstream's default array size of 128.
    ///
    /// Seeds outside `1..31328` (first) or `1..30081` (second) are replaced
    /// by 1802 and 9373, as upstream does.
    pub fn new(ixx: i32, jxx: i32) -> Ranmar {
        Ranmar::seeded(ixx, jxx, DEFAULT_ARRAY_SIZE)
    }

    /// `EGS_Ranmar(ixx, jxx, n)`: an array of `n` numbers per refill.
    ///
    /// **Upstream:** the constructor calls `EGS_RandomGenerator(n)` (which
    /// fails for `n < 1`, `allocate`) and then `setState(ixx, jxx)`.
    pub fn with_array_size(ixx: i32, jxx: i32, n: usize) -> Result<Ranmar, RanmarError> {
        if n < 1 {
            return Err(RanmarError::ArraySizeBelowOne);
        }
        Ok(Ranmar::seeded(ixx, jxx, n))
    }

    /// The constructor body for an `n` already known to be at least 1.
    ///
    /// **Upstream:** `EGS_RandomGenerator(int n)` (`count = 0`, `np = 0`,
    /// `allocate(n)` sets `ip = np` so the first draw refills, `have_x =
    /// false`, `rarray` zeroed), then `high_res(false)`, `copy(0)` and
    /// `setState(ixx, jxx)`.
    fn seeded(ixx: i32, jxx: i32, n: usize) -> Ranmar {
        let mut r = Ranmar {
            count: 0,
            ip: n,
            rarray: alloc::vec![0.0; n],
            have_x: false,
            the_x: 0.0,
            ix: 0,
            jx: 0,
            c: 0,
            u: [0; U_LEN],
            iseed1: 0,
            iseed2: 0,
            high_res: false,
            saved: None,
        };
        r.init_seeds(ixx, jxx);
        r
    }

    /// `EGS_RandomGenerator::defaultRNG(sequence)`: seeds 33 and
    /// `97 + sequence`, carried into the first seed every 30081.
    ///
    /// **Upstream:** `egs_rndm.cpp`, `defaultRNG`. Sequences whose carry
    /// exceeds 31328 are an error (upstream: `egsFatal`), as is an `i32`
    /// overflow of `sequence + 97` (undefined behaviour upstream).
    pub fn default_rng(sequence: i32) -> Result<Ranmar, RanmarError> {
        let sequence = sequence
            .checked_add(97)
            .ok_or(RanmarError::SequenceOutOfRange)?;
        let mut ixx = 33;
        let iaux = sequence / 30081;
        let jxx = sequence - iaux * 30081;
        if iaux > 0 {
            ixx += iaux;
            if iaux > 31328 {
                return Err(RanmarError::SequenceOutOfRange);
            }
        }
        Ok(Ranmar::new(ixx, jxx))
    }

    /// Seed the lag table from two integers.
    ///
    /// **Upstream:** `egs_rndm.cpp`, `EGS_Ranmar::setState(int ixx, int
    /// jxx)`, line for line: the clamping of out-of-range seeds, the four
    /// sub-generator states `i, j, k, l`, 24 bits per table entry, then
    /// `c = 362436`, `ix = 96`, `jx = 32`.
    fn init_seeds(&mut self, mut ixx: i32, mut jxx: i32) {
        if ixx <= 0 || ixx >= 31328 {
            ixx = 1802;
        }
        if jxx <= 0 || jxx >= 30081 {
            jxx = 9373;
        }
        self.iseed1 = ixx;
        self.iseed2 = jxx;
        let mut i = (ixx / 177) % 177 + 2;
        let mut j = ixx % 177 + 2;
        let mut k = (jxx / 169) % 178 + 1;
        let mut l = jxx % 169;

        for uii in self.u.iter_mut() {
            let mut s = 0;
            let mut t = 8388608;
            for _jj in 0..24 {
                let m = (((i * j) % 179) * k) % 179;
                i = j;
                j = k;
                k = m;
                l = (53 * l + 1) % 169;
                if (l * m) % 64 >= 32 {
                    s += t;
                }
                t /= 2;
            }
            *uii = s;
        }
        self.c = 362436;
        self.ix = 96;
        self.jx = 32;
    }

    /// Turn high resolution on or off (`setHighResolution`).
    ///
    /// In high resolution each array element receives a second RANMAR
    /// number scaled by 2^-48, so a draw carries 48 bits.
    pub fn set_high_resolution(&mut self, hr: bool) {
        self.high_res = hr;
    }

    /// Whether high resolution is on.
    pub fn high_resolution(&self) -> bool {
        self.high_res
    }

    /// The two seeds in use, after upstream's clamping (`iseed1`, `iseed2`).
    pub fn initial_seeds(&self) -> (i32, i32) {
        (self.iseed1, self.iseed2)
    }

    /// The size of the array refilled at once (`np`).
    pub fn array_size(&self) -> usize {
        self.rarray.len()
    }

    /// One step of the subtract-with-borrow recurrence: the next 24-bit
    /// RANMAR integer.
    ///
    /// **Upstream:** the loop body of `EGS_Ranmar::fillArray`, written out
    /// twice there (once per pass); the MSVC-only diagnostic is not ported.
    /// `ix` and `jx` stay in `0..=96` and `u` entries in `0..2^24`, so the
    /// lookups cannot miss and the arithmetic cannot overflow; the
    /// `unwrap_or(0)` arms are unreachable.
    #[inline]
    fn next_int(&mut self) -> i32 {
        let uix = self.u.get(self.ix as usize).copied().unwrap_or(0);
        let ujx = self.u.get(self.jx as usize).copied().unwrap_or(0);
        self.jx -= 1;
        let mut r = uix - ujx;
        if r < 0 {
            r += 16777216;
        }
        if let Some(slot) = self.u.get_mut(self.ix as usize) {
            *slot = r;
        }
        self.ix -= 1;
        if self.ix < 0 {
            self.ix = 96;
        }
        if self.jx < 0 {
            self.jx = 96;
        }
        self.c -= CD;
        if self.c < 0 {
            self.c += CM;
        }
        r -= self.c;
        if r < 0 {
            r += 16777216;
        }
        r
    }

    /// Fill `array` with random numbers in `[0, 1)`, advancing the
    /// generator by `array.len()` numbers (twice that in high resolution).
    ///
    /// **Upstream:** `EGS_Ranmar::fillArray(int n, EGS_Float *array)`: the
    /// first pass sets `array[ii] = twom24*r`; in high resolution a second
    /// pass adds `twom24*twom24*r` to each element, in that order of
    /// operations; then `count += n`.
    pub fn fill_array(&mut self, array: &mut [f64]) {
        for a in array.iter_mut() {
            let r = self.next_int();
            *a = TWOM24 * f64::from(r);
        }
        if self.high_res {
            for a in array.iter_mut() {
                let r = self.next_int();
                *a += TWOM24 * TWOM24 * f64::from(r);
            }
        }
        self.count += array.len() as i64;
    }

    /// The next random number, uniform in `[0, 1)`.
    ///
    /// **Upstream:** `EGS_RandomGenerator::getUniform` (`egs_rndm.h`): when
    /// the pointer has passed the array, refill all of it with
    /// `fillArray(np, rarray)` and restart at 0.
    #[inline]
    pub fn get_uniform(&mut self) -> f64 {
        if self.ip >= self.rarray.len() {
            let mut rarray = core::mem::take(&mut self.rarray);
            self.fill_array(&mut rarray);
            self.rarray = rarray;
            self.ip = 0;
        }
        let x = self.rarray.get(self.ip).copied().unwrap_or(0.0);
        self.ip += 1;
        x
    }

    /// Cosine and sine of an azimuth uniform in `[0, 2 pi)`.
    ///
    /// **Upstream:** `EGS_RandomGenerator::getAzimuth` (`egs_rndm.h`), the
    /// default (rejection) branch, compiled unless `FAST_SINCOS` is defined:
    /// a point in the upper unit half-disc, then the double-angle formulas.
    /// No transcendental function is called.
    pub fn get_azimuth(&mut self) -> (f64, f64) {
        let mut xphi;
        let mut xphi2;
        let mut yphi;
        let mut yphi2;
        let mut rhophi;
        loop {
            xphi = 2.0 * self.get_uniform() - 1.0;
            xphi2 = xphi * xphi;
            yphi = self.get_uniform();
            yphi2 = yphi * yphi;
            rhophi = xphi2 + yphi2;
            if rhophi <= 1.0 {
                break;
            }
        }
        let cphi = (xphi2 - yphi2) / rhophi;
        let sphi = 2.0 * xphi * yphi / rhophi;
        (cphi, sphi)
    }

    /// A Gaussian random number with mean 0 and standard deviation 1.
    ///
    /// **Upstream:** `EGS_RandomGenerator::getGaussian` (`egs_rndm.h`):
    /// Box-Muller in polar form, `r = sqrt(-2*log(1 - u))` times the cosine
    /// and sine of [`Ranmar::get_azimuth`]; the sine is kept for the next
    /// call.
    ///
    /// `sqrt` is [`Real::sqrt`] (IEEE exact). `log` is [`crate::real::ln`],
    /// PETIR's port of ARM optimized-routines' `log`, the code glibc ships,
    /// in its portable (non-FMA) form. Upstream calls the platform `log`, so
    /// **upstream's own Gaussian stream depends on the machine**: on x86-64,
    /// glibc picks an FMA-contracted `log` when the CPU has FMA. Measured
    /// with the harness (glibc 2.44, 2026-11-05): with glibc's FMA variant
    /// masked (`GLIBC_TUNABLES=glibc.cpu.hwcaps=-FMA,-AVX2,-FMA4`) this
    /// stream is bit-identical to upstream's (200 000 of 200 000); with the
    /// FMA variant, 9 of 100 000 `log` calls round the other way. The
    /// `libm` crate's `log` (`Real::ln`) was rejected: it differed from
    /// both in 3.3 % of draws.
    pub fn get_gaussian(&mut self) -> f64 {
        if self.have_x {
            self.have_x = false;
            return self.the_x;
        }
        let r = Real::sqrt(-2.0 * crate::real::ln(1.0 - self.get_uniform()));
        let (cphi, sphi) = self.get_azimuth();
        self.have_x = true;
        self.the_x = r * sphi;
        r * cphi
    }

    /// Random numbers generated so far (`numbersGenerated`), including any
    /// still unused in the buffer.
    pub fn numbers_generated(&self) -> i64 {
        self.count
    }

    /// Random numbers handed out so far (`numbersUsed`).
    ///
    /// **Upstream:** `ip < np ? count - np + ip : count`.
    pub fn numbers_used(&self) -> i64 {
        let np = self.rarray.len();
        if self.ip < np {
            self.count - np as i64 + self.ip as i64
        } else {
            self.count
        }
    }

    /// Set the generated-numbers counter to zero (`resetCounter`).
    pub fn reset_counter(&mut self) {
        self.count = 0;
    }

    /// Keep a copy of the current state for [`Ranmar::reset_state`]
    /// (`saveState`). A later call replaces it.
    pub fn save_state(&mut self) {
        self.saved = Some(Saved {
            count: self.count,
            ip: self.ip,
            rarray: self.rarray.clone(),
            have_x: self.have_x,
            the_x: self.the_x,
            ix: self.ix,
            jx: self.jx,
            c: self.c,
            iseed1: self.iseed1,
            iseed2: self.iseed2,
            u: self.u,
        });
    }

    /// Return to the state kept by the last [`Ranmar::save_state`]
    /// (`resetState`); nothing happens if none was kept. The saved copy
    /// stays, so the state can be reset to again.
    ///
    /// **Upstream:** `EGS_Ranmar::resetState` → `set(*copy)`, which does not
    /// touch `high_res`. `copyBaseState` copies `rarray` only from `ip`
    /// onwards; PETIR copies all of it. The elements before `ip` are never
    /// read before the next refill overwrites them, so no draw differs.
    pub fn reset_state(&mut self) {
        if let Some(s) = &self.saved {
            self.count = s.count;
            self.ip = s.ip;
            self.rarray.clone_from(&s.rarray);
            self.have_x = s.have_x;
            self.the_x = s.the_x;
            self.ix = s.ix;
            self.jx = s.jx;
            self.c = s.c;
            self.iseed1 = s.iseed1;
            self.iseed2 = s.iseed2;
            self.u = s.u;
        }
    }

    /// A copy of this generator without its saved state (`getCopy`).
    ///
    /// **Upstream:** `EGS_Ranmar::getCopy`: the copy constructor (all of
    /// the state, `high_res` included) with `copy = 0`.
    pub fn get_copy(&self) -> Ranmar {
        let mut c = self.clone();
        c.saved = None;
        c
    }

    /// Take the state of `other` (`setState(EGS_RandomGenerator *r)`).
    ///
    /// **Upstream:** `copyBaseState(*r)` (counters, pointer, buffer, the
    /// waiting Gaussian), then `ix`, `jx`, `c`, the seeds and `u`; like
    /// upstream, `high_res` is not taken.
    pub fn set_state_from(&mut self, other: &Ranmar) {
        self.count = other.count;
        self.ip = other.ip;
        self.rarray.clone_from(&other.rarray);
        self.have_x = other.have_x;
        self.the_x = other.the_x;
        self.ix = other.ix;
        self.jx = other.jx;
        self.c = other.c;
        self.iseed1 = other.iseed1;
        self.iseed2 = other.iseed2;
        self.u = other.u;
    }

    /// The full state, as `storeState` would write it (see [`RanmarState`]).
    pub fn state(&self) -> RanmarState {
        RanmarState {
            count: self.count,
            ip: self.ip,
            rarray: self.rarray.clone(),
            ix: self.ix,
            jx: self.jx,
            c: self.c,
            iseed1: self.iseed1,
            iseed2: self.iseed2,
            high_res: self.high_res,
            u: self.u,
        }
    }

    /// Restore a state written by [`Ranmar::state`] (`setState(istream&)`).
    ///
    /// **Upstream:** `EGS_RandomGenerator::setState(istream&)` then
    /// `EGS_Ranmar::setPrivateState`. Like upstream, the waiting Gaussian
    /// (`have_x`) is not part of the stored state and is left as it is.
    /// **DEVIATION:** a state no RANMAR can reach is refused with
    /// [`RanmarError::InvalidState`] and leaves `self` unchanged (upstream
    /// does not check, and a bad `ix` there reads outside `u`).
    pub fn set_state(&mut self, s: &RanmarState) -> Result<(), RanmarError> {
        let pointer_ok = |p: i32| (0..U_LEN as i32).contains(&p);
        let valid = !s.rarray.is_empty()
            && s.ip <= s.rarray.len()
            && pointer_ok(s.ix)
            && pointer_ok(s.jx)
            && (0..CM).contains(&s.c)
            && s.u.iter().all(|&x| (0..16777216).contains(&x));
        if !valid {
            return Err(RanmarError::InvalidState);
        }
        self.count = s.count;
        self.ip = s.ip;
        self.rarray.clone_from(&s.rarray);
        self.ix = s.ix;
        self.jx = s.jx;
        self.c = s.c;
        self.iseed1 = s.iseed1;
        self.iseed2 = s.iseed2;
        self.high_res = s.high_res;
        self.u = s.u;
        Ok(())
    }

    /// Restore a state and add the current count to it (`addState`), as
    /// used to combine the results of parallel runs.
    ///
    /// **Upstream:** `EGS_RandomGenerator::addState`: `setState`, then
    /// `count += count_save`. On an invalid state nothing changes.
    pub fn add_state(&mut self, s: &RanmarState) -> Result<(), RanmarError> {
        let count_save = self.count;
        self.set_state(s)?;
        self.count += count_save;
        Ok(())
    }
}

/// The top 24 bits of a draw in `[0, 1)`, as an integer (`floor(x * 2^24)`).
///
/// This and the three functions below are PETIR's adapter from EGSnrc's
/// uniform outputs to [`Rng`](super::Rng) words; upstream has no integer
/// output.
///
/// # The contract: one draw per word, and `random::<f64>()` IS `getUniform`
///
/// Every EGSnrc output is an exact multiple of 2^-48 in `[0, 1)` (RANMAR:
/// `r * 2^-24`, or `+ r2 * 2^-48` in high resolution; RANLUX: `f32` values
/// no finer than 2^-48). So `x * 2^64` is an exact integer below 2^64, and
/// [`word64`] returns it. `RngExt::random::<f64>()` takes the top 53 bits
/// of that word times 2^-53, which gives back `x` exactly: **through PETIR's
/// `Rng`, `random::<f64>()` returns upstream's `getUniform()` bit for bit,
/// one draw per call** (pinned by `tests/egsnrc_rndm_vs_upstream.rs`). That
/// is what lets ports of EGSnrc code that draw through `Rng` (the alias
/// table, gh:#1289) reproduce EGSnrc's streams.
///
/// **The price, stated plainly: the low bits of every word are zero** (the
/// low 40 bits for a 24-bit draw, 16 for a 48-bit one). Any consumer that
/// reads low bits gets garbage. In PETIR that is [`super::standard_normal`]
/// and [`super::Normal`], whose ziggurat takes its layer from the low 8 bits
/// of `next_u64` (`normal.rs`): with these generators it would always pick
/// layer 0. **Do not feed RANMAR or RANLUX to `Normal`; use
/// [`Ranmar::get_gaussian`], upstream's Gaussian.** `random::<f64>()` and
/// [`super::Uniform`] (top 52 bits) are safe.
#[inline]
pub(super) fn top24(x: f64) -> u32 {
    (x * 16777216.0) as u32
}

/// A 32-bit word from one draw: `floor(x * 2^32)`, the top 32 bits of the
/// draw (8 of them zero for a 24-bit draw).
#[inline]
pub(super) fn word32<F: FnMut() -> f64>(mut draw: F) -> u32 {
    (draw() * 4294967296.0) as u32
}

/// A 64-bit word from one draw: `x * 2^64`, exact (see [`top24`]), so that
/// `random::<f64>()` returns the draw itself.
#[inline]
pub(super) fn word64<F: FnMut() -> f64>(mut draw: F) -> u64 {
    (draw() * 18446744073709551616.0) as u64
}

/// Bytes three to a draw, most significant first; a final partial chunk
/// consumes a whole draw.
#[inline]
pub(super) fn fill_bytes24<F: FnMut() -> f64>(dest: &mut [u8], mut draw: F) {
    for chunk in dest.chunks_mut(3) {
        let w = top24(draw()).to_be_bytes();
        for (d, s) in chunk.iter_mut().zip(w.iter().skip(1)) {
            *d = *s;
        }
    }
}

impl super::Rng for Ranmar {
    /// One draw, its top 32 bits: see [`word32`]. **Not upstream** (EGSnrc
    /// has no integer output).
    fn next_u32(&mut self) -> u32 {
        word32(|| self.get_uniform())
    }

    /// One draw times 2^64, exact: see [`top24`] for why
    /// `random::<f64>()` then equals [`Ranmar::get_uniform`] bit for bit,
    /// and why `Normal` must not be fed this generator.
    fn next_u64(&mut self) -> u64 {
        word64(|| self.get_uniform())
    }

    /// Three bytes per draw (its top 24 bits): see [`fill_bytes24`].
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        fill_bytes24(dest, || self.get_uniform())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{Rng, RngExt};

    /// Out-of-range seeds fall back to upstream's defaults; the array size
    /// must be at least 1.
    #[test]
    fn seeds_clamp_and_array_size_checked() {
        assert_eq!(Ranmar::new(0, 40000).initial_seeds(), (1802, 9373));
        assert_eq!(Ranmar::new(31328, 30081).initial_seeds(), (1802, 9373));
        assert_eq!(Ranmar::new(31327, 30080).initial_seeds(), (31327, 30080));
        assert_eq!(Ranmar::default(), Ranmar::new(1802, 9373));
        assert_eq!(
            Ranmar::with_array_size(1, 1, 0),
            Err(RanmarError::ArraySizeBelowOne)
        );
        assert_eq!(
            Ranmar::with_array_size(1, 1, 7).map(|r| r.array_size()),
            Ok(7)
        );
        assert!(!Ranmar::default().high_resolution());
    }

    /// `defaultRNG`: seeds 33 / 97 + sequence, carrying every 30081; too
    /// large a carry, or an overflow, is an error.
    #[test]
    fn default_rng_seeds_and_range() {
        assert_eq!(
            Ranmar::default_rng(0).map(|r| r.initial_seeds()),
            Ok((33, 97))
        );
        assert_eq!(
            Ranmar::default_rng(100000).map(|r| r.initial_seeds()),
            Ok((36, 100097 - 3 * 30081))
        );
        assert_eq!(
            Ranmar::default_rng(31329 * 30081).map(|r| r.initial_seeds()),
            Err(RanmarError::SequenceOutOfRange)
        );
        assert_eq!(
            Ranmar::default_rng(i32::MAX).map(|r| r.initial_seeds()),
            Err(RanmarError::SequenceOutOfRange)
        );
    }

    /// Counters: generated counts whole refills, used counts draws;
    /// resetting the counter zeroes the generated count.
    #[test]
    fn counters_follow_refills() {
        let mut r = Ranmar::default();
        assert_eq!((r.numbers_generated(), r.numbers_used()), (0, 0));
        for _ in 0..130 {
            r.get_uniform();
        }
        assert_eq!((r.numbers_generated(), r.numbers_used()), (256, 130));
        r.reset_counter();
        assert_eq!(r.numbers_generated(), 0);
    }

    /// saveState / resetState replay the same numbers (and the waiting
    /// Gaussian); getCopy continues the stream without the saved state;
    /// setState(other) takes the stream but not the resolution.
    #[test]
    fn save_reset_copy_and_set_state_from() {
        let mut r = Ranmar::default();
        r.reset_state(); // nothing saved: no change
        assert_eq!(r, Ranmar::default());
        for _ in 0..1000 {
            r.get_uniform();
        }
        r.get_gaussian();
        r.save_state();
        let a: alloc::vec::Vec<f64> = (0..300).map(|_| r.get_gaussian()).collect();
        r.reset_state();
        let mut copy = r.get_copy();
        let b: alloc::vec::Vec<f64> = (0..300).map(|_| r.get_gaussian()).collect();
        let c: alloc::vec::Vec<f64> = (0..300).map(|_| copy.get_gaussian()).collect();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert!(copy.saved.is_none() && r.saved.is_some());

        let mut hr = Ranmar::new(5, 5);
        hr.set_high_resolution(true);
        hr.set_state_from(&r);
        assert!(hr.high_resolution());
        assert_eq!(hr.state().u, r.state().u);
        assert_eq!(hr.get_uniform(), r.get_uniform());
    }

    /// state / set_state round-trip exactly (unlike upstream's text
    /// format); add_state adds the counts; unreachable states are refused.
    #[test]
    fn state_round_trip_and_validation() {
        let mut r = Ranmar::new(12, 34);
        r.set_high_resolution(true);
        for _ in 0..777 {
            r.get_uniform();
        }
        let s = r.state();
        let mut t = Ranmar::default();
        assert_eq!(t.set_state(&s), Ok(()));
        assert!(t.high_resolution());
        for _ in 0..1000 {
            assert_eq!(t.get_uniform().to_bits(), r.get_uniform().to_bits());
        }
        let mut sum = Ranmar::default();
        for _ in 0..10 {
            sum.get_uniform();
        }
        assert_eq!(sum.add_state(&s), Ok(()));
        assert_eq!(sum.numbers_generated(), s.count + 128);

        let mut bad = s.clone();
        bad.ix = 97;
        assert_eq!(t.set_state(&bad), Err(RanmarError::InvalidState));
        let mut bad = s.clone();
        bad.ip = bad.rarray.len() + 1;
        assert_eq!(sum.add_state(&bad), Err(RanmarError::InvalidState));
        let mut bad = s.clone();
        bad.u[3] = -1;
        assert_eq!(t.set_state(&bad), Err(RanmarError::InvalidState));
        let mut bad = s;
        bad.rarray.clear();
        assert_eq!(t.set_state(&bad), Err(RanmarError::InvalidState));
        for e in [
            RanmarError::ArraySizeBelowOne,
            RanmarError::SequenceOutOfRange,
            RanmarError::InvalidState,
        ] {
            assert!(!alloc::format!("{e}").is_empty());
        }
    }

    /// The Rng adapter: one draw per word; `random::<f64>()` is exactly
    /// `get_uniform`, in standard and high resolution; the low bits of a
    /// word are zero (the documented hazard for `Normal`).
    #[test]
    fn rng_words_are_one_draw_each() {
        for hr in [false, true] {
            let mut a = Ranmar::default();
            let mut b = Ranmar::default();
            a.set_high_resolution(hr);
            b.set_high_resolution(hr);
            for _ in 0..1000 {
                let x: f64 = a.random();
                assert_eq!(x.to_bits(), b.get_uniform().to_bits());
            }
            let w = a.next_u64();
            assert_eq!(w & 0xffff, 0);
            assert_eq!(w as f64 / 18446744073709551616.0, b.get_uniform());
            let d = b.get_uniform();
            assert_eq!(a.next_u32(), (d * 4294967296.0) as u32);
            let mut bytes = [0u8; 4];
            a.fill_bytes(&mut bytes);
            let e5 = top24(b.get_uniform()).to_be_bytes();
            let e6 = top24(b.get_uniform()).to_be_bytes();
            assert_eq!(bytes, [e5[1], e5[2], e5[3], e6[1]]);
        }
    }
}
