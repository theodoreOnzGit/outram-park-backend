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

# STATUS: pyDOSEIA ported (2026-09-28)

~~PLACEHOLDER. Nothing is implemented.~~ **CHANGED 2026-09-28** (maintainer:
"work on translating pyDOSEIA into buangkok under a module", then "port all
of pyDOSEIA into buangkok"). The crate holds [`pydoseia`], a faithful port
of the MIT-licensed pyDOSEIA code (Sadhu et al., *Health Physics* 130(1)
(2026) 94-110, doi:10.1097/HP.0000000000002014): met processing,
Gaussian-plume dilution factors, the inhalation, ground-shine, submersion,
ingestion and plume-shine pathways, multi-source DCF screening, plume rise,
the run configuration and the driver with its summary tables. ~~Ingestion
and plume shine are **not ported**~~ (**ported 2026-09-28**, second
tranche). Everything is **code-to-code verified against upstream** on
synthetic inputs (`tests/pydoseia_code_to_code.rs`: 1 899 cases, 41 of 43
groups bit-exact). The agreement is with pyDOSEIA, not with experiment,
and there is no validation of any kind; 26 upstream defects are recorded
(`docs/pydoseia-code-to-code.md`). What is not ported is I/O and UI (see
`docs/pydoseia-port-scoping.md`). ~~No dose-coefficient data ships with the
crate.~~ **CHANGED 2026-09-29:** the port ships none, but [`coefficients`]
holds US EPA FGR-15 (2025) and FGR-11 coefficients for the five nuclides
`htgr_sim_v1` tracks, which that example's Map tab uses for an indicative
dose rate. Human V&V review is still outstanding (see README).

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

## Module `coefficients`

Freely usable US EPA dose coefficients (FGR-15 2025 external, FGR-11
inhalation) for the nuclides `htgr_sim_v1` tracks, as the port's own table
types. Not a port; see the module docs for provenance.
Freely usable dose coefficients from the US EPA Federal Guidance Reports,
for the five nuclides `htgr_sim_v1` tracks (Kr-85, Xe-133, I-131, Cs-137,
Ag-110m) plus Cs-137's short-lived daughter Ba-137m.

**Not a port.** The pyDOSEIA port ([`crate::pydoseia`]) ships no
coefficient data and takes caller-supplied tables in upstream's CSV layout.
This module is such a caller-supplied set, compiled in from `reference/`,
and returned as the port's own table types so the port's lookups
([`crate::pydoseia::dcf::external_dcf`],
[`crate::pydoseia::dcf::InhalationDcfTable`]) do the selecting.
Added 2026-09-29 when the maintainer asked for a dose-rate map in
`htgr_sim_v1`.

| Table | Source | CSV |
|---|---|---|
| air submersion, Sv m^3 Bq^-1 s^-1 | FGR-15 (EPA 402-R-25-001, **July 2025 revision**), Table 4-6 | `fgr15_2025_air_submersion_dose_rate_coefficients.csv` |
| ground surface, Sv m^2 Bq^-1 s^-1 | FGR-15 (2025), Table 4-1 | `fgr15_2025_ground_surface_dose_rate_coefficients.csv` |
| Cs-137 -> Ba-137m, 0.944; T1/2 2.552 min | FGR-15 (2025), worked Example 4, pp. 269-270 | `fgr15_2025_short_lived_progeny_*.csv` |
| inhalation, committed Sv/Bq, adult | FGR-11 (EPA-520/1-88-020, 1988), Table 2.1, "Effective" | `fgr11_inhalation_committed_dose_coefficients.csv` |

Page numbers, the licence basis (EPA's statement: non-commercial, scientific
and educational use) and how each value was read are in
`crates/buangkok/docs/References.md`. The withdrawn 2019 FGR-15
(EPA-402/R-19/002) is **not** used: EPA says its tables contain errors.

# Two known inconsistencies, stated rather than hidden

- FGR-15 (2025) coefficients are **ICRP 103** effective dose; FGR-11's are
  ICRP 26/30 **committed effective dose equivalent** for Reference Man.
  Adding them is common screening practice but mixes two weighting
  schemes.
- FGR-11 is **adult only**; the other five age columns of its CSV are blank
  (NaN), so a non-adult inhalation lookup returns `None`, never a number.

Research-grade only: never a dose to a real person, and not for emergency,
regulatory, occupational or medical use (`RESPONSIBLE_USE.md`).

```rust
pub mod coefficients { /* ... */ }
```

### Functions

#### Function `fgr15_air_submersion`

**Attributes:**

- `MustUse { reason: None }`

FGR-15 (2025) Table 4-6, air submersion.

# Panics
Never for the shipped CSV (a test parses it).

```rust
pub fn fgr15_air_submersion() -> crate::pydoseia::dcf::ExternalDcfTable { /* ... */ }
```

#### Function `fgr15_ground_surface`

**Attributes:**

- `MustUse { reason: None }`

FGR-15 (2025) Table 4-1, ground surface.

# Panics
Never for the shipped CSV (a test parses it).

```rust
pub fn fgr15_ground_surface() -> crate::pydoseia::dcf::ExternalDcfTable { /* ... */ }
```

#### Function `fgr15_short_lived_progeny`

**Attributes:**

- `MustUse { reason: None }`

The one short-lived progeny link these nuclides need (Cs-137 -> Ba-137m).

# Panics
Never for the shipped CSVs (a test parses them).

```rust
pub fn fgr15_short_lived_progeny() -> crate::pydoseia::dcf::ProgenyChains { /* ... */ }
```

#### Function `fgr11_inhalation`

**Attributes:**

- `MustUse { reason: None }`

FGR-11 Table 2.1 inhalation, adult only, in the port's inhalation-table
layout (the `Type` column holds FGR-11's D/W/Y clearance class).

# Panics
Never for the shipped CSV (a test parses it).

```rust
pub fn fgr11_inhalation() -> crate::pydoseia::dcf::InhalationDcfTable { /* ... */ }
```

#### Function `external_coefficient`

**Attributes:**

- `MustUse { reason: None }`

An external dose-rate coefficient through the port's own lookup
([`dcf::external_dcf`], with [`PROGENY`]), or `None` when the table has no
row for `nuclide` at that age. **`None` means missing, never zero.**

```rust
pub fn external_coefficient(table: &crate::pydoseia::dcf::ExternalDcfTable, chains: &crate::pydoseia::dcf::ProgenyChains, nuclide: &str, age: crate::pydoseia::dcf::AgeBracket) -> Option<f64> { /* ... */ }
```

#### Function `fgr11_inhalation_max_over_classes`

**Attributes:**

- `MustUse { reason: None }`

The FGR-11 inhalation coefficient for `nuclide` at `age`, taking the
**largest over FGR-11's lung clearance classes** (the same rule as the
port's `LungAbsorptionType::Max`; with no chemical-form information that is
the conservative choice). `None` when there is no entry -- the noble gases,
and every non-adult age, since FGR-11 is adult only.

```rust
pub fn fgr11_inhalation_max_over_classes(table: &crate::pydoseia::dcf::InhalationDcfTable, nuclide: &str, age: crate::pydoseia::dcf::AgeBracket) -> Option<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `PROGENY`

The progeny correction used with these tables: include daughters with a
half-life of at most 1800 s (pyDOSEIA's default `ignore_half_life`).
For the five nuclides here that adds exactly one term, Ba-137m to Cs-137,
taken in secular equilibrium (0.944 Bq of Ba-137m per Bq of Cs-137).

```rust
pub const PROGENY: crate::pydoseia::dcf::ProgenyCorrection = _;
```

## Module `pydoseia`

Port of pyDOSEIA (met processing, Gaussian-plume dilution, inhalation,
ground-shine, submersion, ingestion and plume-shine doses, the driver),
code-to-code verified against upstream. See the module docs for
provenance and scope.
# A port of pyDOSEIA: Gaussian-plume dilution and five dose pathways

> **Research, education and V&V only.** Not for medical, occupational,
> public-health, emergency-response, licensing or regulatory use, and
> never a dose to a real person or population (`RESPONSIBLE_USE.md`).

## Upstream and provenance

| | |
|---|---|
| Upstream project | **pyDOSEIA**, <https://github.com/BiswajitSadhu/pyDOSEIA> |
| Commit ported | `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce` (branch `head`, 2025-08-26), checked 2026-09-28 |
| Source files | `metfunc.py`, `dosefunc.py`, `raddcffunc.py`, `outputfunc.py`, `main.py`, the input generator's defaults (function-level mapping in each submodule) |
| Copyright | Copyright (c) 2024 Dr. Biswajit Sadhu |
| Licence | MIT. The full notice is in `crates/buangkok/NOTICE` and must stay with every ported file. MIT is compatible with this crate's GPL-3.0 |
| Paper | B. Sadhu, T. Sarkar, S. Anand, K. D. Singh, D. K. Aswal, "pyDOSEIA: A Python Package for Radiological Impact Assessment during Long-term or Accidental Atmospheric Releases", *Health Physics* **130**(1) (2026) 94-110, doi:[10.1097/HP.0000000000002014](https://doi.org/10.1097/HP.0000000000002014), PMID [40622262](https://pubmed.ncbi.nlm.nih.gov/40622262/). The article is (c) 2025 Health Physics Society: cited, not reproduced |

## What is ported, and how it is verified

**All of pyDOSEIA's computation** (2026-09-28, two tranches); what is not
ported is I/O and UI (Excel reading, plots, text formatting, the YAML
dialogue, joblib), listed function by function in
`docs/pydoseia-port-scoping.md`.

| Module | Upstream | Status |
|---|---|---|
| [`met`](crate::pydoseia::met) | met processing: gap filling, TJFD, missing and calm corrections, speed distribution | ported, code-to-code verified |
| [`dispersion`](crate::pydoseia::dispersion) | sigmas, height correction, master equations, dilution factor | ported, code-to-code verified (3 modes); the 4th (single plume with met data, D3) as a labelled divergence |
| [`nuclide`](crate::pydoseia::nuclide) | half-life text parsing, `0.693 / T` | ported, code-to-code verified |
| [`dcf`](crate::pydoseia::dcf) | age brackets, absorption-type lookup, progeny correction | ported, code-to-code verified on **synthetic** tables |
| [`dose`](crate::pydoseia::dose) | inhalation, ground shine, submersion, deposition velocity, weathering | ported, code-to-code verified |
| [`ingestion`](crate::pydoseia::ingestion) | SRS 19 food chain, H-3 and C-14 models | ported, code-to-code verified (faithful driver, D8-D14); corrected per-nuclide driver as a divergence |
| [`plume_shine`](crate::pydoseia::plume_shine) | finite-cloud gamma dose, photon tables, integration limits, point source | ported, code-to-code verified, bit-exact |
| [`quadpack`](crate::pydoseia::quadpack) | SciPy's QUADPACK `dqagse` and `tplquad`: the bit-exact **regression reference** for plume shine (the default integrator is petir's GSL QAGS) | ported, verified against SciPy directly |
| [`dcf_screening`](crate::pydoseia::dcf_screening) | multi-source DCF screening (report only, D7) | ported, code-to-code verified |
| [`plume_rise`](crate::pydoseia::plume_rise) | plume rise, building wake (never called upstream) | ported / labelled divergences (D5, D6) |
| [`config`](crate::pydoseia::config) | the run configuration and its defaults | ported (schema and checks) |
| [`assessment`](crate::pydoseia::assessment) | the driver and the summary tables | ported, code-to-code verified against upstream's joblib run |

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
defects found while porting (D1-D26 in `docs/pydoseia-code-to-code.md`).
Where a defect is clear, a corrected variant sits beside the faithful one
and is labelled as a **divergence**, e.g.
[`dispersion::CalmCorrection::LowestSpeedClassTotal`](crate::pydoseia::dispersion::CalmCorrection::LowestSpeedClassTotal) (D1),
[`dispersion::max_dilution_factor`](crate::pydoseia::dispersion::max_dilution_factor) (D2),
[`ingestion::corrected`](crate::pydoseia::ingestion::corrected) (D8-D11) and
[`assessment::SummaryIngestion::PerNuclideRowsOnly`](crate::pydoseia::assessment::SummaryIngestion::PerNuclideRowsOnly) (D20).

## No data tables

No dose coefficient, transfer factor, photon line or attenuation
coefficient, half-life, decay chain or met record from upstream's
`library/` or `met_data/` is in this crate. The caller supplies them in
upstream's column layouts (see [`dcf`](crate::pydoseia::dcf),
[`ingestion`](crate::pydoseia::ingestion),
[`plume_shine`](crate::pydoseia::plume_shine) and
[`dcf_screening`](crate::pydoseia::dcf_screening) for the per-table licence
reasoning). A whole run is in `examples/pydoseia_assessment.rs`.

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

## Module `assessment`

The pyDOSEIA driver: every distance and age of a configuration through the
dilution factor and all five pathways, and the summary tables upstream
writes.

# Provenance

Ported from pyDOSEIA `outputfunc.py` (`OutputFunc.dose_calculation_script`,
`dil_fac_all_sectors_all_dist`, `agewise_dose_inh_gs_submersion`,
`agewise_ingestion_dose`, `all_dist_agewise_plume_shine_dose`,
`agewise_dcfs_inh_gs_submersion`, the totals in `output_to_txt`),
`metfunc.py` (`get_max_dilution_factor`) and `main.py`
(`reshape_ingestion_dose_data`, `reshape_dose_data`,
`process_plume_doses_have_met_data`, `process_plume_doses_have_no_met_data`),
upstream <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).

# What is not ported

Upstream runs the (distance, age) grid through `joblib.Parallel`; the port
runs it in a plain loop, which gives the same numbers (each cell is
independent). The text report, CSV writing, pickling, logging and plots are
output formatting and are not ported; the **numbers** they print are, as
the functions below.

```rust
pub mod assessment { /* ... */ }
```

### Types

#### Struct `AssessmentTables`

The coefficient tables a run reads (all caller-supplied; see each type).

```rust
pub struct AssessmentTables {
    pub inhalation: crate::pydoseia::dcf::InhalationDcfTable,
    pub surface: crate::pydoseia::dcf::ExternalDcfTable,
    pub submersion: crate::pydoseia::dcf::ExternalDcfTable,
    pub chains: crate::pydoseia::dcf::ProgenyChains,
    pub ingestion: crate::pydoseia::ingestion::IngestionDcfTable,
    pub eco: crate::pydoseia::ingestion::EcoParamTable,
    pub gamma: crate::pydoseia::plume_shine::GammaLineTable,
    pub attenuation: crate::pydoseia::plume_shine::AttenuationTable,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inhalation` | `crate::pydoseia::dcf::InhalationDcfTable` | Inhalation e(g). |
| `surface` | `crate::pydoseia::dcf::ExternalDcfTable` | Ground-surface dose-rate coefficients. |
| `submersion` | `crate::pydoseia::dcf::ExternalDcfTable` | Air-submersion dose-rate coefficients. |
| `chains` | `crate::pydoseia::dcf::ProgenyChains` | Decay chains for the progeny correction. |
| `ingestion` | `crate::pydoseia::ingestion::IngestionDcfTable` | Ingestion e(g). |
| `eco` | `crate::pydoseia::ingestion::EcoParamTable` | Element transfer factors. |
| `gamma` | `crate::pydoseia::plume_shine::GammaLineTable` | Gamma lines (plume shine). |
| `attenuation` | `crate::pydoseia::plume_shine::AttenuationTable` | Air attenuation (plume shine). |

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
    fn clone(self: &Self) -> AssessmentTables { /* ... */ }
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
    fn default() -> AssessmentTables { /* ... */ }
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
    fn eq(self: &Self, other: &AssessmentTables) -> bool { /* ... */ }
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
#### Struct `PathwayDoses`

Inhalation, ground-shine and submersion doses per nuclide, mSv (mSv/y for
a long-term release).

```rust
pub struct PathwayDoses {
    pub inhalation: Vec<f64>,
    pub ground_shine: Vec<f64>,
    pub submersion: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inhalation` | `Vec<f64>` | Inhalation, per nuclide. |
| `ground_shine` | `Vec<f64>` | Ground shine, per nuclide. |
| `submersion` | `Vec<f64>` | Submersion, per nuclide. |

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
    fn clone(self: &Self) -> PathwayDoses { /* ... */ }
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
    fn default() -> PathwayDoses { /* ... */ }
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
    fn eq(self: &Self, other: &PathwayDoses) -> bool { /* ... */ }
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
#### Enum `IngestionFailure`

Why a (distance, age) cell has no ingestion result.

```rust
pub enum IngestionFailure {
    AgeHasNoReceiver,
    Upstream(crate::pydoseia::ingestion::upstream::UpstreamIngestionError),
}
```

##### Variants

###### `AgeHasNoReceiver`

The age is neither `> 17` nor `== 1`: upstream's driver has no receiver
for it and raises `UnboundLocalError` (defect D13).

###### `Upstream`

`ingestion_dose` itself raised.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::pydoseia::ingestion::upstream::UpstreamIngestionError` |  |

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
    fn clone(self: &Self) -> IngestionFailure { /* ... */ }
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
    fn eq(self: &Self, other: &IngestionFailure) -> bool { /* ... */ }
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
#### Struct `CellResult`

One (distance, age) cell.

```rust
pub struct CellResult {
    pub distance_m: f64,
    pub age: f64,
    pub pathways: PathwayDoses,
    pub ingestion: Result<crate::pydoseia::ingestion::upstream::UpstreamIngestionOutput, IngestionFailure>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `distance_m` | `f64` | Distance, m. |
| `age` | `f64` | Age, years. |
| `pathways` | `PathwayDoses` | Inhalation, ground shine, submersion. |
| `ingestion` | `Result<crate::pydoseia::ingestion::upstream::UpstreamIngestionOutput, IngestionFailure>` | Ingestion, as upstream's `ingestion_dose` returns it. |

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
    fn clone(self: &Self) -> CellResult { /* ... */ }
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
    fn eq(self: &Self, other: &CellResult) -> bool { /* ... */ }
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
#### Struct `AssessmentResults`

Everything `dose_calculation_script` returns.

```rust
pub struct AssessmentResults {
    pub distances_m: Vec<f64>,
    pub dilution: Vec<Vec<crate::pydoseia::units::DilutionFactor>>,
    pub max_chi_over_q: Vec<crate::pydoseia::units::DilutionFactor>,
    pub cells: Vec<Vec<CellResult>>,
    pub plume_shine: Option<Vec<Vec<Vec<f64>>>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `distances_m` | `Vec<f64>` | Distances computed, plant boundary appended. |
| `dilution` | `Vec<Vec<crate::pydoseia::units::DilutionFactor>>` | Dilution factor per distance: 6 values (per class) without met data,<br>16 (per sector) with; empty for [`DilutionSource::UserSupplied`]. |
| `max_chi_over_q` | `Vec<crate::pydoseia::units::DilutionFactor>` | The `chi/Q` every pathway uses at each distance. |
| `cells` | `Vec<Vec<CellResult>>` | `[distance][age]`. |
| `plume_shine` | `Option<Vec<Vec<Vec<f64>>>>` | Plume shine `[distance][nuclide][class or sector]`, when requested. |

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
    fn clone(self: &Self) -> AssessmentResults { /* ... */ }
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
    fn eq(self: &Self, other: &AssessmentResults) -> bool { /* ... */ }
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
#### Enum `AssessmentError`

Why a run cannot proceed.

```rust
pub enum AssessmentError {
    Config(crate::pydoseia::config::ConfigError),
    SinglePlumeWithMetCannotRun,
    MissingMetData,
    DecayConstants,
    NoDilutionFactorFor(f64),
}
```

##### Variants

###### `Config`

The configuration failed [`PyDoseiaConfig::validate`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::pydoseia::config::ConfigError` |  |

###### `SinglePlumeWithMetCannotRun`

A single-plume release with met data: upstream fails its own shape
assertion (defect D3).

###### `MissingMetData`

Met data configured but none given.

###### `DecayConstants`

Decay constants do not match the nuclide list.

###### `NoDilutionFactorFor`

A user-supplied dilution factor has no entry for a distance.

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
    fn clone(self: &Self) -> AssessmentError { /* ... */ }
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
    fn eq(self: &Self, other: &AssessmentError) -> bool { /* ... */ }
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
#### Enum `Zeroing`

Whether the report zeroes milk and meat for elements without transfer
factors (`zeroing_ingestion`).

```rust
pub enum Zeroing {
    ChainedAssignmentNoOp,
    ZeroMilkAndMeat,
}
```

##### Variants

###### `ChainedAssignmentNoOp`

pandas >= 3 (copy-on-write): upstream's chained assignment changes a
copy and the table is unchanged (defect D14). What the fixture records.

###### `ZeroMilkAndMeat`

What the function means to do (and what pandas < 3 did).

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
    fn clone(self: &Self) -> Zeroing { /* ... */ }
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
    fn eq(self: &Self, other: &Zeroing) -> bool { /* ... */ }
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
#### Struct `BoundaryTotal`

One nuclide's line of upstream's "total dose at plant boundary" table
(`output_to_txt`), mSv (mSv/y).

```rust
pub struct BoundaryTotal {
    pub inhalation: f64,
    pub ground_shine: f64,
    pub submersion: f64,
    pub ingestion: f64,
    pub total: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inhalation` | `f64` | Inhalation. |
| `ground_shine` | `f64` | Ground shine. |
| `submersion` | `f64` | Submersion. |
| `ingestion` | `f64` | Ingestion (sum of veg, milk and meat, NaN as 0). |
| `total` | `f64` | `Total` (NaN as 0). Plume shine is **not** included, as upstream. |

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
    fn clone(self: &Self) -> BoundaryTotal { /* ... */ }
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
    fn eq(self: &Self, other: &BoundaryTotal) -> bool { /* ... */ }
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
#### Struct `SummaryRow`

One (distance, age) row of upstream's `summary_summed_inh_gs_sub_dose.csv`
(`main.reshape_dose_data`), mSv (mSv/y).

```rust
pub struct SummaryRow {
    pub distance_m: f64,
    pub age: f64,
    pub inhalation: f64,
    pub ground_shine: f64,
    pub submersion: f64,
    pub ingestion: f64,
    pub total: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `distance_m` | `f64` | Distance, m. |
| `age` | `f64` | Age, years. |
| `inhalation` | `f64` | Sum over nuclides. |
| `ground_shine` | `f64` | Sum over nuclides. |
| `submersion` | `f64` | Sum over nuclides. |
| `ingestion` | `f64` | Ingestion as the summary reports it. |
| `total` | `f64` | Sum of the four. |

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
    fn clone(self: &Self) -> SummaryRow { /* ... */ }
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
    fn eq(self: &Self, other: &SummaryRow) -> bool { /* ... */ }
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
#### Enum `SummaryIngestion`

How the summary counts ingestion.

```rust
pub enum SummaryIngestion {
    UpstreamDoubleCounted,
    PerNuclideRowsOnly,
}
```

##### Variants

###### `UpstreamDoubleCounted`

Upstream: the group sum runs over the per-nuclide rows **and** the
`SUM` row `reshape_ingestion_dose_data` appended, so ingestion is
counted **twice** (defect D20).

###### `PerNuclideRowsOnly`

**Divergence:** the per-nuclide rows only.

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
    fn clone(self: &Self) -> SummaryIngestion { /* ... */ }
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
    fn eq(self: &Self, other: &SummaryIngestion) -> bool { /* ... */ }
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

#### Function `dilution_for_distance`

Dilution factor for one distance under the configured mode
(`dil_fac_all_sectors_all_dist`).

# Errors
D3, or missing met data.

```rust
pub fn dilution_for_distance(cfg: &crate::pydoseia::config::PyDoseiaConfig, x_m: f64, met: Option<&crate::pydoseia::met::MetClimatology>) -> Result<Vec<crate::pydoseia::units::DilutionFactor>, AssessmentError> { /* ... */ }
```

#### Function `pathway_doses`

**Attributes:**

- `MustUse { reason: None }`

Inhalation, ground shine and submersion for one (distance, age), with the
configured progeny and weathering settings (`agewise_dose_inh_gs_submersion`).

```rust
pub fn pathway_doses(cfg: &crate::pydoseia::config::PyDoseiaConfig, tables: &AssessmentTables, decay_constants_per_s: &[f64], chi: crate::pydoseia::units::DilutionFactor, age: f64) -> PathwayDoses { /* ... */ }
```

#### Function `run_assessment`

The whole run (`dose_calculation_script`).

With [`DilutionSource::UserSupplied`] every pathway uses the caller's
maximum `chi/Q` for the distance, as upstream does (its `MetFunc` keeps
`list_max_dilution_factor` as `dict_max_dilution_factor`; checked in the
fixture's `user_dilution_factor` scenario). No dilution factor per class
or sector is computed then.

# Errors
See [`AssessmentError`].

```rust
pub fn run_assessment(cfg: &crate::pydoseia::config::PyDoseiaConfig, tables: &AssessmentTables, decay_constants_per_s: &[f64], met: Option<&crate::pydoseia::met::MetClimatology>) -> Result<AssessmentResults, AssessmentError> { /* ... */ }
```

#### Function `driver_ingestion_matrix`

**Attributes:**

- `MustUse { reason: None }`

Upstream's driver reshapes each cell's ingestion array to
`(nuclides, 3)` in C order (`INGESTION_DOSES.reshape(..., n, 3)`). For the
transposed layout (H-3, C-14 and others together) that **scrambles** the
routes across nuclides (part of D9); this reproduces it.

```rust
pub fn driver_ingestion_matrix(out: &crate::pydoseia::ingestion::upstream::UpstreamIngestionOutput) -> Vec<[f64; 3]> { /* ... */ }
```

#### Function `pandas_sum`

**Attributes:**

- `MustUse { reason: None }`

pandas' `sum(skipna=True)` over a short row or column: NaN counts as 0 and
the sum is numpy's (pairwise for 8 or more values).

```rust
pub fn pandas_sum(values: &[f64]) -> f64 { /* ... */ }
```

#### Function `numpy_pairwise_sum`

**Attributes:**

- `MustUse { reason: None }`

numpy's pairwise summation (`pairwise_sum_DOUBLE`, block size 128, eight
accumulators).

```rust
pub fn numpy_pairwise_sum(a: &[f64]) -> f64 { /* ... */ }
```

#### Function `pandas_groupby_sum`

**Attributes:**

- `MustUse { reason: None }`

pandas' groupby `sum` (Kahan-compensated, NaN skipped).

```rust
pub fn pandas_groupby_sum(values: &[f64]) -> f64 { /* ... */ }
```

#### Function `plant_boundary_totals`

**Attributes:**

- `MustUse { reason: None }`

The per-nuclide totals `output_to_txt` prints for each age at the plant
boundary. `None` where the cell has no ingestion result.

```rust
pub fn plant_boundary_totals(cfg: &crate::pydoseia::config::PyDoseiaConfig, results: &AssessmentResults, zeroing: Zeroing, no_transfer_factor_elements: &[String]) -> Vec<Option<Vec<BoundaryTotal>>> { /* ... */ }
```

#### Function `summary_rows`

**Attributes:**

- `MustUse { reason: None }`

The rows of upstream's summed summary, one per (distance, age), in
upstream's final order (a stable sort by age). Cells without ingestion are
skipped (upstream crashes before writing the file for them).

```rust
pub fn summary_rows(results: &AssessmentResults, mode: SummaryIngestion) -> Vec<SummaryRow> { /* ... */ }
```

#### Function `plume_shine_maxima`

**Attributes:**

- `MustUse { reason: None }`

`process_plume_doses_*`: per (distance, nuclide) the maximum over classes
or sectors and, for sectors, its index (pandas `idxmax`, first maximum,
NaN skipped).

```rust
pub fn plume_shine_maxima(values: &[f64]) -> (f64, Option<usize>) { /* ... */ }
```

#### Function `dcf_report`

**Attributes:**

- `MustUse { reason: None }`

The coefficients upstream's report prints for one age
(`agewise_dcfs_inh_gs_submersion`): the screened inhalation and ingestion
maxima of [`compute_max_dcf`] (not the coefficients the doses use: D7),
and the ground-surface and submersion coefficient pairs (with and without
progeny, as upstream returns them). The ground-surface one
is looked up with progeny **always included**, whatever the configuration
says (upstream passes `consider_progeny=True` explicitly: defect D22).

```rust
pub fn dcf_report(cfg: &crate::pydoseia::config::PyDoseiaConfig, tables: &AssessmentTables, screening: &crate::pydoseia::dcf_screening::ScreeningTables, alternate_names: &[crate::pydoseia::dcf_screening::AlternateNames], age: f64) -> Vec<(Option<crate::pydoseia::dcf_screening::ScreenedDcf>, crate::pydoseia::dcf::ExternalDcfPair, crate::pydoseia::dcf::ExternalDcfPair)> { /* ... */ }
```

## Module `config`

pyDOSEIA's run configuration, as a typed Rust structure with upstream's
defaults and upstream's consistency checks.

# Provenance

Upstream configures a run with a YAML file, written by hand or by its
interactive input generator (`auto_input_generator.py`,
`auto_input_generator_funcs_class.py`, `auto_input_v18.py`), and checked in
`DoseFunc.__init__` / `OutputFunc.__init__` (`dosefunc.py`,
`outputfunc.py`). Upstream <https://github.com/BiswajitSadhu/pyDOSEIA> at
commit `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024
Dr. Biswajit Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).

The interactive prompts, the YAML reader/writer, logging and the `pickle_it`
dump are user interface and are **not** ported; a Rust caller fills
[`PyDoseiaConfig`] directly. Every key that changes a number is here, under
its upstream name in the field docs. Keys upstream reads but never uses in
a calculation are listed at [`PyDoseiaConfig::unused_upstream_keys`].

```rust
pub mod config { /* ... */ }
```

### Types

#### Enum `ReleaseScenario`

Release scenario (upstream's mutually exclusive `long_term_release` and
`single_plume`).

```rust
pub enum ReleaseScenario {
    LongTerm {
        annual_discharge_bq: Vec<f64>,
    },
    SinglePlume {
        instantaneous_release_bq: Vec<f64>,
    },
}
```

##### Variants

###### `LongTerm`

Continuous release; `annual_discharge_bq_rad_list`, Bq/y per nuclide.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `annual_discharge_bq` | `Vec<f64>` | Bq/y per nuclide. |

###### `SinglePlume`

Instantaneous release; `instantaneous_release_bq_list`, Bq per nuclide.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `instantaneous_release_bq` | `Vec<f64>` | Bq per nuclide. |

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
    fn clone(self: &Self) -> ReleaseScenario { /* ... */ }
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
    fn eq(self: &Self, other: &ReleaseScenario) -> bool { /* ... */ }
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
#### Enum `DilutionSource`

Where the dilution factor comes from.

```rust
pub enum DilutionSource {
    Computed,
    UserSupplied(Vec<(f64, f64)>),
}
```

##### Variants

###### `Computed`

Compute it from the plume model (upstream `have_dilution_factor:
False`), with or without met data.

###### `UserSupplied`

Use the caller's maximum `chi/Q` per distance, s/m^3
(`have_dilution_factor: True`, `list_max_dilution_factor`), as
`(distance m, chi/Q)` pairs.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<(f64, f64)>` |  |

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
    fn clone(self: &Self) -> DilutionSource { /* ... */ }
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
    fn eq(self: &Self, other: &DilutionSource) -> bool { /* ... */ }
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
#### Struct `MetSettings`

The meteorological settings (upstream keys used when `have_met_data`).

```rust
pub struct MetSettings {
    pub calm_correction: bool,
    pub start_operation_time: i64,
    pub end_operation_time: i64,
    pub num_days: Vec<i64>,
    pub sampling_time: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `calm_correction` | `bool` | `calm_correction`. |
| `start_operation_time` | `i64` | `start_operation_time` (hour). |
| `end_operation_time` | `i64` | `end_operation_time` (hour). |
| `num_days` | `Vec<i64>` | `num_days`, per year of data. |
| `sampling_time` | `f64` | `sampling_time`, minutes (read; its correction is never applied). |

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
    fn clone(self: &Self) -> MetSettings { /* ... */ }
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
    fn eq(self: &Self, other: &MetSettings) -> bool { /* ... */ }
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
#### Struct `PyDoseiaConfig`

A pyDOSEIA run configuration.

```rust
pub struct PyDoseiaConfig {
    pub release: ReleaseScenario,
    pub dilution: DilutionSource,
    pub met: Option<MetSettings>,
    pub nuclides: Vec<String>,
    pub elements: Vec<String>,
    pub absorption_types: Vec<crate::pydoseia::dcf::LungAbsorptionType>,
    pub release_height_m: f64,
    pub measurement_height_m: f64,
    pub downwind_distances_m: Vec<f64>,
    pub plant_boundary_m: f64,
    pub age_group: Vec<f64>,
    pub centreline_ground_level: bool,
    pub receptor_y_m: f64,
    pub receptor_z_m: f64,
    pub mean_speed_scaling: Option<[f64; 6]>,
    pub weathering_corr: bool,
    pub exposure_period_y: f64,
    pub consider_progeny: bool,
    pub ignore_half_life_s: f64,
    pub run_dose_computation: bool,
    pub run_plume_shine_dose: bool,
    pub plume_shine_integrator: crate::pydoseia::plume_shine::PlumeShineIntegrator,
    pub ingestion_parameters: crate::pydoseia::ingestion::IngestionParameters,
    pub diet_adult: crate::pydoseia::ingestion::DietaryIntake,
    pub diet_infant: crate::pydoseia::ingestion::DietaryIntake,
    pub soil: crate::pydoseia::ingestion::SoilType,
    pub climate: crate::pydoseia::ingestion::food_chain::Climate,
    pub veg_type: crate::pydoseia::ingestion::food_chain::VegetationType,
    pub animal_feed_type: crate::pydoseia::ingestion::food_chain::VegetationType,
    pub animal_products_h3: Vec<crate::pydoseia::ingestion::food_chain::AnimalProduct>,
    pub animal_products_c14: Vec<crate::pydoseia::ingestion::food_chain::AnimalProduct>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `release` | `ReleaseScenario` | Release scenario. |
| `dilution` | `DilutionSource` | Dilution factor source. |
| `met` | `Option<MetSettings>` | `have_met_data` and its settings (`None` = no met data). |
| `nuclides` | `Vec<String>` | `rads_list`. |
| `elements` | `Vec<String>` | `element_list`. |
| `absorption_types` | `Vec<crate::pydoseia::dcf::LungAbsorptionType>` | `type_rad`, per nuclide. |
| `release_height_m` | `f64` | `release_height`, m. |
| `measurement_height_m` | `f64` | `measurement_height`, m. |
| `downwind_distances_m` | `Vec<f64>` | `downwind_distances`, m. |
| `plant_boundary_m` | `f64` | `plant_boundary`, m (appended to the distances if absent). |
| `age_group` | `Vec<f64>` | `age_group`, years. |
| `centreline_ground_level` | `bool` | `max_conc_plume_central_line_gl`. |
| `receptor_y_m` | `f64` | `Y`, m (single plume and plume shine receptor). |
| `receptor_z_m` | `f64` | `Z`, m. |
| `mean_speed_scaling` | `Option<[f64; 6]>` | `like_to_scale_with_mean_speed` with `ask_mean_speed_data` (m/s,<br>classes A-F). |
| `weathering_corr` | `bool` | `weathering_corr` (ground shine). |
| `exposure_period_y` | `f64` | `exposure_period`, years. |
| `consider_progeny` | `bool` | `consider_progeny`. |
| `ignore_half_life_s` | `f64` | `ignore_half_life`, s. |
| `run_dose_computation` | `bool` | `run_dose_computation`. |
| `run_plume_shine_dose` | `bool` | `run_plume_shine_dose`. |
| `plume_shine_integrator` | `crate::pydoseia::plume_shine::PlumeShineIntegrator` | Which quadrature the plume-shine integral runs on (not an upstream<br>setting). Defaults to [`PlumeShineIntegrator::Petir`]; set<br>[`PlumeShineIntegrator::ScipyQuadpackReference`] to reproduce<br>pyDOSEIA's numbers bit for bit. |
| `ingestion_parameters` | `crate::pydoseia::ingestion::IngestionParameters` | `inges_param_dict`. |
| `diet_adult` | `crate::pydoseia::ingestion::DietaryIntake` | `inges_param_dict_adult`. |
| `diet_infant` | `crate::pydoseia::ingestion::DietaryIntake` | `inges_param_dict_infant`. |
| `soil` | `crate::pydoseia::ingestion::SoilType` | `soiltype`. |
| `climate` | `crate::pydoseia::ingestion::food_chain::Climate` | `climate`. |
| `veg_type` | `crate::pydoseia::ingestion::food_chain::VegetationType` | `veg_type_list` (one type; see the ingestion docs). |
| `animal_feed_type` | `crate::pydoseia::ingestion::food_chain::VegetationType` | `animal_feed_type`. |
| `animal_products_h3` | `Vec<crate::pydoseia::ingestion::food_chain::AnimalProduct>` | `animal_product_list_for_tritium`. |
| `animal_products_c14` | `Vec<crate::pydoseia::ingestion::food_chain::AnimalProduct>` | `animal_product_list_for_C14`. |

##### Implementations

###### Methods

- ```rust
  pub fn input_generator_defaults(release: ReleaseScenario) -> Self { /* ... */ }
  ```
  A configuration with the input generator's defaults for everything

- ```rust
  pub fn distances_with_boundary(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  The distances upstream computes at: `downwind_distances` with the plant

- ```rust
  pub fn releases(self: &Self) -> &[f64] { /* ... */ }
  ```
  The release list of the active scenario.

- ```rust
  pub fn validate(self: &Self) -> Result<(), ConfigError> { /* ... */ }
  ```
  Upstream's `__init__` checks, plus the list-length consistency upstream

- ```rust
  pub const fn unused_upstream_keys() -> &'static [(&'static str, &'static str)] { /* ... */ }
  ```
  Keys upstream reads (or its input generator writes) that no

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PyDoseiaConfig { /* ... */ }
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
    fn eq(self: &Self, other: &PyDoseiaConfig) -> bool { /* ... */ }
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
#### Enum `ConfigError`

Why a configuration is rejected (upstream's `ValueError` messages, in
substance).

```rust
pub enum ConfigError {
    NoNuclides,
    LengthMismatch(&'static str),
    NoDistances,
    NoAges,
    NoReleaseHeight,
    MetSettings,
    TritiumSettings,
}
```

##### Variants

###### `NoNuclides`

No nuclides while dose computation is requested.

###### `LengthMismatch`

`element_list` (or `type_rad`, or the release list) does not match
`rads_list` in length.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `NoDistances`

No distances.

###### `NoAges`

No ages.

###### `NoReleaseHeight`

Release height missing or zero (upstream tests `not release_height`).

###### `MetSettings`

Met data requested without day counts or with a zero measurement height.

###### `TritiumSettings`

`H-3` requested without the tritium food-chain settings (upstream
requires the animal product list).

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
    fn clone(self: &Self) -> ConfigError { /* ... */ }
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
    fn eq(self: &Self, other: &ConfigError) -> bool { /* ... */ }
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

**Note (2026-09-29):** the FGR-15 edition named above is upstream's, the
2019 EPA-402/R-19/002, which EPA has since **withdrawn** ("contained errors
in the dose coefficient tables"). The coefficients buangkok does ship, in
[`crate::coefficients`], are from the **July 2025 revision, EPA
402-R-25-001**, for five nuclides only.

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

## Module `dcf_screening`

Multi-source dose-coefficient screening: pyDOSEIA's `compute_max_dcf`,
which looks a nuclide up in five inhalation and two ingestion coefficient
compilations and reports either the coefficient of the requested lung
absorption type or the largest one found.

# Provenance

Ported from pyDOSEIA `raddcffunc.py` (`get_dcfs_for_radionuclides`,
`compute_max_dcf` — the second definition, the one Python binds —
`merge_dataframes_with_source_hc2` and the seven `screen_*` readers),
upstream <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).

# What upstream uses it for (defect D7)

Only the **report**: `outputfunc.agewise_dcfs_inh_gs_submersion` prints
these coefficients, while the inhalation dose itself uses
[`super::dcf::InhalationDcfTable::lookup`] on a different table. The two
need not agree.

# No coefficient data ships with this crate

| Upstream file | Source | Here |
|---|---|---|
| `inhalation_HC2/Annex_G_ICRP119_dcf_inh_public.xlsx` | ICRP Publication 119, Annex G (copyright ICRP) | not copied |
| `inhalation_HC2/Annex_H_ICRP119_...csv` | ICRP 119 Annex H | not copied; **never read** upstream (defect D18) |
| `inhalation_HC2/Table_A2-DOE-STD-1196-2011_dcf_inhal.csv` | US DOE-STD-1196-2011, Table A-2 (a US government standard) | not copied in this pass |
| `inhalation_HC2/Table_5_JAERI_...csv`, `Table_7_JAERI_...csv`, `ingestion_public/table_4_jaeri_ingestion_public.csv` | JAERI-Data/Code 2002-013 (JAEA; terms not established) | not copied |
| `ingestion_public/AnnexF_ICRP119_dcf_ingestion_public.csv` | ICRP 119 Annex F | not copied |

Tables are read with [`ScreeningTable::from_csv`] from CSVs carrying
upstream's **renamed** column headers (upstream reassigns the headers by
position after reading).

```rust
pub mod dcf_screening { /* ... */ }
```

### Types

#### Enum `ScreeningSource`

The seven compilations `compute_max_dcf` reads, in the order it merges
them.

```rust
pub enum ScreeningSource {
    Table7Jaeri,
    Table5Jaeri,
    TableA2Doe,
    AnnexGIcrp119,
    AnnexFIcrp119,
    Table4Jaeri,
}
```

##### Variants

###### `Table7Jaeri`

JAERI-Data/Code 2002-013 Table 7 (soluble/reactive gases), inhalation.

###### `Table5Jaeri`

JAERI-Data/Code 2002-013 Table 5 (particulates), inhalation.

###### `TableA2Doe`

DOE-STD-1196-2011 Table A-2, inhalation.

###### `AnnexGIcrp119`

ICRP 119 Annex G, inhalation.

###### `AnnexFIcrp119`

ICRP 119 Annex F, ingestion.

###### `Table4Jaeri`

JAERI-Data/Code 2002-013 Table 4, ingestion.

##### Implementations

###### Methods

- ```rust
  pub const fn upstream_file_name(self: Self) -> &'static str { /* ... */ }
  ```
  The file name upstream opens (`get_corrected_nuclide` matches words of

- ```rust
  pub const fn age_columns(self: Self) -> [&'static str; 6] { /* ... */ }
  ```
  The six age columns after upstream's renaming. DOE's three middle ones

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ScreeningSource { /* ... */ }
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
    fn eq(self: &Self, other: &ScreeningSource) -> bool { /* ... */ }
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
#### Struct `ScreeningRow`

One row of a screening table.

```rust
pub struct ScreeningRow {
    pub nuclide: String,
    pub absorption_type: Option<String>,
    pub coefficients: [f64; 6],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `String` | Nuclide field as written. |
| `absorption_type` | `Option<String>` | Absorption type field (`None` for the ingestion tables or a blank). |
| `coefficients` | `[f64; 6]` | Coefficients, Sv/Bq, in the source's [`ScreeningSource::age_columns`] order. |

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
    fn clone(self: &Self) -> ScreeningRow { /* ... */ }
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
    fn eq(self: &Self, other: &ScreeningRow) -> bool { /* ... */ }
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
#### Struct `ScreeningTable`

One screening table.

```rust
pub struct ScreeningTable {
    pub source: ScreeningSource,
    pub rows: Vec<ScreeningRow>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source` | `ScreeningSource` | Which compilation. |
| `rows` | `Vec<ScreeningRow>` | Rows in file order. |

##### Implementations

###### Methods

- ```rust
  pub fn from_csv(source: ScreeningSource, text: &str) -> Result<Self, String> { /* ... */ }
  ```
  Read a CSV with upstream's renamed headers: `Nuclide`, `Type` (the

- ```rust
  pub fn screen(self: &Self, name: &str) -> Vec<&ScreeningRow> { /* ... */ }
  ```
  Upstream's `screen_*` filter: rows whose trimmed, upper-cased nuclide

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ScreeningTable { /* ... */ }
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
    fn eq(self: &Self, other: &ScreeningTable) -> bool { /* ... */ }
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
#### Struct `AlternateNames`

Upstream's alternate nuclide names (from its nomenclature file), in its
key order.

```rust
pub struct AlternateNames {
    pub doe_std_1196: Option<String>,
    pub fgr_12: Option<String>,
    pub icrp119_107: Option<String>,
    pub icrp_38: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `doe_std_1196` | `Option<String>` | `DOE_STD_1196_name`. |
| `fgr_12` | `Option<String>` | `FGR_12_name`. |
| `icrp119_107` | `Option<String>` | `ICRP119_107_name`. |
| `icrp_38` | `Option<String>` | `ICRP_38_name`. |

##### Implementations

###### Methods

- ```rust
  pub fn corrected(self: &Self, file_name: &str, nuclide: &str) -> String { /* ... */ }
  ```
  `get_corrected_nuclide`: the first alternate name whose key shares a

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AlternateNames { /* ... */ }
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
    fn default() -> AlternateNames { /* ... */ }
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
    fn eq(self: &Self, other: &AlternateNames) -> bool { /* ... */ }
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
#### Struct `ScreeningTables`

The screening tables available (any may be empty or absent).

```rust
pub struct ScreeningTables {
    pub inhalation: Vec<ScreeningTable>,
    pub ingestion: Vec<ScreeningTable>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inhalation` | `Vec<ScreeningTable>` | Inhalation tables, in any order (merged in upstream's order). |
| `ingestion` | `Vec<ScreeningTable>` | Ingestion tables. |

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
    fn clone(self: &Self) -> ScreeningTables { /* ... */ }
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
    fn default() -> ScreeningTables { /* ... */ }
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
    fn eq(self: &Self, other: &ScreeningTables) -> bool { /* ... */ }
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
#### Struct `ScreenedDcf`

`compute_max_dcf`'s result: `max_dcf_inh_public` and `max_dcf_ing_public`
(`None` where upstream stores `None`).

```rust
pub struct ScreenedDcf {
    pub inhalation: Option<f64>,
    pub ingestion: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inhalation` | `Option<f64>` | Inhalation, Sv/Bq. |
| `ingestion` | `Option<f64>` | Ingestion, Sv/Bq. |

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
    fn clone(self: &Self) -> ScreenedDcf { /* ... */ }
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
    fn eq(self: &Self, other: &ScreenedDcf) -> bool { /* ... */ }
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

#### Function `screening_bracket`

**Attributes:**

- `MustUse { reason: None }`

Upstream's age-to-column bracket for this function.

```rust
pub fn screening_bracket(age: f64) -> usize { /* ... */ }
```

#### Function `compute_max_dcf`

**Attributes:**

- `MustUse { reason: None }`

`compute_max_dcf(radionuclide, user_type, age)`. `None` when neither
merged table has the age column (upstream returns `None`).

For `user_type == "Max"` both results are the largest coefficient among
the screened rows (of any nuclide the delimited match selected, and any
type). Otherwise the first row, in merge order, whose nuclide field
**equals** the name (not the alternate name used to screen) and whose
type equals `user_type` (ingestion: name only), falling back to the
maximum. The Annex H reader always fails upstream (D18) and contributes
nothing, so it has no input here.

```rust
pub fn compute_max_dcf(tables: &ScreeningTables, radionuclide: &str, user_type: &str, alternate: &AlternateNames, age: f64) -> Option<ScreenedDcf> { /* ... */ }
```

### Constants and Statics

#### Constant `MERGED_AGE_COLUMNS`

The age columns `compute_max_dcf` selects from, by upstream's brackets
(`<= 1`, `<= 2`, `<= 7`, `<= 12`, `<= 17`, otherwise adult).

```rust
pub const MERGED_AGE_COLUMNS: [&str; 6] = _;
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

#### Function `dilution_single_plume_with_met_speeds`

**Attributes:**

- `MustUse { reason: None }`

**Divergence from upstream (D3 corrected):** the single-plume dilution
factor per class, divided by the class's mean wind speed from the met
record, converted from km/h to m/s.

Upstream's `dilution_per_sector` for a single plume **with** met data
divides the six per-class values by `mean_speeds[:, None]`, which
broadcasts to a 6 x 6 array and fails its own `shape == (6,)` assertion,
and the means it divides by are in km/h (the met column) while the
dilution factor is per 1 m/s. This does what the code evidently intends:
class `i` divided by `mean_i / 3.6` m/s, with the means of
[`super::met::speed_distribution`] (quantile 0.90) over all years. Not
verifiable against upstream, which cannot run this path.

```rust
pub fn dilution_single_plume_with_met_speeds(x: uom::si::f64::Length, geometry: PlumeGeometry, met: &super::met::MetClimatology) -> [super::units::DilutionFactor; 6] { /* ... */ }
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

The three pyDOSEIA dose pathways ported in the first tranche: **inhalation**,
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

#### Function `inhalation_committed_dose_rate_msv_per_s`

**Attributes:**

- `MustUse { reason: None }`

The inhalation pathway's coefficient product,
`C * DCF_inh * breathing rate * 1000`, which [`inhalation_dose`] is built on.

- Given an **instantaneous** air concentration `C` \[Bq/m^3\], the result is
  the **committed** effective dose per second of breathing \[mSv/s\]: the
  dose committed by one second's intake, not a dose received in that
  second. It is what an "inhalation dose rate" means on a map, and it must
  be labelled that way.
- Given a **time-integrated** concentration \[Bq s/m^3\] it is the
  committed dose \[mSv\], which is how [`inhalation_dose`] uses it.

Returns `None` only for a NaN age. Not an upstream function: a 2026-09-29
refactor so a dose-rate caller (`htgr_sim_v1`'s map) and the ported dose
share one formula. Research-grade only (`RESPONSIBLE_USE.md`).

```rust
pub fn inhalation_committed_dose_rate_msv_per_s(air_concentration_bq_per_m3: f64, dcf_sv_per_bq: f64, age_years: f64) -> Option<f64> { /* ... */ }
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

#### Function `ground_shine_dose_rate_msv_per_s`

**Attributes:**

- `MustUse { reason: None }`

The ground-shine coefficient product `A * DCF_gs * 1000`: the effective
dose **rate** \[mSv/s\] from a ground-surface concentration `A`
\[Bq/m^2\] and a ground-surface dose-rate coefficient
\[Sv m^2 Bq^-1 s^-1\] (FGR-15 Table 4-1 is one).

[`ground_shine_dose`] is built on it (its `gs_dose` step). Not an upstream
function: a 2026-09-29 refactor so a dose-rate caller and the ported dose
share one formula. Research-grade only (`RESPONSIBLE_USE.md`).

```rust
pub fn ground_shine_dose_rate_msv_per_s(ground_bq_per_m2: f64, dcf_gs: f64) -> f64 { /* ... */ }
```

#### Function `submersion_dose`

**Attributes:**

- `MustUse { reason: None }`

Submersion dose for one nuclide: `chi/Q * Q * DCF_sub * 1000` (mSv, or
mSv/y). `dcf_sub` (Sv m^3 Bq^-1 s^-1) from [`super::dcf::external_dcf`].

```rust
pub fn submersion_dose(chi_over_q: super::units::DilutionFactor, release: Release, dcf_sub: f64) -> super::units::EffectiveDose { /* ... */ }
```

#### Function `submersion_dose_rate_msv_per_s`

**Attributes:**

- `MustUse { reason: None }`

The submersion coefficient product `C * DCF_sub * 1000`: given an
**instantaneous** air concentration `C` \[Bq/m^3\] and an air-submersion
dose-rate coefficient \[Sv m^3 Bq^-1 s^-1\] (FGR-15 Table 4-6 is one), the
effective dose **rate** \[mSv/s\]; given a time-integrated concentration
\[Bq s/m^3\], the dose \[mSv\], which is how [`submersion_dose`] uses it.

**Semi-infinite cloud.** The coefficient assumes the receptor stands in a
uniform cloud of concentration `C` extending far beyond a photon mean free
path (~100 m in air at 1 MeV). For a narrow plume that over-states the dose
on the centreline; beneath an elevated plume that has not yet reached the
ground it under-states it. The finite-cloud alternative is
[`super::plume_shine`].

Not an upstream function: a 2026-09-29 refactor so a dose-rate caller and
the ported dose share one formula. Research-grade only.

```rust
pub fn submersion_dose_rate_msv_per_s(air_concentration_bq_per_m3: f64, dcf_sub: f64) -> f64 { /* ... */ }
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

## Module `ingestion`

Ingestion: pyDOSEIA's terrestrial food-chain model (IAEA SRS 19 screening
equations for leafy vegetables / food crops, pasture, stored feed, milk and
meat) and its specific-activity models for H-3 and C-14 (IAEA TECDOC-1616).

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Never a
> dose to a real person.

# Provenance

Ported from pyDOSEIA `dosefunc.py` (`DoseFunc.ingestion_dose` with its
nested `conc_tritium_in_terrestrial_plant`,
`conc_tritium_in_terrestrial_animal`, `conc_c14_in_terrestrial_plants`,
`conc_c14_in_terrestrial_animal`, `conc_c14_in_fish`; `zeroing_ingestion`)
and `raddcffunc.py` (`dcf_list_ingestion`, `fv_list_ecerman_ingestion`,
`ingestion_weathering_correction_real`, `ingestion_weathering_correction`,
`effective_surface_soil_density_rho`), upstream
<https://github.com/BiswajitSadhu/pyDOSEIA> at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). The model
equations are those of IAEA Safety Reports Series No. 19 (2001) section 5
and IAEA-TECDOC-1616 (2009), which upstream cites; the handful of scalar
constants upstream hard-codes (soil densities, humidities, water contents,
stable-carbon contents, concentration ratios) are reproduced as upstream's
code literals with its citations.

# No coefficient table ships with this crate

| Upstream sheet | Content | Source | Here |
|---|---|---|---|
| `Dose_ecerman_final.xlsx` / `eco_param` | element transfer factors `Fv1`, `Fv2`, `Fm`, `Ff`, soil and plant loss rates | IAEA SRS 19 Tables VII, X, XI (IAEA copyright) | **not copied**; read your own with [`EcoParamTable::from_csv`] |
| `Dose_ecerman_final.xlsx` / `ingestion_gsr3` | ingestion e(g), six ages; `HTO`, `OBT` rows for tritium | IAEA GSR Part 3 Schedule III / ICRP 72 (copyright) | **not copied**; read your own with [`IngestionDcfTable::from_csv`] |

The code-to-code test uses **synthetic** tables in these layouts.

# Two drivers: upstream's, and a corrected one

[`upstream::ingestion_dose_upstream`] reproduces `ingestion_dose` exactly,
including its list-indexing defect (D8: the per-element lists skip H and C
but are indexed by position in the full nuclide list, so any H or C
nuclide that is not at the end shifts or breaks every later nuclide), its
output layout (non-H/C rows, then H-3, then C-14; transposed when all three
kinds are present, and no result at all for exactly `[H-3, C-14]`: D9), the
C-14 air concentration left per year (D10), and the C-14 milk/meat test on
the whole product list (D11). [`corrected::ingestion_dose_per_nuclide`] is
a labelled **divergence** that computes each nuclide on its own and fixes
D8-D11; the default everywhere else stays upstream's.

```rust
pub mod ingestion { /* ... */ }
```

### Modules

## Module `corrected`

**Divergence from upstream**: ingestion dose computed one nuclide at a
time, fixing pyDOSEIA defects D8-D11 (see `docs/pydoseia-code-to-code.md`).
The equations are upstream's ([`super::food_chain`]); only the bookkeeping
and the two clear bugs change:

- **D8** each nuclide uses its own transfer factors and its own
  concentrations, whatever its position in the list;
- **D9** every nuclide gets a row, in input order, for any mix of H-3,
  C-14 and other nuclides;
- **D10** for a long-term release the C-14 air concentration is divided by
  the seconds in a year (`365 * 24 * 3600`), as upstream already does for
  H-3, so that `C_air` is Bq/m^3 and not Bq s/(y m^3);
- **D11** C-14 milk and meat are added according to **each** product,
  not according to whether the product *list* contains any milk or meat;
  and a tritium product list starting with meat no longer raises.

Everything else (constants, units, the per-day diet times 365, the H-3
division by a year for a single plume) is upstream's. Not checked against
SRS 19 or TECDOC-1616 beyond what the code comments cite.

```rust
pub mod corrected { /* ... */ }
```

### Types

#### Enum `IngestionModel`

Which model a nuclide gets. The caller decides; upstream decides by the
exact names `"H-3"` and `"C-14"` in one place and by the element symbols
`H` and `C` in another (part of D8).

```rust
pub enum IngestionModel {
    Deposition {
        deposition_velocity_m_per_s: f64,
        transfer: super::TransferFactors,
    },
    Tritium,
    Carbon14,
}
```

##### Variants

###### `Deposition`

The SRS 19 deposition model with the element's transfer factors
([`TransferFactors::ZERO`] if it has none, as upstream).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `deposition_velocity_m_per_s` | `f64` | Total deposition velocity, m/s. |
| `transfer` | `super::TransferFactors` | Element transfer factors. |

###### `Tritium`

The TECDOC-1616 specific-activity model for tritium.

###### `Carbon14`

The specific-activity model for C-14.

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
    fn clone(self: &Self) -> IngestionModel { /* ... */ }
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
    fn eq(self: &Self, other: &IngestionModel) -> bool { /* ... */ }
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
#### Struct `IngestionNuclide`

One nuclide's inputs.

```rust
pub struct IngestionNuclide {
    pub model: IngestionModel,
    pub decay_constant_per_s: f64,
    pub release_bq: f64,
    pub dcf: super::IngestionDcf,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `model` | `IngestionModel` | Model. |
| `decay_constant_per_s` | `f64` | Decay constant, 1/s. |
| `release_bq` | `f64` | Release: Bq/y (long term) or Bq (single plume). |
| `dcf` | `super::IngestionDcf` | Ingestion coefficient ([`IngestionDcf::Tritium`] for H-3). |

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
    fn clone(self: &Self) -> IngestionNuclide { /* ... */ }
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
    fn eq(self: &Self, other: &IngestionNuclide) -> bool { /* ... */ }
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
#### Struct `IngestionSettings`

Settings shared by all nuclides.

```rust
pub struct IngestionSettings {
    pub mode: super::upstream::IngestionReleaseMode,
    pub parameters: super::IngestionParameters,
    pub diet: super::DietaryIntake,
    pub soil: super::SoilType,
    pub climate: super::food_chain::Climate,
    pub veg_type: super::food_chain::VegetationType,
    pub animal_feed_type: super::food_chain::VegetationType,
    pub animal_products_h3: Vec<super::food_chain::AnimalProduct>,
    pub animal_products_c14: Vec<super::food_chain::AnimalProduct>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mode` | `super::upstream::IngestionReleaseMode` | Release mode. |
| `parameters` | `super::IngestionParameters` | Food-chain parameters. |
| `diet` | `super::DietaryIntake` | Receiver's diet (per day). |
| `soil` | `super::SoilType` | Soil type. |
| `climate` | `super::food_chain::Climate` | Climate (H-3). |
| `veg_type` | `super::food_chain::VegetationType` | Vegetables eaten (H-3 and C-14). |
| `animal_feed_type` | `super::food_chain::VegetationType` | Animal feed (H-3 and C-14). |
| `animal_products_h3` | `Vec<super::food_chain::AnimalProduct>` | Animal products for H-3. |
| `animal_products_c14` | `Vec<super::food_chain::AnimalProduct>` | Animal products for C-14. |

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
    fn clone(self: &Self) -> IngestionSettings { /* ... */ }
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
    fn eq(self: &Self, other: &IngestionSettings) -> bool { /* ... */ }
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
#### Struct `IngestionDoseByRoute`

Ingestion dose by route.

```rust
pub struct IngestionDoseByRoute {
    pub veg: crate::pydoseia::units::EffectiveDose,
    pub milk: crate::pydoseia::units::EffectiveDose,
    pub meat: crate::pydoseia::units::EffectiveDose,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `veg` | `crate::pydoseia::units::EffectiveDose` | Vegetables / food crops. |
| `milk` | `crate::pydoseia::units::EffectiveDose` | Milk. |
| `meat` | `crate::pydoseia::units::EffectiveDose` | Meat. |

##### Implementations

###### Methods

- ```rust
  pub fn total(self: Self) -> EffectiveDose { /* ... */ }
  ```
  Sum of the three routes.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> IngestionDoseByRoute { /* ... */ }
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
    fn default() -> IngestionDoseByRoute { /* ... */ }
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
    fn eq(self: &Self, other: &IngestionDoseByRoute) -> bool { /* ... */ }
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

#### Function `ingestion_dose_per_nuclide`

**Attributes:**

- `MustUse { reason: None }`

Ingestion dose for each nuclide, in input order (the corrected driver; see
the module docs for what differs from upstream).

```rust
pub fn ingestion_dose_per_nuclide(nuclides: &[IngestionNuclide], chi_over_q: crate::pydoseia::units::DilutionFactor, s: &IngestionSettings) -> Vec<IngestionDoseByRoute> { /* ... */ }
```

## Module `food_chain`

The food-chain equations of pyDOSEIA's `ingestion_dose`, one nuclide at a
time, written in upstream's operation order. Ported from `dosefunc.py` and
`raddcffunc.py` at commit `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`,
Copyright (c) 2024 Dr. Biswajit Sadhu, MIT (see `crates/buangkok/NOTICE`).
The equations are IAEA SRS 19 (2001) eqs. for direct deposition, soil
uptake, pasture, stored feed, milk and meat, and IAEA-TECDOC-1616 (2009)
for the H-3 and C-14 specific-activity models, as upstream cites them.

```rust
pub mod food_chain { /* ... */ }
```

### Types

#### Struct `CropConcentrations`

Food-crop concentrations (SRS 19 section 5.1), Bq/kg.

```rust
pub struct CropConcentrations {
    pub c_vi1: f64,
    pub c_si: f64,
    pub c_vi2: f64,
    pub cvi: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `c_vi1` | `f64` | Direct deposition on the crop, `C_vi1`. |
| `c_si` | `f64` | Soil concentration (crop root zone), `C_si`, Bq/kg dry soil. |
| `c_vi2` | `f64` | Root uptake, `C_vi2 = Fv2 C_si`. |
| `cvi` | `f64` | At consumption, `(C_vi1 + C_vi2) exp(-lambda_i t_h)`. |

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
    fn clone(self: &Self) -> CropConcentrations { /* ... */ }
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
    fn eq(self: &Self, other: &CropConcentrations) -> bool { /* ... */ }
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
#### Struct `AnimalConcentrations`

Pasture, feed, milk and meat (SRS 19 section 5.2).

```rust
pub struct AnimalConcentrations {
    pub c_vi1: f64,
    pub c_si: f64,
    pub c_vi2: f64,
    pub cvi_animal: f64,
    pub cpi: f64,
    pub c_ai: f64,
    pub c_mi: f64,
    pub c_fi: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `c_vi1` | `f64` | Direct deposition on forage, Bq/kg dry. |
| `c_si` | `f64` | Pasture soil, Bq/kg dry soil. |
| `c_vi2` | `f64` | Root uptake by pasture, `Fv1 C_si`. |
| `cvi_animal` | `f64` | Fresh pasture at grazing, `C_pasture`. |
| `cpi` | `f64` | Stored feed, `C_pi`. |
| `c_ai` | `f64` | Average feed, `f_p C_pasture + (1 - f_p) C_pi`. |
| `c_mi` | `f64` | Milk, Bq/L. |
| `c_fi` | `f64` | Meat, Bq/kg. |

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
    fn clone(self: &Self) -> AnimalConcentrations { /* ... */ }
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
    fn eq(self: &Self, other: &AnimalConcentrations) -> bool { /* ... */ }
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
#### Enum `Climate`

Climate for the tritium model (upstream `climate_humidity`): latitude,
absolute humidity `H_a` (kg/m^3) and relative humidity.

```rust
pub enum Climate {
    Mediterranean,
    Continental,
    Maritime,
    Arctic,
}
```

##### Variants

###### `Mediterranean`

`'Mediterranean'`: 34, 0.0115, 0.6.

###### `Continental`

`'Continental'`: 48, 0.0087, 0.71.

###### `Maritime`

`'Maritime'`: 50, 0.0078, 0.795. (The input generator offers
`'meritime'`, which the pathway's dictionary does not contain.)

###### `Arctic`

`'Arctic'`: 50, 0.0067, 0.73. (The input generator offers `'arctic'`.)

##### Implementations

###### Methods

- ```rust
  pub const fn humidity(self: Self) -> (f64, f64, f64) { /* ... */ }
  ```
  `(latitude, H_a, RH)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Climate { /* ... */ }
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
    fn eq(self: &Self, other: &Climate) -> bool { /* ... */ }
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
#### Enum `VegetationType`

Vegetation (or animal feed) type for the H-3 and C-14 models.

```rust
pub enum VegetationType {
    LeafyVegetables,
    NonLeafyVegetables,
    RootCrops,
    AllOthers,
}
```

##### Variants

###### `LeafyVegetables`

`'leafy_vegetables'`.

###### `NonLeafyVegetables`

`'non_leafy_vegetables'`.

###### `RootCrops`

`'root_crops'`.

###### `AllOthers`

`'all_others'`.

##### Implementations

###### Methods

- ```rust
  pub const fn weq_wcp(self: Self) -> (f64, f64) { /* ... */ }
  ```
  `(WEQ, WCp)`: water equivalent factor (L/kg dry) and water content

- ```rust
  pub const fn stable_carbon(self: Self) -> f64 { /* ... */ }
  ```
  Stable carbon content `S_p`, gC/kg fresh weight (upstream's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> VegetationType { /* ... */ }
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
    fn eq(self: &Self, other: &VegetationType) -> bool { /* ... */ }
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
#### Enum `AnimalProduct`

Animal products of the H-3 and C-14 models.

```rust
pub enum AnimalProduct {
    CowMilk,
    GoatMilk,
    GoatMeat,
    LambMeat,
    BeefMeat,
    PorkMeat,
    BroilerMeat,
    Egg,
}
```

##### Variants

###### `CowMilk`

`'cow_milk'`.

###### `GoatMilk`

`'goat_milk'`.

###### `GoatMeat`

`'goat_meat'`.

###### `LambMeat`

`'lamb_meat'`.

###### `BeefMeat`

`'beef_meat'`.

###### `PorkMeat`

`'pork_meat'`.

###### `BroilerMeat`

`'broiler_meat'`.

###### `Egg`

`'egg'` (has ratios but no dose route upstream).

##### Implementations

###### Methods

- ```rust
  pub const fn cr_hto(self: Self) -> f64 { /* ... */ }
  ```
  `CR_a_HTO`, the HTO concentration ratio.

- ```rust
  pub const fn cr_obt(self: Self) -> f64 { /* ... */ }
  ```
  `CR_a_OBT`, the OBT concentration ratio.

- ```rust
  pub const fn stable_carbon(self: Self) -> f64 { /* ... */ }
  ```
  `S_a`, stable carbon in the product, gC/kg (TECDOC-1616 Table 12).

- ```rust
  pub const fn is_milk(self: Self) -> bool { /* ... */ }
  ```
  In upstream's milk list (`cow_milk`, `goat_milk`).

- ```rust
  pub const fn is_meat(self: Self) -> bool { /* ... */ }
  ```
  In upstream's meat list (`goat_meat`, `lamb_meat`, `beef_meat`,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AnimalProduct { /* ... */ }
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
    fn eq(self: &Self, other: &AnimalProduct) -> bool { /* ... */ }
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
#### Struct `TritiumPlant`

Output of [`tritium_in_plant`].

```rust
pub struct TritiumPlant {
    pub weq: f64,
    pub wcp: f64,
    pub c_tfwt: f64,
    pub c_pfw_hto: f64,
    pub c_pfw_obt: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `weq` | `f64` | Water equivalent factor. |
| `wcp` | `f64` | Water content. |
| `c_tfwt` | `f64` | Tissue-free water tritium. |
| `c_pfw_hto` | `f64` | HTO in the fresh plant. |
| `c_pfw_obt` | `f64` | OBT in the fresh plant. |

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
    fn clone(self: &Self) -> TritiumPlant { /* ... */ }
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
    fn eq(self: &Self, other: &TritiumPlant) -> bool { /* ... */ }
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

#### Function `deposition_rate_per_day`

**Attributes:**

- `MustUse { reason: None }`

Upstream's per-day deposition rate `d * v_d * chi/Q`, Bq m^-2 d^-1 (with
`d` the release per day: `Q / 365` for a long-term release, and the
released Bq itself for a single plume, as upstream).

```rust
pub fn deposition_rate_per_day(day_discharge: f64, v_d_m_per_s: f64, chi_over_q: f64) -> f64 { /* ... */ }
```

#### Function `effective_removal_rates`

**Attributes:**

- `MustUse { reason: None }`

`ingestion_weathering_correction_real` for one element: effective removal
rates from plants and from soil, 1/d, `(lambda_w + lambda_i,
lambda_s + lambda_i)` with `lambda_i` the decay constant per day.

```rust
pub fn effective_removal_rates(tf: super::TransferFactors, lambda_i_per_d: f64) -> (f64, f64) { /* ... */ }
```

#### Function `per_day`

**Attributes:**

- `MustUse { reason: None }`

Decay constant per day as upstream converts it: `lambda * 24 * 3600`.

```rust
pub fn per_day(lambda_per_s: f64) -> f64 { /* ... */ }
```

#### Function `food_crop`

**Attributes:**

- `MustUse { reason: None }`

Food crops for human consumption (upstream's "veg route").

```rust
pub fn food_crop(dep_per_day: f64, lambda_eiv: f64, lambda_eis: f64, fv2: f64, lambda_i_per_d: f64, rho_crop: f64, p: &super::IngestionParameters) -> CropConcentrations { /* ... */ }
```

#### Function `animal_products`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Upstream's milk and meat route for one nuclide.

```rust
pub fn animal_products(dep_per_day: f64, lambda_eiv: f64, lambda_eis: f64, fv1: f64, fm: f64, ff: f64, lambda_i_per_d: f64, rho_pasture: f64, p: &super::IngestionParameters) -> AnimalConcentrations { /* ... */ }
```

#### Function `tritium_in_plant`

**Attributes:**

- `MustUse { reason: None }`

`conc_tritium_in_terrestrial_plant`: air HTO from the release (Bq/y times
`chi/Q`, divided by `365 * 24 * 3600`), then air moisture, soil water,
tissue-free water, HTO and OBT in the plant.

Note (upstream behaviour, kept): the division by a year is applied for a
single-plume release as well, where the release is in Bq, not Bq/y.

```rust
pub fn tritium_in_plant(chi_over_q: f64, discharge: f64, climate: Climate, veg: VegetationType, cr_s: f64, gamma: f64, r_p: f64) -> TritiumPlant { /* ... */ }
```

#### Function `tritium_in_animal`

**Attributes:**

- `MustUse { reason: None }`

`conc_tritium_in_terrestrial_animal`: `(C_afw_T_HTO, C_f_OBT,
C_afw_T_OBT)`. Upstream calls it with `C_f_HTO = 0` (the HTO in drinking
water is not modelled), so the HTO term is always zero there.

```rust
pub fn tritium_in_animal(c_tfwt: f64, c_f_hto: f64, product: AnimalProduct, feed: VegetationType, r_p: f64) -> (f64, f64, f64) { /* ... */ }
```

#### Function `c14_in_plant`

**Attributes:**

- `MustUse { reason: None }`

`conc_c14_in_terrestrial_plants`: `C_air S_p / S_air`, Bq/kg fresh.

```rust
pub fn c14_in_plant(c_air: f64, veg: VegetationType, s_air: f64) -> f64 { /* ... */ }
```

#### Function `c14_in_animal`

**Attributes:**

- `MustUse { reason: None }`

`conc_c14_in_terrestrial_animal`: `f_c C_pfw S_a / S_p`, Bq/kg fresh
(upstream uses `f_c = 1`).

```rust
pub fn c14_in_animal(c_pfw: f64, feed: VegetationType, product: AnimalProduct, f_c: f64) -> f64 { /* ... */ }
```

#### Function `c14_in_fish`

**Attributes:**

- `MustUse { reason: None }`

C-14 in fish, `C_DIC * S_f` (default `S_f = 120` gC/kg).

**Divergence from upstream:** upstream's `conc_c14_in_fish(C_air, S_f)`
ignores `C_air` and reads an undefined `C_DIC` (a `NameError` if called;
it is never called, and marked TODO). This takes the dissolved inorganic
C-14 concentration explicitly, which is what the formula needs.

```rust
pub fn c14_in_fish(c_dic: f64, s_f: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `TRITIUM_CR_S`

Upstream's defaults for the tritium plant model: `CR_s = 0.23`,
`gamma = 0.909`, `R_p = 0.54`.

```rust
pub const TRITIUM_CR_S: f64 = 0.23;
```

#### Constant `TRITIUM_GAMMA`

Vapour-pressure ratio HTO/H2O.

```rust
pub const TRITIUM_GAMMA: f64 = 0.909;
```

#### Constant `TRITIUM_R_P`

OBT/TFWT concentration ratio.

```rust
pub const TRITIUM_R_P: f64 = 0.54;
```

#### Constant `C14_S_AIR`

Stable carbon in air, gC/m^3 (upstream `S_air = 0.20`).

```rust
pub const C14_S_AIR: f64 = 0.20;
```

## Module `upstream`

`DoseFunc.ingestion_dose` exactly as upstream runs it, list indexing and
output layout included. Ported from pyDOSEIA `dosefunc.py` at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`, Copyright (c) 2024
Dr. Biswajit Sadhu, MIT (see `crates/buangkok/NOTICE`).

Use [`super::corrected::ingestion_dose_per_nuclide`] for new work; this
function exists so that the port can be compared with upstream number for
number, and so that upstream's defects D8-D11 are demonstrable.

```rust
pub mod upstream { /* ... */ }
```

### Types

#### Enum `IngestionReleaseMode`

Release mode, which sets the per-day discharge and the diet multiplier.

```rust
pub enum IngestionReleaseMode {
    LongTerm,
    SinglePlume,
}
```

##### Variants

###### `LongTerm`

`long_term_release`: releases are Bq/y; the per-day discharge is
`Q / 365` and the diet is multiplied by 365.

###### `SinglePlume`

`single_plume`: releases are Bq; the discharge is used as is and the
diet is multiplied by 1 (upstream's `consumption_time_frac = 1`; the
config's `consumption_time_food` is never read).

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
    fn clone(self: &Self) -> IngestionReleaseMode { /* ... */ }
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
    fn eq(self: &Self, other: &IngestionReleaseMode) -> bool { /* ... */ }
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
#### Struct `UpstreamIngestionInputs`

Everything `ingestion_dose` reads, per call.

```rust
pub struct UpstreamIngestionInputs {
    pub nuclides: Vec<String>,
    pub elements: Vec<String>,
    pub decay_constants_per_s: Vec<f64>,
    pub releases_bq: Vec<f64>,
    pub mode: IngestionReleaseMode,
    pub chi_over_q: f64,
    pub parameters: super::IngestionParameters,
    pub diet_adult: super::DietaryIntake,
    pub diet_infant: super::DietaryIntake,
    pub soil: super::SoilType,
    pub climate: super::food_chain::Climate,
    pub veg_type: super::food_chain::VegetationType,
    pub animal_feed_type: super::food_chain::VegetationType,
    pub animal_products_h3: Vec<super::food_chain::AnimalProduct>,
    pub animal_products_c14: Vec<super::food_chain::AnimalProduct>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclides` | `Vec<String>` | `rads_list`. |
| `elements` | `Vec<String>` | `element_list` (same length). |
| `decay_constants_per_s` | `Vec<f64>` | Decay constants, 1/s (`lambda_of_rads`), per nuclide. |
| `releases_bq` | `Vec<f64>` | Releases per nuclide (`annual_discharge_bq_rad_list` or<br>`instantaneous_release_bq_list`). |
| `mode` | `IngestionReleaseMode` | Release mode. |
| `chi_over_q` | `f64` | Maximum `chi/Q` for the distance, s/m^3. |
| `parameters` | `super::IngestionParameters` | `inges_param_dict`. |
| `diet_adult` | `super::DietaryIntake` | `inges_param_dict_adult`. |
| `diet_infant` | `super::DietaryIntake` | `inges_param_dict_infant`. |
| `soil` | `super::SoilType` | `soiltype`. |
| `climate` | `super::food_chain::Climate` | `climate` (H-3 only). |
| `veg_type` | `super::food_chain::VegetationType` | `veg_type_list` (H-3 only; upstream iterates `[veg_type_list]`, so it<br>must be a single type: a YAML list raises `TypeError`). |
| `animal_feed_type` | `super::food_chain::VegetationType` | `animal_feed_type` (H-3 and C-14). |
| `animal_products_h3` | `Vec<super::food_chain::AnimalProduct>` | `animal_product_list_for_tritium`. |
| `animal_products_c14` | `Vec<super::food_chain::AnimalProduct>` | `animal_product_list_for_C14`. |

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
    fn clone(self: &Self) -> UpstreamIngestionInputs { /* ... */ }
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
    fn eq(self: &Self, other: &UpstreamIngestionInputs) -> bool { /* ... */ }
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
#### Enum `UpstreamIngestionError`

Why upstream's `ingestion_dose` raises (or returns nothing).

```rust
pub enum UpstreamIngestionError {
    IndexError(&'static str),
    NameError,
    NoBranchMatches,
    Age,
}
```

##### Variants

###### `IndexError`

A per-element list indexed past its end (`IndexError`, defect D8).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `NameError`

Upstream prints `sum_hto_obt_animal_milk_tritium` before any milk
product has defined it (`NameError`): the first tritium animal product
is not a milk (defect D11b).

###### `NoBranchMatches`

Exactly `['H-3', 'C-14']` (or both with nothing else): no branch of the
output assembly matches and upstream returns whatever the previous call
left (`None` on a fresh object; defect D9).

###### `Age`

A NaN age bracket (cannot happen for adult/infant; kept for totality).

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
    fn clone(self: &Self) -> UpstreamIngestionError { /* ... */ }
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
    fn eq(self: &Self, other: &UpstreamIngestionError) -> bool { /* ... */ }
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
#### Type Alias `Routes`

The three routes of one nuclide in upstream's output, in upstream's order
`(veg, milk, meat)`, mSv (or mSv/y).

```rust
pub type Routes = [f64; 3];
```

#### Struct `UpstreamIngestionOutput`

Upstream's result: the stacked array, and the elements that had no
transfer factors (upstream's `notransfer_factor_rad`, used by the report).

```rust
pub struct UpstreamIngestionOutput {
    pub rows: Vec<Vec<f64>>,
    pub transposed: bool,
    pub no_transfer_factors: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `rows` | `Vec<Vec<f64>>` | Rows exactly as upstream's array: one `[veg, milk, meat]` per non-H-3,<br>non-C-14 nuclide in list order, then H-3, then C-14. When H-3, C-14 and<br>at least one other nuclide are all present, upstream **transposes** the<br>array; `transposed` is then true and `rows` holds the three route rows<br>(veg, milk, meat), each of nuclide length. |
| `transposed` | `bool` | Whether `rows` is upstream's transposed layout. |
| `no_transfer_factors` | `Vec<String>` | Elements missing from the eco-parameter table. |

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
    fn clone(self: &Self) -> UpstreamIngestionOutput { /* ... */ }
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
    fn eq(self: &Self, other: &UpstreamIngestionOutput) -> bool { /* ... */ }
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

#### Function `ingestion_dose_upstream`

**Attributes:**

- `Other("#[allow(clippy::too_many_lines)]")`

`DoseFunc.ingestion_dose` for one distance and receiver, with upstream's
arithmetic and indexing.

# Errors
Where upstream raises; see [`UpstreamIngestionError`].

```rust
pub fn ingestion_dose_upstream(inp: &UpstreamIngestionInputs, eco: &super::EcoParamTable, dcf_table: &super::IngestionDcfTable, receiver: super::Receiver) -> Result<UpstreamIngestionOutput, UpstreamIngestionError> { /* ... */ }
```

### Types

#### Struct `IngestionParameters`

Upstream's `inges_param_dict`: SRS 19 food-chain parameters (days, m^2/kg,
kg/d, m^3/d, Bq/m^3).

```rust
pub struct IngestionParameters {
    pub alpha_wet_crops: f64,
    pub alpha_dry_forage: f64,
    pub t_e_food_crops: f64,
    pub t_e_forage_grass: f64,
    pub t_b: f64,
    pub t_h_wet_crops: f64,
    pub t_h_animal_pasture: f64,
    pub t_h_animal_stored_feed: f64,
    pub c_wi: f64,
    pub f_p: f64,
    pub alpha: f64,
    pub t_e: f64,
    pub t_m: f64,
    pub t_f: f64,
    pub q_m: f64,
    pub q_w: f64,
    pub q_f: f64,
    pub q_w_meat: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `alpha_wet_crops` | `f64` | Interception per unit mass, wet food crops, m^2/kg. |
| `alpha_dry_forage` | `f64` | Interception per unit mass, dry forage, m^2/kg. |
| `t_e_food_crops` | `f64` | Crop exposure period during growth, food crops, d. |
| `t_e_forage_grass` | `f64` | Crop exposure period, forage grass, d. |
| `t_b` | `f64` | Duration of the discharge (soil build-up), d. |
| `t_h_wet_crops` | `f64` | Harvest-to-consumption delay, food crops, d. |
| `t_h_animal_pasture` | `f64` | Delay for fresh pasture, d. |
| `t_h_animal_stored_feed` | `f64` | Delay for stored feed, d. |
| `c_wi` | `f64` | Radionuclide concentration in the animals' water, Bq/m^3. |
| `f_p` | `f64` | Fraction of the year on fresh pasture. |
| `alpha` | `f64` | Upstream key `alpha` (read, unused). |
| `t_e` | `f64` | Upstream key `t_e` (read, unused). |
| `t_m` | `f64` | Milk collection-to-consumption delay, d. |
| `t_f` | `f64` | Meat collection-to-consumption delay, d. |
| `q_m` | `f64` | Dry feed eaten by a milk animal, kg/d. |
| `q_w` | `f64` | Water drunk by a milk animal, m^3/d. |
| `q_f` | `f64` | Feed eaten by a meat animal, kg/d. |
| `q_w_meat` | `f64` | Water drunk by a meat animal, m^3/d. |

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
    fn clone(self: &Self) -> IngestionParameters { /* ... */ }
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
    fn eq(self: &Self, other: &IngestionParameters) -> bool { /* ... */ }
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
#### Struct `DietaryIntake`

Upstream's dietary intake dictionary (`inges_param_dict_adult` /
`_infant`): `DID_veg`, `DID_milk`, `DID_meat`, `DID_fish`,
`DID_water_and_beverage`.

The pathway multiplies these by 365 for a long-term release and by 1 for a
single plume, so they are **per day**. The input generator's defaults are
per day (1.05 kg/d of vegetables for an adult); the fallback in
`dosefunc.py` holds **annual** values (76.7 kg of vegetables) that are then
multiplied by 365 again (defect D12). Fish and water are read but no
pathway uses them.

```rust
pub struct DietaryIntake {
    pub veg: f64,
    pub milk: f64,
    pub meat: f64,
    pub fish: f64,
    pub water_and_beverage: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `veg` | `f64` | Vegetables, kg/d. |
| `milk` | `f64` | Milk, L/d. |
| `meat` | `f64` | Meat, kg/d. |
| `fish` | `f64` | Fish, kg/d (unused upstream). |
| `water_and_beverage` | `f64` | Water and beverages, m^3/d (unused upstream). |

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
    fn clone(self: &Self) -> DietaryIntake { /* ... */ }
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
    fn eq(self: &Self, other: &DietaryIntake) -> bool { /* ... */ }
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
#### Enum `Receiver`

Who eats: upstream's `receiver`, which fixes the ingestion coefficient age
(adult 18, infant 1) and the diet.

```rust
pub enum Receiver {
    Adult,
    Infant,
}
```

##### Variants

###### `Adult`

`'adult'`, age 18.

###### `Infant`

`'infant'`, age 1.

##### Implementations

###### Methods

- ```rust
  pub const fn age_years(self: Self) -> f64 { /* ... */ }
  ```
  The age upstream uses for the coefficient lookup.

- ```rust
  pub fn from_driver_age(age: f64) -> Option<Self> { /* ... */ }
  ```
  Upstream's driver (`agewise_ingestion_dose`): `age > 17` is an adult,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Receiver { /* ... */ }
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
    fn eq(self: &Self, other: &Receiver) -> bool { /* ... */ }
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
#### Enum `SoilType`

Soil type for the effective surface density (`soiltype`), SRS 19 Table IX.

```rust
pub enum SoilType {
    PeatSoil,
    OtherSoil,
}
```

##### Variants

###### `PeatSoil`

`'peatsoil'`: 50 kg/m^2 (pasture), 100 kg/m^2 (crops).

###### `OtherSoil`

`'othersoil'`: 130 kg/m^2 (pasture), 260 kg/m^2 (crops).

##### Implementations

###### Methods

- ```rust
  pub const fn surface_densities(self: Self) -> (f64, f64) { /* ... */ }
  ```
  `(rho_pasture_depth_lt_11, rho_crop_depth_ge_11)`, kg/m^2 dry soil.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SoilType { /* ... */ }
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
    fn eq(self: &Self, other: &SoilType) -> bool { /* ... */ }
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
#### Struct `TransferFactors`

Element transfer factors and loss rates, one row of upstream's `eco_param`.

```rust
pub struct TransferFactors {
    pub lambda_s_per_d: f64,
    pub fv1: f64,
    pub fv2: f64,
    pub lambda_w_per_d: f64,
    pub fm_milk_d_per_l: f64,
    pub ff_meat_d_per_kg: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lambda_s_per_d` | `f64` | Soil loss rate `lambda_s`, 1/d. |
| `fv1` | `f64` | Soil-to-pasture concentration factor `Fv1`. |
| `fv2` | `f64` | Soil-to-crop concentration factor `Fv2`. |
| `lambda_w_per_d` | `f64` | Plant-surface loss rate `lambda_w`, 1/d. |
| `fm_milk_d_per_l` | `f64` | Feed-to-milk transfer `Fm`, d/L. |
| `ff_meat_d_per_kg` | `f64` | Feed-to-meat transfer `Ff`, d/kg. |

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
    fn clone(self: &Self) -> TransferFactors { /* ... */ }
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
    fn eq(self: &Self, other: &TransferFactors) -> bool { /* ... */ }
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
#### Struct `EcoParamTable`

Upstream's `eco_param` sheet.

```rust
pub struct EcoParamTable {
    pub rows: Vec<(String, TransferFactors)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `rows` | `Vec<(String, TransferFactors)>` | `(Element field, factors)` in file order. |

##### Implementations

###### Methods

- ```rust
  pub fn from_csv(text: &str) -> Result<Self, String> { /* ... */ }
  ```
  Read a CSV with upstream's columns `Element, lambda_s_per_d, Fv1, Fv2,

- ```rust
  pub fn lookup(self: &Self, element: &str) -> Option<TransferFactors> { /* ... */ }
  ```
  Upstream's lookup in `fv_list_ecerman_ingestion`: the **first** row

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> EcoParamTable { /* ... */ }
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
    fn default() -> EcoParamTable { /* ... */ }
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
    fn eq(self: &Self, other: &EcoParamTable) -> bool { /* ... */ }
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
#### Enum `IngestionDcf`

An ingestion coefficient: one value, or tritium's HTO and OBT pair.

```rust
pub enum IngestionDcf {
    Single(f64),
    Tritium {
        hto: f64,
        obt: f64,
    },
}
```

##### Variants

###### `Single`

Sv/Bq.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Tritium`

Tritium, Sv/Bq: tritiated water and organically bound tritium.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `hto` | `f64` | HTO. |
| `obt` | `f64` | OBT. |

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
    fn clone(self: &Self) -> IngestionDcf { /* ... */ }
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
    fn eq(self: &Self, other: &IngestionDcf) -> bool { /* ... */ }
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
#### Struct `IngestionDcfTable`

Upstream's `ingestion_gsr3` sheet: `Nuclide` and the six e(g) columns of
[`InhalationDcfTable::AGE_COLUMNS`].

```rust
pub struct IngestionDcfTable {
    pub rows: Vec<(String, [f64; 6])>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `rows` | `Vec<(String, [f64; 6])>` | `(nuclide, coefficients by age bracket)`. |

##### Implementations

###### Methods

- ```rust
  pub fn from_csv(text: &str) -> Result<Self, String> { /* ... */ }
  ```
  Read a CSV with columns `Nuclide` and the six `e_g_age_g_*_Sv/Bq`.

- ```rust
  pub fn lookup(self: &Self, nuclide: &str, age: AgeBracket) -> IngestionDcf { /* ... */ }
  ```
  Upstream's `dcf_list_ingestion` for one nuclide: the largest

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> IngestionDcfTable { /* ... */ }
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
    fn default() -> IngestionDcfTable { /* ... */ }
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
    fn eq(self: &Self, other: &IngestionDcfTable) -> bool { /* ... */ }
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

#### Function `ingestion_weathering_correction_unused`

**Attributes:**

- `MustUse { reason: None }`

Upstream's `ingestion_weathering_correction` (the **unused** variant: the
pathway calls `ingestion_weathering_correction_real` instead). Adds a
14-day weathering half-life (`0.693 / (14 * 86400)` 1/s) to the decay
constant of every element except H, C and the noble gases, when enabled.

```rust
pub fn ingestion_weathering_correction_unused(decay_constant_per_s: f64, element: &str, enabled: bool) -> f64 { /* ... */ }
```

#### Function `zero_milk_and_meat`

**Attributes:**

- `MustUse { reason: None }`

Upstream's `zeroing_ingestion` intent: the milk and meat doses of a
nuclide whose element has no transfer factors are set to zero.

Note: upstream writes `df.loc[rad][1:] = 0`, a chained assignment. Under
pandas copy-on-write (pandas 3, used for the fixture) it modifies a copy and
the report is **unchanged** (defect D14; checked in the fixture). This
function does what the code says it means; the code-to-code test records
both.

```rust
pub fn zero_milk_and_meat(route: [f64; 3], element_has_no_transfer_factors: bool) -> [f64; 3] { /* ... */ }
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
assertion (defect D3). ~~so nothing in this port consumes them~~
**CHANGED 2026-09-28:** the labelled divergence
[`super::dispersion::dilution_single_plume_with_met_speeds`] uses them
(converted to m/s).

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

## Module `plume_rise`

Plume rise and building wake, which pyDOSEIA defines but never calls.

# Provenance

Ported from pyDOSEIA `metfunc.py` (`MetFunc.compute_plume_rise_neutral_unstable_cat`,
`compute_plume_rise_stable_cat`, `building_wake_effect_gifford`), upstream
<https://github.com/BiswajitSadhu/pyDOSEIA> at commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). Upstream cites
the AERB/NF/SG/S-1 guide (p. 44) and IAEA-TECDOC-379; neither was
available to check the formulas against.

None of these is used by upstream's dose calculation (its release height is
the effective height, with no rise and no wake), and none is used by this
crate's pathways either.

| Function | Upstream state | Here |
|---|---|---|
| [`plume_rise_neutral_unstable`] | runs, never called | ported faithfully |
| [`plume_rise_stable_upstream`] | runs, never called, marked TO-DO; defect **D6** | ported faithfully (always class F, second formula) |
| [`plume_rise_stable_both_formulas`] | — | **divergence**: D6 corrected |
| building wake | cannot run (`TypeError`), marked TO-DO; defect **D5** | [`building_wake_gifford`] is a **divergence** only |

```rust
pub mod plume_rise { /* ... */ }
```

### Types

#### Enum `StableClass`

Stable class for [`plume_rise_stable_both_formulas`].

```rust
pub enum StableClass {
    E,
    F,
}
```

##### Variants

###### `E`

Class E, `S = 8.7e-4`.

###### `F`

Class F, `S = 1.75e-3`.

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
    fn clone(self: &Self) -> StableClass { /* ... */ }
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
    fn eq(self: &Self, other: &StableClass) -> bool { /* ... */ }
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
#### Struct `StablePlumeRise`

Both stable momentum-rise formulas upstream writes, m.

```rust
pub struct StablePlumeRise {
    pub calm_formula: f64,
    pub windy_formula: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `calm_formula` | `f64` | `4 (Fm/S)^(1/4)` (upstream's first, overwritten, formula). |
| `windy_formula` | `f64` | `1.5 S^(-1/6) (Fm/U)^(1/3)`. |

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
    fn clone(self: &Self) -> StablePlumeRise { /* ... */ }
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
    fn eq(self: &Self, other: &StablePlumeRise) -> bool { /* ... */ }
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

#### Function `plume_rise_neutral_unstable`

**Attributes:**

- `MustUse { reason: None }`

`compute_plume_rise_neutral_unstable_cat(W0, x, U, D_i, D_e)` (classes
A-D), m: the smaller of
`1.44 D_i (W0/U)^(2/3) (x/D_i)^(1/3) - 3 (1.5 - W0/U) D_e` and
`3 D_i W0/U`. Upstream's defaults are `W0 = 10` m/s, `x = 100` m,
`U = 2` m/s, `D_i = 5` m, `D_e = 8` m.

```rust
pub fn plume_rise_neutral_unstable(w0: f64, x: f64, u: f64, d_i: f64, d_e: f64) -> f64 { /* ... */ }
```

#### Function `momentum_flux`

**Attributes:**

- `MustUse { reason: None }`

Upstream's momentum flux parameter `Fm = W0^2 (D_i/2)^2`.

```rust
pub fn momentum_flux(w0: f64, d_i: f64) -> f64 { /* ... */ }
```

#### Function `plume_rise_stable_upstream`

**Attributes:**

- `MustUse { reason: None }`

`compute_plume_rise_stable_cat(W0, U, D_i)` **as upstream computes it**
(defect D6): the stability parameter is assigned for class E and then
overwritten with class F's, and the calm formula is computed and then
overwritten by the windy one, so the result is always
`1.5 S_F^(-1/6) (Fm/U)^(1/3)` with `S_F = 1.75e-3`.

```rust
pub fn plume_rise_stable_upstream(w0: f64, u: f64, d_i: f64) -> f64 { /* ... */ }
```

#### Function `plume_rise_stable_both_formulas`

**Attributes:**

- `MustUse { reason: None }`

**Divergence from upstream (D6 corrected):** uses the stability parameter
of the class asked for and returns **both** formulas upstream writes,
instead of discarding the first. Which one applies (Briggs' guidance takes
the smaller) is left to the caller: the AERB guide upstream cites was not
available to check what it prescribes.

```rust
pub fn plume_rise_stable_both_formulas(w0: f64, u: f64, d_i: f64, class: StableClass) -> StablePlumeRise { /* ... */ }
```

#### Function `building_wake_gifford`

**Attributes:**

- `MustUse { reason: None }`

**Divergence from upstream (D5 corrected):** Gifford's building-wake
dilution factor `chi/Q = 1 / ((c A + pi sigma_y sigma_z) U)`, `c = 0.5`,
floored at one third of the unwaked value, s/m^3.

Upstream's `building_wake_effect_gifford` cannot run: it multiplies the
bound methods `self.sigmay` and `self.sigmaz` (a `TypeError`), and it
**multiplies** by `U` where the formula divides. This version takes the
sigmas (m) as arguments and divides by the wind speed (m/s); the floor is
upstream's. Not checked against the AERB guide or TECDOC-379.

```rust
pub fn building_wake_gifford(chi_over_q_unwaked: f64, building_area_m2: f64, wind_speed_m_per_s: f64, sigma_y_m: f64, sigma_z_m: f64) -> f64 { /* ... */ }
```

## Module `plume_shine`

Plume shine: the external gamma dose from the passing cloud, by pyDOSEIA's
finite-cloud model: the Gaussian plume concentration folded with a
point-kernel photon flux (exponential attenuation, linear build-up
`1 + k mu r`) and integrated over a box around the receptor.

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Never a
> dose to a real person.

# Provenance

Ported from pyDOSEIA `dosefunc.py` (`DoseFunc.plumeshine_dose`, with its
nested `adgq_single_plume`, `adgq_sector_average` and
`get_all_integral_stab_cat_energy_wise_for_all_rad_parallel`) and
`raddcffunc.py` (`gamma_energy_abundaces`, `add_zero_energy_for_pure_beta`,
`atten_coeff` — the second definition, which is the one Python binds —
`get_k_mu_mua_MFP`, `zyx_lim_for_integral`,
`zyx_lim_for_integral_single_plume`,
`zyx_lim_for_integral_sector_averaged_plume`,
`get_limit_lists_per_rad_for_all_energies`, and the module-level
`point_source_dose`), upstream <https://github.com/BiswajitSadhu/pyDOSEIA>
at commit `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024
Dr. Biswajit Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).
Upstream cites Wang, Ling and Shi, *Nucl. Eng. Des.* 231 (2004) 211-216
for the mean-free-path integration limits.

~~The triple integral uses [`super::quadpack::tplquad`], a port of the SciPy
QUADPACK routine upstream calls~~ **CHANGED 2026-09-28** (maintainer:
"include petir as a dependency to buangkok. quadpack should be used as
regression test, but petir is the main one"): by default the triple integral
runs on petir's port of GSL QAGS ([`petir_tplquad`],
[`PlumeShineIntegrator::Petir`]). [`super::quadpack::tplquad`], the port of
the SciPy QUADPACK routine upstream calls, stays as
[`PlumeShineIntegrator::ScipyQuadpackReference`]: select it to reproduce
upstream's adaptive subdivision, and so its numbers, bit for bit (the
code-to-code fixture does). Both are QUADPACK `dqagse`; the difference
between them is measured in `docs/pydoseia-code-to-code.md`.

# No photon data ships with this crate

| Upstream sheet | Content | Source as upstream states | Here |
|---|---|---|---|
| `Dose_ecerman_final.xlsx` / `gamma_energy_radionuclide` | gamma energies and emission probabilities | IAEA "Update of X-ray and gamma-ray decay data standards" (2007), `www-nds.iaea.org/xgamma_standards` | **not copied** (IAEA terms of use not established); read your own table with [`GammaLineTable::from_csv`] |
| `Dose_ecerman_final.xlsx` / `mass_attenuation_coeff` | mass attenuation and mass energy-absorption coefficients of air | NIST (Hubbell and Seltzer), `physics.nist.gov/PhysRefData/XrayMassCoef/ComTab/air.html` | **not copied**: NIST Standard Reference Data may carry copyright under the Standard Reference Data Act (15 U.S.C. 290e), and the terms of this table were not established; read your own with [`AttenuationTable::from_csv`] |

The code-to-code test uses **synthetic** tables in these layouts.

# What the numbers mean (and an upstream unit ambiguity)

Upstream multiplies each line's integral by `5e-4 * E * mu_a * yield`, sums
the lines, and multiplies by the release (Bq/s for a long-term release,
`annual / 31 536 000`; Bq for a single plume). Its comments call the
result **microSv/h**, while its text report heads the met-data table
**"microSv per year"** and the met-data branch sums the frequency table's
raw **hour counts** without dividing by the hours of data (defect D16 in
`docs/pydoseia-code-to-code.md`). The port reproduces the numbers and does
not assign them a unit type: they are plain `f64` "upstream plume-shine
values".

```rust
pub mod plume_shine { /* ... */ }
```

### Types

#### Enum `PlumeShineRelease`

The release multiplier upstream applies at the end: Bq/s
(`annual / 31 536 000`) for a long-term release, Bq for a single plume.

```rust
pub enum PlumeShineRelease {
    AnnualDischargeBq(f64),
    InstantaneousBq(f64),
}
```

##### Variants

###### `AnnualDischargeBq`

Long-term release: activity discharged per year, Bq/y.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `InstantaneousBq`

Instantaneous release: activity released, Bq.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn multiplier(self: Self) -> f64 { /* ... */ }
  ```
  The factor upstream multiplies by.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PlumeShineRelease { /* ... */ }
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
    fn eq(self: &Self, other: &PlumeShineRelease) -> bool { /* ... */ }
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
#### Enum `PointSourceUnit`

Output unit branch of [`point_source_dose`].

```rust
pub enum PointSourceUnit {
    MilliSievertPerHour,
    MilliRoentgenPerHour,
}
```

##### Variants

###### `MilliSievertPerHour`

Upstream `'mSv/hr'`.

###### `MilliRoentgenPerHour`

Upstream `'mR/hr'`.

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
    fn clone(self: &Self) -> PointSourceUnit { /* ... */ }
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
    fn eq(self: &Self, other: &PointSourceUnit) -> bool { /* ... */ }
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

#### Function `prefactor`

**Attributes:**

- `MustUse { reason: None }`

Upstream's plume-shine prefactor `5 * 10 ** (-4)`, evaluated as Python
does (`10 ** -4` is `pow(10.0, -4.0)`).

```rust
pub fn prefactor() -> f64 { /* ... */ }
```

#### Function `line_integrals`

**Attributes:**

- `MustUse { reason: None }`

The per-class integrals of one gamma line, `[A..F]` (upstream
`all_integral_stab_cat_energy_wise[rad][line]`). A zero-energy placeholder
line (pure beta emitter) is not integrated and gives zeros: upstream does
integrate it, and multiplies the result by the zero energy and yield.

```rust
pub fn line_integrals(line: GammaLine, table: &AttenuationTable, geometry: PlumeShineGeometry, integrator: PlumeShineIntegrator) -> [f64; 6] { /* ... */ }
```

#### Function `per_class_unit_release`

**Attributes:**

- `MustUse { reason: None }`

Plume shine per stability class for **unit release**, summed over the
lines (upstream's `pl_sh_sectors` before the release multiplication, for
the single-plume and the long-term no-met branches).

`lines` are what [`GammaLineTable::plume_shine_lines`] returns (upstream
`gamma_energy_abundaces` + `add_zero_energy_for_pure_beta`).

```rust
pub fn per_class_unit_release(lines: &[GammaLine], table: &AttenuationTable, geometry: PlumeShineGeometry, integrator: PlumeShineIntegrator) -> [f64; 6] { /* ... */ }
```

#### Function `per_class`

**Attributes:**

- `MustUse { reason: None }`

Plume shine per stability class for one nuclide (single plume, or long
term without met data): [`per_class_unit_release`] times the release.

```rust
pub fn per_class(lines: &[GammaLine], table: &AttenuationTable, geometry: PlumeShineGeometry, release: PlumeShineRelease, integrator: PlumeShineIntegrator) -> [f64; 6] { /* ... */ }
```

#### Function `per_sector_with_met`

**Attributes:**

- `MustUse { reason: None }`

Plume shine per 22.5-degree sector for one nuclide, long-term release
**with met data**, averaged over the years of `met` and multiplied by the
release (upstream's `have_met_data` branch).

Per year and line, the per-class integrals (times
`5e-4 * E * mu_a * yield`) are weighted by the missing-corrected TJFD
count divided by the speed-class wind speed (m/s) and the class's height
correction factor, and summed over the nine non-calm speed classes and the
six classes. Lines with zero energy or yield contribute zero.

Note (upstream behaviour, kept; defect D16): the counts are **not**
divided by the number of hours in the year, so the result grows with the
length of the met record; upstream computes `hours_without_calm` and never
uses it.

```rust
pub fn per_sector_with_met(lines: &[GammaLine], table: &AttenuationTable, geometry: PlumeShineGeometry, met: &super::met::MetClimatology, measurement_height: uom::si::f64::Length, release: PlumeShineRelease, integrator: PlumeShineIntegrator) -> [f64; 16] { /* ... */ }
```

#### Function `point_source_dose`

**Attributes:**

- `MustUse { reason: None }`

Upstream's module-level `point_source_dose`, one distance: the rule-of-thumb
dose rate `6 C E / d^2` of a point gamma source (C in curie, E in MeV,
d converted from metres with `3.28034 ft/m`), summed over lines and
multiplied by a damage ratio.

`unit` selects upstream's two branches: [`PointSourceUnit::MilliSievertPerHour`]
divides by `114 * 3.28034^2`, [`PointSourceUnit::MilliRoentgenPerHour`] by
`3.28034^2` only. Both then multiply by `damage_ratio * 1000`, as upstream.

Upstream's defaults (`gamma_energy`, `g_yield` for Ir-192, activity 1e6 Ci,
damage ratio 5e-5) are in [`POINT_SOURCE_DEFAULT_IR192_KEV`] and
[`POINT_SOURCE_DEFAULT_IR192_YIELD`]; they are upstream's literals, not
data curated here. The *method* `RaddcfFunc.point_source_dose` cannot run
upstream (it calls the list `self.rads_list()`, appends to a dict, and
unpacks three of four return values: defect D15) and is not ported.

```rust
pub fn point_source_dose(gamma_energy_kev: &[f64], yields: &[f64], activity_curie: f64, distance_m: f64, damage_ratio: f64, unit: PointSourceUnit) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `POINT_SOURCE_DEFAULT_IR192_KEV`

Upstream's default gamma energies for [`point_source_dose`] (keV; upstream
says Ir-192). Upstream's literals, reproduced as code defaults.

```rust
pub const POINT_SOURCE_DEFAULT_IR192_KEV: [f64; 9] = _;
```

#### Constant `POINT_SOURCE_DEFAULT_IR192_YIELD`

Upstream's default yields for [`point_source_dose`].

```rust
pub const POINT_SOURCE_DEFAULT_IR192_YIELD: [f64; 9] = _;
```

#### Constant `POINT_SOURCE_DEFAULT_DISTANCES_M`

Upstream's default distances for [`point_source_dose`], m.

```rust
pub const POINT_SOURCE_DEFAULT_DISTANCES_M: [f64; 10] = _;
```

### Re-exports

#### Re-export `integration_limits_legacy`

```rust
pub use integral::integration_limits_legacy;
```

#### Re-export `integration_limits_sector_averaged`

```rust
pub use integral::integration_limits_sector_averaged;
```

#### Re-export `integration_limits_single_plume`

```rust
pub use integral::integration_limits_single_plume;
```

#### Re-export `kernel_sector_averaged`

```rust
pub use integral::kernel_sector_averaged;
```

#### Re-export `kernel_single_plume`

```rust
pub use integral::kernel_single_plume;
```

#### Re-export `line_integral`

```rust
pub use integral::line_integral;
```

#### Re-export `petir_tplquad`

```rust
pub use integral::petir_tplquad;
```

#### Re-export `NestedQuadrature`

```rust
pub use integral::NestedQuadrature;
```

#### Re-export `PlumeShineGeometry`

```rust
pub use integral::PlumeShineGeometry;
```

#### Re-export `PlumeShineIntegrator`

```rust
pub use integral::PlumeShineIntegrator;
```

#### Re-export `PlumeShineMode`

```rust
pub use integral::PlumeShineMode;
```

#### Re-export `SECTOR_AVERAGED_EPS`

```rust
pub use integral::SECTOR_AVERAGED_EPS;
```

#### Re-export `SINGLE_PLUME_EPS`

```rust
pub use integral::SINGLE_PLUME_EPS;
```

#### Re-export `numpy_interp`

```rust
pub use tables::numpy_interp;
```

#### Re-export `AirPhotonCoefficients`

```rust
pub use tables::AirPhotonCoefficients;
```

#### Re-export `AttenuationTable`

```rust
pub use tables::AttenuationTable;
```

#### Re-export `GammaLine`

```rust
pub use tables::GammaLine;
```

#### Re-export `GammaLineTable`

```rust
pub use tables::GammaLineTable;
```

#### Re-export `NuclideGammaLines`

```rust
pub use tables::NuclideGammaLines;
```

#### Re-export `AIR_DENSITY_G_PER_CM3`

```rust
pub use tables::AIR_DENSITY_G_PER_CM3;
```

## Module `quadpack`

The adaptive quadrature pyDOSEIA's plume-shine integral runs on:
QUADPACK's `dqagse` (21-point Gauss-Kronrod, bisection, Wynn epsilon
extrapolation), and SciPy's `tplquad` nesting of it.

# Provenance

pyDOSEIA (`dosefunc.py`, `plumeshine_dose`) integrates the finite-cloud
kernel with `scipy.integrate.tplquad`. In SciPy 1.18.1 that is `nquad`
(`scipy/integrate/_quadpack_py.py`, class `_NQuad`), which calls
`quad` -> `_quadpack._qagse` at each of the three levels. `_qagse` is
SciPy's C translation of QUADPACK, `scipy/integrate/__quadpack.c`
(functions `dqagse`, `dqk21`, `dqelg`, `dqpsrt`), Copyright (C) 2024 SciPy
developers, BSD 3-clause; itself a translation of the public-domain Fortran
QUADPACK by R. Piessens, E. de Doncker-Kapenga, C. Ueberhuber and
D. Kahaner (1983), <https://www.netlib.org/quadpack/> (`dqagse.f`,
`dqk21.f`, `dqelg.f`, `dqpsrt.f`). Both were read for this port
(SciPy tag `v1.18.1`, netlib files fetched 2026-09-28). The port follows
the **C** text, including its 0-based indexing, because that is what
pyDOSEIA executes. BSD 3-clause is compatible with this crate's GPL-3.0;
the notice is reproduced in `crates/buangkok/NOTICE`.

# Role: the regression reference, not the default

~~Why not `petir::integration::qag`~~ **CHANGED 2026-09-28** (maintainer:
"include petir as a dependency to buangkok. quadpack should be used as
regression test, but petir is the main one"). Plume shine now integrates
by default with `petir::integration::qags`, petir's port of GSL
`gsl_integration_qags` (itself QUADPACK `dqagse`), ported into petir for
this purpose. This module stays because it is what pyDOSEIA executes: the
code-to-code fixture selects it
([`PlumeShineIntegrator::ScipyQuadpackReference`](crate::pydoseia::plume_shine::PlumeShineIntegrator))
so the comparison against upstream stays bit-exact, and
`tests/plume_shine_petir_vs_quadpack.rs` measures petir against it.

What was true on 2026-09-28 before the change, and still is: petir's
plain `qag` is `dqage`, without the epsilon extrapolation `dqagse` adds, and
differs from SciPy's routine on singular integrands (the `quadpack`-group
mutation test). Petir's `qags` does not have that gap.

```rust
pub mod quadpack { /* ... */ }
```

### Types

#### Struct `QuadResult`

Outcome of one [`qagse`] call.

```rust
pub struct QuadResult {
    pub value: f64,
    pub abserr: f64,
    pub ier: i32,
    pub neval: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `value` | `f64` | The integral estimate (what `scipy.integrate.quad` returns first). |
| `abserr` | `f64` | The error estimate. |
| `ier` | `i32` | QUADPACK's `ier` (0 = requested accuracy reached). SciPy only warns on<br>a non-zero value and still returns `value`, so callers here do the same. |
| `neval` | `usize` | Integrand evaluations. |

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
    fn clone(self: &Self) -> QuadResult { /* ... */ }
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
    fn eq(self: &Self, other: &QuadResult) -> bool { /* ... */ }
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

#### Function `qagse`

**Attributes:**

- `Other("#[allow(clippy::too_many_lines)]")`

QUADPACK `dqagse` as SciPy's `scipy.integrate.quad` calls it for finite
limits (`limit` = 50 there). Returns the estimate even when `ier != 0`, as
SciPy does.

```rust
pub fn qagse<F: FnMut(f64) -> f64>(f: F, a: f64, b: f64, epsabs: f64, epsrel: f64, limit: usize) -> QuadResult { /* ... */ }
```

#### Function `tplquad`

`scipy.integrate.tplquad(func, a, b, gfun, hfun, qfun, rfun, epsabs=,
epsrel=)` with constant limits, as `nquad` evaluates it: the **outer**
integral over `z` in `[z_lo, z_hi]`, the middle over `y` in `[y_lo, y_hi]`,
the inner over `x` in `[x_lo, x_hi]`; every level is [`qagse`] with the same
tolerances and [`SCIPY_QUAD_LIMIT`].

`f(x, y, z)` receives the innermost variable first, which is how SciPy
calls `func` (pyDOSEIA's integrand is written `lambda x, y, z`). The
limits array is in pyDOSEIA's order `[z_lo, z_hi, y_lo, y_hi, x_lo, x_hi]`.

```rust
pub fn tplquad<F: FnMut(f64, f64, f64) -> f64>(f: F, limits: [f64; 6], epsabs: f64, epsrel: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `SCIPY_QUAD_LIMIT`

SciPy's default `limit` for `quad` (and so for every level of `tplquad`).

```rust
pub const SCIPY_QUAD_LIMIT: usize = 50;
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

