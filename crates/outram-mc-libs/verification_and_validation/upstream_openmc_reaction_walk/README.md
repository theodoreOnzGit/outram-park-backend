# V&V: OpenMC `scatter()` reaction walk vs the port's `classify_reaction` (gh:#1386, #1444 items a, b)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft, unreviewed.

**Methodology.** `run.sh` checks OpenMC `8f79d9d0` and the SHA-256 of `physics.cpp` and
`random_lcg.cpp`, extracts `physics.cpp:732-775` (the cumulative reaction walk of
`scatter()`) verbatim, and compiles it in `driver.cpp` with mock `Micro`/`Nuc`/`Rx`
and recorder versions of `elastic_scatter`, `sab_scatter`, `inelastic_scatter`.
Four nuclides (heavy with MT 5/16/17/22/51-53/91, light, S(a,b), elastic only) x 400
cutoffs `prn(init_seed(s,0)) * (total - absorption)` from the real OpenMC LCG; the
output (`reference.txt`) is cutoff bits, branch and chosen MT.
Port side: `tests/reaction_walk_vs_openmc_code_to_code.rs`.

**Prediction (before the run).** All 1600 cases choose the same group (elastic /
S(a,b) both map to `Elastic`; MT 5/16/17/22 to their groups; MT 51-91 to
`Inelastic`), and the level, picked by walking the levels with the returned cutoff
using upstream's `>=` rule, is the same MT.

**Not covered.** The extraction is the group/level walk only; the draw order inside
`sample_neutron_reaction` (fission before absorption, item c) and fission-neutron
sampling (item d) are not in this harness. The test exercises `classify_reaction`
and the level rule; the level walk over a real `Nuclide` (`sample_inelastic_at`) is
exercised by the transport tests.
