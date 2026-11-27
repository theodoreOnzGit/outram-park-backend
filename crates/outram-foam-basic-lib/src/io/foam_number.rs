// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. Written for this workspace; it reproduces the
// BEHAVIOUR of OpenFOAM-ESI v2506 `readDouble` (src/OpenFOAM/primitives/Scalar/
// doubleScalar/doubleScalar.C, scalarImpl.C) and copies none of its code.

//! Text-to-`f64` conversion exactly as OpenFOAM-ESI v2506 does it (gh:#1897).
//!
//! # What upstream does
//!
//! `doubleScalar.C` defines `ScalarConvert` as `::strtold` ("convert using larger
//! representation to properly capture underflow") and `readDouble` returns that
//! `long double` converted to `double`. On x86-64 `long double` is the x87
//! 80-bit extended type (64-bit significand). So every number OpenFOAM reads
//! from an ASCII file is rounded **twice**: decimal to a 64-bit significand,
//! then 64 to 53 bits, both to nearest-even. That is not the correctly rounded
//! value Rust's `str::parse::<f64>` returns, and it differs from it by one ulp
//! in roughly one decimal in 10^4 to 10^3 of ten-digit values: the point
//! `0.8417393185` reads as `0.84173931849999994` in OpenFOAM and as
//! `0.8417393185` (the nearest double) in Rust.
//!
//! # Why it is here
//!
//! It was the whole cause of gh:#1897 (README round 49, `genfoam_vs_upstream_gh1361`):
//! the 8 symmetric quad faces of the MSFR neutronics mesh whose `Sf`, `Cf`, `C`
//! and weights differed from upstream by up to 1.8e-14 were computed from points
//! that differed in the last bit (4 coordinates of 52 860), and the face routine
//! itself ([`crate::io::face_centre_and_area`]) is bit-identical to upstream's
//! `primitiveMeshTools::makeFaceCentresAndAreas` on all 34 578 faces.
//!
//! # Scope
//!
//! The two-step rounding is the platform behaviour of the reference build
//! (x86-64 Linux, GCC). A platform whose `long double` is 53-bit (MSVC, aarch64
//! Apple) gives the single rounding. The conversion is done in integer
//! arithmetic, so it is the same on every target (wasm, Android, no_std).
//! Anything that is not a plain decimal (`inf`, `nan`, hex, a binary exponent
//! out of the normal `f64` range) falls back to `str::parse::<f64>`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

/// Parse `s` as OpenFOAM-ESI v2506 on x86-64 Linux would (`strtold`, then
/// narrowed to `f64`). Returns `None` when `s` is not a number.
///
/// ```
/// use outram_foam_basic_lib::io::parse_foam_f64;
/// // OpenFOAM reads this point coordinate one ulp below the nearest double.
/// let of = parse_foam_f64("0.8417393185").unwrap();
/// let correct: f64 = "0.8417393185".parse().unwrap();
/// assert_ne!(of, correct);
/// assert_eq!(of.to_bits() + 1, correct.to_bits());
/// ```
pub fn parse_foam_f64(s: &str) -> Option<f64> {
    match decimal_to_f64_via_extended(s) {
        Some(v) => Some(v),
        None => s.parse::<f64>().ok(),
    }
}

/// Which rounding a reader applies to the decimal numbers of an OpenFOAM text file
/// (gh:#1909, README round 53).
///
/// [`Self::CorrectlyRounded`] is `str::parse::<f64>` and stays the default of the
/// shared dictionary and field readers: this workspace writes shortest
/// round-trip text and reads it back, which only the correctly rounded value
/// reproduces. [`Self::Esi`] is [`parse_foam_f64`], what OpenFOAM-ESI holds after
/// reading the same file, and is what a comparison against the real binary needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NumberRounding {
    /// `str::parse::<f64>` (the default).
    #[default]
    CorrectlyRounded,
    /// `strtold` narrowed to `f64`, as OpenFOAM-ESI `readDouble`.
    Esi,
}

impl NumberRounding {
    /// Parse `s` as a number with this rounding; `None` when it is not one.
    ///
    /// ```
    /// use outram_foam_basic_lib::io::foam_number::NumberRounding;
    /// let s = "0.8417393185";
    /// assert_eq!(NumberRounding::CorrectlyRounded.parse(s), s.parse::<f64>().ok());
    /// assert_ne!(NumberRounding::Esi.parse(s), s.parse::<f64>().ok());
    /// assert!(NumberRounding::Esi.parse("word").is_none());
    /// ```
    pub fn parse(self, s: &str) -> Option<f64> {
        match self {
            NumberRounding::CorrectlyRounded => s.parse::<f64>().ok(),
            NumberRounding::Esi => parse_foam_f64(s),
        }
    }
}

