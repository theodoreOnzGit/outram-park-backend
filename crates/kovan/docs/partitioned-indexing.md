# Partitioned semantic indexing: plan, implementation and results (#1668)

~~**Status: plan for maintainer review, 2026-11-19 (GitHub #1668). Nothing in the indexing
pipeline has changed yet.** The measurements below were taken by throwaway
probes in a cloud session (4 cores, 15 GB RAM, rust-analyzer 1.99.1
`58cf8f9 2026-10-13`); they are not yet a kovan command.~~
**UPDATED 2026-11-19 (same day):** Phases B–G are implemented
(`kovan-cli index-plan`, `index-partition`, `scip-merge`; Pages job matrix;
local Index) and the partitioned indexer is the default on Linux and macOS.
§2 is the feasibility gate as first recorded; **§5 holds the whole-workspace
results**, including the equivalence check against the monolithic index. All
measurements: a cloud session (4 cores, 15 GB RAM plus a 12 GiB swap file for
the monolithic reference), rust-analyzer 1.99.1 `58cf8f9 2026-10-13`. Not yet
measured on GitHub's runners (§5.5).

Guiding principle (maintainer brief, 2026-11-19): **partition computation,
not coverage.** Every workspace crate is still indexed; no single process
analyses the whole workspace.

## 1. What kovan actually needs from rust-analyzer (Phase A audit)

The monolithic step is one `rust-analyzer scip` over the whole workspace
(`crates/kovan-web/web/data.sh` `ensure_scip`, `kovan-cli index`,
`kovan-cli call-graph --backend scip`). Everything after it is kovan's own
code and is already fast:

| Step | Where | Uses rust-analyzer? |
|---|---|---|
| Function list, ids, signatures, docs, upstream, history, citations | `commands::call_graph`, `kovan_common::call_graph::*`, `code_walk` source scanner | no |
| "What does the reference at this position resolve to?" | `crate::scip` (`ScipIndex::nearest_definition`) | **yes: the only use** |
| `kovan.toml` folders, links, hashes, reach | `kovan_common::code_index::*` | no |
| Per-crate cache key | `kovan-cli call-graph-keys` (transitive over workspace dependencies) | version string only |

**The fact the plan rests on.** `crate::scip` resolves a reference by its
SCIP **symbol string**, looked up among the definition occurrences of
**every** document, then disambiguated by location. So SCIP indexes written
for **disjoint sets of crates merge by taking the union of their
documents**, provided the symbol strings agree. Graph storage, symbol
representation, call-graph and dependency-graph generation, queries,
visualisation and incremental updates need no change (brief §5): the
adapter is a merged SCIP file, read by the existing reader.

## 2. Feasibility gate (Phase B): bounded indexing works with the shipped rust-analyzer

**Method.** For a partition (a set of workspace members):

1. `cargo check --release -p <members> --all-targets --message-format=json`
   builds the closure's build scripts and proc macros (OUT_DIR, build-script
   cfgs and env, proc-macro dylibs).
2. A `rust-project.json` is generated from `cargo metadata` holding **only the
   partition's dependency closure**: one crate per library target (cfg from
   the workspace's resolved features plus build-script cfgs, env, edition,
   version, deps), plus each member's tests, examples and bins; **only the
   members are `is_workspace_member`**; `test` cfg on members only.
3. `rust-analyzer scip <view> --exclude-vendored-libraries`, where `<view>`
   is a per-worker directory holding that `rust-project.json` and symlinks to
   the workspace's top-level entries (so concurrent workers never write into
   the checkout, and document paths stay workspace-relative).

`--exclude-vendored-libraries` is essential, and the reason was found by
reading the source (`ra_ap_ide` 0.0.358 `StaticIndex::compute`): a library
crate whose files lie under the scip root counts as **vendored** and is
indexed by default, so without it every workspace dependency of the
partition was indexed again (the `changi` partition wrote 182 `petir` and
85 `raffles` documents).

**Results (2026-11-19).** Peak RSS is the waited child's `ru_maxrss`.

