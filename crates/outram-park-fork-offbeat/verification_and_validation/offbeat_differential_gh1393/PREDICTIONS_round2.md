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

Derived gate (written before measuring).  Most strains are a *difference*
`a(T) - a(Tref)`.  If the port evaluates each operand within 4 ulp of the
upstream operand (round 1's observed worst for the same operations,
`pow(T,3)` vs `t*t*t`, Horner vs expanded sum), then
`|port - upstream| <= 4 * eps * (|a| + |b|)` with `eps = 2^-52`, where
`|a|+|b|` is the *operand scale* the driver emits.  Gate G1: that bound at
every compared point.  Gate G2: relative difference < 1e-14 at every point
where `|result| >= 0.1 * scale` (there the condition number is <= 10, so
G1 implies `4*eps*10 = 8.9e-15 < 1e-14`).  Near `T = Tref` the relative
difference is expected to be large and is reported, not gated.

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
