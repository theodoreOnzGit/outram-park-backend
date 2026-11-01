# HEM pipe networks: flow junctions and volume branches (gh:#1071)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). The RELAP-7 Theory Manual (§6.5;
cited, not quoted) describes:

- **FlowJunction:** a zero-volume junction with Lagrange-multiplier
  constraints;
- **VolumeBranch:** a 0-D volume with mass and energy storage (Eqs 346–348).

This is a clean-room finite-volume equivalent. Code:
`src/hem/network.rs` (`HemNetwork`, `HemJunction`, `VolumeBranch`,
`PipeEndRef`), plus two additions to `HemPipe`: `HemBoundary::Flux` and
`HemPipe::step_by`. Re-exported as `tampines::components::junction`. Tests:
`tests/hem_network.rs`.

## Method

Each step:

1. One common `dt` from the Courant target over all pipes and volumes.
2. At every junction face, a KNP flux between the pipe-end cell and the state
   across the junction: the other pipe's end cell, or the volume at rest.
   - **Form loss** `Δp = K·½ρu²` enters as RELAP5 puts it. The downstream
     pressure is raised by `Δp` in the flux, and the downstream pipe receives
     the flux with its momentum lowered by `Δp`.
   - **Area change** at a flow junction: the flux passes through the smaller
     end area; each side gets the same mass and energy rate.
3. The flux is applied as a `HemBoundary::Flux`, held for the step, and every
   pipe advances `dt`.
4. Each volume integrates the rates its ends carried.

**Properties of the method:**

- **Mass and total energy are conserved by construction.**
- A flow junction joins exactly two ends; an n-way meeting is a volume branch.
- The junction face is first order and frozen over the Runge–Kutta stages.

## Predictions (written before the first run, 2026-11-01)

1. A closed network conserves mass and energy to < 1e-12 relative.
2. Sod across a `K = 0` equal-area junction differs from one continuous pipe
   by < 3 % of the pressure jump at worst, < 0.5 % in L1.
3. At steady low-Mach flow, the pressure step across a `K = 2` junction is
   `K·½ρu²` within 10 %.
4. A symmetric tee through a volume splits equally (to round-off) and
   balances mass within 0.5 % at steady state.

## Results (2026-11-01, release build, ideal-gas air)

| # | Case | Measured | Prediction |
|---|---|---|---|
| 1 | 3 pipes, a K = 0.5 flow junction and a 0.05 m³ volume, walls, 5 ms (287 steps) | mass −7.8e-16, energy 4.4e-16 | held |
| 2 | Sod, diaphragm 0.8 m, 2 m pipe against two 1 m pipes, 2 ms | worst 1.88 % of the jump, L1 0.053 % | held |
| 3 | Two 1 m pipes, K = 2, reservoir 1.02e5 Pa → 1e5 Pa, 200 ms | step 1313 Pa against 1370 Pa (−4.1 %), u = 34.1 m/s | held |
| 4 | Tee through a 0.01 m³ volume, 1.05e5 → 1e5 Pa, 2 s | B = C = 4.811879 kg/s (bit-identical); (B + C)/A − 1 = −0.15 % | held |

## Corrections made during the work

Each correction is also recorded on its test.

- **Form-loss formulation (case 3).** The first version added `Δp` to the
  upstream momentum flux after the KNP flux. The KNP flux averages the two
  pressures, so only part of the loss was carried: the flow ran at 32.0 m/s
  against the 33.9 m/s that `p₀ − p_e = (1 + K)·½ρu²` requires. It was
  replaced by the RELAP5 formulation above, which gives `Δp` exactly at steady
  state.
- **Instrument (case 3).** The first measure extrapolated the two end cells to
  the junction and read 288 Pa. Those cells hold a thin transition layer at
  the first-order face: A's last cell is 101 037 Pa against 101 293 next to
  it. The pipes themselves, uniform at steady state since they are
  frictionless with constant area, differ by 1313 Pa. The measure is now the
  mid-pipe pressures.
- **Protocol (case 4).** At 200 ms the tee was not steady. The inviscid pipes
  and the volume slosh: A's flow was 13.37, 11.71, 10.54, 9.57 and 9.64 kg/s
  at 0.05, 0.2, 0.5, 1 and 2 s. The settle time was lengthened to 2 s; the
  0.5 % criterion was not changed.

## Limits

- A zero-volume junction joins two ends only. RELAP-7's n-way Lagrange
  FlowJunction is represented here by a volume branch.
- A volume's kinetic energy is neglected.
- The junction is first order in space and frozen over each step, so it is
  visibly not transparent (case 2).
