# `melt_foam` vs ESI v2506 `solidificationMeltingSource` (gh:#1363, row 6)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## Is there an ESI counterpart? (checked 2026-11-12)
**There is no melting or solidification SOLVER in ESI v2506**, and there is none in
the Foundation line either (`melt_foam` itself says it is not a port of any
application). What ESI has is the `solidificationMeltingSource` **fvOption**
(`src/fvOptions/sources/derived/solidificationMeltingSource`, in the built
`libfvOptions`; type name `solidificationMeltingSource`), attached to a solver's
equations by `fvOptions`. Searched `src`, `applications` and `tutorials` for
`solidificationMelting`: the only tutorial that uses it is
`heatTransfer/chtMultiRegionFoam/windshieldDefrost` (the solid `ice` region,
Tmelt 273 K); the VoF variant `VoFSolidificationMeltingSource` is under
`compressibleInterFilmFoam`. (The first attempt used the Foundation type name
`solidificationMelting` and failed with "Unknown fvOption type"; recorded because
it is also a doc point: the ESI name has the `Source` suffix.)

So the comparison is **the option's energy contribution inside a solid's energy
equation**, which is exactly how `windshieldDefrost` runs it. The momentum
contribution (Darcy drag, Boussinesq buoyancy) is not exercised: a solid region
has no momentum equation and ESI ships no incompressible buoyant solver that
takes `fvOptions` and is small enough to build here (`buoyantFoam` is compressible,
`p_rgh`-based).

## What was run
`driver/meltDump.C`: `chtMultiRegionFoam`'s `solid/solveSolid.H` transcribed (as in
`chtDump.C`) on ONE region, on the **unmodified** ESI libraries: the real
`solidThermo::New` (`heSolidThermo`, `constIso`, `hConst`, `rhoConst`,
`sensibleEnthalpy`) and the real `fv::options` reading the real option.
`hEqn: ddt(betav rho, h) - laplacian(betav alpha, h) == fvOptions(rho, h)`.
Omitted: radiation, anisotropic coordinate systems; `betav = 1`; hex dumps for
`runTime.write()`. **It is not the unmodified chtMultiRegionFoam binary** (which
is not built: `thermoTools`, `regionFaModels`, `combustionModels` are missing).

**Case.** A 1-D slab, 40 cells over 20 mm (1 mm x 1 mm cross-section, `empty`
sides), gallium-like properties (rho 6093, Cp 381.5, kappa 32; `L` 80160 J/kg,
`Tmelt` 302.78 K, `relax` 0.9, `beta` 1.2e-4 (unused: no momentum equation)),
initial 301 K, hot wall fixed 311 K, far wall adiabatic, Euler ddt, fixed
dt = 0.5 s, 200 steps (100 s), `PCG`/`DIC` at 1e-15. The Stefan number is
`Cp (311 - 302.78)/L = 0.039`.

