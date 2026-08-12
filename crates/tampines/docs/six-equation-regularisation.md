# How OpenFOAM regularises the six-equation two-fluid system

A source study of the vendored OpenFOAM `multiphaseEuler` solver module,
written to tell the next implementer of
`crates/tampines/src/multiphase_1d/two_fluid.rs` exactly which mechanisms
upstream uses, which are physics and which are numerics, and which of them a
1-D reduction has to carry.

> **Status: untrusted AI-assisted draft, no human review.** This is a reading
> of source code, not a derivation and not a validation. Every claim below
> carries a `file:line` so a human can check it against the code rather than
> trusting this document. Nothing here has been run. See the workspace
> `RESPONSIBLE_USE.md` and `VERIFICATION_AND_VALIDATION.md`.

---

## 0. Provenance of the source read

| Item | Value |
|---|---|
| Upstream project | OpenFOAM (OpenFOAM Foundation) |
| Repository | `https://github.com/OpenFOAM/OpenFOAM-dev` |
| Version stamp | `dev-c04e1b9659a7` — from `crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM/.build` |
| `WM_PROJECT_VERSION` | `dev` — `.../OpenFOAM/etc/bashrc:36` |
| README date | 14th July 2026 — `.../OpenFOAM/README.org:5` |
| Licence | GPL-3.0 — `.../OpenFOAM/README.org:19-24`, `COPYING` |
| Vendored at | `crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM/` (read-only reference material) |
| Date read | 2026-08-12 |

The vendored tree **does** contain the `multiphaseEuler` solver module in
complete, buildable form (module sources, `phaseSystem`, interfacial models,
momentum-transport models, population balance, and 30 tutorial cases). This is
not a partial or stubbed vendoring.

**Path abbreviation used below.** Every path prefixed `$ME/` expands to:

```
crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM/applications/modules/multiphaseEuler/
```

and `$OF/` expands to:

```
crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM/
```

---

## 1. The finding, up front

**OpenFOAM's `multiphaseEuler` does not contain an interfacial-pressure term.
There is no Stuhmiller/Bestion $p_{i}$ difference anywhere in the vendored
tree.** A case-insensitive search for `interfacial pressure`, `interface
pressure`, `pInterface`, `Stuhmiller`, `Bestion`, `well-posed` and `ill-posed`
across every `.C`/`.H` in `$OF/` returns no hit in any two-fluid context — the
only `ill-posed` in the whole tree is an unrelated boundary-condition comment
at `$OF/src/finiteVolume/fields/fvPatchFields/derived/supersonicFreestream/supersonicFreestreamFvPatchVectorField.H:61`.

What upstream uses instead splits by flow regime, and **is optional in both
cases** — the solver runs with none of it:

1. **Fluid-fluid (bubbly, gas-liquid): virtual mass.** An inertial coupling
   $K_{vm}(\mathrm{D}U_{1}/\mathrm{D}t - \mathrm{D}U_{2}/\mathrm{D}t)$ folded
   into the *implicit* phase-coupling matrix, not added as an explicit source.
   Every gas-liquid tutorial in the vendored tree enables it with
   $C_{vm} = 0.5$.
2. **Fluid-solid (granular, fluidised bed): a phase pressure**
   $p^{\prime} = \partial p_{\mathrm{phase}} / \partial \alpha$, entering both
   as an explicit face force $p^{\prime}\,\nabla_{n}\alpha$ in the momentum
   balance and as an implicit Laplacian in the phase-fraction equation. Every
   granular tutorial enables it; it is **identically zero** for a fluid phase.

Everything else — the implicit pairwise drag matrix, the MULES-limited bounded
phase-fraction transport, the `residualAlpha` diagonal flooring — is a
*numerical* device. It buys robustness in the solve; it does not restore real
characteristics to the continuous PDE system. Upstream nowhere claims
hyperbolicity and nowhere discusses it.

**Consequence for the 1-D reduction:** virtual mass is the mechanism to
implement, because TAMPINES' target regime (blowdown, flashing gas-liquid) is
the fluid-fluid branch. Granular phase pressure is not applicable. See §7.

---

## 2. What is, and is not, in the phase momentum equation

The per-phase momentum equation is assembled in
`$ME/phaseSystem/phaseModels/MovingPhaseModel/MovingPhaseModel.C:339-354`:

```cpp
Foam::MovingPhaseModel<BasePhaseModel>::UEqn()
{
    const volScalarField& alpha = *this;
    const volScalarField& rho = this->rho();

    return
    (
        fvm::ddt(alpha, rho, U_)
      + fvm::div(alphaRhoPhi_, U_)
      + fvm::SuSp(-this->continuityError(), U_)
      + this->fluid().MRF().DDt(alpha*rho, U_)
      + momentumTransport_->divDevTau(U_)
    );
}
```

Reading this literally, the assembled matrix is

$$\frac{\partial}{\partial t}(\alpha_{k}\rho_{k}U_{k}) + \nabla \cdot (\alpha_{k}\rho_{k}\phi_{k} U_{k}) - \epsilon^{c}_{k} U_{k} + \nabla \cdot \tau^{\mathrm{dev}}_{k}$$

where $\alpha_{k}$ is the phase fraction [-], $\rho_{k}$ the phase density
[kg/m^3], $U_{k}$ the phase velocity [m/s], $\alpha_{k}\rho_{k}\phi_{k}$ the
phase mass flux `alphaRhoPhi_` [kg/s], $\epsilon^{c}_{k}$ the continuity error
[kg/(m^3 s)] (`continuityError()`, defined at `MovingPhaseModel.C:274` as
$\partial_{t}(\alpha_{k}\rho_{k}) + \nabla\cdot(\alpha_{k}\rho_{k}\phi_{k}) - S$),
and $\tau^{\mathrm{dev}}_{k}$ the deviatoric stress.

**Four things are conspicuously absent, and their absence is the architecture:**

- **No pressure gradient.** There is no $-\alpha_{k}\nabla p$ term and no
  $-\nabla(\alpha_{k}p)$ term.
- **No gravity.**
- **No drag.**
- **No virtual mass.**

All four are handled downstream, at *face* level, in the pressure corrector.
Drag and virtual mass go into an implicitly-inverted coupling matrix (§3);
pressure and gravity appear only as face fluxes (§6). This is the single most
important structural fact for a faithful reduction: **upstream never forms a
cell-centred phase momentum equation containing the pressure gradient.**

The face-momentum variant `UfEqn()`
(`MovingPhaseModel.C:357-373`) is the same equation with the `ddt` term moved
out, selected by the `faceMomentum` switch (`$ME/multiphaseEuler.H:86-88`).

---

## 3. The implicit drag + virtual mass coupling matrix

This is the structural heart of the solver and the place both drag and virtual
mass live.

### 3.1 What is assembled

