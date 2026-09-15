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
  when the embedded excerpt is a sample rather than the whole table).
  **Whether these are ignored depends on where they sit**, and both globs are
  top-level-only:
  - Directly under `verification_and_validation/` — **gitignored** (see
    `.gitignore`) and **excluded from `cargo publish`** (see `Cargo.toml`'s
    `exclude`, which uses the same `verification_and_validation/*.csv` glob).
  - Inside a per-case **sub-folder** (e.g.
    `sod_shock_tube_validation/…csv`) — matched by *neither* glob, so such a
    CSV **is committed and is packaged into the published crate**. Keep those
    small and deliberate.

  Regenerate either kind by re-running the verification test/example that
  produced it; don't hand-edit.

See `outram-park-fork-coolprop/verification_and_validation/` for a worked
example (`water_critical_point_iapws95.md`).

