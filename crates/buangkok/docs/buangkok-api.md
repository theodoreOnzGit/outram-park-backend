# Crate Documentation

**Version:** 0.0.1

**Format Version:** 60

# Module `buangkok`

# BUANGKOK

**B**ioeffects, **U**ncertainty and **A**LARA for **N**uclear
**G**uidance, **K**eeping **O**perational **K**nowledge.

The reserved home for **radiation dose and its biological effects**, for
research-grade safety analysis: turning air concentrations and ground
deposition into dose (cloudshine, groundshine, inhalation, ingestion),
dose coefficients, dose uncertainty, and ALARA reasoning. The question it
answers is **"what dose follows from what was released and where it
went?"**

# STATUS: partial pyDOSEIA port (2026-09-28)

~~PLACEHOLDER. Nothing is implemented.~~ **CHANGED 2026-09-28** (maintainer:
"work on translating pyDOSEIA into buangkok under a module"). The crate
now holds [`pydoseia`], a partial, faithful port of the MIT-licensed
pyDOSEIA code (Sadhu et al., *Health Physics* 130(1) (2026) 94-110,
doi:10.1097/HP.0000000000002014). That covers its met processing,
Gaussian-plume dilution factors, and the inhalation, ground-shine and
submersion dose pathways. They are **code-to-code verified against
upstream** on synthetic inputs (`tests/pydoseia_code_to_code.rs`). The
agreement is with pyDOSEIA, not with experiment, and there is no
validation of any kind. Ingestion and plume shine are **not ported** (see
`docs/pydoseia-port-scoping.md`). No dose-coefficient data ships with the
crate. Human V&V review is still outstanding (see README).

# Why dose has its own crate

Dose is a biological quantity. Keeping it apart from the dispersion
physics (CHANGI) and the source term (SEMBAWANG, BISHAN) means neither of
those is mistaken for a health-assessment capability, and it keeps the
"no dose" boundary those crates already state easy to hold. The workspace
uses dose for **safety analysis in the research sense only**: never for
medical, occupational-exposure, public-health, emergency-response,
licensing or regulatory decisions (`RESPONSIBLE_USE.md`).

# Where it would sit

```text
  source term ──► CHANGI ──────────────► BUANGKOK
  (SEMBAWANG,     air concentration,      dose, pathways,
   BISHAN)        deposition              uncertainty (+ RAFFLES)
```

# Already in the workspace, not yet here

~~Two published HTR-10 dose-versus-distance tables are stored in `changi`,
parked there by the maintainer … do not move them unasked.~~ **MOVED
2026-09-28** (maintainer: "move table 7 and 9 to buangkok"): the published
HTR-10 dose tables now live here, in [`published`] — normal operation
(Liu and Cao 2002, Table 7) and two design-basis accidents (Table 9).
They are stored reference data; nothing in this crate computes a dose
from them.

## Modules

## Module `published`

Published dose tables (Liu and Cao 2002, Tables 7 and 9), stored as cited
reference data. See the module docs.
Published dose tables, stored as cited reference data. **Nothing here
computes a dose.**

