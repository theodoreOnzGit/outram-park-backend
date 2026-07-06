<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore

This file is part of Outram Park (outram-park-backend), openfoam-appbuilder-lib.
Sod shock tube V&V reference + Claude Code operating instructions.

Primary source: Sod, G. A. (1978). A Survey of Several Finite Difference
Methods for Systems of Nonlinear Hyperbolic Conservation Laws.
Journal of Computational Physics, 27(1), 1-31.
DOI: 10.1016/0021-9991(78)90023-2
HAL open-access mirror: https://hal.science/hal-01635155
-->

# Sod Shock Tube V&V — `openfoam-appbuilder-lib::rho_central_foam`

**Status:** rhoCentralFoam-Rust port passes at 0.7% L1 with vanLeer MUSCL (as of 1 Jul 2026).
This document is the canonical reference for regenerating / extending the V&V case.

---

## 1. Purpose

Validate `openfoam-appbuilder-lib`'s Rust port of OpenFOAM `rhoCentralFoam` (Kurganov-Tadmor
central-upwind scheme, 2nd-order MUSCL-reconstructed) against:

1. **Sod (1978) Table II** — 9-point Glimm's-method reference (near-exact Riemann)
2. **Exact Riemann solution** — computed on-the-fly from Sod Appendix eqs. (46)-(62), Fig. 16 flow chart
3. **OpenFOAM `rhoCentralFoam` reference run** — bit-parity target for the Rust port

---

## 2. Governing equations (Sod §1, eqs. 2a-2c)

1D Euler equations, inviscid non-heat-conducting polytropic gas:

$$
\partial_t \rho + \partial_x (\rho u) = 0
$$
$$
\partial_t (\rho u) + \partial_x (\rho u^2 + P) = 0
$$
$$
\partial_t e + \partial_x (u(e + P)) = 0
$$

with $e = P/(\gamma-1) + \tfrac{1}{2}\rho u^2$ (total energy per unit volume), $\gamma = 1.4$.

---

## 3. Nondimensionalisation (CRITICAL — read before comparing anything)

### 3.1 Sod (1978) convention

Sod reports all quantities **unitless**. The 1D Euler equations are scale-invariant under:

$$
\rho \to \rho_0 \tilde{\rho}, \quad
P \to P_0 \tilde{P}, \quad
u \to \sqrt{P_0/\rho_0}\,\tilde{u}, \quad
x \to L_0 \tilde{x}, \quad
t \to (L_0 \sqrt{\rho_0/P_0})\,\tilde{t}
$$

Sod's ICs: $\tilde{\rho}_L = 1.0$, $\tilde{P}_L = 1.0$, $\tilde{\rho}_R = 0.125$, $\tilde{P}_R = 0.1$,
$\tilde{u}_{L,R} = 0$, on $\tilde{x} \in [0,1]$ with diaphragm at $\tilde{x} = 0.5$.

