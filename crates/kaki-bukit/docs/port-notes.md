# Port notes — deliberate divergences from upstream CYCLUS/CYCAMORE

> ⚠️ **Unverified until validated.** This crate is an AI-assisted draft with no
> human V&V. Not for nuclear facility operation, reactor control,
> safety-critical decision-making, or licensing decisions.

This file is the single place where **every point at which this port does not
do what upstream does** is recorded, with the reason. It exists because a
translation's most dangerous defect is a silent difference: a reader who opens
`material.cc` next to `material.rs` and finds them line-for-line similar will
assume the rest matches too.

Three categories, and the distinction matters:

- **Forced** — the workspace's Rust rules or `no_std` leave no alternative.
- **Hardening** — upstream accepts something that produces a wrong answer
  quietly; this port rejects it.
- **Preserved quirk** — upstream does something arguably wrong and this port
  reproduces it deliberately, because changing a comparison silently is how a
  regression ships.

Upstream reference: CYCLUS `d4faab7ce0566ccb8febfcf50915cdeedee59db0` and
CYCAMORE `fee8c80190e0b91dafccae6b1a3129dc544441e8`, both BSD-3-Clause.

---

## Forced

### Object graphs become arena indices

| Upstream | Here |
|---|---|
| `boost::shared_ptr<ExchangeNode>` | `NodeId(usize)` into `ExchangeGraph`'s `Vec` |
| `Agent* parent_`, `std::set<Agent*> children_` | `Option<AgentId>` / `Vec<AgentId>` |
| `Resource::Ptr` | `Resource`, an owned enum |

The workspace forbids `Box`, `dyn` and lifetime parameters, and names
index-into-a-`Vec` as the replacement for graph and topology links. Beyond
compliance this removes a real question upstream leaves open — who owns a node
once its graph is destroyed.

### `virtual` dispatch becomes enum dispatch

`Agent`'s virtual interface, `Resource`'s two subclasses, and
`toolkit::Function`'s hierarchy all become enums matched at the call site. The
sets are closed and have been for years, so exhaustiveness becomes a compile
error rather than a runtime string comparison against `"Material"`.

The traits are still declared, as a compiler-enforced contract that every
archetype implements the right methods. They are simply not used for dispatch.

### Exceptions become `Result`

Upstream throws `cyclus::ValueError` and friends. Every fallible function here
returns `Result<T, CyclusError>`. The variants map one-to-one; see
`src/error.rs`.

`CyclusError` variants carry `&'static str`, not `String`, because an
allocation on the error path is the one a constrained target can least afford.
Where a *value* identifies the fault, the variant carries it as a field
(`InvalidNuclide(i32)`).

### `boost::math::float_distance` is hand-rolled

Upstream uses Boost in exactly two places — `Arc` construction and the greedy
solver's exclusivity adjustment. Boost is not available and a floating-point
utility crate would be a heavier dependency than the function, so
`limits::float_distance` implements the standard monotone-key construction
Boost documents. Tested at 0, ±1 ULP and across zero.

### `Composition` has no global id, and no cached decay chain

Upstream's `Composition::next_id_` is a mutable process global; `no_std` has no
good way to provide one, and it makes two runs disagree on ids if anything is
constructed in a different order. Identity moved to `Context`.

Upstream also threads a shared `decay_line_` memo through every composition
descended from a common ancestor. That is a `shared_ptr` graph, and the memo is
a cache rather than semantics, so `decay` recomputes. A caller who wants the
memo can hold one.

### Both composition bases are computed eagerly

Upstream converts atom↔mass lazily, caching by mutating the object through a
non-`const` accessor. Doing that here needs interior mutability, which in
`no_std` means a lock or a `Cell`, for no benefit — the conversion is one
multiply per nuclide and every live composition gets asked for both bases
eventually.

### Atomic masses and decay chains are caller-supplied

Upstream compiles in PyNE's atomic-mass table and `Decayer`'s decay-chain data.
The workspace rule is that nuclear data lives in `njoy-outram-park-fork`, so
both become explicit parameters: `AtomicMasses` and `DecayChain`. This crate
ships **no nuclear data**.

