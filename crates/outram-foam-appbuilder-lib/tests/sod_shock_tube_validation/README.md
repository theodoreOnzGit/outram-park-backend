<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore
Part of Outram Park (outram-park-backend), outram-foam-appbuilder-lib.
-->

# Sod Shock Tube Validation — Rust `rhoCentralFoam` port vs Sod (1978) Table II

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


This directory holds the **validation** case for the Rust port of OpenFOAM
`rhoCentralFoam` (KNP central-upwind flux, 2nd-order vanLeer MUSCL). The port is
judged against the published benchmark profile in **Sod (1978) Table II**.

> The companion **tutorial** case
> (`tutorials/rho_central_foam_shock_tube.rs`) instead compares the Rust port
> against an OpenFOAM `rhoCentralFoam` reference run. This validation case
> compares the Rust port against Sod's published table.

- Methodology, exact-Riemann arbiter, and pass criteria: see `CLAUDE.md` in this
  directory and the module doc comment in `main.rs`.
- Numbers below were produced by the `rho_central_foam_matches_sod_table_ii` test
  (2026-07-06; 100 cells, `dt = 1e-6 s`, run to Sod's canonical $\tau = 0.2$,
  i.e. $t = 6.3246 \times 10^{-3}$ s). Regenerate with:

  ```bash
  cargo test -p outram-foam-appbuilder-lib --test sod_shock_tube_validation \
    rho_central_foam_matches_sod_table_ii -- --nocapture
  ```

  That same run auto-writes a **plottable per-cell CSV** overlaying the
  numerical profile on the analytic exact Riemann solution at every cell centre
  (columns `x_m, rho_numerical, u_numerical, p_numerical, rho_exact, u_exact,
  p_exact`, with $L_2$/$L_\infty$ error norms as `#`-comment header lines) to
  `verification_and_validation/sod_shock_tube_validation/sod_shock_tube_profile_vs_exact_riemann.csv`.
  The whole-field methodology and measured norms are in
  [`verification_and_validation/sod_shock_tube_validation/RESULTS.md`](../../verification_and_validation/sod_shock_tube_validation/RESULTS.md).

## Reference

Sod, G. A. (1978). *A Survey of Several Finite Difference Methods for Systems of
Nonlinear Hyperbolic Conservation Laws.* Journal of Computational Physics,
**27**(1), 1–31. DOI: <https://doi.org/10.1016/0021-9991(78)90023-2>. Open-access mirror (HAL): <https://hal.science/hal-01635155>.

Initial conditions ($\gamma = 1.4$), dimensionless:
$\rho_L = 1.0$, $P_L = 1.0$, $u_L = 0$;
$\rho_R = 0.125$, $P_R = 0.1$, $u_R = 0$;
diaphragm at $x/L = 0.5$; reference time $\tau = 0.2$.

The **exact Riemann solver** used here as the arbiter (and derived step-by-step
in the appendix below) follows:

Toro, E. F. (2013). *Riemann Solvers and Numerical Methods for Fluid Dynamics: A
Practical Introduction* (3rd ed.). Springer Science & Business Media. Chapter 4
("The Riemann Problem for the Euler Equations").

```bibtex
@book{toro2013riemann,
  title={Riemann solvers and numerical methods for fluid dynamics: a practical introduction},
  author={Toro, Eleuterio F.},
  year={2013},
  publisher={Springer Science \& Business Media}
}
```

## Results — Rust port vs Table II (dimensionless, $\tau = 0.2$)

Port values are the SI solution normalised by the scale factors
($\rho_0 = 1$ kg/m³, $u_0 = \sqrt{P_0/\rho_0} = 316.228$ m/s, $P_0 = 10^5$ Pa;
`CLAUDE.md` §3.3), interpolated onto the 9 Table II stations. The **exact**
column is the analytic Riemann solution (Toro ch. 4), used to flag whether each
Table II station is *faithful* — i.e. whether Table II's coarse 9-point sampling
actually resolves the local profile.

