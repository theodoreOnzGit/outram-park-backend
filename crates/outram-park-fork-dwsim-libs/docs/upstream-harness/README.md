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
