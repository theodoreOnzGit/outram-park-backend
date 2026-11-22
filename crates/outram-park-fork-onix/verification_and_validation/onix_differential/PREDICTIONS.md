# Predictions, written BEFORE any comparison run (gh:#1404, 2026-11-13)

Committed ahead of the first comparison so it can fail. Upstream: ONIX
`jlanversin/ONIX` commit `7328dc6acf705827470819a4367ef2a98c642b74` (MIT), cloned
over git into an empty scratch directory, run with `python -I` from a venv
(numpy 2.4.6, Python 3.11) with only `numpy matplotlib networkx uncertainties scipy
pandas` installed. The driver lives outside the clone.

**Disclosure of timing.** Two things were observed before this file was written, while
checking that the clone imports. First, `onix/__init__.py` imports `onix.cell`,
`onix.couple` and `onix.data.read_lib_functions`, all of which `import openmc`, so
"the stand-alone path needs no OpenMC" (issue text) is false for the package import
(OpenMC is only *called* in the coupled path; the driver stubs the module). Second,
`onix.salameche.cram.CRAM16` builds `theta`, `alpha` as `np.complex256` and hands the
`complex256` matrix to `np.linalg.solve`, which raises
`TypeError: array type complex256 is unsupported in linalg` on numpy 2.4.6. P1 below is
therefore a record of that, not a prediction; P2 onward are written before any
numerical comparison ran.

What I read before predicting (source, not a run): `salameche/cram.py`,
`salameche/mat_builder.py`, `salameche/burn.py` `burn_microstep`, `passport.py`
(`_set_xsparent`, `_set_decayparent`, `_get_*_prod_*_dic`), `data/list_and_dict.py`
(`xs_prod_*`, `decay_prod_*` tables), `data/read_lib_functions.py`
(`conv_decay_b_a`, `read_xs_lib` removal sum), and this crate's `reactions.rs`,
`nuclide.rs`, `chain.rs`, `driver.rs`.

| # | claim | predicted outcome |
|---|---|---|
| P1 | upstream `CRAM16` unmodified, current numpy | raises `TypeError` (complex256 in linalg); the comparison needs a documented shim that casts the `solve` arguments to `complex128` (driver-side, upstream file untouched). Observed, not predicted |
| P2 | case 1, `CRAM16` (shimmed) vs `cram16_pfd`, fixed matrices (3-chain, 20-nuclide FP chain with branching, stiff chain lambda 1e-12..1e4 /s), dt = 1 s, 1 d, 30 d | normwise error `max abs(r-o)/max abs(o)` <= 1e-13 for the chains, <= 1e-12 for the stiff chain; NOT everywhere bit-identical (LAPACK `zgesv` pivoting vs this elimination; upstream finishes `alpha_0*N_0 + 2*sum` in `longdouble` and returns `longdouble`). The shim itself (long-double subtraction `At - theta` rounded again to double) is a further ulp source |
| P3 | case 2, burnup matrix `A = B*1e-24*phi + C` vs `DepletionSystem::build_matrix`, ONIX default libraries (`decay_lib_reduced`, `xs_lib`, `fy_lib_reduced`, ENDF/B-VIII-derived, bundled in the MIT repo), `reduced_nucl_set` | (a) decay entries and capture/(n,2n)/(n,p)/(n,a) entries between ground-state nuclides bit-identical; (b) diagonals (upstream sums sigma then scales; port sums scaled rates) and fission-yield entries (`(sigma_f*pct*1e-2)*1e-24*phi` upstream, `(sigma_f*1e-24*phi)*(pct*1e-2)` port) differ by a few ulp, none above 1e-15 relative; (c) **real defects expected in the isomer handling**: the port has no ground->isomer channel (`(n,gamma)X`, `(n,2n)X`, `betanegX`, ...) and keeps `m` unchanged for daughters of an excited-state parent where ONIX returns to the ground state (`X(n,gamma)`, `Xbetaneg`, ...). I expect a non-zero count of entries present upstream and absent (or in the wrong row) in the port, confined to isomers (zamid ending in 1) and their ground-state partners (137 `(n,gamma)X` entries in `xs_lib` alone) |
| P4 | case 3, 10-step constant-flux burn (`A`, `At = A*dt`, `CRAM16`, `CRAM_density_check`: negatives and < 1e-24 to zero) | per-step inventory normwise <= 1e-12 and no step worse than 10x the previous; entries below the 1e-24 floor counted separately: the port's `deplete_multi` does not apply the floor (`clamp_nonnegative` exists but is not called), so those entries differ by up to the floor |

Gap survey additions I expect to make (reading only): `onix/salameche/py_pade.py`
and `onix/nax/functions.py` (2036 lines, nuclear-archaeology tools) are absent from the
issue's table; `(n,t)t` special case (tritium from Li6/Be10); `Passport.fy` setter bug;
`CRAM_reality_check`.

Any observed number outside these is reported as-is with its localisation. None of the
bounds above will be edited after the run.
