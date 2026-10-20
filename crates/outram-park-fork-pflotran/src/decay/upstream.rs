// Ported from PFLOTRAN, https://bitbucket.org/pflotran/pflotran, at commit
// 2af572b8759c34aacc2216f56a906637f90af03e, read 2026-10-20 (gh:#1000):
//   src/pflotran/pm_ufd_decay.F90  PMUFDDecaySolveISPDIAtCell (lines 1760-2077)
//                                  and PMUFDDecaySetup (lines 1203-1262)
//   src/pflotran/utility.F90       LUDecomposition1 / LUBackSubstitution
//                                  (lines 561-699)
//   src/pflotran/reaction.F90      RRadioactiveDecay (lines 5323-5424)
//
// Copyright (C) 2023 Battelle Memorial Institute (PFLOTRAN v5.X)
// Copyright (C) 2020 National Technology & Engineering Solutions of Sandia (v3.X)
// Copyright (C) 2009 Los Alamos National Security (v2.X)          (upstream)
// Copyright (C) 2026 Theodore Ong and the outram-park contributors  (this port)
//
// PFLOTRAN is LGPL-3.0 (verified from its LICENSE file -- see NOTICE and
// upstream_source/README.md), elected here as GPL-3.0. This derivative work is
// GPL-3.0-only, conveyed inside this crate's AGPL-3.0 combined work.

