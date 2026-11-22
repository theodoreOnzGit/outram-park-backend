# Upstream source

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


~~**N/A — original work.**~~ **CORRECTED 2026-11-10 (gh:#1433):** mostly original work, but
`src/lib/beta_testing/z_domain/discrete_tf.rs` is ported in part from the GNU Octave
control package (`inst/filt.m`, control 4.2.2+, commit `f39e2625`, GPL-3.0-or-later,
Copyright 2009-2016 Lukas F. Reichlin), as its own header records. This crate (a PID / transfer-function process-control
library for real-time simulators) has no single upstream repository it forks
or translates. It draws on standard chemical-engineering process-control
theory (textbook PID tuning, transfer-function block algebra) rather than a
specific existing codebase.

If a genuinely reusable upstream reference implementation is identified later
(e.g. a specific open-source process-control library this crate is modelled
on), document it here with the same fields the other crates' `upstream_source/README.md`
files use (repository URL, license, commit, provenance, licensing note).

**Differential run (2026-11-13, gh:#1433):** `z_domain` was compared against a run of
Octave 8.4.0 with control 4.0.1 (the version `apt` provides; the cited 4.2.2+ commit
`f39e2625` could not be installed). See
`verification_and_validation/octave_differential/`.
