<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore
Part of Outram Park (outram-park-backend), outram-foam-appbuilder-lib.
-->

# AI-fleet review manifest — Sod shock tube plottable CSV

> ⚠️ **UNTRUSTED AI DRAFT.** The changes below were produced by an AI assistant
> (Claude Opus 4.8) and are untrusted draft material until a human reviews them
> (see workspace `RESPONSIBLE_USE.md` / `AI_USAGE.md`). Nothing here flips a V&V
> or human-interface completeness flag.

## Run

- **Date:** 2026-07-17 (Asia/Singapore)
- **Branch / base:** `develop`, based on `origin/develop` `71b63e5`.
- **Scope:** only `crates/outram-foam-appbuilder-lib/` — the Sod shock tube V&V
  test and its V&V output. No other crate touched.

## What changed

1. **`tests/sod_shock_tube_validation/main.rs`** — added, to the existing
   `rho_central_foam_matches_sod_table_ii` test:
   - `write_plottable_csv(...)` — a **non-fatal** writer targeting the committed
     sub-folder `verification_and_validation/sod_shock_tube_validation/`
     (filesystem errors log to stderr instead of failing the physics test).
   - `l2_linf(...)` — discrete $L_2$ (RMS) and $L_\infty$ (max-abs) error norms.
   - A per-cell CSV emission block: evaluates the analytic exact Riemann
     solution at **every cell centre** and writes columns
     `x_m, rho_numerical, u_numerical, p_numerical, rho_exact, u_exact, p_exact`,
     with the whole-field $L_2$/$L_\infty$ norms (SI + relative-to-peak) as
     `#`-comment header lines. Written on every `cargo test --release` run.
   - The existing Table II CSV writer and all three tests are unchanged in
     behaviour (still pass).
2. **`verification_and_validation/sod_shock_tube_validation/sod_shock_tube_profile_vs_exact_riemann.csv`**
   — the generated plottable dataset (100 cells). Committed (small; the crate
   `.gitignore` only excludes `*.csv` directly under `verification_and_validation/`,
   not this sub-directory).
3. **`verification_and_validation/sod_shock_tube_validation/RESULTS.md`** — new
   V&V record (methodology + measured results) for the whole-field comparison
   against the exact Riemann solution.
4. **This manifest.**

## Measured results (release build, 2026-07-17)

All three Sod tests **pass**. Whole-field error of the port vs the exact
Riemann solution over 100 cell centres at $\tau = 0.2$:

| variable | $L_2$ (rel. peak) | $L_\infty$ (rel. peak) |
|---|---|---|
| density  | 1.41 % | 8.61 % |
| velocity | 5.17 % | 48.04 % |
| pressure | 1.06 % | 7.45 % |

Table II faithful-station worst errors (asserted): $p = 0.43\%$, $u = 0.96\%$,
$\rho = 0.43\%$. The large $L_\infty$ figures are single cells straddling the
shock/contact (expected shock smearing) — see `RESULTS.md` for the honest
interpretation.

- **CSV path:**
  `crates/outram-foam-appbuilder-lib/verification_and_validation/sod_shock_tube_validation/sod_shock_tube_profile_vs_exact_riemann.csv`

## Human-verify list

- [ ] Confirm the per-cell numerical/exact overlay looks physically right when
      plotted (monotone rarefaction, contact, shock; correct wave positions).
- [ ] Confirm the $L_\infty$ interpretation (single-cell jumps, not a defect) is
      an acceptable framing, or request a mesh-refinement convergence study.
- [ ] Confirm committing the CSV (vs gitignoring) is the desired policy for this
      sub-folder; adjust `.gitignore` / `Cargo.toml exclude` if not.
- [ ] Re-read `main.rs` diff for the new helpers (no `Box`/`dyn`/lifetimes added;
      release-only; provenance headers intact).
