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
| 1 | 3 pipes, a K = 0.5 flow junction and a 0.05 m³ volume, walls, 5 ms (~~287~~ **551** steps, re-measured 2026-11-01 after the port correction below) | mass ~~−7.8e-16, energy 4.4e-16~~ **−4.4e-16, energy 2.2e-16** | held |
| 2 | Sod, diaphragm 0.8 m, 2 m pipe against two 1 m pipes, 2 ms | worst 1.88 % of the jump, L1 0.053 % | held |
| 3 | Two 1 m pipes, K = 2, reservoir 1.02e5 Pa → 1e5 Pa, 200 ms | step 1313 Pa against 1370 Pa (−4.1 %), u = 34.1 m/s | held |
| 4 | Tee through a 0.01 m³ volume, 1.05e5 → 1e5 Pa, 2 s | ~~B = C = 4.811879 kg/s (bit-identical); (B + C)/A − 1 = −0.15 %~~ **CORRECTED 2026-11-01:** A 97.1574, B = C = 48.578690 kg/s (bit-identical); (B + C)/A − 1 = +0.0000 %; hand Bernoulli estimate 96.4 kg/s, run +0.8 % | held |

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

- **Volume ports (CORRECTED 2026-11-01, gh:#1073).** Each volume port was a
  KNP flux between the end cell and the volume at rest. At steady flow that
  forces `ρ_L − ρ_R = ρu/a`, which is an artificial port loss `Δp ≈ ρcu`. In
  case 4 the 5 kPa drive went mostly into it, and the flows were **ten times
  too low** (4.81 against 48.58 kg/s per branch). Case 4 never checked a flow
  rate against a hand value, so the defect went unseen; the separator work
  exposed it. Every volume port is now a characteristic boundary of its pipe,
  like `HemBoundary::Reservoir`:
  - the face velocity follows the pipe's outgoing acoustic invariant;
  - outflow from the volume is a stagnation reservoir,
    `p_port = p_f + (1 + K)·½ρu²`;
  - inflow is a sink, `p_f = p_port + K·½ρu²`, so the inflow's kinetic energy
    is lost in the volume (Borda);
  - donor-cell transport.

  Case 4 now has a hand check (written after the run, so not a prediction).
  Methods step 2 below describes the old KNP flux for volume ports and is
  superseded for them.

## Limits

- A zero-volume junction joins two ends only. RELAP-7's n-way Lagrange
  FlowJunction is represented here by a volume branch.
- A volume's kinetic energy is neglected.
- The junction is first order in space and frozen over each step, so it is
  visibly not transparent (case 2).

## Time-dependent boundaries (gh:#1076, network part)

`TimeTable` holds piecewise-linear, clamped tables. A
`TimeDependentBoundary` drives a pipe end each step:

- **`Volume`:** a stagnation reservoir at `p(t)` with `T(t)` or `h(t)`;
- **`Junction`:** `ṁ(t)` entering at `T(t)` and the end cell's pressure.

Tests: `tests/hem_time_dependent.rs`, 2026-11-01. Predictions written before
the run; all held.

| Case | Measured |
|---|---|
| Constant TDV against the static `Reservoir` | bit-identical, every cell |
| TDJ ramp 0 → 0.2 kg/s over 2 ms, 5 ms run | mass gained = Σ ṁ(t_k)·dt_k to 1e-12; −0.35 % from the exact integral (start-of-step sampling) |

The two-fluid pressure boundary and inflow state in `multiphase_1d` are not
done here.

## Pump, valve and turbine (gh:#1072)

New `HemJunction` variants, built on the same two-end flux as the flow
junction:

- **`Pump`** (RELAP-7 §6.6).
  - The head comes from a table, or from shaft power by Eq 355, capped at a
    shut-off head.
  - `ρ g H` raises `a`'s pressure in the junction flux. The pump work `ṁ Δp/ρ`
    enters `b` only.
  - Reverse flow is a `K1 + K2` resistance.
- **`Valve`:** area `opening·min(A)`, loss `K/opening²`; both ends are walls
  at opening ≤ 0.
- **`Turbine`** (§6.7).
  - Mass flow from the characteristic, `tanh(β(p01/p02 − 1))`, with `β` from
    the design point (Eq 362).
  - Shaft power `ṁ (h01 − h02)` with `η`.
  - The isentropic drop uses the manual's ideal-gas form with a user `γ`.
  - Closed when `p01 ≤ p02`.

Tests: `tests/hem_components.rs`, 2026-11-01. Predictions written before the
run.

| Case | Measured | Prediction |
|---|---|---|
| Pump H = 50 m, air, 1 s | rise +0.91 % of `ρ g H`; power = `ṁ g H` | held |
| Pump from shaft power W = ṁ g H of that run | rise within 0.00 % of the head run | held |
| Valve fully open, K = 2, against a flow junction with K = 2 | bit-identical | held |
| Valve closed between 2e5 and 1e5 Pa pipes | each pipe's mass conserved to 1e-13 | held |
| Turbine 2e5 → 1e5 Pa, ṁ_max 3, ratio 2, flow ratio 0.8, η 0.85 | junction 2.0191 kg/s, both pipes 2.0191; shaft power 75.9 kW; closed when reversed | held |

### Corrections, each recorded on its test

- **The shaft-power pump never started.** The first version gave zero head
  until flow existed: Eq 355 is singular at `ṁ = 0`. It now caps the head at a
  shut-off head.
- **The pump runs were not steady at 0.3 s.** The head run read −1.83 % and
  the shaft-power run +1.44 % of it; both were rerun to 1 s.

### Limits

- The turbine's stagnation pressure is `p + ½ρu²` (low Mach), and its
  isentropic drop is the ideal-gas form with a user exponent, not an EOS
  entropy.
- No homologous pump curves.

## Relief valve (gh:#1074)

`HemJunction::ReliefValve` (RELAP-7 §6.11): opens when the protected end cell
reaches `p_set`, closes when it falls to `p_reseat`, and when open passes the
EOS break flow (`HemEquationOfState::break_flow`, choked or not) through
`area_fraction·min(A)` at the end cell's stagnation enthalpy. Closed, both ends
are walls. `relief_valve_state` reports open and choked.

Test: `tests/hem_relief_valve.rs`, 2026-11-01, IF97 water. Predictions written
before the run.

| Case | Measured | Prediction |
|---|---|---|
| 7.5 MPa / 500 K pipe, set 7.0, reseat 6.5 MPa, into 0.5 MPa / 400 K, fraction 0.1, 2000 steps | opens on step 1, choked, 90.7 kg/s; recloses at 0.040 ms (end cell 5.95 MPa) | held |
| Same: stays closed after reseating | **missed**: 12 openings, 12 closings — water hammer from the abrupt closure lifts the end cell back over the set pressure (chatter) | missed, physical |
| Same: hysteresis thresholds, closed-step mass, total mass | every transition on the right side of its threshold; 4.4e-16; 4.4e-16 | held |

Limits: the valve opens and closes instantly (no stroke time), which is what
drives the chatter; the break flow uses the end cell's static pressure, not a
stagnation pressure.

## Separator-dryer and downcomer (gh:#1073)

`HemNetwork::add_separated_volume` creates a vertical volume (`V`, `A`, height
`V/A`) in which each port draws as its `VolumePort` says. It is based on
RELAP-7 §6.8–6.9 (cited, not quoted).

- **`Mixture`:** draws the mixture.
- **`Vapour` / `Liquid`:** saturated phases, from the new
  `HemEquationOfState::saturated_phases`. IF97 supplies them below 16.529 MPa.
  This is the ideal separator.
- **`AtHeight(z)`:** liquid at `p + ρ_f g (L − z)` while the collapsed level
  `L = M(1 − x)/(ρ_f A)` is above `z`, and vapour once it is not. This is the
  downcomer's uncovered-port switch.

`volume_level` and `port_draw` report the level and what each port draws.
Mass and energy are stored and conserved as in a volume branch. Every port is
a characteristic boundary (see the volume-port correction above).

Searched first:
- `outram-park-fork-dwsim-libs/src/separator` is a steady flash drum with no
  dynamics.
- sembawang names separators only in its parameters, and its reactor-vessel
  level is lumped.
- Nothing dynamic existed.

Tests: `tests/hem_separator_downcomer.rs`, 2026-11-01, IF97 water.
Predictions were written before the first run.

| Case | Measured | Prediction |
|---|---|---|
| Closed separator network, 7 MPa, x = 0.2, 200 steps | mass 0, energy 2.2e-16 | held |
| Ideal separator, inlet 7.0 MPa x = 0.2, steam to 6.99 MPa, bottom liquid port to 6.99 MPa + ρ_f g·0.35 m, K = 20, 1 s | split 0.2001 (+0.05 %); mass −9.0e-4, energy −6.7e-4; steam stream h_g to +0.003 %; level 0.313 m | held |
| Downcomer, 1 MPa x = 0.001 (level 0.853 m), drained to 0.98 MPa, port at 0.5 m | switched to vapour on the first step at L = 0.49998 m, matched L every step; closure −3.4e-14 / −2.7e-14 | held |

### Corrections during the work

- **A KNP port drained the volume.** KNP's dissipation across a drawn phase
  and a pipe mixture moved mass at rest. A liquid port emptied a 0.3 kg volume
  in four steps.
- **A donor-cell energy flux alone** stopped the energy runaway but not the
  mass drain.
- **A linearised acoustic flux** charged steady outflow a `ρcu` loss:
  3.9 kPa at 0.22 m/s of steam.
- **The fix:** a characteristic boundary. Checking it at the Mixture inlet
  exposed the same artificial loss in the #1071 volume branch, now corrected.
- **The shared `(ρ, e)` inversion** could not find a valid 7 MPa, x = 0.6
  state without a pressure hint. Its doubling probes jumped from 6.7 to
  100 MPa, which the flash rejects. A log-grid fallback, reached only on
  failure, fixes it and has its own regression test.
- **Steady-state protocol:** see the test's doc comment. The criteria were not
  changed.

### Limits

- The level is the collapsed level. There is no swell or carry-under, and
  separation is ideal.
- A single-phase volume draws the mixture at every port, with no hydrostatic
  head.
- **Not compared with sembawang:** its reactor-vessel level is a lumped
  collapsed level with the same definition, but it is not a separable
  function to drive with the same input.
- **Found and not fixed here:** a `FixedPressure` outlet with saturated liquid
  flashing to 0.5 MPa drives the last cell toward 100 MPa (filed separately).
