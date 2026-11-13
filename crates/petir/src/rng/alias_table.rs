// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// ---------------------------------------------------------------------------
// PORTED (NOT from GSL) from EGSnrc (National Research Council Canada):
//
//   HEN_HOUSE/egs++/egs_alias_table.h    (EGS_AliasTable, EGS_SimpleAliasTable)
//   HEN_HOUSE/egs++/egs_alias_table.cpp  (make, initialize, sample, sampleBin,
//                                          EGS_SimpleAliasTable constructor)
//   HEN_HOUSE/egs++/egs_functions.h      (the constants `epsilon`, `loopMax`)
//
// Read at commit bd49a47aae3938f49f4100b8e018091861408b19 (2026-09-28).
// Copyright lines as upstream writes them:
//
//   egs_alias_table.h:   EGSnrc egs++ alias table headers
//                        Copyright (C) 2015 National Research Council Canada
//                        Author: Iwan Kawrakow, 2005
//                        Contributors: Ernesto Mainegra-Hing, Marc Chamberland
//   egs_alias_table.cpp: EGSnrc egs++ alias table
//                        Copyright (C) 2015 National Research Council Canada
//                        Author: Iwan Kawrakow, 2005
//                        Contributors: Ernesto Mainegra-Hing, Frederic Tessier,
//                        Blake Walters, Reid Townson
//
// Licence: GNU Affero General Public License, version 3 or (at your option)
// any later version (AGPL-3.0-or-later). This port is AGPL-3.0-only, which
// exercises the "or later" option by choosing version 3. See NOTICE.
// Issue: gh:#1289.
// ---------------------------------------------------------------------------

//! **Walker alias tables**, ported from EGSnrc's `egs_alias_table`.
//!
//! Two classes are ported, both with their construction and their sampling:
//!
//! | upstream | here | samples |
//! |---|---|---|
//! | `EGS_AliasTable`, type 0 | [`EgsAliasTable`] with [`AliasTableKind::Discrete`] | one of `N` discrete abscissas |
//! | `EGS_AliasTable`, type 1 | [`AliasTableKind::Histogram`] | a bin, then uniformly within it |
//! | `EGS_AliasTable`, type 2 (and 3) | [`AliasTableKind::LinearInterpolation`] | a bin, then from the linear density across it |
//! | `EGS_AliasTable::initialize(xmin, xmax, accu, nmax, func, data)` | [`EgsAliasTable::from_function`] | type 2, built adaptively from a function |
//! | `EGS_AliasTable::sampleBin` | [`EgsAliasTable::sample_bin`] | the bin index only |
//! | `EGS_SimpleAliasTable` | [`EgsSimpleAliasTable`] | a bin index, two uniforms per draw |
//!
//! # The random numbers
//!
//! Upstream draws from `EGS_RandomGenerator::getUniform()`, a `double` in
//! `[0, 1)`. Here every sampler takes PETIR's [`Rng`] trait and draws
//! `rng.random::<f64>()` ([`super::RngExt`]: the top 53 bits of one
//! `next_u64`, times `2^-53`), in the same order and the same number of times
//! as upstream calls `getUniform()`. Given the same uniforms, the samples are
//! bit-identical to upstream's; `verification_and_validation/egsnrc_alias_table/`
//! shows it against upstream compiled from source.
//!
//! **Consequence for a caller wanting EGSnrc's exact streams:** a generator
//! whose `getUniform()` is not of the form `k * 2^-53` (RANMAR's are
//! multiples of `2^-24`, which are) reproduces EGSnrc only if its [`Rng`]
//! implementation returns `next_u64() = k << 11` for the uniform `k * 2^-53`.
//!
//! # Deviations from upstream (none changes a result for valid input)
//!
//! 1. **Memory:** `allocate`, `clear`, `copy` and the copy constructor are
//!    `Vec` and `#[derive(Clone)]`. The function-driven `initialize`'s
//!    `xtemp`/`ftemp` leak on the `is_ok` path upstream is gone.
//! 2. **Errors instead of undefined behaviour.** Upstream does not check `N`;
//!    `N < 1` (discrete) or `N < 2` (continuous) leads to out-of-bounds reads
//!    when sampling. Those return [`PetirError::Domain`], as does a too-short
//!    `f`. A type other than 0..=3 is [`PetirError::Invalid`] (upstream would
//!    read an uninitialised `fi[np]`). `egsFatal` calls (simple table with
//!    `sum <= 0`; `loopMax` reached) return errors rather than exiting.
//! 3. **Warnings are flags.** `egsWarning(...)` in `make()` and in the simple
//!    table's constructor set [`EgsAliasTable::aliasing_incomplete`] /
//!    [`EgsSimpleAliasTable::aliasing_incomplete`]; `no_std` has nowhere to
//!    print.
//! 4. **The bin index is clamped to `np - 1`.** Upstream's `(int)(r1*np)`
//!    overruns the arrays only if `getUniform()` returns `>= 1`, which
//!    `random::<f64>()` cannot (its maximum `1 - 2^-53` times any `np` rounds
//!    below `np`). The clamp is there so no subscript can fail
//!    (`tests/no_panic_gate.rs`), not to change a sample.
//! 5. **Type 3** is accepted by [`AliasTableKind::from_egs_type`] and mapped
//!    to type 2: upstream's `initialize`/`copy` treat 2 and 3 alike and `make`
//!    and `sample` branch only on 0 and 1, so the two are the same table.