| Partition | Closure (packages) | Documents written | Wall | Peak RSS | Index |
|---|---|---|---|---|---|
| `redhill` (no deps) | 1 | 1 (`crates/redhill`) | 3 s | **0.5 GiB** | <0.1 MB |
| `changi` (dep: `petir`) | 184 | 78 (`crates/changi` only) | 40 s | **5.8 GiB** | 9 MB |
| `petir` | 175 | 263 (`crates/petir` only) | 117 s | **6.1 GiB** | 24 MB |
| *whole workspace, monolithic (recorded, not re-run)* | *1,033* | *all* | *222–292 s desktop; 27–60 min CI* | ***~16 GiB*** | *~349 MB* |

**Correctness checks (2026-11-19).**

- **Cross-partition identity:** `changi` references **51** distinct `petir`
  symbols; **51 of 51** are defined, by the identical string, in `petir`'s own
  partition index. Merging needs no symbol translation.
- **Bounding loses nothing for the members:** `changi`'s 78 documents are
  **identical** (every occurrence) with and without its dependencies' bodies
  indexed, and identical from the view directory and from the root.
- The `--exclude-vendored-libraries` run of `changi` took 40 s against 115 s
  with the dependencies indexed, at similar memory.

**What this did not show yet (the open risks, Phase B/C work; each is answered in §5):**

1. ~~**Equivalence with the monolithic cargo-based index** is not measured:
   the monolithic run needs ~16 GiB, more than this machine has.~~
   **MEASURED (§5.2):** with a 12 GiB swap file the monolithic run completed
   (966 s, 13.2 GiB resident); the merged graph holds every one of its edges.
   The rest of this item was the plan: Phase C
   compares the kovan call graph built from merged partitions with the one
   from a monolithic index (on a ≥32 GB machine or the maintainer's), using
   the existing `kovan_common::call_graph::compare`.
2. **Memory scales with the dependency closure, not the crate's size**
   (0.5 GiB with `std` alone; ~6 GiB for 175–184 packages). The largest
   closures (`outram-park-digital-twin-engine` with 48 direct deps,
   `kovan` 35, `dhoby-ghaut` 34) ~~are unmeasured~~ **were measured (§5.1):
   10.0–12.0 GiB, over the 8 GiB target**. If one exceeds the 8 GiB
   target alone, that is reported and investigated (finer cfg/target split,
   or `ra_ap_*` with a shared dependency database), never hidden.
3. **Feature resolution:** the generated project takes features from the
   workspace's unified `cargo metadata` resolve (what the monolithic run
   sees), but build scripts come from a `-p` check of the partition, whose
   feature set may differ. ~~To be measured in Phase C.~~ **Not measured on
   its own;** the end-to-end equivalence in §5.2 would show a feature
   difference that changed a resolved call, and none did.
4. **cfg coverage:** native target only, `test` on members, as today.
   wasm32 is a separate configuration axis, tracked in the coverage report,
   not silently assumed.

## 3. Why not `ra_ap_*` or rustc code (brief §3.2–3.3)

`ra_ap_project_model`, `ra_ap_load-cargo`, `ra_ap_ide` and
`ra_ap_rust-analyzer` 0.0.358 (MIT OR Apache-2.0, compatible) were read for
the gate. The shipped binary already exposes what is needed (a
`rust-project.json` workspace, `--exclude-vendored-libraries`), so linking
the libraries buys nothing yet and would add a heavy, weekly-versioned
dependency that has to be kept in step with the toolchain. **Kept as the
fallback** if a partition's closure is too large: an in-process
`ra_ap_load_cargo` + `StaticIndex` worker could load one closure once and
index several partitions from it. No rustc algorithm is needed:
rust-analyzer already supplies name resolution, inference, trait impls and
cross-crate metadata. No compiler fork.

## 4. Plan

Each phase ends with its tests and measurements recorded here.

