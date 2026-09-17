# REVIEW MANIFEST — njoy P2/P4 fleet pass (op-cjw.13/15/16/17/18, op-6tz.6.3/6.4)

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`**

This document records an AI fleet porting pass over the remaining P2/P4 njoy items.
Every code change below was produced by an Opus subagent translating upstream
Fortran (or ENDF-6 format logic) and integrated + built + tested by a lead Opus
agent. **None of the numeric-engine work is validated against a real NJOY run or a
physical benchmark** unless a specific measured number with a cited reference is
stated in that item's section. Numeric-engine correctness and every
hand-transcribed constant table are the top human-verify asks.

- **Date:** 2026-07-15 (Asia/Singapore, within working hours).
- **Upstream oracle:** NJOY2016, git commit
  `ac5adf5f33d893e42f2eed7fb286b0d51c7580da` (version 2016.79), files
  `/…/NJOY2016/src/<module>.f90`. NJOY2016 is modified BSD-3-Clause (LANL/DOE),
  GPL-compatible; every ported file carries the GPL-3.0-only provenance header.
  Non-LANL, not endorsed by LANL/DOE.
- **ENDF data:** ENDF/B-VIII.0 (open-source), U-235 test tape in
  `tests/resources/n-092_U_235-ENDF8.0.endf`.
- **Notebook reference (mgxs/mdgxs):** openmc-notebooks commit
  `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
- **Integration base:** `origin/develop` (`0db6f02`). Changes confined to
  `src/{errorr,groupr,gaminr,leapr,covr,mixr,resxsr,dtfr,nuclear_data}/` and
  `tests/openmc_notebooks_data/`.
- **Baseline before this pass:** `cargo test -p njoy-outram-park-fork --lib
  --release` → 274 passed / 0 failed / 0 warnings.

## Items in this pass

| Bead | Title | Status |
|---|---|---|
| op-cjw.13 | Split `errorr/groups.rs` (>cap) | see `sections/op-cjw-13.md` |
| op-cjw.15 | GROUPR/GAMINR numeric engine + GENDF writer | see `sections/op-cjw-15.md` |
| op-cjw.16 | LEAPR MF=7 writer + coldh + coherent-elastic | see `sections/op-cjw-16.md` |
| op-cjw.17 | COVR covard reader + BOXER writer | see `sections/op-cjw-17.md` |
| op-cjw.18 | MIXR/RESXSR/DTFR run() drivers + tape I/O | see `sections/op-cjw-18.md` |
| op-6tz.6.4 | mdgxs delayed-neutron readers (MF=1/455, MF=5/455) | see `sections/op-6tz-6-4.md` |
| op-6tz.6.3 | mgxs flux-solved/self-shielded MGXS (depends on op-cjw.15) | see `sections/op-6tz-6-3.md` |
| op-cjw.14 | Phase-6 formatters (CCCCR/MATXSR/POWR/WIMSR/PLOTR/VIEWR) | DEFERRED — see below |
| op-cjw.9 | umbrella GROUPR/GAMINR/COVR/LEAPR | updated as children land |

**op-cjw.14 (deferred, honest scoping):** CCCCR/MATXSR/POWR/WIMSR are output
formatters for codes OUTRAM PARK does not target (ISOTXS/BRKOXS/DLAYXS, MATXS/
TRANSX, EPRI-CELL/CPM, WIMS); PLOTR/VIEWR are PostScript plotting (out of scope
by directive). Left as `NjoyError::NotPorted` stubs and NOT worked in this pass to
avoid runaway on low-value targets. Bead stays OPEN.

## Aggregate build & test (actual, run by the lead after integration)

```
cargo build -p njoy-outram-park-fork --release            → Finished, 0 errors, 0 warnings
scripts/test.sh  (cargo test --lib --tests --release, 12 GB cap)
    lib:   327 passed; 0 failed; 0 ignored
    all 14 test binaries: 0 failed  (10 ignored — documented #[ignore] scaffolds)
    exit 0
```

- **Lib test count: 274 (baseline) → 327 (+53).** No baseline regression.
- New lib tests by item: op-cjw.15 +14, op-cjw.16 +7 (net, −1 removed stub),
  op-cjw.17 +18, op-cjw.18 +11, op-6tz.6.4 +3 (delayed readers). op-cjw.13 +0
  (pure refactor).
- Notebook target `openmc_notebooks_data`: mdgxs-part-i 3 live (op-6tz.6.4),
  mgxs-part-i `groupr_engine_vector_group_average` live (op-6tz.6.3), remaining
  self-shielded/scatter-matrix + transport ops correctly `#[ignore]`.

| Bead | New/changed files (src) | New tests | Status |
|---|---|---|---|
| op-cjw.13 | errorr/groups/{mod,tables}.rs (was groups.rs) | 0 (refactor) | **DONE / CLOSED** |
| op-cjw.15 | groupr/{panel,gendf,kinematics}.rs, gaminr/gpanel.rs, mod.rs×2 | +14 | PARTIAL (vector done; matrix NotPorted → op-3ut) |
| op-cjw.16 | leapr/{coher,endout}.rs, coldh.rs, discrete.rs, mod.rs | +7 | PARTIAL (MF=7 writer+coldh+coher done; copys/skold/run() → op-b2k) |
| op-cjw.17 | covr/{covard,boxer}.rs, mod.rs | +18 | PARTIAL (transform+BOXER done; real ERRORR tape I/O → op-7fb) |
| op-cjw.18 | mixr/driver.rs, resxsr/resxs.rs, dtfr/gendf.rs, +mods | +11 | PARTIAL (MIXR done; RESXSR/DTFR gaps → op-7aq) |
| op-6tz.6.4 | nuclear_data/delayed.rs, mod.rs; mdgxs tests | +3 lib | PARTIAL (readers done; delayed-group MGXS → op-0hv) |
| op-6tz.6.3 | (test only) mgxs_part_i.rs | +1 int | PARTIAL (vector engine live; scatter matrix/Chi blocked by op-3ut) |

All changes confined to `crates/njoy-outram-park-fork/`. Every new file is under
the 1000-line cap and carries the GPL-3.0/NJOY-BSD provenance header (NJOY2016
`ac5adf5`). Bead op-cjw.13 closed; all other items left OPEN with honest notes;
follow-up beads op-3ut/op-b2k/op-7fb/op-7aq/op-0hv filed for the documented gaps.

## Top human-verify asks (untrusted AI draft)

1. **Numeric-engine golden-file validation.** The GROUPR panel quadrature
   (op-cjw.15) and the LEAPR MF=7 writer (op-cjw.16) are verified only by
   internal self-consistency (round-trips, property asserts, one hand-computed
   value) — NOT against a real NJOY GENDF/MF=7 tape. Validate before trusting.
2. **Hand-transcribed constant tables.** errorr group-boundary tables
   (op-cjw.13, moved verbatim), LEAPR coherent-elastic lattice / BeO form-factor
   / cold-H molecular constants (op-cjw.16) — re-verify digit-for-digit.
3. **COVR reads a constructed matrix, not a real ERRORR tape** (op-cjw.17); the
   auto-vs-cross rsd sourcing needs confirmation against a real ERRORR flow.
4. **Delayed-neutron λ / β** (op-6tz.6.4) matched to the openmc-notebooks
   reference to <1e-3; confirm the ENDF MF=1/455 LDG=1 branch (spec-faithful but
   unexercised by the U-235 tape).

---

Per-item details are in `sections/*.md`, each written by the subagent that did the
work (op-6tz-6-3 by the lead). The lead assembled this file and verified the
aggregate build/test.
