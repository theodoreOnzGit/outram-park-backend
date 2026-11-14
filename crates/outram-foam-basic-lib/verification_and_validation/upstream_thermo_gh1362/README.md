# Thermophysics against OpenFOAM-v2506 (ESI), gh:#1362 row 4

Upstream line: **ESI v2506** (`/home/user/upstream-build/OpenFOAM-v2506`, the
runnable build). The Foundation line was not read or run. Maintainer
direction "ESI preferred"; where the port follows Foundation the ESI form is
added as a named option and both are recorded.

Driver: `driver/thermoDump.C` (unmodified `libspecie` /
`libthermophysicalProperties` templates, built from dictionaries as a case file
would). Rebuild: `scripts/rebuild_reference.sh`. Reference:
`reference/thermo_esi_v2506.txt.gz` (2560 rows: 64 combinations x 10 T x 4 p;
Cp, Ha, Hs, Hc, S, rho, psi, Z, CpMCv, mu, kappa, alphah as hex bit patterns).
Test: `tests/openfoam_thermo_code_to_code.rs`.

Inputs (all literature-style values, none tuned): air `W = 28.96`, CO2
`W = 44.01, Tc = 304.12 K, Pc = 7.374 MPa, omega = 0.2239` for Peng-Robinson;
hConst `Cp = 1004.5, Hf = 2.5e5`; N2 JANAF coefficients; hPolynomial
`CpCoeffs<8> (1000 0.1 -2e-5 3e-9 0 0 0 0), Hf = 1e5, Sf = 200`; Sutherland
`As = 1.458e-6, Ts = 110.4`. Grid: T = 250..1900 K (10), p = 1e5..8e6 Pa (4).

## Findings read from the source before any run

- **Upstream cannot compile `hTabulatedThermo::S`** in v2506: it calls
  `EquationOfState::Sp`, which no equation of state in
  `specie/equationOfState/` defines (`hTabulatedThermoI.H:106`). The driver
  prints 0 for S on those 16 combinations; the port's S for hTabulated is
  therefore not comparable.
- `hTabulatedThermo` upstream is one 1-D `Cp(T)` table with `Hs = int Cp dT`,
  `Ha = Hs + Hf`; the port takes three independent tables (Cp, Ha, S).
- `hPolynomialThermo` upstream: `hCoeffs` offset to `Href` at `Tref`
  (defaults 0 and Tstd), `Ha = hCoeffs(T)`, `Hs = Ha - Hf`, `S = sCoeffs(T)`;
  `Hf` and `Sf` are NOT added to Ha / S (v2506 reads `Sf` but only uses
  `Sref`). The port follows the Foundation form (Hf added at Tstd).
- `Pstd`: upstream `1e5` Pa (`etc/controlDict`, `standard`); port `P_REF = 101325`.

## Predictions (written before the Rust side ran)

Sign and size stated per group; wrong ones will be corrected in place.

- P1: hConst x perfectGas: Cp, Hs, Ha, Hc bit-identical (no libm, same
  operation order); rho, psi differ at ulp level (R = RR/W against R/(W/1000));
  **S differs by `R ln(101325/1e5)` = +3.8 J/kg/K** from the `Pstd` value, so
  not identical, relative size ~1e-2.
- P2: janaf: Cp, Ha, Hs, Hc within a few ulp (coefficients scaled by a port R
  that is ulp-different); S differs by the `Pstd` term.
- P3: hPolynomial: Ha, Hs, S differ **structurally** (kJ/kg size), Cp identical.
- P4: rhoConst: Z differs (upstream 0, port `p/(rho R T)`); rho identical;
  icoPolynomial: rho identical, Z identical (0).
- P5: sutherland / polynomial / tabulated / const transport: mu bit-identical
  for polynomial and tabulated, ulp-level for sutherland (`sqrt` is exact);
  kappa follows Cp/Cv so inherits their differences.
