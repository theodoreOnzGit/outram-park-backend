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
- **`.md` files under `generated/` are committed too** — see below. The rule is
  the same either way: markdown is the record and is committed; `.csv` is not,
  at any depth.

## `generated/` — machine-written reports

`generated/` holds V&V reports **written by the diagnostic tests themselves**,
not by a human. They are committed like every other `.md` here, and are
regenerable in seconds:

```bash
cargo test --release -p tampines-steam-tables --lib backward_eqn_chebyshev_experimental
```

Although both are committed, they are different kinds of artifact and the
distinction matters:

- The hand-written files are a **durable, human-reviewed record**, authored
  once and kept. They are the trust workflow described in the banner at the top.
- The `generated/` files are a **live measurement dump** — they are rewritten
  wholesale on every run and always describe the code as it is right now. They
  carry a "do not hand-edit" banner naming the command that regenerates them,
  and a `Status` section stating plainly that they are measurements rather than
  a validation sign-off.

Because they are committed, a diff on `generated/` after a code change is a
useful review signal in its own right: it shows exactly how the measured
accuracy moved.

Currently `generated/` covers the experimental non-IAPWS Chebyshev backward
correlations in `src/backward_eqn_chebyshev_experimental/` (see GitHub issue
#34): Region 5 `T(p,h)`/`T(p,s)`, the near-critical Region 4 `(h,s)` flash, and
`p(rho,h)` across the regions including a report on why the Region 1 inversion
is ill-conditioned at low pressure.

A generated report is **not** a substitute for a hand-written V&V case. If one
of these correlations is ever to be described as validated, that needs a
committed `.md` here and a human sign-off, per the banner.

See `outram-park-fork-coolprop/verification_and_validation/` for a worked
example (`water_critical_point_iapws95.md`).

