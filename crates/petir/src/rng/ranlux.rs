// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// ---------------------------------------------------------------------------
// PORTED (NOT from GSL) from EGSnrc, National Research Council Canada (NRC):
//
//   HEN_HOUSE/src/ranlux.mortran  (subroutine ranlux and its entries
//                                  init_ranlux, get_ranlux_state,
//                                  set_ranlux_state)
//   HEN_HOUSE/src/ranlux.macros   (the 24-number buffer: COMIN/RANDOM,
//                                  $RANDOMSET, $INITIALIZE RNG USING,
//                                  $DEFAULT-LL, $STORE/$RETRIEVE RNG STATE)
//
// read at commit bd49a47aae3938f49f4100b8e018091861408b19 (2026-09-28) of
// https://github.com/nrc-cnrc/EGSnrc. Copyright lines as upstream states them
// (both files):
//
//   Copyright (C) 2015 National Research Council Canada
//   Authors: Iwan Kawrakow, 2000; Dave Rogers, 2000
//   "Parts of the EGS code that originate from SLAC are distributed by NRC
//   under the terms of the AGPL 3.0 licence, in agreement with SLAC."
//
// Licence: EGSnrc is free software under the GNU Affero General Public
// License, version 3 or (at your option) any later version. This file is
// conveyed under AGPL-3.0-only, which that grant permits.
//
// The algorithm, as upstream credits it: "Subtract-and-borrow random number
// generator proposed by Marsaglia and Zaman, implemented by F. James with the
// name RCARRY in 1991, and later improved by Martin Luescher in 1993 to
// produce 'Luxury Pseudorandom Numbers'. Fortran 77 coded by F. James, 1993."
// References (upstream's): M. Luscher, Computer Physics Communications 79,
// 100 (1994); F. James, Computer Physics Communications 79, 111 (1994).
// See NOTICE. Issue: gh:#1289.
//
// NOTE: egs++'s egs_rndm.cpp has no RANLUX ("should offer implementation of
// RANLUX in a future release"); EGSnrc's RANLUX is the Mortran one above,
// the default RNG of the Mortran user codes (specs/all_common.spec:
// RANDOM = $(EGS_SOURCEDIR)ranlux).
// ---------------------------------------------------------------------------

//! **RANLUX**, EGSnrc's default random number generator for its Mortran
//! codes: Lüscher's luxury subtract-and-borrow generator in F. James's
//! coding, as `HEN_HOUSE/src/ranlux.mortran` has it, with the 24-number
//! buffer of `ranlux.macros`.
//!
//! # Luxury levels
//!
//! Upstream's table `nskipll = 0, 24, 73, 199, 365`: after each batch of 24
//! numbers, that many further numbers are generated and thrown away. Level 0
//! is RCARRY ("fails many tests"); upstream notes that "in actual EGSnrc
//! calculations we have obtained incorrect results when using level 0, but
//! level 1 and higher give the same results." The default is level 1
//! (`$DEFAULT-LL`). Levels outside `0..=4` become level 1, as upstream does.
//!
//! # Single precision inside, as upstream
//!
//! `twom24` is `real*4` upstream, so `uni*twom24` and, for small `uni`, the
//! 48-bit refinement `uni*twom24 + seeds(j24)*twom24*twom24` are computed
//! in **single precision** and only then stored to the `real*8` array. The
//! refinement is therefore rounded to 24 significant bits. PETIR computes it
//! in `f32` in the same order, which is what makes the stream bit-identical.
//!
//! # What is not ported
//!
//! `show_ranlux_seeds`, `print_ranlux_seeds`, the initialisation banner and
//! `$RNG-INPUTS` (output and input plumbing). `twop24` is computed upstream
//! but never used, so it is not ported.
//!
//! # Verification
//!
//! `tests/egsnrc_rndm_vs_upstream.rs` replays 28 streams of 10^6 numbers
//! made by `ranlux.mortran` itself (Mortran3 + gfortran with EGSnrc's flags,
//! `verification_and_validation/egsnrc_rndm/harness/`): every luxury level,
//! five seeds, the default path, the out-of-range clamps and a state round
//! trip. All bit-identical. The harness also reproduces the sums upstream's
//! own `user_codes/ranlux_test` says it should print.