/// Little-endian base-2^32 unsigned integer, just enough for the conversion.
#[derive(Clone)]
struct Big {
    limbs: Vec<u32>,
}

impl Big {
    fn zero() -> Self {
        Big { limbs: Vec::new() }
    }

    fn trim(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    fn mul_small_add(&mut self, m: u32, a: u32) {
        let mut carry = u64::from(a);
        for l in self.limbs.iter_mut() {
            let v = u64::from(*l) * u64::from(m) + carry;
            *l = v as u32;
            carry = v >> 32;
        }
        if carry != 0 {
            self.limbs.push(carry as u32);
        }
    }

    fn bit_len(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(&top) => (self.limbs.len() - 1) * 32 + (32 - top.leading_zeros() as usize),
        }
    }

    fn bit(&self, i: usize) -> bool {
        let w = i / 32;
        w < self.limbs.len() && (self.limbs[w] >> (i % 32)) & 1 == 1
    }

    fn shl(&self, n: usize) -> Big {
        if self.is_zero() {
            return Big::zero();
        }
        let (w, b) = (n / 32, n % 32);
        let mut out = vec![0u32; w];
        let mut carry = 0u32;
        for &l in &self.limbs {
            if b == 0 {
                out.push(l);
            } else {
                out.push((l << b) | carry);
                carry = l >> (32 - b);
            }
        }
        if carry != 0 {
            out.push(carry);
        }
        Big { limbs: out }
    }

    fn cmp(&self, o: &Big) -> core::cmp::Ordering {
        if self.limbs.len() != o.limbs.len() {
            return self.limbs.len().cmp(&o.limbs.len());
        }
        for i in (0..self.limbs.len()).rev() {
            if self.limbs[i] != o.limbs[i] {
                return self.limbs[i].cmp(&o.limbs[i]);
            }
        }
        core::cmp::Ordering::Equal
    }

    fn sub_assign(&mut self, o: &Big) {
        let mut borrow = 0i64;
        for i in 0..self.limbs.len() {
            let b = if i < o.limbs.len() { i64::from(o.limbs[i]) } else { 0 };
            let mut v = i64::from(self.limbs[i]) - b - borrow;
            if v < 0 {
                v += 1 << 32;
                borrow = 1;
            } else {
                borrow = 0;
            }
            self.limbs[i] = v as u32;
        }
        self.trim();
    }

    /// `self << 1 | bit`.
    fn shl1_or(&mut self, bit: bool) {
        let mut carry = u32::from(bit);
        for l in self.limbs.iter_mut() {
            let next = *l >> 31;
            *l = (*l << 1) | carry;
            carry = next;
        }
        if carry != 0 {
            self.limbs.push(carry);
        }
    }

    /// Quotient (as `Big`) and whether the remainder is non-zero, `self / d`.
    fn div_rem_sticky(&self, d: &Big) -> (Big, bool) {
        let n = self.bit_len();
        let mut q = vec![0u32; n / 32 + 1];
        let mut rem = Big::zero();
        for i in (0..n).rev() {
            rem.shl1_or(self.bit(i));
            rem.trim();
            if rem.cmp(d) != core::cmp::Ordering::Less {
                rem.sub_assign(d);
                q[i / 32] |= 1 << (i % 32);
            }
        }
        let mut qb = Big { limbs: q };
        qb.trim();
        (qb, !rem.is_zero())
    }

    /// Bits `[lo, lo + 64)` of the value as a `u64` (zero above the top).
    fn bits64(&self, lo: usize) -> u64 {
        let mut v = 0u64;
        for k in 0..64 {
            if self.bit(lo + k) {
                v |= 1u64 << k;
            }
        }
        v
    }

    /// Whether any bit below `n` is set.
    fn any_below(&self, n: usize) -> bool {
        (0..n).any(|i| self.bit(i))
    }
}

