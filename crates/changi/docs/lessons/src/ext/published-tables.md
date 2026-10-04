# Published HTR-10 tables: reference data, not inputs to tune

> **Extended deep dive.** **Research, education and V&V only**
> (`RESPONSIBLE_USE.md`): none of these numbers may be quoted as a release or
> a dose for HTR-10 or any plant for an operational, licensing or safety
> purpose.

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).

## What is held, and where

Seven tables from one paper: Liu Yuanzhong and Cao Jianzhu, *Nuclear
Engineering and Design* **218** (2002) 81–90, an HTR-10 radiological
assessment. The paper is **restricted access**: the tables are cited and
transcribed with provenance, the paper is not redistributed, and it is not in
the open corpus. Activity tables live in `changi` (the dispersion chain is
their consumer); the two dose tables live in `buangkok` (dose is kept out of
the dispersion crate). Each CSV's provenance is in the crate's
`docs/References.md`.

| Table | What | Crate::loader |
|---|---|---|
| 1 | Equilibrium-core inventory, 22 nuclides (Bq) | `changi::activity::inventory::{htr10_equilibrium_core, htr10_core_inventory}` |
| 2 | Release rate from the fuel elements, normal operation (Bq h⁻¹ MWt⁻¹) | `changi::activity::fuel_release::htr10_fuel_element_release_rate_bq_per_h_per_mwt` |
| 3 | Primary-helium activity at end of a 20-year life (Bq) | `changi::activity::primary_helium::{htr10_primary_helium_end_of_life, htr10_primary_helium_activity}` |
| 5 | Annual normal-operation airborne release (Bq/a) | `changi::activity::airborne_release::{htr10_normal_operation_annual_release, htr10_annual_airborne_release}` |
| 7 | Individual effective dose vs distance, normal operation (mSv/a), an AIRDOS-EPA result | `buangkok::published::normal_operation_dose_by_distance` |
| 8 | Airborne release for two design-basis accidents (Bq per accident); the paper's "C-4" stored as C-14 | `changi::activity::accident_airborne_release::{htr10_accident_release, htr10_accident_airborne_release, AccidentCase}` |
| 9 | Individual thyroid and whole-body dose vs distance, the same two accidents (mSv), a STOERNEU result | `buangkok::published::accident_dose_by_distance` (re-exports `AccidentCase`) |

## How they are used: comparison targets and stated inputs

The workspace rule is that a reference is a **check**, never a tuning input.
Each use says which it is:

| Use | Table(s) | Role |
|---|---|---|
| Capstone (rung 7) | 1 (inventory), 3 (circulating activity, printed beside the release) | stated **inputs** of a bounding case |
| `buangkok`'s Liu & Cao cross-check (gh:#379, rung 6) | 5, 8 in; 7, 9 compared | inputs and **comparison targets**; "consistent, not verified" |
| `sembawang::lwr_comparison` | 8 (design-basis arm), 9 (`htr10_dba_vs_table9`) | a published arm and its cross-check |
| `htgr_sim_v1` (another track's simulator) | 1, 2, 3 | inputs, and an uncalibrated-release comparison against Table 2 (gh:#399) |

**Context, not validation.** Tables 7 and 9 are the outputs of other codes
(AIRDOS-EPA, STOERNEU) with site meteorology the paper does not give. Agreement
with them would be code-to-code consistency at best; disagreement is
investigated, never tuned away.

## The two loaders and their parsing

The two-column CSVs share `inventory::parse_nuclide_bq_csv`; Table 8's
two-case CSV has its own three-column parser. All are compiled in with
`include_str!`, so there is no file I/O at run time and the crates stay
wasm- and Android-clean.

## Coverage

| Item | Section |
|---|---|
| `changi::activity::{inventory, fuel_release, primary_helium, airborne_release, accident_airborne_release}` | the table |
| `buangkok::published::{normal_operation_dose_by_distance, accident_dose_by_distance}` | the table |