`$ME/phaseSystem/momentumTransferSystem/momentumTransferSystem.C:541-766`
builds an $n \times n$ matrix per cell (and per face), $n$ = number of moving
phases, and inverts it. Its declared purpose, from the header
`$ME/phaseSystem/momentumTransferSystem/momentumTransferSystem.H:195-203`:

```cpp
        //- Return the inverse of the central + drag + virtual mass
        //  coefficient matrix
        void invADVs
        (
            const PtrList<volScalarField>& As,
            PtrList<volVectorField>& HVms,
            PtrList<PtrList<volScalarField>>& invADVs,
            PtrList<PtrList<surfaceScalarField>>& invADVfs
        ) const;
```

**Diagonal seed** — the momentum-matrix central coefficient
(`momentumTransferSystem.C:554-561`):

$$(AD)_{ii} = A_{i}$$

where $A_{i}$ is `UEqns[i].A()`, the diagonal of the phase momentum matrix
[kg/(m^3 s)].

**Drag contribution** (`momentumTransferSystem.C:617-634`):

```cpp
                const volScalarField Kdij
                (
                    (otherPhase/max(otherPhase, otherPhase.residualAlpha()))*Kd
                );
...
                invADVs[i][i] += Kdij;
                invADVfs[i][i] += Kdijf;
...
                    invADVs[i].set(j, -Kdij);
                    invADVfs[i].set(j, -Kdijf);
```

So

$$(AD)_{ii} \mathrel{+}= \tilde{K}^{d}_{ij}, \qquad (AD)_{ij} = -\tilde{K}^{d}_{ij}$$

with

$$\tilde{K}^{d}_{ij} = \frac{\alpha_{j}}{\max(\alpha_{j}, \alpha^{\mathrm{res}}_{j})} K^{d}_{ij}$$

The $\alpha_{j}/\max(\alpha_{j},\alpha^{\mathrm{res}}_{j})$ ratio is a
vanishing-phase guard: it is exactly 1 wherever $\alpha_{j} \ge
\alpha^{\mathrm{res}}_{j}$ and tapers to 0 as $\alpha_{j} \to 0$.

The drag coefficient itself, for a dispersed interface
(`$ME/phaseSystem/interfacialModels/dragModels/dispersedDragModel/dispersedDragModel.C:58-78`):

$$K^{d}_{i} = 0.75\, C_{d}\mathrm{Re}\, C_{s}\, \frac{\rho_{c} \nu_{c}}{d_{d}^{2}}, \qquad K^{d} = \max(\alpha_{d}, \alpha^{\mathrm{res}}_{d})\, K^{d}_{i}$$

Units: $K^{d}$ is [kg/(m^3 s)], matching $A$. $\rho_{c}$, $\nu_{c}$ are
continuous-phase density [kg/m^3] and kinematic viscosity [m^2/s]; $d_{d}$ is
the dispersed-phase diameter [m]; $C_{s}$ is the swarm correction [-].

**Virtual mass contribution** (`momentumTransferSystem.C:704-762`):

```cpp
                const volScalarField VmPhase
                (
                    (otherPhase/max(otherPhase, otherPhase.residualAlpha()))*Vm
                );

                {
                    const volScalarField AVm(VmPhase*ADUDts[i]);

                    invADVs[i][i] += AVm;
                    invADVfs[i][i] += fvc::interpolate(AVm);
...
                const label j = movingPhases[otherPhase.index()];

                if (j != -1)
                {
                    const volScalarField AVm(VmPhase*ADUDts[j]);

                    invADVs[i][j] -= AVm;
                    invADVfs[i][j] -= fvc::interpolate(AVm);
```

so

$$(AD)_{ii} \mathrel{+}= \tilde{K}^{vm}_{ij} A^{\mathrm{D}}_{i}, \qquad (AD)_{ij} \mathrel{-}= \tilde{K}^{vm}_{ij} A^{\mathrm{D}}_{j}$$

where $A^{\mathrm{D}}_{i}$ is the diagonal of the *material derivative* matrix
$\mathrm{D}U_{i}/\mathrm{D}t$, cached at `momentumTransferSystem.C:696-698`
from `phase.DUDt()`. That operator is
(`$ME/phaseSystem/phaseModels/MovingPhaseModel/MovingPhaseModel.C:533-536`):

```cpp
Foam::MovingPhaseModel<BasePhaseModel>::DUDt() const
{
    return fvm::ddt(U_) + UgradU();
}
```

i.e. $\mathrm{D}U/\mathrm{D}t = \partial_{t}U + (U \cdot \nabla)U$, assembled
implicitly, so $A^{\mathrm{D}} \approx 1/\Delta t + \ldots$ [1/s].

The explicit (off-diagonal-of-`H`) remainder of the virtual mass term is
accumulated separately into `HVms` (`momentumTransferSystem.C:734-758`) as
$\tilde{K}^{vm}_{ij}(H^{\mathrm{D}}_{i} - H^{\mathrm{D}}_{j})$, and added to
the momentum `H` in the pressure corrector at
`$ME/cellPressureCorrector.C:246-249`.

**Inversion** (`momentumTransferSystem.C:764-765`, implementation at
`:461-499`): a per-cell dense LU decompose and back-substitute,

```cpp
        // Calculate the inverse of AD using LD decomposition
        // and back-substitution
        LUDecompose(AD, pivotIndices);
```

This is an **exact** $n$-phase generalisation of the two-phase *partial
elimination algorithm*. It is not an approximation and not an iteration.

### 3.2 The virtual mass model itself

Header comment,
`$ME/phaseSystem/interfacialModels/virtualMassModels/dispersedVirtualMassModel/dispersedVirtualMassModel.H:88-98`:

```cpp
        //- Return the phase-intensive virtual mass coefficient Ki
        //  used in the momentum equation
        //    ddt(alpha1*rho1*U1) + ... = ... alphad*K*(DU1_Dt - DU2_Dt)
        //    ddt(alpha2*rho2*U2) + ... = ... alphad*K*(DU1_Dt - DU2_Dt)
        virtual tmp<volScalarField> Ki() const;
```

Implementation
(`.../dispersedVirtualMassModel/dispersedVirtualMassModel.C:51-67`):

$$K^{vm}_{i} = C_{vm}\rho_{c}, \qquad K^{vm} = \max(\alpha_{d}, \alpha^{\mathrm{res}}_{d})\, C_{vm}\, \rho_{c}$$

Units: $K^{vm}$ is [kg/m^3]; multiplied by $\mathrm{D}U/\mathrm{D}t$ [m/s^2] it
gives [kg/(m^2 s^2)] = force per unit volume, correct. $C_{vm}$ [-] is the
added-mass coefficient, $\rho_{c}$ [kg/m^3] the continuous-phase density,
$\alpha_{d}$ [-] the dispersed-phase fraction.

Two implementations exist:

