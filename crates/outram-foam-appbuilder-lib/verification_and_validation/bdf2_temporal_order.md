# BDF2 (`DdtScheme::Backward`) with its own Rhie–Chow `ddtCorr` partner: temporal order and steady consistency

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-11-01 (release build, Rust 1.99.1, this machine)
**Crate version / commit:** `outram-foam-appbuilder-lib` 0.1.6 and `outram-foam-basic-lib` on top of `e8c579d6b`, the gh:#1067 change
**Tests:** `tests/bdf2_temporal_order.rs` (this crate), `src/fv_operators/fvc/ddt_corr.rs` unit tests (`outram-foam-basic-lib`), `src/solvers/schemes.rs::ddt_corr_scheme_dispatches_to_the_matching_partner`, `tests/fv_scheme_selection.rs` (re-measured)

## Summary: which numbers to quote

- **The fix:** `Backward` now pairs with the BDF2 Rhie–Chow correction
  `fvc::ddt_corr_backward`, a port of OpenFOAM's
  `backwardDdtScheme::fvcDdtPhiCorr`. Before this change it paired with the Euler one.
- **What changed (quote this):** the steady-state disagreement between Euler and Backward
  on the Re = 100 cavity falls **2.7–3.6×** at every Δt tested. At Δt = 1e-3 s it is
  **8.04e-3 m/s, down from 2.93e-2 m/s** (Study 3).
- **What did not change (quote this too):** the disagreement **still grows** as Δt is
  refined, from 3.80e-3 to 8.04e-3 m/s over Δt = 8e-3 to 1e-3 s. **`Backward` is not
  verified second order.**
- **Do not quote Study 1's "orders" as temporal orders.** They are about 0 for every
  pairing, Euler included. Study 2 shows they measure how the discrete fixed point
  depends on Δt, not time-truncation error.

## Methodology

### Port

`backwardDdtScheme<Type>::fvcDdtPhiCorr(U, phi)` in OpenFOAM-dev
`src/finiteVolume/finiteVolume/ddtSchemes/backwardDdtScheme/backwardDdtScheme.C`
(Copyright (C) 2011-2026 OpenFOAM Foundation, GPL-3.0). The routine was read from a copy
of that file taken on 2026-11-01. The file is not vendored here. The port computes

```text
coeff    = 1 − min(|φⁿ⁻¹ − S_f·interp(Uⁿ⁻¹)| / (|φⁿ⁻¹| + SMALL), 1)      (fvcDdtPhiCoeff, single old level)
ddtCorr  = coeff/Δt · [(coefft0 φⁿ⁻¹ − coefft00 φⁿ⁻²) − S_f·interp(coefft0 Uⁿ⁻¹ − coefft00 Uⁿ⁻²)]
coefft   = 1 + Δt/(Δt + Δt₀),  coefft00 = Δt²/(Δt₀(Δt + Δt₀)),  coefft0 = coefft + coefft00
```

On the first step, when no old-old level exists, it returns the Euler form exactly. Upstream
gets there with `Δt₀ = great`. `ddtScheme.C` (`fvcDdtPhiCoeff`) was **not** re-read for this
change: the limiter is the one `fvc::ddt_corr` already carried.

`PimpleFoam` dispatches through `schemes::ddt_corr_scheme`, which mirrors OpenFOAM's
`fvc::ddtCorr` selecting on `ddtSchemes`:

- Euler uses the Euler form.
- Backward uses the BDF2 form.
- steadyState gives zero (`steadyStateDdtScheme`).

The old Euler-only pairing remains available only as the explicit ablation
`DdtCorrForm::EulerOnly`.

### Common case

`tutorials/cases/pimple_foam_cavity`: the Re = 100 lid-driven cavity with these settings:

- 20×20 uniform mesh, L = 0.1 m, U_lid = 1 m/s, ν = 1e-3 m²/s
- `Gauss upwind` convection
- `nCorrectors = 2`