`AtomicMasses::MassNumber` is the zero-data approximation (error is the mass
defect — under 1 % everywhere, under 0.1 % for the actinides). It is adequate
for a structural test and **not** adequate for a mass balance anyone will
quote.

### `Context` is split in two

Upstream's `Context` owns the clock, the recipes, the agents, the recorder, the
RNG and the schedules, and every agent holds a `Context*` back into it. A
back-pointer from every agent to a mutable object that also owns those agents
is the exact aliasing Rust prevents. So `Context` keeps the clock, recipes and
schedules; the agent arena and the phase loop live in the simulation driver.
An agent is handed `&Context` for the duration of a phase call rather than
holding a pointer to it, which means **an agent cannot mutate the simulation
from inside `Tick`** — it returns what it wants and the driver applies it.

### `DecayMode::Lazy` is rejected, not downgraded

Upstream's `"lazy"` decay applies decay implicitly whenever a composition is
read, memoised along the shared chain. Besides needing the memo graph, "decay
happens when you look at it" makes a result depend on which accessors a caller
happened to invoke. `SimInfo::validate` rejects it rather than silently
behaving like `"manual"`.

---

## Hardening

### `compmath` comparisons report *which* nuclide is invalid

`compmath::ValidNucs` answers only yes/no, so every caller that wants to report
the problem has to re-scan. `comp_math::first_invalid_nuc` returns the
offending key.

### `Material::extract_comp` rejects over-extraction explicitly

Upstream subtracts, thresholds, and lets `Composition::CreateFromMass` throw
`ValueError("negative quantity in CompMap")` when the subtraction went
negative. So it *is* rejected, but incidentally, and the message names the
composition rather than the extraction. Here the check is explicit and the
error says what happened: "extraction requests more of a nuclide than the
material holds".

### Duplicate recipe names are rejected

`Context::AddRecipe` upstream silently overwrites, so a duplicated recipe name
in an input deck produces a simulation that depends on parse order.
`Context::add_recipe` returns a `KeyError`; `replace_recipe` is the explicit
overwrite.

### The agent hierarchy is enforced

Upstream's `Agent::Connect` accepts any parent, so a malformed input deck can
put a facility directly under a region and the simulation runs with a quietly
wrong hierarchy. `AgentRole::may_sit_under` makes the nesting checkable and the
driver rejects a violating build.

### `Lifetime` is an enum, not `-1`

Upstream encodes "infinite" as `-1` in an `int`, which is precisely the in-band
sentinel that gets compared with `<` by accident. `Lifetime::Forever` /
`Lifetime::Steps(n)` cannot be. `from_upstream_int` rejects `0` and negatives
other than `-1`, which upstream accepts and turns into an exit time in the
past.

### `DecayMode` is an enum, not a string

Upstream's `SimInfo::decay` is a `std::string`. A typo produces a silent
no-decay simulation.

### `kahan_sum` sorts with `total_cmp`

Upstream sorts with `std::sort` and `operator<`, which is undefined behaviour
in the presence of NaN because `<` is not a strict weak ordering over floats.
`total_cmp` is a total order over every `f64`. On NaN-free input — every input
upstream intends — the two orderings agree exactly, so this is strictly a
hardening.

---

## Preserved quirks — reproduced deliberately, NOT fixed

### `comp_math::almost_eq`'s zero branch

When either quantity is exactly zero, upstream falls into a branch testing
`|diff| > |diff| * threshold`. For `threshold < 1` and non-zero `diff` that is
always true, so **a nuclide present at zero in one map and non-zero in the
other never compares equal**, however small the difference.

This is reproduced rather than fixed. A caller's tolerance may have been tuned
against it, and silently changing a comparison is how a regression ships. It is
a candidate to raise upstream.

Test: `comp_math::tests::almost_eq_preserves_the_upstream_zero_branch`.

### `Agent::exit_time`'s off-by-one

`exit_time = enter_time + lifetime - 1`, not `+ lifetime`. Upstream's own
comment explains it: decommissioning happens at the *end* of a step, so an
agent with a lifetime of 1 goes through one whole step and is decommissioned on
the step it was created. Reproducing the `- 1` is required for a deployment
schedule to match upstream's.

