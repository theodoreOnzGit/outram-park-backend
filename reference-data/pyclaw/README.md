# PyClaw code-to-code reference for `petir::ssprk`

`ssp-rk-pyclaw-reference.txt` is the output of PyClaw's **own**
`SharpClawSolver.step` (time integrators `SSP22`, `SSP33`, `SSP104`), produced
by `ssp_rk_reference_driver.py`. `crates/petir/tests/ssprk_vs_pyclaw.rs`
compares `petir::ssprk` against it bit for bit.

| field | value |
|---|---|
| Upstream | Clawpack / PyClaw, <https://github.com/clawpack/pyclaw> |
| Licence | BSD-3-Clause, Copyright (c) 1994-2018 Clawpack Developers |
| File run | `src/pyclaw/sharpclaw/solver.py`, unmodified (sha256 `58a436efe8129fab1c550724f082cff7414d4e332d13e0db278cf4a973a717ac`) |
| Commit | `f522337ef75abef1153e2025a204b7ef4f7c5c9f` (2026-06-25) |
| Generated | 2026-10-16, x86_64 Linux, CPython 3.11.17, NumPy 2.4.6 |

## Method

The driver loads upstream's `solver.py` from disk and stubs **only its
imports**: the `Solver` base class, the `add_parent_doc` docstring helper and
the WENO reconstructor. None of these is touched by the SSP branches of
`step`. A bare `SharpClawSolver` is built without its constructor, and
`dq_hyperbolic` returns `dt * L(t, q)`, which is PyClaw's `dq` contract.

The right-hand side is pure arithmetic, so NumPy and Rust perform the same
IEEE-754 operations:

```text
L(t, q)_i = -4 (q_i - q_{i-1}) - 0.5 q_i q_i + 0.1 t      (16 cells, periodic)
q_i(0)   = 1 for 4 <= i < 10, else 0.25 + 0.01 i
```

The run is 40 steps at `dt = 0.0625` (a power of two) and at `dt = 0.03`. Each
line of the output holds the method, `dt` as a hex float, the step count, the
cell index and the final value's `f64` bit pattern.

Regenerate with:

```bash
git clone --depth 1 https://github.com/clawpack/pyclaw.git
python3 -I ssp_rk_reference_driver.py pyclaw/src/pyclaw/sharpclaw/solver.py > ssp-rk-pyclaw-reference.txt
```

## Provenance re-verified (2026-10-16, after a container restart)

The reference was produced by a session that was then interrupted, so a
second session checked it rather than trusting it:

- The upstream clone was at `f522337ef75abef1153e2025a204b7ef4f7c5c9f` with a
  clean `git status`, and `solver.py` hashed to the sha256 in the table above.
- Re-running the driver against it (CPython 3.11.17, NumPy 2.4.6) reproduced
  `ssp-rk-pyclaw-reference.txt` **byte for byte** (96/96 lines, `diff` empty).
- **Mutation check, so the driver is shown to execute upstream's code rather
  than its own:** swapping the weights on upstream's SSP33 second-stage line
  (`0.75*state.q + 0.25*(...)` → `0.25*state.q + 0.75*(...)`, line 318) in a
  scratch copy changed exactly the 32 SSP33 lines and no others.
