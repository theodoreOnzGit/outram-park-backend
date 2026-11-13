# QNG, GLFIXED, QAGP, QAWC against compiled GSL 2.8 (gh:#1340, round 2)

Parent: gh:#1340, epic gh:#1336. Research, education and V&V only
(`RESPONSIBLE_USE.md`). AI-assisted work: untrusted until a human reviews it.

## Methodology

- **Ports** (all from GSL 2.8, `ampl/gsl@cf180cd7`, read 2026-11-13):

  | PETIR | GSL source |
  |---|---|
  | `integration/qng.rs` | `qng.c` + `qng.h` (tables copied by script) |
  | `integration/glfixed.rs`, `glfixed_tables.rs` | `glfixed.c` (27 tables and `ltbl` copied by script) |
  | `integration/qagp.rs` | `qagp.c`, `ptsort.c`, `append.c` |
  | `integration/qawc.rs`, `qcheb.rs` | `qawc.c`, `qc25c.c`, `qcheb.c` |

  QAGP and QAWC re-use the QAGS engine pieces (`Workspace`, `qpsrt`, `qelg`,
  `update`, `large_interval`, ...). Their visibility was widened to
  `pub(super)`; no body changed (`git diff` of `qags.rs` is visibility only).
- **Reference:** one C driver per family in `reference-data/gsl/`, linked
  against GSL 2.8 built from the same commit (`gcc -O2`, glibc, x86-64;
  recipe in `build_gsl_family.sh`). Output: status, result, abserr, workspace
  size and number of integrand evaluations (QNG: GSL's own `*neval` and a
  count made by the integrand); GLFIXED: every table entry, integrals and
  `glfixed_point` values.
- **Replayed by** `tests/gsl_qng_*`, `gsl_glfixed_*`, `gsl_qagp_*`,
  `gsl_qawc_*` (`*_code_to_code.rs`).
- **Pass criteria:** status, size and evaluation count **exactly**; results
  and abserr counted bit-identical, and the count pinned (a 1-ulp difference
  from glibc's `pow`/`exp`/`cos` is accepted, counted and pinned per
  `AGENTS.md`).

## Predictions (written 2026-11-13 BEFORE any port was run against its reference)

- **P1 (all four).** Status, workspace size and evaluation count identical in
  every case.
- **P2 QNG.** Result bit-identical in every case with a rational or `sqrt`
  integrand; for `exp`/`cos`/`log` integrands a last-bit difference is
  possible. Prediction: results identical in at least 90 % of the cases,
  `abserr` identical in at least 80 % (it goes through `pow(.., 1.5)` in
  `rescale_error`).
- **P3 GLFIXED.** The 27 precomputed tables are bit-identical (copied by
  script): 100 %. The generated tables (Newton from a `cos` start) agree to
  within 1 ulp per entry: at least 95 % of entries bit-identical, the rest
  1 ulp. Integrals of the polynomial/rational integrands: bit-identical for
  the precomputed tables. `glfixed_point`: bit-identical wherever the table is.
  `ltbl[k] == 1.0 - 1.0/k` for every `k` in 2..1023 (the C comment's claim):
  I expect it holds, but have not tested it.
- **P4 QAGP.** Same machinery as QAGS, which was 95 % bit-identical in
  `abserr`. Prediction: results bit-identical in at least 90 % of the cases,
  `abserr` in at least 80 %; the misses are in `exp`/`log`/`cos`/`pow`
  integrands. The 12-point cases exercise the stale-index roundoff test
  (`i > 10`): if that quirk were ported wrongly, a status or evaluation count
  would differ in `wavy_12pts` or `sharp_12pts`.
- **P5 QAWC.** `compute_moments` uses `log`; the Chebyshev part is
  exact-arithmetic-identical. Prediction: status, size and neval identical in
  every case, results identical in at least 80 %.

## Results

(Filled in after the runs; see below.)
