# CoolProp HEM closure, compressed-liquid water, after the `density_pt` fix (gh:#1349, 2026-11-13)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft, unreviewed.

## Why this record exists

`tampines/tests/hem_coolprop.rs::coolprop_refuses_compressed_liquid_water_instead_of_returning_a_wrong_root`
pinned a **recorded defect**: `outram_park_fork_coolprop::flash::density_pt` returned the vapour
root for liquids, so `CoolPropHem::state_p_t(2 MPa, 350 K)` was an `EquationOfState` error.
gh:#1349 fixed `density_pt` (water, 350 K, 2 MPa: 974.573279182657 against CoolProp
974.573279182652, 5e-15; see `crates/outram-park-fork-coolprop/verification_and_validation/coolprop_differential/RESULTS.md`).
The refusal test therefore fails, correctly, and is replaced by a test of the corrected behaviour.
No tolerance is relaxed: the old test asserted that a known defect was present.

## Predictions, written BEFORE any run

What I read first (source, not a run): `CoolPropHem::{state_p_t, close_ph}` (`state_pt` then `state_p_h`;
the guard rejects a single-phase root with `rho_vapour < rho < rho_liquid` at its own T), the steam
shock-tube sibling in the same test file, and RESULTS.md defect 1 of the CoolProp differential.
The published IAPWS R7-97 table of permissible IF97-vs-IAPWS-95 differences could NOT be re-read
here (the `reactor-literature` submodule is empty in this checkout and iapws.org is unreachable
through the proxy). The bounds below are therefore my recollection of the Region 1 consistency
(density of order 3e-5 relative, enthalpy of order 0.2 kJ/kg) widened by a factor of a few, **marked
Not re-checked against the table**, and they are registered before the run so they can fail.

| # | claim | predicted outcome |
|---|---|---|
| L1 | `state_p_t(2 MPa, 350 K)`, water, CoolProp | `Ok`, single phase (`quality == None`), `rho` = 974.573 kg/m3 to 1e-9 relative (CoolProp reference value in RESULTS.md), T = 350 K |
| L2 | same state against `Iapws97Hem` | `abs(rho_CP/rho_IF97 - 1) <= 1e-4`; `abs(h_CP - h_IF97) <= 0.5 kJ/kg`; expected actual ~3e-5 and ~0.2 kJ/kg |
| L3 | `state_p_t(1.3 MPa, 455 K)`, water | `Ok`, single phase, liquid (rho about 900 kg/m3, above `rho_liquid`-side of the dome, i.e. not 6.84 kg/m3), IF97 agreement as L2 |
| L4 | `HemPipe::new` with `HemEos::CoolProp(Water)` at L1 | `Ok` |
| L5 | liquid shock tube: 1 m, 40 cells, walls, water 350 K, 2 MPa left and 1 MPa right (liquid both sides: p_sat(350 K) = 41.7 kPa), `t = 0.2 ms` (waves travel about 0.3 m), same solver closed by IF97 and by CoolProp | both run to `t`, step counts within 10 % of each other; star pressure about 1.5 MPa; `max_cells abs(p_IF97 - p_CP) <= 1 % of the 1 MPa jump` (10 kPa), expected ~0.1 %, because the liquid sound speed of the two formulations differs by a few 1e-4 |
| L6 | cost | the CoolProp run is slower than IF97 by more than 10x (steam: 28 s against 0.12 s); the liquid run is no slower than the steam run now that the liquid flash no longer falls into a 400-step bisection |

Sign/size reasoning for L5: the acoustic star pressure is `p_star = (pL + pR)/2 + O(Mach)`, and the jump
is carried by `rho c du`; a 3e-4 difference in `c` moves the plateau by less than 3e-4 of the jump, so
1 % has a factor of order 30 of margin, which is deliberate: the gate is a sanity bound for two
different water formulations (as the steam sibling's 5 % is), not the expected value.

If any of L1-L5 misses, the finding is the miss: investigate `density_pt`/`state_ph` and the
closure, do not move the bound.
