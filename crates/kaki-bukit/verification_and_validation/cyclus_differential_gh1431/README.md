# kaki-bukit vs CYCLUS / CYCAMORE: differential record (gh:#1431, epic #1336)

> Research, education and V&V only. Not for nuclear facility operation,
> reactor control, licensing or safety-critical decisions (`RESPONSIBLE_USE.md`).
> AI-assisted draft: unreviewed by a human.

Recorded 2026-11-13. Predictions: `PREDICTIONS.md` (committed before any run,
commit `351ac3f61`). Tests: `tests/cyclus_differential.rs` (exchange, five
scenarios) and `tests/cyclus_differential_toolkit.rs` (kernels).

## Route

| step | what |
|---|---|
| binary | conda-forge (through micromamba 2.0.5, `conda.anaconda.org`): `cyclus 1.6.0 py315he15e396_10` (sha256 `be89d118...21c3`), `cycamore 1.6.0 py315ha906d00_5` (sha256 `2891ee2d...9374`), both BSD-3-Clause. `apt-cache search cyclus` found nothing; `pip download cyclus` found nothing; GitHub tarball download (`codeload`) is blocked here, but `git clone`/`git fetch` of the pinned commits works. Disk: the environment is 2.2 GB under the session scratchpad (not committed). |
| version | **The binary is 1.6.0, not the pinned commits** (cyclus `d4faab7`, cycamore `fee8c80`, both July 2026, after 1.6.0). Recorded, not hidden. |
| is the pinned code the same? | The upstream clones were re-fetched at the pinned commits and at tag `1.6.0` and diffed (`git diff -w --ignore-blank-lines 1.6.0 <pin>`). **Unchanged apart from clang-format whitespace:** `greedy_solver.cc`, `greedy_preconditioner.cc`, `exchange_translator.cc` (0 lines), `exchange_graph.cc`, `uniform_taylor.cc`, `cyc_arithmetic.h`, `comp_math.cc` (0 lines), `toolkit/enrichment.cc`, `decayer.cc`. `composition.cc` only gains `CreateFromNuclide` and `ToString`. **Changed in substance:** `material.cc` (packages by name, unit value, `Absorb` decays both operands when `decay=lazy`, decay threshold `eps` 1e-3 -> 1e-4 and no decay past sim time), cycamore `source.cc/h` (inventory held in a `ResBuf` created in `Build`, `package`/`transport_unit` bids and trades, `max_qty` from `inventory.quantity()`), `sink.cc/h` (`keep_packaging`), `context.cc`, `timer.cc`. |
| consequence | The exchange solver, the toolkit kernels and the arithmetic compared below are source-identical to the pinned commits, so those results stand for the pins. For Source/Sink the changes matter only through packaging (default `unpackaged`, `unrestricted` transport: no effect) and the `ResBuf` inventory; the scenarios use no recipe and no package, but **this is an argument from the diff, not a run at the pins.** A source build at the pins was not done (Boost, HDF5, SQLite, libxml++, Coin-OR, estimated > 1 h on 1 core with the disk at 6 GB). Deferred to the maintainer: see the issue. |
| harness | `harness.cc` links the 1.6.0 `libcyclus.so` and calls cyclus's own `toolkit::Assays/FeedQty/TailsQty/SwuRequired/ValueFunc`, `CycArithmetic::KahanSum`, `compmath::*`, `Composition::Decay`, `Decayer`. Not a transcription: no kernel was re-typed. |
| scenarios | `make_inputs.sh` writes `inputs/*.xml`; `run_cyclus.sh <env>` runs them and dumps the SQLite `Transactions` table (joined to `AgentEntry` and `Resources`) to `cyclus_output/*.csv`, quantity as `%.17g`. |

## Results

### Exchange: Transactions table (24 steps, greedy solver, default exclusive)

| case | rows | bit-identical rows (sorted by time, sender, receiver) | worst rel. diff | identical in emitted order | prediction |
|---|---|---|---|---|---|
| C1a Source(100, inv 1000) -> Sink(80) | 13 | 13 | 0 | 13 | met |
| C1b Source(33.3) -> Sink(100, max_inv 500) | 16 | 16 | 0 | 16 | met (the t=15 trade `0.4999999999998863` is bit-identical; the predicted 1e-12 allowance was not needed) |
| C2a one Source, Sinks A(60), B(70), equal preference | 48 | 48 | 0 | 0 | amounts met; **emitted order prediction missed**, see below |
| C2b same, preferences A=1, B=2 | 48 | 48 | 0 | 0 | amounts met (B 70, A 30) |
| C2c Sources P(60), Q(70) -> Sink(100) | 48 | 48 | 0 | 0 | amounts met (Q 70, P 30) |
| total | 173 | 173 | 0 | 29 | |

Inventories follow: the Sink holds the running sum of the trades, and the
bit-identical C1b trade at t=15 is `500 - 15*33.3` computed from that sum, so
Sink inventory arithmetic agrees to the bit across all 16 steps.

