# Predictions, written and committed BEFORE any trace or trial of the dense-piece diagnosis (gh:#1390, 2026-11-19)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-written and unreviewed.

Question (maintainer, 2026-11-19, "Diagnose one dense piece"): on U-238 piece 296 (7 789 points; OpenMC
67 poles and 0 grid points outside rtol/atol, the port 60 poles and 2 094 outside), is the difference a port
defect or chaos in the fit-order search? Reference: OpenMC `8f79d9d` pure-Python `vectfit.py` / `multipole.py`
(`crates/outram-mc-libs/upstream_source/OpenMC`), the commit the port translates.

## Side-by-side reading (done before running; code read: `_vectfit_xs` against `generate/fit.rs::vectfit_xs`, `vectfit.py` against `generate/vectfit.rs`, numpy/scipy primitives against `generate/npy.rs`)

Control flow, bounds, tie-breaking and defaults: **no difference found.** Checked line by line:

| item | upstream | port | same? |
|---|---|---|---|
| order list | `range(max(2,2*n_peaks), max(200,4*n_peaks)+1, 2)`, `n_peaks` = `find_peaks(xs0+xs1)` | `lowest..=highest.step_by(2)`, `count_peaks` = `_local_maxima_1d` (plateau middle, strict) | yes |
| initial poles | `linspace(s0, s[-1], order//2)`, `p(1+0.01j)`, append conj, `np.sort` (real, then imag) | same, `ccmp` sort | yes |
| iterations | `n_vf_iter = 30`; next iteration restarts from the **un-split** `poles` returned by the relocation call, not from `new_poles` | same | yes |
| real-pole split | `s0 <= p_r <= s[-1]` and `p_i == 0.` -> `p_r(1 +- 0.01j)`, then residues recomputed with `skip_pole_update=True` | same | yes |
| test grid | 10x finer `np.interp`; reference `np.interp` | `interp` ported from `arr_interp` incl. NaN fall-backs | yes |
| metric | `ratio`, `ratio2`, `maxre` over `relerr[abserr > atol]`, NaN and all-below-atol branches, `quality = ratio + ratio2 - min(0.1 maxre, 1) - 0.001 n_poles`, `-inf` if any `test_xs < -atol` | same, `min` and NaN semantics copied | yes |
| acceptance | strict `quality > best_quality`; `best_ratio >= 1.0` stops (ideal) | same | yes |
| acceleration | `order > max(2 n_peaks, 50)` and `best_ratio > 0.7`: `n_discarded += 1`, stop at 10, or at 5 if `best_ratio > 0.9`; reset by any improvement at that order | same | yes |
| lstsq | `scipy.linalg.lstsq` (DGELSD), `cond = eps`; singular values `<= cond*s_max` dropped | QR then Golub-Reinsch SVD (petir), `rcond = f64::EPSILON`, same drop rule | **numerics differ** (not control flow) |
| QR | LAPACK `DGEQRF` (blocked) | unblocked `DGEQR2` convention | numerics differ (round-off) |
| eigenvalues | `scipy.linalg.eigvals` = `DGEEV` (permute+scale balance, Hessenberg QR) | petir `eigen_nonsymm(balance = true)` (GSL, scaling only); pair order forced to `DGEEV`'s | numerics differ; order of non-paired eigenvalues differs |
| `evaluate` | `residues @ (1/(s - p))` (BLAS zgemm), numpy Smith division | sequential complex sum, Smith division | summation order differs (round-off) |
| norms | `scipy.linalg.norm` (dnrm2), `np.linalg.norm` | `sqrt(sum x^2)` | round-off |

So the only places a *defect* can hide are the three numerical kernels (lstsq truncation, eigenvalue accuracy,
evaluate round-off), not the search logic. A defect there would be systematic; chaos would not.

## Hypotheses and what each predicts

**H-chaos** (the two codes are two draws from one distribution; the search is sensitive to round-off).
**H-defect** (the port's numerical kernels differ systematically so the port's distribution is shifted).

My own odds before running: H-chaos 55 %, H-defect 30 %, something in between (both noisy, but the port's
noise has a different size, for example from the eigen-solver) 15 %.

| observation | H-chaos predicts | H-defect predicts |
|---|---|---|
| **Order-by-order trace, iteration 1 of each order** (initial poles are the same deterministic `linspace`, so no chaos has accumulated yet) | the two codes agree on quality, ratio and the count of test-grid points outside, to within round-off effects, at most orders (>= 80 % of orders within 1e-6 in quality) | systematic gaps already at iteration 1 at a subset of orders (rank/truncation or eigen-solver) |
| **Iterations 2..30 of an order** | the two codes' pole sets drift apart as iterations accumulate (growth of the difference), without any iteration where one code is wrong and the other right | the same drift, plus one code visibly failing to improve where the other does |
| **First order where an ideal fit (`ratio >= 1`) exists** | OpenMC's unperturbed run hits ideal at its order (final 67 poles); the port reaches near-ideal (a few points outside) at a nearby order but never exactly 1.0 or not before the acceleration stop | the port's best ratio at every order stays clearly below OpenMC's (a gap of the whole curve, not of a few orders) |
| **Why the port stops** | acceleration stop (`n_discarded >= 5` with `best_ratio > 0.9`) at some order after the best; OpenMC stops by `found_ideal` | same |
| **+-1 ulp trials, 4 or more per code (same input files for both)** | pole count and points-outside distributions overlap: OpenMC will not always give 0 outside (previously 66 on 296, 186 on 250) and the port will sometimes give 0 (previously 0 on 275). Concretely: at least one of the port's trials has fewer than 100 points outside, and at least one of OpenMC's has more than 100 or a pole count that differs from 67 | the port's trials all stay at hundreds-to-thousands outside and 55-65 poles, while OpenMC's cluster at 0 |

Predicted size of the answer if chaos: pole count over trials spans at least 6 values for each code
(previous OpenMC counts on 275: 46, 50, 48, 46; on 296: 67, 74), the means of the two codes differ by less than
the pooled spread. A "mixed" result (overlapping supports but clearly different means) will be reported as
such and not rounded to either hypothesis.

## What would make me call it a defect

(a) the iteration-1 trace differs systematically between the two codes at many orders, and
(b) I can name the kernel (lstsq truncation, eigenvalues, evaluate) with a measurement on a captured input
that shows the port departing from LAPACK/BLAS by more than the LAPACK answer's own sensitivity to a 1-ulp
input change. Without both, the conclusion is not "defect".

## Method fixed in advance

* Same input files for both codes (`openmc_inputs/make_trial_inputs.py`: the captured OpenMC input, and trials
  that multiply every cross-section value by `1 +- 2^-52`, sign drawn per value from
  `default_rng(1000 + k)`). Points outside rtol/atol are counted on the piece's own grid against the
  **unperturbed** data (as the existing self-sensitivity tables).
* OpenMC is traced without editing it: `openmc_inputs/trace_openmc.py` wraps `multipole.vectfit`,
  `multipole.evaluate` and `vectfit.lstsq` (module-level names) and recomputes the metric from the test grid
  with the formulas of `_vectfit_xs`; the final result of the wrapped run is compared with the unwrapped
  fixture (`fitted_pieces_ext.f64`, piece 296).
* The port is traced by a copy of the loop in a `#[cfg(test)]` `#[ignore]` module (`generate/trace.rs`) that
  calls the library's `vectfit` and `evaluate`; the library functions are unchanged. The copy is validated
  against `vectfit_xs` (bit-identical poles) on a cheap piece.
* Piece 296 first (OpenMC 1 319 s and port 1 357 s earlier, loaded). 298 only if 296 proves too slow.
