/// PCG-RXS-M-XS generator over a 64-bit LCG — port of OpenMC's `random_lcg`.
///
/// C++ source: `src/random_lcg.cpp`, `include/openmc/random_lcg.h`
/// (canonical tree: `/home/teddy0/Documents/research/openmc/`).
///
/// The **state** is a 64-bit LCG with modulus 2^64 (implicit wrapping):
///   x_{n+1} = MULT * x_n + INC  (mod 2^64)
///
/// The **output** is not that state. [`prn`] applies the PCG-RXS-M-XS output
/// permutation before converting to a double, because the raw LCG state carries
/// Marsaglia lattice structure that Monte Carlo transport is directly exposed
/// to (a history draws distance, direction, and energy from consecutive draws).
/// See [`prn`] for the derivation and the measured before/after statistics.
///
/// Keeping the two apart matters when reading this module: the permutation
/// touches the *output only*. The recurrence, and therefore [`future_seed`],
/// [`init_seed`], the jump-ahead identity, and the GPU shaders' bit-exact
/// integer-state mirror, are all independent of it.
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

/// PCG output-permutation multiplier (the "M" of RXS-M-XS).
///
/// Upstream `12605985483714917081ull`,
/// `/home/teddy0/Documents/research/openmc/src/random_lcg.cpp:41`.
const PCG_PERM_MULT: u64 = 12_605_985_483_714_917_081;

