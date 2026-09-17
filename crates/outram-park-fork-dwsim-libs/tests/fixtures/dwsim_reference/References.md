# DWSIM reference flowsheets — provenance

Test fixtures for `flowsheet::import`, the read-only importer for DWSIM's saved
flowsheet files. Each file below is an **unmodified copy** of a reference
flowsheet shipped with the DWSIM source distribution, renamed to a
shell-friendly snake_case name. Nothing in these files was edited.

## Source

| Field | Value |
|---|---|
| Project | **DWSIM** — Open Source Process Simulator |
| Author / organisation | Daniel Wagner O. de Medeiros and the DWSIM contributors |
| Repository | <https://github.com/DanWBR/dwsim> |
| Branch | `windows` |
| Commit | `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` |
| Licence | **GPL-3.0** |
| Date accessed | 2026-08-11 |
| Local clone read from | `/home/teddy0/Documents/research/dwsim-upstream` |

## Licence and redistribution

DWSIM is GPL-3.0 and so is this crate (`outram-park-fork-dwsim-libs`,
GPL-3.0-only), so redistributing these files inside this repository is
compatible with their licence. They travel with this `References.md`, which
records their origin; the crate's `NOTICE` and `TRADEMARKS.md` carry the
project-level attribution and the statement that this is an independent OUTRAM
PARK fork, **not** the official DWSIM software.

These are **openly published sample files from a public open-source project** —
no NUS Confidential/Restricted data, no proprietary or partner data, no
operational facility data. They are within the scope the workspace
`DATA_POLICY.md` permits.

## Files

Original paths are relative to `PlatformFiles/Common/` in the upstream tree.

| Committed name | Original path | Size | Why it is here |
|---|---|---:|---|
| `wind_turbine.dwxmz` | `samples/Wind Turbine.dwxmz` | 13 KB | Smallest reference file; a clean-power source exporting a duty to an energy stream. Pins the known `WindTurbine`-cannot-supply-an-energy-stream gap. |
| `wind_turbine_extracted.dwxml` | *derived* — see note below | 72 KB | The plain-XML twin of the file above, for the "a `.dwxmz` imports identically to its extracted `.xml`" test. |
| `three_phase_separator.dwxmz` | `samples/Three Phase Separator.dwxmz` | 26 KB | Vapour/oil/water flash with a converged three-phase split; source of the hand-checked exact stream values. |
| `humid_air.dwxml` | `samples/Humid Air.dwxml` | 178 KB | The smallest **plain** (non-zipped) `.dwxml` in the corpus, so the un-zipped path is exercised on a real file. Also carries two `GO_Text` drawing annotations, which must be skipped rather than imported. |
| `cavetts_problem.dwxmz` | `samples/Cavett's Problem.dwxmz` | 244 KB | The classic recycle-convergence benchmark: 3 recycle blocks, 4 flash vessels, 3 compressors, 3 valves, 2 mixers, 15 compounds. The largest committed fixture (3.7 MB of XML uncompressed). |
| `heating_and_cooling.dwxmz` | `tests/basic/heating and cooling.dwxmz` | 44 KB | Heaters and coolers with their energy streams. |
| `pump_and_valve.dwxmz` | `tests/basic/pump and valve.dwxmz` | 58 KB | Pumps and valves; uses the Lee-Kesler-Plocker package, which this crate does not implement — pins the unsupported-property-package gap. |
| `compression_and_expansion.dwxmz` | `tests/basic/compression and expansion.dwxmz` | 52 KB | Compressors and expanders; the only fixture with **two** property packages (Peng-Robinson and SRK). |
| `basic_distillation.dwxmz` | `tests/basic/basic distillation.dwxmz` | 31 KB | A rigorous distillation column with condenser and reboiler duties. |
| `linde_lng_process.dwxmz` | `tests/Linde_Liquified_Natural_Gas_Production_Process.dwxmz` | 30 KB | Broadest single-file unit-operation coverage: mixer, valve, separator vessel, heat exchanger, compressor, coolers, two recycles. |
| `esterification.dwxmz` | `tests/esterification.dwxmz` | 31 KB | Mixer **and** splitter plus a conversion reactor and a recycle. |

### Processing steps

Ten of the eleven files were copied byte-for-byte with `cp` and only renamed.

`wind_turbine_extracted.dwxml` is the one **derived** file: it is the
`e0958db8-fcf2-4415-99b9-aa92ae1a6c43.xml` member extracted verbatim from
`samples/Wind Turbine.dwxmz` with Python's `zipfile` module, written out with no
transformation of any kind (no re-encoding, no reformatting). Its bytes are
identical to the archive member's decompressed contents. It exists so a test can
prove that the ZIP path adds nothing but decompression.

The `.db` members of the `.dwxmz` archives (small SQLite compound databases) are
carried along inside the archives as shipped; the importer ignores them and
reads compound data from the flowsheet's own `<Compounds>` section.

## What these fixtures are used for

Verification of `flowsheet::import` only — that the importer recovers what each
file says. **No physics is evaluated and no benchmark result is reproduced.**
Cavett's Problem in particular is present as a *parsing* fixture; nothing in
this crate has yet solved it, and no convergence or parity claim is made about
it. Using these files as genuine validation cases — running the flowsheet solver
on them and comparing against the stored converged states — is future work, and
is the reason they were imported in the first place.

## Not redistributed

The upstream corpus holds 175 flowsheet files under
`PlatformFiles/Common/{samples,tests}`; only the eleven above are committed, to
keep the repository small. The corpus-wide coverage census
(`corpus_census` in `tests/flowsheet_import.rs`) runs against a local DWSIM
checkout named by the `DWSIM_REFERENCE_DIR` environment variable and is
`#[ignore]`d by default. The `.dwrsd` (regression-study) and `.dwcsd`
(compound-creator) files in the same directories are a different format and are
out of the importer's scope.
