# NJOY2016 decks for the ACE-audit test tables (GitHub #365)

Each deck is run as NJOY2016 2016.79 (`~/Documents/research/NJOY2016/build/njoy`)
from a directory `target/ace_extra/<name>/`. There, `tape20` is the neutron
evaluation and `tape26` the thermal evaluation, both from `reference-data/endf/`.
ACER writes `tape30`. The tables themselves are regenerable scratch and are
not committed.

| deck | tape20 | tape26 | table | used by |
|---|---|---|---|---|
| `HH2O_iwt2.input` | `n-001_H_001-ENDF8.0-Beta6.endf` | `tsl-HinH2O.endf` | H in H2O, 293.6 K, IFENG = 2 | `thermal_ifeng2_vs_openmc` |
| `HH2O_iwt0.input` | same | same | H in H2O, 293.6 K, IFENG = 1 (skewed) | `thermal_skewed_vs_openmc` |
| `HZrH.input` | same | `tsl-HinZrH-ENDF8.0.endf` | H in ZrH, 296 K, incoherent elastic | `thermal_incoherent_elastic_vs_openmc`, `thermal_mixed_elastic_vs_openmc` |
| `Cgraph.input` | `n-006_C_012-ENDF8.0.endf` | `tsl-crystalline-graphite.endf` | C in graphite, 296 K, coherent elastic | `thermal_mixed_elastic_vs_openmc` |

The Li-7 (ENDF/B-VIII.0) and U-238 (JENDL-3.3) neutron tables used by
`ace_law4_continuum_cosine` are built by
`../openmc_godiva_cross_code/make_ace.sh` (MATs 328 and 9237) into
`target/ace_extra/{Li7,U238J33}/tape24`.

The mixed-elastic, polynomial-NU and lump-less tables are **constructed** from
these by the scripts in `../openmc_inputs/`. No held evaluation produces those
forms; the search is recorded on #365.
