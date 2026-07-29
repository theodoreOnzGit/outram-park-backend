# Scoping: transforming OUTRAM PARK into a Type I digital twin

**Date:** 2026-07-29 · **Status:** scoping analysis, not an approved plan
**Relationship to `docs/outram-park-dt-plan.md` (2026-07-13):** that document is
the *tactical* plan for `tampines` and `outram-park-digital-twin-engine` and
still stands. This one is the *strategic* capability roadmap above it. One
revision: §2 below supersedes its "reusable frameworks only" premise, per a
maintainer decision on 2026-07-29 that simulator binaries may live in this repo.

> **What this is.** An honest gap analysis between what OUTRAM PARK is today and
> what a Type I (real-time, operator-training) digital twin requires, with an
> effort model and a phased route.
>
> **What this is not.** A commitment, a schedule, or a validated estimate. Effort
> figures are engineering judgement, not measurements. Nothing here has been
> built or tested; every "have" claim below was checked against the tree at
> `develop` = `ba0e55a`, and every "absent" claim was checked by search.

---

## 1. Target definition

Following the taxonomy in
`crates/kovan-literature/open/reports/nuclear-digital-twins-and-shadows-review.md`:

| Class | Coupling | OUTRAM PARK position |
|---|---|---|
| Digital **model** | none — offline physics | **where we are today** |
| Digital **shadow** | one-way, plant to model | not pursued |
| Digital **twin** | bidirectional | the target below |

**"Type I"** here means a *real-time, human-in-the-loop training simulator*:
physics that runs at or faster than wall-clock, an operator-facing interface, an
instructor station with malfunction insertion and snapshot/restore, and a
defensible V&V basis. It does **not** mean a plant-connected operational twin —
that remains prohibited by `RESPONSIBLE_USE.md` and is out of scope permanently.

---

## 2. Where the simulator lives

**Maintainer decision, 2026-07-29: simulator binaries may live in this
repository.** This revises the "reusable frameworks only" framing of the
2026-07-13 plan, which anticipated that reactor simulators would be *example
applications* destined for a separate parent repo (`outram-park`) that does not
yet exist.

Practice had already outrun that framing. As of `develop`, this repository ships
12 binary targets and 10 simulator-shaped examples, including:

| Target | Kind | Crate |
|---|---|---|
| `ciet_educational_simulator_v2` | bin | `outram-park-digital-twin-engine` |
| `ciet_v2_opcua_client` | bin | `outram-park-digital-twin-engine` |
| `htgr_sim_v1`, `fhr_sim_v2` | examples | `outram-park-digital-twin-engine` |
| `fhr_sim_v1` | example | `tampines-steam-tables`, `teh-o-prke` |
| `triso_simulator` | example | `boon-lay` |
| `mc_studio`, `mesh_studio` | examples | `outram-blender`, `outram-park-fork-cfmesh` |
| `pimpleFoam`, `rhoCentralFoam`, `gen_foam`, `blockMesh`, … | bins | `outram-foam-cli` |
| `kovan`, `kovan-tui`, `njoy-tui`, `outram-mc-tui` | bins | KOVAN, njoy, outram-mc |

**Consequence for this roadmap: the Type I DT can be built here directly**, as a
binary crate alongside the frameworks it consumes. It does not wait on the outer
repo existing, and no part of the plan below is blocked on that. The outer
`outram-park` repo remains useful later for lessons, papers, and per-plant
deliverables, but it is not on the critical path.

The framework/application distinction survives as a *design* discipline rather
than a repository boundary: physics, real-time infrastructure and
instructor-station primitives belong in library crates so they stay reusable and
independently testable; a specific plant's HMI layout, scenario library and V&V
dossier belong in its binary. That is the same separation the CIET v2 simulator
already observes.

---

## 3. Current state (verified)

### 3.1 Already real

