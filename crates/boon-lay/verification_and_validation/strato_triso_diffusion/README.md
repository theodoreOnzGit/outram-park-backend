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
and the dated protocol correction after run 1, is in [`PLAN.md`](PLAN.md).
Raw logs: [`run1_2026-11-26.log`](run1_2026-11-26.log) (the failed run) and
[`run2_2026-11-26.log`](run2_2026-11-26.log) (the counted run).

## Outcome

| Case | What | Outcome | Measured | Gate |
|---|---|---|---|---|
| U1 | port vs upstream STRATO Sturm-Liouville, 15 cases | **PASS** | **978 of 978 values bit-identical** | relative $\le 10^{-12}$ |
| U2 | port vs STRATO's Python oracle, $F(2\times10^5\text{ s})$ | **PASS** | $2.3\times10^{-13}$ | absolute $\le 10^{-10}$ |
| A1 | series vs independent closed form (equal $D$) | **PASS** | $1.8\times10^{-15}$ | absolute $\le 10^{-10}$ |
| A1 | boon-lay FV, spatial order / extrapolated error | **PASS** in run 2 (FAIL in run 1) | 1.995, 1.980 / $6.6\times10^{-11}$ | [1.8, 2.2] / $\le 2\times10^{-5}$ |
| A2a | two layers, $D$ drops 100× outward | **PASS** | 2.009, 2.002 / $7.9\times10^{-10}$ | as A1 |
| A2b | two layers, $D$ rises 100× outward | **PASS** | 2.002, 2.001 / $9.7\times10^{-9}$ | as A1 |
| A3/A5 | five-layer synthetic TRISO, mesh refinement | **PASS** in run 2 (FAIL in run 1) | 2.000, 2.000 / $1.7\times10^{-10}$ | as A1 |
| A4 | SiC contrast $10^{-6}$ to $10^{6}$ | **PASS** | floor $\le 1.9\times10^{-10}$; FV $\le 7.4\times10^{-7}$ | floor $\le 10^{-8}$; FV $\le 2\times10^{-5}$ |
| A6 | temporal self-convergence, 160 cells per layer | **PASS** | 0.998, 0.999 | [0.8, 1.2] |
| A7 | modal convergence | **PASS** | largest change at resolved times $4\times10^{-13}$ | $\le 10^{-12}$ |
| B1 | STRATO FV and boon-lay FV, each vs the series | **PASS** in run 2 (FAIL in run 1) | STRATO 1.97, 1.88 (A1), 2.000, 2.000 (A3); boon-lay as A1 and A3 | [1.8, 2.2] |
| B2 | ATOPS `booth_longlived` vs the series' one-sphere adapter | **PASS** | $5.6\times10^{-16}$ | absolute $\le 10^{-10}$ |
| B3 | STRATO FV vs ATOPS, five layers | **BLOCKED** | | ATOPS has no five-layer model |
| B4 | ATOPS `breakthrough_model` vs the five-layer problem | **BLOCKED** | | not the same equation |

Run 2: 11 tests passed, 0 failed, 2.2 s.

Command:

```bash
cargo test --release -p boon-lay --test strato_triso_diffusion_verification -- --nocapture --test-threads=1
```

Hardware: Intel Xeon @ 2.30 GHz, 4 cores, 15.7 GiB RAM, CPU only, Linux.
Toolchain: rustc 1.99.1.

## Run 1 failed, and why that is recorded

Run 1 missed the spatial-order gate on A1, A3 and B1 (8 passed, 3 failed).

| Case | Measured order | Gate |
|---|---|---|
| A1 | $-0.29$ | [1.8, 2.2] |
| A3 | 1.80 | [1.8, 2.2] |
| STRATO's own FV on A1 | $-0.04$ | [1.8, 2.2] |

On A1 both FV codes, which are independent implementations, sat on the
**same** error, $+2.5\times10^{-7}$, at every mesh from 20 cells up. Their
mutual difference fell at order 2.00. A floor that two codes share and that
does not move with the mesh is not a spatial error. It is the $O(\Delta t^2)$
part of backward Euler's error, which one-step Richardson in time does not
remove.

