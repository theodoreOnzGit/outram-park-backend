# Windowed-multipole generation against OpenMC (gh:#1390)

> Research, education and V&V only. Not for facility operation, licensing or
> safety decisions (`RESPONSIBLE_USE.md`). AI-written and unreviewed.

`src/wmp/generate/` translates OpenMC's windowed-multipole generator
(`openmc/data/multipole.py`, `openmc/data/vectfit.py`, OpenMC
`8f79d9d02483c90e4134f03a9df4b75ce6b062bf`, 2026-11-10, MIT). Gate:
`tests/wmp_generate_vs_openmc.rs`.

## What upstream is

OpenMC replaced the C++ `vectfit` extension (`mit-crpg/vectfit`) with a
pure-Python `openmc/data/vectfit.py` (numpy/scipy); that file is the
specification here, so the C++ code was not needed. The generator's LAPACK
calls are `scipy.linalg.qr` (economic, DGEQRF), `scipy.linalg.lstsq` (DGELSD,
`cond = eps`), `np.linalg.lstsq` (DGELSD, `rcond = eps max(M, N)`),
`scipy.linalg.eigvals` (DGEEV), and the complex matrix product (ZGEMM), all
through OpenBLAS.

## Methodology

**Reference run.** OpenMC `WindowedMultipole.from_endf` (defaults: NJOY error
5e-4, automatic pieces, `from_multipole(search=True)`) on ENDF/B-VIII.0 Mg-26
(`reference-data/endf/n-012_Mg_026-ENDF8.0.endf`, MAT 1231, MLBW, resolved
range 1e-5 eV to 450 keV, six resonances). Environment:

- OpenMC Python package from the clone at
  `crates/outram-mc-libs/upstream_source/OpenMC` (`8f79d9d`), on `PYTHONPATH`;
  only `openmc.data` is used, so no C++ build;
- numpy 2.4.6, scipy 1.17.1 (OpenBLAS), h5py, `endf` 0.1.12, Python 3.11,
  `OPENBLAS_NUM_THREADS=1`;
- NJOY2016.80 (`njoy` on `PATH`), built from this crate's
  `upstream_source/NJOY2016`.

A wrapper (`ref.py`, below) records the input and output of every stage:
the pointwise input to `_vectfit_xs`, the first `vectfit` call's input and
output, the multipole data, the searched library and a fixed
`n_win = 100, n_cf = 5` windowing of the same multipole data.
`to_fixture.py` writes them, as shortest round-trip decimals, to
`mg26_openmc_8f79d9d.txt` (committed, 270 kB).

**Port run.** Each stage of the port takes **the reference's own input to
that stage**, so a difference is charged to the stage that made it. The
whole chain is also run from the tape.

**Instrument for "is this residual the port's fault?"** OpenMC's own
round-off sensitivity: rerun OpenMC's stage on its input perturbed by one ulp
(random sign per value) and measure how far OpenMC moves itself
(`sensitivity.py`). A port that changes the arithmetic (different QR, SVD and
eigen-solver) cannot be expected to agree more closely than upstream agrees
with itself under a one-ulp change.

## Results (2026-11-14)

| stage | quantity | count | port vs OpenMC | OpenMC vs itself, input +-1 ulp |
|---|---|---|---|---|
| front end (RECONR -> GASPR -> ACER -> extraction) | energy grid, elastic, MT=27, AWR, name, E range | 2115 x 3 | **bit-identical** | - |
| one `vectfit` call | poles | 12 | 7.3e-8 relative (0 bit-identical) | 3.2e-9 - 6.2e-9 |
| one `vectfit` call | residues | 24 | 8.1e-4 relative | 1.6e-5 - 2.0e-5 (worst at the real pole 658.6) |
| `_vectfit_xs` (order search, 30 iterations) | pole count | 8 | **identical** | identical |
| `_vectfit_xs` | poles | 8 | 2.1e-3 relative | 2.5e-4 |
| `_vectfit_xs` | residues | 16 | 9.4e-3 relative | 9.3e-4 |
| `_vectfit_xs`, sigma on upstream's 10x test grid vs the pointwise data | points outside `rtol = 1e-3` / `atol = 1e-5` b | 42 282 | port 730, OpenMC 730; worst 4.963e-3 both | - |
| `_windowing`, `n_win = 100, n_cf = 5`, OpenMC's multipole data | windows, spacing, used poles and residues | 100 | **identical** | identical |
| same | curve-fit coefficients | 1200 | 1.6e-7 of each vector's largest entry (0 bit-identical) | 3.2e-7 |
| same | sigma(E, 0 K) of the two libraries | 4000 | worst 5.2e-9 relative | - |
| `from_multipole` search, OpenMC's multipole data | chosen `n_win`, order, windows, used poles | 110 windows, order 2, 4 poles | **identical** | - |
| same | curve-fit coefficients | 660 | 2.1e-9 of scale | - |
| whole chain from the tape | library shape | - | 4 poles / 110 windows / order 2, as OpenMC | - |
| whole chain | sigma(E, T) at 0, 293.6, 1200 K vs OpenMC's library | 2400 | every difference below `atol` = 1e-5 b | - |

