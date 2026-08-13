# Crate Documentation

**Version:** 0.2.5

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
  flux) and [`check_ps_envelope`]. Note its pressure floor is
  **exclusive** where the `(p,h)` one is inclusive: the `(p,s)` validity
  check evaluates `s_tp_eqm_single_phase` on the 273.15 K isotherm and
  so hits the Region-4 trap at exactly `p = p_sat(273.15 K)`.
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
| `pt_flash_eqm/mod.rs:198` "entropy: two-phase (T,p) ... under-determined" | reached *inside* `is_below_isotherm_t_273_15`, which evaluates `s_tp_eqm_single_phase(273.15 K, p)`; the `(T,p)` router returns Region 4 when `p == p_sat(273.15 K)` **bit-for-bit** | **exclusive** pressure lower gate (`p > p_sat(273.15 K)`, not `>=`) |
| `boundaries_between_single_phase_regions.rs:20,23,60,63,106` "p in (p,s) point is outside validity range" | `p` below 16.529 MPa / above 100 MPa inside the `p >= 16.529 MPa` branch | unreachable: that branch is only entered when `16.529 MPa <= p <= 100 MPa` already holds |
| `boundaries_from_single_phase_regions_to_region_4_multiphase.rs:30,33,75,78` "entropy of p,s point is outside validity range" | `s` outside the Region-3/4 boundary entropy band | unreachable inside the gate: the Region-1 and Region-2 tests immediately above them already returned for every `s` outside that band |
| `boundaries_from_single_phase_regions_to_region_4_multiphase.rs:123,151` "pressure of p,s point is outside validity range" | `p` outside `[p_sat(273.15 K), p_sat(623.15 K)]` in the `p < 16.529 MPa` branch | pressure gate plus the branch condition |
| `ps_flash_eqm/mod.rs:64,154` `todo!("region 5 ps flash not implemented")` | `ps_flash_region` returning `Region5` | unreachable: `ps_flash_region` has no code path that returns `Region5` |

The last four rows are "unreachable" arguments rather than direct gates,
so they were checked empirically as well — see the V&V note in
[`super::tests`]: a 273 609-point sweep of the accepted set found zero
surviving panics across all ten wrapped functions.

## The `p == p_sat(273.15 K)` trap (differs from the `(p,h)` facade)

The `(p,h)` validity check dodges this trap deliberately by evaluating
its lower enthalpy bound with the Region-1 forward equation `h_tp_1`
instead of the `(T,p)` router (see the note in
`ph_flash_eqm/validity_range.rs`). The `(p,s)` check does **not** — it
calls `s_tp_eqm_single_phase(273.15 K, p)`, which routes to Region 4 and
panics at exactly `p = p_sat(273.15 K)`. The `(p,s)` pressure floor here
is therefore **exclusive** where the `(p,h)` one is inclusive. This is a
defect in the internals, not in IF97 (the triple-point isobar is a
perfectly good IF97 state); it is recorded as a follow-up rather than
papered over.

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

- `p` — absolute pressure, Pa. Valid in the **half-open** interval
  `(p_sat(273.15 K), 100 MPa]`, i.e. strictly above the triple-point
  saturation pressure (about 611.213 Pa) and up to and including
  100 MPa. The lower edge is exclusive because of the Region-4 trap
  documented at module level; the upper edge is inclusive, matching
  `is_outside_pressure_range`, which rejects only `p > 100 MPa`.
- `s` — specific entropy, J/(kg K). Valid between the 273.15 K and
  1073.15 K isotherm entropies **at that pressure**, both edges
  inclusive. Both bounds are evaluated with the same
  `s_tp_eqm_single_phase` call the internal check uses, so the accepted
  set is identical rather than merely similar.

Non-finite input is rejected first with
[`SteamTablesError::NonFinite`].

```rust
pub fn check_ps_envelope(p: Pressure, s: SpecificHeatCapacity) -> super::Result<()> { /* ... */ }
```

#### Function `try_t_ps_eqm`

Checked temperature T (K) from a `(p,s)` flash (Regions 1-4; Region 5
is not implemented for this flash path and lies outside the entropy
window). Valid range: the `(p,s)` envelope in
[`check_ps_envelope`]. Agrees exactly with [`t_ps_eqm`] for in-range
input.

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
    fn clone(self: &Self) -> SteamTablesError { /* ... */ }
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
    fn eq(self: &Self, other: &SteamTablesError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
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
[`TampinesSteamArray::step`], `assemble_hybrid_dissipation`, …) is real and
cited exactly.

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

## 7. The all-Mach hybrid ([`SolverMode::HybridAllMach`])

