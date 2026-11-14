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

### QAWS results (2026-11-13, Rust 1.99.1 release, GSL 2.8 `cf180cd7`, `gcc -O2`, glibc)

`tests/gsl_qaws_code_to_code.rs`: 12 `TERR` rows, 8 tables, 17 integrations.

| item | identical |
|---|---|
| table accepted / refused (12) | 12/12 |
| status, workspace size, `neval` (17) | 17/17 |
| Chebyshev moments (8 tables x 100) | **675/800** (6 tables 100 %; `(0.5, 0.5, 1, 1)` 16 %; `(1.5, -0.3, 1, 0)` 59 %) |
| results (17) | **13/17** bit-identical, the other 4 within 1e-12 |
| abserr (17) | **9/17** bit-identical |

- **P10 (a), (b), (c) held; (d) half failed.** Moments 84 % (>= 50 % predicted)
  and results 13/17 (>= 8 predicted) held. The prediction that abserr agrees
  to 1e-6 **failed**: `both_log` differs by 8.9e-6 and `mix` by 4.7 %
  (`mix`'s abserr is 8e-16, at the rounding floor, so a last-bit change in a
  moment moves it by percents). The test therefore checks abserr only to a
  factor of two for the non-identical cases, and says why.
- **Cause demonstrated, not assumed**: GSL's driver relinked with PETIR's
  `pow` and `log` (diagnostic shim, not committed) gives **800/800 moments,
  17/17 results and 17/17 abserr bit-identical** to this port. Everything
  that differs from stock GSL is glibc's `pow`/`log` against PETIR's; the
  logic, including `qc25s`'s three branches, is faithful.
- **GSL defect found, not copied**: with `limit = 2` GSL's `qaws` appends a
  third interval to a workspace of two (size 3, status 5 observed): an
  out-of-bounds write. PETIR returns `Invalid` for `limit < 2` and, for
  `limit = 2`, when the third write fails. `limit = 3` and above are
  compared.

## QAWO and QAWF (round 3, 2026-11-14; predictions committed BEFORE the comparison is run)

- **Port:** `src/integration/qawo.rs` (`QawoTable`, `qawo`, `qawf`) from GSL
  2.8 `qawo.c`, `qawf.c`, `qc25f.c`, `qmomof.c` (`ampl/gsl@cf180cd7`, read
  2026-11-14). Re-uses the QAGS engine (`Workspace`, `ExtrapolationTable`,
  `qelg`, `qpsrt`, `update`, `increase_nrmax`, `large_interval`), `qcheb`,
  `append_interval` (QAGP), `kronrod(Qk15)` and `qagiu` (for QAWF with
  `omega = 0`). No body of a reviewed function changed.
- **Reference:** `reference-data/gsl/qawo_reference_driver.c` against GSL 2.8
  built **without running `configure`** (`reference-data/gsl/build_gsl_nocfg.sh`:
  a hand-written `config.h`, headers linked, `gcc -O2` over the `Makefile.am`
  source lists of `integration/ err/ sys/ specfunc/`). Replay:
  `tests/gsl_qawo_code_to_code.rs`. Cases: 12 moment-table rows (the `|par|
  <= 24` boundary-value branch, the `> 24` recursion branch, tables that cross
  24 as the level halves `par`, a negative `omega`, and a `set_length` /
  `set` sequence), 28 QAWO integrations (both `qc25f` branches, extrapolation,
  `limit` 1/2/4/10, `GSL_ETABLE` with 1/3/6 levels, `EBADTOL`, `omega = 0`,
  zero integrand, noise) and 18 QAWF integrations (cos/sin, negative and zero
  `omega`, `EBADTOL`, small `limit`, a `cycle_limit` below `limit`, a
  non-decaying integrand).
- **Design choice that makes the cause testable:** the test integrands call
  no `sin`/`cos` (only `exp`, `ln`, `sqrt`, rational), so the only `sin`/`cos`
  in either program are GSL's weights in `qmomof.c` and `qc25f.c`. A GSL
  relink with PETIR's `sin`/`cos` alone (`petir_sincos_shim.rs`, diagnostic,
  not committed as a result) then isolates exactly the libm difference.
- **Pass criteria:** GSL's integer status, workspace size(s) and the number
  of integrand calls **exactly**; moments, results and `abserr` counted
  bit-identical and the counts pinned (1-ulp libm differences accepted,
  counted and pinned per `AGENTS.md`); a non-identical result must be within
  1e-8 relative.

**Predictions (P11 QAWO / tables, P12 QAWF), written before any run:**

- **P11a.** Status, workspace size and integrand-call count identical in
  28 of 28 QAWO cases. They are decided by comparisons of error estimates
  that a last-bit `sin`/`cos` change could flip only in a marginal case; I
  expect none.
