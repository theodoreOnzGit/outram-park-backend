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

## Results

To be filled in from the run (the example writes `history.csv` and `images/history.png`).
