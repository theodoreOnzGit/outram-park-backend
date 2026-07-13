# ThermoSysPro: Two-Phase Choked and Critical Flow

**Package:** ThermoSysPro v4.1 (EDF, Modelica)  
**Scope:** How the package handles (or deliberately avoids) two-phase choked / critical flow

---

## Executive Summary

ThermoSysPro **does not implement explicit two-phase critical flow models**. There is no Henry-Fauske correlation, no Homogeneous Equilibrium Model (HEM) discharge, no Moody slip-flow critical mass flux, and no omega-method choking criterion anywhere in the codebase. Instead, the library uses a set of continuous, algebraic pressure-drop correlations that remain well-behaved through the two-phase region but never enforce a sonic or choked-flow limit. The closest the library comes to "critical flow" is Stodola's ellipse law for steam turbines, which caps mass flow based on pressure ratio — but this is not a two-phase choking model.

The practical consequence is that models built with ThermoSysPro will **not limit mass flow at a choked condition** when two-phase flashing occurs in a restriction. This is acceptable for the library's target use cases (power-plant thermal cycle simulation at quasi-steady conditions) but is a gap for safety-valve discharge, pipe break, or pressure-relief analysis.

---

## 1. Two-Phase Friction in Pipes

**Files:**
- `ThermoSysPro/Fluid/HeatExchangers/DynamicTwoPhaseFlowPipe.mo` (lines 478–497)
- `ThermoSysPro/Fluid/HeatExchangers/DynamicTwoPhaseFlowRiser.mo` (identical structure)
- `ThermoSysPro/WaterSteam/HeatExchangers/DynamicTwoPhaseFlowPipe.mo` (legacy namespace)

### Algorithm

The momentum balance for each cell `i` is:

```
P[i] - P[i+1] - dpf[i] - dpg[i] - dpa[i] = 0   (quasi-static)
(1/A)*d(Q[i])/dt*dx = ...                          (with inertia option)
```

**Friction pressure drop** uses a two-phase friction multiplier `filo`:

```modelica
dpf[i] = dpfCorr * khi[i] * Q[i]*abs(Q[i]) / (2*A^2*rhol2[i])

khi[i] = filo[i] * lambdal[i] * dx2/D
```

The Darcy-Weisbach liquid friction factor `lambdal` uses a Colebrook-like approximation:

```modelica
lambdal[i] = 0.25 * (log10(13/Rel2[i] + rugosrel/3.7/D))^(-2)   if Re > 1
```

The **two-phase multiplier** `filo` is a piecewise correlation in steam quality `xv2[i]`:

| Quality range | Expression |
|---|---|
| `xv2 < 0` (subcooled) | `filo = 1` |
| `0 <= xv2 < 0.8` | `filo = 1 + a*xv2*rgliss / (19 + P_bar) / exp(P_bar/84)` |
| `xv2 >= 0.8` | blend toward all-vapor limit using `rhol/rhov * lambdav/lambdal` |

**Key parameters:**
- `a = 4200` — empirical pressure-loss coefficient
- `rgliss = 1` — slip ratio (homogeneous flow assumption, no velocity slip between phases)
- `P_bar = P/1e5` — pressure in bar used in the empirical terms

### What this means for critical flow

This is a **subsonic friction model**, not a critical flow model. The multiplier `filo` accounts for the increased pressure drop in two-phase flow (analogous to Lockhart-Martinelli style correlations), but:
- It imposes no upper bound on mass flux.
- It assumes homogeneous flow (`rgliss = 1` slip ratio).
- It does not evaluate the local speed of sound or Mach number.
- Flow reversal is handled continuously via `ThermoSquare(Q, eps)`.

---

## 2. Valve and Orifice Models

All restriction models in ThermoSysPro use a single algebraic form:

```
deltaP * Cv * |Cv| = K * ThermoSquare(Q, eps) / rho^n
```

