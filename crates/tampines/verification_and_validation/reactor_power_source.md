# Reactor power source (gh:#1079)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). The RELAP-7 Theory Manual §6.16
(cited, not quoted) describes a reactor component that distributes core
power. This is a clean-room equivalent.

## What it does

`tampines::components::reactor_power::ReactorPowerSource`:

- **Total power** comes from a `TotalPower`: either a time table or a value
  the caller sets each step. ~~`tampines` stays free of `teh-o-prke`, so~~
  ~~**CORRECTED 2026-11-03 (gh:#1151):** `tampines` now depends on
  `teh-o-prke` (decision D2, for `htr10_primary_loop`), but this component
  takes no kinetics:~~ **CORRECTED again 2026-11-03: maintainer moved
  coupled kinetics to nee_soon** (`nee_soon::htr10_th_transient`), so
  `tampines` is free of `teh-o-prke` again and this component takes no
  kinetics: a
  `PointReactor`'s fission and decay power arrive through `External`.
- **Distribution over channels** is by a normalised radial weight.
- **Distribution within a channel** is by an `AxialPowerShape`:
  - uniform;
  - chopped cosine with `H_e = ratio·H`, integrated exactly over each cell;
  - a table.
- **Deposition:** `apply_to_hem_pipe` puts the power directly into the fluid
  through the new `HemPipe::set_cell_heat_input`. `HemNetwork::pipe_mut` lets
  the caller update it between steps.

Searched first:
- `tampines` has no power-distribution component.
- `dhoby-ghaut`'s `PowerShape` is a GeN-Foam case-setup enum, not a reusable
  distributor.
- `teh-o-prke` provides the total, not the shape.

## Predictions (written before the first run, 2026-11-01)

1. Every channel's cell powers sum to its share, and all of them to the total,
   to < 1e-14. The bare cosine's peak/mean tends to π/2.
2. A decay-heat transient in a closed HEM pipe raises the total energy by
   `Σ P(t_k)·dt_k` to 1e-9, within 1 % of the exact integral.

## Results (2026-11-01, `tests/reactor_power_source.rs`, release)

| Case | Measured | Prediction |
|---|---|---|
| Channel weights 1 : 2 : 0.5; uniform, cosine (ratio 1.2) and table shapes; 20 cells; 3 MW | every sum within 1e-14 | held |
| Bare cosine, 2000 cells | peak/mean 1.570795681 against π/2 = 1.570796327 (−4.1e-7, cell averaging) | held |
| Way–Wigner decay power, P0 = 10 kW, 1 day of operation, 1 s → 1.005 s after shutdown, closed air pipe | gained 2.792025 J = step sum to 1e-9; +0.001 % from the exact integral 2.792007 J | held |

## Limits

- Power is deposited in the coolant directly. A fuel heat structure between
  fuel and coolant, with its time constant, is #1077's work.
- The shape is fixed in time. A spatial-kinetics shape needs the
  `nee_soon`/`dhoby-ghaut` coupling (#1084).