| Phase | Deliverable | Acceptance |
|---|---|---|
| **B** (finish) | `kovan-cli index-partition --crates a,b --out <dir>`: cargo check → `rust-project.json` (Rust port of the probe) → view dir → `rust-analyzer scip --exclude-vendored-libraries` (with `scip_stack`'s stack fix) → `<dir>/partition.scip` + `partition.json` (members, closure, key, toolchain, RA version, cfg/target, wall, peak RSS, documents, exit) | per-crate runs on `petir`, `changi`, and the three largest closures, memory and time recorded; members' documents only |
| **C** | `kovan-cli scip-merge`: union of documents; refuses mixed RA versions, duplicate documents, or a missing member; writes a coverage report. Then `call-graph --scip merged` | call graph from merged partitions vs. monolithic, compared with `call_graph::compare`; every difference explained |
| **D** | `kovan-cli index-plan`: cargo metadata → dependency graph → partitions under a memory budget (estimate from measured closure size, refined by recorded peaks) → deterministic manifest. Pages: a `plan` job → a matrix of partition jobs (each its own runner) → a `unify` job that publishes only when coverage is complete | full Pages build with no monolithic scip; a failed partition fails only itself, others' artifacts kept |
| **E** | partition key = `call-graph-keys` of its members (already transitive) + `Cargo.lock` hash + toolchain + RA version + target + features; `actions/cache` per partition | an unchanged partition is reused; a change below re-indexes its dependents only |
| **F** | local kovan's Index runs the same planner and workers as child processes (concurrency, memory budget, cancel, progress, retry) | local full and incremental index without GitHub |
| **G** | the partitioned indexer becomes the default; monolithic scip kept as fallback (`KOVAN_INDEXER=monolithic`) until it has matched on several builds | brief §12 criteria |

Web kovan needs no format change: it already loads the call graph one crate
at a time (`call-graph --split-dir`). Browser-native indexing of uploaded
source (brief §8) is secondary and not started.

**Configurable (Phase D/F):** maximum concurrent workers, per-worker memory
budget, partition size, retries, timeout.

## 5. Results on the whole workspace (2026-11-19)

### 5.1 Every member in seven bounded partitions

`kovan-cli index-plan` (max closure 750 packages, max 8 members per
partition, largest closure first) splits the 49 members into 7 partitions;
`outram-park-digital-twin-engine` stands alone because its closure (818)
exceeds the cap by itself. Each partition was run with
`kovan-cli index-partition` one after another (the local default), with the
generator as committed (partition schema 2). Peak RSS is rust-analyzer's
`VmHWM`, sampled every second (the `cargo check` before it is a separate,
smaller step); the monolithic figure is the largest waited child's
`ru_maxrss`, the same process.

| Partition | Members | Closure (packages) | Documents | Wall | Peak RSS |
|---|---|---|---|---|---|
| p00 | 1 (`outram-park-digital-twin-engine`) | 818 | 216 | 188 s | **10.9 GiB** |
| p01 | 8 (`dhoby-ghaut`, `kovan`, `kovan-web`, …) | 736 | 944 | 244 s | **12.0 GiB** |
| p02 | 8 (`boon-lay`, `tampines-steam-tables`, …) | 617 | 1,177 | 243 s | **10.0 GiB** |
| p03 | 8 (`njoy-outram-park-fork`, `outram-mc-libs`, …) | 358 | 1,580 | 350 s | **10.3 GiB** |
| p04 | 8 (`changi`, `outram-foam-*`, …) | 163 | 859 | 119 s | 6.4 GiB |
| p05 | 8 | 121 | 628 | 80 s | 4.8 GiB |
| p06 | 8 (`petir`, `bedok`, …) | 109 | 418 | 134 s | 5.9 GiB |
| **all, merged** (`scip-merge`) | 49 | — | 5,822 | 1,358 s in sequence | **12.0 GiB** worst worker |
| *monolithic, same tree* | 49 | *all* | — | *966 s* | ***13.2 GiB resident*** (plus swap; ~16 GiB recorded on larger machines) |

The merged index is 550 MB (monolithic 545 MB). Every partition wrote only
its own members' documents (`outside_members` empty), and the merge's
coverage check against the plan passed.

**Against the targets.** Peak memory per worker fell from the monolithic
13.2 GiB (which needed swap on this 15 GB machine) to at most 12.0 GiB, and
three of seven partitions sit under 8 GiB. ~~**The 8 GiB per-worker target is
not met for the four GUI- or GPU-heavy closures (10.0–12.0 GiB)**~~
**TARGET REVISED 2026-11-19 (maintainer: "Okay 14GB acceptable"): the
per-worker target is 14 GiB, and every partition meets it (worst 12.0 GiB).**
Why 8 GiB was out of reach, kept for the record: memory
follows the dependency closure (§2, item 2), and those closures are
dominated by `wgpu`/`egui`/`eframe` and their platform crates, which every
member that draws a window pulls in. A partition cannot be smaller than its
largest member's closure, so a finer plan does not help p00; what would is
indexing several partitions from one loaded dependency database (`ra_ap_*`,
§3), not started. Two cheaper levers were measured and ruled out
(2026-11-19): dev-dependencies are not the cause (the engine's library alone
needs 834 packages, 839 with dev-dependencies; likewise `dhoby-ghaut`
747/749, `kovan` 707/712), so splitting tests and examples from libraries
would not help; and rust-analyzer's syntax-tree cache is not either (p00
with `lru.capacity = 16`: 11,130 MiB against 11,139 MiB), because the peak
is the semantic database of the closure, held for the whole run. Sequential wall time is 1.4× the monolithic run, the price
of loading shared dependencies once per partition; on CI the partitions run
in parallel, so the critical path is the slowest one (350 s plus setup).