A pressure-based solver is superb at low Mach but **rings** at a sharp,
near-sonic front: its central (non-upwinded) flux has no numerical dissipation
to damp the shortest wavelengths, so a steep flashing front develops
Gibbs-like oscillations. A density-based KNP scheme (Kurganov–Noelle–Petrova
central-upwind, the `rhoCentralFoam` flux) has exactly the right dissipation
for a shock — its `a_L·a_R·(W_R − W_L)` jump term is an upwind viscosity keyed
to the local wave speeds `a = U_n ± c`. The hybrid keeps the pressure-based
solver everywhere and **borrows only the KNP jump term as a deferred-correction
dissipation**, switched on continuously by a Mach-blend weight:

> `β(Ma) = clamp((Ma − lo)/(hi − lo), 0, 1)`   ([`central_upwind::mach_blend`],
> defaults `lo = 0.3`, `hi = 1.0`).

Subsonic faces get `β = 0` and see **identically zero** added flux, so
[`SolverMode::Pimple`] stays bit-for-bit the validated path; only near-sonic
faces (the flashing front) receive the shock-capturing damping. The dissipation
is `β·(knp − central)·|Sf|` — the pure KNP jump term — assembled per face in
[`TampinesSteamArray::assemble_hybrid_dissipation`] and injected into
continuity (folded into `self.phi`) and momentum (a deferred per-cell source);
energy shock-capturing rides implicitly on the continuity flux through the
EEqn's `∇·(φh)`, so no separate — destabilising — energy source is added (see
`HybridDissipation`).

Three details make it work on *this* fluid:

- **The characteristic speed must be the HEM *equilibrium* sound speed**, not
  the frozen Wood–Wallis two-phase speed. The wave speeds `U_n ± c` and the
  Mach number both use [`central_upwind::hem_sound_speed_ph`], which in the
  dome takes the Kieffer equilibrium speed
  [`crate::region_4_vap_liq_equilibrium::w_ps_eqm_region4_kieffer`] (entropy
  from the `(p,h)` flash into Kieffer eq. 28). The frozen speed would put the
  characteristics in the wrong place because it ignores interphase mass
  transfer — the very flashing this solver is about.
- **Blend on `min(Ma_owner, Ma_neighbour)`, not `max`.** At a
  liquid/two-phase interface the liquid side's `c ≈ 1400 m/s` makes the KNP
  viscosity `~c_liq/2` enormous, but that liquid acoustic wave is genuinely
  *low Mach* and must not be dissipated. `min(Ma)` sees the subsonic liquid
  side and returns `β = 0`, activating dissipation only where *both* sides are
  near-sonic — the fully-developed two-phase front where `c` is uniformly small
  and the damping is physical.
- **A rarefied-tail density taper** scales `β` to zero below a mixture-density
  floor (`HYBRID_RHO_TAPER_LO`/`HYBRID_RHO_TAPER_HI`, 50–100 kg/m³). As the
  pipe empties toward vacuum the HEM closure degrades and there is no shock to
  capture; an explicit dissipation on a nearly-empty cell would tip it across
  the `(p,h)` 273.15 K validity edge and panic. The taper is inert over the
  physics window (the front sits at `ρ ≳ 106 kg/m³`), so the ~55 % ringing
  reduction and the ≈ 388 psia plateau are unchanged (bug op-21g.15.7).

See the `central_upwind` module for the KNP flux math and the `FaceState`
reconstruction.

---

## Where to read next

- [`TampinesSteamArray::step`] — the timestep loop; the block comments there
  annotate every equation cited above.
- [`TampinesSteamArray::correct_thermo`] — the `(p,h)` EOS closure and the
  `ψ = ∂ρ/∂p|_h` finite difference.
- `central_upwind` — the KNP central-upwind flux and HEM sound speed.
- Stability failure modes (BC well-posedness, pressure-source clobbering,
  water-hammer, pressure bounding) are walked through in the appbuilder crate's
  `docs/stability_a_students_guide.md`, which applies here verbatim.

C++ reference (reproduced verbatim below for provenance):
`applications/solvers/compressible/rhoPimpleFoam/`.

```rust
pub mod rhoPimpleFoam { /* ... */ }
```

### Types

#### Enum `SolverMode`

Selects the flux discretisation used by [`TampinesSteamArray::step`].

This is an **opt-in** switch (enum dispatch — no trait objects, per the
workspace design rules). The default [`SolverMode::Pimple`] runs the
pressure-based compressible PIMPLE algorithm exactly as before, bit-for-bit
(the recent Edwards flashing-plateau fix and every existing test are
preserved by construction). [`SolverMode::HybridAllMach`] additionally
injects a **Mach-weighted KNP central-upwind dissipation** (see the
`central_upwind` module) as a deferred-correction flux, active only on
near-sonic faces (`β(Ma) > 0`), to damp the ringing at a near-sonic flashing
front while leaving subsonic regions untouched.