//! Upstream PFLOTRAN's own decay algorithms, ported as explicit fidelity
//! choices (gh:#1000): [`super::DecaySolver::UfdThreeGeneration`],
//! [`super::DecaySolver::UfdImplicit`] and [`super::DecaySolver::BackwardEuler`].
//!
//! Use these **to reproduce what PFLOTRAN computes**, quirks included. For the
//! most accurate answer to the decay ODE use the default CRAM48, which these
//! are measured against in the tests below.
//!
//! # What is ported, and how exactly
//!
//! - **UFD_DECAY default** (`ufd_three_generation`): the explicit
//!   3-generation analytical (Bateman) solution of `pm_ufd_decay.F90` lines
//!   1868-1928, every expression in upstream's order of evaluation, so the
//!   result is **bit-identical** to the compiled Fortran (code-to-code test
//!   below). Then the `1e-90` floor of line 1994.
//! - **UFD_DECAY `IMPLICIT_SOLUTION`** (`ufd_implicit`): the backward-Euler
//!   Newton of lines 1929-1991, in upstream's log formulation, with upstream's
//!   row scaling, its own Crout LU (`lu_decomposition`,
//!   `lu_back_substitution`, `utility.F90`), the update clip at 10 and the
//!   relative-change test at `1e-6`, then the `1e-90` floor. **Bit-identical**
//!   to the compiled Fortran, Newton iteration counts included, when the
//!   Fortran calls scalar `exp`; a plain `gfortran -O2` build vectorises line
//!   1989's `exp` into glibc libmvec and differs from it (and from this port)
//!   by 1 ulp in 19 of 115 masses (see Verification).
//! - **CHEMISTRY `RADIOACTIVE_DECAY_REACTION`** (`backward_euler`): upstream
//!   puts `rate = kf * N` into the reaction residual with its analytical
//!   Jacobian (`reaction.F90` lines 5374-5384) and lets the global (or
//!   operator-split `RReact`) Newton converge it. For an isolated decay
//!   system that root is the backward-Euler step `(I - dt A) N1 = N0`, which
//!   is what is ported: one direct solve with upstream's LU. **Not
//!   reproduced:** coupling to transport or to other reactions, sorbed and
//!   gas-phase inventories (upstream decays aqueous + gas + equilibrium-sorbed
//!   totals), and the Newton's own stopping error (upstream stops at its
//!   residual tolerances, so its answer differs from the exact root by up to
//!   those tolerances; the direct solve has no such error).
//!
//! # Upstream behaviour carried over, which a user must know
//!
//! 1. **The 3-generation form ignores branching fractions.** Lines 1880-1925
//!    use only decay constants and the parent list; `isotope_daughter_stoich`
//!    is never read on this path. A daughter fed with branching `b < 1`
//!    grows as if `b = 1`, so a branching chain *gains* atoms (measured in
//!    `ufd_three_generation_ignores_branching_like_upstream`). Upstream refuses
//!    only `b > 1` (`PMUFDDecayInitializeRun`, line 1463, "Daughter
//!    stoichiometries have not been set up"); [`super::DecayChain::new`]
//!    refuses that too. The implicit path does use `b`.
//! 2. **The 3-generation form truncates.** A nuclide receives ingrowth from
//!    its parents and grandparents only; atoms that would arrive through a
//!    great-grandparent within one step are lost from it (they are not moved
//!    anywhere, so a deep chain loses atoms per step). Exact for chains of
//!    depth <= 3.
//! 3. **Equal decay constants divide by zero.** Upstream guards only an
//!    isotope that is its own parent or grandparent (lines 1243-1262); a
//!    parent and daughter with equal `lambda` give `Inf`/`NaN`, which it
//!    then carries on. This port refuses such a chain with
//!    [`PflotranError::InvalidInput`] before computing (a signalled error
//!    instead of a silent `NaN`). Nearly equal constants are not refused and
//!    lose accuracy by cancellation, as upstream does.
//! 4. **The implicit Newton has no iteration cap upstream** (`it` is counted,
//!    line 1937, and never tested). From an exactly zero inventory it never
//!    stops: with a zero daughter the Jacobian column is zero, the pivot
//!    becomes `tiny = 1e-20`, the clipped update for that nuclide pollutes the
//!    others and the iterate cycles forever (observed: the extracted Fortran
//!    run past 1.9e6 iterations without converging, `driver zero`); with a zero
//!    parentless nuclide the Jacobian row is zero and `LUDecomposition`
//!    aborts the run (`driver zeroparent`). This port caps the loop at
//!    [`UFD_IMPLICIT_MAX_ITERATIONS`] and returns
//!    [`PflotranError::Convergence`], and returns
//!    [`PflotranError::Convergence`] for the zero row instead of aborting.
//!    **Both are deviations from upstream**, which hangs or aborts there.
//!    Upstream never reaches them in practice after its first step, because
//!    every step ends with the `1e-90` floor.
//! 5. **The implicit Newton starts its convergence test from 1.** Line 1931
//!    sets `prev_solution = 1`, so an inventory whose every entry is within
//!    `1e-6` (relative) of `1.0` passes the test before the first iteration
//!    and is returned **undecayed** (`ufd_implicit_skips_an_inventory_of_ones`).
//! 6. **The `1e-90` floor creates matter**: a nuclide at exactly zero leaves
//!    either UFD solve at `1e-90`. Harmless in mol, but not an exact zero.
//!
//! # Verification
//!
//! Code-to-code against the compiled upstream routines:
//! `verification_and_validation/upstream_decay/` holds `extract.sh`, which
//! cuts lines 1868-1994 of `pm_ufd_decay.F90` and lines 542-699 of
//! `utility.F90` out of the upstream clone verbatim (checked against their
//! SHA-256), `driver.F90`, which supplies only declarations, the parent list
//! as `PMUFDDecaySetup` builds it and the cases, and `run.sh`, which builds it
//! twice: `upstream_output.txt` (`-O2 -fno-tree-vectorize`, scalar `exp`) and
//! `upstream_output_O2_vector.txt` (plain `-O2`, libmvec `exp` on line 1989),
//! as hex bit patterns. Measured 2026-10-20 (gfortran 16.2.1, glibc x86-64):
//!
//! | path | vs scalar-`exp` build | vs plain `-O2` build |
//! |---|---|---|
//! | 3-generation | 165 / 165 bit-identical | 165 / 165 bit-identical |
//! | `IMPLICIT_SOLUTION` masses | 115 / 115 bit-identical | 96 / 115; worst 2.4e-16 relative (1 ulp) |
//! | `IMPLICIT_SOLUTION` Newton counts | 25 / 25 | 25 / 25 |
//! | decay constants (HALF_LIFE card) | 18 / 18 | 18 / 18 |
//!
//! Which build a real PFLOTRAN is compiled like depends on its PETSc
//! `FOPTFLAGS`; the difference is 1 ulp either way. The analytic checks
//! (exactness at depth 3, convergence order, branching, atom balance) are in
//! the tests' doc comments. Summary and the zero-inventory probes:
//! `verification_and_validation/upstream_decay/README.md`.