- P6: PengRobinson: Z, rho, psi, Cp, H, S differ above ulp level (the port
  solves the cubic with `CubicEqn`, upstream uses the trigonometric / cbrt
  closed form and `RR/W` units); size unknown, predicted below 1e-10 relative
  away from the three-root region and larger inside it.
- P7: hTabulated: not comparable until an ESI-form table option exists.

## Results (measured 2026-11-13, release; test `tests/openfoam_thermo_code_to_code.rs`)

Three configurations of the port are compared with the 64 + 16 reference
combinations (counts are bit-identical values; the full per-combination table
is printed by `--nocapture`, the pinned subset is in the test).

| configuration | outcome |
|---|---|
| port defaults (CODATA R, Foundation forms) | `rho`, `psi`, `CpMCv` 0/40 identical, rel 9.0e-7 (the gas constant); `kappa`/`alphah` through Cp; hPolynomial `Ha`, `Hs`, `S` 0/40 (structural); tabulated transport outside the table 0/8 (clamp against extrapolation); janaf outside [Tlow, Thigh] 0/20 (clamp) |
| ESI forms, `W` from `gram_per_mole` | residual is only the unit-conversion ulp: `(28.96*0.001)*1e3 != 28.96`, so `R = RR/W` is 1 ulp off: hConst/perfectGas `rho` 4/40, `S` 11/40, `psi` 0/40 (1-2 ulp) |
| ESI forms, exactly round-tripping `W` | **every non-PengRobinson combination is bit-identical in every quantity** (48 combinations x 40 + the extrap block), PengRobinson 10 of 40 points differ in `Z` by up to 2 ulp |

### Causes, each localised to its first divergent operation

1. **Gas constant (rel 9.0e-7).** v2506 `etc/controlDict` sets `NA = 6.0221417930e23`,
   `k = 1.38065e-23`; `RR = 1e3 * NA * k = 8314.47006650545` J/(kmol K), which is
   9.0e-7 above CODATA 2019 (8314.462618...). Every `R`-dependent quantity
   (`rho`, `psi`, `CpMCv`, janaf coefficients scaled by `R`, Hc, S, kappa of
   sutherland) carried it. **Option added:** `GasConstant::EsiV2506`
   (`with_gas_constant` on `PerfectGas`, `RhoConst`, `IcoPolynomial`,
   `PengRobinsonGas`); default stays CODATA 2019, which is the more accurate
   constant (the case file's 6-digit `k` is the source of upstream's value).
2. **`Pstd`: defect, fixed (default).** Port `P_REF = 101325`, upstream `Pstd = 1e5`.
   Entropy shifted by `R ln(1.01325)` = 3.8 J/(kg K) for air. Now `1e5`.
3. **`perfectGas::psi`: fixed.** Port `rho/p`, upstream `1/(R T)`: equal
   mathematically, 27/40 points 1 ulp apart. Now upstream's form.
4. **`constTransport::alphah`: fixed.** Port `kappa/Cp`, upstream `mu*rPr`.
5. **`rhoConst::Z`: fixed.** Port `p/(rho R T)`, upstream (both lines) `0`. No
   consumer in the workspace.
6. **`PengRobinsonGas`: defect, fixed.** `ap = da/dT` and `app = d2a/dT2` were
   built from `a*alpha(T)` where upstream uses `a` without `alpha` (the
   `alpha` derivative is inside the bracket already). Effect on `Cp` departure
   and `CpMCv`: up to **6.9e-2 / 1.0e-1** relative at T up to 1900 K on the
   test grid (alpha falls to 3.5e-3 at 1900 K). The independent check added to
   the unit tests: `Cp_dep = dH_dep/dT` by finite difference to 2e-3.