- `constantCoefficient` —
  `.../constantVirtualMassCoefficient/constantVirtualMassCoefficient.C:71-79`,
  returns a user-set uniform $C_{vm}$.
- `Lamb` — `.../Lamb/Lamb.C:69-77`, an aspect-ratio-dependent form:

$$C_{vm} = \frac{\sqrt{1-E^{2}} - E \arccos E}{E \arccos E - E^{2}\sqrt{1-E^{2}}}$$

with $E$ the bubble aspect ratio [-], clipped to $(0,1)$ at `Lamb.C:71`.

A `noVirtualMass` null model exists
(`.../noVirtualMass/noVirtualMass.C`), i.e. **virtual mass is optional**; the
model table is read from the `virtualMass` sub-dictionary of
`constant/momentumTransfer` at `momentumTransferSystem.C:147-152`, and an empty
sub-dictionary is legal.

### 3.3 What the tutorials actually enable

Surveying all 30 vendored cases under
`$OF/tutorials/multiphaseEuler/*/constant/momentumTransfer`:

| Regime | Cases | Virtual mass | Granular phase pressure |
|---|---|---|---|
| Gas-liquid bubbly | `bubbleColumn`, `bubbleColumnLES`, `bubbleColumnLaminar`, `bubbleColumnIATE`, `bubbleColumnEvaporating*`, `bubblePipe`, `aeratedStirredTankMRF`, `mixerVessel2D(MRF)`, `injection`, `condenser`, `wallBoiling*`, `wallCondensation`, `Grossetete`, `steamInjection`, `bed`, `boilingBed` | yes | no |
| Granular / fluidised | `fluidisedBed`, `fluidisedBedLaminar`, `LBend`, `pipeBend`, `titaniaSynthesis*` | mostly no | yes (`kineticTheory`) |
| Neither | `damBreak4phase`, `hydrofoil` | no | no |

The canonical bubbly case,
`$OF/tutorials/multiphaseEuler/bubbleColumn/constant/momentumTransfer`, sets
`SchillerNaumann` drag and:

```
virtualMass
{
    air_dispersedIn_water
    {
        type            constantCoefficient;
        Cvm             0.5;
    }
    water_dispersedIn_air
    {
        type            constantCoefficient;
        Cvm             0.5;
    }
}

lift
{}

wallLubrication
{}

turbulentDispersion
{}
```

$C_{vm} = 0.5$ is the potential-flow value for a sphere. Note that lift, wall
lubrication and turbulent dispersion are all *empty* here — so in the reference
bubbly case, **virtual mass is the only non-drag interfacial momentum term
present**. That is the strongest available evidence in the vendored tree that
virtual mass is doing the regularising work in the fluid-fluid branch.

Two cases (`damBreak4phase`, `hydrofoil`) run with drag alone and no
regularising term at all. Upstream does not flag this as a problem anywhere in
the source.

---

## 4. The phase pressure — the granular branch

### 4.1 Definition and default

`pPrime` is the derivative of a phase's own pressure with respect to its phase
fraction. **The base-class implementation returns zero**, at
`$OF/src/MomentumTransportModels/phaseCompressible/phaseCompressibleMomentumTransportModel.C:99-108`:

```cpp
Foam::tmp<Foam::surfaceScalarField>
Foam::phaseCompressibleMomentumTransportModel::pPrimef() const
{
    return surfaceScalarField::New
    (
        this->groupName("pPrimef"),
        this->mesh_,
        dimensionedScalar(dimensions::pressure, 0)
    );
}
```

(and `pPrime()` likewise at `:87-96`). **This is the single most important
negative fact in this document:** for an ordinary fluid phase — water, steam,
air — with any ordinary turbulence model, $p^{\prime} \equiv 0$ and the
phase-pressure machinery contributes nothing. It is not a generic
well-posedness fix; it is a granular closure.

Only two momentum-transport models override it:

**`phasePressure`** — a pure particle-particle repulsion, documented at
`$ME/momentumTransportModels/phasePressureModel/phasePressureModel.H:28-43`:

```
    Particle-particle phase-pressure RAS model

    The derivative of the phase-pressure with respect to the phase-fraction
    is evaluated as

        g0*min(exp(preAlphaExp*(alpha - alphaMax)), expMax)
```

implemented at `.../phasePressureModel.C:140-169`:

$$p^{\prime} = g_{0}\,\min\left(\exp\left(c_{\alpha}\left(\alpha - \alpha_{\max}\right)\right), E_{\max}\right)$$

Units: $p^{\prime}$ [Pa] (per unit phase fraction, which is dimensionless).
Defaults, from the same header block: $g_{0} = 1000$ Pa, $c_{\alpha} =
\mathtt{preAlphaExp} = 500$ [-], $E_{\max} = \mathtt{expMax} = 1000$ [-].
$\alpha_{\max}$ is the packing limit. The boundary values are forced to zero on
non-coupled patches (`phasePressureModel.C:160-166`).

**`kineticTheory`** — the full granular kinetic-theory closure,
`$ME/momentumTransportModels/kineticTheoryModels/kineticTheoryModel/kineticTheoryModel.C:302-351`:

$$p^{\prime} = \Theta\, \frac{\partial}{\partial \alpha}\left[\text{granularPressureCoeff}\right] + \frac{\partial p^{\mathrm{fric}}}{\partial \alpha}$$

with $\Theta$ the granular temperature [m^2/s^2], the first term from
`granularPressureModel_->granularPressureCoeffPrime(...)` (a function of
$\alpha$, the radial distribution $g_{0}$ and $g_{0}^{\prime}$, $\rho$, and the
restitution coefficient $e$), and the second from
`frictionalStressModel_->frictionalPressurePrime(...)`. The composition is
transcribed exactly from the source; the sub-model internals were not opened.

In both cases the face value is a plain interpolation of the cell value
(`phasePressureModel.C:205-213`, `kineticTheoryModel.C:354-362`).

### 4.2 Where it enters — two distinct places

**(a) As an explicit face force in the momentum balance**, in the assembly of
the explicit force fluxes `Fs()` at
`$ME/phaseSystem/momentumTransferSystem/momentumTransferSystem.C:287-299` (and
identically in the face-based `Ffs()` at `:405-417`):

```cpp
    // Add the phase pressure
    forAll(fluid_.movingPhases(), movingPhasei)
    {
        const phaseModel& phase = fluid_.movingPhases()[movingPhasei];

        addField
        (
            phase,
            "F",
            phase.pPrimef()*fvc::snGrad(phase)*fluid_.mesh().magSf(),
            Fs
        );
    }
```

i.e. a face force flux

$$F^{pp}_{k,f} = p^{\prime}_{k,f}\,\left(\nabla_{n}\alpha_{k}\right)_{f}\,\left|S_{f}\right|$$

