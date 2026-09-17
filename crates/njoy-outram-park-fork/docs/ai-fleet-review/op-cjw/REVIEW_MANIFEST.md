# ERRORR port + SAMM mf2 bug fix — AI fleet review manifest (epic op-cjw)

> **⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED (untrusted until licence-provenance review + V&V per `RESPONSIBLE_USE.md`)**

Everything below was produced by an Opus lead agent + three Opus porting
subagents in a single automated session on **2026-07-15** (Asia/Singapore),
working in an isolated git worktree. It compiles and its unit tests pass, but
it is **untrusted draft material**: no human has reviewed the licence
provenance, and — critically — **none of it has been validated end-to-end
against upstream NJOY golden output or a real ENDF evaluation**. Treat it as a
starting point for review, not as verified functionality.

## Scope

Two beads under epic **op-cjw** (crate `njoy-outram-park-fork` only; no other
crate touched):

- **op-cjw.3** — SAMM `mf2` eliminated-channel reorder bug (surgical fix + tests).
- **op-cjw.1** — Port ERRORR (multigroup covariance, ~11.2k Fortran lines) —
  **PARTIAL** port of self-contained pieces; the numeric covariance pipeline is
  explicitly **not** ported and returns `NjoyError::NotPorted`.

## Provenance

- **Upstream project:** NJOY2016.
- **Upstream git commit:** `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`
  (from `git -C /home/teddy0/Documents/research/NJOY2016 rev-parse HEAD`).
