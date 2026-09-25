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
| `cstr_driver.cs` | a real `Reactor_CSTR.Calculate()` on a headless flowsheet | **verified** — matches this port to 6.1e-10 per species once upstream is converged; see below |

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
