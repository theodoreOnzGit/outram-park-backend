# Nu and yield tables on their own interpolation regions (GitHub #365 audit)

*2026-09-29. Research, education and V&V only; not for any operational,
licensing or safety use (see `RESPONSIBLE_USE.md`).*

## What was wrong

A tabulated function in ENDF and ACE carries `(NBT, INT)` interpolation
regions: 1 histogram, 2 lin-lin, 3 lin-log, 4 log-lin, 5 log-log. OpenMC
evaluates every such table with `Tabulated1D::operator()` (`src/endf.cpp`):
- each region's own law;
- the end values outside the table, never zero.

Before this change the nu and yield tables here did the following:

| table | route | before |
|---|---|---|
| total nu (NU, MF=1/452) | ACE | a non-lin-lin region was refused by name |
| total nu (NU, MF=1/452) | ENDF | regions **dropped silently**, read lin-lin |
| delayed nu (DNU, MF=1/455) | both | regions **dropped silently** |
| group probability `p_k(E)` (BDD, MF=5/455) | both | regions **dropped silently** |
| `\|TY\| > 100` yields (MT=5, the other neutron channels) | ACE | a non-lin-lin region was refused |
| MF=6 neutron yields | ENDF | regions **dropped silently** |

## What changed

- **One evaluator** does all of it:
  `njoy_outram_park_fork::nuclear_data::secondary::tabulated1d_at` (and
  `tabulated1d_at_xy`). It is OpenMC's `Tabulated1D::operator()`, line for line:
  - the `lower_bound_index` bin, which takes the left-hand value at a
    repeated abscissa;
  - the 0-based `NBT` test (`i < NBT - 1`);
  - the same formulas for INT 1–5.
- **Where the regions are carried:**
  - `NuBar::interp`;
  - `DelayedNuBar::interp` and `DelayedChiGroup::fraction_interp` (ENDF route);
  - `AceDelayed::{nu_delayed_interp, group_fraction_interp}` (ACE route);
  - `DelayedData::{nu_delayed_interp, group_fraction_interp}` (transport);
  - `ContinuumBranch::yield_interp`;
  - `OtherYield::Tabulated { pairs, interp }`.
- **Lin-lin tables are untouched.** A lin-lin table (no regions, or every
  `INT = 2`) keeps the arithmetic it always had, so every table in use
  evaluates **bit-identically**. `is_lin_lin` is the switch.
- **One narrow refusal remains:** a single (prompt) NU block plus DNU, where
  either carries a non-lin-lin region. Here the total is prompt + delayed.
  The sum of two such tables is not one tabulated function on either's
  regions, and `NuBar` holds one table. No producer is known: ACER writes a
  lone NU block beside DNU only for a prompt-only evaluation, and every nu
  table in ENDF/B-VIII.0 is lin-lin.

## Methodology

**Census (2026-09-29), over ENDF/B-VIII.0:**
- Every nu table is lin-lin: 86 MT=452, 86 MT=456 and 84 MT=455.
- Of 4950 first neutron MF=6 subsections, 4946 have lin-lin yields. The other
  4 are all F-19 (MT=16/22/28/91), with histogram yields that are constant.

So no real table can show the difference, and a table was **constructed**.
`openmc_inputs/interp_regions_reference.py` takes NJOY2016 U-235 (293.6 K,
ENDF/B-VIII.0, `reference-data/ace`) and appends four blocks:

| block | construction | regions (`INT`) |
|---|---|---|
| NU | prompt + total form, on U-235's own grid; prompt = total − 0.0165 | 5 regions: 1, 2, 3, 4, 5 |
| DNU | U-235's delayed table | 2 regions: 4, 1 |
| BDD | group 1 made energy-dependent, `p1 (1 + 0.5 x)` along the DNU grid; groups 2–6 as they were | 2 regions: 5, 3 |
| MT=5 yield | U-235's own `TY = −101` yield | 3 regions: 1, 5, 2 |

It then writes `target/ace_extra/U235_regions.ace`.

The script asserts that OpenMC's ACE reader sees each block's regions. The
reference values are OpenMC's C++ rule, re-implemented in the script (`tab1d`),
and it agrees with Python's `Tabulated1D` to **0.0** at every interior probe.

**Probes:** each table gets 12 geometric midpoints between grid points, spread
across the table, plus one energy below the table and one above it.

**The BDD reference** is `p_g(E) / Σ_g p_g(E_first)`, which is the evaluated
table under this crate's renormalisation. For an energy-dependent `p_g`,
OpenMC's ACE reader does something else:
1. it evaluates `nu_d(E) p_g(E)` on the union grid;
2. it re-tabulates that product lin-lin (`reaction.py:338-347`);
3. it does not renormalise unless some group is constant.

That is an approximation of the product, not a reading of the table, so it is
**deliberately not reproduced**. No held table has an energy-dependent `p_g`:
U-235's six groups are constant.

**Pass criterion:** |rel| ≤ 1e-12 on all 56 values, read through the
transport path:
- `Nuclide::nu_bar`;
- `DelayedData::nu_delayed_at`;
- `DelayedData::group_fraction`;
- `Nuclide::mt5_yield`.

A zero reference (MT=5 below its threshold) is compared absolutely.

**Power:** in every quantity, at least one probe must differ from the lin-lin
reading by more than 1e-4 relative.

## Results (2026-09-29)

Test: `crates/outram-mc-libs/tests/ace_interp_regions_vs_openmc.rs`.

| quantity | values | worst \|rel\| vs OpenMC's rule | largest lin-lin error |
|---|---|---|---|
| total nu | 14 | 0.0 | 6.4e-3 |
| delayed nu | 14 | 0.0 | 2.0e-1 |
| group-1 share | 14 | 1.9e-16 | 5.7e-2 |
| MT=5 yield | 14 | 0.0 | 1.7e-1 |

**Mutation:** forcing `is_lin_lin` to return `true`, i.e. the old behaviour,
fails at the first probe inside a histogram region: total nu 2.40894 against
2.39850.

**ENDF route:** `endf_route_carries_f19_histogram_yield` checks F-19's MT=91
branch. It carries `yield_interp = [(2, 1)]`, a histogram, as the evaluation
states. Because the yield is constant, this pins that the regions are carried,
not a number.

**Interpretation.** No k in this workspace moves. Every table in use is
lin-lin, and lin-lin tables evaluate bit-identically. What this closes are the
silent drops: DNU, BDD and the ENDF-route regions used to be read lin-lin
whatever the evaluation said, and nothing reported it.
