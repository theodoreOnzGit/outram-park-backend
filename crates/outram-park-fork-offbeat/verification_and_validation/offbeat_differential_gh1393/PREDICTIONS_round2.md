# Round 2 predictions, written before any run of each family (gh:#1393, 2026-11-13)

Upstream: OFFBEAT `80e84450a115b0c411e1bfa5d166379f6bf6c084`, archive
`/home/user/upstream-build/dl/offbeat/offbeat.tar.gz`, sha256
`7b5cde47ceba60903850f02e66b4eb6591cebfc7d272539b8189d625934b1e93`
(its single top-level directory is `offbeat-80e84450...`).  **Round 1 recorded
no archive hash**, so the round-2 hash cannot be compared with a round-1 value;
it is recorded here for rounds 3+.  The archive was still on disk and was not
re-downloaded.

Same route as round 1: standalone C++ transcription of the `correct()` bodies
(`check_verbatim.sh` greps every `UPSTREAM` line back into the tree); NOT a run
of the compiled OFFBEAT library.

Each section below is committed before the corresponding driver is compiled.

## A. Thermal expansion (14 upstream models; `MatproFthconUO2` is not upstream)

The issue says 15 models: the port enum has 15 variants, one of which
(`MatproFthconUO2`, gh:#1070) has no OFFBEAT class.  Compared: 14.

Derived gate (written before measuring; revised once, before any run, when
the first draft's "4 eps" could not be derived).  Most strains are a
*difference* `a(T) - a(Tref)` of polynomial-like operands.  A degree-3
polynomial evaluated by Horner has forward error <= gamma_6 * S ~ 6u, where
`u = 2^-53` and `S = sum |terms|`; the expanded `pow` form is bounded the same
way, so the two differ by <= 12u*S per operand, plus <= 2u for the outer
multiply/subtract.  Gate G1: `|port - upstream| <= 16u * scale = 8 eps * scale`
(`eps = 2^-52`) where `scale` is the sum of absolute values of all additive
terms of both operands, emitted by the driver.  Gate G2: relative difference
< 1e-14 wherever `|result| >= 0.2 * scale` (condition number <= 5, so G1
implies `8 eps * 5 = 8.9e-15 < 1e-14`).  Near `T = Tref` the relative
difference is expected to be large; it is reported, not gated by G2.

Predictions:
1. G1 and G2 hold for every model.
2. Bit-identical 100 % for: Constant, PARFUMESiC, Molybdenum (same operation
   order, no transcendental or `pow`).  Swindeman: `par1*x*x + par2*x + par3` is
   written upstream as `par1*(T-273.15)*(T-273.15)` and port as
   `SWINDEMAN[0]*tc*tc` -> same order, predicted 100 %.
3. RelapUO2: `exp(-ED/k/Ti)` upstream (two divisions) vs the port's
   `exp(-ED/(k*T))` (one product, one division) -> NOT the same operations;
   predicted differences of 1-3 ulp on the exponential term, bit-identity below
   100 %.  This is a *predicted association difference*, not a defect.
4. Martin, MatproUPuO2, Gehr1515Ti, Lemehov, MAMOX, SneadSiC: `pow(T,2)`,
   `pow(T,3)` vs Horner -> bit-identity well below 100 %, within the G1 bound.
5. MatproZy: the beta-alpha blend is written upstream
   `axAlpha*(TBeta-Ti)/(TBeta-TAlpha) + axBeta*(Ti-TAlpha)/(TBeta-TAlpha)`, port
   `s_alpha*(1-w) + s_beta*w` -> 1-2 ulp in the 200 K blend band only; outside it
   100 % identical.
6. PARFUMEBuffer and PARFUMEPyC: upstream's dead zone omits `mag()`, so upstream
   returns exactly 0 for every `T < Tref` with |strain| >= 1e-7.  The port
   documents that it does NOT reproduce this (deliberate deviation).  Predicted:
   for `T >= Tref` the port agrees (G1/G2); for `T < Tref` upstream = 0 and the
   port is non-zero.  The gate is applied against the driver's `mag()`-fixed
   twin, and the divergence rows are counted and pinned (count predicted from
   the grid before the run: every grid point with `T < Tref` and
   `|strain| >= 1e-7`).
7. MatproUPuO2 constructor reads `par5..par8` from the dictionary keys
   `par1..par4` (upstream copy-paste defect).  Dictionary values are not
   exercised by the port (defaults only), so this is not observable on the
   grid; recorded as an upstream defect only.
8. Lemehov: upstream feeds `Bu/1000` (field is MWd/t-oxide) straight in as
   "GWd/tHM" with no 0.881 factor.  The port takes `state.burnup` in MWd/kgHM
   (= `Bu/1000/0.881` by the crate convention) and uses it as GWd/tHM.  The
   test therefore maps `state.burnup = Bu/1000` (as round 1 did for
   LanningBeyer/Magni/Philipponneau).  If the crate's burnup module feeds
   `Bu/1000/0.881` to Lemehov the result differs by ~13 % in the burnup
   argument; that is an interface-basis question, listed not fixed.

## B. Densification, relocation, swelling (written before the driver is compiled)

Structure.  Upstream's densification and swelling classes are explicit
*incremental* updates driven by `oldTime()` fields (and the min/max against
the old value), the port's are stateless functions of burnup/fluence.  A
comparison is only meaningful where the two coincide, and I fix those cases
before running:

* Single-step cases (old fields zero): densification Empirical/FRAPCON (the
  `min(nominal, old)` with `old = 0` is the identity for a negative nominal;
  `F = 1`, `delta = 0` defaults), swelling FRAPCON/Constant/FrCrAl (stateless
  upstream), growth AIM1/Generalized1515Ti/WrightSham/BISON-Zy (`old` fluence 0 so
  `pow(0, p) = 0`), swelling `PyroCarbonCorrelation` (stateless upstream).
* Multi-step cases: swelling MATPRO UO2 (explicit right-endpoint time stepping at
  fixed T and density vs the port's analytic integral), FBR-MOX (explicit steps
  at a fixed gap state vs the port's piecewise-linear closed form), BISON-Zy
  growth (non-linear in the increment).  Not bit comparisons; derived bounds below.
* Not compared, with reasons: swelling PARFUME buffer/PyC (not ported: need flux,
  timestep and interpolation tables, see the module doc of the port);
  `swellingGrowthMatproZy` (checked 2026-11-13: absent from `Make/files`, so not
  compiled upstream; the port doc's claim is right); relocation recovery
  (`recoveryFraction` default 0 -> inactive; the port has no recovery state);
  upstream's `min()`/`max()` against old-time fields (stateful).

Gate form.  Every row carries a `scale`; gate G1: `|port - upstream| <= 8 eps *
scale`; G2: relative < 1e-14 wherever `|value| >= 0.2 * scale`.  For a product of
factors `scale = cond * |value|` with `cond = 1 + sum(|exponent| * |log-sensitivity|)`
(a power `x^p` with a base that is rounded differently contributes `p`; an
`exp(a)` contributes `|a|`), derived from relative-error propagation.  For sums of
terms of mixed sign, `scale = sum |terms|`.

Predictions:
1. G1/G2 hold for relocation (modified and legacy), densification (Empirical and
   FRAPCON), swelling Constant/FrCrAl/FRAPCON/AIM1/Generalized1515Ti/WrightSham/
   BISON-Zy (one step)/PyroCarbonCorrelation.
2. Bit-identity: Constant, FrCrAl, FRAPCON swelling (same operation order up to the
   burnup conversion `Bu/0.881` vs `(Bu/1000/0.881)*1000`) mostly identical but not
   100 %: 0.881 conversion costs <= 1 ulp.  Empirical densification: upstream's
   `dLmax = -dc/(frac*100+dc)/3` applied as `dLmax*(1-exp)` vs the port's
   `(asymptote*(1-exp))/3`: association difference, predicted bit-identity < 100 %.
3. FRAPCON densification: the fixed-point iteration is the same recurrence; the
   iteration count is predicted <= 10 for the whole grid (the port allows 10,
   upstream 11; if any grid point needs the 11th, port returns zero and upstream
   does not, which would be a port defect).  `abs(B - B0)` is assumed to be the
   double overload (OpenFOAM's `mag` is `::fabs`, <cmath> supplies `std::abs`);
   an `int abs` would stop the loop after one step and is not what this driver
   models.  **Unverified assumption**, listed.
4. Resintering route (`resintDensChange != 0`): upstream writes
   `(0.0015*dc*10960/100)/100`, the port `(0.0015*dc*109.6)/100`; `0.0015*dc*10960`
   is a different rounding from `0.0015*dc*109.6` (109.6 is not exact): predicted
   1-2 ulp on those rows, not 100 % identical.
5. Swelling MATPRO UO2 (stepwise vs analytic): upstream adds, per step, `par2*rho*dBu*
   dT^p exp(par5 dT) exp(par6 rho Bu_k)` with `Bu_k` the END-of-step burnup, i.e. a
   right-endpoint Riemann sum of a monotone integrand.  Exact bound:
   `|sum - integral| <= h |g(b) - g(a)|` -> relative `<= |d| F / N` for `d = par6*rho`,
   `F` the final FIMA, `N` steps, plus rounding `(N+8) eps`.  Predicted: gate holds
   for every `N`; observed error scales ~1/N (slope -1 on a log-log of N from 1 to 4096).
6. FBR-MOX: the port is the continuous limit at fixed gap state; explicit steps use
   `par2` for a whole step that straddles `BuI = 1`.  Bound
   `|sum - port| <= (par1 - par2) * dF + (N+8) eps * value` with `dF` the step in
   %FIMA.  Predicted to hold, with exact agreement (rounding only) when no step
   straddles 1 %FIMA.
7. BISON-Zy growth is path dependent upstream: the radial/hoop increment
   `-(1 - (1+d)^-0.5)` is non-linear in `d`, so summing `N` increments is not the
   transform of the total.  Prediction (second-order expansion): for total axial
   growth `S`, relative difference of the transverse strain between `N` equal steps
   and the port's single cumulative form is `0.75 * S * (1 - 1/N)` (to first order in
   `S`); axial component identical to rounding.  Gate: observed within 20 % of the
   prediction for `S <= 0.02`.  N = 1 is predicted bit-/ulp-identical.
8. No port defect expected in this family.  Documented non-reproductions (NOT
   defects): `min(nominal, old)` / `max(old, new)` histories, recovery, PARFUME
   swelling, `intra/intergranularGasSwelling` additions.

## C. Young/Poisson: MATPRO Zircaloy, PARFUME buffer/PyC/SiC, Snead SiC, constants (before the driver)

Models: `YoungModulusMatproZy` (needs `oxC_`, `cW_`, fluence), `PoissonRatioMatproZy`
(same fields, plus the Young's-modulus *field*), `YoungModulusPARFUMEBuffer/PyC/SiC`,
`YoungModulusSneadSiC`, and the four constant Poisson classes.  Not compared: the
isotropic-cracking branches (need the slice mapper and `lhgr`).

Interface choices fixed before the run.  Upstream Young MatproZy scales the fluence
field (n/cm^2) by `1e4` to n/m^2 before `exp(-phi/1e25)`; upstream Poisson MatproZy
does NOT.  The port uses n/m^2 in both (documented divergence 3 in `poisson_ratio.rs`).
The driver therefore emits two Poisson rows: `as written` (E with `phi*1e4`, G with the
raw `phi`, exactly as the two classes compile) and `consistent` (G also with `phi*1e4`,
which is what the port computes).

Predictions:
1. G1/G2 (section A form) hold for Young MatproZy, PARFUME buffer/PyC/SiC, Snead, the
   constants, and for Poisson-`consistent`.
2. Young MatproZy has the same operation order as the port (`(par7 - par8*Ti + K1 + K2)/K3`,
   the same blend), so it is bit-identical wherever `exp(-phi/par6)` agrees between
   glibc and PETIR (`phi = 0` gives `exp(-0) = 1` exactly): predicted 100 % bit-identical at
   zero fluence and in the beta phase, >= 80 % at non-zero fluence.
3. Poisson-`as written` differs from the port at non-zero fluence in the alpha phase
   (`T < 1073 K`) by exactly the K3 mismatch: `(nu_up + 1)/(nu_port + 1) =
   (0.88 + 0.12 exp(-phi_cm2/1e25)) / (0.88 + 0.12 exp(-phi_cm2*1e4/1e25))`, because the
   fluence factor cancels between E and G in the port but not in the upstream pair.  For
   `phi_cm2 = 1e21` that ratio is 1.0821 (8.2 %; arithmetic slip "1.0831" corrected before any run).  Predicted: the closed form matches the
   observed ratio to <= 16 eps (condE + condG) (derived: each side's relative error is
   <= 8 eps cond), and the difference is zero (at fluence 0) in the beta phase.  The port's
   doc sentence "the choice is immaterial to nu in the alpha phase" is true only inside
   the port's own consistent convention; against the upstream pair it is an 8 % effect at
   1e21 n/cm^2.  If so that sentence needs a qualification (doc fix, not a model change).
4. PARFUME PyC multiplies its factors in a different order from the port
   (`par1*(..)*(BAF)*(Lc)*(phi)*(T)` upstream vs `common*(BAF)` last): not 100 %
   bit-identical, within 8 eps.  Buffer: same order, 100 %.  SiC: identical operations,
   100 %.  Snead SiC: 100 % except `exp`.
5. No port defect expected.

## D. Yield stress `yieldStressFRAPTRAN` (before the driver)

Compared: the yield stress `sigmaY = max((K/E^n (rate/1e-3)^m)^(1/(1-n)), K eps_old^n (rate/1e-3)^m)`
against the port's `YieldStressModel::Fraptran.yield_stress(eps, inputs)` with `eps = eps_old`.  Not
compared: `Hp_` (depends on the current stress `sigmaEq`, a tensor state the port passes differently;
the port's `hardening_modulus` is the derivative of the flow curve, a different quantity).

Reading of the upstream before the run (stated so the prediction can fail):
* The fluence-factor chain is `if (phi < 0.1e25) {..} else if (phi <= 0.1e25 && phi < 2e25) {..}
  else if (phi <= 2e25 && phi < 12e25) {..}`.  Evaluated literally: for `phi` in `(0.1e25, 2e25]`
  the THIRD branch fires (`Kphi = 0.53236 + 2.6618e-27 phi`); for `(2e25, 12e25)` none fires;
  at exactly `0.1e25` the second fires; for `phi >= 12e25` none fires.  The port's code comment
  says that "for any fluence in (0.1e25, 12e25) no fluence factor is applied at all", which this
  reading contradicts for `(0.1e25, 2e25]`.
* `correctYieldStress` executes `phiValue_ *= 1e25;` on a member, i.e. the fluence is multiplied by
  1e25 on EVERY call, not once: the n-multiplier saturates at 1.608953 and the K factor is skipped
  from the second call on.  The driver emits `call = 1` (literal first call) and `call = 2`.
* The port implements the evidently intended `>=` chain (documented deviation) and, above 12e25,
  holds `Kphi` at its 12e25 value (upstream applies none).

Predictions:
1. The port equals the **intended** chain (driver variant `intended`) within G1/G2 on the whole grid
   (scales derived in the driver from the sensitivity `d ln sigma/dn = (ln K - ln E + m ln r)/(1-n)^2`).
2. The port differs from the literal first call for exactly the fluences where the chains differ:
   `phi` in `(0.1e25, 12e25]` and `phi >= 12e25` (K), with relative differences of K of up to
   `+0.7` (`(0.1e25, 2e25]`: literal applies `0.5324..0.5856`, intended `0.0029..0.0586`) and
   `0.5857..0.8518` for `(2e25, 12e25)` (literal: none).  For `phi < 0.1e25` and `phi = 0` literal first call
   and port agree within the gates (`n`-multiplier chain is not affected by the typo).
3. From the second call on the literal upstream is independent of the input fluence (saturated); the
   port cannot reproduce that (stateless), a documented non-reproduction.
4. No other difference; if the "intended" comparison fails, the first divergent operation is
   localised before any change.

## E. Gap conductance, FRAPCON fuel-rod gap (before the driver)

Upstream kernels: `fuelRodGapFvPatchScalarField::hGap` (gas, radiation, contact; scaling `F = 1`, `delta = 0`,
`relax = 1`) and `gapFRAPCON::kappa(T)` / `::a(T)` (mixture conductivity and accommodation coefficient).
Port: `GapConductanceModel::FuelRodFrapcon` and `GapGasMixture`.  Mixtures use dyadic mole fractions
(sum exactly 1) from {He, Ar, Kr, Xe} in the port's species order; the upstream mole fractions `M_` are
inputs (`correctMolFractions` not exercised).  Not compared: the gas-pressure / plenum bookkeeping
(`correct()`), `gapWidth()` geometry, TRISO gap, `gapGasTimeTabulated`.

Known interface facts before the run:
* Radiation uses `physicoChemical::sigma`.  In OpenFOAM-ESI v2506 `sigma = (pi^2/60) k^4 / ((h/2pi)^3 c^2)` with
  `k = 1.38065e-23`, `h = 6.62607e-34`, `c = 2.99792e8` (etc/controlDict:1098-1115), computed 5.6704085580608236e-08,
  i.e. `+6.02e-6` relative to the CODATA 2018 value 5.670374419e-8 the port uses.  Same class as the gas-constant finding
  of round 1.  The driver emits both (`_Foam`, `_2018`).
* Upstream's pressure conversion is `interfaceP/1e4/9.8`, the port's `interfaceP/(1e4*9.8)`.

Predictions:
1. Gas conductivity `kappa`, accommodation `a`, `hGas`, `hContact` and `hRad_2018`: G1/G2 hold with
   scales derived in the driver (`kappa`: 8 eps x value x 2, from ~16 elementary operations per pair term each <= 1 ulp;
   `hGas`: `kappa` cond + `1 + (d_gap + d_eff + 1.397e-6)/den` for the `max(.., 0)` subtraction).
2. `hRad_Foam` differs from the port by exactly `+6.02e-6` relative (bound `|dsigma/sigma|`, first order exact since
   `h_rad` is linear in sigma): observed within 2 eps of 6.0206e-6.
3. Contact conductance: 100 % bit-identical except through `interfaceP` conversion and `sqrt`/`pow` differences;
   `Prel` branch edges (0.003, 9e-6, 0.0087) agree.
4. Mixture rules: no discrepancy for species sets without Ne/Rn.  A discrepancy is expected NOT.
5. No port defect expected.

## F. Creep: MATPRO UO2 fuel creep rate and Newton derivative, power law (before the driver)

Compared: `MatproCreepModel::correctCreep` rate `fTrial = c1 + c2 + c3` (each already multiplied by
`sigmaEff`) and, for `NewtonRaphsonMethod`, `fTrialDev`; `powerLaw::correctCreep` rate
`B (sigmaEff 1e-6/sigmaC)^n` (per hour upstream; the port is per second, so the driver emits the value / 3600).
Not compared: Limback, Tobbe, Malygin, Routbort, PARFUME, Zhang and the other creep classes (not ported or stateful
in the strain history; listed), the implicit time-integration loop and relaxation.

Fixed before the run:
* The port does not reproduce upstream's lower clamp `sigmaEff >= 1e5 Pa` (documented in the port) and refuses
  `D <= 90.5 %TD`; the grid uses `sigmaEff >= 1e5` and `D > 90.5`, so those do not enter.
* `F = heatSource/312e-13` is the input `fission_rate` of the port (driver: `fission_rate = Q/312e-13`).
* `D = rho/10970*100` upstream; the port takes `(1-p)*100`: the driver passes `rho = 10970*(1-p)`.

Predictions:
1. G1/G2 (scales: the sum of the three positive terms, `exp(-Q/R/T)` with `|Q/(R T)|` of 10-60 amplifying
   the relative error of the exponent: scale = value x (3 + max |Q/(R T)|)) hold for the rate and the derivative.
2. Association differences (`c2` ordering `(A4/(A6+D))*sigma^4.5*exp(..)` upstream vs `(A4/(A6+d)*exp(..))*q^4.5`
   in the port; `-Q/R/T` vs `-q/(R t)`) give 1-3 ulp on a fraction of rows: bit-identity < 100 %, > 40 %.
3. Power law: `pow(sigma*1e-6/sigmaC, n)` against `x.r_powf(n)` with the same `x`: >= 90 % bit-identical.
4. No port defect expected.
