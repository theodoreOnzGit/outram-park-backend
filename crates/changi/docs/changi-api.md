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
~~only its surface-layer and deposition scalar kernels are ported so far~~
**CORRECTED 2026-10-03:** `flexpart/mod.rs` now declares 34 ported modules
(among them `advance`, `turbulence`, `convection`, `wet_deposition`,
`concentration`); see that module for which are verified, and read it as a
port in progress rather than "FLEXPART in Rust". [`puff`] is complete for upstream's physics, needs no meteorological
input, and is cheap enough to run interactively over a site-sized domain —
but its dispersion fits are empirical over roughly 0.1–10 km and its unit
conversion is methane-specific. See each module for what it does and does
not cover.

## What it builds on

- [`petir`] — the workspace's core numerics crate, for `erf` and (in later
  phases) interpolation, quadrature and ODE integration. FLEXPART's own
  `erf.f90` is deliberately not ported.
- ~~`outram-mc-libs`' LCG, for the pseudo-random numbers the Langevin
  turbulence scheme will need. Not yet wired in — no stochastic code has
  landed.~~ **CORRECTED 2026-10-03:** `petir`'s LCG (`petir::rng::lcg`) is
  the random source, and it is wired in: the stochastic `advance` mode and
  `tests/flexpart_stochastic_advance.rs` draw from it.
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
  **CORRECTED 2026-10-04** (checked against `src/activity/` and
  `reference/`): it holds **five**, not three; also the two design-basis
  accident releases ([`activity::accident_airborne_release`], Table 8,
  2026-09-28) and the fuel-element release rate
  ([`activity::fuel_release`], Table 2, 2026-09-29).

## Licence

GPL-3.0, containing a port of FLEXPART (GPL-3.0-or-later). See
`LICENSE.flexpart` and `NOTICE.flexpart` at the crate root. Independent
fork; not affiliated with or endorsed by NILU or the FLEXPART developers.

## Modules

## Module `activity`

Radionuclide activity in air and on the ground, from a released source term.

This module is **not a port**. Everything under it was written here, and it
therefore has **no upstream and no code-to-code verification**. ~~The
strongest evidence it can carry is internal consistency, which is what its
tests assert.~~ **CORRECTED 2026-09-29** (gh:#380): [`chi_over_q`] now has an
**independent** check, `crates/buangkok/tests/changi_puff_train_vs_plume.rs`.
At constant wind and fixed stability class, `dilution_factors` matches an
age sum written without any of this module's code to <= 3.9e-16, and in the
steady limit reproduces buangkok's pyDOSEIA Gaussian plume with the
residual a second-order expansion predicts. Varying wind, `FromWind`
stability, deposition and the survey are still covered by internal
consistency only (`tests/activity_properties.rs`). Read every number it
produces in that light. `changi`'s two ported
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

~~One published dose table is~~ ~~two published dose tables are *stored*
here as cited reference data~~ **MOVED 2026-09-28** (maintainer: "move
table 7 and 9 to buangkok"): the published HTR-10 dose tables (Liu and Cao
2002, Tables 7 and 9) now live in the dose crate, `buangkok::published`.
This module holds no dose data. The accident releases (Table 8) stay here
as a nuclide table, and `AccidentCase` is defined in
[`accident_airborne_release`].

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

It is **not FLEXPART**. ~~`changi::flexpart` is four scalar-kernel modules that
its own documentation calls *"the first verified slice of a port"*; it cannot
transport a plume~~ **CORRECTED 2026-10-03:** `changi::flexpart` is a port in
progress (34 modules declared in `flexpart/mod.rs`, including `advance`), but
it is not used for transport here. Say which model ran
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
tabulates separately (its Table 8, ~~not digitised here~~ **CORRECTED
2026-09-28**: now in [`crate::activity::accident_airborne_release`]).

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

## Module `accident_airborne_release`

Published HTR-10 airborne release for two design-basis accidents
(depressurization, water ingress; Liu and Cao 2002, Table 8), in Bq per
accident. Reference data only; nothing in this crate consumes it.
Added 2026-09-28.
**A published airborne release from HTR-10 for two design-basis
accidents, stored as reference data.**

# What this is

The activity of each nuclide that the source calculates is released to the
environment, **in Bq per accident**, for the same two accidents as the
paper's Table 9 doses (now in `buangkok::published::accident_dose_by_distance`):

- **Depressurization accident** ([`AccidentCase::Depressurization`][D]):
  primary helium lost through a ruptured 65 mm fuel-element charging tube.
  The paper's Section 4.1.1 sums the primary-helium activity, fission
  products desorbed from primary-circuit surfaces, dust-bound activity
  (10 % of the dust assumed released) and activity bound in the helium
  purification system (100 % of the noble gases, H-3 and C-14, 10 % of the
  iodine and metal fission products, if its isolation fails).
- **Water ingress accident** ([`AccidentCase::WaterIngress`][W]): two-ended
  rupture of two steam-generator tubes with the steam relief system
  failed. The paper's Section 4.1.2 sums about 23 % of the primary-helium
  activity, water wash-off of the whole steam-generator deposit, and the
  activity in up to 4.88 kg of corroded graphite.

No release from the coated particles is assumed in either case. Both go
out through the 40 m stack with no filtering or plate-out credited. The
[`AccidentCase`][AC] enum is defined here and re-exported by buangkok's
Table 9 loader, so the two tables name the accidents identically. (It was
defined by the Table 9 loader until that moved to buangkok, 2026-09-28.)

It is a **published model result, not a measurement.** The paper does not
name a code for the release calculation. STOERNEU, named in its Section
4.2, is the code for the *doses* computed from these releases (Table 9).
The paper does **not** state the release duration or time profile, or the
inventory state (e.g. end of life) the release is taken from.

Eighteen nuclides: eight noble gases, four iodines (no I-134), Sr-90,
Cs-134, Cs-137, Ag-110m, H-3 and C-14. Sr-90 appears here although the
paper's Tables 3 and 5 list Sr-89 instead.

# The C-14 label is a correction

**The paper prints the last row as "C-4"** (text layer and rendered page
agree, and so does the maintainer's kovan record). There is no nuclide
C-4. The paper's Section 4.1.1.4 names C-14 among the released species,
its Table 5 lists C-14 in the same position (after H-3), and the water
ingress value is 0.30 of the paper's stated primary-helium C-14 total, the
same fraction as every noble gas (see the test
`water_ingress_noble_gases_h3_and_c14_are_0_30_of_primary_helium`). The row
is therefore stored as **C-14**. Asking for `"C-4"` returns `None`.

# A check against the paper's own statement, and what it found

For water ingress the paper assumes "approximate 23 %" of the primary-helium
activity is released. Wash-off acts on plated-out deposits, which do not
include noble gases, so for noble gases and H-3 the helium should be the
main source (the paper does not break down the corroded-graphite
contribution by nuclide), and their Table 8 values divided by the Table 3
primary-helium values should be about 0.23. **They are not: all eight
noble gases and H-3 give 0.295 to 0.315**, and C-14 gives 0.30 against the
text's stated total. The
uniformity across half-lives from hours to years says a single fraction
of about 0.30 was applied. The paper does not explain the difference; it
may be a different helium inventory from Table 3's end-of-life one, or a
different fraction. It is recorded, not reconciled.

# What this is NOT

- **Not wired into any model.** Nothing in this crate or in `htgr_sim_v1`
  reads it, and `changi` computes no dose from it.
- **Not a basis for emergency planning, emergency-zone sizing, siting,
  licensing or any safety decision**, for HTR-10 or any other plant.
  `RESPONSIBLE_USE.md` applies in full.
- **Not a beyond-design-basis source term** (no particle failure), and not
  the normal-operation release ([`crate::activity::airborne_release`], Table 5).

# Provenance

Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
environment impact for normal reactor operations and for relevant
accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 8
(p. 88), "The HTR-10 accidental radioactivity release (Bq)". The basis
above comes from the paper's Section 4.1 (pp. 86–89).

Access terms, digitisation and verification are in
`crates/changi/docs/References.md`. The document carries no reuse licence
and is **not** redistributed here; only the cited table of 36 values is,
which is ordinary scientific citation.

[AC]: AccidentCase
[D]: AccidentCase::Depressurization
[W]: AccidentCase::WaterIngress

```rust
pub mod accident_airborne_release { /* ... */ }
```

### Types

#### Enum `AccidentCase`

Which of the paper's two tabulated accidents a release (Table 8) or dose
(Table 9, now in `buangkok::published::accident_dose_by_distance`)
belongs to. Defined here, where the release lives; buangkok re-exports it
so both tables name the accidents identically.

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
#### Struct `AccidentReleaseEntry`

One row of the published accident release.

```rust
pub struct AccidentReleaseEntry {
    pub nuclide: &'static str,
    pub depressurization: uom::si::f64::Radioactivity,
    pub water_ingress: uom::si::f64::Radioactivity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `&'static str` | Nuclide label, as the source tabulates it except that the source's<br>"C-4" is stored as `"C-14"` (see the module docs). |
| `depressurization` | `uom::si::f64::Radioactivity` | Activity released to the environment in the depressurization accident. |
| `water_ingress` | `uom::si::f64::Radioactivity` | Activity released to the environment in the water ingress accident. |

##### Implementations

###### Methods

- ```rust
  pub fn release(self: &Self, case: AccidentCase) -> Radioactivity { /* ... */ }
  ```
  The release for one accident case.

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
    fn clone(self: &Self) -> AccidentReleaseEntry { /* ... */ }
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
    fn eq(self: &Self, other: &AccidentReleaseEntry) -> bool { /* ... */ }
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

#### Function `htr10_accident_release`

**Attributes:**

- `MustUse { reason: None }`

Every nuclide in the published HTR-10 accident release, in the source's
order (eighteen entries).

The CSV has three columns, so the two-column
`inventory::parse_nuclide_bq_csv` cannot read it; this parser
follows the same rules (header skipped, a row that does not parse is
dropped, which the row-count test catches).

```rust
pub fn htr10_accident_release() -> Vec<AccidentReleaseEntry> { /* ... */ }
```

#### Function `htr10_accident_airborne_release`

**Attributes:**

- `MustUse { reason: None }`

Look one nuclide's release up by label and accident.

Returns `None` for a nuclide the table does not list. An absent nuclide
was not reported, which is not the same thing as a zero release.

```rust
pub fn htr10_accident_airborne_release(nuclide: &str, case: AccidentCase) -> Option<uom::si::f64::Radioactivity> { /* ... */ }
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

## Module `fuel_release`

A published HTR-10 core inventory, so a source term can be built from
measured magnitudes rather than round illustrative numbers. An inventory
is NOT a source term -- see the module docs.
**A published HTR-10 release rate of fission products from the fuel
elements, equilibrium core, normal operation.**

# What this is

The rate at which each important fission product (and tritium) leaves the
**fuel elements** into the primary helium, per hour and per megawatt
thermal, for the equilibrium core in normal operation -- the source term of
the primary circuit, upstream of [`super::primary_helium`] (what then
accumulates in the helium) and [`super::airborne_release`] (what reaches the
environment). Unit: **Bq h^-1 MWt^-1**, stated in the table's column header.

The source's text gives the basis: fission products released from the
coated particles by diffusion, the maximum fuel-centre temperature in normal
operation being 864 degC (section 2.3), with free uranium and defective
particles the dominant release source.

# What this is NOT

- **Not an input.** `htgr_sim_v1`'s stage-1 release V&V test compares its
  **uncalibrated** release against it (gh:#399); nothing is tuned to it.
- **Not a release to the environment**, and not a dose.
- `RESPONSIBLE_USE.md` applies: nothing here may be quoted as a release
  figure for HTR-10 or any other plant for any operational, licensing or
  safety purpose.

# Provenance

Liu Yuanzhong and Cao Jianzhu, *Nuclear Engineering and Design* **218**
(2002) 81-90, **Table 2** (p. 83), "Release rates of important fission
products from the fuel elements in the equilibrium core", 22 nuclides.
Transcribed 2026-09-29 from the text layer of the maintainer's copy; see
`crates/changi/docs/References.md`. Restricted access: cited, not
redistributed.

```rust
pub mod fuel_release { /* ... */ }
```

### Functions

#### Function `htr10_fuel_element_release_rate_bq_per_h_per_mwt`

**Attributes:**

- `MustUse { reason: None }`

One nuclide's release rate from the fuel elements \[Bq h^-1 MWt^-1\].

```rust
pub fn htr10_fuel_element_release_rate_bq_per_h_per_mwt(nuclide: &str) -> Option<f64> { /* ... */ }
```

## Module `inventory`

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

- **Not wired into any model.** ~~Nothing in this crate or in `htgr_sim_v1`
  reads it.~~ **CORRECTED 2026-09-29**: `htgr_sim_v1`'s stage-1 release
  V&V test compares its uncalibrated live pools against it (gh:#399); no
  model takes it as an input.
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
so a receptor at 10 km reads ~~**exactly zero**~~ only the Gaussian tail of
the oldest puffs, with no error and no warning. **CORRECTED 2026-10-04**
(measured, `examples/site_activity_survey.rs`, 2 m/s, class D, reach
5000 m): the 8000 m receptor reads 1.06e-19 Bq·s/m³ of Kr-88 against
9.59e4 at 5000 m, 24 decades down but not zero; a receptor reads exactly
zero only once that tail underflows. Either way the far field is missing,
not small.
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

This module covers the **surface-layer, turbulence, deposition and
boundary-layer kernels**: the functions that turn meteorological fields
into turbulence statistics, deposition velocities and mixing heights. Each
is verified against the real FLEXPART, compiled from upstream source, with
no meteorological input files, no GRIB reader and no NetCDF. The first set
depends only on `par_mod` constants. The stage-1 set (2026-10-02) reads
`com_mod`, and its driver writes synthetic fields there (see
`docs/flexpart-code-to-code.md`).

| Submodule | Upstream files | Content |
|---|---|---|
| [`constants`] | `par_mod.f90` | Physical constants |
| [`thermo`] | `ew.f90`, `dynamic_viscosity.f90` | Saturation vapour pressure, dynamic viscosity |
| [`surface_layer`] | `psim.f90`, `psih.f90`, `scalev.f90`, `obukhov.f90`, `raerod.f90` | Monin–Obukhov similarity, friction velocity, aerodynamic resistance |
| [`aerosol`] | `part0.f90` | Lognormal size distribution, settling, Cunningham, Schmidt |
| [`decay`] | `readreleases.f90`, `timemanager.f90` | Radioactive decay |
| [`turbulence`] | `hanna*.f90`, `windalign.f90` | Hanna (1982) turbulence statistics |
| [`cbl`] | `cbl.f90` | Skewed convective-boundary-layer drift and diffusion |
| [`dry_deposition`] | `getrb.f90`, `getrc.f90`, `partdep.f90`, `getvdep.f90`, `get_settling.f90` | Dry deposition velocity, settling |
| [`boundary_layer`] | `pbl_profile.f90`, `richardson.f90`, `qvsat.f90` | Profile fluxes, mixing height, saturation humidity |
| [`solar`] | `zenithangle.f90`, `photo_O1D.f90` | Solar zenith angle, O(¹D) photolysis |
| [`geodesy`] | `distance.f90`, `distance2.f90` | Great-circle distance |
| [`calendar`] | `juldate.f90`, `caldate.f90` | Julian date (day count re-derived; NR provenance) |
| [`interpolation`] | `interpol_*.f90` (+ `_nests`) | Met fields at a particle |
| [`advance`] | `advance.f90`, `initialize.f90`, `get_vdep_prob.f90` | The Lagrangian particle step |
| [`cmapf`] | `cmapf_mod.f90` | Map projections (polar stereographic, Lambert) |
| [`coordtrafo`] | `coordtrafo.f90` | Release-point coordinates |
| [`met_fields`] | `calcpar*.f90`, `calcpv*.f90` | Surface/PBL parameters, potential vorticity |
| [`wet_deposition`] | `interpol_rain*.f90`, `get_wetscav.f90`, `wetdepo.f90`, `wetdepokernel*.f90` | Wet scavenging |
| [`oh_chemistry`] | `gethourlyOH.f90`, `ohreaction.f90` | OH reaction |
| [`landuse`] | `assignland.f90` | Landuse assignment |
| [`concentration`] | `conccalc.f90`, `drydepokernel*.f90` | Concentration and deposition gridding |
| [`plume_trajectory`] | `centerofmass.f90`, `clustering.f90`, `plumetraj.f90`, `mean_mod.f90` | Plume statistics |
| [`particle_average`] | `partpos_average.f90` | Particle-position averages |
| [`convection`] | `convect43c.f90` | Emanuel convection |
| [`convmix`] | `calcmatrix.f90`, `redist.f90`, `convmix.f90` | Convective redistribution |
| [`verttransform`] | `verttransform_ecmwf.f90`, `_gfs.f90`, `_nests.f90` | Model levels to FLEXPART's z grid |
| [`shift_field`] | `shift_field.f90`, `shift_field_0.f90` | Global-grid shifts |
| [`release`] | `releaseparticles.f90` | Particle release |
| [`domainfill`] | `init_domainfill.f90`, `boundcond_domainfill.f90` | Domain filling |
| [`outgrid`] | `outgrid_init*.f90` | Output-grid areas and volumes |
| [`fluxes`] | `calcfluxes.f90`, `fluxoutput.f90` | Mass fluxes |
| [`initial_condition`] | `initial_cond_calc.f90` | Backward initial conditions |
| [`concoutput`] | `concoutput*.f90` | Concentration output conversion |
| [`timemanager`] | `timemanager.f90` (inline blocks) | Per-step bookkeeping, output clock |

## What is NOT ported

~~The particle advection loop (`advance.f90`), the meteorological
interpolation, wet scavenging, the output grids, and the OH reaction.~~
**CORRECTED 2026-10-02**: all of those, and the Hanna turbulence, `cbl.f90`
and the Richardson mixing height, are ported and verified (gh:#410). Still
not ported: the GRIB/NetCDF readers and the file writers, which are I/O
rather than numerics. (**CORRECTED 2026-10-02**: `verttransform_*`,
particle release and domain filling, the output-grid set-up, the
`concoutput*` conversion and `timemanager`'s bookkeeping were listed here;
they are ported and verified, gh:#410.)
Do not read this module as "FLEXPART in Rust": it is a verified set of its
kernels.

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

## Module `advance`

The Lagrangian particle step: `advance.f90` moves one particle through one
synchronisation interval, and `initialize.f90` gives a newly released
particle its initial turbulent velocities.

# Random numbers are an INPUT

Upstream draws from two sources: the pre-computed Gaussian array
`rannumb(maxrand)` in `com_mod`, and `ran3` (Numerical Recipes, in
`random_mod.f90`) once per call, to pick the starting index
`nrand = int(ran3 * (maxrand-1)) + 1`. Numerical Recipes code is not ported
and not re-implemented here (maintainer decision, gh:#410). So the port
takes both as arguments: `rannumb` as a slice (1-based in the comments,
`rannumb[n-1]` in code) and the starting index `nrand0`. Given the same
draws, the step is deterministic, and that is what the code-to-code test
verifies. Where the draws come from is the caller's choice: the stochastic
comparison uses `petir`'s LCG and RAFFLES' samplers. FLEXPART's own
`rannumb` is clipped to `[-3, 3]`; reproduce that with [`limit_rannumb`].

# State shared through modules

`advance` reads and writes `interpol_mod` ([`Interpolator`]) and
`hanna_mod` ([`HannaState`]), and both carry values from one particle to
the next. Two cases matter:
- the "already interpolated" flags are only reset for levels `1..nmixz`;
- `u`, `v`, `w` from the previous step feed the Petterssen correction.

Pass the same two structs to successive calls, as upstream's call sequence
does.

# Upstream quirks reproduced (each verified against the compiled Fortran)

- After the `ifine` sub-step loop, `nrand = nrand + i` adds the loop
  variable's **exit** value, `ifine + 1`, not `ifine`.
- At a pole crossing, `xt = mod(xt + 180, 360)` is applied to a **grid**
  coordinate, as though it were a longitude in degrees.
- On a nest, `ddx = xt - ix` mixes the mother-grid `xt` with the nest index
  `ix`. The weights it sets are overwritten by `interpol_all_nests` before
  any wind is used, so the only effect is on `interpolhmix` (next item).
- With `interpolhmix` on a nest, `h` is computed from the array `h1`, which
  upstream never assigns on that branch. The port refuses
  ([`AdvanceError::UndefinedMixingHeight`]) rather than guess.
- `memindnext` is computed and never used. It is not ported.
- `tropop` and `get_settling` read memory slot **1** (`tropopause(...,1,1)`,
  `rho(...,1)`), not the time-interpolated field.
- In `initialize`, the draws used for `up`, `vp`, `wp` in the boundary
  layer are used **again** for `usigold`, `vsigold`, `wsigold`, because
  `nrand` is not advanced in between.

# Units

FLEXPART's: positions in grid units (`xt`, `yt`) and m (`zt`), velocities
m/s, times s.

```rust
pub mod advance { /* ... */ }
```

### Types

#### Struct `AdvanceSettings`

Run settings `advance` reads from `com_mod`.

```rust
pub struct AdvanceSettings {
    pub ldirect: i64,
    pub lsynctime: i64,
    pub method: i32,
    pub ctl: f64,
    pub ifine: i64,
    pub fine: f64,
    pub mintime: i64,
    pub turbswitch: bool,
    pub cblflag: bool,
    pub turboff: bool,
    pub drydep: bool,
    pub drydepspec: Vec<bool>,
    pub d_trop: f64,
    pub d_strat: f64,
    pub lwindinterv: i64,
    pub turbmesoscale: f64,
    pub mdomainfill: i32,
    pub lsettling: bool,
    pub interpolhmix: bool,
    pub nmixz: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ldirect` | `i64` | `+1` forward, `-1` backward in time. |
| `lsynctime` | `i64` | Synchronisation interval, s (signed with `ldirect`). |
| `method` | `i32` | `1`: time step from the Lagrangian time scale; otherwise one step per<br>synchronisation interval. |
| `ctl` | `f64` | Fraction of the Lagrangian time scale used as time step (`ctl`). |
| `ifine` | `i64` | Number of vertical sub-steps (`ifine`). |
| `fine` | `f64` | `1 / ifine` as upstream stores it (`fine`). |
| `mintime` | `i64` | Minimum time step, s. |
| `turbswitch` | `bool` | `true`: `hanna` + Gaussian scheme; `false`: `hanna1` + well-mixed. |
| `cblflag` | `bool` | Skewed CBL scheme (`cblflag == 1`). |
| `turboff` | `bool` | Turbulence switched off (`turboff`). |
| `drydep` | `bool` | Dry deposition on (`DRYDEP`). |
| `drydepspec` | `Vec<bool>` | Per-species dry deposition (`DRYDEPSPEC`). |
| `d_trop` | `f64` | Horizontal diffusivity in the troposphere, m²/s. |
| `d_strat` | `f64` | Vertical diffusivity in the stratosphere, m²/s. |
| `lwindinterv` | `i64` | Interval between wind fields, s. |
| `turbmesoscale` | `f64` | Mesoscale-turbulence factor. |
| `mdomainfill` | `i32` | Domain-filling mode (`0` = off). |
| `lsettling` | `bool` | Gravitational settling on (`lsettling`). |
| `interpolhmix` | `bool` | Interpolate the mixing height bilinearly instead of taking the corner<br>maximum (`interpolhmix`). |
| `nmixz` | `usize` | Number of levels whose profiles are re-interpolated each step<br>(`nmixz`). |

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
    fn clone(self: &Self) -> AdvanceSettings { /* ... */ }
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
    fn eq(self: &Self, other: &AdvanceSettings) -> bool { /* ... */ }
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
#### Struct `NestGeometry`

One nested grid's placement in mother-grid coordinates.

```rust
pub struct NestGeometry {
    pub xln: f64,
    pub yln: f64,
    pub xrn: f64,
    pub yrn: f64,
    pub xresoln: f64,
    pub yresoln: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xln` | `f64` | Lower-left corner, mother-grid units. |
| `yln` | `f64` | Lower-left corner, mother-grid units. |
| `xrn` | `f64` | Upper-right corner, mother-grid units. |
| `yrn` | `f64` | Upper-right corner, mother-grid units. |
| `xresoln` | `f64` | Nest points per mother-grid unit. |
| `yresoln` | `f64` | Nest points per mother-grid unit. |

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
    fn clone(self: &Self) -> NestGeometry { /* ... */ }
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
    fn eq(self: &Self, other: &NestGeometry) -> bool { /* ... */ }
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
#### Struct `Domain`

The model domain as `advance` sees it.

```rust
pub struct Domain {
    pub nxmin1: i64,
    pub nymin1: i64,
    pub nxmax: i64,
    pub nymax: i64,
    pub xglobal: bool,
    pub nglobal: bool,
    pub sglobal: bool,
    pub switchnorthg: f64,
    pub switchsouthg: f64,
    pub dx: f64,
    pub dy: f64,
    pub xlon0: f64,
    pub ylat0: f64,
    pub dxconst: f64,
    pub dyconst: f64,
    pub northpolemap: super::cmapf::Strcmp,
    pub southpolemap: super::cmapf::Strcmp,
    pub nests: Vec<NestGeometry>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nxmin1` | `i64` | `nx - 1`. |
| `nymin1` | `i64` | `ny - 1`. |
| `nxmax` | `i64` | The compiled `nxmax` (array dimension), which sets upstream's<br>`eps = nxmax / 3e5`. |
| `nymax` | `i64` | The compiled `nymax` (array dimension). |
| `xglobal` | `bool` | Global in longitude. |
| `nglobal` | `bool` | Covers the north pole (use the polar grid north of `switchnorthg`). |
| `sglobal` | `bool` | Covers the south pole. |
| `switchnorthg` | `f64` | Grid `y` above which the north-polar grid is used. |
| `switchsouthg` | `f64` | Grid `y` below which the south-polar grid is used. |
| `dx` | `f64` | Grid spacing, degrees. |
| `dy` | `f64` | Grid spacing, degrees. |
| `xlon0` | `f64` | Longitude of the grid origin, degrees. |
| `ylat0` | `f64` | Latitude of the grid origin, degrees. |
| `dxconst` | `f64` | Metres-to-grid-units factors (`dxconst`, `dyconst`). |
| `dyconst` | `f64` | Metres-to-grid-units factors (`dxconst`, `dyconst`). |
| `northpolemap` | `super::cmapf::Strcmp` | Polar-stereographic projections (`cmapf` descriptors). |
| `southpolemap` | `super::cmapf::Strcmp` | Polar-stereographic projections (`cmapf` descriptors). |
| `nests` | `Vec<NestGeometry>` | The nests, innermost last (upstream searches from the last). |

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
    fn clone(self: &Self) -> Domain { /* ... */ }
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
    fn eq(self: &Self, other: &Domain) -> bool { /* ... */ }
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
#### Struct `SettlingSpecies`

Properties of one species that `advance` needs for settling.

```rust
pub struct SettlingSpecies {
    pub density: f64,
    pub dquer: f64,
    pub cunningham: f64,
    pub vsetaver: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `f64` | Particle density, kg/m³ (`<= 0` for a gas). |
| `dquer` | `f64` | Mean diameter, µm. |
| `cunningham` | `f64` | Cunningham factor. |
| `vsetaver` | `f64` | Stokes settling velocity, m/s (negative). |

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
    fn clone(self: &Self) -> SettlingSpecies { /* ... */ }
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
    fn eq(self: &Self, other: &SettlingSpecies) -> bool { /* ... */ }
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
#### Struct `Particle`

One particle's prognostic state.

```rust
pub struct Particle {
    pub xt: f64,
    pub yt: f64,
    pub zt: f64,
    pub up: f64,
    pub vp: f64,
    pub wp: f64,
    pub usigold: f64,
    pub vsigold: f64,
    pub wsigold: f64,
    pub icbt: i64,
    pub prob: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xt` | `f64` | Position, mother-grid units (`real(kind=dp)` upstream). |
| `yt` | `f64` | Position, mother-grid units. |
| `zt` | `f64` | Height above ground, m. |
| `up` | `f64` | Turbulent velocity components, m/s (`wp` is scaled by `sigma_w` in<br>the Gaussian scheme). |
| `vp` | `f64` | Turbulent velocity components. |
| `wp` | `f64` | Turbulent velocity components. |
| `usigold` | `f64` | Mesoscale velocity fluctuations, m/s. |
| `vsigold` | `f64` | Mesoscale velocity fluctuations, m/s. |
| `wsigold` | `f64` | Mesoscale velocity fluctuations, m/s. |
| `icbt` | `i64` | `+1`, or `-1` after a reflection in the last sub-step. |
| `prob` | `Vec<f64>` | Per-species probability of having been deposited this interval. |

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
    fn clone(self: &Self) -> Particle { /* ... */ }
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
    fn eq(self: &Self, other: &Particle) -> bool { /* ... */ }
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
#### Struct `AdvanceOutcome`

What [`advance`] reports besides the updated particle.

```rust
pub struct AdvanceOutcome {
    pub nstop: i32,
    pub nan_count: u64,
    pub nan_count2: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nstop` | `i32` | `3` if the particle left the domain, else `0`. |
| `nan_count` | `u64` | Particles re-initialised by the CBL scheme (`nan_count` increment). |
| `nan_count2` | `u64` | NaN velocities replaced in the CBL branch (`nan_count2` increment). |

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
    fn clone(self: &Self) -> AdvanceOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &AdvanceOutcome) -> bool { /* ... */ }
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
#### Enum `AdvanceError`

Why [`advance`] refused.

```rust
pub enum AdvanceError {
    UndefinedMixingHeight,
    AboveTopLevel,
    RandomNumbersExhausted,
}
```

##### Variants

###### `UndefinedMixingHeight`

`interpolhmix` on a nest: upstream reads the unassigned `h1`.

###### `AboveTopLevel`

The particle is not below the top model level; upstream would reuse a
stale level index.

###### `RandomNumbersExhausted`

`re_initialize_particle` ran out of draws.

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
    fn clone(self: &Self) -> AdvanceError { /* ... */ }
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
    fn eq(self: &Self, other: &AdvanceError) -> bool { /* ... */ }
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

#### Function `limit_rannumb`

**Attributes:**

- `MustUse { reason: None }`

One `rannumb` entry from a standard normal deviate, clipped as
`gasdev1` clips it (see [`RANNUMB_LIMIT`]). The generator is the caller's.

```rust
pub fn limit_rannumb(x: f64) -> f64 { /* ... */ }
```

#### Function `advance`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments, clippy::too_many_lines,\nclippy::needless_range_loop)]")`

`advance.f90`: move one particle through one synchronisation interval.

# Arguments
- `itime` — current model time, s.
- `ldt` — in: the particle's last time step; out: its next one, s.
- `p` — the particle.
- `nrand0` — the starting index into `rannumb` (1-based), i.e. upstream's
  `int(ran3(idummy) * (maxrand-1)) + 1`.
- `rannumb` — the Gaussian draws (`maxrand = rannumb.len()`).
- `ip`, `hs` — `interpol_mod` and `hanna_mod`, carried between calls.
- `met`, `nests` — the mother grid and the nests.
- `dom`, `cfg` — domain and run settings.
- `species`, `xmass` — settling properties, and the release point's mass
  per species (picks the species whose settling applies).

# Errors
See [`AdvanceError`]: each is a state in which upstream reads an undefined
value.

```rust
pub fn advance(itime: i64, ldt: &mut i64, p: &mut Particle, nrand0: usize, rannumb: &[f64], ip: &mut super::interpolation::Interpolator, hs: &mut super::turbulence::HannaState, met: &super::interpolation::MetFields, nests: &[super::interpolation::MetFields], dom: &Domain, cfg: &AdvanceSettings, species: &[SettlingSpecies], xmass: &[f64]) -> Result<AdvanceOutcome, AdvanceError> { /* ... */ }
```

#### Function `initialize`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`initialize.f90`: initial turbulent and mesoscale velocities, and the first
time step, of a newly released particle.

Uses the mother grid only, as upstream does, but does **not** set
`ngrid`: `interpol_all` then reads the polar winds if the previous
`advance` call left `ngrid < 0`. The port keeps that (pass the same
[`Interpolator`]). `nrand0` is upstream's
`int(ran3(idummy) * (maxrand-1)) + 1`; `cbl_draws` the `(ran3, gasdev)`
pair `initialize_cbl_vel` would draw (used only in the CBL branch).

# Returns
The first time step `ldt`, s.

```rust
pub fn initialize(itime: i64, p: &mut Particle, nrand0: usize, rannumb: &[f64], cbl_draws: (f64, f64), ip: &mut super::interpolation::Interpolator, hs: &mut super::turbulence::HannaState, met: &super::interpolation::MetFields, cfg: &AdvanceSettings) -> Result<i64, AdvanceError> { /* ... */ }
```

#### Function `get_vdep_prob`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]")`

`get_vdep_prob.f90`: the dry deposition velocity of each species at a
receptor particle, written into `prob` (upstream stores the **velocity**,
m/s, in the array it calls `prob`).

# Upstream quirk reproduced

It sets the cell indices `ix`, `jy`, `ixp`, `jyp` and `ngrid`, but never
the bilinear and time weights `p1..p4`, `dt1`, `dt2`, `dtt`, so
`interpol_vdep` uses whatever weights the previous interpolation left in
`interpol_mod`. The port reads them from the [`Interpolator`] in the same
way, so pass the one shared with [`advance`].

Species without `DRYDEPSPEC`, and every species when `zt >= 2 href` or dry
deposition is off, get `0` (upstream's reset value), or keep the previous
value when `DRYDEP` is off, as upstream.

```rust
pub fn get_vdep_prob(xt: f64, yt: f64, zt: f64, prob: &mut [f64], ip: &mut super::interpolation::Interpolator, met: &super::interpolation::MetFields, nests: &[super::interpolation::MetFields], dom: &Domain, cfg: &AdvanceSettings) { /* ... */ }
```

### Constants and Statics

#### Constant `RANNUMB_LIMIT`

The bound FLEXPART puts on every Gaussian draw in `rannumb`.

`random_mod.f90`'s `gasdev1`, which fills `rannumb` at start-up, clips each
deviate to `[-3, 3]` ("Limit the random numbers to lie within the interval
-3 and +3"). That is a modelling choice, not a property of the generator:
it lowers the variance of the draws to `0.99501` and so narrows every
turbulent velocity distribution FLEXPART samples by 0.5 %. Measured on the
compiled upstream: the mean square of a filled `rannumb` is `0.99534` over
16 seeds (gh:#410, stage 7). A port fed unclipped `N(0, 1)` draws is
measurably more diffusive than FLEXPART, so fill `rannumb` through
[`limit_rannumb`].

```rust
pub const RANNUMB_LIMIT: f64 = 3.0;
```

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

## Module `boundary_layer`

Boundary-layer diagnostics from a meteorological column: surface fluxes
from the profile method (`pbl_profile`), the bulk-Richardson mixing height
with its convective velocity scale (`richardson`), and saturation specific
humidity (`f_qvsat`).

# Units

FLEXPART's, as bare `f64`: pressure Pa, temperature K, heights m, wind
m/s, heat flux W/m², stress N/m², specific humidity kg/kg.

```rust
pub mod boundary_layer { /* ... */ }
```

### Types

#### Struct `PblProfile`

What [`pbl_profile`] returns.

```rust
pub struct PblProfile {
    pub stress: f64,
    pub hf: f64,
    pub ustar: f64,
    pub ol: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stress` | `f64` | Surface stress, N/m². |
| `hf` | `f64` | Sensible heat flux, W/m² (FLEXPART's sign convention: positive is<br>downward, so negative means unstable). |
| `ustar` | `f64` | Friction velocity, m/s. |
| `ol` | `f64` | Obukhov length, m, clamped to `±9999`. |

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
    fn clone(self: &Self) -> PblProfile { /* ... */ }
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
    fn eq(self: &Self, other: &PblProfile) -> bool { /* ... */ }
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
#### Struct `MixingHeight`

What [`richardson`] returns.

```rust
pub struct MixingHeight {
    pub h: f64,
    pub wst: f64,
    pub hmixplus: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `h` | `f64` | Mixing height `h`, m above ground. |
| `wst` | `f64` | Convective velocity scale `w*`, m/s (0 unless the heat flux is upward). |
| `hmixplus` | `f64` | Turbulent-entrainment increment `hmixplus`, m; `9999` when the layer<br>above `h` is not stably stratified. |

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
    fn clone(self: &Self) -> MixingHeight { /* ... */ }
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
    fn eq(self: &Self, other: &MixingHeight) -> bool { /* ... */ }
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

#### Function `pbl_profile`

**Attributes:**

- `MustUse { reason: None }`

`pbl_profile.f90`: surface stress and sensible heat flux from the 10 m and
lowest-model-level wind and the 2 m and lowest-level temperature (Berkowicz
and Prahm 1982 profile method).

# Arguments
- `ps` — surface pressure, Pa.
- `td2m` — 2 m dew point, K.
- `zml1` — height of the lowest model level, m.
- `t2m`, `tml1` — temperature at 2 m and at the lowest level, K.
- `u10m`, `uml1` — wind speed at 10 m and at the lowest level, m/s.

# Branches, as upstream
- wind shear `<= 0.001 m/s`: similarity not applicable, `u* = 0.01`, `hf = 0`;
- `|Δθ| <= 0.03 K`: neutral, `hf = 0`;
- stable with critical ratio `<= 1`: `L = 50 m`, no iteration;
- otherwise up to 10 successive approximations, stopping at 1 % in `L`.

```rust
pub fn pbl_profile(ps: f64, td2m: f64, zml1: f64, t2m: f64, tml1: f64, u10m: f64, uml1: f64) -> PblProfile { /* ... */ }
```

#### Function `f_esl`

**Attributes:**

- `MustUse { reason: None }`

`f_esl` (`qvsat.f90`): saturation vapour pressure over liquid water, Pa,
with the pressure enhancement factor (Buck 1981).

```rust
pub fn f_esl(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `f_esi`

**Attributes:**

- `MustUse { reason: None }`

`f_esi` (`qvsat.f90`): saturation vapour pressure over ice, Pa.

```rust
pub fn f_esi(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `f_qvsat`

**Attributes:**

- `MustUse { reason: None }`

`f_qvsat` (`qvsat.f90`): saturation specific humidity, kg/kg, over liquid
water at or above 253.15 K and over ice below. Upstream returns `1` where
the denominator vanishes; so does the port.

```rust
pub fn f_qvsat(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `richardson`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

`richardson.f90`: mixing height from the bulk Richardson number (Vogelezang
and Holtslag 1996), with the convective excess temperature iterated up to
three times for an unstable surface layer.

# Arguments
- `psurf` — surface pressure, Pa.
- `ust` — friction velocity, m/s.
- `akz`, `bkz` — hybrid coefficients of each level (Pa, dimensionless); the
  level pressure is `akz + bkz * psurf`.
- `ttlev`, `qvlev`, `ulev`, `vlev` — temperature (K), specific humidity
  (kg/kg) and wind components (m/s) on each level.

The level slices are 0-based (upstream's level `k` is index `k-1`) and all
the same length. For ECMWF data, index 0 is the surface level.
- `hf` — sensible heat flux, W/m² (negative = upward, unstable).
- `tt2`, `td2` — 2 m temperature and dew point, K.
- `format` — which product; ECMWF columns start at level 2, NCEP columns at
  the first pressure level above the ground. Only the variant is used; the
  coefficients it carries are `obukhov`'s, not this routine's.

# Upstream behaviour preserved
- If no level exceeds the critical Richardson number, upstream steps `k`
  back to the top level and interpolates within a zero-thickness layer.
  The Brunt–Väisälä frequency then comes out `0/0`, so `hmixplus` is
  **NaN**, and the port returns NaN there too.
- The relative-humidity profile upstream computes along the way is never
  used for any output and is not computed here.

```rust
pub fn richardson(psurf: f64, ust: f64, akz: &[f64], bkz: &[f64], ttlev: &[f64], qvlev: &[f64], ulev: &[f64], vlev: &[f64], hf: f64, tt2: f64, td2: f64, format: &super::surface_layer::MetDataFormat) -> MixingHeight { /* ... */ }
```

## Module `calendar`

FLEXPART's Julian-date calendar: `juldate` (calendar → Julian date) and
`caldate` (Julian date → calendar).

# What is ported and what is not

`caldate.f90`'s header says it is *"adapted from Numerical Recipes"*, and
`juldate.f90` uses Numerical Recipes' `julday` constants (`igreg =
15+31*(10+12*1582)`). The maintainer's ruling on FLEXPART's Numerical
Recipes code (GitHub issue #410, 2026-10-02, for `random_mod.f90`) is that it
is not ported and not re-implemented clean-room. This module follows that
ruling for the **date arithmetic**: the integer day-number conversion uses
**Howard Hinnant's `days_from_civil` / `civil_from_days`**, which are public
domain (<http://howardhinnant.github.io/date_algorithms.html>) and already the
workspace's reference implementation in `crates/kovan-metrics/src/date.rs`.
That crate cannot be a dependency here (it is outside the wasm gate), so the
two short functions are repeated with the citation.

The **time-of-day arithmetic** — hours, minutes and rounded seconds from the
fractional day, and the `ss == 60` / `mi == 60` roll-overs — is FLEXPART's own
and is ported line for line.

# Day-number convention

FLEXPART's integer day is the Julian Day Number with the day starting at
**midnight**: 1970-01-01 is day 2 440 588, and `12:00` on that day is
`2440588.5`. (Astronomical Julian dates start at noon; FLEXPART's do not.)

# Valid range

Gregorian dates from **1582-10-15** onwards. Upstream switches to the
Julian calendar before that date; Hinnant's algorithm is proleptic
Gregorian. The two agree on every date the fixture covers, which starts at
1582-10-15 itself. Upstream also refuses year 0 and shifts negative years by
one; neither is reachable for a meteorological date and neither is ported.

```rust
pub mod calendar { /* ... */ }
```

### Functions

#### Function `juldate`

**Attributes:**

- `MustUse { reason: None }`

Calendar date and time → FLEXPART Julian date (`juldate.f90`).

# Arguments
- `yyyymmdd` — date as the integer `YYYYMMDD`, e.g. `20210615`.
- `hhmmss` — time of day as the integer `HHMMSS`, e.g. `81530` for 08:15:30.

# Returns
The Julian date in days, midnight-based (see the module docs), with the
time of day added as `hh/24 + mi/1440 + ss/86400` in that order, exactly as
upstream sums it.

```rust
pub fn juldate(yyyymmdd: i64, hhmmss: i64) -> f64 { /* ... */ }
```

#### Function `caldate`

**Attributes:**

- `MustUse { reason: None }`

FLEXPART Julian date → calendar date and time (`caldate.f90`).

# Arguments
- `jul` — Julian date in days, midnight-based.

# Returns
`(yyyymmdd, hhmmss)` as integers.

# Rounding, ported from upstream

Hours and minutes are **truncated**, seconds are **rounded** (`nint`, half
away from zero). A second count that rounds up to 60 rolls into the minute,
and a minute count of 60 rolls into the hour. Upstream does **not** roll an
hour count of 24 into the next day, and neither does this port: a Julian
date within half a second of midnight returns `hhmmss = 240000` on the
earlier date.

```rust
pub fn caldate(jul: f64) -> (i64, i64) { /* ... */ }
```

## Module `cbl`

Skewed convective-boundary-layer turbulence (Cassiani et al. 2015,
*Boundary-Layer Meteorol.* 154, 367–390): the drift and diffusion terms of
the Langevin equation for vertical velocity when `cblflag` is set.

The vertical-velocity PDF is a bi-Gaussian (Luhar et al. 1996 closure, with
`costluar4 = 0.66667`) whose third moment follows the Lenschow profile
`w'^3 = 1.2 zeta (1 - zeta)^{3/2} w*^3`, tapered to Gaussian by a sine
`transition` as `-h/L` falls below 15. The routine returns the PDF value,
the flux terms and the drift/diffusion pair `(a, b)` that `advance.f90`
uses to update `w`.

# Translation notes

* The arithmetic is transcribed **operation for operation**, in Fortran's
  left-to-right order, so the port agrees with a `real(8)` build of upstream
  to the last bit or two. Fortran integer powers (`x**2`, `x**3`) are
  products; real powers (`x**1.5`, `x**0.5`, `x**(-2.)`) are `powf`, as
  gfortran emits them.
* `erf` is the gfortran **intrinsic** in upstream (the `real :: erf`
  declaration has no `external`, so the intrinsic wins over `erf.f90`).
  The port uses [`petir::specfunc::erf`], per the crate's reuse rule; the
  two are compared in the code-to-code test.
* `cuberoot` is upstream's own `sign(|x|**0.333333333, x)`, kept with its
  truncated exponent rather than replaced by `cbrt`.
* `ldirect` comes from `com_mod` upstream; here it is the `time_direction`
  argument.

# Units

Bare `f64` in FLEXPART's units: velocities m/s, heights and `L` m, times s,
density kg/m³, density gradient kg/m⁴.

```rust
pub mod cbl { /* ... */ }
```

### Types

#### Struct `CblTerms`

What [`cbl`] returns.

```rust
pub struct CblTerms {
    pub ptot: f64,
    pub q: f64,
    pub phi: f64,
    pub ath: f64,
    pub bth: f64,
    pub reinitialise: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ptot` | `f64` | Bi-Gaussian PDF of `w` at the particle's velocity, times density. |
| `q` | `f64` | Upstream's `Q`. |
| `phi` | `f64` | Upstream's `Phi`, the flux term. |
| `ath` | `f64` | Drift coefficient `a` of the Langevin equation. |
| `bth` | `f64` | Diffusion coefficient `b` of the Langevin equation. |
| `reinitialise` | `bool` | True when the velocity sits more than six standard deviations from<br>both Gaussian modes; upstream sets `flagrein = 1` and `advance.f90`<br>re-initialises the particle's velocity. |

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
    fn clone(self: &Self) -> CblTerms { /* ... */ }
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
    fn eq(self: &Self, other: &CblTerms) -> bool { /* ... */ }
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

#### Function `cbl`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`cbl.f90`: drift and diffusion of `w` in the skewed convective boundary
layer.

# Arguments
- `wp` — particle vertical velocity, m/s.
- `zp` — particle height, m.
- `ust` — friction velocity, m/s (unused upstream, kept for the signature).
- `wst` — convective velocity scale `w*`, m/s.
- `h` — mixing height, m.
- `rhoa`, `rhograd` — air density and its vertical gradient.
- `sigmaw`, `dsigmawdz` — `sigma_w` and its gradient from [`super::turbulence`].
- `tlw` — Lagrangian time scale of `w`, s.
- `ol` — Obukhov length, m.
- `time_direction` — `+1` forward, `-1` backward (`ldirect`).

```rust
pub fn cbl(wp: f64, zp: f64, _ust: f64, wst: f64, h: f64, rhoa: f64, rhograd: f64, sigmaw: f64, dsigmawdz: f64, tlw: f64, ol: f64, time_direction: f64) -> CblTerms { /* ... */ }
```

#### Function `initialize_cbl_vel`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

`initialize_cbl_vel.f90`: initial vertical velocity of a particle in the
skewed CBL, m/s.

Upstream draws `dcas = ran3(idum)` (uniform) to pick the updraft or
downdraft mode and `dcas1 = gasdev(idum)` (Gaussian) within it; both are
Numerical Recipes and are taken here as inputs.

```rust
pub fn initialize_cbl_vel(zp: f64, _ust: f64, wst: f64, h: f64, sigmaw: f64, ol: f64, time_direction: f64, dcas: f64, dcas1: f64) -> f64 { /* ... */ }
```

#### Function `re_initialize_particle`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`re_initialize_particle.f90`: redraw the vertical velocity of a particle
that [`cbl`] flagged as too far in the tails, keeping its direction.

Draws `rannumb(nrand+1)`, `rannumb(nrand+2)`, ... until the velocity has the
sign of the particle's mode, advancing `nrand` (1-based) past every draw it
consumes, exactly as upstream does.

# Returns
`None` if it runs off the end of `rannumb`; upstream would read out of
bounds.

```rust
pub fn re_initialize_particle(zp: f64, _ust: f64, wst: f64, h: f64, sigmaw: f64, wp: &mut f64, nrand: &mut usize, ol: f64, time_direction: f64, rannumb: &[f64]) -> Option<()> { /* ... */ }
```

## Module `cmapf`

General conformal map projections: Albion Taylor's (NOAA/ARL) `cmapf`
library, as FLEXPART ships it in `cmapf_mod.f90`.

One family of maps covers polar stereographic (`tnglat = ±90`), Lambert
conformal (`0 < |tnglat| < 90`) and Mercator (`tnglat = 0`): the cone
constant is `gamma = sin(tnglat)`. FLEXPART uses it for exactly two maps,
`northpolemap` and `southpolemap`, built in `gridcheck_ecmwf.f90` as

```text
sizenorth = 6*(90 - switchnorth)/dy
call stlmbr(northpolemap, 90., 0.)
call stcm2p(northpolemap, 0.,0., switchnorth,0., sizenorth,sizenorth, switchnorth,180.)
```

(south: `-90`, `6*(switchsouth + 90)/dy`), and `advance.f90` moves
particles poleward of `switchnorth`/`switchsouth` on those maps with
[`cll2xy`], [`cgszll`] and [`cxy2ll`]; `verttransform_ecmwf.f90` rotates
winds onto them with [`cc2gll`].

# The map descriptor `strcmp(9)`

[`Strcmp`] holds upstream's nine-element array; `Strcmp.0[i]` is
`strcmp(i+1)`:

| index | upstream | meaning |
|---|---|---|
| 0 | `strcmp(1)` | `gamma`, sine of the tangent latitude |
| 1 | `strcmp(2)` | `lambda_0`, reference longitude (deg, in (-180, 180]) |
| 2 | `strcmp(3)` | `x_0`, grid x of the canonical origin |
| 3 | `strcmp(4)` | `y_0`, grid y of the canonical origin |
| 4 | `strcmp(5)` | cosine of the rotation from (xi, eta) to (x, y) |
| 5 | `strcmp(6)` | sine of that rotation |
| 6 | `strcmp(7)` | grid size at the equator (km per grid unit) |
| 7 | `strcmp(8)` | radial coordinate 1 degree from the north pole |
| 8 | `strcmp(9)` | radial coordinate 1 degree from the south pole |

# Translation notes

* **Line for line, f64 throughout**, in Fortran's left-to-right operation
  order, so the port agrees with a `-fdefault-real-8` build bit for bit.
  Upstream mixes default `real` (the descriptor, positions, winds) with
  `real(kind=dp)` (A. Stohl's `xi`, `eta`, `xi0`, `eta0` and most
  intermediates). It is *not* consistent about it: `cnllxy` returns `xi`,
  `eta` as default `real`, `cll2xy` stores them as `real`, and `cg2cxy`
  holds `radial` as `real`. In the shipped (real(4)) build those values
  are rounded to `f32`; the port does not reproduce that rounding — it is
  what the code-to-code test's real(4) comparison measures.
* **Constants are upstream's literals**: `pi = 3.14159265358979` (which is
  *not* `std::f64::consts::PI`; it is 3.2e-15 short), `rearth = 6371.2` km,
  `almst1 = .9999999`, and `radpdg = pi/180`, `dgprad = 180/pi` derived from
  that `pi`.
* Fortran `mod` on reals is `fmod`, which is Rust's `%`. Fortran
  `sign(a, b)` is `|a|` with the sign of `b` (`-0.0` counts as negative,
  gfortran's default), i.e. `a.abs().copysign(b)`.

# Upstream quirks, reproduced and documented

* `cc2gxy` is documented in upstream's header comment but **does not
  exist** in `cmapf_mod.f90`; it is not ported.
* Only six routines are public upstream (`cc2gll`, `cll2xy`, `cgszll`,
  `cxy2ll`, `stlmbr`, `stcm2p`); the rest are module-private and unused by
  FLEXPART. They are ported because they are part of the library and
  verified like the others.
* **The ll and xy routines use different polar thresholds.** `cc2gll` /
  `cg2cll` switch to the polar wind orientation at `|lat| > 89.985`;
  `cg2cxy` switches when its radial coordinate passes `strcmp(8)` /
  `strcmp(9)`, i.e. at `|lat| > 89` (as `stlmbr` sets them). Between 89
  and 89.985 degrees the two conventions disagree on what "north" means.
* **Mercator maps return non-finite values at the poles.** `cnllxy` sets
  `eta = 1/gamma` when `|sin(lat)| >= almst1` (`|lat| >~ 89.974`); with
  `gamma = 0` that is `+inf`, and `cll2xy` then yields `inf` or `NaN`.
* **No pole guard in `cnxyll`.** A. Stohl commented out the
  `arg1 >= almst1` guard "to avoid problems close to the poles". At the
  image of the pole `arg1` is 1 up to rounding, so `log(1 - arg1)` can be
  `log` of a tiny negative number: `NaN` latitude, reproduced. Measured
  (2026-10-02, real(8) build): the exact canonical pole `(0, 1/gamma)` of
  a southern Lambert map (tangent -35) returns `NaN`.
* **Stohl's double precision does not reach `xi0`, `eta0` in the shipped
  build.** `xi0 = (x - strcmp(3))*strcmp(7)/rearth` has only default-real
  operands, so in real(4) it is evaluated in single precision and only
  then stored to `real(kind=dp)`. Near the pole image the shipped
  `cxy2ll` therefore resolves direction no better than an all-`f32`
  code would; `cnxyll`, whose `xi`, `eta` arrive in `dp`, agrees with the
  port to 1.3e-6 everywhere in the shipped build.
* **`eqvlat(±90, ±90)` is `NaN`**, not ±90: with `sinl1 == sinl2 == ±1`
  the near-equal branch forms `tau = 0/0`.
* `cgszll` returns `2*strcmp(7)` at `|lat| > 89.985` only for
  `|gamma| > 0.9999`; for other maps it evaluates
  `cos(lat)*exp(gamma*ymerc)` there, and returns 0 when `cos(lat) <= 0`
  (latitudes beyond the pole, or rounding at exactly ±90: with upstream's
  `pi`, `cos(90 deg)` is +1.6e-15 in `f64` but negative in `f32`, so the
  shipped build returns 0 at exactly ±90 where the port and real(8) do not).
* `ccrvxy`'s `temp == 0` branch needs `xpolg` and `ypolg` to be exactly
  zero. For `|gamma| == 1` it is not reached on the `gridcheck` polar maps
  (no `x` within 2e5 ulps of the pole zeroes `xpolg`, searched with this
  port); it is reached on a polar map straight from [`stlmbr`].
* In the `-fdefault-real-8` build, `cnxyll`'s `sngl(...)` returns an
  8-byte real (the port's `f64` longitudes are bit-exact against it).

# Units

Bare `f64` in upstream's units: latitudes and longitudes in degrees, grid
coordinates in grid units, grid sizes in km per grid unit, curvature in
radians per km, wind components in any one consistent unit (upstream's
comment says km/h; the rotation is unit-free).

```rust
pub mod cmapf { /* ... */ }
```

### Types

#### Struct `Strcmp`

Upstream's 9-element map descriptor `strcmp(9)`; element `i` is
`strcmp(i+1)`. See the module documentation for the index mapping.

```rust
pub struct Strcmp(pub [f64; 9]);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `[f64; 9]` |  |

##### Implementations

###### Methods

- ```rust
  pub fn gamma(self: &Self) -> f64 { /* ... */ }
  ```
  `strcmp(1)`: `gamma`, the sine of the tangent latitude.

- ```rust
  pub fn reference_longitude(self: &Self) -> f64 { /* ... */ }
  ```
  `strcmp(2)`: reference longitude, degrees.

- ```rust
  pub fn origin(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  `strcmp(3)`, `strcmp(4)`: grid coordinates of the canonical origin.

- ```rust
  pub fn rotation(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  `strcmp(5)`, `strcmp(6)`: cosine and sine of the grid rotation.

- ```rust
  pub fn equator_grid_size(self: &Self) -> f64 { /* ... */ }
  ```
  `strcmp(7)`: grid size at the equator, km per grid unit.

- ```rust
  pub fn polar_radials(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  `strcmp(8)`, `strcmp(9)`: radial coordinates 1 degree from the north

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
    fn clone(self: &Self) -> Strcmp { /* ... */ }
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
    fn default() -> Strcmp { /* ... */ }
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
    fn eq(self: &Self, other: &Strcmp) -> bool { /* ... */ }
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

#### Function `cspanf`

`cspanf(value, begin, end)`: `value` reduced modulo `end - begin` into the
half-open interval `(min, max]` of the two bounds (either order).
`cspanf(-180, -180, 180)` is therefore `180`.

```rust
pub fn cspanf(value: f64, begin: f64, end: f64) -> f64 { /* ... */ }
```

#### Function `eqvlat`

`eqvlat(xlat1, xlat2)`: the tangent latitude (deg) equivalent to a Lambert
map "true at `xlat1` and `xlat2`". Returns `NaN` for `(±90, ±90)` (see the
module quirks).

```rust
pub fn eqvlat(xlat1: f64, xlat2: f64) -> f64 { /* ... */ }
```

#### Function `stlmbr`

`stlmbr(strcmp, tnglat, xlong)`: a map of tangent latitude `tnglat` (deg;
+90 north polar stereographic, -90 south, 0 Mercator, else Lambert) whose
region is connected for longitudes `xlong ± 180`. The grid placement
(`strcmp(3..7)`) is the canonical one; complete it with [`stcm2p`] or
[`stcm1p`].

```rust
pub fn stlmbr(tnglat: f64, xlong: f64) -> Strcmp { /* ... */ }
```

#### Function `stcm1p`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`stcm1p`: place the grid so that `(x1, y1)` is at `(xlat1, xlong1)`, the
grid size at `(xlatg, xlongg)` is `gridsz` km, and a y grid line there
points `orient` degrees from north.

```rust
pub fn stcm1p(s: &mut Strcmp, x1: f64, y1: f64, xlat1: f64, xlong1: f64, xlatg: f64, xlongg: f64, gridsz: f64, orient: f64) { /* ... */ }
```

#### Function `stcm2p`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`stcm2p`: place the grid so that `(x1, y1)` is at `(xlat1, xlong1)` and
`(x2, y2)` at `(xlat2, xlong2)`.

```rust
pub fn stcm2p(s: &mut Strcmp, x1: f64, y1: f64, xlat1: f64, xlong1: f64, x2: f64, y2: f64, xlat2: f64, xlong2: f64) { /* ... */ }
```

#### Function `cnllxy`

`cnllxy`: geographic `(xlat, xlong)` (deg) to canonical `(xi, eta)`
(equator-centred, radian units). Within `|sin(lat)| >= almst1` of a pole
it returns `(0, 1/gamma)` — `+inf` for a Mercator map.

```rust
pub fn cnllxy(s: &Strcmp, xlat: f64, xlong: f64) -> (f64, f64) { /* ... */ }
```

#### Function `cnxyll`

`cnxyll`: canonical `(xi, eta)` to geographic `(xlat, xlong)` (deg). The
longitude is NOT wrapped (see [`cxy2ll`]). No pole guard (module quirks).

```rust
pub fn cnxyll(s: &Strcmp, xi: f64, eta: f64) -> (f64, f64) { /* ... */ }
```

#### Function `cll2xy`

`cll2xy`: geographic `(xlat, xlong)` (deg) to grid `(x, y)`.

```rust
pub fn cll2xy(s: &Strcmp, xlat: f64, xlong: f64) -> (f64, f64) { /* ... */ }
```

#### Function `cxy2ll`

`cxy2ll`: grid `(x, y)` to geographic `(xlat, xlong)` (deg), longitude
wrapped into (-180, 180].

```rust
pub fn cxy2ll(s: &Strcmp, x: f64, y: f64) -> (f64, f64) { /* ... */ }
```

#### Function `cgszll`

`cgszll`: grid size (km per grid unit) at `(xlat, xlong)`.

```rust
pub fn cgszll(s: &Strcmp, xlat: f64, _xlong: f64) -> f64 { /* ... */ }
```

#### Function `cgszxy`

`cgszxy`: grid size (km per grid unit) at grid `(x, y)`.

```rust
pub fn cgszxy(s: &Strcmp, x: f64, y: f64) -> f64 { /* ... */ }
```

#### Function `cc2gll`

`cc2gll`: geographic wind `(ue, vn)` at `(xlat, xlong)` to grid
components `(ug, vg)`.

```rust
pub fn cc2gll(s: &Strcmp, xlat: f64, xlong: f64, ue: f64, vn: f64) -> (f64, f64) { /* ... */ }
```

#### Function `cg2cll`

`cg2cll`: grid wind `(ug, vg)` at `(xlat, xlong)` to geographic
components `(ue, vn)`.

```rust
pub fn cg2cll(s: &Strcmp, xlat: f64, xlong: f64, ug: f64, vg: f64) -> (f64, f64) { /* ... */ }
```

#### Function `cg2cxy`

`cg2cxy`: grid wind `(ug, vg)` at grid `(x, y)` to geographic components
`(ue, vn)`. The polar orientation switches at `strcmp(8)`/`strcmp(9)`
(±89 deg), not at ±89.985 as in [`cg2cll`].

```rust
pub fn cg2cxy(s: &Strcmp, x: f64, y: f64, ug: f64, vg: f64) -> (f64, f64) { /* ... */ }
```

#### Function `ccrvll`

`ccrvll`: map-curvature vector `(gx, gy)` (rad/km) at `(xlat, xlong)`.

```rust
pub fn ccrvll(s: &Strcmp, xlat: f64, xlong: f64) -> (f64, f64) { /* ... */ }
```

#### Function `ccrvxy`

`ccrvxy`: map-curvature vector `(gx, gy)` (rad/km) at grid `(x, y)`.

```rust
pub fn ccrvxy(s: &Strcmp, x: f64, y: f64) -> (f64, f64) { /* ... */ }
```

#### Function `cpolll`

`cpolll`: direction cosines `(enx, eny, enz)` of the Earth's rotation axis
in map coordinates at `(xlat, xlong)`.

```rust
pub fn cpolll(s: &Strcmp, xlat: f64, xlong: f64) -> (f64, f64, f64) { /* ... */ }
```

#### Function `cpolxy`

`cpolxy`: direction cosines `(enx, eny, enz)` of the Earth's rotation axis
in map coordinates at grid `(x, y)`.

```rust
pub fn cpolxy(s: &Strcmp, x: f64, y: f64) -> (f64, f64, f64) { /* ... */ }
```

## Module `concentration`

Gridding of particle mass onto the output grids: concentrations
(`conccalc.f90`) and dry deposition (`drydepokernel.f90`,
`drydepokernel_nest.f90`).

# The two kernels

* **Uniform kernel** on the output grids. A particle's mass is shared
  between the cell it is in and the three neighbours towards its nearest
  corner, with bilinear "tent" weights `wx = 1.5 - ddx` or `0.5 + ddx`
  (and the same in y) — a box of one cell width centred on the particle.
  `conccalc` falls back to **direct attribution** (whole mass to one cell)
  for particles younger than 10 800 s, for particles within half a cell
  of the grid edge, and when the kernel is switched off
  (`lusekerneloutput = .false.`).
* **Parabolic (Epanechnikov) kernel** at receptor points:
  `K = 0.596831 (1 - r²)` on the ellipsoid
  `r² = (Δx/hx)² + (Δy/hy)² + (z/hz)²`, with age-dependent bandwidths
  `hz = min(50 + 0.3 sqrt(age), 150) m`,
  `hx = min((0.29 + 2.222e-3 sqrt(age)) dx + 1.2e-5 age, 6)`,
  `hy = min((0.18 + 1.389e-3 sqrt(age)) dy + 7.5e-6 age, 4)` grid units.
  `0.596831 = 15/(8π)` normalises the kernel over a half ellipsoid; the
  final `2 * weight * c / receptorarea` accounts for the ground.

# One function per domain, not one per upstream copy

The nested-output block of `conccalc` repeats the mother-grid block on the
nest arrays; [`conccalc`] runs one Rust routine on each domain. The two
copies are **not** textually identical: in the backward-deposition
(`DRYBKDEP`/`WETBKDEP`) kernel branch the mother grid multiplies
`xmass/rhoi*w*weight` at the `(ix,jy)` and `(ixp,jyp)` corners but
`xmass/rhoi*weight*w` at the other two, while the nest uses `*weight*w`
at all four. With upstream's weights (0.5 or 1.0, exact powers of two)
the order is invisible; for any other weight it changes the last bit. The
port reproduces both orders ([`ProductOrder`]).

`drydepokernel_nest` is `drydepokernel` on the nest grid **without the
`lusekerneloutput` switch**: the nest always uses the kernel. The port is
one function, [`drydepokernel`], with an [`Attribution`] argument;
[`drydepokernel_nest`] calls it with [`Attribution::UniformKernel`].

# Upstream quirks reproduced

* `conccalc` computes the cell as `ix = int(xl); if (xl < 0) ix = ix - 1`,
  a floor that is wrong at negative integers (`xl = -1.0` gives `-2`); it
  only affects cells off the grid. `drydepokernel` has **no** correction:
  `int()` truncates toward zero, so `-1 < xl < 0` lands in column 0 with
  `ddx < 0` and a kernel weight `wx = 0.5 + ddx < 0.5`.
* `drydepokernel` uses the kernel right up to the grid edge (no half-cell
  fallback, no age threshold), so mass whose kernel corners fall off the
  grid is lost; `conccalc` instead attributes such particles directly.
* The kernel branch takes the `else` side when `ddx` is exactly 0.5
  (`ixp = ix - 1`, `wx = 1`): all of the x weight stays in the cell.
* With `ind_samp = -1` (mass mixing ratio) the density at the particle is
  interpolated with the first corner from slot `memind(2)` and the other
  three from **slot 2 literally** (`rho(ix,jy,ind,memind(2)) +
  p2*rho(ixp,jy,ind,2) + ...`); after the fields swap (`memind(2) = 1`)
  the four corners come from different times. Reproduced.
* The density branch's north-pole fix compares `jyp` with the
  compile-time `nymax`, not `ny` (see [`GriddedMet::ny`]), and has no
  check on `ixp`.
* The age class loop `do nage=1,nageclass; if (itage < lage(nage)) exit`
  leaves `nage = nageclass + 1` for a particle older than the last class
  boundary, and upstream then writes **past the end** of `gridunc`.
  `timemanager` terminates such particles first, so in a normal run it
  does not happen; the port refuses ([`ConcCalcError::AgeBeyondLastClass`]).
* The density branch's level search leaves `indz` unassigned (stale) for
  `ztra1 >= height(nz)`; it runs for **every** particle at `itime`, even
  one above the output grid. The port refuses
  ([`ConcCalcError::AboveTopLevel`]).
* Any `ind_samp` other than 0 / -1 would leave `rhoi` stale; it cannot be
  expressed through [`SamplingUnit`].
* **Upstream defect: particle-count output with the kernel on mixes
  units.** `lparticlecountoutput` is tested only in the direct-attribution
  branch; the kernel branch adds `xmass/rhoi*weight*w` regardless. With
  `lparticlecountoutput = .true.` and `lusekerneloutput = .true.`, young
  particles and particles near the edge add 1 per species, older ones add
  mass. Observed in the code-to-code fixture (variant V3: 34 count cells
  and 88 mass cells in one grid) and reproduced by the port.
* The receptor estimate ignores the particle's age class and release
  point, uses `zd = ztra1/hz` (one-sided: no `zd < -1` test), and only
  counts particles with `r2 < 1` strictly.
* **Storage precision.** Upstream accumulates `gridunc`/`creceptor` in
  default `real` (single precision as shipped) and the deposition grids in
  `real(dep_prec)`, where `dep_prec = sp` is `real(4)` **even in a
  `-fdefault-real-8` build**. The port accumulates in `f64`; a caller
  wanting upstream's storage rounds to `f32` itself.

# Index conventions

Output-grid columns/rows `ix`, `jy` are 0-based, as upstream's
`0:numxgrid-1`; output level, species, release point, uncertainty class
and age class are **0-based here** (upstream's are 1-based).

# Units

Positions in meteorological grid units (`xtra1`, `ytra1`), heights m,
times s; `dx`, `dy`, `dxout`, `dyout`, `xoutshift`, `youtshift` in
degrees; masses kg; receptor areas m². `gridunc` accumulates
`mass/rhoi * weight` (kg, or kg/(kg/m³) for a mixing ratio).

```rust
pub mod concentration { /* ... */ }
```

### Types

#### Struct `OutputDomain`

Geometry of one output grid (`numxgrid`, `numygrid`, `dxout`, `dyout`,
`xoutshift`, `youtshift`, or their nest counterparts with suffix `n`).

```rust
pub struct OutputDomain {
    pub numxgrid: usize,
    pub numygrid: usize,
    pub dxout: f64,
    pub dyout: f64,
    pub xoutshift: f64,
    pub youtshift: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `numxgrid` | `usize` | Number of output columns. |
| `numygrid` | `usize` | Number of output rows. |
| `dxout` | `f64` | Output cell width, degrees. |
| `dyout` | `f64` | Output cell height, degrees. |
| `xoutshift` | `f64` | `xlon0 - outlon0`, degrees. |
| `youtshift` | `f64` | `ylat0 - outlat0`, degrees. |

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
    fn clone(self: &Self) -> OutputDomain { /* ... */ }
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
    fn eq(self: &Self, other: &OutputDomain) -> bool { /* ... */ }
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
#### Struct `ConcentrationGrid`

A 7-D concentration accumulator, `gridunc(0:numxgrid-1, 0:numygrid-1,
numzgrid, nspec, npointspec, nclassunc, nageclass)` (or `griduncn`).

```rust
pub struct ConcentrationGrid {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub nspec: usize,
    pub npointspec: usize,
    pub nclassunc: usize,
    pub nageclass: usize,
    pub values: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Columns. |
| `ny` | `usize` | Rows. |
| `nz` | `usize` | Output levels. |
| `nspec` | `usize` | Species. |
| `npointspec` | `usize` | Release points with separate output (`maxpointspec_act`). |
| `nclassunc` | `usize` | Uncertainty classes (`nclassunc`). |
| `nageclass` | `usize` | Age classes. |
| `values` | `Vec<f64>` | Values, `ix` fastest, then `jy`, `kz`, `ks`, release, class, age. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(nx: usize, ny: usize, nz: usize, nspec: usize, npointspec: usize, nclassunc: usize, nageclass: usize) -> Self { /* ... */ }
  ```
  All-zero grid.

- ```rust
  pub fn index(self: &Self, ix: usize, jy: usize, kz: usize, ks: usize, kp: usize, nc: usize, na: usize) -> usize { /* ... */ }
  ```
  Flat index; all indices 0-based.

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
    fn clone(self: &Self) -> ConcentrationGrid { /* ... */ }
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
    fn eq(self: &Self, other: &ConcentrationGrid) -> bool { /* ... */ }
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
#### Struct `DepositionGrid`

A 6-D deposition accumulator, `drygridunc(0:numxgrid-1, 0:numygrid-1,
nspec, npointspec, nclassunc, nageclass)` (or `drygriduncn`).

```rust
pub struct DepositionGrid {
    pub nx: usize,
    pub ny: usize,
    pub nspec: usize,
    pub npointspec: usize,
    pub nclassunc: usize,
    pub nageclass: usize,
    pub values: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Columns. |
| `ny` | `usize` | Rows. |
| `nspec` | `usize` | Species. |
| `npointspec` | `usize` | Release points with separate output. |
| `nclassunc` | `usize` | Uncertainty classes. |
| `nageclass` | `usize` | Age classes. |
| `values` | `Vec<f64>` | Values, `ix` fastest, then `jy`, `ks`, release, class, age. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(nx: usize, ny: usize, nspec: usize, npointspec: usize, nclassunc: usize, nageclass: usize) -> Self { /* ... */ }
  ```
  All-zero grid.

- ```rust
  pub fn index(self: &Self, ix: usize, jy: usize, ks: usize, kp: usize, nc: usize, na: usize) -> usize { /* ... */ }
  ```
  Flat index; all indices 0-based.

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
    fn clone(self: &Self) -> DepositionGrid { /* ... */ }
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
    fn eq(self: &Self, other: &DepositionGrid) -> bool { /* ... */ }
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
#### Enum `SamplingUnit`

`ind_samp`: what a sampled particle contributes.

```rust
pub enum SamplingUnit {
    Mass,
    MassMixingRatio,
}
```

##### Variants

###### `Mass`

`ind_samp = 0`: mass (`rhoi = 1`).

###### `MassMixingRatio`

`ind_samp = -1`: mass divided by the air density at the particle.

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
    fn clone(self: &Self) -> SamplingUnit { /* ... */ }
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
    fn eq(self: &Self, other: &SamplingUnit) -> bool { /* ... */ }
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
#### Struct `Particles`

The particle arrays `conccalc` reads from `com_mod`.

```rust
pub struct Particles {
    pub itra1: Vec<i64>,
    pub itramem: Vec<i64>,
    pub xtra1: Vec<f64>,
    pub ytra1: Vec<f64>,
    pub ztra1: Vec<f64>,
    pub npoint: Vec<usize>,
    pub nclass: Vec<usize>,
    pub nspec: usize,
    pub xmass1: Vec<f64>,
    pub xscav_frac1: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `itra1` | `Vec<i64>` | Time each particle is at, s (`itra1`). |
| `itramem` | `Vec<i64>` | Release time, s (`itramem`). |
| `xtra1` | `Vec<f64>` | x position, grid units (`xtra1`). |
| `ytra1` | `Vec<f64>` | y position, grid units (`ytra1`). |
| `ztra1` | `Vec<f64>` | Height above ground, m (`ztra1`). |
| `npoint` | `Vec<usize>` | Release point, 0-based (`npoint - 1`). |
| `nclass` | `Vec<usize>` | Uncertainty class, 0-based (`nclass - 1`). |
| `nspec` | `usize` | Number of species stored per particle in `xmass1`/`xscav_frac1`. |
| `xmass1` | `Vec<f64>` | Mass per species, kg, `[i*nspec + ks]` (`xmass1`). |
| `xscav_frac1` | `Vec<f64>` | Scavenged fraction per species, `[i*nspec + ks]` (`xscav_frac1`). |

##### Implementations

###### Methods

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of particles (`numpart`).

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` when there are no particles.

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
    fn clone(self: &Self) -> Particles { /* ... */ }
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
    fn default() -> Particles { /* ... */ }
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
    fn eq(self: &Self, other: &Particles) -> bool { /* ... */ }
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
#### Struct `ConcCalcSettings`

The `com_mod` / `par_mod` switches and tables `conccalc` reads.

```rust
pub struct ConcCalcSettings {
    pub sampling: SamplingUnit,
    pub output_for_each_release: bool,
    pub domain_filling: bool,
    pub use_kernel: bool,
    pub backward_deposition: bool,
    pub particle_count_output: bool,
    pub lage: Vec<i64>,
    pub outheight: Vec<f64>,
    pub dx: f64,
    pub dy: f64,
    pub nspec: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sampling` | `SamplingUnit` | `ind_samp`. |
| `output_for_each_release` | `bool` | `ioutputforeachrelease = 1`. |
| `domain_filling` | `bool` | `mdomainfill = 1` (then `npoint` is a particle number and all mass<br>goes to release slot 0). |
| `use_kernel` | `bool` | `lusekerneloutput` (a `par_mod` parameter upstream). |
| `backward_deposition` | `bool` | `DRYBKDEP .or. WETBKDEP`: weight mass by `max(xscav_frac1, 0)`. |
| `particle_count_output` | `bool` | `lparticlecountoutput` (a `par_mod` parameter): count particles<br>instead of mass (direct attribution only; the kernel still grids mass). |
| `lage` | `Vec<i64>` | Upper age of each age class, s (`lage(1:nageclass)`). |
| `outheight` | `Vec<f64>` | Upper height of each output level, m (`outheight`). |
| `dx` | `f64` | Meteorological grid spacing in x, degrees (`dx`). |
| `dy` | `f64` | Meteorological grid spacing in y, degrees (`dy`). |
| `nspec` | `usize` | Species to grid (`nspec`). |

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
    fn clone(self: &Self) -> ConcCalcSettings { /* ... */ }
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
    fn eq(self: &Self, other: &ConcCalcSettings) -> bool { /* ... */ }
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
#### Struct `Receptors`

Receptor points (`xreceptor`, `yreceptor` in grid units, `receptorarea`
in m²).

```rust
pub struct Receptors {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub area: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `Vec<f64>` | x, grid units. |
| `y` | `Vec<f64>` | y, grid units. |
| `area` | `Vec<f64>` | Area of a 1 x 1 grid cell at the receptor, m². |

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
    fn clone(self: &Self) -> Receptors { /* ... */ }
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
    fn default() -> Receptors { /* ... */ }
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
    fn eq(self: &Self, other: &Receptors) -> bool { /* ... */ }
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
#### Enum `ConcCalcError`

Why [`conccalc`] refused. The index is the particle's.

```rust
pub enum ConcCalcError {
    AgeBeyondLastClass(usize),
    AboveTopLevel(usize),
    DensityOutsideGrid(usize),
    MissingDensity,
    IndexOutsideGrid(usize),
}
```

##### Variants

###### `AgeBeyondLastClass`

Age at or beyond `lage(nageclass)`: upstream writes past `gridunc`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `AboveTopLevel`

`ztra1 >= height(nz)` with `ind_samp = -1`: upstream's level index is
stale.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `DensityOutsideGrid`

The density corners fall outside the supplied arrays.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `MissingDensity`

`ind_samp = -1` but no density field supplied.

###### `IndexOutsideGrid`

Release point or uncertainty class outside the concentration grid.

Fields:

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
    fn clone(self: &Self) -> ConcCalcError { /* ... */ }
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
    fn eq(self: &Self, other: &ConcCalcError) -> bool { /* ... */ }
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
#### Enum `ProductOrder`

Multiplication order of the backward-deposition kernel products (see the
module doc).

```rust
pub enum ProductOrder {
    Mother,
    Nest,
}
```

##### Variants

###### `Mother`

Mother grid: `*w*weight` at `(ix,jy)` and `(ixp,jyp)`, `*weight*w` at
the other corners.

###### `Nest`

Nest: `*weight*w` at every corner.

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
    fn clone(self: &Self) -> ProductOrder { /* ... */ }
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
    fn eq(self: &Self, other: &ProductOrder) -> bool { /* ... */ }
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
#### Enum `Attribution`

How `drydepokernel` attributes a deposit.

```rust
pub enum Attribution {
    DirectCell,
    UniformKernel,
}
```

##### Variants

###### `DirectCell`

`lusekerneloutput = .false.`: the whole deposit to the particle's cell.

###### `UniformKernel`

The uniform kernel (four cells).

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
    fn clone(self: &Self) -> Attribution { /* ... */ }
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
    fn eq(self: &Self, other: &Attribution) -> bool { /* ... */ }
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
#### Enum `DepositionError`

Why a deposition call refused.

```rust
pub enum DepositionError {
    IndexOutsideGrid,
}
```

##### Variants

###### `IndexOutsideGrid`

`nunc`, `nage`, `kp` or the species count outside the grid.

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
    fn clone(self: &Self) -> DepositionError { /* ... */ }
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
    fn eq(self: &Self, other: &DepositionError) -> bool { /* ... */ }
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

#### Function `conccalc`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`conccalc.f90`: add the particles at time `itime` to the concentration
grids (mother, and nest when given) with sampling weight `weight`, and add
the receptor concentrations to `creceptor` (`[n*nspec + ks]`).

`density` is required for [`SamplingUnit::MassMixingRatio`] (its `rho`,
`height`, `nz`, `memind` and `ny` — read as upstream's `nymax` — are
used). On error nothing has been modified.

```rust
pub fn conccalc(itime: i64, weight: f64, particles: &Particles, settings: &ConcCalcSettings, density: Option<&crate::flexpart::particle_average::GriddedMet>, mother: &OutputDomain, gridunc: &mut ConcentrationGrid, nest: Option<(&OutputDomain, &mut ConcentrationGrid)>, receptors: &Receptors, creceptor: &mut [f64]) -> Result<(), ConcCalcError> { /* ... */ }
```

#### Function `drydepokernel`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`drydepokernel.f90`: add one particle's dry deposit (`deposit[ks]`, kg)
at grid position `(x, y)` (meteorological grid units) to `grid`, for the
species with `drydepspec[ks]` and a non-zero deposit. `nunc`, `nage`, `kp`
are the 0-based uncertainty class, age class and release slot.

The cell is `int(xl)` (truncation toward zero, see the module doc). With
[`Attribution::UniformKernel`] the four corners are tested and filled in
upstream's order `(ix,jy)`, `(ixp,jyp)`, `(ixp,jy)`, `(ix,jyp)`.

```rust
pub fn drydepokernel(domain: &OutputDomain, dx: f64, dy: f64, attribution: Attribution, deposit: &[f64], drydepspec: &[bool], x: f64, y: f64, nunc: usize, nage: usize, kp: usize, grid: &mut DepositionGrid) -> Result<(), DepositionError> { /* ... */ }
```

#### Function `drydepokernel_nest`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`drydepokernel_nest.f90`: [`drydepokernel`] on a nested output grid. The
nest version has no `lusekerneloutput` switch, so it **always** uses the
uniform kernel, even when the mother grid attributes directly.

```rust
pub fn drydepokernel_nest(nest: &OutputDomain, dx: f64, dy: f64, deposit: &[f64], drydepspec: &[bool], x: f64, y: f64, nunc: usize, nage: usize, kp: usize, grid: &mut DepositionGrid) -> Result<(), DepositionError> { /* ... */ }
```

## Module `concoutput`

Conversion of the accumulated particle mass on the output grids into
FLEXPART's gridded output: concentrations (or, backward, residence
times), mixing ratios, wet and dry deposition, the uncertainty statistics
over the `nclassunc` particle classes and the **sparse packing** in which
FLEXPART writes them (`concoutput.f90`, `concoutput_nest.f90`,
`concoutput_surf.f90`).

The numerics are ported; the file I/O is not. [`concoutput`] returns, in
upstream's record order, every record the routine writes (see
[`ConcOutput`]); a caller wanting FLEXPART's binary files writes them
from that.

# What upstream computes

* **Air density at each output cell** (`densityoutgrid`), from the
  coarse met grid: the met column nearest the output cell centre
  (`nint` of the cell's position in met grid units, clamped to the grid),
  interpolated linearly in height to the middle of the output layer,
  `halfheight = (outheight(kz) + outheight(kz-1))/2`. The same for the
  dry-air density, and `factor_drygrid = densityoutgrid/densitydrygrid`.
* **The unit factor** `factor3d = 1e12/volume/outnum` forward (kg → ng
  per m³, averaged over the `outnum` samples), `|loutaver|/outnum`
  backward (residence time, s).
* **The uncertainty statistics.** For every cell, the `nclassunc`
  per-class values go through `mean_mod`'s one-pass mean and sample
  standard deviation; the mean times `nclassunc` is the cell's total, the
  standard deviation times `sqrt(nclassunc)` its uncertainty. The three
  numbers `concoutput` returns are `sum(sigma)/sum(total)` over all
  species, release slots, age classes and cells.
* **The sparse packing.** A field is written as four records: the number
  of runs, the flat index of the first cell of every run of consecutive
  non-zero (`> tiny(0.0)`) cells, the number of values, and the values,
  whose **sign alternates from run to run** (`+` for the first run) — the
  run boundaries are recoverable from the sign changes. Runs continue
  across row and level boundaries: the scan is one linear sweep, `ix`
  fastest. Deposition fields are `1e12*value/area`, concentrations
  `value*factor3d/tot_mu` (or the particle count), mixing ratios
  `1e12*value/volume/outnum*28.97/weightmolar/density`; `factor_drygrid`
  is packed with "`!= 1`" in place of "non-zero".

# The three routines ([`Routine`])

| | `concoutput` | `concoutput_nest` | `concoutput_surf` |
|---|---|---|---|
| grids | `gridunc`, `area`, `volume` | `griduncn`, `arean`, `volumen` | as `concoutput` |
| density time slot | `memind(2)` | `memind(2)` | **literally 2** |
| levels written | all | all | **`kz = 1` only** (means and totals still over all levels) |
| uncertainty totals | yes | no | yes |
| receptor records | yes | no | yes |

# Upstream quirks reproduced (and documented, not fixed)

* **Level search.** The density level is found by
  `height(kzz-1) < halfheight < height(kzz)`, both strict. If no level
  satisfies it — `halfheight` above the top level, below `height(1)`, or
  **exactly equal to a model level** — the loop runs out and `kzz` is
  clamped to `nz`, so the density is *extrapolated* from the top two
  levels. The code-to-code sweep includes an output layer whose
  half-height is exactly `height(2) = 50 m`: upstream (and the port)
  then return `(rho(6)*(-950) + rho(5)*2450)/1500` instead of `rho(2)`.
* **`concoutput_surf` reads time slot 2, not `memind(2)`**, for every
  density (`rho(...,2)`; the other two use `mind = memind(2)`, with the
  comment "added to ensure identical results between 2&3-fields
  versions"). After the fields swap, the surface output uses the other
  time's density.
* **`mean_mod`'s `eps = 1e-30` is absolute.** The sample variance is set
  to 0 when `sum(x^2) - sum(x)^2/n < 1e-30`. Grid values are masses (kg)
  divided by the density; for masses below about `1e-15` the uncertainty
  is therefore always reported as exactly 0, whatever the spread. The
  sweep's case 3 (masses ~1e-17 kg, classes spread by a factor 50) shows
  it: every `gridsigma` is 0.
* **The uncertainty totals mix species, release points and age classes**,
  and `gridtotal`/`gridsigmatotal`/`gridtotalunc` are declared `real(sp)`
  — single precision even in a double-precision build (reproduced: the
  port accumulates them in `f32`). Backward runs and runs without
  deposition return 0.
* **The concentration index carries `kz` 1-based**: the first level's
  cells are numbered from `numxgrid*numygrid`, not 0.
* **`concoutput_nest` writes no receptor records** (it computes the
  receptor densities and discards them) and **zeroes `creceptor`** as
  well as `griduncn`; `concoutput` zeroes `creceptor` and `gridunc`. The
  port leaves resetting the accumulators to the caller.
* **Only `concoutput` honours `lparticlecountoutput`.** `concoutput_nest`
  and `concoutput_surf` have no particle-count branch and always write
  `grid*factor3d/tot_mu`; with particle-count output on, their files
  hold particle counts times `1e12/volume/outnum` under a concentration
  file name. Observed in the fixture (variant V2) and reproduced.
* **Receptor concentrations are written whenever `numreceptor > 0`**,
  also for `iout = 2`, when `openreceptors.f90` has not opened the unit
  (gfortran then writes `fort.91`). Not reproduced (file I/O); the record
  is returned.
* **Mixing ratios ignore `ldirect`**: in a backward run with `iout = 2/3`
  they are still `1e12*grid/volume/outnum*...` — a residence time
  divided by a volume and an air density. Reproduced.
* **`wetgrid`/`drygrid` are only recomputed for forward runs with the
  switch on**; otherwise their (stale) contents are never written, since
  the sparse loops test the same switches. Reproduced by returning
  `None`.
* **Upstream does not compile in double precision as shipped.** With
  `-fdefault-real-8` and `dep_prec = sp`, `call mean(auxgrid, grid(...),
  gridsigma(...), nclassunc)` has a `real(4)` sample and `real(8)`
  results, and `mean_mod` has no matching specific (gfortran: "There is no
  specific subroutine for the generic 'mean'", `concoutput.f90:322`,
  `concoutput_nest.f90:272`, `concoutput_surf.f90:294`). The real(8)
  reference therefore sets `dep_prec = dp`, the alternative `par_mod.f90`
  documents.

# Precision model

The port reproduces upstream compiled with `-fdefault-real-8` and
`dep_prec = dp`: default `real` and `real(dep_prec)` are `f64`; the
quantities upstream declares **`real(sp)` explicitly** stay `f32` in
every build and are rounded here as there — `wetgrid`, `drygrid` (whose
mean comes from `mean_mixed_dsd`, `xm = real(xl,sp)/real(n,sp)`), and
`gridtotal`, `gridsigmatotal`, `gridtotalunc`. Against the shipped
single-precision build the residual is upstream's `real(4)` arithmetic.

# Units and indices

Grid inputs are what `conccalc`/`wetdepokernel`/`drydepokernel`
accumulate (kg, or kg/(kg m⁻³)). Areas m², volumes m³, heights m,
densities kg/m³, `outnum` dimensionless, `loutaver` s, molar weights
g/mol. Concentration output ng/m³ (forward) or s (backward), deposition
ng/m², mixing ratio pptv. Output and met cell indices are 0-based as
upstream's `0:numxgrid-1`; levels, species, release slots, classes and age
classes are 0-based here (1-based upstream) — except the packed flat
index, which is upstream's number.

```rust
pub mod concoutput { /* ... */ }
```

### Types

#### Enum `Routine`

Which upstream routine to reproduce. See the module table.

```rust
pub enum Routine {
    Concoutput,
    Nest,
    Surface,
}
```

##### Variants

###### `Concoutput`

`concoutput.f90`: mother output grid, all levels.

###### `Nest`

`concoutput_nest.f90`: nested output grid, no totals, no receptors.

###### `Surface`

`concoutput_surf.f90`: mother grid, first level written only,
density from time slot 2.

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
    fn clone(self: &Self) -> Routine { /* ... */ }
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
    fn eq(self: &Self, other: &Routine) -> bool { /* ... */ }
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
#### Struct `DensityFields`

The meteorological fields `concoutput` reads: the coarse grid's density
and dry-air density columns (`rho`, `rho_dry`) for every time slot in
memory, and the level heights.

```rust
pub struct DensityFields {
    pub nxmin1: usize,
    pub nymin1: usize,
    pub nz: usize,
    pub xlon0: f64,
    pub ylat0: f64,
    pub dx: f64,
    pub dy: f64,
    pub height: Vec<f64>,
    pub rho: Vec<f64>,
    pub rho_dry: Vec<f64>,
    pub nslots: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nxmin1` | `usize` | `nxmin1` (the met grid has `nxmin1 + 1` columns). |
| `nymin1` | `usize` | `nymin1`. |
| `nz` | `usize` | Model levels `nz`. |
| `xlon0` | `f64` | Met grid origin, degrees (`xlon0`). |
| `ylat0` | `f64` | Met grid origin, degrees (`ylat0`). |
| `dx` | `f64` | Met grid spacing, degrees (`dx`). |
| `dy` | `f64` | Met grid spacing, degrees (`dy`). |
| `height` | `Vec<f64>` | Level heights, m (`height(1:nz)`). |
| `rho` | `Vec<f64>` | Air density, kg/m³: `ix` fastest, then `jy`, level, time slot. |
| `rho_dry` | `Vec<f64>` | Dry-air density, kg/m³, laid out as [`DensityFields::rho`]. |
| `nslots` | `usize` | Time slots held (`numwfmem`). |

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
    fn clone(self: &Self) -> DensityFields { /* ... */ }
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
    fn eq(self: &Self, other: &DensityFields) -> bool { /* ... */ }
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
#### Struct `OutputGrid`

One output grid (`numxgrid`, `outlon0`, `dxout`, `area`, `volume`, or
the nest's `numxgridn`, `outlon0n`, ..., `arean`, `volumen`).

```rust
pub struct OutputGrid {
    pub numxgrid: usize,
    pub numygrid: usize,
    pub outlon0: f64,
    pub outlat0: f64,
    pub dxout: f64,
    pub dyout: f64,
    pub area: Vec<f64>,
    pub volume: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `numxgrid` | `usize` | Columns. |
| `numygrid` | `usize` | Rows. |
| `outlon0` | `f64` | Lower-left cell longitude, degrees. |
| `outlat0` | `f64` | Lower-left cell latitude, degrees. |
| `dxout` | `f64` | Cell width, degrees. |
| `dyout` | `f64` | Cell height, degrees. |
| `area` | `Vec<f64>` | Cell areas, m², `ix` fastest. |
| `volume` | `Vec<f64>` | Cell volumes, m³, `ix` fastest, then `jy`, level. |

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
    fn clone(self: &Self) -> OutputGrid { /* ... */ }
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
    fn eq(self: &Self, other: &OutputGrid) -> bool { /* ... */ }
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
#### Struct `ConcOutputSettings`

Run settings `concoutput` reads from `com_mod` / `par_mod`.

```rust
pub struct ConcOutputSettings {
    pub routine: Routine,
    pub ldirect: i64,
    pub iout: i32,
    pub wetdep: bool,
    pub drydep: bool,
    pub outnum: f64,
    pub loutaver: i64,
    pub memind2: usize,
    pub particle_count_output: bool,
    pub outheight: Vec<f64>,
    pub weightmolar: Vec<f64>,
    pub xmass: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `routine` | `Routine` | Which routine. |
| `ldirect` | `i64` | `+1` forward, `-1` backward (`ldirect`). |
| `iout` | `i32` | Output selector `iout`: concentrations for 1, 3, 5; mixing ratios<br>for 2, 3. |
| `wetdep` | `bool` | Wet deposition switched on (`WETDEP`). |
| `drydep` | `bool` | Dry deposition switched on (`DRYDEP`). |
| `outnum` | `f64` | Number of samples in the averaging interval (`outnum`). |
| `loutaver` | `i64` | Averaging time, s (`loutaver`, negative in backward runs). |
| `memind2` | `usize` | `memind(2)`, 1-based time slot (ignored by [`Routine::Surface`]). |
| `particle_count_output` | `bool` | `lparticlecountoutput` (a `par_mod` parameter): write grid values<br>unconverted. Honoured by [`Routine::Concoutput`] only, as upstream. |
| `outheight` | `Vec<f64>` | Output level tops, m (`outheight(1:numzgrid)`). |
| `weightmolar` | `Vec<f64>` | Molar weight per species, g/mol (`weightmolar`). |
| `xmass` | `Vec<f64>` | Released mass per release slot and species, kg, `xmass(kp, ks)` laid<br>out `kp` fastest (backward runs divide by it). |

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
    fn clone(self: &Self) -> ConcOutputSettings { /* ... */ }
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
    fn eq(self: &Self, other: &ConcOutputSettings) -> bool { /* ... */ }
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
#### Struct `ReceptorInput`

Receptor points (`xreceptor`, `yreceptor` in met grid units) and their
accumulated concentrations `creceptor(i, ks)`, `i` fastest.

```rust
pub struct ReceptorInput {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub creceptor: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `Vec<f64>` | x, met grid units. |
| `y` | `Vec<f64>` | y, met grid units. |
| `creceptor` | `Vec<f64>` | `creceptor(i, ks)`, receptor fastest. |

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
    fn clone(self: &Self) -> ReceptorInput { /* ... */ }
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
    fn default() -> ReceptorInput { /* ... */ }
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
    fn eq(self: &Self, other: &ReceptorInput) -> bool { /* ... */ }
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
#### Struct `SparseField`

One sparse-packed field: the four records upstream writes
(`sp_count_i`, `sparse_dump_i(1:sp_count_i)`, `sp_count_r`,
`sparse_dump_r(1:sp_count_r)`); the counts are the lengths.

```rust
pub struct SparseField {
    pub indices: Vec<i64>,
    pub values: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `indices` | `Vec<i64>` | Flat index of the first cell of every run. |
| `values` | `Vec<f64>` | Values, the sign alternating from run to run. |

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
    fn clone(self: &Self) -> SparseField { /* ... */ }
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
    fn default() -> SparseField { /* ... */ }
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
    fn eq(self: &Self, other: &SparseField) -> bool { /* ... */ }
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
#### Struct `SliceRecords`

The three packed fields of one file for one (species, release slot, age
class): wet deposition, dry deposition, then the 3-D field. Wet and dry
are empty unless the run is forward with the switch on.

```rust
pub struct SliceRecords {
    pub wet: SparseField,
    pub dry: SparseField,
    pub field: SparseField,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `wet` | `SparseField` | Wet deposition, ng/m². |
| `dry` | `SparseField` | Dry deposition, ng/m². |
| `field` | `SparseField` | Concentration (ng/m³; backward: s; or particle counts), or mixing<br>ratio (pptv). |

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
    fn clone(self: &Self) -> SliceRecords { /* ... */ }
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
    fn default() -> SliceRecords { /* ... */ }
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
    fn eq(self: &Self, other: &SliceRecords) -> bool { /* ... */ }
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
#### Struct `SliceStats`

The `nclassunc` statistics of one slice (upstream's `grid`, `gridsigma`,
`wetgrid`, ... work arrays after the slice).

```rust
pub struct SliceStats {
    pub grid: Vec<f64>,
    pub gridsigma: Vec<f64>,
    pub wetgrid: Option<Vec<f32>>,
    pub wetgridsigma: Option<Vec<f64>>,
    pub drygrid: Option<Vec<f32>>,
    pub drygridsigma: Option<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `Vec<f64>` | Total over classes, `ix` fastest, then `jy`, level. |
| `gridsigma` | `Vec<f64>` | Its uncertainty (`sigma * sqrt(nclassunc)`). |
| `wetgrid` | `Option<Vec<f32>>` | Wet deposition total, `real(sp)` upstream; `None` unless forward<br>with `WETDEP`. |
| `wetgridsigma` | `Option<Vec<f64>>` | Its uncertainty. |
| `drygrid` | `Option<Vec<f32>>` | Dry deposition total, `real(sp)` upstream. |
| `drygridsigma` | `Option<Vec<f64>>` | Its uncertainty. |

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
    fn clone(self: &Self) -> SliceStats { /* ... */ }
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
    fn default() -> SliceStats { /* ... */ }
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
    fn eq(self: &Self, other: &SliceStats) -> bool { /* ... */ }
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
#### Struct `SliceOutput`

Everything concerning one (species, release slot, age class).

```rust
pub struct SliceOutput {
    pub ks: usize,
    pub kp: usize,
    pub nage: usize,
    pub conc: Option<SliceRecords>,
    pub pptv: Option<SliceRecords>,
    pub stats: SliceStats,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ks` | `usize` | Species, 0-based. |
| `kp` | `usize` | Release slot, 0-based. |
| `nage` | `usize` | Age class, 0-based. |
| `conc` | `Option<SliceRecords>` | Records of the concentration file (`grid_conc_*` / `grid_time_*`),<br>present for `iout` 1, 3, 5. |
| `pptv` | `Option<SliceRecords>` | Records of the mixing-ratio file (`grid_pptv_*`), present for `iout`<br>2, 3. |
| `stats` | `SliceStats` | The class statistics. |

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
    fn clone(self: &Self) -> SliceOutput { /* ... */ }
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
    fn eq(self: &Self, other: &SliceOutput) -> bool { /* ... */ }
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
#### Struct `Uncertainty`

`gridtotalunc`, `wetgridtotalunc`, `drygridtotalunc`.

```rust
pub struct Uncertainty {
    pub gridtotalunc: f32,
    pub wetgridtotalunc: f64,
    pub drygridtotalunc: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gridtotalunc` | `f32` | `sum(gridsigma)/sum(grid)`, `real(sp)` upstream. |
| `wetgridtotalunc` | `f64` | The same for wet deposition. |
| `drygridtotalunc` | `f64` | The same for dry deposition. |

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
    fn clone(self: &Self) -> Uncertainty { /* ... */ }
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
    fn eq(self: &Self, other: &Uncertainty) -> bool { /* ... */ }
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
#### Struct `ReceptorOutput`

Receptor records (`concoutput`, `concoutput_surf` only).

```rust
pub struct ReceptorOutput {
    pub conc: Vec<Vec<f64>>,
    pub pptv: Option<Vec<Vec<f64>>>,
    pub factor_dry: Vec<f64>,
    pub density: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `conc` | `Vec<Vec<f64>>` | `receptor_conc`: one record per species, `1e12*creceptor/outnum`. |
| `pptv` | `Option<Vec<Vec<f64>>>` | `receptor_pptv` (`iout` 2, 3): per species, mixing ratio. |
| `factor_dry` | `Vec<f64>` | `factor_dryreceptor`: `rho/rho_dry` at each receptor. |
| `density` | `Vec<f64>` | The receptor air density used (`densityoutrecept`). |

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
    fn clone(self: &Self) -> ReceptorOutput { /* ... */ }
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
    fn eq(self: &Self, other: &ReceptorOutput) -> bool { /* ... */ }
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
#### Struct `ConcOutput`

What one call of the routine produces.

```rust
pub struct ConcOutput {
    pub density: Vec<f64>,
    pub density_dry: Vec<f64>,
    pub factor_drygrid: Vec<f64>,
    pub factor3d: Vec<f64>,
    pub slices: Vec<SliceOutput>,
    pub factor_drygrid_sparse: SparseField,
    pub uncertainty: Option<Uncertainty>,
    pub receptors: Option<ReceptorOutput>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `Vec<f64>` | `densityoutgrid`, kg/m³, `ix` fastest, then `jy`, level. |
| `density_dry` | `Vec<f64>` | `densitydrygrid`. |
| `factor_drygrid` | `Vec<f64>` | `factor_drygrid = density/density_dry`. |
| `factor3d` | `Vec<f64>` | `factor3d`. |
| `slices` | `Vec<SliceOutput>` | Slices in upstream's loop order: species, then release slot, then<br>age class (age fastest). |
| `factor_drygrid_sparse` | `SparseField` | The `factor_drygrid` file's packed field (no time record). |
| `uncertainty` | `Option<Uncertainty>` | `None` for [`Routine::Nest`]. |
| `receptors` | `Option<ReceptorOutput>` | `None` for [`Routine::Nest`] or without receptors. |

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
    fn clone(self: &Self) -> ConcOutput { /* ... */ }
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
    fn eq(self: &Self, other: &ConcOutput) -> bool { /* ... */ }
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
#### Enum `ConcOutputError`

Why [`concoutput`] refused.

```rust
pub enum ConcOutputError {
    DimensionMismatch,
    SlotOutOfRange,
    TooFewLevels,
}
```

##### Variants

###### `DimensionMismatch`

An input array does not match the stated dimensions.

###### `SlotOutOfRange`

`memind2` (or slot 2 for the surface routine) is not a held slot.

###### `TooFewLevels`

`nz < 2`: the density interpolation needs two levels.

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
    fn clone(self: &Self) -> ConcOutputError { /* ... */ }
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
    fn eq(self: &Self, other: &ConcOutputError) -> bool { /* ... */ }
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

#### Function `concoutput`

**Attributes:**

- `Other("#[allow(clippy::too_many_lines)]")`

Run `concoutput` / `concoutput_nest` / `concoutput_surf`.

`gridunc` (or `griduncn`) and, for forward runs with deposition, the
deposition grids are the accumulated inputs; their `nx`/`ny` must match
`out`, and their level count `set.outheight`. `wetgridunc`/`drygridunc`
may be `None` when the switch is off or the run is backward (upstream
does not allocate them then). `receptors` is ignored by
[`Routine::Nest`].

# Errors
[`ConcOutputError`] for inconsistent dimensions or a time slot out of
range; upstream would read out of bounds.

```rust
pub fn concoutput(set: &ConcOutputSettings, met: &DensityFields, out: &OutputGrid, gridunc: &crate::flexpart::concentration::ConcentrationGrid, wetgridunc: Option<&crate::flexpart::concentration::DepositionGrid>, drygridunc: Option<&crate::flexpart::concentration::DepositionGrid>, receptors: &ReceptorInput) -> Result<ConcOutput, ConcOutputError> { /* ... */ }
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

#### Constant `VONKARMAN`

Von Kármán constant as used by `pbl_profile.f90` (`vonkarman=0.4`).

Numerically identical to [`KARMAN`]; kept as a separate name because
upstream uses the two names in different routines and a reader comparing
the port with `pbl_profile.f90` should find the same identifier.

```rust
pub const VONKARMAN: f64 = 0.4;
```

#### Constant `CONVKE`

Factor converting the Brunt–Väisälä-limited convective kinetic energy into
the `hmixplus` mixing-height increment (`convke=2.0`, `par_mod.f90:76`).

```rust
pub const CONVKE: f64 = 2.0;
```

#### Constant `NUMCLASS`

Number of landuse classes in the dry-deposition tables (`numclass=13`,
`par_mod.f90:222`). Class 12 (1-based) is the snow/ice class that
`getvdep.f90` substitutes when the snow depth exceeds 1 mm.

```rust
pub const NUMCLASS: usize = 13;
```

## Module `convection`

Emanuel's moist-convection scheme, version 4.3c (Emanuel 1991,
*J. Atmos. Sci.* 48, 2313–2335; Emanuel & Živković-Rothman 1999,
*J. Atmos. Sci.* 56, 1766–1782), as FLEXPART v10.4 carries it in
`convect43c.f90`: the buoyancy-sorting scheme that diagnoses, for one grid
column, the cloud-base mass flux, the entrainment/detrainment mass fluxes
between every pair of levels, and from them the **mass displacement matrix
`FMASS`** and the **compensating subsidence `SUB`** that FLEXPART's
`calcmatrix`/`redist` turn into particle redistribution probabilities.

# What is ported

[`convect`] is `SUBROUTINE CONVECT` and [`tlift`] is `SUBROUTINE TLIFT`,
translated line for line in `f64`, keeping Fortran's left-to-right operation
order (so a `real(8)` build of upstream and this port agree to the last bit
or two). Fortran's 1-based arrays are kept 1-based internally (index 0
unused) so every subscript can be read against the upstream line.

The commented-out dry-adiabatic adjustment (`IPBL /= 0`, upstream lines
314–389) is dead code upstream (`IPBL = 0` is a parameter) and is not
ported. The potential temperature `TH(I)` that upstream computes at lines
303–307 is read only by that dead block, so it is not computed here; it
cannot affect any output.

# Inputs and outputs

Upstream reads the column from `conv_mod` (`TCONV`, `QCONV`, `QSCONV`,
`PCONV_HPA`, `PHCONV_HPA`) and writes `FT`, `FQ`, `FMASS`, `SUB`,
`NCONVTOP` back into it. Here the column is the [`ConvectSounding`]
argument and the results come back in [`ConvectOutput`].

Units (FLEXPART's, bare `f64`): temperature K, specific humidity kg/kg,
pressure **hPa** (the scheme's own convention), `delt` s, cloud-base mass
flux kg m⁻² s⁻¹, tendencies K/s and (kg/kg)/s, `FMASS`/`SUB` kg m⁻² s⁻¹,
precipitation mm/day, `WD` m/s, `TPRIME` K, `QPRIME` kg/kg.

# Upstream quirks, reproduced and documented

1. **`NCONVTOP` is not assigned on any early return.** The five early
   `RETURN`s (lines 444, 456, 470, 489, 617) leave `NCONVTOP` holding
   whatever the previous call left in `conv_mod`. The port returns
   `nconvtop: None` there rather than inventing a value. FLEXPART's only
   caller, `calcmatrix`, never reads it on those paths (it requires
   `IFLAG` ∈ {1, 4} *and* a non-zero mass flux), which the code-to-code test
   confirms against the stale value upstream actually printed.
2. **`CBMF` is in/out state.** It must be "remembered by the calling
   program" (upstream's own comment); FLEXPART keeps it per grid column in
   `cbaseflux`. The `IFLAG = 0` return at line 489 (parcel stable at cloud
   base, no convection last step) returns `CBMF` unmodified (it is `0`).
   The return at line 617 (`CBMF` and `CBMFOLD` both `0` after the update)
   leaves `IFLAG = 1` with no mass fluxes computed.
3. **`IF(IFLAG.NE.4)IFLAG=1`** (line 494) is always taken, because `IFLAG`
   was set to `0` at line 312 and nothing sets it to `4` before then.
4. **`SIJ(I,I)=1.0` inside the `J` loop** (line 648): at `J = I` the
   just-computed `ANUM/DEI` is overwritten before use; the port does the
   same assignment in the same place.
5. **`TVAPLCL`** (line 595) extrapolates the *environmental* virtual
   temperature to the LCL using the *parcel* gradient
   `TVP(ICB)-TVP(ICB+1)`. Kept as written.
6. **Geopotential** (line 404) divides the layer thickness by the
   half-level pressure `PHCONV_HPA(I)`, not the mid-level one. Kept.
7. **The CBMF relaxation does not depend on the time step**: Forster's
   change sets `DELT0 = DELT/3` and `DAMPS = DAMP*DELT/DELT0`, i.e.
   `DAMPS ≈ 0.3` for any `DELT` (exactly what the floating-point division
   gives, which the port reproduces rather than writing `0.3`).
8. **Gotos.** `GOTO 405` (skip the precipitating downdraft when
   `EP(INB) < 1e-4`), `GOTO 360` (no downdraft mass flux at level 1) and
   `GOTO 400` (no `QP` update at `INB`) are reproduced as structured
   control flow with identical semantics.
9. **Local arrays are not stale.** The `NA×NA` locals (`MENT`, `QENT`,
   `ELIJ`, `SIJ`; 76 KB at `NA = 138`) exceed gfortran's documented default
   `-fmax-stack-var-size` (64 KiB), so they would live in static memory and
   persist between calls (not inspected in the binary). Every element the
   routine *reads* is re-initialised earlier in the same call (checked index
   by index during the port: all reads stay within `1..NL+1`, which lines
   290–302 and 533–549 initialise). Evidence: the reference driver runs all
   73 calls in one process, carrying whatever upstream leaves behind, while
   the port starts every call fresh, and the two agree bit for bit. No
   stale numerical state reaches any output except `NCONVTOP` (quirk 1).
10. **Two guards are unreachable or dead.** `FRAC = MIN(FRAC, 1.0)`
    (line 576) can never bind: `FRAC = -(CAPEM+BYP)/MAX(-BYP, 0.001)` with
    `CAPEM >= 0` is at most 1. `TG = MAX(TG, 35.0)` in `TLIFT` needs a
    lifted parcel colder than 35 K. Both are ported as written.

# Verification

Code-to-code against upstream compiled from source at both precisions
(`dev/flexpart_reference_convection.f90`,
`tests/flexpart_convection_code_to_code.rs`), 2026-10-02: on 73 soundings
straddling every branch above, **bit-exact against the real(8) build on
all 15 220 outputs** (`IFLAG`, `CBMF`, `PRECIP`, `WD`, `TPRIME`,
`QPRIME`, `NCONVTOP`, `FT`, `FQ`, `SUB`, `FMASS`). Against the shipped
real(4) build the residual (up to 2.7e-3 in `FMASS`) is upstream's own
single precision, measured on identical inputs; see the test's doc.
Mutation-tested: 22 mutations of this file, 17 killed; the 5 survivors
are `MIN(FRAC,1)` and the 35 K floor (quirk 10), the `|DEI| < 0.01`
guard (needs a J/kg-scale coincidence), dropping `GOTO 405` (on the sweep
the skipped downdraft computes exact zeros, since `EP` grows with height),
and `DAMP*DELT/DELT0 -> DAMP*3` (`1-DAMPS` is the same double either
way).

# Not established

Verification only: the port computes what upstream computes. Nothing here
says the scheme represents real convection, and per the crate's scope limit
nothing here supports operational or emergency use.

```rust
pub mod convection { /* ... */ }
```

### Types

#### Struct `ConvectSounding`

One grid column as `convect` reads it from `conv_mod`. Every vector is
0-based in Rust and holds Fortran level `k` at index `k-1`.

```rust
pub struct ConvectSounding {
    pub t: Vec<f64>,
    pub q: Vec<f64>,
    pub qs: Vec<f64>,
    pub p_hpa: Vec<f64>,
    pub ph_hpa: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t` | `Vec<f64>` | `TCONV`: temperature, K, levels `1..=NL+1`. |
| `q` | `Vec<f64>` | `QCONV`: specific humidity, kg/kg, levels `1..=NL+1`. |
| `qs` | `Vec<f64>` | `QSCONV`: saturation specific humidity, kg/kg, levels `1..=NL+1`. |
| `p_hpa` | `Vec<f64>` | `PCONV_HPA`: mid-level pressure, hPa, levels `1..=NL+1`. |
| `ph_hpa` | `Vec<f64>` | `PHCONV_HPA`: half-level pressure, hPa; `PHCONV_HPA(k)` is the bottom<br>interface of level `k`. Levels `1..=NL+1`. |

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
    fn clone(self: &Self) -> ConvectSounding { /* ... */ }
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
    fn eq(self: &Self, other: &ConvectSounding) -> bool { /* ... */ }
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
#### Enum `ConvectFlag`

`IFLAG` as upstream defines it.

```rust
pub enum ConvectFlag {
    NoConvection,
    Convection,
    LclTooHigh,
    CloudBaseTooHigh,
    CflViolated,
}
```

##### Variants

###### `NoConvection`

`0`: no moist convection (stable, parcel too cold or dry, or stable at
cloud base with no convection at the previous step).

###### `Convection`

`1`: moist convection occurs.

###### `LclTooHigh`

`2`: LCL above 200 hPa (or the LCL formula gives ≥ 2000 hPa).

###### `CloudBaseTooHigh`

`3`: cloud base at or above level `NL-1`.

###### `CflViolated`

`4`: convection occurs but the subsidence CFL condition is violated.

##### Implementations

###### Methods

- ```rust
  pub fn code(self: Self) -> i32 { /* ... */ }
  ```
  Upstream's integer code.

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
    fn clone(self: &Self) -> ConvectFlag { /* ... */ }
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
    fn eq(self: &Self, other: &ConvectFlag) -> bool { /* ... */ }
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
#### Struct `ConvectOutput`

What `convect` returns or writes into `conv_mod`.

```rust
pub struct ConvectOutput {
    pub iflag: ConvectFlag,
    pub cbmf: f64,
    pub precip: f64,
    pub wd: f64,
    pub tprime: f64,
    pub qprime: f64,
    pub ft: Vec<f64>,
    pub fq: Vec<f64>,
    pub sub: Vec<f64>,
    pub fmass: Vec<f64>,
    pub n: usize,
    pub nconvtop: Option<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `iflag` | `ConvectFlag` | `IFLAG`. |
| `cbmf` | `f64` | `CBMF` on return: the updated cloud-base mass flux, kg m⁻² s⁻¹. |
| `precip` | `f64` | `PRECIP`, mm/day. |
| `wd` | `f64` | `WD`, downdraft velocity scale, m/s. |
| `tprime` | `f64` | `TPRIME`, K. |
| `qprime` | `f64` | `QPRIME`, kg/kg. |
| `ft` | `Vec<f64>` | `FT(1..=NL+1)`, K/s, at index `k-1`. |
| `fq` | `Vec<f64>` | `FQ(1..=NL+1)`, (kg/kg)/s. |
| `sub` | `Vec<f64>` | `SUB(1..=NL+1)`, kg m⁻² s⁻¹, positive downwards. |
| `fmass` | `Vec<f64>` | `FMASS(1..=NL+1, 1..=NL+1)`, kg m⁻² s⁻¹, row-major: `FMASS(i,j)` at<br>`(i-1)*(NL+1) + (j-1)`. Use [`ConvectOutput::fmass`]. |
| `n` | `usize` | `NL + 1`, the side of `fmass`. |
| `nconvtop` | `Option<usize>` | `NCONVTOP`; `None` where upstream returns early without assigning it<br>(quirk 1 of the module doc). |

##### Implementations

###### Methods

- ```rust
  pub fn fmass(self: &Self, i: usize, j: usize) -> f64 { /* ... */ }
  ```
  `FMASS(i, j)`, 1-based like upstream.

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
    fn clone(self: &Self) -> ConvectOutput { /* ... */ }
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
    fn eq(self: &Self, other: &ConvectOutput) -> bool { /* ... */ }
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
#### Struct `LiftedParcel`

Lifted-parcel quantities from [`tlift`].

```rust
pub struct LiftedParcel {
    pub tvp: Vec<f64>,
    pub tp: Vec<f64>,
    pub clw: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tvp` | `Vec<f64>` | `TVP`, K, 0-based (Fortran level `k` at `k-1`); untouched levels are 0. |
| `tp` | `Vec<f64>` | `TPK`, K. |
| `clw` | `Vec<f64>` | `CLW`, kg/kg. |

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
    fn clone(self: &Self) -> LiftedParcel { /* ... */ }
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
    fn eq(self: &Self, other: &LiftedParcel) -> bool { /* ... */ }
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

#### Function `tlift`

**Attributes:**

- `MustUse { reason: None }`

Public form of upstream `TLIFT`, both passes (`KK = 1` then `KK = 2`) on a
zeroed parcel, exactly as `CONVECT` drives it but without the virtual
temperature correction `CONVECT` applies between the passes. `gz` is the
geopotential, 0-based, levels `1..=nl+1`. Exposed for inspection; the
code-to-code test checks it through [`convect`].

```rust
pub fn tlift(sounding: &ConvectSounding, gz: &[f64], icb: usize, nk: usize, nl: usize) -> LiftedParcel { /* ... */ }
```

#### Function `convect`

**Attributes:**

- `Other("#[allow(clippy::too_many_lines, clippy::cognitive_complexity,\nclippy::needless_range_loop)]")`
- `MustUse { reason: None }`

`SUBROUTINE CONVECT(ND, NL, DELT, IFLAG, PRECIP, WD, TPRIME, QPRIME, CBMF)`.

- `sounding` — the column (`conv_mod`'s `TCONV`, `QCONV`, `QSCONV`,
  `PCONV_HPA`, `PHCONV_HPA`), each at least `nl + 1` levels.
- `nl` — `NL`, the highest level convection may reach plus one
  (FLEXPART passes `nconvlev`). Must be at least 3.
- `delt` — `DELT`, s.
- `cbmf` — `CBMF` on entry: the cloud-base mass flux remembered from the
  previous call for this column, kg m⁻² s⁻¹ (`0` at the first call).

`ND` (array dimension) has no role once the arrays are slices.

# Panics
If `nl < 3` or a sounding vector is shorter than `nl + 1`.

```rust
pub fn convect(sounding: &ConvectSounding, nl: usize, delt: f64, cbmf: f64) -> ConvectOutput { /* ... */ }
```

## Module `convmix`

FLEXPART's convective mixing of particles: [`calcmatrix`] turns Emanuel's
mass-flux diagnosis ([`super::convection::convect`]) for one grid column
into a **redistribution matrix** `fmassfrac`; [`redist`] moves one particle
with it (a random destination level, or compensating subsidence); and
[`convmix`] drives both over every grid column that holds particles, on the
mother grid and the nests.

# State

Upstream couples the three routines through module state: `conv_mod`
(column profiles, `fmass`, `fmassfrac`, `sub`, `nconvtop`), `com_mod`
(particles, fields, `ldirect`, `lsynctime`, `height`) and one `SAVE`d
array in `redist` (`uvzlev`). The port makes that state explicit:
[`ConvMod`] mirrors `conv_mod` plus `redist`'s `uvzlev`; everything
`com_mod` supplies is an argument. Persistent state is exactly what upstream
keeps between calls, so stale-value behaviour is reproduced rather than
guessed (see the quirks below).

# Random numbers

`redist` draws one uniform number per redistributed particle with
Numerical Recipes' `ran3`. That generator is **not ported and not
reimplemented** (licence; maintainer decision gh:#410). The port takes the
draws as an input iterator, consumed in the same order and at the same point
(only for particles inside the convective domain) as upstream consumes
`ran3`.

# Sorting

`convmix` visits particles grouped by grid column, sorting with
Numerical Recipes' `sort2` (a median-of-three quicksort with insertion sort
below 7 elements). It is **not translated**. The port uses Rust's stable
`sort_by_key`: the sorted key sequence is identical for any correct sort,
so columns are visited in the same order and each column's particles are
the same set. Only the order of particles **within one column** can
differ: the stable sort keeps ascending particle index, `sort2` is unstable
for more than 7 particles. That order matters for exactly one thing — which
particle receives which `ran3` draw. The code-to-code test measures the
permutation against upstream's compiled `sort2` and records where it
differs (see `dev/flexpart_reference_convection.f90`).

# Upstream quirks, reproduced and documented

1. **`calcmatrix` restores the old cloud-base mass flux whenever `convect`
   reports no convection** (`calcmatrix.f90:110–113`). `convect` sets
   `CBMF = 0` on its `IFLAG` 0/2/3 returns, but `calcmatrix` overwrites it
   with `cbmfold`, so a column's `cbaseflux` never relaxes to 0 when it
   stops convecting. Emanuel's interface comment (`convect43c.f90:144–147`)
   asks the caller to keep the value `CONVECT` returns. Reproduced.
2. **GFS `phconv` at the top reads `pconv(nuvz)`, which nothing assigns**
   (`calcmatrix.f90:76` with `kuvz = nuvz`; `convmix.f90:181–188` fills
   `pconv(1..nuvz-1)`). In a fresh process that element is the static
   zero (when `nuvz < nuvzmax`; at `nuvz = nuvzmax` it is out of bounds).
   [`ConvMod`] keeps `pconv[nuvz]` at its initial `0.0` and never writes
   it, which is what upstream does when `nuvz < nuvzmax`.
3. **GFS nests use the previous column's pressures.** The nest loop of
   `convmix` (`convmix.f90:260–265`) has no GFS branch: it fills `tconv`/
   `qconv` from `tthn`/`qvhn` at `kz+1` and never sets `pconv`, so a GFS
   run's nested columns are computed with the `pconv` left by the last
   mother-grid column. Reproduced through the persistent [`ConvMod`].
4. **`redist`'s top half-level height reads unassigned levels.** The
   `uvzlev` integration runs to `nconvtop + 1`, which can be `nuvz`; there
   it reads `tconv(nuvz)`, `qconv(nuvz)` and (ECMWF) `pconv(nuvz)`, none
   of which `convmix`/`calcmatrix` assign. The value is used only if a
   particle is sent to level `nconvtop`, whose matrix entries are zero by
   construction (`nconvtop` is one above the highest non-zero `fmass`
   index), so it does not reach a particle position. [`ConvMod`] keeps
   those elements at their static `0.0`, as upstream.
5. **`nconvlev = nuvz - 1` would read unassigned levels in `convect`.**
   `gridcheck_ecmwf.f90:535–539` leaves `nconvlev = nuvz - 1` when no
   level lies above 50 hPa at standard surface pressure (the `DO` index
   after a completed loop); `convect` then reads `TCONV(nuvz)` etc., which
   `convmix` never fills. The port refuses that configuration
   ([`ConvMod::new`] requires `nconvlev <= nuvz - 2`) rather than guess.
6. **`convect`'s `NCONVTOP` on early return is stale** (see
   [`super::convection`]). `calcmatrix` only reads it after `IFLAG` ∈ {1,4}
   and a non-zero flux, when it has been assigned; [`ConvMod::nconvtop`] is
   left untouched otherwise, as upstream leaves it.
7. **`redist` recomputes `uvzlev` only when `ktop <= 1`**, and its comment
   says "when ktop.eq.1"; `convmix` resets `ktop = 0` at each new column
   and `redist` sets it to 2. Reproduced.
8. **`redist`'s subsidence step uses the signed `lsynctime`** while the
   matrix uses `abs(lsynctime)` (`convmix.f90:67`): in a backward run the
   subsidence displacement is reversed and the matrix transposed.
9. **`redist` reflects negative heights** (`ztra1 = -ztra1`) and caps every
   particle it is called for — including one above the convective domain
   (`goto 90`) — at `height(nz) - 0.5`.
10. **Gross-flux accounting (`iflux = 1`, `calcfluxes`) is not ported**:
    it belongs to FLEXPART's flux output, not to particle transport. The
    port is upstream with `iflux = 0`.
11. **A draw of exactly 0 relocates every particle to level 1.** `redist`
    accepts the first `k` with `rn <= ffraction`; `ffraction` starts at 0
    and `fmassfrac(levold, 1)` is 0 for any level that sends no mass to
    level 1, so `rn = 0` gives `levnew = 1` and, through the
    `ffraction > 1e-20` guard, `dlevfrac = 0.5`: the particle is placed at
    the log-pressure centre of level 1 whatever the matrix says. Measured in
    the fixture: every particle between 150 m and 15.5 km on the deep test
    column goes to 66.71 m at `rn = 0`, while at `rn = 1e-6` one of them
    moves. Whether `ran3` can return exactly 0 was not examined (its source
    is not read beyond its interface). Reproduced.

# Verification

Code-to-code against upstream compiled from source at both precisions
(`dev/flexpart_reference_convection.f90`,
`tests/flexpart_convection_code_to_code.rs`), 2026-10-02, **bit-exact
against the real(8) build** on every output: `calcmatrix` 21 calls
(4 200 outputs incl. `fmassfrac`), `redist` 736 single-particle calls
(2 944 outputs), `convmix` four successive calls on a mother grid and a
nest with 60 particles (424 outputs). `nest` coverage uses a `par_mod.f90`
copy with `maxnests=1, nxmaxn=12, nymaxn=12`. Mutation-tested: 20
mutations of this file, all killed. Stability of the sort: the test
measures that the stable sort and `sort2` agree on the sorted keys always,
and on the permutation whenever keys are distinct or `n <= 7`; on the
`convmix` calls they differ at 89-97 of 120 positions, and the test
re-pairs draws to particles to compare end to end.

# Units

Bare `f64` in FLEXPART's units: pressures Pa in `pconv`/`phconv`/`dpr`
(hPa in `*_hpa`), temperature K, specific humidity kg/kg, heights m above
ground, times s, mass fluxes kg m⁻² s⁻¹, `fmassfrac` kg/m².

```rust
pub mod convmix { /* ... */ }
```

### Types

#### Enum `ConvMetFormat`

Which meteorological product the columns come from (upstream's
`metdata_format`, `GRIBFILE_CENTRE_ECMWF` or not).

```rust
pub enum ConvMetFormat {
    Ecmwf,
    Gfs,
}
```

##### Variants

###### `Ecmwf`

ECMWF: pressures from the hybrid coefficients, convection on the
original model levels (`tth`, `qvh` at `kz+1`).

###### `Gfs`

NCEP GFS: pressures `pplev` supplied per level, convection on
FLEXPART's levels (`tt`, `qv` at `kz`).

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
    fn clone(self: &Self) -> ConvMetFormat { /* ... */ }
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
    fn eq(self: &Self, other: &ConvMetFormat) -> bool { /* ... */ }
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
#### Struct `HybridCoefficients`

Hybrid vertical coefficients from `com_mod` (`akz`, `bkz` at the model
levels, `akm`, `bkm` at the half levels), level `k` at index `k-1`. Pa and
dimensionless. Read only for [`ConvMetFormat::Ecmwf`].

```rust
pub struct HybridCoefficients {
    pub akz: Vec<f64>,
    pub bkz: Vec<f64>,
    pub akm: Vec<f64>,
    pub bkm: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `akz` | `Vec<f64>` | `akz`, Pa. |
| `bkz` | `Vec<f64>` | `bkz`. |
| `akm` | `Vec<f64>` | `akm`, Pa. |
| `bkm` | `Vec<f64>` | `bkm`. |

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
    fn clone(self: &Self) -> HybridCoefficients { /* ... */ }
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
    fn default() -> HybridCoefficients { /* ... */ }
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
    fn eq(self: &Self, other: &HybridCoefficients) -> bool { /* ... */ }
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
#### Struct `ConvMod`

`conv_mod`, plus `redist`'s `SAVE`d `uvzlev`. All vectors are **1-based**
(index 0 unused) so subscripts read like upstream; matrices are
`(i, j)` → `i * (dim) + j` via [`ConvMod::fmassfrac`].

Every element starts at `0.0`, as `conv_mod`'s static arrays do, and is
written only where upstream writes it.

```rust
pub struct ConvMod {
    pub nuvz: usize,
    pub nconvlev: usize,
    pub dim: usize,
    pub pconv: Vec<f64>,
    pub phconv: Vec<f64>,
    pub dpr: Vec<f64>,
    pub pconv_hpa: Vec<f64>,
    pub phconv_hpa: Vec<f64>,
    pub ft: Vec<f64>,
    pub fq: Vec<f64>,
    pub sub: Vec<f64>,
    pub tconv: Vec<f64>,
    pub qconv: Vec<f64>,
    pub qsconv: Vec<f64>,
    pub fmass: Vec<f64>,
    pub fmassfrac: Vec<f64>,
    pub psconv: f64,
    pub tt2conv: f64,
    pub td2conv: f64,
    pub nconvtop: usize,
    pub uvzlev: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuvz` | `usize` | `nuvz`: number of model levels. |
| `nconvlev` | `usize` | `nconvlev`: `NL` passed to `convect`. |
| `dim` | `usize` | Side of the 1-based arrays (`nuvz + 2`). |
| `pconv` | `Vec<f64>` | `pconv`, Pa. |
| `phconv` | `Vec<f64>` | `phconv`, Pa. |
| `dpr` | `Vec<f64>` | `dpr`, Pa. |
| `pconv_hpa` | `Vec<f64>` | `pconv_hpa`, hPa. |
| `phconv_hpa` | `Vec<f64>` | `phconv_hpa`, hPa. |
| `ft` | `Vec<f64>` | `ft`, K/s (written by `convect`). |
| `fq` | `Vec<f64>` | `fq`, (kg/kg)/s. |
| `sub` | `Vec<f64>` | `sub`, kg m⁻² s⁻¹. |
| `tconv` | `Vec<f64>` | `tconv`, K. |
| `qconv` | `Vec<f64>` | `qconv`, kg/kg. |
| `qsconv` | `Vec<f64>` | `qsconv`, kg/kg. |
| `fmass` | `Vec<f64>` | `fmass`, kg m⁻² s⁻¹, `dim × dim`. |
| `fmassfrac` | `Vec<f64>` | `fmassfrac`, kg/m², `dim × dim`. |
| `psconv` | `f64` | `psconv`, Pa. |
| `tt2conv` | `f64` | `tt2conv`, K. |
| `td2conv` | `f64` | `td2conv`, K. |
| `nconvtop` | `usize` | `nconvtop`. |
| `uvzlev` | `Vec<f64>` | `redist`'s `SAVE`d `uvzlev`, m. |

##### Implementations

###### Methods

- ```rust
  pub fn new(nuvz: usize, nconvlev: usize) -> Self { /* ... */ }
  ```
  Fresh `conv_mod` (all zeros) for `nuvz` levels and `nconvlev`.

- ```rust
  pub fn fmassfrac(self: &Self, i: usize, j: usize) -> f64 { /* ... */ }
  ```
  `fmassfrac(i, j)`, 1-based.

- ```rust
  pub fn fmass(self: &Self, i: usize, j: usize) -> f64 { /* ... */ }
  ```
  `fmass(i, j)`, 1-based.

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
    fn clone(self: &Self) -> ConvMod { /* ... */ }
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
    fn eq(self: &Self, other: &ConvMod) -> bool { /* ... */ }
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
#### Struct `CalcMatrixResult`

What [`calcmatrix`] reports besides the state it leaves in [`ConvMod`].

```rust
pub struct CalcMatrixResult {
    pub lconv: bool,
    pub iflag: super::convection::ConvectFlag,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lconv` | `bool` | `lconv`: the column convects and `fmassfrac` was updated. |
| `iflag` | `super::convection::ConvectFlag` | `IFLAG` from `convect`. |

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
    fn clone(self: &Self) -> CalcMatrixResult { /* ... */ }
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
    fn eq(self: &Self, other: &CalcMatrixResult) -> bool { /* ... */ }
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
#### Struct `RedistResult`

What [`redist`] did to one particle.

```rust
pub struct RedistResult {
    pub z: f64,
    pub ipconv: i32,
    pub drew: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `z` | `f64` | New `ztra1`, m. |
| `ipconv` | `i32` | `ipconv`: `-1` if the particle was moved by the matrix, else `1`. |
| `drew` | `bool` | Whether a random number was drawn (the particle was inside the<br>convective domain). |

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
    fn clone(self: &Self) -> RedistResult { /* ... */ }
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
    fn eq(self: &Self, other: &RedistResult) -> bool { /* ... */ }
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
#### Struct `ConvMixMet`

Fields `convmix` interpolates for one grid (mother or nest), at FLEXPART's
two memory slots. 2-D fields are indexed `ix + nx*jy`; 3-D fields
`(ix + nx*jy)*nlev + (k-1)` for level `k`.

- ECMWF (and every nest): `t3`/`q3` are `tth`/`qvh` (`tthn`/`qvhn`), read
  at `k = kz + 1`, `kz = 1..nuvz-1`; `p3` is unused.
- GFS mother grid: `t3`/`q3`/`p3` are `tt`/`qv`/`pplev`, read at `k = kz`.

```rust
pub struct ConvMixMet {
    pub nx: usize,
    pub ny: usize,
    pub nlev: usize,
    pub ps: [Vec<f64>; 2],
    pub tt2: [Vec<f64>; 2],
    pub td2: [Vec<f64>; 2],
    pub t3: [Vec<f64>; 2],
    pub q3: [Vec<f64>; 2],
    pub p3: [Vec<f64>; 2],
    pub cbaseflux: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | `nx` (`nxn` for a nest). |
| `ny` | `usize` | `ny`. |
| `nlev` | `usize` | Levels stored per column in the 3-D fields. |
| `ps` | `[Vec<f64>; 2]` | `ps`, Pa, per slot. |
| `tt2` | `[Vec<f64>; 2]` | `tt2`, K. |
| `td2` | `[Vec<f64>; 2]` | `td2`, K. |
| `t3` | `[Vec<f64>; 2]` | Temperature, K. |
| `q3` | `[Vec<f64>; 2]` | Specific humidity, kg/kg. |
| `p3` | `[Vec<f64>; 2]` | Pressure, Pa (GFS mother grid only). |
| `cbaseflux` | `Vec<f64>` | `cbaseflux` (`cbasefluxn`), kg m⁻² s⁻¹, in/out, `ix + nx*jy`. |

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
    fn clone(self: &Self) -> ConvMixMet { /* ... */ }
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
    fn eq(self: &Self, other: &ConvMixMet) -> bool { /* ... */ }
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
#### Struct `ConvMixNest`

One nested grid: its fields and `xln`, `yln`, `xrn`, `yrn` (mother-grid
coordinates of its corners) and `xresoln`, `yresoln`.

```rust
pub struct ConvMixNest {
    pub met: ConvMixMet,
    pub xln: f64,
    pub yln: f64,
    pub xrn: f64,
    pub yrn: f64,
    pub xresoln: f64,
    pub yresoln: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `met` | `ConvMixMet` | The nest's fields and `cbasefluxn`. |
| `xln` | `f64` | `xln`. |
| `yln` | `f64` | `yln`. |
| `xrn` | `f64` | `xrn`. |
| `yrn` | `f64` | `yrn`. |
| `xresoln` | `f64` | `xresoln`. |
| `yresoln` | `f64` | `yresoln`. |

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
    fn clone(self: &Self) -> ConvMixNest { /* ... */ }
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
    fn eq(self: &Self, other: &ConvMixNest) -> bool { /* ... */ }
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
#### Struct `ConvParticles`

The particle arrays of `com_mod` that `convmix` reads and writes.

```rust
pub struct ConvParticles {
    pub itra1: Vec<i64>,
    pub xtra1: Vec<f64>,
    pub ytra1: Vec<f64>,
    pub ztra1: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `itra1` | `Vec<i64>` | `itra1`, s. |
| `xtra1` | `Vec<f64>` | `xtra1`, grid units (`real(kind=dp)` upstream). |
| `ytra1` | `Vec<f64>` | `ytra1`, grid units. |
| `ztra1` | `Vec<f64>` | `ztra1`, m above ground; updated. |

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
    fn clone(self: &Self) -> ConvParticles { /* ... */ }
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
    fn eq(self: &Self, other: &ConvParticles) -> bool { /* ... */ }
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
#### Struct `ConvMixParams`

Scalars `convmix` takes from `com_mod`/`par_mod`.

```rust
pub struct ConvMixParams {
    pub itime: i64,
    pub memtime: [i64; 2],
    pub memind: [usize; 2],
    pub lsynctime: i64,
    pub ldirect: i32,
    pub format: ConvMetFormat,
    pub height_nz: f64,
    pub nxmax: usize,
    pub coeffs: HybridCoefficients,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `itime` | `i64` | `itime`, s. |
| `memtime` | `[i64; 2]` | `memtime(1..2)`, s. |
| `memind` | `[usize; 2]` | `memind(1..2)`: the memory slot (1 or 2) of each time. |
| `lsynctime` | `i64` | `lsynctime`, s (signed). |
| `ldirect` | `i32` | `ldirect`. |
| `format` | `ConvMetFormat` | `metdata_format`. |
| `height_nz` | `f64` | `height(nz)`, m. |
| `nxmax` | `usize` | `par_mod`'s `nxmax` (enters the nest-edge margin `eps = nxmax/3e5`). |
| `coeffs` | `HybridCoefficients` | Hybrid coefficients (ECMWF). |

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
    fn clone(self: &Self) -> ConvMixParams { /* ... */ }
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
    fn eq(self: &Self, other: &ConvMixParams) -> bool { /* ... */ }
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
#### Struct `ConvMixReport`

What [`convmix`] did, for inspection: the particles in the order they were
visited (0-based index, grid: 0 mother, `n` nest `n`) and the number of
draws consumed.

```rust
pub struct ConvMixReport {
    pub visited: Vec<(usize, usize, bool)>,
    pub draws_used: usize,
    pub columns: Vec<(usize, i64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `visited` | `Vec<(usize, usize, bool)>` | `(particle, grid, drew)` in visiting order, for columns that convect;<br>`drew` is whether `redist` consumed a random number for it. |
| `draws_used` | `usize` | Random numbers consumed. |
| `columns` | `Vec<(usize, i64)>` | Columns for which `calcmatrix` was called: `(grid, igrid)`. |

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
    fn clone(self: &Self) -> ConvMixReport { /* ... */ }
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
    fn default() -> ConvMixReport { /* ... */ }
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
    fn eq(self: &Self, other: &ConvMixReport) -> bool { /* ... */ }
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

#### Function `calcmatrix`

`subroutine calcmatrix(lconv, delt, cbmf, metdata_format)`.

Reads `psconv`, `tconv(1..nuvz-1)`, `qconv(1..nuvz-1)` and (GFS)
`pconv(1..nuvz-1)` from `cm`, as `convmix` leaves them; writes `pconv`
(ECMWF), `phconv`, `dpr`, `qsconv`, `*_hpa`, `ft`, `fq`, `fmass`, `sub`,
`nconvtop` and `fmassfrac` exactly where upstream writes them.

- `delt` — convection time step, s (`abs(lsynctime)`).
- `cbmf` — the column's cloud-base mass flux, kg m⁻² s⁻¹, in/out
  (`cbaseflux(ix,jy)`), see quirk 1.

```rust
pub fn calcmatrix(cm: &mut ConvMod, delt: f64, cbmf: &mut f64, format: ConvMetFormat, coeffs: &HybridCoefficients) -> CalcMatrixResult { /* ... */ }
```

#### Function `redist`

**Attributes:**

- `Other("#[allow(clippy::neg_multiply, clippy::assign_op_pattern)]")`

`subroutine redist(ipart, ktop, ipconv)` for one particle at height `z`
(`ztra1(abs(ipart))`, m).

- `ktop` — in/out; `<= 1` makes `redist` (re)compute `uvzlev` for this
  column and set it to 2 (quirk 7).
- `ldirect` — `1` forward (matrix rows), otherwise backward (columns).
- `lsynctime` — signed synchronisation step, s (quirk 8).
- `height_nz` — `height(nz)`, the top FLEXPART level, m.
- `draws` — the uniform numbers upstream takes from `ran3`; one is
  consumed iff the particle is inside the convective domain.

# Panics
If a draw is needed and `draws` is exhausted.

```rust
pub fn redist<I: Iterator<Item = f64>>(cm: &mut ConvMod, z: f64, ktop: &mut i32, ldirect: i32, lsynctime: i64, height_nz: f64, draws: &mut I) -> RedistResult { /* ... */ }
```

#### Function `sort_particles_by_column`

**Attributes:**

- `MustUse { reason: None }`

Stable replacement for Numerical Recipes' `sort2` (not translated): sort
`igrid` ascending, returning the sorted keys and the permutation `ipoint`
(0-based particle indices). Ties keep ascending particle index; see the
module doc for how that relates to `sort2`.

```rust
pub fn sort_particles_by_column(igrid: &[i64]) -> (Vec<i64>, Vec<usize>) { /* ... */ }
```

#### Function `convmix_column_keys`

**Attributes:**

- `MustUse { reason: None }`

The pseudo grid numbers `convmix` sorts by (`convmix.f90:85–137`):
`igrid` for the mother grid and `igridn[n]` for nest `n+1`, `1 + jy*nx + ix`
of the particle's nearest grid point, or `-1` for a particle not on that
grid (not yet released, i.e. `itra1 != itime`, or in another domain). A
particle goes to the innermost nest that contains it — strictly, and for
ECMWF with a margin `eps = nxmax/3e5` grid units — and otherwise to the
mother grid. `nint` is Fortran's round-half-away-from-zero, i.e.
[`f64::round`].

```rust
pub fn convmix_column_keys(mother: &ConvMixMet, nests: &[ConvMixNest], parts: &ConvParticles, prm: &ConvMixParams) -> (Vec<i64>, Vec<Vec<i64>>) { /* ... */ }
```

#### Function `convmix`

**Attributes:**

- `Other("#[allow(clippy::too_many_lines)]")`

`subroutine convmix(itime, metdata_format)` with `iflux = 0` (quirk 10).

# Panics
If `draws` runs out, or a particle's column index falls outside its grid.

```rust
pub fn convmix<I: Iterator<Item = f64>>(cm: &mut ConvMod, mother: &mut ConvMixMet, nests: &mut [ConvMixNest], parts: &mut ConvParticles, prm: &ConvMixParams, draws: &mut I) -> ConvMixReport { /* ... */ }
```

## Module `coordtrafo`

Release-point coordinate transformation: `coordtrafo.f90`.

Converts each release box from geographic degrees to mother-grid units,
`x = (lon - xlon0)/dx`, `y = (lat - ylat0)/dy`, then removes boxes that
fall outside the domain. Upstream reads the boxes from `point_mod`
(`xpoint1`, `ypoint1`, `xpoint2`, `ypoint2`) and the grid from `com_mod`
(`xlon0`, `ylat0`, `dx`, `dy`, `nxmin1`, `nymin1`, `xglobal`, `sglobal`,
`nglobal`); the port takes both as arguments and returns the surviving
boxes with their **original indices**, so a caller can carry the other
per-point arrays (`zpoint*`, `npart`, `kindz`, release times, `xmass`) the
way upstream shifts them.

# Translation notes and upstream quirks

* The domain test, after two polar clamps, is
  - `ypoint1 < 1e-6` or `ypoint1 >= nymin1 - 1e-6`, or
  - `ypoint2 < 1e-6` or `ypoint2 >= nymin1 - yrspc`, or
  - (non-global in x only) `xpoint1`/`xpoint2` `< 1e-6` or
    `>= nxmin1 - 1e-6`,

  where `yrspc = spacing(real(nymin1, kind=sp))` is the **single
  precision** spacing at `nymin1` in both upstream builds (`sp` is
  `selected_real_kind(6)`, untouched by `-fdefault-real-8`); the port
  computes it from the `f32` bit pattern ([`spacing_f32`]).
* Clamps: with `sglobal`, `ypoint1 < 1e-6` becomes `1e-6`; with `nglobal`,
  `ypoint2 > nymin1 - 1e-5` becomes `nymin1 - 10*yrspc`. **`ypoint1` is
  never clamped at the north pole and `ypoint2` never at the south**, so
  a box whose *lower* edge is at the north pole is removed even on a
  global grid, while a box whose lower edge is at the south pole is kept.
* The upper bound is asymmetric: `1e-6` for `ypoint1`, `yrspc` for
  `ypoint2`.
* **On a global 1-degree grid the margins are finer than the shipped
  build can resolve.** At `y = 180` an `f32` has a spacing of 1.5e-5, wider
  than the 1e-6 and 1e-5 margins, so for boxes whose edge is within ~1e-5
  deg of the north pole the outcome is decided by rounding: measured
  (2026-10-02) on four such boxes, the shipped build keeps 2, the real(8)
  build and this port keep 3.
* On a globally cyclic grid (`xglobal`) x is neither checked nor wrapped:
  a box west of `xlon0` keeps a negative x, one past `nxmin1` keeps it.
* Upstream restarts the scan from point 1 (`goto 15`) after every removal.
  The port does the same; because both clamps are idempotent this yields
  the same survivors as a single filtering pass.
* If no box survives (or none was given) upstream prints an error and
  executes `stop`; the port returns [`NoReleasePoints`].
* Upstream shifts `compoint` (the point names, 1001 slots) only for
  `j <= 1000`; names are not ported (they only feed messages here).

# Units

Input boxes in degrees; output boxes in mother-grid units; `dx`, `dy` in
degrees per grid cell.

```rust
pub mod coordtrafo { /* ... */ }
```

### Types

#### Struct `ReleaseGrid`

The mother-grid description `coordtrafo` reads from `com_mod`.

```rust
pub struct ReleaseGrid {
    pub xlon0: f64,
    pub ylat0: f64,
    pub dx: f64,
    pub dy: f64,
    pub nxmin1: i32,
    pub nymin1: i32,
    pub xglobal: bool,
    pub sglobal: bool,
    pub nglobal: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xlon0` | `f64` | `xlon0`: longitude of the lower-left grid point, deg. |
| `ylat0` | `f64` | `ylat0`: latitude of the lower-left grid point, deg. |
| `dx` | `f64` | `dx`: grid distance in x, deg. |
| `dy` | `f64` | `dy`: grid distance in y, deg. |
| `nxmin1` | `i32` | `nxmin1 = nx - 1`. |
| `nymin1` | `i32` | `nymin1 = ny - 1`. |
| `xglobal` | `bool` | `xglobal`: the grid is cyclic in longitude. |
| `sglobal` | `bool` | `sglobal`: the grid contains the south pole. |
| `nglobal` | `bool` | `nglobal`: the grid contains the north pole. |

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
    fn clone(self: &Self) -> ReleaseGrid { /* ... */ }
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
    fn eq(self: &Self, other: &ReleaseGrid) -> bool { /* ... */ }
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
#### Struct `ReleaseBox`

One release box, `point_mod`'s `xpoint1, ypoint1, xpoint2, ypoint2`.

```rust
pub struct ReleaseBox {
    pub xpoint1: f64,
    pub ypoint1: f64,
    pub xpoint2: f64,
    pub ypoint2: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xpoint1` | `f64` | Lower-left x (deg on input, grid units on output). |
| `ypoint1` | `f64` | Lower-left y. |
| `xpoint2` | `f64` | Upper-right x. |
| `ypoint2` | `f64` | Upper-right y. |

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
    fn clone(self: &Self) -> ReleaseBox { /* ... */ }
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
    fn eq(self: &Self, other: &ReleaseBox) -> bool { /* ... */ }
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
#### Struct `NoReleasePoints`

Upstream's `stop` "NO PARTICLE RELEASES ARE DEFINED": every box was out of
the domain, or none was given.

```rust
pub struct NoReleasePoints;
```

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
    fn clone(self: &Self) -> NoReleasePoints { /* ... */ }
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
    fn eq(self: &Self, other: &NoReleasePoints) -> bool { /* ... */ }
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

#### Function `spacing_f32`

Fortran `spacing(x)` for a default-kind (`f32`) `x`: `2^(e - 24)` where
`x = f * 2^e`, `0.5 <= |f| < 1`; `tiny(x)` for zero or when that would be
subnormal. Returned widened to `f64` (exact).

```rust
pub fn spacing_f32(x: f32) -> f64 { /* ... */ }
```

#### Function `coordtrafo`

`coordtrafo`: transform release boxes to grid units and drop those outside
the domain. Returns `(original index, box in grid units)` for every
survivor, in upstream's order.

```rust
pub fn coordtrafo(grid: &ReleaseGrid, points: &[ReleaseBox]) -> Result<Vec<(usize, ReleaseBox)>, NoReleasePoints> { /* ... */ }
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

## Module `domainfill`

Domain-filling mode (`mdomainfill = 1` air, `2` stratospheric ozone):
`init_domainfill.f90` fills the first release box with particles carrying
equal shares of the air mass in it, and `boundcond_domainfill.f90`
injects particles at the box's boundaries as air flows in.

# [`init_domainfill`]

The box is the first release point's, widened to whole grid cells
(`nx_we`, `ny_sn`). Each column's air mass is `(p(1) - p(nz)) / g` times
the cell area, with `p = rho R T` from memory slot **1**. Column `(ix, jy)`
receives `nint(0.999 npart colmass / total)` particles, placed at equal
pressure intervals when there are more than 20 and at random pressures
otherwise. For `mdomainfill = 2` a particle is kept only above 3000 m with
potential vorticity above `pvcrit`, and its mass becomes an ozone mass.

A second pass fixes, for each boundary column, a smaller set of release
heights (`zcolumn_we`, `zcolumn_sn`) and their number (`numcolumn_we`,
`numcolumn_sn`) for [`boundcond_domainfill`].

# [`boundcond_domainfill`]

Terminates particles at the current time that have left the box, then at
every boundary release height accumulates the inflowing air mass
`u rho A lsynctime` (time- and height-interpolated) and releases one
particle per `xmassperparticle` accumulated (rounded). Outflow resets the
accumulator to zero.

# Random numbers are an INPUT

Upstream draws from `ran1` (Numerical Recipes, not ported and not
re-implemented: gh:#410). The port takes the draws from an iterator. The
order is upstream's: in `init_domainfill`, per particle, the random
pressure (columns of 20 or fewer only), then x, an extra x draw in the
first and in the last grid column, y, and the uncertainty class (only for
a kept particle); in `boundcond_domainfill`, the position along the
boundary, the height (interior release heights only), then the class.

# Upstream quirks and defects reproduced

- **`acc_mass_sn` is zeroed at the wrong index.** `init_domainfill` writes
  `acc_mass_sn(1,jy,j) = 0` and `acc_mass_sn(2,jy,j) = 0` — indexed by the
  *latitude* index `jy`, although the array's second index is the
  longitude `ix` (`boundcond_domainfill` reads `acc_mass_sn(k,ix,j)`). The
  southern/northern accumulators of the boundary columns are therefore
  zeroed only where some `jy` of the box equals that `ix`. The western/
  eastern accumulators are zeroed for every column, not only the boundary
  ones. Both are harmless in a fresh run (the static arrays start at zero)
  and wrong after a restart into a different box. The port writes the same
  addresses, using upstream's column-major layout (so an index past
  `nxmax` aliases exactly as upstream's would).
- **Boundary heights and counts are only written for non-empty columns**:
  a column whose second-pass count rounds to 0 keeps the previous
  `numcolumn_*` and `zcolumn_*`.
- **`deltaz` of the last release height reads `zcolumn(j - 2)`**. With
  exactly two release heights in a boundary column that is
  `zcolumn(k, i, 0)`, outside the array; the port refuses
  ([`DomainFillError::UndefinedRead`]) before changing anything. With one
  release height, `deltaz` and the height read `zcolumn(k, i, 2)`, which
  `init_domainfill` never wrote for that column (zero in a fresh run,
  otherwise stale); the port reads the same element.
- **A rejected ozone particle still overwrites its slot.** In
  `boundcond_domainfill`, when the PV test fails, the vacant slot has
  already received x, y and z; it stays vacant (`itra1` unchanged) with
  those positions, and the next particle searches from the same slot.
- **`ylat` of the PV sign**: `init_domainfill` uses the column's latitude;
  on the western/eastern boundaries `boundcond_domainfill` uses the
  particle's, on the southern/northern ones the boundary's.
- **Reads past the filled grid** with a nonzero weight: in the last row of
  a box that ends at `nymin1`, a particle at `ytra1 > nymin1` interpolates
  `pv` from row `ny`. For the static `pv` that is the unused tail of the
  array (zero) when `ny < nymax`. The port reproduces the address (see
  [`StaticExtents`]); such a particle is then marked out of the domain.
- **Bilinear weights may be negative**: a particle at `ytra1 < 0` in the
  first row keeps `jym = int(ytra1) = 0`, so PV is extrapolated.
- **`hzone = 1/dyconst`** (the arc length) on the equatorial row instead of
  the zone height; polar rows of a global grid take a half-cell cap.
- **`numparttot` sums the planned column counts**, not the particles kept,
  so with `mdomainfill = 2` `xmassperparticle` is the air mass per
  *planned* particle.
- **`gdomainfill` is assigned only for global wind fields** (`xglobal`,
  `sglobal` and `nglobal`); otherwise the input value is kept.
- **`numactiveparticles`, `xm`, `accmasst`** are computed upstream and
  never used (their `write` is commented out). They are not ported.

# Not ported: file I/O

With `ipin = 1` (restart) and a non-global box, `init_domainfill` reads
`boundcond.bin`, overriding every boundary array; with `ipout > 0` at
`itime == loutend`, `boundcond_domainfill` writes it. The port does not do
file I/O: it reports [`InitDomainFillOutcome::restart_read_required`] and
[`BoundcondOutcome::dump_required`], and the caller loads or saves
[`DomainFillState`].

# Refusals

Upstream's `stop`s, and its writes past `maxpart` (it warns *after* having
written out of bounds), become [`DomainFillError`]s. Except where noted,
the state is then partly updated and should be treated as invalid.

# Index conventions and units

Grid indices are 0-based as upstream's; the boundary side `k` is 0
(west/south) or 1 (east/north); release heights `j` are 0-based
(upstream's `j - 1`); level indices 0-based. Positions in grid units,
heights m, masses kg, winds m/s, densities kg/m³, PV in pvu.

```rust
pub mod domainfill { /* ... */ }
```

### Types

#### Enum `DomainFillMode`

`mdomainfill`.

```rust
pub enum DomainFillMode {
    Air,
    StratosphericOzone,
}
```

##### Variants

###### `Air`

`1`: air tracer, every particle kept.

###### `StratosphericOzone`

`2`: stratospheric ozone tracer, particles kept only above 3000 m where
`pv > pvcrit`, mass scaled by `pv * 48/29 * ozonescale / 1e9`.

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
    fn clone(self: &Self) -> DomainFillMode { /* ... */ }
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
    fn eq(self: &Self, other: &DomainFillMode) -> bool { /* ... */ }
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
#### Struct `DomainFillGrid`

The grid as the domain-filling routines read it from `com_mod`.

```rust
pub struct DomainFillGrid {
    pub nx: i64,
    pub ny: i64,
    pub xglobal: bool,
    pub sglobal: bool,
    pub nglobal: bool,
    pub dx: f64,
    pub dy: f64,
    pub ylat0: f64,
    pub dyconst: f64,
    pub extents: super::release::StaticExtents,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `i64` | `nx`. |
| `ny` | `i64` | `ny`. |
| `xglobal` | `bool` | Cyclic in longitude. |
| `sglobal` | `bool` | Contains the south pole. |
| `nglobal` | `bool` | Contains the north pole. |
| `dx` | `f64` | Grid spacing, degrees. |
| `dy` | `f64` | Grid spacing, degrees. |
| `ylat0` | `f64` | Latitude of the grid origin, degrees. |
| `dyconst` | `f64` | Metres-to-grid-units factor in y, 1/m (`dyconst = 180/(dy r_earth pi)`). |
| `extents` | `super::release::StaticExtents` | Static array extents (`pv` reads past the grid). |

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
    fn clone(self: &Self) -> DomainFillGrid { /* ... */ }
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
    fn eq(self: &Self, other: &DomainFillGrid) -> bool { /* ... */ }
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
#### Struct `DomainFillSettings`

Run switches the domain-filling routines read from `com_mod`.

```rust
pub struct DomainFillSettings {
    pub mode: DomainFillMode,
    pub ipin: i64,
    pub ipout: i64,
    pub pvcrit: f64,
    pub ozonescale: f64,
    pub nclassunc: i64,
    pub mintime: i64,
    pub ldirect: i64,
    pub itsplit: i64,
    pub lsynctime: i64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mode` | `DomainFillMode` | `mdomainfill`. |
| `ipin` | `i64` | `ipin`: `0` fresh start; non-zero resumes from a particle dump. |
| `ipout` | `i64` | `ipout`: `> 0` dumps particles. |
| `pvcrit` | `f64` | PV threshold of the stratosphere, pvu (`pvcrit`). |
| `ozonescale` | `f64` | Ozone/PV ratio, ppb/pvu (`ozonescale`). |
| `nclassunc` | `i64` | Number of uncertainty classes. |
| `mintime` | `i64` | Minimum time step, s. |
| `ldirect` | `i64` | `+1` forward, `-1` backward. |
| `itsplit` | `i64` | Splitting time constant, s. |
| `lsynctime` | `i64` | Synchronisation interval, s. |

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
    fn clone(self: &Self) -> DomainFillSettings { /* ... */ }
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
    fn eq(self: &Self, other: &DomainFillSettings) -> bool { /* ... */ }
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
#### Struct `DomainFillState`

The domain-filling globals of `com_mod`. The four boundary arrays are
stored in **upstream's column-major layout** with upstream's extents,
so that upstream's index arithmetic, including its index defect (see the
module docs), addresses the same elements.

```rust
pub struct DomainFillState {
    pub nx_we: [i64; 2],
    pub ny_sn: [i64; 2],
    pub numcolumn: i64,
    pub gdomainfill: bool,
    pub xmassperparticle: f64,
    pub nxmax: usize,
    pub nymax: usize,
    pub maxcolumn: usize,
    pub numcolumn_we: Vec<i64>,
    pub numcolumn_sn: Vec<i64>,
    pub zcolumn_we: Vec<f64>,
    pub zcolumn_sn: Vec<f64>,
    pub acc_mass_we: Vec<f64>,
    pub acc_mass_sn: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx_we` | `[i64; 2]` | Western and eastern boundary x index (`nx_we`). |
| `ny_sn` | `[i64; 2]` | Southern and northern boundary y index (`ny_sn`). |
| `numcolumn` | `i64` | Largest particle count of any column (`numcolumn`). |
| `gdomainfill` | `bool` | Global domain filling: no boundary conditions (`gdomainfill`). |
| `xmassperparticle` | `f64` | Air mass per particle, kg (`xmassperparticle`). |
| `nxmax` | `usize` | `nxmax` of the arrays. |
| `nymax` | `usize` | `nymax` of the arrays. |
| `maxcolumn` | `usize` | `maxcolumn`. |
| `numcolumn_we` | `Vec<i64>` | `numcolumn_we(2, 0:nymax-1)`, at `k + 2*jy`. |
| `numcolumn_sn` | `Vec<i64>` | `numcolumn_sn(2, 0:nxmax-1)`, at `k + 2*ix`. |
| `zcolumn_we` | `Vec<f64>` | `zcolumn_we(2, 0:nymax-1, maxcolumn)`, m. |
| `zcolumn_sn` | `Vec<f64>` | `zcolumn_sn(2, 0:nxmax-1, maxcolumn)`, m. |
| `acc_mass_we` | `Vec<f64>` | `acc_mass_we(2, 0:nymax-1, maxcolumn)`, kg. |
| `acc_mass_sn` | `Vec<f64>` | `acc_mass_sn(2, 0:nxmax-1, maxcolumn)`, kg. |

##### Implementations

###### Methods

- ```rust
  pub fn new(nxmax: usize, nymax: usize, maxcolumn: usize) -> Self { /* ... */ }
  ```
  Zeroed arrays (a fresh run's static memory).

- ```rust
  pub fn we(self: &Self, k: usize, jy: i64, j: i64) -> Option<usize> { /* ... */ }
  ```
  Index into `zcolumn_we`/`acc_mass_we` of `(k, jy, j)`.

- ```rust
  pub fn sn(self: &Self, k: usize, ix: i64, j: i64) -> Option<usize> { /* ... */ }
  ```
  Index into `zcolumn_sn`/`acc_mass_sn` of `(k, ix, j)`.

- ```rust
  pub fn ncol_we(self: &Self, k: usize, jy: i64) -> i64 { /* ... */ }
  ```
  `numcolumn_we(k, jy)`.

- ```rust
  pub fn ncol_sn(self: &Self, k: usize, ix: i64) -> i64 { /* ... */ }
  ```
  `numcolumn_sn(k, ix)`.

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
    fn clone(self: &Self) -> DomainFillState { /* ... */ }
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
    fn eq(self: &Self, other: &DomainFillState) -> bool { /* ... */ }
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
#### Enum `DomainFillError`

Why a domain-filling routine refused.

```rust
pub enum DomainFillError {
    TooManyParticles,
    MaxColumnTooSmall,
    DrawsExhausted,
    UndefinedRead,
    UndefinedLevel,
}
```

##### Variants

###### `TooManyParticles`

More particles than slots. In `boundcond_domainfill` upstream `stop`s;
in `init_domainfill` it writes past `maxpart` and only warns.

###### `MaxColumnTooSmall`

A boundary column needs more than `maxcolumn` release heights
(upstream: `stop 'maxcolumn too small'`).

###### `DrawsExhausted`

The draw iterator ran dry.

###### `UndefinedRead`

Upstream would read or write outside an array (see the module docs).

###### `UndefinedLevel`

No model level lies above a height (upstream would use an undefined
level index).

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
    fn clone(self: &Self) -> DomainFillError { /* ... */ }
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
    fn eq(self: &Self, other: &DomainFillError) -> bool { /* ... */ }
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
#### Struct `InitDomainFillOutcome`

What [`init_domainfill`] reports besides the state it writes.

```rust
pub struct InitDomainFillOutcome {
    pub early_return: bool,
    pub colmasstotal: f64,
    pub restart_read_required: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `early_return` | `bool` | `true` if upstream returned early (global domain filling resumed from a<br>dump: nothing else was done). |
| `colmasstotal` | `f64` | Total air mass in the box, kg (`colmasstotal`). |
| `restart_read_required` | `bool` | Upstream would now read `boundcond.bin` (`ipin == 1`, not global); the<br>caller must load the boundary arrays of [`DomainFillState`]. |

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
    fn clone(self: &Self) -> InitDomainFillOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &InitDomainFillOutcome) -> bool { /* ... */ }
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
#### Struct `BoundcondOutcome`

What [`boundcond_domainfill`] reports besides the state it writes.

```rust
pub struct BoundcondOutcome {
    pub early_return: bool,
    pub dump_required: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `early_return` | `bool` | `true` if upstream returned at once (`gdomainfill`). |
| `dump_required` | `bool` | Upstream would now write `boundcond.bin` (`ipout > 0` and<br>`itime == loutend`); the caller saves [`DomainFillState`]. |

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
    fn clone(self: &Self) -> BoundcondOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &BoundcondOutcome) -> bool { /* ... */ }
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

#### Function `init_domainfill`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`init_domainfill`: fill the first release box with particles.

# Arguments
- `xpoint1`, `xpoint2`, `ypoint1`, `ypoint2` — the first release box,
  grid units; `npart1` — its particle count (`npart(1)`).
- `grid`, `settings` — the `com_mod` grid and switches.
- `met` — mother-grid `rho`, `tt` (memory slot **1**) and `height`.
- `pv` — potential vorticity, pvu, laid out like `met`'s 3-D fields.
- `draws` — the `ran1` sequence.
- `parts` — the particle arrays (`numpart` is reset to 0 when `ipin == 0`).
- `state` — the domain-filling globals; written.

# Errors
See [`DomainFillError`].

```rust
pub fn init_domainfill<I: Iterator<Item = f64>>(xpoint1: f64, xpoint2: f64, ypoint1: f64, ypoint2: f64, npart1: i64, grid: &DomainFillGrid, settings: &DomainFillSettings, met: &super::interpolation::MetFields, pv: &[f64], draws: &mut I, parts: &mut super::release::ParticleStore, state: &mut DomainFillState) -> Result<InitDomainFillOutcome, DomainFillError> { /* ... */ }
```

#### Function `boundcond_domainfill`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]")`

`boundcond_domainfill(itime, loutend)`: terminate particles that left the
box and inject new ones where air flows in.

# Arguments
- `itime` — current time, s; `loutend` — end of the current output
  averaging interval, s (only decides [`BoundcondOutcome::dump_required`]).
- `grid`, `settings` — the `com_mod` grid and switches.
- `met` — mother-grid `uu`, `vv`, `rho`, `height`, `memtime`, `memind`.
- `pv` — potential vorticity, pvu, laid out like `met`'s 3-D fields.
- `draws` — the `ran1` sequence.
- `parts`, `state` — read and written.

# Errors
See [`DomainFillError`]. A boundary column with exactly two release heights
is refused before anything is changed.

```rust
pub fn boundcond_domainfill<I: Iterator<Item = f64>>(itime: i64, loutend: i64, grid: &DomainFillGrid, settings: &DomainFillSettings, met: &super::interpolation::MetFields, pv: &[f64], draws: &mut I, parts: &mut super::release::ParticleStore, state: &mut DomainFillState) -> Result<BoundcondOutcome, DomainFillError> { /* ... */ }
```

## Module `dry_deposition`

Dry deposition velocity: the resistance model for gases (Wesely 1989), the
particle scheme (Slinn 1982), their composition per grid cell, and the
Reynolds-dependent settling velocity.

| Function | Upstream | Returns |
|---|---|---|
| [`getrb`] | `getrb.f90` | quasi-laminar sublayer resistance `r_b`, s/m |
| [`getrc`] | `getrc.f90` | bulk surface resistance `r_c`, s/m |
| [`partdep`] | `partdep.f90` | particle deposition velocity, m/s |
| [`getvdep`] | `getvdep.f90` | deposition velocity for a grid cell, m/s |
| [`get_settling`] | `get_settling.f90` | settling velocity with drag iteration, m/s |

# Inputs that upstream reads from `com_mod`

Upstream reads the Wesely resistance tables, landuse fractions, roughness
lengths and species properties from global module arrays filled by
`readlanduse.f90`, `readdepo.f90` and `readreleases.f90`. The port takes
them as arguments: [`SurfaceResistances`] per season and class,
[`GasSpecies`] and [`DepositionSpecies`] per species. **The tables are data,
not physics**; nothing here ships FLEXPART's `surfdepo.t` values.

# Units

FLEXPART's, as bare `f64`: resistances s/m, velocities m/s, temperatures K
except where a doc says Celsius, pressure Pa, kinematic viscosity m²/s.

```rust
pub mod dry_deposition { /* ... */ }
```

### Types

#### Struct `SurfaceResistances`

Wesely (1989) resistances for one season, one landuse class and one
species, s/m. Upstream's `ri(season,class)`, `rac(season,class)` and the
species-specific `rcl`, `rgs`, `rlu(species,season,class)`.

`9999` and `1e25` are upstream's "no such pathway" markers and are used as
they are, as very large resistances.

```rust
pub struct SurfaceResistances {
    pub ri: f64,
    pub rac: f64,
    pub rcl: f64,
    pub rgs: f64,
    pub rlu: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ri` | `f64` | Minimum bulk canopy stomatal resistance for water vapour. |
| `rac` | `f64` | In-canopy aerodynamic resistance. |
| `rcl` | `f64` | Resistance of the lower canopy. |
| `rgs` | `f64` | Ground surface resistance. |
| `rlu` | `f64` | Leaf cuticle resistance. |

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
    fn clone(self: &Self) -> SurfaceResistances { /* ... */ }
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
    fn default() -> SurfaceResistances { /* ... */ }
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
    fn eq(self: &Self, other: &SurfaceResistances) -> bool { /* ... */ }
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
#### Struct `GasSpecies`

Gas-phase deposition properties of one species (from `SPECIES_nnn`).

```rust
pub struct GasSpecies {
    pub reldiff: f64,
    pub henry: f64,
    pub f0: f64,
    pub rm: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reldiff` | `f64` | Diffusivity of water vapour relative to the species, dimensionless.<br>A gas when `> 0`. |
| `henry` | `f64` | Effective Henry's constant, M/atm. |
| `f0` | `f64` | Reactivity relative to ozone, dimensionless. |
| `rm` | `f64` | Mesophyll resistance, s/m. |

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
    fn clone(self: &Self) -> GasSpecies { /* ... */ }
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
    fn eq(self: &Self, other: &GasSpecies) -> bool { /* ... */ }
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
#### Enum `DepositionSpecies`

**Attributes:**

- `Other("#[allow(clippy::large_enum_variant)]")`

How a species deposits, mapped from upstream's flag combination.

Upstream encodes this in three numbers per species: `reldiff > 0` marks a
gas, `density > 0` a particle, and `reldiff < 0`, `density < 0`,
`dryvel > 0` a species with a prescribed velocity.

```rust
pub enum DepositionSpecies {
    Gas(GasSpecies),
    Particle {
        density: f64,
        bins: super::aerosol::AerosolBins,
    },
    Prescribed(f64),
}
```

##### Variants

###### `Gas`

Wesely resistance model.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `GasSpecies` |  |

###### `Particle`

Slinn particle scheme with the given density (kg/m³) and size bins.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `f64` | Particle density, kg/m³. |
| `bins` | `super::aerosol::AerosolBins` | Size bins from [`super::aerosol::part0`]. |

###### `Prescribed`

Constant deposition velocity, m/s (`dryvel`). Upstream applies it only
when it is `> 0`; otherwise the species does not deposit.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

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
    fn clone(self: &Self) -> DepositionSpecies { /* ... */ }
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
    fn eq(self: &Self, other: &DepositionSpecies) -> bool { /* ... */ }
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
#### Struct `SurfaceMet`

Surface meteorology `getvdep` needs at one grid cell.

```rust
pub struct SurfaceMet {
    pub ust: f64,
    pub temp: f64,
    pub pa: f64,
    pub ol: f64,
    pub gr: f64,
    pub rh: f64,
    pub rr: f64,
    pub snow: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ust` | `f64` | Friction velocity, m/s. |
| `temp` | `f64` | 2 m temperature, K. |
| `pa` | `f64` | Surface pressure, Pa. |
| `ol` | `f64` | Obukhov length, m. |
| `gr` | `f64` | Global radiation, W/m². |
| `rh` | `f64` | Relative humidity, fraction. |
| `rr` | `f64` | Precipitation rate, mm/h. |
| `snow` | `f64` | Snow depth, m water equivalent. Above `0.001` the cell is treated as<br>fully snow-covered (landuse class 12). |

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
    fn clone(self: &Self) -> SurfaceMet { /* ... */ }
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
    fn eq(self: &Self, other: &SurfaceMet) -> bool { /* ... */ }
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

#### Function `getrb`

**Attributes:**

- `MustUse { reason: None }`

`getrb.f90`: quasi-laminar sublayer resistance for one gas.

`r_b = 2 (Sc / Pr)^0.67 / (kappa u*)`, with `Sc = (nu / D_H2O) * reldiff`.

# Arguments
- `ustar` — friction velocity, m/s.
- `nyl` — kinematic viscosity of air, m²/s.
- `diffh2o` — molecular diffusivity of water vapour, m²/s.
- `reldiff` — diffusivity of water relative to the species.

# Returns
`None` when `reldiff <= 0`: upstream then leaves `rb` unassigned, which is
how it marks a species that is not a gas.

```rust
pub fn getrb(ustar: f64, nyl: f64, diffh2o: f64, reldiff: f64) -> Option<f64> { /* ... */ }
```

#### Function `getrc`

**Attributes:**

- `MustUse { reason: None }`

`getrc.f90`: Wesely (1989) bulk surface resistance for one gas.

# Arguments
- `cell` — resistances for the current season and landuse class.
- `species` — the gas.
- `t_celsius` — 2 m temperature, **°C**.
- `gr` — global radiation, W/m².
- `rh` — relative humidity, fraction.
- `rr` — precipitation rate, mm/h.

# Returns
`None` when `species.reldiff <= 0` (upstream leaves `rc` unassigned).
Otherwise `r_c >= 10 s/m`; upstream floors it there.

```rust
pub fn getrc(cell: &SurfaceResistances, species: &GasSpecies, t_celsius: f64, gr: f64, rh: f64, rr: f64) -> Option<f64> { /* ... */ }
```

#### Function `partdep`

**Attributes:**

- `MustUse { reason: None }`

`partdep.f90`: particle dry deposition velocity, summed over the size bins.

Per bin, with Stokes number `St = v_s u*^2 / (g nu)`:
`r_dp = 1 / ((Sc^-2/3 + 10^(-3/St)) u*)` and
`v_d = v_s + 1 / (r_a + r_dp + r_a r_dp v_s)`; below `u* = 1e-5` only
settling remains. The bins are mass-weighted.

# Arguments
- `vdep` — value to accumulate into; upstream **adds** to its `vdepo`
  (`getvdep` zeroes it first). Kept as an argument because the order of
  that sum is part of what is verified.
- `density` — particle density, kg/m³; `<= 0` marks a non-particle species
  and returns `vdep` unchanged.
- `bins` — from [`super::aerosol::part0`]: uses `settling_velocity`,
  `schmidt_factor` and `mass_fraction`.
- `ra` — aerodynamic resistance, s/m.
- `ustar` — friction velocity, m/s.
- `nyl` — kinematic viscosity of air, m²/s.

```rust
pub fn partdep(vdep: f64, density: f64, bins: &super::aerosol::AerosolBins, ra: f64, ustar: f64, nyl: f64) -> f64 { /* ... */ }
```

#### Function `wesely_season`

**Attributes:**

- `MustUse { reason: None }`

The Wesely season (1-based, as upstream numbers them) `getvdep` uses for a
Julian date and latitude.

Reproduces upstream exactly, including two simplifications worth knowing:
the southern hemisphere is handled by adding **182 days** (`365/2` in
integer arithmetic), and everything between 20°S and 20°N is **always
summer** (`mmdd = 600`).

```rust
pub fn wesely_season(jul: f64, ylat: f64) -> usize { /* ... */ }
```

#### Function `getvdep`

**Attributes:**

- `MustUse { reason: None }`

`getvdep.f90`: dry deposition velocity of one species at one grid cell,
m/s.

# Arguments
- `jul` — Julian date of the meteorological field (`bdate + wftime/86400`).
- `ylat` — latitude of the cell, degrees.
- `met` — surface meteorology.
- `landuse` — fraction of each of the `NUMCLASS` landuse classes.
- `z0` — roughness length of each class, m.
- `table` — Wesely resistances `[season][class]` for this species.
- `species` — how the species deposits.

# Notes
The particle scheme uses the landuse-weighted **mean** `r_a`, as upstream
does, not a per-class velocity.

```rust
pub fn getvdep(jul: f64, ylat: f64, met: &SurfaceMet, landuse: &[f64; 13], z0: &[f64; 13], table: &[[SurfaceResistances; 13]; 5], species: &DepositionSpecies) -> f64 { /* ... */ }
```

#### Function `get_settling`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

`get_settling.f90`: settling velocity of a particle species at height `zt`,
m/s, **negative** (downward).

Starts from the Stokes estimate `vsetaver` and iterates the drag
coefficient (`24/Re`, `18.5/Re^0.6`, `0.44` for `Re` below `1.917`, below
`500`, and above) up to 20 times, stopping at a 1 % change. Air temperature
and density are interpolated linearly from the column.

# Arguments
- `zt` — particle height, m.
- `height` — model level heights, m, increasing (upstream's `height(1:nz)`).
- `tt`, `rho` — temperature (K) and air density (kg/m³) on those levels.
- `dquer` — mean particle diameter, **µm**.
- `density` — particle density, kg/m³.
- `cunningham` — Cunningham slip factor of the species.
- `vsetaver` — the Stokes settling velocity from `readreleases`, m/s
  (negative).

# Returns
`None` if `zt` is not below the top level. Upstream then uses an
**undefined** level index; the port refuses rather than guess.

```rust
pub fn get_settling(zt: f64, height: &[f64], tt: &[f64], rho: &[f64], dquer: f64, density: f64, cunningham: f64, vsetaver: f64) -> Option<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `NSEASON`

Number of Wesely seasons (`1` midsummer .. `5` transitional spring).

```rust
pub const NSEASON: usize = 5;
```

## Module `fluxes`

Gross mass fluxes through the output grid (`calcfluxes.f90`) and their
conversion to flux densities for output (`fluxoutput.f90`).

# What is counted

`calcfluxes` is called once per particle per time step (`iflux = 1`) with
the particle's position before (`xold`, `yold`, `zold`) and after the
step. It adds the particle's mass, per species, to six **gross** flux
accumulators, `flux(1:6, ix, jy, kz, species, release, age class)`:

| upstream | [`FluxGrid`] index | counts a crossing of | located at |
|---|---|---|---|
| `flux(1)` | [`WEST_TO_EAST`] | `xl = ix + 0.5` moving east | row `jyave`, level `kzave` |
| `flux(2)` | [`EAST_TO_WEST`] | `xl = ix + 0.5` moving west | row `jyave`, level `kzave` |
| `flux(3)` | [`SOUTH_TO_NORTH`] | `yl = jy + 0.5` moving north | column `ixave`, level `kzave` |
| `flux(4)` | [`NORTH_TO_SOUTH`] | `yl = jy + 0.5` moving south | column `ixave`, level `kzave` |
| `flux(5)` | [`UPWARD`] | `outheighthalf(kz+1)` moving up | cell `(ixave, jyave)` |
| `flux(6)` | [`DOWNWARD`] | `outheighthalf(kz+1)` moving down | cell `(ixave, jyave)` |

where `xl`, `yl` are positions in output-cell units and `ixave`, `jyave`,
`kzave` the cell of the **mean** horizontal position and of the **new**
height. The crossing indices come from `int(xl + 0.5)`, so the flux
"faces" are the **centre lines** of the output cells (and the vertical
faces the mid-levels `outheighthalf`), half a cell from the concentration
cells' faces. That is upstream's convention and the port keeps it.

# Upstream quirks and defects reproduced

* **Defect: the cyclic-boundary flux is never recorded.** For a particle
  that wraps around a global domain (`|xold - xtra1| >= nx/2`), upstream
  attributes the zonal flux to the column
  `ixs = int(((real(nxmin1) - 1.e5)*dx + xoutshift)/dxout)`. `1.e5` is
  almost certainly meant to be `1.e-5` ("just inside the eastern edge"):
  as written, `ixs` is about `-1e5*dx/dxout`, always negative for any
  realistic `xoutshift`, so the range check fails and the crossing is
  dropped. The code-to-code fixture has wrapping particles in both
  directions; upstream records nothing for them and nor does the port.
* `int()` truncates toward zero, so the mean-position cell `ixave` is 0 for
  `-1 < xl < 0` (no floor correction, unlike `conccalc`), and
  `int(xl + 0.5)` is 0 for `-1.5 < xl < 0.5`.
* The **vertical** fluxes are counted whenever the mean horizontal
  position is on the grid, even for a particle above the top output level
  (`kzave > numzgrid`); its crossing levels are clamped to `numzgrid`.
  The **horizontal** fluxes need `kzave <= numzgrid`.
* `xold`, `yold`, `zold` are default `real` upstream, `xtra1`, `ytra1`
  `real(kind=dp)`, so in the shipped build `xmean` and the old-position
  indices are single precision and the new-position indices double. The
  port computes everything in `f64`.
* The `flux` array is default `real` (single precision as shipped); the
  port accumulates in `f64`.

# `fluxoutput`

Upstream writes the six fields to the unformatted file
`grid_flux_YYYYMMDDHHMMSS` as `1e12 * flux / wall area / outstep`
(ng m⁻² s⁻¹ for masses in kg), choosing per species and age class between
a sparse list and a dense dump, then resets the six fields. The port
([`fluxoutput`]) returns exactly the records upstream writes, in its
order, and resets the grid; writing the file is left to the caller.
Quirks reproduced:

* The sparse/dense choice is made per (species, age class) from a count
  of positive cells **summed over all release points**, then applied to
  each release point; with several release points a block can be written
  dense although it has few positive cells.
* The record for `flux(2)` (east-to-west) is written **first**, under
  upstream's "east" counter `ncellse`, then `flux(1)`, `flux(3)`,
  `flux(4)`, `flux(5)`, `flux(6)`.
* The sparse index is `ix + jy*numxgrid + kz*numxgrid*numygrid` with the
  **1-based** level `kz` (the same convention as `concoutput`); the port
  reports it unchanged.
* A sparse record lists cells with flux `> 0` only; a dense record lists
  every value, ordered level, column, then row fastest.

# Units and indices

Positions in meteorological grid units (`xtra1`, `ytra1`), heights m above
ground; `dx`, `dy`, `dxout`, `dyout`, `xoutshift`, `youtshift` in degrees;
masses kg; `outstep` s; wall areas m². Columns, rows, levels, species,
release slots and age classes are **0-based** in [`FluxGrid`].

```rust
pub mod fluxes { /* ... */ }
```

### Types

#### Struct `FluxGrid`

The gross-flux accumulator `flux(6, 0:numxgrid-1, 0:numygrid-1,
numzgrid, nspec, maxpointspec_act, nageclass)`, kg.

```rust
pub struct FluxGrid {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub nspec: usize,
    pub npointspec: usize,
    pub nageclass: usize,
    pub values: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Columns. |
| `ny` | `usize` | Rows. |
| `nz` | `usize` | Levels. |
| `nspec` | `usize` | Species. |
| `npointspec` | `usize` | Release points with separate output (`maxpointspec_act`). |
| `nageclass` | `usize` | Age classes. |
| `values` | `Vec<f64>` | Values, direction fastest, then `ix`, `jy`, `kz`, species, release,<br>age class. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(nx: usize, ny: usize, nz: usize, nspec: usize, npointspec: usize, nageclass: usize) -> Self { /* ... */ }
  ```
  All-zero grid (all six directions; see the `outgrid` module doc on

- ```rust
  pub fn index(self: &Self, dir: usize, ix: usize, jy: usize, kz: usize, ks: usize, kp: usize, na: usize) -> usize { /* ... */ }
  ```
  Flat index; all indices 0-based, `dir` one of [`WEST_TO_EAST`] ...

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
    fn clone(self: &Self) -> FluxGrid { /* ... */ }
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
    fn eq(self: &Self, other: &FluxGrid) -> bool { /* ... */ }
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
#### Struct `FluxSettings`

The `com_mod` values `calcfluxes` reads besides the particle.

```rust
pub struct FluxSettings {
    pub output_for_each_release: bool,
    pub domain_filling: bool,
    pub dx: f64,
    pub dy: f64,
    pub nx: i64,
    pub outheight: Vec<f64>,
    pub outheighthalf: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `output_for_each_release` | `bool` | `ioutputforeachrelease = 1`. |
| `domain_filling` | `bool` | `mdomainfill = 1` (all mass to release slot 0). |
| `dx` | `f64` | Meteorological grid spacing in x, degrees (`dx`). |
| `dy` | `f64` | Meteorological grid spacing in y, degrees (`dy`). |
| `nx` | `i64` | Meteorological grid points in x (`nx`; `nxmin1 = nx - 1`). |
| `outheight` | `Vec<f64>` | Output level tops, m (`outheight`). |
| `outheighthalf` | `Vec<f64>` | Output mid-levels, m (`outheighthalf`). |

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
    fn clone(self: &Self) -> FluxSettings { /* ... */ }
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
    fn eq(self: &Self, other: &FluxSettings) -> bool { /* ... */ }
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
#### Struct `ParticleStep`

One particle's step, as `calcfluxes` sees it.

```rust
pub struct ParticleStep {
    pub old: [f64; 3],
    pub new: [f64; 3],
    pub npoint: usize,
    pub nage: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `old` | `[f64; 3]` | Position before the step, grid units / m (`xold`, `yold`, `zold`). |
| `new` | `[f64; 3]` | Position after the step, grid units / m (`xtra1`, `ytra1`, `ztra1`). |
| `npoint` | `usize` | Release point, 0-based (`npoint - 1`). |
| `nage` | `usize` | Age class, 0-based (`nage - 1`). |

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
    fn clone(self: &Self) -> ParticleStep { /* ... */ }
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
    fn eq(self: &Self, other: &ParticleStep) -> bool { /* ... */ }
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
#### Enum `FluxError`

Why [`calcfluxes`] refused.

```rust
pub enum FluxError {
    IndexOutsideGrid,
}
```

##### Variants

###### `IndexOutsideGrid`

Release slot, age class or species count outside the flux grid
(upstream writes out of bounds).

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
    fn clone(self: &Self) -> FluxError { /* ... */ }
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
    fn eq(self: &Self, other: &FluxError) -> bool { /* ... */ }
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
#### Enum `FluxLayout`

How one block of [`fluxoutput`] is written.

```rust
pub enum FluxLayout {
    Sparse(Vec<(i64, f64)>),
    Dense(Vec<f64>),
}
```

##### Variants

###### `Sparse`

Record `1`: `(index, value)` for every cell with flux `> 0`, index
`ix + jy*numxgrid + kz*numxgrid*numygrid` with 1-based `kz`; the
terminator `-999, 999.` is not included.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<(i64, f64)>` |  |

###### `Dense`

Record `2`: every value, level outermost, then column, row fastest.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<f64>` |  |

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
    fn clone(self: &Self) -> FluxLayout { /* ... */ }
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
    fn eq(self: &Self, other: &FluxLayout) -> bool { /* ... */ }
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
#### Struct `FluxBlock`

One block of the flux file: one direction of one species, release slot and
age class (0-based).

```rust
pub struct FluxBlock {
    pub ks: usize,
    pub kp: usize,
    pub nage: usize,
    pub dir: usize,
    pub layout: FluxLayout,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ks` | `usize` | Species. |
| `kp` | `usize` | Release slot. |
| `nage` | `usize` | Age class. |
| `dir` | `usize` | Direction ([`WEST_TO_EAST`] ... [`DOWNWARD`]). |
| `layout` | `FluxLayout` | The values, `1e12 * flux / area / outstep`. |

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
    fn clone(self: &Self) -> FluxBlock { /* ... */ }
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
    fn eq(self: &Self, other: &FluxBlock) -> bool { /* ... */ }
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
#### Struct `FluxOutput`

Everything `fluxoutput` writes.

```rust
pub struct FluxOutput {
    pub date: i64,
    pub time: i64,
    pub itime: i64,
    pub blocks: Vec<FluxBlock>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `date` | `i64` | `YYYYMMDD` of the file name. |
| `time` | `i64` | `HHMMSS` of the file name. |
| `itime` | `i64` | The time written as the first record, s. |
| `blocks` | `Vec<FluxBlock>` | The blocks, in file order. |

##### Implementations

###### Methods

- ```rust
  pub fn file_name(self: &Self) -> String { /* ... */ }
  ```
  The file name upstream opens under `path(2)`:

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
    fn clone(self: &Self) -> FluxOutput { /* ... */ }
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
    fn eq(self: &Self, other: &FluxOutput) -> bool { /* ... */ }
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

#### Function `calcfluxes`

**Attributes:**

- `Other("#[allow(clippy::int_plus_one)]")`

`calcfluxes.f90`: add one particle step's mass `xmass` (kg per species,
`xmass1(jpart, 1:nspec)`) to the gross fluxes in `flux`.

# Errors

[`FluxError::IndexOutsideGrid`] before anything is modified.

```rust
pub fn calcfluxes(step: &ParticleStep, xmass: &[f64], set: &FluxSettings, dom: &crate::flexpart::concentration::OutputDomain, flux: &mut FluxGrid) -> Result<(), FluxError> { /* ... */ }
```

#### Function `fluxoutput`

**Attributes:**

- `MustUse { reason: None }`

`fluxoutput.f90`: the records of the flux file at time `itime` (s after
the start, Julian date `bdate`), with output interval `outstep` (s,
`real(abs(loutstep))`), using the wall areas of `geometry` (the mother
output grid). Resets all six directions of `flux` to zero afterwards, as
upstream does.

# Panics

If `geometry`'s dimensions differ from `flux`'s.

```rust
pub fn fluxoutput(itime: i64, bdate: f64, outstep: f64, geometry: &crate::flexpart::outgrid::CellGeometry, flux: &mut FluxGrid) -> FluxOutput { /* ... */ }
```

### Constants and Statics

#### Constant `WEST_TO_EAST`

`flux(1)`: west-to-east crossings.

```rust
pub const WEST_TO_EAST: usize = 0;
```

#### Constant `EAST_TO_WEST`

`flux(2)`: east-to-west crossings.

```rust
pub const EAST_TO_WEST: usize = 1;
```

#### Constant `SOUTH_TO_NORTH`

`flux(3)`: south-to-north crossings.

```rust
pub const SOUTH_TO_NORTH: usize = 2;
```

#### Constant `NORTH_TO_SOUTH`

`flux(4)`: north-to-south crossings.

```rust
pub const NORTH_TO_SOUTH: usize = 3;
```

#### Constant `UPWARD`

`flux(5)`: upward crossings.

```rust
pub const UPWARD: usize = 4;
```

#### Constant `DOWNWARD`

`flux(6)`: downward crossings.

```rust
pub const DOWNWARD: usize = 5;
```

## Module `geodesy`

Great-circle distance on a sphere, as FLEXPART's plume-trajectory
clustering uses it.

Both routines use their own Earth radius, `6.3712e6 m`, not `par_mod`'s
`r_earth = 6.371e6`, and their own `pi = 3.14159265358979`. They return
**km**. Points closer than a threshold in both coordinates are reported as
exactly `0`, which upstream uses to avoid `acos` of a value a rounding
above 1.

```rust
pub mod geodesy { /* ... */ }
```

### Functions

#### Function `distance`

**Attributes:**

- `MustUse { reason: None }`

`distance.f90`: great-circle distance in km between two points given in
**degrees**; `0` when both coordinates differ by less than `0.03°`.

```rust
pub fn distance(rlat1: f64, rlon1: f64, rlat2: f64, rlon2: f64) -> f64 { /* ... */ }
```

#### Function `distance2`

**Attributes:**

- `MustUse { reason: None }`

`distance2.f90`: as [`distance`], with coordinates in **radians** and a
`0.0003 rad` threshold.

```rust
pub fn distance2(rlat1: f64, rlon1: f64, rlat2: f64, rlon2: f64) -> f64 { /* ... */ }
```

## Module `initial_condition`

Sensitivity to the initial conditions in backward runs
(`initial_cond_calc.f90`).

In a backward run (`ldirect = -1`) with `linit_cond > 0`, FLEXPART grids
each particle's mass onto `init_cond` at the moment the particle is
terminated (end of the run, or leaving the domain): the field it builds is
the sensitivity of the receptor to the concentration at the start of the
backward period. The gridding is `conccalc`'s uniform kernel without its
age threshold.

# The arithmetic, as upstream

* `linit_cond = 1` ("mass unit"): `rhoi` is the air density at the
  particle, bilinear in the horizontal and linear in height between the two
  model levels bracketing `ztra1`, all four corners from memory slot
  `memind(2)` (no time interpolation: "accurate enough"). The particle
  contributes `xmass1/rhoi`.
* `linit_cond = 2` ("mass mixing ratio unit"): `rhoi = 1`.
* The output level is the first with `outheight(kz) > ztra1`; a particle
  at or above the top level contributes nothing.
* Direct attribution (whole mass to the particle's cell) within half a
  cell of the grid edge (`xl < 0.5`, `yl < 0.5`, `xl > numxgrid - 1.5`,
  `yl > numygrid - 1.5`); otherwise the four-cell uniform kernel with
  weights `wx = 1.5 - ddx` / `0.5 + ddx` (and the same in y), each corner
  added only if it is on the grid. The contribution is
  `xmass1/rhoi*w`, i.e. `(xmass1/rhoi)*w`.

# Upstream quirks reproduced (or refused)

* The labels look inverted but are upstream's: the **mass** unit divides
  by the air density, the **mixing-ratio** unit does not.
* The output cell is `ix = int(xl); if (xl < 0) ix = ix - 1`, a floor that
  is wrong at negative integers (`xl = -1` gives `-2`); only off-grid
  cells are affected. The density corners use plain `int(xtra1)`
  (truncation toward zero) and have no bounds or pole check.
* With `linit_cond = 1`, the level search leaves `indz` **unassigned**
  for `ztra1 >= height(nz)`; upstream then uses a stale or undefined
  value. The search runs for every particle at `itime`, even one above the
  output grid. The port refuses ([`InitCondError::AboveTopLevel`]).
* Any `linit_cond` other than 1 or 2 leaves `rhoi` undefined; it cannot be
  expressed through [`InitCondUnit`].
* A tie `ddx = 0.5` takes the `else` side (`ixp = ix - 1`, `wx = 1`).
  The neighbour's weight `1 - wx` is then 0 on either side of the switch,
  so the tie cannot be seen in the result (a mutation moving it is an
  equivalent mutant); it is reproduced for fidelity only.
* The kernel branch tests the `ix` column before the `ixp` column and adds
  the corners in the order `(ix,jy)`, `(ix,jyp)`, `(ixp,jyp)`, `(ixp,jy)`.
  They are distinct cells, so the order does not change any sum.
* `init_cond` is default `real` (single precision as shipped); the port
  accumulates in `f64`.

# Units and indices

Positions in meteorological grid units, heights m, masses kg (or
whatever unit the release carries), density kg/m³. `init_cond` is a
[`ConcentrationGrid`] with one uncertainty class and one age class:
upstream's `init_cond(0:numxgrid-1, 0:numygrid-1, numzgrid, maxspec,
maxpointspec_act)`; all indices 0-based.

```rust
pub mod initial_condition { /* ... */ }
```

### Types

#### Enum `InitCondUnit`

`linit_cond`: the unit of the initial-condition sensitivity.

```rust
pub enum InitCondUnit {
    Mass,
    MassMixingRatio,
}
```

##### Variants

###### `Mass`

`linit_cond = 1`: mass divided by the air density at the particle.

###### `MassMixingRatio`

`linit_cond = 2`: mass, `rhoi = 1`.

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
    fn clone(self: &Self) -> InitCondUnit { /* ... */ }
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
    fn eq(self: &Self, other: &InitCondUnit) -> bool { /* ... */ }
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
#### Struct `InitCondSettings`

The `com_mod` switches `initial_cond_calc` reads.

```rust
pub struct InitCondSettings {
    pub unit: InitCondUnit,
    pub output_for_each_release: bool,
    pub domain_filling: bool,
    pub outheight: Vec<f64>,
    pub dx: f64,
    pub dy: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `unit` | `InitCondUnit` | `linit_cond`. |
| `output_for_each_release` | `bool` | `ioutputforeachrelease = 1`. |
| `domain_filling` | `bool` | `mdomainfill = 1` (all mass to release slot 0). |
| `outheight` | `Vec<f64>` | Output level tops, m (`outheight`). |
| `dx` | `f64` | Meteorological grid spacing in x, degrees (`dx`). |
| `dy` | `f64` | Meteorological grid spacing in y, degrees (`dy`). |

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
    fn clone(self: &Self) -> InitCondSettings { /* ... */ }
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
    fn eq(self: &Self, other: &InitCondSettings) -> bool { /* ... */ }
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
#### Struct `InitCondParticle`

One particle as `initial_cond_calc` reads it.

```rust
pub struct InitCondParticle {
    pub itra1: i64,
    pub xtra1: f64,
    pub ytra1: f64,
    pub ztra1: f64,
    pub npoint: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `itra1` | `i64` | Time the particle is at, s (`itra1`). |
| `xtra1` | `f64` | Position, grid units (`xtra1`). |
| `ytra1` | `f64` | Position, grid units (`ytra1`). |
| `ztra1` | `f64` | Height above ground, m (`ztra1`). |
| `npoint` | `usize` | Release point, 0-based (`npoint - 1`). |

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
    fn clone(self: &Self) -> InitCondParticle { /* ... */ }
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
    fn eq(self: &Self, other: &InitCondParticle) -> bool { /* ... */ }
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
#### Enum `InitCondError`

Why [`initial_cond_calc`] refused. Nothing has been modified then.

```rust
pub enum InitCondError {
    MissingDensity,
    AboveTopLevel,
    DensityOutsideGrid,
    IndexOutsideGrid,
}
```

##### Variants

###### `MissingDensity`

[`InitCondUnit::Mass`] but no density field supplied.

###### `AboveTopLevel`

`ztra1 >= height(nz)` with [`InitCondUnit::Mass`]: upstream's level
index is unassigned.

###### `DensityOutsideGrid`

A density corner falls outside the supplied arrays.

###### `IndexOutsideGrid`

Release slot, level or species count outside `init_cond`.

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
    fn clone(self: &Self) -> InitCondError { /* ... */ }
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
    fn eq(self: &Self, other: &InitCondError) -> bool { /* ... */ }
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

#### Function `initial_cond_calc`

**Attributes:**

- `Other("#[allow(clippy::int_plus_one)]")`

`initial_cond_calc.f90`: add one particle, with mass `xmass` per species
(`xmass1(i, 1:nspec)`), to `init_cond` if it is at time `itime`
(otherwise do nothing, as upstream returns at once).

`density` is required for [`InitCondUnit::Mass`]; its `rho`, `height`,
`nz` and `memind[1]` (upstream's `memind(2)`) are used.

# Errors

See [`InitCondError`].

```rust
pub fn initial_cond_calc(itime: i64, particle: &InitCondParticle, xmass: &[f64], set: &InitCondSettings, density: Option<&crate::flexpart::particle_average::GriddedMet>, domain: &crate::flexpart::concentration::OutputDomain, init_cond: &mut crate::flexpart::concentration::ConcentrationGrid) -> Result<(), InitCondError> { /* ... */ }
```

## Module `interpolation`

Interpolation of the gridded meteorology to a particle position: bilinear
in the horizontal, linear in height, linear in time between the two wind
fields held in memory, plus the sub-grid standard deviation of the corner
values that FLEXPART uses for its mesoscale-turbulence term.

# Two structs, mirroring upstream's two modules

* [`MetFields`] is the slice of `com_mod` these routines read: one grid
  (the mother grid or one nest) at the two time levels in memory.
* [`Interpolator`] is `interpol_mod`: the per-particle interpolation state
  (corner indices, bilinear weights, time weights, vertical profiles and
  the "already interpolated" flags). Upstream shares it between
  `advance.f90` and the `interpol_*` routines through the module; the port
  passes it explicitly, and the routines read and write the same fields in
  the same order.

# Nests and the polar grids

Each `interpol_*_nests.f90` routine is the mother-grid routine reading the
nest arrays `uun`, `vvn`, ... instead of `uu`, `vv`, ... . The arithmetic
is the same, and only the order in which independent accumulators are
updated differs, so one Rust function serves both: pass the nest's
[`MetFields`]. The code-to-code test checks each port against **both**
upstream versions.

On the polar grids (`ngrid < 0`), the horizontal wind is read from the
polar-stereographic components `uupol`, `vvpol` instead of `uu`, `vv`;
everything else is identical.

# Index conventions

Grid indices `ix`, `jy` are 0-based, as upstream's `0:nxmax-1` arrays are.
**Level indices are 0-based here** (upstream's level `k` is `k-1`), as are
the two memory slots (`memind`) and species.

# Units

FLEXPART's: winds m/s (`ww` in m/s after `verttransform`), heights m,
times s, density kg/m³.

```rust
pub mod interpolation { /* ... */ }
```

### Types

#### Struct `MetFields`

The meteorological fields of one grid at the two time levels in memory.

Three-dimensional fields are stored `[slot][k][j][i]` (`i` fastest), two-
dimensional ones `[slot][j][i]`, and `vdep` `[slot][species][j][i]`; use
[`MetFields::idx3`] / [`MetFields::idx2`] / [`MetFields::idx_vdep`].

```rust
pub struct MetFields {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub nspec: usize,
    pub height: Vec<f64>,
    pub memtime: [i64; 2],
    pub memind: [usize; 2],
    pub uu: Vec<f64>,
    pub vv: Vec<f64>,
    pub ww: Vec<f64>,
    pub uupol: Vec<f64>,
    pub vvpol: Vec<f64>,
    pub rho: Vec<f64>,
    pub drhodz: Vec<f64>,
    pub tt: Vec<f64>,
    pub ustar: Vec<f64>,
    pub wstar: Vec<f64>,
    pub oli: Vec<f64>,
    pub hmix: Vec<f64>,
    pub tropopause: Vec<f64>,
    pub vdep: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Grid points in x (`nx`). |
| `ny` | `usize` | Grid points in y (`ny`). |
| `nz` | `usize` | Model levels (`nz`). |
| `nspec` | `usize` | Number of species carried in `vdep`. |
| `height` | `Vec<f64>` | Height of each model level above ground, m (`height`). |
| `memtime` | `[i64; 2]` | Validity times of the two fields, s (`memtime`). |
| `memind` | `[usize; 2]` | Which storage slot holds the earlier and the later field (`memind`). |
| `uu` | `Vec<f64>` | Horizontal wind, m/s. |
| `vv` | `Vec<f64>` | Horizontal wind, m/s. |
| `ww` | `Vec<f64>` | Vertical wind, m/s. |
| `uupol` | `Vec<f64>` | Polar-stereographic wind components, m/s (mother grid only). |
| `vvpol` | `Vec<f64>` | Polar-stereographic wind components, m/s (mother grid only). |
| `rho` | `Vec<f64>` | Air density, kg/m³. |
| `drhodz` | `Vec<f64>` | Vertical density gradient, kg/m⁴. |
| `tt` | `Vec<f64>` | Temperature, K (read by `get_settling` through `advance`). |
| `ustar` | `Vec<f64>` | Friction velocity, m/s. |
| `wstar` | `Vec<f64>` | Convective velocity scale, m/s. |
| `oli` | `Vec<f64>` | Inverse Obukhov length, 1/m. |
| `hmix` | `Vec<f64>` | Mixing height, m. |
| `tropopause` | `Vec<f64>` | Tropopause height, m. |
| `vdep` | `Vec<f64>` | Dry deposition velocity per species, m/s. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(nx: usize, ny: usize, nz: usize, nspec: usize) -> Self { /* ... */ }
  ```
  A grid of the given size with every field zero.

- ```rust
  pub fn idx3(self: &Self, i: usize, j: usize, k: usize, slot: usize) -> usize { /* ... */ }
  ```
  Index of `(i, j, k, slot)` in a three-dimensional field.

- ```rust
  pub fn idx2(self: &Self, i: usize, j: usize, slot: usize) -> usize { /* ... */ }
  ```
  Index of `(i, j, slot)` in a two-dimensional field.

- ```rust
  pub fn idx_vdep(self: &Self, i: usize, j: usize, species: usize, slot: usize) -> usize { /* ... */ }
  ```
  Index of `(i, j, species, slot)` in `vdep`.

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
    fn clone(self: &Self) -> MetFields { /* ... */ }
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
    fn default() -> MetFields { /* ... */ }
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
    fn eq(self: &Self, other: &MetFields) -> bool { /* ... */ }
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
#### Struct `Interpolator`

`interpol_mod`: the per-particle interpolation state.

```rust
pub struct Interpolator {
    pub ix: usize,
    pub jy: usize,
    pub ixp: usize,
    pub jyp: usize,
    pub ngrid: i32,
    pub p1: f64,
    pub p2: f64,
    pub p3: f64,
    pub p4: f64,
    pub ddx: f64,
    pub ddy: f64,
    pub rddx: f64,
    pub rddy: f64,
    pub dt1: f64,
    pub dt2: f64,
    pub dtt: f64,
    pub indz: usize,
    pub indzp: usize,
    pub depoindicator: Vec<bool>,
    pub indzindicator: Vec<bool>,
    pub uprof: Vec<f64>,
    pub vprof: Vec<f64>,
    pub wprof: Vec<f64>,
    pub usigprof: Vec<f64>,
    pub vsigprof: Vec<f64>,
    pub wsigprof: Vec<f64>,
    pub rhoprof: Vec<f64>,
    pub rhogradprof: Vec<f64>,
    pub u: f64,
    pub v: f64,
    pub w: f64,
    pub usig: f64,
    pub vsig: f64,
    pub wsig: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ix` | `usize` | Lower-left corner of the cell, 0-based. |
| `jy` | `usize` | Lower-left corner of the cell, 0-based. |
| `ixp` | `usize` | Upper-right corner of the cell. |
| `jyp` | `usize` | Upper-right corner of the cell. |
| `ngrid` | `i32` | Which grid: `> 0` nest, `0` mother, `-1`/`-2` north/south polar. |
| `p1` | `f64` | Bilinear weights of the four corners. |
| `p2` | `f64` | Bilinear weights of the four corners. |
| `p3` | `f64` | Bilinear weights of the four corners. |
| `p4` | `f64` | Bilinear weights of the four corners. |
| `ddx` | `f64` | Fractional position in the cell. |
| `ddy` | `f64` | Fractional position in the cell. |
| `rddx` | `f64` | `1 - ddx`. |
| `rddy` | `f64` | `1 - ddy`. |
| `dt1` | `f64` | Time since the earlier field, s. |
| `dt2` | `f64` | Time to the later field, s. |
| `dtt` | `f64` | `1 / (dt1 + dt2)`. |
| `indz` | `usize` | Level below the particle, 0-based. |
| `indzp` | `usize` | Level above the particle, 0-based. |
| `depoindicator` | `Vec<bool>` | `true` while a species' deposition velocity is still to be<br>interpolated in this time step. |
| `indzindicator` | `Vec<bool>` | `true` while a level's profile is still to be interpolated. |
| `uprof` | `Vec<f64>` | Interpolated profiles, per level. |
| `vprof` | `Vec<f64>` | Interpolated profiles, per level. |
| `wprof` | `Vec<f64>` | Interpolated profiles, per level. |
| `usigprof` | `Vec<f64>` | Sub-grid standard deviations, per level. |
| `vsigprof` | `Vec<f64>` | Sub-grid standard deviations, per level. |
| `wsigprof` | `Vec<f64>` | Sub-grid standard deviations, per level. |
| `rhoprof` | `Vec<f64>` | Density and its gradient, per level. |
| `rhogradprof` | `Vec<f64>` | Density and its gradient, per level. |
| `u` | `f64` | Wind at the particle, m/s. |
| `v` | `f64` | Wind at the particle, m/s. |
| `w` | `f64` | Wind at the particle, m/s. |
| `usig` | `f64` | Sub-grid standard deviations at the particle, m/s. |
| `vsig` | `f64` | Sub-grid standard deviations at the particle, m/s. |
| `wsig` | `f64` | Sub-grid standard deviations at the particle, m/s. |

##### Implementations

###### Methods

- ```rust
  pub fn new(nz: usize, nspec: usize) -> Self { /* ... */ }
  ```
  A state for a grid with `nz` levels and `nspec` species, as upstream's

- ```rust
  pub fn interpol_all(self: &mut Self, met: &MetFields, itime: i64, xt: f64, yt: f64, zt: f64) -> Option<SurfaceScales> { /* ... */ }
  ```
  `interpol_all.f90` / `interpol_all_nests.f90`: everything at the two

- ```rust
  pub fn interpol_misslev(self: &mut Self, met: &MetFields, n: usize) { /* ... */ }
  ```
  `interpol_misslev.f90` / `interpol_misslev_nests.f90`: the profiles at

- ```rust
  pub fn interpol_wind(self: &mut Self, met: &MetFields, itime: i64, xt: f64, yt: f64, zt: f64) -> Option<()> { /* ... */ }
  ```
  `interpol_wind.f90` / `interpol_wind_nests.f90`: wind and its sub-grid

- ```rust
  pub fn interpol_wind_short(self: &mut Self, met: &MetFields, itime: i64, xt: f64, yt: f64, zt: f64) -> Option<()> { /* ... */ }
  ```
  `interpol_wind_short.f90` / `interpol_wind_short_nests.f90`: as

- ```rust
  pub fn interpol_vdep(self: &mut Self, met: &MetFields, level: usize) -> f64 { /* ... */ }
  ```
  `interpol_vdep.f90` / `interpol_vdep_nests.f90`: deposition velocity

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
    fn clone(self: &Self) -> Interpolator { /* ... */ }
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
    fn default() -> Interpolator { /* ... */ }
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
    fn eq(self: &Self, other: &Interpolator) -> bool { /* ... */ }
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
#### Struct `SurfaceScales`

Surface scales interpolated by [`Interpolator::interpol_all`] into
upstream's `hanna_mod`.

```rust
pub struct SurfaceScales {
    pub ust: f64,
    pub wst: f64,
    pub ol: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ust` | `f64` | Friction velocity, m/s. |
| `wst` | `f64` | Convective velocity scale, m/s. |
| `ol` | `f64` | Obukhov length, m (`99999` where the interpolated `1/L` is exactly 0). |

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
    fn clone(self: &Self) -> SurfaceScales { /* ... */ }
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
    fn eq(self: &Self, other: &SurfaceScales) -> bool { /* ... */ }
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
## Module `landuse`

Landuse fractions of the 13 FLEXPART classes at every grid point, from a
0.3° landuse inventory (`assignland.f90`).

The inventory (IGBP-DIS DISCover, Belward et al. 1999, as packed in
FLEXPART's `IGBP_int1.dat` and unpacked by `readlanduse.f90`) is **data**
and is not shipped: [`LanduseInventory`] holds, for each of the
1200 x 600 cells, the three most abundant classes and their weights.

# Algorithm (upstream's)

1. Per inventory cell, class `k_li` gets `p_li / (p_1 + p_2 + p_3)` (0 if
   the weights sum to 0).
2. Per grid point, the 10 x 10 sample points
   `(ix + (iix-1)/10, jy + (jjy-1)/10)` are mapped to inventory cells and
   their class fractions summed.
3. If anything was found: divide by 100, and renormalise to 1 if the sum
   is below `1 - 1e-5`. Otherwise use the land-sea mask: class 3 (ocean)
   if `lsm < 0.1`, else class 7.

# Upstream quirks reproduced

- **Duplicate classes overwrite.** If two of a cell's three entries name
  the same class, the later weight **replaces** the earlier one rather
  than adding to it, so that cell's fractions sum to less than 1.
- **The samples are not centred on the grid point**: they cover
  `[ix, ix + 0.9] x [jy, jy + 0.9]` grid lengths, i.e. the cell to the
  north-east.
- **Latitude wraps.** A sample at or north of 90° maps to inventory row
  601+ and is moved to row `yj - 600`, i.e. next to the **South Pole**.
  On a global grid the top row's samples at 90.1°–90.9° therefore average
  Antarctic landuse. (Exactly 90° itself also maps to row 601 in double
  precision, `180/0.3 = 600`.)
- Mother grid only: a longitude at or beyond 180° is wrapped once by
  `-360`, and a cell index below 0 stops the program. The nest loop has
  neither (its longitudes beyond 180° are wrapped through the cell index
  instead, which is equivalent up to rounding).
- Upstream's running `sumperc` adds the accumulated value, not the
  increment; it is only tested for `> 0`. Reproduced literally.

Cases where upstream indexes outside its arrays (a class number outside
`1..=13`, an inventory cell index of 0 or below) are refused with
[`LanduseError`] instead of guessed.

```rust
pub mod landuse { /* ... */ }
```

### Types

#### Struct `LanduseInventory`

The landuse inventory as `readlanduse.f90` leaves it in `landinvent`.
Cell `(i, j)` (1-based, `i` along longitude from 180°W, `j` along
latitude from 90°S) is element `(i-1) + 1200*(j-1)`.

```rust
pub struct LanduseInventory {
    pub classes: Vec<[i8; 3]>,
    pub weights: Vec<[i8; 3]>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `classes` | `Vec<[i8; 3]>` | The three class numbers of each cell (`landinvent(i,j,1:3)`). |
| `weights` | `Vec<[i8; 3]>` | Their weights (`landinvent(i,j,4:6)`), 0–15 as unpacked upstream. |

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
    fn clone(self: &Self) -> LanduseInventory { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LanduseInventory) -> bool { /* ... */ }
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
#### Enum `LanduseGrid`

Mother grid or nest (`assignland` treats them differently, see the module
docs).

```rust
pub enum LanduseGrid {
    Mother,
    Nest,
}
```

##### Variants

###### `Mother`

The mother-grid loop.

###### `Nest`

The nest loop.

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
    fn clone(self: &Self) -> LanduseGrid { /* ... */ }
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
    fn eq(self: &Self, other: &LanduseGrid) -> bool { /* ... */ }
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
#### Enum `LanduseError`

Why [`assignland`] refused.

```rust
pub enum LanduseError {
    SamplingStop,
    CellOutOfRange,
    ClassOutOfRange,
    ShapeMismatch,
}
```

##### Variants

###### `SamplingStop`

A sample maps to a longitude cell below 0: upstream prints
"problem with landuseinv sampling" and stops (mother grid).

###### `CellOutOfRange`

A sample maps to inventory cell 0 (or below) in either direction,
which upstream reads outside its array.

###### `ClassOutOfRange`

An inventory class number outside `1..=13`, written outside the array
upstream.

###### `ShapeMismatch`

The inventory or the land-sea mask has the wrong size.

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
    fn clone(self: &Self) -> LanduseError { /* ... */ }
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
    fn eq(self: &Self, other: &LanduseError) -> bool { /* ... */ }
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

#### Function `assignland`

`assignland.f90`: landuse fractions `xlanduse(ix, jy, 1:13)` (or
`xlandusen`) for every point of a grid; index `ix + nx*jy`.

`lsm` is the land-sea mask of the grid (fraction land).

# Errors
See [`LanduseError`].

```rust
pub fn assignland(inv: &LanduseInventory, geom: &super::met_fields::GridGeometry, lsm: &super::met_fields::Field2, kind: LanduseGrid) -> Result<Vec<[f64; 13]>, LanduseError> { /* ... */ }
```

### Constants and Statics

#### Constant `LUMAXX`

Inventory cells in longitude (`lumaxx`).

```rust
pub const LUMAXX: usize = 1200;
```

#### Constant `LUMAXY`

Inventory cells in latitude (`lumaxy`).

```rust
pub const LUMAXY: usize = 600;
```

#### Constant `DXLU`

Inventory resolution, degrees (`dxlu`).

```rust
pub const DXLU: f64 = 0.3;
```

#### Constant `XLON0LU`

Inventory origin longitude, degrees (`xlon0lu`).

```rust
pub const XLON0LU: i64 = -180;
```

#### Constant `YLAT0LU`

Inventory origin latitude, degrees (`ylat0lu`).

```rust
pub const YLAT0LU: i64 = -90;
```

## Module `met_fields`

Per-grid-point boundary-layer parameters (`calcpar`) and potential
vorticity on the 3-d grid (`calcpv`).

| Function | Upstream | Computes |
|---|---|---|
| [`calcpar`] | `calcpar.f90`, `calcpar_nests.f90` | `u*`, `1/L`, mixing height, `w*`, dry deposition velocity, thermal tropopause, then PV |
| [`calcpv`] | `calcpv.f90`, `calcpv_nests.f90` | Ertel potential vorticity on the model levels, PVU |

# One function per routine pair

`calcpar_nests.f90` and `calcpv_nests.f90` are copies of the mother-grid
routines on the nest arrays. The port has one function each that takes the
grid as an argument, and the code-to-code test drives it against **both**
upstream versions. The copies differ in exactly these ways, all
reproduced through arguments:

- `calcpar_nests` has **no NCEP branch**: it always calls `obukhov` with
  `tthn(.,.,2)` and a never-assigned `dummyakzllev`, always lets
  `richardson` write the mixing height and always starts the level-height
  loop at 2. For ECMWF input that is identical to `calcpar`'s ECMWF branch.
  For NCEP input `obukhov` would read the undefined `dummyakzllev`, so
  [`calcpar`] **refuses** [`GridKind::Nest`] with
  [`MetDataFormat::Ncep`] ([`MetFieldsError::NestWithNcep`]).
- `calcpar_nests` calls `getvdep_nests`, which computes its latitude as
  `jy*dy + ylat0` — the **mother grid's** `dy` and `ylat0` with the
  **nest's** row index. That is an upstream defect (a nest at 30–32°N on a
  mother grid starting at 60°S is given southern-hemisphere seasons); the
  port reproduces it by taking the season latitude origin and spacing as
  separate arguments ([`DryDepositionInput::season_lat0`],
  [`DryDepositionInput::season_dy`]). A caller porting a nest passes the
  mother grid's values, as upstream does.
- `calcpv_nests` has no global-domain branches: pass
  [`GlobalDomain::LIMITED`].

# Upstream quirks reproduced (and documented)

- **`L` passed to `getvdep` is `1/(1/L)`**, not `L`: upstream stores
  `oli = 1/ol` and calls `getvdep(..., 1./oli, ...)`. The double reciprocal
  is kept (it can differ from `ol` in the last bit).
- **`z0(7)` is overwritten in `com_mod`** at every grid point with the
  Charnock water roughness `0.016 u*^2 / g`; the port passes the modified
  table to `getvdep` and reports the last value written
  ([`CalcparOutput::z0_water_last`]).
- **`ustar` is floored at `1e-8`** before everything else.
- **Tropopause left unassigned.** If no layer satisfies the thermal
  criterion, `tropopause(ix,jy)` keeps whatever it held before the call
  (the previous meteorological field). The port returns `None` there.
- **Stale `kzmin`.** `kzmin` (the lowest level searched for the
  tropopause) is assigned only when some level reaches the minimum
  altitude `altmin`. Otherwise upstream reuses the value from the
  **previous column** of the same call. The port carries it across columns
  in upstream's loop order (`jy` outer, `ix` inner); if the first column
  has none, the value is undefined and the port returns `None` for that
  column's tropopause.
- **`zlev(1)` is never assigned in the ECMWF branch** (the height loop
  starts at level 2) but the `kzmin` search starts at level 1, so upstream
  compares an uninitialised stack value with `altmin`. The port starts the
  search at level 2. That equals upstream whenever the stale value is below
  `altmin` (>= 2500 m) or NaN — e.g. zero; it was so in the reference run,
  where the port matches upstream on every tropopause. A different stack
  history could make upstream pick level 1; nothing in FLEXPART guards it.
- **NCEP `llev`.** `llev` is one above the last level whose pressure
  exceeds `ps`, set to `nuvz-1` if that is above the top.

# Units

Bare `f64` in FLEXPART units: Pa, K, kg/kg, m/s, W/m², mm/h, m; PV in PVU
(`1e-6 K m² kg⁻¹ s⁻¹`), sign as upstream (positive in the northern
hemisphere for a stable atmosphere).

```rust
pub mod met_fields { /* ... */ }
```

### Types

#### Struct `Field2`

A 2-d field on the horizontal grid, stored as upstream does (`ix`
fastest): element `(ix, jy)` is `data[ix + nx*jy]`, both 0-based like
upstream's `0:nxmax-1` bounds.

```rust
pub struct Field2 {
    pub nx: usize,
    pub ny: usize,
    pub data: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Points in x. |
| `ny` | `usize` | Points in y. |
| `data` | `Vec<f64>` | Values, `ix` fastest. |

##### Implementations

###### Methods

- ```rust
  pub fn filled(nx: usize, ny: usize, v: f64) -> Self { /* ... */ }
  ```
  A field filled with `v`.

- ```rust
  pub fn at(self: &Self, ix: usize, jy: usize) -> f64 { /* ... */ }
  ```
  Value at `(ix, jy)`.

- ```rust
  pub fn set(self: &mut Self, ix: usize, jy: usize, v: f64) { /* ... */ }
  ```
  Set the value at `(ix, jy)`.

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
    fn clone(self: &Self) -> Field2 { /* ... */ }
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
    fn eq(self: &Self, other: &Field2) -> bool { /* ... */ }
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
#### Struct `Field3`

A 3-d field: element `(ix, jy, k)` is `data[ix + nx*(jy + ny*k)]`, with
`k` 0-based (upstream's level `k` is index `k-1`).

```rust
pub struct Field3 {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub data: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Points in x. |
| `ny` | `usize` | Points in y. |
| `nz` | `usize` | Levels. |
| `data` | `Vec<f64>` | Values, `ix` fastest, then `jy`, then `k`. |

##### Implementations

###### Methods

- ```rust
  pub fn filled(nx: usize, ny: usize, nz: usize, v: f64) -> Self { /* ... */ }
  ```
  A field filled with `v`.

- ```rust
  pub fn at(self: &Self, ix: usize, jy: usize, k: usize) -> f64 { /* ... */ }
  ```
  Value at `(ix, jy, k)`, `k` 0-based.

- ```rust
  pub fn set(self: &mut Self, ix: usize, jy: usize, k: usize, v: f64) { /* ... */ }
  ```
  Set the value at `(ix, jy, k)`, `k` 0-based.

- ```rust
  pub fn column(self: &Self, ix: usize, jy: usize) -> Vec<f64> { /* ... */ }
  ```
  The column at `(ix, jy)`, levels 0-based.

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
    fn clone(self: &Self) -> Field3 { /* ... */ }
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
    fn eq(self: &Self, other: &Field3) -> bool { /* ... */ }
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
#### Struct `GridGeometry`

Horizontal geometry of a meteorological grid (mother or nest).

```rust
pub struct GridGeometry {
    pub nx: usize,
    pub ny: usize,
    pub dx: f64,
    pub dy: f64,
    pub xlon0: f64,
    pub ylat0: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Points in x (`nx` / `nxn(l)`). |
| `ny` | `usize` | Points in y (`ny` / `nyn(l)`). |
| `dx` | `f64` | Grid spacing in x, degrees. |
| `dy` | `f64` | Grid spacing in y, degrees. |
| `xlon0` | `f64` | Longitude of the lower-left point, degrees. |
| `ylat0` | `f64` | Latitude of the lower-left point, degrees. |

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
    fn clone(self: &Self) -> GridGeometry { /* ... */ }
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
    fn eq(self: &Self, other: &GridGeometry) -> bool { /* ... */ }
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
#### Struct `GlobalDomain`

Which global-domain special cases [`calcpv`] applies (`xglobal`,
`sglobal`, `nglobal` in `com_mod`).

```rust
pub struct GlobalDomain {
    pub xglobal: bool,
    pub sglobal: bool,
    pub nglobal: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xglobal` | `bool` | The grid wraps in longitude; column `nx-1` duplicates column 0, so the<br>west neighbour of column 0 is column `nx-2`. |
| `sglobal` | `bool` | Row 0 is the South Pole. |
| `nglobal` | `bool` | Row `ny-1` is the North Pole. |

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
    fn clone(self: &Self) -> GlobalDomain { /* ... */ }
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
    fn eq(self: &Self, other: &GlobalDomain) -> bool { /* ... */ }
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
#### Enum `GridKind`

Mother grid or nest (selects `calcpar` or `calcpar_nests` behaviour).

```rust
pub enum GridKind {
    Mother,
    Nest,
}
```

##### Variants

###### `Mother`

`calcpar.f90`.

###### `Nest`

`calcpar_nests.f90` (ECMWF only, see the module docs).

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
    fn clone(self: &Self) -> GridKind { /* ... */ }
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
    fn eq(self: &Self, other: &GridKind) -> bool { /* ... */ }
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
#### Enum `MetFieldsError`

Why [`calcpar`] / [`calcpv`] refused.

```rust
pub enum MetFieldsError {
    NestWithNcep,
    DegenerateStencil,
    ShapeMismatch,
}
```

##### Variants

###### `NestWithNcep`

`calcpar_nests` with NCEP input reads the never-assigned
`dummyakzllev`; upstream's result is undefined.

###### `DegenerateStencil`

A finite-difference stencil in [`calcpv`] has fewer than two points
(`nx < 2`, `ny < 2`, or a 3-row grid that is both `sglobal` and
`nglobal`); upstream then reads an unassigned `vx(2)` / `uy(2)`.

###### `ShapeMismatch`

Field dimensions disagree with the grid geometry.

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
    fn clone(self: &Self) -> MetFieldsError { /* ... */ }
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
    fn eq(self: &Self, other: &MetFieldsError) -> bool { /* ... */ }
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
#### Struct `CalcparInput`

The inputs `calcpar` reads from `com_mod` at one time index `n`.

```rust
pub struct CalcparInput {
    pub ps: Field2,
    pub tt2: Field2,
    pub td2: Field2,
    pub surfstr: Field2,
    pub sshf: Field2,
    pub ssr: Field2,
    pub lsprec: Field2,
    pub convprec: Field2,
    pub sd: Field2,
    pub excessoro: Field2,
    pub hmix_in: Field2,
    pub tth: Field3,
    pub qvh: Field3,
    pub uuh: Field3,
    pub vvh: Field3,
    pub akz: Vec<f64>,
    pub bkz: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ps` | `Field2` | Surface pressure, Pa (`ps`). |
| `tt2` | `Field2` | 2 m temperature, K (`tt2`). |
| `td2` | `Field2` | 2 m dew point, K (`td2`). |
| `surfstr` | `Field2` | Surface stress, N/m² (`surfstr`). |
| `sshf` | `Field2` | Surface sensible heat flux, W/m² (`sshf`; negative = upward). |
| `ssr` | `Field2` | Surface solar radiation, W/m² (`ssr`). |
| `lsprec` | `Field2` | Large-scale precipitation, mm/h (`lsprec`). |
| `convprec` | `Field2` | Convective precipitation, mm/h (`convprec`). |
| `sd` | `Field2` | Snow depth, m water equivalent (`sd`). |
| `excessoro` | `Field2` | Sub-grid orography excess, m (`excessoro`). |
| `hmix_in` | `Field2` | Mixing height read by `readwind` for NCEP input, m (`hmix` on entry).<br>Ignored for ECMWF, where `richardson` computes it. |
| `tth` | `Field3` | Temperature on the model levels, K (`tth`, `nuvz` levels). |
| `qvh` | `Field3` | Specific humidity on the model levels, kg/kg (`qvh`). |
| `uuh` | `Field3` | Wind components on the model levels, m/s (`uuh`, `vvh`). |
| `vvh` | `Field3` | See [`CalcparInput::uuh`]. |
| `akz` | `Vec<f64>` | Hybrid coefficient `A` of each model level, Pa (`akz`). |
| `bkz` | `Vec<f64>` | Hybrid coefficient `B` of each model level (`bkz`). |

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
    fn clone(self: &Self) -> CalcparInput { /* ... */ }
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
    fn eq(self: &Self, other: &CalcparInput) -> bool { /* ... */ }
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
#### Struct `DryDepositionInput`

Dry deposition inputs (`DRYDEP` true). Upstream reads all of it from
`com_mod`; the Wesely tables are data, never shipped here.

```rust
pub struct DryDepositionInput {
    pub jul: f64,
    pub season_lat0: f64,
    pub season_dy: f64,
    pub landuse: Vec<[f64; 13]>,
    pub z0: [f64; 13],
    pub species: Vec<(super::dry_deposition::DepositionSpecies, [[super::dry_deposition::SurfaceResistances; 13]; 5])>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `jul` | `f64` | Julian date of the field, `bdate + wftime(n)/86400`. |
| `season_lat0` | `f64` | Latitude origin `getvdep` uses for the season, degrees: `ylat0` of the<br>**mother** grid, also for a nest (upstream defect, module docs). |
| `season_dy` | `f64` | Latitude spacing `getvdep` uses for the season, degrees: `dy` of the<br>mother grid, also for a nest. |
| `landuse` | `Vec<[f64; 13]>` | Landuse fractions of each column (`xlanduse(ix,jy,:)`), index<br>`ix + nx*jy`. |
| `z0` | `[f64; 13]` | Roughness length of each class, m (`z0`); class 7 (index 6) is<br>overwritten at every point. |
| `species` | `Vec<(super::dry_deposition::DepositionSpecies, [[super::dry_deposition::SurfaceResistances; 13]; 5])>` | Species, each with its Wesely table `[season][class]`. |

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
    fn clone(self: &Self) -> DryDepositionInput { /* ... */ }
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
    fn eq(self: &Self, other: &DryDepositionInput) -> bool { /* ... */ }
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
#### Struct `CalcparOutput`

What [`calcpar`] computes.

```rust
pub struct CalcparOutput {
    pub ustar: Field2,
    pub oli: Field2,
    pub hmix: Field2,
    pub wstar: Field2,
    pub vdep: Vec<Field2>,
    pub tropopause: Vec<Option<f64>>,
    pub z0_water_last: Option<f64>,
    pub pv: Field3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ustar` | `Field2` | Friction velocity, m/s (`ustar`). |
| `oli` | `Field2` | Inverse Obukhov length, 1/m (`oli`; `99999` when `L == 0`). |
| `hmix` | `Field2` | Mixing height, m (`hmix`), clamped to `[hmixmin, hmixmax]`. |
| `wstar` | `Field2` | Convective velocity scale, m/s (`wstar`). |
| `vdep` | `Vec<Field2>` | Dry deposition velocity per species, m/s (`vdep`); empty without<br>dry deposition. |
| `tropopause` | `Vec<Option<f64>>` | Thermal tropopause height, m, index `ix + nx*jy`; `None` where<br>upstream leaves it unassigned (see the module docs). |
| `z0_water_last` | `Option<f64>` | The last value written to `z0(7)` (`None` without dry deposition). |
| `pv` | `Field3` | Potential vorticity from [`calcpv`], PVU. |

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
    fn clone(self: &Self) -> CalcparOutput { /* ... */ }
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
    fn eq(self: &Self, other: &CalcparOutput) -> bool { /* ... */ }
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
#### Struct `CalcparOptions`

Options selecting upstream's branches.

```rust
pub struct CalcparOptions {
    pub format: super::surface_layer::MetDataFormat,
    pub kind: GridKind,
    pub lsubgrid: bool,
    pub domain: GlobalDomain,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `format` | `super::surface_layer::MetDataFormat` | ECMWF (with `akm`, `bkm` of the lowest two half levels) or NCEP. |
| `kind` | `GridKind` | Mother grid or nest. |
| `lsubgrid` | `bool` | `lsubgrid == 1`: add `min(excessoro, hmixplus)` to the mixing height. |
| `domain` | `GlobalDomain` | Global-domain flags for [`calcpv`] (nests: [`GlobalDomain::LIMITED`]). |

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
    fn clone(self: &Self) -> CalcparOptions { /* ... */ }
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
    fn eq(self: &Self, other: &CalcparOptions) -> bool { /* ... */ }
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

#### Function `calcpar`

`calcpar.f90` / `calcpar_nests.f90`: boundary-layer parameters over the
whole grid at one time, followed by [`calcpv`].

Per column, in upstream's order: `u* = scalev(...)` floored at `1e-8`;
`L = obukhov(...)` and `oli = 1/L` (or `99999` for `L == 0`);
`richardson` for `h`, `w*` and `hmixplus` (for NCEP `h` is the value read
in, [`CalcparInput::hmix_in`]); `h += min(excessoro, hmixplus)` when
`lsubgrid`; `h` clamped to `[100, 4500]` m; the dry deposition velocity
with `z0(7) = 0.016 u*^2/g` and `rh = e(Td)/e(T)`; and the thermal
tropopause of Hoinka (1997): the first level above `altmin` (5000 m in
the tropics, 2500 m poleward of 40°, linear between) from which the
temperature falls by less than 2 K/km over the next 2000 m.

See the module docs for every upstream quirk this reproduces.

# Errors
[`MetFieldsError::NestWithNcep`], [`MetFieldsError::ShapeMismatch`], and
those of [`calcpv`].

```rust
pub fn calcpar(geom: &GridGeometry, opts: &CalcparOptions, input: &CalcparInput, drydep: Option<&DryDepositionInput>) -> Result<CalcparOutput, MetFieldsError> { /* ... */ }
```

#### Function `calcpv`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`calcpv.f90` / `calcpv_nests.f90`: Ertel potential vorticity on the model
levels, PVU,

```text
  PV = -g dtheta/dp (f + (dv/dx / cos(phi) - du/dy + u tan(phi)) / R) * 1e6
```

with the horizontal derivatives taken **along the potential-temperature
surface**: in each neighbouring column the wind is interpolated to the
level where `theta` equals the current point's (a spiral search of at most
`nuvz/3` layers, see `theta_search`). Where no such level is found the
current point's own wind is used and the gap shrinks by one grid length;
where both sides fail the derivative falls back to the same-level
difference.

# Upstream behaviour reproduced
- `theta = T (100000/p)^kappa` with `kappa = 0.286`; `dtheta/dp` is
  centred, one-sided at the bottom and top level.
- Coriolis `f = 0.00014585 sin(phi)` (upstream's literal), `pi` is
  `par_mod`'s `3.14159265`, `R = 6.371e6 m`, `g = 9.81` (literal).
- A bracket whose two levels and `theta` all agree within `1e-5` K takes
  the plain average of the two winds ("avoid division by zero").
- `xglobal`: the west neighbour of column 0 is column `nx-2` (column
  `nx-1` duplicates column 0), the east neighbour of `nx-1` is column 1.
- `sglobal` / `nglobal`: the pole rows are skipped, the next row in
  uses a one-sided difference, and the pole row is then filled with the
  mean of that next row (summed in `ix` order, divided by `nx`).

# Errors
[`MetFieldsError::DegenerateStencil`] where upstream would read an
unassigned stencil value; [`MetFieldsError::ShapeMismatch`].

```rust
pub fn calcpv(geom: &GridGeometry, domain: GlobalDomain, akz: &[f64], bkz: &[f64], ps: &Field2, tth: &Field3, uuh: &Field3, vvh: &Field3) -> Result<Field3, MetFieldsError> { /* ... */ }
```

## Module `oh_chemistry`

OH reaction: hourly OH fields scaled from a monthly climatology by the
O(¹D) photolysis rate, and the first-order loss of particle mass.

| Function | Upstream | Computes |
|---|---|---|
| [`gethourly_oh`] | `gethourlyOH.f90` | the two hourly OH fields bracketing `itime` |
| [`ohreaction`] | `ohreaction.f90` | particle mass after reaction with OH |

The climatology (`OH_field`, `jrate_average`, the coordinate vectors) is
**data**, read upstream from `OH_variables.bin` by `readOHfield.f90`; the
port takes it as arguments ([`OhClimatology`], [`JrateClimatology`]) and
ships none. The solar zenith angle and `J(O1D)` come from the already
verified [`super::solar`]; the month from [`super::calendar::caldate`].

# Upstream quirks reproduced

- **`gethourlyOH` advances by one hour per call at most.** If `itime` has
  moved more than an hour past the second field, it shifts the pair by one
  hour only, so the fields lag the model time until enough calls have
  been made. When neither "in range" nor "advance" applies (first call:
  `memOHtime = (0, 0)`), it rebuilds both fields for **`t = 0` and
  `t = ldirect*3600`**, whatever `itime` is.
- `gethourlyOH`'s initial second field is at `bdate + ldirect*real(1./24.,
  kind=dp)`: the `1./24.` is a default real, so the shipped real(4) build
  places it `1.3e-9` days (0.1 ms) late; the port uses the `f64` value of
  the real(8) build.
- `memOHtime` is default `real`: in the shipped build the hour arithmetic
  and the `itime` comparisons are single precision (exact for integral
  hours below 2^24 s, ~194 days; spacing grows beyond).
- **`ohreaction` reacts every particle**, with no check of `itra1`: an
  inactive (`-999999999`) or not yet released particle loses mass too.
- **`ohreaction` indexes the temperature with the time-order index `n`
  (1 or 2) as the storage slot**, not with `memind(n)` as `get_wetscav`
  does. When `memind` is swapped, it reads the other time's temperature.
  The port takes `tt` by storage slot ([`OhMet::tt_slots`]) and reproduces
  this.
- **On a nest, `ohreaction` computes `ix, jy` in nest coordinates and uses
  them to index the mother-grid `tt`.** Reproduced (the port refuses with
  [`OhError::OutsideGrid`] only where that leaves the array).
- **`altOHtop(i-1) = altOH(i) + (altOH(i) - altOH(i-1))/2`**: the "top" of
  level `i-1` is placed half a layer **above level `i`**, i.e. one level
  too high; the OH level is then the one whose shifted "top" is nearest
  the particle (`minloc`, first on ties).
- The month `m`, hour `h`, `ldeltat` and `ohreacted` that `ohreaction`
  computes are never used.

# Units

OH in molecules/cm³, rate constants as in the SPECIES file
(`k = C T^N exp(-D/T)` in cm³/molecule/s), temperatures K, times s.

```rust
pub mod oh_chemistry { /* ... */ }
```

### Types

#### Struct `OhClimatology`

Monthly OH climatology (`oh_mod`: `lonOH`, `latOH`, `altOH`, `OH_field`).

```rust
pub struct OhClimatology {
    pub lon: Vec<f64>,
    pub lat: Vec<f64>,
    pub alt: Vec<f64>,
    pub field: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lon` | `Vec<f64>` | Longitudes, degrees (`lonOH`, `nxOH` values). |
| `lat` | `Vec<f64>` | Latitudes, degrees (`latOH`). |
| `alt` | `Vec<f64>` | Level heights above orography, m (`altOH`). |
| `field` | `Vec<f64>` | `OH_field(ix, jy, kz, month)`, molecules/cm³, `ix` fastest:<br>index `ix + nx*(jy + ny*(kz + nz*(month-1)))`, all 0-based except month. |

##### Implementations

###### Methods

- ```rust
  pub fn index3(self: &Self, ix: usize, jy: usize, kz: usize) -> usize { /* ... */ }
  ```
  Flat index of `(ix, jy, kz)` (0-based) in one hourly field.

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
    fn clone(self: &Self) -> OhClimatology { /* ... */ }
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
    fn eq(self: &Self, other: &OhClimatology) -> bool { /* ... */ }
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
#### Struct `JrateClimatology`

Monthly mean `J(O1D)` on its own 360 x 180 grid (`oh_mod`: `lonjr`,
`latjr`, `jrate_average`).

```rust
pub struct JrateClimatology {
    pub lon: Vec<f64>,
    pub lat: Vec<f64>,
    pub average: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lon` | `Vec<f64>` | Longitudes, degrees (`lonjr`, 360 values). |
| `lat` | `Vec<f64>` | Latitudes, degrees (`latjr`, 180 values). |
| `average` | `Vec<f64>` | `jrate_average(i, j, month)`, 1/s: index `i + nlon*(j + nlat*(month-1))`. |

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
    fn clone(self: &Self) -> JrateClimatology { /* ... */ }
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
    fn eq(self: &Self, other: &JrateClimatology) -> bool { /* ... */ }
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
#### Struct `HourlyOh`

The two hourly OH fields in memory (`OH_hourly(:,:,:,1:2)`) and their
times (`memOHtime`, s relative to `bdate`). Start from
[`HourlyOh::empty`], which is upstream's zero-initialised module state.

```rust
pub struct HourlyOh {
    pub memtime: [f64; 2],
    pub hourly: [Vec<f64>; 2],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `memtime` | `[f64; 2]` | `memOHtime(1:2)`, s. |
| `hourly` | `[Vec<f64>; 2]` | The two fields, each indexed by [`OhClimatology::index3`]. |

##### Implementations

###### Methods

- ```rust
  pub fn empty(clim: &OhClimatology) -> Self { /* ... */ }
  ```
  No fields yet: `memOHtime = (0, 0)`.

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
    fn clone(self: &Self) -> HourlyOh { /* ... */ }
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
    fn eq(self: &Self, other: &HourlyOh) -> bool { /* ... */ }
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
#### Struct `OhSpecies`

OH rate constants of one species (`ohcconst`, `ohdconst`, `ohnconst`):
`k = C T^N exp(-D/T)`. A species with `C <= 0` does not react.

```rust
pub struct OhSpecies {
    pub ohcconst: f64,
    pub ohdconst: f64,
    pub ohnconst: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ohcconst` | `f64` | `C`, cm³/molecule/s. |
| `ohdconst` | `f64` | `D`, K. |
| `ohnconst` | `f64` | `N`, dimensionless. |

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
    fn clone(self: &Self) -> OhSpecies { /* ... */ }
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
    fn eq(self: &Self, other: &OhSpecies) -> bool { /* ... */ }
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
#### Struct `OhMet`

What `ohreaction` reads from `com_mod` besides the particles.

```rust
pub struct OhMet {
    pub memtime: [i64; 2],
    pub height: Vec<f64>,
    pub tt_slots: [super::met_fields::Field3; 2],
    pub dx: f64,
    pub dy: f64,
    pub xlon0: f64,
    pub ylat0: f64,
    pub nests: Vec<super::wet_deposition::NestFrame>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `memtime` | `[i64; 2]` | Validity times of the two met fields, s (`memtime`). |
| `height` | `Vec<f64>` | Model level heights, m (`height(1:nz)`). |
| `tt_slots` | `[super::met_fields::Field3; 2]` | Mother-grid temperature in **storage slots** 1 and 2 (`tt(:,:,:,1)`,<br>`tt(:,:,:,2)`), indexed by the time-order `n` as upstream does. |
| `dx` | `f64` | Mother grid spacing and origin, degrees (`dx`, `dy`, `xlon0`,<br>`ylat0`). |
| `dy` | `f64` | See [`OhMet::dx`]. |
| `xlon0` | `f64` | See [`OhMet::dx`]. |
| `ylat0` | `f64` | See [`OhMet::dx`]. |
| `nests` | `Vec<super::wet_deposition::NestFrame>` | Nest frames (only the cell index depends on them). |

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
    fn clone(self: &Self) -> OhMet { /* ... */ }
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
    fn eq(self: &Self, other: &OhMet) -> bool { /* ... */ }
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
#### Enum `OhError`

Why [`ohreaction`] refused.

```rust
pub enum OhError {
    AboveTopLevel,
    OutsideGrid,
}
```

##### Variants

###### `AboveTopLevel`

The particle is at or above the top model level while OH is present;
upstream's level index `indz` is then never assigned.

###### `OutsideGrid`

The temperature lookup falls outside the mother-grid array.

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
    fn clone(self: &Self) -> OhError { /* ... */ }
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
    fn eq(self: &Self, other: &OhError) -> bool { /* ... */ }
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

#### Function `gethourly_oh`

`gethourlyOH.f90`: make sure the hourly OH fields in `state` bracket
`itime` (s after `bdate`, Julian days), in the run direction `ldirect`
(`+1` / `-1`).

- In range (`ldirect*t1 <= ldirect*itime < ldirect*t2`): nothing.
- Past the second field (and `t2 != 0`): shift by **one** hour,
  `t1 = t2`, `t2 = t1 + ldirect*3600`, and compute the new second field.
- Otherwise: compute both, for `t = 0` and `t = ldirect*3600`.

See the module docs for the quirks this reproduces.

```rust
pub fn gethourly_oh(state: &mut HourlyOh, itime: i64, ldirect: i64, bdate: f64, clim: &OhClimatology, jr: &JrateClimatology) { /* ... */ }
```

#### Function `ohreaction`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`ohreaction.f90`: first-order loss of every particle's mass to OH over
`ltsample` (s) at `itime`.

Per particle: OH from the nearest climatology column (`minloc` on
longitude, wrapped to `(-180, 180]` once, and latitude) and the level by
upstream's shifted `altOHtop`, linearly interpolated in time between the
two hourly fields (and extrapolated outside them). If OH exceeds `tiny`,
each species with `C > 0` loses `m (1 - exp(-k OH |ltsample|))`, the
remaining mass set to 0 if not above `tiny`. `tiny` is
`f64::MIN_POSITIVE` (the real(8) build's `tiny(0.0)`).

See the module docs for the quirks this reproduces (no activity check,
time-order `tt` slot, nest indices into the mother grid).

# Errors
[`OhError::AboveTopLevel`], [`OhError::OutsideGrid`], each only where
upstream would actually read the undefined value. Particles before the
failing one have been updated.

```rust
pub fn ohreaction(itime: i64, ltsample: i64, particles: &mut [super::wet_deposition::ParticleRecord], species: &[OhSpecies], met: &OhMet, clim: &OhClimatology, hourly: &HourlyOh) -> Result<(), OhError> { /* ... */ }
```

## Module `outgrid`

Output-grid set-up (`outgrid_init.f90`, `outgrid_init_nest.f90`): the
surface area, volume and wall areas of every output cell, and the mean
model topography under each cell.

# What is ported, and what is not

Both routines do three things. The port carries the **numerics** of the
first two and leaves the third to the caller:

1. **Cell geometry** ([`cell_geometry`]): `area`, `volume`, `areaeast`,
   `areanorth` (`outgrid_init`) or `arean`, `volumen` (`outgrid_init_nest`).
   The nest routine's area and volume code is the mother routine's,
   textually, on the `*n` variables; it computes no wall areas (the nest
   has no flux output). One Rust function serves both; for a nest, ignore
   [`CellGeometry::areaeast`] / [`CellGeometry::areanorth`].
2. **Mean topography** ([`output_orography`]): `oroout` / `orooutn`, the
   average of 100 bilinear samples of `oro` (or of the innermost nest's
   `oron`) on a 10 x 10 lattice inside each output cell.
3. **Allocation and zeroing** of `gridunc`, `wetgridunc`, `drygridunc`,
   `flux`, `init_cond`, `creceptor` and the `concoutput` work arrays. Not
   ported: the Rust accumulators ([`ConcentrationGrid::zeros`],
   [`DepositionGrid::zeros`], [`FluxGrid::zeros`]) are created zeroed.

[`ConcentrationGrid::zeros`]: crate::flexpart::concentration::ConcentrationGrid::zeros
[`DepositionGrid::zeros`]: crate::flexpart::concentration::DepositionGrid::zeros
[`FluxGrid::zeros`]: crate::flexpart::fluxes::FluxGrid::zeros

# The cell area

Upstream uses the area of a spherical zone, `M = 2 pi R h dx / 360`
(Netz, *Formeln der Mathematik*, 5th ed. 1983, p. 90), where `h` is the
zone height `R |sin(lat_n) - sin(lat_s)|`. It evaluates `|sin|` as
`sqrt(1 - cos^2)` and picks the order of the difference from the cosines,
so the same expression serves both hemispheres. A cell that **straddles
the equator** (`lat_s < 0 < lat_n`, strictly) takes a different formula,
`h = dy R pi/180` — the arc length, not the zone height. That is a planar
approximation of the zone; for a 2.5° cell it overestimates the exact zone
area by 8e-5 relative.

# Upstream quirks reproduced (or refused)

* **`sqrt(1 - cos^2)` near the equator.** For a cell boundary at small
  `|lat|`, `1 - cos^2` cancels: at 0.25° it is `1.9e-5`, so `real(4)`
  loses about four of its seven digits there. The shipped build's areas of
  near-equator cells are correspondingly imprecise (the code-to-code test
  measures how much); the port, in `f64`, keeps about twelve digits.
* **Cells beyond a pole** are not guarded: `cos` changes sign past ±90°
  and `sqrt(1 - cos^2)` returns `|sin|`, so the area is wrong. Upstream's
  `readoutgrid` does not prevent such a grid; neither does the port.
* **`areanorth`** uses `cos` of the cell **centre** latitude, i.e. it is the
  area of a wall through the cell's middle, not of its northern face; and
  `areaeast` is the same for every cell. Both are reproduced as written
  (and match how [`calcfluxes`] counts crossings of the cell centre
  lines).
* **Topography samples** take the cell with `int()` (truncation toward
  zero) and have **no bounds check**: a sample left of or below the
  meteorological grid, or whose `ix + 1`/`jy + 1` corner falls off the
  supplied array, reads whatever memory lies there. The port refuses
  ([`OutgridError::OrographyOutsideGrid`]); a sample in `(-1, 0)` truncates
  to cell 0 with a negative weight, as upstream, and is accepted.
* **Nest selection** for the samples is FLEXPART's usual one: the
  highest-numbered nest whose rectangle contains the sample with margin
  `eps = nxmax/3e5` (in mother-grid units) on all four sides.
* **`outgrid_init` zeroes only `flux(1:5, ...)`**: the loop is
  `do i=1,5`, so the downward component `flux(6, ...)` starts from
  whatever `allocate` returned until the first `fluxoutput` resets all six.
  Allocation is not ported; [`FluxGrid::zeros`] zeroes all six.

# Units and indices

Degrees for `outlon0`, `outlat0`, `dxout`, `dyout` and the
meteorological grid (`xlon0`, `ylat0`, `dx`, `dy`); metres for heights,
m² for areas, m³ for volumes; topography in m. Output columns `ix`, rows
`jy` and levels `kz` are **0-based** (upstream's levels are 1-based).

```rust
pub mod outgrid { /* ... */ }
```

### Types

#### Struct `OutputGrid`

Placement of one output grid: `outlon0`, `outlat0`, `dxout`, `dyout`,
`numxgrid`, `numygrid` (or the nest's `*n` counterparts).

```rust
pub struct OutputGrid {
    pub outlon0: f64,
    pub outlat0: f64,
    pub dxout: f64,
    pub dyout: f64,
    pub numxgrid: usize,
    pub numygrid: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `outlon0` | `f64` | Longitude of the western edge of column 0, degrees. |
| `outlat0` | `f64` | Latitude of the southern edge of row 0, degrees. |
| `dxout` | `f64` | Cell width, degrees. |
| `dyout` | `f64` | Cell height, degrees. |
| `numxgrid` | `usize` | Number of columns. |
| `numygrid` | `usize` | Number of rows. |

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
    fn clone(self: &Self) -> OutputGrid { /* ... */ }
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
    fn eq(self: &Self, other: &OutputGrid) -> bool { /* ... */ }
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
#### Struct `CellGeometry`

Geometry of every output cell. 2-D arrays are `[jy*numxgrid + ix]`, 3-D
arrays `[(kz*numygrid + jy)*numxgrid + ix]` (`ix` fastest, `kz` 0-based).

```rust
pub struct CellGeometry {
    pub numxgrid: usize,
    pub numygrid: usize,
    pub numzgrid: usize,
    pub area: Vec<f64>,
    pub volume: Vec<f64>,
    pub areaeast: Vec<f64>,
    pub areanorth: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `numxgrid` | `usize` | Columns. |
| `numygrid` | `usize` | Rows. |
| `numzgrid` | `usize` | Levels. |
| `area` | `Vec<f64>` | Horizontal cell area, m² (`area` / `arean`). |
| `volume` | `Vec<f64>` | Cell volume, m³ (`volume` / `volumen`). |
| `areaeast` | `Vec<f64>` | Area of the cell's east-facing wall, m² (`areaeast`; mother grid<br>only upstream). |
| `areanorth` | `Vec<f64>` | Area of the cell's north-facing wall, m² (`areanorth`; mother grid<br>only upstream). |

##### Implementations

###### Methods

- ```rust
  pub fn idx2(self: &Self, ix: usize, jy: usize) -> usize { /* ... */ }
  ```
  Index into [`CellGeometry::area`].

- ```rust
  pub fn idx3(self: &Self, ix: usize, jy: usize, kz: usize) -> usize { /* ... */ }
  ```
  Index into the 3-D arrays; `kz` 0-based.

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
    fn clone(self: &Self) -> CellGeometry { /* ... */ }
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
    fn eq(self: &Self, other: &CellGeometry) -> bool { /* ... */ }
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
#### Struct `NestOrography`

One nested meteorological grid's topography: its placement and `oron`
(`oron(0:nxmaxn-1, 0:nymaxn-1, n)` as a [`Field2`]).

```rust
pub struct NestOrography {
    pub frame: crate::flexpart::wet_deposition::NestFrame,
    pub oron: crate::flexpart::met_fields::Field2,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `frame` | `crate::flexpart::wet_deposition::NestFrame` | Placement in mother-grid units (`xln`, `xrn`, `yln`, `yrn`) and<br>refinement (`xresoln`, `yresoln`). |
| `oron` | `crate::flexpart::met_fields::Field2` | Topography on the nest grid, m. |

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
    fn clone(self: &Self) -> NestOrography { /* ... */ }
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
    fn eq(self: &Self, other: &NestOrography) -> bool { /* ... */ }
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
#### Enum `OutgridError`

Why [`output_orography`] refused.

```rust
pub enum OutgridError {
    OrographyOutsideGrid {
        ix: usize,
        jy: usize,
    },
}
```

##### Variants

###### `OrographyOutsideGrid`

A topography sample of output cell `(ix, jy)` has a bilinear corner
outside the supplied `oro` / `oron` array (upstream reads out of
bounds).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ix` | `usize` | Output column. |
| `jy` | `usize` | Output row. |

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
    fn clone(self: &Self) -> OutgridError { /* ... */ }
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
    fn eq(self: &Self, other: &OutgridError) -> bool { /* ... */ }
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

#### Function `zone_height`

**Attributes:**

- `MustUse { reason: None }`

The zone height `hzone` of output row `jy`, m: `outgrid_init.f90:51-75`.

`R |sin(lat_n) - sin(lat_s)|` evaluated as upstream does, or the arc
length `dyout R pi/180` for a row straddling the equator (see the module
doc).

```rust
pub fn zone_height(outlat0: f64, dyout: f64, jy: usize) -> f64 { /* ... */ }
```

#### Function `cell_area`

**Attributes:**

- `MustUse { reason: None }`

Surface area of a cell in output row `jy`, m²:
`gridarea = 2*pi*r_earth*hzone*dxout/360` (`outgrid_init.f90:80`).

```rust
pub fn cell_area(outlat0: f64, dyout: f64, dxout: f64, jy: usize) -> f64 { /* ... */ }
```

#### Function `cell_geometry`

**Attributes:**

- `MustUse { reason: None }`

`outgrid_init.f90:50-102` / `outgrid_init_nest.f90:87-127`: area, volume
and wall areas of every cell of `grid`, with level tops `outheight`
(m, increasing; `numzgrid = outheight.len()`).

Level 0 spans the ground to `outheight[0]`; level `kz` spans
`outheight[kz-1]` to `outheight[kz]`.

```rust
pub fn cell_geometry(grid: &OutputGrid, outheight: &[f64]) -> CellGeometry { /* ... */ }
```

#### Function `nest_margin`

**Attributes:**

- `MustUse { reason: None }`

`eps = nxmax/3.e5`, the nest-selection margin of `outgrid_init*.f90`
(and of `advance.f90`), mother-grid units.

```rust
pub fn nest_margin(nxmax: usize) -> f64 { /* ... */ }
```

#### Function `output_orography`

`outgrid_init.f90:110-180` / `outgrid_init_nest.f90:134-204`: the mean
topography `oroout` of every cell of `grid`, `[jy*numxgrid + ix]`, m.

`met` places the mother grid (`xlon0`, `ylat0`, `dx`, `dy`), `oro` is its
topography (`oro(0:nxmax-1, 0:nymax-1)`), `nests` the nested grids in
upstream's order (nest `j` is `nests[j-1]`), `eps` the selection margin
([`nest_margin`]).

# Errors

[`OutgridError::OrographyOutsideGrid`] where upstream would read outside
the arrays; nothing is returned then.

```rust
pub fn output_orography(grid: &OutputGrid, met: &crate::flexpart::particle_average::GridGeometry, oro: &crate::flexpart::met_fields::Field2, nests: &[NestOrography], eps: f64) -> Result<Vec<f64>, OutgridError> { /* ... */ }
```

## Module `particle_average`

Particle-dump averaging (`partpos_average.f90`), and [`GriddedMet`], the
slice of `com_mod`'s meteorology that this routine, `plumetraj.f90` and
the mixing-ratio branch of `conccalc.f90` read.

When `ipout = 3`, FLEXPART dumps each particle's position and a set of
met variables **averaged over the output interval**. `partpos_average` is
called once per particle per time step and adds the particle's current
values to running sums (`part_av_*`, `npart_av` in `com_mod`); the dump
divides by the count. The port keeps the sums in a [`ParticleAverages`].

# What is interpolated, and how

Bilinear in the horizontal on the corner weights `p1..p4`, linear in time
between the two fields in memory (`memtime`, `memind`), linear in height
between the two model levels bracketing the particle: PV, specific
humidity, temperature, `u`, `v` and density on model levels; tropopause
height and mixing height on the surface; topography without time
interpolation. The arithmetic is transcribed operation for operation.

# Upstream quirks reproduced (and where the port refuses)

* `ix = xtra1(j)` is an implicit `real(dp) -> integer` conversion, i.e.
  truncation toward zero (a position in `(-1, 0)` gives `ix = 0` and a
  negative weight, which the port reproduces). There is **no** bounds
  check on `ixp = ix + 1`.
  The port returns `None` when a corner falls outside the supplied arrays
  (upstream would read whatever lies there).
* The "north pole fix" `if (jyp >= nymax) jyp = jyp - 1` compares against
  the compile-time array bound `nymax`, not the actual grid size `ny`. The
  port compares against [`GriddedMet::ny`], which is therefore to be read
  as upstream's `nymax`: pass the arrays at their full upstream extent.
* The level search `do il=2,nz; if (height(il) > z)` leaves `indz`
  **unassigned** when the particle is at or above the top level
  `height(nz)`; upstream then uses the value left from the previous call
  (or garbage). The port returns `None` instead of guessing.
* The energy uses the literal `9.81`, not `par_mod`'s `ga` (equal value),
  and `par_mod`'s `cpa`; `(uui**2+vvi**2)/2.` is an integer power, i.e. a
  product.

# Units

FLEXPART's: positions in grid units (`xtra1`, `ytra1`) and m above ground
(`ztra1`); `xlon0`, `ylat0`, `dx`, `dy` in degrees; times in s; met fields
in their `com_mod` units (PV in pvu, `qv` kg/kg, `tt` K, winds m/s,
density kg/m³, heights m).

```rust
pub mod particle_average { /* ... */ }
```

### Types

#### Struct `GriddedMet`

The meteorological arrays of one grid at the two time levels in memory,
as `com_mod` holds them: `oro(0:nxmax-1,0:nymax-1)`,
`tropopause`/`hmix(0:nxmax-1,0:nymax-1,1,numwfmem)`, and `pv`, `qv`, `tt`,
`uu`, `vv`, `rho(0:nxmax-1,0:nymax-1,nzmax,numwfmem)`.

Storage: 2-D `[j][i]`, 2-D per slot `[slot][j][i]`, 3-D `[slot][k][j][i]`
(`i` fastest); level `k` and memory slot are **0-based** here (upstream's
level `k` is `k-1`, slot `m` is `m-1`). Use the `idx*` helpers.

```rust
pub struct GriddedMet {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub height: Vec<f64>,
    pub memtime: [i64; 2],
    pub memind: [usize; 2],
    pub oro: Vec<f64>,
    pub tropopause: Vec<f64>,
    pub hmix: Vec<f64>,
    pub pv: Vec<f64>,
    pub qv: Vec<f64>,
    pub tt: Vec<f64>,
    pub uu: Vec<f64>,
    pub vv: Vec<f64>,
    pub rho: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Array extent in x (upstream's `nxmax` when passed at full size). |
| `ny` | `usize` | Array extent in y. The north-pole clamp compares against this, as<br>upstream compares against `nymax`. |
| `nz` | `usize` | Number of model levels in use (`nz`). |
| `height` | `Vec<f64>` | Height of each model level above ground, m (`height`), length `nz`. |
| `memtime` | `[i64; 2]` | Validity times of the two fields in memory, s (`memtime`). |
| `memind` | `[usize; 2]` | Storage slot of the earlier and of the later field, 0-based (`memind`). |
| `oro` | `Vec<f64>` | Topography, m (`oro`), `[j][i]`. |
| `tropopause` | `Vec<f64>` | Tropopause height, m (`tropopause`), `[slot][j][i]`. |
| `hmix` | `Vec<f64>` | Mixing height, m (`hmix`), `[slot][j][i]`. |
| `pv` | `Vec<f64>` | Potential vorticity, pvu (`pv`), `[slot][k][j][i]`. |
| `qv` | `Vec<f64>` | Specific humidity, kg/kg (`qv`), `[slot][k][j][i]`. |
| `tt` | `Vec<f64>` | Temperature, K (`tt`), `[slot][k][j][i]`. |
| `uu` | `Vec<f64>` | Zonal wind, m/s (`uu`), `[slot][k][j][i]`. |
| `vv` | `Vec<f64>` | Meridional wind, m/s (`vv`), `[slot][k][j][i]`. |
| `rho` | `Vec<f64>` | Air density, kg/m³ (`rho`), `[slot][k][j][i]`. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(nx: usize, ny: usize, nz: usize) -> Self { /* ... */ }
  ```
  All-zero fields of the given extent (two memory slots).

- ```rust
  pub fn idx2(self: &Self, i: usize, j: usize) -> usize { /* ... */ }
  ```
  Index into `oro`.

- ```rust
  pub fn idx2s(self: &Self, i: usize, j: usize, slot: usize) -> usize { /* ... */ }
  ```
  Index into `tropopause` / `hmix`.

- ```rust
  pub fn idx3(self: &Self, i: usize, j: usize, k: usize, slot: usize) -> usize { /* ... */ }
  ```
  Index into the 3-D fields; `k` and `slot` 0-based.

- ```rust
  pub fn level_below(self: &Self, z: f64) -> Option<usize> { /* ... */ }
  ```
  The level-bracketing search FLEXPART repeats inline:

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
    fn clone(self: &Self) -> GriddedMet { /* ... */ }
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
    fn default() -> GriddedMet { /* ... */ }
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
    fn eq(self: &Self, other: &GriddedMet) -> bool { /* ... */ }
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
#### Struct `GridGeometry`

Origin and spacing of the meteorological grid (`xlon0`, `ylat0`, `dx`,
`dy` in `com_mod`), degrees.

```rust
pub struct GridGeometry {
    pub xlon0: f64,
    pub ylat0: f64,
    pub dx: f64,
    pub dy: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xlon0` | `f64` | Longitude of grid point `ix = 0`, degrees. |
| `ylat0` | `f64` | Latitude of grid point `jy = 0`, degrees. |
| `dx` | `f64` | Grid spacing in x, degrees. |
| `dy` | `f64` | Grid spacing in y, degrees. |

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
    fn clone(self: &Self) -> GridGeometry { /* ... */ }
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
    fn eq(self: &Self, other: &GridGeometry) -> bool { /* ... */ }
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
#### Struct `ParticleAverages`

One particle's running sums for the averaged particle dump
(`npart_av(j)`, `part_av_*(j)` in `com_mod`).

```rust
pub struct ParticleAverages {
    pub count: i64,
    pub cartx: f64,
    pub carty: f64,
    pub cartz: f64,
    pub z: f64,
    pub topo: f64,
    pub pv: f64,
    pub qv: f64,
    pub tt: f64,
    pub uu: f64,
    pub vv: f64,
    pub rho: f64,
    pub tro: f64,
    pub hmix: f64,
    pub energy: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `count` | `i64` | Number of samples added (`npart_av`). |
| `cartx` | `f64` | Sum of the Cartesian unit-sphere x coordinate (`part_av_cartx`). |
| `carty` | `f64` | Sum of the y coordinate (`part_av_carty`). |
| `cartz` | `f64` | Sum of the z coordinate (`part_av_cartz`). |
| `z` | `f64` | Sum of the height above ground, m (`part_av_z`). |
| `topo` | `f64` | Sum of the topography, m (`part_av_topo`). |
| `pv` | `f64` | Sum of PV, pvu (`part_av_pv`). |
| `qv` | `f64` | Sum of specific humidity, kg/kg (`part_av_qv`). |
| `tt` | `f64` | Sum of temperature, K (`part_av_tt`). |
| `uu` | `f64` | Sum of `u`, m/s (`part_av_uu`). |
| `vv` | `f64` | Sum of `v`, m/s (`part_av_vv`). |
| `rho` | `f64` | Sum of density, kg/m³ (`part_av_rho`). |
| `tro` | `f64` | Sum of tropopause height, m (`part_av_tro`). |
| `hmix` | `f64` | Sum of mixing height, m (`part_av_hmix`). |
| `energy` | `f64` | Sum of the specific energy `cp T + g z + L q + (u²+v²)/2`, J/kg<br>(`part_av_energy`). |

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
    fn clone(self: &Self) -> ParticleAverages { /* ... */ }
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
    fn default() -> ParticleAverages { /* ... */ }
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
    fn eq(self: &Self, other: &ParticleAverages) -> bool { /* ... */ }
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

#### Function `partpos_average`

`partpos_average.f90`: add one particle's current values to its running
sums.

* `itime` — current time, s; `memtime` must bracket it.
* `xtra1`, `ytra1` — position in grid units; `ztra1` — height above
  ground, m.

Returns `None`, leaving `acc` untouched, where upstream would read an
undefined level index (`ztra1 >= height(nz)`) or outside the arrays (see
the module doc).

```rust
pub fn partpos_average(acc: &mut ParticleAverages, itime: i64, xtra1: f64, ytra1: f64, ztra1: f64, met: &GriddedMet, geo: &GridGeometry) -> Option<()> { /* ... */ }
```

## Module `plume_trajectory`

Plume-centroid trajectories and particle clustering (`iout = 4, 5`):
for each release point, the centre of mass of its particles on the
sphere, mean height, mean topography / mixing height / tropopause / PV,
the fractions of particles in the PBL, in the troposphere and with
|PV| < 2 pvu, and a five-cluster partition of the particle positions
(Dorling et al. 1992, *Atmos. Environ.* 26A, 2575).

# Clustering is deterministic

`clustering.f90` seeds cluster `j` with the particle `j*n/ncluster`
(integer division) — **no random numbers are drawn**, so the port takes
no random input. It is a k-means on the sphere with great-circle distance
([`distance2`]) and centroids re-projected from the mean 3-D unit vector;
it is ported line for line rather than replaced by a generic k-means,
because code-to-code agreement depends on the exact seeds, tie-breaking
(strict `<`, so the lowest-index cluster wins a tie) and stopping rule.

# Upstream quirks reproduced

* `clustering` **returns immediately when `n < ncluster`**, leaving every
  output (`xclust`, ..., `rms`, `zrms`) unassigned; `plumetraj` then
  writes them anyway, so its record carries the previous release point's
  cluster values (observed in the fixture) or stack garbage. The port
  returns `None` for the cluster block in that case.
* `clustering` converts the caller's `xl`, `yl` to radians and back **in
  place**; the round trip `x*pi180/pi180` is not the identity in floating
  point, and `plumetraj` then passes the perturbed arrays to
  `centerofmass` and `distance`. The port takes `&mut` slices and
  reproduces the perturbation.
* The returned `rms` and `rmsclust` are distances to the centroids of the
  **previous** iteration (computed before the centroids are updated),
  while `xclust`, `yclust` are the updated centroids.
* The stopping test `abs(rms-rmsold)/rmsold < 0.005` divides by zero when
  the previous `rms` was 0 (all particles on their centroids); the NaN/inf
  comparison is false and the loop runs all 100 iterations. IEEE in Rust
  gives the same.
* An empty cluster keeps its previous centroid and gets `rmsclust = 0`,
  `zclust = 0`, `fclust = 0`.
* `ncl` (the nearest cluster) is a local that persists between particles:
  if no cluster is nearer than `10.**10.` km (only possible with NaN
  distances) the previous particle's cluster is reused, and the very first
  one is undefined. The port carries `ncl` the same way and returns `None`
  in the undefined case.
* `plumetraj`'s level search leaves `indz` unassigned for a particle at or
  above `height(nz)` (stale from the previous particle). The port refuses
  ([`PlumeError::AboveTopLevel`]). There is no north-pole clamp here,
  unlike `partpos_average`.
* `tropocenter = tropocenter + tri + topo` (evaluated left to right), the
  PV hemisphere test on `yl > 0` (degrees; the equator counts as south),
  `rmsdist = max(rmsdist, 0.)`, and the record's time
  `itime - (ireleasestart + ireleaseend)/2` with Fortran integer division
  (truncation toward zero; Rust's `/` on `i64` is the same).
* `mean` is the single-/double-precision `mean_sp`/`mean_dp` pair (the
  same algorithm): the one-pass `xq - xl*xl/n` variance, set to 0 when it
  is below `eps = 1e-30`; `n = 1` therefore gives `xs = 0` rather than a
  division by zero. The mixed-precision `mean_mixed_*` variants are not
  used by `plumetraj` and are not ported.

# Units

Longitudes/latitudes in degrees (inputs and outputs), heights in m,
distances (`rms`, `rmsclust`, `rmsdist`) in km, fractions in percent.

```rust
pub mod plume_trajectory { /* ... */ }
```

### Types

#### Struct `Clusters`

The outputs of `clustering.f90`.

```rust
pub struct Clusters {
    pub xclust: [f64; 5],
    pub yclust: [f64; 5],
    pub zclust: [f64; 5],
    pub fclust: [f64; 5],
    pub rms: f64,
    pub rmsclust: [f64; 5],
    pub zrms: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xclust` | `[f64; 5]` | Cluster centroid longitudes, degrees. |
| `yclust` | `[f64; 5]` | Cluster centroid latitudes, degrees. |
| `zclust` | `[f64; 5]` | Mean height of each cluster's members, m. |
| `fclust` | `[f64; 5]` | Percentage of particles in each cluster. |
| `rms` | `f64` | Total horizontal rms distance, km (to the previous iteration's centroids). |
| `rmsclust` | `[f64; 5]` | Per-cluster horizontal rms distance, km. |
| `zrms` | `f64` | Total vertical rms deviation from the cluster mean heights, m. |

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
    fn clone(self: &Self) -> Clusters { /* ... */ }
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
    fn eq(self: &Self, other: &Clusters) -> bool { /* ... */ }
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
#### Struct `PlumeParticle`

One particle as `plumetraj` reads it from `com_mod`.

```rust
pub struct PlumeParticle {
    pub itra1: i64,
    pub release: usize,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `itra1` | `i64` | Time the particle is at, s (`itra1`); only particles at `itime` count. |
| `release` | `usize` | Release point, 0-based (`npoint - 1`). |
| `x` | `f64` | Position in grid units (`xtra1`, `ytra1`). |
| `y` | `f64` | Position in grid units. |
| `z` | `f64` | Height above ground, m (`ztra1`). |

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
    fn clone(self: &Self) -> PlumeParticle { /* ... */ }
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
    fn eq(self: &Self, other: &PlumeParticle) -> bool { /* ... */ }
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
#### Struct `ReleaseWindow`

Start and end of a release, s (`ireleasestart`, `ireleaseend`).

```rust
pub struct ReleaseWindow {
    pub start: i64,
    pub end: i64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start` | `i64` | `ireleasestart`. |
| `end` | `i64` | `ireleaseend`. |

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
#### Struct `PlumeRecord`

One record of `trajectories.txt`, as `plumetraj` writes it.

```rust
pub struct PlumeRecord {
    pub release: usize,
    pub time_offset: i64,
    pub xcenter: f64,
    pub ycenter: f64,
    pub zcenter: f64,
    pub topocenter: f64,
    pub hmixcenter: f64,
    pub tropocenter: f64,
    pub pvcenter: f64,
    pub rmsdist: f64,
    pub zrmsdist: f64,
    pub hmixfract: f64,
    pub pvfract: f64,
    pub tropofract: f64,
    pub clusters: Option<Clusters>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `release` | `usize` | Release point, 0-based (upstream prints `j`, 1-based). |
| `time_offset` | `i64` | `itime - (ireleasestart + ireleaseend)/2`, s (integer division). |
| `xcenter` | `f64` | Centre-of-mass longitude, degrees. |
| `ycenter` | `f64` | Centre-of-mass latitude, degrees. |
| `zcenter` | `f64` | Mean height above sea level, m. |
| `topocenter` | `f64` | Mean topography, m. |
| `hmixcenter` | `f64` | Mean mixing height above ground, m. |
| `tropocenter` | `f64` | Mean tropopause height above sea level, m. |
| `pvcenter` | `f64` | Mean PV, pvu. |
| `rmsdist` | `f64` | Horizontal rms distance from the centre of mass, km. |
| `zrmsdist` | `f64` | Standard deviation of the height above sea level, m. |
| `hmixfract` | `f64` | Percentage of particles below the mixing height. |
| `pvfract` | `f64` | Percentage with PV < 2 pvu (north) / PV > -2 pvu (south). |
| `tropofract` | `f64` | Percentage below the tropopause. |
| `clusters` | `Option<Clusters>` | The cluster block (`rms`, `zrms` and the five clusters); `None` when<br>fewer than [`NCLUSTER`] particles, where upstream writes values it<br>never assigned. |

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
    fn clone(self: &Self) -> PlumeRecord { /* ... */ }
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
    fn eq(self: &Self, other: &PlumeRecord) -> bool { /* ... */ }
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
#### Enum `PlumeError`

Why [`plumetraj`] refused.

```rust
pub enum PlumeError {
    AboveTopLevel(usize),
    OutsideGrid(usize),
    UndefinedCluster(usize),
}
```

##### Variants

###### `AboveTopLevel`

A particle is at or above the top model level, where upstream's level
index is unassigned (stale). Index of the particle.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `OutsideGrid`

A particle's interpolation corners fall outside the met arrays.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `UndefinedCluster`

`clustering`'s nearest-cluster index would be undefined.

Fields:

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
    fn clone(self: &Self) -> PlumeError { /* ... */ }
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
    fn eq(self: &Self, other: &PlumeError) -> bool { /* ... */ }
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

#### Function `centerofmass`

**Attributes:**

- `MustUse { reason: None }`

`centerofmass.f90`: centre of mass on the unit sphere of points given in
degrees, re-projected to (longitude, latitude) in degrees.

```rust
pub fn centerofmass(xl: &[f64], yl: &[f64]) -> (f64, f64) { /* ... */ }
```

#### Function `mean`

**Attributes:**

- `MustUse { reason: None }`

`mean_mod`'s `mean` (`mean_sp` / `mean_dp`): mean and sample standard
deviation, one-pass formula, 0 below `eps = 1e-30`. Returns `(xm, xs)`.

```rust
pub fn mean(x: &[f64]) -> (f64, f64) { /* ... */ }
```

#### Function `clustering`

`clustering.f90`: partition `n` particle positions (degrees) into
[`NCLUSTER`] clusters. `xl`, `yl` are converted to radians and back in
place, as upstream does (see the module doc). Returns `None` when
`n < NCLUSTER` (upstream returns with its outputs unassigned and `xl`, `yl`
untouched) or where upstream's `ncl` would be undefined.

```rust
pub fn clustering(xl: &mut [f64], yl: &mut [f64], zl: &[f64]) -> Option<Clusters> { /* ... */ }
```

#### Function `plumetraj`

`plumetraj.f90`: the plume statistics for every release point, at time
`itime`. `lage_last` is `lage(nageclass)`: release points whose start is
more than that before (or after) `itime` are skipped, as are release
points with no particle at `itime` (no record is written for either).

```rust
pub fn plumetraj(itime: i64, particles: &[PlumeParticle], releases: &[ReleaseWindow], lage_last: i64, met: &crate::flexpart::particle_average::GriddedMet, geo: &crate::flexpart::particle_average::GridGeometry) -> Result<Vec<PlumeRecord>, PlumeError> { /* ... */ }
```

### Constants and Statics

#### Constant `NCLUSTER`

Number of clusters (`ncluster` in `par_mod.f90`).

```rust
pub const NCLUSTER: usize = 5;
```

## Module `release`

Particle release from the release points: `releaseparticles.f90`.

At every synchronisation time `itime`, each release point whose window
`[ireleasestart, ireleaseend]` contains `itime` releases `numrel`
particles into vacant storage slots. A slot is vacant when its particle
is not at the current time (`itra1 != itime`): every live particle is
synchronised to `itime` when this routine runs, so any other value marks
a slot that was never used or whose particle has terminated.

Per particle, upstream draws a random position in the release box, gives
it the mass of the point divided by `npart` (scaled by the species-
dependent emission time profile), converts the starting height to metres
above ground (`kindz`), clamps it to `[eps2, height(nz) - 0.5]`, and for
`ind_rel` 1, 3, 4 multiplies the mass by the air density at the start.

# What this routine does NOT do

There is **no radioactive decay** of the release mass in
`releaseparticles.f90` (v10.4): decay is applied to particles after
release, in `timemanager.f90` (ported in [`super::decay`]). There is also
no particle splitting here; only `itrasplit`, the time of the first split,
is set.

# Random numbers are an INPUT

Upstream calls `ran1` (Numerical Recipes, `random_mod.f90`, not ported and
not re-implemented: gh:#410) four times per released particle, in the
order x, y, uncertainty class, z. The port takes these draws from an
iterator the caller supplies; the code-to-code driver replaces `ran1` by a
shim that returns driver-chosen values, so both codes consume identical
numbers. Draws must lie in `[0, 1)` for upstream's arithmetic to mean what
it says, but the port does not check.

# Upstream quirks reproduced

- **The local day of week and hour** use `julmonday = juldate(19000101, 0)`
  and a longitude correction `xlonav / 360` days; daylight saving is a flat
  `+1 h` for **every** release whose UTC month (of `bdate + itime`) is
  April to September, in both hemispheres.
- **Hour 0 is hour 24 of the previous day.** `nint` of the hour can also
  give 24 directly (from 23:30 local), so hour 24 of the *same* day is
  reachable too; both read the profile's 24th entry.
- **Point or area profile** is chosen by the box's horizontal extent
  (`|dx| < 1e-4` and `|dy| < 1e-4` grid units), not by its height (the
  height test is commented out upstream).
- **`xmasssave` carries the fractional particle** from one call to the
  next, per release point.
- **`kindz = 3` (pressure)**: a pressure above every level's pressure
  places the particle at `height(1) / 2`; a pressure below every level's
  leaves `ztra1` holding the **pressure value in hPa, read as metres**
  (upstream never assigns it), after which only the clamps apply.
- **Topography is interpolated for every particle**, whatever `kindz`, and
  used only for `kindz = 2`.
- **Density** for `ind_rel` 1, 3, 4 and for `kindz = 3` comes from memory
  slot **2** of `rho`/`tt` (`rho(...,2)`, "accurate enough" upstream), not
  from the time-interpolated field.
- **`rho_rel(i)`** holds the density of the *last* particle released from
  point `i`.
- **`minpart` persists across release points** within one call, so
  points released later in the loop never back-fill slots earlier than
  the last one used.
- **Reads past the filled grid.** At `xtra1 == nxmin1` exactly (and the
  same in y), `ixp = nx` addresses a column of the static `com_mod`
  arrays (dimension `0:nxmax-1`) that no reader fills; its weight is zero.
  See [`StaticExtents`] for how the port reproduces that read.

# Refusals

Where upstream would `stop` or read memory it never defined, the port
returns a [`ReleaseError`]. The particle arrays are then partly updated,
as upstream's would have been at that point; the caller should treat them
as invalid.

# Index conventions and units

Particle slots and release points are 0-based (`ipart - 1`, `i - 1`);
the *values* stored in `npoint` and `nclass` are upstream's (1-based).
Positions in grid units (`xtra1`, `ytra1`), heights m, times s, masses kg
(or whatever unit the release file used), pressures hPa, densities kg/m³.

```rust
pub mod release { /* ... */ }
```

### Types

#### Struct `StaticExtents`

The `par_mod` array extents of upstream's **static** `com_mod` fields
(`oro`, `rho`, `tt`, `pv`, ...: `(0:nxmax-1, 0:nymax-1, nzmax, numwfmem)`).

Upstream sometimes indexes one column or row past the filled grid with a
zero weight (e.g. `ixp = nx` at `xtra1 == nxmin1`). For a static array that
is a defined read: column-major addressing lands on the unused tail of the
array when `nx < nxmax`, or on the next row/level when `nx == nxmax`. The
port reproduces that address arithmetic exactly, and returns the stored
value when the address falls inside the filled grid and **zero**
otherwise. Zero is what a static array holds where no reader ever wrote
(they live in `.bss`), which is true of a FLEXPART run on one fixed grid;
it is not true if the same process earlier filled a larger grid, which
FLEXPART never does.

```rust
pub struct StaticExtents {
    pub nxmax: usize,
    pub nymax: usize,
    pub nzmax: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nxmax` | `usize` | `nxmax`. |
| `nymax` | `usize` | `nymax`. |
| `nzmax` | `usize` | `nzmax`. |

##### Implementations

###### Methods

- ```rust
  pub fn read3(self: &Self, met: &MetFields, field: &[f64], ix: i64, jy: i64, k: i64, slot: i64) -> Option<f64> { /* ... */ }
  ```
  Read `field(ix, jy, k, slot)` of a static `com_mod` array whose filled

- ```rust
  pub fn read2(self: &Self, nx: usize, ny: usize, field: &[f64], ix: i64, jy: i64) -> Option<f64> { /* ... */ }
  ```
  Read `field(ix, jy)` of a static two-dimensional `com_mod` array

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
    fn clone(self: &Self) -> StaticExtents { /* ... */ }
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
    fn eq(self: &Self, other: &StaticExtents) -> bool { /* ... */ }
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
#### Enum `KindZ`

`kindz`: what a release point's `zpoint1`, `zpoint2` mean.

```rust
pub enum KindZ {
    AboveGround,
    AboveSeaLevel,
    Pressure,
}
```

##### Variants

###### `AboveGround`

`1`: metres above ground.

###### `AboveSeaLevel`

`2`: metres above sea level (topography is subtracted).

###### `Pressure`

`3`: pressure, hPa.

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
    fn clone(self: &Self) -> KindZ { /* ... */ }
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
    fn eq(self: &Self, other: &KindZ) -> bool { /* ... */ }
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
#### Struct `ReleasePoint`

One release point of `point_mod`, in grid units (after `coordtrafo`).

```rust
pub struct ReleasePoint {
    pub ireleasestart: i64,
    pub ireleaseend: i64,
    pub npart: i64,
    pub kindz: KindZ,
    pub xpoint1: f64,
    pub xpoint2: f64,
    pub ypoint1: f64,
    pub ypoint2: f64,
    pub zpoint1: f64,
    pub zpoint2: f64,
    pub xmass: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ireleasestart` | `i64` | Start of the release window, s (`ireleasestart`). |
| `ireleaseend` | `i64` | End of the release window, s (`ireleaseend`). |
| `npart` | `i64` | Total number of particles over the window (`npart`). |
| `kindz` | `KindZ` | Meaning of `zpoint1`, `zpoint2` (`kindz`). |
| `xpoint1` | `f64` | Release box, mother-grid units. |
| `xpoint2` | `f64` | Release box, mother-grid units. |
| `ypoint1` | `f64` | Release box, mother-grid units. |
| `ypoint2` | `f64` | Release box, mother-grid units. |
| `zpoint1` | `f64` | Release box bottom (m or hPa per `kindz`). |
| `zpoint2` | `f64` | Release box top (m or hPa per `kindz`). |
| `xmass` | `Vec<f64>` | Total mass per species over the window (`xmass(i, 1:nspec)`). |

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
    fn clone(self: &Self) -> ReleasePoint { /* ... */ }
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
    fn eq(self: &Self, other: &ReleasePoint) -> bool { /* ... */ }
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
#### Struct `EmissionVariation`

Emission time profiles of `com_mod` (`area_hour`, `point_hour`,
`area_dow`, `point_dow`), as read from the `RELEASES` file.

```rust
pub struct EmissionVariation {
    pub area_hour: Vec<f64>,
    pub point_hour: Vec<f64>,
    pub area_dow: Vec<f64>,
    pub point_dow: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `area_hour` | `Vec<f64>` | Hourly factors of area sources, `[k * 24 + (nhour - 1)]`. |
| `point_hour` | `Vec<f64>` | Hourly factors of point sources, `[k * 24 + (nhour - 1)]`. |
| `area_dow` | `Vec<f64>` | Day-of-week factors of area sources, `[k * 7 + (ndayofweek - 1)]`<br>(day 1 = Monday). |
| `point_dow` | `Vec<f64>` | Day-of-week factors of point sources, `[k * 7 + (ndayofweek - 1)]`. |

##### Implementations

###### Methods

- ```rust
  pub fn uniform(nspec: usize) -> Self { /* ... */ }
  ```
  All factors 1 (no time variation) for `nspec` species.

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
    fn clone(self: &Self) -> EmissionVariation { /* ... */ }
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
    fn eq(self: &Self, other: &EmissionVariation) -> bool { /* ... */ }
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
#### Struct `ParticleStore`

The particle arrays of `com_mod` (`itra1`, ..., `xscav_frac1`) and the two
counters `numpart`, `numparticlecount`. The arrays' length is `maxpart`.

```rust
pub struct ParticleStore {
    pub itra1: Vec<i64>,
    pub npoint: Vec<i64>,
    pub nclass: Vec<i64>,
    pub idt: Vec<i64>,
    pub itramem: Vec<i64>,
    pub itrasplit: Vec<i64>,
    pub xtra1: Vec<f64>,
    pub ytra1: Vec<f64>,
    pub ztra1: Vec<f64>,
    pub nspec: usize,
    pub xmass1: Vec<f64>,
    pub xscav_frac1: Vec<f64>,
    pub numpart: usize,
    pub numparticlecount: i64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `itra1` | `Vec<i64>` | Time the particle is at, s; [`ITRA_INACTIVE`] when terminated. |
| `npoint` | `Vec<i64>` | Release point (1-based) or, with `mquasilag`/domain filling, the<br>particle's running number. |
| `nclass` | `Vec<i64>` | Uncertainty class, `1..=nclassunc`. |
| `idt` | `Vec<i64>` | Time step of the next integration, s. |
| `itramem` | `Vec<i64>` | Release time, s. |
| `itrasplit` | `Vec<i64>` | Time of the next split, s. |
| `xtra1` | `Vec<f64>` | x position, grid units (`real(kind=dp)` upstream). |
| `ytra1` | `Vec<f64>` | y position, grid units (`real(kind=dp)` upstream). |
| `ztra1` | `Vec<f64>` | Height above ground, m. |
| `nspec` | `usize` | Species stored per particle. |
| `xmass1` | `Vec<f64>` | Mass per species, `[ipart * nspec + k]`. |
| `xscav_frac1` | `Vec<f64>` | Scavenged fraction per species, `[ipart * nspec + k]`. |
| `numpart` | `usize` | Highest slot in use, 1-based (`numpart`). |
| `numparticlecount` | `i64` | Particles released so far (`numparticlecount`). |

##### Implementations

###### Methods

- ```rust
  pub fn new(maxpart: usize, nspec: usize) -> Self { /* ... */ }
  ```
  `maxpart` empty slots, as `FLEXPART.f90` initialises them

- ```rust
  pub fn maxpart(self: &Self) -> usize { /* ... */ }
  ```
  `maxpart`.

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
    fn clone(self: &Self) -> ParticleStore { /* ... */ }
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
    fn eq(self: &Self, other: &ParticleStore) -> bool { /* ... */ }
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
#### Struct `ReleaseNest`

One nest as `releaseparticles` reads it.

```rust
pub struct ReleaseNest {
    pub geometry: super::advance::NestGeometry,
    pub met: super::interpolation::MetFields,
    pub oro: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `geometry` | `super::advance::NestGeometry` | Placement in mother-grid units. |
| `met` | `super::interpolation::MetFields` | The nest's fields; `rho`, `tt` (slot 2) and `height` are read. |
| `oro` | `Vec<f64>` | Nest topography, m, `[jy * nx + ix]` (`oron`). |

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
    fn clone(self: &Self) -> ReleaseNest { /* ... */ }
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
    fn eq(self: &Self, other: &ReleaseNest) -> bool { /* ... */ }
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
#### Struct `ReleaseDomain`

The mother grid and clock as `releaseparticles` reads them.

```rust
pub struct ReleaseDomain {
    pub nxmin1: i64,
    pub xglobal: bool,
    pub xlon0: f64,
    pub dx: f64,
    pub bdate: f64,
    pub extents: StaticExtents,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nxmin1` | `i64` | `nx - 1`. |
| `xglobal` | `bool` | Cyclic in longitude (`xglobal`). |
| `xlon0` | `f64` | Longitude of the grid origin, degrees (`xlon0`). |
| `dx` | `f64` | Grid spacing in x, degrees (`dx`). |
| `bdate` | `f64` | Julian date of the simulation start, days (`bdate`, `real(kind=dp)`). |
| `extents` | `StaticExtents` | Static array extents; `nxmax` also sets the nest margin<br>`eps = nxmax / 3e5`. |

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
    fn clone(self: &Self) -> ReleaseDomain { /* ... */ }
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
    fn eq(self: &Self, other: &ReleaseDomain) -> bool { /* ... */ }
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
#### Struct `ReleaseSettings`

Run switches `releaseparticles` reads from `com_mod`.

```rust
pub struct ReleaseSettings {
    pub ldirect: i64,
    pub lsynctime: i64,
    pub mintime: i64,
    pub itsplit: i64,
    pub nclassunc: i64,
    pub mquasilag: bool,
    pub density_weighted: bool,
    pub backward_deposition: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ldirect` | `i64` | `+1` forward, `-1` backward. |
| `lsynctime` | `i64` | Synchronisation interval, s. |
| `mintime` | `i64` | Minimum time step, s (`mintime`), assigned to `idt`. |
| `itsplit` | `i64` | Splitting time constant, s (`itsplit`). |
| `nclassunc` | `i64` | Number of uncertainty classes (`nclassunc`). |
| `mquasilag` | `bool` | `mquasilag != 0`: `npoint` stores the particle number. |
| `density_weighted` | `bool` | `ind_rel` is 1, 3 or 4: multiply the mass by the air density. |
| `backward_deposition` | `bool` | `DRYBKDEP .or. WETBKDEP`: set `xscav_frac1 = -1` on release. |

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
    fn clone(self: &Self) -> ReleaseSettings { /* ... */ }
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
    fn eq(self: &Self, other: &ReleaseSettings) -> bool { /* ... */ }
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
#### Enum `ReleaseError`

Why [`releaseparticles`] refused.

```rust
pub enum ReleaseError {
    TooManyParticles,
    DrawsExhausted,
    TimeProfileIndex,
    UndefinedRead,
    UndefinedLevel,
}
```

##### Variants

###### `TooManyParticles`

No vacant slot left: upstream's `stop` ("TOTAL NUMBER OF PARTICLES
REQUIRED EXCEEDS THE MAXIMUM ALLOWED NUMBER").

###### `DrawsExhausted`

The draw iterator ran dry.

###### `TimeProfileIndex`

The local day of week or hour fell outside `1..=7` / `1..=24`
(a date before 1900-01-01): upstream reads outside the profile arrays.

###### `UndefinedRead`

A grid read outside the static array (upstream reads another
variable), or outside a nest (allocatable, undefined there).

###### `UndefinedLevel`

No model level lies above the particle (a NaN height); upstream would
use an undefined level index.

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
    fn clone(self: &Self) -> ReleaseError { /* ... */ }
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
    fn eq(self: &Self, other: &ReleaseError) -> bool { /* ... */ }
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

#### Function `releaseparticles`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]")`

`releaseparticles(itime)`: release the particles due at `itime`.

# Arguments
- `itime` — current time, s since the simulation start.
- `points` — the release points (`point_mod`), grid units.
- `xmasssave` — per point, the fractional particle carried between calls
  (`xmass_mod`); read and updated.
- `rho_rel` — per point, the air density at the last particle released
  when `density_weighted`; written.
- `variation` — the emission time profiles.
- `settings`, `domain` — the `com_mod` switches and grid.
- `mother` — mother-grid fields; `rho`, `tt` (memory slot 2) and `height`
  are read.
- `oro` — mother-grid topography, m, `[jy * nx + ix]`.
- `nests` — nested grids, innermost last.
- `draws` — the `ran1` sequence (see the module docs).
- `parts` — the particle arrays; vacant slots are filled.

# Errors
See [`ReleaseError`].

```rust
pub fn releaseparticles<I: Iterator<Item = f64>>(itime: i64, points: &[ReleasePoint], xmasssave: &mut [f64], rho_rel: &mut [f64], variation: &EmissionVariation, settings: &ReleaseSettings, domain: &ReleaseDomain, mother: &super::interpolation::MetFields, oro: &[f64], nests: &[ReleaseNest], draws: &mut I, parts: &mut ParticleStore) -> Result<(), ReleaseError> { /* ... */ }
```

### Constants and Statics

#### Constant `ITRA_INACTIVE`

`itra1` value of a terminated or never-used particle (`-999999999`).

```rust
pub const ITRA_INACTIVE: i64 = -999_999_999;
```

#### Constant `NUMWFMEM`

Number of wind fields held in memory (`numwfmem`, serial build).

```rust
pub const NUMWFMEM: usize = 2;
```

## Module `shift_field`

Longitude shift of a global field, and the cyclic duplicate column.

For a global grid, `gridcheck`/`readwind` may rotate every field by
`nxshift` columns (so a nest can straddle the date line), and always copy
column 0 into column `nxf` so the grid closes on itself
(`nx = nxfield + 1`). `shift_field_0` does this for a 2-d field,
`shift_field` for every level `1..nzf` of time slot `n` of a 4-d field.

Column `ix` moves to `ix - nxshift` if `ix >= nxshift`, else to
`nxf - nxshift + ix`, for `ix = 0..nxf-1`; then `field(nxf, jy) =
field(0, jy)`. A pure permutation and a copy: no arithmetic, so the port
is exact at any precision.

# Storage

The field is a slice with upstream's column-major layout: element
`(ix, jy[, kz])` at `ix + ldx*(jy + ldy*kz)`, where `ldx`, `ldy` are the
declared extents (`nxmax`, `nymax` upstream). For `shift_field`, pass the
time slot `n` (`ldx*ldy*nzfmax` values) as the slice.

# Quirks and guards

* Rows `nyf..` and levels `nzf..` are untouched.
* `nxshift = 0` still writes the duplicate column.
* `nxshift == nxf` maps every column to itself (`gridcheck` forbids
  `nxshift >= nxfield`, but the routine is well defined there).
* `nxshift > nxf` or `< 0` would write `xshiftaux` out of bounds upstream;
  the port refuses. So does `ldx <= nxf` (the duplicate column must fit).

```rust
pub mod shift_field { /* ... */ }
```

### Types

#### Enum `ShiftError`

Why a shift was refused.

```rust
pub enum ShiftError {
    ShiftOutOfRange,
    ShapeMismatch,
}
```

##### Variants

###### `ShiftOutOfRange`

`nxshift < 0` or `nxshift > nxf`: upstream writes outside `xshiftaux`.

###### `ShapeMismatch`

`ldx <= nxf`, `nyf > ldy`, or the slice is too short.

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
    fn clone(self: &Self) -> ShiftError { /* ... */ }
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
    fn eq(self: &Self, other: &ShiftError) -> bool { /* ... */ }
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

#### Function `shift_field`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`shift_field(field, nxf, nyf, nzfmax, nzf, nmax, n)` on time slot `n`:
`slot` holds `ldx*ldy*nzf` (or more) values of that slot.

# Errors

See [`ShiftError`].

```rust
pub fn shift_field(slot: &mut [f64], ldx: usize, ldy: usize, nxf: usize, nyf: usize, nzf: usize, nxshift: i64) -> Result<(), ShiftError> { /* ... */ }
```

#### Function `shift_field_0`

`shift_field_0(field, nxf, nyf)`: the 2-d version (`oro`, `lsm`,
`excessoro`, `ewss`, `nsss`), identical to [`shift_field`] with one level.

# Errors

See [`ShiftError`].

```rust
pub fn shift_field_0(field: &mut [f64], ldx: usize, ldy: usize, nxf: usize, nyf: usize, nxshift: i64) -> Result<(), ShiftError> { /* ... */ }
```

## Module `solar`

Solar geometry and the O(¹D) photolysis rate that drives FLEXPART's OH
reaction scaling.

Both routines declare their **own** `pi = 3.1415927`, not `par_mod`'s
`3.14159265`; the port keeps each routine's literal.

```rust
pub mod solar { /* ... */ }
```

### Functions

#### Function `zenithangle`

**Attributes:**

- `MustUse { reason: None }`

`zenithangle.f90`: solar zenith angle, degrees, from an approximate
declination and equation of time (Fourier series in the day of year).

# Arguments
- `ylat`, `xlon` — latitude and longitude, degrees.
- `jul` — Julian date (UTC).

The day-of-year count is upstream's: `31 (m-1) + d - int(0.4 m + 2.3)`
after February, plus one in years divisible by four (no century rule).

```rust
pub fn zenithangle(ylat: f64, xlon: f64, jul: f64) -> f64 { /* ... */ }
```

#### Function `photo_o1d`

**Attributes:**

- `MustUse { reason: None }`

`photo_O1D.f90`: O(¹D) photolysis rate, 1/s, for a solar zenith angle in
degrees. Log-linear in `1/cos(sza)` between table angles, scaled by a
parameterised NO₂ photolysis rate; zero at night (`sza >= 90`).

# Returns
`None` for `sza < 0`: upstream's table index is then never assigned and
the result is undefined. A zenith angle is non-negative by definition, so
this only arises from a caller error.

```rust
pub fn photo_o1d(sza: f64) -> Option<f64> { /* ... */ }
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

## Module `timemanager`

The per-synchronisation-step bookkeeping of `timemanager.f90`, as
deterministic functions. File I/O, `getfields`/`readwind`, particle
release and domain filling are out of scope; the routines `timemanager`
calls are ported in their own modules and are named below.

# One synchronisation step, in upstream's order

`do itime = 0, ideltas, lsynctime` (line 151):

1. `wetdepo` if `WETDEP`, `itime /= 0` and there are particles
   ([`crate::flexpart::wet_deposition::wetdepo`]);
2. `ohreaction` under the same condition
   ([`crate::flexpart::oh_chemistry::ohreaction`]);
3. backward runs with convection: `convmix` for `itime < 0`
   ([`crate::flexpart::convmix`]);
4. `getfields` (not ported, I/O), `gethourlyOH` if `OHREA`;
5. release (`releaseparticles`, or `init_domainfill` /
   `boundcond_domainfill`; not ported);
6. forward runs with convection: `convmix`;
7. **decay of the deposition grids** at the middle of the averaging
   interval ([`ClockEvents::decay_deposition`],
   [`decay_deposition_grid`]);
8. inside the averaging window: **sampling** (`conccalc`,
   [`crate::flexpart::concentration::conccalc`]) with weight ½ at the
   window ends; at the window end, **output** (`concoutput*`,
   [`crate::flexpart::concoutput::concoutput`]), the clock moves on one
   output step, the new window's first sample, and **particle splitting**
   ([`split_particles`]);
9. `exit` at `itime == ideltas`;
10. `ldeltat`, the time since the deposition decay was last applied;
11. for every particle due at `itime` ([`particle_due`]):
    [`pre_advance`] (release slot, age class, `initialize` for new
    particles, backward scavenging initialisation), then `advance`
    ([`crate::flexpart::advance::advance`]), then `partpos_average` /
    `calcfluxes` (optional output), then [`post_advance`] (termination,
    radioactive decay and dry-deposition mass removal, the `minmass`
    test, `drydepokernel` calls, the age limit).

Termination by **height or leaving the domain** happens inside `advance`
(it returns `nstop = 3`); [`post_advance`] then only marks the particle.

# Upstream quirks reproduced (and documented, not fixed)

* **Particles are split only at output times.** The split test (line
  468) sits inside `if (itime == loutend .and. outnum > 0)`, so a
  particle whose `itrasplit` passes between outputs waits for the next
  output.
* **With `iout = 4` (plume trajectories only) `outnum` is never reset**
  (the reset, line 431, is inside `if (iout <= 3 .or. iout == 5)`), so it
  grows for the whole run. Nothing reads it then.
* **The clock stalls if `loutend` is never sampled with a positive
  `outnum`**: the clock only advances inside the output branch. Upstream
  reaches `loutend` with `outnum > 0` whenever any sample fell in the
  window; the sweep includes `loutsample` not dividing the window, where
  `loutend` itself is not sampled but output still happens.
* **A particle whose release mass `xmass(npoint, ks)` is 0 for every
  species is terminated by the `minmass` test** (`xmassfract` stays 0),
  unless domain filling or quasi-Lagrangian mode skips the test.
* **The dry deposit is passed to `drydepokernel` for a particle that has
  just been terminated for small mass**: the `minmass` termination comes
  first and does not skip the deposition call.
* **`drydeposit` is a timemanager local kept between particles**: for a
  species without dry deposition it is not assigned, and
  `drydepokernel` receives the previous particle's value (it ignores it,
  since it tests `DRYDEPSPEC` itself). The port takes the buffer as an
  argument so the stale value is visible.
* **The age termination calls `initial_cond_calc` again** after a
  `minmass` termination when `linit_cond >= 1`: `itra1 = -999999999`
  then makes `|itra1 - itramem| >= lage(nageclass)` true.
* **The decay back-dating of the dry deposit uses `|ldeltat|`**, and
  `ldeltat` is computed with `itime < loutnext`, a test that is
  direction-blind; dry deposition onto the output grid only happens in
  forward runs, so backward runs never use it.
* **The age class of a particle older than the last class boundary is
  `nageclass + 1`** (the `do ... exit` loop runs out), out of the bounds of
  every age-classed grid. [`pre_advance`] returns `None`.
* **`get_wetscav`'s counters are 64-bit arrays, `timemanager` passes a
  default-integer scalar** (`call get_wetscav(...,idummy,idummy,wetscav)`,
  line 583, against `integer(selected_int_kind(16)), dimension(nspec)
  :: blc_count, inc_count` in `get_wetscav.f90:56`): a `WETBKDEP` run
  increments 8 bytes at `idummy + 8*(ks-1)`, out of bounds. Found by
  reading; not exercised (the counters do not affect the result).

# Precision

Default `real` is `f64` here (upstream's `-fdefault-real-8` build). The
`real(xtra1(j))` conversion of the `drydepokernel` call is therefore an
identity here and an `f32` rounding in the shipped build.

# Units

Times s (signed with `ldirect` in backward runs, as `readcommand.f90`
stores them), masses kg, decay constants s⁻¹, deposition probability
dimensionless.

```rust
pub mod timemanager { /* ... */ }
```

### Types

#### Struct `ClockSettings`

The `com_mod` settings the clock reads.

```rust
pub struct ClockSettings {
    pub ldirect: i64,
    pub loutstep: i64,
    pub loutaver: i64,
    pub loutsample: i64,
    pub itsplit: i64,
    pub iout: i32,
    pub dep: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ldirect` | `i64` | `+1` forward, `-1` backward. |
| `loutstep` | `i64` | Output interval, s (`loutstep`, negative backward). |
| `loutaver` | `i64` | Averaging time, s (`loutaver`). |
| `loutsample` | `i64` | Sampling interval, s (`loutsample`). |
| `itsplit` | `i64` | Time at which particles start to be split, s (`itsplit`). |
| `iout` | `i32` | Output selector (`iout`). |
| `dep` | `bool` | Any deposition switched on (`DEP`). |

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
    fn clone(self: &Self) -> ClockSettings { /* ... */ }
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
    fn eq(self: &Self, other: &ClockSettings) -> bool { /* ... */ }
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
#### Struct `ClockEvents`

What happens at one `itime` (all `false`/`None` when nothing does).

```rust
pub struct ClockEvents {
    pub decay_deposition: bool,
    pub sample: Option<f64>,
    pub output: Option<f64>,
    pub resample: bool,
    pub split: bool,
    pub ldeltat: Option<i64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `decay_deposition` | `bool` | Decay the deposition grids now (line 264). |
| `sample` | `Option<f64>` | `conccalc` is called with this weight (lines 353-359). |
| `output` | `Option<f64>` | The output routines are called with this `outnum` (lines 371-430). |
| `resample` | `bool` | The new window starts now: `conccalc` with weight 0.5 (lines<br>455-459). |
| `split` | `bool` | The particle-split test runs (line 468). |
| `ldeltat` | `Option<i64>` | `ldeltat` (lines 509-513), `None` on the last step (upstream exits<br>first, line 504). |

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
    fn clone(self: &Self) -> ClockEvents { /* ... */ }
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
    fn eq(self: &Self, other: &ClockEvents) -> bool { /* ... */ }
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
#### Struct `OutputClock`

The clock's state: `loutnext`, `loutstart`, `loutend`, `outnum`.

```rust
pub struct OutputClock {
    pub settings: ClockSettings,
    pub loutnext: i64,
    pub loutstart: i64,
    pub loutend: i64,
    pub outnum: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `settings` | `ClockSettings` | Settings. |
| `loutnext` | `i64` | Middle of the current averaging window, s. |
| `loutstart` | `i64` | Window start, s. |
| `loutend` | `i64` | Window end, s. |
| `outnum` | `f64` | Accumulated sample weight (`outnum`). |

##### Implementations

###### Methods

- ```rust
  pub fn new(settings: ClockSettings) -> Self { /* ... */ }
  ```
  Lines 118-121: the first window is centred on `loutstep/2` (integer

- ```rust
  pub fn step(self: &mut Self, itime: i64, last: bool) -> ClockEvents { /* ... */ }
  ```
  The events at `itime`, updating the clock. `last` is

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
    fn clone(self: &Self) -> OutputClock { /* ... */ }
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
    fn eq(self: &Self, other: &OutputClock) -> bool { /* ... */ }
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
#### Struct `SplitParticle`

The particle fields the split copies (`com_mod` arrays at one index).

```rust
pub struct SplitParticle {
    pub itra1: i64,
    pub itramem: i64,
    pub itrasplit: i64,
    pub idt: i64,
    pub npoint: usize,
    pub nclass: usize,
    pub xtra1: f64,
    pub ytra1: f64,
    pub ztra1: f64,
    pub uap: f64,
    pub ucp: f64,
    pub uzp: f64,
    pub us: f64,
    pub vs: f64,
    pub ws: f64,
    pub cbt: i16,
    pub xmass1: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `itra1` | `i64` | Current time, s (`itra1`). |
| `itramem` | `i64` | Release time, s (`itramem`). |
| `itrasplit` | `i64` | Next split time, s (`itrasplit`). |
| `idt` | `i64` | Time step, s (`idt`). |
| `npoint` | `usize` | Release point, 1-based (`npoint`). |
| `nclass` | `usize` | Uncertainty class, 1-based (`nclass`). |
| `xtra1` | `f64` | Position, grid units (`xtra1`, `real(kind=dp)`). |
| `ytra1` | `f64` | See [`SplitParticle::xtra1`]. |
| `ztra1` | `f64` | Height, m. |
| `uap` | `f64` | Turbulent velocities, m/s (`uap`, `ucp`, `uzp`). |
| `ucp` | `f64` | See [`SplitParticle::uap`]. |
| `uzp` | `f64` | See [`SplitParticle::uap`]. |
| `us` | `f64` | Previous turbulent sigmas, m/s (`us`, `vs`, `ws`). |
| `vs` | `f64` | See [`SplitParticle::us`]. |
| `ws` | `f64` | See [`SplitParticle::us`]. |
| `cbt` | `i16` | CBL flag (`cbt`, `integer(kind=2)`). |
| `xmass1` | `Vec<f64>` | Mass per species, kg (`xmass1`). |

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
    fn clone(self: &Self) -> SplitParticle { /* ... */ }
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
    fn eq(self: &Self, other: &SplitParticle) -> bool { /* ... */ }
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
#### Struct `PreAdvanceSettings`

Settings [`pre_advance`] reads.

```rust
pub struct PreAdvanceSettings {
    pub output_each_release: bool,
    pub lage: Vec<i64>,
    pub drybkdep: bool,
    pub wetbkdep: bool,
    pub drydepspec: Vec<bool>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `output_each_release` | `bool` | `ioutputforeachrelease == 1`: one output slot per release point. |
| `lage` | `Vec<i64>` | Age class upper bounds, s (`lage(1:nageclass)`). |
| `drybkdep` | `bool` | Backward dry-deposition receptor mode (`DRYBKDEP`). |
| `wetbkdep` | `bool` | Backward wet-deposition receptor mode (`WETBKDEP`). |
| `drydepspec` | `Vec<bool>` | Per-species dry deposition (`DRYDEPSPEC`). |

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
    fn clone(self: &Self) -> PreAdvanceSettings { /* ... */ }
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
    fn eq(self: &Self, other: &PreAdvanceSettings) -> bool { /* ... */ }
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
#### Struct `PreAdvance`

What [`pre_advance`] decided.

```rust
pub struct PreAdvance {
    pub kp: usize,
    pub nage: Option<usize>,
    pub initialize: bool,
    pub vdep_calls: usize,
    pub wetscav_calls: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kp` | `usize` | Output release slot, 0-based (`kp - 1`). |
| `nage` | `Option<usize>` | Age class, 0-based; `None` when the particle is older than the last<br>class boundary (upstream's `nage = nageclass + 1`). |
| `initialize` | `bool` | `initialize` is to be called (`itramem == itime` or `itime == 0`). |
| `vdep_calls` | `usize` | How many times `get_vdep_prob` was called (once per species still<br>unset, `DRYBKDEP`). |
| `wetscav_calls` | `usize` | How many times `get_wetscav` was called (`WETBKDEP`). |

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
    fn clone(self: &Self) -> PreAdvance { /* ... */ }
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
    fn eq(self: &Self, other: &PreAdvance) -> bool { /* ... */ }
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
#### Struct `WetScavResult`

One species' `get_wetscav` result for this particle.

```rust
pub struct WetScavResult {
    pub wetscav: f64,
    pub grfraction1: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `wetscav` | `f64` | Scavenging coefficient, s⁻¹ (`wetscav`). |
| `grfraction1` | `f64` | `grfraction(1)`, the precipitating fraction of the grid cell. |

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
    fn clone(self: &Self) -> WetScavResult { /* ... */ }
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
    fn eq(self: &Self, other: &WetScavResult) -> bool { /* ... */ }
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
#### Struct `PostAdvanceSettings`

Settings [`post_advance`] reads.

```rust
pub struct PostAdvanceSettings {
    pub ldirect: i64,
    pub lsynctime: i64,
    pub decay: Vec<f64>,
    pub drydep: bool,
    pub drydepspec: Vec<bool>,
    pub mdomainfill: i32,
    pub mquasilag: i32,
    pub nested_output: bool,
    pub linit_cond: i32,
    pub lage_max: i64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ldirect` | `i64` | `+1` forward, `-1` backward. |
| `lsynctime` | `i64` | Synchronisation interval, s (signed). |
| `decay` | `Vec<f64>` | Decay constant per species, s⁻¹ (`decay`, 0 = stable). |
| `drydep` | `bool` | Dry deposition switched on (`DRYDEP`). |
| `drydepspec` | `Vec<bool>` | Per-species dry deposition (`DRYDEPSPEC`). |
| `mdomainfill` | `i32` | Domain-filling mode (`mdomainfill`, 0 = off). |
| `mquasilag` | `i32` | Quasi-Lagrangian mode (`mquasilag`, 0 = off). |
| `nested_output` | `bool` | Nested output on (`nested_output == 1`). |
| `linit_cond` | `i32` | Initial-condition output (`linit_cond`, backward runs). |
| `lage_max` | `i64` | The last age-class bound, s (`lage(nageclass)`). |

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
    fn clone(self: &Self) -> PostAdvanceSettings { /* ... */ }
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
    fn eq(self: &Self, other: &PostAdvanceSettings) -> bool { /* ... */ }
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
#### Struct `StepParticle`

The particle state [`post_advance`] reads and updates.

```rust
pub struct StepParticle {
    pub itra1: i64,
    pub itramem: i64,
    pub nclass: usize,
    pub xtra1: f64,
    pub ytra1: f64,
    pub xmass1: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `itra1` | `i64` | Time, s (`itra1`). |
| `itramem` | `i64` | Release time, s (`itramem`). |
| `nclass` | `usize` | Uncertainty class, 1-based (`nclass`). |
| `xtra1` | `f64` | Position, grid units (`xtra1`). |
| `ytra1` | `f64` | See [`StepParticle::xtra1`]. |
| `xmass1` | `Vec<f64>` | Mass per species, kg (`xmass1`). |

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
    fn clone(self: &Self) -> StepParticle { /* ... */ }
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
    fn eq(self: &Self, other: &StepParticle) -> bool { /* ... */ }
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
#### Struct `KernelCall`

One `drydepokernel` (or `drydepokernel_nest`) call.

```rust
pub struct KernelCall {
    pub nest: bool,
    pub nunc: usize,
    pub deposit: Vec<f64>,
    pub x: f64,
    pub y: f64,
    pub nage: usize,
    pub kp: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nest` | `bool` | `drydepokernel_nest` rather than `drydepokernel`. |
| `nunc` | `usize` | Uncertainty class, 1-based (`nclass(j)`). |
| `deposit` | `Vec<f64>` | The whole `drydeposit` buffer at the call, kg per species. |
| `x` | `f64` | `real(xtra1(j))`. |
| `y` | `f64` | `real(ytra1(j))`. |
| `nage` | `usize` | Age class as passed (1-based, `nage`). |
| `kp` | `usize` | Release slot as passed (1-based, `kp`). |

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
    fn clone(self: &Self) -> KernelCall { /* ... */ }
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
    fn eq(self: &Self, other: &KernelCall) -> bool { /* ... */ }
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
#### Enum `Termination`

Why a particle stopped.

```rust
pub enum Termination {
    Active,
    Stopped,
    SmallMass,
    Age,
}
```

##### Variants

###### `Active`

Still active; next due at `itime + lsynctime`.

###### `Stopped`

`advance` returned `nstop > 1` (left the domain or the top).

###### `SmallMass`

Carries less than [`MINMASS`] of its initial mass.

###### `Age`

Reached `lage(nageclass)`.

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
    fn clone(self: &Self) -> Termination { /* ... */ }
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
    fn eq(self: &Self, other: &Termination) -> bool { /* ... */ }
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
#### Struct `PostAdvance`

What [`post_advance`] did.

```rust
pub struct PostAdvance {
    pub termination: Termination,
    pub xmassfract: Option<f64>,
    pub drydeposit_assigned: Vec<bool>,
    pub kernel_calls: Vec<KernelCall>,
    pub initial_cond_calls: Vec<i64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `termination` | `Termination` | The termination decided last (`Age` overrides `SmallMass`, as the<br>age test runs after it). |
| `xmassfract` | `Option<f64>` | `xmassfract`, `None` when `nstop > 1` (not computed). |
| `drydeposit_assigned` | `Vec<bool>` | Which species had `drydeposit` assigned this call. |
| `kernel_calls` | `Vec<KernelCall>` | `drydepokernel` calls, in order. |
| `initial_cond_calls` | `Vec<i64>` | `initial_cond_calc` calls (their time argument), when `linit_cond >=<br>1`. |

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
    fn clone(self: &Self) -> PostAdvance { /* ... */ }
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
    fn eq(self: &Self, other: &PostAdvance) -> bool { /* ... */ }
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

#### Function `deposition_decay_due`

**Attributes:**

- `MustUse { reason: None }`

Line 264: the deposition grids decay when deposition is on, the run is
forward and `itime` is the middle of the averaging interval.

```rust
pub fn deposition_decay_due(dep: bool, itime: i64, loutnext: i64, ldirect: i64) -> bool { /* ... */ }
```

#### Function `decay_deposition_grid`

Lines 264-299: when [`ClockEvents::decay_deposition`] fires, every
deposition grid value of a species with `decay(ks) > 0` is multiplied by
`exp(-outstep*decay(ks))` (a full output step: the deposits were
back-dated to the previous decay epoch, see [`post_advance`]). Apply to
the mother grids and, with nested output, to the nest grids.

`decay[ks]` in s⁻¹; `outstep = |loutstep|` s. Species with a
non-positive constant are left unchanged.

```rust
pub fn decay_deposition_grid(grid: &mut crate::flexpart::concentration::DepositionGrid, decay: &[f64], outstep: f64) { /* ... */ }
```

#### Function `split_particles`

Lines 468-499: when `ldirect*itime >= ldirect*itsplit`, every particle
(of those present on entry) with `ldirect*itime >= ldirect*itrasplit` is
split in two while fewer than `maxpart` exist: its next split time
doubles its age at split (`itrasplit = 2*(itrasplit - itramem) +
itramem`), both halves carry half the mass, and the copy is appended.
Only called at output times (see the module doc).

```rust
pub fn split_particles(parts: &mut Vec<SplitParticle>, itime: i64, ldirect: i64, itsplit: i64, maxpart: usize) { /* ... */ }
```

#### Function `particle_due`

**Attributes:**

- `MustUse { reason: None }`

Line 532: a particle is integrated at `itime` iff `itra1 == itime`
(terminated particles carry [`TERMINATED`]).

```rust
pub fn particle_due(itra1: i64, itime: i64) -> bool { /* ... */ }
```

#### Function `pre_advance`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Lines 534-593, before `advance`. `itra1`/`itramem` and `npoint`
(1-based) are the particle's; `xmass1` and `xscav_frac1` its per-species
arrays (updated in place). `prob_rec` is what `get_vdep_prob` returns at
the particle ([`crate::flexpart::advance::get_vdep_prob`]), `wetscav` per
species what `get_wetscav` returns
([`crate::flexpart::wet_deposition::get_wetscav`]); `zpoint1`/`zpoint2`
the release's bottom and top, m.

Backward receptor modes: a species whose `xscav_frac1` is still negative
gets its scavenged fraction (dry: `prob_rec`; wet: `wetscav * (zpoint2 -
zpoint1) * grfraction(1)`), or, with no dry deposition for it / no wet
scavenging, its mass set to 0.

```rust
pub fn pre_advance(set: &PreAdvanceSettings, itime: i64, itra1: i64, itramem: i64, npoint: usize, xmass1: &mut [f64], xscav_frac1: &mut [f64], prob_rec: &[f64], wetscav: &[WetScavResult], zpoint1: f64, zpoint2: f64) -> PreAdvance { /* ... */ }
```

#### Function `post_advance`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Lines 625-703, after `advance`.

`nstop` and `prob` (per species dry-deposition probability) come from
`advance`; `ldeltat` from [`ClockEvents::ldeltat`]; `nage` and `kp`
(1-based) from [`pre_advance`]; `xmass_release` is `xmass(npoint(j),
:)` and `npart` the release's particle count. `drydeposit` is
timemanager's per-species buffer, kept by the caller between particles
(see the module doc).

Per species: `decfact = exp(-|lsynctime| decay)` for `decay > 0`; with
dry deposition `drydeposit = xmass1*prob*decfact`, `xmass1 =
xmass1*(1-prob)*decfact`, and for a decaying species the deposit is
multiplied by `exp(|ldeltat| decay)` (back-dated to the last decay of the
deposition grids, which [`decay_deposition_grid`] then applies in full).

```rust
pub fn post_advance(set: &PostAdvanceSettings, p: &mut StepParticle, itime: i64, nstop: i32, prob: &[f64], ldeltat: i64, nage: usize, kp: usize, xmass_release: &[f64], npart: i64, drydeposit: &mut [f64]) -> PostAdvance { /* ... */ }
```

### Constants and Statics

#### Constant `MINMASS`

`minmass = 0.0001` (`par_mod.f90`): particles carrying less than this
fraction of their initial mass (all species) are terminated.

```rust
pub const MINMASS: f64 = 0.0001;
```

#### Constant `TERMINATED`

The value upstream stores in `itra1` of a terminated particle.

```rust
pub const TERMINATED: i64 = -999_999_999;
```

## Module `turbulence`

Boundary-layer turbulence statistics after Hanna (1982): the velocity
standard deviations `sigma_u`, `sigma_v`, `sigma_w`, the vertical gradient of
`sigma_w`, and the Lagrangian time scales `T_Lu`, `T_Lv`, `T_Lw` that drive
FLEXPART's Langevin equation.

# Three variants, and why

| Routine | Used by `advance.f90` when | Gradient it returns |
|---|---|---|
| [`hanna`] | Gaussian turbulence (`turb_option` default) | `d sigma_w / dz` |
| [`hanna1`] | the well-mixed scheme with the density correction | `d sigma_w^2 / dz` |
| [`hanna_short`] | the short inner time step, `w` only | `d sigma_w / dz` |

Each splits on stability in the same order: **neutral** when
`h / |L| < 1`, else **unstable** when `L < 0`, else **stable**.

# State is shared, exactly as upstream shares it

Upstream keeps all of these quantities in the module `hanna_mod`, which the
three routines read and write and `advance.f90` reads afterwards. Two outputs
depend on what an *earlier* call left there, and the port preserves that
rather than tidying it away, because `advance.f90` relies on the call order:

* [`hanna_short`] applies `max(10, tlu)` and `max(10, tlv)` to time scales it
  never computes — they are whatever the last full [`hanna`] call produced.
* [`hanna1`] assigns no `sigma_w` or `d sigma_w^2/dz` for an unstable layer at
  `zeta >= 1`, so those keep their previous values (clamped to `>= 1e-6`).

[`HannaState`] is that module, one struct per particle; the code-to-code
fixture sets it to sentinels before every call so both behaviours are
verified, not just tolerated.

# NaN, and Fortran `MAX`

In an unstable layer above the mixing height (`zeta > 1.11`), `hanna` and
`hanna_short` take the square root of a negative number, so `sigma_w` and
its gradient are NaN, and upstream does not guard against it. The port
returns the same NaN. The floors that follow (`max(30, tlw)` etc.) then see
a NaN. gfortran's `MAX` returns the non-NaN operand there, and so does
`f64::max`, so the floors are plain `f64::max`. That is exercised by the 20
fixture rows where this happens: `tlw` comes out exactly `30` in both
codes.

# Units

Bare `f64` in FLEXPART's units, as throughout this module (see
[`super::surface_layer`]): velocities m/s, lengths m, times s.

```rust
pub mod turbulence { /* ... */ }
```

### Types

#### Struct `HannaState`

The `hanna_mod` turbulence state for one particle.

Inputs the caller sets before a call: `ust`, `wst`, `ol`, `h`, `zeta`.
Everything else is output. Field names follow upstream so the port can be
read side by side with `hanna*.f90` and `advance.f90`.

```rust
pub struct HannaState {
    pub ust: f64,
    pub wst: f64,
    pub ol: f64,
    pub h: f64,
    pub zeta: f64,
    pub sigu: f64,
    pub sigv: f64,
    pub sigw: f64,
    pub dsigwdz: f64,
    pub dsigw2dz: f64,
    pub tlu: f64,
    pub tlv: f64,
    pub tlw: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ust` | `f64` | Friction velocity `u*`, m/s. Clamped to `>= 1e-4` in the neutral branch,<br>and the clamped value is written back (upstream mutates the module). |
| `wst` | `f64` | Convective velocity scale `w*`, m/s. |
| `ol` | `f64` | Obukhov length `L`, m (negative = unstable). |
| `h` | `f64` | Mixing height `h`, m. |
| `zeta` | `f64` | `z / h`, dimensionless. Set by the caller, not by these routines. |
| `sigu` | `f64` | Along-wind velocity standard deviation, m/s. |
| `sigv` | `f64` | Cross-wind velocity standard deviation, m/s. |
| `sigw` | `f64` | Vertical velocity standard deviation, m/s. |
| `dsigwdz` | `f64` | `d sigma_w / dz`, 1/s (set by [`hanna`] and [`hanna_short`]). |
| `dsigw2dz` | `f64` | `d sigma_w^2 / dz`, m/s² (set by [`hanna1`]). |
| `tlu` | `f64` | Lagrangian time scale for `u`, s (floor 10 s). |
| `tlv` | `f64` | Lagrangian time scale for `v`, s (floor 10 s). |
| `tlw` | `f64` | Lagrangian time scale for `w`, s (floor 30 s). |

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
    fn clone(self: &Self) -> HannaState { /* ... */ }
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
    fn default() -> HannaState { /* ... */ }
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
    fn eq(self: &Self, other: &HannaState) -> bool { /* ... */ }
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

#### Function `hanna`

Hanna (1982) turbulence for the Gaussian scheme (`hanna.f90`).

# Arguments
- `state` — the per-particle `hanna_mod` state; `ust`, `wst`, `ol`, `h` and
  `zeta` must be set. Outputs are written into it.
- `z` — particle height above ground, m.

# Notes
`d sigma_w / dz` is never left at exactly zero: upstream replaces `0` by
`1e-10`, because `advance.f90` divides by it.

```rust
pub fn hanna(state: &mut HannaState, z: f64) { /* ... */ }
```

#### Function `hanna1`

Hanna (1982) turbulence for the density-corrected well-mixed scheme
(`hanna1.f90`). Returns `d sigma_w^2 / dz` in `state.dsigw2dz` rather than
`d sigma_w / dz`.

# Arguments
As [`hanna`].

# The `zeta >= 1` carry-over
In an unstable layer the `sigma_w` profile is defined piecewise for
`zeta < 0.03`, `< 0.4`, `< 0.96` and `< 1.0`. Above that, upstream assigns
nothing, so `sigw` and `dsigw2dz` keep the values the state already held —
then `sigw` is clamped to `>= 1e-6`. Ported as is; see the module docs.

```rust
pub fn hanna1(state: &mut HannaState, z: f64) { /* ... */ }
```

#### Function `hanna_short`

Vertical-only Hanna turbulence for the short inner time step
(`hanna_short.f90`): `sigma_w`, `d sigma_w/dz` and `T_Lw`.

# Arguments
As [`hanna`].

# Stale `tlu` / `tlv`
Upstream ends with `tlu = max(10, tlu)` and `tlv = max(10, tlv)` although it
computes neither, so they keep — floored — whatever the state held. The port
reproduces this; see the module docs.

```rust
pub fn hanna_short(state: &mut HannaState, z: f64) { /* ... */ }
```

#### Function `windalign`

**Attributes:**

- `MustUse { reason: None }`

Rotate along-wind and cross-wind turbulent velocities into the grid's `u`
and `v` directions (`windalign.f90`).

# Arguments
- `u`, `v` — the mean wind components, m/s, which define the rotation.
- `ffap` — the along-wind turbulent component, m/s.
- `ffcp` — the cross-wind turbulent component, m/s.

# Returns
`(ux, vy)`, the turbulent components on the grid axes, m/s. A calm wind
(`|V| < 1e-30`) is floored to `1e-30` before dividing, so the direction is
then that of the (tiny) wind rather than undefined.

```rust
pub fn windalign(u: f64, v: f64, ffap: f64, ffcp: f64) -> (f64, f64) { /* ... */ }
```

## Module `verttransform`

Transformation of the raw model-level meteorology to FLEXPART's internal
Cartesian `z` grid: the `verttransform_*` routines that `getfields.f90`
calls once per wind field read.

For every column the routines integrate the hypsometric equation over the
model levels (virtual temperature, `const = r_air/ga`), interpolate the
horizontal wind, temperature, humidity, potential vorticity, density (and
cloud water) linearly in height onto the fixed reference heights
`height(1..nz)`, convert the vertical velocity from pressure units to m/s
with `pinmconv = dz/dp` and add the terrain-slope correction
`dz/dx u + dz/dy v`, compute `drhodz`, rotate the winds into the
polar-stereographic grids near the poles (via [`cmapf::cc2gll`]), and
diagnose the cloud / precipitation-type field `clouds` that the wet
scavenging reads.

| Rust | Upstream |
|---|---|
| [`verttransform_ecmwf`] | `verttransform_ecmwf.f90` (hybrid eta levels, `eta-dot` in Pa/s) |
| [`verttransform_gfs`] | `verttransform_gfs.f90` (NCEP pressure levels, `omega` in Pa/s) |
| [`verttransform_nests`] | `verttransform_nests.f90` (ECMWF nests, no polar part) |

# State, explicitly

Upstream reads and writes `com_mod` and keeps state between calls; the
port passes all of it explicitly:

* [`RawFields`] — the raw level fields `readwind` fills (`uuh`, `vvh`,
  `pvh`, `wwh` arguments; `tth`, `qvh`, `clwch`, `ciwch`, `ps`, `tt2`,
  `td2`, `lsprec`, `convprec` from `com_mod` at time slot `n`).
* [`ZFields`] — the `com_mod` output arrays of one time slot (`uu`, `vv`,
  `ww`, `tt`, `qv`, `pv`, `rho`, `drhodz`, `prs`, `pplev`, `uupol`,
  `vvpol`, `clwc`, `ciwc`, `clw`, `clouds`, `cloudsh`, `ctwc`). It is
  **in/out**: several outputs are only partly rewritten (see the quirks),
  so the caller must pass the previous contents of that slot, as upstream's
  `com_mod` would hold them. `interpolation::MetFields` has no `qv`, `pv`,
  `prs`, cloud fields, so it cannot hold everything; [`ZFields::copy_into_met`]
  copies the fields `MetFields` does have.
* [`ZGrid`] — `com_mod`'s `height` and `nmixz`, which the ECMWF and GFS
  routines set **on their first call only** and every routine reads.
* [`SavedInit`] (ECMWF) and [`GfsSaved`] (GFS) — the routines' own `SAVE`d
  state: the `init` flag, and for GFS the static array `uvwzlev`, whose
  levels below a column's first above-ground level are never rewritten.

Field storage reuses [`Field2`]/[`Field3`] (`ix` fastest, levels 0-based:
upstream level `k` is index `k-1`). A field may be **larger** than the grid
(like upstream's `0:nxmax-1` arrays): elements are addressed by
`(ix, jy, k)`, so values outside the active grid persist untouched between
calls on grids of different size, exactly as in `com_mod`.

# Units

FLEXPART's: Pa, K, kg/kg, m, m/s, kg/m³, kg/m⁴ (`drhodz`); `wwh` in Pa/s
(ECMWF `eta-dot` is already multiplied by `dp/deta` in `readwind`); the
output `ww` in m/s. Precipitation in mm/h. `cloudsh` in whole metres
(upstream `integer`).

# Translation notes

The arithmetic follows upstream left to right (`a*b*c/d` is
`((a*b)*c)/d`), so a `-fdefault-real-8` build agrees bit for bit. The
virtual-temperature factor of the reference column, `tt2*(1+0.378*ew(td2)/ps)`,
calls [`thermo::ew_kelvin`]; the old cloud scheme calls
[`boundary_layer::f_qvsat`]; both are the verified ports.

# Upstream quirks reproduced (each pinned by the code-to-code fixture)

1. **Heights are set once, from the first field ever read.** `init` is a
   `SAVE`d logical. The reference profile `height(1..nuvz)` is integrated at
   the first column (`jy` outer, `ix` inner) with `ps > 100000 Pa`, and
   `nmixz` is the first level above `hmixmax = 4500 m`. Every later call
   reuses them, whatever its surface pressures. The ECMWF and GFS
   routines each have their **own** `init`, but write the **same**
   `com_mod` `height`: the second routine to be called for the first time
   overwrites the first's heights. If no level exceeds `hmixmax`, `nmixz`
   keeps whatever it held. If no column has `ps > 100000 Pa`, upstream
   reads undefined `ixm, jym`; the port returns
   [`VertError::NoReferenceColumn`].
2. **`cloudsh` is an `integer`**: `cloudsh = cloudsh + height(kz) -
   height(kz-1)` is evaluated in `real` and **truncated** on assignment,
   at every layer, so the sum loses up to 1 m per layer.
3. **The cloud-water scheme (`readclouds`) never resets `cloudsh`**: it
   accumulates on whatever the slot held (the old scheme resets it to 0
   per column). The nest version accumulates on the never-initialised
   `allocate`d `cloudshn`.
4. **`cloudh_min` is the bottom of the *highest* cloud layer, and is stale
   across columns.** It is assigned at every cloudy level scanning
   *upwards*, so the last assignment wins; a column with no cloud water but
   with precipitation reuses the previous column's (or previous nest's)
   value. It is a non-`SAVE` local, undefined before the first assignment
   in a call: the port returns [`VertError::UndefinedCloudBase`] if it
   would be read then.
5. **The old cloud scheme never writes `clouds` at level 1**, and the
   cloud-water scheme leaves level 1 at the 0 it reset everything to; the
   nest cloud-water scheme instead runs to level 1 and reads `height(0)`,
   **outside the array**, when level 1 is cloudy in a precipitating column
   ([`VertError::NestHeightZero`]).
6. **The south-pole wind is rotated with the NORTH-pole map**:
   `call cc2gll(northpolemap, -90., 180., ...)` in both ECMWF and GFS. On a
   grid that holds the South Pole but not the North Pole, `northpolemap` is
   never set (`gridcheck` only sets it for `nglobal`) and is all zeros, so
   the pole row's `uupol`/`vvpol` are 0.
7. **GFS south pole, `vv > 0`: `ddpol = pi + atan(u/v) - xlonr`**, where
   the ECMWF routine (and both other GFS branches) have `+ xlonr`.
8. **`uupol`/`vvpol` are only written in the polar bands** (`jy >=
   int(switchnorthg)-2`, `jy <= int(switchsouthg)+3`); every other row keeps
   the slot's previous values. Where the two bands overlap, the south pass
   (run second) wins.
9. **Linear extrapolation with a stale bracket.** ECMWF/nests: when a
   reference height lies above the column's top `w` level (or, in the slope
   correction, above its top `u` level), the bracket search fails and the
   previous level's bracket index is reused, so the value is extrapolated.
   GFS: when the slope-correction search fails, `kl`, `klp`, `dz1`, `dz2`,
   `dz` keep their last values (from the previous level, the previous
   column, or the `w` interpolation of the last column); the port carries
   them exactly, and returns [`VertError::UndefinedSlopeBracket`] if `kl`
   was never assigned in the call.
10. **GFS `ww` levels the bracket search misses keep the slot's previous
    `ww`** (no extrapolation in GFS).
11. **GFS reads stale `uvwzlev` at neighbours.** `uvwzlev` is a static
    local, rewritten only from each column's first above-ground level
    `llev` upward; the slope correction reads neighbouring columns at the
    centre column's levels, which may lie below the neighbour's `llev`, so
    it reads that neighbour's values from an earlier call (0 before the
    first). [`GfsSaved`] carries the array.
12. **GFS `llev` is capped at `nuvz-2`** even when more levels are below
    ground; the heights then start from that level's pressure `akz(llev)`,
    not from `ps`.
13. **`pinmconv` at level 1 divides `uvzlev(2)` by `p(2)-p(1)`** (one-sided)
    and the `ww` of level `nz` set before the loop is overwritten by the
    `iz = nz` interpolation (ECMWF/nests).
14. **ECMWF/nests: with `readclouds` and not `sumclouds`, `clwc` is
    overwritten in place by `clwc + ciwc`** (all levels), so the stored
    `clwc` is total condensate.
15. **The `virr` test counter** and its never-executed file output
    (`if (1.eq.2)`) have no effect and are not ported.

# Where the port deliberately differs

Upstream's whole-array statements (`uu(:,:,1,n)=uuh(:,:,1)`,
`clw(:,:,:,n)=0.`, `clwc = clwc + ciwc`, ...) run over the full
`0:nxmax-1 x 0:nymax-1` (and, for the cloud resets, `nzmax`) extent,
copying whatever the raw arrays hold outside the active grid. The port
writes only the active `nx x ny x nz` grid. The difference is visible only
if a later call on a *larger* grid reads a stale value outside the earlier
grid; the code-to-code driver keeps each slot's grid size fixed within a
process, so it is not exercised there.

```rust
pub mod verttransform { /* ... */ }
```

### Types

#### Enum `VertError`

Why a `verttransform_*` port refused. Each is an input on which upstream
reads an undefined value or writes outside an array.

```rust
pub enum VertError {
    ShapeMismatch,
    NoReferenceColumn,
    PolarBandOutOfGrid,
    UndefinedCloudBase,
    UndefinedSlopeBracket,
    NestHeightZero,
}
```

##### Variants

###### `ShapeMismatch`

A field is smaller than the grid it is used on, or `nz != nuvz`, or
`nwz > nuvz` / `nwz < 2`, or `nuvz < 3`.

###### `NoReferenceColumn`

First call, and no column has `ps > 100000 Pa`: upstream integrates the
reference profile at undefined indices `ixm, jym`.

###### `PolarBandOutOfGrid`

A polar band starts below row 0 (`int(switchnorthg)-2 < 0`) or ends
above row `ny-1` (`int(switchsouthg)+3 > ny-1`): upstream indexes
outside the grid.

###### `UndefinedCloudBase`

The precipitation-type diagnostic needs `cloudh_min` before any
cloudy level has assigned it in this call (undefined upstream).

###### `UndefinedSlopeBracket`

GFS slope correction: the first bracket search of the call failed, so
upstream uses never-assigned `kl`, `klp`.

###### `NestHeightZero`

Nest cloud-water scheme: level 1 is cloudy in a precipitating column,
and upstream reads `height(0)`, outside the array.

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
    fn clone(self: &Self) -> VertError { /* ... */ }
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
    fn eq(self: &Self, other: &VertError) -> bool { /* ... */ }
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
#### Struct `ZGrid`

`com_mod`'s reference heights and PBL level cap.

```rust
pub struct ZGrid {
    pub height: Vec<f64>,
    pub nmixz: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `height` | `Vec<f64>` | `height(1..)`, m above ground, index `k-1`. At least `nz` long. |
| `nmixz` | `usize` | `nmixz`: the first level (1-based) whose height exceeds `hmixmax`. |

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
    fn clone(self: &Self) -> ZGrid { /* ... */ }
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
    fn eq(self: &Self, other: &ZGrid) -> bool { /* ... */ }
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
#### Struct `SavedInit`

A routine's `SAVE`d `init` flag (`logical :: init = .true.`).

```rust
pub struct SavedInit {
    pub init: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `init` | `bool` | `true` until the routine's first call has set `height` and `nmixz`. |

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
    fn clone(self: &Self) -> SavedInit { /* ... */ }
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
    fn eq(self: &Self, other: &SavedInit) -> bool { /* ... */ }
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
#### Struct `GfsSaved`

`verttransform_gfs`'s state between calls: its `init` flag and its static
local `uvwzlev(0:nxmax-1,0:nymax-1,nzmax)` (zero before the first call;
see quirk 11).

```rust
pub struct GfsSaved {
    pub init: bool,
    pub uvwzlev: super::met_fields::Field3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `init` | `bool` | The `init` flag. |
| `uvwzlev` | `super::met_fields::Field3` | `uvwzlev`, m; capacity fixed at construction (like `nxmax`, ...). |

##### Implementations

###### Methods

- ```rust
  pub fn new(nxmax: usize, nymax: usize, nzmax: usize) -> Self { /* ... */ }
  ```
  Fresh state, with room for grids up to `nxmax x nymax x nzmax`.

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
    fn clone(self: &Self) -> GfsSaved { /* ... */ }
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
    fn eq(self: &Self, other: &GfsSaved) -> bool { /* ... */ }
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
#### Struct `HybridCoefficients`

Hybrid coefficients from `gridcheck`: `akz`, `bkz` (length `>= nuvz`) and
`aknew`, `bknew` (length `>= nz`), index `k-1`. Level pressure
`p = a + b*ps`, Pa. For GFS `akz` holds the pressure levels and `bkz` is 0.

```rust
pub struct HybridCoefficients {
    pub akz: Vec<f64>,
    pub bkz: Vec<f64>,
    pub aknew: Vec<f64>,
    pub bknew: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `akz` | `Vec<f64>` | `akz`, Pa. |
| `bkz` | `Vec<f64>` | `bkz`. |
| `aknew` | `Vec<f64>` | `aknew`, Pa. |
| `bknew` | `Vec<f64>` | `bknew`. |

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
    fn clone(self: &Self) -> HybridCoefficients { /* ... */ }
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
    fn eq(self: &Self, other: &HybridCoefficients) -> bool { /* ... */ }
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
#### Struct `VertGrid`

The grid a routine works on.

```rust
pub struct VertGrid {
    pub geom: super::met_fields::GridGeometry,
    pub dxconst: f64,
    pub dyconst: f64,
    pub nuvz: usize,
    pub nwz: usize,
    pub nz: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `geom` | `super::met_fields::GridGeometry` | Horizontal geometry (`nx`, `ny`, `dx`, `dy`, `xlon0`, `ylat0`). |
| `dxconst` | `f64` | `dxconst = 180/(dx r_earth pi)` (rad/m per degree), from `gridcheck`. |
| `dyconst` | `f64` | `dyconst`. |
| `nuvz` | `usize` | Levels of `u`, `v`, `T`, `q` (`nuvz`). |
| `nwz` | `usize` | Levels of `w` (`nwz`); `gridcheck` gives `nwz <= nuvz`. |
| `nz` | `usize` | Output levels (`nz`); `gridcheck` sets `nz = nuvz`, which the port<br>requires (upstream would read unset levels otherwise). |

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
    fn clone(self: &Self) -> VertGrid { /* ... */ }
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
    fn eq(self: &Self, other: &VertGrid) -> bool { /* ... */ }
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
#### Struct `PolarCaps`

`com_mod`'s polar-grid settings.

```rust
pub struct PolarCaps {
    pub nglobal: bool,
    pub sglobal: bool,
    pub switchnorthg: f64,
    pub switchsouthg: f64,
    pub northpolemap: super::cmapf::Strcmp,
    pub southpolemap: super::cmapf::Strcmp,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nglobal` | `bool` | Row `ny-1` is the North Pole. |
| `sglobal` | `bool` | Row 0 is the South Pole. |
| `switchnorthg` | `f64` | `switchnorthg`, grid units. |
| `switchsouthg` | `f64` | `switchsouthg`, grid units. |
| `northpolemap` | `super::cmapf::Strcmp` | `northpolemap` (all zeros unless `gridcheck` set it; see quirk 6). |
| `southpolemap` | `super::cmapf::Strcmp` | `southpolemap`. |

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
    fn clone(self: &Self) -> PolarCaps { /* ... */ }
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
    fn eq(self: &Self, other: &PolarCaps) -> bool { /* ... */ }
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
#### Struct `CloudScheme`

Cloud diagnostics selected by `readwind` (`readclouds`, `sumclouds`).

```rust
pub struct CloudScheme {
    pub readclouds: bool,
    pub sumclouds: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `readclouds` | `bool` | Cloud water was read from the GRIB file (`readclouds`); otherwise the<br>relative-humidity parameterisation is used. |
| `sumclouds` | `bool` | Liquid and ice were read already summed (`sumclouds`). |

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
    fn clone(self: &Self) -> CloudScheme { /* ... */ }
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
    fn default() -> CloudScheme { /* ... */ }
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
    fn eq(self: &Self, other: &CloudScheme) -> bool { /* ... */ }
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
#### Struct `RawFields`

The raw level fields of one time slot (what `readwind` provides).
3-d fields have `nuvz` levels (`wwh`: `nwz`), index `k-1`.

```rust
pub struct RawFields {
    pub uuh: super::met_fields::Field3,
    pub vvh: super::met_fields::Field3,
    pub pvh: super::met_fields::Field3,
    pub wwh: super::met_fields::Field3,
    pub tth: super::met_fields::Field3,
    pub qvh: super::met_fields::Field3,
    pub clwch: super::met_fields::Field3,
    pub ciwch: super::met_fields::Field3,
    pub ps: super::met_fields::Field2,
    pub tt2: super::met_fields::Field2,
    pub td2: super::met_fields::Field2,
    pub lsprec: super::met_fields::Field2,
    pub convprec: super::met_fields::Field2,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `uuh` | `super::met_fields::Field3` | `uuh`, m/s (level 1: 10-m wind). |
| `vvh` | `super::met_fields::Field3` | `vvh`, m/s. |
| `pvh` | `super::met_fields::Field3` | `pvh`, pvu. |
| `wwh` | `super::met_fields::Field3` | `wwh`, Pa/s. |
| `tth` | `super::met_fields::Field3` | `tth`, K (level 1: 2-m temperature). |
| `qvh` | `super::met_fields::Field3` | `qvh`, kg/kg. |
| `clwch` | `super::met_fields::Field3` | `clwch`, kg/kg (read only with `readclouds`). |
| `ciwch` | `super::met_fields::Field3` | `ciwch`, kg/kg (read only with `readclouds` and not `sumclouds`). |
| `ps` | `super::met_fields::Field2` | `ps`, Pa. |
| `tt2` | `super::met_fields::Field2` | `tt2`, K. |
| `td2` | `super::met_fields::Field2` | `td2`, K. |
| `lsprec` | `super::met_fields::Field2` | `lsprec`, mm/h. |
| `convprec` | `super::met_fields::Field2` | `convprec`, mm/h. |

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
    fn clone(self: &Self) -> RawFields { /* ... */ }
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
    fn eq(self: &Self, other: &RawFields) -> bool { /* ... */ }
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
#### Struct `ZFields`

The `com_mod` output arrays of one time slot. In/out: see the module docs.
`clouds` and `cloudsh` are upstream `integer(kind=1)` / `integer` arrays,
stored with the [`Field3`]/[`Field2`] index layout of `nx_cap`, `ny_cap`.

```rust
pub struct ZFields {
    pub uu: super::met_fields::Field3,
    pub vv: super::met_fields::Field3,
    pub ww: super::met_fields::Field3,
    pub tt: super::met_fields::Field3,
    pub qv: super::met_fields::Field3,
    pub pv: super::met_fields::Field3,
    pub rho: super::met_fields::Field3,
    pub drhodz: super::met_fields::Field3,
    pub prs: super::met_fields::Field3,
    pub pplev: super::met_fields::Field3,
    pub uupol: super::met_fields::Field3,
    pub vvpol: super::met_fields::Field3,
    pub clwc: super::met_fields::Field3,
    pub ciwc: super::met_fields::Field3,
    pub clw: super::met_fields::Field3,
    pub clouds: Vec<i8>,
    pub cloudsh: Vec<i32>,
    pub ctwc: super::met_fields::Field2,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `uu` | `super::met_fields::Field3` | `uu`, m/s. |
| `vv` | `super::met_fields::Field3` | `vv`, m/s. |
| `ww` | `super::met_fields::Field3` | `ww`, m/s. |
| `tt` | `super::met_fields::Field3` | `tt`, K. |
| `qv` | `super::met_fields::Field3` | `qv`, kg/kg. |
| `pv` | `super::met_fields::Field3` | `pv`, pvu. |
| `rho` | `super::met_fields::Field3` | `rho`, kg/m³. |
| `drhodz` | `super::met_fields::Field3` | `drhodz`, kg/m⁴. |
| `prs` | `super::met_fields::Field3` | `prs`, Pa (ECMWF only). |
| `pplev` | `super::met_fields::Field3` | `pplev`, Pa (GFS only). |
| `uupol` | `super::met_fields::Field3` | `uupol`, m/s (polar-stereographic grid component). |
| `vvpol` | `super::met_fields::Field3` | `vvpol`, m/s. |
| `clwc` | `super::met_fields::Field3` | `clwc`, kg/kg. |
| `ciwc` | `super::met_fields::Field3` | `ciwc`, kg/kg. |
| `clw` | `super::met_fields::Field3` | `clw`, m (column-integrated cloud water per layer, upstream "m3/m3"). |
| `clouds` | `Vec<i8>` | `clouds`: 0 none, 1 cloud, 2/3 convective/large-scale in-cloud,<br>4/5 convective/large-scale washout. |
| `cloudsh` | `Vec<i32>` | `cloudsh`, m (integer). |
| `ctwc` | `super::met_fields::Field2` | `ctwc`, total column cloud water. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn filled(nx: usize, ny: usize, nz: usize, real: f64, cloud: i8, cloudsh: i32) -> Self { /* ... */ }
  ```
  Arrays of capacity `nx x ny x nz`, every real set to `real`, `clouds`

- ```rust
  pub fn cloud(self: &Self, ix: usize, jy: usize, k: usize) -> i8 { /* ... */ }
  ```
  `clouds` at `(ix, jy, k)`, `k` 0-based.

- ```rust
  pub fn cloud_height(self: &Self, ix: usize, jy: usize) -> i32 { /* ... */ }
  ```
  `cloudsh` at `(ix, jy)`.

- ```rust
  pub fn copy_into_met(self: &Self, met: &mut MetFields, slot: usize) { /* ... */ }
  ```
  Copy the fields [`MetFields`] holds (`uu`, `vv`, `ww`, `uupol`,

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
    fn clone(self: &Self) -> ZFields { /* ... */ }
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
    fn eq(self: &Self, other: &ZFields) -> bool { /* ... */ }
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
#### Struct `NestInput`

One nest for [`verttransform_nests`].

```rust
pub struct NestInput {
    pub grid: VertGrid,
    pub xresoln: f64,
    pub yresoln: f64,
    pub clouds: CloudScheme,
    pub raw: RawFields,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `VertGrid` | The nest grid (`nxn`, `nyn`, `dxn`, `dyn`, `xlon0n`, `ylat0n`) with the<br>**mother** grid's `dxconst`, `dyconst`, `nuvz`, `nwz`, `nz`. |
| `xresoln` | `f64` | `xresoln(l)`. |
| `yresoln` | `f64` | `yresoln(l)`. |
| `clouds` | `CloudScheme` | `readclouds_nest(l)`, `sumclouds_nest(l)`. |
| `raw` | `RawFields` | Raw fields of the nest at slot `n`. |

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
    fn clone(self: &Self) -> NestInput { /* ... */ }
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
    fn eq(self: &Self, other: &NestInput) -> bool { /* ... */ }
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

#### Function `verttransform_ecmwf`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`verttransform_ecmwf(n, uuh, vvh, wwh, pvh)`: ECMWF hybrid levels to the
`z` grid, for one time slot.

* `saved` — the routine's `SAVE`d `init` (start with
  [`SavedInit::default`]).
* `zgrid` — `com_mod` `height`/`nmixz`: written on the first call, read
  always.
* `grid`, `hyb`, `polar` — the `gridcheck` settings.
* `clouds` — `readclouds`, `sumclouds`.
* `raw` — the raw fields of slot `n`.
* `out` — slot `n` of the `com_mod` output arrays, in/out.

On `Err` nothing upstream would have computed is defined; `out` may be
partly written.

# Errors

See [`VertError`].

```rust
pub fn verttransform_ecmwf(saved: &mut SavedInit, zgrid: &mut ZGrid, grid: &VertGrid, hyb: &HybridCoefficients, polar: &PolarCaps, clouds: CloudScheme, raw: &RawFields, out: &mut ZFields) -> Result<(), VertError> { /* ... */ }
```

#### Function `verttransform_nests`

`verttransform_nests(n, uuhn, vvhn, wwhn, pvhn)`: every nest `l =
1..numbnests` in order (`cloudh_min` carries from one nest to the next,
quirk 4). `outs[l]` is nest `l`'s slot `n`; its `prs`, `pplev`, `uupol`,
`vvpol` are not touched. Reads `height` (set by the mother-grid routine);
no `init`, no polar part.

# Errors

See [`VertError`]; also [`VertError::ShapeMismatch`] if `outs` is
shorter than `nests`.

```rust
pub fn verttransform_nests(height: &[f64], hyb: &HybridCoefficients, nests: &[NestInput], outs: &mut [ZFields]) -> Result<(), VertError> { /* ... */ }
```

#### Function `verttransform_gfs`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`verttransform_gfs(n, uuh, vvh, wwh, pvh)`: NCEP pressure levels to the
`z` grid, for one time slot. Arguments as [`verttransform_ecmwf`], with
[`GfsSaved`] for the `SAVE`d state and `pplev` written instead of `prs`.

# Errors

See [`VertError`].

```rust
pub fn verttransform_gfs(saved: &mut GfsSaved, zgrid: &mut ZGrid, grid: &VertGrid, hyb: &HybridCoefficients, polar: &PolarCaps, clouds: CloudScheme, raw: &RawFields, out: &mut ZFields) -> Result<(), VertError> { /* ... */ }
```

## Module `wet_deposition`

Wet deposition: precipitation interpolation, below- and in-cloud
scavenging coefficients, the per-particle mass loss and its attribution
to the output grid.

| Function | Upstream | Computes |
|---|---|---|
| [`interpol_rain`] | `interpol_rain.f90`, `interpol_rain_nests.f90` | bilinear `lsp`, `convp`, `tcc` at a point |
| [`get_wetscav`] | `get_wetscav.f90` | scavenging coefficient `Lambda`, 1/s, and the precipitating fraction |
| [`wetdepo`] | `wetdepo.f90` | mass removed from every particle, gridded |
| [`wetdepokernel`] | `wetdepokernel.f90`, `wetdepokernel_nest.f90` | uniform-kernel attribution to the output grid |

Each `_nests` routine is a copy of the mother-grid routine on the nest
arrays; the port has one function that takes the grid, and is verified
against both. `wetdepokernel_nest` differs in two ways, selected by
[`KernelRule`]: it uses `floor` where the mother uses `int` (truncation),
and it always applies the kernel (it ignores `lusekerneloutput`).

# Inputs upstream reads from `com_mod`

Precipitation, cloud cover and cloud codes, temperature, cloud water,
level heights, the nest frames, species parameters and the particle
arrays. The port takes them as arguments ([`WetScavMet`],
[`WetScavSpecies`], [`ParticleRecord`]). No random numbers are drawn.

# Upstream quirks reproduced (each documented where it applies)

- `interpol_rain` **moves its `xt`, `yt` arguments in place** when they
  reach the last grid line; [`RainAtPoint`] returns the moved values. It
  does no temporal interpolation ("skip to be consistent with clouds"):
  it reads the single field nearest in time.
- `get_wetscav` picks the field by `nint(itime - 0.5 ltsample)` (half
  away from zero), the earlier one only if strictly closer.
- `get_wetscav` **sets a negative `ccn_aero` or `in_aero` to zero in
  `com_mod`** the first time a particle of that species is in cloud: the
  port takes the species `&mut` and does the same.
- Rain vs snow below cloud: a species with `crain_aero <= 0` but
  `csnow_aero > 0` above 273 K (or the reverse below) is **counted** as
  below-cloud scavenging but scavenged at `Lambda = 0`.
- `liq_frac + ice_frac != 1` between 253 and 273 K:
  `ice = ((T-273)/20)^2`, `liq = max(0, 1 - ice)` (upstream's v10.4 fix).
- The cloud height `cloudsh` is read and never used.
- `wetdepo`: see [`wetdepo`] for the stale-deposit and age-class defects.

# Units

Precipitation mm/h, temperatures K, `dquer` µm, masses as the caller's
(kg), times s, grid coordinates in grid units.

```rust
pub mod wet_deposition { /* ... */ }
```

### Types

#### Struct `RainAtPoint`

What [`interpol_rain`] returns.

```rust
pub struct RainAtPoint {
    pub lsp: f64,
    pub convp: f64,
    pub cc: f64,
    pub xt: f64,
    pub yt: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lsp` | `f64` | Large-scale precipitation, mm/h. |
| `convp` | `f64` | Convective precipitation, mm/h. |
| `cc` | `f64` | Total cloud cover, fraction. |
| `xt` | `f64` | `xt` after upstream's in-place move off the last grid line. |
| `yt` | `f64` | `yt` after the same move. |

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
    fn clone(self: &Self) -> RainAtPoint { /* ... */ }
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
    fn eq(self: &Self, other: &RainAtPoint) -> bool { /* ... */ }
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
#### Struct `CloudField`

Cloud codes on the model levels (`clouds` / `cloudsn`, `integer(kind=1)`):
0 no cloud, 1 cloud without precipitation, 2/3 in-cloud (convective /
large-scale dominated), 4/5 below-cloud. Element `(ix, jy, k)` is
`data[ix + nx*(jy + ny*k)]`, `k` 0-based (upstream level `k+1`).

```rust
pub struct CloudField {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub data: Vec<i8>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Points in x. |
| `ny` | `usize` | Points in y. |
| `nz` | `usize` | Levels. |
| `data` | `Vec<i8>` | Codes. |

##### Implementations

###### Methods

- ```rust
  pub fn at(self: &Self, ix: usize, jy: usize, k: usize) -> i8 { /* ... */ }
  ```
  Code at `(ix, jy, k)`, `k` 0-based.

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
    fn clone(self: &Self) -> CloudField { /* ... */ }
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
    fn eq(self: &Self, other: &CloudField) -> bool { /* ... */ }
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
#### Struct `WetFields`

The wet-deposition fields of one grid at one time.

```rust
pub struct WetFields {
    pub lsprec: super::met_fields::Field2,
    pub convprec: super::met_fields::Field2,
    pub tcc: super::met_fields::Field2,
    pub clouds: CloudField,
    pub tt: super::met_fields::Field3,
    pub ctwc: super::met_fields::Field2,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lsprec` | `super::met_fields::Field2` | Large-scale precipitation, mm/h (`lsprec`). |
| `convprec` | `super::met_fields::Field2` | Convective precipitation, mm/h (`convprec`). |
| `tcc` | `super::met_fields::Field2` | Total cloud cover, fraction (`tcc`). |
| `clouds` | `CloudField` | Cloud codes (`clouds`). |
| `tt` | `super::met_fields::Field3` | Temperature on the model levels, K (`tt`). |
| `ctwc` | `super::met_fields::Field2` | Total cloud water content (`ctwc`), used only when clouds were read. |

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
    fn clone(self: &Self) -> WetFields { /* ... */ }
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
    fn eq(self: &Self, other: &WetFields) -> bool { /* ... */ }
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
#### Struct `WetGrid`

One grid (mother or nest): the fields at the two times in memory, **in
time order** (upstream's `memind` indirection already applied: slot 0 is
the field valid at `memtime[0]`).

```rust
pub struct WetGrid {
    pub slots: [WetFields; 2],
    pub readclouds: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `slots` | `[WetFields; 2]` | Fields at `memtime[0]` and `memtime[1]`. |
| `readclouds` | `bool` | Cloud water was read from the met input (`readclouds` /<br>`readclouds_nest(l)`), so `ctwc` is used instead of the<br>parameterisation. |

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
    fn clone(self: &Self) -> WetGrid { /* ... */ }
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
    fn eq(self: &Self, other: &WetGrid) -> bool { /* ... */ }
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
#### Struct `NestFrame`

Location of a nest in mother-grid coordinates (`xln`, `xrn`, `yln`,
`yrn`) and its refinement (`xresoln`, `yresoln`).

```rust
pub struct NestFrame {
    pub xl: f64,
    pub xr: f64,
    pub yl: f64,
    pub yr: f64,
    pub xresol: f64,
    pub yresol: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xl` | `f64` | Left edge, mother grid units. |
| `xr` | `f64` | Right edge. |
| `yl` | `f64` | Lower edge. |
| `yr` | `f64` | Upper edge. |
| `xresol` | `f64` | Nest points per mother grid unit in x. |
| `yresol` | `f64` | Nest points per mother grid unit in y. |

##### Implementations

###### Methods

- ```rust
  pub fn select(nests: &[NestFrame], x: f64, y: f64) -> Option<usize> { /* ... */ }
  ```
  Upstream's nest selection: the highest-numbered nest whose open

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
    fn clone(self: &Self) -> NestFrame { /* ... */ }
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
    fn eq(self: &Self, other: &NestFrame) -> bool { /* ... */ }
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
#### Struct `WetScavMet`

Everything `get_wetscav` reads from `com_mod` besides the species.

```rust
pub struct WetScavMet {
    pub memtime: [i64; 2],
    pub height: Vec<f64>,
    pub mother: WetGrid,
    pub nests: Vec<(NestFrame, WetGrid)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `memtime` | `[i64; 2]` | Validity times of the two fields in memory, s (`memtime`). |
| `height` | `Vec<f64>` | Height of each model level, m (`height(1:nz)`). |
| `mother` | `WetGrid` | Mother grid. |
| `nests` | `Vec<(NestFrame, WetGrid)>` | Nests, with their frames, in upstream's order. |

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
    fn clone(self: &Self) -> WetScavMet { /* ... */ }
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
    fn eq(self: &Self, other: &WetScavMet) -> bool { /* ... */ }
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
#### Struct `WetScavSpecies`

Wet scavenging parameters of one species (`com_mod`, from the SPECIES
file).

```rust
pub struct WetScavSpecies {
    pub dquer: f64,
    pub weta_gas: f64,
    pub wetb_gas: f64,
    pub crain_aero: f64,
    pub csnow_aero: f64,
    pub ccn_aero: f64,
    pub in_aero: f64,
    pub henry: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dquer` | `f64` | Mean particle diameter, µm (`dquer`); `<= 0` marks a gas. |
| `weta_gas` | `f64` | Below-cloud gas coefficient `A`, 1/s (`weta_gas`). |
| `wetb_gas` | `f64` | Below-cloud gas exponent `B` (`wetb_gas`). |
| `crain_aero` | `f64` | Below-cloud rain efficiency for aerosol (`crain_aero`). |
| `csnow_aero` | `f64` | Below-cloud snow efficiency for aerosol (`csnow_aero`). |
| `ccn_aero` | `f64` | In-cloud CCN efficiency (`ccn_aero`); negative = off. |
| `in_aero` | `f64` | In-cloud ice-nuclei efficiency (`in_aero`); negative = off. |
| `henry` | `f64` | Henry's constant, M/atm (`henry`). |

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
    fn clone(self: &Self) -> WetScavSpecies { /* ... */ }
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
    fn eq(self: &Self, other: &WetScavSpecies) -> bool { /* ... */ }
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
#### Enum `ScavengingRegime`

Which counter a scavenging event incremented.

```rust
pub enum ScavengingRegime {
    BelowCloud,
    InCloud,
}
```

##### Variants

###### `BelowCloud`

`blc_count`.

###### `InCloud`

`inc_count`.

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
    fn clone(self: &Self) -> ScavengingRegime { /* ... */ }
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
    fn eq(self: &Self, other: &ScavengingRegime) -> bool { /* ... */ }
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
#### Struct `WetScav`

What [`get_wetscav`] returns.

```rust
pub struct WetScav {
    pub wetscav: f64,
    pub grfraction: Option<f64>,
    pub counted: Option<ScavengingRegime>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `wetscav` | `f64` | Scavenging coefficient, 1/s (`wetscav`); 0 when nothing applies. |
| `grfraction` | `Option<f64>` | Precipitating fraction of the cell (`grfraction(1)`); `None` where<br>upstream returns before assigning it. |
| `counted` | `Option<ScavengingRegime>` | The counter upstream incremented, if any. |

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
    fn clone(self: &Self) -> WetScav { /* ... */ }
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
    fn eq(self: &Self, other: &WetScav) -> bool { /* ... */ }
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
#### Enum `WetDepError`

Why a wet-deposition routine refused.

```rust
pub enum WetDepError {
    AboveTopLevel,
    OutsideGrid,
    UndefinedDeposit,
    AgeBeyondLastClass,
    ClassOutOfRange,
}
```

##### Variants

###### `AboveTopLevel`

The particle is at or above the top level `height(nz)` while it is
raining; upstream's level index `hz` is then never assigned.

###### `OutsideGrid`

The particle's cell lies outside the field arrays.

###### `UndefinedDeposit`

`wetdepo` would grid a deposit it never computed (a species with
`WETDEPSPEC` false, forward run): upstream reads a stale local.

###### `AgeBeyondLastClass`

The particle is older than the last age class; upstream's `nage` is
`nageclass + 1` and the kernel writes outside the grid array.

###### `ClassOutOfRange`

A release point or uncertainty class outside the deposition grid.

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
    fn clone(self: &Self) -> WetDepError { /* ... */ }
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
    fn eq(self: &Self, other: &WetDepError) -> bool { /* ... */ }
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
#### Struct `ParticleRecord`

A particle as `wetdepo` and `ohreaction` see it (`com_mod`'s particle
arrays at one index).

```rust
pub struct ParticleRecord {
    pub itra1: i64,
    pub itramem: i64,
    pub xtra1: f64,
    pub ytra1: f64,
    pub ztra1: f64,
    pub npoint: usize,
    pub nclass: usize,
    pub xmass1: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `itra1` | `i64` | Time of the particle, s (`itra1`); `-999999999` = inactive. |
| `itramem` | `i64` | Release time, s (`itramem`). |
| `xtra1` | `f64` | Position, mother grid units (`xtra1`, `ytra1`, `real(kind=dp)`). |
| `ytra1` | `f64` | See [`ParticleRecord::xtra1`]. |
| `ztra1` | `f64` | Height above ground, m (`ztra1`). |
| `npoint` | `usize` | Release point, 1-based (`npoint`). |
| `nclass` | `usize` | Uncertainty class, 1-based (`nclass`). |
| `xmass1` | `Vec<f64>` | Mass per species (`xmass1(jpart,:)`). |

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
    fn clone(self: &Self) -> ParticleRecord { /* ... */ }
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
    fn eq(self: &Self, other: &ParticleRecord) -> bool { /* ... */ }
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
#### Struct `DepositionGrid`

Accumulated deposition on an output grid (`wetgridunc` /
`wetgriduncn`): `(ix, jy, species, release point, uncertainty class, age
class)`, `ix` fastest, all stored 0-based.

Stored as `f32` because upstream declares these grids `real(dep_prec)`
with `dep_prec = sp` in `par_mod.f90` — single precision even in a
`-fdefault-real-8` build. Each update is computed in `f64` and rounded
once on store, as the real(8) build does.

```rust
pub struct DepositionGrid {
    pub nx: usize,
    pub ny: usize,
    pub nspec: usize,
    pub npoint: usize,
    pub nclassunc: usize,
    pub nageclass: usize,
    pub data: Vec<f32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | `numxgrid`. |
| `ny` | `usize` | `numygrid`. |
| `nspec` | `usize` | Species slots (`maxspec`). |
| `npoint` | `usize` | Release-point slots (`maxpointspec_act`). |
| `nclassunc` | `usize` | Uncertainty classes (`nclassunc`). |
| `nageclass` | `usize` | Age classes (`nageclass`). |
| `data` | `Vec<f32>` | Values. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(nx: usize, ny: usize, nspec: usize, npoint: usize, nclassunc: usize, nageclass: usize) -> Self { /* ... */ }
  ```
  A zeroed grid.

- ```rust
  pub fn index(self: &Self, ix: usize, jy: usize, ks: usize, kp: usize, nunc: usize, nage: usize) -> usize { /* ... */ }
  ```
  Flat index, all 0-based.

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
    fn clone(self: &Self) -> DepositionGrid { /* ... */ }
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
    fn eq(self: &Self, other: &DepositionGrid) -> bool { /* ... */ }
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
#### Struct `OutputFrame`

Geometry of an output grid relative to the met grid.

```rust
pub struct OutputFrame {
    pub dx: f64,
    pub dy: f64,
    pub xoutshift: f64,
    pub youtshift: f64,
    pub dxout: f64,
    pub dyout: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dx` | `f64` | Met (mother) grid spacing, degrees (`dx`, `dy`): the kernel converts<br>mother grid units with these, also for the nested output grid. |
| `dy` | `f64` | See [`OutputFrame::dx`]. |
| `xoutshift` | `f64` | `xlon0 - outlon0` (or `...n`), degrees. |
| `youtshift` | `f64` | `ylat0 - outlat0`, degrees. |
| `dxout` | `f64` | Output grid spacing, degrees. |
| `dyout` | `f64` | See [`OutputFrame::dxout`]. |

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
    fn clone(self: &Self) -> OutputFrame { /* ... */ }
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
    fn eq(self: &Self, other: &OutputFrame) -> bool { /* ... */ }
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
#### Enum `KernelRule`

Mother-grid or nested-output kernel.

```rust
pub enum KernelRule {
    Mother {
        use_kernel: bool,
    },
    Nest,
}
```

##### Variants

###### `Mother`

`wetdepokernel.f90`: cell index by `int` (truncation toward zero);
`use_kernel = lusekerneloutput` selects the 4-point kernel or direct
attribution to the cell.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `use_kernel` | `bool` | `lusekerneloutput`. |

###### `Nest`

`wetdepokernel_nest.f90`: cell index by `floor`, always the kernel.

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
    fn clone(self: &Self) -> KernelRule { /* ... */ }
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
    fn eq(self: &Self, other: &KernelRule) -> bool { /* ... */ }
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
#### Struct `WetDepoSettings`

Run-level settings `wetdepo` reads from `com_mod`.

```rust
pub struct WetDepoSettings {
    pub ldirect: i64,
    pub loutstep: i64,
    pub lage: Vec<i64>,
    pub ioutputforeachrelease: bool,
    pub decay: Vec<f64>,
    pub wetdepspec: Vec<bool>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ldirect` | `i64` | `+1` forward, `-1` backward (`ldirect`). |
| `loutstep` | `i64` | Output interval, s (`loutstep`). |
| `lage` | `Vec<i64>` | Upper bounds of the age classes, s (`lage(1:nageclass)`). |
| `ioutputforeachrelease` | `bool` | `ioutputforeachrelease == 1`: grid by release point. |
| `decay` | `Vec<f64>` | Decay constant per species, 1/s (`decay`; `<= 0` = stable). |
| `wetdepspec` | `Vec<bool>` | Wet deposition switched on per species (`WETDEPSPEC`). |

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
    fn clone(self: &Self) -> WetDepoSettings { /* ... */ }
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
    fn eq(self: &Self, other: &WetDepoSettings) -> bool { /* ... */ }
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
#### Struct `ScavCounts`

Below- and in-cloud event counters (`tot_blc_count`, `tot_inc_count`).

```rust
pub struct ScavCounts {
    pub blc: Vec<i64>,
    pub inc: Vec<i64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `blc` | `Vec<i64>` | Below-cloud events per species. |
| `inc` | `Vec<i64>` | In-cloud events per species. |

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
    fn clone(self: &Self) -> ScavCounts { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ScavCounts) -> bool { /* ... */ }
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
#### Struct `WetOutputGrid`

An output grid with its frame and kernel rule.

```rust
pub struct WetOutputGrid {
    pub grid: DepositionGrid,
    pub frame: OutputFrame,
    pub rule: KernelRule,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `DepositionGrid` | Accumulated deposition. |
| `frame` | `OutputFrame` | Geometry. |
| `rule` | `KernelRule` | Kernel variant. |

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
    fn clone(self: &Self) -> WetOutputGrid { /* ... */ }
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
    fn eq(self: &Self, other: &WetOutputGrid) -> bool { /* ... */ }
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

#### Function `interpol_rain`

**Attributes:**

- `MustUse { reason: None }`

`interpol_rain.f90` / `interpol_rain_nests.f90`: bilinear interpolation
of three 2-d fields at `(xt, yt)` (grid units of the grid the fields are
on).

A point on or beyond the last grid line is moved to `n-1 - 0.00001`
first (upstream modifies its argument). The lower-left index is
`int(xt)` — truncation toward zero — so a point in `(-1, 0)` is
extrapolated from cell 0, as upstream does.

# Returns
`None` for a point at or left of `-1` (or below), where upstream indexes
outside the array.

```rust
pub fn interpol_rain(lsprec: &super::met_fields::Field2, convprec: &super::met_fields::Field2, tcc: &super::met_fields::Field2, xt: f64, yt: f64) -> Option<RainAtPoint> { /* ... */ }
```

#### Function `nearest_field`

**Attributes:**

- `MustUse { reason: None }`

The field slot (0 = earlier) `get_wetscav` and `ohreaction` read:
`nint(itime - 0.5 ltsample)`, the earlier field only if strictly closer.

```rust
pub fn nearest_field(itime: i64, ltsample: i64, memtime: [i64; 2]) -> usize { /* ... */ }
```

#### Function `get_wetscav`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`get_wetscav.f90`: scavenging coefficient of species `species` for a
particle at `(xtra, ytra)` (mother grid units) and height `ztra` (m).

1. Grid: the highest nest containing the point, else the mother grid.
2. Field: the one nearest `itime - ltsample/2` ([`nearest_field`]).
3. `lsp`, `convp`, `cc` by [`interpol_rain`]; below 0.01 mm/h each,
   nothing happens (`wetscav = 0`, `grfraction` unassigned).
4. Cloud code at the particle's level (cell `int(x), int(y)`, not the
   interpolation cell); `<= 1`: nothing.
5. `grfraction = max(0.05, cc (lsp lfr + convp cfr)/(lsp + convp))` with
   upstream's rate classes, and the sub-grid rate `prec = (lsp+convp)/grfraction`.
6. Below cloud (code >= 4): gas `A prec^B`; aerosol rain (`T >= 273`)
   Laakso et al. (2003) or snow Kyrö et al. (2009) polynomial in
   `log10(d)`, `d = min(10 µm, dquer)`.
   In cloud (code 2, 3): `incloud_ratio * S_i * prec/3.6e6` with `S_i`
   the activated fraction over the cloud water (aerosol) or the
   Henry-law partitioning (gas); cloud water from `ctwc` when read, else
   `0.2 prec^0.36`.

`species` is `&mut` because upstream zeroes a negative `ccn_aero` /
`in_aero` in `com_mod` on the in-cloud path; the port does the same.

# Errors
[`WetDepError::AboveTopLevel`] where upstream reads an unassigned level
index; [`WetDepError::OutsideGrid`] where it would index outside its
arrays.

```rust
pub fn get_wetscav(itime: i64, ltsample: i64, xtra: f64, ytra: f64, ztra: f64, species: &mut WetScavSpecies, met: &WetScavMet) -> Result<WetScav, WetDepError> { /* ... */ }
```

#### Function `wetdepokernel`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`wetdepokernel.f90` / `wetdepokernel_nest.f90`: add `deposit[ks]` of a
particle at `(x, y)` (mother grid units) to the output grid with a
uniform kernel one output cell wide.

`xl = (x dx + xoutshift)/dxout`; the cell is `int(xl)` (mother) or
`floor(xl)` (nest); the neighbour is to the right if `ddx > 0.5`, else to
the left, with weight `wx = 1.5 - ddx` or `0.5 + ddx` on the own cell.
Points outside the grid are dropped.

**Upstream defect, reproduced:** with truncation (mother grid) a point
with `xl` in `(-1, -0.5]` gets `ix = 0` and `ddx < 0`, so `wx < 0.5`
and possibly negative: deposition is attributed with a **negative**
weight to cell 0 and a weight above 1 to cell `-1` (dropped). The nest
version's `floor` was introduced upstream (ESO) to fix exactly this.

`kp`, `nunc`, `nage` are upstream's 1-based class numbers.

# Errors
[`WetDepError::AgeBeyondLastClass`] or [`WetDepError::ClassOutOfRange`]
where upstream writes outside the array.

```rust
pub fn wetdepokernel(rule: KernelRule, frame: &OutputFrame, x: f64, y: f64, deposit: &[f64], grid: &mut DepositionGrid, kp: usize, nunc: usize, nage: usize) -> Result<(), WetDepError> { /* ... */ }
```

#### Function `wetdepo`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`wetdepo.f90`: wet deposition of every active particle over the interval
`ltsample` ending at `itime`.

For each particle active at `itime` (`itra1 != -999999999` and not
released after `itime` in the run direction), its age class, then for each
species with `WETDEPSPEC`: [`get_wetscav`];
`deposit = m (1 - exp(-Lambda |ltsample|)) grfraction` when `Lambda > 0`;
the particle keeps `m - deposit` (or 0 if that is not above `tiny`); the
deposit is corrected for the decay since the last output,
`exp(|ldeltat| lambda)`. In a forward run the deposits are gridded with
[`wetdepokernel`] on the mother output grid and, if present, the nested
one. Counters are added to `counts`.

`tiny` is `f64::MIN_POSITIVE`, the `tiny(0.0)` of the real(8) build (the
shipped real(4) build uses `1.18e-38`).

# Upstream defects, refused rather than guessed
- A species with `WETDEPSPEC` false is skipped (`cycle`) **without
  assigning its `wetdeposit`**, yet in a forward run the kernel grids
  `wetdeposit(ks)` for every species: a stale value from an earlier
  particle or call. The port returns [`WetDepError::UndefinedDeposit`].
- A particle at least `lage(nageclass)` old falls through the age loop
  with `nage = nageclass + 1`, which the kernel uses as an array index
  ([`WetDepError::AgeBeyondLastClass`]). FLEXPART normally removes such
  particles earlier; nothing here guards it.

On error, particles before the failing one have already been updated.

# Errors
As above, and those of [`get_wetscav`] and [`wetdepokernel`].

```rust
pub fn wetdepo(itime: i64, ltsample: i64, loutnext: i64, particles: &mut [ParticleRecord], species: &mut [WetScavSpecies], met: &WetScavMet, settings: &WetDepoSettings, mother_out: &mut WetOutputGrid, nested_out: Option<&mut WetOutputGrid>, counts: &mut ScavCounts) -> Result<(), WetDepError> { /* ... */ }
```

### Constants and Statics

#### Constant `INCLOUD_RATIO`

Upstream's in-cloud scavenging ratio, `par_mod.f90` (`incloud_ratio`).

```rust
pub const INCLOUD_RATIO: f64 = 6.2;
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
constant per class. The `465.11628` is ~~`1000 / (2 * 2.15)`~~ **CORRECTED
2026-10-03:** `1000 / 2.15` (= 465.116; `1000 / (2 * 2.15)` is 232.56) — the
metres-per-kilometre conversion divided by the half-width-to-sigma factor
2.15 that the original nomograms were drawn with.

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

