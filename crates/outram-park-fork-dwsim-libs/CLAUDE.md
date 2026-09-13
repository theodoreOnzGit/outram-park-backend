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

## Maturity

**Declared mature by the maintainer on 2026-09-13.**

- 2026-09-13 — mature. **Bar:** flash and column results agree with upstream
  DWSIM at the pinned commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` on the
  same input, to 4 significant figures. **Evidence class:** cross-code
  comparison.
  **NOT YET MEASURED.** No run has been performed against this bar. `dotnet`
  and `mono` are absent from the development container, so upstream DWSIM
  cannot be *executed* — only read. This is recorded as the standard the crate
  is held to, not as a result it has met.

**What was actually measured at declaration time** (2026-09-11, release), and
what a reader should treat as the real current evidence:

| check | result |
|---|---|
| PR-EOS density vs `outram-park-fork-coolprop` Helmholtz EOS | CO₂ 400 K/5 MPa +1.03 %, 400 K/10 MPa +1.00 %; N₂ 300 K/10 MPa +1.68 %, 200 K/5 MPa +2.28 % |
| dwsim-libs PR vs `tampines-steam-tables` PR | agree to 4 significant figures at all four points |
| MESH column per-component material balance | worst relative imbalance 5.55e-6; bypassed components 1.7e-16 |
| Refluxed-absorber bottom-stage energy residual | −2.87e-9 W (β = 1), +2.68e-11 W (β = 0.5) |
| Wang-Henke ⟷ Naphtali-Sandholm cross-check at NS's D | Q₇ −0.018785 W, profiles agree to 0.001 K |
| Test suite | 705 lib, 18 doc, 11 integration, 0 failing |

**Known gaps at declaration, recorded so a later reader is not misled:**

- **0 of 107 rows in `docs/upstream-port-coverage.md` are `PORTED + VALIDATED`.**
  Nothing in the crate is backed by agreement with an analytical result, a
  published value, or an external code. The three closest are single
  pure-component spot checks.
- Compound coverage is **seven** presets, only two of which
  (benzene, toluene) carry real ideal-gas Cp coefficients.
- Six `MISSING + REQUIRED` items remain, including material-stream property
  calculation (#176) and unit-op registry wiring; unit outputs are not
  composable streams.
- `Stage::heat_duty`'s documented sign convention contradicts its use.

**Consequence of this declaration:** the workspace `CLAUDE.md` "dogfood the API
on a small model" rule is now a **hard rule** for this crate. GitHub #72's
Haiku run must be repeated against the current API and its friction log acted
on; it is no longer optional.

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
