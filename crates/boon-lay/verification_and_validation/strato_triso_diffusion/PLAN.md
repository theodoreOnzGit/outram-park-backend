# STRATO integration and TRISO diffusion verification: reconnaissance and predeclared plan

**Written 2026-11-26, before any implementation code.** This file is the
immutable record of what was found (Stage 0) and what will be checked, and
how (Stage 1). Results go in [`README.md`](README.md) beside it. If a gate
below fails, the gate stays as written: the failure is reported and
diagnosed, never repaired by moving the gate. Any later correction to this
file is struck through and dated, never silently edited.

**Credit.** STRATO is the work of **Wee Zhern Ray (GitHub `weewzr`)**,
<https://github.com/weewzr/STRATO>. Everything in boon-lay derived from it
says so in its header.

Research, education and V&V only (`RESPONSIBLE_USE.md`). Numerical
code-to-code and analytical agreement is **verification**, not validation:
nothing here compares against an experiment.

## Stage 0: reconnaissance

### What boon-lay already has

| Capability | Where | Reviewed (kovan)? |
|---|---|---|
| TRISO-ATOPS `breakthrough_model`: closed-form Daynes-Barrer **planar** membrane time-lag series through **one** SiC layer, constant-concentration (inexhaustible) kernel source, perfect sink outside, scaled by the kernel surface-to-volume ratio `3/r`, clamped to [0, 1]. 999 terms. No grid, no time stepping, no interfaces. | `src/triso_atops_fork/release_models/steady_state.rs::breakthrough_model` | **yes**, closed to modification |
| TRISO-ATOPS `booth_longlived`: Crank's homogeneous-sphere series `1 - (6/pi^2) sum (1/n^2) exp(-n^2 pi^2 D t / a^2)`, uniform start, `c = 0` surface. 4999 terms. | `steady_state.rs::booth_longlived` | **yes** |
| A **numerical multilayer spherical diffusion solver**: cell-centred finite volume, uniform cells per layer with every layer boundary on a face, cell-midpoint nodes, series-resistance face conductance `G = A_f / (dr_1/(2 D_1) + dr_2/(2 D_2))`, symmetry at `r = 0`, `c = 0` at the surface, backward Euler, `petir::linalg::solve_symm_tridiag`, optional decay, release and decay accounting. NOT a port (gh:#1209, 2026-11-04). | `src/triso_atops_extensions/multilayer_diffusion.rs` (`LayeredSphere`, `LayeredDiffusion`) | no |
| Walk-on-Spheres Lagrangian multilayer engine (stochastic). Its own docs say it cannot cross the buffer/PyC boundary in practice. | `src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/` | partly |

Searched the whole workspace (2026-11-26) for STRATO, Sturm-Liouville,
eigenfunction, transfer-matrix and composite-sphere diffusion code:
**none exists.** The only eigenfunction code is neutronics (`bedok`) and a
Graetz series (`tuas_boussinesq_solver`). So:

- **STRATO's Sturm-Liouville solver is missing** from the workspace, and
  is the capability to integrate.
- **STRATO's finite-volume solver duplicates the algorithm class** of
  `multilayer_diffusion.rs`: both are cell-centred FV with backward Euler and
  series-resistance faces. They differ in node placement (STRATO uses the
  cell's volume centroid `0.75 (b^4 - a^4)/(b^3 - a^3)`, boon-lay the
  mid-radius), in the boundary conductance, and in release summation (STRATO
  Kahan-sums; boon-lay sums directly). **It is not ported into the library**,
  by the workspace's search-before-building rule. It is run **as upstream
  code**, unmodified, in an external harness that writes reference files
  (`dev/strato_reference/`), so it serves as an independent implementation
  without becoming a second production solver.

### The integration target

The brief names "BOON-LAY's TRISO-ATOPS intact-particle diffusion solver".
**TRISO-ATOPS has no numerical intact-particle solver.** Upstream
`breakthrough_model` (read at INL commit `0e87d36`, v2.2.0, and identical at
the fork pin `de374c8`) is the closed-form planar series above. The
numerical solver of the multilayer problem in boon-lay's ATOPS area is the
`triso_atops_extensions` solver, whose own docs call it "the reference the
port's analytical formulas are measured against". So:

- The **numerical target** of both pathways is
  `triso_atops_extensions::multilayer_diffusion` ("boon-lay FV" below).
- The **ported ATOPS closed forms** enter only where they solve the same
  mathematical problem:
  - `booth_longlived` **does**: one homogeneous sphere, uniform start,
    `c = 0` surface, no decay.
  - `breakthrough_model` **does not**: it is planar, has an inexhaustible
    source and one layer. A comparison with the five-layer finite-inventory
    problem is a model-form difference, not verification, so it is
    **BLOCKED** (case B4).

### Upstream provenance and licences

| | STRATO | TRISO-ATOPS |
|---|---|---|
| Repository | <https://github.com/weewzr/STRATO> | <https://github.com/IdahoLabResearch/TRISO-ATOPS> |
| Commit read | `36f1a98848bec9d66fe515994e075cff09d6cefb` (2026-11-25) | HEAD `0e87d36d0963c2d94e19ec7203d04ad51575e3bd` (v2.2.0); fork pin `de374c881743938c25c01a71a9adb8fbfd488f95` |
| Author | Wee Zhern Ray (`weewzr`) | Battelle Energy Alliance, LLC (INL) |
| Licence | **AGPL-3.0-only** since commit `8c44fb7` (2026-11-10). Tags `v1.0.0` and `v1.0.1` were MIT. | MIT |
| Compatible with boon-lay (AGPL-3.0-only) | yes, same licence | yes |

STRATO's Rust sources carry no headers; its `CITATION.cff` gives the alias
`weewzr`. STRATO is not derived from ATOPS: its source never refers to it,
and its planning notes mention ATOPS and outram-park only for web layout.

### Workspace conventions confirmed

- **`no_std`:** boon-lay is `no_std` + `alloc` without its default `std`
  feature (`src/lib.rs:15`). Float methods resolve to std's inherent methods
  with `std`, and to `petir::real::Real` (libm) through
  `crate::nostd_prelude` without it. Every method STRATO uses (`sin`, `cos`,
  `tan`, `atan2`, `sqrt`, `exp`, `powi`, `floor`, `ceil`, `abs`) exists on
  `Real`. A `std` build of a port therefore computes with the **same** libm
  calls as upstream.
- **API units:** `uom` at the API (workspace rule); the existing solver takes
  `Length`, `DiffusionCoefficient`, `Time`.
- **Rules that bind this work:**
  - No macros.
  - No trait objects.
  - Every new function reached by a test.
  - Release builds only.
  - Kovan-reviewed functions are not modified: `breakthrough_model` and
    `booth_longlived` are **called**, never edited.
  - The new module is added to the existing `scripts/check-nostd.sh` gate,
    which already covers boon-lay.

## Stage 1: the problem, the equivalences, and the gates

### Common problem

$$\frac{\partial c}{\partial t} = \frac{1}{r^2}\frac{\partial}{\partial r}\left(r^2 D(r)\frac{\partial c}{\partial r}\right), \quad D(r) = D_i \text{ on } R_{i-1} < r < R_i$$

Boundary and interface conditions:
- symmetry at the centre: $\partial c / \partial r = 0$ at $r = 0$;
- $c$ and $D \, \partial c/\partial r$ continuous at every $R_i$;
- $c(R_N, t) = 0$ at the outer surface.

Initial condition: $c = c_0$ on $r < R_1$ and $0$ elsewhere.

No decay, source, trapping, temperature dependence or failure. Release
fraction:

$$F(t) = 1 - \frac{M(t)}{M_0}$$

#### Which implementation solves which problem

| Implementation | Solves the common problem? | Notes |
|---|---|---|
| STRATO Sturm-Liouville (port) | yes, any $N \ge 2$ layers | $N = 1$ (Booth) needs the adapter below |
| STRATO FV (upstream, harness) | yes, exactly 5 layers (`Config::validate`) | |
| boon-lay FV | yes, any $N \ge 1$, decay set to 0 | |
| ATOPS `booth_longlived` | only $N = 1$ with $R_1 = R_N$ | uniform start |
| ATOPS `breakthrough_model` | **no** | planar, inexhaustible source, one layer |

**Adapter, stated in advance.** Upstream's `validate_layers` requires at
least two layers, so the port cannot pose Booth's single sphere with a
uniform start. The port adds one explicit, documented constructor,
`ModalRelease::single_sphere`. It runs the same shooting, counting and
weight code with one layer and skips only upstream's two-layer check. It is
not upstream behaviour, and is marked as a local change. Upstream's `new` is
unchanged.

### Independent references

| Source | Role | Independent of |
|---|---|---|
| Homogeneous-sphere closed form, written for this study in the test file from $u_n = \sin(n\pi r/R)$ | mathematical reference for A1 | all of STRATO, boon-lay FV, ATOPS |
| Crank's uniform-sphere series (ATOPS `booth_longlived`) | reference for B2 | STRATO; boon-lay FV |
| STRATO's Python oracle value `F(2e5 s) = 0.4325556390930383` (upstream `tests/finite_volume_vs_analytical.rs`) | external spot value for the synthetic five-layer case | the Rust port (it is an upstream artefact, not re-derived here) |
| STRATO Sturm-Liouville (port) | multilayer reference | both FV solvers |
| STRATO FV (upstream, unmodified, harness) | independent numerical implementation | boon-lay FV in code (same algorithm class: **not** algorithmically independent) |
| boon-lay FV | integration target | STRATO |

For the closed form, with $k_n = n\pi/R$:

$$\lambda_n = D k_n^2$$

$$I_{1,n} = \frac{\sin(k_n R_1) - k_n R_1 \cos(k_n R_1)}{k_n^2}$$

$$I_{n} = \frac{(-1)^{n+1} R^2}{n\pi}$$

$$N_n = \frac{R}{2}$$

$$w_n = \frac{3 I_{1,n} I_n}{R_1^3 N_n}$$

Forbidden, by the brief and the workspace rules:
- generating an analytical expectation from either FV solver;
- calibrating any implementation against another;
- changing a physical parameter to improve agreement;
- regenerating reference data because a test failed.

### Inputs

All diffusivities are **synthetic verification inputs**, not literature
data.

- **Geometry G5** (STRATO's bundled synthetic case): $R$ = 200, 300, 350,
  400, 450 µm (kernel, buffer, IPyC, SiC, OPyC).
- **Diffusivities D5** (m²/s): $1\times10^{-12}$, $4\times10^{-10}$,
  $8\times10^{-13}$, $2\times10^{-14}$, $7\times10^{-13}$.
- **Initial concentration:** $c_0 = 1$ (the release fraction is
  independent of it).
- **Output times:** $2\times10^{3}$, $10^{4}$, $5\times10^{4}$ and
  $2\times10^{5}$ s, unless a case says otherwise.

### Benchmark matrix

**Upstream differential (Stage 2).**

| Case | Inputs | Compared | Gate |
|---|---|---|---|
| U1 | G5/D5, A1, A2a, A2b and A4 layer sets | port vs upstream STRATO SL: eigenvalues, weights, $F(t)$, `lambda_covered`, `accuracy_floor` | relative difference $\le 10^{-12}$ (upstream's own regression tolerance). The count bit-identical is reported. |
| U2 | G5/D5 | port $F(2\times10^5)$ vs upstream Python oracle 0.4325556390930383 | absolute $\le 10^{-10}$ (upstream's gate) |

Predicted for U1: **bit-identical** on this machine. A `std` build calls the
same libm through the same toolchain. Elsewhere, a 1-ulp libm difference is
possible, which is why the gate is $10^{-12}$ and not equality.

**Pathway A: analytical.**

| Case | Inputs | Compared | Gate | Prediction |
|---|---|---|---|---|
| A1 | G5 radii, all $D = 10^{-12}$ | SL vs closed form; boon-lay FV vs both | SL vs closed form: absolute $\le 10^{-10}$ at $t \ge$ `min_valid_time` | ~$10^{-13}$ |
| A2a | $R_1 = 200$ µm with $D_1 = 10^{-12}$; $R_2 = 450$ µm with $D_2 = 10^{-14}$ | SL vs boon-lay FV; SL eigenfunction interface checks | $c$ and $D\,\partial c/\partial r$ continuous at $R_1$, relative $\le 10^{-12}$; FV gates as A5 | pass |
| A2b | as A2a with $D_1 \leftrightarrow D_2$ | as A2a | as A2a | pass |
| A3 | G5/D5 | SL vs boon-lay FV, Richardson-extrapolated in space and time | absolute $\le 2\times10^{-5}$ at $t = 2\times10^5$ s (upstream's gate for the same quantity) | pass |
| A4 | G5 radii, $D = 10^{-12}$ except SiC $D = 10^{-12}\rho$, $\rho \in \{10^{-6}, 10^{-4}, 10^{-2}, 10^{2}, 10^{4}, 10^{6}\}$; times $0.5\bar\tau$, $\bar\tau$, $2\bar\tau$ with the exact mean exit time $\bar\tau = R_1^2/(15 D_1) + \sum_l (R_{o,l}^2 - R_{i,l}^2)/(6 D_l)$ | SL `accuracy_floor`; SL vs extrapolated boon-lay FV | SL usable as reference iff `accuracy_floor` $\le 10^{-8}$; then absolute $\le 2\times10^{-5}$ | floor grows with contrast; all six usable; FV gate may be hardest at $\rho = 10^{-6}$ |
| A5 | G5/D5, 10, 20, 40, 80, 160 cells per layer; each mesh at $\Delta t$ and $\Delta t/2$, time-extrapolated (BE order 1) | spatial error vs SL at $t = 2\times10^5$ s | observed order on the two finest pairs in [1.8, 2.2] | 2.0 |
| A6 | G5/D5, 160 cells per layer; $\Delta t = T/50, T/100, \dots, T/1600$, $T = 2\times10^5$ s | temporal self-convergence of $F(T)$ | order in [0.8, 1.2] on the finest triples | 1.0 (BE; the initial discontinuity has been smoothed by $T$) |
| A7 | G5/D5 SL with $\lambda_{max} = m/t_{min}$, $m \in \{10, 20, 30, 45, 60\}$ | $F$ at the A times vs $m$; earliest valid time 40/`lambda_covered` | for $t \ge$ `min_valid_time`: successive-$m$ change $\le 10^{-12}$. Values at earlier times are reported as unresolved, never compared. | pass |

**Pathway B: code-to-code.**

| Case | Inputs | Compared | Gate | Prediction |
|---|---|---|---|---|
| B1 | A1 and A3 inputs; the same cells per layer (10 to 160) and the same $\Delta t$ in both codes | STRATO FV (upstream) and boon-lay FV, each against SL | each converges at spatial order in [1.8, 2.2]; their difference is reported, not gated | **not** bit-identical (different node placement); the difference shrinks like $h^2$ |
| B2 | one sphere $R = 450$ µm, $D = 10^{-12}$, uniform start; $Dt/R^2 \in \{10^{-3}, 10^{-2}, 0.1, 1\}$ | ATOPS `booth_longlived` vs SL `single_sphere` adapter vs boon-lay FV | `booth_longlived` vs SL absolute $\le 10^{-10}$ | pass; 4999 terms leaves a tail far below the gate at these times |
| B3 | five-layer STRATO FV vs ATOPS | | **BLOCKED** | STRATO FV needs exactly five regions with a kernel-only start; ATOPS has no five-layer model |
| B4 | ATOPS `breakthrough_model` vs the five-layer problem | | **BLOCKED** | not the same equation (planar, inexhaustible source, one layer) |

### Diagnostics recorded for every FV run

For every FV run the report records:
- initial and remaining inventory, and cumulative release;
- $F$ and the relative conservation residual;
- minimum concentration;
- cells per layer and $\Delta t$;
- mode count and `min_valid_time` for the reference;
- absolute and relative errors and observed orders;
- wall time, with the hardware line (`HardwareInfo::headline`).

Where $F < 10^{-6}$ the error is reported as absolute only.

**Gates that apply to every boon-lay FV run:**
- **Conservation:** $|1 - \text{inside} - \text{released}| \le \epsilon \cdot \max(1, \max h D/\Delta r^2) \cdot \text{steps}$. This is the bound derived in `multilayer_diffusion.rs`, with $\epsilon = 2.2\times10^{-16}$.
- **Positivity:** minimum cell atoms $\ge -\epsilon \cdot \max$ cell atoms. Backward Euler gives an M-matrix, so the values are non-negative in exact arithmetic.

### Known limitations, stated before running

- A1 is the only reference independent of every implementation. A2 to A7
  rest on STRATO's SL, which is itself checked against A1, U2 and its own
  interface tests.
- STRATO FV and boon-lay FV share an algorithm class. Their agreement shows
  implementation correctness, not method correctness; method correctness
  comes from convergence to SL.
- The tolerance of $2\times10^{-5}$ is upstream's, chosen by STRATO for its
  own extrapolated FV. It is adopted, not derived here.
- The STRATO FV reference files are generated on one machine, with the
  toolchain and libm recorded. Re-generation is reproducible
  (`dev/strato_reference/`), but floating-point results may differ by ulps
  on another libm.
