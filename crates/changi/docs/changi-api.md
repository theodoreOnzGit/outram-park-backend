# Crate Documentation

**Version:** 0.0.0

**Format Version:** 60

# Module `changi`

# CHANGI — atmospheric consequences

**C**onsequence and **H**azard **A**nalysis for **N**uclear **G**round-level
and atmospheric **I**mpacts.

CHANGI answers *"what happens after release?"*: how a radionuclide plume is
transported through the atmosphere, how it is depleted by dry and wet
deposition and radioactive decay, where it reaches the ground, and what
consequence that implies.

It is the middle link of the OUTRAM PARK offsite chain — SEMBAWANG (severe
accident: *what gets released?*) → **CHANGI** (*what happens after
release?*) → REDHILL (*what happens after deposition and infiltration?*).
Its input is a source term produced by SEMBAWANG. ~~`sembawang` exists in the
workspace as an explicit placeholder (no implementation), so until it
produces a real source term, a release-rate time series must be supplied
by hand.~~ **CORRECTED 2026-09-28** — `sembawang` is implemented
(`crates/sembawang/src/`) and hands this crate a source term through
`sembawang::chain::pad_for_dispersion`; a hand-supplied release-rate time
series still works.

## Scope

**Now** — research and educational use only: atmospheric dispersion, plume
transport, radionuclide deposition, ground contamination.

**Future, not current** (maintainer direction, 2026-09-15): radiological
consequence assessment, dose assessment, emergency-planning support and
Level 3 PSA support. None is implemented; none should be described as
available. Moving any of them into the current scope is a deliberate
maintainer decision taken in `RESPONSIBLE_USE.md`, not a side effect of
adding a feature.

## Intended use — binding limit

**CHANGI is for research, education and verification/validation only.**

It must **not** be presented as, or used for, emergency planning, emergency
response, dose assessment for real populations, Level 3 PSA support, nuclear
facility operation, or any safety-critical or licensing decision. The limit
is set by the workspace `RESPONSIBLE_USE.md` and by
`docs/ecosystem-naming.md` decision 3 (2026-08-05), reaffirmed by the
maintainer on 2026-09-15. An earlier naming draft claimed emergency-response
capability and was corrected precisely because it contradicted that policy.
Do not reintroduce that framing.

## Status

**Untrusted AI-assisted draft. No human V&V.** The crate is not declared
mature and carries no maturity bar. Nothing here has been compared against
measured atmospheric dispersion data. Code-to-code agreement with an
upstream code is *verification* (is it implemented as upstream specifies?),
never *validation* (does it represent reality well enough?).

## The two models

Two independent ports, each with its own upstream, licence and verification
harness. They are complementary, not alternatives:

| Module | Model | Upstream | Driven by |
|---|---|---|---|
| [`flexpart`] | Lagrangian particle dispersion | FLEXPART v10.4 (GPL-3.0-or-later) | gridded meteorology |
| [`puff`] | Analytic Gaussian puff | `puff` 0.1.1 (MIT) | a single wind series |

[`flexpart`] is built for synoptic scales and needs meteorological files;
only its surface-layer and deposition scalar kernels are ported so far, so
read it as the first verified slice of a port rather than "FLEXPART in
Rust". [`puff`] is complete for upstream's physics, needs no meteorological
input, and is cheap enough to run interactively over a site-sized domain —
but its dispersion fits are empirical over roughly 0.1–10 km and its unit
conversion is methane-specific. See each module for what it does and does
not cover.

## What it builds on

- [`petir`] — the workspace's core numerics crate, for `erf` and (in later
  phases) interpolation, quadrature and ODE integration. FLEXPART's own
  `erf.f90` is deliberately not ported.
- `outram-mc-libs`' LCG, for the pseudo-random numbers the Langevin
  turbulence scheme will need. Not yet wired in — no stochastic code has
  landed.
- `outram-foam-basic-lib`, for the gridded field and interpolation layer when
  the concentration-grid phase arrives. Not yet a dependency: the scalar
  kernels ported so far need nothing from it, and adding a finite-volume CFD
  dependency before there is a field to put on a mesh would be premature.
- `boon-lay`'s nuclide database is the intended source of half-lives for
  [`flexpart::decay`]; this crate deliberately carries no ~~nuclide data~~
  half-life data of its own so the two cannot drift. **CORRECTED
  2026-09-28** — it does embed ~~one nuclide table~~ ~~two~~ three published HTR-10
  nuclide tables of activities: the equilibrium-core inventory
  ([`activity::inventory`]), the annual normal-operation airborne
  release ([`activity::airborne_release`], added 2026-09-28), and the
  end-of-life primary-helium activity ([`activity::primary_helium`],
  added 2026-09-28). Provenance for all three is in `docs/References.md`.

## Licence

GPL-3.0, containing a port of FLEXPART (GPL-3.0-or-later). See
`LICENSE.flexpart` and `NOTICE.flexpart` at the crate root. Independent
fork; not affiliated with or endorsed by NILU or the FLEXPART developers.

## Modules

## Module `activity`

Radionuclide activity in air and on the ground, from a released source term.

This module is **not a port**. Everything under it was written here, and it
therefore has **no upstream and no code-to-code verification** — the strongest
evidence it can carry is internal consistency, which is what its tests
assert. Read every number it produces in that light. `changi`'s two ported
modules ([`crate::puff`] and [`crate::flexpart`]) keep their own harnesses
and are untouched by this one.

## What it computes

Given a release of activity at a point, over one or more time windows:

| Quantity | Unit | Where |
|---|---|---|
| dilution factor, `chi/Q` | s/m^3 | [`chi_over_q`] |
| time-integrated air concentration | Bq·s/m^3 | [`units`] |
| dry ground deposition | Bq/m^2 | [`deposition`] |

[`source`] is the input side — what was released, over which windows — and
[`survey`] is the one call that puts the three together.

**It computes no dose quantity of any kind**, and none is planned here. See
the scope limit below, which is binding.

~~One published dose table is~~ **CORRECTED 2026-09-28: two published
dose tables are** *stored* here as cited reference data. The first is
[`published_dose_by_distance`] (added 2026-09-28). It is read from a CSV,
not computed, and nothing uses it. Storing it does not change the
sentence above or the scope limit. **Added 2026-09-28:** a second one,
the same paper's accident doses, [`published_accident_dose_by_distance`],
on exactly the same terms.

## Relationship to the two ports — a consumer, not a shared abstraction

The crate rule is to keep `puff` and `flexpart` separate, because merging
their shared-looking pieces would make each one's comparison against *its
own* upstream harder to read. This module **consumes** both — [`crate::puff`]
for dispersion and [`crate::flexpart::decay`] for decay in transit — and that
is deliberately a different thing:

- It sits **above** both and defines no type either port uses.
- It changes **no ported signature**. The only edits it required in
  `puff::simulate` were widening `Puff` to `pub(crate)`, splitting
  `emit_with_classes` out of the existing `emit`, and adding
  `puff_unit_response` beside `sum_over_puffs`. Nothing was removed,
  generalised, or made to serve two upstreams at once.
- Neither fixture, tolerance, or reference script moved.
  `tests/puff_code_to_code.rs` and `tests/flexpart_code_to_code.rs` are the
  check on that claim, and both must stay green.

If a future change here would require a *common* stability class, a *common*
dispersion coefficient, or any other type spanning the two ports, that is the
rule biting and the answer is to duplicate rather than unify.

## Which dispersion model actually runs

[`crate::puff`], which is ported from the **R package `puff` 0.1.1**
(Hammerling Research Group, MIT) — a Gaussian puff model with empirical
Pasquill-Gifford sigmas, written for **methane** leak detection, fitted over
roughly 0.1-10 km and carrying no turbulence closure.

It is **not FLEXPART**. `changi::flexpart` is four scalar-kernel modules that
its own documentation calls *"the first verified slice of a port"*; it cannot
transport a plume and is not used for transport here. Say which model ran
when reporting a number from this module.

## Scope limit — binding, do not soften

Research, education and V&V only, exactly as the crate root states. Nothing
in this module may be described, in code, docs, commit messages or chat, as
supporting emergency planning, emergency response, dose assessment for real
populations, or Level 3 PSA. Implementing the physics is not what promotes
an item out of the crate's *future* scope list; that is a maintainer
decision taken in `RESPONSIBLE_USE.md`.

```rust
pub mod activity { /* ... */ }
```

### Modules

## Module `airborne_release`

Published HTR-10 annual airborne release to the environment under normal
operation (Liu and Cao 2002, Table 5). Reference data only; nothing in this
crate consumes it.
**A published annual airborne release from HTR-10 under normal operation.**

# What this is

The activity of each nuclide that the source calculates is released to the
environment as airborne effluent in **one year of normal operation** of
HTR-10. The table itself gives no unit or time basis; the paper's text
calls it the annual amount and states totals in becquerels, so each entry
here is activity released over one year, in Bq.

It is the counterpart to [`super::inventory`]. That module is what is *in
the core*; this one is what the source calculates *gets out* during normal
operation. That is **not** an accident source term, which the same paper
tabulates separately (its Table 8, not digitised here).

# Basis of the source's calculation (so a reader knows what it includes)

- Twenty-two nuclides: noble gases, iodines, Sr-89, Cs-134, Cs-137,
  Ag-110m, and three nuclides that are not fission products of the core
  inventory: **H-3**, **C-14** and **Ar-41**. Ar-41 comes from neutron
  activation of argon in the reactor-cavity air.
- Contributions from cavity-air activation, primary-helium leakage, the
  contaminated-helium tank, fuel-handling vacuum systems, tritiated
  secondary-steam leakage, and maintenance.
- **Filtration is not credited.** The source says its calculation is
  conservative and leaves out the filter system, so these figures are
  upper estimates of what a filtered plant would release.

# What this is NOT

- **Not wired into any model.** Nothing in this crate or in `htgr_sim_v1`
  reads it. Whether and how it replaces a leak-rate-times-inventory
  estimate is a modelling decision for the maintainer.
- **Not a dose**, and nothing here computes one (see the scope limit in
  [`crate::activity`]).
- `RESPONSIBLE_USE.md` applies: nothing here may be quoted as a release
  figure for HTR-10 or any other plant for any operational, licensing or
  safety purpose.

# Provenance

Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
environment impact for normal reactor operations and for relevant
accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 5
(p. 85), "Amount of airborne radioactivity released into the environment
in the HTR-10 normal operation conditions".

Access terms and how the values were obtained (the maintainer's kovan
digitisation and an independent text-layer transcription, which agree on
all 22 values) are in `crates/changi/docs/References.md`. The document carries no
reuse licence and is **not** redistributed here; only the cited table of 22
values is, which is ordinary scientific citation.

```rust
pub mod airborne_release { /* ... */ }
```

### Types

#### Struct `AirborneReleaseEntry`

One row of the published annual release.

```rust
pub struct AirborneReleaseEntry {
    pub nuclide: &'static str,
    pub annual_release: uom::si::f64::Radioactivity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `&'static str` | Nuclide label as the source tabulates it, e.g. `"Ar-41"`. |
