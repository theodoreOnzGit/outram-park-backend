# HTR-10: Monte Carlo MGXS into a GeN-Foam R-Z diffusion core (gh:#1187)

Module: `nee_soon::mgxs_genfoam`. Example: `examples/htr10_mgxs_genfoam_rz.rs`. Status: untrusted AI-generated draft, 2026-11-03. For research, education and V&V only; not validated.

## What is compared

The comparison is a deterministic `k` against the Monte Carlo `k` of the same records.

- **Records:** the four isothermal whole-core Monte Carlo records `core_n12_t{300.15, 600, 900, 1200}` (`../htr10_mgxs_runs/`).
  - Each was run with every material at one temperature, on the N = 12 Şeker bed: 10 000 particles x 135 active generations, published-tape graphite laws.
  - Each record carries its own `k_eff` and per-material 70-group constants.
- **The chain:**
  - The constants are homogenised into the regions of an R-Z mesh.
  - They are collapsed to 4 groups (0.625 eV, 9.118 keV, 0.821 MeV), rebalanced, put in descending energy order and converted to SI.
  - They are stacked as `xsVariables { TFuel log; }` and solved by GeN-Foam's diffusion port (`DiffusionNeutronics::new_with_cell_parameters`), every cell at the record's temperature.
- **The R-Z mesh:**
  - Lines are the `htr10_rz` Fig 2.5 lines, cropped to the Monte Carlo model (`r <= 190` cm, the model's top and bottom).
  - Lines are added at the Monte Carlo bed top, bed floor, cavity top and cone floor. Cells are at most 20 cm.
  - Each cell's region is read from the **assembled Monte Carlo geometry**: 288 sample points per cell, using `Geometry::locate`.
  - The regions are:
    - the bed (volume-mixed solids and helium);
    - the fuel-free balls of the cone and tube;
    - the cavity (helium);
    - one region per Monte Carlo reflector material.
- **Boundaries:** Marshak vacuum on the outer, top and bottom faces; the axis is reflective.

## Prediction (written 2026-11-03, before the first run)

**`k_det - k_MC = +1000 pcm`, expected inside `-1000 .. +3000` pcm at every state point.**

The reasons, with their signs:

1. **P0 diffusion coefficient.** `D = 1/(3 Sigma_t)` without the transport correction (no P1 matrix was tallied) under-states `D` in graphite by about `mu_bar = 2/(3A) = 5.6 %`. Less leakage from the bed means a higher `k`: **positive, several hundred to ~1500 pcm.**
2. **Four groups condensed on whole-material spectra.** Each reflector material's constants are flux-weighted over its whole extent (near-bed and far graphite alike), and the bed's over all pebbles, cone included. **Sign not predicted; magnitude up to ~1000 pcm.**
3. **The cavity in diffusion.** Helium at 1 atm has `D ~ 10^2` m, so diffusion makes the cavity a flat-flux, nearly lossless volume where transport streams. **Sign not predicted.**
4. **Volume mixing of the bed's helium** (equal flux per unit volume in pebbles and voids). **Small.**

**The temperature trend** `k(300 K) - k(1200 K)`: the Monte Carlo trend is `0.99655 - 0.88527 = 11 128 pcm`. Predicted to be reproduced **within 20 %**, because the constants come from the same runs and the leakage errors largely cancel in a difference.

## Results (2026-11-03, `cargo run --release -p nee_soon --example htr10_mgxs_genfoam_rz`)

**Mesh:** 792 cells (20 cm), 26 regions:

- the bed (70 cells), solid fraction sampled at 0.6264;
- the cone and tube balls (100 cells), solid fraction 0.6045;
- the cavity (110 cells);
- 23 Monte Carlo reflector material slots.

The picture is `images/zones.png`.

### What the picture showed (and the defect it caught)

**What is right in the picture:**

- the bed is 123.6 cm tall, at `r < 90`;
- the cavity above it is 98 cm;
- the stepped cone and the 25 cm discharge tube are in place;
- the side reflector and the boronated outer ring are in place;
- the top reflector's TECDOC zones are layered as Fig 4.10 layers them.

**The defect it caught.** The first run labelled most of the top reflector, and two bottom bands, as **helium**. A slot was admitted only if it had flux in all 70 tally groups, and reflector slots have none in some of the top fast groups, so the majority vote fell through to helium. Now the slot must have flux in every group of the 8-group structure, which is the finest coarse structure used.

**The first run's numbers are struck:** ~~+305 / +27 / +536 / +1379 pcm at 300 / 600 / 900 / 1200 K~~. They were on the defective mesh. Their closeness to zero was an accident of a wrong geometry: half the top reflector was a near-void.

**Not checked:** each reflector cell's slot against TECDOC-1382 Fig 4.10 box by box. The palette also repeats colours after 12 regions; the legend names each one.

### Deterministic `k` against the Monte Carlo `k`, four groups

| T [K] | MC k (record) | GeN-Foam k | residual [pcm] | / MC sigma |
|---|---|---|---|---|
| 300.15 | 0.99655 +/- 0.00105 | 1.02789 | **+3135** | +30 |
| 600 | 0.95405 +/- 0.00096 | 0.98867 | **+3463** | +36 |
| 900 | 0.91719 +/- 0.00113 | 0.95974 | **+4255** | +38 |
| 1200 | 0.88527 +/- 0.00105 | 0.93700 | **+5173** | +49 |
| 750 (interpolated, log law) | — | 0.97454 | — | — |

**Against the prediction:**

- **The residual is outside the predicted band** (-1000..+3000 pcm) at every state point: by 135 pcm at 300 K, rising to +5173 pcm at 1200 K. The sign was predicted; the size was under-predicted.
- **It grows with temperature.** The prediction did not expect that.
- **Temperature trend:** `k(300) - k(1200)` is 9089 pcm deterministic against 11 128 pcm Monte Carlo. That is **-18 %, inside the predicted 20 %**.

**Nothing was tuned.** The candidate causes, unmeasured:

- the P0 `D` (prediction item 1);
- the cavity as a diffusion region;
- whole-material condensation spectra.

The growth with temperature points at a temperature-dependent spectral effect in the condensation, e.g. the bed's 4-group constants weighted by a whole-material spectrum that hardens with `T` differently from the bed's own. This was not tested.

### Group structure and ablations at 300.15 K

| variant | k | residual [pcm] |
|---|---|---|
| default (4 groups) | 1.02789 | +3135 |
| 2 groups | 1.05323 | +5668 |
| 8 groups | 1.02284 | +2629 |
| without rebalance | 1.04837 | +5182 (the rebalance carries 2047 pcm) |
| without descending order | 1.02789 | +3135, **identical**; 77 outer iterations instead of 23 |
| without unit conversion (cm read as SI) | 0.00001 | the core is 100x too small and leaks everything |

- **The group structure is not converged:** 2, 4 and 8 groups differ by 3000 pcm. Quote the 8-group result as the best available here, not the 4-group default.
- **The descending-order trap does not move a steady eigenvalue.** Reversing every group axis consistently is a relabelling, which diffusion is invariant to. What it does change is `genfoam_xs`'s inverse velocity, which assumes descending order: that matters for a transient, not for `k`. The trap is real for kinetics only. The test pins that the ablation reverses the groups.

`results.csv` holds the run's rows.