use petir::mathf::RealMath;

use crate::error::PflotranError;

/// The relative-change tolerance of the `IMPLICIT_SOLUTION` Newton,
/// `pm_ufd_decay.F90` line 1816 (`PetscReal, parameter :: tolerance = 1.d-6`).
pub const UFD_IMPLICIT_TOLERANCE: f64 = 1.0e-6;

/// The floor applied to every isotope mass after either UFD solve,
/// `pm_ufd_decay.F90` line 1994 (`max(mass_iso_tot1,1.d-90)`).
pub const UFD_MASS_FLOOR: f64 = 1.0e-90;

/// The clip on each Newton update in log space, `pm_ufd_decay.F90` line 1987
/// (`min(dabs(rhs),10.d0)`).
pub const UFD_UPDATE_CLIP: f64 = 10.0;

/// **Not upstream.** The iteration cap this port adds to the
/// `IMPLICIT_SOLUTION` Newton, which upstream runs without one (see the module
/// docs, item 4). Measured counts on the code-to-code cases are 4 to 25
/// (`upstream_output.txt`), so 1000 binds only where upstream would never stop.
pub const UFD_IMPLICIT_MAX_ITERATIONS: usize = 1000;

/// `tiny`, the value `LUDecomposition1` writes over an exactly zero pivot,
/// `utility.F90` line 572 (`PetscReal, parameter :: tiny=1.0d-20`).
const LU_TINY: f64 = 1.0e-20;

/// Parent lists as `PMUFDDecaySetup` builds them, `pm_ufd_decay.F90` lines
/// 1213-1222: for each isotope in index order, for each of its daughters in
/// the order given, append the isotope to that daughter's list. A parent that
/// names the same daughter twice appears twice, as upstream.
pub(crate) fn parent_lists(daughters: &[Vec<(usize, f64)>]) -> Vec<Vec<usize>> {
    let mut parents = vec![Vec::new(); daughters.len()];
    for (p, ds) in daughters.iter().enumerate() {
        for &(d, _) in ds {
            if let Some(list) = parents.get_mut(d) {
                list.push(p);
            }
        }
    }
    parents
}

