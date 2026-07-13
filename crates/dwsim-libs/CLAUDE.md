# CLAUDE.md — dwsim-libs

Pure-Rust port of DWSIM thermal-hydraulics and thermodynamics kernels.

The reference C# source lives at:
`/home/teddy0/Documents/research/dwsim/`

**Upstream:** DWSIM is GPL-3.0 (confirmed against the upstream repository,
2026-07-13 — an earlier version of this note incorrectly said LGPL-3.0).
This Rust port is GPL-3.0-only per the workspace default.

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

## Port scope & order (read on demand)

The prioritised list of which DWSIM C# modules to port (flash algorithms,
property packages / EOS, equipment models, reactors, advanced EOS), the
numerical-kernel support library, what is out of scope, and the bottom-up
porting order all live in **`docs/port-scope.md`**.

## Design decisions

### Units: raw `f64`, documented
DWSIM uses SI internally (Pa, K, J/mol, kg/m³) for all thermodynamic
calculations but exposes a unit-conversion layer to users.  This port will use
`uom` for **public-facing APIs** (matching the outram-foam-basic-lib pattern) and
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
