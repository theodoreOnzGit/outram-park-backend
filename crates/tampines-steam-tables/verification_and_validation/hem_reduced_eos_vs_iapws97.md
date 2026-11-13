# Reduced equations of state against IAPWS-IF97 (gh:#1057)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). Code: `src/hem/reduced_eos.rs`;
tests: `src/hem/reduced_eos/tests.rs` and, for the run-time dispatch,
`crates/tampines/tests/hem_reduced_eos_dispatch.rs`.

## What was built

Four analytic closures implementing `HemEquationOfState`, so `HemPipe` runs on
each unchanged:

| Closure | Form | Source |
|---|---|---|
| `StiffenedGasHem` | `p = (γ−1)ρ(e−q) − γp∞`, caloric law `ρ = (p+p∞)/((γ−1)c_v T)` | RELAP-7 Theory Manual §2.2.3.4, Eqs 78–93; parameters Le Métayer, Massoni & Saurel (2004) |
| `StiffenedGasPair` | saturation from equal Gibbs energies, Newton on Eq 215; `L_v` from Eq 220 | RELAP-7 §3.2.5, after Le Métayer et al. |
| `IsentropicStiffenedGasHem` | `p = (p₀+p∞)(ρ/ρ₀)^γ − p∞` | RELAP-7 §2.2.3.2, Eq 59; Courant–Friedrichs water |
| `BarotropicHem` | `p = p₀ + a²(ρ−ρ₀)` | RELAP-7 §2.2.3.1, Eq 54 |
| `LinearEosHem` | `p = p₀ + K_ρ(ρ−ρ₀) + K_T(T−T₀)`, `e = e₀ + c_v(T−T₀)` | RELAP-7 §2.2.3.3, Eqs 66–77; **coefficients from IAPWS-IF97**, not the manual's tables |

The RELAP-7 manual is private and is cited by section and equation only. These
are **verification closures, not substitutes for IAPWS-IF97**.

## Predictions (written before the first run, 2026-11-01)

1. Stiffened-gas `T_sat(p)` within 2 % of IF97 over 298–473 K.
2. Latent heat within 5 %.
3. Liquid at (1 MPa, 400 K): density 5–10 % high, sound speed within 2 %.
4. The IF97-derived linear EOS: exact at its reference point, within 1 % over
   ±5 K and ±0.5 MPa.
5. Barotropic and isentropic forms equal to their closed forms to 1e-12.
6. Water shock tube (10 → 1 MPa, 400 K liquid): plateau within 5 % of IF97,
   wave position within the sound-speed difference plus one cell.

## Methodology and results (2026-11-01, release build)

### 1–2. Saturation curve and latent heat (uncalibrated)

At `T = 298, 323, …, 473` K, `p = p_sat,IF97(T)`; `T_sat,SG(p)` from Eq 215;
`L_v,SG(T)` from Eq 220 against IF97's `h_g − h_l` (regions 2 and 1 at `p_sat`).
At every computed `T_sat` the two phases' Gibbs energies agree to 1e-6
relative, which confirms the Newton solves Eq 215 as written.

| T (K) | p_sat (MPa) | T_sat,SG (K) | ΔT_sat | L_v SG (kJ/kg) | L_v IF97 | ΔL_v |
|---|---|---|---|---|---|---|
| 298 | 0.0031 | 283.30 | −4.93 % | 2368.4 | 2442.1 | −3.01 % |
| 323 | 0.0123 | 305.44 | −5.44 % | 2298.9 | 2382.3 | −3.50 % |
| 348 | 0.0384 | 327.45 | −5.91 % | 2229.4 | 2321.0 | −3.95 % |
| 373 | 0.1009 | 349.36 | −6.34 % | 2159.9 | 2256.9 | −4.30 % |
| 398 | 0.2312 | 371.24 | −6.72 % | 2090.4 | 2188.5 | −4.48 % |
| 423 | 0.4742 | 393.17 | −7.05 % | 2020.9 | 2114.1 | −4.41 % |
| 448 | 0.8893 | 415.23 | −7.32 % | 1951.4 | 2032.1 | −3.97 % |
| 473 | 1.5498 | 437.51 | −7.50 % | 1881.9 | 1940.3 | −3.01 % |

- **Latent heat: held** (worst −4.48 % against the 5 % prediction).
- **Saturation temperature: MISSED.** Worst −7.50 % against the 2 % prediction:
  the curve sits 15–35 K below IF97.

