# REVIEW MANIFEST — "port the rest of NJOY" fleet pass (op-cjw)

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED (untrusted until licence-provenance
review + end-to-end V&V per `RESPONSIBLE_USE.md`)**

This document records an AI fleet porting pass over the remaining unported /
stubbed NJOY2016 modules in `njoy-outram-park-fork`. Every module below was
produced by an Opus subagent translating the upstream Fortran and was integrated,
built, and tested by a lead Opus agent. **None of it is validated against a real
NJOY run or a physical benchmark.** All of it is front-end / self-contained-core
work: the heavy numeric group-averaging / tape-writer engines are honestly left
`NotPorted` (documented per module), not faked.

- **Date:** 2026-07-15 (Asia/Singapore, within working hours).
- **Upstream oracle:** NJOY2016, git commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`
  (version 2016.79), files `/…/NJOY2016/src/<module>.f90`.
- **Licence:** NJOY2016 is modified BSD-3-Clause (LANL/DOE), GPL-compatible; every
  ported file carries the GPL-3.0-only provenance header. Non-LANL, not endorsed
  by LANL/DOE. See crate-root `LICENSE.njoy` + `NOTICE`.
- **Integration base:** `origin/develop` (`8fd32636`). Changes are confined to the
  seven module directories `src/{groupr,gaminr,leapr,covr,mixr,resxsr,dtfr}/`.
  `lib.rs`, `modules.rs`, `Cargo.toml` are unchanged from develop (all modules
  keep the no-arg `run()` registry contract).

## Aggregate build & test (actual, run by the lead after integration)

```
cargo check -p njoy-outram-park-fork --lib --tests --release   → Finished, 0 errors, 0 warnings
cargo test  -p njoy-outram-park-fork --lib --release (12 GB cap) → test result: ok. 274 passed; 0 failed
```

274 = 171 pre-existing lib tests (develop) + 103 new tests added by this pass. All
new tests are inline `#[cfg(test)]` unit tests; integration-test files under
`tests/` are unchanged from develop.

| Module | New files | Lines (dir) | New tests | Fortran src | LOC (F90) |
|---|---|---|---|---|---|
| groupr | input.rs, photon_groups.rs, weights.rs, mod.rs | 2185 | 19 | groupr.f90 | 12690 |
| gaminr | input.rs, photon_groups.rs, weights.rs, mod.rs | 1298 | 14 | gaminr.f90 | 1517 |
| leapr | input, sct, frequency, continuous, translation, discrete, coldh, mod | 2378 | 20 | leapr.f90 | 3625 |
| covr | input.rs, correlation.rs, mod.rs | 1305 | 17 | covr.f90 | 2250 |
| mixr | input.rs, mix.rs, mod.rs | 788 | 9 (+2 doctest) | mixr.f90 | 533 |
| resxsr | input, format, assemble, driver, mod | 996 | 10 | resxsr.f90 | 505 |
| dtfr | input, table, format, driver, mod | 1116 | 14 | dtfr.f90 | 1510 |

All files carry the GPL-3.0/NJOY-BSD provenance header; every public item is
documented; no trait objects / `Box` / lifetime params; pure-Rust (Android-safe).
All files are under the 1000-line cap **except** `groupr/input.rs` (981, ok) — the
largest file is `leapr` split across 8 siblings, each < ~400 lines.

---

## GROUPR (`src/groupr/`)

- **Files:** `mod.rs` (150, module map + `run()` skeleton), `input.rs` (981,
  `GrouprInput` card deck + free-format `parse` + selector enums),
  `photon_groups.rs` (441, `gengpg` photon group structures), `weights.rs` (613,
  `genwtf`/`getwtf` analytic weights + preserved tabulated tables), `README.md`.
- **Ported (tested):** card-input deck (`ruinb` `groupr.f90:1044-1130`, reads
  `:628`,`:993`); photon group structures `igg=2..10` incl. VITAMIN-C `eg8` splice
  (`gengpg` `:4651-4863`); analytic weight functions `iwt=2,3,4,6,7,11,12`
  (`genwtf` `:4865-5113`, `getwtf` `:5115-5307`); tabulated tables `iwt=5,8,9`
  preserved verbatim. **Neutron group structures are re-exported from
  `crate::errorr::groups` (`gengpn`), not duplicated.**
