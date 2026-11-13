# Results (gh:#1404, run 2026-11-13)

Upstream ONIX `7328dc6` run as described in `README.md`; this crate at the commit that
adds this file. numpy 2.4.6 (its bundled OpenBLAS LAPACK), Python 3.11, x86_64 Linux.
Predictions: `PREDICTIONS.md`, committed before the first comparison and not edited.
Gated by `tests/onix_differential.rs` (pins are exact counts and the bounds quoted below).

## Which numbers to quote

Quote the **post-fix** rows. The pre-fix rows are what the unmodified port gave and are
kept as the record of the defect. Per-entry relative error is meaningless at the CRAM
noise floor (the approximant has absolute error ~1e-16, so an entry whose true value is
1e-37 comes back as +-5e-16 on both sides): the bounds below are normwise
(`max|r-o| / max|o|`) and, for entries >= 1e-12 of the largest, per entry.

## Case 1: `CRAM16`, upstream function vs `cram16`

The upstream function, as committed, **cannot run** on this numpy: `cram.py` builds
`theta`, `alpha`, `alpha_0` as `np.complex256` and passes the resulting matrix to
`np.linalg.solve`, which raises `TypeError: array type complex256 is unsupported in
linalg`. Two runnable forms, identical in every one of the 96 outputs:

- **ld**: the upstream function untouched, with `np.linalg.solve` wrapped to cast to
  `complex128` (the long-double `At - theta*I`, `alpha*N_0` and the final
  `2*_N.real + alpha_0*N_0` stay as upstream; the result is `longdouble`);
- **d**: the upstream source text with `complex256` replaced by `complex128`.

| case | dt | bit-identical (of n) | normwise error | worst significant-entry rel. error |
|---|---|---|---|---|
| scalar (A = -1e-3 /s) | 1 s | 1 of 1 | 0 | 0 |
| scalar | 1 d, 30 d | 0 of 1 each | answer underflows (e^-86, e^-2592): both return CRAM noise 3.8e-16 vs -5.1e-16 (|diff| 8.9e-16) | n/a |
| chain3 | 1 s / 1 d / 30 d | 0 of 3 each | 3.59e-15 / 1.78e-15 / 8.99e-15 | same |
| fp20 (branching + capture loop) | 1 s / 1 d / 30 d | 2 / 0 / 0 of 20 | 8.88e-15 / 1.94e-15 / 2.97e-14 | 8.88e-15 / 4.97e-15 / 1.24e-13 |
| stiff (lambda 1e-12..1e4 /s) | 1 s / 1 d / 30 d | 3 / 0 / 0 of 8 | 1.42e-14 / 1.82e-14 / 7.72e-15 | 2.39e-14 / 3.98e-11 / 9.41e-14 |

**6 of 96 entries bit-identical** to numpy; worst normwise error 2.97e-14.

**Localisation (first divergent operation).** `cram_r` in the driver runs the upstream
`CRAM16` source (all `complex128`) with the pole solve replaced by a pure-Python replica
of `petir::depletion::onix_cram::solve_pfd` (partial pivoting on `hypot`, textbook complex
division `(ac+bd)/(c^2+d^2)`, `(ac-bd)+(ad+bc)i` products, same loop order). **96 of 96
entries are bit-identical to this crate.** So everything upstream of the pole solves
(`At = A*dt`, `At - theta*I`, `alpha*N_0`, the real-part sum, `2*S + alpha_0*N_0`, the
coefficients) is bit-identical, and the whole difference is numpy's LAPACK `zgesv` against
the textbook elimination: already visible on the 1 x 1 case at dt = 1 d, where the only
operation is one complex division. It is not the long-double tail (ld == d), nor the
shim.

## Case 2: burnup matrix, `mat_builder` vs `build_matrix`

36 synthetic nuclides, 99 non-zero entries of `A = B*1e-24*3e14 + C` in ONIX.

| | entries (ONIX / port) | bit-identical | within 4 ulp | worse | only in ONIX | only in port | channels the port cannot express | worst rel. diff |
|---|---|---|---|---|---|---|---|---|
| pre-fix | 99 / 91 | 78 | 9 | 4 | 8 | 0 | 6 | 148 |
| **post-fix** | 99 / 99 | **88** | **11** | **0** | 0 | 0 | 0 | 2.12e-16 |

Isomer-touching entries post-fix: 17 of 17 bit-identical.

The 11 non-identical entries are all operation order, not data: the diagonals (ONIX
`removal = sum(sigma)` then `*1e-24*flux`; this crate sums the already-scaled rates) and the
fission-yield entries (`((sigma_f*pct)*1e-2)*1e-24*phi` against
`(sigma_f*1e-24*phi)*(pct*1e-2)`). Largest 2.12e-16 relative (1 ulp). Reaching
bit-identity needs a second assembly path that keeps barns and percent (a public API
addition): **deferred to the maintainer**, see gh:#1404.

**One-off, ONIX's bundled libraries** (`reduced_nucl_set`, ENDF/B-VIII-derived; not
committed; `onix_driver.py --real`, ignored test `real_libraries_burnup_matrix_vs_onix`):

