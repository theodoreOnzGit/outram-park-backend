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
  **MEASURED 2026-09-13 — BAR NOT MET.** Upstream DWSIM 9.0.5.0 was built
  from the pinned source and run headless on Linux (see "Running upstream
  DWSIM headless" below). Peng-Robinson vapour density, same inputs:

  | case | upstream DWSIM | this port | deviation |
  |---|---|---|---|
  | CO2 400 K, 5 MPa | 73.2995 | 73.554 | +0.35 % |
  | CO2 400 K, 10 MPa | 161.9046 | 163.148 | +0.77 % |
  | N2 300 K, 10 MPa | 111.5136 | 113.603 | +1.87 % |
  | N2 200 K, 5 MPa | 94.0138 | 95.494 | +1.57 % |

  Agreement is 2-3 significant figures, not the 4 the bar demands, so the
  crate **does not currently meet its own bar**. The deviation is systematic
  and one-signed (this port always reads high), which points at a definite
  cause — a differing alpha-function, kij default, or root-selection
  convention — rather than noise. That is a tractable investigation, not a
  rewrite. The declaration stands as the maintainer's decision; this entry
  records that the evidence does not yet support it.

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

## Running upstream DWSIM headless (for code-to-code verification)

Established 2026-09-13. Upstream DWSIM **can** be built and run on Linux from
the pinned commit, which makes the cross-code bar above measurable. The
pinned clone at `/home/user/dwsim-upstream` stays **read-only**; build from a
copy.

```bash
apt-get install -y dotnet-sdk-8.0 mono-complete   # MS CDN is proxy-blocked; Ubuntu's archive works
dotnet msbuild DWSIM.Thermodynamics/DWSIM.Thermodynamics.vbproj \
  /p:Configuration=Release /p:FrameworkPathOverride=/usr/lib/mono/4.8-api \
  /p:GenerateSerializationAssemblies=Off /p:GenerateSatelliteAssemblies=false
```

Six things are required, and none of them touches thermodynamic code:

1. **`packages/` is ~80 % incomplete** (102 of 128 absent). `dist.nuget.org` is
   proxy-blocked, so fetch each `.nupkg` from `api.nuget.org`'s
   `v3-flatcontainer` endpoint at the exact version in `packages.config` and
   unzip to `packages/<Id>.<Version>/`.
2. **`System.Resources.Extensions`** must be referenced by every project that
   embeds resources (49 of them) — .NET Core MSBuild needs it, Windows MSBuild
   does not.
3. **Culture-qualified `.resx` must be dropped** (38 in the thermo project, 3
   in SharedClasses) because the `AL` task does not exist on .NET Core MSBuild.
   All of them are under `EditingForms/` — GUI dialog translations.
4. **Filename case must be fixed** — upstream's Windows branch is not
   case-consistent, so symlink `Steam67.vb`→`STEAM67.vb`,
   `Elements.txt`→`elements.txt`, `JobackGroups.txt`→`jobackgroups.txt`, and
   `System.configuration.dll`→`System.Configuration.dll` in Mono's 4.8-api dir.
5. **Mono's real VB runtime is needed.** Ubuntu 24.04 has no
   `libmono-microsoft-visualbasic10.0-cil`; the `-api` assemblies are
   reference-only and fail at runtime. Fetch the `.deb` from the `mono-basic`
   pool and drop `Microsoft.VisualBasic.dll` into `/usr/lib/mono/4.5/`.
6. Drive it through `DWSIM.Thermodynamics.CalculatorInterface.Calculator` —
   `Initialize()`, then `CalcProp(package, prop, basis, phase, comps, T, P, z)`.
   Compile the driver with `mcs` against the built DLLs and run it **in the
   output directory** (Mono probes the assembly's own folder). Reference
   `CapeOpen.dll`, since `CalcProp`'s signature exposes CAPE-OPEN types.

**Two upstream portability defects found doing this, worth reporting upstream:**
the filename case mismatches above, and `DWSIM.Thermodynamics` holding a direct
project reference to `DWSIM.Controls.DockPanel` (a WinForms docking library), so
the thermodynamics layer is not separable from the GUI.

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
