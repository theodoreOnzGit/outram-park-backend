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

## Results

Not yet run at the time of writing. The example's output is recorded below once run.
