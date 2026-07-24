# Crate Documentation

**Version:** 0.1.0

**Format Version:** 60

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
  rating, pump/expander thermodynamics) with `uom`-typed public APIs.
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

- [`component`] — the pure-compound constant-property data model
  ([`Component`]): critical properties, acentric factor, molar mass,
  ideal-gas heat-capacity coefficients. The shared substrate every other
  thermo module consumes. **Data substrate (this file's author).**
- [`cubic_eos`] — Peng-Robinson and SRK cubic EOS: compressibility solve,
  fugacity coefficients, enthalpy/entropy departures, van der Waals mixing.
- [`activity`] — NRTL / UNIQUAC / Ideal (Raoult) liquid-phase activity
  coefficients.
- [`unifac`] — UNIFAC group-contribution activity coefficients.
- [`ideal_props`] — ideal-gas heat capacity / enthalpy / entropy from the
  [`Component`] Cp0 coefficients (the departure reference state).
- [`flash`] — isothermal-isobaric (TP) vapour-liquid-equilibrium flash via
  the Rachford-Rice / Nested-Loops method, with Wilson K-value initialisation.
- [`property_package`] — glue that composes the cubic-EOS / ideal models into
  K-values and drives an EOS-consistent PT two-phase flash
  ([`property_package::PropertyPackageModel`], enum dispatch, no `dyn`).
- [`energy_flash`] — isenthalpic (PH) / energy flash: solve the temperature at
  which a mixture's total molar enthalpy meets a target `H` at fixed `P`.
- [`saturation`] — bubble-point / dew-point temperature & pressure of a
  multicomponent mixture, on top of the isothermal-isobaric VLE kernel.
- [`stability`] — phase-stability analysis via Michelsen's tangent-plane
  distance (TPD) criterion (single-/two-phase identification, flash init).
- [`transport`] — transport-property correlations (viscosity, thermal
  conductivity, surface tension) and their phase-mixing rules.
- [`eos_variants`] — cubic-EOS refinements: the PRSV α-function and the
  Peneloux volume translation, composed on top of [`cubic_eos`].

## Design (crate `CLAUDE.md`)

Enum dispatch (no `dyn`) for the EOS / activity / flash model choices; `uom`
at public boundaries where practical, documented raw `f64` (SI) in the inner
EOS/flash arithmetic loops where `uom` overhead would fight the math (the
DWSIM-internal SI convention: Pa, K, J/mol, kg/m³).

## Honest scope

This is the **core kernel**, not the whole of DWSIM's thermodynamics. The
one-parameter PRSV α-function and the Peneloux volume translation are ported
([`eos_variants`]); the long tail — Gibbs-minimisation and inside-out flashes,
3-phase / electrolyte / solid equilibria, the LKP and PRSV2/Mathias-Copeman/Twu
α-variants, seawater/sour-water/black-oil packages — remains future work (see
`docs/port-scope.md`, epic `op-qo2`).

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

- `PropertyPackageModel`

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

