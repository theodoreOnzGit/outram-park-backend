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

## Upstream is built and run here

`develop.openfoam.com` is egress-blocked, but OpenCFD's source is mirrored on
GitHub: [`ZhangYanTJU/OpenFOAM-ESI`](https://github.com/ZhangYanTJU/OpenFOAM-ESI)
carries `refs/tags/OpenFOAM-v2506`, the version GeN-Foam targets. Built with
`SYSTEMOPENMPI` and system scotch, no ThirdParty — 127 libraries, 267 binaries.
GeN-Foam `652b3da` builds on top.

```bash
git clone --depth 1 --branch OpenFOAM-v2506 \
    https://github.com/ZhangYanTJU/OpenFOAM-ESI.git /workspace/OpenFOAM-v2506
```

**Three obstacles, all in GeN-Foam's offbeat submodule:** `.gitmodules` names the
branch `ffn/Gen-Foam2` where it is actually `ffn/GeN-Foam2`; the pinned commit
`9845e449` was force-pushed away; and offbeat introduced `offbeatTime` in
2025-08 while GeN-Foam still calls `timeHandling(const fvMesh&, Time&)`, so no
surviving offbeat satisfies both GeN-Foam and v2506. Resolved by taking the
v2506-compatible tip and excluding the two files needing the lost API —
GeN-Foam's `extendedThermoMechanics` and offbeat's `fuelBehaviour` (stubbed so
`liboffbeatMain` still links). Only ESFR's `newSolver` is affected; its
`legacySolver` sibling is checked against the **same** `expectedKeff`.

**Every tutorial reproduces its own published reference**, which is what makes
them usable as reference runs:

| case | run here | `Alltest` expects | difference |
|---|---|---|---|
| ESFR | 0.936873 | 0.936827 | +49 pcm |
| MSFR | 0.96031 | 0.960283 | +2.8 pcm |
| gFHR | 981.812 / 900.664 / 1062.96 | 981.808 / 900.664 / 1062.87 | min **exact** |
| PSBT | 0.123835 / 620.178 | 0.123835 / 620.178 | **exact** |

## The four cases: port vs upstream

| case | what is compared | result |
|---|---|---|
| **ESFR** | `k_eff` at upstream's converged state | **+60.7 pcm** — inside upstream's own 0.001 tolerance |
| **MSFR** | `k_eff`, per-cell cross sections, circulating fuel, **as the tutorial ships it** | **+0.2 pcm** |
| **gFHR** | every stage of the pebble model, cell by cell | **≤ 7.2e-06** |
| **PSBT** | the `ReynoldsPower` wall-friction closure, cell by cell | **6.15e-06** |

### ESFR — verified

| | `k_eff` | vs upstream |
|---|---|---|
| port @ nominal reference | 0.944987 | +866.1 pcm |
| **port @ upstream's converged state** | **0.937442** | **+60.7 pcm** |

Inside upstream's own 0.001 relative tolerance, asserted. `TFuel`/`TClad` are
zero in the non-fuelled zones, so the state is averaged over the **6540 fuelled
cells** of 23322 — a plain mean over the whole mesh is the obvious trap and is
meaningless.

### MSFR — verified, as the tutorial ships it

| | `k_eff` | vs upstream as shipped |
|---|---|---|
| upstream, as shipped (`liquidFuel true`) | 0.960312 | — |
| **port, per-cell state + precursor drift** | **0.960314** | **+0.2 pcm** |
| port, drift off | 0.962032 | +179.1 pcm |
| upstream, `liquidFuel false` | 0.962019 | — |

The drift term agrees on its own, not just in the total: the port makes it worth
**−178.6 pcm** where upstream makes it **−177.4 pcm**, 1.1 pcm apart. A
compensating pair of errors would match the total and fail that.

Drift is the circulating-fuel precursor transport of `precEq.H` —
`λ_k α C*_k + div(φ, C*_k) − laplacian(D_C, C*_k) = S_n β_k / k_eff`, with
`C_k = α C*_k` and the delayed source `Σ λ_k C_k` entering the flux equation
**undivided by `k_eff`**. It is verified independently of any case by two
mesh-agnostic properties: with zero flow it reproduces the `chi_eff` collapse to
5.1e-16 relative, and it conserves the volume-integrated delayed source to
5.4e-11 at every flow rate tested.

The port is handed upstream's own `alpha`, `phi = fvc::flux(U·alpha)` and
`diffCoeffPrec` — those are thermal-hydraulic state, which the port has no
coupled driver to produce. All five are `NO_WRITE` in upstream; harvesting them
needed those `IOobject`s flipped to `AUTO_WRITE` in the clone, a diagnostic
change verified as such (`keff` unchanged at 0.960312) and since reverted.

#### Intermediate results, each against its own matched upstream run

| | `k_eff` | upstream, same configuration | difference |
|---|---|---|---|
| reference state, `albedoSP3` boundary | 0.973858 | 0.973858 | **0.0 pcm** |
| reference state, `fixedValue 0` | 0.950415 | 0.950409 | +0.6 pcm |
| reference state, `zeroGradient` | 1.075834 | 1.075830 | +0.4 pcm |
| upstream's mean state, drift off | 0.963278 | 0.962019 | +130.9 pcm |
| upstream's per-cell state, drift off | 0.962032 | 0.962019 | +1.4 pcm |

The three boundary rows span 12 500 pcm of leakage and agree to better than
1 pcm across it, which is what makes the boundary condition verified rather than
tuned.

The boundary condition is verified on its own, across the full span of leakage,
by switching upstream's parametrisation and drift off and varying only the
boundary:

| boundary | port | upstream, run here | difference |
|---|---|---|---|
| `albedoSP3`, per-patch gamma (0.1 / 0.5) | 0.973858 | 0.973858 | **0.0 pcm** |
| `fixedValue 0` (vacuum) | 0.950415 | 0.950409 | +0.6 pcm |
| `zeroGradient` (reflective) | 1.075834 | 1.075830 | +0.4 pcm |

#### How the old +787.8 pcm was found and closed

The gap was bisected by disabling, one at a time and **in upstream itself**, the
cross-section parametrisation (empty `xsVariables`), the precursor drift
(`liquidFuel false`) and the albedo boundary (`fixedValue 0` / `zeroGradient`).
Each row below is a run, not an inference:

| part | worth | verdict |
|---|---|---|
| Laplacian delta coefficient | **609 pcm** | **port defect, fixed.** `fvm::laplacian` divided by `\|d\|`, i.e. OpenFOAM's `orthogonal` scheme. Every GeN-Foam neutronics `fvSchemes` asks for `uncorrected`, which uses `nonOrthDeltaCoeffs` = `1/max(n·d, 0.05\|d\|)`. On this mesh that is 1.7 % on interior faces and up to 12 % on the boundary patches. |
| albedo Robin weight | (exposed by the above) | **port defect, fixed.** The weight `γ·δ/D` only cancels to upstream's `γ·\|Sf\|` when its `δ` is the Laplacian's `δ`; the two were computed independently. |
| precursor drift | **178 pcm** | unported physics; **now ported** (`op-exma`), agreeing with upstream's own measurement of it to 1.1 pcm. |
| cross-section parametrisation | 14 pcm | already correct. The RBF port moves `k_eff` by −1201 pcm where upstream moves it by −1215 pcm. |

The fix is `DeltaCoeff` in `outram-foam-basic-lib`'s `fvm::laplacian_with_delta`,
with `DeltaCoeff::Orthogonal` kept as the default so no existing caller changed.
The same unprojected-distance pattern remains in `fvc::interpolate`,
`fvc::sn_grad` and `fvm::laplacian_vec` — unmeasured, tracked as `op-h83b`.

### gFHR — verified, whole model

`nuclearSteadyStatePebble` is ported and every stage checked against upstream's
own fields over all 292 500 cells, with upstream's own pebble surface
temperature as the input:

| stage | worst relative difference |
|---|---|
| `keffmatrix` (Maxwell mixture) | 9.44e-07 |
| `Tmout` | 3.82e-06 |
| `Tmav` | 5.07e-06 |
| `TfS` | 7.21e-06 |
| `Tfav` | 4.50e-06 |
| `Tfmax` | 5.10e-06 |

~5 ppm is the fields' own 6-significant-figure ASCII write precision. Two details
a from-physics derivation gets wrong, and reading upstream fixes: the annulus
integral goes to its **volume average**, not its centre, and the matrix
conductivity is a **Maxwell mixture**, not bare graphite.

### PSBT — closure verified

The port has no two-phase Euler-Euler solver, so it cannot compute the exit void
fraction, and nothing here pretends otherwise. What it has are the closures.
Upstream assembles `Kd = 0.5/Dh·(1−α_s)·ρ·max(|U|,minMagU)·f(Re)`, so evaluating
the **port's** friction factor against **upstream's** drag must reproduce the
hydraulic diameter — and the case declares exactly three, one per subchannel
zone:

| | |
|---|---|
| cells compared | 836 of 1170 (`alpha.vapour == 0`) |
| Reynolds range | 2.01e5 to 6.12e5 |
| worst deviation from a declared `Dh` | **6.15e-06** |

**With a control:** the 334 two-phase cells must *not* satisfy the relation,
because PSBT applies a `LockhartMartinelli` multiplier there. They don't — worst
deviation 0.53, five orders larger. Without that, the 6 ppm agreement would not
distinguish a correct closure from a relation loose enough to admit anything.

## Scope

None of this is validation against experiment. The two `k_eff` cases compare
against another code's published result on identical inputs; the gFHR case is a
consistency check on conduction alone. Each test's own doc comment states its
methodology, its measured numbers and what it does **not** cover.
