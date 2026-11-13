# Provenance of the committed PFLOTRAN regression files (gh:#1537)

| Field | Value |
|---|---|
| Files | `default/infiltration1d/infiltration1d.in`, `default/infiltration1d/infiltration1d.regression.gold` (verbatim, unmodified) |
| Project | PFLOTRAN, https://www.pflotran.org, https://bitbucket.org/pflotran/pflotran |
| Commit | `669fbce204af39428020c6b9d74beae363376689` |
| Path upstream | `regression_tests/default/infiltration1d/` |
| Licence | GNU Lesser General Public License v3.0 (LGPL-3.0), copyright the PFLOTRAN authors (PNNL, SNL, LANL, ORNL, LBNL) |
| Date accessed | 2026-11-13, from `https://bitbucket.org/pflotran/pflotran/raw/669fbce.../regression_tests/default/infiltration1d/` |
| Processing | none; the `.regression.gold` file is real PFLOTRAN output, reproduced bit for bit (apart from a wall-time line) by a binary built from the same commit (round 1, `../README.md`) |
| Use | reference input and expected values for `tests/regression_decks_gh1537.rs` (research, education and V&V only) |

LGPL-3.0 permits redistribution of these files with this notice; the crate is AGPL-3.0-only and already translates LGPL-3.0
PFLOTRAN code (see the crate `NOTICE`). The maintainer may prefer to commit only the extracted numbers; recorded as a
deferred decision on gh:#1537.
