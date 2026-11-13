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

## On our own shapes (2026-11-10)

The same line model and moltres drift, driven by the **N-1 Monte Carlo
model's** axial shapes instead of TM-1626 Fig. 4
(`n4_on_the_n1_monte_carlo_shape`). The chain is now: our MC model → our
precursor drift → loss. It still depends on unreviewed moltres.

**Method.**
- `msre_n1_first_criticality --mo natural --seeds 1-2 --axial-csv`: arm C,
  RVS, 30P, cold. Fission and flux on an R-Z mesh: r ∈ [0, R_graphite] and
  [R_graphite, R_vessel], z in 4 + 32 + 4 bins through the plenums and the
  graphite (`data/n1_axial_tally.csv`).
- **Identity check:** scoring draws no random number, and both seeds
  reproduce their recorded k (1.009476, 1.008513) exactly.
- `scripts/axial_shape.py`:
  - **source:** fission per unit transit time; inner ring only in the
    graphite region (the outer ring there is gap, can and downcomer), both
    rings in the plenums;
  - **importance:** a one-group proxy, the scalar flux per unit volume
    inside the graphite radius. Prince used the fast adjoint, so this is an
    approximation.
- **Region extents:** the model's own (cold): −16.61, 0, 162.56,
  180.00 cm.
- **Prediction** (posted on gh:#1130 before the run): 0.214–0.244 %,
  probably toward the high end.

| quantity | loss (% δk/k) |
|---|---|
| exact, lower plenum plug flow | **0.2191** |
| exact, well-stirred lower plenum | 0.2209 |
| moltres, N = 10080 | 0.2191 (error −4.8e-8) |
| seed 1 / seed 2 shapes alone | 0.2196 / 0.2187 (statistical ±0.0005) |
| plenum importance zeroed | 0.2530 |
| *on Prince's shapes (above)* | *0.2293* |
| *Prince, TM-1626 Eq. 14* | *0.222* |
| **measured** | **0.212 ± 0.004** |

- **Inside the prediction**, but at the low end, not the high end I
  guessed: the thermal-flux proxy did not lower the plenums' worth.
- **Against the measurement: +0.007 ± 0.004 % (+3 %, 1.8σ)**, closer than
  Prince's own calculation (+0.010) and than this line model on his shapes
  (+0.017).
- **What it rests on:** the importance proxy (scalar flux, not the fast
  adjoint), plug flow, and TM-1626's residence times. ~~The proxy is the
  largest untested assumption.~~ **Tested 2026-11-10 (below).**

**The importance proxy, tested.** The tally was rerun with an energy split
at 0.625 eV, the conventional two-group thermal cutoff (TM-1626 gives no
boundary), one seed (`data/n1_axial_tally_energy_split.csv`; seed 1
reproduces k = 1.009476 and its total-flux loss 0.2196 % exactly). The
same fission source with the **fast flux** (above 0.625 eV) as the
importance (`MC_AXIAL_FAST`):

| importance | loss (% δk/k) |
|---|---|
| total scalar flux (seeds 1–2) | 0.2191 |
| **fast flux > 0.625 eV** (seed 1) | **0.2101** (moltres agrees to < 1e-5) |
| measured | 0.212 ± 0.004 |

- **The proxy carries −0.009 %:** the fast flux falls off more slowly into
  the plenums, which raises their delayed neutrons' worth.
- **Prediction** (posted on gh:#1130 before the run): 0 to −0.007 %, i.e.
  0.212–0.219 %. The sign held; the magnitude is 0.002 beyond the band,
  so the result is **just outside** it.
- **Against the measurement: −0.002 ± 0.004 % (0.5σ).**
- **Against Prince's Fig. 4:** his fast adjoint in the plenums (0.045–0.26)
  sits well below this fast flux (0.17–0.34). That is why his shapes gave
  0.2293 here. The comparison is between a fast *flux* and a fast
  *adjoint*: they are not the same quantity, and neither is the true
  importance.
- **Plenums:** zeroing their importance gives 0.2530, so their delayed
  neutrons are worth 0.034 %, against 0.036 % on Prince's shapes.
- An independent Python integrator (midpoint, `n4loss`-style) reproduces
  0.2191 %.

Runtime: the tally doubles transport time, 1722–1726 s per seed against
about 790 s without it (4-core Intel Xeon, 4 threads, 15.7 GiB RAM, Linux,
CPU only).

**What is not covered:** moltres's own diffusion (no sourced MSRE
constants), the U-233 loading (unsourced), and ~~the flux shape itself, which
is TM-1626's calculation, not ours~~ (the section above uses our own).

Runtime: under 0.1 s for the whole test (4-core Intel Xeon, 15.7 GiB RAM,
Linux, CPU only, single thread).