/// `nskipll` (`ranlux.mortran`): numbers skipped per 24 at each luxury level.
const NSKIPLL: [i32; 5] = [0, 24, 73, 199, 365];
/// `$DEFAULT-LL` (`ranlux.macros`): the default luxury level.
pub const DEFAULT_LUXURY_LEVEL: i32 = 1;
/// `jseed_dflt` (`ranlux.mortran`): the seed used for `seedin <= 0`.
pub const DEFAULT_SEED: i32 = 314159265;
/// `icon` (`ranlux.mortran`): the modulus of the seeding LCG, 2147483563.
const ICON: i32 = 2147483563;
/// 2^24, the modulus of the subtract-and-borrow recurrence.
const TWOP24_I: i32 = 16777216;
/// `twom24` (`ranlux.mortran`), **`real*4`**: 2^-24. Upstream halves 1
/// twenty-four times, which is exact.
const TWOM24: f32 = 1.0 / 16777216.0;

/// Why a RANLUX state could not be restored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RanluxError {
    /// The packed pointers of `state(25)` decode to an `i24` or `j24`
    /// outside `1..=24`. Upstream: `set_ranlux_state` prints "Error in
    /// set_ranlux_state: seeds outside of allowed range!" and `stop`s.
    PointersOutOfRange,
    /// A seed outside `0..2^24`, or a buffer pointer outside `1..=25`.
    /// **DEVIATION:** upstream does not check these; out-of-range seeds
    /// would overflow its `integer*4` arithmetic.
    InvalidState,
}

impl core::fmt::Display for RanluxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RanluxError::PointersOutOfRange => {
                write!(f, "RANLUX state: seeds outside of allowed range")
            }
            RanluxError::InvalidState => write!(f, "RANLUX state is outside the reachable range"),
        }
    }
}

/// The full RANLUX state as upstream's `$STORE RNG STATE` writes it:
/// `seeds` (the 25 integers of `get_ranlux_state`), `rng_array` and
/// `rng_seed`.
#[derive(Clone, Debug, PartialEq)]
pub struct RanluxState {
    /// `get_ranlux_state`'s 25 integers: the 24 seeds, then
    /// `i24 + 100*(j24 + 100*nskip)`, negated when the carry is set.
    pub seeds: [i32; 25],
    /// The 24 buffered numbers (`rng_array`).
    pub rng_array: [f64; 24],
    /// The 1-based pointer to the next buffered number (`rng_seed`);
    /// 25 means the buffer is used up.
    pub rng_seed: i32,
}

/// EGSnrc's RANLUX generator with the 24-number buffer of its macros.
///
/// [`Ranlux::new`] is `$INITIALIZE RNG USING luxury_level AND seed;`
/// (`init_ranlux`, then a first fill); [`Ranlux::get_uniform`] is
/// `$RANDOMSET`. The raw `ranlux(rng_array)` subroutine is
/// [`Ranlux::fill_array`].
#[derive(Clone, Debug, PartialEq)]
pub struct Ranlux {
    /// `seeds(1..24)`: the lagged state, entries in `0..2^24`.
    seeds: [i32; 24],
    /// `carry`: 0 or 1.
    carry: i32,
    /// `i24`, `j24`: 1-based lag pointers, in `1..=24`.
    i24: i32,
    j24: i32,
    /// `nskip`: numbers discarded after each 24.
    nskip: i32,
    /// `rng_array`: the buffer of `COMIN/RANDOM`.
    rng_array: [f64; 24],
    /// `rng_seed`: 1-based pointer into `rng_array`, `25` when used up.
    rng_seed: i32,
}

impl Default for Ranlux {
    /// `$RNG-DEFAULT-INITIALIZATION;`: luxury level 1, the default seed.
    ///
    /// This is also the state of upstream's `not_initialized` branch (a
    /// `ranlux` call before any `init_ranlux`), which the harness checks.
    fn default() -> Self {
        Ranlux::new(DEFAULT_LUXURY_LEVEL, 0)
    }
}

/// `next(j)` of `ranlux.mortran`: `j - 1`, with `next(1) = 24`.
///
/// Upstream builds the 24-entry table `next(j) = j-1; next(1) = 24` and
/// looks pointers up in it; this returns the same entry.
#[inline]
fn next(j: i32) -> i32 {
    if j == 1 { 24 } else { j - 1 }
}