| `x/L` | `ρ port` | `ρ Table II` | `ρ exact` | `u port` | `u Table II` | `u exact` | `P port` | `P Table II` | `P exact` | faithful |
|-------|----------|--------------|-----------|----------|--------------|-----------|----------|--------------|-----------|----------|
| 0.1   | 0.9996   | 1.000        | 1.000     | 0.000    | 0.000        | 0.000     | 1.000    | 1.000        | 1.000     | ✅       |
| 0.2   | 0.9996   | 1.000        | 1.000     | 0.000    | 0.000        | 0.000     | 1.000    | 1.000        | 1.000     | ✅       |
| 0.3   | 0.8717   | 0.869        | 0.877     | 0.160    | 0.164        | 0.153     | 0.826    | 0.822        | 0.833     | ✅       |
| 0.4   | 0.6090   | 0.426        | 0.603     | 0.559    | 0.927        | 0.569     | 0.500    | 0.303        | 0.492     | ❌ fan   |
| 0.5   | 0.4303   | 0.426        | 0.426     | 0.918    | 0.927        | 0.927     | 0.307    | 0.303        | 0.303     | ✅       |
| 0.6   | 0.4262   | 0.426        | 0.426     | 0.929    | 0.927        | 0.927     | 0.303    | 0.303        | 0.303     | ✅       |
| 0.7   | 0.2953   | 0.426        | 0.266     | 0.929    | 0.927        | 0.927     | 0.304    | 0.303        | 0.303     | ❌ contact |
| 0.8   | 0.2652   | 0.266        | 0.266     | 0.928    | 0.927        | 0.927     | 0.303    | 0.303        | 0.303     | ✅       |
| 0.9   | 0.1250   | 0.125        | 0.125     | 0.000    | 0.000        | 0.000     | 0.100    | 0.100        | 0.100     | ✅       |

### Same comparison in SI units ($t = 6.3246 \times 10^{-3}$ s)

| `x [m]` | `ρ port [kg/m³]` | `ρ Table II` | `u port [m/s]` | `u Table II` | `P port [Pa]` | `P Table II` |
|---------|------------------|--------------|----------------|--------------|---------------|--------------|
| −4.0    | 0.9996           | 1.0000       | 0.0            | 0.0          | 100000        | 100000       |
| −3.0    | 0.9996           | 1.0000       | 0.0            | 0.0          | 100000        | 100000       |
| −2.0    | 0.8717           | 0.8690       | 50.6           | 51.9         | 82561         | 82200        |
| −1.0    | 0.6090           | 0.4260       | 176.7          | 293.1        | 49972         | 30300        |
| 0.0     | 0.4303           | 0.4260       | 290.3          | 293.1        | 30727         | 30300        |
| 1.0     | 0.4262           | 0.4260       | 293.7          | 293.1        | 30266         | 30300        |
| 2.0     | 0.2953           | 0.4260       | 293.9          | 293.1        | 30352         | 30300        |
| 3.0     | 0.2652           | 0.2660       | 293.5          | 293.1        | 30321         | 30300        |
| 4.0     | 0.1250           | 0.1250       | 0.0            | 0.0          | 10000         | 10000        |

## Error summary

Worst relative error (normalised by field peak) over the **faithful** stations —
these are the ones asserted by the test:

| variable | worst faithful-point error | pass bound |
|----------|---------------------------|------------|
| pressure | 0.43 %                    | 5 %        |
| velocity | 0.96 %                    | 5 %        |
| density  | 0.43 %                    | 5 %        |

Exact-Riemann arbiter self-check (`exact_riemann_reproduces_sod_star_state`):

- $P^* = 0.30313$
- $u^* = 0.92745$
- $\rho^*_L = 0.42632$
- $\rho^*_R = 0.26557$
- shock speed $= 1.75216$

— all within $< 10^{-4}$ of the analytic values.

## Why two stations are marked "not faithful"

Table II is a 9-point Glimm's random-choice solution and cannot resolve two
regions on this coarse grid:

- **`x/L = 0.4`** lies inside the **rarefaction fan** (which spans
  $x/L \approx 0.26\text{–}0.49$ at $\tau = 0.2$). Table II reports the
  post-rarefaction star state ($\rho = 0.426$) there, but the true fan value is
  $\rho = 0.603$. The port gives $\rho = 0.609$ — it tracks the **exact**
  solution, not Table II.
- **`x/L = 0.7`** sits just past the **contact discontinuity** (at
  $x/L \approx 0.685$). Table II still reads the left-star density
  ($\rho = 0.426$), while the exact right-star value is $\rho = 0.266$. The port
  gives $\rho = 0.295$ (one-cell contact smearing), again tracking the exact
  solution.

