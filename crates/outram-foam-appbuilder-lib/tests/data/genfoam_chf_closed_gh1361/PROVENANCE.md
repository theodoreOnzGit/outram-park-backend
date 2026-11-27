# genfoam_chf_closed_gh1361: provenance

AI-written, not human-reviewed. Research, education and V&V only (`RESPONSIBLE_USE.md`).

**Source.** GeN-Foam `652b3da0cc558543b89e880812d910bf122b356c`, GPL-3.0, EPFL, <https://gitlab.com/foam-for-nuclear/GeN-Foam>, tutorial
`Tutorials/featureCases/1D_CHF/imposedPower` (a 17-cell boiling channel with a critical-heat-flux crisis). The mesh, `thermophysicalProperties.*`,
`turbulenceProperties.*`, `g`, `fvSchemes`, `fvSolution`, `controlDict` and the closures in `phaseProperties` are unmodified, except as listed.
Copied 2026-11-26 from the local checkout `/home/user/upstream-build/GeN-Foam-src` (no download). No data beyond the tutorial and, for `lookup_table_chf`,
the table of `../genfoam_chf_gh1223/` (its own `PROVENANCE.md`).

**Why they are not the shipped case, and what was changed.** The shipped cases cannot run in the port (README "Round 10"): through-flow boundary
conditions (gh:#1801), and the structure power models with time profiles. Both variants differ from the shipped `imposedPower` in:

| file | shipped | here |
|---|---|---|
| `0/fluidRegion/U.liquid`, `U.vapour`, `alpha.vapour` | inlet `fixedValue (4.68292412 0 0)` / `zeroGradient`; outlet `pressureInletOutletVelocity`; walls `slip`; alpha `inletOutlet` | all patches `zeroGradient` (the initial `U.liquid` is the shipped `(4.68292412 0 0)`) |
| `0/fluidRegion/p_rgh` | inlet and walls `fixedFluxPressure`; outlet `fixedValue 14739394.95` | inlet `fixedValue 14751394.95` (a 1.2e4 Pa drive), outlet `fixedValue 14739394.95`, walls `zeroGradient` |
| `constant/fluidRegion/phaseProperties` `powerModel` | `powerDensity 5e8` with a `powerTimeProfile` ramp 5e8 to 3e9 and back | `powerDensity 5e9`, **no profile** (constant); the wall crosses the critical heat flux in the first steps |

`lookup_table_chf` additionally changes (so it is a further step from the shipped tutorial):

| file | shipped | here |
|---|---|---|
| `criticalHeatFluxModel` | `constantCHF` `value 1.66e6` | `lookUpTableCHF`, `PitchToDiameter 1.32631578947`, the axes of `PhaseII_Ex2_04_6770` and `#include "chfTables"` (copied next to `phaseProperties`, bit-identical) |
| `contactPartitionModel` of `liquid.structure` | `constant 1.0` | `constant 0.7`, so that the vapour-side `multiRegimeBoilingVapourTRACE` htc is not multiplied by 0 |

This is NOT a validation against the CHF experiments or the shipped tutorial's results: it is a code-to-code comparison on a modified channel.
`T.liquid`, `T.vapour`, `p` and the other files are the shipped ones. The upstream and port runs both use these files (`scripts/chf_closed_run.sh`).