Run time on this machine (4 cores, one used): OpenMC 51.7 s for
`vectfit_nuclide` and 149.5 s for the search; the port's whole test, which
fits twice (OpenMC's input, then its own `from_endf` from the tape) and
runs two searches and a fixed windowing, ~10 min.

### Predictions against results

Posted on gh:#1390 before the first run:

1. Front end bit-identical: **held**.
2. One `vectfit` call, 1e-12 to 1e-9: **wrong**. Poles 7.3e-8, residues
   8.1e-4. The residues of the real poles are ill-determined: OpenMC moves its
   own by 2e-5 under a one-ulp input change, and the port changes the
   algorithm, not one ulp.
3. `_vectfit_xs`: same pole count **held**; poles 1e-9 to 1e-6 **wrong** (2.1e-3,
   against OpenMC's own 2.5e-4 sensitivity). The pole relocation is iterated
   30 times per order and the best iteration is kept, which amplifies.
4. `_windowing`: identical windows **held**; curve fit ~1e-10 **wrong** for
   order 5 (1.6e-7, inside OpenMC's own 3.2e-7) and right in magnitude for the
   searched order 2 (2.1e-9). The `a/E + b/sqrt(E) + c + ...` basis is badly
   conditioned at order 5; numpy's SIMD `pow` (which differs from the C
   library's `pow` in the last bit, measured) adds to it.

## Divergences

- **The code-to-code allowance (1e-8) is not met by the fit coefficients**,
  and no change of instrument in the port can meet it: upstream does not meet
  it against itself under a one-ulp perturbation. This is reported for the
  maintainer's decision, not resolved here. The prediction's 1e-8 gates are
  kept unchanged in the ignored test
  `mg26_coefficients_within_code_to_code_allowance`, which fails.
- **A planned gate rested on a false premise.** "Every test point within
  `rtol`/`atol`" assumed OpenMC's search ends with its "ideal" fit. On Mg-26
  it does not: OpenMC's own poles leave 730 of 42 282 points outside (worst
  4.96e-3). The binding gate is therefore "the port's fit misses no more
  points than OpenMC's"; the original form stays in the ignored test.
- **Linear algebra.** An unblocked Householder QR (`DGEQR2` convention) in
  place of OpenBLAS's blocked DGEQRF; DGELSD's QR-then-SVD solve with PETIR's
  Golub-Reinsch SVD in place of divide and conquer; PETIR's balanced Francis
  QR in place of DGEEV, with conjugate pairs put in DGEEV's order (positive
  imaginary part first), which the merge step relies on.
- **Not ported:** logging, the diagnostic figures (`path_out`), pickle I/O,
  and a complex `response_matrix` in `vectfit` (OpenMC's only caller passes a
  real one). User-supplied `orders` are sorted; upstream iterates a Python
  `set`, ascending for these small integers.
- **Upstream edge cases that would crash Python** (no accepted fit, a lone
  complex pole at the end, every pole in a window and still out of tolerance)
  return `NjoyError::WmpData` instead.

## Files

- `mg26_openmc_8f79d9d.txt`: the reference intermediates.
- Scripts (kept here as text so the run can be repeated):
  `ref.py` (reference run), `to_fixture.py`, `sensitivity.py`.

Recipe: create a venv with numpy, scipy, h5py, lxml, pandas, uncertainties
and `endf`; add a stub `openmc-0.15.3.dist-info/METADATA` to its
`site-packages` (OpenMC's `__init__` reads its own version); put the OpenMC
clone on `PYTHONPATH` and `njoy` on `PATH`; run
`python ref.py <tape> ref.json`, then `python to_fixture.py ref.json
mg26_openmc_8f79d9d.txt`.