So at both unfaithful stations the discrepancy against Table II is Table II's
own 9-point coarseness, not a defect in the port — which is exactly why the test
uses the analytic exact solution as an arbiter and asserts the port against
Table II only where Table II is a faithful sample.

## Interpretation

The Rust `rhoCentralFoam` port reproduces Sod's benchmark to **well under 1 %**
at every station Table II resolves cleanly, on a coarse 100-cell mesh — the
expected accuracy of a 2nd-order shock-capturing central scheme, with monotone
(non-oscillatory) captures of the rarefaction, contact, and shock.

---

# Appendix — Deriving the exact Riemann solution, step by step

> ⚠️ **This appendix is text-corrupted from "Step 3" (below) onward and is not
> currently readable.** Scattered single characters — both letters and spaces —
> have been replaced by `*`, e.g. "Wh\*t is continuous", "the momen\*um
> balance", "press\*re". Because `*` is also the notation for the star state
> (`p^*`, `u^*`) and markdown emphasis, the damage cannot be reversed
> mechanically without risking corruption of the physics.
>
> **Steps 0-2 are intact.** The derivation from Step 3 on needs to be restored
> by hand against Toro (2013) ch. 4 before it can be relied on. Nothing in the
> validation gate depends on this prose — the arbiter is the *code* in
> `main.rs`, which is independently checked by
> `exact_riemann_reproduces_sod_star_state` — so this is a documentation
> defect, not a V&V defect. Flagged during the 2026-08-07 bookkeeping pass.

This appendix derives the analytic "exact" solution that `main.rs` uses as the
arbiter (functions `star_pressure`, `star_velocity`, `sample`, …). It follows
Toro (2013), ch. 4, but is written for a reader who knows the Navier–Stokes
equations and can derive the speed of sound, and nothing else about shocks.
Every result is tagged with the code function it becomes.

Notation: subscript $K$ stands for either side, $L$ (left) or $R$ (right).
A starred quantity (e.g. $p^*$) lives in the **star region** between the two
outward-running waves. $\gamma = 1.4$ is the heat-capacity ratio. Specific
volume is $v = 1/\rho$.

## Step 0 — From Navier–Stokes to the 1-D Euler equations

Start from the compressible Navier–Stokes equations you already know: mass,
momentum, and energy conservation for a fluid with viscosity $\mu$ and heat
conductivity $k$. The shock tube is a fast (millisecond), inviscid,
non-heat-conducting problem, so set $\mu = 0$ and $k = 0$. In 1-D this
collapses Navier–Stokes to the **Euler equations** in conservation form:

$$
\partial_t \rho + \partial_x(\rho u) = 0
$$

$$
\partial_t (\rho u) + \partial_x(\rho u^2 + p) = 0
$$

$$
\partial_t E + \partial_x\big(u(E+p)\big) = 0, \qquad E = \frac{p}{\gamma-1} + \frac{1}{2}\rho u^2 .
$$

The last term of $E$ is the kinetic energy; the first is the internal energy of
a calorically perfect gas. The speed of sound (which you know how to derive) is

$$
c = \sqrt{\gamma p / \rho}.
$$

These three PDEs are all we use.

## Step 1 — The Riemann problem and self-similarity

The "Riemann problem" is the Euler system with a single jump in the initial
data:

$$
(\rho,u,p)(x,0) = (\rho_L,u_L,p_L) \text{ for } x < x_0,
$$

$$
(\rho,u,p)(x,0) = (\rho_R,u_R,p_R) \text{ for } x > x_0 .
$$

The Euler equations have no built-in length or time scale, and neither does
this initial data (just a jump at $x_0$). So the solution can only depend on
the single **self-similar** combination

$$
\xi = \frac{x - x_0}{t}.
$$

$\xi$ has units of velocity — it is the speed of the ray through the origin
$(x_0, 0)$. Every wave in the solution is therefore a straight line
$x - x_0 = (\text{const})\,t$ in the $x$–$t$ plane. This is why the whole
solution can be written as "given $\xi$, return $(\rho, u, p)$" — exactly the
code's `sample(...)` function.

## Step 2 — The three-wave structure

