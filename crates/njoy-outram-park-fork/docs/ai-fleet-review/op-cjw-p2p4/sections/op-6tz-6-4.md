# op-6tz.6.4 — ENDF delayed-neutron readers + mdgxs verification tests

**Status: DONE** for the ENDF delayed-neutron *data readers* and the
directly-verifiable mdgxs-part-i operations; **PARTIAL** for the mdgxs ops that
require a group-collapse engine or a transport solve (kept `#[ignore]` with the
missing capability named).

Date: 2026-07-15. Data: ENDF/B-VIII.0 neutron sublibrary (open-source, NNDC/IAEA),
U-235 `n-092_U_235-ENDF8.0.endf` (MAT 9228), already in the repo at
`crates/njoy-outram-park-fork/tests/resources/`.

## Files changed

| File | Change |
|---|---|
| `src/nuclear_data/delayed.rs` | **New** (467 lines). ENDF MF=1/455 + MF=5/455 readers. |
| `src/nuclear_data/mod.rs` | `pub mod delayed;` + re-exports (`DecayConstant`, `DelayedNuBar`, `DelayedChi`, `DelayedChiGroup`); module-doc bullet. |
| `tests/openmc_notebooks_data/mdgxs_part_i.rs` | 3 scaffolds turned **live**. |
| `tests/openmc_notebooks_data/mdgxs_part_ii.rs` | 2 ops kept `#[ignore]`; reasons updated (data now readable, collapse/transport still missing). |

No other files touched. `src/endf/`, `src/lib.rs`, `src/modules.rs`,
`tests/.../main.rs` unmodified (mdgxs modules already declared).

## Provenance

- **ENDF-6 Formats Manual** (ENDF-102 / BNL-203218-2018-INRE, 2018): §1.6
  (MF=1/MT=455 record layout — HEAD `LDG`/`LNU`, LDG=0 `NNF` decay-constant LIST,
  delayed ν̄_d TAB1) and §5 (MF=5 subsection `LF` laws — LF=5 general evaporation,
  LF=1 arbitrary tabulated). ENDF is an open, published format; the reader is
  written from the spec, not ported from NJOY.
- **NJOY2016** commit `ac5adf5` `groupr`/`acer` delayed-ν̄ handling was consulted
  only to confirm the LDG/LNU/LF branch semantics; no NJOY code is reproduced.
- Reuses the crate's generic ENDF record parsers (`src/endf/records.rs`:
  `SectionCursor::{read_cont,read_list,read_tab1,read_tab2}`). GPLv3 provenance
  header is in the module `//!` doc.

## What the readers do

- `DelayedNuBar::from_endf(tape, mat)` — MF=1/455. `LDG=0` (energy-independent
  decay constants, the standard actinide form) fully supported: reads the `NNF`
  precursor λ \[s⁻¹\] LIST + delayed ν̄_d(E) (`LNU=2` TAB1, or `LNU=1` polynomial
  sampled onto a log grid). `LDG=1` parsed spec-faithfully (lowest-energy λ set,
  flag exposed) but **unverified** — no LDG=1 fixture in-repo. Named uom alias
  `DecayConstant = uom::si::f64::Frequency`; `decay_constant(g)` returns it.
- `DelayedChi::from_endf(tape, mat)` — MF=5/455. `NK` subsections; `LF=5`
  (general evaporation, θ(E) + tabulated g) and `LF=1` (arbitrary tabulated)
  supported. Any other `LF` → `Ok(None)` (aborts, never fabricates). Per-group
  `fraction` p_k(E), `normalization()` (trapezoidal ∫), `min_density()`.

## DONE vs PARTIAL — mdgxs ops

| Notebook op | Test | State | Why |
|---|---|---|---|
| `mgxs.DecayRate` | `mdgxs_part_i::precursor_decay_rates` | **LIVE** | MF=1/455 reader |
| `mgxs.Beta` / `DelayedNuFissionXS` | `mdgxs_part_i::delayed_beta_and_nu_fission` | **LIVE** | ν̄_d/ν̄_total + p_k |
| `mgxs.ChiDelayed` | `mdgxs_part_i::delayed_chi_spectrum` | **LIVE** | MF=5/455 reader |
| delayed-group condensation | `mdgxs_part_ii::delayed_group_condensation` | `#[ignore]` | needs GROUPR-style delayed-group MGXS collapse (not ported) |
| precursor concentration + export | `mdgxs_part_ii::precursor_concentration_and_export` | `#[ignore]` | needs transport tallies (`outram-mc-libs`) |

