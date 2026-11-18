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
- `openmc_inputs/ref_u238.py`: the full-run driver, committed before it was run; stopped
  after 12 pieces (`ref_u238_full_run_aborted.log`).
- `openmc_inputs/ref_u238_subset.py`: the reference driver that produced the fixtures
  (embedded below); log `ref_u238_subset_run.log`.
- `openmc_inputs/sensitivity_u238.py`: OpenMC against itself under a 1-ulp input change.
- `openmc_inputs/to_fixture_u238.py`: run directory to fixtures.
- Fixtures `pieces.txt`, `meta.txt`, `fitted_pieces.f64`, `library_prefix.f64` (little-endian f64).
- Run (paths for this machine): `PATH` with NJOY2016.80 `njoy`, `PYTHONPATH` the `8f79d9d`
  clone, venv Python with numpy 2.4.6, scipy 1.17.1, h5py, `endf` 0.1.12, Python 3.11:
  `python ref_u238_subset.py reference-data/endf/n-092_U_238.endf <out> 12 100,200,303`, then
  `python to_fixture_u238.py <out> .`; `python sensitivity_u238.py <tape> <out> 3 5,7,8,100`.

## Plan change, written before the subset run (2026-11-18 11:58 UTC)

The full reference run (`openmc_inputs/ref_u238.py`) was started 11:22 UTC and
stopped at 11:58 UTC, after 12 of 304 pieces (log:
`ref_u238_full_run_aborted.log`; its "piece n" is 1-based, so n-1 below).
Measured on this machine under load (average 10-12 on 4 cores, two threads for
OpenMC):

| piece (0-based) | points | poles | seconds |
|---|---|---|---|
| 0 - 4 | 469, 196, 65, 47, 242 | 3, 3, 5, 4, 5 | 0.0 - 0.1 each |
| 5 | 905 | 5 | 21.8 |
| 6 | 1220 | 6 | 15.6 |
| 7, 8 | 561, 235 | 6, 5 | 2.4, 0.4 |
| 9 | 866 | 5 | **962.4** |
| 10 | 1059 | 6 | **398.8** |
| 11 | 297 | 6 | **157.3** |

A piece whose search never reaches an "ideal" fit runs every order from
`max(2, 2 n_peaks)` to 200 with 30 relocation iterations each, so the cost
per piece is hundreds to a thousand seconds almost regardless of size. The
12 pieces took ~25 min, dominated by three; if the other 292 cost like those
three the total is days. The prediction table's "2 to 10 hours" is not
supported by this measurement (not a precise estimate: only 12 pieces, all at
the lowest energies). Neither OpenMC nor the port fits this session at full
size, so:

- **What is run instead** (`openmc_inputs/ref_u238_subset.py`, unmodified
  OpenMC; only `_vectfit_xs` is wrapped): OpenMC's `vectfit_nuclide` runs end
  to end on the tape, so NJOY, the front end, the 304 piece boundaries and
  slices are upstream's, and every piece's input is checksummed. Upstream's
  `_vectfit_xs` is called unchanged only on pieces 0-11 (the "prefix") and on
  pieces 100, 200 and 303 (middle and high energy, where resonances are
  dense); every other piece gets an empty stand-in without fitting. Their
  poles and residues are saved.
- **Windowing** (`from_multipole(search=True)`) is run by OpenMC and by the
  port on the prefix: the 12 fitted pieces over their own energy range
  (`E_max = (sqrt(E_min) + 12 w)^2`, `w` the 304-piece width), a U-238 library
  of the lowest 12 pieces. It is a windowing differential on U-238 poles, not
  the whole library.
- **Not done, stated now**: the 304-piece fit, the whole-chain pole count and
  window parameters of the full library, and whole-chain sigma(E, T). The
  `#[ignore]`d test `u238_whole_chain_from_the_tape` is the instrument for
  that.
- Predictions for the subset: front end bit-identical in all 304 pieces (as
  above); pole count identical in at least 3 of the 4 pieces beyond the
  prefix and in at least 11 of the 12 prefix pieces; prefix windowing:
  windows, spacing, fit order, used poles identical, curve fit ~1e-7 of scale,
  sigma within rtol/atol at every sampled value. Port time per fitted piece
  3 to 6 times OpenMC's.

## Results (2026-11-18)

Hardware for every timing below: Intel Xeon @ 2.10 GHz, 4 logical cores, 15 GB
RAM, shared with two other agents (load average 8 to 12 early on, falling to
about 4 toward the end); OpenMC with `OMP_NUM_THREADS=2`,
`OPENBLAS_NUM_THREADS=2`; the port single-threaded; `ulimit -v 12582912`.
OpenMC timings therefore carry the load of the time: the same piece 100 fit
took 613 s in the subset run and 256, 241 and 235 s in the sensitivity run
later on a quieter machine. Quote the ratios, not the seconds.

