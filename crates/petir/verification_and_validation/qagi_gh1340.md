# QAGI / QAGIU / QAGIL against compiled GSL 2.8 (gh:#1340)

Parent: gh:#1340, epic gh:#1336. Research, education and V&V only
(`RESPONSIBLE_USE.md`). AI-assisted work: untrusted until a human reviews it.

## Methodology

- **Port:** `src/integration/qagi.rs`, from GSL 2.8 `integration/qags.c` lines
  52-192 (`gsl_integration_qagi`, `_qagil`, `_qagiu` and the `*_transform`
  functions), commit `cf180cd7`, read 2026-11-13. They are transformations
  that call the existing QAGS engine (`qags_core`, already bit-identical to
  GSL) with the `qk15` rule. `qags_core` became `pub(super)`; no body change.
  Note that this is GSL's QAGI, not QUADPACK's `dqagi` (which has a dedicated
  transformed rule `dqk15i`); GSL has no `qk15i`.
- **Reference:** `reference-data/gsl/qagi_reference_driver.c`, GSL 2.8 rebuilt
  from `ampl/gsl@cf180cd7` (`gcc -O2`, glibc, x86-64), output
  `reference-data/gsl/qagi-gsl-2.8-reference.txt`: 18 cases (convergent,
  iteration limit, divergent, bad tolerance), per case status, result, abserr,
  workspace size and number of integrand evaluations.
- **Replayed by** `tests/gsl_qagi_code_to_code.rs`. The integrands are
  rational or use `sqrt`/`exp`/`pow`/`ln`; the test is a `std` build, so those
  reach the same glibc as the driver (as in `gsl_qags_code_to_code.rs`).
- **Pass criteria:** status, workspace size and evaluation count **exactly**
  (they identify the path through the routine); result and abserr reported
  bit-identical, with the same relative 1e-12 / 1e-3 bars as the QAGS test
  for a libm-only difference.

## Prediction (written 2026-11-13 BEFORE the port was run against the reference)

- **P1.** Status, size and neval identical in 18 of 18 cases.
- **P2.** Results bit-identical in 18 of 18 (the QAGS engine is
  bit-identical to GSL and the transforms keep upstream's grouping).
- **P3.** Error estimates bit-identical in at least 16 of 18; any miss is in an
  `exp`/`pow`/`ln` case and is the libm effect documented for QAGS
  (`abserr` is a difference of large numbers), not a port difference.

## Results (2026-11-13, Rust 1.99.1 release, GSL 2.8 `cf180cd7`, `gcc -O2`, glibc)

- **P1 held**: status, workspace size and evaluation count identical in
  18 of 18 cases (statuses 0, 11, 13 all reached).
- **P2 held**: results **18 of 18 bit-identical**.
- **P3 held**: error estimates **17 of 18 bit-identical**. The one miss is
  `gauss` (`exp(-x*x)`): 1.0306660836945013e-12 against GSL's
  1.0306660836946047e-12, relative 1.0e-13. As for QAGS, `abserr` is a running
  difference of larger numbers amplifying a last-ulp difference in
  `rescale_error`'s `pow(200 err / resasc, 1.5)`, which the engine evaluates
  with PETIR's `libm` and GSL with glibc. That is the mechanism measured for
  QAGS (gsl_qags_code_to_code.rs). ~~I did **not** re-test it for this case (by
  nudging GSL's `pow`), so it is the likely cause, not a demonstrated one.~~
  **CORRECTED 2026-11-13:** re-tested. GSL's `qagi` driver relinked with
  PETIR's `pow` alone prints `gauss` abserr 1.03066608369450132e-12, exactly
  PETIR's value, so the cause is demonstrated
  (`integration_family_gh1340.md`).
  The result and evaluation count are identical, and the value is inside the
  1e-3 bar the QAGS test uses.
- Still open in gh:#1340: `qng`, `qagp`, `qawc`, `qaws`, `qawo`, `qawf`,
  `cquad`, `romberg`, `fixed`/`glfixed`.
