# Code-to-code verification against the GeN-Foam tutorials

Status map for verifying this workspace's GeN-Foam port against **upstream's own
published reference values**. Tracked as bead `op-2df1`.

## Why upstream's tutorials are a good target

Upstream GeN-Foam is archived at commit `652b3da` on
`gitlab.com/foam-for-nuclear/GeN-Foam` — the same commit this port's file headers
cite. Every tutorial ships an `Alltest` regression script carrying upstream's
**own expected values** at upstream's **own stated tolerance**, alongside the
complete `polyMesh` and `nuclearData` those values were produced from. So the
inputs are real upstream inputs, not a reconstruction, and the references are
numbers upstream published for exactly those inputs.

Get the clone (this repository deliberately does not vendor it — 39 MB of
third-party source):

```bash
git clone --depth 1 https://gitlab.com/foam-for-nuclear/GeN-Foam.git /workspace/GeN-Foam
```

Every test below **skips** rather than fails when it is absent. Override the
location with `GENFOAM_UPSTREAM`.

## The four cases

| case | upstream reference | reference is | status |
|---|---|---|---|
| `reactorCases/3D_SmallESFR_NewSolverVerification` | `expectedKeff = 0.936827` | `k_eff` | **verified** |
| `reactorCases/2D_MSFR` | `expectedKeff = 0.960283`, power 20 MW | `k_eff` | **posed identically**; +2002 pcm residual = feedback state |
| `reactorCases/3D_gFHR` | `Tfmax` avg/min/max = 981.808 / 900.664 / 1062.87 K | fuel temperature | **power model ported**; rise fits upstream's numbers, surface temp needs TH |
| `featureCases/1D_PSBT_SC` | `alpha.vapour = 0.123835`, `T = 620.178 K` | exit void + temperature | **blocked** — two-phase solver missing |

Upstream's tolerance is 0.001 relative for the two `k_eff` cases and 0.01 for
gFHR and PSBT.

## What runs today

```bash
# Neutronics: ESFR and MSFR.
cargo test --release -p outram-foam-appbuilder-lib --test genfoam_tutorial_keff -- --nocapture

# gFHR pebble conduction consistency.
cargo test --release -p tampines --test genfoam_gfhr_pebble_conduction -- --nocapture
```

### ESFR — verified

Port, reference state: `k_eff = 0.944987`. Upstream: `0.936827`. **+871 pcm.**

Upstream's number comes from a fully coupled run (`fluidRegion onePhase` +
`diffusionNeutronics` + `extendedThermoMechanics`) read at the converged steady
state, so its cross sections sit at fed-back temperatures and densities. Every
SFR feedback is negative at power, so the coupled value must fall **below** the
reference-state one — and it does. That is a consistency argument about sign and
magnitude, not a measurement of the feedback. Running the perturbed states
through the parametrisation and comparing the reactivity coefficients themselves
is what would upgrade it.

### MSFR — posed identically, residual is feedback state

Upstream's `albedoSP3` boundary condition is now ported (`op-3fer`), so the
spatial neutronics is posed exactly as upstream poses it: same mesh, cross
sections, zone map, diffusion operator, boundary conditions and power iteration.
Upstream's per-patch spec is `topwall`/`bottomwall`/`reflector` at `gamma 0.1`
and **`hx` at `gamma 0.5`** — assuming one gamma for the whole boundary is the
obvious mistake, and this test made it before the dictionary was read in full.

| | `k_eff` |
|---|---|
| port, reference state, upstream's boundaries | **0.979508** |
| upstream coupled `expectedKeff` | 0.960283 |
| difference | **+2002 pcm** |
| (previously, blanket `fixedValue 0`) | 0.958444 |

So the albedo condition is worth **+2106 pcm** — about as much as the whole
remaining discrepancy.

**The residual is the cross-section state, and it is measured, not guessed.**
Upstream's number comes from its coupled `steadyStateEN` stage, where the salt
has heated under 20 MW against a heat exchanger held at 900 K; this test
evaluates at the nominal reference state, because the port has no coupled TH
driver. Running the case's **own** perturbed states through the port's
parametrisation gives

| state | `k_eff` | coefficient |
|---|---|---|
| `TFuel` 900 → 1500 K | 0.960741 | **−3.3238 pcm/K** |
| `rhoCool` 4125 → 3419 kg/m³ | 0.944979 | **+5.2838 pcm per kg/m³** |
| both | 0.923777 | — |

`+2002 pcm` is therefore a core running some 600 K above the 900 K cold leg, or
a smaller rise with the density drop that accompanies it. The test asserts only
what is rigorous and unfitted — the sign (feedback is negative, so upstream must
sit below the reference state) and that upstream's value lies inside the span the
case's own perturbed states allow. **It does not claim to reproduce 0.960283.**

Closing it needs the coupled TH solve (pump, buoyancy, turbulence, the
`fixedTemperature` heat exchanger) plus circulating-fuel precursor drift — filed
separately. Note `beta_total` here is `2.853e-3`, so drift is worth at most
~285 pcm and is *not* the main term.

**A wrong answer this found.** Posed with `fixedValue 0` on *every* patch —
including the two `wedge` planes of the axisymmetric mesh — the same solve
returns `k_eff = 0.153242`, −84 042 pcm. A wedge plane is a geometric artefact,
not a surface. Patch *kind* decides the boundary.

### gFHR — power model ported, rise fits upstream's own numbers