- **NotPorted (returns `NjoyError::NotPorted`):** the group-averaging engine —
  `getmf6`/`getff`/`getflx`/`getyld`/`getsig`/`getdis`, `panel`/`epanel`/`gengr`/
  `glmol`, kinematics (`cm2lab`/`f6cm`/`f6lab`/`getaed`/`gam102`), GENDF writer;
  input cards 8a/8b/8d (`iwt<0/1/0`), card 9a (`mfd=-1`); tabulated-weight
  evaluation (needs the `terpa` interpolator).
- **Flags / auto-mode guesses:** `parse` assumes one card per line (real decks do);
  `GrouprInput::default` uses `iwt=6, lord=3` from the manual example (not a
  Fortran hard default); a `neutron_group_from_ign` round-trip test guards against
  drift from ERRORR's `NeutronGroupStructure::ign()`.
- **Human must verify:** transcribed tables `EG2..EG10`, `W1/W2/W8/W9` and
  `getwtf` closed-form constants character-by-character vs source; the `eg8` splice
  and `iwt=4` normalization vs a real NJOY run; the whole numeric path is unported
  (no group xs / matrix V&V gate).

## GAMINR (`src/gaminr/`)

- **Files:** `mod.rs` (179), `input.rs` (426, `GaminrInput` deck + `standard_reactions`),
  `photon_groups.rs` (413, `PhotonGroupStructure` = `genggp`), `weights.rs` (280,
  `gnwtf`/`gtflx`), `README.md`.
- **Ported (tested):** cards 1–7 (`ruing` `gaminr.f90:538-583`); the 9-reaction
  "process all" set (`:111-118`, incl. ENDF-VI MT 602/621→522/525); photon group
  structures `igg=0,2..10` (`genggp` `:585-776`); weights `iwt=2` (constant),
  `iwt=3` (1/E + rolloffs, log-log TAB1) (`:778-872`).
- **NotPorted:** `gtsig` (`:1133-1160`), feed functions `gtff` (coherent/incoherent/
  pair `:1162-1514`), `gpanel` Lobatto quadrature (`:874-1011`), `dspla` (`:1013-1131`),
  ENDF/GENDF I/O (`:133-536`), read-in weight `iwt=1` / read-in grid `igg=1`.
- **Flags:** the Fortran subroutine is **`genggp`**, not the task's "gengpg"
  spelling — ported under the correct name. GAMINR's photon tables were ported
  **locally** (a doc comment flags that they duplicate `groupr`'s photon structures;
  a human should decide whether to dedupe). `PhotonWeight::evaluate` reproduces
  in-range log-log TAB1 only (clamps out-of-range; NJOY `terpa` extrapolation and
  the `gtflx` `step=1.05` panel limiter not reproduced). `igg=0` → `vec![0.0]`.
- **Human must verify:** a group-boundary table vs `gaminr.f90:609-666` (≈430 hand-
  transcribed literals); the `igg=8` VITAMIN-C index arithmetic; dedupe decision.

## LEAPR (`src/leapr/`)

- **Files:** `mod.rs` (module map + `SabMatrix` + `run()→NotPorted`), `input.rs`
  (`LeaprInput` + enums), `sct.rs` (free-gas / short-collision-time Gaussian),
  `frequency.rs` (`start`/`fsum` → `FrequencyModel`), `continuous.rs` (`contin`/
  `terpt`/`convol` phonon expansion + moment checks), `translation.rs` (`trans`/
  `stable`/`terps`/`sbfill`/`besk1`), `discrete.rs` (`discre`/`bfact`/`bfill`/
  `exts`/`sint` + `I0`/`I1`), `coldh.rs` (`bt`/`sumh`/`cn`/`sjbes`/`terpk`),
  `README.md`.
- **Ported (tested):** input model; SCT/free-gas; frequency integrals (Debye-Waller
  λ, effective-T, T₁); full phonon-expansion sum with SCT tail; translational
  (free-gas + diffusion) orchestrator + helpers; discrete-oscillator orchestrator +
  helpers; cold-H₂/D₂ numerical helpers. Line ranges: input 122-372; contin 455-645;
  start/fsum 647-764; terpt/convol 766-842; trans 844-1007; stable 1009-1122;
  terps/sbfill 1124-1251; besk1 1253-1318; discre 1320-1661; bfact 1663-1796;
  bfill/exts/sint 1798-1934; coldh helpers 2185-2466.
