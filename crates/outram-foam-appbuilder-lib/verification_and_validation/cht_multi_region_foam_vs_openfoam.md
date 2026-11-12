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
