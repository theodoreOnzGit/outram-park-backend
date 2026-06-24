/// Linear Congruential Generator — direct port of `Foam::random_lcg`.
///
/// C++ source: `src/random_lcg.cpp`, `include/openmc/random_lcg.h`.
///
/// OpenMC uses a 64-bit LCG with modulus 2^64 (implicit wrapping):
///   x_{n+1} = MULT * x_n + INC  (mod 2^64)
///
/// The jump-ahead feature lets each particle own a completely independent
/// stream by skipping ahead by a per-particle stride (default 152917).
/// This is the key technique enabling reproducible parallel Monte Carlo.

/// LCG multiplier — Knuth's choice (identical to PCG-64).
pub const MULT: u64 = 6364136223846793005;
/// LCG additive increment.
pub const INC: u64 = 1442695040888963407;
/// Default per-particle stride (number of RNG draws reserved per particle).
pub const DEFAULT_STRIDE: u64 = 152917;

/// Advance the seed one step and return a uniform sample in [0, 1).
///
/// Maps to `double prn(uint64_t* seed)` in OpenMC.
/// The upper 52 bits of the new seed are used to form an IEEE double mantissa,
/// giving uniform floating-point samples with no division.
#[inline]
pub fn prn(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(MULT).wrapping_add(INC);
    (*seed >> 12) as f64 * (1.0 / (1u64 << 52) as f64)
}

/// Advance the seed `n` steps in O(log n) using the LCG jump-ahead identity.
///
/// Maps to `uint64_t future_seed(uint64_t n, uint64_t seed)`.
/// Algorithm: each iteration squares `a` and halves `n`, accumulating the
/// combined multiplier/increment for odd bits.  Identical to Knuth §3.2.1.
pub fn future_seed(mut n: u64, seed: u64) -> u64 {
    let mut a = MULT;
    let mut c = INC;
    let mut a_m: u64 = 1;
    let mut c_m: u64 = 0;
    while n > 0 {
        if n & 1 == 1 {
            a_m = a_m.wrapping_mul(a);
            c_m = c_m.wrapping_mul(a).wrapping_add(c);
        }
        c = a.wrapping_add(1).wrapping_mul(c);
        a = a.wrapping_mul(a);
        n >>= 1;
    }
    a_m.wrapping_mul(seed).wrapping_add(c_m)
}

/// Stateful 64-bit LCG — drop-in replacement for `oorandom::Rand64`.
///
/// Provides the same interface (`new`, `rand_float`, `rand_u64`) so boon-lay
/// code can substitute `use openmc_libs::rng::lcg::Lcg64 as Rand64` with no
/// other changes to call sites.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lcg64 {
    state: u64,
}

impl Lcg64 {
    /// Create a new generator from a 128-bit seed (matches `oorandom::Rand64::new`).
    /// The lower 64 bits are used; the generator is advanced once to avoid a
    /// trivial first output when seeded with 0.
    pub fn new(seed: u128) -> Self {
        let mut state = seed as u64;
        state = state.wrapping_mul(MULT).wrapping_add(INC);
        Self { state }
    }

    /// Return a uniform sample in [0, 1) and advance the state.
    #[inline]
    pub fn rand_float(&mut self) -> f64 {
        prn(&mut self.state)
    }

    /// Return a raw 64-bit integer and advance the state.
    #[inline]
    pub fn rand_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(MULT).wrapping_add(INC);
        self.state
    }
}

/// Derive an independent seed for particle `id` from a master seed.
///
/// Maps to `uint64_t init_seed(int64_t id, int offset)`.
/// Each particle gets a unique starting seed by striding from the master seed.
pub fn init_seed(id: i64, offset: i64, master_seed: i64) -> u64 {
    let base = future_seed(DEFAULT_STRIDE, master_seed as u64);
    future_seed((id + offset) as u64, base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prn_in_unit_interval() {
        let mut seed = 12345u64;
        for _ in 0..1000 {
            let x = prn(&mut seed);
            assert!(x >= 0.0 && x < 1.0, "prn out of [0,1): {x}");
        }
    }

    #[test]
    fn future_seed_matches_sequential() {
        let seed0 = 99999u64;
        // advance 1 step sequentially
        let mut s = seed0;
        prn(&mut s);
        let s_seq = s;
        // advance 1 step via jump-ahead
        let s_jump = future_seed(1, seed0);
        assert_eq!(s_seq, s_jump, "future_seed(1) must equal one sequential step");
    }

    #[test]
    fn future_seed_n_steps() {
        let seed0 = 0xdeadbeef_u64;
        let mut s = seed0;
        for _ in 0..100 { prn(&mut s); }
        let s_jump = future_seed(100, seed0);
        assert_eq!(s, s_jump, "future_seed(100) must match 100 sequential steps");
    }
}
