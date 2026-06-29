# TAMPINES Steam Tables
In house steam tables for the Thermo-hydraulic Artificial intelligence 
Multi-Phase INtegrated Emulator System (TAMPINES) solver.


This relies heavily upon the [Rust-steam](https://github.com/marciorvneto/rusteam)
library licensed using the MIT license. 

However, [Rust-steam](https://github.com/marciorvneto/rusteam) is incomplete 
for now. Moreover, it does not use the units of measure library. This 
set of steam-tables is meant to used dimensioned units by default. It will 
also incorporate verification tests from the following reference:

Kretzschmar, H. J., & Wagner, W. (2019). 
International steam tables. Springer Berlin Heidelberg.

Significant portions of code will be copied from the rust-steam package.
Hence, I am putting the rust-steam license here.

## Note on AI usage

Until last month, AI was hardly used in this project. From this month
(June 2026) onwards, Claude Code was used in the testing and development of
the choked flow algorithms in vapour-liquid equilibrium (VLE).

# FHR Educational Simulator 

## To Run on Windows

For installation, you can just download the fhr_sim_v2.exe from the 
release tags. Just download the exe file will do

## Development and Testing
tampines-steam-tables was used to construct the secondary loop of the  
a Fluoride Salt Cooled High Temperature Reactor (FHR) educational 
simulator. The secondary loop just runs at steady state (no transient 
calculations for simplicity.
```bash
cargo run --release --example fhr_sim_v2
```

Note that for windows PCs, sometimes there will be problems where 
windows defender blocks the fhr_sim_v2 from being run. In those cases,
it's better to use windows subsystem for linux (WSL). One needs to note 
to use:

```bash
sudo apt install libopenblas-dev
```

Before running:
```bash
cargo run --release --example fhr_sim_v2
```

I used rustup to install rust. So if versions of Rust are outdated 
(error messages may tell you so), then use:

```bash
rustup update stable
```


## To resize

Note: If you want to resize, use Ctrl+ and Ctrl- to change the size of the 
simulator.


# Changelog

v0.2.1 — transient mass & energy balance and
unified multiphase critical-flow dispatcher

> **Design / thought process: human-authored** (written by the maintainer; the
> Rust implementation and tests were carried out by an AI assistant following
> this specification).

Control volumes can now gain or lose mass over a timestep, with the new
thermodynamic state recovered by an iterative `(p, h)` flash.

The mental model and algorithm (as specified):

1. Build a **vector of `(mass, specific-enthalpy)`** source/sink terms *outside*
   the control volume — kept external so `TampinesSteamTableCV` stays a small
   `Copy` value rather than carrying a `Vec`. This is the new
   `CvMassEnthalpyChanges` ledger; `add_mass()` and `remove_mass()` push a
   `(mass, enthalpy)` tuple (with `uom` types) onto it (removal stored as a
   negative mass).
2. Call `TampinesSteamTableCV::advance_timestep(&ledger)`. It sums the **total
   mass change** and the **total enthalpy change** added to the system.
3. The control-volume geometric volume is fixed, so the new state has a new
   `(ρ, h)` point: the mass is `ρ · V`, the specific enthalpy is the
   mass-weighted total enthalpy over the new mass, and the specific volume is
   `V / m_new`.
4. To get the thermodynamic state from that `(ρ, h)` point we iterate on
   pressure with **regula falsi (false position)**, evaluating the `(p, h)`
   flash `v(p, h)` until it matches the target specific volume `V / m_new`, then
   rebuild the control volume from `(p, h_new)`.

Implementation notes from the build-out:

- At fixed enthalpy, specific volume decreases monotonically with pressure, so
  the residual `v(p, h) − v_target` has a single sign change — well suited to
  regula falsi.
- The bracket is grown **outward from the control volume's current pressure**
  (a known-valid point) toward the root, not from a blind 100 MPa endpoint. The
  `(p, h)` flashes are **not implemented for region 5 (T > 800 °C)**, and
  marching down from 100 MPa could land there and panic; seeding from the
  current pressure keeps every evaluation inside the validated range.
- `advance_timestep` panics on non-physical input (removing more mass than is
  present, or a `(v, h)` target outside the validated steam-table range) rather
  than silently returning a wrong state.


`get_crit_pressure_and_massflux` (and `get_stagnation_critical_mass_flux`) on
`TampinesSteamTableCV` are now a single **generic multiphase dispatcher**,
`get_critical_pressure_and_mass_flux_multiphase_ph`, that routes a stagnation
state `(p0, h0)` to the right HEM solver by its `ph_flash_region`:

| Stagnation region | Solver |
|---|---|
| Region 4 (two-phase, in-dome) | `get_critical_pressure_and_mass_flux_ph_vle_dome` |
| Region 1 (subcooled liquid) | `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` |
| Region 2 / 5 (superheated / ultra-high-T vapour) | `get_critical_pressure_and_mass_flux_superheated_vapour_ph` |
| Region 3 (supercritical), isentrope re-enters the dome | `dome_crossing_interior_choke` (new) |
| Region 3 (supercritical), no dome crossing | superheated or subcooled by `s0 ⋛ s_crit` |

All 13 `generic_multiphase_stagnation` tests now pass (previously `#[ignore]`d),
with **per-point tolerances that match the dedicated region tests**: Region 4 →
0.005 (0.01 for the near-bubble x_t = 0.05 curve, as `in_dome_stagnation.rs`),
Region 1 → 0.03 (as the subcooled test), Region 2/3 → 0.05 (as the near-critical
x_t = 0.80 superheated curve), mass flux → 0.05 log10 everywhere. The deprecated
combined `get_critical_pressure_and_mass_flux_with_stagnation_props` is retained
for reference only and is no longer wired into the OOP API.

**Debugging trail — why the near-critical Region 3 points were hard.**
The only points that failed when the dispatcher first routed by region were the
single **3000 psia** throat of every quality curve. Every other point (≈600)
passed with < 1 % error; the failures sat at +5.6 … +6.6 %. The investigation:

1. *Round-trip diagnostic per point.* Reporting region, `p_calc`, `p_ref` and
   error for every point (instead of panicking on the first failure) isolated
   the failures to exactly the 3000 psia point, which backward-maps to a
   **supercritical Region 3** stagnation state (`p0 ≈ 28 MPa`) whose throat sits
   at 20.68 MPa ≈ 0.94·p_crit — right under the dome apex.
2. *Scanning the HEM energy-balance `G(p) = ρ·√(2(h0−h))` along the isentrope*
   showed the true cause: a **spurious kink-peak at the supercritical→two-phase
   phase boundary** (the apex, ~22 MPa). Crossing the boundary makes `G` non-
   smooth, and the `max-G ⇔ M = 1` choke equivalence only holds at *smooth*
   stationary points — so the single-phase solvers latched onto the apex kink
   instead of the genuine, much shallower interior two-phase choke at ~20.7 MPa
   (which matches Zaloudek to ~0.3 %). This is the critical-point analogue of the
   already-documented bubble-point artifact on the subcooled side. The mass flux
   barely differs across the band (`G` is flat to < 0.6 %), so only the *pressure*
   localisation was wrong.
3. *A fine (20 kPa) scan near the apex* exposed why naive interior-max searches
   failed: the IF97 Region-3/4 backward equations lose digits within ~0.5 K of
   Tc, peppering `G` with isolated single-sample glitches (one spike dropped `G`
   to ~60 % of its neighbours) on top of a band that is otherwise flat to
   < 0.01 %. "Strongest interior max", "first local min then first local max",
   and "global max below the trough" each got fooled by a different glitch or by
   the high-G decline just below the apex.
4. *Robust finder (`dome_crossing_interior_choke`).* Coarse-scan `G` over the
   two-phase stretch, apply **two passes of a 3-point median filter** to excise
   the IF97 glitches, **exclude a 0.8 MPa kink+decline margin** below the dome
   entry, then take the band peak nearest the **low-pressure** side (the first
   local max within 1 % of the band maximum). This matches Zaloudek's choke
   pressure, which sits at the low end of the flat near-critical band.
5. *Tolerances & methodology.* Switching the test from Zaloudek's digitised
   stagnation enthalpy to the self-consistent backward-mapped `h0_calc` (the
   methodology the split tests use) tightened the whole round trip: with the
   robust finder, **all Region 3 points land < 0.5 %**, Region 1 < 1.4 %, and
   Region 4 < 0.9 % (the > 0.5 % cases are all the near-bubble x_t = 0.05 curve,
   exactly where the in-dome test also relaxes to 0.01).

The near-critical choke pressure remains genuinely ill-conditioned (the HEM `G(p)`
is flat to < 0.6 % across a ~1.5 MPa band, and IF97 loses precision near Tc), so
the Region 3 tolerance is the same 0.05 the superheated test already uses for its
near-critical x_t = 0.80 / 3000 psia point — not a physics limit, an IF97 one.

v0.2.0

Consolidated into the OUTRAM PARK workspace. Dependency bumps (`uom` 0.36→0.38,
`ndarray` 0.15→0.17, `ndarray-linalg` 0.16→0.18, `thiserror` 1→2,
egui/eframe 0.29→0.34, `egui_plot`→0.35). All egui examples migrated to 0.34.

**Multiphase HEM choked flow — near-saturation (x ≈ 0) fix (v0.2.0)**

The multiphase Homogeneous Equilibrium Model (HEM) critical-flow solvers now
reproduce all Zaloudek quality curves, including the saturated-liquid line:

- `get_critical_pressure_and_mass_flux_subcooled_liquid_ph`: now validated for
  **all** throat qualities x_t = 0.0–1.00. The previous near-saturated (x_t ≈ 0)
  failure — mass-flux artifacts at 5–10 psia and 11–21% choke-pressure errors at
  15–200 psia — was a numerical issue in the forward choke finder, **not** an HEM
  physics limitation (HEM evaluated at the throat reproduces the reference to
  ±0.04 in log10 G). The energy-balance maximum of G is blind to the sound-speed
  discontinuity at the bubble point; the solver now detects the near-saturation
  regime by the two-phase quality at the energy-max choke (< 0.03) and takes the
  bubble-point kink choke, reading ρ_f·c_2φ from a precomputed sonic map along the
  saturated-liquid line. The test `quality_bubble_point_subcooled` is no longer
  `#[ignore]`d.

- `get_critical_pressure_and_mass_flux_ph_vle_dome`: validated for two-phase
  stagnation states (x_t = 0.0–1.00, all 21 Zaloudek HEM reference curves pass;
  note: these are HEM-computed curves digitised from Saha 1978, not measurements).

- `get_critical_pressure_and_mass_flux_with_stagnation_props`: older combined
  dispatcher, **superseded** by the two split solvers above. Had a +25%
  choke-pressure artifact near the saturated-liquid line due to the
  finite-difference sound speed used internally. Retained for reference only.

v0.1.8 

The key part for this is to do verification and validation for critical 
flow using the homogeneous equation model. The steam tables themselves are 
done, but the sonic flow thermodynamics equations are to be added, with 
simple demonstration of the rhoPimpleFoam derived algorithms.

The critical mass flux for homogeneous equilibrium steam-water will be 
verified and validated against figure 1 in Moody's publication:

```
https://www.osti.gov/servlets/purl/7309475
```
Moody, F. J. (1975). Maximum discharge rate of liquid-vapor mixtures 
from vessels (No. NEDO--21052). General Electric Co., San Jose, 
CA (United States). BWR Projects Dept..


Data was read via graph reader

v0.1.7

Added and tested some diverging nozzle functions post choked flow.
This includes where choked flow isentropically decelerates to subsonic 
speeds at outlet pressure, or isentropically accelerates supersonically to 
outlet pressure. This is done using a combination of (p,s) and/or (h,s)
algorithms.

Moreover, between these two pressures, we expect normal shocks to occur 
in the nozzle. For this, we use a combination of (p,h) algorithms with a 
velocity scanning method with regula falsi, to solve for v, such that 
the outlet mass flowrate equals that at the choke.

Added a joule thomson algorithm for throttling where kinetic energy is 
non negligible.

For verification and validation, I'm considering using:
```
https://www-pub.iaea.org/MTCD/Publications/PDF/TE-1677_web.pdf
https://www.kns.org/files/pre_paper/11/63%EA%B9%80%EC%8B%9C%EB%8B%AC.pdf
https://www.osti.gov/servlets/purl/7309475
```

I am searching for blowdown tests. And it seems this one at NRC may 
just be the right one:

```
https://www.nrc.gov/docs/ML1927/ML19270F127.pdf
```

RELAP5 - MODELS, CODE STRUCTURE, AND APPLICATIONS

And then, based on an AI search (Gemini), Marviken tests:

```
Marviken critical flow test data
https://www.nrc.gov/docs/ML2005/ML20052H367.pdf
```

The Marviken tests seem to best fit these.

However, doing these tests do involve phase equilibria, and some metastable 
states. Hence, these are not yet implemented. What are implemented are 
tests that deal with superheated steam. For these, the CD nozzle equations 
work relatively well.

Moreover, TampinesSteamTableCV has been given a few more functions for 
convenience such as obtaining saturation temperature and pressure.

v0.1.6

more to be added towards the fhr\_sim\_v2, including turbine animation

Now, (h,s) algorithm is implemented and tested against steam table.
These tests are under interfaces folder of source code.
The backward equations are slightly less accurate 
than (p,h) and (p,s) algorithm. And for low quality steam, it reverts 
to iteratively doing a (p,h) flashing with bisection method. Not too 
efficient, but it does the job (ish).



v0.1.5 

minor update: added a get\_mass() method for the TampinesSteamTableCV

v0.1.4

added object oriented interfaces, aka TampinesSteamTableCV

v0.1.3 

Added a depressurisation example

v0.1.2

Added the FHR educational simulator with a STEADY STATE steam turbine cycle 
as an example of how to use the flash algorithms within this code.


v0.1.1 

Starting the h,s flash algorithms.

Also, copied some openfoam algorithms which will form the basis for which 
the steam tables in tampines are used to solve two phase flow in transient 
scenarios. Since OpenFOAM is licensed under 
GNU GPLv3, tampines-steam-tables will also be licensed under GNU GPL v3.

Note that for points near boundaries, correction factors have not been 
applied for (p,h) and (p,s) flashes. (h,s) flashes have only been 
partly implemented. 

Near critical point for (h,s) flashing backward eqns, 
the temperature, volume and all may be less 
accurate for backward equations, temperatures may be off 
by as much as 5 degrees c, and volumes may differ by 10%
enthalpy of vapourisation may differ by up to 5% compared 
to steam table data compared to steam table data... so beware...
Though in the first place, the sat temperature equations were 
never meant for this critical region..

Thermal conductivity for (h,s) flash off by about 8%. 
Also basic temperature equations also tend
to fail to be accurate around the saturation line for bubble point,
but not sure about dew point. Sometimes, dew point doesn't work 
as in for 8 bar

Also, pressure equations fail to be accurate at low pressures such as 
0.1 bar, 1 bar up to 10 bar. At 0.1 bar, 1 bar, only expect the 
pressure to be accurate to within 20% at least within region 1. 
Accuracy up to 8% was observed for 2 bar pressure, 5% for 4 bar and 6 bar. 
4% for 8 bar, 2% for 10 bar and 20 bar.

For triple point pressure, hs flash doesn't work.

Moreover, not all of (h,s) flash works for region 4. The equations 
only work over a certain entropy bound.

Kappa should not be trusted for hs flash at low temps.
Quality for hs test should not be trusted past supercritical pressure. 
Though at that pressure, we don't really care about quality anymore.
Kind of meaningless because liquid and vapour properties are indistinguishable.

hs flash also fails near boundaries, eg 800C or 1073.15K isotherm.

v0.1.0 

Added dielectric constant and surface tension functions.
Didn't yet test across the whole steam table, but it works for the 
small unit tests.

v0.0.9
Implemented thermal conductivity for ps flash. However, for 160 bar 
and 220 bar steam tables, the max error is 30 and 40% respectively.
For the 220 bar steam tables, it is quite near critical point, 
so thermal conductivity equations were not meant to be accurate there.
However, for 160 bar, it is sufficiently far from critical point that this 
shouldn't be the case. But i'm leaving it as such for now, till such time 
there is a better reference for such properties.

Near critical temperatures and pressures, eg. 180 bar about 
357+ degrees C, the speed of sound, isentropic exponent
the specific heat capacity are not accurate to within 1%. Some discrepancies 
are larger than 5-10% esp near critical region for these properties.

The thermal conductivity and dynamic viscosity also have similar-ish degrees 
of uncertainty.


v0.0.8
Implemented thermal conductivity for ph flash. However, for 160 bar 
and 220 bar steam tables, the max error is 30 and 40% respectively.
For the 220 bar steam tables, it is quite near critical point, 
so thermal conductivity equations were not meant to be accurate there.
However, for 160 bar, it is sufficiently far from critical point that this 
shouldn't be the case.

I'm quite puzzled as to why that is the case. But this is a bug that needs 
to be fixed.

v0.0.7

Starting development of an interface for the forward and backward flash.
First using functional programming, then object oriented programming.
OOP not implemented in this version yet.

Firstly, (p,T) flash, then (p,H) flash.
This is done for all steam table values except for 1000 bar. 
There is a problem for (p,T) and (p,H) flashing at 1000 bar 
as the algorithm complains it's out of range. Will take some debugging 
to settle.

Dynamic viscosity added, can reproduce steam table to within 2%.
Thermal conductivity can reproduce steam table values to within 1% 
except for regions near critical point (220 bar) and from 100 bar 
up to 200 bar.
Thermal conductivity yet to be implemented for (p,H) flash,
requires some debugging.




v0.0.6
beginning the addition of backwards eqns

First, pressure and enthalpy (p,h) flash. 
This is applicable for 

region 1, which forward equations are (p,t) flash:
- T(p,h)

region 2, which forward eqns are (p,t) flash:
- T(p,h) 

region 3, which forward equations are (v,t) flash, so it accounts for quality:
- T(p,h)
- V(p,h)

once (p,h) flash is done for regions 1,2 and 3, then you can get T,
for region 1 and 2 or (V,T) for region 3

and then get all your other thermodynamic variables

the ps3 equations (enthalpy to pressure equations) are also added

However, the interface for an overall ph flash or tp flash is not yet 
available.


v0.0.5
Add Region 5 equations (no backwards equations here)

v0.0.4 
Added region 4 vapour liq saturation temp and pressure 
line up to critical point. This includes triple point, 
normal boiling point (100C at 1 atm) and critical point of water.

v0.0.3 
Added region 3, and the saturation temperature and pressure boundary equation 
p23 and b23.
Only forward eqns added. That is (T,P) flash.

v0.0.2

Added region 2, including metastable, dimensioned equations, with verification tests.
Only forward eqns added. That is (T,P) flash.

v0.0.1 

Added region 1 dimensioned equations, with verification tests.
Only forward eqns added. That is (T,P) flash.

## Rust-steam license:

Copyright 2023

Permission is hereby granted, free of charge, to any person obtaining a 
copy of this software and associated documentation files (the “Software”), 
to deal in the Software without restriction, including without limitation 
the rights to use, copy, modify, merge, publish, distribute, sublicense, 
and/or sell copies of the Software, and to permit persons to whom the 
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included 
in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, 
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF 
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. 
IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY 
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, 
TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE 
SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