There are three pairings:

| label | `schemes.ddt` | `ddt_corr_form` |
|---|---|---|
| `euler` | `Euler` | `MatchDdtScheme` |
| `bdf2-eulercorr` | `Backward` | `EulerOnly` (ablation = pre-gh:#1067 behaviour) |
| `bdf2-bdf2corr` | `Backward` | `MatchDdtScheme` (default) |

### Study 1: Richardson temporal order

- **Run:** impulsive start, advanced to T = 0.4 s at Δt = 4e-3, 2e-3, 1e-3, 5e-4 and
  2.5e-4 s.
- **Order:** `e_k = ‖U(Δt_k) − U(Δt_k/2)‖₂` (volume-weighted), and `p_k = log2(e_k/e_{k+1})`.
- **Loop settings:** `nOuterCorrectors = 1` and `= 10`.

**Prediction (written before any run):**

- `euler` ≈ 1.
- `bdf2-bdf2corr` with converged PIMPLE ≈ 2.
- `bdf2-eulercorr` ≈ 1 or worse.
- PISO ≈ 1 for every pairing, because the convection flux lags.

**Gates:** on the finest pair with converged PIMPLE, `bdf2-bdf2corr` must fall in [1.7, 2.3]
and `euler` in [0.8, 1.2].

### Study 2: restart-from-steady control

Study 2 was designed **after** Study 1 missed. It tests the assumption Study 1 rests on:
that runs at different Δt differ only by time-truncation error. The procedure:

1. Spin up to steady state (Euler, Δt = 2e-3 s, 15 s).
2. Restart every pairing from that state at every Δt and run for 0.2 s.

**Gate:** a restart at the spin-up's own Δt and scheme must drift less than 1e-9 m/s.

### Study 3: steady-state consistency (the defect's own signature)

- **Run:** t = 12 s (steady) at Δt = 8e-3, 4e-3, 2e-3 and 1e-3 s, with `nOuterCorrectors = 1`.
- **Measure:** `d = max |U_backward − U_euler|`.
- **Why it isolates the face flux:** at steady state both ddt terms vanish, so `d` can only
  come from the face flux.

**Prediction (written after Studies 1 and 2, before this run):**

- (a) The ablation reproduces the 2026-08-07 record to the printed digits.
- (b) For `bdf2-bdf2corr`, `d` decreases strictly as Δt is refined.

**Post-hoc regression pin (not evidence):**

- (c) `bdf2-bdf2corr` reproduces its own 2026-11-01 values within 1e-3 relative.

## Reference

```bibtex
@misc{openfoam_dev_backwardDdtScheme,
  author = {{OpenFOAM Foundation}},
  title  = {backwardDdtScheme.C, OpenFOAM-dev,
            src/finiteVolume/finiteVolume/ddtSchemes/backwardDdtScheme/},
  note   = {fvcDdtPhiCorr(U, phi); GPL-3.0; read 2026-11-01},
  url    = {https://github.com/OpenFOAM/OpenFOAM-dev}
}
@article{rhie_chow_1983,
  author  = {Rhie, C. M. and Chow, W. L.},
  title   = {Numerical study of the turbulent flow past an airfoil with trailing edge separation},
  journal = {AIAA Journal}, volume = {21}, number = {11}, pages = {1525--1532}, year = {1983}
}
```

Studies 1 and 2 use self-convergence as the reference. Study 3 uses an analytical identity:
at steady state both ddt terms are zero.

## Results (2026-11-01)

### Unit tests (`outram-foam-basic-lib`, `ddt_corr.rs`): all pass

- For a constant step, `BackwardDdtCoeffs` gives exactly 1.5, 2.0 and 0.5.
- For a variable step (Δt = 0.3, Δt₀ = 0.7), BDF2 differentiates t² exactly (|Δ| < 1e-12).
- With no old-old level, `ddt_corr_backward` matches `ddt_corr` **bit for bit**.
- The hand value with a constant step is 0.75 (|Δ| < 1e-12).
- A consistent history gives 0.

### Study 1: prediction MISSED; the test is `#[ignore]`d, with its gates unchanged

These are the finest-pair L2 orders, over Δt = 1e-3, 5e-4 and 2.5e-4 s:

| pairing | nOuter = 1 | nOuter = 10 |
|---|---|---|
| `euler` | −0.058 | −0.058 |
| `bdf2-eulercorr` | +0.066 | +0.067 |
| `bdf2-bdf2corr` | −0.016 | −0.015 |

Successive L2 differences, with `nOuter = 10`:

```csv
pairing,dt=4e-3:2e-3,2e-3:1e-3,1e-3:5e-4,5e-4:2.5e-4
euler,1.0177e-3,1.1786e-3,1.2873e-3,1.3397e-3
bdf2-eulercorr,1.5736e-3,1.7850e-3,1.8532e-3,1.7697e-3
bdf2-bdf2corr,1.1247e-3,1.2519e-3,1.3264e-3,1.3402e-3
```

The differences do not shrink. Runs with `nOuter = 1` and `nOuter = 10` agree to three
digits, so lagging the convection flux is not the limiting error.

### Study 2: explains the miss; live test, passes

Study 2 drifts from the spun-up state after 0.2 s (L2, m/s):

| pairing | Δt = 4e-3 | 2e-3 | 1e-3 | 5e-4 | 2.5e-4 |
|---|---|---|---|---|---|
| `euler` | 1.020e-3 | **2.985e-11** | 1.180e-3 | 2.452e-3 | 3.747e-3 |
| `bdf2-eulercorr` | 8.991e-4 | 2.256e-3 | 4.010e-3 | 5.810e-3 | 7.466e-3 |
| `bdf2-bdf2corr` | 4.468e-4 | 6.753e-4 | 1.917e-3 | 3.211e-3 | 4.487e-3 |

- The spin-up is steady: the restart at its own Δt drifted 3.0e-11.
- Every other (Δt, pairing) moves away from the steady state.
- The Δt-to-Δt differences (1.020e-3 to 1.853e-3) and the "orders" (finest pair −0.057,
  +0.066, −0.016) match Study 1 to two digits.

**The discrete steady state depends on Δt.** Study 1 measured that dependence, not temporal
order. A Richardson study on a Rhie–Chow PISO case cannot measure temporal order while the
fixed point moves with Δt. Two mechanisms are visible in the algebra:

- `rAU = V/A`, which contains Δt, enters the face flux through
  `interpolate(rAU·H) − interpolate(rAU)·interpolate(H)`.
- The `fvcDdtPhiCoeff` limiter damps the flux history by a factor `coeff < 1` per step, so
  the damping per unit time grows as Δt shrinks.

### Study 3: (a) passed, (b) MISSED, (c) pinned

`max |U_backward − U_euler|` at t = 12 s, in m/s:

| Δt [s] | Euler-only ddtCorr (ablation) | 2026-08-07 record | BDF2 ddtCorr (default) | reduction |
|---|---|---|---|---|
| 8e-3 | 1.0394e-2 | 1.0394e-2 | 3.8032e-3 | 2.73× |
| 4e-3 | 1.6499e-2 | 1.6499e-2 | 5.2084e-3 | 3.17× |
| 2e-3 | 2.3420e-2 | 2.3420e-2 | 6.8244e-3 | 3.43× |
| 1e-3 | 2.9258e-2 | 2.9258e-2 | 8.0361e-3 | 3.64× |

**RE-MEASURED 2026-11-03 (gh:#1154)**, after `PimpleFoam` gained upstream's boundary-flux
handling (the lid's flux in `phi` is now exactly `U_lid·Sf = 0`): ablation 8.3883e-3,
1.3833e-2, 2.0748e-2, 2.8047e-2; BDF2 ddtCorr 2.1260e-3, 3.1705e-3, 4.5065e-3, 5.7053e-3
(reduction 3.95–4.92×). Both still grow as Δt shrinks; the test now pins these values.
The Ghia RMS quoted below became 0.0207 for both Euler and Backward. Studies 1 and 2 moved
too (printed by the test) and were not re-tabulated here.

- **(a) passed.** The ablation reproduces the record to five digits, so the Euler path is
  unchanged.
- **(b) missed.** The consistent pairing still grows as Δt is refined.

The re-run of `tests/fv_scheme_selection.rs` at Δt = 4e-3 s shows the change on the
benchmark:

- **Ghia RMS:** Euler 0.0363 and Backward 0.0362. Before the fix Backward scored 0.0359, so
  most of its earlier edge came from the inconsistent pairing.
- **Transient difference after 5 steps:** 2.836295e-2 m/s, up from 2.829482e-2.

### Diagnostic: the limiter replaced by `coeff = 1` (uncommitted; not reproducible from the committed code)

This diagnostic tested whether the `fvcDdtPhiCoeff` limiter carries the remaining Δt
dependence. Both `let coeff = 1.0 - (...)` lines in
`outram-foam-basic-lib/src/fv_operators/fvc/ddt_corr.rs` were temporarily replaced by
`let coeff = 1.0;`. The tests were re-run, and the patch was then reverted.

- **Study 3 with `coeff = 1`:**
  - The ablation gave 1.2333e-2, 2.1969e-2, 3.6399e-2 and 5.4664e-2: it grows faster.
  - The consistent pairing gave 4.9723e-3, 5.6923e-3, 5.1687e-3 and 3.7963e-3: it rises,
    then **falls** below Δt ≈ 2e-3 s.
- **Study 1 with `coeff = 1`, finest pair (L2):**
  - `euler`: 0.256, then 0.513, then **0.715**. The order rises towards 1.
  - `bdf2-eulercorr`: −0.319, then −0.136, then 0.054. It stays at about 0.
  - `bdf2-bdf2corr`: 0.395, then 0.636, then **0.797**. The order rises.
  - The `nOuter = 1` and `nOuter = 10` results are identical to three digits.
- **Study 2 with `coeff = 1`:** the spun-up state also drifts at other Δt. The fixed point
  is Δt-dependent even without the limiter, but the dependence weakens as Δt shrinks.

**Interpretation:**

- Without the limiter, the consistent BDF2 pairing starts to converge. Its observed order is
  still rising (0.8 at the finest pair), and the case is pre-asymptotic for Δt ≳ V/A_conv ≈
  2 ms.
- The mismatched pairing never converges.
- The limiter is upstream's default (`ddtPhiCoeff < 0`) and was **kept**. Disabling it would
  make the comparison flattering rather than faithful.

## Open findings and limitations

1. **`Backward` is not verified second order.** No order could be measured on this case. A
   case with a controlled Rhie–Chow residual is needed, for example a periodic Taylor–Green
   vortex. That case needs `PimpleFoam`'s pressure-flux correction to handle cyclic patches.
   This is listed in `TODO.md`.
2. **No OpenFOAM run was made.** It is **not checked** whether upstream `pimpleFoam` shows
   the same Δt-dependent fixed point on this case. The algebra above suggests it would, since
   the structure is identical.
3. **The `SMALL` guard is 1e-37.** OpenFOAM's double-precision `small` is 1e-15. This value
   was not checked against a vendored `doubleScalar.H` and is noted at the constant in
   `ddt_corr.rs`. It was left unchanged so that the Euler path would not move. Correcting it
   is a separate change.
4. **Fixed Δt only.** `ddt_corr_scheme` passes `Δt₀ = Δt`, matching `ddt_vec_scheme`'s
   constant-step weights. `BackwardDdtCoeffs` supports a variable step, and that path is unit
   tested.
