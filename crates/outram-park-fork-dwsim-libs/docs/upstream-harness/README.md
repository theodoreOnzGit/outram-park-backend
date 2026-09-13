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
| `column_driver.cs` | `WangHenkeMethod.SolveColumn` | **harness works, case not yet meaningful** — see below |

## Column driver: what works and what does not

It runs. A 6-stage methane/ethane column with a reflux-ratio spec on the
condenser and a bottoms-rate spec on the reboiler converges in **18 iterations
to a final error of 4.569e-8**.

**The converged case is degenerate and must not be used as a reference yet.**
Every stage comes out at ~217.4 K — a flat profile, i.e. essentially no
separation. Before this is a real comparison against this crate's MESH solver
it needs a genuinely separating case, which means feeding *both* solvers the
same initial estimates rather than the ad-hoc Wilson-K and linear-temperature
guesses used here.

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
