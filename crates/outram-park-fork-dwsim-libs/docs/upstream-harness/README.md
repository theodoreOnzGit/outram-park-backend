# Upstream DWSIM headless harness

C# drivers that run **upstream DWSIM** (pinned commit
`1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`) on Linux, so this port can be
compared against it code-to-code rather than against transcribed numbers.

These are *not* part of the crate and are not compiled by `cargo`. They are
kept here because the build environment they need is fiddly to rediscover and
the scratch directory they were developed in does not survive a session.

Build prerequisites and the six required workarounds are documented in the
crate's `CLAUDE.md` under "Running upstream DWSIM headless". Compile each driver
with `mcs` **in the DWSIM output directory** and run it there, because Mono
probes the assembly's own folder:

```bash
cd <dwsim-build>/DWSIM.Thermodynamics/bin/Release
mcs /target:exe /out:d.exe /r:DWSIM.Thermodynamics.dll /r:DWSIM.Interfaces.dll \
    /r:DWSIM.SharedClasses.dll /r:DWSIM.UnitOperations.dll \
    /r:DWSIM.FlowsheetBase.dll /r:CapeOpen.dll  <driver>.cs
LD_LIBRARY_PATH=. mono d.exe
```

| driver | what it exercises | status |
|---|---|---|
| `zpr_driver.cs` | `PengRobinson.Z_PR` directly | **verified** — matches this port to 6 s.f. |
| `eos_driver.cs` | `Calculator.CalcProp` density/Z | **verified** — differs from `Z_PR` by a Peneloux volume translation |
| `flash_driver.cs` | `Calculator.CalcEquilibrium` PT flash | **verified** — differs from this port by the `k_ij` this port cannot apply |
| `column_driver.cs` | `WangHenkeMethod.SolveColumn` | **verified** — converges on a separating case from this port's own initial estimates |
| `cstr_driver.cs` | a real `Reactor_CSTR.Calculate()` on a headless flowsheet | **verified** — matches this port to 6.1e-10 per species once upstream is converged; see below. Case `hetcat_liq` (2026-10-02): Langmuir–Hinshelwood with `CatalystAmount`, 2.4e-13 after the R3 fix (#481) |
| `conversion_driver.cs` | a real `Reactor_Conversion`, reactions from `CreateConversionReaction`, set ranks | **verified 2026-10-02** — single / sequential / parallel bit-identical to ≤ 3.7e-16 after porting rank groups (#477); upstream penalty-scope defect loses 25 % of carbon (#478); `ReactionPhase` not ported (#479). `tests/upstream_conversion_parity.rs` |
| `equilibrium_driver.cs` | a real `Reactor_Equilibrium`, vapour reaction, explicit `ln K(T)` in both codes | **verified 2026-10-02** — ≤ 3.9e-12 (WGS) and ≤ 1.7e-15 (SMR) on mole fractions at `InternalLoopTolerance = 1e-20` (default 1e-3 stops short, #484); fugacity basis fixed (#482); ideal φ costs +0.32 % / +1.1 % at 10 / 30 bar (#483). `tests/upstream_equilibrium_parity.rs` |
| `gibbs_driver.cs` | a real `Reactor_Gibbs` (GibbsMin with BFGS-B, and Lagrange); prints upstream's `g°/RT` and outlet `ln φ` | **verified 2026-10-02** — ≤ 8.5e-6 vs GibbsMin (upstream's penalty leaves a 2e-6 element imbalance), ≤ 3.5e-7 vs Lagrange once its `ln(P/P0)/(RT)` slip is added; Lagrange fails at ≤ 2 bar (#485). Needs `liblpsolve55.so` (below). `tests/upstream_gibbs_parity.rs` |
| `pfr_driver.cs` | a real `Reactor_PFR`, ΔP pinned to 0; prints upstream's per-segment profile | **verified 2026-10-02** — on upstream's own per-segment `Q`: 1.4e-10 / 2.6e-9 / 2.0e-10 (vapour, SMR, LH bed). Upstream integrates only 99 % of each segment when `ΔV/(0.01·ΔV)` truncates (#480). Fixtures in `tests/fixtures/upstream_pfr/`. `tests/upstream_pfr_parity.rs` |

## Column driver

Runs a 10-stage equimolar methane/ethane column at 500 kPa (feed on stage 5,
total condenser, reflux ratio 2.0, bottoms 0.5 mol/s, Peng-Robinson), seeded
with **this port's own initial estimates** so the comparison is solver-to-solver
rather than guess-to-guess. Converges in 69 iterations to 8.927e-8.

The head-to-head result is recorded in `tests/upstream_column_parity.rs`.

Three things had to be true before the solver would run at all, each worth
knowing:

1. **The property package is stateful.** It needs a `CurrentMaterialStream`
   with the compound slate attached; there are 858 back-pointer sites upstream.
2. **The material stream needs a `Flowsheet`**, because `Solve_Internal` calls
   `pp.CurrentMaterialStream.Flowsheet.CheckStatus()`. `FlowsheetBase` is
   `MustInherit` with 12 abstract members, all UI-facing, so a headless stub is
   short — it is in `column_driver.cs`.
3. **Constructing any flowsheet requires native SkiaSharp.** `FlowsheetBase`'s
   constructor builds a `GraphicsSurface`, so `libSkiaSharp.so` must be present
   (from `SkiaSharp.NativeAssets.Linux`, since the `SkiaSharp` package itself
   ships only tizen/android natives). A drawing library is therefore a hard
   dependency of running a *column calculation* headlessly.

`ns` is the **last stage index**, not a count: arrays are `ns + 1` long.

### Choosing an operating point

Not every methane/ethane case converges in this port. 500 kPa / 160 K does;
most points between 1 and 4 MPa fail with a non-finite K-value, and above
methane's critical temperature (190.56 K) there is often no liquid root at all.
The crate's own module-level example — 101 325 Pa, 200 K — puts **both**
components fully in the vapour (K = 58.7 and 2.15), so no distillation is
possible; it is marked `no_run`, which is why that was never noticed.

## CSTR driver (2026-09-25)

Builds a real flowsheet — inlet, outlet and energy streams connected to a
`Reactor_CSTR`, a kinetic reaction set on a `MolarConc` basis, isothermal,
Peng-Robinson — and calls `Calculate()`. Cases are selected by name:

```bash
LD_LIBRARY_PATH=. mono cstr.exe iso_liq        # liquid n-butane -> isobutane
LD_LIBRARY_PATH=. mono cstr.exe iso_gas_mix    # same, all vapour
LD_LIBRARY_PATH=. mono cstr.exe smr            # DOVER's steam-reforming base deck
CSTR_TOL=1e-13 CSTR_MAXIT=100000000 ...        # upstream's own Tolerance / MaxIterations
```

Output is `KEY=value` lines. The head-to-head result, and why the port is
handed upstream's *outlet* `Q`, is in `tests/upstream_cstr_parity.rs`.

Three things worth knowing before using it:

1. **Upstream writes the outlet's mass flow and mole fractions, not molar
   flows.** In single-outlet mode it sets per-compound molar flow from a
   stream molar flow that is still stale (zero); in a real flowsheet the
   solver calculates the outlet next. The driver does the same
   (`outs.Calculate`) and also prints the raw mole fractions.
2. **An all-vapour CSTR returns zero conversion on the pristine build**
   (GitHub issue #326): the initial relaxation step is `ResidenceTimeL/10`,
   and `ResidenceTimeL` is zero with no liquid. `cstr_diagnostic_dt_seed.patch`
   is a one-line diagnostic that seeds the step from `V/Q` instead. **Apply it
   to a build copy only**, build that project into a separate run directory
   (`/p:BuildProjectReferences=false`), and label any number it produces as
   coming from the diagnostic build. `CSTR.vb` is Latin-1, not UTF-8.
3. **Upstream's default `Tolerance = 1e-5` is not converged on stiff cases**
   (issue #326): its loop stops on per-step change, not on the balance
   residual. Pass `CSTR_TOL` and record the value you used.

## Building with `Directory.Build.props` / `.targets` (2026-09-25)

Two of the workarounds in the crate `CLAUDE.md` can be applied without
touching any project file: copy `Directory.Build.props` and
`Directory.Build.targets` from this folder to the **root of the build copy**,
and MSBuild applies them to every project beneath it.

- `.props` sets `GenerateResourceUsePreserializedResources`, references
  `System.Resources.Extensions` (package 8.0.0, `lib/net462`), and references
  Mono's `netstandard` facade, which `DWSIM.GlobalSettings` otherwise fails
  on with BC30652.
- `.targets` drops culture-qualified `EmbeddedResource` items right after
  `SplitResourcesByCulture`, so no satellite assembly is built or copied and
  the missing `AL` task is never reached. Disabling only
  `GenerateSatelliteAssemblies` is **not** enough: the copy step still looks for
  the satellites and fails with MSB3030.

Two other things were needed on 2026-09-25 that the crate `CLAUDE.md` does
not list:

- `SkiaSharp.NativeAssets.Linux` 1.68.2.1 for `libSkiaSharp.so` (linux-x64),
  which the flowsheet constructor needs; and
- running from a directory holding **every** project's `bin/Release`
  output, not just one. FlowsheetBase pulls in `DWSIM.DynamicsManager`,
  `LiteDB` and others that only another project copies locally.

## Building without root (snrsi-arch-desktop, 2026-10-02)

The machine had a `dotnet` SDK (10.0.112) but no Mono and no sudo. Everything
below is user-space; it reproduced the recorded CSTR numbers bit-for-bit
(`X = 0.65770328398504`, `τ_L = 192.14419923492437`, and the frozen 5-species
SMR vector).

1. **Mono from conda-forge.** Fetch the static `micromamba` binary
   (`https://micro.mamba.pm/api/micromamba/linux-64/latest`) and run
   `micromamba create -p <toolchain>/env -c conda-forge mono` (Mono 6.12.0.199).
   Its `lib/mono/4.8-api` is the `FrameworkPathOverride`.
2. **VB runtime** (workaround 5): `libmono-microsoft-visualbasic10.0-cil_4.0.1-3_all.deb`
   from `archive.ubuntu.com/ubuntu/pool/universe/m/mono-basic/`, unpacked with
   `bsdtar`; copy its `Microsoft.VisualBasic.dll` into the env's `lib/mono/4.5/`
   and its GAC folder. Add the `System.configuration.dll` symlink in the env's
   `4.8-api`.
3. **`packages/` was entirely absent** in a fresh clone (not 80 %): all 128
   `id/version` pairs from the 46 `packages.config` files were fetched from
   `api.nuget.org/v3-flatcontainer`, plus `System.Resources.Extensions` 8.0.0
   and `SkiaSharp.NativeAssets.Linux` 1.68.2.1.
4. **Build** `DWSIM.FlowsheetBase/DWSIM.FlowsheetBase.vbproj` (it pulls in
   Thermodynamics and UnitOperations) with `dotnet msbuild ... -m:14
   /p:Configuration=Release /p:RestorePackages=false
   /p:FrameworkPathOverride=<env>/lib/mono/4.8-api
   /p:GenerateSerializationAssemblies=Off /p:GenerateSatelliteAssemblies=false`.
   `RestorePackages=false` is needed, or `.nuget/NuGet.targets` tries to run
   `mono NuGet.exe` and fails. `Directory.Build.props` now takes the
   `netstandard` facade from `$(FrameworkPathOverride)`, so it works for any
   Mono location.
5. **Run directory:** merge every `*/bin/Release` into one folder, add
   `libSkiaSharp.so`, and — for the Gibbs driver's Lagrange path —
   `PlatformFiles/Linux/liblpsolve55.so` from the upstream tree. Put the env's
   `bin` on `PATH`, compile with `mcs` (also reference `DWSIM.GlobalSettings.dll`
   and `DWSIM.MathOps.dll`), and run with `LD_LIBRARY_PATH=. mono <driver>.exe`.
6. **Diagnostic build** (CSTR all-vapour cases, #326): copy
   `DWSIM.UnitOperations` to a sibling folder inside the build copy, apply
   `cstr_diagnostic_dt_seed.patch` with `patch -p2 --binary`, build that
   `.vbproj` with `/p:BuildProjectReferences=false` (no `OutDir`, or the
   project references are looked for there), and copy the resulting
   `DWSIM.UnitOperations.dll` over a copy of the run directory.

Two things to know when writing drivers: upstream's reaction expressions go
through Flee, which **cannot parse `1e-5`** (write `0.00001`); and a PFR with
a catalyst bed applies Ergun, which needs a particle diameter (default 0), so
pin `UseUserDefinedPressureDrop` for an isobaric comparison.