**Diagnosis of the miss.** The vapour parameter `q'` is used as RELAP-7 Table 5
prints it, −23×10³ J/(kg·K), because that is the document held. Eq 215 is very
sensitive to it: `A` contains `q'_v/((γ_v−1)c_v,v)`, so a change of 400
J/(kg·K) moves `ln p_sat` by 400/447.2 = 0.89. At 373.15 K and 101 325 Pa the
right-hand side of Eq 215 comes out 0.89 above `ln p`, so the whole
disagreement is that one rounding. The value usually reproduced from the
original paper is −23.4×10³. **It is not adopted.** The paper is not held, so
the value cannot be checked here, and choosing it because it matches IF97 would
be calibration (root `CLAUDE.md`, "PROCESS right first"). The test pins the
miss at its measured size (7.5027 %) so that any change shows. **Open: obtain
Le Métayer, Massoni & Saurel (2004), Int. J. Therm. Sci. 43, 265–276, and set
`q'_v` from its Table 1.**

### 3. Stiffened-gas liquid against IF97 at (1 MPa, 400 K)

| | SG (Le Métayer) | IF97 | Difference |
|---|---|---|---|
| density | 1020.762 kg/m³ | 937.87 kg/m³ | **+8.84 %** |
| sound speed | 1518.06 m/s | IF97 region 1 | **+0.39 %** |

Both predictions held. The stiffened-gas liquid is fitted to the saturation
curve, not to density, which is why density is the larger error.

### 4. Linear EOS generated from IF97

Built at (1 MPa, 400 K): `ρ₀`, `e₀`, `c_v` from IF97's forward region-1
equations; `K_ρ = 1/(∂ρ/∂p)_T` and `K_T = −(∂ρ/∂T)_p K_ρ` by central
differences.

- At the reference point, density and internal energy equal IF97's forward
  equation to 1e-12.
- Worst density error over ±5 K and ±0.5 MPa: **0.0064 %** (held).

Cross-check against RELAP-7 Table 3, whose row the manual says came from NIST:

| | IF97 (here) | RELAP-7 Table 3 | Difference |
|---|---|---|---|
| `K_ρ` | 1.9519e6 Pa·m³/kg | 1.9474e6 | +0.23 % |
| `K_T` | 1.6350e6 Pa/K | 1.6497e6 | −0.89 % |
| `ρ₀` | 937.87 kg/m³ | 937.87 | 0 |
| `c_v` | 3634.0 J/(kg·K) | 4220 | −14 % |

**The manual's `c_v` is water's `c_p`, not `c_v`.** At 400 K, IF97 gives `c_p`
≈ 4.26 kJ/(kg·K) and `c_v` = 3.63 kJ/(kg·K); 4.22 matches neither exactly but
sits with `c_p`. Code that copied Table 3 would carry a 14 % error in the
energy-temperature relation of the linear EOS. That is why the coefficients
are generated here, not copied. `K_ρ` and `K_T` agree to the expected
data-source difference.

One correction to the method: the first run compared against
`Iapws97Hem::state_p_t`, which closes through the backward `(p, h)` equations
and so differs from the forward equation by their own error (caught by the
1e-9 check). The reference is now the forward equation, the one the
coefficients come from.

### 5. Barotropic and isentropic stiffened gas

Pressure and sound speed equal the closed forms to 1e-12. An inconsistent
`(p, ρ)` pair is refused. The reference temperature is reported, since these
forms carry none.

### 6. Water shock tube through `HemPipe`

10 m, 200 cells, 400 K liquid, 10 MPa left of 5 m and 1 MPa right,
transmissive ends, run to 2 ms. The plateau is the mean pressure over
4.5–5.5 m. The front is the outermost cell above the midpoint of 1 MPa and the
plateau.

| | IF97 | SG | Difference |
|---|---|---|---|
| plateau | 5.4770 MPa | 5.4871 MPa | +0.18 % |
| right-running front | 8.025 m | 8.025 m | 0 |

Held. **Instrument correction:** the first run used a fixed 5.5 MPa threshold
(the midpoint of 1 and 10 MPa), which no cell right of the diaphragm reaches,
since the plateau is 5.48 MPa. The threshold is now the midpoint of 1 MPa and
each run's own plateau, the same rule for both closures.

### Run-time dispatch (`tampines`)

Each closure wrapped in `tampines::hem::HemEos` gives bit-identical states, and
a shock tube through the enum-dispatched `HemPipe` is byte-identical to the
concrete closure (`hem_reduced_eos_dispatch.rs`, 2 tests).

## Which numbers to quote

- Quote the **saturation miss (−4.9 to −7.5 %) with its cause**: a rounded
  input, not the stiffened-gas model's ceiling.
- Quote the linear-EOS agreement (0.0064 %) only for the ±5 K, ±0.5 MPa
  neighbourhood it was measured on.
- The shock-tube agreement is liquid-only and single-phase; it says nothing
  about two-phase flow.
