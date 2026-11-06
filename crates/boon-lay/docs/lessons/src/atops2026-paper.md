# Tutorial case: reproducing the TRISO-ATOPS 2026 paper's release results

> **Research, education and V&V only** ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> This is library verification of boon-lay's TRISO-ATOPS port. Nothing on
> this page is a source term for any plant.
> **Review status:** AI-assisted draft, 2026-10-22, not yet human-reviewed.

**Extended deep dive, tutorial case.** Covers
[`triso_atops_fork::variant`](../../api/boon_lay/triso_atops_fork/variant/index.html)
and the test that reproduces the paper,
[`tests/mhtgr_stoyer_workflow.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/tests/mhtgr_stoyer_workflow.rs).
Tracking issue: [gh:#1015](https://github.com/theodoreOnzGit/outram-park-backend/issues/1015).
The full record, with every attempt, is
[`docs/triso-atops-2026-paper-reproduction.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/triso-atops-2026-paper-reproduction.md).

The paper is B. Stoyer, A. Raichart and D. Petti, "TRISO-ATOPS: A Mechanistic
Source Term Model for Gas-Cooled Reactors", *Nuclear Technology* (2026),
published online 17 August 2026,
[doi:10.1080/00295450.2026.2671624](https://doi.org/10.1080/00295450.2026.2671624).
It is restricted literature. This page cites it by printed page, and quotes
only the values the test compares. No text, figure or table of it is
reproduced.

## The question

**Can we reproduce the paper's printed release numbers?** And if we can, which
model produces them: the equations the paper writes down, or the code it
describes?

boon-lay's `triso_atops_fork` is a port of INL's TRISO-ATOPS code at upstream
commit `de374c8`, verified against it code to code. The paper describes that
code. Reading the paper against the port turned up ten places where they
differ (gh:#1015). So the port now carries both, as a model variant:

| Variant | What runs | Id |
|---|---|---|
| `TrisoAtopsVariant::Upstream` | the ported upstream code, bit for bit. **The default.** | `"upstream"` |
| `TrisoAtopsVariant::ATOPS2026PAPER` | the paper's equations as written, `Atops2026Paper(PaperReadings::AS_WRITTEN)` | `"atops2026paper"` |

On the Code Review map:
[`PlantConstants::new`](../../code-review/#crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs::PlantConstants::new)
and
[`PlantConstants::with_variant`](../../code-review/#crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs::PlantConstants::with_variant)
choose the variant for normal operation;
[`booth_radius_m`](../../code-review/#crates/boon-lay/src/triso_atops_fork/variant.rs::booth_radius_m)
holds the three readings of the Booth radius.

> **Predict first.** The paper's authors wrote the code. If the paper's
> printed tables were computed with it, which variant should reproduce them?

## The setup: the paper's MHTGR cases, inputs as printed

The test runs the paper's own worked cases, with inputs as the paper prints
them (values in cited CSV files beside the test, in
[`verification_and_validation/mhtgr_stoyer/`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/mhtgr_stoyer/README.md)):

| Case | What it is | Inputs (printed page) | Compared against |
|---|---|---|---|
| A | MHTGR, 350 MWth, normal operation then a heat-up | failure fractions Table 2 and constants Table 3 (p. 7); inventories Table 6 (pp. 14-15); core temperatures Table 7 (p. 15); heat-up curves Fig. 5 (p. 15) | pools Table 9 (pp. 17-18); initial and final releases Table 10 (p. 18) |
| B | MHTGR, 600 MWth | fractions Table 12 (p. 21); inventories Table 11 (pp. 20-21); temperatures Table 7 + 250 K | Table 13 (pp. 22-23); Table 14 (p. 23) |
| Table 19 | complete failure of every particle, 1250 °C, 3 years | s.III.C (p. 21); Table 17 (p. 24); inventories Table 16 (p. 24) | Table 19 (p. 25) |

Three details matter for reading the results:

- **The plate-out constant.** Table 3 prints `k_plate = 7.5e-5 /s`. gh:#413
  found that upstream's own default, `7.5e-4 /s`, reproduces every printed pool,
  so the printed value is probably a misprint. Both are run. The counts below
  use `7.5e-4`.
- **Case A's grid.** Each radial section's inventory is spread evenly over 14
  axial nodes (p. 13), giving a 14 × 3 grid.
- **The accident.** Fig. 5's curves are the maintainer's digitisation,
  resampled to 0.1 h. Finals therefore count as reproduced within 10 %. Pools
  and the initial release count within 2 %, the paper's three significant
  figures.

## The headline finding

**The paper's printed release numbers were computed by the upstream code, not
by the equations as the paper writes them.** `Upstream` reproduces them.
`atops2026paper` does not.

### Table 19 (p. 25), complete failure at 1250 °C

| Nuclide | Paper | `upstream` | rel. diff | `atops2026paper` | rel. diff |
|---|---|---|---|---|---|
| Xe-133 | 2.23e-2 | 2.232e-2 | +0.1 % | 2.232e-2 | +0.1 % |
| Kr-85 | 5.85e-1 | 5.855e-1 | +0.1 % | 9.525e-1 | **+62.8 %** |
| Kr-88 | 1.98e-2 | 1.984e-2 | +0.2 % | 1.984e-2 | +0.2 % |
| I-131 | 2.54e-2 | 2.538e-2 | −0.1 % | 2.538e-2 | −0.1 % |
| I-133 | 1.30e-2 | 1.296e-2 | −0.3 % | 1.296e-2 | −0.3 % |
| Cs-134 | 1.00 | 1.000 | 0.0 % | 1.000 | 0.0 % |
| Cs-137 | 1.00 | 1.000 | 0.0 % | 1.000 | 0.0 % |
| Sr-90 | 9.89e-2 | 9.882e-2 | −0.1 % | 4.344e-2 | **−56.1 %** |

The two misses have clear causes. Kr-85 is long-lived. The paper sends
long-lived gases to the Booth Eq. (6c); the code uses the empirical Eq. (6a).
Sr-90's miss comes from the paper's larger Booth radius, 150.8 µm against the
code's 65.3 µm. The predictions, written before the run, were +60 % and −55 %.

### Case A, normal operation and accident, `k_plate = 7.5e-4`

Each cell is "reproduced / compared".

| Variant | graphite | circulating | plate-out | HPS | initial release | final ±10 % (median ours/paper) |
|---|---|---|---|---|---|---|
| `atops2026paper` | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 3/46 (1.21) |
| `upstream` | 40/40 | 64/64 | 52/52 | 24/24 | 46/46 | 24/46 (0.90) |

Case B is the same to within a count or two. Upstream's finals sit at a
median of 0.90, the factor gh:#413 recorded for the digitised Fig. 5. Every
pool and initial release is reproduced.

## The ten differences between the paper and the code

Each row is one field of `PaperReadings`. The paper column is the text's
reading, the default of `atops2026paper`.

| # | Item | Paper (printed page) | Upstream code |
|---|---|---|---|
| a | Booth radius `a`, normal operation | RMS of kernel radius and grain size, 150.8 µm (pp. 7-8, 26) | `√(2·a_grain·r)`, 65.3 µm |
| b | Booth radius, accident Eq. (19) | the same `a` (pp. 8, 11-12) | kernel radius `r`, 213 µm |
| c | Yttrium | Sr group (Table 1 p. 6; p. 9) | "other" metal |
| d | Below a diffusion law's range | nearest in-range temperature, every law (pp. 8-9) | clamped for Rb/Cs, Sr/Ba/Eu, Ag graphite only |
| e | Infinite series | summed to convergence (pp. 7-8, 10-12) | fixed term counts; transient Booth zeroed below 1e-6 |
| f | "Other" fission metals | Booth on `D = 1e-19 m²/s` (pp. 6-7, 9) | fixed `<R/B> = 1e-5` |
| g | Palladium, accident | silver's laws (Table 1 p. 6; p. 11) | Booth on the Ag kernel law |
| h | Long-lived noble gases, halogens, Te, Se | Booth Eq. (6c) on Kr's `D` (pp. 5, 9) | empirical Eq. (6a) |
| i | Long-lived birth rate | reactor run time, Eqs. (4b), (4d) (pp. 4-5) | irradiation time |
| j | Accident fuel inventory | plate-out and HPS × `t_irrad/t_run`, no branch, Eqs. (22), (23), (28) (pp. 12-13) | unscaled, with a branch |

Not different, so not changed: the venting trapezoid (Eqs. (26)-(27)), the
`1e8` attenuation cap, Table 4's Eq. (6a) constants, the half-life screens and
the coolant balances.

## Ambiguities: ranked readings, fixed before any run

Some of these items are ambiguous. Item (a) is the clearest example: the paper
calls the radius the RMS of kernel and grain "diameter" on p. 7, and of kernel
"radius" and grain size on p. 8 and in the nomenclature (p. 26).

The tempting move is to try each reading and keep whichever matches the
printed numbers. That would make the printed numbers an input, and the
comparison would stop being a check. The maintainer's rule instead fixes the
**protocol**:

1. List the candidate readings of each ambiguity **before any run**, ranked
   by how well the text supports them. 13 ambiguities were listed, A1 to A13.
2. Make the best-supported reading the default.
3. Try the others one at a time against the printed tables, and keep every
   attempt, including the misses.
4. If the text's reading disagrees with the printed numbers, keep it, and
   flag the disagreement.

**The radius, resolved by the text.** Three readings of `a` were ranked for
Case A's 213 µm kernel and 10 µm grains:

| Rank | Reading | `a` | Why |
|---|---|---|---|
| 1 | RMS of kernel radius and grain size | 150.8 µm | the nomenclature's definition (p. 26) and p. 8's. The only reading inside p. 8's own bound, between the kernel's radius and its grain size. Smaller than the kernel radius, as p. 24 says of it. |
| 2 | RMS of the diameters | 301.3 µm | p. 7's and p. 24's wording, but larger than the kernel radius, so outside p. 8's bound |
| 3 | upstream's geometric mean | 65.3 µm | the code's value; not a reading of the text |

Rank 1 became the default because the text supports it. The printed numbers
were not used to choose it.

**What the runs showed.** For six ambiguities (A1 radius, A2 accident radius,
A3 other metals, A4 long-lived volatiles, A5 birth time, A9 yttrium) the
code's reading, not the text's, moves a printed number toward the paper. Each
is flagged, and the text's reading stays the default. No single candidate
reproduces the paper. A combination of the code's readings of A1, A3, A4 and
A5 brings normal operation within reach (43/46 initial releases), but then its
accident finals run 2.6× high, because the paper's accident Booth uses that
same, now smaller, radius. Only upstream's split reproduces both: 65.3 µm in
normal operation and the kernel radius in the accident. No reading of the text
gives that split. The search stopped at the end of the pre-written list, as
the protocol says.

## The source code that reproduces it

Everything below is pulled from the test file when the site is built
([`tests/mhtgr_stoyer_workflow.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/tests/mhtgr_stoyer_workflow.rs)),
so it is the code that ran. To run it yourself (about a minute in release):

```bash
cargo test --release -p boon-lay --test mhtgr_stoyer_workflow \
  atops2026paper_against_the_papers_printed_releases -- --nocapture
```

It prints the tables of this page. The run record is
[`atops2026paper_reproduction_2026-10-22.txt`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/mhtgr_stoyer/atops2026paper_reproduction_2026-10-22.txt).

### Step 1: a run is a variant and a parent policy

A `Setup` pairs the model variant with the nuclide-selection policy. The
pre-#1015 tests all run `UPSTREAM`.

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:setup}}
```

### Step 2: normal operation, with the variant on the plant constants

[`port_case_nodal_with`](../../code-review/#crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::port_case_nodal_with)
builds the plant constants from the cited `constants.csv`.
`PlantConstants::new` gives `Upstream`. `.with_variant` switches the model:

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:plant_constants}}
```

It then selects the nuclides under the run's parent policy. The paper's
reading (A11) is `ParentDecayPolicy::ShortLivedParentOnly`: a daughter is fed
by its parent only when the parent is short-lived (pp. 10-11). Upstream's
table flag is `UpstreamTableDefault`.

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:select_nuclides}}
```

Each node then goes through
[`normal_operation_node`](../../code-review/#crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs::normal_operation_node),
which reads `plant.variant` for every choice in the table above.

### Step 3: the accident, through the `*_for` functions

The accident path has no plant struct, so the variant is an argument.
[`port_accident_curve_with`](../../code-review/#crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::port_accident_curve_with)
integrates `D` over each Fig. 5 curve, then takes the release fractions.
Each `*_for` function calls the ported function itself under upstream
readings.

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:accident_release_fractions}}
```

Then, for each node, what is left to release:

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:accident_release_activity}}
```

On the Code Review map:
[`integrate_diffusion_over_time_for`](../../code-review/#crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::integrate_diffusion_over_time_for),
[`release_fraction_transient_for`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/mod.rs::release_fraction_transient_for)
and
[`release_activity_for`](../../code-review/#crates/boon-lay/src/triso_atops_fork/accident/mod.rs::release_activity_for).

### Step 4: the paper variant, and every ranked reading

`paper_with` changes one reading from the as-written default:

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:paper_variant}}
```

[`configurations`](../../code-review/#crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::configurations)
is the pre-written candidate list, in order: the as-written variant, each
ambiguity moved to its next candidate, the one combination written down
before the run, and upstream.

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:configurations}}
```

[`score_case`](../../code-review/#crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::score_case)
runs one case under one configuration and counts what lands within 2 % (pools,
initial release) or 10 % (finals).

### Step 5: Table 19

[`table19`](../../code-review/#crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::table19)
builds the paper's complete-failure run: one node at 1250 °C, 3 years, every
particle failed, no HPS.

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:table19}}
```

The comparison printed above comes from the end of
[`atops2026paper_against_the_papers_printed_releases`](../../code-review/#crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::atops2026paper_against_the_papers_printed_releases):

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:table19_comparison}}
```

### Step 6: the switches are the only difference

If the paper variant is given every reading as the code makes it, it must be
`Upstream`, exactly. This test pins that, on both MHTGR cases, for every pool
and every accident total:

```rust,ignore
{{#include ../../../tests/mhtgr_stoyer_workflow.rs:bit_for_bit}}
```

[`paper_variant_with_upstream_readings_is_upstream_bit_for_bit`](../../code-review/#crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::paper_variant_with_upstream_readings_is_upstream_bit_for_bit)
passed on 2026-10-22 with exact equality. Without it, a difference between
the variants could come from a bug in the plumbing as well as from the
readings.

**Why a test and not an example.** The test already prints this page's
tables with `--nocapture`. An example would have to share the test's harness,
several hundred lines, or duplicate it.

## What this teaches

- **A printed number has a provenance.** These came from the code. A
  reimplementation from the equations alone would miss Kr-85 by +63 % and
  Sr-90 by −56 % in Table 19, and would be right to.
- **Fix the protocol, not the criterion.** The readings were ranked from the
  text before any run, and the defaults did not move after the runs. That is
  why the answer "the text's model does not reproduce the paper" means
  something: the comparison was able to come out the other way.
- **Ports start faithful and may deviate for a reason.** `Upstream` stays the
  pinned default, so the code-to-code verification still holds. The paper's
  model is an explicit, documented option beside it, with each difference
  cited.

> **Check yourself.** Item (a) moves the Booth radius from 65.3 µm to
> 150.8 µm. For a long-lived metal at short `D·t/a²`, the release goes as `a⁻¹`.
> Roughly what factor should the release change by? (The record gives
> ÷ 2.29 for Eq. (6c).)

For the physics of each piece, see the chapter 5 lessons:
[diffusion laws](../../science/fission-products/atops-diffusion.html),
[release models](../../science/fission-products/atops-release-models.html)
and [accidents](../../science/fission-products/atops-accident.html).