Reference run: `openmc_inputs/ref_u238_subset.py`, log
`ref_u238_subset_run.log`, 11:59 to 13:35 UTC. Fixtures: `pieces.txt`,
`meta.txt`, `fitted_pieces.f64`, `library_prefix.f64` (made by
`openmc_inputs/to_fixture_u238.py`). Gate: `tests/wmp_generate_u238_vs_openmc.rs`.

### Front end (all 304 pieces)

| | OpenMC | port | agreement |
|---|---|---|---|
| grid points, 0 K, error 5e-4 | 609 072 | 609 072 | identical |
| `vf_pieces` | 304 (3 289 peaks) | 304 | identical |
| energy, elastic, MT=27 and MT=18 of every piece (checksums), first and last energy | | | **304 of 304 pieces bit-identical** |
| time | NJOY + `from_njoy` 242 s (probe, loaded) | RECONR+GASPR+ACER 97.7 s, 99.2 s | - |

Prediction (bit-identical): **held**, including for the 0 K deck the U-238
PENDF gate does not cover.

### The fit, piece by piece (15 pieces OpenMC fitted)

Port `vectfit_xs` on OpenMC's own input to it. Pole count identical in
**15 of 15** pieces. Differences are relative (nearest-pole matching; residues
over entries above zero), "sigma" is the cross section each set of poles gives
on the piece's grid points: absolute difference between the port's and
OpenMC's, in barn. "OpenMC vs itself" is `openmc_inputs/sensitivity_u238.py`:
upstream's `_vectfit_xs` rerun on its own input with the cross sections
changed by +-1 ulp (random sign per value; 2 or 3 trials; the unperturbed
rerun reproduces the saved fit exactly, 0 difference).

| piece | points | poles | port - OpenMC: poles | residues | sigma (b) | OpenMC vs itself, +-1 ulp: poles | residues | sigma (b) |
|---|---|---|---|---|---|---|---|---|
| 0 | 469 | 3 | 3.3e-9 | 1.5e-8 | 4.7e-9 | - | - | - |
| 1 | 196 | 3 | 1.5e-12 | 3.6e-12 | 1.3e-11 | - | - | - |
| 2 | 65 | 3 | 6.0e-11 | 9.3e-10 | 4.7e-13 | - | - | - |
| 3 | 47 | 5 | 6.7e-10 | 2.6e-7 | 2.0e-12 | - | - | - |
| 4 | 242 | 4 | 8.7e-12 | 4.8e-10 | 1.3e-12 | - | - | - |
| 5 | 905 | 5 | 8.0e-8 | 1.6e-6 | 3.6e-8 | 3.9e-8, 7.3e-8 | 1.1e-6, 2.0e-6 | 2.6e-8 |
| 6 | 1 220 | 6 | 1.5e-9 | 2.2e-8 | 2.8e-8 | - | - | - |
| 7 | 561 | 6 | 4.2e-10 | 6.1e-5 | 9.5e-7 | 2.0e-9, 4.9e-10 | 5.8e-5, 1.8e-5 | 6.4e-7, 2.7e-7 |
| 8 | 235 | 5 | 2.7e-8 | 8.5e-6 | 2.5e-10 | 2.9e-8, 1.1e-7 | 8.9e-6, 3.4e-5 | 2.5e-10, 9.4e-10 |
| 9 | 866 | 5 | 9.8e-9 | 3.3e-7 | 6.1e-9 | - | - | - |
| 10 | 1 059 | 6 | 2.2e-9 | 2.2e-7 | 6.1e-8 | - | - | - |
| 11 | 297 | 6 | 3.8e-10 | 7.0e-9 | 5.5e-12 | - | - | - |
| 100 | 3 058 | 16 | 1.5e-3 | 1.5e-1 | 2.1e-3 | 2.9e-3, 2.7e-3 | 2.8e-1, 2.8e-1 | 4.1e-3, 4.5e-3 |
| 200 | 5 049 | 34 | 3.0e-2 | 4.1e+1 | 3.9e-2 | 2.8e-2 | 3.6e+1 | 3.6e-2 |
| 303 | 6 623 | 48 | 1.9e-3 | 5.6e-1 | 5.8e-3 | not run | | |

