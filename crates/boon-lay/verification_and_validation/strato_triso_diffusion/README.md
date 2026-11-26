# STRATO integration: TRISO multilayer diffusion verification (2026-11-26)

<!-- vv-unverified-banner -->
> ⚠️ **Verification, not validation.** These results compare numerical
> implementations against each other and against analytical solutions. No
> experimental data is involved. Research, education and V&V only
> (`RESPONSIBLE_USE.md`). Everything here is AI-drafted and awaits human
> review.

**Credit.** STRATO is the work of **Wee Zhern Ray (GitHub `weewzr`)**,
<https://github.com/weewzr/STRATO>, licensed AGPL-3.0-only, read at commit
`36f1a98848bec9d66fe515994e075cff09d6cefb`. Its Sturm-Liouville solver is
ported into boon-lay as `boon_lay::strato`. Its finite-volume solver is run
unmodified.

Tracking issue: gh:#1800. The plan, with every gate written before any code,
and its dated corrections, is in [`PLAN.md`](PLAN.md).

**Runs** (raw logs in this folder):

| Run | Log | Result |
|---|---|---|
| 1 | [`run1_2026-11-26.log`](run1_2026-11-26.log) | 8 of 11 tests passed. Failed on the protocol; see below. |
| 2 | [`run2_2026-11-26.log`](run2_2026-11-26.log) | All passed under the corrected protocol. |
| 3 | [`run3_2026-11-26.log`](run3_2026-11-26.log) | **Counted.** All passed after the review corrections below, which added the A2 interface gate and the missing diagnostics. |

## Outcome (run 3)

| Case | What | Outcome | Measured | Gate |
|---|---|---|---|---|
| U1 | port vs upstream STRATO Sturm-Liouville, 15 cases | **PASS** | ~~**978 of 978 values bit-identical**~~ **2026-11-26 (local change L8, PETIR transcendentals): 767 of 978 bit-identical; worst relative $2.5 \times 10^{-14}$ on eigenvalues, weights, `lambda_covered`, `accuracy_floor`; worst $F(t)$ absolute $4.4 \times 10^{-15}$** | ~~relative $\le 10^{-12}$~~ relative $\le 10^{-12}$ on everything but $F(t)$; $F(t)$ absolute $\le 10^{-14}$ |
| U2 | port vs STRATO's Python oracle, $F(2\times10^5\text{ s})$ | **PASS** | $2.3\times10^{-13}$ | absolute $\le 10^{-10}$ |
| A1 | series vs homogeneous-sphere closed form | **PASS** | $1.8\times10^{-15}$ | absolute $\le 10^{-10}$ |
| A1 | boon-lay FV: spatial orders / extrapolated error | **PASS** (FAIL in run 1) | 1.995, 1.980 / $6.6\times10^{-11}$ | [1.8, 2.2] / $\le 2\times10^{-5}$ |
| A2 | interface: $c$ and $D\,\partial c/\partial r$ continuous at $R_1$, all modes | **PASS** (first run: run 3) | flux $2.5\times10^{-15}$ (A2a), $3.7\times10^{-16}$ (A2b); $c$ exact | relative $\le 10^{-12}$ |
| A2a | two layers, $D$ drops 100× outward | **PASS** | 2.009, 2.002 / $7.9\times10^{-10}$ | as A1 |
| A2b | two layers, $D$ rises 100× outward | **PASS** | 2.002, 2.001 / $9.7\times10^{-9}$ | as A1 |
| A3/A5 | five-layer synthetic TRISO, mesh refinement | **PASS** (FAIL in run 1) | 2.000, 2.000 / $1.7\times10^{-10}$ | as A1 |
| A4 | SiC contrast $10^{-6}$ to $10^{6}$ | **PASS** (prediction partly missed) | floor $\le 1.9\times10^{-10}$; FV $\le 7.4\times10^{-7}$ | floor $\le 10^{-8}$; FV $\le 2\times10^{-5}$ |
| A6 | temporal self-convergence, 160 cells per layer | **PASS** | 0.998, 0.999 | [0.8, 1.2] |
| A7 | modal convergence | **PASS** | worst change at resolved times $4.3\times10^{-13}$ | $\le 10^{-12}$ |
| B1 | STRATO FV and boon-lay FV, each vs the series | **PASS** (FAIL in run 1) | STRATO 1.969, 1.883 (A1) and 2.000, 2.000 (A3); boon-lay as A1, A3 | [1.8, 2.2] |
| B2 | ATOPS `booth_longlived` vs the series' one-sphere adapter | **PASS** | $5.6\times10^{-16}$ | absolute $\le 10^{-10}$ |
| B2 | boon-lay FV on the same sphere | reported, not gated | $-2.1\times10^{-4}$ at $Dt/R^2 = 10^{-3}$, falling to $-6.4\times10^{-9}$ at 1 | none in PLAN |
| B3 | STRATO FV vs ATOPS, five layers | **BLOCKED** | | ATOPS has no five-layer model |
| B4 | ATOPS `breakthrough_model` vs the five-layer problem | **BLOCKED** | | not the same equation |