- **NotPorted:** `run()` card driver + `endout`/`copys` MF=7 tape writer; the
  `coldh` **orchestrator** (helpers ported); coherent-elastic `coher`/`formf`/
  `tausq`/`taufcc`/`taubcc` (deferred — `thermr::coherent` already consumes MF=7
  MT=2); `skold` Sköld correction.
- **V&V numbers (self-consistency / closed-form, NOT validation):** `besk1`
  K₁(0.5)=1.656441, K₁(1)=0.601907, e²K₁(2)=1.03339 (<2e-4); I₀(1)=1.2660658,
  I₁(1)=0.5651591 (<5e-7); j₀(1)=0.8414710, j₁(1)=0.3011687 (<1e-5); `cn(0,0,0)=1`;
  SCT detailed balance `S(α,−β)=e^{−β}S(α,β)` <1e-13; Debye model `f0=0.29726`,
  `tbar=2.31659`, phonon normalization `sum0=0.99289`, sum-rule `sum1=0.98867`.
- **Flags:** `discre` faithfully reproduces two NJOY quirks (inline + README):
  `tbart` accumulates across the α-loop, and the delta-placement `idone` flag is
  shared with the inner search (≤1 delta per α). Kept for line-traceability.
- **Human must verify:** end-to-end vs a real LEAPR MF=7 tape (e.g. H-in-H₂O /
  graphite from a published phonon spectrum) + downstream THERMR; the two `discre`
  quirks vs the oracle; `trans` convolution `sbfill` underflow guard + Simpson
  weights.

## COVR (`src/covr/`)

- **Files:** `input.rs` (653, card deck + input kernels), `correlation.rs` (472,
  cov→corr math), `mod.rs` (180, `run()→NotPorted` + `run_with_deck()`), `README.md`.
- **Ported (tested):** full card deck (`CovrInput`, `CovrMode` plot/library enum,
  selector enums with round-trip codes); shade-level expansion (`covr.f90:289-305`);
  MT-strip predicate (`:546-553`); `epmin` scaling; `CovarianceMatrix` with
  rsd=√diag (`:636-641`); covariance→correlation auto (`:672-688`) and cross-reaction;
  null test (`:930`); plot-stage clamp (`:1371-1372`); plottability/`ismall`
  (`:683`); shade indexing (`:1601-1619`); 5-stage pipeline skeleton.
- **NotPorted:** ERRORR tape reader `covard` (`:720-937`); MT-list scan `expndo`
  (`:508-576`); BOXER writer `press`/`setfor` (`:1991-2247`); **all PostScript
  plotting** `plotit`/`matshd`/`patlev`/… (`:939-1910`).
- **Flags:** covariance/correlation kept as `f64` (aliases `Correlation`/
  `RelativeStdDev`/`CovarianceValue`) **not uom Quantity** — a covariance matrix is
  dimensionally variable (absolute vs relative), so uom would be wrong; units in
  prose. Exact-zero guards (`!= 0.0`) reproduce Fortran (zero-variance → 0, not NaN).
- **Human must verify:** auto- vs cross-covariance rsd sourcing in `subroutine corr`
  (`:597-670`) vs real ERRORR tape data flow (math ported, not tape control flow);
  the unported `covard` record layout / absolute→relative conversion; no golden-file
  comparison run.

## MIXR (`src/mixr/`) — most complete port in this pass

- **Files:** `input.rs` (165, `MixrInput`/`MixComponent`/`from_cards`), `mix.rs`
  (564, mixing engine + tests), `mod.rs` (59, `run()→NotPorted`), `README.md`.
- **Ported (complete + tested):** card deck; `gety` value retrieval w/ out-of-range
  rules (`gety_value`, `mixr.f90:392-530`); union-grid weighted sum
  `σ_out(E)=Σ w_i·σ_i(E)` (`mix_reaction`, `:291-303`); `sigfig` 7-figure rounding
  (`util.f90:361-393`); full tape assembly MF=1/451 + MF=3 (`mix`, `:197-370`).
  **Drivable end-to-end in memory:** `Tape::read` → `MixrInput::mix` → `Tape::write`.
- **NotPorted:** only `run()` — the `nsysi→nout` card-file driver (matches the
  `moder::run` convention).