### 5.2 Equivalence with the monolithic index

**Method.** The monolithic `rust-analyzer scip` (as `data.sh` runs it) and
the seven partitions were run on **the identical tree** (HEAD `4b0737ec6`,
empty `git diff`, recorded before and after the monolithic run), and the
call graph was built from each with `kovan-cli call-graph --scip`, then
compared with `kovan-cli call-graph-diff` (`call_graph::compare`). An edge
matches when caller, callee and kind agree; lines are compared too.

**Result.**

| | monolithic (A) | merged partitions (B) |
|---|---|---|
| functions | 66,648 | 66,648 (all in both) |
| edges | 176,329 | 176,340 |
| in both, same lines and kind | **176,329** | |
| only in A | **0** | |
| only in B | | **11** |
| unresolved: closure / macro / trait / other | 20,276 / 12 / 1,278 / 7,537 | identical |
| unresolved: no definition | 3,677 | 3,657 |

**Every edge of the monolithic graph is in the merged graph, with the same
lines and kind.** The 11 extra edges are all calls through a **cross-package
`#[path]` module**: `dhoby-ghaut`'s web examples include
`outram-mc-libs/examples/common/*.rs` and `outram-mc-libs/src/vv/lct008.rs`
(10 edges), and `tampines/tests/pcice.rs` includes
`tampines-steam-tables/tests/common/toro_exact.rs` (1). The monolithic run
indexes such a file only in its owning package, so the including crate's
calls into it have no definition (they are among A's 20 extra
no-definition entries). The partitioned run indexes the file in both
packages' partitions, and `scip-merge` combines the two documents, so both
crates' references resolve. These are real calls, correctly attributed; the
partitioned graph is a strict superset.

**Which number to quote:** 100 % recall of the monolithic edges (0 missed)
on an identical tree. Jaccard (99.99 %) counts the 11 extra real calls
against it and is not the measure of agreement.

**How it got there (kept because each step was a real defect).** The first
whole-workspace merge (generator as first written) missed 459 monolithic
edges (99.74 % recall) in `boon-lay`, `dhoby-ghaut`, `kovan` and
`outram-mc-libs`, all same- or cross-package `#[path]` modules. Three fixes
followed: include the package root rather than the root module's folder;
include the folders a member's `#[path]` attributes reach; combine documents
with the same path in the merge. The rerun then missed 16 `kovan-literature`
edges into `tests/zotero_db/mod.rs`, a folder `kovan`'s tests also include
by `#[path]`: when two crates in one rust-analyzer run include the same file,
it is indexed under one of them only, and here not under its owner. A member
now leaves a `#[path]` folder to a member of the same partition that owns it
(`not_owned_by`, schema 2), and the third run gave the table above. The
throwaway comparison scripts are not committed; the method above reproduces
them.

### 5.3 Where it runs

- **Pages (`.github/workflows/pages.yml`):** `index-plan` → a job matrix
  `index-partition` (one runner per partition, `fail-fast: false`, cached
  by partition key with `actions/cache`) → `index-unify` (`scip-merge
  --plan`, which refuses a missing member or a failed partition) →
  `callgraph`, which uses the merged index through `data.sh`'s
  `KOVAN_SCIP_FROM`. If unify fails, `callgraph` falls back to the
  monolithic run and says so; an incomplete merge is never published as
  complete.
