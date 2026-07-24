# Crate Documentation

**Version:** 0.0.0

**Format Version:** 60

# Module `outram_foam_multiphase`

# outram-foam-multiphase

**OUTRAM-FOAM Phase II — multiphase CFD** (bead epic `op-2kk`). Pure-Rust
translation of OpenFOAM's multiphase solver family on top of
[`outram_foam_basic_lib`]'s finite-volume framework (`FvMesh`, fields,
`fvc`/`fvm` operators). This is the **authoritative high-fidelity reference**
from which TAMPINES' 1D reduced-order system-code physics (epic `op-dt3`)
are derived — 1D models must trace back to a validated 3D reference here,
never be invented independently.

> **⚠️ Unverified until validated — early/scaffold.** Everything here is a
> work-in-progress translation with no human V&V yet. Not for nuclear
> facility operation, reactor control, safety-critical, or licensing
> decisions. Independent OUTRAM PARK fork, not the official OpenFOAM
> software (see the workspace `TRADEMARKS.md`).

## Roadmap (per the Phase II architecture; each solver bead has its own DoD:
## theory docs + verification tests + reference-benchmark comparison + uom)

- **Stage 1 — Drift Flux** ([`drift_flux`]) — mixture continuity/momentum,
  void-fraction transport, algebraic slip / drift-velocity closures
  (Zuber-Findlay, terminal velocity, user-defined). Ref OpenFOAM
  `incompressibleDriftFlux`. **In progress** (bead `op-2kk.1`).
- **Stage 2 — Euler-Euler two-fluid** ([`two_fluid`]) — per-phase
  continuity + drag closures (Schiller-Naumann, Wen-Yu), 6-equation
  architecture scaffolded. Ref OpenFOAM `multiphaseEuler`. Foundation done
  (bead `op-2kk.2`).
- **Stage 3 — Wall boiling framework** ([`wall_boiling`]) — RPI heat-flux
  partitioning (Kurul & Podowski). Foundation done (bead `op-2kk.3`).
- **Stage 4 — CHF models** ([`chf`]) — Biasi / W-3 / Bowring correlations +
  Groeneveld LUT framework. Foundation done (bead `op-2kk.4`).
- **Stage 5 — Dryout / post-dryout framework** ([`dryout`]) — reserved
  interfaces + Dougall-Rohsenow worked example. Foundation done (`op-2kk.5`).

All Stage 2-5 modules are **unit-tested foundations, not validated solvers**
(no full pressure coupling; benchmark validation is a later human step) —
see each module's "Honest scope".

## Design rules (workspace `CLAUDE.md`)

Enum dispatch (no `Box<dyn>`), no lifetime parameters (`Arc`, index ids),
`uom`-typed API boundaries, GPLv3 + OpenFOAM provenance headers on ported
files, Android-buildable (pure-Rust, no system BLAS/GUI).

## Modules

## Module `chf`

Stage 4 — **Critical-heat-flux (CHF / DNB / dryout) correlations**
(bead `op-2kk.4`).

Point (0-D) engineering correlations that predict the **critical heat flux**
`q''_c` `[W/m²]` — the wall heat flux at which the boiling-heat-transfer
mode degrades (departure from nucleate boiling at low quality, dryout at
high quality) and the wall temperature excursion begins — as a function of
local flow conditions. These are the closures a wall-boiling or two-fluid
solver queries to detect the CHF limit; they are *not* CFD themselves.

## Models implemented (all from the open literature)

| Correlation | Regime | Source |
|---|---|---|
| [`Biasi`]  | round tube, local quality | Biasi et al. (1967) |
| [`W3`]     | PWR rod bundle / tube DNB, low quality + subcooling | Tong (1967) |
| [`Bowring`]| round tube, uniform flux, dryout | Bowring (1972) |
| [`GroeneveldLut`] | tabulated CHF(P, G, x) for an 8 mm tube | Groeneveld et al. (2007) |

Runtime model selection uses the [`ChfCorrelation`] **enum** (no `dyn`
dispatch, per the workspace design rules), which forwards the [`ChfModel`]
contract to the selected concrete correlation.

## Units and the API boundary

Every public entry point takes and returns `uom`-typed physical quantities so
the dimensions are checked at the boundary:

- pressure `P` — [`Pressure`] `[Pa]`
- mass flux `G` — [`MassFlux`] `[kg·m⁻²·s⁻¹]`
- thermodynamic equilibrium quality `x` — `f64`, dimensionless `[-]`
- (heated / hydraulic) diameter `D` — [`Length`] `[m]`
- result CHF `q''_c` — [`HeatFluxDensity`] `[W/m²]`

Two correlations need one extra scalar that the four-argument [`ChfModel`]
signature does not carry, so it is stored on the model struct at
construction (also `uom`-typed):

- [`W3`] needs the **inlet subcooling enthalpy** `Δh_in = h_f − h_in`
  `[J/kg]` ([`AvailableEnergy`]) for its subcooling factor.
- [`Bowring`] needs the **latent heat of vaporisation** `h_fg` `[J/kg]`
  ([`AvailableEnergy`]) at the system pressure (from steam tables — public
  literature data).

## Honest scope — verification, not validation

The inline tests below are **verification** checks: each correlation is
evaluated against a **hand-computed reference point** (the full arithmetic is
written out in the test doc comment) to confirm the algebra is implemented
correctly, and the look-up-table interpolation is checked for node-exactness,
midpoint correctness, and CSV round-trip. **No benchmark validation against
experimental CHF databases has been performed here** — that is a later,
human-run V&V step. Do not read these correlations as validated for any
design or safety purpose (see the workspace `RESPONSIBLE_USE.md`: AI-assisted
output is untrusted draft material until human-reviewed).

The stated pressure / mass-flux / quality / diameter validity range of each
correlation is documented on the model and exposed via
[`ChfModel::in_valid_range`]; evaluating **outside** that range extrapolates
(it is not auto-clamped) and the caller is responsible for checking. Clearly
non-physical inputs (`P ≤ 0`, `G ≤ 0`, `D ≤ 0`, `x > 1`) return
[`MultiphaseError::InvalidInput`]. The look-up table instead **clamps** an
out-of-range query to its axis bounds (documented on [`GroeneveldLut`]).

```rust
pub mod chf { /* ... */ }
```

### Types

#### Enum `ChfCorrelation`

Runtime-selectable critical-heat-flux correlation (enum dispatch, no `dyn`).

Wraps the four concrete models so a solver can choose a correlation at run
time while keeping the exhaustiveness, zero-allocation, and
rust-analyzer-navigability benefits of an enum over a trait object. Forwards
the [`ChfModel`] contract to the selected variant.

# Variants
- [`Biasi`](Self::Biasi) — Biasi et al. (1967) round-tube correlation.
- [`W3`](Self::W3) — Westinghouse W-3 / Tong (1967) DNB correlation.
- [`Bowring`](Self::Bowring) — Bowring (1972) round-tube dryout correlation.
- [`GroeneveldLut`](Self::GroeneveldLut) — Groeneveld 2006 look-up-table
  framework (interpolated CHF for an 8 mm reference tube).

```rust
pub enum ChfCorrelation {
    Biasi(Biasi),
    W3(W3),
    Bowring(Bowring),
    GroeneveldLut(GroeneveldLut),
}
```

##### Variants

###### `Biasi`

