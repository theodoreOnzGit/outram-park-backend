# Crate Documentation

**Version:** 0.0.2

**Format Version:** 61

# Module `outram_park_fork_dwsim_libs`

# outram-park-fork-dwsim-libs

Pure-Rust translation of selected [DWSIM](https://dwsim.org) chemical-
process equipment models and correlations -- an independent OUTRAM PARK
fork, not the official DWSIM software (see `TRADEMARKS.md`). See
`CLAUDE.md` for build/test instructions and `docs/port-scope.md` for the
prioritised porting scope, C# source map, and porting order.

## What belongs here / what does not

- **Belongs here:** equipment-model correlations translated from DWSIM's
  `UnitOperations` (pipe pressure drop, valve sizing, heat-exchanger
  rating, pump/expander thermodynamics, [`reactors`] built on the
  [`reactions`] model) with `uom`-typed public APIs, plus the [`thermo`]
  thermodynamics kernel (EOS, activity models, flash algorithms, property
  packages) those equipment models draw on.
- **Does NOT belong here:** DWSIM's GUI, XML/JSON serialization,
  property-grid reflection, or flowsheet-solver plumbing -- none of that
  is physics, and none of it is ported (see each module's doc comment for
  what was deliberately excluded from its source file).

## Modules

## Module `compressor`

Compressor thermodynamics: isentropic (adiabatic-efficiency) and Schultz
polytropic-efficiency compression duty.

Ported from DWSIM `UnitOperations/Compressor.vb` (GPL-3.0), the mirror of
the already-ported expander (`crate::expander::isentropic`). DWSIM computes
the ideal outlet enthalpy `H2s` via a pressure-entropy (isentropic) flash
and the actual outlet state via repeated pressure-enthalpy flashes -- this
equipment port is deliberately kept decoupled from the crate's flash kernel
([`crate::thermo`]), so the flash-dependent steps are pushed to
the caller: the functions below take already-known enthalpies/densities as
inputs, and [`solve_polytropic_efficiency`] takes a caller-supplied closure
for the one flash-dependent step in DWSIM's iteration (a generic `Fn`
parameter, not a trait object, per the workspace's no-`dyn`-dispatch rule).

**Sign convention** (chosen for this port): a compressor **CONSUMES**
power. [`consumed_power`] and the polytropic solver's duty are **positive
for a normal compression** (`h2s > h1`, `h2 > h1`) -- work is done *on* the
fluid to raise its pressure. This is the mirror of the expander, whose duty
is positive when power is *extracted* from the fluid.

### Ported vs. excluded

Ported (the physics core of `Compressor.vb`'s `Calculate`):
- adiabatic duty `W = w (h2s - h1) / eta`         (Compressor.vb:959)
- actual outlet enthalpy `h2 = h1 + W / w`         (Compressor.vb:969)
- isentropic exponent `n_i = ln(p2/p1)/ln(rho2i/rho1)` (Compressor.vb:995)
- polytropic exponent `n_p = ln(p2/p1)/ln(rho2/rho1)`  (Compressor.vb:999)
- Schultz correction factor `fce`                  (Compressor.vb:1001)
- polytropic-efficiency fixed-point loop           (Compressor.vb:885-947)

Deliberately **NOT** ported (out of scope for a units library):
- the `Curves` calculation mode and its speed/flow interpolation
  (Compressor.vb:418-568) -- pump/compressor performance-curve tables.
- the outlet-pressure-from-power root-find (`PFunction`/Brent,
  Compressor.vb:637-770) and the `k = Cp/Cv` discharge-pressure estimate
  (Compressor.vb:624) -- these belong to a flash-owning caller.
- GUI/editing-form plumbing, XML/JSON serialization (`SaveData`/`LoadData`/
  `CloneXML`/`CloneJSON`), property-grid reflection (`GetPropertyValue`/
  `GetProperties`), dynamic-mode and flowsheet-solver wiring.

The heads (`Wic`/`Wpc`, Compressor.vb:1012-1022) and the `k`-based discharge
estimate are not ported here; only the enthalpy-based duty/exponent core is.

```rust
pub mod compressor { /* ... */ }
```

### Types

#### Struct `PolytropicResult`

Converged result of [`solve_polytropic_efficiency`].

```rust
pub struct PolytropicResult {
    pub adiabatic_efficiency: uom::si::f64::Ratio,
    pub schultz_factor: uom::si::f64::Ratio,
    pub consumed: uom::si::f64::Power,
    pub h2: uom::si::f64::AvailableEnergy,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `adiabatic_efficiency` | `uom::si::f64::Ratio` | Converged adiabatic (isentropic) efficiency<br>`eta_adiabatic = eta_polytropic / f_ce`, dimensionless. |
| `schultz_factor` | `uom::si::f64::Ratio` | Schultz correction factor at convergence, dimensionless. |
| `consumed` | `uom::si::f64::Power` | Power consumed at convergence, W (positive for compression). |
| `h2` | `uom::si::f64::AvailableEnergy` | Actual outlet specific enthalpy at convergence, J/kg (`h2 > h1`). |

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
    fn clone(self: &Self) -> PolytropicResult { /* ... */ }
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
    fn eq(self: &Self, other: &PolytropicResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `consumed_power`

Power consumed by an adiabatic compression from inlet enthalpy `h1` to the
isentropic (ideal) outlet enthalpy `h2s`, at `adiabatic_efficiency`
\[0, 1\]: `W = w (h2s - h1) / eta`.

**Physical quantity:** shaft power drawn by the compressor, in watts.

**Sign convention:** positive for a normal compression (`h2s > h1`), since
the machine consumes work to raise the fluid's pressure. This mirrors
DWSIM `Compressor.vb:959` (`DeltaQ = Wi * (H2s - Hi) / (Eta/100)`), except
DWSIM carries efficiency as a percentage and duty in kW; here efficiency is
a dimensionless `Ratio` and the result is a uom `Power`.

**Inputs / units:**
- `w`: mass flow, kg/s (> 0).
- `h1`: inlet specific enthalpy, J/kg.
- `h2s`: isentropic outlet specific enthalpy at the discharge pressure,
  J/kg (from a caller's pressure-entropy flash; `h2s > h1` for compression).
- `adiabatic_efficiency`: isentropic efficiency, dimensionless in \[0, 1\].
  Must be > 0 (an `eta = 0` machine is unphysical and yields `+inf`).

```rust
pub fn consumed_power(w: uom::si::f64::MassRate, h1: uom::si::f64::AvailableEnergy, h2s: uom::si::f64::AvailableEnergy, adiabatic_efficiency: uom::si::f64::Ratio) -> uom::si::f64::Power { /* ... */ }
```

#### Function `outlet_enthalpy`

Actual outlet specific enthalpy `h2 = h1 + W / w`, given the consumed power
from [`consumed_power`].

**Physical quantity:** discharge specific enthalpy, J/kg.

Mirrors DWSIM `Compressor.vb:969` (`H2 = Hi + DeltaQ / Wi`). Because a
compressor consumes power (`W > 0`), the outlet enthalpy is **higher** than
the inlet (`h2 > h1`) -- the opposite of the expander.

**Inputs / units:**
- `h1`: inlet specific enthalpy, J/kg.
- `consumed`: shaft power consumed, W (from [`consumed_power`]).
- `w`: mass flow, kg/s (> 0).

```rust
pub fn outlet_enthalpy(h1: uom::si::f64::AvailableEnergy, consumed: uom::si::f64::Power, w: uom::si::f64::MassRate) -> uom::si::f64::AvailableEnergy { /* ... */ }
```

#### Function `isentropic_exponent`

Isentropic polytropic exponent `n_isentropic = ln(p2/p1) / ln(rho2i/rho1)`,
from the inlet density `rho1` and the density at the *ideal* isentropic
outlet state `rho2_ideal`.

**Physical quantity:** dimensionless volume (path) exponent for the ideal,
constant-entropy compression path.

Mirrors DWSIM `Compressor.vb:995`
(`n_isent = Math.Log(P2 / Pi) / Math.Log(rho2i / rho1)`).

**Inputs / units:** all pressures in Pa, all densities in kg/m^3. For a
compressor `p2 > p1` and `rho2_ideal > rho1`, so both logs are positive and
`n_isentropic > 0` (typically ~1.1-1.7 for gases).

```rust
pub fn isentropic_exponent(p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, rho1: uom::si::f64::MassDensity, rho2_ideal: uom::si::f64::MassDensity) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `polytropic_exponent`

Polytropic exponent `n_polytropic = ln(p2/p1) / ln(rho2/rho1)`, from the
inlet density `rho1` and the density at the *actual* outlet state `rho2`.

**Physical quantity:** dimensionless volume (path) exponent for the real
compression path (which sits between the ideal isentropic and an isothermal
path, reflecting irreversibility).

Mirrors DWSIM `Compressor.vb:999`
(`n_poly = Math.Log(P2 / Pi) / Math.Log(rho2 / rho1)`).

**Inputs / units:** pressures in Pa, densities in kg/m^3. `n_polytropic > 0`
for a real compression (`p2 > p1`, `rho2 > rho1`).

```rust
pub fn polytropic_exponent(p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, rho1: uom::si::f64::MassDensity, rho2: uom::si::f64::MassDensity) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `schultz_correction_factor`

Schultz (ASME PTC-10-style) polytropic work-correction factor:
```text
f_ce = [(p2/p1)^((n_p-1)/n_p) - 1] * [(n_p/(n_p-1)) * (n_i-1)/n_i]
       / [(p2/p1)^((n_i-1)/n_i) - 1]
```

**Physical quantity:** dimensionless correction relating the polytropic and
adiabatic efficiencies of a real gas compression. For a compressor DWSIM
uses `eta_polytropic = eta_adiabatic * f_ce` (Compressor.vb:1003), i.e. the
polytropic efficiency exceeds the adiabatic one by `f_ce` (> 1 for a real
compression -- reheat effect). Equivalently the iteration below inverts it:
`eta_adiabatic = eta_polytropic / f_ce`.

Mirrors DWSIM `Compressor.vb:1001`.

**Inputs / units:** pressures in Pa; exponents dimensionless (`Ratio`).

```rust
pub fn schultz_correction_factor(p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, n_isentropic: uom::si::f64::Ratio, n_polytropic: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `solve_polytropic_efficiency`

Iteratively resolve the adiabatic efficiency consistent with a specified
polytropic efficiency for a compression, mirroring DWSIM `Compressor.vb`
`ProcessPathType.Polytropic` loop (Compressor.vb:885-947).

DWSIM seeds `AdiabaticEfficiency = PolytropicEfficiency`, then repeats:
compute the actual outlet state at `p2` (a pressure-enthalpy flash), read
its density `rho2`, form `n_isent`, `n_poly`, and `fce`, and update
`AdiabaticEfficiency = PolytropicEfficiency / fce` until the efficiency
change is below `0.00001`. This port keeps that structure but pushes the
single flash-dependent step out to the caller.

`evaluate_outlet` is that flash-dependent step: given a trial
`adiabatic_efficiency`, it must return `(h2, rho2)` -- the actual outlet
specific enthalpy (J/kg) and outlet density (kg/m^3) at `p2` for that trial
efficiency. A caller with flash access computes this via a pressure-enthalpy
flash at `(p2, outlet_enthalpy(h1, consumed_power(w, h1, h2s, eta), w))`,
then reads the density at that state.

**Inputs / units:** `w` kg/s; `p1`, `p2` Pa (`p2 > p1`); `h1`, `h2s` J/kg
(`h2s > h1`); `rho1`, `rho2_ideal` kg/m^3; `polytropic_efficiency`,
`tolerance` dimensionless `Ratio`. The loop is capped at 100 iterations
(matching the fixed-point nature of the DWSIM `Do ... Loop`).

Returns a [`PolytropicResult`]; if the loop fails to converge within 100
iterations, `schultz_factor` is `NaN` and the last iterate is returned.

```rust
pub fn solve_polytropic_efficiency<F>(w: uom::si::f64::MassRate, p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, h1: uom::si::f64::AvailableEnergy, h2s: uom::si::f64::AvailableEnergy, rho1: uom::si::f64::MassDensity, rho2_ideal: uom::si::f64::MassDensity, polytropic_efficiency: uom::si::f64::Ratio, evaluate_outlet: F, tolerance: uom::si::f64::Ratio) -> PolytropicResult
where
    F: FnMut(uom::si::f64::Ratio) -> (uom::si::f64::AvailableEnergy, uom::si::f64::MassDensity) { /* ... */ }
```

## Module `cooler`

Cooler: enthalpy-driven cooling duty and outlet-state relations.

Ported from DWSIM `UnitOperations/Cooler.vb` (GPL-3.0), the
`Public Overrides Sub Calculate` routine (Cooler.vb:~415-720). The cooler
is the near-mirror of the heater ([`crate::heater`]): the same energy
balance with the duty sign flipped. As with the heater, this crate has no
property-package / flash access, so flash-dependent steps are pushed to the
caller — specific enthalpies / mass flow / Cp are taken as inputs, and the
rigorous "outlet temperature given" path takes a caller-supplied flash
closure (a generic `Fn`, not a trait object, per the workspace no-`dyn`
rule).

# Sign convention

A **cooler removes duty**: the reported heat duty `Q` is positive for heat
*removed* from the stream, and removing heat lowers the specific enthalpy,
so the outlet enthalpy `h2 <= h1` for `Q >= 0`. This mirrors DWSIM, whose
cooler balance is `H2 = -Q*(eta/100)/W + H1` (Cooler.vb:498) — the leading
minus is the only structural difference from the heater
([`crate::heater::outlet_enthalpy_heat_added`], Heater.vb:475). Likewise the
duty from an outlet spec is `Q = -w*(h2-h1)/eta` (Cooler.vb:561), positive
when `h2 < h1`.

# Efficiency

Identical semantics to the heater: DWSIM's percentage `Eficiencia`
(default 100) is taken here as a dimensionless [`Ratio`] (`1.0` = 100%), the
fraction of the utility duty coupled to the fluid. `efficiency = 0` is
degenerate (no heat removed in the duty-given path; non-finite duty in the
spec-given path).

# Pressure drop & temperature change

`p2 = p1 - dp` (Cooler.vb:436) and `T2 = T1 + dT` (Cooler.vb:587) are
identical to the heater's, so they are re-exported from [`crate::heater`]
rather than duplicated: see [`outlet_pressure`] and
[`outlet_temperature_from_change`]. This is the only cross-file dependency;
the duty/enthalpy math below is cooler-specific (opposite sign).

# Excluded DWSIM behaviour

As with the heater, the GUI editor, XML/JSON serialization, property-grid
reflection, report builders, energy-stream flowsheet plumbing, and the
dynamic-mode accumulation model are NOT ported — they are application/solver
infrastructure, not the thermodynamic kernel. The `EnergyStream` and
`OutletVaporFraction` modes require an internal flash with no closed-form
duty relation and are left to the caller (`EnergyStream` is identical to
`HeatRemoved` once the duty is read from the energy stream — but note it
*adds* enthalpy in DWSIM, using the heater sign, Cooler.vb:460).

```rust
pub mod cooler { /* ... */ }
```

### Functions

#### Function `outlet_enthalpy_heat_removed`

Outlet specific enthalpy for the **heat-removed** mode (duty given):
`h2 = h1 - Q * eta / w`.

Ported from Cooler.vb:498
(`H2 = -Me.DeltaQ * (Me.Eficiencia / 100) / Wi + Hi`). This is the exact
mirror of [`crate::heater::outlet_enthalpy_heat_added`] with the duty sign
flipped: a cooler *removes* duty, so `h2 <= h1` for `q >= 0`.

- `h1` — inlet specific enthalpy \[J/kg\].
- `q` — heat duty \[W\]; positive **removes** heat.
- `w` — mass flow rate \[kg/s\]; must be `> 0`.
- `efficiency` — fraction of duty coupled to the fluid, `1.0` = 100%.

```rust
pub fn outlet_enthalpy_heat_removed(h1: uom::si::f64::AvailableEnergy, q: uom::si::f64::Power, w: uom::si::f64::MassRate, efficiency: uom::si::f64::Ratio) -> uom::si::f64::AvailableEnergy { /* ... */ }
```

#### Function `duty_from_outlet_enthalpy`

Heat duty from a known outlet specific enthalpy (rigorous "outlet spec"
direction): `Q = -w * (h2 - h1) / eta`.

Ported from Cooler.vb:561
(`Me.DeltaQ = -(H2 - Hi) / (Me.Eficiencia / 100) * Wi`). The outlet enthalpy
`h2` comes from the caller's flash at the specified outlet temperature (or
vapour fraction); the same relation serves the `OutletTemperature` and
`TemperatureChange` modes (Cooler.vb:561 and 606). The leading minus makes
the reported duty positive when the stream is cooled (`h2 < h1`).

`efficiency = 0` gives a non-finite duty.

```rust
pub fn duty_from_outlet_enthalpy(h1: uom::si::f64::AvailableEnergy, h2: uom::si::f64::AvailableEnergy, w: uom::si::f64::MassRate, efficiency: uom::si::f64::Ratio) -> uom::si::f64::Power { /* ... */ }
```

#### Function `duty_constant_cp`

Heat duty for the "outlet temperature given" mode under a **constant-`Cp`
approximation**: `Q = -w * Cp * (T2 - T1) / eta`.

Port-added closed-form stand-in for the rigorous PT-flash path
(Cooler.vb:557-561); DWSIM has no constant-`Cp` branch. Valid only where
`Cp` is sensibly constant over `[T1, T2]` (no phase change, mild range). For
the rigorous path use [`duty_from_outlet_temperature`]. Positive duty for
cooling (`T2 < T1`), matching the cooler sign convention.

- `w` — mass flow rate \[kg/s\].
- `cp` — mean specific heat capacity \[J/(kg·K)\] over the interval.
- `t1`, `t2` — inlet / outlet temperatures \[K\].
- `efficiency` — utility-to-fluid fraction, `1.0` = 100%.

```rust
pub fn duty_constant_cp(w: uom::si::f64::MassRate, cp: uom::si::f64::SpecificHeatCapacity, t1: uom::si::f64::ThermodynamicTemperature, t2: uom::si::f64::ThermodynamicTemperature, efficiency: uom::si::f64::Ratio) -> uom::si::f64::Power { /* ... */ }
```

#### Function `duty_from_outlet_temperature`

Heat duty for the "outlet temperature given" mode, **rigorous** path: the
caller supplies the outlet enthalpy at `t2` via a flash closure.

Mirrors Cooler.vb:557-561, where DWSIM does a PT flash at `(P2, T2)` to get
`H2` and then `Me.DeltaQ = -(H2 - Hi) / (eta/100) * Wi`. `outlet_enthalpy_at`
is the caller's PT-flash step (a generic `Fn`, no `dyn`): given the
specified outlet temperature it returns the outlet specific enthalpy `h2`.
The returned duty equals [`duty_from_outlet_enthalpy`] at the flashed `h2`.

```rust
pub fn duty_from_outlet_temperature<F>(h1: uom::si::f64::AvailableEnergy, w: uom::si::f64::MassRate, efficiency: uom::si::f64::Ratio, t2: uom::si::f64::ThermodynamicTemperature, outlet_enthalpy_at: F) -> uom::si::f64::Power
where
    F: Fn(uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::AvailableEnergy { /* ... */ }
```

### Re-exports

#### Re-export `outlet_pressure`

```rust
pub use crate::heater::outlet_pressure;
```

#### Re-export `outlet_temperature_from_change`

```rust
pub use crate::heater::outlet_temperature_from_change;
```

## Module `expander`

Turbine/expander thermodynamics: isentropic expansion with adiabatic or
Schultz-corrected polytropic efficiency.

Ported from DWSIM `UnitOperations/Expander.vb` -- see `isentropic`'s
module doc for the full source mapping, the flash-dependency boundary,
and the sign convention this port uses. DWSIM's `Curves` calculation mode
(Floater-Hormann rational interpolation of head/efficiency/power vs.
flow) is not ported -- see the workspace's `op-qo2.9` bead.

```rust
pub mod expander { /* ... */ }
```

### Modules

## Module `isentropic`

Isentropic expansion + Schultz polytropic-efficiency correction, for a
turbine/expander.

Ported from DWSIM `UnitOperations/Expander.vb`. DWSIM computes the ideal
outlet enthalpy `H2s` via a pressure-entropy (isentropic) flash and the
actual outlet state via repeated pressure-enthalpy flashes -- this
equipment port is deliberately kept decoupled from the crate's flash kernel
([`crate::thermo`]), so the flash-dependent steps are pushed to the caller:
the functions below take already-known enthalpies/densities as inputs,
and [`solve_polytropic_efficiency`] takes a caller-supplied closure for
the one flash-dependent step in DWSIM's iteration (not a trait object --
a generic `Fn` parameter, per the workspace's no-`dyn`-dispatch rule).

**Sign convention** (chosen for this port, not necessarily matching
DWSIM's own internal accounting byte-for-byte): [`generated_power`] and
the polytropic solver's duty are **positive when the expander extracts
power from the fluid** (the physically intuitive case, `H2 < H1`).

```rust
pub mod isentropic { /* ... */ }
```

### Types

#### Struct `PolytropicResult`

Result of [`solve_polytropic_efficiency`].

```rust
pub struct PolytropicResult {
    pub adiabatic_efficiency: uom::si::f64::Ratio,
    pub schultz_factor: uom::si::f64::Ratio,
    pub generated: uom::si::f64::Power,
    pub h2: uom::si::f64::AvailableEnergy,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `adiabatic_efficiency` | `uom::si::f64::Ratio` | Converged adiabatic efficiency `η_adiabatic = η_polytropic / f_ce`. |
| `schultz_factor` | `uom::si::f64::Ratio` | Schultz correction factor at convergence. |
| `generated` | `uom::si::f64::Power` | Generated power at convergence. |
| `h2` | `uom::si::f64::AvailableEnergy` | Actual outlet enthalpy at convergence. |

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
    fn clone(self: &Self) -> PolytropicResult { /* ... */ }
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
    fn eq(self: &Self, other: &PolytropicResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `generated_power`

Power generated (extracted from the fluid) by an adiabatic expansion from
`h1` to the isentropic outlet enthalpy `h2s`, at `adiabatic_efficiency`
\[0, 1\]: `W_gen = -w (h2s - h1) η`. Positive for a normal expansion
(`h2s < h1`).

```rust
pub fn generated_power(w: uom::si::f64::MassRate, h1: uom::si::f64::AvailableEnergy, h2s: uom::si::f64::AvailableEnergy, adiabatic_efficiency: uom::si::f64::Ratio) -> uom::si::f64::Power { /* ... */ }
```

#### Function `outlet_enthalpy`

Actual outlet enthalpy `h2 = h1 - W_gen/w`, given the generated power
from [`generated_power`].

```rust
pub fn outlet_enthalpy(h1: uom::si::f64::AvailableEnergy, generated: uom::si::f64::Power, w: uom::si::f64::MassRate) -> uom::si::f64::AvailableEnergy { /* ... */ }
```

#### Function `isentropic_exponent`

Isentropic polytropic exponent `n_isentropic = ln(p2/p1) / ln(ρ2i/ρ1)`,
from the inlet density `rho1` and the density at the *ideal* isentropic
outlet state `rho2_ideal`.

```rust
pub fn isentropic_exponent(p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, rho1: uom::si::f64::MassDensity, rho2_ideal: uom::si::f64::MassDensity) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `polytropic_exponent`

Polytropic exponent `n_polytropic = ln(p2/p1) / ln(ρ2/ρ1)`, from the
inlet density `rho1` and the density at the *actual* outlet state `rho2`.

```rust
pub fn polytropic_exponent(p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, rho1: uom::si::f64::MassDensity, rho2: uom::si::f64::MassDensity) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `schultz_correction_factor`

Schultz (ASME PTC-10-style) polytropic work-correction factor:
```text
f_ce = [(p2/p1)^((n_p-1)/n_p) - 1] * [(n_p/(n_p-1)) * (n_i-1)/n_i]
       / [(p2/p1)^((n_i-1)/n_i) - 1]
```
Relates polytropic efficiency to adiabatic efficiency:
`η_adiabatic = η_polytropic / f_ce`.

```rust
pub fn schultz_correction_factor(p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, n_isentropic: uom::si::f64::Ratio, n_polytropic: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `solve_polytropic_efficiency`

Iteratively resolve the adiabatic efficiency that is consistent with a
specified polytropic efficiency, mirroring DWSIM's `Expander.vb`
`ProcessPathType.Polytropic` loop.

`evaluate_outlet` is the one flash-dependent step: given a trial
`adiabatic_efficiency`, it must return `(h2, rho2)` -- the actual outlet
enthalpy and density at `p2` for that trial efficiency (a caller with
flash access computes this via a pressure-enthalpy flash at
`(p2, outlet_enthalpy(h1, generated_power(w,h1,h2s,eta), w))`, then reads
density at that state).

```rust
pub fn solve_polytropic_efficiency<F>(w: uom::si::f64::MassRate, p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, h1: uom::si::f64::AvailableEnergy, h2s: uom::si::f64::AvailableEnergy, rho1: uom::si::f64::MassDensity, rho2_ideal: uom::si::f64::MassDensity, polytropic_efficiency: uom::si::f64::Ratio, evaluate_outlet: F, tolerance: uom::si::f64::Ratio) -> PolytropicResult
where
    F: FnMut(uom::si::f64::Ratio) -> (uom::si::f64::AvailableEnergy, uom::si::f64::MassDensity) { /* ... */ }
```

## Module `heat_exchanger`

Heat exchanger rating: LMTD, epsilon-NTU effectiveness, the
Bowman/Underwood multi-pass LMTD correction factor, and Tinker's
(simplified) shell-and-tube method.

Ported from DWSIM `UnitOperations/HeatExchanger.vb`. See
[`tinker_shell_and_tube`]'s module doc for that method's full source
mapping and the outer-convergence-loop flash-dependency boundary.

```rust
pub mod heat_exchanger { /* ... */ }
```

### Modules

## Module `f_correction`

Bowman/Underwood multi-pass LMTD correction factor `F`, for a shell with
`N` shell passes in series (each with 2, 4, 6, ... tube passes).

Ported from DWSIM `UnitOperations/HeatExchanger.vb`'s
`ShellandTube_Rating` (the F-factor sub-step; the surrounding Tinker
shell-side correlations are not ported, see `op-qo2.7`). Standard TEMA
reference formula (Bowman, Mueller & Nagle 1940).

```rust
pub mod f_correction { /* ... */ }
```

### Functions

#### Function `f_correction_factor`

Multi-pass LMTD correction factor `F` \[0, 1\] for `n_shell_passes`
shells in series, given the four terminal temperatures. Multiply the
single-pass counter-current LMTD (see
[`crate::heat_exchanger::lmtd::lmtd`]) by this factor to get the
effective mean temperature difference for a multi-pass shell-and-tube
exchanger: `ΔT_m = F * LMTD`.

`t_shell_in`/`t_shell_out` are the shell-side fluid's inlet/outlet
temperatures; `t_tube_in`/`t_tube_out` the tube-side fluid's.

```rust
pub fn f_correction_factor(n_shell_passes: u32, t_shell_in: uom::si::f64::ThermodynamicTemperature, t_shell_out: uom::si::f64::ThermodynamicTemperature, t_tube_in: uom::si::f64::ThermodynamicTemperature, t_tube_out: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::Ratio { /* ... */ }
```

## Module `lmtd`

Log-mean temperature difference (LMTD) rating.

Ported from DWSIM `UnitOperations/HeatExchanger.vb`'s `Calculate()`
preamble (the co/counter-current LMTD forms shared by every calculation
mode in that file).

```rust
pub mod lmtd { /* ... */ }
```

### Types

#### Enum `FlowArrangement`

Which end of the exchanger the hot and cold streams enter from.

```rust
pub enum FlowArrangement {
    CoCurrent,
    CounterCurrent,
}
```

##### Variants

###### `CoCurrent`

Hot and cold streams flow in the same direction.

###### `CounterCurrent`

Hot and cold streams flow in opposite directions (the common case,
and always more effective than co-current for the same `U`, `A`).

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
    fn clone(self: &Self) -> FlowArrangement { /* ... */ }
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
    fn eq(self: &Self, other: &FlowArrangement) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `lmtd`

Log-mean temperature difference for a 1-1 (single-pass) exchanger.

`t_hot_in`/`t_hot_out` are the hot stream's inlet/outlet temperatures;
`t_cold_in`/`t_cold_out` the cold stream's. Returns `TemperatureInterval`
(a magnitude, not an absolute temperature).

```rust
pub fn lmtd(arrangement: FlowArrangement, t_hot_in: uom::si::f64::ThermodynamicTemperature, t_hot_out: uom::si::f64::ThermodynamicTemperature, t_cold_in: uom::si::f64::ThermodynamicTemperature, t_cold_out: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::TemperatureInterval { /* ... */ }
```

#### Function `duty`

Heat duty `Q = U A (LMTD) F`, where `F` is an optional multi-pass
correction factor (see [`crate::heat_exchanger::f_correction`]; pass
`Ratio::new::<ratio>(1.0)` for a true 1-1 exchanger).

```rust
pub fn duty(overall_coefficient: uom::si::f64::HeatTransfer, area: uom::si::f64::Area, lmtd: uom::si::f64::TemperatureInterval, f_correction: uom::si::f64::Ratio) -> uom::si::f64::Power { /* ... */ }
```

#### Function `overall_coefficient_from_duty`

Overall heat-transfer coefficient `U = Q / (A * LMTD * F)` -- the inverse
of [`duty`], used when `Q` and `A` are known and `U` is the sought output
(DWSIM's `CalcTempHotOut`/`CalcTempColdOut`/`CalcBothTemp` modes).

```rust
pub fn overall_coefficient_from_duty(duty: uom::si::f64::Power, area: uom::si::f64::Area, lmtd: uom::si::f64::TemperatureInterval, f_correction: uom::si::f64::Ratio) -> uom::si::f64::HeatTransfer { /* ... */ }
```

#### Function `area_from_duty`

Heat-transfer area `A = Q / (U * LMTD * F)` -- used when `Q` and `U` are
known and `A` is the sought output (DWSIM's `CalcArea`/`ThermalEfficiency`
modes).

```rust
pub fn area_from_duty(duty: uom::si::f64::Power, overall_coefficient: uom::si::f64::HeatTransfer, lmtd: uom::si::f64::TemperatureInterval, f_correction: uom::si::f64::Ratio) -> uom::si::f64::Area { /* ... */ }
```

## Module `ntu_effectiveness`

epsilon-NTU (number of transfer units) effectiveness method for a 1-1
(single-pass) co- or counter-current heat exchanger.

Ported from DWSIM `UnitOperations/HeatExchanger.vb`'s `CalcBothTemp_UA`
mode, which iterates because it derives heat-capacity rates from
flash-derived enthalpy slopes (non-constant, real-fluid `Cp`). This heat-
exchanger port is deliberately kept decoupled from the crate's flash kernel
([`crate::thermo`]), so this port takes the
heat-capacity rates `C_hot`/`C_cold` \[W/K\] as given (the standard
textbook epsilon-NTU form -- Kays & London / Incropera -- which DWSIM's
own per-stream effectiveness formulas reduce to when `C` is constant); a
caller with flash access (e.g. `tampines`) can supply a locally
linearised `C = W (H2-H1)/(T2-T1)` the same way DWSIM does, or a plain
`W * Cp` for ideal/near-ideal fluids.

```rust
pub mod ntu_effectiveness { /* ... */ }
```

### Types

#### Struct `NtuResult`

Result of an epsilon-NTU evaluation.

```rust
pub struct NtuResult {
    pub ntu: uom::si::f64::Ratio,
    pub effectiveness: uom::si::f64::Ratio,
    pub duty: uom::si::f64::Power,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ntu` | `uom::si::f64::Ratio` | Number of transfer units, `NTU = UA / C_min`. |
| `effectiveness` | `uom::si::f64::Ratio` | Heat-exchanger effectiveness `ε = Q / Q_max`, \[0, 1\]. |
| `duty` | `uom::si::f64::Power` | Actual heat duty `Q = ε C_min ΔT_max`. |

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
    fn clone(self: &Self) -> NtuResult { /* ... */ }
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
    fn eq(self: &Self, other: &NtuResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `evaluate`

Evaluate epsilon-NTU effectiveness and duty for a 1-1 exchanger, given
the overall conductance `ua = U * A`, both streams' heat-capacity rates,
and the maximum possible temperature difference `delta_t_max = T_hot_in -
T_cold_in` (always positive by definition of "hot"/"cold").

```rust
pub fn evaluate(arrangement: super::lmtd::FlowArrangement, ua: uom::si::f64::ThermalConductance, c_hot: uom::si::f64::ThermalConductance, c_cold: uom::si::f64::ThermalConductance, delta_t_max: uom::si::f64::TemperatureInterval) -> NtuResult { /* ... */ }
```

#### Function `outlet_temperature_changes`

Outlet temperature changes implied by an [`NtuResult`]:
`ΔT_hot = Q/C_hot`, `ΔT_cold = Q/C_cold` (hot stream cools by
`ΔT_hot`, cold stream warms by `ΔT_cold`).

```rust
pub fn outlet_temperature_changes(result: &NtuResult, c_hot: uom::si::f64::ThermalConductance, c_cold: uom::si::f64::ThermalConductance) -> (uom::si::f64::TemperatureInterval, uom::si::f64::TemperatureInterval) { /* ... */ }
```

## Module `tinker_shell_and_tube`

Tinker's method (simplified), shell-and-tube heat exchanger rating.

Ported from DWSIM `UnitOperations/HeatExchanger.vb`'s
`ShellandTube_Rating`/`ShellandTube_CalcFoulingFactor` calculation modes
(both are `HeatExchangerCalcMode` branches inside one `Select Case` at
lines 2051-2635 of that file, itself inside `Calculate` at lines
1206-2751) and the `STHXProperties` geometry class (lines 3469-3537).
Bell-Delaware-like shell-side treatment citing Tinker, ch. 5.

This module covers the **self-contained geometric/correlation building
blocks**: tube-side Reynolds/friction/HTC, the shell-side `Nh`/`Y`/`Np`
regressions, Colburn `j_h` and shell friction factor `f_s` (both
piecewise by tube layout and Reynolds/pitch-ratio bins), the shell-side
HTC with its baffle-leakage correction `E_c`, shell/tube pressure drop,
and the overall-`U`/fouling-factor formulas. It does **not** replicate
DWSIM's outer convergence loop (`Do ... Loop Until fx < 0.001`, max 100
iterations, converging outlet temperatures in Rating mode or `U` in
CalcFoulingFactor mode) -- that loop re-flashes both streams' properties
at the current mean temperature every iteration, which needs a
property-package/flash this crate intentionally does not have (see this
crate's top-level doc). A caller with flash access (e.g. `tampines`)
assembles [`tube_side`]/[`shell_side`]/[`overall_coefficient`] plus
[`super::f_correction::f_correction_factor`] (the same Bowman/Underwood
multi-pass `F` factor DWSIM uses here) into that outer loop itself.

# Known dead inputs (confirmed unread in the DWSIM source)
`Shell_BaffleType`, `Shell_BaffleOrientation`, and `Shell_Roughness` are
declared in `STHXProperties` but never used by either calculation mode --
not represented in [`ShellAndTubeGeometry`].

# A note on `d_e`
DWSIM does **not** use a classical Kern-style layout-dependent hydraulic
diameter here -- `d_e` is simply the tube outer diameter, reused directly
everywhere (Reynolds, Prandtl, `j_h`, `f_s`, pressure drop, HTC). This is
a real property of the ported method, not an omission.

```rust
pub mod tinker_shell_and_tube { /* ... */ }
```

### Types

#### Struct `FoulingFactor`

Fouling resistance, area-specific \[K m^2 / W\]. `uom` has no dedicated
quantity for this (its `ThermalResistance` is K/W for a whole object,
dimensionally different from the per-unit-area R-value used here), so
this is a documented newtype rather than a bare `f64`.

```rust
pub struct FoulingFactor(pub f64);
```

##### Fields

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
    fn clone(self: &Self) -> FoulingFactor { /* ... */ }
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
    fn eq(self: &Self, other: &FoulingFactor) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &FoulingFactor) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `TubeLayout`

Tube bundle layout pattern (`STHXProperties.Tube_Layout` 0/1/2/3).

```rust
pub enum TubeLayout {
    Triangular30,
    TriangularRotated,
    Square90,
    SquareRotated45,
}
```

##### Variants

###### `Triangular30`

Triangular, 30 degree pitch angle.

###### `TriangularRotated`

Triangular, rotated (60 degree effective).

###### `Square90`

Square, 90 degree pitch angle.

###### `SquareRotated45`

Square, rotated 45 degrees.

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
    fn clone(self: &Self) -> TubeLayout { /* ... */ }
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
    fn eq(self: &Self, other: &TubeLayout) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ShellAndTubeGeometry`

Shell-and-tube exchanger geometry (`uom`-typed `STHXProperties` fields
this module's correlations actually use).

```rust
pub struct ShellAndTubeGeometry {
    pub number_of_shells_in_series: u32,
    pub shell_passes: u32,
    pub shell_diameter: uom::si::f64::Length,
    pub shell_fouling: FoulingFactor,
    pub shell_baffle_cut: uom::si::f64::Ratio,
    pub shell_baffle_spacing: uom::si::f64::Length,
    pub tube_inner_diameter: uom::si::f64::Length,
    pub tube_outer_diameter: uom::si::f64::Length,
    pub tube_length: uom::si::f64::Length,
    pub tube_passes_per_shell: u32,
    pub tube_number_per_shell: u32,
    pub tube_layout: TubeLayout,
    pub tube_pitch: uom::si::f64::Length,
    pub tube_roughness: uom::si::f64::Length,
    pub tube_friction_correction_factor: uom::si::f64::Ratio,
    pub tube_thermal_conductivity: uom::si::f64::ThermalConductivity,
    pub tube_fouling: FoulingFactor,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `number_of_shells_in_series` | `u32` | Number of shells connected in series, `Nc`. |
| `shell_passes` | `u32` | Number of shell-side passes (drives the Bowman/Underwood `F` factor). |
| `shell_diameter` | `uom::si::f64::Length` | Shell internal diameter. |
| `shell_fouling` | `FoulingFactor` | Shell-side fouling resistance, `r_s`. |
| `shell_baffle_cut` | `uom::si::f64::Ratio` | Baffle cut, as a fraction of shell diameter \[0, 1\]. |
| `shell_baffle_spacing` | `uom::si::f64::Length` | Baffle spacing. |
| `tube_inner_diameter` | `uom::si::f64::Length` | Tube internal diameter, `d_i`. |
| `tube_outer_diameter` | `uom::si::f64::Length` | Tube outer diameter, `d_e` (also DWSIM's shell-side "equivalent<br>diameter" -- see this module's doc). |
| `tube_length` | `uom::si::f64::Length` | Tube length. |
| `tube_passes_per_shell` | `u32` | Tube passes per shell. |
| `tube_number_per_shell` | `u32` | Total tube count per shell. |
| `tube_layout` | `TubeLayout` | Tube bundle layout. |
| `tube_pitch` | `uom::si::f64::Length` | Tube pitch (centre-to-centre spacing). |
| `tube_roughness` | `uom::si::f64::Length` | Tube absolute roughness (default 0.045 mm in DWSIM). |
| `tube_friction_correction_factor` | `uom::si::f64::Ratio` | User-exposed empirical multiplier on computed tube-side friction<br>factor (DWSIM default 1.2 -- a pure fudge factor, not a computed<br>correction; applies to tube side only, not shell side). |
| `tube_thermal_conductivity` | `uom::si::f64::ThermalConductivity` | Tube wall thermal conductivity, `k_t`. |
| `tube_fouling` | `FoulingFactor` | Tube-side fouling resistance, `r_t`. |

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
    fn clone(self: &Self) -> ShellAndTubeGeometry { /* ... */ }
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
    fn eq(self: &Self, other: &ShellAndTubeGeometry) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ShellAndTubeError`

Errors from the shell-side correlations.

```rust
pub enum ShellAndTubeError {
    PitchToDiameterRatioTooLarge {
        pitch_over_de: f64,
    },
}
```

##### Variants

###### `PitchToDiameterRatioTooLarge`

`pitch / tube_outer_diameter` exceeded 1.5, the upper bound DWSIM's
shell friction-factor correlation covers (DWSIM: `Throw New
Exception("ratio between tube spacing and tube external diameter
needs to be <= 1.5")`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pitch_over_de` | `f64` | The out-of-range `pitch / tube_outer_diameter` ratio. |

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
    fn clone(self: &Self) -> ShellAndTubeError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ShellAndTubeError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TubeSideResult`

Result of [`tube_side`].

```rust
pub struct TubeSideResult {
    pub reynolds: uom::si::f64::Ratio,
    pub friction_factor: uom::si::f64::Ratio,
    pub htc: uom::si::f64::HeatTransfer,
    pub pressure_drop: uom::si::f64::Pressure,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reynolds` | `uom::si::f64::Ratio` | Tube-side Reynolds number. |
| `friction_factor` | `uom::si::f64::Ratio` | Darcy friction factor, after `tube_friction_correction_factor`. |
| `htc` | `uom::si::f64::HeatTransfer` | Tube-side heat-transfer coefficient (Petukhov 1970). |
| `pressure_drop` | `uom::si::f64::Pressure` | Tube-side pressure drop (Darcy-Weisbach, `tube_passes_per_shell`<br>equivalent lengths). |

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
    fn clone(self: &Self) -> TubeSideResult { /* ... */ }
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
    fn eq(self: &Self, other: &TubeSideResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ShellSideResult`

Result of [`shell_side`].

```rust
pub struct ShellSideResult {
    pub htc: uom::si::f64::HeatTransfer,
    pub pressure_drop: uom::si::f64::Pressure,
    pub reynolds_friction: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `htc` | `uom::si::f64::HeatTransfer` | Shell-side heat-transfer coefficient, baffle-leakage-corrected. |
| `pressure_drop` | `uom::si::f64::Pressure` | Shell-side pressure drop (all shells in series). |
| `reynolds_friction` | `uom::si::f64::Ratio` | Shell-side crossflow Reynolds number (friction context). |

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
    fn clone(self: &Self) -> ShellSideResult { /* ... */ }
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
    fn eq(self: &Self, other: &ShellSideResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `tube_side`

Tube-side Reynolds/friction-factor/HTC/pressure-drop, for the fluid
flowing through the tubes at mass flow `w`, density `rho`, viscosity
`mu`, thermal conductivity `k`, and specific heat `cp`.

Friction factor: the same two-branch explicit Colebrook-type fit as
[`super::super::pipe::friction_factor::darcy_friction_factor`] (confirmed
identical constants against DWSIM `Pipe.vb`), Reynolds-gated at 3250 (not
2100/4000 as in the pipe module -- this exchanger method uses a single
threshold), then scaled by `tube_friction_correction_factor`.

HTC: genuine Petukhov (1970) form, `h_i = (k/d_i)(f/8) Re Pr / (1.07 +
12.7 (f/8)^0.5 (Pr^(2/3)-1))` -- not Dittus-Boelter, and not the
Gnielinski-form correlation DWSIM's own `Pipe.vb` confusingly also calls
`hint_petukhov` (that one has a `(Re-1000)` term and constant `1.0`; this
one does not and is not Reynolds-gated).

```rust
pub fn tube_side(geometry: &ShellAndTubeGeometry, w: uom::si::f64::MassRate, rho: uom::si::f64::MassDensity, mu: uom::si::f64::DynamicViscosity, k: uom::si::f64::ThermalConductivity, cp: uom::si::f64::SpecificHeatCapacity) -> TubeSideResult { /* ... */ }
```

#### Function `nh_y_np`

Shell-side geometry factors `(Nh, Y, Np)`, from `xx = D_shell /
baffle_spacing` and `yy = pitch / d_e` via layout-dependent power-law
fits (DWSIM lines ~2302-2358).

```rust
pub fn nh_y_np(layout: TubeLayout, shell_diameter: uom::si::f64::Length, baffle_spacing: uom::si::f64::Length, pitch: uom::si::f64::Length, tube_outer_diameter: uom::si::f64::Length) -> (uom::si::f64::Ratio, uom::si::f64::Ratio, uom::si::f64::Ratio) { /* ... */ }
```

#### Function `colburn_j_h_friction`

Colburn `j_h` for the shell-side pressure-drop (crossflow) context --
two Reynolds bins (`< 100`, `>= 100`), separate fits per layout (DWSIM
lines ~2372-2379). **Not the same fit** as [`colburn_j_h_htc`] (see that
function's doc for the discrepancy DWSIM's own source has between the two
contexts).

```rust
pub fn colburn_j_h_friction(layout: TubeLayout, re_shell: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `colburn_j_h_htc`

Colburn `j_h` for the shell-side heat-transfer-coefficient context.

**Not identical to [`colburn_j_h_friction`]**: DWSIM recomputes `j_h`
separately here from a different Reynolds number (`Rsh`, based on the
leakage-corrected area `Ssh` rather than `Ssf`), and for the triangular
layouts the high-Reynolds exponent is **0.61**, not 0.59 as in the
friction context (confirmed against DWSIM source -- an easy point to
silently merge when porting, so kept as two functions here). Square and
square-rotated layouts share one undifferentiated fit in this context
(DWSIM does not split `Square90`/`SquareRotated45` here, unlike
[`colburn_j_h_friction`]).

```rust
pub fn colburn_j_h_htc(layout: TubeLayout, re_shell_htc: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `shell_friction_factor`

Shell-side Darcy friction factor `f_s`, piecewise in Reynolds number
(`< 100`, `< 1000`, `>= 1000`) and in `pitch/d_e` bins (`<= 1.2, 1.3,
1.4, 1.5`), separately for the triangular-family and square-family
layouts (DWSIM lines ~2408-2478).

# Errors
[`ShellAndTubeError::PitchToDiameterRatioTooLarge`] if
`pitch/tube_outer_diameter > 1.5` (DWSIM: hard `Throw`).

```rust
pub fn shell_friction_factor(layout: TubeLayout, re_shell: uom::si::f64::Ratio, pitch: uom::si::f64::Length, tube_outer_diameter: uom::si::f64::Length) -> Result<uom::si::f64::Ratio, ShellAndTubeError> { /* ... */ }
```

#### Function `shell_side`

Full shell-side coefficient and pressure-drop evaluation, combining the
bundle geometry, `Nh`/`Y`/`Np`, both `j_h` contexts, `f_s`, and the
baffle-leakage correction `E_c` (DWSIM lines ~2298-2537).

`w`, `rho`, `mu`, `k`, `cp` are the shell-side fluid's mass flow rate and
properties (evaluated at the shell-side mean temperature -- the caller's
responsibility, since this rating port is decoupled from the flash kernel).

# Errors
Propagates [`ShellAndTubeError::PitchToDiameterRatioTooLarge`] from
[`shell_friction_factor`].

```rust
pub fn shell_side(geometry: &ShellAndTubeGeometry, w: uom::si::f64::MassRate, rho: uom::si::f64::MassDensity, mu: uom::si::f64::DynamicViscosity, k: uom::si::f64::ThermalConductivity, cp: uom::si::f64::SpecificHeatCapacity) -> Result<ShellSideResult, ShellAndTubeError> { /* ... */ }
```

#### Function `overall_coefficient`

Overall heat-transfer coefficient `U` from the tube-side and shell-side
HTCs, fouling resistances, and wall conduction --
`1/U = d_e/(h_i d_i) + r_t d_e/d_i + d_e/(2 k_t) ln(d_e/d_i) + r_s + 1/h_e`
(DWSIM lines ~2539-2545, Rating-mode form).

```rust
pub fn overall_coefficient(geometry: &ShellAndTubeGeometry, tube_side: &TubeSideResult, shell_side: &ShellSideResult) -> uom::si::f64::HeatTransfer { /* ... */ }
```

#### Function `fouling_factor_from_design_u`

Back-calculate the overall fouling factor `R_f = 1/U_design - 1/U_clean`
-- DWSIM's `ShellandTube_CalcFoulingFactor` mode (lines ~2547-2554).
`u_clean` excludes the fouling terms (`f1 + f3 + f5` only); `u_design` is
the duty-derived `U` (`Q / (A * mean_delta_t)`, the caller's
responsibility to compute).

```rust
pub fn fouling_factor_from_design_u(geometry: &ShellAndTubeGeometry, tube_side: &TubeSideResult, shell_side: &ShellSideResult, u_design: uom::si::f64::HeatTransfer) -> FoulingFactor { /* ... */ }
```

## Module `heater`

Heater: enthalpy-driven heating duty and outlet-state relations.

Ported from DWSIM `UnitOperations/Heater.vb` (GPL-3.0), the
`Public Overrides Sub Calculate` routine (Heater.vb:421-722). A DWSIM
heater performs a stream-heating energy balance: the outlet enthalpy is set
by the duty and the outlet temperature comes from a pressure-enthalpy (PH)
flash, or vice-versa. This equipment port is deliberately kept decoupled
from the crate's flash kernel ([`crate::thermo`]), so every
flash-dependent step is pushed to the caller: the functions below take
already-known specific enthalpies / mass flow / Cp as inputs, and the
rigorous "outlet temperature given" path takes a caller-supplied closure
(a generic `Fn`, not a trait object, per the workspace no-`dyn` rule) that
evaluates the outlet enthalpy at the specified outlet temperature via the
caller's own PH/PT flash.

# Sign convention

A **heater adds duty**: a positive heat duty `Q` raises the specific
enthalpy, so the outlet enthalpy `h2 >= h1` for `Q >= 0`. (The mirror
[`crate::cooler`] removes duty: positive `Q` there *lowers* `h2`.) This
matches DWSIM's own accounting, where `DeltaQ` is reported positive for a
heater and the enthalpy balance is `H2 = +Q*(eta/100)/W + H1`
(Heater.vb:475).

# Efficiency

DWSIM stores efficiency as a percentage in `[0, 100]` (`Eficiencia`,
default 100). This port takes it as a dimensionless [`Ratio`] where `1.0`
means 100% — the fraction of the electrical/utility duty that actually
reaches the process fluid. In the "duty given" direction the fluid sees
`Q * eta`; in the "outlet spec given" direction the required duty is
`w*(h2-h1)/eta`, i.e. *more* duty than the enthalpy rise because part is
lost. Efficiency `0` is degenerate: the "duty given" path then delivers no
heat (`h2 = h1`) and the "spec given" path divides by zero (returns a
non-finite duty) — callers must guard against it.

# Pressure drop

Pressure drop is a simple specified value: `p2 = p1 - dp` (Heater.vb:458),
exposed as [`outlet_pressure`]. It carries no flash coupling and is shared
with [`crate::cooler`], which re-exports it through this module.

# Excluded DWSIM behaviour

The GUI editor (`EditingForm_HeaterCooler`), XML/JSON serialization
(`CloneXML`/`CloneJSON`/`SaveData`), the property-grid reflection
(`GetPropertyValue`/`SetPropertyValue`/`GetProperties`), the report
builders (`GetReport`/`GetStructuredReport`), the flowsheet energy-stream
plumbing (`GetInletEnergyStream`/`EnergyFlow`), and the dynamic-mode
accumulation model (`RunDynamicModel`, Heater.vb:244-419) are deliberately
NOT ported — they are DWSIM application/solver infrastructure, not the
thermodynamic kernel. The `EnergyStream` and `OutletVaporFraction`
calculation modes both require an internal flash with no closed-form
duty/enthalpy relation of their own, so they are left to the caller (the
`EnergyStream` mode is arithmetically identical to `HeatAdded` once the
duty is read from the energy stream — use [`outlet_enthalpy_heat_added`]).

```rust
pub mod heater { /* ... */ }
```

### Functions

#### Function `outlet_pressure`

Outlet pressure after a specified pressure drop: `p2 = p1 - dp`.

Ported from Heater.vb:458 (`P2 = Pi - Me.DeltaP`). This is a plain
subtraction with no flash coupling; [`crate::cooler`] re-uses it (Cooler.vb
applies the identical `P2 = Pi - Me.DeltaP`, Cooler.vb:436). `pressure_drop`
is the specified drop (>= 0 for a real device); no check is applied, so a
negative drop yields `p2 > p1`.

```rust
pub fn outlet_pressure(p1: uom::si::f64::Pressure, pressure_drop: uom::si::f64::Pressure) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `outlet_temperature_from_change`

Outlet temperature for the "temperature change" mode: `T2 = T1 + dT`.

Ported from Heater.vb:561 (`T2 = Ti + Me.DeltaT`). `delta_t` is a signed
temperature *interval* (positive to heat, negative to cool). Shared with
[`crate::cooler`], whose temperature-change mode uses the identical
`T2 = Ti + Me.DeltaT` (Cooler.vb:587).

```rust
pub fn outlet_temperature_from_change(t1: uom::si::f64::ThermodynamicTemperature, delta_t: uom::si::f64::TemperatureInterval) -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

#### Function `outlet_enthalpy_heat_added`

Outlet specific enthalpy for the **heat-added** mode (duty given):
`h2 = h1 + Q * eta / w`.

Ported from Heater.vb:475
(`H2 = Me.DeltaQ * (Me.Eficiencia / 100) / Wi + Hi`). This is the
`HeatAdded` / `HeatAddedRemoved` mode, and is also the closed-form part of
the `EnergyStream` mode (Heater.vb:608), which is identical once the duty
is taken from the energy stream.

Sign: a heater *adds* duty, so `h2 >= h1` for `q >= 0`.

- `h1` — inlet specific enthalpy \[J/kg\].
- `q` — heat duty \[W\]; positive adds heat.
- `w` — mass flow rate \[kg/s\]; must be `> 0`.
- `efficiency` — fraction of duty reaching the fluid, `1.0` = 100%.

```rust
pub fn outlet_enthalpy_heat_added(h1: uom::si::f64::AvailableEnergy, q: uom::si::f64::Power, w: uom::si::f64::MassRate, efficiency: uom::si::f64::Ratio) -> uom::si::f64::AvailableEnergy { /* ... */ }
```

#### Function `duty_from_outlet_enthalpy`

Heat duty from a known outlet specific enthalpy (rigorous "outlet spec"
direction): `Q = w * (h2 - h1) / eta`.

Ported from Heater.vb:536
(`Me.DeltaQ = (H2 - Hi) / (Me.Eficiencia / 100) * Wi`). The outlet enthalpy
`h2` comes from the caller's flash at the specified outlet temperature (or
vapour fraction); the same relation serves both the `OutletTemperature` and
`TemperatureChange` modes (Heater.vb:536 and 580).

Division by `efficiency` inflates the duty above the raw enthalpy rise
`w*(h2-h1)` to account for losses; `efficiency = 0` gives a non-finite duty.

```rust
pub fn duty_from_outlet_enthalpy(h1: uom::si::f64::AvailableEnergy, h2: uom::si::f64::AvailableEnergy, w: uom::si::f64::MassRate, efficiency: uom::si::f64::Ratio) -> uom::si::f64::Power { /* ... */ }
```

#### Function `duty_constant_cp`

Heat duty for the "outlet temperature given" mode under a **constant-`Cp`
approximation**: `Q = w * Cp * (T2 - T1) / eta`.

This is the closed-form stand-in for the rigorous PT-flash path
(Heater.vb:532-536). DWSIM itself always does a rigorous flash — there is
no constant-`Cp` branch in `Heater.vb` — so this is a *port-added*
approximation valid only where `Cp` is sensibly constant over `[T1, T2]`
(no phase change, mild `T` range). For the rigorous enthalpy path use
[`duty_from_outlet_temperature`] with a flash closure.

- `w` — mass flow rate \[kg/s\].
- `cp` — mean specific heat capacity \[J/(kg·K)\] over the interval.
- `t1`, `t2` — inlet / outlet temperatures \[K\].
- `efficiency` — utility-to-fluid fraction, `1.0` = 100%.

```rust
pub fn duty_constant_cp(w: uom::si::f64::MassRate, cp: uom::si::f64::SpecificHeatCapacity, t1: uom::si::f64::ThermodynamicTemperature, t2: uom::si::f64::ThermodynamicTemperature, efficiency: uom::si::f64::Ratio) -> uom::si::f64::Power { /* ... */ }
```

#### Function `duty_from_outlet_temperature`

Heat duty for the "outlet temperature given" mode, **rigorous** path: the
caller supplies the outlet enthalpy at `t2` via a flash closure.

This mirrors Heater.vb:532-536, where DWSIM does a PT flash at `(P2, T2)`
to get `H2` and then `Me.DeltaQ = (H2 - Hi) / (eta/100) * Wi`. Since this
crate cannot flash, `outlet_enthalpy_at` is the caller's PT-flash step:
given the specified outlet temperature it returns the outlet specific
enthalpy `h2`. It is a generic `Fn` (no `dyn`), so it monomorphises with
zero dispatch overhead.

The returned duty equals [`duty_from_outlet_enthalpy`] evaluated at the
flashed `h2`.

```rust
pub fn duty_from_outlet_temperature<F>(h1: uom::si::f64::AvailableEnergy, w: uom::si::f64::MassRate, efficiency: uom::si::f64::Ratio, t2: uom::si::f64::ThermodynamicTemperature, outlet_enthalpy_at: F) -> uom::si::f64::Power
where
    F: Fn(uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::AvailableEnergy { /* ... */ }
```

## Module `interpolation`

General-purpose numerical interpolation methods used by the equipment
models in this crate -- not DWSIM-specific code, just the supporting
numerics one of DWSIM's calculation modes needs (see [`floater_hormann`]'s
module doc for exactly which one, and its literature citations).

```rust
pub mod interpolation { /* ... */ }
```

### Modules

## Module `floater_hormann`

Floater-Hormann barycentric rational interpolation.

Ported (as a general numerical method, not DWSIM-specific code) to
replace DWSIM `UnitOperations/Pump.vb`/`Expander.vb`'s `Curves`
calculation mode, which interpolates manufacturer performance data
(head/efficiency/NPSHr/power vs. flow) using this same method via
`ratinterpolation.buildfloaterhormannrationalinterpolant`/
`polinterpolation.barycentricinterpolation` (DWSIM vendors ALGLIB for
this; ALGLIB's own source was not consulted -- see the citations below
instead).

# Primary reference

Floater, M. S., & Hormann, K. (2007). Barycentric rational interpolation
with no poles and high rates of approximation. *Numerische Mathematik*,
107(2), 315-331. <https://doi.org/10.1007/s00211-007-0093-y>

# Formula cross-checked against

The exact indexing convention implemented here (weight sign, summation
range) was cross-checked against SciPy's documentation for
`scipy.interpolate.FloaterHormannInterpolator` (SciPy is BSD-3-Clause
licensed; NumPy, which SciPy depends on, is also BSD-3-Clause -- no
SciPy/NumPy source code is copied here, only the openly-documented
mathematical formula, reproduced independently in Rust):
<https://docs.scipy.org/doc/scipy/reference/generated/scipy.interpolate.FloaterHormannInterpolator.html>

```text
w_k = (-1)^(k-d) * sum_{i in J_k} prod_{j=i, j != k}^{i+d} 1/|x_k - x_j|
J_k = { i in I : k-d <= i <= k },  I = {0, 1, ..., n-d}

r(x) = ( sum_k w_k y_k / (x - x_k) ) / ( sum_k w_k / (x - x_k) )
```
where `n` is the number of data points minus one (points indexed
`0..=n`) and `d` (`0 <= d < n`) is the blended-polynomial degree
(higher `d` gives smoother, higher-order interpolation; `d=3`-`4` is a
common default, matching ALGLIB's/DWSIM's typical usage).

```rust
pub mod floater_hormann { /* ... */ }
```

### Types

#### Enum `FloaterHormannError`

Errors constructing or evaluating a [`FloaterHormannInterpolant`].

```rust
pub enum FloaterHormannError {
    TooFewPoints {
        count: usize,
    },
    DegreeOutOfRange {
        degree: usize,
        n: usize,
    },
}
```

##### Variants

###### `TooFewPoints`

Fewer than 2 data points were supplied.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `count` | `usize` | Number of points actually supplied. |

###### `DegreeOutOfRange`

`degree` did not satisfy `0 <= degree < n` (`n = points.len() - 1`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `degree` | `usize` | The invalid degree that was supplied. |
| `n` | `usize` | The number of points minus one. |

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
    fn clone(self: &Self) -> FloaterHormannError { /* ... */ }
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Eq**
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FloaterHormannError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FloaterHormannInterpolant`

A Floater-Hormann rational interpolant through a fixed set of `(x, y)`
data points -- no real poles, reproduces polynomials of degree `d`
exactly, well-conditioned for tabulated manufacturer performance data
(pump/turbine head, efficiency, NPSHr, power vs. flow).

```rust
pub struct FloaterHormannInterpolant {
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
  pub fn new(points: &[(f64, f64)], degree: usize) -> Result<Self, FloaterHormannError> { /* ... */ }
  ```
  Build the interpolant from data points `(x_i, y_i)`, blending

- ```rust
  pub fn evaluate(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Evaluate the interpolant at `x`. Returns the exact data value if `x`

- ```rust
  pub fn evaluate_ratio(self: &Self, x: Ratio) -> Ratio { /* ... */ }
  ```
  [`Self::evaluate`], with `uom`-typed dimensionless input/output --

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FloaterHormannInterpolant { /* ... */ }
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
    fn eq(self: &Self, other: &FloaterHormannInterpolant) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `mixer`

Mixer: adiabatic multi-stream mass and energy balance.

Pure-Rust port of DWSIM's stream mixer, from
`UnitOperations/Mixer.vb` (GPL-3.0, `Public Overrides Sub Calculate`,
lines 85-241). Upstream copyright: 2008 Daniel Wagner O. de Medeiros.

A mixer combines up to N inlet material streams into one outlet, closing the
steady-state **mass** and **adiabatic energy** balances:

- **Total mass flow** — `w = Σ w_i` (Mixer.vb:143-144, `W += We`).
- **Mixed specific enthalpy** — `h = Σ(w_i·h_i) / Σ w_i`, the adiabatic
  (zero heat loss, zero shaft work) energy balance (Mixer.vb:145 `H += We*enthalpy`,
  :165 `Hs = H / W`).
- **Outlet pressure** — one of three user-selected modes ported as the
  [`PressureBehavior`] enum: Minimum, Maximum, or Average of the inlet
  pressures (Mixer.vb:40-44 `Enum PressureBehavior`, :124-139, :167).
- **Composition** — mass-flow-weighted mass fractions, with an optional
  mass-to-mole-fraction conversion, ported as free functions
  ([`mixed_mass_fraction`], [`mass_to_mole_fractions`]; Mixer.vb:184-232).

# Flash boundary (pushed to the caller)

DWSIM finishes by writing `(P, Hs, W)` onto the outlet stream and letting the
flowsheet solver run a **pressure-enthalpy (PH) flash** to recover the outlet
temperature and phase split (Mixer.vb:98-100, :234
`StreamSpec.Pressure_and_Enthalpy`). This mixer port is deliberately kept
decoupled from the crate's flash kernel ([`crate::thermo`]), so — exactly as
in [`crate::expander`] — the flash is **not** performed here. [`mix`] returns the
mixed [`MixerOutlet`] `(pressure, specific_enthalpy, mass_flow)`, and the
caller runs the PH flash on `(p_out, h)` to obtain outlet `T` and phase.

# Excluded DWSIM behavior

Deliberately **not** ported (GUI / solver / persistence plumbing, no physics):
the `EditingForm_Mixer` editor and icon/bitmap resources (Mixer.vb:38, :302-338),
XML/JSON serialization (`CloneXML`/`CloneJSON`/`SaveData`, :69-77), the
flowsheet `Inspector` trace paragraphs (:87-101, :120-142, :201-205), the
dynamic-mode backward pressure propagation (:169-179), `DeCalculate` outlet
clearing (:243-261), and the property-grid reflection accessors
(`GetPropertyValue`/`GetProperties`/`GetPropertyUnit`, :263-300). DWSIM's
six-inlet GUI cap is dropped — this port accepts any number of inlets.

```rust
pub mod mixer { /* ... */ }
```

### Types

#### Enum `PressureBehavior`

Outlet-pressure calculation mode — DWSIM's `Mixer.PressureBehavior` enum
(Mixer.vb:40-44). Selects how the single outlet pressure is derived from the
inlet pressures. Modeled as an enum (no `dyn` dispatch), per the workspace
design rules.

```rust
pub enum PressureBehavior {
    Minimum,
    Maximum,
    Average,
}
```

##### Variants

###### `Minimum`

Outlet pressure = minimum of the inlet pressures (Mixer.vb:124-129).
This is DWSIM's default (`m_pressurebehavior = PressureBehavior.Minimum`,
Mixer.vb:46) and the physically conservative choice: a passive tee cannot
raise a stream above the lowest feed pressure without a pump/compressor.

###### `Maximum`

Outlet pressure = maximum of the inlet pressures (Mixer.vb:130-135).

###### `Average`

Outlet pressure = arithmetic mean of the inlet pressures
(Mixer.vb:136-139 accumulate, :167 `P = P / (i - 1)`).

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
    fn clone(self: &Self) -> PressureBehavior { /* ... */ }
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
    fn eq(self: &Self, other: &PressureBehavior) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `InletStream`

One inlet material stream to the mixer, owned by value (no references, no
lifetimes). Only the quantities the mass/energy/pressure balance needs are
carried; composition is handled separately (see module docs).

Units (all SI, `uom`-typed):
- `mass_flow` — mass flow rate `w_i`, kg/s. Must be finite and `>= 0`.
- `specific_enthalpy` — specific enthalpy `h_i`, J/kg (mass basis, matching
  DWSIM's `Phases(0).Properties.enthalpy`). Any real value; the datum only
  needs to be consistent across all inlets and the caller's outlet flash.
- `pressure` — stream pressure `p_i`, Pa. Must be finite and `> 0`.

```rust
pub struct InletStream {
    pub mass_flow: uom::si::f64::MassRate,
    pub specific_enthalpy: uom::si::f64::AvailableEnergy,
    pub pressure: uom::si::f64::Pressure,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mass_flow` | `uom::si::f64::MassRate` | Mass flow rate `w_i` \[kg/s\], `>= 0`. |
| `specific_enthalpy` | `uom::si::f64::AvailableEnergy` | Specific enthalpy `h_i` \[J/kg\], mass basis. |
| `pressure` | `uom::si::f64::Pressure` | Stream pressure `p_i` \[Pa\], `> 0`. |

##### Implementations

###### Methods

- ```rust
  pub fn from_si(mass_flow: f64, specific_enthalpy: f64, pressure: f64) -> Self { /* ... */ }
  ```
  Convenience constructor from SI scalars: `mass_flow` \[kg/s\],

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> InletStream { /* ... */ }
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
    fn eq(self: &Self, other: &InletStream) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MixerOutlet`

The mixed outlet state produced by [`mix`]. These are exactly the three
quantities DWSIM writes onto the outlet stream before the PH flash
(Mixer.vb:211-213). Outlet temperature and phase are **not** here — the
caller obtains them by flashing `(pressure, specific_enthalpy)` (see the
module-level "Flash boundary" note).

```rust
pub struct MixerOutlet {
    pub pressure: uom::si::f64::Pressure,
    pub specific_enthalpy: uom::si::f64::AvailableEnergy,
    pub mass_flow: uom::si::f64::MassRate,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `uom::si::f64::Pressure` | Outlet pressure `p_out` \[Pa\], per the chosen [`PressureBehavior`]. |
| `specific_enthalpy` | `uom::si::f64::AvailableEnergy` | Mixed specific enthalpy `h = Σ(w_i·h_i)/Σ w_i` \[J/kg\], mass basis. |
| `mass_flow` | `uom::si::f64::MassRate` | Total mass flow `w = Σ w_i` \[kg/s\]. |

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
    fn clone(self: &Self) -> MixerOutlet { /* ... */ }
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
    fn eq(self: &Self, other: &MixerOutlet) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `MixerError`

Errors from the mixer balance. Ported behavior differs from DWSIM here: DWSIM
silently substitutes `Hs = 0` / `T = 273.15 K` when the total mass flow is
zero (Mixer.vb:165, :199); this port instead reports the condition, because a
specific enthalpy is genuinely undefined for zero flow (`0/0`), and hiding
that would violate the workspace honesty rules.

```rust
pub enum MixerError {
    NoInlets,
    ZeroTotalMassFlow,
}
```

##### Variants

###### `NoInlets`

No inlet streams were supplied (empty slice). DWSIM guards the analogous
"no attached streams" case by throwing (Mixer.vb:102-104, :121).

###### `ZeroTotalMassFlow`

The total mass flow is zero, so the mixed specific enthalpy
`Σ(w_i·h_i)/Σ w_i` is `0/0` and undefined.

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
    fn clone(self: &Self) -> MixerError { /* ... */ }
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
    fn fmt(self: &Self, f: &mut core::fmt::Formatter<''_>) -> core::fmt::Result { /* ... */ }
    ```

- **Eq**
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MixerError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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

#### Function `total_mass_flow`

Total outlet mass flow `w = Σ w_i` (Mixer.vb:143-144).

Returns `0 kg/s` for an empty slice (an empty sum), so this is
error-free by construction. Units: kg/s in, kg/s out.

```rust
pub fn total_mass_flow(inlets: &[InletStream]) -> uom::si::f64::MassRate { /* ... */ }
```

#### Function `mixed_specific_enthalpy`

Mixed (flow-weighted) specific enthalpy `h = Σ(w_i·h_i) / Σ w_i` — the
adiabatic energy balance (Mixer.vb:145 `H += We*enthalpy`, :165 `Hs = H/W`).

This is the mass-flow-weighted mean of the inlet specific enthalpies; it
assumes no heat exchange with the surroundings and no shaft work (adiabatic
tee). Units: J/kg out.

# Errors
- [`MixerError::NoInlets`] if `inlets` is empty.
- [`MixerError::ZeroTotalMassFlow`] if `Σ w_i == 0` (enthalpy is `0/0`).

```rust
pub fn mixed_specific_enthalpy(inlets: &[InletStream]) -> Result<uom::si::f64::AvailableEnergy, MixerError> { /* ... */ }
```

#### Function `outlet_pressure`

Outlet pressure per the selected [`PressureBehavior`] (Mixer.vb:124-139, :167).

This port takes the plain min / max / arithmetic-mean over the actual inlet
pressures. DWSIM's imperative code uses a `P = 0` "unset" sentinel while
scanning (Mixer.vb:125-135), which is an initialization artifact, not a
physical rule; the set-based min/max/mean here is equivalent for physical
(`p_i > 0`) inlets and is what DWSIM intends. Units: Pa out.

# Errors
[`MixerError::NoInlets`] if `inlets` is empty (min/max/mean undefined).

```rust
pub fn outlet_pressure(inlets: &[InletStream], mode: PressureBehavior) -> Result<uom::si::f64::Pressure, MixerError> { /* ... */ }
```

#### Function `mix`

Full adiabatic mixer balance: combine `inlets` into one [`MixerOutlet`]
`(pressure, specific_enthalpy, mass_flow)` using pressure mode `mode`
(Mixer.vb `Calculate`, :85-241).

A single inlet is handled as a passthrough (its mass flow, enthalpy, and
pressure carry straight through — min/max/mean of one value is that value),
mirroring DWSIM's single-source shortcut (Mixer.vb:149-162, `isSS`).

The returned state is the **input to a caller-side PH flash** — see the
module-level "Flash boundary" note — which recovers outlet temperature and
phase from `(pressure, specific_enthalpy)`.

# Errors
- [`MixerError::NoInlets`] if `inlets` is empty.
- [`MixerError::ZeroTotalMassFlow`] if `Σ w_i == 0`.

```rust
pub fn mix(inlets: &[InletStream], mode: PressureBehavior) -> Result<MixerOutlet, MixerError> { /* ... */ }
```

#### Function `mixed_mass_fraction`

Mixed mass fraction of **one compound** across the inlets:
`x_out = Σ(w_i · x_i) / Σ w_i` (DWSIM `Vw` accumulation, Mixer.vb:193, and
normalization at :219 `comp.MassFraction = Vw(comp.Name) / W`).

`inlet_flows[i]` is inlet `i`'s total mass flow `w_i`; `inlet_mass_fractions[i]`
is that compound's mass fraction `x_i` in inlet `i` (dimensionless \[0, 1\]).
The two slices must be the same length. This is composition mixing on a mass
basis; call it once per compound to build the full outlet composition vector.

# Errors
- [`MixerError::NoInlets`] if the slices are empty.
- [`MixerError::ZeroTotalMassFlow`] if `Σ w_i == 0`.

# Panics
Panics if `inlet_flows.len() != inlet_mass_fractions.len()`.

```rust
pub fn mixed_mass_fraction(inlet_flows: &[uom::si::f64::MassRate], inlet_mass_fractions: &[uom::si::f64::Ratio]) -> Result<uom::si::f64::Ratio, MixerError> { /* ... */ }
```

#### Function `mass_to_mole_fractions`

Convert a mixed mass-fraction vector to mole fractions given each compound's
molar mass (DWSIM Mixer.vb:221-232):
`y_i = (x_i / M_i) / Σ_j (x_j / M_j)`.

`mass_fractions[i]` and `molar_weights[i]` describe the same compound `i`.
The `M_i` unit cancels in the ratio, so any consistent molar-mass unit is
fine (`uom` `MolarMass`, base kg/mol). Returns a mole-fraction vector that
sums to 1 (dimensionless \[0, 1\]).

# Panics
Panics if the two slices differ in length. Returns an empty vector for empty
input. If `Σ_j (x_j / M_j) == 0` (e.g. all-zero mass fractions), the result
entries are `NaN` — the caller should mix nonzero compositions.

```rust
pub fn mass_to_mole_fractions(mass_fractions: &[uom::si::f64::Ratio], molar_weights: &[uom::si::f64::MolarMass]) -> Vec<uom::si::f64::Ratio> { /* ... */ }
```

## Module `pipe`

Pipe pressure-drop correlations (single-phase and two-phase gas-liquid flow).

Ported from DWSIM `UnitOperations/Pipe.vb` and
`FluidFlowCorrelations/{FlowPackageBaseClass,BeggsBrill,LockhartMartinelli}.vb`
(see this crate's `docs/port-scope.md`). DWSIM's third correlation,
Petalas-Aziz, only wraps an unshipped native DLL upstream and has no
portable source -- see [`petalas_aziz`]'s module doc for the literature
review and why it is a documented stub, not a port.

```rust
pub mod pipe { /* ... */ }
```

### Modules

## Module `beggs_brill`

Beggs & Brill (1973) two-phase gas-liquid pipe flow correlation.

Ported from DWSIM `FluidFlowCorrelations/BeggsBrill.vb`. DWSIM's own
source documents the full derivation inline (HTML/MathJax); the equations
below follow that documentation and the standard Beggs & Brill (1973)
reference ("A Study of Two-Phase Flow in Inclined Pipes", JPT).

The empirical regime-boundary and holdup correlations below are
dimensionless curve fits (not unit-checked physics beyond the base
quantities), so -- matching DWSIM's own approach -- intermediate work is
done in plain `f64` (SI) after unwrapping the `uom` inputs; only the
function boundary and the final results are dimensioned.

```rust
pub mod beggs_brill { /* ... */ }
```

### Types

#### Enum `FlowRegime`

Flow regime identified by the Beggs & Brill regime map.

```rust
pub enum FlowRegime {
    Segregated,
    Intermittent,
    Distributed,
    Transition,
}
```

##### Variants

###### `Segregated`

Stratified/annular-like, low liquid holdup change with inclination.

###### `Intermittent`

Slug flow.

###### `Distributed`

Bubble / dispersed-bubble flow.

###### `Transition`

Blended between [`Self::Segregated`] and [`Self::Intermittent`].

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
    fn clone(self: &Self) -> FlowRegime { /* ... */ }
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
    fn eq(self: &Self, other: &FlowRegime) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BeggsBrillResult`

Result of a Beggs & Brill two-phase pressure-drop evaluation.

```rust
pub struct BeggsBrillResult {
    pub regime: FlowRegime,
    pub liquid_holdup: uom::si::f64::Ratio,
    pub mixture_density: uom::si::f64::MassDensity,
    pub friction_pressure_drop: uom::si::f64::Pressure,
    pub elevation_pressure_drop: uom::si::f64::Pressure,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `regime` | `FlowRegime` | Identified flow regime. |
| `liquid_holdup` | `uom::si::f64::Ratio` | Liquid holdup at the pipe's actual inclination `H_L(θ)` \[-\]. |
| `mixture_density` | `uom::si::f64::MassDensity` | Two-phase mixture density using the inclined holdup, `ρ_m`. |
| `friction_pressure_drop` | `uom::si::f64::Pressure` | Frictional pressure drop. |
| `elevation_pressure_drop` | `uom::si::f64::Pressure` | Elevation (hydrostatic) pressure drop. |

##### Implementations

###### Methods

- ```rust
  pub fn total_pressure_drop(self: &Self) -> Pressure { /* ... */ }
  ```
  Total pressure drop, friction plus elevation.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BeggsBrillResult { /* ... */ }
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
    fn eq(self: &Self, other: &BeggsBrillResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `beggs_brill_pressure_drop`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Evaluate the Beggs & Brill (1973) two-phase pressure drop over a pipe
segment of length `length`, diameter `diameter`, absolute roughness
`roughness`, and net elevation rise `elevation_change` (positive =
uphill, must satisfy `|elevation_change| <= length`), given liquid and
gas volumetric flow rates, densities, a single no-slip-mixture dynamic
viscosity (matching DWSIM's treatment), and liquid surface tension.

```rust
pub fn beggs_brill_pressure_drop(length: uom::si::f64::Length, diameter: uom::si::f64::Length, roughness: uom::si::f64::Length, elevation_change: uom::si::f64::Length, q_liquid: uom::si::f64::VolumeRate, q_gas: uom::si::f64::VolumeRate, density_liquid: uom::si::f64::MassDensity, density_gas: uom::si::f64::MassDensity, viscosity_no_slip: uom::si::f64::DynamicViscosity, surface_tension_liquid: uom::si::f64::SurfaceTension) -> BeggsBrillResult { /* ... */ }
```

## Module `friction_factor`

Single-phase Darcy friction factor and pressure drop.

Ported from DWSIM `FluidFlowCorrelations/FlowPackageBaseClass.vb`
(`FrictionFactor`, `CalculateDeltaPLiquid`/`CalculateDeltaPGas`). DWSIM
carries two near-identical explicit Colebrook fits (the other is inlined
in `Pipe.vb`'s `CalcOverallHeatTransferCoefficient`); this is the single
consolidated Rust version.

```rust
pub mod friction_factor { /* ... */ }
```

### Functions

#### Function `reynolds_number`

Reynolds number `Re = ρ v D / μ` \[-\] for flow in a pipe of diameter `D`.

```rust
pub fn reynolds_number(density: uom::si::f64::MassDensity, velocity: uom::si::f64::Velocity, diameter: uom::si::f64::Length, viscosity: uom::si::f64::DynamicViscosity) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `darcy_friction_factor`

Darcy friction factor `f` \[-\] for flow in a pipe of diameter `D` and
absolute roughness `k`, from an explicit approximation to the Colebrook
equation (turbulent branch) with laminar (`Re < 2100`) and transitional
(`2100 <= Re <= 4000`) closures either side.

Ported from DWSIM `FlowPackageBaseClass.FrictionFactor`:
```text
Turbulent (Re>4000):  a1 = log10( (k/D)^1.1096/2.8257 + (5.8506/Re)^0.8961 )
                      b1 = -2*log10( (k/D)/3.7065 - 5.0452*a1/Re )
                      f  = (1/b1)^2
Laminar (Re<2100):    f = 64/Re
Transitional:          f = 8*[ (8/Re)^12 + 1/(b+c)^1.5 ]^(1/12)
                       b = (2.457*ln(1/((7/Re)^0.9+0.27*k/D)))^16
                       c = (37530/Re)^16
```

```rust
pub fn darcy_friction_factor(reynolds: uom::si::f64::Ratio, diameter: uom::si::f64::Length, roughness: uom::si::f64::Length) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `frictional_pressure_drop`

Frictional pressure drop `ΔP_f = f (L/D) (ρ v^2 / 2)` for single-phase
flow through a straight pipe run of length `L`.

```rust
pub fn frictional_pressure_drop(friction_factor: uom::si::f64::Ratio, length: uom::si::f64::Length, diameter: uom::si::f64::Length, density: uom::si::f64::MassDensity, velocity: uom::si::f64::Velocity) -> uom::si::f64::Pressure { /* ... */ }
```

## Module `lockhart_martinelli`

Lockhart-Martinelli (1949) two-phase gas-liquid pipe flow correlation.

Ported from DWSIM `FluidFlowCorrelations/LockhartMartinelli.vb`. DWSIM's
implementation is a separated-flow / two-phase-multiplier model (each
phase's single-phase pressure drop, corrected by a multiplier derived
from the Martinelli parameter `X`), not the classic void-fraction chart.

```rust
pub mod lockhart_martinelli { /* ... */ }
```

### Types

#### Struct `LockhartMartinelliResult`

Result of a Lockhart-Martinelli two-phase pressure-drop evaluation.

```rust
pub struct LockhartMartinelliResult {
    pub martinelli_parameter: uom::si::f64::Ratio,
    pub liquid_holdup: uom::si::f64::Ratio,
    pub friction_pressure_drop: uom::si::f64::Pressure,
    pub elevation_pressure_drop: uom::si::f64::Pressure,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `martinelli_parameter` | `uom::si::f64::Ratio` | Martinelli parameter `X = sqrt(dP_SL / dP_SG)` \[-\]. |
| `liquid_holdup` | `uom::si::f64::Ratio` | Liquid holdup estimate `1 / sqrt(1 + 20/X + 1/X^2)` \[-\]. |
| `friction_pressure_drop` | `uom::si::f64::Pressure` | Frictional pressure drop: `max(phi_L^2 dP_SL, phi_G^2 dP_SG)`. |
| `elevation_pressure_drop` | `uom::si::f64::Pressure` | Elevation (hydrostatic) pressure drop. |

##### Implementations

###### Methods

- ```rust
  pub fn total_pressure_drop(self: &Self) -> Pressure { /* ... */ }
  ```
  Total pressure drop, friction plus elevation.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LockhartMartinelliResult { /* ... */ }
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
    fn eq(self: &Self, other: &LockhartMartinelliResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `lockhart_martinelli_pressure_drop`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Evaluate the Lockhart-Martinelli (1949) two-phase pressure drop over a
pipe segment, given liquid and gas volumetric flow rates, densities,
viscosities, and the pipe's inclination angle from horizontal (positive =
uphill).

```rust
pub fn lockhart_martinelli_pressure_drop(length: uom::si::f64::Length, diameter: uom::si::f64::Length, roughness: uom::si::f64::Length, inclination: uom::si::f64::Angle, q_liquid: uom::si::f64::VolumeRate, q_gas: uom::si::f64::VolumeRate, density_liquid: uom::si::f64::MassDensity, density_gas: uom::si::f64::MassDensity, viscosity_liquid: uom::si::f64::DynamicViscosity, viscosity_gas: uom::si::f64::DynamicViscosity) -> LockhartMartinelliResult { /* ... */ }
```

## Module `petalas_aziz`

Petalas & Aziz (2000) mechanistic multiphase-flow model.

## Status: not implemented

DWSIM's own `PetalasAziz.vb` is not portable -- it is a `DllImport`
wrapper around an external native `PetAz.dll` that is not present
anywhere in the DWSIM source tree (see this crate's `docs/port-scope.md`
and the workspace's `op-qo2.6` bead, P4/low priority). Implementing this
model here would mean an independent derivation from the primary
literature, not a translation of existing code.

## Literature review (2026-07-13)

Primary reference, correctly cited (DWSIM's own in-code citation as
"SPE 71124" appears to be an error -- no such paper was found under that
number for this model):

> Petalas, N., & Aziz, K. (2000). A mechanistic model for multiphase flow
> in pipes. *Journal of Canadian Petroleum Technology*, 39(6), 43-55.
> <https://doi.org/10.2118/00-06-04>

Also indexed as a PETSOC Annual Technical Meeting paper (PETSOC-98-39,
1998) that appears to be an earlier/companion presentation of the same
model. The full closed-form correlations (flow-pattern transition
criteria; stratified-flow liquid/wall and liquid/gas interfacial friction
factors; annular-mist entrained-liquid-fraction and interfacial friction;
the distribution coefficient for intermittent-flow holdup) are described
in secondary sources (review articles, theses) at a summary level only --
e.g. Petalas's own Stanford PhD dissertation is the fullest public
derivation referenced across the review literature found, but was not
independently retrieved or verified as part of this review. No source
consulted here reproduces the complete equation set with enough fidelity
to port responsibly (this crate's other correlations were ported only
once the exact source formulas were confirmed -- see `pipe::beggs_brill`,
`pipe::lockhart_martinelli`).

At a high level (not sufficient to implement from), the model:
- determines the flow pattern (stratified, annular-mist, intermittent,
  bubble/dispersed) from mechanistic stability/transition criteria rather
  than an empirical map like Beggs & Brill's;
- is applicable across all pipe inclinations and geometries (unlike
  correlations developed for a narrower range);
- proposes its own interfacial-friction and holdup-distribution
  correlations per flow pattern, rather than reusing older ones.

## What a real implementation would need

Direct access to Petalas & Aziz (2000) itself (or the underlying Stanford
PhD dissertation) to extract and verify the exact correlations, then the
same treatment given to this crate's other correlations: `uom`-typed
functions, doc comments citing the exact source equation, and unit tests.
Not attempted here -- flagged as P4/deferred per the user's direction
(2026-07-13): this is a low-priority literature-review placeholder, not
an implementation.

```rust
pub mod petalas_aziz { /* ... */ }
```

### Functions

#### Function `petalas_aziz_pressure_drop`

Not implemented -- see the module documentation above for the literature
review and why this was not ported. Calling this always panics; it exists
so the gap is a discoverable, documented item in the crate's API surface
rather than a silent absence.

# Panics
Always. This is an explicit "not implemented" marker, not a usable API.

```rust
pub fn petalas_aziz_pressure_drop() -> never { /* ... */ }
```

## Module `transient`

Transient (dynamic) pipe network: a chain of accumulation-volume cells
with inter-cell mass flow solved by Brent root-finding.

Ported from DWSIM `UnitOperations/Pipe.vb`'s `RunDynamicModel()`, which
represents a pipe as a chain of "accumulation stream" cells (one per
length increment) and, each substep, solves for the inter-cell mass flow
that reconciles the pressure difference between adjacent cells via a
Brent root-find on `Pdrop_transition - dpt(mass_flow) = 0` (falling back
to a least-squares solve if Brent doesn't converge -- not replicated
here, see below). After the mass transfer, DWSIM updates each cell's
pressure via a volume-temperature flash -- this transient pipe port is
deliberately **not** coupled to the crate's flash kernel (the `(V, T) -> P`
step is left to the caller), so [`PipeCell`] only does the mass-balance
bookkeeping; a caller (e.g. `tampines`, or the crate's own
[`crate::thermo`] flash) does the `(V, T) -> P` update itself after calling
[`solve_intercell_mass_flow`].

Root-finding uses the [`roots`] crate's Brent implementation
(BSD-2-Clause licensed; already an OUTRAM PARK workspace dependency) --
reused rather than reimplemented, since Brent's method itself is
standard numerical-methods machinery, not DWSIM-specific.

```rust
pub mod transient { /* ... */ }
```

### Types

#### Struct `PipeCell`

One control volume in a transient pipe network -- fixed geometric
volume, tracked mass. Pressure/temperature/density are the caller's
responsibility (via a `(V, T) -> P` flash after each
[`solve_intercell_mass_flow`] call), since this crate has no
property-package access of its own.

```rust
pub struct PipeCell {
    pub volume: uom::si::f64::Volume,
    pub mass: uom::si::f64::Mass,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `volume` | `uom::si::f64::Volume` | Fixed geometric volume of this cell. |
| `mass` | `uom::si::f64::Mass` | Current mass held in this cell. |

##### Implementations

###### Methods

- ```rust
  pub fn new(volume: Volume, initial_mass: Mass) -> Self { /* ... */ }
  ```
  A new cell with the given volume and initial mass.

- ```rust
  pub fn density(self: &Self) -> uom::si::f64::MassDensity { /* ... */ }
  ```
  Mass density implied by this cell's current mass and (fixed) volume.

- ```rust
  pub fn advance_mass_balance(self: &mut Self, inflow: MassRate, outflow: MassRate, dt: Time) { /* ... */ }
  ```
  Advance this cell's mass balance over `dt`, given the mass flow rate

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PipeCell { /* ... */ }
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
    fn eq(self: &Self, other: &PipeCell) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `solve_intercell_mass_flow`

Solve for the mass flow rate `w` between two adjacent cells such that a
flow correlation's predicted pressure drop `pressure_drop(w)` equals the
actual pressure difference `p_upstream - p_downstream` between them --
DWSIM's `Pdrop_transition - dpt(mass_flow) = 0`.

`pressure_drop` should be monotonically increasing in `w` over the
search bracket `w_bracket = (w_min, w_max)` (as for any standard
friction correlation -- e.g. [`super::friction_factor::frictional_pressure_drop`]
composed with a mass-flow-to-velocity conversion) for Brent's method to
bracket a root correctly; `w_bracket` can include negative values to
allow for reverse flow.

```rust
pub fn solve_intercell_mass_flow<F>(p_upstream: uom::si::f64::Pressure, p_downstream: uom::si::f64::Pressure, pressure_drop: F, w_bracket: (uom::si::f64::MassRate, uom::si::f64::MassRate), tolerance: uom::si::f64::Pressure, max_iterations: usize) -> Result<uom::si::f64::MassRate, roots::SearchError>
where
    F: FnMut(uom::si::f64::MassRate) -> uom::si::f64::Pressure { /* ... */ }
```

### Types

#### Struct `PipeFlowInputs`

Shared inputs for a two-phase pipe pressure-drop evaluation, common to
every [`PipeFlowCorrelation`] variant.

```rust
pub struct PipeFlowInputs {
    pub length: uom::si::f64::Length,
    pub diameter: uom::si::f64::Length,
    pub roughness: uom::si::f64::Length,
    pub inclination: uom::si::f64::Angle,
    pub q_liquid: uom::si::f64::VolumeRate,
    pub q_gas: uom::si::f64::VolumeRate,
    pub density_liquid: uom::si::f64::MassDensity,
    pub density_gas: uom::si::f64::MassDensity,
    pub viscosity_liquid: uom::si::f64::DynamicViscosity,
    pub viscosity_gas: uom::si::f64::DynamicViscosity,
    pub surface_tension_liquid: uom::si::f64::SurfaceTension,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `length` | `uom::si::f64::Length` | Pipe segment length. |
| `diameter` | `uom::si::f64::Length` | Pipe internal diameter. |
| `roughness` | `uom::si::f64::Length` | Absolute pipe-wall roughness. |
| `inclination` | `uom::si::f64::Angle` | Pipe inclination from horizontal, positive = uphill. |
| `q_liquid` | `uom::si::f64::VolumeRate` | Liquid-phase volumetric flow rate. |
| `q_gas` | `uom::si::f64::VolumeRate` | Gas-phase volumetric flow rate. |
| `density_liquid` | `uom::si::f64::MassDensity` | Liquid-phase mass density. |
| `density_gas` | `uom::si::f64::MassDensity` | Gas-phase mass density. |
| `viscosity_liquid` | `uom::si::f64::DynamicViscosity` | Liquid-phase dynamic viscosity. |
| `viscosity_gas` | `uom::si::f64::DynamicViscosity` | Gas-phase dynamic viscosity. |
| `surface_tension_liquid` | `uom::si::f64::SurfaceTension` | Liquid-phase surface tension (only used by<br>[`PipeFlowCorrelation::BeggsBrill`]). |

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
    fn clone(self: &Self) -> PipeFlowInputs { /* ... */ }
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
    fn eq(self: &Self, other: &PipeFlowInputs) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `PipeFlowResult`

Result of a [`PipeFlowCorrelation`] evaluation -- the correlation-specific
detail (flow regime, holdup, ...) plus the always-available total
pressure drop.

```rust
pub enum PipeFlowResult {
    BeggsBrill(beggs_brill::BeggsBrillResult),
    LockhartMartinelli(lockhart_martinelli::LockhartMartinelliResult),
}
```

##### Variants

###### `BeggsBrill`

Detail from the Beggs & Brill (1973) correlation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `beggs_brill::BeggsBrillResult` |  |

###### `LockhartMartinelli`

Detail from the Lockhart-Martinelli (1949) correlation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `lockhart_martinelli::LockhartMartinelliResult` |  |

##### Implementations

###### Methods

- ```rust
  pub fn total_pressure_drop(self: &Self) -> Pressure { /* ... */ }
  ```
  Total two-phase pressure drop (friction plus elevation), regardless

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PipeFlowResult { /* ... */ }
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
    fn eq(self: &Self, other: &PipeFlowResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `PipeFlowCorrelation`

Which two-phase gas-liquid pipe flow correlation to evaluate.

DWSIM's third correlation, Petalas-Aziz, only wraps an unshipped native
DLL upstream and has no portable source -- not represented here (see the
workspace's `op-qo2.6` bead).

```rust
pub enum PipeFlowCorrelation {
    BeggsBrill,
    LockhartMartinelli,
}
```

##### Variants

###### `BeggsBrill`

Beggs & Brill (1973) -- flow-regime-routed holdup and inclination
correction. DWSIM's default.

###### `LockhartMartinelli`

Lockhart-Martinelli (1949) -- separated-flow two-phase multiplier.

##### Implementations

###### Methods

- ```rust
  pub fn pressure_drop(self: &Self, inputs: &PipeFlowInputs) -> PipeFlowResult { /* ... */ }
  ```
  Evaluate this correlation's two-phase pressure drop for the given

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PipeFlowCorrelation { /* ... */ }
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
    fn default() -> PipeFlowCorrelation { /* ... */ }
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
    fn eq(self: &Self, other: &PipeFlowCorrelation) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `pump`

Pump duty/pressure-rise calculation, and net positive suction head (NPSH).

Ported from DWSIM `UnitOperations/Pump.vb` -- see `modes`' module doc for
the full source mapping and the flash-dependency boundary.

```rust
pub mod pump { /* ... */ }
```

### Modules

## Module `modes`

Pump duty/pressure-rise calculation modes, and NPSH.

Ported from DWSIM `UnitOperations/Pump.vb`'s `Calculate()`. DWSIM follows
each mode's algebra with a pressure-enthalpy flash to get outlet
temperature -- this pump port is deliberately kept decoupled from the
crate's flash kernel ([`crate::thermo`]), so [`PumpResult::outlet_enthalpy`]
is as far as this port goes; a caller (e.g. `tampines`, or the crate's own
flash) does the final `(p2, h2) -> T2` flash itself. DWSIM's `Curves`
calculation mode is not wired into the pump here -- though the underlying
Floater-Hormann rational interpolation of head/efficiency/NPSHr/power
vs. flow is available in [`crate::interpolation`]; see `op-qo2.9`.

```rust
pub mod modes { /* ... */ }
```

### Types

#### Struct `PumpInlet`

The pump's inlet stream state and the one liquid property (density) its
hydraulic-power calculation needs.

```rust
pub struct PumpInlet {
    pub pressure: uom::si::f64::Pressure,
    pub enthalpy: uom::si::f64::AvailableEnergy,
    pub density_liquid: uom::si::f64::MassDensity,
    pub mass_flow: uom::si::f64::MassRate,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `uom::si::f64::Pressure` | Inlet pressure. |
| `enthalpy` | `uom::si::f64::AvailableEnergy` | Inlet specific enthalpy. |
| `density_liquid` | `uom::si::f64::MassDensity` | Inlet liquid mass density. |
| `mass_flow` | `uom::si::f64::MassRate` | Mass flow rate through the pump. |

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
    fn clone(self: &Self) -> PumpInlet { /* ... */ }
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
    fn eq(self: &Self, other: &PumpInlet) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `PumpSpecification`

Which quantity specifies the pump's operating point -- exactly one
degree of freedom, matching DWSIM's `CalculationMode` enum (`Curves` is
not represented, see this module's doc).

```rust
pub enum PumpSpecification {
    DeltaP(uom::si::f64::Pressure),
    OutletPressure(uom::si::f64::Pressure),
    Power(uom::si::f64::Power),
    EnergyStreamDuty(uom::si::f64::Power),
}
```

##### Variants

###### `DeltaP`

Fixed pressure rise `ΔP = p2 - p1`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Pressure` |  |

###### `OutletPressure`

Fixed outlet pressure `p2` (equivalent to `DeltaP(p2 - p1)`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Pressure` |  |

###### `Power`

Fixed shaft power.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Power` |  |

###### `EnergyStreamDuty`

Fixed duty from an external energy stream (DWSIM's `EnergyStream`
mode) -- note this mode's enthalpy-rise relation differs from the
other three (see [`evaluate`]'s source comments): efficiency
*reduces* the usable enthalpy rise here, rather than inflating the
hydraulic power to a larger shaft power.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Power` |  |

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
    fn clone(self: &Self) -> PumpSpecification { /* ... */ }
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
    fn eq(self: &Self, other: &PumpSpecification) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PumpResult`

Result of a [`evaluate`] call.

```rust
pub struct PumpResult {
    pub outlet_pressure: uom::si::f64::Pressure,
    pub power: uom::si::f64::Power,
    pub outlet_enthalpy: uom::si::f64::AvailableEnergy,
    pub head: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `outlet_pressure` | `uom::si::f64::Pressure` | Outlet pressure. |
| `power` | `uom::si::f64::Power` | Shaft power (or externally-specified duty, in `EnergyStreamDuty` mode). |
| `outlet_enthalpy` | `uom::si::f64::AvailableEnergy` | Outlet specific enthalpy. |
| `head` | `uom::si::f64::Length` | Pump head, `(p2 - p1) / (ρ_l g)`. |

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
    fn clone(self: &Self) -> PumpResult { /* ... */ }
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
    fn eq(self: &Self, other: &PumpResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `evaluate`

Evaluate a pump's outlet state for the given [`PumpSpecification`] and
pump efficiency \[0, 1\].

```rust
pub fn evaluate(inlet: PumpInlet, spec: PumpSpecification, efficiency: uom::si::f64::Ratio) -> PumpResult { /* ... */ }
```

#### Function `npsh`

Net positive suction head available,
`NPSH = (p1 - p_bubble) / (ρ_l g)`, from the inlet pressure `p1`, the
liquid's bubble-point pressure `p_bubble` at the inlet temperature (a
`(T, VF=0)` flash, supplied by the caller), and inlet liquid density.
Returns `None` if `p1 <= p_bubble` (already at or below the bubble point
-- DWSIM returns `+infinity` here via a caught flash failure; `None` is
this port's equivalent "not meaningful" signal instead of an
infinite/NaN `Length`).

```rust
pub fn npsh(p1: uom::si::f64::Pressure, p_bubble: uom::si::f64::Pressure, density_liquid: uom::si::f64::MassDensity) -> Option<uom::si::f64::Length> { /* ... */ }
```

## Module `reactions`

Reaction model — kinetic / equilibrium / conversion reaction definitions.

Pure-Rust port of DWSIM's reaction data model and rate/equilibrium
evaluation. It supplies the stoichiometry, the Arrhenius forward/reverse
rate constants, the power-law rate expression, and the temperature-dependent
equilibrium constant `K_eq(T)` that the reactor unit operations in
[`crate::reactors`] integrate.

## Provenance (GPL-3.0)

Ported, algorithm-for-algorithm, from **DWSIM** (commit `1abf72d`,
GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros):

- `DWSIM.Interfaces/Enums.vb` — `ReactionType` (lines 436–441) and
  `ReactionBasis` (lines 443–451) enumerations, mirrored here by
  [`ReactionKind`] and [`ReactionBasis`].
- `DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb` — the `Reaction`
  class (`EvaluateK`, line 262) and `ReactionStoichBase` (line 1225),
  mirrored by [`Reaction`], [`EquilibriumConstant`], and [`ReactionComponent`].
- The Arrhenius rate constant and the power-law rate expression follow
  `DWSIM.UnitOperations/Reactors/PFR.vb` (lines 303–359) and `CSTR.vb`
  (lines 707–762): `k = A·exp(-E_a / (R·T))`,
  `rate = k_f·∏ Cᵢ^(order_i,fwd) − k_r·∏ Cᵢ^(order_i,rev)`.

## Honest scope (⚠️ untrusted draft, pending human V&V)

This is an **early-stage translation with no human verification &
validation** — untrusted draft material per the workspace `RESPONSIBLE_USE.md`.
Not for nuclear facility operation, reactor control, safety-critical, or
licensing decisions. Independent OUTRAM PARK fork, not the official DWSIM.

Deliberate simplifications versus upstream, each an honest limitation:

- DWSIM evaluates the `KOpt.Expression` equilibrium constant with an embedded
  expression engine (Flee / Mages) over an arbitrary `f(T)` string. This port
  replaces that with a fixed closed-form `ln K` correlation
  ([`EquilibriumConstant::LnPolynomial`]) — the common thermochemical form,
  not a general expression evaluator.
- DWSIM's `KOpt.Gibbs` path calls the full property package
  (`AUX_DELGig_RT`) for the ideal-gas Gibbs-energy change of reaction. This
  port uses a two-parameter van 't Hoff form
  ([`EquilibriumConstant::GibbsVantHoff`]) with a **constant** reaction
  enthalpy and entropy, i.e. temperature-independent `ΔH°`/`ΔS°`.
- DWSIM's `Heterogeneous_Catalytic` type carries a Langmuir–Hinshelwood
  `numerator / denominator` rate expression (`PFR.vb:367–421`,
  `CSTR.vb` mirror). Upstream both numerator and denominator are *arbitrary
  user-typed strings* evaluated by the Flee expression engine over the
  variables `T`, `R1…Rn` (reactant amounts), `P1…Pn` (product amounts),
  `N1…Nn` (inert amounts). This port cannot embed an expression engine (no
  new dependencies), so it ports the **canonical Langmuir–Hinshelwood
  surface rate law** structurally instead: the numerator is the existing
  Arrhenius power-law ([`Reaction::net_rate`]) and the denominator is a
  parameterised adsorption term `(1 + Σ_j K_j(T)·C_j^{m_j})^p`
  ([`LangmuirHinshelwood`]), evaluated by [`Reaction::langmuir_hinshelwood_rate`].
  The plain power-law [`Reaction::net_rate`] is left untouched and is exactly
  the LH numerator, so an empty adsorption list reduces LH to the previous
  power-law behaviour (backward-compatible). Honest scope: a *general*
  user-string num/den (as DWSIM's Flee path) is **not** ported, only the
  standard LH/Hougen–Watson algebraic form.

## Units (documented raw `f64`, SI — the DWSIM-internal convention)

Inner arithmetic uses plain `f64` in SI base units, per the crate `CLAUDE.md`
"raw f64 in inner loops" rule:

| Quantity | Unit |
|---|---|
| Temperature `T` | K |
| Activation energy `E_a` | J/mol |
| Reaction enthalpy `ΔH°`, entropy `ΔS°` | J/mol, J/(mol·K) |
| Concentration `C` | mol/m³ |
| Reaction rate `rate` | mol/(m³·s) |
| Pre-exponential factor `A` | units make `k·∏Cⁿ` come out mol/(m³·s) |
| Equilibrium constant `K` | dimensionless (basis-dependent) |

The Arrhenius pre-exponential factor `A` has whatever units render the
product `k · ∏ Cᵢ^(order)` a volumetric rate `mol/(m³·s)`; that depends on
the overall reaction order, exactly as in DWSIM (which leaves `A` and the
rate's `VelUnit` to the user).

```rust
pub mod reactions { /* ... */ }
```

### Types

#### Enum `ReactionKind`

The four DWSIM reaction types (`DWSIM.Interfaces/Enums.vb`, `ReactionType`).

This is the *classification* that selects which reactor can consume the
reaction and how its extent is determined:

- [`Conversion`](ReactionKind::Conversion) — a fixed fractional conversion of
  the base reactant is imposed (no rate, no equilibrium). Consumed by the
  conversion reactor.
- [`Equilibrium`](ReactionKind::Equilibrium) — the extent is whatever makes
  the basis-activity product equal `K_eq(T)`. Consumed by the equilibrium
  reactor.
- [`Kinetic`](ReactionKind::Kinetic) — an Arrhenius power-law rate drives the
  extent. Consumed by the CSTR and PFR.
- [`HeterogeneousCatalytic`](ReactionKind::HeterogeneousCatalytic) — surface
  (Langmuir–Hinshelwood) kinetics. The genuine LH surface rate law
  `rate = numerator / (1 + Σ_j K_j C_j^{m_j})^p` is evaluated by
  [`Reaction::langmuir_hinshelwood_rate`], with the numerator supplied by the
  Arrhenius power-law [`Reaction::net_rate`] and the adsorption denominator by
  the reaction's [`Reaction::lh`] ([`LangmuirHinshelwood`]) field. With no
  adsorption terms the denominator is `1` and the rate equals the power-law.

Enum, not a trait object, per the workspace "no `dyn`" rule — every reactor
`match`es exhaustively over it.

```rust
pub enum ReactionKind {
    Conversion,
    Equilibrium,
    Kinetic,
    HeterogeneousCatalytic,
}
```

##### Variants

###### `Conversion`

Fixed fractional conversion of the base reactant.

###### `Equilibrium`

Extent fixed by `K_eq(T)` (chemical equilibrium).

###### `Kinetic`

Arrhenius power-law rate kinetics.

###### `HeterogeneousCatalytic`

Heterogeneous catalytic (Langmuir–Hinshelwood in DWSIM; power-law
placeholder here — see the type-level note).

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
    fn clone(self: &Self) -> ReactionKind { /* ... */ }
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
    fn default() -> ReactionKind { /* ... */ }
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
    fn eq(self: &Self, other: &ReactionKind) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ReactionBasis`

The quantity a reaction's rate / equilibrium expression is written against
(`DWSIM.Interfaces/Enums.vb`, `ReactionBasis`).

DWSIM lets each reaction declare whether its concentrations are molar
concentration, partial pressure, mole fraction, etc. This port carries the
full enum for fidelity, but the reactor solvers currently exercise
[`MolarConcentration`](ReactionBasis::MolarConcentration) (kinetic reactors)
and [`MolarFraction`](ReactionBasis::MolarFraction) /
[`PartialPressure`](ReactionBasis::PartialPressure) (equilibrium reactor).
Activity- and fugacity-basis evaluation assumes ideal coefficients of unity
(an honest simplification — DWSIM calls the property package for the real
activity/fugacity coefficients).

```rust
pub enum ReactionBasis {
    Activity,
    Fugacity,
    MolarConcentration,
    MassConcentration,
    MolarFraction,
    MassFraction,
    PartialPressure,
}
```

##### Variants

###### `Activity`

Activity `aᵢ = γᵢ xᵢ` (ideal `γᵢ = 1` in this port).

###### `Fugacity`

Fugacity `fᵢ = φᵢ yᵢ P` (ideal `φᵢ = 1` in this port).

###### `MolarConcentration`

Molar concentration `Cᵢ` [mol/m³]. The default kinetic basis.

###### `MassConcentration`

Mass concentration [kg/m³].

###### `MolarFraction`

Mole fraction `xᵢ` (or `yᵢ`) [-].

###### `MassFraction`

Mass fraction [-].

###### `PartialPressure`

Partial pressure `pᵢ = yᵢ P` [Pa].

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
    fn clone(self: &Self) -> ReactionBasis { /* ... */ }
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
    fn default() -> ReactionBasis { /* ... */ }
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
    fn eq(self: &Self, other: &ReactionBasis) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ReactionComponent`

One compound's participation in a reaction — the port of DWSIM's
`ReactionStoichBase` (`ThermodynamicsBase.vb`, line 1225).

Compounds are referenced by an **index** into the reactor's component list
(`component_index`), following the workspace rule that graph/topology links
are `usize` indices rather than borrowed references (no lifetimes). The
reactor holds the master `Vec<Component>` and every reaction's
`component_index` addresses that same list.

```rust
pub struct ReactionComponent {
    pub component_index: usize,
    pub stoich_coeff: f64,
    pub direct_order: f64,
    pub reverse_order: f64,
    pub is_base_reactant: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `component_index` | `usize` | Index of this compound in the reactor's shared component list. |
| `stoich_coeff` | `f64` | Signed stoichiometric coefficient `νᵢ` [-]: **negative for reactants,<br>positive for products**, matching DWSIM's convention. |
| `direct_order` | `f64` | Forward reaction order in this compound's concentration (DWSIM<br>`DirectOrder`). Need not equal `|stoich_coeff|`. |
| `reverse_order` | `f64` | Reverse reaction order in this compound's concentration (DWSIM<br>`ReverseOrder`). |
| `is_base_reactant` | `bool` | Whether this is the reaction's *base reactant* — the compound whose<br>stoichiometric coefficient normalises the extent (DWSIM `IsBaseReactant`). |

##### Implementations

###### Methods

- ```rust
  pub fn new(component_index: usize, stoich_coeff: f64, direct_order: f64, reverse_order: f64, is_base_reactant: bool) -> Self { /* ... */ }
  ```
  Construct a reactant/product entry. `stoich_coeff` is signed (negative =

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReactionComponent { /* ... */ }
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
    fn eq(self: &Self, other: &ReactionComponent) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `EquilibriumConstant`

Temperature-dependent equilibrium constant `K_eq(T)` — the port of DWSIM's
`Reaction.EvaluateK` (`ThermodynamicsBase.vb`, line 262) and its `KOpt`
selector (`Enums.vb`).

| Variant | DWSIM `KOpt` | Formula |
|---|---|---|
| [`Constant`](EquilibriumConstant::Constant) | `Constant` | `K = K₀` |
| [`GibbsVantHoff`](EquilibriumConstant::GibbsVantHoff) | `Gibbs` (simplified) | `ln K = −ΔH°/(R T) + ΔS°/R` |
| [`LnPolynomial`](EquilibriumConstant::LnPolynomial) | `Expression` (fixed form) | `ln K = a + b/T + c·ln T + d·T` |

`GibbsVantHoff` is the pure-Rust stand-in for DWSIM's Gibbs path, which in
upstream calls the property package for the ideal-gas `ΔG°(T)/RT`. Here the
reaction supplies a **constant** standard enthalpy and entropy of reaction,
giving `ΔG°(T) = ΔH° − T ΔS°` and `K = exp(−ΔG°/(R T))` — exact only if
`ΔH°`, `ΔS°` are temperature-independent over the range of interest.

```rust
pub enum EquilibriumConstant {
    Constant(f64),
    GibbsVantHoff {
        delta_h: f64,
        delta_s: f64,
    },
    LnPolynomial {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
    },
}
```

##### Variants

###### `Constant`

A temperature-independent constant `K₀` [-].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `GibbsVantHoff`

Van 't Hoff form from constant reaction enthalpy/entropy:
`ln K = −ΔH°/(R T) + ΔS°/R`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `delta_h` | `f64` | Standard reaction enthalpy `ΔH°` [J/mol]. Endothermic (`> 0`) lowers<br>`K` with rising `T`? No — `∂ln K/∂T = ΔH°/(R T²)`, so endothermic<br>reactions have `K` *increasing* with `T`. (Sign per van 't Hoff.) |
| `delta_s` | `f64` | Standard reaction entropy `ΔS°` [J/(mol·K)]. |

###### `LnPolynomial`

Closed-form `ln K` correlation `a + b/T + c·ln T + d·T` (the fixed-form
stand-in for DWSIM's arbitrary `Expression`). Set unused coefficients to
`0.0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Constant term `a`. |
| `b` | `f64` | `1/T` coefficient `b` [K]. |
| `c` | `f64` | `ln T` coefficient `c`. |
| `d` | `f64` | `T` coefficient `d` [1/K]. |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, temperature_k: f64) -> f64 { /* ... */ }
  ```
  Evaluate the equilibrium constant `K` [-] at absolute temperature

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> EquilibriumConstant { /* ... */ }
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
    fn eq(self: &Self, other: &EquilibriumConstant) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AdsorptionTerm`

One adsorption term in a Langmuir–Hinshelwood denominator:
`K_j(T) · C_j^{order}` \[-\].

The **adsorption equilibrium constant** `K_j(T) = A_j · exp(−E_j / (R T))`
follows the same Arrhenius parameterisation the crate uses for rate constants
([`Reaction::forward_rate_constant`]). Physically `K_j` is the ratio of the
adsorption to desorption rate constants; because chemisorption is usually
**exothermic**, its adsorption enthalpy is negative, so `E_j` here (which
enters as `exp(−E_j/RT)`, i.e. `E_j = −ΔH_ads`) is typically **positive** and
makes `K_j` *decrease* with temperature. A caller who prefers to think in
`ΔH_ads` should pass `E_j = −ΔH_ads`.

## Units (SI)
| Quantity | Unit |
|---|---|
| `A_j` pre-exponential | such that `K_j · C_j^{order}` is dimensionless |
| `E_j` | J/mol |
| `order` | \[-\] (surface-coverage exponent on `C_j`) |
| concentration `C_j` | mol/m³ |

Ported structurally from the Langmuir–Hinshelwood denominator DWSIM builds
per reaction (`PFR.vb:410` `RateEquationDenominator`; the LH/Hougen–Watson
adsorption group). DWSIM evaluates a free-form string; this is the canonical
algebraic term.

```rust
pub struct AdsorptionTerm {
    pub component_index: usize,
    pub a_ads: f64,
    pub e_ads: f64,
    pub order: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `component_index` | `usize` | Index of the adsorbing compound in the reactor's shared component list. |
| `a_ads` | `f64` | Adsorption pre-exponential factor `A_j` (DWSIM: a coefficient inside the<br>denominator string). Units render `K_j · C_j^{order}` dimensionless. |
| `e_ads` | `f64` | Adsorption activation-energy parameter `E_j` \[J/mol\], entering as<br>`exp(−E_j/(R T))`. Equals `−ΔH_ads` (positive for exothermic adsorption). |
| `order` | `f64` | Surface-coverage exponent `order` \[-\] on the concentration `C_j`<br>(usually `1` for single-site molecular adsorption). |

##### Implementations

###### Methods

- ```rust
  pub fn new(component_index: usize, a_ads: f64, e_ads: f64, order: f64) -> Self { /* ... */ }
  ```
  Construct an adsorption term. For a temperature-independent adsorption

- ```rust
  pub fn adsorption_constant(self: &Self, temperature_k: f64) -> f64 { /* ... */ }
  ```
  Adsorption equilibrium constant `K_j(T) = A_j · exp(−E_j / (R T))` at

- ```rust
  pub fn value(self: &Self, concentrations: &[f64], temperature_k: f64) -> f64 { /* ... */ }
  ```
  This term's contribution `K_j(T) · C_j^{order}` \[-\] to the LH

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AdsorptionTerm { /* ... */ }
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
    fn eq(self: &Self, other: &AdsorptionTerm) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `LangmuirHinshelwood`

Langmuir–Hinshelwood adsorption denominator
`D(T, C) = (1 + Σ_j K_j(T) · C_j^{m_j})^exponent` \[-\].

This is the surface-coverage term that turns the Arrhenius power-law numerator
into a genuine LH/Hougen–Watson catalytic rate,
`rate = numerator / D` (see [`Reaction::langmuir_hinshelwood_rate`]). The `1`
represents the fraction of vacant catalyst sites; each `K_j C_j^{m_j}` is the
fraction covered by species `j`; the `exponent` `p` is the number of active
sites participating in the rate-determining step (typically `1` or `2`).

**Low-coverage (weak-adsorption) limit.** As every `K_j → 0` the denominator
`→ 1` and the LH rate reduces exactly to the power-law
[`Reaction::net_rate`] — the defining sanity check verified in the tests.

The [`Default`] is an **empty** denominator (`exponent = 1`, no terms), i.e.
`D ≡ 1`, so a reaction with no adsorption terms behaves as pure power-law
(backward-compatible with the previous `HeterogeneousCatalytic` placeholder).

Enum-free plain data (no `dyn`/`Box`/lifetimes, per the workspace rules).

```rust
pub struct LangmuirHinshelwood {
    pub adsorption_terms: Vec<AdsorptionTerm>,
    pub exponent: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `adsorption_terms` | `Vec<AdsorptionTerm>` | The adsorption terms summed inside the denominator (may be empty). |
| `exponent` | `f64` | Denominator exponent `p` \[-\] — the site count in the rate-determining<br>step. `1` for single-site, `2` for dual-site LH, etc. Must be finite. |

##### Implementations

###### Methods

- ```rust
  pub fn new(adsorption_terms: Vec<AdsorptionTerm>, exponent: f64) -> Self { /* ... */ }
  ```
  Construct an LH denominator from its adsorption terms and site exponent.

- ```rust
  pub fn denominator_value(self: &Self, concentrations: &[f64], temperature_k: f64) -> f64 { /* ... */ }
  ```
  Evaluate the denominator `D(T, C) = (1 + Σ_j K_j(T) C_j^{m_j})^p` \[-\] at

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LangmuirHinshelwood { /* ... */ }
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
    Empty denominator: `exponent = 1`, no adsorption terms, so `D ≡ 1`.

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &LangmuirHinshelwood) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Reaction`

A single reaction — stoichiometry, Arrhenius kinetics, and equilibrium
constant. Port of DWSIM's `Reaction` class (`ThermodynamicsBase.vb`, line
245), reduced to the numeric physics (no XML/GUI/expression-engine plumbing).

The reaction addresses compounds through [`ReactionComponent::component_index`]
into the reactor's shared component list. Construct with [`Reaction::new`]
and the `with_*` builders, or field-by-field.

```rust
pub struct Reaction {
    pub kind: ReactionKind,
    pub basis: ReactionBasis,
    pub components: Vec<ReactionComponent>,
    pub a_forward: f64,
    pub e_forward: f64,
    pub a_reverse: f64,
    pub e_reverse: f64,
    pub k_eq: EquilibriumConstant,
    pub conversion: f64,
    pub reaction_heat: f64,
    pub t_min: f64,
    pub t_max: f64,
    pub lh: LangmuirHinshelwood,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `ReactionKind` | Which reaction type this is (selects the reactor and extent rule). |
| `basis` | `ReactionBasis` | The basis the rate / equilibrium expression is written against. |
| `components` | `Vec<ReactionComponent>` | Every participating compound with its signed stoichiometry and orders. |
| `a_forward` | `f64` | Forward Arrhenius pre-exponential factor `A_f` (DWSIM `A_Forward`). |
| `e_forward` | `f64` | Forward activation energy `E_a,f` [J/mol] (DWSIM `E_Forward`). |
| `a_reverse` | `f64` | Reverse Arrhenius pre-exponential factor `A_r` (DWSIM `A_Reverse`). |
| `e_reverse` | `f64` | Reverse activation energy `E_a,r` [J/mol] (DWSIM `E_Reverse`). |
| `k_eq` | `EquilibriumConstant` | Equilibrium constant model `K_eq(T)` (used by the equilibrium reactor). |
| `conversion` | `f64` | Fixed fractional conversion `X ∈ [0, 1]` of the base reactant (used only<br>by the conversion reactor; DWSIM stores this as a percentage 0–100 and<br>divides by 100 — here it is already the fraction). |
| `reaction_heat` | `f64` | Standard reaction enthalpy `ΔH°` [J/mol of reaction extent], DWSIM<br>`ReactionHeat`. Positive = endothermic (absorbs heat). Used for the<br>energy balance / heat-duty accounting. |
| `t_min` | `f64` | Lower temperature bound `T_min` [K] of kinetic validity (DWSIM `Tmin`).<br>Below it the rate constants are forced to zero, per DWSIM. |
| `t_max` | `f64` | Upper temperature bound `T_max` [K] of kinetic validity (DWSIM `Tmax`). |
| `lh` | `LangmuirHinshelwood` | Langmuir–Hinshelwood adsorption denominator for a<br>[`ReactionKind::HeterogeneousCatalytic`] reaction (DWSIM's<br>`RateEquationDenominator`, `PFR.vb:410`). Empty by default (`D ≡ 1`), so<br>it has no effect on [`net_rate`](Self::net_rate) or any non-catalytic<br>reaction; only [`langmuir_hinshelwood_rate`](Self::langmuir_hinshelwood_rate)<br>consumes it. |

##### Implementations

###### Methods

- ```rust
  pub fn new(kind: ReactionKind, basis: ReactionBasis, components: Vec<ReactionComponent>) -> Self { /* ... */ }
  ```
  Construct a reaction from its kind, basis, and component list. All

- ```rust
  pub fn with_forward(self: Self, a_forward: f64, e_forward: f64) -> Self { /* ... */ }
  ```
  Set the forward Arrhenius parameters `A_f`, `E_a,f` [J/mol].

- ```rust
  pub fn with_reverse(self: Self, a_reverse: f64, e_reverse: f64) -> Self { /* ... */ }
  ```
  Set the reverse Arrhenius parameters `A_r`, `E_a,r` [J/mol].

- ```rust
  pub fn with_k_eq(self: Self, k_eq: EquilibriumConstant) -> Self { /* ... */ }
  ```
  Set the equilibrium-constant model `K_eq(T)`.

- ```rust
  pub fn with_conversion(self: Self, conversion: f64) -> Self { /* ... */ }
  ```
  Set the fixed fractional conversion `X ∈ [0, 1]` of the base reactant.

- ```rust
  pub fn with_reaction_heat(self: Self, reaction_heat: f64) -> Self { /* ... */ }
  ```
  Set the standard reaction enthalpy `ΔH°` [J/mol of extent].

- ```rust
  pub fn with_langmuir_hinshelwood(self: Self, lh: LangmuirHinshelwood) -> Self { /* ... */ }
  ```
  Set the Langmuir–Hinshelwood adsorption denominator (consumed only by

- ```rust
  pub fn base_stoich_coeff(self: &Self) -> f64 { /* ... */ }
  ```
  The base reactant's signed stoichiometric coefficient `ν_BC`. Returns the

- ```rust
  pub fn base_component_index(self: &Self) -> usize { /* ... */ }
  ```
  The base reactant's `component_index`. See [`base_stoich_coeff`](Self::base_stoich_coeff)

- ```rust
  pub fn forward_rate_constant(self: &Self, temperature_k: f64) -> f64 { /* ... */ }
  ```
  Forward rate constant `k_f = A_f · exp(−E_a,f / (R T))` at temperature

- ```rust
  pub fn reverse_rate_constant(self: &Self, temperature_k: f64) -> f64 { /* ... */ }
  ```
  Reverse rate constant `k_r = A_r · exp(−E_a,r / (R T))` at temperature

- ```rust
  pub fn net_rate(self: &Self, concentrations: &[f64], temperature_k: f64) -> f64 { /* ... */ }
  ```
  Net volumetric reaction rate `[mol/(m³·s)]` at temperature `temperature_k`

- ```rust
  pub fn langmuir_hinshelwood_rate(self: &Self, concentrations: &[f64], temperature_k: f64) -> f64 { /* ... */ }
  ```
  Net **Langmuir–Hinshelwood** surface reaction rate `[mol/(m³·s)]` at

- ```rust
  pub fn equilibrium_constant(self: &Self, temperature_k: f64) -> f64 { /* ... */ }
  ```
  Evaluate `K_eq(T)` at temperature `temperature_k` [K] (delegates to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Reaction { /* ... */ }
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
    fn eq(self: &Self, other: &Reaction) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Constant `R_GAS`

Universal gas constant `R` [J/(mol·K)].

The literal `8.314` matches DWSIM's hard-coded value in the reactor rate
laws (`PFR.vb` line 307, `CSTR.vb` line 711), retained verbatim so the port
reproduces upstream numbers exactly rather than using a more precise CODATA
value.

```rust
pub const R_GAS: f64 = 8.314;
```

## Module `reactors`

Reactor unit operations — conversion, equilibrium, CSTR, and PFR.

Pure-Rust port of DWSIM's `DWSIM.UnitOperations/Reactors/` unit operations,
built on the reaction model in [`crate::reactions`]. Each reactor takes a
[`ReactorFeed`] (inlet molar flows, `T`, `P`, volumetric flow) and returns a
[`ReactorOutcome`] (outlet molar flows, per-reaction extents, heat of
reaction).

## Provenance (GPL-3.0)

Ported from **DWSIM** (commit `1abf72d`, GPL-3.0; upstream copyright Daniel
Wagner O. de Medeiros):

- [`conversion_reactor`] ← `Reactors/Conversion.vb`
- [`equilibrium_reactor`] ← `Reactors/Equilibrium.vb`
- [`gibbs_reactor`] ← `Reactors/Gibbs.vb`
- [`cstr`] ← `Reactors/CSTR.vb`
- [`pfr`] ← `Reactors/PFR.vb`

## Design

Enum dispatch, no trait objects (workspace "no `dyn`" rule): the reactor
choice is the closed set [`ReactorModel`], which `match`es to the wrapped
reactor's own `solve`. Compounds are addressed by index into the feed's
molar-flow vector; every reaction's `component_index` shares that index
space.

## Honest scope (⚠️ untrusted draft, pending human V&V)

Early-stage translation, **no human V&V** — untrusted draft material
(workspace `RESPONSIBLE_USE.md`). Not for nuclear facility operation, reactor
control, safety-critical, or licensing decisions. Independent OUTRAM PARK
fork, not the official DWSIM.

Simplifications versus upstream, each an honest limitation:

- **Constant volumetric flow.** DWSIM re-flashes the mixture each iteration
  to update the volumetric flow `Q` as composition changes. This port holds
  `Q` fixed at the feed value (exact for equimolar / liquid-phase reactions,
  approximate when the mole count changes). The reactor solvers are decoupled
  from [`crate::thermo`]'s flash, matching how [`crate::pump`] is decoupled.
- **Isothermal solve.** The reactors solve the material balance at the feed
  temperature. The heat of reaction is *reported* ([`ReactorOutcome::heat_of_reaction`])
  but not fed back into an energy balance to update `T` (DWSIM's adiabatic /
  outlet-T modes are not ported).
- **Ideal equilibrium basis.** The equilibrium reactor uses mole-fraction or
  partial-pressure activity with unit activity/fugacity coefficients.

```rust
pub mod reactors { /* ... */ }
```

### Modules

## Module `conversion_reactor`

Fixed-conversion reactor — port of DWSIM `Reactors/Conversion.vb`.

## Provenance (GPL-3.0)

Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/Conversion.vb` (commit
`1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). The
per-compound mole-flow update mirrors the delta-mole-flow loop at lines
622–702: `Δnᵢ = −X · νᵢ / ν_BC · n_BC`, where `X` is the specified fractional
conversion of the base reactant, `ν` the stoichiometric coefficients, and
`n_BC` the base reactant's inlet molar flow.

## Model

Each [`ReactionKind::Conversion`](crate::reactions::ReactionKind::Conversion)
reaction imposes a fixed fractional conversion `X ∈ [0, 1]` of its base
reactant. There is no rate and no equilibrium — the extent follows directly
from `X`:

`ζ_r = −X_r · n_BC / ν_BC`   (extent as written, `[mol/s]`)

and `Δnᵢ = νᵢ · ζ_r`. Reactions are applied in list order (DWSIM's
sequential-group treatment); each reactant flow is clamped at zero so a
later reaction cannot drive a compound negative.

⚠️ Untrusted draft, pending human V&V (see [`crate::reactors`]).

```rust
pub mod conversion_reactor { /* ... */ }
```

### Types

#### Struct `ConversionReactor`

A fixed-conversion reactor holding a list of conversion reactions applied in
order.

```rust
pub struct ConversionReactor {
    pub reactions: Vec<crate::reactions::Reaction>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reactions` | `Vec<crate::reactions::Reaction>` | The reactions to apply; each should be<br>[`ReactionKind::Conversion`](crate::reactions::ReactionKind::Conversion)<br>with its [`Reaction::conversion`] set. |

##### Implementations

###### Methods

- ```rust
  pub fn new(reactions: Vec<Reaction>) -> Self { /* ... */ }
  ```
  Construct a conversion reactor from its reaction list.

- ```rust
  pub fn solve(self: &Self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> { /* ... */ }
  ```
  Apply every reaction's fixed conversion to the `feed`, returning the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ConversionReactor { /* ... */ }
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
    fn eq(self: &Self, other: &ConversionReactor) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `cstr`

Continuous stirred-tank reactor (CSTR) — port of DWSIM `Reactors/CSTR.vb`.

## Provenance (GPL-3.0)

Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/CSTR.vb` (commit
`1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). The
rate law (`k = A·exp(−E/(R·T))`, power-law `rate = kf·∏Cᵈ − kr·∏Cʳ`) mirrors
lines 707–762, and the well-mixed molar balance `F_out = F_in + V·R` mirrors
the inventory update at lines 873–899. DWSIM converges the tank contents with
a pseudo-transient relaxation loop; this port instead solves the equivalent
**steady-state algebraic balance directly** with a damped Newton iteration on
the reaction extents (falling back to scalar Newton for a single reaction).

## Model

A CSTR is perfectly mixed, so the outlet composition equals the tank
composition. At steady state the molar balance for each compound is

`Fᵢ,out = Fᵢ,in + V · Rᵢ(C_out)`,   `Cᵢ = Fᵢ,out / Q`,

with `Rᵢ = Σ_r (−rate_r · νᵢᵣ / ν_BC,r)`. Introducing the per-reaction extent
`ζ_r = V · rate_r(C_out)` [mol/s] gives `Fᵢ,out = Fᵢ,in + Σ_r (−νᵢᵣ/ν_BC,r) ζ_r`,
and the unknowns `ζ` are found from the residual

`g_r(ζ) = ζ_r − V · rate_r(C_out(ζ)) = 0`.

For the single-reaction first-order case this reproduces the textbook CSTR
result `X = k·τ / (1 + k·τ)`, `τ = V/Q`.

⚠️ Untrusted draft, pending human V&V (see [`crate::reactors`]).

```rust
pub mod cstr { /* ... */ }
```

### Types

#### Struct `Cstr`

A continuous stirred-tank reactor: a reaction list and the tank volume.

```rust
pub struct Cstr {
    pub reactions: Vec<crate::reactions::Reaction>,
    pub volume: f64,
    pub max_iter: usize,
    pub tol: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reactions` | `Vec<crate::reactions::Reaction>` | The kinetic reactions driving the balance. |
| `volume` | `f64` | Reactor (tank) volume `V` [m³]. |
| `max_iter` | `usize` | Maximum Newton iterations (default via [`Cstr::new`]: 200). |
| `tol` | `f64` | Convergence tolerance on the residual norm (default: `1e−10`). |

##### Implementations

###### Methods

- ```rust
  pub fn new(reactions: Vec<Reaction>, volume: f64) -> Self { /* ... */ }
  ```
  Construct a CSTR with default solver settings (`max_iter = 200`,

- ```rust
  pub fn solve(self: &Self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> { /* ... */ }
  ```
  Solve the steady-state CSTR balance for the `feed`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Cstr { /* ... */ }
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
    fn eq(self: &Self, other: &Cstr) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `equilibrium_reactor`

Chemical-equilibrium reactor — port of DWSIM `Reactors/Equilibrium.vb`.

## Provenance (GPL-3.0)

Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/Equilibrium.vb` (commit
`1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). The
reaction-extent formulation `nᵢ = nᵢ,0 + Σ_r Eᵢᵣ ζ_r` and the per-reaction
residual `f_r = ln(∏ᵢ basisᵢ^νᵢ) − ln K_r` mirror `FunctionValue2N`
(lines 234–381: mole update lines 244–253, basis product lines 332–363,
`f(i) = ln(prod/kr)` line 379, with `kr = EvaluateK(T + Approach)`).

## Model

An equilibrium reactor finds the reaction extents `ζ` such that every
reaction simultaneously satisfies its equilibrium constant:

`∏ᵢ (basisᵢ)^νᵢ = K_r(T)`   ⇔   `f_r(ζ) = ln(∏ᵢ basisᵢ^νᵢ) − ln K_r = 0`,

with the mole amounts parameterised by the extents, `nᵢ = nᵢ,0 + Σ_r νᵢᵣ ζ_r`
(guaranteeing the atom balance by construction). The nonlinear system is
solved by a damped Newton iteration (finite-difference Jacobian, Gaussian
elimination) that keeps all `nᵢ ≥ 0`.

The activity basis is evaluated **ideally** (activity/fugacity coefficients
= 1): [`ReactionBasis::MolarFraction`] uses `xᵢ`,
[`ReactionBasis::PartialPressure`] uses `xᵢ·P` [Pa], and
[`ReactionBasis::Activity`] is treated as `xᵢ`. This is an honest
simplification — DWSIM calls the property package for real fugacity
coefficients.

⚠️ Untrusted draft, pending human V&V (see [`crate::reactors`]).

```rust
pub mod equilibrium_reactor { /* ... */ }
```

### Types

#### Struct `EquilibriumReactor`

A chemical-equilibrium reactor: a list of equilibrium reactions solved
simultaneously for their extents.

```rust
pub struct EquilibriumReactor {
    pub reactions: Vec<crate::reactions::Reaction>,
    pub max_iter: usize,
    pub tol: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reactions` | `Vec<crate::reactions::Reaction>` | The equilibrium reactions (each carries its<br>[`k_eq`](Reaction::k_eq) model). |
| `max_iter` | `usize` | Maximum Newton iterations (default via [`EquilibriumReactor::new`]: 300). |
| `tol` | `f64` | Convergence tolerance on the `ln`-residual norm (default: `1e−10`). |

##### Implementations

###### Methods

- ```rust
  pub fn new(reactions: Vec<Reaction>) -> Self { /* ... */ }
  ```
  Construct an equilibrium reactor with default solver settings.

- ```rust
  pub fn solve(self: &Self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> { /* ... */ }
  ```
  Solve the equilibrium reactor for the `feed`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> EquilibriumReactor { /* ... */ }
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
    fn eq(self: &Self, other: &EquilibriumReactor) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `gibbs_reactor`

Gibbs-minimisation equilibrium reactor — port of DWSIM `Reactors/Gibbs.vb`.

## Provenance (GPL-3.0)

Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/Gibbs.vb` (commit
`1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). DWSIM's
Gibbs reactor computes the outlet speciation of a feed **without a reaction
list**, by minimising the total Gibbs energy of the mixture subject to
element (atom) mass balance — the "non-stoichiometric" / element-abundance
formulation (`Gibbs.vb`: the `FunctionValue` / `MinimizeError` objective is
`Σ_i n_i·(g°_i/RT + ln(f_i))`, minimised over the element-balance constraint
`Σ_i a_ki n_i = b_k`). Upstream delegates the constrained optimisation to an
external solver (IPOPT / DotNumerics); this port reuses the crate's pure-Rust
RAND / element-potential minimiser [`crate::thermo::gibbs::GibbsSystem`],
which encodes the identical objective and constraints (see that module's
provenance for the `GibbsMinimization*.vb` line citations).

## Model

A Gibbs reactor takes a feed of species molar flows and returns the
equilibrium outlet molar flows that **minimise `G/RT`** at the feed `T`, `P`
subject to conservation of every chemical element — no reaction
stoichiometry, rate, or `K_eq` list is supplied. Which species can appear and
how they interconvert is encoded entirely in the atom matrix `a_ki` (atoms of
element `k` per molecule of species `i`) carried by the wrapped
[`GibbsSystem`]. The per-species standard molar Gibbs energy of formation
`g°_i(T)` \[J/mol\] is supplied by a [`GibbsFormation`] model per species.

At the minimum every species satisfies the element-potential relation
`μ_i/RT = Σ_k a_ki π_k`, which is *equivalent* to every atom-conserving
reaction simultaneously satisfying its `K_eq = exp(−ΔG°/RT)` — so a Gibbs
reactor reproduces the [`super::EquilibriumReactor`] answer without ever being
given the reaction (verified in the V&V tests below).

## Units (SI)

| Quantity | Unit |
|---|---|
| Molar flow (feed & outlet) | mol/s |
| Temperature `T` | K |
| Pressure `P`, reference `P°` | Pa |
| Standard Gibbs energy of formation `g°_i` | J/mol |
| Heat of reaction | W |

The minimiser is scale-agnostic (homogeneous of degree 1 in the feed
amounts), so feeding molar **flows** \[mol/s\] returns outlet molar **flows**
\[mol/s\] directly.

## Honest scope (⚠️ untrusted AI-assisted draft, pending human V&V)

Early-stage translation, **no human V&V** — untrusted draft material
(workspace `RESPONSIBLE_USE.md`). Not for nuclear facility operation, reactor
control, safety-critical, or licensing decisions. Independent OUTRAM PARK
fork, not the official DWSIM. Verification (against closed-form equilibrium
and the equilibrium-constant reactor), not validation against measured data.

Simplifications versus upstream, each an honest limitation:

- **Single gas phase only.** Inherits [`GibbsSystem`]'s single-phase,
  no-condensed-phase scope (DWSIM's Gibbs reactor supports multi-phase and
  solid carbon; that is future work). No vapour–liquid split.
- **Caller-supplied `g°_i(T)`.** DWSIM pulls `AUX_DELGF_T` from the property
  package; this port takes the standard Gibbs energy of formation from a
  simple [`GibbsFormation`] model (constant, or a two-parameter
  `g° = ΔH_f − T·ΔS_f`). No property-package coupling.
- **Ideal-gas / frozen fugacity.** Uses the [`FugacityModel`] passed through
  to [`GibbsSystem::minimize`]; a self-consistent EOS coupling is not wired.
- **Isothermal.** Solves at the feed temperature. The heat of reaction is
  *reported* (from formation enthalpies when available) but not fed back into
  an energy balance to update `T`, matching the other reactors in
  [`crate::reactors`].
- **No per-reaction extents.** Being reaction-free, the returned
  [`ReactorOutcome::extents`] is empty (the concept does not apply).

```rust
pub mod gibbs_reactor { /* ... */ }
```

### Types

#### Enum `GibbsFormation`

Standard molar Gibbs energy of formation model `g°_i(T)` \[J/mol\] for one
species (enum dispatch, no `dyn`).

Only **differences** between species matter to the equilibrium (a common
additive offset cancels in [`GibbsSystem::minimize`]), so any consistent
reference shared by all species works — e.g. `g°_i = ΔG°_{f,i}(T)`, or an
element-referenced set.

```rust
pub enum GibbsFormation {
    Constant(f64),
    EnthalpyEntropy {
        delta_h_f: f64,
        delta_s_f: f64,
    },
}
```

##### Variants

###### `Constant`

Temperature-independent standard Gibbs energy of formation `g°` \[J/mol\].
Carries no separate enthalpy, so it contributes an *unknown* heat of
formation to the heat-of-reaction accounting (that term is then omitted).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `EnthalpyEntropy`

Two-parameter form `g°(T) = ΔH_f − T·ΔS_f` \[J/mol\] with **constant**
standard enthalpy and entropy of formation (exact only when `ΔH_f`,
`ΔS_f` are temperature-independent over the range of interest). The
`ΔH_f` term also feeds the heat-of-reaction report.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `delta_h_f` | `f64` | Standard enthalpy of formation `ΔH_f` \[J/mol\]. |
| `delta_s_f` | `f64` | Standard entropy of formation `ΔS_f` \[J/(mol·K)\]. |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, temperature_k: f64) -> f64 { /* ... */ }
  ```
  Evaluate `g°_i(T)` \[J/mol\] at temperature `temperature_k` \[K\].

- ```rust
  pub fn enthalpy_of_formation(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Standard enthalpy of formation `ΔH_f` \[J/mol\] if this model carries one

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GibbsFormation { /* ... */ }
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
    fn eq(self: &Self, other: &GibbsFormation) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GibbsReactor`

A Gibbs-minimisation equilibrium reactor: a reacting system (species,
elements, atom matrix) plus each species' standard Gibbs energy of formation.

[`solve`](Self::solve) returns the equilibrium outlet molar flows that
minimise `G/RT` at the feed conditions — no reaction list is used.

```rust
pub struct GibbsReactor {
    pub system: crate::thermo::gibbs::GibbsSystem,
    pub gibbs_formation: Vec<GibbsFormation>,
    pub p_ref: f64,
    pub fugacity: crate::thermo::gibbs::FugacityModel,
    pub options: crate::thermo::gibbs::GibbsOptions,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `system` | `crate::thermo::gibbs::GibbsSystem` | The reacting system: species names, element symbols, and atom matrix<br>(who is made of what). Determines the feasible speciation. |
| `gibbs_formation` | `Vec<GibbsFormation>` | Standard Gibbs energy of formation model `g°_i(T)` for each species, in<br>species order. Length must equal `system.n_species()`. |
| `p_ref` | `f64` | Reference pressure `P°` \[Pa\] entering the composition term<br>`ln(y_i P/P°)`. Conventionally `1e5` Pa (1 bar). Must be `> 0`. |
| `fugacity` | `crate::thermo::gibbs::FugacityModel` | Fugacity model for the gas-phase chemical potential (`IdealGas` for<br>`φ_i = 1`). |
| `options` | `crate::thermo::gibbs::GibbsOptions` | Convergence / iteration controls for the RAND minimiser. |

##### Implementations

###### Methods

- ```rust
  pub fn new(system: GibbsSystem, gibbs_formation: Vec<GibbsFormation>) -> Self { /* ... */ }
  ```
  Construct a Gibbs reactor with default solver settings, an ideal-gas

- ```rust
  pub fn with_p_ref(self: Self, p_ref: f64) -> Self { /* ... */ }
  ```
  Set the reference pressure `P°` \[Pa\].

- ```rust
  pub fn with_fugacity(self: Self, fugacity: FugacityModel) -> Self { /* ... */ }
  ```
  Set the fugacity model.

- ```rust
  pub fn with_options(self: Self, options: GibbsOptions) -> Self { /* ... */ }
  ```
  Set the RAND minimiser options.

- ```rust
  pub fn solve(self: &Self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> { /* ... */ }
  ```
  Solve the Gibbs reactor for the `feed`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GibbsReactor { /* ... */ }
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
    fn eq(self: &Self, other: &GibbsReactor) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `pfr`

Plug-flow reactor (PFR) — port of DWSIM `Reactors/PFR.vb`.

## Provenance (GPL-3.0)

Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/PFR.vb` (commit
`1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). The
molar-balance ODE and the per-compound production term
`dNᵢ/dV = −rate · νᵢ / ν_BC` mirror the `ODEFunc` derivative assembly at lines
260–500 (rate law lines 303–359; production sum lines 461–470; the returned
`dy = −Ri` at lines 475–481). DWSIM offers several stiff/non-stiff ODE
integrators (`InternalSolver`, lines 1108–1170); this port uses a fixed-step
classical Runge–Kutta 4 marching over reactor volume.

## Model

A PFR integrates the steady-state molar balance along the reactor volume `V`
for each compound `i`:

`dFᵢ/dV = Σ_r ( −rate_r(C) · νᵢᵣ / ν_BC,r )`,   `Cᵢ = Fᵢ / Q`

from `V = 0` (the feed) to `V = volume`, with the volumetric flow `Q` held
constant (see [`crate::reactors`] "Honest scope"). `rate_r` is the power-law
rate from [`Reaction::net_rate`]. The per-reaction extent
`ζ_r = ∫₀^V rate_r dV` [mol/s] is accumulated with the same RK4 weights.

⚠️ Untrusted draft, pending human V&V (see [`crate::reactors`]).

```rust
pub mod pfr { /* ... */ }
```

### Types

#### Struct `Pfr`

A plug-flow reactor: a reaction list, a total volume, and the number of RK4
integration sub-steps.

```rust
pub struct Pfr {
    pub reactions: Vec<crate::reactions::Reaction>,
    pub volume: f64,
    pub n_steps: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reactions` | `Vec<crate::reactions::Reaction>` | The kinetic reactions driving the balance (typically<br>[`ReactionKind::Kinetic`](crate::reactions::ReactionKind::Kinetic)). |
| `volume` | `f64` | Total reactor volume `V` [m³]. |
| `n_steps` | `usize` | Number of fixed RK4 sub-steps over `[0, V]`. More steps = more accurate;<br>100–1000 is ample for the smooth balances here. |

##### Implementations

###### Methods

- ```rust
  pub fn new(reactions: Vec<Reaction>, volume: f64, n_steps: usize) -> Self { /* ... */ }
  ```
  Construct a PFR. `n_steps` is clamped to at least 1.

- ```rust
  pub fn solve(self: &Self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> { /* ... */ }
  ```
  Integrate the PFR balance from the `feed` to the reactor outlet.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Pfr { /* ... */ }
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
    fn eq(self: &Self, other: &Pfr) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Types

#### Struct `ReactorFeed`

Inlet state of a reactor: the per-compound molar flows plus the intensive
conditions the reactors need.

`molar_flows` is indexed by compound; every reaction's
[`ReactionComponent::component_index`](crate::reactions::ReactionComponent::component_index)
addresses this same vector.

```rust
pub struct ReactorFeed {
    pub molar_flows: Vec<f64>,
    pub temperature: f64,
    pub pressure: f64,
    pub volumetric_flow: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `molar_flows` | `Vec<f64>` | Inlet molar flow of each compound `[mol/s]`, by component index. |
| `temperature` | `f64` | Temperature `T` [K]. |
| `pressure` | `f64` | Pressure `P` [Pa]. |
| `volumetric_flow` | `f64` | Total volumetric flow `Q` [m³/s] (held constant through the reactor —<br>see the module "Honest scope" note). Used to turn molar flows into<br>concentrations `Cᵢ = Fᵢ / Q` for the kinetic reactors. |

##### Implementations

###### Methods

- ```rust
  pub fn new(molar_flows: Vec<f64>, temperature: f64, pressure: f64, volumetric_flow: f64) -> Self { /* ... */ }
  ```
  Construct a feed. `volumetric_flow` may be `0.0` for reactors that do not

- ```rust
  pub fn total_molar_flow(self: &Self) -> f64 { /* ... */ }
  ```
  Total molar flow `Σᵢ Fᵢ` [mol/s].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReactorFeed { /* ... */ }
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
    fn eq(self: &Self, other: &ReactorFeed) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ReactorOutcome`

Result of a reactor solve.

```rust
pub struct ReactorOutcome {
    pub molar_flows: Vec<f64>,
    pub extents: Vec<f64>,
    pub heat_of_reaction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `molar_flows` | `Vec<f64>` | Outlet molar flow of each compound `[mol/s]`, by component index. |
| `extents` | `Vec<f64>` | Per-reaction extent `[mol/s]` (the reaction-as-written progress, one<br>entry per reaction in the reactor's list). For flow reactors this is the<br>reaction rate integrated over the reactor volume. |
| `heat_of_reaction` | `f64` | Net heat of reaction `[W]` = `Σ_r ΔH°_r · ζ_r`. Positive = net<br>endothermic (heat must be supplied to hold the feed temperature). |

##### Implementations

###### Methods

- ```rust
  pub fn conversion_of(self: &Self, feed: &ReactorFeed, component_index: usize) -> f64 { /* ... */ }
  ```
  Fractional conversion of the compound at `component_index`, relative to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReactorOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &ReactorOutcome) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ReactorError`

What can go wrong in a reactor solve.

```rust
pub enum ReactorError {
    NonConvergence {
        iterations: usize,
        residual: f64,
    },
    InvalidFeed(String),
}
```

##### Variants

###### `NonConvergence`

The iterative solver did not converge within its iteration budget.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations attempted. |
| `residual` | `f64` | Final residual norm. |

###### `InvalidFeed`

The feed was malformed (e.g. a kinetic reactor with `Q ≤ 0`, or a
component index out of range).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

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
    fn clone(self: &Self) -> ReactorError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReactorError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ReactorModel`

The closed set of reactor unit operations — enum dispatch, no `dyn`.

Each variant wraps a fully-configured reactor (reactions + geometry).
[`solve`](ReactorModel::solve) `match`es to the wrapped reactor's own solve,
so adding a variant forces every dispatch site to handle it.

```rust
pub enum ReactorModel {
    Conversion(ConversionReactor),
    Equilibrium(EquilibriumReactor),
    Gibbs(GibbsReactor),
    Cstr(Cstr),
    Pfr(Pfr),
}
```

##### Variants

###### `Conversion`

Fixed-conversion reactor (`Reactors/Conversion.vb`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ConversionReactor` |  |

###### `Equilibrium`

Chemical-equilibrium reactor (`Reactors/Equilibrium.vb`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `EquilibriumReactor` |  |

###### `Gibbs`

Gibbs-energy-minimisation equilibrium reactor (`Reactors/Gibbs.vb`) —
outlet speciation from a feed with **no reaction list**.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `GibbsReactor` |  |

###### `Cstr`

Continuous stirred-tank reactor (`Reactors/CSTR.vb`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Cstr` |  |

###### `Pfr`

Plug-flow reactor (`Reactors/PFR.vb`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Pfr` |  |

##### Implementations

###### Methods

- ```rust
  pub fn solve(self: &Self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> { /* ... */ }
  ```
  Solve this reactor for the given `feed`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReactorModel { /* ... */ }
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
    fn eq(self: &Self, other: &ReactorModel) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Re-export `ConversionReactor`

```rust
pub use conversion_reactor::ConversionReactor;
```

#### Re-export `Cstr`

```rust
pub use cstr::Cstr;
```

#### Re-export `EquilibriumReactor`

```rust
pub use equilibrium_reactor::EquilibriumReactor;
```

#### Re-export `GibbsFormation`

```rust
pub use gibbs_reactor::GibbsFormation;
```

#### Re-export `GibbsReactor`

```rust
pub use gibbs_reactor::GibbsReactor;
```

#### Re-export `Pfr`

```rust
pub use pfr::Pfr;
```

## Module `separator`

Two-phase separator / flash drum: flash a combined feed into vapour + liquid
outlet streams.

Pure-Rust port of DWSIM's gas-liquid separator vessel, from
`UnitOperations/Vessel.vb` (GPL-3.0, `Public Overrides Sub Calculate`, lines
653-1140). Upstream copyright: Daniel Wagner O. de Medeiros. DWSIM itself
describes the unit as a *flash drum*: "The separator vessel simply divides
the inlet stream phases into two or three distinct streams. If the user
defines values for the separation temperature and/or pressure, a TP Flash is
done in the new conditions before the distribution of phases through the
outlet streams." (Vessel.vb:664-668).

This is the **first equipment model in the crate that actually invokes the
thermodynamic flash kernel** ([`crate::thermo::property_package`]): the
upstream mixer/splitter/expander push the flash to the caller, whereas a
separator's whole job *is* the flash and the routing of its phases.

# What this computes (the material-balance flash)

Given a single **combined feed** — total molar flow `F` \[mol/s\], overall
mole-fraction composition `z_i` \[-\], and (for the adiabatic mode) a feed
specific enthalpy — plus the vessel conditions, the separator runs a
two-phase flash and routes the equilibrium phases to two outlets:

- **Vapour outlet** — molar flow `V = β·F`, composition `y_i` (DWSIM writes
  `Phases(2)` — the vapour phase — onto `OutputConnectors(0)`,
  Vessel.vb:952-971).
- **Liquid outlet** — molar flow `L = (1−β)·F`, composition `x_i` (DWSIM
  writes the liquid phase onto `OutputConnectors(1)`, Vessel.vb:986-1045).

where `β` \[-\] is the vapour molar fraction from the flash. Two operating
specifications are ported (DWSIM's `CalculationModes`, Vessel.vb:99-106):

- **Isothermal TP** ([`Separator::flash_isothermal`]) — DWSIM `Legacy` mode
  with an overridden separation `T`/`P` (Vessel.vb:820-844): a
  temperature-and-pressure flash at the vessel `T`, `P`. This uses
  [`crate::thermo::property_package::PropertyPackageModel::flash_pt`]
  directly.
- **Adiabatic** ([`Separator::flash_adiabatic`]) — DWSIM `Adiabatic` mode
  (Vessel.vb:793-818): the vessel `T` is unknown and is found by a
  pressure-enthalpy (PH) flash at the fixed vessel `P` from the feed
  enthalpy. To keep this module decoupled from the energy-flash driver (and
  free of `dyn`), the PH step is a **caller-supplied generic `Fn` closure**
  — a caller typically wraps [`crate::thermo::energy_flash::flash_ph`].

# Mass conservation

Because the flash defines `x_i`, `y_i`, `β` so that
`z_i = (1−β) x_i + β y_i` holds identically (see
[`crate::thermo::flash`]), routing `V = β·F` to the vapour and `L = (1−β)·F`
to the liquid conserves total moles (`F = V + L`) and per-component moles
(`z_i F = y_i V + x_i L`) exactly. Multiplying each per-component molar flow
by the fixed component molar mass `M_i` gives the same conservation on a
**mass** basis, so the phase mass flows reported here satisfy
`w_feed = w_vapour + w_liquid` to round-off. This is the pure
material-balance content of DWSIM's routine.

# Units — `uom` at the boundary, documented `f64` (SI) inside

Public flows/temperatures/pressures are `uom`-typed (per the crate
`CLAUDE.md`): molar flow as [`MolarFlowRate`] (katal = mol/s), mass flow as
`MassRate` (kg/s), `T` as `ThermodynamicTemperature` (K), `P` as `Pressure`
(Pa), compositions as `Ratio` (mole fractions \[-\]). The flash kernel it
sits on works in raw `f64` SI (`T` \[K\], `P` \[Pa\], mole fractions \[-\]),
so the internals convert at the boundary.

# Design (workspace + crate `CLAUDE.md`)

Enum dispatch, **no `dyn`**: the operating specification is the closed
[`SeparatorMode`] enum, not a trait object; the one caller-dependent step
(the adiabatic PH flash) is a **generic `Fn` closure**, exactly as in
[`crate::expander`] and [`crate::thermo::flash`]. No `Box`, no lifetimes, no
channels; feed/outlet data owned by value.

# Honest scope — the material-balance flash separator only

This ports **only** the material-balance / flash-and-route behaviour of
DWSIM's vessel. Deliberately **excluded** (present in Vessel.vb, out of
scope here — no physics content for the flash, or beyond a two-phase
gas-liquid balance):

- **Vessel sizing / hydraulics** — `CalculateVolume`, the head-type geometry
  (`HeadTypes`), the `DimensionRatio`/`SurgeFactor`/`ResidenceTime` and the
  Souders-Brown-style vapour-velocity/liquid-level sizing (Vessel.vb:34-114,
  223-283, 434-651).
- **Liquid-level / holdup dynamics** — the entire dynamic-mode
  `RunDynamicModel` accumulation-stream integration and the dynamic-property
  editor (Vessel.vb:155-433).
- **Water decant / three-phase (VLLE) split** — DWSIM's second liquid phase
  `Phases(4)` and the solid phase `Phases(7)` distribution
  (Vessel.vb:893-950, 1046-1100). This port is a **two-phase** (single
  vapour + single liquid) separator; the flash kernel it uses is two-phase
  ([`crate::thermo::flash`]).
- **Heating/cooling modes** — `HeatingCoolingIsothermic` /
  `HeatingCoolingIsobaric` with an attached energy stream and the `DeltaQ`
  duty back-calculation (Vessel.vb:846-889, 1118-1130).
- **GUI / serialization / flowsheet plumbing** — the editing forms, icon
  resources, XML/JSON persistence, `Inspector` trace paragraphs, and the
  property-grid reflection accessors (Vessel.vb:170-222, 655-668,
  1140-1744).

**Verification, not validation.** The tests below check the port's algebra
and the flash kernel's mass balance against hand values; they are **not**
validated against experimental separator data. AI-assisted port — untrusted
draft material until human-reviewed per the crate `CLAUDE.md`. Not for
nuclear facility operation, reactor control, safety-critical, or licensing
decisions. Independent OUTRAM PARK fork, not the official DWSIM.

```rust
pub mod separator { /* ... */ }
```

### Types

#### Type Alias `MolarFlowRate`

Molar (mole) flow rate — amount of substance per unit time.

`uom` 0.38 has no dedicated `MolarFlowRate` quantity, but a molar flow is
dimensionally `T⁻¹N` (mol · s⁻¹), which is exactly the SI unit **katal**
(`CatalyticActivity`). This alias (matching [`crate::splitter::MolarFlowRate`])
gives the separator's public API a human-readable name for that quantity
while reusing the correct `uom` dimension. Base unit: **katal = mol/s**.

```rust
pub type MolarFlowRate = uom::si::f64::CatalyticActivity;
```

#### Struct `SeparatorFeed`

The combined feed to the separator: everything the material-balance flash
needs, owned by value (no references, no lifetimes).

This is the already-mixed inlet DWSIM builds in `MixedStream`
(Vessel.vb:691-789) before the flash — the crate's [`crate::mixer`] produces
exactly this combined state.

Units (SI, `uom`-typed):
- `molar_flow` — total feed molar flow `F` \[katal = mol/s\], `>= 0`, finite.
- `composition` — overall feed mole fractions `z_i` \[-\], one per component,
  physically summing to 1.
- `specific_enthalpy` — feed specific enthalpy \[J/kg\], mass basis
  (DWSIM `Phases(0).Properties.enthalpy`, Vessel.vb:763). Used **only** by
  [`Separator::flash_adiabatic`]; ignored by the isothermal mode. The datum
  only needs to be consistent with the caller's PH-flash closure.

```rust
pub struct SeparatorFeed {
    pub molar_flow: MolarFlowRate,
    pub composition: Vec<uom::si::f64::Ratio>,
    pub specific_enthalpy: uom::si::f64::AvailableEnergy,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `molar_flow` | `MolarFlowRate` | Total feed molar flow `F` \[katal = mol/s\], `>= 0`. |
| `composition` | `Vec<uom::si::f64::Ratio>` | Overall feed composition as mole fractions `z_i` \[-\]. |
| `specific_enthalpy` | `uom::si::f64::AvailableEnergy` | Feed specific enthalpy \[J/kg\], mass basis (adiabatic mode only). |

##### Implementations

###### Methods

- ```rust
  pub fn from_si(molar_flow: f64, composition: &[f64], specific_enthalpy: f64) -> Self { /* ... */ }
  ```
  Convenience constructor from SI scalars: `molar_flow` \[mol/s\], a slice

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SeparatorFeed { /* ... */ }
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
    fn eq(self: &Self, other: &SeparatorFeed) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SeparatorMode`

The operating specification of the separator — DWSIM's `CalculationModes`
(Vessel.vb:99-106), reduced to the two the material-balance flash needs.

Modeled as an enum (no `dyn` dispatch), per the workspace design rules. It is
carried on [`SeparatorResult::mode`] so a caller can report which
specification produced a result. The adiabatic mode's caller-dependent
PH-flash step is **not** stored here (a closure cannot be an enum field
without `dyn`); it is passed to [`Separator::flash_adiabatic`] instead.

```rust
pub enum SeparatorMode {
    IsothermalTp,
    Adiabatic,
}
```

##### Variants

###### `IsothermalTp`

Isothermal-isobaric: flash at fixed vessel temperature and pressure
(DWSIM `Legacy` with `OverrideT`/`OverrideP`, Vessel.vb:820-844).

###### `Adiabatic`

Adiabatic: fixed vessel pressure, temperature found by a PH flash from
the feed enthalpy (DWSIM `Adiabatic`, Vessel.vb:793-818).

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
    fn clone(self: &Self) -> SeparatorMode { /* ... */ }
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
    fn eq(self: &Self, other: &SeparatorMode) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PhaseOutlet`

One equilibrium-phase outlet stream produced by the separator.

The intensive state (`T`, `P`) is shared with the sibling outlet and carried
on [`SeparatorResult`]; this struct holds the phase's *extensive* flows and
its composition. Units (SI, `uom`-typed).

```rust
pub struct PhaseOutlet {
    pub molar_flow: MolarFlowRate,
    pub mass_flow: uom::si::f64::MassRate,
    pub mole_fractions: Vec<uom::si::f64::Ratio>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `molar_flow` | `MolarFlowRate` | This phase's molar flow \[katal = mol/s\]: `β·F` (vapour) or `(1−β)·F`<br>(liquid). |
| `mass_flow` | `uom::si::f64::MassRate` | This phase's mass flow \[kg/s\]: molar flow times the phase mixture molar<br>mass `Σ_i c_i M_i` (with `c_i` the phase mole fractions). |
| `mole_fractions` | `Vec<uom::si::f64::Ratio>` | This phase's composition as mole fractions \[-\]: `y_i` (vapour) or `x_i`<br>(liquid). Sums to 1 for a nonzero phase. |

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
    fn clone(self: &Self) -> PhaseOutlet { /* ... */ }
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
    fn eq(self: &Self, other: &PhaseOutlet) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SeparatorResult`

The full separator result: the flash split, the two phase outlets, and the
vessel intensive state they inherit.

Conservation (see the module docs) holds by construction:
`vapour.molar_flow + liquid.molar_flow == feed.molar_flow` and, per
component, `y_i·V + x_i·L == z_i·F`, both on a molar basis; multiplying by
the fixed molar masses gives the same on a mass basis.

```rust
pub struct SeparatorResult {
    pub mode: SeparatorMode,
    pub flash: crate::thermo::flash::FlashResult,
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub pressure: uom::si::f64::Pressure,
    pub vapour: PhaseOutlet,
    pub liquid: PhaseOutlet,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mode` | `SeparatorMode` | The operating specification that produced this result. |
| `flash` | `crate::thermo::flash::FlashResult` | The underlying two-phase flash (`β`, `x`, `y`, `K`, iterations). |
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Vessel temperature \[K\] the outlets inherit — the input `T` for<br>[`SeparatorMode::IsothermalTp`], the PH-flash result for<br>[`SeparatorMode::Adiabatic`]. |
| `pressure` | `uom::si::f64::Pressure` | Vessel pressure \[Pa\] the outlets inherit. |
| `vapour` | `PhaseOutlet` | Vapour outlet (`Phases(2)` → `OutputConnectors(0)`, Vessel.vb:952-971). |
| `liquid` | `PhaseOutlet` | Liquid outlet (liquid phase → `OutputConnectors(1)`, Vessel.vb:986-1045). |

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
    fn clone(self: &Self) -> SeparatorResult { /* ... */ }
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
    fn eq(self: &Self, other: &SeparatorResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SeparatorError`

Errors from the separator flash-and-route.

```rust
pub enum SeparatorError {
    Flash(crate::thermo::flash::FlashError),
    LengthMismatch {
        components: usize,
        composition: usize,
    },
    InvalidMolarFlow(f64),
}
```

##### Variants

###### `Flash`

The flash kernel (or the caller's adiabatic PH closure) failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::thermo::flash::FlashError` |  |

###### `LengthMismatch`

`components` and the feed `composition` were different lengths.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `components` | `usize` | Number of components supplied. |
| `composition` | `usize` | Number of feed mole fractions supplied. |

###### `InvalidMolarFlow`

The feed molar flow was negative or non-finite.

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
    fn clone(self: &Self) -> SeparatorError { /* ... */ }
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
  - ```rust
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private18::Error + ''static> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: FlashError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SeparatorError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Separator`

A two-phase gas-liquid separator (flash drum) bound to a thermodynamic
property package.

Holds the [`PropertyPackageModel`] used for the flash; `Copy` so it can be
passed by value. Construct with [`Separator::new`], then flash a combined
feed with [`Separator::flash_isothermal`] (TP) or [`Separator::flash_adiabatic`]
(PH).

```rust
pub struct Separator {
    pub package: crate::thermo::property_package::PropertyPackageModel,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `package` | `crate::thermo::property_package::PropertyPackageModel` | The property package the flash uses (`Ideal` / `PengRobinson` / `Srk`). |

##### Implementations

###### Methods

- ```rust
  pub fn new(package: PropertyPackageModel) -> Self { /* ... */ }
  ```
  Build a separator that flashes with `package`.

- ```rust
  pub fn flash_isothermal(self: &Self, components: &[Component], feed: &SeparatorFeed, t: ThermodynamicTemperature, p: Pressure) -> Result<SeparatorResult, SeparatorError> { /* ... */ }
  ```
  **Isothermal TP separator** — flash the combined `feed` at the vessel

- ```rust
  pub fn flash_adiabatic<PhFlash>(self: &Self, components: &[Component], feed: &SeparatorFeed, p: Pressure, ph_flash: PhFlash) -> Result<SeparatorResult, SeparatorError>
where
    PhFlash: Fn(&[Component], &[f64], f64, f64) -> Result<(f64, FlashResult), FlashError> { /* ... */ }
  ```
  **Adiabatic separator** — fix the vessel pressure `p`, find the vessel

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Separator { /* ... */ }
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
    fn eq(self: &Self, other: &Separator) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `splitter`

Splitter (stream splitter / tee): a pure mass-balance unit operation that
divides one inlet material stream into N outlet streams.

Pure-Rust port of DWSIM's splitter, from `UnitOperations/Splitter.vb`
(GPL-3.0, `Public Overrides Sub Calculate`, lines 201-395). Upstream
copyright: 2008 Daniel Wagner O. de Medeiros.

A splitter routes the inlet's flow to several outlets **without changing the
intensive state**. Every outlet inherits the inlet's temperature, pressure,
specific enthalpy, and composition unchanged (Splitter.vb:258-266 copies
`temperature`, `pressure`, `enthalpy`, and each compound's mole/mass fraction
straight onto every outlet); only the *extensive* flow is divided
(Splitter.vb:268 `massflow = W * Ratios(i)`). DWSIM itself describes it as
"a mass balance unit operation — splits a material stream into two or three
other streams with different overall flow rates but with the same
composition" (Splitter.vb:209-210).

# Split-specification modes (ported as the [`SplitSpec`] enum)

DWSIM's `OpMode` enum (Splitter.vb:38-42) offers three specification modes;
this port mirrors them as one enum (no `dyn` dispatch, per the workspace
design rules):

- [`SplitSpec::Fractions`] — DWSIM `OpMode.SplitRatios` (Splitter.vb:239-276):
  each outlet `i` gets a fraction `f_i` of the inlet flow, `Σ f_i = 1`,
  `w_i = f_i · w_in` (Splitter.vb:268).
- [`SplitSpec::MassFlows`] — DWSIM `OpMode.StreamMassFlowSpec`
  (Splitter.vb:278-332): fixed **mass** flows to the leading outlets, the
  remainder to the last (Splitter.vb:293 `wn(1) = W - w1`, :303
  `wn(2) = W - w1 - w2`).
- [`SplitSpec::MoleFlows`] — DWSIM `OpMode.StreamMoleFlowSpec`
  (Splitter.vb:334-389): the same, on a **mole** flow basis
  (Splitter.vb:349 `mn(1) = M - m1`, :359 `mn(2) = M - m1 - m2`).

# Flash boundary — none needed for the intensive state

Unlike [`crate::mixer`] and [`crate::expander`], a splitter needs **no**
property-package / flash call to fix its outlet intensive state: because the
outlets share the inlet's temperature, pressure, specific enthalpy, and
composition byte-for-byte, DWSIM copies the *already-solved* inlet state
directly onto each outlet (Splitter.vb:258-260 sets outlet `temperature`,
`pressure`, `enthalpy` equal to the inlet's) rather than re-flashing. DWSIM
still tags each outlet `SpecType = Pressure_and_Enthalpy` (Splitter.vb:272)
so the flowsheet solver stays consistent, but the outlet temperature is
already known from the inlet. This port therefore carries the inlet
[`IntensiveState`] through to every [`OutletStream`] verbatim and requires no
caller-supplied flash closure. (A caller who *changes* an outlet's pressure
downstream of the tee would flash there, but that is outside the splitter.)

# Excluded DWSIM behavior

Deliberately **not** ported (GUI / solver / persistence plumbing, no physics):
the `EditingForm_Splitter` editor and icon/bitmap resources
(Splitter.vb:32, :559-595), XML/JSON serialization and I/O
(`CloneXML`/`CloneJSON`/`SaveData`/`LoadData`, :73-126), the flowsheet
`Inspector` trace paragraphs (:203-209), `GetCalculationModes` /
`SetCalculationMode` string plumbing (:53-71), `RunDynamicModel`
(:148-199) and `DeCalculate` outlet clearing (:397-426), and the
property-grid reflection accessors
(`GetPropertyValue`/`GetProperties`/`SetPropertyValue`/`GetPropertyUnit`,
:428-557). DWSIM's two/three-outlet GUI cap is dropped — this port accepts
any number of outlets `N >= 1`.

```rust
pub mod splitter { /* ... */ }
```

### Types

#### Type Alias `MolarFlowRate`

Molar (mole) flow rate — amount of substance per unit time.

`uom` 0.38 has no dedicated `MolarFlowRate` quantity, but a molar flow is
dimensionally `T⁻¹N` (mol · s⁻¹), which is exactly the SI unit **katal**
(`CatalyticActivity`). This alias gives the splitter's public API a
human-readable name for that quantity (per the workspace "named type alias"
rule) while reusing the correct `uom` dimension. Base unit: **katal =
mol/s** (`uom::si::catalytic_activity::katal`).

```rust
pub type MolarFlowRate = uom::si::f64::CatalyticActivity;
```

#### Struct `IntensiveState`

The intensive (flow-independent, per-unit-mass) thermodynamic state that a
splitter passes unchanged from its inlet to every outlet.

A splitter alters only extensive flows, never this state — so the same
`IntensiveState` is shared by the inlet and all outlets
(Splitter.vb:258-266). Units (all SI, `uom`-typed):
- `temperature` — `T` \[K\], `> 0`. Copied verbatim from the inlet
  (Splitter.vb:258).
- `pressure` — `p` \[Pa\], `> 0` (Splitter.vb:259).
- `specific_enthalpy` — `h` \[J/kg\], mass basis, matching DWSIM's
  `Phases(0).Properties.enthalpy` (Splitter.vb:260). Any real value; only the
  datum must be consistent with the caller's convention.
- `mole_fractions` — overall composition as mole fractions `y_i`
  (dimensionless \[0, 1\], summing to 1), one entry per compound
  (Splitter.vb:263-266). Empty is allowed for a composition-agnostic
  flow-only split.

```rust
pub struct IntensiveState {
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub pressure: uom::si::f64::Pressure,
    pub specific_enthalpy: uom::si::f64::AvailableEnergy,
    pub mole_fractions: Vec<uom::si::f64::Ratio>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature `T` \[K\], `> 0`. |
| `pressure` | `uom::si::f64::Pressure` | Pressure `p` \[Pa\], `> 0`. |
| `specific_enthalpy` | `uom::si::f64::AvailableEnergy` | Specific enthalpy `h` \[J/kg\], mass basis. |
| `mole_fractions` | `Vec<uom::si::f64::Ratio>` | Overall composition as mole fractions `y_i` (dimensionless \[0, 1\]). |

##### Implementations

###### Methods

- ```rust
  pub fn from_si(temperature: f64, pressure: f64, specific_enthalpy: f64, mole_fractions: &[f64]) -> Self { /* ... */ }
  ```
  Convenience constructor from SI scalars: `temperature` \[K\],

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> IntensiveState { /* ... */ }
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
    fn eq(self: &Self, other: &IntensiveState) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SplitSpec`

How the inlet flow is divided among the outlets — DWSIM's `Splitter.OpMode`
enum (Splitter.vb:38-42). Modeled as an enum (no `dyn` dispatch), per the
workspace design rules. The specification list is owned **by value** (a
`Vec`, indexed by `usize`), never by reference — no lifetimes.

```rust
pub enum SplitSpec {
    Fractions(Vec<uom::si::f64::Ratio>),
    MassFlows(Vec<uom::si::f64::MassRate>),
    MoleFlows(Vec<MolarFlowRate>),
}
```

##### Variants

###### `Fractions`

`OpMode.SplitRatios` (Splitter.vb:239-276). Each outlet `i` receives a
fraction `f_i` of the inlet flow; the vector has one entry per outlet and
**must** satisfy `Σ f_i = 1` (validated to within
[`FRACTION_SUM_TOLERANCE`]) with every `f_i >= 0`.

Divergence from DWSIM: DWSIM does not validate the sum — it *overwrites*
the last ratio with `1 − Σ(others)` (Splitter.vb:247-250), silently
"fixing" an inconsistent input. This port instead treats the fractions as
caller-authoritative and errors on a non-unit sum (workspace honesty
rule: do not silently repair a mis-specification).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<uom::si::f64::Ratio>` |  |

###### `MassFlows`

`OpMode.StreamMassFlowSpec` (Splitter.vb:278-332). Fixed **mass** flows
`w_1 … w_{N-1}` to the leading `N−1` outlets; the last outlet gets the
remainder `w_in − Σ w_k` (Splitter.vb:293, :303). The vector holds the
`N−1` fixed flows (\[kg/s\], each `>= 0`); an empty vector yields a single
pass-through outlet carrying the whole inlet flow.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<uom::si::f64::MassRate>` |  |

###### `MoleFlows`

`OpMode.StreamMoleFlowSpec` (Splitter.vb:334-389). As [`Self::MassFlows`]
but on a **mole** flow basis: fixed mole flows `m_1 … m_{N-1}` to the
leading outlets, remainder `m_in − Σ m_k` to the last (Splitter.vb:349,
:359). Units: katal (mol/s), each `>= 0`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<MolarFlowRate>` |  |

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
    fn clone(self: &Self) -> SplitSpec { /* ... */ }
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
    fn eq(self: &Self, other: &SplitSpec) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SplitError`

Errors from resolving a [`SplitSpec`].

These correspond to DWSIM's `Throw New Exception(...)` guards
(Splitter.vb:295, :305, :351, :361 for over-drawn flow specs) plus this
port's stricter fraction-sum check (see [`SplitSpec::Fractions`]).

```rust
pub enum SplitError {
    NegativeFraction {
        index: usize,
        value: f64,
    },
    FractionSumNotUnity {
        sum: f64,
        tolerance: f64,
    },
    NegativeFixedFlow {
        index: usize,
        value: f64,
    },
    InsufficientInletFlow {
        inlet: f64,
        fixed_total: f64,
    },
    NonPositiveInletFlow {
        value: f64,
    },
    NoOutlets,
}
```

##### Variants

###### `NegativeFraction`

A [`SplitSpec::Fractions`] entry was negative. `index` is its position;
`value` is the offending fraction.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | Position of the offending fraction in the vector. |
| `value` | `f64` | The negative fraction value (dimensionless). |

###### `FractionSumNotUnity`

The [`SplitSpec::Fractions`] entries do not sum to 1 within
[`FRACTION_SUM_TOLERANCE`]. `sum` is `Σ f_i`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `sum` | `f64` | The actual `Σ f_i` (dimensionless). |
| `tolerance` | `f64` | The tolerance applied ([`FRACTION_SUM_TOLERANCE`]). |

###### `NegativeFixedFlow`

A fixed flow in [`SplitSpec::MassFlows`] / [`SplitSpec::MoleFlows`] was
negative. `index` is its position; `value` is the offending flow (SI:
kg/s for mass, katal for mole).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | Position of the offending fixed flow. |
| `value` | `f64` | The negative flow value in SI units (kg/s or katal). |

###### `InsufficientInletFlow`

The fixed flows exceed the inlet flow, so the remainder to the last
outlet would be negative — DWSIM `Throw New Exception` when
`W < Σ spec` (Splitter.vb:290-296, :298-305, :346-351, :354-361).
`inlet` and `fixed_total` are in SI units (kg/s or katal).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `inlet` | `f64` | Inlet flow in SI units (kg/s or katal). |
| `fixed_total` | `f64` | Sum of the fixed flows in SI units (kg/s or katal). |

###### `NonPositiveInletFlow`

A flow-spec mode needs a strictly positive inlet flow to convert flows to
fractions, but the inlet flow was `<= 0`. `value` is the inlet flow in SI
units (kg/s or katal).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `value` | `f64` | The non-positive inlet flow in SI units (kg/s or katal). |

###### `NoOutlets`

A [`SplitSpec::Fractions`] had no entries — a splitter needs `>= 1`
outlet.

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
    fn clone(self: &Self) -> SplitError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SplitError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SplitResult`

The resolved per-outlet split produced by [`split`].

All three vectors have the same length `N` (the outlet count) and are ordered
by outlet index. Because the intensive state is uniform, mass and mole flow
are both simply the inlet flow scaled by the *same* fraction, so all three
stay mutually consistent.

```rust
pub struct SplitResult {
    pub fractions: Vec<uom::si::f64::Ratio>,
    pub mass_flows: Vec<uom::si::f64::MassRate>,
    pub mole_flows: Vec<MolarFlowRate>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fractions` | `Vec<uom::si::f64::Ratio>` | Per-outlet split fractions `f_i` (dimensionless \[0, 1\]), `Σ f_i = 1`. |
| `mass_flows` | `Vec<uom::si::f64::MassRate>` | Per-outlet mass flows `w_i = f_i · w_in` \[kg/s\] (Splitter.vb:268). |
| `mole_flows` | `Vec<MolarFlowRate>` | Per-outlet mole flows `m_i = f_i · m_in` \[katal = mol/s\]<br>(Splitter.vb:381). |

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
    fn clone(self: &Self) -> SplitResult { /* ... */ }
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
    fn eq(self: &Self, other: &SplitResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `OutletStream`

One outlet material stream produced by [`split_streams`]: the inlet's
[`IntensiveState`] carried through unchanged, tagged with this outlet's flow.

The `state` field is identical (`==`) to the splitter inlet's intensive state
— that is the defining property of a splitter (Splitter.vb:258-266).

```rust
pub struct OutletStream {
    pub state: IntensiveState,
    pub split_fraction: uom::si::f64::Ratio,
    pub mass_flow: uom::si::f64::MassRate,
    pub mole_flow: MolarFlowRate,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `state` | `IntensiveState` | Intensive state, identical to the inlet's (T, p, h, composition). |
| `split_fraction` | `uom::si::f64::Ratio` | This outlet's split fraction `f_i` (dimensionless \[0, 1\]). |
| `mass_flow` | `uom::si::f64::MassRate` | This outlet's mass flow `w_i = f_i · w_in` \[kg/s\]. |
| `mole_flow` | `MolarFlowRate` | This outlet's mole flow `m_i = f_i · m_in` \[katal = mol/s\]. |

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
    fn clone(self: &Self) -> OutletStream { /* ... */ }
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
    fn eq(self: &Self, other: &OutletStream) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `resolve_fractions`

Resolve a [`SplitSpec`] into per-outlet split fractions `f_i` (dimensionless,
`Σ f_i = 1`).

This is the core of the DWSIM `Calculate` routine (Splitter.vb:237-391)
reduced to its dimensionless essence: whatever the mode, the outcome is a set
of fractions of the inlet flow. The flow-spec modes need the inlet flow (mass
or mole, in SI units) to convert fixed flows to fractions; [`SplitSpec::Fractions`]
ignores it.

- `inlet_mass_flow_si` — inlet mass flow \[kg/s\], used only by
  [`SplitSpec::MassFlows`].
- `inlet_mole_flow_si` — inlet mole flow \[katal = mol/s\], used only by
  [`SplitSpec::MoleFlows`].

# Errors
See [`SplitError`] — negative/over-unity fractions, non-unit fraction sum,
negative fixed flows, fixed flows exceeding the inlet, non-positive inlet
flow for a flow-spec mode, or an empty fraction spec.

```rust
pub fn resolve_fractions(spec: &SplitSpec, inlet_mass_flow_si: f64, inlet_mole_flow_si: f64) -> Result<Vec<uom::si::f64::Ratio>, SplitError> { /* ... */ }
```

#### Function `split`

Resolve a [`SplitSpec`] into per-outlet flows, given the inlet mass and mole
flows (DWSIM `Calculate`, Splitter.vb:237-391).

Computes the fractions via [`resolve_fractions`], then scales **both** the
inlet mass flow and inlet mole flow by each fraction:
`w_i = f_i · w_in` (Splitter.vb:268) and `m_i = f_i · m_in`
(Splitter.vb:381). This is exact because the splitter leaves the intensive
state (hence the mixture molar mass) uniform across all outlets, so mass and
mole flow scale by the identical fraction.

- `inlet_mass_flow` — inlet mass flow `w_in` \[kg/s\].
- `inlet_mole_flow` — inlet mole flow `m_in` \[katal = mol/s\].

# Errors
Propagates every [`SplitError`] from [`resolve_fractions`].

```rust
pub fn split(spec: &SplitSpec, inlet_mass_flow: uom::si::f64::MassRate, inlet_mole_flow: MolarFlowRate) -> Result<SplitResult, SplitError> { /* ... */ }
```

#### Function `split_streams`

Build the full set of [`OutletStream`]s: every outlet carries the inlet's
[`IntensiveState`] unchanged, tagged with its own split flows
(DWSIM `Calculate`, Splitter.vb:254-276 and the analogous flow-spec loops).

This is the complete splitter: the intensive state (`inlet_state`) is cloned
verbatim onto each outlet — no flash is performed or needed (see the
module-level "Flash boundary" note) — while the flows come from [`split`].

- `inlet_state` — the inlet's intensive state (T, p, h, composition).
- `inlet_mass_flow` — inlet mass flow `w_in` \[kg/s\].
- `inlet_mole_flow` — inlet mole flow `m_in` \[katal = mol/s\].

# Errors
Propagates every [`SplitError`] from [`split`].

```rust
pub fn split_streams(spec: &SplitSpec, inlet_state: &IntensiveState, inlet_mass_flow: uom::si::f64::MassRate, inlet_mole_flow: MolarFlowRate) -> Result<Vec<OutletStream>, SplitError> { /* ... */ }
```

### Constants and Statics

#### Constant `FRACTION_SUM_TOLERANCE`

Tolerance on the split-fraction sum and on flow-remainder non-negativity.

[`SplitSpec::Fractions`] requires `|Σ f_i − 1| <= FRACTION_SUM_TOLERANCE`
(dimensionless), and the flow-spec modes reject a remainder more negative
than `−FRACTION_SUM_TOLERANCE · w_in` (i.e. the fixed flows over-drawing the
inlet). `1e-9` comfortably absorbs `f64` round-off in a sum of order-1
fractions while still catching genuine mis-specifications.

```rust
pub const FRACTION_SUM_TOLERANCE: f64 = 1.0e-9;
```

## Module `thermo`

# Thermodynamics kernel (DWSIM Tier-1 port)

The core thermodynamic kernel translated from DWSIM's `DWSIM.Thermodynamics`
(GPL-3.0): the compound data model, cubic equations of state, liquid-phase
activity-coefficient models, and the vapour-liquid-equilibrium flash. These
supply the fugacity coefficients, K-values, and enthalpy/entropy departures
that every equipment model ultimately needs.

> **⚠️ Unverified until validated.** Early-stage translation, no human V&V.
> Not for nuclear facility operation, reactor control, safety-critical, or
> licensing decisions. Independent OUTRAM PARK fork, not the official DWSIM.

## Modules

### Data substrate

- [`component`] — the pure-compound constant-property data model
  ([`Component`]): critical properties, acentric factor, molar mass,
  ideal-gas heat-capacity coefficients. The shared substrate every other
  thermo module consumes. **Data substrate (this file's author).**

### Equations of state

- [`cubic_eos`] — Peng-Robinson and SRK cubic EOS: compressibility solve,
  fugacity coefficients, enthalpy/entropy departures, van der Waals mixing.
- [`eos_variants`] — cubic-EOS refinements: the PRSV α-function and the
  Peneloux volume translation, composed on top of [`cubic_eos`].
- [`pr1978`] — Peng-Robinson 1978 (PR78), the ω-dependent α-slope refit.
- [`prsv2_full`] — full Peng-Robinson-Stryjek-Vera 2 (PRSV2): the
  three-parameter (κ₁, κ₂, κ₃) α-function with a complete Z / fugacity /
  departure / vapour-pressure surface.
- [`lkp`] — Lee-Kesler-Plöcker (LKP) three-parameter corresponding-states EOS.
- [`pr_lee_kesler`] — Peng-Robinson + Lee-Kesler enthalpy/entropy hybrid
  property package (PR fugacities, LKP caloric departures).

### Activity-coefficient / group-contribution models

- [`activity`] — NRTL / UNIQUAC / Ideal (Raoult) liquid-phase activity
  coefficients.
- [`unifac`] — UNIFAC group-contribution activity coefficients.
- [`unifac_dortmund`] — modified UNIFAC (Dortmund) group-contribution
  activity coefficients (Weidlich & Gmehling temperature-dependent
  interaction + modified combinatorial term).
- [`unifac_lle`] — UNIFAC with the liquid-liquid-equilibrium (LLE)
  parameterised group-interaction table (same functional form, LLE `a_mn`).
- [`electrolyte`] — aqueous-ionic (electrolyte) activity-coefficient tier.
- [`ideal_props`] — ideal-gas heat capacity / enthalpy / entropy from the
  [`Component`] Cp0 coefficients (the departure reference state).
- [`transport`] — transport-property correlations (viscosity, thermal
  conductivity, surface tension) and their phase-mixing rules.

### Flash algorithms

- [`flash`] — isothermal-isobaric (TP) vapour-liquid-equilibrium flash via
  the Rachford-Rice / Nested-Loops method, with Wilson K-value initialisation.
- [`flash_insideout`] — Boston-Britt Inside-Out two-phase (VLE) PT flash.
- [`flash_insideout_3p`] — Boston-Fournier Inside-Out three-phase (VLLE)
  PT flash.
- [`flash_vlle`] — three-phase vapour-liquid-liquid equilibrium (VLLE)
  nested-loops PT flash.
- [`flash_lle`] — simple liquid-liquid equilibrium (LLE) isothermal split.
- [`flash_sle`] — solid-liquid equilibrium (SLE) flash (ideal-solubility
  saturation with heat-of-fusion temperature dependence).
- [`flash_svlle`] — solid + three-phase (SVLLE) flash: precipitation coupled
  to the VLLE split.
- [`flash_single_comp`] — single-component (pure-fluid) saturation-shortcut
  flash.
- [`energy_flash`] — isenthalpic (PH) / energy flash: solve the temperature at
  which a mixture's total molar enthalpy meets a target `H` at fixed `P`.
- [`saturation`] — bubble-point / dew-point temperature & pressure of a
  multicomponent mixture, on top of the isothermal-isobaric VLE kernel.
- [`stability`] — phase-stability analysis via Michelsen's tangent-plane
  distance (TPD) criterion (single-/two-phase identification, flash init).

### Gibbs-minimisation & reacting equilibria

- [`gibbs`] — Gibbs-energy-minimisation speciation flash (single gas phase,
  reacting mixture) under element / atom mass-balance constraints.
- [`gibbs_multiphase`] — multi-phase (N-phase) Gibbs-energy-minimisation
  flash: species distribution across several coexisting solution phases.
- [`electrolyte_svle`] — electrolyte SVLE flash: weak-electrolyte
  reaction-set speciation coupled to solid (Ksp) precipitation.
- [`sour_water`] — sour-water aqueous ionic-equilibrium speciation
  (H₂S / NH₃ / CO₂ / H₂O), built on the [`electrolyte_svle`] conventions.

### Property-package glue

- [`property_package`] — glue that composes the cubic-EOS / ideal models into
  K-values and drives an EOS-consistent PT two-phase flash
  ([`property_package::PropertyPackageModel`], enum dispatch, no `dyn`).

## Design (crate `CLAUDE.md`)

Enum dispatch (no `dyn`) for the EOS / activity / flash model choices; `uom`
at public boundaries where practical, documented raw `f64` (SI) in the inner
EOS/flash arithmetic loops where `uom` overhead would fight the math (the
DWSIM-internal SI convention: Pa, K, J/mol, kg/m³).

## Honest scope

This is a substantial slice of DWSIM's thermodynamics, though still not the
whole of it. What was once flagged as "future work" is now ported:

- **Advanced EOS.** PR78 ([`pr1978`]), the full PRSV2 κ₁/κ₂/κ₃ α-function
  ([`prsv2_full`]), Lee-Kesler-Plöcker ([`lkp`]), and the PR + Lee-Kesler
  caloric hybrid ([`pr_lee_kesler`]) — on top of the earlier one-parameter
  PRSV α-function and Peneloux volume translation ([`eos_variants`]).
- **Inside-Out flashes.** Boston-Britt two-phase ([`flash_insideout`]) and
  Boston-Fournier three-phase ([`flash_insideout_3p`]).
- **Multi-phase / solid / electrolyte equilibria.** Three-phase VLLE
  ([`flash_vlle`]), LLE ([`flash_lle`]), SLE ([`flash_sle`]), SVLLE
  ([`flash_svlle`]), single-component shortcut ([`flash_single_comp`]),
  single- and multi-phase Gibbs minimisation ([`gibbs`], [`gibbs_multiphase`]),
  the electrolyte activity tier ([`electrolyte`]), the electrolyte SVLE
  speciation + Ksp solver ([`electrolyte_svle`]), and the sour-water package
  ([`sour_water`]).
- **Group-contribution activity.** Modified UNIFAC (Dortmund)
  ([`unifac_dortmund`]) and UNIFAC-LLE ([`unifac_lle`]).

**Still out of scope / future work:** the Mathias-Copeman and Twu α-variants,
seawater and black-oil property packages, and the CoolProp/steam-table
external-property bridges. Everything here is **verified, not
benchmark-validated** — see `docs/port-scope.md` and epic `op-qo2` for the
remaining backlog.

```rust
pub mod thermo { /* ... */ }
```

### Modules

## Module `activity`

NRTL / UNIQUAC / Ideal liquid-phase activity-coefficient models.

Ported from DWSIM (GPL-3.0). The activity-coefficient math lives in DWSIM's
auxiliary model classes, not the property-package wrappers:

- **NRTL** — `DWSIM.Thermodynamics/PropertyPackages/Models/NRTL.vb`,
  `NRTL.GAMMA_MR` (NRTL.vb:222-417). The package wrapper
  `PropertyPackages/NRTL.vb` only wires this up and estimates missing
  parameters via UNIFAC (not ported — see *Excluded behaviour*).
- **UNIQUAC** — `DWSIM.Thermodynamics/PropertyPackages/Models/UNIQUAC.vb`,
  `UNIQUAC.GAMMA_MR` (UNIQUAC.vb:245-448). Wrapper `PropertyPackages/UNIQUAC.vb`.
- **Ideal** — Raoult's law, `gamma_i = 1`; DWSIM's `RaoultPropertyPackage`
  returns unit activity coefficients for the liquid phase. Trivial, but a
  member of the model enum so the caller has one uniform dispatch surface.

# What this computes

Each model returns the vector of **liquid-phase activity coefficients**
`gamma_i` (dimensionless, `> 0`) at a given liquid mole-fraction vector `x`
(dimensionless, should sum to 1) and temperature `T` (kelvin). `gamma_i`
corrects Raoult's law for non-ideal mixing: `f_i = x_i * gamma_i * f_i^pure`.

# Gas constant and parameter units (DWSIM convention — read carefully)

DWSIM's NRTL/UNIQUAC kernels divide the interaction *energies* by
`R = 1.98721 cal/(mol.K)` (see [`R_CAL`]) — i.e. the interaction parameters
`a_ij` carried through this module are in **cal/mol**, exactly as stored in
DWSIM's `nrtl.dat` / `uniquac.dat` and its `NRTL_IPData` / `UNIQUAC_IPData`
records (NRTL.vb:321-322, UNIQUAC.vb:394-395). This module reproduces that
literally so results match DWSIM. The SI gas constant
`R = 8.31446261815324 J/(mol.K)` ([`R_GAS`], = `R_CAL * 4.184`) is provided
for callers that work in J/mol and is **not** used inside these two kernels.

# Design (workspace + crate `CLAUDE.md`)

Enum dispatch ([`ActivityModel`]), no `dyn`, no lifetimes, no channels.
Binary interaction parameters are **owned by value** as dense `Vec<Vec<f64>>`
matrices ([`NrtlParams`], [`UniquacParams`]); components are indexed by
`usize`. Raw `f64` (SI/DWSIM base units) in the inner arithmetic loops per
the crate unit policy; every public item documents its quantity/range/units.

# Relationship to [`crate::thermo::Component`]

[`Component`](crate::thermo::Component) carries critical constants and Cp0
coefficients but **not** the UNIQUAC van-der-Waals volume/surface parameters
`r_i` / `q_i`. Those are therefore taken as explicit inputs to
[`UniquacParams`]; a caller pairs each `Component` with its `(r_i, q_i)` from
a group-contribution table (e.g. UNIFAC `R_k`/`Q_k` sums). NRTL needs no
pure-component structural parameters at all.

# Excluded DWSIM behaviour (honest scope)

This is a **verification-grade port of the `GAMMA_MR` activity-coefficient
routine only**, not the whole property package. Deliberately *not* ported:

- **Automatic parameter estimation.** DWSIM's `EstimateMissingInteraction`
  `Parameters` (NRTL.vb:133-320, UNIQUAC.vb:156-339) regresses missing binary
  parameters against Modified-UNIFAC using an IPOPT optimiser. Here **all**
  parameters are caller-supplied; there is no fallback estimation.
- **The bundled databases** (`nrtl.dat`, `uniquac.dat`, ChemSep IP data). The
  whole ID-lookup / symmetric-fill machinery (NRTL.vb:319-359,
  UNIQUAC.vb:352-385) is replaced by dense caller-supplied matrices.
- **Excess enthalpy / heat capacity** (`HEX_MIX`, `CPEX_MIX`, `DLNGAMMA_DT`;
  NRTL.vb:419-465, UNIQUAC.vb:450-496) — the `d ln gamma / dT` finite
  differences and their `H^E`/`Cp^E` derivatives are **not** ported.
- **Temperature-dependent NRTL non-randomness.** DWSIM's `NRTL_IPData` stores
  a single constant `alpha12`; there is no `T`-dependent `alpha` in the source
  to port, and none is added here (`alpha_ij` is a plain constant matrix).

The `B`/`C` temperature coefficients of the interaction energy
(`tau ~ (A + B*T + C*T^2)/(R*T)`) **are** ported, defaulting to zero.

> **⚠️ Verification, not validation.** The tests below verify that this code
> reproduces the DWSIM closed-form expressions and their known analytical
> limits (pure component, ideal mixture, infinite dilution, Gibbs-Duhem).
> They do **not** validate any parameter set against experimental VLE data,
> and nothing here is cleared for nuclear / safety-critical use.

```rust
pub mod activity { /* ... */ }
```

### Types

#### Enum `ActivityModel`

Liquid-phase activity-coefficient model (enum dispatch, no `dyn`).

Given a liquid composition and temperature, [`Self::activity_coefficients`]
returns the vector of `gamma_i`. Variants:

- [`ActivityModel::Ideal`] — Raoult's law, every `gamma_i = 1`.
- [`ActivityModel::Nrtl`] — Non-Random Two-Liquid model ([`NrtlParams`]).
- [`ActivityModel::Uniquac`] — UNIQUAC model ([`UniquacParams`]).

```rust
pub enum ActivityModel {
    Ideal,
    Nrtl(NrtlParams),
    Uniquac(UniquacParams),
}
```

##### Variants

###### `Ideal`

Ideal solution (Raoult's law): `gamma_i = 1` for every component,
independent of composition and temperature.

###### `Nrtl`

NRTL model with its binary interaction parameters.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NrtlParams` |  |

###### `Uniquac`

UNIQUAC model with its pure-component structural parameters and binary
interaction energies.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `UniquacParams` |  |

##### Implementations

###### Methods

- ```rust
  pub fn activity_coefficients(self: &Self, x: &[f64], t: f64) -> Vec<f64> { /* ... */ }
  ```
  Liquid-phase activity coefficients `gamma_i` (dimensionless, `> 0`) at

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ActivityModel { /* ... */ }
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
    fn eq(self: &Self, other: &ActivityModel) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `NrtlParams`

Binary interaction parameters for the **NRTL** model.

All four matrices are dense and `n x n` (`n` = component count). Following
DWSIM (`NRTL.GAMMA_MR`, NRTL.vb:321-323) the reduced interaction parameter is

```text
tau_ij = (a_ij + b_ij * T + c_ij * T^2) / (R_CAL * T)
G_ij   = exp(-alpha_ij * tau_ij)
```

with `T` in kelvin and [`R_CAL`] in cal/(mol.K). Sign / index convention
(matching DWSIM's `NRTL_IPData`): `a[i][j]` is the energy of the `i-j`
interaction felt by molecule `i` (`a_ij != a_ji` in general); the diagonal
`a[i][i]` should be `0` (so `tau_ii = 0`, `G_ii = 1`).

Units: `a` in **cal/mol**, `b` in **cal/(mol.K)**, `c` in **cal/(mol.K^2)**,
`alpha` dimensionless. `alpha` is physically symmetric (`alpha_ij = alpha_ji`,
diagonal irrelevant since `tau_ii = 0`); DWSIM stores a single `alpha12` per
pair. Typical `alpha` is `0.2`-`0.47`.

```rust
pub struct NrtlParams {
    pub a: Vec<Vec<f64>>,
    pub b: Vec<Vec<f64>>,
    pub c: Vec<Vec<f64>>,
    pub alpha: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `Vec<Vec<f64>>` | Constant part of the interaction energy `a_ij` [cal/mol], `n x n`. |
| `b` | `Vec<Vec<f64>>` | Linear-in-`T` coefficient `b_ij` [cal/(mol.K)], `n x n`. Zero for the<br>common temperature-independent-energy form. |
| `c` | `Vec<Vec<f64>>` | Quadratic-in-`T` coefficient `c_ij` [cal/(mol.K^2)], `n x n`. Usually zero. |
| `alpha` | `Vec<Vec<f64>>` | Non-randomness factor `alpha_ij` [-], `n x n`, symmetric. |

##### Implementations

###### Methods

- ```rust
  pub fn new(a: Vec<Vec<f64>>, b: Vec<Vec<f64>>, c: Vec<Vec<f64>>, alpha: Vec<Vec<f64>>) -> Self { /* ... */ }
  ```
  Full NRTL parameter set with temperature-dependent energies.

- ```rust
  pub fn from_a_alpha(a: Vec<Vec<f64>>, alpha: Vec<Vec<f64>>) -> Self { /* ... */ }
  ```
  Convenience constructor for the common temperature-independent-energy

- ```rust
  pub fn n(self: &Self) -> usize { /* ... */ }
  ```
  Number of components `n`.

- ```rust
  pub fn activity_coefficients(self: &Self, x: &[f64], t: f64) -> Vec<f64> { /* ... */ }
  ```
  NRTL activity coefficients `gamma_i` at composition `x` and `T` [K].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> NrtlParams { /* ... */ }
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
    fn eq(self: &Self, other: &NrtlParams) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `UniquacParams`

Parameters for the **UNIQUAC** model: pure-component structural parameters
plus binary interaction energies.

`r` and `q` are the van-der-Waals relative molecular **volume** and
**surface-area** parameters (dimensionless, `> 0`), each length `n`. They are
*not* carried by [`Component`] and must be supplied here (e.g. from a
UNIFAC-group `R_k`/`Q_k` summation). The interaction energies follow DWSIM
(`UNIQUAC.GAMMA_MR`, UNIQUAC.vb:394-395):

```text
tau_ij = exp( (-a_ij + b_ij * T + c_ij * T^2) / (R_CAL * T) )
```

where `a_ij = u_ij - u_jj` is the interaction energy [cal/mol]
(`a_ij != a_ji`; diagonal `a_ii = 0` so `tau_ii = 1`). Note the **leading
minus sign on `a`** in DWSIM's form. Units: `a` cal/mol, `b` cal/(mol.K),
`c` cal/(mol.K^2). The coordination number is fixed at `z = 10` (UNIQUAC.vb:333).

```rust
pub struct UniquacParams {
    pub r: Vec<f64>,
    pub q: Vec<f64>,
    pub a: Vec<Vec<f64>>,
    pub b: Vec<Vec<f64>>,
    pub c: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r` | `Vec<f64>` | Van-der-Waals volume parameters `r_i` [-], length `n`, each `> 0`. |
| `q` | `Vec<f64>` | Van-der-Waals surface-area parameters `q_i` [-], length `n`, each `> 0`. |
| `a` | `Vec<Vec<f64>>` | Interaction energy `a_ij = u_ij - u_jj` [cal/mol], `n x n`. |
| `b` | `Vec<Vec<f64>>` | Linear-in-`T` coefficient `b_ij` [cal/(mol.K)], `n x n`. Usually zero. |
| `c` | `Vec<Vec<f64>>` | Quadratic-in-`T` coefficient `c_ij` [cal/(mol.K^2)], `n x n`. Usually zero. |

##### Implementations

###### Methods

- ```rust
  pub fn new(r: Vec<f64>, q: Vec<f64>, a: Vec<Vec<f64>>, b: Vec<Vec<f64>>, c: Vec<Vec<f64>>) -> Self { /* ... */ }
  ```
  Full UNIQUAC parameter set with temperature-dependent energies.

- ```rust
  pub fn from_energies(r: Vec<f64>, q: Vec<f64>, a: Vec<Vec<f64>>) -> Self { /* ... */ }
  ```
  Convenience constructor for the common temperature-independent-energy

- ```rust
  pub fn n(self: &Self) -> usize { /* ... */ }
  ```
  Number of components `n`.

- ```rust
  pub fn activity_coefficients(self: &Self, x: &[f64], t: f64) -> Vec<f64> { /* ... */ }
  ```
  UNIQUAC activity coefficients `gamma_i` at composition `x` and `T` [K].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> UniquacParams { /* ... */ }
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
    fn eq(self: &Self, other: &UniquacParams) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `component_label`

Bind a compile-time reference to [`Component`] so the module's documented
relationship to the shared pure-component type is checked by the compiler
(UNIQUAC's `r_i`/`q_i` are *not* fields of `Component` and so are taken
explicitly by [`UniquacParams`]). Returns the component's name.

This is a documentation/traceability helper, not part of the model math.

```rust
pub fn component_label(component: &crate::thermo::Component) -> &str { /* ... */ }
```

### Constants and Statics

#### Constant `R_CAL`

DWSIM's gas constant for the NRTL/UNIQUAC `tau` reduction:
`R = 1.98721 cal/(mol.K)` (NRTL.vb:321, UNIQUAC.vb:394). Because DWSIM
divides the interaction energies by this value, the `a`/`b`/`c` parameters
carried by [`NrtlParams`] and [`UniquacParams`] are in **cal/mol**,
**cal/(mol.K)**, and **cal/(mol.K^2)** respectively.

```rust
pub const R_CAL: f64 = 1.98721;
```

#### Constant `R_GAS`

SI molar gas constant `R = 8.31446261815324 J/(mol.K)` (CODATA). Provided for
callers working in J/mol; note `R_GAS = R_CAL * 4.184`. **Not** used inside
the NRTL/UNIQUAC kernels, which follow DWSIM in using [`R_CAL`] (cal units).

```rust
pub const R_GAS: f64 = 8.314_462_618_153_24;
```

## Module `component`

Pure-compound **constant-property data model** — the substrate every thermo
module consumes.

Mirrors the fields of DWSIM's `ICompoundConstantProperties`
(`DWSIM.Interfaces/ICompoundConstantProperties.vb`) needed by the cubic EOS,
activity models, and flash: molar mass, the critical point, the acentric
factor, the normal boiling point, and the ideal-gas heat-capacity
coefficients (the departure-function reference state).

## Units (documented raw `f64`, SI — the DWSIM-internal convention)

Stored as plain `f64` in SI base units because these feed the inner EOS/flash
arithmetic loops (per the crate `CLAUDE.md` "raw f64 in inner EOS loops"
rule). Every field spells out its unit below.

| Field | Quantity | Unit |
|---|---|---|
| `molar_mass` | molar mass `M` | kg/mol |
| `critical_temperature` | `Tc` | K |
| `critical_pressure` | `Pc` | Pa |
| `critical_volume` | `Vc` | m³/mol |
| `acentric_factor` | Pitzer acentric factor `ω` | dimensionless |
| `normal_boiling_point` | `Tb` at 1 atm | K |
| `cp_ig_a..e` | ideal-gas Cp correlation coefficients | see [`Component`] |

```rust
pub mod component { /* ... */ }
```

### Modules

## Module `reference`

Reference-compound presets with **public-literature** constant properties.

Critical constants, acentric factors, and molar masses are the standard
tabulated values from Poling, Prausnitz & O'Connell, *The Properties of
Gases and Liquids*, 5th ed. (McGraw-Hill, 2001), Appendix A — an open,
widely-cited reference (workspace `DATA_POLICY`: public literature data
only). Ideal-gas Cp coefficients are left as `0.0` placeholders here (the
`ideal_props` module owns the Cp correlation and its data); callers needing
enthalpy departures should supply real Cp coefficients.

```rust
pub mod reference { /* ... */ }
```

### Functions

#### Function `water`

**Attributes:**

- `MustUse { reason: None }`

Water (H₂O). Tc = 647.14 K, Pc = 22.064 MPa, ω = 0.344, M = 18.015 g/mol.

```rust
pub fn water() -> super::Component { /* ... */ }
```

#### Function `methane`

**Attributes:**

- `MustUse { reason: None }`

Methane (CH₄). Tc = 190.56 K, Pc = 4.599 MPa, ω = 0.011, M = 16.043 g/mol.

```rust
pub fn methane() -> super::Component { /* ... */ }
```

#### Function `ethane`

**Attributes:**

- `MustUse { reason: None }`

Ethane (C₂H₆). Tc = 305.32 K, Pc = 4.872 MPa, ω = 0.099, M = 30.070 g/mol.

```rust
pub fn ethane() -> super::Component { /* ... */ }
```

#### Function `nitrogen`

**Attributes:**

- `MustUse { reason: None }`

Nitrogen (N₂). Tc = 126.20 K, Pc = 3.398 MPa, ω = 0.037, M = 28.014 g/mol.

```rust
pub fn nitrogen() -> super::Component { /* ... */ }
```

#### Function `carbon_dioxide`

**Attributes:**

- `MustUse { reason: None }`

Carbon dioxide (CO₂). Tc = 304.12 K, Pc = 7.374 MPa, ω = 0.225, M = 44.01 g/mol.

```rust
pub fn carbon_dioxide() -> super::Component { /* ... */ }
```

### Types

#### Struct `Component`

Pure-compound constant properties.

A plain data record: no behaviour beyond validated construction and
accessors. The ideal-gas heat capacity is evaluated from `cp_ig_a..e` by
[`crate::thermo::ideal_props`] (this struct only stores the coefficients);
the EOS `a(T)`/`b` parameters are computed by [`crate::thermo::cubic_eos`]
from `critical_temperature`, `critical_pressure`, and `acentric_factor`.

## Ideal-gas Cp correlation

`cp_ig_a..e` are DWSIM's `Ideal_Gas_Heat_Capacity_Const_A..E`. The exact
polynomial/DIPPR form they parameterise is implemented by
[`crate::thermo::ideal_props`] against DWSIM's `PropertyPackageMethods`; this
record is agnostic to that form and merely carries the five coefficients
plus the reference entropy of formation.

```rust
pub struct Component {
    pub name: String,
    pub molar_mass: f64,
    pub critical_temperature: f64,
    pub critical_pressure: f64,
    pub critical_volume: f64,
    pub acentric_factor: f64,
    pub normal_boiling_point: f64,
    pub cp_ig_a: f64,
    pub cp_ig_b: f64,
    pub cp_ig_c: f64,
    pub cp_ig_d: f64,
    pub cp_ig_e: f64,
    pub ig_entropy_formation_25c: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-readable compound name (identification only). |
| `molar_mass` | `f64` | Molar mass `M` [kg/mol]. Must be > 0. |
| `critical_temperature` | `f64` | Critical temperature `Tc` [K]. Must be > 0. |
| `critical_pressure` | `f64` | Critical pressure `Pc` [Pa]. Must be > 0. |
| `critical_volume` | `f64` | Critical volume `Vc` [m³/mol]. Must be > 0 (or `f64::NAN` if unknown —<br>EOS use Tc/Pc, not Vc, so it may be absent). |
| `acentric_factor` | `f64` | Pitzer acentric factor `ω` [-]. |
| `normal_boiling_point` | `f64` | Normal boiling point `Tb` [K] at 1 atm. Must be > 0 (used by Wilson<br>K-value init only indirectly; may be `f64::NAN` if unknown). |
| `cp_ig_a` | `f64` | Ideal-gas Cp coefficient A (units per the correlation in `ideal_props`). |
| `cp_ig_b` | `f64` | Ideal-gas Cp coefficient B. |
| `cp_ig_c` | `f64` | Ideal-gas Cp coefficient C. |
| `cp_ig_d` | `f64` | Ideal-gas Cp coefficient D. |
| `cp_ig_e` | `f64` | Ideal-gas Cp coefficient E. |
| `ig_entropy_formation_25c` | `f64` | Ideal-gas entropy of formation at 25 °C [J/(mol·K)] (`f64::NAN` if unused). |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, molar_mass: f64, critical_temperature: f64, critical_pressure: f64, critical_volume: f64, acentric_factor: f64, normal_boiling_point: f64, cp_ig: [f64; 5], ig_entropy_formation_25c: f64) -> Result<Self, ComponentError> { /* ... */ }
  ```
  Construct a component from its critical constants, acentric factor, and

- ```rust
  pub fn reduced_temperature(self: &Self, temperature: f64) -> f64 { /* ... */ }
  ```
  Reduced temperature `Tr = T / Tc` [-] at `temperature` [K].

- ```rust
  pub fn reduced_pressure(self: &Self, pressure: f64) -> f64 { /* ... */ }
  ```
  Reduced pressure `Pr = P / Pc` [-] at `pressure` [Pa].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Component { /* ... */ }
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
    fn eq(self: &Self, other: &Component) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ComponentError`

Error constructing a [`Component`] from out-of-range constants.

```rust
pub enum ComponentError {
    NonPositive {
        name: String,
        property: &'static str,
        value: f64,
    },
}
```

##### Variants

###### `NonPositive`

A required positive property was zero, negative, or non-finite.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Compound name. |
| `property` | `&'static str` | Offending property. |
| `value` | `f64` | Offending value. |

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
    fn clone(self: &Self) -> ComponentError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ComponentError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `cubic_eos`

Peng-Robinson & Soave-Redlich-Kwong cubic equations of state.

Ported from DWSIM (GPL-3.0), Visual-Basic reference source:
- `DWSIM.Thermodynamics/PropertyPackages/Models/PengRobinson.vb`
  (`Z_PR` L212-351, `CalcLnFugCPU` L1149-1289, `H_PR_MIX_CPU` L435-574,
  `S_PR_MIX` L576-…, `Calc_dadT` L870-890, `Calc_SUM1`/`Calc_SUM2`
  L892-929).
- `DWSIM.Thermodynamics/PropertyPackages/Models/SoaveRedlichKwong.vb`
  (`Z_SRK` L92-…, `H_SRK_MIX` L236-…, `S_SRK_MIX` L436-…) and
  `.../Models/SoaveRedlichKwong2.vb` (`CalcLnFugCPU` L311-436).

The two models share one algebraic skeleton — the generalised cubic

`P = RT/(V - b) - a(T) / (V^2 + u b V + w b^2)` —

differing only in the constant pair `(u, w)` and the pure-component
parameter constants `(Ωa, Ωb, α-slope)`. This port therefore keeps a single
[`CubicEos`] enum whose per-variant constants drive one shared set of
routines (van der Waals one-fluid mixing, the `Z` cubic, fugacity
coefficients, and enthalpy/entropy departures), exactly as DWSIM's two model
files duplicate the same generalised `(u, w)` departure expression
(`PengRobinson.vb` L551-558; `SoaveRedlichKwong.vb` L412-418).

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

DWSIM works in SI internally, and so does this kernel: temperature K,
pressure Pa, the EOS `a` parameter J·m³/mol² (= Pa·m⁶/mol²), `b` m³/mol,
enthalpy departure J/mol, entropy departure J/(mol·K). Mole fractions and
the compressibility factor `Z` are dimensionless. Raw `f64` is used in the
multi-component inner loops (matching [`crate::thermo::component`]) rather
than `uom`, because the mixing/fugacity math is over `&[f64]` slices and
`&[Component]` where `uom` wrapping would add friction without adding
safety; every public signature spells out its units in its doc comment.

## Design (crate `CLAUDE.md`)

Enum dispatch, **no `dyn`**: the closed set `{PengRobinson, Srk}` is a
[`CubicEos`] enum, not a trait object; no `Box`, no lifetimes, no channels.

## Honest scope — what is and is NOT ported

This is a faithful port of DWSIM's **base** PR and SRK cubic EOS math only.
Deliberately **excluded** (documented here so the omission is explicit):

- **Peneloux volume translation.** DWSIM's density path applies a
  volume-translation (Peneloux) shift to the molar volume; this port returns
  the untranslated cubic-EOS `Z`/`V`. Densities from this module are the raw
  cubic-EOS values, not the volume-corrected ones DWSIM reports.
- **Temperature-dependent binary interaction parameters.** DWSIM can read
  `k_ij(T)` correlations (`PRSRKTDep` advanced package). Here `k_ij` is a
  constant matrix (default 0); no temperature dependence.
- **PR78 / PRSV / advanced α-functions and association terms.** Only the
  classic 1976 PR κ(ω) and the 1972 SRK m(ω) α-slopes are ported. The
  Mathias-Copeman / Twu / PRSV2 α-forms and any association/CPA terms are
  out of scope (`PengRobinson78.vb`, `PRSV2.vb`, `SRKAdvanced.vb`).
- **Gibbs-energy root selection.** DWSIM's mixture fugacity path can pick the
  `Z` root of minimum Gibbs energy (`ZtoMinG`). This port selects by phase
  (vapour = largest root, liquid = smallest positive root), matching DWSIM's
  own `Z_PR`/`H_*_MIX` phase-tagged selection (`PengRobinson.vb` L339-343).

> **⚠️ Unverified until validated.** Early-stage AI-assisted translation.
> The tests below are *verification* against hand-computed / textbook
> single-point values (are the equations implemented correctly?), **not**
> validation against experimental VLE benchmarks. Not for nuclear facility
> operation, reactor control, safety-critical, or licensing decisions.
> Independent OUTRAM PARK fork, not the official DWSIM.

```rust
pub mod cubic_eos { /* ... */ }
```

### Types

#### Enum `Phase`

Fluid phase selector for root/departure choice.

Determines which real root of the compressibility cubic is returned:
[`Phase::Vapor`] takes the largest real root; [`Phase::Liquid`] the smallest
strictly-positive real root. With a single real root, both return it.

```rust
pub enum Phase {
    Vapor,
    Liquid,
}
```

##### Variants

###### `Vapor`

Vapour phase — largest real `Z` root.

###### `Liquid`

Liquid phase — smallest positive real `Z` root.

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

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Phase) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `CubicEos`

A cubic equation of state.

Enum dispatch over the closed set of supported cubic EOS (no trait objects
per the workspace `CLAUDE.md`). Each variant carries no data; its physical
constants are returned by the methods below. All variants share the
generalised `(u, w)` cubic form and the van der Waals one-fluid mixing rule.

```rust
pub enum CubicEos {
    PengRobinson,
    Srk,
}
```

##### Variants

###### `PengRobinson`

Peng-Robinson (1976): `Ωa = 0.45724`, `Ωb = 0.07780`,
`κ(ω) = 0.37464 + 1.54226 ω − 0.26992 ω²`, `(u, w) = (2, −1)`.

###### `Srk`

Soave-Redlich-Kwong (1972): `Ωa = 0.42748`, `Ωb = 0.08664`,
`m(ω) = 0.480 + 1.574 ω − 0.176 ω²`, `(u, w) = (1, 0)`.

##### Implementations

###### Methods

- ```rust
  pub fn omega_a(self: Self) -> f64 { /* ... */ }
  ```
  Attraction-parameter prefactor `Ωa` [-] in `a_i = Ωa α(Tr) R² Tc² / Pc`.

- ```rust
  pub fn omega_b(self: Self) -> f64 { /* ... */ }
  ```
  Co-volume prefactor `Ωb` [-] in `b_i = Ωb R Tc / Pc`.

- ```rust
  pub fn u(self: Self) -> f64 { /* ... */ }
  ```
  Cubic-form constant `u` [-] in the denominator `V² + u b V + w b²`.

- ```rust
  pub fn w(self: Self) -> f64 { /* ... */ }
  ```
  Cubic-form constant `w` [-] in the denominator `V² + u b V + w b²`.

- ```rust
  pub fn sqrt_disc(self: Self) -> f64 { /* ... */ }
  ```
  `√(u² − 4w)` [-] — the discriminant root appearing in every

- ```rust
  pub fn alpha_slope(self: Self, acentric_factor: f64) -> f64 { /* ... */ }
  ```
  α-function slope: PR's `κ(ω)` or SRK's `m(ω)` [-].

- ```rust
  pub fn alpha(self: Self, tr: f64, acentric_factor: f64) -> f64 { /* ... */ }
  ```
  Temperature-dependent scaling factor `α(Tr) = [1 + slope·(1 − √Tr)]²`

- ```rust
  pub fn a_i(self: Self, comp: &Component, t: f64) -> f64 { /* ... */ }
  ```
  Pure-component attraction parameter `a_i(T) = Ωa α(Tr) R² Tc² / Pc`

- ```rust
  pub fn b_i(self: Self, comp: &Component) -> f64 { /* ... */ }
  ```
  Pure-component co-volume `b_i = Ωb R Tc / Pc` [m³/mol].

- ```rust
  pub fn b_mix(self: Self, comps: &[Component], z: &[f64]) -> f64 { /* ... */ }
  ```
  Van der Waals one-fluid mixture co-volume `b_mix = Σ z_i b_i` [m³/mol].

- ```rust
  pub fn a_mix(self: Self, comps: &[Component], z: &[f64], t: f64, kij: Option<&BinaryInteraction>) -> f64 { /* ... */ }
  ```
  Van der Waals one-fluid mixture attraction

- ```rust
  pub fn z_roots(self: Self, a: f64, b: f64) -> Vec<f64> { /* ... */ }
  ```
  Real roots of the compressibility-factor cubic

- ```rust
  pub fn z_factor(self: Self, comps: &[Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&BinaryInteraction>) -> Option<f64> { /* ... */ }
  ```
  Assemble `A`, `B` and return the phase-selected compressibility factor

- ```rust
  pub fn z_vapor(self: Self, a: f64, b: f64) -> Option<f64> { /* ... */ }
  ```
  Largest real compressibility root — the vapour-phase `Z` [-].

- ```rust
  pub fn z_liquid(self: Self, a: f64, b: f64) -> Option<f64> { /* ... */ }
  ```
  Smallest strictly-positive real compressibility root — the liquid-phase

- ```rust
  pub fn ln_phi(self: Self, comps: &[Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&BinaryInteraction>) -> Option<Vec<f64>> { /* ... */ }
  ```
  Natural log of the fugacity coefficient `ln φ_i` [-] for every component

- ```rust
  pub fn dadt(self: Self, comps: &[Component], z: &[f64], t: f64, kij: Option<&BinaryInteraction>) -> f64 { /* ... */ }
  ```
  Temperature derivative of the mixture attraction `d a_mix / dT`

- ```rust
  pub fn enthalpy_departure(self: Self, comps: &[Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&BinaryInteraction>) -> Option<f64> { /* ... */ }
  ```
  Molar enthalpy departure `H(T,P) − H_ideal(T)` [J/mol] for a phase.

- ```rust
  pub fn entropy_departure(self: Self, comps: &[Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&BinaryInteraction>) -> Option<f64> { /* ... */ }
  ```
  Molar entropy departure `S(T,P) − S_ideal(T,P)` [J/(mol·K)] for a phase.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CubicEos { /* ... */ }
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
    fn eq(self: &Self, other: &CubicEos) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BinaryInteraction`

Symmetric binary-interaction-parameter (`k_ij`) matrix for the van der Waals
one-fluid mixing rule.

`k_ij` [-] is a small empirical correction to the geometric-mean cross
attraction `√(a_i a_j)(1 − k_ij)`. It is dimensionless, usually `|k_ij| <
0.2`, with `k_ii = 0` on the diagonal. A `None` matrix everywhere (or this
all-zeros matrix) recovers the ideal geometric-mean combining rule.

```rust
pub struct BinaryInteraction {
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
  pub fn zeros(n: usize) -> Self { /* ... */ }
  ```
  All-zero `k_ij` matrix for `n` components (the geometric-mean default).

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of components `n` [-].

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  True if the matrix carries no components.

- ```rust
  pub fn get(self: &Self, i: usize, j: usize) -> f64 { /* ... */ }
  ```
  `k_ij` [-] for the `(i, j)` pair. Panics if either index is `≥ n`.

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, value: f64) { /* ... */ }
  ```
  Set both `k_ij` and `k_ji` to `value` [-] (the matrix stays symmetric).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BinaryInteraction { /* ... */ }
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
    fn eq(self: &Self, other: &BinaryInteraction) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Constant `R`

Universal gas constant `R` [J/(mol·K)] — CODATA 2018 exact value.

DWSIM's VB source hard-codes the rounded `R = 8.314`; this port uses the
exact SI value, so single-point numbers differ from DWSIM at the 5th
significant figure. All verification numbers below are computed with this
`R`.

```rust
pub const R: f64 = 8.31446261815324;
```

## Module `electrolyte`

Electrolyte (aqueous-ionic) activity-coefficient tier — DWSIM port.

GPLv3 provenance (upstream: **DWSIM**, GPL-3.0, commit `1abf72d`):

- `DWSIM.Thermodynamics/PropertyPackages/ElectrolyteBase.vb` — the
  electrolyte property-package substrate (ion speciation, phase glue).
- `DWSIM.Thermodynamics/PropertyPackages/ElectrolyteIdeal.vb` — the simplest
  aqueous-ionic model (molality-scale ideal + a Debye-Hückel mean-ionic term,
  `Models/ElectrolyteProperties.vb::MIAC`).
- `DWSIM.Thermodynamics/PropertyPackages/LIQUAC2PropertyPackage.vb` and its
  activity kernel `PropertyPackages/Models/LIQUAC2.vb::GAMMA_MR` — LIQUAC =
  Debye-Hückel **long-range** + a **middle-range** ionic virial term +
  UNIQUAC **short-range** for strong electrolytes.
- `DWSIM.Thermodynamics/FlashAlgorithms/ElectrolyteSVLE.vb` — the aqueous
  solid-vapour-liquid ionic equilibrium / speciation flash.
- Osmotic coefficient / pH / freezing-point functions:
  `PropertyPackages/Models/ElectrolyteProperties.vb`.

This is a GPL-3.0 derivative of those files. Copyright of the original
algorithm: Daniel Wagner O. de Medeiros (DWSIM). LIQUAC reference:
Li, Polka & Gmehling, *Fluid Phase Equilibria* (1994) / the ACS paper cited
in `LIQUAC2.vb` (`https://pubs.acs.org/doi/10.1021/ie0510122`).

> **⚠️ Untrusted draft, pending human V&V.** Early-stage translation, no human
> review. Independent OUTRAM PARK fork, **not** the official DWSIM. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions (workspace `RESPONSIBLE_USE.md`). The verification tests below
> check the Debye-Hückel limiting law and electroneutrality analytically;
> nothing here is *validated* against experimental electrolyte data.

# What this computes, and in what units

For an aqueous mixture of neutral solvent(s) and ions given as **mole
fractions** `x` (dimensionless, should sum to 1) at temperature `T` (kelvin),
this module computes:

- **Molality** `m_i = x_i / w` where `w = Σ_solvent x_s M_s` is the solvent
  mass per mole of mixture [kg]. Molality is in **mol/kg (solvent)**.
  (`LIQUAC2.vb:263-283`, `ElectrolyteProperties.vb:86-103`.)
- **Ionic strength** `I = ½ Σ_i z_i² m_i` [mol/kg] (`LIQUAC2.vb:319-324`).
- **Electroneutrality residual** `Σ_i z_i m_i` [mol/kg], which must be `0`.
- **Activity coefficients** `γ_i` (dimensionless, `> 0`). For ions these are
  on the **molality scale, unsymmetric (McMillan-Mayer) convention**
  (`γ_i → 1` as `I → 0`); for the solvent on the mole-fraction scale.

# Debye-Hückel constant and convention (read carefully)

The long-range term uses the **natural-log** Debye-Hückel form

```text
ln γ_i^LR = -A z_i² √I / (1 + b √I)          (LIQUAC2.vb:382)
```

with, following `LIQUAC2.vb:326-327`,

```text
A = 132775.7 · √ρ_solv / (ε_solv · T)^{3/2}   [kg^{1/2} mol^{-1/2}]
b =   6.359696 · √ρ_solv / (ε_solv · T)^{1/2}  [kg^{1/2} mol^{-1/2}]
```

where `ρ_solv` is the solvent mass density [kg/m³] and `ε_solv` its (static,
dimensionless) relative permittivity. For **water at 25 °C**
(`ρ = 997.05 kg/m³`, `ε = 78.25`) this gives **`A ≈ 1.1766 kg^{1/2}
mol^{-1/2}`** and `b ≈ 1.3147` — see [`DebyeHuckel::water_25c`]. The value
`A ≈ 1.174` on the natural-log scale is the textbook water-25 °C constant
(`= 2.303 × 0.5108`, the log₁₀ constant `A_10 ≈ 0.511`). Everything in this
module is on the **ln (natural-log)** scale.

In the **limiting law** (`I → 0`, drop the `1 + b√I` denominator):

```text
ln γ_i^LR → -A z_i² √I ,     ln γ_± = -A |z_+ z_-| √I
```

which the analytic V&V test below reproduces to machine precision at low `I`.

# Design (workspace + crate `CLAUDE.md`)

Enum dispatch ([`ElectrolyteModel`]); no `dyn`, no `Box<T>`, no lifetimes.
Species are held **by value** and referenced **by index** (`usize`). Raw
`f64` in the inner arithmetic loops with every quantity's unit documented.
The UNIQUAC short-range term **reuses** [`crate::thermo::activity::UniquacParams`]
(symmetric convention) rather than re-deriving the combinatorial/residual
math — see [`LiquacModel`] for the documented reference-state caveat.

# Honest scope — what is and is not ported

**Done, and analytically verified:** molality / ionic-strength /
electroneutrality bookkeeping; the Debye-Hückel long-range term (LIQUAC form)
and its limiting law; the mean ionic activity coefficient; the osmotic
coefficient and pH helpers; strong-electrolyte (complete-dissociation)
speciation — the kernel the `ElectrolyteSVLE` flash iterates on.

**Lean / partial (documented):** the LIQUAC **middle-range** ionic-virial
term is implemented structurally but its interaction coefficients `B_ij`
(`LIQUAC2_IP.txt`, an extensive tabulated database) are **caller-supplied and
default to zero** — this port does not inline that database. The UNIQUAC
short-range reuses the symmetric kernel; DWSIM's ion **reference-state
normalization** (`LIQUAC2.vb:607-614`) is *not* applied, so ion short-range
coefficients are approximate. The full `ElectrolyteSVLE` flash (reaction-set
chemical equilibria solved with an IPOPT extent optimiser, coupled to a VLE
nested-loops flash — `ElectrolyteSVLE.vb:241-540`) is **not** ported; only
the strong-electrolyte dissociation speciation is.

The **data tables are deliberately lean**: a tiny built-in Na⁺ / Cl⁻ / H₂O
set for tests and examples (see [`presets`]). Any real electrolyte study
needs the full DWSIM `LIQUAC2_RiQi.txt` / `LIQUAC2_IP.txt` /
`dielectricconstants.txt` parameter databases, which are **not** bundled here.

```rust
pub mod electrolyte { /* ... */ }
```

### Modules

## Module `presets`

Lean built-in species presets — a **tiny** Na⁺ / Cl⁻ / H₂O set for tests and
examples. **Not** a parameter database: any real study needs DWSIM's full
`LIQUAC2_RiQi.txt` / `LIQUAC2_IP.txt` / `dielectricconstants.txt`, which are
not bundled in this fork.

Molar masses are standard public IUPAC atomic-weight values.

```rust
pub mod presets { /* ... */ }
```

### Functions

#### Function `water`

**Attributes:**

- `MustUse { reason: None }`

Water H₂O (solvent). `M = 0.018015 kg/mol`.

```rust
pub fn water() -> super::AqueousSpecies { /* ... */ }
```

#### Function `sodium_ion`

**Attributes:**

- `MustUse { reason: None }`

Sodium ion Na⁺ (`z = +1`). `M = 0.022990 kg/mol`.

```rust
pub fn sodium_ion() -> super::AqueousSpecies { /* ... */ }
```

#### Function `chloride_ion`

**Attributes:**

- `MustUse { reason: None }`

Chloride ion Cl⁻ (`z = -1`). `M = 0.035453 kg/mol`.

```rust
pub fn chloride_ion() -> super::AqueousSpecies { /* ... */ }
```

#### Function `hydrogen_ion`

**Attributes:**

- `MustUse { reason: None }`

Hydrogen ion (hydron) H⁺ (`z = +1`). `M = 0.001008 kg/mol`.

```rust
pub fn hydrogen_ion() -> super::AqueousSpecies { /* ... */ }
```

### Types

#### Struct `AqueousSpecies`

A single aqueous species: a neutral solvent, a neutral solute, or an ion.

Held **by value**; a mixture is an ordered `Vec<AqueousSpecies>` and every
composition / coefficient vector is indexed to match (species `i` ↔ `x[i]`).

# Units
- `molar_mass` — molar mass `M` [kg/mol], must be `> 0`
- `charge` — signed ionic charge number `z` [-] (`0` for neutral species)

```rust
pub struct AqueousSpecies {
    pub name: String,
    pub charge: i32,
    pub molar_mass: f64,
    pub is_solvent: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-readable species name (identification only; e.g. `"Water"`,<br>`"Na+"`, `"Cl-"`). |
| `charge` | `i32` | Signed charge number `z_i` [-]. `0` for neutral molecules; `+1` for Na⁺,<br>`-1` for Cl⁻, `-2` for SO₄²⁻, etc. |
| `molar_mass` | `f64` | Molar mass `M_i` [kg/mol]. Must be finite and `> 0`. |
| `is_solvent` | `bool` | Whether this species counts as **solvent** for the molality mass basis<br>(`w = Σ_solvent x_s M_s`). DWSIM treats Water (and Methanol) as solvent<br>(`LIQUAC2.vb:267`, `ElectrolyteProperties.vb:90`). Neutral solutes and<br>ions are *not* solvent. |

##### Implementations

###### Methods

- ```rust
  pub fn solvent</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, molar_mass: f64) -> Self { /* ... */ }
  ```
  Construct a neutral **solvent** species (e.g. water) of molar mass

- ```rust
  pub fn ion</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, charge: i32, molar_mass: f64) -> Self { /* ... */ }
  ```
  Construct an **ion** of signed charge `charge` [-] and molar mass

- ```rust
  pub fn is_ion(self: &Self) -> bool { /* ... */ }
  ```
  `true` if this species carries a non-zero charge.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AqueousSpecies { /* ... */ }
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
    fn eq(self: &Self, other: &AqueousSpecies) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AqueousSystem`

An ordered aqueous mixture: the species list plus the molality / ionic-
strength / electroneutrality bookkeeping every electrolyte model needs.

Composition is supplied per call as a mole-fraction slice `x` whose length
must equal [`Self::n`]. All quantities follow the module-level unit
conventions (molality in mol/kg, ionic strength in mol/kg).

```rust
pub struct AqueousSystem {
    pub species: Vec<AqueousSpecies>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `species` | `Vec<AqueousSpecies>` | The species, in index order. |

##### Implementations

###### Methods

- ```rust
  pub fn new(species: Vec<AqueousSpecies>) -> Self { /* ... */ }
  ```
  Build a system from an ordered species list. Panics if empty.

- ```rust
  pub fn n(self: &Self) -> usize { /* ... */ }
  ```
  Number of species `n`.

- ```rust
  pub fn solvent_mass(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Solvent mass per mole of mixture `w = Σ_solvent x_s M_s` [kg], the

- ```rust
  pub fn molalities(self: &Self, x: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Molalities `m_i = x_i / w` [mol/kg] for every species (`LIQUAC2.vb:277`).

- ```rust
  pub fn ionic_strength(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Ionic strength `I = ½ Σ_i z_i² m_i` [mol/kg] on the molality scale

- ```rust
  pub fn net_charge_molality(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Electroneutrality residual `Σ_i z_i m_i` [mol/kg]. A physically valid

- ```rust
  pub fn is_electroneutral(self: &Self, x: &[f64], tol: f64) -> bool { /* ... */ }
  ```
  `true` if the composition is electroneutral to within `tol` [mol/kg]

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AqueousSystem { /* ... */ }
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
    fn eq(self: &Self, other: &AqueousSystem) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DebyeHuckel`

Debye-Hückel long-range term with the LIQUAC constants.

Stores the two solvent-dependent coefficients `A` and `b` (both in
`kg^{1/2} mol^{-1/2}`, natural-log scale) so a model can evaluate the
long-range activity contribution of any ion at a given ionic strength.

```rust
pub struct DebyeHuckel {
    pub a: f64,
    pub b: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Debye-Hückel `A`-coefficient [kg^{1/2} mol^{-1/2}], natural-log scale<br>(`= A_CONST √ρ / (ε T)^{3/2}`). |
| `b` | `f64` | Debye-Hückel `b`-coefficient [kg^{1/2} mol^{-1/2}] in the `1 + b√I`<br>denominator (`= B_CONST √ρ / (ε T)^{1/2}`). |

##### Implementations

###### Methods

- ```rust
  pub fn from_solvent(density: f64, dielectric: f64, t: f64) -> Self { /* ... */ }
  ```
  Build the coefficients from solvent density and permittivity at `T`,

- ```rust
  pub fn water_25c() -> Self { /* ... */ }
  ```
  Water at 25 °C: `ρ = 997.05 kg/m³`, `ε` from

- ```rust
  pub fn ln_gamma_ion(self: &Self, z: i32, im: f64) -> f64 { /* ... */ }
  ```
  Long-range `ln γ` of an ion of charge `z` at ionic strength `im` [mol/kg]

- ```rust
  pub fn ln_gamma_ion_limiting(self: &Self, z: i32, im: f64) -> f64 { /* ... */ }
  ```
  Debye-Hückel **limiting law** `ln γ = -A z² √I` (the `I → 0` limit of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> DebyeHuckel { /* ... */ }
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
    fn eq(self: &Self, other: &DebyeHuckel) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ElectrolyteModel`

Aqueous-ionic activity-coefficient model (enum dispatch, no `dyn`).

Given an [`AqueousSystem`], a mole-fraction composition `x`, and temperature
`t` [K], [`Self::activity_coefficients`] returns `γ_i` (dimensionless, `> 0`)
— molality-scale unsymmetric for ions, mole-fraction-scale for the solvent.

```rust
pub enum ElectrolyteModel {
    Ideal(IdealElectrolyte),
    Liquac(LiquacModel),
}
```

##### Variants

###### `Ideal`

**Ideal electrolyte** (`ElectrolyteIdeal.vb`): ions carry only the
Debye-Hückel long-range term; neutral species are ideal (`γ = 1`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `IdealElectrolyte` |  |

###### `Liquac`

**LIQUAC** (`LIQUAC2.vb`): long-range (Debye-Hückel) + middle-range
(ionic virial) + short-range (UNIQUAC).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LiquacModel` |  |

##### Implementations

###### Methods

- ```rust
  pub fn activity_coefficients(self: &Self, system: &AqueousSystem, x: &[f64], t: f64) -> Vec<f64> { /* ... */ }
  ```
  Activity coefficients `γ_i` at composition `x` (mole fractions) and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ElectrolyteModel { /* ... */ }
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
    fn eq(self: &Self, other: &ElectrolyteModel) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `IdealElectrolyte`

Ideal aqueous-ionic model — the simplest electrolyte tier
(`ElectrolyteIdeal.vb`). Ions get the Debye-Hückel long-range term only;
neutral solvents/solutes are ideal (`γ = 1`).

```rust
pub struct IdealElectrolyte {
    pub dh: DebyeHuckel,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dh` | `DebyeHuckel` | The Debye-Hückel long-range coefficients (solvent + temperature fixed at<br>construction; e.g. [`DebyeHuckel::water_25c`]). |

##### Implementations

###### Methods

- ```rust
  pub fn new(dh: DebyeHuckel) -> Self { /* ... */ }
  ```
  Construct from a [`DebyeHuckel`] coefficient set.

- ```rust
  pub fn activity_coefficients(self: &Self, system: &AqueousSystem, x: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  `γ_i`: ions get `exp(ln γ^LR)` at the composition's ionic strength;

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> IdealElectrolyte { /* ... */ }
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
    fn eq(self: &Self, other: &IdealElectrolyte) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `LiquacModel`

LIQUAC model: Debye-Hückel long-range + (lean) middle-range + UNIQUAC
short-range (`LIQUAC2.vb::GAMMA_MR`).

```text
ln γ_i = ln γ_i^LR + ln γ_i^MR + ln γ_i^SR
```

- **Long-range** `LR` — [`DebyeHuckel::ln_gamma_ion`] for ions; `0` for the
  solvent (DWSIM's solvent-`LR` `σ` expression, `LIQUAC2.vb:383-387`, is a
  documented omission — it is small and the reference itself carries a
  commented-out alternative form).
- **Middle-range** `MR` — ionic virial term (`LIQUAC2.vb:414-531`). Requires
  the `B_ij` interaction database (`LIQUAC2_IP.txt`). **Not inlined here**:
  `middle_range` is caller-supplied and, when `None`, the MR term is `0`.
- **Short-range** `SR` — **reuses** [`UniquacParams`] (workspace UNIQUAC
  kernel) in the *symmetric* convention. DWSIM additionally applies an ion
  **reference-state normalization** (`LIQUAC2.vb:607-614`) to make the ion SR
  term unsymmetric; that normalization is **not** applied here, so ion SR
  coefficients are approximate. When `short_range` is `None`, SR is `0`.

The long-range term is the analytically-verified core; MR and SR are the
documented lean/partial parts.

```rust
pub struct LiquacModel {
    pub dh: DebyeHuckel,
    pub short_range: Option<crate::thermo::activity::UniquacParams>,
    pub middle_range: Option<Vec<Vec<f64>>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dh` | `DebyeHuckel` | Debye-Hückel long-range coefficients. |
| `short_range` | `Option<crate::thermo::activity::UniquacParams>` | Optional UNIQUAC short-range parameters (symmetric convention, reused<br>from [`crate::thermo::activity`]). `r`/`q` and interaction energies must<br>be `n × n` for the same `n` as the [`AqueousSystem`]. |
| `middle_range` | `Option<Vec<Vec<f64>>>` | Optional lean middle-range interaction matrix `B_ij` [kg/mol], `n × n`.<br>When present, contributes an ion-molality virial term (see<br>[`Self::activity_coefficients`]). `None` ⇒ MR term is `0`. |

##### Implementations

###### Methods

- ```rust
  pub fn long_range_only(dh: DebyeHuckel) -> Self { /* ... */ }
  ```
  Long-range-only LIQUAC (no MR, no SR) — the analytically-grounded core.

- ```rust
  pub fn with_short_range(self: Self, uniquac: UniquacParams) -> Self { /* ... */ }
  ```
  Attach a UNIQUAC short-range term (reused workspace kernel).

- ```rust
  pub fn with_middle_range(self: Self, b: Vec<Vec<f64>>) -> Self { /* ... */ }
  ```
  Attach a lean middle-range `B_ij` [kg/mol] matrix (`n × n`).

- ```rust
  pub fn activity_coefficients(self: &Self, system: &AqueousSystem, x: &[f64], t: f64) -> Vec<f64> { /* ... */ }
  ```
  `γ_i = exp(ln γ_i^LR + ln γ_i^MR + ln γ_i^SR)`. Panics if `x.len()` (or

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LiquacModel { /* ... */ }
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
    fn eq(self: &Self, other: &LiquacModel) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `water_dielectric_constant`

**Attributes:**

- `MustUse { reason: None }`

Relative static permittivity (dielectric constant) of liquid water as a
function of temperature `t` [K], DWSIM's polynomial
(`ElectrolyteProperties.vb:42`):

```text
ε(T) = 289.82 - 1.148 T + 0.0017843 T² - 1.053e-6 T³   [-]
```

Valid roughly `273–373 K`. At `T = 298.15 K` this returns `ε ≈ 78.26`.

# Units
- `t` — temperature [K]
- returns — relative permittivity `ε_r` [dimensionless]

```rust
pub fn water_dielectric_constant(t: f64) -> f64 { /* ... */ }
```

#### Function `mean_ionic_ln_gamma`

**Attributes:**

- `MustUse { reason: None }`

Mean ionic activity coefficient (natural log) of a single salt
`C_{ν+} A_{ν-}` from its individual-ion long-range coefficients:

```text
ln γ_± = (ν_+ ln γ_+ + ν_- ln γ_-) / (ν_+ + ν_-)
```

Combined with the limiting law and electroneutrality (`ν_+ z_+ = ν_- |z_-|`)
this collapses to `ln γ_± = -A |z_+ z_-| √I`, the form the V&V test checks.

# Arguments (all dimensionless)
- `nu_plus`, `nu_minus` — stoichiometric coefficients `ν_+`, `ν_-`
- `ln_gamma_plus`, `ln_gamma_minus` — the cation / anion `ln γ`

```rust
pub fn mean_ionic_ln_gamma(nu_plus: f64, nu_minus: f64, ln_gamma_plus: f64, ln_gamma_minus: f64) -> f64 { /* ... */ }
```

#### Function `osmotic_coefficient`

**Attributes:**

- `MustUse { reason: None }`

Osmotic coefficient of the solvent (`ElectrolyteProperties.vb::OsmoticCoeff`,
lines 76-108):

```text
Φ = -ln(x_w · γ_w) / (M_w · Σ_ion m_i)
```

where `x_w`, `γ_w` are the solvent mole fraction and activity coefficient,
`M_w` its molar mass [kg/mol], and `Σ_ion m_i` the total ion molality
[mol/kg]. Dimensionless.

# Arguments
- `system` — the aqueous mixture
- `x` — mole fractions [-]
- `gamma` — activity coefficients [-] (same order/length as `x`)
- `solvent_index` — index of the reference solvent (e.g. water)

Returns `f64::NAN` if there are no ions (undefined osmotic coefficient).
Panics if lengths disagree.

```rust
pub fn osmotic_coefficient(system: &AqueousSystem, x: &[f64], gamma: &[f64], solvent_index: usize) -> f64 { /* ... */ }
```

#### Function `ph_from_hydrogen`

**Attributes:**

- `MustUse { reason: None }`

pH from the hydrogen-ion molality and its activity coefficient
(`ElectrolyteProperties.vb::pH`, lines 111-148, molality form):

```text
pH = -log₁₀(m_{H⁺} · γ_{H⁺})
```

# Note on DWSIM's density factor
DWSIM multiplies `m_{H⁺}` by `ρ_liq/1000` to approximate a molarity
(mol/L) basis before taking `-log₁₀`. This port uses the **molality**
(mol/kg) basis directly (the `ρ/1000` factor is `≈ 1` for dilute aqueous
solutions and requires a liquid-density model not ported here). Documented
simplification.

# Arguments
- `system`, `x` — mixture and mole fractions
- `gamma` — activity coefficients [-]
- `hydrogen_index` — index of the H⁺ (`Hydron`) species

Panics if lengths disagree.

```rust
pub fn ph_from_hydrogen(system: &AqueousSystem, x: &[f64], gamma: &[f64], hydrogen_index: usize) -> f64 { /* ... */ }
```

#### Function `dissociate_strong_salt`

**Attributes:**

- `MustUse { reason: None }`

Strong-electrolyte **complete-dissociation speciation** — the kernel the
`ElectrolyteSVLE` flash iterates on for a strong salt (`ElectrolyteSVLE.vb`:
its `SolveChemicalEquilibria` drives every strong reaction to completion).

Given a salt `C_{ν+} A_{ν-}` dissolved at molality `m_salt` [mol/kg], returns
the resulting `(cation_molality, anion_molality)` [mol/kg] assuming the salt
fully dissociates (the strong-electrolyte limit):

```text
m_+ = ν_+ · m_salt ,   m_- = ν_- · m_salt
```

The result is charge-balanced by construction when `ν_+ z_+ + ν_- z_- = 0`;
[`is_dissociation_neutral`] checks that stoichiometry.

This is the *strong* speciation only. DWSIM's full flash also solves weak
(finite-`K_eq`) equilibria via an extent optimiser — **not** ported here.

```rust
pub fn dissociate_strong_salt(m_salt: f64, nu_plus: f64, nu_minus: f64) -> (f64, f64) { /* ... */ }
```

#### Function `is_dissociation_neutral`

**Attributes:**

- `MustUse { reason: None }`

Check that a salt's dissociation stoichiometry is charge-neutral:
`ν_+ z_+ + ν_- z_- = 0` (`z_-` is negative). Returns `true` when balanced.

```rust
pub fn is_dissociation_neutral(nu_plus: f64, z_plus: i32, nu_minus: f64, z_minus: i32) -> bool { /* ... */ }
```

### Constants and Statics

#### Constant `A_CONST`

LIQUAC Debye-Hückel long-range **`A`-constant** numerator
(`LIQUAC2.vb:326`): `A = A_CONST · √ρ / (ε T)^{3/2}`. Dimensioned so that, with
`ρ` in kg/m³, `ε` dimensionless and `T` in K, `A` comes out in
`kg^{1/2} mol^{-1/2}` on the natural-log scale.

```rust
pub const A_CONST: f64 = 132_775.7;
```

#### Constant `B_CONST`

LIQUAC Debye-Hückel **`b`-constant** numerator (`LIQUAC2.vb:327`):
`b = B_CONST · √ρ / (ε T)^{1/2}`, giving `b` in `kg^{1/2} mol^{-1/2}`.

```rust
pub const B_CONST: f64 = 6.359_696;
```

#### Constant `M_WATER`

Molar mass of water `M_{H₂O} = 0.018015 kg/mol`. Used as the default solvent
mass basis for molality.

```rust
pub const M_WATER: f64 = 0.018_015;
```

## Module `electrolyte_svle`

Electrolyte SVLE flash — weak-electrolyte reaction-set speciation solver plus
solid (Ksp) precipitation coupling. DWSIM port.

---

# GPLv3 provenance

Upstream project: **DWSIM** (open-source chemical process simulator),
GPL-3.0, upstream commit `1abf72d`.

Ported from
`DWSIM.Thermodynamics/FlashAlgorithms/ElectrolyteSVLE.vb`, specifically the
chemical-equilibrium core the file's own doc comment in
[`crate::thermo::electrolyte`] flags as *not yet ported*:

- `ElectrolyteSVLE.vb:241-554` — `SolveChemicalEquilibria`: assembles the
  equilibrium-reaction stoichiometry matrix `E`, seeds reaction extents, and
  drives a Newton solve of the reaction-set chemical equilibria in the liquid
  phase. Ported here as [`SvleSystem::solve_speciation`].
- `ElectrolyteSVLE.vb:556-698` — `FunctionValue2N`: the residual function
  `f_i = ln(K_i / Q_i)` where `Q_i = Π_s a_s^{ν_{s,i}}` is the reaction
  activity quotient built from **molality-scale** activities for ions/salts
  and **mole-fraction-scale** activities for neutral species. Ported here as
  the private `SvleSystem::residual`.
- `ElectrolyteSVLE.vb:73-239` — `Flash_PT`: the outer VLE ⇄ speciation ⇄
  solid loop. The **solid-liquid split** it delegates to (`nl3.Flash_SL`,
  `ElectrolyteSVLE.vb:191`) is DWSIM `NestedLoopsSLE.Flash_SL`, already ported
  at [`crate::thermo::flash_sle::flash_sl`]. This file adds the **Ksp
  solubility** precipitation piece ([`SaltSolubility`]) that the outer loop
  needs for ionic solids, and documents (Honest scope, below) what of the
  full VLE outer loop is deliberately *not* reproduced here.

Copyright of the original algorithm: Daniel Wagner O. de Medeiros (DWSIM).
This Rust file is a GPL-3.0 derivative work.

> **⚠️ Untrusted AI-assisted draft, pending human V&V.** Early-stage
> translation, no human review. Independent OUTRAM PARK fork, **not** the
> official DWSIM. Not for nuclear facility operation, reactor control,
> safety-critical, licensing, or any operational decision
> (`RESPONSIBLE_USE.md`). The tests below are **verification** (the solver
> reproduces analytic / closed-form / literature-constant references), **not
> validation** against an experimental electrolyte database.

---

# What this computes

Given an aqueous liquid phase described by a list of [`SvleSpecies`] (a
solvent, neutral solutes, ions, and optionally solids), a set of
[`EquilibriumReaction`]s with their temperature-evaluated equilibrium
constants `K_i`, and an initial mole-amount vector `n0` \[mol\], this module
finds the reaction extents `ξ_j` \[mol\] such that every reaction satisfies
its mass-action law

```text
K_i = Π_s a_s(n)^{ν_{s,i}} ,     n_s = n0_s + Σ_j ν_{s,j} ξ_j
```

and returns the speciated equilibrium mole amounts `n_s` \[mol\]. Activities
`a_s` follow DWSIM's mixed-scale convention (`ElectrolyteSVLE.vb:633-641`):

- **Ion / salt** species: `a_s = m_s · γ_s`, molality scale, where
  `m_s = x_s / w` \[mol/kg\] and `w = Σ_solvent x_s M_s` \[kg\] is the solvent
  mass per mole of liquid (identical basis to [`crate::thermo::electrolyte`]).
- **Solvent / neutral** species: `a_s = x_s · γ_s`, mole-fraction scale.
- **Solid** species: `a_s = 1` (pure-solid reference state).

Activity coefficients `γ_s` are supplied by [`SvleActivity`]: `Ideal`
(`γ = 1`, which is exactly what DWSIM's `FunctionValue2N` uses —
`activcoeff = proppack.RET_UnitaryVector()`, `ElectrolyteSVLE.vb:628`) or a
Debye-Hückel long-range term reusing [`crate::thermo::electrolyte::DebyeHuckel`].

# Units

| Quantity | Unit |
|---|---|
| Mole amounts `n`, `n0`, extents `ξ` | mol |
| Molar mass `M` | kg/mol |
| Molality `m` | mol/kg (solvent) |
| Ionic strength `I` | mol/kg |
| Temperature `T` | K |
| Equilibrium constant `K`, activity `a`, mole fraction `x`, charge `z` | dimensionless |
| Solubility product `Ksp` | (mol/kg)^(ν₊+ν₋) |

# Honest scope — what is and is NOT ported

**Ported and verified here:**
- The reaction-set chemical-equilibrium solver: extent formulation,
  `ln(K/Q)` residual, the ion/neutral/solid activity-scale convention, and a
  robust root solve (bracketed bisection for a single reaction; damped
  feasibility-clamped Newton with a finite-difference Jacobian for coupled
  reactions). This is the `SolveChemicalEquilibria` / `FunctionValue2N` core.
- The Ksp solid-precipitation piece ([`SaltSolubility`]): saturation
  molality, supersaturation test, and the 1:1-salt precipitation amount —
  giving the solid-onset / reduction-to-no-solid behaviour of the outer loop.

**Deliberately NOT reproduced** (documented omissions, not silent gaps):
- **The full `Flash_PT` VLE outer loop** (`ElectrolyteSVLE.vb:139-217`): the
  alternating vapour-liquid `NestedLoops` flash, the `AUX_PVAPi` vapour-
  pressure feed rebuild, and the `nl.CalculateEquilibrium` calls need a live
  property package. Those pieces exist elsewhere in this crate
  ([`crate::thermo::flash`], [`crate::thermo::property_package`]) but wiring
  the whole outer loop is out of this file's scope. What is ported is the
  *liquid-phase speciation kernel* that loop iterates on, plus the solid
  split it delegates to ([`crate::thermo::flash_sle`]).
- **DWSIM's density-scaled molality** (`ElectrolyteSVLE.vb:604-624`,
  `m = x/w · ρ_liq/1000`, i.e. a molarity approximation) — this port uses the
  clean `m = x/w` \[mol/kg solvent\] basis of [`crate::thermo::electrolyte`]
  (`ρ/1000 ≈ 1` for dilute aqueous), because the liquid-density model DWSIM
  calls is not ported. For dilute aqueous systems the two agree closely.
- **DWSIM's variable scaling + 4 penalty-value schemes**
  (`ElectrolyteSVLE.vb:439-508`, `676-693`): heuristics to nurse the external
  `DotNumerics` optimiser. This port's bracketed/damped solve is robust
  enough for the verification cases without them; they are not reproduced.
  One consequence of dropping DWSIM's per-variable scaling: the coupled
  Newton path uses a **single** finite-difference step across all extents, so
  a reaction set whose extents span many orders of magnitude (e.g. water
  autoionization `ξ ~ 1e-11` coupled to a `ξ ~ 1e-4` acid/base dissociation)
  converges but needs many iterations. It stays well within `max_iter` for
  the two-reaction verification case; larger stiff sets may want per-extent
  scaling (a future refinement, tracked in Honest scope, not yet ported).
- **The `Flash_PH` / `Flash_PV` / `Flash_TV` energy/spec outer flashes**
  (`ElectrolyteSVLE.vb:736-1104`): temperature/pressure outer iterations that
  repeatedly call `Flash_PT`; out of scope for the same reason as the VLE loop.

No experimental electrolyte database is bundled; parameters in tests are
public-literature constants (`Kw`, `Ksp`) or closed-form-checkable values.

```rust
pub mod electrolyte_svle { /* ... */ }
```

### Modules

## Module `presets`

Built-in **public-literature** presets for tests and examples. These are the
only "data" in this file; no experimental electrolyte database is bundled.

```rust
pub mod presets { /* ... */ }
```

### Functions

#### Function `water`

**Attributes:**

- `MustUse { reason: None }`

Water H₂O (solvent). `M = 0.018015 kg/mol`.

```rust
pub fn water() -> super::SvleSpecies { /* ... */ }
```

#### Function `hydrogen_ion`

**Attributes:**

- `MustUse { reason: None }`

Hydrogen ion H⁺ (`z = +1`). `M = 0.001008 kg/mol`.

```rust
pub fn hydrogen_ion() -> super::SvleSpecies { /* ... */ }
```

#### Function `hydroxide_ion`

**Attributes:**

- `MustUse { reason: None }`

Hydroxide ion OH⁻ (`z = -1`). `M = 0.017007 kg/mol`.

```rust
pub fn hydroxide_ion() -> super::SvleSpecies { /* ... */ }
```

#### Function `sodium_ion`

**Attributes:**

- `MustUse { reason: None }`

Sodium ion Na⁺ (`z = +1`). `M = 0.022990 kg/mol`.

```rust
pub fn sodium_ion() -> super::SvleSpecies { /* ... */ }
```

#### Function `chloride_ion`

**Attributes:**

- `MustUse { reason: None }`

Chloride ion Cl⁻ (`z = -1`). `M = 0.035453 kg/mol`.

```rust
pub fn chloride_ion() -> super::SvleSpecies { /* ... */ }
```

#### Function `silver_ion`

**Attributes:**

- `MustUse { reason: None }`

Silver ion Ag⁺ (`z = +1`). `M = 0.107868 kg/mol`.

```rust
pub fn silver_ion() -> super::SvleSpecies { /* ... */ }
```

#### Function `silver_chloride`

**Attributes:**

- `MustUse { reason: None }`

**AgCl solubility product** `Ksp = 1.77e-10` at 25 °C (public CRC-handbook
value; molality/molarity scale, `≈` for dilute aqueous). Solubility
`√Ksp ≈ 1.33e-5 mol/kg`.

```rust
pub fn silver_chloride() -> super::SaltSolubility { /* ... */ }
```

### Constants and Statics

#### Constant `KW_25C`

**Water ion product** `Kw = 1.0e-14` at 25 °C, 1 bar (public CRC/IUPAC
value; molality scale). Used by the autoionization V&V test.

```rust
pub const KW_25C: f64 = 1.0e-14;
```

### Types

#### Enum `SpeciesRole`

Physical role of a species in the aqueous electrolyte, selecting its
activity **scale** in the mass-action law (`ElectrolyteSVLE.vb:633-641`).

Enum dispatch (no `dyn`). Dimensionless classification.

```rust
pub enum SpeciesRole {
    Solvent,
    Neutral,
    Ion,
    Solid,
}
```

##### Variants

###### `Solvent`

Neutral solvent (e.g. water). Counts toward the molality solvent-mass
basis `w = Σ_solvent x_s M_s`; its activity is mole-fraction-scale
`a = x · γ`.

###### `Neutral`

Neutral (uncharged) solute. Activity is mole-fraction-scale `a = x · γ`;
does **not** contribute to the solvent mass basis.

###### `Ion`

Charged ion (or a molality-scale salt). Activity is molality-scale
`a = m · γ` with `m = x / w` \[mol/kg\].

###### `Solid`

Precipitated pure solid. Activity is fixed at `1` (pure-solid reference);
excluded from the liquid mole-fraction and solvent-mass sums.

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
    fn clone(self: &Self) -> SpeciesRole { /* ... */ }
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
    fn eq(self: &Self, other: &SpeciesRole) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SvleSpecies`

A single species in the electrolyte liquid phase.

Held **by value**; a system is an ordered `Vec<SvleSpecies>` and every
composition / stoichiometry vector is indexed to match (species `i` ↔
`n[i]` ↔ `stoich[i]`).

# Units / ranges
- `charge` — signed ionic charge number `z` \[-\] (`0` for neutral/solid).
- `molar_mass` — molar mass `M` \[kg/mol\], finite and `> 0`.

```rust
pub struct SvleSpecies {
    pub name: String,
    pub charge: i32,
    pub molar_mass: f64,
    pub role: SpeciesRole,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-readable name (identification only, e.g. `"Water"`, `"H+"`). |
| `charge` | `i32` | Signed charge number `z_i` \[-\]. `0` for neutral molecules and solids. |
| `molar_mass` | `f64` | Molar mass `M_i` \[kg/mol\], finite and `> 0`. |
| `role` | `SpeciesRole` | Activity-scale role (see [`SpeciesRole`]). |

##### Implementations

###### Methods

- ```rust
  pub fn solvent</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, molar_mass: f64) -> Self { /* ... */ }
  ```
  Neutral **solvent** species (e.g. water) of molar mass `molar_mass`

- ```rust
  pub fn neutral</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, molar_mass: f64) -> Self { /* ... */ }
  ```
  Neutral (uncharged) **solute** species of molar mass `molar_mass`

- ```rust
  pub fn ion</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, charge: i32, molar_mass: f64) -> Self { /* ... */ }
  ```
  **Ion** of signed charge `charge` \[-\] and molar mass `molar_mass`

- ```rust
  pub fn solid</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, molar_mass: f64) -> Self { /* ... */ }
  ```
  Precipitated **solid** species of molar mass `molar_mass` \[kg/mol\]

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SvleSpecies { /* ... */ }
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
    fn eq(self: &Self, other: &SvleSpecies) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `EquilibriumReaction`

A single equilibrium reaction over the system's species, in the
reaction-extent formulation (`ElectrolyteSVLE.vb:301-318` builds the
stoichiometry matrix `E`; `ElectrolyteSVLE.vb:672-675` reads `K`).

The mass-action law satisfied at equilibrium is
`K = Π_s a_s^{stoich_s}` (products have `stoich > 0`, reactants `< 0`).

# Units / ranges
- `stoich` — stoichiometric coefficients `ν_s` \[-\], length = species count.
- `k` — equilibrium constant `K` \[-\] **at the system temperature**
  (DWSIM's `rxn.EvaluateK(T)`; the caller evaluates `K(T)` and passes the
  value). Must be finite and `> 0`.

```rust
pub struct EquilibriumReaction {
    pub stoich: Vec<f64>,
    pub k: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stoich` | `Vec<f64>` | Stoichiometric coefficients `ν_s` \[-\], indexed to the species list;<br>negative for reactants, positive for products. |
| `k` | `f64` | Equilibrium constant `K` \[-\] at the system temperature, `> 0`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(stoich: Vec<f64>, k: f64) -> Self { /* ... */ }
  ```
  Construct from a stoichiometry vector and equilibrium constant `K` \[-\].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> EquilibriumReaction { /* ... */ }
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
    fn eq(self: &Self, other: &EquilibriumReaction) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SvleActivity`

Activity-coefficient model for the speciation solve (enum dispatch, no `dyn`).

DWSIM's `FunctionValue2N` evaluates the reaction quotient with **unit**
activity coefficients (`activcoeff = RET_UnitaryVector()`,
`ElectrolyteSVLE.vb:628`), i.e. [`SvleActivity::Ideal`]. The
[`SvleActivity::DebyeHuckel`] variant is an offered extension that adds the
long-range ion term from [`crate::thermo::electrolyte`] (documented as beyond
the literal DWSIM path).

```rust
pub enum SvleActivity {
    Ideal,
    DebyeHuckel(crate::thermo::electrolyte::DebyeHuckel),
}
```

##### Variants

###### `Ideal`

Ideal: `γ_s = 1` for every species (matches DWSIM's `FunctionValue2N`).

###### `DebyeHuckel`

Debye-Hückel long-range term for ions (`γ_neutral = γ_solvent = 1`),
evaluated at the liquid ionic strength via
[`crate::thermo::electrolyte::DebyeHuckel::ln_gamma_ion`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::thermo::electrolyte::DebyeHuckel` |  |

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
    fn clone(self: &Self) -> SvleActivity { /* ... */ }
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
    fn eq(self: &Self, other: &SvleActivity) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SvleOptions`

Tuning parameters for [`SvleSystem::solve_speciation`].

# Units / ranges
- `max_iter` — maximum solver iterations (DWSIM `MaximumIterations = 100`,
  `ElectrolyteSVLE.vb:60`).
- `tol` — convergence tolerance on `max_i |ln(K_i / Q_i)|` \[-\].
- `n_floor` — smallest mole amount \[mol\] any species is allowed to reach
  during the solve (feasibility clamp keeping activities finite/positive).

```rust
pub struct SvleOptions {
    pub max_iter: usize,
    pub tol: f64,
    pub n_floor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iter` | `usize` | Maximum solver iterations. |
| `tol` | `f64` | Convergence tolerance on the max absolute log-residual \[-\]. |
| `n_floor` | `f64` | Minimum species mole amount during the solve \[mol\]. |

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
    fn clone(self: &Self) -> SvleOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &SvleOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SvleResult`

Converged result of a speciation solve.

# Units
- `n` — equilibrium mole amounts per species \[mol\].
- `extents` — reaction extents `ξ_j` \[mol\].
- `x_liquid` — liquid-phase mole fractions \[-\] over the **non-solid**
  species (sum to 1); solid entries are 0.
- `ionic_strength` — `I = ½ Σ_ion z² m` \[mol/kg\].
- `net_charge_molality` — `Σ_ion z · m` \[mol/kg\] (0 for a neutral solution).
- `residual` — final `max_i |ln(K_i / Q_i)|` \[-\].
- `iterations` — solver iterations performed.

```rust
pub struct SvleResult {
    pub n: Vec<f64>,
    pub extents: Vec<f64>,
    pub x_liquid: Vec<f64>,
    pub ionic_strength: f64,
    pub net_charge_molality: f64,
    pub residual: f64,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `Vec<f64>` | Equilibrium mole amounts per species \[mol\]. |
| `extents` | `Vec<f64>` | Reaction extents `ξ_j` \[mol\]. |
| `x_liquid` | `Vec<f64>` | Liquid-phase mole fractions over non-solid species \[-\]. |
| `ionic_strength` | `f64` | Ionic strength `I` \[mol/kg\]. |
| `net_charge_molality` | `f64` | Net charge molality `Σ z·m` \[mol/kg\]. |
| `residual` | `f64` | Final max absolute log-residual \[-\]. |
| `iterations` | `usize` | Solver iterations performed. |

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
    fn clone(self: &Self) -> SvleResult { /* ... */ }
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
    fn eq(self: &Self, other: &SvleResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SvleError`

Error conditions for [`SvleSystem::solve_speciation`].

```rust
pub enum SvleError {
    Empty,
    LengthMismatch {
        expected: usize,
        got: usize,
    },
    NonFinite,
    InvalidK {
        index: usize,
        k: f64,
    },
    NotConverged {
        iterations: usize,
        residual: f64,
    },
}
```

##### Variants

###### `Empty`

The system has no species.

###### `LengthMismatch`

A slice length did not match the species count.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `expected` | `usize` | Expected length (species count). |
| `got` | `usize` | Actual length received. |

###### `NonFinite`

A non-finite value appeared in an input or during the solve.

###### `InvalidK`

An equilibrium constant was non-positive or non-finite.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | Reaction index. |
| `k` | `f64` | Offending constant. |

###### `NotConverged`

The solver did not converge within `max_iter`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations performed. |
| `residual` | `f64` | Final max absolute log-residual. |

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
    fn clone(self: &Self) -> SvleError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SvleError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SvleSystem`

An aqueous electrolyte system: an ordered species list plus its equilibrium
reaction set. The chemical-equilibrium half of DWSIM's `ElectrolyteSVLE`.

Composition enters per solve as an initial mole-amount vector `n0` \[mol\]
whose length must equal the species count.

```rust
pub struct SvleSystem {
    pub species: Vec<SvleSpecies>,
    pub reactions: Vec<EquilibriumReaction>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `species` | `Vec<SvleSpecies>` | Species in index order. |
| `reactions` | `Vec<EquilibriumReaction>` | Equilibrium reactions; each `stoich` has length = species count. |

##### Implementations

###### Methods

- ```rust
  pub fn new(species: Vec<SvleSpecies>, reactions: Vec<EquilibriumReaction>) -> Result<Self, SvleError> { /* ... */ }
  ```
  Build a system from a species list and reaction set. Reactions may be

- ```rust
  pub fn n_species(self: &Self) -> usize { /* ... */ }
  ```
  Number of species `n`.

- ```rust
  pub fn n_reactions(self: &Self) -> usize { /* ... */ }
  ```
  Number of reactions `r`.

- ```rust
  pub fn net_charge_molality(self: &Self, n: &[f64]) -> f64 { /* ... */ }
  ```
  Net charge molality `Σ_ion z · m` \[mol/kg\] from mole amounts `n`

- ```rust
  pub fn solve_speciation(self: &Self, n0: &[f64], act: &SvleActivity, opts: SvleOptions) -> Result<SvleResult, SvleError> { /* ... */ }
  ```
  Solve the reaction-set chemical equilibria in the liquid phase for the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SvleSystem { /* ... */ }
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
    fn eq(self: &Self, other: &SvleSystem) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SaltSolubility`

Solubility-product model for one ionic solid `C_{ν+} A_{ν-} (s) ⇌ ν₊ C + ν₋ A`
— the Ksp piece coupling speciation to solid precipitation
(the ionic-solid analogue of the `nl3.Flash_SL` split DWSIM's outer loop
calls at `ElectrolyteSVLE.vb:191`).

# Units / ranges
- `ksp` — solubility product `Ksp` \[(mol/kg)^(ν₊+ν₋)\], `> 0`.
- `nu_cation`, `nu_anion` — dissolution stoichiometry `ν₊`, `ν₋` \[-\], `> 0`.

```rust
pub struct SaltSolubility {
    pub ksp: f64,
    pub nu_cation: f64,
    pub nu_anion: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ksp` | `f64` | Solubility product `Ksp` \[(mol/kg)^(ν₊+ν₋)\], `> 0`. |
| `nu_cation` | `f64` | Cation dissolution stoichiometry `ν₊` \[-\], `> 0`. |
| `nu_anion` | `f64` | Anion dissolution stoichiometry `ν₋` \[-\], `> 0`. |

##### Implementations

###### Methods

- ```rust
  pub fn one_to_one(ksp: f64) -> Self { /* ... */ }
  ```
  A 1:1 salt (`ν₊ = ν₋ = 1`, e.g. AgCl, NaCl) of solubility product `ksp`

- ```rust
  pub fn ion_product(self: &Self, a_cation: f64, a_anion: f64) -> f64 { /* ... */ }
  ```
  Ion activity product `IAP = (a_C)^{ν₊} (a_A)^{ν₋}` \[same units as Ksp\]

- ```rust
  pub fn is_supersaturated(self: &Self, a_cation: f64, a_anion: f64) -> bool { /* ... */ }
  ```
  `true` if the solution is **supersaturated** (`IAP > Ksp`), i.e. the

- ```rust
  pub fn solubility(self: &Self) -> f64 { /* ... */ }
  ```
  Saturation solubility `s` \[mol/kg\] of the pure salt dissolving into

- ```rust
  pub fn precipitate_one_to_one(self: &Self, m_cation: f64, m_anion: f64) -> (f64, f64, f64) { /* ... */ }
  ```
  Precipitate a **1:1** salt from a supersaturated solution: find the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SaltSolubility { /* ... */ }
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
    fn eq(self: &Self, other: &SaltSolubility) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `energy_flash`

Isenthalpic (PH) / energy flash: solve the temperature `T` at which a
mixture's total molar enthalpy equals a target `H`, at fixed pressure `P`.

Ported/composed from **DWSIM** (GPL-3.0). The reference is the PH-flash idea
in DWSIM's flash algorithms:
- `DWSIM.Thermodynamics/FlashAlgorithms/BaseFlashAlgorithm.vb` `Flash_PH`
  (the generic energy-flash driver template), and
- `DWSIM.Thermodynamics/FlashAlgorithms/NestedLoops.vb` `Flash_PH`
  (the concrete nested-loops implementation): an outer temperature loop that
  drives `H(T) − H_target → 0` with an inner TP (pressure-temperature) flash
  supplying the phase split at each candidate `T`, using the mixture heat
  capacity as the Newton derivative.

## The physics this module implements

At a candidate temperature `T` and the fixed pressure `P`, the mixture's
total molar enthalpy is the ideal-gas part plus the phase-weighted real-fluid
departures:

```text
H(T,P) = H_ideal_mix(T)                         (Σ_i z_i ∫_{T_ref}^{T} Cp0_i dT)
       + β · H_dep(y, T, P, vapour)             (β   = vapour molar fraction)
       + (1 − β) · H_dep(x, T, P, liquid)
```

- The **ideal-gas part** is [`crate::thermo::ideal_props::mixture_ideal_gas_enthalpy`]
  evaluated at the *feed* composition `z`. The ideal-gas enthalpy of mixing
  is zero and ideal-gas enthalpy is pressure-independent, so the phase split
  does not change this term — moles are conserved across the split.
- The **departures** are [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]
  evaluated at each phase's own composition (`y` for vapour, `x` for liquid)
  and weighted by that phase's molar fraction.

## Decoupling (the crate's push-to-caller pattern, no `dyn`)

The two model-dependent steps are **caller-supplied generic `Fn` closures**,
never trait objects — mirroring [`crate::thermo::flash::nested_loops_flash`]'s
K-value closure and the `expander::isentropic` pattern, and obeying the
workspace no-`Box<dyn>`/no-`dyn` rule:

- a **TP-flash closure** `Fn(T, P) -> FlashResult` gives the phase split
  `(β, x, y)` at `(T, P)` — this is where the property model (fugacity /
  activity) enters, so keeping it a closure keeps this module independent of
  any particular EOS/activity choice;
- an **enthalpy-departure closure** `Fn(&[f64], T, P, Phase) -> f64` returns
  the molar enthalpy departure `[J/mol]` of a phase of the given composition.
  The intended argument wraps [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]
  (`.unwrap_or(0.0)` for the ideal-gas / no-root limit), but any real-fluid
  departure model satisfies the signature.

## Solver: safeguarded Newton / bisection

`H(T)` is monotonically increasing in `T` (the mixture `Cp > 0`), so
`f(T) = H(T) − H_target` has exactly one root. The driver:

1. **Brackets** the root outward from an initial guess using a
   `Cp`-scaled, geometrically-growing step, until a sign change of `f` is
   found (documented bounds `[t_min, t_max]`; failure → [`EnergyFlashError::NoBracket`]).
2. **Solves** inside the bracket with a Numerical-Recipes-style `rtsafe`
   **safeguarded Newton**: a Newton step `ΔT = f / (dH/dT)` with
   `dH/dT ≈ Cp_mix(T)` (the *ideal-gas* mixture Cp — the departure's
   temperature derivative is intentionally neglected, an approximation the
   bisection safeguard makes robust), falling back to bisection whenever the
   Newton step would leave the bracket or is not reducing the interval fast
   enough. This cannot diverge because the root stays bracketed throughout.

## Honest scope (verification, NOT benchmark validation)

- The inline tests are **verification** — they check the driver against
  hand-computed analytic cases (constant-`Cp` ideal gas), an internal
  round-trip, phase-weighting algebra, and monotonicity. They do **not**
  validate the enthalpy model against measured PH-flash data or the DWSIM
  reference outputs; that is benchmark validation and is future work.
- **Enthalpy model = ideal-gas Cp0 integral + cubic-EOS departure only.**
  No enthalpy-of-formation offsets are applied (consistent with
  [`crate::thermo::ideal_props`]'s *sensible*-enthalpy convention), so
  `H_target` must be expressed on that same sensible-enthalpy scale relative
  to `T_ref`. Mixing across the two product phases beyond the mole-weighted
  departures is not modelled.
- **Two-phase PH flash is supported** through the TP-flash closure's `β`,
  but the *quality* of the two-phase result is only as good as the supplied
  TP flash; this module owns the temperature root-find, not the VLE.
- **`flash_ps` (entropy / PS flash)** is implemented on the same skeleton
  with `dS/dT = Cp/T`, but its two-phase entropy is a documented
  approximation (see [`flash_ps`]); it is verified only for the single-phase
  case here.

```rust
pub mod energy_flash { /* ... */ }
```

### Types

#### Struct `EnergyFlashOptions`

Tuning parameters for the safeguarded Newton/bisection temperature solve
used by [`flash_ph`] and [`flash_ps`].

```rust
pub struct EnergyFlashOptions {
    pub max_iter: usize,
    pub value_tol: f64,
    pub temperature_tol: f64,
    pub t_min: f64,
    pub t_max: f64,
    pub bracket_expansions: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iter` | `usize` | Maximum safeguarded-Newton iterations inside the bracket before<br>returning [`EnergyFlashError::NotConverged`]. |
| `value_tol` | `f64` | Absolute convergence tolerance on the objective value<br>`|H(T) − H_target|` \[J/mol\] (for [`flash_ph`]) or<br>`|S(T) − S_target|` \[J/(mol·K)\] (for [`flash_ps`]). |
| `temperature_tol` | `f64` | Absolute convergence tolerance on the temperature step `|ΔT|` \[K\]. |
| `t_min` | `f64` | Lower bound of the temperature search \[K\] (bracket floor). |
| `t_max` | `f64` | Upper bound of the temperature search \[K\] (bracket ceiling). |
| `bracket_expansions` | `usize` | Maximum geometric bracket-expansion steps before giving up with<br>[`EnergyFlashError::NoBracket`]. |

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
    fn clone(self: &Self) -> EnergyFlashOptions { /* ... */ }
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
    Defaults sized for the verification tests: `1e-6` value tolerance,

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &EnergyFlashOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `EnergyFlashResult`

A converged energy-flash result (from [`flash_ph`] or [`flash_ps`]).

```rust
pub struct EnergyFlashResult {
    pub temperature: f64,
    pub vapour_fraction: f64,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub iterations: usize,
    pub residual: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `f64` | The converged temperature `T` \[K\] at which the mixture enthalpy<br>(or entropy) equals the target, at the fixed input pressure. |
| `vapour_fraction` | `f64` | Vapour molar fraction `β` \[-\] at the converged state, in `[0, 1]`:<br>`0` = all liquid, `1` = all vapour, interior = two coexisting phases.<br>Taken from the TP-flash closure evaluated at the converged `T`. |
| `x` | `Vec<f64>` | Liquid-phase mole fractions `x_i` \[-\] at the converged `T` (sum to 1). |
| `y` | `Vec<f64>` | Vapour-phase mole fractions `y_i` \[-\] at the converged `T` (sum to 1). |
| `iterations` | `usize` | Number of safeguarded-Newton iterations performed inside the bracket. |
| `residual` | `f64` | Achieved absolute residual of the objective at convergence:<br>`|H(T) − H_target|` \[J/mol\] for [`flash_ph`], or<br>`|S(T) − S_target|` \[J/(mol·K)\] for [`flash_ps`]. |

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
    fn clone(self: &Self) -> EnergyFlashResult { /* ... */ }
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
    fn eq(self: &Self, other: &EnergyFlashResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `EnergyFlashError`

Error conditions for the energy-flash routines.

```rust
pub enum EnergyFlashError {
    Empty,
    LengthMismatch {
        a: usize,
        b: usize,
    },
    NonFinite {
        temperature: f64,
    },
    NoBracket {
        t_min: f64,
        t_max: f64,
    },
    DegenerateDerivative {
        temperature: f64,
    },
    NotConverged {
        iterations: usize,
        residual: f64,
    },
}
```

##### Variants

###### `Empty`

An empty composition was supplied (need at least one component).

###### `LengthMismatch`

`components` and `z` were different lengths.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `usize` | Length of `components`. |
| `b` | `usize` | Length of `z`. |

###### `NonFinite`

A non-finite objective value (`NaN`/`inf`) was produced by the enthalpy
(or entropy) evaluation at a candidate temperature.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `f64` | The temperature at which the non-finite value appeared \[K\]. |

###### `NoBracket`

The root could not be bracketed within `[t_min, t_max]` in the allowed
number of expansions — the target lies outside the reachable enthalpy
range, or the objective is non-monotone within the window.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_min` | `f64` | Search-window floor \[K\]. |
| `t_max` | `f64` | Search-window ceiling \[K\]. |

###### `DegenerateDerivative`

The derivative `dH/dT` (or `dS/dT`) was ~0 at the initial guess, so no
Newton predictor could be formed (a zero-`Cp` mixture).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `f64` | The temperature at which the derivative vanished \[K\]. |

###### `NotConverged`

The safeguarded Newton loop did not reach tolerance in `max_iter`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations performed. |
| `residual` | `f64` | Final objective residual. |

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
    fn clone(self: &Self) -> EnergyFlashError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &EnergyFlashError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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

#### Function `mixture_enthalpy`

Total molar enthalpy `H(T, P)` \[J/mol\] of the mixture at temperature `T`
\[K\] and pressure `P` \[Pa\]: the ideal-gas part plus the phase-weighted
real-fluid enthalpy departures.

```text
H = Σ_i z_i ∫_{T_ref}^{T} Cp0_i dT
  + β · H_dep(y, T, P, Vapor) + (1 − β) · H_dep(x, T, P, Liquid)
```

The ideal-gas term uses the **feed** composition `z` (moles are conserved
across the phase split and ideal-gas enthalpy is composition-linear and
pressure-independent, so the split does not affect it). Each departure is
evaluated at its own phase composition and weighted by the phase's molar
fraction; a phase with zero fraction contributes nothing (its departure
closure is not called).

# Parameters
- `components`, `z`: pure compounds and their feed mole fractions \[-\]
  (same length; `z` normally sums to 1).
- `temperature` `T` \[K\] > 0, `pressure` `P` \[Pa\] > 0.
- `t_ref` `T_ref` \[K\]: the ideal-gas enthalpy reference temperature (the
  `H = 0` datum for the sensible-enthalpy scale).
- `tp_flash`: `Fn(T, P) -> FlashResult` — the phase split at `(T, P)`.
- `enthalpy_departure`: `Fn(&[f64], T, P, Phase) -> f64` — molar enthalpy
  departure \[J/mol\] of a phase of the given composition.

# Panics
Panics (via [`mixture_ideal_gas_enthalpy`]) if
`components.len() != z.len()`.

# Returns
Total molar enthalpy `H(T, P)` \[J/mol\] on the sensible-enthalpy scale
(no enthalpy-of-formation offset).

```rust
pub fn mixture_enthalpy<Flash, Dep>(components: &[crate::thermo::Component], z: &[f64], temperature: f64, pressure: f64, t_ref: f64, tp_flash: Flash, enthalpy_departure: Dep) -> f64
where
    Flash: Fn(f64, f64) -> crate::thermo::flash::FlashResult,
    Dep: Fn(&[f64], f64, f64, crate::thermo::cubic_eos::Phase) -> f64 { /* ... */ }
```

#### Function `mixture_entropy`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Total molar entropy `S(T, P)` \[J/(mol·K)\] of the mixture: the ideal-gas
entropy plus the phase-weighted real-fluid entropy departures.

```text
S = S_ideal_mix(z, T, P)                        (incl. −R ln(P/P_ref) and mixing)
  + β · S_dep(y, T, P, Vapor) + (1 − β) · S_dep(x, T, P, Liquid)
```

The ideal-gas term is [`crate::thermo::ideal_props::mixture_ideal_gas_entropy`]
at the **feed** composition and the total pressure.

**Two-phase caveat (documented approximation).** Unlike enthalpy, ideal-gas
entropy is *not* independent of composition or pressure, so evaluating the
ideal part at the feed composition and total pressure is exact only in the
single-phase limit (`β = 0` or `β = 1`). In the two-phase region it omits
the entropy of separating the feed into distinct-composition phases at their
partial pressures. Full two-phase entropy accounting is deferred; treat
[`flash_ps`] as verified for single-phase only.

# Parameters
As [`mixture_enthalpy`], plus `p_ref` `P_ref` \[Pa\] for the ideal-gas
pressure term, and an `entropy_departure` closure returning molar entropy
departure \[J/(mol·K)\].

# Panics
Panics if `components.len() != z.len()`.

# Returns
Total molar entropy `S(T, P)` \[J/(mol·K)\].

```rust
pub fn mixture_entropy<Flash, Dep>(components: &[crate::thermo::Component], z: &[f64], temperature: f64, pressure: f64, t_ref: f64, p_ref: f64, tp_flash: Flash, entropy_departure: Dep) -> f64
where
    Flash: Fn(f64, f64) -> crate::thermo::flash::FlashResult,
    Dep: Fn(&[f64], f64, f64, crate::thermo::cubic_eos::Phase) -> f64 { /* ... */ }
```

#### Function `flash_ph`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Isenthalpic (PH) flash: find the temperature `T` \[K\] at which the mixture's
total molar enthalpy equals `h_target` \[J/mol\], at the fixed pressure `P`.

Drives `f(T) = H(T, P) − H_target → 0` (with [`mixture_enthalpy`]) by the
safeguarded Newton/bisection scheme documented at the module level:
bracket outward with a `Cp`-scaled growing step, then `rtsafe` inside the
bracket using `dH/dT ≈ Cp_mix(T)` (ideal-gas mixture heat capacity) as the
Newton derivative, with bisection safeguarding every step so it cannot
diverge.

# Parameters
- `components`, `z`: pure compounds and feed mole fractions \[-\]
  (same non-zero length).
- `pressure` `P` \[Pa\] > 0 (held fixed).
- `h_target` `H_target` \[J/mol\]: the target total molar enthalpy on the
  sensible-enthalpy scale relative to `t_ref` (no formation offset).
- `t_ref` `T_ref` \[K\]: ideal-gas enthalpy reference (the `H = 0` datum).
- `t_initial` \[K\]: initial temperature guess (must satisfy
  `t_min ≤ t_initial ≤ t_max`); the bracket is grown outward from here.
- `tp_flash`: `Fn(T, P) -> FlashResult` — phase split at each candidate `T`.
- `enthalpy_departure`: `Fn(&[f64], T, P, Phase) -> f64` — molar enthalpy
  departure \[J/mol\] (wrap [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]
  with `.unwrap_or(0.0)`).
- `opts`: solver tolerances and bounds ([`EnergyFlashOptions`]).

# Errors
- [`EnergyFlashError::Empty`] / [`EnergyFlashError::LengthMismatch`] for a
  malformed composition.
- [`EnergyFlashError::NoBracket`] if the target enthalpy is unreachable in
  `[t_min, t_max]`.
- [`EnergyFlashError::DegenerateDerivative`] if `Cp ~ 0` at `t_initial`.
- [`EnergyFlashError::NonFinite`] / [`EnergyFlashError::NotConverged`] on a
  non-finite evaluation or failure to reach tolerance in `max_iter`.

# Returns
The converged [`EnergyFlashResult`] (temperature, vapour fraction, phase
compositions, iteration count, and achieved enthalpy residual).

```rust
pub fn flash_ph<Flash, Dep>(components: &[crate::thermo::Component], z: &[f64], pressure: f64, h_target: f64, t_ref: f64, t_initial: f64, tp_flash: Flash, enthalpy_departure: Dep, opts: EnergyFlashOptions) -> Result<EnergyFlashResult, EnergyFlashError>
where
    Flash: Fn(f64, f64) -> crate::thermo::flash::FlashResult,
    Dep: Fn(&[f64], f64, f64, crate::thermo::cubic_eos::Phase) -> f64 { /* ... */ }
```

#### Function `flash_ps`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Isentropic-target (PS) flash: find the temperature `T` \[K\] at which the
mixture's total molar entropy equals `s_target` \[J/(mol·K)\], at fixed `P`.

Same safeguarded Newton/bisection skeleton as [`flash_ph`], driving
`S(T, P) − S_target → 0` (with [`mixture_entropy`]) using the ideal-gas
derivative `dS/dT = Cp_mix(T) / T`.

**Scope / honesty.** Because [`mixture_entropy`]'s two-phase term is a
documented approximation (it evaluates the ideal entropy at the feed
composition and total pressure, omitting the entropy of inter-phase
separation), this routine is **verified only for the single-phase case**.
Two-phase PS flash with rigorous entropy accounting is deferred future work.

# Parameters
As [`flash_ph`], but `s_target` \[J/(mol·K)\] and an additional
`p_ref` `P_ref` \[Pa\] for the ideal-gas pressure term, and an
`entropy_departure` closure returning molar entropy departure
\[J/(mol·K)\] (wrap [`crate::thermo::cubic_eos::CubicEos::entropy_departure`]).

# Errors
As [`flash_ph`] (the entropy analogue of each condition).

# Returns
The converged [`EnergyFlashResult`]; its `residual` is `|S(T) − S_target|`
\[J/(mol·K)\].

```rust
pub fn flash_ps<Flash, Dep>(components: &[crate::thermo::Component], z: &[f64], pressure: f64, s_target: f64, t_ref: f64, p_ref: f64, t_initial: f64, tp_flash: Flash, entropy_departure: Dep, opts: EnergyFlashOptions) -> Result<EnergyFlashResult, EnergyFlashError>
where
    Flash: Fn(f64, f64) -> crate::thermo::flash::FlashResult,
    Dep: Fn(&[f64], f64, f64, crate::thermo::cubic_eos::Phase) -> f64 { /* ... */ }
```

## Module `eos_variants`

Cubic-EOS refinements: PRSV α-function + Peneloux volume translation.

Two independent, additive refinements layered on top of the base
Peng-Robinson / SRK kernel in [`crate::thermo::cubic_eos`] (which this module
reads but never edits):

1. **PRSV α-function** — the Stryjek-Vera (1986) modification of the PR
   attraction temperature-dependence. It refines *only* the per-component
   attraction `a_i(T)`; the co-volume `b_i`, the van der Waals one-fluid
   mixing rule, the compressibility cubic, the fugacity coefficient, and the
   enthalpy/entropy departures are all **unchanged**. So a PRSV run is a base
   PR run with `a_i(T)` swapped — see [`prsv_a_i`] / [`prsv_a_mix`] and the
   "Composing with `cubic_eos`" note below.
2. **Peneloux volume translation** — a constant per-component molar-volume
   shift `c_i` (Peneloux et al. 1982) subtracted from the EOS molar volume:
   `v = v_EOS − c`. It improves the predicted *liquid density* without
   changing VLE: the shift is identical in every phase, so it **cancels
   exactly in K-values / fugacity ratios** (proven numerically in the tests).

## Ported from DWSIM (GPL-3.0), Visual-Basic reference source

- **PRSV κ / α**: `PropertyPackages/Models/PRSV2-VL.vb` L130-141 (and the
  identical block repeated at L346-357, L528-539, L718-729, L1031-1033,
  L1253-1264). DWSIM implements the more general **PRSV2** three-parameter
  form; setting its `κ2 = κ3 = 0` collapses it to the one-parameter **PRSV**
  ported here. Selector/data plumbing:
  `PropertyPackages/PengRobinsonStryjekVera2VL.vb` L485 (`kappa1`).
- **Peneloux translation**: applied in
  `PropertyPackages/SoaveRedlichKwong.vb` L433 (`Z −= AUX_CM/(RT)·P`) and
  L1048/L1179 (`v −= AUX_CM`); mixture `c = Σ zᵢ cᵢ` in
  `PropertyPackages/PengRobinson.vb` L565-604 and
  `SoaveRedlichKwong.vb` L144-170 (`AUX_CM`). The default Rackett
  compressibility `Z_RA = 0.29056 − 0.08775 ω` used when no tabulated value
  exists is `PropertyPackages/PropertyPackage.vb` L5604 (also
  `Models/FluidProperties.vb` L314).

## Two forms of the Peneloux coefficient — which one this port uses

DWSIM's PR/SRK-Peneloux packages actually store `cᵢ = γᵢ · bᵢ` with `γᵢ` a
**tabulated dimensionless coefficient** per compound (`AUX_Ci`,
`PengRobinson.vb` L592-…). This port instead uses the **original
Rackett-based Peneloux (1982) correlation**

`cᵢ = 0.40768 (R Tc / Pc)(0.29441 − Z_RA)`,  `Z_RA = 0.29056 − 0.08775 ω`,

because it needs no per-compound translation table (deferred, same as the
PRSV κ1 table) and is fully determined by the critical constants + acentric
factor already carried by [`Component`]. DWSIM uses this exact `Z_RA`
default (see citation above). Both forms produce a small positive `cᵢ` for
normal fluids and share the identical VLE-invariance property.

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

Temperature K, pressure Pa, molar volume m³/mol, the translation `cᵢ` m³/mol,
the EOS attraction `a` J·m³/mol² (= Pa·m⁶/mol²). `κ0`, `κ1`, `α`, `Z_RA`,
mole fractions `z`, and `Tr = T/Tc` are dimensionless. Raw `f64` matches the
base kernel; every public signature spells out its units.

## Design (crate `CLAUDE.md`)

No `Box`/`dyn`, no lifetimes, no channels. These refinements are exposed as
**free functions** rather than an enum variant of [`CubicEos`]: the PRSV κ1
is a *per-compound* datum (an array over the mixture), which an EOS-selector
enum carrying no per-component state cannot hold cleanly. Free functions
compose with the existing [`CubicEos`] kernel without duplicating its Z-solve
or fugacity code (see below), so no enum is warranted here.

## Composing with `cubic_eos`

PRSV changes one thing: `a_i(T)`. To run a full PRSV flash you reuse the base
kernel unchanged and only substitute the attraction:

```text
b_mix = CubicEos::PengRobinson.b_mix(comps, z)         // unchanged
a_mix = prsv_a_mix(comps, kappa1, z, T, kij)           // refined attraction
A = a_mix · P / (R T)²;  B = b_mix · P / (R T)
Z = CubicEos::PengRobinson.z_roots(A, B) → phase-select // unchanged solver
v_EOS = Z R T / P
v = corrected_molar_volume(v_EOS, comps, z)            // Peneloux density fix
```

The fugacity-coefficient and departure expressions in [`CubicEos`] take the
same `(a_mix, b_mix)` and are algebraically identical for PRSV — only the
numeric `a_mix` fed in differs.

## Honest scope — what is and is NOT here

- **Verification, not validation.** The tests below check the equations are
  implemented correctly against hand-computed values and the published
  constants (Stryjek & Vera 1986; Peneloux et al. 1982), **not** against
  experimental VLE / liquid-density benchmarks. The Peneloux liquid-density
  test shows the shift *moves density in the right direction*, not that it
  hits an experimental datum.
- **PRSV one-parameter κ1 only.** The PRSV2 second/third parameters
  (`κ2, κ3`) and other α-forms (Mathias-Copeman, Twu, PC-SAFT) are out of
  scope. With `κ1 = 0` the α-function reduces to the PRSV κ0(ω) correlation
  — *close to* but not identical to the classic 1976 PR κ(ω).
- **κ1 data table deferred.** `κ1` is a fitted per-compound constant; this
  port takes it as an input argument (default `0.0`) rather than shipping a
  compound→κ1 table.
- **Peneloux Rackett form only.** The tabulated `γᵢ·bᵢ` DWSIM variant is not
  ported (needs the deferred translation table).

> **⚠️ Unverified until validated.** Early-stage AI-assisted translation.
> Not for nuclear facility operation, reactor control, safety-critical, or
> licensing decisions. Independent OUTRAM PARK fork, not the official DWSIM.

```rust
pub mod eos_variants { /* ... */ }
```

### Functions

#### Function `prsv_kappa0`

**Attributes:**

- `MustUse { reason: None }`

PRSV `κ0(ω)` — the acentric-factor part of the PRSV α-slope [-].

`κ0 = 0.378893 + 1.4897153 ω − 0.17131848 ω² + 0.0196554 ω³`
(Stryjek & Vera 1986; DWSIM `PRSV2-VL.vb` L130). `ω` is the Pitzer acentric
factor [-]. This is the PRSV refit of the PR α-slope; it is *close to* but
not identical to the classic 1976 PR `κ(ω) = 0.37464 + 1.54226 ω −
0.26992 ω²`. Valid for the usual `−0.1 ≲ ω ≲ 1` range.

```rust
pub fn prsv_kappa0(acentric_factor: f64) -> f64 { /* ... */ }
```

#### Function `prsv_kappa`

**Attributes:**

- `MustUse { reason: None }`

Full PRSV temperature-dependent α-slope `κ(T)` [-] for a component.

`κ = κ0 + κ1 (1 + √Tr)(0.7 − Tr)`, with `Tr = T/Tc` [-] and `κ0` from
[`prsv_kappa0`] (Stryjek & Vera 1986, Eq. 7; DWSIM `PRSV2-VL.vb` L130 with
the PRSV2 `κ2 = κ3 = 0`). `kappa1` [-] is the compound-specific fitted
parameter (default `0.0` → pure κ0). `t` [K] must be `> 0`. The `(0.7 − Tr)`
factor makes the κ1 correction vanish at `Tr = 0.7` and change sign across
it, the reduced temperature at which Stryjek & Vera anchored the fit.

```rust
pub fn prsv_kappa(comp: &crate::thermo::Component, kappa1: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `prsv_alpha`

**Attributes:**

- `MustUse { reason: None }`

PRSV α-function `α(T) = [1 + κ(1 − √Tr)]²` [-] for a component.

The temperature scaling of the PR attraction under the PRSV κ (DWSIM
`PRSV2-VL.vb` L131). `κ` is [`prsv_kappa`]; `t` [K] must be `> 0`;
`Tr = T/Tc`. At the critical point (`Tr = 1`) `α = 1` **exactly**, for any
`κ1`, because `(1 − √1) = 0`. With `κ1 = 0` this reduces to the PRSV κ0(ω)
α-function, numerically close to the base PR α from
[`CubicEos::alpha`].

```rust
pub fn prsv_alpha(comp: &crate::thermo::Component, kappa1: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `prsv_a_i`

**Attributes:**

- `MustUse { reason: None }`

PRSV pure-component attraction `a_i(T) = 0.45724 · α_PRSV(T) · R² Tc² / Pc`
[J·m³/mol²] at `t` [K].

Identical to [`CubicEos::a_i`] for Peng-Robinson except the α-function is the
PRSV [`prsv_alpha`] instead of the base PR α (DWSIM `PRSV2-VL.vb` L140 uses
the same `Ωa = 0.45724`). The co-volume `b_i` is **unchanged** — obtain it
from `CubicEos::PengRobinson.b_i(comp)`. Valid for `t > 0`.

```rust
pub fn prsv_a_i(comp: &crate::thermo::Component, kappa1: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `prsv_a_mix`

**Attributes:**

- `MustUse { reason: None }`

PRSV mixture attraction `a_mix = Σ_i Σ_j z_i z_j √(a_i a_j)(1 − k_ij)`
[J·m³/mol²] at `t` [K], using the PRSV pure-component `a_i` from
[`prsv_a_i`].

This is the **identical** van der Waals one-fluid mixing rule as
[`CubicEos::a_mix`] — only the per-component `a_i` differs (PRSV vs base PR).
`kappa1` [-] is the per-component array (same length as `comps` and `z`);
pass all-zeros for the pure-κ0 PRSV. `kij = None` uses the geometric-mean
rule. The mixture co-volume `b_mix` is unchanged: use
`CubicEos::PengRobinson.b_mix(comps, z)`.

# Panics
Panics (via slice indexing) if `comps`, `kappa1`, and `z` differ in length.

```rust
pub fn prsv_a_mix(comps: &[crate::thermo::Component], kappa1: &[f64], z: &[f64], t: f64, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> f64 { /* ... */ }
```

#### Function `rackett_z_ra`

**Attributes:**

- `MustUse { reason: None }`

Rackett compressibility `Z_RA = 0.29056 − 0.08775 ω` [-] for a component.

The Yamada-Gunn estimate of the Rackett parameter from the acentric factor,
used as the DWSIM default when no tabulated `Z_RA` exists
(`PropertyPackage.vb` L5604). Dimensionless; typically `0.24–0.29` for normal
fluids.

```rust
pub fn rackett_z_ra(comp: &crate::thermo::Component) -> f64 { /* ... */ }
```

#### Function `peneloux_shift`

**Attributes:**

- `MustUse { reason: None }`

Peneloux per-component volume-translation shift `c_i` [m³/mol].

`c_i = 0.40768 (R Tc / Pc)(0.29441 − Z_RA)` with `Z_RA` from
[`rackett_z_ra`] (Peneloux, Rauzy & Fréze 1982). `c_i` is a **constant** —
independent of `T`, `P`, and composition — which is exactly why it cancels in
K-values (see [`peneloux_lnphi_shift`]). For a normal fluid
`Z_RA < 0.29441`, so `c_i > 0`: the translation *reduces* the EOS molar
volume, correcting PR/SRK's systematic liquid-volume overprediction. Units:
`R Tc / Pc` is m³/mol, the parenthesised factor is dimensionless.

```rust
pub fn peneloux_shift(comp: &crate::thermo::Component) -> f64 { /* ... */ }
```

#### Function `peneloux_c_mix`

**Attributes:**

- `MustUse { reason: None }`

Mixture volume-translation `c = Σ z_i c_i` [m³/mol].

Linear (mole-fraction) mixing of the pure-component shifts, matching DWSIM's
`AUX_CM` (`PengRobinson.vb` L580-590, `SoaveRedlichKwong.vb` L159-169).
`comps` and mole fractions `z` [-] must have equal length (a mismatch is
silently truncated by the `zip`, so the caller must pass equal-length
slices); `z` should sum to 1.

```rust
pub fn peneloux_c_mix(comps: &[crate::thermo::Component], z: &[f64]) -> f64 { /* ... */ }
```

#### Function `corrected_molar_volume`

**Attributes:**

- `MustUse { reason: None }`

Peneloux-corrected molar volume `v = v_EOS − c` [m³/mol].

Subtracts the mixture translation [`peneloux_c_mix`] from the untranslated
cubic-EOS molar volume `v_eos` [m³/mol] (which the caller forms as
`Z R T / P` from the base [`CubicEos`] solve). Since `c > 0` for normal
fluids, `v < v_eos`, so the predicted **liquid density** `ρ = M/v`
**increases** toward experiment. Applying it to a vapour molar volume is
harmless (the relative shift is negligible at low density). DWSIM applies the
same subtraction at `SoaveRedlichKwong.vb` L1048/L1179.

```rust
pub fn corrected_molar_volume(v_eos: f64, comps: &[crate::thermo::Component], z: &[f64]) -> f64 { /* ... */ }
```

#### Function `peneloux_lnphi_shift`

**Attributes:**

- `MustUse { reason: None }`

Fugacity-coefficient shift from the Peneloux translation, `c_i P / (R T)`
[-], for one component at `t` [K], `p` [Pa].

Under a constant volume translation the fugacity coefficient transforms as
`ln φ_i^translated = ln φ_i^EOS − c_i P/(R T)` (Peneloux et al. 1982). This
returns the subtracted term. Because `c_i` and `P, T` are the **same in the
liquid and the vapour**, the term is identical in both phases and **cancels
exactly** in the K-value `K_i = φ_i^L / φ_i^V`:

`ln K_i^translated = (ln φ_i^L − c_iP/RT) − (ln φ_i^V − c_iP/RT) = ln K_i^EOS`.

Hence the Peneloux shift improves liquid density **without altering VLE**.
Verified numerically in the tests.

```rust
pub fn peneloux_lnphi_shift(comp: &crate::thermo::Component, t: f64, p: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `PRSV_KAPPA0_C0`

PRSV κ0 polynomial constant term [-] (Stryjek & Vera 1986, Eq. 8).

```rust
pub const PRSV_KAPPA0_C0: f64 = 0.378893;
```

#### Constant `PRSV_KAPPA0_C1`

PRSV κ0 linear coefficient (in ω) [-].

```rust
pub const PRSV_KAPPA0_C1: f64 = 1.4897153;
```

#### Constant `PRSV_KAPPA0_C2`

PRSV κ0 quadratic coefficient (in ω²) [-].

```rust
pub const PRSV_KAPPA0_C2: f64 = 0.17131848;
```

#### Constant `PRSV_KAPPA0_C3`

PRSV κ0 cubic coefficient (in ω³) [-].

The published Stryjek-Vera (1986) value is `0.0196554`; DWSIM's source
(`PRSV2-VL.vb` L130) carries `0.0196544` — a last-digit transcription
difference. This port uses the **published** `0.0196554`; the resulting κ0
differs from DWSIM by `< 1e-8` for `ω ≲ 1`, far below any physical
significance.

```rust
pub const PRSV_KAPPA0_C3: f64 = 0.0196554;
```

#### Constant `PENELOUX_RACKETT_PREFACTOR`

Peneloux Rackett prefactor `0.40768` [-] (Peneloux et al. 1982, Eq. 8).

```rust
pub const PENELOUX_RACKETT_PREFACTOR: f64 = 0.40768;
```

#### Constant `PENELOUX_RACKETT_OFFSET`

Peneloux Rackett offset `0.29441` [-] (Peneloux et al. 1982, Eq. 8).

```rust
pub const PENELOUX_RACKETT_OFFSET: f64 = 0.29441;
```

#### Constant `RACKETT_ZRA_C0`

Rackett `Z_RA` correlation constant term `0.29056` [-]
(DWSIM `PropertyPackage.vb` L5604; Yamada-Gunn form).

```rust
pub const RACKETT_ZRA_C0: f64 = 0.29056;
```

#### Constant `RACKETT_ZRA_C1`

Rackett `Z_RA` acentric-factor coefficient `0.08775` [-]
(DWSIM `PropertyPackage.vb` L5604).

```rust
pub const RACKETT_ZRA_C1: f64 = 0.08775;
```

## Module `flash`

Isothermal-isobaric (**TP**) two-phase vapour-liquid-equilibrium flash via the
Rachford-Rice / Nested-Loops method, with Wilson K-value initialisation.

Ported from DWSIM `DWSIM.Thermodynamics/FlashAlgorithms/NestedLoops.vb`
(GPL-3.0), with the Wilson first-guess taken from
`DWSIM.Thermodynamics/PropertyPackages/PropertyPackage.vb`
(`DW_CalcKvalue_Ideal_Wilson`, lines 1650-1668). Specific ported lines are
cited at each function below.

# What this module computes

Given a feed of overall mole fractions `z_i` \[-\] at fixed temperature
`T` \[K\] and pressure `P` \[Pa\], split it into a liquid phase (mole
fractions `x_i`) and a vapour phase (mole fractions `y_i`) in equilibrium,
returning the **vapour molar fraction** `β` \[-\] (moles of vapour per mole
of feed, in `[0, 1]`). The equilibrium is expressed through the K-values
`K_i = y_i / x_i` \[-\], which come from a property model (fugacity /
activity coefficients).

The method has two nested levels:

1. **Inner** — [`solve_rachford_rice`]: for a *fixed* K-vector, solve the
   Rachford-Rice equation for `β`, then recover `x_i`, `y_i`.
2. **Outer** — [`nested_loops_flash`]: the "nested loops" successive-
   substitution driver. Start from the Wilson K-guess, solve Rachford-Rice,
   recompute the K-values from the resulting `(x, y)` via a caller-supplied
   closure, and repeat until the K-values stop moving.

The outer level is where the property model enters. Mirroring the crate's
`expander::isentropic` pattern, the one model-dependent step (turning a
trial `(x, y, T, P)` into updated K-values) is pushed to the caller as a
**generic `Fn` closure**, *not* a trait object — this file therefore has no
dependency on the EOS / activity modules and no `dyn` dispatch, per the
workspace `CLAUDE.md`.

# Rachford-Rice objective

```text
g(β) = Σ_i z_i (K_i − 1) / (1 + β (K_i − 1)) = 0
```

`g` is strictly decreasing in `β` between its adjacent poles, so it has at
most one root in any pole-free interval. For a physical two-phase split the
root lies in `[0, 1]`; the theoretical negative-flash window that always
brackets it is `[1/(1 − K_max), 1/(1 − K_min)]` (Whitson & Michelsen). The
compositions then follow directly:

```text
x_i = z_i / (1 + β (K_i − 1)),   y_i = K_i x_i.
```

With `β` solved so that `g(β) = 0`, both `Σ x_i = 1` and `Σ y_i = 1` hold
automatically, and the overall mass balance `z_i = (1 − β) x_i + β y_i`
holds *identically* for any `β` (it is how `x_i`, `y_i` are defined).

# Honest scope (verification, not benchmark validation)

This is the **isothermal-isobaric two-phase VLE core only**. The tests below
are *verification* against hand-computed / closed-form values and internal
consistency (mass balance, monotonicity) — they are **not** validation
against an experimental or NIST/DECHEMA benchmark. Deliberately **excluded**
(all present in the fuller DWSIM `NestedLoops.vb` and its siblings, out of
scope here):

- phase-stability analysis / trivial-solution rejection and the associated
  single-phase phase-identification (`AUX_CheckTrivial`, Gibbs-energy tie
  test, `AUX_Z` compressibility branch selection);
- three-phase (VLLE) and solid/salt-out flashes (`Flash_PT_3P`, `Flash_SVLE`);
- the inside-out / Boston-Britt acceleration and the Gibbs-minimisation and
  IPOPT convergence paths (`ConvergeVF2`, `Flash_PT_NL_IO`);
- the non-isothermal energy flashes — PH (enthalpy), PS (entropy), TV, PV —
  this module does the TP specification only.

The K-closure itself (fugacity / activity model) is *not* implemented here;
it is the caller's responsibility. AI-assisted port — untrusted draft
material until human-reviewed per the crate `CLAUDE.md`.

```rust
pub mod flash { /* ... */ }
```

### Types

#### Struct `FlashResult`

A converged (or best-effort) two-phase VLE flash result.

`x`, `y`, and the feed all use mole fractions \[-\]. `beta` is the vapour
molar fraction \[-\] in `[0, 1]`: `0.0` = all liquid (subcooled),
`1.0` = all vapour (superheated), interior = two coexisting phases.

```rust
pub struct FlashResult {
    pub beta: f64,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub k: Vec<f64>,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `beta` | `f64` | Vapour molar fraction `β` \[-\], moles vapour per mole feed, in `[0, 1]`. |
| `x` | `Vec<f64>` | Liquid-phase mole fractions `x_i` \[-\] (sum to 1). |
| `y` | `Vec<f64>` | Vapour-phase mole fractions `y_i` \[-\] (sum to 1). |
| `k` | `Vec<f64>` | K-values `K_i = y_i / x_i` \[-\] at the returned state. |
| `iterations` | `usize` | Number of completed outer nested-loops iterations. `0` for a bare<br>[`solve_rachford_rice`] call (no K-update was performed). |

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
    fn clone(self: &Self) -> FlashResult { /* ... */ }
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
    fn eq(self: &Self, other: &FlashResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `FlashError`

Error conditions for the flash routines.

```rust
pub enum FlashError {
    LengthMismatch {
        a: usize,
        b: usize,
    },
    Empty,
    NonFinite,
    NotConverged {
        iterations: usize,
        residual: f64,
    },
}
```

##### Variants

###### `LengthMismatch`

Two input slices that must be the same length were not.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `usize` | Length of the first slice (e.g. `z`). |
| `b` | `usize` | Length of the second slice (e.g. `k`). |

###### `Empty`

An empty composition was supplied (need at least one component).

###### `NonFinite`

A non-finite value (`NaN`/`inf`) appeared in an input `z` or `K`.

###### `NotConverged`

The outer nested-loops iteration did not converge within `max_outer_iter`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations attempted. |
| `residual` | `f64` | Final K-value change residual `Σ_i |K_i^new − K_i^old|`. |

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
    fn clone(self: &Self) -> FlashError { /* ... */ }
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: FlashError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: FlashError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FlashError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `NestedLoopsOptions`

Tuning parameters for [`nested_loops_flash`].

Defaults mirror the DWSIM `NestedLoops` tolerances closely enough for the
verification tests: a tight inner Rachford-Rice tolerance and a K-value
change tolerance for the outer successive-substitution loop.

```rust
pub struct NestedLoopsOptions {
    pub max_outer_iter: usize,
    pub k_tol: f64,
    pub rr_tol: f64,
    pub rr_max_iter: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_outer_iter` | `usize` | Maximum outer (K-update) iterations before returning<br>[`FlashError::NotConverged`]. DWSIM's `maxit_e` default is 100. |
| `k_tol` | `f64` | Outer convergence tolerance on the total K-value change<br>`Σ_i |K_i^new − K_i^old|` \[-\]. |
| `rr_tol` | `f64` | Inner Rachford-Rice absolute tolerance on the Newton/bisection step in<br>`β` \[-\]. |
| `rr_max_iter` | `usize` | Maximum inner Rachford-Rice iterations per outer pass. |

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
    fn clone(self: &Self) -> NestedLoopsOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &NestedLoopsOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `wilson_k_values`

**Attributes:**

- `MustUse { reason: None }`

Wilson K-value first guess `K_i = (Pc_i / P) · exp[5.373 (1 + ω_i)(1 − Tc_i / T)]`.

The standard ideal first estimate used to seed the nested-loops iteration
(Wilson, 1969). Ported from DWSIM `PropertyPackage.vb`
`DW_CalcKvalue_Ideal_Wilson` (line 1663).

# Units

- `components`: each supplies `critical_pressure` `Pc` \[Pa\],
  `critical_temperature` `Tc` \[K\], `acentric_factor` `ω` \[-\].
- `temperature` `T` \[K\] (> 0), `pressure` `P` \[Pa\] (> 0).
- Returns dimensionless `K_i` \[-\], one per component.

# Panics / range

Does not panic; with a non-positive `P` or `T` the returned values are
non-finite (the caller is expected to pass physical `T`, `P` > 0).

```rust
pub fn wilson_k_values(components: &[crate::thermo::Component], temperature: f64, pressure: f64) -> Vec<f64> { /* ... */ }
```

#### Function `rachford_rice_g`

**Attributes:**

- `MustUse { reason: None }`

The Rachford-Rice objective `g(β) = Σ_i z_i (K_i − 1) / (1 + β (K_i − 1))`.

Dimensionless. `z`, `k` must be the same length. Ported from the inner sum
of DWSIM `NestedLoops.vb` line 344 / 553.

```rust
pub fn rachford_rice_g(z: &[f64], k: &[f64], beta: f64) -> f64 { /* ... */ }
```

#### Function `rachford_rice_dg`

**Attributes:**

- `MustUse { reason: None }`

Derivative `g'(β) = −Σ_i z_i (K_i − 1)² / (1 + β (K_i − 1))²` of
[`rachford_rice_g`] (always ≤ 0, i.e. `g` is monotonically decreasing).

Ported from DWSIM `NestedLoops.vb` line 554 (`dF`).

```rust
pub fn rachford_rice_dg(z: &[f64], k: &[f64], beta: f64) -> f64 { /* ... */ }
```

#### Function `solve_rachford_rice`

Solve the Rachford-Rice equation for a **fixed** K-vector and return the
vapour fraction `β` with the equilibrium phase compositions.

# Method

1. **Degenerate single-phase detection.** `g` is decreasing in `β`. If
   `g(0) ≤ 0` the feed is subcooled liquid (`β = 0`); if `g(1) ≥ 0` it is
   superheated vapour (`β = 1`). These subsume the all-`K > 1` (→ `β = 1`)
   and all-`K < 1` (→ `β = 0`) cases. In each degenerate case the
   infinitesimal incipient phase is returned as a *normalised* composition
   (`y_i ∝ K_i z_i` at `β = 0`; `x_i ∝ z_i / K_i` at `β = 1`), matching
   DWSIM `NestedLoops.vb` lines 418-429.
2. **Bracketed root.** Otherwise the root lies strictly in `(0, 1)` (a
   subset of the negative-flash window `[1/(1 − K_max), 1/(1 − K_min)]`,
   DWSIM lines 323-334). It is found by a **safeguarded Newton iteration
   with bisection fallback** (the Numerical-Recipes `rtsafe` scheme): a
   Newton step is accepted only while it stays inside the current bracket
   and keeps shrinking it, otherwise the interval is bisected. This is
   globally convergent (bisection can never fail on a sign-changing
   bracket) yet retains Newton's quadratic local rate — combining DWSIM's
   own Newton update (line 574) with a guaranteed bracket.

# Units / ranges

`z`, `k`: mole fractions and K-values \[-\], equal length, `≥ 1` component,
all finite. Returns [`FlashResult`] with `iterations = 0` (no K-update was
done). `x`, `y` are mole fractions \[-\]; `beta` \[-\] ∈ `[0, 1]`.

# Errors

[`FlashError::Empty`] if `z` is empty, [`FlashError::LengthMismatch`] if
`z.len() != k.len()`, [`FlashError::NonFinite`] on any non-finite input.

```rust
pub fn solve_rachford_rice(z: &[f64], k: &[f64]) -> Result<FlashResult, FlashError> { /* ... */ }
```

#### Function `nested_loops_flash`

Full **nested-loops** isothermal-isobaric flash: iterate the K-values to
self-consistency, solving Rachford-Rice at each outer pass.

This is the successive-substitution outer loop of DWSIM `NestedLoops.vb`
`ConvergeVF` (lines 481-624). Difference documented for honesty: DWSIM
interleaves *one* Newton step on `β` with each K-update inside the same
loop; this port instead **fully solves** Rachford-Rice ([`solve_rachford_rice`])
on every outer pass before updating the K-values — the classic textbook
nested-loops structure the task specifies. Both converge to the same fixed
point `K_i = k_values(x, y, T, P)`.

# The K-closure (decoupling boundary)

`k_values(x, y, T, P) -> Vec<f64>` is the sole model-dependent step: given
trial liquid `x` and vapour `y` mole fractions \[-\] at `T` \[K\], `P` \[Pa\],
it returns updated K-values \[-\] from a fugacity/activity property model.
It is a **generic `Fn`**, not a trait object — so this module stays
independent of the EOS/activity code (mirrors `expander::isentropic`'s
closure pattern; obeys the workspace no-`dyn` rule).

# Initialisation

The first K-guess is Wilson ([`wilson_k_values`]) from `components` at
`(T, P)`.

# Units / ranges

`z`: feed mole fractions \[-\] (need not be pre-normalised — Rachford-Rice
is homogeneous in `z`, but physical feeds sum to 1). `components` supplies
the Wilson critical constants; `components.len()` must equal `z.len()`.
`temperature` `T` \[K\] > 0, `pressure` `P` \[Pa\] > 0.

# Convergence

Stops when the total K-value change `Σ_i |K_i^new − K_i^old| < opts.k_tol`.
Returns [`FlashError::NotConverged`] after `opts.max_outer_iter` passes.
For a **constant** K-closure the second pass reproduces the first K-vector
exactly, so it converges in a single outer iteration (`iterations = 1`),
reducing to one [`solve_rachford_rice`] solve.

# Errors

Propagates [`solve_rachford_rice`] errors; [`FlashError::LengthMismatch`]
if `components.len() != z.len()`; [`FlashError::NotConverged`] on
non-convergence.

```rust
pub fn nested_loops_flash<F>(z: &[f64], components: &[crate::thermo::Component], temperature: f64, pressure: f64, k_values: F, opts: NestedLoopsOptions) -> Result<FlashResult, FlashError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64> { /* ... */ }
```

## Module `flash_insideout`

Boston-Britt **Inside-Out** two-phase (VLE) isothermal-isobaric (**PT**) flash.

Ported from DWSIM `DWSIM.Thermodynamics/FlashAlgorithms/BostonBrittInsideOut.vb`
(`Flash_PT`, lines 76-368), GPL-3.0, commit `1abf72d`. The Wilson K-value
first guess is DWSIM `BostonBrittInsideOut.vb` line 114 / 437. Specific ported
lines are cited at each function below.

Ref: J. F. Boston, H. I. Britt, *A radically different formulation and
solution of the single-stage flash problem*, Computers & Chemical
Engineering **2** (1978) 109-122
(<https://doi.org/10.1016/0098-1354(78)80015-5>).

# Provenance

```text
Upstream project : DWSIM (Daniel Wagner O. de Medeiros)
Source file      : DWSIM.Thermodynamics/FlashAlgorithms/BostonBrittInsideOut.vb
Commit           : 1abf72d
Licence          : GPL-3.0
```

# What this module computes

Given a feed of overall mole fractions `z_i` \[-\] at fixed temperature `T`
\[K\] and pressure `P` \[Pa\], split it into an equilibrium liquid (`x_i`) and
vapour (`y_i`), returning the vapour molar fraction `V ≡ β` \[-\] and the
K-values `K_i = y_i / x_i` \[-\].

# The Inside-Out idea (why it exists)

A rigorous property model (fugacity coefficients from a cubic EOS) is
expensive. The Inside-Out method wraps the phase-split solve in **two nested
loops** that call the rigorous model as *rarely* as possible:

- **Inner ("inside") loop — cheap simple model.** The K-values are frozen and
  re-expressed through the log-relative-volatility variables `u_i = ln K_i`
  (DWSIM `BostonBrittInsideOut.vb` line 240; the base-component reference is
  pinned to `K_b = K_b0 = 1` in the DWSIM source, lines 236-237, so `u_i` is
  taken relative to unity). Holding `u` fixed, the vapour fraction is found
  from the **mole-balance / energy-balance** stationarity condition
  [`inner_stripping_solve`] — no property-model call happens here.
- **Outer ("outside") loop — rigorous update.** With the inner `(x, y)`
  converged, the rigorous property model is called *once* to recompute the
  K-values (`K_i ← k_values(x, y, T, P)`), the `u_i` are updated by successive
  substitution (DWSIM line 344, non-`fastmode`), and the inner loop is
  re-entered. Convergence is on `Σ_i (u_i^{old} − u_i^{new})²` \[-\]
  (DWSIM `AbsSqrSumY(fx) < etol`, line 358).

With the base component pinned to unity (as in the DWSIM source), the inner
simple model reduces algebraically to a Rachford-Rice solve — see
[`inner_stripping_solve`] — so at a fixed K-vector the Inside-Out split is
*identical* to the classic nested-loops split
([`crate::thermo::flash::solve_rachford_rice`]). The two methods therefore
converge to the **same** fixed point `K_i = k_values(x, y, T, P)`; only the
bookkeeping of *how* the outer K-update is organised differs. This identity is
exactly V&V check (1) below and is the strongest correctness test available.

# The K-closure (decoupling boundary)

Mirroring [`crate::thermo::flash::nested_loops_flash`], the sole
model-dependent step — turning a trial `(x, y, T, P)` into rigorous K-values —
is handed in as a **generic `Fn` closure**, *not* a trait object. This module
therefore has no dependency on the EOS / activity code and no `dyn` dispatch,
per the workspace `CLAUDE.md` (no trait objects / `Box` / lifetimes).

# Honest scope (verification, not benchmark validation)

- **PT (isothermal-isobaric) two-phase VLE only.** DWSIM's
  `BostonBrittInsideOut.vb` also carries PH / PS / TV / PV energy flashes and a
  Broyden `fastmode` acceleration (lines 324-347); this port implements the
  **PT specification with plain successive substitution** (the `fastmode = 0`
  branch). The energy flashes and the Broyden acceleration are out of scope.
- **Base component pinned to unity** (`K_b = 1`), following the DWSIM source
  exactly (its `CalcKbj1` / `CalcKbj2` base-component selectors, lines
  2192-2247, are commented out in `Flash_PT`). A genuinely
  *variable*-base-component Inside-Out (the full Boston-Britt simple K-model
  `ln K_i = A + B/T`) is therefore **not** reproduced; the "inside" model here
  is the unity-referenced log-K Rachford-Rice inner solve. This is faithful to
  the checked-in DWSIM behaviour, not the textbook maximum.
- The K-closure (fugacity / activity model) is the caller's responsibility.
- The tests below are **verification** — agreement with the already-ported
  nested-loops flash and internal consistency (mass balance, K-ordering) — not
  validation against an experimental / NIST / DECHEMA VLE benchmark.

> **⚠️ Unverified until validated.** AI-assisted port — untrusted draft
> material until human-reviewed per the crate `CLAUDE.md`. Not for nuclear
> facility operation, reactor control, safety-critical, or licensing
> decisions. Independent OUTRAM PARK fork, not the official DWSIM.

```rust
pub mod flash_insideout { /* ... */ }
```

### Types

#### Struct `InsideOutOptions`

Tuning parameters for [`inside_out_flash`].

Field names mirror the DWSIM `BostonBrittInsideOut` settings: an external
(outer, rigorous-K) tolerance and iteration cap, and an internal (inner,
simple-model stripping) tolerance and iteration cap.

```rust
pub struct InsideOutOptions {
    pub max_outer_iter: usize,
    pub outer_tol: f64,
    pub max_inner_iter: usize,
    pub inner_tol: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_outer_iter` | `usize` | Maximum outer (rigorous-K successive-substitution) iterations before<br>returning [`FlashError::NotConverged`]. DWSIM `maxit_e` default is 100. |
| `outer_tol` | `f64` | Outer convergence tolerance on `Σ_i (u_i^{old} − u_i^{new})²` \[-\], the<br>squared change in the log-K variables (DWSIM `etol`, `AbsSqrSumY`). |
| `max_inner_iter` | `usize` | Maximum inner (simple-model stripping) bisection iterations per outer pass. |
| `inner_tol` | `f64` | Inner convergence tolerance on the vapour-fraction bracket width \[-\]<br>(DWSIM `itol`). |

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
    fn clone(self: &Self) -> InsideOutOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &InsideOutOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `InnerSplit`

Converged inner ("inside") simple-model split at a **fixed** K-vector.

All compositions are mole fractions \[-\] summing to 1; `vapor_fraction`
`V ≡ β` \[-\] ∈ `[0, 1]`.

```rust
pub struct InnerSplit {
    pub vapor_fraction: f64,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vapor_fraction` | `f64` | Vapour molar fraction `V ≡ β` \[-\] ∈ `[0, 1]`. |
| `x` | `Vec<f64>` | Liquid-phase mole fractions `x_i` \[-\] (sum to 1). |
| `y` | `Vec<f64>` | Vapour-phase mole fractions `y_i` \[-\] (sum to 1). |

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
    fn clone(self: &Self) -> InnerSplit { /* ... */ }
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
    fn eq(self: &Self, other: &InnerSplit) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `inner_stripping_solve`

Solve the **inner ("inside") simple-model** phase split for a *fixed* K-vector
via the Boston-Britt stripping-factor stationarity condition.

# Method (DWSIM `TPErrorFunc`, lines 2249-2279; base `K_b0 = 1`)

With log-relative-volatility variables `u_i = ln K_i` and the transformed
vapour fraction `R` (equal to the vapour fraction `V` when the base component
is pinned to unity, DWSIM line 249), define the un-normalised phase amounts

```text
p_i(R) = z_i / (1 − R + R·exp(u_i)) = z_i / (1 + R (K_i − 1)),
```

then `S_x = Σ_i p_i`, `S_y = Σ_i exp(u_i) p_i`, with normalised compositions
`x_i = p_i / S_x`, `y_i = exp(u_i) p_i / S_y` and `V = 1 − (1 − R) S_x`. The
inner stationarity ("energy-balance") residual DWSIM minimises is
`e(R) = S_x / S_y − 1`.

Because `S_y − S_x = Σ_i (K_i − 1) z_i / (1 + R (K_i − 1)) = g(R)` is exactly
the Rachford-Rice function [`crate::thermo::flash::rachford_rice_g`], the
residual is `e(R) = −g(R) / S_y`, which vanishes **iff `g(R) = 0`**. The
unity-base inner simple model is therefore algebraically the Rachford-Rice
equation, and this routine solves `g(R) = 0` directly by monotone bisection
(`g` is strictly decreasing in `R`), replacing DWSIM's Brent minimisation of
`e(R)²` with the equivalent — and more robust — bracketed root of `g`.

# Single-phase limits (DWSIM lines 264-299)

`g` is decreasing, so `g(0) ≤ 0` ⇒ subcooled liquid (`V = 0`, `x = z`) and
`g(1) ≥ 0` ⇒ superheated vapour (`V = 1`, `y = z`); the incipient phase is
returned normalised.

# Units / ranges

`z`, `k`: mole fractions and K-values \[-\], equal length, ≥ 1 component, all
finite and `k_i > 0`. `max_iter`, `tol` bound the bisection.

# Errors

[`FlashError::Empty`] if `z` is empty, [`FlashError::LengthMismatch`] if
`z.len() != k.len()`, [`FlashError::NonFinite`] on a non-finite input.

```rust
pub fn inner_stripping_solve(z: &[f64], k: &[f64], max_iter: usize, tol: f64) -> Result<InnerSplit, crate::thermo::flash::FlashError> { /* ... */ }
```

#### Function `inside_out_flash`

Full **Boston-Britt Inside-Out** isothermal-isobaric two-phase VLE flash.

Ported from DWSIM `BostonBrittInsideOut.vb` `Flash_PT` (lines 76-368),
`fastmode = 0` (plain successive substitution) branch, base component pinned
to unity.

# Algorithm

1. **Seed.** K-values from Wilson ([`wilson_k_values`]) at `(T, P)`;
   log-K variables `u_i = ln K_i` (DWSIM line 240).
2. **Outer loop** (rigorous K update, DWSIM lines 251-358), for up to
   `opts.max_outer_iter` passes:
   a. **Inner ("inside") solve** — [`inner_stripping_solve`] on the frozen
      `K = exp(u)` gives `(x, y, V)` with no property-model call.
   b. **Rigorous K** — `K^{new} ← k_values(x, y, T, P)` (DWSIM line 308).
   c. **Successive substitution** — `u_i ← ln K_i^{new}` (DWSIM line 344).
   d. **Convergence** — stop when `Σ_i (u_i^{old} − u_i^{new})² <
      opts.outer_tol` (DWSIM `AbsSqrSumY`, line 358).

# The K-closure (decoupling boundary)

`k_values(x, y, T, P) -> Vec<f64>`: given trial liquid `x` and vapour `y`
mole fractions \[-\] at `T` \[K\], `P` \[Pa\], returns rigorous K-values \[-\]
from a fugacity / activity property model. A **generic `Fn`**, not a trait
object — so this module stays free of the EOS / activity code and of `dyn`
dispatch (workspace `CLAUDE.md`).

# Units / ranges

`z`: feed mole fractions \[-\] (physical feeds sum to 1); `components` supplies
the Wilson critical constants with `components.len() == z.len()`;
`temperature` `T` \[K\] > 0, `pressure` `P` \[Pa\] > 0.

# Returns

A [`FlashResult`] whose `beta` is the vapour molar fraction `V` \[-\] ∈
`[0, 1]`, `x` / `y` the equilibrium mole fractions \[-\], `k` the converged
rigorous K-values \[-\], and `iterations` the number of completed outer
passes. For a **constant** K-closure the second pass sees zero log-K change
and returns, so it converges in exactly **two** outer passes
(`iterations = 2`) — reproducing one [`inner_stripping_solve`] on that
constant K, exactly as [`crate::thermo::flash::nested_loops_flash`] does.

# Errors

[`FlashError::LengthMismatch`] on a `components`/`z` (or closure-output) size
mismatch; [`FlashError::NonFinite`] on a non-finite K-value; propagates
[`inner_stripping_solve`] errors; [`FlashError::NotConverged`] if the outer
successive substitution does not reach `opts.outer_tol` within the budget.

```rust
pub fn inside_out_flash<F>(z: &[f64], components: &[crate::thermo::Component], temperature: f64, pressure: f64, k_values: F, opts: InsideOutOptions) -> Result<crate::thermo::flash::FlashResult, crate::thermo::flash::FlashError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64> { /* ... */ }
```

## Module `flash_insideout_3p`

Boston-Fournier **Inside-Out** three-phase (VLLE) isothermal-isobaric
(**PT**) flash.

Ported from DWSIM
`DWSIM.Thermodynamics/FlashAlgorithms/BostonFournierInsideOut3P.vb`
(`Flash_PT` orchestration lines 74-225, the Inside-Out three-phase core
`Flash_PT_3P` lines 1285-1504, and the inner simple-model residuals
`TPErrorFunc` lines 1506-1557 / `SErrorFunc` lines 1571-1581), GPL-3.0,
commit `1abf72d`. The second-liquid estimate mirrors the same source's
`Flash_PT` lines 176-209. Specific ported lines are cited at each function
below.

Ref: J. F. Boston, V. B. Fournier, *A quasi-Newton algorithm for solving
multiphase equilibrium flash problems* (the Inside-Out family; the
two-phase parent is Boston & Britt, Computers & Chemical Engineering **2**
(1978) 109-122, <https://doi.org/10.1016/0098-1354(78)80015-5>).

# Provenance

```text
Upstream project : DWSIM (Daniel Wagner O. de Medeiros)
Source file      : DWSIM.Thermodynamics/FlashAlgorithms/BostonFournierInsideOut3P.vb
Commit           : 1abf72d
Licence          : GPL-3.0
```

# What this module computes

Given a feed of overall mole fractions `z_i` \[-\] at fixed temperature `T`
\[K\] and pressure `P` \[Pa\], split it into up to three coexisting phases —
a vapour (`y_i`, molar fraction `V`) and two liquids (`x^{I}_i`, fraction
`L^{I}`; `x^{II}_i`, fraction `L^{II}`) with `V + L^{I} + L^{II} = 1`, in
mutual equilibrium (`φ_i^V y_i = φ_i^{L I} x^{I}_i = φ_i^{L II} x^{II}_i`).
The K-values are `K^{j}_i = φ_i^{L j} / φ_i^{V} = y_i / x^{j}_i`
\[-\] for liquid `j ∈ {I, II}`.

# The Inside-Out idea, extended to three phases

Exactly as the two-phase parent
([`crate::thermo::flash_insideout`]), the rigorous property model (fugacity
coefficients from a cubic EOS) is called as *rarely* as possible by wrapping
the phase split in **two nested loops**:

- **Inner ("inside") loop — cheap simple model.** With the two liquid
  K-vectors frozen and the base component pinned to unity (DWSIM `Flash_PT_3P`
  line 1363, `Kb = Kb0 = 1`), the compositions and the two liquid fractions
  `(L^{I}, L^{II})` are found with **no property-model call**. DWSIM
  parametrises this inner solve with a vapour-stripping variable `R` and a
  liquid-split variable `S` and minimises `(Kb − 1)^2` over `R` (outer Brent,
  `TPErrorFunc`) with `SErrorFunc = 0` over `S` (inner Brent). With the base
  pinned to unity that `(R, S)` model is **algebraically the two-equation
  three-phase Rachford-Rice system** (see [`inside_out_3p_core`] docs for the
  identity), so this port solves that system directly with the already-ported,
  more robust damped 2×2 Newton core
  [`crate::thermo::flash_vlle::solve_3p_fixed_k`] — the same substitution
  [`crate::thermo::flash_insideout`] makes in the two-phase case (Rachford-Rice
  root instead of Brent minimisation).
- **Outer ("outside") loop — rigorous update.** With the inner
  `(x^{I}, x^{II}, y, L^{I}, L^{II})` converged, the rigorous property model
  is called *twice* (once per liquid) to recompute
  `K^{I} ← k_values(x^{I}, y)`, `K^{II} ← k_values(x^{II}, y)`, the log-K
  variables `u^{j}_i = ln K^{j}_i` are updated by plain successive
  substitution (DWSIM `Flash_PT_3P` lines 1455-1458, the `fastmode = 0`
  branch), and the inner loop is re-entered. Convergence is on
  `Σ_i |u^{I}_i − u^{I,new}_i| + Σ_i |u^{II}_i − u^{II,new}_i|` \[-\]
  (DWSIM `AbsSum(fx) < etol`, line 1475).

# Orchestration (when three phases appear)

[`inside_out_flash_3p`] mirrors DWSIM `Flash_PT` (lines 74-225): first a
rigorous **two-phase VLE** Inside-Out flash
([`crate::thermo::flash_insideout::inside_out_flash`]); then, if a liquid
exists, a **phase-stability test** on that liquid
([`crate::thermo::stability::stability_test`], the analogue of DWSIM's
`StabTest2`) to detect a distinct second liquid. Only if the liquid is
unstable does the three-phase Inside-Out core [`inside_out_3p_core`] run;
otherwise the two-phase result is returned unchanged (VLLE with `L^{II} = 0`).

This is the identical orchestration as the nested-loops three-phase port
[`crate::thermo::flash_vlle::flash_pt_vlle`]; the sole difference is that the
three-phase *inner/outer* split here is organised as Inside-Out (fully solve
the frozen-K inner system, then successive-substitute the log-K), whereas
`flash_pt_vlle` interleaves one Newton step per rigorous-K refresh.

# Honest scope (verification, not benchmark validation, and a *partial* port)

Three-phase flash robustness is genuinely hard, and this is a **first port**:

- **Base component pinned to unity** (`Kb = Kb0 = 1`), following the DWSIM
  source exactly (its `CalcKbjw` base-component selector, line 1363, is
  commented out). The variable-base Boston-Fournier simple K-model is therefore
  **not** reproduced; the inner model is the unity-referenced three-phase
  Rachford-Rice split.
- **PT specification with plain successive substitution** (`fastmode = 0`).
  DWSIM's Broyden `fastmode` acceleration (lines 1433-1451) and the PH / PS /
  TV / PV energy flashes are out of scope.
- **Second-liquid detection is only as good as the two Wilson-seeded stability
  trials** ([`crate::thermo::stability`]); a liquid-liquid split neither Wilson
  seed reaches is missed and the flash silently returns two phases. No global
  TPD minimisation.
- **Liquid labelling is not physically canonical.** DWSIM condenses trivial
  (identical) liquids via `AUX_CheckTrivial` and orders the two liquids by
  density `AUX_LIQDENS` (lines 1485-1500). This port applies a
  composition-distance trivial-liquid check (condense to two phases when the
  two liquid compositions coincide) but does **not** density-order the two
  liquids — that needs an absolute-density closure this K-only interface does
  not expose. Which liquid is labelled `L^{I}` vs `L^{II}` is therefore not
  canonical; mass balance and the sum-to-one identities (the V&V checks) are
  independent of the labelling.
- **`k_ij = 0`** throughout (geometric-mean mixing), which makes a genuine
  liquid-liquid split under a cubic EOS with the bundled reference compounds
  unlikely; the three-phase numerics are therefore verified on the **fixed-K**
  core [`inside_out_3p_core`] (constant K-closure) against the algebraic
  mass-balance identity and against the already-ported
  [`crate::thermo::flash_vlle::solve_3p_fixed_k`], and the composed driver is
  verified to **reduce to the two-phase result** when no second liquid is
  found. A full EOS-driven LLE benchmark is deferred.

> **⚠️ Unverified until validated.** AI-assisted **partial** port — untrusted
> draft material until human-reviewed per the crate `CLAUDE.md`. Verification,
> not validation. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
> the official DWSIM.

# Design (workspace + crate `CLAUDE.md`)

Enum dispatch (the fugacity model is the [`CubicEos`] **enum**), no trait
objects / `dyn` / `Box` / lifetimes / channels. The rigorous K-update is a
**generic `Fn` closure**, so this module carries no dependency on the EOS /
activity code and no `dyn` dispatch. Compositions owned by value; documented
raw `f64` (SI: K, Pa, mole fractions \[-\]) in the inner loops.

```rust
pub mod flash_insideout_3p { /* ... */ }
```

### Types

#### Struct `InsideOut3POptions`

Tuning parameters for [`inside_out_3p_core`] and [`inside_out_flash_3p`].

Combines the Inside-Out **outer** (rigorous-K successive-substitution)
controls with the [`VlleOptions`] that bound the frozen-K **inner** three-phase
Newton solve ([`crate::thermo::flash_vlle::solve_3p_fixed_k`]).

```rust
pub struct InsideOut3POptions {
    pub max_outer_iter: usize,
    pub outer_tol: f64,
    pub min_phase_fraction: f64,
    pub trivial_tol: f64,
    pub inner: crate::thermo::flash_vlle::VlleOptions,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_outer_iter` | `usize` | Maximum outer (rigorous-K successive-substitution) iterations before<br>returning [`FlashError::NotConverged`]. DWSIM `maxit_e` default is 100. |
| `outer_tol` | `f64` | Outer convergence tolerance on<br>`Σ_i |u^{I}_i − u^{I,new}_i| + Σ_i |u^{II}_i − u^{II,new}_i|` \[-\], the<br>summed absolute change in the two liquids' log-K variables (DWSIM<br>`AbsSum(fx) < etol`, line 1475). |
| `min_phase_fraction` | `f64` | A liquid phase whose fraction falls below this \[-\] is treated as absent<br>(the split has collapsed back to two phases). |
| `trivial_tol` | `f64` | Composition distance `Σ_i |x^{I}_i − x^{II}_i|` \[-\] below which the two<br>liquids are deemed identical (the trivial-liquid solution) and condensed<br>to a single liquid — the K-only analogue of DWSIM `AUX_CheckTrivial`<br>(line 1485). |
| `inner` | `crate::thermo::flash_vlle::VlleOptions` | Controls for the frozen-K inner three-phase Newton solve<br>([`crate::thermo::flash_vlle::solve_3p_fixed_k`]). |

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
    fn clone(self: &Self) -> InsideOut3POptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &InsideOut3POptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `inside_out_3p_core`

**Inside-Out three-phase core**: the frozen-K inner three-phase split driven
to rigorous-K self-consistency by successive substitution on the two liquids'
log-K variables.

Ported from DWSIM `BostonFournierInsideOut3P.vb` `Flash_PT_3P` (lines
1285-1504), `fastmode = 0` (plain successive substitution) branch, base
component pinned to unity (`Kb = Kb0 = 1`, line 1363).

# The inner model *is* three-phase Rachford-Rice (the base-unity identity)

DWSIM's inner simple model (`TPErrorFunc`, line 1523) writes the un-normalised
vapour amounts, with `Kb0 = 1`, as

```text
p_i = z_i / [ R + (1 − R + S) / (2 K^{I}_i) + (1 − R − S) / (2 K^{II}_i) ],
```

with the liquid fractions recovered (lines 1546-1548) as `L^{I} = ½(1 + S −
V)`, `L^{II} = ½(1 − S − V)` at the converged base `Kb = 1`. Substituting
`R = V`, the bracket equals
`V + L^{I}/K^{I}_i + L^{II}/K^{II}_i = 1 − β^{I}_i L^{I} − β^{II}_i L^{II}
= D_i`, with `β^{j}_i = 1 − 1/K^{j}_i` — exactly the denominator of the
two-equation three-phase Rachford-Rice system solved in
[`crate::thermo::flash_vlle`]. Moreover DWSIM's inner `S`-residual
(`SErrorFunc`, line 1577) is `Σ_i z_i (1/K^{I}_i − 1/K^{II}_i) / D_i`, which is
precisely `F_2 − F_1` of that system, and its `Kb = 1` condition closes the
remaining equation. This port therefore solves the frozen-K inner split with
the already-ported, monotone-well-posed damped 2×2 Newton
[`crate::thermo::flash_vlle::solve_3p_fixed_k`], targeting the identical root
as DWSIM's Brent-in-Brent `(R, S)` minimisation — the direct three-phase
analogue of the substitution [`crate::thermo::flash_insideout`] makes.

# Algorithm

1. **Seed.** Frozen liquid K-vectors `K^{I}`, `K^{II}` and liquid-fraction
   seeds `L^{I}_est`, `L^{II}_est`. Inner solve → converged
   `(x^{I}, x^{II}, y, L^{I}, L^{II})`.
2. **Outer loop** (rigorous K update, DWSIM lines 1385-1475), up to
   `opts.max_outer_iter` passes:
   a. **Rigorous K** — `K^{I,new} ← k_values(x^{I}, y)`,
      `K^{II,new} ← k_values(x^{II}, y)` (DWSIM lines 1410-1411).
   b. **Outer residual** — `Σ|u^{I} − u^{I,new}| + Σ|u^{II} − u^{II,new}|`
      with `u = ln K` (DWSIM `AbsSum(fx)`, line 1475).
   c. **Successive substitution** — `u^{j} ← ln K^{j,new}` (lines 1455-1458).
   d. **Inner solve** — [`crate::thermo::flash_vlle::solve_3p_fixed_k`] on the
      new K, reseeded from the previous `(L^{I}, L^{II})`.
   e. **Convergence** — stop when the residual `< opts.outer_tol`.
3. **Trivial-liquid guard** — if the two converged liquids coincide
   (`Σ|x^{I}_i − x^{II}_i| < opts.trivial_tol`) the split is condensed to two
   phases (DWSIM `AUX_CheckTrivial`, line 1485).

# The K-closure (decoupling boundary)

`k_values(x, y, T, P) -> Vec<f64>`: given a trial liquid `x` and the vapour
`y` (mole fractions \[-\]) at `T` \[K\], `P` \[Pa\], returns rigorous K-values
\[-\] for that liquid from a fugacity property model. It is called **twice** per
outer pass (once per liquid). A **generic `Fn`**, not a trait object — so this
module stays free of the EOS / activity code and of `dyn` dispatch.

# Units / ranges

`z`, `k1_init`, `k2_init`: equal length `n ≥ 1`; `z` feed mole fractions \[-\];
`k1_init`, `k2_init` liquid/vapour K-values \[-\] (`> 0`). `l1_est`, `l2_est`
∈ `(0, 1)` with `l1_est + l2_est < 1` seed the inner Newton iteration.
`t` \[K\] > 0, `p` \[Pa\] > 0. `opts` bounds the iterations and tolerances.

# Returns

A [`VlleResult`] with `v = 1 − L^{I} − L^{II}` and the normalised
compositions. At convergence the inner `F_1 = F_2 = 0`, so `Σ y = Σ x^{I} =
Σ x^{II} = 1` and the overall mass balance
`z_i = v y_i + L^{I} x^{I}_i + L^{II} x^{II}_i` closes. `three_phase` is
`false` (and `l2 = 0`) when the trivial-liquid guard condensed the split.

# Errors

[`FlashError::Empty`] on empty `z`; [`FlashError::LengthMismatch`] on a size
mismatch (including a closure-output size mismatch);
[`FlashError::NonFinite`] on a non-finite / non-positive K;
[`FlashError::NotConverged`] if the outer successive substitution does not
reach `opts.outer_tol` within `opts.max_outer_iter`. Propagates
[`crate::thermo::flash_vlle::solve_3p_fixed_k`] errors from the inner solve.

```rust
pub fn inside_out_3p_core<F>(z: &[f64], k1_init: &[f64], k2_init: &[f64], l1_est: f64, l2_est: f64, t: f64, p: f64, k_values: F, opts: InsideOut3POptions) -> Result<crate::thermo::flash_vlle::VlleResult, crate::thermo::flash::FlashError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64> { /* ... */ }
```

#### Function `inside_out_flash_3p`

Full **Boston-Fournier Inside-Out three-phase VLLE** isothermal-isobaric flash
of feed `z` at `T` \[K\], `P` \[Pa\] using the cubic EOS `eos` (`k_ij = 0`).

Ported from DWSIM `BostonFournierInsideOut3P.vb` `Flash_PT` (lines 74-225).

# Orchestration

1. **Two-phase VLE** via the Inside-Out parent
   ([`crate::thermo::flash_insideout::inside_out_flash`]) with the EOS
   K-closure ([`crate::thermo::flash_vlle::eos_k_values`]).
2. If a liquid exists, **stability-test** it
   ([`crate::thermo::stability::stability_test`]). Stable ⇒ return the
   two-phase result (`l2 = 0`).
3. Unstable ⇒ build a second-liquid estimate ([`phase_split_estimate`]) and
   run the **three-phase Inside-Out core** ([`inside_out_3p_core`]).
4. If the second liquid collapses below `opts.min_phase_fraction`, or the two
   liquids turn out trivially identical, fall back to the two-phase result.

# Units / ranges

`components.len() == z.len()`; `z` feed mole fractions \[-\] (sum to 1);
`t` \[K\] > 0, `p` \[Pa\] > 0. See the module scope note for the honest limits
(label ordering, missed splits, base pinned to unity, `k_ij = 0`).

# Errors

[`FlashError::LengthMismatch`] on a `components`/`z` size mismatch; propagates
[`FlashError`] from the two-phase Inside-Out flash and the three-phase core.

```rust
pub fn inside_out_flash_3p(components: &[crate::thermo::Component], z: &[f64], t: f64, p: f64, eos: crate::thermo::cubic_eos::CubicEos, opts: InsideOut3POptions) -> Result<crate::thermo::flash_vlle::VlleResult, crate::thermo::flash::FlashError> { /* ... */ }
```

## Module `flash_lle`

**Simple liquid-liquid equilibrium** (LLE) isothermal split at fixed
temperature, driven by an activity-coefficient model.

Ported from DWSIM `DWSIM.Thermodynamics/FlashAlgorithms/SimpleLLE.vb`
(`Flash_PT`, lines 82-330), GPL-3.0, commit `1abf72d`. Specific ported lines
are cited at each function below. The vapour-free, energy-flash paths
(`Flash_PH`/`Flash_PS`/`Flash_TV`/`Flash_PV`) of the DWSIM class are **not**
ported here — see *Honest scope*.

# Provenance

```text
Upstream project : DWSIM (Daniel Wagner O. de Medeiros; Gregor Reichert)
Source file      : DWSIM.Thermodynamics/FlashAlgorithms/SimpleLLE.vb
Commit           : 1abf72d
Licence          : GPL-3.0
```

# What this module computes

Given a single-liquid feed of overall mole fractions `z_i` \[-\] at a fixed
temperature `T` \[K\], split it (if it is unstable) into two coexisting liquid
phases — phase I (`x^{I}_i`, molar fraction `L^{I}`) and phase II (`x^{II}_i`,
molar fraction `L^{II}`) with `L^{I} + L^{II} = 1` — in mutual equilibrium.
For an activity-coefficient description the equilibrium (isoactivity)
condition is

```text
gamma_i^{I}(x^{I}, T) x^{I}_i = gamma_i^{II}(x^{II}, T) x^{II}_i   for every i,
```

i.e. the **activity** `a_i = gamma_i x_i` of each species is equal in the two
liquids. When the feed is a stable single liquid, no split exists and the
flash reports one phase.

## Why there is no pressure argument

DWSIM's `SimpleLLE.Flash_PT` takes `P` and forms the activity coefficient as
`gamma_i = P / Vp_i · phi_i` from its liquid **fugacity-coefficient** call
(`SimpleLLE.vb` lines 214-224): the `P` and the vapour pressure `Vp_i` cancel
exactly, leaving the liquid activity coefficient. This port takes the activity
coefficient **directly** from [`crate::thermo::activity::ActivityModel`], so
`P` never enters. Physically, at the low-to-moderate pressures where an
incompressible-liquid activity model applies, `gamma_i` is pressure-
independent, so the LLE split at fixed `T` depends only on `T` and `z`. The
API therefore takes `t` only; a caller wanting the DWSIM "PT" framing supplies
the same `T` and any `P` — the split is unchanged.

# Method (successive substitution, DWSIM `SimpleLLE.vb` lines 194-285)

With the liquid-liquid distribution ratio written through the activity
coefficients as `K_i = x^{I}_i / x^{II}_i = gamma_i^{II} / gamma_i^{I}`, the
per-component material balance `n^{I}_i + n^{II}_i = z_i` (mole numbers per
mole of feed) with `x^{I}_i = n^{I}_i / L^{I}`, `x^{II}_i = n^{II}_i / L^{II}`
gives the closed update

```text
n^{I}_i = z_i / ( 1 + gamma_i^{I} L^{II} / (gamma_i^{II} L^{I}) ),
n^{II}_i = z_i - n^{I}_i,   L^{I} = sum_i n^{I}_i,   L^{II} = 1 - L^{I}
```

(DWSIM `SimpleLLE.vb` lines 260-267). Each outer pass renormalises the two
liquid compositions (`x^{j} = n^{j} / L^{j}`), refreshes both activity-
coefficient vectors, then applies the update — a fixed-point iteration on the
phase split. An oscillation guard (DWSIM lines 269-278) averages the current
and previous phase-I mole numbers when the two phase fractions swap identities.

Convergence is declared (DWSIM lines 251-258) when the summed isoactivity
residual `sum_i |gamma_i^{I} x^{I}_i - gamma_i^{II} x^{II}_i|` falls below
`activity_tol`, or a phase fraction collapses (`< min_phase_fraction`), or the
two compositions coincide (`sum_i |x^{I}_i - x^{II}_i| < composition_merge_tol`),
or the phase fractions stop moving (`< fraction_change_tol`). The last three
all mean the split has *merged* back to a single liquid.

# Phase labelling

On a genuine split the two liquids are ordered by a **reduced molar Gibbs
energy of mixing** `g/RT = sum_i x_i (ln x_i + ln gamma_i)` (DWSIM re-orders by
`DW_CalcGibbsEnergy`, lines 305-312). This mixing Gibbs energy is self-
contained in the activity model; it omits the pure-component reference
`sum_i x_i g_i^{pure}` that DWSIM's absolute Gibbs energy includes (that needs
pure-component chemical potentials this activity-only interface does not
expose), so the phase-I / phase-II **labelling is not guaranteed identical to
DWSIM's** — mass balance and the sum-to-one / isoactivity identities (the V&V
checks) are label-independent.

# Honest scope (verification, not benchmark validation; a *partial* port)

- **`Flash_PT` only.** DWSIM `SimpleLLE` also exposes `Flash_PH`, `Flash_PS`,
  `Flash_TV`, `Flash_PV` (energy / spec flashes that wrap `Flash_PT` in a
  temperature/pressure root-find). None of those are ported here.
- **Activity-coefficient driver only.** The split is driven by
  [`crate::thermo::activity`] (NRTL / UNIQUAC / Ideal), not by a cubic-EOS
  liquid fugacity. No phi-phi LLE, no vapour phase, no solid.
- **Seeding is DWSIM's heuristic** ([`flash_pt_lle`]) or a caller-supplied
  estimate ([`flash_pt_lle_with_estimates`]); there is no built-in stability
  pre-test. A feed already inside a miscibility gap that the heuristic seed
  cannot leave may be reported as a single liquid — supply an estimate (e.g.
  from [`crate::thermo::stability`]) for a hard case.
- The tests below are **verification** against the algebraic identities (mass
  balance, sum-to-one, the isoactivity condition, the single-liquid limit),
  **not** validation against measured LLE tie-line data.

> **⚠️ Unverified until validated.** Untrusted AI-assisted **draft** material,
> pending human V&V per the crate `CLAUDE.md` (verification, not validation).
> Not for nuclear facility operation, reactor control, safety-critical, or
> licensing decisions. Independent OUTRAM PARK fork, not the official DWSIM.

# Design (workspace + crate `CLAUDE.md`)

The activity model is the [`crate::thermo::activity::ActivityModel`] **enum**
(no trait object, no `dyn` / `Box` / lifetimes / channels). Compositions are
owned by value (`Vec<f64>`); inner arithmetic is documented raw `f64` (SI:
K, mole fractions \[-\]). `#![forbid(unsafe_code)]` at the crate root.

```rust
pub mod flash_lle { /* ... */ }
```

### Types

#### Struct `LleOptions`

Tuning parameters for [`flash_pt_lle`] / [`flash_pt_lle_with_estimates`].

The defaults mirror the DWSIM `SimpleLLE` hard-coded tolerances
(`SimpleLLE.vb` lines 251-282).

```rust
pub struct LleOptions {
    pub max_iter: usize,
    pub activity_tol: f64,
    pub min_phase_fraction: f64,
    pub composition_merge_tol: f64,
    pub fraction_change_tol: f64,
    pub relaxation: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iter` | `usize` | Maximum outer successive-substitution passes before returning<br>[`FlashError::NotConverged`]. Matches DWSIM's abort at `ecount > 10000`<br>(`SimpleLLE.vb` line 282); interior splits converge in far fewer (the<br>reference water/n-butanol 70/30 split takes ~222), but feeds sitting right<br>on a miscibility-gap boundary converge only slowly (successive<br>substitution is near-singular there — an honest limitation, see the module<br>header). |
| `activity_tol` | `f64` | Convergence tolerance on the summed **isoactivity residual**<br>`sum_i |gamma_i^{I} x^{I}_i - gamma_i^{II} x^{II}_i|` \[-\] (DWSIM `etol`,<br>line 251 uses `1e-6`). |
| `min_phase_fraction` | `f64` | A liquid phase whose molar fraction falls below this \[-\] is treated as<br>absent — the split has merged to one liquid (DWSIM line 251 uses `1e-4`). |
| `composition_merge_tol` | `f64` | Total composition difference `sum_i |x^{I}_i - x^{II}_i|` \[-\] below which<br>the two liquids are deemed identical (merge to one liquid; DWSIM line 251<br>uses `1e-3`). |
| `fraction_change_tol` | `f64` | Convergence tolerance on the per-pass change of the two phase fractions<br>`|L^{I}_{prev} - L^{I}| + |L^{II}_{prev} - L^{II}|` \[-\] (DWSIM line 255<br>uses `1e-7`). |
| `relaxation` | `f64` | Optional successive-substitution **under-relaxation factor** `lambda`<br>∈ `(0, 1]` applied to the phase-I mole-number update<br>`n^{I} <- (1 - lambda) n^{I}_{prev} + lambda n^{I}_{raw}` \[-\].<br><br>The **default `lambda = 1.0`** is DWSIM's literal undamped substitution<br>(`SimpleLLE.vb` lines 260-267) and is the fastest — for the reference<br>water/n-butanol split the iteration is monotone (not oscillatory), so<br>damping only slows it. A `lambda < 1` is offered as a stabilizer for a<br>caller that hits a genuinely oscillatory system; it converges to the<br>identical fixed point and subsumes DWSIM's conditional 50/50 swap-average<br>(`SimpleLLE.vb` lines 269-278), which this port replaces with the general<br>relaxation knob rather than the swap-specific guard (the guard mis-fires<br>at symmetric fixed points). |

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
    fn clone(self: &Self) -> LleOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &LleOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `LleResult`

A converged (or best-effort) simple-LLE flash result.

When [`split`](LleResult::split) is `true` the feed separates into two liquids
with `l1 + l2 = 1`, each composition summing to 1, satisfying the isoactivity
condition `gamma_i^{I} x^{I}_i = gamma_i^{II} x^{II}_i` to `activity_tol`. When
`split` is `false` the feed is a single stable liquid: `l1 = 1`, `l2 = 0`, and
`x1 == x2 ==` the (normalised) feed, with `gamma1 == gamma2` its activity
coefficients.

The `l1`/`x1` vs `l2`/`x2` labelling follows a reduced-molar-Gibbs-of-mixing
ordering that is **not** guaranteed identical to DWSIM's absolute-Gibbs
ordering (see the module *Phase labelling* note); the mass-balance and
sum-to-one identities are label-independent.

```rust
pub struct LleResult {
    pub split: bool,
    pub l1: f64,
    pub l2: f64,
    pub x1: Vec<f64>,
    pub x2: Vec<f64>,
    pub gamma1: Vec<f64>,
    pub gamma2: Vec<f64>,
    pub iterations: usize,
    pub activity_residual: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `split` | `bool` | `true` iff a genuine two-liquid split was found; `false` for a single<br>stable liquid. |
| `l1` | `f64` | Phase-I molar fraction `L^{I}` \[-\] ∈ `[0, 1]` (`1.0` when `split` is<br>`false`). |
| `l2` | `f64` | Phase-II molar fraction `L^{II}` \[-\] ∈ `[0, 1]` (`0.0` when `split` is<br>`false`). |
| `x1` | `Vec<f64>` | Phase-I mole fractions `x^{I}_i` \[-\] (sum to 1). |
| `x2` | `Vec<f64>` | Phase-II mole fractions `x^{II}_i` \[-\] (sum to 1); equals `x1` when<br>`split` is `false`. |
| `gamma1` | `Vec<f64>` | Phase-I activity coefficients `gamma_i^{I}` \[-\] at `(x1, T)`. |
| `gamma2` | `Vec<f64>` | Phase-II activity coefficients `gamma_i^{II}` \[-\] at `(x2, T)`; equals<br>`gamma1` when `split` is `false`. |
| `iterations` | `usize` | Number of completed outer successive-substitution passes. |
| `activity_residual` | `f64` | Final summed isoactivity residual<br>`sum_i |gamma_i^{I} x^{I}_i - gamma_i^{II} x^{II}_i|` \[-\]. Near `0` on a<br>converged split; carries the last loop value on a merge. |

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
    fn clone(self: &Self) -> LleResult { /* ... */ }
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
    fn eq(self: &Self, other: &LleResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `flash_pt_lle`

Simple-LLE isothermal flash of feed `z` at temperature `t` \[K\] using the
**DWSIM default seed** ([`default_seed`]).

Entry point for DWSIM `SimpleLLE.Flash_PT` (`SimpleLLE.vb` lines 82-330) with
the no-initial-estimate seeding branch (lines 141-160). Splits the feed into
two liquids in equilibrium under the activity model
`model` (NRTL / UNIQUAC / Ideal), or reports a single stable liquid. See the
module header for the method and the pressure-independence rationale.

# Units / ranges

- `model`: the [`ActivityModel`]; for the non-ideal variants its parameter
  dimension must equal `z.len()`.
- `z`: feed mole fractions \[-\] (need not be pre-normalised; normalised
  internally). Every `z_i` should be `> 0` for a meaningful split.
- `t` \[K\] (`> 0`).

# Returns

An [`LleResult`]. On a genuine split `l1 + l2 = 1`, each phase sums to 1, and
the isoactivity residual is below `opts.activity_tol`. On a stable feed
`split = false`, `l1 = 1`, `l2 = 0`.

# Errors

[`FlashError::Empty`] on empty `z`; [`FlashError::NonFinite`] on a non-finite
or non-positive-sum feed; [`FlashError::NotConverged`] if the successive
substitution does not converge within `opts.max_iter` passes.

# Panics

Panics (via the activity model) if `model`'s parameter matrices are not sized
to `z.len()` — a programming error, not a runtime input error.

```rust
pub fn flash_pt_lle(model: &crate::thermo::activity::ActivityModel, z: &[f64], t: f64, opts: LleOptions) -> Result<LleResult, crate::thermo::flash::FlashError> { /* ... */ }
```

#### Function `flash_pt_lle_with_estimates`

Simple-LLE isothermal flash of feed `z` at `t` \[K\] from **caller-supplied
initial phase-composition estimates** (DWSIM `UseInitialEstimatesForPhase1/2`,
`SimpleLLE.vb` lines 117-172).

`x1_est`, `x2_est` are initial guesses for the phase-I and phase-II mole
fractions \[-\] (each length `z.len()`, normalised internally), and `l1_est`
∈ `(0, 1)` seeds the phase-I molar fraction; the phase-I mole-number seed is
`n^{I}_i = l1_est · x1_est_i` and `n^{II}_i = (1 - l1_est) · x2_est_i`. Use
this for a feed inside a miscibility gap that the default heuristic seed
cannot reach — e.g. seeding phase II from a
[`crate::thermo::stability`] destabilising trial.

Units / ranges / errors / panics are otherwise as [`flash_pt_lle`], plus
[`FlashError::LengthMismatch`] if `x1_est` or `x2_est` is not length `z.len()`.

```rust
pub fn flash_pt_lle_with_estimates(model: &crate::thermo::activity::ActivityModel, z: &[f64], t: f64, x1_est: &[f64], x2_est: &[f64], l1_est: f64, opts: LleOptions) -> Result<LleResult, crate::thermo::flash::FlashError> { /* ... */ }
```

## Module `flash_single_comp`

**Attributes:**

- `Other("#[forbid(unsafe_code)]")`

Single-component (pure-fluid) **saturation-shortcut** flash.

For a one-component system — or a degenerate multicomponent feed dominated by
a single effective component — a full multicomponent VLE flash is
unnecessary. The phase split at a given temperature `T` \[K\] and pressure
`P` \[Pa\] is decided entirely by the pure-fluid saturation curve:

```text
P < Psat(T)  ->  all vapour   (V = 1)
P > Psat(T)  ->  all liquid   (V = 0)
P = Psat(T)  ->  two-phase at the specified vapour fraction V in [0, 1]
```

where `Psat(T)` is the pure-component vapour pressure and `Tsat(P)` its
inverse (the saturation temperature). This module reproduces DWSIM's
`SingleCompFlash` VLE logic on top of the already-ported
[`crate::thermo::saturation`] bubble/dew kernel, which supplies `Psat`/`Tsat`
(for a pure feed `z = [1]` the bubble point, the dew point, and the vapour
pressure all coincide — the pressure/temperature at which `K = 1`).

## Provenance (GPLv3)

Ported from **DWSIM** (GPL-3.0):
`DWSIM.Thermodynamics/FlashAlgorithms/SingleCompFlash.vb`, commit `1abf72d`.
Copyright 2021 Daniel Wagner O. de Medeiros; DWSIM is distributed under the
GNU General Public License v3. This independent OUTRAM PARK fork is GPL-3.0.
Per-function line citations (`SingleCompFlash.vb:<line>`) appear at each item.

## What is ported (and what is not) — honest scope

DWSIM's `SingleCompFlash` handles vapour, liquid, **and solid** phases
(sublimation, melting/freezing, forced solids, and a special CO₂ triple-point
guard). This port covers the **vapour–liquid** shortcut only:

- [`flash_pt`] — phase state at `(T, P)` from `Psat(T)` vs `P`
  (`SingleCompFlash.vb:59`, the non-solid `If Pvap > P` / `Else` branches).
- [`flash_tv`] — saturation pressure `Psat(T)` at a specified vapour fraction
  (`SingleCompFlash.vb:290`, the `T > Tfus` liquid+vapour branch).
- [`flash_pv`] — saturation temperature `Tsat(P)` at a specified vapour
  fraction (`SingleCompFlash.vb:306`, the `Tsat > Tfus` liquid+vapour branch).
- [`flash_ph`] — pressure–enthalpy flash: superheated-vapour / two-phase /
  subcooled-liquid classification against the saturated enthalpies, with a
  single-phase temperature solve (`SingleCompFlash.vb:80`, the non-solid
  `H >= HsatV` / `H >= HsatL` / `Else` branches).

**Deliberately NOT ported** (documented, not silently dropped): every solid
branch — sublimation, partial/complete freezing and melting, `IsSolid` /
`ForcedSolids`, the fusion enthalpy `Hfus`, the `RET_VTF` fusion temperature,
the CO₂ triple-point guard — and the PS (pressure–entropy) flash. A feed
below its triple point is therefore out of scope here; the VLE shortcut
assumes the fluid is at or above its melting line.

## Decoupling — no `dyn`, no `Box`, no lifetimes

`Psat`/`Tsat` come from a [`PropertyPackageModel`] (enum dispatch; the
`Ideal` package gives the Wilson vapour pressure, a cubic package gives the
EOS saturation pressure). The PH-flash's one model-dependent step — the molar
enthalpy of a phase at `(T, P)` — is a **caller-supplied generic `Fn`
closure** (`Fn(T, P, Phase) -> f64`), never a trait object, mirroring
[`crate::thermo::energy_flash`] and the crate's push-to-caller pattern.

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

Temperature `T` \[K\], pressure `P` \[Pa\], molar enthalpy \[J/mol\], mole
fractions and vapour fraction \[-\], all `f64` in SI base units — the same
convention as the [`crate::thermo::flash`] / [`crate::thermo::saturation`]
kernel this sits on. Every parameter's unit is spelled out in its doc comment.

## V&V status

**Untrusted AI-assisted draft pending human V&V.** The inline tests are
*verification* (internal consistency against the defining saturation
relations and hand-computed analytic enthalpy cases), **not** validation
against experimental / NIST / DECHEMA saturation data. Not for nuclear
facility operation, reactor control, safety-critical, or licensing decisions.
Independent OUTRAM PARK fork, not the official DWSIM.

```rust
pub mod flash_single_comp { /* ... */ }
```

### Types

#### Enum `SingleCompPhase`

The equilibrium phase state a single-component flash resolves to.

```rust
pub enum SingleCompPhase {
    Vapour,
    Liquid,
    TwoPhase,
}
```

##### Variants

###### `Vapour`

All vapour (`V = 1`): the specification lies above the saturation curve
(`P < Psat(T)`, i.e. superheated).

###### `Liquid`

All liquid (`V = 0`): the specification lies below the saturation curve
(`P > Psat(T)`, i.e. subcooled).

###### `TwoPhase`

Two coexisting phases (`0 <= V <= 1`) on the saturation curve
(`P = Psat(T)`).

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
    fn clone(self: &Self) -> SingleCompPhase { /* ... */ }
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
    fn eq(self: &Self, other: &SingleCompPhase) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SingleCompResult`

A resolved single-component flash state.

All fields are SI: temperatures \[K\], pressures \[Pa\], fractions \[-\].
Exactly which of `temperature` / `pressure` was an input versus a solved
unknown depends on the entry point (see each `flash_*` function).

```rust
pub struct SingleCompResult {
    pub vapour_fraction: f64,
    pub liquid_fraction: f64,
    pub temperature: f64,
    pub pressure: f64,
    pub saturation_pressure: f64,
    pub phase: SingleCompPhase,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vapour_fraction` | `f64` | Vapour molar fraction `V` \[-\] in `[0, 1]` (moles vapour per mole feed):<br>`1` = all vapour, `0` = all liquid, interior = two-phase. |
| `liquid_fraction` | `f64` | Liquid molar fraction `1 - V` \[-\] in `[0, 1]`. |
| `temperature` | `f64` | Temperature `T` \[K\] of the resolved state (an input for [`flash_pt`] /<br>[`flash_tv`]; solved for [`flash_pv`] / [`flash_ph`]). |
| `pressure` | `f64` | Pressure `P` \[Pa\] of the resolved state (an input for [`flash_pt`] /<br>[`flash_pv`] / [`flash_ph`]; solved — `= Psat(T)` — for [`flash_tv`]). |
| `saturation_pressure` | `f64` | Pure-component saturation pressure `Psat` \[Pa\] evaluated at<br>`temperature`, from [`crate::thermo::saturation`]. On the saturation<br>curve this equals `pressure` to solver tolerance. |
| `phase` | `SingleCompPhase` | The resolved [`SingleCompPhase`]. |

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
    fn clone(self: &Self) -> SingleCompResult { /* ... */ }
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
    fn eq(self: &Self, other: &SingleCompResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SingleCompOptions`

Tuning parameters for the single-component flashes.

Only [`flash_ph`] uses the enthalpy-solve fields (`h_tol`, `t_min`, `t_max`,
`max_iter`); the other entry points are closed-form on top of the saturation
kernel and ignore them.

```rust
pub struct SingleCompOptions {
    pub h_tol: f64,
    pub t_min: f64,
    pub t_max: f64,
    pub max_iter: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `h_tol` | `f64` | Absolute convergence tolerance on the enthalpy residual<br>`|H(T) - H_target|` \[J/mol\] for the [`flash_ph`] single-phase solve. |
| `t_min` | `f64` | Lower bound of the [`flash_ph`] single-phase temperature search \[K\]. |
| `t_max` | `f64` | Upper bound of the [`flash_ph`] single-phase temperature search \[K\]. |
| `max_iter` | `usize` | Maximum bisection iterations for the [`flash_ph`] single-phase solve. |

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
    fn clone(self: &Self) -> SingleCompOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &SingleCompOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SingleCompError`

Error conditions for the single-component flashes.

```rust
pub enum SingleCompError {
    Empty,
    LengthMismatch {
        a: usize,
        b: usize,
    },
    NonFinite,
    NonPositive {
        what: &'static str,
        value: f64,
    },
    VapourFractionOutOfRange {
        value: f64,
    },
    Saturation(crate::thermo::saturation::SaturationError),
    NoBracket {
        t_min: f64,
        t_max: f64,
    },
    NotConverged {
        iterations: usize,
        residual: f64,
    },
}
```

##### Variants

###### `Empty`

An empty feed was supplied (need at least one component).

###### `LengthMismatch`

`components` and `z` were different lengths.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `usize` | Length of `components`. |
| `b` | `usize` | Length of `z`. |

###### `NonFinite`

A non-finite value (`NaN`/`inf`) appeared in an input.

###### `NonPositive`

A quantity that must be strictly positive was not.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which quantity (e.g. `"pressure"`). |
| `value` | `f64` | The offending value. |

###### `VapourFractionOutOfRange`

A specified vapour fraction `V` was outside `[0, 1]`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `value` | `f64` | The offending vapour fraction. |

###### `Saturation`

The pure-component saturation solve ([`crate::thermo::saturation`])
failed while computing `Psat`/`Tsat`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::thermo::saturation::SaturationError` |  |

###### `NoBracket`

The [`flash_ph`] single-phase temperature solve could not bracket the
target enthalpy within `[t_min, t_max]` (target unreachable in-window).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_min` | `f64` | Search-window floor \[K\]. |
| `t_max` | `f64` | Search-window ceiling \[K\]. |

###### `NotConverged`

The [`flash_ph`] single-phase temperature solve did not reach `h_tol`
within `max_iter` iterations.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations performed. |
| `residual` | `f64` | Final `|H(T) - H_target|` \[J/mol\]. |

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
    fn clone(self: &Self) -> SingleCompError { /* ... */ }
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
  - ```rust
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private18::Error + ''static> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: SaturationError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SingleCompError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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

#### Function `saturation_pressure`

Pure-component **saturation pressure** `Psat(T)` \[Pa\] of the dominant
component of feed `z` at temperature `T` \[K\], using `package` for the
K-model.

DWSIM analogue: `PP.AUX_PVAPi(idx, T)` (`SingleCompFlash.vb:62`). Here it is
the bubble pressure of the pure feed `z = [1]` from
[`crate::thermo::saturation::bubble_pressure`] — for a single component the
bubble point, dew point, and vapour pressure coincide (the pressure at which
`K = 1`). With [`PropertyPackageModel::Ideal`] this is the Wilson vapour
pressure `Psat = Pc·exp[5.373(1+ω)(1 − Tc/T)]`; with a cubic package it is
the EOS saturation pressure (equal-fugacity `φ^L = φ^V`).

# Units / ranges
`components.len() == z.len()`, `z` mole fractions \[-\], `temperature` `T`
\[K\] > 0. Returns `Psat` \[Pa\].

# Errors
[`SingleCompError::Empty`] / [`SingleCompError::LengthMismatch`] /
[`SingleCompError::NonFinite`] on bad inputs, [`SingleCompError::NonPositive`]
for `T <= 0`, and [`SingleCompError::Saturation`] if the bubble-pressure
solve fails.

```rust
pub fn saturation_pressure(components: &[crate::thermo::Component], z: &[f64], temperature: f64, package: crate::thermo::property_package::PropertyPackageModel) -> Result<f64, SingleCompError> { /* ... */ }
```

#### Function `saturation_temperature`

Pure-component **saturation temperature** `Tsat(P)` \[K\] of the dominant
component of feed `z` at pressure `P` \[Pa\], using `package` for the K-model.

DWSIM analogue: `PP.AUX_TSATi(P, idx)` (`SingleCompFlash.vb:86`). Here it is
the bubble temperature of the pure feed `z = [1]` from
[`crate::thermo::saturation::bubble_temperature`] — the inverse of
[`saturation_pressure`].

# Units / ranges / errors
As [`saturation_pressure`], with `pressure` `P` \[Pa\] > 0; returns `Tsat`
\[K\].

```rust
pub fn saturation_temperature(components: &[crate::thermo::Component], z: &[f64], pressure: f64, package: crate::thermo::property_package::PropertyPackageModel) -> Result<f64, SingleCompError> { /* ... */ }
```

#### Function `flash_pt`

**Pressure–temperature** single-component flash: resolve the phase state at
`(T, P)` from the saturation curve.

Ported from DWSIM `SingleCompFlash.vb:59` (`Flash_PT`, the non-solid path):
compute `Pvap = Psat(T)` and compare to `P`. `Pvap > P` ⇒ superheated vapour
(`V = 1`, `SingleCompFlash.vb:69-70`); otherwise subcooled liquid (`V = 0`,
`SingleCompFlash.vb:73-74`). At exact equality `Pvap == P` the split is
on the saturation line and reported as [`SingleCompPhase::TwoPhase`]; the
vapour fraction is undetermined by a PT specification alone and is reported
as `0` (specify it with [`flash_tv`] / [`flash_pv`] instead). The solid
branches (`IsSolid`, `T < Tfus`) are out of scope — see the module header.

# Units / ranges
`components.len() == z.len()`; `z` mole fractions \[-\]; `pressure` `P` \[Pa\]
> 0; `temperature` `T` \[K\] > 0. The returned [`SingleCompResult`] carries
the input `T`, `P`, the classified phase, and `Psat(T)` in
`saturation_pressure`.

# Errors
[`SingleCompError::Empty`] / [`SingleCompError::LengthMismatch`] /
[`SingleCompError::NonFinite`] / [`SingleCompError::NonPositive`] on bad
inputs; [`SingleCompError::Saturation`] if the `Psat` solve fails.

```rust
pub fn flash_pt(components: &[crate::thermo::Component], z: &[f64], pressure: f64, temperature: f64, package: crate::thermo::property_package::PropertyPackageModel) -> Result<SingleCompResult, SingleCompError> { /* ... */ }
```

#### Function `flash_tv`

**Temperature–vapour-fraction** single-component flash: return the
saturation pressure `Psat(T)` at a specified vapour fraction `V`.

Ported from DWSIM `SingleCompFlash.vb:290` (`Flash_TV`, the `T > Tfus`
liquid+vapour branch): for a pure fluid the equilibrium pressure of a
two-phase state at temperature `T` is fixed by the saturation curve
(`Psat(T)`), independent of `V`; `V` only sets how the feed is partitioned.
The solid+vapour branch (`T <= Tfus`) is out of scope.

# Units / ranges
`temperature` `T` \[K\] > 0; `vapour_fraction` `V` \[-\] in `[0, 1]`. The
returned [`SingleCompResult`] has `pressure = saturation_pressure = Psat(T)`
and the phase set from `V` (`1` → vapour, `0` → liquid, interior → two-phase).

# Errors
As [`flash_pt`], plus [`SingleCompError::VapourFractionOutOfRange`] for
`V ∉ [0, 1]`.

```rust
pub fn flash_tv(components: &[crate::thermo::Component], z: &[f64], temperature: f64, vapour_fraction: f64, package: crate::thermo::property_package::PropertyPackageModel) -> Result<SingleCompResult, SingleCompError> { /* ... */ }
```

#### Function `flash_pv`

**Pressure–vapour-fraction** single-component flash: return the saturation
temperature `Tsat(P)` at a specified vapour fraction `V`.

Ported from DWSIM `SingleCompFlash.vb:306` (`Flash_PV`, the `Tsat > Tfus`
liquid+vapour branch): the two-phase temperature of a pure fluid at pressure
`P` is `Tsat(P)`, independent of `V`. The solid+vapour branch is out of scope.

# Units / ranges
`pressure` `P` \[Pa\] > 0; `vapour_fraction` `V` \[-\] in `[0, 1]`. The
returned [`SingleCompResult`] has `temperature = Tsat(P)`, `pressure = P`,
`saturation_pressure = Psat(Tsat(P)) ≈ P`, and the phase set from `V`.

# Errors
As [`flash_tv`] (with `pressure` positivity instead of `temperature`).

```rust
pub fn flash_pv(components: &[crate::thermo::Component], z: &[f64], pressure: f64, vapour_fraction: f64, package: crate::thermo::property_package::PropertyPackageModel) -> Result<SingleCompResult, SingleCompError> { /* ... */ }
```

#### Function `flash_ph`

**Pressure–enthalpy** single-component flash: resolve temperature and vapour
fraction so the molar enthalpy meets `h_target` at fixed pressure `P`.

Ported from DWSIM `SingleCompFlash.vb:80` (`Flash_PH`), non-solid path only
(`SingleCompFlash.vb:151-199`). The method:

1. `Tsat = Tsat(P)` ([`saturation_temperature`], `SingleCompFlash.vb:86`).
2. Saturated molar enthalpies `HsatV = h(Tsat, P, Vapor)` and
   `HsatL = h(Tsat, P, Liquid)` (`SingleCompFlash.vb:92-93`).
3. Classify against the target `H`:
   - `H >= HsatV` ⇒ **superheated vapour** (`V = 1`); solve `h(T,P,Vapor) = H`
     for `T ≥ Tsat` (`SingleCompFlash.vb:151-158`).
   - `HsatL <= H < HsatV` ⇒ **two-phase** at `T = Tsat` with
     `V = (H − HsatL)/(HsatV − HsatL)` (`SingleCompFlash.vb:159-163`).
   - `H < HsatL` ⇒ **subcooled liquid** (`V = 0`); solve `h(T,P,Liquid) = H`
     for `T ≤ Tsat` (`SingleCompFlash.vb:191-199`).

The saturated latent heat `HsatV − HsatL` must be > 0 for the two-phase
branch (it is, for `T` below the critical point); if it is non-positive the
state is treated as single-phase by the `>=`/`<` comparisons.

## The enthalpy closure (model-dependent step, no `dyn`)

`molar_enthalpy(T, P, Phase) -> f64` returns the molar enthalpy \[J/mol\] of
the pure fluid in the given phase at `(T, P)` on **whatever reference scale
the caller uses** — the classification and the interior `V` depend only on
enthalpy *differences*, so any consistent datum works. It is a generic `Fn`,
not a trait object (crate no-`dyn` rule). A natural choice wraps the
ideal-gas Cp0 integral plus a cubic-EOS departure (see
[`crate::thermo::energy_flash`]); it must be monotone increasing in `T`
within a phase for the single-phase bisection to converge.

# Units / ranges
`components.len() == z.len()`; `pressure` `P` \[Pa\] > 0; `h_target` \[J/mol\]
on the closure's enthalpy scale. The returned [`SingleCompResult`] carries
the solved `T` (or `Tsat` in the two-phase branch), the input `P`, the vapour
fraction, and the classified phase. `saturation_pressure` reports
`Psat(Tsat(P))` (the vapour pressure at the boiling point for `P`, which
round-trips to `P`) — *not* `Psat` at the solved single-phase `T`, which
would be ill-posed for a deeply subcooled liquid.

# Errors
Input-validation errors as [`flash_pt`]; [`SingleCompError::Saturation`] if
the `Tsat`/`Psat` solve fails; [`SingleCompError::NoBracket`] /
[`SingleCompError::NotConverged`] if the single-phase temperature solve
cannot bracket or reach `h_target` within `opts`.

```rust
pub fn flash_ph<H>(components: &[crate::thermo::Component], z: &[f64], pressure: f64, h_target: f64, package: crate::thermo::property_package::PropertyPackageModel, molar_enthalpy: H, opts: SingleCompOptions) -> Result<SingleCompResult, SingleCompError>
where
    H: Fn(f64, f64, crate::thermo::cubic_eos::Phase) -> f64 { /* ... */ }
```

## Module `flash_sle`

Isothermal **solid-liquid-equilibrium (SLE) eutectic flash**.

Pure-Rust port of DWSIM's `NestedLoopsSLE.Flash_SL`
(`DWSIM.Thermodynamics/FlashAlgorithms/NestedLoopsSLE.vb:319-541`, GPL-3.0,
commit `1abf72d`). Given a feed and a temperature, it splits the mixture into
a **liquid solution** and one or more **precipitated pure solids** at
equilibrium (the *eutectic* model: each solid is a pure crystalline phase, no
solid solution).

# What this computes

For each component `i` the **maximum solubility** in the liquid is set by the
equilibrium between the pure solid and the dissolved species. Equating solid
and liquid fugacities and using a fusion (melting) thermodynamic cycle gives
DWSIM's relation (`NestedLoopsSLE.vb:334`, rendered here with the heat-capacity
term the source deliberately drops — see below):

```text
-ln(x_i^L gamma_i^L) = (dH_fus,i / (R T)) (1 - T / T_fus,i)
```

so the **activity at saturation** (the solid's fixed liquid-phase activity)
is the van't Hoff / Schröder-van-Laar ideal solubility

```text
a_i^sat = x_i^L gamma_i^L = exp[ -(dH_fus,i / R) (1/T - 1/T_fus,i) ]
```

and the **maximum liquid mole fraction** of `i` before it starts to
precipitate is `x_i^max = a_i^sat / gamma_i` (`NestedLoopsSLE.vb:416,439`).
Because `gamma_i` depends on the (unknown) liquid composition, the flash
iterates: evaluate `gamma` at the current liquid `x`, recompute the solubility
limits, rebalance moles between liquid and solid, and repeat until the liquid
fraction `L` stops moving.

The heat-capacity difference term
`-(dCp_i / R)[(T - T_fus,i)/T + ln(T_fus,i / T)]` is present in DWSIM's
equation but DWSIM **sets `dCp_i = 0`** in code
(`NestedLoopsSLE.vb:411`, comment *"ignoring heat capacity difference due to
issues with DWSIM characterization"*). This port mirrors that: `dCp` is not
used, so the solubility reduces to the two-term van't Hoff law above. The
(dropped) term is documented here for provenance only.

# Units (documented raw `f64`, SI — the DWSIM-internal convention)

| Quantity | Symbol | Unit |
|---|---|---|
| Temperature | `T`, `T_fus` | K |
| Enthalpy of fusion | `dH_fus` | J/mol |
| Gas constant | `R` | J/(mol·K) |
| Mole fractions | `z`, `x`, `s` | dimensionless |
| Liquid / solid molar fraction | `L`, `S = 1 - L` | dimensionless |

> **Unit note vs. DWSIM.** DWSIM stores the fusion enthalpy in **kJ/mol** and
> multiplies by `1000` inline (`Hf(i) * 1000`, `NestedLoopsSLE.vb:416`). This
> port takes `dH_fus` directly in **J/mol** — spell it out at the call site.

# Design (workspace + crate `CLAUDE.md`)

Enum dispatch through [`crate::thermo::activity::ActivityModel`] for the
liquid-phase `gamma` (no `dyn`, no trait objects, no `Box`, no lifetimes);
components are indexed by `usize`; the inner solubility/mass-balance loops are
raw `f64` arithmetic. Every public item documents its physical quantity,
valid ranges, and units.

# Honest scope — what is and is **not** ported

Ported: the **eutectic** `Flash_SL` isothermal SLE split (pure-solid phases),
the van't Hoff solubility limit, the supercritical-gas / ion / forced-solid
overrides (`NestedLoopsSLE.vb:443-447`), and the single-component
above/below-melting shortcut (`NestedLoopsSLE.vb:380-401`).

**Not** ported (present in the fuller `NestedLoopsSLE.vb`, out of scope here):

- **Solid-solution** flash `Flash_PT_SS` (`NestedLoopsSLE.vb:104-317`), where
  the solid is itself a mixed phase with its own distribution coefficients.
- The **solid-vapour-liquid** (SVLE) driver `Flash_PT_NL`
  (`NestedLoopsSLE.vb:543-934`) that wraps this SLE step inside a
  Rachford-Rice VLE loop, and the `Flash_PH/PS/TV/PV` energy/spec variants.
- The heat-capacity-difference (`dCp`) correction — dropped, matching DWSIM.
- DWSIM's fugacity-coefficient plumbing (`DW_CalcFugCoeff · P / Pvap`,
  `NestedLoopsSLE.vb:437`): this port takes the liquid activity coefficient
  `gamma` **directly** from [`crate::thermo::activity`], so no vapour-pressure
  data is needed for the pure solid-liquid split.

> **⚠️ Verification, not validation.** The tests below are *verification*:
> they check the port reproduces the analytic van't Hoff ideal-solubility law
> and a hand-computed binary eutectic split. They are **not** validated
> against experimental SLE data, and nothing here is cleared for nuclear /
> safety-critical use. AI-assisted draft — untrusted until human-reviewed per
> the crate `CLAUDE.md`.

```rust
pub mod flash_sle { /* ... */ }
```

### Types

#### Struct `SleComponent`

Per-component constant data an SLE flash needs beyond the liquid activity model.

The liquid-phase `gamma` comes from [`ActivityModel`]; this record carries the
**solid-phase** fusion properties and the special-case flags DWSIM applies
(`NestedLoopsSLE.vb:443-447`). Indexed positionally to match the `z` / `x` /
`s` vectors.

```rust
pub struct SleComponent {
    pub fusion_enthalpy: f64,
    pub fusion_temperature: f64,
    pub critical_temperature: f64,
    pub is_ion: bool,
    pub forced_solid: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fusion_enthalpy` | `f64` | Enthalpy of fusion `dH_fus` \[J/mol\] at the melting point. `<= 0` marks a<br>component with no solid phase (it never precipitates). |
| `fusion_temperature` | `f64` | Normal melting / fusion temperature `T_fus` \[K\]. `<= 0` marks a component<br>with no solid phase. |
| `critical_temperature` | `f64` | Critical temperature `T_c` \[K\]. Above it the component is a supercritical<br>gas and is forced entirely into the liquid (solubility limit lifted),<br>`NestedLoopsSLE.vb:445`. Use `f64::INFINITY` to disable this override. |
| `is_ion` | `bool` | Whether the component is a dissolved **ion** — ions are kept fully in the<br>liquid (`x_max = 1`), `NestedLoopsSLE.vb:443`. |
| `forced_solid` | `bool` | Whether the component is **forced to the solid phase** regardless of<br>solubility (`x_max = 0`), `NestedLoopsSLE.vb:447`. |

##### Implementations

###### Methods

- ```rust
  pub fn from_fusion(fusion_enthalpy: f64, fusion_temperature: f64) -> Self { /* ... */ }
  ```
  Component with only its fusion properties set, no overrides

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SleComponent { /* ... */ }
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
    fn eq(self: &Self, other: &SleComponent) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SleFlashResult`

Converged (or best-effort) result of an [`flash_sl`] solid-liquid split.

`x` and `s` are mole fractions \[-\] **within** their respective phases (each
sums to 1 when that phase is present). `liquid_fraction` is the molar fraction
of feed in the liquid; `solid_fraction = 1 - liquid_fraction`.

```rust
pub struct SleFlashResult {
    pub liquid_fraction: f64,
    pub solid_fraction: f64,
    pub x: Vec<f64>,
    pub s: Vec<f64>,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `liquid_fraction` | `f64` | Liquid molar fraction `L` \[-\], moles liquid per mole feed, in `[0, 1]`.<br>`1.0` = fully dissolved (no solid); `0.0` = fully frozen (no liquid). |
| `solid_fraction` | `f64` | Solid molar fraction `S = 1 - L` \[-\]. |
| `x` | `Vec<f64>` | Liquid-phase mole fractions `x_i` \[-\] (sum to 1 when `L > 0`, else all 0). |
| `s` | `Vec<f64>` | Solid-phase mole fractions `s_i` \[-\] (sum to 1 when `S > 0`, else all 0). |
| `iterations` | `usize` | Completed outer iterations (activity-coefficient updates). |

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
    fn clone(self: &Self) -> SleFlashResult { /* ... */ }
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
    fn eq(self: &Self, other: &SleFlashResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SleOptions`

Tuning parameters for [`flash_sl`].

```rust
pub struct SleOptions {
    pub max_iter: usize,
    pub tol: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iter` | `usize` | Maximum outer iterations before returning [`SleFlashError::NotConverged`].<br>DWSIM's `maxit_e` default is 100 (`NestedLoopsSLE.vb:521`). |
| `tol` | `f64` | Convergence tolerance on the liquid-fraction change `|L - L_old|` \[-\].<br>DWSIM's `MaxError = 1e-7` (`NestedLoopsSLE.vb:342`). |

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
    fn clone(self: &Self) -> SleOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &SleOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SleFlashError`

Error conditions for [`flash_sl`].

```rust
pub enum SleFlashError {
    Empty,
    LengthMismatch {
        z: usize,
        components: usize,
    },
    NonFinite,
    NonPositiveTemperature(f64),
    NotConverged {
        iterations: usize,
        residual: f64,
    },
}
```

##### Variants

###### `Empty`

An empty feed was supplied (need at least one component).

###### `LengthMismatch`

Two input slices that must be the same length were not.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `z` | `usize` | Length of `z`. |
| `components` | `usize` | Length of `components`. |

###### `NonFinite`

A non-finite value (`NaN`/`inf`) appeared in `z` or was produced mid-solve.

###### `NonPositiveTemperature`

The system temperature was non-positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `NotConverged`

The outer iteration did not converge within `max_iter`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations attempted. |
| `residual` | `f64` | Final liquid-fraction change `|L - L_old|`. |

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
    fn clone(self: &Self) -> SleFlashError { /* ... */ }
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: SleFlashError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SleFlashError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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

#### Function `ideal_solubility`

**Attributes:**

- `MustUse { reason: None }`

Ideal (van't Hoff / Schröder-van-Laar) solid solubility — the **activity at
saturation** of a pure solid in the liquid.

```text
a_i^sat = exp[ -(dH_fus / R) (1/T - 1/T_fus) ]
```

This is the mole-fraction solubility `x_i^sat` in the limit of an **ideal
liquid** (`gamma_i = 1`). Ported from `NestedLoopsSLE.vb:416` (with the `dCp`
term dropped as DWSIM does at `NestedLoopsSLE.vb:411`).

# Units / ranges

- `fusion_enthalpy` `dH_fus` \[J/mol\] — enthalpy of fusion at the melting
  point, `> 0` for a real solid.
- `fusion_temperature` `T_fus` \[K\] — normal melting point, `> 0`.
- `temperature` `T` \[K\] — system temperature, `> 0`.
- Returns the saturation activity \[-\], `> 0`. At `T = T_fus` it is exactly
  `1` (the pure melt); for `T < T_fus` it is `< 1` (partial solubility); for
  `T > T_fus` it exceeds `1` (the solid cannot exist — the component stays
  fully liquid).

Returns `1000.0` (a "never precipitates" sentinel, matching
`NestedLoopsSLE.vb:417`) when `T_fus` or `dH_fus` are non-positive/non-finite,
i.e. when the component has no characterised solid phase.

```rust
pub fn ideal_solubility(fusion_enthalpy: f64, fusion_temperature: f64, temperature: f64) -> f64 { /* ... */ }
```

#### Function `flash_sl`

Isothermal **eutectic solid-liquid-equilibrium flash** at fixed temperature.

Splits the feed `z` into a liquid solution and precipitated pure solids at
`temperature`, returning the phase mole fractions and the liquid fraction `L`.
Direct port of DWSIM `NestedLoopsSLE.Flash_SL`
(`NestedLoopsSLE.vb:319-541`).

# Method (successive substitution on the activity coefficient)

1. Start with all feed in the liquid (`x = z`, `L = 1`;
   `NestedLoopsSLE.vb:356`).
2. Evaluate `gamma_i` at the current liquid `x` and `T` via `activity`
   (`NestedLoopsSLE.vb:437`).
3. Solubility limit `x_i^max = a_i^sat / gamma_i` from [`ideal_solubility`]
   (`NestedLoopsSLE.vb:416,439`), with the overrides: ions and supercritical
   (`T > T_c`) components get `x_i^max = 1` (stay liquid); forced solids get
   `x_i^max = 0` (`NestedLoopsSLE.vb:443-447`).
4. Mole balance (`NestedLoopsSLE.vb:453-501`): components with
   `z_i <= x_i^max` dissolve completely (`n_i^L = z_i`); components with
   `z_i > x_i^max` are saturated (liquid mole fraction pinned at `x_i^max`,
   liquid moles `x_i^max · L`) and the excess precipitates. Enforcing that the
   liquid mole fractions sum to 1 gives
   `L = (Σ_{dissolved} z_i) / (1 - Σ_{saturated} x_i^max)`.
5. Renormalise the liquid (`x`) and solid (`s`) compositions and repeat from
   step 2 until `|L - L_old| < opts.tol`.

Single-component feeds short-circuit (`NestedLoopsSLE.vb:380-401`): above the
melting point → all liquid, below → all solid. A feed whose components all
lack solid data returns all-liquid (`NestedLoopsSLE.vb:361-368`).

# Units / ranges

- `z`: feed mole fractions \[-\], length `n >= 1`, finite; physical feeds sum
  to 1 (the routine normalises defensively).
- `components`: length `n`, fusion properties in J/mol and K (see
  [`SleComponent`]).
- `activity`: liquid-phase `gamma` model; its component count must be `n` for
  the non-ideal variants.
- `temperature` `T` \[K\], `> 0`.

# Errors

[`SleFlashError::Empty`] on empty `z`; [`SleFlashError::LengthMismatch`] if
`z.len() != components.len()`; [`SleFlashError::NonFinite`] on a non-finite
input or intermediate; [`SleFlashError::NonPositiveTemperature`] if `T <= 0`;
[`SleFlashError::NotConverged`] after `opts.max_iter` outer passes.

```rust
pub fn flash_sl(z: &[f64], components: &[SleComponent], activity: &crate::thermo::activity::ActivityModel, temperature: f64, opts: SleOptions) -> Result<SleFlashResult, SleFlashError> { /* ... */ }
```

### Constants and Statics

#### Constant `R_GAS`

CODATA molar gas constant `R = 8.314 462 618 153 24 J/(mol·K)`.

DWSIM hard-codes `8.31446` in `Flash_SL` (`NestedLoopsSLE.vb:416`); the extra
CODATA digits change the solubility by `< 1e-6` relative and are used here for
consistency with the rest of the crate.

```rust
pub const R_GAS: f64 = 8.314_462_618_153_24;
```

## Module `flash_svlle`

Isothermal-isobaric **solid + vapour-liquid-liquid equilibrium (SVLLE)**
global flash.

Pure-Rust port of DWSIM's `NestedLoopsSVLLE.Flash_PT`
(`DWSIM.Thermodynamics/FlashAlgorithms/NestedLoopsSVLLE.vb:63-241`, GPL-3.0,
commit `1abf72d`). Given a feed at fixed `T` \[K\] and `P` \[Pa\], it computes
the equilibrium split into up to **four coexisting phases** — one vapour, two
liquids, and one (eutectic, pure-solid) solid.

# What this module computes

DWSIM's SVLLE algorithm is a *composition* of three already-ported flashes,
not a new solver. It layers a solid-liquid-equilibrium precipitation on top of
the three-phase VLLE fluid split:

1. **Fluid split (V / L^{I} / L^{II}).** Run the three-phase VLLE flash
   ([`crate::thermo::flash_vlle::flash_pt_vlle`], itself a two-phase VLE flash
   plus a stability-driven liquid-liquid split — DWSIM `nl1` = `NestedLoops`
   and `nl2` = `NestedLoops3PV3`, `NestedLoopsSVLLE.vb:119-167`).
2. **Solid precipitation from each liquid.** For every liquid phase that
   exists, run the eutectic solid-liquid-equilibrium flash
   ([`crate::thermo::flash_sle::flash_sl`], DWSIM `nl3` = `NestedLoopsSLE` with
   `SolidSolution = False`, `NestedLoopsSVLLE.vb:171-205`). Each liquid `L^{j}`
   of fluid-phase fraction `L^{j}_0` is split by SLE into a remaining liquid
   (fraction `\ell^{j}` of that liquid) and a precipitated solid (fraction
   `1 - \ell^{j}`), so it contributes `L^{j} = \ell^{j} L^{j}_0` to the final
   liquid and `S^{j} = (1 - \ell^{j}) L^{j}_0` to the solid.
3. **Combine the two solids.** The final solid fraction is `S = S^{I} + S^{II}`
   and its composition is the mole-weighted average of the two precipitates,
   `s_i = (S^{I} s^{I}_i + S^{II} s^{II}_i) / S`.

The result satisfies the four-phase overall mole balance

```text
z_i = V y_i + L^{I} x^{I}_i + L^{II} x^{II}_i + S s_i,
V + L^{I} + L^{II} + S = 1,
```

because the vapour and each liquid balance is preserved by construction:
`L^{j}_0 x^{j}_{0,i} = L^{j} x^{j}_i + S^{j} s^{j}_i` (the SLE mole balance),
and the VLLE step already closes `z_i = V y_i + Σ_j L^{j}_0 x^{j}_{0,i}`.

# Units (documented raw `f64`, SI — the DWSIM-internal convention)

| Quantity | Symbol | Unit |
|---|---|---|
| Temperature | `T` | K |
| Pressure | `P` | Pa |
| Mole fractions | `z`, `y`, `x^{j}`, `s` | dimensionless \[-\] |
| Phase molar fractions | `V`, `L^{I}`, `L^{II}`, `S` | dimensionless \[-\] |
| Enthalpy of fusion | `dH_fus` (in [`SleComponent`]) | J/mol |

# Design (workspace + crate `CLAUDE.md`)

Enum dispatch throughout — the fugacity model is the [`CubicEos`] **enum** and
the liquid `gamma` is the [`ActivityModel`] **enum**; no trait objects, no
`dyn`, no `Box`, no lifetimes, no channels. `#![forbid(unsafe_code)]` at the
crate root. Compositions owned by value; documented raw `f64` (SI) in the
composition arithmetic. Every public item documents its physical quantity,
valid ranges, and units.

# Honest scope — what is and is **not** ported

> **⚠️ Untrusted AI-assisted draft pending human V&V.** This is
> **verification** (does the port reproduce the DWSIM composition algebra and
> close the mass balances?), **not validation** against measured SVLLE data.
> `k_ij = 0` throughout. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
> the official DWSIM.

**Ported:** the non-forced-solids `Flash_PT` orchestration — VLLE fluid split,
per-liquid eutectic solid precipitation, and the two-solid combination
(`NestedLoopsSVLLE.vb:117-205`).

**Not ported (present in `NestedLoopsSVLLE.vb`, out of scope here):**

- **Forced-solids path** (`NestedLoopsSVLLE.vb:91-116`, and the `Flash_PV`
  forced-solids branch `263-305`): DWSIM lets the caller pin named compounds
  into the solid phase and flashes the remainder solids-free. This port takes
  no `ForcedSolids` set; precipitation is governed entirely by the fusion
  thermodynamics in [`SleComponent`].
- **The no-liquid SVLE branch** (`NestedLoopsSVLLE.vb:207-224`): when the VLLE
  step yields essentially no liquid (`L^{I} <= min_liquid_fraction`), DWSIM
  runs `NestedLoopsSLE.Flash_PT` — the solid-vapour-liquid driver
  (`NestedLoopsSLE.vb:543-934`) which is **itself not ported**
  (see [`crate::thermo::flash_sle`] honest scope). This port therefore returns
  the VLLE result **solid-free** in that regime and flags it via
  [`SvlleResult::no_liquid_svle_skipped`]. Direct vapour→solid deposition is
  not modelled.
- **`Flash_PH` / `Flash_PS` / `Flash_PV` / `Flash_TV`** energy- and
  specification-based variants (`NestedLoopsSVLLE.vb:243-309`).
- **Gibbs re-ordering / labelling of the two liquids** — inherited from
  [`crate::thermo::flash_vlle`]: which fluid liquid is `L^{I}` vs `L^{II}` is
  **not** physically canonical (that needs an absolute-fugacity closure the
  K-only interface does not expose). Mass balance and the sum-to-one identities
  (the V&V checks) are independent of that labelling.

**Documented deviation from DWSIM (a correction).** DWSIM combines the two
solid compositions with weights `(S^{I}, 1-\ell^{II})` —
`Vs = Vs·S + s^{II}·result(1)`, `NestedLoopsSVLLE.vb:198` — omitting the
`L^{II}_0` factor on the second precipitate, so its weight is a
*per-mole-of-liquid-II* fraction rather than a *per-mole-of-feed* fraction.
That makes the reported solid composition inexact whenever both liquids
precipitate and `L^{II}_0 ≠ 1`. This port uses the physically correct
feed-basis weights `(S^{I}, S^{II}) = (S^{I}, (1-\ell^{II}) L^{II}_0)`, which
is what makes the overall mole balance close exactly (V&V below).

```rust
pub mod flash_svlle { /* ... */ }
```

### Types

#### Struct `SvlleOptions`

Tuning parameters for [`flash_pt_svlle`].

Bundles the sub-flash option records plus the DWSIM "is there a liquid worth
precipitating from?" gate.

```rust
pub struct SvlleOptions {
    pub vlle: crate::thermo::flash_vlle::VlleOptions,
    pub sle: crate::thermo::flash_sle::SleOptions,
    pub min_liquid_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vlle` | `crate::thermo::flash_vlle::VlleOptions` | Options for the three-phase VLLE fluid split<br>([`crate::thermo::flash_vlle::flash_pt_vlle`]). |
| `sle` | `crate::thermo::flash_sle::SleOptions` | Options for the per-liquid eutectic SLE precipitation<br>([`crate::thermo::flash_sle::flash_sl`]). |
| `min_liquid_fraction` | `f64` | Minimum liquid molar fraction \[-\] below which a fluid liquid is treated<br>as absent (no solid is precipitated from it). DWSIM gates the whole<br>solid-precipitation path on `L^{I} > 0.001` (`NestedLoopsSVLLE.vb:130`);<br>this port uses the same `1e-3` default for **both** liquids. |

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
    fn clone(self: &Self) -> SvlleOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &SvlleOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SvlleResult`

A converged (or best-effort) SVLLE flash result: up to four coexisting phases.

Phase molar fractions are \[-\] and satisfy `v + l1 + l2 + s = 1`. Each phase
composition is a vector of mole fractions \[-\] that sums to 1 **when that
phase is present** (else all zero). The overall mole balance
`z_i = v·y_i + l1·x1_i + l2·x2_i + s·vs_i` holds to solver tolerance.

The `l1`/`l2` (and `x1`/`x2`) labelling is **not** Gibbs-ordered — see the
module scope note. Only mass balance and the sum-to-one identities are
label-independent.

```rust
pub struct SvlleResult {
    pub v: f64,
    pub l1: f64,
    pub l2: f64,
    pub s: f64,
    pub y: Vec<f64>,
    pub x1: Vec<f64>,
    pub x2: Vec<f64>,
    pub vs: Vec<f64>,
    pub three_phase_fluid: bool,
    pub solid_present: bool,
    pub no_liquid_svle_skipped: bool,
    pub vlle_iterations: usize,
    pub sle_iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `v` | `f64` | Vapour molar fraction `V` \[-\] ∈ `[0, 1]`. |
| `l1` | `f64` | First-liquid molar fraction `L^{I}` \[-\] ∈ `[0, 1]` (after solidification). |
| `l2` | `f64` | Second-liquid molar fraction `L^{II}` \[-\] ∈ `[0, 1]` (after<br>solidification); `0.0` when no second liquid formed. |
| `s` | `f64` | Total solid molar fraction `S` \[-\] ∈ `[0, 1]`; `0.0` when nothing<br>precipitated. |
| `y` | `Vec<f64>` | Vapour mole fractions `y_i` \[-\] (sum to 1 when `v > 0`). |
| `x1` | `Vec<f64>` | First-liquid mole fractions `x^{I}_i` \[-\] (sum to 1 when `l1 > 0`). |
| `x2` | `Vec<f64>` | Second-liquid mole fractions `x^{II}_i` \[-\] (sum to 1 when `l2 > 0`);<br>equals `x1` semantics only when `l2 = 0` (then all-zero if the split<br>collapsed). |
| `vs` | `Vec<f64>` | Solid mole fractions `s_i` \[-\] (sum to 1 when `s > 0`, else all zero) —<br>the mole-weighted average of the two liquids' precipitates. |
| `three_phase_fluid` | `bool` | `true` iff a distinct second liquid was detected in the fluid split. |
| `solid_present` | `bool` | `true` iff any solid precipitated (`s > 0`). |
| `no_liquid_svle_skipped` | `bool` | `true` iff the fluid split left essentially no liquid<br>(`L^{I} <= min_liquid_fraction`) so the unported no-liquid SVLE branch was<br>**skipped** and the result is reported solid-free. See the module scope<br>note. `false` in the normal (liquid-present) case. |
| `vlle_iterations` | `usize` | Completed outer iterations of the VLLE fluid split. |
| `sle_iterations` | `usize` | Completed outer iterations of the SLE precipitation from liquid I<br>(`0` if no first liquid was precipitated from). |

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
    fn clone(self: &Self) -> SvlleResult { /* ... */ }
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
    fn eq(self: &Self, other: &SvlleResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SvlleFlashError`

Error conditions for [`flash_pt_svlle`].

```rust
pub enum SvlleFlashError {
    LengthMismatch {
        z: usize,
        components: usize,
        sle: usize,
    },
    NonFinite,
    Vlle(crate::thermo::flash::FlashError),
    Sle(crate::thermo::flash_sle::SleFlashError),
}
```

##### Variants

###### `LengthMismatch`

Two input slices that must all be the same length were not.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `z` | `usize` | Length of `z`. |
| `components` | `usize` | Length of `components`. |
| `sle` | `usize` | Length of `sle_components`. |

###### `NonFinite`

A non-finite value appeared in a phase fraction during the solve.

###### `Vlle`

The three-phase VLLE fluid split failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::thermo::flash::FlashError` |  |

###### `Sle`

A solid-liquid precipitation sub-flash failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::thermo::flash_sle::SleFlashError` |  |

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
    fn clone(self: &Self) -> SvlleFlashError { /* ... */ }
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
  - ```rust
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private18::Error + ''static> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: FlashError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: SleFlashError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SvlleFlashError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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

#### Function `flash_pt_svlle`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Full **solid + vapour-liquid-liquid equilibrium (SVLLE)** isothermal-isobaric
flash of feed `z` at `T` \[K\], `P` \[Pa\].

Direct composition port of DWSIM `NestedLoopsSVLLE.Flash_PT`
(`NestedLoopsSVLLE.vb:63-241`, non-forced-solids path): a three-phase VLLE
fluid split followed by eutectic solid precipitation from each liquid, then a
two-solid combination. See the module header for the full algorithm and the
honest scope.

# Arguments / units

- `components`: EOS constant-property records (critical `T`/`P`, acentric
  factor, …), length `n`. Drives the vapour and liquid fugacities.
- `sle_components`: fusion properties (`dH_fus` \[J/mol\], `T_fus` \[K\]) and
  solid-phase override flags, length `n`, positionally paired with
  `components`. A component with no solid data never precipitates.
- `activity`: liquid-phase `gamma` model used **only** in the SLE
  precipitation step (the VLLE step uses the cubic EOS). Its component count
  must be `n` for the non-ideal variants.
- `z`: feed mole fractions \[-\], length `n ≥ 1`, finite (normalised
  defensively downstream).
- `t` \[K\] `> 0`, `p` \[Pa\] `> 0`.
- `eos`: the [`CubicEos`] fugacity model (`k_ij = 0`).
- `opts`: sub-flash tolerances and the liquid-presence gate.

# Returns

An [`SvlleResult`] with `v + l1 + l2 + s = 1`, each present phase composition
summing to 1, and the overall mole balance
`z_i = v·y_i + l1·x1_i + l2·x2_i + s·vs_i` closing to solver tolerance.

# Errors

[`SvlleFlashError::LengthMismatch`] if `components`, `sle_components`, and `z`
are not all the same length; [`SvlleFlashError::Vlle`] / [`SvlleFlashError::Sle`]
propagated from the sub-flashes; [`SvlleFlashError::NonFinite`] on a non-finite
intermediate phase fraction.

```rust
pub fn flash_pt_svlle(components: &[crate::thermo::Component], sle_components: &[crate::thermo::flash_sle::SleComponent], activity: &crate::thermo::activity::ActivityModel, z: &[f64], t: f64, p: f64, eos: crate::thermo::cubic_eos::CubicEos, opts: SvlleOptions) -> Result<SvlleResult, SvlleFlashError> { /* ... */ }
```

## Module `flash_vlle`

Three-phase **vapour-liquid-liquid equilibrium** (VLLE) isothermal-isobaric
(**PT**) flash via the two-equation nested-loops method.

Ported from DWSIM `DWSIM.Thermodynamics/FlashAlgorithms/NestedLoops3PV3.vb`
(`Flash_PT` orchestration lines 98-185, `Flash_PT_3P` core lines 349-739),
GPL-3.0, commit `1abf72d`, with the second-liquid detection from
`BaseFlashAlgorithm.vb` `GetPhaseSplitEstimates` (lines 1233-1282). Specific
ported lines are cited at each function below.

# Provenance

```text
Upstream project : DWSIM (Daniel Wagner O. de Medeiros; Gregor Reichert)
Source file      : DWSIM.Thermodynamics/FlashAlgorithms/NestedLoops3PV3.vb
                   DWSIM.Thermodynamics/FlashAlgorithms/BaseFlashAlgorithm.vb
Commit           : 1abf72d
Licence          : GPL-3.0
```

# What this module computes

Given a feed `z_i` \[-\] at fixed `T` \[K\], `P` \[Pa\], split it into up to
three coexisting phases — a vapour (`y_i`, fraction `V`) and two liquids
(`x^{I}_i`, fraction `L^{I}`; `x^{II}_i`, fraction `L^{II}`) with
`V + L^{I} + L^{II} = 1`, in mutual equilibrium
(`φ_i^V y_i = φ_i^{L I} x^{I}_i = φ_i^{L II} x^{II}_i`).

# Method (the two Rachford-Rice equations)

Writing `K^{j}_i = φ_i^{L j} / φ_i^{V}` for liquid `j ∈ {I, II}` and
`β^{j}_i = 1 − 1/K^{j}_i` (DWSIM `NestedLoops3PV3.vb` lines 544-545), the
material balance is

```text
z_i = y_i (1 − β^{I}_i L^{I} − β^{II}_i L^{II}),   y_i = z_i / D_i,
```

with `D_i = 1 − β^{I}_i L^{I} − β^{II}_i L^{II}` (DWSIM line 546) and
`x^{I}_i = y_i / K^{I}_i`, `x^{II}_i = y_i / K^{II}_i` (lines 547-548). The two
phase fractions solve the coupled Rachford-Rice pair

```text
F_1(L^{I}, L^{II}) = Σ_i β^{I}_i z_i / D_i = 0,
F_2(L^{I}, L^{II}) = Σ_i β^{II}_i z_i / D_i = 0
```

(DWSIM lines 606-607), which force `Σ x^{I} = Σ y` and `Σ x^{II} = Σ y`. They
are solved by a damped 2×2 Newton iteration (DWSIM lines 620-654); the
K-values are refreshed from the rigorous EOS each outer pass (lines 523-534).

Note the identity: for **any** `(L^{I}, L^{II})`, the un-normalised
`z_i = V y_i + L^{I} x^{I}_i + L^{II} x^{II}_i` holds exactly with
`V = 1 − L^{I} − L^{II}` (substitute `x^{j}_i = y_i/K^{j}_i` and
`β^{j}_i = 1 − 1/K^{j}_i`). `F_1 = F_2 = 0` additionally makes the three
normalised compositions each sum to 1, so overall mass balance closes on the
**normalised** phases too — this is V&V check (2) below.

# Orchestration (when three phases appear)

[`flash_pt_vlle`] mirrors DWSIM `Flash_PT` (lines 145-181): first a rigorous
**two-phase VLE** flash ([`crate::thermo::flash::nested_loops_flash`]); then,
if a liquid exists, a **phase-stability test** on that liquid
([`crate::thermo::stability::stability_test`], the analogue of DWSIM's
`StabTest2` inside `GetPhaseSplitEstimates`) to detect a distinct second
liquid. Only if the liquid is unstable does the three-phase Newton solve
[`solve_3p_fixed_k`] run; otherwise the two-phase result is returned unchanged
(VLLE with `L^{II} = 0`).

# Honest scope (verification, not benchmark validation, and a *partial* port)

Three-phase flash robustness is genuinely hard, and this is a **first port**:

- **Second-liquid detection is only as good as the two Wilson-seeded stability
  trials** ([`crate::thermo::stability`]); a liquid-liquid split that neither
  Wilson seed reaches will be missed and the flash silently returns two
  phases. No global TPD minimisation, no third "ideal-mix" seed.
- **No `SimpleLLE` / Gibbs-minimisation fallback.** DWSIM switches to a
  `SimpleLLE` solver when the vapour vanishes (line 678) and does a final
  Gibbs-energy re-ordering of the two liquids (lines 726-737). This port does
  **neither**: if the vapour collapses it returns the LL-only estimate as-is,
  and it does **not** order the two liquids by Gibbs energy (that needs an
  absolute-fugacity closure this K-only interface does not expose) — so which
  liquid is labelled `L^{I}` vs `L^{II}` is **not** physically canonical.
  Mass balance and the sum-to-one identities (the V&V checks) are independent
  of that labelling.
- **`k_ij = 0`** throughout (geometric-mean mixing), which makes a genuine
  liquid-liquid split under a cubic EOS with the bundled reference compounds
  unlikely; the three-phase numerics are therefore verified on the
  **fixed-K** core [`solve_3p_fixed_k`] against the algebraic mass-balance
  identity, and the composed driver is verified to **reduce to the two-phase
  result** when no second liquid is found. A full EOS-driven LLE benchmark is
  deferred.

> **⚠️ Unverified until validated.** AI-assisted **partial** port — untrusted
> draft material until human-reviewed per the crate `CLAUDE.md`. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions. Independent OUTRAM PARK fork, not the official DWSIM.

# Design (workspace + crate `CLAUDE.md`)

Enum dispatch (the fugacity model is the [`CubicEos`] **enum**), no trait
objects / `dyn` / `Box` / lifetimes / channels. Compositions owned by value;
documented raw `f64` (SI: K, Pa, mole fractions \[-\]) in the inner loops.

```rust
pub mod flash_vlle { /* ... */ }
```

### Types

#### Struct `VlleOptions`

Tuning parameters for [`flash_pt_vlle`] and [`solve_3p_fixed_k`].

```rust
pub struct VlleOptions {
    pub max_outer_iter: usize,
    pub f_tol: f64,
    pub change_tol: f64,
    pub min_phase_fraction: f64,
    pub max_damping: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_outer_iter` | `usize` | Maximum outer (rigorous-K + Newton) iterations before returning<br>[`FlashError::NotConverged`]. DWSIM `maxit_e` default is 100. |
| `f_tol` | `f64` | Convergence tolerance on `|F_1| + |F_2|` \[-\], the summed Rachford-Rice<br>residuals of the two liquid-fraction equations (DWSIM `etol`). |
| `change_tol` | `f64` | Convergence tolerance on the total per-pass composition + phase-fraction<br>change \[-\] (DWSIM line 586 uses `1e-10`). |
| `min_phase_fraction` | `f64` | A liquid phase whose fraction falls below this \[-\] is treated as absent<br>(the split has collapsed back to two phases). |
| `max_damping` | `f64` | Newton per-step damping cap \[-\]: the fractional change in each liquid<br>fraction is limited to this (DWSIM line 646 uses `0.1`). |

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
    fn clone(self: &Self) -> VlleOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &VlleOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `VlleResult`

A converged (or best-effort) three-phase VLLE flash result.

Phase fractions are molar \[-\] and satisfy `v + l1 + l2 = 1`; compositions are
mole fractions \[-\] each summing to 1. When no second liquid is present
`l2 = 0`, `x2` mirrors `x1`, and the result is the two-phase VLE split.

The `l1`/`l2` (and `x1`/`x2`, `k1`/`k2`) labelling is **not** Gibbs-ordered —
see the module scope note. Only mass balance and the sum-to-one identities are
label-independent.

```rust
pub struct VlleResult {
    pub v: f64,
    pub l1: f64,
    pub l2: f64,
    pub y: Vec<f64>,
    pub x1: Vec<f64>,
    pub x2: Vec<f64>,
    pub k1: Vec<f64>,
    pub k2: Vec<f64>,
    pub three_phase: bool,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `v` | `f64` | Vapour molar fraction `V` \[-\] ∈ `[0, 1]`. |
| `l1` | `f64` | First-liquid molar fraction `L^{I}` \[-\] ∈ `[0, 1]`. |
| `l2` | `f64` | Second-liquid molar fraction `L^{II}` \[-\] ∈ `[0, 1]`; `0.0` when no<br>second liquid was detected (two-phase reduction). |
| `y` | `Vec<f64>` | Vapour mole fractions `y_i` \[-\] (sum to 1). |
| `x1` | `Vec<f64>` | First-liquid mole fractions `x^{I}_i` \[-\] (sum to 1). |
| `x2` | `Vec<f64>` | Second-liquid mole fractions `x^{II}_i` \[-\] (sum to 1); equals `x1` when<br>`l2 = 0`. |
| `k1` | `Vec<f64>` | First-liquid K-values `K^{I}_i = y_i / x^{I}_i` \[-\]. |
| `k2` | `Vec<f64>` | Second-liquid K-values `K^{II}_i = y_i / x^{II}_i` \[-\]; equals `k1` when<br>`l2 = 0`. |
| `three_phase` | `bool` | `true` iff a distinct second liquid was detected and retained. |
| `iterations` | `usize` | Number of completed outer iterations of the three-phase Newton solve<br>(`0` when the flash reduced to two phases without entering it). |

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
    fn clone(self: &Self) -> VlleResult { /* ... */ }
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
    fn eq(self: &Self, other: &VlleResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ThreePhaseSplit`

The converged fixed-K three-phase split (compositions + phase fractions).

```rust
pub struct ThreePhaseSplit {
    pub v: f64,
    pub l1: f64,
    pub l2: f64,
    pub y: Vec<f64>,
    pub x1: Vec<f64>,
    pub x2: Vec<f64>,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `v` | `f64` | Vapour molar fraction `V` \[-\]. |
| `l1` | `f64` | First-liquid molar fraction `L^{I}` \[-\]. |
| `l2` | `f64` | Second-liquid molar fraction `L^{II}` \[-\]. |
| `y` | `Vec<f64>` | Vapour mole fractions `y_i` \[-\] (sum to 1). |
| `x1` | `Vec<f64>` | First-liquid mole fractions `x^{I}_i` \[-\] (sum to 1). |
| `x2` | `Vec<f64>` | Second-liquid mole fractions `x^{II}_i` \[-\] (sum to 1). |
| `iterations` | `usize` | Completed Newton iterations. |

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
    fn clone(self: &Self) -> ThreePhaseSplit { /* ... */ }
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
    fn eq(self: &Self, other: &ThreePhaseSplit) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `eos_k_values`

**Attributes:**

- `MustUse { reason: None }`

K-values `K_i = φ_i^L(x) / φ_i^V(y) = exp(ln φ_i^L − ln φ_i^V)` \[-\] from the
cubic EOS, liquid root for `x`, vapour root for `y` (`k_ij = 0`).

Mirrors [`crate::thermo::property_package::PropertyPackageModel`]'s cubic
K-update. Falls back to the Wilson estimate if either phase yields no usable
`Z`-root, so the driver stays well-posed. `t` \[K\] > 0, `p` \[Pa\] > 0.

```rust
pub fn eos_k_values(eos: crate::thermo::cubic_eos::CubicEos, components: &[crate::thermo::Component], x: &[f64], y: &[f64], t: f64, p: f64) -> Vec<f64> { /* ... */ }
```

#### Function `solve_3p_fixed_k`

Solve the three-phase split for **fixed** liquid K-vectors `K^{I}`, `K^{II}`.

This is the numerical core of DWSIM `Flash_PT_3P` (lines 496-705) with the
K-values held constant (no property-model call): a damped 2×2 Newton iteration
on the coupled Rachford-Rice pair `F_1 = F_2 = 0` for `(L^{I}, L^{II})`,
recovering the phase compositions each pass.

# Units / ranges

`z`, `k1`, `k2`: equal length `n ≥ 1`; `z` feed mole fractions \[-\]; `k1`,
`k2` liquid/vapour K-values \[-\] (`> 0`). `l1_est`, `l2_est` ∈ `(0, 1)` with
`l1_est + l2_est < 1` seed the Newton iteration. `opts` bounds the iteration
and tolerances.

# Returns

A [`ThreePhaseSplit`] with `v = 1 − L^{I} − L^{II}` and the normalised
compositions. On convergence `F_1 = F_2 = 0` to `opts.f_tol`, so `Σ y = Σ x^I
= Σ x^II = 1` and the overall mass balance `z_i = v y_i + L^{I} x^{I}_i +
L^{II} x^{II}_i` closes.

# Errors

[`FlashError::Empty`] on empty `z`; [`FlashError::LengthMismatch`] on a size
mismatch; [`FlashError::NonFinite`] on a non-finite / non-positive K;
[`FlashError::NotConverged`] if the Newton iteration does not reach `f_tol`
within `opts.max_outer_iter`.

```rust
pub fn solve_3p_fixed_k(z: &[f64], k1: &[f64], k2: &[f64], l1_est: f64, l2_est: f64, opts: VlleOptions) -> Result<ThreePhaseSplit, crate::thermo::flash::FlashError> { /* ... */ }
```

#### Function `flash_pt_vlle`

Full **three-phase VLLE** isothermal-isobaric flash of feed `z` at `T` \[K\],
`P` \[Pa\] using the cubic EOS `eos` (`k_ij = 0`).

Orchestration (DWSIM `NestedLoops3PV3.vb` `Flash_PT`, lines 145-181):

1. **Two-phase VLE** ([`crate::thermo::flash::nested_loops_flash`]) with the
   EOS K-closure ([`eos_k_values`]).
2. If a liquid exists, **stability test** it
   ([`crate::thermo::stability::stability_test`]). Stable ⇒ return the
   two-phase result (`l2 = 0`).
3. Unstable ⇒ build a second-liquid estimate ([`phase_split_estimate`]) and
   run the **three-phase Newton solve** (an [`solve_3p_fixed_k`] Newton step
   with the K-vectors refreshed from the EOS each outer pass, DWSIM
   lines 496-705).
4. If the second liquid collapses below `opts.min_phase_fraction`, fall back
   to the two-phase result.

# Units / ranges

`components.len() == z.len()`; `z` feed mole fractions \[-\] (sum to 1);
`t` \[K\] > 0, `p` \[Pa\] > 0. See the module scope note for the honest limits
(label ordering, missed splits, no `SimpleLLE` fallback).

# Errors

Propagates [`FlashError`] from the two-phase flash and the three-phase Newton
solve; [`FlashError::LengthMismatch`] on a `components`/`z` size mismatch.

```rust
pub fn flash_pt_vlle(components: &[crate::thermo::Component], z: &[f64], t: f64, p: f64, eos: crate::thermo::cubic_eos::CubicEos, opts: VlleOptions) -> Result<VlleResult, crate::thermo::flash::FlashError> { /* ... */ }
```

## Module `gibbs`

Gibbs-energy-minimisation **speciation** flash (single gas phase, reacting
mixture) — equilibrium composition by direct minimisation of the total Gibbs
energy subject to **element / atom mass-balance** constraints, *without* an
explicit list of reactions.

## Provenance (GPL-3.0)

Ported / adapted from DWSIM (GPL-3.0), commit
`1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`:
- `DWSIM.Thermodynamics/FlashAlgorithms/GibbsMinimization3P.vb`
- `DWSIM.Thermodynamics/FlashAlgorithms/GibbsMinimizationMulti.vb`

Both DWSIM files state the objective as (`GibbsMinimization3P.vb:146`,
`GibbsMinimizationMulti.vb:132`):

```text
Q = Σ_k Σ_i n_ik · ln f_ik
```

i.e. the sum over phases `k` and species `i` of moles times the natural log
of the species fugacity, with gradient (`GibbsMinimization3P.vb:154`,
`GibbsMinimizationMulti.vb:140`)

```text
∂Q/∂n_ij = ln f_ij − ln f_iF
```

and the numeric Gibbs energy assembled in `FunctionValue` as
`Σ_i n_i · ln(φ_i · y_i)` per phase (`GibbsMinimization3P.vb:760-761,
838-840`; `GibbsMinimizationMulti.vb:802-805`). DWSIM's flash minimises that
objective **over a phase split** (the constraint is species mass balance
across phases, `n_iF = z_i − Σ_{k<F} n_ik`, `GibbsMinimization3P.vb:150`) and
delegates the constrained optimisation to an **external** solver — IPOPT via
`Cureos.Numerics` (`GibbsMinimization3P.vb:20`, the `Solver` property
defaults to `OptimizationMethod.IPOPT`, `:66`).

## What THIS module does (and how it differs from the DWSIM file)

This port keeps DWSIM's Gibbs objective and its `μ_i = μ°_i + RT ln(φ_i y_i
P/P°)` chemical-potential model, but targets the **reacting-mixture
speciation** problem the HTGR-water-ingress use-case needs (equilibrium
CO / CO₂ / H₂ / H₂O partitioning; see the crate
`docs/chemistry-model-survey.md` §4a): a **single** gas phase whose species
may interconvert by any reaction the atoms allow, constrained by
**element/atom** balance rather than by a phase split. This is the
"non-stoichiometric" (element-abundance) Gibbs minimisation — the same
formulation DWSIM's own Gibbs **reactor** (`Reactors/Gibbs`) uses, and the
standard White–Johnson–Dantzig / **RAND** element-potential method (Smith &
Missen, *Chemical Reaction Equilibrium Analysis*, 1982; NASA CEA, Gordon &
McBride, RP-1311).

Because DWSIM's flash uses an external optimiser (IPOPT) that this workspace
must not add as a dependency, the constrained optimiser here is written **in
pure Rust**: a damped RAND / element-potential Newton iteration (below). No
external optimisation crate is used.

## The minimisation problem

Minimise the dimensionless total Gibbs energy of one ideal-solution gas phase

```text
G/RT = Σ_i n_i · (μ_i / RT),   μ_i/RT = g°_i/(RT) + ln φ_i + ln(y_i P/P°)
```

(`g°_i` = standard molar Gibbs energy of formation of species `i` at the
system temperature `T` \[J/mol\]; `y_i = n_i / Σ_l n_l`; `φ_i` = fugacity
coefficient, `= 1` for an ideal gas) subject to, for every chemical element
`k`,

```text
Σ_i a_ki · n_i = b_k     (atom balance),   n_i ≥ 0
```

where `a_ki` is the number of atoms of element `k` in one molecule of species
`i` and `b_k` is the total number of moles of element `k` supplied by the
feed. The Lagrange stationarity condition of this problem is the
**element-potential relation**

```text
μ_i / RT = Σ_k a_ki · π_k     for every species i,
```

with `π_k` the (dimensionless) element potential `= λ_k/RT` of element `k`.
For **any** reaction `Σ_i ν_i·Aᵢ = 0` that conserves atoms
(`Σ_i ν_i a_ki = 0`) this immediately gives
`Σ_i ν_i μ_i = 0`, i.e. `Π_i (φ_i y_i P/P°)^{ν_i} = exp(−ΔG°/RT) = K_eq` —
the correct chemical-equilibrium constant, with no reaction ever having been
listed. That identity is what the V&V tests below check analytically.

## The RAND / element-potential iteration (pure Rust)

At a strictly-positive current estimate `n_i`, with `n_t = Σ_i n_i`, form the
`(M+1)×(M+1)` linear system for the `M` element potentials `π_k` and the
total-mole correction `u = Δ ln n_t`:

```text
Σ_j r_kj π_j + q_k u = s_k + (b_k − q_k)     (one row per element k)
Σ_j q_j π_j          = f_g                    (total-mole row)
```

with `q_k = Σ_i a_ki n_i` (current element abundance),
`r_kj = Σ_i a_ki a_ji n_i`, `s_k = Σ_i a_ki n_i (μ_i/RT)`, and
`f_g = Σ_i n_i (μ_i/RT) = G/RT`. The species correction is then

```text
δ_i = Δ ln n_i = Σ_k a_ki π_k + u − μ_i/RT,
n_i ← n_i · exp(ω · δ_i),   ω = min(1, max_step / max_i|δ_i|).
```

The **multiplicative** update keeps every `n_i > 0` for free (no line search
against a positivity boundary), and the `(b_k − q_k)` residual on the
element rows drives the atom balance back onto its target each step, so the
iteration is self-correcting against round-off drift. At convergence `u → 0`
and `δ_i → 0`, recovering `μ_i/RT = Σ_k a_ki π_k` exactly (derivation in the
test module doc). The `(M+1)` system is solved with an in-crate Gaussian
elimination (partial pivoting) — no BLAS/LAPACK, so this compiles on Android
/ Termux like the rest of the non-GUI crate.

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

Raw `f64` in SI throughout the inner loop (the crate "raw f64 in inner
EOS/flash loops" rule): temperature `T` \[K\], pressure `P` and reference
pressure `P°` \[Pa\], standard Gibbs energies `g°_i` \[J/mol\], moles \[mol\],
mole fractions and element potentials dimensionless. The gas constant is the
crate-shared CODATA value [`crate::thermo::ideal_props::R`].

## Design (workspace + crate `CLAUDE.md`)

Enum dispatch, **no `dyn` / `Box` / lifetimes**: the fugacity model is the
closed enum [`FugacityModel`]; the system, options, result, and errors are
plain owned structs / enums. Raw `f64` maths inside.

## Honest scope — untrusted AI-assisted draft, pending human V&V

- **Single gas phase only.** No vapour–liquid(–liquid) split (DWSIM's flash
  does that; it is a separate concern) and **no condensed / solid phase**.
  The Boudouard equilibrium `C(s) + CO₂ ⇌ 2 CO` and steam–graphite
  `C(s) + H₂O ⇌ CO + H₂` therefore need a solid-carbon activity term that is
  **not** implemented here — this module covers the *gas-phase* speciation of
  the HTGR water-ingress system (water-gas shift `CO + H₂O ⇌ CO₂ + H₂` is
  fully in scope). Solid-carbon coupling is future work (see the crate
  survey, `NestedLoopsSLE` / `Reactors/Gibbs`).
- **Ideal solution by default.** [`FugacityModel::IdealGas`] sets `φ_i = 1`.
  [`FugacityModel::FrozenLnPhi`] applies a caller-supplied, **constant**
  `ln φ_i` correction (a *frozen*-fugacity approximation — the coefficients
  are held fixed through the minimisation, not recomputed from an EOS at each
  composition). A self-consistent EOS coupling (recomputing `φ_i(y,T,P)` from
  [`crate::thermo::cubic_eos`] every iteration) is **not** wired here; it is
  a documented extension point.
- **Verified, not validated.** The tests below are analytic **verification**
  (equilibrium constant + atom balance + pressure/Le-Chatelier response
  against closed-form solutions), not validation against a measured
  equilibrium dataset. AI-assisted draft material, untrusted until
  human-reviewed per the crate `CLAUDE.md`. Not for nuclear facility
  operation, reactor control, safety-critical, or licensing decisions.
  Independent OUTRAM PARK fork, not the official DWSIM.

```rust
pub mod gibbs { /* ... */ }
```

### Types

#### Enum `FugacityModel`

Fugacity model for the gas-phase chemical potential (enum dispatch, no
`dyn`).

Selects the `ln φ_i` term added to `μ_i/RT = g°_i/(RT) + ln φ_i + ln(y_i
P/P°)`. The set is closed and known at compile time, so it is an enum, not a
trait object (workspace `CLAUDE.md`).

```rust
pub enum FugacityModel {
    IdealGas,
    FrozenLnPhi(Vec<f64>),
}
```

##### Variants

###### `IdealGas`

Ideal gas: `φ_i = 1`, so `ln φ_i = 0` for every species. The default,
exact-arithmetic model used by the analytic V&V tests.

###### `FrozenLnPhi`

Frozen (composition-independent) fugacity coefficients: `ln φ_i` is the
`i`-th entry of this vector, held **constant** through the whole
minimisation.

This is a first-order, *frozen*-fugacity approximation of a real-gas
correction — the coefficients are supplied by the caller (e.g. evaluated
once from [`crate::thermo::cubic_eos`] at the feed composition) and are
**not** re-evaluated as the composition changes during iteration. Use it
only where the mixture stays near the composition the coefficients were
taken at; a self-consistent EOS coupling is future work. The vector
length must equal the number of species.

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
    fn clone(self: &Self) -> FugacityModel { /* ... */ }
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
    fn eq(self: &Self, other: &FugacityModel) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GibbsOptions`

Convergence / iteration controls for [`GibbsSystem::minimize`].

All tolerances are dimensionless. Defaults (via [`Default`]) are tuned for
the analytic gas-phase tests: `tol = 1e-10`, `max_iter = 500`,
`max_step = 2.0`, `mole_floor = 1e-12`.

```rust
pub struct GibbsOptions {
    pub tol: f64,
    pub max_iter: usize,
    pub max_step: f64,
    pub mole_floor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tol` | `f64` | Convergence tolerance \[-\]. The iteration stops when both the maximum<br>species log-correction `max_i|δ_i|` and the worst relative atom-balance<br>residual fall below this value. |
| `max_iter` | `usize` | Maximum RAND iterations before returning [`GibbsError::NotConverged`]. |
| `max_step` | `f64` | Maximum per-step log-correction `max_i|ω·δ_i|` \[-\]. The damping factor<br>is `ω = min(1, max_step / max_i|δ_i|)`, capping how far any single<br>species moves in one step (in `ln n` space) to keep the Newton step in<br>its region of validity. Typical `2.0` (a factor `e² ≈ 7.4` per step). |
| `mole_floor` | `f64` | Lower floor \[mol\] applied to the *initial* working moles of any species<br>the feed supplies as zero, so `ln n_i` is finite at the start. Species<br>that equilibrium wants absent decay back toward zero on their own. Must<br>be > 0. |

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
    fn clone(self: &Self) -> GibbsOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &GibbsOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GibbsResult`

Result of a converged (or best-effort) Gibbs speciation minimisation.

Moles and mole fractions are `n`-vectors in species order; element
potentials are an `M`-vector in element order.

```rust
pub struct GibbsResult {
    pub moles: Vec<f64>,
    pub mole_fractions: Vec<f64>,
    pub element_potentials: Vec<f64>,
    pub gibbs_energy_rt: f64,
    pub gibbs_energy: f64,
    pub iterations: usize,
    pub converged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `moles` | `Vec<f64>` | Equilibrium species amounts `n_i` \[mol\], same order as the system's<br>species. Sum to the equilibrium total moles (which may differ from the<br>feed total when reactions change the mole count). |
| `mole_fractions` | `Vec<f64>` | Equilibrium species mole fractions `y_i = n_i / Σ_l n_l` \[-\]. |
| `element_potentials` | `Vec<f64>` | Element potentials `π_k = λ_k / RT` \[-\], one per element. At the<br>solution each species satisfies `μ_i/RT = Σ_k a_ki π_k`. |
| `gibbs_energy_rt` | `f64` | Dimensionless total Gibbs energy `G/RT = Σ_i n_i (μ_i/RT)` \[mol\] at the<br>returned composition (the objective that was minimised). |
| `gibbs_energy` | `f64` | Total Gibbs energy `G = RT · (G/RT)` \[J\], relative to the `g°_i`<br>reference states supplied. |
| `iterations` | `usize` | Number of RAND iterations performed. |
| `converged` | `bool` | `true` if both tolerances in [`GibbsOptions`] were met; `false` if the<br>loop exhausted `max_iter` (the returned composition is then the last,<br>best-effort estimate — see [`GibbsError::NotConverged`], which is<br>returned instead of a `false` result). |

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
    fn clone(self: &Self) -> GibbsResult { /* ... */ }
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
    fn eq(self: &Self, other: &GibbsResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `GibbsError`

Errors from constructing or solving a [`GibbsSystem`].

```rust
pub enum GibbsError {
    DimensionMismatch {
        what: &'static str,
        expected: usize,
        got: usize,
    },
    InvalidInput {
        what: &'static str,
        value: f64,
        positive: bool,
    },
    SingularSystem,
    NotConverged {
        iterations: usize,
        max_correction: f64,
        atom_residual: f64,
    },
}
```

##### Variants

###### `DimensionMismatch`

A supplied slice had the wrong length for the system dimensions.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which input was mis-sized. |
| `expected` | `usize` | Expected length. |
| `got` | `usize` | Actual length. |

###### `InvalidInput`

An input value was non-finite, or a required-positive value was ≤ 0.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which input was invalid. |
| `value` | `f64` | Offending value. |
| `positive` | `bool` | Whether the field additionally had to be strictly positive. |

###### `SingularSystem`

The RAND linear system was singular (typically a rank-deficient atom
matrix — e.g. a duplicated element row, or an element no species
contains). Check the atom matrix has full row rank.

###### `NotConverged`

The iteration did not meet both tolerances within `max_iter`. Carries the
worst residual seen so the caller can judge how close it got.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations performed (`= max_iter`). |
| `max_correction` | `f64` | Final `max_i|δ_i|` \[-\]. |
| `atom_residual` | `f64` | Final worst relative atom-balance residual \[-\]. |

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
    fn clone(self: &Self) -> GibbsError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GibbsError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GibbsSystem`

A reacting multicomponent system: species, elements, and the atom matrix
`a_ki` (atoms of element `k` per molecule of species `i`).

The system carries only the *combinatorial* data (who is made of what); the
thermodynamics (standard Gibbs energies, `T`, `P`, fugacity model) are passed
per solve to [`Self::minimize`], so one system can be reused across states.

The atom matrix is stored row-major by element (length `n_elements *
n_species`).

```rust
pub struct GibbsSystem {
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
  pub fn new(species_names: &[&str], element_symbols: &[&str], atom_matrix: &[&[f64]]) -> Result<Self, GibbsError> { /* ... */ }
  ```
  Build a system from species names, element symbols, and the atom matrix.

- ```rust
  pub fn n_species(self: &Self) -> usize { /* ... */ }
  ```
  Number of species `n`.

- ```rust
  pub fn n_elements(self: &Self) -> usize { /* ... */ }
  ```
  Number of chemical elements `M`.

- ```rust
  pub fn species_names(self: &Self) -> &[String] { /* ... */ }
  ```
  Species names, in species order.

- ```rust
  pub fn element_symbols(self: &Self) -> &[String] { /* ... */ }
  ```
  Element symbols, in element order.

- ```rust
  pub fn element_abundance(self: &Self, moles: &[f64]) -> Result<Vec<f64>, GibbsError> { /* ... */ }
  ```
  Total moles of each element supplied by a composition:

- ```rust
  pub fn minimize(self: &Self, gibbs_formation: &[f64], temperature: f64, feed_moles: &[f64], pressure: f64, p_ref: f64, fugacity: &FugacityModel, options: &GibbsOptions) -> Result<GibbsResult, GibbsError> { /* ... */ }
  ```
  Minimise `G/RT` subject to atom balance, returning the equilibrium

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GibbsSystem { /* ... */ }
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
    fn eq(self: &Self, other: &GibbsSystem) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `gibbs_multiphase`

**Attributes:**

- `Other("#[forbid(unsafe_code)]")`

Multi-phase (N-phase) Gibbs-energy-**minimisation** flash — equilibrium
distribution of chemical species across several coexisting solution phases
(gas / molten-salt / condensed solution) by direct minimisation of the total
Gibbs energy, subject to **element / atom mass-balance** constraints shared
across all phases.

This generalises the single-phase reacting-speciation minimiser
[`crate::thermo::gibbs`] (`GibbsSystem::minimize`) from one phase to `P`
coexisting phases. It is aimed at the molten-salt / fission-product
speciation problem, where the same set of chemical elements partitions
between a gas phase, a molten-salt solution, and condensed solutions
simultaneously.

## Provenance (GPL-3.0)

Ported / adapted from DWSIM (GPL-3.0), commit
`1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`:
- `DWSIM.Thermodynamics/FlashAlgorithms/GibbsMinimizationMulti.vb`
- `DWSIM.Thermodynamics/FlashAlgorithms/GibbsMinimization3P.vb`

DWSIM states the multi-phase objective as (`GibbsMinimizationMulti.vb:132`,
`GibbsMinimization3P.vb:146`):

```text
Q = Σ_k Σ_i n_ik · ln f_ik
```

the sum over phases `k` and species `i` of moles times `ln` of the species
fugacity, with gradient (`GibbsMinimizationMulti.vb:140`,
`GibbsMinimization3P.vb:154`)

```text
∂Q/∂n_ij = ln f_ij − ln f_iF
```

and the numeric per-phase Gibbs energy assembled in `FunctionValue` as
`Σ_i x_i · n_phase · ln(φ_i x_i)` per phase
(`GibbsMinimizationMulti.vb:712,797-806`; `GibbsMinimization3P.vb:760-761,
838-840`). DWSIM minimises that objective **over a phase split** (species
conserved individually, `n_iF = z_i − Σ_{k<F} n_ik`,
`GibbsMinimizationMulti.vb:136`) and delegates the constrained optimisation
to an **external** solver — IPOPT via `Cureos.Numerics`
(`GibbsMinimizationMulti.vb:20`; the `Solver` property defaults to
`OptimizationMethod.IPOPT`, `:73`).

## What THIS module does (and how it differs from the DWSIM file)

It keeps DWSIM's Gibbs objective and its ideal-solution chemical-potential
model `μ_ik = g°_ik + RT ln(φ_ik x_ik) [+ RT ln(P/P°) for a gas]`, but:

1. **Element/atom balance replaces the species phase-split constraint.**
   This is the *non-stoichiometric* (element-abundance) formulation — the
   White–Johnson–Dantzig / **RAND** element-potential method (Smith & Missen,
   *Chemical Reaction Equilibrium Analysis*, 1982; NASA CEA, Gordon &
   McBride, RP-1311), extended to multiple phases with a **shared** element
   potential. It reduces to a species phase-split when every species is
   atomically distinct (see the Nernst-partition V&V test), and it also
   admits inter-species reactions where the atoms allow (see the water-gas-
   shift collapse test) — a strict superset of DWSIM's non-reacting flash.
2. **The constrained optimiser is pure Rust** — a damped multi-phase RAND /
   element-potential Newton iteration (below). DWSIM's IPOPT dependency is
   not available in this workspace, so none is used; the `(M + P)` linear
   system is solved with an in-crate Gaussian elimination (no BLAS/LAPACK,
   so this compiles on Android / Termux like the rest of the non-GUI crate).

## The minimisation problem

Minimise the dimensionless total Gibbs energy over `P` phases:

```text
G/RT = Σ_p Σ_s n_ps · (μ_ps / RT),
μ_ps/RT = g°_ps/(RT) + ln φ_ps + ln x_ps [+ ln(P/P°) if phase p is a gas]
```

where `n_ps` is the moles of species `s` in phase `p` \[mol\],
`x_ps = n_ps / Σ_l n_pl` its mole fraction within phase `p`, and `g°_ps` its
standard molar Gibbs energy in that phase \[J/mol\] (a species may take a
different `g°` in different phases, e.g. `H₂O(g)` vs `H₂O(l)`). The atom
matrix `a_ks` (atoms of element `k` per molecule of species `s`) is a
property of the species, identical in every phase. Subject to, for every
element `k`,

```text
Σ_p Σ_s a_ks · n_ps = b_k     (atom balance over ALL phases),   n_ps ≥ 0.
```

The Lagrange stationarity condition is the **shared element-potential**
relation — one `π_k` per element, common to *every* phase:

```text
μ_ps / RT = Σ_k a_ks · π_k     for every species s present in every phase p.
```

Equating this across two phases containing the same species `s` gives
`μ_s(phase p)= μ_s(phase q)`: equal chemical potential — the phase-
equilibrium condition. For a single species distributing between two ideal
solutions it becomes the **Nernst distribution law**
`x_sq / x_sp = exp((g°_sp − g°_sq)/RT)`; for a species over its own pure
condensed phase it becomes the **vapour-pressure** relation
`x_s(P/P°) = exp((g°_cond − g°_gas)/RT)`. Both are checked in closed form in
the V&V tests.

## The multi-phase RAND / element-potential iteration (pure Rust)

At a strictly-positive current estimate `n_ps`, with phase totals
`n_p = Σ_s n_ps`, form the `(M+P)×(M+P)` saddle-point system for the `M`
shared element potentials `π_k` and the `P` per-phase total-mole corrections
`u_p = Δ ln n_p`:

```text
Σ_j R_kj π_j + Σ_p Q_kp u_p = (b_k − q_k) + S_k     (one row per element k)
Σ_j Q_jp π_j                = F_p                    (one row per phase p)
```

with (sums over species `s`, and over phases `p` where written)
`q_k = Σ_p Σ_s a_ks n_ps` (current element abundance),
`Q_kp = Σ_s a_ks n_ps` (element `k` abundance *in phase p*),
`R_kj = Σ_p Σ_s a_ks a_js n_ps`, `S_k = Σ_p Σ_s a_ks n_ps (μ_ps/RT)`, and
`F_p = Σ_s n_ps (μ_ps/RT) = G_p/RT` (phase Gibbs energy). The species
correction is then

```text
δ_ps = Δ ln n_ps = Σ_k a_ks π_k + u_p − μ_ps/RT,
n_ps ← n_ps · exp(ω · δ_ps),   ω = min(1, max_step / max|δ|).
```

The block structure `[[R, Q],[Qᵀ, 0]]` is exactly the single-phase system of
[`crate::thermo::gibbs`] with the scalar total-mole row generalised to `P`
rows/columns; for `P = 1` it is identical. The **multiplicative** update
keeps every `n_ps > 0` for free, and the `(b_k − q_k)` residual drives the
atom balance back onto target each step, so the iteration is self-correcting
against round-off drift.

**Merit line search (monotone descent).** ω is further reduced by a
backtracking line search on the merit `Φ = G/RT + μ·Σ_k|b_k − q_k|` (penalty
[`MERIT_PENALTY`]): the largest halving of the damped ω₀ whose trial `Φ` does
not rise is taken. `Φ` is thus monotonically non-increasing *by construction*,
and since its atom-violation term → 0 at convergence, `Φ → G/RT`. This keeps
the iterate path near the constraint manifold; without it the projection-free
multiplicative step would dip through infeasible points whose `G/RT` lies
*below* the constrained minimum before rebounding (non-monotone `G/RT`). The
monotone witness returned is [`MultiPhaseResult::descent_merit_history`].

**Vanishing phases.** When a phase's total `n_p` falls below
[`MultiPhaseOptions::phase_floor`], its `Q_·p` column vanishes and the saddle
system would go singular. Such a phase is *frozen*: its row `M+p` is replaced
by the identity `u_p = 0`, so the active phases still solve a well-posed
system while the frozen phase's species keep updating from the shared `π`
(they regrow if `μ_ps < Σ_k a_ks π_k` becomes favourable, else stay near
zero). This is how a two-phase run **collapses onto the single-phase result**
when only one phase is thermodynamically stable (V&V test below).

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

Raw `f64` in SI throughout the inner loop (the crate "raw f64 in inner
EOS/flash loops" rule): temperature `T` \[K\], pressure `P` and reference
pressure `P°` \[Pa\], standard Gibbs energies `g°_ps` \[J/mol\], moles
\[mol\], mole fractions and element potentials dimensionless. The gas
constant is the crate-shared CODATA value [`crate::thermo::ideal_props::R`].

## Design (workspace + crate `CLAUDE.md`)

Enum dispatch, **no `dyn` / `Box` / lifetimes**: the per-phase mixing model
is the closed enum [`PhaseModel`]; the system, options, result, and errors
are plain owned structs / enums. `#![forbid(unsafe_code)]`. Raw `f64` maths
inside.

## Honest scope — untrusted AI-assisted draft, pending human V&V

- **Ideal-solution phases only.** Each phase mixes ideally
  ([`PhaseModel::IdealGas`] / [`PhaseModel::IdealSolution`], `φ = 1`, activity
  `= x`). A single-species phase therefore has activity `1` and models a
  **pure condensed** substance exactly (used by the vapour-pressure test). A
  caller-supplied *constant* `ln φ`/`ln γ` correction (frozen-fugacity) and a
  self-consistent EOS/activity coupling that recomputes `φ_ps(x,T,P)` /
  `γ_ps(x,T)` each iteration are **not** implemented — documented extension
  points, matching the same limitation in [`crate::thermo::gibbs`].
- **No automatic phase creation / detection.** The set of candidate phases is
  supplied by the caller. A candidate that is unstable collapses to ~0 (see
  vanishing-phases above), but the algorithm does **not** discover a phase the
  caller did not list. Stability analysis (adding a trial phase from a TPD
  test, cf. [`crate::thermo::stability`]) is future work.
- **Verified, not validated.** The tests below are analytic **verification**
  (Nernst partition, vapour-pressure, water-gas-shift equilibrium constant,
  atom balance, cross-check against the single-phase [`crate::thermo::gibbs`])
  against closed-form solutions, **not** validation against a measured
  multi-phase equilibrium dataset. AI-assisted draft material, untrusted
  until human-reviewed per the crate `CLAUDE.md`. Not for nuclear facility
  operation, reactor control, safety-critical, or licensing decisions.
  Independent OUTRAM PARK fork, not the official DWSIM.

```rust
pub mod gibbs_multiphase { /* ... */ }
```

### Types

#### Enum `PhaseModel`

Ideal mixing model for one phase (enum dispatch, no `dyn`).

Selects the composition term in the species chemical potential
`μ_ps/RT = g°_ps/(RT) + ln x_ps + [ln(P/P°) if gas]`. The set is closed and
known at compile time, so it is an enum, not a trait object (workspace
`CLAUDE.md`). Both variants use ideal mixing (activity `= x`, `φ = 1`); they
differ only by whether the pressure term `ln(P/P°)` is added.

```rust
pub enum PhaseModel {
    IdealGas,
    IdealSolution,
}
```

##### Variants

###### `IdealGas`

Ideal **gas** solution: `μ_ps/RT = g°_ps/(RT) + ln x_ps + ln(P/P°)`. The
`ln(P/P°)` term drives the Le-Chatelier pressure response of any
mole-changing reaction and the vapour-pressure partition of a species
over a condensed phase.

###### `IdealSolution`

Ideal **condensed** solution (molten salt, liquid, or solid solution):
`μ_ps/RT = g°_ps/(RT) + ln x_ps`, with **no** pressure term (condensed-
phase molar volume neglected). A single-species `IdealSolution` phase has
`x = 1`, hence unit activity — an exact **pure condensed** substance.

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
    fn clone(self: &Self) -> PhaseModel { /* ... */ }
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
    fn eq(self: &Self, other: &PhaseModel) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PhaseInput`

One phase's static (combinatorial) description supplied to
[`MultiPhaseGibbsSystem::new`]: its mixing model, its species labels, and the
atom sub-matrix mapping each of its species onto the *global* element list.

The atom matrix has `M` rows (one per global element, in the system's element
order) and `n_species` columns; `atom_matrix[k][s]` is the number of atoms of
element `k` in one molecule of species `s` of this phase (non-negative,
finite; typically small integers).

```rust
pub struct PhaseInput {
    pub model: PhaseModel,
    pub species_names: Vec<String>,
    pub atom_matrix: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `model` | `PhaseModel` | Ideal mixing model for the phase. |
| `species_names` | `Vec<String>` | Species labels for this phase (identification only). Length =<br>`n_species` of the phase; a species may also appear (by the same atoms,<br>possibly different `g°`) in other phases. |
| `atom_matrix` | `Vec<Vec<f64>>` | `M` rows × `n_species` columns: atoms of each global element per molecule<br>of each species in this phase. |

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
    fn clone(self: &Self) -> PhaseInput { /* ... */ }
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
    fn eq(self: &Self, other: &PhaseInput) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MultiPhaseOptions`

Convergence / iteration controls for [`MultiPhaseGibbsSystem::minimize`].

All tolerances are dimensionless. Defaults (via [`Default`]) mirror the
single-phase [`crate::thermo::gibbs::GibbsOptions`], with an added
`phase_floor`: `tol = 1e-10`, `max_iter = 3000`, `max_step = 2.0`,
`mole_floor = 1e-12`, `phase_floor = 1e-11`.

```rust
pub struct MultiPhaseOptions {
    pub tol: f64,
    pub max_iter: usize,
    pub max_step: f64,
    pub mole_floor: f64,
    pub phase_floor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tol` | `f64` | Convergence tolerance \[-\]. The iteration stops when both the maximum<br>species log-correction `max|δ_ps|` and the worst relative atom-balance<br>residual fall below this value. |
| `max_iter` | `usize` | Maximum RAND iterations before returning [`MultiPhaseError::NotConverged`]. |
| `max_step` | `f64` | Maximum per-step log-correction `max|ω·δ_ps|` \[-\]. Damping factor is<br>`ω = min(1, max_step / max|δ|)`, capping how far any species moves in one<br>step (in `ln n` space). Typical `2.0` (a factor `e² ≈ 7.4` per step). |
| `mole_floor` | `f64` | Lower floor \[mol\] applied to the *initial* working moles of any (phase,<br>species) the feed supplies as zero, so `ln n` is finite at the start.<br>Must be > 0. |
| `phase_floor` | `f64` | Phase-total floor \[mol\]. A phase whose total moles fall below this is<br>*frozen* (its `u_p` correction is pinned to `0`) to keep the saddle-point<br>system non-singular as the phase vanishes; its species still update from<br>the shared element potentials. Must be > 0 and ≥ `mole_floor`. |

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
    fn clone(self: &Self) -> MultiPhaseOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &MultiPhaseOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MultiPhaseResult`

Result of a converged multi-phase Gibbs minimisation.

Per-phase quantities are indexed `[phase][species]` in the phase / species
order the system was built with; element potentials are an `M`-vector in
element order.

```rust
pub struct MultiPhaseResult {
    pub moles: Vec<Vec<f64>>,
    pub mole_fractions: Vec<Vec<f64>>,
    pub phase_totals: Vec<f64>,
    pub element_potentials: Vec<f64>,
    pub gibbs_energy_rt: f64,
    pub gibbs_energy: f64,
    pub gibbs_energy_rt_history: Vec<f64>,
    pub descent_merit_history: Vec<f64>,
    pub iterations: usize,
    pub converged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `moles` | `Vec<Vec<f64>>` | Equilibrium species amounts `n_ps` \[mol\], `moles[p][s]`. A phase that<br>collapsed reports species amounts near `mole_floor` or below. |
| `mole_fractions` | `Vec<Vec<f64>>` | Equilibrium within-phase mole fractions `x_ps = n_ps / Σ_l n_pl` \[-\],<br>`mole_fractions[p][s]`. For a collapsed phase (total < `phase_floor`)<br>these are reported as `0`. |
| `phase_totals` | `Vec<f64>` | Total moles in each phase `n_p = Σ_s n_ps` \[mol\], one entry per phase. |
| `element_potentials` | `Vec<f64>` | Shared element potentials `π_k = λ_k / RT` \[-\], one per element. At the<br>solution every present species satisfies `μ_ps/RT = Σ_k a_ks π_k` in<br>*every* phase — the phase-equilibrium condition. |
| `gibbs_energy_rt` | `f64` | Dimensionless total Gibbs energy `G/RT = Σ_p Σ_s n_ps (μ_ps/RT)` \[mol\]<br>at the returned composition (the objective that was minimised). |
| `gibbs_energy` | `f64` | Total Gibbs energy `G = RT · (G/RT)` \[J\], relative to the `g°_ps`<br>reference states supplied. |
| `gibbs_energy_rt_history` | `Vec<f64>` | Trajectory of the physical `G/RT` \[mol\] evaluated at the iterate<br>*entering* each RAND iteration, followed by the final converged value. It<br>descends overall from the feed point to the constrained minimum<br>(`gibbs_energy_rt_history[0]` ≥ `.last()`), but is not guaranteed<br>non-increasing at *every* step: the multiplicative update visits slightly<br>off-manifold points whose `G/RT` can wobble by the step's second-order<br>atom-balance error. The rigorously monotone quantity is<br>[`Self::descent_merit_history`]. |
| `descent_merit_history` | `Vec<f64>` | Trajectory of the **line-search merit** `Φ = G/RT + μ·Σ_k|b_k − q_k|`<br>\[mol\] (dimensionless-times-mol; `μ` the internal penalty weight), one<br>entry per iteration plus the final value. This is **monotonically<br>non-increasing by construction** — each step's backtracking accepts only a<br>`Φ` that does not rise (to float slack). Because the atom-violation term<br>`→ 0` at convergence, `Φ → G/RT`; so a monotone `Φ` descending to the<br>converged `G/RT` is the V&V monotonicity witness for the Gibbs-energy<br>minimisation (the exact-penalty / augmented-objective sense). |
| `iterations` | `usize` | Number of RAND iterations performed. |
| `converged` | `bool` | `true` if both tolerances in [`MultiPhaseOptions`] were met. |

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
    fn clone(self: &Self) -> MultiPhaseResult { /* ... */ }
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
    fn eq(self: &Self, other: &MultiPhaseResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `MultiPhaseError`

Errors from constructing or solving a [`MultiPhaseGibbsSystem`].

```rust
pub enum MultiPhaseError {
    DimensionMismatch {
        what: &'static str,
        expected: usize,
        got: usize,
    },
    InvalidInput {
        what: &'static str,
        value: f64,
        positive: bool,
    },
    SingularSystem,
    NotConverged {
        iterations: usize,
        max_correction: f64,
        atom_residual: f64,
    },
}
```

##### Variants

###### `DimensionMismatch`

A supplied slice / matrix had the wrong length for the system dimensions.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which input was mis-sized. |
| `expected` | `usize` | Expected length. |
| `got` | `usize` | Actual length. |

###### `InvalidInput`

An input value was non-finite, or a required-positive value was ≤ 0.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which input was invalid. |
| `value` | `f64` | Offending value. |
| `positive` | `bool` | Whether the field additionally had to be strictly positive. |

###### `SingularSystem`

The RAND saddle-point system was singular even after freezing vanished
phases — typically a rank-deficient atom matrix (a duplicated element
row, an element no species contains, or two phases with proportional
element content). Check the atom matrices have full row rank and the
phases are distinct in element space.

###### `NotConverged`

The iteration did not meet both tolerances within `max_iter`. Carries the
worst residuals seen so the caller can judge how close it got.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations performed (`= max_iter`). |
| `max_correction` | `f64` | Final `max|δ_ps|` \[-\]. |
| `atom_residual` | `f64` | Final worst relative atom-balance residual \[-\]. |

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
    fn clone(self: &Self) -> MultiPhaseError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MultiPhaseError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MultiPhaseGibbsSystem`

A multi-phase reacting system: a shared element list plus `P` phases, each
with its own species and atom sub-matrix.

The system carries only the *combinatorial* data (which phases exist, who is
made of what); the thermodynamics (standard Gibbs energies, `T`, `P`, and the
feed) are passed per solve to [`Self::minimize`], so one system can be reused
across states.

```rust
pub struct MultiPhaseGibbsSystem {
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
  pub fn new(element_symbols: &[&str], phases: &[PhaseInput]) -> Result<Self, MultiPhaseError> { /* ... */ }
  ```
  Build a multi-phase system from the global element list and one

- ```rust
  pub fn n_elements(self: &Self) -> usize { /* ... */ }
  ```
  Number of chemical elements `M`.

- ```rust
  pub fn n_phases(self: &Self) -> usize { /* ... */ }
  ```
  Number of phases `P`.

- ```rust
  pub fn n_species(self: &Self, p: usize) -> usize { /* ... */ }
  ```
  Number of species in phase `p`. Panics if `p` is out of range.

- ```rust
  pub fn element_symbols(self: &Self) -> &[String] { /* ... */ }
  ```
  Element symbols, in element order.

- ```rust
  pub fn species_names(self: &Self, p: usize) -> &[String] { /* ... */ }
  ```
  Species names of phase `p`, in species order. Panics if `p` is out of

- ```rust
  pub fn element_abundance(self: &Self, moles: &[&[f64]]) -> Result<Vec<f64>, MultiPhaseError> { /* ... */ }
  ```
  Total moles of each element supplied by a per-phase composition:

- ```rust
  pub fn minimize(self: &Self, gibbs_formation: &[&[f64]], temperature: f64, feed: &[&[f64]], pressure: f64, p_ref: f64, options: &MultiPhaseOptions) -> Result<MultiPhaseResult, MultiPhaseError> { /* ... */ }
  ```
  Minimise `G/RT` over all phases subject to shared atom balance, returning

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> MultiPhaseGibbsSystem { /* ... */ }
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
    fn eq(self: &Self, other: &MultiPhaseGibbsSystem) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `ideal_props`

Ideal-gas heat capacity / enthalpy / entropy — the **departure-function
reference state**.

Ported from DWSIM (GPL-3.0):
- `DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb`
  `GetIdealGasHeatCapacity` (lines 1758-1863) and the sibling `AUX_CPi`
  in `PropertyPackages/PropertyPackage.vb` (lines 5828-5851) — the Cp0
  correlation.
- `DWSIM.Thermodynamics/PropertyPackages/PropertyPackage.vb`
  `AUX_INT_CPDTi` / `AUX_INT_CPDT_Ti` (lines 7794-7976, the enthalpy and
  entropy integrals of Cp0), and `RET_Hid` / `RET_Sid` (lines 8558-8718,
  which add the `-R ln(P/P_ref)` pressure term and the `-R Σ x_i ln x_i`
  ideal entropy of mixing).

The `PropertyPackageMethods.vb` file named in the porting brief is only the
method-selection *config* class (which correlation string each phase uses);
the numerical routines it selects live in the two files above, so that is
where this port draws from.

## The DWSIM Cp0 correlation (canonical `"DWSIM"`/`""` database path)

DWSIM parameterises the ideal-gas heat capacity of a pure compound as a
**quartic polynomial in temperature**:

```text
Cp0(T) = A + B·T + C·T² + D·T³ + E·T⁴
```

with `A..E` = DWSIM's `Ideal_Gas_Heat_Capacity_Const_A..E`
([`crate::thermo::Component::cp_ig_a`] … `cp_ig_e`) and `T` in kelvin.

**Coefficient-unit convention (important).** The DWSIM source comment reads
*"Cp in kJ/kg-mol, T in K"* (`ThermodynamicsBase.vb:1776`,
`PropertyPackage.vb:1849`). `kg-mol` is the kilomole (kmol), so the raw
polynomial yields Cp0 in **kJ/(kmol·K)**. DWSIM's `AUX_CPi` then divides by
the molar mass to hand callers a **mass-basis** Cp in kJ/(kg·K)
(`… Return result / Molar_Weight 'kJ/kg.K`, `PropertyPackage.vb:1779/5851`).

This port keeps the **molar** basis, which is a pure units identity — no
extra conversion needed:

```text
1 kJ/(kmol·K) = 1000 J / (1000 mol · K) = 1 J/(mol·K)
```

so **Cp0 in J/(mol·K) equals the polynomial value directly**, and DWSIM's
`/ Molar_Weight` step is exactly the molar→mass conversion we intentionally
omit. The molar mass ([`Component::molar_mass`], kg/mol) is therefore *not*
used by any function here — it is only how DWSIM re-expresses these same
numbers on a mass basis downstream.

## Reference constants

DWSIM hard-codes `R = 8.314` J/(mol·K) and `P_ref = 101325` Pa in these
routines (`PropertyPackage.vb:8695-8698`, `8711-8714`). This port uses the
CODATA gas constant [`R`] = 8.314 462 618 153 24 J/(mol·K) and takes the
reference pressure/temperature as explicit arguments so the departure
reference state is a caller decision, not a buried literal. With
`P_ref = 101325 Pa` and `T_ref = 298.15 K` the results match DWSIM's
convention to within the `8.314` vs CODATA-`R` difference (≈ 5.6e-5
relative on the pressure term only).

## Numerical vs analytic integral

DWSIM integrates Cp0 **numerically** — a midpoint rule with an
adaptive step count (`AUX_INT_CPDTi`, `PropertyPackage.vb:7794-7826`).
Because the underlying correlation is a polynomial, this port instead uses
the **exact closed-form integral** of that same polynomial (below). This is
a faithful, higher-accuracy evaluation of the identical correlation — the
analytic result is what DWSIM's midpoint sum converges to as its step count
grows — not a different model.

```text
∫_{T_ref}^{T} Cp0 dT   = A(T−T₀) + (B/2)(T²−T₀²) + (C/3)(T³−T₀³)
                         + (D/4)(T⁴−T₀⁴) + (E/5)(T⁵−T₀⁵)

∫_{T_ref}^{T} Cp0/T dT = A·ln(T/T₀) + B(T−T₀) + (C/2)(T²−T₀²)
                         + (D/3)(T³−T₀³) + (E/4)(T⁴−T₀⁴)
```

(writing `T₀` for `T_ref`.)

## Honest scope (verification, not benchmark validation)

The inline tests below are **verification** — they check the closed-form
integrals against hand-computed algebra (a constant-Cp component,
polynomial closed forms, the exact pressure term, the mixing term). They do
**not** validate the DWSIM A..E coefficients themselves against measured
Cp0 data — that is compound-data validation and belongs with the compound
database, not this integrator.

**Excluded / deferred, by design:**
- **The non-`"DWSIM"` Cp0 database paths** — DWSIM's `CheResources`
  (cal/mol·K, ×4.1868), `ChemSep`/`User`/`ChEDL Thermo`/`CoolProp`/
  `Biodiesel`/`KDB` (DIPPR/ChemSep equation-number forms evaluated by
  `CalcCSTDepProp`/`ParseEquation`), and the Lee-Kesler petroleum-fraction
  estimate (`Cpig_lk`). Only the canonical **A..E quartic** path is ported
  here. A compound whose data is tabular or equation-number-based is a
  deferred path (future work).
- **Enthalpy/entropy of formation offsets.** These functions return the
  *sensible* ideal-gas enthalpy `∫Cp0 dT` and entropy `∫Cp0/T dT − R ln…`
  relative to `T_ref`/`P_ref` — the reference state for EOS departure
  functions. Absolute enthalpy/entropy of formation (DWSIM's
  `IG_Enthalpy_of_Formation_25C`, [`Component::ig_entropy_formation_25c`])
  are a separate additive offset, not applied here.
- **Real-gas departures.** The Peng-Robinson / SRK enthalpy & entropy
  departures that turn these ideal-gas values into real-fluid properties
  live in [`crate::thermo::cubic_eos`], not here.

```rust
pub mod ideal_props { /* ... */ }
```

### Functions

#### Function `ideal_gas_cp`

**Attributes:**

- `MustUse { reason: None }`

Ideal-gas heat capacity `Cp0(T)` of a pure compound [J/(mol·K)].

Evaluates the canonical DWSIM quartic correlation
`Cp0 = A + B·T + C·T² + D·T³ + E·T⁴` from the component's
`cp_ig_a..e` coefficients (`ThermodynamicsBase.vb:1778`,
`PropertyPackage.vb:5850`). The polynomial value is in kJ/(kmol·K), which
**equals J/(mol·K)** numerically (see the module docs), so no molar-mass
conversion is applied — unlike DWSIM's `AUX_CPi`, which additionally divides
by `Molar_Weight` to return a mass-basis kJ/(kg·K).

# Parameters
- `component`: source of the `cp_ig_a..e` coefficients (kJ/(kmol·K) basis).
- `temperature` `T`: absolute temperature [K]. Must be > 0; the correlation
  is a fit valid only over the compound's regressed temperature range
  (typically ~50-1500 K) — extrapolation beyond it is not checked here.

# Returns
`Cp0(T)` [J/(mol·K)].

```rust
pub fn ideal_gas_cp(component: &crate::thermo::Component, temperature: f64) -> f64 { /* ... */ }
```

#### Function `ideal_gas_enthalpy`

**Attributes:**

- `MustUse { reason: None }`

Sensible ideal-gas molar enthalpy relative to `T_ref` [J/mol]:
`H0(T) − H0(T_ref) = ∫_{T_ref}^{T} Cp0(T') dT'`.

Uses the **exact analytic integral** of the DWSIM quartic Cp0 correlation
(the closed form DWSIM's numerical `AUX_INT_CPDTi` midpoint rule converges
to, `PropertyPackage.vb:7794`; `RET_Hid`, `:8558`):

```text
A(T−T₀) + (B/2)(T²−T₀²) + (C/3)(T³−T₀³) + (D/4)(T⁴−T₀⁴) + (E/5)(T⁵−T₀⁵)
```

This is the *sensible* enthalpy only — the enthalpy of formation offset is
not added (see module "Honest scope").

# Parameters
- `component`: source of `cp_ig_a..e`.
- `temperature` `T` [K], `t_ref` `T_ref` [K]: both > 0. `T < T_ref` is
  allowed and returns a negative enthalpy (the integral is signed).

# Returns
`∫_{T_ref}^{T} Cp0 dT` [J/mol].

```rust
pub fn ideal_gas_enthalpy(component: &crate::thermo::Component, temperature: f64, t_ref: f64) -> f64 { /* ... */ }
```

#### Function `ideal_gas_entropy`

**Attributes:**

- `MustUse { reason: None }`

Sensible ideal-gas molar entropy relative to `(T_ref, P_ref)`
[J/(mol·K)]:
`S0(T,P) − S0(T_ref,P_ref) = ∫_{T_ref}^{T} Cp0/T' dT' − R ln(P/P_ref)`.

Uses the **exact analytic integral** of Cp0/T for the DWSIM quartic (the
closed form DWSIM's numerical `AUX_INT_CPDT_Ti` midpoint rule converges to,
`PropertyPackage.vb:7944`) plus the pressure term of `RET_Sid`
(`:8698`, `-R ln(P/P_ref)`; DWSIM hard-codes `P_ref = 101325 Pa`):

```text
A·ln(T/T₀) + B(T−T₀) + (C/2)(T²−T₀²) + (D/3)(T³−T₀³) + (E/4)(T⁴−T₀⁴)
  − R ln(P/P_ref)
```

# Parameters
- `component`: source of `cp_ig_a..e`.
- `temperature` `T` [K], `t_ref` `T_ref` [K]: both > 0.
- `pressure` `P` [Pa], `p_ref` `P_ref` [Pa]: both > 0. The pressure term is
  the ideal-gas isothermal entropy change `-R ln(P/P_ref)`; larger `P`
  lowers entropy.

# Returns
`∫_{T_ref}^{T} Cp0/T dT − R ln(P/P_ref)` [J/(mol·K)].

```rust
pub fn ideal_gas_entropy(component: &crate::thermo::Component, temperature: f64, pressure: f64, t_ref: f64, p_ref: f64) -> f64 { /* ... */ }
```

#### Function `mixture_ideal_gas_cp`

**Attributes:**

- `MustUse { reason: None }`

Mole-fraction-weighted ideal-gas molar heat capacity of a mixture
[J/(mol·K)]: `Cp0_mix(T) = Σ_i x_i Cp0_i(T)`.

The ideal-gas mixture Cp is a linear mole-fraction average (no mixing
contribution — Cp of mixing is zero for ideal gases).

# Parameters
- `components`: the pure compounds, one per mixture species.
- `mole_fractions` `x_i`: mole fractions [-], same length as `components`,
  normally summing to 1 (not enforced — the caller owns normalisation).
- `temperature` `T` [K].

# Panics
Panics if `components.len() != mole_fractions.len()`.

# Returns
`Σ_i x_i Cp0_i(T)` [J/(mol·K)].

```rust
pub fn mixture_ideal_gas_cp(components: &[crate::thermo::Component], mole_fractions: &[f64], temperature: f64) -> f64 { /* ... */ }
```

#### Function `mixture_ideal_gas_enthalpy`

**Attributes:**

- `MustUse { reason: None }`

Mole-fraction-weighted sensible ideal-gas molar enthalpy of a mixture
[J/mol]: `H0_mix(T) − H0_mix(T_ref) = Σ_i x_i ∫_{T_ref}^{T} Cp0_i dT`.

Enthalpy of mixing is zero for an ideal gas, so this is a plain
mole-fraction sum of the pure-component sensible enthalpies.

# Parameters
- `components`, `mole_fractions`: as [`mixture_ideal_gas_cp`].
- `temperature` `T` [K], `t_ref` `T_ref` [K].

# Panics
Panics if `components.len() != mole_fractions.len()`.

# Returns
`Σ_i x_i ∫_{T_ref}^{T} Cp0_i dT` [J/mol].

```rust
pub fn mixture_ideal_gas_enthalpy(components: &[crate::thermo::Component], mole_fractions: &[f64], temperature: f64, t_ref: f64) -> f64 { /* ... */ }
```

#### Function `ideal_entropy_of_mixing`

**Attributes:**

- `MustUse { reason: None }`

Ideal entropy of mixing [J/(mol·K)]: `Δs_mix = −R Σ_i x_i ln x_i`.

Ports the mixing contribution DWSIM adds inside `RET_Sid`
(`PropertyPackage.vb:8695`, `-R x_i ln x_i` per species; DWSIM's mass-basis
`/Molar_Weight` is dropped here for the molar basis). It is always ≥ 0 and
vanishes for a pure stream (any `x_i = 1`). Species with `x_i = 0` are
skipped (the `x ln x → 0` limit), matching DWSIM's `If x_i <> 0` guard.

# Parameters
- `mole_fractions` `x_i` [-], normally summing to 1. Non-positive entries
  are treated as absent species (skipped).

# Returns
`−R Σ_i x_i ln x_i` [J/(mol·K)].

```rust
pub fn ideal_entropy_of_mixing(mole_fractions: &[f64]) -> f64 { /* ... */ }
```

#### Function `mixture_ideal_gas_entropy`

**Attributes:**

- `MustUse { reason: None }`

Total ideal-gas molar entropy of a mixture relative to `(T_ref, P_ref)`
[J/(mol·K)]:
`S0_mix = Σ_i x_i [∫Cp0_i/T dT − R ln(P/P_ref)] + (−R Σ_i x_i ln x_i)`.

The mole-fraction-weighted pure-component ideal-gas entropies plus the
[`ideal_entropy_of_mixing`] term (DWSIM `RET_Sid`,
`PropertyPackage.vb:8680-8718`). Because the `−R ln(P/P_ref)` pressure term
is species-independent, `Σ_i x_i` of it collapses to a single
`−R ln(P/P_ref)` when the mole fractions sum to 1.

# Parameters
- `components`, `mole_fractions`: as [`mixture_ideal_gas_cp`].
- `temperature` `T` [K], `pressure` `P` [Pa], `t_ref` `T_ref` [K],
  `p_ref` `P_ref` [Pa]: all > 0.

# Panics
Panics if `components.len() != mole_fractions.len()`.

# Returns
The total mixture ideal-gas entropy [J/(mol·K)].

```rust
pub fn mixture_ideal_gas_entropy(components: &[crate::thermo::Component], mole_fractions: &[f64], temperature: f64, pressure: f64, t_ref: f64, p_ref: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `R`

Universal gas constant `R` [J/(mol·K)] — CODATA 2018 exact value.

DWSIM hard-codes the rounded `8.314`; this port uses the exact SI value.
The difference only affects the `-R ln(P/P_ref)` pressure term of the
entropy (≈ 5.6e-5 relative).

```rust
pub const R: f64 = 8.314_462_618_153_24;
```

## Module `lkp`

Lee-Kesler-Plöcker (LKP) three-parameter corresponding-states EOS.

Ported from DWSIM (GPL-3.0), Visual-Basic reference source:
- `DWSIM.Thermodynamics/PropertyPackages/Models/LeeKeslerPlocker.vb`
  (commit `1abf72d`): `Z_LK` L279-331, the Lee-Kesler modified-BWR
  reduced-volume solver `ESTIMAR_Vr2` L552-668, the enthalpy departure
  `H_LK` L333-390, the entropy departure `S_LK` L453-514, the pure
  corresponding-states fugacity `LnFugM` L810-873, and the mixture critical
  combining rule `MixCritProp_LK` L93-147.

## What this model is

Unlike Peng-Robinson / SRK (a cubic in `V`), LKP is a **corresponding-states**
method built on the Lee-Kesler (1975) modified Benedict-Webb-Rubin (BWR)
equation written in reduced coordinates. Two reference fluids are used:

- a **simple fluid** (`ω = 0`, argon/krypton/methane-like), and
- a **heavy reference fluid** (`ω_ref = 0.3978`, n-octane).

Any property `M` (compressibility, enthalpy/entropy departure, log-fugacity)
is linearly interpolated in the acentric factor between the two:

`M = M⁰ + (ω / 0.3978) · (Mʳᵉᶠ − M⁰)`

(`LeeKeslerPlocker.vb` L327 for `Z`, L386 for `H`, L510 for `S`, L869 for
`ln φ`). Plöcker's contribution is the **mixture** critical-property
combining rule ([`mix_crit_props`], `MixCritProp_LK.vb` L93-147) that maps a
multicomponent mixture onto a single set of pseudo-critical `Tcm, Pcm, Vcm,
ωm`, so the pure corresponding-states functions apply to mixtures too.

LKP gives accurate densities and enthalpy departures for **light gases and
their mixtures** (CO, CO₂, H₂, H₂O, He, N₂, CH₄) over a wide pressure range,
which is why it is relevant to HTGR cover-gas / water-ingress chemistry.

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

Temperature K, pressure Pa, molar volume m³/mol, enthalpy departure J/mol,
entropy departure J/(mol·K), critical volume `Vc` m³/mol. The reduced
quantities `Tr = T/Tc`, `Pr = P/Pc`, the reduced volume `Vr = Pc V / (R Tc)`,
the compressibility `Z`, the acentric factor `ω`, and mole fractions are all
dimensionless. Raw `f64` is used throughout the inner corresponding-states
loops (matching [`crate::thermo::cubic_eos`]); every public signature spells
out its units.

Note on `R`: DWSIM hard-codes `R = 8.314`. This port uses the CODATA-2018
exact `R` (re-exported from [`crate::thermo::cubic_eos::R`]) so single-point
numbers differ from DWSIM at the 5th significant figure, consistent with the
rest of this crate.

## Design (crate `CLAUDE.md`)

Enum dispatch, **no `dyn`/`Box`/lifetimes**: the closed set of the two
Lee-Kesler reference fluids is the [`LkFluid`] enum, whose variants carry no
data and return their BWR constants from `const` methods. The
corresponding-states property routines are free functions over `f64` and
`&[Component]`, mirroring [`crate::thermo::eos_variants`].

## Honest scope — what is and is NOT ported

- **Ported:** the pure/mixture compressibility `Z` ([`z_lkp`], [`z_mix`]),
  the enthalpy and entropy departures ([`enthalpy_departure`],
  [`entropy_departure`], and their mixture forms), the pure
  corresponding-states log-fugacity ([`ln_phi_pure`]), and Plöcker's mixture
  critical combining rule ([`mix_crit_props`]).
- **NOT ported:** the full multicomponent fugacity-coefficient path
  (`CalcLnFugCPU` L956-1048), which differentiates the pseudo-critical
  properties w.r.t. composition (`dTcmdx`, `dPcmdx`) to get per-component
  `ln φ_i`; the `CPCV_LK` heat-capacity departures (L679-755); and the LKP
  binary-interaction `k_ij` data table (`lkp_ip.dat`, L70). The `k_ij` here
  defaults to the ideal `1.0` (geometric-mean `Tc` combining), matching
  DWSIM's own default when no table entry exists (`MixCritProp_LK` L104-110).
  These omissions are documented, not hidden.

> **⚠️ Untrusted AI-assisted draft — pending human V&V.** Early-stage
> translation. The tests below are *verification* against an independent
> analytic correlation (the Pitzer/Prausnitz generalized second virial) and
> against published Lee-Kesler behaviour, **not** validation against
> experimental data. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
> the official DWSIM.

```rust
pub mod lkp { /* ... */ }
```

### Types

#### Enum `Phase`

Fluid phase selector for the Lee-Kesler reduced-volume root.

The reduced-volume equation can have multiple roots below the critical
temperature; this picks which one the solver returns.

```rust
pub enum Phase {
    Vapor,
    Liquid,
}
```

##### Variants

###### `Vapor`

Vapour — the largest reduced-volume root.

###### `Liquid`

Liquid — the smallest strictly-positive reduced-volume root.

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

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Phase) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `LkFluid`

One of the two Lee-Kesler reference fluids.

Enum dispatch over the closed pair `{Simple, Reference}` (no trait objects
per the workspace `CLAUDE.md`). Each variant carries no data; the twelve
modified-BWR constants for that fluid are returned by [`LkFluid::constants`].

```rust
pub enum LkFluid {
    Simple,
    Reference,
}
```

##### Variants

###### `Simple`

The simple fluid (`ω = 0`), argon/methane-like.

###### `Reference`

The heavy reference fluid (`ω_ref = 0.3978`), n-octane.

##### Implementations

###### Methods

- ```rust
  pub const fn constants(self: Self) -> LkConstants { /* ... */ }
  ```
  The published Lee-Kesler (1975) BWR constants for this fluid

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LkFluid { /* ... */ }
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
    fn eq(self: &Self, other: &LkFluid) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `LkConstants`

The twelve modified-BWR constants of a Lee-Kesler reference fluid.

These parameterise the reduced pressure equation

`Z = 1 + B/Vr + C/Vr² + D/Vr⁵ + (c4 / (Tr³ Vr²)) (β + γ/Vr²) exp(−γ/Vr²)`,

with `B = b1 − b2/Tr − b3/Tr² − b4/Tr³`, `C = c1 − c2/Tr + c3/Tr³`,
`D = d1 + d2/Tr` (`LeeKeslerPlocker.vb` L284-299 / L363-374). All are
dimensionless.

```rust
pub struct LkConstants {
    pub b1: f64,
    pub b2: f64,
    pub b3: f64,
    pub b4: f64,
    pub c1: f64,
    pub c2: f64,
    pub c3: f64,
    pub c4: f64,
    pub d1: f64,
    pub d2: f64,
    pub beta: f64,
    pub gamma: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `b1` | `f64` | `b1` [-]. |
| `b2` | `f64` | `b2` [-]. |
| `b3` | `f64` | `b3` [-]. |
| `b4` | `f64` | `b4` [-]. |
| `c1` | `f64` | `c1` [-]. |
| `c2` | `f64` | `c2` [-]. |
| `c3` | `f64` | `c3` [-]. |
| `c4` | `f64` | `c4` [-]. |
| `d1` | `f64` | `d1` [-]. |
| `d2` | `f64` | `d2` [-]. |
| `beta` | `f64` | `β` [-]. |
| `gamma` | `f64` | `γ` [-]. |

##### Implementations

###### Methods

- ```rust
  pub fn b_of_tr(self: &Self, tr: f64) -> f64 { /* ... */ }
  ```
  The temperature-dependent group `B = b1 − b2/Tr − b3/Tr² − b4/Tr³` [-]

- ```rust
  pub fn c_of_tr(self: &Self, tr: f64) -> f64 { /* ... */ }
  ```
  The group `C = c1 − c2/Tr + c3/Tr³` [-] (`LeeKeslerPlocker.vb` L298).

- ```rust
  pub fn d_of_tr(self: &Self, tr: f64) -> f64 { /* ... */ }
  ```
  The group `D = d1 + d2/Tr` [-] (`LeeKeslerPlocker.vb` L299).

- ```rust
  pub fn z_from_vr(self: &Self, tr: f64, vr: f64) -> f64 { /* ... */ }
  ```
  Compressibility `Z = Pr·Vr/Tr` [-] from the reduced volume `vr` [-] at

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LkConstants { /* ... */ }
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
    fn eq(self: &Self, other: &LkConstants) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `reduced_volume`

**Attributes:**

- `MustUse { reason: None }`

Solve the Lee-Kesler reduced-volume `Vr` [-] for one reference fluid at
reduced temperature `tr` [-] and reduced pressure `pr` [-].

Finds `Vr > 0` such that `Pr·Vr/Tr = Z(Vr)` where `Z(Vr)` is
[`LkConstants::z_from_vr`]. This replaces DWSIM's `ESTIMAR_Vr2`
(`LeeKeslerPlocker.vb` L552-668, a scan + Brent hybrid) with a bracket-scan
plus bisection: the residual `f(Vr) = Pr·Vr/Tr − Z(Vr)` is scanned for a sign
change (descending in `Vr` for [`Phase::Vapor`], ascending for
[`Phase::Liquid`]) then bisected to a relative tolerance of `1e-12`.

Returns `None` if no positive root brackets in the scan window (should not
happen for physical gas-phase inputs `Tr ≳ 0.3`).

```rust
pub fn reduced_volume(fluid: LkFluid, tr: f64, pr: f64, phase: Phase) -> Option<f64> { /* ... */ }
```

#### Function `z_fluid`

**Attributes:**

- `MustUse { reason: None }`

Compressibility factor `Z` [-] of one Lee-Kesler reference fluid at reduced
`tr, pr` [-] (`LeeKeslerPlocker.vb` L323 `zs`/`zh`).

`Z = Pr·Vr/Tr` evaluated at the solved reduced volume [`reduced_volume`].
Returns `None` if the reduced-volume solve fails.

```rust
pub fn z_fluid(fluid: LkFluid, tr: f64, pr: f64, phase: Phase) -> Option<f64> { /* ... */ }
```

#### Function `z_lkp`

**Attributes:**

- `MustUse { reason: None }`

Lee-Kesler-Plöcker compressibility factor `Z` [-] of a pure fluid (or a
pseudo-pure mixture) at reduced temperature `tr = T/Tc` [-], reduced pressure
`pr = P/Pc` [-], and acentric factor `w = ω` [-].

The three-parameter corresponding-states interpolation
`Z = Z⁰ + (ω/0.3978)(Zʳᵉᶠ − Z⁰)` (`LeeKeslerPlocker.vb` L327), where `Z⁰` is
the simple-fluid compressibility and `Zʳᵉᶠ` the heavy-reference-fluid
compressibility, both at the *same* `tr, pr`. Valid over the gas and dense-gas
region where the Lee-Kesler tables apply (`0.3 ≲ Tr`, `Pr ≲ 10`). Returns
`None` if either reference-fluid reduced-volume solve fails.

With `w = 0` this returns exactly the simple-fluid `Z⁰`.

```rust
pub fn z_lkp(tr: f64, pr: f64, w: f64, phase: Phase) -> Option<f64> { /* ... */ }
```

#### Function `enthalpy_departure_dimensionless`

**Attributes:**

- `MustUse { reason: None }`

Dimensionless enthalpy departure `(H − H_ig) / (R Tc)` [-] of one reference
fluid at reduced `tr, pr` [-] (`LeeKeslerPlocker.vb` L361 / L384).

`= Tr [ Z − 1 − (b2 + 2b3/Tr + 3b4/Tr²)/(Tr Vr) − (c2 − 3c3/Tr²)/(2 Tr Vr²)`
`      + d2/(5 Tr Vr⁵) + 3E ]`, with `E = c4/(2 Tr³ γ)[β + 1 − (β + 1 +`
`γ/Vr²) exp(−γ/Vr²)]`. `Vr` is the fluid's own reduced volume. Returns `None`
if the reduced-volume solve fails.

```rust
pub fn enthalpy_departure_dimensionless(fluid: LkFluid, tr: f64, pr: f64, phase: Phase) -> Option<f64> { /* ... */ }
```

#### Function `entropy_departure_dimensionless`

**Attributes:**

- `MustUse { reason: None }`

Dimensionless entropy departure `(S − S_ig) / R` [-] of one reference fluid
at reduced `tr, pr` [-] (`LeeKeslerPlocker.vb` L483 / L508).

`= ln Z − (b1 + b3/Tr² + 2b4/Tr³)/Vr − (c1 − 2c3/Tr³)/(2 Vr²)`
`  − d1/(5 Vr⁵) + 2E`, with the same `E` as
[`enthalpy_departure_dimensionless`]. This is the residual entropy at the
system `(T, P)` relative to the ideal gas at the *same* `(T, P)`
(`ln Z` term; DWSIM's active branch uses `Math.Log(1)` reference,
`LeeKeslerPlocker.vb` L483). Returns `None` if the solve fails.

```rust
pub fn entropy_departure_dimensionless(fluid: LkFluid, tr: f64, pr: f64, phase: Phase) -> Option<f64> { /* ... */ }
```

#### Function `enthalpy_departure`

**Attributes:**

- `MustUse { reason: None }`

Pure-fluid enthalpy departure `H − H_ig` [J/mol] at reduced `tr, pr` [-],
acentric factor `w` [-], and critical temperature `tc` [K].

The LKP acentric interpolation of the dimensionless departure, scaled by
`R·Tc`: `H − H_ig = R Tc [ h⁰ + (ω/0.3978)(hʳᵉᶠ − h⁰) ]`
(`LeeKeslerPlocker.vb` L185/L386). `h⁰`, `hʳᵉᶠ` are
[`enthalpy_departure_dimensionless`] for the two reference fluids. Negative
for a real gas below its Boyle temperature (attraction dominates). Returns
`None` if a reduced-volume solve fails.

```rust
pub fn enthalpy_departure(tr: f64, pr: f64, w: f64, tc: f64, phase: Phase) -> Option<f64> { /* ... */ }
```

#### Function `entropy_departure`

**Attributes:**

- `MustUse { reason: None }`

Pure-fluid entropy departure `S − S_ig` [J/(mol·K)] at reduced `tr, pr` [-]
and acentric factor `w` [-].

`S − S_ig = R [ s⁰ + (ω/0.3978)(sʳᵉᶠ − s⁰) ]` (`LeeKeslerPlocker.vb`
L273/L510). Returns `None` if a reduced-volume solve fails.

```rust
pub fn entropy_departure(tr: f64, pr: f64, w: f64, phase: Phase) -> Option<f64> { /* ... */ }
```

#### Function `ln_phi_pure`

**Attributes:**

- `MustUse { reason: None }`

Natural log of the pure corresponding-states fugacity coefficient `ln φ` [-]
at reduced `tr, pr` [-] and acentric factor `w` [-].

Per reference fluid (`LeeKeslerPlocker.vb` L841):
`ln φ_f = Z − 1 − ln Z + B/Vr + C/(2 Vr²) + D/(5 Vr⁵) + E`, then the LKP
acentric interpolation `ln φ = ln φ⁰ + (ω/0.3978)(ln φʳᵉᶠ − ln φ⁰)`
(`LeeKeslerPlocker.vb` L869). As `Pr → 0`, `Z → 1` and `ln φ → 0` (ideal-gas
limit). This is the **pure** (or pseudo-pure mixture) fugacity coefficient;
the per-component mixture `ln φ_i` (DWSIM `CalcLnFugCPU`) is not ported (see
module scope). Returns `None` if a reduced-volume solve fails.

```rust
pub fn ln_phi_pure(tr: f64, pr: f64, w: f64, phase: Phase) -> Option<f64> { /* ... */ }
```

#### Function `mix_crit_props`

**Attributes:**

- `MustUse { reason: None }`

Mixture pseudo-critical properties `(Tcm [K], Pcm [Pa], Vcm [m³/mol], ωm [-])`
by the Plöcker combining rule (`MixCritProp_LK`, `LeeKeslerPlocker.vb`
L93-147).

For components with mole fractions `z` [-] (summing to 1), critical
temperatures `Tc_i` [K], critical volumes `Vc_i` [m³/mol], and acentric
factors `ω_i` [-] (all read from `comps`), with a symmetric `k_ij` [-] matrix
(passed as `kij`; `None` → the ideal `k_ij = 1` geometric-mean `Tc`
combining that DWSIM defaults to, L104-110):

- `Vc_ij = (1/8)(Vc_i^{1/3} + Vc_j^{1/3})³`,
- `Tc_ij = √(Tc_i Tc_j) · k_ij`,
- `Vcm = Σ_i Σ_j z_i z_j Vc_ij`,
- `Tcm = (1/Vcm^{0.25}) Σ_i Σ_j z_i z_j Vc_ij^{0.25} Tc_ij`,
- `ωm = Σ_i z_i ω_i`,
- `Pcm = (0.2905 − 0.085 ωm) R Tcm / Vcm`.

**Unit note.** DWSIM stores `Vc` in **cm³/mol** and carries a `/1000` factor
in `Vc_ij` (L116); this port takes `Vc` in **m³/mol** (the [`Component`]
convention) so that `/1000` is dropped and `Pcm` comes out in Pa directly
with the SI `R`. The `Pcm` prefactor `(0.2905 − 0.085 ωm)` is DWSIM's
Plöcker pseudo-critical-compressibility correlation (L143).

`kij` must be an `n×n` symmetric slice-of-slices with `k_ii = 1` on the
diagonal (or `None`). Panics if `comps`, `z`, and any `kij` row disagree in
length.

```rust
pub fn mix_crit_props(comps: &[crate::thermo::Component], z: &[f64], kij: Option<&[Vec<f64>]>) -> (f64, f64, f64, f64) { /* ... */ }
```

#### Function `z_mix`

**Attributes:**

- `MustUse { reason: None }`

Lee-Kesler-Plöcker mixture compressibility factor `Z` [-] at temperature
`t` [K] and pressure `p` [Pa].

Maps the mixture onto its Plöcker pseudo-criticals [`mix_crit_props`], then
applies the pure corresponding-states [`z_lkp`] at `Tr = T/Tcm`,
`Pr = P/Pcm`, `ωm`. `comps` need valid `critical_volume` (m³/mol). Returns
`None` if a reduced-volume solve fails.

```rust
pub fn z_mix(comps: &[crate::thermo::Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&[Vec<f64>]>) -> Option<f64> { /* ... */ }
```

#### Function `enthalpy_departure_mix`

**Attributes:**

- `MustUse { reason: None }`

Lee-Kesler-Plöcker mixture enthalpy departure `H − H_ig` [J/mol] at `t` [K],
`p` [Pa].

`H − H_ig = R Tcm [ h⁰ + (ωm/0.3978)(hʳᵉᶠ − h⁰) ]` on the pseudo-criticals
(`H_LK_MIX`, `LeeKeslerPlocker.vb` L185). Per **mole** of mixture (DWSIM
divides by the mixture molar mass to get J/kg; this port keeps J/mol,
matching [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]). Returns
`None` if a reduced-volume solve fails.

```rust
pub fn enthalpy_departure_mix(comps: &[crate::thermo::Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&[Vec<f64>]>) -> Option<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `OMEGA_REF`

The reference acentric factor `ω_ref` of the Lee-Kesler heavy reference fluid
(n-octane), `0.3978` [-] (`LeeKeslerPlocker.vb` L325).

The LKP interpolation weight is `ω / ω_ref`.

```rust
pub const OMEGA_REF: f64 = 0.3978;
```

## Module `pr1978`

Peng-Robinson 1978 (PR78) — the ω-dependent α-slope refit of Peng-Robinson.

Ported from DWSIM (GPL-3.0), Visual-Basic reference source:
- `DWSIM.Thermodynamics/PropertyPackages/Models/PengRobinson78.vb`
  (commit `1abf72d`): the α-slope branch `L145-151` (repeated at
  L321-327/L478-484/L790-796/L891-897/L1066-1072), the pure-component
  `a_i`/`b_i` at L150-151, and the shared PR compressibility solve `Z_PR`
  and fugacity `CalcLnFugCPU` that this port instead reuses from
  [`crate::thermo::cubic_eos`].

## What PR78 changes vs the base PR (1976)

PR78 is the *only* difference-from-base-PR: the α-function slope `κ(ω)`.
Peng & Robinson (1978) refit the slope for heavy / high-acentric-factor
species, splitting it at `ω = 0.491`:

`κ(ω) = 0.37464 + 1.54226 ω − 0.26992 ω²`                       (ω ≤ 0.491)
`κ(ω) = 0.379642 + 1.48503 ω − 0.164423 ω² + 0.016666 ω³`       (ω > 0.491)

(`PengRobinson78.vb` L145-149). Below the threshold PR78 is **identical** to
the base 1976 PR; above it the cubic-in-ω branch corrects the systematic
vapour-pressure error PR76 makes for heavy species. Everything else — the
co-volume `b_i = Ωb R Tc / Pc`, `Ωa = 0.45724`, the van der Waals one-fluid
mixing, the compressibility cubic, the fugacity coefficient, and the
enthalpy/entropy departures — is **unchanged**.

This module therefore **reuses** [`crate::thermo::cubic_eos`] wherever the
math is unchanged: the compressibility roots come from
[`CubicEos::z_roots`] / [`CubicEos::z_vapor`] / [`CubicEos::z_liquid`], and
the PR constants (`Ωa`, `Ωb`, `u`, `w`, `√(u²−4w)`) from the
[`CubicEos::PengRobinson`] accessors. Only the per-component attraction
`a_i(T)` (through `κ`) is re-derived here.

> **DWSIM low-ω constant note.** DWSIM's PR78 low-ω branch literally carries
> `1.5422` (`PengRobinson78.vb` L146), a truncation of the canonical PR
> `1.54226`. This port uses the canonical `1.54226` for the low-ω branch (by
> delegating to [`CubicEos::PengRobinson`]'s `alpha_slope`), so PR78 reduces
> **exactly** to the base PR for `ω ≤ 0.491`. The `< 6e-5` difference from
> DWSIM's truncated constant is far below physical significance.

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

Temperature K, pressure Pa, `a` J·m³/mol² (= Pa·m⁶/mol²), `b` m³/mol,
enthalpy departure J/mol, entropy departure J/(mol·K). `κ`, `α`, `Z`,
`Tr = T/Tc`, mole fractions `z`, and `k_ij` are dimensionless. Raw `f64`
matches the base kernel; every public signature spells out its units.

## Design (crate `CLAUDE.md`)

No `Box`/`dyn`, no lifetimes, no channels. Exposed as **free functions**
(mirroring [`crate::thermo::eos_variants`]) composed on top of the existing
[`CubicEos`] kernel — no new enum variant, because PR78 differs only in the
scalar `κ(ω)` and would otherwise duplicate the base kernel verbatim.

> **⚠️ Untrusted AI-assisted draft — pending human V&V.** Early-stage
> translation; the tests below are *verification* (does PR78 equal base PR
> below the crossover and diverge above it?), not validation against
> experimental VLE. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
> the official DWSIM.

```rust
pub mod pr1978 { /* ... */ }
```

### Functions

#### Function `pr78_kappa`

**Attributes:**

- `MustUse { reason: None }`

PR78 α-slope `κ(ω)` [-] (`PengRobinson78.vb` L145-149).

For `ω ≤ 0.491` this is the **canonical base-PR slope**
`0.37464 + 1.54226 ω − 0.26992 ω²` — obtained by delegating to
[`CubicEos::PengRobinson`]'s `alpha_slope`, so PR78 is bit-for-bit equal to
base PR below the threshold. For `ω > 0.491` it is the 1978 refit
`0.379642 + 1.48503 ω − 0.164423 ω² + 0.016666 ω³`. `ω` is the Pitzer
acentric factor [-]; valid over the usual `−0.1 ≲ ω ≲ 2` range.

```rust
pub fn pr78_kappa(acentric_factor: f64) -> f64 { /* ... */ }
```

#### Function `pr78_alpha`

**Attributes:**

- `MustUse { reason: None }`

PR78 α-function `α(T) = [1 + κ(1 − √Tr)]²` [-] for a component at `t` [K].

Uses [`pr78_kappa`] for the slope. `Tr = T/Tc`; `t > 0`. Equals 1 exactly at
the critical point (`Tr = 1`). For `ω ≤ 0.491` this equals
[`CubicEos::alpha`] for Peng-Robinson exactly.

```rust
pub fn pr78_alpha(comp: &crate::thermo::Component, t: f64) -> f64 { /* ... */ }
```

#### Function `pr78_a_i`

**Attributes:**

- `MustUse { reason: None }`

PR78 pure-component attraction `a_i(T) = 0.45724 · α_PR78(T) · R² Tc² / Pc`
[J·m³/mol²] at `t` [K] (`PengRobinson78.vb` L150-151).

Identical to [`CubicEos::a_i`] for Peng-Robinson except the α uses the 1978
slope [`pr78_alpha`]. The co-volume `b_i` is **unchanged** — obtain it from
`CubicEos::PengRobinson.b_i(comp)`. Valid for `t > 0`.

```rust
pub fn pr78_a_i(comp: &crate::thermo::Component, t: f64) -> f64 { /* ... */ }
```

#### Function `pr78_a_mix`

**Attributes:**

- `MustUse { reason: None }`

PR78 mixture attraction `a_mix = Σ_i Σ_j z_i z_j √(a_i a_j)(1 − k_ij)`
[J·m³/mol²] at `t` [K], using the PR78 pure-component `a_i` from
[`pr78_a_i`].

The **identical** van der Waals one-fluid mixing rule as
[`CubicEos::a_mix`]; only the per-component `a_i` differs. `z` are mole
fractions [-]; `kij = None` uses the geometric-mean rule. The mixture
co-volume is unchanged: use `CubicEos::PengRobinson.b_mix(comps, z)`.

# Panics
Panics (via slice indexing) if `comps` and `z` differ in length.

```rust
pub fn pr78_a_mix(comps: &[crate::thermo::Component], z: &[f64], t: f64, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> f64 { /* ... */ }
```

#### Function `z_factor`

**Attributes:**

- `MustUse { reason: None }`

Phase-selected PR78 compressibility factor `Z` [-] of a mixture at `t` [K],
`p` [Pa].

Assembles `A = a_mix P/(RT)²`, `B = b_mix P/(RT)` from the PR78 attraction
[`pr78_a_mix`] and the unchanged co-volume, then **reuses**
[`CubicEos::z_vapor`] / [`CubicEos::z_liquid`] for the root solve. `Vapor` →
largest real root; `Liquid` → smallest positive real root. Returns `None` if
the cubic yields no usable root.

```rust
pub fn z_factor(comps: &[crate::thermo::Component], z: &[f64], t: f64, p: f64, phase: crate::thermo::cubic_eos::Phase, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> Option<f64> { /* ... */ }
```

#### Function `ln_phi`

**Attributes:**

- `MustUse { reason: None }`

Natural log of the PR78 fugacity coefficient `ln φ_i` [-] for every component
in a phase at `t` [K], `p` [Pa].

The standard PR mixture expression (identical algebra to
[`CubicEos::ln_phi`], only the PR78 `a_i` fed in):

`ln φ_i = (b_i/b_m)(Z − 1) − ln(Z − B)`
`        − [A/(B√8)](2 Σ_k z_k a_ki / a_m − b_i/b_m)`
`          · ln[(2Z + B(2 + √8)) / (2Z + B(2 − √8))]`,

with `a_ki = √(a_k a_i)(1 − k_ki)`, `√8 = 2√2` for Peng-Robinson. As
`p → 0`, every `ln φ_i → 0`. Returns `None` if no `Z` root is found.

```rust
pub fn ln_phi(comps: &[crate::thermo::Component], z: &[f64], t: f64, p: f64, phase: crate::thermo::cubic_eos::Phase, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> Option<Vec<f64>> { /* ... */ }
```

#### Function `dadt`

**Attributes:**

- `MustUse { reason: None }`

PR78 temperature derivative of the mixture attraction `d a_mix / dT`
[J·m³/(mol²·K)] at `t` [K].

The DWSIM closed form (`Calc_dadT`), with the PR78 α-slope [`pr78_kappa`] as
the per-component `c_i`:

`da/dT = −(R/2)√(Ωa/T) Σ_i Σ_j z_i z_j (1 − k_ij)`
`        [c_j √(a_i Tc_j/Pc_j) + c_i √(a_j Tc_i/Pc_i)]`.

Feeds the entropy/enthalpy departures below.

```rust
pub fn dadt(comps: &[crate::thermo::Component], z: &[f64], t: f64, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> f64 { /* ... */ }
```

#### Function `enthalpy_departure`

**Attributes:**

- `MustUse { reason: None }`

PR78 molar enthalpy departure `H(T,P) − H_ideal(T)` [J/mol] for a phase at
`t` [K], `p` [Pa].

The generalised PR `(u, w) = (2, −1)` residual (identical form to
[`CubicEos::enthalpy_departure`], fed the PR78 `a_mix` and [`dadt`]):

`A_res = a_m/(b_m √8) ln[(2Z+B(2−√8))/(2Z+B(2+√8))] − RT ln((Z−B)/Z) − RT ln Z`,
`S_res = R ln((Z−B)/Z) + R ln Z − (da/dT)/(√8 b_m) ln[(2Z+B(2−√8))/(2Z+B(2+√8))]`,
`H_res = A_res + T S_res + RT(Z − 1)`.

Tends to 0 as `p → 0`. Returns `None` if no `Z` root is found.

```rust
pub fn enthalpy_departure(comps: &[crate::thermo::Component], z: &[f64], t: f64, p: f64, phase: crate::thermo::cubic_eos::Phase, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> Option<f64> { /* ... */ }
```

#### Function `entropy_departure`

**Attributes:**

- `MustUse { reason: None }`

PR78 molar entropy departure `S(T,P) − S_ideal(T,P)` [J/(mol·K)] for a phase.

The `S_res` term of [`enthalpy_departure`]. Tends to 0 as `p → 0`. Returns
`None` if no `Z` root is found.

```rust
pub fn entropy_departure(comps: &[crate::thermo::Component], z: &[f64], t: f64, p: f64, phase: crate::thermo::cubic_eos::Phase, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> Option<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `PR78_OMEGA_THRESHOLD`

The acentric-factor threshold `ω = 0.491` at which PR78 switches from the
base-PR slope to the 1978 cubic-in-ω branch (`PengRobinson78.vb` L145).

```rust
pub const PR78_OMEGA_THRESHOLD: f64 = 0.491;
```

## Module `pr_lee_kesler`

**Attributes:**

- `Other("#[forbid(unsafe_code)]")`

Peng-Robinson + Lee-Kesler enthalpy/entropy hybrid property package.

Ported from DWSIM (GPL-3.0), Visual-Basic reference source:
- `DWSIM.Thermodynamics/PropertyPackages/PengRobinsonLeeKesler.vb`
  (commit `1abf72d`): the class `PengRobinsonLKPropertyPackage` which
  `Inherits PropertyPackages.PengRobinsonPropertyPackage` (L31) and overrides
  only the *departure* path — `DW_CalcEnthalpy` L330-346,
  `DW_CalcEnthalpyDeparture` L348-361, `DW_CalcEntropy` L363-379,
  `DW_CalcEntropyDeparture` L381-393 (all delegating to the Lee-Kesler
  `m_lk`), while `DW_CalcFugCoeff` L395-399 and `DW_CalcP` L401-405 delegate
  to the inherited Peng-Robinson kernel `prn`.

## What this model is — a hybrid

This package is deliberately **two models glued together**:

- **Phase equilibrium (K-values, fugacity coefficients, the `Z` root used for
  fugacity, the flash) is pure Peng-Robinson** — it *inherits*
  `PengRobinsonPropertyPackage` and does not override `DW_CalcKvalue` /
  `DW_CalcFugCoeff` (`PengRobinsonLeeKesler.vb` L395-399 forwards fugacity to
  the PR kernel `prn`). So on this port the K-value / z-factor path is
  **identical, bit-for-bit, to [`crate::thermo::property_package::PropertyPackageModel::PengRobinson`]**.
- **Caloric departures (enthalpy `H − H_ig`, entropy `S − S_ig`) come from the
  Lee-Kesler (LKP) corresponding-states correlation instead of the PR EOS
  departure functions** (`DW_CalcEnthalpyDeparture` L348-361 /
  `DW_CalcEntropyDeparture` L381-393 call `m_lk.H_LK_MIX` / `m_lk.S_LK_MIX`
  with the ideal part set to `0`). LKP gives better caloric properties for
  light gases and their mixtures than the cubic EOS, while PR keeps the good
  VLE.

The physical motivation: a cubic EOS is excellent for phase equilibrium but
its enthalpy departure degrades for light real gases; Lee-Kesler's
three-parameter corresponding-states BWR reproduces the generalized
enthalpy-departure chart far better. This package takes each model where it
is strongest.

## Composition — reuses two already-ported kernels

- Phase-equilibrium / z-factor: [`crate::thermo::cubic_eos::CubicEos::PengRobinson`]
  and [`crate::thermo::property_package::PropertyPackageModel::PengRobinson`]
  (K-values, flash).
- Caloric departures: [`crate::thermo::lkp`] — [`lkp::enthalpy_departure_mix`],
  [`lkp::mix_crit_props`] + [`lkp::entropy_departure`], and (for DWSIM's
  reported compressibility property) [`lkp::z_mix`].

Nothing here re-derives EOS math; it is a thin, faithful composition layer.

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

Temperature K, pressure Pa, mole fractions dimensionless, enthalpy departure
J/mol, entropy departure J/(mol·K), compressibility `Z` dimensionless. Raw
`f64` in SI is used throughout, matching the two kernels this composes
([`crate::thermo::cubic_eos`], [`crate::thermo::lkp`]); every public signature
spells out its units.

## Design (workspace + crate `CLAUDE.md`)

Enum-dispatch / no `dyn`, no `Box`, no lifetimes: the package is the
zero-sized [`PengRobinsonLeeKesler`] marker struct, which implements the
compiler-enforced [`crate::thermo::property_package::PropertyPackage`]
contract (K-values + PT flash) and adds the LKP departure methods. The two
sub-kernels it dispatches to are themselves enum-based
([`CubicEos`], [`lkp::LkFluid`]). `#![forbid(unsafe_code)]` (also crate-wide).

## Honest scope — what is and is NOT ported

- **Ported:** the PR phase-equilibrium delegation ([`PengRobinsonLeeKesler::k_values`],
  [`PengRobinsonLeeKesler::flash_pt`], [`PengRobinsonLeeKesler::z_factor`]),
  the LKP mixture enthalpy departure
  ([`PengRobinsonLeeKesler::enthalpy_departure`]) and entropy departure
  ([`PengRobinsonLeeKesler::entropy_departure`]), and the LKP reported
  compressibility ([`PengRobinsonLeeKesler::compressibility_factor_lkp`],
  mirroring DWSIM's `DW_CalcProp` "compressibilityfactor" branch L133 which
  uses `Z_LK`, distinct from the PR `Z` used for fugacity).
- **NOT ported:**
  - **Solid-phase departures.** DWSIM's `DW_CalcEnthalpy`/`DW_CalcEntropy`
    add a heat-of-fusion term for `State.Solid` (L339, L372); only the
    liquid/vapour departures are ported (this crate has no solid model).
  - **`CpCvR_LK` heat-capacity departures** (`DW_CalcProp` "heatcapacity"
    L135-140, L265, L292) — the Lee-Kesler Cp/Cv departure is not ported in
    [`crate::thermo::lkp`], so it is not exposed here either.
  - **Per-component multicomponent LK fugacity.** Not relevant: this package
    takes fugacity from PR, not LK, so the un-ported LKP `CalcLnFugCPU` (see
    [`crate::thermo::lkp`] scope note) does not affect it.
  - **The absolute enthalpy/entropy (departure + ideal-gas reference).**
    DWSIM's `DW_CalcEnthalpy` adds `RET_Hid(298.15, T, …)` (L335); this port
    exposes the **departure only** (matching
    [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]); the caller
    adds the ideal-gas reference from [`crate::thermo::ideal_props`].
  - **Binary-interaction data tables** for both the PR mixing rule and the
    LKP critical-combining rule default to the ideal case (see the two
    kernels' scope notes).

> **⚠️ Untrusted AI-assisted draft — pending human V&V.** This is
> *verification* (are the two kernels composed correctly, and do the swapped
> departures reduce to the LKP correlation and vanish in the ideal-gas
> limit?), **not** validation against experimental caloric data. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions. Independent OUTRAM PARK fork, not the official DWSIM.

```rust
pub mod pr_lee_kesler { /* ... */ }
```

### Types

#### Struct `PengRobinsonLeeKesler`

The Peng-Robinson + Lee-Kesler hybrid property package.

A zero-sized marker type (no per-instance state, matching DWSIM where the
package holds only the shared `m_pr` / `m_lk` model singletons). Phase
equilibrium is Peng-Robinson; the enthalpy and entropy **departures** are
Lee-Kesler-Plöcker. See the module header for the full hybrid rationale and
honest scope.

Dispatch is by value (the type is `Copy`); it carries the compiler-enforced
[`PropertyPackage`] contract plus the LKP departure methods.

```rust
pub struct PengRobinsonLeeKesler;
```

##### Implementations

###### Methods

- ```rust
  pub fn k_values(self: Self, components: &[Component], x: &[f64], y: &[f64], t: f64, p: f64) -> Vec<f64> { /* ... */ }
  ```
  Equilibrium K-values `K_i = y_i / x_i` \[-\] — **pure Peng-Robinson**.

- ```rust
  pub fn flash_pt(self: Self, components: &[Component], z: &[f64], t: f64, p: f64) -> Result<FlashResult, FlashError> { /* ... */ }
  ```
  Isothermal-isobaric two-phase VLE flash — **pure Peng-Robinson**.

- ```rust
  pub fn z_factor(self: Self, components: &[Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&BinaryInteraction>) -> Option<f64> { /* ... */ }
  ```
  Phase-equilibrium compressibility factor `Z` \[-\] — **pure

- ```rust
  pub fn compressibility_factor_lkp(self: Self, components: &[Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&[Vec<f64>]>) -> Option<f64> { /* ... */ }
  ```
  Lee-Kesler-Plöcker compressibility factor `Z` \[-\] — the value DWSIM

- ```rust
  pub fn enthalpy_departure(self: Self, components: &[Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&[Vec<f64>]>) -> Option<f64> { /* ... */ }
  ```
  Molar **enthalpy departure** `H(T,P) − H_ig(T)` \[J/mol\] — **Lee-Kesler**,

- ```rust
  pub fn entropy_departure(self: Self, components: &[Component], z: &[f64], t: f64, p: f64, phase: Phase, kij: Option<&[Vec<f64>]>) -> Option<f64> { /* ... */ }
  ```
  Molar **entropy departure** `S(T,P) − S_ig(T,P)` \[J/(mol·K)\] —

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PengRobinsonLeeKesler { /* ... */ }
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
    fn default() -> PengRobinsonLeeKesler { /* ... */ }
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
    fn eq(self: &Self, other: &PengRobinsonLeeKesler) -> bool { /* ... */ }
    ```

- **PropertyPackage**
  - ```rust
    fn k_values(self: &Self, components: &[Component], x: &[f64], y: &[f64], t: f64, p: f64) -> Vec<f64> { /* ... */ }
    ```

  - ```rust
    fn flash_pt(self: &Self, components: &[Component], z: &[f64], t: f64, p: f64) -> Result<FlashResult, FlashError> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `property_package`

Property-package glue: compose the cubic EOS / ideal models into K-values and
drive an EOS-consistent isothermal-isobaric (**PT**) two-phase VLE flash.

Ported/composed from DWSIM (GPL-3.0). The "property package" concept mirrors
DWSIM's `DWSIM.Thermodynamics/PropertyPackages/PropertyPackage.vb`
abstraction — the object a flash asks for K-values / fugacities. DWSIM's
`PengRobinsonPropertyPackage.vb`, `SoaveRedlichKwong2PropertyPackage.vb`, and
`RaoultPropertyPackage.vb` each specialise `DW_CalcKvalue` to their model;
this module reproduces exactly that specialisation, sitting on the already-
ported kernel:

- K-values from a cubic EOS use the fugacity-coefficient ratio
  `K_i = φ_i^L(x, T, P) / φ_i^V(y, T, P)` (DWSIM `DW_CalcKvalue`, which calls
  `DW_CalcFugCoeff` for each phase — `PropertyPackage.vb` L4620-4680), with
  the liquid `Z`-root for `φ^L` and the vapour `Z`-root for `φ^V`, both from
  [`crate::thermo::cubic_eos::CubicEos::ln_phi`].
- The `Ideal` package returns the Wilson ideal K-estimate
  (`DW_CalcKvalue_Ideal_Wilson`, `PropertyPackage.vb` L1650-1668), which is a
  composition-independent Raoult-style first estimate.
- The flash itself is the nested-loops successive-substitution driver
  [`crate::thermo::flash::nested_loops_flash`], seeded with Wilson and closed
  with *this* package's [`PropertyPackageModel::k_values`].

## What this computes

Given a feed of overall mole fractions `z_i` \[-\] at fixed temperature
`T` \[K\] and pressure `P` \[Pa\], [`PropertyPackageModel::flash_pt`] splits
it into an equilibrium liquid (`x_i`) and vapour (`y_i`), returning the
vapour molar fraction `β` \[-\] and the K-values `K_i = y_i / x_i` \[-\]. For
a cubic-EOS package this is a genuine EOS-consistent flash: the converged
`(x, y, β)` satisfy `φ_i^L x_i = φ_i^V y_i` for every component (iso-fugacity)
to the outer-loop tolerance.

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

This package sits directly on the `f64`-SI kernel ([`crate::thermo::flash`],
[`crate::thermo::cubic_eos`]), whose inner EOS/flash arithmetic deliberately
uses raw `f64` in SI base units rather than `uom` (the crate `CLAUDE.md`
"raw f64 in inner EOS loops" rule). To avoid wrapping/unwrapping `uom` at
every call into that kernel — which would add friction without adding safety
over slices — the public signatures here follow the same convention:
temperature `T` \[K\], pressure `P` \[Pa\], mole fractions \[-\], all `f64`,
with every parameter's unit spelled out in its doc comment. (A `uom`-typed
outer façade, if desired later, belongs at the equipment-model boundary, not
in this inner composition layer.)

## Design (workspace + crate `CLAUDE.md`)

Enum dispatch, **no `dyn`**: [`PropertyPackageModel`] is the closed model set
`{Ideal, PengRobinson, Srk}`, not a trait object. The [`PropertyPackage`]
trait is a **compiler-enforced contract** (every model must supply `k_values`
and `flash_pt`); dispatch is by `match`, not `&dyn PropertyPackage`. The one
model-dependent step handed to the flash driver is a **generic `Fn`
closure**, not a trait object. No `Box`, no lifetimes, no channels.

## Honest scope — verification, NOT benchmark validation

- **TP (isothermal-isobaric) two-phase VLE flash only.** No PH/PS/TV/PV
  energy flashes; no three-phase (VLLE) or solid/salt equilibria. Those live
  in DWSIM's fuller flash suite and are out of scope (see
  [`crate::thermo::flash`]'s scope note).
- **No phase-stability pre-test.** This driver does *not* run a
  tangent-plane-distance / Michelsen stability analysis before flashing, so
  it can in principle converge to the trivial (single-phase) solution for a
  feed near a phase boundary. Stability testing is the separate
  [`crate::thermo::stability`] module's job and is not wired in here yet.
- **Binary interaction parameters `k_ij = 0`.** The cubic-EOS K-values use
  the geometric-mean combining rule (`None` `k_ij` matrix). Non-zero,
  fitted `k_ij` — which materially improve real VLE — are not applied here;
  pass them at the [`crate::thermo::cubic_eos`] layer if needed.
- **The EOS and flash are *verified*, not *validated*.** The tests below
  check internal consistency (mass balance, iso-fugacity, single-phase
  limits, K-ordering) and reduction to the Wilson-K result — they are **not**
  validated against an experimental / NIST / DECHEMA VLE benchmark. This is
  AI-assisted draft material, untrusted until human-reviewed per the crate
  `CLAUDE.md`. Not for nuclear facility operation, reactor control,
  safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
  the official DWSIM.

```rust
pub mod property_package { /* ... */ }
```

### Types

#### Enum `PropertyPackageModel`

Thermodynamic property-package model (enum dispatch, no `dyn`).

The closed set of PT-flash property models this crate composes from the
thermo kernel. `Copy` so it can be captured by value into the flash driver's
`Fn` closure without borrowing.

- [`PropertyPackageModel::Ideal`] — Raoult/Wilson ideal K-values
  (composition-independent); the K-closure is the Wilson estimate, so a flash
  with this package reduces to a single Wilson-K Rachford-Rice solve.
- [`PropertyPackageModel::PengRobinson`] — Peng-Robinson cubic EOS; K-values
  are the liquid/vapour fugacity-coefficient ratio.
- [`PropertyPackageModel::Srk`] — Soave-Redlich-Kwong cubic EOS; same
  fugacity-ratio K-values with the SRK constants.

```rust
pub enum PropertyPackageModel {
    Ideal,
    PengRobinson,
    Srk,
}
```

##### Variants

###### `Ideal`

Ideal (Raoult's law) package: K-values are the composition-independent
Wilson ideal estimate `K_i = (Pc_i/P)·exp[5.373(1+ω_i)(1−Tc_i/T)]`.

###### `PengRobinson`

Peng-Robinson cubic-EOS package.

###### `Srk`

Soave-Redlich-Kwong cubic-EOS package.

##### Implementations

###### Methods

- ```rust
  pub fn k_values(self: Self, components: &[Component], x: &[f64], y: &[f64], t: f64, p: f64) -> Vec<f64> { /* ... */ }
  ```
  Equilibrium K-values `K_i = y_i / x_i` \[-\] for a trial split.

- ```rust
  pub fn flash_pt(self: Self, components: &[Component], z: &[f64], t: f64, p: f64) -> Result<FlashResult, FlashError> { /* ... */ }
  ```
  Isothermal-isobaric two-phase VLE flash of feed `z` at `t` \[K\], `p`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PropertyPackageModel { /* ... */ }
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
    fn eq(self: &Self, other: &PropertyPackageModel) -> bool { /* ... */ }
    ```

- **PropertyPackage**
  - ```rust
    fn k_values(self: &Self, components: &[Component], x: &[f64], y: &[f64], t: f64, p: f64) -> Vec<f64> { /* ... */ }
    ```

  - ```rust
    fn flash_pt(self: &Self, components: &[Component], z: &[f64], t: f64, p: f64) -> Result<FlashResult, FlashError> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Trait `PropertyPackage`

Compiler-enforced contract every property-package model must satisfy.

This trait is **not** used for `dyn` dispatch (forbidden by the workspace
`CLAUDE.md`); it exists so the compiler checks that every model in
[`PropertyPackageModel`] supplies a K-value routine and a PT flash. Runtime
dispatch is done by matching on the enum. Both methods use the documented
`f64`-SI convention (see the module header): `T` \[K\], `P` \[Pa\], mole
fractions \[-\].

```rust
pub trait PropertyPackage {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `k_values`: Equilibrium K-values `K_i = y_i / x_i` \[-\] for a trial liquid `x` and
- `flash_pt`: Isothermal-isobaric two-phase VLE flash of feed `z` (mole fractions

##### Implementations

This trait is implemented for the following types:

- `PengRobinsonLeeKesler`
- `PropertyPackageModel`

## Module `prsv2_full`

Full Peng-Robinson-Stryjek-Vera 2 (PRSV2) property package — the
three-parameter (`κ1, κ2, κ3`) α-function with a working Z / fugacity /
departure / vapour-pressure surface.

Ported from DWSIM (GPL-3.0), Visual-Basic reference source:
- `DWSIM.Thermodynamics/PropertyPackages/PengRobinsonStryjekVera2.vb`
  (commit `1abf72d`): the package-level `κ1/κ2/κ3` selectors
  `RET_KAPPA1/2/3` L481-537, the `DW_CalcFugCoeff` / `AUX_Z` entry points
  L880-914, `DW_CalcPVAP_ISOL` L417.
- `DWSIM.Thermodynamics/PropertyPackages/Models/PRSV2.vb` (commit `1abf72d`):
  the full three-parameter α-slope `L375-385` (repeated at
  L163-173/L554-564/L740-750), `a_i`/`b_i` L386-387, `Z_PR` L347-516.

## What "full PRSV2" adds over `eos_variants`

[`crate::thermo::eos_variants`] already ports the **one-parameter** PRSV
α-function (`κ = κ0 + κ1 (1 + √Tr)(0.7 − Tr)`) as free functions. This module
adds the two remaining PRSV2 parameters and a **complete package**:

1. The three-parameter slope (`Models/PRSV2.vb` L376):

   `κ = κ0(ω) + [κ1 + κ2 (κ3 − Tr)(1 − √Tr)](1 + √Tr)(0.7 − Tr)`,

   with `κ0(ω) = 0.378893 + 1.4897153 ω − 0.17131848 ω² + 0.0196554 ω³`
   (reused from [`crate::thermo::eos_variants::prsv_kappa0`]).
2. A compressibility `Z`, per-component fugacity coefficient `ln φ_i`, and
   enthalpy/entropy departures — reusing the base PR machinery in
   [`crate::thermo::cubic_eos`] (compressibility roots, PR constants,
   `(u, w) = (2, −1)` departure form) with only the PRSV2 `a_i(T)` swapped in.
3. A pure-component **vapour-pressure** solver [`vapor_pressure`] (DWSIM's
   `DW_CalcPVAP_ISOL`), used in the V&V test below.

## κ-correction activation (DWSIM guard, relaxed)

DWSIM only applies the three-parameter slope when `κ1·κ2·κ3 ≠ 0`, else it
falls back to the base PR76/PR78 slope (`Models/PRSV2.vb` L375-385). That
guard makes the *one-parameter* PRSV limit (`κ2 = κ3 = 0`) unreachable. This
port instead activates on **`κ1 ≠ 0`** so the one-parameter PRSV (matching
[`crate::thermo::eos_variants`]) and the full three-parameter PRSV2 are both
reachable; with `κ1 = 0` it reduces to the **base Peng-Robinson** slope
exactly (delegating to [`crate::thermo::pr1978::pr78_kappa`], which is the
canonical base PR for `ω ≤ 0.491` and the 1978 refit above it — precisely
DWSIM's own fallback). This is documented, not hidden.

## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

Temperature K, pressure Pa, `a` J·m³/mol² (= Pa·m⁶/mol²), `b` m³/mol,
enthalpy departure J/mol, entropy departure J/(mol·K). `κ0..κ3`, `α`, `Z`,
`Tr = T/Tc`, mole fractions `z`, and `k_ij` are dimensionless. Raw `f64`
matches the base kernel; every public signature spells out its units.

## Design (crate `CLAUDE.md`)

No `Box`/`dyn`, no lifetimes, no channels. Free functions composed on the
[`crate::thermo::cubic_eos::CubicEos`] kernel — the PRSV2 `(κ1, κ2, κ3)` are
**per-component** data (arrays over the mixture), which an EOS-selector enum
carrying no per-component state cannot hold, so free functions taking the
parameter arrays are the right shape (identical rationale to
[`crate::thermo::eos_variants`]).

## Honest scope — what is and is NOT ported

- **Ported:** the three-parameter α, `a_i`/`a_mix`, phase-selected `Z`,
  per-component `ln φ_i`, enthalpy/entropy departures, and the pure-component
  vapour pressure.
- **NOT ported:** DWSIM's asymmetric (Panagiotopoulos-Reid) composition-
  dependent mixing term `(1 − x_i k_ij − x_j k_ji)` (`Models/PRSV2.vb` L395);
  this port uses the **symmetric** van der Waals one-fluid rule
  `√(a_i a_j)(1 − k_ij)` (as [`crate::thermo::cubic_eos::CubicEos::a_mix`]),
  which is the `k_ij = k_ji` special case. The multicomponent
  bubble/dew/flash driver is out of scope here (use
  [`crate::thermo::flash`] / [`crate::thermo::saturation`] with these
  fugacities once wired). The `κ1/κ2/κ3` compound data table is deferred —
  parameters are passed in (default `0.0`).

> **⚠️ Untrusted AI-assisted draft — pending human V&V.** Early-stage
> translation; the tests are *verification* (κ1=0 reduces to base PR; vapour
> pressure vs a reference), not validation against experimental VLE
> databases. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
> the official DWSIM.

```rust
pub mod prsv2_full { /* ... */ }
```

### Functions

#### Function `prsv2_kappa`

**Attributes:**

- `MustUse { reason: None }`

Full PRSV2 three-parameter α-slope `κ(T)` [-] for a component
(`Models/PRSV2.vb` L376).

When the correction is **active** (`κ1 ≠ 0`):

`κ = κ0(ω) + [κ1 + κ2 (κ3 − Tr)(1 − √Tr)](1 + √Tr)(0.7 − Tr)`,

with `κ0` from [`prsv_kappa0`], `Tr = T/Tc` [-]. When **inactive**
(`κ1 = 0`) it returns the base Peng-Robinson slope
([`pr78_kappa`]) — i.e. standard PR for `ω ≤ 0.491`. The three fitted
parameters `kappa1, kappa2, kappa3` are dimensionless; `t` [K] must be `> 0`.
The `(0.7 − Tr)` factor makes the `κ1` term change sign at `Tr = 0.7`, the
anchor of the Stryjek-Vera fit.

```rust
pub fn prsv2_kappa(comp: &crate::thermo::Component, kappa1: f64, kappa2: f64, kappa3: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `prsv2_alpha`

**Attributes:**

- `MustUse { reason: None }`

Full PRSV2 α-function `α(T) = [1 + κ(1 − √Tr)]²` [-] for a component at
`t` [K] (`Models/PRSV2.vb` L377).

`κ` is [`prsv2_kappa`]; `Tr = T/Tc`; `t > 0`. Equals 1 exactly at the
critical point (`Tr = 1`) for any parameters. With `κ1 = 0` this is the base
PR α exactly.

```rust
pub fn prsv2_alpha(comp: &crate::thermo::Component, kappa1: f64, kappa2: f64, kappa3: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `prsv2_a_i`

**Attributes:**

- `MustUse { reason: None }`

Full PRSV2 pure-component attraction
`a_i(T) = 0.45724 · α_PRSV2(T) · R² Tc² / Pc` [J·m³/mol²] at `t` [K]
(`Models/PRSV2.vb` L386).

Same `Ωa = 0.45724` and `b_i` as base Peng-Robinson; only the α differs. Get
the unchanged co-volume from `CubicEos::PengRobinson.b_i(comp)`. Valid for
`t > 0`.

```rust
pub fn prsv2_a_i(comp: &crate::thermo::Component, kappa1: f64, kappa2: f64, kappa3: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `prsv2_a_mix`

**Attributes:**

- `MustUse { reason: None }`

Full PRSV2 mixture attraction `a_mix = Σ_i Σ_j z_i z_j √(a_i a_j)(1 − k_ij)`
[J·m³/mol²] at `t` [K] (symmetric van der Waals one-fluid rule).

Identical mixing to [`CubicEos::a_mix`]; only the per-component `a_i` uses the
PRSV2 α. `k1/k2/k3` are per-component parameter slices; `z` mole fractions
[-]; `kij = None` → geometric mean. Mixture co-volume is unchanged
(`CubicEos::PengRobinson.b_mix`).

# Panics
Panics if `comps`, `z`, `k1`, `k2`, `k3` differ in length.

```rust
pub fn prsv2_a_mix(comps: &[crate::thermo::Component], k1: &[f64], k2: &[f64], k3: &[f64], z: &[f64], t: f64, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> f64 { /* ... */ }
```

#### Function `z_factor`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Phase-selected full-PRSV2 compressibility factor `Z` [-] of a mixture at
`t` [K], `p` [Pa].

`A = a_mix P/(RT)²`, `B = b_mix P/(RT)` from the PRSV2 attraction, then reuses
[`CubicEos::z_vapor`] / [`CubicEos::z_liquid`]. Returns `None` if the cubic
yields no usable root.

```rust
pub fn z_factor(comps: &[crate::thermo::Component], k1: &[f64], k2: &[f64], k3: &[f64], z: &[f64], t: f64, p: f64, phase: crate::thermo::cubic_eos::Phase, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> Option<f64> { /* ... */ }
```

#### Function `ln_phi`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Natural log of the full-PRSV2 fugacity coefficient `ln φ_i` [-] for every
component in a phase at `t` [K], `p` [Pa].

The standard PR mixture expression (identical to [`CubicEos::ln_phi`], only
the PRSV2 `a_i` fed in), `(u, w) = (2, −1)`, `√8 = 2√2`:

`ln φ_i = (b_i/b_m)(Z − 1) − ln(Z − B)`
`        − [A/(B√8)](2 Σ_k z_k a_ki / a_m − b_i/b_m)`
`          · ln[(2Z + B(2 + √8)) / (2Z + B(2 − √8))]`.

As `p → 0`, every `ln φ_i → 0`. Returns `None` if no `Z` root is found.

```rust
pub fn ln_phi(comps: &[crate::thermo::Component], k1: &[f64], k2: &[f64], k3: &[f64], z: &[f64], t: f64, p: f64, phase: crate::thermo::cubic_eos::Phase, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> Option<Vec<f64>> { /* ... */ }
```

#### Function `dadt`

**Attributes:**

- `MustUse { reason: None }`

Full-PRSV2 temperature derivative `d a_mix / dT` [J·m³/(mol²·K)] at `t` [K].

DWSIM's `Calc_dadT` closed form with the PRSV2 α-slope [`prsv2_kappa`] as
each `c_i`:

`da/dT = −(R/2)√(Ωa/T) Σ_i Σ_j z_i z_j (1 − k_ij)`
`        [c_j √(a_i Tc_j/Pc_j) + c_i √(a_j Tc_i/Pc_i)]`.

Feeds the departures below.

```rust
pub fn dadt(comps: &[crate::thermo::Component], k1: &[f64], k2: &[f64], k3: &[f64], z: &[f64], t: f64, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> f64 { /* ... */ }
```

#### Function `enthalpy_departure`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Full-PRSV2 molar enthalpy departure `H(T,P) − H_ideal(T)` [J/mol] for a phase
at `t` [K], `p` [Pa].

Same generalised PR `(u, w) = (2, −1)` residual as
[`CubicEos::enthalpy_departure`], fed the PRSV2 `a_mix` and [`dadt`]. Tends to
0 as `p → 0`. Returns `None` if no `Z` root is found.

```rust
pub fn enthalpy_departure(comps: &[crate::thermo::Component], k1: &[f64], k2: &[f64], k3: &[f64], z: &[f64], t: f64, p: f64, phase: crate::thermo::cubic_eos::Phase, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> Option<f64> { /* ... */ }
```

#### Function `entropy_departure`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Full-PRSV2 molar entropy departure `S(T,P) − S_ideal(T,P)` [J/(mol·K)] for a
phase. The `S_res` term of [`enthalpy_departure`]. Returns `None` if no `Z`
root is found.

```rust
pub fn entropy_departure(comps: &[crate::thermo::Component], k1: &[f64], k2: &[f64], k3: &[f64], z: &[f64], t: f64, p: f64, phase: crate::thermo::cubic_eos::Phase, kij: Option<&crate::thermo::cubic_eos::BinaryInteraction>) -> Option<f64> { /* ... */ }
```

#### Function `vapor_pressure`

**Attributes:**

- `MustUse { reason: None }`

Pure-component saturation (vapour) pressure `Psat(T)` [Pa] under full PRSV2
(DWSIM `DW_CalcPVAP_ISOL`, `PengRobinsonStryjekVera2.vb` L417).

Solves the pure-fluid equal-fugacity condition `φ_L(T, P) = φ_V(T, P)` by
successive substitution `P ← P · exp(ln φ_L − ln φ_V)`, seeded with the Wilson
estimate `P₀ = Pc · exp[5.373 (1 + ω)(1 − Tc/T)]`. `comp` supplies `Tc, Pc,
ω`; `kappa1/2/3` are the compound's PRSV2 parameters (`0.0` → base PR). `t`
[K] must be below `Tc` (a saturation pressure exists only sub-critically).

Returns `None` if `t ≥ Tc`, if two distinct liquid/vapour roots never appear
(no two-phase region at that `T`), or if the iteration fails to converge in
200 steps. Converges when `|ln φ_L − ln φ_V| < 1e-10`.

**Physical range.** Valid over roughly `0.5 Tc ≲ T < Tc`; far below `Tc` the
cubic's liquid root can be tiny and the successive substitution slow.

```rust
pub fn vapor_pressure(comp: &crate::thermo::Component, kappa1: f64, kappa2: f64, kappa3: f64, t: f64) -> Option<f64> { /* ... */ }
```

## Module `saturation`

Saturation: **bubble-point / dew-point** temperature & pressure of a
multicomponent mixture, on top of the isothermal-isobaric VLE kernel.

Ported/adapted from DWSIM `DWSIM.Thermodynamics/FlashAlgorithms/NestedLoops.vb`
(GPL-3.0): `Flash_PV` (bubble/dew *temperature* at fixed pressure, the
`V = 0` / `V = 1` branches — line 2662) and `Flash_TV` (bubble/dew *pressure*
at fixed temperature — line 2028). Both DWSIM routines specialise to the
incipient-phase condition when the vapour molar fraction `V` is exactly `0`
(bubble, incipient vapour) or `1` (dew, incipient liquid); the initial K
estimates use the vapour-pressure / Wilson seed
(`PropertyPackage.vb::DW_CalcKvalue_Ideal_Wilson`, L1650-1668, mirrored by
[`crate::thermo::flash::wilson_k_values`]). GUI / serialization / flowsheet /
AI-assisted-convergence scaffolding around those routines is **not** ported.

# What this module computes

For a feed of overall mole fractions `z_i` \[-\] the **saturation curve** is
the locus where an infinitesimal amount of a second phase first appears:

- **Bubble point** — the feed is a saturated liquid (`x_i = z_i`) about to
  boil; the incipient vapour has `y_i = K_i z_i`, and the saturation
  condition is

  ```text
  Σ_i K_i z_i = 1.
  ```

- **Dew point** — the feed is a saturated vapour (`y_i = z_i`) about to
  condense; the incipient liquid has `x_i = z_i / K_i`, and the condition is

  ```text
  Σ_i z_i / K_i = 1.
  ```

Here `K_i = y_i / x_i` \[-\] are the equilibrium ratios from a fugacity /
activity property model. Four public entry points solve for the missing
intensive variable:

| Function | Fixed | Solved for | Incipient phase |
|---|---|---|---|
| [`bubble_pressure`] | `T` | `P` | vapour `y_i = K_i z_i` |
| [`dew_pressure`]    | `T` | `P` | liquid `x_i = z_i / K_i` |
| [`bubble_temperature`] | `P` | `T` | vapour `y_i = K_i z_i` |
| [`dew_temperature`]    | `P` | `T` | liquid `x_i = z_i / K_i` |

# The K-value source (decoupling boundary — no `dyn`)

Each public function comes in two forms:

- a convenience form taking a [`PropertyPackageModel`] (its
  [`PropertyPackageModel::k_values`] supplies the K-values — Wilson for the
  ideal package, a fugacity-coefficient ratio for the cubic EOS packages);
- a generic `*_with` form taking a **generic `Fn` closure**
  `k_values(x, y, T, P) -> Vec<f64>` (any ln-φ / K model), *not* a trait
  object. This keeps the module free of `dyn` dispatch and independent of the
  EOS/activity code, matching the [`crate::thermo::flash`] and
  [`crate::thermo::property_package`] pattern (no `Box`, no lifetimes, no
  channels).

For a **composition-dependent** K-model (e.g. a cubic EOS) the incipient
composition feeds back into the K-values, so each residual evaluation runs a
short inner successive-substitution loop on the incipient phase before the
saturation residual is read off. For a **composition-independent** K-model
(ideal / Wilson / Raoult) that inner loop is trivial and the residual is a
plain monotone function of the solved variable.

**Trivial-solution caveat (composition-dependent K only).** Plain successive
substitution has the trivial solution (`K → 1`, incipient composition `→ z`)
as an *attracting* fixed point. The Wilson-seeded start breaks the initial
symmetry but does not guarantee escape from it for a cubic EOS, so a
cubic-EOS bubble/dew solve here can converge to that spurious point. Robust
cubic saturation requires a phase-stability / tangent-plane pre-test
([`crate::thermo::stability`]) that is **not** wired in. The **verified,
relied-upon** path in this module is therefore the composition-independent
K-model (ideal/Wilson/Raoult); the cubic path is offered but carries no
robustness guarantee — treat its result as unverified until a stability
pre-test is added.

# Convergence

- **Initial guess.** From the Wilson K-values ([`crate::thermo::flash::wilson_k_values`]).
  Because Wilson `K_i = (Pc_i / P) exp[5.373 (1 + ω_i)(1 − Tc_i / T)]`, the
  bubble/dew *pressure* has a closed-form Wilson seed
  (`P_bub = Σ z_i Pc_i E_i`, `P_dew = 1 / Σ z_i /(Pc_i E_i)` with
  `E_i = exp[5.373(1+ω_i)(1−Tc_i/T)]`); the *temperature* seed inverts the
  same expression per component and takes the feed-weighted mean.
- **Root find.** A **safeguarded** scalar solver: geometric bracket
  expansion around the Wilson seed to obtain a sign-changing interval,
  followed by the **Illinois** modified-false-position iteration (globally
  convergent on a bracketed continuous residual, with super-linear local
  rate). The saturation residual (`Σ K_i z_i − 1` or `Σ z_i/K_i − 1`) is
  driven below a tolerance.

# Honest scope — verification, NOT benchmark validation

- **Two-phase VLE saturation only.** Bubble/dew of a single vapour–liquid
  equilibrium. **No** three-phase (VLLE), solid/salt-out, or electrolyte
  saturation; **no** retrograde-region robustness guarantees (near the
  critical point a bubble/dew *pressure* isotherm can be non-monotone or
  double-valued — the safeguarded solver returns the first bracketed root it
  finds, which may not be the physically intended branch there).
- **K-model is the caller's choice**: ideal/Wilson or a cubic EOS via
  [`PropertyPackageModel`], or any generic K-closure. Binary interaction
  parameters follow whatever the supplied model uses (the cubic packages here
  use `k_ij = 0`; see [`crate::thermo::property_package`]).
- **Verified, not validated.** The tests below check the *defining
  saturation identity*, incipient-composition normalisation, the
  bubble > dew pressure ordering, a pure-component collapse to the vapour
  pressure, and pressure/temperature round-tripping — internal consistency
  against closed-form relations, **not** experimental / NIST / DECHEMA VLE
  data. AI-assisted port: untrusted draft material until human-reviewed per
  the crate `CLAUDE.md`. Not for nuclear facility operation, reactor control,
  safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
  the official DWSIM.

# Units — documented raw `f64` (SI), per the crate `CLAUDE.md`

This module sits directly on the `f64`-SI kernel ([`crate::thermo::flash`],
[`crate::thermo::property_package`]), whose inner arithmetic deliberately uses
raw `f64` in SI base units rather than `uom` (the crate `CLAUDE.md` "raw f64
in inner EOS loops" rule). To avoid wrapping/unwrapping `uom` at every call
into that kernel the signatures here follow the same convention: temperature
`T` \[K\], pressure `P` \[Pa\], mole fractions \[-\], all `f64`, with every
parameter's unit spelled out. A `uom`-typed façade, if wanted, belongs at the
equipment-model boundary, not in this inner composition layer.

```rust
pub mod saturation { /* ... */ }
```

### Types

#### Struct `SaturationState`

A converged saturation point (one point on the bubble or dew curve).

All compositions are mole fractions \[-\]. Exactly one of `temperature` /
`pressure` is the solved-for unknown; the other is the fixed specification
the caller supplied.

```rust
pub struct SaturationState {
    pub temperature: f64,
    pub pressure: f64,
    pub incipient: Vec<f64>,
    pub k: Vec<f64>,
    pub iterations: usize,
    pub residual: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `f64` | Temperature `T` \[K\] at the saturation point (the fixed value for the<br>`*_pressure` solvers, the solved value for the `*_temperature` solvers). |
| `pressure` | `f64` | Pressure `P` \[Pa\] at the saturation point (solved for the `*_pressure`<br>solvers, fixed for the `*_temperature` solvers). |
| `incipient` | `Vec<f64>` | Incipient-phase mole fractions \[-\]: the **vapour** `y_i = K_i z_i` for a<br>bubble point, the **liquid** `x_i = z_i / K_i` for a dew point. At exact<br>convergence these sum to `1` (their sum is `residual + 1`). |
| `k` | `Vec<f64>` | Equilibrium K-values `K_i = y_i / x_i` \[-\] at the returned state. |
| `iterations` | `usize` | Inner successive-substitution iterations used at the final residual<br>evaluation (`1`–`2` for a composition-independent K-model, more for a<br>cubic EOS). |
| `residual` | `f64` | Saturation residual at the returned state: `Σ K_i z_i − 1` for a bubble<br>point, `Σ z_i / K_i − 1` for a dew point \[-\]. `|residual|` is below the<br>solver's function tolerance on success. |

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
    fn clone(self: &Self) -> SaturationState { /* ... */ }
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
    fn eq(self: &Self, other: &SaturationState) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SaturationError`

Error conditions for the saturation solvers.

```rust
pub enum SaturationError {
    Empty,
    LengthMismatch {
        a: usize,
        b: usize,
    },
    NonFinite,
    NonPositive {
        what: &'static str,
        value: f64,
    },
    NoBracket {
        var: &'static str,
        residual: f64,
    },
    NotConverged {
        var: &'static str,
        iterations: usize,
        residual: f64,
    },
}
```

##### Variants

###### `Empty`

An empty feed was supplied (need at least one component).

###### `LengthMismatch`

Two slices that must share a length did not.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `usize` | Length of the first slice (e.g. `z`). |
| `b` | `usize` | Length of the second slice (e.g. `components` or the returned `K`). |

###### `NonFinite`

A non-finite value (`NaN`/`inf`) appeared in an input or a returned
K-value.

###### `NonPositive`

A quantity that must be strictly positive was not.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which quantity (e.g. `"temperature"`). |
| `value` | `f64` | The offending value. |

###### `NoBracket`

The bracket-expansion phase could not find a sign change of the
saturation residual within its bounds (e.g. no saturation point exists
at the given specification, or it lies outside the search bounds).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `var` | `&'static str` | The variable being solved for (`"pressure"` or `"temperature"`). |
| `residual` | `f64` | Smallest `|residual|` seen while trying to bracket. |

###### `NotConverged`

The root iteration did not reach the residual tolerance in budget.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `var` | `&'static str` | The variable being solved for. |
| `iterations` | `usize` | Iterations attempted. |
| `residual` | `f64` | Final `|residual|`. |

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
    fn clone(self: &Self) -> SaturationError { /* ... */ }
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: SaturationError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SaturationError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SaturationOptions`

Tuning parameters for the safeguarded saturation root finds.

Defaults are tight enough for the verification tests: a small saturation-
residual tolerance and generous physical search bounds. All fields are `f64`
/ `usize` with the units noted.

```rust
pub struct SaturationOptions {
    pub f_tol: f64,
    pub x_rel_tol: f64,
    pub max_outer: usize,
    pub inner_tol: f64,
    pub inner_max: usize,
    pub p_min: f64,
    pub p_max: f64,
    pub t_min: f64,
    pub t_max: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `f_tol` | `f64` | Convergence tolerance on the saturation residual `|Σ K z − 1|` (bubble)<br>or `|Σ z/K − 1|` (dew) \[-\]. |
| `x_rel_tol` | `f64` | Relative convergence tolerance on the solved variable (`P` or `T`) \[-\]. |
| `max_outer` | `usize` | Maximum outer (Illinois) iterations before [`SaturationError::NotConverged`]. |
| `inner_tol` | `f64` | Inner successive-substitution tolerance on the incipient composition<br>(max per-component change) \[-\]. |
| `inner_max` | `usize` | Maximum inner successive-substitution iterations per residual evaluation. |
| `p_min` | `f64` | Lower bound on the pressure search \[Pa\]. |
| `p_max` | `f64` | Upper bound on the pressure search \[Pa\]. |
| `t_min` | `f64` | Lower bound on the temperature search \[K\]. |
| `t_max` | `f64` | Upper bound on the temperature search \[K\]. |

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
    fn clone(self: &Self) -> SaturationOptions { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &SaturationOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `bubble_pressure_with`

Bubble **pressure** at fixed temperature with a generic K-closure.

Solves `Σ_i K_i(z, y, T, P) z_i = 1` for `P` \[Pa\], with the incipient
vapour `y_i = K_i z_i`. `k_values(x, y, T, P) -> Vec<f64>` is any K-model
(generic `Fn`, no `dyn`); it is called with the liquid `x = z` and the
current incipient vapour `y`.

# Units / ranges

`components.len() == z.len()`; `z` are feed mole fractions \[-\] (physical
feeds sum to 1); `temperature` `T` \[K\] > 0. Returns a [`SaturationState`]
whose `pressure` is the solved bubble pressure \[Pa\] and whose `incipient`
is the raw incipient vapour `K_i z_i` \[-\].

# Errors

[`SaturationError::Empty`] / [`SaturationError::LengthMismatch`] /
[`SaturationError::NonFinite`] on bad inputs, [`SaturationError::NonPositive`]
for `T ≤ 0`, and [`SaturationError::NoBracket`] /
[`SaturationError::NotConverged`] if the safeguarded solve fails (e.g. no
bubble point in the search bounds).

```rust
pub fn bubble_pressure_with<F>(components: &[crate::thermo::Component], z: &[f64], temperature: f64, k_values: F, opts: SaturationOptions) -> Result<SaturationState, SaturationError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64> { /* ... */ }
```

#### Function `dew_pressure_with`

Dew **pressure** at fixed temperature with a generic K-closure.

Solves `Σ_i z_i / K_i(x, z, T, P) = 1` for `P` \[Pa\], with the incipient
liquid `x_i = z_i / K_i`. The closure is called with the current incipient
liquid `x` and the vapour `y = z`.

Units / ranges / errors mirror [`bubble_pressure_with`]; `incipient` is the
raw incipient liquid `z_i / K_i` \[-\].

```rust
pub fn dew_pressure_with<F>(components: &[crate::thermo::Component], z: &[f64], temperature: f64, k_values: F, opts: SaturationOptions) -> Result<SaturationState, SaturationError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64> { /* ... */ }
```

#### Function `bubble_temperature_with`

Bubble **temperature** at fixed pressure with a generic K-closure.

Solves `Σ_i K_i(z, y, T, P) z_i = 1` for `T` \[K\], with the incipient
vapour `y_i = K_i z_i`.

# Units / ranges

`components.len() == z.len()`; `pressure` `P` \[Pa\] > 0. Returns a
[`SaturationState`] whose `temperature` is the solved bubble temperature \[K\].
Errors mirror [`bubble_pressure_with`] (with `var = "temperature"`).

```rust
pub fn bubble_temperature_with<F>(components: &[crate::thermo::Component], z: &[f64], pressure: f64, k_values: F, opts: SaturationOptions) -> Result<SaturationState, SaturationError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64> { /* ... */ }
```

#### Function `dew_temperature_with`

Dew **temperature** at fixed pressure with a generic K-closure.

Solves `Σ_i z_i / K_i(x, z, T, P) = 1` for `T` \[K\], with the incipient
liquid `x_i = z_i / K_i`. Units / ranges / errors mirror
[`bubble_temperature_with`]; `incipient` is the raw incipient liquid.

```rust
pub fn dew_temperature_with<F>(components: &[crate::thermo::Component], z: &[f64], pressure: f64, k_values: F, opts: SaturationOptions) -> Result<SaturationState, SaturationError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64> { /* ... */ }
```

#### Function `bubble_pressure`

Bubble **pressure** at fixed temperature using a [`PropertyPackageModel`].

Convenience wrapper over [`bubble_pressure_with`] whose K-closure is the
package's [`PropertyPackageModel::k_values`] (Wilson for
[`PropertyPackageModel::Ideal`]; a fugacity-ratio for the cubic packages).
See [`bubble_pressure_with`] for the algorithm, units, and errors.

```rust
pub fn bubble_pressure(components: &[crate::thermo::Component], z: &[f64], temperature: f64, package: crate::thermo::property_package::PropertyPackageModel) -> Result<SaturationState, SaturationError> { /* ... */ }
```

#### Function `dew_pressure`

Dew **pressure** at fixed temperature using a [`PropertyPackageModel`].

Convenience wrapper over [`dew_pressure_with`]. See it for details.

```rust
pub fn dew_pressure(components: &[crate::thermo::Component], z: &[f64], temperature: f64, package: crate::thermo::property_package::PropertyPackageModel) -> Result<SaturationState, SaturationError> { /* ... */ }
```

#### Function `bubble_temperature`

Bubble **temperature** at fixed pressure using a [`PropertyPackageModel`].

Convenience wrapper over [`bubble_temperature_with`]. See it for details.

```rust
pub fn bubble_temperature(components: &[crate::thermo::Component], z: &[f64], pressure: f64, package: crate::thermo::property_package::PropertyPackageModel) -> Result<SaturationState, SaturationError> { /* ... */ }
```

#### Function `dew_temperature`

Dew **temperature** at fixed pressure using a [`PropertyPackageModel`].

Convenience wrapper over [`dew_temperature_with`]. See it for details.

```rust
pub fn dew_temperature(components: &[crate::thermo::Component], z: &[f64], pressure: f64, package: crate::thermo::property_package::PropertyPackageModel) -> Result<SaturationState, SaturationError> { /* ... */ }
```

## Module `sour_water`

**Attributes:**

- `Other("#[forbid(unsafe_code)]")`

Sour-water aqueous ionic-equilibrium speciation (H2S / NH3 / CO2 / H2O) —
DWSIM port. Built on the species / reaction / molality conventions of
[`crate::thermo::electrolyte_svle`] (its [`crate::thermo::electrolyte_svle::SvleSpecies`]
and [`crate::thermo::electrolyte_svle::EquilibriumReaction`] types describe the
chemistry — see [`SourWaterSystem::svle_species`] / [`SourWaterSystem::reaction_set`]).

---

# GPLv3 provenance

Upstream project: **DWSIM** (open-source chemical process simulator),
GPL-3.0, upstream commit `1abf72d`. Copyright 2016 Daniel Wagner O. de
Medeiros. This Rust file is a GPL-3.0 derivative work.

Ported from:

- `DWSIM.Thermodynamics/PropertyPackages/SourWater.vb` — the
  `SourWaterPropertyPackage` (Henry-law volatility correlations for NH3,
  CO2, H2S; the aqueous-ion speciation glue).
- `DWSIM.Thermodynamics/FlashAlgorithms/SourWater.vb` — the sour-water flash
  algorithm. The liquid-phase chemical-equilibrium kernel
  `CalculateEquilibriumConcentrations` (`FlashAlgorithms/SourWater.vb:384-559`)
  is the piece ported here: the eight aqueous acid/base/hydrolysis reactions,
  their mass-action laws, and DWSIM's pH-parametrized charge-balance solve
  (see the Honest-scope note on the solver choice).
- `DWSIM.Thermodynamics/Assets/swreactions.dwrxm` — the embedded reaction
  set: the eight `ln K(T)` correlations (`Expression` fields), evaluated as
  `K = exp(expr(1.8 T))` per
  `DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb:262-304`
  (`EvaluateK`, `KExprType = Expression`).

**Data provenance (equilibrium constants).** The eight `ln K(T)`
correlations are the DWSIM sour-water reaction set (`swreactions.dwrxm`),
which implements the **SWEQ** model of Wilson, Grant M. (1980), *A new
correlation of NH3, CO2, and H2S volatility data from aqueous sour water
systems*, **US EPA Report EPA-600/2-80-067** (public domain — a US
Government work), cited verbatim in `FlashAlgorithms/SourWater.vb:21-24`.
Each correlation is a polynomial in **Rankine** temperature `T_R = 1.8 T`
and returns `K` on a **molality (mol/kg)** basis. These are open, published
constants; no proprietary or restricted data is used.

> **⚠️ Untrusted AI-assisted draft, pending human V&V.** Early-stage
> translation, no human review. Independent OUTRAM PARK fork, **not** the
> official DWSIM. The tests below are **verification** (charge/mass balance,
> closed-form single-acid/base pH, correct pH-vs-loading trend, and the
> SWEQ constants reproducing textbook `pK` values), **not validation**
> against an experimental sour-water VLE database. Not for nuclear facility
> operation, reactor control, safety-critical, licensing, or any operational
> decision (`RESPONSIBLE_USE.md`).

---

# What this computes

Given a **feed** of total dissolved CO2, NH3, H2S (and optionally NaOH) at
molalities \[mol/kg water\] in liquid water, this module solves the coupled
aqueous equilibria for the equilibrium **speciation** — the molality of every
species H⁺, OH⁻, NH3, NH4⁺, CO2, HCO3⁻, CO3²⁻, H2NCOO⁻ (carbamate), H2S,
HS⁻, S²⁻ (and Na⁺) — together with the solution **pH**, ionic strength, and
net charge. The eight reactions (`FlashAlgorithms/SourWater.vb:90-99`):

```text
(1) CO2 ionization      CO2 + H2O <-> H+ + HCO3-      K1 = [H+][HCO3-]/[CO2]
(2) Carbonate           HCO3-     <-> CO3-2 + H+      K2 = [CO3-2][H+]/[HCO3-]
(3) Ammonia ionization  H+ + NH3  <-> NH4+            K3 = [NH4+]/([H+][NH3])
(4) Carbamate           HCO3-+NH3 <-> H2NCOO- + H2O   K4 = [H2NCOO-]/([HCO3-][NH3])
(5) H2S ionization       H2S      <-> HS- + H+        K5 = [HS-][H+]/[H2S]
(6) Sulfide             HS-       <-> S-2 + H+         K6 = [S-2][H+]/[HS-]
(7) Water self-ioniz.   H2O       <-> OH- + H+         Kw = [OH-][H+]
(8) NaOH dissociation   NaOH      <-> OH- + Na+        (assumed complete)
```

Following DWSIM, the **water activity is absorbed into `K`** (it never enters
a mass-action quotient — see `SourWater.vb:457,466,486` where `[H2O]` is
absent), so in the reaction stoichiometry passed to the SVLE solver water has
a coefficient of **0**. Every reaction **conserves charge** (`Σ z·ν = 0`), so
a charge-neutral feed yields a charge-neutral solution by construction.

NaOH (reaction 8) is treated as **fully dissociated** exactly as DWSIM does
(`SourWater.vb:484` `conc("Na+") = conc0("NaOH")`): a NaOH feed is entered as
equal molalities of Na⁺ and OH⁻ (charge-neutral, strong base), so reaction 8
is not carried as a finite-`K` equilibrium.

# Activity-scale convention (matches DWSIM's molality basis)

DWSIM's sour-water kernel works in **molality** (mol/kg) for *every* species,
neutral and ionic alike (`SourWater.vb:274-287`, `conc = Vx / kg`). To make
[`crate::thermo::electrolyte_svle`]'s mixed-scale solver reproduce that, each
dissolved **neutral** reacting species (CO2, NH3, H2S) is registered as a
molality-scale species of **zero charge** (role
[`crate::thermo::electrolyte_svle::SpeciesRole::Ion`] with `z = 0`): its
activity is `m·γ` \[mol/kg\] and it adds nothing to ionic strength or charge.
Water is the mole-fraction-scale solvent that sets the molality mass basis.
With ideal activities (`γ = 1`, DWSIM's own base convention) the reaction
quotient `Q_i = Π m_s^{ν_s}` is then in `(mol/kg)^{Σν}`, matching the units
of the SWEQ molality-basis `K`.

# Units

| Quantity | Unit |
|---|---|
| Temperature `T` | K |
| Feed / species molality `m` | mol/kg (water) |
| Ionic strength `I` | mol/kg |
| Equilibrium constant `K` | (mol/kg)^{Σν} |
| pH, charge number `z` | dimensionless |

# Honest scope — what is and is NOT ported

**Ported and verified here:**
- The eight SWEQ `ln K(T)` correlations (`swreactions.dwrxm`), on the
  molality basis, with `K = exp(expr(1.8 T))`.
- The liquid-phase reaction-set speciation (reactions 1–7 as a coupled
  equilibrium set, plus complete NaOH dissociation) and pH, ionic strength,
  and exact charge/mass balance of the result.

**Solver choice (a measured finding, documented not hidden):** the reaction
set was first expressed as
[`crate::thermo::electrolyte_svle::EquilibriumReaction`]s (see
[`SourWaterSystem::reaction_set`]) and handed to the generic reaction-extent
solver [`crate::thermo::electrolyte_svle::SvleSystem::solve_speciation`]. On
the full stiff sour-water set the extents span ~9 orders of magnitude (water
`ξ ~ 1e-11`, acid dissociation `ξ ~ 1e-4`, second sulfide dissociation
`ξ ~ 1e-14`), and that solver's shared-step damped Newton **did not converge**
— it stalled at a log-residual of `~0.03–0.22` even at 20 000 iterations
(measured 2026-08-03). This is exactly why DWSIM itself does **not** use a
reaction-extent solver here but a **pH-parametrized charge-balance** method.
This port therefore implements DWSIM's own method
(`CalculateEquilibriumConcentrations`): every non-`H⁺` species is written in
closed form from the mass-action laws and the element totals, and `[H⁺]` is
found by robust log-scale bisection of the charge balance — see
[`SourWaterSystem::speciate`]. The electrolyte_svle **types and molality /
activity conventions** are reused to *describe* the chemistry; the stiff
multi-order solve is done by the DWSIM-native pH method.

**Deliberately NOT reproduced** (documented omissions, not silent gaps):
- **DWSIM's empirical ionic-strength / cross-species `K` corrections**
  (`SourWater.vb:454` `k1 = exp(ln K1 − 0.278[H2S] + (−1.32 + 1558.8/T_R)·I^{0.4})`
  and `:473` `k5 = exp(ln K5 + 0.427[CO2])`). These make `K1`,`K5`
  composition-dependent. They are provided as standalone helpers
  ([`ionic_strength_correction_k1`], [`co2_correction_k5`]) and applied by
  the optional outer loop [`SourWaterSystem::speciate_corrected`], but the
  base [`SourWaterSystem::speciate`] uses the **uncorrected** SWEQ `K`
  (DWSIM's own commented-out fallback, `SourWater.vb:455`).
- **The full VLE outer loop** (`Flash_PT_Internal`, `SourWater.vb:182-382`):
  the alternating vapour–liquid `NestedLoops` flash and the NH3/CO2/H2S
  Henry-law volatility that partitions gas between phases. Only the
  **liquid-phase speciation** the loop iterates on is ported; the Henry-law
  volatility correlations are ported for reference as
  [`henry_volatility`] but are not wired into a phase split here.
- **`Flash_PH` / `Flash_PS` / `Flash_TV` / `Flash_PV`** energy/spec outer
  flashes (`SourWater.vb:651-783`) — out of scope for the same reason.

No experimental sour-water database is bundled; every constant here is the
published SWEQ correlation or a textbook `pK` used only as a verification
reference.

```rust
pub mod sour_water { /* ... */ }
```

### Types

#### Enum `Species`

**Attributes:**

- `Repr(AttributeRepr { kind: Rust, align: None, packed: None, int: Some("usize") })`

The eight species of the sour-water system, in the fixed index order used by
every stoichiometry / molality vector in this module.

Water is index 0 (the molality-scale solvent). Indices 1–11 are the reacting
aqueous species. `Na` (index 12) is the spectator strong-base cation.

Dimensionless enum; used only as a stable column index.

```rust
pub enum Species {
    Water = 0,
    HPlus = 1,
    OhMinus = 2,
    Nh3 = 3,
    Nh4Plus = 4,
    Co2 = 5,
    Hco3Minus = 6,
    Co3Minus2 = 7,
    CarbamateMinus = 8,
    H2s = 9,
    HsMinus = 10,
    SMinus2 = 11,
    NaPlus = 12,
}
```

##### Variants

###### `Water`

Water H2O — the mole-fraction-scale solvent (index 0).

Discriminant: `0`

Discriminant value: `0`

###### `HPlus`

Hydrogen ion H⁺ (`z = +1`, index 1).

Discriminant: `1`

Discriminant value: `1`

###### `OhMinus`

Hydroxide ion OH⁻ (`z = -1`, index 2).

Discriminant: `2`

Discriminant value: `2`

###### `Nh3`

Free ammonia NH3 (neutral, molality scale, index 3).

Discriminant: `3`

Discriminant value: `3`

###### `Nh4Plus`

Ammonium ion NH4⁺ (`z = +1`, index 4).

Discriminant: `4`

Discriminant value: `4`

###### `Co2`

Free carbon dioxide CO2(aq) (neutral, molality scale, index 5).

Discriminant: `5`

Discriminant value: `5`

###### `Hco3Minus`

Bicarbonate ion HCO3⁻ (`z = -1`, index 6).

Discriminant: `6`

Discriminant value: `6`

###### `Co3Minus2`

Carbonate ion CO3²⁻ (`z = -2`, index 7).

Discriminant: `7`

Discriminant value: `7`

###### `CarbamateMinus`

Carbamate ion H2NCOO⁻ (`z = -1`, index 8).

Discriminant: `8`

Discriminant value: `8`

###### `H2s`

Free hydrogen sulfide H2S(aq) (neutral, molality scale, index 9).

Discriminant: `9`

Discriminant value: `9`

###### `HsMinus`

Bisulfide ion HS⁻ (`z = -1`, index 10).

Discriminant: `10`

Discriminant value: `10`

###### `SMinus2`

Sulfide ion S²⁻ (`z = -2`, index 11).

Discriminant: `11`

Discriminant value: `11`

###### `NaPlus`

Sodium ion Na⁺ (spectator strong-base cation, `z = +1`, index 12).

Discriminant: `12`

Discriminant value: `12`

##### Implementations

###### Methods

- ```rust
  pub fn index(self: Self) -> usize { /* ... */ }
  ```
  The species' fixed column index \[-\] into the molality/stoichiometry

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Species { /* ... */ }
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
    fn eq(self: &Self, other: &Species) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SourWaterFeed`

Sour-water **feed**: total dissolved amounts per kg of water \[mol/kg\].

"Total" means the sum over all speciated forms of that element group before
equilibrium is imposed — e.g. `co2` is total inorganic carbon fed as CO2,
which the solve redistributes among CO2/HCO3⁻/CO3²⁻/carbamate.

# Units / ranges
All fields are molalities \[mol/kg water\], `>= 0`, finite.

```rust
pub struct SourWaterFeed {
    pub co2: f64,
    pub nh3: f64,
    pub h2s: f64,
    pub naoh: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `co2` | `f64` | Total dissolved CO2 \[mol/kg\]. |
| `nh3` | `f64` | Total dissolved NH3 \[mol/kg\]. |
| `h2s` | `f64` | Total dissolved H2S \[mol/kg\]. |
| `naoh` | `f64` | Total NaOH \[mol/kg\], entered as a fully-dissociated strong base<br>(Na⁺ + OH⁻). `0` for a NaOH-free sour water. |

##### Implementations

###### Methods

- ```rust
  pub fn new(co2: f64, nh3: f64, h2s: f64) -> Self { /* ... */ }
  ```
  A feed of the three acid gases with no caustic (`naoh = 0`).

- ```rust
  pub fn with_naoh(self: Self, naoh: f64) -> Self { /* ... */ }
  ```
  The same feed with a NaOH (caustic) molality \[mol/kg\] added as a

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SourWaterFeed { /* ... */ }
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
    fn eq(self: &Self, other: &SourWaterFeed) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SourWaterResult`

Converged sour-water speciation result.

# Units
- `molality` — equilibrium molality \[mol/kg water\] of every [`Species`],
  indexed by [`Species::index`].
- `ph` — `-log10(m_{H+})` \[-\] (molality basis; see the pH note below).
- `ionic_strength` — `I = ½ Σ_ion z² m` \[mol/kg\].
- `net_charge` — `Σ_ion z·m` \[mol/kg\] (≈ 0 for a neutral feed); equals the
  final charge-balance residual driven to zero by the pH solve.
- `residual` — `|charge-balance residual|` \[mol/kg\] at the converged pH.
- `iterations` — pH-bisection iterations.

# pH basis
DWSIM multiplies `m_{H+}` by `ρ_liq/1000` to approximate a molarity (mol/L)
basis before `-log10` (`PropertyPackages/ElectrolyteBase`/`ElectrolyteProperties`).
This port uses the **molality** basis directly (`ρ/1000 ≈ 1` for dilute
aqueous; the liquid-density model is not ported). Documented simplification.

```rust
pub struct SourWaterResult {
    pub molality: [f64; 13],
    pub ph: f64,
    pub ionic_strength: f64,
    pub net_charge: f64,
    pub residual: f64,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `molality` | `[f64; 13]` | Equilibrium molality \[mol/kg\] per species (index = [`Species::index`]). |
| `ph` | `f64` | Solution pH \[-\] (`-log10(m_{H+})`, molality basis). |
| `ionic_strength` | `f64` | Ionic strength `I` \[mol/kg\]. |
| `net_charge` | `f64` | Net charge molality `Σ z·m` \[mol/kg\]. |
| `residual` | `f64` | `|charge-balance residual|` \[mol/kg\] at the converged pH. |
| `iterations` | `usize` | pH-bisection iterations performed. |

##### Implementations

###### Methods

- ```rust
  pub fn m(self: &Self, s: Species) -> f64 { /* ... */ }
  ```
  Molality \[mol/kg\] of a single [`Species`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SourWaterResult { /* ... */ }
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
    fn eq(self: &Self, other: &SourWaterResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SourWaterSystem`

A sour-water aqueous system at a fixed temperature: the species set, the
SWEQ equilibrium constants at that `T`, and the reaction stoichiometry — the
port of DWSIM's sour-water liquid-phase equilibrium
(`FlashAlgorithms/SourWater.vb:384-559`, `CalculateEquilibriumConcentrations`).

Construct with [`SourWaterSystem::at_temperature`]; solve a feed with
[`SourWaterSystem::speciate`].

```rust
pub struct SourWaterSystem {
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
  pub fn at_temperature(t: f64) -> Self { /* ... */ }
  ```
  Build the sour-water system at temperature `t` \[K\], evaluating all

- ```rust
  pub fn temperature(self: &Self) -> f64 { /* ... */ }
  ```
  System temperature \[K\].

- ```rust
  pub fn constants(self: &Self) -> [f64; 7] { /* ... */ }
  ```
  The seven finite-`K` constants `[K1, K2, K3, K4, K5, K6, Kw]` at the

- ```rust
  pub fn svle_species() -> Vec<SvleSpecies> { /* ... */ }
  ```
  The ordered [`SvleSpecies`] list describing the sour-water phase — a

- ```rust
  pub fn reaction_set(self: &Self) -> Vec<EquilibriumReaction> { /* ... */ }
  ```
  The seven finite-`K` sour-water reactions (1–7) as

- ```rust
  pub fn speciate(self: &Self, feed: &SourWaterFeed) -> Result<SourWaterResult, SourWaterError> { /* ... */ }
  ```
  Solve the sour-water liquid-phase speciation for a feed, using the

- ```rust
  pub fn speciate_corrected(self: &Self, feed: &SourWaterFeed, outer_tol: f64, max_outer: usize) -> Result<SourWaterResult, SourWaterError> { /* ... */ }
  ```
  Solve with DWSIM's empirical **ionic-strength / cross-species `K`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SourWaterSystem { /* ... */ }
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
    fn eq(self: &Self, other: &SourWaterSystem) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SourWaterError`

Error conditions for the sour-water speciation solve.

```rust
pub enum SourWaterError {
    InvalidFeed,
    NonFinite,
    NoBracket,
}
```

##### Variants

###### `InvalidFeed`

A feed molality was negative or non-finite.

###### `NonFinite`

A non-finite value appeared during the solve.

###### `NoBracket`

The pH bracket could not be established (residual same sign at both ends).

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
    fn clone(self: &Self) -> SourWaterError { /* ... */ }
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Eq**
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SourWaterError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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

#### Function `rankine`

**Attributes:**

- `MustUse { reason: None }`
- `Other("#[attr = Inline(Hint)]")`

Convert an absolute temperature `t` \[K\] to the **Rankine** scale
`T_R = 1.8 T` \[°R\] used by the SWEQ `ln K(T)` correlations
(`FlashAlgorithms/SourWater.vb` writes `T * 1.8` throughout).

```rust
pub fn rankine(t: f64) -> f64 { /* ... */ }
```

#### Function `ln_k_co2_ionization`

**Attributes:**

- `MustUse { reason: None }`

Natural log of the **CO2-ionization** equilibrium constant
`K1 = [H+][HCO3-]/[CO2]` \[mol/kg\] (reaction 1), SWEQ correlation
`swreactions.dwrxm` reaction #1 (`Expression`), a polynomial in Rankine
`T_R = 1.8 T`. Valid `T` roughly 273–473 K (SWEQ sour-water range).

```rust
pub fn ln_k_co2_ionization(t: f64) -> f64 { /* ... */ }
```

#### Function `ln_k_carbonate`

**Attributes:**

- `MustUse { reason: None }`

Natural log of the **carbonate-production** constant
`K2 = [CO3-2][H+]/[HCO3-]` \[mol/kg\] (reaction 2), SWEQ reaction #2.

```rust
pub fn ln_k_carbonate(t: f64) -> f64 { /* ... */ }
```

#### Function `ln_k_ammonia_ionization`

**Attributes:**

- `MustUse { reason: None }`

Natural log of the **ammonia-ionization** constant
`K3 = [NH4+]/([H+][NH3])` \[(mol/kg)⁻¹\] (reaction 3), SWEQ reaction #3.

```rust
pub fn ln_k_ammonia_ionization(t: f64) -> f64 { /* ... */ }
```

#### Function `ln_k_carbamate`

**Attributes:**

- `MustUse { reason: None }`

Natural log of the **carbamate-production** constant
`K4 = [H2NCOO-]/([HCO3-][NH3])` \[(mol/kg)⁻¹\] (reaction 4), SWEQ reaction #4.

```rust
pub fn ln_k_carbamate(t: f64) -> f64 { /* ... */ }
```

#### Function `ln_k_h2s_ionization`

**Attributes:**

- `MustUse { reason: None }`

Natural log of the **H2S-ionization** constant `K5 = [HS-][H+]/[H2S]`
\[mol/kg\] (reaction 5), SWEQ reaction #5.

```rust
pub fn ln_k_h2s_ionization(t: f64) -> f64 { /* ... */ }
```

#### Function `ln_k_sulfide`

**Attributes:**

- `MustUse { reason: None }`

Natural log of the **sulfide-production** constant `K6 = [S-2][H+]/[HS-]`
\[mol/kg\] (reaction 6), SWEQ reaction #6.

```rust
pub fn ln_k_sulfide(t: f64) -> f64 { /* ... */ }
```

#### Function `ln_kw`

**Attributes:**

- `MustUse { reason: None }`

Natural log of the **water self-ionization** constant `Kw = [OH-][H+]`
\[(mol/kg)²\] (reaction 7), SWEQ reaction #7.

```rust
pub fn ln_kw(t: f64) -> f64 { /* ... */ }
```

#### Function `equilibrium_constants`

**Attributes:**

- `MustUse { reason: None }`

All seven finite-`K` sour-water constants at temperature `t` \[K\], as `K`
(not `ln K`), in the order `[K1, K2, K3, K4, K5, K6, Kw]`. Each is
`exp(ln K(T))` per DWSIM `EvaluateK` (`ThermodynamicsBase.vb:283`).

```rust
pub fn equilibrium_constants(t: f64) -> [f64; 7] { /* ... */ }
```

#### Function `ionic_strength_correction_k1`

**Attributes:**

- `MustUse { reason: None }`

DWSIM's empirical **ionic-strength + H2S correction** to `K1`
(`FlashAlgorithms/SourWater.vb:454`):

```text
K1' = exp( ln K1 - 0.278 [H2S] + (-1.32 + 1558.8/T_R) I^{0.4} )
```

with `[H2S]` the free-H2S molality \[mol/kg\], `I` the ionic strength
\[mol/kg\], and `T_R = 1.8 T`. Composition-dependent; used only by the
optional outer loop [`SourWaterSystem::speciate_corrected`].

```rust
pub fn ionic_strength_correction_k1(k1: f64, t: f64, h2s_molality: f64, ionic_strength: f64) -> f64 { /* ... */ }
```

#### Function `co2_correction_k5`

**Attributes:**

- `MustUse { reason: None }`

DWSIM's empirical **CO2 correction** to `K5`
(`FlashAlgorithms/SourWater.vb:473`):

```text
K5' = exp( ln K5 + 0.427 [CO2] )
```

with `[CO2]` the free-CO2 molality \[mol/kg\]. Composition-dependent; used
only by [`SourWaterSystem::speciate_corrected`].

```rust
pub fn co2_correction_k5(k5: f64, co2_molality: f64) -> f64 { /* ... */ }
```

#### Function `henry_volatility`

**Attributes:**

- `MustUse { reason: None }`

NH3 / CO2 / H2S **Henry-law volatilities** \[psia per (mol/kg)\] from DWSIM's
sour-water property package (`PropertyPackages/SourWater.vb:118-134`,
`AUX_PVAPi_SW`). Reference implementation only — **not** used by the
speciation solve, which is liquid-phase; documented in Honest scope.

# Arguments (all \[mol/kg\] unless noted)
- `t` — temperature \[K\]
- `cas` — free-NH3 molality (`conc("NH3")`)
- `cc` — total-carbon group `[CO2]+[HCO3-]+[CO3-2]+[H2NCOO-]`
- `cs` — total-sulfide group `[H2S]+[HS-]+[S-2]`

Returns `(v_nh3, v_co2, v_h2s)` \[psia/(mol/kg)\]. To convert to Pa/(mol/kg),
divide by `0.000145038` (DWSIM's `psia→Pa` factor, `SourWater.vb:119`).

```rust
pub fn henry_volatility(t: f64, cas: f64, cc: f64, cs: f64) -> (f64, f64, f64) { /* ... */ }
```

### Constants and Statics

#### Constant `N_SPECIES`

Number of species in the sour-water system (fixed at 13).

```rust
pub const N_SPECIES: usize = 13;
```

#### Constant `LN_K_NAOH`

The SWEQ **NaOH dissociation** `ln K8` (reaction 8), a temperature-independent
constant `15.72` (`swreactions.dwrxm` reaction #8 `Expression`). Provided for
completeness; NaOH is treated as fully dissociated (this large `K` confirms
that limit), so it is not carried as a finite-`K` equilibrium.

```rust
pub const LN_K_NAOH: f64 = 15.72;
```

## Module `stability`

Phase-stability analysis via the **tangent-plane distance** (TPD) criterion —
Michelsen's stability test for robust flash initialisation and single-/two-
phase identification.

Composed on the DWSIM (GPL-3.0) thermo kernel: the fugacity coefficients come
from [`crate::thermo::cubic_eos::CubicEos::ln_phi`] and the trial-phase seeds
from [`crate::thermo::flash::wilson_k_values`]. DWSIM applies exactly this
test inside its `NestedLoops` flash (the `StabTest` phase-stability routine on
`PropertyPackage`) to decide whether a candidate single phase should be split
and to reject the *trivial* flash solution (both phases converging back to the
feed). This module is the Rust analogue of that check.

# The criterion (Michelsen, 1982)

At a feed of overall mole fractions `z_i` \[-\] and a fixed temperature `T`
\[K\] and pressure `P` \[Pa\], the single feed phase is **stable** iff the
tangent-plane distance

```text
tm(w) = Σ_i w_i ( ln w_i + ln φ_i(w) − ln z_i − ln φ_i(z) ) ≥ 0
```

for **every** trial composition `w` (`Σ w_i = 1`). `tm(w)` is the vertical gap
between the molar Gibbs-energy-of-mixing surface at `w` and the tangent
hyperplane drawn at the feed `z`; a `w` with `tm(w) < 0` sits **below** that
tangent plane, i.e. a distinct phase of composition `w` has lower Gibbs energy
than the feed, so the feed is **unstable** (it will split).

# How stationary points are found (Michelsen's modified formulation)

Rather than minimise `tm` over the composition simplex globally, Michelsen
locates its **stationary points** by successive substitution on unnormalised
trial mole numbers `Y_i`. Writing `d_i = ln z_i + ln φ_i(z)`, a stationary
point of the modified function

```text
tm*(Y) = 1 + Σ_i Y_i ( ln Y_i + ln φ_i(Y) − d_i − 1 )
```

satisfies `ln Y_i = d_i − ln φ_i(w)`, with `w_i = Y_i / Σ_k Y_k`. That fixed
point is reached by the iteration

```text
Y_i^(k+1) = exp( d_i − ln φ_i(w^(k)) ),   w^(k) = Y^(k) / Σ_j Y_j^(k).
```

At convergence, with `S = Σ_i Y_i`, the tangent-plane distance at the
stationary composition reduces to `tm(w) = −ln S`: therefore `S > 1` (i.e.
`tm < 0`) flags instability. This module reports the **normalised** `tm(w)`
(via [`tangent_plane_distance`]) at each converged stationary point, which
carries the same sign as `tm*` and `−ln S`.

Two Wilson-based seeds are launched — a **vapour-like** trial `Y_i = z_i K_i`
and a **liquid-like** trial `Y_i = z_i / K_i` — so that a split is detected
whether the incipient phase is lighter or heavier than the feed.

# Which cubic-EOS root feeds each `ln φ`

[`CubicEos::ln_phi`] needs an explicit [`Phase`] to pick the compressibility
root. Stability analysis must use the **thermodynamically correct** root at
each composition, i.e. the one of lower Gibbs energy, not a fixed phase label.
At fixed `(w, T, P)` only the `Σ_i w_i ln φ_i(w)` part of `G/RT` depends on the
root, so the helper selects, for both the feed and every trial, whichever of
the vapour/liquid root minimises `Σ_i w_i ln φ_i(w)` (falling back to the sole
real root when only one exists). This is documented so the omission of a
separate Gibbs-root pre-selection API is explicit.

# Honest scope (verification, not benchmark validation)

- **Two Wilson-seeded successive-substitution trials only.** This is *not* a
  full global minimisation of the TPD surface: successive substitution finds a
  *stationary point* near each seed, not a certified global minimum. A phase
  reported stable here is stable *with respect to the two Wilson trials* — the
  standard, practical Michelsen check DWSIM itself uses, but it can in
  principle miss a stationary point that neither Wilson seed reaches (e.g. some
  near-critical or strongly non-ideal multicomponent cases). Acceleration
  (GDEM/DEM), a third "ideal-mix" seed, and second-order/global TPD
  minimisation are out of scope.
- **VLE fugacity from the cubic EOS only** ([`CubicEos`], `k_ij = 0`,
  geometric-mean mixing). No activity-coefficient / γ-φ stability, no
  three-phase (VLLE) or solid stability.
- The tests below are **verification** against closed-form identities
  (`tm(z) = 0`), single-phase sanity, and internal sign-consistency — **not**
  validation against an experimental or NIST/DECHEMA phase-envelope benchmark.

> **⚠️ Unverified until validated.** AI-assisted port — untrusted draft
> material until human-reviewed per the crate `CLAUDE.md`. Not for nuclear
> facility operation, reactor control, safety-critical, or licensing
> decisions. Independent OUTRAM PARK fork, not the official DWSIM.

# Design (crate `CLAUDE.md`)

The fugacity model is taken as the [`CubicEos`] **enum** (no trait object, no
`dyn`, no `Box`, no lifetimes, no channels). Compositions are owned by value
(`Vec<f64>` / `&[f64]`). Inner arithmetic is documented raw `f64` (SI:
K, Pa, mole fractions \[-\]), matching [`crate::thermo::cubic_eos`].

```rust
pub mod stability { /* ... */ }
```

### Types

#### Struct `StabilityResult`

Outcome of a [`stability_test`].

`tm_min` is the smallest tangent-plane distance \[-\] found over the
non-trivial converged trials (or `0.0` if every trial converged to the trivial
feed solution). `stable` is `true` iff no non-trivial trial dipped below the
tangent plane (`tm_min ≥ −STABILITY_TOL`). When `stable` is `false`,
`trial_composition` carries the destabilising trial composition `w`
(mole fractions \[-\], summing to 1) — a good warm-start for a two-phase
flash; it is `None` when the feed is stable.

```rust
pub struct StabilityResult {
    pub stable: bool,
    pub trial_composition: Option<Vec<f64>>,
    pub tm_min: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stable` | `bool` | `true` iff the single feed phase is stable with respect to both Wilson<br>trials (no non-trivial trial found `tm < −STABILITY_TOL`). |
| `trial_composition` | `Option<Vec<f64>>` | The destabilising trial composition `w` (mole fractions \[-\], sum 1) at<br>the most-negative `tm`, or `None` when the feed is stable. |
| `tm_min` | `f64` | Minimum tangent-plane distance `tm` \[-\] over the non-trivial converged<br>trials; `0.0` if both trials converged to the trivial (feed) solution. |

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
    fn clone(self: &Self) -> StabilityResult { /* ... */ }
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
    fn eq(self: &Self, other: &StabilityResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `tangent_plane_distance`

**Attributes:**

- `MustUse { reason: None }`

Tangent-plane distance `tm(w) = Σ_i w_i ( ln w_i + ln φ_i(w) − ln z_i − ln φ_i(z) )`
\[-\] of a trial composition `w` relative to the feed `z`.

This is the vertical gap between the reduced molar Gibbs-energy-of-mixing
surface at `w` and the tangent hyperplane at `z` (both at the same `T`, `P`).
`tm(w) ≥ 0` for all `w` ⟺ the feed phase is stable; any `tm(w) < 0` means a
distinct phase of composition `w` is more stable than the feed.

# Units / ranges

- `components`, `z`, `w`: equal length; `z`, `w` are mole fractions \[-\] that
  should each sum to 1, with `z_i > 0` (the feed must contain every component,
  since `ln z_i` appears). Components of `w` equal to 0 contribute a zero term
  (`w_i ln w_i → 0`) and are skipped.
- `t` \[K\] > 0, `p` \[Pa\] > 0.
- `eos`: the [`CubicEos`] fugacity model (`k_ij = 0`); both `ln φ(z)` and
  `ln φ(w)` use the Gibbs-selected root (see [`ln_phi_min_gibbs`]).

# Special values

`tangent_plane_distance(.., z, z, ..)` is `0.0` **exactly** — the trial equals
the feed, every bracket cancels term-by-term. This is the trivial solution.

# Returns

The scalar `tm` \[-\]. Dimensionless (it is a Gibbs energy divided by `RT`).

```rust
pub fn tangent_plane_distance(components: &[crate::thermo::Component], z: &[f64], w: &[f64], t: f64, p: f64, eos: crate::thermo::cubic_eos::CubicEos) -> f64 { /* ... */ }
```

#### Function `stability_test`

**Attributes:**

- `MustUse { reason: None }`

Michelsen phase-stability test: is the single feed phase `z` at `(T, P)`
stable, or will it split?

Launches two Wilson-seeded successive-substitution trials — vapour-like
(`Y_i = z_i K_i`) and liquid-like (`Y_i = z_i / K_i`) — to locate stationary
points of the tangent-plane-distance surface. The feed is declared
**unstable** if either trial converges to a **non-trivial** composition whose
tangent-plane distance is negative (`tm < −STABILITY_TOL`): such a `w` is a
composition of lower Gibbs energy than the feed.

# Trivial-solution guard

A trial that drifts back to the feed (`Σ_i (w_i − z_i)² < TRIVIAL_TOL`) is the
*trivial* stationary point `w = z` (`tm = 0` by construction) and is
**discarded** — it carries no information about a phase split. This is exactly
why DWSIM runs the test with two opposed seeds: at least one must leave the
feed for an instability to be found.

# Convergence criteria

Each trial iterates `ln Y_i ← d_i − ln φ_i(w)` (with `w = Y/ΣY`) until the sum
of squared `ln Y` changes drops below [`SS_TOL`] (`1e-12`), for at most
[`MAX_SS_ITER`] (`2000`) iterations; a trial that fails to converge in that
budget is dropped (contributes no stationary point). Successive substitution
can be slow very near a phase boundary or critical point — see the honest
scope in the module header.

# Units / ranges

- `components`, `z`: equal length; `z` mole fractions \[-\] summing to 1 with
  every `z_i > 0`. `t` \[K\] > 0, `p` \[Pa\] > 0. `eos`: the [`CubicEos`]
  fugacity model (`k_ij = 0`).

# Returns

A [`StabilityResult`]: `stable`, the minimum non-trivial `tm` found
(`tm_min`), and — when unstable — the destabilising trial composition to
warm-start a flash.

```rust
pub fn stability_test(components: &[crate::thermo::Component], z: &[f64], t: f64, p: f64, eos: crate::thermo::cubic_eos::CubicEos) -> StabilityResult { /* ... */ }
```

## Module `transport`

Transport-property correlations and phase-mixing rules: viscosity, thermal
conductivity, and surface tension of gas and liquid phases.

# Provenance (DWSIM, GPL-3.0)

Ported from DWSIM's property-package transport methods. Two upstream files
are the source (paths relative to the DWSIM solution root; line numbers are
the commit vendored under this crate's `upstream_source/`):

- **`DWSIM.Thermodynamics/PropertyPackages/PropertyPackage.vb`** — the
  *phase-mixing* routines: `AUX_VAPVISCm` (gas viscosity, `:7340`),
  `AUX_VAPVISCi` (`:7410`), `AUX_LIQVISCi`/`AUX_LIQVISCm` (liquid viscosity,
  `:6883`/`:6971`), `AUX_CONDTG` (gas conductivity, `:7257`), `AUX_CONDTL`
  (liquid conductivity, `:7186`), `AUX_SURFTi`/`AUX_SURFTM` (surface
  tension, `:7157`/`:7049`).
- **`DWSIM.Thermodynamics/PropertyPackages/Models/FluidProperties.vb`** —
  the *pure-component* corresponding-states correlations that those
  routines call: `viscg_lucas` (`:216`), `viscl_letsti` (`:142`),
  `condl_latini` (`:381`), `condtg_elyhanley` (`:437`),
  `viscg_jossi_stiel_thodos` (`:678`), `condlm_li` (`:717`), `sigma_bb`
  (`:120`).

# What is ported vs. deferred (honest scope)

DWSIM's `AUX_*` routines first try a per-compound *tabular / experimental*
correlation (a DIPPR/ChemSep polynomial keyed by an equation number in the
compound database) and fall back to the corresponding-states estimators
only when no experimental curve is present. **This port carries the
corresponding-states estimators and the phase-mixing rules only.** The
tabular-coefficient path, the CoolProp/experimental-database backends, and
the compressed-liquid Lucas pressure correction (`viscl_pcorrection_lucas`)
are *not* ported — a compound here is the constant-property [`Component`],
which has no per-compound transport-coefficient tables.

One deliberate deviation: DWSIM's default **gas-viscosity mixing** is a bare
mole average of the pure Lucas viscosities ([`gas_viscosity_mole_average`],
matching `AUX_VAPVISCm`). The task this module answers also asks for the
classical **Wilke** rule, which DWSIM does *not* use for viscosity;
[`gas_viscosity_wilke`] provides it, cited to Poling et al. rather than to
DWSIM. Both are offered so a caller can pick.

# Units

Public boundaries are `uom`-typed: [`DynamicViscosity`] (Pa·s),
[`ThermalConductivity`] (W/(m·K)), [`SurfaceTension`] (N/m),
[`ThermodynamicTemperature`] (K). Pure-component functions read the raw-`f64`
SI constants off a [`Component`] (Tc [K], Pc [Pa], ω [-], M [kg/mol],
Tb [K], Vc [m³/mol]). Phase-mixing functions take raw-`f64` slices — mole or
mass fractions [-] and the *per-component* transport values already in the
SI base unit of the returned quantity (Pa·s, W/(m·K), or N/m) — because a
slice of `uom` quantities is awkward at a call site and these feed tight
summation loops (the crate `CLAUDE.md` "raw f64 in inner loops" convention).

# ⚠️ Unverified until validated

Early-stage translation. The tests below are **verification** (the code
reproduces the cited DWSIM correlation and matches published pure-component
data points within each correlation's stated accuracy), **not** a benchmark
validation of the whole property package. Not for nuclear facility
operation, reactor control, safety-critical or licensing decisions.
Independent OUTRAM PARK fork, not the official DWSIM.

```rust
pub mod transport { /* ... */ }
```

### Types

#### Enum `LiquidViscosityMixingRule`

Liquid-phase viscosity mixing rule selector — the four rules DWSIM offers in
`AUX_LIQVISCm` (`PropertyPackage.vb:7016`), dispatched by enum (no trait
object, per the workspace design rules).

```rust
pub enum LiquidViscosityMixingRule {
    MoleAverage,
    LogMoleAverage,
    InvertedMassAverage,
    InvertedLogMassAverage,
}
```

##### Variants

###### `MoleAverage`

`η = Σ xᵢ ηᵢ` (mole-fraction linear average).

###### `LogMoleAverage`

`η = exp(Σ xᵢ ln ηᵢ)` (mole-fraction log average).

###### `InvertedMassAverage`

`η = 1 / Σ (wᵢ / ηᵢ)` (mass-fraction inverse average).

###### `InvertedLogMassAverage`

`η = exp(1 / Σ (wᵢ / ln ηᵢ))` (mass-fraction inverse-log average).

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
    fn clone(self: &Self) -> LiquidViscosityMixingRule { /* ... */ }
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
    fn eq(self: &Self, other: &LiquidViscosityMixingRule) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `LatiniFluidType`

Liquid-hydrocarbon-family selector for the Latini correlation
(`FluidProperties.vb:381`, the `Tipo` argument). Governs the four fit
constants `A*, α, β, γ`.

```rust
pub enum LatiniFluidType {
    SaturatedHydrocarbon,
    Olefin,
    Cycloparaffin,
    Aromatic,
    Other,
}
```

##### Variants

###### `SaturatedHydrocarbon`

Saturated hydrocarbons (DWSIM default; DWSIM always passes `""` → this).

###### `Olefin`

Olefins.

###### `Cycloparaffin`

Cycloparaffins.

###### `Aromatic`

Aromatics.

###### `Other`

Other (e.g. water) — DWSIM's `"X"` branch.

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
    fn clone(self: &Self) -> LatiniFluidType { /* ... */ }
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
    fn eq(self: &Self, other: &LatiniFluidType) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `gas_viscosity_lucas`

Low-pressure (dilute) gas viscosity of a pure component by the **Lucas
corresponding-states** method.

Physical quantity: dynamic viscosity `η` of the vapour at temperature `T`
and (implicitly) low pressure, where the pressure/density correction is
negligible. Ported from DWSIM `FluidProperties.vb:216` `viscg_lucas`:
```text
ξ  = 0.176 (Tc / (M³ Pc⁴))^(1/6)          [Tc in K, M in g/mol, Pc in bar]
ηξ = 0.807 Tr^0.618 − 0.357 e^(−0.449 Tr) + 0.34 e^(−4.058 Tr) + 0.018
η  = (ηξ / ξ) × 10⁻⁷ Pa·s                  [ηξ/ξ is in micropoise]
```
`Tc`, `Pc`, `M` are read from `component`; `Tr = T/Tc`.

Valid range: dilute gas, `Tr` roughly 0.3–15; stated accuracy of the Lucas
low-pressure method is ~1–3 % for non-polar gases (Poling, Prausnitz &
O'Connell, *The Properties of Gases and Liquids*, 5th ed., §9-4).

```rust
pub fn gas_viscosity_lucas(component: &crate::thermo::Component, temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::DynamicViscosity { /* ... */ }
```

#### Function `gas_viscosity_mole_average`

Molar-average gas-phase viscosity — **DWSIM's default** vapour-viscosity
mixing rule.

Ported from the first step of DWSIM `PropertyPackage.vb:7391` `AUX_VAPVISCm`,
which forms `η_mix = Σ xᵢ ηᵢ` over the pure-component Lucas viscosities
before (optionally) applying the Jossi-Stiel-Thodos density correction
(see [`gas_viscosity_jossi_stiel_thodos`]).

- `mole_fractions` — vapour mole fractions `xᵢ` [-].
- `pure_viscosities` — per-component viscosities `ηᵢ` [Pa·s] (e.g. from
  [`gas_viscosity_lucas`]).

Slices must be the same length. Returns `η_mix` [Pa·s].

```rust
pub fn gas_viscosity_mole_average(mole_fractions: &[f64], pure_viscosities: &[f64]) -> uom::si::f64::DynamicViscosity { /* ... */ }
```

#### Function `gas_viscosity_wilke`

Gas-phase viscosity by the **Wilke** kinetic-theory mixing rule.

Physical quantity: dynamic viscosity of a low-pressure gas mixture from the
pure-component viscosities and molar masses:
```text
η_mix = Σᵢ  xᵢ ηᵢ / (Σⱼ xⱼ φᵢⱼ)
φᵢⱼ   = [1 + (ηᵢ/ηⱼ)^(1/2) (Mⱼ/Mᵢ)^(1/4)]² / sqrt(8 (1 + Mᵢ/Mⱼ))
```
with `φᵢᵢ = 1`. Reference: Wilke, *J. Chem. Phys.* **18**, 517 (1950); as
given in Poling et al. (2001), eq. 9-5.13/9-5.14.

**Not a DWSIM routine** — DWSIM mixes vapour viscosity by the bare mole
average ([`gas_viscosity_mole_average`]). Wilke is provided as the classical
rigorous low-pressure rule; it reduces exactly to the pure value for a single
component and always lies between the pure-component viscosities for a binary.

- `mole_fractions` — `xᵢ` [-].
- `pure_viscosities` — `ηᵢ` [Pa·s].
- `molar_masses` — `Mᵢ` [any consistent unit; only ratios enter].

All three slices must share one length.

```rust
pub fn gas_viscosity_wilke(mole_fractions: &[f64], pure_viscosities: &[f64], molar_masses: &[f64]) -> uom::si::f64::DynamicViscosity { /* ... */ }
```

#### Function `gas_viscosity_jossi_stiel_thodos`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Dense-gas (high-pressure) viscosity correction by the **Jossi-Stiel-Thodos**
residual-viscosity method — the second step of DWSIM `AUX_VAPVISCm`.

Ported from DWSIM `FluidProperties.vb:678` `viscg_jossi_stiel_thodos`:
```text
ρr  = Vc / V                              [reduced density]
ξ   = (Tc / (M³ Pc⁴))^(1/6)               [Tc in K, M in g/mol, Pc in atm]
[(η − η₀) ξ + 1]  →  1.023 + 0.23364 ρr + 0.58533 ρr² − 0.40758 ρr³ + 0.093324 ρr⁴
```
solved for `η` (all with the internal unit scalings DWSIM applies). `η₀` is
the low-pressure mixture viscosity (e.g. from [`gas_viscosity_mole_average`]).

- `low_pressure_viscosity` — `η₀` [Pa·s].
- `temperature` — `T`.
- `molar_volume` — `V` [m³/mol] of the gas at state.
- `critical_volume` — `Vc` [m³/mol] (mixture) — must share `V`'s unit.
- `critical_temperature` — `Tc` [K] (mixture).
- `critical_pressure` — `Pc` [Pa] (mixture).
- `molar_mass` — `M` [g/mol] (mixture).

Returns the density-corrected viscosity [Pa·s]. Note the correlation is a
residual fit and does **not** collapse exactly to `η₀` as `ρr → 0` (it
leaves a small `(1.023⁴ − 1)/ξ` residue); it is intended for `ρr` above
roughly 0.1.

```rust
pub fn gas_viscosity_jossi_stiel_thodos(low_pressure_viscosity: uom::si::f64::DynamicViscosity, temperature: uom::si::f64::ThermodynamicTemperature, molar_volume: f64, critical_volume: f64, critical_temperature: f64, critical_pressure: f64, molar_mass_g: f64) -> uom::si::f64::DynamicViscosity { /* ... */ }
```

#### Function `liquid_viscosity_letsou_stiel`

Pure-component saturated-liquid viscosity by the **Letsou-Stiel**
corresponding-states method.

Physical quantity: dynamic viscosity `η_L` of the saturated liquid at `T`.
Ported from DWSIM `FluidProperties.vb:142` `viscl_letsti`:
```text
ξ  = 0.176 (Tc / (M³ Pc⁴))^(1/6)          [Tc in K, M in g/mol, Pc in bar]
η⁰ = (2.648 − 3.725 Tr + 1.309 Tr²) × 10⁻³
η¹ = (7.425 − 13.39 Tr + 5.933 Tr²) × 10⁻³
η  = (η⁰ + ω η¹) / ξ / 1000  Pa·s
```
`Tc`, `Pc`, `ω`, `M` from `component`; `Tr = T/Tc`.

Valid range: reduced temperature roughly **0.76 ≤ Tr ≤ 0.98** (a
near-critical-liquid corresponding-states correlation); typical accuracy
~15 % in-range, degrading rapidly below Tr ≈ 0.7 (Poling et al., §9-11).

```rust
pub fn liquid_viscosity_letsou_stiel(component: &crate::thermo::Component, temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::DynamicViscosity { /* ... */ }
```

#### Function `liquid_viscosity_mixture`

Liquid-phase mixture viscosity by the selected [`LiquidViscosityMixingRule`].

Ported from DWSIM `PropertyPackage.vb:6971` `AUX_LIQVISCm` (the mixing-rule
`Select Case`, `:7016`–`:7037`). Individual `ηᵢ` come from e.g.
[`liquid_viscosity_letsou_stiel`].

- `mole_fractions` — `xᵢ` [-] (used by the mole-average rules).
- `mass_fractions` — `wᵢ` [-] (used by the inverted mass-average rules).
- `pure_viscosities` — `ηᵢ` [Pa·s].

All three slices must share one length. Returns `η_mix` [Pa·s]. For a single
component every rule collapses to that component's viscosity.

```rust
pub fn liquid_viscosity_mixture(rule: LiquidViscosityMixingRule, mole_fractions: &[f64], mass_fractions: &[f64], pure_viscosities: &[f64]) -> uom::si::f64::DynamicViscosity { /* ... */ }
```

#### Function `liquid_thermal_conductivity_latini`

Pure-component saturated-liquid thermal conductivity by the **Latini**
method.

Physical quantity: liquid thermal conductivity `λ_L` [W/(m·K)] at `T`.
Ported from DWSIM `FluidProperties.vb:381` `condl_latini`:
```text
A = A* Tb^α / (M^β Tc^γ)
λ = A (1 − Tr)^0.38 / Tr^(1/6)            (λ = 0 if Tr > 0.98)
```
with `(A*, α, β, γ)` set by [`LatiniFluidType`]. `Tb`, `Tc`, `M` from
`component`; `Tr = T/Tc`.

**Fidelity note:** DWSIM's `AUX_CONDTL` always calls this with an empty
`Tipo` (→ [`LatiniFluidType::SaturatedHydrocarbon`]) regardless of the actual
compound family; the other families are exposed here for completeness.

Valid range: `Tr < 0.98`; typical accuracy ~10 % for the intended families
(Poling et al., §10-9).

```rust
pub fn liquid_thermal_conductivity_latini(component: &crate::thermo::Component, temperature: uom::si::f64::ThermodynamicTemperature, fluid_type: LatiniFluidType) -> uom::si::f64::ThermalConductivity { /* ... */ }
```

#### Function `liquid_thermal_conductivity_li`

Liquid-phase mixture thermal conductivity by the **Li** method.

Ported from DWSIM `FluidProperties.vb:717` `condlm_li` (called by
`AUX_CONDTL`, `PropertyPackage.vb:7245`):
```text
φᵢ  = xᵢ Vcᵢ / Σⱼ xⱼ Vcⱼ                   [critical-volume fractions]
λᵢⱼ = 2 (1/λᵢ + 1/λⱼ)⁻¹                    [harmonic mean]
λ_L = Σᵢ Σⱼ φᵢ φⱼ λᵢⱼ
```

- `critical_volumes` — `Vcᵢ` [any consistent unit; only ratios enter].
- `pure_conductivities` — `λᵢ` [W/(m·K)] (e.g. from
  [`liquid_thermal_conductivity_latini`]).
- `mole_fractions` — `xᵢ` [-].

All three slices must share one length. Returns `λ_L` [W/(m·K)]; collapses to
`λ₁` for a single component.

```rust
pub fn liquid_thermal_conductivity_li(critical_volumes: &[f64], pure_conductivities: &[f64], mole_fractions: &[f64]) -> uom::si::f64::ThermalConductivity { /* ... */ }
```

#### Function `gas_thermal_conductivity_ely_hanley`

Pure-component low-pressure vapour thermal conductivity by the **Ely-Hanley**
extended-corresponding-states method (methane reference fluid).

Physical quantity: vapour thermal conductivity `λ_V` [W/(m·K)] at `T`.
Ported from DWSIM `FluidProperties.vb:437` `condtg_elyhanley`:
```text
λ_V = λ* + (1000 η* / M) 1.32 (Cv − 3R/2)
```
where `λ*`, `η*` come from the methane reference fluid mapped through the
shape factors `f`, `h`, `θ`, `φ` and the Hanley `η₀(T₀)` polynomial (nine
`Cₙ T₀^((n−4)/3)` terms). See the upstream source for the full shape-factor
expressions, reproduced verbatim here.

Inputs (DWSIM `AUX_VAPTHERMCONDi`, `PropertyPackage.vb:7333`, supplies these):
- `component` — supplies `Tc` [K], `Vc` [m³/mol], `ω` [-], `M` [kg/mol].
- `critical_compressibility` — `Zc` [-]. **Not on [`Component`]** — pass
  explicitly (a reasonable estimate is `Zc = 0.291 − 0.08 ω`, DWSIM `Zc1`).
- `isochoric_heat_capacity` — `Cv` [J/(mol·K)], the ideal-gas
  constant-volume molar heat capacity (DWSIM uses `Cp·M − R`).

Valid range: low-pressure non-polar vapour. **This routine is a faithful
translation whose numeric output is verified against the DWSIM formula
(see the test), not a tight benchmark validation against experiment.**

```rust
pub fn gas_thermal_conductivity_ely_hanley(component: &crate::thermo::Component, temperature: uom::si::f64::ThermodynamicTemperature, critical_compressibility: f64, isochoric_heat_capacity: uom::si::f64::MolarHeatCapacity) -> uom::si::f64::ThermalConductivity { /* ... */ }
```

#### Function `gas_thermal_conductivity_mole_average`

Molar-average vapour thermal conductivity — DWSIM's gas-conductivity mixing
rule.

Ported from DWSIM `PropertyPackage.vb:7257` `AUX_CONDTG`, which forms
`λ_mix = Σ xᵢ λᵢ` over the pure-component (Ely-Hanley) conductivities.

- `mole_fractions` — vapour mole fractions `xᵢ` [-].
- `pure_conductivities` — `λᵢ` [W/(m·K)].

Slices must share one length. Returns `λ_mix` [W/(m·K)].

```rust
pub fn gas_thermal_conductivity_mole_average(mole_fractions: &[f64], pure_conductivities: &[f64]) -> uom::si::f64::ThermalConductivity { /* ... */ }
```

#### Function `surface_tension_brock_bird`

Pure-component liquid surface tension by the **Brock-Bird** corresponding-
states method.

Physical quantity: surface tension `σ` [N/m] of the saturated liquid at `T`.
Ported from DWSIM `FluidProperties.vb:120` `sigma_bb`:
```text
Q   = 0.1196 [1 + Tbr ln(Pc/1.01325) / (1 − Tbr)] − 0.279   [Pc in bar]
σ   = Pc^(2/3) Tc^(1/3) Q (1 − Tr)^(11/9) / 1000  N/m
```
`Tc`, `Pc`, `Tb` from `component`; `Tr = T/Tc`, `Tbr = Tb/Tc`. If the normal
boiling point is unknown (`Tb ≤ 0` / non-finite) DWSIM substitutes
`Tb = 0.7 Tc`. Returns `0` for `T ≥ Tc` (the interface has vanished), so `σ`
goes to zero as `T → Tc`.

Valid range: `Tr < 1`; typical accuracy ~5 % for non-polar/slightly-polar
liquids (Poling et al., §12-3).

```rust
pub fn surface_tension_brock_bird(component: &crate::thermo::Component, temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::SurfaceTension { /* ... */ }
```

#### Function `surface_tension_mixture`

Liquid-phase mixture surface tension by DWSIM's **molar-average** rule.

Ported from DWSIM `PropertyPackage.vb:7049` `AUX_SURFTM`:
```text
σ_mix = Σᵢ xᵢ σᵢ / ftotal        (ftotal starts at 1; a supercritical
                                   component contributes σᵢ = 0 and removes
                                   its xᵢ from ftotal — renormalisation)
```
The per-component tensions `σᵢ` are the Brock-Bird values
([`surface_tension_brock_bird`]).

- `mole_fractions` — `xᵢ` [-].
- `pure_tensions` — `σᵢ` [N/m] (caller passes the sub-critical value; the
  supercritical flag drives the renormalisation, matching DWSIM).
- `subcritical` — `true` where `T < Tcᵢ`; a `false` entry contributes `0`
  and is dropped from the mole-fraction normaliser.

All three slices must share one length. Returns `σ_mix` [N/m]. For an
all-subcritical mixture this is the plain molar average `Σ xᵢ σᵢ`.

**Quirk preserved:** DWSIM decrements `ftotal` *inside* the accumulation
loop, so when some component is supercritical the result is mildly
order-dependent; this port reproduces that behaviour verbatim rather than
"fixing" it.

```rust
pub fn surface_tension_mixture(mole_fractions: &[f64], pure_tensions: &[f64], subcritical: &[bool]) -> uom::si::f64::SurfaceTension { /* ... */ }
```

## Module `unifac`

Classic (original) UNIFAC group-contribution activity-coefficient model.

Computes liquid-phase activity coefficients `γ_i` for a mixture from the
*group* composition of each molecule (the Fredenslund "solution-of-groups"
idea): a molecule is a bag of UNIFAC subgroups, and every subgroup carries a
van-der-Waals volume `R_k` and surface area `Q_k`; each *main* group pair
carries an interaction energy `a_mn` (units of K) entering
`Ψ_mn = exp(−a_mn / T)`.

# Port provenance

Ported from DWSIM (GPL-3.0):
`DWSIM.Thermodynamics/PropertyPackages/Models/UNIFAC.vb`, class `Unifac`.
The algebra mirrored here:

- molecular `r_i = Σ_k ν_k^i R_k` — `RET_Ri`, `UNIFAC.vb:378-387`;
- molecular `q_i = Σ_k ν_k^i Q_k` — `RET_Qi`, `UNIFAC.vb:389-398`;
- group area fraction `e_ki = ν_k^i Q_k / q_i` — `RET_EKI`, `UNIFAC.vb:400-409`;
- `τ_mk = exp(−a_mk / T)` on *main* groups — `TAU`, `UNIFAC.vb:357-376`;
- combinatorial `ln γ_i^C` — `GAMMA_MR`, `UNIFAC.vb:283`;
- residual `ln γ_i^R` — `GAMMA_MR`, `UNIFAC.vb:286-293`;
- `γ_i = exp(ln γ_i^C + ln γ_i^R)` — `GAMMA_MR`, `UNIFAC.vb:294`.

DWSIM's `GAMMA_MR` writes the residual in the compact Smith–Van-Ness
`J/L/β/θ/s` form (`UNIFAC.vb:210-297`). This port instead writes the
*algebraically identical* classic Fredenslund form with explicit group
residual activities `ln Γ_k` (see [`group_ln_gamma`]), because the task
specifies that form and it is easier for a human to read against the
textbook equations. The two forms were checked to agree to 1e-12 on the
ethanol/water cases in the tests below.

# Parameter-table provenance (public-literature subset)

The bundled table [`UnifacParameters::original_vle_subset`] is a **small
public-literature subset**, not DWSIM's full asset files. Sources:

- `R_k`, `Q_k` and subgroup→main-group assignments: Hansen, Rasmussen,
  Fredenslund, Schiller & Gmehling, *Ind. Eng. Chem. Res.* **30**, 2352
  (1991), "Vapor-liquid equilibria by UNIFAC group contribution. 5.
  Revision and extension"; identical values appear in DWSIM's
  `DWSIM.Thermodynamics/Assets/unifac.txt`.
- `a_mn` main-group interaction energies (K): same Hansen et al. (1991)
  revised VLE table; identical values appear in DWSIM's
  `DWSIM.Thermodynamics/Assets/unifac_ip.txt`.
- Original model definition: Fredenslund, Jones & Prausnitz, *AIChE J.*
  **21**, 1086 (1975).

These tables are published, openly-cited literature data and are permitted
under the workspace `DATA_POLICY.md`. The subset covers only alkane, alcohol
(OH / CH3OH) and water groups — enough for the alkane/alcohol/water examples
and tests here.

**Deferred data acquisition:** porting the *full* UNIFAC subgroup and
`a_mn` matrix (DWSIM's complete `unifac.txt` / `unifac_ip.txt`, ~50 main
groups) is deliberately out of scope for this change and tracked as a
separate data-acquisition step.

# Honest scope — verification, not benchmark validation

This module has been **verified** (the code reproduces independently
hand-/script-computed numbers from published parameters, and satisfies the
model's exact identities — see the test docs) but **not validated** against
experimental phase-equilibrium benchmarks. UNIFAC is itself an
approximation; aqueous-alcohol `γ∞` in particular is a known weak spot where
the model over-predicts. Treat outputs as unverified draft physics.

Explicitly **excluded** from this port:
- Modified UNIFAC (Dortmund) and UNIFAC (NIST) — different combinatorial
  term and temperature-polynomial `a_mn(T)`; not implemented.
- Temperature-dependent interaction parameters `a_mn(T) = a⁰ + a¹T + a²T²`;
  only the constant `a_mn` of the original model is used.
- Liquid–liquid (LLE) parameter set (DWSIM's `UnifacLL` / `unifac_ll_ip.txt`).
- The full parameter matrix (see "Deferred data acquisition" above).
- Excess-enthalpy / heat-capacity derivatives (`HEX_MIX`, `CPEX_MIX` in the
  upstream file).

# Units

All public functions take/return raw `f64` in the DWSIM-internal SI
convention (temperature in **K**, mole fractions dimensionless in `[0, 1]`
summing to 1). Activity coefficients `γ_i` are dimensionless and `> 0`. This
follows the crate `CLAUDE.md` rule of raw documented `f64` in inner
thermodynamic loops.

```rust
pub mod unifac { /* ... */ }
```

### Types

#### Struct `UnifacSubgroup`

One UNIFAC subgroup's constant parameters.

A subgroup (e.g. `CH3`, `OH`, `H2O`) belongs to a *main* group; interaction
energies `a_mn` are indexed by **main** group, while volume/area are per
**subgroup**. This mirrors DWSIM's `UnifacGroup` (`UNIFAC.vb:635-710`).

```rust
pub struct UnifacSubgroup {
    pub subgroup_id: usize,
    pub main_group_id: usize,
    pub r: f64,
    pub q: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `subgroup_id` | `usize` | Subgroup id (DWSIM `Secondary_Group`, the `SUB_ID` column of<br>`unifac.txt`). Dimensionless integer key. |
| `main_group_id` | `usize` | Main-group id this subgroup belongs to (DWSIM `PrimaryGroup`, the `ID`<br>column). Interaction parameters are looked up on this id. |
| `r` | `f64` | Van-der-Waals group volume `R_k` (dimensionless, relative to a CH2<br>reference). Valid range roughly `0.2 … 3`. Bondi-derived. |
| `q` | `f64` | Van-der-Waals group surface area `Q_k` (dimensionless). Valid range<br>roughly `0 … 3`. Bondi-derived. |

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
    fn clone(self: &Self) -> UnifacSubgroup { /* ... */ }
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
    fn eq(self: &Self, other: &UnifacSubgroup) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `UnifacComponent`

A molecule expressed as its UNIFAC group counts `ν_k` (the caller-supplied
molecular structure).

`groups` is a list of `(subgroup_id, count)` pairs, e.g. ethanol is
`[(1, 1.0), (2, 1.0), (15, 1.0)]` = one CH3, one CH2, one OH. Counts are
`f64` (they may be fractional for pseudo-components) and must be `≥ 0`.

```rust
pub struct UnifacComponent {
    pub groups: Vec<(usize, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `groups` | `Vec<(usize, f64)>` | `(subgroup_id, ν_k)` pairs. A subgroup id must exist in the<br>[`UnifacParameters`] table used with this component. |

##### Implementations

###### Methods

- ```rust
  pub fn new(groups: Vec<(usize, f64)>) -> Self { /* ... */ }
  ```
  Construct from `(subgroup_id, count)` pairs.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> UnifacComponent { /* ... */ }
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
    fn eq(self: &Self, other: &UnifacComponent) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `UnifacParameters`

A UNIFAC parameter table: subgroup volume/area parameters plus the
main-group interaction matrix `a_mn` (K).

Owned by value (`HashMap`s, no lifetimes, no `dyn`), indexed by integer
subgroup / main-group ids, per the workspace design rules.

```rust
pub struct UnifacParameters {
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
  Build an empty table (no subgroups, no interactions).

- ```rust
  pub fn add_subgroup(self: &mut Self, subgroup: UnifacSubgroup) { /* ... */ }
  ```
  Register a subgroup's `R_k` / `Q_k` and its main-group assignment.

- ```rust
  pub fn set_interaction(self: &mut Self, main_m: usize, main_n: usize, a_mn: f64) { /* ... */ }
  ```
  Set the directional main-group interaction `a_mn` (K). Note `a_mn ≠ a_nm`

- ```rust
  pub fn subgroup(self: &Self, subgroup_id: usize) -> Option<&UnifacSubgroup> { /* ... */ }
  ```
  Look up a subgroup by id.

- ```rust
  pub fn interaction(self: &Self, main_m: usize, main_n: usize) -> f64 { /* ... */ }
  ```
  Directional main-group interaction `a_mn` (K); `0.0` if the pair is not

- ```rust
  pub fn psi(self: &Self, main_m: usize, main_n: usize, temperature: f64) -> f64 { /* ... */ }
  ```
  Group interaction factor `Ψ_mn = exp(−a_mn / T)` (dimensionless), with

- ```rust
  pub fn original_vle_subset() -> Self { /* ... */ }
  ```
  Public-literature subset of the **original (VLE) UNIFAC** table.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> UnifacParameters { /* ... */ }
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
    fn eq(self: &Self, other: &UnifacParameters) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `UnifacTable`

Which UNIFAC parameter table to use — an enum dispatch point (no `dyn`) so
future variants (Dortmund, LLE, the full matrix) slot in as new arms.

```rust
pub enum UnifacTable {
    OriginalVle,
}
```

##### Variants

###### `OriginalVle`

Original (VLE) UNIFAC, public-literature subset
([`UnifacParameters::original_vle_subset`]).

##### Implementations

###### Methods

- ```rust
  pub fn parameters(self: Self) -> UnifacParameters { /* ... */ }
  ```
  Materialise the chosen table's parameters.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> UnifacTable { /* ... */ }
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
    fn eq(self: &Self, other: &UnifacTable) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `molecular_r_q`

Molecular volume `r_i = Σ_k ν_k^i R_k` and surface area `q_i = Σ_k ν_k^i Q_k`
(both dimensionless) for one component.

Ported from `RET_Ri` / `RET_Qi` (`UNIFAC.vb:378-398`). Panics if a subgroup
id in `component` is missing from `params` — the caller is expected to build
components against the same table.

```rust
pub fn molecular_r_q(params: &UnifacParameters, component: &UnifacComponent) -> (f64, f64) { /* ... */ }
```

#### Function `ln_gamma_combinatorial`

Combinatorial part `ln γ_i^C` for every component (entropic, size/shape
term; temperature-independent).

Classic Staverman–Guggenheim form, algebraically identical to DWSIM's
`GAMMA_MR` combinatorial line (`UNIFAC.vb:283`):

`ln γ_i^C = ln(φ_i/x_i) + (z/2) q_i ln(θ_i/φ_i) + l_i − (φ_i/x_i) Σ_j x_j l_j`

with `φ_i = x_i r_i / Σ x_j r_j`, `θ_i = x_i q_i / Σ x_j q_j`,
`l_i = (z/2)(r_i − q_i) − (r_i − 1)`, `z = 10`. The `φ_i/x_i` and `θ_i/φ_i`
ratios are formed without dividing by `x_i`, so `x_i = 0` (infinite dilution)
is finite.

`x` are mole fractions (dimensionless, sum ≈ 1); returns one `ln γ_i^C` per
component.

```rust
pub fn ln_gamma_combinatorial(params: &UnifacParameters, components: &[UnifacComponent], x: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `group_ln_gamma`

Residual group-activity `ln Γ_k` for every subgroup present in `group_counts`.

The Fredenslund group-solution equation:

`ln Γ_k = Q_k [ 1 − ln(Σ_m θ_m Ψ_mk) − Σ_m ( θ_m Ψ_km / Σ_n θ_n Ψ_nm ) ]`

with group area fraction `θ_m = Q_m X_m / Σ_n Q_n X_n` and group mole
fraction `X_m`. `Ψ` is on main groups. Returned map is keyed by subgroup id;
`temperature` is in K.

This is the classic-form equivalent of DWSIM's compact `β/θ/s` block
(`UNIFAC.vb:286-293`), used for both the mixture and the pure-component
reference in [`ln_gamma_residual`].

```rust
pub fn group_ln_gamma(params: &UnifacParameters, group_counts: &std::collections::HashMap<usize, f64>, temperature: f64) -> std::collections::HashMap<usize, f64> { /* ... */ }
```

#### Function `ln_gamma_residual`

Residual part `ln γ_i^R = Σ_k ν_k^i (ln Γ_k − ln Γ_k^i)` for every component
(enthalpic, energetic-interaction term; temperature-dependent).

`ln Γ_k` is the group activity in the actual mixture and `ln Γ_k^i` the
group activity in a fluid of pure component `i` (its reference state), both
from [`group_ln_gamma`]. Ported from `GAMMA_MR` (`UNIFAC.vb:286-293`).
`x` are mole fractions; `temperature` is in K.

```rust
pub fn ln_gamma_residual(params: &UnifacParameters, components: &[UnifacComponent], x: &[f64], temperature: f64) -> Vec<f64> { /* ... */ }
```

#### Function `activity_coefficients`

Liquid-phase activity coefficients `γ_i = exp(ln γ_i^C + ln γ_i^R)` for every
component (dimensionless, `> 0`).

Top-level entry point, ported from `GAMMA_MR` (`UNIFAC.vb:59-311`). Inputs:
- `params` — the parameter table (e.g. [`UnifacTable::OriginalVle`]);
- `components` — each molecule's group counts `ν_k^i`;
- `x` — mole fractions (dimensionless, should sum to ≈ 1), same order/length
  as `components`;
- `temperature` — in K (original UNIFAC is calibrated ~275–425 K).

A pure component (`x = [1.0]`) returns `γ = 1` exactly.

```rust
pub fn activity_coefficients(params: &UnifacParameters, components: &[UnifacComponent], x: &[f64], temperature: f64) -> Vec<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `COORDINATION_NUMBER`

Coordination number `z` of the UNIFAC/UNIQUAC lattice (dimensionless).
Fixed at 10 in the original model (Fredenslund et al. 1975); enters the
Staverman–Guggenheim combinatorial correction as `z/2 = 5`.

```rust
pub const COORDINATION_NUMBER: f64 = 10.0;
```

## Module `unifac_dortmund`

Modified UNIFAC (Dortmund) group-contribution activity-coefficient model.

Computes liquid-phase activity coefficients `γ_i` for a mixture from the
*group* composition of each molecule, like classic UNIFAC (the Fredenslund
"solution-of-groups" idea), but with the two Dortmund modifications of
Weidlich & Gmehling (1987) / Gmehling et al. (1993):

1. **Modified combinatorial part** — the Staverman–Guggenheim volume
   fraction in the Flory–Huggins term is raised to the power `3/4`, and the
   combinatorial is written in the algebraically-compact `1 − J + ln J − …`
   form. See [`ln_gamma_combinatorial`].
2. **Temperature-dependent group interactions** — the interaction energy is
   a quadratic in temperature, `a_mn(T) = a_mn⁰ + a_mn¹ T + a_mn² T²` (K),
   entering `Ψ_mn = exp(−a_mn(T) / T)`. Classic UNIFAC uses a constant
   `a_mn`. See [`ModfacParameters::interaction_a`] / [`ModfacParameters::psi`].

The Dortmund `R_k` / `Q_k` are *fitted* adjustable parameters (Gmehling et
al.), **not** Bondi van-der-Waals volumes/areas, and differ numerically from
the classic-UNIFAC table (e.g. Dortmund CH3 and CH2 share `R = 0.6325`).

The residual part is structurally identical to classic UNIFAC's
group-solution equation — only `Ψ_mn` becomes temperature-dependent — so this
module reuses the classic Fredenslund `ln Γ_k` residual form (mirroring the
sibling [`crate::thermo::unifac`] port) rather than DWSIM's compact
`β/θ/s` residual; the two are algebraically identical.

# Port provenance

Ported from DWSIM (GPL-3.0), commit `1abf72d`:
`DWSIM.Thermodynamics/PropertyPackages/Models/MODFAC.vb`, class `Modfac`.
The algebra mirrored here:

- molecular `r_i = Σ_k ν_k^i R_k` — `RET_Ri`, `MODFAC.vb:398-410`;
- molecular `q_i = Σ_k ν_k^i Q_k` — `RET_Qi`, `MODFAC.vb:412-424`;
- group area fraction `e_ki = ν_k^i Q_k / q_i` — `RET_EKI`, `MODFAC.vb:426-438`;
- `τ_mk = exp(−(a_mk + b_mk T + c_mk T²) / T)` — `TAU`, `MODFAC.vb:358-396`;
- modified combinatorial `ln γ_i^C = 1 − J'_i + ln J'_i
  − 5 q_i (1 − J_i/L_i + ln(J_i/L_i))` with `J'_i = r_i^{3/4}/Σ_j x_j r_j^{3/4}`,
  `J_i = r_i/Σ_j x_j r_j`, `L_i = q_i/Σ_j x_j q_j` — `GAMMA_MR`,
  `MODFAC.vb:283-286`;
- residual `ln γ_i^R` — `GAMMA_MR`, `MODFAC.vb:289-296` (compact form; this
  port writes the equivalent classic `ln Γ_k` form, see above);
- `γ_i = exp(ln γ_i^C + ln γ_i^R)` — `GAMMA_MR`, `MODFAC.vb:297`.

# Parameter-table provenance (public-literature subset)

The bundled table [`ModfacParameters::dortmund_vle_subset`] is a **small
public-literature subset**, not DWSIM's full asset files. Sources (identical
to DWSIM's GPL-3.0 `modfac.txt` / `modfac_ip.txt` asset rows, whose own
literature headers cite the same papers):

- `R_k`, `Q_k` and subgroup→main-group assignments: DWSIM `modfac.txt`
  (Modified UNIFAC (Dortmund) subgroup table), rows for CH3/CH2 (main 1),
  OH (main 5), H2O (main 7). These are the Gmehling-fitted values of
  Weidlich & Gmehling, *Ind. Eng. Chem. Res.* **26**, 1372 (1987) and
  Gmehling, Li & Schiller, *Ind. Eng. Chem. Res.* **32**, 178 (1993).
- `a_mn⁰, a_mn¹, a_mn²` temperature-polynomial interaction parameters:
  DWSIM `modfac_ip.txt`, source-tag `2` = Gmehling, Li & Schiller,
  *Ind. Eng. Chem. Res.* **32**, 178 (1993).

These tables are published, openly-cited literature data and are permitted
under the workspace `DATA_POLICY.md`. The subset covers only alkane (CH2),
alcohol (OH) and water (H2O) groups — enough for the alkane / ethanol /
water examples and tests here.

**Deferred data acquisition:** porting the *full* Dortmund subgroup and
`a/b/c` interaction matrix (DWSIM's complete `modfac.txt` /
`modfac_ip.txt`, ~45 main groups) is deliberately out of scope for this
change and tracked as a separate data-acquisition step.

# Honest scope — untrusted AI-assisted draft, verification not validation

**This is an untrusted AI-assisted draft pending human V&V.** It has been
**verified** (the code reproduces an independent second implementation of the
Dortmund equations from the same published parameters, and satisfies the
model's exact identities — see the test docs) but **not validated** against
experimental phase-equilibrium benchmarks. Modified UNIFAC is itself an
approximation. Treat outputs as unverified draft physics.

Explicitly **excluded** from this port:
- Modified UNIFAC (NIST) — different parameter set (`NISTMFAC.vb`); not here.
- Excess-enthalpy / heat-capacity derivatives (`HEX_MIX`, `CPEX_MIX`,
  `DLNGAMMA_DT` in the upstream file).
- The full parameter matrix (see "Deferred data acquisition" above).
- DWSIM's `CheckParameters` missing-pair diagnostic (`MODFAC.vb:316-356`) —
  here a missing pair falls through to `a = 0 ⇒ Ψ = 1`, matching `TAU`.

# Units

All public functions take/return raw `f64` in the DWSIM-internal SI
convention (temperature in **K**, mole fractions dimensionless in `[0, 1]`
summing to 1). Activity coefficients `γ_i` are dimensionless and `> 0`. This
follows the crate `CLAUDE.md` rule of raw documented `f64` in inner
thermodynamic loops; the quantities here are plain scalars, so no `uom`
type-aliasing is needed (contrast the crate's EOS public surface).

```rust
pub mod unifac_dortmund { /* ... */ }
```

### Types

#### Struct `ModfacSubgroup`

One Modified-UNIFAC (Dortmund) subgroup's constant parameters.

A subgroup (e.g. `CH3`, `OH`, `H2O`) belongs to a *main* group; interaction
polynomials `a_mn(T)` are indexed by **main** group, while the fitted
volume/area are per **subgroup**. Mirrors DWSIM's `ModfacGroup`
(`MODFAC.vb:671-749`).

```rust
pub struct ModfacSubgroup {
    pub subgroup_id: usize,
    pub main_group_id: usize,
    pub r: f64,
    pub q: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `subgroup_id` | `usize` | Subgroup id (DWSIM `Secondary_Group`, the `no.` column of `modfac.txt`).<br>Dimensionless integer key. |
| `main_group_id` | `usize` | Main-group id this subgroup belongs to (DWSIM `PrimaryGroup`, the `main`<br>column). Interaction parameters are looked up on this id. |
| `r` | `f64` | Fitted Dortmund group volume `R_k` (dimensionless). Unlike classic<br>UNIFAC this is an *adjustable* parameter, not a Bondi volume. Valid range<br>roughly `0.3 … 3`. |
| `q` | `f64` | Fitted Dortmund group surface area `Q_k` (dimensionless), likewise an<br>adjustable parameter. Valid range roughly `0 … 3`. |

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
    fn clone(self: &Self) -> ModfacSubgroup { /* ... */ }
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
    fn eq(self: &Self, other: &ModfacSubgroup) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ModfacComponent`

A molecule expressed as its Dortmund group counts `ν_k` (the caller-supplied
molecular structure).

`groups` is a list of `(subgroup_id, count)` pairs, e.g. ethanol is
`[(1, 1.0), (2, 1.0), (14, 1.0)]` = one CH3, one CH2, one OH. Counts are
`f64` (they may be fractional for pseudo-components) and must be `≥ 0`.

```rust
pub struct ModfacComponent {
    pub groups: Vec<(usize, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `groups` | `Vec<(usize, f64)>` | `(subgroup_id, ν_k)` pairs. A subgroup id must exist in the<br>[`ModfacParameters`] table used with this component. |

##### Implementations

###### Methods

- ```rust
  pub fn new(groups: Vec<(usize, f64)>) -> Self { /* ... */ }
  ```
  Construct from `(subgroup_id, count)` pairs.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ModfacComponent { /* ... */ }
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
    fn eq(self: &Self, other: &ModfacComponent) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ModfacInteraction`

One directional main-group interaction polynomial
`a_mn(T) = a + b·T + c·T²` (K).

Coefficients as tabulated in DWSIM `modfac_ip.txt` (`a` in K, `b` in K/K, `c`
in K/K²). Mirrors DWSIM's `InteracParam_aij/bij/cij` triple
(`MODFAC.vb:531-536`). Directional: `a_mn ≠ a_nm` in general.

```rust
pub struct ModfacInteraction {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Constant term `a_mn⁰` (K). |
| `b` | `f64` | Linear coefficient `a_mn¹` (dimensionless, multiplies `T` in K). |
| `c` | `f64` | Quadratic coefficient `a_mn²` (1/K, multiplies `T²`). |

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
    fn clone(self: &Self) -> ModfacInteraction { /* ... */ }
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
    fn eq(self: &Self, other: &ModfacInteraction) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ModfacParameters`

A Modified-UNIFAC (Dortmund) parameter table: subgroup volume/area
parameters plus the temperature-dependent main-group interaction matrix
`a_mn(T)` (K).

Owned by value (`HashMap`s, no lifetimes, no `dyn`), indexed by integer
subgroup / main-group ids, per the workspace design rules.

```rust
pub struct ModfacParameters {
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
  Build an empty table (no subgroups, no interactions).

- ```rust
  pub fn add_subgroup(self: &mut Self, subgroup: ModfacSubgroup) { /* ... */ }
  ```
  Register a subgroup's `R_k` / `Q_k` and its main-group assignment.

- ```rust
  pub fn set_interaction(self: &mut Self, main_m: usize, main_n: usize, a: f64, b: f64, c: f64) { /* ... */ }
  ```
  Set the directional main-group interaction polynomial

- ```rust
  pub fn subgroup(self: &Self, subgroup_id: usize) -> Option<&ModfacSubgroup> { /* ... */ }
  ```
  Look up a subgroup by id.

- ```rust
  pub fn interaction_a(self: &Self, main_m: usize, main_n: usize, temperature: f64) -> f64 { /* ... */ }
  ```
  Directional interaction energy `a_mn(T) = a + b·T + c·T²` (K), evaluated

- ```rust
  pub fn psi(self: &Self, main_m: usize, main_n: usize, temperature: f64) -> f64 { /* ... */ }
  ```
  Group interaction factor `Ψ_mn = exp(−a_mn(T) / T)` (dimensionless), with

- ```rust
  pub fn dortmund_vle_subset() -> Self { /* ... */ }
  ```
  Public-literature subset of the **Modified UNIFAC (Dortmund)** table.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ModfacParameters { /* ... */ }
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
    fn eq(self: &Self, other: &ModfacParameters) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ModfacTable`

Which Modified-UNIFAC (Dortmund) parameter table to use — an enum dispatch
point (no `dyn`) so future variants (the full matrix, NIST) slot in as new
arms.

```rust
pub enum ModfacTable {
    DortmundVle,
}
```

##### Variants

###### `DortmundVle`

Dortmund (VLE) subset ([`ModfacParameters::dortmund_vle_subset`]).

##### Implementations

###### Methods

- ```rust
  pub fn parameters(self: Self) -> ModfacParameters { /* ... */ }
  ```
  Materialise the chosen table's parameters.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ModfacTable { /* ... */ }
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
    fn eq(self: &Self, other: &ModfacTable) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `molecular_r_q`

Molecular volume `r_i = Σ_k ν_k^i R_k` and surface area `q_i = Σ_k ν_k^i Q_k`
(both dimensionless) for one component.

Ported from `RET_Ri` / `RET_Qi` (`MODFAC.vb:398-424`). Panics if a subgroup
id in `component` is missing from `params` — the caller is expected to build
components against the same table.

```rust
pub fn molecular_r_q(params: &ModfacParameters, component: &ModfacComponent) -> (f64, f64) { /* ... */ }
```

#### Function `ln_gamma_combinatorial`

Modified (Dortmund) combinatorial part `ln γ_i^C` for every component
(entropic, size/shape term; temperature-independent).

Dortmund form (`MODFAC.vb:283-286`), which replaces classic UNIFAC's
Staverman–Guggenheim combinatorial with a Flory–Huggins term whose volume
fraction is raised to the power `3/4`:

`ln γ_i^C = 1 − J'_i + ln J'_i − 5 q_i (1 − J_i/L_i + ln(J_i/L_i))`

with the **`3/4`-power** volume fraction `J'_i = r_i^{3/4} / Σ_j x_j r_j^{3/4}`,
the ordinary volume fraction `J_i = r_i / Σ_j x_j r_j`, and the surface-area
fraction `L_i = q_i / Σ_j x_j q_j`; the literal `5` is `z/2` with `z = 10`.
All three fractions are formed without dividing by `x_i`, so `x_i = 0`
(infinite dilution) stays finite.

Reduction check: if all components share the same `r_i` and `q_i`, then
`J'_i = J_i = L_i = 1` and `ln γ_i^C = 0` for every composition (see the
`identical_molecules_are_ideal` test). Setting the exponent to `1`
(`J'_i = J_i`) recovers the classic-UNIFAC combinatorial written in the
equivalent `1 − J + ln J − …` algebra.

`x` are mole fractions (dimensionless, sum ≈ 1); returns one `ln γ_i^C` per
component.

```rust
pub fn ln_gamma_combinatorial(params: &ModfacParameters, components: &[ModfacComponent], x: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `group_ln_gamma`

Residual group-activity `ln Γ_k` for every subgroup present in `group_counts`.

The classic Fredenslund group-solution equation (unchanged in structure from
UNIFAC — only `Ψ` is now temperature-dependent):

`ln Γ_k = Q_k [ 1 − ln(Σ_m θ_m Ψ_mk) − Σ_m ( θ_m Ψ_km / Σ_n θ_n Ψ_nm ) ]`

with group area fraction `θ_m = Q_m X_m / Σ_n Q_n X_n` and group mole
fraction `X_m`. `Ψ` is on main groups. Returned map is keyed by subgroup id;
`temperature` is in K.

This is the classic-form equivalent of DWSIM's compact `β/θ/s` residual
block (`MODFAC.vb:289-296`), used for both the mixture and the
pure-component reference in [`ln_gamma_residual`].

```rust
pub fn group_ln_gamma(params: &ModfacParameters, group_counts: &std::collections::HashMap<usize, f64>, temperature: f64) -> std::collections::HashMap<usize, f64> { /* ... */ }
```

#### Function `ln_gamma_residual`

Residual part `ln γ_i^R = Σ_k ν_k^i (ln Γ_k − ln Γ_k^i)` for every component
(enthalpic, energetic-interaction term; temperature-dependent).

`ln Γ_k` is the group activity in the actual mixture and `ln Γ_k^i` the group
activity in a fluid of pure component `i` (its reference state), both from
[`group_ln_gamma`]. Ported from `GAMMA_MR` (`MODFAC.vb:289-296`).
`x` are mole fractions; `temperature` is in K.

```rust
pub fn ln_gamma_residual(params: &ModfacParameters, components: &[ModfacComponent], x: &[f64], temperature: f64) -> Vec<f64> { /* ... */ }
```

#### Function `activity_coefficients`

Liquid-phase activity coefficients `γ_i = exp(ln γ_i^C + ln γ_i^R)` for every
component (dimensionless, `> 0`), from the Modified UNIFAC (Dortmund) model.

Top-level entry point, ported from `GAMMA_MR` (`MODFAC.vb:58-314`). Inputs:
- `params` — the parameter table (e.g. [`ModfacTable::DortmundVle`]);
- `components` — each molecule's group counts `ν_k^i`;
- `x` — mole fractions (dimensionless, should sum to ≈ 1), same order/length
  as `components`;
- `temperature` — in K (Dortmund UNIFAC is calibrated ~275–425 K; the
  `a_mn(T)` polynomials are fitted over that range).

A pure component (`x = [1.0]`) returns `γ = 1` exactly.

```rust
pub fn activity_coefficients(params: &ModfacParameters, components: &[ModfacComponent], x: &[f64], temperature: f64) -> Vec<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `COORDINATION_NUMBER`

Coordination number `z` of the UNIFAC/UNIQUAC lattice (dimensionless).
Fixed at 10 (as in classic UNIFAC and the Dortmund model); enters the
Staverman–Guggenheim combinatorial correction as `z/2 = 5` (the literal `5`
in `MODFAC.vb:286`).

```rust
pub const COORDINATION_NUMBER: f64 = 10.0;
```

## Module `unifac_lle`

**Attributes:**

- `Other("#[forbid(unsafe_code)]")`

UNIFAC-LLE — UNIFAC group-contribution activity coefficients with the
**liquid–liquid-equilibrium (LLE) parameterised** group-interaction table.

UNIFAC-LLE uses the *identical functional form* to the original (VLE)
UNIFAC model — the same Bondi group volumes `R_k` / surface areas `Q_k`, the
same Staverman–Guggenheim combinatorial term, and the same Fredenslund
group-residual term — but replaces the temperature-dependent VLE
interaction energies with a **separate, temperature-independent set of
`a_mn` (K)** fitted to *liquid–liquid* equilibrium data (Magnussen,
Rasmussen & Fredenslund 1981). These LLE parameters produce the stronger
positive deviations from Raoult's law needed to reproduce partial
miscibility, which is why this variant feeds the LLE flash rather than the
VLE model.

Because the algebra is unchanged, this module **reuses** the verified
implementation in [`super::unifac`] (combinatorial term, group-residual
term, and the `γ_i = exp(ln γ^C + ln γ^R)` assembly) and only supplies the
LLE parameter table. Nothing in `unifac.rs` is modified.

# Port provenance

Ported from DWSIM (GPL-3.0), commit `1abf72d`:

- Property-package wrapper: `DWSIM.Thermodynamics/PropertyPackages/UNIFACLL.vb`,
  class `UNIFACLLPropertyPackage` (`UNIFACLL.vb:28-139`) — thin wrapper that
  holds an `Auxiliary.UnifacLL` model (`UNIFACLL.vb:45-72`).
- Model class: `DWSIM.Thermodynamics/PropertyPackages/Models/UNIFAC.vb`,
  class `UnifacLL` (`Models/UNIFAC.vb:491-501`). `UnifacLL` **inherits**
  `Unifac` and differs *only* by constructing its group table with the LLE
  flag set — `UnifGroups = New UnifacGroups(True)` (`Models/UNIFAC.vb:497`).
- Group-table loader with the LLE branch: `UnifacGroups.New(ll As Boolean)`
  (`Models/UNIFAC.vb:508-625`); the `If ll Then …` block that layers the LLE
  interaction file on top is `Models/UNIFAC.vb:562-594`.
- The activity-coefficient algebra (`Unifac.GAMMA_MR`, `RET_Ri`, `RET_Qi`,
  `TAU`, …) is the same code already cited in [`super::unifac`]'s header and
  is reused here unchanged.

# Parameter-table provenance (public-literature LLE subset)

The bundled table [`magnussen_lle_subset`] is a **small public-literature
subset** of the UNIFAC-LLE interaction matrix, not DWSIM's full asset file.
Sources:

- `a_mn` LLE interaction energies (K): Magnussen, T.; Rasmussen, P.;
  Fredenslund, A., *"UNIFAC Parameter Table for Prediction of
  Liquid–Liquid Equilibria"*, **Ind. Eng. Chem. Process Des. Dev.** 20 (2),
  331–339 (1981), <https://doi.org/10.1021/i200013a024>. The identical
  values appear (comma-delimited, `main_m,name_m,main_n,name_n,a_mn,a_nm`)
  in DWSIM's `DWSIM.Thermodynamics/Assets/unifac_ll_ip.txt`; the specific
  rows replicated here are:
  - `1,CH2,3,ACH,-114.8,156.5`
  - `1,CH2,4,ACCH2,-115.7,104.4`
  - `1,CH2,5,OH,644.6,328.2`
  - `1,CH2,8,H2O,1300,342.4`
  - `3,ACH,4,ACCH2,167,-146.8`
  - `3,ACH,5,OH,703.9,-9.21`
  - `3,ACH,8,H2O,859.4,372.8`
  - `4,ACCH2,5,OH,4000,1.27`
  - `4,ACCH2,8,H2O,5695,203.7`
  - `5,OH,8,H2O,28.73,-122.4`
- `R_k`, `Q_k` and subgroup→main-group assignments: the Bondi-derived group
  parameters common to all UNIFAC variants — Hansen, Rasmussen, Fredenslund,
  Schiller & Gmehling, *Ind. Eng. Chem. Res.* **30**, 2352 (1991); identical
  to DWSIM's `DWSIM.Thermodynamics/Assets/unifac.txt` (`SUB_ID`, `Rk`, `Qk`
  columns). The `R`/`Q` values are the same as in [`super::unifac`]; only
  the interaction matrix differs between VLE and LLE.

These tables are published, openly-cited literature data and are permitted
under the workspace `DATA_POLICY.md`. The subset covers only the alkane
(CH2), aromatic-CH (ACH), aromatic-CH2 (ACCH2), hydroxyl (OH) and water
(H2O) main groups — enough for the alkane / aromatic / alcohol / water LLE
examples and tests here.

## Main-group numbering — deliberate deviation from DWSIM's file merge

DWSIM's `UnifacGroups` keys its interaction dictionary by the *VLE* main-group
ids from `unifac.txt` (where `H2O` is main group **7**) and then overlays the
LLE rows from `unifac_ll_ip.txt` *keyed by the LLE ids* (where `H2O` is main
group **8**), which are not the same numbering for ids ≥ 6. This port does
**not** replicate that cross-numbering overlay. Instead it assigns every
subgroup a main-group id from the **LLE (Magnussen 1981) numbering**
(`CH2 = 1`, `ACH = 3`, `ACCH2 = 4`, `OH = 5`, `H2O = 8`) and keys the
interactions with that *same* scheme, so R/Q assignment and `a_mn` lookup are
internally consistent. See "Honest scope" below.

# Honest scope — untrusted AI-assisted draft, verification not validation

**This module is an untrusted AI-assisted draft pending human V&V.** It has
been **verified** — the reused algebra is the same code already cross-checked
against an independent implementation in [`super::unifac`], and the tests
below confirm the model identities (pure-component and identical-molecule
ideality hold exactly, infinite dilution is finite, and the LLE table
produces strong positive deviations for phase-splitting systems). Note the
LLE table is **not** uniformly "stronger" than the VLE table: for
butanol/water at equimolar composition it is in fact slightly milder, while
for aromatic/aqueous pairs and at infinite dilution it produces the large
deviations that partial miscibility requires (see the tests for the actual
side-by-side LLE-vs-VLE numbers). It has **not** been *validated* against
experimental liquid–liquid tie-line /
mutual-solubility benchmarks; the reported activity coefficients are model
outputs, not measured phase behaviour, and must not be treated as
experimentally confirmed. UNIFAC-LLE is itself a correlation with known
limitations.

Explicitly **excluded** from this port:
- The full LLE `a_mn` matrix (DWSIM's complete `unifac_ll_ip.txt`, ~32 main
  groups / 255 pairs); only the 10-pair subset above is bundled.
- DWSIM's VLE/LLE main-group cross-numbering overlay (see above) — this port
  uses a single, internally-consistent LLE numbering instead.
- The 1-propanol / 2-propanol special main groups (LLE ids 6/7) that DWSIM's
  LLE table distinguishes from the generic CH2/OH split.
- User-database interaction overrides (`Models/UNIFAC.vb:596-623`).
- Excess-enthalpy / heat-capacity derivatives.

# Units

All public functions take/return raw `f64` in the DWSIM-internal SI
convention: temperature in **K**, mole fractions dimensionless in `[0, 1]`
summing to 1. Activity coefficients `γ_i` are dimensionless and `> 0`. This
follows the crate `CLAUDE.md` rule of raw documented `f64` in inner
thermodynamic loops.

```rust
pub mod unifac_lle { /* ... */ }
```

### Types

#### Type Alias `ActivityCoefficients`

A vector of liquid-phase activity coefficients `γ_i` (dimensionless, `> 0`),
one entry per component in the same order as the input `components` / `x`.

Named alias for readability at call sites (the underlying type is a plain
`Vec<f64>`; the semantic content is the per-component `γ_i`).

```rust
pub type ActivityCoefficients = Vec<f64>;
```

#### Enum `UnifacLleTable`

Which UNIFAC-LLE parameter table to use — an enum dispatch point (no `dyn`,
no `Box`) so future LLE tables (e.g. the full Magnussen matrix) slot in as
new arms, mirroring [`super::unifac::UnifacTable`].

```rust
pub enum UnifacLleTable {
    MagnussenLle,
}
```

##### Variants

###### `MagnussenLle`

UNIFAC-LLE, Magnussen et al. (1981) public-literature subset
([`magnussen_lle_subset`]).

##### Implementations

###### Methods

- ```rust
  pub fn parameters(self: Self) -> UnifacParameters { /* ... */ }
  ```
  Materialise the chosen LLE table's parameters.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> UnifacLleTable { /* ... */ }
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
    fn eq(self: &Self, other: &UnifacLleTable) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `magnussen_lle_subset`

Public-literature subset of the **UNIFAC-LLE** parameter table
(Magnussen, Rasmussen & Fredenslund 1981).

Returns a [`super::unifac::UnifacParameters`] populated with:
- the Bondi group volumes/areas `R_k`, `Q_k` for the CH3/CH2/CH/C, ACH/AC,
  ACCH3/ACCH2/ACCH, OH and H2O subgroups (dimensionless, identical to the
  VLE model — only the interactions differ); and
- the temperature-independent LLE interaction energies `a_mn` (K) among the
  CH2 / ACH / ACCH2 / OH / H2O main groups, exactly as listed in the module
  header.

The result plugs directly into the reused [`super::unifac`] algebra. Because
`a_mn` is temperature-independent for the LLE table, `Ψ_mn = exp(−a_mn / T)`
still varies with `T` through the `1/T` factor (as in every UNIFAC variant);
it is only the *fitted energies* `a_mn` that carry no explicit `T`
dependence. Valid over the LLE-fit range (roughly 273–373 K).

```rust
pub fn magnussen_lle_subset() -> super::unifac::UnifacParameters { /* ... */ }
```

#### Function `activity_coefficients_lle`

Liquid-phase activity coefficients `γ_i` from the UNIFAC-**LLE** table, for
every component (dimensionless, `> 0`).

Convenience top-level entry point: identical to
[`super::unifac::activity_coefficients`] but wired to the LLE parameter set.
Inputs:
- `components` — each molecule's group counts `ν_k^i`, built against the
  subgroup ids exposed as `SUB_*` constants in this module;
- `x` — mole fractions (dimensionless, should sum to ≈ 1), same order/length
  as `components`;
- `temperature` — in K (LLE parameters fit over roughly 273–373 K).

A pure component (`x = [1.0]`) returns `γ = 1` exactly. Mole-fraction and
group-count contracts are inherited unchanged from the reused base algebra.

```rust
pub fn activity_coefficients_lle(components: &[super::unifac::UnifacComponent], x: &[f64], temperature: f64) -> ActivityCoefficients { /* ... */ }
```

#### Function `ln_gamma_combinatorial_lle`

Combinatorial part `ln γ_i^C` under the LLE table, for every component.

The combinatorial term is table-independent (it depends only on `R_k` / `Q_k`,
which are shared with the VLE model), so this simply forwards to
[`super::unifac::ln_gamma_combinatorial`] with the LLE `R`/`Q` set. Provided
for parity with the base module and for tests. `x` are mole fractions
(dimensionless).

```rust
pub fn ln_gamma_combinatorial_lle(components: &[super::unifac::UnifacComponent], x: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `ln_gamma_residual_lle`

Residual part `ln γ_i^R` under the **LLE** interaction table, for every
component (this is where the LLE parameters actually enter).

Forwards to [`super::unifac::ln_gamma_residual`] with the LLE `a_mn` set;
`x` are mole fractions (dimensionless) and `temperature` is in K.

```rust
pub fn ln_gamma_residual_lle(components: &[super::unifac::UnifacComponent], x: &[f64], temperature: f64) -> Vec<f64> { /* ... */ }
```

#### Function `molecular_r_q_lle`

Molecular volume `r_i` and surface area `q_i` (both dimensionless) for one
component under the LLE table's `R_k` / `Q_k` (identical to the VLE values).

Forwards to [`super::unifac::molecular_r_q`]. Panics if a subgroup id in
`component` is not in the LLE subset table.

```rust
pub fn molecular_r_q_lle(component: &super::unifac::UnifacComponent) -> (f64, f64) { /* ... */ }
```

### Constants and Statics

#### Constant `MAIN_CH2`

LLE main group 1 — aliphatic `CH2` (subgroups CH3/CH2/CH/C).

```rust
pub const MAIN_CH2: usize = 1;
```

#### Constant `MAIN_ACH`

LLE main group 3 — aromatic `ACH` (subgroups ACH/AC).

```rust
pub const MAIN_ACH: usize = 3;
```

#### Constant `MAIN_ACCH2`

LLE main group 4 — aromatic `ACCH2` (subgroups ACCH3/ACCH2/ACCH).

```rust
pub const MAIN_ACCH2: usize = 4;
```

#### Constant `MAIN_OH`

LLE main group 5 — hydroxyl `OH`.

```rust
pub const MAIN_OH: usize = 5;
```

#### Constant `MAIN_H2O`

LLE main group 8 — water `H2O`.

```rust
pub const MAIN_H2O: usize = 8;
```

#### Constant `SUB_CH3`

Subgroup id for `CH3` (main group [`MAIN_CH2`]).

```rust
pub const SUB_CH3: usize = 1;
```

#### Constant `SUB_CH2`

Subgroup id for `CH2` (main group [`MAIN_CH2`]).

```rust
pub const SUB_CH2: usize = 2;
```

#### Constant `SUB_CH`

Subgroup id for `CH` (main group [`MAIN_CH2`]).

```rust
pub const SUB_CH: usize = 3;
```

#### Constant `SUB_C`

Subgroup id for `C` (main group [`MAIN_CH2`]).

```rust
pub const SUB_C: usize = 4;
```

#### Constant `SUB_ACH`

Subgroup id for aromatic `ACH` (main group [`MAIN_ACH`]).

```rust
pub const SUB_ACH: usize = 10;
```

#### Constant `SUB_AC`

Subgroup id for aromatic `AC` (main group [`MAIN_ACH`]).

```rust
pub const SUB_AC: usize = 11;
```

#### Constant `SUB_ACCH3`

Subgroup id for aromatic `ACCH3` (main group [`MAIN_ACCH2`]).

```rust
pub const SUB_ACCH3: usize = 12;
```

#### Constant `SUB_ACCH2`

Subgroup id for aromatic `ACCH2` (main group [`MAIN_ACCH2`]).

```rust
pub const SUB_ACCH2: usize = 13;
```

#### Constant `SUB_ACCH`

Subgroup id for aromatic `ACCH` (main group [`MAIN_ACCH2`]).

```rust
pub const SUB_ACCH: usize = 14;
```

#### Constant `SUB_OH`

Subgroup id for hydroxyl `OH` (main group [`MAIN_OH`]).

```rust
pub const SUB_OH: usize = 15;
```

#### Constant `SUB_H2O`

Subgroup id for water `H2O` (main group [`MAIN_H2O`]).

```rust
pub const SUB_H2O: usize = 17;
```

### Re-exports

#### Re-export `Component`

```rust
pub use component::Component;
```

## Module `valve`

Control-valve sizing (IEC 60534 / ISA-75.01.01).

Ported from DWSIM `UnitOperations/Valve.vb` -- see `iec_60534`'s module
doc for the full source mapping and unit conventions.

```rust
pub mod valve { /* ... */ }
```

### Modules

## Module `iec_60534`

IEC 60534 / ISA-75.01.01 control-valve sizing equations.

Ported from DWSIM `UnitOperations/Valve.vb` (`KvLiquid`/`WLiquid`,
`KvGas`/`WGas`, `KvTwoPhase`/`WTwoPhase`, `P2Liquid`/`P2_Gas`/`P2TwoPhase`),
which in turn cites SAMSON T00050EN (steam service) and the Masoneilan
Control Valve Sizing Handbook (two-phase combination). Piping geometry
factor `F_P`, style modifiers `F_L`/`x_T`, and the unit constant `N6` are
taken as caller-supplied per IEC 60534-2-1 (defaults of 1.0 are reasonable
placeholders when those effects are not being modelled).

`Kv` here follows the IEC 60534 metric convention: dimensioned in
m^3 h^-1 bar^-0.5, referenced to water density 999.1 kg/m^3 -- this is an
engineering unit, not a `uom` SI quantity, so it is wrapped in
[`ValveFlowCoefficient`] rather than left as a bare `f64`.

```rust
pub mod iec_60534 { /* ... */ }
```

### Types

#### Struct `ValveFlowCoefficient`

Valve flow coefficient `Kv` \[m^3 h^-1 bar^-0.5\], IEC 60534 metric
convention (referenced to water at 999.1 kg/m^3). Not a `uom` SI
quantity -- `Kv`/`Cv` are defined by their sizing equation, not a
fundamental physical dimension.

```rust
pub struct ValveFlowCoefficient(pub f64);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn to_cv(self: Self) -> f64 { /* ... */ }
  ```
  Convert to the US customary flow coefficient `Cv` \[US gal/min

- ```rust
  pub fn from_cv(cv: f64) -> Self { /* ... */ }
  ```
  Construct from a `Cv` \[US gal/min psi^-0.5\] value.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ValveFlowCoefficient { /* ... */ }
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
    fn eq(self: &Self, other: &ValveFlowCoefficient) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &ValveFlowCoefficient) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `OpeningCharacteristic`

Which opening-characteristic curve relates a valve's opening percentage
`OP` \[0, 100\] to its flow coefficient. DWSIM's `UserDefined` (a
user-entered expression) and `DataTable` (an arbitrary lookup table) are
not represented here -- those need a runtime expression evaluator /
interpolation table respectively, out of scope for this port.

```rust
pub enum OpeningCharacteristic {
    Linear,
    QuickOpening,
    EqualPercentage {
        rangeability: f64,
    },
}
```

##### Variants

###### `Linear`

`Kv(OP) = Kv_max * OP/100`.

###### `QuickOpening`

`Kv(OP) = Kv_max * sqrt(OP/100)`.

###### `EqualPercentage`

`Kv(OP) = Kv_max * R^(OP/100 - 1)`, `R` = rangeability parameter.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `rangeability` | `f64` | Rangeability parameter `R` (typically 20-50). |

##### Implementations

###### Methods

- ```rust
  pub fn kv_at_opening(self: &Self, kv_max: ValveFlowCoefficient, opening_pct: Ratio) -> ValveFlowCoefficient { /* ... */ }
  ```
  Evaluate `Kv` at a given opening percentage `opening_pct` \[0, 100\].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> OpeningCharacteristic { /* ... */ }
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
    fn eq(self: &Self, other: &OpeningCharacteristic) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `liquid_critical_pressure_ratio_factor`

Liquid critical-pressure ratio factor `F_F = 0.96 - 0.28 sqrt(p_v/p_c)`
(IEC 60534), used to find the choked-flow pressure-drop limit for liquid
service.

```rust
pub fn liquid_critical_pressure_ratio_factor(vapor_pressure: uom::si::f64::Pressure, critical_pressure: uom::si::f64::Pressure) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `choked_pressure_drop_liquid`

Choked-flow pressure-drop limit for liquid service,
`ΔP_choke = F_L^2 (p1 - F_F p_v)`. If the actual pressure drop `p1 - p2`
exceeds this, the valve is choked and `p2` should be clamped to
`p1 - ΔP_choke` before evaluating [`kv_liquid`]/[`mass_flow_liquid`].

```rust
pub fn choked_pressure_drop_liquid(p1: uom::si::f64::Pressure, liquid_pressure_recovery_factor: uom::si::f64::Ratio, f_f: uom::si::f64::Ratio, vapor_pressure: uom::si::f64::Pressure) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `clamp_to_choked_liquid`

Clamp `p2` to the choked-flow limit for liquid service, if the requested
drop `p1 - p2` would exceed it. Returns the (possibly clamped) outlet
pressure to use in the sizing equations.

```rust
pub fn clamp_to_choked_liquid(p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, liquid_pressure_recovery_factor: uom::si::f64::Ratio, f_f: uom::si::f64::Ratio, vapor_pressure: uom::si::f64::Pressure) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `kv_liquid`

Required `Kv` for liquid service given mass flow `w`, density `rho`, and
pressure drop `p1 - p2` (already clamped to the choked limit if
applicable, see [`clamp_to_choked_liquid`]), and piping geometry factor
`f_p`.

```rust
pub fn kv_liquid(w: uom::si::f64::MassRate, density: uom::si::f64::MassDensity, p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, f_p: uom::si::f64::Ratio) -> ValveFlowCoefficient { /* ... */ }
```

#### Function `mass_flow_liquid`

Mass flow rate for liquid service given `Kv`, density, pressure drop
(already clamped to the choked limit if applicable), and piping geometry
factor `f_p` -- the inverse of [`kv_liquid`].

```rust
pub fn mass_flow_liquid(kv: ValveFlowCoefficient, density: uom::si::f64::MassDensity, p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, f_p: uom::si::f64::Ratio) -> uom::si::f64::MassRate { /* ... */ }
```

#### Function `solve_p2_liquid`

Outlet pressure `p2` for a target liquid mass flow `w`, given `Kv` --
closed-form inverse of [`kv_liquid`] (liquid is the only service where
this back-solve is closed-form; [`solve_p2_gas`] needs bisection instead,
since `Y`/choking make it non-invertible in closed form. A two-phase P2
back-solve -- DWSIM's `P2TwoPhase`/`P1TwoPhase` -- is not ported here;
this pass only covers the forward two-phase sizing in [`kv_two_phase`]/
[`mass_flow_two_phase`]).

```rust
pub fn solve_p2_liquid(w: uom::si::f64::MassRate, kv: ValveFlowCoefficient, density: uom::si::f64::MassDensity, p1: uom::si::f64::Pressure, f_p: uom::si::f64::Ratio) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `expansion_factor_gas`

Expansion factor `Y = 1 - x / (3 x_choked)` for gas/vapour service, where
`x = (p1-p2)/p1` is clamped to the choked-flow limit `x_choked =
(k/1.4) x_T` beforehand (see [`choked_pressure_drop_ratio_gas`]).

```rust
pub fn expansion_factor_gas(pressure_drop_ratio: uom::si::f64::Ratio, choked_pressure_drop_ratio: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `choked_pressure_drop_ratio_gas`

Choked-flow pressure-drop ratio limit for gas/vapour service,
`x_choked = (k/1.4) x_T`, where `k = Cp/Cv` (specific heat ratio) and
`x_T` is the valve's terminal pressure-drop ratio style modifier.

```rust
pub fn choked_pressure_drop_ratio_gas(specific_heat_ratio: uom::si::f64::Ratio, x_t: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `clamp_and_expand_gas`

Clamp the pressure-drop ratio `x = (p1-p2)/p1` to the gas choked-flow
limit, returning `(x_clamped, Y)`.

```rust
pub fn clamp_and_expand_gas(p1: uom::si::f64::Pressure, p2: uom::si::f64::Pressure, specific_heat_ratio: uom::si::f64::Ratio, x_t: uom::si::f64::Ratio) -> (uom::si::f64::Ratio, uom::si::f64::Ratio) { /* ... */ }
```

#### Function `kv_gas`

Required `Kv` for gas/vapour service given mass flow `w`, upstream
pressure `p1`, upstream density `rho1`, the (already-clamped) pressure
drop ratio `x` and expansion factor `y` from [`clamp_and_expand_gas`],
and piping geometry factor `f_p`.

```rust
pub fn kv_gas(w: uom::si::f64::MassRate, p1: uom::si::f64::Pressure, density_upstream: uom::si::f64::MassDensity, x: uom::si::f64::Ratio, y: uom::si::f64::Ratio, f_p: uom::si::f64::Ratio) -> ValveFlowCoefficient { /* ... */ }
```

#### Function `mass_flow_gas`

Mass flow rate for gas/vapour service -- the inverse of [`kv_gas`].

```rust
pub fn mass_flow_gas(kv: ValveFlowCoefficient, p1: uom::si::f64::Pressure, density_upstream: uom::si::f64::MassDensity, x: uom::si::f64::Ratio, y: uom::si::f64::Ratio, f_p: uom::si::f64::Ratio) -> uom::si::f64::MassRate { /* ... */ }
```

#### Function `solve_p2_gas`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Outlet pressure `p2` for a target gas/vapour mass flow `w`, given `Kv` --
bisection inverse of [`kv_gas`] (non-invertible in closed form because of
the `Y`/choking non-linearity, matching DWSIM's own `P2_Gas`).

```rust
pub fn solve_p2_gas(w: uom::si::f64::MassRate, kv: ValveFlowCoefficient, p1: uom::si::f64::Pressure, density_upstream: uom::si::f64::MassDensity, specific_heat_ratio: uom::si::f64::Ratio, x_t: uom::si::f64::Ratio, f_p: uom::si::f64::Ratio, tolerance: uom::si::f64::Pressure) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `kv_two_phase`

Two-phase `Kv`, combining independently-evaluated liquid and gas `Kv` at
the same `(p1, p2)` by mass-fraction-weighted quadrature (Masoneilan-style):
`Kv = sqrt(mass_frac_gas * Kv_gas^2 + mass_frac_liquid * Kv_liquid^2)`.

```rust
pub fn kv_two_phase(kv_liquid: ValveFlowCoefficient, kv_gas: ValveFlowCoefficient, mass_fraction_liquid: uom::si::f64::Ratio, mass_fraction_gas: uom::si::f64::Ratio) -> ValveFlowCoefficient { /* ... */ }
```

#### Function `mass_flow_two_phase`

Two-phase mass flow rate, combining independently-evaluated liquid and
gas mass flow at the same `(Kv, p1, p2)`:
`1/W^2 = mass_frac_liquid/W_liquid^2 + mass_frac_gas/W_gas^2`.

```rust
pub fn mass_flow_two_phase(mass_flow_liquid: uom::si::f64::MassRate, mass_flow_gas: uom::si::f64::MassRate, mass_fraction_liquid: uom::si::f64::Ratio, mass_fraction_gas: uom::si::f64::Ratio) -> uom::si::f64::MassRate { /* ... */ }
```