Points outside `rtol = 1e-3` / `atol = 1e-5` b of the pointwise data (on the
grid points): pieces 0-11: 0 for both codes; piece 100: OpenMC 3, port 3;
piece 200: OpenMC 3, port 2; piece 303: OpenMC 21, port 17.

Reading: from piece 100 up (16 to 48 poles) the fit is ill-conditioned enough
that **OpenMC moves itself by as much as the port differs from it** (piece 100:
poles 2.9e-3 against 1.5e-3, sigma 4.5e-3 b against 2.1e-3 b; piece 200: poles
2.8e-2 against 3.0e-2, residues 36 against 41, sigma 3.6e-2 b against 3.9e-2 b),
so these residuals are not a defect to find in the port: they are what a
one-ulp change does to the algorithm. A relative residue difference of 41
(4 100 %) is a residue that is small in magnitude; which pole it is, and its
contribution to sigma, was not examined (the sigma column is the measure that
matters: 3.9e-2 b worst at piece 200, with 3.6e-2 b for OpenMC against itself).
For the 12 small pieces the poles agree within 1e-8 in 10 of 12 (pieces 5,
8.0e-8, and 8, 2.7e-8, are above); the residues are within 1e-8 in only 4 of
12 (pieces 1, 2, 4, 11). The pieces measured against OpenMC itself (5, 7, 8)
show OpenMC moving by the same size as the port differs from it (piece 7
residues 6.1e-5 against 5.8e-5 and 1.8e-5), so the cause there is the same: the
conditioning of the fit and different linear algebra (petir's QR, SVD and
eigensolver in place of LAPACK/OpenBLAS), as for Mg-26. For the other nine
small pieces no self-sensitivity run was made, so their residuals are
unexplained beyond that. No bit-identity was reached anywhere.

### Windowing of OpenMC's own poles (the lowest 12 pieces, 0 to 31.2 eV)

OpenMC `from_multipole(search=True)` against the port's `search_windowing` on
the same 57 poles (19 used): **110 windows, spacing (bit pattern), fit order 5,
used poles and residues identical**; curve-fit coefficients 0 of 1 980
bit-identical, worst 7.1e-6 of each vector's largest entry; sigma(E, T) of the
two libraries at 0, 293.6 and 1 200 K (12 000 values): **0 outside rtol/atol,
worst absolute difference 1.2e-6 b**. Time: OpenMC 32.9 s, port 9.0 s. The
curve-fit residual is 40 times the Mg-26 figure (1.6e-7); order 5 is the
ill-conditioned fixed-order case there too.

### Times

Port against OpenMC, same inputs (OpenMC from the subset run, under load, two
threads; port single thread):

| piece | OpenMC | port | OpenMC / port |
|---|---|---|---|
| 9 | 495 s | 37 s | 13 |
| 10 | 192 s | 40 s | 4.8 |
| 11 | 78 s | 17.5 s | 4.4 |
| 100 | 613 s | 98 s | 6.3 |
| 200 | 1 599 s | 278 s | 5.8 |
| 303 | 2 416 s | 654 s | 3.7 |

### Predictions against results

| prediction | outcome |
|---|---|
| front end bit-identical in all 304 pieces | **held** |
| `vf_pieces`, boundaries identical | **held** |
| pole count identical in >= 90 % of pieces | **held in the 15 fitted pieces (15 of 15)**; 289 pieces were not fitted, so nothing is said about them |
| total pole count identical (about 30 %) | **not testable**: the 304-piece fit was not run (see below) |
| window parameters identical if the pole count is | held for the 12-piece prefix; not testable for the whole library |
| fit coefficients not within 1e-8 | **held** (and for pieces from 100 up, worse than Mg-26: poles 1e-3 to 3e-2) |
| sigma >= 99 % inside rtol/atol, worst <= 1e-2 relative, median ~1e-6 | prefix windowing: 100 % inside, worst 1.2e-6 b. Piece level: worst port-vs-OpenMC relative difference 1.0e-3 (piece 303), 2.8e-3 (piece 200), 7.4e-5 (piece 100); all well inside 1e-2. **held** |
| OpenMC run time 2 to 10 h | **wrong**: ~300 s per piece early on, 613 to 2 416 s for the three high pieces; extrapolated total is of order tens of hours (see below) |
| port 3 to 6 times slower than OpenMC | **wrong in direction**: 3.7 to 13 times faster on the six pieces timed |
| prefix windowing: windows etc. identical, curve fit ~1e-7 | windows etc. identical **held**; curve fit 7.1e-6, **wrong by 40** |

### Gates and verification run