Moved from `changi::activity` on 2026-09-28 (maintainer: "move table 7 and
9 to buangkok"), because dose belongs in the dose crate:

- [`normal_operation_dose_by_distance`](crate::published::normal_operation_dose_by_distance) — Liu and Cao (2002), Table 7:
  HTR-10 normal-operation individual effective dose vs distance (mSv/a).
- [`accident_dose_by_distance`](crate::published::accident_dose_by_distance) — Liu and Cao (2002), Table 9: HTR-10
  design-basis-accident thyroid and whole-body doses vs distance (mSv).

Both are published model results (AIRDOS-EPA and STOERNEU respectively),
not measurements. Provenance: `crates/buangkok/docs/References.md`.
`RESPONSIBLE_USE.md` applies in full.

```rust
pub mod published { /* ... */ }
```

### Modules

## Module `accident_dose_by_distance`

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
the paper's Table 8, ~~which is **not** digitised in this workspace~~
**CORRECTED 2026-09-28**: now stored in
[`changi::activity::accident_airborne_release`], which reuses the
`AccidentCase` enum below. In both
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
  quantity (see `changi::activity`).
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

Distance is a `uom` [`Length`]. **The doses are
plain `f64` in mSv**, and the field names say so, for the reason given in
[`crate::published::normal_operation_dose_by_distance`]: `uom` 0.38 has no
sievert quantity, and `AvailableEnergy` (J/kg) was rejected on purpose.

# Provenance

Liu Yuanzhong and Cao Jianzhu, *"Fission product release and its
environment impact for normal reactor operations and for relevant
accidents"*, **Nuclear Engineering and Design 218 (2002) 81–90**, Table 9
(p. 88), "Individual doses caused by accidents of the HTR-10 (mSv)". The
basis above comes from the paper's Sections 4.1–4.2 (pp. 86–89).

Access terms, digitisation and verification are in
`crates/buangkok/docs/References.md`. The document carries no reuse licence
and is **not** redistributed here. Only the cited table of 65 numbers is,
which is ordinary scientific citation.

```rust
pub mod accident_dose_by_distance { /* ... */ }
```

### Types

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

### Re-exports

#### Re-export `AccidentCase`

Which of the paper's two tabulated accidents a dose belongs to. Defined in
changi beside the accident releases (Table 8) and re-exported here, so the
release and dose tables name the accidents identically.

```rust
pub use changi::activity::accident_airborne_release::AccidentCase;
```

## Module `normal_operation_dose_by_distance`

**A published dose-versus-distance table for HTR-10 normal operation,
stored as reference data. Nothing here computes a dose.**

# What this is

The source's individual effective dose to an adult member of the public,
in mSv per year, at twelve distances from 0.5 km to 75 km from the release
point. It is given only along the azimuth where the dose is largest. It is
the dose the same paper calculates from its annual normal-operation
airborne release (its Table 5, in [`changi::activity::airborne_release`]).

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
  quantity (see `changi::activity`). This module stores numbers a
  published paper printed. It does not bring dose assessment into this
  crate's current scope. That is a maintainer decision taken in
  `RESPONSIBLE_USE.md`.
- **Not an accident dose.** The same paper tabulates accident doses
  separately (its Table 9), ~~which is not digitised here~~ **CORRECTED
  2026-09-28**: now stored in
  [`crate::published::accident_dose_by_distance`].
- `RESPONSIBLE_USE.md` applies in full. Nothing here may be quoted as a
  dose to the public from HTR-10 or any other plant for any operational,
  licensing, siting, emergency-planning or safety purpose.

# Units

Distance is a `uom` [`Length`]. **The dose is a plain `f64` in mSv per
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
`crates/buangkok/docs/References.md`. The document carries no reuse licence
and is **not** redistributed here. Only the cited table of 12 values is,
which is ordinary scientific citation.

```rust
pub mod normal_operation_dose_by_distance { /* ... */ }
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

## Module `pydoseia`

Partial port of pyDOSEIA (met processing, Gaussian-plume dilution,
inhalation / ground-shine / submersion doses), code-to-code verified
against upstream. See the module docs for provenance and scope.
# A partial port of pyDOSEIA: Gaussian-plume dilution, and inhalation, ground-shine and submersion doses

> **Research, education and V&V only.** Not for medical, occupational,
> public-health, emergency-response, licensing or regulatory use, and
> never a dose to a real person or population (`RESPONSIBLE_USE.md`).

## Upstream and provenance

| | |
|---|---|
| Upstream project | **pyDOSEIA**, <https://github.com/BiswajitSadhu/pyDOSEIA> |
| Commit ported | `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce` (branch `head`, 2025-08-26), checked 2026-09-28 |
| Source files | `metfunc.py`, `dosefunc.py`, `raddcffunc.py` (function-level mapping in each submodule) |
| Copyright | Copyright (c) 2024 Dr. Biswajit Sadhu |
| Licence | MIT. The full notice is in `crates/buangkok/NOTICE` and must stay with every ported file. MIT is compatible with this crate's GPL-3.0 |
| Paper | B. Sadhu, T. Sarkar, S. Anand, K. D. Singh, D. K. Aswal, "pyDOSEIA: A Python Package for Radiological Impact Assessment during Long-term or Accidental Atmospheric Releases", *Health Physics* **130**(1) (2026) 94-110, doi:[10.1097/HP.0000000000002014](https://doi.org/10.1097/HP.0000000000002014), PMID [40622262](https://pubmed.ncbi.nlm.nih.gov/40622262/). The article is (c) 2025 Health Physics Society: cited, not reproduced |

## What is ported, and how it is verified

| Module | Upstream | Status |
|---|---|---|
| [`met`](crate::pydoseia::met) | met processing: gap filling, TJFD, missing and calm corrections, speed distribution | ported, code-to-code verified |
| [`dispersion`](crate::pydoseia::dispersion) | sigmas, height correction, master equations, dilution factor for 3 release modes | ported, code-to-code verified (the 4th mode, single plume with met data, fails upstream's own assertion and cannot run: defect D3) |
| [`nuclide`](crate::pydoseia::nuclide) | half-life text parsing, `0.693 / T` | ported, code-to-code verified |
| [`dcf`](crate::pydoseia::dcf) | age brackets, absorption-type lookup, progeny correction | ported, code-to-code verified on **synthetic** tables |
| [`dose`](crate::pydoseia::dose) | inhalation, ground shine, submersion, deposition velocity, weathering | ported, code-to-code verified |
| — | ingestion, plume shine, H-3/C-14 models, multi-source DCF screening, input generator, output | **not ported** (see `docs/pydoseia-port-scoping.md`) |

Verification is **code-to-code against upstream itself**: the upstream
Python is executed over a grid of inputs by
`verification_and_validation/pydoseia_code_to_code/gen_pydoseia_reference.py`
and the port must reproduce every number
(`tests/pydoseia_code_to_code.rs`; methodology and measured results in
`docs/pydoseia-code-to-code.md`). This checks the **translation**, not the
physics: agreeing with pyDOSEIA says nothing about whether pyDOSEIA's model
is right.

## Faithful, including upstream's defects

The port reproduces upstream's control flow and constants, including the
defects found while porting (D1-D6 in `docs/pydoseia-code-to-code.md`).
Where a defect is clear, a corrected variant sits beside the faithful one
and is labelled as a **divergence**:
[`dispersion::CalmCorrection::LowestSpeedClassTotal`](crate::pydoseia::dispersion::CalmCorrection::LowestSpeedClassTotal) (D1) and
[`dispersion::max_dilution_factor`](crate::pydoseia::dispersion::max_dilution_factor) (D2).

## No data tables

No dose coefficient, half-life, decay chain or met record from upstream's
`library/` or `met_data/` is in this crate. The caller supplies them (see
[`dcf`](crate::pydoseia::dcf) for the per-table licence reasoning).

## Minimal example (inhalation, one nuclide, synthetic coefficient)

```
use buangkok::pydoseia::dispersion::{self, MeanSpeedScaling, PlumeGeometry, Receptor};
use buangkok::pydoseia::dose::{self, Release};
use uom::si::f64::{Length, Radioactivity};
use uom::si::length::meter;
use uom::si::radioactivity::becquerel;

let geometry = PlumeGeometry {
    release_height: Length::new::<meter>(30.0),
    measurement_height: Length::new::<meter>(10.0),
    receptor: Receptor::GroundLevelCentreline,
};
let per_class = dispersion::dilution_long_term_no_met(
    Length::new::<meter>(800.0), geometry, MeanSpeedScaling::UnitSpeed);
let chi_over_q = dispersion::max_dilution_factor(&per_class);
let release = Release::AnnualDischarge(Radioactivity::new::<becquerel>(1.0e9));
let synthetic_dcf_sv_per_bq = 1.0e-9; // not a real coefficient
let d = dose::inhalation_dose(chi_over_q, release, synthetic_dcf_sv_per_bq, 40.0).unwrap();
assert!(d.millisieverts() > 0.0);
```

```rust
pub mod pydoseia { /* ... */ }
```

### Modules

## Module `dcf`

Dose-coefficient (DCF) tables and pyDOSEIA's lookups into them: the age
brackets, the lung-absorption-type selection for inhalation, and the
short-lived-progeny correction for the external pathways.

# Provenance

Ported from pyDOSEIA `raddcffunc.py` (`RaddcfFunc.inhalation_dcf_list`,
`dcf_list_ecerman_ground_shine_include_progeny`,
`dcf_list_ecerman_submersion_include_progeny`,
`find_progeny_name_and_yield_f`), upstream
<https://github.com/BiswajitSadhu/pyDOSEIA> at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).

# No coefficient data ships with this crate — the caller supplies it

Upstream bundles its coefficients. Their sources, and why none is copied
here (details in `crates/buangkok/docs/pydoseia-port-scoping.md`):

| Upstream file / sheet | Content | Source | Here |
|---|---|---|---|
| `RadioToxicityMaster.xls` / `Inhalation CED Sv per Bq Public` | inhalation e(g), six ages, types F/M/S/V | ICRP (Publ. 72 values, as reproduced in the IAEA BSS); ICRP data are copyrighted | **not copied**; load your own via [`InhalationDcfTable::from_csv`] |
| `Dose_ecerman_final.xlsx` / `surface_dose`, `submersion_dose` | external dose-rate coefficients, six ages | US EPA **FGR-15** (EPA-402/R-19/002, 2019), Table 4-1 and the submersion table; a US federal report | not copied in this pass; loadable via [`ExternalDcfTable::from_csv`] |
| `dcf_corr.xlsx` | decay chains and branching | upstream says "SRS 19 based on ICRP 107" (IAEA / ICRP) | **not copied**; load your own via [`ProgenyChains::from_csv`] |

The tables are read from CSVs in upstream's column layout (see each
`from_csv`). The code-to-code test uses **synthetic** tables in that layout
(`tests/data/pydoseia_synthetic_*.csv`), so no copyrighted coefficient is in
the repository.

```rust
pub mod dcf { /* ... */ }
```

### Types

#### Enum `AgeBracket`

The six age brackets every pyDOSEIA DCF lookup uses, selected from an age in
years with upstream's boundaries (`age <= 1`, `1 < age <= 2`, `2 < age <= 7`,
`7 < age <= 12`, `12 < age <= 17`, `age > 17`).

```rust
pub enum AgeBracket {
    Infant,
    OneToTwo,
    TwoToSeven,
    SevenToTwelve,
    TwelveToSeventeen,
    Adult,
}
```

##### Variants

###### `Infant`

`age <= 1` (ICRP "< 1 a" / FGR-15 "Newborn").

###### `OneToTwo`

`1 < age <= 2` ("1-2 a" / "1-yr-old").

###### `TwoToSeven`

`2 < age <= 7` ("2-7 a" / "5-yr-old").

###### `SevenToTwelve`

`7 < age <= 12` ("7-12 a" / "10-yr-old").

###### `TwelveToSeventeen`

`12 < age <= 17` ("12-17 a" / "15-yr-old").

###### `Adult`

`age > 17` ("> 17 a" / "Adult").

##### Implementations

###### Methods

- ```rust
  pub fn from_age_years(age: f64) -> Option<Self> { /* ... */ }
  ```
  Upstream's bracket for an age in years; `None` for NaN (upstream raises

- ```rust
  pub const fn column(self: Self) -> usize { /* ... */ }
  ```
  Column index, 0 (infant) to 5 (adult), in upstream's table order.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AgeBracket { /* ... */ }
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
    fn eq(self: &Self, other: &AgeBracket) -> bool { /* ... */ }
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
#### Enum `LungAbsorptionType`

Lung absorption type used to pick inhalation rows.

```rust
pub enum LungAbsorptionType {
    F,
    M,
    S,
    V,
    Max,
}
```

##### Variants

###### `F`

Fast.

###### `M`

Moderate.

###### `S`

Slow.

###### `V`

Vapour / gas.

###### `Max`

Upstream's `'Max'`: every row whose type contains F, M, S or V, taking
the largest coefficient.

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
    fn clone(self: &Self) -> LungAbsorptionType { /* ... */ }
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
    fn eq(self: &Self, other: &LungAbsorptionType) -> bool { /* ... */ }
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
#### Struct `InhalationDcfRow`

One inhalation-coefficient row: nuclide, absorption type, e(g) in Sv/Bq for
the six [`AgeBracket`]s (NaN where blank).

```rust
pub struct InhalationDcfRow {
    pub nuclide: String,
    pub absorption_type: Option<String>,
    pub sv_per_bq: [f64; 6],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `String` | Nuclide name as written in the table. |
| `absorption_type` | `Option<String>` | Absorption type field (`None` if blank; such rows never match). |
| `sv_per_bq` | `[f64; 6]` | Committed effective dose per unit intake, Sv/Bq, by age bracket. |

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
    fn clone(self: &Self) -> InhalationDcfRow { /* ... */ }
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
    fn eq(self: &Self, other: &InhalationDcfRow) -> bool { /* ... */ }
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
#### Struct `InhalationDcfTable`

An inhalation coefficient table in upstream's layout.

```rust
pub struct InhalationDcfTable {
    pub rows: Vec<InhalationDcfRow>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `rows` | `Vec<InhalationDcfRow>` | The rows, in file order. |

##### Implementations

###### Methods

- ```rust
  pub fn from_csv(text: &str) -> Result<Self, String> { /* ... */ }
  ```
  Read a CSV export of upstream's `Inhalation CED Sv per Bq Public` sheet:

- ```rust
  pub fn lookup(self: &Self, nuclide: &str, absorption: LungAbsorptionType, age: AgeBracket) -> f64 { /* ... */ }
  ```
  Upstream's `inhalation_dcf_list` for one nuclide: the largest

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> InhalationDcfTable { /* ... */ }
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
    fn default() -> InhalationDcfTable { /* ... */ }
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
    fn eq(self: &Self, other: &InhalationDcfTable) -> bool { /* ... */ }
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
#### Struct `ExternalDcfRow`

One external dose-rate coefficient row: nuclide and the six age columns
(Sv m^2/(Bq s) for ground surface; Sv m^3/(Bq s) for submersion).

```rust
pub struct ExternalDcfRow {
    pub nuclide: String,
    pub coefficients: [f64; 6],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `String` | Nuclide name as written in the table. |
| `coefficients` | `[f64; 6]` | Coefficients by age bracket (NaN where blank). |

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
    fn clone(self: &Self) -> ExternalDcfRow { /* ... */ }
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
    fn eq(self: &Self, other: &ExternalDcfRow) -> bool { /* ... */ }
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
#### Struct `ExternalDcfTable`

An external-exposure coefficient table (ground surface or submersion) in
upstream's FGR-15-derived layout.

```rust
pub struct ExternalDcfTable {
    pub rows: Vec<ExternalDcfRow>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `rows` | `Vec<ExternalDcfRow>` | The rows, in file order. |

##### Implementations

###### Methods

- ```rust
  pub fn from_csv(text: &str) -> Result<Self, String> { /* ... */ }
  ```
  Read a CSV export of upstream's `surface_dose` or `submersion_dose`

- ```rust
  pub fn lookup_exact(self: &Self, nuclide: &str, age: AgeBracket) -> f64 { /* ... */ }
  ```
  Largest coefficient over rows whose name **equals** `nuclide` (upstream's

- ```rust
  pub fn lookup_contains(self: &Self, nuclide: &str, age: AgeBracket) -> f64 { /* ... */ }
  ```
  Largest coefficient over rows whose name **contains** `nuclide`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExternalDcfTable { /* ... */ }
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
    fn default() -> ExternalDcfTable { /* ... */ }
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
    fn eq(self: &Self, other: &ExternalDcfTable) -> bool { /* ... */ }
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
#### Struct `ProgenyChains`

Decay chains for the progeny correction: each parent's daughters with
their yields, and each daughter's half-life text.

```rust
pub struct ProgenyChains {
    pub links: Vec<(String, String, f64)>,
    pub half_lives: Vec<(String, String)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `links` | `Vec<(String, String, f64)>` | `(parent, daughter, yield)`, in file order. |
| `half_lives` | `Vec<(String, String)>` | `(nuclide, half-life text)` in upstream's progeny format (parsed by<br>[`super::nuclide::parse_progeny_half_life`]). |

##### Implementations

###### Methods

- ```rust
  pub fn from_csv(links_csv: &str, half_lives_csv: &str) -> Result<Self, String> { /* ... */ }
  ```
  Read two CSVs: `parent,daughter,yield` and `nuclide,half_life`. This is

- ```rust
  pub fn short_lived_daughters(self: &Self, parent: &str, ignore_half_life_s: f64) -> Vec<(String, f64)> { /* ... */ }
  ```
  Upstream's `find_progeny_name_and_yield_f`: the daughters of `parent`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ProgenyChains { /* ... */ }
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
    fn default() -> ProgenyChains { /* ... */ }
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
    fn eq(self: &Self, other: &ProgenyChains) -> bool { /* ... */ }
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
#### Enum `ProgenyCorrection`

Whether to add short-lived progeny to an external coefficient.

```rust
pub enum ProgenyCorrection {
    ParentOnly,
    IncludeShortLived {
        ignore_half_life_s: f64,
    },
}
```

##### Variants

###### `ParentOnly`

Parent only (upstream `consider_progeny: False`).

###### `IncludeShortLived`

Add `yield x coefficient` for each daughter with half-life at most this
many seconds (upstream `consider_progeny: True`, `ignore_half_life`,
default 1800 s).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ignore_half_life_s` | `f64` | Half-life threshold, s. |

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
    fn clone(self: &Self) -> ProgenyCorrection { /* ... */ }
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
    fn eq(self: &Self, other: &ProgenyCorrection) -> bool { /* ... */ }
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
#### Struct `ExternalDcfPair`

A parent coefficient with and without the progeny correction (upstream
returns this pair; the pathways pick one according to the same flag).

```rust
pub struct ExternalDcfPair {
    pub corrected: f64,
    pub uncorrected: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `corrected` | `f64` | Parent plus short-lived progeny (equal to `uncorrected` for<br>[`ProgenyCorrection::ParentOnly`]). |
| `uncorrected` | `f64` | Parent only. |

##### Implementations

###### Methods

- ```rust
  pub fn selected(self: Self, progeny: ProgenyCorrection) -> f64 { /* ... */ }
  ```
  The coefficient a pathway uses: `corrected` when progeny are included.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExternalDcfPair { /* ... */ }
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
    fn eq(self: &Self, other: &ExternalDcfPair) -> bool { /* ... */ }
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

#### Function `external_dcf`

**Attributes:**

- `MustUse { reason: None }`

Upstream's `dcf_list_ecerman_*_include_progeny` for one nuclide.

```rust
pub fn external_dcf(table: &ExternalDcfTable, chains: &ProgenyChains, nuclide: &str, age: AgeBracket, progeny: ProgenyCorrection) -> ExternalDcfPair { /* ... */ }
```

## Module `dispersion`

Gaussian-plume dispersion as pyDOSEIA computes it: the Pasquill-Gifford
sigmas, the release-height wind correction, the two master equations and
the dilution factor (`chi/Q`) for each of pyDOSEIA's release modes.

# Provenance

Ported from pyDOSEIA `metfunc.py` (`MetFunc.sigmay`, `sigmaz`,
`height_correction_factor`, `master_eq_single_plume`,
`master_eq_sector_averaged_plume`, `dilution_per_sector`,
`synthetic_TJFD_for_single_plume`, `get_max_dilution_factor`), upstream
<https://github.com/BiswajitSadhu/pyDOSEIA> at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). Upstream cites
Hukkoo and Bapat's BARC manual (eqs. 2.5, 2.7, 2.32, 2.33) for the master
equations and Pasquill (1974) for the height correction. Paper: Sadhu et
al., *Health Physics* 130(1) (2026) 94-110, doi:10.1097/HP.0000000000002014.

The arithmetic is written in upstream's operation order so the port can be
compared with it to the last bit (`tests/pydoseia_code_to_code.rs`).

# Overlap with `changi`

`changi::puff` and `changi::activity::chi_over_q` also give Pasquill-Gifford
Gaussian dispersion. This is a **separate** port of pyDOSEIA's own model,
kept inside `buangkok` so it can be verified code-to-code against pyDOSEIA.
The sigma fits differ (upstream uses the BARC/AERB power-law set below, not
Briggs), so the two are not interchangeable and are not unified here.

```rust
pub mod dispersion { /* ... */ }
```

### Types

#### Enum `StabilityClass`

Pasquill stability class, A (most unstable) to F (most stable).

Upstream carries this as the integer 1-6; [`StabilityClass::code`] gives
that integer back.

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

Extremely unstable (upstream 1).

###### `B`

Moderately unstable (upstream 2).

###### `C`

Slightly unstable (upstream 3).

###### `D`

Neutral (upstream 4).

###### `E`

Slightly stable (upstream 5).

###### `F`

Moderately stable (upstream 6).

##### Implementations

###### Methods

- ```rust
  pub const fn code(self: Self) -> u8 { /* ... */ }
  ```
  Upstream's integer code, 1 (A) to 6 (F).

- ```rust
  pub const fn index(self: Self) -> usize { /* ... */ }
  ```
  Zero-based index, 0 (A) to 5 (F).

- ```rust
  pub const fn from_code(code: u8) -> Option<Self> { /* ... */ }
  ```
  From upstream's integer code (1-6). `None` outside that range.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
#### Enum `Receptor`

Where the concentration is evaluated.

```rust
pub enum Receptor {
    GroundLevelCentreline,
    Offset {
        y: uom::si::f64::Length,
        z: uom::si::f64::Length,
    },
}
```

##### Variants

###### `GroundLevelCentreline`

Ground level on the plume centreline, `y = z = 0` (upstream's
`max_conc_plume_central_line_gl: True`).

###### `Offset`

A crosswind offset `y` and height `z` (upstream's config `Y`, `Z`). The
sector-averaged equation uses only `z`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `y` | `uom::si::f64::Length` | Crosswind distance from the plume axis. |
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
#### Struct `MasterEquationTerms`

The two factors of upstream's master equation; the dilution factor is
their product (times the frequency weighting of the release mode).

```rust
pub struct MasterEquationTerms {
    pub pre_expo: f64,
    pub expo: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pre_expo` | `f64` | Pre-exponential factor, 1/m^3 per (m/s) (i.e. s/m^3 at unit speed). |
| `expo` | `f64` | Exponential factor (dimensionless), including the ground reflection. |

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
    fn clone(self: &Self) -> MasterEquationTerms { /* ... */ }
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
    fn eq(self: &Self, other: &MasterEquationTerms) -> bool { /* ... */ }
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
#### Struct `PlumeGeometry`

The release and receptor geometry shared by every dilution calculation.

```rust
pub struct PlumeGeometry {
    pub release_height: uom::si::f64::Length,
    pub measurement_height: uom::si::f64::Length,
    pub receptor: Receptor,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `release_height` | `uom::si::f64::Length` | Effective release height `H` (upstream does not add plume rise; see the<br>scoping note). |
| `measurement_height` | `uom::si::f64::Length` | Height at which the wind speed was measured, `H_m`. |
| `receptor` | `Receptor` | Where the concentration is evaluated. |

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
    fn clone(self: &Self) -> PlumeGeometry { /* ... */ }
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
    fn eq(self: &Self, other: &PlumeGeometry) -> bool { /* ... */ }
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
#### Enum `MeanSpeedScaling`

Optional division of the per-stability dilution factors by a mean wind
speed per stability class (upstream's `like_to_scale_with_mean_speed` with
`ask_mean_speed_data`), for the two no-met-data modes.

```rust
pub enum MeanSpeedScaling {
    UnitSpeed,
    PerClass([uom::si::f64::Velocity; 6]),
}
```

##### Variants

###### `UnitSpeed`

No scaling: the dilution factors are for a 1 m/s wind at the
measurement height (upstream default).

###### `PerClass`

Divide class `i`'s dilution factor by `speeds[i]` (A to F).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `[uom::si::f64::Velocity; 6]` |  |

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
    fn clone(self: &Self) -> MeanSpeedScaling { /* ... */ }
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
    fn eq(self: &Self, other: &MeanSpeedScaling) -> bool { /* ... */ }
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
#### Enum `CalmCorrection`

Which calm correction to apply in [`dilution_long_term_with_met`].

```rust
pub enum CalmCorrection {
    Off,
    Upstream,
    LowestSpeedClassTotal,
}
```

##### Variants

###### `Off`

None (upstream `calm_correction: False`).

###### `Upstream`

Upstream's factors exactly, including defect D1 (see
[`super::met::calm_correction_factors`]).

###### `LowestSpeedClassTotal`

**Divergence from upstream** — D1 corrected: `N_L` is the total count in
the lowest non-calm speed class over all sectors and classes. See
[`super::met::calm_correction_factors_lowest_speed_class`].

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
    fn clone(self: &Self) -> CalmCorrection { /* ... */ }
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
    fn eq(self: &Self, other: &CalmCorrection) -> bool { /* ... */ }
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

#### Function `sigma_y`

**Attributes:**

- `MustUse { reason: None }`

Lateral plume spread `sigma_y = A_y x^0.9031`, m, for downwind distance `x`.

Upstream (`MetFunc.sigmay`) also computes a sampling-time correction factor
but never applies it (the multiplication is commented out at `dca4cdc3`);
this port likewise does not apply one.

```rust
pub fn sigma_y(stability: StabilityClass, x: uom::si::f64::Length) -> uom::si::f64::Length { /* ... */ }
```

#### Function `sigma_z`

**Attributes:**

- `MustUse { reason: None }`

Vertical plume spread `sigma_z = A_z x^q + r`, m, with three distance bands
(`x < 100 m`, `100 <= x <= 1000 m`, `x > 1000 m`) exactly as upstream.

The bands meet only approximately at 100 m and 1000 m: the jumps are at
most 0.84 % (class E at 1000 m). The port keeps them. A NaN distance falls through every band; upstream
raises `ValueError`, the port returns NaN.

```rust
pub fn sigma_z(stability: StabilityClass, x: uom::si::f64::Length) -> uom::si::f64::Length { /* ... */ }
```

#### Function `height_correction_factor`

**Attributes:**

- `MustUse { reason: None }`

Wind-speed correction from measurement height to release height,
`(H / H_m)^p` with `p = n / (2 - n)`, `n = 0.2` (A-C), `0.25` (D), `0.5`
(E-F). A release height below 10 m is raised to 10 m first, as upstream.

Upstream multiplies the wind speed by this factor, so the dilution factor
is divided by it.

```rust
pub fn height_correction_factor(stability: StabilityClass, release_height: uom::si::f64::Length, measurement_height: uom::si::f64::Length) -> f64 { /* ... */ }
```

#### Function `master_equation_single_plume`

**Attributes:**

- `MustUse { reason: None }`

Single (instantaneous / short-term) Gaussian plume, Hukkoo-Bapat eq. 2.5:
`1 / (2 pi sigma_y sigma_z u)` times the crosswind and reflected vertical
Gaussians. `speed_factor` is the wind speed in m/s (upstream: unit speed
times [`height_correction_factor`]).

```rust
pub fn master_equation_single_plume(sigma_y: uom::si::f64::Length, sigma_z: uom::si::f64::Length, speed_factor: f64, release_height: uom::si::f64::Length, receptor: Receptor) -> MasterEquationTerms { /* ... */ }
```

#### Function `master_equation_sector_averaged`

**Attributes:**

- `MustUse { reason: None }`

Sector-averaged (long-term) Gaussian plume, Hukkoo-Bapat eq. 2.32:
`1 / (sqrt(2 pi) x theta sigma_z)` times the reflected vertical Gaussian,
with `theta` = [`SECTOR_WIDTH_RAD`]. Wind speed enters later, in
[`dilution_long_term_no_met`] and [`dilution_long_term_with_met`].

```rust
pub fn master_equation_sector_averaged(x: uom::si::f64::Length, sigma_z: uom::si::f64::Length, release_height: uom::si::f64::Length, receptor: Receptor) -> MasterEquationTerms { /* ... */ }
```

#### Function `dilution_single_plume_no_met`

**Attributes:**

- `MustUse { reason: None }`

Dilution factor for an **instantaneous (single-plume) release without met
data**, one value per stability class A-F, s/m^3 (time-integrated
concentration per Bq released, at unit wind speed times the height
correction).

Upstream builds a synthetic joint-frequency table with one hour of calm-class
wind in the first sector for each class, so each class's value is simply
the single-plume master equation at that class.

```rust
pub fn dilution_single_plume_no_met(x: uom::si::f64::Length, geometry: PlumeGeometry, scaling: MeanSpeedScaling) -> [super::units::DilutionFactor; 6] { /* ... */ }
```

#### Function `dilution_long_term_no_met`

**Attributes:**

- `MustUse { reason: None }`

Dilution factor for a **long-term release without met data** ("conservative
assumptions"), one value per stability class A-F, s/m^3.

Sector-averaged master equation with the wind speed `1 m/s * factor` of
[`height_correction_factor`] and a frequency of one hour, per class.

```rust
pub fn dilution_long_term_no_met(x: uom::si::f64::Length, geometry: PlumeGeometry, scaling: MeanSpeedScaling) -> [super::units::DilutionFactor; 6] { /* ... */ }
```

#### Function `dilution_long_term_with_met`

**Attributes:**

- `MustUse { reason: None }`

Dilution factor for a **long-term (continuous) release with met data**, one
value per 22.5-degree sector (upstream's 16 met-direction sectors, index 0
centred on 0 degrees), averaged over the years of `met`, s/m^3.

For each year: `sum over classes i and speed classes k >= 1` of
`pre * expo * N_ik / (u_k * factor_i)`, divided by the non-calm hours
(`days * operating hours - calm hours`), then optionally multiplied by the
calm-correction factors. The years are then averaged.

```rust
pub fn dilution_long_term_with_met(x: uom::si::f64::Length, geometry: PlumeGeometry, met: &super::met::MetClimatology, calm: CalmCorrection) -> [super::units::DilutionFactor; 16] { /* ... */ }
```

#### Function `max_dilution_factor`

**Attributes:**

- `MustUse { reason: None }`

The largest dilution factor in a set (per stability class or per sector):
what upstream's driver (`get_max_dilution_factor`) passes to every dose
pathway for a given distance.

Returns NaN for an empty slice. A NaN entry propagates as `numpy.max` does.

```rust
pub fn max_dilution_factor(values: &[super::units::DilutionFactor]) -> super::units::DilutionFactor { /* ... */ }
```

#### Function `upstream_internal_max_dilution_factor`

**Attributes:**

- `MustUse { reason: None }`

**Reproduces upstream defect D2**: what `MetFunc.max_dilution_factor` holds
after the two no-met-data modes, `dilution_factor_sectorwise.T[0].max()`.

On a one-dimensional array of six classes, `.T[0]` is the **first element**
(class A), so the "maximum" is class A's value, not the maximum. Upstream's
own driver does not use it (it recomputes the maximum over classes), but a
pathway function called without an explicit dilution factor does. Kept for
the code-to-code record; use [`max_dilution_factor`].

```rust
pub fn upstream_internal_max_dilution_factor(per_class: &[super::units::DilutionFactor; 6]) -> super::units::DilutionFactor { /* ... */ }
```

### Constants and Statics

#### Constant `SECTOR_WIDTH_RAD`

Upstream's sector width, 22.5 degrees "in radians", **as written**:
`0.39275`. The exact value is 0.392699...; the port keeps upstream's
constant (a relative difference of 1.3e-4).

```rust
pub const SECTOR_WIDTH_RAD: f64 = 0.39275;
```

## Module `dose`

The three pyDOSEIA dose pathways ported in this pass: **inhalation**,
**ground shine** (external dose from deposited activity) and
**submersion** (external dose from the cloud, semi-infinite-cloud
coefficients), with the deposition velocities and weathering correction
they use.

# Provenance

Ported from pyDOSEIA `dosefunc.py` (`DoseFunc.inhalation_dose`,
`ground_shine_dose`, `submersion_dose`) and `raddcffunc.py`
(`RaddcfFunc.deposition_velocity_of_rad`, `apply_weathering_correction_gs`),
upstream <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). Upstream takes
its screening deposition velocity (1000 m/d) and soil loss rates from IAEA
Safety Reports Series No. 19 (2001), which it cites; the few scalar values
used here are quoted in the code with that citation.

# What goes in and what comes out

Every pathway takes a dilution factor `chi/Q` (from
[`super::dispersion`], or from anywhere else — e.g. a `changi` calculation
— since it is just s/m^3), the release per nuclide, and the coefficients.
The pathway functions do **not** compute dispersion themselves, which is
how upstream's driver uses them too (it passes the per-distance maximum
`chi/Q` in).

Upstream's result is mSv for an instantaneous release and **mSv per year**
for a long-term release (discharge in Bq/year); [`EffectiveDose`] carries
the mSv value either way. Research-grade only (`RESPONSIBLE_USE.md`).

```rust
pub mod dose { /* ... */ }
```

### Types

#### Enum `Release`

How much of a nuclide was released.

```rust
pub enum Release {
    Instantaneous(uom::si::f64::Radioactivity),
    AnnualDischarge(uom::si::f64::Radioactivity),
}
```

##### Variants

###### `Instantaneous`

An instantaneous (single-plume) release: total activity released, Bq
(upstream `instantaneous_release_bq_list`). Doses are mSv.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Radioactivity` |  |

###### `AnnualDischarge`

A long-term release: activity discharged **per year**, Bq/y, carried as
a [`Radioactivity`] whose Bq value is the annual amount (upstream
`annual_discharge_bq_rad_list`). Doses are mSv per year.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Radioactivity` |  |

##### Implementations

###### Methods

- ```rust
  pub fn becquerels(self: Self) -> f64 { /* ... */ }
  ```
  The Bq value upstream multiplies by (total Bq, or Bq per year).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Release { /* ... */ }
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
    fn eq(self: &Self, other: &Release) -> bool { /* ... */ }
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
#### Enum `Weathering`

Whether to add the environmental (soil) loss rate to radioactive decay in
the ground-shine build-up.

```rust
pub enum Weathering {
    Off,
    SoilLossRates,
}
```

##### Variants

###### `Off`

Radioactive decay only (upstream `weathering_corr: False`).

###### `SoilLossRates`

Add upstream's soil loss rates (`weathering_corr: True`): 0.0014 /d for
Tc, Cl, I; 0.00014 /d for Cs, Sr; 0 otherwise (SRS 19 Table X, as
upstream cites it).

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
    fn clone(self: &Self) -> Weathering { /* ... */ }
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
    fn eq(self: &Self, other: &Weathering) -> bool { /* ... */ }
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

#### Function `breathing_rate_m3_per_s`

**Attributes:**

- `MustUse { reason: None }`

Upstream's breathing rate, m^3/s: 8400 m^3/y for `age > 1`, 1400 m^3/y for
`age <= 1`. `None` for a NaN age (upstream raises).

Note (upstream simplification, kept): every age above 1 year uses the
adult rate, so a child's inhalation dose is computed with an adult's
breathing rate and a child's coefficient.

```rust
pub fn breathing_rate_m3_per_s(age_years: f64) -> Option<f64> { /* ... */ }
```

#### Function `inhalation_dose`

**Attributes:**

- `MustUse { reason: None }`

Inhalation dose for one nuclide:
`chi/Q * Q * DCF_inh * breathing rate * 1000` (mSv, or mSv/y).

`dcf_sv_per_bq` comes from [`super::dcf::InhalationDcfTable::lookup`] (NaN
propagates, as upstream). Returns `None` only for a NaN age.

```rust
pub fn inhalation_dose(chi_over_q: super::units::DilutionFactor, release: Release, dcf_sv_per_bq: f64, age_years: f64) -> Option<super::units::EffectiveDose> { /* ... */ }
```

#### Function `deposition_velocity_m_per_s`

**Attributes:**

- `MustUse { reason: None }`

Upstream's total (dry + wet) deposition velocity by **element symbol**,
m/s (`deposition_velocity_of_rad`, citing IAEA SRS 19 p. 27):

- `0` for H, C and the noble gases He, Ne, Ar, Kr, Xe, Rn;
- `0.1` for F, Cl, Br;
- `1000 m/d = 1000 / 86400 m/s` for everything else (SRS 19's screening
  value for aerosols and reactive gases).

Note (upstream behaviour, kept): iodine gets the aerosol value, not the
reactive-halogen 0.1 m/s that F, Cl and Br get.

```rust
pub fn deposition_velocity_m_per_s(element: &str) -> f64 { /* ... */ }
```

#### Function `effective_buildup_time_s`

**Attributes:**

- `MustUse { reason: None }`

Upstream's effective build-up time on the ground, s:
`(1 - exp(-lambda_e T)) / lambda_e`, with `lambda_e` the decay constant
plus the weathering rate and `T` the exposure period (years * 365 d).

Multiplying a deposition rate (Bq m^-2 s^-1) by this gives the ground
concentration at the end of `T` (Bq/m^2). A stable nuclide (`lambda = 0`,
weathering off) gives NaN, as upstream.

```rust
pub fn effective_buildup_time_s(decay_constant_per_s: f64, element: &str, weathering: Weathering, exposure_period_years: f64) -> f64 { /* ... */ }
```

#### Function `ground_shine_dose`

**Attributes:**

- `MustUse { reason: None }`

Ground-shine dose for one nuclide, upstream's arithmetic:

```text
deposition rate = chi/Q * (Q / year_s) * v_d          [Bq m^-2 s^-1]
ground conc.    = deposition rate * build-up time      [Bq m^-2]
dose            = ground conc. * DCF_gs * 1000 * year_s  [mSv (per year)]
```

`dcf_gs` (Sv m^2 Bq^-1 s^-1) from [`super::dcf::external_dcf`] with
[`super::dcf::ExternalDcfPair::selected`].

Note (upstream behaviour, kept): the same formula is used for an
instantaneous release, where `Q` is total Bq; it then amounts to a
constant deposition rate `Q / year` held for the exposure period and an
exposure of one year at the resulting concentration.

```rust
pub fn ground_shine_dose(chi_over_q: super::units::DilutionFactor, release: Release, deposition_velocity_m_per_s: f64, effective_buildup_time_s: f64, dcf_gs: f64) -> super::units::EffectiveDose { /* ... */ }
```

#### Function `submersion_dose`

**Attributes:**

- `MustUse { reason: None }`

Submersion dose for one nuclide: `chi/Q * Q * DCF_sub * 1000` (mSv, or
mSv/y). `dcf_sub` (Sv m^3 Bq^-1 s^-1) from [`super::dcf::external_dcf`].

```rust
pub fn submersion_dose(chi_over_q: super::units::DilutionFactor, release: Release, dcf_sub: f64) -> super::units::EffectiveDose { /* ... */ }
```

#### Function `age_bracket`

**Attributes:**

- `MustUse { reason: None }`

The age bracket a pathway would use for its coefficient, re-exported here
for callers that assemble doses by hand.

```rust
pub fn age_bracket(age_years: f64) -> Option<super::dcf::AgeBracket> { /* ... */ }
```

### Constants and Statics

#### Constant `UPSTREAM_YEAR_S`

Seconds in upstream's year for the dose pathways: `365 * 24 * 3600`.

```rust
pub const UPSTREAM_YEAR_S: f64 = 31_536_000.0;
```

## Module `met`

Meteorological-data processing as pyDOSEIA does it: hourly records to a
**triple joint frequency distribution** (TJFD: stability class x wind-speed
class x wind-direction sector), the missing-data correction, the calm
correction and the per-class speed distribution.

# Provenance

Ported from pyDOSEIA `metfunc.py` (`MetFunc.file_preprocessing`,
`met_data_to_tjfd`, `missing_correction`, `calm_correction_factor_calc`,
`speed_distribution_list`), upstream
<https://github.com/BiswajitSadhu/pyDOSEIA> at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). Upstream cites
the Hukkoo-Bapat BARC manual for the calm correction.

Upstream reads an Excel workbook (one sheet per year). This port takes the
records already parsed ([`RawMetRecord`]); reading a spreadsheet is left to
the caller. Everything from the gap filling onward is ported.

# Units

Wind speeds are **km/h** here, as in upstream's input column
(`WS 10m(kmph)`) and its speed-class edges. They are converted to m/s only
inside the dilution factor (`/ 3.6`). Directions are degrees (the direction
the wind blows *from*, as recorded). Records are plain `f64` because they
mirror a spreadsheet row, including upstream's `999` / `9` missing-value
sentinels.

```rust
pub mod met { /* ... */ }
```

### Types

#### Type Alias `FrequencyTable`

One stability class's joint frequency table: `[speed class][sector]`.

```rust
pub type FrequencyTable<T> = [[T; 16]; 10];
```

#### Struct `RawMetRecord`

One hourly record as read from upstream's spreadsheet, before gap filling.

```rust
pub struct RawMetRecord {
    pub hour: f64,
    pub speed_kmph: Option<f64>,
    pub direction_deg: Option<f64>,
    pub stability_code: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `hour` | `f64` | Hour of day (upstream's first column), used only for the operating-hours<br>filter. |
| `speed_kmph` | `Option<f64>` | Wind speed at the measurement height, km/h; `None` if blank. |
| `direction_deg` | `Option<f64>` | Wind direction, degrees; `None` if blank. |
| `stability_code` | `Option<f64>` | Stability class code: `1..=6`, or a letter's `ord - 64` (`'A'` = 1);<br>`None` if blank. |

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
    fn clone(self: &Self) -> RawMetRecord { /* ... */ }
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
    fn eq(self: &Self, other: &RawMetRecord) -> bool { /* ... */ }
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
#### Struct `MetRecord`

A record after upstream's gap filling: `[speed, direction, stability]`
with `999` for a missing speed/direction and `9` for a missing class.

```rust
pub struct MetRecord {
    pub speed_kmph: f64,
    pub direction_deg: f64,
    pub stability_code: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `speed_kmph` | `f64` | Wind speed, km/h (or [`MISSING_VALUE`]). |
| `direction_deg` | `f64` | Wind direction, degrees (or [`MISSING_VALUE`]). |
| `stability_code` | `f64` | Stability code (or [`MISSING_STABILITY`]). |

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
    fn clone(self: &Self) -> MetRecord { /* ... */ }
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
    fn eq(self: &Self, other: &MetRecord) -> bool { /* ... */ }
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
#### Struct `MetYear`

One year (one upstream sheet) of processed met data.

```rust
pub struct MetYear {
    pub num_days: i64,
    pub records: Vec<MetRecord>,
    pub tjfd: [FrequencyTable<f64>; 6],
    pub missing_corrected: [FrequencyTable<i64>; 6],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `num_days` | `i64` | Number of days the year's data covers (upstream `num_days`). |
| `records` | `Vec<MetRecord>` | The gap-filled records inside the operating hours. |
| `tjfd` | `[FrequencyTable<f64>; 6]` | The raw TJFD, per stability class. |
| `missing_corrected` | `[FrequencyTable<i64>; 6]` | The missing-corrected, integer TJFD, per stability class. |

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
    fn clone(self: &Self) -> MetYear { /* ... */ }
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
    fn eq(self: &Self, other: &MetYear) -> bool { /* ... */ }
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
#### Struct `MetClimatology`

Several years of met data processed as upstream does, ready for
`dispersion::dilution_long_term_with_met`.

```rust
pub struct MetClimatology {
    pub start_hour: i64,
    pub end_hour: i64,
    pub years: Vec<MetYear>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start_hour` | `i64` | First operating hour kept (upstream `start_operation_time`). |
| `end_hour` | `i64` | Last operating hour kept (upstream `end_operation_time`). |
| `years` | `Vec<MetYear>` | One entry per year (upstream sheet), in input order. |

##### Implementations

###### Methods

- ```rust
  pub fn from_raw_years(years: &[(&[RawMetRecord], i64)], start_hour: i64, end_hour: i64) -> Self { /* ... */ }
  ```
  Process raw records, one slice per year with its day count, through

- ```rust
  pub fn operation_hours_per_day(self: &Self) -> i64 { /* ... */ }
  ```
  Operating hours per day, `|start - end|` (upstream's definition; note

- ```rust
  pub fn all_records(self: &Self) -> Vec<MetRecord> { /* ... */ }
  ```
  All years' records concatenated (upstream's input to the speed

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> MetClimatology { /* ... */ }
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
    fn eq(self: &Self, other: &MetClimatology) -> bool { /* ... */ }
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

#### Function `stability_code_from_letter`

**Attributes:**

- `MustUse { reason: None }`

Stability letter to upstream's code, `ord(letter) - 64` (`'A'` = 1).

```rust
pub fn stability_code_from_letter(letter: char) -> f64 { /* ... */ }
```

#### Function `preprocess_records`

**Attributes:**

- `MustUse { reason: None }`

Upstream's `file_preprocessing` for one sheet: keep the hours in
`[start, end]` (inclusive), fill blanks with `999` (speed, direction) or
`9` (stability).

```rust
pub fn preprocess_records(raw: &[RawMetRecord], start_hour: f64, end_hour: f64) -> Vec<MetRecord> { /* ... */ }
```

#### Function `invalid_record_count`

**Attributes:**

- `MustUse { reason: None }`

Upstream's count of **invalid** records: direction above 360 degrees or
stability code above 6.

Note (upstream behaviour, kept): a record with a missing *speed* (999) but
a valid direction and class is counted as **valid**, yet it lands in no
speed class (999 km/h is beyond the last edge) and so is silently dropped
from the TJFD without being redistributed by the missing correction.

```rust
pub fn invalid_record_count(records: &[MetRecord]) -> usize { /* ... */ }
```

#### Function `tjfd_from_records`

**Attributes:**

- `MustUse { reason: None }`

Upstream's `met_data_to_tjfd` for one year: a per-class histogram of speed
class against direction bin, with the first (`[0, 11.25)`) and last
(`[348.75, 360]`) direction bins folded into sector 0.

Counts are `f64` because upstream's histogram is floating point.

```rust
pub fn tjfd_from_records(records: &[MetRecord]) -> [FrequencyTable<f64>; 6] { /* ... */ }
```

#### Function `missing_correction`

**Attributes:**

- `MustUse { reason: None }`

Upstream's `missing_correction` for one year: each count is scaled by
`1 + invalid / valid` and **truncated to an integer** (`astype(int)`), as
upstream does.

```rust
pub fn missing_correction(tjfd: &[FrequencyTable<f64>; 6], invalid: usize, total: usize) -> [FrequencyTable<i64>; 6] { /* ... */ }
```

#### Function `calm_correction_factors`

**Attributes:**

- `MustUse { reason: None }`

Upstream's calm-correction factors, one per sector:
`1 + N_0 N_JL / (N_L N_J)`, with `N_0` the calm count, `N_J` the non-calm
count in sector `J`, `N_JL` the count in the lowest non-calm speed class in
sector `J` (all over the six classes).

**Upstream defect D1, reproduced here:** upstream computes `N_L` as
`TJFD.reshape(6,10,16).sum(axis=1).sum(axis=0)[1]`, which is the total count
in **direction sector 1 over all speed classes** (including calm), not the
total in the lowest speed class over all sectors that the formula calls
for. [`calm_correction_factors_lowest_speed_class`] is the corrected
variant. A sector with `N_J = 0` gives `inf`/NaN, as upstream.

```rust
pub fn calm_correction_factors(tjfd: &[FrequencyTable<i64>; 6]) -> [f64; 16] { /* ... */ }
```

#### Function `calm_correction_factors_lowest_speed_class`

**Attributes:**

- `MustUse { reason: None }`

**Divergence from upstream (D1 corrected).** As
[`calm_correction_factors`] but with `N_L` = the total count in the lowest
non-calm speed class (class 1) over every sector and stability class. Not
verified against the Hukkoo-Bapat manual itself, which was not available;
the correction follows from the formula's own definition of `N_L`.

```rust
pub fn calm_correction_factors_lowest_speed_class(tjfd: &[FrequencyTable<i64>; 6]) -> [f64; 16] { /* ... */ }
```

#### Function `speed_distribution`

**Attributes:**

- `MustUse { reason: None }`

Upstream's `speed_distribution_list`: for each stability class, the speeds
strictly below the class's `quantile` (default 0.90), and their mean, km/h.

Records with a missing speed (999 km/h) are **included** in the quantile,
as upstream does; with fewer than 10 % missing they are then cut by the
0.9 quantile. A class with no records gives a NaN mean.

Upstream uses these means (km/h) to divide a single-plume dilution factor
computed at a 1 m/s reference speed, but that path fails its own shape
assertion (defect D3), so nothing in this port consumes them.

```rust
pub fn speed_distribution(records: &[MetRecord], quantile: f64) -> ([Vec<f64>; 6], [f64; 6]) { /* ... */ }
```

### Constants and Statics

#### Constant `WSRANGE_KMPH`

Upstream's wind-speed class edges, km/h (10 classes; class 0 is calm).

```rust
pub const WSRANGE_KMPH: [f64; 11] = _;
```

#### Constant `WDRANGE_DEG`

Upstream's wind-direction bin edges, degrees (17 bins; the first and last
are folded into sector 0).

```rust
pub const WDRANGE_DEG: [f64; 18] = _;
```

#### Constant `WSPEED_K_KMPH`

Representative speed of each speed class, km/h (upstream `WSPEED_K` before
its `/ 3.6`). Class 0 (calm) is never used in a dilution factor.

```rust
pub const WSPEED_K_KMPH: [f64; 10] = _;
```

#### Constant `SPEED_CLASS_COUNT`

Number of wind-speed classes.

```rust
pub const SPEED_CLASS_COUNT: usize = 10;
```

#### Constant `SECTOR_COUNT`

Number of direction sectors.

```rust
pub const SECTOR_COUNT: usize = 16;
```

#### Constant `CALM_SPEED_CLASS`

Index of the calm speed class.

```rust
pub const CALM_SPEED_CLASS: usize = 0;
```

#### Constant `MISSING_VALUE`

Upstream's fill value for a missing speed or direction.

```rust
pub const MISSING_VALUE: f64 = 999.0;
```

#### Constant `MISSING_STABILITY`

Upstream's code for a missing stability class (`'I'` = `ord('I') - 64`).

```rust
pub const MISSING_STABILITY: f64 = 9.0;
```

## Module `nuclide`

Half-life text parsing and decay constants, as pyDOSEIA does them.

# Provenance

Ported from pyDOSEIA `raddcffunc.py`: the nested
`convert_half_life_to_seconds` of `RaddcfFunc.get_nuclide_info` (primary
half-life table) and the half-life branch of
`RaddcfFunc.find_progeny_name_and_yield_f` (progeny table). Upstream
<https://github.com/BiswajitSadhu/pyDOSEIA> at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).

# No half-life data here

Upstream bundles half-life tables (its `library/half_life/`, derived from
ICRP Publication 107 and JAERI-Data/Code 2002-013). **None is copied into
this crate.** The workspace's half-life source is `boon-lay`
(`boon_lay::nuclide_reaction_and_decay_data`, `try_get_half_life`); a
caller supplies decay constants to [`super::dose`]. This module only
parses upstream's text formats, so a user-supplied table in upstream's
format can be read and the port can be verified against upstream.

# Two different year lengths and `ln 2 = 0.693`

Upstream is not internally consistent, and the port keeps both behaviours:
the primary-table parser uses a Gregorian year of **31 556 952 s**, the
progeny parser uses **365 days**, and the decay constant is
`0.693 / T_half` (not `ln 2 = 0.693147...`, a relative difference of
2.1e-4).

```rust
pub mod nuclide { /* ... */ }
```

### Functions

#### Function `upstream_decay_constant`

**Attributes:**

- `MustUse { reason: None }`

Upstream's decay constant, `0.693 / T_half`, 1/s, for a half-life in s.

```rust
pub fn upstream_decay_constant(half_life_s: f64) -> f64 { /* ... */ }
```

#### Function `parse_primary_half_life`

**Attributes:**

- `MustUse { reason: None }`

Parse a primary-table half-life string such as `"30.0 y"`, `"8.0 d"`,
`"3.0 ms"` to seconds (upstream `convert_half_life_to_seconds`).

Units are tried in upstream's order, `ls` (1e-6), `ms`, `s`, `m`, `h`,
`d`, `y` (31 556 952 s), by *suffix*; the unit is then removed and the
rest parsed. Returns `None` if no unit matches or the number does not
parse, as upstream does.

```rust
pub fn parse_primary_half_life(text: &str) -> Option<f64> { /* ... */ }
```

#### Function `parse_progeny_half_life`

**Attributes:**

- `MustUse { reason: None }`

Parse a progeny-table half-life string such as `"2.5 m"` or `"12.32 y"` to
seconds, exactly as upstream's `find_progeny_name_and_yield_f` does.

The unit tests are **substring** tests in upstream's order: `'s'`, then
`'m'`, `'d'`, `'a'` or `'y'` (365 days), `'h'`; the number is everything
but the last character. A string containing none of these letters is an
error upstream (`ValueError`); here it is `None`.

```rust
pub fn parse_progeny_half_life(text: &str) -> Option<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `UPSTREAM_LN2`

Upstream's value of `ln 2`, as written: `0.693`.

```rust
pub const UPSTREAM_LN2: f64 = 0.693;
```

## Module `units`

The dose newtype of the pyDOSEIA port, and the dilution-factor type it
shares with `changi`.

# Why dose is a newtype and not a `uom` quantity

`uom` 0.38 has no sievert, gray or equivalent-dose quantity (checked
2026-09-28; there is no `absorbed_dose` or `dose_equivalent` module in
`uom::si`). `AvailableEnergy` (J/kg) has the right *dimension* and was
rejected on purpose: a sievert is a J/kg only after radiation and tissue
weighting, and a type that lets a dose be added to a specific energy would
hide that. [`EffectiveDose`] therefore wraps an `f64` and names its unit in
every accessor.

**It stores millisieverts, not sieverts.** Upstream computes every dose in
mSv (it multiplies by `1000` inside each pathway), and the code-to-code test
compares the port against upstream's mSv values. Storing the mSv value as
computed keeps that comparison bit-exact; converting to Sv and back would
add a rounding step upstream does not have.

# The dilution factor is `changi`'s

`chi/Q` (s/m^3) is [`DilutionFactor`], re-exported from
`changi::activity::units`, the workspace's existing type for it. The port
therefore hands its dilution factors straight to anything in `changi`
and takes `changi`'s. `buangkok -> changi` is the dependency direction: dose
sits downstream of dispersion.

```rust
pub mod units { /* ... */ }
```

### Types

#### Struct `EffectiveDose`

An effective dose (or, for a long-term release, the dose accrued over one
year of discharge), as computed by the pyDOSEIA port.

Stored in **millisieverts** (see the module docs). Research-grade only:
never a dose to a real person (`RESPONSIBLE_USE.md`).

```rust
pub struct EffectiveDose(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Methods

- ```rust
  pub const fn from_millisieverts(msv: f64) -> Self { /* ... */ }
  ```
  From a value in millisieverts.

- ```rust
  pub fn from_sieverts(sv: f64) -> Self { /* ... */ }
  ```
  From a value in sieverts.

- ```rust
  pub const fn millisieverts(self: Self) -> f64 { /* ... */ }
  ```
  The dose in millisieverts (exactly the value upstream reports).

- ```rust
  pub fn sieverts(self: Self) -> f64 { /* ... */ }
  ```
  The dose in sieverts.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> EffectiveDose { /* ... */ }
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
    fn default() -> EffectiveDose { /* ... */ }
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
    fn eq(self: &Self, other: &EffectiveDose) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &EffectiveDose) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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
### Re-exports

#### Re-export `DilutionFactor`

```rust
pub use changi::activity::units::DilutionFactor;
```

## Constants and Statics

### Constant `SCOPE`

The scope this crate reserves, as a machine-readable string.

Created when the crate was a placeholder (2026-09-28) so it had something
testable. It is kept so that a downstream `use buangkok::SCOPE;` fails
loudly if the crate is ever repurposed without updating its own
documentation.

```rust
pub const SCOPE: &str = "radiation dose and bioeffects for research-grade safety analysis";
```

