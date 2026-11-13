# Predictions, written before any run (gh:#1434, 2026-11-13)

Committed BEFORE any driver was compiled or run and before the Rust comparison
test existed.  Pinned upstreams (all three verified to be the exact pins in
`docs/upstream-provenance.md` by `git log` after fetching the pinned commit):
PRISMS-Plasticity `ffdf4eb6`, PRISMS-Fatigue `2c8fc9a2`, MOOSE `fe80d24f`.

## Route (decided from feasibility, not from a result)

* MOOSE and deal.II/PRISMS-Plasticity were NOT built.  Disk was 6.7-7.6 GB
  free with a 5 GB floor; conda `moose` + `dealii` + p4est/trilinos exceed
  the ~2 GB allowance and would not match the pinned commits (conda carries a
  release, the pins are 2026-08/09 heads).  Not attempted, so not measured.
* PRISMS-Fatigue is plain Python: the real upstream functions
  `calculate_FIPs.import_PRISMS_data` and `volume_average_FIPs.Al7075_band_averaging`
  are IMPORTED AND RUN unmodified from the pinned tree (a true run, not a
  transcription), on synthetic CSVs, inside a throwaway venv.
* PRISMS-Plasticity and MOOSE: transcription route.  Standalone C++ drivers
  copy the kernels' arithmetic lines verbatim (marked `UPSTREAM`, checked by
  `check_verbatim.sh`) with a minimal shim for `FullMatrix` / `RankTwoTensor`.
  Nothing here ran deal.II or libMesh/Eigen; where upstream goes through them
  (Eigen mat-vec in `RankFourTensor * RankTwoTensor`) the summation order is
  NOT reproduced, and that is stated where it matters.
* The issue's case 1 (full single-crystal material-point driver) is NOT run.
  Reading upstream shows its predicted "< 1e-10 if only sub-stepping and line
  search differ" premise is wrong: the port is a SMALL-STRAIN additive-split,
  semi-implicit-hardening model with no backstress, upstream is
  finite-deformation (F = Fe Fp, exponential Fp update, Ohno-Wang backstress,
  `numberOfCuts`, an outer fixed point on s).  No bound below 1e-10 can be
  derived for a step, only an O(strain) one.  The honest differential is
  therefore per KERNEL (slip tables, Rodrigues, stiffness rotation, resolved
  shear, power-law increment, hardening); the full-model transcription is a
  filed child issue.

## Case 2: Fatigue FIP (run of the real Python)

Inputs: 3x3x3 elements, 12 systems, slips ~1e-3, normal stress +-600 (MPa as
upstream), k = 10, sigma_y = 517, both signs of normal stress so the clip is
exercised.  Row positions are on an integer grid.

P2.1 amplitude `|tens - comp| * 0.5`, clip and abs: port == upstream
**bit-identical, 100 %** (multiplication by 0.5 is exact).
P2.2 FS FIP: upstream is `slip * (1 + k * (n / sigma_y))`; the port computes
`|amp| * (1 + k * n / sigma_ref)` = `(k*n)/sigma_ref`.  Different association,
so BEFORE any fix I predict **not** 100 % bit-identical: between 10 % and 50 %
of the points with n > 0 differ, worst **<= 2 ulp (4.4e-16 relative)**.  Points
with n <= 0 (clipped) are identical.  This is a port transcription defect
(operation order), fixed by translating upstream's parenthesisation; AFTER the
fix I predict **100 %** bit-identical.
P2.3 band average, equal unit weights (w = 1.0, every product `w*f` exact and
`volume = n` exact): **bit-identical 100 %**, same winning system.
P2.4 band average with an equal non-trivial weight (w = 3.7e-18 m^3): the port
forms `sum(w f)/ (n w)` where upstream forms `sum(f)/n`.  Predicted: differs by
<= 2 ulp on some points, same winning system.  This is a documented
generalisation (unequal element volumes), not a defect; counted and pinned.
P2.5 pandas' default CSV float parser is not correctly rounded; the driver
dumps the values pandas actually loaded, and counts how many differ from the
correctly rounded parse (informational; expected a small nonzero count at
17 significant digits).

## Case 3: MOOSE J2 radial return (transcription)