#[allow(unused_imports)]
use crate::real::Real;
use crate::{PetirError, Result};
use alloc::vec::Vec;

use super::traits::{Rng, RngExt};

/// `egs_functions.h:60-63`, double branch: `const EGS_Float epsilon =
/// 1.0/(1ULL<<50);`, "4 times the machine epsilon".
pub const EGS_EPSILON: f64 = 1.0 / ((1u64 << 50) as f64);

/// `egs_functions.h:96`: `const EGS_I64 loopMax = 1e10;`, the iteration
/// guard EGSnrc puts on its otherwise unbounded loops.
pub const EGS_LOOP_MAX: i64 = 10_000_000_000;

/// `egs_alias_table.cpp:189`: `#define AT_NCHECK 3`, the number of interior
/// points per interval at which the function-driven `initialize` tests the
/// linear interpolation.
const AT_NCHECK: usize = 3;

/// How an [`EgsAliasTable`] reads its `(x, f)` input — upstream's `type`.
///
/// **Upstream:** `egs_alias_table.h:85-96` (constructor documentation) and
/// the `type` member comment: "0 => sum of delta functions, 1 => histogram,
/// 2 => linear interpolation between bin edges".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliasTableKind {
    /// Type 0: `sum_i f_i delta(x - x_i)`, `N` points, `N` probabilities.
    Discrete,
    /// Type 1: `N` edges, `N - 1` bins, bin `i` on `[x_i, x_{i+1})` with
    /// density `f_i`.
    Histogram,
    /// Type 2: `N` edges, a density varying linearly from `f_i` at `x_i` to
    /// `f_{i+1}` at `x_{i+1}`.
    LinearInterpolation,
}

impl AliasTableKind {
    /// Map upstream's integer `type` to the enum.
    ///
    /// `0`, `1` and `2` map one to one; `3` maps to
    /// [`AliasTableKind::LinearInterpolation`] (module docs, deviation 5).
    ///
    /// # Errors
    /// [`PetirError::Invalid`] for any other value.
    pub fn from_egs_type(egs_type: i32) -> Result<Self> {
        match egs_type {
            0 => Ok(AliasTableKind::Discrete),
            1 => Ok(AliasTableKind::Histogram),
            2 | 3 => Ok(AliasTableKind::LinearInterpolation),
            _ => Err(PetirError::Invalid),
        }
    }

    /// Upstream's integer `type` for this kind (`3` is reported as `2`).
    pub fn egs_type(self) -> i32 {
        match self {
            AliasTableKind::Discrete => 0,
            AliasTableKind::Histogram => 1,
            AliasTableKind::LinearInterpolation => 2,
        }
    }
}

/// Whether the function-driven constructor met its accuracy goal — the
/// integer upstream's `initialize(xmin, xmax, accu, nmax, ...)` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccuracyGoal {
    /// Upstream return `0`: every check point is within `accu`.
    Met,
    /// Upstream return `1`: the table reached `nmax` points first. The table
    /// is still built and usable.
    NotMetAtNmax,
}

/// The alias-table data `make()` produces from `(x, f)`.
///
/// Returned by [`egs_alias_make`]; [`EgsAliasTable`] wraps it.
#[derive(Clone, Debug, PartialEq)]
pub struct AliasMade {
    /// `fi`, divided by the total probability (`fi[i] /= sum`).
    pub fi: Vec<f64>,
    /// `wi`: the probability of keeping bin `j` rather than its alias.
    pub wi: Vec<f64>,
    /// `bin`: the alias of each bin.
    pub bin: Vec<usize>,
    /// `average`: the mean of the distribution.
    pub average: f64,
    /// `true` where `make()` issued "found a high bin, but no low bin".
    pub incomplete: bool,
}

