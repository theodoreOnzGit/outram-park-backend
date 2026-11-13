# Predictions, written BEFORE the fixture rebuild (gh:#1428, 2026-11-13)

Committed ahead of the FLEXPART and puff rebuilds so they can fail. Upstreams, each cloned
over git into scratch and linked into the gitignored-in-spirit `crates/changi/upstream_source/`
(the harness scripts hard-code that path; the links are not committed):
FLEXPART `flexpart/flexpart` at `3d7eebf` (v10.4 line), built with gfortran 13.3.0 on
glibc 2.39; `Hammerling-Research-Group/puff` at `5213d58`, run with R 4.3.3 and dplyr from apt.

**Disclosure of timing.** Before this file was written I had already observed, by running
the committed fixtures through the current tree (merged with `origin/claude/outram-foam-8ookor`,
`cargo test --release -p changi --lib --tests --no-fail-fast`):

1. all 295 tests in 19 binaries pass (116 lib + 179 integration); the "7 failing FLEXPART
   code-to-code tests" of `crates/petir/docs/verification-summary.md` do not reproduce;
2. `Rscript dev/gen_puff_reference.R --check` prints "fixture up to date" (puff regenerates
   byte-identically under R 4.3.3).

I also read `docs/flexpart-code-to-code.md` "PETIR route" and commits `db1335541` and
`11a678aed` before predicting: the seven are `cxy2ll`, `cg2cxy`, `outgrid_init.cell`,
`outgrid_init_nest.cell`, `cbl`, `clustering` and the evaluation half of
`calcpar_inputs_identical_in_both_builds`, and they were closed on 2026-10-20 by
re-derived or maintainer-criterion bounds after changi's transcendentals moved to PETIR,
not by a code change. Everything below about the rebuild and the per-group numbers is
predicted before it was run.

| # | claim | predicted outcome |
|---|---|---|
| P1 | the seven records | they pass because the gates moved (not because the port became bit-exact again): the issue's "all bit-exact vs real-8 build" is stale for 27 groups (the PETIR-route table) |
| P2 | rebuild all 13 stage drivers (26 fixtures) from upstream `3d7eebf` with this gfortran/glibc | the FLEXPART Fortran side is deterministic; fixtures byte-identical except where glibc's libm differs from the committed host's. Prediction: at least 20 of 26 byte-identical; any differing file is a libm-heavy stage (cmapf, physics, vert, met, output) and differs at the last 1-3 digits |
| P3 | per-group `max_rel_dev` (harness lines under `--nocapture`) vs the "PETIR route" table in `docs/flexpart-code-to-code.md` | identical for every group in that table, because the Rust side (PETIR) is host-independent and the fixtures are committed; the only possible movement is through P2 |
| P4 | puff | unchanged (observed, see above) |
| P5 | #559 top clamps, and the end-to-end forward run (cases 2 and 3 of the issue) | not attempted here: case 3 needs a full driver around `FLEXPART.f90` and a random stream shim for the whole run; recorded as deferred, not as passed |

Any observed number outside these is reported as-is with its localisation. None of the
bounds above will be edited after the run.
