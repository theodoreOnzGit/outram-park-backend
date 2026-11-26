# genfoam_chf_gh1223: provenance of `chfTables`

AI-written, not human-reviewed. Research, education and V&V only (`RESPONSIBLE_USE.md`).

| item | value |
|---|---|
| file | `chfTables` (71,082 bytes, sha256 `f654d5dd9cc7970d957b4c5c045931aeead31a324ea0a001356feb97d1a9ee32`) |
| copied from | GeN-Foam `652b3da0cc558543b89e880812d910bf122b356c` (short `652b3da`), `Tutorials/featureCases/1D_PSBT_SC/PhaseII_Ex2_04_6770/constant/fluidRegion/chfTables` (the copy in `PhaseII_Ex3_11-0312_power_increase` is byte-identical: `cmp`) |
| how | `cp` from the local checkout `/home/user/upstream-build/GeN-Foam-src` on 2026-11-26, then `cmp` (identical). Not edited. |
| licence | GPL-3.0, GeN-Foam's own repository (`LICENSE` at its root), <https://gitlab.com/foam-for-nuclear/GeN-Foam>, (C) EPFL. GPL-3.0 is compatible with this workspace (AGPL-3.0-only). |
| content | 15 pressures x 21 mass fluxes x 23 qualities of critical heat flux [kW/m^2]; axes (`pressureValues`, `massFlowRateValues`, `qualityValues`) are in the `criticalHeatFluxModel` dictionary of the case, not in this file. Pressure in kPa, mass flux in kg/(m^2 s). |
| original data | Groeneveld, D.C., et al., "The 2006 CHF look-up table", *Nuclear Engineering and Design* 237 (2007) 1909-1922. **DOI not verified**: believed 10.1016/j.nucengdes.2007.02.014, but doi.org and crossref were not reachable from the build environment on 2026-11-26 (agent proxy refused the hosts), so it was not checked. The publication details are as stated in the maintainer's decision on gh:#1223. |
| decision | Maintainer, 2026-11-26, recorded on gh:#1223: "Port can use table with proper citation. For that matter, they released it in their repo." (Earlier, 2026-11-10: not to be committed until discussed.) |
| used by | `src/genfoam/thermal_hydraulics/closures/heat_transfer/chf_table.rs` (unit tests read it); the closed `1D_CHF` variants with `lookUpTableCHF` (`tests/data/genfoam_chf_closed_gh1361/`) via `#include "chfTables"`. |

Not a validation of the table against experiment: the port reads it as case data and interpolates it as upstream does.
