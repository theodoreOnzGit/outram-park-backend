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

## Results (measured 2026-11-13, `cargo test --release -p tampines --test hem_coolprop`, 1 build job, shared machine)

| # | predicted | measured | verdict |
|---|---|---|---|
| L1 | `Ok`, single phase, rho = 974.573 to 1e-9 | rho = 974.573279183 kg/m3 (CoolProp reference 974.573279182652), T = 350.000000 K | held |
| L2 | `Δρ/ρ ≤ 1e-4`, `Δh ≤ 0.5 kJ/kg`, expected ~3e-5, ~0.2 kJ/kg | 2 MPa / 350 K: IF97 974.585762, **Δρ/ρ = −1.281e-5, Δh = +59.5 J/kg**, c 1558.670 vs 1561.140 m/s (−0.16 %) | held, better than expected |
| L3 | `Ok`, liquid, ~900 kg/m3, IF97 as L2 | 1.3 MPa / 455 K: rho = 885.186131 (not 6.84), IF97 885.172864, **Δρ/ρ = +1.499e-5, Δh = −135.2 J/kg**, c 1387.353 vs 1386.909 m/s (+0.03 %) | held (my 900 estimate was 1.7 % high; the bound was on agreement, not on the number) |
| L4 | `HemPipe::new` `Ok` | `Ok` | held |
| L5 | star p ~1.5 MPa; max abs dp ≤ 1 % of jump, expected ~0.1 %; step counts within 10 % | 32 steps each; star p 1.499970 (IF97) vs 1.499951 MPa (CoolProp); **max abs dp = ~~1464.6 Pa~~ 1465.3 Pa = 0.1465 % of the 1 MPa jump** (2026-11-13, gh:#1346: now with the IF97 pipe ablated of wall friction, like for like; 1464.6 Pa was with friction) | held |
| L6 | CoolProp > 10x slower than IF97; liquid no slower than steam | liquid: IF97 0.17 s, CoolProp 83.6 s (about 490x). Steam in the same run: 78.95 s. The liquid run was about 6 % slower than steam, while the two ran concurrently on a shared machine | first half held; **second half missed by a small margin** (not investigated: concurrent timing on a shared box; the 400-step bisection is gone, the per-step cost is the `(rho, e)` Brent root over the Helmholtz `(p, h)` flash) |

Interpretation: the corrected `density_pt` makes the CoolProp HEM closure usable for compressed liquid, and the
two water formulations agree on the liquid to ~1.5e-5 in density and 0.15 % of the jump in a shock tube, in line
with IF97 Region 1 being a fit to IAPWS-95. The acoustic star pressure is the same to 2e-5 relative. The IF97 vs
IAPWS-95 consistency figures used for the L2 bound were not re-checked against the IAPWS R7-97 table (see above).

### Moved numbers (old to new)

| quantity | old (2026-10-16 record) | new (2026-11-13) |
|---|---|---|
| `CoolPropHem::state_p_t(2 MPa, 350 K)` | `EquationOfState` error (coolprop root rho = 322.0, h about −6.4e18) | Ok, rho = 974.573279183 kg/m3 |
| `CoolPropHem::state_p_t(1.3 MPa, 455 K)` | `EquationOfState` error (metastable vapour, rho = 6.84) | Ok, rho = 885.186131 kg/m3 |
| steam shock tube (500 K, 0.2 / 0.1 MPa, 0.5 ms) star p, IF97 | 0.139940 MPa | **0.140066 MPa** |
| same, CoolProp | 0.139938 MPa | 0.139938 MPa (unchanged) |
| same, max abs dp | 8.7 Pa = 0.009 % of the jump | **290.2 Pa = 0.290 %** |
| same, CoolProp wall time | 28 s | 79 s (concurrent run on a shared machine) |

~~The steam-tube movement is on the **IF97** side; CoolProp's star pressure is unchanged to the digits recorded. It
is not caused by this change (the steam test and the solver were not touched). `tampines-steam-tables/src/hem`
took many commits after 2026-10-16 (variable area, friction and gravity, networks, PETIR transcendentals,
gh:#1102/#1111 choke work); the responsible one was not bisected. The steam test's 5 % bound still holds with
a factor of 17 of margin; the 0.29 % figure is the one to quote now.~~
**EXPLAINED 2026-11-13 (gh:#1346, bisected):** the first bad commit is **1211cc9de** (2026-11-01, gh:#1061, wall
friction in `HemPipe`), not the IF97 backward-equation fix 7be8a7d24 (this state is Region 2a, which that fix does
not touch) and not the PETIR, variable-area, network or choke commits. `git bisect` over
`crates/tampines-steam-tables/src` from 3037f2036 (0.139940) found 2c9b7a02e good and 1211cc9de, c8182db14,
3008d6027, 831b1ae1a and HEAD bad (0.140066). That commit made wall friction a DEFAULT of `HemPipe`, acting whenever
the closure supplies a viscosity. `Iapws97Hem` does; `HemEos::CoolProp` returns `None`, so the CoolProp pipe is
frictionless and the two default pipes stopped being like for like. With `HemPipe::without_wall_friction` the IF97
tube gives **0.139940 MPa and max |dp| = 8.7 Pa = 0.009 %, identical to the 2026-10-16 record**, so the 7 corrected
transposed coefficients moved nothing here. Quote **8.7 Pa (0.009 %)** for IF97 vs CoolProp like for like; **290.2 Pa
(0.290 %)** is IF97 with friction against a frictionless CoolProp, which compares two different models. The
test now gates both (see `coolprop_and_if97_agree_on_a_single_phase_water_shock_tube`). Wall friction is physics
IF97 supplies and stays ON by default; CoolProp's missing viscosity is the gap (gh:#1346 follow-up), not the IF97 side.

| quantity | 2026-11-13 first record | after ablation (like for like) |
|---|---|---|
| steam tube star p, IF97 | 0.140066 MPa (friction) | 0.139940 MPa (same as 2026-10-16) |
| steam tube max abs dp vs CoolProp | 290.2 Pa | 8.7 Pa |
| liquid tube star p, IF97 | 1.499970 MPa | 1.499970 MPa (friction does not move it at 7 digits) |
| liquid tube max abs dp vs CoolProp (L5) | 1464.6 Pa (friction) | 1465.3 Pa (IF97 ablated; the L5 test now ablates) |