/// Advance the seed one step and return a uniform sample in [0, 1).
///
/// **Upstream:** `double prn(uint64_t* seed)` —
/// `/home/teddy0/Documents/research/openmc/src/random_lcg.cpp:32-44`
/// (declared `include/openmc/random_lcg.h:33`). The C++ body is:
///
/// ```text
/// *seed = (prn_mult * (*seed) + prn_add);
/// uint64_t word =
///   ((*seed >> ((*seed >> 59u) + 5u)) ^ *seed) * 12605985483714917081ull;
/// uint64_t result = (word >> 43u) ^ word;
/// return ldexp(result, -64);
/// ```
///
/// # What this computes
///
/// Two separate stages, and it matters which is which:
///
/// 1. **State advance** — `x <- MULT * x + INC (mod 2^64)`, the plain 64-bit
///    LCG recurrence. This is **unchanged** by the output permutation, so
///    [`future_seed`], [`init_seed`], the jump-ahead identity, and every
///    integer-state guarantee in this crate (including the GPU shaders'
///    bit-exact state mirror) are untouched.
/// 2. **Output permutation** — `PCG-RXS-M-XS` (O'Neill 2014, HMC-CS-2014-0905;
///    upstream adapts <https://github.com/imneme/pcg-c>): a **r**andom-length
///    **x**or-**s**hift whose shift amount `(x >> 59) + 5` is drawn from the
///    state's own top five bits, then a **m**ultiply by [`PCG_PERM_MULT`], then
///    a final **x**or-**s**hift fold by 43. The permuted word, not the raw
///    state, becomes the double.
///
/// # Why the permutation is here (this is the whole point)
///
/// A bare LCG has **Marsaglia lattice structure**: successive k-tuples of its
/// outputs do not fill the unit cube, they lie on a limited family of parallel
/// hyperplanes. Taking the top 52 bits of the state — what this function used
/// to return — sidesteps the weak-low-order-bit problem but does nothing at all
/// about the lattice, because the top bits *are* the state.
///
/// Monte Carlo transport consumes **tuples**: one history draws a flight
/// distance, then a scattering direction, then a secondary energy from
/// *consecutive* draws. That is precisely where hyperplane structure bites. The
/// RXS-M-XS permutation exists to destroy it, and measurably does — see
/// [`tests::lattice_structure_lag1`] and [`tests::lattice_structure_lag2`],
/// which measure the defect directly and record the before/after numbers.
///
/// Reproducing OpenMC's stream bit-for-bit is a **side effect** of this port,
/// not its purpose; see the "RNG goal: statistical correctness, NOT
/// particle-for-particle parity" section of this crate's `CLAUDE.md`.
///
/// # Range
///
/// Returns a sample in `[0, 1)` for every state reachable in practice.
///
/// **Known boundary case, inherited from upstream and deliberately not
/// patched.** `ldexp(result, -64)` converts a `u64` to a `f64` first, and the
/// 1024 values `result >= 2^64 - 1024` all round to `2^64`, giving exactly
/// `1.0`. The probability is `1024 / 2^64 = 2^-54 ~ 5.6e-17` per draw. Measured
/// 2026-08-06: **zero** occurrences in 5.0e7 consecutive draws from `seed = 1`
/// (expected count 2.8e-9), and the observed maximum was
/// `0.99999998925400047`. This is safe for every consumer in this crate — all
/// six index-by-uniform sites clamp with `.min(len - 1)`, and `-ln(1.0) = 0`
/// merely yields a zero-length flight — but it is recorded here rather than
/// silently "fixed", because clamping would be an undocumented divergence from
/// the reference implementation.
///
/// # Example
///
/// ```
/// use outram_mc_libs::rng::lcg::prn;
/// let mut seed = 1u64;
/// let x = prn(&mut seed);
/// assert!((0.0..1.0).contains(&x));
/// ```
#[inline]
pub fn prn(seed: &mut u64) -> f64 {
    // 1. Advance the LCG (unchanged recurrence).
    *seed = seed.wrapping_mul(MULT).wrapping_add(INC);

    // 2. Permute the output: PCG-RXS-M-XS.
    let word = ((*seed >> ((*seed >> 59) + 5)) ^ *seed).wrapping_mul(PCG_PERM_MULT);
    let result = (word >> 43) ^ word;

    // 3. ldexp(result, -64). 2^-64 is exactly representable, so this is the
    //    same value C++ computes; `wrapping_*` above keeps debug == release.
    (result as f64) * (1.0 / 18_446_744_073_709_551_616.0)
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
/// code can substitute `use outram_mc_libs::rng::lcg::Lcg64 as Rand64` with no
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

/// Derive an independent RNG stream seed for particle `id` from a master seed.
///
/// **Upstream:** `uint64_t init_seed(int64_t id, int offset)` —
/// `/home/teddy0/Documents/research/openmc/src/random_lcg.cpp:60-64`
/// (declared at `include/openmc/random_lcg.h:50`). The C++ body is:
///
/// ```text
/// return future_seed(static_cast<uint64_t>(id) * prn_stride,
///                    master_seed + offset);
/// ```
///
/// **The mapping, stated plainly.** `id` is multiplied by the stride and that
/// product is the *jump-ahead distance*; `offset` is added to the **master
/// seed**, selecting a different starting point of the LCG orbit (OpenMC uses
/// it to give one particle several disjoint streams — `STREAM_TRACKING`,
/// `STREAM_SOURCE`, `STREAM_URR_PTABLE`, `STREAM_VOLUME`; see
/// `init_particle_seeds`, `random_lcg.cpp:70-76`). So consecutive `id`s are
/// [`DEFAULT_STRIDE`] draws apart, and a different `offset` is a different run
/// rather than a shift inside the same one.
///
/// **Signature deviation (deliberate, not a porting error).** OpenMC reads
/// `master_seed` from a mutable global (`random_lcg.cpp:8`, set via
/// `openmc_set_seed`). This port takes it as an explicit third parameter —
/// there is no global RNG state in this crate, so the caller passes it in. The
/// *semantics* are identical to upstream; only the plumbing of `master_seed`
/// differs. Likewise the stride is pinned to the compile-time
/// [`DEFAULT_STRIDE`] because this port has no `openmc_set_stride` equivalent
/// (upstream `prn_stride` is a mutable global, `random_lcg.cpp:13`).
///
/// **Wrapping.** `static_cast<uint64_t>(id) * prn_stride` is an unsigned
/// 64-bit multiply upstream, and `master_seed + offset` is an `int64_t` add
/// that is then reinterpreted as `uint64_t`. Both are reproduced with
/// `wrapping_*` so the result is identical in debug and release rather than
/// panicking on overflow.
///
/// # Parameters
///
/// - `id` — particle (or other stream) index; stream `k` starts
///   `k * DEFAULT_STRIDE` draws into the sequence.
/// - `offset` — stream selector, added to `master_seed`. Different values give
///   independent runs, not shifted views of one run.
/// - `master_seed` — the run's master seed (OpenMC's `DEFAULT_SEED` is `1`).
///
/// # Example
///
/// ```
/// use outram_mc_libs::rng::lcg::{init_seed, future_seed, DEFAULT_STRIDE};
/// // Consecutive ids are one full stride apart in the sequence.
/// assert_eq!(
///     init_seed(4, 0, 1),
///     future_seed(DEFAULT_STRIDE, init_seed(3, 0, 1))
/// );
/// ```
pub fn init_seed(id: i64, offset: i64, master_seed: i64) -> u64 {
    future_seed(
        (id as u64).wrapping_mul(DEFAULT_STRIDE),
        master_seed.wrapping_add(offset) as u64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================================
    // Statistical gates for the PCG-RXS-M-XS output permutation (bead op-jis,
    // added 2026-08-06).
    //
    // These are gates, not golden values. Per this crate's CLAUDE.md ("RNG
    // goal: statistical correctness, NOT particle-for-particle parity") the
    // permutation is justified by the *statistical* defect it removes, so every
    // gate below is run against BOTH the pre-op-jis output function
    // (`prn_legacy`, the raw top 52 state bits) and the current [`prn`], and
    // asserts the legacy one FAILS. A test that passes for both proves nothing
    // and would not be a gate.
    // =======================================================================

    /// The pre-`op-jis` output function: the raw top 52 bits of the LCG state,
    /// with no permutation. Kept **only** so the gates below can demonstrate
    /// that they actually discriminate. Not public, not used in transport.
    fn prn_legacy(seed: &mut u64) -> f64 {
        *seed = seed.wrapping_mul(MULT).wrapping_add(INC);
        (*seed >> 12) as f64 * (1.0 / (1u64 << 52) as f64)
    }

    /// Shortest nonzero vector of the 2-D integer lattice spanned by `u` and
    /// `v`, by Lagrange–Gauss reduction.
    ///
    /// Norms are accumulated in `f64` because the input basis vectors start out
    /// at ~2^64, whose exact square would overflow `i128`; the *reduced* vector
    /// has components of order `sqrt(2^64) = 2^32`, so the returned pair is
    /// exact. `f64` norms are only ever used for the comparisons that steer the
    /// reduction, which are not close calls here.
    fn shortest_lattice_vector(mut u: (i128, i128), mut v: (i128, i128)) -> (i128, i128) {
        let n2 = |w: (i128, i128)| (w.0 as f64) * (w.0 as f64) + (w.1 as f64) * (w.1 as f64);
        let dot = |a: (i128, i128), b: (i128, i128)| {
            (a.0 as f64) * (b.0 as f64) + (a.1 as f64) * (b.1 as f64)
        };
        if n2(u) > n2(v) {
            std::mem::swap(&mut u, &mut v);
        }
        loop {
            let mu = (dot(v, u) / n2(u)).round() as i128;
            let w = (v.0 - mu * u.0, v.1 - mu * u.1);
            if n2(w) >= n2(u) {
                return u;
            }
            v = u;
            u = w;
        }
    }

    /// Occupancy and chi-square of `t_n = frac(p * u_n + q * u_{n+lag})` over
    /// `bins` equal bins, plus the observed spread `max(t) - min(t)`.
    ///
    /// Each uniform is quantised to a 53-bit integer and the combination is
    /// formed in exact `i128` arithmetic modulo `2^53`; doing it in `f64` would
    /// lose the signal, because `p` and `q` are of order `2^32` and would eat
    /// the whole mantissa.
    fn hyperplane_stat(p: i128, q: i128, us: &[f64], lag: usize, bins: usize) -> (usize, f64, f64) {
        const S: i128 = 1i128 << 53;
        let mut occ = vec![0u64; bins];
        let (mut lo, mut hi) = (1.0f64, 0.0f64);
        for i in 0..us.len() - lag {
            let m0 = (us[i] * S as f64).round() as i128;
            let m1 = (us[i + lag] * S as f64).round() as i128;
            let t = (p * m0 + q * m1).rem_euclid(S) as f64 / S as f64;
            lo = lo.min(t);
            hi = hi.max(t);
            occ[((t * bins as f64) as usize).min(bins - 1)] += 1;
        }
        let n: u64 = occ.iter().sum();
        let e = n as f64 / bins as f64;
        let chi2 = occ
            .iter()
            .map(|&o| {
                let d = o as f64 - e;
                d * d / e
            })
            .sum();
        (occ.iter().filter(|&&o| o > 0).count(), chi2, hi - lo)
    }

    fn draw(f: fn(&mut u64) -> f64, seed0: u64, n: usize) -> Vec<f64> {
        let mut s = seed0;
        (0..n).map(|_| f(&mut s)).collect()
    }

    /// Chi-square of a `k`-cell equidistribution histogram in `dim` dimensions
    /// (consecutive draws consumed `dim` at a time).
    fn equidistribution_chi2(us: &[f64], dim: usize, k: usize) -> f64 {
        let cells = k.pow(dim as u32);
        let mut c = vec![0u64; cells];
        for w in us.chunks_exact(dim) {
            let mut idx = 0usize;
            for &x in w {
                idx = idx * k + ((x * k as f64) as usize).min(k - 1);
            }
            c[idx] += 1;
        }
        let n: u64 = c.iter().sum();
        let e = n as f64 / cells as f64;
        c.iter()
            .map(|&o| {
                let d = o as f64 - e;
                d * d / e
            })
            .sum()
    }

    /// Body shared by the lag-1 and lag-2 lattice gates.
    ///
    /// `mult_lag` is the effective LCG multiplier over `lag` steps (`a` for
    /// lag 1, `a^2` for lag 2 — composing an LCG with itself gives another LCG).
    fn assert_no_hyperplane_structure(lag: usize, mult_lag: u64) {
        const N: usize = 1 << 20;
        const BINS: usize = 1024;

        // Dual lattice of the pair (x_n, x_{n+lag}): the integer pairs (p, q)
        // with p + q*a_lag = 0 (mod 2^64), generated by (2^64, 0) and (-a, 1).
        // Its shortest vector is the normal of the densest hyperplane family,
        // i.e. exactly what the spectral test measures.
        let (p, q) = shortest_lattice_vector((1i128 << 64, 0), (-(mult_lag as i128), 1));

        let (occ_new, chi2_new, width_new) = hyperplane_stat(p, q, &draw(prn, 1, N), lag, BINS);
        let (occ_old, chi2_old, width_old) =
            hyperplane_stat(p, q, &draw(prn_legacy, 1, N), lag, BINS);

        // Discrimination check: the legacy generator MUST fail this gate.
        assert!(
            occ_old < BINS / 4,
            "lag {lag}: gate does not discriminate — the un-permuted LCG \
             occupied {occ_old}/{BINS} bins (chi2 {chi2_old:.4e}, width \
             {width_old:.3e}); it is supposed to collapse onto a hyperplane"
        );

        // The gate itself.
        assert_eq!(
            occ_new,
            BINS,
            "lag {lag}: permuted output left {} of {BINS} bins empty — \
             residual lattice structure (chi2 {chi2_new:.4e}, width {width_new:.3e})",
            BINS - occ_new
        );
        // chi2 on BINS-1 = 1023 dof: mean 1023, sd sqrt(2*1023) ~ 45.2.
        // A 5-sigma two-sided band is roughly [797, 1249].
        assert!(
            (797.0..1249.0).contains(&chi2_new),
            "lag {lag}: t-statistic is not uniform — chi2 = {chi2_new:.2} on \
             1023 dof, outside the 5-sigma band [797, 1249]"
        );
    }

    /// **Gate: no 2-D lattice (hyperplane) structure at lag 1.**
    ///
    /// **Methodology.** This is the spectral test, carried out directly rather
    /// than by binning the raw pairs. For a bare LCG `x_{n+1} = a*x_n + c mod
    /// 2^64`, the pairs `(x_n, x_{n+1})` lie on a lattice; the *dual* lattice
    /// `{ (p,q) : p + q*a = 0 (mod 2^64) }` has a shortest vector `(p,q)` that
    /// is the normal of the densest family of parallel hyperplanes covering
    /// them. Along that direction the structure is total: `p*x_n + q*x_{n+1} =
    /// q*c (mod 2^64)` for **every** n, exactly.
    ///
    /// So the test computes `(p,q)` by Lagrange–Gauss reduction of the basis
    /// `(2^64, 0), (-a, 1)`, draws 2^20 consecutive uniforms, forms
    /// `t_n = frac(p*u_n + q*u_{n+1})` in exact `i128` arithmetic (quantising
    /// each uniform to 53 bits), and histograms `t` into 1024 bins. A generator
    /// with no lattice spreads `t` uniformly; a generator on a lattice pins `t`
    /// to a single value. **Pass criterion:** all 1024 bins occupied and
    /// chi-square within 5 sigma of 1023 dof, i.e. `[797, 1249]`. The test also
    /// asserts the un-permuted generator FAILS, so it cannot silently stop
    /// discriminating.
    ///
    /// **Results (measured 2026-08-06, 2^20 draws from seed = 1).** The dual
    /// shortest vector is `(p, q) = (1381628436, 2627121436)`, `|v| = 2.968e9`.
    ///
    /// | generator | bins occupied | chi-square (1023 dof) | spread of `t` |
    /// |---|---|---|---|
    /// | legacy (top 52 state bits) | **1 / 1024** | **1.0727e9** | **8.810e-7** |
    /// | PCG-RXS-M-XS (current) | **1024 / 1024** | **1020.6** | 1.000 |
    ///
    /// **Interpretation.** The legacy output put all ~1.05e6 pairs inside a
    /// window of width 8.8e-7 of the unit interval — every pair the crate ever
    /// drew lay on one hyperplane family, which is the Marsaglia defect in its
    /// starkest form. After the permutation the same statistic is
    /// indistinguishable from uniform (chi-square 1020.6 against an expected 1023
    /// +- 45). The gate discriminates by a factor of ~1e6 in chi-square.
    #[test]
    fn lattice_structure_lag1() {
        assert_no_hyperplane_structure(1, MULT);
    }

    /// **Gate: no 2-D lattice structure at lag 2 either.**
    ///
    /// **Methodology.** As [`lattice_structure_lag1`], but on the pairs
    /// `(u_n, u_{n+2})`. Composing an LCG with itself gives another LCG with
    /// multiplier `a^2 (mod 2^64)`, so the same dual-lattice construction
    /// applies with `MULT.wrapping_mul(MULT)`. This rules out the lag-1 result
    /// being an artefact of one particular pairing — a transport history does
    /// not consume its draws in adjacent pairs only. **Pass criterion:**
    /// identical (1024/1024 bins, chi-square in `[797, 1249]`), plus the
    /// un-permuted generator must fail.
    ///
    /// **Results (measured 2026-08-06, 2^20 draws from seed = 1).** Dual
    /// shortest vector `(p, q) = (-1435621913, -843563855)`, `|v| = 1.665e9`.
    ///
    /// | generator | bins occupied | chi-square (1023 dof) | spread of `t` |
    /// |---|---|---|---|
    /// | legacy (top 52 state bits) | **1 / 1024** | **1.0727e9** | **4.895e-7** |
    /// | PCG-RXS-M-XS (current) | **1024 / 1024** | **1000.3** | 1.000 |
    ///
    /// **Interpretation.** The lattice was not a lag-1 artefact — the legacy
    /// generator collapsed just as completely at lag 2 (spread 4.9e-7). The
    /// permuted generator is uniform at both lags (chi-square 1000.3 vs an
    /// expected 1023 +- 45).
    #[test]
    fn lattice_structure_lag2() {
        assert_no_hyperplane_structure(2, MULT.wrapping_mul(MULT));
    }

    /// **Gate: a single output must not reveal the generator's internal state.**
    ///
    /// **Methodology.** The legacy output *was* the state: `u_n = (x_n >> 12) /
    /// 2^52`, so `u_n` pins 52 of the 64 state bits and only 12 are unknown.
    /// The attack is therefore trivial — take `u_0`, recover the 52 known bits,
    /// enumerate all 4096 completions of the low 12 bits, and for each candidate
    /// generate the next five outputs and compare against the true ones. If any
    /// candidate reproduces all five, the full internal state has been recovered
    /// from one observed sample and the entire future stream is predictable.
    ///
    /// This matters beyond cryptography: exact recoverability from one draw is
    /// the same property as "the outputs are an affine image of the orbit", the
    /// structural defect the lattice gates measure, expressed as predictability.
    /// **Pass criterion:** zero candidates survive for the current generator,
    /// and (discrimination check) exactly one survives for the legacy one.
    ///
    /// **Results (measured 2026-08-06, seed = 12345, 5 confirmation draws).**
    ///
    /// | generator | surviving candidates out of 4096 |
    /// |---|---|
    /// | legacy (top 52 state bits) | **1** — state fully recovered |
    /// | PCG-RXS-M-XS (current) | **0** |
    ///
    /// **Interpretation.** Under the legacy output function the state was
    /// recoverable from a single uniform with 4096 trial evaluations, i.e. no
    /// work at all. The permutation removes that: the top 52 bits of the
    /// returned double are no longer the top 52 bits of the state, so the
    /// reconstruction has nothing to hook into.
    #[test]
    fn output_does_not_expose_internal_state() {
        fn surviving_candidates(f: fn(&mut u64) -> f64, seed0: u64) -> usize {
            let observed = draw(f, seed0, 6);
            let hi = (observed[0] * (1u64 << 52) as f64) as u64;
            (0..4096u64)
                .filter(|lo| {
                    let mut cand = (hi << 12) | lo;
                    (1..6).all(|k| f(&mut cand) == observed[k])
                })
                .count()
        }

        let legacy = surviving_candidates(prn_legacy, 12345);
        assert_eq!(
            legacy, 1,
            "gate does not discriminate — the un-permuted LCG's state should be \
             recoverable from one output, but {legacy} candidates survived"
        );

        let current = surviving_candidates(prn, 12345);
        assert_eq!(
            current, 0,
            "{current} candidate state(s) reproduced five consecutive outputs — \
             the output permutation is not hiding the LCG state"
        );
    }

    /// **Regression guard (NOT a discriminating gate — stated plainly).**
    ///
    /// **Methodology.** Chi-square equidistribution of 2^20 draws from
    /// `seed = 1`, in 1-D over 1024 bins, in 2-D over a 64x64 grid of
    /// consecutive pairs, and in 3-D over a 16x16x16 grid of consecutive
    /// triples. Pass criterion: each chi-square within 5 sigma of its dof.
    ///
    /// **Results (measured 2026-08-06).**
    ///
    /// | test | dof | legacy | PCG-RXS-M-XS | 5-sigma band |
    /// |---|---|---|---|---|
    /// | 1-D, 1024 bins | 1023 | 961.6 | 1055.8 | [797, 1249] |
    /// | 2-D, 64x64 | 4095 | 4161.3 | 4052.1 | [3643, 4547] |
    /// | 3-D, 16x16x16 | 4095 | 4301.5 | 4025.5 | [3643, 4547] |
    ///
    /// **THIS GATE DOES NOT DISCRIMINATE, and that is the finding.** The
    /// un-permuted LCG passes every one of these comfortably — including the
    /// 3-D cell test, which was the candidate most likely to expose a lattice.
    /// The reason is that a full-period 64-bit LCG's hyperplane spacing is of
    /// order 2^-32, some seven orders of magnitude finer than a 16-cell-per-axis
    /// grid can resolve; the structure is real but invisible at this
    /// resolution. Only the *exact* dual-lattice projection in
    /// [`lattice_structure_lag1`] sees it.
    ///
    /// This test is retained as a cheap regression guard against a future
    /// change that breaks uniformity outright. It must not be cited as evidence
    /// that the output permutation improved anything.
    #[test]
    fn equidistribution_1d_2d_3d() {
        const N: usize = 1 << 20;
        let us = draw(prn, 1, N);

        let chi1 = equidistribution_chi2(&us, 1, 1024);
        assert!(
            (797.0..1249.0).contains(&chi1),
            "1-D equidistribution chi2 = {chi1:.2} on 1023 dof, outside [797, 1249]"
        );
        for (dim, k) in [(2usize, 64usize), (3, 16)] {
            let chi = equidistribution_chi2(&us, dim, k);
            assert!(
                (3643.0..4547.0).contains(&chi),
                "{dim}-D equidistribution chi2 = {chi:.2} on 4095 dof, outside [3643, 4547]"
            );
        }
    }

    /// **Regression guard (NOT a discriminating gate — stated plainly).**
    ///
    /// **Methodology.** Pearson serial autocorrelation of 2^20 draws from
    /// `seed = 1` at lags 1 through 8. For N = 2^20 the standard error of each
    /// coefficient under the null is `1/sqrt(N) = 9.77e-4`; the pass criterion
    /// is `|r| < 5e-3`, about 5 sigma.
    ///
    /// **Results (measured 2026-08-06), largest `|r|` over lags 1..=8:**
    ///
    /// | generator | max abs serial correlation | lag |
    /// |---|---|---|
    /// | legacy (top 52 state bits) | 2.14e-3 | 8 |
    /// | PCG-RXS-M-XS (current) | 2.06e-3 | 4 |
    ///
    /// **THIS GATE DOES NOT DISCRIMINATE either, and it is worth saying why.**
    /// Serial correlation is a *linear, one-dimensional* statistic; the LCG
    /// lattice is a *joint* property of tuples and is invisible to it. Both
    /// generators sit within ~2.2 standard errors of zero at every lag. Kept as
    /// a guard against gross correlation only.
    #[test]
    fn serial_correlation_lags_1_to_8() {
        const N: usize = 1 << 20;
        let us = draw(prn, 1, N);
        let mean: f64 = us.iter().sum::<f64>() / us.len() as f64;
        let var: f64 = us.iter().map(|u| (u - mean) * (u - mean)).sum();
        for lag in 1..=8usize {
            let cov: f64 = (0..us.len() - lag)
                .map(|i| (us[i] - mean) * (us[i + lag] - mean))
                .sum();
            let r = cov / var;
            assert!(
                r.abs() < 5e-3,
                "serial correlation at lag {lag} is r = {r:.3e} (|r| >= 5e-3)"
            );
        }
    }

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
        assert_eq!(
            s_seq, s_jump,
            "future_seed(1) must equal one sequential step"
        );
    }

    #[test]
    fn future_seed_n_steps() {
        let seed0 = 0xdeadbeef_u64;
        let mut s = seed0;
        for _ in 0..100 {
            prn(&mut s);
        }
        let s_jump = future_seed(100, seed0);
        assert_eq!(
            s, s_jump,
            "future_seed(100) must match 100 sequential steps"
        );
    }

    // -----------------------------------------------------------------------
    // `init_seed` regression tests (bead op-rbo, added 2026-08-06).
    //
    // The pre-fix implementation was
    //     future_seed((id + offset) as u64, future_seed(DEFAULT_STRIDE, master))
    // i.e. it ADDED `id` to the stride instead of MULTIPLYING by it. Every test
    // below fails against that implementation and passes against the corrected
    // port of `/home/teddy0/Documents/research/openmc/src/random_lcg.cpp:60`.
    // -----------------------------------------------------------------------

    /// **Methodology.** OpenMC's `init_seed` (`random_lcg.cpp:60`) jumps
    /// `id * prn_stride` steps, so stream `id+1` must start exactly
    /// `DEFAULT_STRIDE` (152917) LCG steps after stream `id`. Assert that
    /// directly against [`future_seed`] for several ids, and separately assert
    /// the streams are *not* one step apart — the specific defect being fixed.
    ///
    /// **Result (measured 2026-08-06).** Passes: for `id = 0,1,2,3,7,100`,
    /// `init_seed(id+1) == future_seed(152917, init_seed(id))` exactly, and
    /// `init_seed(id+1) != future_seed(1, init_seed(id))` in every case.
    /// Against the pre-fix implementation the first assertion failed at
    /// `id = 0` (`init_seed(1)` was one step, not 152917 steps, past
    /// `init_seed(0)`).
    #[test]
    fn init_seed_consecutive_ids_are_one_full_stride_apart() {
        let master = 1i64;
        for id in [0i64, 1, 2, 3, 7, 100] {
            let s_k = init_seed(id, 0, master);
            let s_k1 = init_seed(id + 1, 0, master);
            assert_eq!(
                s_k1,
                future_seed(DEFAULT_STRIDE, s_k),
                "init_seed({}) must be DEFAULT_STRIDE steps past init_seed({id})",
                id + 1
            );
            assert_ne!(
                s_k1,
                future_seed(1, s_k),
                "init_seed({}) must NOT be a single step past init_seed({id}) \
                 — that is the op-rbo defect",
                id + 1
            );
        }
    }

    /// **Methodology.** Golden values computed independently of this crate, by
    /// iterating OpenMC's exact state recurrence
    /// `x <- (6364136223846793005 * x + 1442695040888963407) mod 2^64`
    /// sequentially `id * 152917` times from `master_seed + offset` (a Python
    /// reference implementation of `random_lcg.cpp:32-44` + `:60-64`, i.e. no
    /// jump-ahead involved). Matching these pins the port numerically to
    /// upstream semantics, not merely to self-consistency.
    ///
    /// **Result (measured 2026-08-06).** All five golden values reproduced
    /// exactly:
    /// `init_seed(0,0,1) = 0x0000000000000001`,
    /// `init_seed(1,0,1) = 0x29C2CA2988433A80`,
    /// `init_seed(2,0,1) = 0x9F13A395D9D6F4C3`,
    /// `init_seed(1,1,1) = 0x4C4B503A910B70BD`,
    /// `init_seed(3,0,12345) = 0x5BB7F43433844CD2`.
    /// The pre-fix implementation matched **none** of the five — including the
    /// trivial `id = 0, offset = 0, master = 1` case, where it returned
    /// `future_seed(DEFAULT_STRIDE, 1) = 0x29C2CA2988433A80` instead of the
    /// correct `1` (measured 2026-08-06).
    #[test]
    fn init_seed_matches_openmc_golden_values() {
        assert_eq!(
            init_seed(0, 0, 1),
            0x0000_0000_0000_0001,
            "id=0,off=0,master=1"
        );
        assert_eq!(
            init_seed(1, 0, 1),
            0x29C2_CA29_8843_3A80,
            "id=1,off=0,master=1"
        );
        assert_eq!(
            init_seed(2, 0, 1),
            0x9F13_A395_D9D6_F4C3,
            "id=2,off=0,master=1"
        );
        assert_eq!(
            init_seed(1, 1, 1),
            0x4C4B_503A_910B_70BD,
            "id=1,off=1,master=1"
        );
        assert_eq!(
            init_seed(3, 0, 12345),
            0x5BB7_F434_3384_4CD2,
            "id=3,off=0,master=12345"
        );
    }

    /// **Methodology.** The property the op-rbo defect destroyed: two
    /// consecutive particle streams must not be shifted copies of each other.
    /// Draw 4096 uniforms from `init_seed(0,..)` and from `init_seed(1,..)`,
    /// then (a) check stream 1's leading value never appears anywhere in
    /// stream 0 — under the old "one step apart" seeding, stream 1 was exactly
    /// stream 0 dropped by one draw, so this collided immediately; (b) for
    /// every small lag `k` in `1..=8`, check the streams do not agree under
    /// that shift; (c) require the lag-0 Pearson correlation between the two
    /// streams to be small, `|r| < 0.05`.
    ///
    /// **Result (measured 2026-08-06, re-run under the `op-jis` PCG output
    /// permutation).** Passes. No value of stream 1 collides with stream 0's
    /// 4096 draws at any lag `0..=8`; the measured lag-0 Pearson correlation is
    /// `r = -0.02139` (|r| under the 0.05 gate). Against the pre-fix
    /// implementation, checks (a) and (b) both failed decisively:
    /// `stream1[j] == stream0[j+1]` for **4095 of 4095** draws, i.e. stream 1
    /// *was* stream 0 with a single draw dropped.
    ///
    /// **Supersedes** the `op-rbo` figures recorded earlier on 2026-08-06,
    /// which were measured with the pre-`op-jis` output function (raw top 52
    /// state bits): lag-0 Pearson `r = -0.00211` post-fix and `r = 0.00729`
    /// pre-fix. Those two numbers are stale; the correlation is a statistic of
    /// the *uniform values*, so it moved when the output function changed. The
    /// exact-match counts (4095/4095, and 0 at every lag) are properties of the
    /// integer state and did **not** move.
    ///
    /// **Caveat — check (c) alone would NOT have caught this bug.** The
    /// pre-fix implementation's lag-0 Pearson correlation was `r = 0.00729`
    /// under the then-current output function, comfortably inside the same 0.05
    /// gate: a one-draw shift of a good LCG is still lag-0 uncorrelated. The
    /// *exact-match at small lag* checks (a) and (b) are what have teeth here;
    /// (c) is retained only as a weak guard against gross correlation, and
    /// must not be mistaken for the load-bearing assertion.
    #[test]
    fn init_seed_streams_do_not_overlap() {
        const N: usize = 4096;
        let master = 1i64;

        let draw = |id: i64| -> Vec<f64> {
            let mut s = init_seed(id, 0, master);
            (0..N).map(|_| prn(&mut s)).collect()
        };
        let a = draw(0);
        let b = draw(1);

        // (a) stream 1's first draw must not occur anywhere in stream 0.
        assert!(
            !a.contains(&b[0]),
            "stream(id=1) starts inside stream(id=0) — streams are not independent"
        );

        // (b) no small-lag shift reproduces one stream from the other.
        for lag in 1..=8usize {
            let matches = (0..N - lag).filter(|&j| a[j + lag] == b[j]).count();
            assert!(
                matches < 4,
                "stream(id=1) reproduces stream(id=0) shifted by {lag} \
                 ({matches}/{} draws matched)",
                N - lag
            );
        }

        // (c) lag-0 correlation must be negligible.
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let (ma, mb) = (mean(&a), mean(&b));
        let cov: f64 = a.iter().zip(&b).map(|(x, y)| (x - ma) * (y - mb)).sum();
        let va: f64 = a.iter().map(|x| (x - ma).powi(2)).sum();
        let vb: f64 = b.iter().map(|y| (y - mb).powi(2)).sum();
        let r = cov / (va.sqrt() * vb.sqrt());
        assert!(
            r.abs() < 0.05,
            "consecutive-id streams are correlated: Pearson r = {r:.5}"
        );
    }

    /// **Methodology.** Upstream adds `offset` to the **master seed**
    /// (`random_lcg.cpp:63`), which selects a different run of the generator;
    /// the pre-fix port added it to the **id**, which merely slid along the
    /// same sequence. Assert the upstream identity
    /// `init_seed(id, offset, master) == init_seed(id, 0, master + offset)`,
    /// assert the buggy identity `init_seed(id, offset, m) ==
    /// init_seed(id + offset, 0, m)` does *not* hold, and check that the four
    /// offsets OpenMC uses for its `N_STREAMS = 4` particle streams
    /// (`random_lcg.h:12-16`, `init_particle_seeds`) give four distinct,
    /// mutually non-overlapping seeds.
    ///
    /// **Result (measured 2026-08-06).** Passes. The upstream identity holds
    /// for all tested `(id, offset)`; the buggy identity holds for none; the
    /// four stream seeds for `id = 42` are pairwise distinct and none is
    /// within 64 sequential steps of another (checked exhaustively).
    #[test]
    fn init_seed_offset_selects_an_independent_run() {
        let master = 1i64;

        for id in [0i64, 1, 5, 99] {
            for offset in [1i64, 2, 3] {
                assert_eq!(
                    init_seed(id, offset, master),
                    init_seed(id, 0, master + offset),
                    "offset must shift the MASTER SEED (id={id}, offset={offset})"
                );
                assert_ne!(
                    init_seed(id, offset, master),
                    init_seed(id + offset, 0, master),
                    "offset must NOT be equivalent to bumping the id \
                     (id={id}, offset={offset}) — that is the op-rbo defect"
                );
            }
        }

        // OpenMC's N_STREAMS = 4 per-particle streams must be disjoint.
        let id = 42i64;
        let seeds: Vec<u64> = (0..4).map(|s| init_seed(id, s, master)).collect();
        for i in 0..4 {
            for j in 0..4 {
                if i == j {
                    continue;
                }
                assert_ne!(seeds[i], seeds[j], "streams {i} and {j} share a seed");
                let mut s = seeds[i];
                for step in 1..=64u32 {
                    prn(&mut s);
                    assert_ne!(
                        s, seeds[j],
                        "stream {j} starts only {step} draws into stream {i}"
                    );
                }
            }
        }
    }
}