- **Flags:** union grid uses distinct tabulated energies (duplicate-energy
  discontinuities collapse — exact for lin-lin PENDF); MF=1/451 comment **text is
  dropped** (crate's `[f64;6]` row model can't hold Hollerith); AWI/EMAX/NSUB use
  MIXR default seeds (AWI=1, NSUB=10). **uom deliberately not introduced** (reuses
  f64 `endf` tabulated model; units in every doc comment — documented divergence
  from the crate's "uom at boundaries" guideline, justified by reuse).
- **Human must verify:** against a genuine NJOY MIXR run on a real multi-isotope
  PENDF (tests check arithmetic + out-of-range logic vs hand-computed refs, not a
  golden file); MF=1/451 header fidelity (dropped comment / defaulted AWI/EMAX/NSUB).

## RESXSR (`src/resxsr/`)

- **Files:** `mod.rs` (registry `run()`), `input.rs` (`ResxsrInput`/`MaterialSpec`),
  `format.rs` (RESXS record layout + word-count math), `assemble.rs` (union grid +
  linear thinning), `driver.rs` (`run(&ResxsrInput)` skeleton), `README.md`.
- **Ported (tested):** card deck; full RESXS record layout w/ header/field values +
  word-count arithmetic (`resxsr.f90:43-189`); union-grid assembly + adaptive
  linear-thinning (`:306-397`).
- **NotPorted:** PENDF tape reader `gety1`/`loada`/`finda` (`:250-352`); binary RESXS
  writer (`:435-501`); `run()` → `NotPorted("resxsr::run")`.
- **Flags:** thinning reproduces the Fortran's run-*start* node selection (can drop a
  sharp peak — verified by hand-trace); `xs_at` returns 0 outside a reaction's grid;
  `FILE_NAME="resxsr"` per the code constant (spec comment says `resxs` — noted).
- **Human must verify:** golden-file comparison vs an upstream RESXS file (needs the
  NotPorted tape I/O); acceptability of node-selection thinning for IR use.

## DTFR (`src/dtfr/`)

- **Files:** `mod.rs`, `input.rs` (`DtfrInput`, selector enums, CLAW tables),
  `table.rs` (group ordering, `sig` indexing, scatter packing), `format.rs`
  (`dtfout` line formatting), `driver.rs`, `README.md`.
- **Ported (tested):** `ruin` card deck incl. CLAW standard edit tables + 3 appended
  standard edits (`dtfr.f90:590-767`, tables `:603-621`); DTF `sig` layout, group
  ordering (`:342-346`), reduced-length **triangular scatter-matrix packing** w/
  fold-back clamps + P₀ absorption correction (`:410-424`); `dtfout` card/line
  formatting (`:769-946`).
- **NotPorted:** GENDF tape reader (`:181-553`; kernels consume an in-memory
  `DtfTable`); plotting `ploted`/`plotnn`/`plotnp` (permanently out of scope);
  `run()` → `NotPorted("dtfr::run")`.
- **Flags:** `add_scatter_record` takes already-extracted per-k transfer values
  (decoupled from GENDF layout); `fortran_e` reproduces the signed 2-digit-exponent
  `eW.D` field but **exact byte-for-byte agreement is a validation-pass item**;
  format-1 per-position edit-selection is represented (`pack_dtf_block`) but not
  fully wired (needs the GENDF reader).
- **Human must verify:** golden DTF-table comparison for scatter-band clamps +
  absorption correction and `fortran_e` byte layout, once the GENDF reader lands.

---

## What a human reviewer must do before trusting ANY of this

1. **Licence-provenance review** — confirm each file's GPL-3.0/NJOY-BSD header and
   that no non-open data leaked into tests (all inputs here are synthetic /
   published-constant only).
2. **Verification** — spot-check the hand-transcribed constant tables (GROUPR/GAMINR
   group structures + weights especially) character-by-character against the Fortran.
3. **Validation** — none performed. Each module's numeric core is either NotPorted or
   only self-consistency-tested. End-to-end validation requires the NotPorted tape
   I/O + a golden NJOY run or a physical benchmark (LEAPR→THERMR being the most
   valuable).
4. **Follow-up beads** (filed under `op-cjw`): the NotPorted numeric engines are the
   real remaining work per module — see the beads created for this pass.
