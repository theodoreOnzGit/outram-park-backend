# Steam-generator tube: `TampinesSteamArray` integration (fhr_sim_v2)

*How `fhr_sim_v2`'s secondary loop was moved from a lumped energy balance to
the spatially-resolved `TampinesSteamArray` steam-table physics, and the
debugging journey behind it. Written 2026-07-14.*

## What changed

`app/thermal_hydraulics_backend/secondary_loop/mod.rs` used to model the
steam-generator tube with a single lumped energy balance:

```text
h_out = h_in + Q_dot / m_dot          // instant "steady state", one flash
```

It now drives a **persistent** `TampinesSteamArray` (a 1-D compressible
rhoPimpleFoam tube closed with the real IAPWS-IF97 `(p, h)` flash):

- `build_steam_generator_tube()` creates it once (15 cells, 2 m, aggregate
  bundle area, PIMPLE + under-relaxation + default pressure bounding),
  pre-initialised as subcooled feedwater already flowing at the nominal
  velocity and **at the operating pressure**.
- Each thermal-hydraulics step, `secondary_loop_single_timestep` sets the
  inlet velocity (from `m_dot / (ρ_feed · A)`), the inlet enthalpy (pump
  outlet), and the outlet pressure; registers the primary-side heat as a
  distributed volumetric source; advances the array a **bounded** number of
  acoustic-CFL substeps; and reads the outlet enthalpy back with
  `get_outlet_enthalpy()` to feed the (unchanged) isentropic turbine.

The array is a **quasi-steady sub-model**: it is *not* re-converged each call
(that would need thousands of acoustic-CFL steps — the timestep is set by the
liquid sound speed, ~1e-4 s). It is nudged a little each TH step and relaxes
toward the current boundary conditions over real time, as a physical steam
generator takes seconds to heat up.

Validated standalone by
`tampines_steam_tables … steam_generator_tube_boils_feedwater`: feedwater at
80 °C heats through the 2-bar saturation dome into the two-phase region
(quality crosses 0) without panicking.

## The debugging journey (read this before touching it)

Getting the live coupling stable took four fixes; all are cheap to re-break.

1. **The pressure-source clobbering bug** (upstream, in `step()` itself). A
   fixed-pressure outlet blew up even from equilibrium because the pressure
   equation overwrote, rather than added to, `fvm::laplacian`'s Dirichlet
   boundary source — dropping `coeff·p_bc` while keeping its diagonal, i.e.
   silently imposing `p_outlet = 0`. See the full teacher's walkthrough:
   `outram-foam-appbuilder-lib/src/solvers/rho_pimple_foam/docs/stability_a_students_guide.md`.

2. **Use `(p, h)` flashing, not `(T, p)` single-phase.** The array's state is
   `(p, he)`; the `(p, h)` flashes carry phase/quality data and stay defined
   across the saturation dome, whereas `(T, p)` single-phase flashes
   `panic!`/`todo!()` on any two-phase state. Drive with `set_inlet_enthalpy`,
   read with `get_outlet_enthalpy`. (`set_temperature_vector` is a `(T, p)`
   convenience for a *known subcooled* initial condition only.)

3. **Pressure floor on the saturation line.** The default pressure-bounding
   floor was `sat_pressure_4(273.15 K)` *exactly*; a clamped cell landed on
   the saturation line, where the `(p, h)` validity guard classifies its
   273.15 K isotherm with a `(T, p)` flash whose Region-4 test is exact float
   equality (`pres == p_sat`) → `todo!()` panic. Fixed by nudging the default
   floor `* 1.001` into Region 1.

4. **Initial-vs-operating pressure mismatch.** Pre-initialising the tube at
   2 bar while the runtime outlet BC was 1.2 bar caused a depressurisation
   rarefaction on the first driven step that cooled a cell below the
   273.15 K floor → panic. Fixed by pre-initialising at the operating
   pressure.

## Known limits / follow-ups

- The array **panics on out-of-range `(p, h)`** by design (it does not
  silently degrade — a deliberate choice). So it must be kept inside its
  envelope: drive stiff liquids *gently* (this is why the tube uses a large
  aggregate area → low velocity, heavy under-relaxation, and a matched
  pre-init). Driving it far outside the envelope (e.g. a violent user
  slider change) can still crash the TH thread.
- An **enthalpy-bounding** analogue to the existing pressure bounding would
  make it robust to sub-freezing/superheat transients the way `pMin`/`pMax`
  handles pressure — a natural next addition.
- Full provenance and the OpenFOAM `pressureControl` reference for the
  pressure bounding:
  `tampines-steam-tables/verification_and_validation/pressure_bounding_vs_openfoam_pressurecontrol.md`;
  the flashing-choice correction log:
  `tampines-steam-tables/docs/notes.md`; the debugging trail: bead
  `op-21g.12`.
