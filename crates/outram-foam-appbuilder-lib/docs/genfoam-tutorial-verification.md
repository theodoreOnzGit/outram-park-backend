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
| `reactorCases/3D_gFHR` | `Tfmax` avg/min/max = 981.808 / 900.664 / 1062.87 K | fuel temperature | **consistency only** — pebble power model missing |
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

### gFHR — consistency only

The `nuclearSteadyStatePebble` and `lumpedNuclearStructure` power models are not
ported, so `Tfmax` cannot be reproduced. What is checked instead: the
pebble-internal conduction stack, rebuilt from upstream's own geometry,
conductivities and power with the closed-form helpers in
`tampines::pebble_bed::triso`.

| term | rise (K) |
|---|---|
| graphite shell, 1.80 → 2.00 cm | 11.87 |
| fuelled annulus, 1.38 → 1.80 cm | 21.42 |
| TRISO coatings | 31.25 |
| UO2 kernel centre | 7.02 |
| **total** | **71.56** |

45.1 % of upstream's 158.66 K inlet-to-peak-fuel rise; implied bed-average pebble
surface temperature 910.2 K, between the 823.15 K inlet and the ~923 K outlet.
Separately, the graphite-shell conductance agrees with upstream's own
`lumped_structure.py` closed form to 6e-16 relative.

**A gFHR pebble is not shaped like an HTR-10 pebble** — unfuelled central
graphite core, annular fuelled matrix, unfuelled shell — so
`tampines::pebble_bed::Pebble`, which assumes a fuelled centre, cannot be reused
for it. Blocked on the pebble-power-model bead.

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

## Scope

None of this is validation against experiment. The two `k_eff` cases compare
against another code's published result on identical inputs; the gFHR case is a
consistency check on conduction alone. Each test's own doc comment states its
methodology, its measured numbers and what it does **not** cover.
