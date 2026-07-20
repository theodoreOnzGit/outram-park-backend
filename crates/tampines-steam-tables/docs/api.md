# Crate Documentation

**Version:** 0.2.2

**Format Version:** 60

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
[`steam_turbine_equations`].

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

### Re-exports

#### Re-export `functional_programming`

```rust
pub use crate::interfaces::functional_programming;
```

#### Re-export `get_choked_flow_massrate_and_state_from_stagnation_properties_and_area`

```rust
pub use crate::steam_turbine_equations::converging_diverging_nozzles::isentropic_converging_nozzle::get_choked_flow_massrate_and_state_from_stagnation_properties_and_area;
```

#### Re-export `crate::interfaces::object_oriented_programming::*`

```rust
pub use crate::interfaces::object_oriented_programming::*;
```

## Module `constants`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

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
Only forward `(p,T)` equations exist for Region 5 — there are no backward
equations.

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
    fn clone(self: &Self) -> FwdEqnRegion { /* ... */ }
    ```

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
    fn cmp(self: &Self, other: &FwdEqnRegion) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FwdEqnRegion) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &FwdEqnRegion) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `region_fwd_eqn_single_phase`

Determines which region of the pT chart
a point belongs to.

Returns an error if the point is outside the
bounds of the IAPWS-IF97 correlations.

Temperature is assumed to be in K
Pressure is assumed to be in Pa


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

obtains temperature given pressure and enthalpy

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

returns the speed of sound given temperature and pressure
Note: when in the equilibrium zone (Region 4) it WILL give the
wood wallis speed of sound, the frozen speed of sound

```rust
pub fn w_ph_wood_wallis(p: Pressure, h: AvailableEnergy) -> Velocity { /* ... */ }
```

#### Function `w_two_phase_homogeneous_wood_wallis`

Returns the speed of sound in a two-phase mixture using the
homogeneous equilibrium model

This model assumes:
- Thermal and mechanical equilibrium between phases
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
`(p,h)` flash via [`ph_flash_region`]. Valid over the same `(p,h)` range
as the rest of this module (Regions 1-4; Region 5 is unsupported).

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
the point falls outside the valid pressure/entropy envelope; Region 5 is
not yet implemented for the callers that key off this dispatcher (see
`todo!` panics in `t_ps_eqm`/`v_ps_eqm`).

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
    fn clone(self: &Self) -> BackwdEqnSubRegion { /* ... */ }
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
    fn cmp(self: &Self, other: &BackwdEqnSubRegion) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BackwdEqnSubRegion) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &BackwdEqnSubRegion) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
    fn clone(self: &Self) -> CvMassEnthalpyChanges { /* ... */ }
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
    fn default() -> CvMassEnthalpyChanges { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CvMassEnthalpyChanges) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
  returns speed of sound

- ```rust
  pub fn get_mach_number(self: &Self, v: Velocity) -> Ratio { /* ... */ }
  ```
  get mach number

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
    fn clone(self: &Self) -> TampinesSteamTableCV { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &TampinesSteamTableCV) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

- `Other("#[allow(non_snake_case)]")`

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
    fn clone(self: &Self) -> ThreePhaseElectricGeneratorTurbine { /* ... */ }
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
    fn eq(self: &Self, other: &ThreePhaseElectricGeneratorTurbine) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Re-export `converging_diverging_nozzles::*`

```rust
pub use converging_diverging_nozzles::*;
```

#### Re-export `generator::*`

```rust
pub use generator::*;
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

- `Other("#[allow(non_snake_case)]")`

Rust port of OpenFOAM's `rhoPimpleFoam` (compressible PIMPLE) solver,
specialised into `TampinesSteamArray` — a 1-D pipe solver over TAMPINES
IAPWS-IF97 steam properties. This is the only public export of
`openfoam_algorithms`.

```rust
pub mod rhoPimpleFoam { /* ... */ }
```