- **`outram-park-digital-twin-engine` — CIET Educational Simulator v2.** A
  working real-time loop at test-loop scale: wall-clock pacing, fast-forward and
  slow-motion, timestep clamped to 0.1 s for Courant stability, coarse (8-node)
  vs fine (15-node) heater mesh switching, an OPC-UA server, and
  malfunction-shaped controls (`CtahBranchBlocked`, `DhxBranchBlocked` —
  "as if a valve were shut"). **This is already a Type I DT for one loop.**
- **`tuas_boussinesq_solver`** — 164k LOC source, 37k LOC tests, validated
  against the CIET facility, journal-published (Ong, Xiao & Peterson 2025,
  doi:10.1016/j.jandt.2025.03.006).
- **`tampines-steam-tables`** — 81k LOC IAPWS-IF97 plus steam-turbine equations.
  Balance-of-plant fluid properties are done.
- **`teh-o-prke`** — six-group precursor point kinetics, `decay_heat.rs` (seven
  precursor groups), reactivity feedback.
- **`chem-eng-real-time-process-control-simulator`** — PID and transfer functions
  built for real-time loops.
- **Source term, fuel-side** — `boon-lay/src/triso_atops_fork/` (GPL-3.0 port of
  INL's MIT-licensed TRISO-ATOPS, upstream commit `de374c8`): Booth
  equivalent-sphere, breakthrough and graphite-attenuation release models,
  Arrhenius diffusion in kernel/matrix/SiC, transient variants, and
  circulating/plate-out/clean-up activity bookkeeping. Verified against upstream.
- **Second, independent source-term method** — the Lagrangian Walk-on-Spheres
  Monte Carlo path through buffer/IPyC/SiC/OPyC, GPU-accelerated. Cross-check
  against the Eulerian ATOPS path is already planned as bead `op-b4a.4.10`.
- **Nuclide inventory** — `outram-mc-libs/depletion` (CRAM, decay chains).

### 3.2 Absent

Checked by search across `crates/`; nothing substantive found for any of these.

1. **Containment and severe accident** — no containment pressure/temperature
   response, no aerosol transport or deposition, no fission-product chemistry, no
   hydrogen behaviour. The source term currently stops at *release from fuel* and
   never reaches *release to environment*.
2. **Atmospheric dispersion and dose** — nothing.
3. **Construction costing and spacing** — geometry exists (`outram-blender`,
   `outram-park-fork-cfmesh`) but there is no BIM/IFC ingest, no quantity
   takeoff, no cost model, no 4D schedule linkage.
4. **Snapshot / restore / backtrack** — architectural, and the cheapest thing on
   this list to fix now and the most expensive to retrofit later. Every training
   simulator needs freeze, save, restore and replay.
5. **TRISO mechanical failure** — see §5.

---

## 4. Track A — real-time plant simulator

The Type I core. Scale what CIET v2 proves at loop scale up to a plant.

| Workstream | Effort (py) | Notes |
|---|---|---|
| Plant-scale nodalization + real-time TH/neutronics | 3–6 | The hard one. CIET is tens of nodes; a plant is 10^2–10^3 across multiple loops |
| Balance-of-plant integration | 1–2 | Steam generator, turbine, condenser, feedwater. Physics largely exists; integration is the work |
| Control and protection logic | 1–2 | RPS/ESFAS, trips, interlocks, rod control. `chem-eng` is the substrate |
| Instructor station | 1–2 | Malfunction insertion, scenario scripting, freeze. Depends on snapshot/restore |
| Snapshot / restore / backtrack | 0.5–1 | **Do this first.** Full state serialization + deterministic replay |
| Operator HMI | 1–2 | egui stack proven three times over; fidelity target drives cost |
| V&V to ANSI/ANS-3.5 | 2–4 | Gated on a decision — see §7 |

**Track A subtotal: 10.5–19 py.**

---

## 5. Track B — fuel performance

Independent of Track A, **no policy blockers, no restricted-data problem**, and
it contains the one genuinely novel contribution available here.

### 5.1 Adoptable upstreams

| Code | Licence | Scope | Action |
|---|---|---|---|
| **OFFBEAT** | GPL-3.0 (LICENSE verified) | 3D FV fuel performance on OpenFOAM; LWR UO2/Zircaloy, MOX and fast underway | **Port** |
| **SCIANTIX** | MIT | 0D single-grain fission gas behaviour; embeddable as `libsciantix.a` | **Port or wrap** |
| BISON | Controlled nuclear code | Best TRISO capability | **Do not port.** Papers only |
| PARFUME | Not open | TRISO particle | **Do not port.** Theory manual only |
| FRAPCON / FRAPTRAN / FAST | Restricted | LWR | Public NUREG model docs only |

OFFBEAT is the highest-leverage item in this entire document. It shares our
licence, it is cell-centred FV solid mechanics on OpenFOAM — which
`outram-foam-basic-lib` already translates into Rust — and it comes from the same
`foam-for-nuclear` project as GeN-Foam, which is already being ported into
`outram-foam-appbuilder-lib`. The porting effort compounds instead of starting
cold.

> **Compliance note.** BISON and PARFUME are INL-controlled nuclear codes.
> Controlled status carries export-control implications; their source must never
> enter this public GPL-3.0 repository even if access is granted. Use the
> published model-basis documents for model *forms* only, and record provenance
> per the workspace GPLv3/attribution rule.

### 5.2 The TRISO mechanical gap

There is **no openly-licensed TRISO mechanical fuel performance code**. This is a
real hole in the open ecosystem, and OUTRAM PARK is unusually well placed to fill
it.

The gap has an exact address in the existing code. `FailureFractions` in
`triso_atops_fork/activities/source_terms.rs` is a plain **input** struct:

- `heavy_metal` — as-manufactured contamination. Correctly an input.
- `sic` — as-manufactured defective SiC. Correctly an input.
- `incremental` — **in-service particle failure. Currently assumed.**
- `incremental_sic` — **in-service SiC-only failure. Currently assumed.**

A mechanical performance model is precisely the thing that computes the last two,
as functions of burnup, temperature history and fast fluence. The seam already
exists; nothing downstream needs restructuring.

**Model content required:** internal gas pressure (kernel fission gas release
plus CO production for oxide kernels), multi-layer stress analysis with
thermal-expansion mismatch, anisotropic irradiation-induced dimensional change
and creep in PyC, Weibull statistical failure of SiC, IPyC cracking driving SiC
stress concentration, kernel migration (amoeba effect), and Pd attack / SiC
thinning. Then Monte Carlo over a particle population with distributed dimensions
and strengths — for which the workspace already has the machinery.

**Why it is tractable:** the model forms are public (PARFUME Theory and Model
Basis Report, INL/EXT-08-14497 Rev 1, freely downloadable), and the validation
data is public (the AGR irradiation programme). That second point matters — it
means Track B has **no restricted-data dependency at all**, unlike Track A.

| Workstream | Effort (py) |
|---|---|
| OFFBEAT port (LWR mechanical, FV substrate) | 1.5–3 |
| SCIANTIX port or wrap (fission gas) | 0.3–0.7 |
| TRISO mechanical model + AGR validation | 1.5–3 |

**Track B subtotal: 3.3–6.7 py.**

---

## 6. Tracks C and D

### Track C — source term to environment (**deferred**)

Containment response, aerosol and fission-product transport, atmospheric
dispersion, dose. **3–5 py** for containment/severe accident, **1–2 py** for
dispersion and dose.

**Deferred by maintainer decision, 2026-07-29.** `RESPONSIBLE_USE.md` currently
lists emergency response as prohibited use and stays as written; the maintainer
may amend later. Track A does **not** require that amendment — only Track C does.

### Track D — construction costing and spacing (**separate product line**)

BIM/IFC ingest, quantity takeoff, cost models, 4D schedule linkage. **2–4 py**,
with near-zero reuse from the physics stack. EPRI selected construction-sequence
simulation as one of two priority DT use cases, so the interest is well founded —
but it shares almost nothing with Tracks A–C and should be decided on its own
merits.

---

## 7. Decisions required before committing

1. **Emergency response scope.** Settled for now: deferred. Revisit only if
   `RESPONSIBLE_USE.md` is amended. Track C is blocked until then; Tracks A, B
   and D are not.
2. **Reference plant for Track A V&V.** ANSI/ANS-3.5 validation needs
   reference-plant data, and `DATA_POLICY.md` forbids operational facility data.
   **A training simulator validated against a real plant is therefore blocked.**
   The workable path is to anchor to a *published benchmark plant* instead. This
   decision gates the 2–4 py V&V workstream and should be made before Track A
   starts, not during it.
3. **Whether Track D is in scope at all.**

---

## 8. Recommended sequence

Do not attack this frontally.

1. **Snapshot/restore first** (0.5–1 py). It is cheap now, invasive later, and
   everything in the instructor station depends on it.
2. **Track B in parallel, starting with OFFBEAT.** It is unblocked, it has public
   validation data, it strengthens the physics under every other track, and the
   TRISO piece is publishable.
3. **Scale CIET v2 to one full simple plant** — an SMR, or the FHR for which
   simulator examples already exist — reusing nearly everything and requiring no
   severe-accident code. This is the shortest path to a genuinely useful
   training simulator: **roughly 4–6 py** rather than the full Track A total.
4. **Then** decide on Track C, having learned what the first plant cost.

---

## 9. Totals and estimate caveats

| Track | Effort (py) | Blocked? |
|---|---|---|
| A — real-time plant simulator | 10.5–19 | V&V anchor undecided |
| B — fuel performance | 3.3–6.7 | no |
| C — source term to environment | 4–7 | yes, policy |
| D — construction and costing | 2–4 | scope undecided |
| **Total if all four** | **~20–37** | |
| **Type I core only (A + B)** | **~14–26** | |

**These are conventional-development person-year estimates** and should be read
as order-of-magnitude. They are not calibrated to this workspace's actual
throughput, which is heavily AI-assisted. The workspace already records the data
needed to calibrate them — per-commit `API-Usage` trailers and the
`docs/historian/` KLOC-and-token reports — and any serious planning should derive
a local multiplier from that record rather than trusting the figures above.

Nothing in this document has been built. Treat it as a map, not a measurement.

---

## 10. Provenance

Open-literature sources only, per `DATA_POLICY.md`.

- PARFUME Theory and Model Basis Report, INL/EXT-08-14497 Rev 1 —
  <https://art.inl.gov/ART%20Document%20Library/INL%20ART%20Documents/EXT-08-14497_PARFUME_Theory_Manual_R1.pdf>
  (accessed 2026-07-29)
- AGR-5/6/7 Irradiation As-Run Predictions Using PARFUME, INL/EXT-21-64576 —
  <https://art.inl.gov/Rotating%20Files/64576%20AGR-567_PARFUME_As_Run.pdf>
  (accessed 2026-07-29)
- OFFBEAT — <https://gitlab.com/foam-for-nuclear/offbeat> (GPL-3.0; LICENSE file
  retrieved and confirmed 2026-07-29)
- SCIANTIX — <https://github.com/sciantix/sciantix-official> (MIT)
- TRISO-ATOPS — <https://github.com/IdahoLabResearch/TRISO-ATOPS> (MIT)
- BISON availability — <https://inlsoftware.inl.gov/product/bison> and
  <https://inl.gov/ncrc/code-descriptions/> (controlled nuclear code)
- PNNL-31427, TRISO Fuel: Properties and Failure Modes —
  <https://www.nrc.gov/docs/ML2117/ML21175A152.pdf>
- Internal: `docs/outram-park-dt-plan.md`, `docs/architecture.md`,
  `crates/kovan-literature/open/reports/nuclear-digital-twins-and-shadows-review.md`

**Beads:** `op-bj7` (Type I DT gap analysis epic), `op-bvf` (fuel performance
survey), `op-b4a` (boon-lay TRISO release simulator epic).
