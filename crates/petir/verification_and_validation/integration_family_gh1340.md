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

## Results (2026-11-13, Rust 1.99.1 release, GSL 2.8 `cf180cd7`, `gcc -O2`, glibc)

Replay tests: `tests/gsl_qng_code_to_code.rs` (27 cases),
`gsl_glfixed_code_to_code.rs` (40 table sizes), `gsl_qagp_code_to_code.rs`
(31), `gsl_qawc_code_to_code.rs` (22). All pass with the counts below pinned.

| family | cases | status / size / neval | result | abserr |
|---|---|---|---|---|
| QNG | 27 | 27/27 (GSL status, `*neval`, integrand calls) | 27/27 | **26/27** |
| QAGP | 31 | 31/31 | 31/31 | 31/31 |
| QAWC | 22 | 22/22 | 22/22 | 22/22 |
| GLFIXED precomputed tables | 27 sizes, 2 430 entries | n/a | **2 430/2 430** | n/a |
| GLFIXED generated tables | 13 sizes, 5 086 entries | n/a | **5 031/5 086**, worst 224 ulp (weights) | n/a |
| GLFIXED integrals | 320 | n/a | 309/320, worst 22 ulp | n/a |
| GLFIXED `glfixed_point` | 2 808 values, 40 `EINVAL` rows | n/a | 2 786/2 808, worst 88 ulp | n/a |

- **P1 held** (status, size, neval identical in every case of every family).
- **P2 (QNG) held**: results 27/27 (>= 90 % predicted), abserr 26/27 (>= 80 %).
  The one miss, `log_0_1`, is 1 ulp: 3.599909489090861e-3 against GSL's
  3.5999094890908604e-3. **Cause demonstrated, not assumed**: GSL's driver
  relinked with PETIR's `pow` alone (diagnostic shim exporting only `pow`,
  not committed) prints 3.59990948909086079e-03, PETIR's value. The `pow` is
  the one in `rescale_error`.
- **The `gauss` abserr miss of round 1 (qagi, 1e-13) is the same cause, now
  demonstrated.** With the same `pow`-only shim, GSL's `qagi` driver prints
  `gauss` abserr 1.03066608369450132e-12, exactly PETIR's
  1.0306660836945013e-12 (it was 1.03066608369460472e-12 with glibc `pow`).
  The shim also moves `x_m11_from1`, but that integrand itself calls `pow`,
  so that row is not a test of `rescale_error` and is excluded.
- **P3 (GLFIXED), partly FAILED.** The 27 precomputed tables are 100 %
  bit-identical, as predicted. For the generated tables 5 031 of 5 086
  entries (98.9 %) are bit-identical, which meets the ">= 95 %" half; the
  "rest within 1 ulp" half **failed**: the worst difference is 224 ulp, in
  **weights** of the large tables (n >= 513, weights ~1e-3, relative
  differences ~1e-14), none in the abscissae. **Cause demonstrated**: GSL's
  driver relinked with PETIR's `cos` alone makes all 5 086 generated entries
  and all 2 808 `glfixed_point` values bit-identical to PETIR's. The Newton
  start is `cos(pi (4i-1)/(4n+2)) t0`; a last-bit difference in `cos` changes
  the iterate path, the loop stops at `eps = 1e-10`, and the weight
  `2/((1-x^2) p'^2)` is evaluated at the pre-step iterate. Accepted per
  `AGENTS.md` (platform libm differences are accepted, counted and pinned);
  it is a larger spread than "1 ulp" because of that amplification, which I
  had not predicted. The 11 integral differences are all in generated
  tables, none in a precomputed one.
- **`ltbl[k] == 1.0 - 1.0/k`** (the C comment): **holds**, bit for bit, for
  every `k` in 2..=1023 (unit test `ltbl_is_one_minus_reciprocal`).
- **Upstream accuracy of generated tables, measured** (unit test
  `glfixed::tests`): the 1025-point rule integrates `x^2` over `[0, 3]` to
  9.000000002742878 (3e-10 relative), because GSL stops Newton at
  `eps = 1e-10`. PETIR reproduces that, being a port. Recorded, not changed.