### Types

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
    pub n_outer_correctors: usize,
    pub n_inner_correctors: usize,
    pub p_under_relaxation: uom::si::f64::Ratio,
    pub u_under_relaxation: uom::si::f64::Ratio,
    pub p_min: uom::si::f64::Pressure,
    pub p_max: uom::si::f64::Pressure,
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
    pub pressure_loss: uom::si::f64::Pressure,
    pub internal_pressure_source: uom::si::f64::Pressure,
    pub lateral_adjacent_array_temperature_vector: Vec<Vec<uom::si::f64::ThermodynamicTemperature>>,
    pub lateral_adjacent_array_conductance_vector: Vec<Vec<uom::si::f64::ThermalConductance>>,
    pub q_vector: Vec<uom::si::f64::Power>,
    pub q_fraction_vector: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<FvMesh>` | 1-D finite-volume mesh (built by [`create_one_d_mesh`]). |
| `delta_t` | `uom::si::f64::Time` | Fixed time step Δt \[s\]. |
| `n_outer_correctors` | `usize` | Number of PIMPLE outer correctors (≥ 1). See<br>[`Self::set_piso_algorithm`] / [`Self::set_simple_algorithm`] /<br>[`Self::set_pimple_algorithm`] for the PISO/SIMPLE/PIMPLE presets. |
| `n_inner_correctors` | `usize` | Number of PISO pressure correctors per outer loop (≥ 1). |
| `p_under_relaxation` | `uom::si::f64::Ratio` | Explicit pressure under-relaxation factor α_p ∈ (0, 1] applied once<br>per inner correction: `p ← p_prev + α_p·(p_solved − p_prev)`.<br>`1.0` (the [`Self::new`] default, matching classic transient PISO)<br>takes each correction in full; smaller values trade convergence<br>speed for stability in iterative (SIMPLE-style) solves. |
| `u_under_relaxation` | `uom::si::f64::Ratio` | Explicit velocity under-relaxation factor α_u ∈ (0, 1] -- see<br>[`Self::p_under_relaxation`]. |
| `p_min` | `uom::si::f64::Pressure` | Lower pressure bound \[Pa\] applied after every pressure solve (see<br>[`Self::step`]). Defaults to the IAPWS-IF97 lower validity limit<br>(triple-point pressure ≈ 611.657 Pa); raise it with<br>[`Self::set_pressure_bounds`] to clamp a violent transient (e.g. a<br>water-hammer rarefaction that would otherwise undershoot to negative<br>absolute pressure) instead of letting the `(p, h)` flash panic<br>out-of-range. This mirrors OpenFOAM's `pressureControl::limit`<br>`pMin`/`pMax` bounding — see [`Self::step`] for the reference. |
| `p_max` | `uom::si::f64::Pressure` | Upper pressure bound \[Pa\] applied after every pressure solve.<br>Defaults to the IAPWS-IF97 upper validity limit (100 MPa). See<br>[`Self::p_min`]. |
| `u` | `VolField<crate::openfoam_algorithms::openfoam_source::Vector3>` | Velocity field \[m/s\]. |
| `p` | `VolField<f64>` | Pressure field \[Pa\]. |
| `rho` | `VolField<f64>` | Density field \[kg/m³\]. |
| `t` | `VolField<f64>` | Temperature field \[K\]. |
| `he` | `VolField<f64>` | Specific enthalpy \[J/kg\]. |
| `mu` | `VolField<f64>` | Dynamic viscosity μ \[Pa·s\]. |
| `alpha_h` | `VolField<f64>` | Effective thermal diffusivity αh = κ/Cp \[kg/(m·s)\]. |
| `psi` | `VolField<f64>` | Compressibility ψ = ∂ρ/∂p|_T = ρ·κ_T \[s²/m²\] (κ_T from a real<br>IAPWS-IF97 flash — see [`Self::correct_thermo`]). |
| `phi` | `SurfaceField<f64>` | Mass flux φ = ρ U·Sf \[kg/s\]. |
| `xs_area` | `uom::si::f64::Area` | Constant cross-sectional area \[m²\] (same value passed to [`Self::new`]). |
| `wetted_perimeter` | `uom::si::f64::Length` | Wetted perimeter \[m\] (bookkeeping -- see [`Self::get_hydraulic_diameter`]). |
| `incline_angle` | `uom::si::f64::Angle` | Incline angle from horizontal \[rad\] (bookkeeping only). |
| `mass_flowrate` | `uom::si::f64::MassRate` | Bulk mass flowrate \[kg/s\] (plain storage -- `step()` does not read<br>this; it is bookkeeping for a caller, same as `OPCPFluidArray`'s field). |
| `pressure_loss` | `uom::si::f64::Pressure` | Pressure loss \[Pa\] (plain storage, independent of `mass_flowrate`). |
| `internal_pressure_source` | `uom::si::f64::Pressure` | Internal pressure source \[Pa\] (e.g. a simulated pump; plain storage). |
| `lateral_adjacent_array_temperature_vector` | `Vec<Vec<uom::si::f64::ThermodynamicTemperature>>` | Per-registered-link neighbour temperature, one inner `Vec` per cell.<br>Registered via<br>[`Self::lateral_link_new_temperature_vector_avg_conductance`] and<br>cleared once per [`Self::step`] (see [`Self::clear_vectors`]). |
| `lateral_adjacent_array_conductance_vector` | `Vec<Vec<uom::si::f64::ThermalConductance>>` | Parallel to `lateral_adjacent_array_temperature_vector`: per-cell<br>thermal conductance for the same link. |
| `q_vector` | `Vec<uom::si::f64::Power>` | Per-registered-source total power; distributed across cells by the<br>matching entry in `q_fraction_vector`. |
| `q_fraction_vector` | `Vec<Vec<f64>>` | Parallel to `q_vector`: per-cell distribution fraction for the same<br>source (need not sum to 1). |

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
  Bulk mass flowrate \[kg/s\] (plain storage — see the field's doc comment

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
  pub fn set_inlet_enthalpy(self: &mut Self, h: AvailableEnergy) { /* ... */ }
  ```
  Prescribes a fixed inlet specific-enthalpy boundary condition on

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
  pub fn correct_thermo(self: &mut Self) { /* ... */ }
  ```
  Update the thermodynamic and transport state from the current

- ```rust
  pub fn step(self: &mut Self) { /* ... */ }
  ```
  Advance one time step with the compressible PIMPLE algorithm.

- ```rust
  pub fn run(self: &mut Self, n_steps: usize) { /* ... */ }
  ```
  Advance `n_steps` time steps of size `delta_t`.

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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> TampinesSteamArray { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Freeze**
- **From**
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
### Re-exports

#### Re-export `TampinesSteamArrayError`

```rust
pub use lateral_coupling::TampinesSteamArrayError;
```

## Re-exports

### Re-export `TampinesSteamArray`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray;
```

### Re-export `TampinesSteamArrayError`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::TampinesSteamArrayError;
```