- **P11b.** Moments: libm's `sin`/`cos` (fdlibm lineage) and glibc's differ in
  the last bit on a few percent of arguments, and every moment of a level
  depends on `sin(par)` and `cos(par)`. Prediction: at least 70 % of the 1 000
  table entries bit-identical, the rest different by a small multiple of an
  ulp for `|par| > 24` (forward recursion is mildly unstable) and larger for
  the `|par| <= 24` tridiagonal solve.
- **P11c.** QAWO results bit-identical in at least 17 of 28, `abserr`
  identical in at least 14 of 28; every non-identical result within 1e-8
  relative.
- **P12.** QAWF: status, both workspace sizes and call count identical in at
  least 16 of 18; results bit-identical in at least 9 of 18 (it sums many QAWO
  calls at different `omega`-cycle lengths, so more `sin`/`cos` arguments).
  `cycle_lt_limit` gives `EINVAL` from every inner call, as in GSL.
- **P13 (cause).** With GSL relinked to PETIR's `sin`/`cos` alone, **all**
  moments, results and error estimates become bit-identical to the port
  (100 %), so every difference from stock GSL is libm. If it does not, the
  port has a logic defect and the residual is not libm.

### QAWO / QAWF results (2026-11-14, Rust 1.99.1 release, GSL 2.8 `cf180cd7`, `gcc -O2`, glibc, x86-64)

`tests/gsl_qawo_code_to_code.rs`: 12 table rows, 28 QAWO integrations, 18 QAWF
integrations; two tests, one against stock GSL and one against GSL relinked to
PETIR's libm.

| item | stock GSL (glibc) | GSL + PETIR `sin`/`cos`/`pow` |
|---|---|---|
| Chebyshev moments (1 000 values) | **959/1000** | **1000/1000** |
| QAWO status, size, integrand calls (28) | 28/28 each | 28/28 each |
| QAWO results / `abserr` (28) | **23/28**, **23/28**; worst result 1.13e-15 rel (`limit4`, `limit10`), worst `abserr` ratio 1.000008 | 28/28, 28/28 |
| QAWF status, both workspace sizes (18) | 18/18 | 18/18 |
| QAWF integrand calls (18) | **17/18** (`tiny_eps`: 2160 against GSL's 2070) | 18/18 |
| QAWF results / `abserr` (18) | **15/18**, **16/18**; worst result 5.4e-16 rel (`omega50`), worst `abserr` ratio 32.5 (`tiny_eps`) | 18/18, 18/18 |

- **P11a held** (28/28 on status, size and calls). **P11b held on the count**
  (959/1000 = 95.9 %, 70 % predicted) **and failed on the size of the
  difference**: the prediction that the rest differ by "a small multiple of an
  ulp" is wrong for the `|par| <= 24` branch. The differing entries are at
  `par = 2.5` (`mid_cos` level 1, `mid_sin` level 3) and `par = 0.875`
  (`seq_set` level 1); the worst is `-4.115456182072121e-2` against GSL's
  `-4.115456182062424e-2` (2.4e-12 relative), because the tridiagonal solve
  amplifies the 1-ulp difference in `sin`/`cos` (`sin 2.5`: glibc
  `0x1.326af0dcfcab1p-1`, `libm` crate `0x1.326af0dcfcabp-1`; `cos 0.875`:
  `...eb3p-1` against `...eb4p-1`). **P11c held** (23 and 23 of 28; at least 17
  and 14 predicted). **P12 held** (17/18 on calls, at least 16 predicted;
  15/18 results, at least 9 predicted).
- **P13 held: every difference from stock GSL is libm.** GSL relinked to
  PETIR's `sin`, `cos` and `pow` gives 1000/1000 moments and 28/28 and 18/18
  on every value. Two traps found while isolating it, both recorded in
  `petir_sincos_shim.rs` and `build_gsl_nocfg.sh`: (1) `gcc -O2` fuses
  `sin(x)` and `cos(x)` on one argument (`qmomof.c`, `qc25f.c`) into a single
  `sincos` call, so a shim of `sin` and `cos` alone changed the QAWO results
  but left every moment table at stock values (959/1000 for the wrong reason);
  (2) a shim archive listed before `libgslnocfg.a` on the link line is never
  pulled in, because nothing before it references `sin`. `pow` enters through
  the `omega = 0` QAWF case, which calls `qagiu`, whose `rescale_error` uses
  `pow` (the round-2 finding for QNG/QAGI): that case's `abserr` differs in
  the last bit until `pow` is linked from PETIR. The shim-linked output is
  committed as `reference-data/gsl/qawo-gsl-2.8-petir-sincos-pow-reference.txt`
  and replayed as a bit-identity gate.
- **Port detail checked by a case, not by reading:** `GSL_MAX_DBL(a, b)` is
  `a > b ? a : b`, which returns `b` when either operand is NaN, whereas
  Rust's `f64::max` returns the other operand. `invsqrt_nan_sin20`
  (`1/sqrt(x)` from 0: the Chebyshev nodes include the endpoint, `inf * 0 = NaN`)
  reaches `GSL_ETABLE` with a NaN `abserr` and matches. The port therefore
  uses a `gsl_max` helper. **The earlier ports (`qags.rs`, `qagp.rs`,
  `qawc.rs`, `qaws.rs`, `qng.rs`, `romberg.rs`) use `f64::max` where GSL uses
  `GSL_MAX_DBL`**; no NaN-integrand case was in their references, so this
  difference is unmeasured there (child issue filed; not changed here because
  the `qags` engine is shared and its verified behaviour must not move without
  a comparison).
- **Process notes (what changed after the first run of the driver, and why).**
  The first driver run used `1/sqrt(x)` or `log(x)` on `[0, ...]` in 11 QAWO
  cases; `f(0) = inf` poisons the Chebyshev expansion (NaN `abserr`), so those
  cases tested only NaN propagation. Before the Rust test had ever run, I
  changed 10 of them to `sqrt(x)` / `log(x + 0.001)` and kept one as
  `invsqrt_nan_sin20` (and
  `invsqrt_cos3` in QAWF, which also starts at 0, stays). The predictions were
  committed before the driver's first run and were not edited.
- **Build:** GSL is built by `reference-data/gsl/build_gsl_nocfg.sh` without
  running the cloned tree's `configure` (hand-written `config.h`, 20
  `HAVE_DECL_*`/feature macros plus `RETURN_IF_NULL`). Not checked: that this
  build matches an autoconf build bit for bit on the earlier families (the
  earlier references were made with `build_gsl_family.sh`); the new
  reference's integrand-independent parts, e.g. `qk15` arithmetic, agree with
  the port at 100 % under the shim, which is the evidence available.