/// Build the alias table — `EGS_AliasTable::make()`.
///
/// `xi` holds the `N` abscissas (discrete) or edges; `fi` holds `np = N`
/// (discrete), `np = N - 1` (histogram) or `N` (linear) values. The caller
/// ([`EgsAliasTable::new`]) checks those lengths.
///
/// **Upstream:** `egs_alias_table.cpp:116-187`, line for line, in the same
/// operation order (the sums are accumulated bin by bin; the pairing loop
/// takes the FIRST not-done high bin and the FIRST not-done low bin, which is
/// what makes the table, and therefore every sample, match bit for bit).
pub fn egs_alias_make(kind: AliasTableKind, xi: &[f64], fi: &[f64]) -> AliasMade {
    // Lines 117-143: fcum, the running sum and the first moment.
    let mut fcum: Vec<f64> = Vec::new();
    let mut sum = 0.0_f64;
    let mut sum1 = 0.0_f64;
    match kind {
        AliasTableKind::Discrete => {
            for (&x, &f) in xi.iter().zip(fi.iter()) {
                let c = f;
                fcum.push(c);
                sum += c;
                sum1 += c * x;
            }
        }
        AliasTableKind::Histogram => {
            for (xw, &f) in xi.windows(2).zip(fi.iter()) {
                if let [x0, x1] = *xw {
                    let c = f * (x1 - x0);
                    fcum.push(c);
                    sum += c;
                    sum1 += 0.5 * c * (x1 + x0);
                }
            }
        }
        AliasTableKind::LinearInterpolation => {
            for (xw, fw) in xi.windows(2).zip(fi.windows(2)) {
                if let (&[x0, x1], &[f0, f1]) = (xw, fw) {
                    let c = 0.5 * (f0 + f1) * (x1 - x0);
                    fcum.push(c);
                    sum += c;
                    // Line 141-142: fcum[i]*(...)/(3*(...)), left to right.
                    sum1 += c * (f0 * (2.0 * x0 + x1) + f1 * (x0 + 2.0 * x1)) / (3.0 * (f0 + f1));
                }
            }
        }
    }
    let np = fcum.len();
    let average = sum1 / sum;

    // Lines 146-151: normalise every stored f (fi[0..np], plus fi[np] for
    // types 2 and 3 -- that is all of `fi` in each case).
    let fi_norm: Vec<f64> = fi.iter().map(|&f| f / sum).collect();
    // Line 152: `sum /= np;`, the average bin weight.
    let sum = sum / (np as f64);

    // Lines 131-133: wi = 1, bin = 0, not_done = true.
    let mut wi: Vec<f64> = alloc::vec![1.0; np];
    let mut bin: Vec<usize> = alloc::vec![0; np];
    let mut not_done: Vec<bool> = alloc::vec![true; np];
    let mut incomplete = false;

    // Lines 155-185.
    for _ in 0..np.saturating_sub(1) {
        let high_bin = not_done
            .iter()
            .zip(fcum.iter())
            .position(|(&nd, &c)| nd && c > sum);
        let Some(high_bin) = high_bin else {
            break;
        };
        let low_bin = not_done
            .iter()
            .zip(fcum.iter())
            .position(|(&nd, &c)| nd && c < sum);
        let Some(low_bin) = low_bin else {
            // egsWarning("EGS_AliasTable::make(): found a high bin, but no
            // low bin; this is abnormal.");
            incomplete = true;
            break;
        };
        let Some(&f_low) = fcum.get(low_bin) else {
            break;
        };
        let aux = sum - f_low;
        if let Some(f_high) = fcum.get_mut(high_bin) {
            *f_high -= aux;
        }
        // Upstream: `not_done[jl] = false;` -- jl is low_bin after the break.
        if let Some(nd) = not_done.get_mut(low_bin) {
            *nd = false;
        }
        if let Some(w) = wi.get_mut(low_bin) {
            *w = f_low / sum;
        }
        if let Some(b) = bin.get_mut(low_bin) {
            *b = high_bin;
        }
    }

    AliasMade {
        fi: fi_norm,
        wi,
        bin,
        average,
        incomplete,
    }
}

