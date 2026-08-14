# `kloc_accounting` parity baseline — frozen Python output

**These files are a test fixture, not a source of truth.** They are the exact
output of `scripts/kloc_accounting.py` captured on **2026-08-14**, immediately
before that script was ported to Rust (`kovan kloc`) and deleted.

## Why they exist

The workspace's "no Python for documentation or accounting" hard rule requires a
**parity gate** on any such port, and requires generating the old output *before*
deleting the script when it is not otherwise committed. Epic `op-yz7b` — the
historian/token-usage port — shipped without one, and that is recorded in
`CLAUDE.md` as a known weakness. This is the gate that stops the same gap
recurring here.

The Rust implementation must reproduce these byte-for-byte. `git diff --quiet`
on a regenerated copy is the check.

## What was captured

| File | Bytes | What it is |
|---|---|---|
| `baseline_repositories.csv` | 477 | pre-agentic repositories, the baseline |
| `agentic_crates.csv` | 5,686 | per-crate agentic line counts |
| `summary.txt` | 4,961 | the console report |
| `baseline_table.tex` | 2,788 | `tab:preagentic_baseline` |
| `rate_table.tex` | 1,435 | lines-per-active-day table |
| `agentic_table.tex` | 3,553 | `tab:agentic_crates` |

Headline numbers in that run: **187,378 agentic code lines**, **349,541 total
Rust code lines in `crates/`**, **6,246 code lines per active day**.

## What is NOT captured

`fig_kloc_productivity.png` — the matplotlib figure. It was skipped
(`--no-figure`) deliberately: the Rust port emits **SVG** rather than a 200-dpi
raster, so a byte comparison is meaningless across the format change. The
figure is gated instead on the numbers it plots, which are the ones in
`summary.txt` and the CSVs above, plus a visual check of the rendering.

## A caveat on re-running

These numbers are measured from **live git history**, so re-running the
measurement at a later date legitimately produces different values — the
workspace keeps growing. Parity means "the Rust reproduces the Python **on the
same inputs**", which is why this snapshot is frozen here rather than
regenerated. Do not update these files to make a later run match.
