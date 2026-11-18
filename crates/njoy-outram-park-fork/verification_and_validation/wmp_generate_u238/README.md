# U-238 windowed-multipole generation against OpenMC (gh:#1390)

> Research, education and V&V only. Not for facility operation, licensing or
> safety decisions (`RESPONSIBLE_USE.md`). AI-written and unreviewed.

Follows `../wmp_generate/README.md` (Mg-26). That record left U-238, which
gh:#1390 names, undone because OpenMC could not be run here. This one runs it.

## What is the reference (answers gh:#536)

- **Reference library for this differential: OpenMC's own generator, run on
  the same ENDF tape.** Tape `reference-data/endf/n-092_U_238.endf`
  (ENDF/B-VIII.0, MAT 9237, 14 939 396 bytes).
- **Reference code: OpenMC `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`**
  (`crates/outram-mc-libs/upstream_source/OpenMC`), the commit the port
  translates, with its pure-Python `openmc/data/vectfit.py`.
- **The installed OpenMC 0.15.3 (`27e38e894697bb32a1dac7848d2618818b6b8daf`)
  could not be used for the fit.** Its `multipole.py` does `import vectfit`,
  the C++ extension `liangjg/vectfit`. Measured 2026-11-18: `pip download
  vectfit` finds no distribution on PyPI, and the GitHub repository is not
  reachable from this session (codeload answers 403, and the request to attach
  the repository was denied). So 0.15.3's generator cannot run here, and this
  record does not claim anything about it. It runs `8f79d9d`'s Python on the
  0.15.3 venv's numpy 2.4.6 / scipy 1.17.1 / h5py, with `PYTHONPATH` set to the
  `8f79d9d` clone (only `openmc.data` is imported; same recipe as the Mg-26
  record).
- **The #536 pole counts (602 against 4 062) are not the reference here.**
  Both describe the embedded MIT CRPG library the workspace *evaluates*
  (`src/wmp/`), not a library any generator here made. The workspace record
  of 2026-10-05 re-counted the embedded table: 4 062 poles, 4 309 windows,
  fit order 2, `e_max` 2.0e4 eV (`docs/lessons/src/wmp.md`); 602 was the
  stale figure. That table was built by a different tool chain (and
  evaluation) from a different tape, so a generator run on VIII.0 is not
  expected to give 4 062 either; the count that matters is the one OpenMC
  produces from this tape, recorded below.

## Prediction (written and committed before any U-238 run)

Front end measured before the prediction, by running only OpenMC's NJOY step
(`openmc_inputs/probe_frontend.py`): 609 072 grid points from 1e-5 eV to
19 999.99 eV, 3 289 peaks in the total cross section, hence `vf_pieces = 304`
(`max(5, 3289 // 50, 609072 // 2000)`). NJOY plus `from_njoy` took 242 s on
this machine under load (below). Three channels are fitted (elastic, MT=27,
fission).

| quantity | prediction |
|---|---|
| front end (grid, elastic, MT=27, MT=18; per-piece checksum) | **bit-identical** in all 304 pieces. The crate's RECONR+ACER is word for word with NJOY2016 on U-238 at 293.6 K (AGENTS.md), but WMP uses the 0 K deck with no BROADR, which the U-238 PENDF gate does not cover. If a piece differs, that is a front-end defect to find. |
| `vf_pieces`, piece boundaries | identical (follows from the front end). |
| per-piece pole count | identical in **at least 90 %** of the 304 pieces (>= 274). Reason: the order search keeps the best `quality` over 30 relocation iterations x up to ~90 orders, and a flip needs two candidates within round-off. Mg-26 (one piece) did not flip, but here there are 304 searches and thousands of near-ties are possible. |
| total pole count | identical with probability about 30 %. A differing total is most likely a handful of +-2 pole differences, within +-1 % of the total. This is the prediction most likely to be falsified. |
| window parameters (`n_win`, spacing, fit order, poles per window) | identical **if** the total pole count is identical (the `n_win` search grid is `linspace(max(5, n_poles // 20), 8000, 20)`); otherwise they differ because the grid does. A separate stage runs the port's windowing on OpenMC's own multipole data to isolate that: windows, spacing, fit order and used poles **identical**, curve fit ~1e-7 of scale (as Mg-26, conditioning limited). |
| fit coefficients | not within the 1e-8 code-to-code allowance (Mg-26: poles 2e-3, residues 1e-2 over a whole `_vectfit_xs`); expect equal or worse with thousands of poles. Not claimed bit-identical. |
| sigma(E, T) of the two libraries at 0, 293.6, 1200 K, ~2 400 points | the cross sections the poles represent are fixed by the data, so: **>= 99 % of values inside `rtol = 1e-3` or `atol = 1e-5` b**, worst relative difference at a sampled point <= 1e-2, median ~1e-6 or better. If the pole counts differ, the two libraries are two valid fits of the same data and differ at the fitting tolerance (~1e-3) instead. |
| OpenMC run time (`vectfit_nuclide`) | 2 to 10 hours on this machine (Mg-26 took 51.7 s for one 2 115-point piece with 8 poles; here 304 pieces of ~2 000 points, ~11 peaks each, orders from 22 upward). Central estimate 4 h. `from_multipole(search=True)`: 0.5 to 2 h. |
| port run time | 3 to 6 times OpenMC's (Mg-26: port ~4x), i.e. 10 to 40 h for the whole chain. Whole-chain port runs may therefore not fit in one session; if a measured piece time says so, the port is run on a stated subset of pieces and the rest is reported as not done. |

Hardware for every timing: Intel Xeon @ 2.10 GHz, 4 logical cores, 15 GB RAM,
shared with two other agents (load average about 8 when this was written),
`OMP_NUM_THREADS=2`, `OPENBLAS_NUM_THREADS=2`, `ulimit -v 12582912`.

## Files

- `openmc_inputs/probe_frontend.py`: the size probe above (committed verbatim).
- `openmc_inputs/ref_u238.py`: the reference driver (committed verbatim before it was run).