impl Ranlux {
    /// `$INITIALIZE RNG USING luxury_level AND seed;`: `init_ranlux`, then
    /// fill the buffer once.
    ///
    /// A `seed <= 0` means the default seed 314159265; a level outside
    /// `0..=4` means level 1, as upstream.
    pub fn new(luxury_level: i32, seed: i32) -> Ranlux {
        let mut r = Ranlux::init(luxury_level, seed);
        let mut a = [0.0; 24];
        r.fill_array(&mut a);
        r.rng_array = a;
        r.rng_seed = 1;
        r
    }

    /// `entry init_ranlux(luxury_level, seedin)`.
    ///
    /// **Upstream:** `ranlux.mortran`, line for line: the seed default, the
    /// luxury clamp, then 24 steps of the L'Ecuyer LCG `40014` mod
    /// `2147483563` in Schrage's form, each seed `mod 2^24`; `i24 = 24`,
    /// `j24 = 10`, and the carry set when `seeds(24) = 0`. The banner it
    /// prints is not ported. The buffer is left used up (`rng_seed = 25`).
    fn init(mut luxury_level: i32, seedin: i32) -> Ranlux {
        let mut jseed = seedin;
        if jseed <= 0 {
            jseed = DEFAULT_SEED;
        }
        if !(0..=4).contains(&luxury_level) {
            luxury_level = DEFAULT_LUXURY_LEVEL;
        }
        let nskip = NSKIPLL.get(luxury_level as usize).copied().unwrap_or(0);

        let mut seeds = [0i32; 24];
        for s in seeds.iter_mut() {
            let k = jseed / 53668;
            jseed = 40014 * (jseed - k * 53668) - k * 12211;
            if jseed < 0 {
                jseed += ICON;
            }
            *s = jseed % TWOP24_I;
        }
        let carry = if seeds.last() == Some(&0) { 1 } else { 0 };
        Ranlux {
            seeds,
            carry,
            i24: 24,
            j24: 10,
            nskip,
            rng_array: [0.0; 24],
            rng_seed: 25,
        }
    }

    /// The seed at 1-based position `j` (`seeds(j)`). `j` is always in
    /// `1..=24` (the pointers are kept there), so the `0` arm is
    /// unreachable.
    #[inline]
    fn seed(&self, j: i32) -> i32 {
        self.seeds.get((j - 1) as usize).copied().unwrap_or(0)
    }

    /// One subtract-and-borrow step: `uni = seeds(j24) - seeds(i24) -
    /// carry`, wrapped into `0..2^24` with the borrow, stored at `i24`, and
    /// both pointers advanced. Returns `uni`.
    ///
    /// **Upstream:** the body shared by both loops of `subroutine ranlux`.
    #[inline]
    fn step(&mut self) -> i32 {
        let mut uni = self.seed(self.j24) - self.seed(self.i24) - self.carry;
        if uni < 0 {
            uni += TWOP24_I;
            self.carry = 1;
        } else {
            self.carry = 0;
        }
        if let Some(s) = self.seeds.get_mut((self.i24 - 1) as usize) {
            *s = uni;
        }
        self.i24 = next(self.i24);
        self.j24 = next(self.j24);
        uni
    }

    /// `subroutine ranlux(rng_array)`: 24 new numbers in `[0, 1)`, then
    /// `nskip` discarded.
    ///
    /// **Upstream:** `ranlux.mortran`. A number is `uni*twom24`; when
    /// `uni < 4096` (fewer than 12 significant bits) it is refined with the
    /// next seed, `uni*twom24 + seeds(j24)*twom24*twom24`, using `j24`
    /// AFTER the pointer advance. Both are `real*4` expressions upstream
    /// (`twom24` is `real*4`), evaluated left to right, then widened to
    /// `real*8`; so they are `f32` here.
    pub fn fill_array(&mut self, rng_array: &mut [f64; 24]) {
        for a in rng_array.iter_mut() {
            let uni = self.step();
            *a = if uni >= 4096 {
                f64::from(uni as f32 * TWOM24)
            } else {
                let s = self.seed(self.j24) as f32;
                f64::from(uni as f32 * TWOM24 + s * TWOM24 * TWOM24)
            };
        }
        for _ in 0..self.nskip.max(0) {
            self.step();
        }
    }

