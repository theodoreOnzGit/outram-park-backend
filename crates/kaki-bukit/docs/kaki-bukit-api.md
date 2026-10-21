# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `kaki_bukit`

An independent, `no_std` Rust translation of **CYCLUS**, the agent-based
nuclear fuel-cycle simulator, and **CYCAMORE**, its library of fuel-cycle
facility agents.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is
> **unverified and untrusted** unless a specific verification & validation
> (V&V) case demonstrates otherwise. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

# What Cyclus is, in one paragraph

A fuel-cycle simulation is a set of facilities — mines, enrichment plants,
reactors, repositories — that pass material to each other over discrete
time steps. Cyclus's distinctive idea is that it does *not* hard-wire who
sends what to whom. Each time step it runs a **dynamic resource exchange**:
consumers post [requests](exchange::request), producers answer with
[bids](exchange::bid), each side declares its capacity constraints, and a
solver picks a set of trades. The facilities are agents; the market between
them is recomputed every step. That exchange is the heart of the code and
the heart of this translation.

# Where to start reading

Follow the material, not the module list:

1. A [`Nuc`](nuclide::Nuc) is a nuclide id; a [`CompMap`](comp_math::CompMap)
   maps nuclides to dimensionless quantities.
2. A [`Composition`](composition::Composition) fixes whether those
   quantities are atom or mass fractions, and can convert between them.
3. A [`Material`](material::Material) is a quantity in kilograms plus a
   composition. [`Product`](product::Product) is the same idea for things
   that are not nuclides.
4. Facilities hold resources in a [`ResBuf`](toolkit::res_buf::ResBuf).
5. Each time step, [`exchange`] matches requests to bids.

# `no_std`

This crate is `#![no_std]` and depends on `alloc`. Fuel-cycle bookkeeping is
irreducibly dynamic — a composition is a map whose size is not known until
run time — so `alloc` is required; `std` is not, and is never used. All
numerics come from [`petir`], which is itself `no_std`, so a fuel-cycle
model can be embedded in a controller, compiled to wasm for a browser, or
run on Android without a second implementation of anything.

Because PETIR's transcendentals are one fixed pure-Rust implementation
rather than whatever libm the platform ships, two runs of the same input on
different machines produce **bit-identical** results. For a code whose
output is a mass balance someone will cite, that is worth the constraint.

# Scope: what is NOT ported, and why

This crate is the **simulation kernel only**. The following upstream
subsystems are deliberately absent, and their absence is a design decision
rather than an unfinished task:

| Upstream | Why it is not here |
|---|---|
| `hdf5_back`, `sqlite_back`, `recorder` | Output persistence. Both are C library bindings; neither can be `no_std`. |
| `xml_file_loader`, `xml_parser`, `infile_tree` | Input decks, via libxml2. Same reason. |
| `prog_solver`, `prog_translator`, `OsiCbcSolverInterface` | The mixed-integer LP exchange solver, via Coin-OR/Cbc. A C++ dependency an order of magnitude larger than this crate. The [greedy solver](exchange::greedy) *is* ported, and is upstream's default. |
| `dynamic_module`, `discovery` | Loading agent archetypes from shared libraries at run time. Rust resolves the equivalent at compile time, through the [`AgentKind`](agents::AgentKind) enum. |
| `pyhooks`, `pymodule`, `pyinfile`, the Cython layer | Python bindings. |
| `Decayer` and its bundled chain data | Deprecated upstream in favour of PyNE's decay. More importantly, the workspace rule is that nuclear data belongs in `njoy-outram-park-fork`, so [`decay`] takes a caller-supplied [`DecayChain`](decay::DecayChain) and ships no data of its own. |

A downstream crate that wants persistence or XML input adds it on top; the
kernel does not need to know.

# Translation rules this crate follows

The workspace forbids trait objects, `Box`, and lifetime parameters. Cyclus
is built from all three — `Agent*` polymorphism, `boost::shared_ptr`
resource graphs — so the translation is not mechanical, and the substitutions
are uniform enough to state once:

| Upstream C++ | Here |
|---|---|
| `virtual` dispatch over `Agent*` | [`AgentKind`](agents::AgentKind), an enum matched at each call |
| `boost::shared_ptr<ExchangeNode>` | [`NodeId`](exchange::graph::NodeId), an index into an arena |
| `Resource::Ptr` | [`Resource`](resource::Resource), an owned enum |
| `std::map<Nuc, double>` | [`CompMap`](comp_math::CompMap), a `BTreeMap` — ordered, so results reproduce |
| thrown exceptions | [`Result<T>`](error::Result) |

Every ported file carries a `PROVENANCE` header block naming the upstream
file and commit, so any routine here can be opened next to its source and
read line for line.

## Modules

## Module `agent`

Agent identity, the institutional hierarchy, and agent lifetimes.

# The three-level hierarchy

A Cyclus simulation is a tree, exactly three levels deep below the root:

```text
  Region            a country, or a market
    Institution     an owner/operator — decides what gets built
      Facility      a mine, enrichment plant, reactor, repository
```

Only [`AgentRole::Facility`] agents trade material. Institutions decide
what to build and when to decommission; regions set the demand the
institutions build against. The hierarchy is not decoration — an
institution can only build facilities beneath itself, and a region's growth
target only reaches the facilities under it.

# Identity without pointers

Upstream every agent holds `Agent* parent_` and `std::set<Agent*>
children_`. This workspace forbids that shape, so parentage is stored as
[`AgentId`] indices into the [`Context`](crate::context::Context)'s arena.
The pattern is the one the workspace `CLAUDE.md` names explicitly for graph
and topology links, and it buys something beyond compliance: an
[`AgentInfo`] is `Copy`-cheap to pass around and can be compared, stored
and serialised without the aliasing questions a raw pointer raises.

```rust
pub mod agent { /* ... */ }
```

### Types

#### Struct `AgentId`

A handle to an agent, an index into the simulation's agent arena.

Upstream's `Agent::id()`, which is an `int` assigned from a global counter.
Here it is an arena index, so it doubles as the lookup key and no separate
map is needed.

```rust
pub struct AgentId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Methods

- ```rust
  pub const fn index(self: Self) -> usize { /* ... */ }
  ```
  The underlying index.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AgentId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &AgentId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AgentId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &AgentId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `AgentRole`

Which level of the hierarchy an agent occupies. Upstream `Agent::kind()`,
which returns the string `"Region"`, `"Inst"` or `"Facility"`.

An enum rather than a string, so `ancestor_of_kind` cannot be asked for a
level that does not exist.

```rust
pub enum AgentRole {
    Region,
    Institution,
    Facility,
}
```

##### Variants

###### `Region`

A country or market. Upstream kind string `"Region"`.

###### `Institution`

An owner/operator that builds and decommissions facilities. Upstream
kind string `"Inst"`.

###### `Facility`

A physical facility that holds and trades material. Upstream kind
string `"Facility"`.

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: Self) -> &''static str { /* ... */ }
  ```
  The upstream kind string.

- ```rust
  pub fn parent_role(self: Self) -> Option<Self> { /* ... */ }
  ```
  The role this one must sit beneath, or `None` for

- ```rust
  pub fn may_sit_under(self: Self, parent: Option<Self>) -> bool { /* ... */ }
  ```
  `true` if an agent of this role may be a child of `parent`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AgentRole { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &AgentRole) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AgentRole) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &AgentRole) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Lifetime`

How long an agent lives, in time steps. Upstream `Agent::lifetime()`,
which uses `-1` as its infinite sentinel.

An enum, because `-1` meaning "forever" is precisely the kind of in-band
sentinel that gets compared with `<` by accident. Upstream's integer form
is still available through [`Lifetime::as_upstream_int`] and
[`Lifetime::from_upstream_int`] so a configuration round-trips.

```rust
pub enum Lifetime {
    Forever,
    Steps(i64),
}
```

##### Variants

###### `Forever`

The agent is never decommissioned on account of age. Upstream `-1`.

###### `Steps`

The agent lives this many time steps. Must be positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `i64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn as_upstream_int(self: Self) -> i64 { /* ... */ }
  ```
  Upstream's integer encoding: `-1` for infinite.

- ```rust
  pub fn from_upstream_int(n: i64) -> Result<Self> { /* ... */ }
  ```
  Decodes upstream's integer encoding.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Lifetime { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Lifetime { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Lifetime) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AgentInfo`

The bookkeeping every agent carries, whatever its archetype.

Upstream this is the state on the `Agent` base class. Here it is a plain
struct that each archetype *contains*, rather than inherits — the
composition equivalent of upstream's inheritance, and the shape that lets
[`AgentKind`](crate::agents::AgentKind) stay a flat enum.

