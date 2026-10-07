# Verification & validation

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


This folder holds this crate's **verification-and-validation (V&V) records**:
one markdown file per benchmark/comparison, each documenting **both the
methodology and the results** (per the workspace-root `CLAUDE.md`'s mandatory
V&V rule) with the generated-vs-reference data embedded directly in the file.

This is a **durable record**, not a live report — it captures what was checked,
against what, and what the numbers were, at the time it was written. It
complements (does not replace) the crate's own `tests/` — a V&V doc explains
*why* a test's tolerance is what it is and *what the reference actually says*,
in more depth than a doc-comment on the test function itself typically allows.

## Convention for each file

Name it for the comparison it documents, e.g. `<topic>_vs_<reference>.md`.
Structure:

```markdown
# <Title>

**Generated:** <ISO 8601 timestamp, UTC — when this comparison was run>
**Crate version / commit:** <git short hash or crate version at generation time>

## Methodology

What is being computed, the reference/benchmark it is judged against, the
inputs, the tolerance, and the pass criterion.

## Reference

BibTeX entry(ies) for the benchmark/reference data, with page and/or table
number so a reader can find the exact number being checked against.

## Results

A CSV table (computed vs reference vs relative error) plus prose
interpretation of what the numbers mean and whether the pass criterion was met.
```

## What's committed vs gitignored

- **The `.md` files themselves are committed** — the narrative, methodology,
  BibTeX, and a representative CSV *excerpt* embedded as a fenced code block
  are the durable, human-readable record and stay small.
- **Standalone `.csv` files** (a full benchmark dataset a `.md` references,
  when the embedded excerpt is a sample rather than the whole table) are
  **gitignored** (see `.gitignore`) and **excluded from `cargo publish`** (see
  `Cargo.toml`'s `exclude`). Regenerate them by re-running the verification
  test/example that produced them; don't hand-edit.

See `outram-park-fork-coolprop/verification_and_validation/` for a worked
example (`water_critical_point_iapws95.md`).

## Geometry images

- [`htr10_geometry_images/`](htr10_geometry_images/README.md) — the assembled
  HTR-10 explicit-TRISO core drawn as PNG (R-Z, x-y at bed / conus / cavity,
  zooms down to one TRISO particle), with what was and was not checked in
  them. Regenerate with `cargo run --release -p nee_soon --example
  htr10_geometry_images` whenever the geometry changes.

## HTR-10 k against loading height

- [`htr10_endf8_kvsh_quick_2026-10-02/`](htr10_endf8_kvsh_quick_2026-10-02/README.md)
  and [`htr10_endf8_kvsh_heavy_2026-10-02/`](htr10_endf8_kvsh_heavy_2026-10-02/README.md) — the whole ENDF/B-VIII.0 sweep from
  `examples/htr10_endf8_kvsh_quick.rs` / `_heavy.rs` (gh:#501), against RMC
  and both MCNP columns, with the figure script that drew them.
- [`htr10_seker_2026_10_01_10k/`](htr10_seker_2026_10_01_10k/README.md) — the
  same sweep on VIII.0 and VII.0 at 10 000 × [5 + 135], run through
  `htr10_rmc_keff`.
  Measured on a majorant under-bound 14× at 661 eV (gh:#589).
- [`htr10_seker_2026_10_05_majorant_fix/`](htr10_seker_2026_10_05_majorant_fix/README.md):
  the re-measurement on the bounded majorant (gh:#589). It covers 4 of the
  heights on VIII.0 and 1 on VII.0, at 10 000 × [5 + 20], and includes a
  same-code control that switches back to the old majorant.
- [`htr10_seker_2026_10_07_10k/`](htr10_seker_2026_10_07_10k/README.md) —
  **the current record**: all 22 points (N = 10 to 20, VIII.0 and VII.0) on
  the bounded majorant at 10 000 × [5 + 135]. Supersedes the two above.
- [`htr10_run_all.sh`](htr10_run_all.sh) — the reusable launcher for the
  sweep (`htr10_run_all.sh <out_dir>`; statistics, heights, libraries and
  slots set by environment variables listed in its header).
