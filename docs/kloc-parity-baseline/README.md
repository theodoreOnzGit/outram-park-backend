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

**Every figure in this capture reproduces the manuscript exactly.** Run with
`--clone --check`, all eight drift-check deltas are `+0`:

| Quantity | Manuscript | Measured | Delta |
|---|---|---|---|
| `baseline_total_lines` | 303,463 | 303,463 | +0 |
| `baseline_code_lines` | 181,298 | 181,298 | +0 |
| `baseline_active_days` | 367 | 367 | +0 |
| `agentic_code_lines` | 175,997 | 175,997 | +0 |
| `subtotal_translated` | 136,462 | 136,462 | +0 |
| `subtotal_original` | 27,177 | 27,177 | +0 |
| `subtotal_extension` | 12,358 | 12,358 | +0 |
| `n_crates` | 26 | 26 | +0 |

That makes this a gate on the published numbers, not merely on the script's
self-consistency.

### The first capture was thrown away, and why

An earlier attempt ran without `--clone`, so `thermal_hydraulics_rs`,
`chem-eng-real-time-process-control-simulator` and `teh-o-prke` were absent from
the machine. It still produced a complete-looking set of files — and the numbers
were wrong in a specific, instructive way. Because an extension crate subtracts
its standalone pre-agentic original, a **missing baseline repository silently
moves those lines into the agentic total**: the baseline read 162,163 code lines
instead of 181,298, and the agentic total read 187,378 instead of 175,997,
inflated by very nearly the amount the baseline lost. The script warns about
exactly this, and refuses to validate its own drift check when a repo is
missing.

Worse for a parity fixture, that capture never exercised the TUAS
net-of-predecessor subtraction at all, which is among the most intricate logic
in the script. A fixture that skips the hard part is not a gate.

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