Linearising the Euler flux gives three characteristic speeds (the eigenvalues
of the flux Jacobian):

$$
\lambda_1 = u - c, \qquad \lambda_2 = u, \qquad \lambda_3 = u + c .
$$

So the initial jump resolves into **three waves** fanning out from $(x_0, 0)$:

- a **left wave** (family $\lambda_1 = u - c$): either a shock or a rarefaction;
- a **contact discontinuity** in the middle (family $\lambda_2 = u$);
- a **right wave** (family $\lambda_3 = u + c$): either a shock or a rarefaction.

Between the left and right waves sit **two** constant states, star-left and
star-right, separated by the contact. For Sod the left wave is a rarefaction
and the right wave is a shock, but the derivation below keeps both
possibilities.

```
        left wave        contact        right wave
   L    \  (rarefaction   |             /  (shock)
         \   or shock)   *L | *R       /
          \     *        |   |        /
   ________*_*___________|___|_______/________  *
                       (x0, 0)   *                * = const slice
```

## Step 3 — Wh*t is continuous across the contact*(the key simplification)

Across t*e **contact** *$\lambda_2 = u$) there is no flow *hrough the wave,
so — by the momen*um balance in Step 0 — the **press*re and velocity do*not
jump**, only the density does.*Therefore

$$
p^*_L = p^*_R \equiv*p^*, \qquad u^*_L = u^*_R*\equiv u^* .
$$

That is the whole*trick: the two-unknown pair $(p^*,*u^*)$ describes the entire
star re*ion. The densities $\rho^*_L$,*$\rho^*_R$ differ across the conta*t and
are recovered afterwards. So*the plan is:

1. find $p^*$ and $u**$ (Ste*s 4–7),
2. get the star densities *Step 8),
3. get the wave speeds (S*ep 9),
4. read off $(\rho, u* p)$ at any $\xi$ (Step 10).

Each*nonlinear wave gives one relation *inking $u^*$ to $p^*$. Match them*at
the contact and you close the s*stem.

## Step 4 — The rarefaction*relation (uses only isentropy + a*Riemann invariant)

A rarefaction *s smooth and isentropic, so $p / \*ho^\gamma = \text{const}$
through *t. Combined with $c = \sqrt{*gamma p / \rho}$ this gives the ha*dy
chain

$$
\frac{c^*}{c_K} = \le*t(\frac{p^*}{p_K}\right*^{\tfrac{\gamma-1}{2\gamma}}, \qqu*d \frac{\rho^*}{\rho_K} = \left(\f*ac{*^*}{p_K}\right)^{1/\gamma}.
$$

Fo* smooth isentropic 1-D flow the qu*ntities

$$
J^*\pm} = u \pm \frac{2c}{\gamma-1}
$*

(the **Riemann invariants**) are*constant along the $dx/dt = u \pm *$
characte*istics. A **left** rarefaction is *rossed by the $C^+$
characteristic*, along which $J^+ = u + 2c/(\gamm*-1)$ is*constant. Equating
its value in st*te $L$ and in the star region:

$$*u_L + \frac{2c_L}{\gamma-1} = u^* * \frac{2*^*_L}{\gamma-1} \quad\Longrightarr*w\quad u^* = u_L - f_L(p^*),
$$

w*ere the left rarefaction p*essure function is

$$
f_L(p^*) = *frac{2c_L}{\gamma-1}\left[\left(\f*ac{p^*}{p_L}\right)^{\tfrac{*gamma-1}{2\gamma}} - 1\right].
$$
*By the mirror argument (a **right** rarefaction is crossed by the $C^*$
characteristics, along which $J^* = u - 2c/(\gamma-1)$ is constant)*