| | entries (ONIX / port) | bit-identical | within 4 ulp | worse | only in ONIX | only in port | iso entries bit-identical (of 6386) | worst rel. diff |
|---|---|---|---|---|---|---|---|---|
| pre-fix | 36143 / 35771 | 18699 | 16892 | 149 | 403 | 31 | 2952 | 9450 |
| **post-fix** | 36143 / 36143 | **19184** | **16959** | **0** | 0 | 0 | 3368 | 5.1e-16 |

Zero fission-yield overwrite collisions: ONIX assigns (`=`) the yield entry where the port
adds, and no worse-than-ulp entry exists, so on this library the two conventions coincide.

### Defects found and fixed (translation from upstream)

1. **No ground-to-isomer channels.** ONIX `xs_prod_fromS_toX` (`(n,gamma)X`, `(n,2n)X`) and
   `decay_prod_fromS_toX` (`betanegX`, `betaposX`, `alphaX`, `neutronX`, `protonX`,
   `list_and_dict.py:244-259`) had no counterpart: 137 `(n,gamma)X` entries exist in
   `xs_lib` alone. Added `ReactionChannel::{NGammaToIsomer, N2nToIsomer}` and
   `DecayMode::{BetaMinusToIsomer, ..., ProtonEmissionToIsomer}`.
2. **Excited-state parents kept `m`.** ONIX `fromX` tables lower `dm` by one
   (`X(n,gamma)` `[0,1,-1]`, `Xbetaneg` `[1,0,-1]`, `X(n,gamma)X` `[0,1,0]`); the port applied
   the ground-state delta unchanged, so a capture on Am-242m produced Am-243m instead of
   Am-243. Fixed in `daughter()` via `state_adjusted_dm`. A parent with `m > 1` now has no
   daughter (ONIX models states 0 and 1; it would raise `UnboundLocalError`); previously it
   kept `m`.

Pinned by `reactions::tests::daughters_equal_the_four_onix_tables` (all eight ONIX tables
for both parent states) and `states_above_one_have_no_daughter`.

### Doc claim corrected

`DepletionSystem::deplete_multi` said the matrix is re-assembled at each step; the code
assembles it once before the loop. Struck through and corrected in place (the README and
`docs/outram-park-fork-onix-api.md` mirror carry the generated text; see "Not done").

## Case 3: 10 steps of 30 d, constant flux

ONIX's `burn_microstep` arithmetic with the upstream functions (state carried in
`longdouble` between steps, as upstream does) against `deplete_steps_floored` (new: the
existing `cram16` + `clamp_nonnegative(1e-24)` per step, which is `CRAM_density_check`;
`deplete_multi` has no floor).

| step | pre-fix normwise | post-fix normwise | post-fix worst significant rel. |
|---|---|---|---|
| 1 | 3.26e-7 | 6.7e-15 | 4.1e-14 |
| 5 | 1.42e-6 | 1.2e-14 | 1.3e-13 |
| 10 | 2.40e-6 | 4.5e-14 | 1.1e-13 |

Post-fix bounds pinned: normwise < 5e-14, significant-entry < 2e-13, at every step. Pre-fix,
entries fed through an isomer were wrong by 100 % (significant rel. 1.0).

## Verdict on predictions

| # | verdict |
|---|---|
| P1 | held (observed before writing) |
| P2 | held on the bounds (<= 3e-14 normwise). **Wrong on the cause**: I expected the long-double tail and the `At - theta` double rounding to contribute; ld == d on all 96 entries, and the replica is bit-identical, so only LAPACK vs textbook elimination matters |
| P3 (a) | held (decay and capture entries between ground-state nuclides bit-identical) |
| P3 (b) | held (<= 1 ulp; diagonals and fission yields) |
| P3 (c) | held: defects found, 8 of 99 synthetic and 403 of 36143 real entries missing, 31 real entries in a wrong row. I said 137 `(n,gamma)X` entries; the full missing count is 403 |
| P4 | **failed on the unfixed port** (normwise 3.3e-7 to 2.4e-6, not 1e-12): the defect above. Held after the fix |

## Upstream behaviour recorded (not port defects)

- `CRAM16` does not run on numpy 2.4.6 (`np.linalg` rejects `complex256`; not checked on
  other releases); the
  stand-alone path needs OpenMC *importable* (module-level imports in `onix/__init__.py`).
  Both contradict the issue text ("pure Python (+NumPy/SciPy) ... OpenMC not needed").
- `burn_cell` calls `burn_substep_pc(bucell, B, C, N, s, i, microsteps_number, mode)` and
  `burn_substep_pcME4(...)` with eight arguments; both are defined with six
  (`burn.py:235`, `:294`): the predictor-corrector paths raise `TypeError` as committed.
- `Passport.fy` setter tests `self.get_FAM == 'ACT'` (the bound method, never equal), so
  its `Not_a_Fission_Product` guard never fires.
- `salameche/py_pade.py` (6 lines, `scipy.linalg.expm`) is the commented-out alternative to
  CRAM in `burn.py`.

## Not done

- `docs/outram-park-fork-onix-api.md` not regenerated: `kovan-cli` is not built in this
  worktree and the CPU is shared; the mirror still shows the pre-fix enum docs.
- `(n,t)t` (tritium from Li-6 / Be-10 `(n,t)`, `passport.py` `_set_xschild`) not ported; the
  default libraries carry no `(n,t)` column, so no case exercises it. Deferred.
- First-divergence localisation for case 2's 11 ulp entries is by inspection of the
  operation order (above), not by a replica run.
