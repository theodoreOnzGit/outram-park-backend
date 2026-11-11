# `rho_central_foam` vs upstream rhoCentralFoam, shockTube (gh:#1363, row 2)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## Which upstream, and what exactly was run
The port's doc comment cites `applications/solvers/compressible/rhoCentralFoam/`,
which exists only in the **ESI** line (OpenFOAM-v2506 here). The Foundation line
(OpenFOAM-dev 852527377c7e) has no rhoCentralFoam: it became the `shockFluid`
module (`applications/modules/shockFluid`, run through foamRun), which needs the
thermophysical, momentumTransport and fluidThermo libraries, not built here and
not affordable in the disk budget. So the comparison is against **ESI v2506
`rhoCentralFoam.C`**; the Foundation `shockFluid` text was not run and not read
for this record (same Kurganov-Tadmor algorithm by lineage, not checked).

`psiThermo`, the turbulence model and their libraries (libfluidThermophysicalModels,
libturbulenceModels, libcompressibleTurbulenceModels, ... a build of the order of
hundreds of MB and an hour of two-core compile) were NOT built. Instead
`upstream_openfoam_gh1363/driver/rhoCentralDump.C` is rhoCentralFoam.C line for
line (directed interpolation with the real `interpolate()` of
`directionInterpolate.H`, Kurganov flux, `solve(fvm::ddt(rho) + fvc::div(phi))`
etc. with the `diagonal` solver of the tutorial's fvSolution, `e = rhoE/rho -
0.5 |U|^2`, `p = rho/psi`) built on the unmodified upstream libOpenFOAM and
libfiniteVolume, with the thermo classes used directly:
`constTransport<species::thermo<hConstThermo<perfectGas<specie>>, sensibleInternalEnergy>>`,
and a transcription of `hePsiThermo::calculate()` (T = THE(e,p,T), psi) on cells
and boundary faces. **This is not the unmodified upstream binary**: it is the
upstream algorithm with thermo/turbulence plumbing replaced. The shockTube
tutorial has mu = 0 so `inviscid` is true and the viscous branches (muEff, tauMC,
laplacians) are not executed in upstream either; the driver aborts if mu > 0.

Case: v2506 tutorial `compressible/rhoCentralFoam/shockTube` (100 cells,
x in [-5, 5] m, T = 348.432 K / p = 1e5 Pa for x < 0, T = 278.746 K / p = 1e4 Pa
for x >= 0, U = 0, hConst Cp = 1004.5, molWeight 28.96, perfectGas, mu = 0;
`fluxScheme Kurganov`; `reconstruct(rho) vanLeer; reconstruct(U) vanLeerV;
reconstruct(T) vanLeer`) with a FIXED dt = 1e-5 s (instead of the tutorial's
adjustTimeStep maxCo 0.2) so both codes take identical steps; steps 1, 2, 10, 100
and 700 (t = 0.007 s, the tutorial end time) are dumped as IEEE-754 bit patterns
(`scripts/run_shock.sh`; mesh = v2506 blockMesh output committed under
`shock/constant/polyMesh`).

## Method
`tests/upstream_shock_tube_gh1363.rs` reads the committed polyMesh and reference,
sets the port's initial state from the same T and p, runs `RhoCentralFoam` with the
same dt, and compares rho, Ux, T and p per cell at the dumped steps: count of
bit-identical cells and the worst relative difference (against the field's max
|value|, and the worst per-cell relative difference). The port stores `e` and no
`T`: its T is `e / Cv`.

## Predictions (written 2026-11-11, BEFORE the port was run)
Read: upstream `rhoCentralFoam.C`, `directionInterpolate.H`, `readFluxScheme.H`,
`centralCourantNo.H`, `createFields.H`; and the port's `rho_central_foam/mod.rs`.
Differences I can see by reading, each with its expected size:
1. **Hard-coded gamma = 1.4.** Upstream's gamma is Cp/Cv from the case:
   Cv = Cp - R, R = RR/W = 287.1 (dump constants), so gamma = 1.4002..., not 1.4.
   The sound speed `c^2 = gamma R T` differs by about 1.8e-4 relative.
   Predicted effect at step 1: no difference in cells away from the initial jump
   (U = 0, flux = p, which has no gamma in it); in the two cells at the jump a
   relative difference of order 1e-4 in rho/U/T/p (through `a_pos`, `ap`, `am`).
2. **Reconstructed variables differ.** Upstream reconstructs rho, rhoU (vector
   limiter vanLeerV), rPsi (the T scheme), e and `c = sqrt(Cp/Cv*rPsi)` itself;
   the port reconstructs rho, the velocity components (scalar vanLeer) and e, and
   derives p and c from the reconstructed rho and e. Reconstructing `c` is not the
   same as `sqrt(gamma (gamma-1) e_pos)` away from a constant state, and U is not
   rhoU/rho reconstructed. Predicted: at step 1 the initial state is piecewise
   constant, so every limiter is 0 at the jump (r = 0) and the two formulations
   coincide apart from item 1; from step 2 on the difference is real and of order
   the limited slope times the nonlinearity, i.e. **1e-4 to 1e-2 of the field
   range in the wave region at steps 10 to 700**.
3. **Arithmetic order everywhere** (fluxes built as `a_pos*phiv_pos - aSf` in
   upstream against the port's `(a_R F_L - a_L F_R + a_L a_R (W_R - W_L))/(a_R - a_L)`;
   the explicit update `rho - dt*div` against `ddt` + diagonal solve; the port
   recomputing rho*U from U every step where upstream keeps rhoU as a field;
   boundary faces as the physical flux of the owner cell against the KNP formula with
   equal pos/neg states). Predicted: ulp-level (1e-16 relative) differences after
   step 1 on every cell that is not at a wave, in addition to 1 and 2.
4. **Bit-identity of the initial state is not automatic**: upstream `rho = psi*p`
   with `psi = 1/(R T)`; the port's tutorial builds `p/(R T)` (an ulp). The test
   builds the initial state with upstream's order so any difference after step 1 is
   the solver's.
5. **Clamps** (`rho >= 1e-6`, `e >= 0`, `p >= 0`): not active in this problem
   (rho in 0.1 to 1.3, e > 0 in the port's convention). **T**: the port's `e = Cv T`
   differs from upstream's `Es = Cp (T - Tref) - R T`-type offset by a constant,
   which is exact in exact arithmetic (energy is additive) but not in floating point.

Expected outcome, stated plainly: **not bit-identical** (T cannot be, since the port
derives it), step 1 agreement of order 1e-4 at the two jump cells and exact in
the other 98 for rho and U; and a real, formulation-level disagreement of up to
1e-2 of range in the waves later. The fix, if the predictions hold, is to translate
upstream's formulation (items 1 and 2) into `rho_central_foam`.

## Results
(to be filled after the port run)