**The tie-break outcomes agree**: with equal preference the same sink is filled
(A 60, B 40); P/Q: Q 70, P 30. **First divergent operation in C2a/b/c: none in
the exchange.** The only difference is the order in which a step's rows are
listed (`identical in emitted order` column). Cause, from the pinned source:
`trade_executor.h` records Transactions by iterating `all_trades`, a
`std::map<std::pair<Trader*, Trader*>, ...>` keyed by the **heap addresses** of
the two facility objects (`RecordTrades`), and executes suppliers by iterating
`std::set<Trader*>`. The order is therefore an allocator artefact, not a
solver rule; the port lists trades in solver order. This was proved by reading
the code and by the set equality of all 48 rows per case; no second CYCLUS
build was run to show the order moving with addresses. The tests compare after
sorting and do not assert emitted order.

### Toolkit kernels

| case | n | bit-identical | worst rel. diff | prediction |
|---|---|---|---|---|
| C4 SWU, feed, tails (10 assays x 2 quantities) + `ValueFunc` (10) | 70 | 70 | 0 | met (<= 1e-14; better than predicted) |
| C3a `KahanSum`, 1005-term list | 1 | 1 | 0 | met |
| C3b `comp_math` add, sub, sum, normalise | 13 | 13 | 0 | met |
| C3c decay, CRAM48 (port default) vs `Composition::Decay` (CRAM14), 5 nuclides, 1/400/4000 steps of 30 d | 20 | 0 | 2.84e-8 | **missed on 1 of 20** (see below); other 19 <= 2.4e-11 |
| C3d decay, uniformized Taylor (`tol` 1e-3), `Decayer` vs port, slow nuclides, 1/40/4000 steps | 18 | 13 | 1.95e-15 | met |

C3c miss, explained and pinned in the test: Y-90 at 4000 steps is 9.3e-8 of a
unit inventory; the port and CYCLUS differ by 2.8e-8 relative (2.6e-15
absolute). The analytic Bateman value (Sr-90 -> Y-90, `exp(-lambda_Y t)`
underflows) is matched by the port to 1.4e-13 and by CYCLUS to 2.8e-8: CRAM14's
error is absolute (about 1e-15 of the input) and shows on a trace nuclide. The
tolerance was not changed; the test asserts that this is the only value over
1e-10 and checks it against the analytic solution.

### Upstream defects and drift found

1. **`Decayer` freezes fast nuclides** (C3e). `BuildDecayMatrix` zeroes a decay
   constant when `exp(-2903040000 * lambda) == 0`; the `exp` is the double
   overload, which underflows for `lambda > 2.57e-7 /s` (half-life under ~31
   days), the opposite of the comment ("mostly stable"). Y-90 (3.0e-6 /s) after
   one 30-day step: `Decayer` 0.2019 (grows from its initial 0.2), physical
   3.35e-4. `Decayer` is deprecated upstream and not on the live path; the port
   already documents not porting the heuristic (`decay.rs`), now with a run.
2. **The live decay path is CRAM14** (`composition.cc:148-175`), confirming the
   README: `Decayer` + `UniformTaylor` are dead code except in tests.
3. **Long double range.** Upstream's `UniformTaylor` holds `exp(-alpha t)` in
   `long double` (range to e^-11356); the port uses `f64` (e^-745) and returns
   an error where upstream returns a result (found when Taylor at 400 steps
   errored with `exp(-alpha * t) underflows f64`). Not fixed: `f64` is the
   crate's no_std choice and the solver is not on the live path. Recorded in the
   issue as deferred.
4. **Version drift**: see "Route".

### Port defect fixed

`Sink` had no preferences. Upstream `in_commod_prefs` (one per commodity) is
passed to `AddRequest`; the port hard-coded `DEFAULT_PREF`. Added
`Sink::with_preferences` (translated from `sink.cc`), needed for C2b, which
then matched. Also added `Simulation::transactions()` (sender/receiver
resolved), the equivalent of upstream's Transactions table, which the
differential reads. Both are reached by the tests above plus a unit test.

### Not comparable (gaps)

Reactor refuelling (C5), Enrichment facility, FuelFab, Separations, Storage,
Mixer, institutions: not ported; tracked by #1432. `ProgSolver` (Coin-OR) is
not ported and was not exercised (the run used the greedy solver). Not
compared: `Material` operations (`ExtractRes`/`Absorb`) beyond what Sink/Source
exercise, `ResBuf` directly, recipes with a non-trivial composition through
the exchange, preferences > 2 commodities, exclusive orders.

## Reproduce

```
micromamba create -n cy -c conda-forge cyclus=1.6.0 cycamore=1.6.0
bash make_inputs.sh && bash run_cyclus.sh <prefix of env cy>
g++ -std=c++17 -O2 -I$E/include -I$E/include/cyclus -I$E/include/libxml++-4.0 \
    -I$E/include/libxml2 harness.cc -L$E/lib -lcyclus -lhdf5 \
    -Wl,-rpath,$E/lib -Wl,-rpath-link,$E/lib -o harness
CYCLUS_NUC_DATA=$E/share/cyclus/cyclus_nuc_data.h5 LD_LIBRARY_PATH=$E/lib ./harness 2>/dev/null > cyclus_output/toolkit.txt
cargo test --release -p kaki-bukit --test cyclus_differential --test cyclus_differential_toolkit
```

Data provenance: inputs are authored here; outputs are produced by the
BSD-3-Clause conda binaries listed above; pyne decay data is the
`cyclus_nuc_data.h5` shipped in that package (accessed 2026-11-13).