Run 3: 11 tests passed, 0 failed, 2.4 s.

Command:

```bash
cargo test --release -p boon-lay --test strato_triso_diffusion_verification -- --nocapture --test-threads=1
```

Hardware: Intel Xeon @ 2.30 GHz, 4 cores, 15.7 GiB RAM, CPU only, Linux.
Toolchain: rustc 1.99.1.

## The history: what failed, what was corrected, and why that is recorded

### Run 1: the protocol failed, not a solver

Run 1 missed the spatial-order gate [1.8, 2.2] on A1, A3 and B1.

| Case | Measured orders |
|---|---|
| A1 | $-0.29$, $-0.06$ |
| A3 | 1.944, **1.795** |
| STRATO FV on A1 | $-0.04$, $-0.01$ |

The B1 A3 orders were never checked in run 1: the test stopped at A1 first.

**Diagnosis.**
- On A1 both FV codes sat on the same error, $+2.5\times10^{-7}$: STRATO
  from 20 cells per layer up, boon-lay from 80. Their mutual difference
  fell at order 2.00.
- The two codes are separate implementations of the same method, so the
  shared floor alone could not tell a time error from a method error.
- The size settles it. Run 2's A1 values give the $O(\Delta t^2)$
  coefficient of backward Euler, $b = -1.98\times10^{-12}\ \text{s}^{-2}$.
- One-step Richardson in time leaves $-b \Delta t^2/2$, which at
  $\Delta t = 500$ s is $+2.48\times10^{-7}$: the plateau.

**Correction.** The protocol changed, not the gate: two-step Richardson at
$\Delta t$ = 200, 100, 50 s replaced one-step at 500, 250 s. It was applied to
A1, A2, A3/A5, A4 and B1 alike. The prediction was committed before run 2
(`PLAN.md`, "Protocol correction") and run 2 met it. A1's residual fell from
$2.5\times10^{-7}$ to $6.6\times10^{-11}$.

The cases that passed under both protocols changed as follows:

| Case | Run 1 (one-step) | Runs 2 and 3 (two-step) |
|---|---|---|
| A2a | orders 2.039, 2.125; error $3.9\times10^{-8}$ | 2.009, 2.002; $7.9\times10^{-10}$ |
| A2b | orders 2.003, 2.002; error $4.2\times10^{-8}$ | 2.002, 2.001; $9.7\times10^{-9}$ |
| A4 | worst FV error $5.1\times10^{-6}$ | $7.4\times10^{-7}$ |

### Run 2 to run 3: an independent review

A reviewer agent re-derived the closed form, diffed the port against
upstream and checked the numbers against the logs. Its findings, all fixed
before run 3 with no gate changed:

1. **The A2 interface gate in PLAN.md had never been run**, although A2 was
   reported PASS. It now runs on every mode of A2a and A2b.
2. The minimum-concentration diagnostic started at 0, so it could not show
   a positive minimum. It now starts at $+\infty$.
3. A4, A6 and B1 printed no FV diagnostics. They do now.
4. B2's planned boon-lay FV leg had not been run. It now runs and is
   reported.
5. Several statements in this report were overstated or wrong. They are
   corrected here:
   - the independence of A1's closed form, and of the multilayer reference;
   - the run-1 wording and its rounding;
   - the size of STRATO's A1 error.

## The problem, and who solves it

$$\frac{\partial c}{\partial t} = \frac{1}{r^2}\frac{\partial}{\partial r}\left(r^2 D(r)\frac{\partial c}{\partial r}\right)$$

Boundary and interface conditions:
- symmetry at $r = 0$;
- $c$ and $D \, \partial c / \partial r$ continuous at each interface;
- $c = 0$ at the outer surface.

The initial condition is $c_0$ in the kernel only. There is no decay and no
source. The release fraction is $F = 1 - M(t)/M_0$.