    /// The next random number in `[0, 1)`: `$RANDOMSET`.
    ///
    /// **Upstream:** `ranlux.macros`: when `rng_seed > 24`, refill
    /// `rng_array` and set `rng_seed = 1`; then take `rng_array(rng_seed)`
    /// and advance.
    #[inline]
    pub fn get_uniform(&mut self) -> f64 {
        if self.rng_seed > 24 {
            let mut a = self.rng_array;
            self.fill_array(&mut a);
            self.rng_array = a;
            self.rng_seed = 1;
        }
        let x = self
            .rng_array
            .get((self.rng_seed - 1) as usize)
            .copied()
            .unwrap_or(0.0);
        self.rng_seed += 1;
        x
    }

    /// Numbers discarded after each batch of 24 (`nskip`): 0, 24, 73, 199
    /// or 365 for luxury levels 0 to 4.
    pub fn nskip(&self) -> i32 {
        self.nskip
    }

    /// `entry get_ranlux_state(state)`: the 24 seeds and the packed
    /// `i24 + 100*(j24 + 100*nskip)`, negative when the carry is set.
    pub fn get_ranlux_state(&self) -> [i32; 25] {
        let mut state = [0i32; 25];
        for (s, &x) in state.iter_mut().zip(self.seeds.iter()) {
            *s = x;
        }
        let mut packed = self.i24 + 100 * (self.j24 + 100 * self.nskip);
        if self.carry > 0 {
            packed = -packed;
        }
        if let Some(last) = state.last_mut() {
            *last = packed;
        }
        state
    }

    /// `entry set_ranlux_state(state)`: the inverse of
    /// [`Ranlux::get_ranlux_state`]. The buffer is not touched.
    ///
    /// **Upstream:** `ranlux.mortran`: `state(25) <= 0` means the carry is
    /// set; `nskip = status/10000`, `j24`, `i24` the next two pairs of
    /// digits; pointers outside `1..=24` are an error (upstream `stop`s).
    /// **DEVIATION:** seeds outside `0..2^24` are refused too, and on any
    /// error `self` is unchanged.
    pub fn set_ranlux_state(&mut self, state: &[i32; 25]) -> Result<(), RanluxError> {
        let (last, seeds) = match state.split_last() {
            Some(p) => p,
            None => return Err(RanluxError::InvalidState),
        };
        let (mut status, carry) = if *last <= 0 {
            (last.checked_neg().ok_or(RanluxError::InvalidState)?, 1)
        } else {
            (*last, 0)
        };
        let nskip = status / 10000;
        status -= nskip * 10000;
        let j24 = status / 100;
        let i24 = status - 100 * j24;
        if !(1..=24).contains(&j24) || !(1..=24).contains(&i24) {
            return Err(RanluxError::PointersOutOfRange);
        }
        if !seeds.iter().all(|s| (0..TWOP24_I).contains(s)) {
            return Err(RanluxError::InvalidState);
        }
        for (d, &s) in self.seeds.iter_mut().zip(seeds.iter()) {
            *d = s;
        }
        self.carry = carry;
        self.nskip = nskip;
        self.j24 = j24;
        self.i24 = i24;
        Ok(())
    }

    /// The full state, as `$STORE RNG STATE` writes it.
    pub fn state(&self) -> RanluxState {
        RanluxState {
            seeds: self.get_ranlux_state(),
            rng_array: self.rng_array,
            rng_seed: self.rng_seed,
        }
    }

    /// Restore a state, as `$RETRIEVE RNG STATE` does (read `rng_array`
    /// and `rng_seed`, then `set_ranlux_state`).
    ///
    /// **DEVIATION:** a `rng_seed` outside `1..=25` is refused (upstream
    /// would read outside the array); on any error `self` is unchanged.
    pub fn set_state(&mut self, s: &RanluxState) -> Result<(), RanluxError> {
        if !(1..=25).contains(&s.rng_seed) {
            return Err(RanluxError::InvalidState);
        }
        self.set_ranlux_state(&s.seeds)?;
        self.rng_array = s.rng_array;
        self.rng_seed = s.rng_seed;
        Ok(())
    }
}

impl super::Rng for Ranlux {
    /// One draw, its top 32 bits: see `ranmar::word32`. **Not upstream**
    /// (EGSnrc has no integer output).
    fn next_u32(&mut self) -> u32 {
        super::ranmar::word32(|| self.get_uniform())
    }