where $\nabla_{n}$ is the surface-normal gradient [1/m] and $|S_{f}|$ the face
area [m^2]. Since $p^{\prime} = \partial p_{k}/\partial \alpha_{k}$, this is
the chain-rule form of $\nabla p_{k}$ — a genuine extra pressure gradient in
the phase momentum balance, carried entirely on faces.

**This is the term that is structurally analogous to an interfacial-pressure
term, and it is the closest thing in the codebase to one — but it is a
granular closure, active only for particulate phases, and zero for a
gas-liquid pair.**

**(b) As an implicit diffusion in the phase-fraction equation.** The
coefficient is built by
`$ME/phaseSystem/momentumTransferSystem/momentumTransferSystem.C:969-1030`:

$$D^{\alpha}_{f} = \sum_{k} \left(\max(\alpha_{k},0)\right)_{f} \left(r A_{k}\right)_{f} p^{\prime}_{k,f} \;+\; \sum_{\text{pairs}} \frac{\alpha_{1f}\alpha_{2f}}{\max(\alpha_{1f}+\alpha_{2f}, \alpha^{\mathrm{res}}_{1})}\left(\max(rA_{1},rA_{2})\,D^{td}\right)_{f}$$

where $rA_{k} = 1/A_{k}$ [m^3 s/kg] is the reciprocal momentum diagonal
(`$ME/cellPressureCorrector.C:95-103`) and $D^{td}$ is the turbulent
dispersion diffusivity. Units of $D^{\alpha}_{f}$: [Pa] $\times$ [m^3 s/kg] =
[m^2/s], a diffusivity, as required.

It is applied twice per corrector: once as an **explicit flux** added to the
high-order phase flux before MULES limiting
(`$ME/phaseSystem/phaseSystem/phaseSystemSolve.C:290-309` and `:500-504`):

```cpp
            alphaPhiDByA.set
            (
                movingPhasei,
                alphaDByAf*fvc::snGrad(alpha, "bounded")*mesh_.magSf()
            );
```

and once as an **implicit deferred correction** after the MULES correct
(`phaseSystemSolve.C:741-768`):

```cpp
                    fvScalarMatrix alphaEqn
                    (
                        fvm::ddt(alpha) - fvc::ddt(alpha)
                      - fvm::laplacian(alphaDByAf, alpha, "bounded")
                    );

                    alphaEqn.solve
                    (
                        mesh_.solution().solverDict("alpha")
                       .optionalSubDict("phasePressure")
                    );

                    alphaPhis[solveMovingPhaseIndices[solvePhasei]] +=
                        alphaEqn.flux();
```

The `fvm::ddt(alpha) - fvc::ddt(alpha)` pair cancels in the converged solution,
so this adds only the diffusion implicitly without changing the transport — the
standard deferred-correction pattern for taking a stiff diffusion term out of an
explicit bounded advection scheme.

This whole path is gated by a per-phase run-time switch
(`phaseSystemSolve.C:48-65`, read from `fvSolution`'s `alpha.<phase>` solver
dictionary):

```cpp
bool Foam::phaseSystem::implicitPhasePressure() const
{
    forAll(phases(), phasei)
    {
        if
        (
            mesh()
           .solution()
           .solverDict(phases()[phasei].volScalarField::name())
           .lookupOrDefault<Switch>("implicitPhasePressure", false)
        )
```

**Default is `false`.** In the vendored tutorials it is set only in the four
granular cases (`fluidisedBed`, `fluidisedBedLaminar`, `LBend`, `pipeBend`).

---

## 5. The other candidate terms

### 5.1 Turbulent dispersion

Enters exactly parallel to the phase pressure — as an explicit face force in
`Fs()`/`Ffs()` (`momentumTransferSystem.C:301-337`, `:419-455`):

$$F^{td}_{1,f} = D_{f}\left(\nabla_{n}\frac{\alpha_{1}}{\max(\alpha_{1}+\alpha_{2}, \alpha^{\mathrm{res}}_{1})}\right)_{f}\left|S_{f}\right|$$

and as the second sum in $D^{\alpha}_{f}$ above. The `Burns` model
(`$ME/phaseSystem/interfacialModels/turbulentDispersionModels/Burns/Burns.C:69-99`):

$$D = K^{d}_{i}\,\frac{\nu_{t,c}}{\sigma}\,\frac{\alpha_{d}\left(\alpha_{d}+\alpha_{c}\right)^{2}}{\max(\alpha_{d},\alpha^{\mathrm{res}}_{d})\max(\alpha_{c},\alpha^{\mathrm{res}}_{c})}$$

with $\nu_{t,c}$ the continuous-phase turbulent viscosity [m^2/s] and $\sigma$
a turbulent Schmidt number [-]. This is a real diffusion of $\alpha$ and *does*
change the principal part of the system, so it is regularising where enabled —
but it is turbulence-driven, vanishes in laminar flow, and is empty in the
canonical bubbly tutorial.

### 5.2 Lift and wall lubrication

Both enter `Fs()`/`Ffs()` as explicit face fluxes only
(`momentumTransferSystem.C:230-285`, `:348-403`). Neither introduces a
derivative of $\alpha$ or of a velocity difference that would alter the
characteristics. They are physics, not regularisation.

### 5.3 The `dragCorrection` device

`$ME/multiphaseEuler.H:90-92` declares a `dragCorrection` switch,
"Cell/face drag correction for cell momentum corrector", defaulting to false.
Implementation at `momentumTransferSystem.C:1128-1190`: it forms
$\tilde{K}^{d}_{ij}(U_{j} - U_{i})$ from the *reconstructed* fluxes and the
corresponding face difference, and at `$ME/cellPressureCorrector.C:408-438`
subtracts the cell form while adding the reconstructed face form:

```cpp
                        phase.URef() =
                            HbyADs[movingPhasei]
                          + fvc::reconstruct
                            (
                                alphaByADfs[movingPhasei]*mSfGradp
                              - FgByADfs[movingPhasei]
                              + dragCorrByADfs[movingPhasei]
                            )
                          - dragCorrByADs[movingPhasei];
```

This is a cell/face consistency correction for the drag — a discretisation
device, not physics.

### 5.4 `residualAlpha` — vanishing-phase flooring

Declared at `$ME/phaseSystem/phaseModel/phaseModel.H:70-72`:

```cpp
        //- Return the residual phase-fraction for given phase
        //  Used to stabilise the phase momentum as the phase-fraction -> 0
        dimensionedScalar residualAlpha_;
```

Typical value $10^{-6}$ (e.g. `bubbleColumn/constant/phaseProperties:30,43`).
It appears in three distinct roles:

1. **Diagonal augmentation** — `$ME/cellPressureCorrector.C:82-91`:

```cpp
            As.set
            (
                movingPhasei,
                UEqns[phase.index()].A()
              + byDt
                (
                    max(phase.residualAlpha() - alpha, scalar(0))
                   *phase.rho()
                )
            );
```