Each agreement above is a gate in `tests/wmp_generate_u238_vs_openmc.rs`: pole
count equal, front end bit-identical, windows/spacing/order/used poles
identical, and every measured difference bounded by its 2026-11-18 value
rounded up in the first figure (`piece_gate`; curve fit 8e-6, windowed sigma
2e-6 b), plus "the port misses no more grid points than OpenMC". Run in
release on this machine after the last edit: the new file's three default
tests (200 s), the ignored `u238_all_fitted_pieces_against_openmc` (1 201 s,
passes), `--lib wmp::generate` (11 passed, including the new
`piece_ranges_cover_the_grid_and_follow_the_piece_rule`), and the Mg-26 file
`wmp_generate_vs_openmc` (2 passed, 781 s), re-run because
`vectfit_pointwise` was refactored to call the new `resolve_vf_pieces` and
`piece_ranges` (no behaviour change; Mg-26 front end bit-identical, 4 poles /
110 windows / order 2, sigma(E, T) 0 of 2 400 outside rtol/atol, as before).
Sensitivity log: `sensitivity_u238_run.log`.

## What was not done

- **The 304-piece fit, hence the whole-library pole count, window parameters
  and whole-chain sigma(E, T) against OpenMC.** OpenMC fitted 15 of 304 pieces
  in 95 min (the three that cost 10 to 40 min each are at 100, 200, 303; the
  early pieces cost ~1 to 16 min each); a rough extrapolation by point count
  puts the full run at tens of hours, and the port, 4 to 13 times faster,
  at several hours. Neither was started. The `from_endf` call and the
  comparison are what `tests/wmp_generate_u238_vs_openmc.rs` lacks; the ignored
  test `u238_all_fitted_pieces_against_openmc` runs what exists.
  This leaves gh:#1390's "pole counts and window parameters identical" shown
  for 15 pieces and the 12-piece prefix windowing, not for the full library.
- Piece 303 has no OpenMC self-sensitivity run (about 40 min per trial).
- The installed OpenMC 0.15.3 (27e38e89) was not used for any fit; see the top.

## Appendix: the reference driver, verbatim

`openmc_inputs/ref_u238_subset.py`, run 2026-11-18 11:59 UTC against OpenMC
`8f79d9d02483c90e4134f03a9df4b75ce6b062bf`
(`crates/outram-mc-libs/upstream_source/OpenMC`), tape
`reference-data/endf/n-092_U_238.endf` (ENDF/B-VIII.0, MAT 9237), NJOY2016.80,
arguments `12 100,200,303`, upstream defaults otherwise (`njoy_error = 5e-4`,
`rtol = 1e-3`, `atol = 1e-5`, `n_vf_iter = 30`, automatic pieces).

