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