i.e. $A_{k} \leftarrow A_{k} + \max(\alpha^{\mathrm{res}}_{k}-\alpha_{k},0)\rho_{k}/\Delta t$.
This is zero wherever $\alpha_{k} > \alpha^{\mathrm{res}}_{k}$ and floors the
diagonal to a $\Delta t$-scaled inertia as $\alpha_{k} \to 0$, keeping $AD$
invertible. `byDt` is $1/\Delta t$, or the local reciprocal timestep under LTS
(`$ME/phaseSystem/phaseSystem/phaseSystem.C:945-955`). The matching source term
$\max(\alpha^{\mathrm{res}}-\alpha,0)\rho\,U^{n}/\Delta t$ is added to $H$ at
`cellPressureCorrector.C:238-243`, so the augmentation relaxes $U_{k}$ towards
its old value rather than towards zero.

2. **Drag/virtual-mass tapering** — the
   $\alpha_{j}/\max(\alpha_{j},\alpha^{\mathrm{res}}_{j})$ ratios in §3.1.

3. **Denominator guards** throughout the interfacial models.

All three are numerical. Note the face-momentum variant uses a slightly
different, non-vanishing form — `$ME/facePressureCorrector.C:77-85` adds
$\max(\alpha^{n}_{k}, \alpha^{\mathrm{res}}_{k})\rho^{n}_{k}/\Delta t$
unconditionally, not the `max(residual - alpha, 0)` difference.

### 5.5 MULES-limited bounded phase-fraction transport

The $\alpha$ equation is solved by a bounded explicit scheme with a
flux-corrected-transport limiter. In `$ME/phaseSystem/phaseSystem/phaseSystemSolve.C`:

- Optional **semi-implicit predictor** (`MULESCorr`, `:327-373`) using
  first-order **upwind** on the mean flux:

```cpp
                  + fv::gaussConvectionScheme<scalar>
                    (
                        mesh_,
                        phiMoving,
                        upwind<scalar>(mesh_, phiMoving)
                    ).fvmDiv(phiMoving, alpha)
```

- High-order flux plus optional interface compression `cAlpha` (`:428-498`).
- `MULES::limitCorr` per phase (`:563-575`), then `MULES::limitSumCorr` across
  phases to enforce $\sum_{k}\alpha_{k} = 1$ (`:580`), then `MULES::correct`
  (`:593-599`).
- Sub-cycling `nAlphaSubCycles` (`:311-319`), Courant-adaptive via
  `alphaControl::correct(CoNum)` (`$ME/phaseSystem/phaseSystem/phaseSystem.H:81-117`).
- Optional final clipping and re-scaling to sum to 1 (`:820-`).

The upwind predictor carries first-order numerical diffusion of $\alpha$; the
limiter enforces boundedness. **Both are numerical regularisation.** They stop
$\alpha$ from going unbounded when the continuous system tries to grow a
short-wavelength instability, but they do so by damping the discrete solution,
not by fixing the PDE. A converged, mesh-refined solution of an ill-posed
system does not exist to converge to — this is the honest characterisation of
what MULES buys.

---

## 6. The pressure/momentum solution procedure, in order

Outer driver, `$OF/applications/solvers/foamRun/foamRun.C:122-198`:

```
while (pimple.run(runTime))
    solver.preSolve()
    runTime++
    while (pimple.loop())                       // PIMPLE outer corrector
        solver.moveMesh(); solver.motionCorrector()
        solver.fvModels().correct()
        solver.prePredictor()                   // <- alpha equation
        solver.momentumTransportPredictor()
        solver.momentumPredictor()              // <- assemble UEqns
        solver.thermophysicalPredictor()
        solver.pressureCorrector()              // <- PISO inner loop
        solver.momentumTransportCorrector()
    solver.postSolve()
```

`prePredictor` (`$ME/multiphaseEuler.C:235-249`) solves the phase fractions:

```cpp
        fluid_.solve(alphaControls, rAs, momentumTransferSystem_);
```

**Note the ordering hazard, and that upstream accepts it:** `rAs` is the list
of reciprocal momentum diagonals, and it is *populated by the pressure
corrector* (`$ME/cellPressureCorrector.C:93-104`) and cleared at its start
(`:66-70`). So the $\alpha$ equation's phase-pressure diffusion coefficient
$D^{\alpha}$ uses `rAs` **from the previous PIMPLE iteration** — it lags by one
outer iteration. On the first iteration of the first timestep `rAs` is empty
and the `rAs.size()` guard at `phaseSystemSolve.C:293` and `:741` disables the
term entirely.

`pressureCorrector` (`$ME/pressureCorrector.C:30-42`) dispatches on the
`faceMomentum` switch to `cellPressureCorrector()` or
`facePressureCorrector()`. Taking the cell variant,
`$ME/cellPressureCorrector.C:44-501`, in order:

1. **Recompute `p_rgh` for the current density** (`:49-52`):
   $p_{rgh} = p - \rho g h - p_{\mathrm{ref}}$.
2. **Interpolate face phase fractions** $\alpha_{k,f}$ (`:55-63`), clipped at 0.
3. **Build $A_{k}$** with the residual-alpha augmentation, and cache
   $rA_{k}=1/A_{k}$ if `implicitPhasePressure` (`:76-105`).
4. **Build and invert the $AD$ matrix** — drag + virtual mass — in both cell
   and face form (`:107`). §3.
5. **Assemble the explicit force fluxes** (`:110-147`): `Fs()` (lift, wall
   lubrication, **phase pressure**, turbulent dispersion) plus buoyancy and
   surface tension, giving

```cpp
            Fgfs.set
            (
                movingPhasei,
                Ffs[phase.index()]
              + alphafs[phase.index()]
               *(
                   ghSnGradRho
                 - fluid.surfaceTension(phase)*mesh.magSf()
                )
              - fvc::interpolate(max(phase, phase.residualAlpha()))
               *fvc::interpolate(phase.rho() - rho)*(buoyancy.g & mesh.Sf())
            );
```

   then premultiplying by the inverse coupling matrix (`:145-146`):

```cpp
        alphaByADfs = invADVfs & movingAlphafs;
        FgByADfs = invADVfs & Fgfs;
```

   These are the $(AD)^{-1}\alpha_{f}$ and $(AD)^{-1}F_{f}$ operators — the
   partially-eliminated pressure-gradient and body-force coefficients.

6. **Optional explicit momentum predictor** (`:152-208`), off by default
   (`predictMomentum`, `$ME/multiphaseEuler.H:82-84`).

