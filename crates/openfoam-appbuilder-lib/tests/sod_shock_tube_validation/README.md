<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore
Part of Outram Park (outram-park-backend), openfoam-appbuilder-lib.
-->

# Sod Shock Tube Validation — Rust `rhoCentralFoam` port vs Sod (1978) Table II

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
  (2026-07-06; 100 cells, `dt = 1e-6 s`, run to Sod's canonical τ = 0.2, i.e.
  `t = 6.3246e-3 s`). Regenerate with:

  ```bash
  cargo test -p openfoam-appbuilder-lib --test sod_shock_tube_validation \
    rho_central_foam_matches_sod_table_ii -- --nocapture
  ```

## Reference

Sod, G. A. (1978). *A Survey of Several Finite Difference Methods for Systems of
Nonlinear Hyperbolic Conservation Laws.* Journal of Computational Physics,
**27**(1), 1–31. DOI: [10.1016/0021-9991(78)90023-2](https://doi.org/10.1016/0021-9991(78)90023-2).

Open-access mirror (HAL): <https://hal.science/hal-01635155>.

Initial conditions (γ = 1.4), dimensionless: ρ_L = 1.0, P_L = 1.0, u_L = 0;
ρ_R = 0.125, P_R = 0.1, u_R = 0; diaphragm at x/L = 0.5; reference time τ = 0.2.

The **exact Riemann solver** used here as the arbiter (and derived step-by-step
in the appendix below) follows:

Toro, E. F. (2013). *Riemann Solvers and Numerical Methods for Fluid Dynamics: A
Practical Introduction* (3rd ed.). Springer Science & Business Media. Chapter 4
(“The Riemann Problem for the Euler Equations”).

```bibtex
@book{toro2013riemann,
  title={Riemann solvers and numerical methods for fluid dynamics: a practical introduction},
  author={Toro, Eleuterio F},
  year={2013},
  publisher={Springer Science \& Business Media}
}
```

## Results — Rust port vs Table II (dimensionless, τ = 0.2)

Port values are the SI solution normalised by the scale factors (ρ₀ = 1 kg/m³,
u₀ = √(P₀/ρ₀) = 316.228 m/s, P₀ = 10⁵ Pa; `CLAUDE.md` §3.3), interpolated onto
the 9 Table II stations. The **exact** column is the analytic Riemann solution
(Toro ch. 4), used to flag whether each Table II station is *faithful* — i.e.
whether Table II's coarse 9-point sampling actually resolves the local profile.

| x/L | ρ port | ρ Table II | ρ exact | u port | u Table II | u exact | P port | P Table II | P exact | faithful |
|-----|--------|-----------|---------|--------|-----------|---------|--------|-----------|---------|----------|
| 0.1 | 0.9996 | 1.000 | 1.000 | 0.000 | 0.000 | 0.000 | 1.000 | 1.000 | 1.000 | ✅ |
| 0.2 | 0.9996 | 1.000 | 1.000 | 0.000 | 0.000 | 0.000 | 1.000 | 1.000 | 1.000 | ✅ |
| 0.3 | 0.8717 | 0.869 | 0.877 | 0.160 | 0.164 | 0.153 | 0.826 | 0.822 | 0.833 | ✅ |
| 0.4 | 0.6090 | 0.426 | 0.603 | 0.559 | 0.927 | 0.569 | 0.500 | 0.303 | 0.492 | ❌ fan |
| 0.5 | 0.4303 | 0.426 | 0.426 | 0.918 | 0.927 | 0.927 | 0.307 | 0.303 | 0.303 | ✅ |
| 0.6 | 0.4262 | 0.426 | 0.426 | 0.929 | 0.927 | 0.927 | 0.303 | 0.303 | 0.303 | ✅ |
| 0.7 | 0.2953 | 0.426 | 0.266 | 0.929 | 0.927 | 0.927 | 0.304 | 0.303 | 0.303 | ❌ contact |
| 0.8 | 0.2652 | 0.266 | 0.266 | 0.928 | 0.927 | 0.927 | 0.303 | 0.303 | 0.303 | ✅ |
| 0.9 | 0.1250 | 0.125 | 0.125 | 0.000 | 0.000 | 0.000 | 0.100 | 0.100 | 0.100 | ✅ |

### Same comparison in SI units (t = 6.3246×10⁻³ s)

| x [m] | ρ port [kg/m³] | ρ Table II | u port [m/s] | u Table II | P port [Pa] | P Table II |
|-------|----------------|-----------|--------------|-----------|-------------|-----------|
| −4.0 | 0.9996 | 1.0000 | 0.0 | 0.0 | 100000 | 100000 |
| −3.0 | 0.9996 | 1.0000 | 0.0 | 0.0 | 100000 | 100000 |
| −2.0 | 0.8717 | 0.8690 | 50.6 | 51.9 | 82561 | 82200 |
| −1.0 | 0.6090 | 0.4260 | 176.7 | 293.1 | 49972 | 30300 |
| 0.0 | 0.4303 | 0.4260 | 290.3 | 293.1 | 30727 | 30300 |
| 1.0 | 0.4262 | 0.4260 | 293.7 | 293.1 | 30266 | 30300 |
| 2.0 | 0.2953 | 0.4260 | 293.9 | 293.1 | 30352 | 30300 |
| 3.0 | 0.2652 | 0.2660 | 293.5 | 293.1 | 30321 | 30300 |
| 4.0 | 0.1250 | 0.1250 | 0.0 | 0.0 | 10000 | 10000 |

## Error summary

Worst relative error (normalised by field peak) over the **faithful** stations —
these are the ones asserted by the test:

| variable | worst faithful-point error | pass bound |
|----------|---------------------------|------------|
| pressure | 0.43 % | 5 % |
| velocity | 0.96 % | 5 % |
| density  | 0.43 % | 5 % |

Exact-Riemann arbiter self-check (`exact_riemann_reproduces_sod_star_state`):
P\* = 0.30313, u\* = 0.92745, ρ\*_L = 0.42632, ρ\*_R = 0.26557, shock speed =
1.75216 — all within < 10⁻⁴ of the analytic values.

## Why two stations are marked "not faithful"

Table II is a 9-point Glimm's random-choice solution and cannot resolve two
regions on this coarse grid:

- **x/L = 0.4** lies inside the **rarefaction fan** (which spans x/L ≈ 0.26–0.49
  at τ = 0.2). Table II reports the post-rarefaction star state (ρ = 0.426) there,
  but the true fan value is ρ = 0.603. The port gives ρ = 0.609 — it tracks the
  **exact** solution, not Table II.
- **x/L = 0.7** sits just past the **contact discontinuity** (at x/L ≈ 0.685).
  Table II still reads the left-star density (ρ = 0.426), while the exact
  right-star value is ρ = 0.266. The port gives ρ = 0.295 (one-cell contact
  smearing), again tracking the exact solution.

So at both unfaithful stations the discrepancy against Table II is Table II's own
9-point coarseness, not a defect in the port — which is exactly why the test uses
the analytic exact solution as an arbiter and asserts the port against Table II
only where Table II is a faithful sample.

## Interpretation

The Rust `rhoCentralFoam` port reproduces Sod's benchmark to **well under 1 %**
at every station Table II resolves cleanly, on a coarse 100-cell mesh — the
expected accuracy of a 2nd-order shock-capturing central scheme, with monotone
(non-oscillatory) captures of the rarefaction, contact, and shock.

---

# Appendix — Deriving the exact Riemann solution, step by step

This appendix derives the analytic “exact” solution that `main.rs` uses as the
arbiter (functions `star_pressure`, `star_velocity`, `sample`, …). It follows
Toro (2013), ch. 4, but is written for a reader who knows the Navier–Stokes
equations and can derive the speed of sound, and nothing else about shocks. Every
result is tagged with the code function it becomes.

Notation: subscript `K` stands for either side, `L` (left) or `R` (right).
A starred quantity (e.g. `p*`) lives in the **star region** between the two
outward-running waves. `γ = 1.4` is the heat-capacity ratio. Specific volume is
`v = 1/ρ`.

## Step 0 — From Navier–Stokes to the 1-D Euler equations

Start from the compressible Navier–Stokes equations you already know: mass,
momentum, and energy conservation for a fluid with viscosity `μ` and heat
conductivity `k`. The shock tube is a fast (millisecond), inviscid,
non-heat-conducting problem, so set `μ = 0` and `k = 0`. In 1-D this collapses
Navier–Stokes to the **Euler equations** in conservation form:

$$
\partial_t
\begin{bmatrix}\rho \\ \rho u \\ E\end{bmatrix}
+ \partial_x
\begin{bmatrix}\rho u \\ \rho u^2 + p \\ u(E+p)\end{bmatrix}
= 0,
\qquad
E = \frac{p}{\gamma-1} + \tfrac12 \rho u^2 .
$$

The last term of `E` is the kinetic energy; the first is the internal energy of
a calorically perfect gas. The speed of sound (which you know how to derive) is

$$
c = \sqrt{\gamma p / \rho}.
$$

These three PDEs are all we use.

## Step 1 — The Riemann problem and self-similarity

The “Riemann problem” is the Euler system with a single jump in the initial data:

$$
(\rho,u,p)(x,0) =
\begin{cases}
(\rho_L,u_L,p_L), & x < x_0 \\
(\rho_R,u_R,p_R), & x > x_0 .
\end{cases}
$$

The Euler equations have no built-in length or time scale, and neither does this
initial data (just a jump at `x₀`). So the solution can only depend on the single
**self-similar** combination

$$
\xi = \frac{x - x_0}{t}.
$$

`ξ` has units of velocity — it is the speed of the ray through the origin `(x₀,0)`.
Every wave in the solution is therefore a straight line `x - x₀ = (\text{const})\,t`
in the `x–t` plane. This is why the whole solution can be written as “given `ξ`,
return `(ρ, u, p)`” — exactly the code's `sample(...)` function.

## Step 2 — The three-wave structure

Linearising the Euler flux gives three characteristic speeds (the eigenvalues of
the flux Jacobian):

$$
\lambda_1 = u - c, \qquad \lambda_2 = u, \qquad \lambda_3 = u + c .
$$

So the initial jump resolves into **three waves** fanning out from `(x₀,0)`:

- a **left wave** (family `λ₁ = u − c`): either a shock or a rarefaction;
- a **contact discontinuity** in the middle (family `λ₂ = u`);
- a **right wave** (family `λ₃ = u + c`): either a shock or a rarefaction.

Between the left and right waves sit **two** constant states, `*L` and `*R`,
separated by the contact. For Sod the left wave is a rarefaction and the right
wave is a shock, but the derivation below keeps both possibilities.

```
        left wave        contact        right wave
   L    \  (rarefaction   |             /  (shock)
         \   or shock)   *L | *R       /
          \              |   |        /
   ________\_____________|___|_______/________  x
                       (x₀,0)                     t = const slice
```

## Step 3 — What is continuous across the contact (the key simplification)

Across the **contact** (`λ₂ = u`) there is no flow through the wave, so — by the
momentum balance in Step 0 — the **pressure and velocity do not jump**, only the
density does. Therefore

$$
p^*_L = p^*_R \equiv p^*, \qquad u^*_L = u^*_R \equiv u^* .
$$

That is the whole trick: the two-unknown pair `(p*, u*)` describes the entire star
region. The densities `ρ*_L`, `ρ*_R` differ across the contact and are recovered
afterwards. So the plan is:

1. find `p*` and `u*` (Steps 4–7),
2. get the star densities (Step 8),
3. get the wave speeds (Step 9),
4. read off `(ρ,u,p)` at any `ξ` (Step 10).

Each nonlinear wave gives one relation linking `u*` to `p*`. Match them at the
contact and you close the system.

## Step 4 — The rarefaction relation (uses only isentropy + a Riemann invariant)

A rarefaction is smooth and isentropic, so `p/ρ^γ = const` through it. Combined
with `c = √(γp/ρ)` this gives the handy chain

$$
\frac{c^*}{c_K} = \left(\frac{p^*}{p_K}\right)^{\frac{\gamma-1}{2\gamma}},
\qquad
\frac{\rho^*}{\rho_K} = \left(\frac{p^*}{p_K}\right)^{1/\gamma}.
$$

For smooth isentropic 1-D flow the quantities

$$
J^{\pm} = u \pm \frac{2c}{\gamma-1}
$$

(the **Riemann invariants**) are constant along the `dx/dt = u \pm c`
characteristics. A **left** rarefaction is crossed by the `C^{+}` characteristics,
along which `J^{+} = u + 2c/(\gamma-1)` is constant. Equating its value in state
`L` and in the star region:

$$
u_L + \frac{2c_L}{\gamma-1} = u^* + \frac{2c^*_L}{\gamma-1}
\;\Longrightarrow\;
u^* = u_L - \underbrace{\frac{2c_L}{\gamma-1}\!\left[\left(\tfrac{p^*}{p_L}\right)^{\frac{\gamma-1}{2\gamma}} - 1\right]}_{\displaystyle f_L(p^*)} .
$$

By the mirror argument (a **right** rarefaction is crossed by `C^{-}`,
`J^{-} = u - 2c/(\gamma-1)` constant):

$$
u^* = u_R + f_R(p^*),
\qquad
f_K(p^*) = \frac{2c_K}{\gamma-1}\!\left[\left(\tfrac{p^*}{p_K}\right)^{\frac{\gamma-1}{2\gamma}} - 1\right]
\quad (\text{rarefaction, } p^* \le p_K).
$$

Note `f_K < 0` when `p* < p_K` — a rarefaction *lowers* the pressure. This is the
rarefaction branch of `pressure_fn` in the code.

## Step 5 — The shock relation (Rankine–Hugoniot)

A shock is a discontinuity, so isentropy fails; instead the three conservation
laws must hold **across** it. Move into the frame travelling with the shock at
speed `S`. With relative velocities `\hat u = u - S`, mass / momentum / energy
conservation (Step 0, no viscosity) read

$$
\rho_K \hat u_K = \rho^* \hat u^* \equiv Q, \qquad
\rho_K \hat u_K^2 + p_K = \rho^* \hat u^{*2} + p^*, \qquad
h_K + \tfrac12\hat u_K^2 = h^* + \tfrac12\hat u^{*2},
$$

with specific enthalpy `h = \frac{\gamma}{\gamma-1}\frac{p}{\rho}`. These are the
**Rankine–Hugoniot** conditions. Because `S` cancels in the velocity *difference*,
the mass + momentum pair collapse to

$$
p^* - p_K = Q\,(u^* - u_K)\quad(\text{sign per side}),
\qquad
Q^2 = \frac{p^* - p_K}{v_K - v^*}\; .
$$

Eliminating the density with the energy equation gives the **Hugoniot** density
ratio (pure algebra, Toro §4.3.1):

$$
\frac{\rho^*}{\rho_K}
= \frac{\dfrac{p^*}{p_K} + \dfrac{\gamma-1}{\gamma+1}}
       {\dfrac{\gamma-1}{\gamma+1}\dfrac{p^*}{p_K} + 1}.
$$

Substituting `v* = 1/ρ*` back into `Q²` turns the mass flux into a clean function
of `p*` alone:

$$
Q_K = \sqrt{\frac{p^* + B_K}{A_K}},
\qquad
A_K = \frac{2}{(\gamma+1)\rho_K},
\qquad
B_K = \frac{\gamma-1}{\gamma+1}\,p_K .
$$

Then `p^* - p_K = Q_K\,(u^* - u_K)` becomes `u^* = u_K \mp f_K(p^*)` with

$$
f_K(p^*) = (p^* - p_K)\sqrt{\frac{A_K}{p^* + B_K}}
\quad (\text{shock, } p^* > p_K),
$$

and post-shock density from the Hugoniot ratio above. This is the shock branch of
`pressure_fn`, and the Hugoniot ratio is the `rho = ... (p*/p + g1)/(g1 p*/p + 1)`
line in `sample`.

## Step 6 — One equation for `p*`

Both nonlinear waves now give `u*` as a function of `p*`. Matching them at the
contact (Step 3), `u_L - f_L(p^*) = u_R + f_R(p^*)`, i.e.

$$
\boxed{\,F(p^*) \equiv f_L(p^*) + f_R(p^*) + (u_R - u_L) = 0\,}
$$

where each `f_K` uses its **shock** form if `p* > p_K` and its **rarefaction**
form if `p* ≤ p_K`. `F` is smooth and monotonically increasing in `p*`, so it has a unique
positive root. This is exactly what `star_pressure` builds.

## Step 7 — Solve for `p*` (Newton–Raphson)

`F(p*) = 0` is transcendental, so solve numerically. Newton's method needs `F'`,
which is a sum of the two `f_K'`:

$$
f_K'(p^*) =
\begin{cases}
\sqrt{\dfrac{A_K}{B_K + p^*}}\left(1 - \dfrac{p^*-p_K}{2(B_K+p^*)}\right), & \text{shock},\\[2ex]
\dfrac{1}{\rho_K c_K}\left(\dfrac{p^*}{p_K}\right)^{-\frac{\gamma+1}{2\gamma}}, & \text{rarefaction}.
\end{cases}
$$

Iterate `p_{n+1} = p_n - F(p_n)/F'(p_n)` from a cheap positive guess (the code
uses the two-rarefaction / PVRS estimate) until `|Δp|/p < 10^{-13}`. These are
`pressure_fn`'s two return values and the loop in `star_pressure`. For Sod the
root is `p* = 0.30313` (dimensionless).

## Step 8 — Recover `u*` and the star densities

With `p*` in hand:

$$
u^* = \tfrac12(u_L + u_R) + \tfrac12\big(f_R(p^*) - f_L(p^*)\big)
$$

(`star_velocity`; for Sod `u* = 0.92745`). The star densities come from Step 4
(rarefaction side) and Step 5 (shock side):

$$
\rho^*_K =
\begin{cases}
\rho_K\left(\dfrac{p^*}{p_K}\right)^{1/\gamma}, & \text{rarefaction side},\\[2ex]
\rho_K\,\dfrac{p^*/p_K + \frac{\gamma-1}{\gamma+1}}{\frac{\gamma-1}{\gamma+1}\,p^*/p_K + 1}, & \text{shock side}.
\end{cases}
$$

For Sod: `ρ*_L = 0.42632` (post-rarefaction), `ρ*_R = 0.26557` (post-shock).

## Step 9 — Wave speeds

- **Contact:** `S_contact = u*` (the contact rides with the flow).
- **Shock** (right side here): `S_R = u_R + Q_R/\rho_R`, which simplifies to

$$
S_R = u_R + c_R\sqrt{\frac{\gamma+1}{2\gamma}\frac{p^*}{p_R} + \frac{\gamma-1}{2\gamma}}
$$

  (`right_shock_speed`; for Sod `S_R = 1.75216`, the value checked in Test 1).

- **Rarefaction fan** (left side here) is not a single line but a spread of
  characteristics between its **head** and **tail**:

$$
S_{\text{head}} = u_L - c_L, \qquad
S_{\text{tail}} = u^* - c^*_L, \quad c^*_L = c_L\!\left(\tfrac{p^*}{p_L}\right)^{\frac{\gamma-1}{2\gamma}}.
$$

## Step 10 — Sample the solution at a given `ξ`

Now assemble `(ρ,u,p)(ξ)`. Compare `ξ` to the wave speeds from Step 9. Taking the
Sod configuration (left rarefaction, right shock):

1. `ξ ≤ S_head` → undisturbed **left** state `(ρ_L,u_L,p_L)`.
2. `S_head < ξ < S_tail` → **inside the fan**. Here the local characteristic
   speed equals `ξ`: `u − c = ξ`. Combine with the constant invariant
   `u + 2c/(γ−1) = u_L + 2c_L/(γ−1)` and solve the two linear equations:

$$
u = \frac{2}{\gamma+1}\!\left[c_L + \frac{\gamma-1}{2}u_L + \xi\right],\qquad
c = \frac{2}{\gamma+1}\!\left[c_L + \frac{\gamma-1}{2}(u_L - \xi)\right],
$$

   then `ρ = ρ_L (c/c_L)^{2/(γ−1)}`, `p = p_L (c/c_L)^{2γ/(γ−1)}`.
3. `S_tail ≤ ξ ≤ u*` → **left star** state `(ρ*_L, u*, p*)`.
4. `u* < ξ < S_R` → **right star** state `(ρ*_R, u*, p*)`.
5. `ξ ≥ S_R` → undisturbed **right** state `(ρ_R,u_R,p_R)`.

This branch-on-`ξ` ladder is precisely `sample(...)` in `main.rs`; wrapping it
with `ξ = (x − x₀)/t` gives `exact_state(...)`, evaluated at every cell centre to
score the port.

## Summary — derivation ↔ code map

| Derivation step | Result | Code |
|---|---|---|
| 4, 5 | `f_K(p*)` and `f_K'(p*)` | `pressure_fn` |
| 6, 7 | root of `F(p*) = 0` | `star_pressure` |
| 8 | `u*` | `star_velocity` |
| 9 | shock speed `S_R` | `right_shock_speed` |
| 8–10 | densities, fan, wave branching | `sample` |
| 1, 10 | `ξ = (x−x₀)/t`, evaluate | `exact_state` |