```rust
pub struct AgentInfo {
    pub id: AgentId,
    pub parent: Option<AgentId>,
    pub children: alloc::vec::Vec<AgentId>,
    pub role: AgentRole,
    pub prototype: alloc::string::String,
    pub spec: alloc::string::String,
    pub enter_time: i64,
    pub lifetime: Lifetime,
    pub decommissioned: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `AgentId` | This agent's handle. |
| `parent` | `Option<AgentId>` | Its parent in the hierarchy, or `None` for a region. |
| `children` | `alloc::vec::Vec<AgentId>` | Its children, in the order they were built. |
| `role` | `AgentRole` | Which level of the hierarchy it occupies. |
| `prototype` | `alloc::string::String` | The user-facing prototype name from the input deck, e.g.<br>`"LWR"`. Upstream `Agent::prototype()`. |
| `spec` | `alloc::string::String` | The archetype specification, e.g. `":cycamore:Reactor"`. Upstream<br>`Agent::spec()`. |
| `enter_time` | `i64` | The time step at which the agent entered the simulation. Upstream<br>`Agent::enter_time()`. |
| `lifetime` | `Lifetime` | How long it lives. |
| `decommissioned` | `bool` | `true` once it has been decommissioned. |

##### Implementations

###### Methods

- ```rust
  pub fn new(id: AgentId, role: AgentRole, prototype: &str, spec: &str, enter_time: i64) -> Self { /* ... */ }
  ```
  Creates bookkeeping for an agent entering at `enter_time`.

- ```rust
  pub fn with_lifetime(self: Self, lifetime: Lifetime) -> Self { /* ... */ }
  ```
  Sets the lifetime, consuming and returning `self`.

- ```rust
  pub fn with_parent(self: Self, parent: AgentId) -> Self { /* ... */ }
  ```
  Sets the parent, consuming and returning `self`.

- ```rust
  pub fn exit_time(self: &Self) -> Option<i64> { /* ... */ }
  ```
  The final time step of this agent's life, or `None` if it lives

- ```rust
  pub fn retired(self: &Self, now: i64) -> bool { /* ... */ }
  ```
  `true` if the agent has reached or passed its exit time at time step

- ```rust
  pub fn alive(self: &Self, now: i64) -> bool { /* ... */ }
  ```
  `true` if the agent is in the simulation at time step `now`: it has

- ```rust
  pub fn age(self: &Self, now: i64) -> i64 { /* ... */ }
  ```
  The number of time steps the agent has been in the simulation at `now`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AgentInfo { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AgentInfo) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `agents`

Fuel-cycle facility agents, and the driver that runs a simulation of them.

# The trading protocol, in the order it happens

Every time step, [`Simulation::step`] runs the six phases of
[`Phase`](crate::sim::Phase). The middle one is the dynamic resource
exchange, and it is worth following once end to end because every agent in
this module is written against it:

1. **Requests.** Each agent returns [`RequestPortfolio`]s saying what it
   wants — a commodity, a quantity, a target composition, and a preference
   for each supplier. ([`Facility::material_requests`])
2. **Bids.** Each agent is shown every request, grouped by commodity, and
   answers with [`BidPortfolio`]s. A bid names the request it answers and
   the resource it would supply, and its portfolio carries the supplier's
   capacity constraints. ([`Facility::material_bids`])
3. **Solve.** The portfolios are translated into an
   [`ExchangeGraph`](crate::exchange::graph::ExchangeGraph) and matched by
   the [`GreedySolver`], which respects every constraint and prefers
   higher-preference arcs.
4. **Respond.** Each winning supplier is told how much of what it must
   produce, and hands over real resources.
   ([`Facility::respond_to_trades`])
5. **Accept.** Each requester receives them. ([`Facility::accept_trades`])

Nothing moves outside that protocol. An agent cannot push material at
another agent, and cannot reach into the simulation — it is handed
`&Context` for the duration of a call and returns what it wants, which the
driver applies. See [`context`](crate::context) for why.

# A trait for the contract, an enum for the dispatch

[`Facility`] is a trait, so the compiler checks that every archetype
implements the whole protocol. It is **not** used for dispatch — the
workspace forbids trait objects. [`AgentKind`] is an enum over the concrete
archetypes and every call site matches on it.

The payoff is not just rule compliance. Upstream resolves archetypes at run
time by loading shared libraries (`dynamic_module.cc`), so a typo in an
input deck's `spec` string is a run-time failure and adding a new
interface method silently breaks any archetype that does not override it.
Here, adding a variant to [`AgentKind`] makes the compiler point at every
`match` that must handle it.

# What is implemented

| Archetype | Status |
|---|---|
| [`Source`] | implemented |
| [`Sink`] | implemented |
| `Storage`, `Enrichment`, `Reactor`, `Separations`, `FuelFab`, `Mixer` | not yet ported |
| `DeployInst`, `ManagerInst`, `GrowthRegion` | not yet ported |

The unported archetypes are absent rather than stubbed: a stub that
silently trades nothing is worse than a missing variant, because a
simulation built on it runs and produces a plausible, wrong answer.

# Not ported: trade packaging

Recent upstream splits a trade into `Package`-sized units and limits how
many `TransportUnit`s may ship per step. That changes the *numbers* a
simulation produces, not merely how they are recorded, so it is a genuine
feature gap rather than an infrastructure boundary. Agents here trade the
matched quantity in one piece. See `docs/port-notes.md`.

```rust
pub mod agents { /* ... */ }
```

### Modules

## Module `sink`

A material sink: the downstream end of a fuel cycle.

A repository, or any other terminus that accepts material and never
releases it. It requests one or more commodities, limited by a
per-time-step **capacity** and a total **inventory size**.

```rust
pub mod sink { /* ... */ }
```

### Types

#### Struct `Sink`

A facility that accepts one or more commodities and supplies nothing.

# What it asks for

If `inrecipe` is set the sink requests that composition; otherwise it
requests a placeholder composition, since the exchange needs a target
resource to size the request even when the requester does not care what it
receives. Upstream does the same, using a single unit of a nuclide as the
placeholder.

```rust
pub struct Sink {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(in_commods: &[&str]) -> Result<Self> { /* ... */ }
  ```
  A sink accepting `in_commods`, with unbounded capacity and inventory.

- ```rust
  pub fn with_capacity(self: Self, kg_per_step: f64) -> Result<Self> { /* ... */ }
  ```
  Sets the maximum mass accepted per time step, in kilograms.

- ```rust
  pub fn with_max_inventory(self: Self, kg: f64) -> Result<Self> { /* ... */ }
  ```
  Sets the total mass this sink can ever hold, in kilograms.

- ```rust
  pub fn with_recipe(self: Self, recipe: &str) -> Self { /* ... */ }
  ```
  Requests this named recipe rather than a placeholder composition.

- ```rust
  pub fn in_commods(self: &Self) -> &[String] { /* ... */ }
  ```
  The commodities accepted.

- ```rust
  pub fn quantity(self: &Self) -> f64 { /* ... */ }
  ```
  The mass currently held, in kilograms.

- ```rust
  pub fn space(self: &Self) -> f64 { /* ... */ }
  ```
  Remaining room, in kilograms.

- ```rust
  pub fn inventory(self: &Self) -> &ResBuf { /* ... */ }
  ```
  The inventory buffer, for inspection.

- ```rust
  pub fn request_amt(self: &Self) -> f64 { /* ... */ }
  ```
  The most this sink will take this step: the lesser of its per-step

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Sink { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Facility**
  - ```rust
    fn material_requests(self: &mut Self, ctx: &Context, me: AgentId) -> Result<Vec<RequestPortfolio>> { /* ... */ }
    ```
    Requests [`request_amt`](Sink::request_amt) of each accepted commodity.

  - ```rust
    fn respond_to_trades(self: &mut Self, _ctx: &Context, orders: &[Order], _masses: &AtomicMasses) -> Result<Vec<Resource>> { /* ... */ }
    ```
    A sink never supplies anything.

  - ```rust
    fn accept_trades(self: &mut Self, _ctx: &Context, deliveries: Vec<Delivery>, _masses: &AtomicMasses) -> Result<()> { /* ... */ }
    ```
    Stores everything received.

  - ```rust
    fn summary(self: &Self) -> String { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Sink) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `source`

A material source: the upstream end of a fuel cycle.

A mine, or anything else that supplies material without consuming any. It
offers a single commodity, limited by a per-time-step **throughput** and a
lifetime **inventory**, both of which default to unbounded.

```rust
pub mod source { /* ... */ }
```

### Types

#### Struct `Source`

A facility that supplies one commodity and requests nothing.

# Composition behaviour

Upstream: if `outrecipe` is set the source supplies that composition; if it
is empty the source supplies **exactly the composition the requester asked
for**. Both are reproduced. The second is what makes a `Source` usable as a
generic "material appears here" stub in a test deck.

```rust
pub struct Source {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(outcommod: &str) -> Result<Self> { /* ... */ }
  ```
  A source of `outcommod` with unbounded throughput and inventory.

- ```rust
  pub fn with_throughput(self: Self, kg_per_step: f64) -> Result<Self> { /* ... */ }
  ```
  Sets the maximum mass supplied per time step, in kilograms.

- ```rust
  pub fn with_inventory(self: Self, kg: f64) -> Result<Self> { /* ... */ }
  ```
  Sets the total mass this source can ever supply, in kilograms.

- ```rust
  pub fn with_recipe(self: Self, recipe: &str) -> Self { /* ... */ }
  ```
  Supplies this named recipe instead of the requested composition.

- ```rust
  pub fn outcommod(self: &Self) -> &str { /* ... */ }
  ```
  The commodity supplied.

- ```rust
  pub fn supplied(self: &Self) -> f64 { /* ... */ }
  ```
  Total mass supplied so far, in kilograms.

- ```rust
  pub fn remaining(self: &Self) -> f64 { /* ... */ }
  ```
  Mass still available over the source's lifetime, in kilograms.

- ```rust
  pub fn max_qty(self: &Self) -> f64 { /* ... */ }
  ```
  The most this source may supply this step: the lesser of its throughput

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Source { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Facility**
  - ```rust
    fn material_bids(self: &mut Self, ctx: &Context, me: AgentId, commod_requests: &CommodRequests) -> Result<Vec<BidPortfolio>> { /* ... */ }
    ```
    Bids on every request for [`outcommod`](Source::outcommod), up to

  - ```rust
    fn respond_to_trades(self: &mut Self, ctx: &Context, orders: &[Order], _masses: &AtomicMasses) -> Result<Vec<Resource>> { /* ... */ }
    ```
    Produces the matched material and charges it against the inventory.

  - ```rust
    fn accept_trades(self: &mut Self, _ctx: &Context, deliveries: Vec<Delivery>, _masses: &AtomicMasses) -> Result<()> { /* ... */ }
    ```
    A source never receives anything.

  - ```rust
    fn check_decommission(self: &Self, _ctx: &Context) -> bool { /* ... */ }
    ```
    Retires once the lifetime inventory is exhausted. Upstream's

  - ```rust
    fn summary(self: &Self) -> String { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Source) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Types

#### Type Alias `CommodRequests`

Every request in a time step, grouped by the commodity it asks for.

Upstream `cyclus::CommodMap<Material>::type`, which is
`std::map<std::string, std::vector<Request<Material>*>>`. Bidders are shown
this and answer the requests they can serve.

A `BTreeMap` rather than a hash map, so bidders see commodities in the same
order on every run and every platform — the same reproducibility argument
as [`CompMap`](crate::comp_math::CompMap).

```rust
pub type CommodRequests = alloc::collections::BTreeMap<alloc::string::String, alloc::vec::Vec<(crate::exchange::RequestId, crate::exchange::Request)>>;
```

#### Struct `Order`

One order a supplier has won and must now fill.

Upstream passes `std::vector<Trade<Material>>` and expects the supplier to
push `(trade, material)` pairs into an out-parameter. Returning the
resources is clearer and makes the "responded to the wrong trade" mistake
impossible, so [`Facility::respond_to_trades`] returns them in the same
order it was given the orders.

```rust
pub struct Order {
    pub trade: crate::exchange::Trade,
    pub commodity: alloc::string::String,
    pub target: crate::resource::Resource,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `trade` | `crate::exchange::Trade` | The matched trade: which request, which bid, how much. |
| `commodity` | `alloc::string::String` | The commodity being supplied, for an agent that sells more than one. |
| `target` | `crate::resource::Resource` | The composition the requester asked for, so a supplier that offers<br>"whatever is requested" knows what to make. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Order { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Order) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Delivery`

One delivery a requester has received.

```rust
pub struct Delivery {
    pub trade: crate::exchange::Trade,
    pub commodity: alloc::string::String,
    pub resource: crate::resource::Resource,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `trade` | `crate::exchange::Trade` | The trade this fills. |
| `commodity` | `alloc::string::String` | The commodity delivered. |
| `resource` | `crate::resource::Resource` | The resource itself. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Delivery { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Delivery) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `AgentKind`

**Attributes:**

- `NonExhaustive`

The concrete archetypes, dispatched by `match` rather than by `dyn`.

Upstream resolves these at run time from shared libraries; here they are
resolved at compile time, so an unknown archetype cannot reach a running
simulation.

```rust
pub enum AgentKind {
    Source(Source),
    Sink(Sink),
    Inert,
}
```

##### Variants

###### `Source`

A fixed-throughput material source. See [`Source`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Source` |  |

###### `Sink`

A fixed-throughput material sink. See [`Sink`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Sink` |  |

###### `Inert`

An agent with no behaviour of its own.

This is what a [`Region`](crate::agent::AgentRole::Region) or
[`Institution`](crate::agent::AgentRole::Institution) is until the
CYCAMORE archetypes that give them behaviour — `GrowthRegion`,
`DeployInst`, `ManagerInst` — are ported. It holds a place in the
hierarchy and does nothing else.

This is **not** a stub standing in for an unported facility. A
facility that silently trades nothing would make a simulation run and
produce a plausible wrong answer, which is why the unported facility
archetypes are absent rather than inert. Upstream's own base `Region`
and `Institution` genuinely do almost nothing beyond the hierarchy, so
this variant is a faithful translation rather than a placeholder.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AgentKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Facility**
  - ```rust
    fn tick(self: &mut Self, ctx: &Context) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn tock(self: &mut Self, ctx: &Context) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn material_requests(self: &mut Self, ctx: &Context, me: AgentId) -> Result<Vec<RequestPortfolio>> { /* ... */ }
    ```

  - ```rust
    fn material_bids(self: &mut Self, ctx: &Context, me: AgentId, commod_requests: &CommodRequests) -> Result<Vec<BidPortfolio>> { /* ... */ }
    ```

  - ```rust
    fn respond_to_trades(self: &mut Self, ctx: &Context, orders: &[Order], masses: &AtomicMasses) -> Result<Vec<Resource>> { /* ... */ }
    ```

  - ```rust
    fn accept_trades(self: &mut Self, ctx: &Context, deliveries: Vec<Delivery>, masses: &AtomicMasses) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn check_decommission(self: &Self, ctx: &Context) -> bool { /* ... */ }
    ```

  - ```rust
    fn summary(self: &Self) -> String { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AgentKind) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Simulation`

A complete simulation: the agent arena, the context, and the phase loop.

Upstream splits this between `Context` (which owns the agents) and `Timer`
(which drives them). Here [`Context`] holds the simulation-global services
and `Simulation` holds the agents, so an agent can be handed `&Context`
without the aliasing that a `Context` owning it would create.

```rust
pub struct Simulation {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(ctx: Context, masses: AtomicMasses) -> Self { /* ... */ }
  ```
  Creates a simulation over `ctx`, converting compositions with `masses`.

- ```rust
  pub fn add_agent(self: &mut Self, role: AgentRole, prototype: &str, spec: &str, parent: Option<AgentId>, agent: AgentKind) -> Result<AgentId> { /* ... */ }
  ```
  Adds an agent and returns its handle.

- ```rust
  pub fn context(self: &Self) -> &Context { /* ... */ }
  ```
  The simulation context.

- ```rust
  pub fn info(self: &Self, id: AgentId) -> Result<&AgentInfo> { /* ... */ }
  ```
  The bookkeeping for `id`.

- ```rust
  pub fn agent(self: &Self, id: AgentId) -> Result<&AgentKind> { /* ... */ }
  ```
  The archetype state for `id`.

- ```rust
  pub fn trade_log(self: &Self) -> &[(i64, Trade)] { /* ... */ }
  ```
  Every trade that has cleared, with the time step it cleared on.

- ```rust
  pub fn trading_agents(self: &Self) -> Vec<AgentId> { /* ... */ }
  ```
  The handles of every agent that trades this step.

- ```rust
  pub fn live_agents(self: &Self) -> Vec<AgentId> { /* ... */ }
  ```
  The handles of every agent alive at the current time step.

- ```rust
  pub fn run(self: &mut Self) -> Result<()> { /* ... */ }
  ```
  Runs the simulation to completion. Upstream `Timer::RunSim`.

- ```rust
  pub fn step(self: &mut Self) -> Result<()> { /* ... */ }
  ```
  Runs one time step, all six phases in order.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Simulation { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Simulation) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `Facility`

The contract every facility archetype satisfies.

A compiler-enforced interface, **not** a dispatch mechanism — see the
module documentation. Every method has a default that does nothing, so an
archetype implements only the phases it participates in; a pure consumer
never writes a `material_bids`, and a pure producer never writes a
`material_requests`.

Every method takes `&Context` rather than holding one. An agent therefore
cannot mutate the simulation from inside a phase — it returns what it
wants and the driver applies it.

```rust
pub trait Facility {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `summary`: A one-line human-readable state summary, for diagnostics. Upstream

##### Provided Methods

- ```rust
  fn tick(self: &mut Self, _ctx: &Context) -> Result<()> { /* ... */ }
  ```
  Update internal state at the start of a time step, before any trading.

- ```rust
  fn tock(self: &mut Self, _ctx: &Context) -> Result<()> { /* ... */ }
  ```
  Process whatever was received, after trading. Upstream

- ```rust
  fn material_requests(self: &mut Self, _ctx: &Context, _me: AgentId) -> Result<Vec<RequestPortfolio>> { /* ... */ }
  ```
  Say what this agent wants this step. Upstream

- ```rust
  fn material_bids(self: &mut Self, _ctx: &Context, _me: AgentId, _commod_requests: &CommodRequests) -> Result<Vec<BidPortfolio>> { /* ... */ }
  ```
  Answer the requests this agent can serve. Upstream

- ```rust
  fn respond_to_trades(self: &mut Self, _ctx: &Context, _orders: &[Order], _masses: &AtomicMasses) -> Result<Vec<Resource>> { /* ... */ }
  ```
  Produce the resources for the orders this agent won. Upstream

- ```rust
  fn accept_trades(self: &mut Self, _ctx: &Context, _deliveries: Vec<Delivery>, _masses: &AtomicMasses) -> Result<()> { /* ... */ }
  ```
  Take delivery. Upstream `Trader::AcceptMatlTrades`.

- ```rust
  fn check_decommission(self: &Self, _ctx: &Context) -> bool { /* ... */ }
  ```
  Whether this agent should be decommissioned at the end of this step.

##### Implementations

This trait is implemented for the following types:

- `Sink`
- `Source`
- `AgentKind`

### Re-exports

#### Re-export `Sink`

```rust
pub use sink::Sink;
```

#### Re-export `Source`

```rust
pub use source::Source;
```

## Module `arithmetic`

Compensated summation, translated from `CycArithmetic`.

Cyclus sums composition vectors constantly — every normalisation, every
mass balance, every `Material::Absorb`. Over a long simulation a naive
`fold(0.0, Add::add)` accumulates enough rounding error to make an
inventory visibly drift, so upstream sums with Kahan compensation after
sorting ascending. Both halves matter and both are preserved here.

```rust
pub mod arithmetic { /* ... */ }
```

### Functions

#### Function `kahan_sum`

**Attributes:**

- `MustUse { reason: None }`

Kahan-compensated sum of `values`, ascending-sorted first.

# Method

Translated line-for-line from `CycArithmetic::KahanSum`, which itself cites
the Wikipedia description of the algorithm. Two things happen, in order:

 1. **Sort ascending.** Adding the smallest magnitudes first keeps the
    running sum as small as possible for as long as possible, so fewer
    low-order bits of each addend fall off the end. Upstream does this with
    `std::sort` on a copy.
 2. **Kahan compensation.** A running compensation term `c` recovers the
    low-order part lost from each addend and feeds it back into the next.

# A deliberate difference from upstream

Upstream sorts with `std::sort`, whose comparator is `operator<`. That is
undefined behaviour in the presence of NaN, because `<` is not a strict
weak ordering over floats. Here the sort uses [`f64::total_cmp`], which is
a total order over every `f64` including NaN and signed zeros. On NaN-free
input — which is every input upstream intends — the two orderings agree
exactly, so this is strictly a hardening, not a behavioural change.

# Examples

```
use kaki_bukit::arithmetic::kahan_sum;

// A large value followed by many small ones: naive summation loses them.
let mut v = vec![1.0e16];
v.extend(core::iter::repeat(1.0).take(10));
assert_eq!(kahan_sum(&v), 1.0e16 + 10.0);
```

```rust
pub fn kahan_sum(values: &[f64]) -> f64 { /* ... */ }
```

#### Function `sort_ascending`

**Attributes:**

- `MustUse { reason: None }`

Returns `values` sorted ascending. Upstream `CycArithmetic::sort_ascending`.

Exposed because upstream exposes it, and because a caller that sums the
same vector repeatedly can sort once and call [`kahan_sum_sorted`].

```rust
pub fn sort_ascending(values: &[f64]) -> alloc::vec::Vec<f64> { /* ... */ }
```

#### Function `kahan_sum_sorted`

**Attributes:**

- `MustUse { reason: None }`

Kahan-compensated sum of already-ascending-sorted `values`.

This is [`kahan_sum`] without the sort and without the copy it needs. Use
it only when `values` is genuinely sorted ascending; passing unsorted input
is not unsound, but it gives up the error-reduction that the sort provides
and the result will differ from [`kahan_sum`] in the low-order bits.

```rust
pub fn kahan_sum_sorted(values: &[f64]) -> f64 { /* ... */ }
```

## Module `comp_math`

Component-wise arithmetic on composition vectors, translated from
`namespace cyclus::compmath`.

A [`CompMap`] is a map from nuclide to a **dimensionless** quantity. It is
deliberately not normalised: upstream's own header says "In general
CompMaps are not assumed to be normalized to any particular value", and
most of the resource layer depends on that — [`normalize`] to a mass is how
a composition and a quantity get combined before they are added.

Whether the quantity is an atom fraction or a mass fraction is **not**
recorded in the map. It is carried by the caller, and
[`Composition`](crate::composition::Composition) is the type that keeps the
two straight. Every function here is basis-agnostic.

```rust
pub mod comp_math { /* ... */ }
```

### Types

#### Type Alias `CompMap`

A raw map from nuclide to dimensionless quantity. Upstream `CompMap`.

`BTreeMap` rather than a hash map, and that choice is load-bearing: it
reproduces `std::map`'s ordered iteration, so a composition sums, prints
and records in nuclide order on every run and every platform. A hash map
would make [`sum`]'s result depend on iteration order in the low-order
bits, which is exactly the reproducibility this crate is built to preserve.

```rust
pub type CompMap = alloc::collections::BTreeMap<crate::nuclide::Nuc, f64>;
```

### Functions

#### Function `add`

**Attributes:**

- `MustUse { reason: None }`

Component-wise addition. Upstream `compmath::Add`.

No normalisation is performed. Nuclides present in only one operand are
carried through unchanged.

# Examples

```
use kaki_bukit::comp_math::{add, CompMap};
use kaki_bukit::nuclide::nuc;

let mut v1 = CompMap::new();
v1.insert(nuc::U235, 2.3);
v1.insert(nuc::U238, 1.3);
let mut v2 = CompMap::new();
v2.insert(nuc::U235, 1.1);
v2.insert(nuc::U238, 1.2);

let v3 = add(&v1, &v2);
assert!((v3[&nuc::U235] - 3.4).abs() < 1e-12);
assert!((v3[&nuc::U238] - 2.5).abs() < 1e-12);
```

```rust
pub fn add(v1: &CompMap, v2: &CompMap) -> CompMap { /* ... */ }
```

#### Function `sub`

**Attributes:**

- `MustUse { reason: None }`

Component-wise subtraction, `v1 - v2`. Upstream `compmath::Sub`.

No normalisation is performed, and **the result may contain negative
entries**. That is upstream behaviour and callers rely on it:
[`Material::extract_comp`](crate::material::Material::extract_comp)
subtracts and then applies a threshold, so a small negative left by
rounding is cleared by [`apply_threshold`] rather than rejected here.

```rust
pub fn sub(v1: &CompMap, v2: &CompMap) -> CompMap { /* ... */ }
```

#### Function `sum`

**Attributes:**

- `MustUse { reason: None }`

Sum of every quantity, without normalisation. Upstream `compmath::Sum`.

Uses [`kahan_sum`](crate::arithmetic::kahan_sum) — sorted, compensated —
exactly as upstream does. This is the single most-called numerical routine
in the crate, and the compensation is why a long simulation's mass balance
stays closed.

```rust
pub fn sum(v: &CompMap) -> f64 { /* ... */ }
```

#### Function `apply_threshold`

Zeroes — by removing — every entry whose magnitude is at or below
`threshold`. Upstream `compmath::ApplyThreshold`.

Upstream erases the entry rather than setting it to zero, and this does
too: a composition carrying a thousand nuclides at `1e-30` costs real time
in every later sum, and the distinction between "absent" and "present at
zero" is not one the resource layer makes.

# Errors

[`CyclusError::Value`] if `threshold` is negative, matching upstream's
`ValueError`.

```rust
pub fn apply_threshold(v: &mut CompMap, threshold: f64) -> crate::error::Result<()> { /* ... */ }
```

#### Function `normalize`

Scales every quantity so the total is `val`. Upstream `compmath::Normalize`.

A no-op when the sum already equals `val`, and — importantly — also a no-op
when the sum is zero, since there is no scale factor that makes an empty
composition total `val`. Upstream guards the same way, and silently: an
all-zero composition is a legitimate state for a fully depleted buffer.

```rust
pub fn normalize(v: &mut CompMap, val: f64) { /* ... */ }
```

#### Function `valid_nucs`

**Attributes:**

- `MustUse { reason: None }`

`true` if every key is a valid nuclide. Upstream `compmath::ValidNucs`.

```rust
pub fn valid_nucs(v: &CompMap) -> bool { /* ... */ }
```

#### Function `first_invalid_nuc`

**Attributes:**

- `MustUse { reason: None }`

The first invalid nuclide key, if any.

Not an upstream function. `valid_nucs` answering only yes/no means a caller
that wants to *report* the problem has to re-scan for it, and every caller
in this crate does want to report it. Returning the offending key here is
the "human interface layer" rule applied to an error path.

```rust
pub fn first_invalid_nuc(v: &CompMap) -> Option<crate::nuclide::Nuc> { /* ... */ }
```

#### Function `all_positive`

**Attributes:**

- `MustUse { reason: None }`

`true` if no quantity is negative. Upstream `compmath::AllPositive`.

Note the name is upstream's and is mildly misleading in both languages:
the test is `>= 0`, so an all-zero composition passes.

```rust
pub fn all_positive(v: &CompMap) -> bool { /* ... */ }
```

#### Function `almost_eq`

`true` if `v1` and `v2` agree to within a **relative** `threshold`.
Upstream `compmath::AlmostEq`.

# Method

Upstream cites a note on floating-point comparison and implements
"almost equal if `|x-y| < |x|*eps` and `|x-y| < |y|*eps`" — a relative
test, not an absolute one, so it behaves sensibly across the many orders of
magnitude a composition spans. Two structural conditions come first: the
maps must be the same size, and every key of `v1` must be present in `v2`.

# A preserved upstream quirk

When either quantity is exactly zero, upstream falls into a branch that
tests `|diff| > |diff| * threshold`. For `threshold < 1` and a non-zero
`diff` that is always true, so the comparison reports *not equal* — meaning
a nuclide present at zero in one map and non-zero in the other never
compares equal regardless of how small the difference is. That is
reproduced here rather than "fixed", because a caller's tolerance may have
been tuned against it and silently changing a comparison is how a
regression gets shipped. It is called out in
`docs/port-notes.md` as a candidate defect to raise upstream.

# Errors

[`CyclusError::Value`] if `threshold` is negative.

```rust
pub fn almost_eq(v1: &CompMap, v2: &CompMap, threshold: f64) -> crate::error::Result<bool> { /* ... */ }
```

## Module `composition`

Nuclide compositions in both atom and mass bases.

A [`Composition`] is an immutable pair of [`CompMap`]s describing the same
material two ways — per atom and per unit mass. Neither is normalised: a
composition describes *ratios*, and the absolute scale lives on the
[`Material`](crate::material::Material) that carries it.

# Where the atomic masses come from

Upstream converts between the two bases lazily, calling
`pyne::atomic_mass(nuc)` against a table compiled into the binary. This
crate ships **no nuclear data** — the workspace rule is that all of it lives
in `njoy-outram-park-fork` — so the conversion takes an explicit
[`AtomicMasses`] argument instead, and the caller says where the masses come
from. [`AtomicMasses::MassNumber`] is the zero-data option, good to a few
tenths of a percent, and is what the tests here use.

# Three divergences from upstream, all deliberate

1. **Both bases are computed eagerly**, at construction. Upstream computes
   the second basis on first access and caches it by mutating the object
   through a non-`const` accessor. Doing that here would need interior
   mutability, which in `no_std` means a lock or a `Cell`, for no benefit:
   the conversion is one multiply per nuclide and every composition in a
   running simulation gets asked for both bases sooner or later.

2. **There is no `id()` and no global counter.** Upstream's `next_id_` is a
   mutable process global, which `no_std` has no good way to provide and
   which makes two runs of the same input disagree on ids if anything is
   ever constructed in a different order. Identity is handled by
   [`Context`](crate::context::Context), which interns compositions and
   hands out a [`CompId`]; a composition that has never been recorded
   simply has no id.

3. **There is no cached decay chain.** Upstream threads a shared
   `decay_line_` map through every composition descended from a common
   ancestor, memoising decay results. That is a `shared_ptr` graph of
   exactly the kind this workspace's rules exclude, and the memo is a cache,
   not semantics. [`decay`](crate::decay) recomputes; a caller who wants the
   memo can hold one.

```rust
pub mod composition { /* ... */ }
```

### Types

#### Struct `CompId`

An identifier for a composition that has been recorded by a
[`Context`](crate::context::Context).

Corresponds to upstream's `Composition::id()` / the `QualId` column in the
output database. Two compositions built separately from the same
[`CompMap`] get different ids, exactly as upstream documents.

```rust
pub struct CompId(pub u64);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u64` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CompId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &CompId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CompId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &CompId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `AtomicMasses`

A source of atomic masses, in unified atomic mass units (u).

An enum rather than a trait, per the workspace's no-trait-objects rule, and
the closed set is genuine: either you have a table of measured masses or
you are approximating by mass number. There is no third case.

```rust
pub enum AtomicMasses {
    MassNumber,
    Table(alloc::collections::BTreeMap<crate::nuclide::Nuc, f64>),
}
```

##### Variants

###### `MassNumber`

Approximate each nuclide's atomic mass by its mass number `A`.

Carries no data, which is why it is the default in tests and examples.
The error is the mass defect — under 1 % everywhere and under 0.1 % for
the actinides that dominate a fuel cycle — so it is fine for a
structural test and **not** fine for a mass balance anyone will quote.
Use [`AtomicMasses::Table`] with evaluated masses for that.

A natural-element id (`A == 0`) has no mass number to use, so
[`AtomicMasses::mass_of`] reports
[`CyclusError::InvalidNuclide`] for one.

###### `Table`

Explicit atomic masses in u, keyed by nuclide.

Populate this from `njoy-outram-park-fork`, or from any evaluated
nuclear data library. A nuclide absent from the table is an error, not
a silent zero.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::collections::BTreeMap<crate::nuclide::Nuc, f64>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn from_pairs(pairs: &[(Nuc, f64)]) -> Self { /* ... */ }
  ```
  Builds a table from `(nuclide, mass in u)` pairs.

- ```rust
  pub fn mass_of(self: &Self, nuc: Nuc) -> Result<f64> { /* ... */ }
  ```
  The atomic mass of `nuc`, in u.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AtomicMasses { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AtomicMasses) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Composition`

An immutable nuclide composition, held in both atom and mass bases.

Built through [`Composition::from_atom`], [`Composition::from_mass`] or
[`Composition::from_nuclide`], each of which validates that every key is a
real nuclide and that no quantity is negative — the same two checks
upstream's factory functions make.

Neither basis is normalised. To read fractions rather than raw ratios, use
[`Composition::atom_frac`] and [`Composition::mass_frac`], or
[`MatQuery`](crate::toolkit::mat_query::MatQuery) for quantities on a
specific material.

```rust
pub struct Composition {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn from_atom(v: CompMap, masses: &AtomicMasses) -> Result<Self> { /* ... */ }
  ```
  Creates a composition whose components are in atom ratios.

- ```rust
  pub fn from_mass(v: CompMap, masses: &AtomicMasses) -> Result<Self> { /* ... */ }
  ```
  Creates a composition whose components are in mass ratios.

- ```rust
  pub fn from_nuclide(nuc: Nuc, masses: &AtomicMasses) -> Result<Self> { /* ... */ }
  ```
  Creates a composition of one pure nuclide. Upstream

- ```rust
  pub fn atom(self: &Self) -> &CompMap { /* ... */ }
  ```
  The unnormalised atom-basis composition. Upstream `Composition::atom()`.

- ```rust
  pub fn mass(self: &Self) -> &CompMap { /* ... */ }
  ```
  The unnormalised mass-basis composition. Upstream `Composition::mass()`.

- ```rust
  pub fn atom_frac(self: &Self, nuc: Nuc) -> f64 { /* ... */ }
  ```
  The atom fraction of `nuc`: its atom quantity over the total.

- ```rust
  pub fn mass_frac(self: &Self, nuc: Nuc) -> f64 { /* ... */ }
  ```
  The mass fraction of `nuc`: its mass quantity over the total.

- ```rust
  pub fn normalized_atom(self: &Self) -> CompMap { /* ... */ }
  ```
  The atom-basis composition normalised to sum to 1.

- ```rust
  pub fn normalized_mass(self: &Self) -> CompMap { /* ... */ }
  ```
  The mass-basis composition normalised to sum to 1.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` if the composition holds no nuclides.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  The number of nuclides tracked.

- ```rust
  pub fn almost_eq(self: &Self, other: &Self, threshold: f64) -> Result<bool> { /* ... */ }
  ```
  `true` if this and `other` agree in the mass basis to within a relative

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Composition { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Composition) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `context`

Simulation-global services: the clock, the recipe registry, and the
build/decommission schedule.

# What upstream's `Context` does, and how it is split here

Upstream's `Context` is the god object of a Cyclus simulation. It owns the
clock, the recipe registry, the prototype registry, the agent list, the
output recorder, the random number generator, the package and
transport-unit registries, and the build/decommission queues, and every
agent holds a `Context*` back-pointer to reach all of it.

That shape does not translate. A back-pointer from every agent to a mutable
object that also *owns* those agents is the exact aliasing Rust exists to
prevent, and working around it would mean `Rc<RefCell<…>>` threaded through
the whole crate.

So the responsibilities are split in two, along the line that makes each
half independently testable:

| | owns | this crate |
|---|---|---|
| [`Context`] | clock, recipes, build/decom schedule | this module |
| `Simulation` | the agent arena and the phase loop | [`agents`](crate::agents) |

An agent is then *handed* `&Context` for the duration of a phase call
rather than holding a pointer to it. That is a real interface change and it
is stated here rather than hidden: an agent cannot mutate the simulation
from inside `Tick`, it returns what it wants and the driver applies it.

# Not carried over

The output recorder, the prototype registry (Rust resolves archetypes at
compile time through an enum), and the `Package`/`TransportUnit`
registries. Packaging is a recent upstream feature that discretises a trade
into shippable units; it is a genuine feature gap here, noted in
[`agents`](crate::agents) rather than silently skipped.

```rust
pub mod context { /* ... */ }
```

### Types

#### Struct `ScheduledBuild`

A scheduled build: which prototype, under which parent, at which time step.
Upstream `Timer::build_queue_`.

```rust
pub struct ScheduledBuild {
    pub parent: crate::agent::AgentId,
    pub prototype: alloc::string::String,
    pub time: i64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `parent` | `crate::agent::AgentId` | The institution the new agent will sit under. |
| `prototype` | `alloc::string::String` | The prototype name to build, as registered in the input deck. |
| `time` | `i64` | The time step at which to build it. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ScheduledBuild { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ScheduledBuild) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ScheduledDecom`

A scheduled decommissioning. Upstream `Timer::decom_queue_`.

```rust
pub struct ScheduledDecom {
    pub agent: crate::agent::AgentId,
    pub time: i64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `agent` | `crate::agent::AgentId` | The agent to decommission. |
| `time` | `i64` | The time step at which to decommission it. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ScheduledDecom { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ScheduledDecom) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Context`

Simulation-global state that agents read and schedule against.

Holds the clock, the named recipes, and the pending build and
decommission schedules. It deliberately does **not** hold the agents — see
this module's documentation for why.

```rust
pub struct Context {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(info: SimInfo) -> Result<Self> { /* ... */ }
  ```
  Creates a context for the given simulation configuration.

- ```rust
  pub fn time(self: &Self) -> i64 { /* ... */ }
  ```
  The current time step. Upstream `Context::time()`.

- ```rust
  pub fn sim_info(self: &Self) -> &SimInfo { /* ... */ }
  ```
  The simulation configuration. Upstream `Context::sim_info()`.

- ```rust
  pub fn timer(self: &Self) -> &Timer { /* ... */ }
  ```
  The clock.

- ```rust
  pub fn timer_mut(self: &mut Self) -> &mut Timer { /* ... */ }
  ```
  The clock, mutably — for the driver that advances it.

- ```rust
  pub fn add_recipe(self: &mut Self, name: &str, c: Composition) -> Result<()> { /* ... */ }
  ```
  Registers a named recipe. Upstream `Context::AddRecipe`.

- ```rust
  pub fn replace_recipe(self: &mut Self, name: &str, c: Composition) -> Option<Composition> { /* ... */ }
  ```
  Registers a named recipe, replacing any existing one of that name.

- ```rust
  pub fn recipe(self: &Self, name: &str) -> Result<&Composition> { /* ... */ }
  ```
  Looks up a named recipe. Upstream `Context::GetRecipe`.

- ```rust
  pub fn has_recipe(self: &Self, name: &str) -> bool { /* ... */ }
  ```
  `true` if a recipe of that name is registered.

- ```rust
  pub fn recipe_names(self: &Self) -> Vec<&str> { /* ... */ }
  ```
  The registered recipe names, in sorted order.

- ```rust
  pub fn sched_build(self: &mut Self, parent: AgentId, prototype: &str, t: i64) -> Result<()> { /* ... */ }
  ```
  Schedules a prototype to be built under `parent` at time step `t`.

- ```rust
  pub fn sched_decom(self: &mut Self, agent: AgentId, t: i64) { /* ... */ }
  ```
  Schedules `agent` to be decommissioned at time step `t`. Upstream

- ```rust
  pub fn take_builds_due(self: &mut Self, t: i64) -> Vec<ScheduledBuild> { /* ... */ }
  ```
  Removes and returns every build scheduled for time step `t`, in the

- ```rust
  pub fn take_decoms_due(self: &mut Self, t: i64) -> Vec<ScheduledDecom> { /* ... */ }
  ```
  Removes and returns every decommissioning scheduled at or before time

- ```rust
  pub fn build_queue(self: &Self) -> &[ScheduledBuild] { /* ... */ }
  ```
  The pending build schedule.

- ```rust
  pub fn decom_queue(self: &Self) -> &[ScheduledDecom] { /* ... */ }
  ```
  The pending decommission schedule.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Context { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Context) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `decay`

Radioactive decay: the Bateman equations, solved by matrix exponential.

# ⚠️ This is upstream's DEPRECATED solver, not its live one

Read this before comparing anything here against a Cyclus run.

This module translates `cyclus/src/uniform_taylor.cc` faithfully. But
upstream calls `UniformTaylor::MatrixExpSolver` from exactly one site —
`decayer.cc:159` — and `Decayer` is marked DEPRECATED in its own header and
**is never instantiated anywhere in Cyclus**. (`decayer.h` is `#include`d
by `composition.cc` and `material.cc`, which is what makes it look live.)

Upstream's **actual** decay path is CRAM: `Composition::NewDecay` builds a
sparse decay matrix from `pyne_cram_transmute_info` and calls
`pyne_cram_expm_multiply14`, a 14th-order Chebyshev Rational
Approximation.

So **this module will not reproduce a modern Cyclus decay result.** It is a
correct matrix-exponential solver — verified against the closed-form
two-species Bateman solution to 7.7e-11 at tight tolerance — but it is a
different algorithm from the one upstream actually runs.

Two further consequences, both measured:

* **The truncation error is one-signed, so it accumulates.** Every
  discarded term of the truncated Poisson series is non-negative. At
  upstream's default `tol = 1e-3` that is a **0.095 % atom loss over one
  year** of the Sr-90 chain, and an isobaric chain that conserves mass by
  construction **loses 0.093 % of it**. Tightening to `1e-12` brings the
  residual to ~1e-13 for 172 series terms against 127, since the term count
  grows only logarithmically in the tolerance. Prefer the
  `*_with_tol` variants for anything whose mass balance matters; upstream
  exposes no way to ask for this.
* **`Decayer::BuildDecayMatrix` carries a real defect** which this port
  deliberately does not reproduce — see [`build_decay_matrix`]. In short, a
  "gross heuristic for mostly stable nuclides" in fact freezes every
  nuclide with a half-life under ~31.3 days as stable.

**Adding a CRAM backend is tracked as `op-i68k`**, and
`crates/outram-park-fork-onix/src/cram.rs` already has `cram16()` — reuse
it rather than writing a third matrix-exponential routine. Uniform Taylor
should stay alongside as an independent check.


# The physics in one paragraph

A decay chain is a linear, constant-coefficient system. Write `n(t)` for
the column vector of **atom counts** (any consistent unit — atoms, moles,
or the `kg/u` this module uses internally; decay is linear, so the scale
cancels). Then

```text
dn/dt = A n,      n(t) = exp(A t) n(0)
```

where `A` is the **decay matrix**, in units of per second:

```text
A[j][j] = -lambda_j                      (nuclide j decaying away)
A[i][j] = branch_ratio(j -> i) * lambda_j  (nuclide j feeding daughter i)
```

Note the convention, because getting it backwards is the single likeliest
defect in this file and is exactly what the Bateman test below exists to
catch: **the parent owns a column, the daughter owns a row.** Column sums
are `-lambda_j + lambda_j * (sum of branching ratios)`, which is zero when
the branching ratios sum to one — that is atom conservation, written as a
property of the matrix.

# The solver

[`UniformTaylor`] is a direct port of upstream's `uniform_taylor.cc`: the
Taylor series with **uniformization** (also called Jensen's method or
randomization). Set `alpha = max |A[i][i]|` and `B = A + alpha I`. Then

```text
exp(A t) = exp(-alpha t) exp(B t) = exp(-alpha t) sum_k (t^k / k!) B^k
```

Every entry of `B` is non-negative for a physical decay matrix (the
diagonal becomes `alpha - lambda_j >= 0`, the off-diagonals are already
`>= 0`), so every term of the series is non-negative and **nothing
cancels**. That is the whole point of the transformation: a naive Taylor
series for `exp(A t)` with `A` having large negative diagonal entries
suffers catastrophic cancellation, and this one does not. The truncation
point is chosen by [`UniformTaylor::max_num_terms`], which keeps the
discarded Poisson tail below `tol`.

# The practical limit on one step

`alpha = max lambda` is set by the **shortest-lived** tracked nuclide, and
`exp(-alpha t)` must not underflow `f64`. That caps a single call at
`alpha t < ~709`, i.e. about **1023 half-lives of the shortest-lived
nuclide in the chain**. For a chain containing Y-90 (`T_1/2 = 64.05 h`)
that is roughly 7.5 years in one step. Beyond it the solver returns
[`CyclusError::Value`] rather than silently returning zeros. Upstream's
`long double` reaches about 16384 half-lives instead; see
[`UniformTaylor::solve`]'s deviation notes, and
[`build_decay_matrix`]'s note on the "gross heuristic" upstream uses to
dodge the same problem incorrectly.

# Units, everywhere

| Quantity | Unit |
|---|---|
| decay constant `lambda` | per second (s^-1) |
| half-life | seconds |
| elapsed time `secs` / `t` | seconds |
| branching ratio | dimensionless, in `[0, 1]` |
| [`DecayChain`] matrix entries | per second (s^-1) |
| vectors passed to [`UniformTaylor::solve`] | **atom counts**, any consistent scale |
| [`Material`] quantity | kilograms |

# This module ships NO nuclear data — and that is deliberate

Upstream's `Decayer` carries a compiled-in decay-data table (via PyNE) and
reaches into it with `pyne::decay_const` and `pyne::decay_children`. This
crate does not, for two reasons:

 1. The workspace rule is that **all nuclear data belongs in
    `njoy-outram-park-fork`**; a transport or fuel-cycle crate is data-free
    and pulls what it needs from there.
 2. Upstream marks `Decayer` DEPRECATED in favour of `pyne::decayers::decay`
    anyway, so its data path is not the one to preserve. The *algorithm* is.

So the caller supplies a [`DecayChain`]: a map from parent nuclide to its
[`DecayData`]. A nuclide **absent** from the chain, or present with
`decay_constant == 0.0`, is **stable**.

# Verification & validation

**Methodology.** Three independent references, all closed-form; no
published benchmark is involved and none is claimed.

1. *Single nuclide, no daughters.* Reference `N(t) = N0 exp(-lambda t)`.
   This case is special: `alpha = lambda` makes `B` the zero matrix, so the
   series terminates after its first term and the answer is
   `exp(-lambda t) N0` **exactly**, up to [`petir::real::exp`]'s own error.
   Tolerance 1e-12 relative. Nuclide Sr-90, `lambda = 7.632e-10 s^-1`,
   `t = 1 year`.
2. *Two-nuclide chain against the analytic Bateman solution.* Reference,
   derived in the test itself:
   `N2(t) = N1(0) lambda_1 / (lambda_2 - lambda_1) (exp(-lambda_1 t) - exp(-lambda_2 t))`.
   Chain Sr-90 -> Y-90 -> Zr-90 (stable). This is the test that catches a
   transposed decay matrix, and it does: with the matrix transposed the
   daughter is fed by nothing and comes out at **exactly zero** (measured
   0.000e0 of its true amount). `transposed_decay_matrix_fails_bateman`
   pins that, so the sign convention cannot be "fixed" the wrong way and
   still leave the suite green.
3. *Diagonal matrix against elementwise `exp`.* Exercises
   [`UniformTaylor::solve`] on its own with `alpha t = 2`.

**Results, measured 2026-09-16** on this crate's own test suite
(`cargo test -p kaki-bukit --release --lib decay`), reported
as relative error against the closed form. Every row is a real number
printed by `decay::tests::vv_report`, which regenerates the whole table:

| Case | at `tol = 1e-3` (the default) | at a tight `tol` |
|---|---|---|
| Sr-90 single nuclide, 1 a | 0.000e0 (bit-exact) | — |
| Sr-90 -> Y-90 Bateman **daughter**, 1 a | 9.431e-4 | 7.737e-11 (`tol` 1e-10) |
| Sr-90 -> Y-90 Bateman **parent**, 1 a | 9.431e-4 | 7.737e-11 (`tol` 1e-10) |
| diagonal `exp`, `alpha t = 2` | — | 1.128e-11 (`tol` 1e-10) |
| atom conservation, 3-nuclide chain, 1 a | 9.515e-4 | 7.446e-13 (`tol` 1e-12) |
| atom conservation, 3-nuclide chain, 5 a | 9.331e-4 | 9.731e-13 (`tol` 1e-12) |
| half-life: fraction left after one `T_1/2` | 0.000e0 (bit-exact) | — |
| 1 kg Po-210 -> Pb-206 mass ratio, 1 a | 6.216e-4 | 1.398e-13 (`tol` 1e-12) |
| isobaric chain mass, 5 a | 9.331e-4 | 9.688e-13 (`tol` 1e-12) |

**Interpretation, and the one thing to take away.** The solver is correct
to round-off; **upstream's default tolerance is not tight enough for a
mass balance.** At `tol = 1e-3` the truncated Poisson tail discards up to
that fraction of the atom inventory — measured 9.5e-4, i.e. 0.095 %, over
one year of the Sr-90 chain — and the loss is **one-signed**, because every
discarded term is non-negative. So it accumulates over repeated steps
rather than cancelling. Pass a tighter `tol` through
[`decay_composition_with_tol`] or [`decay_material_with_tol`] for anything
that has to balance; the cost is mild, since the series length grows only
logarithmically in the tolerance (127 terms at 1e-3 against 172 at 1e-12,
for `alpha t = 94.9`). Upstream offers no way to ask for this at all.

What the tight-tolerance column establishes is that **the matrix build and
the solver are correct**. It says nothing about whether any particular
decay data is right, because this module holds none.

**Not covered.** No comparison against a published decay benchmark, no
comparison against upstream Cyclus compiled and run, no branching-ratio
chain with more than one branch verified against a reference. Nothing here
is validated in the workspace's sense.

```rust
pub mod decay { /* ... */ }
```

### Types

#### Struct `DecayData`

One parent nuclide's decay data: how fast it decays and into what.

# Units and valid ranges

- `decay_constant` is in **per second** (s^-1) and must be finite and
  `>= 0`. Zero means stable.
- each branching ratio is **dimensionless** and must be finite and in
  `[0, 1]`; the ratios for one parent must sum to at most `1 + tol`
  (see [`DecayChain::validate`]).

# Branching ratios that sum to less than one

This is permitted and is *not* an error. It means atoms leave the tracked
system — either because the caller deliberately truncated the chain, or
because a minor branch's daughter was not worth tracking. The consequence
is that total atom count is **not** conserved, and that is the caller's
choice to make, not this module's to refuse.

```rust
pub struct DecayData {
    pub decay_constant: f64,
    pub branches: alloc::vec::Vec<(crate::nuclide::Nuc, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `decay_constant` | `f64` | Decay constant `lambda`, in per second (s^-1). Non-negative; `0.0`<br>means stable. |
| `branches` | `alloc::vec::Vec<(crate::nuclide::Nuc, f64)>` | `(daughter nuclide, branching ratio)` pairs. The branching ratio is<br>the dimensionless fraction of decays of this parent that produce that<br>daughter.<br><br>A daughter may legitimately be absent from the chain itself, in which<br>case it is a stable end point. |

##### Implementations

###### Methods

- ```rust
  pub fn new(decay_constant: f64, branches: Vec<(Nuc, f64)>) -> Self { /* ... */ }
  ```
  Decay data with an explicit decay constant (per second) and branch

- ```rust
  pub fn stable() -> Self { /* ... */ }
  ```
  A stable nuclide: `lambda = 0` per second, no daughters.

- ```rust
  pub fn single(decay_constant: f64, daughter: Nuc) -> Self { /* ... */ }
  ```
  A single decay path with branching ratio `1.0`.

- ```rust
  pub fn from_half_life(half_life_secs: f64, branches: Vec<(Nuc, f64)>) -> Result<Self> { /* ... */ }
  ```
  Decay data given a **half-life in seconds** rather than a decay

- ```rust
  pub fn is_stable(self: &Self) -> bool { /* ... */ }
  ```
  `true` if this nuclide does not decay (`lambda == 0` per second).

- ```rust
  pub fn half_life(self: &Self) -> f64 { /* ... */ }
  ```
  The half-life in **seconds**, `ln(2) / lambda`.

- ```rust
  pub fn branch_sum(self: &Self) -> f64 { /* ... */ }
  ```
  The sum of this parent's branching ratios, dimensionless.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> DecayData { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DecayData) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DecayChain`

The caller-supplied decay chain: parent nuclide to its [`DecayData`].

# What "stable" means here

A nuclide is stable if it is **absent** from the chain, or present with
`decay_constant == 0.0`. There is no third state and no implicit data
lookup — this crate has no decay-data table (see the module docs).

# Ordering

Backed by a `BTreeMap`, so iteration is in nuclide-id order on every run
and every platform. That is load-bearing: it fixes the row/column order of
the decay matrix, which fixes the floating-point summation order, which is
what makes two runs of the same input bit-identical.

```rust
pub struct DecayChain {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty chain: every nuclide is stable.

- ```rust
  pub fn insert(self: &mut Self, parent: Nuc, data: DecayData) -> Option<DecayData> { /* ... */ }
  ```
  Inserts (or replaces) one parent's decay data.

- ```rust
  pub fn with(self: Self, parent: Nuc, data: DecayData) -> Self { /* ... */ }
  ```
  Builder form of [`insert`](DecayChain::insert), for assembling a chain

- ```rust
  pub fn get(self: &Self, nuc: Nuc) -> Option<&DecayData> { /* ... */ }
  ```
  The decay data for `nuc`, or `None` if it is not a tracked parent

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  The number of parent nuclides in the chain.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` if the chain has no entries, so every nuclide is stable.

- ```rust
  pub fn parents(self: &Self) -> Vec<Nuc> { /* ... */ }
  ```
  Every parent nuclide in the chain, in nuclide-id order.

- ```rust
  pub fn decay_constant(self: &Self, nuc: Nuc) -> f64 { /* ... */ }
  ```
  The decay constant of `nuc`, in **per second** (s^-1).

- ```rust
  pub fn is_stable(self: &Self, nuc: Nuc) -> bool { /* ... */ }
  ```
  `true` if `nuc` does not decay: absent from the chain, or present with

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Checks the chain is physically well formed, with the default tolerance

- ```rust
  pub fn validate_with_tol(self: &Self, tol: f64) -> Result<()> { /* ... */ }
  ```
  Checks the chain is physically well formed.

- ```rust
  pub fn reachable_closure(self: &Self, seeds: &[Nuc]) -> Vec<Nuc> { /* ... */ }
  ```
  Every nuclide reachable from `seeds` by following decay branches, plus

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> DecayChain { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> DecayChain { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DecayChain) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `UniformTaylor`

The Taylor-series-with-uniformization matrix exponential solver.

A direct port of upstream's `UniformTaylor` class. It solves

```text
dx/dt = A x,   x(t) = exp(A t) x(0)
```

for a square `A` whose diagonal is non-positive and whose off-diagonal
entries are non-negative — that is, a decay matrix. It is exposed as a
public type rather than kept private to [`decay_composition`] because it is
independently useful (any linear first-order system with those signs) and,
more to the point, independently **testable**.

A unit-struct namespace rather than a value: it has no state, exactly as
upstream's all-static class has none.

```rust
pub struct UniformTaylor;
```

##### Implementations

###### Methods

- ```rust
  pub fn solve(a: &Matrix, x0: &[f64], t: f64, tol: f64) -> Result<Vec<f64>> { /* ... */ }
  ```
  Solves `x(t) = exp(A t) x0`.

- ```rust
  pub fn solve_default_tol(a: &Matrix, x0: &[f64], t: f64) -> Result<Vec<f64>> { /* ... */ }
  ```
  Solves with upstream's hard-coded tolerance, [`DEFAULT_TOL`].

- ```rust
  pub fn max_abs_diag(a: &Matrix) -> f64 { /* ... */ }
  ```
  The diagonal element of `a` with the largest absolute value.

- ```rust
  pub fn max_num_terms(alpha_t: f64, epsilon: f64) -> Result<usize> { /* ... */ }
  ```
  The number of Taylor terms needed for a truncation error of at most

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> UniformTaylor { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> UniformTaylor { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &UniformTaylor) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `build_decay_matrix`

Builds the decay matrix `A`, in **per second** (s^-1), over `nuclides`.

This is the translation of upstream's `Decayer::BuildDecayMatrix`. Row and
column `k` both correspond to `nuclides[k]`, so the caller controls the
ordering and can read the matrix back against its own list. The
convention, restated because a transposed matrix is the likeliest defect
in this file:

```text
A[j][j] = -lambda_j
A[i][j] = branch_ratio(j -> i) * lambda_j
```

i.e. **parent by column, daughter by row**, so that `dn/dt = A n`.

# Daughters outside `nuclides`

A daughter not present in `nuclides` is **dropped**: its atoms leave the
tracked system and the corresponding column sum becomes negative. That is
deliberate — it is what makes this function usable for a truncated
nuclide set — but it is silent, so prefer
[`DecayChain::reachable_closure`] to build the list.
[`decay_composition`] does exactly that and therefore never hits this case.

# Deviation from upstream: the "mostly stable" heuristic is NOT ported

`BuildDecayMatrix` contains this, commented *"Gross heuristic for mostly
stable nuclides 2903040000 sec / 100 years"*:

```text
if (static_cast<long double>(exp(-2903040000 * decay_const)) == 0.0)
  decay_const = 0.0;
```

Reading it was the surprise of this port, and it is recorded here because
it does the **opposite** of what its comment says. The test fires when
`exp(-2.9e9 * lambda)` underflows, i.e. when `lambda` is **large**
(`> ~2.6e-7` per second in `double`, a half-life under about 31 days) —
not when the nuclide is "mostly stable". Its effect is therefore to freeze
every short-lived nuclide as if it were stable, which is a large,
silent physics error rather than a rounding convenience. The plausible
intent is numerical: a large `lambda` makes `alpha t` huge, which makes
`exp(-alpha t)` underflow and the solver refuse. That is a real problem,
but zeroing the decay constant is not a correct answer to it, so it is not
ported. The port instead reports the underflow as
[`CyclusError::Value`], and the caller can either shorten the step or
exclude the nuclide knowingly.

# Errors

- [`CyclusError::Value`] if `nuclides` is empty, contains a duplicate, or
  if a decay constant or branching ratio reached through the chain is
  non-finite or negative, or if a parent lists itself as a daughter.
- [`CyclusError::Numeric`] if the matrix allocation fails.

# Examples

```
use kaki_bukit::decay::{build_decay_matrix, DecayChain, DecayData};
use kaki_bukit::nuclide::Nuc;

let a_nuc = Nuc::new(380900000); // Sr-90
let b_nuc = Nuc::new(390900000); // Y-90
let chain = DecayChain::new().with(a_nuc, DecayData::single(2.0, b_nuc));
let m = build_decay_matrix(&chain, &[a_nuc, b_nuc])?;

assert_eq!(m.get(0, 0), -2.0); // parent decays away
assert_eq!(m.get(1, 0), 2.0);  // ... into the daughter: row 1, column 0
assert_eq!(m.get(0, 1), 0.0);
assert_eq!(m.get(1, 1), 0.0);  // daughter is stable
# Ok::<(), kaki_bukit::error::CyclusError>(())
```

```rust
pub fn build_decay_matrix(chain: &DecayChain, nuclides: &[crate::nuclide::Nuc]) -> crate::error::Result<petir::linalg::Matrix> { /* ... */ }
```

#### Function `decay_composition`

Decays a composition for `secs` seconds, in the **atom basis**.

This is the translation of upstream's `Decayer` pipeline — build the
tracked-nuclide set, build the decay matrix, call the matrix exponential,
read the result back — with the decay data supplied by the caller rather
than compiled in.

# Why the atom basis

**Decay conserves atoms, not mass.** A decay event turns one atom into one
atom of the daughter (plus an emitted particle), so the natural state
variable is the atom count and the matrix above is written for it. Mass is
*not* conserved, for a reason that has nothing to do with the mass defect:
an alpha decay carries four nucleons out of the tracked heavy nuclide, and
unless the caller tracks He-4 as a daughter those four nucleons leave the
composition. Working in the mass basis would therefore need a
mass-per-decay bookkeeping term that the Bateman equations do not have.
So the solve is in atoms and the mass basis is reconstructed afterwards by
[`Composition::from_atom`], which is where `masses` is used.

# Scale

[`Composition`] is a set of ratios, not absolute amounts, and this function
preserves whatever scale the input carried: pass atom *fractions* and you
get the decayed atom fractions scaled by the same factor the absolute atom
count changed by. If you want absolute amounts, scale the input yourself —
or use [`decay_material`], which does exactly that against the material's
mass in kilograms.

# The nuclide set

The matrix is built over the union of the composition's own nuclides and
every nuclide reachable from them through `chain`
([`DecayChain::reachable_closure`]), so no atom silently leaves the system
because its daughter was not listed. That is upstream's recursive
`AddNucToMaps` behaviour.

# Deviation: no static accumulating state

Upstream's `Decayer` keeps `parent_`, `daughters_`, `decay_matrix_` and
`nuclides_tracked_` as **class statics**, so the tracked set grows for the
life of the process and the matrix is rebuilt only when a new nuclide is
seen. Every later decay then solves a system as large as every nuclide
ever seen. This port builds the matrix per call from the nuclides that are
actually present. The result is the same numbers (the extra rows carry
zero) with no cross-call coupling and no global mutable state, which
`no_std` could not express anyway.

# Zero-valued results are dropped

Upstream's `GetResult` copies a nuclide into the output only when its atom
count is `> 0`, and this does the same. The uniformized series produces
only non-negative terms, so an entry is zero exactly when nothing fed it.

# Errors

- [`CyclusError::Value`] for a negative or non-finite `secs`, or any of
  the solver's failure modes.
- [`CyclusError::InvalidNuclide`] / [`CyclusError::Key`] if `masses`
  cannot supply an atomic mass for a daughter that the decay produced.
  Note this can fail for a composition that was fine going in, because
  decay introduces nuclides the caller never mentioned.

# Examples

```
use kaki_bukit::comp_math::CompMap;
use kaki_bukit::composition::{AtomicMasses, Composition};
use kaki_bukit::decay::{
    decay_composition, decay_composition_with_tol, DecayChain, DecayData,
};
use kaki_bukit::nuclide::Nuc;

let po210 = Nuc::new(842100000);
let pb206 = Nuc::new(822060000);
// Po-210 alpha decays to Pb-206 with a 138.376 day half-life.
let chain = DecayChain::new().with(
    po210,
    DecayData::from_half_life(138.376 * 86400.0, vec![(pb206, 1.0)])?,
);

let mut v = CompMap::new();
v.insert(po210, 1.0);
let comp = Composition::from_atom(v, &AtomicMasses::MassNumber)?;

let year = 365.25 * 86400.0;
let after = decay_composition(&comp, &chain, year, &AtomicMasses::MassNumber)?;

// Atom count is conserved -- but only to the solver's tolerance, which
// defaults to upstream's 1e-3 and always errs by LOSING atoms.
let total: f64 = after.atom().values().sum();
assert!((total - 1.0).abs() < 1e-3);

// Ask for a tighter answer when the books have to balance.
let tight = decay_composition_with_tol(&comp, &chain, year, &AtomicMasses::MassNumber, 1e-12)?;
let total: f64 = tight.atom().values().sum();
assert!((total - 1.0).abs() < 1e-9);
# Ok::<(), kaki_bukit::error::CyclusError>(())
```

```rust
pub fn decay_composition(comp: &crate::composition::Composition, chain: &DecayChain, secs: f64, masses: &crate::composition::AtomicMasses) -> crate::error::Result<crate::composition::Composition> { /* ... */ }
```

#### Function `decay_composition_with_tol`

[`decay_composition`] with an explicit solver tolerance.

`tol` is dimensionless and must lie strictly within `(0, 1)`; see
[`UniformTaylor::solve`]. [`decay_composition`] is this function with
[`DEFAULT_TOL`], which is upstream's hard-coded value.

Not an upstream entry point — upstream offers no way to ask for a tighter
answer. It is here because the tolerance is the one knob that trades cost
against accuracy, and burying it would make the V&V cases in this file
impossible to write.

# Errors

As [`decay_composition`], plus [`CyclusError::Value`] for a `tol` outside
`(0, 1)`.

```rust
pub fn decay_composition_with_tol(comp: &crate::composition::Composition, chain: &DecayChain, secs: f64, masses: &crate::composition::AtomicMasses, tol: f64) -> crate::error::Result<crate::composition::Composition> { /* ... */ }
```

#### Function `decay_material`

Decays a material in place for `secs` seconds, recording `now` as its new
decay time.

# What is conserved, and what is not — read this

**Atom count is conserved through the solve; total mass is not, and must
not be.** The solve runs on absolute atom amounts obtained by dividing each
nuclide's mass in kilograms by its atomic mass in u, so — for a chain whose
branching ratios sum to one and whose end points are tracked — the total
number of atoms coming out equals the number going in. The material's
quantity in kilograms is then **rebuilt** from the decayed nuclide vector:

```text
new quantity [kg] = sum over nuclides of (atom amount * atomic mass)
```

That total differs from the old one, and the difference is physical, not a
bookkeeping error. An alpha decay removes four nucleons from the tracked
heavy nuclide; a beta decay barely changes the mass number but does change
which mass is used. Holding the kilograms fixed instead would mean
inventing atoms to make the books balance — for Po-210 decaying to Pb-206
over a year that would be a **1.6 % error** in atom count (measured: the
mass falls to 9.8400912e-1 of its initial value while the atom count is
unchanged), silently, in the direction of over-counting the inventory. So: **atoms through the solve,
mass rebuilt afterwards.**

The mass defect itself (the binding-energy difference, of order 1e-3 of the
mass number) is ignored, exactly as upstream ignores it: it lives entirely
in the atomic masses `masses` supplies, and with
[`AtomicMasses::MassNumber`] it is not represented at all.

# Parameters

- `mat` — the material, whose quantity is in kilograms.
- `chain` — the caller-supplied decay data; constants in per second.
- `secs` — elapsed time in **seconds**, finite and `>= 0`.
- `now` — the simulation time to record as the material's new decay time.
  Units are the simulation's own (upstream: integer time steps); this
  function only stores it.
- `masses` — atomic masses in u, needed for both the mass-to-atom
  conversion going in and the atom-to-mass conversion coming out.

# An empty material is a no-op

A material with zero quantity, or whose composition has zero total mass,
has its decay time updated and nothing else. There is nothing to decay and
the atom conversion would divide by zero.

# Errors

As [`decay_composition`]. On any error the material is left **unchanged**
except that its decay time is not advanced, because the new value is
computed in full before anything is written back.

```rust
pub fn decay_material(mat: &mut crate::material::Material, chain: &DecayChain, secs: f64, now: i64, masses: &crate::composition::AtomicMasses) -> crate::error::Result<()> { /* ... */ }
```

#### Function `decay_material_with_tol`

[`decay_material`] with an explicit solver tolerance.

`tol` is dimensionless and must lie strictly within `(0, 1)`; see
[`UniformTaylor::solve`]. [`decay_material`] is this function with
[`DEFAULT_TOL`], upstream's hard-coded value.

**Pass a tighter `tol` than the default for anything that has to balance.**
At `tol = 1e-3` the truncated series discards up to that fraction of the
atom inventory — measured at 9.331e-4 over a five-year decay of the
Sr-90 -> Y-90 -> Zr-90 chain, which showed up directly as a 0.093 % loss of
mass in a chain that conserves mass exactly. The loss is always in the
same direction (the discarded terms are all non-negative, so the solver
under-counts and never creates atoms), so it accumulates rather than
cancelling over repeated steps.

# Errors

As [`decay_material`], plus [`CyclusError::Value`] for a `tol` outside
`(0, 1)`.

```rust
pub fn decay_material_with_tol(mat: &mut crate::material::Material, chain: &DecayChain, secs: f64, now: i64, masses: &crate::composition::AtomicMasses, tol: f64) -> crate::error::Result<()> { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_TOL`

The truncation tolerance upstream hard-codes in
`UniformTaylor::MatrixExpSolver`: `tol = 1e-3`, dimensionless.

It bounds the discarded Poisson tail of the uniformized series, so the
solution's error is at most `DEFAULT_TOL` times the initial total atom
count. Upstream offers no way to change it; here it is both this constant
and the `tol` parameter of [`UniformTaylor::solve`], so a caller who wants
a tighter answer can ask for one.

```rust
pub const DEFAULT_TOL: f64 = 1e-3;
```

#### Constant `MAX_SERIES_TERMS`

Hard cap on the number of series terms, dimensionless.

**Not upstream.** Upstream's `MaxNumTerms` loop has no bound; in a
`no_std` library an unbounded loop that could stall on a pathological
input is worse than an error, so the cap turns that into
[`CyclusError::Value`]. It is set far above any reachable value: the
series length grows like `alpha t + O(sqrt(alpha t))`, and `alpha t` is
already bounded below ~709 by the `exp` range checks the port inherits,
so a real call needs fewer than a thousand terms.

```rust
pub const MAX_SERIES_TERMS: usize = 1_000_000;
```

## Module `error`

Error taxonomy, translated from Cyclus's exception hierarchy.

# Why an enum and not an exception hierarchy

Upstream defines `cyclus::Error` and derives `ValueError`, `KeyError`,
`StateError`, `IOError` and `ValidationError` from it, then throws them.
Rust has no exceptions and this workspace forbids trait objects, so the
hierarchy collapses into one enum whose variants carry the same meanings.
The mapping is exact and one-to-one, so a reader can open `error.h` next to
this file:

| Upstream C++ | Here |
|---|---|
| `cyclus::Error` | [`CyclusError::Generic`] |
| `cyclus::ValueError` | [`CyclusError::Value`] |
| `cyclus::KeyError` | [`CyclusError::Key`] |
| `cyclus::StateError` | [`CyclusError::State`] |
| `cyclus::IOError` | [`CyclusError::Io`] |
| `cyclus::ValidationError` | [`CyclusError::Validation`] |

One variant has no upstream counterpart: [`CyclusError::Numeric`], which
carries a [`petir::PetirError`] out of a numerical kernel. Upstream has
nowhere for this to come from because its linear algebra throws
`cyclus::Error` directly; here the numerics live in a separate crate with
its own error type, and swallowing it would lose the diagnosis.

```rust
pub mod error { /* ... */ }
```

### Types

#### Type Alias `Result`

The result type used throughout this crate.

```rust
pub type Result<T> = core::result::Result<T, CyclusError>;
```

#### Enum `CyclusError`

**Attributes:**

- `NonExhaustive`

Every way a Cyclus kernel operation can fail.

Each variant carries a `&'static str` context rather than a formatted
`String`. That is a deliberate `no_std` choice: an allocation on the error
path is the one allocation a constrained target can least afford, and the
variant plus the static context has in practice been enough to locate the
fault. Where a number genuinely identifies the fault — an invalid nuclide
id, say — the variant carries it as a field instead of formatting it.

```rust
pub enum CyclusError {
    Generic(&''static str),
    Value(&''static str),
    Key(&''static str),
    State(&''static str),
    Io(&''static str),
    Validation(&''static str),
    InvalidNuclide(i32),
    Numeric(petir::PetirError),
}
```

##### Variants

###### `Generic`

Upstream `cyclus::Error`: a generic failure with no better category.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&''static str` |  |

###### `Value`

Upstream `cyclus::ValueError`: a value was outside its allowed range.

This is by far the most common failure in the resource layer — a
negative quantity, an extraction larger than the inventory, an
enrichment assay outside `(0, 1)`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&''static str` |  |

###### `Key`

Upstream `cyclus::KeyError`: a lookup by name or id found nothing.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&''static str` |  |

###### `State`

Upstream `cyclus::StateError`: an object was used in a state that does
not permit the operation (an exchange node with no group, a facility
asked to trade before it entered the simulation).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&''static str` |  |

###### `Io`

Upstream `cyclus::IOError`. Retained for translation fidelity; nothing
in this `no_std` kernel performs I/O, so it is produced only by
downstream crates that add a persistence layer.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&''static str` |  |

###### `Validation`

Upstream `cyclus::ValidationError`: input failed schema validation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&''static str` |  |

###### `InvalidNuclide`

A nuclide id was not a well-formed `zzzaaammmm` identifier.

Carries the offending id, because the id *is* the diagnosis here and a
static string could not convey it.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `i32` |  |

###### `Numeric`

A numerical kernel in [`petir`] failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `petir::PetirError` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CyclusError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut fmt::Formatter<''_>) -> fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(e: PetirError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(r: Rejected) -> Self { /* ... */ }
    ```
    Discards the refused resources and keeps the reason, so that `?` works

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CyclusError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `exchange`

**Dynamic resource exchange** — how a Cyclus time step decides who sends
what to whom.

This is the idea that distinguishes Cyclus from every fuel-cycle code that
came before it, and it is worth reading before anything else in this crate.
If you understand this module you understand Cyclus.

# The problem

A fuel cycle is a set of facilities — mines, conversion plants, enrichment
plants, fabricators, reactors, reprocessing plants, repositories — that
pass material between one another. The obvious way to simulate that is to
wire them together: this mine feeds that conversion plant, which feeds that
enricher. Every fuel-cycle code before Cyclus did roughly this.

The trouble is that the interesting questions are exactly the ones that
break the wiring. What happens if a reprocessing plant is unavailable for
six months? If a new reactor type is introduced in 2045? If two utilities
compete for the same enrichment capacity? Answering any of those means
rewiring, and a model that must be rewired to ask a question is a model
that answers only the questions its author already had.

# The idea

Cyclus does not wire facilities together at all. **Every time step, it
holds a market.** The market is recomputed from scratch each step, so who
trades with whom is an *output* of the simulation rather than an input.

The market runs in five phases.

## 1. Requests

Every facility that needs something posts a [`Request`](request::Request):
a commodity name, a target [`Resource`](crate::resource::Resource) saying
how much is wanted (and, for a material, of what composition), and a
**preference** saying how keenly it is wanted. A reactor posts a request
for `"uox"`; a repository posts one for `"waste"`.

Requests are grouped into a [`RequestPortfolio`](portfolio::RequestPortfolio) —
one per facility per commodity family. A portfolio is the unit of *mutual*
satisfaction: putting two requests in one portfolio and declaring them
mutual (see
[`add_mutual_reqs`](portfolio::RequestPortfolio::add_mutual_reqs)) says
"either of these will do", which is how a reactor says it will take MOX or
UOX but does not need both.

## 2. Bids

Every facility that can supply something looks at the posted requests and
answers the ones it can fill with a [`Bid`](bid::Bid): *this* request, and
*this* resource in reply. A bid may offer less than was asked for, or more.
A bidder may answer many requests, and many bidders may answer one request.
Bids are grouped into a [`BidPortfolio`](portfolio::BidPortfolio), one per
bidding facility.

## 3. Constraints

Neither side can honour everything it posted. An enrichment plant that bids
on six requests cannot fill all six — it has a fixed monthly separative
work capacity, and a fixed natural-uranium inventory. So each portfolio
carries [`CapacityConstraint`](constraint::CapacityConstraint)s, which act
across *all* of its requests or bids at once.

A constraint is a **budget** plus an **exchange rate**. The budget is a
number: 100 SWU, 4 tonnes of throughput, 50 kg of inventory. The exchange
rate is a [`Converter`](constraint::Converter), which says how much of that
budget one unit of a particular resource would consume — trivially the
resource's own mass, but for an enricher, the separative work that
*specific* product would take. Dividing budget by rate turns "100 SWU"
into "therefore at most 43 kg of this product", which is a number the
solver can work with.

## 4. Solving

The portfolios are then **translated** (see [`translate`]) into an
[`ExchangeGraph`](graph::ExchangeGraph): a bipartite graph whose left-hand
nodes are requests, whose right-hand nodes are bids, and whose arcs are
possible trades. Everything resource-specific is stripped out in the
process — a node is a quantity, an exclusivity flag and a list of unit
capacities, nothing more — so the solver works on pure numbers and the same
solver handles materials and products alike.

The [`GreedySolver`](greedy::GreedySolver) then walks the graph: requests
in order of how keenly they want their bids, and within each request, bids
in order of preference, assigning as much flow as the constraints allow
until demand is met or supply runs out. It is greedy — nothing is ever
revisited — which is why the ordering done by the
[`GreedyPreconditioner`](preconditioner::GreedyPreconditioner) is as
important as the matching itself.

## 5. Trades

The solution comes back as a list of [`Match`](graph::Match)es on the
graph, which [`Translation::back_translate`](translate::Translation::back_translate)
turns into [`Trade`](trade::Trade)s: this request, this bid, this much.
The agents then actually move the material, and the time step ends.

# Preferences, in one paragraph

A preference is a strictly positive dimensionless number attached to a
(request, bid) pair; larger means more wanted. It does two jobs. It
**orders** the greedy walk, so a keenly preferred bid is offered the flow
first. And it **prices** the solution: the solver minimises
`sum(quantity / preference)`, so a preference behaves as the reciprocal of
a unit cost. Unfilled demand is charged a pseudo-cost constructed to exceed
any real arc's cost, which is what makes two solutions that fill different
amounts of demand comparable at all.

A requester sets the preference on its own request. A bidder may override
it — but only on the requester's behalf, by evaluating a cost function the
requester published. A bidder that simply raises its own preference is
rigging the market.

# Exclusive orders

Some things cannot be split. A reactor's fuel assembly is one assembly or
none; half of one is worthless. Either side may declare itself
**exclusive**, and an arc touching an exclusive node carries either its
whole quantity or nothing at all. This is the subtlest part of the
subsystem, it is arithmetically delicate, and it is documented at
[`exclusive_value`](graph::exclusive_value) and in the
[`greedy`] module doc.

# A worked example

```
use kaki_bukit::agent::AgentId;
use kaki_bukit::exchange::constraint::CapacityConstraint;
use kaki_bukit::exchange::greedy::GreedySolver;
use kaki_bukit::exchange::portfolio::{BidPortfolio, RequestPortfolio};
use kaki_bukit::exchange::request::{RequestId, DEFAULT_PREF};
use kaki_bukit::exchange::translate::ExchangeContext;
use kaki_bukit::product::Product;
use kaki_bukit::resource::Resource;

# fn main() -> Result<(), kaki_bukit::error::CyclusError> {
let power = |qty| Resource::from(Product::new(qty, "MWh").unwrap());

// A city wants 100 MWh.
let mut ctx = ExchangeContext::new();
let mut demand = RequestPortfolio::new();
demand.request(power(100.0), AgentId(1), "power", DEFAULT_PREF, false)?;
ctx.add_request_portfolio(demand);
let the_request = RequestId { portfolio: 0, index: 0 };

// A cheap plant bids the lot but can only deliver 40 MWh this step...
let mut cheap = BidPortfolio::new();
cheap.bid(the_request, power(100.0), AgentId(2), false)?;
cheap.add_constraint(CapacityConstraint::new(40.0)?);
ctx.add_bid_portfolio(cheap);

// ...and a peaker bids 80 MWh, less preferred.
let mut peaker = BidPortfolio::new();
peaker.add_bid(
    kaki_bukit::exchange::bid::Bid::new(
        the_request, power(80.0), AgentId(3), false,
    )?
    .with_preference(0.5)?,
)?;
ctx.add_bid_portfolio(peaker);

let mut xlate = ctx.translate()?;
let mut solver = GreedySolver::new();
solver.solve(&mut xlate.graph)?;

// The cheap plant supplies its 40 MWh first; the peaker covers the rest.
let trades = xlate.back_translate()?;
assert_eq!(trades.len(), 2);
assert_eq!(trades[0].amt, 40.0);
assert_eq!(trades[1].amt, 60.0);
assert_eq!(solver.unmatched(), 0.0);
# Ok(())
# }
```

# How this translation differs from upstream

The substitutions are uniform and are each argued for where they occur;
this table is the index.

| Upstream | Here | Where it is argued |
|---|---|---|
| `shared_ptr`/`weak_ptr` object graph | arena indices: [`NodeId`](graph::NodeId), [`ArcId`](graph::ArcId), [`GroupId`](graph::GroupId) | [`graph`] |
| `RequestGroup : ExchangeNodeGroup` | one struct with a [`GroupKind`](graph::GroupKind) tag | [`graph`] |
| `Converter<T>`, an abstract base class | [`Converter`](constraint::Converter), an enum | [`constraint`] |
| `Request<T>` / `Bid<T>` templates | non-generic, holding a [`Resource`](crate::resource::Resource) enum | [`request`] |
| `int agent_id` with `-1` for unset | `Option<AgentId>` | [`graph`] |
| bid preference as a quiet `NaN` sentinel | `Option<f64>` | [`bid`] |
| offer identity by pointer | an explicit [`shared offer tag`](bid::Bid::with_shared_offer) | [`bid`] |
| thrown exceptions | [`Result`](crate::error::Result) | [`crate::error`] |
| `ProgSolver` on Coin-OR/Cbc | **not ported** | [`greedy`] |

Two upstream behaviours are preserved *exactly* even though they look like
defects, because they are not: exact float equality against
[`UNLIMITED`](crate::limits::UNLIMITED) as an unbounded-capacity sentinel,
and ULP-distance comparison for exclusive orders. Both are explained in the
[`greedy`] module doc. Do not "fix" either.

```rust
pub mod exchange { /* ... */ }
```

### Modules

## Module `bid`

A bid: one supplier's answer to one request.

Like [`Request`](crate::exchange::request::Request), [`Bid`] is not generic
over the resource — it holds a [`Resource`](crate::resource::Resource). The
reasoning is written out in the
[`request`](crate::exchange::request) module doc.

```rust
pub mod bid { /* ... */ }
```

### Types

#### Struct `BidId`

Identifies one bid inside an
[`ExchangeContext`](crate::exchange::translate::ExchangeContext), as
(portfolio, index). The counterpart of
[`RequestId`](crate::exchange::request::RequestId).

```rust
pub struct BidId {
    pub portfolio: usize,
    pub index: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `portfolio` | `usize` | Index of the owning [`BidPortfolio`](crate::exchange::portfolio::BidPortfolio)<br>within the exchange context. |
| `index` | `usize` | Index of the bid within that portfolio. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BidId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &BidId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BidId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &BidId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Bid`

An offer of a resource in response to a specific request.

Upstream `cyclus::Bid<T>`.

```rust
pub struct Bid {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(request: RequestId, offer: Resource, bidder: AgentId, exclusive: bool) -> Result<Self> { /* ... */ }
  ```
  Creates a bid answering `request` with `offer`.

- ```rust
  pub fn with_preference(self: Self, preference: f64) -> Result<Self> { /* ... */ }
  ```
  Overrides the arc preference for this bid.

- ```rust
  pub fn with_shared_offer(self: Self, tag: u32) -> Self { /* ... */ }
  ```
  Tags this bid as offering the same physical resource as every other bid

- ```rust
  pub fn request(self: &Self) -> RequestId { /* ... */ }
  ```
  The request being answered.

- ```rust
  pub fn offer(self: &Self) -> &Resource { /* ... */ }
  ```
  The resource offered.

- ```rust
  pub fn quantity(self: &Self) -> f64 { /* ... */ }
  ```
  How much is offered, in the resource's units. Strictly positive.

- ```rust
  pub fn bidder(self: &Self) -> AgentId { /* ... */ }
  ```
  The bidding agent. Mandatory, for the same reason

- ```rust
  pub fn exclusive(self: &Self) -> bool { /* ... */ }
  ```
  Whether the offer must be taken whole or not at all.

- ```rust
  pub fn preference(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The bid's own preference, or `None` to inherit the requester's.

- ```rust
  pub fn shared_offer(self: &Self) -> Option<u32> { /* ... */ }
  ```
  The shared-offer tag set by [`with_shared_offer`](Self::with_shared_offer).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Bid { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Bid) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `constraint`

Capacity constraints: the limits an agent puts on what it can trade.

A constraint is a **capacity** (a budget) plus a **converter** (an exchange
rate). The capacity says how much of some limited thing the agent has; the
converter says how much of that thing one unit of a given resource would
consume. Dividing the first by the second is how the solver turns "I have
100 SWU of enrichment capacity this month" into "therefore I can supply at
most 43 kg of *this particular* enriched product".

# The converter is an enum, not a functor

Upstream's `Converter<T>` is an abstract base class with one pure virtual
`convert()`, held by `shared_ptr`, and subclassed once per conversion —
`TrivialConverter`, `QtyCoeffConverter`, and in CYCAMORE `SWUConverter`,
`NatUConverter`, `FissConverter`, `FillConverter`, `TopupConverter`. That
is a trait object, which this workspace forbids, so [`Converter`] is an
enum of the conversions that are actually used.

**Adding a novel conversion means adding a variant.** That sounds worse
than `dyn` and is in fact better here, for three reasons:

1. **The set is closed and small.** Seven subclasses exist across CYCLUS
   and CYCAMORE combined, and five of them are `coefficient * quantity` or
   a constant. A closed set is what an enum is for.
2. **A new variant is a compile error at every `match`**, which is exactly
   where a reviewer wants to be stopped. A new `dyn` subclass compiles
   silently and is discovered at run time, if at all.
3. **Equality becomes real.** Upstream's `operator==` on a converter is a
   `dynamic_cast` that returns `false` by default, so two constraints that
   are semantically identical usually compare unequal — and constraints
   live in a `std::set`. Here `#[derive(PartialEq)]` compares what the
   converter actually is.

The cost is real and should be stated: a downstream crate cannot add a
conversion without editing this enum. Given that a converter's result feeds
straight into a mass balance, having every conversion in the fuel cycle
visible in one place is the trade this crate wants.

```rust
pub mod constraint { /* ... */ }
```

### Types

#### Enum `Converter`

How much of a constrained quantity one resource consumes.

Upstream `cyclus::Converter<T>` and its subclasses; see the module doc for
why this is an enum.

```rust
pub enum Converter {
    Quantity,
    Scaled(f64),
    Constant(f64),
    RequestMassCoeff,
}
```

##### Variants

###### `Quantity`

The resource's own quantity, unchanged — kg for a material.

Upstream `TrivialConverter<T>`, the default, and by a wide margin the
most common: every plain `CapacityConstraint<Material>(throughput)` in
CYCAMORE's source, sink, reactor, separations, conversion and fuel-fab
agents uses it.

###### `Scaled`

`coefficient * quantity`.

Covers two upstream converters:

* `QtyCoeffConverter`, which weights a request in a portfolio of
  *mutual* requests (10 kg of MOX and 9 kg of UOX meeting one demand
  get coefficients `9.5/10` and `9.5/9`). The coefficient is per
  request, so translation resolves it — see
  [`Converter::RequestMassCoeff`].
* CYCAMORE's fuel-fab `FissConverter` / `FillConverter` /
  `TopupConverter`, each of which is a blending fraction times the
  offered quantity.

The coefficient is dimensionless and normally in `[0, 1]`, though
nothing requires that.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Constant`

A fixed amount, independent of the resource.

Covers the two constant branches of CYCAMORE's fuel-fab converters:
`0.0` ("this stream is not needed for this blend") and
[`CY_LARGE_DOUBLE`](crate::limits::CY_LARGE_DOUBLE) ("this blend is
impossible — do not bid at all", which drives the computed capacity to
effectively zero).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `RequestMassCoeff`

`quantity * (the requesting node's mass coefficient)`.

This is upstream's `QtyCoeffConverter` in its unresolved form: the
coefficient depends on *which request* the arc leads to, which the
converter cannot know until the arc exists. Upstream passes the arc and
an `ExchangeTranslationContext` into `convert()` to look it up; here
[`convert`](Self::convert) takes the coefficient as its second argument
and the translator supplies it.

Everywhere else the second argument is `1.0`, making this identical to
[`Converter::Quantity`].

##### Implementations

###### Methods

- ```rust
  pub fn convert(self: &Self, offer: &Resource, request_mass_coeff: f64) -> f64 { /* ... */ }
  ```
  Converts `offer` into the constrained quantity it would consume.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Converter { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    [`Converter::Quantity`], matching upstream's `TrivialConverter` default.

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Converter) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CapacityConstraint`

A limit on what an agent can trade: a capacity plus how resources consume
it.

Upstream `cyclus::CapacityConstraint<T>`.

# Conversions deliberately not ported

CYCAMORE's `SWUConverter` and `NatUConverter` (separative work and natural
uranium feed for an enrichment plant) are **not** variants of
[`Converter`]. Both are built on the enrichment toolkit — `Assays`,
`SwuRequired`, `FeedQty`, `UraniumAssayMass` — which lives in
[`toolkit`](crate::toolkit) and is not part of this module's port. Writing
the assay arithmetic a second time here to fill the enum would create the
duplicate implementation this workspace exists to avoid. When the toolkit
lands they are two more variants and one more `match` arm each.

```rust
pub struct CapacityConstraint {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(capacity: f64) -> Result<Self> { /* ... */ }
  ```
  A constraint of `capacity` units consumed at the resource's own

- ```rust
  pub fn with_converter(capacity: f64, converter: Converter) -> Result<Self> { /* ... */ }
  ```
  A constraint of `capacity` units in the converter's own units.

- ```rust
  pub fn capacity(self: &Self) -> f64 { /* ... */ }
  ```
  The capacity, in the converter's units.

- ```rust
  pub fn converter(self: &Self) -> Converter { /* ... */ }
  ```
  The converter.

- ```rust
  pub fn convert(self: &Self, offer: &Resource, request_mass_coeff: f64) -> f64 { /* ... */ }
  ```
  Shorthand for `self.converter().convert(offer, request_mass_coeff)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CapacityConstraint { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CapacityConstraint) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `graph`

The resource-neutral exchange graph: nodes, arcs, groups and matches.

This is the structure a [solver](crate::exchange::greedy) actually works
on. Requests and bids — which know about resources, agents and commodities
— are *translated* into it, and the solution comes back out as a list of
[`Match`]es which are translated back into
[`Trade`](crate::exchange::trade::Trade)s. Nothing in this module knows
what a [`Material`](crate::material::Material) is; that is the point.

# Shape of the graph

It is bipartite. By upstream's convention the **u** side is the requesting
side and the **v** side is the bidding side, so for every [`Arc`],
`arc.unode()` is a request node and `arc.vnode()` is a bid node.

Nodes are collected into [`ExchangeNodeGroup`]s. A *request* group is one
requester's portfolio and carries the total quantity that portfolio wants;
a *supply* group is one bidder's portfolio. Each group carries a vector of
**capacities**, and each node carries, per arc, a vector of **unit
capacities** — how much of each group capacity one unit of flow along that
arc consumes. The solver divides one by the other to get the flow a
constraint permits.

# Arena indices instead of `shared_ptr`

Upstream's graph is an object graph of `boost::shared_ptr<ExchangeNode>`
with `weak_ptr` back-references on each arc, and a `std::map` keyed on the
pointers. This workspace forbids trait objects, `Box` and lifetimes, so the
whole thing becomes an **arena**: [`ExchangeGraph`] owns flat `Vec`s of
nodes, arcs and groups, and every cross-reference is a [`NodeId`],
[`ArcId`] or [`GroupId`] index into them.

Three things improve as a result, and they are worth stating because they
are not merely a workaround:

1. **The cycle disappears.** Upstream needs `weak_ptr` on `Arc` precisely
   because nodes own arcs which reference nodes. An index has no ownership,
   so there is nothing to break.
2. **Iteration order becomes deterministic.** `std::map<Arc, double>` keyed
   on an `Arc` whose `operator<` compares *pointer values* iterates in an
   order that depends on the allocator. Here the same maps are keyed on
   [`ArcId`], which is insertion order, so two runs of the same input sum
   preferences in the same order and produce bit-identical results.
3. **`arc_ids_` and `arc_by_id_` vanish.** Upstream keeps two maps just to
   give each arc an integer id; here the id *is* the index.

# Inheritance becomes a tag

Upstream's `RequestGroup` derives from `ExchangeNodeGroup` and adds a
`qty_`. There is one struct here, [`ExchangeNodeGroup`], carrying a
[`GroupKind`] and a quantity that only request groups use. Both group
vectors on the graph are then lists of [`GroupId`] into a single arena,
which is what lets [`ExchangeNode::group`] be one plain index — a
`GroupId` that had to say *which* of two arenas it indexed would be an
enum, and every capacity lookup would have to match on it.

```rust
pub mod graph { /* ... */ }
```

### Types

#### Struct `NodeId`

An index into an [`ExchangeGraph`]'s node arena.

Replaces upstream's `boost::shared_ptr<ExchangeNode>`. Ordering is
insertion order, which makes every map keyed on a node deterministic.

```rust
pub struct NodeId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodeId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &NodeId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodeId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &NodeId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ArcId`

An index into an [`ExchangeGraph`]'s arc arena.

Replaces upstream's `Arc` used as a `std::map` key. Ordering is insertion
order; upstream ordered by the two node *pointers*, which is why its
preference maps iterate unpredictably.

```rust
pub struct ArcId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ArcId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &ArcId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ArcId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &ArcId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GroupId`

An index into an [`ExchangeGraph`]'s group arena.

Both request and supply groups live in one arena, so a `GroupId` alone
identifies a group; [`ExchangeNodeGroup::kind`] says which sort it is.

```rust
pub struct GroupId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GroupId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &GroupId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GroupId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &GroupId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `GroupKind`

Which side of the exchange a group sits on.

Upstream expresses this with inheritance — `RequestGroup : public
ExchangeNodeGroup` — which cannot survive the no-trait-objects rule.

```rust
pub enum GroupKind {
    Request,
    Supply,
}
```

##### Variants

###### `Request`

A requester's portfolio. Upstream `RequestGroup`. Carries a total
requested quantity, and automatically places each exclusive node in an
exclusive group of its own.

###### `Supply`

A bidder's portfolio. Upstream a plain `ExchangeNodeGroup`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GroupKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &GroupKind) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GroupKind) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &GroupKind) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ExchangeNode`

A translated request or bid.

One node is one [`Request`](crate::exchange::request::Request) or one
[`Bid`](crate::exchange::bid::Bid), stripped of everything the solver does
not need. Upstream `cyclus::ExchangeNode`.

```rust
pub struct ExchangeNode {
    pub group: Option<GroupId>,
    pub unit_capacities: alloc::collections::BTreeMap<ArcId, alloc::vec::Vec<f64>>,
    pub prefs: alloc::collections::BTreeMap<ArcId, f64>,
    pub exclusive: bool,
    pub commod: alloc::string::String,
    pub agent_id: Option<crate::agent::AgentId>,
    pub qty: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `group` | `Option<GroupId>` | The group this node belongs to, set by<br>[`ExchangeGraph::add_node`]. `None` only for a node that has not been<br>added to a graph; the solver rejects such a node, because a capacity<br>has no meaning without a group to draw it from. |
| `unit_capacities` | `alloc::collections::BTreeMap<ArcId, alloc::vec::Vec<f64>>` | Per-arc unit capacities: how much of each of the parent group's<br>capacities one unit of flow along that arc consumes.<br><br>The inner `Vec` is positionally aligned with<br>[`ExchangeNodeGroup::capacities`] — entry `i` is the unit consumption<br>of group capacity `i`. Dimensionless: it is<br>`converter(offer) / offer.quantity()`, so for the default<br>quantity-converter every entry is `1.0`.<br><br>An arc absent from this map, or present with an empty vector, means<br>"unconstrained along this arc" and the node's own [`qty`](Self::qty) is<br>the only limit. Upstream gets that behaviour from `std::map::operator[]`<br>default-inserting an empty vector. |
| `prefs` | `alloc::collections::BTreeMap<ArcId, f64>` | Per-arc preference, as seen by the *requesting* node. Strictly<br>positive; larger is more preferred.<br><br>Only request nodes carry entries here. Upstream reads it with<br>`std::map::operator[]`, which silently yields `0` for an arc that was<br>never assigned a preference; that is reproduced by treating a missing<br>key as `0.0`, and it matters because the objective divides by it. |
| `exclusive` | `bool` | Whether this request or bid must be satisfied in full or not at all. |
| `commod` | `alloc::string::String` | The commodity name, used by the<br>[preconditioner](crate::exchange::preconditioner) to look up a weight. |
| `agent_id` | `Option<crate::agent::AgentId>` | The agent this node belongs to, or `None` if it is unattached.<br><br>Used only to break preference ties, so that the ordering is total and<br>therefore reproducible.<br><br>**Divergence from upstream:** upstream holds an `int` with `-1` meaning<br>unset; here it is an `Option<AgentId>` with `None` for that case. An<br>in-band sentinel in an integer is the shape this crate hardens away<br>wherever it appears, and it matters for the tie-break specifically:<br>`Option`'s ordering puts `None` below every `Some`, which is exactly<br>where `-1` sorted, so the translated comparison is unchanged. |
| `qty` | `f64` | The maximum quantity that may be assigned to this node — the requested<br>amount for a request node, the offered amount for a bid node. Units are<br>the resource's (kilograms for a material). |

##### Implementations

###### Methods

- ```rust
  pub fn new(qty: f64) -> Self { /* ... */ }
  ```
  A node for `qty` units of a resource, not exclusive.

- ```rust
  pub fn exclusive(self: Self, exclusive: bool) -> Self { /* ... */ }
  ```
  Marks the node exclusive (all-or-nothing) or not.

- ```rust
  pub fn commodity(self: Self, commod: &str) -> Self { /* ... */ }
  ```
  Sets the commodity name.

- ```rust
  pub fn agent(self: Self, agent_id: AgentId) -> Self { /* ... */ }
  ```
  Sets the owning agent.

- ```rust
  pub fn pref(self: &Self, a: ArcId) -> f64 { /* ... */ }
  ```
  The preference of the arc `a` as seen from this node, or `0.0` if this

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExchangeNode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    An unconstrained node: quantity [`UNLIMITED`], not exclusive, no

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ExchangeNode) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Arc`

A possible trade: a connection from a request node to a bid node.

Upstream `cyclus::Arc`. Copyable here, because it is five scalars once the
`weak_ptr`s become indices.

```rust
pub struct Arc {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn unode(self: &Self) -> NodeId { /* ... */ }
  ```
  The request node. Upstream `Arc::unode()`.

- ```rust
  pub fn vnode(self: &Self) -> NodeId { /* ... */ }
  ```
  The bid node. Upstream `Arc::vnode()`.

- ```rust
  pub fn exclusive(self: &Self) -> bool { /* ... */ }
  ```
  Whether either endpoint is exclusive, so that flow along this arc must

- ```rust
  pub fn excl_val(self: &Self) -> f64 { /* ... */ }
  ```
  The only quantity that may flow along this arc if it is exclusive, in

- ```rust
  pub fn pref(self: &Self) -> f64 { /* ... */ }
  ```
  The requester's preference for this arc. Strictly positive; larger is

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Arc { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Arc) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ExchangeNodeGroup`

One requester's or one bidder's portfolio, as the solver sees it.

Upstream `cyclus::ExchangeNodeGroup` and its subclass `RequestGroup`,
merged into one struct tagged with a [`GroupKind`] (see the module doc).

```rust
pub struct ExchangeNodeGroup {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn kind(self: &Self) -> GroupKind { /* ... */ }
  ```
  Which side of the exchange this group is on.

- ```rust
  pub fn qty(self: &Self) -> f64 { /* ... */ }
  ```
  The total quantity this group wants, in the resource's units.

- ```rust
  pub fn nodes(self: &Self) -> &[NodeId] { /* ... */ }
  ```
  The nodes in this group, in solver order.

- ```rust
  pub fn excl_node_groups(self: &Self) -> &[Vec<NodeId>] { /* ... */ }
  ```
  Sets of nodes over which flow may exist on at most one arc.

- ```rust
  pub fn capacities(self: &Self) -> &[f64] { /* ... */ }
  ```
  The group's flow capacities, one per

- ```rust
  pub fn has_arcs(self: &Self, graph: &ExchangeGraph) -> bool { /* ... */ }
  ```
  `true` if any node in this group has at least one arc. Upstream

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExchangeNodeGroup { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ExchangeNodeGroup) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Match`

A solved flow: `qty` units along `arc`.

Upstream `typedef std::pair<Arc, double> Match`. A named struct here
because `.0`/`.1` on a pair of an arc and a quantity reads badly at every
call site.

```rust
pub struct Match {
    pub arc: ArcId,
    pub qty: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `arc` | `ArcId` | The arc carrying the flow. |
| `qty` | `f64` | How much flows, in the resource's units. Strictly positive. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Match { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Match) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ExchangeGraph`

The whole exchange, as a graph.

Built by translating portfolios (see
[`ExchangeContext`](crate::exchange::translate::ExchangeContext)), solved
by a [`GreedySolver`](crate::exchange::greedy::GreedySolver), and read back
as [`Match`]es. Upstream `cyclus::ExchangeGraph`.

```rust
pub struct ExchangeGraph {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty graph.

- ```rust
  pub fn add_request_group(self: &mut Self, qty: f64) -> GroupId { /* ... */ }
  ```
  Adds a request group wanting `qty` units in total, and returns its id.

- ```rust
  pub fn add_supply_group(self: &mut Self) -> GroupId { /* ... */ }
  ```
  Adds a supply (bid) group and returns its id. Upstream

- ```rust
  pub fn add_node(self: &mut Self, group: GroupId, node: ExchangeNode) -> Result<NodeId> { /* ... */ }
  ```
  Adds `node` to `group` and returns its id.

- ```rust
  pub fn add_excl_group(self: &mut Self, group: GroupId, nodes: Vec<NodeId>) -> Result<()> { /* ... */ }
  ```
  Records that flow may exist over at most one of `nodes`' arcs.

- ```rust
  pub fn add_capacity(self: &mut Self, group: GroupId, capacity: f64) -> Result<()> { /* ... */ }
  ```
  Appends a flow capacity to `group`. Upstream `AddCapacity`.

- ```rust
  pub fn add_arc(self: &mut Self, unode: NodeId, vnode: NodeId) -> Result<ArcId> { /* ... */ }
  ```
  Connects request node `unode` to bid node `vnode` and returns the new

- ```rust
  pub fn set_arc_pref(self: &mut Self, arc: ArcId, pref: f64) -> Result<()> { /* ... */ }
  ```
  Sets an arc's preference. Upstream `Arc::pref(double)`.

- ```rust
  pub fn add_match(self: &mut Self, arc: ArcId, qty: f64) { /* ... */ }
  ```
  Records `qty` units of flow along `arc`. Upstream `AddMatch`.

- ```rust
  pub fn clear_matches(self: &mut Self) { /* ... */ }
  ```
  Discards every recorded match, so the graph can be re-solved.

- ```rust
  pub fn matches(self: &Self) -> &[Match] { /* ... */ }
  ```
  The solution: every flow the solver assigned, in the order it assigned

- ```rust
  pub fn nodes(self: &Self) -> &[ExchangeNode] { /* ... */ }
  ```
  Every node, indexed by [`NodeId`].

- ```rust
  pub fn arcs(self: &Self) -> &[Arc] { /* ... */ }
  ```
  Every arc, indexed by [`ArcId`].

- ```rust
  pub fn groups(self: &Self) -> &[ExchangeNodeGroup] { /* ... */ }
  ```
  Every group, indexed by [`GroupId`], request and supply intermixed in

- ```rust
  pub fn request_groups(self: &Self) -> &[GroupId] { /* ... */ }
  ```
  The request groups, in the order the solver will visit them. The

- ```rust
  pub fn supply_groups(self: &Self) -> &[GroupId] { /* ... */ }
  ```
  The supply groups, in creation order.

- ```rust
  pub fn arcs_for_node(self: &Self, node: NodeId) -> &[ArcId] { /* ... */ }
  ```
  The arcs incident on a node. Upstream `node_arc_map()`.

- ```rust
  pub fn node(self: &Self, node: NodeId) -> Result<&ExchangeNode> { /* ... */ }
  ```
  Borrows a node.

- ```rust
  pub fn node_mut(self: &mut Self, node: NodeId) -> Result<&mut ExchangeNode> { /* ... */ }
  ```
  Mutably borrows a node, to set its preferences or unit capacities.

- ```rust
  pub fn arc(self: &Self, arc: ArcId) -> Result<&Arc> { /* ... */ }
  ```
  Borrows an arc.

- ```rust
  pub fn group(self: &Self, group: GroupId) -> Result<&ExchangeNodeGroup> { /* ... */ }
  ```
  Borrows a group.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExchangeGraph { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ExchangeGraph { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ExchangeGraph) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `exclusive_value`

**Attributes:**

- `MustUse { reason: None }`

The quantity an exclusive arc must carry, or `0.0` if it can carry none.

This is upstream's `Arc::Arc(unode, vnode)` body, lifted out so it can be
tested directly, and it is one of the subtlest few lines in Cyclus.

# Why the ULP comparison

An exclusive order is all-or-nothing, so an arc between an exclusive
request for `x` and a bid of `y` can only ever carry flow if `x` and `y`
are the *same* quantity. But both numbers have usually been through a
division and a multiplication by the time they get here, so an exact `==`
would reject arcs that are equal in every sense that matters. Upstream
therefore compares them with `boost::math::float_distance` and accepts a
disagreement of up to [`FLOAT_ULP_EQ`] (2) representable values, with the
comment that this "is vital for preventing false positive constraint
violations w.r.t. exclusivity-related capacity". The translation uses
[`float_distance`](crate::limits::float_distance), which is that function.

# The three cases

With `dist = float_distance(u_qty, v_qty)` (positive when the bid offers
more than the request wants):

* **both exclusive** — the quantities must agree to within 2 ULP in either
  direction, and the arc carries the request's quantity.
* **only the request is exclusive** — the bid must offer *at least* the
  requested amount (`dist >= -2` ULP), and the arc carries the requested
  amount. A larger bid is fine; it is simply not fully consumed.
* **only the bid is exclusive** — the request must want *at least* the
  offered amount (`dist <= +2` ULP), and the arc carries the offered
  amount.

# Parameters

`u_qty`/`v_qty` are the request-side and bid-side quantities in the
resource's units; `u_excl`/`v_excl` say which side is exclusive.

```rust
pub fn exclusive_value(u_qty: f64, u_excl: bool, v_qty: f64, v_excl: bool) -> f64 { /* ... */ }
```

## Module `greedy`

The greedy solver — Cyclus's default, and the only one here.

# The algorithm, in full

[Condition](crate::exchange::preconditioner) the graph, then walk every
request group in the order conditioning left them. Within a group, walk the
request nodes in order of descending average preference. For each request
node, sort its arcs by the requester's preference for them, descending, and
walk those. For each arc, offer it the smaller of (what is still needed)
and (what the arc's two endpoints can still supply given every constraint
they are under). If that is more than [`EPS`], record the match, decrement
both endpoints' group capacities, and move on. Stop when the group's demand
is met or its request nodes run out.

It is greedy in the strict sense: no assignment is ever revisited. A
request that grabs a supplier's last capacity keeps it even if a later,
keener request would have valued it more. That is why the ordering work in
the preconditioner matters as much as the matching work here.

# What is NOT here: the Coin-OR solver

Upstream also ships `ProgSolver`, a mixed-integer-programme formulation
solved through `OsiCbcSolverInterface` (Coin-OR / Cbc), which finds a true
optimum rather than a greedy one. It is **deliberately not ported**, and it
is not a gap to be filled later on a whim: Cbc is a large C++ dependency, it
cannot be `no_std`, and this crate's single-dependency design (see the
crate root) exists precisely so a fuel-cycle model can run on a
microcontroller or in a browser. The greedy solver is what upstream selects
by default, and what the great majority of published Cyclus results were
produced with.

One visible consequence: [`ExchangeNodeGroup::excl_node_groups`](crate::exchange::graph::ExchangeNodeGroup::excl_node_groups) is ported
and populated but never read, because it exists for the programme
formulation's "at most one of these arcs may carry flow" constraint. The
greedy solver enforces exclusivity per arc instead, through
[`Arc::excl_val`].

# Two float comparisons that look wrong and are not

Both are load-bearing, both are upstream's, and both are easy to "fix" into
a bug.

**Exact equality against [`UNLIMITED`].** A group capacity of exactly
`f64::MAX` means *unbounded*. It is a sentinel, not a measurement: nothing
computes `f64::MAX` by accident, and the only way a capacity holds that
value is that a caller put it there. Comparing a sentinel by identity is
correct; comparing it with a tolerance would make a merely enormous
capacity behave as infinite. The sentinel is checked twice — when computing
a capacity (so the division `capacity / unit_capacity` is skipped, which
would otherwise turn `f64::MAX` into something finite) and when decrementing
one (so it stays unbounded forever).

**ULP-distance equality for exclusive orders.** See
[`exclusive_value`](crate::exchange::graph::exclusive_value) for the
construction; the solver applies it a second time, to the quantity it is
about to match. Upstream's comment is worth repeating: the careful float
comparison "is vital for preventing false positive constraint violations
w.r.t. exclusivity-related capacity". An exclusive order whose available
capacity falls one or two ULP short of the required quantity is matched in
full; three ULP short and it is matched not at all. There is no middle
ground, by construction.

```rust
pub mod greedy { /* ... */ }
```

### Types

#### Struct `GreedySolver`

The greedy exchange solver.

Create one, hand it a graph, read the [`Match`](crate::exchange::graph::Match)es
off the graph afterwards. A solver may be reused across time steps; every
call to [`solve`](Self::solve) resets its internal state.

Upstream `cyclus::GreedySolver`, which also inherits the pseudo-cost
machinery from `cyclus::ExchangeSolver`; both are here.

```rust
pub struct GreedySolver {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  A solver with exclusive orders enforced and a default (unweighted)

- ```rust
  pub fn with_exclusive_orders(self: Self, exclusive_orders: bool) -> Self { /* ... */ }
  ```
  Chooses whether exclusive orders are enforced when costing arcs.

- ```rust
  pub fn with_conditioner(self: Self, conditioner: Option<GreedyPreconditioner>) -> Self { /* ... */ }
  ```
  Chooses the preconditioner, or `None` to leave the graph's order

- ```rust
  pub fn solve(self: &mut Self, graph: &mut ExchangeGraph) -> Result<f64> { /* ... */ }
  ```
  Solves `graph` in place, appending a

- ```rust
  pub fn group_capacities(self: &Self, group: GroupId) -> &[f64] { /* ... */ }
  ```
  The solver's remaining working capacities for a group, in the same

- ```rust
  pub fn node_qty(self: &Self, node: NodeId) -> f64 { /* ... */ }
  ```
  The quantity the solver has assigned to a node so far, in the

- ```rust
  pub fn obj(self: &Self) -> f64 { /* ... */ }
  ```
  The objective value of the last solve.

- ```rust
  pub fn unmatched(self: &Self) -> f64 { /* ... */ }
  ```
  The total demand left unfilled by the last solve, in the resource's

- ```rust
  pub fn capacity_arc(self: &Self, graph: &ExchangeGraph, arc: ArcId, u_curr_qty: f64, v_curr_qty: f64) -> Result<f64> { /* ... */ }
  ```
  The flow an arc can carry, given how much its endpoints already carry.

- ```rust
  pub fn capacity_node(self: &Self, graph: &ExchangeGraph, node: NodeId, arc: ArcId, min_cap: bool, curr_qty: f64) -> Result<f64> { /* ... */ }
  ```
  The flow a single node can still take along an arc.

- ```rust
  pub fn cost(arc: &Arc, exclusive_orders: bool) -> f64 { /* ... */ }
  ```
  The cost of an arc. Upstream `ExchangeSolver::Cost`.

- ```rust
  pub fn arc_cost(self: &Self, arc: &Arc) -> f64 { /* ... */ }
  ```
  [`cost`](Self::cost) with this solver's exclusive-orders setting.

- ```rust
  pub fn pseudo_cost(self: &Self, graph: &ExchangeGraph) -> f64 { /* ... */ }
  ```
  The per-unit cost charged to unmet demand, at upstream's default cost

- ```rust
  pub fn pseudo_cost_by_pref(self: &Self, graph: &ExchangeGraph, cost_factor: f64) -> f64 { /* ... */ }
  ```
  The per-unit cost charged to unmet demand.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GreedySolver { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> GreedySolver { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GreedySolver) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `DEFAULT_EXCLUSIVE`

Whether upstream's exclusive-order handling is applied at all.

Upstream `ExchangeSolver::kDefaultExclusive`, which is `true`.

```rust
pub const DEFAULT_EXCLUSIVE: bool = true;
```

## Module `portfolio`

Portfolios: one agent's requests, or one agent's bids, with the
constraints that act across all of them.

A portfolio is the unit of *mutual* satisfaction. Putting two requests in
one portfolio and declaring them mutual says "either of these will do";
putting two bids in one portfolio says "these share my throughput". The
constraints hang off the portfolio, not the individual request or bid,
which is why one portfolio becomes one
[`ExchangeNodeGroup`](crate::exchange::graph::ExchangeNodeGroup) in the
graph and its constraints become that group's capacities.

# Ownership without `shared_ptr`

Upstream portfolios hold raw `Request<T>*`/`Bid<T>*` and delete them in the
destructor, while the portfolio itself is a `shared_ptr` with
`enable_shared_from_this` so each request can hold a `weak_ptr` back to it.
Here the portfolio owns its requests by value in a `Vec`, and the back
reference is the index — a [`RequestId`] or [`BidId`](crate::exchange::bid::BidId) naming (portfolio,
index). Nothing is shared, nothing is freed twice, and the cycle that
forced the `weak_ptr` never forms.

```rust
pub mod portfolio { /* ... */ }
```

### Types

#### Struct `RequestPortfolio`

One requester's demands, and the constraints across them.

Upstream `cyclus::RequestPortfolio<T>`.

```rust
pub struct RequestPortfolio {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty portfolio, with no requester yet.

- ```rust
  pub fn add_request(self: &mut Self, request: Request) -> Result<usize> { /* ... */ }
  ```
  Adds a request and returns its index within this portfolio.

- ```rust
  pub fn request(self: &mut Self, target: Resource, requester: AgentId, commodity: &str, preference: f64, exclusive: bool) -> Result<usize> { /* ... */ }
  ```
  Convenience: builds a request from its parts and adds it.

- ```rust
  pub fn add_mutual_reqs(self: &mut Self, indices: &[usize]) -> Result<()> { /* ... */ }
  ```
  Declares the named requests mutually satisfying: any one of them fills

- ```rust
  pub fn add_constraint(self: &mut Self, constraint: CapacityConstraint) { /* ... */ }
  ```
  Adds a constraint acting across every request in the portfolio.

- ```rust
  pub fn requester(self: &Self) -> Option<AgentId> { /* ... */ }
  ```
  The requesting agent, or `None` if no request has been added yet.

- ```rust
  pub fn qty(self: &Self) -> f64 { /* ... */ }
  ```
  The total quantity this portfolio demands, in the resource's units.

- ```rust
  pub fn requests(self: &Self) -> &[Request] { /* ... */ }
  ```
  The requests, in insertion order.

- ```rust
  pub fn constraints(self: &Self) -> &[CapacityConstraint] { /* ... */ }
  ```
  The constraints, in insertion order.

- ```rust
  pub fn mass_coeff(self: &Self, index: usize) -> f64 { /* ... */ }
  ```
  The mass coefficient of request `index` — `1.0` unless it was named in

- ```rust
  pub fn qty_constraint(self: &Self) -> Result<CapacityConstraint> { /* ... */ }
  ```
  The automatic mass constraint every request portfolio gets during

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> RequestPortfolio { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> RequestPortfolio { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RequestPortfolio) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BidPortfolio`

One bidder's offers, and the constraints across them.

Upstream `cyclus::BidPortfolio<T>`.

```rust
pub struct BidPortfolio {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty portfolio, with no bidder yet.

- ```rust
  pub fn add_bid(self: &mut Self, bid: Bid) -> Result<usize> { /* ... */ }
  ```
  Adds a bid and returns its index within this portfolio.

- ```rust
  pub fn bid(self: &mut Self, request: RequestId, offer: Resource, bidder: AgentId, exclusive: bool) -> Result<usize> { /* ... */ }
  ```
  Convenience: builds a bid from its parts and adds it.

- ```rust
  pub fn add_constraint(self: &mut Self, constraint: CapacityConstraint) { /* ... */ }
  ```
  Adds a constraint acting across every bid in the portfolio — a

- ```rust
  pub fn bidder(self: &Self) -> Option<AgentId> { /* ... */ }
  ```
  The bidding agent, or `None` if no bid has been added yet. Same

- ```rust
  pub fn bids(self: &Self) -> &[Bid] { /* ... */ }
  ```
  The bids, in insertion order.

- ```rust
  pub fn constraints(self: &Self) -> &[CapacityConstraint] { /* ... */ }
  ```
  The constraints, in insertion order.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BidPortfolio { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> BidPortfolio { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BidPortfolio) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `preconditioner`

Node and group ordering: deciding what the greedy solver sees first.

A greedy solver serves whatever it meets first, so the order *is* the
policy. The preconditioner sets that order from two inputs: a
caller-supplied **commodity weight** (how important is this commodity?) and
the **average preference** of a node's arcs (how much does the requester
want these particular bids?).

The node weight is

```text
w_node = w_commod * (1 + p / (1 + p))
```

where `p` is the node's average arc preference. The second factor is
bounded in `[1, 2)`, so **preference can never outrank commodity weight**:
a node whose commodity is twice as important always sorts ahead, however
keenly the other node's bids are preferred. That is the whole design, and
it is easy to lose by "simplifying" the formula.

Conditioning then happens in three steps, matching upstream exactly:

1. Within each request group, sort nodes by node weight, descending.
2. Compute each group's weight as the mean of its nodes' weights.
3. Sort the request groups by group weight, descending.

Every sort is **stable**, so nodes of equal weight keep the order they were
created in and the solution is reproducible. Upstream uses
`std::stable_sort` for the same reason; Rust's `sort_by` is stable, and
`sort_unstable_by` must never be substituted here.

```rust
pub mod preconditioner { /* ... */ }
```

### Types

#### Enum `WgtOrder`

Whether commodity weights were given heaviest-first or lightest-first.

Upstream `GreedyPreconditioner::WgtOrder`.

```rust
pub enum WgtOrder {
    Reverse,
    HeaviestFirst,
}
```

##### Variants

###### `Reverse`

Weights are given lightest-first and must be reversed:
`w -> max + min - w`. Upstream `REVERSE`.

###### `HeaviestFirst`

Weights are already heaviest-first and are used as given. Upstream
`END`, the default.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> WgtOrder { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &WgtOrder) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GreedyPreconditioner`

Orders an [`ExchangeGraph`] for the greedy solver.

Upstream `cyclus::GreedyPreconditioner`. Conditioning is in place and
idempotent for a given graph: it only permutes node and group order.

```rust
pub struct GreedyPreconditioner {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  A preconditioner with no commodity weights: every commodity counts

- ```rust
  pub fn with_weights(commod_weights: BTreeMap<String, f64>) -> Self { /* ... */ }
  ```
  A preconditioner with commodity weights given **heaviest first** —

- ```rust
  pub fn with_weights_ordered(commod_weights: BTreeMap<String, f64>, order: WgtOrder) -> Self { /* ... */ }
  ```
  A preconditioner with commodity weights in either direction.

- ```rust
  pub fn commod_weights(self: &Self) -> &BTreeMap<String, f64> { /* ... */ }
  ```
  The commodity weights, after any reversal.

- ```rust
  pub fn condition(self: &Self, graph: &mut ExchangeGraph) { /* ... */ }
  ```
  Reorders `graph`'s request groups, and the nodes within each, as

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GreedyPreconditioner { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> GreedyPreconditioner { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GreedyPreconditioner) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `avg_pref`

**Attributes:**

- `MustUse { reason: None }`

The mean preference across a node's arcs, or `0.0` if it has none.

Upstream `cyclus::AvgPref`. Only request nodes carry preferences, so a bid
node always scores `0.0`.

The sum is taken in [`ArcId`](crate::exchange::graph::ArcId) order, which
is arc creation order. Upstream sums in `std::map<Arc, double>` order,
which compares the arcs' *node pointers* — so the result there depends on
where the allocator happened to put the nodes, and two runs of the same
input can differ in the last bits. This ordering is deterministic.

```rust
pub fn avg_pref(node: &crate::exchange::graph::ExchangeNode) -> f64 { /* ... */ }
```

#### Function `node_weight`

**Attributes:**

- `MustUse { reason: None }`

The conditioning weight of a node: `w_commod * (1 + p / (1 + p))`.

Upstream `cyclus::NodeWeight`.

`weights` maps commodity name to weight; pass `None` (or an empty map) to
weight every commodity equally at `1.0`. `avg_pref` is the node's average
arc preference, from [`avg_pref`].

# A commodity missing from a non-empty map scores zero

Upstream's header says `Condition` "throws KeyError if a commodity is in
the graph but not in the weight mapping". The implementation does no such
thing: it indexes a `std::map` with `operator[]`, which default-constructs
a `0.0` and inserts it. The *code* is translated, not the comment, because
the code is what every existing simulation has been running against — but
the consequence is worth knowing: one misspelled commodity name silently
sends those requests to the back of the queue instead of failing loudly.

```rust
pub fn node_weight(node: &crate::exchange::graph::ExchangeNode, weights: Option<&alloc::collections::BTreeMap<alloc::string::String, f64>>, avg_pref: f64) -> f64 { /* ... */ }
```

#### Function `group_weight`

**Attributes:**

- `MustUse { reason: None }`

The mean node weight across a group, or `0.0` for an empty group.

Upstream `cyclus::GroupWeight`. `avg_prefs` supplies each node's average
preference, so that it is computed once per node rather than once per
comparison.

```rust
pub fn group_weight(graph: &crate::exchange::graph::ExchangeGraph, group: crate::exchange::graph::GroupId, weights: Option<&alloc::collections::BTreeMap<alloc::string::String, f64>>, avg_prefs: &alloc::collections::BTreeMap<crate::exchange::graph::NodeId, f64>) -> f64 { /* ... */ }
```

## Module `request`

A request: "I want this much of this commodity, and here is how much I
want it."

# Not generic over the resource

Upstream is `Request<T>`, instantiated as `Request<Material>` and
`Request<Product>`, with the target held as a `boost::shared_ptr<T>`. A
faithful generic translation would be `Request<T>` holding a `T` — which
works, but then every downstream container, every portfolio, every
translation map and the whole exchange context becomes generic too, and any
attempt to hold requests for both resource kinds in one collection needs a
trait object.

So [`Request`] is **not** generic: it holds a
[`Resource`](crate::resource::Resource), the crate's owned enum with
`Material` and `Product` variants. One non-generic type covers both
upstream instantiations, a single exchange can carry both kinds at once,
and no lifetime or `dyn` appears anywhere. The cost is that a requester
wanting a material out of a trade calls
[`Resource::into_material`](crate::resource::Resource::into_material) and
handles the error case — which upstream gets for free from its template
parameter, and which is the one genuine ergonomic loss.

```rust
pub mod request { /* ... */ }
```

### Types

#### Struct `RequestId`

Identifies one request inside an
[`ExchangeContext`](crate::exchange::translate::ExchangeContext).

Upstream passes bare `Request<T>*` pointers around and relies on the owning
portfolio to free them. Here a request is owned by its portfolio and
referred to by the pair (portfolio, index), so a [`Bid`](crate::exchange::bid::Bid)
can name the request it answers without a pointer, a lifetime or an `Rc`.

```rust
pub struct RequestId {
    pub portfolio: usize,
    pub index: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `portfolio` | `usize` | Index of the owning [`RequestPortfolio`](crate::exchange::portfolio::RequestPortfolio)<br>within the exchange context. |
| `index` | `usize` | Index of the request within that portfolio. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> RequestId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &RequestId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RequestId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &RequestId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Request`

One agent's demand for one commodity.

Upstream `cyclus::Request<T>`.

```rust
pub struct Request {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(target: Resource, requester: AgentId, commodity: &str, preference: f64, exclusive: bool) -> Result<Self> { /* ... */ }
  ```
  Creates a request.

- ```rust
  pub fn simple(target: Resource, requester: AgentId, commodity: &str) -> Result<Self> { /* ... */ }
  ```
  A request at [`DEFAULT_PREF`], not exclusive.

- ```rust
  pub fn target(self: &Self) -> &Resource { /* ... */ }
  ```
  The resource wanted.

- ```rust
  pub fn quantity(self: &Self) -> f64 { /* ... */ }
  ```
  How much is wanted, in the resource's units.

- ```rust
  pub fn requester(self: &Self) -> AgentId { /* ... */ }
  ```
  The requesting agent.

- ```rust
  pub fn commodity(self: &Self) -> &str { /* ... */ }
  ```
  The commodity name.

- ```rust
  pub fn preference(self: &Self) -> f64 { /* ... */ }
  ```
  The preference, strictly positive and dimensionless.

- ```rust
  pub fn exclusive(self: &Self) -> bool { /* ... */ }
  ```
  Whether this request must be met in full by one bid.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Request { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Request) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `DEFAULT_PREF`

The preference a request carries unless it says otherwise.

Upstream `kDefaultPref`, which was changed from `0` to `1` in Cyclus 1.4
because a zero preference makes the solver's objective term `qty / pref`
infinite. Preferences may be any strictly positive value; larger means more
preferred. Dimensionless.

```rust
pub const DEFAULT_PREF: f64 = 1.0;
```

## Module `trade`

The output of the exchange: who gives how much of what, to whom.

```rust
pub mod trade { /* ... */ }
```

### Types

#### Struct `Trade`

A decided trade — a request, the bid that fills it, and how much moves.

Upstream `cyclus::Trade<T>`. The amount may be less than either the request
or the bid quantity, because the solver splits partially-filled orders
(unless either side is exclusive, in which case it is all or nothing).

```rust
pub struct Trade {
    pub request: crate::exchange::request::RequestId,
    pub bid: crate::exchange::bid::BidId,
    pub amt: f64,
    pub price: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `request` | `crate::exchange::request::RequestId` | The request being filled. |
| `bid` | `crate::exchange::bid::BidId` | The bid filling it. |
| `amt` | `f64` | How much actually moves, in the resource's units (kg for a material).<br>Strictly positive. |
| `price` | `f64` | Price per unit.<br><br>Upstream carries this field and its own comment says it "is not<br>currently used"; it is defaulted to `0.0` and kept for fidelity, so<br>that an economics layer added later does not have to change this type. |

##### Implementations

###### Methods

- ```rust
  pub fn new(request: RequestId, bid: BidId, amt: f64) -> Self { /* ... */ }
  ```
  A trade of `amt` units at the default price of zero.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Trade { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Trade) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `translate`

Turning portfolios into a graph, and matches back into trades.

This is the bridge between the two halves of the module: the
resource-aware half ([`Request`], [`Bid`], portfolios, constraints) and the
resource-blind half ([`ExchangeGraph`]). Collect portfolios into an
[`ExchangeContext`], [`translate`](ExchangeContext::translate) it, solve the
resulting graph, then
[`back_translate`](Translation::back_translate) the matches into
[`Trade`]s.

Upstream splits this across `ExchangeContext` (collecting portfolios and
resolving preferences), `ExchangeTranslator` (building the graph) and
`ExchangeTranslationContext` (the four node/request/bid maps). All three
are small, none is useful without the others, and two of them are templates
over the resource type — so they are one non-generic type here plus its
output.

```rust
pub mod translate { /* ... */ }
```

### Types

#### Struct `ExchangeContext`

Every portfolio offered in one time step's exchange.

Upstream `cyclus::ExchangeContext<T>`.

```rust
pub struct ExchangeContext {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty context.

- ```rust
  pub fn add_request_portfolio(self: &mut Self, portfolio: RequestPortfolio) -> usize { /* ... */ }
  ```
  Adds a request portfolio and returns its index, which is the

- ```rust
  pub fn add_bid_portfolio(self: &mut Self, portfolio: BidPortfolio) -> usize { /* ... */ }
  ```
  Adds a bid portfolio and returns its index.

- ```rust
  pub fn request_portfolios(self: &Self) -> &[RequestPortfolio] { /* ... */ }
  ```
  The request portfolios, in insertion order.

- ```rust
  pub fn bid_portfolios(self: &Self) -> &[BidPortfolio] { /* ... */ }
  ```
  The bid portfolios, in insertion order.

- ```rust
  pub fn request(self: &Self, id: RequestId) -> Result<&Request> { /* ... */ }
  ```
  Borrows a request by id.

- ```rust
  pub fn bid(self: &Self, id: BidId) -> Result<&Bid> { /* ... */ }
  ```
  Borrows a bid by id.

- ```rust
  pub fn arc_preference(self: &Self, bid: &Bid) -> Result<f64> { /* ... */ }
  ```
  The preference of the arc a bid would create.

- ```rust
  pub fn translate(self: &Self) -> Result<Translation> { /* ... */ }
  ```
  Builds the exchange graph.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExchangeContext { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ExchangeContext { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ExchangeContext) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Translation`

A translated exchange: the graph, plus the maps needed to read its solution
back as trades.

Upstream `ExchangeTranslationContext<T>` together with the graph the
translator produced.

```rust
pub struct Translation {
    pub graph: crate::exchange::graph::ExchangeGraph,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `graph` | `crate::exchange::graph::ExchangeGraph` | The graph to solve. Public because solving mutates it in place, and<br>the caller chooses the solver. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn request_of(self: &Self, node: NodeId) -> Option<RequestId> { /* ... */ }
  ```
  The request a node was translated from, if it is a request node.

- ```rust
  pub fn bid_of(self: &Self, node: NodeId) -> Option<BidId> { /* ... */ }
  ```
  The bid a node was translated from, if it is a bid node.

- ```rust
  pub fn back_translate(self: &Self) -> Result<Vec<Trade>> { /* ... */ }
  ```
  Converts the solved graph's matches into trades.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Translation { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Translation) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `Bid`

```rust
pub use bid::Bid;
```

#### Re-export `BidId`

```rust
pub use bid::BidId;
```

#### Re-export `CapacityConstraint`

```rust
pub use constraint::CapacityConstraint;
```

#### Re-export `Converter`

```rust
pub use constraint::Converter;
```

#### Re-export `exclusive_value`

```rust
pub use graph::exclusive_value;
```

#### Re-export `Arc`

```rust
pub use graph::Arc;
```

#### Re-export `ArcId`

```rust
pub use graph::ArcId;
```

#### Re-export `ExchangeGraph`

```rust
pub use graph::ExchangeGraph;
```

#### Re-export `ExchangeNode`

```rust
pub use graph::ExchangeNode;
```

#### Re-export `ExchangeNodeGroup`

```rust
pub use graph::ExchangeNodeGroup;
```

#### Re-export `GroupId`

```rust
pub use graph::GroupId;
```

#### Re-export `GroupKind`

```rust
pub use graph::GroupKind;
```

#### Re-export `Match`

```rust
pub use graph::Match;
```

#### Re-export `NodeId`

```rust
pub use graph::NodeId;
```

#### Re-export `GreedySolver`

```rust
pub use greedy::GreedySolver;
```

#### Re-export `BidPortfolio`

```rust
pub use portfolio::BidPortfolio;
```

#### Re-export `RequestPortfolio`

```rust
pub use portfolio::RequestPortfolio;
```

#### Re-export `GreedyPreconditioner`

```rust
pub use preconditioner::GreedyPreconditioner;
```

#### Re-export `WgtOrder`

```rust
pub use preconditioner::WgtOrder;
```

#### Re-export `Request`

```rust
pub use request::Request;
```

#### Re-export `RequestId`

```rust
pub use request::RequestId;
```

#### Re-export `DEFAULT_PREF`

```rust
pub use request::DEFAULT_PREF;
```

#### Re-export `Trade`

```rust
pub use trade::Trade;
```

#### Re-export `ExchangeContext`

```rust
pub use translate::ExchangeContext;
```

#### Re-export `Translation`

```rust
pub use translate::Translation;
```

## Module `limits`

Floating-point tolerances, translated from `cyc_limits.h.in`.

# Why these are constants here and mutable globals upstream

Upstream declares `extern double cy_eps` and `extern double cy_eps_rsrc` —
process-global mutable state, settable from the Python bindings at run
time. That does not survive the translation for two reasons, and the
difference is worth stating because it is a genuine behavioural change:

 1. A mutable global would need a `static mut` or a lock, and in `no_std`
    there is no `OnceLock` to reach for.
 2. A tolerance that can change underneath a running simulation makes two
    runs of the same input irreproducible, which defeats the point of
    building this on PETIR's bit-identical arithmetic in the first place.

The values are upstream's compiled-in defaults, taken from
`CMakeLists.txt`'s `CY_NEAR_ZERO`/`cy_eps` initialisation. A caller who
needs a different tolerance passes it explicitly — every function in this
crate that compares against a tolerance takes it as an argument, and these
constants are what the convenience wrappers supply.

```rust
pub mod limits { /* ... */ }
```

### Functions

#### Function `abs`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Absolute value, `no_std`-safe.

`f64::abs` is a `std`-only inherent method. Rather than depend on `libm`
directly for one sign flip, this does it arithmetically.

```rust
pub fn abs(x: f64) -> f64 { /* ... */ }
```

#### Function `is_negative`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Returns `true` if `d` is less than `-EPS`. Upstream `cyclus::IsNegative`.

Note this is *not* `d < 0.0`: a value in `(-EPS, 0)` is treated as zero,
which is what lets the resource layer tolerate the rounding that
accumulates over a long simulation without reporting a negative inventory.

```rust
pub fn is_negative(d: f64) -> bool { /* ... */ }
```

#### Function `almost_eq`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Returns `true` if `d1` and `d2` are within [`EPS`]. Upstream
`cyclus::AlmostEq`.

```rust
pub fn almost_eq(d1: f64, d2: f64) -> bool { /* ... */ }
```

#### Function `float_distance`

**Attributes:**

- `MustUse { reason: None }`

The number of representable `f64` values between `a` and `b`, signed.

This is the translation of `boost::math::float_distance`, which upstream
uses in exactly two places — [`Arc`](crate::exchange::graph::Arc)
construction and the greedy solver's exclusive-order adjustment — and
nowhere else. Boost is not available here and pulling in a floating-point
utility crate for one function would be a heavier dependency than the
function.

# How it works

IEEE-754 binary64 is ordered such that, for two finite values of the same
sign, reinterpreting the bit patterns as `i64` and subtracting gives
exactly the count of representable values between them. Values of opposite
sign are handled by mapping each to a monotone key first. Boost documents
the same construction.

# Returns

`b - a` measured in ULPs, positive when `b > a`. Returns `0.0` when either
argument is NaN, which matches how the two call sites behave: a NaN
quantity there is already a bug upstream of this comparison, and returning
`0` makes it surface as a rejected match rather than a panic.

# Examples

```
use kaki_bukit::limits::float_distance;

assert_eq!(float_distance(1.0, 1.0), 0.0);
assert_eq!(float_distance(1.0, f64::from_bits(1.0f64.to_bits() + 1)), 1.0);
assert_eq!(float_distance(f64::from_bits(1.0f64.to_bits() + 1), 1.0), -1.0);
```

```rust
pub fn float_distance(a: f64, b: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `EPS`

Generic epsilon. Upstream `cyclus::eps()`, default `1e-6`.

Used for comparing dimensionless quantities — preferences, capacities,
exchange-graph flows.

```rust
pub const EPS: f64 = 1e-6;
```

#### Constant `EPS_RSRC`

Resource epsilon. Upstream `cyclus::eps_rsrc()`, default `1e-6` kg.

Used for comparing resource quantities in kilograms. It is a separate
constant from [`EPS`] upstream — and stays separate here — because the two
are dimensionally different things that merely happen to share a value, and
collapsing them would hide that from anyone who later wants to tighten one.

```rust
pub const EPS_RSRC: f64 = 1e-6;
```

#### Constant `FLOAT_ULP_EQ`

Distance in ULPs within which two floats count as equal.

Upstream `float_ulp_eq = 2`. This governs the exclusive-order arithmetic in
the exchange solver, where upstream's comment is emphatic that the careful
float comparison "is vital for preventing false positive constraint
violations w.r.t. exclusivity-related capacity". See
[`float_distance`](crate::limits::float_distance).

```rust
pub const FLOAT_ULP_EQ: f64 = 2.0;
```

#### Constant `MODIFIER_LIMIT`

Maximum value for a function modifier (upstream `kModifierLimit`, `1e10`).

```rust
pub const MODIFIER_LIMIT: f64 = 1e10;
```

#### Constant `CY_LARGE_INT`

Upstream `CY_LARGE_INT`. A stand-in for "unbounded" in integer-valued
capacities; upstream sets it from CMake to the largest `int` it can use
without overflow in downstream arithmetic.

```rust
pub const CY_LARGE_INT: i32 = i32::MAX;
```

#### Constant `CY_LARGE_DOUBLE`

Upstream `CY_LARGE_DOUBLE`. A stand-in for "unbounded" in real-valued
capacities.

Note that upstream's greedy solver *also* compares group capacities against
`std::numeric_limits<double>::max()` directly, as its special case for an
unlimited capacity. [`UNLIMITED`] is that value; the two are deliberately
distinct and must not be merged.

```rust
pub const CY_LARGE_DOUBLE: f64 = 1e299;
```

#### Constant `CY_NEAR_ZERO`

Upstream `CY_NEAR_ZERO`.

```rust
pub const CY_NEAR_ZERO: f64 = 1e-8;
```

#### Constant `UNLIMITED`

The sentinel an exchange capacity carries to mean "unbounded".

This is `std::numeric_limits<double>::max()`, which the greedy solver tests
for by exact equality in both [`capacity`](crate::exchange::greedy) and its
capacity-update step. Exact-equality comparison of a float is normally a
defect; here it is upstream's deliberate sentinel protocol and is preserved
as such.

```rust
pub const UNLIMITED: f64 = f64::MAX;
```

## Module `material`

Material: a mass of nuclides with a composition.

[`Material`] is the resource almost everything in a fuel cycle moves
around. It is a **quantity in kilograms** plus a
[`Composition`](crate::composition::Composition), and its whole interface
is four operations that conserve mass between them:

| Operation | Effect |
|---|---|
| [`extract_qty`](Material::extract_qty) | split off `qty` kg of the same composition |
| [`extract_comp`](Material::extract_comp) | split off `qty` kg of a *different* composition, leaving the remainder |
| [`absorb`](Material::absorb) | merge another material in, mass-weighting the compositions |
| [`transmute`](Material::transmute) | replace the composition, keeping the mass |

Upstream's units are kilograms by fiat — `Material::units()` returns the
literal string `"kg"` — and this translation keeps that rather than
reaching for `uom`. See the note on the `petir` dependency in `Cargo.toml`
for why.

```rust
pub mod material { /* ... */ }
```

### Types

#### Struct `Material`

A quantity of material, in kilograms, with a nuclide composition.

# Tracking

Upstream threads a `ResTracker` through every operation to write a resource
genealogy to the output database. There is no database in this `no_std`
kernel, so no tracker: a [`Material`] here is the physical object only.
`prev_decay_time` is kept, because it is not bookkeeping — it is the state
[`absorb`](Material::absorb) needs to refuse to merge materials that have
been decayed to different times.

```rust
pub struct Material {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(qty: f64, comp: Composition) -> Result<Self> { /* ... */ }
  ```
  Creates `qty` kilograms of material with composition `comp`.

- ```rust
  pub fn with_unit_value(qty: f64, comp: Composition, unit_value: f64) -> Result<Self> { /* ... */ }
  ```
  Creates `qty` kilograms with composition `comp` and a per-unit economic

- ```rust
  pub fn quantity(self: &Self) -> f64 { /* ... */ }
  ```
  The mass in kilograms. Upstream `Material::quantity()`.

- ```rust
  pub fn units(self: &Self) -> &''static str { /* ... */ }
  ```
  The units of [`quantity`](Material::quantity), always `"kg"`.

- ```rust
  pub fn comp(self: &Self) -> &Composition { /* ... */ }
  ```
  The composition. Upstream `Material::comp()`.

- ```rust
  pub fn unit_value(self: &Self) -> f64 { /* ... */ }
  ```
  The per-unit economic value. Upstream `Resource::unit_value()`.

- ```rust
  pub fn set_unit_value(self: &mut Self, v: f64) { /* ... */ }
  ```
  Sets the per-unit economic value.

- ```rust
  pub fn prev_decay_time(self: &Self) -> i64 { /* ... */ }
  ```
  The simulation time this material was last decayed to.

- ```rust
  pub fn set_prev_decay_time(self: &mut Self, t: i64) { /* ... */ }
  ```
  Sets the last-decayed time. Called by [`decay`](crate::decay).

- ```rust
  pub fn nuclide_masses(self: &Self) -> CompMap { /* ... */ }
  ```
  The **mass** of each nuclide, in kilograms: the composition scaled to

- ```rust
  pub fn extract_qty(self: &mut Self, qty: f64) -> Result<Self> { /* ... */ }
  ```
  Splits `qty` kilograms off this material, keeping the same composition.

- ```rust
  pub fn extract_comp(self: &mut Self, qty: f64, c: &Composition, threshold: f64, masses: &AtomicMasses) -> Result<Self> { /* ... */ }
  ```
  Splits `qty` kilograms of composition `c` off this material, leaving

- ```rust
  pub fn absorb(self: &mut Self, other: Self, masses: &AtomicMasses) -> Result<()> { /* ... */ }
  ```
  Merges `other` into this material. Upstream `Material::Absorb`.

- ```rust
  pub fn transmute(self: &mut Self, c: Composition, now: i64) { /* ... */ }
  ```
  Replaces the composition, keeping the mass. Upstream

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` if the quantity is at or below the resource epsilon.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Material { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: Material) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(m: Material) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Material) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `DEFAULT_THRESHOLD`

The default extraction threshold. Upstream `Material::kDefaultThreshold`.

Quantities at or below this are cleared from the remainder composition
after an [`extract_comp`](Material::extract_comp), so a separation that
removes "all" of a nuclide does not leave `1e-30` kg of it behind to be
carried for the rest of the simulation.

```rust
pub const DEFAULT_THRESHOLD: f64 = 1e-14;
```

## Module `nuclide`

Nuclide identifiers in PyNE `zzzaaammmm` form.

# The identifier format

A nuclide is one `i32` packing three fields:

```text
    Z Z Z A A A M M M M
    \_____/ \___/ \_____/
     proton   mass  metastable
     number  number   state
```

so U-235 in its ground state is `92_235_0000` = `922350000`, and the first
metastable state of Am-242 is `95_242_0001` = `952420001`. Uranium as a
natural element, with no mass number, is `920000000`.

# Scope of this translation

PyNE's `nucname` also parses and prints roughly a dozen other conventions
(`zzaaam`, MCNP, Serpent, NIST, CINDER, ALARA, SZA). **None of those are
ported**, because Cyclus itself only ever calls `id`, `isnuclide`, `znum`,
`anum`, `snum` and `name`, and the remaining converters exist to talk to
specific external codes that this crate does not talk to. Porting them
speculatively would add several hundred lines of lookup tables with no
caller to exercise them.

What *is* ported is exact: [`Nuc::is_nuclide`] reproduces
`pyne::nucname::isnuclide(int)` condition for condition, including the
`aaa <= zzz * 7` physicality bound that upstream applies in `id()`.

```rust
pub mod nuclide { /* ... */ }
```

### Modules

## Module `nuc`

Nuclides this crate names directly, for readability at call sites.

Upstream writes these as bare integer literals throughout (`922350000`
appears eleven times in the enrichment toolkit alone). Naming them is the
"human interface layer" rule applied to a magic number: a reader hovering
over `nuc::U235` learns more than one hovering over `922350000`.

```rust
pub mod nuc { /* ... */ }
```

### Constants and Statics

#### Constant `U235`

U-235, ground state.

```rust
pub const U235: super::Nuc = _;
```

#### Constant `U238`

U-238, ground state.

```rust
pub const U238: super::Nuc = _;
```

#### Constant `U234`

U-234, ground state — the minor feed isotope tracked by enrichment.

```rust
pub const U234: super::Nuc = _;
```

#### Constant `PU239`

Pu-239, ground state.

```rust
pub const PU239: super::Nuc = _;
```

#### Constant `PU241`

Pu-241, ground state.

```rust
pub const PU241: super::Nuc = _;
```

#### Constant `O16`

O-16, ground state — the oxygen in an oxide fuel.

```rust
pub const O16: super::Nuc = _;
```

#### Constant `U_NATURAL`

Natural uranium as an element (no mass number).

```rust
pub const U_NATURAL: super::Nuc = _;
```

### Types

#### Struct `Nuc`

A nuclide identifier in PyNE `zzzaaammmm` form.

# Validity

The wrapper does **not** enforce validity on construction, and that is
deliberate rather than an oversight: upstream's `CompMap` is keyed on a
bare `int`, and compositions read from a file routinely contain ids that
have to be *checked and reported*, not rejected at the point of parsing.
[`Nuc::new`] is therefore infallible and [`Nuc::is_nuclide`] is the check,
mirroring `compmath::ValidNucs`. Use [`Nuc::validated`] where an invalid id
should be an error at construction.

```rust
pub struct Nuc(pub i32);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `i32` |  |

##### Implementations

###### Methods

- ```rust
  pub const fn new(id: i32) -> Self { /* ... */ }
  ```
  Wraps a raw `zzzaaammmm` identifier without checking it.

- ```rust
  pub fn validated(id: i32) -> Result<Self> { /* ... */ }
  ```
  Wraps a raw identifier, returning an error if it is not a nuclide.

- ```rust
  pub fn from_zam(z: i32, a: i32, m: i32) -> Result<Self> { /* ... */ }
  ```
  Builds an identifier from proton number, mass number and metastable

- ```rust
  pub const fn raw(self: Self) -> i32 { /* ... */ }
  ```
  The raw `zzzaaammmm` identifier.

- ```rust
  pub const fn z(self: Self) -> i32 { /* ... */ }
  ```
  Proton number `Z`. Upstream `pyne::nucname::znum`.

- ```rust
  pub const fn a(self: Self) -> i32 { /* ... */ }
  ```
  Mass number `A`. Upstream `pyne::nucname::anum`.

- ```rust
  pub const fn m(self: Self) -> i32 { /* ... */ }
  ```
  Metastable state. Upstream `pyne::nucname::snum`. `0` is the ground

- ```rust
  pub const fn n(self: Self) -> i32 { /* ... */ }
  ```
  Neutron number `N = A - Z`.

- ```rust
  pub const fn is_nuclide(self: Self) -> bool { /* ... */ }
  ```
  `true` if this is a specific nuclide. Upstream

- ```rust
  pub const fn is_element(self: Self) -> bool { /* ... */ }
  ```
  `true` if this id names a natural element rather than a nuclide

- ```rust
  pub fn symbol(self: Self) -> Option<&''static str> { /* ... */ }
  ```
  The IUPAC element symbol for this nuclide's proton number, e.g. `"U"`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Nuc { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut fmt::Formatter<''_>) -> fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Nuc) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Nuc) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Nuc) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `element_symbol`

**Attributes:**

- `MustUse { reason: None }`

The IUPAC symbol for proton number `z`, or `None` outside 1..=118.

```rust
pub fn element_symbol(z: i32) -> Option<&''static str> { /* ... */ }
```

## Module `product`

Product: a traded resource that is not made of nuclides.

Electricity, separative work units, money, cooling water. A [`Product`] has
a quantity and a **quality** — an opaque string that plays the role a
composition plays for [`Material`](crate::material::Material): two products
may only be combined if their qualities match.

```rust
pub mod product { /* ... */ }
```

### Types

#### Struct `Product`

A quantity of some non-nuclide resource.

Units are whatever the quality implies — upstream's `Product::units()`
returns the quality string itself, and that convention is kept here.

```rust
pub struct Product {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(qty: f64, quality: &str) -> Result<Self> { /* ... */ }
  ```
  Creates `qty` units of a product of the given `quality`.

- ```rust
  pub fn quantity(self: &Self) -> f64 { /* ... */ }
  ```
  The quantity, in whatever units [`quality`](Product::quality) implies.

- ```rust
  pub fn quality(self: &Self) -> &str { /* ... */ }
  ```
  The quality string. Upstream `Product::quality()`.

- ```rust
  pub fn units(self: &Self) -> &str { /* ... */ }
  ```
  The units, which upstream defines to be the quality string.

- ```rust
  pub fn unit_value(self: &Self) -> f64 { /* ... */ }
  ```
  The per-unit economic value.

- ```rust
  pub fn set_unit_value(self: &mut Self, v: f64) { /* ... */ }
  ```
  Sets the per-unit economic value.

- ```rust
  pub fn extract_qty(self: &mut Self, qty: f64) -> Result<Self> { /* ... */ }
  ```
  Splits `qty` units off this product. Upstream `Product::ExtractQty`.

- ```rust
  pub fn absorb(self: &mut Self, other: Self) -> Result<()> { /* ... */ }
  ```
  Merges `other` into this product. Upstream `Product::Absorb`.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` if the quantity is at or below the resource epsilon.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Product { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(p: Product) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Product) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `resource`

The traded-resource enum.

Upstream's `Resource` is an abstract base class with two concrete
subclasses, [`Material`] and [`Product`], and everything in the exchange
layer holds a `Resource::Ptr`. This workspace forbids trait objects, so the
polymorphism becomes an enum and dispatch becomes a `match`.

That substitution is not a compromise here — it is a better fit. The set of
resource types is closed and has been closed since Cyclus 1.0; upstream's
own `ResourceType` is a string compared against two constants. An enum
makes the exhaustiveness a compile error instead of a runtime string
comparison, and lets a third resource type be added without any call site
silently ignoring it.

```rust
pub mod resource { /* ... */ }
```

### Types

#### Enum `Resource`

A traded resource: either a [`Material`] or a [`Product`].

```rust
pub enum Resource {
    Material(crate::material::Material),
    Product(crate::product::Product),
}
```

##### Variants

###### `Material`

A mass of nuclides. Upstream `ResourceType == "Material"`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::material::Material` |  |

###### `Product`

A non-nuclide resource with a quality string. Upstream
`ResourceType == "Product"`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::product::Product` |  |

##### Implementations

###### Methods

- ```rust
  pub fn resource_type(self: &Self) -> ResourceType { /* ... */ }
  ```
  Which concrete resource this is. Upstream `Resource::type()`.

- ```rust
  pub fn quantity(self: &Self) -> f64 { /* ... */ }
  ```
  The quantity — kilograms for a material, quality-defined units for a

- ```rust
  pub fn units(self: &Self) -> &str { /* ... */ }
  ```
  The units of [`quantity`](Resource::quantity). Upstream

- ```rust
  pub fn unit_value(self: &Self) -> f64 { /* ... */ }
  ```
  The per-unit economic value. Upstream `Resource::unit_value()`.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` if the quantity is at or below the resource epsilon.

- ```rust
  pub fn extract_qty(self: &mut Self, qty: f64) -> Result<Self> { /* ... */ }
  ```
  Splits `qty` off this resource. Upstream `Resource::ExtractRes`.

- ```rust
  pub fn as_material(self: &Self) -> Option<&Material> { /* ... */ }
  ```
  Borrows the inner material, or `None` if this is a product.

- ```rust
  pub fn as_material_mut(self: &mut Self) -> Option<&mut Material> { /* ... */ }
  ```
  Mutably borrows the inner material, or `None` if this is a product.

- ```rust
  pub fn as_product(self: &Self) -> Option<&Product> { /* ... */ }
  ```
  Borrows the inner product, or `None` if this is a material.

- ```rust
  pub fn into_material(self: Self) -> Result<Material> { /* ... */ }
  ```
  Consumes this resource and returns the inner material.

- ```rust
  pub fn into_product(self: Self) -> Result<Product> { /* ... */ }
  ```
  Consumes this resource and returns the inner product.

- ```rust
  pub fn quality_tag(self: &Self) -> Option<String> { /* ... */ }
  ```
  The quality identifier used to decide whether two resources may be

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Resource { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: Material) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(p: Product) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Resource) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ResourceType`

Which concrete resource a [`Resource`] holds. Upstream `ResourceType`,
which is a `std::string`.

```rust
pub enum ResourceType {
    Material,
    Product,
}
```

##### Variants

###### `Material`

[`Resource::Material`].

###### `Product`

[`Resource::Product`].

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: Self) -> &''static str { /* ... */ }
  ```
  The upstream `ResourceType` string, for output compatible with

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ResourceType { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &ResourceType) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ResourceType) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &ResourceType) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `sim`

Simulation configuration and the time step clock.

# The time step

A Cyclus time step is **one month**, and "one month" is a fixed number of
seconds rather than a calendar month — [`SECONDS_PER_MONTH`]. Every month
in a simulation is therefore exactly the same length, which is what makes
a decay over `n` steps depend only on `n`.

# The phases of a time step

Each step runs five phases in a fixed order, and the order is the whole
protocol between agents. [`Phase`] names them; the loop that drives them is
[`Simulation::step`](crate::agents::Simulation::step).

1. [`Phase::Build`] — agents scheduled to be built enter the simulation.
2. [`Phase::Tick`] — every agent updates its own state and decides what it
   will want. Nothing is traded yet.
3. [`Phase::Exchange`] — the dynamic resource exchange runs: requests,
   bids, solve, trades. See [`exchange`](crate::exchange).
4. [`Phase::Tock`] — agents process what they received.
5. [`Phase::Decision`] — agents make end-of-step decisions (whether to
   decommission, whether to request a new build).

Upstream then runs a decommission sweep and increments the clock. The split
matters: an agent that traded in phase 3 may only act on the result in
phase 4, which is what stops the outcome from depending on the order agents
happen to be stored in.

```rust
pub mod sim { /* ... */ }
```

### Types

#### Enum `DecayMode`

Whether materials may decay during the simulation. Upstream's
`SimInfo::decay`, which is a `std::string` holding `"never"`, `"manual"` or
`"lazy"`.

An enum rather than a string, because the set is closed and a typo in a
string is a silent no-decay simulation — a failure mode worth making
impossible.

```rust
pub enum DecayMode {
    Never,
    Manual,
    Lazy,
}
```

##### Variants

###### `Never`

Decay is never applied. Upstream `"never"`.

###### `Manual`

Decay is applied only when an agent explicitly asks for it. Upstream
`"manual"`. This is the mode this crate supports.

###### `Lazy`

Upstream `"lazy"`: decay is applied implicitly whenever a composition
is read, memoised along a shared decay chain.

**Not implemented here.** The memo is a `shared_ptr` graph of exactly
the kind the workspace's Rust rules exclude, and "decay happens when
you look at it" makes a result depend on which accessors a caller
happened to invoke. Selecting it is accepted so a configuration can
round-trip, but [`SimInfo::validate`] rejects it rather than silently
behaving like [`DecayMode::Manual`].

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: Self) -> &''static str { /* ... */ }
  ```
  The upstream string form, for round-tripping a configuration.

- ```rust
  pub fn parse(s: &str) -> Result<Self> { /* ... */ }
  ```
  Parses the upstream string form.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> DecayMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> DecayMode { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DecayMode) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SimInfo`

Simulation-wide configuration. Upstream `SimInfo`.

# Fields not carried over

Upstream also holds `parent_sim` (a `boost::uuids::uuid`), `parent_type`
and `branch_time`, which exist to record a simulation's provenance in the
output database when one run branches from another. There is no output
database in this `no_std` kernel, so they are omitted rather than kept as
dead weight. `explicit_inventory` and `explicit_inventory_compact` are
omitted for the same reason — both select output tables.

```rust
pub struct SimInfo {
    pub handle: alloc::string::String,
    pub decay: DecayMode,
    pub duration: i64,
    pub y0: i32,
    pub m0: i32,
    pub dt: u64,
    pub eps: f64,
    pub eps_rsrc: f64,
    pub seed: u64,
    pub stride: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `handle` | `alloc::string::String` | A user-defined label for this simulation. |
| `decay` | `DecayMode` | Whether and how materials decay. |
| `duration` | `i64` | Length of the simulation, in time steps (months). Must be positive. |
| `y0` | `i32` | Start year, e.g. 1973. |
| `m0` | `i32` | Start month, 1 (January) through 12 (December). |
| `dt` | `u64` | Duration of one time step, in seconds. Defaults to<br>[`DEFAULT_TIME_STEP_DUR`]. |
| `eps` | `f64` | Generic tolerance for this simulation. Defaults to<br>[`crate::limits::EPS`]. |
| `eps_rsrc` | `f64` | Resource tolerance, in kilograms. Defaults to<br>[`crate::limits::EPS_RSRC`]. |
| `seed` | `u64` | Seed for a random number generator, for agents that need one. |
| `stride` | `u64` | Stride length for a random number generator. Unused by this crate;<br>carried because upstream carries it. |

##### Implementations

###### Methods

- ```rust
  pub fn new(duration: i64) -> Self { /* ... */ }
  ```
  A simulation of `duration` time steps starting in January 2010.

- ```rust
  pub fn starting(duration: i64, y0: i32, m0: i32) -> Self { /* ... */ }
  ```
  A simulation of `duration` time steps starting at `y0`/`m0`.

- ```rust
  pub fn with_handle(self: Self, handle: &str) -> Self { /* ... */ }
  ```
  Sets the user-defined handle, consuming and returning `self`.

- ```rust
  pub fn with_decay(self: Self, decay: DecayMode) -> Self { /* ... */ }
  ```
  Sets the decay mode, consuming and returning `self`.

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Checks that this configuration can actually be run.

- ```rust
  pub fn calendar_at(self: &Self, t: i64) -> (i32, i32) { /* ... */ }
  ```
  The calendar year and month at time step `t`.

- ```rust
  pub fn steps_until(self: &Self, year: i32, month: i32) -> i64 { /* ... */ }
  ```
  The number of time steps from the simulation start to `year`/`month`.

- ```rust
  pub fn seconds_for(self: &Self, n: i64) -> f64 { /* ... */ }
  ```
  The elapsed seconds represented by `n` time steps.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SimInfo { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    Upstream's `SimInfo()` default: one time step, starting January 2010,

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SimInfo) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Phase`

The phases of a single time step, in the order they run.

Upstream has no such enum — the phases are five method calls in
`Timer::RunSim`. Naming them makes the protocol inspectable: a test can
assert that a step visited the phases in order, and an agent's
documentation can say which phase it acts in without describing a call
site.

```rust
pub enum Phase {
    Build,
    Tick,
    Exchange,
    Tock,
    Decision,
    Decommission,
}
```

##### Variants

###### `Build`

Scheduled agents enter the simulation. Upstream `Timer::DoBuild`.

###### `Tick`

Agents update their own state and decide what they want. Upstream
`Timer::DoTick`.

###### `Exchange`

The dynamic resource exchange runs. Upstream `Timer::DoResEx`.

###### `Tock`

Agents process what they received. Upstream `Timer::DoTock`.

###### `Decision`

End-of-step decisions. Upstream `Timer::DoDecision`.

###### `Decommission`

Agents scheduled for decommissioning leave. Upstream `Timer::DoDecom`.

##### Implementations

###### Methods

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Phase { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Phase) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Phase) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Phase) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Timer`

The simulation clock. Upstream `Timer`, minus the output-recording and
progress-bar machinery.

This type owns the time step counter and the early-termination flag, and
nothing else. Upstream's `Timer` also owns the agent registry and the
build/decommission queues; here those belong to
[`Context`](crate::context::Context), because an agent registry that lives
on the clock makes the clock impossible to test on its own.

```rust
pub struct Timer {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(info: SimInfo) -> Result<Self> { /* ... */ }
  ```
  Creates a clock for `info`, starting at time step 0.

- ```rust
  pub fn time(self: &Self) -> i64 { /* ... */ }
  ```
  The current time step. Upstream `Timer::time()`.

- ```rust
  pub fn duration(self: &Self) -> i64 { /* ... */ }
  ```
  The simulation duration in time steps. Upstream `Timer::dur()`.

- ```rust
  pub fn info(self: &Self) -> &SimInfo { /* ... */ }
  ```
  The simulation configuration.

- ```rust
  pub fn running(self: &Self) -> bool { /* ... */ }
  ```
  `true` while there are steps left and the simulation has not been

- ```rust
  pub fn kill(self: &mut Self) { /* ... */ }
  ```
  Requests early termination. Upstream `Timer::KillSim`.

- ```rust
  pub fn killed(self: &Self) -> bool { /* ... */ }
  ```
  `true` if [`kill`](Timer::kill) was called.

- ```rust
  pub fn advance(self: &mut Self) { /* ... */ }
  ```
  Advances the clock by one step. Upstream's `time_++`.

- ```rust
  pub fn calendar(self: &Self) -> (i32, i32) { /* ... */ }
  ```
  The calendar year and month of the current step.

- ```rust
  pub fn reset(self: &mut Self) { /* ... */ }
  ```
  Resets the clock to time step 0 and clears the kill flag. Upstream

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Timer { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Timer) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `SECONDS_PER_YEAR`

Seconds in a Cyclus year. Upstream `cyclusYear = 31558200`.

That is 365.2569 days, not the Julian year's 365.25 — upstream's value, and
preserved exactly, because a decay computed over a different year length
would not reproduce an upstream result.

```rust
pub const SECONDS_PER_YEAR: u64 = 31_558_200;
```

#### Constant `MONTHS_PER_YEAR`

Months in a year. Upstream `kMonthsPerYear`.

```rust
pub const MONTHS_PER_YEAR: u64 = 12;
```

#### Constant `SECONDS_PER_MONTH`

Seconds in one time step. Upstream `cyclusMonth = cyclusYear / 12`, which
is `2_629_850` seconds exactly (integer division).

```rust
pub const SECONDS_PER_MONTH: u64 = _;
```

#### Constant `DEFAULT_TIME_STEP_DUR`

The default duration of one time step, in seconds. Upstream
`kDefaultTimeStepDur`.

```rust
pub const DEFAULT_TIME_STEP_DUR: u64 = SECONDS_PER_MONTH;
```

#### Constant `DEFAULT_SEED`

Upstream `kDefaultSeed`.

```rust
pub const DEFAULT_SEED: u64 = 20_160_212;
```

#### Constant `DEFAULT_STRIDE`

Upstream `kDefaultStride`.

```rust
pub const DEFAULT_STRIDE: u64 = 10_000;
```

## Module `toolkit`

Reusable facility building blocks: the pieces every fuel-cycle archetype
assembles itself from.

Upstream's `src/toolkit/` is where Cyclus keeps the things that are not the
kernel but that every archetype needs anyway — an inventory buffer, a
material inspector, the separative-work arithmetic, a commodity registry, a
demand curve. None of it is simulation machinery; all of it is the
vocabulary a facility is written in.

# What is here

| Module | Upstream | What it is for |
|---|---|---|
| [`mat_query`] | `mat_query.{h,cc}` | Read masses, moles and fractions out of a [`Material`](crate::material::Material) |
| [`enrichment`] | `enrichment.{h,cc}` | Assays, feed/tails mass balance, SWU |
| [`res_buf`] | `res_buf.h` | The inventory buffer every facility holds its stock in |
| [`total_inv_tracker`] | `total_inv_tracker.h` | A facility-wide cap across several buffers |
| [`commodity`] | `commodity.{h,cc}`, `commodity_producer.{h,cc}` | Named commodities, and who produces them at what capacity and cost |
| [`symbolic`] | `symbolic_functions.{h,cc}` | Linear / exponential / piecewise demand curves |
| [`position`] | `position.{h,cc}` | Latitude, longitude and great-circle distance |

# What is NOT here, and what comes next

**`matl_buy_policy` and `matl_sell_policy` are the intended next step.**
They are the two pieces that turn a [`ResBuf`](res_buf::ResBuf) into a
participant in the dynamic resource exchange — a buy policy posts requests
to fill a buffer, a sell policy posts bids to drain one — and they are
deliberately absent here because both need the `Agent`/`Context` framework
(`Agent::context()`, `Context::NewDatum`, trade callbacks) that is being
built separately. Once an agent can be handed a context, those two files
are the obvious next port and they slot straight onto [`res_buf`] and
[`commodity`].

Also not ported, with reasons:

| Upstream | Why not |
|---|---|
| `res_manip.{h,cc}` (`Squash`, `ResCast`) | `ResCast` is C++ pointer down-casting that the [`Resource`](crate::resource::Resource) enum makes unnecessary. `Squash` needs [`AtomicMasses`](crate::composition::AtomicMasses) to absorb materials, which upstream reaches through a global PyNE table; see [`res_buf`] for how the split-pop path works without it. |
| `building_manager`, `supply_demand_manager`, `commodity_producer_manager` | All are `Agent`-framework managers. |
| `symbolic_function_factories` | XML-input parsing for the function types in [`symbolic`]. There is no XML layer in this kernel; build the enums directly. |
| `timeseries`, `infile_converters`, `*.cycpp.h` | Output recording and the C++ preprocessor's generated state, neither of which exists here. |
| `agent_managed`, `commodity_recipe_context` | `Agent` framework. |

# Translation rules, applied here

The workspace forbids trait objects, `Box` and lifetime parameters on
structs. Each of those appears in this directory upstream, and the
substitution is the same one the crate root documents:

| Upstream C++ | Here |
|---|---|
| `boost::shared_ptr<SymFunction>` hierarchy | [`SymFunction`](symbolic::SymFunction), an enum matched at each call |
| `ResBuf<T>` template over `Material`/`Product` | [`ResBuf`](res_buf::ResBuf) holding the [`Resource`](crate::resource::Resource) enum |
| `std::list<Resource::Ptr>` with a `std::set` duplicate guard | a `VecDeque<Resource>`; ownership makes a duplicate push unrepresentable |
| `std::vector<ResBuf<Material>*>` | a borrowed slice passed per call |
| thrown `ValueError` | [`Result<T>`](crate::error::Result) |

```rust
pub mod toolkit { /* ... */ }
```

### Modules

## Module `commodity`

Named commodities, and who produces them at what capacity and cost.

A **commodity** is the label a fuel-cycle market trades under — `"natural_u"`,
`"enriched_u"`, `"spent_fuel"`, `"power"`. It is a name and nothing else:
upstream's own comment calls the class "currently super simple" and explains
that it exists rather than a bare `std::string` only so the code reads
better and so the type can grow later. That reasoning carries over exactly,
and a newtype costs nothing here.

A [`CommodityProducer`] is the registry an agent keeps of what it makes: for
each commodity, a **production capacity** and a **production cost**.

# Units

| Quantity | Units |
|---|---|
| [`CommodInfo::capacity`] | production capacity **per time step**, in the commodity's own units — kilograms for a material commodity, megawatt-hours or similar for a product one. Upstream fixes no unit and neither does this. |
| [`CommodInfo::cost`] | cost per unit of that commodity, dimensionless to the kernel. Defaults to [`MODIFIER_LIMIT`] (`1e10`), upstream's "effectively infinite" sentinel, so an unpriced commodity is never chosen by a cost-minimising decision. |

# Divergences from upstream

Upstream's `CommodityProducer` inherits `AgentManaged`, holding a back
pointer to the agent that owns it. There is no agent framework here yet, so
that link is absent; a facility owns a [`CommodityProducer`] by value.

Two smaller behavioural differences are called out on the methods
themselves, because both are easy to trip over when reading the two files
side by side: [`CommodityProducer::add`] does not overwrite (matching
`std::map::insert`, which is not obvious), and the queries do **not** insert
a default entry the way upstream's `operator[]` silently does.

[`MODIFIER_LIMIT`]: crate::limits::MODIFIER_LIMIT

```rust
pub mod commodity { /* ... */ }
```

### Types

#### Struct `Commodity`

A named commodity traded in the fuel cycle. Upstream `Commodity`.

Ordered by name, so a [`CommodityProducer`]'s registry iterates
deterministically — the translation of upstream's `CommodityCompare`
functor, which exists for exactly that purpose ("we do not care how they
are compared, only that they can be").

```rust
pub struct Commodity {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(name: &str) -> Self { /* ... */ }
  ```
  A commodity with the given name. Upstream `Commodity(std::string)`.

- ```rust
  pub fn name(self: &Self) -> &str { /* ... */ }
  ```
  The commodity's name. Upstream `Commodity::name()`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Commodity { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Commodity { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(s: &str) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(name: String) -> Self { /* ... */ }
    ```

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Commodity) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Commodity) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Commodity) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CommodInfo`

What a producer can make of one commodity, and at what price. Upstream
`CommodInfo`.

See the [module docs](self) for the units, which the kernel does not fix.

```rust
pub struct CommodInfo {
    pub capacity: f64,
    pub cost: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `capacity` | `f64` | Production capacity per time step, in the commodity's own units.<br>Non-negative in any sensible configuration; not checked, as upstream<br>does not check it either. |
| `cost` | `f64` | Production cost per unit. Defaults to<br>[`MODIFIER_LIMIT`](crate::limits::MODIFIER_LIMIT). |

##### Implementations

###### Methods

- ```rust
  pub fn new(capacity: f64, cost: f64) -> Self { /* ... */ }
  ```
  A capacity/cost pair. Upstream `CommodInfo(capacity, cost)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CommodInfo { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    Upstream's defaults: zero capacity, cost

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CommodInfo) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CommodityProducer`

A registry of the commodities an agent produces. Upstream
`CommodityProducer`.

Held by value on the producing facility. The registry is ordered by
commodity name, so [`produced_commodities`](CommodityProducer::produced_commodities)
and any iteration over it reproduce run to run.

# Example

```
use kaki_bukit::toolkit::commodity::{Commodity, CommodityProducer};

let mut mine = CommodityProducer::new();
let natu = Commodity::new("natural_u");
mine.add(&natu);
mine.set_capacity(&natu, 2.5e5);   // kg per time step
mine.set_cost(&natu, 40.0);        // per kg

assert!(mine.produces(&natu));
assert_eq!(mine.capacity(&natu), 2.5e5);
```

```rust
pub struct CommodityProducer {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  A producer registering nothing, with upstream's defaults for any

- ```rust
  pub fn with_defaults(default_capacity: f64, default_cost: f64) -> Self { /* ... */ }
  ```
  A producer whose [`add`](CommodityProducer::add) uses the given

- ```rust
  pub fn produces(self: &Self, commodity: &Commodity) -> bool { /* ... */ }
  ```
  `true` if `commodity` is registered. Upstream `Produces()`.

- ```rust
  pub fn capacity(self: &Self, commodity: &Commodity) -> f64 { /* ... */ }
  ```
  The registered production capacity per time step, in the commodity's

- ```rust
  pub fn cost(self: &Self, commodity: &Commodity) -> f64 { /* ... */ }
  ```
  The registered production cost per unit. Upstream `Cost()`.

- ```rust
  pub fn info(self: &Self, commodity: &Commodity) -> Option<&CommodInfo> { /* ... */ }
  ```
  The full record for a commodity, or `None` if it is not registered.

- ```rust
  pub fn set_capacity(self: &mut Self, commodity: &Commodity, capacity: f64) { /* ... */ }
  ```
  Sets the production capacity per time step. Upstream `SetCapacity()`.

- ```rust
  pub fn set_cost(self: &mut Self, commodity: &Commodity, cost: f64) { /* ... */ }
  ```
  Sets the production cost per unit. Upstream `SetCost()`.

- ```rust
  pub fn add(self: &mut Self, commodity: &Commodity) { /* ... */ }
  ```
  Registers `commodity` at this producer's defaults. Upstream

- ```rust
  pub fn add_with(self: &mut Self, commodity: &Commodity, info: CommodInfo) { /* ... */ }
  ```
  Registers `commodity` with an explicit record. Upstream

- ```rust
  pub fn rm(self: &mut Self, commodity: &Commodity) { /* ... */ }
  ```
  Unregisters `commodity`. Upstream `Rm()`. A no-op if it was not

- ```rust
  pub fn produced_commodities(self: &Self) -> Vec<Commodity> { /* ... */ }
  ```
  Every registered commodity, in ascending name order. Upstream

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  The number of registered commodities.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` if nothing is registered.

- ```rust
  pub fn copy(self: &mut Self, source: &Self) { /* ... */ }
  ```
  Copies every commodity produced by `source`, with its capacity and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CommodityProducer { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CommodityProducer) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `enrichment`

Uranium enrichment: assays, the feed/tails mass balance, and separative
work.

This is the arithmetic behind an enrichment facility. Given three assays —
what comes in, what goes out as product, what goes out as tails — it answers
two questions: **how much feed** is needed per unit of product, and **how
much separative work** the separation costs.

# The two-isotope idealisation

Every function here treats uranium as a **binary mixture of U-235 and
U-238**. U-234 and U-236 are real and are ignored, which is upstream's
choice and the standard textbook one; the error it introduces in the SWU is
below the uncertainty in any plant-level cascade model. A material that
contains other nuclides is not rejected — [`uranium_assay_atom`] simply
ignores everything that is not U-235 or U-238, so the assay of a UO2 pellet
is the assay of its uranium, not of the pellet.

# Units

| Quantity | Units |
|---|---|
| assay (feed, product, tails) | dimensionless fraction in the **open** interval `(0, 1)` — 0.711 % natural uranium is `0.00711`, not `0.711` |
| `product_qty`, [`feed_qty`], [`tails_qty`] | kilograms (or any mass unit, consistently — the relations are homogeneous of degree one) |
| [`swu_required`] | **SWU**, separative work units, dimensionally a mass |

**An assay is an atom fraction unless the caller has chosen otherwise.**
Upstream's `UraniumAssay` is an alias for [`uranium_assay_atom`], and
[`feed_qty`] / [`tails_qty`] / [`swu_required`] are basis-agnostic: they are
correct in either basis as long as `product_qty` and the three assays share
one. Mixing a mass assay with an atom quantity is the classic error here,
and nothing in the types can catch it — upstream's own header says only
"product_qty and assay must share the same units".

# The value function

The separative potential of a stream at assay `f` is

`V(f) = (1 - 2f) * ln(1/f - 1)`

and the separative work to make `P` of product from `F` of feed leaving `T`
of tails is `P*V(xp) + T*V(xt) - F*V(xf)`. `V` is computed through
[`petir::real::ln`], not `f64::ln`: this crate is `no_std`, and PETIR's
logarithm is one fixed implementation, so a SWU figure reproduces
bit-for-bit on any platform. That matters for a number someone will cite.

# Verification

**Methodology.** The textbook case — natural-uranium feed at 0.711 % U-235,
product at 4.5 %, tails at 0.25 % — evaluated against the closed forms
above, worked by hand in the test comments. Pass criterion: agreement to
`1e-9` relative on both the feed mass and the SWU, plus exact mass balance
(`F = P + T`) and exact reproduction of an assay round-trip through
[`uranium_assay_mass`].

**Results (measured 2026-09-16, this crate, `--release`).** For 1 kg of
4.5 % product with 0.25 % tails from 0.711 % feed:

| Quantity | Measured |
|---|---|
| feed | `9.219088937093275` kg |
| tails | `8.219088937093277` kg |
| SWU | `6.871112989320217` SWU |
| `V(0.045)` | `2.7800944541464734` |
| `V(0.0025)` | `5.959016609805414` |
| `V(0.00711)` | `4.868883385844146` |

These are consistent with the standard published figure of roughly
6.9 SWU per kilogram of 4.5 % product at 0.25 % tails. **Interpretation:** the
implementation reproduces the closed-form two-isotope cascade arithmetic;
it is *not* validated against a plant, and says nothing about real cascade
efficiency, hold-up, or U-234 carry-over.

```rust
pub mod enrichment { /* ... */ }
```

### Types

#### Struct `Assays`

The three assays of an enrichment step. Upstream `Assays`.

Each is a **dimensionless fraction in `(0, 1)`** — the fraction of the
uranium that is U-235. For 0.711 % U-235 natural uranium, `feed` is
`0.00711`.

All three must share a basis (atom or mass); see the [module docs](self).
Construction does not check that `feed` lies between `tails` and `product`,
because upstream does not and because a "stripping" configuration with
`product < feed` is a legitimate, if unusual, thing to ask about. It *is*
what makes [`feed_qty`] positive, so a caller that inverts them will get a
negative feed mass rather than an error.

```rust
pub struct Assays {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(feed: f64, product: f64, tails: f64) -> Self { /* ... */ }
  ```
  Builds the assay triple. Upstream `Assays::Assays`.

- ```rust
  pub fn feed(self: &Self) -> f64 { /* ... */ }
  ```
  The feed assay, a dimensionless fraction in `(0, 1)`. Upstream

- ```rust
  pub fn product(self: &Self) -> f64 { /* ... */ }
  ```
  The product assay, a dimensionless fraction in `(0, 1)`. Upstream

- ```rust
  pub fn tails(self: &Self) -> f64 { /* ... */ }
  ```
  The tails assay, a dimensionless fraction in `(0, 1)`. Upstream

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Assays { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Assays) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `uranium_assay`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

The U-235 **atom** fraction of the uranium in `mat`, dimensionless in
`[0, 1)`. Upstream `UraniumAssay`, which is an alias for
`UraniumAssayAtom`.

See [`uranium_assay_atom`].

```rust
pub fn uranium_assay(mat: &crate::material::Material) -> f64 { /* ... */ }
```

#### Function `uranium_assay_atom`

**Attributes:**

- `MustUse { reason: None }`

The U-235 **atom** fraction with respect to U-235 + U-238, dimensionless in
`[0, 1)`. Upstream `UraniumAssayAtom`.

Computed as `n235 / (n235 + n238)` from the material's atom fractions.
Everything that is not U-235 or U-238 — oxygen in an oxide, U-234,
fission products — is excluded from both numerator and denominator, so this
is the assay *of the uranium*, not of the material.

Returns `0.0` for a material with no U-235 and no U-238, which is upstream's
guard against dividing by zero.

```rust
pub fn uranium_assay_atom(mat: &crate::material::Material) -> f64 { /* ... */ }
```

#### Function `uranium_assay_mass`

**Attributes:**

- `MustUse { reason: None }`

The U-235 **mass** fraction with respect to U-235 + U-238, dimensionless in
`[0, 1)`. Upstream `UraniumAssayMass`.

As [`uranium_assay_atom`], but in the mass basis. For the same material the
mass assay is always the *smaller* of the two, U-235 being the lighter
isotope: natural uranium is 0.711 % by mass and 0.720 % by atom.

```rust
pub fn uranium_assay_mass(mat: &crate::material::Material) -> f64 { /* ... */ }
```

#### Function `uranium_qty`

**Attributes:**

- `MustUse { reason: None }`

The mass of uranium in `mat`, in **kilograms**. Upstream `UraniumQty`.

The sum of the U-235 and U-238 masses only — consistent with the
two-isotope idealisation, so the uranium in a UO2 pellet excludes its
oxygen *and* its U-234.

```rust
pub fn uranium_qty(mat: &crate::material::Material) -> f64 { /* ... */ }
```

#### Function `feed_qty`

**Attributes:**

- `MustUse { reason: None }`

The feed required to make `product_qty` of product. Upstream `FeedQty`.

From the two simultaneous balances — total mass `F = P + T` and U-235 mass
`F*xf = P*xp + T*xt` — eliminating `T` gives

`F = P * (xp - xt) / (xf - xt)`

# Parameters

- `product_qty` — kilograms of product uranium.
- `assays` — the three assays, in a basis matching `product_qty`.

# Returns

Kilograms of feed uranium. Always at least `product_qty` for a sane
configuration (`xt < xf < xp`); negative or infinite if the assays are
ordered wrongly or if `feed == tails`, neither of which is checked, exactly
as upstream.

```rust
pub fn feed_qty(product_qty: f64, assays: &Assays) -> f64 { /* ... */ }
```

#### Function `tails_qty`

**Attributes:**

- `MustUse { reason: None }`

The tails produced alongside `product_qty` of product. Upstream `TailsQty`.

`T = P * (xp - xf) / (xf - xt)`, which is [`feed_qty`] minus `product_qty`
— the total-mass balance `F = P + T`, rearranged. The tests assert that
identity holds to machine precision.

# Returns

Kilograms of depleted uranium.

```rust
pub fn tails_qty(product_qty: f64, assays: &Assays) -> f64 { /* ... */ }
```

#### Function `value_func`

The separative potential of a stream at assay `frac`, dimensionless.
Upstream `ValueFunc`.

`V(f) = (1 - 2f) * ln(1/f - 1)`

`V` is symmetric about `f = 0.5`, where it is zero, and diverges at both
ends of the interval: a stream that is already pure, in either isotope, has
unbounded separative potential.

# Parameters

- `frac` — a dimensionless fraction, valid in the **open** interval
  `(0, 1)`.

# Errors

[`CyclusError::Value`] if `frac` is outside `(0, 1)`.

# Divergence from upstream

Upstream rejects `frac < 0` and `frac >= 1`, which leaves `frac == 0`
admitted: it evaluates `ln(1/0 - 1)` and returns `+inf`, and that infinity
then propagates silently into a SWU figure. Here `frac == 0` is rejected as
well. `V` genuinely has no value at zero, and returning an error is the
honest report.

```rust
pub fn value_func(frac: f64) -> crate::error::Result<f64> { /* ... */ }
```

#### Function `swu_required`

The separative work needed to make `product_qty` of product. Upstream
`SwuRequired`.

`SWU = P*V(xp) + T*V(xt) - F*V(xf)`

with `F` from [`feed_qty`], `T` from [`tails_qty`] and `V` from
[`value_func`].

# Parameters

- `product_qty` — kilograms of product uranium.
- `assays` — the three assays, in a basis matching `product_qty`.

# Returns

**Separative work units (SWU)**, which carry the dimension of the mass unit
used for `product_qty` — kilogram-SWU here, since this crate's masses are
kilograms.

# Errors

[`CyclusError::Value`] if any of the three assays is outside `(0, 1)`. The
three are evaluated in the order product, tails, feed, so the first
offending one names the failure.

# Worked value

0.711 % feed, 4.5 % product, 0.25 % tails, 1 kg product:
**6.871112989320217 SWU** from **9.219088937093275 kg** of feed (measured
2026-09-16 — see the [module docs](self)).

```rust
pub fn swu_required(product_qty: f64, assays: &Assays) -> crate::error::Result<f64> { /* ... */ }
```

## Module `mat_query`

Reading physical quantities out of a [`Material`].

A [`Material`] carries a quantity in kilograms and a *ratio* — its
[`Composition`] is not normalised and is not a mass. Turning that pair into
"how many kilograms of U-235 are in this material" is a two-step operation
that is easy to get subtly wrong (normalise the right basis, then scale by
the quantity), and upstream's `MatQuery` exists to do it once. So does this.

# Units, stated plainly

| Function | Returns | Units |
|---|---|---|
| [`qty`] | total mass | kilograms |
| [`mass`] | mass of one nuclide | kilograms |
| [`moles`] | amount of substance of one nuclide | moles |
| [`mass_frac`], [`mass_frac_of`] | mass fraction | dimensionless, in `[0, 1]` |
| [`atom_frac`], [`atom_frac_of`] | atom (mole) fraction | dimensionless, in `[0, 1]` |
| [`all_mass`] | mass of every nuclide | kilograms |
| [`all_atoms`] | moles of every nuclide | moles |
| [`amount`] | extractable mass of a given composition | kilograms |

# Free functions and [`MatQuery`] both

Upstream's `MatQuery` holds a `Material::Ptr` — a `boost::shared_ptr`. This
workspace forbids lifetime parameters on structs, so a borrowing wrapper is
not available, and every query here is *also* a free function taking
`&Material`. [`MatQuery`] is the owning form, for a caller who wants the
upstream shape; it clones the material on construction and every method
delegates to the free function of the same name. Nothing is lost: these are
all read-only queries.

Prefer the free functions. They allocate nothing.

# A note on upstream's API surface

The brief for this port named `qty_bumped`, `AllMass` and `AllAtoms`. **No
such members exist at commit `d4faab7c`** — a `grep` over the whole
upstream tree finds none of the three. [`all_mass`] and [`all_atoms`] here
are therefore *not* translations; they are this port's own convenience over
[`Material::nuclide_masses`], named after what was asked for, and are
marked as such below. There is no `qty_bumped` equivalent because there is
nothing upstream to translate.

Upstream's `std::string`-keyed overloads (`mass("U235")` and friends) are
also absent: they call `pyne::nucname::id`, a name parser that belongs with
the nuclear data, not here. Construct a [`Nuc`] instead.

```rust
pub mod mat_query { /* ... */ }
```

### Types

#### Struct `MatQuery`

An owning inspector over a [`Material`]. Upstream `MatQuery`.

Every method is the free function of the same name, applied to the held
material. See the [module docs](self) for why this owns rather than
borrows, and for the units of each query.

# Example

```
use kaki_bukit::comp_math::CompMap;
use kaki_bukit::composition::{AtomicMasses, Composition};
use kaki_bukit::material::Material;
use kaki_bukit::nuclide::nuc;
use kaki_bukit::toolkit::mat_query::MatQuery;

// 10 kg of 5 % (by mass) enriched uranium.
let map: CompMap = [(nuc::U235, 0.05), (nuc::U238, 0.95)].into_iter().collect();
let comp = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
let mq = MatQuery::new(Material::new(10.0, comp).unwrap());

assert!((mq.mass(nuc::U235) - 0.5).abs() < 1e-12);      // kilograms
assert!((mq.mass_frac(nuc::U235) - 0.05).abs() < 1e-12); // dimensionless
```

```rust
pub struct MatQuery {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(m: Material) -> Self { /* ... */ }
  ```
  Wraps `m` for inspection. Upstream `MatQuery::MatQuery(Material::Ptr)`.

- ```rust
  pub fn material(self: &Self) -> &Material { /* ... */ }
  ```
  Borrows the wrapped material.

- ```rust
  pub fn into_material(self: Self) -> Material { /* ... */ }
  ```
  Consumes the query and returns the wrapped material.

- ```rust
  pub fn qty(self: &Self) -> f64 { /* ... */ }
  ```
  Total mass, in kilograms. See [`qty`].

- ```rust
  pub fn mass(self: &Self, nuc: Nuc) -> f64 { /* ... */ }
  ```
  Mass of `nuc`, in kilograms. See [`mass`].

- ```rust
  pub fn moles(self: &Self, nuc: Nuc, masses: &AtomicMasses) -> Result<f64> { /* ... */ }
  ```
  Amount of `nuc`, in moles. See [`moles`].

- ```rust
  pub fn mass_frac(self: &Self, nuc: Nuc) -> f64 { /* ... */ }
  ```
  Mass fraction of `nuc`, dimensionless. See [`mass_frac`].

- ```rust
  pub fn mass_frac_of(self: &Self, nucs: &[Nuc]) -> f64 { /* ... */ }
  ```
  Combined mass fraction of `nucs`, dimensionless. See [`mass_frac_of`].

- ```rust
  pub fn atom_frac(self: &Self, nuc: Nuc) -> f64 { /* ... */ }
  ```
  Atom fraction of `nuc`, dimensionless. See [`atom_frac`].

- ```rust
  pub fn atom_frac_of(self: &Self, nucs: &[Nuc]) -> f64 { /* ... */ }
  ```
  Combined atom fraction of `nucs`, dimensionless. See [`atom_frac_of`].

- ```rust
  pub fn all_mass(self: &Self) -> CompMap { /* ... */ }
  ```
  Mass of every nuclide, in kilograms. See [`all_mass`].

- ```rust
  pub fn all_atoms(self: &Self, masses: &AtomicMasses) -> Result<CompMap> { /* ... */ }
  ```
  Moles of every nuclide. See [`all_atoms`].

- ```rust
  pub fn almost_eq(self: &Self, other: &Material, threshold: f64) -> Result<bool> { /* ... */ }
  ```
  Composition equality to within `threshold`. See [`almost_eq`].

- ```rust
  pub fn amount(self: &Self, c: &Composition) -> f64 { /* ... */ }
  ```
  Extractable mass of composition `c`, in kilograms. See [`amount`].

- ```rust
  pub fn nuclides(self: &Self) -> Vec<Nuc> { /* ... */ }
  ```
  The nuclides present, in ascending id order.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> MatQuery { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: Material) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MatQuery) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `qty`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

The total mass of `m`, in kilograms. Upstream `MatQuery::qty`.

A thin alias for [`Material::quantity`], kept so a reader can follow
upstream's `mat_query.cc` line for line.

```rust
pub fn qty(m: &crate::material::Material) -> f64 { /* ... */ }
```

#### Function `mass`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

The mass of nuclide `nuc` in `m`, in kilograms. Upstream `MatQuery::mass`.

Computed as `mass_frac(nuc) * qty`, exactly as upstream does. Returns `0.0`
for a nuclide the material does not contain, and for an empty material.

Valid range of the result: `0.0` to `m.quantity()`.

```rust
pub fn mass(m: &crate::material::Material, nuc: crate::nuclide::Nuc) -> f64 { /* ... */ }
```

#### Function `moles`

The amount of nuclide `nuc` in `m`, in **moles**. Upstream
`MatQuery::moles`.

# The unit conversion, spelled out

Upstream computes `mass(nuc) / (pyne::atomic_mass(nuc) * units::g)`, where
`units::g` is `1e-3` — the mass of one gram expressed in the kilogram base
unit. An atomic mass in unified atomic mass units (u) is numerically equal
to a molar mass in grams per mole, so:

`moles = mass_kg / (A_u [g/mol] * 1e-3 [kg/g]) = 1000 * mass_kg / A_u`

# Parameters

- `nuc` — the nuclide to count.
- `masses` — the source of `A_u`. Upstream reaches a PyNE global here;
  this crate has no nuclear data of its own (workspace rule: that lives in
  `njoy-outram-park-fork`), so the table is an explicit argument.
  [`AtomicMasses::MassNumber`] approximates `A_u` by the mass number, which
  is under 0.1 % wrong for the actinides and is *not* good enough for a
  mass balance anyone will quote.

# Errors

Whatever [`AtomicMasses::mass_of`] reports — [`CyclusError::Key`] for a
nuclide missing from a [`AtomicMasses::Table`], or
[`CyclusError::InvalidNuclide`] for a natural-element id under
[`AtomicMasses::MassNumber`].

[`CyclusError::Key`]: crate::error::CyclusError::Key
[`CyclusError::InvalidNuclide`]: crate::error::CyclusError::InvalidNuclide

```rust
pub fn moles(m: &crate::material::Material, nuc: crate::nuclide::Nuc, masses: &crate::composition::AtomicMasses) -> crate::error::Result<f64> { /* ... */ }
```

#### Function `mass_frac`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

The mass fraction of `nuc` in `m`: dimensionless, in `[0, 1]`. Upstream
`MatQuery::mass_frac(Nuc)`.

Upstream copies the mass-basis composition, normalises it, and indexes it.
[`Composition::mass_frac`] is that operation, so this delegates. Returns
`0.0` for an absent nuclide or an empty composition.

```rust
pub fn mass_frac(m: &crate::material::Material, nuc: crate::nuclide::Nuc) -> f64 { /* ... */ }
```

#### Function `mass_frac_of`

**Attributes:**

- `MustUse { reason: None }`

The combined mass fraction of a set of nuclides: dimensionless, in
`[0, 1]`. Upstream `MatQuery::mass_frac(std::set<Nuc>)`.

Upstream sums `mass(nuc)` over the set and divides by `qty()`. That is
reproduced exactly, including the consequence that a **repeated nuclide in
`nucs` is counted twice** — upstream takes a `std::set`, which cannot
repeat, and a slice can. Pass distinct nuclides.

Returns `0.0` for an empty material (upstream would divide by zero and
return NaN; returning zero is a deliberate hardening, since every caller
treats the result as a fraction).

```rust
pub fn mass_frac_of(m: &crate::material::Material, nucs: &[crate::nuclide::Nuc]) -> f64 { /* ... */ }
```

#### Function `atom_frac`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

The atom (mole) fraction of `nuc` in `m`: dimensionless, in `[0, 1]`.
Upstream `MatQuery::atom_frac(Nuc)`.

Returns `0.0` for an absent nuclide or an empty composition.

```rust
pub fn atom_frac(m: &crate::material::Material, nuc: crate::nuclide::Nuc) -> f64 { /* ... */ }
```

#### Function `atom_frac_of`

**Attributes:**

- `MustUse { reason: None }`

The combined atom fraction of a set of nuclides: dimensionless, in
`[0, 1]`. Upstream `MatQuery::atom_frac(std::set<Nuc>)`.

Note that upstream implements this differently from its mass-fraction
counterpart — it normalises the atom map once and sums the *fractions*,
rather than summing absolute amounts and dividing. The two agree; the
normalise-once form is reproduced here. Nuclides absent from the
composition contribute nothing.

```rust
pub fn atom_frac_of(m: &crate::material::Material, nucs: &[crate::nuclide::Nuc]) -> f64 { /* ... */ }
```

#### Function `all_mass`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

The mass of **every** nuclide in `m`, in kilograms.

**Not an upstream translation.** There is no `AllMass` at commit
`d4faab7c`; this is an alias for [`Material::nuclide_masses`], provided
under the name the port brief asked for. Keys are nuclides, values are
kilograms, and the values sum to `m.quantity()`.

```rust
pub fn all_mass(m: &crate::material::Material) -> crate::comp_math::CompMap { /* ... */ }
```

#### Function `all_atoms`

The amount of **every** nuclide in `m`, in moles.

**Not an upstream translation** — see [`all_mass`]. Keys are nuclides,
values are moles, computed per [`moles`].

# Errors

Whatever [`AtomicMasses::mass_of`] reports for the first nuclide it cannot
price. A material containing a nuclide absent from `masses` fails as a
whole rather than silently dropping it.

```rust
pub fn all_atoms(m: &crate::material::Material, masses: &crate::composition::AtomicMasses) -> crate::error::Result<crate::comp_math::CompMap> { /* ... */ }
```

#### Function `almost_eq`

`true` if `m` and `other` have the same mass-basis composition to within a
relative `threshold`. Upstream `MatQuery::AlmostEq`.

Both compositions are normalised before comparison, so this compares
*composition*, not quantity: 1 kg and 1000 kg of the same enrichment are
almost-equal. Upstream's default `threshold` is
[`EPS_RSRC`](crate::limits::EPS_RSRC); use [`almost_eq_default`] for it.

# Errors

[`CyclusError::Value`](crate::error::CyclusError::Value) if `threshold` is
negative.

```rust
pub fn almost_eq(m: &crate::material::Material, other: &crate::material::Material, threshold: f64) -> crate::error::Result<bool> { /* ... */ }
```

#### Function `almost_eq_default`

[`almost_eq`] at upstream's default tolerance,
[`EPS_RSRC`](crate::limits::EPS_RSRC).

# Errors

Cannot fail in practice — [`EPS_RSRC`] is positive — but the signature is
kept fallible so the two forms stay interchangeable.

```rust
pub fn almost_eq_default(m: &crate::material::Material, other: &crate::material::Material) -> crate::error::Result<bool> { /* ... */ }
```

#### Function `amount`

**Attributes:**

- `MustUse { reason: None }`

The maximum mass, in kilograms, of composition `c` that can be extracted
from `m`. Upstream `MatQuery::Amount`.

This is the limiting-reagent calculation behind every separation and
fabrication step: given a recipe `c` and a feedstock `m`, how much product
does the scarcest ingredient allow?

# Method (upstream's, reproduced)

1. Normalise both mass-basis compositions to sum to 1.
2. For each nuclide in `c`, form the ratio `m_frac / c_frac`. If `m` lacks
   a nuclide that `c` needs in nonzero amount, the answer is `0.0`.
3. The smallest such ratio is the limiting one. Multiply it by
   `m.quantity()` and sum the scaled recipe.

Because step 1 normalises `c` to sum to 1, step 3's sum is just
`min_ratio * m.quantity()` — upstream's explicit re-sum is arithmetically
redundant, and is kept here only so the two files read alike.

# Returns

Kilograms, in `[0, m.quantity()]`. Returns `0.0` for an empty `c` — there
is no limiting ratio to take, and upstream's `CY_LARGE_DOUBLE` sentinel
would otherwise escape as a nonsense answer. That guard is this port's, not
upstream's.

```rust
pub fn amount(m: &crate::material::Material, c: &crate::composition::Composition) -> f64 { /* ... */ }
```

## Module `position`

Where a facility is: latitude, longitude, and the distance between two of
them.

A fuel-cycle study cares about geography for transport cost, for
proliferation-resistance metrics, and for plotting. [`Position`] is the
ISO 6709 coordinate pair every agent can carry, and [`Position::distance`]
is the great-circle distance between two.

# Units

| Quantity | Units |
|---|---|
| [`latitude`](Position::latitude) | **decimal degrees**, north positive, valid in `[-90, 90]` |
| [`longitude`](Position::longitude) | **decimal degrees**, east positive, valid in `[-180, 180]` |
| [`distance`](Position::distance) | **kilometres** |

# Internal representation: seconds of arc

Upstream stores both coordinates as *seconds* of degree — decimal degrees
times 3600 — quantised to a tenth of a second, and quantises the value
again to six decimal places when reading it back. That is Jaime Olivares's
design, and his reason is that it keeps degrees, minutes and seconds on the
integral part of the value so only the fraction of a second loses
precision. It is reproduced exactly here, because it is observable: a
coordinate does **not** round-trip bit-for-bit, and
[`latitude`](Position::latitude) can differ from what was set by up to
about `1.4e-5` degrees (a tenth of an arcsecond, about 3 mm on the ground
— far below any use this is put to, but not zero).

# Divergence: an out-of-range coordinate is an error, not a warning

Upstream calls `cyclus::Warn<VALUE_WARNING>` and stores a quiet NaN, so a
typo'd latitude silently poisons every distance computed from it — NaN
propagates through the haversine and comes out as a NaN distance, which
compares false against every threshold. Here
[`Position::new`] and the setters return
[`CyclusError::Value`](crate::error::CyclusError::Value). There is no
warning channel in a `no_std` kernel to write to, and a rejected coordinate
is strictly better than a contagious NaN.

# Not ported: ISO 6709 string formatting

Upstream's `ToString` / `ToStringHelper*` produce ISO 6709 Annex H strings
(`+51.5074-000.1278/`) through `std::stringstream` with `setprecision`,
`modf` and a stack of digit-padding special cases. Reproducing
`std::setprecision`'s exact output in `core::fmt` is a formatting exercise
with no physics in it, and the only consumer upstream is the output
database — which is out of scope for this kernel (see the crate root).
[`Position::latitude`] and [`Position::longitude`] give a caller everything
needed to format the pair however its own output layer wants.

`RecordPosition` is likewise absent: it writes an `AgentPosition` datum
through an agent's context.

# Verification

**Methodology.** Great-circle distance against published city coordinates
and the standard haversine closed form on the same sphere radius
(`R = 6372.8` km, upstream's value). Pass criterion: within 1 km of the
commonly quoted London-Paris great-circle distance of ~344 km, plus exact
symmetry, a zero self-distance, and a quarter-meridian check against
`R * pi / 2`.

**Results (measured 2026-09-16, this crate, `--release`).**

| Case | Measured | Reference |
|---|---|---|
| London (51.5074, -0.1278) to Paris (48.8566, 2.3522) | **343.6510 km** | ~344 km, published |
| Equator, 0 deg E to 90 deg E | **10011.13 km** | `6372.8 * pi / 2` = 10011.13 km, exact |
| Pole to pole (90 to -90) | **20022.26 km** | `6372.8 * pi`, exact |

**Interpretation.** The haversine on a sphere of radius 6372.8 km is
reproduced correctly. It is a *spherical* model: the real Earth is an
oblate spheroid, and distances from this differ from a geodesic (WGS-84)
calculation by up to about 0.5 %. That is upstream's model, not a defect of
the translation, and it is well inside what a transport-cost estimate needs.

```rust
pub mod position { /* ... */ }
```

### Types

#### Struct `Position`

A geographic location in latitude and longitude, following ISO 6709.
Upstream `Position`.

North latitude and east longitude are positive. Construct with
[`Position::new`]; the default is `(0, 0)`, upstream's default-constructed
value.

See the [module docs](self) for units, for the seconds-of-arc storage that
makes a coordinate quantise on the way in, and for the divergence on
out-of-range input.

```rust
pub struct Position {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(decimal_lat: f64, decimal_lon: f64) -> Result<Self> { /* ... */ }
  ```
  A position at the given decimal degrees. Upstream

- ```rust
  pub fn latitude(self: &Self) -> f64 { /* ... */ }
  ```
  The latitude in decimal degrees, north positive. Upstream

- ```rust
  pub fn longitude(self: &Self) -> f64 { /* ... */ }
  ```
  The longitude in decimal degrees, east positive. Upstream

- ```rust
  pub fn set_latitude(self: &mut Self, lat: f64) -> Result<()> { /* ... */ }
  ```
  Sets the latitude, in decimal degrees north. Upstream

- ```rust
  pub fn set_longitude(self: &mut Self, lon: f64) -> Result<()> { /* ... */ }
  ```
  Sets the longitude, in decimal degrees east. Upstream

- ```rust
  pub fn set_position(self: &mut Self, lat: f64, lon: f64) -> Result<()> { /* ... */ }
  ```
  Sets both coordinates, in decimal degrees. Upstream `set_position()`.

- ```rust
  pub fn distance(self: &Self, target: &Self) -> f64 { /* ... */ }
  ```
  The great-circle distance to `target`, in **kilometres**. Upstream

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Position { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Position { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Position) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `EARTH_RADIUS_KM`

The sphere radius used for [`Position::distance`], in kilometres.

Upstream's literal `6372.8`, which is a common single-value approximation
to the Earth's radius. See the module's verification note on what a
spherical model costs.

```rust
pub const EARTH_RADIUS_KM: f64 = 6372.8;
```

## Module `res_buf`

The inventory buffer every facility holds its stock in.

[`ResBuf`] is a FIFO queue of [`Resource`]s with a capacity. A facility
declares one per stream — an inventory and an outventory, say — fills them
through the resource exchange, and moves material between them on each time
step. It is the single most-used type in the whole toolkit.

```
use kaki_bukit::comp_math::CompMap;
use kaki_bukit::composition::{AtomicMasses, Composition};
use kaki_bukit::material::Material;
use kaki_bukit::nuclide::nuc;
use kaki_bukit::resource::Resource;
use kaki_bukit::toolkit::res_buf::ResBuf;

let map: CompMap = [(nuc::U238, 1.0)].into_iter().collect();
let comp = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();

let mut inventory = ResBuf::with_capacity(1000.0).unwrap();  // kilograms
inventory.push(Resource::from(Material::new(600.0, comp.clone()).unwrap())).unwrap();
assert_eq!(inventory.space(), 400.0);

// Take a 250 kg batch; the 600 kg object is split, and 350 kg stays.
let batch = inventory.pop_qty(250.0).unwrap();
assert_eq!(batch.len(), 1);
assert_eq!(inventory.quantity(), 350.0);
```

# Units

Every quantity here — [`capacity`](ResBuf::capacity),
[`quantity`](ResBuf::quantity), [`space`](ResBuf::space), and the `qty`
argument to [`pop_qty`](ResBuf::pop_qty) — is a **mass in kilograms**, the
unit [`Material::units`](crate::material::Material::units) reports.
[`count`](ResBuf::count) is a dimensionless object count.

# Why this is not generic

Upstream is `template <class T> class ResBuf`, instantiated at
`ResBuf<Material>` and `ResBuf<Product>`, and every `Push` does a
`dynamic_pointer_cast<T>` that throws `CastError` when an agent pushes the
wrong kind of resource into a buffer. **This port holds
[`Resource`](crate::resource::Resource) — the crate's non-generic owned
enum — instead**, for two reasons:

1. A generic `ResBuf<T>` would have to be bounded by a resource trait, and
   the workspace forbids trait objects; the enum *is* how this crate does
   closed-set polymorphism (see the crate root's translation table).
2. Upstream's run-time `CastError` becomes a [`Resource`] variant that the
   consumer matches on. Nothing is lost — a facility that wants materials
   only calls [`Resource::into_material`](crate::resource::Resource::into_material),
   which reports the same mistake, at the same moment, with a better message.

# What is deliberately not here

| Upstream | Status |
|---|---|
| `rs_present_`, the duplicate-push guard | **Unrepresentable, so removed.** Upstream stores `shared_ptr`s and two buffers can hold the same object; here a [`Resource`] is owned by exactly one place and a duplicate push cannot be written. |
| `is_bulk_` / bulk squashing on push | Squashing needs `Material::Absorb`, which needs an [`AtomicMasses`](crate::composition::AtomicMasses) table — upstream reaches one globally through PyNE, this crate has no nuclear data of its own. Call [`squash`](ResBuf::squash) explicitly instead; it does the same thing at a moment the caller chooses, with the table named. |
| `keep_packaging_` / `ChangePackage` | There is no `Package` type in this kernel. |
| `Pop(qty)` returning one squashed resource | Same reason as bulk mode: upstream's `res_manip::Squash`. [`pop_qty`](ResBuf::pop_qty) returns the pieces; squash them with [`squash_all`] if you want one object. |
| `Decay(curr_time)` | [`decay`](crate::decay) takes a caller-supplied chain rather than a context; a facility decays its buffer by popping, decaying and pushing back. |

# Mass conservation on a rejected push

Upstream throws on a push that would overflow, and the caller still holds
its `shared_ptr` — nothing is lost. Here the resource is *moved* into
[`push`](ResBuf::push), so an ordinary `Result<()>` would **destroy
material on the error path**, which is precisely the class of bug this
module must not have. Both push methods therefore fail with
[`Rejected`], which carries the resources back out. `?` still works in a
function returning [`Result`], because [`Rejected`] converts into
[`CyclusError`] — but that conversion is where the mass goes, so prefer
matching on it.

```rust
pub mod res_buf { /* ... */ }
```

### Types

#### Struct `Rejected`

Resources handed back by a [`ResBuf`] push that could not be accepted.

Carries the reason *and* the resources, so no mass is lost on the error
path. See the [module docs](self) for why this exists.

```rust
pub struct Rejected {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn error(self: &Self) -> &CyclusError { /* ... */ }
  ```
  The reason the push was refused.

- ```rust
  pub fn resources(self: &Self) -> &[Resource] { /* ... */ }
  ```
  Borrows the refused resources, in the order they were offered.

- ```rust
  pub fn into_resources(self: Self) -> Vec<Resource> { /* ... */ }
  ```
  Takes the refused resources back.

- ```rust
  pub fn into_resource(self: Self) -> Option<Resource> { /* ... */ }
  ```
  Takes back the single refused resource, for a rejected

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Rejected { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(r: Rejected) -> Self { /* ... */ }
    ```
    Discards the refused resources and keeps the reason, so that `?` works

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Rejected) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Type Alias `PushResult`

A push result: `Ok(())`, or the refused resources with their reason.

```rust
pub type PushResult = core::result::Result<(), Rejected>;
```

#### Struct `ResBuf`

A FIFO inventory buffer of [`Resource`]s with a mass capacity, in
kilograms. Upstream `ResBuf<T>`.

Resources come out in the order they went in — oldest first — unless
[`pop_back`](ResBuf::pop_back) is used. A freshly constructed buffer has
**infinite** capacity, as upstream's does; call
[`set_capacity`](ResBuf::set_capacity) or build it with
[`with_capacity`](ResBuf::with_capacity).

```rust
pub struct ResBuf {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty buffer of infinite capacity. Upstream `ResBuf()`.

- ```rust
  pub fn with_capacity(cap: f64) -> Result<Self> { /* ... */ }
  ```
  An empty buffer holding at most `cap` kilograms.

- ```rust
  pub fn capacity(self: &Self) -> f64 { /* ... */ }
  ```
  The maximum mass this buffer can hold, in kilograms. Upstream

- ```rust
  pub fn set_capacity(self: &mut Self, cap: f64) -> Result<()> { /* ... */ }
  ```
  Sets the maximum mass, in kilograms. Upstream `capacity(double)`.

- ```rust
  pub fn count(self: &Self) -> usize { /* ... */ }
  ```
  The number of resource objects held, dimensionless. Upstream `count()`.

- ```rust
  pub fn quantity(self: &Self) -> f64 { /* ... */ }
  ```
  The total mass held, in kilograms. Upstream `quantity()`.

- ```rust
  pub fn space(self: &Self) -> f64 { /* ... */ }
  ```
  The mass that can still be pushed, in kilograms, never negative.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` if the buffer holds no resource objects. Upstream `empty()`.

- ```rust
  pub fn resources(self: &Self) -> &VecDeque<Resource> { /* ... */ }
  ```
  Borrows the held resources, oldest first.

- ```rust
  pub fn peek(self: &Self) -> Result<&Resource> { /* ... */ }
  ```
  The next resource to be popped, without removing it. Upstream `Peek()`.

- ```rust
  pub fn push(self: &mut Self, r: Resource) -> PushResult { /* ... */ }
  ```
  Adds one resource. Upstream `Push(Resource::Ptr)`.

- ```rust
  pub fn push_all(self: &mut Self, rs: Vec<Resource>) -> PushResult { /* ... */ }
  ```
  Adds several resources, all or nothing. Upstream

- ```rust
  pub fn pop_qty(self: &mut Self, qty: f64) -> Result<Vec<Resource>> { /* ... */ }
  ```
  Removes and returns exactly `qty` kilograms, splitting a resource if

- ```rust
  pub fn pop_qty_eps(self: &mut Self, qty: f64, eps: f64) -> Result<Vec<Resource>> { /* ... */ }
  ```
  [`pop_qty`](ResBuf::pop_qty) with a caller-supplied tolerance. Upstream

- ```rust
  pub fn pop_n(self: &mut Self, n: usize) -> Result<Vec<Resource>> { /* ... */ }
  ```
  Removes and returns the `n` oldest resource objects, unsplit. Upstream

- ```rust
  pub fn pop(self: &mut Self) -> Result<Resource> { /* ... */ }
  ```
  Removes and returns the **oldest** resource object, unsplit. Upstream

- ```rust
  pub fn pop_back(self: &mut Self) -> Result<Resource> { /* ... */ }
  ```
  Removes and returns the **most recently pushed** resource object,

- ```rust
  pub fn squash(self: &mut Self, masses: &AtomicMasses) -> Result<()> { /* ... */ }
  ```
  Combines everything held into a single resource object, in place.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ResBuf { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ResBuf { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ResBuf) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `squash_all`

Combines a list of resources into one. Upstream `res_manip::Squash`.

Materials are merged with
[`Material::absorb`](crate::material::Material::absorb), which mass-weights
the compositions; products are merged with
[`Product::absorb`](crate::product::Product::absorb), which requires equal
quality strings. The result's mass is the sum of the inputs' masses.

# Parameters

- `rs` — the resources to merge. Must be all materials or all products.
- `masses` — atomic masses in u, for the material case; ignored for
  products.

# Errors

- [`CyclusError::Value`] if `rs` is empty — there is nothing to return.
- [`CyclusError::Value`] if `rs` mixes materials and products.
- Whatever the underlying `absorb` reports.

```rust
pub fn squash_all(rs: alloc::vec::Vec<crate::resource::Resource>, masses: &crate::composition::AtomicMasses) -> crate::error::Result<crate::resource::Resource> { /* ... */ }
```

## Module `symbolic`

Closed-form demand curves: linear, exponential and piecewise.

These are the functions a `GrowthRegion` evaluates to decide how much of a
commodity the simulation wants at a given time step. The independent
variable `x` is **the time step number**, dimensionless; the value is a
demand in the commodity's own units (kilograms per step for a material
commodity, or an energy for a power one). Neither upstream nor this port
fixes those units.

| Variant | Form |
|---|---|
| [`SymFunction::Linear`] | `f(x) = slope * x + intercept` |
| [`SymFunction::Exponential`] | `f(x) = constant * exp(exponent * x) + intercept` |
| [`SymFunction::Piecewise`] | a different function on each interval of `x` |

# No trait objects: an enum instead

Upstream is an abstract `SymFunction` base with three
`boost::shared_ptr<SymFunction>` subclasses and a `virtual double
value(double)`. This workspace forbids trait objects, and the set of
function shapes is closed and known at compile time — exactly the case the
workspace rule names — so [`SymFunction`] is an enum and `value` is a
`match`. A fourth shape becomes a new variant, and the compiler then points
at every site that must handle it, which the `virtual` version cannot do.

The recursion in [`SymFunction::Piecewise`] needs no `Box`: a
[`PiecewiseFunction`] owns a `Vec` of pieces, and the `Vec`'s own heap
allocation breaks the cycle.

`exp` comes from [`petir::real::exp`] rather than `f64::exp`, which is
`std`-only. See the crate root on why that also buys bit-identical results
across platforms.

# Not ported: the factories

Upstream's `symbolic_function_factories.{h,cc}` build these from XML input
strings. There is no XML layer in this kernel (see the crate root's scope
table), so construct the enums directly — [`PiecewiseFunction::push`] is the
replacement for `PiecewiseFunctionFactory`, which upstream declares a
`friend` purely to reach the private piece list.

# Verification

**Methodology.** Each variant is evaluated against its closed form, written
out independently in the tests, at points chosen to exercise the joins:
`x = 0`, either side of each piecewise breakpoint, and a decade of
exponential growth. Pass criterion: agreement to `1e-12` relative.
**Result:** all cases pass (measured 2026-09-16, this crate, `--release`);
e.g. a doubling-per-10-steps curve `f(x) = 100 * exp(ln(2)/10 * x)` returns
`200` at `x = 10` and `400` at `x = 20` to within `1e-12`.

```rust
pub mod symbolic { /* ... */ }
```

### Types

#### Struct `PiecewisePiece`

One piece of a [`PiecewiseFunction`]: a function and where it starts.
Upstream `PiecewiseFunction::PiecewiseFunctionInfo`.

The piece is active for `x >= x_offset` (until the next piece starts), and
is evaluated on the *shifted* argument: the value is
`function.value(x - x_offset) + y_offset`. So a piece is written in its own
local coordinates and then placed, which is what makes a continuous curve
easy to assemble.

```rust
pub struct PiecewisePiece {
    pub function: SymFunction,
    pub x_offset: f64,
    pub y_offset: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `function` | `SymFunction` | The function evaluated on this interval, in local coordinates. |
| `x_offset` | `f64` | Where this piece takes over, in time steps (dimensionless). |
| `y_offset` | `f64` | A constant added to this piece's value, in the demand's own units. |

##### Implementations

###### Methods

- ```rust
  pub fn new(function: SymFunction, x_offset: f64, y_offset: f64) -> Self { /* ... */ }
  ```
  A piece starting at `x_offset` with value offset `y_offset`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PiecewisePiece { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PiecewisePiece) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PiecewiseFunction`

A function defined piecewise over `x`. Upstream `PiecewiseFunction`.

Pieces must be pushed in **ascending `x_offset`** order; upstream's
`value` walks the list forward and stops at the first piece whose offset
exceeds `x`, so an out-of-order list silently gives wrong answers there too.
[`push`](PiecewiseFunction::push) enforces the order rather than reproducing
that trap — see its docs.

Below the first piece's `x_offset`, and for an empty function, the value is
`0.0`. That is upstream's documented "f(x) for all x in `[lhs,rhs]`, 0
otherwise".

```rust
pub struct PiecewiseFunction {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty piecewise function, which evaluates to `0.0` everywhere.

- ```rust
  pub fn push(self: &mut Self, piece: PiecewisePiece) { /* ... */ }
  ```
  Appends a piece, keeping the list in ascending `x_offset` order.

- ```rust
  pub fn pieces(self: &Self) -> &[PiecewisePiece] { /* ... */ }
  ```
  The pieces, in ascending `x_offset` order.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` if no pieces have been added; such a function is `0.0`

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  The number of pieces.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PiecewiseFunction { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> PiecewiseFunction { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PiecewiseFunction) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SymFunction`

A closed-form demand curve. Upstream's `SymFunction` hierarchy, collapsed
into one enum — see the [module docs](self).

Evaluate with [`value`](SymFunction::value); the argument is a time step
(dimensionless) and the result is a demand in the commodity's own units.

```rust
pub enum SymFunction {
    Linear {
        slope: f64,
        intercept: f64,
    },
    Exponential {
        constant: f64,
        exponent: f64,
        intercept: f64,
    },
    Piecewise(PiecewiseFunction),
}
```

##### Variants

###### `Linear`

`f(x) = slope * x + intercept`. Upstream `LinearFunction`.

Constant growth. The commonest demand curve in a fuel-cycle study, and
the one a flat demand (`slope == 0`) is written as.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `slope` | `f64` | Demand units per time step. |
| `intercept` | `f64` | Demand at `x = 0`, in the demand's own units. |

###### `Exponential`

`f(x) = constant * exp(exponent * x) + intercept`. Upstream
`ExponentialFunction`.

Compound growth. For a doubling every `T` steps, set
`exponent = ln(2) / T`; for decay, a negative `exponent`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `constant` | `f64` | The leading coefficient, in the demand's own units: the value at<br>`x = 0` less the intercept. |
| `exponent` | `f64` | The growth rate, per time step. Positive grows, negative decays. |
| `intercept` | `f64` | A constant offset, in the demand's own units — the asymptote as<br>`x -> -inf` for positive `exponent`. |

###### `Piecewise`

A different function on each interval of `x`. Upstream
`PiecewiseFunction`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PiecewiseFunction` |  |

##### Implementations

###### Methods

- ```rust
  pub fn linear(slope: f64, intercept: f64) -> Self { /* ... */ }
  ```
  A linear function `slope * x + intercept`. Upstream

- ```rust
  pub fn exponential(constant: f64, exponent: f64, intercept: f64) -> Self { /* ... */ }
  ```
  An exponential `constant * exp(exponent * x) + intercept`. Upstream

- ```rust
  pub fn value(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Evaluates the function at `x`. Upstream `SymFunction::value(double)`.

- ```rust
  pub fn print(self: &Self) -> String { /* ... */ }
  ```
  A human-readable form of the function. Upstream `SymFunction::Print()`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SymFunction { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SymFunction) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `total_inv_tracker`

A facility-wide inventory cap spanning several [`ResBuf`]s.

A facility that keeps its stock in more than one buffer — an inventory and
an outventory, say — often has a licence condition or a site limit on the
**total**, independent of what each buffer can hold. [`TotalInvTracker`] is
that limit. It stores no material itself; it only answers questions about
buffers it is shown.

All masses are **kilograms**.

# Divergence: the buffers are passed in, not held

Upstream holds `std::vector<ResBuf<Material>*>` — raw pointers into buffers
that live on the agent — and every query walks that vector. This workspace
forbids lifetime parameters on structs, so the tracker cannot hold
borrows, and holding `Arc<RwLock<ResBuf>>` would force every facility to
wrap its inventories in a lock they otherwise do not need.

So the buffers travel as an argument: `tracker.quantity(&[&inv, &outv])`.
The consequences are worth stating plainly, because they are the whole
difference:

- **The caller is responsible for passing the same set every time.** A
  facility that forgets a buffer under-reports its own inventory. Upstream's
  `Init` fixes the set once; here the discipline is at the call site, and a
  facility should have exactly one private method that builds the slice.
- Upstream's `buf_in_tracker` is meaningless here and is not ported —
  membership is decided by what the caller passes.
- Upstream throws if the tracker was never initialised (`num_bufs() == 0`).
  The equivalent here is an empty slice, and it is refused the same way; see
  [`CyclusError::State`].

[`ResBuf`]: crate::toolkit::res_buf::ResBuf
[`CyclusError::State`]: crate::error::CyclusError::State

```rust
pub mod total_inv_tracker { /* ... */ }
```

### Types

#### Struct `TotalInvTracker`

A cap, in kilograms, on the combined contents of several [`ResBuf`]s.
Upstream `TotalInvTracker`.

Holds no material. Every query takes the buffers it should look at — see
the [module docs](self).

```rust
pub struct TotalInvTracker {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An unbounded tracker. Upstream `TotalInvTracker()`.

- ```rust
  pub fn with_max(max_inv_size: f64) -> Result<Self> { /* ... */ }
  ```
  A tracker capping the combined inventory at `max_inv_size` kilograms.

- ```rust
  pub fn tracker_capacity(self: &Self) -> f64 { /* ... */ }
  ```
  The tracker's own cap, in kilograms, ignoring the buffers. Upstream

- ```rust
  pub fn quantity(self: &Self, bufs: &[&ResBuf]) -> Result<f64> { /* ... */ }
  ```
  The combined mass held by `bufs`, in kilograms. Upstream `quantity()`.

- ```rust
  pub fn total_capacity_bufs(self: &Self, bufs: &[&ResBuf]) -> Result<f64> { /* ... */ }
  ```
  The sum of the buffers' own capacities, in kilograms, ignoring the

- ```rust
  pub fn capacity(self: &Self, bufs: &[&ResBuf]) -> Result<f64> { /* ... */ }
  ```
  The effective capacity, in kilograms: the lesser of the tracker's cap

- ```rust
  pub fn space(self: &Self, bufs: &[&ResBuf]) -> Result<f64> { /* ... */ }
  ```
  The mass that may still be accepted across all buffers, in kilograms,

- ```rust
  pub fn constrained_buf_space(self: &Self, bufs: &[&ResBuf], buf: &ResBuf) -> Result<f64> { /* ... */ }
  ```
  How much one buffer may still accept once the facility-wide limit is

- ```rust
  pub fn is_empty(self: &Self, bufs: &[&ResBuf]) -> Result<bool> { /* ... */ }
  ```
  `true` if every tracked buffer is empty. Upstream `empty()`.

- ```rust
  pub fn set_capacity(self: &mut Self, cap: f64, bufs: &[&ResBuf]) -> Result<()> { /* ... */ }
  ```
  Changes the facility-wide cap, in kilograms. Upstream `set_capacity()`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> TotalInvTracker { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    An effectively unbounded tracker. Upstream's default-constructed

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TotalInvTracker) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Re-exports

### Re-export `CyclusError`

```rust
pub use error::CyclusError;
```

### Re-export `Result`

```rust
pub use error::Result;
```

### Re-export `Nuc`

```rust
pub use nuclide::Nuc;
```

