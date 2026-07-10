# Verification & validation

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

Name it for the comparison it documents, e.g. `water_critical_point_iapws95.md`.
Structure:

```markdown
# <Title>

**Generated:** 2026-07-10T14:32:00Z (ISO 8601, UTC — when this comparison was run)
**Crate version / commit:** <git short hash or crate version at generation time>

## Methodology

What is being computed, the reference/benchmark it is judged against, the
inputs (state points, material, data source), the tolerance, and the pass
criterion — the same content a test's doc comment would have, written for a
reader who doesn't have the source open.

## Reference

BibTeX entry(ies) for the benchmark/reference data, **with page and/or table
number** so a reader can find the exact number being checked against:

​```bibtex
@book{wagner2002iapws,
  author = {Wagner, W. and Pru{\ss}, A.},
  title  = {The IAPWS Formulation 1995 for the Thermodynamic Properties of
            Ordinary Water Substance for General and Scientific Use},
  journal = {J. Phys. Chem. Ref. Data},
  volume = {31},
  number = {2},
  pages  = {387--535},
  year   = {2002},
  note   = {Critical point value, Table 13.1, p. 429}
}
​```

## Results

​```csv
quantity,computed,reference,units,rel_error
p_critical,22064000.00000115,22064000,Pa,5.2e-14
​```

Prose interpretation: what the numbers mean, whether the pass criterion was
met, and any caveat about the comparison's limits.
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