    /// One draw times 2^64, exact (every RANLUX output is a multiple of
    /// 2^-48), so `random::<f64>()` equals [`Ranlux::get_uniform`] bit for
    /// bit. The low bits are zero: do not feed this generator to
    /// `Normal` (see `ranmar::top24`).
    fn next_u64(&mut self) -> u64 {
        super::ranmar::word64(|| self.get_uniform())
    }

    /// Three bytes per draw (its top 24 bits): see `ranmar::fill_bytes24`.
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        super::ranmar::fill_bytes24(dest, || self.get_uniform())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    /// Luxury clamp, seed default, and the skip table.
    #[test]
    fn luxury_and_seed_defaults() {
        let skips: alloc::vec::Vec<i32> = (0..5).map(|l| Ranlux::new(l, 1).nskip()).collect();
        assert_eq!(skips, [0, 24, 73, 199, 365]);
        assert_eq!(Ranlux::new(7, 1), Ranlux::new(1, 1));
        assert_eq!(Ranlux::new(-1, 1), Ranlux::new(1, 1));
        assert_eq!(Ranlux::new(2, -5), Ranlux::new(2, DEFAULT_SEED));
        assert_eq!(Ranlux::default(), Ranlux::new(1, 0));
        assert_eq!(next(1), 24);
        assert_eq!(next(24), 23);
    }

    /// get/set state round-trip, including the buffer; bad states refused.
    #[test]
    fn state_round_trip_and_validation() {
        let mut r = Ranlux::new(3, 99);
        for _ in 0..1001 {
            r.get_uniform();
        }
        let s = r.state();
        let a: alloc::vec::Vec<u64> = (0..500).map(|_| r.get_uniform().to_bits()).collect();
        let mut t = Ranlux::new(0, 5);
        assert_eq!(t.set_state(&s), Ok(()));
        let b: alloc::vec::Vec<u64> = (0..500).map(|_| t.get_uniform().to_bits()).collect();
        assert_eq!(a, b);

        // A carry of 1 packs as a negative status.
        let mut c = Ranlux::new(0, 1);
        let mut found = false;
        for _ in 0..2000 {
            c.get_uniform();
            let st = c.get_ranlux_state();
            if st[24] < 0 {
                let mut d = Ranlux::new(4, 4);
                assert_eq!(d.set_ranlux_state(&st), Ok(()));
                assert_eq!(d.get_ranlux_state(), st);
                found = true;
                break;
            }
        }
        assert!(found);

        let mut bad = s.clone();
        bad.seeds[24] = 0;
        assert_eq!(t.set_state(&bad), Err(RanluxError::PointersOutOfRange));
        bad.seeds[24] = i32::MIN;
        assert_eq!(t.set_state(&bad), Err(RanluxError::InvalidState));
        let mut bad = s.clone();
        bad.seeds[0] = 1 << 24;
        assert_eq!(t.set_state(&bad), Err(RanluxError::InvalidState));
        let mut bad = s;
        bad.rng_seed = 26;
        assert_eq!(t.set_state(&bad), Err(RanluxError::InvalidState));
        for e in [RanluxError::PointersOutOfRange, RanluxError::InvalidState] {
            assert!(!alloc::format!("{e}").is_empty());
        }
    }

    /// The Rng adapter: `random::<f64>()` is exactly `get_uniform`, over
    /// enough draws to include the f32-refined small values (`uni < 4096`).
    #[test]
    fn rng_words() {
        use crate::rng::RngExt;
        let mut a = Ranlux::new(2, 7);
        let mut b = Ranlux::new(2, 7);
        let mut refined = 0;
        for _ in 0..200_000 {
            let x: f64 = a.random();
            let y = b.get_uniform();
            assert_eq!(x.to_bits(), y.to_bits());
            refined += usize::from(y < 4096.0 / 16777216.0);
        }
        assert!(refined > 0);
        let d = b.get_uniform();
        assert_eq!(a.next_u32(), (d * 4294967296.0) as u32);
        let mut bytes = [0u8; 2];
        a.fill_bytes(&mut bytes);
        let e = super::super::ranmar::top24(b.get_uniform()).to_be_bytes();
        assert_eq!(bytes, [e[1], e[2]]);
    }
}