| `annual_release` | `uom::si::f64::Radioactivity` | Activity released to the environment over one year of normal<br>operation (unfiltered, per the source's conservative basis). |

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
    fn clone(self: &Self) -> AirborneReleaseEntry { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AirborneReleaseEntry) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `htr10_normal_operation_annual_release`

**Attributes:**

- `MustUse { reason: None }`

Every nuclide in the published HTR-10 normal-operation annual airborne
release.

Twenty-two entries, in the order the source tabulates them. Note that the
source orders Kr-85m before Kr-85, Xe-133m before Xe-133 and Xe-135m
before Xe-135, which is not the order its Table 1 uses.

```rust
pub fn htr10_normal_operation_annual_release() -> Vec<AirborneReleaseEntry> { /* ... */ }
```

#### Function `htr10_annual_airborne_release`

**Attributes:**

- `MustUse { reason: None }`

Look one nuclide's annual airborne release up by label.

Returns `None` for a nuclide the table does not list. A nuclide that is
absent was not reported, which is not the same thing as a zero release.

```rust
pub fn htr10_annual_airborne_release(nuclide: &str) -> Option<uom::si::f64::Radioactivity> { /* ... */ }
```

## Module `decay_transfer`

**Per-nuclide radioactive decay, as a transfer function on the puff.**

Each nuclide gets its own scalar gain applied on top of one shared
transport field (maintainer, 2026-09-24: "each nuclide should have its own
attenuation factor slapped on top of the puff model ... this feels a little
like a transfer function"). It does, and the analogy is exact rather than
a convenience.

# Why this is EXACT, not an approximation

Transport with decay obeys

```text
  dC/dt = K grad^2 C  -  u . grad C  -  lambda C
```

Substitute `C = exp(-lambda t) C_0`. The first two terms are linear in `C`
and carry the factor straight through, while the time derivative produces
`-lambda exp(-lambda t) C_0` which cancels the decay term exactly, leaving

```text
  dC_0/dt = K grad^2 C_0  -  u . grad C_0
```

i.e. the **decay-free** equation. So `C = exp(-lambda t) C_0` solves the
full problem whenever `C_0` solves the transport problem: decay separates
completely and contributes a multiplier that depends **only on travel
time**, never on position within the puff.

Two consequences worth stating plainly:

- **The spatial field is computed once and shared.** Every nuclide sees
  the same Gaussian; they differ only in a scalar. That is the transfer
  function — the puff is the plant, decay is a first-order gain on its
  output, and `N` nuclides cost one transport solve and `N` multiplies.
- **A Monte-Carlo decay simulation here would buy nothing.** It would
  sample a distribution whose mean is this closed form and whose variance
  is an artefact of the sampling, not of the physics. `boon-lay`'s
  Lagrangian decay engine earns its place where the answer is *not* a
  single exponential — see "Chains" below — not here.

# Where the shared field stops being shared

Decay is uniform; **depletion is not**. Dry deposition, wet scavenging and
gravitational settling all depend on the nuclide's chemical and physical
form, so a noble gas, an iodine and a caesium aerosol released together do
**not** stay the same shape as they travel — the depleted ones lose mass
from the bottom of the plume first.

So the rule is: **one transport field per depletion class, one scalar per
nuclide within it.** For an HTR-10 source term that is three fields, not
thirty:

| class | depletion | members |
|---|---|---|
| noble gases | none | Kr, Xe |
| halogens | dry + wet, reactive | I |
| particulate / metallic | dry + wet, aerosol | Cs, Sr, Ag, Te |

[`crate::activity::deposition`] owns that side. This module owns only the
part that is genuinely uniform.

# Chains, and when `boon-lay` becomes the right tool

For a nuclide with an ingrowing parent the gain is **not** a single
exponential — it is the Bateman solution, and a daughter's airborne
activity can *rise* during transport while its parent falls. The spatial
shape is still shared (same puff, same transport), so it is still a scalar
per nuclide; it is just a different scalar. [`bateman_two_step`] covers the
common parent-daughter case in closed form.

Beyond two steps, or where a daughter changes depletion class mid-flight
(a noble-gas parent decaying to a particulate daughter genuinely does),
the closed form stops being convenient and `boon_lay`'s decay engine is
the honest tool. That case is **not** implemented here.

# Does it matter at all? Usually not, and the table says when

Travel time to 10 km at 5 m/s is about 2000 s. Against that:

| half-life | `exp(-lambda t)` at 2000 s | verdict |
|---|---|---|
| Cs-137, 30 y | 1.0000 | neglect |
| Kr-85, 10.7 y | 1.0000 | neglect |
| I-131, 8.02 d | 0.9980 | neglect |
| Xe-133, 5.24 d | 0.9969 | neglect |
| I-133, 20.8 h | 0.9817 | marginal |
| Xe-135, 9.14 h | 0.9587 | keep |
| Kr-88, 2.84 h | 0.8732 | keep |
| Kr-87, 76.3 min | 0.7387 | dominant |
| Xe-138, 14.1 min | 0.1942 | dominant |

So the maintainer's instinct is right twice over: the long-lived nuclides
really can be neglected over transport, and the short-lived ones really do
need the factor. Both follow from one line of arithmetic, which is why
this module exposes [`decay_is_negligible`] rather than leaving each
caller to guess.

```rust
pub mod decay_transfer { /* ... */ }
```

### Types

#### Struct `DecayTransfer`

One nuclide's decay gain: the scalar this nuclide contributes on top of
the shared transport field.

```rust
pub struct DecayTransfer {
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
  pub fn from_half_life(half_life: Time) -> Self { /* ... */ }
  ```
  From a half-life. A non-positive or non-finite half-life gives a

- ```rust
  pub fn stable() -> Self { /* ... */ }
  ```
  A nuclide that does not decay on any transport timescale.

- ```rust
  pub fn decay_constant(self: &Self) -> f64 { /* ... */ }
  ```
  `lambda`, s^-1.

- ```rust
  pub fn gain(self: &Self, travel_time: Time) -> Ratio { /* ... */ }
  ```
  The gain, `exp(-lambda t)`, for a given travel time.

- ```rust
  pub fn decay_is_negligible(self: &Self, travel_time: Time, tolerance: f64) -> bool { /* ... */ }
  ```
  Whether decay may be neglected over `travel_time` at the given

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
    fn clone(self: &Self) -> DecayTransfer { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DecayTransfer) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `bateman_two_step`

The airborne activity of a **daughter** fed by a decaying parent, per unit
parent activity released, after `travel_time` (the two-step Bateman
solution).

```text
  A_d(t) / A_p(0) = lambda_d/(lambda_d - lambda_p)
                    * ( exp(-lambda_p t) - exp(-lambda_d t) )
```

Still a scalar on the same shared puff, because parent and daughter travel
together — **provided they share a depletion class**. A noble-gas parent
decaying to a particulate daughter does not, and this function is the
wrong tool for that case; see the module docs.

Returns zero for a stable parent (nothing to feed the daughter) and
handles `lambda_p == lambda_d` by its limit, `lambda t exp(-lambda t)`,
rather than dividing by zero.

```rust
pub fn bateman_two_step(parent: DecayTransfer, daughter: DecayTransfer, travel_time: uom::si::f64::Time) -> uom::si::f64::Ratio { /* ... */ }
```

## Module `inventory`

A published HTR-10 core inventory, so a source term can be built from
measured magnitudes rather than round illustrative numbers. An inventory
is NOT a source term -- see the module docs.
**A published core inventory, so a source term can be a real one.**

# Why this exists

[`SourceTerm`](super::source::SourceTerm) takes activities in becquerels
and says nothing about where they come from — deliberately, since it is a
dispersion input and not a reactor model. The consequence is that every
number in this crate's tests and examples has been an **illustrative
fixture**: `1.0e12` Bq of I-131 and so on, chosen to be round rather than
to be true.

That is fine for exercising the arithmetic and useless for anything else.
This module supplies one **published** inventory so a caller can build a
source term whose magnitudes mean something, and so the crate's own
examples stop quoting numbers that came from nowhere.

# What this is NOT

**An inventory is not a source term.** It is what is *in the core*, not
what gets *out*. Turning one into the other needs a release fraction —
how much escapes the fuel, the vessel and the building — and this module
supplies none, because that is a reactor and containment question rather
than a dispersion one. A caller that multiplies these figures by a leak
fraction is doing the modelling; this module only removes the need to
invent the starting magnitude.

`RESPONSIBLE_USE.md` applies: nothing here may be quoted as a source term
for HTR-10 or any other plant.

# Provenance

Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
environment impact for normal reactor operations and for relevant
accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 1,
"fission product inventories for equilibrium core of HTR-10" — computed
there with ORIGEN2 at an average burnup of 80 000 MWd/t.

Full access terms and the transcription steps are in
`crates/changi/docs/References.md`. The document itself carries no reuse
licence and is **not** redistributed here; only the cited table of 22
values is, which is ordinary scientific citation.

```rust
pub mod inventory { /* ... */ }
```

### Types

#### Struct `InventoryEntry`

One row of the published inventory.

```rust
pub struct InventoryEntry {
    pub nuclide: &'static str,
    pub activity: uom::si::f64::Radioactivity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `&'static str` | Nuclide label as the source tabulates it, e.g. `"Cs-137"`. |
| `activity` | `uom::si::f64::Radioactivity` | Core inventory. |

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
    fn clone(self: &Self) -> InventoryEntry { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &InventoryEntry) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `htr10_equilibrium_core`

**Attributes:**

- `MustUse { reason: None }`

Every nuclide in the published HTR-10 equilibrium-core inventory.

Twenty-two entries, in the order the source tabulates them. This is **not**
the whole fission-product set — it is the subset Liu and Cao report, which
is the radiologically interesting one for a release, not a complete
depletion output.

```rust
pub fn htr10_equilibrium_core() -> Vec<InventoryEntry> { /* ... */ }
```

#### Function `htr10_core_inventory`

**Attributes:**

- `MustUse { reason: None }`

Look one nuclide up by label.

Returns `None` for a nuclide the table does not list, which is the honest
answer — a caller asking for something outside the 22 must not be handed a
zero that reads like a measurement.

```rust
pub fn htr10_core_inventory(nuclide: &str) -> Option<uom::si::f64::Radioactivity> { /* ... */ }
```

## Module `published_dose_by_distance`

Published HTR-10 normal-operation dose-versus-distance table (Liu and Cao
2002, Table 7): stored reference data, **not** a dose this crate computes.
Nothing in this crate consumes it.
**A published dose-versus-distance table for HTR-10 normal operation,
stored as reference data. Nothing here computes a dose.**

# What this is

The source's individual effective dose to an adult member of the public,
in mSv per year, at twelve distances from 0.5 km to 75 km from the release
point. It is given only along the azimuth where the dose is largest. It is
the dose the same paper calculates from its annual normal-operation
airborne release (its Table 5, in [`crate::activity::airborne_release`]).

It is a **published model result, not a measurement.** The source
calculated it with the US EPA code **AIRDOS-EPA** (Moore et al., 1979),
which the authors say they partly modified. The paper states this basis:

- **Release:** routine airborne effluent from one year of normal
  operation, from a 40 m exhaust stack (the reactor building is 12 m) with
  a 9 m/s exit velocity. It is the unfiltered, conservative release of the
  paper's Table 5.
- **Pathways:** gamma submersion in the plume, gamma radiation from
  contaminated ground, inhalation, and ingestion.
- **Receptor:** adults. The food-consumption rates are the source's
  Table 6, which is not reproduced here.
- **Meteorology:** measurements from an observatory 7.5 km from the site.
  The dose differs by direction, and the table gives only the direction of
  maximum dose.
- **Dose factors:** USDOE/EH-0070 (1988) and IAEA (1996).

# What this is NOT

- **Not computed here, and not wired into any model.** Nothing in this
  crate or in `htgr_sim_v1` reads it. `changi` still computes no dose
  quantity (see [`crate::activity`]). This module stores numbers a
  published paper printed. It does not bring dose assessment into this
  crate's current scope. That is a maintainer decision taken in
  `RESPONSIBLE_USE.md`.
- **Not an accident dose.** The same paper tabulates accident doses
  separately (its Table 9), ~~which is not digitised here~~ **CORRECTED
  2026-09-28**: now stored in
  [`crate::activity::published_accident_dose_by_distance`].
- `RESPONSIBLE_USE.md` applies in full. Nothing here may be quoted as a
  dose to the public from HTR-10 or any other plant for any operational,
  licensing, siting, emergency-planning or safety purpose.

# Units

Distance is a `uom` [`Length`](uom::si::f64::Length). **The dose is a plain `f64` in mSv per
year**, and the field name says so. `uom` 0.38 has no sievert quantity:
there is no equivalent-dose or absorbed-dose module in `uom::si`. It was
checked 2026-09-28, and no crate in this workspace defines one either.
`AvailableEnergy` (J/kg) has the right dimension but was rejected on
purpose. A sievert is J/kg only after radiation and tissue weighting, and
a type that lets a dose be added to a specific energy would hide that.

# Provenance

Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
environment impact for normal reactor operations and for relevant
accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 7
(p. 87, printed sideways), "Individual effective doses (mSv a⁻¹) to the
public at various distance (km) from the release point in the azimuth
where the maximum dose occurs". The basis above comes from the paper's
Section 3.2 (pp. 85–86).

Access terms, digitisation and verification are in
`crates/changi/docs/References.md`. The document carries no reuse licence
and is **not** redistributed here. Only the cited table of 12 values is,
which is ordinary scientific citation.

```rust
pub mod published_dose_by_distance { /* ... */ }
```

### Types

#### Struct `PublishedDoseAtDistance`

One row of the published dose-versus-distance table.

```rust
pub struct PublishedDoseAtDistance {
    pub distance: uom::si::f64::Length,
    pub effective_dose_msv_per_year: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `distance` | `uom::si::f64::Length` | Distance from the release point (the stack), along the azimuth of<br>maximum dose. |
| `effective_dose_msv_per_year` | `f64` | Individual effective dose to an adult member of the public, in<br>**millisieverts per year**, as published. A plain `f64` because `uom`<br>has no sievert quantity (see the module docs). |

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
    fn clone(self: &Self) -> PublishedDoseAtDistance { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PublishedDoseAtDistance) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `htr10_normal_operation_dose_by_distance`

**Attributes:**

- `MustUse { reason: None }`

Every row of the published HTR-10 normal-operation dose-versus-distance
table, in the source's order (increasing distance, 0.5 km to 75 km).

```rust
pub fn htr10_normal_operation_dose_by_distance() -> Vec<PublishedDoseAtDistance> { /* ... */ }
```

## Module `published_accident_dose_by_distance`

Published HTR-10 accident dose-versus-distance table (Liu and Cao 2002,
Table 9; depressurization and water ingress, thyroid and whole-body, mSv):
stored reference data, **not** a dose this crate computes. Nothing in this
crate consumes it.
**A published dose-versus-distance table for two HTR-10 design-basis
accidents, stored as reference data. Nothing here computes a dose.**

# What this is

The source's individual dose to a member of the public, in **mSv** (a
dose per accident, not a rate), at thirteen distances from 0.25 km to
75 km from the release point. It gives two organ/body quantities,
**thyroid** and **whole-body**, for each of two accidents:

- **Depressurization accident** ([`AccidentCase::Depressurization`]):
  loss of primary helium through a ruptured 65 mm fuel-element charging
  tube. The paper's Section 4.1.1 sums four sources: the primary-helium
  activity, fission products desorbed from primary-circuit surfaces, dust
  mobilised from "dead-water regions", and activity bound in the helium
  purification system. Fission products in the coated particles are
  taken as not released, because the paper's cited transient analysis
  puts peak fuel temperature at 1033 °C, below the 1600 °C limit.
- **Water ingress accident** ([`AccidentCase::WaterIngress`]): a two-ended
  rupture of two steam-generator heat-transfer tubes with the steam relief
  system failing, admitting at most 129.9 kg of water. The paper's
  Section 4.1.2 sums three sources: about 23 % of the primary-helium
  activity, wash-off of the activity deposited on the steam generator,
  and activity in up to 4.88 kg of corroded graphite.

The paper names these two as the design-basis accidents that lead to the
largest potential dose to the public. The releases behind this table are
the paper's Table 8, which is **not** digitised in this workspace. In both
cases the release goes out through the 40 m exhaust stack, and the paper
credits no filtering and no plate-out in the reactor building.

It is a **published model result, not a measurement.** The source
calculated it with the German code **STOERNEU**. The paper's Section 4.2
states this basis:

- **Pathways:** gamma and beta submersion, gamma radiation from
  contaminated ground, inhalation, and ingestion.
- **Geometry:** a 40 m stack; the reactor building is 28 m high and 30 m
  wide.

**The paper does NOT state**, for this table: the integration period of
the dose (the table is in mSv, with no time basis), the receptor age
group, the meteorology or dispersion conditions, whether the values are
for a worst azimuth, the dose coefficients, or whether "whole-body" means
effective dose. Do not read any of those into it. In particular, do not
assume Table 7's "azimuth of maximum dose" or its adult receptor carry
over; Table 7 was computed with a different code (AIRDOS-EPA).

# What the paper says about it

Comparing this table with its Table 10 (the emergency intervention levels
of Chinese Nuclear Safety Criterion HAD 002/03), the paper concludes the
doses are much lower than the lowest sheltering level, so no intervention
(evacuation, sheltering or stable iodine) would be needed even for the
worst of the accidents it analysed. Table 10's lowest sheltering levels
are 5 mSv whole-body and 50 mSv for the thyroid and other important
organs. The test
`every_dose_is_below_the_lowest_sheltering_level_the_paper_compares_against`
checks that statement against the stored numbers. It holds: the largest
whole-body dose (0.20 mSv, water ingress, 0.25 km) is 25 times below 5 mSv,
and the largest thyroid dose (1.1 mSv, same case and distance) is about 45
times below 50 mSv. That is the paper's comparison, reproduced; it is not
an endorsement of it and not an emergency-planning finding of this
workspace.

# What this is NOT

- **Not computed here, and not wired into any model.** Nothing in this
  crate or in `htgr_sim_v1` reads it. `changi` still computes no dose
  quantity (see [`crate::activity`]).
- **Parked here, not settled here.** Dose is to live in the placeholder
  crate `buangkok` eventually, but the maintainer (2026-09-28) has asked
  for the dose tables to stay in `changi` until they decide. Do not move
  it, and do not build dose computation around it, unasked.
- **Not a basis for emergency planning, emergency-zone sizing, siting,
  licensing or any safety decision**, for HTR-10 or any other plant.
  `RESPONSIBLE_USE.md` applies in full. This workspace uses it for
  research-grade safety analysis only: a published number that a future
  research calculation may be compared with.
- **Not a beyond-design-basis result.** It covers the two design-basis
  accidents above, with no release from the coated particles.

# Units

Distance is a `uom` [`Length`](uom::si::f64::Length). **The doses are
plain `f64` in mSv**, and the field names say so, for the reason given in
[`crate::activity::published_dose_by_distance`]: `uom` 0.38 has no
sievert quantity, and `AvailableEnergy` (J/kg) was rejected on purpose.

# Provenance

Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
environment impact for normal reactor operations and for relevant
accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 9
(p. 88), "Individual doses caused by accidents of the HTR-10 (mSv)". The
basis above comes from the paper's Sections 4.1–4.2 (pp. 86–89).

Access terms, digitisation and verification are in
`crates/changi/docs/References.md`. The document carries no reuse licence
and is **not** redistributed here. Only the cited table of 65 numbers is,
which is ordinary scientific citation.

```rust
pub mod published_accident_dose_by_distance { /* ... */ }
```

### Types

#### Enum `AccidentCase`

Which of the paper's two tabulated accidents a dose belongs to.

```rust
pub enum AccidentCase {
    Depressurization,
    WaterIngress,
}
```

##### Variants

###### `Depressurization`

Primary-circuit depressurization through a ruptured 65 mm
fuel-element charging tube (paper Section 4.1.1).

###### `WaterIngress`

Water ingress through two ruptured steam-generator tubes with the
steam relief system failed (paper Section 4.1.2).

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
    fn clone(self: &Self) -> AccidentCase { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AccidentCase) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PublishedAccidentDoses`

The two doses the table gives for one accident at one distance.

```rust
pub struct PublishedAccidentDoses {
    pub thyroid_msv: f64,
    pub whole_body_msv: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `thyroid_msv` | `f64` | Thyroid dose, in **millisieverts**, as published. The paper says only<br>"Thyroid". |
| `whole_body_msv` | `f64` | Whole-body dose, in **millisieverts**, as published. The paper says<br>only "Whole-body"; it does not say whether this is an effective dose. |

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
    fn clone(self: &Self) -> PublishedAccidentDoses { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PublishedAccidentDoses) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PublishedAccidentDoseAtDistance`

One row of the published accident dose-versus-distance table.

```rust
pub struct PublishedAccidentDoseAtDistance {
    pub distance: uom::si::f64::Length,
    pub depressurization: PublishedAccidentDoses,
    pub water_ingress: PublishedAccidentDoses,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `distance` | `uom::si::f64::Length` | Distance from the release point (the stack). The paper does not say<br>along which azimuth. |
| `depressurization` | `PublishedAccidentDoses` | Doses for the depressurization accident. |
| `water_ingress` | `PublishedAccidentDoses` | Doses for the water ingress accident. |

##### Implementations

###### Methods

- ```rust
  pub fn doses(self: &Self, case: AccidentCase) -> PublishedAccidentDoses { /* ... */ }
  ```
  The doses for one accident case at this distance.

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
    fn clone(self: &Self) -> PublishedAccidentDoseAtDistance { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PublishedAccidentDoseAtDistance) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `htr10_accident_dose_by_distance`

**Attributes:**

- `MustUse { reason: None }`

Every row of the published HTR-10 accident dose-versus-distance table, in
the source's order (increasing distance, 0.25 km to 75 km).

```rust
pub fn htr10_accident_dose_by_distance() -> Vec<PublishedAccidentDoseAtDistance> { /* ... */ }
```

## Module `primary_helium`

Published HTR-10 primary-helium activity at the end of a 20-year full-power
life (Liu and Cao 2002, Table 3). Reference data only; nothing in this
crate consumes it.
**A published HTR-10 primary-helium activity at the end of a 20-year life.**

# What this is

The activity of each important fission product (and tritium) that the
source calculates is *circulating in the primary helium* of HTR-10 at the
end of 20 years of full-power operation, in Bq. The table's title states
the unit (Bq) and the basis (end of a 20-year lifetime at full power).

It sits between the other two published tables in this module:
[`super::inventory`] is what is *in the core*, this one is what the source
calculates has got *into the coolant*, and [`super::airborne_release`] is
what it calculates gets *out* to the environment each year. The activity
the source calculates is *deposited on the primary-circuit surfaces*
(its Table 4) is a different quantity and is not digitised here.

# Basis of the source's calculation (so a reader knows what it includes)

- Twenty nuclides: ten noble gases, five iodines, Sr-89, Cs-134, Cs-137,
  Ag-110m and **H-3**. **C-14 is not in the table**, although the source's
  text states a primary-helium C-14 total (6.3×10^4 Bq); asking for it here
  returns `None`.
- The source credits helium purification (99 % for I, Kr, Xe, C and
  tritium; 90 % for Sr, Ag, Cs, Rb, which it calls conservative),
  plate-out of metals and iodine in the circuit, and a primary-helium
  leakage of 1 % of the inventory per day.

# What this is NOT

- **Not wired into any model.** Nothing in this crate or in `htgr_sim_v1`
  reads it.
- **Not a release.** Helium in the circuit is not effluent; a release
  needs a leak or discharge path, which is a modelling decision for the
  maintainer.
- **Not a dose**, and nothing here computes one (see the scope limit in
  [`crate::activity`]).
- `RESPONSIBLE_USE.md` applies: nothing here may be quoted as a coolant
  activity for HTR-10 or any other plant for any operational, licensing or
  safety purpose.

# Provenance

Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
environment impact for normal reactor operations and for relevant
accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 3
(p. 84), "Activities of important fission products in the primary helium
of the HTR-10 at the end of 20a lifetime of full power operation (Bq)".

Access terms and how the values were obtained (the maintainer's kovan
digitisation and an independent text-layer transcription, which agree on
all 20 values) are in `crates/changi/docs/References.md`. The document
carries no reuse licence and is **not** redistributed here; only the cited
table of 20 values is, which is ordinary scientific citation.

```rust
pub mod primary_helium { /* ... */ }
```

### Types

#### Struct `PrimaryHeliumActivityEntry`

One row of the published primary-helium activity.

```rust
pub struct PrimaryHeliumActivityEntry {
    pub nuclide: &'static str,
    pub activity: uom::si::f64::Radioactivity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `&'static str` | Nuclide label as the source tabulates it, e.g. `"Kr-88"`. |
| `activity` | `uom::si::f64::Radioactivity` | Activity circulating in the primary helium at the end of 20 years of<br>full-power operation. |

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
    fn clone(self: &Self) -> PrimaryHeliumActivityEntry { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PrimaryHeliumActivityEntry) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `htr10_primary_helium_end_of_life`

**Attributes:**

- `MustUse { reason: None }`

Every nuclide in the published HTR-10 end-of-life primary-helium activity.

Twenty entries, in the order the source tabulates them (Kr-85 before
Kr-85m, Xe-133 before Xe-133m, Xe-135 before Xe-135m — the reverse of its
Table 5's order for those pairs).

```rust
pub fn htr10_primary_helium_end_of_life() -> Vec<PrimaryHeliumActivityEntry> { /* ... */ }
```

#### Function `htr10_primary_helium_activity`

**Attributes:**

- `MustUse { reason: None }`

Look one nuclide's end-of-life primary-helium activity up by label.

Returns `None` for a nuclide the table does not list (including C-14,
whose total the source states in text but does not tabulate). Absent means
not reported, not zero.

```rust
pub fn htr10_primary_helium_activity(nuclide: &str) -> Option<uom::si::f64::Radioactivity> { /* ... */ }
```

## Module `chi_over_q`

Unit-release dilution factors, binned by travel time.

# Why a unit release

A consequence calculation needs the same dispersion answer for every nuclide
in the source term, differing only in how much was released and how fast it
decays on the way. Running the puff train once per nuclide would recompute
identical geometry ten or more times.

So it is run **once with unit mass** and the response scaled afterwards.
This is **exact, not an approximation**: the puff kernel is linear in mass
(pinned by `puff::concentration::tests::concentration_is_linear_in_puff_mass`)
and puffs superpose by summation, so

```text
chi(r; Q) = Q * chi(r; 1)
```

holds identically. It is the standard idiom in consequence codes.

# Why binned by travel time, and why the binning is exact

Decay in transit is **not** optional at these scales — Kr-89's half-life is
189 s against a default puff lifetime of 1200 s, so ignoring it overstates
the far field by roughly 80x. But the surviving fraction depends on how long
a puff has been travelling, which differs puff by puff, so a single scalar
`chi/Q` cannot carry it.

The fix is to keep the response resolved by puff **age**. A puff's age is
`elapsed - time_emitted`, and both are integer multiples of `sim_dt` —
`RunConfig` asserts that `puff_dt` is an integer multiple of `sim_dt`, which
is upstream's own documented requirement. So bin `a` carries travel time
*exactly* `a * sim_dt`, `exp(-lambda * a * sim_dt)` is exact, and nothing is
smeared across bins.

The reported dilution factor for nuclide `n` released in segment `s` is then

```text
chi/Q (r, s, n) = sum over a of  bins[r][s][a] * exp(-lambda_n * a * sim_dt)
```

# Known truncation: puffs are dropped at `puff_duration`

`RunConfig::puff_duration` (upstream default 1200 s) is a hard cutoff, not a
decay — a puff older than it simply stops existing. At 4 m/s that is 4800 m,
so a receptor at 10 km reads **exactly zero** with no error and no warning.
[`dilution_factors`] computes the reach and panics if *every* receptor is
beyond it, because an all-zero result is otherwise indistinguishable from a
correct answer. A mixed set is allowed through — check [`DilutionFactors::reach`].

```rust
pub mod chi_over_q { /* ... */ }
```

### Types

#### Enum `StabilitySource`

Where each puff's Pasquill stability class comes from.

Upstream derives the class *from* the wind speed and the hour, which means a
sweep cannot hold wind constant and vary stability — the very thing a
sensitivity study wants. [`Self::Fixed`] is the way to say it.

```rust
pub enum StabilitySource {
    FromWind,
    Fixed(crate::puff::stability::StabilityClass),
}
```

##### Variants

###### `FromWind`

**Default.** Derive it as upstream does, from the wind speed at the
moment of emission and [`RunConfig::start_hour`].

Six of the ten wind-speed/day-night regimes are ambiguous between two
adjacent classes. Whether the second one is also emitted is
[`RunConfig::emission_policy`]'s decision, not this enum's — pass
[`crate::puff::simulate::EmissionPolicy::UpstreamRecycleStabilityClasses`]
to reproduce upstream's doubling.

###### `Fixed`

Hold one class for the whole run, ignoring wind speed and hour.

The set is then unambiguous, so [`RunConfig::emission_policy`] has
nothing to recycle and both policies emit one puff per event.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::puff::stability::StabilityClass` |  |

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
    fn clone(self: &Self) -> StabilitySource { /* ... */ }
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
    fn default() -> StabilitySource { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &StabilitySource) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DilutionFactors`

Dilution factors for every receptor, release segment and travel-time bin.

Indexed `[receptor][segment][bin]`, each entry in **s/m^3 per unit activity
released by one source in that segment**. Multiple sources are summed into
the same entry, matching upstream's convention that one emission rate is
"applied uniformly to all sources" — so the activity a caller multiplies
back in is the release from **each** source, not the total over all of them.

Size is `n_receptors * n_segments * n_bins` f64s: for 20 receptors, 10
segments and 121 bins that is about 200 kB.

```rust
pub struct DilutionFactors {
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
  pub const fn n_receptors(self: &Self) -> usize { /* ... */ }
  ```
  Number of receptors, in the order supplied.

- ```rust
  pub const fn n_segments(self: &Self) -> usize { /* ... */ }
  ```
  Number of release segments, in the order the boundaries defined them.

- ```rust
  pub const fn n_bins(self: &Self) -> usize { /* ... */ }
  ```
  Number of travel-time bins. Bin `a` is travel time `a * sim_dt`.

- ```rust
  pub fn travel_time(self: &Self, bin: usize) -> Time { /* ... */ }
  ```
  The travel time bin `a` represents — exactly `a * sim_dt`.

- ```rust
  pub fn reach(self: &Self) -> Length { /* ... */ }
  ```
  How far a puff travels before `puff_duration` drops it, at the fastest

- ```rust
  pub fn bin(self: &Self, receptor: usize, segment: usize, bin: usize) -> DilutionFactor { /* ... */ }
  ```
  One bin's contribution, undecayed.

- ```rust
  pub fn dilution(self: &Self, receptor: usize, segment: usize, decay_constant: Frequency) -> DilutionFactor { /* ... */ }
  ```
  The dilution factor for one receptor and segment, with decay in transit.

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
    fn clone(self: &Self) -> DilutionFactors { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DilutionFactors) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `dilution_factors`

**Attributes:**

- `MustUse { reason: None }`

Run the puff train once with unit mass and accumulate the binned response.

# Arguments
- `sources` — one or more release points. Every source emits the same unit
  release, so the result is per unit released **by each source**.
- `segment_boundaries` — ascending times splitting the run into release
  segments, first element `0` and last the run duration; `n + 1` boundaries
  define `n` segments. A puff is attributed to the segment containing its
  emission time. One segment covering the whole run is the simple case.
- `wind` — one sample per simulation step, as [`crate::puff::simulate`].
- `receptors` — where the dilution factor is wanted.
- `config` — timing; `emission_policy` still decides whether an ambiguous
  stability class emits one puff or two.
- `stability` — see [`StabilitySource`].

# Panics
Panics if `sources`, `receptors` or the segment list is empty or malformed,
if `wind` is shorter than the number of steps, if `config` is inconsistent,
or if **every** receptor lies beyond [`DilutionFactors::reach`] (which would
silently return all zeros — see the module docs).

```rust
pub fn dilution_factors(sources: &[crate::puff::simulate::Source], segment_boundaries: &[uom::si::f64::Time], wind: &[crate::puff::wind::WindComponents], receptors: &[crate::puff::simulate::Receptor], config: &crate::puff::simulate::RunConfig, stability: StabilitySource) -> DilutionFactors { /* ... */ }
```

## Module `deposition`

Dry deposition of airborne activity onto the ground.

The model is the standard one-line dry-deposition relation: the activity
deposited per unit area is the deposition velocity times the
time-integrated air concentration at ground level,

```text
D [Bq/m^2] = v_d [m/s] * TIC(z = 0) [Bq.s/m^3]
```

# Four limits, none of them buried

- **Dry only.** Wet scavenging is not ported, so the answer is **not an
  upper bound** — rain would raise it. That is the opposite of the usual
  conservative framing and is the one most likely to be misread.
- **Diagnostic, not depleting.** Deposited activity is *not* removed from
  the plume, so the airborne concentration downwind of a deposition
  receptor is unchanged by it. At long range this over-predicts both the
  airborne concentration and the cumulative deposition.
- **Evaluated at `z = 0`, not breathing height.** `TIC` falls off with
  height, so feeding a 1.5 m receptor's `TIC` into this relation
  under-predicts deposition. Carry two receptor sets and pair them by
  index — the day-3 example does exactly that.
- **One velocity per group, no speciation.** A single elemental-iodine
  velocity over-predicts iodine deposition and under-predicts airborne
  iodine downwind, because organic iodides deposit far more slowly.

# Deposition velocities are NOT supplied as cited data

[`DryDepositionVelocity`] has **no default and no cited table**, and that is
deliberate. The values in general circulation for these three groups are
order-of-magnitude figures that vary with surface roughness, wind speed,
stability and particle size by more than an order of magnitude each; quoting
one from memory and attaching a reference to it would be exactly the
"tuned parameter wearing a citation" the workspace rules forbid.

So: **the caller supplies the velocity.** [`DryDepositionVelocity::new`]
takes a `Velocity` and says nothing about where it came from;
[`DryDepositionVelocity::order_of_magnitude_placeholder`] exists so the
end-to-end example can run, and is named so that no reader can mistake its
output for a sourced number.

Candidate sources to read before this module reports anything trustworthy —
**none of them has been consulted, none is in this workspace's literature
archive, and the placeholder values below were not taken from any of them**:
NUREG/CR-4691 (the MACCS2 model description), IAEA Safety Reports Series
No. 19, and Sehmel's 1980 deposition review. Tracked as a GitHub issue.

```rust
pub mod deposition { /* ... */ }
```

### Types

#### Enum `DepositionGroup`

How a nuclide behaves when it meets the ground.

Three groups, because dry deposition velocity spans roughly three orders of
magnitude across them and finer resolution is not supported by anything this
module knows.

**This is a DEPOSITION grouping and deliberately differs from `boon-lay`'s
transport `ElementGroup`.** Selenium (Z 34) and tellurium (Z 52) travel
through TRISO layers like halogens and are grouped with them for *release*;
once airborne they behave as condensed aerosols and are grouped here with
aerosols instead. Mapping one grouping onto the other would be wrong in one
of the two places, so they are kept separate and the divergence is named.

```rust
pub enum DepositionGroup {
    NobleGas,
    Halogen,
    Aerosol,
}
```

##### Variants

###### `NobleGas`

Chemically inert; does not deposit. Deposition velocity is exactly zero,
not merely small — a noble gas passes over the ground and leaves nothing.

###### `Halogen`

Reactive halogens — F, Cl, Br, I, At. The fastest-depositing group, and
the one where the lack of speciation hurts most: this treats all of it as
the elemental form, which deposits far faster than organic iodides.

###### `Aerosol`

Everything else, treated as a condensed particulate: caesium, strontium,
barium, silver, the lanthanides, and — see the note on this enum —
selenium and tellurium.

##### Implementations

###### Methods

- ```rust
  pub const fn from_atomic_number(z: u32) -> Self { /* ... */ }
  ```
  Classify by atomic number.

- ```rust
  pub const fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short label for printing.

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
    fn clone(self: &Self) -> DepositionGroup { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DepositionGroup) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DryDepositionVelocity`

A dry deposition velocity, in m/s.

Constructed from a value the **caller** justifies. See the module docs for
why this module ships no cited table.

```rust
pub struct DryDepositionVelocity(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Methods

- ```rust
  pub fn new(velocity: Velocity) -> Self { /* ... */ }
  ```
  From a velocity the caller has a source for.

- ```rust
  pub fn order_of_magnitude_placeholder(group: DepositionGroup) -> Self { /* ... */ }
  ```
  **NOT A CITED VALUE.** An order-of-magnitude placeholder so an

- ```rust
  pub fn meters_per_second(self: Self) -> f64 { /* ... */ }
  ```
  The value in metres per second.

- ```rust
  pub const fn velocity(self: Self) -> Velocity { /* ... */ }
  ```
  The underlying dimensioned velocity.

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
    fn clone(self: &Self) -> DryDepositionVelocity { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DryDepositionVelocity) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &DryDepositionVelocity) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `dry_deposition`

**Attributes:**

- `MustUse { reason: None }`

Dry deposition from a ground-level time-integrated air concentration.

`D = v_d * TIC`, giving Bq/m^2 from (m/s) times (Bq.s/m^3).

`air` must be the `TIC` evaluated at **`z = 0`**, not at breathing height —
see the module docs. Nothing here can check that, because a
[`TimeIntegratedAirConcentration`] does not carry the height it was
evaluated at.

```rust
pub fn dry_deposition(air: super::units::TimeIntegratedAirConcentration, velocity: DryDepositionVelocity) -> super::units::GroundDeposition { /* ... */ }
```

## Module `source`

The released source term: how much of what, over which windows.

This is the **input** to [`super::survey`], and the boundary at which
`sembawang` hands over. `changi` computes none of it — what gets out of the
fuel and out of the building is a severe-accident question, not a dispersion
one.

# This module holds no nuclide data, deliberately

There is no half-life table, no decay-constant table and no atomic-number
table here, and none may be added — the crate rule is that nuclide data
comes from `boon-lay` so the two cannot drift. A [`NuclideRelease`]
therefore carries its decay constant and its deposition group **as supplied
by the caller**, and its `label` is a printing label with no meaning to any
lookup.

The caller should take the decay constant from `boon-lay`'s
`TrisoAtopsNuclide::decay_constant()`, and **not** from
[`crate::flexpart::decay::decay_constant`], which carries upstream
FLEXPART's truncated `0.693147` in place of `ln 2`.

# Why windows carry an activity and not a rate

A release rate in Bq/s has dimension T^-2, because a becquerel is already
s^-1. `uom` will name that type, but nothing human reads it as a release
rate. So a window carries **how much was released during it**, and any
consumer that wants a rate divides by the window's own duration.

```rust
pub mod source { /* ... */ }
```

### Types

#### Struct `ReleaseWindow`

One release window: activity leaves the building at some unspecified
profile between `start` and `end`.

The model treats the release as **uniform over the window** — puffs are
emitted at a constant rate through it. A release that is strongly peaked
inside its window should be split into more, shorter windows rather than
described by one long one.

```rust
pub struct ReleaseWindow {
    pub start: uom::si::f64::Time,
    pub end: uom::si::f64::Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start` | `uom::si::f64::Time` | Start of the window, measured from the start of the dispersion run. |
| `end` | `uom::si::f64::Time` | End of the window. Must be strictly after `start`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(start: Time, end: Time) -> Self { /* ... */ }
  ```
  A window from `start` to `end`.

- ```rust
  pub fn duration(self: Self) -> Time { /* ... */ }
  ```
  How long the window lasts.

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
    fn clone(self: &Self) -> ReleaseWindow { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReleaseWindow) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `NuclideRelease`

One nuclide's release, resolved by window.

`released[i]` is the activity that left during `SourceTerm::windows[i]`, so
the two must be the same length.

```rust
pub struct NuclideRelease {
    pub label: String,
    pub decay_constant: uom::si::f64::Frequency,
    pub deposition_group: super::deposition::DepositionGroup,
    pub released: Vec<uom::si::f64::Radioactivity>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `label` | `String` | A printing label, e.g. `"I-131"`. Carries no meaning to any lookup —<br>see the module docs on why there is no nuclide table here. |
| `decay_constant` | `uom::si::f64::Frequency` | Radioactive decay constant, `ln(2) / half_life`. Supplied by the caller,<br>normally from `boon-lay`. |
| `deposition_group` | `super::deposition::DepositionGroup` | How this nuclide behaves on meeting the ground. Note this is a<br>*deposition* grouping and differs from `boon-lay`'s transport grouping<br>for Se and Te — see [`DepositionGroup`]. |
| `released` | `Vec<uom::si::f64::Radioactivity>` | Activity released in each window, in the same order as<br>[`SourceTerm::windows`]. |

##### Implementations

###### Methods

- ```rust
  pub fn total_released(self: &Self) -> Radioactivity { /* ... */ }
  ```
  Total activity released across every window.

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
    fn clone(self: &Self) -> NuclideRelease { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NuclideRelease) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SourceTerm`

A complete released source term: the windows, and every nuclide's release
resolved across them.

```rust
pub struct SourceTerm {
    pub windows: Vec<ReleaseWindow>,
    pub nuclides: Vec<NuclideRelease>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `windows` | `Vec<ReleaseWindow>` | Release windows, ascending and **contiguous** — see [`Self::validate`]. |
| `nuclides` | `Vec<NuclideRelease>` | One entry per nuclide. |

##### Implementations

###### Methods

- ```rust
  pub fn new(windows: Vec<ReleaseWindow>, nuclides: Vec<NuclideRelease>) -> Self { /* ... */ }
  ```
  Build and validate in one step.

- ```rust
  pub fn validate(self: &Self) { /* ... */ }
  ```
  Check the invariants the dispersion driver relies on.

- ```rust
  pub fn segment_boundaries(self: &Self) -> Vec<Time> { /* ... */ }
  ```
  The window boundaries, as [`super::chi_over_q::dilution_factors`] wants

- ```rust
  pub fn end(self: &Self) -> Time { /* ... */ }
  ```
  When the last window closes.

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
    fn clone(self: &Self) -> SourceTerm { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SourceTerm) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `survey`

Combine a released source term with dilution factors into per-receptor
air and ground totals.

This is the step that turns geometry into numbers:

```text
TIC(r, n) = sum over segments of  chi/Q(r, seg, lambda_n) * Q(n, seg)
D(r, n)   = v_d(group_n) * TIC_ground(r, n)
```

Everything expensive happened in [`super::chi_over_q`]; this is arithmetic
over the result, linear in the number of nuclides.

# Two receptor sets, paired by index

Air concentration is wanted at breathing height; deposition is evaluated at
`z = 0`. `TIC` falls off with height, so one receptor set cannot serve both
without under-predicting deposition. [`survey`] therefore takes **two**
[`super::chi_over_q::DilutionFactors`] — one computed at breathing height,
one at ground level — with receptor `r` meaning the same ground position in
both. The puff loop dominates the cost either way, and running it twice at
different heights is cheaper than getting the pairing wrong.

Nothing can check that the two were actually computed at different heights;
a `DilutionFactors` does not carry the height it was evaluated at. Passing
the same set twice is legal and gives deposition evaluated at breathing
height, which under-predicts.

```rust
pub mod survey { /* ... */ }
```

### Types

#### Struct `DepositionVelocities`

One dry deposition velocity per [`DepositionGroup`].

See [`DryDepositionVelocity`] for why this crate ships no cited table and
why the caller is expected to supply values it can justify.

```rust
pub struct DepositionVelocities {
    pub noble_gas: super::deposition::DryDepositionVelocity,
    pub halogen: super::deposition::DryDepositionVelocity,
    pub aerosol: super::deposition::DryDepositionVelocity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `noble_gas` | `super::deposition::DryDepositionVelocity` | For inert gases. Physically exactly zero. |
| `halogen` | `super::deposition::DryDepositionVelocity` | For F, Cl, Br, I, At. |
| `aerosol` | `super::deposition::DryDepositionVelocity` | For everything else, including Se and Te. |

##### Implementations

###### Methods

- ```rust
  pub fn order_of_magnitude_placeholder() -> Self { /* ... */ }
  ```
  **NOT CITED VALUES.** Order-of-magnitude placeholders, one per group —

- ```rust
  pub const fn for_group(self: &Self, group: DepositionGroup) -> DryDepositionVelocity { /* ... */ }
  ```
  The velocity for one group.

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
    fn clone(self: &Self) -> DepositionVelocities { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DepositionVelocities) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `NuclideTotals`

One nuclide's result at one receptor.

```rust
pub struct NuclideTotals {
    pub label: String,
    pub air: super::units::TimeIntegratedAirConcentration,
    pub ground: super::units::GroundDeposition,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `label` | `String` | The nuclide's printing label, copied from the source term. |
| `air` | `super::units::TimeIntegratedAirConcentration` | Time-integrated air concentration at **breathing height**, Bq.s/m^3. |
| `ground` | `super::units::GroundDeposition` | Dry ground deposition, Bq/m^2. Dry only, and **not an upper bound** —<br>see [`super::deposition`]. |

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
    fn clone(self: &Self) -> NuclideTotals { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NuclideTotals) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SiteSurvey`

Air and ground totals for every receptor and nuclide.

Indexed `[receptor][nuclide]`, in the order the receptors were given to
[`super::chi_over_q::dilution_factors`] and the nuclides to
[`SourceTerm`].

```rust
pub struct SiteSurvey {
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
  pub fn n_receptors(self: &Self) -> usize { /* ... */ }
  ```
  Number of receptors.

- ```rust
  pub fn at(self: &Self, receptor: usize) -> &[NuclideTotals] { /* ... */ }
  ```
  Every nuclide's totals at one receptor.

- ```rust
  pub fn total_air(self: &Self, receptor: usize) -> TimeIntegratedAirConcentration { /* ... */ }
  ```
  Air concentration summed over every nuclide at one receptor.

- ```rust
  pub fn total_ground(self: &Self, receptor: usize) -> GroundDeposition { /* ... */ }
  ```
  Ground deposition summed over every nuclide at one receptor. The same

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
    fn clone(self: &Self) -> SiteSurvey { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SiteSurvey) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `survey`

**Attributes:**

- `MustUse { reason: None }`

Scale dilution factors by a source term, per nuclide, and deposit.

# Arguments
- `source` — what was released, over which windows.
- `air_factors` — dilution factors at **breathing height**.
- `ground_factors` — dilution factors at **`z = 0`**, for deposition. See
  the module docs on why these are separate.
- `velocities` — one dry deposition velocity per group, supplied by the
  caller.

# Panics
Panics if the two factor sets disagree on the number of receptors or
segments, or if either has a different number of segments than the source
term has release windows.

```rust
pub fn survey(source: &super::source::SourceTerm, air_factors: &super::chi_over_q::DilutionFactors, ground_factors: &super::chi_over_q::DilutionFactors, velocities: &DepositionVelocities) -> SiteSurvey { /* ... */ }
```

#### Function `surviving_fraction_against_stable`

**Attributes:**

- `MustUse { reason: None }`

A dimensionless helper for reporting: how much of a nuclide's release
survived to a receptor, relative to a stable nuclide released identically.

Useful for showing the effect of decay in transit without quoting two
absolute numbers. Returns `None` when the stable reference is zero.

```rust
pub fn surviving_fraction_against_stable(decayed: super::units::TimeIntegratedAirConcentration, stable: super::units::TimeIntegratedAirConcentration) -> Option<f64> { /* ... */ }
```

#### Function `total_released`

**Attributes:**

- `MustUse { reason: None }`

Total activity released across every window and nuclide.

The same "a becquerel is not a becquerel" caveat as [`SiteSurvey::total_air`]
applies — this is an inventory figure, not a consequence one.

```rust
pub fn total_released(source: &super::source::SourceTerm) -> uom::si::f64::Radioactivity { /* ... */ }
```

## Module `units`

Newtypes for the three quantities this module reports.

All three are dimensioned, and `uom` does carry a quantity that matches each
one dimensionally. They are newtypes anyway, for a reason worth stating: the
`uom` quantity that matches reads as something else entirely.

| This | Unit | Dimensionally equal `uom` quantity | Why not that |
|---|---|---|---|
| [`DilutionFactor`] | s/m^3 | — (no named quantity) | `uom` has none |
| [`TimeIntegratedAirConcentration`] | Bq·s/m^3 | `VolumetricNumberDensity` (m^-3) | that counts *particles per volume*; this is *decays per volume*, integrated over time |
| [`GroundDeposition`] | Bq/m^2 | `ArealNumberRate` (m^-2 s^-1) | that is a *flux of countable things*; this is an activity per unit area |

Activity itself is **not** newtyped: it crosses public boundaries as
[`uom::si::f64::Radioactivity`], which has a built-in `@curie` unit, so
`3.7e10` never has to be written down.

Note that `boon-lay`'s `Activity` alias is `uom::si::f64::Frequency` — the
same dimension, a **different Rust type**. Converting between the two is
`sembawang`'s job and belongs in exactly one file there, not here.

```rust
pub mod units { /* ... */ }
```

### Types

#### Struct `DilutionFactor`

A dilution factor, `chi/Q`, in **s/m^3**.

The time-integrated air concentration at a receptor per unit activity
released at the source. Multiplying by an activity in Bq gives Bq·s/m^3;
see [`TimeIntegratedAirConcentration`].

It depends only on geometry, wind and stability — never on *what* was
released — which is what makes it worth computing once and reusing across
every nuclide. [`super::chi_over_q`] relies on that, and
`chi_over_q::tests::dilution_is_independent_of_the_activity_released` pins
it.

```rust
pub struct DilutionFactor(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Methods

- ```rust
  pub const fn new(seconds_per_cubic_meter: f64) -> Self { /* ... */ }
  ```
  From a bare value in seconds per cubic metre.

- ```rust
  pub const fn seconds_per_cubic_meter(self: Self) -> f64 { /* ... */ }
  ```
  The value in seconds per cubic metre.

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **AddAssign**
  - ```rust
    fn add_assign(self: &mut Self, rhs: Self) { /* ... */ }
    ```

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
    fn clone(self: &Self) -> DilutionFactor { /* ... */ }
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
    fn default() -> DilutionFactor { /* ... */ }
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

- **IntoEither**
- **Mul**
  - ```rust
    fn mul(self: Self, released: Radioactivity) -> TimeIntegratedAirConcentration { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DilutionFactor) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &DilutionFactor) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sum**
  - ```rust
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self { /* ... */ }
    ```

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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TimeIntegratedAirConcentration`

Time-integrated air concentration at a receptor, in **Bq·s/m^3**.

The integral of activity concentration over the whole passage of the plume.
It is the quantity a dose model would consume, and this crate deliberately
stops here: **no dose quantity is computed anywhere in `changi`**.

```rust
pub struct TimeIntegratedAirConcentration(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Methods

- ```rust
  pub const fn new(becquerel_seconds_per_cubic_meter: f64) -> Self { /* ... */ }
  ```
  From a bare value in becquerel-seconds per cubic metre.

- ```rust
  pub const fn becquerel_seconds_per_cubic_meter(self: Self) -> f64 { /* ... */ }
  ```
  The value in becquerel-seconds per cubic metre.

- ```rust
  pub fn mean_concentration_becquerel_per_cubic_meter(self: Self, over: Time) -> f64 { /* ... */ }
  ```
  The mean activity concentration over an averaging time, in Bq/m^3.

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **AddAssign**
  - ```rust
    fn add_assign(self: &mut Self, rhs: Self) { /* ... */ }
    ```

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
    fn clone(self: &Self) -> TimeIntegratedAirConcentration { /* ... */ }
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
    fn default() -> TimeIntegratedAirConcentration { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TimeIntegratedAirConcentration) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &TimeIntegratedAirConcentration) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sum**
  - ```rust
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self { /* ... */ }
    ```

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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GroundDeposition`

Activity deposited on the ground, in **Bq/m^2**.

Dry deposition only — wet scavenging is not ported, so this is **not an
upper bound**: rain would raise it. That is the opposite of the usual
conservative framing and must not be glossed over when a number is quoted.

```rust
pub struct GroundDeposition(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Methods

- ```rust
  pub const fn new(becquerel_per_square_meter: f64) -> Self { /* ... */ }
  ```
  From a bare value in becquerels per square metre.

- ```rust
  pub const fn becquerel_per_square_meter(self: Self) -> f64 { /* ... */ }
  ```
  The value in becquerels per square metre.

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **AddAssign**
  - ```rust
    fn add_assign(self: &mut Self, rhs: Self) { /* ... */ }
    ```

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
    fn clone(self: &Self) -> GroundDeposition { /* ... */ }
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
    fn default() -> GroundDeposition { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GroundDeposition) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &GroundDeposition) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sum**
  - ```rust
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self { /* ... */ }
    ```

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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `flexpart`

# `flexpart` — Rust port of FLEXPART's dispersion physics

FLEXPART is a Lagrangian particle dispersion model: it releases computational
particles, advects them on meteorological fields, perturbs them with
parameterised turbulence, and removes mass by dry deposition, wet deposition
and radioactive decay.

## What is ported so far

This module currently covers the **surface-layer and deposition scalar
kernels** — the pure functions that turn meteorological surface fields into
turbulence scales and aerosol deposition properties. These were chosen first
because every one of them depends only on `par_mod` constants in upstream
(verified by inspecting their `use` statements), so each can be called
directly from a Fortran driver and verified against the real FLEXPART with
no meteorological input files, no GRIB reader and no NetCDF.

| Submodule | Upstream files | Content |
|---|---|---|
| [`constants`] | `par_mod.f90` | Physical constants |
| [`thermo`] | `ew.f90`, `dynamic_viscosity.f90` | Saturation vapour pressure, dynamic viscosity |
| [`surface_layer`] | `psim.f90`, `psih.f90`, `scalev.f90`, `obukhov.f90`, `raerod.f90` | Monin–Obukhov similarity, friction velocity, aerodynamic resistance |
| [`aerosol`] | `part0.f90` | Lognormal size distribution, settling, Cunningham, Schmidt |
| [`decay`] | `readreleases.f90`, `timemanager.f90` | Radioactive decay |

## What is NOT ported

Everything else, which is most of FLEXPART: the particle advection loop
(`advance.f90`), the Hanna turbulence parameterisation, the convective
boundary-layer scheme (`cbl.f90`), wet scavenging (`wetdepo.f90`,
`get_wetscav.f90`), the Richardson-number mixing-height diagnostic, the
GRIB/NetCDF meteorological readers, the output grids, and the OH-reaction
chemistry. Do not read this module as "FLEXPART in Rust" — it is the first
verified slice of one.

## Precision, and why results differ from a stock FLEXPART build

FLEXPART's makefile passes no `-fdefault-real-8`, so its default `real` is
**single precision**. This port computes in `f64`. Agreement with an
as-shipped FLEXPART build is therefore bounded at roughly `1e-7` relative by
upstream's own storage format, not by any translation error. The
verification measures both: against a `-fdefault-real-8` build of the same
Fortran the port agrees to near machine precision, which is what isolates
the translation from the precision. See `docs/flexpart-code-to-code.md`.

## Intended use

Research, education and verification/validation only. See the crate-level
documentation for the scope limits, which are binding.

```rust
pub mod flexpart { /* ... */ }
```

### Modules

## Module `aerosol`

Lognormal aerosol size distribution: mass fractions, gravitational settling
velocities, Cunningham slip correction and Schmidt-number factors.

This is FLEXPART's `part0`, the routine that turns a two-parameter lognormal
aerosol description (mass median diameter and geometric standard deviation)
into the per-bin quantities the dry-deposition scheme needs.

# Reuse note

The error function comes from [`petir::specfunc::erf`], **not** from a port
of FLEXPART's own `erf.f90`. `petir` is this workspace's core numerics crate
and its `erf` is GSL-derived with its own test suite, so porting upstream's
Numerical-Recipes-style `erf` would have created a second, less-tested
implementation of a function the workspace already has — exactly the
duplication the workspace "reuse before porting" rule exists to prevent.

This *is* a deliberate numerical divergence from upstream and is measured:
see `docs/flexpart-code-to-code.md`.

```rust
pub mod aerosol { /* ... */ }
```

### Types

#### Struct `AerosolBins`

Per-bin properties of a lognormal aerosol size distribution.

Every field is an `NI`-element array (`NI = 11`, from `par_mod.f90`), one
entry per diameter class, ordered from the smallest class upward.

```rust
pub struct AerosolBins {
    pub mass_fraction: [f64; 11],
    pub schmidt_factor: [f64; 11],
    pub settling_velocity: [f64; 11],
    pub cunningham: [f64; 11],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mass_fraction` | `[f64; 11]` | Mass fraction in each diameter class, dimensionless.<br><br>These sum to slightly **less than 1**: the distribution is truncated at<br>±3 geometric standard deviations, so the tails outside that range are<br>discarded rather than redistributed. Upstream does not renormalise, and<br>neither does this port. |
| `schmidt_factor` | `[f64; 11]` | Schmidt-number factor `Sc^(-2/3)` for each class, dimensionless.<br><br>This is the combination the dry-deposition resistance formula consumes<br>directly, which is why upstream stores the power rather than `Sc` itself. |
| `settling_velocity` | `[f64; 11]` | Gravitational settling velocity of each class, m/s, as a **positive**<br>magnitude.<br><br>Stokes' law with the Cunningham slip correction:<br>`v_s = g rho d^2 C_c / (18 eta)`. Note the caller in `readreleases.f90`<br>accumulates `vsetaver = -sum(v_s · fract)`, i.e. it flips the sign to<br>make settling downward; this port keeps `part0`'s own positive<br>convention and leaves that choice to the caller. |
| `cunningham` | `[f64; 11]` | Cunningham slip correction factor of each class, dimensionless.<br><br># Upstream quirk, preserved and exposed<br><br>`part0.f90` declares `cun` as a **scalar** output but assigns it inside<br>the per-class loop, so on return it holds **only the last (largest)<br>class's value**. Its caller, `readreleases.f90:338`, then computes<br>`cunningham = sum_j(cun · fract_j)` — mass-weighting a value that is not<br>per-class, which collapses to `cun_last · sum(fract)`.<br><br>The port returns the genuine per-class array here, which is what the<br>physics needs, and exposes upstream's scalar separately as<br>[`AerosolBins::upstream_scalar_cunningham`] so the code-to-code test can<br>still compare like for like. |

##### Implementations

###### Methods

- ```rust
  pub fn upstream_scalar_cunningham(self: &Self) -> f64 { /* ... */ }
  ```
  The value upstream's scalar `cun` output actually carries: the Cunningham

- ```rust
  pub fn mean_settling_velocity(self: &Self) -> f64 { /* ... */ }
  ```
  Mass-weighted mean settling velocity, m/s, positive downward-magnitude.

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
    fn clone(self: &Self) -> AerosolBins { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AerosolBins) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `part0`

**Attributes:**

- `MustUse { reason: None }`

Split a lognormal aerosol into [`NI`] diameter classes and compute each
class's deposition-relevant properties.

Ports `part0.f90`. The distribution is cut at ±3 geometric standard
deviations about the mass median diameter and divided into `NI` equal
intervals in `log(d)`; for each interval the mass fraction comes from the
error function, and the settling velocity from Stokes' law with the
Cunningham slip correction:

```text
  Kn   = 2 lambda / d                       Knudsen number
  C_c  = 1 + Kn (1.257 + 0.4 exp(-1.1/Kn))  Cunningham slip correction
  D    = k_B T C_c / (3 pi eta d)           Brownian diffusivity
  Sc   = nu / D                             Schmidt number
  v_s  = g rho d^2 C_c / (18 eta)           settling velocity
```

# Fixed reference state

Upstream evaluates this at a **fixed** reference state, not at the ambient
conditions of the release: `T = 293.15 K`, dynamic viscosity
`eta = 1.81e-5 Pa·s`, kinematic viscosity `nu = 0.15e-4 m²/s`, mean free path
`lambda = 6.53e-8 m`. The settling velocities this returns are therefore
reference-state values; FLEXPART rescales them to ambient conditions later,
in `get_settling.f90`. That is worth knowing before using the output
directly.

# Arguments
- `mass_median_diameter` — `dquer`, in **micrometres**. Upstream's caller
  converts from metres immediately before the call
  (`readreleases.f90:331`), so this unit is the routine's real interface.
- `geometric_std_dev` — `dsigma`, dimensionless, `> 1`.
- `density` — particle density, kg/m³.

# Returns
Per-class [`AerosolBins`].

# Panics
Panics if `geometric_std_dev <= 1.0` (`log` would be non-positive, making
the bin edges meaningless) or if `mass_median_diameter <= 0.0`. Upstream has
no such guard and would return garbage or NaN.

```rust
pub fn part0(mass_median_diameter: f64, geometric_std_dev: f64, density: f64) -> AerosolBins { /* ... */ }
```

## Module `constants`

Physical constants, transcribed from FLEXPART's `par_mod.f90`.

# Why these are `f32`-valued constants written as `f64`

FLEXPART's makefile carries no `-fdefault-real-8`, so its default `real` is
**single precision**. `pi = 3.14159265` in `par_mod.f90` is therefore stored
as the `f32` value `3.14159274…`, not the `f64` value.

This port keeps the **decimal literals exactly as upstream writes them** and
evaluates them in `f64`. That is a deliberate choice: it ports the *intended*
constant rather than the rounding artefact of upstream's storage format, so
the port is at least as accurate as FLEXPART everywhere. The consequence is
that results differ from an as-shipped FLEXPART build at the `f32` level
(~1e-7 relative), which is measured and reported rather than assumed — see
`docs/flexpart-code-to-code.md`.

```rust
pub mod constants { /* ... */ }
```

### Constants and Statics

#### Constant `PI`

**Attributes:**

- `Other("#[allow(clippy::approx_constant)]")`

Circle constant, as written in `par_mod.f90` (`pi=3.14159265`).

Upstream truncates π to nine significant figures; this port keeps that exact
literal rather than substituting `std::f64::consts::PI`, so the arithmetic
matches the Fortran term for term.

```rust
pub const PI: f64 = 3.141_592_65;
```

#### Constant `R_EARTH`

Mean Earth radius, m (`r_earth=6.371e6`).

```rust
pub const R_EARTH: f64 = 6.371e6;
```

#### Constant `R_AIR`

Specific gas constant for dry air, J/(kg·K) (`r_air=287.05`).

```rust
pub const R_AIR: f64 = 287.05;
```

#### Constant `GA`

Standard gravitational acceleration, m/s² (`ga=9.81`).

```rust
pub const GA: f64 = 9.81;
```

#### Constant `CPA`

Specific heat capacity of dry air at constant pressure, J/(kg·K) (`cpa=1004.6`).

```rust
pub const CPA: f64 = 1004.6;
```

#### Constant `KAPPA`

Poisson constant `R/cp` for dry air (`kappa=0.286`).

Note upstream carries this *and* computes `r_air/cpa` separately in
`obukhov.f90`; the two differ slightly (0.286 vs 0.285_78…). The port
reproduces each site's own choice rather than unifying them.

```rust
pub const KAPPA: f64 = 0.286;
```

#### Constant `PI180`

Degrees-to-radians factor (`pi180=pi/180.`).

```rust
pub const PI180: f64 = _;
```

#### Constant `RGAS`

Universal gas constant, J/(mol·K) (`rgas=8.31447`).

```rust
pub const RGAS: f64 = 8.314_47;
```

#### Constant `R_WATER`

Specific gas constant for water vapour, J/(kg·K) (`r_water=461.495`).

```rust
pub const R_WATER: f64 = 461.495;
```

#### Constant `KARMAN`

Von Kármán constant as used by the surface-layer routines (`karman=0.40`).

`par_mod.f90` defines both `vonkarman=0.4` and `karman=0.40`. They are
numerically identical; the surface-layer routines use `karman`.

```rust
pub const KARMAN: f64 = 0.40;
```

#### Constant `HREF`

Reference height for dry deposition, m (`href=15.`).

```rust
pub const HREF: f64 = 15.0;
```

#### Constant `HMIXMIN`

Minimum allowed mixing height, m (`hmixmin=100.`).

```rust
pub const HMIXMIN: f64 = 100.0;
```

#### Constant `HMIXMAX`

Maximum allowed mixing height, m (`hmixmax=4500.`).

```rust
pub const HMIXMAX: f64 = 4500.0;
```

#### Constant `RHO_WATER`

Density of liquid water, kg/m³ (`rho_water=1000.`).

```rust
pub const RHO_WATER: f64 = 1000.0;
```

#### Constant `NI`

Number of aerosol diameter classes in the lognormal size distribution
(`ni=11`, `par_mod.f90:222`).

`part0` splits a lognormal distribution into this many bins spanning
±3 geometric standard deviations about the mass median diameter.

```rust
pub const NI: usize = 11;
```

## Module `decay`

Radioactive decay of airborne and deposited activity.

FLEXPART treats decay as a per-species exponential applied to particle mass
and to deposited mass. It is the piece of FLEXPART that makes it a
*radionuclide* dispersion model rather than a generic tracer model, and it is
why FLEXPART is the right upstream for CHANGI.

# Relationship to `boon-lay`

`boon-lay`'s `triso_atops_fork` also models radioactive decay, from the
TRISO-ATOPS lineage, and its nuclide database carries half-lives from the
IAEA Live Chart. That database is the natural source for the half-lives fed
into these functions — this module deliberately holds **no nuclide data of
its own**, so the two cannot drift apart.

```rust
pub mod decay { /* ... */ }
```

### Functions

#### Function `decay_constant`

**Attributes:**

- `MustUse { reason: None }`

Convert a half-life to a decay constant, `lambda = ln2 / t_half`.

Ports `readreleases.f90:317`, `decay(i) = 0.693147/decay(i)`.

# Upstream's truncated `ln 2`

Upstream hard-codes `0.693147`, a six-decimal truncation of
`ln 2 = 0.6931471805…`. This port keeps that literal rather than using
[`core::f64::consts::LN_2`], so the decay constant matches FLEXPART term for
term; substituting the exact value would shift every decay constant by
~2.6e-7 relative and silently break code-to-code agreement. Use
[`decay_constant_exact`] where accuracy matters more than fidelity.

# Arguments
- `half_life_seconds` — `t_half`, s. Must be `> 0`.

# Returns
Decay constant `lambda`, s⁻¹.

# Panics
Panics if `half_life_seconds <= 0.0`. Upstream has no guard and would divide
by zero, yielding an infinite decay constant that silently annihilates the
species.

```rust
pub fn decay_constant(half_life_seconds: f64) -> f64 { /* ... */ }
```

#### Function `decay_constant_exact`

**Attributes:**

- `MustUse { reason: None }`

As [`decay_constant`], but with the exact `ln 2`.

Differs from upstream by about 2.6e-7 relative. Offered because the
truncation in [`decay_constant`] is a fidelity choice, not a physics one, and
a caller doing its own dose work should not be forced to inherit it.

```rust
pub fn decay_constant_exact(half_life_seconds: f64) -> f64 { /* ... */ }
```

#### Function `surviving_fraction`

**Attributes:**

- `MustUse { reason: None }`

Surviving fraction after `dt` seconds of decay: `exp(-lambda dt)`.

Ports the form applied in `timemanager.f90:275`,
`exp(-1.*outstep*decay(ks))`.

# Arguments
- `decay_constant` — `lambda`, s⁻¹, as returned by [`decay_constant`]. A
  value of `0` means a stable species and returns exactly `1`.
- `dt_seconds` — elapsed time, s.

# Returns
Surviving fraction in `(0, 1]` for non-negative `dt`.

# Note on FLEXPART's guard
`timemanager.f90:267` applies decay only when `decay(ks) > 0`, treating
non-positive constants as "stable" rather than as growth. This function
mirrors that: a negative `lambda` would otherwise produce unphysical growth,
so it is rejected.

# Panics
Panics if `decay_constant` is negative.

```rust
pub fn surviving_fraction(decay_constant: f64, dt_seconds: f64) -> f64 { /* ... */ }
```

#### Function `decayed`

**Attributes:**

- `MustUse { reason: None }`

Apply decay to a mass or activity over `dt` seconds.

Convenience wrapper: `amount · exp(-lambda dt)`.

# Arguments
- `amount` — mass (kg) or activity (Bq); the function is linear so either
  works, provided the caller is consistent.
- `decay_constant` — `lambda`, s⁻¹.
- `dt_seconds` — elapsed time, s.

```rust
pub fn decayed(amount: f64, decay_constant: f64, dt_seconds: f64) -> f64 { /* ... */ }
```

## Module `surface_layer`

Monin–Obukhov surface-layer similarity: stability corrections, the Obukhov
length, friction velocity and aerodynamic resistance.

These five routines are what turn raw meteorological surface fields into the
turbulence scales that the dispersion and dry-deposition schemes need. All
of them are pure functions of scalars in upstream — none touches a
meteorological field array — which is why they are the first module ported
and the first verified against the Fortran.

# Sign convention

The Obukhov length `L` is **negative for unstable** stratification (upward
surface heat flux) and **positive for stable**. Both stability functions
branch on the sign of `z/L`, and they do **not** use the same empirical
constants as each other — see each function's notes.

```rust
pub mod surface_layer { /* ... */ }
```

### Types

#### Enum `MetDataFormat`

Which meteorological product the input fields came from.

`obukhov.f90` branches on this: for ECMWF input it reconstructs the level-1
pressure from the model's hybrid coefficients, whereas for NCEP/GFS input
the caller supplies that pressure directly.

Modelled as an enum rather than upstream's bare integer flag so a caller
cannot pass a meaningless value, and so adding a third product forces every
`match` site to be revisited (workspace Rust design rule: enums for dispatch,
never trait objects).

```rust
pub enum MetDataFormat {
    Ecmwf {
        akm: [f64; 2],
        bkm: [f64; 2],
    },
    Ncep,
}
```

##### Variants

###### `Ecmwf`

ECMWF: level-1 pressure is rebuilt from the hybrid `A`/`B` coefficients.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `akm` | `[f64; 2]` | `A` coefficients of the two lowest model half-levels, Pa. |
| `bkm` | `[f64; 2]` | `B` coefficients of the two lowest model half-levels, dimensionless. |

###### `Ncep`

NCEP/GFS: the caller supplies the level-1 pressure directly.

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
    fn clone(self: &Self) -> MetDataFormat { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MetDataFormat) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `psim`

**Attributes:**

- `MustUse { reason: None }`

Monin–Obukhov stability correction for **momentum**, `psi_m`.

Ports `psim.f90`. Dimensionless.

```text
  zeta = z / L
  zeta <= 0 (unstable):  x = (1 - 15 zeta)^{1/4}
                         psi_m = ln[((1+x)/2)^2 · (1+x^2)/2] - 2 atan(x) + pi/2
  zeta >  0 (stable):    psi_m = -4.7 zeta
```

The unstable branch is the Businger–Dyer/Paulson form; the stable branch is
the linear Businger form with coefficient 4.7.

# Arguments
- `z` — height above the surface, m.
- `obukhov_length` — Obukhov length `L`, m (negative = unstable).

# Returns
The dimensionless correction `psi_m`.

# Note on `zeta == 0`
Upstream tests `zeta.le.0.`, so exactly-zero `zeta` (neutral) takes the
*unstable* branch, where `x = 1` and the expression collapses to
`ln(1) - 2·atan(1) + pi/2 = 0`. The port keeps that branch ordering so the
neutral limit is reached identically.

```rust
pub fn psim(z: f64, obukhov_length: f64) -> f64 { /* ... */ }
```

#### Function `psih`

**Attributes:**

- `MustUse { reason: None }`

Monin–Obukhov stability correction for **heat**, `psi_h`.

Ports `psih.f90`. Dimensionless.

```text
  zeta > 0 (stable):    psi_h = -(1 + 0.667 a zeta)^{3/2}
                                - b (zeta - c/d) exp(-d zeta) - b c/d + 1
  zeta <= 0 (unstable): x = (1 - 16 zeta)^{1/4}
                        psi_h = 2 ln[(1 + x^2)/2]
```

with upstream's `a = 1`, `b = 0.667`, `c = 5`, `d = 0.35` — the Beljaars–Holtslag
stable form. Note the unstable branch uses **16**, where [`psim`] uses **15**;
that asymmetry is upstream's and is preserved deliberately.

# Two guards ported verbatim

1. **Near-zero `L` is nudged away from zero**, to `±1e-20` matching its sign,
   so the division cannot produce an infinity. Upstream mutates its `l`
   argument in place to do this (Fortran arguments are by reference); this
   port takes `L` by value and nudges its local copy, which is the same
   arithmetic without the caller-visible side effect. **That side effect is a
   real behavioural difference**: an upstream caller passing a variable with
   `|L| < 1e-20` would find it modified on return. No in-tree caller relies
   on it (`raerod.f90` passes its own `l` straight through), so the port
   drops it — recorded here rather than silently.
2. **Far-field short circuit**: when `log10(z) - log10(|L|) < log10(1e-20)`,
   i.e. `z` is more than twenty decades below `|L|`, `psi_h` is exactly zero.

# Arguments
- `z` — height above the surface, m.
- `obukhov_length` — Obukhov length `L`, m (negative = unstable).

# Returns
The dimensionless correction `psi_h`.

```rust
pub fn psih(z: f64, obukhov_length: f64) -> f64 { /* ... */ }
```

#### Function `scalev`

**Attributes:**

- `MustUse { reason: None }`

Friction velocity `u*` from the surface momentum stress, m/s.

Ports `scalev.f90`:

```text
  e    = e_w(T_d)                  saturation vapour pressure at dew point
  T_v  = T (1 + 0.378 e / p)       virtual temperature
  rho  = p / (R_air T_v)           moist-air density
  u*   = sqrt(|tau| / rho)
```

# Arguments
- `surface_pressure` — `p`, Pa.
- `temperature` — surface air temperature `T`, K.
- `dew_point` — surface dew-point temperature `T_d`, K.
- `stress` — surface momentum stress `tau`, N/m². The absolute value is
  taken, so sign conventions on the input do not matter.

# Returns
Friction velocity `u*`, m/s. Always non-negative.

```rust
pub fn scalev(surface_pressure: f64, temperature: f64, dew_point: f64, stress: f64) -> f64 { /* ... */ }
```

#### Function `obukhov`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Obukhov length `L`, m.

Ports `obukhov.f90`:

```text
  e        = e_w(T_d,surf)
  T_v      = T_surf (1 + 0.378 e / p_s)
  rho      = p_s / (R_air T_v)
  theta    = T_lev (100000 / p_lev)^{R_air/cp}      potential temperature
  theta*   = H / (rho cp u*)                        scale temperature
  L        = theta u*^2 / (karman g theta*)
```

# Three guards ported verbatim

1. `u* <= 0` is raised to `1e-8` before use, preventing division by zero.
2. When `|theta*| <= 1e-10` — effectively zero surface heat flux — `L` is set
   to the sentinel `9999`, **not** to infinity.
3. `L` is then clamped to `[-9999, 9999]`. So `9999` is doing double duty as
   both "neutral" and "clamped"; a caller cannot distinguish them, and that
   ambiguity is upstream's.

# Arguments
- `surface_pressure` — `p_s`, Pa.
- `surface_temperature` — `T_surf`, K.
- `surface_dew_point` — `T_d,surf`, K.
- `level_temperature` — `T_lev`, temperature at the first model level, K.
- `ustar` — friction velocity `u*`, m/s (see [`scalev`]).
- `surface_heat_flux` — `H`, sensible heat flux, W/m² (positive upward).
- `level_pressure` — `p_lev`, Pa. **Used only for [`MetDataFormat::Ncep`]**;
  for ECMWF it is recomputed from the hybrid coefficients and the value
  passed here is ignored.
- `format` — which product the fields came from.

# Returns
Obukhov length `L` in metres, clamped to `[-9999, 9999]`; negative is
unstable, positive stable, `9999` means "no usable heat flux".

```rust
pub fn obukhov(surface_pressure: f64, surface_temperature: f64, surface_dew_point: f64, level_temperature: f64, ustar: f64, surface_heat_flux: f64, level_pressure: f64, format: MetDataFormat) -> f64 { /* ... */ }
```

#### Function `raerod`

**Attributes:**

- `MustUse { reason: None }`

Aerodynamic resistance `r_a` between the surface and the reference height,
s/m.

Ports `raerod.f90`:

```text
  r_a = [ ln(h_ref / z0) - psi_h(h_ref, L) + psi_h(z0, L) ] / (karman u*)
```

with `h_ref = 15 m` ([`HREF`], `par_mod.f90`). This is the first of the three
resistances in the standard resistance analogy for dry deposition.

# Arguments
- `obukhov_length` — `L`, m.
- `ustar` — friction velocity, m/s.
- `roughness_length` — surface roughness length `z0`, m. Must be `> 0`.

# Returns
Aerodynamic resistance, s/m.

# Note
Upstream applies no guard on `u* = 0` or `z0 = 0` here, so both produce a
non-finite result. The port reproduces that rather than inventing a guard
upstream does not have; callers are expected to supply a positive `u*` from
[`scalev`] and a positive `z0` from the land-use table.

```rust
pub fn raerod(obukhov_length: f64, ustar: f64, roughness_length: f64) -> f64 { /* ... */ }
```

## Module `thermo`

Moist-air thermodynamic properties used by the surface-layer and deposition
schemes.

Two kernels, both pure functions of temperature, both taken from FLEXPART
files that have no module dependencies at all.

```rust
pub mod thermo { /* ... */ }
```

### Functions

#### Function `saturation_vapour_pressure`

**Attributes:**

- `MustUse { reason: None }`

Saturation water-vapour pressure over liquid water, in pascals.

Ports `ew.f90`. This is the **Goff–Gratch** formulation, expressed against
the steam point `T_st = 373.16 K`:

```text
  y = 373.16 / T
  a = -7.90298 (y - 1) + 5.02808 · log10(y)
  c = -1.3816e-7 · (10^(11.344 (1 - 1/y)) - 1)
  d =  8.1328e-3 · (10^(-3.49149 (y - 1)) - 1)
  e_w = 101324.6 · 10^(a + c + d)      [Pa]
```

# Arguments
- `temperature` — air (or dew-point) temperature. Must be **strictly above
  absolute zero**; upstream halts the program on `T <= 0 K`.

# Returns
Saturation vapour pressure as a [`Pressure`] (pascals).

# Valid range
The Goff–Gratch fit is over liquid water and is intended for roughly
223–373 K. It is *not* the ice-phase formulation, and upstream applies no
range check beyond the `T > 0` guard, so out-of-range inputs return an
extrapolated value rather than an error.

# Panics
Panics if `temperature <= 0 K`, mirroring upstream's
`stop 'sorry: t not in [k]'`. A Rust caller reaching this has passed a
physically impossible temperature.

```rust
pub fn saturation_vapour_pressure(temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `ew_kelvin`

**Attributes:**

- `MustUse { reason: None }`

Bare-`f64` form of [`saturation_vapour_pressure`]: kelvin in, pascals out.

Exposed because the surface-layer routines call it in tight arithmetic where
the `uom` wrapper adds nothing, and because the code-to-code test drives it
directly against the Fortran.

```rust
pub fn ew_kelvin(t_kelvin: f64) -> f64 { /* ... */ }
```

#### Function `dynamic_viscosity_of_air`

**Attributes:**

- `MustUse { reason: None }`

Dynamic viscosity of air, in Pa·s.

Ports `dynamic_viscosity.f90` — **Sutherland's law**:

```text
  eta(T) = eta_0 · (T_0 + C)/(T + C) · (T/T_0)^{3/2}
```

with upstream's constants `C = 120 K`, `T_0 = 291.15 K`,
`eta_0 = 1.827e-5 Pa·s`.

# Arguments
- `temperature` — air temperature.

# Returns
Dynamic viscosity as a [`DynamicViscosity`] (Pa·s).

# Valid range
Sutherland's law is accurate to a few per cent over roughly 200–1000 K for
air. Upstream applies no range check.

```rust
pub fn dynamic_viscosity_of_air(temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::DynamicViscosity { /* ... */ }
```

#### Function `viscosity_kelvin`

**Attributes:**

- `MustUse { reason: None }`

Bare-`f64` form of [`dynamic_viscosity_of_air`]: kelvin in, Pa·s out.

```rust
pub fn viscosity_kelvin(t_kelvin: f64) -> f64 { /* ... */ }
```

## Module `puff`

# `puff` — Gaussian puff forward dispersion

A Gaussian *puff* model discretises a continuous release into a train of
discrete puffs. Each puff is emitted at a fixed interval, carries a fixed
mass, is advected by the wind, and spreads by a Pasquill–Gifford dispersion
coefficient that grows with the distance it has travelled. The concentration
at a receptor is the sum over all puffs still alive.

~~"is advected by the wind sampled at its moment of emission"~~
**CORRECTED 2026-09-27.** That was upstream's rule and is now this port's
*ablation*, not its default: see [`simulate::AdvectionPolicy`]. By default
each puff integrates its own trajectory on the wind that actually blows, so
it turns when the wind turns, and its dispersion distance is the **path
length** it has travelled rather than the straight-line distance from the
source. The two are identical on a steady wind and differ on every other,
which is why the old wording read as true for as long as it did. Recorded as
upstream defect 5 in `docs/puff-code-to-code.md`.

This complements [`crate::flexpart`] rather than duplicating it. FLEXPART is
a Lagrangian *particle* model driven by gridded meteorology; this is an
analytic puff model driven by a single wind time series. The puff model is
cheap enough to run interactively over a site-sized domain and needs no
meteorological files, which makes it the natural near-field complement to a
particle model built for synoptic scales.

## Module map (R → Rust)

| Upstream `R/` | Rust | Content |
|---|---|---|
| `helpers.R` — `is_day`, `get_stab_class` | [`stability`] | Pasquill stability classification |
| `helpers.R` — `compute_sigma_vals` | [`dispersion`] | Pasquill–Gifford `sigma_y`, `sigma_z` |
| `helpers.R` — `wind_vector_convert`, `interpolate_wind_data` | [`wind`] | Met-convention wind handling |
| `helpers.R` — `gpuff` | [`concentration`] | The Gaussian puff kernel itself |
| `simulate_sensor_mode.R`, `simulate_grid_mode.R` | [`simulate`] | The two run modes |

[`climatology`] has **no upstream** — it is this crate's own, holding the
illustrative Singapore wind conditions the examples run at, with their
provenance. It is not part of the ported physics and is not covered by the
code-to-code verification.

## What is NOT ported

**`R/plots.R` (1 077 of upstream's 2 296 lines — 47 % of the package).**
Upstream describes itself as "primarily a visualization-focused package";
everything in `plots.R` is `ggplot2`/`plotly` chart construction, with no
physics. Porting it would contradict two workspace rules at once — non-GUI
library code must build for Android and for `wasm32`, and a headless library
does not own its caller's plotting. The physics is the whole of the
remaining 1 219 lines, and that is what is here.

Also not ported: R's `POSIXct` handling. Upstream classifies day/night by
formatting a timestamp with `"%H"`; this port takes an hour-of-day integer
directly, which is the only part of the timestamp that the physics reads.

## Intended use

Research, education and verification/validation only, exactly as for the
rest of this crate — see the crate-level documentation, whose scope limits
are binding. Upstream's application domain is oil-and-gas methane leak
detection; CHANGI's is radionuclide transport. The dispersion mathematics is
species-independent, but **the unit conversion is not**: upstream's
`gpuff` returns parts-per-million *of methane*. See
[`concentration::METHANE_PPM_PER_KG_PER_M3`] for why that factor must not be
reused for another species.

## Status

**Untrusted AI-assisted draft. No human V&V.** Verified code-to-code against
the upstream R at commit `5213d58` — see `docs/puff-code-to-code.md`. That
establishes the translation is faithful; it says nothing about whether the
model reproduces measured dispersion.

```rust
pub mod puff { /* ... */ }
```

### Modules

## Module `wgsl`

**The Gaussian puff field as a GPU kernel**, plus its `f32` CPU mirror and
a dedicated CPU thread pool.

The Map tab wants a 64 x 64 field refreshed at **10 Hz**. At
`htgr_sim_v1`'s own working point -- 7 260 puffs, from emitting every 10 s
over a 1 200 s run -- that is ~30 million kernel evaluations per field.

**Measured 2026-09-24**, 16 logical cores, one adapter present, by
`cargo run --release -p changi --example field_timing [--features gpu]`
(median of 5 after a warm-up, the simulator's own 64x64 grid over
+/-1250 m at 50 m):

| path | 1 000 puffs | **7 260 puffs** | 20 000 puffs | 50 000 puffs |
|---|---|---|---|---|
| [`field_serial`], one core | 18.7 ms | **136 ms** | 369 ms | 890 ms |
| [`field_pooled`], 16 cores | 2.3 ms | **15.9 ms** | 43.0 ms | 104 ms |
| [`field_gpu`] | 0.57 ms | **1.98 ms** | 12.8 ms | 11.6 ms |

So at the working point the pooled CPU path takes **~16 ms**, comfortably
inside both 10 Hz (100 ms) and `htgr_sim_v1`'s `PHYSICS_TICK`. The GPU is
~8x faster again and is what one would reach for at 20 000+ puffs, but it
is **not** required to meet the 10 Hz target.

> ~~"one CPU core ~2.2 s per field; [`field_pooled`] on 16 cores ~140 ms,
> marginal; GPU single-digit ms -- so the GPU path is the one that meets
> the requirement and the pooled CPU path is the fallback."~~
> **CORRECTED 2026-09-24.** The pooled row was wrong by ~9x: 140 ms is
> close to the *serial* cost at the working point (136 ms), not the pooled
> one (15.9 ms), so the table appears to have recorded a one-core timing in
> the sixteen-core row. The correction matters because that row was the
> sole argument for treating a map refresh as unaffordable inside a physics
> tick, and it is not. The old numbers carried no reproducible instrument;
> `examples/field_timing.rs` now is one.

**Both paths must exist regardless**: `outram-mc-libs/CLAUDE.md`'s GPU
policy is a hard rule across this workspace -- CI must never fail for want
of a GPU, detection is at run time, and the CPU path stays mandatory and
trusted. Here the CPU path is also the *reference*: it is what the GPU
result is checked against.

# PETIR runs the shader; changi does not own any wgpu plumbing

[`GAUSSIAN_PUFF_FIELD`] declares WGSL **functions**, in the same style as
`petir::wgsl::POLY`, and is dispatched by
`petir::wgsl::gpu::GpuContext::eval_map`. PETIR's own docs record why:
leaving each consumer to write "the same 150 lines of buffer plumbing" is
how they all get the bind-group layout "subtly wrong in the same way", and
it names a real defect that caused. changi therefore gains a GPU path
**without gaining a `wgpu` dependency** -- the feature stays behind
`petir/wgpu`, off by default, and a host with no adapter simply uses
[`field_pooled`].

# Why the puff states are flattened on the CPU first

A puff's `sigma_y`/`sigma_z` come from a Pasquill-Gifford table walk --
branchy, cache-unfriendly, everything a shader is worst at. But they are
needed **once per puff**, not once per (puff, cell), so evaluating them on
the CPU costs 4096 times less than doing it in the kernel. What crosses to
the GPU is the finished state: position, both sigmas, and the mass-time
weight. The shader then does nothing but arithmetic.

# `f32`, and where the f64 path remains the reference

The field drives a colour ramp over four decades and `f32` carries about
seven decimal digits, so single precision is ample for the picture. It is
**not** what a quoted number comes from: `chi/Q` for the receptor ring
stays on the f64
[`super::concentration::gaussian_puff_concentration`] path, which is the
one carrying the analytical verification.

```rust
pub mod wgsl { /* ... */ }
```

### Types

#### Struct `PuffState`

One puff, already reduced to what the kernel needs.

Deliberately **not** a puff as the simulator holds one: no emission time,
no travel distance, no stability class. Those are what produce `sigma_y`
and `sigma_z`, and they have been spent by the time a state is built.

```rust
pub struct PuffState {
    pub x: f32,
    pub y: f32,
    pub sigma_y: f32,
    pub sigma_z: f32,
    pub weight: f32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f32` | Puff centre easting \[m\]. |
| `y` | `f32` | Puff centre northing \[m\]. |
| `sigma_y` | `f32` | Crosswind dispersion at this puff's travel distance \[m\]. |
| `sigma_z` | `f32` | Vertical dispersion at this puff's travel distance \[m\]. |
| `weight` | `f32` | Mass times the time weight being integrated with \[kg s\], so a sum of<br>contributions is already a time-integrated quantity. |

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
    fn clone(self: &Self) -> PuffState { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PuffState) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FieldGrid`

The grid a field is evaluated on.

```rust
pub struct FieldGrid {
    pub cells: usize,
    pub half_width_m: f32,
    pub source_height_m: f32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cells` | `usize` | Cells per side; the grid is square. |
| `half_width_m` | `f32` | Half-width of the covered square \[m\]. |
| `source_height_m` | `f32` | Release height \[m\]. |

##### Implementations

###### Methods

- ```rust
  pub fn cell_centre(self: &Self, column: usize, row: usize) -> (f32, f32) { /* ... */ }
  ```
  The `(easting, northing)` of a cell's **centre**, in metres.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Total cells.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the grid has no cells.

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
    fn clone(self: &Self) -> FieldGrid { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FieldGrid) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `pack_states`

Flatten puff states into the `f32` array the shader reads as `src`.

The layout is the shader's contract; `STATE_STRIDE` is the one place the
stride is written on this side.

```rust
pub fn pack_states(states: &[PuffState]) -> Vec<f32> { /* ... */ }
```

#### Function `contribution`

One puff state's contribution at a ground-level receptor — the `f32` CPU
mirror of the shader's `changi_puff_contribution`.

At `z = 0` the real and image terms coincide, so the vertical factor is
`2 exp(-H^2 / 2 sigma_z^2)`. Written out as a sum rather than folded to
the factor of two, matching the shader line for line so the two can be
read side by side.

```rust
pub fn contribution(state: &PuffState, easting: f32, northing: f32, height_m: f32) -> f32 { /* ... */ }
```

#### Function `field_cell`

The field value at one cell — the `f32` CPU mirror of the shader's
`changi_puff_field`.

```rust
pub fn field_cell(states: &[PuffState], grid: &FieldGrid, column: usize, row: usize) -> f32 { /* ... */ }
```

#### Function `field_serial`

The whole field, single-threaded. The reference the GPU and the pooled
path are both checked against.

```rust
pub fn field_serial(states: &[PuffState], grid: &FieldGrid) -> Vec<f32> { /* ... */ }
```

#### Function `field_pooled`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_arch\", value: Some(\"wasm32\"), span: crates/changi/src/puff/wgsl.rs:239:11: 239:33 (#0) }, crates/changi/src/puff/wgsl.rs:239:10: 239:34 (#0))])]")`

The whole field on changi's own thread pool.

The CPU fallback for hosts with no usable GPU adapter. Parallel over
**rows**, not cells: a row is 64 contributions-sums of identical cost, so
the chunks are even and large enough that the scheduling overhead
disappears against the work.

```rust
pub fn field_pooled(states: &[PuffState], grid: &FieldGrid) -> Vec<f32> { /* ... */ }
```

#### Function `field_auto`

The field, GPU-accelerated when the `gpu` feature is on and a usable
adapter is found, falling back to [`field_pooled`] otherwise.

This is what a caller should reach for. With the `gpu` feature off this
compiles down to [`field_pooled`] alone, with no `cfg` visible at the call
site. The CPU path is not a stopgap: `outram-mc-libs/CLAUDE.md`'s GPU
policy makes it mandatory and trusted, and here it is also the reference
the GPU result is checked against -- see [`field_serial`] and the
GPU-vs-CPU comparison test below.

```rust
pub fn field_auto(states: &[PuffState], grid: &FieldGrid) -> Vec<f32> { /* ... */ }
```

#### Function `has_gpu_field`

Whether [`field_auto`] will actually take the GPU path on this host.

# Why a caller needs to ask

[`field_auto`] always returns the right answer, but not at the same
*cost*: the CPU path is measured in this module's doc at ~140 ms on 16
cores and ~2.2 s on one, for the 64x64 grid. A caller on a real-time
budget -- `htgr_sim_v1`'s physics thread has 100 ms per tick -- therefore
cannot afford to call [`field_auto`] at map cadence unless the GPU path is
genuinely available, and "is the `gpu` feature on" is not the same
question: the feature can be on with no usable adapter behind it.

This probes the adapter (once, cached in [`gpu_context`]) rather than
reporting the feature flag, so it answers what the caller actually needs
to know. `false` with the feature off, `false` with the feature on and no
adapter, `false` on Android and wasm where `petir::wgsl::gpu` is itself
gated out.

This is a scheduling hint and nothing more. It must never select a
different *model*: both paths compute the same field, and
`field_gpu_agrees_with_the_serial_reference` pins that to 1e-4 relative.

```rust
pub fn has_gpu_field() -> bool { /* ... */ }
```

### Constants and Statics

#### Constant `GAUSSIAN_PUFF_FIELD`

The WGSL source. Declares `changi_puff_field` and
`changi_puff_contribution`; see the file header for the `src`/`params`
layout contract it shares with `eval_map`.

```rust
pub const GAUSSIAN_PUFF_FIELD: &str = "// SPDX-License-Identifier: GPL-3.0\n//\n// The Gaussian puff concentration kernel, in WGSL (f32).\n//\n// TRANSCRIBED from `changi::puff::concentration::gaussian_puff_concentration`,\n// which is itself a port of the R `puff` package\'s `gpuff`. This shader and\n// that function must agree; `super::wgsl::field_cell` is the f32 CPU mirror\n// that lets them be compared with no GPU present, and the tests assert the\n// mirror against the f64 kernel.\n//\n// WRITTEN TO PETIR\'S CONVENTION, and dispatched by PETIR\'S RUNNER\n//\n// This file declares FUNCTIONS, not an entry point, exactly as\n// `petir::wgsl::POLY` and friends do. `petir::wgsl::gpu::GpuContext::eval_map`\n// supplies the entry point, the bind-group layout and the buffer plumbing.\n// That is deliberate reuse: PETIR\'s own docs record that leaving every\n// consumer to write \"the same 150 lines of buffer plumbing\" is how they each\n// get the layout \"subtly wrong in the same way\", and it names a real defect\n// that caused. changi gains a GPU path without gaining a wgpu dependency.\n//\n// LAYOUT CONTRACT with `eval_map`\n//\n//   src      flattened puff states, STATE_STRIDE floats each:\n//              [0] x        puff centre easting  [m]\n//              [1] y        puff centre northing [m]\n//              [2] sigma_y  crosswind dispersion [m]\n//              [3] sigma_z  vertical dispersion  [m]\n//              [4] weight   mass x time weight   [kg s]\n//   x        the cell\'s linear index, as f32 (probe value)\n//   params.m state count\n//   params.k cells per side\n//   params.a half-width of the covered square [m]\n//   params.b release height [m]\n//\n// GROUND LEVEL ONLY. The field is evaluated at z = 0, which is what\n// deposition and inhalation see and what the map paints. An elevated slice\n// would need a receptor height and is not what this is for.\n//\n// WHAT IS DELIBERATELY NOT HERE\n//\n// No Pasquill-Gifford lookup. `sigma_y`/`sigma_z` arrive per puff state,\n// computed on the CPU, because that lookup is a branchy table walk -- what a\n// shader is worst at -- and it is evaluated once per puff rather than once per\n// (puff, cell), so it is thousands of times cheaper there anyway.\n//\n// f32, AND WHY THAT IS ENOUGH HERE\n//\n// This drives a colour ramp over four decades; f32 carries about seven\n// decimal digits. The f64 CPU path stays the reference and is what any\n// QUOTED number comes from. This is for the picture.\n\n// Floats per puff state in `src`.\nconst CHANGI_STATE_STRIDE: u32 = 5u;\n\n// (2 pi)^{3/2}, the normalisation of a unit-mass 3-D Gaussian.\nconst CHANGI_TWO_PI_THREE_HALVES: f32 = 15.749609945722419;\n\n// One puff state\'s contribution at a ground-level receptor.\n//\n// Mirrors `gaussian_puff_concentration` term for term, including the\n// ground-reflection image at -H. At z = 0 the real and image terms coincide,\n// so the vertical factor is 2 exp(-H^2 / 2 sigma_z^2) -- written out rather\n// than folded to the factor of two, so the correspondence with the CPU kernel\n// stays readable.\nfn changi_puff_contribution(\n    px: f32, py: f32, sigma_y: f32, sigma_z: f32, weight: f32,\n    rx: f32, ry: f32, h: f32,\n) -> f32 {\n    // A degenerate sigma is upstream\'s NA path, mapped to zero. Guarded\n    // because a shader has no NaN reporting and one NaN would poison the\n    // whole cell.\n    if (sigma_y <= 0.0 || sigma_z <= 0.0) {\n        return 0.0;\n    }\n    let sy2 = sigma_y * sigma_y;\n    let sz2 = sigma_z * sigma_z;\n\n    let dx = rx - px;\n    let dy = ry - py;\n    let horizontal = exp(-0.5 * (dx * dx + dy * dy) / sy2);\n\n    // Receptor at z = 0: (0 - h) and (0 + h) are equal in magnitude.\n    let vertical = exp(-0.5 * h * h / sz2) + exp(-0.5 * h * h / sz2);\n\n    let amplitude = weight / (CHANGI_TWO_PI_THREE_HALVES * sy2 * sigma_z);\n    return amplitude * horizontal * vertical;\n}\n\n// The field value at one grid cell: the sum over every puff state.\n//\n// `cell` is the linear index as f32, row-major and NORTH-UP -- row 0 is the\n// northernmost, so the result can be painted straight down the screen without\n// the caller flipping it.\nfn changi_puff_field(cell: f32, cells: u32, state_count: u32, half_width: f32, h: f32) -> f32 {\n    if (cells == 0u) {\n        return 0.0;\n    }\n    let index = u32(cell);\n    let column = index % cells;\n    let row = index / cells;\n    if (row >= cells) {\n        return 0.0;\n    }\n\n    let n = f32(cells);\n    let step = 2.0 * half_width / n;\n    // Cell CENTRES, not corners: the value is the concentration in the\n    // middle of the cell rather than on its edge.\n    let easting = -half_width + (f32(column) + 0.5) * step;\n    let northing = half_width - (f32(row) + 0.5) * step;\n\n    var total: f32 = 0.0;\n    for (var i: u32 = 0u; i < state_count; i = i + 1u) {\n        let base = i * CHANGI_STATE_STRIDE;\n        total = total + changi_puff_contribution(\n            src[base], src[base + 1u], src[base + 2u], src[base + 3u], src[base + 4u],\n            easting, northing, h,\n        );\n    }\n    return total;\n}\n";
```

#### Constant `STATE_STRIDE`

Floats per puff state in the flattened `src` array. Must match
`CHANGI_STATE_STRIDE` in the shader.

```rust
pub const STATE_STRIDE: usize = 5;
```

#### Constant `TWO_PI_THREE_HALVES`

`(2 pi)^{3/2}`, the normalisation of a unit-mass three-dimensional
Gaussian. Matches the shader's constant to `f32` precision.

```rust
pub const TWO_PI_THREE_HALVES: f32 = 15.749_61;
```

## Module `climatology`

Illustrative wind conditions for examples and tests — Singapore.

# What this is, and what it is NOT

These are **illustrative climatological conditions for demonstrations**,
chosen so that the crate's examples run somewhere real rather than at an
arbitrary round number. CHANGI is named for Changi, and the repository
owner's local climate is the natural default.

**This is not a site characterisation and not a design basis.** A real
assessment needs the site's own measured wind rose at the release height,
over a defined averaging period, with a stability joint-frequency
distribution — not four representative points read off a national
climatology. Nothing here may be used for emergency planning, emergency
response, dose assessment for real populations, Level 3 PSA, or any
safety-critical or licensing decision; the crate-level scope limits are
binding and this module does not relax them.

# Why the numbers matter more than they look

Singapore's mean surface wind is about **2 m/s** — roughly half the 4 m/s
the crate's examples used before. That is not merely "a bit calmer": 2 m/s
sits **exactly on a band edge** of the Pasquill lookup in
[`super::stability::stability_class`], which switches at 2, 3, 5 and 6 m/s.

The consequences are sharp, and [`mod@tests`] pins both:

* **By day, the class is knife-edge.** Just below 2 m/s the table returns
  the ambiguous pair `A/B`; at exactly 2 m/s and just above it returns a
  single `B`. A measurement uncertainty of a few cm/s straddles that.
* **By night, the class is ambiguous across the whole range Singapore
  normally sees.** Everything below 5 m/s at night returns two classes, so
  upstream `puff`'s mass-doubling defect — see
  [`super::simulate::EmissionPolicy`] — is **live in essentially every
  Singapore night-time condition**, not in some corner case.

Light winds are also where a Gaussian puff model is weakest: the
Pasquill–Gifford fits come from tracer campaigns in steadier flow, and at
1–2 m/s the wind direction wanders enough over a puff's lifetime that
holding it fixed from the moment of emission — which is exactly what
[`super::simulate`] does, following upstream — is a real approximation and
not a small one.

# Provenance

Figures attributed to the **Meteorological Service Singapore (MSS)**,
*Climate of Singapore*, <https://www.weather.gov.sg/climate-climate-of-singapore/>.

**Accessed 2026-09-23 via web-search summaries of that page, NOT by
retrieving it directly** — `weather.gov.sg` and `nea.gov.sg` are both
blocked by this development environment's network egress policy. The
figures below are therefore recorded as **`Not re-checked against the
primary source`**, per the workspace rule that an unverifiable claim is
marked rather than left standing. They are round representative values in
any case, not precise climatological statistics, and they are used only to
make a demonstration concrete.

Full record, with the corroborating sources: `docs/References.md`.

```rust
pub mod climatology { /* ... */ }
```

### Types

#### Enum `SingaporeWind`

A representative Singapore wind condition.

Dispatched as an enum rather than a trait object, per the workspace Rust
design rules: the set of monsoon regimes is closed and known at compile
time, so adding one forces every `match` to handle it.

```rust
pub enum SingaporeWind {
    NortheastMonsoon,
    NortheastMonsoonSurge,
    SouthwestMonsoon,
    InterMonsoon,
}
```

##### Variants

###### `NortheastMonsoon`

**Northeast Monsoon**, December to early March. Winds from the
northerly-to-northeasterly sector; the windiest season, strongest in
January and February.

###### `NortheastMonsoonSurge`

**Northeast Monsoon surge** — an episode within the NE monsoon when
mean speeds reach 10 m/s or more. Included because it is the only
common Singapore condition that clears the `U >= 6 m/s` band, where the
Pasquill class is an unambiguous `D` day or night.

###### `SouthwestMonsoon`

**Southwest Monsoon**, June to September. Winds from the
southeasterly-to-southerly sector.

###### `InterMonsoon`

**Inter-monsoon**, April–May and October–November. Light and variable;
the direction here is nominal, and a real study would not treat an
inter-monsoon direction as persistent.

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human label, e.g. `"NE monsoon"`.

- ```rust
  pub fn season(self: Self) -> &'static str { /* ... */ }
  ```
  The months this condition covers, as prose.

- ```rust
  pub fn speed(self: Self) -> Velocity { /* ... */ }
  ```
  Representative scalar wind speed.

- ```rust
  pub fn direction(self: Self) -> Angle { /* ... */ }
  ```
  Prevailing wind direction in the **meteorological convention** — the

- ```rust
  pub fn components(self: Self) -> WindComponents { /* ... */ }
  ```
  The wind as `(u, v)` components in the site's local Cartesian frame.

- ```rust
  pub fn stability(self: Self, hour: u32) -> StabilitySet { /* ... */ }
  ```
  The Pasquill class(es) this condition selects at a given hour.

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
    fn clone(self: &Self) -> SingaporeWind { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SingaporeWind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `MEAN_WIND_SPEED_M_PER_S`

Singapore's mean surface wind speed, m/s.

About 2 m/s (roughly 4 knots) — light, and low enough that it sits on the
`U = 2 m/s` Pasquill band edge. See the module documentation for why that
matters.

Source: MSS, *Climate of Singapore*. Not re-checked against the primary
source — see the module's Provenance section.

```rust
pub const MEAN_WIND_SPEED_M_PER_S: f64 = 2.0;
```

## Module `concentration`

The Gaussian puff concentration kernel.

```rust
pub mod concentration { /* ... */ }
```

### Functions

#### Function `gaussian_puff_concentration`

**Attributes:**

- `MustUse { reason: None }`

Concentration at a receptor from one Gaussian puff, as a **mass density**.

Ports the body of `gpuff`, less its methane unit conversion. The puff is a
three-dimensional Gaussian of total mass `Q` centred on `(x_p, y_p, H)`,
with a mirror image at `(x_p, y_p, -H)` that enforces zero flux through the
ground:

```text
  C = Q / ((2 pi)^{3/2} sigma_y^2 sigma_z)
      * exp(-((x_r - x_p)^2 + (y_r - y_p)^2) / (2 sigma_y^2))
      * [ exp(-(z_r - H)^2 / (2 sigma_z^2)) + exp(-(z_r + H)^2 / (2 sigma_z^2)) ]
```

The horizontal spread is **isotropic** — `sigma_y` is used for both `x` and
`y`, so the puff is circular in plan rather than elongated along the wind.
That is a real modelling choice of upstream's, not an oversight: a puff
model represents along-wind spread by the *spacing* of successive puffs, so
giving each puff an along-wind `sigma_x` as well would double-count it.

# Arguments
- `mass` — the puff's total mass `Q`.
- `class` — stability class. Upstream accepts a vector here and silently
  uses only its first element; this takes the single class the caller means,
  which is what [`super::stability::StabilitySet::primary`] supplies.
- `puff_x`, `puff_y` — the puff centre's horizontal position.
- `source_height` — release height `H` above ground, which is also the
  height of the reflected image below it.
- `receptor` — where the concentration is wanted, `(x, y, z)`.
- `travel_distance` — how far the puff has travelled from its source. This
  drives the dispersion coefficients and is **not** the puff-to-receptor
  distance.

# Returns
Mass concentration at the receptor. Zero when the puff has not yet moved:
[`pasquill_gifford_sigmas`] is undefined at zero distance (upstream's `NA`),
and upstream's final `ifelse(is.na(C), 0, C)` turns that into a zero. That
zero is a modelling artefact, not physics — a freshly emitted puff has a
very high concentration at its own centre, and this model reports none.

# Note on `U`
Upstream's `gpuff` declares a wind-speed parameter `U` and **never
references it in the body**. It is not taken here. See
`docs/puff-code-to-code.md`, upstream defect 1.

```rust
pub fn gaussian_puff_concentration(mass: uom::si::f64::Mass, class: super::stability::StabilityClass, puff_x: uom::si::f64::Length, puff_y: uom::si::f64::Length, source_height: uom::si::f64::Length, receptor: (uom::si::f64::Length, uom::si::f64::Length, uom::si::f64::Length), travel_distance: uom::si::f64::Length) -> uom::si::f64::MassDensity { /* ... */ }
```

#### Function `gaussian_puff_methane_ppm`

**Attributes:**

- `MustUse { reason: None }`

[`gaussian_puff_concentration`] expressed as parts-per-million of methane.

This is upstream's `gpuff` exactly, including its unit conversion. It exists
for the code-to-code comparison and for callers actually modelling methane.
**For any other species the factor is wrong** — see
[`METHANE_PPM_PER_KG_PER_M3`].

# Why this returns a bare `f64` and not a `uom::Ratio`

ppm is dimensionless, so `uom`'s `Ratio` is the obvious type — and it
silently destroys small values here. `Ratio` stores its magnitude in the
**base** unit, so constructing one from a ppm figure divides by `1e6` and
reading it back multiplies by `1e6`. Puff concentrations reach far enough
below `1e-300` that the divided value lands in the **subnormal** range,
where the mantissa is truncated and the multiply back cannot recover it.

Measured on the fixture, at `class A, Q = 1e-6 kg, travel = 1 m,
receptor (-10, -10, 2)`:

| | value |
|---|---|
| upstream R | `7.4821302396969955e-307` |
| through `Ratio` | `7.48213023968e-307` |

— five significant digits gone, a `2.3e-12` relative error, with no warning.
The physical concentration there is negligible, but the mechanism is not
specific to negligible values: it applies to anything whose ppm figure is
below about `2.2e-302`, and it would corrupt a sum just as quietly. The
dimensioned return is [`gaussian_puff_concentration`], which stores kg/m^3
directly and does not round-trip.

```rust
pub fn gaussian_puff_methane_ppm(mass: uom::si::f64::Mass, class: super::stability::StabilityClass, puff_x: uom::si::f64::Length, puff_y: uom::si::f64::Length, source_height: uom::si::f64::Length, receptor: (uom::si::f64::Length, uom::si::f64::Length, uom::si::f64::Length), travel_distance: uom::si::f64::Length) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `METHANE_PPM_PER_KG_PER_M3`

Upstream's `conversion.factor`: `(1e6) * 1.524`, turning kg/m^3 into
parts-per-million **of methane**.

The `1e6` is the ppm scaling; the `1.524` is the molar-volume ratio that
converts a methane mass concentration to a volume mixing ratio at upstream's
implied reference temperature and pressure (`M_air / M_CH4 / rho_air`
≈ `28.96 / 16.04 / 1.185` ≈ `1.524 m^3/kg`).

# This factor is species-specific and must not be reused

It encodes methane's molar mass. Applying it to a radionuclide — CHANGI's
actual subject — would be wrong twice over: the molar mass is different, and
ppm is the wrong unit for an activity concentration, which belongs in
Bq/m^3. Use [`gaussian_puff_concentration`], which returns a mass density,
and convert with the species' own factor. [`gaussian_puff_methane_ppm`]
exists so the code-to-code comparison against upstream has something to
compare, not because ppm is the right output for this crate.

```rust
pub const METHANE_PPM_PER_KG_PER_M3: f64 = _;
```

## Module `dispersion`

Pasquill–Gifford dispersion coefficients `sigma_y` and `sigma_z`.

These are the standard deviations of the Gaussian concentration profile
crosswind (`sigma_y`) and vertically (`sigma_z`), as functions of how far
the puff has travelled and of the [stability
class](super::stability::StabilityClass). They are empirical fits to the
Prairie Grass and related tracer campaigns, in the algebraic form given by
Martin (1976) as used by the US EPA's ISC models:

```text
  sigma_z = a * x^b                       (x in km, sigma_z in m)
  sigma_y = 465.11628 * x * tan(theta),  theta = (pi/180) * (c - d * ln x)
```

with `(a, b)` selected from a per-class table of distance bins and `(c, d)`
constant per class. The `465.11628` is `1000 / (2 * 2.15)` — the metres-per-
kilometre conversion folded into the half-width-to-sigma factor 2.15 that
the original nomograms were drawn with.

**These fits are only defined over roughly 0.1–10 km.** Upstream applies
them at any positive distance; see [`pasquill_gifford_sigmas`] for what that
produces beyond the fitted range, which is measured rather than assumed.

```rust
pub mod dispersion { /* ... */ }
```

### Types

#### Struct `DispersionSigmas`

The crosswind and vertical spread of a puff.

Both are standard deviations of a Gaussian, in metres.

```rust
pub struct DispersionSigmas {
    pub sigma_y: uom::si::f64::Length,
    pub sigma_z: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sigma_y` | `uom::si::f64::Length` | Crosswind (horizontal, perpendicular to travel) standard deviation. |
| `sigma_z` | `uom::si::f64::Length` | Vertical standard deviation. |

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
    fn clone(self: &Self) -> DispersionSigmas { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DispersionSigmas) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `pasquill_gifford_sigmas`

**Attributes:**

- `MustUse { reason: None }`

Pasquill–Gifford `sigma_y` and `sigma_z` at a travel distance.

Ports `compute_sigma_vals` for a single class and distance. Upstream
vectorises over both and returns a `2 × n` matrix; a scalar function plus an
iterator is the Rust equivalent and avoids the column-major indexing that
makes upstream's own `gpuff` discard its second stability class.

# Arguments
- `class` — the stability class selecting the coefficient set.
- `distance` — distance travelled from the source. Converted to **kilometres**
  internally, because the fits are defined in km; the `uom` type means the
  caller cannot get that conversion wrong, which upstream's bare `f64` and
  its `total_dist <- total_dist / 1000` line inside `gpuff` can.

# Returns
`None` when `distance <= 0`, which is upstream's `NA` case: a puff that has
not moved has no defined spread, and upstream's `gpuff` converts the
resulting `NA` concentration to `0`. Returning `Option` rather than a NaN
makes that a case the caller must handle.

# Valid range, and what happens outside it

The fits are empirical over roughly **0.1–10 km**. Upstream applies them at
any positive distance and this port reproduces that, so the caller can hit
two regimes where the answer is formally defined and physically meaningless:

* **`sigma_z` saturates** at [`SIGMA_Z_CAP_METERS`], by upstream's explicit
  `min(..., 5000)`.
* **`sigma_y` does not saturate, and eventually goes negative.** It is
  `465.11628 * x * tan(theta)` with `theta = (pi/180)(c - d ln x)`, so
  `theta` falls through zero as `x` grows and `tan(theta)` follows it.
  Measured for class `D` (`c = 8.333`, `d = 0.72382`): `theta` reaches zero
  at `x = exp(8.333 / 0.72382) = 9.6e4 km`, well past any sane use, but for
  class `F` (`c = 4.1667`, `d = 0.36191`) it is `x = 9.9e4 km`. Both are
  beyond Earth's circumference, so the sign change is unreachable in
  practice — but a *negative* `sigma_y` would not be caught by the model,
  because [`super::concentration`] only ever uses `sigma_y^2`.

Neither regime is clamped here. Adding a range check upstream does not have
would make this function disagree with upstream inside the fitted range's
own edge cases, and the code-to-code comparison is the point. Callers who
need a bound should apply it at the call site.

```rust
pub fn pasquill_gifford_sigmas(class: super::stability::StabilityClass, distance: uom::si::f64::Length) -> Option<DispersionSigmas> { /* ... */ }
```

### Constants and Statics

#### Constant `SIGMA_Z_CAP_METERS`

Upstream's hard cap on `sigma_z`, in metres.

`sigma_z <- min(a * x^b, 5000)`. Physically this stands in for the mixing
height: a plume cannot spread vertically past the capping inversion, and
5 km is a generous upper bound on a daytime mixed layer. Note there is
**no matching cap on `sigma_y`**, which is why the horizontal spread runs
away at long range — see [`pasquill_gifford_sigmas`].

```rust
pub const SIGMA_Z_CAP_METERS: f64 = 5000.0;
```

## Module `simulate`

The two run modes: concentration at named sensors, and on a regular grid.

Both march a fixed time step, emit a puff every `puff_dt`, advect every live
puff, and sum the contributions. They differ in *where* concentrations are
evaluated and — more consequentially — in *how they are reduced in time*:
sensor mode averages over each output interval, grid mode samples
instantaneously at the end of it. See [`simulate_grid_mode`] for why that
asymmetry matters.

**How a puff is advected is a policy** — see [`AdvectionPolicy`]. The
default integrates each puff's own trajectory step by step with the wind
that actually blows, so a puff *turns* when the wind turns and remembers
where it had got to. Upstream's analytic `source + u_emit * age` is retained
as the bug-compatible variant the code-to-code fixture asks for by name.

```rust
pub mod simulate { /* ... */ }
```

### Types

#### Enum `AdvectionPolicy`

How a live puff's position and dispersion distance are obtained.

# Why this exists — a puff that cannot turn is not Lagrangian

Upstream places a puff analytically: it stores the wind sampled at the
puff's **moment of emission** and, forever after, puts the puff at
`source + (u, v) * age`. A puff therefore flies a perfectly straight line on
the wind of its birth, and **never responds to the wind again**. For the
steady wind upstream's own examples run at that is exact and free. For a
wind that veers — which is the case a puff model is reached for in the first
place — it is wrong: a plume that should bend into a dog-leg stays a
straight ray, and the model has no memory of where each puff had actually
got to.

A *Lagrangian* treatment is the fix, and it is what the word already
promises: each puff is a parcel carrying its own state, and its position is
the **integral of the wind it has actually experienced**,

```text
(x, y)_{n+1} = (x, y)_n + (u, v)(t_n) * dt
```

so the wind changing rotates only what happens *next*. History is state, not
something recomputed from the present.

# Dispersion distance: PATH LENGTH, not net displacement

The second half of the fix, and the easier one to miss. Pasquill–Gifford
`sigma_y`, `sigma_z` grow with the distance a puff has travelled *through
the turbulent field*. On a straight trajectory that is the same number as
the straight-line distance from the source, which is why upstream can write
`hypot(dx, dy)` and be right. On a curved trajectory the two part company,
and net displacement is the wrong one — a puff blown 500 m east and then
500 m back west has dispersed for 1 km of travel while sitting 0 m from the
stack, and `hypot` would call it undispersed.

[`Self::LagrangianTrajectory`] therefore accumulates **path length**
alongside position. [`Self::UpstreamFrozenWind`] keeps `hypot`, because on
its straight ray the two agree identically and the fixture compares digits.

# The two agree exactly for a constant wind — by construction

Marching `dx += u * dt` for `n` steps gives `u * n * dt = u * age`, so on a
constant wind the Lagrangian population sits exactly where the analytic one
does, and the accumulated path equals `|U| * age`. The difference is
therefore confined to precisely the case upstream gets wrong.

It is **not bit-identical**: `n` accumulated additions do not round the same
way as one multiplication. That is the whole reason the bug-compatible
variant has to exist rather than being inferred — `tests/puff_code_to_code.rs`
compares against upstream R near machine epsilon, and a summation-order
difference of a few ulps would read as a translation error.
`tests::the_two_advection_policies_agree_on_a_constant_wind` measures the
actual agreement.

# Which is the default, and why

[`Self::LagrangianTrajectory`], because the workspace rule is that physics
the model is meant to represent is applied unless a caller explicitly
ablates it, and an ablation must be a visible act rather than the default
state. Same reasoning, and the same shape, as [`EmissionPolicy`] in this
file.

```rust
pub enum AdvectionPolicy {
    LagrangianTrajectory,
    UpstreamFrozenWind,
}
```

##### Variants

###### `LagrangianTrajectory`

**Default.** Integrate each puff's trajectory with the wind that
actually blows at each step, accumulating position and path length. A
puff turns when the wind turns, and keeps the position it had reached.

###### `UpstreamFrozenWind`

**Bug-compatible.** Upstream's analytic placement: the wind is frozen at
emission and the puff is put at `source + u_emit * age`, with dispersion
distance `hypot` of that displacement. A puff never turns. Required to
reproduce upstream's numbers digit for digit, and used by the
code-to-code fixture for exactly that reason.

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
    fn clone(self: &Self) -> AdvectionPolicy { /* ... */ }
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
    fn default() -> AdvectionPolicy { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AdvectionPolicy) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `EmissionPolicy`

How many puffs an emission event produces when the stability class is
ambiguous.

Six of upstream's ten (wind speed × day/night) regimes return **two**
stability classes — see [`super::stability::StabilitySet`]. Upstream then
builds its puff record with

```r
new_puff <- data.frame(
  time_emitted = current_elapsed,   # length 1
  ...,
  stab_class   = as.character(stab_class),  # length 1 OR 2
  mass         = q_per_puff         # length 1
)
```

and R recycles the length-1 columns against the length-2 `stab_class`, so
the data frame gains **two rows — each carrying the full `q_per_puff`**.
The emitted mass is therefore doubled across most of the wind-speed range,
which `q_per_puff = (emission_rate / 3600) * puff_dt` plainly does not
intend.

The workspace rule is that correct physics is the default and a divergence
must be an explicit, visible act, so [`Self::OnePuffPerEmission`] is
`Default` and the bug-compatible variant has to be asked for by name.

```rust
pub enum EmissionPolicy {
    OnePuffPerEmission,
    UpstreamRecycleStabilityClasses,
}
```

##### Variants

###### `OnePuffPerEmission`

**Default, and mass-conserving.** One puff per emission event, carrying
the whole of `q_per_puff`, dispersing with the primary (more unstable)
class. This matches what upstream's own `gpuff` does with an ambiguous
class — it uses the first and discards the second — so it is also the
reading most consistent with the rest of upstream.

###### `UpstreamRecycleStabilityClasses`

**Bug-compatible.** Reproduces upstream's recycling: one puff per
stability class, each with the full `q_per_puff`, so an ambiguous
condition emits twice the mass. Required to reproduce upstream's
numbers, and used by the code-to-code fixture for exactly that reason.

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
    fn clone(self: &Self) -> EmissionPolicy { /* ... */ }
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
    fn default() -> EmissionPolicy { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &EmissionPolicy) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Source`

A source of emissions: a position and a release height.

```rust
pub struct Source {
    pub x: uom::si::f64::Length,
    pub y: uom::si::f64::Length,
    pub height: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `uom::si::f64::Length` | Eastward position, metres in the site frame. |
| `y` | `uom::si::f64::Length` | Northward position, metres in the site frame. |
| `height` | `uom::si::f64::Length` | Release height above ground. |

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
    fn clone(self: &Self) -> Source { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Source) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Receptor`

A point where concentration is evaluated.

```rust
pub struct Receptor {
    pub x: uom::si::f64::Length,
    pub y: uom::si::f64::Length,
    pub z: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `uom::si::f64::Length` | Eastward position, metres in the site frame. |
| `y` | `uom::si::f64::Length` | Northward position, metres in the site frame. |
| `z` | `uom::si::f64::Length` | Height above ground. |

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
    fn clone(self: &Self) -> Receptor { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Receptor) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `RunConfig`

Everything that does not change between the two run modes.

```rust
pub struct RunConfig {
    pub sim_dt: uom::si::f64::Time,
    pub puff_dt: uom::si::f64::Time,
    pub output_dt: uom::si::f64::Time,
    pub duration: uom::si::f64::Time,
    pub puff_duration: uom::si::f64::Time,
    pub start_hour: u32,
    pub emission_policy: EmissionPolicy,
    pub advection: AdvectionPolicy,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sim_dt` | `uom::si::f64::Time` | Simulation time step. Every puff is advected and every receptor<br>evaluated at this cadence. |
| `puff_dt` | `uom::si::f64::Time` | Interval between puff emissions. Upstream requires this to be a positive<br>integer multiple of `sim_dt` so no temporal interpolation is needed;<br>that requirement is asserted here rather than left to the caller. |
| `output_dt` | `uom::si::f64::Time` | Output reporting interval. |
| `duration` | `uom::si::f64::Time` | Total simulated duration, `end_time - start_time`. |
| `puff_duration` | `uom::si::f64::Time` | How long a puff is tracked before being dropped. Upstream's default is<br>1200 s. |
| `start_hour` | `u32` | Hour of day at the start of the run, 0–23 local, used for the day/night<br>half of the stability lookup.<br><br>Upstream re-derives this from each timestamp, so a run crossing 07:00 or<br>19:00 changes regime mid-run. This port takes the *start* hour and holds<br>it, which is identical for any run inside a single day/night block and<br>differs otherwise — see `docs/puff-code-to-code.md`. The fixture only<br>covers runs inside one block, so the difference is stated rather than<br>verified. |
| `emission_policy` | `EmissionPolicy` | Which emission policy to apply. See [`EmissionPolicy`]. |
| `advection` | `AdvectionPolicy` | How puffs are advected. See [`AdvectionPolicy`]; the default is the<br>Lagrangian trajectory, and upstream's frozen wind must be asked for. |

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
    fn clone(self: &Self) -> RunConfig { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RunConfig) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SensorSeries`

Concentration at each sensor, averaged over each output interval.

Row `i` is output interval `i`, column `j` is sensor `j`, in the order the
sensors were supplied.

```rust
pub struct SensorSeries {
    pub concentrations: Vec<Vec<f64>>,
    pub interval_starts: Vec<uom::si::f64::Time>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `concentrations` | `Vec<Vec<f64>>` | `[interval][sensor]`, **parts per million** of methane. A bare `f64`<br>rather than a `uom::Ratio` — see<br>[`super::concentration::gaussian_puff_methane_ppm`] for the measured<br>reason (`Ratio`'s base-unit round trip pushes small concentrations<br>subnormal and truncates them). |
| `interval_starts` | `Vec<uom::si::f64::Time>` | The start time of each interval, as an offset from the run start. |

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
    fn clone(self: &Self) -> SensorSeries { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SensorSeries) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GridSeries`

Concentration on the grid, sampled at the end of each output interval.

```rust
pub struct GridSeries {
    pub concentrations: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `concentrations` | `Vec<Vec<f64>>` | `[output step][grid point]`, **parts per million** of methane, as a bare<br>`f64` for the same reason as [`SensorSeries::concentrations`]. Grid points<br>are in the order produced by varying `x` fastest, then `y`, then `z` —<br>R's `expand.grid` order, preserved so a comparison is index-for-index. |

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
    fn clone(self: &Self) -> GridSeries { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GridSeries) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `simulate_sensor_mode`

**Attributes:**

- `MustUse { reason: None }`

Simulate concentration at a set of sensors.

Ports `simulate_sensor_mode`. Each source emits independently and the
contributions are summed, so multiple sources are linear — which they are,
the model being a sum of Gaussians.

# Arguments
- `sources` — one or more emission points. All share `emission_rate`,
  exactly as upstream ("Applied uniformly to all sources").
- `emission_rate` — mass per unit time from **each** source.
- `wind` — one [`WindComponents`] per simulation step. Must be at least as
  long as the number of steps; upstream indexes `wind_u[t_idx]` with no
  bounds check and silently produces `NA` concentrations past the end.
- `sensors` — receptor positions.
- `config` — timing and policy, see [`RunConfig`].

# Returns
A [`SensorSeries`]: the **mean** concentration over each output interval,
where interval `i` covers simulation times `[i*output_dt, (i+1)*output_dt)`.
The half-open interval and the arithmetic mean both follow upstream's
`aggregate(..., by = cut(sim_timestamps, breaks = output_timestamps),
FUN = mean)`; R's `cut.POSIXt` defaults to `right = FALSE`, which is what
makes the intervals left-closed. The final simulation timestamp falls
outside the last interval and is dropped, so there is **one fewer output row
than there are output timestamps** — a rounding-off of the last step that is
upstream's behaviour and is reproduced.

# Panics
Panics if `config` is inconsistent (see [`RunConfig`]), if `sensors` or
`sources` is empty, or if `wind` is shorter than the number of simulation
steps. Upstream produces `NA` in the last case rather than stopping.

```rust
pub fn simulate_sensor_mode(sources: &[Source], emission_rate: uom::si::f64::MassRate, wind: &[super::wind::WindComponents], sensors: &[Receptor], config: &RunConfig) -> SensorSeries { /* ... */ }
```

#### Function `simulate_grid_mode`

**Attributes:**

- `MustUse { reason: None }`

Simulate concentration on a regular grid.

Ports `simulate_grid_mode`. The grid is the Cartesian product of the three
coordinate vectors, flattened with `x` varying fastest then `y` then `z`,
which is R's `expand.grid` order.

# Arguments
- `sources`, `emission_rate`, `wind`, `config` — as for
  [`simulate_sensor_mode`].
- `grid_x`, `grid_y`, `grid_z` — the grid axes.

# Returns
A [`GridSeries`] with one row per output timestamp.

# Two divergences from sensor mode, both upstream's

1. **Instantaneous, not averaged.** Grid mode writes the concentration at
   the single step where `step_index % (output_dt / sim_dt) == 0`, whereas
   sensor mode averages the whole interval. The two modes therefore do not
   report the same quantity, and a grid value will be noisier than the
   sensor value at the same place and time.
2. **The last output row is usually zero.** Upstream allocates
   `length(seq(start, end, by = output_dt))` rows but only ever fills index
   `t_idx / (output_dt / sim_dt)` for `t_idx` in `1..n_steps`, which reaches
   at most `floor(n_steps / (output_dt / sim_dt))`. With upstream's own
   documented example (`n_steps = 361`, `output_dt/sim_dt = 12`) that is 30
   of 31 rows, leaving the last all zeros.

Both are reproduced rather than corrected, because this function's contract
is to be comparable with upstream. They are recorded in
`docs/puff-code-to-code.md` as upstream defects 3 and 4.

# Panics
As [`simulate_sensor_mode`], plus if any grid axis is empty.

```rust
pub fn simulate_grid_mode(sources: &[Source], emission_rate: uom::si::f64::MassRate, wind: &[super::wind::WindComponents], grid_x: &[uom::si::f64::Length], grid_y: &[uom::si::f64::Length], grid_z: &[uom::si::f64::Length], config: &RunConfig) -> GridSeries { /* ... */ }
```

#### Function `constant_wind`

**Attributes:**

- `MustUse { reason: None }`

Build a constant wind series of `n` samples, a convenience for examples and
tests that do not care about wind variability.

```rust
pub fn constant_wind(u: uom::si::f64::Velocity, v: uom::si::f64::Velocity, n: usize) -> Vec<super::wind::WindComponents> { /* ... */ }
```

## Module `stability`

Pasquill stability classification from wind speed and time of day.

The Pasquill–Gifford scheme sorts the atmosphere into six turbulence
regimes, `A` (strongly unstable, vigorous daytime convection) through `F`
(strongly stable, calm clear night). The class selects the dispersion
coefficients in [`super::dispersion`], so it is the single largest control
on how fast a plume spreads.

Upstream's rule is a lookup on wind speed crossed with day/night, and it
returns **one or two** classes: the original Pasquill table is a range, not
a point, and upstream preserves that ambiguity rather than picking a
midpoint. [`StabilitySet`] carries the one-or-two distinction in the type,
so a caller cannot forget the second class exists.

```rust
pub mod stability { /* ... */ }
```

### Types

#### Enum `StabilityClass`

A Pasquill–Gifford atmospheric stability class.

Ordered from most unstable to most stable. `A` is a hot, calm, sunny
afternoon — strong convection, rapid vertical mixing, a plume that fattens
quickly and dilutes fast. `F` is a clear, calm night — a stable layer that
suppresses vertical motion, so a plume stays narrow and travels far at high
concentration. `D` is neutral, typical of overcast or windy conditions, and
is the default when nothing better is known.

Dispatched with a `match` (no trait objects, per the workspace Rust design
rules), so adding a class would force every consumer to handle it.

```rust
pub enum StabilityClass {
    A,
    B,
    C,
    D,
    E,
    F,
}
```

##### Variants

###### `A`

Extremely unstable — strong daytime convection, light wind.

###### `B`

Moderately unstable.

###### `C`

Slightly unstable.

###### `D`

Neutral — overcast or windy, day or night. The fallback class.

###### `E`

Slightly stable — night, light-to-moderate wind.

###### `F`

Moderately stable — clear, calm night.

##### Implementations

###### Methods

- ```rust
  pub fn letter(self: Self) -> &'static str { /* ... */ }
  ```
  The single-letter label upstream uses (`"A"` … `"F"`).

- ```rust
  pub fn from_letter(letter: &str) -> Option<Self> { /* ... */ }
  ```
  Parse a single-letter label, accepting either case.

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
    fn clone(self: &Self) -> StabilityClass { /* ... */ }
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

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &StabilityClass) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &StabilityClass) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &StabilityClass) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `StabilitySet`

The one or two stability classes the Pasquill table admits for a condition.

Upstream returns an R character vector of length 1 or 2. **Six** of its ten
(wind speed × day/night) regimes are ambiguous and return two classes; the
four that are not are `2 ≤ U < 3` by day (`B`), `5 ≤ U < 6` by night (`D`),
and `U ≥ 6` by day or night (`D`).

Representing that as an enum rather than a `Vec` matters: it is total (there
is no empty or three-element case to handle), it allocates nothing, and it
makes the ambiguity impossible to drop silently — which is exactly what
upstream's own `gpuff` does. See [`Self::primary`].

```rust
pub enum StabilitySet {
    One(StabilityClass),
    Two(StabilityClass, StabilityClass),
}
```

##### Variants

###### `One`

The condition selects exactly one class.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `StabilityClass` |  |

###### `Two`

The condition is ambiguous between two adjacent classes, in upstream's
order (the more unstable of the pair first).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `StabilityClass` |  |
| 1 | `StabilityClass` |  |

##### Implementations

###### Methods

- ```rust
  pub fn primary(self: Self) -> StabilityClass { /* ... */ }
  ```
  The first class, which is the one upstream's `gpuff` actually uses.

- ```rust
  pub fn secondary(self: Self) -> Option<StabilityClass> { /* ... */ }
  ```
  The second class where the condition is ambiguous.

- ```rust
  pub fn len(self: Self) -> usize { /* ... */ }
  ```
  How many classes this set carries — 1 or 2.

- ```rust
  pub fn is_empty(self: Self) -> bool { /* ... */ }
  ```
  Always `false` — a set never has zero classes. Present because clippy

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
    fn clone(self: &Self) -> StabilitySet { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &StabilitySet) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `is_day`

**Attributes:**

- `MustUse { reason: None }`

Whether an hour of the day counts as daytime for the stability lookup.

Ports `is_day`. Upstream takes a timestamp and formats it with `"%H"`, then
tests `hour >= 7 & hour <= 18`; this takes the hour directly, since that is
the only thing the physics reads. The bounds are **inclusive at both ends**,
so 07:00 and 18:00 are both day and the daytime window is 12 hours long.

# Arguments
- `hour` — hour of day, 0–23 in local time.

# Note
This is a fixed clock window, not a solar calculation: it does not depend on
latitude, date or season. Upstream is a near-field oil-and-gas leak model
where that approximation is cheap and conventional. For a high-latitude or
seasonal application it is wrong, and a solar-zenith criterion should be
used instead — [`crate::flexpart`]'s upstream carries `zenithangle.f90` for
exactly this reason, though it is not yet ported.

```rust
pub fn is_day(hour: u32) -> bool { /* ... */ }
```

#### Function `stability_class`

**Attributes:**

- `MustUse { reason: None }`

Pasquill stability class(es) for a wind speed and hour of day.

Ports `get_stab_class`. The lookup, with upstream's own band edges:

| wind speed `U` (m/s) | day | night |
|---|---|---|
| `U < 2` | A, B | E, F |
| `2 ≤ U < 3` | B | E, F |
| `3 ≤ U < 5` | B, C | D, E |
| `5 ≤ U < 6` | C, D | D |
| `U ≥ 6` | D | D |

# Arguments
- `wind_speed` — scalar wind speed. `None` reproduces upstream's
  missing-value path and yields neutral [`StabilityClass::D`]; upstream also
  emits an R `warning()` there, which this port does not (a library that
  prints is a library that cannot be used quietly — the `None` in the
  argument is the signal).
- `hour` — hour of day, 0–23 local, passed to [`is_day`].

# Returns
One or two classes, per the table. See [`StabilitySet`] for why the
two-class case is not collapsed.

# Panics
Panics if `wind_speed` is negative, which is not a wind speed. Upstream
accepts it silently and lands in the `U ≥ 6` branch only for large
magnitudes — a negative speed falls in `U < 2` and is classified as calm,
which is a plausible-looking wrong answer rather than an error.

```rust
pub fn stability_class(wind_speed: Option<uom::si::f64::Velocity>, hour: u32) -> StabilitySet { /* ... */ }
```

## Module `wind`

Wind handling: meteorological convention to vector components, and
resampling a wind series onto the simulation time step.

```rust
pub mod wind { /* ... */ }
```

### Types

#### Struct `WindComponents`

Wind as vector components in the site's local Cartesian frame.

`u` is the eastward component and `v` the northward one — i.e. the
direction the air is *going*, not the direction it comes *from*. That
inversion is the whole content of [`wind_vector_convert`], and it is the
single most common sign error in dispersion code.

```rust
pub struct WindComponents {
    pub u: uom::si::f64::Velocity,
    pub v: uom::si::f64::Velocity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `u` | `uom::si::f64::Velocity` | Eastward (`+x`) component. |
| `v` | `uom::si::f64::Velocity` | Northward (`+y`) component. |

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
    fn clone(self: &Self) -> WindComponents { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &WindComponents) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `wind_vector_convert`

**Attributes:**

- `MustUse { reason: None }`

Convert meteorological wind speed and direction to `(u, v)` components.

Ports `wind_vector_convert`. The meteorological convention names the
direction the wind blows **from**, measured clockwise from north: `0°` is a
northerly (air moving south), `90°` easterly, `180°` southerly, `270°`
westerly. The mathematical convention wanted by the advection step measures
anticlockwise from east and names where the air is **going**. Upstream's
one-line reconciliation is

```text
  theta = (270 - direction) * pi / 180
  u = speed * cos(theta)
  v = speed * sin(theta)
```

# Arguments
- `speed` — scalar wind speed.
- `direction` — meteorological wind direction (blowing *from*).

# Returns
Components in the local Cartesian frame, in the direction the air travels.

# Note on exactness
The cardinal directions do not come out exactly zero: `270 - 0 = 270°` in
radians is not exactly `3*pi/2` in binary, so a due-northerly gives
`u = -9.18e-16 * speed` rather than `0`. Upstream has the same residue, to
the bit, because the arithmetic is identical. It is left alone rather than
snapped to zero — rounding it would be a silent divergence from upstream for
no physical gain, and `1e-16 m/s` advects a puff by `1e-13 m` over a day.

```rust
pub fn wind_vector_convert(speed: uom::si::f64::Velocity, direction: uom::si::f64::Angle) -> WindComponents { /* ... */ }
```

#### Function `interpolate_wind`

**Attributes:**

- `MustUse { reason: None }`

Resample a wind series onto a regular simulation time step.

Ports `interpolate_wind_data`. Upstream converts speed/direction to `(u, v)`
**first** and interpolates the components, not the polar coordinates. That
ordering is deliberate and worth preserving: interpolating a direction
across the 360°/0° wrap would sweep the long way round, and interpolating a
speed through a calm would miss the reversal. Interpolating `u` and `v`
does both correctly.

The observation times are assumed **evenly spaced across the whole
simulation window** — upstream builds them with
`seq(sim_start, sim_end, length.out = length(wind_speeds))`, so the input
series is stretched to fit the window regardless of what its real sampling
interval was. A caller whose observations do not span exactly
`[sim_start, sim_end]` will get a silently time-shifted series. This port
keeps that behaviour and names it here.

# Arguments
- `components` — the observed wind, already in `(u, v)` form. Callers with
  speed/direction data should map [`wind_vector_convert`] over it first,
  which is what upstream does internally.
- `duration` — the simulation window length, `sim_end - sim_start`.
- `step` — the output interval (upstream's `puff_dt`).

# Returns
One [`WindComponents`] per output step, at times
`0, step, 2*step, …` up to and including `duration` if it falls on a step.
Matches R's `seq(from, to, by = step)`, which stops at or before `to`.

# Panics
Panics if `components` is empty, if `step` is not strictly positive, or if
`duration` is negative. R's `approx` errors on fewer than two points and
`seq` errors on a non-positive `by`; these are the same conditions, checked
up front.

```rust
pub fn interpolate_wind(components: &[WindComponents], duration: uom::si::f64::Time, step: uom::si::f64::Time) -> Vec<WindComponents> { /* ... */ }
```

#### Function `wind_speed`

**Attributes:**

- `MustUse { reason: None }`

Scalar wind speed from components, `sqrt(u^2 + v^2)`.

Upstream computes this inline in both simulate drivers. It is named here
because the stability lookup and the puff advection both need it and must
agree on it.

```rust
pub fn wind_speed(c: WindComponents) -> uom::si::f64::Velocity { /* ... */ }
```

## Module `prelude`

Everything a caller normally needs, re-exported in one place.

The workspace "human interface layer" rule asks that a Rust developer be able
to drive a crate with rust-analyzer alone; this is the entry point for that.

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

#### Re-export `dilution_factors`

```rust
pub use crate::activity::chi_over_q::dilution_factors;
```

#### Re-export `DilutionFactors`

```rust
pub use crate::activity::chi_over_q::DilutionFactors;
```

#### Re-export `StabilitySource`

```rust
pub use crate::activity::chi_over_q::StabilitySource;
```

#### Re-export `dry_deposition`

```rust
pub use crate::activity::deposition::dry_deposition;
```

#### Re-export `DepositionGroup`

```rust
pub use crate::activity::deposition::DepositionGroup;
```

#### Re-export `DryDepositionVelocity`

```rust
pub use crate::activity::deposition::DryDepositionVelocity;
```

#### Re-export `NuclideRelease`

```rust
pub use crate::activity::source::NuclideRelease;
```

#### Re-export `ReleaseWindow`

```rust
pub use crate::activity::source::ReleaseWindow;
```

#### Re-export `SourceTerm`

```rust
pub use crate::activity::source::SourceTerm;
```

#### Re-export `survey`

```rust
pub use crate::activity::survey::survey;
```

#### Re-export `DepositionVelocities`

```rust
pub use crate::activity::survey::DepositionVelocities;
```

#### Re-export `NuclideTotals`

```rust
pub use crate::activity::survey::NuclideTotals;
```

#### Re-export `SiteSurvey`

```rust
pub use crate::activity::survey::SiteSurvey;
```

#### Re-export `DilutionFactor`

```rust
pub use crate::activity::units::DilutionFactor;
```

#### Re-export `GroundDeposition`

```rust
pub use crate::activity::units::GroundDeposition;
```

#### Re-export `TimeIntegratedAirConcentration`

```rust
pub use crate::activity::units::TimeIntegratedAirConcentration;
```

#### Re-export `part0`

```rust
pub use crate::flexpart::aerosol::part0;
```

#### Re-export `AerosolBins`

```rust
pub use crate::flexpart::aerosol::AerosolBins;
```

#### Re-export `constants`

```rust
pub use crate::flexpart::constants;
```

#### Re-export `decay_constant`

```rust
pub use crate::flexpart::decay::decay_constant;
```

#### Re-export `decay_constant_exact`

```rust
pub use crate::flexpart::decay::decay_constant_exact;
```

#### Re-export `decayed`

```rust
pub use crate::flexpart::decay::decayed;
```

#### Re-export `surviving_fraction`

```rust
pub use crate::flexpart::decay::surviving_fraction;
```

#### Re-export `obukhov`

```rust
pub use crate::flexpart::surface_layer::obukhov;
```

#### Re-export `psih`

```rust
pub use crate::flexpart::surface_layer::psih;
```

#### Re-export `psim`

```rust
pub use crate::flexpart::surface_layer::psim;
```

#### Re-export `raerod`

```rust
pub use crate::flexpart::surface_layer::raerod;
```

#### Re-export `scalev`

```rust
pub use crate::flexpart::surface_layer::scalev;
```

#### Re-export `MetDataFormat`

```rust
pub use crate::flexpart::surface_layer::MetDataFormat;
```

#### Re-export `gaussian_puff_concentration`

```rust
pub use crate::puff::concentration::gaussian_puff_concentration;
```

#### Re-export `gaussian_puff_methane_ppm`

```rust
pub use crate::puff::concentration::gaussian_puff_methane_ppm;
```

#### Re-export `METHANE_PPM_PER_KG_PER_M3`

```rust
pub use crate::puff::concentration::METHANE_PPM_PER_KG_PER_M3;
```

#### Re-export `pasquill_gifford_sigmas`

```rust
pub use crate::puff::dispersion::pasquill_gifford_sigmas;
```

#### Re-export `DispersionSigmas`

```rust
pub use crate::puff::dispersion::DispersionSigmas;
```

#### Re-export `constant_wind`

```rust
pub use crate::puff::simulate::constant_wind;
```

#### Re-export `simulate_grid_mode`

```rust
pub use crate::puff::simulate::simulate_grid_mode;
```

#### Re-export `simulate_sensor_mode`

```rust
pub use crate::puff::simulate::simulate_sensor_mode;
```

#### Re-export `EmissionPolicy`

```rust
pub use crate::puff::simulate::EmissionPolicy;
```

#### Re-export `GridSeries`

```rust
pub use crate::puff::simulate::GridSeries;
```

#### Re-export `Receptor`

```rust
pub use crate::puff::simulate::Receptor;
```

#### Re-export `RunConfig`

```rust
pub use crate::puff::simulate::RunConfig;
```

#### Re-export `SensorSeries`

```rust
pub use crate::puff::simulate::SensorSeries;
```

#### Re-export `Source`

```rust
pub use crate::puff::simulate::Source;
```

#### Re-export `is_day`

```rust
pub use crate::puff::stability::is_day;
```

#### Re-export `stability_class`

```rust
pub use crate::puff::stability::stability_class;
```

#### Re-export `StabilityClass`

```rust
pub use crate::puff::stability::StabilityClass;
```

#### Re-export `StabilitySet`

```rust
pub use crate::puff::stability::StabilitySet;
```

#### Re-export `interpolate_wind`

```rust
pub use crate::puff::wind::interpolate_wind;
```

#### Re-export `wind_speed`

```rust
pub use crate::puff::wind::wind_speed;
```

#### Re-export `wind_vector_convert`

```rust
pub use crate::puff::wind::wind_vector_convert;
```

#### Re-export `WindComponents`

```rust
pub use crate::puff::wind::WindComponents;
```

#### Re-export `dynamic_viscosity_of_air`

```rust
pub use crate::flexpart::thermo::dynamic_viscosity_of_air;
```

#### Re-export `ew_kelvin`

```rust
pub use crate::flexpart::thermo::ew_kelvin;
```

#### Re-export `saturation_vapour_pressure`

```rust
pub use crate::flexpart::thermo::saturation_vapour_pressure;
```

#### Re-export `viscosity_kelvin`

```rust
pub use crate::flexpart::thermo::viscosity_kelvin;
```