### The unlimited-capacity sentinel is compared by exact float equality

The greedy solver tests group capacities against
`std::numeric_limits<double>::max()` with `==`, both when computing a capacity
and when decrementing one. Exact float equality is normally a defect; here it
is a deliberate sentinel protocol and the arithmetic depends on it —
decrementing `f64::MAX` must be a no-op. `limits::UNLIMITED` is that value.

### Exclusive orders compare in ULPs, not against an epsilon

Upstream's comment is emphatic: the careful float comparison "is vital for
preventing false positive constraint violations w.r.t. exclusivity-related
capacity". `limits::float_distance` plus `limits::FLOAT_ULP_EQ` reproduces it.
Replacing it with an epsilon comparison would silently change which
all-or-nothing trades clear.

### The greedy solver's sorts are stable

Upstream uses `std::stable_sort`. Rust's `sort_by` is stable and
`sort_unstable_by` is not; using the latter would make the trade schedule
depend on the initial order of equal-preference bids. Floats are compared with
`total_cmp`, never `partial_cmp().unwrap()`.

---

## Not ported at all

See the scope table in `README.md`. In brief: the HDF5/SQLite backends, XML
input loading, the Coin-OR/Cbc mixed-integer LP solver (`ProgSolver`), dynamic
module loading, the Python bindings, and the `Package`/`TransportUnit`
discretisation of trades.

Of these, **packaging is the only genuine feature gap rather than an
infrastructure boundary**: recent upstream splits a trade into shippable
package-sized units, which changes the *numbers* a simulation produces, not
just how they are stored. A port aiming to reproduce a modern upstream run will
need it.

---

## What would close the biggest hole

Nothing here has been compared against an actual Cyclus run. Every check is
against a closed form or a hand-worked case. Building upstream Cyclus, running
a simple simulation, committing its trade schedule under `reference-data/`, and
diffing this port against it is the highest-value next step — and is exactly
how `petir` and `outram-park-fork-liggghts` earned their cross-code evidence.

---

## Late finding: the decay solver is upstream's *deprecated* one

**Recorded 2026-09-16, during review of the decay port (`op-svoc`, follow-up
`op-i68k`). This is the most consequential divergence in the crate and it was
found by reading the upstream call graph, not the upstream file.**

`src/decay.rs` is a faithful translation of `cyclus/src/uniform_taylor.cc`. The
problem is what calls it. Upstream, `UniformTaylor::MatrixExpSolver` is
referenced from exactly one site — `decayer.cc:159` — and `Decayer` is marked
DEPRECATED in its own header and **is never instantiated anywhere in Cyclus**.
`decayer.h` is `#include`d by `composition.cc` and `material.cc`, which is what
makes it look live, but no `Decayer` object is ever constructed.

Upstream's **actual** decay path is CRAM. `Composition::NewDecay`
(`composition.cc:136-168`) builds a sparse decay matrix from
`pyne_cram_transmute_info` and calls `pyne_cram_expm_multiply14` — a
14th-order Chebyshev Rational Approximation.

### What follows from this

1. **This port's decay results will not reproduce a modern Cyclus run.**
   Uniform Taylor and CRAM are different algorithms with different truncation
   behaviour. Any claim that this crate "matches Cyclus" must exclude decay
   until a CRAM backend exists.
2. **The Uniform Taylor truncation error is one-signed, so it accumulates.**
   Every discarded term of the truncated Poisson series is non-negative, so
   the loss does not cancel across repeated steps. Measured at upstream's
   default `tol = 1e-3`: **9.5e-4 (0.095 %) atom loss over one year** of the
   Sr-90 chain, and an isobaric Sr-90 → Y-90 → Zr-90 chain — which conserves
   mass exactly by construction — **loses 0.093 % of it**. Tightening to
   `1e-12` reduces the residual to ~1e-13 and costs only 172 series terms
   against 127, because the term count grows logarithmically in the tolerance.
   This is why `decay_*_with_tol` variants exist; upstream exposes no way to
   ask. CRAM does not have this failure mode.