- **Local kovan (desktop Index, `kovan-cli call-graph --backend scip`,
  `kovan-cli index`):** the same planner and workers, run in-process with
  cancel, progress, retries and reuse by partition key under
  `target/kovan-scip/`. **Default one partition at a time**
  (`KOVAN_INDEX_JOBS=1`; maintainer choice, 2026-11-19), so a desktop's
  peak is one worker's (§5.1), below the monolithic run's.
  `KOVAN_INDEXER=monolithic` restores the single whole-workspace run; other
  knobs: `KOVAN_INDEX_MAX_CLOSURE`, `KOVAN_INDEX_MAX_MEMBERS`,
  `KOVAN_INDEX_MEMORY_MIB`, `KOVAN_INDEX_TIMEOUT`, `KOVAN_INDEX_RETRIES`.
- **Windows stays monolithic by default**: the worker's view directory is
  made of symlinks, which Windows does not grant by default.

### 5.4 Tests

`crates/kovan/tests/index_partition_cli.rs` runs the whole chain on a
throwaway two-crate workspace: plan → two partitions → merge (and its
refusal of a missing member) → call graph equal to the monolithic one plus
exactly the cross-package `#[path]` edge; the local default is partitioned,
a second run reuses both partitions, and `KOVAN_INDEXER=monolithic` still
works. Unit tests cover the planner, closure and `rust-project.json`
(`kovan_common::code_index::partition`), the merge (`crate::scip`), and the
key, coverage, settings and `#[path]` ownership
(`commands::index_partition`).

### 5.5 Phase status and what is left

| Phase | Status |
|---|---|
| B `index-partition` | done; every member measured (§5.1) |
| C `scip-merge` + equivalence | done; 100 % recall on an identical tree (§5.2) |
| D `index-plan` + Pages matrix | done; ~~**not yet run on GitHub**~~ **first GitHub run passed 2026-11-19** (§5.6) |
| E partition cache key | done (members' call-graph keys + `Cargo.lock` + host + rustc + rust-analyzer + schema); reuse shown locally |
| F local Index | done |
| G default + fallback | partitioned is the default on Linux/macOS; monolithic kept as `KOVAN_INDEXER=monolithic` until several CI builds have matched (brief §12) |

Open: ~~the 8 GiB per-worker target for the GUI closures (§5.1)~~ (target
revised to 14 GiB, met; §5.1); Windows
(symlink view); a wasm32 configuration axis (not indexed, as before).

### 5.6 First run on GitHub's runners (2026-11-19)

`pages.yml` run 44524814281, dispatched on `develop` at `1f4e9cc47` (a
dispatch builds every job but deploys nothing; `deploy` runs only from
`web-deploy`). Standard `ubuntu-latest` runners (16 GB), toolchain and
rust-analyzer **1.98.0** (the Pages pin), one runner per partition, all
seven started together at 12:43:34 UTC.

| Partition | Peak RSS | `index-partition` wall (incl. `cargo check`) |
|---|---|---|
| p00 `outram-park-digital-twin-engine` | 11,536 MiB | 301 s |
| p01 `dhoby-ghaut`, `kovan`, … | **13,026 MiB** | 338 s |
| p02 | 10,749 MiB | 288 s |
| p03 | 10,834 MiB | 392 s |
| p04 / p05 / p06 | not read (jobs 2–4 min) | — |

- **Every partition succeeded; `index-unify` merged all seven** with
  coverage checked against the plan (4 s).
- **`callgraph` used the merged index:** its log reads `data.sh: using the
  SCIP index …/merged.scip (KOVAN_SCIP_FROM); rust-analyzer scip not run`,
  then `call graph of 49 crates` and `every crate built from this commit`.
  The monolithic fallback and its swap step were skipped.
- **Against the 14 GiB target:** met, with p01 the tightest (1.3 GiB to
  spare). It is ~1.0 GiB above the local 1.99.1 figure; the rust-analyzer
  version is the likely reason (not isolated). A partition over budget is
  stopped by `--memory-mib 14336` and reported, and `callgraph` then falls
  back to the monolithic run.
- **Wall time:** start to call graph written, 36 min (12:32 → 13:08). The
  critical path is `index-plan`'s kovan-cli build (10 min), the slowest
  partition (~7.5 min with setup), unify (<1 min) and `callgraph` (16 min,
  mostly building kovan-cli again and writing the split graph; the graph
  itself took 81 s). The monolithic scip step alone had taken 27–60 min on
  CI (§2 table).
- First run, so every partition cache was a miss; reuse on GitHub is
  shown on the next run whose partitions are unchanged.
