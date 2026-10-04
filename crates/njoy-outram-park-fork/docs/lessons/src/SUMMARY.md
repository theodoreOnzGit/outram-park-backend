# Summary

[How to read this deep dive](./intro.md)

# Part I — the core rungs

- [Rung 0 — Where does σ(E) come from?](./where-sigma-comes-from.md)
- [Rung 1 — ENDF: what the evaluator hands us](./endf.md)
- [Rung 2 — RECONR: from resonance parameters to σ(E)](./reconr.md)
- [Rung 3 — BROADR: what temperature does to σ(E)](./broadr.md)
- [Rung 4 — UNRESR and PURR: the unresolved range](./purr.md)
- [Rung 5 — THERMR and LEAPR: thermal scattering](./thermr.md)
- [Rung 6 — HEATR and GASPR: heating, damage and gas](./heatr.md)
- [Rung 7 — GROUPR and GAMINR: multigroup constants](./groupr.md)
- [Rung 8 — ERRORR and COVR: how sure are we?](./errorr.md)
- [Rung 9 — ACER: what the transport code reads](./acer.md)

# Part II — extended deep dives

- [Other codes' formats](./output-formats.md)
- [OpenMC interchange: HDF5 and XML](./openmc-interchange.md)
- [Windowed multipole and the GPU path](./wmp.md)
- [The consumer surface](./consumer-surface.md)

# Appendices

- [Coverage table](./coverage.md)
- [Re-measurement log, 2026-10-04](./remeasured.md)
- [Call trees](./call-trees.md)