Biasi et al. (1967) correlation. See [`Biasi`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Biasi` |  |

###### `W3`

Westinghouse W-3 / Tong (1967) DNB correlation. See [`W3`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `W3` |  |

###### `Bowring`

Bowring (1972) round-tube dryout correlation. See [`Bowring`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Bowring` |  |

###### `GroeneveldLut`

Groeneveld look-up-table framework. See [`GroeneveldLut`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `GroeneveldLut` |  |

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

- **ChfModel**
  - ```rust
    fn critical_heat_flux(self: &Self, pressure: Pressure, mass_flux: MassFlux, quality: f64, diameter: Length) -> Result<HeatFluxDensity, MultiphaseError> { /* ... */ }
    ```

  - ```rust
    fn in_valid_range(self: &Self, pressure: Pressure, mass_flux: MassFlux, quality: f64, diameter: Length) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Biasi`

**Biasi et al. (1967)** critical-heat-flux correlation for upward flow of
water in uniformly heated round tubes.

Source: L. Biasi, G.C. Clerici, S. Garribba, R. Sala, A. Tozzi (1967),
*"Studies on burnout — Part 3: A new correlation for round ducts and uniform
heating and its comparison with world data"*, Energia Nucleare **14**(9),
530–536. Form and constants as reproduced in N.E. Todreas & M.S. Kazimi,
*Nuclear Systems Volume I: Thermal Hydraulic Fundamentals* (2nd ed., 2012),
and S.M. Ghiaasiaan, *Two-Phase Flow, Boiling and Condensation* (2nd ed.,
2017).

## Correlation

Two candidate heat fluxes are formed (`p` in **bar**, `D` in **cm**,
`G` in `kg·m⁻²·s⁻¹`, `x` dimensionless, `q''` in `W/m²`):

low-quality / high-flux branch
`q''_1 = (1.883e7 / (D^n · G^{1/6})) · ( f_p / G^{1/6} − x )`

high-quality / low-flux branch
`q''_2 = (3.78e7 · h_p / (D^n · G^{0.6})) · ( 1 − x )`

with the pressure functions
`f_p = 0.7249 + 0.099·p·exp(−0.032·p)` and
`h_p = −1.159 + 0.149·p·exp(−0.019·p) + 8.99·p / (10 + p²)`,
and diameter exponent `n = 0.4` for `D ≥ 1 cm`, `n = 0.6` for `D < 1 cm`.

Selection rule: for `G ≥ 300 kg·m⁻²·s⁻¹`, `q''_c = max(q''_1, q''_2)`;
for `G < 300`, only the high-quality branch is used, `q''_c = q''_2`.

## Validity range (from the source)
- pressure `P`: 2.7 – 140 bar (0.27 – 14.0 MPa)
- mass flux `G`: 100 – 6000 kg·m⁻²·s⁻¹
- diameter `D`: 0.3 – 3.75 cm
- quality `x`: up to the value making the selected branch non-negative

This model is stateless (no stored parameters).

```rust
pub struct Biasi;
```

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  Construct the (stateless) Biasi correlation.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ChfModel**
  - ```rust
    fn critical_heat_flux(self: &Self, pressure: Pressure, mass_flux: MassFlux, quality: f64, diameter: Length) -> Result<HeatFluxDensity, MultiphaseError> { /* ... */ }
    ```

  - ```rust
    fn in_valid_range(self: &Self, pressure: Pressure, mass_flux: MassFlux, _quality: f64, diameter: Length) -> bool { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Biasi { /* ... */ }
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
    fn default() -> Biasi { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `W3`

**Westinghouse W-3** (Tong, 1967) departure-from-nucleate-boiling (DNB)
correlation for pressurised-water conditions.

Source: L.S. Tong (1967), *"Prediction of departure from nucleate boiling for
an axially non-uniform heat flux distribution"*, J. Nuclear Energy **21**(3),
241–248; uniform-flux form as tabulated in Todreas & Kazimi, *Nuclear
Systems Volume I* (2nd ed., 2012), and Tong & Weisman, *Thermal Analysis of
Pressurized Water Reactors* (3rd ed., 1996).

## Correlation (original mixed English units, converted internally)

The W-3 uniform-flux DNB heat flux `q''_{DNB,EU}` in `BTU·hr⁻¹·ft⁻²` is
(`P` in psia, `G` in `lbm·hr⁻¹·ft⁻²`, `D_e` in inch, `x` `[-]`,
`Δh_in = h_f − h_in` in `BTU·lbm⁻¹`):

```text
q''/1e6 = { (2.022 − 0.0004302 P) + (0.1722 − 0.0000984 P)·exp[(18.177 − 0.004129 P)·x] }
         × [ (0.1484 − 1.596 x + 0.1729 x|x|)·G/1e6 + 1.037 ]·(1.157 − 0.869 x)
         × [ 0.2664 + 0.8357·exp(−3.151 D_e) ]·[ 0.8258 + 0.000794·Δh_in ]
```

The implementation converts SI inputs → these units, evaluates, and converts
the result `BTU·hr⁻¹·ft⁻²` → `W/m²` (`× 3.154591`). The inlet subcooling
`Δh_in = h_f − h_in` `[J/kg]` is stored on the struct (see [`W3::new`]); a
call-time override is available via [`ChfSubCoolModel`].

## Validity range (from the source)
- pressure `P`: 1000 – 2300 psia (6.9 – 15.9 MPa)
- mass flux `G`: 1.0e6 – 5.0e6 lbm·hr⁻¹·ft⁻² (≈ 1356 – 6800 kg·m⁻²·s⁻¹)
- equivalent diameter `D_e`: 0.2 – 0.7 inch (5.1 – 17.8 mm)
- quality `x`: −0.15 – +0.15 (subcooled to low quality)
- inlet enthalpy `h_in ≥ 400 BTU·lbm⁻¹`

```rust
pub struct W3 {
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
  pub fn new(subcooling: AvailableEnergy) -> Self { /* ... */ }
  ```
  Construct a W-3 model with a fixed inlet subcooling enthalpy.

- ```rust
  pub fn subcooling(self: &Self) -> AvailableEnergy { /* ... */ }
  ```
  The stored inlet subcooling enthalpy `Δh_in` `[J/kg]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ChfModel**
  - ```rust
    fn critical_heat_flux(self: &Self, pressure: Pressure, mass_flux: MassFlux, quality: f64, diameter: Length) -> Result<HeatFluxDensity, MultiphaseError> { /* ... */ }
    ```

  - ```rust
    fn in_valid_range(self: &Self, pressure: Pressure, mass_flux: MassFlux, quality: f64, diameter: Length) -> bool { /* ... */ }
    ```

- **ChfSubCoolModel**
  - ```rust
    fn critical_heat_flux_subcooled(self: &Self, pressure: Pressure, mass_flux: MassFlux, quality: f64, diameter: Length, subcooling: AvailableEnergy) -> Result<HeatFluxDensity, MultiphaseError> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Bowring`

**Bowring (1972)** round-tube, uniform-heat-flux dryout correlation for
water over 0.2 – 19 MPa.

Source: R.W. Bowring (1972), *"A simple but accurate round tube, uniform heat
flux, dryout correlation over the pressure range 0.7 to 17 MN/m²"*,
report AEEW-R789, UK Atomic Energy Authority, Winfrith. Form as reproduced in
Todreas & Kazimi, *Nuclear Systems Volume I* (2nd ed., 2012) and Collier &
Thome, *Convective Boiling and Condensation* (3rd ed., 1994).

## Correlation (SI throughout)

`q''_c = ( A − (1/4)·D·G·h_fg·x ) / C`  `[W/m²]`, with (`D` in m, `G` in
`kg·m⁻²·s⁻¹`, `h_fg` in `J/kg`, `q''` in `W/m²`)

```text
A = 2.317·(h_fg·D·G/4)·F1 / (1 + 0.0143·F2·D^{1/2}·G)
C = 0.077·F3·D·G / (1 + 0.347·F4·(G/1356)^n)
n = 2.0 − 0.5·p_R
```

where `p_R = 0.145·P` with `P` in **MPa** (so `p_R` is the pressure in units
of 1000 psi; `p_R = 1` at `P ≈ 6.895 MPa`). The pressure functions, for
`p_R ≤ 1`:

```text
F1     = ( p_R^{18.942}·exp[20.89(1−p_R)] + 0.917 ) / 1.917
F1/F2  = ( p_R^{1.316}·exp[2.444(1−p_R)] + 0.309 ) / 1.309
F3     = ( p_R^{17.023}·exp[16.658(1−p_R)] + 0.667 ) / 1.667
F4/F3  = p_R^{1.649}
```

and for `p_R > 1`:

```text
F1     = p_R^{-0.368}·exp[0.648(1−p_R)]
F1/F2  = p_R^{-0.448}·exp[0.245(1−p_R)]
F3     = p_R^{0.219}
F4/F3  = p_R^{1.649}
```

with `F2 = F1 / (F1/F2)` and `F4 = F3 · (F4/F3)`.

The latent heat `h_fg` at the system pressure is a required input, stored on
the struct at construction (see [`Bowring::new`]) from steam tables (public
literature data).

## Validity range (from the source)
- pressure `P`: 0.2 – 19 MPa (original fit 0.7 – 17 MN/m²)
- diameter `D`: 2 – 45 mm (0.002 – 0.045 m)
- mass flux `G`: 136 – 18 600 kg·m⁻²·s⁻¹

```rust
pub struct Bowring {
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
  pub fn new(h_fg: AvailableEnergy) -> Result<Self, MultiphaseError> { /* ... */ }
  ```
  Construct a Bowring model with the latent heat at the system pressure.

- ```rust
  pub fn h_fg(self: &Self) -> AvailableEnergy { /* ... */ }
  ```
  The stored latent heat `h_fg` `[J/kg]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ChfModel**
  - ```rust
    fn critical_heat_flux(self: &Self, pressure: Pressure, mass_flux: MassFlux, quality: f64, diameter: Length) -> Result<HeatFluxDensity, MultiphaseError> { /* ... */ }
    ```

  - ```rust
    fn in_valid_range(self: &Self, pressure: Pressure, mass_flux: MassFlux, _quality: f64, diameter: Length) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GroeneveldLut`

**Groeneveld 2006 CHF look-up-table** framework — tabulated critical heat
flux `q''_c(P, G, x)` for a vertical 8 mm water-cooled tube, with trilinear
interpolation and CSV import/export.

Source of the method: D.C. Groeneveld, J.Q. Shan, A.Z. Vasić, L.K.H. Leung,
A. Durmayaz, J. Yang, S.C. Cheng, A. Tanase (2007), *"The 2006 CHF look-up
table"*, Nuclear Engineering and Design **237**(15–17), 1909–1922. The LUT
gives CHF at discrete nodes over the axes
(pressure `P`, mass flux `G`, quality `x`) for a reference diameter of 8 mm;
values between nodes are obtained by **trilinear interpolation**, and a
diameter-correction factor scales the 8 mm value to other tube sizes.

## Data scope — honest note

The **full** published 2006 table is a large dataset (≈24 pressures × 20 mass
fluxes × 23 qualities). It is **not embedded here**: importing the complete
public LUT is a deferred data-acquisition step (respecting the workspace
`DATA_POLICY` — only properly sourced public literature data, with provenance
recorded). This type implements the table structure, the interpolation, and a
CSV loader/writer; the tests exercise them against a **small, synthetic,
illustrative** sample grid (round made-up numbers — *not* the copyrighted
Groeneveld values). Load the real table via [`from_csv`](Self::from_csv) once
it has been obtained and its provenance documented.

## Axes and storage

The three axis vectors must be **strictly ascending**. Table values are
stored row-major with index `[ip·(ng·nx) + ig·nx + ix]`, i.e. quality varies
fastest, then mass flux, then pressure. Units: `P` `[Pa]`, `G`
`[kg·m⁻²·s⁻¹]`, `x` `[-]`, stored CHF `[W/m²]`.

## Out-of-range behaviour

A query `(P, G, x)` outside the axis bounds is **clamped** to the nearest
bound on each axis before interpolation (documented, deterministic). This is
the boundary-value (no-extrapolation) convention;
[`in_valid_range`](ChfModel::in_valid_range) reports whether clamping
occurred.

[`from_csv`]: Self::from_csv

```rust
pub struct GroeneveldLut {
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
  pub fn new(pressures: Vec<f64>, mass_fluxes: Vec<f64>, qualities: Vec<f64>, chf: Vec<f64>, reference_diameter: Length) -> Result<Self, MultiphaseError> { /* ... */ }
  ```
  Build a look-up table from explicit axes and a row-major value array.

- ```rust
  pub fn n_nodes(self: &Self) -> usize { /* ... */ }
  ```
  Number of `(pressure, mass_flux, quality)` nodes in the table.

- ```rust
  pub fn reference_diameter(self: &Self) -> Length { /* ... */ }
  ```
  The table's reference tube diameter `[m]`.

- ```rust
  pub fn lookup(self: &Self, pressure_pa: f64, mass_flux: f64, quality: f64) -> f64 { /* ... */ }
  ```
  Raw trilinear-interpolated CHF `[W/m²]` for the **reference** 8 mm tube,

- ```rust
  pub fn diameter_factor(self: &Self, diameter_m: f64) -> f64 { /* ... */ }
  ```
  Groeneveld cylindrical-tube **diameter-correction factor** `K1`, scaling

- ```rust
  pub fn to_csv(self: &Self) -> String { /* ... */ }
  ```
  Serialise the table to CSV text (tidy / long format).

- ```rust
  pub fn from_csv(text: &str) -> Result<Self, MultiphaseError> { /* ... */ }
  ```
  Parse a table from CSV text produced by [`to_csv`](Self::to_csv) (tidy

- ```rust
  pub fn sample() -> Self { /* ... */ }
  ```
  A **small synthetic sample** LUT for demos/tests — **NOT** the real

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ChfModel**
  - ```rust
    fn critical_heat_flux(self: &Self, pressure: Pressure, mass_flux: MassFlux, quality: f64, diameter: Length) -> Result<HeatFluxDensity, MultiphaseError> { /* ... */ }
    ```
    Diameter-corrected CHF: `q''_c = K1(D)·lookup(P, G, x)`.

  - ```rust
    fn in_valid_range(self: &Self, pressure: Pressure, mass_flux: MassFlux, quality: f64, _diameter: Length) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `ChfModel`

Compiler-enforced contract every critical-heat-flux correlation satisfies.

A `ChfModel` maps the local flow state `(P, G, x, D)` to the critical heat
flux `q''_c`. It is used purely as a **contract** (the compiler checks each
concrete correlation implements it); runtime dispatch goes through the
[`ChfCorrelation`] enum, not a trait object, per the workspace no-`dyn` rule.

# Units
- `pressure` — system pressure `P` `[Pa]`.
- `mass_flux` — mass flux `G` `[kg·m⁻²·s⁻¹]`.
- `quality` — thermodynamic equilibrium quality `x` `[-]` (may be negative
  for subcooled conditions; must be `≤ 1`).
- `diameter` — heated / hydraulic diameter `D` `[m]`.
- returns critical heat flux `q''_c` `[W/m²]`.

```rust
pub trait ChfModel {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `critical_heat_flux`: Critical heat flux `q''_c` `[W/m²]` at the given local flow state.
- `in_valid_range`: Whether `(P, G, x, D)` lies inside the correlation's documented validity

##### Implementations

This trait is implemented for the following types:

- `ChfCorrelation`
- `Biasi`
- `W3`
- `Bowring`
- `GroeneveldLut`

#### Trait `ChfSubCoolModel`

Compiler-enforced contract for correlations that additionally account for
**inlet subcooling** explicitly at call time.

Most CHF correlations here are functions of the *local* state only; the
Westinghouse [`W3`] correlation carries an inlet-subcooling correction
factor. This trait exposes that dependence as an explicit call-time argument
(the subcooling enthalpy `Δh_in = h_f − h_in` `[J/kg]`), so a caller can
sweep subcooling without rebuilding the model.

```rust
pub trait ChfSubCoolModel {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `critical_heat_flux_subcooled`: Critical heat flux `q''_c` `[W/m²]` with an explicit inlet-subcooling

##### Implementations

This trait is implemented for the following types:

- `W3`

## Module `drift_flux`

Stage 1 — **Drift-flux mixture model foundation** (bead `op-2kk.1`).

Pure-Rust port of the mixture-property, algebraic-slip, and void-fraction
transport pieces of OpenFOAM's `incompressibleDriftFlux` solver module,
built on [`outram_foam_basic_lib`]'s finite-volume framework.

## Physics

The drift-flux model represents a two-phase mixture (a **dispersed** phase
`d` — e.g. gas bubbles or solid particles — carried in a **continuous**
phase `c` — e.g. liquid) with a *single* mixture momentum field plus a
transported dispersed-phase volume fraction `α`. The inter-phase slip is
not resolved by a second momentum equation (that is the Euler-Euler
Stage 2 job); instead it is closed **algebraically** by a relative- /
drift-velocity model. This foundation implements three ingredients:

1. **Mixture properties** ([`DriftFluxMixture`]) — mixture density
   `ρ_m = α·ρ_d + (1−α)·ρ_c` and a volume-weighted mixture viscosity.
2. **Algebraic slip closures** ([`SlipModel`]) — the per-cell drift
   velocity `U_dm` (dispersed-phase velocity relative to the mixture
   volumetric flux), as an enum (no `dyn` dispatch).
3. **Void-fraction transport** ([`DriftFlux::advance_alpha`]) — one
   implicit-Euler step of
   `∂α/∂t + ∇·(φ_m·α) + ∇·(φ_dm·α(1−α)) = 0`, clamped to `[0,1]`.

## Upstream provenance (ported C++ → Rust)

Source tree: `applications/modules/incompressibleDriftFlux/` of OpenFOAM-dev
(GPL-3.0). Exact file/line citations appear on each item below. Summary:

| Rust item | OpenFOAM source |
|---|---|
| [`DriftFluxMixture::rho_mixture`] | `incompressibleDriftFluxMixture.C:89` |
| [`DriftFluxMixture::mu_mixture`]  | `incompressibleDriftFluxMixture.C:90` + `mixtureViscosityModels/plastic/plastic.C:79` (concept) |
| [`SlipModel::TerminalVelocity`]   | `relativeVelocityModels/simple/simple.C:64` |
| [`SlipModel::UserDefined`]        | `relativeVelocityModels/general/general.C:65` (concept) |
| [`SlipModel::ZuberFindlay`]       | Zuber & Findlay (1965) — literature closure |
| [`DriftFlux::advance_alpha`]      | `alphaSuSp.C:32` (`alphaPhi`) + `twoPhaseSolver` `alphaEqn` |

## Honest scope — what is **NOT** modelled here

This is a **tested foundation**, not a converged coupled solver. Reviewers
and users must not read it as validated multiphase CFD. Specifically:

- **No mixture-momentum / pressure coupling.** There is no PIMPLE/PISO loop,
  no pressure Poisson solve, no `U_m` update. The mixture velocity `U_m`
  and the face flux `φ_m` are *inputs* the caller prescribes; `α` transport
  is advanced on that given flux. Coupling `U_m`–`p`–`α` is later work.
- **Simplified `α` bounding.** OpenFOAM bounds `α` with MULES (a flux-
  corrected, strictly conservative limiter). Here we use a plain post-solve
  **clamp to `[0,1]`**, which keeps `α` physical but is **not** globally
  conservative when the clamp is active. Documented as a known restriction.
- **Drift term treated explicitly.** The nonlinear compression flux
  `∇·(φ_dm·α(1−α))` is discretised explicitly (deferred correction, upwind
  on `α(1−α)`) and added as a source, not implicitly. Fine for small drift
  Courant numbers; can go unstable for large ones.
- **No turbulence coupling, no packing/dispersion, no non-Newtonian
  rheology.** OpenFOAM's `mixtureViscosityModels` (plastic, slurry,
  Bingham, Herschel-Bulkley, Quemada) and `packingDispersionModels` are
  not ported; the mixture viscosity is a single volume-weighted rule.
- **First-order in space and time.** Upwind convection + implicit Euler.
- **No `uom` on the `Vector3` velocity boundary.** Following the crate's
  `k_omega_sst` precedent, vector velocities are carried as `Vector3` in
  SI `m/s` (documented per field); `uom` types the *scalar* property inputs
  (density, viscosity) at the constructor boundary.

**Benchmark validation is a later, human-run step.** No benchmark
comparison has been performed; the tests below are *verification* checks
(formula exactness, hand-computed slip velocities, advection boundedness),
not *validation* against experimental or reference-solver data.

```rust
pub mod drift_flux { /* ... */ }
```

### Types

#### Type Alias `VoidFraction`

Dispersed-phase volume fraction `α`, dimensionless, valid range `[0, 1]`.

Carried as an [`outram_foam_basic_lib`] [`VolScalarField`] (per-cell `f64`).
This alias documents the physical meaning at the API boundary.

```rust
pub type VoidFraction = outram_foam_basic_lib::prelude::VolScalarField;
```

#### Struct `DriftFluxMixture`

Two-phase mixture material properties for the drift-flux model.

Holds the constant per-phase densities and dynamic viscosities of the
**dispersed** (`d`) and **continuous** (`c`) phases together with the
transported dispersed-phase volume-fraction field `α`. From these it forms
the cell-wise **mixture** density and viscosity used by the momentum and
transport equations.

Ports the state of OpenFOAM's `incompressibleDriftFluxMixture`
(`incompressibleDriftFluxMixture.C`). There, `rhod_`/`rhoc_` are constant
per-phase densities and `rho_ = alpha1()*rhod_ + alpha2()*rhoc_`
(`incompressibleDriftFluxMixture.C:89`), with `alpha2 = 1 − alpha1`.

## Units and valid ranges

- `ρ_d`, `ρ_c` — mass density `[kg/m³]`, must be **> 0**.
- `μ_d`, `μ_c` — dynamic viscosity `[Pa·s]`, must be **≥ 0**.
- `α` — dispersed volume fraction `[-]`, physical range `[0, 1]`.

Densities/viscosities are stored internally as `f64` in SI base units
(`kg/m³`, `Pa·s`); the constructor and scalar accessors use `uom` types so
the physical dimension is checked at the API boundary. Field-valued outputs
([`rho_mixture`](Self::rho_mixture), [`mu_mixture`](Self::mu_mixture)) are
`VolScalarField`s in those same SI units (documented, not `uom`-typed,
matching the finite-volume field convention used across this workspace).

```rust
pub struct DriftFluxMixture {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` | Finite-volume mesh the fields live on. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, rho_d: MassDensity, rho_c: MassDensity, mu_d: DynamicViscosity, mu_c: DynamicViscosity, alpha0: f64) -> Result<Self, MultiphaseError> { /* ... */ }
  ```
  Construct a mixture from per-phase properties and a uniform initial `α`.

- ```rust
  pub fn rho_dispersed(self: &Self) -> MassDensity { /* ... */ }
  ```
  Dispersed-phase density `ρ_d` `[kg/m³]`.

- ```rust
  pub fn rho_continuous(self: &Self) -> MassDensity { /* ... */ }
  ```
  Continuous-phase density `ρ_c` `[kg/m³]`.

- ```rust
  pub fn mu_dispersed(self: &Self) -> DynamicViscosity { /* ... */ }
  ```
  Dispersed-phase dynamic viscosity `μ_d` `[Pa·s]`.

- ```rust
  pub fn mu_continuous(self: &Self) -> DynamicViscosity { /* ... */ }
  ```
  Continuous-phase dynamic viscosity `μ_c` `[Pa·s]`.

- ```rust
  pub fn alpha(self: &Self) -> &VoidFraction { /* ... */ }
  ```
  The dispersed-phase volume-fraction field `α` `[-]` (read-only).

- ```rust
  pub fn alpha_mut(self: &mut Self) -> &mut VoidFraction { /* ... */ }
  ```
  The dispersed-phase volume-fraction field `α` `[-]` (mutable).

- ```rust
  pub fn rho_mixture(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Cell-wise **mixture density** `ρ_m = α·ρ_d + (1−α)·ρ_c` `[kg/m³]`.

- ```rust
  pub fn mu_mixture(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Cell-wise **mixture dynamic viscosity** `μ_m = α·μ_d + (1−α)·μ_c`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SlipModel`

Algebraic-slip closure for the dispersed-phase **drift velocity** `U_dm`.

`U_dm` is the velocity of the dispersed phase **relative to the mixture
volumetric flux** (OpenFOAM's `Udm`, `relativeVelocityModel.H`). It closes
the phase slip without a second momentum equation. All variants return a
per-cell `Vector3` in SI `m/s`.

Enum dispatch (not `dyn`) per the workspace design rules: the set of slip
closures is closed and known at compile time, so a `match` is exhaustive
and every variant is rust-analyzer-navigable.

## Variants
- [`ZuberFindlay`](Self::ZuberFindlay) — the classic distribution-parameter
  drift-flux correlation (Zuber & Findlay, 1965).
- [`TerminalVelocity`](Self::TerminalVelocity) — a constant terminal / slip
  velocity with a hindered-settling factor; ports OpenFOAM's `simple` model.
- [`UserDefined`](Self::UserDefined) — a caller-supplied per-cell `U_dm`
  field, for closures not covered above (mirrors the flexibility of
  OpenFOAM's `general` model).

```rust
pub enum SlipModel {
    ZuberFindlay {
        c0: f64,
        vgj: outram_foam_basic_lib::prelude::Vector3,
    },
    TerminalVelocity {
        u_t: outram_foam_basic_lib::prelude::Vector3,
        hindrance_exp: f64,
    },
    UserDefined(Vec<outram_foam_basic_lib::prelude::Vector3>),
}
```

##### Variants

###### `ZuberFindlay`

**Zuber-Findlay** drift-flux correlation (Zuber, N. & Findlay, J.A.,
1965, *"Average Volumetric Concentration in Two-Phase Flow Systems"*,
J. Heat Transfer **87**(4):453-468).

The dispersed-phase (gas) velocity is `v_d = C₀·j + V_gj`, where `j` is
the mixture volumetric flux (here approximated by the mixture cell
velocity `U_m`), `C₀` the **distribution parameter** `[-]` and `V_gj`
the **drift velocity** `[m/s]`. The drift velocity of the dispersed
phase *relative to the mixture flux* is then
`U_dm = v_d − j = (C₀ − 1)·U_m + V_gj`.

Typical values: `C₀ ≈ 1.0–1.2` (`1.13` churn-turbulent bubbly flow),
`V_gj` a small buoyant rise velocity `~0.1–0.3 m/s`. Valid for
low-to-moderate `α`; the constant-`C₀`/`V_gj` form degrades near
close-packing.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `c0` | `f64` | Distribution parameter `C₀` `[-]`. |
| `vgj` | `outram_foam_basic_lib::prelude::Vector3` | Drift velocity `V_gj` `[m/s]` (vector; typically aligned with, or<br>against, gravity). |

###### `TerminalVelocity`

**Terminal-velocity** closure with hindered settling —
`U_dm = u_t · 10^{−a·max(α,0)}`.

Ports the structure of OpenFOAM's `relativeVelocityModels::simple`
(`simple.C:64`), whose diffusion-velocity coefficient carries the
hindered-settling factor `pow(10, −a·max(αd,0))`. Here `u_t` `[m/s]` is
the unhindered terminal (buoyant rise / settling) velocity of an
isolated inclusion — it absorbs the upstream `(ρ_c/ρ_m)·V_c·|g|`
coefficient and direction into one vector — and `a ≥ 0` `[-]` is the
hindrance exponent (`a = 0` ⇒ no hindrance). As `α → 1` the drift
velocity is damped toward zero, mimicking crowding.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `u_t` | `outram_foam_basic_lib::prelude::Vector3` | Unhindered terminal / slip velocity `u_t` `[m/s]`. |
| `hindrance_exp` | `f64` | Hindered-settling exponent `a ≥ 0` `[-]` in `10^{−a·α}`. |

###### `UserDefined`

**User-defined** per-cell drift-velocity field `U_dm` `[m/s]`, one
`Vector3` per mesh cell.

For closures outside the two built-ins — e.g. a correlation evaluated
externally, or a tabulated field. Owns its captured state as a plain
`Vec<Vector3>` (no `dyn Fn`, per the workspace no-trait-object rule),
mirroring the arbitrary-coefficient flexibility of OpenFOAM's
`relativeVelocityModels::general` (`general.C:65`). The vector length
must equal the mesh cell count.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<outram_foam_basic_lib::prelude::Vector3>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn drift_velocity(self: &Self, u_m: &VolVectorField, alpha: &VolScalarField) -> Result<Vec<Vector3>, MultiphaseError> { /* ... */ }
  ```
  Per-cell drift velocity `U_dm` `[m/s]`, one `Vector3` per mesh cell.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DriftFlux`

Drift-flux model driver — owns the mixture, the slip closure, and the
prescribed mixture flow, and advances the void-fraction transport equation.

**Not a coupled solver.** The mixture velocity `u_m` and its face flux
`phi` are *inputs*: the caller sets them (from a momentum solve elsewhere,
or a prescribed field) each step, then calls
[`advance_alpha`](Self::advance_alpha) to march `α` one implicit-Euler step.
See the module-level "Honest scope" section for what this deliberately does
not do (no pressure coupling, MULES-free bounding, explicit drift term).

```rust
pub struct DriftFlux {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub mixture: DriftFluxMixture,
    pub slip: SlipModel,
    pub phi: outram_foam_basic_lib::prelude::SurfaceScalarField,
    pub u_m: outram_foam_basic_lib::prelude::VolVectorField,
    pub dt: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` | Finite-volume mesh (shared, `Arc`). |
| `mixture` | `DriftFluxMixture` | Mixture material properties and the `α` field. |
| `slip` | `SlipModel` | Algebraic slip closure for the drift velocity `U_dm`. |
| `phi` | `outram_foam_basic_lib::prelude::SurfaceScalarField` | Mixture volumetric face flux `φ_m = U_m·S_f` `[m³/s]`. Prescribe each<br>step (e.g. via [`outram_foam_basic_lib::fv_operators::fvc::flux`]). |
| `u_m` | `outram_foam_basic_lib::prelude::VolVectorField` | Mixture cell velocity `U_m` `[m/s]` (drives the slip closures). |
| `dt` | `f64` | Time step `Δt` `[s]`, must be `> 0`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mixture: DriftFluxMixture, slip: SlipModel) -> Self { /* ... */ }
  ```
  Construct a driver from a [`DriftFluxMixture`] and a [`SlipModel`].

- ```rust
  pub fn drift_velocity(self: &Self) -> Result<Vec<Vector3>, MultiphaseError> { /* ... */ }
  ```
  Per-cell drift velocity `U_dm` `[m/s]` from the current slip closure and

- ```rust
  pub fn advance_alpha(self: &mut Self) -> Result<(), MultiphaseError> { /* ... */ }
  ```
  Advance the dispersed-phase volume fraction `α` by one implicit-Euler

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `dryout`

Stage 5 — **Dryout & post-dryout framework** (bead `op-2kk.5`).

A **reserved interface layer** for the dryout / post-dryout boiling-crisis
regimes of a heated two-phase channel. This module deliberately ships the
*architecture* — traits as compiler-enforced contracts, plus `enum`
dispatch (no `dyn`) — with **one genuinely-implemented worked closure per
trait** to prove the interface is real, and every remaining regime honestly
flagged [`MultiphaseError::NotImplemented`]. It is **not** a validated
boiling-crisis model.

## The physical picture (what this framework will eventually cover)

In a heated channel carrying a boiling flow, once the wall heat flux or the
flow quality is high enough the continuous liquid contact with the wall
breaks down — the **boiling crisis**. Two limiting mechanisms:

- **Departure from nucleate boiling (DNB)** — at low quality / high flux,
  bubble crowding and vapour-blanket formation lift the liquid off the wall.
- **Dryout** — at high quality in annular flow, the liquid film on the wall
  is depleted by evaporation and entrainment until it vanishes.

Past that point the wall is cooled by a far less effective mechanism and its
temperature can excurse sharply. The downstream **post-dryout** heat-transfer
regimes are:

- **Film boiling** — a stable vapour film blankets the wall; heat crosses it
  by forced convection to the vapour (plus radiation at high `T_w`).
- **Transition boiling** — an unstable, intermittent mix of film and
  nucleate boiling between the critical-heat-flux point and the minimum-film-
  boiling point; the time-averaged flux *falls* with increasing `ΔT`.
- **Critical-heat-flux (CHF) recovery / rewetting (quench)** — the reverse
  transition, where a quench front re-establishes liquid–wall contact.

## What is genuinely implemented here (the two worked examples)

1. [`DryoutOnsetModel::CriticalVoidFraction`] — a simple **critical-void /
   critical-quality onset indicator** `α > α_crit`. Enough to exercise the
   [`DryoutModel`] contract with an exact hand-checkable result.
2. [`PostDryoutModel::FilmBoilingDougallRohsenow`] — the
   **Dougall–Rohsenow (1963)** post-dryout film-boiling HTC: Dittus–Boelter
   applied to the vapour phase flowing at the total mass flux with a
   density-ratio quality weighting. A real, dimensionally-consistent,
   literature-cited closure.

## Honest scope — what is **reserved (NOT implemented)**

Every other enum variant returns [`MultiphaseError::NotImplemented`] with a
message naming the physics a future implementer must supply. None of it is
faked or stubbed to a plausible-looking number:

- **DNB / critical-heat-flux onset** ([`DryoutOnsetModel::CriticalHeatFlux`])
  — bubble crowding, near-wall void build-up, vapour-blanket lift-off; needs
  a CHF look-up table or correlation (e.g. Groeneveld, W-3, Biasi).
- **Mechanistic liquid-film dryout**
  ([`DryoutOnsetModel::LiquidFilmDepletion`]) — the annular-flow film
  mass balance (evaporation + entrainment − deposition → film thickness → 0).
- **Mechanistic / radiative film boiling**
  ([`PostDryoutModel::FilmBoilingMechanistic`]) — vapour-film boundary-layer
  separation, interfacial waves, wall-to-interface radiation.
- **Transition boiling** ([`PostDryoutModel::TransitionBoiling`]) — the
  unstable film/nucleate interpolation between CHF and the minimum-film-
  boiling point.
- **CHF recovery / rewetting** ([`PostDryoutModel::ChfRecovery`]) — quench-
  front conduction–convection and the rewetting (Leidenfrost) temperature.

No benchmark validation has been performed. The tests below are
*verification* checks (formula exactness against an independently computed
value, enum-dispatch coverage, clean `NotImplemented` errors), **not**
*validation* against experimental boiling-crisis data.

```rust
pub mod dryout { /* ... */ }
```

### Types

#### Struct `DryoutConditions`

Local channel flow conditions consumed by a [`DryoutModel`] to decide
whether the boiling crisis (dryout / DNB) has occurred at a point.

## Fields, units and valid ranges
- `quality` — thermodynamic (or flow) vapour mass fraction `x` `[-]`,
  physical range `[0, 1]` (a [`Ratio`]).
- `void_fraction` — vapour volume fraction `α` `[-]`, physical range
  `[0, 1]` (a [`Ratio`]).
- `mass_flux` — total mixture mass flux `G` `[kg/(m²·s)]` (a [`MassFlux`]),
  `≥ 0`.
- `wall_temperature` — heated-wall temperature `T_w` `[K]` (a
  [`ThermodynamicTemperature`]); reserved for the CHF / film-boiling
  criteria that key off wall superheat.

```rust
pub struct DryoutConditions {
    pub quality: uom::si::f64::Ratio,
    pub void_fraction: uom::si::f64::Ratio,
    pub mass_flux: uom::si::f64::MassFlux,
    pub wall_temperature: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `quality` | `uom::si::f64::Ratio` | Vapour mass fraction (quality) `x` `[-]`, range `[0, 1]`. |
| `void_fraction` | `uom::si::f64::Ratio` | Vapour volume fraction (void) `α` `[-]`, range `[0, 1]`. |
| `mass_flux` | `uom::si::f64::MassFlux` | Total mixture mass flux `G` `[kg/(m²·s)]`, `≥ 0`. |
| `wall_temperature` | `uom::si::f64::ThermodynamicTemperature` | Heated-wall temperature `T_w` `[K]`. |

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
    fn clone(self: &Self) -> DryoutConditions { /* ... */ }
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `VaporProperties`

Vapour-phase thermophysical properties (plus the liquid density needed by
the density-ratio quality weighting) evaluated by the caller at the local
state — the closure inputs a post-dryout HTC correlation needs.

The caller is responsible for evaluating these at the appropriate reference
state (bulk-vapour or film temperature, per the correlation's basis) from a
property library; this framework does not compute properties.

## Fields, units and valid ranges
- `density` — vapour density `ρ_g` `[kg/m³]`, `> 0`.
- `liquid_density` — saturated-liquid density `ρ_f` `[kg/m³]`, `> 0`.
- `dynamic_viscosity` — vapour dynamic viscosity `μ_g` `[Pa·s]`, `> 0`.
- `thermal_conductivity` — vapour thermal conductivity `k_g` `[W/(m·K)]`,
  `> 0`.
- `prandtl` — vapour Prandtl number `Pr_g` `[-]` (a [`Ratio`]), `> 0`.

```rust
pub struct VaporProperties {
    pub density: uom::si::f64::MassDensity,
    pub liquid_density: uom::si::f64::MassDensity,
    pub dynamic_viscosity: uom::si::f64::DynamicViscosity,
    pub thermal_conductivity: uom::si::f64::ThermalConductivity,
    pub prandtl: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `uom::si::f64::MassDensity` | Vapour density `ρ_g` `[kg/m³]`, `> 0`. |
| `liquid_density` | `uom::si::f64::MassDensity` | Saturated-liquid density `ρ_f` `[kg/m³]`, `> 0`. |
| `dynamic_viscosity` | `uom::si::f64::DynamicViscosity` | Vapour dynamic viscosity `μ_g` `[Pa·s]`, `> 0`. |
| `thermal_conductivity` | `uom::si::f64::ThermalConductivity` | Vapour thermal conductivity `k_g` `[W/(m·K)]`, `> 0`. |
| `prandtl` | `uom::si::f64::Ratio` | Vapour Prandtl number `Pr_g` `[-]`, `> 0`. |

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
    fn clone(self: &Self) -> VaporProperties { /* ... */ }
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PostDryoutConditions`

Local channel flow conditions consumed by a [`PostDryoutModel`] to evaluate
the wall→fluid heat-transfer coefficient once the wall has dried out.

## Fields, units and valid ranges
- `mass_flux` — total mixture mass flux `G` `[kg/(m²·s)]`, `> 0`.
- `quality` — vapour mass fraction `x` `[-]`, range `[0, 1]`.
- `hydraulic_diameter` — channel hydraulic diameter `D_h` `[m]`, `> 0`.
- `wall_temperature` — heated-wall temperature `T_w` `[K]`; reserved for the
  wall-superheat–based transition / radiative film-boiling closures (the
  Dougall–Rohsenow worked example is a bulk-vapour correlation and does not
  use it).
- `vapor` — vapour-phase [`VaporProperties`] at the reference state.

```rust
pub struct PostDryoutConditions {
    pub mass_flux: uom::si::f64::MassFlux,
    pub quality: uom::si::f64::Ratio,
    pub hydraulic_diameter: uom::si::f64::Length,
    pub wall_temperature: uom::si::f64::ThermodynamicTemperature,
    pub vapor: VaporProperties,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mass_flux` | `uom::si::f64::MassFlux` | Total mixture mass flux `G` `[kg/(m²·s)]`, `> 0`. |
| `quality` | `uom::si::f64::Ratio` | Vapour mass fraction (quality) `x` `[-]`, range `[0, 1]`. |
| `hydraulic_diameter` | `uom::si::f64::Length` | Channel hydraulic diameter `D_h` `[m]`, `> 0`. |
| `wall_temperature` | `uom::si::f64::ThermodynamicTemperature` | Heated-wall temperature `T_w` `[K]`. |
| `vapor` | `VaporProperties` | Vapour-phase properties at the reference state. |

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
    fn clone(self: &Self) -> PostDryoutConditions { /* ... */ }
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DryoutIndicator`

Outcome of a dryout-onset evaluation: whether the wall has dried out, and a
dimensionless margin to the criterion.

`margin` is defined as **(actual − critical)** in whatever dimensionless
quantity the criterion uses (e.g. `α − α_crit`). It is therefore **negative
before onset**, zero at the criterion, and **positive after** the wall has
dried out. `dried_out == (margin > 0)`.

```rust
pub struct DryoutIndicator {
    pub dried_out: bool,
    pub margin: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dried_out` | `bool` | `true` once the dryout criterion is exceeded. |
| `margin` | `f64` | Dimensionless margin `actual − critical` `[-]`: `< 0` pre-onset,<br>`> 0` post-onset. |

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
    fn clone(self: &Self) -> DryoutIndicator { /* ... */ }
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
    fn eq(self: &Self, other: &DryoutIndicator) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `DryoutOnsetModel`

Dryout / boiling-crisis onset criteria, dispatched by `enum` (no `dyn`).

The set of onset criteria is closed and known at compile time, so a `match`
is exhaustive and every variant is rust-analyzer-navigable.

## Variants
- [`CriticalVoidFraction`](Self::CriticalVoidFraction) — **implemented**
  worked example: the `α > α_crit` critical-void / critical-quality
  indicator.
- [`CriticalHeatFlux`](Self::CriticalHeatFlux) — **reserved** (DNB/CHF
  look-up or correlation).
- [`LiquidFilmDepletion`](Self::LiquidFilmDepletion) — **reserved**
  (mechanistic annular-film mass balance).

```rust
pub enum DryoutOnsetModel {
    CriticalVoidFraction {
        alpha_crit: f64,
    },
    CriticalHeatFlux,
    LiquidFilmDepletion,
}
```

##### Variants

###### `CriticalVoidFraction`

**Critical-void-fraction onset (worked example).** Dryout is declared
when the local void fraction exceeds a critical value: `α > α_crit`.

This is the simplest defensible onset indicator and a common
annular-flow surrogate for film disappearance (in annular flow a high
void fraction corresponds to a thin liquid film). `alpha_crit` is the
critical void fraction `[-]` and must lie in `(0, 1]`.

The returned [`DryoutIndicator::margin`] is `α − α_crit`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `alpha_crit` | `f64` | Critical void fraction `α_crit` `[-]`, in `(0, 1]`. |

###### `CriticalHeatFlux`

**Critical-heat-flux (DNB) onset — reserved, not implemented.**

A future implementation supplies a CHF look-up table or correlation
(e.g. Groeneveld 2006 CHF table, the EPRI/W-3 correlation, or Biasi)
and compares the local heat flux against `q″_CHF(G, x, p, D)`. The
physics to add: near-wall bubble crowding and vapour-blanket lift-off
at low quality / high flux. Currently returns
[`MultiphaseError::NotImplemented`].

###### `LiquidFilmDepletion`

**Mechanistic liquid-film-depletion dryout — reserved, not
implemented.**

A future implementation solves the annular-flow liquid-film mass
balance (evaporation + entrainment − deposition) and declares dryout
when the film thickness reaches zero. The physics to add: the
entrainment/deposition closure and the film-flow-rate ODE. Currently
returns [`MultiphaseError::NotImplemented`].

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
    fn clone(self: &Self) -> DryoutOnsetModel { /* ... */ }
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

- **DryoutModel**
  - ```rust
    fn dryout_onset(self: &Self, conditions: &DryoutConditions) -> Result<DryoutIndicator, MultiphaseError> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `PostDryoutModel`

Post-dryout heat-transfer regimes, dispatched by `enum` (no `dyn`).

The set of post-dryout regimes is closed and known at compile time, so a
`match` is exhaustive and every variant is rust-analyzer-navigable.

## Variants
- [`FilmBoilingDougallRohsenow`](Self::FilmBoilingDougallRohsenow) —
  **implemented** worked example: the Dougall–Rohsenow (1963) vapour-phase
  film-boiling HTC.
- [`FilmBoilingMechanistic`](Self::FilmBoilingMechanistic) — **reserved**
  (vapour-film boundary layer + radiation).
- [`TransitionBoiling`](Self::TransitionBoiling) — **reserved**.
- [`ChfRecovery`](Self::ChfRecovery) — **reserved** (rewetting / quench).

```rust
pub enum PostDryoutModel {
    FilmBoilingDougallRohsenow,
    FilmBoilingMechanistic,
    TransitionBoiling,
    ChfRecovery,
}
```

##### Variants

###### `FilmBoilingDougallRohsenow`

**Dougall–Rohsenow (1963) film-boiling HTC (worked example).**

The post-dryout vapour-phase forced-convection correlation: apply the
Dittus–Boelter form `Nu = 0.023 Re^0.8 Pr^0.4` to the vapour flowing at
the *total* mass flux, using a density-ratio quality weighting for the
Reynolds number.

### Formulae
Reynolds number (vapour reference, Dougall–Rohsenow weighting):

`Re = (G · D_h / μ_g) · [ x + (ρ_g / ρ_f)·(1 − x) ]`

Nusselt number (Dittus–Boelter, heating exponent `0.4`):

`Nu = 0.023 · Re^0.8 · Pr_g^0.4`

Heat-transfer coefficient:

`h = Nu · k_g / D_h`   `[W/(m²·K)]`

where `G` is the total mass flux, `D_h` the hydraulic diameter, `x` the
quality, `ρ_g`/`ρ_f` the vapour/liquid densities, and `μ_g`, `k_g`,
`Pr_g` the vapour viscosity, conductivity and Prandtl number.

### Validity / limitations
A single-phase-vapour convective estimate. It ignores wall-to-interface
**radiation** (matters at high `T_w`), thermodynamic **non-equilibrium**
(actual vapour superheat / droplet content), and **transition-boiling**
effects near CHF. Reasonable at moderate-to-high quality in dispersed-
flow film boiling; degrades near the dryout front and at very high wall
superheat. `wall_temperature` is unused by this correlation.

###### `FilmBoilingMechanistic`

**Mechanistic / radiative film boiling — reserved, not implemented.**

A future implementation resolves the vapour-film boundary layer and adds
a wall→interface radiation path (and, for dispersed-flow film boiling,
wall→droplet transfer). Physics to add: boundary-layer separation,
interfacial waves, non-equilibrium vapour superheat. Currently returns
[`MultiphaseError::NotImplemented`].

###### `TransitionBoiling`

**Transition boiling — reserved, not implemented.**

A future implementation interpolates the unstable film/nucleate regime
between the critical-heat-flux point and the minimum-film-boiling point,
where the time-averaged flux *decreases* with increasing wall superheat.
Physics to add: the CHF and minimum-film-boiling anchor points and the
intermittent-contact fraction. Currently returns
[`MultiphaseError::NotImplemented`].

###### `ChfRecovery`

**CHF recovery / rewetting (quench) — reserved, not implemented.**

A future implementation models the quench front that re-establishes
liquid–wall contact: conduction–convection ahead of the front and the
rewetting (Leidenfrost/minimum-film-boiling) temperature. Physics to
add: the moving quench-front energy balance. Currently returns
[`MultiphaseError::NotImplemented`].

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
    fn clone(self: &Self) -> PostDryoutModel { /* ... */ }
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

- **PostDryoutHeatTransfer**
  - ```rust
    fn heat_transfer_coefficient(self: &Self, conditions: &PostDryoutConditions) -> Result<HeatTransfer, MultiphaseError> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `DryoutModel`

Compiler-enforced contract for a **dryout / boiling-crisis onset** criterion.

A model maps local [`DryoutConditions`] to a [`DryoutIndicator`] telling the
caller whether the wall has dried out and by what margin. This trait exists
to make the interface uniform and to let the compiler check every concrete
model implements it; runtime dispatch goes through the [`DryoutOnsetModel`]
enum (no `dyn`), per the workspace design rules.

```rust
pub trait DryoutModel {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `dryout_onset`: Evaluate the dryout-onset criterion at the given local conditions.

##### Implementations

This trait is implemented for the following types:

- `DryoutOnsetModel`

#### Trait `PostDryoutHeatTransfer`

Compiler-enforced contract for a **post-dryout wall heat-transfer** closure.

A model maps local [`PostDryoutConditions`] to the wall→fluid heat-transfer
coefficient [`HeatTransfer`] `[W/(m²·K)]` in the post-dryout regime. As with
[`DryoutModel`], the trait fixes the interface and the compiler checks each
concrete model; runtime dispatch goes through the [`PostDryoutModel`] enum
(no `dyn`).

```rust
pub trait PostDryoutHeatTransfer {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `heat_transfer_coefficient`: Wall→fluid heat-transfer coefficient `h` `[W/(m²·K)]` in the post-dryout

##### Implementations

This trait is implemented for the following types:

- `PostDryoutModel`

## Module `pimple`

**Drift-flux mixture PISO/PIMPLE pressure-velocity coupling** — the
foundation of the segregated, incompressible-mixture momentum + pressure
solve that [`crate::drift_flux`] deliberately left out (its "Honest scope"
note: *"No mixture-momentum / pressure coupling."*). This module closes that
#1 gap with a real, tested Rhie-Chow pressure-correction loop.

## What it solves

One time step of the single-field **mixture** momentum equation coupled to a
pressure Poisson equation enforcing discrete incompressibility
`∇·U_m = 0`, then advances the dispersed-phase void fraction `α` on the
freshly corrected mixture flux via [`DriftFlux::advance_alpha`]:

```text
∂U_m/∂t + ∇·(φ_m U_m) − ∇·(ν_eff,m ∇U_m) = −(1/ρ_m)∇p + g      (momentum)
∇·( (r_AU/ρ_m) ∇p ) = ∇·φ_HbyA                                  (pressure)
φ_m = φ_HbyA − (r_AU/ρ_m)_f |S_f| ∂p/∂n ,   U_m = reconstruct(φ_m)         (correction)
∂α/∂t + ∇·(φ_m α) + ∇·(φ_dm α(1−α)) = 0                          (void transport)
```

where `ρ_m`, `ν_eff,m = μ_m/ρ_m` are the cell-wise mixture density `[kg/m³]`
and kinematic viscosity `[m²/s]` from [`DriftFluxMixture`], `p` is the
**dynamic** (mechanical) pressure `[Pa]`, `g` the gravitational acceleration
`[m/s²]`, `HbyA = H/A` the momentum "off-diagonal / reciprocal-diagonal"
operator and `r_AU = V/A` its reciprocal diagonal.

## Units and valid ranges

- `U_m` — mixture velocity `[m/s]`, `Vector3` per cell (SI, not `uom`-typed,
  matching the finite-volume field convention across this workspace).
- `p` — dynamic pressure `[Pa]`, `VolScalarField`, defined up to an additive
  constant (pinned at a reference cell in a closed / velocity-driven domain).
- `φ_m` — mixture volumetric face flux `[m³/s]`, `SurfaceScalarField`.
- `g` — gravitational acceleration `[m/s²]`, any direction; `Vector3::ZERO`
  for a gravity-free case.
- `α` — dispersed volume fraction `[-]`, kept in `[0,1]` by the transport
  clamp.
- `dt` — time step `[s]`, must be `> 0`.

## Upstream provenance (ported C++ → Rust)

The loop follows OpenFOAM's `incompressibleFluid` solver module — the
incompressible base that `incompressibleDriftFlux` (a `twoPhaseSolver`)
reuses for its pressure-velocity coupling. Exact file/line citations:

| Rust step | OpenFOAM source |
|---|---|
| [`DriftFluxPimple::momentum_predictor`] | `incompressibleFluid/momentumPredictor.C:33` (`fvm::ddt(U)+fvm::div(phi,U)+… == -fvc::grad(p)`) |
| [`DriftFluxPimple::pressure_corrector`] | `incompressibleFluid/correctPressure.C:38` (`rAU=1/UEqn.A()`, `HbyA=rAU*UEqn.H()`, `phiHbyA=fvc::flux(HbyA)`, `fvm::laplacian(rAtU,p)==fvc::div(phiHbyA)`, `phi=phiHbyA-pEqn.flux()`, `U=HbyA-rAtU*fvc::grad(p)`) |
| mixture `ρ_m`, `μ_m` in `ν_eff,m` | `incompressibleDriftFluxMixture.C:89-90` (via [`DriftFluxMixture`]) |
| `α` advance on corrected `φ_m` | `incompressibleDriftFlux` `alphaPredictor` / `twoPhaseSolver` `alphaEqn` (via [`DriftFlux::advance_alpha`]) |

## Honest scope — what is **NOT** modelled here

This is a **tested foundation**, not a validated multiphase CFD solver.
Reviewers and users must not read it as production-ready. Specifically:

- **Segregated, incompressible-mixture only.** A single mixture-momentum
  field with a Rhie-Chow pressure correction; there is no per-phase momentum
  (that is the Euler-Euler Stage 2 job) and no compressibility / energy
  coupling. The phase slip is carried purely by the algebraic drift closure
  inside `α` transport, and is **not** fed back into the mixture-momentum
  drift-stress term `∇·(α ρ_d/ρ_m U_dm U_dm)` that OpenFOAM's `UEqn` adds —
  omitting that term is a documented simplification of the mixture momentum
  balance.
- **Leading-order `HbyA`.** `HbyA` is formed from the momentum matrix's
  snapshotted `H`-source (ddt-old + body force + BC terms); the convection /
  diffusion off-diagonal-times-`U*` contribution is not re-folded each PISO
  sweep. This is the standard segregated-PISO leading term and is exact for
  the at-rest and hydrostatic cases (off-diagonals act on `U*→0`); on a
  strongly advecting flow it converges as the outer loop iterates but is not
  the full `UEqn.H()`. Documented restriction.
- **First order in space and time.** Implicit-Euler `ddt`, first-order
  upwind convection (`fvm::div`), Gauss-orthogonal Laplacian. No higher-order
  or TVD convection, no second-order time.
- **No non-orthogonal correction.** The pressure Laplacian is the orthogonal
  part only (single non-orthogonal corrector); skewed / non-orthogonal meshes
  lose accuracy. No `fvc::ddtCorr` Rhie-Chow time-derivative flux correction
  is added, so on strongly transient coarse meshes some pressure-velocity
  decoupling ("checkerboarding") can appear.
- **Laminar mixture viscosity.** `ν_eff,m = μ_m/ρ_m` with the volume-weighted
  mixture `μ_m`; **no turbulence model** is coupled (no `divDevReff` eddy
  viscosity, no `k`-`ω`). Turbulence coupling is later work.
- **No MULES.** `α` is bounded by the plain post-solve clamp inherited from
  [`DriftFlux::advance_alpha`] (not strictly conservative when the clamp
  bites), and no packing/dispersion or non-Newtonian rheology is modelled.
- **Pressure reference pinned.** The pressure is pinned at one reference cell
  (`set_reference`), assuming an all-Neumann (closed / velocity-driven)
  pressure field; a fixed-pressure outlet BC is not specially handled.
- **Boundary flux from `φ_HbyA`.** Corrected boundary face fluxes are taken
  from `φ_HbyA` (exact `0` on no-slip walls); no boundary non-orthogonal
  pressure-flux correction is applied.

**Benchmark validation is a later, human-run step.** No benchmark comparison
(lid-driven cavity, bubble column, dam-break, …) has been performed. The
tests below are *verification* checks — hydrostatic balance, at-rest
stability, finiteness/boundedness, `α ∈ [0,1]` — **not** *validation* against
experimental or reference-solver data. Nothing here is validated multiphase
CFD and it must not be described as such.

[`DriftFluxMixture`]: crate::drift_flux::DriftFluxMixture

```rust
pub mod pimple { /* ... */ }
```

### Types

#### Type Alias `Pressure`

Dynamic (mechanical) pressure field `p` `[Pa]`, defined up to an additive
constant. Carried as a [`VolScalarField`]; this alias documents the physical
meaning at the API boundary.

```rust
pub type Pressure = outram_foam_basic_lib::prelude::VolScalarField;
```

#### Struct `DriftFluxPimple`

Segregated PISO/PIMPLE pressure-velocity coupler for the drift-flux mixture.

Owns a [`DriftFlux`] driver (which itself owns the [`DriftFluxMixture`], the
slip closure, the mixture velocity `U_m`, and the mixture face flux `φ_m`),
plus the dynamic pressure field and the loop controls. One call to
[`solve_timestep`](Self::solve_timestep) advances the coupled
`U_m`–`p`–`α` system by one time step.

[`DriftFluxMixture`]: crate::drift_flux::DriftFluxMixture

## Field ownership (per the workspace design rules)

All fields are owned **by value**; the mesh is shared with `Arc<FvMesh>`;
cells and faces are indexed by `usize`. No `Box<dyn>`/`dyn`, no lifetime
parameters, no channels.

## Setting up a case

After construction, set the boundary conditions and any driving flow on the
owned [`DriftFlux`] (`self.drift.u_m` velocity BCs, `self.drift.mixture`
`α` BCs), the gravity vector [`gravity`](Self::gravity), and the corrector
counts, then call [`solve_timestep`](Self::solve_timestep) each step.

```rust
pub struct DriftFluxPimple {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub drift: crate::drift_flux::DriftFlux,
    pub p: Pressure,
    pub gravity: outram_foam_basic_lib::prelude::Vector3,
    pub n_correctors: usize,
    pub n_outer_correctors: usize,
    pub p_ref_cell: usize,
    pub p_ref_value: f64,
    pub solver_settings: outram_foam_basic_lib::prelude::SolverSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` | Finite-volume mesh (shared, `Arc`). |
| `drift` | `crate::drift_flux::DriftFlux` | Drift-flux driver: owns the mixture (`ρ_m`, `μ_m`, `α`), the slip<br>closure, the mixture velocity `U_m` `[m/s]`, and the mixture face flux<br>`φ_m` `[m³/s]`. The momentum/pressure loop writes `U_m` and `φ_m`; the<br>`α` advance reads them. |
| `p` | `Pressure` | Dynamic pressure `p` `[Pa]` (see [`Pressure`]). |
| `gravity` | `outram_foam_basic_lib::prelude::Vector3` | Gravitational acceleration `g` `[m/s²]`. `Vector3::ZERO` disables gravity. |
| `n_correctors` | `usize` | Number of PISO pressure correctors per outer iteration (`nCorrectors`).<br>Must be `≥ 1`; `2` is a common default. |
| `n_outer_correctors` | `usize` | Number of outer PIMPLE iterations per time step (`nOuterCorrectors`).<br>Must be `≥ 1`; `1` recovers plain PISO. |
| `p_ref_cell` | `usize` | Reference cell whose pressure is pinned to [`p_ref_value`](Self::p_ref_value)<br>(fixes the otherwise-singular all-Neumann pressure matrix). |
| `p_ref_value` | `f64` | Value the reference cell's pressure is pinned to `[Pa]`. |
| `solver_settings` | `outram_foam_basic_lib::prelude::SolverSettings` | Linear-solver settings shared by the momentum and pressure solves. |

##### Implementations

###### Methods

- ```rust
  pub fn new(drift: DriftFlux) -> Self { /* ... */ }
  ```
  Construct a coupler around a [`DriftFlux`] driver.

- ```rust
  pub fn solve_timestep(self: &mut Self, dt: f64) -> Result<(), MultiphaseError> { /* ... */ }
  ```
  Advance the coupled mixture momentum, pressure, and void fraction by one

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `two_fluid`

Stage 2 — **Euler-Euler two-fluid model foundation** (bead `op-2kk.2`).

Pure-Rust port of the phase / interfacial-momentum-transfer pieces of
OpenFOAM's `multiphaseEuler` solver family (the `phaseSystem`
interfacial-models layer), built on [`outram_foam_basic_lib`]'s
finite-volume framework.

## Physics

The Euler-Euler ("two-fluid") model treats each phase as an
**interpenetrating continuum** with *its own* volume fraction, velocity, and
momentum balance — unlike the Stage-1 drift-flux model ([`crate::drift_flux`])
which collapses the two phases onto a single mixture momentum plus an
algebraic slip. Here a **dispersed** phase `d` (bubbles / droplets /
particles) and a **continuous** phase `c` (the carrier fluid) each carry a
fraction `α_k` and a velocity `U_k`, coupled through **interfacial momentum
transfer** — chiefly interphase drag. This foundation implements:

1. **Per-phase representation** ([`Phase`]) — the fields `α_k`, `U_k` plus
   constant density, viscosity and (for the dispersed phase) an inclusion
   diameter.
2. **A two-phase system** ([`TwoFluidSystem`]) — a dispersed + continuous
   [`Phase`] on a shared [`FvMesh`], holding the saturation constraint
   `α_d + α_c = 1` ([`TwoFluidSystem::enforce_alpha_constraint`]).
3. **Interfacial force closures as an enum** ([`DragModel`], wrapped by the
   extensible [`InterfacialForce`]) — the per-cell volumetric drag
   coefficient `K_d` `[kg/(m³·s)]` from Schiller-Naumann, Wen-Yu, or a
   prescribed constant. Lift / virtual-mass / wall-lubrication /
   turbulent-dispersion are **documented scaffolds** (see honest scope).
4. **Per-phase continuity** ([`TwoFluidSystem::advance_dispersed_alpha`]) —
   one implicit-Euler step of `∂α_d/∂t + ∇·(φ_d·α_d) = 0` on the dispersed
   phase's own flux, clamped to `[0,1]`, with `α_c` re-derived from the
   saturation constraint.

## Upstream provenance (ported C++ → Rust)

Source tree: `applications/modules/multiphaseEuler/phaseSystem/` of
OpenFOAM-dev (GPL-3.0). Exact file/line citations appear on each item.
Summary:

| Rust item | OpenFOAM source |
|---|---|
| [`DragModel::SchillerNaumann`] `CdRe` | `interfacialModels/dragModels/SchillerNaumann/SchillerNaumann.C:62` |
| [`DragModel::WenYu`] `CdRe` | `interfacialModels/dragModels/WenYu/WenYu.C:62` |
| [`DragModel::k_d`] (`Ki`/`K`) | `interfacialModels/dragModels/dispersedDragModel/dispersedDragModel.C:58` (`Ki`), `:70` (`K`) |
| [`TwoFluidSystem::reynolds_number`] | `phaseInterface/dispersedPhaseInterface/dispersedPhaseInterface.C:136` (`Re`) + `phaseInterface/phaseInterface/phaseInterface.C:545` (`magUr`) |
| [`TwoFluidSystem::advance_dispersed_alpha`] | `phaseSystem/phaseSystem/phaseSystemSolve.C:336` (`fvScalarMatrix alphaEqn`) |

## Honest scope — what is **NOT** modelled here

This is a **tested foundation**, not a converged coupled two-fluid solver.
Reviewers and users must not read it as validated multiphase CFD.
Specifically:

- **No phase-momentum / pressure-coupled PIMPLE loop.** There is no
  `U_d`/`U_c`/`p` solve, no shared pressure Poisson equation, no
  partial-elimination (PEA) drag coupling. The phase velocities `U_k` are
  *inputs* the caller prescribes; only the dispersed-phase continuity
  (`α_d` transport) is advanced, on the given `U_d`. Assembling and solving
  the two coupled momentum equations with the drag `K_d` computed here is
  later work.
- **Drag is the only implemented interfacial force.** Lift, virtual (added)
  mass, wall lubrication, and turbulent dispersion are **documented scaffold
  variants** ([`InterfacialForce`]) that return
  [`MultiphaseError::NotImplemented`] — the API shape is present so the
  architecture extends to a full 6-equation system, but the closures are
  **not** ported and must not be treated as available.
- **Constant per-phase properties, constant diameter.** `ρ_k`, `μ_k` and the
  dispersed diameter `d` are constants; no thermo, no population balance / no
  bubble-size distribution, no swarm correction (OpenFOAM's `Cs` is taken as
  `1`, matching its `noSwarm` default).
- **Simplified `α` bounding.** OpenFOAM bounds phase fractions with MULES (a
  flux-corrected, strictly conservative limiter). Here `α_d` is bounded by a
  plain post-solve **clamp to `[0,1]`**, which keeps it physical but is
  **not** globally conservative when the clamp is active. `α_c` is then set
  by `α_c = 1 − α_d`, so the saturation constraint holds exactly but shares
  the clamp's (small) non-conservation.
- **First-order in space and time.** Upwind convection + implicit Euler.
- **No `uom` on the `Vector3` velocity boundary.** Following the crate's
  `drift_flux` / `k_omega_sst` precedent, vector velocities are carried as
  `Vector3` in SI `m/s` (documented per field); `uom` types the *scalar*
  property inputs (density, viscosity, diameter) at the constructor boundary.

**Benchmark validation is a later, human-run step.** No benchmark comparison
has been performed; the tests below are *verification* checks (formula
exactness against hand-computed Reynolds-number drag cases, the saturation
constraint, advection boundedness), not *validation* against experimental or
reference-solver data.

```rust
pub mod two_fluid { /* ... */ }
```

### Types

#### Type Alias `PhaseFraction`

Per-phase volume fraction `α_k`, dimensionless, valid range `[0, 1]`.

Carried as an [`outram_foam_basic_lib`] [`VolScalarField`] (per-cell `f64`).
This alias documents the physical meaning at the API boundary.

```rust
pub type PhaseFraction = outram_foam_basic_lib::prelude::VolScalarField;
```

#### Struct `Phase`

One phase of a Euler-Euler two-fluid system: its volume-fraction field
`α_k`, velocity field `U_k`, and constant material properties.

Ports the state a `Foam::phaseModel` exposes to the interfacial-momentum
closures — a volume fraction (`phase()`), a velocity (`U()`), a density
(`rho()`), a kinematic viscosity (`fluidThermo().nu()`), and, for a
dispersed phase, a diameter (`d()`). Here those properties are **constants**
(incompressible, isothermal foundation); thermo and size distributions are
out of scope (see the module "Honest scope" section).

## Units and valid ranges

- `α_k` — volume fraction `[-]`, physical range `[0, 1]`.
- `U_k` — velocity `[m/s]`, carried as `Vector3` in SI (not `uom`-typed).
- `ρ_k` — mass density `[kg/m³]`, must be **> 0**.
- `μ_k` — dynamic viscosity `[Pa·s]`, must be **≥ 0**.
- `d`  — characteristic inclusion diameter `[m]`, must be **> 0**. Physically
  meaningful only when this phase acts as the *dispersed* phase in a drag
  closure; a continuous phase may carry any positive placeholder.

Scalar properties are stored internally as `f64` in SI base units; the
constructor and scalar accessors use `uom` types so the physical dimension
is checked at the API boundary.

```rust
pub struct Phase {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` | Finite-volume mesh the fields live on (shared, `Arc`). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(mesh: Arc<FvMesh>, name: impl Into<String>, rho: MassDensity, mu: DynamicViscosity, d: Length, alpha0: f64) -> Result<Self, MultiphaseError> { /* ... */ }
  ```
  Construct a phase from constant properties and a uniform initial `α_k`.

- ```rust
  pub fn name(self: &Self) -> &str { /* ... */ }
  ```
  Phase name.

- ```rust
  pub fn rho(self: &Self) -> MassDensity { /* ... */ }
  ```
  Density `ρ_k` `[kg/m³]`.

- ```rust
  pub fn mu(self: &Self) -> DynamicViscosity { /* ... */ }
  ```
  Dynamic viscosity `μ_k` `[Pa·s]`.

- ```rust
  pub fn nu(self: &Self) -> KinematicViscosity { /* ... */ }
  ```
  Kinematic viscosity `ν_k = μ_k / ρ_k` `[m²/s]`.

- ```rust
  pub fn diameter(self: &Self) -> Length { /* ... */ }
  ```
  Characteristic inclusion diameter `d` `[m]` (dispersed-phase property).

- ```rust
  pub fn alpha(self: &Self) -> &PhaseFraction { /* ... */ }
  ```
  The volume-fraction field `α_k` `[-]` (read-only).

- ```rust
  pub fn alpha_mut(self: &mut Self) -> &mut PhaseFraction { /* ... */ }
  ```
  The volume-fraction field `α_k` `[-]` (mutable) — set the initial

- ```rust
  pub fn u(self: &Self) -> &VolVectorField { /* ... */ }
  ```
  The velocity field `U_k` `[m/s]` (read-only).

- ```rust
  pub fn u_mut(self: &mut Self) -> &mut VolVectorField { /* ... */ }
  ```
  The velocity field `U_k` `[m/s]` (mutable) — prescribe the phase velocity

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `DragModel`

Interphase **drag** closure returning the per-cell volumetric drag
coefficient `K_d` `[kg/(m³·s)]`.

`K_d` is the coefficient in the interfacial drag force `K_d·(U_c − U_d)` that
appears (with opposite signs) in the two phase-momentum equations
(`dragModel.H`: *"`ddt(alpha1*rho1*U1) + … = … K*(U1-U2)`"*). Enum dispatch
(not `dyn`) per the workspace design rules: the set of drag closures is
closed and known at compile time, so a `match` is exhaustive and every
variant is rust-analyzer-navigable.

## Coefficient construction (ported)

For the correlation-based variants the coefficient follows OpenFOAM's
dispersed-drag chain (`dispersedDragModel.C`):

- the model supplies `CdRe = C_d·Re` (a Reynolds-number-scaled drag
  coefficient), then
- `Ki = 0.75·CdRe·C_s·ρ_c·ν_c / d²` (`dispersedDragModel.C:58`, with the
  swarm correction `C_s = 1`, matching the `noSwarm` default), and
- `K_d = max(α_d, α_res)·Ki` (`dispersedDragModel.C:70`).

where `Re = |U_d − U_c|·d/ν_c` (see [`TwoFluidSystem::reynolds_number`]).

## Variants
- [`SchillerNaumann`](Self::SchillerNaumann) — standard dispersed-bubble /
  sphere correlation.
- [`WenYu`](Self::WenYu) — Wen-Yu correlation with a `α_c^{−2.65}` voidage
  correction (dense particle beds / high dispersed loading).
- [`Constant`](Self::Constant) — a prescribed, spatially-uniform `K_d`, for
  testing and for cases where a coefficient is supplied externally.

```rust
pub enum DragModel {
    SchillerNaumann,
    WenYu,
    Constant {
        k_d: f64,
    },
}
```

##### Variants

###### `SchillerNaumann`

**Schiller-Naumann** drag for dispersed bubbly / particulate flow
(Schiller & Naumann, 1935). Ports `SchillerNaumann::CdRe()`
(`SchillerNaumann.C:62`):

```text
CdRe = Re < 1000 ? 24·(1 + 0.15·Re^0.687) : 0.44·Re
```

Valid for a single inclusion in an unbounded fluid; the dilute limit of
the [`WenYu`](Self::WenYu) form (`α_c = 1`).

###### `WenYu`

**Wen-Yu** drag (Wen & Yu, 1966) — Schiller-Naumann evaluated on a
voidage-scaled Reynolds number with a `α_c^{−2.65}` hindrance amplifier.
Ports `WenYu::CdRe()` (`WenYu.C:62`):

```text
α₂   = max(α_c, α_res)
Res  = α₂·Re
CdRe = [ Res < 1000 ? 24·(1 + 0.15·Res^0.687) : 0.44·Res ] · α₂^(−2.65)
```

Reduces to [`SchillerNaumann`](Self::SchillerNaumann) as `α_c → 1`
(dilute dispersed phase). Suited to higher dispersed loadings.

###### `Constant`

**Constant** prescribed volumetric drag coefficient `K_d` `[kg/(m³·s)]`,
spatially uniform. Not a physical correlation — a fixed value for
verification tests and for coefficients supplied by an external model.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `k_d` | `f64` | The uniform drag coefficient `K_d` `[kg/(m³·s)]`, must be `≥ 0`. |

##### Implementations

###### Methods

- ```rust
  pub fn k_d(self: &Self, system: &TwoFluidSystem) -> Result<VolScalarField, MultiphaseError> { /* ... */ }
  ```
  Per-cell volumetric drag coefficient `K_d` `[kg/(m³·s)]` for the given

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `InterfacialForce`

Interfacial momentum-transfer force closure — the extensible enum from which
a full 6-equation two-fluid system draws its phase-coupling terms.

**Only [`Drag`](Self::Drag) is implemented** at this foundation stage. The
remaining variants are **documented scaffolds**: they exist so the API and
the dispatch `match` already have the right shape for the 6-equation
extension noted in the crate roadmap, but their closures are **not** ported
and their [`momentum_coefficient`](Self::momentum_coefficient) arm returns
[`MultiphaseError::NotImplemented`]. They must not be treated as available.

Enum dispatch (not `dyn`) per the workspace design rules.

## Variants and their (future) OpenFOAM sources
- [`Drag`](Self::Drag) — **implemented**, see [`DragModel`].
- [`Lift`](Self::Lift) — scaffold; ports from
  `interfacialModels/liftModels/` (Tomiyama, Moraga, constant …).
- [`VirtualMass`](Self::VirtualMass) — scaffold;
  `interfacialModels/virtualMassModels/` (constant-coefficient, Lamb).
- [`WallLubrication`](Self::WallLubrication) — scaffold;
  `interfacialModels/wallLubricationModels/` (Antal, Tomiyama, Frank).
- [`TurbulentDispersion`](Self::TurbulentDispersion) — scaffold;
  `interfacialModels/turbulentDispersionModels/` (Burns, Gosman, …).

```rust
pub enum InterfacialForce {
    Drag(DragModel),
    Lift,
    VirtualMass,
    WallLubrication,
    TurbulentDispersion,
}
```

##### Variants

###### `Drag`

Interphase **drag** — the only implemented interfacial force. Wraps a
[`DragModel`]; [`momentum_coefficient`](Self::momentum_coefficient)
returns its `K_d`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `DragModel` |  |

###### `Lift`

**Lift** force — documented scaffold, not implemented.

###### `VirtualMass`

**Virtual (added) mass** force — documented scaffold, not implemented.

###### `WallLubrication`

**Wall lubrication** force — documented scaffold, not implemented.

###### `TurbulentDispersion`

**Turbulent dispersion** force — documented scaffold, not implemented.

##### Implementations

###### Methods

- ```rust
  pub fn momentum_coefficient(self: &Self, system: &TwoFluidSystem) -> Result<VolScalarField, MultiphaseError> { /* ... */ }
  ```
  Per-cell interfacial momentum-transfer coefficient `[kg/(m³·s)]` for the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TwoFluidSystem`

A Euler-Euler two-fluid system: a **dispersed** and a **continuous**
[`Phase`] on a shared [`FvMesh`], subject to the saturation constraint
`α_d + α_c = 1`.

**Not a coupled solver.** The phase velocities `U_k` are prescribed inputs
(this foundation does not solve phase momentum); the system advances only the
dispersed-phase continuity ([`advance_dispersed_alpha`](Self::advance_dispersed_alpha))
and re-derives `α_c` from the constraint. Interfacial drag is available via a
[`DragModel`] / [`InterfacialForce`] evaluated against this system. See the
module "Honest scope" section for what is deliberately omitted.

The constraint `α_d + α_c = 1` is held by construction and re-imposed after
each transport step by [`enforce_alpha_constraint`](Self::enforce_alpha_constraint).

```rust
pub struct TwoFluidSystem {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub dispersed: Phase,
    pub continuous: Phase,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` | Finite-volume mesh (shared, `Arc`). |
| `dispersed` | `Phase` | Dispersed phase `d` (bubbles / droplets / particles). |
| `continuous` | `Phase` | Continuous phase `c` (carrier fluid); its `α_c` is kept equal to `1 − α_d`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(dispersed: Phase, continuous: Phase) -> Result<Self, MultiphaseError> { /* ... */ }
  ```
  Assemble a two-fluid system from a dispersed and a continuous [`Phase`].

- ```rust
  pub fn enforce_alpha_constraint(self: &mut Self) { /* ... */ }
  ```
  Re-impose the saturation constraint `α_c = 1 − α_d` on every cell.

- ```rust
  pub fn reynolds_number(self: &Self) -> Result<VolScalarField, MultiphaseError> { /* ... */ }
  ```
  Per-cell slip **Reynolds number** `Re = |U_d − U_c|·d / ν_c` `[-]`.

- ```rust
  pub fn advance_dispersed_alpha(self: &mut Self, dt: f64) -> Result<(), MultiphaseError> { /* ... */ }
  ```
  Advance the dispersed-phase volume fraction `α_d` by one implicit-Euler

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
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

#### Constant `RESIDUAL_ALPHA`

Residual (floor) volume fraction `α_res` `[-]` used to keep the drag
coefficient well-defined as a phase disappears.

Mirrors OpenFOAM's `phase::residualAlpha()` guard used in
`dispersedDragModel::K()` (`dispersedDragModel.C:70`, `max(αd, residualAlpha)`)
and in `WenYu::CdRe()` (`WenYu.C:66`, `max(αc, residualAlpha)`). A small
positive constant (`1e-6`) rather than a per-phase dictionary entry at this
foundation stage.

```rust
pub const RESIDUAL_ALPHA: f64 = 1.0e-6;
```

## Module `two_fluid_pimple`

**Two-fluid Euler-Euler phase pressure-velocity coupling** — the
two-momentum analogue of the drift-flux PIMPLE loop ([`crate::pimple`]).
This closes the #1 gap the [`crate::two_fluid`] foundation left open (its
honest scope: *"No phase-momentum / pressure-coupled PIMPLE loop"*) with a
real, tested **shared-pressure** Euler-Euler PISO: two per-phase momentum
predictors, a single mixture-continuity pressure Poisson equation, and a
checkerboard-safe `fvc::reconstruct` velocity/flux correction for each phase.

## What it solves

One time step of the two coupled **phase-intensive** (per-unit-density)
momentum equations for a dispersed phase `d` and a continuous phase `c`,
coupled to a single shared **dynamic pressure** `p` through the mixture
volumetric-continuity constraint, then advances the dispersed volume
fraction `α_d` on the corrected phase velocity:

```text
 ∂(α_k U_k)/∂t + ∇·(α_k φ_k U_k) − ∇·(α_k ν_k ∇U_k)
       = −(α_k/ρ_k)∇p + α_k g + (K_d/ρ_k)(U_o − U_k)      (phase k momentum)

 Σ_k ∇·(α_k U_k) = 0    ⇒    ∇·( Γ_mix ∇p ) = ∇·φ_HbyA,mix   (shared pressure)

 φ_k = φ_HbyA_k − (r_AU,k α_k/ρ_k)_f |S_f| ∂p/∂n ,  U_k = reconstruct(φ_k)   (correct)

 ∂α_d/∂t + ∇·(α_d φ_d) = 0 ,  α_c = 1 − α_d ,  both in [0,1]   (continuity)
```

where `U_k` is the phase velocity `[m/s]`, `φ_k = U_k·S_f` the phase
velocity face flux `[m³/s]`, `α_k` the volume fraction `[-]`, `ρ_k` the
density `[kg/m³]`, `ν_k = μ_k/ρ_k` the kinematic viscosity `[m²/s]`, `g`
gravity `[m/s²]`, `K_d` the interphase drag coefficient `[kg/(m³·s)]`
(from [`DragModel`]), `U_o` the *other* phase's velocity, `p` the dynamic
pressure `[Pa]`, `r_AU,k = V/A_k` the reciprocal momentum diagonal `[s]`,
and `φ_HbyA,mix` / `Γ_mix` the mixture predicted flux / pressure diffusivity
assembled below.

## Drag coupling — semi-implicit split (documented choice)

The interphase drag `(K_d/ρ_k)(U_o − U_k)` couples the two momentum
balances. This foundation uses a **simple semi-implicit split** (not full
partial elimination / PEA): in phase `k`'s own matrix the sink `−(K_d/ρ_k)U_k`
is **implicit** (added to the diagonal via [`fvm::sp`]-style assembly, which
keeps the matrix diagonally dominant), while the source `+(K_d/ρ_k)U_o` is
**explicit** in the *other* phase's velocity from the start of the current
outer iteration. Iterating the PIMPLE outer loop
([`n_outer_correctors`](TwoFluidPimple::n_outer_correctors)) converges the
explicit coupling. OpenFOAM's `multiphaseEuler` instead eliminates the drag
implicitly across phases through the block-coupled `invADV` operator
(`momentumTransferSystem::invADVs`); porting that partial elimination is
later work (see honest scope).

## Upstream provenance (ported C++ → Rust)

Structure follows OpenFOAM-dev's `multiphaseEuler` solver module (the
shared-pressure `pU` / cell-pressure-corrector path). Exact citations:

| Rust step | OpenFOAM source |
|---|---|
| [`TwoFluidPimple::phase_momentum_predictor`] (per-phase `U_k` predictor) | `multiphaseEuler/momentumPredictor.C:44` (`cellMomentumPredictor`, `UEqns.set(... phase.UEqn() == ...)`) + `phaseSystem/phaseModels/MovingPhaseModel/MovingPhaseModel.C:341` (`UEqn()` = `fvm::ddt(alpha,rho,U)+fvm::div(alphaRhoPhi,U)+…`) |
| `r_A,k = 1/A_k` diagonal, `HbyA_k` | `multiphaseEuler/cellPressureCorrector.C:82` (`As.set(... UEqns[...].A())`), `:167` (`Hs.set(... UEqns[...].H())`) |
| mixture predicted flux `φ_HbyA,mix = Σ α_kf φ_HbyA_k` | `cellPressureCorrector.C:289` (`phiHbyA += alphafs[phase]*phiHbyADs[...]`) |
| mixture pressure diffusivity `Γ_mix = Σ α_kf²·r_A,kf/ρ_k` | `cellPressureCorrector.C:312` (`rAf += alphafs[phase]*alphaByADfs[...]`, and `alphaByADfs = invADVfs & movingAlphafs` carries the second `α`) |
| shared pressure Poisson `∇·(Γ_mix∇p) = ∇·φ_HbyA,mix` | `cellPressureCorrector.C:352` (`fvc::div(phiHbyA) − fvm::laplacian(rAf, p_rgh)`) |
| per-phase flux `φ_k = φ_HbyA_k − (r_A,kα_k/ρ_k)_f|S_f|∂p/∂n`, `U_k = reconstruct(...)` | `cellPressureCorrector.C:384` (`phi_ = phiHbyA + pEqnIncomp.flux()`), `:445` (`URef() = HbyADs[...] + fvc::reconstruct(alphaByADfs[...]*mSfGradp − …)`) |
| `α_d` advance on corrected `U_d`, `α_c = 1−α_d` | via [`TwoFluidSystem::advance_dispersed_alpha`] (`phaseSystem/phaseSystem/phaseSystemSolve.C:336`) |

The collocated-grid **odd-even ("checkerboard") decoupling** fix is exactly
the one [`crate::pimple`] documents: each phase velocity is reconstructed
from its Rhie-Chow-coupled *face* flux `φ_k` (which carries the correct
nearest-neighbour pressure coupling), **not** from the cell-centred
`HbyA_k − r_AU,k(α_k/ρ_k)∇p`. With zero-gradient pressure wall BCs the Gauss
cell gradient is inaccurate near walls and would otherwise seed a spurious
oscillation; the flux reconstruction is clean, giving exact `U_k = 0` for the
at-rest and hydrostatic balances.

## Honest scope — what is **NOT** modelled here

This is a **tested foundation**, not a validated or converged coupled
two-fluid solver. Reviewers and users must not read it as production
multiphase CFD. Specifically:

- **Shared-pressure Euler-Euler only.** A single dynamic pressure shared by
  both phases (no per-phase pressure, no `implicitPhasePressure` particle
  pressure, no compressible `p_rgh`/buoyancy split). Only the two phase
  momenta and the mixture volumetric-continuity constraint are coupled.
- **Drag is the only interfacial force in the coupling.** Lift, virtual
  (added) mass, wall lubrication, and turbulent dispersion are *not* fed into
  the momentum coupling — they remain the documented scaffolds in
  [`crate::two_fluid::InterfacialForce`]. Only the [`DragModel`] `K_d` couples
  the phases.
- **Semi-implicit (not partial-elimination) drag.** The `+(K_d/ρ_k)U_o`
  source is explicit in the other phase's velocity; strong drag is only fully
  consistent once the outer loop converges. No block `invADV` elimination.
- **Leading-order `HbyA`.** As in [`crate::pimple`], `HbyA_k` is formed from
  the momentum matrix's snapshotted `H`-source (ddt-old + body force +
  explicit drag + BC terms); the convection/diffusion off-diagonal-times-`U*`
  contribution is not re-folded each PISO sweep. Exact for the at-rest and
  hydrostatic cases (off-diagonals act on `U*→0`); on strongly advecting flow
  it converges as the outer loop iterates but is not the full `UEqn.H()`.
- **First order in space and time.** Implicit-Euler `ddt` with a frozen
  volume-fraction coefficient, first-order upwind convection, Gauss-orthogonal
  Laplacian. No higher-order/TVD convection, no second-order time, **no
  non-orthogonal correction**, and no `fvc::ddtCorr` Rhie-Chow time-derivative
  flux correction (so strongly transient coarse meshes can show some
  pressure-velocity decoupling).
- **Laminar, constant properties.** `ρ_k`, `μ_k`, and the dispersed diameter
  are constants; no turbulence model, no thermo, no population balance.
- **No MULES.** `α_d` is bounded by the plain post-solve clamp inherited from
  [`TwoFluidSystem::advance_dispersed_alpha`] (physical but not strictly
  conservative when the clamp bites); `α_c = 1 − α_d`.
- **Pressure reference pinned** at one cell (all-Neumann / closed or
  velocity-driven pressure field); a fixed-pressure outlet BC is not specially
  handled.

**Benchmark validation is a later, human-run step.** No benchmark comparison
(bubble column, sedimentation, dam-break, …) has been performed. The tests
below are *verification* checks — finiteness/boundedness, at-rest stability,
hydrostatic balance `dp/dz = −ρ_mix·g`, the `α_d + α_c = 1` / `[0,1]`
constraint, and drag-driven velocity equilibration — **not** *validation*
against experimental or reference-solver data. Nothing here is validated
multiphase CFD and it must not be described as such.

```rust
pub mod two_fluid_pimple { /* ... */ }
```

### Types

#### Type Alias `Pressure`

Dynamic (mechanical) pressure field `p` `[Pa]`, defined up to an additive
constant. Carried as a [`VolScalarField`]; this alias documents the physical
meaning at the API boundary. (Same convention as [`crate::pimple::Pressure`].)

```rust
pub type Pressure = outram_foam_basic_lib::prelude::VolScalarField;
```

#### Struct `TwoFluidPimple`

Shared-pressure Euler-Euler PISO/PIMPLE pressure-velocity coupler for a
[`TwoFluidSystem`] (dispersed phase `d` + continuous phase `c`).

Owns the two-fluid system (each phase's `α_k`, `U_k`, and constant
properties), the interphase [`DragModel`] that couples the two momenta, the
shared dynamic pressure field, the two phase velocity fluxes, and the loop
controls. One call to [`solve_timestep`](Self::solve_timestep) advances the
coupled `U_d`–`U_c`–`p`–`α_d` system by one time step.

## Field ownership (per the workspace design rules)

All fields are owned **by value**; the mesh is shared with `Arc<FvMesh>`;
cells and faces are indexed by `usize`. No `Box<dyn>`/`dyn`, no lifetime
parameters, no channels. Model dispatch (the drag closure) is the
[`DragModel`] enum.

## Setting up a case

After construction, prescribe the phase velocity boundary conditions on the
owned [`system`](Self::system) (`system.dispersed.u_mut()` /
`system.continuous.u_mut()` and their `.boundary`), any dispersed-fraction
inlet BC (`system.dispersed.alpha_mut().boundary`), the gravity vector
[`gravity`](Self::gravity), and the corrector counts, then call
[`solve_timestep`](Self::solve_timestep) each step.

```rust
pub struct TwoFluidPimple {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub system: crate::two_fluid::TwoFluidSystem,
    pub drag: crate::two_fluid::DragModel,
    pub p: Pressure,
    pub phi_d: outram_foam_basic_lib::prelude::SurfaceScalarField,
    pub phi_c: outram_foam_basic_lib::prelude::SurfaceScalarField,
    pub gravity: outram_foam_basic_lib::prelude::Vector3,
    pub n_correctors: usize,
    pub n_outer_correctors: usize,
    pub p_ref_cell: usize,
    pub p_ref_value: f64,
    pub solver_settings: outram_foam_basic_lib::prelude::SolverSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` | Finite-volume mesh (shared, `Arc`). |
| `system` | `crate::two_fluid::TwoFluidSystem` | The two-fluid system: dispersed + continuous phase fields (`α_k`, `U_k`)<br>and the saturation constraint `α_d + α_c = 1`. |
| `drag` | `crate::two_fluid::DragModel` | Interphase drag closure providing `K_d` `[kg/(m³·s)]` — the only<br>interfacial force coupling the two momenta at this foundation stage. |
| `p` | `Pressure` | Shared dynamic pressure `p` `[Pa]` (see [`Pressure`]). |
| `phi_d` | `outram_foam_basic_lib::prelude::SurfaceScalarField` | Dispersed-phase velocity face flux `φ_d = U_d·S_f` `[m³/s]` after the<br>last correction (Rhie-Chow coupled). |
| `phi_c` | `outram_foam_basic_lib::prelude::SurfaceScalarField` | Continuous-phase velocity face flux `φ_c = U_c·S_f` `[m³/s]` after the<br>last correction (Rhie-Chow coupled). |
| `gravity` | `outram_foam_basic_lib::prelude::Vector3` | Gravitational acceleration `g` `[m/s²]`. `Vector3::ZERO` disables gravity. |
| `n_correctors` | `usize` | Number of PISO pressure correctors per outer iteration (`nCorrectors`).<br>Must be `≥ 1`; `2` is a common default. |
| `n_outer_correctors` | `usize` | Number of outer PIMPLE iterations per time step (`nOuterCorrectors`).<br>Must be `≥ 1`; `1` recovers plain PISO. More iterations tighten the<br>explicit inter-phase drag coupling. |
| `p_ref_cell` | `usize` | Reference cell whose pressure is pinned to [`p_ref_value`](Self::p_ref_value)<br>(fixes the otherwise-singular all-Neumann pressure matrix). |
| `p_ref_value` | `f64` | Value the reference cell's pressure is pinned to `[Pa]`. |
| `solver_settings` | `outram_foam_basic_lib::prelude::SolverSettings` | Linear-solver settings shared by the momentum and pressure solves. |

##### Implementations

###### Methods

- ```rust
  pub fn new(system: TwoFluidSystem, drag: DragModel) -> Self { /* ... */ }
  ```
  Construct a coupler around a [`TwoFluidSystem`] and an interphase

- ```rust
  pub fn solve_timestep(self: &mut Self, dt: f64) -> Result<(), MultiphaseError> { /* ... */ }
  ```
  Advance the coupled phase momenta, shared pressure, and dispersed volume

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `wall_boiling`

Stage 3 — **Wall-boiling framework** (bead `op-2kk.3`).

This module defines the *architecture* for wall-boiling closures — the enum,
the compiler-enforced contract, and the input/output boundary types — and
ships **one fully worked, tested concrete model**: the RPI (Rensselaer
Polytechnic Institute) heat-flux–partitioning model of Kurul & Podowski
(1991) for the **nucleate-boiling** regime. The other boiling regimes
(critical heat flux, subcooled CHF, dryout, film boiling) are present as
**architecture placeholders** that return
[`MultiphaseError::NotImplemented`] — honest scaffolding, not faked physics.

The design deliberately follows the roadmap directive to settle
architecture / traits / interfaces / verification **before** adding advanced
closures.

## The RPI heat-flux partitioning (nucleate boiling)

On a heated wall in the nucleate-boiling regime the total wall heat flux is
split into three additive mechanisms (Kurul & Podowski, 1991):

```text
q_wall = q_convective + q_quenching + q_evaporative
```

- **`q_convective`** — single-phase forced convection over the fraction `A1`
  of the wall not currently influenced by bubbles:
  `q_convective = A1 · h_c · (T_wall − T_liquid)`.
- **`q_quenching`** — transient conduction into cold liquid that rushes in to
  fill the site as a bubble departs, over the bubble-influenced fraction `A2`
  (Mikic & Rohsenow, 1969):
  `q_quenching = A2 · h_q · (T_wall − T_liquid)` with
  `h_q = 2·sqrt(k_l·ρ_l·Cp_l·f·τ / π)`.
- **`q_evaporative`** — latent heat carried off by the departing bubbles:
  `q_evaporative = (π/6)·d_dep³ · ρ_v · N · f · L`.

The partition is closed by three **sub-closures**, each a standard
literature correlation (cited on its method below):

| Sub-closure | Correlation | Symbol |
|---|---|---|
| Nucleation-site density | Lemmert & Chawla (1977) | `N` `[1/m²]` |
| Bubble departure diameter | Tolubinski & Kostanchuk (1970) | `d_dep` `[m]` |
| Bubble departure frequency | Cole (1960) | `f` `[1/s]` |

and the bubble-influence area fraction `A2` uses the Del Valle & Kenning
(1985) influence factor. See [`RpiPartitioning`] for the equations and their
coefficients.

## Upstream provenance (concepts ported C++ → Rust)

Correlation *forms and default coefficients* are taken from OpenFOAM's
`multiphaseEuler` wall-boiling `fvModel`
(`applications/modules/multiphaseEuler/fvModels/wallBoiling/`, GPL-3.0).
Exact file citations:

| Rust item | OpenFOAM source |
|---|---|
| [`RpiPartitioning::nucleation_site_density`] | `nucleationSiteModels/LemmertChawla/LemmertChawla.C` (`calculate`) |
| [`RpiPartitioning::departure_diameter`] | `departureDiameterModels/TolubinskiKostanchuk/TolubinskiKostanchuk.C` (`calculate`) |
| [`RpiPartitioning::departure_frequency`] | `departureFrequencyModels/Cole/Cole.C` (`calculate`) |
| [`RpiPartitioning::partition`] (area split, `q_c`/`q_q`/`q_e`) | `fvModels/wallBoiling/wallBoiling.C` (`calcBoiling`) |

## Honest scope — what this is and is **not**

This is a **verified framework with one worked model**, not validated
boiling CFD. Reviewers and users must not read it as a qualified wall-boiling
solver. Specifically:

- **Point (0-D) evaluation, not a wall-function field.** [`RpiPartitioning`]
  evaluates the partition at a *single wall condition* — a set of scalar
  near-wall states supplied by the caller ([`WallBoilingConditions`]). It is
  not wired into a mesh boundary field, a turbulence wall function, or a
  phase-change mass source; those couplings are later work. OpenFOAM obtains
  the convective conductance from a turbulent thermal wall function
  (`alphatJayatillekeWallFunction`); here the caller supplies the
  single-phase convective HTC `h_c` directly.
- **Classic evaporative term.** The evaporative flux uses the classic
  Kurul–Podowski `(π/6)·d³·ρ_v·N·f·L` form. OpenFOAM's `calcBoiling`
  modernises this with a bubble-influence-area cap (`A2E`); that refinement
  is documented but not ported here.
- **Fully-wetted wall assumed.** The wall wetted fraction is taken as `1`
  (no partitioning-model dryout weighting); dry-patch weighting belongs to
  the CHF / dryout regimes below, which are not implemented.
- **Constant properties.** Fluid properties are inputs held constant over the
  evaluation; no property tables, no temperature dependence within a call.
- **Only the nucleate-boiling regime is implemented.** [`ChfModel`],
  [`ChfSubCoolModel`], [`DryoutModel`], and [`FilmBoilingModel`] are
  architecture placeholders that error with
  [`MultiphaseError::NotImplemented`].

**Benchmark validation is a later, human-run step.** No experimental or
reference-solver comparison has been performed. The tests below are
*verification* checks (partition-sum conservation, non-negativity of each
component, hand-computed sub-closure values, enum-dispatch reachability,
clean `NotImplemented` errors) — **not** *validation*.

```rust
pub mod wall_boiling { /* ... */ }
```

### Types

#### Struct `WallBoilingConditions`

Near-wall thermodynamic state at which a wall-boiling model is evaluated.

A wall-boiling closure needs the wall temperature, the local near-wall
liquid temperature, the saturation temperature, the two-phase fluid
properties, the single-phase convective heat-transfer coefficient, and
gravity. This struct carries them `uom`-typed at the API boundary so the
physical dimension of every input is checked by the compiler.

## Fields, units, and valid ranges

- `t_wall` — heated-wall surface temperature `T_w` `[K]`.
- `t_liquid` — near-wall bulk **liquid** temperature `T_l` `[K]`. For
  *subcooled* boiling `T_l < T_sat`; for saturated boiling `T_l ≈ T_sat`.
- `t_sat` — saturation temperature `T_sat` at the local pressure `[K]`.
- `rho_liquid`, `rho_vapour` — liquid / vapour density `[kg/m³]`, **> 0**.
- `cp_liquid` — liquid specific heat capacity `Cp_l` `[J/(kg·K)]`, **> 0**.
- `k_liquid` — liquid thermal conductivity `k_l` `[W/(m·K)]`, **> 0**.
- `latent_heat` — latent heat of vaporisation `L` `[J/kg]`, **> 0**.
- `h_convective` — single-phase forced-convection heat-transfer coefficient
  `h_c` `[W/(m²·K)]`, **≥ 0** (drives the convective partition).
- `gravity` — gravitational acceleration magnitude `g` `[m/s²]`, **≥ 0**
  (drives the Cole departure-frequency buoyancy term).

The wall superheat is `T_w − T_sat` and the liquid subcooling is
`T_sat − T_l`; both are derived inside the model, not stored here.

```rust
pub struct WallBoilingConditions {
    pub t_wall: uom::si::f64::ThermodynamicTemperature,
    pub t_liquid: uom::si::f64::ThermodynamicTemperature,
    pub t_sat: uom::si::f64::ThermodynamicTemperature,
    pub rho_liquid: uom::si::f64::MassDensity,
    pub rho_vapour: uom::si::f64::MassDensity,
    pub cp_liquid: uom::si::f64::SpecificHeatCapacity,
    pub k_liquid: uom::si::f64::ThermalConductivity,
    pub latent_heat: uom::si::f64::AvailableEnergy,
    pub h_convective: uom::si::f64::HeatTransfer,
    pub gravity: uom::si::f64::Acceleration,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_wall` | `uom::si::f64::ThermodynamicTemperature` | Heated-wall surface temperature `T_w` `[K]`. |
| `t_liquid` | `uom::si::f64::ThermodynamicTemperature` | Near-wall bulk liquid temperature `T_l` `[K]`. |
| `t_sat` | `uom::si::f64::ThermodynamicTemperature` | Saturation temperature `T_sat` `[K]`. |
| `rho_liquid` | `uom::si::f64::MassDensity` | Liquid density `ρ_l` `[kg/m³]`. |
| `rho_vapour` | `uom::si::f64::MassDensity` | Vapour density `ρ_v` `[kg/m³]`. |
| `cp_liquid` | `uom::si::f64::SpecificHeatCapacity` | Liquid specific heat capacity `Cp_l` `[J/(kg·K)]`. |
| `k_liquid` | `uom::si::f64::ThermalConductivity` | Liquid thermal conductivity `k_l` `[W/(m·K)]`. |
| `latent_heat` | `uom::si::f64::AvailableEnergy` | Latent heat of vaporisation `L` `[J/kg]`. |
| `h_convective` | `uom::si::f64::HeatTransfer` | Single-phase convective heat-transfer coefficient `h_c` `[W/(m²·K)]`. |
| `gravity` | `uom::si::f64::Acceleration` | Gravitational acceleration magnitude `g` `[m/s²]`. |

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
    fn clone(self: &Self) -> WallBoilingConditions { /* ... */ }
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `HeatFluxPartition`

Result of a wall-boiling heat-flux partition.

The three additive flux components plus their sum are `uom`-typed
[`HeatFluxDensity`] `[W/m²]`. The diagnostic sub-closure outputs are carried
as documented `f64` in SI units (some — e.g. nucleation-site density
`[1/m²]` — have no convenient `uom` alias), following the field-value
convention used elsewhere in this crate.

By construction `total = convective + quenching + evaporative` and every
component is non-negative for physically-valid inputs (see
[`RpiPartitioning::partition`]).

```rust
pub struct HeatFluxPartition {
    pub convective: uom::si::f64::HeatFluxDensity,
    pub quenching: uom::si::f64::HeatFluxDensity,
    pub evaporative: uom::si::f64::HeatFluxDensity,
    pub total: uom::si::f64::HeatFluxDensity,
    pub nucleation_site_density: f64,
    pub departure_diameter: f64,
    pub departure_frequency: f64,
    pub area_fraction_convective: f64,
    pub area_fraction_boiling: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `convective` | `uom::si::f64::HeatFluxDensity` | Single-phase convective flux `q_convective` `[W/m²]`. |
| `quenching` | `uom::si::f64::HeatFluxDensity` | Quenching (transient-conduction) flux `q_quenching` `[W/m²]`. |
| `evaporative` | `uom::si::f64::HeatFluxDensity` | Evaporative (latent) flux `q_evaporative` `[W/m²]`. |
| `total` | `uom::si::f64::HeatFluxDensity` | Total wall heat flux `q_wall = q_c + q_q + q_e` `[W/m²]`. |
| `nucleation_site_density` | `f64` | Nucleation-site density `N` `[1/m²]` (diagnostic). |
| `departure_diameter` | `f64` | Bubble departure diameter `d_dep` `[m]` (diagnostic). |
| `departure_frequency` | `f64` | Bubble departure frequency `f` `[1/s]` (diagnostic). |
| `area_fraction_convective` | `f64` | Single-phase convective area fraction `A1` `[-]` (diagnostic). |
| `area_fraction_boiling` | `f64` | Bubble-influenced area fraction `A2` `[-]` (diagnostic). |

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
    fn clone(self: &Self) -> HeatFluxPartition { /* ... */ }
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `RpiPartitioning`

RPI heat-flux–partitioning model for the **nucleate-boiling** regime
(Kurul & Podowski, 1991).

Splits the wall flux into convective, quenching, and evaporative parts (see
the [module docs](crate::wall_boiling)) and closes it with three literature
sub-closures. The struct holds the tunable coefficients of those
sub-closures; [`RpiPartitioning::default`] uses OpenFOAM's default values.

## Coefficients (units and defaults)

Lemmert–Chawla nucleation-site density:
- `cn` — model constant `Cn` `[-]`, default `1.0`.
- `n_ref` — reference site density `N_ref` `[1/m²]`, default `9.922e5`.
- `delta_t_ref` — reference superheat `ΔT_ref` `[K]`, default `10.0`.
- `nucleation_exponent` — superheat exponent `[-]`, default `1.805`.

Tolubinski–Kostanchuk departure diameter:
- `d_ref` — reference diameter `[m]`, default `6.0e-4`.
- `d_max` — upper clamp `[m]`, default `1.4e-3`.
- `d_min` — lower clamp `[m]`, default `1.0e-6`.
- `subcooling_scale` — subcooling scale `[K]`, default `45.0`.

Cole departure frequency:
- `min_density_difference` — floor on `ρ_l − ρ_v` `[kg/m³]`, default `0.1`.

Quenching:
- `bubble_waiting_time_ratio` — waiting-time fraction `τ` `[-]`, default `0.8`.

All defaults are the OpenFOAM `wallBoiling` `fvModel` defaults (see the
module provenance table).

```rust
pub struct RpiPartitioning {
    pub cn: f64,
    pub n_ref: f64,
    pub delta_t_ref: f64,
    pub nucleation_exponent: f64,
    pub d_ref: f64,
    pub d_max: f64,
    pub d_min: f64,
    pub subcooling_scale: f64,
    pub min_density_difference: f64,
    pub bubble_waiting_time_ratio: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cn` | `f64` | Lemmert–Chawla constant `Cn` `[-]`. |
| `n_ref` | `f64` | Lemmert–Chawla reference site density `N_ref` `[1/m²]`. |
| `delta_t_ref` | `f64` | Lemmert–Chawla reference superheat `ΔT_ref` `[K]`. |
| `nucleation_exponent` | `f64` | Lemmert–Chawla superheat exponent `[-]`. |
| `d_ref` | `f64` | Tolubinski–Kostanchuk reference departure diameter `[m]`. |
| `d_max` | `f64` | Tolubinski–Kostanchuk upper diameter clamp `[m]`. |
| `d_min` | `f64` | Tolubinski–Kostanchuk lower diameter clamp `[m]`. |
| `subcooling_scale` | `f64` | Tolubinski–Kostanchuk subcooling scale `[K]`. |
| `min_density_difference` | `f64` | Cole floor on the phase density difference `ρ_l − ρ_v` `[kg/m³]`. |
| `bubble_waiting_time_ratio` | `f64` | Bubble waiting-time ratio `τ` `[-]` in the quenching HTC. |

##### Implementations

###### Methods

- ```rust
  pub fn nucleation_site_density(self: &Self, delta_t_superheat: f64) -> f64 { /* ... */ }
  ```
  **Nucleation-site density** `N` `[1/m²]` (Lemmert & Chawla, 1977).

- ```rust
  pub fn departure_diameter(self: &Self, delta_t_subcooling: f64) -> f64 { /* ... */ }
  ```
  **Bubble departure diameter** `d_dep` `[m]` (Tolubinski & Kostanchuk, 1970).

- ```rust
  pub fn departure_frequency(self: &Self, departure_diameter: f64, rho_liquid: f64, rho_vapour: f64, gravity: f64) -> f64 { /* ... */ }
  ```
  **Bubble departure frequency** `f` `[1/s]` (Cole, 1960).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> RpiPartitioning { /* ... */ }
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
    OpenFOAM `wallBoiling` `fvModel` default coefficients.

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **WallHeatFluxPartition**
  - ```rust
    fn partition(self: &Self, c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> { /* ... */ }
    ```
    Evaluate the full RPI three-way partition at the given near-wall state.

#### Struct `ChfModel`

**Critical-heat-flux (CHF) regime** — architecture placeholder.

The CHF regime marks the boiling crisis where the wall dries out and the
heat-transfer coefficient collapses. A concrete model would evaluate a CHF
correlation (e.g. Zuber's hydrodynamic-instability limit, or a lookup table)
and blend the partition toward the transition boiling curve.

**Not implemented.** [`partition`](WallHeatFluxPartition::partition) returns
[`MultiphaseError::NotImplemented`]. Present so the [`WallBoilingModel`] enum
spans the full regime set at the architecture stage (bead `op-2kk.4`).

```rust
pub struct ChfModel;
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
    fn clone(self: &Self) -> ChfModel { /* ... */ }
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
    fn default() -> ChfModel { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **WallHeatFluxPartition**
  - ```rust
    fn partition(self: &Self, _c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> { /* ... */ }
    ```

#### Struct `ChfSubCoolModel`

**Subcooled-CHF regime** — architecture placeholder.

A concrete model would apply a subcooling-corrected CHF correlation for the
forced-convection subcooled regime (e.g. a Groeneveld-style look-up-table
correction, or the Hall–Mudawar subcooled CHF correlation).

**Not implemented.** [`partition`](WallHeatFluxPartition::partition) returns
[`MultiphaseError::NotImplemented`] (bead `op-2kk.4`).

```rust
pub struct ChfSubCoolModel;
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
    fn clone(self: &Self) -> ChfSubCoolModel { /* ... */ }
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
    fn default() -> ChfSubCoolModel { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **WallHeatFluxPartition**
  - ```rust
    fn partition(self: &Self, _c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> { /* ... */ }
    ```

#### Struct `DryoutModel`

**Dryout / post-dryout regime** — architecture placeholder.

Past dryout, the wall is vapour-blanketed and heat transfer is governed by
the vapour phase plus droplet deposition. A concrete model would evaluate a
post-dryout heat-transfer correlation and a droplet-deposition closure.

**Not implemented.** [`partition`](WallHeatFluxPartition::partition) returns
[`MultiphaseError::NotImplemented`] (bead `op-2kk.5`).

```rust
pub struct DryoutModel;
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
    fn clone(self: &Self) -> DryoutModel { /* ... */ }
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
    fn default() -> DryoutModel { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **WallHeatFluxPartition**
  - ```rust
    fn partition(self: &Self, _c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> { /* ... */ }
    ```

#### Struct `FilmBoilingModel`

**Film-boiling regime** — architecture placeholder.

In stable film boiling a continuous vapour film insulates the wall; heat
transfer is by conduction/convection across the film plus radiation. A
concrete model would evaluate a film-boiling correlation (e.g. Bromley) and
a radiation contribution.

**Not implemented.** [`partition`](WallHeatFluxPartition::partition) returns
[`MultiphaseError::NotImplemented`] (bead `op-2kk.5`).

```rust
pub struct FilmBoilingModel;
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
    fn clone(self: &Self) -> FilmBoilingModel { /* ... */ }
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
    fn default() -> FilmBoilingModel { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **WallHeatFluxPartition**
  - ```rust
    fn partition(self: &Self, _c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> { /* ... */ }
    ```

#### Enum `WallBoilingModel`

Wall-boiling closure, dispatched by regime.

Enum dispatch (not `dyn`) per the workspace design rules: the set of boiling
regimes is closed and known at compile time, so a `match` is exhaustive
(adding a regime forces every call site to handle it) and every variant is
rust-analyzer-navigable. Each variant wraps a concrete struct that implements
the [`WallHeatFluxPartition`] contract.

## Variants
- [`NucleateBoiling`](Self::NucleateBoiling) — **implemented**: RPI
  three-way partitioning ([`RpiPartitioning`]).
- [`Chf`](Self::Chf) — critical heat flux (placeholder, [`ChfModel`]).
- [`ChfSubCool`](Self::ChfSubCool) — subcooled CHF (placeholder,
  [`ChfSubCoolModel`]).
- [`Dryout`](Self::Dryout) — dryout / post-dryout (placeholder,
  [`DryoutModel`]).
- [`FilmBoiling`](Self::FilmBoiling) — film boiling (placeholder,
  [`FilmBoilingModel`]).

```rust
pub enum WallBoilingModel {
    NucleateBoiling(RpiPartitioning),
    Chf(ChfModel),
    ChfSubCool(ChfSubCoolModel),
    Dryout(DryoutModel),
    FilmBoiling(FilmBoilingModel),
}
```

##### Variants

###### `NucleateBoiling`

Nucleate boiling — RPI heat-flux partitioning (implemented).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `RpiPartitioning` |  |

###### `Chf`

Critical heat flux (architecture placeholder).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ChfModel` |  |

###### `ChfSubCool`

Subcooled critical heat flux (architecture placeholder).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ChfSubCoolModel` |  |

###### `Dryout`

Dryout / post-dryout (architecture placeholder).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `DryoutModel` |  |

###### `FilmBoiling`

Film boiling (architecture placeholder).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FilmBoilingModel` |  |

##### Implementations

###### Methods

- ```rust
  pub fn partition(self: &Self, conditions: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> { /* ... */ }
  ```
  Partition the wall heat flux using the selected regime model.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> WallBoilingModel { /* ... */ }
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `WallHeatFluxPartition`

Compiler-enforced contract for a wall heat-flux–partitioning model.

Every concrete boiling-regime struct implements this trait, so the compiler
verifies each one provides the same [`partition`](Self::partition) entry
point. The trait is **not** used for runtime dispatch — that is done by the
[`WallBoilingModel`] enum (workspace rule: no `Box<dyn>` / `dyn`). It exists
purely as the interface check.

```rust
pub trait WallHeatFluxPartition {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `partition`: Partition the wall heat flux for the given near-wall condition.

##### Implementations

This trait is implemented for the following types:

- `RpiPartitioning`
- `ChfModel`
- `ChfSubCoolModel`
- `DryoutModel`
- `FilmBoilingModel`

## Types

### Enum `MultiphaseError`

Errors produced by the multiphase solvers in this crate.

```rust
pub enum MultiphaseError {
    InvalidInput(String),
    NotImplemented(String),
    Solver(String),
}
```

#### Variants

##### `InvalidInput`

A model input was outside its valid physical range.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### `NotImplemented`

A requested feature is scaffolded but not yet implemented.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### `Solver`

A numerical failure (non-convergence, non-physical state) occurred.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

#### Implementations

##### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
