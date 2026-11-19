# Results (gh:#1428, run 2026-11-13)

Upstreams: FLEXPART `flexpart/flexpart` at `3d7eebf` (gfortran 13.3.0, glibc 2.39), puff
`5213d58` (R 4.3.3, dplyr from apt), each cloned over git into scratch. This crate at the commit that
adds this file, merged with `origin/claude/outram-foam-8ookor`. Predictions:
`PREDICTIONS.md`, committed before the rebuild and not edited.

| # | prediction | outcome |
|---|---|---|
| P1 | the seven recorded failures pass because the gates moved | **held**. `cargo test --release -p changi --lib --tests --no-fail-fast`: 295 passed, 0 failed (116 lib, 179 integration, 18 binaries with tests). The seven were `cxy2ll`, `cg2cxy`, `outgrid_init.cell`, `outgrid_init_nest.cell`, `cbl`, `clustering`, and the evaluation half of `calcpar_inputs_identical_in_both_builds`. Closed on 2026-10-20 by `db1335541` (derived bounds) and `11a678aed` (maintainer criterion, relative difference < 1e-10) after changi's libm moved to PETIR (gh:#985) |
| P2 | at least 20 of 26 rebuilt fixtures byte-identical | **beaten**: all 26 byte-identical (`git status` clean after `dev/build_reference.sh` regenerated every `tests/data/flexpart_*_real{4,8}.csv`), including the libm-heavy stages, although the host's glibc/gfortran differ from the committed ones |
| P3 | every row of the PETIR-route table unchanged | **held**: the 27 real(8) rows I compared (`cnllxy` ... `outgrid_init_nest.cell`, `gfs.out`) match the doc's "after" column in bit-exact count and `max_rel_dev` to the digits printed (e.g. `cxy2ll` 1474/1498, 1.359e-13; `cbl` 7144/7200, 6.307e-12; `clustering` 438/459, 1.822e-13; `outgrid_init.cell` 3018/3094, 1.954e-14) |
| P4 | puff unchanged | **held**: `gen_puff_reference.R --check` -> "fixture up to date"; `puff_code_to_code` 18 passed |
| P5 | #559 clamps and the end-to-end run | **not attempted**, see below |

## What still holds, what has moved

- Holds: the FLEXPART translation agrees with the real(8) build of upstream to the numbers in
  `docs/flexpart-code-to-code.md` "PETIR route"; fixtures regenerate identically; puff regenerates identically.
- Moved relative to the issue text: "bit-exact against a `-fdefault-real-8` build" is no longer the
  right headline. Since 2026-10-20 the port is bit-exact with upstream only for the groups that do
  not call transcendentals (e.g. `get_vdep_prob`, `initialize`, the zgrid/cloud vert outputs, `calcpar`,
  `outgrid_init.oro`); the others sit at the last few ulp (worst relative: `cbl` 6.3e-12, `clustering` 1.8e-13,
  `cxy2ll` 1.4e-13). The PETIR doc's "7 failing tests" statement was stale and is corrected in place.
- No defect found in the port in this pass.

## Not done

- Case 2 of the issue (the two `init_domainfill` top clamps, #559): no case reaches them; unchanged.
- Case 3 (an end-to-end forward run, 6 h, 1e4 particles): needs a Fortran driver around the time loop
  with the `random_mod` stream fed to both sides and met written into `com_mod`; large, not started.
  FLEXPART itself needs GRIB met for a real run, which is not openly available at small size; the
  synthetic-grid route is the only one and remains open.
