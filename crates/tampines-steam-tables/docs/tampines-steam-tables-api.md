# Crate Documentation

**Version:** 0.2.9

**Format Version:** 61

# Module `tampines_steam_tables`

# TAMPINES Steam Tables

In-house Rust implementation of the IAPWS-IF97 industrial formulation for
the thermodynamic and transport properties of water and steam, used by the
TAMPINES (Thermo-hydraulic Artificial-intelligence Multi-Phase INtegrated
Emulator System) solver. Unlike the upstream `rust-steam` library it draws
from, every public property function takes and returns `uom` dimensioned
quantities (SI units) rather than bare `f64`.

## Organisation

Properties are grouped by IAPWS-IF97 region:

- Region 1 — subcooled liquid (273.15–623.15 K, up to 100 MPa, below saturation)
- Region 2 — vapour / superheated steam (incl. a metastable subregion)
- Region 3 — single-phase near-critical liquid/vapour + supercritical fluid
- Region 4 — vapour-liquid equilibrium (the saturation line)
- Region 5 — ultra-high-temperature steam (> 800 °C, up to ~2273 K)

Forward equations are `(p,T)` / `(v,T)` flashes; the backward (inverse)
equations solve from `(p,h)`, `(p,s)`, or `(h,s)`. The user-facing
region-dispatch entry points (both a functional API and the
`TampinesSteamTableCV` control-volume type) live in [`interfaces`]. Transport
and miscellaneous properties are in [`dynamic_viscosity`],
[`thermal_conductivity`], [`surface_tension`], and [`dielectric_constant`].
Nozzle / turbine and HEM choked-flow equations are in
[`steam_turbine_equations`]. The transient HEM pipe solver (density-based
KNP on `[ρ, ρu, ρE]`, closed by IF97 or any [`hem::HemEquationOfState`]) is
in [`hem`].

All quantities are SI: pressure in Pa, temperature in K, specific enthalpy
and energy in J/kg, specific entropy and heat capacity in J/(kg·K), specific
volume in m³/kg, density in kg/m³.

## Modules

## Module `prelude`

allows for easy importing as with most rust
crates.

```rust
pub mod prelude { /* ... */ }
```

### Modules

## Module `_prelude_is_sufficient`

Worked check that this prelude is sufficient on its own: saturation
properties and a backward flash, with no deeper import from this crate.

```
use tampines_steam_tables::prelude::*;
use uom::si::f64::*;
use uom::si::pressure::kilopascal;
use uom::si::thermodynamic_temperature::degree_celsius;

// Boiling point of water at 101.325 kPa is 99.97 C by IF97.
let p = Pressure::new::<kilopascal>(101.325);
let t_sat = sat_temp_4(p);
assert!((t_sat.get::<degree_celsius>() - 99.97).abs() < 0.02);
```

```rust
pub mod _prelude_is_sufficient { /* ... */ }
```

### Re-exports

#### Re-export `functional_programming`

```rust
pub use crate::interfaces::functional_programming;
```

#### Re-export `checked`

Bounds-checked, `Result`-returning facade over the panicking flash
internals — see [`crate::interfaces::checked`]. Import as
`tampines_steam_tables::prelude::checked::try_h_tp_eqm_single_phase`
(etc.) when out-of-range input must not kill the calling thread.

```rust
pub use crate::interfaces::checked;
```

#### Re-export `get_choked_flow_massrate_and_state_from_stagnation_properties_and_area`

```rust
pub use crate::steam_turbine_equations::converging_diverging_nozzles::isentropic_converging_nozzle::get_choked_flow_massrate_and_state_from_stagnation_properties_and_area;
```

#### Re-export `sat_pressure_4`

```rust
pub use crate::region_4_vap_liq_equilibrium::sat_pressure::sat_pressure_4;
```

#### Re-export `sat_temp_4`

```rust
pub use crate::region_4_vap_liq_equilibrium::sat_temp::sat_temp_4;
```

#### Re-export `crate::interfaces::object_oriented_programming::*`

```rust
pub use crate::interfaces::object_oriented_programming::*;
```

#### Re-export `crate::interfaces::functional_programming::hs_flash_eqm::*`

```rust
pub use crate::interfaces::functional_programming::hs_flash_eqm::*;
```

#### Re-export `crate::interfaces::functional_programming::ph_flash_eqm::*`

```rust
pub use crate::interfaces::functional_programming::ph_flash_eqm::*;
```

#### Re-export `crate::interfaces::functional_programming::ps_flash_eqm::*`

```rust
pub use crate::interfaces::functional_programming::ps_flash_eqm::*;
```

#### Re-export `crate::interfaces::functional_programming::pt_flash_eqm::*`

```rust
pub use crate::interfaces::functional_programming::pt_flash_eqm::*;
```

## Module `constants`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: \"missing_docs\", kind: Warn}])]")`

constants for the steam table calculations
Physical and reference constants for the IAPWS-IF97 steam-table
calculations: water's critical point, triple point, normal boiling
point, gas constants, and molecular constants (dipole moment,
polarisability) used by the dielectric-constant correlation. Scalar
`pub const`s carry their SI unit in the name suffix; the accessor `fn`s
return the same values as dimensioned `uom` quantities.

```rust
pub mod constants { /* ... */ }
```

### Functions

#### Function `specific_gas_constant_of_water`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns the specific gas constant of water
in proper dimensioned units using uom

```rust
pub fn specific_gas_constant_of_water() -> SpecificHeatCapacity { /* ... */ }
```

#### Function `t_crit_water`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns the dimensioned critical temperature of water

```rust
pub fn t_crit_water() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `p_crit_water`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns the dimensioned critical pressure of water

```rust
pub fn p_crit_water() -> Pressure { /* ... */ }
```

#### Function `rho_crit_water`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns the dimensioned critical density of water

```rust
pub fn rho_crit_water() -> MassDensity { /* ... */ }
```

#### Function `s_crit_water`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns the dimensioned critical specific entropy of water
(the entropy at the critical point, where s_f = s_g)

```rust
pub fn s_crit_water() -> SpecificHeatCapacity { /* ... */ }
```

#### Function `boltzmann_constant_k`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns dimensioned boltzmann_constant_k

```rust
pub fn boltzmann_constant_k() -> HeatCapacity { /* ... */ }
```

#### Function `avogadro_number_na`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns dimensioned avogadro number

```rust
pub fn avogadro_number_na() -> uom::si::Quantity<uom::si::ISQ<uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::N1, uom::typenum::Z0>, uom::si::SI<f64>, f64> { /* ... */ }
```

#### Function `permittivity_of_vacuum_eps_0`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns dimensioned electric permittivity of vacuum

```rust
pub fn permittivity_of_vacuum_eps_0() -> ElectricPermittivity { /* ... */ }
```

#### Function `molecular_dipole_moment_mu`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns molectular dipole moment

```rust
pub fn molecular_dipole_moment_mu() -> ElectricDipoleMoment { /* ... */ }
```

#### Function `water_mean_molecular_polarisability_alpha`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns dimensioned mean molecular polarisability alpha

```rust
pub fn water_mean_molecular_polarisability_alpha() -> uom::si::Quantity<uom::si::ISQ<uom::typenum::Z0, uom::typenum::N1, uom::typenum::P4, uom::typenum::P2, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64> { /* ... */ }
```

### Constants and Statics

#### Constant `R_KJ_PER_KG_KELVIN`

gas constant for water

```rust
pub const R_KJ_PER_KG_KELVIN: f64 = 0.461526;
```

#### Constant `T_C_KELVIN`

critical temp for water

```rust
pub const T_C_KELVIN: f64 = 647.096;
```

#### Constant `P_C_MPA`

critical pressure for water

```rust
pub const P_C_MPA: f64 = 22.064;
```

#### Constant `RHO_C_KG_PER_M3`

critical density for water (kg/m^3); feeds `rho_crit_water()`

```rust
pub const RHO_C_KG_PER_M3: f64 = 322.0;
```

#### Constant `S_C_KJ_PER_KG_K`

critical specific entropy for water (where s_f = s_g at the critical point)

```rust
pub const S_C_KJ_PER_KG_K: f64 = 4.412_021_482_234_76;
```

#### Constant `T_TRIPLE_PT_KELVIN`

triple pt temp for water

```rust
pub const T_TRIPLE_PT_KELVIN: f64 = 273.16;
```

#### Constant `P_TRIPLE_PT_PASCAL`

triple pt pressure for water

```rust
pub const P_TRIPLE_PT_PASCAL: f64 = 611.657;
```

#### Constant `T_NORMAL_BP_KELVIN`

boiling pt temp for water at 1 atm (normal condition)

```rust
pub const T_NORMAL_BP_KELVIN: f64 = 373.1243;
```

#### Constant `P_NORMAL_BP_MPA`

1 atmosphere, this is the pressure for normal boiling pt

```rust
pub const P_NORMAL_BP_MPA: f64 = 0.101325;
```

#### Constant `MOLAR_MASS_WATER_G_PER_GMOL`

molecular weight of water in g/gmol

```rust
pub const MOLAR_MASS_WATER_G_PER_GMOL: f64 = 18.015257;
```

#### Constant `R_M_J_PER_MOL_KELVIN`

molar gas constant (R_M) in joules/(mol kelvin)

```rust
pub const R_M_J_PER_MOL_KELVIN: f64 = 8.31451;
```

## Module `region_1_subcooled_liquid`

region 1

Temperature from 273.15 to 623.15 K
Pressure from 0 to 100 MPa

Up to the saturation line.
This I believe is subcooled liquid region
IAPWS-IF97 Region 1 (subcooled liquid / compressed water): valid for
273.15 K <= T <= 623.15 K at pressures up to 100 MPa, on the liquid side
of the saturation line. Properties are derived from the dimensionless
specific Gibbs free energy `gamma_1(tau, pi)` and its `pi`/`tau`
derivatives (see [`gamma_derivatives`]); forward `(p,T)` calls live here,
backward `(p,h)`, `(p,s)` and `(h,s)` flashes live in the
`backward_eqn_*_1` submodules.

```rust
pub mod region_1_subcooled_liquid { /* ... */ }
```

### Modules

## Module `gamma_dimensionless_specific_gibbs_free_energy`

Dimensionless specific Gibbs free energy `gamma(pi, tau)` for Region 1
(subcooled liquid), the base potential the intensive properties derive from
(dimensionless; `pi` = reduced pressure, `tau` = reduced temperature).

```rust
pub mod gamma_dimensionless_specific_gibbs_free_energy { /* ... */ }
```

### Functions

#### Function `gamma_1`

Returns the region-1 gamma
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_1(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

## Module `gamma_derivatives`

derivatives for dimensionless gibbs free energy
used to calculate specific volume, entropy,
internal energy, enthalpy,
cp, cv
speed of sound
isentropic exopnent

isobaric cubic exapnsion coeff
isothermal compressibility

```rust
pub mod gamma_derivatives { /* ... */ }
```

### Functions

#### Function `gamma_pi_1`

Returns the region-1 gamma_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_1(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_pi_1`

Returns the region-1 gamma_pi_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_pi_1(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_1`

Returns the region-1 gamma_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_1(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_tau_1`

Returns the region-1 gamma_tau_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_tau_1(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_tau_1`

Returns the region-1 gamma_pi_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_tau_1(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

## Module `intensive_properties`

intensive properties caluclated using the gamma_derivatives

these include
specific volume
specific enthalpy
specific internal energy
specific entropy
specific cp
specific cv
speed of sound
isentropic exponent (not done)
isobaric cubic expansion coeff (not done)
isothermal compressibility (not done)

```rust
pub mod intensive_properties { /* ... */ }
```

### Types

#### Type Alias `InversePressure`

Reciprocal pressure (units of 1/Pa, i.e. m·s²/kg) — the return type of the
region-1 isothermal compressibility, which `uom` has no named quantity for.

```rust
pub type InversePressure = uom::si::Quantity<uom::si::ISQ<uom::typenum::P1, uom::typenum::N1, uom::typenum::P2, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

### Functions

#### Function `h_tp_1`

Returns the region-1 specific enthalpy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn h_tp_1(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `v_tp_1`

Returns the region-1 specific volume
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn v_tp_1(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `u_tp_1`

Returns the region-1 specific internal energy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn u_tp_1(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `s_tp_1`

Returns the region-1 specific entropy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

units are same as cp

```rust
pub fn s_tp_1(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cp_tp_1`

Returns the region-1 specific isobaric heat capacity
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cp_tp_1(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cv_tp_1`

Returns the region-1 specific isochoric heat capacity
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cv_tp_1(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `w_tp_1`

Returns the region-1 speed of sound
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn w_tp_1(t: ThermodynamicTemperature, p: Pressure) -> Velocity { /* ... */ }
```

#### Function `kappa_tp_1`

Returns the region-1 isentropic exponent

```rust
pub fn kappa_tp_1(t: ThermodynamicTemperature, p: Pressure) -> Ratio { /* ... */ }
```

#### Function `alpha_v_tp_1`

Returns the region-1 isobaric cubic expansion coeff

```rust
pub fn alpha_v_tp_1(t: ThermodynamicTemperature, p: Pressure) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_tp_1`

Returns the region-1 isobaric isothermal compressibility

```rust
pub fn kappa_t_tp_1(t: ThermodynamicTemperature, p: Pressure) -> InversePressure { /* ... */ }
```

## Module `backward_eqn_ph_1`

contains code and functions for backward equations for region 1
pressure and enthalpy (p,h) flash
IAPWS-IF97 Region 1 backward equation: temperature as a function of
pressure and specific enthalpy, `T(p,h)` (Table 6).

```rust
pub mod backward_eqn_ph_1 { /* ... */ }
```

### Functions

#### Function `eta_1_back`

Returns the region-1 eta for backwards calculations
Enthalpy is assumed to be in kJ/kg

```rust
pub fn eta_1_back(h: AvailableEnergy) -> f64 { /* ... */ }
```

#### Function `pi_1_back`

Returns the region-1 pi for backwards calculations

```rust
pub fn pi_1_back(p: Pressure) -> f64 { /* ... */ }
```

#### Function `t_ph_1`

Returns the region-1 backward correlation for T(p,h)

the reference temperature is 1K

```rust
pub fn t_ph_1(p: Pressure, h: AvailableEnergy) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `theta_ph_1`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the region-1 backward correlation for theta = T/T* (p,h)

```rust
pub fn theta_ph_1(p: Pressure, h: AvailableEnergy) -> f64 { /* ... */ }
```

## Module `backward_eqn_ps_1`

contains code and functions for backwards equations
pressure and entropy (p,s) flash
IAPWS-IF97 Region 1 backward equation: temperature as a function of
pressure and specific entropy, `T(p,s)` (Table 8).

```rust
pub mod backward_eqn_ps_1 { /* ... */ }
```

### Modules

## Module `float_equations`

float equations from rust-steam
legacy and ported over

```rust
pub mod float_equations { /* ... */ }
```

### Functions

#### Function `t_ps_1`

Returns the region-1 backward-equation temperature `T(p,s)`: pressure `p`
(Pa) and specific entropy `s` (J/(kg.K)) in, `ThermodynamicTemperature`
(K) out.

```rust
pub fn t_ps_1(p: Pressure, s: SpecificHeatCapacity) -> ThermodynamicTemperature { /* ... */ }
```

## Module `backward_eqn_hs_1`

contains code and functions for backward equations for region 1
specific enthalpy and specific entropy (h,s) flash
IAPWS-IF97 Region 1 backward equation: pressure as a function of specific
enthalpy and specific entropy, `p(h,s)`.

```rust
pub mod backward_eqn_hs_1 { /* ... */ }
```

### Functions

#### Function `p_hs_1`

Returns the region-1 backward-equation pressure `p(h,s)`: specific
enthalpy `h` (J/kg) and specific entropy `s` (J/(kg.K)) in, `Pressure`
(Pa) out.

```rust
pub fn p_hs_1(h: AvailableEnergy, s: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

### Functions

#### Function `tau_1`

Returns the region-1 tau (dimensionless reduced temperature, 1386/T)
Temperature is assumed to be in K

```rust
pub fn tau_1(t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

#### Function `pi_1`

Returns the region-1 pi (dimensionless reduced pressure, p/16.53 MPa)
Pressure is assumed to be in Pa

```rust
pub fn pi_1(p: Pressure) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `REGION_1_COEFFS`

IAPWS-IF97 Table 2 coefficients (I_i, J_i, n_i) for the Region 1
dimensionless Gibbs free energy `gamma_1(pi, tau)` and its derivatives.

```rust
pub const REGION_1_COEFFS: [[f64; 3]; 34] = _;
```

#### Constant `REGION_1_BACK_COEFFS_PH`

IAPWS-IF97 Region 1 backward-equation coefficients (I_i, J_i, n_i) for
`T(p,h)` (Table 6), used by `theta_ph_1`/`t_ph_1` in [`backward_eqn_ph_1`].

```rust
pub const REGION_1_BACK_COEFFS_PH: [[f64; 3]; 20] = _;
```

### Re-exports

#### Re-export `gamma_dimensionless_specific_gibbs_free_energy::*`

```rust
pub use gamma_dimensionless_specific_gibbs_free_energy::*;
```

#### Re-export `gamma_derivatives::*`

```rust
pub use gamma_derivatives::*;
```

#### Re-export `intensive_properties::*`

```rust
pub use intensive_properties::*;
```

#### Re-export `backward_eqn_ph_1::*`

```rust
pub use backward_eqn_ph_1::*;
```

#### Re-export `backward_eqn_ps_1::*`

```rust
pub use backward_eqn_ps_1::*;
```

#### Re-export `backward_eqn_hs_1::*`

```rust
pub use backward_eqn_hs_1::*;
```

## Module `region_2_vapour`

region 2

vapour region

```rust
pub mod region_2_vapour { /* ... */ }
```

### Modules

## Module `gamma_ideal_gas_plus_derivatives`

Ideal-gas part of the dimensionless Gibbs free energy `gamma_0(pi, tau)`
and its `pi`/`tau` derivatives for Region 2 (dimensionless).

```rust
pub mod gamma_ideal_gas_plus_derivatives { /* ... */ }
```

### Functions

#### Function `gamma_2_ideal`

Returns the region-2 ideal gamma
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_2_ideal(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_2_ideal`

Returns the region-2 ideal gamma_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_2_ideal(t: ThermodynamicTemperature, _p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_tau_2_ideal`

Returns the region-2 ideal gamma_tau_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_tau_2_ideal(t: ThermodynamicTemperature, _p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_2_ideal`

Returns the region-2 ideal gamma_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_2_ideal(_t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

## Module `gamma_residual_plus_derivatives`

Residual part of the dimensionless Gibbs free energy `gamma_r(pi, tau)`
and its `pi`/`tau` derivatives for Region 2 (dimensionless).

```rust
pub mod gamma_residual_plus_derivatives { /* ... */ }
```

### Functions

#### Function `gamma_2_res`

Returns the region-2 residual gamma
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_2_res`

Returns the region-2 residual gamma_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_tau_2_res`

Returns the region-2 residual gamma_tau_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_tau_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_2_res`

Returns the region-2 residual gamma_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_pi_2_res`

Returns the region-2 residual gamma_pi_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_pi_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_tau_2_res`

Returns the region-2 residual gamma_pi_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_tau_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

## Module `dimensionless_tau_and_pi`

dimensionless temperature and pressure

```rust
pub mod dimensionless_tau_and_pi { /* ... */ }
```

### Functions

#### Function `tau_2`

Returns the region-2 tau (dimensionless temperature)
Pressure is assumed to be in Pa

```rust
pub fn tau_2(t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

#### Function `pi_2`

Returns the region-2 pi (dimensionless pressure)
Temperature is assumed to be in K

```rust
pub fn pi_2(p: Pressure) -> f64 { /* ... */ }
```

## Module `intensive_properties`

Region 2 intensive properties (specific volume m³/kg, enthalpy J/kg,
entropy and heat capacities J/(kg·K), speed of sound m/s, etc.) assembled
from the Gibbs `gamma` derivatives via `(p,T)` forward equations.

```rust
pub mod intensive_properties { /* ... */ }
```

### Functions

#### Function `v_tp_2`

Returns the region-2 specific volume
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn v_tp_2(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `h_tp_2`

Returns the region-2 enthalpy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn h_tp_2(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `u_tp_2`

Returns the region-2 internal energy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn u_tp_2(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `s_tp_2`

Returns the region-2 entropy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn s_tp_2(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cp_tp_2`

Returns the region-2 isobaric specific heat
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cp_tp_2(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cv_tp_2`

Returns the region-2 isochoric specific heat
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cv_tp_2(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `w_tp_2`

Returns the region-2 sound velocity
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn w_tp_2(t: ThermodynamicTemperature, p: Pressure) -> Velocity { /* ... */ }
```

#### Function `kappa_tp_2`

Returns the region-2 isentropic exponent

```rust
pub fn kappa_tp_2(t: ThermodynamicTemperature, p: Pressure) -> Ratio { /* ... */ }
```

#### Function `alpha_v_tp_2`

Returns the region-2 isobaric cubic expansion coeff

```rust
pub fn alpha_v_tp_2(t: ThermodynamicTemperature, p: Pressure) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_tp_2`

Returns the region-2 isobaric isothermal compressibility

```rust
pub fn kappa_t_tp_2(t: ThermodynamicTemperature, p: Pressure) -> super::InversePressure { /* ... */ }
```

## Module `metastable_region_2`

section 2.2.3.2 page 34 of 390 on pdf
page 20 according to internal numbering

```rust
pub mod metastable_region_2 { /* ... */ }
```

### Modules

## Module `metastable_ideal_gas_gamma`

metastable ideal gas correlations

```rust
pub mod metastable_ideal_gas_gamma { /* ... */ }
```

### Functions

#### Function `gamma_metastable_2_ideal`

Returns the region-2 ideal gamma
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_2_ideal(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_metastable_tau_2_ideal`

Returns the region-2 ideal gamma_metastable_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_tau_2_ideal(t: ThermodynamicTemperature, _p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_metastable_tau_tau_2_ideal`

Returns the region-2 ideal gamma_metastable_tau_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_tau_tau_2_ideal(t: ThermodynamicTemperature, _p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_metastable_pi_2_ideal`

Returns the region-2 ideal gamma_metastable_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_pi_2_ideal(_t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

## Module `metastable_residual_gamma`

metastable residual correlations

```rust
pub mod metastable_residual_gamma { /* ... */ }
```

### Functions

#### Function `gamma_metastable_2_res`

Returns the region-2 residual gamma
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_metastable_tau_2_res`

Returns the region-2 residual gamma_metastable_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_tau_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_metastable_tau_tau_2_res`

Returns the region-2 residual gamma_metastable_tau_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_tau_tau_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_metastable_pi_2_res`

Returns the region-2 residual gamma_metastable_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_pi_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_metastable_pi_pi_2_res`

Returns the region-2 residual gamma_metastable_pi_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_pi_pi_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_metastable_pi_tau_2_res`

Returns the region-2 residual gamma_metastable_pi_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_metastable_pi_tau_2_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

## Module `intensive_properties`

intensive properties in metastable region 2

```rust
pub mod intensive_properties { /* ... */ }
```

### Types

#### Type Alias `InversePressure`

Inverse pressure, 1/Pa (SI units Pa⁻¹) — the physical dimension of an
isothermal compressibility. Named alias for the raw `uom` quantity so the
return types below read clearly.

```rust
pub type InversePressure = uom::si::Quantity<uom::si::ISQ<uom::typenum::P1, uom::typenum::N1, uom::typenum::P2, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

### Functions

#### Function `v_tp_2_metastable`

Returns the region-2 specific volume
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn v_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `h_tp_2_metastable`

Returns the region-2 enthalpy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn h_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `u_tp_2_metastable`

Returns the region-2 internal energy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn u_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `s_tp_2_metastable`

Returns the region-2 entropy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn s_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cp_tp_2_metastable`

Returns the region-2 isobaric specific heat
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cp_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cv_tp_2_metastable`

Returns the region-2 isochoric specific heat
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cv_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `w_tp_2_metastable`

Returns the region-2 sound velocity
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn w_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> Velocity { /* ... */ }
```

#### Function `kappa_tp_2_metastable`

Returns the region-2 isentropic exponent

```rust
pub fn kappa_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> Ratio { /* ... */ }
```

#### Function `alpha_v_tp_2_metastable`

Returns the region-2 isobaric cubic expansion coeff

```rust
pub fn alpha_v_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_tp_2_metastable`

Isothermal compressibility kappa_T (1/Pa) of **metastable** Region 2
(supersaturated vapour), from the `(T,p)` metastable Gibbs formulation.
Temperature in K, pressure in Pa.

```rust
pub fn kappa_t_tp_2_metastable(t: ThermodynamicTemperature, p: Pressure) -> InversePressure { /* ... */ }
```

### Re-exports

#### Re-export `metastable_ideal_gas_gamma::*`

```rust
pub use metastable_ideal_gas_gamma::*;
```

#### Re-export `metastable_residual_gamma::*`

```rust
pub use metastable_residual_gamma::*;
```

#### Re-export `intensive_properties::*`

```rust
pub use intensive_properties::*;
```

## Module `backward_eqn_ph_2`

backward equations for pressure enthalpy flash

```rust
pub mod backward_eqn_ph_2 { /* ... */ }
```

### Functions

#### Function `t_ph_2`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region 2 backward `(p,h)` equation: temperature T (K) from pressure (Pa)
and specific enthalpy (J/kg). Dispatches to the 2a / 2b / 2c subregion
correlations by pressure and the 2b/2c boundary `p_2b2c(h)`.

```rust
pub fn t_ph_2(p: Pressure, h: AvailableEnergy) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `p_2b2c`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

eqn for determining pressure boundary between subregion 2b and 2c
using dimensionless enthalpy eta

```rust
pub fn p_2b2c(h: AvailableEnergy) -> Pressure { /* ... */ }
```

#### Function `h_2b2c`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

eqn for determining enthalpy boundary between subregion 2b and 2c
using dimensionless pressure pi

```rust
pub fn h_2b2c(p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `t_ph_2a`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Subregion 2a backward `(p,h)` correlation: temperature T (K) from the
dimensionless reduced pressure `pi` and reduced enthalpy `eta`
(both dimensionless). Valid for p ≤ 4 MPa.

```rust
pub fn t_ph_2a(pi: f64, eta: f64) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `t_ph_2b`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Subregion 2b backward `(p,h)` correlation: temperature T (K) from the
dimensionless reduced pressure `pi` and reduced enthalpy `eta`
(both dimensionless). Valid for p > 4 MPa below the `p_2b2c(h)` boundary.

```rust
pub fn t_ph_2b(pi: f64, eta: f64) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `t_ph_2c`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Subregion 2c backward `(p,h)` correlation: temperature T (K) from the
dimensionless reduced pressure `pi` and reduced enthalpy `eta`
(both dimensionless). Valid for p > 4 MPa above the `p_2b2c(h)` boundary.

```rust
pub fn t_ph_2c(pi: f64, eta: f64) -> ThermodynamicTemperature { /* ... */ }
```

## Module `backward_eqn_ps_2`

backward eqns for pressure entropy flash

```rust
pub mod backward_eqn_ps_2 { /* ... */ }
```

### Modules

## Module `subregion_2a`

Subregion 2a backward `(p,s)` correlation (temperature from pressure and entropy).

```rust
pub mod subregion_2a { /* ... */ }
```

## Module `subregion_2b`

Subregion 2b backward `(p,s)` correlation (temperature from pressure and entropy).

```rust
pub mod subregion_2b { /* ... */ }
```

## Module `subregion_2c`

Subregion 2c backward `(p,s)` correlation (temperature from pressure and entropy).

```rust
pub mod subregion_2c { /* ... */ }
```

### Functions

#### Function `t_ps_2`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region 2 backward `(p,s)` equation: temperature T (K) from pressure (Pa)
and specific entropy (J/(kg·K)). Dispatches to the 2a / 2b / 2c subregion
correlations by the 4 MPa (2a|2b) and 5.85 kJ/(kg·K) (2b|2c) boundaries.

```rust
pub fn t_ps_2(p: Pressure, s: SpecificHeatCapacity) -> ThermodynamicTemperature { /* ... */ }
```

## Module `backward_eqn_hs_2`

backward eqns for pressure entropy flash

```rust
pub mod backward_eqn_hs_2 { /* ... */ }
```

### Modules

## Module `boundary_eqns`

boundary equations for 2a and 2b

```rust
pub mod boundary_eqns { /* ... */ }
```

### Functions

#### Function `h_2a2b`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

eqn for determining pressure boundary between subregion 2b and 2c
using dimensionless enthalpy eta
for (h,s) points on this boundary, it belongs to subregion a

```rust
pub fn h_2a2b(s: SpecificHeatCapacity) -> AvailableEnergy { /* ... */ }
```

### Functions

#### Function `p_hs_2`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region 2 backward `(h,s)` equation: pressure p (Pa) from specific enthalpy
(J/kg) and specific entropy (J/(kg·K)). Dispatches to the 2a / 2b / 2c
subregion correlations by the 5.85 kJ/(kg·K) (2b|2c) and 4 MPa (2a|2b)
boundaries.

```rust
pub fn p_hs_2(h: AvailableEnergy, s: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

### Re-exports

#### Re-export `boundary_eqns::*`

```rust
pub use boundary_eqns::*;
```

### Re-exports

#### Re-export `gamma_ideal_gas_plus_derivatives::*`

```rust
pub use gamma_ideal_gas_plus_derivatives::*;
```

#### Re-export `gamma_residual_plus_derivatives::*`

```rust
pub use gamma_residual_plus_derivatives::*;
```

#### Re-export `dimensionless_tau_and_pi::*`

```rust
pub use dimensionless_tau_and_pi::*;
```

#### Re-export `intensive_properties::*`

```rust
pub use intensive_properties::*;
```

#### Re-export `metastable_region_2::*`

```rust
pub use metastable_region_2::*;
```

#### Re-export `backward_eqn_ph_2::*`

```rust
pub use backward_eqn_ph_2::*;
```

#### Re-export `backward_eqn_ps_2::*`

```rust
pub use backward_eqn_ps_2::*;
```

#### Re-export `backward_eqn_hs_2::*`

```rust
pub use backward_eqn_hs_2::*;
```

## Module `region_3_single_phase_plus_supercritical_steam`

region 3

single phase liquid and vapour
region, also includes supercritical region
and critical point

auxilliary equation for region 2 and 3 are also put here


IAPWS-IF97 Region 3: single-phase liquid/vapour near the critical point,
plus the supercritical region. Unlike regions 1/2, region 3 is
formulated directly on the dimensionless Helmholtz free energy
`phi(delta,tau)` in a `(rho,T)` basis (`delta` = reduced density,
`tau` = inverse reduced temperature, both dimensionless) rather than
`(p,T)`; see `phi_dimensionless_helmholtz_free_energy` and
`phi_deriviatives`. `intensive_properties` derives the forward
`(rho,T) -> {p,u,s,h,cv,cp,w,...}` properties from those derivatives.
`backward_eqn_pt_3`, `backward_eqn_ph_3`, `backward_eqn_ps_3` and
`backward_eqn_hs_3` hold the backward (inverse) equations that recover
`T` and/or `v`/`rho` from `(p,T)`, `(p,h)`, `(p,s)` and `(h,s)`
respectively, several of them split into lettered subregions (3a/3b, or
the 26 subregions a-z near the critical point for `(p,T)`).
`aux_eqn_boundary_region_2_and_region_3` holds the p23/b23 auxiliary
boundary equation that separates region 2 from region 3.

Accuracy caveat: region-3 backward equations lose precision within
~0.5 K of the critical point (Tc ~= 647.096 K, pc ~= 22.064 MPa);
prefer the forward `(rho,T)` equations there.

```rust
pub mod region_3_single_phase_plus_supercritical_steam { /* ... */ }
```

### Modules

## Module `dimensionless_tau_and_delta`

dimensionless temperature (tau)
and dimensionless density (delta)

```rust
pub mod dimensionless_tau_and_delta { /* ... */ }
```

### Functions

#### Function `delta_3`

Returns the region-3 delta
dimensionless specific density
Specific mass is assumed to be in kg/m3

```rust
pub fn delta_3(rho: MassDensity) -> f64 { /* ... */ }
```

#### Function `tau_3`

Returns the region-3 tau
dimensionless temperature
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn tau_3(t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

## Module `phi_dimensionless_helmholtz_free_energy`

dimensionless Helmholtz free energy phi(delta,tau) and its
polynomial coefficients (`REGION_3_COEFFS`) for region 3

```rust
pub mod phi_dimensionless_helmholtz_free_energy { /* ... */ }
```

### Functions

#### Function `phi_3`

Returns the region-3 phi
remember, phi is dimensionless_helmholtz_free_energy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn phi_3(rho: MassDensity, t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

## Module `phi_deriviatives`

first and second partial derivatives of phi(delta,tau) with
respect to the dimensionless density (delta) and inverse
reduced temperature (tau)

```rust
pub mod phi_deriviatives { /* ... */ }
```

### Functions

#### Function `phi_delta_3`

Returns the region-3 phi_delta
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn phi_delta_3(rho: MassDensity, t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

#### Function `phi_delta_delta_3`

Returns the region-3 phi_delta_delta
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn phi_delta_delta_3(rho: MassDensity, t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

#### Function `phi_tau_3`

Returns the region-3 phi_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn phi_tau_3(rho: MassDensity, t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

#### Function `phi_tau_tau_3`

Returns the region-3 phi_tau_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn phi_tau_tau_3(rho: MassDensity, t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

#### Function `phi_delta_tau_3`

Returns the region-3 phi_delta_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn phi_delta_tau_3(rho: MassDensity, t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

## Module `intensive_properties`

intensive properties for forward equations in region 3

```rust
pub mod intensive_properties { /* ... */ }
```

### Types

#### Type Alias `InversePressure`

Reciprocal-pressure quantity (SI unit `m s^2 / kg`, i.e. Pa^-1).
`uom` has no built-in inverse-pressure quantity, so this alias is used
as the return type for isothermal compressibility (`kappa_t_rho_t_3`,
`kappa_t_tp_3`).

```rust
pub type InversePressure = uom::si::Quantity<uom::si::ISQ<uom::typenum::P1, uom::typenum::N1, uom::typenum::P2, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

### Functions

#### Function `p_rho_t_3`

Returns the pressure given t and rho
Temperature is assumed to be in K
density is assumed to be in kg/m^3

```rust
pub fn p_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> Pressure { /* ... */ }
```

#### Function `u_rho_t_3`

Returns the internal energy given t and rho
Temperature is assumed to be in K
density is assumed to be in kg/m^3

```rust
pub fn u_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
```

#### Function `s_rho_t_3`

Returns the entropy given t and rho
Temperature is assumed to be in K
density is assumed to be in kg/m^3

```rust
pub fn s_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `h_rho_t_3`

Returns the enthalpy given t and rho
Temperature is assumed to be in K
density is assumed to be in kg/m^3

```rust
pub fn h_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
```

#### Function `cv_rho_t_3`

Returns the isochoric specific heat given t and rho
Temperature is assumed to be in K
density is assumed to be in kg/m^3

```rust
pub fn cv_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cp_rho_t_3`

Returns the isobaric specific heat given t and rho
Temperature is assumed to be in K
density is assumed to be in kg/m^3

```rust
pub fn cp_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `w_rho_t_3`

speed of sound in region 3

```rust
pub fn w_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> Velocity { /* ... */ }
```

#### Function `kappa_rho_t_3`

isentropic exponent in region 3

```rust
pub fn kappa_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

#### Function `alpha_v_rho_t_3`

Returns the region-3 isobaric cubic expansion coeff

```rust
pub fn alpha_v_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_rho_t_3`

Returns the region-3 isothermal compressibility

```rust
pub fn kappa_t_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> InversePressure { /* ... */ }
```

#### Function `alpha_p_rho_t_3`

Returns the region-3 relative pressure coefficient

```rust
pub fn alpha_p_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> TemperatureCoefficient { /* ... */ }
```

#### Function `beta_p_rho_t_3`

Returns the region-3 isothermal stress coefficient

```rust
pub fn beta_p_rho_t_3(rho: MassDensity, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
```

## Module `aux_eqn_boundary_region_2_and_region_3`

region_2_3_auxiliary_boundary

```rust
pub mod aux_eqn_boundary_region_2_and_region_3 { /* ... */ }
```

### Functions

#### Function `p_boundary_2_3`

boundary equation between region 2 and 3
note that points ON this line belong to region 2

```rust
pub fn p_boundary_2_3(t: ThermodynamicTemperature) -> Pressure { /* ... */ }
```

#### Function `t_boundary_2_3`

boundary equation between region 2 and 3
note that points ON this line belong to region 2

```rust
pub fn t_boundary_2_3(p: Pressure) -> ThermodynamicTemperature { /* ... */ }
```

## Module `backward_eqn_ph_3`

region 3 ph equations

```rust
pub mod backward_eqn_ph_3 { /* ... */ }
```

### Modules

## Module `t_ph_flash`

obtains temperature from pressure and enthalpy in region 3

```rust
pub mod t_ph_flash { /* ... */ }
```

### Functions

#### Function `t_ph_3`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region-3 backward equation: returns temperature given pressure and
specific enthalpy, `(p,h) -> T`.
Assumes `(p,h)` is already known to be in region 3; dispatches to the
3a or 3b subregion equation based on `is_3a_when_in_region_3`.

```rust
pub fn t_ph_3(p: Pressure, h: AvailableEnergy) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `t_ph_3a`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

eq 2.28

```rust
pub fn t_ph_3a(p: Pressure, h: AvailableEnergy) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `t_ph_3b`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

eq 2.29

```rust
pub fn t_ph_3b(p: Pressure, h: AvailableEnergy) -> ThermodynamicTemperature { /* ... */ }
```

## Module `v_ph_flash`

obtains specific volume from pressure and enthalpy in region 3

```rust
pub mod v_ph_flash { /* ... */ }
```

### Functions

#### Function `v_ph_3`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region-3 backward equation: returns specific volume given pressure and
specific enthalpy, `(p,h) -> v`.
Assumes `(p,h)` is already known to be in region 3; dispatches to the
3a or 3b subregion equation based on `is_3a_when_in_region_3`.

```rust
pub fn v_ph_3(p: Pressure, h: AvailableEnergy) -> SpecificVolume { /* ... */ }
```

#### Function `v_ph_3a`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region 3a backward equation: specific volume from `(p,h)`, valid on the
higher-density (liquid-like) side of the 3a/3b enthalpy boundary.

```rust
pub fn v_ph_3a(p: Pressure, h: AvailableEnergy) -> SpecificVolume { /* ... */ }
```

#### Function `v_ph_3b`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

eq 2.27

```rust
pub fn v_ph_3b(p: Pressure, h: AvailableEnergy) -> SpecificVolume { /* ... */ }
```

## Module `boundary_eqn_3a_3b`

boundary equations for 3a and 3b

```rust
pub mod boundary_eqn_3a_3b { /* ... */ }
```

### Functions

#### Function `h_3a3b_backwards_ph_boundary`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

based on eq 2.25

```rust
pub fn h_3a3b_backwards_ph_boundary(p: Pressure) -> AvailableEnergy { /* ... */ }
```

### Re-exports

#### Re-export `t_ph_flash::*`

```rust
pub use t_ph_flash::*;
```

#### Re-export `v_ph_flash::*`

```rust
pub use v_ph_flash::*;
```

#### Re-export `boundary_eqn_3a_3b::*`

```rust
pub use boundary_eqn_3a_3b::*;
```

## Module `backward_eqn_pt_3`

region 3 pt equations for volume
this enables pt flashing in this region

```rust
pub mod backward_eqn_pt_3 { /* ... */ }
```

### Modules

## Module `intensive_properties`

region-3 intensive properties (h, u, s, cp, cv, w, ...) computed from a
`(T,p)` pair by first recovering `v` via `v_tp_3`, then reusing the
`(rho,T)`-based Helmholtz functions

```rust
pub mod intensive_properties { /* ... */ }
```

### Functions

#### Function `h_tp_3`

Returns the region-3 enthalpy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn h_tp_3(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `u_tp_3`

Returns the region-3 internal energy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn u_tp_3(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `s_tp_3`

Returns the region-3 entropy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn s_tp_3(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cp_tp_3`

Returns the region-3 isobaric specific heat
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cp_tp_3(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cv_tp_3`

Returns the region-3 isochoric specific heat
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cv_tp_3(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `w_tp_3`

Returns the region-3 sound velocity
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn w_tp_3(t: ThermodynamicTemperature, p: Pressure) -> Velocity { /* ... */ }
```

#### Function `kappa_tp_3`

Returns the region-3 isentropic exponent

```rust
pub fn kappa_tp_3(t: ThermodynamicTemperature, p: Pressure) -> Ratio { /* ... */ }
```

#### Function `alpha_v_tp_3`

Returns the region-3 isobaric cubic expansion coeff

```rust
pub fn alpha_v_tp_3(t: ThermodynamicTemperature, p: Pressure) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_tp_3`

Returns the region-3 isobaric isothermal compressibility

```rust
pub fn kappa_t_tp_3(t: ThermodynamicTemperature, p: Pressure) -> crate::region_3_single_phase_plus_supercritical_steam::InversePressure { /* ... */ }
```

### Functions

#### Function `v_tp_3`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

obtains volume for region 3 based on pt flash

then using vt flash, you can get everything else

```rust
pub fn v_tp_3(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `v_tp_3c`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

these are needed for ph flashing at saturation line
liq phase specific vol
from 623.15 K to 634.659 K

```rust
pub fn v_tp_3c(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `v_tp_3s`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

 these are needed for ph flashing at saturation line
liq phase specific vol
from 634.659 K to 643.15 K

```rust
pub fn v_tp_3s(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `v_tp_3t`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

these are needed for ph flashing at saturation line
vap phase specific vol
from 623.15 K to 640.961 K

```rust
pub fn v_tp_3t(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `v_tp_3r`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

these are needed for ph flashing at saturation line
vap phase specific vol
from 640.931 K to 643.15

```rust
pub fn v_tp_3r(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `v_tp_3u`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

these are needed for ph flashing at saturation line
vap phase specific vol
from 643.15 K, 21.0434 Mpa to 21.9316 MPa


```rust
pub fn v_tp_3u(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `v_tp_3y`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

these are needed for ph flashing at saturation line
vap phase specific vol
from 21.9316 MPa to crit pt at 22.064 Mpa (crit pt)


```rust
pub fn v_tp_3y(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `v_tp_3x`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

these are needed for ph flashing at saturation line
vap phase specific vol
from 643.15 K, 21.0434 Mpa to 21.9010 MPa


```rust
pub fn v_tp_3x(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `v_tp_3z`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

these are needed for ph flashing at saturation line
vap phase specific vol
from 21.9010 MPa to crit pt at 22.064 Mpa (crit pt)


```rust
pub fn v_tp_3z(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

### Re-exports

#### Re-export `intensive_properties::*`

```rust
pub use intensive_properties::*;
```

## Module `backward_eqn_ps_3`

region 3 ps equations for volume and temperature
this enables pt flashing in this region

```rust
pub mod backward_eqn_ps_3 { /* ... */ }
```

### Modules

## Module `t_ps_flash`

region 3a/3b temperature backward equation, `(p,s) -> T`

```rust
pub mod t_ps_flash { /* ... */ }
```

## Module `v_ps_flash`

region 3a/3b specific-volume backward equation, `(p,s) -> v`

```rust
pub mod v_ps_flash { /* ... */ }
```

### Functions

#### Function `s_3a3b_backwards_ps_boundary`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

The 3a/3b subregion boundary entropy for the region-3 `(p,s)` backward
equations: `s3a3b ~= 4.412 kJ/(kg K)`. Below this value use subregion
3a, above it use subregion 3b.

```rust
pub fn s_3a3b_backwards_ps_boundary() -> SpecificHeatCapacity { /* ... */ }
```

#### Function `v_ps_3`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region-3 backward equation: returns specific volume given pressure and
specific entropy, `(p,s) -> v`.
Assumes `(p,s)` is already known to be in region 3; dispatches to the
3a or 3b subregion equation via `s_3a3b_backwards_ps_boundary`.

```rust
pub fn v_ps_3(p: Pressure, s: SpecificHeatCapacity) -> SpecificVolume { /* ... */ }
```

#### Function `t_ps_3`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region-3 backward equation: returns temperature given pressure and
specific entropy, `(p,s) -> T`.
Assumes `(p,s)` is already known to be in region 3; dispatches to the
3a or 3b subregion equation via `s_3a3b_backwards_ps_boundary`.

```rust
pub fn t_ps_3(p: Pressure, s: SpecificHeatCapacity) -> ThermodynamicTemperature { /* ... */ }
```

## Module `backward_eqn_hs_3`

region 3 hs equations for volume and temperature
this enables pt flashing in this region
Region-3 (h,s) backward equation: recovers pressure from specific
enthalpy and specific entropy, `(h,s) -> p`. This is the entry point for
an (h,s) flash in region 3 — once `p` is known, `T`/`v` follow from the
`(p,h)` or `(p,s)` backward equations in the sibling modules.

```rust
pub mod backward_eqn_hs_3 { /* ... */ }
```

### Functions

#### Function `p_hs_3`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region-3 backward equation: returns pressure given specific enthalpy
and specific entropy, `(h,s) -> p`.
Assumes `(h,s)` is already known to be in region 3; dispatches to the
3a or 3b subregion equation based on the s3a3b entropy boundary
(`s3a3b ~= 4.412 kJ/(kg K)`).

```rust
pub fn p_hs_3(h: AvailableEnergy, s: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

### Re-exports

#### Re-export `dimensionless_tau_and_delta::*`

```rust
pub use dimensionless_tau_and_delta::*;
```

#### Re-export `phi_dimensionless_helmholtz_free_energy::*`

```rust
pub use phi_dimensionless_helmholtz_free_energy::*;
```

#### Re-export `phi_deriviatives::*`

```rust
pub use phi_deriviatives::*;
```

#### Re-export `intensive_properties::*`

```rust
pub use intensive_properties::*;
```

#### Re-export `aux_eqn_boundary_region_2_and_region_3::*`

```rust
pub use aux_eqn_boundary_region_2_and_region_3::*;
```

#### Re-export `backward_eqn_ph_3::*`

```rust
pub use backward_eqn_ph_3::*;
```

#### Re-export `backward_eqn_pt_3::*`

```rust
pub use backward_eqn_pt_3::*;
```

#### Re-export `backward_eqn_ps_3::*`

```rust
pub use backward_eqn_ps_3::*;
```

#### Re-export `backward_eqn_hs_3::*`

```rust
pub use backward_eqn_hs_3::*;
```

## Module `region_4_vap_liq_equilibrium`

region 4

two phase region
where vapour liq equilibrium exists
IAPWS-IF97 Region 4 (saturation line / vapour-liquid equilibrium): the
`p_sat(T)` / `T_sat(p)` correlation valid from the triple point up to the
critical point (T_c = 647.096 K, p_c = 22.064 MPa), plus two-phase
backward equations (`T_sat(h,s)`) and equilibrium speed-of-sound helpers
used to route flashes onto or off the dome.

```rust
pub mod region_4_vap_liq_equilibrium { /* ... */ }
```

### Modules

## Module `sat_temp`

saturation temperature equation

```rust
pub mod sat_temp { /* ... */ }
```

### Functions

#### Function `sat_temp_4`

Returns the IAPWS-IF97 Region 4 saturation temperature `T_sat(p)`:
pressure `p` (Pa) in, `ThermodynamicTemperature` (K) out. Valid from the
triple point up to the critical point (p_c = 22.064 MPa).

```rust
pub fn sat_temp_4(p: Pressure) -> ThermodynamicTemperature { /* ... */ }
```

## Module `sat_pressure`

saturation pressure equation

```rust
pub mod sat_pressure { /* ... */ }
```

### Functions

#### Function `sat_pressure_4`

returns sat pressure in region 4

```rust
pub fn sat_pressure_4(t: ThermodynamicTemperature) -> Pressure { /* ... */ }
```

## Module `backward_eqn_hs_4`

backward equation T_s(h,s)

```rust
pub mod backward_eqn_hs_4 { /* ... */ }
```

### Functions

#### Function `tsat_hs_4`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the IAPWS-IF97 Region 4 backward-equation saturation temperature
`T_sat(h,s)`: specific enthalpy `h` (J/kg) and specific entropy `s`
(J/(kg.K)) in, `ThermodynamicTemperature` (K) out. Used to locate the
saturation temperature of a two-phase state directly from `(h,s)` without
first solving for pressure.

```rust
pub fn tsat_hs_4(h: AvailableEnergy, s: SpecificHeatCapacity) -> ThermodynamicTemperature { /* ... */ }
```

## Module `speed_of_sound_eqm`

two-phase (Region 4) equilibrium speed-of-sound correlations, `w(p,s)`

```rust
pub mod speed_of_sound_eqm { /* ... */ }
```

### Functions

#### Function `w_ps_eqm_region4_kieffer`

this is a more complicated version that makes use of derivatives
directly based on thermodynamic calculus

AI generated...


```rust
pub fn w_ps_eqm_region4_kieffer(p: Pressure, s: SpecificHeatCapacity) -> Velocity { /* ... */ }
```

#### Function `w_ps_eqm_region4_finite_diff_vol`

this is a simpler version that makes use of derivatives
that makes use of derivatives
AI generated

```rust
pub fn w_ps_eqm_region4_finite_diff_vol(p: Pressure, s: SpecificHeatCapacity) -> Velocity { /* ... */ }
```

### Functions

#### Function `beta_dimensionless_pressure_4`

returns dimensionless pressure
(there is an exponent to the power of 1/4)
in region 4

```rust
pub fn beta_dimensionless_pressure_4(p: Pressure) -> f64 { /* ... */ }
```

#### Function `theta_dimensionless_temp_4`

returns dimensionless temp for region 4

```rust
pub fn theta_dimensionless_temp_4(t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

### Re-exports

#### Re-export `sat_temp::*`

```rust
pub use sat_temp::*;
```

#### Re-export `sat_pressure::*`

```rust
pub use sat_pressure::*;
```

#### Re-export `backward_eqn_hs_4::*`

```rust
pub use backward_eqn_hs_4::*;
```

#### Re-export `speed_of_sound_eqm::*`

```rust
pub use speed_of_sound_eqm::*;
```

## Module `region_5_steam_at_800_plus_degc`

region 5

superheated steam region (ultra high temp)
IAPWS-IF97 Region 5 (ultra-high-temperature steam): valid for
1073.15 K <= T <= 2273.15 K (roughly 800 °C and above) at pressures up to
50 MPa. Results above ~2273 K are extrapolations beyond the IF97
specification, not validated IF97 output (callers default to returning
`OutOfRange` there). Properties are derived from the dimensionless
specific Gibbs free energy, split into an ideal-gas part
([`gamma_ideal_gas_plus_derivatives`]) and a residual part
([`gamma_residual_plus_derivatives`]) with their `pi`/`tau` derivatives.
IAPWS-IF97 publishes only forward `(p,T)` equations for Region 5 — there
are no official backward equations. This crate carries experimental,
non-IAPWS `T(p,h)` and `T(p,s)` correlations fitted against the forward
equations below, in
[`crate::backward_eqn_chebyshev_experimental::region_5_t_ph_ps`]; they are
an accelerator for what would otherwise be an iterative solve, not IAPWS
values.

```rust
pub mod region_5_steam_at_800_plus_degc { /* ... */ }
```

### Modules

## Module `dimensionless_tau_and_pi`

dimensionless reduced pressure `pi` and reduced temperature `tau` for
Region 5

```rust
pub mod dimensionless_tau_and_pi { /* ... */ }
```

### Functions

#### Function `tau_5`

Returns the region-5 tau (dimensionless reduced temperature, 1000/T)
Temperature is assumed to be in K

```rust
pub fn tau_5(t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

#### Function `pi_5`

Returns the region-5 pi (dimensionless reduced pressure, p/1 MPa)
Pressure is assumed to be in Pa

```rust
pub fn pi_5(p: Pressure) -> f64 { /* ... */ }
```

## Module `gamma_ideal_gas_plus_derivatives`

ideal-gas part of the dimensionless Gibbs free energy and its `pi`/`tau`
derivatives, for Region 5

```rust
pub mod gamma_ideal_gas_plus_derivatives { /* ... */ }
```

### Functions

#### Function `gamma_5_ideal`

Returns the region-5 ideal gamma
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_5_ideal(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_tau_5_ideal`

Returns the region-5 ideal gamma_tau_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_tau_5_ideal(t: ThermodynamicTemperature, _p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_5_ideal`

Returns the region-5 ideal gamma_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_5_ideal(_t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_5_ideal`

Returns the region-5 ideal gamma_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_5_ideal(t: ThermodynamicTemperature, _p: Pressure) -> f64 { /* ... */ }
```

## Module `gamma_residual_plus_derivatives`

residual part of the dimensionless Gibbs free energy and its `pi`/`tau`
derivatives, for Region 5

```rust
pub mod gamma_residual_plus_derivatives { /* ... */ }
```

### Functions

#### Function `gamma_5_res`

Returns the region-5 residual gamma
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_5_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_tau_5_res`

Returns the region-5 residual gamma_tau_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_tau_5_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_5_res`

Returns the region-5 residual gamma_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_5_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_pi_5_res`

Returns the region-5 residual gamma_pi_pi
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_pi_5_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_tau_5_res`

Returns the region-5 residual gamma_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_tau_5_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

#### Function `gamma_pi_tau_5_res`

Returns the region-5 residual gamma_pi_tau
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn gamma_pi_tau_5_res(t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
```

## Module `intensive_properties`

intensive properties (specific volume, enthalpy, entropy, cp, cv, speed
of sound, etc.) computed from the Region 5 gamma derivatives

```rust
pub mod intensive_properties { /* ... */ }
```

### Types

#### Type Alias `InversePressure`

Reciprocal pressure (units of 1/Pa, i.e. m·s²/kg) — the return type of the
region-5 isothermal compressibility, which `uom` has no named quantity for.

```rust
pub type InversePressure = uom::si::Quantity<uom::si::ISQ<uom::typenum::P1, uom::typenum::N1, uom::typenum::P2, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

### Functions

#### Function `v_tp_5`

Returns the region-5 specific volume
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn v_tp_5(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `h_tp_5`

Returns the region-5 enthalpy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn h_tp_5(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `u_tp_5`

Returns the region-5 internal energy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn u_tp_5(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `s_tp_5`

Returns the region-5 entropy
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn s_tp_5(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cp_tp_5`

Returns the region-5 isobaric specific heat
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cp_tp_5(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cv_tp_5`

Returns the region-5 isochoric specific heat
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn cv_tp_5(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `w_tp_5`

Returns the region-5 sound velocity
Temperature is assumed to be in K
Pressure is assumed to be in Pa

```rust
pub fn w_tp_5(t: ThermodynamicTemperature, p: Pressure) -> Velocity { /* ... */ }
```

#### Function `kappa_tp_5`

Returns the region-5 isentropic exponent

```rust
pub fn kappa_tp_5(t: ThermodynamicTemperature, p: Pressure) -> Ratio { /* ... */ }
```

#### Function `alpha_v_tp_5`

Returns the region-5 isobaric cubic expansion coeff

```rust
pub fn alpha_v_tp_5(t: ThermodynamicTemperature, p: Pressure) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_tp_5`

Returns the region-5 isobaric isothermal compressibility

```rust
pub fn kappa_t_tp_5(t: ThermodynamicTemperature, p: Pressure) -> InversePressure { /* ... */ }
```

### Re-exports

#### Re-export `dimensionless_tau_and_pi::*`

```rust
pub use dimensionless_tau_and_pi::*;
```

#### Re-export `gamma_ideal_gas_plus_derivatives::*`

```rust
pub use gamma_ideal_gas_plus_derivatives::*;
```

#### Re-export `gamma_residual_plus_derivatives::*`

```rust
pub use gamma_residual_plus_derivatives::*;
```

#### Re-export `intensive_properties::*`

```rust
pub use intensive_properties::*;
```

## Module `backward_eqn_ph_region_1_to_4`

backward equations ph boundary equations
overall equation
`(p,h)` region-dispatch / boundary equations across IAPWS-IF97 Regions
1-4. Given a pressure-enthalpy pair, the surrounding code decides which
region a state lies in — Region 1 (subcooled liquid), Region 2 (vapour),
Region 3 (near-critical/supercritical single phase), or Region 4
(vapour-liquid equilibrium / saturation) — using the boundary curves
described below, then calls that region's backward equations. This module
supplies the near-critical Region 3/4 saturation boundary (`boundary_eqn_ps3`).

```rust
pub mod backward_eqn_ph_region_1_to_4 { /* ... */ }
```

### Modules

## Module `boundary_eqn_ps3`

this is the boundary eqn ps3 for the critical temp
isotherm between region 4 and 3

```rust
pub mod boundary_eqn_ps3 { /* ... */ }
```

### Functions

#### Function `p_s3_h`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

IAPWS-IF97 backward boundary equation p_s3(h): the saturation pressure
(`Pressure`, returned in SI Pa) as a function of the saturated specific
enthalpy `h` (`AvailableEnergy`, J/kg) along the Region 3 / Region 4
(near-critical) boundary. Used by the `(p,h)` region dispatch to place a
state relative to the saturation line near the critical point.

```rust
pub fn p_s3_h(h: AvailableEnergy) -> Pressure { /* ... */ }
```

### Re-exports

#### Re-export `boundary_eqn_ps3::*`

```rust
pub use boundary_eqn_ps3::*;
```

## Module `backward_eqn_ps_region_1_to_4`

backward equations ps boundary equations
overall equation
`(p,s)` region-dispatch / boundary equations across IAPWS-IF97 Regions
1-4. Given a pressure-entropy pair, the code determines which region a
state lies in — Region 1 (subcooled liquid), Region 2 (vapour), Region 3
(near-critical/supercritical single phase), or Region 4 (vapour-liquid
equilibrium / saturation) — and calls that region's backward equations.
This module supplies the near-critical Region 3/4 saturation boundary
(`boundary_eqn_ps3`, the p_s3(s) curve).

```rust
pub mod backward_eqn_ps_region_1_to_4 { /* ... */ }
```

### Modules

## Module `boundary_eqn_ps3`

Near-critical Region 3 / Region 4 saturation boundary as a function of
specific entropy — the p_s3(s) backward equation.

```rust
pub mod boundary_eqn_ps3 { /* ... */ }
```

### Functions

#### Function `p_s3_s`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

IAPWS-IF97 backward boundary equation p_s3(s): the saturation pressure
(`Pressure`, returned in SI Pa) as a function of the saturated specific
entropy `s` (represented with the `SpecificHeatCapacity` unit, J/(kg*K))
along the Region 3 / Region 4 (near-critical) boundary. Used by the
`(p,s)` region dispatch to place a state relative to the saturation line
near the critical point.

```rust
pub fn p_s3_s(s: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

## Module `backward_eqn_hs_region_1_to_4`

backward equations hs boundary equations
overall equation
`(h,s)` region-dispatch / boundary equations across IAPWS-IF97 Regions
1-4. Given an enthalpy-entropy pair, the code determines which region a
state lies in — Region 1 (subcooled liquid), Region 2 (vapour), Region 3
(near-critical/supercritical single phase), or Region 4 (vapour-liquid
equilibrium / saturation) — and calls that region's backward equations.
The submodules supply the boundary curves used to make that decision:
the saturated-liquid (bubble) and saturated-vapour (dew) lines, and the
B13 (Region 1/3) and B23 (Region 2/3) inter-region boundaries.

```rust
pub mod backward_eqn_hs_region_1_to_4 { /* ... */ }
```

### Modules

## Module `region_1_and_3`

B13 boundary between Region 1 (subcooled liquid) and Region 3.

```rust
pub mod region_1_and_3 { /* ... */ }
```

### Functions

#### Function `hb13_s_boundary_enthalpy`

IAPWS-IF97 h_B13(s) boundary equation: the specific enthalpy
(`AvailableEnergy`, J/kg) of the B13 boundary between Region 1 (subcooled
liquid) and Region 3 (near-critical) as a function of specific entropy `s`
(`SpecificHeatCapacity` unit, J/(kg*K)). Used by the `(h,s)` region
dispatch to separate Region 1 from Region 3.

```rust
pub fn hb13_s_boundary_enthalpy(s: SpecificHeatCapacity) -> AvailableEnergy { /* ... */ }
```

## Module `region_2_and_3`

B23 boundary between Region 2 (vapour) and Region 3.

```rust
pub mod region_2_and_3 { /* ... */ }
```

### Functions

#### Function `tb23_s_boundary_enthalpy`

IAPWS-IF97 T_B23(h,s) boundary equation: the temperature
(`ThermodynamicTemperature`, K) of the B23 boundary between Region 2
(vapour) and Region 3 (near-critical) as a function of specific enthalpy
`h` (`AvailableEnergy`, J/kg) and specific entropy `s`
(`SpecificHeatCapacity` unit, J/(kg*K)). Used by the `(h,s)` region
dispatch to separate Region 2 from Region 3.

```rust
pub fn tb23_s_boundary_enthalpy(s: SpecificHeatCapacity, h: AvailableEnergy) -> ThermodynamicTemperature { /* ... */ }
```

## Module `saturated_liquid_line`

Saturated-liquid (bubble) line h'(s) boundaries for the `(h,s)` dispatch.

```rust
pub mod saturated_liquid_line { /* ... */ }
```

### Functions

#### Function `h1_prime_s_boundary_enthalpy`

this function represents the saturated liquid line
for hs flashing between region 1 and region 4

```rust
pub fn h1_prime_s_boundary_enthalpy(s: SpecificHeatCapacity) -> AvailableEnergy { /* ... */ }
```

#### Function `h3a_prime_s_boundary_enthalpy`

this function represents the saturated liquid line
for hs flashing between region 3a and region 4

```rust
pub fn h3a_prime_s_boundary_enthalpy(s: SpecificHeatCapacity) -> AvailableEnergy { /* ... */ }
```

## Module `saturated_vapour_line`

Saturated-vapour (dew) line h''(s) boundaries for the `(h,s)` dispatch.

```rust
pub mod saturated_vapour_line { /* ... */ }
```

### Functions

#### Function `h2ab_double_prime_s_boundary_enthalpy`

Saturated-vapour-line boundary enthalpy h''(s): the specific enthalpy
(`AvailableEnergy`, J/kg) on the saturated vapour (dew) line as a function
of specific entropy `s` (`SpecificHeatCapacity` unit, J/(kg*K)) for the
region 2a/2b portion, used by the `(h,s)` region dispatch.

```rust
pub fn h2ab_double_prime_s_boundary_enthalpy(s: SpecificHeatCapacity) -> AvailableEnergy { /* ... */ }
```

#### Function `h2c3b_prime_s_boundary_enthalpy`

Saturated-vapour-line boundary enthalpy: the specific enthalpy
(`AvailableEnergy`, J/kg) on the saturated vapour (dew) line as a function
of specific entropy `s` (`SpecificHeatCapacity` unit, J/(kg*K)) for the
region 2c/3b portion (near the top of the dome), used by the `(h,s)`
region dispatch.

```rust
pub fn h2c3b_prime_s_boundary_enthalpy(s: SpecificHeatCapacity) -> AvailableEnergy { /* ... */ }
```

## Module `backward_eqn_chebyshev_experimental`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {tool: clippy#0,\nlint_name: \"excessive_precision\", kind: Allow}])]")`

experimental, non-IAPWS explicit backward correlations fitted as Chebyshev
polynomials — Region 5 `T(p,h)`/`T(p,s)` (which IF97 does not publish at
all) and the near-critical Region 4 `(h,s)` flash. Kept apart from the
IAPWS-traceable equations above; see the module documentation.
Experimental explicit (non-iterative) backward correlations, fitted as
Chebyshev polynomials.

# What lives here, and why it is separate

The rest of this crate's backward equations are line-for-line
transcriptions of published IAPWS-IF97 tables. **Nothing in this module is.**
Every correlation here is an in-house fit, produced to cover a gap where
IF97 either publishes no backward equation at all (Region 5) or where this
crate previously had to fall back on an iterative solve (the near-critical
Region 4 `(h,s)` band).

They are kept in their own module tree, rather than folded into the
`region_*` modules, precisely so that the IAPWS-traceable equations stay
unambiguously IAPWS-traceable. Treat everything here as a numerical
accelerator whose authority is this crate's own forward equations — never
as an IAPWS reference value, and never as a published correlation.

# Provenance

Fitted with AI assistance and contributed as prototypes; see GitHub issue
[#34](https://github.com/theodoreOnzGit/outram-park-backend/issues/34) for
the original prototype sources, the fitting approach, and the discussion.
Per the workspace's AI-usage policy this is untrusted draft material until
a human has reviewed it: the round-trip tests in [`tests`] characterise the
measured accuracy, but no human V&V sign-off has been recorded.

# Contents

- [`region_5_t_ph_ps`] — Region 5 `T(p,h)` and `T(p,s)`. IF97 publishes no
  backward equations for Region 5 at all.
- [`region_4_near_critical_hs`] — near-critical Region 4 `p(h,s)` and
  vapour quality `x(h,s)`, for `623.15 K <= T_sat <= 647.04 K`.
- [`p_rho_h`] — pressure from density and enthalpy across Regions 1 to 5,
  for solvers that carry `(rho, h)` as their state. Includes a statistical
  region classifier for callers that have nothing else to go on; read its
  accuracy caveats before relying on it.

# Shared caveat — fit domains are hard edges

A Chebyshev polynomial diverges rapidly outside the interval it was fitted
on. Every function here documents its fit domain, and **none of them clamp
or validate their inputs**. A caller that cannot guarantee an in-domain
state must bound-check before calling; an out-of-domain call returns a
plausible-looking number that is meaningless.

```rust
pub mod backward_eqn_chebyshev_experimental { /* ... */ }
```

### Modules

## Module `region_4_near_critical_hs`

Near-critical Region 4 `(h,s)` flash: saturation pressure and vapour
quality, for `623.15 K <= T_sat <= 647.04 K`.
Experimental explicit near-critical Region 4 `(h,s)` flash: saturation
pressure `p(h,s)` and vapour quality `x(h,s)` for states close to the
critical point.

# Status — NOT an IAPWS release

IAPWS-IF97 supplementary release S04 covers `(h,s)` backward equations, but
the near-critical band handled here is where this crate previously fell
back on an iterative pressure solve. These correlations are an in-house fit
intended to replace that fallback with a direct evaluation; they are not a
normative IAPWS release and must not be cited as IAPWS values.

# Methodology

`p(h,s)` is a tensor-product Chebyshev polynomial in scaled `h` and scaled
`s`, fitted in `log(p)` and exponentiated on evaluation — so the returned
pressure is positive by construction.

Quality is then recovered without iteration: the fitted pressure is fed to
this crate's existing IAPWS Region 4 saturation-temperature backward
equation ([`sat_temp_4`]), and the saturated-liquid and saturated-vapour
enthalpies at that temperature come from two one-dimensional Chebyshev
series in scaled temperature. Quality is the usual lever rule
`x = (h - h_f) / (h_g - h_f)`.

The saturation-temperature step reuses the crate's IAPWS equation rather
than the duplicate copy carried in the original prototype, so there is one
`T_sat(p)` implementation in this crate, not two.

# Fit domain

- Saturation temperature: `623.15 K <= T_sat <= 647.04 K` (the near-critical
  band; the critical point is 647.096 K)
- Enthalpy: `1672.2 <= h <= 2560.9 kJ/kg`
- Entropy: `3.780 <= s <= 5.206 kJ/(kg K)`

# The `(h,s)` box is a bounding box, NOT the valid domain

This is the sharpest edge on these correlations, and it is easy to get
wrong. The enthalpy and entropy ranges above are the bounding box of the
fitted data. The states actually fitted occupy a **curved two-phase wedge
inside that box**, because on the saturation line `h` and `s` are strongly
coupled — most `(h,s)` pairs drawn from the box are not near-critical
two-phase states at all.

`p(h,s)` is fitted in `log(p)` with coefficients of order `1e4`, which
cancel to give a `log(p)` of order 3 on the wedge. Off the wedge that
cancellation does not happen and the exponential runs away: sampling the
bounding box uniformly on 2026-08-31 produced pressures as large as
`1e71 MPa`, while the same correlation evaluated on genuine two-phase
states reproduced IAPWS saturation pressure to a maximum relative error of
`1.14e-4`.

So a large or absurd result here does not mean the fit is broken — it
almost certainly means the input `(h,s)` pair is not a near-critical
two-phase state. **Callers must establish that before calling.** Neither
function clamps or validates its input.

# Known accuracy limitation

Accuracy degrades approaching the critical point, where `h_g - h_f` tends
to zero and the quality lever rule is ill-conditioned: a small absolute
error in the fitted `h_f`/`h_g` becomes a large relative error in `x`. This
is inherent to the formulation, not to the fit. See the crate's "Known
accuracy pitfalls" note on Region 3/4 behaviour within ~0.5 K of `T_c`.

# Verification results

Measured 2026-08-31 against the crate's IAPWS `sat_pressure_4`, over 795
two-phase states generated forwards across the fitted band (saturation
temperature 623.15–647.04 K, vapour quality 0.05–0.95):

| Statistic | Relative error in `p` |
|---|---|
| median | 1.34e-6 |
| 90th percentile | 7.96e-6 |
| 99th percentile | 5.90e-5 |
| maximum | 1.14e-4 |

The reference is IAPWS throughout, so this is a genuine comparison against
IAPWS rather than a self-consistency check. It is **not** a validation
sign-off: no human has reviewed these results, and the quality correlation
`x(h,s)` is not covered by the above — only the pressure is. See
[`super::tests`] for the methodology and to reproduce the numbers.

```rust
pub mod region_4_near_critical_hs { /* ... */ }
```

### Functions

#### Function `p_hs_4_near_critical_explicit`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Experimental near-critical Region 4 saturation pressure from enthalpy and
entropy, on bare floats.

`h_kj_kg` is specific enthalpy in kJ/kg and `s_kj_kg_k` specific entropy in
kJ/(kg K); the return value is pressure in MPa. See the module
documentation for the fit domain.

```rust
pub fn p_hs_4_near_critical_explicit(h_kj_kg: f64, s_kj_kg_k: f64) -> f64 { /* ... */ }
```

#### Function `h_f_near_critical_explicit`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Saturated-liquid enthalpy `h_f` on the near-critical branch, in kJ/kg, for
a saturation temperature `t_kelvin` in K.

Fitted over `623.15 K <= T_sat <= 647.04 K`.

```rust
pub fn h_f_near_critical_explicit(t_kelvin: f64) -> f64 { /* ... */ }
```

#### Function `h_g_near_critical_explicit`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Saturated-vapour enthalpy `h_g` on the near-critical branch, in kJ/kg, for
a saturation temperature `t_kelvin` in K.

Fitted over `623.15 K <= T_sat <= 647.04 K`.

```rust
pub fn h_g_near_critical_explicit(t_kelvin: f64) -> f64 { /* ... */ }
```

#### Function `x_hs_4_near_critical_explicit`

Experimental near-critical Region 4 vapour quality from enthalpy and
entropy, on bare floats.

`h_kj_kg` is specific enthalpy in kJ/kg and `s_kj_kg_k` specific entropy in
kJ/(kg K); the return value is the dimensionless vapour mass fraction.

Accuracy degrades near the critical point, where `h_g - h_f` tends to zero
— see the module documentation.

```rust
pub fn x_hs_4_near_critical_explicit(h_kj_kg: f64, s_kj_kg_k: f64) -> f64 { /* ... */ }
```

#### Function `p_hs_4_near_critical`

Experimental near-critical Region 4 saturation pressure from specific
enthalpy and specific entropy, `p(h,s)`.

This is the dimensioned entry point; it wraps
[`p_hs_4_near_critical_explicit`]. Valid for the near-critical band
`623.15 K <= T_sat <= 647.04 K` — see the module documentation.

```rust
pub fn p_hs_4_near_critical(h: AvailableEnergy, s: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

#### Function `x_hs_4_near_critical`

Experimental near-critical Region 4 vapour quality from specific enthalpy
and specific entropy, `x(h,s)`.

This is the dimensioned entry point; it wraps
[`x_hs_4_near_critical_explicit`]. Returns the dimensionless vapour mass
fraction. Accuracy degrades near the critical point — see the module
documentation.

```rust
pub fn x_hs_4_near_critical(h: AvailableEnergy, s: SpecificHeatCapacity) -> Ratio { /* ... */ }
```

## Module `p_rho_h`

Pressure from density and enthalpy, `p(rho,h)`, across Regions 1 to 5,
plus the statistical region classifier that a `(rho,h)`-only caller needs.
Experimental explicit pressure-from-density-and-enthalpy correlations,
`p(rho,h)`, spanning IAPWS-IF97 Regions 1 to 5.

# Status — NOT an IAPWS release

IAPWS-IF97 publishes no `(rho,h)` backward equations. These are in-house
fits against this crate's own forward equations, useful as an inverse-flash
building block for solvers that carry density and enthalpy as their state
variables (mass- and energy-conserving finite-volume schemes, for example).
Do not cite them as IAPWS values.

# Structure: two independent partitions

Getting a pressure out of `(rho, h)` needs two separate decisions, and they
must not be confused:

1. **Which thermodynamic region** the state is in (IF97 Region 1 to 5).
   This is a physical question, answered by [`RhoHRegion`].
2. **Which fitted polynomial surface** to evaluate ([`RhoHFitPiece`]).
   This is a purely numerical question: each region's fit is split into
   several local surfaces so that no single polynomial has to cover too
   wide a range. **These fit-piece boundaries are not thermodynamic
   boundaries** — they carry no physical meaning and must never be quoted
   as saturation, quality or region boundaries.

# Choosing a region: prefer the caller's own knowledge

If the calling code already knows the region — which it usually does, since
it generally knows pressure or temperature from the surrounding solve —
pass it in via [`p_rho_h_in_region`] and the classifier is bypassed
entirely. That is the accurate path, and it is the recommended one.

[`rho_h_region_candidate`] exists for the genuinely `(rho,h)`-only case,
where no pressure or temperature is available yet. **It is a statistical
surrogate, not a boundary equation**, with roughly 97.3% overall accuracy on
its derivation sample (Region 1 ~98.5%, Region 2 ~98.7%, Region 3 ~99.8%,
**Region 4 ~91.6%**, Region 5 ~96.3%). A misclassification is not a small
numerical error — it selects a polynomial fitted for different physics and
returns a badly wrong pressure. Region 4 is both the weakest case and the
one a two-phase solver spends most of its time in.

[`rho_h_region_scores`] returns the raw discriminant scores, whose top-two
margin is a cheap ambiguity signal: a narrow margin is the natural trigger
for verifying the choice against reconstructed pressure and this crate's
IAPWS saturation machinery.

# Fit domains are hard edges

As elsewhere in this module tree, each surface is a Chebyshev polynomial
that diverges outside the interval it was fitted on, and nothing here
clamps or validates its input.

# Verification results, and two real limitations

Measured 2026-08-31 over 3600 single-phase states generated from the
crate's own forward equations, with the region supplied (classifier not in
the loop). Relative error in the recovered pressure:

| Region | n | median | 90th pct | max |
|---|---|---|---|---|
| 1 | 299 | 5.62e-4 | 7.58e-2 | **3.19** |
| 2 | 1185 | 9.79e-6 | 2.26e-5 | 1.55e-4 |
| 3 | 16 | 1.08e-4 | 3.25e-4 | 3.69e-4 |
| 5 | 2100 | 2.15e-5 | 5.25e-5 | 1.31e-3 |

Region 4 is covered separately, over the two-phase dome, by a `(T, p, x)`
sweep from the bubble point to the dew point — 1050 states over
280–645 K: median 2.20e-4, 90th percentile 1.34e-3, 99th 4.73e-3,
maximum 1.77e-1.

# The one real limitation: low pressure in the liquid-like corner

`p(rho,h)` is **ill-conditioned wherever density stops responding to
pressure**, which in practice means cold liquid at low pressure. Liquid
water is very nearly incompressible, so along a low-pressure isotherm the
density barely moves while pressure changes by orders of magnitude;
inverting that mapping amplifies any error enormously. This is a property
of the state variables, not a defect in the fit — **no better fit can
remove it.**

It is strongly pressure-dependent, so the blunt advice "do not use this in
liquid" would be wrong. Region 1, by pressure decade (measured 2026-08-31,
relative error as a percentage):

| Pressure | n | median | 90th pct | max |
|---|---|---|---|---|
| 1e-3 – 1e-2 MPa | 15 | 88.2% | 220.7% | 318.5% |
| 1e-2 – 1e-1 MPa | 32 | 6.77% | 20.3% | 29.4% |
| 1e-1 – 1 MPa | 54 | 0.699% | 1.83% | 2.7% |
| 1 – 10 MPa | 100 | 0.054% | 0.173% | 0.3% |
| 10 – 100 MPa | 98 | 0.0072% | 0.017% | ~0.0% |

So: **above ~1 MPa Region 1 is usable** (worst 0.3%, and 0.017% above
10 MPa, which covers ordinary power-cycle liquid conditions). **Below
~0.1 MPa it is not** — errors reach several hundred percent. Region 4's
own worst case sits in the same corner (the 280 K bubble point, ~1e-3 MPa),
for the same reason.

If a caller must work in that corner, carry pressure as a state variable
rather than recovering it from `(rho, h)`.

The classifier itself was measured on the same sweep at **97.5% agreement**
with the crate's forward dispatcher over single-phase states (Region 1
98.3%, Region 2 99.3%, Region 3 100%, Region 5 96.4%), consistent with its
self-reported ~97.3%. Region 4 is absent from that measurement.

```rust
pub mod p_rho_h { /* ... */ }
```

### Types

#### Enum `RhoHRegion`

IAPWS-IF97 thermodynamic region of a `(rho,h)` state.

This is a physical classification. It is distinct from [`RhoHFitPiece`],
which only selects a fitted polynomial surface.

```rust
pub enum RhoHRegion {
    Region1,
    Region2,
    Region3,
    Region4,
    Region5,
}
```

##### Variants

###### `Region1`

Region 1 — subcooled liquid.

###### `Region2`

Region 2 — vapour / superheated steam.

###### `Region3`

Region 3 — near-critical single phase and supercritical fluid.

###### `Region4`

Region 4 — vapour-liquid equilibrium (the saturation line).

###### `Region5`

Region 5 — ultra-high-temperature steam.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Enum `RhoHFitPiece`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: \"missing_docs\", kind: Allow}])]")`

Identifies one fitted local polynomial surface.

**These are numerical fit partitions, not thermodynamic boundaries.** The
suffix indexes the local surface within a region's fit; it carries no
physical meaning.

```rust
pub enum RhoHFitPiece {
    R1_0,
    R1_1,
    R1_2,
    R2_0,
    R2_1,
    R2_2,
    R2_3,
    R2_4,
    R2_5,
    R3_0,
    R3_1,
    R3_2,
    R4_0,
    R4_1,
    R4_2,
    R4_3,
    R4_4,
    R4_5,
    R4_6,
    R4_7,
    R4_8,
    R4_9,
    R4_10,
    R5,
}
```

##### Variants

###### `R1_0`

###### `R1_1`

###### `R1_2`

###### `R2_0`

###### `R2_1`

###### `R2_2`

###### `R2_3`

###### `R2_4`

###### `R2_5`

###### `R3_0`

###### `R3_1`

###### `R3_2`

###### `R4_0`

###### `R4_1`

###### `R4_2`

###### `R4_3`

###### `R4_4`

###### `R4_5`

###### `R4_6`

###### `R4_7`

###### `R4_8`

###### `R4_9`

###### `R4_10`

###### `R5`

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

#### Function `region_1_piece_from_h`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Selects the Region 1 fit surface from specific enthalpy in kJ/kg.

```rust
pub fn region_1_piece_from_h(h_kj_kg: f64) -> RhoHFitPiece { /* ... */ }
```

#### Function `region_2_piece_from_rho`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Selects the Region 2 fit surface from density in kg/m3.

```rust
pub fn region_2_piece_from_rho(rho_kg_m3: f64) -> RhoHFitPiece { /* ... */ }
```

#### Function `region_3_piece_from_rho`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Selects the Region 3 fit surface from density in kg/m3.

```rust
pub fn region_3_piece_from_rho(rho_kg_m3: f64) -> RhoHFitPiece { /* ... */ }
```

#### Function `region_4_piece_from_rho`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Selects the Region 4 fit surface from density in kg/m3.

```rust
pub fn region_4_piece_from_rho(rho_kg_m3: f64) -> RhoHFitPiece { /* ... */ }
```

#### Function `fit_piece_for_region`

Selects the fit surface for a state whose thermodynamic region is known.

This is the accurate path: it uses the caller's own region knowledge rather
than the statistical classifier.

```rust
pub fn fit_piece_for_region(region: RhoHRegion, rho_kg_m3: f64, h_kj_kg: f64) -> RhoHFitPiece { /* ... */ }
```

#### Function `rho_h_region_candidate`

Candidate IF97 region for a `(rho,h)` state, from the statistical
classifier.

`rho_kg_m3` is density in kg/m3 and `h_kj_kg` specific enthalpy in kJ/kg.

**This is a numerical surrogate, not a boundary equation.** Accuracy on the
derivation sample is roughly 97.3% overall, and only ~91.6% for Region 4.
Prefer [`fit_piece_for_region`] with a region the caller already knows;
reach for this only when nothing but `(rho,h)` is available. Check
[`rho_h_region_scores`] for the top-two margin when the answer matters.

# Panics

Panics if `rho_kg_m3` is not strictly positive, since the classifier works
in `log10(rho)`.

```rust
pub fn rho_h_region_candidate(rho_kg_m3: f64, h_kj_kg: f64) -> RhoHRegion { /* ... */ }
```

#### Function `rho_h_region_scores`

Raw discriminant scores for each region, ordered Region 1 to Region 5.

The largest score is the classifier's choice. The margin between the two
largest is an ambiguity signal: when it is small, verify the region against
reconstructed pressure and this crate's IAPWS saturation machinery rather
than trusting the classification.

# Panics

Panics if `rho_kg_m3` is not strictly positive.

```rust
pub fn rho_h_region_scores(rho_kg_m3: f64, h_kj_kg: f64) -> [f64; 5] { /* ... */ }
```

#### Function `p_rho_h_piece_explicit`

Evaluates the fitted `p(rho,h)` surface identified by `piece`.

`rho_kg_m3` is density in kg/m3 and `h_kj_kg` specific enthalpy in kJ/kg;
the return value is pressure in MPa. The caller is responsible for having
selected an appropriate `piece` — see [`fit_piece_for_region`].

```rust
pub fn p_rho_h_piece_explicit(piece: RhoHFitPiece, rho_kg_m3: f64, h_kj_kg: f64) -> f64 { /* ... */ }
```

#### Function `p_rho_h_in_region_explicit`

Pressure in MPa from density in kg/m3 and specific enthalpy in kJ/kg, for a
state whose thermodynamic region the caller already knows.

This is the accurate path — it does not use the statistical classifier.

```rust
pub fn p_rho_h_in_region_explicit(region: RhoHRegion, rho_kg_m3: f64, h_kj_kg: f64) -> f64 { /* ... */ }
```

#### Function `p_rho_h_in_region`

Pressure from density and specific enthalpy, for a state whose
thermodynamic region the caller already knows.

This is the dimensioned entry point and the recommended one: supplying the
region bypasses the statistical classifier entirely.

```rust
pub fn p_rho_h_in_region(region: RhoHRegion, rho: MassDensity, h: AvailableEnergy) -> Pressure { /* ... */ }
```

#### Function `p_rho_h_classified`

Pressure from density and specific enthalpy, classifying the region with
the statistical surrogate.

Use this only when nothing but `(rho,h)` is available. Where the region is
known, [`p_rho_h_in_region`] is both faster and accurate. See the module
documentation for the classifier's error rates — a misclassification
produces a badly wrong pressure, not a slightly wrong one.

```rust
pub fn p_rho_h_classified(rho: MassDensity, h: AvailableEnergy) -> Pressure { /* ... */ }
```

## Module `region_5_t_ph_ps`

Region 5 backward correlations `T(p,h)` and `T(p,s)`, covering a gap where
IAPWS-IF97 publishes no backward equations.
Experimental explicit backward correlations for IAPWS-IF97 Region 5:
temperature from pressure and enthalpy, `T(p,h)`, and from pressure and
entropy, `T(p,s)`.

# Status — NOT an IAPWS release

IAPWS-IF97 publishes **no** backward equations for Region 5; the only
official route from `(p,h)` or `(p,s)` to temperature there is to iterate
on the forward equations. These correlations are an in-house fit to this
crate's own Region 5 forward Gibbs formulation, so they are an accelerator
for that iteration, not a normative IAPWS supplementary release. Do not
cite them as IAPWS values.

# Methodology

Each correlation is a tensor-product Chebyshev polynomial in
`x = scaled(log10(p/MPa))` and `y = scaled(h)` or `scaled(s)`. Evaluation
is a direct polynomial sum — there is no root finding and no iteration, so
cost is fixed and the result is a pure function of its inputs.

Coefficients were fitted against states generated by this crate's Region 5
forward equations ([`h_tp_5`](crate::region_5_steam_at_800_plus_degc::h_tp_5)
and [`s_tp_5`](crate::region_5_steam_at_800_plus_degc::s_tp_5)) over the fit
domain below.

# Fit domain

- Pressure: `1e-4 MPa <= p <= 50 MPa`
- Temperature: `1073.15 K <= T <= 2273.15 K`

Enthalpy and entropy inputs are correspondingly bounded by the forward
equations over that box — roughly `3940 <= h <= 7377 kJ/kg` and
`6.54 <= s <= 14.72 kJ/(kg K)`. **Outside the fit domain the Chebyshev
polynomial is an unbounded extrapolation and the result is meaningless.**
Neither function clamps or validates its input; callers that cannot
guarantee an in-domain state should bound-check first.

# Verification results

Round-trip accuracy against the Region 5 forward equations is measured by
the tests in [`super::tests`]; see those tests for the measured maximum and
RMS temperature deviation and the date they were taken. No comparison
against published IAPWS backward-equation reference values is possible,
because no such published backward equations exist for Region 5.

```rust
pub mod region_5_t_ph_ps { /* ... */ }
```

### Functions

#### Function `t_ph_5_explicit`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region 5 experimental backward correlation `T(p,h)`, on bare floats.

`p_mpa` is pressure in MPa and `h_kj_kg` specific enthalpy in kJ/kg; the
return value is temperature in K. See the module documentation for the fit
domain and its limitations.

# Panics

Panics if `p_mpa` is not strictly positive, since the correlation is a
polynomial in `log10(p)`.

```rust
pub fn t_ph_5_explicit(p_mpa: f64, h_kj_kg: f64) -> f64 { /* ... */ }
```

#### Function `t_ps_5_explicit`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Region 5 experimental backward correlation `T(p,s)`, on bare floats.

`p_mpa` is pressure in MPa and `s_kj_kg_k` specific entropy in kJ/(kg K);
the return value is temperature in K. See the module documentation for the
fit domain and its limitations.

# Panics

Panics if `p_mpa` is not strictly positive, since the correlation is a
polynomial in `log10(p)`.

```rust
pub fn t_ps_5_explicit(p_mpa: f64, s_kj_kg_k: f64) -> f64 { /* ... */ }
```

#### Function `t_ph_5`

Region 5 experimental backward correlation for temperature from pressure
and specific enthalpy, `T(p,h)`.

This is the dimensioned entry point; it wraps [`t_ph_5_explicit`].

Valid for `1e-4 MPa <= p <= 50 MPa` and enthalpies corresponding to
`1073.15 K <= T <= 2273.15 K`. Outside that box the result is a diverging
extrapolation — see the module documentation.

# Panics

Panics if `p` is not strictly positive.

```rust
pub fn t_ph_5(p: Pressure, h: AvailableEnergy) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `t_ps_5`

Region 5 experimental backward correlation for temperature from pressure
and specific entropy, `T(p,s)`.

This is the dimensioned entry point; it wraps [`t_ps_5_explicit`].

Valid for `1e-4 MPa <= p <= 50 MPa` and entropies corresponding to
`1073.15 K <= T <= 2273.15 K`. Outside that box the result is a diverging
extrapolation — see the module documentation.

# Panics

Panics if `p` is not strictly positive.

```rust
pub fn t_ps_5(p: Pressure, s: SpecificHeatCapacity) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `p_hs_5`

Region 5 `(h,s)` flash: the pressure at which the two backward correlations
above agree on temperature.

# Why this composition, rather than a new fit

IAPWS-IF97 publishes no `(h,s)` backward equation for Region 5 — as it
publishes none for `(p,h)` or `(p,s)` there. But a third correlation is not
needed, because the two that already exist over-determine the state: a
Region 5 point has one temperature, so the physical pressure is the one
where `T(p,h)` and `T(p,s)` return the same value.

```text
g(p) = t_ph_5(p, h) - t_ps_5(p, s) = 0
```

That is a **one-dimensional** root find whose residual costs two explicit
polynomial evaluations and no iteration of its own — far cheaper than the
two-dimensional solve over the forward Gibbs equations that the absence of a
backward equation would otherwise force, and it introduces no fit error
beyond what the two correlations already carry.

# Why it is well posed

Region 5 steam is nearly ideal, so at fixed enthalpy the temperature barely
moves with pressure (`h` is almost a function of `T` alone) while at fixed
entropy it rises with pressure roughly as `T ~ p^(R/c_p)` (since
`s ~ c_p ln T - R ln p`). The difference is therefore monotone decreasing in
`p`, and a bracketed search over the fit domain cannot land on a spurious
root.

# Valid range

The `T(p,h)` / `T(p,s)` fit domain: `1e-4 MPa <= p <= 50 MPa` and
`1073.15 K <= T <= 2273.15 K`. Inputs that do not intersect it have no
Region 5 solution.

# Status

**Not an IAPWS value.** This inherits the provenance of the two
correlations it composes: an in-house fit to this crate's own Region 5
forward equations, reviewed by no human. See the module documentation.

# Panics

Panics when no pressure in the fit domain makes the two correlations agree,
which means the `(h,s)` pair is not a Region 5 state.

```rust
pub fn p_hs_5(h: AvailableEnergy, s: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

#### Function `is_region_5_hs`

Returns `true` when an `(h,s)` pair has a Region 5 solution, without
panicking.

The non-panicking counterpart to [`p_hs_5`], for a dispatcher that must
decide whether Region 5 is the right arm before committing to it.

```rust
pub fn is_region_5_hs(h: AvailableEnergy, s: SpecificHeatCapacity) -> bool { /* ... */ }
```

### Constants and Statics

#### Constant `REGION_5_BACK_P_MIN_MPA`

Lowest pressure of the `T(p,h)` / `T(p,s)` fit domain, 1e-4 MPa.

```rust
pub const REGION_5_BACK_P_MIN_MPA: f64 = 1.0e-4;
```

#### Constant `REGION_5_BACK_P_MAX_MPA`

Highest pressure of the `T(p,h)` / `T(p,s)` fit domain, 50 MPa.

```rust
pub const REGION_5_BACK_P_MAX_MPA: f64 = 50.0;
```

#### Constant `REGION_5_BACK_T_MIN_KELVIN`

Lowest temperature of the `T(p,h)` / `T(p,s)` fit domain, 1073.15 K.

```rust
pub const REGION_5_BACK_T_MIN_KELVIN: f64 = 1073.15;
```

#### Constant `REGION_5_BACK_T_MAX_KELVIN`

Highest temperature of the `T(p,h)` / `T(p,s)` fit domain, 2273.15 K.

```rust
pub const REGION_5_BACK_T_MAX_KELVIN: f64 = 2273.15;
```

### Re-exports

#### Re-export `region_4_near_critical_hs::*`

```rust
pub use region_4_near_critical_hs::*;
```

#### Re-export `p_rho_h::*`

```rust
pub use p_rho_h::*;
```

#### Re-export `region_5_t_ph_ps::*`

```rust
pub use region_5_t_ph_ps::*;
```

## Module `dynamic_viscosity`

dynamic viscosity calcs
IAPWS R12-08 dynamic viscosity of water/steam, mu, in pascal-seconds
(Pa*s, uom `DynamicViscosity`). The viscosity is built from a
dilute-gas/background factor (`psi_0`) times a density-and-temperature
residual factor (`psi_1`), evaluated at the density (`MassDensity`,
kg/m^3) and temperature (`ThermodynamicTemperature`, K) of the state;
the critical-enhancement term is omitted (fast path). Public entry
points flash the state from `(T, p)`, `(rho, T)`, or `(p, h)` first.

```rust
pub mod dynamic_viscosity { /* ... */ }
```

### Functions

#### Function `mu_tp_eqm_two_phase`

Dynamic viscosity mu (Pa*s) of a two-phase equilibrium mixture at
temperature `t` (K), pressure `p` (Pa) and quality `x` (dimensionless),
using the HEM-mixture specific volume to get the mixture density.

```rust
pub fn mu_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> DynamicViscosity { /* ... */ }
```

#### Function `mu_tp_eqm_single_phase`

for viscosity estimates in single phase region

```rust
pub fn mu_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> DynamicViscosity { /* ... */ }
```

#### Function `mu_rho_t_eqm`

for viscosity estimates in two phase region
and single phase region

```rust
pub fn mu_rho_t_eqm(t: ThermodynamicTemperature, rho: MassDensity) -> DynamicViscosity { /* ... */ }
```

#### Function `mu_ph_eqm`

for viscosity estimates in two phase region
and single phase region
using enthalpy and pressure

```rust
pub fn mu_ph_eqm(p: Pressure, h: AvailableEnergy) -> DynamicViscosity { /* ... */ }
```

## Module `thermal_conductivity`

thermal conductivity calcs
IAPWS R15-11 thermal conductivity of water/steam, lambda, in watts per
metre-kelvin (W/(m*K), uom `ThermalConductivity`), as a function of the
state's density (`MassDensity`, kg/m^3) and temperature
(`ThermodynamicTemperature`, K). The conductivity is assembled from a
dilute-gas term (`lambda_0`), a density-dependent residual term
(`lambda_1`), and a near-critical enhancement term (`lambda_2`). Public
entry points flash the density from a `(T, p)` or `(T, p, x)` state.

```rust
pub mod thermal_conductivity { /* ... */ }
```

### Functions

#### Function `lambda_tp_eqm_single_phase`

Thermal conductivity lambda (W/(m*K)) of a single-phase state at
temperature `t` (K) and pressure `p` (Pa) per IAPWS R15-11, summing the
dilute-gas, residual, and critical-enhancement contributions.

```rust
pub fn lambda_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> ThermalConductivity { /* ... */ }
```

#### Function `lambda_tp_eqm_two_phase`

Thermal conductivity lambda (W/(m*K)) of a two-phase equilibrium mixture
at temperature `t` (K), pressure `p` (Pa) and quality `x` (dimensionless)
per IAPWS R15-11, using the HEM-mixture density and a quality-weighted
critical-enhancement estimate.

```rust
pub fn lambda_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> ThermalConductivity { /* ... */ }
```

## Module `interfaces`

public facing interfaces where the user
simply inputs pressure and temperature
or pressure and enthalpy etc
and gets all the required data automatically

the logic for splitting between regions is
mostly here

```rust
pub mod interfaces { /* ... */ }
```

### Modules

## Module `functional_programming`

this set of interfaces allows the user
to interact using a more functional programming
style (no objects)

this keeps things simple.

```rust
pub mod functional_programming { /* ... */ }
```

### Modules

## Module `pt_flash_eqm`

allows for pressure and temperature flash
for all other properties
(except steam quality, which cannot be
determined via pt flashing)

this uses the forward equations

water/steam is assumed at thermodynamic equilibrium,
ie not metastable

```rust
pub mod pt_flash_eqm { /* ... */ }
```

### Modules

## Module `multiphase_flashing`

Two-phase (Region 4) `(T,p)` dispatch helpers: mixture property evaluation
that carries steam quality, for states the single-phase `(T,p)` flashes
cannot represent on their own.

```rust
pub mod multiphase_flashing { /* ... */ }
```

### Functions

#### Function `region_fwd_eqn_two_phase`

Determines which IAPWS-IF97 forward-equation region a `(T,p,x)` point
belongs to, where temperature `T` is in K, pressure `p` is in Pa, and `x`
is the steam quality (vapour mass fraction, clamped to `[0,1]`).

This is the two-phase-aware counterpart of `region_fwd_eqn_single_phase`:
it additionally recognises when `(T,p)` sits on the saturation line with
`0 < x < 1` (Region 4), or exactly at the bubble/dew point (`x == 0` or
`x == 1`), routing those points to Region 1/2 below 623.15 K or Region 3
above it. Points above the critical temperature or pressure, or off the
saturation line, fall back to `region_fwd_eqn_single_phase`.

```rust
pub fn region_fwd_eqn_two_phase(t: ThermodynamicTemperature, p: Pressure, steam_quality: f64) -> FwdEqnRegion { /* ... */ }
```

#### Function `h_tp_eqm_two_phase`

Returns the specific enthalpy in J/kg given a `(T,p,x)` forward flash,
where temperature `T` is in K, pressure `p` is in Pa, and `x` is the
steam quality (vapour mass fraction). Unlike `h_tp_eqm_single_phase`,
this variant handles two-phase (Region 4) points and the Region 3/4
boundary near the critical point by weighting the liquid/vapour enthalpy
by `x`.

```rust
pub fn h_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> AvailableEnergy { /* ... */ }
```

#### Function `u_tp_eqm_two_phase`

returns the internal energy given temperature and pressure

```rust
pub fn u_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> AvailableEnergy { /* ... */ }
```

#### Function `s_tp_eqm_two_phase`

returns the specific entropy given temperature and pressure

```rust
pub fn s_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cp_tp_eqm_two_phase`

returns the isobaric (const pressure) heat capacitygiven temperature and pressure

```rust
pub fn cp_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cv_tp_eqm_two_phase`

returns the isochoric (const vol) heat capacity given temperature and pressure

```rust
pub fn cv_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `v_tp_eqm_two_phase`

returns the specific volume given temperature and pressure

```rust
pub fn v_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> SpecificVolume { /* ... */ }
```

#### Function `w_tp_eqm_two_phase`

returns the speed of sound given temperature and pressure

note, for region 4, this is estimated using weighted average of
liquid and vapour phases (ACCURACY NOT GUARANTEED)

```rust
pub fn w_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> Velocity { /* ... */ }
```

#### Function `kappa_tp_eqm_two_phase`

returns the isentropic exponent

```rust
pub fn kappa_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> Ratio { /* ... */ }
```

#### Function `alpha_v_tp_eqm_two_phase`

returns the isobaric cubic expansion coefficient

```rust
pub fn alpha_v_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_tp_eqm`

returns the isothermal compressibility

```rust
pub fn kappa_t_tp_eqm(t: ThermodynamicTemperature, p: Pressure, x: f64) -> crate::region_1_subcooled_liquid::InversePressure { /* ... */ }
```

### Re-exports

#### Re-export `mu_tp_eqm_two_phase`

viscosity function import

```rust
pub use crate::dynamic_viscosity::mu_tp_eqm_two_phase;
```

#### Re-export `lambda_tp_eqm_two_phase`

thermal conductivity function import

```rust
pub use crate::thermal_conductivity::lambda_tp_eqm_two_phase;
```

### Types

#### Enum `FwdEqnRegion`

an enum to help represent the appropriate
regions in the forward equations

```rust
pub enum FwdEqnRegion {
    Region1,
    Region2,
    Region3,
    Region4,
    Region5,
}
```

##### Variants

###### `Region1`

this is from T = 273.15 K to T=623.15K
liquid

###### `Region2`

this is vapour then line p23/t23
all the way up to 1073.15 K (800 degC)


###### `Region3`

this is supercritical region and
single phase liquid  / vapour near
supercritical region

###### `Region4`

two phase vapour liq equilibrium
region up to supercritical region
(saturation line, but not including the line itself)

###### `Region5`

ultra high temperature steam  (more than 800 degC)
1073.15 K to 2273.15 K
pressure from triple pt pressure to 500 bar

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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

  - ```rust
    fn into(self: Self) -> FwdEqnRegion { /* ... */ }
    ```

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

#### Function `region_fwd_eqn_single_phase`

Determines which region of the pT chart
a point belongs to.

Temperature is assumed to be in K
Pressure is assumed to be in Pa

# Validity envelope (and which edges are inclusive)

IAPWS-IF97 is defined for `273.15 K <= T <= 1073.15 K` at
`0 < p <= 100 MPa` (Regions 1-4), extended to
`1073.15 K <= T <= 2273.15 K` at `0 < p <= 50 MPa` (Region 5). **Both
pressure ceilings are inclusive** — `p = 100 MPa` exactly is a valid
IF97 state at every temperature up to 1073.15 K, and `p = 50 MPa`
exactly is valid in Region 5. Corroborated inside this crate by the
IAPWS-published backward-equation verification points at exactly
100 MPa in Region 3, i.e. at temperatures well above 623.15 K:
`t_ph_3a(100 MPa, 2100 kJ/kg) = 733.6163014 K` and
`v_ph_3a(100 MPa, 2100 kJ/kg) = 1.676229776e-3 m^3/kg`
(see `region_3_.../tests/region_3_backward_t_ph.rs::t3a_ph_test3` and
`.../region_3_backward_v_ph.rs::v3a_ph_test3`), and by
`is_outside_pressure_range` in the `(p,h)` validity check, which
rejects only `p > 100 MPa`.

The match arms below therefore all close their 100 MPa edge with
`..=100e6`. Before 2026-08-11 the Region-2 and Region-3 arms used a
half-open `..100e6`, so exactly 100 MPa above 623.15 K matched no arm
and fell through to the `panic!` (bead `op-cv1c`); that also made every
`(p,h)` call at exactly 100 MPa panic, because
`is_above_isotherm_t_1073_15` evaluates `h_tp_eqm_single_phase` on the
1073.15 K isotherm at that pressure.

# Panics

Panics if the `(T,p)` point lies outside the envelope above. Callers
that need a `Result` instead should use
[`crate::interfaces::checked`].

```rust
pub fn region_fwd_eqn_single_phase(t: ThermodynamicTemperature, p: Pressure) -> FwdEqnRegion { /* ... */ }
```

#### Function `h_tp_eqm_single_phase`

returns the enthalpy given temperature and pressure
single phase only!

```rust
pub fn h_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `u_tp_eqm_single_phase`

returns the internal energy given temperature and pressure

```rust
pub fn u_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> AvailableEnergy { /* ... */ }
```

#### Function `s_tp_eqm_single_phase`

returns the specific entropy given temperature and pressure

```rust
pub fn s_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cp_tp_eqm_single_phase`

returns the isobaric (const pressure) heat capacitygiven temperature and pressure

```rust
pub fn cp_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cv_tp_eqm_single_phase`

returns the isochoric (const vol) heat capacity given temperature and pressure

```rust
pub fn cv_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `v_tp_eqm_single_phase`

returns the specific volume given temperature and pressure

```rust
pub fn v_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> SpecificVolume { /* ... */ }
```

#### Function `w_tp_eqm_single_phase`

returns the speed of sound given temperature and pressure

```rust
pub fn w_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Velocity { /* ... */ }
```

#### Function `w_tpx_eqm`

returns speed of sound at vle given (t,p and x)
x being quality


note: there is some bug in the regioning algorithm here,
it is better to use p,s algorithm

```rust
pub fn w_tpx_eqm(t: ThermodynamicTemperature, p: Pressure, x: f64) -> Velocity { /* ... */ }
```

#### Function `kappa_tp_eqm_single_phase`

returns the isentropic exponent

```rust
pub fn kappa_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Ratio { /* ... */ }
```

#### Function `alpha_v_tp_eqm_single_phase`

returns the isobaric cubic expansion coefficient

```rust
pub fn alpha_v_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_tp_eqm`

returns the isothermal compressibility

```rust
pub fn kappa_t_tp_eqm(t: ThermodynamicTemperature, p: Pressure) -> crate::region_1_subcooled_liquid::InversePressure { /* ... */ }
```

### Re-exports

#### Re-export `alpha_p_rho_t_3`

re-exports the relative pressure coeff function for
region 3 relative pressure coeff (other regions don't have it)

```rust
pub use crate::region_3_single_phase_plus_supercritical_steam::alpha_p_rho_t_3;
```

#### Re-export `beta_p_rho_t_3`

re-exports the isothermal stress coeff function for
region 3 isothermal stress coeff (other regions don't have it)

```rust
pub use crate::region_3_single_phase_plus_supercritical_steam::beta_p_rho_t_3;
```

#### Re-export `mu_tp_eqm_single_phase`

viscosity function import

```rust
pub use crate::dynamic_viscosity::mu_tp_eqm_single_phase;
```

#### Re-export `multiphase_flashing::*`

```rust
pub use multiphase_flashing::*;
```

## Module `ph_flash_eqm`

allows for pressure enthalpy flash

```rust
pub mod ph_flash_eqm { /* ... */ }
```

### Functions

#### Function `t_ph_eqm`

Obtains temperature given pressure and enthalpy.

# ~~Region 5 is *deliberately* unsupported~~ — **CORRECTED 2026-09-27**

This doc block used to describe a *panic message* (the constant it documented
is long gone; the text was left stranded on this function) and read:

> ~~IAPWS-IF97 (see Wagner & Kretzschmar, *International Steam Tables*, 2019)
> provides **no backward `(p,h)` (or `(p,s)`) correlation for Region 5** — the
> released backward equations cover Regions 1, 2 and 3 only. This crate does
> **not** substitute a home-grown numerical inversion of the Region 5 forward
> enthalpy, so a `(p,h)` flash into Region 5 is *deliberately* unsupported for
> this physics/standards reason, not merely unfinished.~~

**The first sentence is still true; the conclusion is not.** IAPWS-IF97
indeed publishes no Region 5 backward equation — but commit `2ab91fefc3`
(2026-09-14) made this crate substitute exactly the home-grown inversion the
old text disclaims: `t_ph_eqm` now dispatches Region 5 to the in-house
Chebyshev fit `t_ph_5`.

**Verified by running**, not by reading:
`tests/boundary_273_15_repro.rs::does_lambda_ph_eqm_work_in_region_5` builds
a Region 5 state forward at `T = 1500 K`, `p = 1 MPa`
(`h = 5218.863 kJ/kg`) and gets `t_ph_eqm(p, h) = 1499.999 K` — no panic.
`region_5_ph_flash_round_trips_through_the_in_house_correlation`
(`interfaces/tests_and_examples/ph_flash_region4_edge_and_region5.rs`) pins
the same round trip at 0.5 MPa to 1e-4 relative, and that test *used to*
assert the panic this doc described.

**What is genuinely still refused**, and is pinned by
`region_5_above_its_pressure_limit_is_still_refused`: above Region 5's own
50 MPa ceiling (Regions 1-4 run to 100 MPa, Region 5 does not) and above the
2273.15 K isotherm — those are outside IF97 altogether, not merely outside a
backward equation.

**Read the Region 5 number as a fit, not a standard.** A dispatch that
succeeds is not a value IAPWS stands behind. Callers that already know the
temperature should still prefer the Region 5 forward `(T,p)` equations
(`h_tp_5`, `v_tp_5`, `s_tp_5`, ...), which are IF97.

```rust
pub fn t_ph_eqm(p: Pressure, h: AvailableEnergy) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `v_ph_eqm`

obtains volume given pressure and enthalpy (except for region 5)

```rust
pub fn v_ph_eqm(p: Pressure, h: AvailableEnergy) -> SpecificVolume { /* ... */ }
```

#### Function `u_ph_eqm`

returns the internal energy given temperature and pressure

```rust
pub fn u_ph_eqm(p: Pressure, h: AvailableEnergy) -> AvailableEnergy { /* ... */ }
```

#### Function `s_ph_eqm`

returns the specific entropy given temperature and pressure

```rust
pub fn s_ph_eqm(p: Pressure, h: AvailableEnergy) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cp_ph_eqm`

returns the isobaric (const pressure) heat capacitygiven temperature and pressure

```rust
pub fn cp_ph_eqm(p: Pressure, h: AvailableEnergy) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cv_ph_eqm`

returns the isochoric (const vol) heat capacity given temperature and pressure

```rust
pub fn cv_ph_eqm(p: Pressure, h: AvailableEnergy) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `w_ph_wood_wallis`

returns the speed of sound given ~~temperature and pressure~~ pressure and
specific enthalpy (**CORRECTED 2026-10-16**: the arguments are `(p, h)`).
Note: when in the equilibrium zone (Region 4) it WILL give the
wood wallis speed of sound, the frozen speed of sound. For the
homogeneous-equilibrium speed use [`w_ph_eqm_hem`].

```rust
pub fn w_ph_wood_wallis(p: Pressure, h: AvailableEnergy) -> Velocity { /* ... */ }
```

#### Function `w_ph_eqm_hem`

Homogeneous-**equilibrium** (HEM) speed of sound at `(p, h)`.

This is the speed at which a small pressure wave travels when the two
phases stay in thermodynamic equilibrium while it passes, so liquid flashes
and vapour condenses within the wave. Inside the vapour–liquid dome it is
far below the frozen (Wood–Wallis) speed of [`w_ph_wood_wallis`], and it is
the speed that places the characteristics of a homogeneous-equilibrium
flow model (a KNP central-upwind flux, a Courant number, a choking
criterion).

- **Two-phase dome (Region 4):** entropy `s = s_ph_eqm(p, h)` feeds Kieffer
  (1977) eq. 28, [`w_ps_eqm_region4_kieffer`], which differentiates the
  mixture volume along the saturation curve. That closure is
  AI-generated and not yet validated, so when it returns a non-finite or
  non-positive value this falls back to the finite difference of the
  equilibrium isentrope, [`w_ps_eqm_region4_finite_diff_vol`].
- **Single phase (Regions 1, 2, 3, 5):** there is no phase change to lag,
  so the equilibrium and frozen speeds coincide. Returns the region forward
  speed `w_tp_{1,2,3,5}(T, p)` at `T = t_ph_eqm(p, h)`.

Valid where the `(p, h)` flash is valid. **Not clamped:** if both Region 4
closures fail (e.g. very close to the critical point) the value can be
non-finite, and the caller decides what to do with that.

Reference: Kieffer, S. W. (1977), "Sound speed in liquid-gas mixtures:
water-air and water-steam", *J. Geophys. Res.* 82(20), 2895–2904.

Moved here from the private `hem_sound_speed_ph` of the `HybridAllMach`
KNP path (gh #901), ~~which now calls this function~~. **Updated
2026-10-16:** that path was retired (gh:#914) and `hem_sound_speed_ph` is
gone; the callers are now `crate::hem::Iapws97Hem` (the `HemPipe`
equation of state) and `TampinesSteamTableCV`'s HEM sound-speed getter.

```rust
pub fn w_ph_eqm_hem(p: Pressure, h: AvailableEnergy) -> Velocity { /* ... */ }
```

#### Function `w_two_phase_homogeneous_wood_wallis`

~~Returns the speed of sound in a two-phase mixture using the
homogeneous equilibrium model~~

~~This model assumes:~~
~~- Thermal and mechanical equilibrium between phases~~

**CORRECTED 2026-10-16 (gh #901):** the formula below is Wood's (Wallis)
mixture speed, which is the **frozen** two-phase speed of sound: the
mixture compressibility is the volume-weighted sum of the two phases' own
compressibilities, with **no** mass exchange between them during the wave.
It assumes mechanical equilibrium (one pressure) and no slip, but not
thermal or phase equilibrium. The homogeneous-*equilibrium* speed, which
includes flashing, is [`w_ph_eqm_hem`] and is much lower inside the dome.

This model assumes:
- Mechanical equilibrium (one pressure), no interphase mass or heat transfer
- No slip between liquid and vapor phases
- Isentropic process

Formula: w_mix = sqrt(1 / (rho_mix * ((x/(rho_g * w_g^2)) + ((1-x)/(rho_f * w_f^2)))))

where:
- x = steam quality (vapor mass fraction)
- rho_g = vapor density
- rho_f = liquid density
- w_g = speed of sound in vapor
- w_f = speed of sound in liquid
- rho_mix = mixture density = 1/((x/rho_g) + ((1-x)/rho_f))

Though to be fair,
we find that the speed of sound drops drastically in steam
we need to account for that

This is shown in:

Kieffer, S. W. (1977). Sound speed in liquid‐gas mixtures:
Water‐air and water‐steam. Journal of Geophysical research,
82(20), 2895-2904.
https://geology.illinois.edu/~skieffer/papers/SoundSpeed_JGR1977.pdf

The steam tables aren't that helpful
Though page 364 of Kretzchmar wagner provides the speed of sound
for purely vapour or purely fluid, and supercritical phase

However, VLE is not covered

The illinois paper is more useful, and so is this

https://ojs.cvut.cz/ojs/index.php/ap/article/view/2321/3200
Fig 1. also gives a similar diagram.

This is quite important for choked flow behaviour

Unless one assumes that speed of sound in this region does not
directly correlate to choked flow due to the non-equilibrium process
of choking




```rust
pub fn w_two_phase_homogeneous_wood_wallis(steam_quality: Ratio, w_liq: Velocity, w_vap: Velocity, rho_liq: MassDensity, rho_vap: MassDensity) -> Velocity { /* ... */ }
```

#### Function `kappa_ph_eqm`

returns the isentropic exponent

```rust
pub fn kappa_ph_eqm(p: Pressure, h: AvailableEnergy) -> Ratio { /* ... */ }
```

#### Function `alpha_v_ph_eqm`

returns the isobaric cubic expansion coefficient

```rust
pub fn alpha_v_ph_eqm(p: Pressure, h: AvailableEnergy) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_ph_eqm`

returns the isothermal compressibility

```rust
pub fn kappa_t_ph_eqm(p: Pressure, h: AvailableEnergy) -> crate::region_1_subcooled_liquid::InversePressure { /* ... */ }
```

#### Function `x_ph_flash`

obtains steam quality (vap fraction) given
pressure and enthalpy

```rust
pub fn x_ph_flash(p: Pressure, h: AvailableEnergy) -> f64 { /* ... */ }
```

#### Function `ph_flash_region`

Determines which IAPWS-IF97 forward-equation region a `(p,h)` point
belongs to, where pressure `p` is in Pa and specific enthalpy `h` is in
J/kg.

Dispatches to Region 1 (subcooled liquid), Region 2 (vapour), Region 3
(single-phase near-critical/supercritical) or Region 4 (vapour-liquid
equilibrium) by comparing `h` against the region-boundary enthalpies at
the given pressure. Panics (via `check_if_within_ph_validity_region`) if
the point falls outside the valid pressure/enthalpy envelope, including
above the 1073.15 K isotherm — Region 5 has no IAPWS-IF97 backward
`(p,h)` correlation, so `(p,h)` flashing does not work in Region 5.

```rust
pub fn ph_flash_region(p: Pressure, h: AvailableEnergy) -> super::pt_flash_eqm::FwdEqnRegion { /* ... */ }
```

#### Function `lambda_ph_eqm`

Returns the thermal conductivity in W/(m*K) given a `(p,h)` flash, where
pressure `p` is in Pa and specific enthalpy `h` is in J/kg.

Combines the IAPWS thermal-conductivity correlation's dilute-gas
(`lambda_0`), residual (`lambda_1`) and critical-enhancement (`lambda_2`)
terms, evaluated at the temperature/density/quality resolved from the
`(p,h)` flash via [`ph_flash_region`].

# ~~"Regions 1-4; Region 5 is unsupported"~~ -- CORRECTED 2026-09-27

**Region 5 works.** Measured at `T = 1500 K, p = 1 MPa`: `lambda_ph_eqm`
returns **0.16748 W/(m*K)** with no panic, and `t_ph_eqm` recovers 1499.999 K
from the forward Region 5 enthalpy of 5218.863 kJ/kg
(`tests/boundary_273_15_repro.rs::does_lambda_ph_eqm_work_in_region_5`).

The struck claim predates commit `2ab91fefc3` ("(p,h), (p,s) and (h,s) now
cover Region 5") and was never updated. All three properties the critical-
enhancement term needs now dispatch there --
`cp_tp_eqm_single_phase -> cp_tp_5`, `cv_tp_eqm_single_phase -> cv_tp_5`,
`kappa_t_tp_eqm -> kappa_t_tp_5` -- and `region_fwd_eqn_single_phase` has a
`Region5` arm.

# But RUNNING is not the same as being VALID

**IAPWS R15-11, the thermal-conductivity release, is stated for
`273.16 K <= T <= 1173.15 K`.** Region 5 runs to **2273.15 K**, so between
roughly 1173 K and 2273 K this function returns an **extrapolation of the
correlation**, not a tabulated value. That is a physics limit, not a dispatch
limit, and no amount of region plumbing removes it. A caller relying on
`lambda` above 1173.15 K is extrapolating and should say so.

The **low** end has its own, separate defect: a `(p,h)` point the validity
guard accepts can invert to a temperature ~18 mK below the 273.15 K floor,
which this function then hands to a strict `(T,p)` region lookup that panics.
See `tests/boundary_273_15_repro.rs` and GitHub #342.

```rust
pub fn lambda_ph_eqm(p: Pressure, h: AvailableEnergy) -> ThermalConductivity { /* ... */ }
```

### Re-exports

#### Re-export `mu_ph_eqm`

viscosity

```rust
pub use crate::dynamic_viscosity::mu_ph_eqm;
```

## Module `pt_flash_metastable`

this pt_flash allows for metastable steam
which is NOT at thermodynamic equilibrium

this mostly deals with areas around region 2

```rust
pub mod pt_flash_metastable { /* ... */ }
```

## Module `ps_flash_eqm`

allows for pressure entropy flash

```rust
pub mod ps_flash_eqm { /* ... */ }
```

### Functions

#### Function `t_ps_eqm`

obtains temperature given pressure and entropy

```rust
pub fn t_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `v_ps_eqm`

obtains volume given pressure and entropy (except for region 5)

```rust
pub fn v_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> SpecificVolume { /* ... */ }
```

#### Function `x_ps_flash`

obtains steam quality (vap fraction) given
pressure and entropy

```rust
pub fn x_ps_flash(p: Pressure, s: SpecificHeatCapacity) -> f64 { /* ... */ }
```

#### Function `u_ps_eqm`

returns the internal energy given entropy and pressure

```rust
pub fn u_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> AvailableEnergy { /* ... */ }
```

#### Function `h_ps_eqm`

returns the specific enthalpy given entropy and pressure

```rust
pub fn h_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> AvailableEnergy { /* ... */ }
```

#### Function `ps_flash_region`

Determines which IAPWS-IF97 forward-equation region a `(p,s)` point
belongs to, where pressure `p` is in Pa and specific entropy `s` is in
J/(kg*K).

Dispatches to Region 1 (subcooled liquid), Region 2 (vapour), Region 3
(single-phase near-critical/supercritical) or Region 4 (vapour-liquid
equilibrium) by comparing `s` against the region-boundary entropies at
the given pressure. Panics (via `check_if_within_ps_validity_region`) if
the point falls outside the valid pressure/entropy envelope.

~~Region 5 is not yet implemented for the callers that key off this
dispatcher (see `todo!` panics in `t_ps_eqm`/`v_ps_eqm`).~~
**CORRECTED 2026-09-27** — this dispatcher **does** return
[`FwdEqnRegion::Region5`], from the `is_above_isotherm_t_1073_15` branch a
few lines below, and both `todo!`s are gone: commit `2ab91fefc3` (2026-09-14,
"(p,h), (p,s) and (h,s) now cover Region 5") replaced them with a dispatch to
the in-house Chebyshev `t_ps_5` correlation plus the IAPWS Region 5 forward
`v_tp_5`. **Verified by running**, not by reading: at `T = 1500 K`,
`p = 1 MPa` (state built from the forward `s_tp_eqm_single_phase`,
`s = 9.33359 kJ/(kg K)`) this function returns `Region5` and
`t_ps_eqm(p, s) = 1500.0000 K`, `|dT/T| = 6.0e-9` — see
`tests/boundary_273_15_repro.rs::does_the_ps_flash_reach_region_5_and_does_the_checked_facade`.
`grep -rn 'todo!' ps_flash_eqm/` returns nothing.

**But a dispatch that succeeds is not a value IAPWS stands behind.** IAPWS-
IF97 publishes **no** backward `(p,s)` equation for Region 5 at all; the
temperature step here is this crate's own fit, verified only against the
forward equations it inverts. Treat it as an in-house correlation, not an
IF97 result.

```rust
pub fn ps_flash_region(p: Pressure, s: SpecificHeatCapacity) -> super::pt_flash_eqm::FwdEqnRegion { /* ... */ }
```

#### Function `cp_ps_eqm`

returns the isobaric (const pressure) heat capacitygiven temperature and pressure

```rust
pub fn cp_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `cv_ps_eqm`

returns the isochoric (const vol) heat capacity given temperature and pressure

```rust
pub fn cv_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `w_ps_wood_wallis`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns the speed of sound given temperature and pressure
Note: when in the equilibrium zone (Region 4) it WILL give the
wood wallis speed of sound, the frozen speed of sound

```rust
pub fn w_ps_wood_wallis(p: Pressure, s: SpecificHeatCapacity) -> Velocity { /* ... */ }
```

#### Function `mass_flux_ps_eqm_throat`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns the mass flux given properties at throat (this is not
stagnation pressure)
c² = -v² * (dp/dv|_s) = -v² / (dv/dp|_s)
c = v * sqrt(-1/dv_dp_s)

consider that critical mass flux in terms of throat properties is c*rho
which is 1/v

c*rho = sqrt(-1/dv_dp_s)

# Known limitation — x = 0 (saturated liquid boundary)

When s == s_f(p) exactly (throat quality = 0), the finite-difference step
dp = p * 1e-5 is too small to span a meaningful two-phase region:
  - v_ps_eqm(p + dp, s) lands in Region 1 (s < s_f at p+dp) → pure-liquid compressibility
  - v_ps_eqm(p - dp, s) barely enters Region 4 with near-zero quality → still ~pure-liquid

The resulting dv/dp_s reflects liquid compressibility, so G ≈ ρ_l · c_l ≈ 1.5×10⁶ kg/m²/s
(liquid sound speed), which is unphysically large for HEM two-phase critical flow.
This inflated G then drives h_0 = h_f + 1125 kJ/kg, causing p_hs_eqm to panic with
"enthalpy too high".


Basically around bubble point, the function will return a mass flux
reflective of quality at 1e-4

This was validated using Zaloudek's data

```rust
pub fn mass_flux_ps_eqm_throat(p: Pressure, s: SpecificHeatCapacity) -> MassFlux { /* ... */ }
```

#### Function `kappa_ps_eqm`

returns the isentropic exponent

```rust
pub fn kappa_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Ratio { /* ... */ }
```

#### Function `alpha_v_ps_eqm`

returns the isobaric cubic expansion coefficient

```rust
pub fn alpha_v_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> TemperatureCoefficient { /* ... */ }
```

#### Function `kappa_t_ps_eqm`

returns the isothermal compressibility

```rust
pub fn kappa_t_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> crate::region_1_subcooled_liquid::InversePressure { /* ... */ }
```

## Module `hs_flash_eqm`

allows for enthalpy entropy flash;
Note: this only works for part of the steam table

```rust
pub mod hs_flash_eqm { /* ... */ }
```

### Modules

## Module `validity_range`

note:
(h,s) flashes along the isotherms 273.15K are not implemented
for simplicity to avoid iterations

```rust
pub mod validity_range { /* ... */ }
```

### Functions

#### Function `hs_is_above_isotherm_t_273_15_kelvin`

based on page 72 boundary, we use this
for all pressure

```rust
pub fn hs_is_above_isotherm_t_273_15_kelvin(h: AvailableEnergy, s: SpecificHeatCapacity) -> bool { /* ... */ }
```

#### Function `hs_is_below_isobar_p_100_mpa_in_region1`

based on page 73 boundary, we use this at
this only applies to region 1

NOTE: unimplemented for the in-region-1 case — falls through to
`todo!()` and panics unless the early `(h,s)` bound check above already
returns `false`.

```rust
pub fn hs_is_below_isobar_p_100_mpa_in_region1(h: AvailableEnergy, s: SpecificHeatCapacity) -> bool { /* ... */ }
```

### Types

#### Enum `BackwdEqnSubRegion`

an enum to help represent the appropriate
regions in the forward equations

```rust
pub enum BackwdEqnSubRegion {
    Region1,
    Region2a,
    Region2b,
    Region2c,
    Region3a,
    Region3b,
    Region4,
    Region5,
}
```

##### Variants

###### `Region1`

this is from T = 273.15 K to T=623.15K
liquid

###### `Region2a`

this is vapour then line p23/t23
all the way up to 1073.15 K (800 degC)


higher entropy than 5.85 kJ/(kg K)
but even higher than 2b
includes the boundary line h2ab

###### `Region2b`

this is vapour then line p23/t23
all the way up to 1073.15 K (800 degC)

higher or equal to entropy than 5.85 kJ/(kg K)
includes boundary line 5.85 kJ/(kg K)

###### `Region2c`

this is vapour then line p23/t23
all the way up to 1073.15 K (800 degC)

lower entropy than 5.85 kJ/(kg K)

###### `Region3a`

this is supercritical region and
single phase liquid  / vapour near
supercritical region

below or equal to critical entropy

###### `Region3b`

this is supercritical region and
single phase liquid  / vapour near
supercritical region
above critical entropy

###### `Region4`

two phase vapour liq equilibrium
region up to supercritical region
(saturation line, but not including the line itself)

###### `Region5`

ultra high temperature steam  (more than 800 degC)
1073.15 K to 2273.15 K
pressure from triple pt pressure to 500 bar

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> FwdEqnRegion { /* ... */ }
    ```

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

#### Function `t_hs_eqm`

returns temperature given
enthalpy and entropy point



```rust
pub fn t_hs_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `p_hs_eqm`

returns pressure given
enthalpy and entropy point



```rust
pub fn p_hs_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

#### Function `v_hs_eqm`

returns specific volume given
enthalpy and entropy point



```rust
pub fn v_hs_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> SpecificVolume { /* ... */ }
```

#### Function `x_hs_eqm`

returns quality given
enthalpy and entropy point



```rust
pub fn x_hs_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> Ratio { /* ... */ }
```

#### Function `cp_hs_eqm`

returns cp given
enthalpy and entropy point
uses ph flash

```rust
pub fn cp_hs_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `w_hs_eqm`

returns w (speed of sound) given
enthalpy and entropy point
uses ph flash

```rust
pub fn w_hs_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> Velocity { /* ... */ }
```

#### Function `kappa_hs_eqm`

returns kappa (isentropic exponent) given
enthalpy and entropy point
uses ph flash

```rust
pub fn kappa_hs_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> Ratio { /* ... */ }
```

#### Function `mu_hs_eqm`

returns mu, or sometimes eta (dynamic viscosity) given
enthalpy and entropy point
uses ph flash

```rust
pub fn mu_hs_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> DynamicViscosity { /* ... */ }
```

#### Function `lambda_hs_eqm`

returns lambda (thermal conductivity) given
enthalpy and entropy point
uses ph flash

```rust
pub fn lambda_hs_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> ThermalConductivity { /* ... */ }
```

#### Function `tpvx_hs_flash_eqm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns temperature, pressure, specific volume and quality given
enthalpy and entropy point

I'm doing this combined function to prevent double calculation


```rust
pub fn tpvx_hs_flash_eqm(h: AvailableEnergy, s: SpecificHeatCapacity) -> (ThermodynamicTemperature, Pressure, SpecificVolume, Ratio) { /* ... */ }
```

#### Function `hs_flash_region`

allows the user to check which region one is in based on a ph flash

note that ph flash does not work in region 5

the way to do region separation is first by entropy according to
fig 2.14

once that is done, then we separate region by enthalpy.

```rust
pub fn hs_flash_region(h: AvailableEnergy, s: SpecificHeatCapacity) -> BackwdEqnSubRegion { /* ... */ }
```

#### Function `find_pressure_from_hs_region_4`

Finds pressure given enthalpy and entropy using bisection method

Given: h and s (known state point)
Find: p such that s(p, h) = s_target

Uses bisection between minimum pressure
(triple point) and critical pressure
vibe coded and edited

```rust
pub fn find_pressure_from_hs_region_4(h_target: AvailableEnergy, s_target: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

## Module `rho_h_flash_eqm`

allows for density-enthalpy `(rho,h)` flash: pressure, temperature and
steam quality, obtained by inverting the IAPWS-IF97 `(p,h)` backward
equations.

This is what a compressible flow solver needs, since continuity gives it
density and the energy equation gives it enthalpy, but IF97 publishes no
`(rho,h)` backward equation of its own.
Density-enthalpy `(rho,h)` flash: pressure, temperature and steam quality
for a state whose independent variables are **density and specific
enthalpy**.

# Why this module exists

IAPWS-IF97 publishes backward equations for `(p,h)`, `(p,s)` and `(h,s)`,
but none for `(rho,h)`. A compressible flow solver, however, carries
**density** (from continuity) and **enthalpy** (from the energy equation)
as its conserved variables, and needs pressure back out of them. Without a
`(rho,h)` entry point such a solver has to reach for a two-dimensional
iterative solve over the forward (Gibbs/Helmholtz) equations, which is both
slow and fragile near the saturation line.

This module closes that gap **using the published backward equations
themselves**. The observation it rests on is that
[`v_ph_eqm`](crate::interfaces::functional_programming::ph_flash_eqm::v_ph_eqm)
is already an *explicit* function: given `(p,h)` it selects a region and
evaluates a backward equation (`T_ph` in Regions 1 and 2, `v_ph_3` directly
in Region 3, a quality-weighted mixture in Region 4) with no inner
iteration. Inverting it therefore collapses from a two-dimensional solve to
a **one-dimensional, bracketed root find in pressure alone**, where every
residual evaluation costs exactly one explicit backward-equation flash.

# What is IAPWS-traceable here, and what is not

Every thermodynamic value this module returns comes from the crate's
IAPWS-IF97 equations — it adds no fitted correlation of its own. What it
adds is the **inversion strategy** (bracketing and root finding), which is
numerics, not thermodynamics. That distinction matters: the accuracy of
`p(rho,h)` is the accuracy of IF97's own `v(p,h)` plus a convergence
tolerance that the caller can read off [`P_RHO_H_REL_TOL`].

Contrast this with
[`backward_eqn_chebyshev_experimental::p_rho_h`](crate::backward_eqn_chebyshev_experimental::p_rho_h),
which is an in-house Chebyshev *fit* to the same surface together with a
statistical region classifier. That module is fast and explicit but carries
fit error and a classifier that misfiles roughly 2.7 % of states (8.4 % in
Region 4). **This module is the accurate path**; the Chebyshev module is the
cheap-and-approximate one. They are complementary, not redundant.

# Validity

The same domain as the `(p,h)` flash it inverts: pressure in
`[p_sat(273.15 K), 100 MPa]` and enthalpy between the `273.15 K` and
`1073.15 K` isotherms. Region 5 (above 1073.15 K) has no `(p,h)` backward
equation in this crate and is therefore not reachable from `(rho,h)` either.

# Measured accuracy, and where this does NOT work

Verified against the published steam tables in
`interfaces::tests_and_examples::rho_h_flash_steam_table` (2334 single-phase
nodes and 220 saturation rows, measured 2026-09-14). **No node failed**: every
one was recovered either to the right pressure (`|dp/p| < 1e-6`, 2284 nodes)
or to the right density (`|dv/v| < 1e-9`, 50 nodes).

Worst pressure error by region, inverting this crate's own `v(p,h)`:

| region | max `\|dp/p\|` | comment |
|---|---|---|
| Region 4 (two-phase) | `3.7e-13` | the blowdown regime; excellent |
| Region 2 (vapour) | `3.0e-5` | well conditioned |
| Region 3 | `4.8e-5` | well conditioned |
| Region 1 (compressed liquid) | `7.9e-1` | **input-error sensitive — see below** |

**What Region 1 does and does not mean — an earlier version of this file
got this wrong, so it is stated carefully.** The `7.9e-1` above is measured
against *table-rounded* density: the published `v` carries six significant
figures, and in nearly-incompressible liquid that rounding is amplified.
The amplification

```text
A = |d ln p / d ln v|_h  ~  1 / (p * kappa_T)
```

multiplies whatever error is already in the **input**. It does not mean the
inversion is inaccurate.

Given an *exact* density the inversion is exact throughout the compressed
liquid. Measured 2026-09-14 on subcooled liquid at 18 degC, feeding density
straight from this crate's own `v(p,h)`
(`diagnose_saturation_fallback_against_the_root_find`):

| true p | recovered p | `A` |
|---|---|---|
| 0.5 bar | 0.50000 bar | `4.0e4` |
| 1 bar | 1.00000 bar | `2.0e4` |
| 10 bar | 10.00000 bar | `2.0e3` |
| 70 bar | 70.00000 bar | `2.9e2` |
| 99 MPa | 990.00000 bar | `2.3e1` |

Exact at every one, including where `A = 4.0e4`, because a large `A` acting
on a machine-precision input error is still a machine-precision output
error. **So "the compressed liquid is not recoverable" is false as a
blanket claim** — it is recoverable whenever the caller's density is better
known than the printed tables.

The one state in that sweep that genuinely fails is **0.1 bar**, which
returns `0.02065 bar` against a true `0.1 bar`. That value is exactly
`p_sat(T(h))`, the bubble point — i.e. the search converged to the bottom
edge of its own bracket, not to the root. **That is a branch-selection
defect in this module, not a physical limit**, and it is filed rather than
papered over.

[`p_rho_h_conditioning`] reports `A` for a state, which is what a caller
needs to turn its own density uncertainty into a pressure uncertainty. In
the two-phase and vapour regions — where a depressurisation transient
actually spends its time — `A` is order 1.

# Cost, and where this belongs

Measured 2026-09-14 over a spread of states
(`diagnose_the_cost_of_the_inversion_relative_to_a_ph_flash`):
`v_ph_eqm` 3.45 us/call, `p_rho_h_eqm` 205 us/call â a factor of **59.5**.

That ordering is structural, not a tuning failure: this is a bracketed root
find built out of repeated `v(p,h)` evaluations, plus a region scan, so it
cannot be cheaper than the flash it inverts.

The practical consequence is worth stating plainly, because it is easy to
reach for this function in the wrong place. **A pressure-based solver has
nothing to gain here.** If the algorithm already carries `p` as a primary
variable and derives density from it â which is what
`TampinesSteamArray::correct_thermo` does â then pressure never has to be
recovered, and routing through this module would only add cost. The payoff
is against a *two-dimensional* iterative solve over the forward equations,
i.e. in a density-based algorithm where `rho` and `h` are the conserved
variables and `p` genuinely must be inverted for.

Most of the cost is the region scan rather than the root find; narrowing it
is tracked as a follow-up rather than done here, since correctness across
the seams was the point.

# Contents

- [`p_rho_h_eqm`] / [`p_rho_h_eqm_explicit`] — pressure from `(rho,h)`.
- [`tpx_rho_h_eqm`] — temperature, pressure and steam quality together,
  returned as [`TpxRhoH`].
- [`rho_h_is_within_validity_range`] — a non-panicking domain predicate, for
  callers that must check before committing to a flash.

```rust
pub mod rho_h_flash_eqm { /* ... */ }
```

### Types

#### Struct `TpxRhoH`

Temperature, pressure and steam quality recovered from a `(rho,h)` state.

Returned by [`tpx_rho_h_eqm`]. The quality convention is documented on that
function — in particular, `vapour_quality` is a genuine equilibrium quality
only inside Region 4; elsewhere it is the single-phase convention 0 (liquid)
or 1 (vapour).

```rust
pub struct TpxRhoH {
    pub temperature: ThermodynamicTemperature,
    pub pressure: Pressure,
    pub vapour_quality: f64,
    pub region: crate::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `ThermodynamicTemperature` | Temperature of the state. |
| `pressure` | `Pressure` | Pressure of the state. |
| `vapour_quality` | `f64` | Steam (vapour) quality, dimensionless, on `[0, 1]`.<br><br>Inside Region 4 this is the equilibrium vapour mass fraction. Outside<br>it, it is a convention flag rather than a measured fraction — see<br>[`tpx_rho_h_eqm`]. |
| `region` | `crate::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion` | The IF97 region the state was found in. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

#### Function `rho_h_is_within_validity_range`

Returns `true` when a `(rho,h)` state can be flashed by [`p_rho_h_eqm`].

`rho` is mass density and `h` specific enthalpy. A state is flashable when
some pressure in the `(p,h)` domain reproduces the requested density; this
predicate answers that question without panicking, so a caller carrying
possibly-unphysical solver state can check first.

```rust
pub fn rho_h_is_within_validity_range(rho: MassDensity, h: AvailableEnergy) -> bool { /* ... */ }
```

#### Function `p_rho_h_eqm_explicit`

Pressure in pascal from density in kg/m3 and specific enthalpy in J/kg, by
**iteration** — the undimensioned twin of [`p_rho_h_eqm`].

For hot loops that already hold SI scalars and do not want to pay for `uom`
construction. Same algorithm, same tolerance, same panics as
[`p_rho_h_eqm`].

# Renamed 2026-09-15 — read this if you were calling `p_rho_h_eqm_explicit`

This function used to be called `p_rho_h_eqm_explicit`, where "explicit"
meant *explicit SI scalars*. That sat one module away from
[`crate::backward_eqn_chebyshev_experimental::p_rho_h_in_region_explicit`],
where "explicit" means *explicit closed-form equation* — the opposite kind
of thing, since this one iterates and that one does not. The name now means
what the rest of the crate means by it: [`p_rho_h_eqm_explicit`] is the
closed-form correlation, and this is the iterative solve in SI scalars.

# Panics

Panics when `rho_si` is not strictly positive and finite, when `h_si` is not
finite, or when the `(rho,h)` state lies outside the `(p,h)` flash domain
(see [`rho_h_is_within_validity_range`] for a non-panicking check).
Pressure from density and specific enthalpy by a **closed-form correlation**
— no iteration.

This is the fast, approximate route: it evaluates this crate's own fitted
`(rho,h)` backward surfaces (see
[`crate::backward_eqn_chebyshev_experimental::p_rho_h`], GitHub issue #34)
rather than inverting the IF97 backward equations numerically the way
[`p_rho_h_eqm`] does. Where [`p_rho_h_eqm`] costs about 75 forward flashes
per call, this costs one polynomial evaluation.

# THESE ARE NOT IAPWS EQUATIONS, AND THE ERROR IS LARGE IN PLACES

IAPWS-IF97 publishes **no** `(rho,h)` backward equations. These surfaces are
fitted in-house, are **experimental, AI-assisted draft material with no
human V&V sign-off**, and their accuracy varies enormously by region.
Measured against this crate's own forward equations (regenerate with
`cargo test --release -p tampines-steam-tables --lib
backward_eqn_chebyshev_experimental::tests::p_rho_h`):

| region / subset | median | 99th pct | maximum |
|---|---|---|---|
| Region 1 (liquid) | 5.619e-4 | 1.838e0 | **3.185e0** |
| Region 2 | 9.788e-6 | 6.240e-5 | 1.549e-4 |
| Region 3 | 1.082e-4 | 3.407e-4 | 3.688e-4 |
| Region 5 | 2.146e-5 | 1.833e-4 | 1.306e-3 |
| Region 4, interior `0.05 <= x < 0.95` | 2.041e-4 | 2.967e-3 | 4.158e-3 |
| Region 4, bubble point `x < 0.05` | 1.145e-4 | 1.409e-1 | **1.773e-1** |

**Region 1 can be wrong by a factor of four** — the worst recorded state
recovers 0.00419 MPa where the reference is 0.00100 MPa. That is not a fit
defect so much as conditioning: liquid water is nearly incompressible, so
`|d ln p / d ln v|_h` reaches about `4.0e4` at 0.5 bar and any error in the
density is amplified by that factor into the pressure. See
[`p_rho_h_conditioning`], which reports the amplification for a given state
so a caller can decide whether this route is usable there.

# Choosing between this and [`p_rho_h_eqm`]

Reach for this only where **speed matters more than the last few digits and
the state is not in liquid or near the bubble point** — a first guess to
seed an iteration, a plotting heuristic, a classifier. Use
[`p_rho_h_eqm`] (or [`p_rho_h_eqm_si`]) wherever the answer is the product.

A worked example of choosing wrongly, and of a third option beating both:
the steam-tables GUI draws constant-volume lines, which spend their length
in exactly the two regions above that are weakest — the liquid branch and
the bubble point. It uses neither route. Re-parameterising the curve by
temperature makes the dome crossing a closed-form lever rule accurate to
about `1e-14`, because at fixed `T` the saturation pressure is already
known and nothing has to be inverted at all.

# Cost, measured 2026-09-15

The speed advantage is real but **not uniform**, and there is one regime
where this function is slower than the iterative route it exists to replace:

| state | this | [`p_rho_h_eqm_si`] | ratio |
|---|---|---|---|
| vapour, `rho = 4.07`, `h = 3000 kJ/kg` | 1 893 ns | 54 043 ns | 28x faster |
| low-pressure vapour, `rho = 0.84` | 4 316 ns | 184 381 ns | 43x faster |
| liquid, `rho = 982`, `h = 300 kJ/kg` | 3 848 ns | 58 486 ns | 15x faster |
| **near-critical two-phase, `rho = 315.6`** | **242 713 ns** | 225 577 ns | **0.93x — SLOWER** |

The last row is a direct consequence of the near-critical exclusion: such a
state pays for the classifier, the fitted surface and the region flash, then
is handed to the iterative route anyway. It pays twice.

Fixing it means deciding to iterate **before** any of that work, from `rho`
and `h` alone — a proximity test against the critical point rather than a
quality test needing a pressure. Tracked in GH #207; until then a caller in
that regime should call [`p_rho_h_eqm`] directly.

# Panics

Panics when `rho_si` is not strictly positive and finite, or when `h_si` is
not finite. Unlike [`p_rho_h_eqm_si`] it does **not** panic for a state
outside the `(p,h)` flash domain: a fitted surface has no domain check of
its own, and will happily extrapolate. Call
[`rho_h_is_within_validity_range`] first if that matters.

```rust
pub fn p_rho_h_eqm_explicit(rho_si: f64, h_si: f64) -> f64 { /* ... */ }
```

#### Function `p_rho_h_eqm_si`

```rust
pub fn p_rho_h_eqm_si(rho_si: f64, h_si: f64) -> f64 { /* ... */ }
```

#### Function `p_rho_h_eqm`

Pressure from density and specific enthalpy, by inverting the IAPWS-IF97
`(p,h)` backward equations.

`rho` is mass density and `h` is specific enthalpy. The returned pressure is
the one at which this crate's own `v(p,h)` reproduces `1/rho`, converged to
[`P_RHO_H_REL_TOL`] relative.

# Valid range

The `(p,h)` flash domain: `p_sat(273.15 K) <= p <= 100 MPa` and
`273.15 K <= T <= 1073.15 K`. Region 5 is not reachable. Use
[`rho_h_is_within_validity_range`] to test a state without risking a panic.

# Accuracy

This inverts the published backward equations rather than fitting them, so
the error is IF97's own `v(p,h)` error plus the convergence tolerance. That
makes it the accurate counterpart to the explicit Chebyshev fit in
[`crate::backward_eqn_chebyshev_experimental::p_rho_h`], which is cheaper
but carries fit and classifier error.

# Panics

Panics on a non-finite or non-positive density, a non-finite enthalpy, or a
state outside the flash domain.

```rust
pub fn p_rho_h_eqm(rho: MassDensity, h: AvailableEnergy) -> Pressure { /* ... */ }
```

#### Function `tpx_rho_h_eqm`

Temperature, pressure and steam quality from density and specific enthalpy.

`rho` is mass density and `h` is specific enthalpy. The pressure comes from
[`p_rho_h_eqm`]; temperature and quality then follow from the `(p,h)`
backward equations at that pressure, so the three returned values are
mutually consistent by construction.

# The steam-quality convention outside Region 4

Quality is only physically meaningful in the two-phase dome. Region 4
therefore returns the genuine equilibrium vapour mass fraction from
`x_ph_flash`. Everywhere else this function reports a **convention flag**,
so that a caller carrying `x` as a field never has to special-case a
missing value:

- **Region 1** (subcooled / compressed liquid): `x = 0`.
- **Regions 2 and 5** (superheated vapour): `x = 1`.
- **Region 3 below the critical pressure**: compared against the saturation
  temperature at that pressure — `T < T_sat(p)` gives `x = 0`
  (liquid-like), otherwise `x = 1` (vapour-like).
- **At or above the critical pressure** (`p >= 22.064 MPa`): there is no
  phase boundary to cross, so the convention is taken from the critical
  *temperature* instead — a state to the **left** of the critical point
  (`T < 647.096 K`) is reported `x = 0`, and one to the **right**
  (`T >= T_c`) is reported `x = 1`.

That last rule is the project convention for supercritical states. It is a
labelling choice, not a physical claim: above the critical pressure the
fluid is a single supercritical phase and no vapour fraction exists. Do not
feed a supercritical `x` into a two-phase correlation and expect meaning
from it.

# Panics

Panics under the same conditions as [`p_rho_h_eqm`], and additionally if the
recovered state classifies as Region 5, for which this crate has no `(p,h)`
backward equation.

```rust
pub fn tpx_rho_h_eqm(rho: MassDensity, h: AvailableEnergy) -> TpxRhoH { /* ... */ }
```

#### Function `p_rho_h_conditioning`

How well the pressure is determined by the density at this state.

Returns the **amplification factor**

```text
A = |d ln p / d ln v|_h
```

which is the factor by which a relative error in specific volume becomes a
relative error in the pressure that [`p_rho_h_eqm`] returns. `A` near 1
means the inversion is well conditioned; `A` of 1e5 means a part-per-million
error in density becomes a 10 % error in pressure.

# Why this is public

Because the limitation it measures is real, and a caller cannot see it
otherwise. In the **low-pressure subcooled liquid** the pressure signal
carried by the density falls *below IF97's own backward-equation noise*:
water at 18 degC has almost exactly the same density at 0.02 bar as at
0.1 bar, while the `T(p,h)` backward equation's ~25 mK uncertainty moves the
volume by ~5e-6 relative — more than the whole pressure signal across that
range. `p(rho,h)` there returns a pressure that satisfies the density to
machine precision and is still badly wrong, and **no implementation can do
better**: the information is not in the inputs.

Measured 2026-09-14 (`diagnose_the_conditioning_measure_across_regimes`):

| state | `A` |
|---|---|
| superheated vapour, 10 bar | `1.0` |
| two-phase, 1 bar | `1.0` |
| saturated liquid, 8 bar | `5.3e-2` |
| compressed liquid, 99 MPa | `2.1e1` |
| subcooled liquid, 0.5 bar | `4.0e4` |
| subcooled liquid, 0.1 bar | `4.8e6` |

# Cost

Two extra flashes, because it perturbs the density and re-solves rather
than differentiating locally. That is deliberate â the cheap local
derivative is blind in exactly the regime this exists to detect â so treat
it as a diagnostic to call when in doubt, not something for a hot loop.

A solver carrying `(rho,h)` should check this where it might be in the
subcooled liquid, and take its pressure from the momentum/pressure equation
rather than the equation of state when `A` is large. In the two-phase and
vapour regions — where a blowdown actually spends its time — `A` is order 1
and `p(rho,h)` is trustworthy.

# Panics

Panics under the same conditions as [`p_rho_h_eqm`].

```rust
pub fn p_rho_h_conditioning(rho: MassDensity, h: AvailableEnergy) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `P_RHO_H_REL_TOL`

Relative convergence tolerance on pressure for [`p_rho_h_eqm`].

The returned pressure satisfies `|p - p_exact| <= P_RHO_H_REL_TOL * p`,
where `p_exact` is the pressure at which this crate's own `v(p,h)` equals
the requested specific volume exactly. It is a *numerical* tolerance on the
inversion and says nothing about IF97's own uncertainty.

```rust
pub const P_RHO_H_REL_TOL: f64 = 1.0e-12;
```

## Module `checked`

Bounds-checked, `Result`-returning facade over the panicking flash
internals: validates `(T,p)` / `(p,h)` input against the IAPWS-IF97
validity envelope BEFORE calling the unchecked functions, returning
[`checked::SteamTablesError`] instead of panicking on out-of-range or
non-finite input (bead `op-t647`).
Bounds-checked, `Result`-returning facade over the IAPWS-IF97 flash
internals (bead `op-t647`).

The unchecked entry points in
[`super::functional_programming::pt_flash_eqm`] and
[`super::functional_programming::ph_flash_eqm`] **panic** on
out-of-range input (`region_fwd_eqn_single_phase` and
`check_if_within_ph_validity_region` both end in `panic!`). In a
transient simulation a single overshooting state kills the physics
thread. This module wraps the commonly used `(T,p)` and `(p,h)` flash
functions with an explicit validity-envelope check performed **before**
any panicking internal is called, returning
[`SteamTablesError`] instead of panicking.

## The envelope enforced here

The bounds below are **read from this crate's own region router**
(`region_fwd_eqn_single_phase` in `pt_flash_eqm/mod.rs` and
`check_if_within_ph_validity_region` + `validity_range.rs` in
`ph_flash_eqm/`), verified against the source on 2026-08-11 — they are
the set of inputs the internals actually accept, which matches the
documented IAPWS-IF97 envelope with three sharp edges noted below.

`(T,p)` single-phase flashes ([`try_h_tp_eqm_single_phase`] etc.):

- `T` in `[273.15 K, 2273.15 K]` overall;
- `p` in `(0, 100 MPa]` for `T` in `[273.15 K, 1073.15 K]` — the
  100 MPa ceiling is **inclusive at every temperature in this band**.
  (Between this module landing on 2026-08-11 and the `op-cv1c` fix
  later the same day, the edge was exclusive above 623.15 K, because
  the internal region router's Region-2 and Region-3 arms used
  half-open ranges (`..100e6`) and exactly 100 MPa fell through to
  their `panic!`. The router now closes those arms with `..=100e6`, so
  the facade no longer has to carve the point out.)
- `p` in `(0, 50 MPa]` for `T` in `(1073.15 K, 2273.15 K]` (Region 5);
- a `(T, p)` pair lying **exactly** on the saturation line
  (`p == p_sat(T)` bit-for-bit, `T < 647.096 K`) is rejected with
  [`SteamTablesError::SaturatedTpUnderdetermined`]: the internals route
  it to Region 4, where a single-phase `(T,p)` flash is physically
  under-determined without a steam quality (see
  `REGION_4_TP_UNDERDETERMINED` in `pt_flash_eqm`).

`(p,h)` flashes ([`try_t_ph_eqm`] etc.):

- `p` in `[p_sat(273.15 K) ≈ 611.213 Pa, 100 MPa]` — the upper edge is
  **inclusive**, matching `is_outside_pressure_range`, which rejects
  only `p > 100 MPa`. (It was exclusive here until `op-cv1c`: the
  internal 1073.15 K-isotherm bound helper
  `is_above_isotherm_t_1073_15` evaluates
  `h_tp_eqm_single_phase(1073.15 K, p)`, so the router's half-open
  100 MPa edge made *every* `(p,h)` call at exactly 100 MPa panic
  regardless of `h`. With the router fixed that call succeeds.);
- `h` in `[h(273.15 K, p), h(1073.15 K, p)]`, evaluated with the same
  Region-1 forward equation (`h_tp_1`) and single-phase `(T,p)` flash
  the internal validity check uses, so the accepted set is identical.
  `(p,h)` flashes into Region 5 (`T > 1073.15 K`) are unsupported by
  IAPWS-IF97 itself (no backward `(p,h)` correlation) and fall out of
  the `h` upper bound here.

Non-finite input (`NaN`, `±inf`) is rejected up front with
[`SteamTablesError::NonFinite`] — the internals' range comparisons are
silently `false` for `NaN`, which would otherwise let `NaN` states flow
through undetected.

## Other families covered by this facade

Three more families were added by bead `op-dt3.26`; each lives in its
own submodule with its own panic-site-to-gate table, and each is
re-exported here so `interfaces::checked::*` remains the single import:

- [`ps`] — the `(p,s)` flash family (11 functions plus the throat mass
  flux) and [`check_ps_envelope`]. Its pressure floor is **inclusive**,
  matching the `(p,h)` one: `p = p_sat(273.15 K)` exactly is accepted.
  It was exclusive until bead `op-znjx` (2026-08-11) — the `(p,s)`
  validity check used to evaluate `s_tp_eqm_single_phase` on the
  273.15 K isotherm and so hit the Region-4 trap at exactly that
  pressure; it now uses the Region-1 forward equation `s_tp_1`, as the
  `(p,h)` check already did with `h_tp_1`.
- [`two_phase`] — the quality-carrying `(T,p,x)` family (11 functions)
  and [`check_tpx_envelope`]. This one **accepts** saturation-line
  `(T,p)` pairs, which the single-phase gate above must reject, and it
  rejects a steam quality outside `[0, 1]` that the internals would
  silently clamp.
- [`control_volume`] — additive `Result`-returning constructors for
  [`crate::interfaces::object_oriented_programming::TampinesSteamTableCV`].

Two `(T,p)` and two `(p,h)` properties that the original facade did not
reach — the isobaric cubic expansion coefficient `alpha_v` and the
isothermal compressibility `kappa_T` — are wrapped below against the
existing validators.

## Known gaps (deliberately not gated)

Nothing here half-gates: a family with a panic this facade cannot
exclude is left out entirely rather than given a check that would claim
a safety it does not provide.

- **The `(h,s)` flash family** (`t_hs_eqm`, `p_hs_eqm`, `v_hs_eqm`,
  `x_hs_eqm`, `cp_hs_eqm`, `w_hs_eqm`, `kappa_hs_eqm`, `mu_hs_eqm`,
  `lambda_hs_eqm`, `tpvx_hs_flash_eqm`, `hs_flash_region`,
  `find_pressure_from_hs_region_4`) — 26 distinct `panic!`/`todo!`
  sites spread over nine entropy-band sub-routers, whose enthalpy
  bounds are themselves computed by `(p,s)` flashes, plus a bisection
  that panics when its bracket fails. Gating it faithfully means
  mirroring the whole band dispatch, not adding a bounds check.
- **`TampinesSteamTableCV::new_from_hs`** and the setter methods, for
  the same reason (see [`control_volume`]).
- **The choked-flow solvers** in
  [`crate::steam_turbine_equations::converging_diverging_nozzles`], and
  `TampinesSteamTableCV::get_crit_pressure_and_massflux` which calls
  them. Their panics — "unable to find bracket for critical mass flux
  root finding", the Joule-Thomson "bounds are same sign!" and "failed
  to converge" — are **iteration failures, not envelope violations**.
  No check on the stagnation state can exclude them without running the
  solver, so the honest fix is for those functions to return `Result`,
  not for a facade to pretend it can screen the input. The one piece
  that *is* an envelope check, the throat mass flux, is wrapped as
  [`ps::try_mass_flux_ps_eqm_throat`].
- **`pt_flash_metastable`** — its `mod.rs` is empty (zero lines); the
  metastable Region-2 equations live in
  `region_2_vapour::metastable_region_2` and are reached through
  `TampinesSteamTableCV::get_metastable_steam_*`, which already return
  `Option`. There is no panicking entry point in that module to gate.

## No `catch_unwind`

This facade uses **bounds-checking only** — no
`std::panic::catch_unwind`. Every reachable `panic!` site in the
wrapped call graph is an envelope violation (region-router fallthrough,
`(p,h)` validity check, Region-4/Region-5 dispatch arms), and all of
them are excluded by the checks above before the internals run.
Closed-form IAPWS polynomial evaluation inside the envelope does not
panic (it may lose accuracy near the critical point — see the crate
`CLAUDE.md` "Known accuracy pitfalls" — but degraded accuracy is
returned as a value, not a panic). This was confirmed by grep over
every `region_*` and `backward_eqn_*` module on 2026-08-11: zero
`panic!`, `todo!`, `unimplemented!`, `unwrap` or `expect` sites. The
whole panic surface of this crate's property layer lives in the
`interfaces/` routers.

```rust
pub mod checked { /* ... */ }
```

### Modules

## Module `ps`

The `(p,s)` flash family — see the module's own docs for its panic
trace and its **exclusive** pressure floor.
Bounds-checked `(p,s)` flash facade (bead `op-dt3.26`).

The unchecked `(p,s)` entry points in
[`crate::interfaces::functional_programming::ps_flash_eqm`] **panic** on
out-of-range input. This module wraps them with an explicit
validity-envelope check performed **before** any panicking internal is
called, returning [`SteamTablesError`] instead.

## Panic-trace: every reachable `panic!` and the gate that excludes it

Traced against the source on 2026-08-11. Every `(p,s)` entry point
starts by calling `ps_flash_region(p, s)`, which calls
`check_if_within_ps_validity_region(p, s)`; the region equations
themselves (`region_1_.../region_5_...`, `backward_eqn_ps_*`) contain no
`panic!`/`todo!`/`unwrap` at all, so the entire panic surface lives in
the router.

| Panic site | Condition | Excluded by |
|---|---|---|
| `ps_flash_eqm/validity_range.rs:21` "p,s point is lower than acceptable pressure range" | `p < p_sat(273.15 K)` | pressure lower gate |
| `ps_flash_eqm/validity_range.rs:26` "p,s point is higher than acceptable pressure range" | `p > 100 MPa` | pressure upper gate |
| `ps_flash_eqm/validity_range.rs:40`, `:60` "outside pressure range" | re-check of the two above | same two gates |
| `ps_flash_eqm/mod.rs:828` "p,s point is outside pressure range" | same predicate | same two gates |
| `ps_flash_eqm/mod.rs:833` "p,s point below 273.15K" | `s < s(273.15 K, p)` | entropy lower gate |
| `ps_flash_eqm/mod.rs:837` "p,s point above 1073.15K" | `s > s(1073.15 K, p)` | entropy upper gate |
| `pt_flash_eqm/mod.rs:198` "entropy: two-phase (T,p) ... under-determined" | formerly reached *inside* `is_below_isotherm_t_273_15` when `p == p_sat(273.15 K)` **bit-for-bit** | **no longer reachable**: that check now evaluates its lower bound with the Region-1 forward equation `s_tp_1` (bead `op-znjx`), and this facade mirrors it |
| `boundaries_between_single_phase_regions.rs:20,23,60,63,106` "p in (p,s) point is outside validity range" | `p` below 16.529 MPa / above 100 MPa inside the `p >= 16.529 MPa` branch | unreachable: that branch is only entered when `16.529 MPa <= p <= 100 MPa` already holds |
| `boundaries_from_single_phase_regions_to_region_4_multiphase.rs:30,33,75,78` "entropy of p,s point is outside validity range" | `s` outside the Region-3/4 boundary entropy band | unreachable inside the gate: the Region-1 and Region-2 tests immediately above them already returned for every `s` outside that band |
| `boundaries_from_single_phase_regions_to_region_4_multiphase.rs:123,151` "pressure of p,s point is outside validity range" | `p` outside `[p_sat(273.15 K), p_sat(623.15 K)]` in the `p < 16.529 MPa` branch | pressure gate plus the branch condition |
| ~~`ps_flash_eqm/mod.rs:64,154` `todo!("region 5 ps flash not implemented")`~~ **GONE 2026-09-27** | ~~`ps_flash_region` returning `Region5`~~ | ~~unreachable: `ps_flash_region` has no code path that returns `Region5`~~ **CORRECTED** — the panic sites no longer exist, and the premise was false: `ps_flash_region` *does* return `Region5`. See the note below. |

The last four rows are "unreachable" arguments rather than direct gates,
so they were checked empirically as well — see the V&V note in
[`super::tests`]: a 273 609-point sweep of the accepted set found zero
surviving panics across all ten wrapped functions.

### ~~Region 5 is unreachable from `ps_flash_region`~~ — **CORRECTED 2026-09-27**

The 2026-08-11 trace above was true when written and is now wrong in both
halves. Commit `2ab91fefc3` (2026-09-14, "(p,h), (p,s) and (h,s) now cover
Region 5") deleted both `todo!("region 5 ps flash not implemented")` arms and
gave `ps_flash_region` an explicit `is_above_isotherm_t_1073_15` branch that
**returns `FwdEqnRegion::Region5`**.

**What was verified by running** (not by reading the source):
`does_the_ps_flash_reach_region_5_and_does_the_checked_facade` in
`tests/boundary_273_15_repro.rs` builds a Region 5 state from the IAPWS
*forward* equation — `s_tp_eqm_single_phase(1500 K, 1 MPa) =
9.33359 kJ/(kg K)` — and reports `ps_flash_region(p, s) == Region5` and
`t_ps_eqm(p, s) = 1500.0000 K` (`|dT/T| = 6.0e-9`). `grep -rn 'todo!'` over
`ps_flash_eqm/` now returns nothing.

**This does not change what THIS facade accepts, and that is the point.**
[`check_ps_envelope`] still caps `s` at the **1073.15 K** isotherm, so the
same test measures `try_t_ps_eqm` at that state returning
`Err(OutOfRange { quantity: "specific entropy", value: 9333.5885,
min: -0.0884, max: 8502.3497, unit: "J/(kg K)" })`. Region 5 is therefore
reachable through the **unchecked** `(p,s)` entry points and refused by the
checked ones — the panic-surface argument in the table above survives, its
stated reason does not.

And, as with `lambda_ph_eqm`'s Region 5 correction: IAPWS-IF97 publishes **no**
backward `(p,s)` equation for Region 5, so the unchecked path's temperature is
this crate's own Chebyshev fit, verified only against the forward equations it
inverts. A dispatch that succeeds outside a correlation's published scope is an
extrapolation and must be read as one.

## The `p == p_sat(273.15 K)` trap — fixed 2026-08-11 (bead `op-znjx`)

This used to be a real asymmetry with the `(p,h)` facade. The `(p,h)`
validity check dodges the trap by evaluating its lower enthalpy bound
with the Region-1 forward equation `h_tp_1` instead of the `(T,p)`
router (see the note in `ph_flash_eqm/validity_range.rs`); the `(p,s)`
check used to call `s_tp_eqm_single_phase(273.15 K, p)`, which routes to
Region 4 and panics at exactly `p = p_sat(273.15 K)`, so this facade
carried an **exclusive** pressure floor where the `(p,h)` one is
inclusive.

`ps_flash_eqm/validity_range.rs::is_below_isotherm_t_273_15` now uses
`s_tp_1` for that bound, exactly as the `(p,h)` sibling uses `h_tp_1`.
The floor here is therefore **inclusive** again, matching `(p,h)`, and
[`check_ps_envelope`] mirrors the internal change by computing its own
lower entropy bound with `s_tp_1` too. (Precedent for this kind of
carve-out removal: bead `op-cv1c`, where fixing the region router's
half-open 100 MPa edge let this facade drop its matching carve-out.)

## No `catch_unwind`

This facade is a bounds check, not an exception handler. Non-finite
input (`NaN`, `+/-inf`) is rejected explicitly up front, because the
internals' range comparisons are silently `false` for `NaN` and would
otherwise let `NaN` states through undetected.

```rust
pub mod ps { /* ... */ }
```

### Functions

#### Function `check_ps_envelope`

Validates a `(p, s)` pair against the envelope the unchecked `(p,s)`
internals actually accept, returning `Ok(())` when every wrapped
`try_*_ps_*` function in this module is safe to call.

# Physical quantities and valid ranges

- `p` — absolute pressure, Pa. Valid in the **closed** interval
  `[p_sat(273.15 K), 100 MPa]`, i.e. from the triple-point saturation
  pressure (about 611.213 Pa) up to and including 100 MPa. Both edges
  are inclusive, matching `is_outside_pressure_range`, which rejects
  only `p < p_sat(273.15 K)` and `p > 100 MPa`. The lower edge was
  exclusive until bead `op-znjx` removed the Region-4 trap documented
  at module level.
- `s` — specific entropy, J/(kg K). Valid between the 273.15 K and
  1073.15 K isotherm entropies **at that pressure**, both edges
  inclusive. Each bound is evaluated with the same call the internal
  check uses — `s_tp_1` on the 273.15 K isotherm (which lies wholly in
  Region 1) and `s_tp_eqm_single_phase` on the 1073.15 K one — so the
  accepted set is identical rather than merely similar.

Non-finite input is rejected first with
[`SteamTablesError::NonFinite`].

```rust
pub fn check_ps_envelope(p: Pressure, s: SpecificHeatCapacity) -> super::Result<()> { /* ... */ }
```

#### Function `try_t_ps_eqm`

Checked temperature T (K) from a `(p,s)` flash (Regions 1-4). Valid range:
the `(p,s)` envelope in [`check_ps_envelope`]. Agrees exactly with
[`t_ps_eqm`] for in-range input.

~~Region 5 is not implemented for this flash path and lies outside the
entropy window.~~ **CORRECTED 2026-09-27** — the two halves of that sentence
had opposite fates, and the first was already false when it was written down:

- **"not implemented for this flash path" — STALE.** Commit `2ab91fefc3`
  (2026-09-14) implemented it. Measured by running
  `does_the_ps_flash_reach_region_5_and_does_the_checked_facade`
  (`tests/boundary_273_15_repro.rs`): from a Region 5 state built with the
  forward equation (`T = 1500 K`, `p = 1 MPa`,
  `s = 9.33359 kJ/(kg K)`), the unchecked `t_ps_eqm` returns
  **1500.0000 K**, `|dT/T| = 6.0e-9`.
- **"lies outside the entropy window" — STILL TRUE.** The same run has
  `try_t_ps_eqm` at that state returning `Err(OutOfRange { quantity:
  "specific entropy", value: 9333.5885, min: -0.0884, max: 8502.3497,
  unit: "J/(kg K)" })`, because [`check_ps_envelope`] evaluates its upper
  bound on the **1073.15 K** isotherm (`T_R5_LOWER_KELVIN`).

So a caller who needs Region 5 through `(p,s)` must use the unchecked
[`t_ps_eqm`] and accept that it *panics* rather than erring outside the
envelope — and must read the result as this crate's own Chebyshev fit, since
IAPWS-IF97 publishes no backward `(p,s)` equation for Region 5 at all.

```rust
pub fn try_t_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> super::Result<ThermodynamicTemperature> { /* ... */ }
```

#### Function `try_v_ps_eqm`

Checked specific volume v (m^3/kg) from a `(p,s)` flash (two-phase
states return the quality-weighted mixture volume). Valid range: the
`(p,s)` envelope in [`check_ps_envelope`]. Agrees exactly with
[`v_ps_eqm`] for in-range input.

```rust
pub fn try_v_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> super::Result<SpecificVolume> { /* ... */ }
```

#### Function `try_rho_ps_eqm`

Checked mass density rho (kg/m^3) from a `(p,s)` flash — the reciprocal
of [`try_v_ps_eqm`]. Valid range: the `(p,s)` envelope in
[`check_ps_envelope`].

```rust
pub fn try_rho_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> super::Result<MassDensity> { /* ... */ }
```

#### Function `try_h_ps_eqm`

Checked specific enthalpy h (J/kg) from a `(p,s)` flash. Valid range:
the `(p,s)` envelope in [`check_ps_envelope`]. Agrees exactly with
[`h_ps_eqm`] for in-range input.

```rust
pub fn try_h_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> super::Result<AvailableEnergy> { /* ... */ }
```

#### Function `try_x_ps_flash`

Checked steam quality x (dimensionless, bare `f64` to agree exactly with
the unchecked [`x_ps_flash`]): 0 for subcooled liquid, 1 for superheated
vapour, in `(0, 1)` inside the vapour-liquid dome. Valid range: the
`(p,s)` envelope in [`check_ps_envelope`].

```rust
pub fn try_x_ps_flash(p: Pressure, s: SpecificHeatCapacity) -> super::Result<f64> { /* ... */ }
```

#### Function `try_cp_ps_eqm`

Checked isobaric specific heat capacity cp (J/(kg K)) from a `(p,s)`
flash (two-phase states return a quality-interpolated estimate — see
[`cp_ps_eqm`]). Valid range: the `(p,s)` envelope in
[`check_ps_envelope`].

```rust
pub fn try_cp_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> super::Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_cv_ps_eqm`

Checked isochoric specific heat capacity cv (J/(kg K)) from a `(p,s)`
flash (two-phase states return a quality-interpolated estimate — see
[`cv_ps_eqm`]). Valid range: the `(p,s)` envelope in
[`check_ps_envelope`].

```rust
pub fn try_cv_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> super::Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_w_ps_wood_wallis`

Checked speed of sound w (m/s) from a `(p,s)` flash; in the two-phase
region this is the Wood/Wallis homogeneous-mixture sound speed. Valid
range: the `(p,s)` envelope in [`check_ps_envelope`]. Agrees exactly
with [`w_ps_wood_wallis`] for in-range input.

```rust
pub fn try_w_ps_wood_wallis(p: Pressure, s: SpecificHeatCapacity) -> super::Result<Velocity> { /* ... */ }
```

#### Function `try_kappa_ps_eqm`

Checked isentropic exponent kappa (dimensionless `Ratio`) from a `(p,s)`
flash. Valid range: the `(p,s)` envelope in [`check_ps_envelope`].
Agrees exactly with [`kappa_ps_eqm`] for in-range input.

```rust
pub fn try_kappa_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> super::Result<Ratio> { /* ... */ }
```

#### Function `try_alpha_v_ps_eqm`

Checked isobaric cubic expansion coefficient alpha_v (1/K) from a
`(p,s)` flash. Valid range: the `(p,s)` envelope in
[`check_ps_envelope`]. Agrees exactly with [`alpha_v_ps_eqm`] for
in-range input.

```rust
pub fn try_alpha_v_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> super::Result<TemperatureCoefficient> { /* ... */ }
```

#### Function `try_kappa_t_ps_eqm`

Checked isothermal compressibility kappa_T (1/Pa) from a `(p,s)` flash.
Valid range: the `(p,s)` envelope in [`check_ps_envelope`]. Agrees
exactly with [`kappa_t_ps_eqm`] for in-range input.

```rust
pub fn try_kappa_t_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> super::Result<crate::region_1_subcooled_liquid::InversePressure> { /* ... */ }
```

#### Function `try_mass_flux_ps_eqm_throat`

Checked HEM critical mass flux G (kg/(m^2 s)) evaluated **at throat
conditions** `(p, s)` — not stagnation conditions. Valid range: a
**stricter** envelope than [`check_ps_envelope`], because the unchecked
[`mass_flux_ps_eqm_throat`] differentiates `v(p,s)` by finite difference
and therefore evaluates the `(p,s)` flash at *three* pressures.

# Panic-trace

Beyond the `(p,s)` panics listed at module level, the unchecked function
reaches them again at the perturbed pressures:

- `v_ps_eqm(p + p * 1e-5, s_adjusted)` — panics for `p` within
  `1e-5` relative of 100 MPa (the step walks over the ceiling), and for
  `s` within a step of the 1073.15 K isotherm (raising the pressure
  lowers that isotherm's entropy). Excluded by re-running
  [`check_ps_envelope`] at `p + dp`.
- `v_ps_eqm(max(p - p * 1e-5, 611.823 Pa), s_adjusted)` — same, at the
  lower edge. Excluded by re-running [`check_ps_envelope`] at that
  clamped pressure.
- `s_tp_eqm_two_phase(sat_temp_4(p), p, 0.0 | 1.0)`, used to locate the
  bubble point for the near-saturation entropy adjustment. Excluded by
  [`check_tpx_envelope`] at `(sat_temp_4(p), p, 0)`; the `x = 1` call is
  the same `(T,p)` point and both qualities are inside `[0, 1]`.

The entropy actually passed to the perturbed flashes is the *adjusted*
entropy the unchecked function computes (it snaps entropies just below
the bubble point up to a quality of `1e-4`), reproduced here exactly so
the checked bound is the one the internals will use.

Units: `p` in Pa, `s` in J/(kg K), result in kg/(m^2 s).

```rust
pub fn try_mass_flux_ps_eqm_throat(p: Pressure, s: SpecificHeatCapacity) -> super::Result<MassFlux> { /* ... */ }
```

## Module `two_phase`

The quality-carrying two-phase `(T,p,x)` flash family — see the
module's own docs for its panic trace.
Bounds-checked two-phase `(T, p, x)` flash facade (bead `op-dt3.26`).

The `*_tp_eqm_two_phase` family in
[`crate::interfaces::functional_programming::pt_flash_eqm::multiphase_flashing`]
is the quality-carrying counterpart of the single-phase `(T,p)` flashes:
it resolves saturation-line states that
[`super::check_tp_single_phase_envelope`] has to reject as
under-determined. These functions **panic** on out-of-envelope `(T,p)`,
and silently *clamp* an out-of-range steam quality; this module rejects
both before the internals run.

## Panic-trace: every reachable `panic!` and the gate that excludes it

Traced against the source on 2026-08-11. Each `*_tp_eqm_two_phase`
function begins with `region_fwd_eqn_two_phase(t, p, x)` and then
`match`es on the returned region. Unlike the single-phase family, the
`Region4` arm is **implemented** in every one of these functions, so the
`REGION_4_TP_UNDERDETERMINED` panics do not apply here.

| Panic site | Condition | Excluded by |
|---|---|---|
| `pt_flash_eqm/mod.rs:157` "t,p flashing at eqm out of bounds!" | `region_fwd_eqn_two_phase` falls through to `region_fwd_eqn_single_phase` (i.e. `(T,p)` is off the saturation line, or above the critical temperature/pressure) with `(T,p)` outside the IF97 envelope | the `(T,p)` envelope gate below: `T` in `[273.15 K, 2273.15 K]`, `0 < p <= 100 MPa` for `T <= 1073.15 K`, `0 < p <= 50 MPa` above it |
| `pt_flash_eqm/mod.rs:171,184,198,211,225,242,256,321,334,348` `REGION_4_TP_UNDERDETERMINED` | the single-phase `(T,p)` functions' Region-4 arms | **not reachable from this family** — these functions never call the single-phase property functions; they call `region_fwd_eqn_single_phase` (the router) only, and handle `Region4` themselves |

The Region-3 arms call the near-saturation backward volume equations
(`v_tp_3c`, `v_tp_3r`, `v_tp_3s`, `v_tp_3t`, `v_tp_3u`, `v_tp_3x`,
`v_tp_3y`, `v_tp_3z`) and `h_rho_t_3`/`s_rho_t_3`/... at Region-3 points
that may lie far from the saturation line. Those are closed-form
polynomial evaluations containing no `panic!`, `todo!`, `unwrap` or
`expect` (verified by grep over `region_1_..` through `region_5_..` and
`backward_eqn_*`, which return zero hits), so they can lose accuracy but
cannot panic — and the result is discarded unless the point really is
near the saturation line.

## Steam quality is rejected, not clamped

`region_fwd_eqn_two_phase` and each Region-3/Region-4 arm silently clamp
`x` into `[0, 1]`. That is a correctness hazard rather than a panic: a
caller passing `x = 1.7` gets a saturated-vapour answer with no
indication that the input was nonsense. The checked facade rejects
`x < 0` and `x > 1` with [`SteamTablesError::QualityOutOfRange`], while
**accepting `x = 0` and `x = 1` exactly** — those are the physically
meaningful bubble- and dew-point states the internals route to
Region 1/2 (below 623.15 K) or Region 3 (above it).

`NaN` quality is worse: it survives every clamp (`NaN < 0.0` and
`NaN > 1.0` are both `false`), fails the `x == 0.0` / `x == 1.0`
bubble/dew tests, and propagates into the mixture weighting to return a
silent `NaN`. It is rejected explicitly as
[`SteamTablesError::NonFinite`].

## Difference from the single-phase `(T,p)` gate

[`check_tpx_envelope`] is deliberately **not**
[`super::check_tp_single_phase_envelope`] plus a quality check: it omits
that gate's `SaturatedTpUnderdetermined` rejection. A `(T, p_sat(T))`
pair is exactly what this family exists to evaluate, and accepting it is
the whole point.

## No `catch_unwind`

This facade is a bounds check, not an exception handler.

```rust
pub mod two_phase { /* ... */ }
```

### Functions

#### Function `check_tpx_envelope`

Validates a `(T, p, x)` triple against the envelope the unchecked
two-phase `(T,p,x)` internals actually accept, returning `Ok(())` when
every wrapped `try_*_tp_eqm_two_phase` function is safe to call.

# Physical quantities and valid ranges

- `t` — thermodynamic temperature, K. Valid in `[273.15, 2273.15]`.
- `p` — absolute pressure, Pa. Valid in `(0, 100 MPa]` for `t` in
  `[273.15 K, 1073.15 K]`, and in `(0, 50 MPa]` for `t` above
  1073.15 K (IF97 Region 5). Both ceilings are **inclusive**; the floor
  is exclusive at 0 (vacuum has no IF97 state).
- `x` — steam quality, i.e. vapour mass fraction, dimensionless. Valid
  in `[0, 1]`, **both edges inclusive**. It only affects the answer for
  `(T,p)` pairs on (or within `5e-4` relative pressure of) the
  saturation line; elsewhere the underlying single-phase equations
  ignore it.

Unlike [`super::check_tp_single_phase_envelope`] this check **accepts**
saturation-line `(T,p)` pairs — resolving them is what the two-phase
family is for.

Non-finite input is rejected first with
[`SteamTablesError::NonFinite`].

```rust
pub fn check_tpx_envelope(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<()> { /* ... */ }
```

#### Function `try_h_tp_eqm_two_phase`

Checked specific enthalpy h (J/kg) from a two-phase-aware `(T,p,x)`
flash. Valid range: the `(T,p,x)` envelope in [`check_tpx_envelope`]
(saturation-line pairs accepted; `x` in `[0, 1]`). Agrees exactly with
[`h_tp_eqm_two_phase`] for in-range input.

```rust
pub fn try_h_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<AvailableEnergy> { /* ... */ }
```

#### Function `try_u_tp_eqm_two_phase`

Checked specific internal energy u (J/kg) from a two-phase-aware
`(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
[`check_tpx_envelope`]. Agrees exactly with [`u_tp_eqm_two_phase`] for
in-range input.

```rust
pub fn try_u_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<AvailableEnergy> { /* ... */ }
```

#### Function `try_s_tp_eqm_two_phase`

Checked specific entropy s (J/(kg K)) from a two-phase-aware `(T,p,x)`
flash. Valid range: the `(T,p,x)` envelope in [`check_tpx_envelope`].
Agrees exactly with [`s_tp_eqm_two_phase`] for in-range input.

```rust
pub fn try_s_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_cp_tp_eqm_two_phase`

Checked isobaric specific heat capacity cp (J/(kg K)) from a
two-phase-aware `(T,p,x)` flash (two-phase states return a
quality-weighted mixture value). Valid range: the `(T,p,x)` envelope in
[`check_tpx_envelope`]. Agrees exactly with [`cp_tp_eqm_two_phase`] for
in-range input.

```rust
pub fn try_cp_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_cv_tp_eqm_two_phase`

Checked isochoric specific heat capacity cv (J/(kg K)) from a
two-phase-aware `(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
[`check_tpx_envelope`]. Agrees exactly with [`cv_tp_eqm_two_phase`] for
in-range input.

```rust
pub fn try_cv_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_v_tp_eqm_two_phase`

Checked specific volume v (m^3/kg) from a two-phase-aware `(T,p,x)`
flash (two-phase states return the quality-weighted mixture volume).
Valid range: the `(T,p,x)` envelope in [`check_tpx_envelope`]. Agrees
exactly with [`v_tp_eqm_two_phase`] for in-range input.

```rust
pub fn try_v_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<SpecificVolume> { /* ... */ }
```

#### Function `try_rho_tp_eqm_two_phase`

Checked mass density rho (kg/m^3) from a two-phase-aware `(T,p,x)`
flash — the reciprocal of [`try_v_tp_eqm_two_phase`]. Valid range: the
`(T,p,x)` envelope in [`check_tpx_envelope`].

```rust
pub fn try_rho_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<MassDensity> { /* ... */ }
```

#### Function `try_w_tp_eqm_two_phase`

Checked speed of sound w (m/s) from a two-phase-aware `(T,p,x)` flash.
Valid range: the `(T,p,x)` envelope in [`check_tpx_envelope`]. Agrees
exactly with [`w_tp_eqm_two_phase`] for in-range input.

```rust
pub fn try_w_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<Velocity> { /* ... */ }
```

#### Function `try_kappa_tp_eqm_two_phase`

Checked isentropic exponent kappa (dimensionless `Ratio`) from a
two-phase-aware `(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
[`check_tpx_envelope`]. Agrees exactly with [`kappa_tp_eqm_two_phase`]
for in-range input.

```rust
pub fn try_kappa_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<Ratio> { /* ... */ }
```

#### Function `try_alpha_v_tp_eqm_two_phase`

Checked isobaric cubic expansion coefficient alpha_v (1/K) from a
two-phase-aware `(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
[`check_tpx_envelope`]. Agrees exactly with
[`alpha_v_tp_eqm_two_phase`] for in-range input.

```rust
pub fn try_alpha_v_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<TemperatureCoefficient> { /* ... */ }
```

#### Function `try_kappa_t_tp_eqm_two_phase`

Checked isothermal compressibility kappa_T (1/Pa) from a
two-phase-aware `(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
[`check_tpx_envelope`]. Agrees exactly with
`multiphase_flashing::kappa_t_tp_eqm` for in-range input.

```rust
pub fn try_kappa_t_tp_eqm_two_phase(t: ThermodynamicTemperature, p: Pressure, x: f64) -> super::Result<crate::region_1_subcooled_liquid::InversePressure> { /* ... */ }
```

## Module `control_volume`

Additive `Result`-returning constructors for the object-oriented
`TampinesSteamTableCV` control volume.
Bounds-checked constructors for the object-oriented
[`TampinesSteamTableCV`] control volume (bead `op-dt3.26`).

[`TampinesSteamTableCV`]'s `new_from_*` constructors are thin composers
over the functional-programming flashes, so they inherit every panic
those flashes have. The free functions here are **additive**: they gate
the same inputs with the validators in this module's siblings and then
call the existing constructor unchanged. No existing signature is
touched, and the struct's fields stay private — these wrappers add a
`Result` entry point, they do not re-implement the flash.

## Panic-trace: constructor to gate

Traced against `interfaces/object_oriented_programming/mod.rs` on
2026-08-11. Each row names the internal calls the constructor makes and
the validator that excludes their panics.

| Constructor | Internal calls | Gate |
|---|---|---|
| `new_from_tp_quality(t, p, volume, x)` | `v_tp_eqm_two_phase`, `h_tp_eqm_two_phase`, `s_tp_eqm_two_phase` | [`check_tpx_envelope`] — see [`super::two_phase`] for that family's full panic trace |
| `new_from_tp_quality_1(t, p, volume)` | same, with `x = 1` | [`check_tpx_envelope`] at `x = 1` |
| `new_from_tp_quality_0(t, p, volume)` | same, with `x = 0` | [`check_tpx_envelope`] at `x = 0` |
| `new_from_ph(p, h, volume)` | `t_ph_eqm`, `v_ph_eqm`, `s_ph_eqm` | [`super::check_ph_envelope`] |
| `new_from_ps(p, s, volume)` | `t_ps_eqm`, `v_ps_eqm`, `h_ps_eqm` | [`check_ps_envelope`] — see [`super::ps`] |
| `new_from_sat_pressure_quality(p, x, volume)` | `sat_temp_4(p)`, then `new_from_tp_quality` | [`check_tpx_envelope`] at `(sat_temp_4(p), p, x)`; `sat_temp_4` itself is a closed-form correlation with no panic site |
| `new_from_sat_temp_quality(t, x, volume)` | `sat_pressure_4(t)`, then `new_from_tp_quality` | [`check_tpx_envelope`] at `(t, sat_pressure_4(t), x)`; `sat_pressure_4` is likewise panic-free |

## Not covered here

- **`new_from_hs`** — it resolves pressure with `hs_flash_eqm::p_hs_eqm`
  first, so it inherits the `(h,s)` flash's panic surface, which this
  module does not yet gate. It is deliberately left out rather than
  given a gate that would miss those panics.
- **The setter methods and the getters** (`set_tpx`, `set_ph`,
  `set_ps`, `compress_isentropically`, `get_crit_pressure_and_massflux`,
  ...) — the state-changing ones inherit the same flash panics, and
  `get_crit_pressure_and_massflux` additionally reaches the choked-flow
  root finder, whose panic is a *convergence* failure that no input
  bounds check can exclude.

## No `catch_unwind`

This facade is a bounds check, not an exception handler.

```rust
pub mod control_volume { /* ... */ }
```

### Functions

#### Function `try_new_from_tp_quality`

Checked [`TampinesSteamTableCV::new_from_tp_quality`]: builds a control
volume from a two-phase-aware `(T, p, x)` flash.

Inputs: `temperature` in K (valid 273.15-2273.15), `pressure` in Pa
(valid up to and including 100 MPa below 1073.15 K, 50 MPa above),
`volume` the fixed control-volume size in m^3 (not validated — any
finite volume is geometrically meaningful and no internal reads it
during the flash), and `x` the steam quality (vapour mass fraction,
valid in `[0, 1]` inclusive). Saturation-line `(T,p)` pairs are
accepted: resolving them with an explicit quality is the point of this
constructor.

```rust
pub fn try_new_from_tp_quality(temperature: ThermodynamicTemperature, pressure: Pressure, volume: Volume, x: f64) -> super::Result<crate::interfaces::object_oriented_programming::TampinesSteamTableCV> { /* ... */ }
```

#### Function `try_new_from_tp_quality_1`

Checked [`TampinesSteamTableCV::new_from_tp_quality_1`]: builds a
control volume from a `(T, p)` flash with steam quality fixed at 1
(saturated vapour / dew point on the saturation line, ignored
elsewhere). Same `(T,p)` envelope as [`try_new_from_tp_quality`].

```rust
pub fn try_new_from_tp_quality_1(temperature: ThermodynamicTemperature, pressure: Pressure, volume: Volume) -> super::Result<crate::interfaces::object_oriented_programming::TampinesSteamTableCV> { /* ... */ }
```

#### Function `try_new_from_tp_quality_0`

Checked [`TampinesSteamTableCV::new_from_tp_quality_0`]: builds a
control volume from a `(T, p)` flash with steam quality fixed at 0
(saturated liquid / bubble point on the saturation line, ignored
elsewhere). Same `(T,p)` envelope as [`try_new_from_tp_quality`].

```rust
pub fn try_new_from_tp_quality_0(temperature: ThermodynamicTemperature, pressure: Pressure, volume: Volume) -> super::Result<crate::interfaces::object_oriented_programming::TampinesSteamTableCV> { /* ... */ }
```

#### Function `try_new_from_ph`

Checked [`TampinesSteamTableCV::new_from_ph`]: builds a control volume
from a `(p, h)` flash.

Inputs: `p` in Pa (valid `[p_sat(273.15 K), 100 MPa]`, both edges
inclusive), `h` in J/kg (valid between the 273.15 K and 1073.15 K
isotherm enthalpies at that pressure), `volume` in m^3. See
[`super::check_ph_envelope`] for the exact bounds.

```rust
pub fn try_new_from_ph(p: Pressure, h: AvailableEnergy, volume: Volume) -> super::Result<crate::interfaces::object_oriented_programming::TampinesSteamTableCV> { /* ... */ }
```

#### Function `try_new_from_ps`

Checked [`TampinesSteamTableCV::new_from_ps`]: builds a control volume
from a `(p, s)` flash.

Inputs: `p` in Pa (valid `(p_sat(273.15 K), 100 MPa]` — note the
**exclusive** lower edge, see [`super::ps`]), `s` in J/(kg K) (valid
between the 273.15 K and 1073.15 K isotherm entropies at that
pressure), `volume` in m^3.

```rust
pub fn try_new_from_ps(p: Pressure, s: SpecificHeatCapacity, volume: Volume) -> super::Result<crate::interfaces::object_oriented_programming::TampinesSteamTableCV> { /* ... */ }
```

#### Function `try_new_from_sat_pressure_quality`

Checked [`TampinesSteamTableCV::new_from_sat_pressure_quality`]: builds
a saturation-line control volume from saturation pressure and quality.

Inputs: `p` the saturation pressure in Pa, `x` the steam quality
(dimensionless, valid `[0, 1]` inclusive), `volume` in m^3. The
saturation temperature is looked up with `sat_temp_4(p)` and the
resulting `(T, p, x)` triple is validated with
[`check_tpx_envelope`] — so a pressure whose saturation temperature
falls outside `[273.15 K, 2273.15 K]` is rejected here rather than
panicking downstream.

```rust
pub fn try_new_from_sat_pressure_quality(p: Pressure, x: f64, volume: Volume) -> super::Result<crate::interfaces::object_oriented_programming::TampinesSteamTableCV> { /* ... */ }
```

#### Function `try_new_from_sat_temp_quality`

Checked [`TampinesSteamTableCV::new_from_sat_temp_quality`]: builds a
saturation-line control volume from saturation temperature and quality.

Inputs: `t` the saturation temperature in K, `x` the steam quality
(dimensionless, valid `[0, 1]` inclusive), `volume` in m^3. The
saturation pressure is looked up with `sat_pressure_4(t)` and the
resulting `(T, p, x)` triple is validated with
[`check_tpx_envelope`].

```rust
pub fn try_new_from_sat_temp_quality(t: ThermodynamicTemperature, x: f64, volume: Volume) -> super::Result<crate::interfaces::object_oriented_programming::TampinesSteamTableCV> { /* ... */ }
```

### Types

#### Enum `SteamTablesError`

Error type for the bounds-checked IF97 facade.

Every variant carries the offending raw SI value so a caller (or a log
line) can see exactly which input broke which bound without re-deriving
units: temperatures in kelvin, pressures in pascal, specific enthalpies
in J/kg.

```rust
pub enum SteamTablesError {
    NonFinite {
        quantity: &'static str,
        value: f64,
        unit: &'static str,
    },
    OutOfRange {
        quantity: &'static str,
        value: f64,
        min: f64,
        max: f64,
        unit: &'static str,
    },
    SaturatedTpUnderdetermined {
        t_kelvin: f64,
        p_pascal: f64,
    },
    QualityOutOfRange {
        x: f64,
    },
}
```

##### Variants

###### `NonFinite`

An input was `NaN` or infinite. The unchecked internals cannot
detect this (range comparisons on `NaN` are silently `false`), so
it is rejected here before anything else.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `quantity` | `&'static str` | Which input quantity was non-finite (e.g. `"temperature"`). |
| `value` | `f64` | The offending raw value in the SI unit named by `unit`. |
| `unit` | `&'static str` | SI unit of `value` as prose (e.g. `"K"`, `"Pa"`, `"J/kg"`). |

###### `OutOfRange`

An input lies outside the IAPWS-IF97 validity envelope enforced by
this facade (see the module-level doc for the exact bounds,
including which edges are exclusive).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `quantity` | `&'static str` | Which input quantity broke the bound (e.g. `"pressure"`). |
| `value` | `f64` | The offending raw value in the SI unit named by `unit`. |
| `min` | `f64` | Lower bound of the valid range, same unit. |
| `max` | `f64` | Upper bound of the valid range, same unit. May be an exclusive<br>edge — see the module-level doc. |
| `unit` | `&'static str` | SI unit of `value`/`min`/`max` as prose (e.g. `"K"`, `"Pa"`). |

###### `SaturatedTpUnderdetermined`

The `(T, p)` pair lies exactly on the saturation line
(`p == p_sat(T)` bit-for-bit, `T < 647.096 K`). A single-phase
`(T,p)` flash cannot resolve a two-phase state — the steam quality
is a free variable there. Use a `(p,h)` flash (which carries the
quality) instead.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_kelvin` | `f64` | Temperature of the rejected state, K. |
| `p_pascal` | `f64` | Pressure of the rejected state (equal to `p_sat(T)`), Pa. |

###### `QualityOutOfRange`

The steam quality (vapour mass fraction) lies outside `[0, 1]`.

This is not a panic the internals raise — the two-phase `(T,p,x)`
functions silently **clamp** an out-of-range quality, so a caller
passing `x = 1.7` gets a saturated-vapour answer with no signal that
the input was nonsense. The checked facade rejects it instead.
`x = 0` (bubble point) and `x = 1` (dew point) are valid and
accepted.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` | The offending quality, dimensionless (vapour mass fraction). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
#### Type Alias `Result`

Convenience alias: every checked facade function returns
`Result<uom quantity, SteamTablesError>`.

```rust
pub type Result<T> = core::result::Result<T, SteamTablesError>;
```

### Functions

#### Function `check_tp_single_phase_envelope`

Validates a `(T, p)` pair against the IF97 single-phase envelope the
unchecked `(T,p)` internals actually accept (module-level doc has the
full bound list). Returns `Ok(())` when every wrapped
`try_*_tp_eqm_single_phase` function is safe to call.

Inputs: `t` is a thermodynamic temperature (valid 273.15-2273.15 K),
`p` an absolute pressure (valid up to and including 100 MPa from
273.15 K to 1073.15 K, up to and including 50 MPa above 1073.15 K).

```rust
pub fn check_tp_single_phase_envelope(t: ThermodynamicTemperature, p: Pressure) -> Result<()> { /* ... */ }
```

#### Function `check_ph_envelope`

Validates a `(p, h)` pair against the envelope the unchecked `(p,h)`
internals actually accept: `p` in `[p_sat(273.15 K), 100 MPa]` (both
edges inclusive, matching `is_outside_pressure_range`) and `h` between
the 273.15 K and 1073.15 K isotherm enthalpies at that pressure.
Returns `Ok(())` when every wrapped `try_*_ph_*` function is safe to
call.

Inputs: `p` is an absolute pressure (Pa), `h` a specific enthalpy
(J/kg); the valid `h` window is pressure-dependent and is reported in
the error when violated.

```rust
pub fn check_ph_envelope(p: Pressure, h: AvailableEnergy) -> Result<()> { /* ... */ }
```

#### Function `try_h_tp_eqm_single_phase`

Checked specific enthalpy h (J/kg) from a single-phase `(T,p)` flash.
Valid range: the `(T,p)` envelope in the module doc (T 273.15-2273.15 K
with the band-dependent pressure ceiling); exact saturation-line pairs
are rejected. Agrees exactly with
[`h_tp_eqm_single_phase`] for in-range input.

```rust
pub fn try_h_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<AvailableEnergy> { /* ... */ }
```

#### Function `try_u_tp_eqm_single_phase`

Checked specific internal energy u (J/kg) from a single-phase `(T,p)`
flash. Valid range: the `(T,p)` envelope in the module doc. Agrees
exactly with [`u_tp_eqm_single_phase`] for in-range input.

```rust
pub fn try_u_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<AvailableEnergy> { /* ... */ }
```

#### Function `try_s_tp_eqm_single_phase`

Checked specific entropy s (J/(kg K)) from a single-phase `(T,p)`
flash. Valid range: the `(T,p)` envelope in the module doc. Agrees
exactly with [`s_tp_eqm_single_phase`] for in-range input.

```rust
pub fn try_s_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_cp_tp_eqm_single_phase`

Checked isobaric specific heat capacity cp (J/(kg K)) from a
single-phase `(T,p)` flash. Valid range: the `(T,p)` envelope in the
module doc. Agrees exactly with [`cp_tp_eqm_single_phase`] for
in-range input.

```rust
pub fn try_cp_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_cv_tp_eqm_single_phase`

Checked isochoric specific heat capacity cv (J/(kg K)) from a
single-phase `(T,p)` flash. Valid range: the `(T,p)` envelope in the
module doc. Agrees exactly with [`cv_tp_eqm_single_phase`] for
in-range input.

```rust
pub fn try_cv_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_v_tp_eqm_single_phase`

Checked specific volume v (m^3/kg) from a single-phase `(T,p)` flash.
Valid range: the `(T,p)` envelope in the module doc. Agrees exactly
with [`v_tp_eqm_single_phase`] for in-range input.

```rust
pub fn try_v_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<SpecificVolume> { /* ... */ }
```

#### Function `try_rho_tp_eqm_single_phase`

Checked mass density rho (kg/m^3) from a single-phase `(T,p)` flash —
the reciprocal of [`try_v_tp_eqm_single_phase`]. Valid range: the
`(T,p)` envelope in the module doc.

```rust
pub fn try_rho_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<MassDensity> { /* ... */ }
```

#### Function `try_w_tp_eqm_single_phase`

Checked speed of sound w (m/s) from a single-phase `(T,p)` flash.
Valid range: the `(T,p)` envelope in the module doc. Agrees exactly
with [`w_tp_eqm_single_phase`] for in-range input.

```rust
pub fn try_w_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<Velocity> { /* ... */ }
```

#### Function `try_kappa_tp_eqm_single_phase`

Checked isentropic exponent kappa (dimensionless `Ratio`) from a
single-phase `(T,p)` flash. Valid range: the `(T,p)` envelope in the
module doc. Agrees exactly with [`kappa_tp_eqm_single_phase`] for
in-range input.

```rust
pub fn try_kappa_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<Ratio> { /* ... */ }
```

#### Function `try_mu_tp_eqm_single_phase`

Checked dynamic viscosity mu (Pa s, IAPWS R12-08 fast path without the
critical enhancement) from a single-phase `(T,p)` flash. Valid range:
the `(T,p)` envelope in the module doc. Agrees exactly with
[`mu_tp_eqm_single_phase`] for in-range input.

```rust
pub fn try_mu_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<DynamicViscosity> { /* ... */ }
```

#### Function `try_lambda_tp_eqm_single_phase`

Checked thermal conductivity lambda (W/(m K), IAPWS R15-11) from a
single-phase `(T,p)` flash. Valid range: the `(T,p)` envelope in the
module doc. Agrees exactly with [`lambda_tp_eqm_single_phase`] for
in-range input.

```rust
pub fn try_lambda_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<ThermalConductivity> { /* ... */ }
```

#### Function `try_alpha_v_tp_eqm_single_phase`

Checked isobaric cubic expansion coefficient alpha_v (1/K) from a
single-phase `(T,p)` flash — the thermal expansivity
`(1/v)(dv/dT)_p`. Valid range: the `(T,p)` envelope in the module doc.
Agrees exactly with [`alpha_v_tp_eqm_single_phase`] for in-range input.

Panic-trace: `alpha_v_tp_eqm_single_phase` calls
`region_fwd_eqn_single_phase` (fallthrough `panic!` at
`pt_flash_eqm/mod.rs:157`, excluded by the `(T,p)` envelope gate) and
panics on its Region-4 arm (`pt_flash_eqm/mod.rs:334`, excluded by the
exact-saturation-line rejection). The per-region kernels
`alpha_v_tp_1/2/3/5` are panic-free.

```rust
pub fn try_alpha_v_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<TemperatureCoefficient> { /* ... */ }
```

#### Function `try_kappa_t_tp_eqm`

Checked isothermal compressibility kappa_T (1/Pa) from a single-phase
`(T,p)` flash — `-(1/v)(dv/dp)_T`. Valid range: the `(T,p)` envelope in
the module doc. Agrees exactly with [`kappa_t_tp_eqm`] for in-range
input.

Panic-trace: identical to [`try_alpha_v_tp_eqm_single_phase`], with the
Region-4 arm at `pt_flash_eqm/mod.rs:348`.

```rust
pub fn try_kappa_t_tp_eqm(t: ThermodynamicTemperature, p: Pressure) -> Result<crate::region_1_subcooled_liquid::InversePressure> { /* ... */ }
```

#### Function `try_t_ph_eqm`

Checked temperature T (K) from a `(p,h)` flash (Regions 1-4; Region 5
has no IAPWS-IF97 backward `(p,h)` correlation and lies outside the
enthalpy window). Valid range: the `(p,h)` envelope in the module doc.
Agrees exactly with [`t_ph_eqm`] for in-range input.

```rust
pub fn try_t_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<ThermodynamicTemperature> { /* ... */ }
```

#### Function `try_v_ph_eqm`

Checked specific volume v (m^3/kg) from a `(p,h)` flash (two-phase
states return the quality-weighted mixture volume). Valid range: the
`(p,h)` envelope in the module doc. Agrees exactly with [`v_ph_eqm`]
for in-range input.

```rust
pub fn try_v_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<SpecificVolume> { /* ... */ }
```

#### Function `try_rho_ph_eqm`

Checked mass density rho (kg/m^3) from a `(p,h)` flash — the
reciprocal of [`try_v_ph_eqm`]. Valid range: the `(p,h)` envelope in
the module doc.

```rust
pub fn try_rho_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<MassDensity> { /* ... */ }
```

#### Function `try_u_ph_eqm`

Checked specific internal energy u (J/kg) from a `(p,h)` flash. Valid
range: the `(p,h)` envelope in the module doc. Agrees exactly with
[`u_ph_eqm`] for in-range input.

```rust
pub fn try_u_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<AvailableEnergy> { /* ... */ }
```

#### Function `try_s_ph_eqm`

Checked specific entropy s (J/(kg K)) from a `(p,h)` flash. Valid
range: the `(p,h)` envelope in the module doc. Agrees exactly with
[`s_ph_eqm`] for in-range input.

```rust
pub fn try_s_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_cp_ph_eqm`

Checked isobaric specific heat capacity cp (J/(kg K)) from a `(p,h)`
flash (two-phase states return a quality-interpolated estimate — see
[`cp_ph_eqm`]). Valid range: the `(p,h)` envelope in the module doc.
Agrees exactly with [`cp_ph_eqm`] for in-range input.

```rust
pub fn try_cp_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_cv_ph_eqm`

Checked isochoric specific heat capacity cv (J/(kg K)) from a `(p,h)`
flash (two-phase states return a quality-interpolated estimate — see
[`cv_ph_eqm`]). Valid range: the `(p,h)` envelope in the module doc.
Agrees exactly with [`cv_ph_eqm`] for in-range input.

```rust
pub fn try_cv_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<SpecificHeatCapacity> { /* ... */ }
```

#### Function `try_w_ph_wood_wallis`

Checked speed of sound w (m/s) from a `(p,h)` flash; in the two-phase
region this is the Wood/Wallis homogeneous-mixture sound speed. Valid
range: the `(p,h)` envelope in the module doc. Agrees exactly with
[`w_ph_wood_wallis`] for in-range input.

```rust
pub fn try_w_ph_wood_wallis(p: Pressure, h: AvailableEnergy) -> Result<Velocity> { /* ... */ }
```

#### Function `try_kappa_ph_eqm`

Checked isentropic exponent kappa (dimensionless `Ratio`) from a
`(p,h)` flash. Valid range: the `(p,h)` envelope in the module doc.
Agrees exactly with [`kappa_ph_eqm`] for in-range input.

```rust
pub fn try_kappa_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<Ratio> { /* ... */ }
```

#### Function `try_x_ph_flash`

Checked steam quality x (dimensionless, bare `f64` to agree exactly
with the unchecked [`x_ph_flash`]): 0 for subcooled liquid, 1 for
superheated vapour, in `(0, 1)` inside the vapour-liquid dome. Valid
range: the `(p,h)` envelope in the module doc.

```rust
pub fn try_x_ph_flash(p: Pressure, h: AvailableEnergy) -> Result<f64> { /* ... */ }
```

#### Function `try_mu_ph_eqm`

Checked dynamic viscosity mu (Pa s, IAPWS R12-08 fast path) from a
`(p,h)` flash (two-phase states use the HEM-mixture density). Valid
range: the `(p,h)` envelope in the module doc. Agrees exactly with
[`mu_ph_eqm`] for in-range input.

```rust
pub fn try_mu_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<DynamicViscosity> { /* ... */ }
```

#### Function `try_lambda_ph_eqm`

Checked thermal conductivity lambda (W/(m K), IAPWS R15-11) from a
`(p,h)` flash. Valid range: the `(p,h)` envelope in the module doc.
Agrees exactly with [`lambda_ph_eqm`] for in-range input.

```rust
pub fn try_lambda_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<ThermalConductivity> { /* ... */ }
```

#### Function `try_alpha_v_ph_eqm`

Checked isobaric cubic expansion coefficient alpha_v (1/K) from a
`(p,h)` flash (two-phase states return a quality-interpolated
estimate). Valid range: the `(p,h)` envelope in the module doc. Agrees
exactly with [`alpha_v_ph_eqm`] for in-range input.

Panic-trace: `alpha_v_ph_eqm` calls `t_ph_eqm` and `ph_flash_region`,
whose panics (`ph_flash_eqm/mod.rs:833,837,846` and the Region-5 arm at
`:51`) are all excluded by [`check_ph_envelope`]. Its Region-4 arm
evaluates `alpha_v_tp_1`/`alpha_v_tp_2` directly at `(T_sat, p)`,
bypassing the `(T,p)` router, so it adds no new panic site.

```rust
pub fn try_alpha_v_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<TemperatureCoefficient> { /* ... */ }
```

#### Function `try_kappa_t_ph_eqm`

Checked isothermal compressibility kappa_T (1/Pa) from a `(p,h)` flash
(two-phase states return a quality-interpolated estimate). Valid range:
the `(p,h)` envelope in the module doc. Agrees exactly with
[`kappa_t_ph_eqm`] for in-range input.

Panic-trace: identical to [`try_alpha_v_ph_eqm`].

```rust
pub fn try_kappa_t_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<crate::region_1_subcooled_liquid::InversePressure> { /* ... */ }
```

### Re-exports

#### Re-export `ps::*`

```rust
pub use ps::*;
```

#### Re-export `two_phase::*`

```rust
pub use two_phase::*;
```

#### Re-export `control_volume::*`

```rust
pub use control_volume::*;
```

## Module `object_oriented_programming`

for OOP users who want to make a struct (class)
and then use that for extracting data,
this is where the stuff is stored

this is basically a simple control volume

```rust
pub mod object_oriented_programming { /* ... */ }
```

### Modules

## Module `getter_methods`

vibe coded getter methods

```rust
pub mod getter_methods { /* ... */ }
```

## Module `setter_methods`

setter methods
this will deal with setting new thermodynamic equilibrium
based on user input parameters

```rust
pub mod setter_methods { /* ... */ }
```

## Module `mass_and_energy_balance`

transient mass-and-energy balance: accumulate `(mass, enthalpy)` source/sink
terms in a [`CvMassEnthalpyChanges`] ledger and apply them with
`TampinesSteamTableCV::advance_timestep`
Transient mass-and-energy balance for a [`TampinesSteamTableCV`] control
volume.

Design (human-authored — see the README changelog entry "Transient mass &
energy balance"):

A control volume has a **fixed geometric volume**. Over a timestep, streams
add or remove mass, each stream carrying its own specific enthalpy. The
caller accumulates these `(mass, specific-enthalpy)` source/sink terms in a
[`CvMassEnthalpyChanges`] ledger (built *outside* the control volume so the
`Copy` control-volume stays `Copy`), then calls
[`TampinesSteamTableCV::advance_timestep`] to apply them.

Applying a timestep is a conservation step followed by a flash:

1. **Mass:** `m_new = m_old + Σ dm_i`, where `m_old = V / v` (volume over
   specific volume). Removal terms carry a negative `dm_i`.
2. **Energy:** `H_new = H_old + Σ dm_i · h_i`, where `H_old = m_old · h_old`.
   The new specific enthalpy is `h_new = H_new / m_new`.
3. **New state point:** the volume is fixed, so the new specific volume is
   `v_new = V / m_new` (equivalently the new density `ρ_new = m_new / V`).
   The system now sits at a new `(ρ, h)` thermodynamic point. IF97 forward
   flashes are `(p, h)`-based, so we recover the pressure by solving
   `v(p, h_new) = v_new` for `p` with **regula falsi** (false position),
   then rebuild the control volume from `(p, h_new)`.

```rust
pub mod mass_and_energy_balance { /* ... */ }
```

### Types

#### Struct `CvMassEnthalpyChanges`

Ordered ledger of mass source/sink terms applied to a control volume over a
single timestep.

Each entry is a `(mass, specific-enthalpy)` pair: the amount of mass crossing
the boundary and the specific enthalpy that mass carries. Mass added to the
system is stored with a positive mass; mass removed is stored with a negative
mass (use [`add_mass`](Self::add_mass) / [`remove_mass`](Self::remove_mass) so
the sign convention is handled for you).

This is deliberately a separate, owned (non-`Copy`) type: the control volume
itself stays a small `Copy` value, and the variable-length list of pending
changes lives here. Build one per timestep (or reuse it and
[`clear`](Self::clear) between timesteps), then hand it to
[`TampinesSteamTableCV::advance_timestep`].

```rust
pub struct CvMassEnthalpyChanges {
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
  Creates an empty ledger.

- ```rust
  pub fn add_mass(self: &mut Self, mass: Mass, specific_enthalpy: AvailableEnergy) { /* ... */ }
  ```
  Records mass **added** to the control volume.

- ```rust
  pub fn remove_mass(self: &mut Self, mass: Mass, specific_enthalpy: AvailableEnergy) { /* ... */ }
  ```
  Records mass **removed** from the control volume.

- ```rust
  pub fn clear(self: &mut Self) { /* ... */ }
  ```
  Removes all recorded changes, leaving an empty ledger ready for the next

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` if no changes have been recorded.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of recorded source/sink terms.

- ```rust
  pub fn total_mass_change(self: &Self) -> Mass { /* ... */ }
  ```
  Net mass change `Σ dm_i` over all recorded terms (positive = net gain).

- ```rust
  pub fn total_enthalpy_change(self: &Self) -> Energy { /* ... */ }
  ```
  Net enthalpy change `Σ dm_i · h_i` over all recorded terms — the total

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

#### Struct `TampinesSteamTableCV`

this is the bread and butter for tampines steam tables,
the control volume

```rust
pub struct TampinesSteamTableCV {
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
  pub fn get_pressure(self: &Self) -> Pressure { /* ... */ }
  ```
  Returns the pressure of the control volume.

- ```rust
  pub fn get_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  Returns the thermodynamic temperature of the control volume.

- ```rust
  pub fn get_specific_volume(self: &Self) -> SpecificVolume { /* ... */ }
  ```
  Returns the specific volume of the fluid in the control volume.

- ```rust
  pub fn get_specific_enthalpy(self: &Self) -> AvailableEnergy { /* ... */ }
  ```
  Returns the specific enthalpy of the fluid in the control volume.

- ```rust
  pub fn get_specific_entropy(self: &Self) -> SpecificHeatCapacity { /* ... */ }
  ```
  Returns the specific entropy of the fluid in the control volume.

- ```rust
  pub fn get_volume(self: &Self) -> Volume { /* ... */ }
  ```
  Returns the total volume of the control volume.

- ```rust
  pub fn get_mass(self: &Self) -> Mass { /* ... */ }
  ```
  returns the mass within the control volume

- ```rust
  pub fn get_viscosity(self: &Self) -> DynamicViscosity { /* ... */ }
  ```
  returns viscosity (important for Reynold's number)

- ```rust
  pub fn get_speed_of_sound(self: &Self) -> Velocity { /* ... */ }
  ```
  Returns the **frozen** speed of sound of the control volume

- ```rust
  pub fn get_equilibrium_speed_of_sound(self: &Self) -> Velocity { /* ... */ }
  ```
  Returns the **homogeneous-equilibrium (HEM)** speed of sound of the

- ```rust
  pub fn get_mach_number(self: &Self, v: Velocity) -> Ratio { /* ... */ }
  ```
  Mach number `v / c` of a flow speed `v`, using the **frozen** speed of

- ```rust
  pub fn get_specific_heat_ratio(self: &Self) -> Ratio { /* ... */ }
  ```
  returns the specific heat ratio cp/cv of steam

- ```rust
  pub fn get_cp(self: &Self) -> SpecificHeatCapacity { /* ... */ }
  ```
  returns cp

- ```rust
  pub fn get_cv(self: &Self) -> SpecificHeatCapacity { /* ... */ }
  ```
  returns cv

- ```rust
  pub fn get_thermal_conductivity(self: &Self) -> ThermalConductivity { /* ... */ }
  ```
  returns thermal thermal_conductivity of steam

- ```rust
  pub fn get_critical_pressure_ratio_ideal_gas(self: &Self) -> Ratio { /* ... */ }
  ```
  returns critical pressure ratio for choked flow

- ```rust
  pub fn get_critical_pressure_ratio_pure_vapour(self: &Self) -> Ratio { /* ... */ }
  ```
  Returns critical pressure ratio for choked flow using isentropic relations

- ```rust
  pub fn get_crit_pressure_and_massflux(self: &Self) -> (Pressure, MassFlux) { /* ... */ }
  ```
  Critical pressure and mass flux for choked flow, assuming `self` holds

- ```rust
  pub fn get_critical_pressure_vle(self: &Self) -> Pressure { /* ... */ }
  ```
  finds pressure where mach number = 1 during isentropic expansion

- ```rust
  pub fn get_critical_pressure_pure_vapour(self: &Self) -> Pressure { /* ... */ }
  ```
  Finds the pressure where Mach number = 1 during isentropic expansion

- ```rust
  pub fn get_rho(self: &Self) -> MassDensity { /* ... */ }
  ```
  Returns the mass density (kg/m^3) of the fluid in the control

- ```rust
  pub fn get_region(self: &Self) -> FwdEqnRegion { /* ... */ }
  ```
  Returns the IAPWS-IF97 forward-equation region (1-5) the control

- ```rust
  pub fn get_metastable_steam_specific_volume(self: &Self) -> Option<SpecificVolume> { /* ... */ }
  ```
  get metastable steam state, (region 2 only)

- ```rust
  pub fn get_metastable_steam_specific_enthalpy(self: &Self) -> Option<AvailableEnergy> { /* ... */ }
  ```
  get metastable steam state, (region 2 only)

- ```rust
  pub fn get_metastable_steam_internal_energy(self: &Self) -> Option<AvailableEnergy> { /* ... */ }
  ```
  get metastable steam state, (region 2 only)

- ```rust
  pub fn get_metastable_steam_specific_entropy(self: &Self) -> Option<SpecificHeatCapacity> { /* ... */ }
  ```
  get metastable steam state, (region 2 only)

- ```rust
  pub fn get_metastable_steam_cp(self: &Self) -> Option<SpecificHeatCapacity> { /* ... */ }
  ```
  get metastable steam state, (region 2 only)

- ```rust
  pub fn get_metastable_steam_cv(self: &Self) -> Option<SpecificHeatCapacity> { /* ... */ }
  ```
  get metastable steam state, (region 2 only)

- ```rust
  pub fn get_metastable_steam_speed_of_sound(self: &Self) -> Option<Velocity> { /* ... */ }
  ```
  get metastable steam state, (region 2 only)

- ```rust
  pub fn get_quality(self: &Self) -> f64 { /* ... */ }
  ```
  get the steam quality, only if the region is in region 4

- ```rust
  pub fn try_new_tsat_based_on_pressure(self: &Self) -> Option<ThermodynamicTemperature> { /* ... */ }
  ```
  get the saturation temperature based on pressure

- ```rust
  pub fn try_new_psat_based_on_temperature(self: &Self) -> Option<Pressure> { /* ... */ }
  ```
  get the saturation pressure based on temperature

- ```rust
  pub fn try_get_tsat(p: Pressure) -> Option<ThermodynamicTemperature> { /* ... */ }
  ```
  get the saturation temperature based on pressure

- ```rust
  pub fn try_get_psat(t: ThermodynamicTemperature) -> Option<Pressure> { /* ... */ }
  ```
  get the saturation pressure based on temperature

- ```rust
  pub fn get_ref_vol() -> Volume { /* ... */ }
  ```
  just a convenience function to get ref volume

- ```rust
  pub fn get_stagnation_critical_mass_flux(self: &Self) -> MassFlux { /* ... */ }
  ```
  Critical mass flux for choked flow, assuming `self` holds stagnation properties.

- ```rust
  pub fn set_tpx(self: &mut Self, t: ThermodynamicTemperature, p: Pressure, x: f64) { /* ... */ }
  ```
  Re-flashes the control volume in place from `(T,p,x)`, where

- ```rust
  pub fn set_ph(self: &mut Self, p: Pressure, h: AvailableEnergy) { /* ... */ }
  ```
  Re-flashes the control volume in place from `(p,h)`, where pressure

- ```rust
  pub fn set_ps(self: &mut Self, p: Pressure, s: SpecificHeatCapacity) { /* ... */ }
  ```
  Re-flashes the control volume in place from `(p,s)`, where pressure

- ```rust
  pub fn compress_isentropically(self: &mut Self, new_p: Pressure) { /* ... */ }
  ```
  Models an ideal (isentropic) compression process to a new pressure.

- ```rust
  pub fn expand_isentropically(self: &mut Self, new_p: Pressure) { /* ... */ }
  ```
  Models an ideal (isentropic) expansion process to a new pressure.

- ```rust
  pub fn add_heat_isobaric(self: &mut Self, heat_added_per_kg: AvailableEnergy) { /* ... */ }
  ```
  Models a process of adding heat at constant pressure (isobaric).

- ```rust
  pub fn remove_heat_isobaric(self: &mut Self, heat_removed_per_kg: AvailableEnergy) { /* ... */ }
  ```
  Models a process of removing heat at constant pressure (isobaric).

- ```rust
  pub fn add_heat_isobaric_extensive(self: &mut Self, total_heat_added: Energy) { /* ... */ }
  ```
  Models adding a total amount of heat (extensive) to the control volume

- ```rust
  pub fn remove_heat_isobaric_extensive(self: &mut Self, total_heat_removed: Energy) { /* ... */ }
  ```
  Models removing a total amount of heat (extensive) from the control volume

- ```rust
  pub fn compress_with_work_extensive(self: &mut Self, total_work_input: Energy, outlet_pressure: Pressure) { /* ... */ }
  ```
  Models the work done on the fluid during compression by specifying the

- ```rust
  pub fn expand_with_work_extensive(self: &mut Self, total_work_output: Energy, outlet_pressure: Pressure) { /* ... */ }
  ```
  Models the work done by the fluid during expansion by specifying the

- ```rust
  pub fn advance_timestep(self: &mut Self, changes: &CvMassEnthalpyChanges) { /* ... */ }
  ```
  Applies one timestep of mass-and-energy exchange recorded in `changes`,

- ```rust
  pub fn new_from_tp_quality(temperature: ThermodynamicTemperature, pressure: Pressure, volume: Volume, x: f64) -> Self { /* ... */ }
  ```
  Creates a new control volume from a `(T,p,x)` forward flash, where

- ```rust
  pub fn new_from_tp_quality_1(temperature: ThermodynamicTemperature, pressure: Pressure, volume: Volume) -> Self { /* ... */ }
  ```
  creates a new control volume assuming quality is 1

- ```rust
  pub fn new_from_tp_quality_0(temperature: ThermodynamicTemperature, pressure: Pressure, volume: Volume) -> Self { /* ... */ }
  ```
  creates a new control volume assuming quality is 0

- ```rust
  pub fn new_from_ph(p: Pressure, h: AvailableEnergy, volume: Volume) -> Self { /* ... */ }
  ```
  Creates a new control volume from a `(p,h)` flash, where pressure

- ```rust
  pub fn new_from_ps(p: Pressure, s: SpecificHeatCapacity, volume: Volume) -> Self { /* ... */ }
  ```
  Creates a new control volume from a `(p,s)` flash, where pressure

- ```rust
  pub fn new_from_hs(h: AvailableEnergy, s: SpecificHeatCapacity, volume: Volume) -> Self { /* ... */ }
  ```
  Creates a new control volume from an `(h,s)` flash, where specific

- ```rust
  pub fn new_from_sat_pressure_quality(p: Pressure, x: f64, volume: Volume) -> Self { /* ... */ }
  ```
  Creates a new control volume on the saturation line (Region 4) given

- ```rust
  pub fn new_from_sat_temp_quality(t: ThermodynamicTemperature, x: f64, volume: Volume) -> Self { /* ... */ }
  ```
  Creates a new control volume on the saturation line (Region 4) given

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

#### Re-export `CvMassEnthalpyChanges`

```rust
pub use mass_and_energy_balance::CvMassEnthalpyChanges;
```

## Module `surface_tension`

surface tension
important for boiling
IAPWS surface tension sigma of water against its own vapour along the
saturation line, in newtons per metre (N/m, equivalently J/m^2), as a
function of temperature (`ThermodynamicTemperature`, K). Returned as a
`uom` `RadiantExposure` (J/m^2) because `uom` has no dedicated
surface-tension quantity — the units coincide (kg/s^2).

```rust
pub mod surface_tension { /* ... */ }
```

### Functions

#### Function `water_surf_tension`

function for surface tension
units are newtons per meter

newtons = kg * m s^(-2)

so newtons per meter is
newtons/m = kg * s^(-2)

this is the same unit as RadiantExposure
Joule per m^2

However, the UOM crate doesn't have surface tension per se
So I'll use RadiantExposure as the return type

```rust
pub fn water_surf_tension(t: ThermodynamicTemperature) -> RadiantExposure { /* ... */ }
```

## Module `dielectric_constant`

dielectric constant
IAPWS correlation for the static (relative) dielectric constant of water,
epsilon — dimensionless — via the Harris-Alder g-bar factor, as a
function of density (`MassDensity`, kg/m^3) and temperature
(`ThermodynamicTemperature`, K).

```rust
pub mod dielectric_constant { /* ... */ }
```

### Functions

#### Function `water_dielectric_const_rho_t`

Static (relative) dielectric constant epsilon of water — dimensionless —
at density `rho` (kg/m^3) and temperature `t` (K), per the IAPWS
correlation.

```rust
pub fn water_dielectric_const_rho_t(rho: MassDensity, t: ThermodynamicTemperature) -> f64 { /* ... */ }
```

## Module `steam_turbine_equations`

useful equations for steam turbines
These include nozzles, impulse turbines
and reaction turbines at some steady
state,
as well as angular momentum balance
Steam-turbine equations: converging-diverging nozzle / choked-flow
relations ([`converging_diverging_nozzles`]) and three-phase electric
generator equations ([`generator`]).

```rust
pub mod steam_turbine_equations { /* ... */ }
```

### Modules

## Module `converging_diverging_nozzles`

equations for isentropic nozzles, including choked flow
at sonic speeds
Converging-diverging (Laval) nozzle equations for the steam-turbine
stator/impulse-turbine flow path: isentropic converging-section and
choked-throat relations ([`isentropic_converging_nozzle`]), diverging-
section perfectly-expanded and shock-containing relations
([`diverging_nozzle`]), Homogeneous-Equilibrium-Model choked (critical)
two-phase flow ([`choked_flow`]), Joule-Thomson throttling
([`joule_thomson`]), and (private, incomplete) differential and
Rayleigh-line helpers.
[`calculate_velocity_mass_flowrate_and_state_in_cd_nozzle`] is the main
entry point tying the converging and diverging sections together.

```rust
pub mod converging_diverging_nozzles { /* ... */ }
```

### Modules

## Module `isentropic_converging_nozzle`

these are converging nozzles,
for this, usually we assume isentropy.
This part is relatively straightforward

```rust
pub mod isentropic_converging_nozzle { /* ... */ }
```

### Functions

#### Function `guess_massrate_and_state_for_converge_nozzle_from_stagnation`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Given inlet stagnation properties `(p0, h0)` and a guessed throat
velocity, computes the resulting mass flow rate (kg/s) and thermodynamic
state at the throat of a converging nozzle, assuming isentropic flow.
The throat velocity is capped at the local speed of sound (choked flow)
if the supplied `v_throat` would otherwise exceed it; in that case the
throat state is the critical (Mach-1) state rather than the state implied
by `v_throat` directly.

```rust
pub fn guess_massrate_and_state_for_converge_nozzle_from_stagnation(p0: Pressure, h0: AvailableEnergy, v_throat: Velocity, a_throat: Area) -> (MassRate, crate::prelude::TampinesSteamTableCV) { /* ... */ }
```

#### Function `get_choked_flow_massrate_and_state_from_stagnation_properties_and_area`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

This gives the mass flowrate for choked flow given the area of a throat
and the inlet stagnation properties (negligble KE)

```rust
pub fn get_choked_flow_massrate_and_state_from_stagnation_properties_and_area(p0: Pressure, h0: AvailableEnergy, a_throat: Area) -> (MassRate, crate::prelude::TampinesSteamTableCV) { /* ... */ }
```

## Module `diverging_nozzle`

this is the diverging part of the C-D nozzle
where (p,h) flashing is used instead of (p,s) or (h,s) flashing
shocks are not explictly calculated, but entropy increase is assumed

```rust
pub mod diverging_nozzle { /* ... */ }
```

### Functions

#### Function `guess_velocity_and_state_for_diverge_nozzle_from_choked_throat`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

given a sonic flow,

note, shocks may occur here

given a pressure at the outlet, p2,
and throat state, guess the state of flow going out
mass flowrate is based on choked flow

stagnation properties should also be supplied to facilitate calculation

note that this is no longer isentropic

```rust
pub fn guess_velocity_and_state_for_diverge_nozzle_from_choked_throat(h0: AvailableEnergy, s0: SpecificHeatCapacity, p2: Pressure, a_exit: Area, mass_rate_throat: MassRate, state_throat: crate::prelude::TampinesSteamTableCV) -> (Velocity, crate::prelude::TampinesSteamTableCV) { /* ... */ }
```

#### Function `calculate_isentropic_exit_pressure_velocity_and_state_supersonic`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Calculate exit pressure for isentropic expansion through CD nozzle
assuming choked flow

this is for perfectly expanded flow


Now, there are two pressures that would work,
the lower bound pressure and upper bound pressure
the lower bound pressure is supersonic
and the upper bound pressure is subsonic

This bisection method is based on a pressure algorithm,
that is to change pressure until the right mass flowrate is achieved
Calculates the exit pressure, velocity, and state for a perfectly expanded,
isentropic flow in a converging-diverging nozzle, targeting the SUPERSONIC solution.

This function assumes the flow is choked at the throat. It finds the exit conditions
in the diverging section that satisfy the choked mass flow rate for a given exit area.

# Algorithm
The function uses a two-stage process:
1.  **Bounding Scan (Velocity-based):** It first performs a rough scan across a range of
    velocities to find a narrow pressure bracket `[p_lower, p_upper]` that contains the
    supersonic root. This is the most critical step for isolating the correct solution.
2.  **Refinement (Pressure-based Bisection):** It then uses a bisection method on pressure
    to refine the solution within that narrow bracket to the required precision.

# Arguments
* `inlet_stagnation_state`: The thermodynamic state at stagnation conditions (h0, s0).
* `a_exit`: The area of the nozzle exit.
* `mass_flowrate_choked`: The mass flow rate determined by the choked throat conditions.


```rust
pub fn calculate_isentropic_exit_pressure_velocity_and_state_supersonic(inlet_stagnation_state: crate::prelude::TampinesSteamTableCV, a_exit: Area, mass_flowrate_choked: MassRate) -> (Pressure, Velocity, crate::prelude::TampinesSteamTableCV) { /* ... */ }
```

#### Function `calculate_isentropic_exit_pressure_velocity_and_state_subsonic`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Calculates the exit pressure, velocity, and state for a perfectly
expanded, isentropic flow in a converging-diverging nozzle, targeting the
SUBSONIC solution (the diverging section acting as a diffuser downstream
of a choked throat).

Bisects on exit pressure between the critical (throat) pressure and the
stagnation pressure `p0` until the mass flow rate implied by the
isentropic `(p, s0)` exit state matches `mass_flowrate_choked`.

# Arguments
* `inlet_stagnation_state`: The thermodynamic state at stagnation
  conditions (h0, s0).
* `a_exit`: The area of the nozzle exit.
* `mass_flowrate_choked`: The mass flow rate determined by the choked
  throat conditions.

```rust
pub fn calculate_isentropic_exit_pressure_velocity_and_state_subsonic(inlet_stagnation_state: crate::prelude::TampinesSteamTableCV, a_exit: Area, mass_flowrate_choked: MassRate) -> (Pressure, Velocity, crate::prelude::TampinesSteamTableCV) { /* ... */ }
```

## Module `choked_flow`

for sonic flow, we need to get conditions where choked flow is
achieved

these are for textbook questions, where basic verification is performed
to see if choked flow calculation is correct

```rust
pub mod choked_flow { /* ... */ }
```

### Modules

## Module `single_phase_basic_choked_flow`

these contain choked flow algorithms for single phase choked flow,

whether be it finding critical pressure for ideal gas, or for those
where the choked flow is in the pure vapour phase

```rust
pub mod single_phase_basic_choked_flow { /* ... */ }
```

### Functions

#### Function `get_choked_flow_state_for_nozzle_subsonic_to_sonic`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

This is an algorithm to obtain outlet thermodynamic state
for a converging nozzle with subsonic flow
Given inlet conditions (p1, h1, v1) and geometry (a1, a2), calculates
the throat conditions assuming choked flow (M = 1 at exit).

# Arguments
* `p1` - Inlet pressure
* `h1` - Inlet specific enthalpy
* `a1` - Inlet area
* `a2` - Throat (exit) area
* `v1` - Inlet velocity

# Returns
Tuple of (p2, h2, mass_flowrate) where:
* `p2` - Throat pressure
* `h2` - Throat specific enthalpy
* `mass_flowrate` - Choked mass flow rate (determined by throat conditions)

# Warnings
- Warns if mass balance error > 5% (inlet vs throat)
- Warns if momentum balance error > 5%

```rust
pub fn get_choked_flow_state_for_nozzle_subsonic_to_sonic(p1: Pressure, h1: AvailableEnergy, a1: Area, a2: Area, v1: Velocity) -> (Pressure, AvailableEnergy, MassRate) { /* ... */ }
```

#### Function `get_choked_flow_nozzle_area`

based on steam pressure and enthalpy,
as well as mass flowrate, obtain the choked flow area
you have to give the stagnation enthalpy and pressure as
inputs

```rust
pub fn get_choked_flow_nozzle_area(p0: Pressure, h0: AvailableEnergy, mass_flowrate: MassRate) -> Area { /* ... */ }
```

#### Function `get_choked_flow_supersonic_nozzle_exit_area_and_state`

based on steam pressure and enthalpy,
as well as mass flowrate, obtain the choked flow area
you have to give the stagnation enthalpy and pressure as
inputs

isentropic nozzle assumed

also, mass flowrate and exit pressure given,
assuming throat velocity is speed of sound

```rust
pub fn get_choked_flow_supersonic_nozzle_exit_area_and_state(p_throat: Pressure, h_throat: AvailableEnergy, p_exit: Pressure, mass_flowrate: MassRate) -> (Pressure, AvailableEnergy, Area) { /* ... */ }
```

#### Function `get_critical_pressure_ratio_ideal_gas_using_throat_ph`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

estimates critical pressure ratio given ideal gas assumptions
for ideal gases, critical ratio depends on k
but k is generally temperature dependent

The evaluation here is to use throat properties to get the critical
pressure ratio


```rust
pub fn get_critical_pressure_ratio_ideal_gas_using_throat_ph(p: Pressure, h: AvailableEnergy) -> Ratio { /* ... */ }
```

#### Function `get_critical_pressure_pure_vapour_ph_stagnation_properties`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Finds the pressure where Mach number = 1 during isentropic expansion
This only works for superheated vapour

this takes in ph and optionally a stagnation entropy (if one wants to save
on calculation speed)

```rust
pub fn get_critical_pressure_pure_vapour_ph_stagnation_properties(p0: Pressure, h0: AvailableEnergy, s0_opt: Option<SpecificHeatCapacity>) -> Pressure { /* ... */ }
```

## Module `basic_multiphase_equations`

these contain functions for generic multiphase equations
eg. obtaining stagnation properties from throat properties

```rust
pub mod basic_multiphase_equations { /* ... */ }
```

### Functions

#### Function `get_stagnation_conditions_from_throat_ps`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Given throat conditions (p_t, s_t), compute the critical mass flux
and back-calculate the stagnation conditions (p_0, h_0)

This is the inverse of the usual approach — instead of finding
the throat from stagnation conditions, we fix the throat and
recover the stagnation state.

From energy conservation (isentropic):
h_0 = h_t + G*² / (2 * rho_t²)
     = h_t + v_t² * G*² / 2

Entropy is conserved: s_0 = s_t
Stagnation pressure recovered via p_hs_eqm(h_0, s_0)

Reference: Saha (1978) NUREG/CR-0417, eq. 10
           Moody (1975) NEDO-21052

Note that this uses the homogeneous equilibrium model.
This was validated using Zaloudek's data

```rust
pub fn get_stagnation_conditions_from_throat_ps(p_t: Pressure, s_t: SpecificHeatCapacity) -> (Pressure, AvailableEnergy, MassFlux) { /* ... */ }
```

#### Function `get_stagnation_conditions_from_throat_ph`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Same as above but takes throat (p_t, h_t) as input
converts h_t to s_t internally
This was validated using Zaloudek's data

```rust
pub fn get_stagnation_conditions_from_throat_ph(p_t: Pressure, h_t: AvailableEnergy) -> (Pressure, AvailableEnergy, MassFlux) { /* ... */ }
```

#### Function `bubble_point_pressure_from_entropy`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Bubble-point pressure along an isentrope `s = s0`.

Returns the pressure `p_bubble` at which the saturated-liquid entropy
equals `s0` — i.e. the pressure where an isentropic depressurisation of a
subcooled / liquid-like state first reaches saturation (x = 0, flashing
inception).

The saturated-liquid entropy
  `s_f(p) = s_tp_eqm_two_phase(T_sat(p), p, 0.0)`
is monotonically increasing in `p` (from ~0 at the triple point up to
`s_crit` at the critical point), so the root `s_f(p_bubble) = s0` is unique
and recovered by bisection. This automatically handles the Region-3 cap
(16.529-22.064 MPa), where the saturated-liquid properties come from the
Region 3 EOS.

Precondition: `s0` lies on the liquid side of the dome, i.e.
`s_f(p_triple) <= s0 <= s_crit`. At (or above) the critical entropy the
bubble line meets the critical point, so `p_crit` is returned directly.
Below the triple-point saturated-liquid entropy it clamps to `p_min`.

```rust
pub fn bubble_point_pressure_from_entropy(s0: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

#### Function `bubble_point_pressure_ph`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Same as [`bubble_point_pressure_from_entropy`] but takes a `(p, h)`
stagnation state and uses its entropy. Convenient for reading subcooled /
liquid-like points straight off a p-h diagram.

```rust
pub fn bubble_point_pressure_ph(p0: Pressure, h0: AvailableEnergy) -> Pressure { /* ... */ }
```

#### Function `dew_point_pressure_from_entropy`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Dew-point pressure along an isentrope `s = s0`.

Returns the pressure `p_dew` at which the saturated-vapour entropy equals
`s0` — i.e. the pressure where an isentropic depressurisation of a
superheated-vapour / supercritical state first reaches saturation (x = 1,
condensation inception). This is the vapour-side analogue of
[`bubble_point_pressure_from_entropy`].

The saturated-vapour entropy
  `s_g(p) = s_tp_eqm_two_phase(T_sat(p), p, 1.0)`
is monotonically *decreasing* in `p` (from large values near the triple
point down to `s_crit` at the critical point), so the root
`s_g(p_dew) = s0` is unique and recovered by bisection. This handles the
Region-3 cap automatically.

Precondition: `s0` lies on the vapour side of the dome, i.e.
`s_crit <= s0 <= s_g(p_triple)`. At (or below) the critical entropy the dew
line meets the critical point, so `p_crit` is returned directly. Above the
triple-point saturated-vapour entropy it clamps to `p_min`.

```rust
pub fn dew_point_pressure_from_entropy(s0: SpecificHeatCapacity) -> Pressure { /* ... */ }
```

#### Function `dew_point_pressure_ph`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Same as [`dew_point_pressure_from_entropy`] but takes a `(p, h)` stagnation
state and uses its entropy. Convenient for reading superheated-vapour /
supercritical points straight off a p-h diagram.

```rust
pub fn dew_point_pressure_ph(p0: Pressure, h0: AvailableEnergy) -> Pressure { /* ... */ }
```

## Module `stagnation_point_within_vle_ph_dome_multiphase`

critical-flow solvers for when the stagnation state lies inside the
p-h VLE dome (two-phase, at or below the critical point)

```rust
pub mod stagnation_point_within_vle_ph_dome_multiphase { /* ... */ }
```

### Functions

#### Function `get_critical_pressure_and_mass_flux_ph_vle_dome`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Critical pressure & mass flux for a stagnation state that sits
INSIDE the p-h VLE dome (two-phase, at or below the critical point).

Precondition: (p0, h0) is two-phase — i.e. ph_flash_region(p0,h0) == Region4.
Once inside the dome, isentropic depressurisation stays inside it
(the dome only widens as p falls), so there is no flashing event and
no region switching to handle here.

Method (Moody / max-flux form of the HEM choking criterion):
  along the isentrope s = s0,
    G(p) = rho(p,s0) * sqrt( 2 * (h0 - h(p,s0)) )
  G(p0) = 0, rises to a single interior maximum at the choke point,
  then falls as rho -> 0. The choke is argmax_p G(p).

This avoids mass_flux_ps_eqm_throat (finite-difference sound speed +
bubble-point clamp) entirely; it only needs smooth h(p,s0), v(p,s0).

Consistent with the validated inverse map: max-G <=> Mach 1 <=>
h0 = h_t + 0.5 * u_t^2  (get_stagnation_conditions_from_throat_ps).

# Validation status

Validated against Zaloudek (1961) HEM critical mass flux curves for
two-phase stagnation states (throat quality x_t = 0.0–1.00, all 21
quality curves). All in-dome points pass within tolerance (worst error
~0.86% pressure at 100 psia for x_t = 0.05, near the bubble-point edge
of the dome).

```rust
pub fn get_critical_pressure_and_mass_flux_ph_vle_dome(p0: Pressure, h0: AvailableEnergy) -> (Pressure, MassFlux) { /* ... */ }
```

## Module `stagnation_point_outside_vle_ph_dome_multiphase`

critical-flow solvers for when the stagnation state lies outside the
p-h VLE dome (single phase: subcooled liquid / liquid-like, and later
superheated vapour / supercritical)
Critical-flow solvers for stagnation states that lie OUTSIDE the p-h VLE
dome (single phase at the inlet).

Two single-phase buckets sit either side of the dome and are handled by
mirror-image solvers here:

* **Subcooled-liquid / liquid-like** (left of the dome, `s0 < s_crit`). On
  isentropic depressurisation such a state stays single-phase liquid down to
  the **bubble point**, then flashes into the dome. See
  [`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`].
* **Superheated-vapour / supercritical** (right of / above the dome,
  `s0 > s_crit`). On isentropic depressurisation such a state stays
  single-phase vapour down to the **dew point**, then condensation begins as
  it enters the dome. See
  [`get_critical_pressure_and_mass_flux_superheated_vapour_ph`].

Both solvers use the same smooth energy-balance max-G HEM criterion as the
in-dome solver — `G(p) = rho(p,s0) * sqrt(2*(h0 - h(p,s0)))` maximised along
the isentrope — so no sound speed (and no finite-difference
`mass_flux_ps_eqm_throat`) is ever evaluated.

```rust
pub mod stagnation_point_outside_vle_ph_dome_multiphase { /* ... */ }
```

### Functions

#### Function `get_critical_pressure_and_mass_flux_subcooled_liquid_ph`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Critical pressure & mass flux for a subcooled-liquid / liquid-like
stagnation state (OUTSIDE the dome, left side).

Precondition: `(p0, h0)` is single-phase on the liquid side — subcooled
liquid (`p0 < p_c`, `T0 < T_sat(p0)`) or liquid-like (`p0 >= p_c`,
`T0 < T_c`). The caller's dispatcher is responsible for routing here.

Method: two-regime choke finder along the isentrope `s = s0`,
    `G_energy(p) = rho(p,s0) * sqrt( 2 * (h0 - h(p,s0)) )`

* **Genuinely subcooled** (throat quality > ~0.03): the choke is a smooth
  interior two-phase point where the energy-balance maximum of `G_energy`
  coincides with the sonic point (`dG/dp = 0 ⇔ v = c`). Located by
  golden-section over `[p_min, p_bubble]`.
* **Near-saturated** (throat quality ≲ 0.03, i.e. throat on the saturated-
  liquid line): the energy maximum is *not* the choke — it either overshoots
  `rho_f·v ≫ rho_f·c` at the bubble point or walks off to a deeper stationary
  point the flow never reaches at `M = 1`. Here the choke is the bubble-point
  kink itself; the mass flux `rho_f·c_2φ` is read from a precomputed sonic map
  along the saturated-liquid line (see [`saturation_line_sonic_mass_flux`]).

The regime is selected by the two-phase quality at the energy-max choke,
which is the only quantity that cleanly separates the two cases (stagnation
subcooling and pressure both overlap between them).

# Validation status

Validated against Zaloudek (1961) HEM critical mass flux curves for all
throat qualities x_t = 0.0–1.00 (the saturated-liquid-line curve x_t ≈ 0
included). All curves pass within tolerance.

# Note — the x ≈ 0 curve is numerical, not a physics limit

The near-saturation correction exists because the energy-balance objective
is blind to the discontinuity in the HEM sound speed at the bubble point, and
the pointwise sonic function is unreliable in the thin band just below it —
*not* because HEM cannot reproduce the x ≈ 0 line. The Zaloudek reference is
itself HEM, and `mass_flux_ps_eqm_throat` evaluated at the throat reproduces
it to ±0.04 in log10 G at every point.

```rust
pub fn get_critical_pressure_and_mass_flux_subcooled_liquid_ph(p0: Pressure, h0: AvailableEnergy) -> (Pressure, MassFlux) { /* ... */ }
```

#### Function `get_critical_pressure_and_mass_flux_superheated_vapour_ph`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Critical pressure & mass flux for a superheated-vapour / supercritical
(vapour-like) stagnation state (OUTSIDE the dome, right side / above the
dome).

Precondition: `(p0, h0)` is single-phase on the vapour side — superheated
vapour (`p0 < p_c`, `T0 > T_sat(p0)`) or supercritical vapour-like
(`s0 > s_crit`). The caller's dispatcher is responsible for routing here.
The distinguishing test is `s0 > s_crit`: such an isentrope can only re-enter
the dome across the **dew** line (x = 1), never the bubble line.

Method (energy-balance max-G, HEM — the same `G(p)` curve as the in-dome and
subcooled solvers, no sound speed involved): along the isentrope `s = s0`,
  `G(p) = rho(p,s0) * sqrt( 2 * (h0 - h(p,s0)) )`.

This is the mirror image of
[`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`], with the **dew**
point playing the role the bubble point plays on the liquid side. There are
two candidate stretches and the choke is the global maximum of `G` over both:

* **Single-phase vapour stretch `[p_dew, p0]`.** Unlike the liquid side
  (where `G` is monotone up to the bubble point), `G` here has an *interior*
  peak — the ordinary perfect-gas-like **vapour sonic choke** — because the
  vapour expands and `rho` falls steeply. This stretch therefore needs its
  own golden-section maximisation, not a single endpoint evaluation.
* **Two-phase stretch `[p_min, p_dew]`.** Below the dew point the flow is a
  condensing mist; `G` develops a second (possibly higher) peak there.

  `G_crit = max( max_{[p_dew, p0]} G,  max_{[p_min, p_dew]} G )`
* vapour-stretch peak wins -> **vapour sonic choke** (strongly superheated)
* two-phase peak wins      -> **condensation choke** (near-saturated vapour)

For strongly superheated / supercritical inlets the dew point sits far below
the choke and the vapour-stretch peak dominates, recovering the classical
single-phase steam-nozzle result; near the saturated-vapour line the
two-phase peak can take over.

# Known limitation — near-saturated stagnation (x_t ≈ 1)

Mirroring the bubble-point limitation on the liquid side, for stagnation
states very close to the dew point the HEM equilibrium assumption breaks
down (droplet condensation lags the local pressure drop). Reproducing the
x ≈ 1 choking line faithfully requires a non-equilibrium / relaxation model
(HRM). Interior superheat is well represented.

```rust
pub fn get_critical_pressure_and_mass_flux_superheated_vapour_ph(p0: Pressure, h0: AvailableEnergy) -> (Pressure, MassFlux) { /* ... */ }
```

### Constants and Statics

#### Constant `DEEP_SUBCOOLING_RATIO`

Threshold on `v_b/c_2φ` (Bernoulli velocity at the bubble point over the
two-phase sound speed) above which a subcooled stagnation state is treated as
unambiguously deeply subcooled and the choke is taken as the energy-balance /
Bernoulli maximum. Set above the maximum reached by any Zaloudek subcooled
reference point (3.30) so this only adds deep-subcooling behaviour and never
perturbs the validated near-bubble (sonic) regime. See README v0.2.1.

Public so the Moody deep-subcooling verification test can classify points with
the exact same threshold the solver uses.

```rust
pub const DEEP_SUBCOOLING_RATIO: f64 = 5.0;
```

### Functions

#### Function `get_critical_pressure_and_mass_flux_with_stagnation_props`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Gets critical pressure and mass flux for water and steam given stagnation
properties using an older combined solver.

# Deprecation notice — use the split solvers instead

This function is the original combined dispatcher and is **superseded** by
the two specialised solvers that correctly route by stagnation region:

* [`get_critical_pressure_and_mass_flux_ph_vle_dome`] — stagnation inside
  the p-h VLE dome (two-phase, `ph_flash_region == Region4`).
* [`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`] — stagnation
  outside the dome on the liquid side (subcooled / liquid-like).

This function relied on `mass_flux_ps_eqm_throat` (finite-difference sound
speed with a bubble-point clamp), which produces a spurious root near the
saturated-liquid line and caused a +25% choke-pressure artifact at
x_t ≈ 0.05 / 100 psia. The split solvers avoid this by using the smooth
energy-balance `G(p) = rho * sqrt(2*(h0-h))` directly.

# Known limitations

* Region 5 (T > 800 °C) is not fully implemented.
* Near-saturated stagnation states (x ≈ 0) are not reliable — see the
  known limitation note on [`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`].
* The `debug` flag is hard-coded to `true` and emits `println!` / `dbg!`
  output unconditionally — this is a work-in-progress artefact.

```rust
pub fn get_critical_pressure_and_mass_flux_with_stagnation_props(s0: SpecificHeatCapacity, h0: AvailableEnergy, p0: Pressure) -> (Pressure, MassFlux) { /* ... */ }
```

#### Function `get_critical_pressure_and_mass_flux_multiphase_ph`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Critical pressure and mass flux for water/steam from stagnation conditions `(p0, h0)`.

Unified forward dispatcher that routes to the correct HEM solver based on the
stagnation region:

| Stagnation state | Solver |
|---|---|
| Region 4 — two-phase, inside VLE dome | [`get_critical_pressure_and_mass_flux_ph_vle_dome`] |
| Region 1 — subcooled liquid | [`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`] |
| Region 2 / 5 — superheated / ultra-high-T vapour | [`get_critical_pressure_and_mass_flux_superheated_vapour_ph`] |
| Region 3 — supercritical, isentrope crosses into the dome near the apex | `dome_crossing_interior_choke` |
| Region 3 — supercritical, no dome crossing (`s0 > s_crit`) | [`get_critical_pressure_and_mass_flux_superheated_vapour_ph`] |
| Region 3 — supercritical, no dome crossing (`s0 ≤ s_crit`) | [`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`] |

The near-critical Region 3 case is special: the energy-balance `G(p)` has a
spurious kink-peak at the phase boundary that masks the true interior
two-phase choke (see `dome_crossing_interior_choke`). When the isentrope
re-enters the dome, that interior choke is used; otherwise the state stays
single-phase and the entropy decides which mirror-image single-phase solver
applies.

Validated against Zaloudek (1961) HEM critical mass flux curves for
stagnation states across the full quality range x_t = 0.0–1.00.

# Parameters
- `p0` — stagnation pressure (any valid IAPWS-IF97 pressure)
- `h0` — stagnation specific enthalpy (J/kg via `uom`)

# Returns
`(p_crit, G_crit)`: critical (choke) pressure and HEM mass flux at the throat.

```rust
pub fn get_critical_pressure_and_mass_flux_multiphase_ph(p0: Pressure, h0: AvailableEnergy) -> (Pressure, MassFlux) { /* ... */ }
```

#### Function `isentropic_pressure_scan_of_mass_flux`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Diagnostic-only helper: scans the HEM mass flux `G` (kg/(m²·s)) along the
isentrope from the stagnation pressure `p0` (Pa) at fixed stagnation entropy
`s0` (J/(kg·K)) down to the steam-table lower pressure limit, printing the
peak via `dbg!`. Returns nothing — for interactive investigation of the
choke, not a production entry point. Use the
`get_critical_pressure_and_mass_flux_multiphase_ph` dispatcher instead.

```rust
pub fn isentropic_pressure_scan_of_mass_flux(s0: SpecificHeatCapacity, p0: Pressure) { /* ... */ }
```

#### Function `g_max_hem_analytical_ps`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Analytical HEM critical mass flux from Saha (1978) NUREG/CR-0417 eq. 10

G²_max = -1 / (dv_mix/dP|_s)

where dv_mix/dP|_s is expanded as:
x * (dv_g/dP)_s + (v_g - v_f) * (dx/dP)_s + (1-x) * (dv_f/dP)_s

This is the analytical version of mass_flux_ps_eqm_throat
which computes the same quantity via finite difference

Takes throat conditions (p, s) or (p, h) — NOT stagnation conditions

This uses region 1 and 2 eqns

```rust
pub fn g_max_hem_analytical_ps(p: Pressure, s: SpecificHeatCapacity) -> MassFlux { /* ... */ }
```

#### Function `g_max_hem_analytical_ph`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

same as g_max_hem_analytical_ps but takes (p, h) as input
converts h to s internally

```rust
pub fn g_max_hem_analytical_ph(p: Pressure, h: AvailableEnergy) -> MassFlux { /* ... */ }
```

### Re-exports

#### Re-export `basic_multiphase_equations::*`

```rust
pub use basic_multiphase_equations::*;
```

#### Re-export `stagnation_point_within_vle_ph_dome_multiphase::*`

```rust
pub use stagnation_point_within_vle_ph_dome_multiphase::*;
```

#### Re-export `stagnation_point_outside_vle_ph_dome_multiphase::*`

```rust
pub use stagnation_point_outside_vle_ph_dome_multiphase::*;
```

## Module `joule_thomson`

Joule-Thomson depressurisation

this is where flow goes through a pipe, and suddenly pressure drops
from p1 to p2, at constant enthalpy and mass flowrate

```rust
pub mod joule_thomson { /* ... */ }
```

### Functions

#### Function `get_outlet_velocity_and_state_joule_thomson`

we want to obtain the outlet thermodynamic state of a flow
going through a joule thomson model
stagnation enthalpy is constant, but enthalpy will differ

In this case however, the kinetic energy is NOT negligible

```rust
pub fn get_outlet_velocity_and_state_joule_thomson(p1: Pressure, h1: AvailableEnergy, p2: Pressure, mass_flowrate_ref: MassRate, a1: Area) -> (Velocity, crate::interfaces::object_oriented_programming::TampinesSteamTableCV) { /* ... */ }
```

### Functions

#### Function `calculate_velocity_mass_flowrate_and_state_in_cd_nozzle`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

here is the main function meant to calculate mass flowrates between
two control volumes using a converging diverging nozzle
This is useful for a stator (CD nozzle) and impulse turbine section
where pressure drop across this impulse turbine is (ideally) zero

given an inlet pressure and enthalpy, and velocity
flow is accelerated isentropically to the choke point

The control volume pressure and enthalpy are reflected by
state 1 and state 2 below
(p1, h1) -> nozzle -> (p2, h2)

the outlet pressure is known (equal to p2), but not the outlet enthalpy
and velocity. This will be calculated here


The algorithm here obtains the stagnation properties of the inlet
by isentropically increasing enthalpy.

It then calculates if choked flow occurs.

In the case of subsonic flow:
Then if no choked flow occurs, mass flowrates are based on the
energy balance depending on nozzle dimensions. Isentropy is also assumed


note that for this code to work, p1 needs to be greater than p2

note for this, velocity v1 is not used to calculate mass flowrate,
but just to get stagnation enthalpy

For subsonic flows, isentropy is assumed
For sonic flows without chokes, isentropy is also assumed

For sonic flows with underexpansion, Joule Thomson throttling is assumed
as a conservative estimate for entropy generation

For sonic flows with over expansion, a (p,h) algorithm is used to
iteratively determine the outlet flow properties.



```rust
pub fn calculate_velocity_mass_flowrate_and_state_in_cd_nozzle(p1: Pressure, h1: AvailableEnergy, v1: Velocity, a_throat: Area, a2: Area, p2: Pressure) -> (Velocity, MassRate, crate::prelude::TampinesSteamTableCV) { /* ... */ }
```

## Module `generator`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: \"non_snake_case\", kind: Allow}])]")`

these contain equations for generator
where flux and stuff are used

```rust
pub mod generator { /* ... */ }
```

### Types

#### Struct `ThreePhaseElectricGeneratorTurbine`

Lumped-parameter model of a three-phase synchronous generator driven by a
steam turbine rotor. The three stator windings are phase-shifted by
0 degrees, 120 degrees, and 240 degrees respectively (this corrects an
earlier version of this doc comment, which said 60 degrees).

Rotor angular velocity advances under an explicit torque balance
(`calculate_new_angular_velocity` / `advance_timestep`); per-phase EMF,
current, and total electrical power are then read out from that angular
velocity and a supplied load resistance.

```rust
pub struct ThreePhaseElectricGeneratorTurbine {
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
  pub fn new_250_megawatt_generator() -> Self { /* ... */ }
  ```
  Builds a generator preset sized for an illustrative 250 MW steam

- ```rust
  pub fn new(B: MagneticFluxDensity, A: Area, N: usize, I: MomentOfInertia, eta: Ratio, omega: AngularVelocity) -> Self { /* ... */ }
  ```
  Constructs a generator from explicit parameters: magnetic flux

- ```rust
  pub fn calculate_new_angular_velocity(self: &Self, source: Torque, load_resistance: ElectricalResistance, current_time: Time, delta_t: Time) -> AngularVelocity { /* ... */ }
  ```
  this immutably calculates new angular velocity

- ```rust
  pub fn advance_timestep(self: &mut Self, torque_source: Torque, load_resistance: ElectricalResistance, current_time: Time, delta_t: Time) { /* ... */ }
  ```
  this mutably calculates new angular velocity

- ```rust
  pub fn set_magnetic_field(self: &mut Self, B: MagneticFluxDensity) { /* ... */ }
  ```
  Sets the rotor magnetic flux density (tesla).

- ```rust
  pub fn get_emf_1(self: &Self, t: Time) -> ElectricPotential { /* ... */ }
  ```
  Computes the instantaneous back-EMF (V) of phase 1 (0 degree phase

- ```rust
  pub fn get_emf_2(self: &Self, t: Time) -> ElectricPotential { /* ... */ }
  ```
  Computes the instantaneous back-EMF (V) of phase 2 (120 degree phase

- ```rust
  pub fn get_emf_3(self: &Self, t: Time) -> ElectricPotential { /* ... */ }
  ```
  Computes the instantaneous back-EMF (V) of phase 3 (240 degree phase

- ```rust
  pub fn get_power(self: &Self, load_resistance: ElectricalResistance, t: Time) -> Power { /* ... */ }
  ```
  Computes total instantaneous three-phase electrical power (W)

- ```rust
  pub fn get_current_1(self: &Self, load_resistance: ElectricalResistance, t: Time) -> ElectricCurrent { /* ... */ }
  ```
  Computes phase-1 instantaneous current (A) delivered into

- ```rust
  pub fn get_current_2(self: &Self, load_resistance: ElectricalResistance, t: Time) -> ElectricCurrent { /* ... */ }
  ```
  Computes phase-2 instantaneous current (A) delivered into

- ```rust
  pub fn get_current_3(self: &Self, load_resistance: ElectricalResistance, t: Time) -> ElectricCurrent { /* ... */ }
  ```
  Computes phase-3 instantaneous current (A) delivered into

- ```rust
  pub fn set_omega(self: &mut Self, omega: AngularVelocity) { /* ... */ }
  ```
  Sets the rotor angular velocity (rad/s).

- ```rust
  pub fn get_omega(self: &Self) -> AngularVelocity { /* ... */ }
  ```
  Returns the current rotor angular velocity (rad/s).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
## Module `mean_flow_stages`

mean-flow, stage-by-stage turbine model: one homogeneous-equilibrium
control volume per stage, with a velocity triangle at each mean radius
Mean-flow, stage-by-stage steam turbine model (bead `op-yi7m`).

A multistage axial steam turbine solved one stage at a time along the mean
streamline, with **one homogeneous-equilibrium control volume per stage**
and a real velocity triangle at each one.

# Why this exists

Before this module the crate's only turbine physics was
[`super::converging_diverging_nozzles`] for the nozzle and
[`super::generator`] for the shaft and electrical side. Both are lumped:
nothing resolved a machine into stages, so nothing could say where along the
expansion the work came from, or where the steam went wet.

# The model

Each [`TurbineStage`] carries a [`TampinesSteamTableCV`] and is bracketed by
two pressures. Within a stage:

1. the stator expands the steam isentropically to the interstage pressure,
   and the enthalpy drop becomes nozzle exit velocity;
2. a [`VelocityTriangle`] is built at the mean radius from that velocity,
   the blade speed and the blade angles;
3. work is read off the triangle by the stage's own mechanism;
4. the control volume is advanced to the stage exit pressure with that work
   removed, through the `(p,h)` equilibrium flash.

Stages chain outlet to inlet, so the wet-steam behaviour of the last stages
falls out of the equilibrium flashes rather than being assumed.

# Impulse and reaction take different routes

This is the substance of the model, not a naming convention:

- the **impulse** part comes from the velocity triangle, by the Euler
  equation: momentum removed from the steam by turning it;
- the **reaction** part comes from a simplified cascade lift equation, with
  the blade row treated as an aerofoil row developing lift from the
  acceleration the rotor pressure drop produces.

No stage is purely one or the other. Both act in every stage, in that order,
and [`StageWorkSplit`] reports how the work divided. See
[`RotorBlading::impulse_specific_work`] and
[`RotorBlading::reaction_specific_work`] for the derivations.

# What this model does not do

- It is a **mean-line** model. There is no radial equilibrium, no spanwise
  variation, and no tip leakage.
- The equilibrium assumption is inherited from the flashes, so
  supersaturation and droplet lag in the wet stages are outside it, exactly
  as they are for the rest of this crate's HEM work.
- Stage pressures are supplied, not solved from a mass-flow match.

```rust
pub mod mean_flow_stages { /* ... */ }
```

### Modules

## Module `stage`

One stage: control volume, blading, and the two mechanisms for extracting
work from it.
One mean-flow turbine stage: a control volume, a velocity triangle, and a
rule for reading work off that triangle (bead `op-yi7m`).

# One control volume per stage

Each stage owns a [`TampinesSteamTableCV`], so the steam state is carried by
the crate's homogeneous-equilibrium flashes rather than by a perfect-gas
shortcut. That matters most in the last stages, where the expansion crosses
into the dome and the equilibrium quality is what sets the density the
triangle is built on.

# Every stage is both impulse and reaction

No rotor row is purely one or the other. A real row redirects the relative
flow *and* accelerates it, so each stage here is solved in two parts, in the
order they physically happen:

1. **Impulse first.** The blade turns the relative flow to its exit angle at
   constant relative speed, bar passage friction. The tangential momentum
   removed is work, by the Euler equation on the velocity triangle. See
   [`RotorBlading::impulse_specific_work`].
2. **Reaction after that.** The rotor passage is also a nozzle, because its
   exit pressure is below its inlet pressure. That drop accelerates the
   relative flow, and the blade row develops lift from the acceleration. See
   [`RotorBlading::reaction_specific_work`].

The balance between the two is an outcome of the rotor pressure drop, not a
label chosen up front. A rotor given no pressure drop reduces exactly to the
classical impulse stage, which is what makes that limit testable.

The two contributions are reported separately in [`StageWorkSplit`], and the
two methods are public so a unit test can exercise either mechanism on its
own.

```rust
pub mod stage { /* ... */ }
```

### Types

#### Struct `RotorBlading`

Rotor blading, carrying both the impulse and the reaction parameters.

There is no "impulse blade" or "reaction blade" type here, deliberately. A
real rotor row is never purely one or the other: it always redirects the
relative flow *and* accelerates it, so every stage built from this struct
gets both contributions. What varies along a machine is the balance, and
that balance is an outcome of the rotor pressure drop rather than a label.

```rust
pub struct RotorBlading {
    pub blade_velocity_coefficient: Ratio,
    pub lift_coefficient: Ratio,
    pub drag_coefficient: Ratio,
    pub solidity: Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `blade_velocity_coefficient` | `Ratio` | `|w2| / |w1|` for the redirection alone, capturing passage friction.<br>Unity is the loss-free blade. This drives the **impulse** part. |
| `lift_coefficient` | `Ratio` | Cascade lift coefficient `C_L`, referred to the mean relative velocity.<br>This drives the **reaction** part. |
| `drag_coefficient` | `Ratio` | Cascade drag coefficient `C_D`. Zero for the loss-free case. |
| `solidity` | `Ratio` | Solidity, chord over pitch, `c / s`. |

##### Implementations

###### Methods

- ```rust
  pub fn impulse_specific_work(self: &Self, triangle: &VelocityTriangle) -> AvailableEnergy { /* ... */ }
  ```
  Work from the **impulse** part: the rotor as a deflector.

- ```rust
  pub fn reaction_specific_work(self: &Self, triangle: &VelocityTriangle, rotor_enthalpy_drop: AvailableEnergy) -> AvailableEnergy { /* ... */ }
  ```
  Work from the **reaction** part: the rotor as an aerofoil cascade.

- ```rust
  pub fn reaction_tangential_velocity_change(self: &Self, triangle: &VelocityTriangle, rotor_enthalpy_drop: AvailableEnergy) -> Velocity { /* ... */ }
  ```
  The tangential velocity change the **reaction** part accounts for, which

- ```rust
  pub fn tangential_velocity_change(self: &Self, triangle: &VelocityTriangle, rotor_enthalpy_drop: AvailableEnergy) -> Velocity { /* ... */ }
  ```
  The total tangential velocity change of the stage, both mechanisms

- ```rust
  pub fn specific_work_split(self: &Self, triangle: &VelocityTriangle, rotor_enthalpy_drop: AvailableEnergy) -> StageWorkSplit { /* ... */ }
  ```
  Both parts of the stage work, in the order they physically happen:

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `StageWorkSplit`

How one stage's work divides between its two mechanisms.

Reported rather than summed away, because the split is the thing a
stage-resolved model can say and a lumped one cannot.

```rust
pub struct StageWorkSplit {
    pub impulse: AvailableEnergy,
    pub reaction: AvailableEnergy,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `impulse` | `AvailableEnergy` | Work from redirecting the relative flow, by the Euler equation on the<br>velocity triangle. |
| `reaction` | `AvailableEnergy` | Work from the rotor pressure drop, by the cascade lift equation. |

##### Implementations

###### Methods

- ```rust
  pub fn total(self: &Self) -> AvailableEnergy { /* ... */ }
  ```
  Total specific work of the stage.

- ```rust
  pub fn reaction_fraction(self: &Self) -> Ratio { /* ... */ }
  ```
  The fraction of stage work that came from the reaction part.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `StageGeometry`

Mean-line geometry of one stage.

```rust
pub struct StageGeometry {
    pub mean_radius: Length,
    pub nozzle_angle: Angle,
    pub relative_exit_angle: Angle,
    pub nozzle_velocity_coefficient: Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mean_radius` | `Length` | Mean blade radius, which with shaft speed sets `U`. |
| `nozzle_angle` | `Angle` | Stator exit angle `alpha1` from axial. |
| `relative_exit_angle` | `Angle` | Rotor blade exit angle `beta2` from axial. Negative turns the flow back<br>against blade motion, which is what extracts work. |
| `nozzle_velocity_coefficient` | `Ratio` | Fraction of the ideal nozzle exit speed actually achieved, capturing<br>stator loss. Unity is the loss-free nozzle. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `TurbineStage`

One stage: geometry, blading, and the two pressures that bracket it.

The interstage and exit pressures are supplied rather than derived. That is
a deliberate choice: splitting an enthalpy drop by an assumed degree of
reaction would need an `(h,s)` flash, whose known gaps at the triple point
and at 1000 bar are documented in this crate's `CLAUDE.md`. Supplying
pressures keeps the stage on the `(p,s)` and `(p,h)` paths, which are the
validated ones. The degree of reaction is then *reported* by
[`VelocityTriangle::get_degree_of_reaction`] rather than prescribed.

```rust
pub struct TurbineStage {
    pub geometry: StageGeometry,
    pub blading: RotorBlading,
    pub stator_exit_pressure: Pressure,
    pub rotor_exit_pressure: Pressure,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `geometry` | `StageGeometry` | Mean-line geometry. |
| `blading` | `RotorBlading` | Rotor blading, carrying both the impulse and the reaction parameters. |
| `stator_exit_pressure` | `Pressure` | Pressure at stator exit / rotor inlet. |
| `rotor_exit_pressure` | `Pressure` | Pressure at rotor exit, which is the next stage's inlet pressure. |

##### Implementations

###### Methods

- ```rust
  pub fn expand(self: &Self, inlet: TampinesSteamTableCV, shaft_speed: AngularVelocity) -> StageOutcome { /* ... */ }
  ```
  Expands `inlet` through this stage at the given shaft speed.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `StageOutcome`

What one stage did: the outlet state, the kinematics it ran at, and the
work it produced against the work it could ideally have produced.

```rust
pub struct StageOutcome {
    pub outlet: crate::interfaces::object_oriented_programming::TampinesSteamTableCV,
    pub triangle: super::velocity_triangle::VelocityTriangle,
    pub work_split: StageWorkSplit,
    pub isentropic_specific_work: AvailableEnergy,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `outlet` | `crate::interfaces::object_oriented_programming::TampinesSteamTableCV` | Steam state leaving the rotor. |
| `triangle` | `super::velocity_triangle::VelocityTriangle` | The mean-radius triangle this stage ran at. |
| `work_split` | `StageWorkSplit` | How the extracted work divided between the two mechanisms. |
| `isentropic_specific_work` | `AvailableEnergy` | Work per unit mass an isentropic expansion over the same pressure range<br>would have given. |

##### Implementations

###### Methods

- ```rust
  pub fn specific_work(self: &Self) -> AvailableEnergy { /* ... */ }
  ```
  Total specific work of the stage, both mechanisms summed.

- ```rust
  pub fn stage_efficiency(self: &Self) -> Ratio { /* ... */ }
  ```
  Stage efficiency, actual work over isentropic work.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
## Module `velocity_triangle`

Mean-radius velocity triangles.
Mean-radius velocity triangle for one axial turbine stage.

# Sign and angle convention

Angles are measured **from the axial direction**, positive in the direction
the blades move. Axial velocity is taken as constant across the rotor, which
is the usual mean-line assumption and the reason this is a *mean-flow*
model rather than a through-flow one.

The stations are the classical three:

- **0** stage inlet (stator inlet),
- **1** stator exit / rotor inlet,
- **2** rotor exit.

Only stations 1 and 2 carry a triangle; station 0 supplies the
thermodynamic state that the stator expands.

# What the triangle is for

It converts a thermodynamic state change into the kinematics of the flow,
so that the work leaving the stage is a consequence of how much the steam
is *turned*, not of an assumed stage efficiency. For an impulse stage this
is the whole story, and [`VelocityTriangle::euler_specific_work`] is the
answer. A reaction stage uses the same triangle but reads work off a blade
lift coefficient instead — see [`super::stage::StageKind`].

```rust
pub mod velocity_triangle { /* ... */ }
```

### Types

#### Struct `VelocityTriangle`

The mean-radius velocity triangle of one stage, at rotor inlet (station 1)
and rotor exit (station 2).

Every field is a resolved component rather than a magnitude-and-angle pair,
because the work relations want components and re-deriving them at each use
invites sign mistakes.

```rust
pub struct VelocityTriangle {
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
  pub fn new_from_nozzle_and_blade_angles(blade_speed: Velocity, absolute_velocity_in: Velocity, nozzle_angle: Angle, relative_exit_angle: Angle, blade_velocity_coefficient: Ratio) -> Self { /* ... */ }
  ```
  Builds the triangle from the stator exit kinematics and the rotor blade

- ```rust
  pub fn tangential_velocity_change(self: &Self) -> Velocity { /* ... */ }
  ```
  The tangential velocity the rotor removed from the steam,

- ```rust
  pub fn euler_specific_work(self: &Self) -> AvailableEnergy { /* ... */ }
  ```
  Specific work by the Euler turbomachinery equation,

- ```rust
  pub fn get_blade_speed(self: &Self) -> Velocity { /* ... */ }
  ```
  Blade speed at the mean radius.

- ```rust
  pub fn get_axial_velocity(self: &Self) -> Velocity { /* ... */ }
  ```
  Axial velocity, constant across the rotor by assumption.

- ```rust
  pub fn get_absolute_tangential_in(self: &Self) -> Velocity { /* ... */ }
  ```
  Tangential component of the absolute velocity at rotor inlet.

- ```rust
  pub fn get_absolute_tangential_out(self: &Self) -> Velocity { /* ... */ }
  ```
  Tangential component of the absolute velocity at rotor exit.

- ```rust
  pub fn get_relative_tangential_in(self: &Self) -> Velocity { /* ... */ }
  ```
  Tangential component of the relative velocity at rotor inlet.

- ```rust
  pub fn get_relative_tangential_out(self: &Self) -> Velocity { /* ... */ }
  ```
  Tangential component of the relative velocity at rotor exit.

- ```rust
  pub fn get_relative_angle_in(self: &Self) -> Angle { /* ... */ }
  ```
  Relative flow angle at rotor inlet, `beta1`, from axial.

- ```rust
  pub fn get_relative_angle_out(self: &Self) -> Angle { /* ... */ }
  ```
  Relative flow angle at rotor exit, `beta2`, from axial.

- ```rust
  pub fn get_relative_speed_in(self: &Self) -> Velocity { /* ... */ }
  ```
  Relative speed at rotor inlet, `|w1|`.

- ```rust
  pub fn get_relative_speed_out(self: &Self) -> Velocity { /* ... */ }
  ```
  Relative speed at rotor exit, `|w2|`.

- ```rust
  pub fn get_absolute_speed_out(self: &Self) -> Velocity { /* ... */ }
  ```
  Absolute speed at rotor exit, `|c2|`. The kinetic energy in this is the

- ```rust
  pub fn get_mean_relative_angle(self: &Self) -> Angle { /* ... */ }
  ```
  The **mean relative flow angle** of the rotor cascade, defined by

- ```rust
  pub fn get_mean_relative_speed(self: &Self) -> Velocity { /* ... */ }
  ```
  The **mean relative speed**, `w_m = c_x / cos(beta_m)`, the velocity a

- ```rust
  pub fn get_degree_of_reaction(self: &Self) -> Ratio { /* ... */ }
  ```
  Degree of reaction inferred from the kinematics,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
## Module `transient`

The same stage chain driven in time by the 1-D HEM PIMPLE array
(~~KNP hybrid solver~~, retired 2026-10-16).
Transient mean-flow turbine: the stage chain driven by the crate's 1-D
HEM PIMPLE array, [`TampinesSteamArray`] (bead `op-yi7m`). ~~driven by the
crate's 1-D HEM KNP hybrid solver~~ **CORRECTED 2026-10-16**: the KNP
hybrid was retired (see "Why plain PIMPLE" below).

# What changes against the steady model

[`super::MeanFlowTurbine`] solves the stage chain once per call, with the
stage pressures supplied. That is a design-point model: it answers what the
machine does at a given operating point, and nothing about how it gets
there.

This model hands the flow field to [`TampinesSteamArray`] instead. The mesh
is the machine, **one cell per stage**, and the solver owns everything the
steady model was told:

- **Pressure is solved, not supplied.** The pressure distribution along the
  machine is whatever the PIMPLE pressure equation produces from the
  boundary conditions.
- **Axial velocity is solved, not derived from an enthalpy drop.** The
  velocity triangle is built on the axial velocity the momentum equation
  gives, so the triangle follows the transient rather than a design
  assumption.
- **The work appears as an energy sink.** Each stage's specific work is
  converted to a power using the local mass flux and registered as a
  negative per-cell power source, so the energy equation sees the work
  leaving through the shaft.

# Why plain PIMPLE (the KNP hybrid was retired 2026-10-16)

~~The constructor sets `SolverMode::HybridAllMach`, PIMPLE with Mach-blended
KNP central-upwind dissipation. Turbine passages run near-sonic at the
nozzle throats and the last stages expand into the dome, which is exactly
the regime the plain pressure-based path rings in. The hybrid damps that
ringing while keeping the flashing behaviour, and it is stable over a full
transient. The mode is public, so a caller who wants the historical
bit-identical PIMPLE path can still ask for it.~~

**RETIRED 2026-10-16** (maintainer decision "retire the KNP hybrid", gh:#914).
After gh:#900 made the array upstream-faithful, the hybrid *added* ringing
on Edwards blowdown instead of damping it, so `SolverMode` was removed from
[`TampinesSteamArray`] and this machine now runs the array's plain PIMPLE
path. The density-based KNP path lives in `crate::hem::HemPipe`.

**The switch changed nothing here, and the reason is worth knowing.**
Measured 2026-10-16, before deleting the hybrid, by running every scenario
of `transient_tests.rs` under both modes (release, x86_64-linux):

| scenario | `HybridAllMach` (old default) | `Pimple` (now) |
|---|---|---|
| 1 step at 420 rad/s: shaft power / torque | 3.760603984866e7 W / 8.953819011585e4 N m | identical |
| 10 steps: electrical power / shaft speed | 5.466174525036e7 W / 419.5509615047 rad/s | identical |
| 20 steps: cell pressures \[Pa\] | 3983988.15158468, 3074050.3216784694, 2298233.230397977, 1681831.872790971 | identical, bit for bit |
| spin-up from rest, after 51 steps: speed / power | 32.22944815266 rad/s / 1.654526780372e7 W | identical |
| 200 steps from rest, 100 ohm / 1 ohm: speed | 285.7730811747 / 181.8973309631 rad/s | identical |

The prediction written before the run was "little change, because the
flow is subsonic". That was right about the outcome and wrong about the
reason: the HEM Mach number reaches **0.70** in the 200-step runs, well
inside the hybrid's `0.3–1.0` blend window. The hybrid was nonetheless a
no-op because its rarefied-tail density taper zeroed the dissipation below
50 kg/m³, and superheated steam at 16–40 bar / 400 °C is roughly
6–14 kg/m³. **The hybrid never acted on this machine**, so the "damps
ringing at the nozzle throats" rationale struck through above was never
exercised by any test.

# The shaft is closed-loop

Shaft speed is **not** an input. Each stage's tangential momentum change
becomes a torque, the torques sum, and
[`ThreePhaseElectricGeneratorTurbine`] advances the rotor against the
electrical braking of its own load. The next timestep's velocity triangles
are built at the new speed, so steam, shaft and load are coupled.

Torque is built from tangential momentum, `T = mdot * r * dc_theta`, and
**not** by dividing shaft power by shaft speed. The difference matters at
exactly one point and it is the important one: at standstill the blade speed
is zero, so the work is zero, but the tangential momentum change is not.
That is a turbine's starting torque. Recovering torque from power would give
`0/0` there and the machine could never spin up.

# What is still assumed

- **The rotor's share of the local pressure drop is an input**, given per
  stage as [`TransientStage::rotor_drop_fraction`], because with pressure
  solved rather than supplied there is no interstage station to read it off.
- Everything the steady model does not do is still not done: mean-line only,
  no radial equilibrium, no spanwise variation, no tip leakage.

```rust
pub mod transient { /* ... */ }
```

### Types

#### Struct `TransientStage`

One stage of the transient machine.

Carries no pressures: in the transient model those are solved by the array,
not supplied. What remains is the mean-line geometry, the blading, and how
much of the locally solved pressure drop the rotor itself takes.

```rust
pub struct TransientStage {
    pub geometry: super::stage::StageGeometry,
    pub blading: super::stage::RotorBlading,
    pub rotor_drop_fraction: Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `geometry` | `super::stage::StageGeometry` | Mean-line geometry. |
| `blading` | `super::stage::RotorBlading` | Rotor blading, carrying both the impulse and the reaction parameters. |
| `rotor_drop_fraction` | `Ratio` | Fraction of this stage's locally solved isentropic drop taken across the<br>rotor rather than the stator. Zero is a pure impulse rotor; 0.5 is the<br>classical fifty-percent reaction stage. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `TransientStageOutcome`

What one stage did during one timestep.

```rust
pub struct TransientStageOutcome {
    pub triangle: super::velocity_triangle::VelocityTriangle,
    pub work_split: super::stage::StageWorkSplit,
    pub mass_flow: MassRate,
    pub torque: Torque,
    pub shaft_power: Power,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `triangle` | `super::velocity_triangle::VelocityTriangle` | The mean-radius triangle this stage ran at, built on the solved axial<br>velocity. |
| `work_split` | `super::stage::StageWorkSplit` | How the extracted work divided between the two mechanisms. |
| `mass_flow` | `MassRate` | Mass flow through this cell, from the solved density and velocity. |
| `torque` | `Torque` | Shaft torque this stage applied, `mdot * r * dc_theta`. Finite at<br>standstill, which is what lets the machine start. |
| `shaft_power` | `Power` | Shaft power this stage extracted. Positive means work leaving the steam.<br>Zero at standstill even when the torque is not. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `TransientStepOutcome`

What the machine did during one timestep.

```rust
pub struct TransientStepOutcome {
    pub stage_outcomes: Vec<TransientStageOutcome>,
    pub shaft_speed_before: AngularVelocity,
    pub shaft_speed_after: AngularVelocity,
    pub electrical_power: Power,
    pub time: Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stage_outcomes` | `Vec<TransientStageOutcome>` | One entry per stage, in flow order. |
| `shaft_speed_before` | `AngularVelocity` | Shaft speed the stages were evaluated at, before the shaft advanced. |
| `shaft_speed_after` | `AngularVelocity` | Shaft speed after the generator's torque balance advanced. |
| `electrical_power` | `Power` | Instantaneous three-phase electrical power delivered into the load,<br>evaluated at the new shaft speed. |
| `time` | `Time` | Simulated time at the end of this step. |

##### Implementations

###### Methods

- ```rust
  pub fn total_shaft_power(self: &Self) -> Power { /* ... */ }
  ```
  Total shaft power over all stages.

- ```rust
  pub fn total_torque(self: &Self) -> Torque { /* ... */ }
  ```
  Total shaft torque over all stages.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `TransientMeanFlowTurbine`

A multistage axial steam turbine solved in time on the 1-D HEM array.

```rust
pub struct TransientMeanFlowTurbine {
    pub array: crate::openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray,
    pub stages: Vec<TransientStage>,
    pub generator: crate::steam_turbine_equations::generator::ThreePhaseElectricGeneratorTurbine,
    pub load_resistance: ElectricalResistance,
    pub current_time: Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `array` | `crate::openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray` | The 1-D HEM solver. One cell per stage, and public so a caller can set<br>boundary conditions, initial conditions and solver controls directly. |
| `stages` | `Vec<TransientStage>` | Stages in flow order, one per cell of `array`. |
| `generator` | `crate::steam_turbine_equations::generator::ThreePhaseElectricGeneratorTurbine` | The shaft and its generator. Owns the rotor speed, which is therefore a<br>solved quantity rather than an input. |
| `load_resistance` | `ElectricalResistance` | Electrical load the generator works into. Smaller resistance means<br>heavier braking, so this is the load-change knob for a transient. |
| `current_time` | `Time` | Simulated time, advanced one `delta_t` per [`Self::step`]. The<br>generator's back-EMF is time-dependent, so this is state, not<br>bookkeeping. |

##### Implementations

###### Methods

- ```rust
  pub fn new(stages: Vec<TransientStage>, length: Length, xs_area: Area, delta_t: Time, generator: ThreePhaseElectricGeneratorTurbine, load_resistance: ElectricalResistance) -> Result<Self, MeshError> { /* ... */ }
  ```
  Builds the machine, meshing one cell per stage. The array runs its

- ```rust
  pub fn shaft_speed(self: &Self) -> AngularVelocity { /* ... */ }
  ```
  Current shaft speed, read from the generator that owns it.

- ```rust
  pub fn initialise_uniform(self: &mut Self, pressure: Pressure, temperature: ThermodynamicTemperature) { /* ... */ }
  ```
  Sets every cell to one uniform `(p, T)` state.

- ```rust
  pub fn stage_control_volume(self: &Self, stage_index: usize) -> TampinesSteamTableCV { /* ... */ }
  ```
  The steam state of one stage, as a control volume.

- ```rust
  pub fn step(self: &mut Self) -> TransientStepOutcome { /* ... */ }
  ```
  Advances the machine one timestep.

- ```rust
  pub fn run(self: &mut Self, n_steps: usize) -> Option<TransientStepOutcome> { /* ... */ }
  ```
  Advances `n_steps` timesteps, returning the last step's outcome.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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

#### Struct `MeanFlowTurbine`

A multistage axial steam turbine, solved along the mean streamline.

The stage list is ordered from admission to exhaust. Nothing here checks
that the stage pressures descend monotonically; a stage handed a rising
pressure produces no nozzle velocity and therefore no work, which shows up
in the outcome rather than as a panic.

```rust
pub struct MeanFlowTurbine {
    pub stages: Vec<TurbineStage>,
    pub shaft_speed: AngularVelocity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stages` | `Vec<TurbineStage>` | Stages in flow order, admission first. |
| `shaft_speed` | `AngularVelocity` | Shaft speed, shared by every stage. |

##### Implementations

###### Methods

- ```rust
  pub fn new(stages: Vec<TurbineStage>, shaft_speed: AngularVelocity) -> Self { /* ... */ }
  ```
  Builds a turbine from stages in flow order and a shaft speed.

- ```rust
  pub fn expand(self: &Self, inlet: TampinesSteamTableCV) -> TurbineOutcome { /* ... */ }
  ```
  Expands `inlet` through every stage in turn, each stage's outlet feeding

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `TurbineOutcome`

What a whole machine did, stage by stage.

```rust
pub struct TurbineOutcome {
    pub stage_outcomes: Vec<StageOutcome>,
    pub outlet: crate::interfaces::object_oriented_programming::TampinesSteamTableCV,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stage_outcomes` | `Vec<StageOutcome>` | One entry per stage, in flow order. |
| `outlet` | `crate::interfaces::object_oriented_programming::TampinesSteamTableCV` | Steam state at the exhaust. |

##### Implementations

###### Methods

- ```rust
  pub fn total_specific_work(self: &Self) -> AvailableEnergy { /* ... */ }
  ```
  Total specific work of the machine, summed over stages.

- ```rust
  pub fn total_isentropic_specific_work(self: &Self) -> AvailableEnergy { /* ... */ }
  ```
  Total isentropic specific work, summed stage by stage.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

#### Re-export `stage::*`

```rust
pub use stage::*;
```

#### Re-export `velocity_triangle::*`

```rust
pub use velocity_triangle::*;
```

#### Re-export `transient::*`

```rust
pub use transient::*;
```

### Re-exports

#### Re-export `converging_diverging_nozzles::*`

```rust
pub use converging_diverging_nozzles::*;
```

#### Re-export `generator::*`

```rust
pub use generator::*;
```

#### Re-export `mean_flow_stages::*`

```rust
pub use mean_flow_stages::*;
```

## Module `hem`

Homogeneous Equilibrium Model (HEM) transient pipe flow: a density-based
KNP central-upwind solver on `[ρ, ρu, ρE]`, closed by IAPWS-IF97 (or any
[`hem::HemEquationOfState`]). Re-exported by `tampines` as `tampines::hem`,
which adds a CoolProp closure. See the module documentation.
Homogeneous Equilibrium Model (HEM) transient pipe flow: a density-based
KNP central-upwind solver on `W = [ρ, ρu, ρE]` (epic gh #896, issue
gh #902).

# What this module answers

*How does a 1-D pipe of water (or any fluid with an equation of state)
depressurise, flash and discharge, when both phases share one velocity, one
pressure and one temperature?* That is the 3-equation HEM rung, below
drift-flux (slip) and two-fluid (thermal non-equilibrium), which live in
`tampines::multiphase_1d`.

# Where it sits (maintainer decision D2, 2026-10-16)

HEM is a thermodynamic model, and Edwards–O'Brien is this crate's
critical-flow validation case, so the solver lives **here**, in
`tampines-steam-tables`. `tampines` re-exports it as `tampines::hem` and adds
a CoolProp closure, because only `tampines` depends on both property crates.
No dependency edge is added either way.

# Entry points

- [`HemPipe`]: the solver. `HemPipe::new(geometry, eos, initial, numerics)`,
  then [`HemPipe::set_boundary`], [`HemPipe::step`] or
  [`HemPipe::run_until`], then [`HemPipe::snapshot`]. Its type parameter
  defaults to [`Iapws97Hem`], so `HemPipe` alone means water.
- Closures: [`Iapws97Hem`] (IAPWS-IF97 water/steam) and [`IdealGasHem`]
  (the Sod limit), both implementing the compile-time trait
  [`HemEquationOfState`].
- Configuration: [`HemPipeGeometry`], [`HemInitialCondition`] /
  [`PrimitiveState`], [`HemBoundary`] / [`PipeEnd`], [`HemNumerics`]
  (with [`TimeStepControl`], [`TimeIntegrator`] and the basic-lib
  [`FluxLimiter`]).
- Errors: [`HemError`]. Nothing is clamped: a negative density or pressure
  is an error and the step is undone.

# Physics, and what is not here

Inviscid, adiabatic, frictionless, horizontal, equilibrium two-phase (both
phases at `T_sat(p)` inside the dome, no slip). The sound speed is the
homogeneous-**equilibrium** one throughout ([`eos`] module doc). Not here:
wall friction, gravity, heat transfer, slip, metastable liquid.

# V&V

Methodology and results: `verification_and_validation/hem_pipe_vs_exact_riemann_and_edwards.md`.
Tests: `tests/hem_pipe_verification.rs` (Sod, positivity, Courant,
conservation, pinned defaults, the HEM water double rarefaction) and
`tests/hem_pipe_edwards.rs` (Edwards–O'Brien against GS-1).

> Research, education and V&V only. Not for nuclear facility operation,
> reactor control, licensing or safety-critical decisions. See
> `RESPONSIBLE_USE.md`.

```rust
pub mod hem { /* ... */ }
```

### Modules

## Module `eos`

The equation-of-state contract a [`super::HemPipe`] is closed with, and the
calorically perfect gas that runs the Sod limit through the same solver.

# What belongs here

- [`HemEquationOfState`]: a **compile-time** trait (no `dyn`, per the
  workspace rule). The solver is generic over it; dispatch over several
  backends at run time is done with an enum one level up (`tampines`'
  `HemEos`, which adds CoolProp).
- [`HemThermoState`]: what every closure call returns.
- [`BreakFlow`]: what a critical-flow model returns for a choked break.
- [`IdealGasHem`]: `p = (γ − 1)ρe`, for verification.

The IAPWS-IF97 implementation is [`super::Iapws97Hem`].

# The sound speed is the EQUILIBRIUM one

Every implementation must return the **homogeneous-equilibrium** sound
speed `c² = (∂p/∂ρ)_s` along the equilibrium isentrope, in which liquid
flashes and vapour condenses inside the wave. Inside the two-phase dome it
is far below the frozen (Wood–Wallis) speed. The solver places its KNP
characteristics `u ± c` and its Courant number with it, so a frozen speed
would put the waves of a flashing flow in the wrong place.

```rust
pub mod eos { /* ... */ }
```

### Types

#### Struct `HemThermoState`

One thermodynamic state as an equation of state closes it: everything the
solver and a caller need, in `uom` units.

```rust
pub struct HemThermoState {
    pub pressure: uom::si::f64::Pressure,
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub density: uom::si::f64::MassDensity,
    pub specific_internal_energy: uom::si::f64::AvailableEnergy,
    pub specific_enthalpy: uom::si::f64::AvailableEnergy,
    pub equilibrium_sound_speed: uom::si::f64::Velocity,
    pub quality: Option<uom::si::f64::Ratio>,
    pub void_fraction: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `uom::si::f64::Pressure` | Absolute static pressure. |
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature. Inside the dome, the saturation temperature at `pressure`. |
| `density` | `uom::si::f64::MassDensity` | Mixture mass density `ρ`. |
| `specific_internal_energy` | `uom::si::f64::AvailableEnergy` | Specific internal energy `e`, J/kg. |
| `specific_enthalpy` | `uom::si::f64::AvailableEnergy` | Specific enthalpy `h = e + p/ρ`, J/kg. |
| `equilibrium_sound_speed` | `uom::si::f64::Velocity` | Homogeneous-**equilibrium** sound speed (never the frozen Wood–Wallis<br>speed); see the module doc. |
| `quality` | `Option<uom::si::f64::Ratio>` | Equilibrium vapour mass fraction inside the two-phase dome; `None` for a<br>single-phase state. |
| `void_fraction` | `uom::si::f64::Ratio` | Homogeneous (no-slip) vapour volume fraction, `x·ρ/ρ_g` in the dome,<br>`0` for liquid, `1` for vapour or gas. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `BreakFlow`

What a critical-flow model returns for a break discharging from a
stagnation state to a back pressure.

```rust
pub struct BreakFlow {
    pub mass_flux: uom::si::f64::MassFlux,
    pub choked: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mass_flux` | `uom::si::f64::MassFlux` | Mass flux through the break area, kg/(m²·s). Zero when the stagnation<br>pressure does not exceed the back pressure (no backflow is modelled). |
| `choked` | `bool` | `true` when the flow is choked (throat pressure above the back<br>pressure), so the mass flux is the critical one. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `IdealGasHem`

Calorically perfect gas, `p = (γ − 1)ρe`, `T = p/(ρR)`, `c = √(γp/ρ)`.

For verification only: it runs the classical Sod and Toro problems through
the same KNP solver as water. Single phase by construction, so the
equilibrium and frozen sound speeds coincide. `γ > 1`, `R > 0`.

```rust
pub struct IdealGasHem {
    pub gamma: f64,
    pub specific_gas_constant: uom::si::f64::SpecificHeatCapacity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gamma` | `f64` | Ratio of specific heats `γ = c_p/c_v`, dimensionless, `> 1`. |
| `specific_gas_constant` | `uom::si::f64::SpecificHeatCapacity` | Specific gas constant `R`, J/(kg·K), `> 0`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(gamma: Ratio, specific_gas_constant: SpecificHeatCapacity) -> Result<Self, HemError> { /* ... */ }
  ```
  Build from `γ` (dimensionless, `> 1`) and `R` (J/(kg·K), `> 0`).

- ```rust
  pub fn air() -> Self { /* ... */ }
  ```
  Dry air, `γ = 1.4`, `R = 8314.46261815324/28.96 ≈ 287.1 J/(kg·K)`: the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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

- **HemEquationOfState**
  - ```rust
    fn name(self: &Self) -> &'static str { /* ... */ }
    ```

  - ```rust
    fn state_rho_e(self: &Self, density: MassDensity, specific_internal_energy: AvailableEnergy, _pressure_hint: Option<Pressure>) -> Result<HemThermoState, HemError> { /* ... */ }
    ```

  - ```rust
    fn state_p_t(self: &Self, pressure: Pressure, temperature: ThermodynamicTemperature) -> Result<HemThermoState, HemError> { /* ... */ }
    ```

  - ```rust
    fn state_p_h(self: &Self, pressure: Pressure, specific_enthalpy: AvailableEnergy) -> Result<HemThermoState, HemError> { /* ... */ }
    ```

  - ```rust
    fn state_p_rho(self: &Self, pressure: Pressure, density: MassDensity) -> Result<HemThermoState, HemError> { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

#### Trait `HemEquationOfState`

**Attributes:**

- `Other("#[attr = OnUnimplemented {directive: Directive {is_rustc_attr: false,\nfilters: [],\nmessage: FormatString {input: \"`{Self}` is not a HEM equation of state\",\npieces: [Lit(\"`\"), Arg(SelfUpper), Lit(\"` is not a HEM equation of state\")]},\nnotes: [FormatString {input: \"implemented by `Iapws97Hem` and `IdealGasHem` here, and by `tampines::hem::HemEos` (which adds CoolProp)\",\npieces: [Lit(\"implemented by `Iapws97Hem` and `IdealGasHem` here, and by `tampines::hem::HemEos` (which adds CoolProp)\")]}]}}]")`

The equation of state that closes the HEM system `W = [ρ, ρu, ρE]`.

Implemented in this crate by [`super::Iapws97Hem`] (water/steam) and
[`IdealGasHem`]; `tampines` adds CoolProp and an enum that dispatches over
all three. A compile-time contract only: never use it as `dyn`.

```rust
pub trait HemEquationOfState: Clone + core::fmt::Debug {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `name`: A short name for messages, e.g. `"IAPWS-IF97"`.
- `state_rho_e`: Close a state from the solver's conserved variables: mass density `ρ`
- `state_p_t`: Close a single-phase state from pressure and temperature. Inside the
- `state_p_h`: Close a state from pressure and specific enthalpy (valid in the dome).
- `state_p_rho`: Close a state from pressure and density (the natural input of the

##### Provided Methods

- ```rust
  fn break_flow(self: &Self, stagnation_pressure: Pressure, stagnation_enthalpy: AvailableEnergy, back_pressure: Pressure) -> Result<BreakFlow, HemError> { /* ... */ }
  ```
  Mass flux through a break from stagnation `(p0, h0)` to `back_pressure`

##### Implementations

This trait is implemented for the following types:

- `IdealGasHem`
- `Iapws97Hem`

### Functions

#### Function `pressure_from_rho_e`

Recover pressure from density `ρ` and specific internal energy `e` for any
closure that has a `(p, h)` flash: the bracketed root of

```text
g(p) = ρ · v(p, h = e + p/ρ) − 1
```

by `petir::roots` Brent. Any root is an equilibrium state with the
requested `(v, e)`, since `e = h − p·v`; for a fluid in stable equilibrium
that state is unique, so is the sign change.

The bracket grows geometrically from `pressure_hint` (or the geometric
middle of `pressure_range`), `[p/f, p·f]` with `f = 1 + 1e-4` squared at
each try, clipped to `pressure_range`. Every probe the flash accepts is
kept, and the bracket is the first pair of pressure-adjacent accepted
probes whose residuals change sign; probes the flash rejects
(`specific_volume_ph` returns `None`) are skipped. A bracket wider than a factor 2 is then narrowed on a 48-point log grid to
the first adjacent pair of accepted probes that changes sign, so Brent only
probes between two pressures the flash accepted. Brent then converges to
`relative_tolerance` on the bracket width.

Used by [`super::Iapws97Hem`] and by `tampines`' CoolProp closure, so both
invert the same way. `specific_volume_ph(p, h)` must return `v(p, h)` in
m³/kg, or `None` outside its domain; it must not panic there.

# Errors

[`HemError::EquationOfState`] if no sign change is found within the range,
or the Brent solve fails (a probe inside the bracket left the domain, or
the iteration limit was hit).

```rust
pub fn pressure_from_rho_e<F>(density: uom::si::f64::MassDensity, specific_internal_energy: uom::si::f64::AvailableEnergy, pressure_hint: Option<uom::si::f64::Pressure>, pressure_range: (uom::si::f64::Pressure, uom::si::f64::Pressure), relative_tolerance: f64, specific_volume_ph: F) -> Result<uom::si::f64::Pressure, super::HemError>
where
    F: Fn(uom::si::f64::Pressure, uom::si::f64::AvailableEnergy) -> Option<uom::si::f64::SpecificVolume> { /* ... */ }
```

### Re-exports

#### Re-export `pressure_from_rho_e`

```rust
pub use eos::pressure_from_rho_e;
```

#### Re-export `BreakFlow`

```rust
pub use eos::BreakFlow;
```

#### Re-export `HemEquationOfState`

```rust
pub use eos::HemEquationOfState;
```

#### Re-export `HemThermoState`

```rust
pub use eos::HemThermoState;
```

#### Re-export `IdealGasHem`

```rust
pub use eos::IdealGasHem;
```

#### Re-export `HemError`

```rust
pub use error::HemError;
```

#### Re-export `Iapws97Hem`

```rust
pub use iapws97::Iapws97Hem;
```

#### Re-export `P_RHO_E_REL_TOL`

```rust
pub use iapws97::P_RHO_E_REL_TOL;
```

#### Re-export `FluxLimiter`

```rust
pub use outram_foam_basic_lib::limiters::FluxLimiter;
```

#### Re-export `HemBoundary`

```rust
pub use pipe::HemBoundary;
```

#### Re-export `HemCellState`

```rust
pub use pipe::HemCellState;
```

#### Re-export `HemInitialCondition`

```rust
pub use pipe::HemInitialCondition;
```

#### Re-export `HemNumerics`

```rust
pub use pipe::HemNumerics;
```

#### Re-export `HemPipe`

```rust
pub use pipe::HemPipe;
```

#### Re-export `HemPipeGeometry`

```rust
pub use pipe::HemPipeGeometry;
```

#### Re-export `HemRunSummary`

```rust
pub use pipe::HemRunSummary;
```

#### Re-export `HemSnapshot`

```rust
pub use pipe::HemSnapshot;
```

#### Re-export `HemStepReport`

```rust
pub use pipe::HemStepReport;
```

#### Re-export `PipeEnd`

```rust
pub use pipe::PipeEnd;
```

#### Re-export `PrimitiveState`

```rust
pub use pipe::PrimitiveState;
```

#### Re-export `TimeIntegrator`

```rust
pub use pipe::TimeIntegrator;
```

#### Re-export `TimeStepControl`

```rust
pub use pipe::TimeStepControl;
```

#### Re-export `FIXED_STEP_COURANT_LIMIT`

```rust
pub use pipe::FIXED_STEP_COURANT_LIMIT;
```

## Module `openfoam_algorithms`

reference openfoam algorithms which will be combined with steam
tables for solving simple two phase flow problems
Pure-Rust ports of OpenFOAM finite-volume solver algorithms, used as
reference/study material and (for `rhoPimpleFoam`) as an actively-used
solver. Most submodules here are `mod`/`pub(crate)` — kept in-tree for
their FV building blocks (see `openfoam_source`) rather than exposed as
part of this crate's public API. The sole public export is
[`rhoPimpleFoam`], which hosts `TampinesSteamArray`, a 1-D compressible
PIMPLE pipe solver built on TAMPINES IAPWS-IF97 steam-table properties.

```rust
pub mod openfoam_algorithms { /* ... */ }
```

### Modules

## Module `rhoPimpleFoam`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: \"non_snake_case\", kind: Allow}])]")`

Rust port of OpenFOAM's `rhoPimpleFoam` (compressible PIMPLE) solver,
specialised into `TampinesSteamArray` — a 1-D pipe solver over TAMPINES
IAPWS-IF97 steam properties. This is the only public export of
`openfoam_algorithms`.
# A `rhoPimpleFoam` derivation from first principles — the HEM-closed 1-D pipe

This module is the solver ([`TampinesSteamArray`]) that marches compressible,
flashing steam/water down a 1-D pipe in time. It is a Rust re-implementation
of OpenFOAM's `rhoPimpleFoam` (the reproduced C++ `main()` is kept verbatim
below for provenance), **closed with the real IAPWS-IF97 steam tables as a
homogeneous-equilibrium (HEM) two-phase equation of state** rather than the
perfect-gas closure the stock solver ships with.

The comment below is written for a reader who has *some* CFD background —
you know what a finite-volume mesh, a divergence, and a linear solve are —
but who has never really understood *why* `rhoPimpleFoam` is built the way it
is. Everything is derived in order, each step leaning on the previous one.
Navigate the code with rust-analyzer as you read: every field and method named
here (`self.phi`, `self.psi`, [`TampinesSteamArray::correct_thermo`],
[`TampinesSteamArray::step`], …) is real and cited exactly.

---

## 1. The governing equations (what we are actually solving)

Treat the pipe as a 1-D continuum. Three conservation laws close the flow of a
single (possibly two-phase, but locally *homogeneous*) fluid. In the units the
code carries:

**Continuity** (mass) — density `ρ` \[kg/m³\], velocity `U` \[m/s\]:

> `∂ρ/∂t + ∇·(ρU) = 0`

"The rate a cell's density rises equals minus the net mass flux leaving it."
In the code the mass flux `ρU·Sf` is stored *directly* as the surface field
`self.phi` \[kg/s\] (`Sf` = face-area vector \[m²\]), so continuity reads
`∂ρ/∂t + ∇·φ = 0`, discretised explicitly as `ρ = ρ_old − dt·∇·φ` — the
`rhoEqn` block at the top of [`TampinesSteamArray::step`].

**Momentum** — pressure `p` \[Pa\], viscosity `μ` \[Pa·s\]:

> `∂(ρU)/∂t + ∇·(ρUU) = −∇p + ∇·(μ∇U)`

Newton's second law per unit volume: inertia (unsteady + advection of
momentum `ρUU`) is driven by the pressure gradient plus viscous diffusion.
In the code the advective term is `∇·(φU)` (reusing the same `self.phi`), so
the discrete operator is `ddt_coeff_vec(ρ,U) + div_vec(φ,U) + laplacian_vec(μ,U)`
— the `UEqn` block. The `−∇p` term is kept **explicit** (added to the source
as `−V·∇p`), which is the whole point of the algorithm below.

**Energy, enthalpy form** — static specific enthalpy `he` \[J/kg\]:

> `∂(ρh)/∂t + ∇·(ρUh) = dp/dt`   (adiabatic, inviscid-work-neglected pipe)

Why enthalpy `h` and not internal energy `e` or temperature `T`? Because for a
flashing fluid `h` is the variable that stays *continuous and monotone* across
the saturation dome (temperature plateaus at `T_sat` while the fluid boils, so
`T` is a terrible primary variable there), and because the pressure-work term
collapses to the clean source `dp/dt` (`enthalpy = internal energy + p·v`, and
the `p·v` bookkeeping cancels the flow-work term, leaving only the local
`∂p/∂t`). In the code this is `∇·(φh)` for the convection, `dp_dt =
(p − p_old)/dt` for the source, plus a small conduction term `∇·(αh∇h)` with
the OpenFOAM effective diffusivity `αh = κ/Cp` \[kg/(m·s)\] — the `EEqn` block.

These three are not independent: `ρ`, `T`, `μ`, `αh`, and the compressibility
`ψ` (below) are all functions of `(p, h)` supplied by the steam tables in
[`TampinesSteamArray::correct_thermo`]. That EOS coupling is what makes the
system compressible, and it is where all the difficulty lives.

---

## 2. Why *pressure-based* (`rhoPimpleFoam`), not density-based

A density-based compressible solver (think `rhoCentralFoam`) treats
`[ρ, ρU, ρE]` as the unknowns and marches them explicitly: compute fluxes,
update the conserved variables, then back out `p` and `T` from the EOS. Simple
and robust for *supersonic* shocks — but it is shackled by the **acoustic CFL
limit**. An explicit scheme can only advance information one cell per step, so
the timestep must resolve the fastest wave in the system: the *sound* wave,
`dt ≲ Δx / (|U| + c)`. In subcooled liquid water `c ≈ 1400 m/s` while the bulk
velocity might be `1 m/s` — so you pay for resolving acoustics you do not care
about. This is the **low-Mach stiffness** problem: `Ma = |U|/c ≪ 1` means the
acoustics are ~1000× faster than the flow, and an explicit density-based
method crawls.

The pressure-based cure: derive an **implicit equation for pressure** (below).
An implicit solve couples the whole domain in one linear system, so acoustic
information crosses many cells per step and the timestep is limited by the
*convective* CFL `dt ≲ Δx/|U|`, not the acoustic one. You trade an explicit
flux update for a linear solve (`solve_cg` on the pressure matrix) and buy back
orders of magnitude in `dt` for low-Mach flow. That is exactly the regime an
FHR secondary loop or an Edwards blowdown lives in for most of its length.

---

## 3. The PIMPLE algorithm — one timestep, walked through

PIMPLE = **PISO** (Pressure-Implicit Split-Operator, the transient
pressure–velocity corrector loop) nested inside **SIMPLE** (Semi-Implicit
Method for Pressure-Linked Equations, which adds outer iterations and
under-relaxation). The structure is two nested loops:

- **outer correctors** (`n_outer_correctors`) — SIMPLE-style; re-linearise the
  whole coupled system. `= 1` gives pure transient PISO
  ([`TampinesSteamArray::set_piso_algorithm`]); `> 1` with under-relaxation
  gives PIMPLE, letting `dt` exceed the strict PISO limit.
- **inner correctors** (`n_inner_correctors`) — PISO; re-solve pressure and
  re-project velocity *at fixed coefficients* to mop up the velocity–pressure
  split error.

### 3a. Momentum predictor

Assemble the momentum matrix `u_eqn = ddt + div + laplacian` and split it into
its diagonal `A` \[kg/s\] and off-diagonal-plus-source operator `H(U)`. The
matrix row for cell *c* reads `A·U_c − H(U) = −V·∇p`. "Predict" a velocity by
solving this with the *old* pressure gradient (`u_eqn.solve("U", …)`). This
`u_pred` satisfies momentum but **not** continuity — it is divergence-dirty.
The code caches `rAU = V/A` \[m³·s/kg\] (the inverse diagonal) for the
projection that follows.

### 3b. The pressure equation — where compressibility enters

This is the heart of the method; derive it. Write the momentum row solved for
velocity, splitting the pressure term back out:

> `U = H(U)/A − (1/A)·∇p = HbyA − rAU·∇p`

`HbyA = H(U)/A` \[m/s\] is the "velocity without its own pressure gradient".
Take the mass flux of this and of the pressure-projection piece:

> `φ = ρ_f·(HbyA·Sf) − ρ_f·rAU_f·∇p·Sf  =  φ_HbyA − ρ_f·rAU_f·snGrad(p)·|Sf|`

(`_f` = face-interpolated; `snGrad` = surface-normal gradient.) Now demand that
this `φ` satisfy **continuity**. For an *incompressible* flow you would demand
`∇·φ = 0`, giving a pure Poisson equation `∇·(ρ_f·rAU_f·∇p) = ∇·φ_HbyA`. But
this fluid is compressible: continuity is `∂ρ/∂t + ∇·φ = 0`, and `ρ` itself
depends on `p`. Linearise that dependence with the **compressibility**

> `ψ = ∂ρ/∂p`   \[s²/m² = kg/(m³·Pa)\]   →   `∂ρ/∂t ≈ ψ·∂p/∂t ≈ ψ·(p − p_old)/dt`.

Substituting turns continuity into an implicit, well-posed pressure equation.
In the code (`pEqn` block) the assembled system is

> `[ laplacian(ρ_f·rAU_f) + ψ·V/dt ]·p = ψ·V/dt·p_old − (net φ_HbyA outflow)`

The `ψ·V/dt` term added to `p_eqn.ldu.diag[c]` is the star of the show. It is
the transient-compressible diagonal. Two things it buys:

1. **Non-singularity.** A pure incompressible pressure-Poisson matrix is
   singular (pressure defined only up to a constant; needs a reference cell).
   The `ψ·V/dt` diagonal makes the matrix SPD with no null space — no reference
   cell needed — so `solve_cg` (PCG) converges directly.
2. **Physics.** It encodes "if you compress this cell, its density rises by
   `ψ·Δp`, which continuity must account for." A stiff (nearly incompressible)
   liquid has tiny `ψ` → the term vanishes → you recover the incompressible
   limit. A compliant vapour or a *flashing* two-phase cell has large `ψ` →
   the term dominates → pressure changes are absorbed by density change instead
   of by acoustic velocity adjustment.

### 3c. Correct, then repeat

With the new `p`, correct the flux `φ ← φ_HbyA − ρ_f·rAU_f·snGrad(p)·|Sf|`
(now divergence-consistent) and the velocity `U ← HbyA − rAU·∇p` (now
continuity-satisfying). Re-close the EOS via
[`TampinesSteamArray::correct_thermo`] and loop the inner corrector. After the
inner loop, solve the energy equation, and (if outer correctors remain)
re-linearise. Optional explicit under-relaxation `p ← p_prev + α_p·(p − p_prev)`
(`p_under_relaxation`, `u_under_relaxation`) stabilises the SIMPLE outer
iterations; at `α = 1` (the PISO default) it is a no-op.

---

## 4. The HEM closure — what makes this *HEM-closed* `rhoPimpleFoam`

Stock `rhoPimpleFoam` closes the EOS with a perfect gas: `ρ = p/(RT)`,
`ψ = ∂ρ/∂p = 1/(RT)`, a constant-ish scalar. Here the EOS is a **real
IAPWS-IF97 `(p, h)` equilibrium flash** ([`TampinesSteamArray::correct_thermo`]),
and the fluid can be subcooled liquid, superheated vapour, *or* a two-phase
mixture. "Homogeneous equilibrium" (HEM) means the two phases share one
velocity, one pressure, and one temperature, always at thermodynamic
equilibrium — so a single `(p, h)` flash returns the mixture `ρ`, `T`, quality
`x`, etc. That is the cheapest self-consistent two-phase closure, and it is the
right first model for fast flashing (Edwards blowdown, choked break flow).

**The subtle part is which compressibility `ψ` to use.** Recall step 3b froze
everything except pressure when we wrote `∂ρ/∂t ≈ ψ·∂p/∂t`. In this segregated
algorithm, during the pressure solve the enthalpy `he` is held fixed (it is
only updated later, by the energy equation). So the density's response to
pressure that the pressure equation actually sees is the **constant-enthalpy**
derivative

> `ψ = ∂ρ/∂p|_h`   — stored in `self.psi`, computed by a central finite
> difference of the `(p,h)` flash in `correct_thermo` (`(rho_hi − rho_lo)/(p_hi − p_lo)`).

Not the isothermal `∂ρ/∂p|_T = ρ·κ_T`. In single phase the two nearly agree
(for an ideal gas `∂ρ/∂p|_h = ρ/p = ρ·κ_T` exactly; for liquid `∂ρ/∂h|_p` is
tiny so `|_h ≈ |_T`), so subcooled/superheated behaviour is unchanged.
**Inside the two-phase dome they differ by ~100×.** The isothermal value
`κ_T = x·κ_vap + (1−x)·κ_liq` freezes the quality and misses the *flashing
term* `(v_g − v_f)·dx/dp`: as pressure drops, the equilibrium quality `x`
rises (liquid flashes to vapour), and that phase change is a huge volumetric
response. Only `∂ρ/∂p|_h` captures it, because the `(p,h)` flash re-solves the
equilibrium quality at each pressure. That flashing compliance is exactly what
pins a boiling cell on the saturation line `p = p_sat(T)` as it depressurises —
the **Edwards flashing plateau**. Use the frozen `κ_T` and the `ψ·V/dt`
diagonal is ~100× too small, so the pressure sails straight through the plateau
(see the long comment in `correct_thermo`).

---

## 5. The conservative energy time-derivative — the plateau, part two

Getting `ψ` right is necessary but not sufficient. The energy equation's
*time derivative* must be discretised conservatively or the enthalpy field
drifts. Write the enthalpy convection as `∇·(φh)`. The unsteady term must be
the **conservative** form `∂(ρh)/∂t`, discretised as
`(ρ_cont·h − ρ_old·h_old)/dt`, and the density multiplying the *new* time level
must be the **continuity density**

> `ρ_cont = ρ_old − dt·∇·φ`

recomputed from the *final* mass flux `self.phi` — **not** the EOS density
`self.rho` that `correct_thermo` wrote. This is the whole reason
[`fvm::ddt_coeff_old`] exists (it takes distinct new/old density fields). Here
is why it matters. Discrete continuity gives `(ρ_cont − ρ_old)/dt = −∇·φ`
*exactly*. Expand the conservative time term and add the convection:

> `(ρ_cont·h − ρ_old·h_old)/dt + ∇·(φh)`

The `h_old·(ρ_cont − ρ_old)/dt = −h_old·∇·φ` piece cancels the `h·∇·φ` part of
`∇·(φh)` term-for-term, and the equation collapses to the **material
derivative** `ρ Dh/Dt = dp/dt`, i.e. the reversible `dh ≈ dp/ρ`. That tiny
reversible enthalpy change is what keeps the state *on the saturation dome* as
`p` falls — the plateau.

Break the cancellation and the plateau dies:

- Reuse the *current* density for both time levels (the naive `ddt_coeff`) and
  you are really solving `ρ·∂h/∂t + ∇·(φh) = dp/dt`, whose un-cancelled
  `h·∇·φ` outflow **over-drains enthalpy** during the violent flash (`∇·φ ≫ 0`
  at the break). The bulk liquid is driven subcooled and the pressure collapses
  straight past `p_sat` — the pre-fix subcooling plateau bug (bead op-21g.14).
- Use the *EOS* density for `ρ_cont` and, mid-flash, it drops faster than the
  `ψ·dp/dt` the pressure equation feeds back into `φ`, leaving a residual that
  spuriously **over-heats** cells (a `(p,h)` flash into Region 5).

Only the continuity density closes the loop. See the fully commented `EEqn`
block in [`TampinesSteamArray::step`].

---

## 6. The choked break boundary condition

A pipe rupture discharges to a much lower back-pressure. Once the flow at the
break reaches the local sound speed it **chokes**: the throat velocity is
pinned at `u_throat = a_HEM` (the HEM critical speed) and further lowering the
downstream pressure cannot raise the mass flux. So the outlet BC is not a
fixed pressure — it is a *critical-flow* condition. The crate already solves
HEM critical flow: [`get_critical_pressure_and_mass_flux_multiphase_ph`]
(`crate::steam_turbine_equations::…::choked_flow`) takes the local stagnation
`(p0, h0)` at the break cell and returns `(p_crit, G_crit)` — the choke
pressure and critical HEM mass flux — dispatching by `(p0,h0)` region to the
in-dome / subcooled / superheated-vapour solvers. The blowdown driver converts
`G_crit` to an equivalent full-face velocity and imposes it via
[`TampinesSteamArray::set_outlet_velocity`] each step (the same critical-flow
machinery `TampinesSteamTableCV::get_crit_pressure_and_massflux` wraps).

---

## 7. ~~The all-Mach hybrid (`SolverMode::HybridAllMach`)~~ RETIRED 2026-10-16

Until 2026-10-16 this array had an opt-in second mode, `HybridAllMach`:
PIMPLE plus a Mach-blended KNP (Kurganov–Noelle–Petrova central-upwind)
deferred-correction dissipation `β(Ma)·(knp − central)·|Sf|`, folded into
`phi` and the momentum source, with `β = clamp((Ma − 0.3)/(1.0 − 0.3), 0, 1)`
on `min(Ma_owner, Ma_neighbour)`, the HEM equilibrium sound speed, and a
rarefied-tail density taper (50–100 kg/m³). It was built to damp the
ringing of the pressure-based path at a near-sonic flashing front, and
before gh:#900 it did (~55 % less excess total variation on Edwards
blowdown, 0–0.15 s).

**Why it was removed.** gh:#900 made this array's defaults
upstream-faithful (`PsiRefresh::OncePerOuterCorrector`, implicit energy
convection, the kinetic-energy terms). After that, plain PIMPLE rings 3.4×
less, and the hybrid **added** ringing instead of damping it: −23.5 %
(`Linear`), −31.0 % (van Leer), −36.7 % (upwind) on its own Edwards gate
(gh:#914). It failed its own acceptance test, so the maintainer retired it
(decision D7, "retire the KNP hybrid", 2026-10-16;
`crates/tampines/docs/hem-knp-stabilisation/README.md`). The density-based
KNP path now lives in [`crate::hem::HemPipe`], built on the shared kernel
`outram_foam_basic_lib::fv_operators::fvc::central_upwind`. The full
derivation of the retired hybrid is in git history before this change
(e.g. commit `42a05c598`, this file and `central_upwind.rs`).

---

## Where to read next

- [`TampinesSteamArray::step`] — the timestep loop; the block comments there
  annotate every equation cited above.
- [`TampinesSteamArray::correct_thermo`] — the `(p,h)` EOS closure and the
  `ψ = ∂ρ/∂p|_h` finite difference.
- ~~`central_upwind` — the KNP central-upwind flux and HEM sound speed.~~
  Removed with the hybrid, 2026-10-16. The HEM sound speed is the public
  [`crate::interfaces::functional_programming::ph_flash_eqm::w_ph_eqm_hem`].
- Stability failure modes (BC well-posedness, pressure-source clobbering,
  water-hammer, pressure bounding) are walked through in the appbuilder crate's
  `docs/stability_a_students_guide.md`, which applies here verbatim.

C++ reference (reproduced verbatim below for provenance):
`applications/solvers/compressible/rhoPimpleFoam/`.

```rust
pub mod rhoPimpleFoam { /* ... */ }
```

### Types

#### Enum `PsiRefresh`

How often `psi = d(rho)/dp|_h` is rebuilt inside the PIMPLE loop.

`psi` is the compressibility that forms the pressure equation's diagonal.
It is a **linearisation coefficient**, not a state variable: PIMPLE iterates
to a fixed point, so `psi` affects the path taken to convergence, not the
converged answer -- provided it still converges.

**Default changed 2026-10-16 (gh:#900, maintainer decision D5 of epic
#896):** ~~`EveryCorrector` is the default~~ → [`Self::OncePerOuterCorrector`]
is the default, because it is the closer match to upstream (read below).
`EveryCorrector` is kept as the explicit ablation of the pre-2026-10-16
path.

```rust
pub enum PsiRefresh {
    EveryCorrector,
    OncePerOuterCorrector,
}
```

##### Variants

###### `EveryCorrector`

Rebuild `psi` on every corrector, inner and outer. The historical path
(the default until 2026-10-16), bit-identical to what this solver did
before the option existed. Kept as an **ablation** of the default.

###### `OncePerOuterCorrector`

Rebuild `psi` once per **outer** corrector and hold it across the inner
PISO correctors.

# Why this is defensible

Within a pressure-correction inner iteration `he` is **frozen** -- it is
only updated by the energy equation after the inner loop, which this
module documents at [`TampinesSteamArray::correct_thermo`]. So across
the inner correctors `psi = d(rho)/dp|_h` moves only through `p`, and it
is being rebuilt from scratch each time to track a second-order effect
on a coefficient that is already an approximation.

# Why it is worth doing

`psi` is the single most expensive quantity in `correct_thermo`. It is
computed by a central finite difference, so it costs **two full `(p,h)`
flashes per cell**, against five for everything else combined
(`t`, `v`, `mu`, `lambda`, `cp`). At the Edwards settings of 4 outer and
4 inner correctors, holding it across the inner loop removes 6 of every
8 psi flashes -- about 21 % of all thermodynamic work in the solver.

# What is NOT claimed

# MEASURED: no wall-clock saving, but a real accuracy gain

Edwards blowdown, 0-50 ms, 24 cells, dt = 30 us (2026-09-14):

| refresh | wall | flashing plateau | GS-1 RMSE | break peak |
|---|---|---|---|---|
| `EveryCorrector` | 24.0 s | 392.4 psia | 109.0 psia | 125.5 lbm/s |
| `OncePerOuterCorrector` | 29.3 s | 360.7 psia | 80.6 psia | 98.5 lbm/s |

**Slower, despite doing ~21 %% fewer flashes.** Holding `psi` changes the
path the PIMPLE loop takes, so the pressure solves converge differently
and the saved thermodynamic work is more than repaid in linear-solver
iterations. The two runs are not the same trajectory, so the timings are
not a like-for-like comparison of the same work.

**But markedly more accurate.** The flashing plateau moves from 392.4 to
360.7 psia, squarely into the experimental 350-367 psia band, and GS-1
RMSE falls 26 %%. That is a larger effect than a linearisation
coefficient has any business having if the loop were fully converged,
which suggests it is **not** converged at 4 outer x 4 inner correctors,
and that rebuilding `psi` against a just-updated `p` while `he` is held
fixed is inconsistent with the PISO splitting rather than merely
wasteful. Not chased here; recorded so the next person starts from the
measurement.

~~Left default-off regardless: it changes the validated trajectory, and
that is the maintainer's call, not an agent's.~~ **CORRECTED
2026-10-16** -- the maintainer made that call (D5, gh:#900): this is now
the default. The Edwards numbers it moved are recorded in
`tests/edwards_blowdown.rs` and `docs/edwards_post_petir_ablation_log.md`.

~~OpenFOAM's own `rhoPimpleFoam` updates `psi` through `thermo.correct()`,
and whether that sits in its outer or inner loop was **not verified** --
the OpenFOAM sources are not vendored in this workspace~~ **CORRECTED
2026-10-16** -- verified against OpenFOAM-10 commit `89f925dc`
(`applications/solvers/compressible/rhoPimpleFoam/`, sparse-cloned for
epic #896): `thermo.correct()` is called **once per outer corrector**,
at the end of `EEqn.H`, and nowhere in `pEqn.H`. Inside the pressure
corrector upstream only linearises the density,
`thermo.correctRho(psi*p - psip0)`, holding `psi` fixed. So holding
`psi` across the inner correctors is upstream behaviour. **One
difference remains:** this port still re-flashes the full `(p,h)`
density on every inner corrector rather than applying upstream's
linear `correctRho` update; only `psi` is held.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Enum `EnergyConvectionScheme`

Face-interpolation scheme for the **energy equation's** convection term
`∇·(φh)` (see [`TampinesSteamArray::he_convection_scheme`]) -- OpenFOAM's
`div(phi,h)` entry in `fvSchemes`.

Enum dispatch (no trait objects) over the flux limiter `λ(r)` applied to the
upwind-biased face reconstruction: `λ = 0` is first-order upwind, `λ = 1`
unlimited central differencing, and the TVD variants pick `λ(r)` from the
local slope ratio so that no new extremum is created.

**Ported 2026-09-29 (gh:#319, #343)** from the sibling port's
`outram_park_fork_coolprop::openfoam_algorithms::rhoPimpleFoam::EnergyConvectionScheme`,
which gained it on 2026-08-12 (bead `op-1fyp`). Until this date the steam
array had no such setting: it convected enthalpy with an explicit
`fvc::div` whose face value is a plain **linear** interpolation, i.e.
`Gauss linear` -- which no upstream `rhoPimpleFoam` tutorial uses for
`div(phi,h)` / `div(phi,e)` (they use `limitedLinear 1`, `linearUpwind`,
`LUST` or `upwind`; ~~checked in the vendored
`crates/outram-foam-basic-lib/upstream_source/openfoam/tutorials/compressible/rhoPimpleFoam/`~~
**CORRECTED 2026-10-16:** that path does not exist -- `upstream_source/`
holds only a README. Re-checked against OpenFOAM-10 `89f925dc` for three
tutorials: `RAS/angledDuct` uses `bounded Gauss upwind` for `div(phi,e)`,
`LES/pitzDaily` `Gauss LUST grad(e)`, `laminar/shockTube`
`Gauss limitedLinear 1`; the other tutorials were not re-checked).

Orthogonal to [`EnergyBalanceMode`], which decides whether the convection
term sits in the matrix (implicit, the default since 2026-10-16) or in the
source vector (explicit). Under the implicit mode the limiter chosen here is
carried by a deferred correction.

# Why the default changed, and what it fixed

On a coarse steam-generator tube the cell Péclet number is enormous, and
central differencing is then unbounded. At steady state a cell's balance
reads `m_dot (h_in,face - h_out,face) + Q = 0` with
`h_out,face = (h_0 + h_1)/2`, so the inlet cell settles at
`h_0 = 2 h_in - h_1 + 2 Q_0 / m_dot`: **whenever the next cell boils while
the inlet cell is only weakly heated, `h_0` is driven below the feedwater
enthalpy, without bound**. Measured 2026-09-29 on the HTR-10 tube (8 cells,
34 m, 4 MPa, 313.15 K feedwater at 3 kg/s, inlet-cell wall at the feedwater
temperature, 700 K downstream): the inlet cell fell 100 kJ/kg below the
feedwater at 10.74 s and was still falling at 1.5 kJ/kg per 0.125 s -- the
walk to the 273.15 K IF97 floor behind gh:#319. With the van Leer default it
never undercuts the feedwater
(`tests/heated_tube_never_undercuts_its_feedwater.rs`).

```rust
pub enum EnergyConvectionScheme {
    VanLeer,
    Minmod,
    Upwind,
    Linear,
}
```

##### Variants

###### `VanLeer`

**van Leer TVD limiter**, `λ(r) = (r + |r|)/(1 + |r|)` -- the default.
Second order where the enthalpy field is smooth, falling back toward
upwind at a front so the field stays bounded by its own initial and
boundary data. OpenFOAM equivalent: `div(phi,h) Gauss vanLeer`.

###### `Minmod`

**minmod TVD limiter**, `λ(r) = max(0, min(r, 1))` -- the most diffusive
TVD limiter, and the most robust.

###### `Upwind`

**First-order upwind**, `λ ≡ 0`. Bounded (at CFL ≤ 1), strongly diffusive.

###### `Linear`

**Unlimited central differencing**, `λ ≡ 1` -- second order and
**unbounded**. The scheme this array used until 2026-09-29, kept only as
an explicit ablation so a historical study can be re-run: it calls the
historical operator `fvc::div` itself, so it is **bit-for-bit** the
pre-2026-09-29 behaviour. **Do not select this for new work** -- see the
enum doc for the undershoot it produces.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Enum `EnergyBalanceMode`

Time/matrix treatment of the **energy equation's** convection term `∇·(φh)`
(see [`TampinesSteamArray::he_balance_mode`]).

**Ported 2026-10-16 (gh:#900, epic #896 A2)** -- reused, not rewritten, from
the sibling port's
`outram_park_fork_coolprop::openfoam_algorithms::rhoPimpleFoam::EnergyBalanceMode`
(bead `op-j2oq`, 2026-08-13), whose doc carries the measured stability /
accuracy trade on a nitrogen pipe (explicit fails at `Co = 1.25`; implicit
holds to `Co = 2` at +1.19 % of span). Same two variants, same deferred
correction, same operators.

It is **orthogonal to [`EnergyConvectionScheme`]**, which picks the flux
limiter: any limiter pairs with either mode.

## Why implicit is the default here (maintainer decision D5, 2026-10-16)

Upstream `rhoPimpleFoam`'s `EEqn.H` (OpenFOAM-10 `89f925dc`) assembles
`fvm::div(phi, he)` -- the convection is **implicit**. The explicit form
this array used until 2026-10-16 stops being a convex combination of
neighbouring enthalpies as soon as a cell's outflow over one step exceeds
its content, and on the Edwards break cell that is how a `-1.7e10 J/kg`
enthalpy was produced (diagnosis H2 in
`crates/tampines/docs/hem-knp-stabilisation/01-instability-diagnosis.md`).
With implicit upwind convection and the conservative time derivative of
[`fvm::ddt_coeff_old`], a cell's diagonal is
`ρ_cont·V/dt + F_out = ρ_old·V/dt + F_in > 0`, positive **even when the
continuity density `ρ_cont` is negative**.

The measured Edwards movement caused by this default is recorded in
`tests/edwards_blowdown.rs`.

```rust
pub enum EnergyBalanceMode {
    Implicit,
    Explicit,
}
```

##### Variants

###### `Implicit`

**Implicit upwind convection plus deferred correction** -- the default.
First-order-upwind `∇·(φh)` is assembled into the matrix (`fvm::div`),
and the difference `(scheme − upwind)` between the selected
[`EnergyConvectionScheme`]'s divergence and the upwind one is carried
as an explicit deferred-correction source. With
[`EnergyConvectionScheme::Upwind`] that correction is identically zero
(`fvm_div_matches_explicit_upwind_divergence`). Upstream `fvm::div`.

###### `Explicit`

**Explicit convection** -- the whole `∇·(φh)` enters the source vector.
The path this array used until 2026-10-16, bit-for-bit; kept as an
**ablation**. Conditionally stable: it needs a cell outflow Courant
number below 1, which no number of outer correctors lifts.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Enum `KineticEnergyTerms`

Whether the energy equation carries upstream's **kinetic-energy terms**
`fvc::ddt(rho, K) + fvc::div(phi, K)` with `K = ½|U|²` \[J/kg\]
(OpenFOAM-10 `rhoPimpleFoam/EEqn.H`).

**Added 2026-10-16 (gh:#900).** Until then the port omitted both terms
without a recorded reason (diagnosis O4 in
`crates/tampines/docs/hem-knp-stabilisation/01-instability-diagnosis.md`).
With `he` the static enthalpy, the total-energy balance needs them: they
move energy between the static enthalpy and the kinetic energy of the flow.
The expected size on Edwards is small, `½u² ≈ 0.1–5 kJ/kg` at 16–100 m/s.

Discretisation: `ddt` uses the same continuity density `ρ_cont` and
old-time density `ρ_old` as the enthalpy's conservative time derivative
(upstream's `rho` at that point is likewise the `rhoEqn` density), and
`div(φ,K)` uses the same face scheme as `div(φ,h)`
([`TampinesSteamArray::he_convection_scheme`]), as upstream's
`laminar/shockTube` tutorial does (`limitedLinear 1` for both).
The boundary `K` is computed from the velocity's boundary values.

Correct physics is the default; [`Self::Excluded`] is the ablation.

```rust
pub enum KineticEnergyTerms {
    Included,
    Excluded,
}
```

##### Variants

###### `Included`

Include `fvc::ddt(ρ,K) + fvc::div(φ,K)` as explicit sources, as
upstream does. The default.

###### `Excluded`

Omit them -- the pre-2026-10-16 behaviour, kept as an **ablation**.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Enum `TimeStepControl`

How [`TampinesSteamArray`] chooses its time step.

**Added 2026-10-16 (gh:#900, epic #896 A1).** Before this, the array used
a fixed Δt, computed no Courant number anywhere, and drained cells silently
past empty (the diagnosis's observed `ρ_old − dt·∇·φ = −5.47 kg/m³` from a
cell holding 1.36 kg/m³).

## Upstream (OpenFOAM-10 `89f925dc`)

`compressibleCourantNo.H`:
`CoNum = 0.5*gMax(fvc::surfaceSum(mag(phi))/rho/V)*deltaT` -- the mass-flux
form of the convective Courant number. `setDeltaT.H`, with
`adjustTimeStep yes` (upstream default **no**, `readTimeControls.H`):

```cpp
scalar deltaT = maxCo*runTime.deltaTValue()/(CoNum + small);
deltaT = min(deltaT, fvModels.maxDeltaT());
deltaT = min(min(deltaT, runTime.deltaTValue() + 0.1*deltaT),
             1.2*runTime.deltaTValue());
runTime.setDeltaT(min(deltaT, maxDeltaT));
```

Reduction is immediate; growth is damped. On the first step
`setInitialDeltaT.H` additionally applies
`min(maxCo*deltaT/CoNum, min(deltaT, maxDeltaT))` if `CoNum > small`. The
Courant number is evaluated from the flux and density **at the start of
the step**, as upstream does.

## Why the fixed mode is guarded

A cell's **outflow Courant number** `Co_out = dt·Σφ_out/(ρ_old·V)` is the
fraction of the mass it held at the start of the step that the step's
flux exports. Above 1 the explicit continuity update
`ρ = ρ_old − dt·∇·φ` can be asked for more mass than the cell holds, which
is what the drained-cell hold used to paper over. Under [`Self::Fixed`]
that is now an error rather than a hold.

```rust
pub enum TimeStepControl {
    Fixed,
    FixedUnguarded,
    AdaptiveCourant {
        max_courant: uom::si::f64::Ratio,
        max_delta_t: uom::si::f64::Time,
    },
}
```

##### Variants

###### `Fixed`

The caller's Δt ([`TampinesSteamArray::delta_t`]) is used as given.
If any cell's outflow Courant number exceeds 1, [`TampinesSteamArray::step`]
returns [`TampinesSteamArrayError::CourantViolation`] and leaves the
array in its start-of-step state, so the caller can retry with a smaller
step. The default, so callers that drive Δt themselves keep a fixed Δt.

###### `FixedUnguarded`

Fixed Δt with **no** Courant guard: a cell whose continuity density
falls to the floor gets the counted drained-cell enthalpy hold
([`TampinesSteamArray::drained_hold_events`]) instead of an error. The
pre-2026-10-16 behaviour, kept only as an **ablation**.

###### `AdaptiveCourant`

Upstream `adjustTimeStep yes`: before every step Δt is reset from the
convective Courant number by `setDeltaT.H`'s rule (see the enum doc).
The step actually taken is returned by [`TampinesSteamArray::step`] and
stored in [`TampinesSteamArray::delta_t`].

No error is raised for an outflow Courant number above 1 here, matching
upstream, which does not check it; a step that still over-drains a cell
gets the counted drained-cell hold.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `max_courant` | `uom::si::f64::Ratio` | `maxCo` -- the target convective Courant number (dimensionless,<br>> 0). Upstream's default when unspecified is 1. |
| `max_delta_t` | `uom::si::f64::Time` | `maxDeltaT` -- the largest step the controller may take \[s\]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Enum `ThermoClosure`

Which thermodynamic closure `correct_thermo` uses to obtain the cell state.

Exists so the two can be **measured against each other** on the same case;
[`ThermoClosure::PressureEnthalpy`] is the validated default and the one the
Edwards V&V numbers were produced with.

```rust
pub enum ThermoClosure {
    PressureEnthalpy,
    DensityEnthalpyOnMassFlow,
    DensityEnthalpyOnMassInflow,
    DensityEnthalpyEverywhere,
}
```

##### Variants

###### `PressureEnthalpy`

Pressure-based: take `p` from the pressure equation and `h` from the
energy equation, and derive density as `rho = 1/v(p,h)`.

This is what a segregated PIMPLE algorithm naturally wants, since `p` is
already a primary variable. One explicit `(p,h)` flash per property.

###### `DensityEnthalpyOnMassFlow`

Density-based **in cells whose mass is changing**, either way.

This is the closure that makes physical sense. Mass crossing a control
volume boundary carries enthalpy with it, and it changes the cell's
density -- so density and enthalpy are both genuinely new information
and the pressure that goes with them is what has to be recovered. That
argument does not care about the sign: a draining cell has just as
surely had its density changed as a filling one.

A cell with no net mass flux has neither, and its pressure is already
the pressure equation's answer -- re-deriving it there buys nothing and
costs a root find.

The test is the cell's own mass balance, `div(phi) != 0`, taken from the
same face flux the continuity density is built from, so the criterion
and the physics cannot drift apart. "Non-zero" is judged against the
field's own scale rather than against zero, so floating-point dust in a
quiescent cell does not trigger the expensive path.

###### `DensityEnthalpyOnMassInflow`

Density-based **only in cells that are gaining mass** (`div(phi) < 0`).

The narrower half of the criterion above, and on the Edwards blowdown it
is the better trade. Measured over the 0-50 ms window (24 cells,
dt = 30 us, 2026-09-14):

| closure | wall | flashing plateau | GS-1 RMSE vs data |
|---|---|---|---|
| `PressureEnthalpy` | 23.9 s | 392.4 psia | 109.0 psia |
| `DensityEnthalpyOnMassInflow` | 67.7 s | 376.7 psia | 96.2 psia |
| `DensityEnthalpyOnMassFlow` | 149.3 s | 368.6 psia | 93.2 psia |
| `DensityEnthalpyEverywhere` | 156.0 s | 367.4 psia | 93.2 psia |

The experimental plateau is roughly 350-367 psia, so every density-based
variant moves toward the data and the pressure-based one is the outlier.
But in a blowdown almost every cell is flowing, so
`DensityEnthalpyOnMassFlow` selects nearly all of them and lands within
noise of doing it everywhere -- 2.2x the cost of inflow-only to buy a
further 3 psia of RMSE. Restricting it to cells actually **taking in**
mass keeps about nine tenths of the accuracy gain for under half the
cost, which is why both variants are kept rather than one.

###### `DensityEnthalpyEverywhere`

Density-based in **every** cell, every corrector iteration.

Kept only as the measurement baseline that shows why the selective
variant above exists. Recovering a pressure the algorithm already holds
cannot be cheaper than not recovering it -- the inversion is a bracketed
root find built out of the same `(p,h)` flashes it replaces -- so this
is expected to be slower everywhere it is not needed. Do not reach for
it in a solver.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Enum `AdvectionTerminalState`

The thermodynamic state available at one **advection terminal** (one end
patch) of the pipe, used to pick the upwind state for the energy equation.

## Why a terminal and not a patch

[`TampinesSteamArray`] is a **1-D flow component in a network**, so its two
ends are **junctions** to other components, not patches on a standalone CFD
domain. The convention this enum serves is therefore TUAS's upwind advection
terminal, not OpenFOAM's `zeroGradient` / `fixedValue` patch semantics — see
`docs/boundary-conditions-convention.md` and
[`TampinesSteamArray::correct_advection_terminals`].

Ported from `tuas_boussinesq_solver`'s
`single_control_vol/boundary_condition_interactions/advection_to_bcs.rs`,
whose two pairs of methods map onto the two variants below one-for-one:

| TUAS method pair | Variant here |
|---|---|
| `calculate_*_advection_non_set_temperature` (BC state = zero-gradient extrapolation of the control volume) | [`Self::ZeroGradientExtrapolated`] |
| `calculate_*_advection_set_temperature` (BC state prescribed by the caller) | [`Self::Junction`] |

In **both** TUAS pairs the enthalpy *and* the density actually used are
selected by the sign of the mass flow, never defaulted — which is what makes
the historical "zero-gradient at both ends" failure structurally impossible.

```rust
pub enum AdvectionTerminalState {
    ZeroGradientExtrapolated,
    Junction(uom::si::f64::AvailableEnergy),
}
```

##### Variants

###### `ZeroGradientExtrapolated`

No junction state is prescribed at this terminal, so the only upstream
information available on **inflow** is the adjacent cell itself: the
upwind enthalpy degenerates to the zero-gradient extrapolation
`h_face = h_cell`, and the upwind density to the cell density.

This is TUAS's `*_non_set_temperature` path, and it is the default —
an array nobody has connected to anything advects in its own state,
which is the honest answer when there is no other.

###### `Junction`

A junction specific enthalpy \[J/kg\] supplied by whatever component is
connected at this terminal (an upstream pipe, a plenum, a reservoir).

Used as the upstream state **only while the flow at this terminal is
into the domain**; on outflow the control-volume state is upwind and
this value is ignored. TUAS's `*_set_temperature` path.

Valid range is whatever the IAPWS-IF97 `(p, h)` flash accepts at the
terminal's pressure; a junction enthalpy that flashes out of range is
ignored for the density selection rather than propagating a NaN.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::AvailableEnergy` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `TampinesSteamArray`

One-dimensional compressible PIMPLE pipe array driven by the TAMPINES steam
tables.

This is the tampines-steam-tables analogue of `outram-foam-appbuilder-lib`'s
`RhoPimpleFoam`, specialised to a **1-D pipe**: the mesh is built
automatically from a length, a cross-sectional area, and a cell count via
[`create_one_d_mesh`], instead of being read from an OpenFOAM `polyMesh`
directory. It is intended as the transient-flow backbone for coupling the
IAPWS-IF97 steam properties into a system-code-style pipe network.

It solves the same compressible PIMPLE system as `RhoPimpleFoam`:
```text
  ∂ρ/∂t   + ∇·(ρU)    = 0            (continuity, explicit rhoEqn)
  ∂(ρU)/∂t + ∇·(ρUU)  = −∇p + ∇·τ    (momentum, UEqn)
  ∂(ρh)/∂t + ∇·(ρUh)  = dp/dt        (energy, h-form, EEqn)
  ρ, T, ψ, μ, αh from a real IAPWS-IF97 (p,h) flash (see `correct_thermo`)
```

## What differs from `RhoPimpleFoam`
- **Mesh**: a uniform 1-D `FvMesh` (`n_cells` cells along x) rather than an
  arbitrary polyMesh.
- **Control**: a few plain fields (`delta_t`, corrector counts) replace the
  `ControlDict` / `FvSchemes` / `FvSolution` dictionaries — this crate does
  not consume OpenFOAM case files.
- **Thermophysics**: [`Self::correct_thermo`] closes the EOS with a real
  IAPWS-IF97 `(p, h)` flash (not a placeholder linearisation) — see that
  method's doc comment for the exact per-cell property list.

C++ reference: `applications/solvers/compressible/rhoPimpleFoam/`.

```rust
pub struct TampinesSteamArray {
    pub mesh: std::sync::Arc<FvMesh>,
    pub delta_t: uom::si::f64::Time,
    pub time_step_control: TimeStepControl,
    pub n_outer_correctors: usize,
    pub n_inner_correctors: usize,
    pub p_under_relaxation: uom::si::f64::Ratio,
    pub u_under_relaxation: uom::si::f64::Ratio,
    pub p_min: uom::si::f64::Pressure,
    pub p_max: uom::si::f64::Pressure,
    pub psi_refresh: PsiRefresh,
    pub thermo_closure: ThermoClosure,
    pub he_convection_scheme: EnergyConvectionScheme,
    pub he_balance_mode: EnergyBalanceMode,
    pub kinetic_energy_terms: KineticEnergyTerms,
    pub u: VolField<crate::openfoam_algorithms::openfoam_source::Vector3>,
    pub p: VolField<f64>,
    pub rho: VolField<f64>,
    pub t: VolField<f64>,
    pub he: VolField<f64>,
    pub mu: VolField<f64>,
    pub alpha_h: VolField<f64>,
    pub psi: VolField<f64>,
    pub phi: SurfaceField<f64>,
    pub xs_area: uom::si::f64::Area,
    pub wetted_perimeter: uom::si::f64::Length,
    pub incline_angle: uom::si::f64::Angle,
    pub mass_flowrate: uom::si::f64::MassRate,
    pub inlet_mass_flowrate: Option<uom::si::f64::MassRate>,
    pub inlet_terminal: AdvectionTerminalState,
    pub outlet_terminal: AdvectionTerminalState,
    pub pressure_loss: uom::si::f64::Pressure,
    pub internal_pressure_source: uom::si::f64::Pressure,
    pub lateral_adjacent_array_temperature_vector: Vec<Vec<uom::si::f64::ThermodynamicTemperature>>,
    pub lateral_adjacent_array_conductance_vector: Vec<Vec<uom::si::f64::ThermalConductance>>,
    pub q_vector: Vec<uom::si::f64::Power>,
    pub q_fraction_vector: Vec<Vec<f64>>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<FvMesh>` | 1-D finite-volume mesh (built by [`create_one_d_mesh`]). |
| `delta_t` | `uom::si::f64::Time` | Time step Δt \[s\]: the caller's fixed step under<br>[`TimeStepControl::Fixed`], or the step the Courant controller last<br>chose under [`TimeStepControl::AdaptiveCourant`]. |
| `time_step_control` | `TimeStepControl` | How Δt is chosen (default [`TimeStepControl::Fixed`], guarded). See<br>[`Self::set_time_step_control`]. |
| `n_outer_correctors` | `usize` | Number of PIMPLE outer correctors (≥ 1). See<br>[`Self::set_piso_algorithm`] / [`Self::set_simple_algorithm`] /<br>[`Self::set_pimple_algorithm`] for the PISO/SIMPLE/PIMPLE presets. |
| `n_inner_correctors` | `usize` | Number of PISO pressure correctors per outer loop (≥ 1). |
| `p_under_relaxation` | `uom::si::f64::Ratio` | Explicit pressure under-relaxation factor α_p ∈ (0, 1] applied once<br>per inner correction: `p ← p_prev + α_p·(p_solved − p_prev)`.<br>`1.0` (the [`Self::new`] default, matching classic transient PISO)<br>takes each correction in full; smaller values trade convergence<br>speed for stability in iterative (SIMPLE-style) solves. |
| `u_under_relaxation` | `uom::si::f64::Ratio` | Explicit velocity under-relaxation factor α_u ∈ (0, 1] -- see<br>[`Self::p_under_relaxation`]. |
| `p_min` | `uom::si::f64::Pressure` | Lower pressure bound \[Pa\] applied after every pressure solve (see<br>[`Self::step`]). Defaults to the IAPWS-IF97 lower validity limit<br>(triple-point pressure ≈ 611.657 Pa); raise it with<br>[`Self::set_pressure_bounds`] to clamp a violent transient (e.g. a<br>water-hammer rarefaction that would otherwise undershoot to negative<br>absolute pressure) instead of letting the `(p, h)` flash panic<br>out-of-range. This mirrors OpenFOAM's `pressureControl::limit`<br>`pMin`/`pMax` bounding — see [`Self::step`] for the reference. |
| `p_max` | `uom::si::f64::Pressure` | Upper pressure bound \[Pa\] applied after every pressure solve.<br>Defaults to the IAPWS-IF97 upper validity limit (100 MPa). See<br>[`Self::p_min`]. |
| `psi_refresh` | `PsiRefresh` | How often `psi` is rebuilt inside the PIMPLE loop (default<br>~~[`PsiRefresh::EveryCorrector`], the historical path~~<br>[`PsiRefresh::OncePerOuterCorrector`] since 2026-10-16, gh:#900). |
| `thermo_closure` | `ThermoClosure` | Thermodynamic closure used by `correct_thermo` (default<br>[`ThermoClosure::PressureEnthalpy`], the validated path). |
| `he_convection_scheme` | `EnergyConvectionScheme` | Face scheme for the energy equation's convection `∇·(φh)` (default<br>[`EnergyConvectionScheme::VanLeer`]). See<br>[`Self::set_he_convection_scheme`]. |
| `he_balance_mode` | `EnergyBalanceMode` | Implicit or explicit treatment of the energy convection (default<br>[`EnergyBalanceMode::Implicit`] since 2026-10-16). See<br>[`Self::set_he_balance_mode`]. |
| `kinetic_energy_terms` | `KineticEnergyTerms` | Whether the energy equation carries upstream's kinetic-energy terms<br>(default [`KineticEnergyTerms::Included`]). See<br>[`Self::set_kinetic_energy_terms`]. |
| `u` | `VolField<crate::openfoam_algorithms::openfoam_source::Vector3>` | Velocity field \[m/s\]. |
| `p` | `VolField<f64>` | Pressure field \[Pa\]. |
| `rho` | `VolField<f64>` | Density field \[kg/m³\]. |
| `t` | `VolField<f64>` | Temperature field \[K\]. |
| `he` | `VolField<f64>` | Specific enthalpy \[J/kg\]. |
| `mu` | `VolField<f64>` | Dynamic viscosity μ \[Pa·s\]. |
| `alpha_h` | `VolField<f64>` | Effective thermal diffusivity αh = κ/Cp \[kg/(m·s)\]. |
| `psi` | `VolField<f64>` | Compressibility ψ = ∂ρ/∂p|_h \[s²/m²\] — the density's response to<br>pressure at **fixed enthalpy**, the correct linearisation for this<br>segregated pressure equation (he is frozen during the pressure solve).<br>Computed by a central finite difference of the real IAPWS-IF97 `(p, h)`<br>flash — see [`Self::correct_thermo`]. In single phase this equals the<br>isothermal ρ·κ_T; in the two-phase dome it is much larger because it<br>carries the flashing term `(v_g − v_f)·dx/dp`. |
| `phi` | `SurfaceField<f64>` | Mass flux φ = ρ U·Sf \[kg/s\]. |
| `xs_area` | `uom::si::f64::Area` | Constant cross-sectional area \[m²\] (same value passed to [`Self::new`]). |
| `wetted_perimeter` | `uom::si::f64::Length` | Wetted perimeter \[m\] (bookkeeping -- see [`Self::get_hydraulic_diameter`]). |
| `incline_angle` | `uom::si::f64::Angle` | Incline angle from horizontal \[rad\] (bookkeeping only). |
| `mass_flowrate` | `uom::si::f64::MassRate` | Bulk mass flowrate \[kg/s\] (plain storage -- `step()` does not read<br>this; it is bookkeeping for a caller, same as `OPCPFluidArray`'s field).<br><br>**This is NOT a boundary condition.** To actually drive the array at a<br>known mass flow, use [`Self::set_inlet_mass_flowrate`], which imposes it<br>on the inlet patch. |
| `inlet_mass_flowrate` | `Option<uom::si::f64::MassRate>` | Prescribed **inlet mass flowrate** \[kg/s\], or `None` for no<br>mass-flow inlet.<br><br>When set, each pressure corrector re-derives the inlet velocity<br>`u_in = m_dot / (rho_in A_in)` from the *same* interpolated inlet-face<br>density that then multiplies it to form the boundary mass flux, so the<br>imposed flux is exactly `m_dot` by construction rather than by a<br>caller's density guess. This is OpenFOAM's `flowRateInletVelocity`.<br><br>Positive is **into** the domain (+x). Set with<br>[`Self::set_inlet_mass_flowrate`], clear with<br>[`Self::clear_inlet_mass_flowrate`] or by calling<br>[`Self::set_inlet_velocity`] (which prescribes a velocity instead and so<br>clears this). |
| `inlet_terminal` | `AdvectionTerminalState` | Upwind advection state at the **inlet terminal** — the `"left"` patch<br>at x = 0 (`INLET_PATCH`).<br><br>Defaults to [`AdvectionTerminalState::ZeroGradientExtrapolated`]. Set a<br>junction enthalpy with [`Self::set_inlet_enthalpy`], clear it with<br>[`Self::clear_inlet_enthalpy`]. Used **only when the flow at this<br>terminal is into the domain**; on outflow the control-volume state is<br>upwind. See [`Self::correct_advection_terminals`]. |
| `outlet_terminal` | `AdvectionTerminalState` | Upwind advection state at the **outlet terminal** — the `"right"` patch<br>at x = length.<br><br>The outlet needs this for exactly the same reason the inlet does: when<br>the flow reverses, the "outlet" is an inlet, and without a junction<br>state the domain advects its own enthalpy back in. Defaults to<br>[`AdvectionTerminalState::ZeroGradientExtrapolated`]; set with<br>[`Self::set_outlet_enthalpy`], clear with<br>[`Self::clear_outlet_enthalpy`]. |
| `pressure_loss` | `uom::si::f64::Pressure` | Pressure loss \[Pa\] (plain storage, independent of `mass_flowrate`). |
| `internal_pressure_source` | `uom::si::f64::Pressure` | Internal pressure source \[Pa\] (e.g. a simulated pump; plain storage). |
| `lateral_adjacent_array_temperature_vector` | `Vec<Vec<uom::si::f64::ThermodynamicTemperature>>` | Per-registered-link neighbour temperature, one inner `Vec` per cell.<br>Registered via<br>[`Self::lateral_link_new_temperature_vector_avg_conductance`] and<br>cleared once per [`Self::step`] (see [`Self::clear_vectors`]). |
| `lateral_adjacent_array_conductance_vector` | `Vec<Vec<uom::si::f64::ThermalConductance>>` | Parallel to `lateral_adjacent_array_temperature_vector`: per-cell<br>thermal conductance for the same link. |
| `q_vector` | `Vec<uom::si::f64::Power>` | Per-registered-source total power; distributed across cells by the<br>matching entry in `q_fraction_vector`. |
| `q_fraction_vector` | `Vec<Vec<f64>>` | Parallel to `q_vector`: per-cell distribution fraction for the same<br>source (need not sum to 1). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn lateral_link_new_temperature_vector_avg_conductance(self: &mut Self, average_thermal_conductance: ThermalConductance, temperature_vec: Vec<ThermodynamicTemperature>) -> Result<(), TampinesSteamArrayError> { /* ... */ }
  ```
  Register one lateral (radial) thermal link to another array/solid at a

- ```rust
  pub fn lateral_link_new_power_vector(self: &mut Self, power_source: Power, q_fraction_vec: Vec<f64>) -> Result<(), TampinesSteamArrayError> { /* ... */ }
  ```
  Register a volumetric heat source for use in the next [`Self::step`].

- ```rust
  pub fn clear_vectors(self: &mut Self) { /* ... */ }
  ```
  Empty all registered lateral-coupling and heat-source vectors.

- ```rust
  pub fn get_wetted_perimeter(self: &Self) -> Length { /* ... */ }
  ```
  Wetted perimeter \[m\] (bookkeeping — see [`Self::get_hydraulic_diameter`]).

- ```rust
  pub fn set_wetted_perimeter(self: &mut Self, wetted_perimeter: Length) { /* ... */ }
  ```
  Set the wetted perimeter \[m\].

- ```rust
  pub fn get_incline_angle(self: &Self) -> Angle { /* ... */ }
  ```
  Incline angle from horizontal \[rad\] (bookkeeping only).

- ```rust
  pub fn set_incline_angle(self: &mut Self, incline_angle: Angle) { /* ... */ }
  ```
  Set the incline angle from horizontal \[rad\].

- ```rust
  pub fn get_hydraulic_diameter(self: &Self) -> Length { /* ... */ }
  ```
  Hydraulic diameter `D_h = 4 * xs_area / wetted_perimeter` \[m\].

- ```rust
  pub fn get_mass_flowrate(self: &Self) -> MassRate { /* ... */ }
  ```
  Bulk mass flowrate \[kg/s\] -- inert bookkeeping, NOT a boundary

- ```rust
  pub fn set_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
  ```
  Set the bulk mass flowrate \[kg/s\].

- ```rust
  pub fn get_pressure_loss(self: &Self) -> Pressure { /* ... */ }
  ```
  Pressure loss \[Pa\] (plain storage, independent of `mass_flowrate`).

- ```rust
  pub fn set_pressure_loss(self: &mut Self, pressure_loss: Pressure) { /* ... */ }
  ```
  Set the pressure loss \[Pa\].

- ```rust
  pub fn get_internal_pressure_source(self: &Self) -> Pressure { /* ... */ }
  ```
  Internal pressure source \[Pa\] (e.g. a simulated pump; plain storage).

- ```rust
  pub fn set_internal_pressure_source(self: &mut Self, internal_pressure_source: Pressure) { /* ... */ }
  ```
  Set the internal pressure source \[Pa\].

- ```rust
  pub fn get_temperature_vector(self: &Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  Per-cell temperature \[K\], read from the `t` field (length `mesh.n_cells`).

- ```rust
  pub fn set_temperature_vector(self: &mut Self, temperature_vec: Vec<ThermodynamicTemperature>) -> Result<(), TampinesSteamArrayError> { /* ... */ }
  ```
  Overwrite the per-cell temperature at the current pressure, via a real

- ```rust
  pub fn set_uniform_velocity_field(self: &mut Self, velocity: Velocity) { /* ... */ }
  ```
  Overwrites the whole **internal** velocity field to a uniform axial

- ```rust
  pub fn set_inlet_velocity(self: &mut Self, velocity: Velocity) { /* ... */ }
  ```
  Prescribes a fixed inlet velocity boundary condition on the

- ```rust
  pub fn set_inlet_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
  ```
  Prescribe a fixed inlet **mass flowrate** \[kg/s\] on the `"left"` patch

- ```rust
  pub fn clear_inlet_mass_flowrate(self: &mut Self) { /* ... */ }
  ```
  Remove any prescribed inlet mass flowrate, leaving whatever velocity

- ```rust
  pub fn set_inlet_enthalpy(self: &mut Self, h: AvailableEnergy) { /* ... */ }
  ```
  Prescribe the **junction specific enthalpy** \[J/kg\] at the inlet

- ```rust
  pub fn clear_inlet_enthalpy(self: &mut Self) { /* ... */ }
  ```
  Remove the inlet junction enthalpy, returning that terminal to

- ```rust
  pub fn set_outlet_enthalpy(self: &mut Self, h: AvailableEnergy) { /* ... */ }
  ```
  Prescribe the **junction specific enthalpy** \[J/kg\] at the outlet

- ```rust
  pub fn clear_outlet_enthalpy(self: &mut Self) { /* ... */ }
  ```
  Remove the outlet junction enthalpy, returning that terminal to

- ```rust
  pub fn set_outlet_velocity(self: &mut Self, velocity: Velocity) { /* ... */ }
  ```
  Prescribes a fixed outlet velocity boundary condition on the

- ```rust
  pub fn set_outlet_pressure(self: &mut Self, p: Pressure) { /* ... */ }
  ```
  Prescribes a fixed outlet pressure boundary condition on the

- ```rust
  pub fn get_outlet_pressure(self: &Self) -> Pressure { /* ... */ }
  ```
  Outlet-cell (the last cell, owner of the `"right"` patch) pressure

- ```rust
  pub fn get_outlet_enthalpy(self: &Self) -> AvailableEnergy { /* ... */ }
  ```
  Outlet-cell specific enthalpy.

- ```rust
  pub fn get_outlet_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  Outlet-cell temperature.

- ```rust
  pub fn new(length: Length, xs_area: Area, number_of_cells: i64, delta_t: Time) -> Result<Self, MeshError> { /* ... */ }
  ```
  Build a 1-D pipe array with uniform initial conditions.

- ```rust
  pub fn correct_thermo(self: &mut Self) -> Result<(), TampinesSteamArrayError> { /* ... */ }
  ```
  Update the thermodynamic and transport state from the current

- ```rust
  pub fn axial_peclet_numbers(self: &Self) -> Vec<Ratio> { /* ... */ }
  ```
  Per-cell **axial Péclet number** `Pe = ρ|u|Δx / (λ/c_p)` \[dimensionless\]

- ```rust
  pub fn peak_axial_conduction_rate(self: &Self) -> Power { /* ... */ }
  ```
  Largest **axial conduction heat rate** across any internal face \[W\].

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TampinesSteamArrayError> { /* ... */ }
  ```
  Advance one time step with the compressible PIMPLE algorithm.

- ```rust
  pub fn step(self: &mut Self) -> Result<Time, TampinesSteamArrayError> { /* ... */ }
  ```
  Advance the solution by one step and return the step taken.

- ```rust
  pub fn courant_number(self: &Self) -> (Ratio, Ratio) { /* ... */ }
  ```
  The convective Courant number of upstream `compressibleCourantNo.H`,

- ```rust
  pub fn run(self: &mut Self, n_steps: usize) -> Result<(), TampinesSteamArrayError> { /* ... */ }
  ```
  Advance `n_steps` steps via [`Self::step`] (each of size `delta_t`

- ```rust
  pub fn get_time_step_control(self: &Self) -> TimeStepControl { /* ... */ }
  ```
  How Δt is chosen; see [`TimeStepControl`].

- ```rust
  pub fn set_time_step_control(self: &mut Self, control: TimeStepControl) { /* ... */ }
  ```
  Select how Δt is chosen: guarded [`TimeStepControl::Fixed`] (the

- ```rust
  pub fn get_he_balance_mode(self: &Self) -> EnergyBalanceMode { /* ... */ }
  ```
  Implicit or explicit energy convection; see [`EnergyBalanceMode`].

- ```rust
  pub fn set_he_balance_mode(self: &mut Self, mode: EnergyBalanceMode) { /* ... */ }
  ```
  Select implicit (default, upstream `fvm::div`) or explicit (ablation)

- ```rust
  pub fn get_kinetic_energy_terms(self: &Self) -> KineticEnergyTerms { /* ... */ }
  ```
  Whether the kinetic-energy terms are in the energy equation; see

- ```rust
  pub fn set_kinetic_energy_terms(self: &mut Self, terms: KineticEnergyTerms) { /* ... */ }
  ```
  Include (default) or exclude (ablation) upstream's kinetic-energy

- ```rust
  pub fn get_n_outer_correctors(self: &Self) -> usize { /* ... */ }
  ```
  Number of PIMPLE outer correctors per [`Self::step`] call.

- ```rust
  pub fn set_n_outer_correctors(self: &mut Self, n: usize) { /* ... */ }
  ```
  Sets the number of PIMPLE outer correctors (clamped to ≥ 1).

- ```rust
  pub fn get_n_inner_correctors(self: &Self) -> usize { /* ... */ }
  ```
  Number of PISO pressure correctors per outer loop.

- ```rust
  pub fn set_n_inner_correctors(self: &mut Self, n: usize) { /* ... */ }
  ```
  Sets the number of PISO inner pressure correctors (clamped to ≥ 1).

- ```rust
  pub fn get_pressure_under_relaxation(self: &Self) -> Ratio { /* ... */ }
  ```
  Pressure under-relaxation factor α_p -- see

- ```rust
  pub fn set_pressure_under_relaxation(self: &mut Self, alpha: Ratio) { /* ... */ }
  ```
  Sets the pressure under-relaxation factor, clamped to (0, 1].

- ```rust
  pub fn get_velocity_under_relaxation(self: &Self) -> Ratio { /* ... */ }
  ```
  Velocity under-relaxation factor α_u -- see

- ```rust
  pub fn set_velocity_under_relaxation(self: &mut Self, alpha: Ratio) { /* ... */ }
  ```
  Sets the velocity under-relaxation factor, clamped to (0, 1].

- ```rust
  pub fn set_piso_algorithm(self: &mut Self, n_correctors: usize) { /* ... */ }
  ```
  Configures this array for a transient PISO solve: one outer

- ```rust
  pub fn set_simple_algorithm(self: &mut Self, n_outer_iterations: usize) { /* ... */ }
  ```
  Configures this array for a SIMPLE steady-state solve:

- ```rust
  pub fn set_pimple_algorithm(self: &mut Self, n_outer_correctors: usize, n_inner_correctors: usize, pressure_under_relaxation: Ratio, velocity_under_relaxation: Ratio) { /* ... */ }
  ```
  Configures this array for a PIMPLE solve -- multiple outer

- ```rust
  pub fn get_pressure_bounds(self: &Self) -> (Pressure, Pressure) { /* ... */ }
  ```
  Current pressure bounds `(p_min, p_max)` applied after every pressure

- ```rust
  pub fn set_pressure_bounds(self: &mut Self, p_min: Pressure, p_max: Pressure) { /* ... */ }
  ```
  Sets the pressure bounds `[p_min, p_max]` clamped after every pressure

- ```rust
  pub fn pressure_bound_events(self: &Self) -> usize { /* ... */ }
  ```
  How many times the pressure solve produced a value outside

- ```rust
  pub fn pressure_bound_worst_undershoot(self: &Self) -> Pressure { /* ... */ }
  ```
  The largest amount by which the pressure solve undershot `p_min` before

- ```rust
  pub fn pressure_bound_worst_overshoot(self: &Self) -> Pressure { /* ... */ }
  ```
  The largest amount by which the pressure solve overshot `p_max` before

- ```rust
  pub fn drained_hold_events(self: &Self) -> usize { /* ... */ }
  ```
  How many times the drained-cell enthalpy hold engaged, counted per

- ```rust
  pub fn get_he_convection_scheme(self: &Self) -> EnergyConvectionScheme { /* ... */ }
  ```
  The face scheme of the energy equation's convection term. See

- ```rust
  pub fn set_he_convection_scheme(self: &mut Self, scheme: EnergyConvectionScheme) { /* ... */ }
  ```
  Select the face scheme of the energy equation's convection term. The

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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

#### Re-export `TampinesSteamArrayError`

```rust
pub use lateral_coupling::TampinesSteamArrayError;
```

## Module `tabulated_data`

published (Wagner/Kretzschmar) steam-table reference data, queryable by
isobar/isotherm cross-section or the full saturation curve — see
[`tabulated_data::TabulatedData`]
Published steam-table reference data as a first-class library type.

**Added for GitHub issue #26, 2026-08-21.** The plotter GUI
(`examples/tampines-steam-tables-gui/`) needed the Wagner/Kretzschmar
single-phase and saturation tables to draw its "validation coverage"
overlays; that data used to live only as a GUI-local copy
(`reference_data/wagner.rs`), itself a mechanical duplicate of the
`#[cfg(test)]` fixtures in `src/interfaces/tests_and_examples/`. The
maintainer asked for it to move here instead, as a queryable type any
consumer of this crate can reach — not just the GUI, and not a third copy
of the same numbers.

The raw tables live in [`wagner_kretzschmar_2019`]; this module wraps them
in a query surface: [`TabulatedData::isobar`] and [`TabulatedData::isotherm`]
pull a cross-section of the published single-phase table, and
[`TabulatedData::saturation_curve`] hands back the saturation table as
typed rows. Every query is a plain filter over the embedded tables —
nothing is interpolated, extrapolated, or invented. A query that matches no
row returns an empty `Vec`; that emptiness is itself informative (per issue
#26: "a region of a diagram with no Wagner points on it is a region with no
table verification behind it").

# Why isobar and isotherm, not more

The published single-phase tables are organised as isobar files (29 fixed
pressures, each swept over a shared temperature grid), so an isobar
cross-section is exactly one file's worth of rows and an isotherm
cross-section is one row from each of the (up to) 29 files. Isentrope,
isenthalp and isochore cross-sections would need interpolation between
tabulated rows to produce a usable line — this module deliberately does
not do that; it is a reference-data lookup, not a computation.

```rust
pub mod tabulated_data { /* ... */ }
```

### Modules

## Module `wagner_kretzschmar_2019`

Wagner / Kretzschmar *International Steam Tables* reference states.

**Relocated here from `examples/tampines-steam-tables-gui/reference_data/`
(GitHub issue #26, 2026-08-21).** It started as a GUI-only convenience
duplicate; issue #26 asked for it to become a first-class part of this
library instead, reachable by any consumer, not just the GUI example.
[`super::TabulatedData`] is the query surface built on top of these raw
tables — isobar/isotherm cross-sections and the saturation curve.

# Source

Kretzschmar, H.-J. & Wagner, W. (2019). *International Steam Tables:
Properties of Water and Steam Based on the Industrial Formulation
IAPWS-IF97* (3rd ed.). Springer. These are the published tabulated values
this crate verifies itself against -- they are the **reference**, not this
crate's output.

# In-repo source fixtures

`src/interfaces/tests_and_examples/ph_flash_steam_table/`:
* `saturation_table_*.rs` -- 5 files, 220 rows total
  (`WAGNER_SATURATION_TABLE`).
* `single_phase_table_*.rs` -- 29 isobar files, 2334 rows total
  (`WAGNER_SINGLE_PHASE_TABLE`).

# Why this data matters as a first-class library type

Querying it shows *where on the thermodynamic surface this crate has
actually been checked against published values* -- a region with no
matching Wagner points is a region with no table verification behind it,
and any consumer (the GUI's "validation coverage" figures, a future V&V
report, a test) should be able to ask the same question the same way.

# Column subsetting -- stated, not hidden

The single-phase fixtures carry ten columns
(`p, T, v, h, s, cp, w, kappa, eta, lambda`). Only the four needed to place a
point on a T-p / p-h / T-s / h-s diagram are kept here (`p, T, h, s`); the
other six are untouched in the fixtures. **No rows are dropped** -- every
row of every fixture file is present, including the ones whose owning test is
`#[ignore]`d (the 1000 bar table). The saturation table keeps all ten columns.

```rust
pub mod wagner_kretzschmar_2019 { /* ... */ }
```

### Types

#### Type Alias `WagnerSaturationRow`

A row of the Wagner saturation tables. Column order, matching the source
fixture's own `dataset[i]` unpacking exactly:

| index | quantity | unit |
|---|---|---|
| 0 | saturation temperature | degrees Celsius |
| 1 | saturation temperature | kelvin |
| 2 | saturation pressure | bar |
| 3 | saturated-liquid specific volume | m^3/kg |
| 4 | saturated-vapour specific volume | m^3/kg |
| 5 | saturated-liquid specific enthalpy | kJ/kg |
| 6 | saturated-vapour specific enthalpy | kJ/kg |
| 7 | enthalpy of vaporisation | kJ/kg |
| 8 | saturated-liquid specific entropy | kJ/(kg K) |
| 9 | saturated-vapour specific entropy | kJ/(kg K) |

```rust
pub type WagnerSaturationRow = [f64; 10];
```

#### Type Alias `WagnerSinglePhaseRow`

A row of the Wagner single-phase isobar tables, column-subset to the four
quantities a T-p / p-h / T-s / h-s diagram needs:

| index | quantity | unit |
|---|---|---|
| 0 | pressure | bar |
| 1 | temperature | degrees Celsius |
| 2 | specific enthalpy | kJ/kg |
| 3 | specific entropy | kJ/(kg K) |

```rust
pub type WagnerSinglePhaseRow = [f64; 4];
```

### Constants and Statics

#### Constant `SAT_COL_T_DEGC`

Index of the saturation temperature (degrees Celsius) column.

```rust
pub const SAT_COL_T_DEGC: usize = 0;
```

#### Constant `SAT_COL_P_BAR`

Index of the saturation pressure (bar) column.

```rust
pub const SAT_COL_P_BAR: usize = 2;
```

#### Constant `SAT_COL_H_LIQ`

Index of the saturated-liquid specific enthalpy (kJ/kg) column.

```rust
pub const SAT_COL_H_LIQ: usize = 5;
```

#### Constant `SAT_COL_H_VAP`

Index of the saturated-vapour specific enthalpy (kJ/kg) column.

```rust
pub const SAT_COL_H_VAP: usize = 6;
```

#### Constant `SAT_COL_S_LIQ`

Index of the saturated-liquid specific entropy (kJ/(kg K)) column.

```rust
pub const SAT_COL_S_LIQ: usize = 8;
```

#### Constant `SAT_COL_S_VAP`

Index of the saturated-vapour specific entropy (kJ/(kg K)) column.

```rust
pub const SAT_COL_S_VAP: usize = 9;
```

#### Constant `WAGNER_SATURATION_TABLE`

All 220 published saturation-table rows, in fixture order.

```rust
pub const WAGNER_SATURATION_TABLE: &[WagnerSaturationRow] = _;
```

#### Constant `WAGNER_SINGLE_PHASE_TABLE`

All 2334 published single-phase isobar rows, in fixture order.

```rust
pub const WAGNER_SINGLE_PHASE_TABLE: &[WagnerSinglePhaseRow] = _;
```

### Types

#### Enum `TabulatedQuantity`

Which cross-section of the tabulated single-phase surface to pull.

A closed enum, not a trait object or a bare `(bool, f64)` pair, per this
workspace's Rust design rules — the set of cross-section kinds this data
can actually answer is fixed (see the module doc for why it stops at these
two), and a `match` on it should be exhaustive at every call site.

```rust
pub enum TabulatedQuantity {
    Isobar(Pressure),
    Isotherm(ThermodynamicTemperature),
}
```

##### Variants

###### `Isobar`

Constant pressure — one (or zero) tabulated isobar file's worth of rows.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Pressure` |  |

###### `Isotherm`

Constant temperature — up to one row from each tabulated isobar file.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ThermodynamicTemperature` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `TabulatedState`

One tabulated single-phase state, unit-tagged from the raw
[`WagnerSinglePhaseRow`] via `uom`.

```rust
pub struct TabulatedState {
    pub pressure: Pressure,
    pub temperature: ThermodynamicTemperature,
    pub specific_enthalpy: AvailableEnergy,
    pub specific_entropy: SpecificHeatCapacity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `Pressure` |  |
| `temperature` | `ThermodynamicTemperature` |  |
| `specific_enthalpy` | `AvailableEnergy` |  |
| `specific_entropy` | `SpecificHeatCapacity` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `TabulatedSaturationState`

One tabulated saturation-line state — both phases, at one temperature.

```rust
pub struct TabulatedSaturationState {
    pub temperature: ThermodynamicTemperature,
    pub pressure: Pressure,
    pub liquid_enthalpy: AvailableEnergy,
    pub vapour_enthalpy: AvailableEnergy,
    pub liquid_entropy: SpecificHeatCapacity,
    pub vapour_entropy: SpecificHeatCapacity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `ThermodynamicTemperature` |  |
| `pressure` | `Pressure` |  |
| `liquid_enthalpy` | `AvailableEnergy` |  |
| `vapour_enthalpy` | `AvailableEnergy` |  |
| `liquid_entropy` | `SpecificHeatCapacity` |  |
| `vapour_entropy` | `SpecificHeatCapacity` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `TabulatedData`

Query surface over the embedded Wagner/Kretzschmar tables.

Zero-sized — every method reads the `pub const` tables in
[`wagner_kretzschmar_2019`] directly, so there is nothing to construct or
own. Exists as a named type (rather than bare free functions) so a
`TabulatedData::isobar(...)` call site reads as "ask the published data",
distinct from `curves::isobar(...)`, which computes the same cross-section
live from this crate's own IAPWS-IF97 implementation.

```rust
pub struct TabulatedData;
```

##### Implementations

###### Methods

- ```rust
  pub fn isobar(self: &Self, p: Pressure, tolerance_relative: f64) -> Vec<TabulatedState> { /* ... */ }
  ```
  Every published single-phase row within `tolerance_relative` of `p`

- ```rust
  pub fn isobar_default_tolerance(self: &Self, p: Pressure) -> Vec<TabulatedState> { /* ... */ }
  ```
  [`Self::isobar`] at [`DEFAULT_ISOBAR_TOLERANCE_RELATIVE`].

- ```rust
  pub fn isotherm(self: &Self, t: ThermodynamicTemperature, tolerance_kelvin: f64) -> Vec<TabulatedState> { /* ... */ }
  ```
  Every published single-phase row within `tolerance_kelvin` of `t` — at

- ```rust
  pub fn isotherm_default_tolerance(self: &Self, t: ThermodynamicTemperature) -> Vec<TabulatedState> { /* ... */ }
  ```
  [`Self::isotherm`] at [`DEFAULT_ISOTHERM_TOLERANCE_KELVIN`].

- ```rust
  pub fn cross_section(self: &Self, quantity: TabulatedQuantity) -> Vec<TabulatedState> { /* ... */ }
  ```
  Dispatches to [`Self::isobar_default_tolerance`] or

- ```rust
  pub fn saturation_curve(self: &Self) -> Vec<TabulatedSaturationState> { /* ... */ }
  ```
  The full published saturation curve, as typed rows in table order.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

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

#### Constant `DEFAULT_ISOBAR_TOLERANCE_RELATIVE`

Default matching tolerance for [`TabulatedData::isobar`]: 0.5 % relative to
the requested pressure. Relative, not absolute, because the tabulated
pressures span almost six decades (0.006 bar to 1000 bar) — a fixed
absolute tolerance either misses everything at the low end or matches
neighbouring isobars at the high end.

```rust
pub const DEFAULT_ISOBAR_TOLERANCE_RELATIVE: f64 = 0.005;
```

#### Constant `DEFAULT_ISOTHERM_TOLERANCE_KELVIN`

Default matching tolerance for [`TabulatedData::isotherm`]: 0.5 °C. The
published single-phase tables step temperature on a shared, small-integer
grid (2 °C near the low end, coarser at high pressure/temperature), so an
absolute tolerance well under one grid step avoids picking up the wrong
neighbouring row.

```rust
pub const DEFAULT_ISOTHERM_TOLERANCE_KELVIN: f64 = 0.5;
```

### Re-exports

#### Re-export `WagnerSaturationRow`

```rust
pub use wagner_kretzschmar_2019::WagnerSaturationRow;
```

#### Re-export `WagnerSinglePhaseRow`

```rust
pub use wagner_kretzschmar_2019::WagnerSinglePhaseRow;
```

#### Re-export `SAT_COL_H_LIQ`

```rust
pub use wagner_kretzschmar_2019::SAT_COL_H_LIQ;
```

#### Re-export `SAT_COL_H_VAP`

```rust
pub use wagner_kretzschmar_2019::SAT_COL_H_VAP;
```

#### Re-export `SAT_COL_P_BAR`

```rust
pub use wagner_kretzschmar_2019::SAT_COL_P_BAR;
```

#### Re-export `SAT_COL_S_LIQ`

```rust
pub use wagner_kretzschmar_2019::SAT_COL_S_LIQ;
```

#### Re-export `SAT_COL_S_VAP`

```rust
pub use wagner_kretzschmar_2019::SAT_COL_S_VAP;
```

#### Re-export `SAT_COL_T_DEGC`

```rust
pub use wagner_kretzschmar_2019::SAT_COL_T_DEGC;
```

#### Re-export `WAGNER_SATURATION_TABLE`

```rust
pub use wagner_kretzschmar_2019::WAGNER_SATURATION_TABLE;
```

#### Re-export `WAGNER_SINGLE_PHASE_TABLE`

```rust
pub use wagner_kretzschmar_2019::WAGNER_SINGLE_PHASE_TABLE;
```

## Re-exports

### Re-export `AdvectionTerminalState`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

~~`SolverMode` and `KnpFaceClosure`~~ were removed 2026-10-16 with the
retired KNP hybrid (gh:#914); the array has one flux discretisation, PIMPLE.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::AdvectionTerminalState;
```

### Re-export `EnergyBalanceMode`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

~~`SolverMode` and `KnpFaceClosure`~~ were removed 2026-10-16 with the
retired KNP hybrid (gh:#914); the array has one flux discretisation, PIMPLE.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::EnergyBalanceMode;
```

### Re-export `EnergyConvectionScheme`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

~~`SolverMode` and `KnpFaceClosure`~~ were removed 2026-10-16 with the
retired KNP hybrid (gh:#914); the array has one flux discretisation, PIMPLE.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::EnergyConvectionScheme;
```

### Re-export `KineticEnergyTerms`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

~~`SolverMode` and `KnpFaceClosure`~~ were removed 2026-10-16 with the
retired KNP hybrid (gh:#914); the array has one flux discretisation, PIMPLE.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::KineticEnergyTerms;
```

### Re-export `PsiRefresh`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

~~`SolverMode` and `KnpFaceClosure`~~ were removed 2026-10-16 with the
retired KNP hybrid (gh:#914); the array has one flux discretisation, PIMPLE.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::PsiRefresh;
```

### Re-export `TampinesSteamArray`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

~~`SolverMode` and `KnpFaceClosure`~~ were removed 2026-10-16 with the
retired KNP hybrid (gh:#914); the array has one flux discretisation, PIMPLE.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray;
```

### Re-export `TampinesSteamArrayError`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

~~`SolverMode` and `KnpFaceClosure`~~ were removed 2026-10-16 with the
retired KNP hybrid (gh:#914); the array has one flux discretisation, PIMPLE.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::TampinesSteamArrayError;
```

### Re-export `ThermoClosure`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

~~`SolverMode` and `KnpFaceClosure`~~ were removed 2026-10-16 with the
retired KNP hybrid (gh:#914); the array has one flux discretisation, PIMPLE.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::ThermoClosure;
```

### Re-export `TimeStepControl`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

~~`SolverMode` and `KnpFaceClosure`~~ were removed 2026-10-16 with the
retired KNP hybrid (gh:#914); the array has one flux discretisation, PIMPLE.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::TimeStepControl;
```