7. **PISO corrector loop** `while (pimple.correct())` (`:211-498`):

   a. Build $H_{k}$ from `UEqns[k].H()` + residual-alpha source + `HVms`
      (virtual mass explicit part) (`:234-249`).

   b. Form face fluxes $\phi H_{k}$ including `ddtCorrs()` — the
      Rhie-Chow-style time-derivative correction, plus an optional virtual-mass
      `ddt` correction under the `VmDdtCorrection` switch
      (`momentumTransferSystem.C:1033-1125`) — then apply $(AD)^{-1}$
      (`:251-259`).

   c. **Total predicted flux** $\phi^{HbyA} = \sum_{k}\alpha_{k,f}\left(\phi H byAD\right)_{k}$
      (`:285-290`).

   d. **Pressure "diffusivity"** $rA_{f} = \sum_{k}\alpha_{k,f}\,\left[(AD)^{-1}\alpha_{f}\right]_{k}$
      (`:296-313`).

   e. Update `fixedFluxPressure` boundary conditions (`:316-340`).

   f. **Compressibility contributions** `compressibilityEqns(dmdts)`
      (`:343`, implementation `$ME/compressibilityEqns.C:36-121`) — per phase,
      density variation, mesh dilatation, $\psi$-compressibility (transonic or
      not), fvModel sources, and mass transfer, all divided by $\rho_{k}$ so
      they carry [m^3/s].

   g. **Non-orthogonal loop** (`:349-464`), solving

```cpp
            fvScalarMatrix pEqnIncomp
            (
                fvc::div(phiHbyA)
              - fvm::laplacian(rAf, p_rgh)
            );
```

      plus $\sum_{k}$ `pEqnComps[k]`. So the pressure equation is

$$\nabla \cdot \phi^{HbyA} - \nabla \cdot \left(rA_{f}\nabla p_{rgh}\right) + \sum_{k} C_{k}(p_{rgh}) = 0$$

      with $C_{k}$ the per-phase compressibility operator. There is **one**
      pressure, shared.

   h. On the final non-orthogonal iteration (`:382-463`): reconstruct the
      total flux, then **each phase flux** (`:392-394`):

```cpp
                    phase.phiRef() =
                        phiHbyADs[movingPhasei]
                      + alphaByADfs[movingPhasei]*mSfGradp;
```

      set the phase dilatation `phase.divU(...)` (`:397`), relax $p_{rgh}$,
      apply the optional drag correction, and reconstruct the cell velocities
      (`:441-452`):

```cpp
                        phase.URef() =
                            HbyADs[movingPhasei]
                          + fvc::reconstruct
                            (
                                alphaByADfs[movingPhasei]*mSfGradp
                              - FgByADfs[movingPhasei]
                            );
```

   i. Update $p$, apply the pressure reference, and update phase densities from
      $\Delta p_{rgh}$ via $\psi$ (`:466-497`).

8. Clear `UEqns` (`:500`).

**The structural point for the reduction:** the phase velocities are *never*
obtained from a solved momentum equation. They are reconstructed from
$(AD)^{-1}\left(H + \text{face forces} + \alpha_{f}\nabla p\right)$ after the
pressure solve. The inter-phase coupling is inverted exactly and implicitly;
the pressure is shared and solved once.

---

## 7. Physics term vs numerical device

For a V&V write-up these have different standing: a physics term changes the
model being validated and must be reported as part of it; a numerical device
must vanish under refinement (or be shown not to contaminate the result).

| Mechanism | Standing | Where | Active by default? |
|---|---|---|---|
| Virtual mass $K^{vm}(\mathrm{D}U_{1}/\mathrm{D}t - \mathrm{D}U_{2}/\mathrm{D}t)$ | **Physics.** Real inertial coupling; alters the principal part of the momentum system. | `momentumTransferSystem.C:704-762` | No — but enabled in every gas-liquid tutorial, $C_{vm}=0.5$ |
| Granular phase pressure $p^{\prime}\nabla\alpha$ | **Physics.** A real particle-particle stress; adds a genuine $\nabla\alpha$ restoring term. | `momentumTransferSystem.C:287-299`, `:405-417` | No — zero for fluid phases by base-class default |
| Turbulent dispersion $D\nabla\alpha$ | **Physics.** Real turbulent diffusion of $\alpha$; regularising where turbulence exists. | `momentumTransferSystem.C:301-337` | No |
| Drag $K^{d}(U_{1}-U_{2})$ | **Physics**, but **not regularising.** A zeroth-order relaxation term; it damps but does not change the characteristics. | `momentumTransferSystem.C:573-636` | Effectively always |
| Lift, wall lubrication | **Physics**, not regularising. | `momentumTransferSystem.C:230-285` | No |
| Implicit pairwise $AD$ inversion (partial elimination) | **Numerical device.** Solves the stiff drag/VM coupling exactly; does not change the equations solved. | `momentumTransferSystem.C:461-766` | Always |
| `residualAlpha` diagonal flooring | **Numerical device.** Vanishing-phase conditioning. | `cellPressureCorrector.C:82-91` | Always |
| MULES limiter + upwind predictor + sub-cycling | **Numerical device.** Boundedness and numerical diffusion of $\alpha$. | `phaseSystemSolve.C:327-738` | Always |
| Implicit $\alpha$ diffusion deferred correction | **Numerical device** wrapping a **physics** coefficient. The `ddt - ddt` cancellation makes it purely a solution technique. | `phaseSystemSolve.C:741-768` | No |
| `dragCorrection`, `VmDdtCorrection` | **Numerical devices.** Cell/face consistency. | `momentumTransferSystem.C:1128-1190`, `:1060-1122` | No |
| Interface compression `cAlpha` | **Numerical device** (sharpening). | `phaseSystemSolve.C:451-498` | Only if `cAlpha` set |

**The honest summary line:** upstream's *well-posedness* mechanisms are virtual
mass (fluid-fluid) and granular phase pressure (fluid-solid), both optional and
both physics. Everything that is on by default is a numerical device that
stabilises the solve without restoring hyperbolicity.

---

## 8. Contrast: `incompressibleDriftFlux`

`$OF/applications/modules/incompressibleDriftFlux/` takes the alternative route
and is worth naming because it is the one TAMPINES already has
(`crates/tampines/src/multiphase_1d/drift_flux.rs`). From
`.../incompressibleDriftFlux.H:27-38`:

```
    Solver module for 2 incompressible fluids using the mixture approach with
    the drift-flux approximation for relative motion of the phases, ...

    The momentum and other fluid properties are of the "mixture" and a single
    momentum equation is solved with mixture transport modelling ...
```

One mixture momentum equation, relative velocity given algebraically by a
`relativeVelocityModel` (`simple`, `general`, `MichaelsBolger`). The
well-posedness question does not arise: there is no second momentum equation to
have complex characteristics with.

Structurally interesting is that its $\alpha$ equation uses **the same
deferred-correction diffusion pattern** as `multiphaseEuler`'s
`implicitPhasePressure`, at `.../incompressibleDriftFlux.C:184-212`:

```cpp
    // Apply the diffusion term separately to allow implicit solution
    // and boundedness of the explicit advection
    {
        volScalarField nuEff(momentumTransport->nut());
        nuEff += packingDispersion->Dd();

        fvScalarMatrix alpha1Eqn
        (
            fvm::ddt(alpha1) - fvc::ddt(alpha1)
          - fvm::laplacian(nuEff, alpha1)
        );

        alpha1Eqn.solve(alpha1.name() + "Diffusion");

        alphaPhi1 += alpha1Eqn.flux();
```

with $D$ from a `packingDispersionModel` (`DeClercq`, `Green`, `Landman`,
`Usher`, `none`) — the drift-flux analogue of the granular phase pressure. The
comment states the motivation plainly: implicit solution of a stiff diffusion
alongside a bounded explicit advection.

---

## 9. Recommendation for the 1-D reduction

Addressed to whoever implements `crates/tampines/src/multiphase_1d/two_fluid.rs`.
The current file is a documented scaffold whose `step()` returns
`TampinesError::NotYetImplemented`; its module docs already flag regularisation
as decision (4). This section answers that decision.

### 9.1 Implement: virtual mass, in the implicit coupling matrix

**This is the mechanism to port.** Reasons, all traceable above:

- TAMPINES' target regime is gas-liquid blowdown, which is upstream's
  fluid-fluid branch. Granular phase pressure is not applicable
  ($p^{\prime}\equiv 0$ for fluid phases —
  `phaseCompressibleMomentumTransportModel.C:99-108`).
- It is the *only* non-drag interfacial momentum term present in the canonical
  bubbly tutorial.
- It is a physics term, so it can be stated in a V&V write-up as part of the
  model rather than apologised for as a numerical hack.

Concretely, in 1-D with two phases $g$ and $l$, add to each phase momentum
equation

$$F^{vm}_{g} = -K^{vm}\left(\frac{\mathrm{D}u_{g}}{\mathrm{D}t} - \frac{\mathrm{D}u_{l}}{\mathrm{D}t}\right), \qquad F^{vm}_{l} = -F^{vm}_{g}$$

$$K^{vm} = \max(\alpha_{g}, \alpha^{\mathrm{res}}_{g})\,C_{vm}\,\rho_{l}$$

$$\frac{\mathrm{D}u_{k}}{\mathrm{D}t} = \frac{\partial u_{k}}{\partial t} + u_{k}\frac{\partial u_{k}}{\partial x}$$

with $C_{vm} = 0.5$ as the default, $\rho_{l}$ the continuous (liquid) density
[kg/m^3], $\alpha_{g}$ the dispersed (vapour) fraction [-]. Units of $K^{vm}$:
[kg/m^3]; of $F^{vm}$: [N/m^3]. Follow
`dispersedVirtualMassModel.C:51-67` and `constantVirtualMassCoefficient.C:71-79`.

**Do not add it as an explicit source.** Upstream deliberately does not: the
implicit part goes into the coupling matrix diagonal/off-diagonal
(`momentumTransferSystem.C:729-750`), the explicit remainder into `H`
(`:734-758`). In 1-D two-phase this is a $2\times 2$ block per face — which the
existing scaffold already identifies as necessary for the *drag* coupling
(`two_fluid.rs`, decision 3). **Virtual mass belongs in that same $2\times 2$
block, not beside it.** The block is

$$A_{g} + \tilde{K}^{d} + \tilde{K}^{vm}A^{\mathrm{D}}_{g} \quad\text{and}\quad -\tilde{K}^{d} - \tilde{K}^{vm}A^{\mathrm{D}}_{l} \quad (\text{row } g)$$

$$-\tilde{K}^{d} - \tilde{K}^{vm}A^{\mathrm{D}}_{g} \quad\text{and}\quad A_{l} + \tilde{K}^{d} + \tilde{K}^{vm}A^{\mathrm{D}}_{l} \quad (\text{row } l)$$

For $n=2$ the LU inversion upstream performs (`momentumTransferSystem.C:484-491`)
collapses to a closed-form $2\times2$ inverse — no linear algebra library is
needed. This is the exact two-phase partial-elimination algorithm.

### 9.2 Also implement: `residualAlpha` flooring

Cheap, and without it the $2\times2$ block becomes singular as
$\alpha \to 0$ — which is guaranteed to happen at a blowdown front. Port both
uses:

- Diagonal augmentation $A_{k} \leftarrow A_{k} + \max(\alpha^{\mathrm{res}}_{k}-\alpha_{k},0)\rho_{k}/\Delta t$
  with the matching $\ldots \times u^{n}_{k}$ source
  (`cellPressureCorrector.C:82-91`, `:238-243`).
- The $\alpha_{j}/\max(\alpha_{j},\alpha^{\mathrm{res}}_{j})$ taper on
  $K^{d}$ and $K^{vm}$ (`momentumTransferSystem.C:617-620`, `:723-727`).

Default $\alpha^{\mathrm{res}} = 10^{-6}$, per the tutorials. Document it as a
numerical device, not physics.

### 9.3 Also implement: a bounded $\alpha$ transport scheme

The 1-D analogue of MULES. At minimum a first-order upwind flux with explicit
clipping to $[0,1]$ and re-normalisation $\alpha_{g}+\alpha_{l}=1$. This is
what actually prevents a blowup in practice, and it is honest to say so. Note in
the doc comment that it carries first-order numerical diffusion of $\alpha$,
and that a grid-refinement study must therefore be reported for any V&V case —
because unlike a well-posed system, refinement here may *worsen* the solution.

### 9.4 Safe to omit in 1-D, with reasons

| Omit | Why it is safe |
|---|---|
| **Granular phase pressure / `pPrime`** | Identically zero for fluid phases (`phaseCompressibleMomentumTransportModel.C:99-108`). TAMPINES has no particulate phase. Omitting it reproduces upstream's fluid-fluid behaviour exactly. |
| **Kinetic theory, frictional stress, radial distribution** | Same reason — granular only. |
| **Lift force** | Requires $\nabla \times U$; identically zero in 1-D. Not a modelling choice, a geometric one. |
| **Wall lubrication** | Requires a wall-normal direction and wall distance; meaningless in a 1-D area-averaged pipe. |
| **Turbulent dispersion** | Requires $\nu_{t}$ from a resolved turbulence model (`Burns.C:83`). A 1-D area-averaged model has none. **State this as a known omission**, not as "not needed": it is a real regularising term that upstream can call on and this reduction cannot. |
| **Surface tension / interface compression `cAlpha`** | Interface-capturing devices for resolved interfaces; there is no resolved interface in an area-averaged 1-D pipe. |
| **The full $n$-phase LU** | $n=2$; use the closed-form inverse. |
| **`faceMomentum` variant, `dragCorrection`, `VmDdtCorrection`** | All cell/face consistency devices for 3-D unstructured meshes. A 1-D staggered or collocated scheme has a different consistency problem; porting these would be cargo-culting. |
| **Population balance, IATE, wall boiling** | Out of scope for the first solver. |

### 9.5 What to record in the V&V documentation