/// Pick a bin from one uniform — the first five lines of upstream's
/// `sampleBin` and `sample`.
///
/// **Upstream:** `egs_alias_table.cpp:256-265`:
/// `aj = r1*np; j = (int) aj; aj -= j; if (aj > wi[j]) j = bin[j];`.
/// `j` is clamped to `np - 1` (module docs, deviation 4).
pub fn egs_alias_pick(wi: &[f64], bin: &[usize], r1: f64) -> usize {
    let np = wi.len();
    let mut aj = r1 * (np as f64);
    let mut j = (aj as usize).min(np.saturating_sub(1));
    aj -= j as f64;
    if let Some(&w) = wi.get(j) {
        if aj > w {
            if let Some(&b) = bin.get(j) {
                j = b;
            }
        }
    }
    j
}

/// The value within bin `j`, from the second uniform `r2` — the rest of
/// upstream's `sample` for types 1 and 2.
///
/// `xi` and `fi` are the table's stored (normalised) arrays. For
/// [`AliasTableKind::Discrete`] the value is `xi[j]` and `r2` is not used
/// (upstream does not draw it).
///
/// **Upstream:** `egs_alias_table.cpp:268-294`, with the same grouping:
/// `x + r2*dx*(1+rnno1*(1-r2*a))` and `x - dx/a*(1-sqrt(1+r2*a*(2+a)))`.
/// An index miss cannot happen for a table built by [`EgsAliasTable`];
/// it returns NaN, as `rng::normal` does for its tables.
pub fn egs_alias_value(kind: AliasTableKind, xi: &[f64], fi: &[f64], j: usize, r2: f64) -> f64 {
    let Some(&x) = xi.get(j) else {
        return f64::NAN;
    };
    if kind == AliasTableKind::Discrete {
        return x;
    }
    let Some(&x1) = xi.get(j + 1) else {
        return f64::NAN;
    };
    let dx = x1 - x;
    if kind == AliasTableKind::Histogram {
        return x + dx * r2;
    }
    let (Some(&f0), Some(&f1)) = (fi.get(j), fi.get(j + 1)) else {
        return f64::NAN;
    };
    if f0 > 0.0 {
        let a = f1 / f0 - 1.0;
        if a.abs() < 0.2 {
            let rnno1 = 0.5 * (1.0 - r2) * a;
            x + r2 * dx * (1.0 + rnno1 * (1.0 - r2 * a))
        } else {
            x - dx / a * (1.0 - Real::sqrt(1.0 + r2 * a * (2.0 + a)))
        }
    } else {
        x + dx * Real::sqrt(r2)
    }
}

/// A Walker alias table over a discrete, histogram or piecewise-linear
/// distribution — EGSnrc's `EGS_AliasTable`.
///
/// Build with [`EgsAliasTable::new`] (tabulated) or
/// [`EgsAliasTable::from_function`] (adaptive, from a function); draw with
/// [`sample`](EgsAliasTable::sample) or
/// [`sample_bin`](EgsAliasTable::sample_bin).
///
/// ```
/// use petir::rng::{AliasTableKind, ChaCha8Rng, EgsAliasTable, SeedableRng};
/// let t = EgsAliasTable::new(&[1.0, 2.0, 3.0], &[1.0, 2.0, 1.0], AliasTableKind::Discrete)?;
/// assert_eq!(t.average(), 2.0);
/// let mut rng = ChaCha8Rng::seed_from_u64(1);
/// let x = t.sample(&mut rng);
/// assert!(x == 1.0 || x == 2.0 || x == 3.0);
/// # Ok::<(), petir::PetirError>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct EgsAliasTable {
    kind: AliasTableKind,
    xi: Vec<f64>,
    fi: Vec<f64>,
    wi: Vec<f64>,
    bin: Vec<usize>,
    average: f64,
    incomplete: bool,
}

impl EgsAliasTable {
    /// Build from `N = x.len()` tabulated points — upstream's
    /// `EGS_AliasTable(int N, const EGS_Float *x, const EGS_Float *f, int Type)`
    /// and `initialize` (`egs_alias_table.cpp:101-114`), then `make()`.
    ///
    /// `f` must hold at least `N` values (discrete, linear) or `N - 1`
    /// (histogram); extra values are ignored, as upstream reads only those.
    ///
    /// # Errors
    /// [`PetirError::Domain`] for `N < 1` (discrete), `N < 2` (histogram,
    /// linear), or `f` shorter than required.
    pub fn new(x: &[f64], f: &[f64], kind: AliasTableKind) -> Result<Self> {
        let n = x.len();
        let nf = match kind {
            AliasTableKind::Discrete => n,
            AliasTableKind::Histogram => n.saturating_sub(1),
            AliasTableKind::LinearInterpolation => n,
        };
        let min_n = match kind {
            AliasTableKind::Discrete => 1,
            _ => 2,
        };
        if n < min_n || f.len() < nf {
            return Err(PetirError::Domain);
        }
        let fi: Vec<f64> = f.iter().take(nf).copied().collect();
        Ok(Self::from_parts(kind, x.to_vec(), &fi))
    }