7. **`PengRobinsonGas` forms.** `H`, `E`, `S` use the rounded `2.414` and
   `0.414` in upstream; the port used the exact `1 +- sqrt(2)` (departure
   1.2e-3 in H, 4e-5 in S); upstream `Cp`/`Cv` use the exact `sqrt(2)`. `Z`: the
   port solves the cubic with `CubicEqn` (the Foundation reading), upstream the
   closed trigonometric / cube-root form. **Option added:** `PengRobinsonForm::EsiV2506`
   (a line-for-line translation in upstream's kmol basis); default `Port`.
8. **`hPolynomialThermo`.** ESI offsets `hCoeffs`, `sCoeffs` to `Href`, `Sref` at
   `Tref` and never adds `Hf`/`Sf`; the port follows the Foundation form
   (`Ha(Tstd) = Hf`). **Option added:** `HPolynomialThermo::new_esi_v2506`; default
   unchanged.
9. **`hTabulatedThermo`.** ESI: one 1-D `Cp(T)` table, `Hs = int Cp dT`. Port: three
   independent tables. **Type added:** `HTabulatedEsiThermo` (with
   `thermophysics::table::{NonUniformTable, IntegratedNonUniformTable}`).
   Upstream defect found: `S` calls `EquationOfState::Sp`, which no equation of
   state defines, so `hTabulatedThermo::S` cannot be instantiated in v2506
   (reference S is 0 and not compared). Second upstream defect:
   `nonUniformTable::index()` for T above the last knot returns the last
   *knot* index, so `f()` reads `values_[size]` one past the end (undefined
   behaviour); those rows are not a reference and the port stops at the last
   interval. Also: for T exactly equal to the last knot, `nd = 0` and the
   interval 1 is used (reproduced).
10. **`tabulatedTransport`.** ESI extrapolates linearly below the table, the port
    clamps. **Option added:** `TabulatedTransport::new_esi_v2506`.
11. **`janafThermo` T clamp.** The port clamps T into [Tlow, Thigh] in `cp`, `ha`,
    `s`; ESI evaluates `coeffs(T)` unclamped (only `limit()` inside the T
    inversions clamps). **Option added:** `with_temperature_clamp(false)`;
    default stays clamped.
12. **PengRobinson residual (ESI form).** 10/40 `Z` differ by 1-2 ulp, 0 other
    quantity differs on a row where `Z` is identical (test
    `peng_robinson_mismatches_follow_z`). `Z` is the only place with `acos`,
    `cos`, `cbrt`: PETIR against glibc. A python replay of upstream's `Z` on
    glibc reproduces all 40 reference values, so the translation is exact.
    Accepted per the PETIR/glibc rule, pinned.

### Not covered

Temperature inversions (`THs`, `THa`, `TEs`: port Newton against upstream's
`T()` with `limit`), `sutherland` constructed from two points, `PengRobinson`
in the liquid root region (the grid has single-root and three-root points but
upstream's choice of root there is the max root as ours), `Boussinesq`,
`rPolynomial`, `perfectFluid`, `WLF`, `hPower` and the other upstream types the
port lacks. Foundation (dev) was not read or run.

### Predictions against the first run (port as it stood)

- P1 half wrong: Cp, Hs, Ha, Hc identical as predicted, `S` differed by the
  `Pstd` term as predicted (rel 6.1e-1 where S is near zero), but `rho`/`psi`
  differed by **9.0e-7** (the gas constant), not at ulp level.
- P2 wrong in size: janaf differed by 9.0e-7 (R scaling the coefficients), not
  a few ulp.
- P3 right (kJ/kg-size `Ha`, `Hs`, `S`; Cp identical). P4 right (rhoConst `Z`
  differs, icoPolynomial identical). P5 right for `mu`; sutherland `kappa`
  carried the gas constant (8e-8 to 2.5e-7).
- P6 **wrong**: predicted below 1e-10, measured up to **6.9e-2** in Cp and
  **1.0e-1** in CpMCv, from a port defect (the `alpha(T)` factor in `da/dT`),
  not from the solver form; the solver form and the rounded constants gave
  ~1e-3 in H and ~4e-5 in S.
- P7: hTabulated needed a new type (done) and upstream cannot compile its `S`.

---

## gh:#1378 family 1: six more EOS, `hPower`, `eConst`, `WLF`, `logPolynomial`

Ported (additive, all read from the v2506 `*I.H` before writing):
EOS `incompressiblePerfectGas`, `Boussinesq`, `adiabaticPerfectFluid`,
`perfectFluid`, `rPolynomial`, `linear` (`eos/esi_simple_eos.rs`); thermo
`hPower` (`thermo/h_power.rs`), `eConst` (`thermo/e_const.rs`); transport `WLF`
(`transport/wlf.rs`), `logPolynomial` (`transport/log_polynomial.rs`). New trait
method `EquationOfState::cv_eos` (upstream `EquationOfState::Cv`, needed by
`eConst`; default `Cp_eos - (CpMCv - R)`, overridden to 0 for the liquid-like EOS).
Not in v2506 at all (checked by listing the directories): `ePower`,
`eIcoTabulated`, `hIcoTabulated`. The `icoTabulated` EOS exists in v2506 and is
deferred (needs a T-table EOS; filed as a child).

Driver `driver_gh1378/thermoDump1378.C`, reference
`reference/thermo_esi_v2506_gh1378.txt.gz`, test
`tests/openfoam_thermo_gh1378.rs`. Inputs (literature-style, none tuned): air
`W = 28.96` (exactly round-tripping `W` and the ESI gas constant, the
2026-11-13 bit-identical configuration); `incompressiblePerfectGas pRef 1e5`;
`Boussinesq rho0 1000, T0 300, beta 2e-4`; `adiabaticPerfectFluid p0 1e5, rho0
1000, gamma 7.15, B 3e8`; `perfectFluid R 3000, rho0 900`; `rPolynomial C
(1e-3 2e-7 1e-9 4e-13 1e-16)`; `linear psi 4.5e-7, rho0 998`; `hPower C0 1000,
n0 0.7, Tref 300, Hf 1e5`; `eConst Cv 718, Hf 2.5e5`; WLF `mu0 1e3, Tr 300, C1
17.44, C2 51.6, Pr 10`; logPolynomial `muLogCoeffs<8> (-13.1 0.7 0 ..)`,
`kappaLogCoeffs<8> (-9.2 0.8 0 ..)`. Same T x p grid as above.

### Predictions (written before the Rust side ran)

- Q1: incompressiblePerfectGas, Boussinesq, rPolynomial and perfectFluid (except
  `S`) carry only `+ - * /` and squares: with the ESI gas constant and exact `W`,
  **bit-identical, 40/40** in every quantity.
- Q2: `linear` `S` and `perfectFluid` `S` use one `log`: PETIR against glibc, a few
  1-ulp differences (<= 10 % of points), worst <= 2 ulp.
- Q3: `adiabaticPerfectFluid` `rho`, `psi`, `S` use `pow` with exponent 1/7.15:
  1-ulp differences on some points (up to 30 %), worst <= 2 ulp; `S` may
  amplify by cancellation in `pow(p+B,n) - pow(Pstd+B,n)`.
- Q4: `hPower` (n0 = 0.7, `pow`): 1-ulp differences in `Cp`, `Hs`, `Ha`, `S` on some
  points; `Ha` follows `Hs`; `Hc` identical.
- Q5: `WLF` `mu` (one `exp`): 1-ulp on some points; `kappa`, `alphah` follow `mu`.
- Q6: `logPolynomial` `mu`, `kappa` (`log` then `exp` of a polynomial of size ~ -9):
  the absolute error of `log` (1 ulp of ~6) becomes a few ulp after `exp`; up
  to 4 ulp, many points differing.
- Q7: `eConst` `Cv`, `Es`, `Ea`, `Hc` (no libm): **bit-identical** on every EOS.
- Q8: wherever a value differs, the first divergence is the `pow`/`exp`/`log` call
  and nothing downstream of it: the rows with identical libm argument and
  differing result localise it.

### Results (measured 2026-11-14, release; `tests/openfoam_thermo_gh1378.rs`)

**56 of 56 combinations are bit-identical in every quantity, 56 x 12 x 40 =
26 880 values, 0 ulp**: {perfectGas, rhoConst, incompressiblePerfectGas,
Boussinesq, adiabaticPerfectFluid, perfectFluid, rPolynomial, linear} x
{hConst, hPower} x {const, WLF, logPolynomial} (48), plus the eight `eConst`
rows (`Cv`, `Ea`, `Es`, `Hc`, `rho`, `psi`, `Z`, `CpMCv`). The check can fail:
perturbing `beta` of the Boussinesq test input by 5e-7 relative drops 7
combinations to `rho` 4/40 at once (mutation run, reverted).

Predictions against the run:

- Q1 right (bit-identical; also `perfectFluid` `S`, the `log` case of Q2).
- Q2, Q3, Q4, Q5, Q6 **wrong in the direction of "more differences"**: I expected
  1-ulp to 4-ulp differences from PETIR `pow`, `exp`, `ln` against glibc;
  there are none. Calls compared: `pow` with exponent 1/7.15, 0.7, 1.7 (adiabatic
  `rho`, `psi`, `S`; hPower `Cp`, `Hs`, `S`), `exp` (WLF, whose argument reaches
  ~ -545 at 250 K, and logPolynomial), `ln` (logPolynomial, linear `S`,
  perfectFluid `S`), 40 points each, all identical. This is a statement about
  these arguments, not a proof that PETIR equals glibc everywhere (the
  gh:#1362 PengRobinson `acos`/`cos`/`cbrt` rows do differ).
- Q7 right. Q8 vacuous (nothing diverges).

Not compared, by construction: `eConst` `Cp`, `Hs`, `Ha`, `S` (undefined in
v2506, Foundation reading in the port, see `thermo/e_const.rs`).

Documented follow-up (not a defect of the new code): the four pre-existing
wrappers `HConstThermo`, `JanafThermo`, `HPolynomialThermo`, `HTabulated*Thermo`
and the four pre-existing transports do not forward `cv_eos`; the trait default
is exact for gases and wrong for a liquid-like EOS if one of them is ever
placed under `EConstThermo` (they are not: `EConstThermo` wraps the EOS
directly).

---

## gh:#1378 family 2: reaction-rate classes

Ported (`thermophysics/reaction_rate.rs`, read from `specie/reaction/reactionRate/**/*I.H`
of v2506, keeping operation order and every `mag(x) > VSMALL` guard):
`ArrheniusReactionRate`, `thirdBodyArrheniusReactionRate`,
`LandauTellerReactionRate`, `powerSeriesReactionRate`, `JanevReactionRate`,
`LangmuirHinshelwoodReactionRate`, `infiniteReactionRate`,
`thirdBodyEfficiencies`, `Lindemann`/`Troe`/`SRI` fall-off functions,
`FallOffReactionRate<Arrhenius, F>`, `ChemicallyActivatedReactionRate<Arrhenius, F>`.
Search before writing: grep for `Arrhenius`, `reaction`, `chemistry`, `species`,
`mixture` over `crates/` found only unrelated domain code (`outram-park-fork-dwsim-libs`
`reactions.rs` is a process-unit kinetics layer with a different interface;
`outram-park-fork-offbeat` corrosion/creep Arrhenius laws are material models;
`outram-park-fork-thermochimica` is Gibbs minimisation); none implements the
OpenFOAM `reactionRate` classes, so this is new.

Driver `driver_rates_gh1378/rateDump1378.C`, reference
`reference/rates_esi_v2506_gh1378.txt.gz` (one hex double per line:
`tag iT ic bits`), test `tests/openfoam_rates_gh1378.rs`. Grid: T = 250, 300,
500, 800, 1000, 1500, 2000, 3000 K (8) x four concentration vectors of three
species (kmol/m^3: (1e-2, 2e-2, 5e-3), (1e-4, 3e-4, 2e-4), (1, 0.5, 0.25),
(1e-8, 1e-8, 1e-8)); third-body efficiencies (1.0, 2.5, 0.4); `p = 1e5` (unused
by every class, as upstream). Inputs are typical combustion-kinetics magnitudes
(H2/O2 mechanism style: Arrhenius `A 3.5e15 beta -0.406 Ta 8353`; Troe `alpha 0.5
Tsss 1e-30 Ts 1e30 Tss 1e100`), none tuned; no measurement is claimed.

### Predictions (written before the Rust side ran)

- R1: Arrhenius, third-body Arrhenius, LandauTeller, powerSeries, Janev,
  LangmuirHinshelwood, infinite, Lindemann fall-off use only `pow`, `exp`, `ln`,
  `cbrt`, `+ - * /`. Given family 1 (PETIR = glibc on every `pow`/`exp`/`ln` point
  tried) I predict **bit-identical, every point**, allowing 1-ulp points in the
  `cbrt` (LandauTeller) and Janev `pow(lnT, n)` cases.
- R2: Troe and SRI add `log10` and `pow(10, x)`: predict **at least one 1-ulp
  difference** among the 32 points of a rate, and that `FallOff` and
  `ChemicallyActivated` built on them inherit it through `F`.
- R3: the Troe `max(.., SMALL)` branch is not active on the main inputs
  (`Fcent` ~ 0.5), so a missing guard would be invisible there; one extra Troe
  row with `alpha 1, Ts 1e-30` (argument underflows to 0) exercises the
  `SMALL` branch; predicted identical.

### Results (measured 2026-11-14, release; `tests/openfoam_rates_gh1378.rs`)

21 rate configurations x 32 points = 672 values. **15 configurations are
bit-identical (480 values)**: all four Arrhenius, thirdBody, powerSeries, Janev,
infinite, LandauTeller/plain, FallOff Lindemann, FallOff Troe (`alpha 0.5`),
FallOff Troe/small (the `max(.., SMALL)` branch, R3), FallOff SRI,
ChemicallyActivated Lindemann and SRI. **Six configurations differ at 1 to 6 ulp
(192 values, of which 15 differ; 657 of 672 values are bit-identical overall)**:

| configuration | identical | worst ulp |
|---|---|---|
| LandauTeller/full | 28/32 | 6 |
| LandauTeller/noTa | 28/32 | 5 |
| LangmuirHinshelwood | 30/32 | 2 |
| FallOff/Troe/finite | 30/32 | 1 |
| ChemicallyActivated/Troe | 30/32 | 1 |
| FallOff/SRI/e | 31/32 | 1 |

**Cause, localised to the first divergent operation.** A Python replay of the
same formulas (`scripts/replay_rates_glibc.py`; `math.*` and `**` are glibc)
reproduces **all 672 reference values**, so every translation is exact. The 569
distinct libm calls it makes are listed in `reference/libm_calls_rates_gh1378.txt`;
PETIR against glibc on those exact arguments (test `libm_localisation_report_and_pins`):

| function | calls | PETIR differs | worst ulp |
|---|---|---|---|
| `cbrt` | 8 | 2 | 1 |
| `exp` | 163 | 1 | 1 |
| `log10` | 42 | 2 | 1 |
| `pow` | 348 | 1 | 1 |
| `log` | 8 | 0 | 0 |

The LandauTeller 5-6 ulp comes from a 1-ulp `cbrt`, amplified by `exp` of an
argument near 3.7. The `cbrt` case was checked against the true value
(`T = 800`: PETIR and Rust `std` return the correctly rounded
`0x402290fca9c761f8`, error 8.1e-16; glibc returns the neighbour
`...61f7`, error 9.7e-16), so these residuals are glibc's last bit, not a PETIR
defect. Accepted per the PETIR/glibc rule, pinned both ways. Rust `std`'s
`cbrt` is therefore not a stand-in for glibc here and is not used as one.

Predictions against the run: R1 **partly wrong**: LandauTeller (`cbrt`, allowed for)
and LangmuirHinshelwood (one `exp` or `pow` call, not predicted) differ; the other
eight classes are identical as predicted. R2 **partly right**: Troe/finite and its
ChemicallyActivated use differ at 1 ulp from `log10` (2 of 42 calls) as predicted,
but `FallOff/Troe` (`alpha 0.5`) and both SRI/`e = 0` rows are identical, and
the SRI difference is in the `e != 0` row (a `pow`). R3 right (identical).

---

## gh:#1378 family 3: multicomponent mixture rules

Ported (`thermophysics/mixture.rs`, plus an additive `impl Mixable` next to each
type): `specie::operator*`/`+=` (`Y`, `W = sumY/(Y/W + Y2/W2)`), the per-model
`operator+=` parameter mixing for `perfectGas`, `rhoConst`, `hConstThermo`,
`constTransport`, `sutherlandTransport`, and the three `multiComponentMixture`
routines `cellMixture`, `cellVolMixture`, `correctMassFractions`. Deferred
(same pattern, other `*I.H` files): the remaining EOS/thermo/transport models;
`janafThermo` (range intersection and `Tcommon` check). `coefficientMulticomponentMixture`
and `valueMulticomponentMixture` of OpenFOAM-dev are not in v2506 (v2506 has
`multiComponentMixture`, `reactingMixture`, `egrMixture`, ...); not ported.
Search before writing: `crates/` has no species-mixture algebra of this form
(grep `mixture`, `species` found `outram-foam-multiphase`, `dover`, `sembawang`
and `thermochimica` domain code only).

Driver `driver_mix_gh1378/mixDump1378.C` evaluates, with the REAL
`species::thermo<transport<hConstThermo<eos<specie>>>>` classes, the exact loops
of `multiComponentMixture::cellMixture`/`cellVolMixture` (a mesh is needed for
the class itself, so the three-line loops are copied from
`multiComponentMixture.C` v2506; the operators `*` and `+=` they call are the
real ones) and `correctMassFractions` (copied). Species: W 28.0, 44.01, 18.015
kg/kmol; Cp 1040, 850, 1900; Hf 0, -8.94e6, -1.34e7; mu 1.8e-5, 1.5e-5, 1.2e-5;
Pr 0.7, 0.8, 0.9; As 1.46e-6, 1.5e-6, 1.2e-6; Ts 110, 240, 150; rho (rhoConst)
800, 1000, 998. Mass-fraction sets: (0.5,0.3,0.2), (0.9,0.05,0.05), (1/3 x3),
(0.7,0.3,0), the un-normalised (0.4,0.4,0.4) before and after
`correctMassFractions`. Grid: T 300, 600, 1200 K x p 1e5, 5e6 Pa. Output per row:
`Cp Ha Hs Hc S rho psi Z CpMCv mu kappa alphah W`. Reference
`reference/mix_esi_v2506_gh1378.txt.gz`, test `tests/openfoam_mixture_gh1378.rs`.

### Predictions (written before the Rust side ran)

- M1: mixing is `+ - * /` only. With the ESI gas constant, every quantity of
  every mixture is **bit-identical**, apart from one risk I can name: the port
  holds `W` in kg/mol, upstream in kg/kmol, and a mixture `W` is an arbitrary
  double for which `cand * 1e3 == W` may have no solution among the
  neighbouring doubles (`molar_mass_from_kg_per_kmol` then falls back to
  `W * 1e-3`). I predict that happens for a minority (<= 30 %) of the
  mixtures and shows as 1-ulp differences in `rho`, `psi`, `CpMCv`, `S` only
  (everything through `R`), not in `Cp`, `Hc`, `mu`, `W`.
- M2: `cellVolMixture` agrees as `cellMixture` does (same operators; the weights
  are computed in the same order).
- M3: `correctMassFractions` of (0.4, 0.4, 0.4): `Yt = 1.2`, warns; normalised
  fractions bit-identical.
