# `fix property/atom/tracer` and `compute nparticles/tracer/region` (gh:#1418)

Translated in `src/tracer.rs` from LIGGGHTS-PUBLIC `3d5c00f2`
(`fix_property_atom_tracer.cpp`, `compute_nparticles_tracer_region.cpp`).
Test: `tests/tracer_cross_code.rs`; deck `reference-data/liggghts/in.tracer`;
references `tracer_thermo.txt`, `tracer_markers_dump.txt` (built binary, `%.17g`).

## Methodology
`stream_ol` (200 spheres, insertion at steps 1/10001/20001/30001) feeds two
tracers marking a block `rm` (z 0.06..0.12): heaviside (`mark_step 3`, every 7
steps) and dirac (`mark_step 9001`, every 10 steps, marks once at step 9010);
two computes over a block `rc` (`reset_marker yes` -- feeds back into the
tracer -- and `no`). Compared on bit patterns: every thermo column every 1000
steps (`atoms`, 4 vector entries per compute, both `f_tr` scalars) and both
per-atom markers of every atom every 5000 steps (the dump is written before
thermo within a step; the test replays that order).

## Prediction (written before the run)
All bit-identical (exact integer counts, same inclusive `<=` region test,
mass summed in upstream local order via `GranularSystem::local_order_indices`).

## Result (2026-11-11, rustc 1.99.1 release)
**2440/2440 bit-identical**, no difference. Upstream wall time 0.3 s, port 0.5 s.

## Not ported
`compute ... periodic` (image flags), `fix property/atom/regiontracer*`,
non-block/cylinder regions, groups other than `all`.
