# CLAUDE.md — outram-park-fork-dwsim-libs

Pure-Rust port of DWSIM thermal-hydraulics and thermodynamics kernels.

The reference source lives at (STRICTLY READ-ONLY, pinned):
`/home/teddy0/Documents/research/dwsim-upstream/`
(branch `windows`, commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`,
cloned 2026-07-17). Do **not** use the older, stale clone at
`/home/teddy0/Documents/research/dwsim/` — ports since 2026-08 cite the
pinned `dwsim-upstream` commit in their attribution headers.

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
cargo check -p outram-park-fork-dwsim-libs --lib
cargo test  -p outram-park-fork-dwsim-libs --lib --release
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

### Dispatch: enums, not trait objects (corrected 2026-07-13)
An earlier draft of this note sketched `PropertyPackage`/`FlashAlgorithm` as
`dyn Trait` interfaces (mirroring DWSIM's own OO interface hierarchy). That
violates the workspace's mandatory "no trait objects" rule (root `CLAUDE.md`,
"Rust design rules"). A broad thermodynamics tier is now ported — the flash
family (VLE / VLLE / LLE / SLE / SVLLE / inside-out / single-component), Gibbs
speciation, the electrolyte tier, the advanced EOS (PR78 / PRSV2 / LKP /
PR+Lee-Kesler), property packages, and the reactions/reactors layer (see
`docs/chemistry-model-survey.md` for per-model status and `docs/port-scope.md`
for the remaining tail) — and it uses enum dispatch as planned here, e.g.:
```rust
pub trait PropertyPackage {
    fn flash_pt(&self, z: &[f64], p: f64, t: f64) -> FlashResult;
    // ... (trait still useful as a compiler-enforced contract per model)
}

pub enum PropertyPackageModel {
    Ideal(IdealPropertyPackage),
    PengRobinson(PengRobinsonPropertyPackage),
    Srk(SrkPropertyPackage),
    // ...
}
// impl PropertyPackage for PropertyPackageModel by match-dispatching to
// the wrapped struct's own impl, not `&dyn PropertyPackage`.
```
This is realized by `thermo::property_package::PropertyPackageModel`
(`Ideal` / `PengRobinson` / `Srk`), and the equipment-model correlations follow
the same pattern: e.g. `pipe::PipeFlowCorrelation`,
`pump::modes::PumpSpecification`, and `separator::SeparatorMode` are enums, not
trait objects.