    /// Run `make()` on stored `xi`, `fi`.
    fn from_parts(kind: AliasTableKind, xi: Vec<f64>, fi: &[f64]) -> Self {
        let made = egs_alias_make(kind, &xi, fi);
        EgsAliasTable {
            kind,
            xi,
            fi: made.fi,
            wi: made.wi,
            bin: made.bin,
            average: made.average,
            incomplete: made.incomplete,
        }
    }

    /// Build a [`AliasTableKind::LinearInterpolation`] table for `func` on
    /// `[xmin, xmax]`, refining until linear interpolation is within `accu`
    /// (relative) at `AT_NCHECK = 3` interior points per interval, or until
    /// the table has `nmax` points.
    ///
    /// **Upstream:** `egs_alias_table.cpp:191-254`,
    /// `initialize(xmin, xmax, accu, nmax, func, data)`. `func` replaces the
    /// function pointer and its `void *data`. Every point that fails the test
    /// is inserted, every interval in one pass, and the pass repeats.
    ///
    /// # Errors
    /// [`PetirError::MaxIterations`] where upstream calls `egsFatal` after
    /// `loopMax` passes.
    pub fn from_function<F: FnMut(f64) -> f64>(
        xmin: f64,
        xmax: f64,
        accu: f64,
        nmax: usize,
        mut func: F,
    ) -> Result<(Self, AccuracyGoal)> {
        // Lines 193-197: allocate(2,2) with the end points.
        let mut xi: Vec<f64> = alloc::vec![xmin, xmax];
        let f_lo = func(xmin);
        let f_hi = func(xmax);
        let mut fi: Vec<f64> = alloc::vec![f_lo, f_hi];
        let mut goal = AccuracyGoal::Met;
        let mut loop_count: i64 = 0;
        loop {
            if loop_count == EGS_LOOP_MAX {
                return Err(PetirError::MaxIterations);
            }
            let n = xi.len();
            let mut xtemp: Vec<f64> = Vec::with_capacity((n - 1) * AT_NCHECK + n);
            let mut ftemp: Vec<f64> = Vec::with_capacity((n - 1) * AT_NCHECK + n);
            let mut is_ok = true;
            // Lines 210-226.
            for (xw, fw) in xi.windows(2).zip(fi.windows(2)) {
                if let (&[x0, x1], &[f0, f1]) = (xw, fw) {
                    xtemp.push(x0);
                    ftemp.push(f0);
                    let dx = (x1 - x0) / ((AT_NCHECK + 1) as f64);
                    for l in 0..AT_NCHECK {
                        let x = x0 + dx * ((l + 1) as f64);
                        let fe = func(x);
                        let fa = f0 + (f1 - f0) / (x1 - x0) * (x - x0);
                        let test = (fa / fe - 1.0).abs();
                        if test > accu {
                            is_ok = false;
                            xtemp.push(x);
                            ftemp.push(fe);
                        }
                    }
                }
            }
            if is_ok {
                break;
            }
            // Lines 230-236: the last edge, then the refined table.
            if let (Some(&xl), Some(&fl)) = (xi.last(), fi.last()) {
                xtemp.push(xl);
                ftemp.push(fl);
            }
            xi = xtemp;
            fi = ftemp;
            if xi.len() >= nmax {
                goal = AccuracyGoal::NotMetAtNmax;
                break;
            }
            loop_count += 1;
        }
        Ok((
            Self::from_parts(AliasTableKind::LinearInterpolation, xi, &fi),
            goal,
        ))
    }