```python
"""Reference run of OpenMC 8f79d9d on U-238, fitting a stated SUBSET of the pieces (gh:#1390).

Usage: python ref_u238_subset.py <endf tape> <out dir> <prefix K> <extra pieces, comma separated>
Environment: njoy (NJOY2016.80) on PATH; OpenMC 8f79d9d on PYTHONPATH.

Why a subset: the full `vectfit_nuclide` on this tape was started with
`ref_u238.py` and measured at ~300 s per piece for the first 12 of 304
pieces (see the README), about 25 h in total, which does not fit the session.

What is real and what is a stand-in:
  * Unmodified OpenMC runs `vectfit_nuclide`: NJOY, the front end, the piece
    boundaries and the slicing of all 304 pieces are upstream's.
  * `_vectfit_xs` is wrapped. For every piece it records the size and a
    checksum of the inputs it was given (the front-end gate covers all 304).
    For pieces 0..K-1 and the extra pieces it calls upstream's `_vectfit_xs`
    unchanged and saves poles and residues at once. For every other piece it
    returns an empty stand-in (0 poles) WITHOUT fitting.
  * The prefix library: the first K pieces' multipole data, over the energy
    range of those K pieces (piece width of the full 304-piece split), is
    given to upstream's `WindowedMultipole.from_multipole(search=True)` and
    saved. That is a library of the lowest K pieces only.

Checksum (identical in the Rust test): the f64 words of [energy, row 0, row 1,
row 2] concatenated; h_xor = XOR of all words, h_sum = sum of the words mod
2^64, h_pos = sum of word*(index+1) mod 2^64.
"""
import sys, os, json, time, copy
from math import sqrt
import numpy as np
import openmc.data as d
from openmc.data import multipole as mp

endf_path, out_dir, K, extra = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
fit_set = set(range(K)) | {int(x) for x in extra.split(",") if x}
os.makedirs(out_dir, exist_ok=True)


def checksum(energy, ce_xs):
    w = np.ascontiguousarray(np.concatenate([np.asarray(energy, dtype='<f8')] +
                                            [np.asarray(r, dtype='<f8') for r in ce_xs])).view('<u8')
    idx = np.arange(1, w.size + 1, dtype=np.uint64)
    with np.errstate(over='ignore'):
        return [int(np.bitwise_xor.reduce(w)), int(w.sum(dtype=np.uint64)), int((w * idx).sum(dtype=np.uint64))]


def write_piece(i, p, r):
    buf = [float(i), float(p.size), float(r.shape[0])]
    buf += np.column_stack([p.real, p.imag]).ravel().tolist()
    buf += np.column_stack([r.real.ravel(), r.imag.ravel()]).ravel().tolist()
    np.array(buf, dtype='<f8').tofile(os.path.join(out_dir, f"piece_{i}.f64"))


pieces = []
fitted = {}
upstream_vectfit_xs = mp._vectfit_xs


def wrap(energy, ce_xs, mts, **kw):
    i = len(pieces)
    rec = {"index": i, "n_points": int(energy.size), "e_first": float(energy[0]).hex(),
           "e_last": float(energy[-1]).hex(), "mts": list(mts), "checksum": checksum(energy, ce_xs)}
    if i in fit_set:
        t = time.time()
        p, r = upstream_vectfit_xs(energy, ce_xs, mts, **kw)
        rec.update(fitted=True, n_poles=int(p.size), seconds=time.time() - t)
        write_piece(i, p, r)
        fitted[i] = (p, r)
        print(f"piece {i} FITTED points {energy.size} poles {p.size} seconds {rec['seconds']:.1f}", flush=True)
    else:
        p, r = np.zeros(0, dtype=complex), np.zeros((len(mts), 0), dtype=complex)
        rec.update(fitted=False, n_poles=0, seconds=0.0)
    pieces.append(rec)
    json.dump(pieces, open(os.path.join(out_dir, "pieces.json"), "w"))
    return p, r


mp._vectfit_xs = wrap
t0 = time.time()
mp_data = mp.vectfit_nuclide(endf_path)
t1 = time.time()
n_pieces = len(pieces)
print("vectfit_nuclide seconds (fits of the chosen pieces only)", t1 - t0, "pieces", n_pieces, flush=True)

# prefix multipole data: the first K pieces over their own share of the 304-piece split
width = (sqrt(mp_data["E_max"]) - sqrt(mp_data["E_min"])) / n_pieces
e_max_prefix = (sqrt(mp_data["E_min"]) + K * width) ** 2
prefix = {"name": mp_data["name"], "AWR": mp_data["AWR"], "E_min": mp_data["E_min"], "E_max": e_max_prefix,
          "poles": [fitted[i][0].copy() for i in range(K)], "residues": [fitted[i][1].copy() for i in range(K)]}
json.dump({"name": mp_data["name"], "AWR": float(mp_data["AWR"]).hex(), "E_min": float(mp_data["E_min"]).hex(),
           "E_max_full": float(mp_data["E_max"]).hex(), "E_max_prefix": float(e_max_prefix).hex(), "K": K,
           "n_pieces": n_pieces, "fit_seconds": t1 - t0}, open(os.path.join(out_dir, "meta.json"), "w"))
t2 = time.time()
wmp = d.WindowedMultipole.from_multipole(copy.deepcopy(prefix))
t3 = time.time()
print("from_multipole(search) on the prefix seconds", t3 - t2, flush=True)
lib = [wmp.spacing, wmp.sqrtAWR, wmp.E_min, wmp.E_max, float(wmp.n_poles), float(wmp.data.shape[1]),
       float(wmp.n_windows), float(wmp.fit_order)]
lib += np.column_stack([wmp.data.real.ravel(), wmp.data.imag.ravel()]).ravel().tolist()
lib += np.asarray(wmp.windows, dtype=float).ravel().tolist()
lib += [float(x) for x in wmp.curvefit.shape]
lib += wmp.curvefit.ravel().tolist()
lib += np.asarray(wmp.broaden_poly, dtype=float).ravel().tolist()
np.array(lib, dtype='<f8').tofile(os.path.join(out_dir, "library_prefix.f64"))
json.dump({"n_poles": int(wmp.n_poles), "n_windows": int(wmp.n_windows), "fit_order": int(wmp.fit_order),
           "spacing": float(wmp.spacing).hex(), "from_multipole_seconds": t3 - t2},
          open(os.path.join(out_dir, "library_prefix_meta.json"), "w"))
print("done: prefix poles", wmp.n_poles, "windows", wmp.n_windows, "fit_order", wmp.fit_order, flush=True)
```