The two `#[ignore]` ops now name the *remaining* gap (collapse engine / transport)
rather than "no data" — the delayed data itself is now read in part-i.

## Measured U-235 results (methodology + numbers)

Reference = the same ENDF/B-VIII.0 U-235 evaluation the notebook's OpenMC
`mgxs.DecayRate`/`mgxs.Beta` read; njoy parses it directly off the tape.

- **Decay constants λ (MF=1/455, LDG=0, 6 groups)** \[s⁻¹\]:
  `[0.013336, 0.032739, 0.12078, 0.30278, 0.84949, 2.853]`.
  Properties asserted: exactly 6 groups; each λ ∈ [0.01, 3]; **strictly
  increasing** with group index; each within **rel 1e-3** of the reference above.
- **Delayed fraction β** = ν̄_d(0.0253 eV)/ν̄_total(0.0253 eV)
  = 0.015850 / 2.429850 = **0.006523**; asserted ∈ [0.006, 0.007] (accepted U-235
  nominal β ≈ 0.0065). This is the ENDF *nominal* β, not transport-adjoint β_eff.
- **Per-group delayed fractions p_k(0.0253 eV)** (MF=5/455 subsection weights):
  `[0.03501, 0.18070, 0.17251, 0.38678, 0.15858, 0.06643]`, **Σ = 1.000000**;
  β_k = β·p_k asserted to sum back to β.
- **χ_delayed (MF=5/455, LF=5 general evaporation, θ ≡ 1)**: 6 groups, all
  sampled densities ≥ 0, per-group trapezoidal normalization **0.99474–0.99882**
  (≈ 1; small tape-rounding deficit), asserted ∈ [0.98, 1.02].

## Tests + properties

Lib unit tests (`src/nuclear_data/delayed.rs`, synthetic hand-built rows):
- `parses_mf1_455_ldg0_six_groups` — LDG=0/LNU=2 round-trip: 6 λ exact, uom
  accessor matches, ν̄_d clamp/interp, out-of-range group → `None`.
- `parses_mf5_455_lf5_two_groups` — two LF=5 groups: LF tag, non-negativity,
  triangular spectrum normalizes to 1.0.
- `unsupported_lf_returns_none` — LF=7 aborts the parse to `None` (no fabrication).

Integration tests (`tests/openmc_notebooks_data/mdgxs_part_i.rs`, real U-235 tape):
the 3 live tests above (methodology + measured numbers in each `///` doc).

## cargo test results (2026-07-15, `--release`, 12 GB ulimit)

Lib target — `cargo test -p njoy-outram-park-fork --lib --release`:
```
test nuclear_data::delayed::tests::parses_mf1_455_ldg0_six_groups ... ok
test nuclear_data::delayed::tests::parses_mf5_455_lf5_two_groups ... ok
test nuclear_data::delayed::tests::unsupported_lf_returns_none ... ok
```
Full lib run: **320 passed, 2 failed** — the 2 failures
(`covr::boxer::tests::symmetric_round_trip_and_mirror`,
`mixr::driver::tests::driver_matches_direct_mix`) are **other subagents'**
in-flight modules in this shared worktree, not this change. My delta: **+3 lib
tests**, all green.

Notebook target —
`cargo test -p njoy-outram-park-fork --test openmc_notebooks_data --release mdgxs`:
```
test mdgxs_part_i::precursor_decay_rates ... ok
test mdgxs_part_i::delayed_chi_spectrum ... ok
test mdgxs_part_i::delayed_beta_and_nu_fission ... ok
test mdgxs_part_ii::delayed_group_condensation ... ignored
test mdgxs_part_ii::precursor_concentration_and_export ... ignored
test result: ok. 3 passed; 0 failed; 2 ignored; 15 filtered out
```

`delayed.rs` is 467 lines (< 1000 cap); this change compiles with 0 warnings
(the only lib warning, `covr/boxer.rs`, is another subagent's module).
