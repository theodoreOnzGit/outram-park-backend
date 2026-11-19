# Partitioned semantic indexing: plan and feasibility (Phase A, Phase B gate)

**Status: plan for maintainer review, 2026-11-19 (GitHub #1668). Nothing in the indexing
pipeline has changed yet.** The measurements below were taken by throwaway
probes in a cloud session (4 cores, 15 GB RAM, rust-analyzer 1.99.1
`58cf8f9 2026-10-13`); they are not yet a kovan command.

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

**What this does not show yet (the open risks, Phase B/C work):**

1. **Equivalence with the monolithic cargo-based index** is not measured:
   the monolithic run needs ~16 GiB, more than this machine has. Phase C
   compares the kovan call graph built from merged partitions with the one
   from a monolithic index (on a ≥32 GB machine or the maintainer's), using
   the existing `kovan_common::call_graph::compare`.
2. **Memory scales with the dependency closure, not the crate's size**
   (0.5 GiB with `std` alone; ~6 GiB for 175–184 packages). The largest
   closures (`outram-park-digital-twin-engine` with 48 direct deps,
   `kovan` 35, `dhoby-ghaut` 34) are unmeasured. If one exceeds the 8 GiB
   target alone, that is reported and investigated (finer cfg/target split,
   or `ra_ap_*` with a shared dependency database), never hidden.
3. **Feature resolution:** the generated project takes features from the
   workspace's unified `cargo metadata` resolve (what the monolithic run
   sees), but build scripts come from a `-p` check of the partition, whose
   feature set may differ. To be measured in Phase C.
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