## CQUAD (round 3, 2026-11-14; predictions committed BEFORE the comparison is run)

- **Port:** `src/integration/cquad.rs` from `cquad.c` (lines 30-683), tables
  `cquad_tables.rs` copied from `cquad_const.c` by
  `reference-data/gsl/gen_cquad_tables_rs.py` (10 tables, 3 807 values, never
  typed). Copyright (C) 2010 Pedro Gonnet. **Reference:**
  `reference-data/gsl/cquad_reference_driver.c` (59 rows: the 25 test
  integrands of GSL's own `test.c`, tolerance sweeps, workspaces of 3-10
  intervals, NaN/inf-valued integrands, divergent integrals, refusals,
  degenerate intervals; 57 integrations and 2 refused workspaces).
  **Replay:** `tests/gsl_cquad_code_to_code.rs`.
- **What differs from stock-GSL arithmetic:** nothing is expected. `cquad.c`
  calls only `sqrt`, which is correctly rounded on both sides, and the
  integrands call glibc in the driver and in the Rust test alike.
- **Pass criteria:** GSL's status, `*nevals` and the integrand-call count
  exactly; result and `abserr` bit-identical (the counts are pinned at 100 %;
  no libm allowance is invoked).

**Predictions, written before any run:**

- **P14a.** Status, `nevals` and integrand calls identical in 57 of 57
  `CQUAD` rows; result and `abserr` bit-identical in 57 of 57. A miss would be
  a logic defect (heap fix-up order, the downdating recurrence) or a mismatch
  between the C and Rust copies of an integrand, not libm.
- **P14b.** GSL's own acceptance test (`test.c:2437-2442`: relative error
  below 1e-12, actual error below 5 `abserr`, status 0) holds for 25 of 25
  integrands in the port, as it does in GSL.
- **P14c.** The downdating path is exercised: `sqrt_signed`, `log_signed`,
  `nan_band`, `nan_mid`, `recip_*` and `inf_at_1` return status 0 or
  `GSL_EDIVERGE`; `1/x` over `(0, 1]` returns status 0 in the port (seen in a
  first unit test, which had assumed the opposite) and GSL must agree.
  `1/x^2` over `(0, 1]` returns `GSL_EDIVERGE` (22) in GSL.
- **P14d.** `GSL_EBADTOL` (13) for the four refused-tolerance rows, with no
  output written (result, `abserr`, `nevals` all 0). A workspace below 3
  intervals is refused (`WSALLOC ... 0`, two rows).

### CQUAD results (2026-11-14, Rust 1.99.1 release, GSL 2.8 `cf180cd7`, `gcc -O2`, glibc, x86-64)

`tests/gsl_cquad_code_to_code.rs`: 57 integrations and 2 refused workspaces.

| item | identical |
|---|---|
| status (57) | **57/57** |
| `*nevals` and integrand calls (57) | **57/57** each |
| result (57) | **57/57 bit-identical** |
| `abserr` (57) | **57/57 bit-identical** |
| GSL's own `test.c` acceptance, 25 integrands (rel < 1e-12, error < 5 `abserr`, status 0) | 25/25 |