    /// Draw a value — upstream's `sample(EGS_RandomGenerator *rndm)`.
    ///
    /// One uniform picks the bin; for histogram and linear tables a second
    /// uniform places the value within it (`egs_alias_table.cpp:256-295`).
    pub fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        let r1: f64 = rng.random::<f64>();
        let j = egs_alias_pick(&self.wi, &self.bin, r1);
        if self.kind == AliasTableKind::Discrete {
            return egs_alias_value(self.kind, &self.xi, &self.fi, j, 0.0);
        }
        let r2: f64 = rng.random::<f64>();
        egs_alias_value(self.kind, &self.xi, &self.fi, j, r2)
    }

    /// Draw a bin index — upstream's `sampleBin` (`egs_alias_table.cpp:256-265`).
    /// One uniform per draw.
    pub fn sample_bin<R: Rng + ?Sized>(&self, rng: &mut R) -> usize {
        let r1: f64 = rng.random::<f64>();
        egs_alias_pick(&self.wi, &self.bin, r1)
    }

    /// The distribution's mean — `getAverage()`.
    ///
    /// Note (upstream behaviour, kept): a linear table with a bin whose two
    /// edge values are both zero has `0 * (...) / 0` in that bin's moment,
    /// so the average is NaN even though sampling is fine.
    pub fn average(&self) -> f64 {
        self.average
    }

    /// The largest abscissa — `getMaximum()`, `xi[n-1]`.
    pub fn maximum(&self) -> f64 {
        self.xi.last().copied().unwrap_or(f64::NAN)
    }

    /// The table's kind (upstream `type`).
    pub fn kind(&self) -> AliasTableKind {
        self.kind
    }

    /// The abscissas or bin edges, `xi` (`N` entries).
    pub fn xi(&self) -> &[f64] {
        &self.xi
    }

    /// The normalised function values, `fi`.
    pub fn fi(&self) -> &[f64] {
        &self.fi
    }

    /// The keep probabilities, `wi` (`np` entries).
    pub fn wi(&self) -> &[f64] {
        &self.wi
    }

    /// The aliases, `bin` (`np` entries).
    pub fn bins(&self) -> &[usize] {
        &self.bin
    }

    /// `true` if `make()` stopped on "found a high bin, but no low bin"
    /// (upstream: `egsWarning`).
    pub fn aliasing_incomplete(&self) -> bool {
        self.incomplete
    }
}

/// A Walker alias table over bin indices only — EGSnrc's
/// `EGS_SimpleAliasTable` (used upstream by source and shape collections and
/// voxelised shapes).
///
/// Its construction is the stack ("Vose") form, different from
/// [`EgsAliasTable`]'s first-high/first-low scan, and each draw takes TWO
/// uniforms: one for the bin, one for the keep test.
#[derive(Clone, Debug, PartialEq)]
pub struct EgsSimpleAliasTable {
    wi: Vec<f64>,
    bins: Vec<usize>,
    incomplete: bool,
}

impl EgsSimpleAliasTable {
    /// Build from bin probabilities `f` (need not be normalised).
    ///
    /// **Upstream:** `egs_alias_table.cpp:298-366`, line for line: normalise,
    /// `wi = p*n`, split into `small` (`wi <= 1`) and `big` lists, then pair
    /// the LAST of each (`vector::back`), moving a big bin to the small list
    /// once `wi[big] < 1 + epsilon`.
    ///
    /// # Errors
    /// [`PetirError::Domain`] for an empty `f` (upstream returns with
    /// `n = 0`, after which `sample` reads out of bounds) and for
    /// `sum(f) <= 0` (upstream: `egsFatal`, "degenerate distribution").
    /// [`PetirError::MaxIterations`] is unreachable in practice (each pass
    /// pops a small bin) but stands for upstream's `loopMax` guard.
    pub fn new(f: &[f64]) -> Result<Self> {
        let n = f.len();
        if n < 1 {
            return Err(PetirError::Domain);
        }
        // Lines 314-319.
        let mut bins: Vec<usize> = (0..n).collect();
        let mut sum = 0.0_f64;
        for &v in f {
            sum += v;
        }
        // Lines 322-329.
        if sum <= 0.0 {
            return Err(PetirError::Domain);
        }
        let nf = n as f64;
        // Lines 332-342: wi = (f/sum)*n, and the two lists.
        let mut wi: Vec<f64> = Vec::with_capacity(n);
        let mut big_list: Vec<usize> = Vec::new();
        let mut small_list: Vec<usize> = Vec::new();
        for (i, &v) in f.iter().enumerate() {
            let p = v / sum;
            let w = p * nf;
            wi.push(w);
            if w <= 1.0 {
                small_list.push(i);
            } else {
                big_list.push(i);
            }
        }
        // Lines 345-359.
        let mut loop_count: i64 = 0;
        while let (Some(&big), Some(&small)) = (big_list.last(), small_list.last()) {
            let count = loop_count;
            loop_count += 1;
            if count > EGS_LOOP_MAX {
                break;
            }
            if let Some(b) = bins.get_mut(small) {
                *b = big;
            }
            let w_small = wi.get(small).copied().unwrap_or(f64::NAN);
            let mut now_small = false;
            if let Some(w_big) = wi.get_mut(big) {
                *w_big -= 1.0 - w_small;
                now_small = *w_big < 1.0 + EGS_EPSILON;
            }
            small_list.pop();
            if now_small {
                big_list.pop();
                small_list.push(big);
            }
        }
        // Lines 360-362: egsWarning "table aliasing may be incomplete".
        let incomplete = !big_list.is_empty();
        Ok(EgsSimpleAliasTable {
            wi,
            bins,
            incomplete,
        })
    }

