# `cht_multi_region_foam` vs upstream chtMultiRegionFoam (gh:#1363, row 4)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## Which upstream, and what exactly was run
ESI **OpenFOAM-v2506** `applications/solvers/heatTransfer/chtMultiRegionFoam`.
`driver/chtDump.C` is `chtMultiRegionFoam.C` with `createMeshes.H`,
`createFluidFields.H`, `createSolidFields.H`, `setRegionFluidFields.H`,
`setRegionSolidFields.H`, `solveFluid.H` (the `frozenFlow` branch), `fluid/EEqn.H`
(h-form) and `solid/solveSolid.H` transcribed in order, on the **unmodified**
ESI libraries: the real `rhoThermo::New` (`heRhoThermo`, `const` transport,
`hConst`, `rhoConst`, `sensibleEnthalpy`), the real `solidThermo::New`
(`heSolidThermo`, `constIso`), the real laminar `compressible::turbulenceModel`,
the real `radiationModel` (none), `fv::options`, and the real coupled boundary
condition `compressible::turbulentTemperatureRadCoupledMixed` (`mappedWall`
patches, `nearestPatchFace`, `kappaMethod fluidThermo` / `solidThermo`).

Substitutions, all of them (**it is not the unmodified binary**):
1. `rhoReactionThermo` / `CombustionModel` (libcombustionModels not built) become
   `rhoThermo::New`; one pure fluid, no composition, no `YEqn`.
2. MRF, `pressureControl`, `UEqn`, `pEqn`, `rhoEqn` and the PISO loop are omitted:
   the fluid has `frozenFlow yes`, so `solveFluid.H` runs only `EEqn.H`. **The fluid
   is at rest (U = 0)**; the `div(phi,h)`, `K` and `dpdt` terms are in the
   transcribed equation but are identically zero. Convection and the buoyant,
   compressible flow solve of chtMultiRegionFoam are therefore **not** compared.
3. The coupled-energy assembly, the `energyCoupling` loops and the Courant /
   diffusion-number time-step control are omitted (not used; fixed dt;
   `nOuterCorrectors 1`).
4. `libthermoTools` is not built (needs `incompressibleMultiphaseSystems`). The
   two BCs it holds that the case needs are compiled into a scratch library
   `libchtBCs` (`scripts/build_cht_bcs.sh`, 23 s) from the unmodified sources,
   with ONE edit: `temperatureCoupledBase.C` loses its `multiphaseInterSystem`
   include and the one block of `kappa()` serving `kappaMethod phaseSystem`
   (`scripts/build_closure/temperatureCoupledBase.diff`).

**Case: not the shipped tutorials.** `multiRegionHeater` is buoyant, has five
regions, `k-epsilon` and `GAMG`; it needs the compressible pressure solve and
turbulence wall functions, which the port's incompressible `PimpleFoam` +
`EnergyEquation` does not represent. The smallest equivalent used instead is
two regions with the tutorial's own `bottomWater` (rho 1000, Cp 4181, mu 959e-6,
Pr 6.62) and `heater` (rho 8000, Cp 450, kappa 80) properties: fluid
x in [0, 20 mm] (20 cells, graded x2 toward the interface), solid
x in [20, 30 mm] (10 cells, graded x0.5 away from it), both 5 cells over
y in [0, 10 mm], one cell over z in [0, 1 mm] (`empty` front and back); fluid
left wall fixed 300 K, solid right wall fixed 360 K, `walls` adiabatic, both
regions start at 300 K; Euler ddt, `Gauss linear corrected` (orthogonal mesh),
`PBiCGStab`/`PCG` at tolerance 1e-15, fixed dt = 0.1 s, 50 steps (5 s).
First attempt (fluid wall 360 K, solid wall 300 K) was discarded before any
port run: the fluid's diffusion length in 5 s is 0.8 mm, so the interface never
moved and the case would have tested nothing; the hot wall was moved to the solid.

