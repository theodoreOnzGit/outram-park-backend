# MSRE N-4: reactivity loss from fuel circulation (gh:#1130)

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). AI draft,
> unreviewed. **Depends on unreviewed moltres** (maintainer, 2026-11-03).
> Verification of moltres's precursor drift, then validation of the line
> model against the measurement.

## Methodology

**Reference: ORNL-TM-1626** (B. E. Prince, 1966), the maintainer's chosen
source (gh:#1130, 2026-11-03).

- **Measured loss:** 0.212 ± 0.004 % δk/k, U-235, zero power (report p. 26, PDF p. 32).
- **Prince's line model:** 0.222 % (Eq. 14, w = 0, same page).
- **Pass criterion:** none against the measurement (validation; the finding
  is the disagreement). The verification gate is moltres against the exact
  solution of the same model (below).

**What moltres does here.** `CirculatingFuelSolver` would need MSRE axial
diffusion constants, and none was found published in TM-0728, ORNL-4233,
TM-1070 or TM-1626. So the test drives the part of moltres that carries
N-4's physics, `PrecursorDrift` (upwind finite-volume advection and decay
on a ring), with **TM-1626's own** fission density and importance. This
rebuilds Prince's line model on moltres.

**Inputs, all from TM-1626:**

| input | source | read |
|---|---|---|
| fission density in salt `P°φs(z)`, fast adjoint `φ1†(z)` | Fig. 4, report p. 21, PDF p. 27 | digitised by eye, 2026-11-09, from a 250 dpi render (`images/tm1626_fig4_250dpi.png`); the 26 points are `FIG4` in the test; reading error about ±0.02 |
| region extents: bottom plenum −22…0, graphite 0…168, top plenum 168…190 cm | the region bands over Figs. 4 and 5 | read from the figures |
| `β_i`, `λ_i` (β = 0.006661) | Table 1, report p. 24, PDF p. 30 | read from the scan |
| residence times 3.8 / 9.4 / 3.9 / 8.1 s (bottom plenum, core, top plenum, external) | Table 1 | read from the scan |

**Model** (`tests/msre_n4_precursor_drift_tm1626.rs`):

- **Coordinate:** transit time along the flow path, with plug flow in every
  region. The salt volume per path element is `Q dt`, so the regions' fuel
  volume fractions (0.225, 1.0, 0.91) drop out exactly.
- **Moltres:** a 25.2 s ring at unit speed, `D_C = 0`, `S_f = P°φs`, zero in
  the external loop.
- **Loss** `= β − Σ λ_i ⟨φ†, C_i⟩ / ⟨φ†, S_f⟩`, with prompt and delayed
  neutrons both weighted by the fast adjoint (TM-1626 p. 20). Static fuel
  gives zero loss exactly (asserted).
- **Exact reference:** `dc/dt = β_i S_f − λ_i c` integrated exactly around
  the loop (exponential integrator, periodic closure), with either a
  plug-flow or a well-stirred lower plenum (Prince's treatment, which the
  ring cannot represent).

**Prediction posted before any code** (gh:#1130): moltres converges at first
order to the exact loss and is within 0.001 % at N = 10080; the loss lands
in 0.214–0.230 %.

## Results (2026-11-09, release build)

| quantity | loss (% δk/k) |
|---|---|
| exact, lower plenum plug flow | 0.2293 |
| exact, lower plenum well-stirred | 0.2299 |
| moltres, N = 2520 | 0.2293 (error −1.75e-7) |
| moltres, N = 10080 | 0.2293 (error −4.40e-8; ratio 3.97) |
| Prince, TM-1626 Eq. 14 | 0.222 |
| **measured** | **0.212 ± 0.004** |

- **Verification: passed.** Moltres converges at first order (error ratio
  3.97 for a 4× finer mesh) and agrees with the exact solution to 4e-8.
- **The precursor distribution matches Prince's Fig. 5.** Circulating
  delayed emission over the stationary peak, at z = 0, 120, 168 and
  190 cm: 0.111, 0.625, 0.512, 0.246 here, against about 0.10, 0.63, 0.51,
  0.26 read from the figure.
- **Digitisation uncertainty, about ±0.005 %** (`scripts/sensitivity.py`):

  | variant | loss (%) |
  |---|---|
  | as read | 0.2293 |
  | graphite height 166 / 170 cm | 0.2267 / 0.2322 |
  | plenum importance +0.02 / −0.02 | 0.2258 / 0.2328 |
  | fission density +0.01 everywhere | 0.2276 |

- **Against Prince's 0.222 %:** +0.007 ± 0.005, consistent.
- **Against the measurement:** **+0.017 ± 0.006 % (+8 %)**, inside the
  posted prediction. The line model, fed Prince's own shapes, reads high,
  as Prince's did (+0.010).
- **Plenum ablation:** with zero importance in both plenums the loss is
  **0.265 %**; the plenums' delayed neutrons are worth **0.036 % δk/k**.
  TM-1626 p. 27 says the plenums were the difference from Haubenreich's
  earlier, larger estimate (ORNL-4233 quotes it as 0.30 %).
- **Lumped comparison** (teh-o-prke, same Table 1 inputs, gh:#1130): core
  only 0.2301 %, core and plenums at full worth 0.1249 %. The spatial
  model lands next to the core-only lump: the plenums add worth that the
  axial skew of the precursors toward the low-importance core top takes
  away.

**What is not covered:** moltres's own diffusion (no sourced MSRE
constants), the U-233 loading (unsourced), and the flux shape itself, which
is TM-1626's calculation, not ours.

Runtime: under 0.1 s for the whole test (4-core Intel Xeon, 15.7 GiB RAM,
Linux, CPU only, single thread).