**What boon-lay already had (Stage 0).** TRISO-ATOPS's `breakthrough_model`
is not a solver of this problem. It is a closed-form time-lag series through
one planar SiC slab, with an inexhaustible kernel. The numerical solver of
this problem in boon-lay is
`triso_atops_extensions::multilayer_diffusion` (gh:#1209), and that is what
was verified here. STRATO's finite-volume code is the same algorithm class,
so it was **not** added to the library. Only the Sturm-Liouville series was
missing from the workspace, and only it was ported.

### Equation to code

| Equation | STRATO port (`src/strato/sturm_liouville.rs`) | boon-lay FV (`src/triso_atops_extensions/multilayer_diffusion.rs`) |
|---|---|---|
| $u = r\phi$; in layer $i$, $u'' + (\lambda/D_i) u = 0$, carried as $(u, q = D u')$ | `:177` `transfer_layer` | |
| interface: $u$ continuous, $q_{i+1} = q_i + (D_{i+1} - D_i) u / R_i$ (flux $D(u' - u/r)$ continuous) | `:189` `interface_state` | |
| regular centre $u(0) = 0$; shooting outward | `:239` `shoot` | |
| eigenvalue isolation by Sturm oscillation count | `:273` `eigenvalue_count`; bisection on the count `:360`-`:365` | |
| $w_n = 3 I_{1,n} I_n / (R_1^3 N_n)$, 16-point Gauss-Legendre | `:508`; quadrature `:387`, `:415` | |
| $F(t) = 1 - \sum_n w_n e^{-\lambda_n t}$ | `:547` `release_fraction` | |
| truncation: valid for $t \ge 40/\lambda_{covered}$ | `:533` `min_valid_time` | |
| accuracy floor $2.2\times10^{-16}/\min(\text{flux ratio}, 1)$ | `:527` `accuracy_floor` | |
| Booth's one sphere (boon-lay addition, local change L5) | `:463` `single_sphere` | |
| cell-centred FV, faces at layer boundaries | | `:148` `LayeredSphere::new` |
| face conductance $G = A_f/(\Delta r_1/(2D_1) + \Delta r_2/(2D_2))$ | | `:208` `conductances` |
| backward Euler, tridiagonal solve, release $h G_{out} c_N$ | | `:314` `step`, `:331` |

## Results, run 3

All FV results are at $t = 2\times10^5$ s, with time-extrapolated errors
against the reference. The orders are between successive meshes.

**A1, equal $D = 10^{-12}$ m²/s.** The reference is the closed form,
$F = 0.999904399505$.

| cells/layer | boon-lay error | STRATO FV error | boon-lay order | STRATO order |
|---|---|---|---|---|
| 10 | $-9.16\times10^{-7}$ | $-1.43\times10^{-7}$ | | |
| 20 | $-2.28\times10^{-7}$ | $-3.60\times10^{-8}$ | 2.004 | |
| 40 | $-5.71\times10^{-8}$ | $-9.06\times10^{-9}$ | 2.000 | |
| 80 | $-1.43\times10^{-8}$ | $-2.32\times10^{-9}$ | 1.995 | 1.969 |
| 160 | $-3.63\times10^{-9}$ | $-6.28\times10^{-10}$ | 1.980 | 1.883 |

**A3, five layers.** $D$ = 1e-12, 4e-10, 8e-13, 2e-14, 7e-13 m²/s, synthetic.
The reference is the series, 32 modes, $F = 0.432555639093$.

| cells/layer | boon-lay error | STRATO FV error |
|---|---|---|
| 10 | $-1.491\times10^{-4}$ | $-8.76\times10^{-5}$ |
| 20 | $-3.725\times10^{-5}$ | $-2.19\times10^{-5}$ |
| 40 | $-9.31\times10^{-6}$ | $-5.48\times10^{-6}$ |
| 80 | $-2.33\times10^{-6}$ | $-1.37\times10^{-6}$ |
| 160 | $-5.82\times10^{-7}$ | $-3.42\times10^{-7}$ |

Both codes converge at order 2.000. Spatially extrapolated, boon-lay's error
is $1.7\times10^{-10}$.

**A2, interface conditions.** Every mode was re-shot from the public
`transfer_layer` (129 modes for A2a, 107 for A2b).
- $c$ is continuous exactly.
- The physical flux $D\,\partial\phi/\partial r$ is continuous to
  $2.5\times10^{-15}$ relative.
- $D u'$ jumps by up to 177 % (A2b), as it must.
- At the surface, $u(R)$ is at most $3\times10^{-13}$ of the largest $|u|$.

**A4, SiC contrast.** $D = 10^{-12}$ m²/s except SiC, where it is
$10^{-12}\rho$. The error is the worst of $0.5\bar{\tau}$, $\bar{\tau}$ and
$2\bar{\tau}$, extrapolated from 80 and 160 cells.

| $\rho$ | $\bar{\tau}$ [s] | modes | accuracy floor | FV error | balance | balance bound |
|---|---|---|---|---|---|---|
| $10^{-6}$ | $6.25\times10^{9}$ | 2 | $1.9\times10^{-10}$ | $2.8\times10^{-7}$ | $2.4\times10^{-7}$ | $2.8\times10^{-5}$ |
| $10^{-4}$ | $6.25\times10^{7}$ | 2 | $1.9\times10^{-12}$ | $4.8\times10^{-10}$ | $1.8\times10^{-9}$ | $2.8\times10^{-7}$ |
| $10^{-2}$ | $6.49\times10^{5}$ | 2 | $1.9\times10^{-14}$ | $4.8\times10^{-11}$ | $2.8\times10^{-11}$ | $2.9\times10^{-9}$ |
| $10^{2}$ | $2.36\times10^{4}$ | 7 | $1.8\times10^{-14}$ | $1.7\times10^{-9}$ | $1.6\times10^{-11}$ | $1.1\times10^{-8}$ |
| $10^{4}$ | $2.35\times10^{4}$ | 7 | $1.8\times10^{-12}$ | $2.5\times10^{-9}$ | $2.3\times10^{-9}$ | $1.1\times10^{-6}$ |
| $10^{6}$ | $2.35\times10^{4}$ | 7 | $1.8\times10^{-10}$ | $7.4\times10^{-7}$ | $1.9\times10^{-7}$ | $1.1\times10^{-4}$ |

- **The prediction partly missed.** PLAN.md expected the FV gate to be
  hardest at $\rho = 10^{-6}$. It was hardest at $10^{6}$: 300 times larger
  than at $10^{4}$.
- **A plausible cause, not proven.** At both extremes the FV error is close
  to the conservation residual: $1.9\times10^{-7}$ at $10^{6}$ and
  $2.4\times10^{-7}$ at $10^{-6}$. That residual is round-off in a very
  stiff tridiagonal solve ($h D/\Delta r^2$ up to about $10^{8}$). The error
  there is likely round-off, not discretisation.
- **Where the conservation gate stops meaning anything.** At $\rho = 10^{6}$
  and $10^{-6}$ the derived bound ($10^{-4}$ and $2.8\times10^{-5}$) exceeds
  the $2\times10^{-5}$ FV tolerance, so that gate cannot catch an error of the
  size being measured.
- **A4 does not check the spatial order.** It uses only 80 and 160 cells and
  assumes order 2.

**A6.** The temporal orders are 0.993, 0.996, 0.998 and 0.999. Balance,
bound, minimum $c$ and wall time are in the log.

**A7.** With $\lambda_{max} = m/2000$ s and $m$ = 10, 20, 30, 45, 60, the
series uses 15, 22, 25, 32 and 37 modes. At resolved times $F$ moves by at
most $4.3\times10^{-13}$. $F(2000\text{ s})$ is unresolved until
$m \ge 45$, and is then $-8.5\times10^{-14}$: zero to round-off.

**B2.** ATOPS's `booth_longlived` and the one-sphere adapter agree to
$5.6\times10^{-16}$ at $Dt/R^2$ = $10^{-3}$, $10^{-2}$, 0.1 and 1. boon-lay
FV on the same sphere (160 cells) differs from them by:

| $Dt/R^2$ | FV error |
|---|---|
| $10^{-3}$ | $-2.1\times10^{-4}$ |
| $10^{-2}$ | $-5.7\times10^{-5}$ |
| 0.1 | $-8.8\times10^{-6}$ |
| 1 | $-6.4\times10^{-9}$ |

The early-time error is the thin boundary layer at the surface, which 160
uniform cells do not resolve.

**Diagnostics.**
- **boon-lay:** every run conserves within its derived bound. The largest
  printed residual away from the A4 extremes is $1.2\times10^{-9}$ (A6,
  $\Delta t = 250$ s). No minimum concentration is negative.
- **STRATO:** its balance residual stays below $2.1\times10^{-23}$ in
  absolute units (it Kahan-sums its release).

## What the comparisons do and do not show

- ~~**The port reproduces upstream exactly.** U1 is bit-identical on 978 of
  978 compared values. It compares eigenvalues, weights, `lambda_covered`,
  `accuracy_floor` and $F$; upstream values that are not finite would be
  skipped, and none occurred. The std build calls the same libm as
  upstream. **The no_std build is only compiled, never run**, so its
  last-bit behaviour is unmeasured.~~
  **CORRECTED 2026-11-26 (local change L8, maintainer: "petir is intended"):**
  the port's transcendentals now go through PETIR, not the platform libm
  upstream uses, so the std build no longer reproduces upstream bit for bit.
  U1: 767 of 978 values bit-identical. Eigenvalues, weights,
  `lambda_covered` and `accuracy_floor` agree to $2.5 \times 10^{-14}$
  relative. The release fraction $F(t) = 1 - \sum w e^{-\lambda t}$ agrees to
  $4.4 \times 10^{-15}$ absolute; its early-time values cancel to
  $\sim 10^{-15}$ (port $-6.7 \times 10^{-16}$ against upstream
  $2.4 \times 10^{-15}$ on `a2a_two_layer`), where a relative difference is
  meaningless, so $F(t)$ is now gated at $10^{-14}$ absolute (maintainer
  decision 2026-11-26). The cause is understood: one-ulp differences
  between PETIR's and the platform's `cos`, `sin`, `exp`, ..., carried
  through the sums.
- **The series is independently checked only where there is no real
  interface.**
  - A1 has equal $D$, so the interface term is exactly zero.
  - A1's closed form is the same formula as upstream's own test oracle
    (`modal_release_matches_closed_form_for_equal_diffusivity`). A1 largely
    repeats that check.
  - B2 has one layer.
  - U2's oracle is an independent *implementation*, the same author's mpmath
    script, but of the same eigen-series, not a different method.
- **For A2 to A4, the series and boon-lay's FV support each other; they are
  not reference and target.** That a method-independent series and an
  independent-code FV converge to the same value at the predicted orders is
  strong evidence. It is not an independent oracle. The A2 interface check
  confirms the rule the series uses is the physical one, flux continuity of
  $D\,\partial\phi/\partial r$ rather than of $D u'$.
- **boon-lay's FV is second-order in space and first-order in time**,
  wherever convergence was measured.
- **The two FV codes implement the same method.** Their agreement shows that
  both are implemented correctly, not that the method is right. At equal
  mesh, STRATO's FV is 1.7 times (A3) to 6 times (A1) more accurate. It
  places nodes at volume centroids where boon-lay uses mid-radii.

## Limitations and open findings

- A4 evaluates at late times ($\ge 0.5\bar{\tau}$), where only 2 to 7 modes
  matter, and checks no order.
- STRATO's A1 order drops to 1.883 at 160 cells (error
  $-6.3\times10^{-10}$). That is within the gate but below boon-lay's 1.980.
  It is not investigated.
- The absolute FV gate $2\times10^{-5}$ is STRATO's, adopted rather than
  derived. Two cases test little against it:
  - A2a at $2\times10^5$ s has $F = 6.0\times10^{-5}$. Its relative error
    is $1.3\times10^{-5}$ extrapolated, and $6\times10^{-3}$ at 160 cells.
  - A1's FV is checked only near $F = 1$.
- `breakthrough_model` (B4) is not verified against a multilayer solution.
  Comparing it would measure model-form error, which is a separate study.
- The diffusivities are synthetic. Nothing here says how fast real fission
  products leave real TRISO.
- `kovan-cli index --crates boon-lay` wrote `src/strato/kovan.toml`. It also
  rewrote 41 other boon-lay `kovan.toml` files: the committed index never
  covered `multilayer_diffusion.rs`, but a single-crate run may also lose
  cross-crate reaching tests. Those rewrites were not kept; regenerate the
  index through the partitioned pipeline.

## Reproducing

1. **Reference files.** Clone STRATO at `36f1a98` into
   `dev/strato_reference/upstream/STRATO`, then run
   `cargo run --release -- ../../tests/data/strato_reference` in
   `dev/strato_reference/` (see its README).
   - On 2026-11-26 re-running it reproduced the original 20 FV files byte
     for byte.
   - Provenance is in `tests/data/strato_reference/PROVENANCE.md`.
2. **Tests.** Run the command above, or
   `cargo test --release -p boon-lay --lib strato` for the port's 13 unit
   tests: 7 ported from upstream and 6 of boon-lay's own.

## Review still needed

- A human review of the port (`src/strato/`) against upstream. The local
  changes L1 to L7 are listed in `src/strato/mod.rs`.
- A human review of the protocol correction. The gates did not move, but the
  procedure changed after a failure. It was predicted, recorded and met, and
  its size was checked afterwards.
- A decision on whether boon-lay's FV should adopt STRATO's centroid nodes:
  1.7 to 6 times smaller error at the same order. It is not proposed here.
- An independent check of the series on a genuinely layered case.
  Candidates: a published composite-sphere benchmark, or a different method
  such as Laplace-transform inversion.