3. **A separate real defect sits in the same unreachable code.**
   `Decayer::BuildDecayMatrix` contains:

   ```cpp
   // Gross heuristic for mostly stable nuclides 2903040000 sec / 100 years
   if (static_cast<long double>(exp(-2903040000 * decay_const)) == 0.0)
     decay_const = 0.0;
   ```

   The comment says "mostly stable", but the test fires when `lambda` is
   **large**. `exp` underflows to zero in `double` below about `-745`, so the
   branch triggers for `lambda > 2.57e-7 s^-1` — **every nuclide with a
   half-life under about 31.3 days is frozen as stable**. Y-90 (2.67 d),
   Mo-99 (2.75 d), Xe-133 (5.25 d) and I-131 (8.02 d) all qualify. The
   `static_cast<long double>` does not help: it is applied to the result of a
   `double` `exp`, after the underflow.

   The port does **not** reproduce this. It reports the underflow as
   `CyclusError::Value` instead, covered by
   `decay::tests::an_underflowing_step_is_refused`.

   Because the code is unreachable upstream, this is recorded here rather than
   reported as a live Cyclus bug. Anyone raising it upstream should confirm
   reachability first.

### Reuse, not reimplementation

`crates/outram-park-fork-onix/src/cram.rs` **already implements CRAM** in this
workspace — `cram16()` plus `clamp_nonnegative()`. A CRAM backend for this
crate must reuse or port from there rather than writing a third
matrix-exponential routine, per the workspace's search-before-building rule.
Uniform Taylor should stay alongside it: it is verified against the
closed-form two-species Bateman solution to 7.7e-11 at tight tolerance, which
makes it a useful independent check on whatever CRAM produces.

---

## A preserved quirk that deserves a maintainer's decision

### `AddMutualReqs`' coefficient is the reciprocal of what its own comment promises

Found 2026-09-16 during the exchange port. Unlike the other preserved quirks
above, here upstream **states its intended behaviour in prose and then does
not implement it**, which makes this the one candidate in this file for being
reported upstream rather than merely reproduced.

`request_portfolio.h` documents the feature (lines 78-84):

> A default constraint will add unity for normal requests in the portfolio,
> but will add a weighted coefficient for requests that meet the same mutual
> demand. For example, if 10 kg of MOX and 9 kg of UOX meet the same demand
> for fuel, coefficients are added such that **a full order of either will
> determine the demand as "met"**. In this case, the total demand is 9.5, the
> MOX order is given a coefficient of **9.5 / 10**, and the UOX order is given
> a coefficient of 9.5 / 9.

The code (lines 150-161) computes the reciprocal:

```cpp
mass_coeffs_[r] = r->target()->quantity() / avg_qty;   // 10 / 9.5
```

The difference is observable, and only the comment's value achieves the
comment's stated intent:

| coefficient | a 10 kg MOX order consumes | outcome |
|---|---|---|
| `9.5/10 = 0.95` (comment) | `10 x 0.95 = 9.5` | exactly the 9.5 demand — "met", as promised |
| `10/9.5 = 1.0526` (code) | `10 x 1.0526 = 10.53` | over the 9.5 demand, so the constraint binds at **9.025 kg** |

No upstream test covers it: `request_portfolio_tests.cc` never calls
`AddMutualReqs`.

**This port reproduces the code, not the comment**, and pins `9.025` in a test
so that changing it later is a deliberate act. That follows the policy above —
silently changing a comparison is how a regression ships — but the case for
this one being a genuine upstream bug is much stronger than for the others,
because the intent is written down and unmet.

Raising it with the Cyclus project is a maintainer decision, not an agent's.

---

## Structural rules the driver enforces that upstream does not

### Only facilities trade

Upstream, `Trader` is implemented by `Facility` alone — a `Region` or
`Institution` decides what gets built, never what gets bought and sold. Nothing
enforces that: the interface is available to any agent, and an archetype that
implemented it at the wrong level would quietly enter the market.

`Simulation::trading_agents` filters to `AgentRole::Facility` before the
exchange runs, so an institution cannot enter the market whatever archetype it
is given. This was found by an integration test, not by reading: a hierarchy
built with placeholder `Source` agents at the region and institution levels had
those placeholders outbidding the real mine, and the simulation moved 400 kg
where 125 kg was correct. That is exactly the failure mode the filter now makes
impossible.
