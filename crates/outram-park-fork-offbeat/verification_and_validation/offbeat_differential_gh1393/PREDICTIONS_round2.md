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
