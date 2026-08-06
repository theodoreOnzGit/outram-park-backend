# Review manifest — op-6tz: openmc-notebooks verification scaffold (outram-mc)

<!-- op-jis-historical-note -->
> ⚠️ **HISTORICAL RECORD — the statistics below predate `op-jis` (noted 2026-08-06).**
> Every measured number in this manifest was produced **before** bead `op-jis`
> added OpenMC's PCG-RXS-M-XS output permutation to `rng::lcg::prn` on
> 2026-08-06. The LCG **state recurrence was not changed**, so integer-state
> facts still hold, but every statistic derived from the sampled **uniform
> values** — k values and their σ, tallies, fractions, σ-distances — **no longer
> reflects the current generator**. This is a dated review record, so its numbers
> are deliberately **left exactly as they were measured** and are *not* rewritten
> here. Do not cite them as current; current values live in the crate's V&V docs
> and test doc comments.

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**

Everything in this changeset was produced by an AI assistant (Claude Opus 4.8,
1M context) and is **untrusted draft material** until a human has reviewed it.
Nothing here is a validated result; the one live test asserts a broad
plausibility band, not a benchmark gate.

- **Epic:** op-6tz — OpenMC-API parity + openmc-notebooks as verification tests.
- **Scope of this run:** the outram-mc notebook subset only. No njoy files were
  touched. No `src/` behaviour changed — this run adds docs + tests + beads.
- **Date:** 2026-07-15 (Asia/Singapore, active working hours confirmed).
- **Worktree branch:** `worktree-agent-a73d361401e2c3f4d` (fast-forwarded to
  develop `bd52224` to obtain the crate, which was absent from the stale
  branch-point).

## Files changed

| File | Kind | What |
|---|---|---|
| `crates/outram-mc-libs/docs/openmc-notebooks-verification.md` | new doc | Master mapping: all 27 notebooks (19 outram-mc + 8 njoy) → owner, OpenMC API, outram-mc/njoy equivalent-or-GAP, tractable-now, gap bead. |
| `crates/outram-mc-libs/tests/openmc_notebooks.rs` | new test entry | Harness binary; `#[path]`-declares one module per notebook in the subset. |
| `crates/outram-mc-libs/tests/openmc_notebooks/pincell.rs` | new test | **LIVE** (partial): Godiva bare-sphere k-eff + leakage sign check; plus one ignored gap test for the true LWR pin. |
| `crates/outram-mc-libs/tests/openmc_notebooks/*.rs` (19 more) | new tests | One `#[ignore]` gap test per notebook, each with `unimplemented!()` body + gap bead. |
| `crates/outram-mc-libs/docs/ai-fleet-review/op-6tz/REVIEW_MANIFEST.md` | new doc | This file. |

Beads (shared DB, not files): op-6tz.7 … op-6tz.22 created with dependency edges
(see below). op-6tz.1/.2 (mapping doc + harness) are now satisfied by this run.

## Provenance

- Notebooks: <https://github.com/openmc-dev/openmc-notebooks> @
  `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (2024-07-10), **MIT**. Only API
  usage and structure were read (fetched raw, parsed for `openmc.*` tokens); no
  notebook prose was copied.
- Live-test cross sections: the crate's own embedded LOW-tier data
  (`Nuclide::from_core`, WMP + fast MGXS). No notebook data, no confidential or
  operational data (`DATA_POLICY.md` clean).
- Godiva model: ICSBEP HEU-MET-FAST-001 (public benchmark).

## What is LIVE vs IGNORED (honest)

**LIVE (2 passing tests, `pincell` module):**

- `pincell_criticality_eigenvalue_via_godiva_bare_sphere` — runs `run_keff` on
  the Godiva bare HEU sphere and asserts a stationary, plausible eigenvalue.
- `pincell_leakage_reduces_reactivity` — smaller sphere leaks more (sign check).

These are an **honest reduced slice**: outram-mc's only working end-to-end path
is a homogeneous bare-sphere k-eff (`physics::keff::run_keff`). That covers the
`pincell` notebook's criticality-eigenvalue core but **not** its real model (LWR
pin, square reflective cell, S(alpha,beta) water, cell flux tally). The full LWR
model is a separate ignored test in the same module.

**IGNORED (20 tests):** every other notebook in the subset, each `#[ignore]`d
with the required-API reason and a gap bead, body = `unimplemented!()` so a
removed ignore fails loudly rather than passing green.

## Measured result (V&V record)

Godiva bare sphere, `--example godiva_keff` (5000 histories × [40 inactive + 110
active], embedded LOW tier), run 2026-07-15:

```
k_eff = 1.01022 ± 0.00177   (Δk = +1022 pcm vs ICSBEP 1.0000 ± 0.0010)
```

The harness's live test uses a lighter setting (1500 × [20 + 40]) for speed and
asserts the band [0.9, 1.4] with σ < 0.02 — it guards the transport chain, not
accuracy. Consistent with the `src/physics/keff.rs` unit test.