## Method
`tests/upstream_melt_gh1363.rs` reads the committed polyMesh and
`melt/reference/melt_v2506.txt.gz` (bit patterns of cell T, h and the option's
liquid fraction `alpha1` at every step, and upstream's kappa / Cp / rho / alpha),
builds `MeltFoam` with a `SolidificationMelting` model with
`solidus = liquidus = Tmelt`, `eutectic_fraction = 0` (so the effective liquidus is
`max(Tsol, Tsol + 0) = Tmelt` exactly, ESI's single melting point), zero gravity
and `beta = 0`, runs 200 steps and compares.

## Predictions (written 2026-11-12, BEFORE the port was run)
Read: `solidificationMeltingSource.C` (`update`, `addSup`, constructor),
`solidificationMeltingSourceTemplates.C` (`apply`), the port's
`fv_options/solidification_melting.rs` (`update`, `add_energy_source`) and
`melt_foam/mod.rs` (`step`).

1. **Same update.** ESI: `alpha1 += relax*Cp*(Tc - Tmelt)/L`, clamp to [0, 1],
   evaluated once per time step from the T of the previous step at the first
   assembly. Port: `alpha += relaxation*Cp*(T - effective_liquidus)/L`, once per
   step. With `Tsol = Tliq` and `alpha1e = 0` they are the same expression in the
   same operation order. **`alpha1` agrees to <= 1e-12 at every cell and step**, and
   the set of cells at exactly 0 and at exactly 1 is **identical at every step**.
2. **Same latent term.** ESI: `eqn -= L*fvc::ddt(rho, alpha1)` in `h`; port:
   `source -= V*(L/Cp)*rho*(alpha - alpha_old)/dt` in `T`. Same equation divided by
   `rho Cp`. **T agrees to <= 1e-10 K** over 200 steps: the conduction floor
   measured in row 4 (1e-11 K after 50 steps, T-form against h-form) plus a
   gain of `relax Cp/L = 0.0043` per K from T into `alpha1`, which is below 1.
3. **Not bit-identical** (T-form against h-form, as in row 4): fewer than 25 % of
   values bit-identical.
4. **No ESI-form option is needed**: the Foundation form reduces to ESI's when
   `Tsol = Tliq`. If the numbers disagree, the first divergent stage is step 1's
   `alpha1` (the update) before T.
5. **Doc claim to check:** `SolidificationMeltingCoefficients::liquidus` says
   "Must exceed `solidus`". The code has no such check, and `Tsol = Tliq` is the
   ESI-equivalent configuration; if the run works, that doc claim is stronger than
   the code and is corrected in this change.

## Results (2026-11-12; `cargo test --release -p outram-foam-appbuilder-lib --test upstream_melt_gh1363`)
Port: `MeltFoam` + `SolidificationMelting` (`solidus = liquidus = 302.78 K`,
`eutectic_fraction = 0`, no gravity), temperature solver 1e-16, thermal
diffusivity `kappa/(rho Cp)` from upstream's own constants. 200 steps, 40 cells,
16 000 compared values (T and `alpha1`).

| quantity | worst abs(port - upstream), 200 steps | prediction | verdict |
|---|---|---|---|
| cell T | 3.4e-11 K | <= 1e-10 K | held |
| `alpha1` | 2.0e-12 | <= 1e-12 | **missed 2x** |
| cells at exactly 0 / exactly 1 | same count at all 200 steps | identical | held |
| bit-identical values | 1874 of 16 000 (11.7 %) | < 25 % | held |
| step-1 `alpha1` | 0 on both (T = 301 K < Tmelt, clamped) | (stage check) | no divergence |

**First run was wrong, and it was my input, not the port.** The first run gave
T off by 9.9 K and `alpha1` off by 1.0 from step 1. The first divergent stage
was step-1 T (the port's cells were nearly uniform, i.e. far too diffusive), and
the cause was that I passed upstream's dumped `thermo.alpha()` (= kappa/Cp =
0.0839 kg/(m s), the `alphahe` of the h-form equation) to the port as its
thermal diffusivity (m^2/s). The port's `alpha_thermal` is `kappa/(rho Cp)`; with
that, step-1 T agrees to 2e-13. No port code changed for this.

**Why `alpha1` is 2e-12 rather than 1e-12.** The same T-form against h-form
floor as row 4 (about 3e-11 K in T), multiplied by the update gain
`relax Cp/L = 0.0043` per K, gives about 1e-13 per step, and `alpha1` accumulates
that over the steps it spends in the mushy range; 2e-12 is that accumulation.
Not isolated further.

**Defects in the port: none found at this scope.** The port's liquid-fraction
update, once-per-step guard, and `-(L/Cp) rho d(alpha)/dt` source reproduce
ESI's `update()` and `apply()` over 200 steps including the clamps at 0 and 1.
**No ESI-form option was added**: with `solidus == liquidus` the Foundation form
IS the ESI form. No number moved; no tolerance was relaxed.

**Doc claims corrected in this change** (both contradicted by the run and by the
code, which has no such check): `SolidificationMeltingCoefficients::liquidus`
("Must exceed `solidus`") and `MeltFoam::boussinesq_coefficients` ("`liquidus`
must exceed `solidus`; a pure metal needs an artificial mushy interval"),
struck through with a dated correction. The generated API mirrors were
regenerated.

## Not covered
The momentum side of the option (Darcy drag `-Cu (1-alpha)^2/(alpha^3 + q)`,
Boussinesq buoyancy), which needs a flow solver taking `fvOptions` (ESI ships
none that is incompressible; `buoyantFoam` is compressible), and the unit
convention of that term (the port divides `Cu` by rho for its kinematic
equation; upstream does not). Also not covered: temperature-dependent Cp,
`thermoMode lookup`, a cell-set selection, the h-form of the port's source
(`energy_is_temperature = false`), and the shipped `windshieldDefrost` tutorial.