    /// Draw a bin — upstream's inline `sample` (`egs_alias_table.h:194-197`):
    /// `bin = (int)(u1*n); return u2 < wi[bin] ? bin : bins[bin];`.
    /// The bin is clamped to `n - 1` (module docs, deviation 4).
    pub fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> usize {
        let u1: f64 = rng.random::<f64>();
        let n = self.wi.len();
        let bin = ((u1 * (n as f64)) as usize).min(n.saturating_sub(1));
        let u2: f64 = rng.random::<f64>();
        match (self.wi.get(bin), self.bins.get(bin)) {
            (Some(&w), Some(&alias)) => {
                if u2 < w {
                    bin
                } else {
                    alias
                }
            }
            _ => bin,
        }
    }

    /// The keep probabilities, `wi` (`n` entries, scaled so the mean is 1).
    pub fn wi(&self) -> &[f64] {
        &self.wi
    }

    /// The aliases, `bins` (`n` entries; a bin never aliased points to
    /// itself).
    pub fn bins(&self) -> &[usize] {
        &self.bins
    }

    /// `true` where upstream warns "table aliasing may be incomplete".
    pub fn aliasing_incomplete(&self) -> bool {
        self.incomplete
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replays a fixed list of uniforms `k * 2^-53`, as `next_u64 = k << 11`.
    struct Replay {
        k: Vec<u64>,
        pos: usize,
    }
    impl Rng for Replay {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }
        fn next_u64(&mut self) -> u64 {
            let k = self.k[self.pos % self.k.len()];
            self.pos += 1;
            k << 11
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for b in dest {
                *b = self.next_u64() as u8;
            }
        }
    }

    #[test]
    fn kind_round_trips_and_rejects_unknown_types() {
        for t in 0..3 {
            assert_eq!(AliasTableKind::from_egs_type(t).unwrap().egs_type(), t);
        }
        assert_eq!(
            AliasTableKind::from_egs_type(3).unwrap(),
            AliasTableKind::LinearInterpolation
        );
        assert_eq!(AliasTableKind::from_egs_type(4), Err(PetirError::Invalid));
        assert_eq!(AliasTableKind::from_egs_type(-1), Err(PetirError::Invalid));
    }

    #[test]
    fn too_few_points_or_values_is_a_domain_error() {
        use AliasTableKind::*;
        assert_eq!(
            EgsAliasTable::new(&[], &[], Discrete),
            Err(PetirError::Domain)
        );
        assert_eq!(
            EgsAliasTable::new(&[1.0], &[1.0], Histogram),
            Err(PetirError::Domain)
        );
        assert_eq!(
            EgsAliasTable::new(&[0.0, 1.0], &[1.0], LinearInterpolation),
            Err(PetirError::Domain)
        );
        assert_eq!(EgsSimpleAliasTable::new(&[]), Err(PetirError::Domain));
        assert_eq!(
            EgsSimpleAliasTable::new(&[0.0, 0.0]),
            Err(PetirError::Domain)
        );
    }

    /// A hand-worked discrete table: f = (1, 2, 1) on x = (1, 2, 3).
    /// sum = 4, mean bin weight 4/3. First pass: high = 1 (2 > 4/3),
    /// low = 0 (1 < 4/3), fcum[1] = 2 - 1/3 = 5/3, wi[0] = 3/4, bin[0] = 1.
    /// Second pass: high = 1 (5/3), low = 2, wi[2] = 3/4, bin[2] = 1.
    #[test]
    fn small_discrete_table_by_hand() {
        let t = EgsAliasTable::new(&[1.0, 2.0, 3.0], &[1.0, 2.0, 1.0], AliasTableKind::Discrete)
            .unwrap();
        assert_eq!(t.bins(), &[1, 0, 1]);
        assert_eq!(t.wi(), &[0.75, 1.0, 0.75]);
        assert_eq!(t.fi(), &[0.25, 0.5, 0.25]);
        assert_eq!(t.average(), 2.0);
        assert_eq!(t.maximum(), 3.0);
        assert_eq!(t.kind(), AliasTableKind::Discrete);
        assert!(!t.aliasing_incomplete());
        // r1 = 0.5 -> aj = 1.5, j = 1, keep: x = 2.
        let mut r = Replay {
            k: alloc::vec![1u64 << 52],
            pos: 0,
        };
        assert_eq!(t.sample(&mut r), 2.0);
        assert_eq!(t.sample_bin(&mut r), 1);
    }

