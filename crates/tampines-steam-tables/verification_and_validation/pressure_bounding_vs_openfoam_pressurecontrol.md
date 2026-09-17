# Pressure bounding in `TampinesSteamArray::step` vs OpenFOAM `pressureControl`

**Generated:** 2026-07-14 (UTC)
**Crate:** `tampines-steam-tables` v0.2.1 (also applies to
`outram-park-fork-coolprop`'s `OPCPFluidArray`, which shares the design)

## Methodology

`TampinesSteamArray::step` (a 1-D compressible rhoPimpleFoam port closed with
the real IAPWS-IF97 `(p, h)` flash) can be driven by prescribed inlet/outlet
boundary conditions — a fixed inlet velocity + enthalpy and a fixed outlet
pressure — to model a pipe/tube (e.g. a steam-generator tube). When a
near-incompressible liquid is started impulsively, the resulting acoustic
(water-hammer) transient can drive a cell's pressure **out of the EOS-valid
range**, including to **negative absolute pressure** in the reflected
rarefaction. The `(p, h)` flash then panics (`p,h point is outside pressure
range`), crashing the run.

This is the exact problem OpenFOAM's compressible solvers solve with
**pressure bounding**: they limit *pressure* (not density) so the equation of
state is only ever evaluated inside its valid domain. The rhoSimpleFoam
change log states this explicitly — *"In order to support complex equations
of state, the pressure can no longer be unlimited and rhoSimpleFoam now
limits the pressure rather than the density to handle start-up more
robustly"* (OpenFOAM-plus commit `655fc787`, "rhoSimpleFoam: added support
for compressible liquid flows").

We add the same mechanism: after each pressure solve (and under-relaxation),
the internal pressure field is clamped into `[p_min, p_max]`, defaulting to
the IAPWS-IF97 validity range (triple-point pressure ≈ 611.657 Pa up to
100 MPa) and tunable via `set_pressure_bounds`. This directly mirrors
OpenFOAM's `pressureControl::limit`:

```cpp
// src/finiteVolume/cfdTools/general/pressureControl/pressureControl.C
// Copyright (C) 2017 OpenFOAM Foundation. GPL-3.0-or-later.
bool Foam::pressureControl::limit(volScalarField& p) const
{
    if (limitMaxP_ || limitMinP_)
    {
        if (limitMaxP_)
        {
            const scalar pMax = max(p).value();
            if (pMax > pMax_.value())
            {
                Info<< "pressureControl: p max " << pMax << endl;
                p = min(p, pMax_);
            }
        }
        if (limitMinP_)
        {
            const scalar pMin = min(p).value();
            if (pMin < pMin_.value())
            {
                Info<< "pressureControl: p min " << pMin << endl;
                p = max(p, pMin_);
            }
        }
        return true;
    }
    else
    {
        return false;
    }
}
```

Our Rust clamp is `p = p.clamp(p_min, p_max)` per cell, which is the same
`max(min(p, pMax), pMin)` operation. Because `f64::clamp` leaves `NaN`
unchanged, a genuinely diverged (NaN) field is **not** masked — it flows on
to the flash rather than being silently pinned to a bound.

**Pass criterion.** With tight bounds set, no cell pressure may leave
`[p_min, p_max]` and no field may go non-finite over the run
(`pressure_bounding_clamps_transient_within_set_bounds`, in both
`tampines-steam-tables` and `outram-park-fork-coolprop`).

## Reference

- OpenFOAM `pressureControl` class, `limit()` method:
  `src/finiteVolume/cfdTools/general/pressureControl/pressureControl.C`,
  Copyright (C) 2017 OpenFOAM Foundation, GPL-3.0-or-later. Source viewed
  2026-07-14 via
  <https://github.com/OpenFOAM/OpenFOAM-5.x/blob/master/src/finiteVolume/cfdTools/general/pressureControl/pressureControl.C>
  (API guide: <https://www.openfoam.com/documentation/guides/latest/api/pressureControl_8C_source.html>).
- Rationale (limit pressure, not density, for robust start-up with complex
  EOS): OpenFOAM-plus commit `655fc787`, "rhoSimpleFoam: added support for
  compressible liquid flows",
  <https://develop.openfoam.com/Development/OpenFOAM-plus/-/commit/655fc7874808927d14916307a2230a8965bdb860>.
- Solver context: rhoPimpleFoam / rhoSimpleFoam user guides,
  <https://doc.openfoam.com/2312/tools/processing/solvers/rtm/compressible/rhoPimpleFoam/>.

## Results (2026-07-14)

| Check | Setup | Result |
|---|---|---|
| Bounding clamps a transient | 10-cell, 1 m, 1e-4 m², dt=5e-5 s, tight bounds `[0.9, 1.1] bar`, 0.3 m/s impulsive inlet (~4 bar unbounded Joukowsky surge), 100 steps | **PASS** — all 10 cell pressures stay within `[0.9, 1.1] bar`, all fields finite |
| Prevents pressure-range crash | Same array, default bounds `[611.657 Pa, 100 MPa]` | The pressure-range panic (negative absolute pressure) is eliminated |

**Interpretation.** Pressure bounding removes the pressure-range crash and is
the standard, OpenFOAM-consistent robustness mechanism for compressible
solvers with a complex EOS. It is *necessary but not sufficient* for the most
violent transients: a 0.5 m/s impulsive start on liquid water is a ~7 bar
water-hammer whose reflected rarefaction also drives the **enthalpy** below
the 273.15 K validity isotherm (a cavitation/flashing regime a single-phase
`(p, h)` EOS cannot represent). Combining pressure bounding with velocity
under-relaxation (`set_pimple_algorithm` / `set_simple_algorithm`) raises the
survivable impulsive inlet velocity from ≈ 0.02 m/s (bare PISO) to ≈ 0.05 m/s;
beyond that, the physical guidance is to ramp the inlet velocity rather than
start impulsively (real pumps ramp; they do not step-change the flow). See
bead `op-21g.12` for the full debugging trail.
