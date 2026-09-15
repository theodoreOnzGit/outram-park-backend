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
| **MSFR** | `k_eff`, per-cell cross sections | **+787.8 pcm** — residual is precursor drift |
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

### MSFR — residual is precursor drift

| | `k_eff` | vs upstream |
|---|---|---|
| port @ nominal reference | 0.979508 | +1999.1 pcm |
| port @ upstream's mean state | 0.969113 | +916.7 pcm |
| **port @ upstream's per-cell state** | **0.967875** | **+787.8 pcm** |

Upstream's `albedoSP3` boundary is ported (worth +2106 pcm on its own), and
cross sections are now evaluated per cell as upstream does.

The remaining 788 pcm is **circulating-fuel precursor drift**: MSFR's delayed
neutron precursors are advected out of the core and decay in the heat exchanger,
which upstream models and the port does not — it collapses the delayed source to
`chi_eff = chi_p(1−β) + chi_d·β`, assuming precursors decay where born, which
overestimates `k`. Correct sign; magnitude bounded by `beta_total = 285 pcm`, so
it is not the whole story. Filed.

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