    /// Histogram and linear values within a bin follow upstream's formulas,
    /// including the `fi == 0` sqrt branch and both `|a|` branches.
    #[test]
    fn within_bin_formulas() {
        use AliasTableKind::*;
        let xi = [0.0, 2.0];
        // Histogram: x + dx*r2.
        assert_eq!(egs_alias_value(Histogram, &xi, &[1.0], 0, 0.25), 0.5);
        // Linear, f0 = 0: x + dx*sqrt(r2).
        assert_eq!(
            egs_alias_value(LinearInterpolation, &xi, &[0.0, 1.0], 0, 0.25),
            1.0
        );
        // Linear, f1 = f0 (a = 0): uniform.
        assert_eq!(
            egs_alias_value(LinearInterpolation, &xi, &[1.0, 1.0], 0, 0.25),
            0.5
        );
        // Linear, a = 1 (|a| >= 0.2): inverse CDF of 1 + x/2 on [0, 2].
        let v = egs_alias_value(LinearInterpolation, &xi, &[1.0, 2.0], 0, 0.5);
        // x - dx/a*(1 - sqrt(1 + r2*a*(2+a))) = -2*(1 - sqrt(2.5)).
        assert_eq!(
            v,
            0.0 - 2.0 / 1.0 * (1.0 - Real::sqrt(1.0 + 0.5 * 1.0 * (2.0 + 1.0)))
        );
        assert!((v - (2.0 * Real::sqrt(2.5) - 2.0)).abs() < 1e-15);
        // Out of range index: NaN.
        assert!(egs_alias_value(Discrete, &xi, &[1.0, 1.0], 5, 0.0).is_nan());
        assert!(egs_alias_value(Histogram, &xi, &[1.0], 1, 0.0).is_nan());
        assert!(
            egs_alias_value(LinearInterpolation, &[0.0, 1.0, 2.0], &[1.0, 1.0], 1, 0.0).is_nan()
        );
    }

    #[test]
    fn simple_table_by_hand_and_clamp() {
        // f = (1, 3): wi = (0.5, 1.5); pair small 0 with big 1:
        // bins[0] = 1, wi[1] = 1.5 - 0.5 = 1.0 < 1 + eps -> small.
        let t = EgsSimpleAliasTable::new(&[1.0, 3.0]).unwrap();
        assert_eq!(t.wi(), &[0.5, 1.0]);
        assert_eq!(t.bins(), &[1, 1]);
        assert!(!t.aliasing_incomplete());
        // u1 = 0 -> bin 0; u2 = 0.75 >= 0.5 -> alias 1.
        let mut r = Replay {
            k: alloc::vec![0, 3u64 << 51],
            pos: 0,
        };
        assert_eq!(t.sample(&mut r), 1);
        // The largest uniform still lands in the last bin.
        assert_eq!(
            egs_alias_pick(&[1.0, 1.0, 1.0], &[0, 0, 0], 1.0 - f64::EPSILON / 2.0),
            2
        );
        // An r1 >= 1 (impossible from `random::<f64>()`) is clamped to the
        // last bin, whose remainder 19 then exceeds wi and takes its alias.
        assert_eq!(egs_alias_pick(&[1.0, 1.0, 1.0], &[0, 0, 0], 7.0), 0);
    }

    /// `from_function` on a straight line needs no refinement (Met, 2
    /// points); on a curve with a tiny `nmax` it stops (NotMetAtNmax).
    #[test]
    fn function_driven_goal_and_nmax() {
        let (t, g) = EgsAliasTable::from_function(0.0, 1.0, 1e-6, 100, |x| 1.0 + x).unwrap();
        assert_eq!(g, AccuracyGoal::Met);
        assert_eq!(t.xi(), &[0.0, 1.0]);
        let (t, g) = EgsAliasTable::from_function(0.0, 1.0, 1e-12, 4, |x| 1.0 + x * x).unwrap();
        assert_eq!(g, AccuracyGoal::NotMetAtNmax);
        assert!(t.xi().len() >= 4);
        assert_eq!(t.kind(), AliasTableKind::LinearInterpolation);
    }
}