## Build / test output (actual)

```
$ cargo build -p outram-mc-libs --release
    Finished `release` profile [optimized] target(s) in 30.23s

$ cargo test -p outram-mc-libs --release --test openmc_notebooks
running 22 tests
... 20 ignored (each with a documented gap bead) ...
test pincell::pincell_leakage_reduces_reactivity ... ok
test pincell::pincell_criticality_eigenvalue_via_godiva_bare_sphere ... ok
test result: ok. 2 passed; 0 failed; 20 ignored; 0 measured; 0 filtered out; finished in 0.37s

$ cargo test -p outram-mc-libs --release --lib --tests --no-run
    Finished `release` profile [optimized]   (all lib + integration tests compile)
```

## Beads filed (finer gaps under op-6tz)

| Bead | Gap | Blocks (notebooks) |
|---|---|---|
| op-6tz.7 | General CSG geometry navigation (`locate_particle`, `distance_to_boundary`, `Universe::find_cell`) | pincell(full), hex, candu, triso, tally set |
| op-6tz.8 | History-based transport loop over general geometry (blocked-by .7) | flux-spectrum, tally set |
| op-6tz.9 | Tally scoring + filter binning wired to transport (blocked-by .8) | flux-spectrum, tally-arithmetic, pandas, post-proc, expansion |
| op-6tz.10 | Rectangular-lattice transport + reflective BC (blocked-by .7) | pincell(full), tally-arithmetic, mg-mode |
| op-6tz.11 | Hexagonal-lattice transport (blocked-by .7) | hexagonal-lattice, candu |
| op-6tz.12 | S(alpha,beta) thermal scatter in transport (blocked-by .8) | pincell(full), flux-spectrum |
| op-6tz.13 | RegularMesh + MeshFilter tally scoring (blocked-by .9) | post-processing, tally-arithmetic, pandas |
| op-6tz.14 | Functional-expansion filters Legendre/Zernike (blocked-by .9) | expansion-filters |
| op-6tz.15 | Multigroup transport mode + MGXS data types | mg-mode i/ii/iii |
| op-6tz.16 | TRISO / stochastic-media full k-eff sim (blocked-by .7) | triso |
| op-6tz.17 | DAGMC/CAD + unstructured-mesh geometry/tallies | cad-based-geometry, unstructured-mesh i/ii |
| op-6tz.18 | Depletion / transmutation driver (out of scope note) | depletion |
| op-6tz.19 | Decay-photon source + photon transport (out of scope note) | gamma-detector |
| op-6tz.20 | In-memory run/introspection API (openmc.lib analog) | capi |
| op-6tz.21 | Weight windows / variance reduction (notebook absent upstream) | shielded_room_weight_window |
| op-6tz.22 | StatePoint + DataFrame export + tally arithmetic (blocked-by .9) | post-proc, pandas, tally-arithmetic, tally-power-normalization |

## What a human MUST verify

1. **Notebook-count discrepancy.** The directive names 28 notebooks incl.
   `shielded_room_weight_window`; the pinned commit has **27** and that notebook
   is **absent**. Confirm whether it was renamed/removed upstream or belongs to a
   different commit, and adjust op-6tz.21 accordingly.
2. **Tractability claims.** Confirm — by reading `src/` — that no other notebook
   is quietly tractable today (the assessment says only bare-sphere k-eff works;
   `geometry::geometry`, `physics::transport`, `tally::scoring`,
   `physics::physics_mg`, `Universe::find_cell` are all stubs/`todo!()`).
3. **The live `pincell` slice is honest, not a benchmark claim.** It is a
   fast-sphere Godiva k-eff standing in for a thermal LWR pin. Verify the
   framing is acceptable, or split it into a dedicated `godiva`/criticality
   verification file distinct from `pincell` if the mapping should be stricter.
4. **Owner split** for the njoy rows (mapped here but not built) matches the
   njoy track's own plan; avoid double-owning `search` (needs both data and a
   working transport path).
5. **Bead granularity** — whether op-6tz.7/.8/.9 should be one epic-like chain vs
   three beads, and whether depletion/photon/DAGMC (op-6tz.18/.19/.17) should be
   marked explicitly out-of-scope-for-now rather than open feature gaps.

## Compliance notes

- No `src/` behaviour changed; no new dependencies added (crate stays
  Android-safe: `thiserror` + `ndarray` + `njoy-outram-park-fork`, no BLAS).
  Tests use only the existing public API, so no Android gate is needed.
- No trait objects / `Box<T>` / lifetime params introduced in the new code.
- Doc comments (`//!` module maps, `///` on the live test items) present.
- This is untrusted AI draft output per `AI_USAGE.md` — human inspection,
  and (when the gap APIs land) real V&V against public benchmarks, are still
  required before any of the ignored cases can be trusted.