`RadialReturnStressUpdate::updateState` + `IsotropicPlasticityStressUpdate`
(linear hardening) + `SingleVariableReturnMappingSolution::internalSolve`, E =
200 GPa, nu = 0.3, sigma_y0 = 250 MPa, H = 1 GPa, 60 incremental steps of a
combined axial/shear strain path with a reversal (so elastic, plastic loading,
unloading and reverse yield all occur).  MOOSE is incremental (elastic strain
old + increment); the port is total-strain with stored plastic strain.
Different algebra, same maths, plus Eigen mat-vec order not reproduced.
P3.1 Not bit-identical; expected agreement ~1e-14.  **Gate (issue): worst
|d stress| / max|stress| < 1e-12**, worst |d eps_p| / max|eps_p| < 1e-12,
|d p| (equivalent plastic strain) relative < 1e-12.  Derivation of the
expected size: ~40 flops per step, each u = 1.1e-16, errors in the incremental
route accumulate additively over 60 steps, deviatoric cancellation
condition ~ 3: expect <= 60*40*u*3 ~ 1e-12 worst case, typical 1e-14.
P3.2 Yield onset step identical in both (f_trial > 0 strict in both).
P3.3 MOOSE's Newton on a LINEAR residual converges in one update + one check;
the port's closed-form is equal to it, so no step shows a difference larger
than the round-off estimate.

## Case 1 kernels (transcription of PRISMS-Plasticity)

K1 slip tables.  Upstream reads `slipNormals.txt` / `slipDirections.txt`
(0.577 / 0.707 three-digit values) and normalises by `sqrt(sum sq)`; the port
takes integer Miller indices and normalises the same way.  (a) The port's
`SlipSystem::from_miller` fed the deck numbers: **bit-identical, all 72
components** (same normalisation expression).  (b) The port's integer table vs
the deck path: not identical, worst **<= 2 ulp (4.4e-16)**, because
0.577/sqrt(3*0.577^2) is not exactly 1/sqrt(3).  Ordering identical (the
dot-product with the deck's m, n matches system by system).
K2 `odfpoint` vs `Orientation::from_rodrigues`, 200 Rodrigues vectors
including |r| ~ 50: **100 % bit-identical** (same expression order; `2.0*r_i*r_j`
differs from `2.0*(r_i*r_j)` only by an exact power-of-two scaling).
K3 `elasticmoduli` (Bond N C N^T) vs `rotate_stiffness`, Cu constants
170/124/75 GPa, 200 orientations: different contraction order.  Predicted
worst |d C| / 170e9 **<= 1e-14**, bit-identical fraction **< 50 %**; entries
that are structurally zero in both stay exactly zero where the sample-frame
tensor keeps the cubic zero pattern (identity orientation: 100 % identical).
Convention check: a wrong crystal-to-sample vs sample-to-crystal convention
would show O(1), not O(1e-14); predicted absent.
K4 resolved shear stress: upstream `trace(T* mTmult S_unsym)` with
`S = R (m x n) R^T` unsymmetrised vs the port's `sigma : sym(R (m x n) R^T)`.
Predicted worst relative-to-|T| difference **<= 1e-14**, not bit-identical.
K5 power-law increment `delgam_ref * pow(fabs(tau/s), 1/m) * sgn`: only `pow`
differs (glibc vs PETIR `r_powf`).  Predicted **>= 90 % bit-identical, worst
<= 2 ulp**; sign and the exact-zero case identical.
K6 `h_beta = h0 * pow(1 - s/s_sat, a)`: same, **>= 90 % bit-identical, <= 2 ulp**.
K7 hardening update.  Upstream accumulates `s_j = s_j + h_ji |dgam_i|`
sequentially starting from `s_t` and clamps with `>=`; the port forms
`d = sum_b q h |dgam|` first and adds `resistance + d`.  Operation order
differs, so BEFORE the fix I predict **>= 10 % of the (system, step) outputs
differ by 1-2 ulp even where the inputs `h` are bit-identical** (that
localises the cause to the summation order, not to `pow`); AFTER translating
upstream's sequential accumulation, the outputs differ only where `h` already
differed (the `pow` ulp), i.e. the same fraction as K6.
K8 Out of scope (no port counterpart): Ohno-Wang backstress, twinning,
`numberOfCuts` sub-stepping, `lnsrch`, exponential `Fp` update, lattice spin.

## What would falsify these

Any kernel above 1e-13 relative is a port defect to be localised to its first
divergent operation before any fix.  A K3 or K4 miss of O(1) means a
convention error (Voigt shear factor, R vs R^T).