## Method
`tests/upstream_cht_gh1363.rs` reads the committed region polyMeshes and
`cht/reference/cht_v2506.txt.gz` (bit patterns of T and h in every cell of both
regions, the interface-patch face temperatures, the heat into each region
through the interface, and upstream's own kappa / Cp / rho, at every step), builds
the port from the same meshes with those constants, runs 50 steps and compares.

## Predictions (written 2026-11-12, BEFORE the port was run)
Read: `chtMultiRegionFoam.C`, `solveFluid.H`, `EEqn.H`, `solveSolid.H`,
`turbulentTemperatureRadCoupledMixed...C` (`updateCoeffs`),
`mixedEnergyFvPatchScalarField` (the `he` BC derived from the `T` BC), and the
port's `cht_multi_region_foam/mod.rs` and `energy.rs`.

1. **Same equation, different variable.** Upstream solves
   `ddt(rho,h) - laplacian(kappa/Cp, h) = 0` for `h`; the port solves
   `ddt(T) - laplacian(kappa/(rho Cp), T) = 0`. With constant Cp, rho the two are
   the same linear system up to a factor `rho Cp`, and `h = Cp (T - Tstd) + const`.
   So **not bit-identical** is expected: T agrees to **<= 1e-12 K at every cell and
   step** (round-off of `h ~ 2.4e5 J/kg` is `~2e-11` J/kg, i.e. `~6e-15` K, plus
   two different linear solvers both converged to 1e-14). Fewer than 10 % of
   values bit-identical.
2. **Interface BC.** With no layers, `KDeltaC = GREAT`, so upstream's
   `alpha = kappa (1 + KDeltaNbr/GREAT) deltaCoeffs` equals `kappa deltaCoeffs`
   exactly and `valueFraction = KDeltaNbr/(KDeltaNbr + KDelta)` is the formula the
   port ports from Foundation's `turbulentTemperatureCoupledBaffleMixed`. Face
   temperatures on both sides agree to **<= 1e-12 K**. **No ESI-form option is
   needed** if this holds; if it fails the first divergent stage is step 1's
   interface face temperature.
3. **Heat flux.** `Qin` of each region (sum of `kappa magSf snGrad(T)`): step 1
   fluid `Qin` is **exactly 0** on both sides (the fluid is solved first against
   the solid's initial 300 K); the two regions' `Qin` sum to **<= 1e-9 relative**
   (the lagged Gauss-Seidel leaves the fluxes unequal at O(dt)), and port vs
   upstream agrees to **<= 1e-9 relative** at every step.
4. **Ordering.** With `energy_outer_correctors = 1` the port's order (interfaces
   from the neighbour's current field, fluid, then solid) is upstream's
   `nOuterCorrectors 1` order. An ablation with 30 outer passes (the port's
   usual converged use) must differ from upstream's single pass by **> 1e-4 K**
   in the interface cell at step 1 to 5 (upstream is not converged within a
   step); if it did not, point 4 of the module docs would be untested.
5. **Where the port could first diverge, in order:** thermo constants (checked
   against upstream's dumped kappa/Cp/rho first), step-1 interface face
   temperature (the BC), step-1 cell T of the fluid, then of the solid.

## Results (2026-11-12; `cargo test --release -p outram-foam-appbuilder-lib --test upstream_cht_gh1363`)
Port: `ChtMultiRegionFoam` with `energy_outer_correctors = 1`, linear solvers
1e-16, upstream's own kappa / Cp / rho fed in (fluid Cp 4181, rho 1000, kappa =
Cp mu / Pr from upstream; solid kappa 80, Cp 450, rho 8000). Upstream: solver
tolerance 1e-15 (re-run at 1e-19: the reference moves by 2.8e-13 K, so the
reference is converged). 50 steps, 150 cells, 7500 cell values.

| quantity | worst abs(port - upstream) over 50 steps | prediction | verdict |
|---|---|---|---|
| fluid cell T | 1.05e-11 K (step 1: 1.1e-13) | <= 1e-12 K | **missed 10x** |
| solid cell T | 4.2e-12 K (step 1: 2.3e-13) | <= 1e-12 K | **missed 4x** |
| interface face T (implied by the BC coefficients and the new cell T) | 2.7e-11 K | <= 1e-12 K | **missed 27x** |
| heat into each region through the interface | 7.7e-13 of the larger flux | <= 1e-9 relative | held |
| bit-identical cell values | 1437 of 7500 (19 %) | < 10 % | missed (more identical than predicted) |
| step-1 fluid heat | upstream exactly 0; port 6e-32 W | exactly 0 | missed by round-off |
| 30 converged passes vs upstream's one pass, steps 1-5 | cells 0.069 K, faces 1.73 K | > 1e-4 K | held |

**Withdrawn prediction.** The two regions' fluxes are NOT equal within a step.
At step 50 upstream's fluid takes +0.30019 W and the solid gives -0.30253 W
(0.8 % apart): the lagged block Gauss-Seidel of `nOuterCorrectors 1`. The port
has the same imbalance to 7.7e-13. Prediction 3 wrote "sum to 1e-9", which
contradicts point 4 of the same list; it is withdrawn. The comparison that
matters is port against upstream.

**Why 1e-11 and not 1e-12 (explained from evidence, not tuned).** It is not the
linear solvers: the port at 1e-14, 1e-16 and 1e-18 gives the same 1.05e-11 at
step 50, and upstream at 1e-15 and 1e-19 differs by 2.8e-13. Step 1 of the fluid
is 100/100 bit-identical at port tolerance 1e-14; the error then grows roughly
linearly with the step (1e-13 at step 1, 3e-13 at step 3, 1e-11 at step 50), a
bias of a few ulp(300 K) = 5.7e-14 K per step. The port stores T (about 300, ulp
5.7e-14 K) and updates it every step; upstream stores h = Cp (T - 298.15)
(about 1e4 J/kg, ulp 1.8e-12 J/kg = 4e-16 K) and converts to T once. Two
correct discretisations of one equation with different primary variables differ
by this representation floor. **Not verified beyond the evidence above** (it
would take an extended-precision solve of the T-form system).

**Found while comparing (an instrument issue, not a port defect).** Upstream's
stored `T.boundaryField()` at the interface lags the BC coefficients by one step
(step 1 reads 300.0 on both sides although the solid already carries a flux).
The first comparison used it and showed a 2.15 K "difference" that was entirely
that lag. The face temperature compared above is the one the BC coefficients
imply with the new cell value, `Tf = vf refValue + (1 - vf)(Tc + refGrad/deltaCoeffs)`,
dumped as `Tf`; the stored value is also in the reference as `Tb`.

**Defects in the port: none found at this scope.** The port's coupled condition
(`CoupledTemperature::update_coeffs`, Foundation's
`turbulentTemperatureCoupledBaffleMixed`) and ESI's
`turbulentTemperatureRadCoupledMixed` give the same face temperatures and
fluxes, to the floor above, on a conformal, orthogonal, graded interface with no
contact layers; `KDeltaC = GREAT` makes the Rad form's `alpha` equal to
`kappa deltaCoeffs`, as predicted. **No ESI-form option was added**, because
nothing differs here. No number moved; no tolerance was relaxed (the pin is the
first measurement with 2x margin).

## Not covered
Flowing fluid (`div(phi,h)`, `K`), buoyancy and the compressible pressure
equation (the port's fluid is incompressible), turbulence (`alphat`),
`thicknessLayers` / `kappaLayers` (the Rad form's `KDeltaC` path), `thermalInertia`
and radiation, `nOuterCorrectors > 1` with the `energyCoupling` loop and
`coupledEnergyField`, non-orthogonal or non-conformal interfaces, variable
properties (`polynomial`, `sutherland`), and the shipped `multiRegionHeater`
tutorial.
