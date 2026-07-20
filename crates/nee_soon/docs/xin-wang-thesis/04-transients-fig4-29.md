<!--
PROVENANCE / AI-ASSISTED EXTRACTION NOTICE
==========================================
Source : Xin Wang, "Coupled neutronics and thermal-hydraulics modeling for
         pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)",
         Ph.D. dissertation, UC Berkeley, 2018. Chapter 4 + Appendices B, C.
         https://escholarship.org/uc/item/40q3985m  (open literature)
AI-ASSISTED extraction of the Mk1 PB-FHR case data + the Figure-4.29 transient.
The transient-curve tables below are DIGITISED BY EYE from the printed plots
(Figs. 4.27-4.29) and are approximate (read to the nearest ~5 C / ~0.1e8 W);
they are NOT tabulated in the thesis. Every number here is UNVERIFIED draft
material — re-read the source figures/tables before using it as a benchmark.
Table/figure/page numbers are the dissertation's own.
-->

# Mk1 PB-FHR case + the Figure 4.29 control-rod-removal transient

This is the concrete case the OUTRAM PARK workflow reproduces:
**Mark-1 Pebble-Bed FHR (Mk1)**, a 236 MW(th) pre-conceptual UC Berkeley design.
The validation target is **Figure 4.29 — the maximum fuel temperature during a
control-rod-removal transient**.

## Core design (Table 4.1)

| Parameter | Value |
|---|---|
| Thermal power | 236 MW |
| TRISO packing fraction | 40 % |
| Pebble packing fraction | 60 % |
| Fuel enrichment (% U-235) | 19.9 % |
| Flibe enrichment (% Li-7) | 99.999 % |
| Number of fuel pebbles | 470 000 |
| Number of graphite (blanket) pebbles | 218 000 |
| TRISO particles per fuel pebble | 4 730 |
| Coolant inlet temperature | 600 °C |
| Coolant bulk-average outlet temperature | 700 °C |
| Coolant flow | 976 kg/s |
| Estimated coolant bypass | 5 % |
| Inner (center) reflector radius | 35 cm |
| Average power density | 23 MW/m³ (fuel-region peak ≈ 22.7 MW/m³) |
| Volume of active fuel region | 10.4 m³ |

Core is annular: center graphite reflector + control-rod channels, active
fuel-pebble region, ~20 cm graphite **blanket-pebble** ring (protects the outer
reflector), outer reflector, then core barrel / downcomer / vessel. Core diameter
≈ 3.5 m (Fig. 4.1). Cross-flow coolant: ~30 % enters from the downcomer at the
bottom, the rest is injected radially from the center-reflector channels.

## Fuel pebble + TRISO geometry (Tables 4.2, 4.3)

Mk1 fuel pebbles are **3 cm** diameter, three-layer (low-density graphite core,
annular fuel layer, dense graphite shell — pebbles are slightly buoyant in flibe):

| Fuel pebble layer | Dimension |
|---|---|
| Graphite core diameter | 25 mm |
| Fuel (annular) layer thickness | 1.5 mm |
| Outer graphite shell thickness | 1 mm |

| TRISO layer | Dimension |
|---|---|
| Fuel kernel diameter | 400 µm |
| Buffer layer thickness | 100 µm |
| Inner PyC (iPyC) thickness | 35 µm |
| SiC layer thickness | 35 µm |
| Outer PyC (oPyC) thickness | 35 µm |

TRISO kernel material is $UC_{0.5}O_{1.5}$ (Table 4.4).

## Energy group structure (Table 3.4) — 8 groups

The 8-group structure chosen to capture U-235, U-238, and flibe XS features.
**Lower** energy boundary of each group:

| Group | Lower boundary (MeV) | Lower boundary (eV) |
|---|---|---|
| 1 | 1.4 × 10⁰ | 1.40 × 10⁶ |
| 2 | 2.5 × 10⁻² | 2.50 × 10⁴ |
| 3 | 4.8 × 10⁻⁵ | 4.80 × 10¹ |
| 4 | 4.0 × 10⁻⁶ | 4.00 × 10⁰ |
| 5 | 5.0 × 10⁻⁷ | 5.00 × 10⁻¹ |
| 6 | 1.9 × 10⁻⁷ | 1.90 × 10⁻¹ |
| 7 | 5.8 × 10⁻⁸ | 5.80 × 10⁻² |
| 8 | 0.0 | 0.0 (thermal cutoff) |