- **P14a, P14b, P14c, P14d held, all of them.** No libm allowance was needed:
  `cquad.c` calls only `sqrt`. The matrices of 1 089 coefficients, the heap
  fix-ups, the interval dropping at workspace sizes 3, 4, 5, 6, 8 and 10 (with
  `abserr` between 5e-4 and 1.35), and the downdating recurrence (`sqrt_signed`
  has NaN on half the range, `nan_band`, `nan_mid`, `log_signed`, `recip_*`
  with infinite nodes) all reproduce GSL's bits and its evaluation counts.
- **Behaviours of GSL worth knowing, now pinned** (they are in the reference, not
  in the port): `1/x` over `(0, 1]` and over `[-1, 1]` return status 0 with
  `abserr = inf` after 196 729 and 197 131 evaluations (the "success" is
  hollow: check `abserr`); `1/x^2` over `(0, 1]` is `GSL_EDIVERGE` after 240
  evaluations, with `result = +inf` and `abserr` unwritten (0 in the driver);
  `1/(1-x)` on `[0, 1]` returns `36.5` with `abserr 7.29` (status 0).
- **Deviation not exercised by any case:** upstream's `int nans[32]` overflows if
  all 33 nodes of an interval are non-finite (undefined behaviour); the port
  uses a `Vec`. Not compared, by construction.

## FIXED (`gsl_integration_fixed`, round 3, 2026-11-14; predictions committed BEFORE the comparison is run)

- **Port:** `src/integration/fixed.rs` (`FixedType`, `FixedWorkspace`) from
  `fixed.c` (alloc, `fixed_compute`, `imtqlx`) and the nine type files
  `legendre.c chebyshev.c chebyshev2.c gegenbauer.c jacobi.c laguerre.c
  hermite.c exponential.c rational.c`. `gsl_sf_gamma` is PETIR's
  `specfunc::gamma` (already compared with GSL); `pow` is `Real::powf`. GSL's
  `eigen_symmv` is **not** involved: `fixed.c` has its own EISPACK/IQPACK
  `imtqlx` (the round-2 note that `fixed` "needs eigen_symmv" was wrong,
  **CORRECTED 2026-11-14**; checked by reading `fixed.c` and grepping the nine
  type files for `gsl_eigen`/`gsl_linalg`: no hit).
- **Reference:** `reference-data/gsl/fixed_reference_driver.c`: 270 accepted
  rules (9 types, 2-3 parameter sets each, orders 1, 2, 3, 4, 5, 8, 10, 16, 25,
  40; Legendre also with reversed limits) and 18 refused parameter sets. Per
  rule: every node and weight, and four integrals (`1`, `x^2`, `x^5`,
  `1/(1+x^2)`; no libm function in the integrands). **Replay:**
  `tests/gsl_fixed_code_to_code.rs`.
- **Pass criteria:** accept/refuse identical in all 288 rows; nodes, weights
  and integrals counted bit-identical per type and pinned. Libm functions in
  GSL's path: `pow` (`fixed_compute`: `pow(slp, al + be + 1)`; `jacobi.c`:
  `pow(2, absum + 1)`, `pow(.., 2.0)`) and `gsl_sf_gamma` (all types but
  Legendre, Chebyshev, Chebyshev2, Exponential).

**Predictions, written before any run:**

- **P15a.** Accept/refuse identical in 288 of 288.
- **P15b.** Nodes bit-identical in every row of every type: `x = shft + slp *
  eigenvalue`, and the eigenvalues depend on `sqrt` (exact) and arithmetic
  only. The one exception I can foresee is Jacobi, whose Jacobi matrix calls
  `pow(y, 2.0)`: `gcc -O2` rewrites that to `y * y` while the port calls
  `powf(y, 2.0)`; I predict they agree on every `y` in these rows, and if a
  Jacobi node differs this is the cause to check first.
- **P15c.** Weights: Legendre (`pow(slp, 1)`) and Chebyshev (`pow(slp, 0)`)
  bit-identical in 100 %; Exponential and Chebyshev2 in at least 95 % (a
  square, and `zemu` is exact). The gamma-using types (Gegenbauer, Jacobi,
  Laguerre, Hermite, Rational): at least 50 % of weights bit-identical, worst
  relative difference at most 1e-12.
- **P15d.** Integrals are bit-identical wherever the nodes and weights of the
  rule are.
- **P15e (cause).** With GSL relinked to PETIR's `exp`, `log`, `pow`, `sin`,
  `cos` (`petir_libm_shim.rs`), **all** nodes, weights and integrals are
  bit-identical (100 %), except possibly Jacobi's `pow(y, 2.0)` as in P15b.