- **P4 (QAGP) held** (31/31 and 31/31, above the 90 %/80 % predicted), **with
  a limit on what it shows**: the stale-index roundoff test
  (`i > 10` where `i == nint`) is **not discriminated** by any case. A negative
  control that replaced it by the iteration count still passed all 31 cases
  (9 cases with 3 sub-ranges and 600 iterations, 4 noise-level integrands).
  The prediction in this file that `wavy_12pts` / `sharp_12pts` exercise it
  was therefore wrong: with `nint = 11` the upstream form is true from the
  first iteration and the iteration-count form from the twelfth, and the
  variant still passed (those runs apparently never count `roundoff_type3`
  high enough to change a decision; I did not instrument that). The line
  stands on reading `qagp.c`.
- **P5 (QAWC) held** (22/22, above the 80 % predicted): both `qc25c` branches,
  reversed limits, a roundoff status (`sqabs_cusp`, GSL 18), an
  iteration-limit status and the argument errors are all in the reference.
  Two cases I meant as roundoff cases (`limit1`, `tight_roundoff`) instead
  hit `GSL_EBADTOL` (their `epsrel` is under `50 eps`); they still pass but do
  not test what their names say.

## Decisions deferred to the maintainer

- `PetirError` has one `Tolerance` variant, documented as `GSL_ETOL` (14), but
  `qags`/`qagi`/`qagp`/`qawc` return it for `GSL_EBADTOL` (13) (the test
  mapping has always said 13). QNG needs both. A new variant would edit the
  kovan-reviewed enum (`fn:08f93c13acb0345f`, in `src/review.md`), so
  `QngOutcome::gsl_status` carries GSL's integer instead. A `ToleranceNotReached`
  variant (or a doc correction on `Tolerance`) is the maintainer's call.
- `qags.rs`: `Workspace`, `ExtrapolationTable` and their helpers became
  `pub(super)` (visibility only, no body changed) so QAGP and QAWC re-use the
  verified engine. No `review.md` in `src/integration/` holds a review block.

## Romberg (written 2026-11-13, prediction committed BEFORE the port is run)

- **Port:** `src/integration/romberg.rs`, from `romberg.c` lines 41-164.
  **Reference:** `reference-data/gsl/romberg_reference_driver.c` (20 cases).
  **Replay:** `tests/gsl_romberg_code_to_code.rs`.
- **P6.** Status, `*neval` and the integrand call count identical in 20 of 20;
  result bit-identical in 20 of 20. The routine calls no libm function itself
  (only `fabs`), and the test integrands are glibc on both sides, so no
  platform allowance is expected to be needed. It fails if the row recurrence
  grouping `(four_j*Rc[j-1] - Rp[j-1]) / (four_j - 1)`, the `a + j*h` abscissae
  or the row swap is mis-transcribed. The `n > 30` cap is not observable
  (a non-converging 30-row run is 5e8 evaluations) and is not tested.

### Romberg results (2026-11-13)

**P6 held.** 20 of 20 cases: GSL status (`EDOM` 1, `EMAXITER` 11), `*neval` and
the integrand call count identical, and 20 of 20 results bit-identical
(`tests/gsl_romberg_code_to_code.rs`; includes five non-converging runs, a
reversed interval, a zero-width interval and the four argument errors).

## QAWS (written 2026-11-13, prediction committed BEFORE the port is run)

- **Port:** `src/integration/qaws.rs` (`QawsTable`, `qaws`) from `qaws.c`,
  `qc25s.c` and `qmomo.c`; re-uses `qcheb.rs`, the QAGS `Workspace`, and
  `append_interval` from `qagp.rs` (made `pub(super)`).
  **Reference:** `reference-data/gsl/qaws_reference_driver.c` (12 table
  allocations, 16 integrations). **Replay:** `tests/gsl_qaws_code_to_code.rs`.
- **P10.** (a) `QawsTable::new` accepts or refuses all 12 parameter sets as
  `gsl_integration_qaws_table_alloc` does (PETIR also refuses `alpha = -1`,
  which GSL accepts and then divides by zero; no such set is in the reference).
  (b) GSL's integer status, workspace size and `neval` identical in 16 of 16.
  (c) The Chebyshev moment arrays: bit-identical for the table with
  `alpha = beta = 0` (powers of two are exact), at least 50 % of all 4 x 25 x
  8 entries overall; the rest differ by `pow(2, alpha+1)` (glibc against
  PETIR's). (d) Results bit-identical in at least 8 of 16 (every integrand
  path calls `pow` or `log`), and where not, within 1e-12 relative; abserr
  within 1e-6 relative.