The upper boundary of group 1 is the ENDF maximum (~20 MeV). Group constants are
generated from a Serpent model with explicit TRISO/pebble packing, ENDF/B-VII.0.

## Materials (Table 4.4, reference Serpent model)

| Component | Density (g/cm³) | Material | T (K) |
|---|---|---|---|
| Fuel: shell | 1.75 | graphite | 900 |
| Fuel: kernel (pebble core) | 1.59 | graphite | 1000 |
| Fuel: coolant | 1.95 | flibe | 1000 |
| TRISO: matrix | 1.7 | graphite | 1000 |
| TRISO: kernel | 10.5 | $UC_{0.5}O_{1.5}$ | 1000 |
| TRISO: buffer | 1.05 | graphite | 1000 |
| TRISO: iPyC | 1.90 | graphite | 1000 |
| TRISO: SiC | 3.18 | SiC | 1000 |
| TRISO: oPyC | 1.90 | graphite | 1000 |
| Blanket: pebbles | — | graphite | 900 |
| Blanket: coolant | 1.97 | flibe | 900 |
| Center reflector | 1.74 | graphite | 900 |
| Control rods | 2.4 | natural B₄C | 900 |
| Control-rod coolant | 1.97 | flibe | 900 |
| Outer reflector | 1.74 | graphite + boron | 900 |
| Outer-reflector coolant channel | 2.08 | graphite + flibe | 900 |
| Core barrel | 8 | SS316 | 900 |
| Reactor vessel | 8 | SS316 | 900 |

SS316 composition (Table 4.5, wt %, density 8.03 g/cm³): C 0.080, Fe 65.345,
Ni 12.000, Cr 17.000, Mo 2.500, Si 1.000, Mn 2.000, S 0.030, P 0.045.

## Control rods (Table 4.6)

| Parameter | Value |
|---|---|
| Number of control rods | 8 (cross-shaped, in center reflector) |
| Control-rod channel diameter | 10 cm |
| Rod width / thickness | 8 cm / 2 cm |
| Bottom height, fully inserted | 112.5 cm |
| Bottom height, fully retracted | 492.85 cm |
| Density | 2400 kg/m³ |
| Material | Boron Carbide (B₄C) |

## Flibe coolant properties (Table B.3, 95 % confidence)

| Property | Value | Uncertainty |
|---|---|---|
| Melting point | 459 °C | — |
| Boiling point | 1430 °C | — |
| Viscosity | $1.16\times10^{-4}\,e^{3755/T[K]}$ kg/m·s | 20 % |
| Heat capacity | 2386 J/kg·K | 3 % |
| Thermal conductivity | 1.1 W/m·K | 10 % |
| Density | $2413 - 0.488\,T[K]$ kg/m³ | 2 % |

## Fuel-pebble thermophysical properties (App. B / Table 4.11)

Equivalent (homogenised) fuel-pebble conductivity vs. temperature $T$ [°C] and
fast-neutron dose [$10^{21}$] (Eq. B.1/4.3):

$$\lambda = 1.2768\left(\frac{0.6829 - 0.3906\times10^{-4}T}{dose + 1.931\times10^{-4}T} + 1.228\times10^{-4}T + 0.042\right)$$

($\lambda$ in W/cm·K, valid $T<1200$ °C). Ranges ~15 W/m·K (irradiated) to
>60 W/m·K (fresh); **15 W/m·K** is the conservative constant. Per-pass conductivity
(Table 4.11, W/K·m): pass 1 = 40, passes 2–8 = 17. Specific heat (Eq. B.2):
$C_p = 1.75(0.645 + 3.14\times10^{-3}T - 2.809\times10^{-6}T^2 + 0.959\times10^{-9}T^3)/\rho$,
≈ 1564 J/kg·K at the nominal 900 °C.

## Porous-media closure values (Table 4.10)

Ergun coefficients used in the Mk1 porous-media momentum equation:
$E_1 = 150$, $E_2 = 1.75$, $c_F = 0.52$. Wakao correlation
$Nu = 2 + 1.1\,Pr^{1/3}Re^{0.6}$ for the local convective coefficient. Fuel region
split radially + axially into **6 zones**, each with its own MGXS fit (multi-scale).

## The control-rod-removal transient (§4.5.2)

**Scenario.** Control-rod ejection is impossible in an FHR (low pressure), but a
control-rod-removal *accident* (multiple control-system failures) is credible;
removal speed is limited by the rod-lifting machinery. Wang simulates a **prompt**
removal as a bounding case for safety analysis.