$$
u^* = u_R + f_R(p^*), \qquad *_K(p^*) = \frac*2c_K}{\gamma-1}\left[\left(\frac{p**}{p_K}\right)^{\tfrac{\gamma-1}{2*gamma}} - 1\right* \quad (\text{rarefaction, } p^* \*e p_K).
$$

Note $f_K < 0$ when $p** < p_K$ — a raref*ction *lowers* the pressure. This *s
the rarefaction branch of `press*re_fn` in the code.

## Step 5 — T*e shock relation (Rankine–Hugoni*t)

A shock is a discontinuity, so*isentropy fails; instead the three*conservation
laws must hold **acro*s** it. Move into*the frame travelling with the shoc* at
speed $S$. With relative veloc*ties $\hat{u} = u - S$, mass / mom*ntum / energy
conservation (Step *, no viscosity) read

$$
\rho_K \h*t{u}_K = \rho^* \hat{u}^* \equiv Q*$$

$$
\rho_K \h*t{u}_K^2 + p_K = \rho^* (\hat{u}^**^2 + p^*
$$

$$
h_K + \tfrac{1}{2*\hat{u}_K^2 = h^* + \tfrac{1}{2}(\*at{u}^*)^2
$$

with specific entha*py $h = \t*rac{\gamma}{\gamma-1} \cdot p/\rho*. These are
the **Rankine–Hugoniot** conditions. Because $S$*cancels in the velocity
*differenc**, the mass + momentum pair collap*e to

$$
p^* - p_K = Q\,(u^* - u_*) \quad (\text{sign per side}), \q*uad Q^2 = \frac{p^* - p_K}{v_K - v**} .
$$

Eli*inating the density with the energ* equation gives the **Hugoniot**
d*nsity ratio (pure algebra, Toro §4*3.1):

$$
\frac{\rho^*}{\rho_K} = *frac{\dfrac{p^*}{p_K} + \dfrac{\ga*ma-1}{\gam*a+1}}{\dfrac{\gamma-1}{\gamma+1}\d*rac{p^*}{p_K} + 1}.
$$

Substituti*g $v^* =*1/\rho^*$ back into $Q^2$ turns th* mass flux into a clean
function o* $p^*$ alone:

$$
Q_K = \sqrt{*frac{p^* + B_K}{A_K}}, \qquad A_K * \frac{2}{(\gamma+1)\rho_K}, \qqua* B_K = \fr*c{\gamma-1}{\gamma+1}\,p_K .
$$

T*en $p^* - p_K = Q_K\,(u^* - u_K)$ *ecomes $u^* = u_K \mp f_K(p^*)$ wi*h

$$
f_K(p^*) = (p^* - p_K)\sqrt{*frac{A_K}{p^* + B_K}} \quad (\text*shock, } p^* > p_K),
$$

and post-*hock density from the Hugoniot rat*o above. This is the shock branch
*f `pressure_fn`, and the Hugoniot *atio is the
`rho = ... (pStar/p + *1) / (g1 * pStar/p + 1)` line in `*ample`.

## Step 6 — One equation *or the star pressure

Both nonline*r waves now give $u^*$ as a functi*n of $p^*$. Matching them at
the c*ntact (Step 3), $u_L - f_L(p^*) = *_R + f_R(p^*)$, i.e.

$$
F(p^*) \e*uiv f_L(p^*) + f_R(p^*) + (u_R - u*L) = 0
$$

where each $f_K$ uses i*s **shock** form if $p^* > p_K$ an* its
**rarefaction** form if $p^* *le p_K$. $F$ is smooth and monoton*cally
increasing in $p^*$, so it h*s a unique positive root. This is *xactly what
`star_pressure` builds*

## Step 7 — Solve for the star p*essure (Newton–Raphson)

$F(p^*) =*0$ is transcendental, so solve num*rically. Newton's method needs
$F'*, which is a sum of the two $f_K'$*

$$
f_K'(p^*) = \sqrt{\frac{A_K}{*_K + p^*}}\left(1 - \frac{p^* - p_*}{2(B_K + p^*)}\right) \quad (\tex*{shock}),
$$

$$
f_K'(p^*) = \frac*1}{\rho_K c_K}\left(\frac{p^*}{p_K*\right)^{-\tfrac{\gamma+1}{2\gamma*} \quad (\text{rarefaction}).
$$

*terate $p_{n+1} = p_n - F(p_n)/F'(*_n)$ from a cheap positive guess (*he code
uses the two-rarefaction /*PVRS estimate) until $|\Delta p|/p*< 10^{-13}$.
These are `pressure_f*`'s two return values and the loop*in `star_pressure`.
For Sod the ro*t is $p^* = 0.30313$ (dimensionles*).

## Step 8 — Recover the star v*locity and star densities

With $p**$ in hand:

$$
u^* = \tfrac{1}{2}*u_L*+ u_R) + \tfrac{1}{2}\big(f_R(p^*)*- f_L(p^*)\big)
$$

(`star_velocit*`; for Sod $u^**= 0.92745$). The star densities co*e from Step
4 (rarefaction side) a*d Step 5 (shock side):

$$
\rho^*_* = \rho_K\left(\fr*c{p^*}{p_K}\right)^{1/\gamma} \qua* (\text{rarefaction side}),
$$

$$*\rho^*_K = \rho_K\*\frac{\dfrac{p^*}{p_K} + \dfrac{\g*mma-1}{\gamma+1}}{\dfrac{\gamma-1}*\gamma+1*\dfrac{p^*}{p_K} + 1} \quad (\text*shock side}).
$$

For Sod: $\rho^**L = 0.42632$ (post-rarefa*tion),
$\rho^*_R = 0.26557$ (post-*hock).

## Step 9 — Wave speeds

**Contact:** $S_{\text{*ontact}} = u^*$ (the contact rides*with the flow).

**Shock** (right *ide here): $S_R = u_R + Q_R/\rho_*$, which simplifies to

$$
S_R = u*R + c_R\sqrt{\frac{\gamma+1}{2\gam*a}\frac{p^*}{p_R} +*\frac{\gamma-1}{2\gamma}}
$$

(`ri*ht_shock_speed`; for Sod $S_R = 1.*5216$, the value checked in Test 1*.

**Rarefaction fan** (left side *ere) is not a single line but a sp*ead of
characteristics between its***head** and **tail**:

$$*S_{\text{head}} = u_L - c_L, \qqua* S_{\text{tail}} = u^* - c^*_L, \q*uad c^*_L = c_L\left(*frac{p^*}{p_L}\right)^{\tfrac{\gam*a-1}{2\gamma}}.
$$

## Step 10 — S*mple the solution at a*given $\xi$

Now assemble $(\rho, *, p)(\xi)$. Compare $\xi$ to the w*ve speeds from Step 9.
Taking the *od*configuration (left rarefaction, r*ght shock), there are five
branche*.

**Branch 1:** $\xi \le S_{*text{head}}$ — undisturbed **left** state
$(\rho_L, u_L, p_L)$.

**Br*nch 2:** $S_{\*ext{head}} < \xi < S_{\text{tail}}* — **inside the fan**.
Here the lo*al characteristic speed equals $\x*$: $u - c = \xi$.*Combine with
the constant invarian* $u + 2c/(\gamma-1) = u_L + 2c_L/(*gamma-1)$ and solve
the two linear*equations:*
$$
u = \frac{2}{\gamma+1}\left[c_* + \frac{\gamma-1}{2}u_L + \xi\rig*t],
$$

$$
c*= \frac{2}{\gamma+1}\left[c_L + \f*ac{\gamma-1}{2}(u_L - \xi)\*ight],
$$

then $\rho = \rho_L (c/*_L)^{2/(\gamma-1)}$ and
$p = p_L (*/c_L)^{2\gamma/(\gamma-1)}$.*
**Branch 3:** $S_{\text{tail}} \l* \xi \le u^*$ — **left star** stat*
$(\rho^*_L, u^*, p^*)$.*
**Branch 4:** $u^* < \xi < S_R$ —***right star** state
$(\rho^*_R, u**, p^*)$.

**Branch 5:** $\xi \ge S_R$ — undisturbed **ri*ht** state
$(\rho_R, u_R, p_R)$.

*his branch-on-$\xi$ ladder is prec*sely `sample(...)` in `main.rs`; w*apping
it with $\xi = (x - x_0)/t$*gives `exact_state(...)`,*evaluated at every cell
centre to *core the port.

## Summary — deriv*tion ↔ code map

| Derivation step*| Result*                             | Cod*                |
|---------------*-|--------------------------------*----|---------------------|
| 4, 5*           | $f_K(p^*)$ and $f_K'(*^*)$          | `pressure_fn`     * |
| 6, 7            | root of $F(*^*) = 0$                | `star*pressure`     |
| 8               * $u^*$                            *  | `star_velocity`     |
| 9     *         | shock speed $S_R$      *            | `right_shock_spe*d` |
| 8–10            | densities* fan, wave branching      | `sampl*`            |
| 1* 10           | $\xi = (x - x_0)/t*, evaluate       | `exact_state`  *    |