- **Upstream source files ported from:**
  - `src/samm.f90` — `rdsammy` `mode==7` branch (SAMM fix).
  - `src/errorr.f90` — `subroutine errorr` driver (input cards) + the
    self-contained math kernels (`efacphi`/`efacts`/`eunfac`/`egnrl`/`cleb`/
    matrix helpers) + `egngpn` entry point.
  - `src/groupr.f90` — `gengpn` (the actual source of the group-boundary
    tables; ERRORR's `egngpn` delegates to it).
  - `src/util.f90` — `sigfig` (lethargy-grid rounding helper).
- **Licence:** NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence,
  GPL-compatible; these derivative files are GPL-3.0-only. Each new file carries
  a `//`-block attribution header naming the upstream file(s), commit, licence,
  and the "modified, non-LANL, not endorsed" language. `LICENSE.njoy` + `NOTICE`
  at the crate root are unchanged.

## Files created / modified

### op-cjw.3 — SAMM mf2 fix (lead agent, verified against Fortran)

| Path | What |
|---|---|
| `src/samm/mf2.rs` (modified) | Extracted the eliminated-channel reorder into a new private testable fn `reorder_eliminated_channel`; **fixed two off-by-one defects**; rewrote the doc comments; added 4 unit tests. |
| `src/samm/README.md` (modified) | Replaced the "unverified, ported literally" caveat with the fix write-up + verification status. |

### op-cjw.1 — ERRORR partial port (three subagents + lead wiring)

| Path | What |
|---|---|
| `src/errorr/math.rs` (new, 885 lines) | Self-contained numeric kernels: `efacphi`, `efacts`, `eunfac`, `egnrl` (unresolved width-fluctuation integral w/ full quadrature tables), `cleb` (specialised Clebsch-Gordan), and 3×3 matrix helpers `eabcmat`/`ethrinv`/`efrobns`. 15 tests. |
| `src/errorr/groups.rs` (new, 1892 lines) | Built-in neutron group structures: `neutron_group_structure(ign)` + `NeutronGroupStructure` enum. `ign` 2..36 ported from `gengpn`; `ign = ±1` (read-from-input) is `NotPorted`. 10 tests. |
| `src/errorr/driver.rs` (new, 1126 lines) | ERRORR user-input card deck: `ErrorrInput` + sub-structs + selector enums (`GroupStructure`, `WeightOption`, `CovarianceFile`, …) with integer round-trip mappings; `run(&ErrorrInput)` orchestration skeleton returning `NotPorted`. 3 tests. |
| `src/errorr/mod.rs` (modified) | Wired `pub mod driver/groups/math;`, re-exported `ErrorrInput`, updated the module `//!` doc to state PARTIAL status + gap list. Kept the no-arg `run()` for the `NjoyModule` dispatch (`modules.rs:100`). |
| `docs/ai-fleet-review/op-cjw/REVIEW_MANIFEST.md` (new) | This file. |

## op-cjw.3 — the SAMM fix in detail (highest-confidence item)

**Bug (as filed):** the eliminated-channel reorder read `gamma(igamma+1)`
(`samm.f90:1186`) where the derived index should be `igamma-1`.

**What verification against the Fortran actually found:** the one-line summary
undersells it — **there are two off-by-one defects**, and fixing only the
extract index still corrupts data:

Provisional layout the immediately-preceding read (`samm.f90:1181-1183`)
establishes: `gamgam = raw₁`, `gamma(k) = raw₍ₖ₊₁₎` (1-indexed). For an
eliminated channel at raw position `p`, the true `Γγ` (raw `p`) therefore sits
at `gamma(p-1)` = 0-indexed `channel_widths[igamma-1]`, **not** `gamma(p+1)`.

1. **Extract index** `gamma(igamma+1)` → **`gamma(igamma-1)`** (`samm.f90:1186`).
   Upstream points two slots too high; when the eliminated channel is last, it
   reads past the resonance's written data into the globally-sized backing array.
2. **Shift-loop bound** `do i = igamma, 2, -1` → **`do i = igamma-1, 2, -1`**
   (`samm.f90:1188`). Upstream's bound clobbers `gamma(igamma)`, an explicit
   channel already in its correct slot, with a duplicate. **Fixing only defect
   (1) leaves defect (2), which corrupts the last explicit channel of any group
   whose eliminated channel is neither first nor last.**

**Tests** (`src/samm/mf2.rs`, `samm::mf2::tests::reorder_eliminated_*`):
first / middle / last / large-middle raw positions. **Result 2026-07-15: 4/4
pass;** the pre-fix code fails `reorder_eliminated_middle`,
`reorder_eliminated_middle_large`, and `reorder_eliminated_last`.

**Still-open validation (NOT done here):** an end-to-end LRF=7 evaluation check
(e.g. ¹⁶O or ¹⁹F, a spin group where the eliminated channel is not first) — this
remains under bead **op-cjw.2**. The fix is verified against the provisional-
layout derivation and unit tests, not against real reconstructed cross sections.

## Assumptions & best-guess decisions made in AUTO mode

1. **Worktree was branched from a develop commit predating the njoy crate.** The
   feature work was started from the current develop tip (`494bc90`, which
   contains `crates/njoy-outram-park-fork`) instead. Per the coordinator's
   mid-task instruction, the final push target is **develop** (not a feature
   branch); the branch is rebased onto `origin/develop` before pushing.
2. **`run()` naming.** The driver subagent introduced `driver::run(&ErrorrInput)`;
   the `NjoyModule` registry calls a no-arg `errorr::run()`. To avoid breaking
   the registry, the no-arg `run()` is kept in `mod.rs` (returns `NotPorted`) and
   the real entry point is `errorr::driver::run`. A human may want to unify these.
3. **`egngpn` vs `gengpn`.** `egngpn` in `errorr.f90` delegates its boundary
   tables to `gengpn` in `groupr.f90`; the group-structure tables were ported
   from `gengpn` (the true source). The union-with-covariance-grid step
   `egngpn` layers on top is ERRORR-internal plumbing and was **not** ported
   (it belongs with the covariance pipeline, which is unported).
4. **Out-parameter → return-value convention.** Fortran out-params became Rust
   return tuples/structs (`efacts → (se, pe)`, `eunfac → (vl, ps)`,
   `efrobns → (c, d)`); documented per function.
5. **Silent-degradation → explicit error.** Where upstream degraded silently on
   a singular matrix (`ethrinv`/`efrobns` `kimerr` flag), the port returns
   `NjoyError::NotConvergent` instead — a deliberate, documented divergence.
6. **Deliberate faithfulness quirks preserved (do NOT "fix" without checking):**
   - `cleb` uses Fortran *integer* `10**n`, which is `0` for negative exponents,
     so `cleb` returns exactly `0.0` for some triples whose true CG coefficient
     is nonzero (e.g. `cleb(2,2,2)`). Reproduced and tested.
   - `efacts`/`eunfac` return `(0.0, 0.0)` for `l` beyond the range upstream
     handles (`l>4`, `l>2`), where Fortran left the out-params untouched.
   - `ign=2` is labelled "CSEWG 239" in a comment but is actually a 240-group /
     241-boundary structure (`groupr.f90:4169`); the code follows the count.

## Known gaps / NOT-yet-ported (ERRORR)

The end-to-end ERRORR pipeline **does not run** — `errorr::run()` and
`errorr::driver::run()` both return `NjoyError::NotPorted`. Unported pieces:

- **Covariance calculation:** `covcal`, `sumchk`, `spcint`, `covbin`, `epanel`.
- **Group averaging / collapse / output:** `grpav`, `grpav4`, `colaps`,
  `covout`, `covadd`, `uniong`, `gridd`, `lumpmt`/`lumpxs`.
- **Resonance-parameter covariance chain (MF32):** `resprx`, `rpxsamm`,
  `rpxlc0/12/2`, `rpxunr`, `rpendf`, `rdumrd2`, `rpxgrp`, `resprp`, `rescon`,
  `ggrmat`, `ggmlbw`, `ssmlbw`, `ssslbw`, `ggunr1`.
- **Covariance readers:** MF=31/33/34/35/40 ENDF readers (`rdsig`, `rdgout`,
  `rdlgnd`, `rdchi`, `stand`, `sigc`, `alsigc`, `musigc`, `fssigc`).
- **`egngpn` union-with-covariance-grid step** and `ign = ±1` (read-from-input).
- **`matrixin`/`matrixej`** math helpers were out of scope for this pass.

## Build & test commands run — ACTUAL output

Release-only, per crate rule. Unit tests run through the crate's 12 GB-capped
wrapper (`scripts/test.sh`).

```
cargo build -p njoy-outram-park-fork --release
    Finished `release` profile [optimized] target(s)   # exit 0, no warnings

crates/njoy-outram-park-fork/scripts/test.sh           # exit 0
```

Test results (2026-07-15), all suites `ok`, 0 failed:

- **lib unit tests: 171 passed / 0 failed** (up from 143 baseline+SAMM;
  +28 new = 15 `errorr::math` + 10 `errorr::groups` + 3 `errorr::driver`).
- Integration test binaries: 12, 7, 12, 1, 20, 11, 11, 4, 5, 2, 3 passed — all 0
  failed. (Longest: 208 s and 95 s ACER/Doppler cases, within the memory cap.)
- New SAMM tests: `samm::mf2::tests::reorder_eliminated_{first,middle,middle_large,last}` — 4/4 pass.

Baseline (pre-change) `cargo build -p njoy-outram-park-fork --release` was also
exit 0, confirming the starting tree compiled.

## File-size-cap deviation (flagged)

`src/errorr/groups.rs` is **1892 lines**, over the crate's hard 1000/1500-line
cap — it is ~1117 lines of literal `const` boundary-table arrays. Follow-up bead
filed under op-cjw to split the tables into a sibling `errorr/groups/tables.rs`
(mechanical, no logic change). `driver.rs` (1126) is slightly over the 1000 soft
cap, within the 1500 ceiling.

## What a human reviewer MUST verify before this is trusted

1. **SAMM fix against a real LRF=7 evaluation** (op-cjw.2): reconstruct a cross
   section for a spin group whose eliminated channel is not first, and confirm
   the widths land correctly. The unit tests prove the array algebra; only a real
   evaluation proves the physics interpretation of "raw channel order" is right.
2. **Licence provenance** of every ported table/kernel: confirm the attribution
   headers name the correct upstream file (esp. `groups.rs`, whose tables come
   from `groupr.f90`, not `errorr.f90`).
3. **`egngpn`/`gengpn` boundary tables** against upstream NJOY output for a few
   `ign` values (SCALE-238, VITAMIN-J/XMAS-172, ECCO-1968) — the numbers were
   machine-extracted from the Fortran `parameter` blocks but never diffed against
   a live NJOY run.
4. **Math kernels** (`egnrl`, `cleb`, `efacts`, `eunfac`) against NJOY reference
   values or closed forms beyond the handful of analytic cases tested here.
5. **ErrorrInput card semantics** against the NJOY manual (LA-UR-17-20093 §ERRORR)
   — defaults, gating conditions, and the ENDF-version-specific card branches.
6. **The `run()` split** (registry no-arg vs `driver::run(input)`) — decide the
   intended public entry point.
7. Confirm none of the deliberate faithfulness quirks (§Assumptions 6) are
   actually latent bugs for the inputs this crate will see.