**Definition:**

- **Initial state.** Control rods inserted **symmetrically** to the height at
  which the core would have **3941 pcm** excess reactivity if all rods were
  removed. Core has fresh fuel; nominal operating conditions (inlet 600 °C).
- **Trigger.** Remove **3 of the 8** control rods from the core (prompt, i.e.
  instantaneous — bounding case), Fig. 4.26.
- **Duration.** 100 s simulated.
- **Model.** Coupled $SP_3$ neutronics + porous-media TH + multi-scale fuel
  temperature (§4.3), fresh fuel.
- **Long-transient boundary.** A simplified CTAH (Coiled-Tube Air Heater) heat-
  exchanger model (Eqs. 4.1–4.2, $\eta = 0.9$, $T_{air,in} = 418.6$ °C,
  $C_{min} = 461540$ J/K) closes the primary loop so the coolant inlet responds to
  returning coolant over transients longer than one circulation time.

**Reported results (§4.5.2 text):** power rises then stabilises slightly higher
from fuel + coolant temperature feedback; **peak power ≈ 30 % above initial**. The
flow-weighted average coolant outlet temperature rises **≈ 35 °C**. The maximum
fuel temperature (Fig. 4.29 — center of the hottest fuel kernel) stays **far below
the safety limit (1600 °C)** — the core is resilient to the accident.

## Digitised reference curves (Figs. 4.27–4.29) — APPROXIMATE

> **These tables are read by eye off the printed plots** and are approximate
> (~5 °C / ~0.1×10⁸ W resolution). They are the digitised reference for the
> Stage-4 comparison; a careful re-digitisation from the source figures should
> replace them before any quantitative pass/fail claim.

### Figure 4.27 — Full core power [W] vs t [s]

| t (s) | Power (×10⁸ W) |
|---|---|
| 0⁻ (pre) | 2.36 (≈ 236 MW nominal) |
| 0⁺ | ~2.5 (step) |
| 3 | ~3.28 |
| 5 | ~3.30 (peak, ≈ +30 %) |
| 10 | ~3.22 |
| 20 | ~3.16 |
| 40 | ~3.12 |
| 60 | ~3.10 |
| 100 | ~3.08 (settles ≈ +30 % above initial) |

### Figure 4.28 — (Flow-weighted) average flibe outlet temperature vs t [s]

Plot y-axis reads 980–1015 (°C label). **Note a labelling inconsistency:** the
§4.5.2 text says the flow-weighted outlet rises ≈ 35 °C and stays *below 750 °C*,
whereas the Fig. 4.28 axis is 980–1015. Recorded as-plotted; resolve against the
source before use.

| t (s) | T_flibe (as plotted, °C) |
|---|---|
| 0 | ~980 |
| 5 | ~989 |
| 20 | ~992 |
| 40 | ~1006 |
| 60 | ~1012 |
| 80 | ~1014 |
| 100 | ~1015 (plateau) |

### Figure 4.29 — Maximum fuel temperature [°C] vs t [s]  ← VALIDATION TARGET

| t (s) | T_fuel,max (°C) |
|---|---|
| 0 | ~908 |
| 2 | ~960 |
| 5 | ~982 |
| 8 | ~988 (peak) |
| 15 | ~982 |
| 28 | ~975 (local minimum) |
| 40 | ~982 |
| 60 | ~992 |
| 80 | ~1000 |
| 100 | ~1006 |

Shape: a sharp prompt jump to a ~988 °C peak (~8 s) as the removed rods insert
reactivity; a shallow dip to ~975 °C (~28 s) as Doppler + coolant feedback and
heat redistribution bite; then a slow climb toward ~1005–1006 °C by 100 s as the
core settles at the higher power. Absolute maximum stays ~600 °C below the
1600 °C fuel-failure limit — the resilience result.

## What Stage 4 must match

1. **Fig. 4.29 curve shape + magnitude** — the primary target (peak ≈ 988 °C near
   8 s; 100 s value ≈ 1005 °C; max well below 1600 °C).
2. **Fig. 4.27 power** — peak ≈ +30 %, settling ≈ +30 % above 236 MW.
3. Consistency of the reactivity budget (3941 pcm all-out; 3-of-8 removed) and the
   feedback coefficients (fuel Doppler, flibe density) derived in Stage 1.