where `ThermoSquare(Q, eps)` is a smooth approximation to `Q*|Q|` near zero flow for numerical stability.

### 2.1 ControlValve

**File:** `ThermoSysPro/Fluid/PressureLosses/ControlValve.mo` (lines 76–82)

```modelica
// option_rho_water = 1:
deltaP * Cv * abs(Cv) = 1.733e12 * ThermoSquare(Q, eps) / rho^2;

// option_rho_water = 2 (reference density at 15.5 degC):
deltaP * Cv * abs(Cv) = 1.733e12 * ThermoSquare(Q, eps) / (rho * rho_15);
```

The constant `1.733e12` is the dimensional conversion factor for the ISA Cv definition (US gpm / psi^0.5). The fluid density `rho` is evaluated at the mean pressure `Pm = (P1 + P2)/2` using IF97 via `Density_Ph`.

**No choking:** As `deltaP -> P1` (outlet approaches vacuum), `Q` grows without bound. There is no critical-pressure-ratio constraint.

### 2.2 DynamicReliefValve

**File:** `ThermoSysPro/Fluid/PressureLosses/DynamicReliefValve.mo` (lines 155–172)

This model adds clapper dynamics (Newton's second law for the disc) but uses the same pressure-loss equation:

```modelica
deltaP * Cv * abs(Cv) = K * ThermoSquare(Q, eps) / (rho * rho60F);
```

with `K = 1.733e12` and `rho60F = 998.98 kg/m3` (water at 60°F, the ISA reference density).

Three Cv characteristic modes are supported:
- **mode_caract = 0** — linear: `Cv = Ouv * Cvmax`
- **mode_caract = 1** — tabulated with linear or spline interpolation
- **mode_caract = 2** — conic clapper geometry:
  ```modelica
  Cv = sqrt(pi*A1*K / (0.3*rho60F)) * (z - z_min) / sqrt(1 + pi/A1*(z - z_min)^2)
  ```

The clapper force balance is:

```
m*a = Fp + Fr + Fd + Fh + Fdyn
Fh   = C1.P * A - C2.P * A2           (net hydraulic force)
Fdyn = sign(vh - v) * (Cd*rho/2*(vh - v)^2) * A   (drag)
```

**No choked-flow correction.** The model simulates the mechanical opening dynamics of a safety valve at sub-critical pressure ratios.

### 2.3 SingularPressureLoss (orifice/fitting)

**File:** `ThermoSysPro/Fluid/PressureLosses/SingularPressureLoss.mo` (line 59)

```modelica
deltaP = K * ThermoSquare(Q, eps) / rho;
```

Simplest form: quadratic deltaP-Q relation with a user-supplied loss coefficient `K`. Single-phase density only.

### 2.4 NonBoilingValve (legacy WaterSteam namespace)

**File:** `ThermoSysPro/WaterSteam/PressureLosses/NonBoilingValve.mo` (lines 52–59)

This is the most explicit treatment of saturation in any restriction model. Instead of modelling choked flow, it **prevents flashing** by enforcing that inlet pressure exceeds the saturation pressure by a security margin `Psecu`:

```modelica
Pebul = IF97.Pressure_sat_hl(Hec);   // saturation pressure at inlet enthalpy
Pec   = if (Psc - Psecu < Pebul) then Pebul + Psecu else Psc;
```

If the downstream pressure would cause the inlet to flash, the model raises `Pec` algebraically. This is a **numerical guard**, not a physical choked-flow model — it simply prevents the thermodynamic state from entering the two-phase dome in upstream volumes.

---

## 3. Stodola's Ellipse Law (Turbines)

**Files:**
- `ThermoSysPro/Fluid/Machines/StodolaTurbine.mo` (lines 130–135)
- `ThermoSysPro/WaterSteam/Machines/StodolaTurbine.mo` (lines 108–113)

This is the only model in ThermoSysPro that has a pressure-ratio-dependent flow limit:

```modelica
// Superheated or supercritical:
Q = sqrt((Pe^2 - Ps^2) / (Cst * Te));

// Two-phase inlet (x < 1):
Q = sqrt((Pe^2 - Ps^2) / (Cst * Te * proe.x));
```

**Parameters:**
- `Cst = 1e7` — Stodola ellipse coefficient (calibrated to rated conditions)
- `Pe`, `Ps` — inlet/outlet absolute pressures [Pa]
- `Te` — inlet temperature [K]
- `proe.x` — inlet steam quality (dryness fraction, 0–1)

### Relationship to critical flow

Stodola's ellipse (`Q ~ sqrt(Pe^2 - Ps^2)`) is an empirical turbine characteristic, **not** a critical flow correlation. It:
- Saturates naturally when `Ps -> 0` (Q approaches a maximum), mimicking the shape of choked behaviour for turbine stages.
- Adjusts for wet steam by dividing by `x` (quality), increasing the apparent flow resistance in two-phase conditions.
- Does **not** use the speed of sound, Mach number, or any thermodynamic critical-point calculation.

The two-phase branch (`Q = sqrt((Pe^2-Ps^2)/(Cst*Te*x))`) reduces mass flow for low-quality mixtures entering the turbine, reflecting the higher specific volume of wet steam. This is a correction factor, not a choking model.

---

## 4. What Is Not Present

The following standard two-phase critical flow models are **absent** from ThermoSysPro:

| Model | Description | Status |
|---|---|---|
| **HEM (Homogeneous Equilibrium Model)** | Critical mass flux via isentropic slip=1 sound speed | Not present |
| **Henry-Fauske (1971)** | Non-equilibrium subcooled/flashing discharge | Not present |
| **Moody (1965)** | Slip-flow critical mass flux with optimised slip ratio | Not present |
| **Leung's omega method** | Simplified ERM for flashing two-phase relief | Not present |
| **EPRI/RELAP-style correlations** | Critical flow with departure from equilibrium | Not present |
| **Critical pressure ratio constraint** | `P_crit/P_inlet` cutoff for flow | Not present |
| **Local speed of sound evaluation** | `c = sqrt(dP/drho|s)` two-phase | Not present |

---

## 5. Design Rationale

ThermoSysPro is targeted at **power-plant thermal cycle simulation** (boilers, turbines, condensers, heat exchangers) under quasi-steady operating conditions. In this context:

- Flow restrictions operate far from critical conditions (pressure ratios typically 0.5–0.95).
- The primary simulation goal is energy balance accuracy, not mass-flux limiting.
- Numerical robustness of the DAE solver takes priority; `ThermoSquare` smoothing and `noEvent()` guards reflect this.
- Safety-valve discharge and pipe-break scenarios are out of scope; these require dedicated tools (RELAP5, TRACE, CATHARE, or custom Modelica libraries with explicit choking).

---

## 6. Relevant Source Locations

| Component | File | Key Lines |
|---|---|---|
| Two-phase pipe friction multiplier | `Fluid/HeatExchangers/DynamicTwoPhaseFlowPipe.mo` | 478–497 |
| Control valve Cv equation | `Fluid/PressureLosses/ControlValve.mo` | 76–82 |
| Relief valve clapper dynamics + Cv | `Fluid/PressureLosses/DynamicReliefValve.mo` | 119–172 |
| Orifice/fitting loss | `Fluid/PressureLosses/SingularPressureLoss.mo` | 59 |
| Flash prevention (non-boiling valve) | `WaterSteam/PressureLosses/NonBoilingValve.mo` | 52–59 |
| Stodola ellipse (turbine) | `Fluid/Machines/StodolaTurbine.mo` | 130–135 |
| Martinelli / boiling heat transfer | `Correlations/Thermal/WBInternalTwoPhaseFlowHeatTransferCoefficient.mo` | 76–85 |
