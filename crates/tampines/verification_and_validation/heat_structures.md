# Heat structures coupled to HEM pipes (gh:#1077)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). The RELAP-7 Theory Manual §4.1
and §6.2–6.4 (cited, not quoted) describe the heat-conduction model,
PipeWithHeatStructure, CoreChannel and HeatExchanger. This is a clean-room
equivalent.

## What it is

The code is `tampines::components::heat_structure`.

- **`HeatStructure`:** transient 1-D conduction across a cylinder (rods,
  `multiplicity` per axial cell) or a slab, at every axial cell.
  - Zones each have their own `k`, `ρc` and volumetric source.
  - The method is finite volume across the thickness, with backward Euler and
    one tridiagonal solve per axial cell, reusing
    `multiphase_1d::thomas_solve`.
  - Boundaries: adiabatic, temperature, convective, or prescribed heat inflow.
- **`ThermalCoupler` + `HeatStructureLink`:** each side of a structure is
  either a boundary or a network pipe with a coefficient `h`. Each step:
  1. The heat to the fluid, `(T_c − T_f)/(R_half + 1/(hA))`, goes into the
     pipe cells (`HemPipe::set_cell_heat_input`).
  2. The network steps.
  3. The structure advances the same `dt` with that same heat leaving it.

  Energy is therefore conserved exactly. The three assemblies are built from
  this:
  - **PipeWithHeatStructure:** one side on a pipe.
  - **CoreChannel:** a fuel rod with a source, its outer side on the coolant.
  - **HeatExchanger:** a wall with each side on a pipe.

Searched first:
- TUAS's `SolidColumn` is radially lumped.
- `bedok`'s transient rod solver is a MATLAB port with its own types, and
  `tampines` does not depend on `bedok`.
- OFFBEAT has no thermal sub-solver.
- MATPRO properties for a real rod can be tabulated from
  `outram-park-fork-offbeat`'s `Relap7HeatStructure` (gh:#1070).

## Predictions (written before the first run, 2026-11-01)

1. **Slab transient:** at Fourier number 0.1, the centre is within 0.5 K of
   the Fourier series for a 100 K step.
2. **Fuel rod at steady state:** the centre and cladding temperatures are
   within 0.5 % of their rise from the series-resistance hand solution.
3. **Coupled closed channel:** `ΔE = P t` to 1e-9.
4. **Heated channel at steady state:** the energy gain equals the wall power
   within 1 %.
5. **Counter-flow wall exchanger:** hot and cold balance within 1 %, and ε is
   within 5 % of ε-NTU.

## Results (`tests/heat_structure.rs`, release, 2026-11-01)

| # | Case | Measured | Prediction |
|---|---|---|---|
| 1 | 2 cm slab, k 15, ρc 4e6, 40 cells, Δt 0.01 s | centre +0.021 K from the series; time error halves with Δt (ratio 2.141) | held |
| 2 | fuel 4.1 mm / gap 0.08 mm / cladding 0.57 mm, q''' 3e8 | centre −0.134 % of rise; first cladding cell 0.000 % | held |
| 3 | 10 rods, 20 kW, closed 7 MPa water pipe, 300 steps | ΔE = P t to 4.8e-11 | held |
| 4 | air channel, 2 kW slab wall, 1 s | gain 1999.95 W (−0.003 %) | held |
| 5 | counter-flow air through a slab wall, 1.5 s | q hot = q cold (0.000 %); ε 0.20093 against 0.20189 (−0.48 %) | held |

**Instrument corrected in case 1.** The first runs rounded the step count, so
they ended at different times. That swamped the time error: the ratio read
0.36, and I first misread it as spatial and temporal errors cancelling. Each
run now ends at exactly `t_end`. Cases 4 and 5 use a deliberately small wall
`ρc` (4e3 J/(m³ K)) so the wall settles in about 0.1 s. That is a
verification choice, not a material.

## Limits and what is not done

- Properties are constant per zone; `k(T)` is not yet supported.
- There is no gap radiation and no axial conduction in the structure.
- `h` is a constant per link. Wiring the RELAP5 dispatcher (gh:#1063) needs
  fluid conductivity and heat capacity, which the HEM state does not carry.
- The coupling is explicit: the heat is frozen over the fluid's acoustic step.
- **Not done:**
  - the comparison of a steam generator against the existing helical-coil
    model;
  - a two-phase secondary side through the drift-flux and two-fluid solvers,
    which needs #1062's remaining phasic wall heat.
- **Geometry images:** none were drawn. The mesh is a 1-D radial (or
  thickness) grid with no CSG, lattice or multi-D mesh. The radii are stated
  in the tests.
