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
