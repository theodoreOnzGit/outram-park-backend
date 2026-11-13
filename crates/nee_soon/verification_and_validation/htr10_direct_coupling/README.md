# HTR-10: whole-core Monte Carlo against the R-Z thermal-hydraulics (gh:#1186)

Module: `nee_soon::htr10_coupling`. Example: `examples/htr10_direct_coupling.rs`. Status: untrusted AI-generated draft, 2026-11-03. For research, education and V&V only; not validated, and not a benchmark comparison.

## Methodology

- **Monte Carlo model:** `ParametricStudy`, the web demo's.
  - Şeker bed N = 17 (172.6 cm, the nearest to the R-Z model's 180 cm), rods withdrawn, LEAPR graphite and SiC laws.
  - 1000 particles x (10 inactive + 20 active) generations per iteration: a cost-limited run, `sigma_k` about 0.003.
- **TH target:** `htr10_rz` `Htr10RzTransientCore` steady state at the FPIC specification (10 MW, 250 °C, 3.0 MPa, 4.32 kg/s), 20 cm mesh, LOFC base options with the FPIC shape.
- **One Picard iteration** (`htr10_coupling` module docs):
  1. The Monte Carlo runs with an R-Z `KappaFission` tally: 9 rings x about 17 slices over the cylindrical bed.
  2. The tally becomes an R-Z table.
  3. The table is put on the 1124 heated bed cells, with 10 MW conserved to round-off.
  4. `set_power_map` (gh:#1185) passes it to the R-Z core, and the steady state is solved.
  5. The zone temperatures come back:
     - fuel and moderator: power-weighted;
     - reflectors, bricks and rods: volume-weighted;
     - coolant: the mean of inlet and outlet.
  6. They are under-relaxed (omega = 0.7) and rounded to 1 K.
  7. The nuclear data are reprocessed exactly at them, with every (nuclide, T) job cached.
- **Start:** fuel 900 K, moderator 850 K, coolant 700 K, top/side/bottom reflector 600/650/700 K, rods 600 K, bricks 550 K.
- **Stop:** max zone `|T_th - T_in| < 10 K` and `|dk| < 0.005`, or 4 iterations.
- **Unit tests** (`cargo test --release -p nee_soon --lib htr10_coupling`, 6 pass):
  - relaxation, rounding and the range clamp;
  - the region-to-zone map;
  - the R-Z target receives 10 MW to 1e-10 and returns ordered zone temperatures;
  - the lumped node reaches `T_sink + P/G`;
  - the default power mesh and the refusals;
  - the history picture.

## Prediction (written 2026-11-03, before the run)

- **Converged fuel (power-weighted) temperature:** 850-1050 K. The R-Z FPIC solid model's fuel at 10 MW, with a Monte Carlo shape more peaked than Tab 2.6's at the top of the bed.
- **Converged moderator temperature:** 30-80 K below the fuel.
- **Hot `k`:** 7000-10 000 pcm below the same model at 300 K. The Monte Carlo records give 11 128 pcm for 300 to 1200 K isothermal at N = 12, and here only the bed is hot.
- **Iterations:** about 3 to converge in temperature at omega = 0.7. `k` may not meet 0.005 if `sigma_k` is near 0.003.

## Results (2026-11-04)

Run on a cloud container (4 vCPU, rustc 1.99.1, release) with `cargo run --release -p nee_soon --example htr10_direct_coupling` and the defaults above. The rows are in `history.csv`; the picture is `images/history.png`.

| it | k +/- sigma | fuel in -> TH [K] | moderator in -> TH | coolant in -> TH | side refl. in -> TH | max \|dT\| [K] | peaking | jobs run / cached | data [s] | MC [s] | TH [s] |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.02675 +/- 0.01028 | 900 -> 916.8 | 850 -> 901.4 | 700 -> 742.8 | 650 -> 567.4 | 82.6 | 1.73 | 64 / 0 | 371 | 220 | 4.2 |
| 2 | 1.01309 +/- 0.00847 | 912 -> 917.1 | 886 -> 901.5 | 730 -> 742.8 | 592 -> 567.1 | 24.9 | 1.89 | 68 / 0 | 368 | 227 | 2.0 |
| 3 | 1.00386 +/- 0.00892 | 916 -> 915.7 | 897 -> 900.3 | 739 -> 742.7 | 575 -> 570.6 | 4.8 | 2.02 | 68 / 0 | 379 | 235 | 2.8 |
| 4 | 1.02259 +/- 0.00681 | 916 -> 918.6 | 899 -> 903.0 | 742 -> 742.8 | 572 -> 568.0 | 4.0 | 2.13 | 53 / 15 | 108 | 230 | 2.2 |

**Outcome: not converged at the cap.**

- **Temperatures converged** from iteration 3: every zone within 5 K of its TH value.
- **`k` did not converge.** Iteration to iteration it moved by 0.0137, 0.0092 and 0.0187. That is the Monte Carlo noise: `sigma_k` is 0.007-0.010 at 20 000 active histories. The 0.005 criterion is below `sqrt(2) sigma`, and the example's own comment warned of exactly that.

**Cost per iteration:** about 10 minutes on 4 threads.

- **Processing data:** about 6 minutes per iteration (370 s), wherever any zone moved by 1 K.
  - U-235 took 278 s and U-238 214 s at a new temperature.
  - The cache saved 15 of 68 jobs only at iteration 4, when some zones repeated a temperature: 108 s.
- **Transport:** about 3.8 minutes (225 s) for 30 000 histories at N = 17.
- **The R-Z TH steady state:** 2-4 s.
- **Whole run:** 2148 s.

**Against the prediction:**

| | Prediction | Result |
|---|---|---|
| Fuel | 850-1050 K | **916-919 K, inside** |
| Moderator below the fuel | 30-80 K | **15-16 K, outside**: the dummy-pebble node at the 10 MW FPIC flow runs close to the fuel |
| Iterations to temperature convergence | about 3 | **3, as predicted** |
| Hot `k` against the same model at 300 K | 7000-10 000 pcm below | **Not measured**: no cold N = 17 run was made, so this cannot be checked |

**Power peaking** (peak over mean density on the TH cells) rose from 1.73 to 2.13 across the iterations. That is the maximum of a noisy R-Z tally (9 x 17 bins, 20 000 histories), not a physical trend: noise raises a maximum. A smoothed shape (`PowerShape::LegendreZ`, or Zernike to Fourier-Bessel) or more histories is the remedy; neither was run.

**Majorant violations:** 0 in every iteration.

**What would make this a result:**

- more histories, so that `sigma_k` falls below the `k` tolerance;
- a `k` tolerance at about `2 sigma`;
- the cold N = 17 reference for the temperature defect;
- the run on the Monte Carlo desktop.

None of this is validation: the R-Z model is TECDOC-1694's full-power core, and the Monte Carlo bed (N = 17) is not the as-run FPIC loading.