/// `Some(f64)` for a plain decimal in the normal range, `None` to fall back.
fn decimal_to_f64_via_extended(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let mut i = 0;
    let neg = match b.first() {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let mut digits = Big::zero();
    let mut n_digits = 0usize;
    let mut frac = 0i64;
    let mut seen_dot = false;
    let mut any = false;
    while i < b.len() {
        match b[i] {
            c @ b'0'..=b'9' => {
                digits.mul_small_add(10, u32::from(c - b'0'));
                n_digits += 1;
                any = true;
                if seen_dot {
                    frac += 1;
                }
            }
            b'.' if !seen_dot => seen_dot = true,
            _ => break,
        }
        i += 1;
    }
    if !any || n_digits > 400 {
        return None;
    }
    let mut exp = 0i64;
    if i < b.len() {
        if b[i] != b'e' && b[i] != b'E' {
            return None;
        }
        i += 1;
        let eneg = match b.get(i) {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            exp = exp * 10 + i64::from(b[i] - b'0');
            if exp > 100_000 {
                return None;
            }
            i += 1;
        }
        if i == start || i != b.len() {
            return None;
        }
        if eneg {
            exp = -exp;
        }
    }
    digits.trim();
    if digits.is_zero() {
        return Some(if neg { -0.0 } else { 0.0 });
    }
    let e10 = exp - frac;
    if !(-340..=340).contains(&e10) {
        return None;
    }

    // value = q * 2^scale2 (+ a fraction of one unit of q when `sticky`)
    let (q, sticky, scale2): (Big, bool, i64) = if e10 >= 0 {
        let mut n = digits;
        for _ in 0..e10 {
            n.mul_small_add(10, 0);
        }
        (n, false, 0)
    } else {
        let mut p = Big { limbs: vec![1] };
        for _ in 0..(-e10) {
            p.mul_small_add(10, 0);
        }
        // shift so the quotient has at least 66 bits
        let need = 66 + p.bit_len() as i64 - digits.bit_len() as i64;
        let sh = if need > 0 { need as usize } else { 0 };
        let (q, st) = digits.shl(sh).div_rem_sticky(&p);
        (q, st, -(sh as i64))
    };

    // Step 1: to the 64-bit significand of the x87 extended type.
    let l = q.bit_len();
    let (m, mut e2): (u64, i64) = if l <= 64 {
        // exact (only reachable without a sticky remainder)
        (q.bits64(0) << (64 - l), scale2 - (64 - l as i64))
    } else {
        let drop = l - 64;
        let m = q.bits64(drop);
        let half_set = q.bit(drop - 1);
        let below = q.any_below(drop - 1) || sticky;
        let up = half_set && (below || m & 1 == 1);
        let mut m = m;
        let mut e = scale2 + drop as i64;
        if up {
            let (v, ov) = m.overflowing_add(1);
            if ov {
                m = 1u64 << 63;
                e += 1;
            } else {
                m = v;
            }
        }
        (m, e)
    };
    // Step 2: 64 to 53 bits (the extended value is exact here: no sticky).
    let low = m & 0x7FF;
    let mut m53 = m >> 11;
    e2 += 11;
    if low > 0x400 || (low == 0x400 && m53 & 1 == 1) {
        m53 += 1;
        if m53 == 1u64 << 53 {
            m53 = 1u64 << 52;
            e2 += 1;
        }
    }
    // value = m53 * 2^e2 with m53 in [2^52, 2^53)
    let biased = e2 + 52 + 1023;
    if !(1..=2046).contains(&biased) {
        return None;
    }
    let bits = ((biased as u64) << 52) | (m53 - (1u64 << 52));
    let v = f64::from_bits(bits);
    Some(if neg { -v } else { v })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_msfr_point_is_read_one_ulp_below_the_nearest_double() {
        // gh:#1897: the value OpenFOAM-ESI v2506 holds for "0.8417393185"
        // (dumped with %.17g from the real binary: 0.84173931849999994).
        let of = parse_foam_f64("0.8417393185").unwrap();
        assert_eq!(of, 0.84173931849999994_f64);
        let nearest: f64 = "0.8417393185".parse().unwrap();
        assert_eq!(of.to_bits() + 1, nearest.to_bits());
        assert_eq!(parse_foam_f64("-0.9766705101").unwrap(), -0.97667051009999994_f64);
    }

    #[test]
    fn exactly_representable_and_plain_values_equal_the_correct_parse() {
        for s in ["0", "-0", "1", "0.5", "123", "1e3", "2.5E-1", "1000000", "0.1", "3.14159", "+7", "1.0", "1e22", "1e-5"] {
            let a = parse_foam_f64(s).unwrap();
            let c: f64 = s.parse().unwrap();
            // within one ulp of the correctly rounded value, and equal when exact
            assert!((a.to_bits() as i64 - c.to_bits() as i64).abs() <= 1, "{s}");
        }
        for s in ["0", "1", "0.5", "123", "1e3", "2.5E-1", "1000000", "1.0", "1e22"] {
            assert_eq!(parse_foam_f64(s).unwrap(), s.parse::<f64>().unwrap(), "{s}");
        }
        assert!(parse_foam_f64("-0").unwrap().is_sign_negative());
    }

    #[test]
    fn non_decimals_fall_back_or_fail() {
        assert!(parse_foam_f64("abc").is_none());
        assert!(parse_foam_f64("").is_none());
        assert!(parse_foam_f64("1e").is_none());
        assert!(parse_foam_f64("inf").unwrap().is_infinite());
        assert_eq!(parse_foam_f64("1e-320").unwrap(), "1e-320".parse::<f64>().unwrap());
    }
}
