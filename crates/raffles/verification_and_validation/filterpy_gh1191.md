# FilterPy's Kalman filters in raffles (gh:#1191)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1191 (epic gh:#1177). **Date:** 2026-11-04. State
estimation on **synthetic** data only (`RESPONSIBLE_USE.md`): no live plant
data enters this work, and nothing here is fit for operating anything.

Sections 1–4 (search, upstream findings, design, predictions) were written
and committed **before any port code and before any comparison was run**,
together with the reference generator, its outputs and the kernel probe.
Results are added afterwards in their own section, without editing the
predictions.

## 1. What was searched before writing anything

| Where | Looked for | Found | Verdict |
|---|---|---|---|
| RAVEN (gh:#1193) | data assimilation, Kalman, ensemble filters | none: RAVEN has no assimilation | no RAVEN route first (the maintainer's ordering rule) |
| `raffles` (`grep -ri "kalman\|ensemble.*filter\|riccati\|smoother"`) | any filter | none; the gh:#1177 epic and `fidelity` docs list Kalman as "comes with gh:#1191" | port |
| workspace (`grep -rli "kalman"` over `crates/*/src`) | a Kalman filter elsewhere | none in a library (`outram-park-digital-twin-engine` only mentions it as future work) | port |
| `raffles::surrogate::smt::linalg` | triangular solves, a small inverse, a QR | dense helpers, unblocked loops, plain sums | **rejected for the filters**: bit-identity with FilterPy needs numpy/OpenBLAS's own rounding order (section 2.3), which these do not follow. Not changed. |
| `raffles::bayesian::transitional::cholesky`, `koh::model::{chol, inverse}` | Cholesky, inverse | Cholesky with a jitter, Gauss–Jordan inverse | rejected for the same reason |
| `raffles::distributions::seeded::sample_normal`, `petir::rng::lcg` | Gaussian draws | Box–Muller on the workspace LCG | **reused** for the EnKF's own draws (with a Cholesky factor of the covariance) |
| `polynomial_chaos::special::pairwise_sum` | numpy's add reduction | numpy's pairwise sum for contiguous runs | not needed: every reduction FilterPy makes here runs over a non-contiguous axis 0, which numpy sums sequentially (measured, section 2.3) |

**Upstream picked:** FilterPy, R. Labbe (companion to *Kalman and Bayesian
Filters in Python*), <https://github.com/rlabbe/filterpy>, commit
`5df4e095fac07684a05c495bd72d3424f7c68c9c` (master, 2026-10-26),
`__version__ = "2.0.0"`. **Licence: MIT**, `Copyright (c) 2015 Roger R.
Labbe Jr` (`LICENSE`, preserved verbatim as `crates/raffles/LICENSE-MIT-FILTERPY`;
`setup.py` declares `license='MIT'`). MIT flows one way into AGPL-3.0; the
crate `NOTICE` records it.

## 2. Upstream findings (FilterPy is the specification)

### 2.1 What each filter does at this commit

1. **`KalmanFilter.update` is always the Joseph form**:
   `P = (I − KH) P (I − KH)ᵀ + K R Kᵀ`, with `PHT = P Hᵀ` shared,
   `S = H PHT + R`, `SI = inv(S)` (numpy's `linalg.inv`), `K = PHT SI`. The
   non-Joseph `P − K(HP + Mᵀ)` lives only in `update_correlated`.
   `predict` is `x = F x (+ B u)`, `P = α² F P Fᵀ + Q` with `α² = 1`.
2. **`rts_smoother(Xs, Ps)`** runs `k = n−2 … 0` with `Fs[k+1]`, `Qs[k+1]`:
   `Pp[k] = F P[k] Fᵀ + Q`, `K[k] = P[k] Fᵀ inv(Pp[k])`,
   `x[k] += K[k](x[k+1] − F x[k])`, `P[k] += K[k](P[k+1] − Pp[k]) K[k]ᵀ`,
   and returns `Pp[n−1] = Ps[n−1]` (a copy, never recomputed).
3. **`ExtendedKalmanFilter.predict` is linear** (`x = F x + B u`, `B = 0`);
   a nonlinear process model means overriding `predict_x`. Its `update`
   takes the caller's `HJacobian(x)` and `Hx(x)`, uses **`scipy.linalg.inv`**
   (not numpy's), and is Joseph-form.
4. **`UnscentedKalmanFilter.predict` re-draws the sigma points** from the
   predicted `(x, P)` after the unscented transform
   (`self.sigmas_f = self.points_fn.sigma_points(self.x, self.P)`), so the
   update's cross-covariance uses fresh points, not the propagated ones.
   That line arrived in commit `94852b4` (2019-10-25), **after the PyPI
   1.4.5 release**: a filter ported from 1.4.5 would differ here.
5. **`unscented_transform`** computes `x = Wm · sigmas` (a numpy `dot`)
   and, for the default residual, `P = yᵀ @ (y * Wc[:, None]) + noise`
   (since `e1ba31e`, 2026-03-07, replacing `np.dot(y.T, np.dot(np.diag(Wc), y))`
   to dodge a numpy 2.4.0 memory leak). The update is
   `P = P − K S Kᵀ` (no Joseph form in the UKF).
6. **`MerweScaledSigmaPoints`**: `λ = α²(n + κ) − n`,
   `U = cholesky((λ + n) P)` (scipy, **upper** triangle), sigma `k+1` is
   `x − (−U[k])` (bitwise `x + U[k]`), sigma `n+k+1` is `x − U[k]`;
   `Wc = Wm = 0.5/(n + λ)` except `Wm[0] = λ/(n + λ)` and
   `Wc[0] = λ/(n + λ) + (1 − α² + β)`.
7. **`EnsembleKalmanFilter`**:
   - the initial ensemble and the process and measurement perturbations are
     `numpy.random.multivariate_normal` draws;
   - `predict` propagates each member, adds the process draw, and sets `x`
     to the ensemble mean and `P` to the sample covariance (divisor N − 1);
   - `update` forms `P_zz` and `P_xz` from `outer_product_sum` (an
     `einsum`, then a sum over axis 0), divides by N − 1, and adds
     `z + e_r[i] − h(σ_i)` times K to each member.
   - **Upstream behaviour, ported as is:** after `update`, `x` is the
     updated ensemble's mean, but `P = P − K S Kᵀ` is the *analytic*
     posterior from the prior sample covariance, **not** the updated
     ensemble's sample covariance. The port keeps this and offers
     `ensemble_covariance()` beside it.
   - `P_xz` uses `self.x`. That is the prior mean after a `predict`, but
     the *initial* `x` (not the ensemble mean) if `update` comes first.
8. **`Q_discrete_white_noise(dim, dt, var, block_size, order_by_dim)`**
   accepts only `dim ∈ {2, 3, 4}`. Entries are Python `dt**k` (C `pow`)
   times the literal coefficients; the blocks are `scipy.linalg.block_diag`,
   or `order_by_derivative` when `order_by_dim=False`, times `var`.
   `Q_continuous_white_noise` is the same with its own table. Both are
   ported; `van_loan_discretization` and `linear_ode_discretation` (they
   need `expm`) are not.

### 2.2 The reference runs

`filterpy_gh1191/generate_references.py` runs FilterPy itself (clone at the
commit above; numpy 2.4.6, scipy 1.17.1, Python 3.11.17, x86-64, OpenBLAS
0.3.31 DYNAMIC_ARCH dispatching the **SkylakeX** core) and writes
`references/filterpy_filters.json` (sha256 `863e7b4a…f6add8`) and
`references/filterpy_enkf_statistical.json` (`c1161f06…4195fa`).

**Inputs are fixed numbers.** Measurements come from closed-form saw-tooth
expressions, not random draws. The EnKF's draws are made once, written into
the JSON, and handed to FilterPy in call order by monkeypatching
`multivariate_normal` in its module. The same arrays go to the port. No
`fx`/`hx` calls a transcendental function except `sqrt`, which is correctly
rounded on both sides.

| Case | Model | dim x / z | Steps | Exercises |
|---|---|---|---|---|
| `kf_random_walk` | x ← x, z = x | 1 / 1 | 60 | scalar path; analytic gain |
| `kf_constant_velocity` | FilterPy's docstring model (dt 0.1, var 0.13, R 5) | 2 / 1 | 50 | `Q_discrete_white_noise(2)` |
| `kf_constant_acceleration` | CA, two correlated measurements, control `B u` | 3 / 2 | 40 | 2×2 S, `dot(B, u)` |
| `kf_cv_2d` | 2-D constant velocity, ordered by derivative | 4 / 2 | 30 | `order_by_derivative`, 4×4 RTS inverse |
| `q` (108 calls) | both noise helpers × dim 2,3,4 × dt 0.1, 0.5, 1.3 × block 1,2,3 × both orders | — | — | the helpers |
| `ekf_radar` | the book's radar (slant range to a 100 m/s, 1000 m target) | 3 / 1 | 40 | Jacobian path, `scipy.linalg.inv` at 1×1 |
| `ukf_constant_velocity` | linear CV, α 0.1, β 2, κ 1 | 2 / 1 | 50 | weights with Wm₀ = −65.7 |
| `ukf_radar` | radar, α 0.5, β 2, κ 0 | 3 / 1 | 40 | nonlinear h, 7 points |
| `ukf_scalar_cubic` | `x + dt(0.5x − 0.1x³)`, `h = 0.05x² + x`, α 1e-3, κ 2 | 1 / 1 | 40 | n = 1 (the `ddot` path), extreme weights |
| `ukf_constant_acceleration` | linear CA, α 0.3 | 3 / 2 | 40 | 2×2 S in the UKF |
| `enkf_constant_velocity` | linear CV, N 20, injected draws | 2 / 1 | 30 | the whole EnKF |
| `enkf_constant_acceleration` | linear CA, N 25, injected draws | 3 / 2 | 25 | 2×2 S in the EnKF |
| `enkf_statistical` | CV, N 50, **numpy's own draws**, 400 seeded replicates | 2 / 1 | 30 | the statistical comparison (kept apart) |

Every per-step quantity is recorded: the prior and posterior x and P, K,
S, SI and y. The UKF case adds the sigma points and the EnKF case the
ensemble. The RTS output is recorded too.

### 2.3 numpy's rounding order, measured (what bit-identity needs)

FilterPy is numpy. Every `dot` is an OpenBLAS gemm, gemv or ddot, `inv` is
LAPACK `dgesv` and scipy's `cholesky` is `dpotrf`. Their rounding order
was **measured, not read from OpenBLAS source**
(`filterpy_gh1191/probe_numpy_kernels.py`). numpy's result was compared
bit for bit with candidate orders computed exactly (`fractions.Fraction`,
so an emulated fused multiply-add rounds once), 200 random cases per row,
on the same SkylakeX dispatch.

| numpy call | order that reproduces it (200/200) | measured envelope |
|---|---|---|
| `dot(A, B)`, both 2-D, neither a vector (gemm) | each element one FMA chain over k, from 0 | 2×2 … 5×5, 3×7·7×3 |
| `dot(A, x)`, x a column or 1-D, A with m ≥ 2 rows (gemv-T) | 4-lane FMA blocks reduced `(l0+l2)+(l1+l3)`, then a k mod 4 tail: `k=1: fma(a,x,y)`; `k=2: y + fma(a0,x0,a1x1)`; `k=3: y + fma(a2,x2,fma(a0,x0,a1x1))` | k ≤ 7 for any m; any k when m is a multiple of 4 |
| `dot(a, x)`, one row (ddot) | one FMA chain from 0 | k ≤ 15 |
| `dot(w, S)`, 1-D w times (k, m) S, m = 2, 3 (gemv-N) | per column: pairs `t + fma(s_i w_i, s_{i+1} w_{i+1})` in fours, then an FMA chain | any k |
| `linalg.inv` (dgesv on I) | left-looking LU (`getf2`, ddot then subtract; first max pivot; reciprocal scaling), then forward and back trsm with **reciprocal diagonals**, rows split into power-of-two blocks (3 = 2 + 1, 5 = 4 + 1) with a GEMM update `c = fma(−1, Σ, c)` between blocks and fused updates inside | n ≤ 5 |
| `scipy.linalg.cholesky` upper (dpotf2) | `u_jj = √(a_jj − ddot)`; off-diagonal `(gemv-T with x scaled by −1, i.e. fused) × (1/u_jj)` | n ≤ 4 |
| `mean(axis=0)`, `sum(axis=0)` | a sequential sum over rows (then ÷ N) | N 20, 50 |

**Outside the envelope** (rows kept in the probe): gemv with 2 rows at
k = 8 (65/200), ddot at k = 16 (60/200), `inv` at 6×6 (54/200). There the
port's kernels keep the same structure but are not claimed bit-identical.
**`scipy.linalg.inv` (getrf + getri) is a different algorithm** from
numpy's `inv`: 15/500 identical at 2×2. FilterPy's EKF uses it, so an EKF
with a vector measurement would be round-off-level, not bit-identical, from
FilterPy. The reference EKF case has a scalar measurement, where both are
`1/s`. **The order is CPU-dispatch dependent**: on another OpenBLAS core
(Haswell, Zen) the references would need regenerating, and the JSON
records the core.

## 3. Design (the port)

- `crates/raffles/src/filtering/`:
  - `numpy_ops.rs` (**written, not ported**): the `Matrix` type and
    kernels in the measured orders above;
  - `kalman.rs` (`KalmanFilter`, `rts_smoother`), `ekf.rs`,
    `sigma_points.rs`, `ukf.rs` (with `unscented_transform`), `enkf.rs`
    (with `outer_product_sum`), `discretization.rs`: **ported**, each with
    FilterPy's header (file, commit, copyright, MIT);
  - `consistency.rs` (NEES, NIS, coverage; **written**).
- `fx`/`hx` are generic closures passed to each call
  (`F: Fn(&[f64]) -> Vec<f64>`), never stored, so no filter struct carries
  a type parameter or a lifetime, and nothing is `dyn`. **Deviation from
  upstream:** FilterPy stores `fx`/`hx` on the UKF/EnKF object.
- The EnKF takes its perturbations either from the caller (the
  bit-identity path, and how a test feeds numpy's draws) or from its own
  draws (`petir` LCG, Box–Muller, a Cholesky factor). The second is
  compared with FilterPy only statistically.
- `raffles::fidelity::StateSpaceModel`: the sibling of `HighFidelityModel`
  and `ParametricModel`. Associated `State: Quantities` and
  `Observation: Quantities`, with `propagate(&x, step)` and
  `observe(&x, step)`, consumed generically. The enum
  `fidelity::Assimilator { Extended, Unscented, Ensemble }` runs any of the
  ported filters on a model, with uom in and out and SI `f64` inside. The
  EKF's Jacobians there are central differences (**written**, not
  FilterPy, which takes the caller's Jacobian).

## 4. Predictions (written before the port and before any comparison)

Code to code against the committed references:

| # | Quantity | Prediction |
|---|---|---|
| D1 | `Q_discrete_white_noise`, `Q_continuous_white_noise`: all 108 | **bit-identical** (`powf` and Python's `**` are both libm `pow`) |
| D2 | KF: x, P, K, S, SI and y at every step (4 cases) | **bit-identical** |
| D3 | RTS smoother: x, P, K and Pp (4 cases) | **bit-identical** |
| D4 | EKF radar: every step | **bit-identical** (S is 1×1, so scipy's `inv` is `1/s`) |
| D5 | Merwe weights and the initial sigma points (4 cases) | **bit-identical** |
| D6 | UKF: x, P, K, S, SI, y and both sigma sets at every step (4 cases) | **bit-identical** |
| D7 | EnKF with injected draws: x, P, K, S, SI and the ensemble at every step (2 cases) | **bit-identical** |
| D8 | EnKF statistical (port: own LCG draws; FilterPy: numpy draws; 400 replicates each, N 50, CV model, last step) | **pass**, with the criterion fixed now: (a) for each state component, the replicate means differ by < 3 √(se₁² + se₂²); (b) the ratio of replicate variances of x̂ lies in [0.74, 1.35] (≈ 3σ for 400 replicates); (c) the replicate-mean `P` diagonal lies within 10 % between the codes; (d) for both codes, the mean of P's diagonal lies in [0.85, 1.05] × the exact KF's |

If a bit-identity prediction misses, the cause is diagnosed and stated.
A miss inside 1e-8 relative is accepted only with its cause explained,
per the workspace rule.

Analytic checks (Rust only, independent of FilterPy):

| # | Check | Prediction |
|---|---|---|
| A1 | Scalar random walk, q = 0.04, r = 0.25: the gain after 60 steps against the Riccati fixed point `P⁻ = (q + √(q² + 4qr))/2 = 0.121980…`, `K = P⁻/(P⁻ + r) = 0.327920…` | relative difference < 1e-12 (the error contracts by (1 − K)² ≈ 0.45 per step, so only round-off remains) |
| A2 | KF + RTS on the CV and CA models against the batch least-squares (dense Gaussian) posterior of the whole trajectory: marginal means and covariance blocks | max relative difference < 1e-10 |
| A3 | UKF against the KF on the linear CV and CA models (the UKF is exact for linear models) | max relative difference < 1e-9 on x and P (the α = 0.1 weights amplify round-off about 100×) |
| A4 | EnKF mean against the exact KF as N grows (N 50, 200, 800; one seed each) | the error falls roughly as 1/√N (each step within a factor 2 of halving per 4× N) |

## 5. Results (2026-11-04, added after the runs; sections 1–4 unedited)

Port commit: `bd1042431` (after `b6bc8cfc3`, the references and predictions).
`cargo test --release -p raffles --test filterpy_gh1191`; every number below
is printed by that test and asserted there.

### 5.1 Code to code against FilterPy

| # | Quantity | Prediction | Measured | Verdict |
|---|---|---|---|---|
| D1 | noise helpers (108 calls) | bit-identical | **4872 / 4872** bit-identical | met |
| D2 | KF, 4 cases, every step | bit-identical | **4670 / 4670** | met |
| D3 | RTS, 4 cases | bit-identical | **3700 / 3700** | met |
| D4 | EKF radar | bit-identical | **1200 / 1200** | met |
| D5 | Merwe weights, initial points | bit-identical | **99 / 99** | met |
| D6 | UKF, 4 cases (incl. α = 1e-3, where Wm₀ = −3.3 × 10⁵) | bit-identical | **7480 / 7480** | met |
| D7 | EnKF, injected draws | bit-identical | first run: about a quarter of the numbers differed (worst 9e-13 relative, ensemble members); after the fix **7580 / 7580** | **missed on the first run**, cause found |
| D8 | EnKF statistical, 400 + 400 replicates | pass | (a) \|Δmean\|/se 1.19, 1.55; (b) variance ratios 1.034, 0.981; (c) mean P within 0.05 % and 0.7 %; (d) P / P_KF 0.955 / 0.955 (position), 0.929 / 0.923 (velocity) | met |

**D7's miss, diagnosed (not tuned).** Section 2.3 measured numpy's axis-0
reductions on rows 3 wide and claimed "sequential". For a one-wide axis
(an `(N, 1)` array, or a stack of 1×1 matrices) the reduced axis is
contiguous, and numpy uses its **pairwise** sum (measured, 300/300, now a
row of `probe_numpy_kernels.py`; sequential is 33–148/300 there). The EnKF
hits this through `mean(sigmas_h)` and `outer_product_sum` when dim_z = 1.
`numpy_ops::{mean_rows, sum_matrices}` now use it, reusing
`bayesian::koh::pairwise_sum`. Nothing else changed, and every number is
then bit-identical. Section 2.3's table entry should be read with this
correction.

**D8's first run did not start.** The draws helper used a strict Cholesky,
and FilterPy's `Q_discrete_white_noise(2)` is **rank 1** (positive
semi-definite), which numpy's SVD-based `multivariate_normal` accepts. The
helper now uses a semi-definite outer-product Cholesky (`psd_factor`, unit
test: rank-1 draws stay on their line). Criterion and replicates
unchanged. Both codes' EnKF P sits at 0.92–0.96 of the KF's, the familiar
small-ensemble under-dispersion (N = 50); it is the same in both, which is
what D8 tests.

**Bit-identity envelope, restated.** It holds for every case here (state
dims 1–4, measurement dims 1–2, ensembles 20–25, sigma sets 3–7). It is
claimed nowhere else (section 2.3), and only on the SkylakeX dispatch the
references were made on.

### 5.2 Analytic checks (Rust only)

| # | Prediction | Measured | Verdict |
|---|---|---|---|
| A1 | gain < 1e-12 from the Riccati fixed point | K 2.2e-16 (one ulp), P⁻ exact | met |
| A2 | KF + RTS = batch LS to < 1e-10 | means: CV 3.2e-12, CA 2.9e-11. Covariance: CV 9.1e-11, **CA 1.9e-10** | **missed on the CA covariance** |
| A3 | UKF = KF on linear models, < 1e-9 | 5.5e-14 … 6.8e-14 | met |
| A4 | EnKF error ∝ ~1/√N | 0.110, 0.075, 0.034, 0.017 σ at N 50, 200, 800, 3200 (ratios 0.69, 0.45, 0.49) | met |

**A2, honestly.** The instrument needed fixing twice, and the miss stays a
miss:

1. The information-form batch could not run at all: FilterPy's kinematic Q
   is singular, so Q⁻¹ does not exist.
2. A covariance-form batch, `Σ − G H Σ`, then disagreed by 3.2e-7. That is
   the instrument's own cancellation under the CV case's diffuse
   P₀ = 1000 I.
3. With the Joseph form, which avoids that cancellation, the numbers are the
   ones in the table.

Two algebraically equal forms of the same batch disagree with **each
other** by 6.4e-7 (CV) and 1.6e-5 (CA). So the batch cannot resolve 1e-10
on these ill-conditioned covariances, and the 1.9e-10 cannot be attributed
to the RTS smoother. The test now gates the means at the predicted 1e-10
and the covariance at **1e-9**. That covariance gate was **set after the
miss**, and the test also requires the difference to be under 10⁻³ of the
instrument's spread. A better instrument (extended precision, or a
square-root batch) is the way to close this. It was not built.

**Maintainer review, 2026-11-04 (gh:#1208):** the post-hoc 1e-9 covariance
gate is accepted ("Okay lah I'm fine with 1208"). The miss against the
predicted 1e-10 stays recorded as a miss.

### 5.3 Typed route

`fidelity::state_space` test: on a linear CV model the typed EKF (central
differences) and UKF agree with FilterPy's KF to < 1e-9 relative at every
step. The typed EnKF (N 400) agrees within its sampling error. The model
used `uom` Length/Velocity in and out.

### 5.4 Tests run

`cargo test --release -p raffles --lib --tests -j 2`: **315 passed, 0
failed, 2 ignored** (2026-11-04, including the 10 tests of
`tests/filterpy_gh1191.rs` and the new unit tests).
`cargo check --release -p raffles --all-targets --target aarch64-linux-android`
and `cargo check --release -p raffles --lib --target wasm32-unknown-unknown`:
clean (2026-11-04; target trees deleted afterwards). No new dependency.
