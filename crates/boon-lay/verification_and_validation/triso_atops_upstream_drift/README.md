# TRISO-ATOPS upstream has moved past the commit boon-lay ports (gh:#1426, 2026-11-13)

Research, education and V&V only (`RESPONSIBLE_USE.md`).

## Finding

`src/triso_atops_fork/` ports TRISO-ATOPS `de374c8`. Upstream `origin/HEAD` (`0e87d36`, "TRISO-ATOPS v release
2.2.0", 2026-09-17) is two releases later, with **two physics changes** (`git diff de374c8 0e87d36`, `trisoatops/ChangeLog.txt`):

* **v2.1.0** (2026-06-10): `calculation_functions.release_activity` gains `run_time` and `irrad_time`
  arguments (`accident_case` passes `constants[7]`, `constants[8]`) and a `normalization_factor = irrad_time / run_time`;
  the failure fractions are split into `normal_fraction` / `accident_fraction`; the `condition` mask
  (`norm_op[0]*sum(fractions[:4]) - norm_op[5] > 0`) and its two branches are removed; noble gases and halogens now use
  `fractions[0] + fractions[2]` and `fractions[4]` instead of `fractions[0] + fractions[2] + fractions[-2]`; Ag/Cd now
  have `accident_fraction = 0`.
* **v2.2.0** (2026-09-16): `R_B_fail` applies the silver correction `(lam_Ag110m / lam)^0.5` only to Ag/Pd nuclides
  whose decay constant exceeds Ag-110m's (shorter half-life); upstream de374c8 applied it to every Ag/Pd nuclide.

The port is faithful to `de374c8` and is not wrong against it; it is **behind upstream**. Whether to bump the pinned
commit (which changes the default and every pinned number) is a maintainer decision, deferred on gh:#1426.

## Size of the effect (upstream against itself, de374c8 vs 2.2.0)

`accident_only.py` runs the end-to-end `accident_case` group of `dev/gen_triso_atops_reference.py` (3 scenarios x 6
nuclides, 432 value rows) against whichever upstream checkout sits at `../upstream_source/TRISO-ATOPS`;
`compare_versions.py` compares two runs. Python 3.11, numpy 2.4.6 (venv in scratch, `python -I`). The generator needs
`constants[8]` (irradiation time), which the committed scenarios set to 0 ("unused by accident_case" at de374c8); two
2.2.0 runs are given.

| 2.2.0 run | `nodal_kernel` rows (180) | median / max change | `total` rows (54) | median / max change | graphite (180), dropped (18) |
|---|---:|---|---:|---|---|
| `constants[8] = 0` (`normalization_factor = 0`) | 65 identical | 2.2e-6 / 0.50 | 20 identical | 3.3e-10 / 3.3e-3 | identical |
| `constants[8] = constants[7]` (factor 1) | 65 identical | 0.27 / 0.41 | 20 identical | 2.6e-5 / 3.2e-3 | identical |

(Files: `accident_case_de374c8.csv`, `accident_case_v2.2.0_irrad_zero.csv`, `accident_case_v2.2.0_irrad_eq_runtime.csv`;
the factor-1 run edited line 8 of `ACCIDENT_CONSTANTS` in a scratch copy of the generator.) Nodal kernel activities move by
up to 41-50 % and totals by up to 0.32 %. These scenarios are not a physical run; read them as the size of the
change, not as a prediction for any plant. The 2.0.1 -> 2.2.0 differences in release fractions of the kernel are what a
port of 2.2.0 would have to re-baseline.

## Also checked on this date

* The committed fixture `tests/data/triso_atops_reference.csv` (14 130 cases) **regenerates byte for byte** from a fresh clone
  of `de374c8` in a fresh venv: `python -I dev/gen_triso_atops_reference.py --check` printed `fixture up to date`
  (numpy 2.4.6, pandas 3.1.1 installed but unused by the generator, which stubs `pandas`).
* Upstream at `de374c8` has **no `examples/` directory and no sample run files** (only the source and, then, the manual PDF),
  so the issue's "full run files (examples/ of upstream)" case has nothing to run. Whole-run coverage is the existing
  `accident_case` end-to-end group (432 rows, worst 2.44e-9 on an Ag-110m kernel row, inside its conditioning-widened bound),
  `tests/mhtgr_stoyer_workflow.rs`, and the seven generated run files of `tests/triso_atops_upstream_run_file.rs`.