The protocol was corrected, not the gate. Two-step Richardson at
$\Delta t = 200, 100, 50$ s replaced one-step at 500 and 250 s. The
prediction was committed before re-running (`PLAN.md`, "Protocol
correction"), and run 2 met it: A1's residual fell from
$2.5\times10^{-7}$ to $6.6\times10^{-11}$.

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
was verified here. STRATO's finite-volume code is the same algorithm class
(cell-centred FV, backward Euler, series-resistance faces), so it was **not**
added to the library. Only the Sturm-Liouville series was missing from the
workspace, and only it was ported.

### Equation to code

| Equation | STRATO port | boon-lay FV |
|---|---|---|
| $u = r\phi$; in layer $i$, $u'' + (\lambda/D_i) u = 0$, carried as $(u, q = D u')$ | `sturm_liouville.rs:177` `transfer_layer` | |
| interface: $u$ continuous, $q_{i+1} = q_i + (D_{i+1} - D_i) u / R_i$ (flux $D(u' - u/r)$ continuous) | `sturm_liouville.rs:189` `interface_state` | |
| regular centre $u(0) = 0$; shooting outward | `sturm_liouville.rs:239` `shoot` | |
| eigenvalue isolation by Sturm oscillation count | `:273` `eigenvalue_count`, `:304` `eigenvalues`, `:313` bisection | |
| $w_n = 3 I_{1,n} I_n / (R_1^3 N_n)$, 16-point Gauss-Legendre | `:508`; quadrature `:387`, `:415` | |
| $F(t) = 1 - \sum_n w_n e^{-\lambda_n t}$ | `:547` `release_fraction` | |
| truncation: valid for $t \ge 40/\lambda_{covered}$ | `:533` `min_valid_time` | |
| accuracy floor $2.2\times10^{-16}/\min(\text{flux ratio}, 1)$ | `:527` `accuracy_floor` | |
| Booth's one sphere (boon-lay addition, local change L5) | `:463` `single_sphere` | |
| cell-centred FV, faces at layer boundaries | | `multilayer_diffusion.rs:148` `LayeredSphere::new` |
| face conductance $G = A_f/(\Delta r_1/(2D_1) + \Delta r_2/(2D_2))$ | | `:208` `conductances` |
| backward Euler, symmetric tridiagonal solve, release $h G_{out} c_N$ | | `:314` `step`, `:331` |

## Results, run 2

All FV results are at $t = 2\times10^5$ s, with time-extrapolated errors
against the reference. The orders are between successive meshes.

**A1, equal $D = 10^{-12}$ m²/s.** The reference is the closed form,
$F = 0.999904399505$. The series agrees with the closed form to
$1.8\times10^{-15}$ at all four times.

| cells/layer | boon-lay error | order |
|---|---|---|
| 10 | $-9.16\times10^{-7}$ | |
| 20 | $-2.28\times10^{-7}$ | 2.004 |
| 40 | $-5.71\times10^{-8}$ | 2.000 |
| 80 | $-1.43\times10^{-8}$ | 1.995 |
| 160 | $-3.63\times10^{-9}$ | 1.980 |

**A3, five layers.** $D$ = 1e-12, 4e-10, 8e-13, 2e-14, 7e-13 m²/s, synthetic.
The reference is the series, 32 modes, $F = 0.432555639093$.

| cells/layer | boon-lay error | STRATO FV error | order (both) |
|---|---|---|---|
| 10 | $-1.491\times10^{-4}$ | $-8.76\times10^{-5}$ | |
| 20 | $-3.725\times10^{-5}$ | $-2.19\times10^{-5}$ | 2.00 |
| 40 | $-9.31\times10^{-6}$ | $-5.48\times10^{-6}$ | 2.00 |
| 80 | $-2.33\times10^{-6}$ | $-1.37\times10^{-6}$ | 2.00 |
| 160 | $-5.82\times10^{-7}$ | $-3.42\times10^{-7}$ | 2.000 |

Spatially extrapolated, boon-lay's error is $1.7\times10^{-10}$.

**A4, SiC contrast.** $D = 10^{-12}$ m²/s everywhere except SiC, where it is
$10^{-12}\rho$. Times are $0.5\bar\tau$, $\bar\tau$ and $2\bar\tau$.

| $\rho$ | $\bar\tau$ [s] | modes | accuracy floor | FV extrapolated error |
|---|---|---|---|---|
| $10^{-6}$ | $6.25\times10^{9}$ | 2 | $1.9\times10^{-10}$ | $2.8\times10^{-7}$ |
| $10^{-4}$ | $6.25\times10^{7}$ | 2 | $1.9\times10^{-12}$ | $4.8\times10^{-10}$ |
| $10^{-2}$ | $6.49\times10^{5}$ | 2 | $1.9\times10^{-14}$ | $4.8\times10^{-11}$ |
| $10^{2}$ | $2.36\times10^{4}$ | 7 | $1.8\times10^{-14}$ | $1.7\times10^{-9}$ |
| $10^{4}$ | $2.35\times10^{4}$ | 7 | $1.8\times10^{-12}$ | $2.5\times10^{-9}$ |
| $10^{6}$ | $2.35\times10^{4}$ | 7 | $1.8\times10^{-10}$ | $7.4\times10^{-7}$ |

As predicted, the series' accuracy floor grows in proportion to the
contrast. All six stay usable as references.

**A6.** The temporal self-convergence orders are 0.993, 0.996, 0.998 and
0.999, the first order expected of backward Euler.

**A7.** With $\lambda_{max} = m/2000$ s and $m$ = 10, 20, 30, 45, 60, the
series uses 15, 22, 25, 32 and 37 modes. At every time where both are
resolved, $F$ moves by at most $4\times10^{-13}$. $F(2000\text{ s})$ is
unresolved until $m \ge 45$, and is then $-8.5\times10^{-14}$, i.e. zero to
round-off.

**B2.** ATOPS's ported `booth_longlived` and the series' one-sphere adapter
agree to $5.6\times10^{-16}$ at $Dt/R^2$ = $10^{-3}$, $10^{-2}$, 0.1 and 1.

**Diagnostics.** These hold on every boon-lay FV run:
- conservation within the derived bound (worst $8.5\times10^{-10}$, A3 at 160 cells, where $h D/\Delta r^2$ is largest in the buffer; the bound is $\epsilon \cdot \max(h D/\Delta r^2) \cdot$ steps);
- no negative concentration;
- STRATO's own balance residual below $2.1\times10^{-23}$ in absolute concentration units (it Kahan-sums its release).

## What the comparisons do and do not show

- **The port reproduces upstream exactly.** U1 is bit-identical on all 978
  values on this machine, as predicted. A std build calls the same libm. A
  no_std build goes through petir/libm and may differ in the last bits.
- **The series is right where it can be checked independently.** It matches
  the independent closed form (A1), Booth's series in ATOPS (B2) and STRATO's
  Python oracle (U2) to round-off.
- **boon-lay's FV is second-order in space and first-order in time on every
  case, and converges to the series.** That includes two-layer jumps either
  way, five layers, and contrasts up to $10^{\pm 6}$.
- **The two FV codes are implementations of the same method, not
  independent methods.** Their agreement shows that both are implemented
  correctly; convergence to the series is what shows the method is right. At
  equal mesh, STRATO's FV is about 1.7 times more accurate on A3. It places
  its nodes at volume centroids where boon-lay uses mid-radii. This is a
  difference in constant, not in order.

## Limitations and open findings

- A4 evaluates at late times ($\ge 0.5\bar\tau$), where only 2 to 7 modes
  matter. It tests the slow modes at high contrast, not the full spectrum.
- STRATO's FV on A1 converges at 1.97 and then 1.88. The second value is
  inside the gate but lower than boon-lay's 1.98. At 160 cells STRATO's
  error is about $3\times10^{-9}$, near the residual temporal floor. This
  is plausible but not proven; it is not investigated further here.
- The $2\times10^{-5}$ FV tolerance is STRATO's own, adopted rather than
  derived. Every result beats it by at least 27 times.
- TRISO-ATOPS's `breakthrough_model` (B4) remains unverified against a
  multilayer solution by this work, because it is a different mathematical
  problem. Comparing it with the five-layer series would measure
  **model-form** error (planar slab, infinite source, one layer). That is a
  separate question, not verification, and is not done here.
- The diffusivities are synthetic. Nothing here says how fast real fission
  products leave real TRISO.

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
  procedure changed after a failure. It was predicted, recorded and met.
- The decision on whether boon-lay should gain STRATO's centroid node
  placement: about 1.7 times smaller error at the same order. It is not
  proposed here.