Canonical comparison time: $\tilde{t} = 0.2$ (community convention; not explicit in Sod's Table II caption).

### 3.2 OpenFOAM `rhoCentralFoam` reference case

Standard Greenshields/Weller Sod tutorial, air with `perfectGas` + `hConst`:

| Quantity | Left | Right | Units |
|---|---|---|---|
| ρ | 1.0000 | 0.12500 | kg/m³ |
| P | 1.0×10⁵ | 1.0×10⁴ | Pa |
| T | 348.432 | 278.746 | K |
| u | 0 | 0 | m/s |
| γ | 1.4 | 1.4 | — |
| $R_{\text{spec}}$ | 287.10 | 287.10 | J/(kg·K) |
| $C_p$ | 1004.5 | 1004.5 | J/(kg·K) |
| μ | 0 | 0 | Pa·s (inviscid) |

Domain: $x \in [-5, +5]$ m, diaphragm at $x = 0$, 100 uniform cells (Δx = 0.1 m).
`endTime` = 0.007 s, `maxCo` = 0.2, `writeInterval` = 0.001 s.

### 3.3 Scale-factor table (Sod ↔ OpenFOAM SI)

**Multiply Sod-dimensionless values by these to get SI. Divide SI by these to normalise to Sod.**

| Sod symbol | Multiplier (this case) | SI unit |
|---|---|---|
| $\rho_0$ | 1.0 | kg/m³ |
| $P_0$ | 1.0×10⁵ | Pa |
| $L_0$ | 10.0 | m |
| $u_0 = \sqrt{P_0/\rho_0}$ | 316.2278 | m/s |
| $t_0 = L_0/u_0$ | 0.0316228 | s |
| $e_0 = P_0$ | 1.0×10⁵ | J/m³ |
| $T_0 = P_0/(\rho_0 R)$ | 348.432 | K |

**Coordinate remap:** $\tilde{x}_{\text{Sod}} = x_{\text{OF}}/L_0 + 0.5 = x_{\text{OF}}/10 + 0.5$

### 3.4 Time-alignment reality check

| Interpretation | OpenFOAM t (s) | Sod τ |
|---|---|---|
| Sod canonical | 0.006325 | 0.2000 |
| Available snapshot | 0.007 (final) | 0.2214 |
| Available snapshot | 0.006 | 0.1897 |

**Sod canonical τ = 0.2 corresponds to t = 6.325 ms, which falls BETWEEN saved snapshots.**

Comparison strategies (pick one, document in test module):

- **(A) Recommended:** Recompute exact Riemann at τ = 0.2214, compare to OpenFOAM 0.007 s snapshot directly. Zero interpolation error.
- **(B)** Re-run OpenFOAM with `writeInterval` 0.006325 s to write exactly at Sod canonical time. Enables direct Table II overlay.
- **(C)** Linear interpolation OpenFOAM 6 ms ↔ 7 ms. **Discouraged** — introduces artefacts at shock/contact discontinuities that inflate L∞ error.

---

## 4. Sod (1978) Table II — Reference profile at τ = 0.2

Glimm's-method solution, 9 interior grid points, γ = 1.4. Effectively exact Riemann to 3 sig figs.

```csv
x_over_L,rho_over_rho0,u_over_u0,P_over_P0,e_over_e0,Gamma_plus
0.1,1.000,0.000,1.000,2.500,2.958
0.2,1.000,0.000,1.000,2.500,2.958
0.3,0.869,0.164,0.822,2.363,2.958
0.4,0.426,0.927,0.303,1.778,2.958
0.5,0.426,0.927,0.303,1.778,2.958
0.6,0.426,0.927,0.303,1.778,2.958
0.7,0.426,0.927,0.303,1.778,2.958
0.8,0.266,0.927,0.303,2.853,3.624
0.9,0.125,0.000,0.100,2.000,2.646
```

With the OpenFOAM SI unit case, this data is redimensionalised 

```csv
x_m,rho_kg_per_m3,u_m_per_s,P_Pa,e_J_per_m3,Gamma_plus_m_per_s,t_s,tau_sod
-4.0,1.000,0.000,1.0000e+05,2.5000e+05,935.4,6.3246e-03,0.2
-3.0,1.000,0.000,1.0000e+05,2.5000e+05,935.4,6.3246e-03,0.2
-2.0,0.869,51.86,8.2200e+04,2.3630e+05,935.4,6.3246e-03,0.2
-1.0,0.426,293.14,3.0300e+04,1.7780e+05,935.4,6.3246e-03,0.2
0.0,0.426,293.14,3.0300e+04,1.7780e+05,935.4,6.3246e-03,0.2
1.0,0.426,293.14,3.0300e+04,1.7780e+05,935.4,6.3246e-03,0.2
2.0,0.426,293.14,3.0300e+04,1.7780e+05,935.4,6.3246e-03,0.2
3.0,0.266,293.14,3.0300e+04,2.8530e+05,1146.0,6.3246e-03,0.2
4.0,0.125,0.000,1.0000e+04,2.0000e+05,836.7,6.3246e-03,0.2
```

$\Gamma_+ = c/(\gamma-1) + u/2$ is the right-going Riemann invariant (conservation check).
$e$ is **total** energy per unit volume: $e = P/(\gamma-1) + \tfrac{1}{2}\rho u^2$.
---

## 5. Exact Riemann solution — Sod Appendix (eqs. 46-62, Fig. 16)

### 5.1 Star-state pressure (transcendental, solve by Newton-Raphson)

$$
u_R - u_L + f_L(P^*) + f_R(P^*) = 0
$$

where for side $K \in \{L, R\}$:

- **Shock** ($P^* > P_K$):
  $f_K(P^*) = (P^* - P_K)\sqrt{\dfrac{A_K}{P^* + B_K}}$, with
  $A_K = \dfrac{2}{(\gamma+1)\rho_K}$, $B_K = \dfrac{\gamma-1}{\gamma+1}P_K$

- **Rarefaction** ($P^* \leq P_K$):
  $f_K(P^*) = \dfrac{2 c_K}{\gamma-1}\left[(P^*/P_K)^{(\gamma-1)/(2\gamma)} - 1\right]$,
  with $c_K = \sqrt{\gamma P_K/\rho_K}$

For Sod ICs: left wave = rarefaction, right wave = shock.

Star velocity:
$$
u^* = \tfrac{1}{2}(u_L + u_R) + \tfrac{1}{2}[f_R(P^*) - f_L(P^*)]
$$

### 5.2 Reference values at τ = 0.2 (γ = 1.4, Sod ICs)

| Quantity | Value |
|---|---|
| $P^*$ | 0.30313 |
| $u^*$ | 0.92745 |
| $\rho_L^*$ (post-rarefaction) | 0.42632 |
| $\rho_R^*$ (post-shock) | 0.26557 |
| Shock speed | 1.75216 |
| Contact speed | 0.92745 |
| Rarefaction head speed | $-\sqrt{1.4}$ ≈ −1.18322 |
| Rarefaction tail speed | $u^* - c_L^*$ ≈ −0.07027 |

### 5.3 Sampling logic (Sod Fig. 16 flow chart, 10 cases)

For each grid point $\tilde{x}_i$, define $\xi = (\tilde{x}_i - 0.5)/\tilde{t}$ (self-similar coordinate).
Branch on $\xi$ relative to the four wave speeds above:

1. $\xi < $ rarefaction head speed → left state
2. rar. head < $\xi < $ rar. tail → **inside left rarefaction fan**
3. rar. tail < $\xi < u^*$ → left star state
4. $u^* < \xi < $ shock speed → right star state
5. $\xi > $ shock speed → right state

**Rarefaction fan interior** (Sod eqs. 49-53):

$$
u = \dfrac{2}{\gamma+1}\left[c_L + \dfrac{\gamma-1}{2}u_L + \xi\right]
$$
$$
c = c_L - \dfrac{\gamma-1}{2}(u - u_L)
$$
$$
\rho = \rho_L \left(\dfrac{c}{c_L}\right)^{2/(\gamma-1)}, \quad
P = P_L \left(\dfrac{c}{c_L}\right)^{2\gamma/(\gamma-1)}
$$

**Isentropic rarefaction star density** (eq. 48): $\rho_L^* = \rho_L (P^*/P_L)^{1/\gamma}$

**Rankine-Hugoniot shock star density** (eq. 15 rearranged):
$$
\rho_R^* = \rho_R \dfrac{(P^*/P_R) + (\gamma-1)/(\gamma+1)}{((\gamma-1)/(\gamma+1))(P^*/P_R) + 1}
$$