`nuclearSteadyStatePebble` is now ported
(`genfoam::thermal_hydraulics::structure::nuclear_steady_state_pebble`,
`op-5eoa`). Upstream's model is a **closed-form chain**, not an iteration, so the
port is an exact transcription. Two things a from-physics derivation gets wrong
and reading upstream fixes:

1. The fuelled annulus goes outer-face-to-**volume-average**, not to its centre —
   the average is what the dispersed particles sit at.
2. The matrix conductivity comes from a **Maxwell** mixture rule over the TRISO
   packing, not from bare graphite.

Together those moved the total rise from 71.56 K (my earlier hand-derived stack)
to **61.67 K**.

| term | rise (K) |
|---|---|
| surface → matrix outer | 11.634 |
| matrix outer → volume average | 12.527 |
| TRISO coatings | 30.626 |
| kernel surface → centre | 6.883 |
| **total** | **61.670** |

**An independent confirmation.** The ported Maxwell rule gives
`k_eff = 29.5870 W/(m·K)`. Upstream's own `lumped_structure.py` hardcodes
`k_matrix = 29`, commenting "taking from python effective calculation" — two
independent upstream artefacts agreeing with the port.

**Why this is checkable without the coupled TH.** `powerDensityNeutronics` is a
uniform *scalar*, `alpha` is uniform, and the chain is linear in power with no
other spatial dependence — so the rise is the same constant in every cell. That
turns upstream's published statistics into a real constraint:

```text
dT <= Tfmax_min - T_coolant_inlet = 900.664 - 823.15 = 77.514 K
```

Measured slack **15.844 K**, an ordinary pebble-bed convective film drop. Implied
bed-average pebble surface **920.14 K**, between the 823.15 K inlet and the
~923 K outlet. Uniform power also predicts the `Tfmax` spread (162.206 K) is
surface-temperature spread only. Both bounds are upstream's own numbers; nothing
is fitted.

Still missing for a direct `Tfmax` reproduction: the convective coupling that
produces the surface temperature, i.e. the porous TH driver. `lumpedNuclearStructure`
(the case's second model) is also not ported.

### PSBT — blocked

Needs `regionSolvers { fluidRegion twoPhase; }`, the Euler-Euler porous
two-phase solve. The port's `genfoam::thermal_hydraulics::solver` has
`one_phase` only. The structures the case uses (`fixedPower`) *are* ported, so
the fluid solver is the sole blocker.

No approximation is attempted, and none should be: `alpha.vapour = 0.123835` is
GeN-Foam's **own computed** exit void fraction, not the PSBT experimental value,
so reproducing it needs GeN-Foam's specific interfacial and wall-boiling
closures. A drift-flux estimate would produce a number that means nothing
against it. Note `crates/outram-foam-multiphase` already carries ~7.2k lines of
drift-flux, wall-boiling, two-fluid and CHF work — whether to build on that
rather than port afresh is the first design question.

## What had to be built to get here

The neutronics solvers and the cross-section layer already existed; the gap was
entirely I/O.

- `outram_foam_basic_lib::io::dict` — `FoamValue::Dict`, so a dictionary
  appearing as a **list element** parses. `states ( name { … } … )`, `cellZones`
  and `boundary` are all that shape; the braces previously tokenised into stray
  words and the entry flattened into nonsense. Plus
  `FoamFile::read_with_includes`, since the ESFR case keeps each state's cross
  sections in separate `#include`d `XS…` files.
- `outram_foam_appbuilder_lib::io::nuclear_data` — the `nuclearData` reader.
- `outram_foam_appbuilder_lib::io::poly_mesh` — `parse_cell_zones`,
  `read_cell_zones`, `zone_of_cell`.

## Routes that were checked and ruled out

Recorded so they are not re-searched.

- **Running upstream.** The definitive route, and it is barred: `dl.openfoam.com`
  and `develop.openfoam.com` both return 403 on CONNECT under this session's
  egress policy. Ubuntu `universe` carries only OpenFOAM v1912, seven years of
  API drift from the v2506 GeN-Foam builds against, so the distro package is not
  a substitute.
- **Upstream's own `tests/` directory.** Contains only
  `hydrogenThermophysicalProperties` and `radialBasisFunctions` build tests. No
  reference values.
- **A one-phase water-cooled substitute for PSBT.** Every `featureCases` entry
  that ships reference values and uses water is two-phase (`1D_PSBT_SC` ×4,
  `1D_CHF`, `1D_boiling`). `2D_fullCoupling` and
  `2D_onePhaseAndPointKineticsCoupling` are one-phase but carry no reference
  values and are generic coupling demos, not reactor cases. `2D_KNS37-L22` is
  sodium.
- **Reducing gFHR's convective coupling to closed form.** The case's own mesh
  (r = 1.2 m, h = 3.0947 m) and power density give 279.5 MW, independently
  confirming the 280 MW in upstream's `lumped_structure.py`; with
  `mdot = 1173 kg/s` and `cp = 2265.75 J/(kg·K)` the mean coolant rise is
  105.2 K. But upstream's `Tfmax` spread is **162.2 K**, half again larger, and
  the film drop implied by `Tfmax_min` (15.8 K) disagrees with the one implied by
  `Tfmax_avg` (44.4 K). That gap is three-dimensional flow maldistribution
  through the bed, so no one-dimensional reduction recovers the statistics. The
  porous flow solve is genuinely required.

## Scope

None of this is validation against experiment. The two `k_eff` cases compare
against another code's published result on identical inputs; the gFHR case is a
consistency check on conduction alone. Each test's own doc comment states its
methodology, its measured numbers and what it does **not** cover.
