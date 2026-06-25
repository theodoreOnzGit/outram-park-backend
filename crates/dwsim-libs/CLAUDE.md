# CLAUDE.md — dwsim-libs

Pure-Rust port of DWSIM thermal-hydraulics and thermodynamics kernels.

The reference C# source lives at:
`/home/teddy0/Documents/research/dwsim/`

**Upstream:** DWSIM is LGPL-3.0.  This Rust port is GPL-3.0-only per the
workspace default.

**Language note:** DWSIM is written in C# (primary) and VB.NET (legacy modules).
Files live in a Visual Studio solution (`DWSIM.sln`), targeting .NET 8 on Linux
and .NET Framework 4.6.2 on Windows.  No existing Rust or C bindings.

---

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check -p dwsim-libs --lib
cargo test  -p dwsim-libs --lib --release
```

## Scope — libraries useful for thermal hydraulics

Listed in priority order for porting.  All paths are relative to
`/home/teddy0/Documents/research/dwsim/`.

---

### Tier 1 — Core thermodynamic kernels (port first)

#### 1. Flash algorithms
**C# source:** `DWSIM.Thermodynamics/FlashAlgorithms/` (~25 400 LOC, 23 solvers)

| Solver | C# file | What it does |
|---|---|---|
| Nested Loops VLE | `NestedLoops.vb` | Rachford-Rice flash: two-phase vapour-liquid equilibrium |
| Nested Loops LLE | `NestedLoopsImmiscible.vb` | Liquid-liquid equilibrium |
| Gibbs minimisation 2P | `GibbsMinimization2P.vb` | 2-phase Gibbs energy minimisation |
| Gibbs minimisation 3P | `GibbsMinimization3P.vb` | 3-phase (VLL) Gibbs minimisation |
| Inside-Out (Boston-Britt) | `BostonBrittInsideOut.vb` | Fast inner-outer loop flash |
| SLE | `SLE.vb` | Solid-liquid equilibrium |
| SVLE | `SVLE.vb` | Solid-vapour-liquid three-phase |

Key interfaces: `IFlashAlgorithm` in `DWSIM.Interfaces/`.

**Why port:** Flash is the innermost calculation in every process simulation step.
Porting the Rachford-Rice solver and Gibbs minimisation first enables all
downstream equipment models.

---

#### 2. Property packages — equations of state
**C# source:** `DWSIM.Thermodynamics/PropertyPackages/` (~75 900 LOC, 30+ packages)

| Package | C# file | EOS / model |
|---|---|---|
| Peng-Robinson (PR) | `PengRobinson.vb` | PR EOS: cubic equation for Vm, departure functions for H/S |
| PR + Peneloux volume correction | `PengRobinsonPeneloux.vb` | PR + Peneloux volume translation |
| Soave-Redlich-Kwong (SRK) | `SRK.vb` | SRK cubic EOS |
| SRK + Peneloux | `SRKPeneloux.vb` | |
| Lee-Kesler-Plöcker (LKP) | `LKP.vb` | 3-parameter corresponding-states |
| NRTL (activity coefficients) | `NRTL.vb` | Non-Random Two-Liquid for liquid phase activity |
| UNIQUAC | `UNIQUAC.vb` | UNIQUAC activity-coefficient model |
| UNIFAC | `UNIFAC.vb` | Group-contribution activity coefficients |
| Ideal | `Ideal.vb` | Raoult's law, ideal gas/liquid |
| Extended UNIQUAC | `ExtendedUNIQUAC.vb` | Electrolyte activity |
| Steam tables (IAPWS-IF97) | `SteamTables.vb` | Water/steam properties |
| Seawater | `SeaWater.vb` | Seawater thermophysical properties |

**Why port:** These provide the fugacity coefficients and K-values that drive
flash calculations and all heat/mass balance equipment models.  PR and SRK cover
the vast majority of hydrocarbon applications.

---

#### 3. Pure-component and mixture properties
**C# source:** `DWSIM.Thermodynamics/BaseClasses/PropertyPackageMethods.vb` (~3 500 LOC)
`DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb` (~3 300 LOC)

Provides: enthalpy departure, entropy departure, Cv, Cp, speed of sound,
Joule-Thomson coefficient — all computed from the EOS above.

---

### Tier 2 — Unit operation equipment models

#### 4. Heat exchanger
**C# source:** `DWSIM.UnitOperations/Unit Operations/HeatExchanger.vb` (3 541 LOC)

| Method | Description |
|---|---|
| LMTD | Log-mean temperature difference rating |
| Effectiveness-NTU | NTU method for compact HX |
| Shell-and-tube types | TEMA E/F/G/H/J/K/X shells |
| Double-pipe | Simple co/counter-current |
| Fouling resistance | Dirt factor accumulation |
| Sizing mode | Calculate area from duty |

Dependencies: property packages (H, Cp), flash for two-phase cases.

---

#### 5. Pipe / pipeline pressure drop
**C# source:** `DWSIM.UnitOperations/Unit Operations/Pipe.vb` (3 536 LOC)
`DWSIM.UnitOperations/FluidFlowCorrelations/` (4 modules)

| Correlation | File | Applicability |
|---|---|---|
| Beggs-Brill | `BeggsBrill.vb` | Two-phase horizontal + inclined flow |
| Lockhart-Martinelli | `LockhartMartinelli.vb` | Two-phase horizontal adiabatic |
| Petalas-Aziz | `PetalasAziz.vb` | Two-phase mechanistic (horizontal/vertical/inclined) |
| Single-phase Darcy-Weisbach | (inline in Pipe.vb) | Turbulent single-phase |

Also models heat transfer to surroundings (soil, air, seawater temperature profiles).

---

#### 6. Heater / cooler
**C# source:** `DWSIM.UnitOperations/Unit Operations/Heater.vb` (1 131 LOC)
`DWSIM.UnitOperations/Unit Operations/Cooler.vb` (1 122 LOC)

Simple enthalpy-driven models: given outlet T (or ΔT or Q), compute duty and
outlet stream state via flash.

---

#### 7. Pump / compressor / expander
**C# source:** `DWSIM.UnitOperations/Unit Operations/Pump.vb` (~1 200 LOC)
`DWSIM.UnitOperations/Unit Operations/Compressor.vb` (~1 200 LOC)
`DWSIM.UnitOperations/Unit Operations/Expander.vb` (~1 200 LOC)

Isentropic + polytropic work calculations; efficiency curves; outlet flash.

---

#### 8. Vessel / separator
**C# source:** `DWSIM.UnitOperations/Unit Operations/Vessel.vb` (~2 000 LOC)

Isothermal flash with phase split; knock-out drum sizing; hold-up volume.

---

#### 9. Mixer / splitter
**C# source:** `DWSIM.UnitOperations/Unit Operations/Mixer.vb` (~1 500 LOC)
`DWSIM.UnitOperations/Unit Operations/Splitter.vb` (~1 500 LOC)

Adiabatic mixing (enthalpy balance + flash) and stream ratio splitting.

---

### Tier 3 — Reactor models

#### 10. CSTR / PFR / Equilibrium / Gibbs reactor
**C# source:** `DWSIM.UnitOperations/Reactors/` (13 100 LOC total)

| Model | File | LOC |
|---|---|---|
| CSTR | `CSTR.vb` | 1 611 |
| PFR | `PFR.vb` | 2 274 |
| Gibbs reactor (minimisation) | `GibbsReactor.vb` | 3 028 |
| Equilibrium reactor | `EquilibriumReactor.vb` | 3 798 |
| Conversion reactor | `ConversionReactor.vb` | 1 374 |

Dependencies: property packages, flash, ODE solvers (PFR uses `DWSIM.Math` ODE).

---

### Tier 4 — Advanced EOS (lower priority)

**C# source:** `DWSIM.Thermodynamics.AdvancedEOS/`

| Package | Description |
|---|---|
| PC-SAFT | Perturbed-Chain Statistical Associating Fluid Theory |
| GERG-2008 | ISO 20765 natural gas properties |
| PR + Twu α-function | Temperature-dependent α for heavy components |
| Modified Huron-Vidal (MHV2) | EOS + activity-coefficient mixing rules |

These are numerically intensive but self-contained algebraic models.

---

### Out of scope

- **CoolProp wrapper** (`DWSIM.Thermodynamics.CoolPropInterface/`) — wraps the
  CoolProp C library via P/Invoke; not worth porting (use CoolProp directly from
  Rust if needed)
- **Reaktoro wrapper** — geochemistry library; niche use case
- **Database link modules** (KDB, Chemeo, DDB) — external web service calls
- **GUI layers** — all `DWSIM.UI.*`, `DWSIM.Drawing.*`
- **Flowsheet solver** — `DWSIM.FlowsheetSolver/`; sequential-modular loop
  belongs in a higher-level crate
- **Reporting/export** — CSV, Excel, PDF exporters

---

## Numerical kernel support library

**C# source:** `DWSIM.Math/` (~13 700 LOC, 36 modules)

Key solvers used internally by flash algorithms and equipment models.
Port these alongside or before the modules that need them:

| Solver | Used by |
|---|---|
| Brent root finder | Flash convergence, pipe ΔP |
| Broyden's method | Multi-variable flash convergence |
| Cubic polynomial roots | EOS Z-factor (cubic in Vm) |
| Bilinear interpolation | Heat-transfer coefficient tables |
| Nelder-Mead simplex | Activity-coefficient regression |

Note: `openfoam-basic-lib` already has a `CubicEqn` solver that can be reused
here (`openfoam_basic_lib::polynomial::CubicEqn`).

---

## Design decisions

### Units: raw `f64`, documented
DWSIM uses SI internally (Pa, K, J/mol, kg/m³) for all thermodynamic
calculations but exposes a unit-conversion layer to users.  This port will use
`uom` for **public-facing APIs** (matching the openfoam-basic-lib pattern) and
raw `f64` in the inner EOS arithmetic loops where uom overhead matters.

Documented base units:
| Quantity | Unit |
|---|---|
| Pressure | Pa |
| Temperature | K |
| Enthalpy/entropy | J/mol |
| Density | kg/m³ |
| Viscosity | Pa·s |
| Thermal conductivity | W/(m·K) |
| Molar flow | mol/s |

### Trait hierarchy mirrors DWSIM interfaces
```rust
pub trait PropertyPackage {
    fn flash_pt(&self, z: &[f64], p: f64, t: f64) -> FlashResult;
    fn enthalpy(&self, z: &[f64], p: f64, t: f64, phase: Phase) -> f64;
    fn entropy(&self, z: &[f64], p: f64, t: f64, phase: Phase) -> f64;
    fn density(&self, z: &[f64], p: f64, t: f64, phase: Phase) -> f64;
    // ...
}

pub trait FlashAlgorithm {
    fn flash_pt(
        &self,
        pkg: &dyn PropertyPackage,
        z: &[f64], p: f64, t: f64,
    ) -> FlashResult;
}
```

---

## Porting order

1. Numerical solvers from `DWSIM.Math` (Brent, cubic roots)
2. Ideal gas property package — baseline for testing
3. Peng-Robinson EOS — most widely used; anchors all subsequent packages
4. Nested Loops flash (Rachford-Rice) — simplest flash; uses PR K-values
5. Gibbs minimisation flash — more robust; needed for reactive systems
6. SRK, NRTL, UNIQUAC — other common packages
7. Heater/cooler, pump, compressor — use only flash + H calculation
8. Heat exchanger — LMTD + NTU methods
9. Pipe pressure drop correlations — Darcy-Weisbach, Beggs-Brill
10. Reactor models (CSTR, PFR) — use ODE solver + flash