/// Upstream's setup guard, `pm_ufd_decay.F90` lines 1243-1262: no isotope may
/// be its own parent or its own grandparent ("this will produce NaNs through
/// a divide by zero"). Upstream calls `PrintErrMsg`, which stops the run;
/// this returns [`PflotranError::InvalidInput`].
pub(crate) fn check_self_ancestry(parents: &[Vec<usize>]) -> Result<(), PflotranError> {
    for (i, ps) in parents.iter().enumerate() {
        for &ip in ps {
            if ip == i {
                return Err(PflotranError::InvalidInput(format!(
                    "PM UFD_DECAY isotope {i} is the same as its parent"
                )));
            }
            for &ig in parents.get(ip).map(Vec::as_slice).unwrap_or(&[]) {
                if ig == i {
                    return Err(PflotranError::InvalidInput(format!(
                        "PM UFD_DECAY isotope {i} is the same as its grandparent"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// The guard this port adds in front of the 3-generation form (module docs,
/// item 3): every difference of decay constants it divides by must be
/// non-zero. Upstream has no such guard and produces `Inf`/`NaN`.
fn check_distinct_rates(lambda: &[f64], parents: &[Vec<usize>]) -> Result<(), PflotranError> {
    let rate = |k: usize| lambda.get(k).copied().unwrap_or(0.0);
    let clash = |a: usize, b: usize| -> Result<(), PflotranError> {
        if rate(a) == rate(b) {
            Err(PflotranError::InvalidInput(format!(
                "UFD 3-generation decay: isotopes {a} and {b} share the decay \
                 constant {} 1/s; upstream divides by their difference \
                 (pm_ufd_decay.F90 lines 1882-1924) and returns Inf/NaN",
                rate(a)
            )))
        } else {
            Ok(())
        }
    };
    for (i, ps) in parents.iter().enumerate() {
        for &ip in ps {
            clash(i, ip)?;
            for &ig in parents.get(ip).map(Vec::as_slice).unwrap_or(&[]) {
                clash(ip, ig)?;
                clash(i, ig)?;
            }
        }
    }
    Ok(())
}

/// Fortran `max(x, 1.d-90)` on a finite `x` (line 1994).
fn floor_mass(x: f64) -> f64 {
    if x > UFD_MASS_FLOOR {
        x
    } else {
        UFD_MASS_FLOOR
    }
}

/// gfortran `MAXVAL` on a non-empty array: the largest non-`NaN` entry, or
/// `NaN` if every entry is `NaN` (probed with gfortran 16.2.1 on 2026-10-20).
fn fortran_maxval(xs: impl Iterator<Item = f64>) -> f64 {
    let mut best = f64::NAN;
    for x in xs {
        if !x.is_nan() && (best.is_nan() || x > best) {
            best = x;
        }
    }
    best
}

/// The UFD_DECAY default: 3-generation explicit analytical Bateman solution,
/// a port of `pm_ufd_decay.F90` lines 1868-1928 plus the floor of line 1994.
///
/// `lambda[i]` is isotope `i`'s decay constant (1/s), `parents` the lists
/// from [`parent_lists`], `mass_old` the inventory at the start of the step.
/// Every product, quotient and sum is evaluated in upstream's order (Fortran
/// evaluates `a - b + c` as `(a - b) + c` and `a*b*c` as `(a*b)*c`).
///
/// # Errors
/// [`PflotranError::InvalidInput`] for a self-parent or self-grandparent
/// (upstream's guard) or for equal decay constants on a path it divides by
/// (this port's guard, module docs item 3).
pub(crate) fn ufd_three_generation(
    lambda: &[f64],
    parents: &[Vec<usize>],
    mass_old: &[f64],
    dt: f64,
) -> Result<Vec<f64>, PflotranError> {
    check_self_ancestry(parents)?;
    check_distinct_rates(lambda, parents)?;
    let n = mass_old.len();
    let l = |k: usize| lambda.get(k).copied().unwrap_or(0.0);
    let m = |k: usize| mass_old.get(k).copied().unwrap_or(0.0);
    let ps = |k: usize| parents.get(k).map(Vec::as_slice).unwrap_or(&[]);

    // FIRST PASS (lines 1876-1897): the coefficient of exp(-lambda_i dt).
    let mut coeff = vec![0.0_f64; n];
    for (i, c) in coeff.iter_mut().enumerate() {
        *c = m(i);
        for &ip in ps(i) {
            *c -= (l(ip) * m(ip)) / (l(i) - l(ip));
            for &ig in ps(ip) {
                *c = *c - ((l(ip) * l(ig) * m(ig)) / ((l(ip) - l(ig)) * (l(i) - l(ig))))
                    + ((l(ip) * l(ig) * m(ig)) / ((l(ip) - l(ig)) * (l(i) - l(ip))));
            }
        }
    }

    // SECOND PASS (lines 1898-1928).
    let mut out = vec![0.0_f64; n];
    for (i, o) in out.iter_mut().enumerate() {
        *o = coeff.get(i).copied().unwrap_or(0.0) * (-1.0 * l(i) * dt).r_exp();
        for &ip in ps(i) {
            *o += ((l(ip) * m(ip)) / (l(i) - l(ip))) * (-1.0 * l(ip) * dt).r_exp();
            for &ig in ps(ip) {
                *o = *o
                    - ((l(ip) * l(ig) * m(ig) * (-1.0 * l(ip) * dt).r_exp())
                        / ((l(ip) - l(ig)) * (l(i) - l(ip))))
                    + ((l(ip) * l(ig) * m(ig) * (-1.0 * l(ig) * dt).r_exp())
                        / ((l(ip) - l(ig)) * (l(i) - l(ig))));
            }
        }
    }

    finish(out)
}

/// The floor of line 1994, after refusing a non-finite result (which
/// upstream would carry on with; module docs item 3).
fn finish(mut out: Vec<f64>) -> Result<Vec<f64>, PflotranError> {
    if let Some((i, x)) = out.iter().enumerate().find(|(_, x)| !x.is_finite()) {
        return Err(PflotranError::Convergence(format!(
            "UFD decay produced a non-finite mass {x} for isotope {i}"
        )));
    }
    for x in &mut out {
        *x = floor_mass(*x);
    }
    Ok(out)
}

/// The UFD_DECAY `IMPLICIT_SOLUTION` path: backward Euler solved by Newton in
/// a log formulation, a port of `pm_ufd_decay.F90` lines 1929-1991 plus the
/// floor of line 1994. Returns the new inventory and the Newton iteration
/// count (upstream's `it`).
///
/// `daughters[p]` lists `(daughter, stoichiometry)` in upstream's order.
///
/// # Errors
/// [`PflotranError::InvalidInput`] for a self-parent or self-grandparent
/// (upstream's guard); [`PflotranError::Convergence`] for a zero Jacobian row
/// (upstream aborts) or after [`UFD_IMPLICIT_MAX_ITERATIONS`] iterations
/// (upstream loops forever); both are module docs item 4.
pub(crate) fn ufd_implicit(
    lambda: &[f64],
    daughters: &[Vec<(usize, f64)>],
    mass_iso_tot0: &[f64],
    dt: f64,
) -> Result<(Vec<f64>, usize), PflotranError> {
    check_self_ancestry(&parent_lists(daughters))?;
    let n = mass_iso_tot0.len();
    let one_over_dt = 1.0 / dt;
    let mut prev_solution = vec![1.0_f64; n]; // line 1931
    let mut solution = mass_iso_tot0.to_vec(); // line 1932
    let mut it = 0_usize;
    let mut residual = vec![0.0_f64; n];
    let mut jac = vec![0.0_f64; n * n]; // row-major: jac[i * n + j] = Jacobian(i, j)
    let mut indices = vec![0_usize; n];
    loop {
        // Line 1936: inf-norm relative change.
        let change = fortran_maxval(
            solution
                .iter()
                .zip(&prev_solution)
                .map(|(s, p)| (s - p).abs() / p),
        );
        if change < UFD_IMPLICIT_TOLERANCE {
            break;
        }
        if it >= UFD_IMPLICIT_MAX_ITERATIONS {
            return Err(PflotranError::Convergence(format!(
                "UFD IMPLICIT_SOLUTION Newton did not converge in {it} iterations \
                 (relative change {change:e}); upstream has no cap and would not \
                 stop either (pm_ufd_decay.F90 lines 1934-1990)"
            )));
        }
        prev_solution.copy_from_slice(&solution);
        it += 1;
        residual.iter_mut().for_each(|r| *r = 0.0);
        jac.iter_mut().for_each(|j| *j = 0.0);
        // Lines 1945-1972.
        for iiso in 0..n {
            let s = solution.get(iiso).copied().unwrap_or(0.0);
            let m0 = mass_iso_tot0.get(iiso).copied().unwrap_or(0.0);
            if let Some(r) = residual.get_mut(iiso) {
                *r += (s - m0) * one_over_dt;
            }
            if let Some(j) = jac.get_mut(iiso * n + iiso) {
                *j += one_over_dt;
            }
            let rate_constant = lambda.get(iiso).copied().unwrap_or(0.0);
            let rate = rate_constant * s;
            if let Some(r) = residual.get_mut(iiso) {
                *r += rate;
            }
            if let Some(j) = jac.get_mut(iiso * n + iiso) {
                *j += rate_constant;
            }
            for &(idaughter, stoich) in daughters.get(iiso).map(Vec::as_slice).unwrap_or(&[]) {
                if let Some(r) = residual.get_mut(idaughter) {
                    *r -= rate * stoich;
                }
                if let Some(j) = jac.get_mut(idaughter * n + iiso) {
                    *j -= rate_constant * stoich;
                }
            }
        }
        // Lines 1974-1981: row scaling.
        let mut rhs = vec![0.0_f64; n];
        for (iiso, (row, r)) in jac.chunks_exact_mut(n.max(1)).zip(&mut rhs).enumerate() {
            let mut norm = fortran_maxval(row.iter().map(|x| x.abs())).max(1.0);
            norm = 1.0 / norm;
            *r = residual.get(iiso).copied().unwrap_or(0.0) * norm;
            row.iter_mut().for_each(|x| *x *= norm);
        }
        // Lines 1982-1985: log formulation, column scaling.
        for row in jac.chunks_exact_mut(n.max(1)) {
            for (x, s) in row.iter_mut().zip(&solution) {
                *x *= s;
            }
        }
        // Lines 1986-1990.
        lu_decomposition(&mut jac, n, &mut indices)?;
        lu_back_substitution(&jac, n, &indices, &mut rhs);
        for (s, r) in solution.iter_mut().zip(&rhs) {
            let clipped = 1.0_f64.copysign(*r) * r.abs().min(UFD_UPDATE_CLIP);
            *s *= (-clipped).r_exp();
        }
    }
    Ok((finish(solution)?, it))
}

/// The backward-Euler step that CHEMISTRY `RADIOACTIVE_DECAY_REACTION`'s
/// Newton converges to for an isolated decay system: solve
/// `(I - dt A) N1 = N0` with upstream's LU. `a` is the decay matrix,
/// row-major (`A[i][i] = -lambda_i`, `A[d][p] = b_{p,d} lambda_p`, the
/// stoichiometries of `reaction.F90` lines 5374-5384: `-1` on the decaying
/// species, `+b` on each product). See the module docs for what is and is not
/// reproduced.
///
/// # Errors
/// [`PflotranError::Convergence`] if a row of `I - dt A` is zero (not
/// reachable for a decay matrix and `dt >= 0`: the diagonal is `1 + dt
/// lambda_i >= 1`).
pub(crate) fn backward_euler(a: &[f64], n0: &[f64], dt: f64) -> Result<Vec<f64>, PflotranError> {
    let n = n0.len();
    let mut m: Vec<f64> = a.iter().map(|x| -dt * x).collect();
    for i in 0..n {
        if let Some(d) = m.get_mut(i * n + i) {
            *d += 1.0;
        }
    }
    let mut indices = vec![0_usize; n];
    lu_decomposition(&mut m, n, &mut indices)?;
    let mut x = n0.to_vec();
    lu_back_substitution(&m, n, &indices, &mut x);
    Ok(x)
}

/// Crout LU with implicit (scaled) partial pivoting, a port of
/// `LUDecomposition1`, `utility.F90` lines 561-652 (Numerical Recipes
/// `ludcmp`). `a` is row-major `n x n`, overwritten by its factors;
/// `indx[j]` records the pivot row (0-based) of column `j`.
///
/// Differences from upstream: a zero row returns [`PflotranError::Convergence`]
/// where upstream (called through `LUDecomposition2`, `stop_on_error =
/// PETSC_TRUE`) prints the Jacobian and calls `MPI_Abort`; the sign `D` is not
/// returned (no caller here uses it).
///
/// `petir::linalg::SquareMatrix` (OpenFOAM's `LUDecompose`, lifted) is the same
/// algorithm with the same arithmetic order, and was checked and rejected for
/// reuse: on a zero row it scales by `0` instead of stopping, and over a zero
/// pivot it writes `f64::EPSILON`, not `1e-20`. The UFD implicit Newton meets a
/// zero pivot whenever a nuclide's inventory is zero, so the guard matters.
pub(crate) fn lu_decomposition(
    a: &mut [f64],
    n: usize,
    indx: &mut [usize],
) -> Result<(), PflotranError> {
    let get = |a: &[f64], i: usize, j: usize| a.get(i * n + j).copied().unwrap_or(0.0);
    let mut vv = vec![0.0_f64; n];
    for (i, v) in vv.iter_mut().enumerate() {
        let mut aamax = 0.0_f64;
        for j in 0..n {
            if get(a, i, j).abs() > aamax {
                aamax = get(a, i, j).abs();
            }
        }
        if aamax <= 0.0 {
            return Err(PflotranError::Convergence(format!(
                "Singular value encountered in LUDecomposition(): row {i} is zero \
                 (upstream utility.F90 line 593 aborts the run here)"
            )));
        }
        *v = 1.0 / aamax;
    }
    for j in 0..n {
        for i in 0..j {
            let mut sum = get(a, i, j);
            for k in 0..i {
                sum -= get(a, i, k) * get(a, k, j);
            }
            if let Some(x) = a.get_mut(i * n + j) {
                *x = sum;
            }
        }
        let mut aamax = 0.0_f64;
        let mut imax = j;
        for i in j..n {
            let mut sum = get(a, i, j);
            for k in 0..j {
                sum -= get(a, i, k) * get(a, k, j);
            }
            if let Some(x) = a.get_mut(i * n + j) {
                *x = sum;
            }
            let dum = vv.get(i).copied().unwrap_or(0.0) * sum.abs();
            if dum >= aamax {
                imax = i;
                aamax = dum;
            }
        }
        if j != imax {
            for k in 0..n {
                a.swap(imax * n + k, j * n + k);
            }
            if let Some(vj) = vv.get(j).copied() {
                if let Some(slot) = vv.get_mut(imax) {
                    *slot = vj;
                }
            }
        }
        if let Some(slot) = indx.get_mut(j) {
            *slot = imax;
        }
        if get(a, j, j) == 0.0 {
            if let Some(x) = a.get_mut(j * n + j) {
                *x = LU_TINY;
            }
        }
        if j + 1 != n {
            let dum = 1.0 / get(a, j, j);
            for i in (j + 1)..n {
                if let Some(x) = a.get_mut(i * n + j) {
                    *x *= dum;
                }
            }
        }
    }
    Ok(())
}

/// Forward and back substitution on the factors from [`lu_decomposition`], a
/// port of `LUBackSubstitution`, `utility.F90` lines 656-699, including its
/// skip of leading zeros in `b` (`ii`). `b` is overwritten by the solution.
pub(crate) fn lu_back_substitution(a: &[f64], n: usize, indx: &[usize], b: &mut [f64]) {
    let get = |i: usize, j: usize| a.get(i * n + j).copied().unwrap_or(0.0);
    // `ii` is 1-based upstream with 0 meaning "not yet"; here `None`.
    let mut ii: Option<usize> = None;
    for i in 0..n {
        let ll = indx.get(i).copied().unwrap_or(i);
        let mut sum = b.get(ll).copied().unwrap_or(0.0);
        let bi = b.get(i).copied().unwrap_or(0.0);
        if let Some(x) = b.get_mut(ll) {
            *x = bi;
        }
        if let Some(start) = ii {
            for j in start..i {
                sum -= get(i, j) * b.get(j).copied().unwrap_or(0.0);
            }
        } else if sum != 0.0 {
            ii = Some(i);
        }
        if let Some(x) = b.get_mut(i) {
            *x = sum;
        }
    }
    for i in (0..n).rev() {
        let mut sum = b.get(i).copied().unwrap_or(0.0);
        for j in (i + 1)..n {
            sum -= get(i, j) * b.get(j).copied().unwrap_or(0.0);
        }
        if let Some(x) = b.get_mut(i) {
            *x = sum / get(i, i);
        }
    }
}

#[cfg(test)]
#[path = "upstream_tests.rs"]
mod tests;