[`SolverMode::HybridAllMach`] damps the near-sonic ringing (~55 % less excess
total variation over 0–0.15 s) while retaining the Edwards flashing plateau
(≈ 388 psia). It is **stable over the full 600 ms transient**: the earlier
late-time instability (an emptying-pipe near-sonic cell driven across the
`(p,h)` 273.15 K validity edge past t ≈ 0.18 s, bug `op-21g.15.7`) is fixed by
the rarefied-tail density taper on the KNP dissipation — see
[`TampinesSteamArray::assemble_hybrid_dissipation`]. The default
[`SolverMode::Pimple`] remains bit-for-bit the historical validated path.

```rust
pub enum SolverMode {
    Pimple,
    HybridAllMach,
}
```

##### Variants

###### `Pimple`

Pressure-based compressible **HEM-closed PIMPLE** — the historical,
validated, default path (recovers the Edwards flashing plateau; stable
over the full 600 ms transient).

###### `HybridAllMach`

PIMPLE + Mach-blended KNP shock-capturing dissipation (all-Mach hybrid).
Damps the near-sonic ringing at the flashing front (~55 %) while retaining
the flashing plateau, and is stable over the full 600 ms Edwards transient
(rarefied-tail density taper, bug `op-21g.15.7`). The default
[`SolverMode::Pimple`] is the bit-identical historical path.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SolverMode { /* ... */ }
    ```

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
    fn default() -> SolverMode { /* ... */ }
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
    fn eq(self: &Self, other: &SolverMode) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    pub n_outer_correctors: usize,
    pub n_inner_correctors: usize,
    pub p_under_relaxation: uom::si::f64::Ratio,
    pub u_under_relaxation: uom::si::f64::Ratio,
    pub p_min: uom::si::f64::Pressure,
    pub p_max: uom::si::f64::Pressure,
    pub mode: SolverMode,
    pub ma_blend_lo: uom::si::f64::Ratio,
    pub ma_blend_hi: uom::si::f64::Ratio,
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
| `mode` | `SolverMode` | Flux-discretisation mode (default [`SolverMode::Pimple`], bit-identical<br>to the historical path). See [`Self::set_solver_mode`]. |
| `ma_blend_lo` | `uom::si::f64::Ratio` | Lower Mach threshold `lo` of the hybrid blend window<br>`β(Ma) = clamp((Ma−lo)/(hi−lo), 0, 1)` (default `0.3`, dimensionless).<br>Below `lo` the KNP dissipation is identically zero. Only read when<br>`mode == HybridAllMach`. See [`Self::set_mach_blend_window`]. |
| `ma_blend_hi` | `uom::si::f64::Ratio` | Upper Mach threshold `hi` of the hybrid blend window (default `1.0`,<br>dimensionless). At/above `hi` the KNP dissipation is applied at full<br>weight. See [`Self::set_mach_blend_window`]. |
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
  pub fn correct_thermo(self: &mut Self) { /* ... */ }
  ```
  Update the thermodynamic and transport state from the current

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TampinesSteamArrayError> { /* ... */ }
  ```
  Advance one time step with the compressible PIMPLE algorithm.

- ```rust
  pub fn step(self: &mut Self) { /* ... */ }
  ```
  Advance the solution by the array's stored [`Self::delta_t`].

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

- ```rust
  pub fn get_solver_mode(self: &Self) -> SolverMode { /* ... */ }
  ```
  The current flux-discretisation mode (see [`SolverMode`]).

- ```rust
  pub fn set_solver_mode(self: &mut Self, mode: SolverMode) { /* ... */ }
  ```
  Selects the flux-discretisation mode. [`SolverMode::Pimple`] (the

- ```rust
  pub fn get_mach_blend_window(self: &Self) -> (Ratio, Ratio) { /* ... */ }
  ```
  The current hybrid Mach-blend window `(lo, hi)` (dimensionless Mach

- ```rust
  pub fn set_mach_blend_window(self: &mut Self, lo: Ratio, hi: Ratio) { /* ... */ }
  ```
  Sets the hybrid Mach-blend window `β(Ma) = clamp((Ma−lo)/(hi−lo), 0, 1)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
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

### Re-export `SolverMode`

Re-export of the 1-D compressible PIMPLE pipe array solver
(`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
[`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
convenience. `TampinesSteamArray` backs each finite-volume cell with an
IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.

```rust
pub use openfoam_algorithms::rhoPimpleFoam::SolverMode;
```

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