Per the workspace rule that V&V docs state methodology *and* results:

- The regularisation actually used, named, with $C_{vm}$ and
  $\alpha^{\mathrm{res}}$ values, and the statement that **it is a modelling
  choice that changes the answer**.
- That the six-equation system with drag alone has complex characteristics and
  the implementation does not claim otherwise.
- A grid-refinement study for every benchmark case, with the observed order,
  since numerical diffusion is load-bearing here.
- A sensitivity of the headline result to $C_{vm}$ over at least $[0, 0.5]$.
  If the answer moves materially, that is the honest measure of how much the
  regularisation is doing, and it belongs in the report.

---

## 10. Honest gaps — what this study did NOT establish

Stated as unknown rather than filled in.

1. **No hyperbolicity analysis was performed.** Nothing here computes the
   characteristics of the system with virtual mass included, or establishes the
   $C_{vm}$ threshold at which they become real. That is a known result in the
   two-fluid literature, but **it is not in the vendored source**, so this
   document does not assert it. The claim made here is only the weaker,
   source-backed one: virtual mass is the term upstream uses in the fluid-fluid
   branch, and it enters the principal part of the momentum system where drag
   does not.

2. **Upstream never states its own reasoning.** There is no comment, header
   block, or doc file anywhere in the vendored tree explaining *why* virtual
   mass is enabled in every bubbly tutorial, or acknowledging the
   well-posedness problem at all. The regime split in §3.3 is inferred from
   tutorial configuration files, not from an upstream statement of intent. It
   is a strong pattern (23 of 30 cases) but it is inference.

3. **Two tutorials run with no regularising term at all** —
   `damBreak4phase` and `hydrofoil` (drag only, no virtual mass, no phase
   pressure, no dispersion). Whether these are known to be robust, are
   tolerated because MULES holds them together, or are simply short enough not
   to blow up, is **not determinable from the source**. This is the strongest
   counter-evidence to the §1 finding and is reported rather than suppressed.

4. **Sub-model internals not opened.** For `kineticTheory` I transcribed the
   composition of `pPrime` from `kineticTheoryModel.C:302-351` but did not open
   `granularPressureModel`, `radialModel`, or `frictionalStressModel`. The
   granular branch is out of scope for the 1-D reduction, so this was not
   pursued.

5. **`MULES` itself was not read.** I established *where* it is called and
   with what arguments (`phaseSystemSolve.C:563-599`, `:609-622`, `:719-737`)
   but did not open `$OF/src/finiteVolume/fvMatrices/solvers/MULES/`. The
   characterisation "flux-corrected transport limiter enforcing boundedness"
   comes from the call signature and the surrounding comments, not from reading
   the limiter algorithm.

6. **`BlendedInterfacialModel` and the blending methods were not read in
   depth.** The `blending` sub-dictionary (e.g.
   `bubbleColumn/constant/phaseProperties:46-64`) switches models between
   dispersed and segregated topologies as $\alpha$ varies. This affects *which*
   drag/virtual-mass model is active in a given cell and therefore matters for
   a faithful port, but it was not traced. Flagged as follow-up.

7. **Nothing was compiled or run.** No OpenFOAM build, no tutorial execution,
   no numerical verification of any transcribed formula. Every equation above
   is a transcription of source text, and transcription errors are possible —
   check against the cited lines.

8. **No literature was consulted or catalogued.** Per the workspace rule that
   any document informing the code goes into `kovan-literature`: this study
   cites *no* literature, only vendored source. If the implementer wants the
   $C_{vm}$ hyperbolicity threshold from the two-fluid literature, that paper
   must be catalogued in `crates/kovan-literature` before its numbers are used
   in code.

9. **Markdown not machine-validated.** Neither `pandoc` nor `cmark-gfm` is
   installed in this environment (`which pandoc cmark-gfm cmark` returns
   nothing). The math in this document was written to the conservative subset
   required by the workspace `CLAUDE.md` — no matrix/array environments, no
   `cases`, no `\boxed`/`\underbrace`/`\displaystyle`/`\tfrac`/`\dfrac`, no
   Unicode Greek or operators inside math, explicit braces on sub/superscripts
   — and checked by eye, but **not verified by a renderer**.

---

## 11. File index

Every file opened for this study, repo-relative.

**Solver module** (under `$ME/`):

- `multiphaseEuler.H`, `multiphaseEuler.C`
- `momentumPredictor.C`
- `pressureCorrector.C`, `cellPressureCorrector.C`, `facePressureCorrector.C`
- `compressibilityEqns.C`
- `phaseSystem/momentumTransferSystem/momentumTransferSystem.H`, `.C`
- `phaseSystem/phaseSystem/phaseSystem.H`, `phaseSystem.C`, `phaseSystemSolve.C`
- `phaseSystem/phaseModel/phaseModel.H`
- `phaseSystem/phaseModels/MovingPhaseModel/MovingPhaseModel.C`
- `phaseSystem/interfacialModels/virtualMassModels/virtualMassModel/virtualMassModel.H`
- `phaseSystem/interfacialModels/virtualMassModels/dispersedVirtualMassModel/dispersedVirtualMassModel.H`, `.C`
- `phaseSystem/interfacialModels/virtualMassModels/constantVirtualMassCoefficient/constantVirtualMassCoefficient.C`
- `phaseSystem/interfacialModels/virtualMassModels/Lamb/Lamb.C`
- `phaseSystem/interfacialModels/virtualMassModels/noVirtualMass/noVirtualMass.C`
- `phaseSystem/interfacialModels/dragModels/dispersedDragModel/dispersedDragModel.C`
- `phaseSystem/interfacialModels/turbulentDispersionModels/Burns/Burns.C`
- `momentumTransportModels/phasePressureModel/phasePressureModel.H`, `.C`
- `momentumTransportModels/kineticTheoryModels/kineticTheoryModel/kineticTheoryModel.C`

**Elsewhere** (under `$OF/`):

- `src/MomentumTransportModels/phaseCompressible/phaseCompressibleMomentumTransportModel.C`
- `applications/solvers/foamRun/foamRun.C`
- `applications/modules/incompressibleDriftFlux/incompressibleDriftFlux.H`, `.C`
- `applications/modules/incompressibleDriftFlux/packingDispersionModels/packingDispersionModel/packingDispersionModel.H`
- `applications/modules/incompressibleDriftFlux/relativeVelocityModels/relativeVelocityModel/relativeVelocityModel.H`
- `tutorials/multiphaseEuler/*/constant/momentumTransfer` (all 30)
- `tutorials/multiphaseEuler/bubbleColumn/constant/phaseProperties`
- `tutorials/multiphaseEuler/fluidisedBed/system/fvSolution`, `constant/momentumTransport.particles`
- `README.org`, `etc/bashrc`, `.build`

**Workspace files read (not modified):**

- `crates/tampines/src/multiphase_1d/two_fluid.rs`
