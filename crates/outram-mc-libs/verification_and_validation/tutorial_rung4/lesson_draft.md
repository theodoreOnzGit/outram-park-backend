# From lumps to lattices: LEU-COMP-THERM-008 and the light-water reactor

> **Research, education and V&V only.** This lesson is not for reactor
> operation, licensing, safety analysis or emergency response
> (`RESPONSIBLE_USE.md`). Its results are verification and comparison with a
> published critical experiment, not authority for any real plant.

**Status: DRAFT, phase 1 of GitHub #526** (rung 4 of the Monte Carlo ladder,
epic #520). Written 2026-10-04 to the shape in
[`docs/lessons/lesson-philosophy.md`](../../../../docs/lessons/lesson-philosophy.md)
and the Godiva exemplar (#521). AI-assisted draft, no human review yet.

What is missing before this can be published (phase 2): the demo rung
`lct008` (blocked by #521's multi-rung app), every history box (needs sources
the maintainer supplies, see "History" below), the mdBook includes that pull
code from source at build time, and exact-line permalinks at the site's build
commit. Code excerpts here are **pointers** (file and function), not copies.

---

## Hook

*Demo (phase 2): the `lct008` rung, thermal, 7 cm/s at 1 eV. One 15 × 15
assembly of rods in water fills the main view; neutrons are born fast in a
rod, zig-zag out into the water, slow down in a handful of collisions, and
wander back thermal into a rod. Zoom into one pin; the side panel shows the
two-panel σ(E) view (H-1 and U-238) with a marker riding each neutron's
energy.*

**The problem.** In the late 1970s Babcock & Wilcox built cores of
low-enriched UO₂ rods standing in borated water and adjusted them until each
was exactly critical. One of them is the benchmark this workspace calls
LEU-COMP-THERM-008. *Can we predict, from evaluated nuclear data alone, that
this arrangement of rods and water is critical?*

[HISTORY: the B&W critical experiments behind LEU-COMP-THERM-008 (year, place,
purpose) — needs source. Candidate: Baldwin et al., BAW-1484-7 (1979), OSTI
5536809; whether those are the exact LCT-008 cores is unverified. The ICSBEP
handbook is licence-restricted and is not quoted.]

**Bridge from rung 3.** Lumps work: putting the uranium in lumps let fast
neutrons escape into the moderator before they met the U-238 resonances. Now
swap the graphite for **water**, and the lumps for **rods** set in a regular
**lattice**. That is the light-water reactor.

### History box (placeholders, nothing here is stated as fact yet)

- [HISTORY: the naval reactors programme under Hyman Rickover chose
  pressurised light water for submarine propulsion because water is both
  moderator and coolant and the core is compact — needs source.]
- [HISTORY: the S1W land prototype at the National Reactor Testing Station,
  Idaho (1953) and USS Nautilus (S2W), first underway on nuclear power in
  January 1955 — needs source.]
- [HISTORY: Samuel Untermyer II proposed letting the water boil in the core;
  the BORAX experiments at Argonne (1953–1955), including the deliberate
  destructive excursion of BORAX-I (1954) and BORAX-III supplying power to
  Arco, Idaho (1955), showed a boiling core is self-limiting through its void
  feedback — needs source. Candidate primary: US Patent 2,936,273
  (Untermyer, "Steam forming neutronic reactor and method of operating it").]
- [HISTORY: Shippingport Atomic Power Station, the first full-scale civilian
  nuclear power station in the US, a PWR from the naval reactors programme,
  first power in December 1957; later the Light Water Breeder Reactor core
  (1977–1982) — needs source.]
- [HISTORY: how the PWR and BWR lines became today's light-water power
  reactors — needs source.]

The candidate source list is posted on #526 for the maintainer.

---

## Step 1 — Why is water such a good moderator?

**Question.** Rung 2 needed about a hundred collisions with carbon to slow a
fission neutron to thermal. Water does it in a rod pitch of about a
centimetre. What is different?

**Shortest answer.** Hydrogen's nucleus has almost exactly the neutron's
mass, so one head-on collision can stop the neutron dead. On average a
neutron loses a fixed fraction of its *logarithmic* energy per collision, and
for hydrogen that fraction is the largest any nucleus allows.

**Formula.** For elastic scattering off a nucleus of mass ratio A (isotropic
in the centre of mass, target at rest), the neutron keeps between αE and E,

    α = ((A − 1)/(A + 1))²

and the mean lethargy gain per collision is

    ξ = 1 + α ln α / (1 − α)        (ξ → 1 as α → 0)

so the number of collisions from 2 MeV to 0.025 eV is n = ln(2×10⁶ / 0.025) / ξ.

**Computed from the tapes' own AWR** (MF=1 MT=451 header of each
ENDF/B-VIII.0 file in `reference-data/endf/`, computed 2026-10-04):

| nuclide | tape | AWR | α | ξ | collisions 2 MeV → 0.025 eV |
|---|---|---|---|---|---|
| H-1 | `n-001_H_001-ENDF8.0-Beta6.endf` | 0.9991673 | 1.7 × 10⁻⁷ | 0.99999730 | 18.2 |
| H-2 | `n-001_H_002-ENDF8.0.endf` | 1.9968 | 0.1106 | 0.7261 | 25.1 |
| C-12 | `n-006_C_012-ENDF8.0.endf` | 11.89365 | 0.7138 | 0.1591 | 114.4 |
| O-16 | `n-008_O_016-ENDF8.0.endf` | 15.85751 | 0.7768 | 0.1210 | 150.4 |
| U-238 | `n-092_U_238.endf` | 236.0058 | 0.9832 | 0.00845 | 2153 |

α for H-1 is not exactly zero: the tape's neutron-mass ratio is 0.99917, so
α = 1.7 × 10⁻⁷. The "α = 0, ξ = 1" of the textbooks is this to six figures.

**Animation (phase 2).** A neutron bouncing off a hydrogen nucleus versus an
oxygen nucleus, with the outgoing-energy band [αE, E] drawn as a bar under
each; a lethargy axis where each collision is a step, H steps of length ~1,
O steps of ~0.12. Predict prompt first: *which nucleus in H₂O does most of
the slowing down?*

**Code walk.** Where the energy loss happens:
[`code_walks/collision_path.md`](code_walks/collision_path.md), from
`transport_history_vr` to `free_gas_elastic_scatter_dbrc`. **There is no
function that computes ξ.** ξ and the 18 collisions *emerge* from repeating
one elastic collision; the code only samples the kinematics.

**Measured check.** ~~None for hydrogen yet.~~ **Measured 2026-10-05
(GitHub #532), code at `6faff1ed8`:** `tests/elastic_xi_h1_o16.rs` samples
4 000 000 collisions per energy through `free_gas_elastic_scatter_dbrc` at
1 keV, 10 keV, 100 keV and 1 MeV (above H(H₂O)'s 10 eV S(α,β) cutoff and
400 kT), isotropic CM cosines, criterion |⟨ln E/E′⟩ − ξ| ≤ 4 sem fixed before
running. H-1: 0.99994 to 1.00045 (sem 5.0e-4, worst 0.9 sem; with DBRC removed,
worst 1.7 sem) against ξ = 0.9999973. O-16: 0.120932 to 0.120987 (sem 3.6e-5,
worst 1.3 sem) against ξ = 0.1209800. All twelve points pass. The rung-2
graphite check (`examples/graphite_energy_decrement.rs`) is the S(α,β)-range
counterpart.

**Predict.** *If hydrogen slows neutrons this well, should a reactor just use
as much water as possible?* (Step 5 answers it.)

---

## Step 2 — Hydrogen in water is not a free proton

**Question.** Below a few eV the neutron's energy is comparable to the
energy of the water molecule's vibrations and rotations. Does the free-atom
picture of step 1 still hold?

**Shortest answer.** No. The proton is bound in a molecule. A slow neutron
sees the molecule, not a lone proton: it can gain energy from the molecule's
motion as well as lose it, and the bound proton scatters more strongly.
That is described by the thermal scattering law S(α, β) for H in H₂O.

**Formula.** In the limit E → 0 the bound-atom cross section is the free
cross section scaled by ((A + 1)/A)²: about 81.8 b per H in H₂O against
20.4 b for the free atom. Quoted from the module documentation of
`crates/outram-mc-libs/src/material/thermal.rs` (lines 7–8 at `3c41d99f5b`).

**Animation (phase 2).** Neutrons at 0.1 eV in water: some gain energy
(up-scatter), most lose a little; the energy histogram settles into a
Maxwellian at 293.6 K instead of piling up at zero.

**Code walk.** The fork in the collision path: below the table's cutoff
`Nuclide::sample_thermal` returns the bound-atom outgoing energy and cosine;
otherwise free gas below 400 kT, target at rest above it. See
[`code_walks/collision_path.md`](code_walks/collision_path.md) ("the forks").
The table itself is built at load time by
`ThermalScattering::from_endf_file` on `tsl-HinH2O.endf` (MAT 1,
ENDF/B-VIII.0), the workspace's THERMR port.

**Measured check (code-to-code, integral).** On the LCT-008 lattice the
outram-mc ENDF route (its own THERMR kernel) and the outram-mc route that
reads the Rust NJOY port's ACE thermal table agree: route 4 − route 1 =
−8 ± 9 pcm and route 5 − route 1 = −3 ± 8 pcm (96 seeds each),
`verification_and_validation/icsbep/five_route_keff_2026_09_29.md`, "Results
— after the OpenMC-parity audit", 2026-09-30, `0414bc8277`. The fix that
brought route 4 there (#459, the S(α,β) cutoff) moved the lattice by
−40.0 ± 17.5 pcm paired, with the **opposite sign to the prediction written
before it** (P7 in the same record).

**Predict.** *Water is both moderator and absorber (H-1 captures, and this
water carries boron). What happens to the balance if we pack the rods
closer together, or spread them apart?*

---

## Step 3 — Rods in a lattice: how does the code describe repeated structure?

**Question.** The LCT-008 core has thousands of identical fuel rods. Does
the code describe each one separately?

**Shortest answer.** No. It describes one **pin** once (a *universe*: fuel
cylinder, clad, water), places copies of it on a grid (a *lattice*), and
places copies of the lattice on a bigger grid. LCT-008 is three levels deep:

    pin universe          r < 0.514858 UO₂ | < 0.602996 Al-6061 | water
      → 15 × 15 assembly lattice, pitch 1.63576 cm   (22 distinct maps)
        → 7 × 7 core lattice, pitch 24.5364 cm        (assembly or water tile)
          → inside a vacuum cylinder r < 76.200 cm, |z| < 81.662 cm

(The numbers are the committed `mit-crpg/benchmarks` cards, parsed at run
time, `examples/common/lct008_model.rs`; described in the module doc of
`examples/lct008_keff.rs`.)

**Formula.** Finding which lattice element contains a point is integer
division:

    i = floor((x − x_ll) / pitch_x),   j = floor((y − y_ll) / pitch_y)

and the point is then moved into that element's own frame (subtract the
element centre) before looking inside the pin. One more subtlety the code
must get right: OpenMC writes lattice rows from the **top** (max y) down; this
code indexes from the bottom up, so the rows are reversed on reading.

**Animation (phase 2).** A point dropped on the core: highlight the core
tile, zoom, highlight the assembly element, zoom, show the local frame and
the pin's rings; the "address" (core i, j → assembly i, j → ring) printed
beside it.

**Code walk.** [`code_walks/lattice_model_build.md`](code_walks/lattice_model_build.md)
(the XML cards → `RectLattice` → `Geometry`) and
[`code_walks/lattice_locate_and_tracking.md`](code_walks/lattice_locate_and_tracking.md)
(`Geometry::locate` descending through the levels).

**Measured check (verification).** `check_geometry` in
`common/lct008_model.rs` compares `Geometry::locate` with an independent
hand-written tile-index predicate at 200 000 random points and requires
exact agreement, and proves that the two row conventions give exact
y-mirror images (50 000 points), so k cannot depend on that choice. Volume
shares from the check: water 68.9 %, fuel 22.6 %, clad 8.5 %
(`five_route_keff_2026_09_29.md`, "Cases").

**Predict.** *A neutron flying between rods crosses a cylinder that belongs
to a pin universe, which sits in a lattice element that has been shifted.
In which frame do you compute the surface normal when it crosses?* (The case
study below is what happened when the answer was wrong.)

---

## Step 4 — Tracking through a lattice

**Question.** Once the neutron is located, how far can it fly before it
leaves its cell?

**Shortest answer.** The distance to the nearest boundary is the smallest of
the distances to every surface of its cell **at every level** (the pin's
cylinders, the lattice element's edges, the assembly's edges), each computed
in that level's own frame. If the sampled collision distance is shorter, it
collides; otherwise it moves to the boundary, crosses it, and is located
again.

**Formula.** d = min(d_collision, d_boundary), with d_collision = −ln ξ / Σ_t
(rung 1) and d_boundary = min over levels and surfaces.

**Code walk.** [`code_walks/lattice_locate_and_tracking.md`](code_walks/lattice_locate_and_tracking.md):
`transport_history_vr` → `Geometry::distance_to_boundary` →
`Geometry::cross_surface_in_frame`.

**Measured check.** See the case-study box: the frame of the crossing was
wrong once, and it cost +2300 pcm.

---

## Step 5 — How much water? Under- and over-moderation

**Question.** Step 1 said hydrogen slows neutrons superbly. So why not just
use more water?

**Shortest answer.** Water also absorbs. More water per rod means fewer
neutrons lost in the U-238 resonances while slowing down (resonance escape
`p` rises) but more thermal neutrons absorbed in the water instead of the
fuel (thermal utilisation `f` falls). k∞ = η f p ε has a maximum. A lattice
with less water than the maximum is **under-moderated**; more is
**over-moderated**.

**Formula.** The four-factor formula of rung 2, k∞ = η f p ε, with the
moderator-to-fuel volume ratio

    V_m / V_f = (pitch² − π r_clad²) / (π r_fuel²)

as the knob. For LCT-008, V_m/V_f = 1.841 (p/d = 1.356).

**Predict (the reader's, and ours).** Our expected outcome was written down
before running, in
[`pitch_sweep.md`](pitch_sweep.md) ("Prediction", 2026-10-04 22:10 +08:00):

- k∞ rises, peaks, then falls;
- `p` rises and `f` falls monotonically;
- with the case-1 borated water, the peak sits at V_m/V_f ≈ 2–3;
- LCT-008 is slightly under-moderated, within ~1000 pcm of the peak.

*Reader: before you look, where would you put the peak, and on which side
is LCT-008?*

**And then:** what does the dissolved boron do to that answer?

**The calculation.** `examples/lct008_pitch_sweep.rs`: one LCT-008 rod in a
reflective square cell (an infinite lattice), k∞ and the six factors at 11
pitches, ENDF/B-VIII.0 at 293.6 K, H₂O S(α,β), URR and DBRC on.

**Result** (`pitch_sweep.md`, 2026-10-04, seed 1, 5000 × [50 + 200],
11-nuclide tier, figure `figures/kinf_vs_moderator_ratio.png`):

| water | where k∞ peaks | k∞ at LCT-008's pitch | LCT-008 is |
|---|---|---|---|
| case 1, 1511 ppm boron | 1.35–1.45 cm (V_m/V_f ≈ 0.8–1.15), k∞ ≈ 1.119 | 1.06403 ± 0.00117 | **over-moderated**, ~5500 pcm below the peak |
| boron removed (ablation) | ≈ 1.75 cm (V_m/V_f ≈ 2.3), k∞ ≈ 1.351 (OpenMC) | 1.34838 ± 0.00112 | slightly under-moderated, within ~400 pcm |

- **Our prediction was wrong where it mattered.** We put the borated peak
  at V_m/V_f ≈ 2–3, with LCT-008 just under-moderated. Both codes put the
  peak near V_m/V_f ≈ 1, with LCT-008 well over it.
- **What the prediction got right was the clean-water picture.** We had
  underestimated what 1511 ppm of boron does to water.
- **p and f behave as the formula says**: p rises from 0.466 to 0.933 and f
  falls from 0.878 to 0.234. ε also falls, from 1.47 to 1.10. That is more
  than we predicted: at tight pitch, fast neutrons reach the next rod before
  the water.
- **OpenMC on the same model agrees without trend.** Mean difference −53 pcm;
  three of 11 points sit at 2.0–2.4σ of the single-run σ. That is mild
  tension, shown as it is (verification, not validation).

*Caution: this is k∞ of an infinite pin lattice, not the finite core.*

**Animation (phase 2).** A pitch slider on a pin cell; the curve k∞(V_m/V_f)
with the reader's point, and the f and p bars moving in opposite directions.
Per the "no lagging" hard rule, a slider move starts a background run and
streams its estimate; the UI never blocks.

**Code walk.** The factors are tallies, not functions of the geometry:
`run_keff_reactor_physics` in `src/physics/reactor_physics.rs` scores flux,
absorption and ν-fission by material and energy group with a track-length
estimator and forms η, f, p, ε from the rates.

---

## Case study — the LCT-008 case differential: test the assumption the result rests on

*A dated box. This is history of how a number moved, not the current result.*

**September 2026.** LCT-008 case 1 came out at **k = 1.02950 ± 0.00061, +2950
± 61 pcm** against a measured critical experiment (10 000 × [250 + 400],
2026-09-11; `examples/lct008_keff.rs` module doc, "Supersedes";
`verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`,
Interpretation 18). A first explanation, from coverage, blamed **U-238
resonance escape**: this was the only reproduced benchmark that is thermal
and U-238-dominated and lumped, and a quantitative prediction from the FHR
pebble (+3200 pcm) matched to 8 %.

**The test that broke it.** LCT-008 ships several configurations that are
each critical on their own and share one pin cell; they differ in how the
poison is supplied:

| case | soluble B-10 | poison rods | Δk (2026-09-11) |
|---|---|---|---|
| 1 | 1511 ppm | none | +2950 ± 61 pcm |
| 2 | 1335.5 ppm | none | +2271 ± 61 pcm |
| 8 | 794 ppm | 144 pyrex | +1713 ± 60 pcm |

Same pitch, pellet, clad and fuel, so `p` is the same in all three, and an
error in `p` must give the **same** Δk. The spread was **1237 ± 86 pcm,
14σ**. The resonance-escape attribution was refuted by the benchmark's own
other cases.

**What it actually was.** On 2026-09-14 a defect in surface crossing was
found: `Geometry::cross_surface` was handed a *global* position for a
cylinder that lives inside a *translated* lattice universe, so the normal was
computed about the wrong centre and the particle could be nudged back into
the cell it was leaving. A paired run at matched settings (3000 × [80 + 150])
gave +2341 ± 188 pcm before the fix and +37 ± 189 after: **−2304 ± 267 pcm,
8.6σ** (`lct008_keff.rs` module doc; commit `2655d83fa1`). The error scales
with how much pin surface a neutron crosses, which differs between the
configurations: that is what produced the spread.

**A second lesson in the same record.** A later section of that file credits
the collapse to the H-in-H₂O kernel fix (#188, `4d074ea8b9`), comparing a
2026-09-13 run with a 2026-09-16 run. Both fixes landed between those two
runs; the paired A/B was made after the kernel fix and isolates the geometry.
The kernel's own share is not resolved (−324 ± 227 pcm, unpaired). The file
was corrected on 2026-10-04 (see the CORRECTED note in
`examples/lct008_keff.rs`). An attribution made across two changes is not a
measurement of either.

**Moral.** The prediction that matched to 8 % was a coincidence. The
assumption the result rested on ("it is resonance escape") was testable with
data already in hand, and the test is what found the truth.

---

## Step 6 — Is LCT-008 critical? The result to quote

**Quote this:** `crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff_2026_09_29.md`,
"Results — after the OpenMC-parity audit" (2026-09-30, routes 3/4/5 at
`0414bc8277`), **route 4 = outram-mc reading ENDF/B-VIII.0 directly**, 96
seeds × 10 000 × [250 + 400]:

**k_eff = 1.00236 ± 0.00008 (+236 ± 8 pcm)** against the critical reference
k = 1.

Side by side, **OpenMC on NJOY2016 ACE (route 1): k = 1.00244 ± 0.00005, +244
± 5 pcm**; difference −8 ± 9 pcm (−0.8σ). The two codes agree with each
other; the shared +240 pcm offset from 1 therefore points at what they share
(the evaluated data and the model), not at either code's transport.

How to read it:

- **± is the standard error of the mean over 96 independent seeds**
  (seed sd / √96; the single-run sd is 74 pcm on route 4).
- **The reference is k = 1 by construction**: an ICSBEP critical is reduced
  to a model with k = 1. **No handbook uncertainty is quoted**, because the
  ICSBEP handbook is licence-restricted (`DATA_POLICY.md`) and is not in the
  repository, the corpus or the local notes. So "+236 pcm" is a distance from
  1, not a number of σ from the experiment.
- **Deliberate liberty: 11 nuclides.** The study runs the `--cheap-nuclides`
  tier (H-1, B-10, O-16, U-234/235/238, Al-27, Si-28/29/30, Mn-55). The 24
  others the cards name (B-11 in the water; Mg, Ti, Cr, Fe, Cu, Zn in the
  clad, 1.86 % of its atoms) are dropped, not renormalised, identically on
  both codes. `lct008_keff.rs` runs all 36 by default (35 are in case 1);
  ~~the worth of the dropped nuclides on the lattice has not been measured in
  this record.~~ **Measured 2026-10-05 (GitHub #533, code `8b17079bc`):**
  ~~16 full-model seeds on route 4 give +171 ± 19 pcm against the 11-nuclide
  +236 ± 8 (96 seeds), a worth of −65 ± 21 pcm (3.2σ; paired over seeds
  1–16, −35 ± 26).~~ **Extended the same day to 32 seeds:** 32 full-model
  seeds on route 4 give +161 ± 12 pcm against the 11-nuclide +236 ± 8 (96
  seeds), a worth of **−75 ± 14 pcm** (5.4σ; paired over seeds 1–32,
  −50 ± 16). That is negative as predicted (−100 from capture, range −40 to
  −250; −70 predicted for the 32-seed result). The record is "Later
  measurements" in `five_route_keff_2026_09_29.md`.
- Route 3 (outram-mc on NJOY2016 ACE) is the one cell outside 2σ of OpenMC:
  −18 ± 8 pcm (2.3σ). It is shown, not hidden.
- "Route 1" in that record is OpenMC, not us.

**Run it yourself.**

```text
cargo run --release -p outram-mc-libs --features endf-pebble-cases \
    --example lct008_keff -- --cheap-nuclides
```

**Modify.** `--case 2` and `--case 8`: three critical configurations, three
k's that should agree with each other. **Create.** Run
`--example lct008_pitch_sweep` at your own pitches with `--no-soluble-boron`
and find where the unborated lattice peaks.

---

## Next rung

Rung 5: TRISO particles in a pebble, then HTR-10 — double heterogeneity, where
the "lumps" are themselves full of smaller lumps, and surface tracking gets
expensive enough that **delta tracking** is worth teaching.

---

## Deliberate liberties (this page)

| liberty | why | cost |
|---|---|---|
| 11-nuclide tier in the quoted result | the OpenMC NJOY2016 library was built for it, so both codes run the same model | ~~not measured on the lattice~~ ~~−65 ± 21 pcm (16 full-tier seeds)~~ −75 ± 14 pcm (32 full-tier seeds, 2026-10-05, #533) |
| k = 1 reference with no uncertainty | handbook is licence-restricted | readers cannot judge σ from experiment |
| pin cell for the pitch sweep is an infinite lattice | isolates moderation from leakage | k∞ ≠ k_eff; the real core leaks |
| pitch sweep on the 11-nuclide tier | the OpenMC library carries only those; the 36-nuclide default was run at 3 pitches, differences ±250 pcm, unresolved | on the pin cell not measured; on the lattice ~~−65 ± 21~~ −75 ± 14 pcm (#533) |
| pitch sweep keeps case-1 water (1511 ppm boron) | the actual critical configuration | the unborated optimum differs; `--no-soluble-boron` measures it |
| 293.6 K for the sweep, not 296 K | the LCT-008 model's own `TEMP_K`, the tabulated temperature of `H(H2O)` | none intended; no thermal interpolation needed |

## Appendix — code walks and call tree

- [`code_walks/lattice_model_build.md`](code_walks/lattice_model_build.md)
- [`code_walks/lattice_locate_and_tracking.md`](code_walks/lattice_locate_and_tracking.md)
- [`code_walks/collision_path.md`](code_walks/collision_path.md)
- Exhaustive call tree: [`code_walks/call_tree.md`](code_walks/call_tree.md)

## Checklist (lesson-philosophy.md)

- [ ] Opens on a demo and a problem — demo is phase 2.
- [x] Every step: question → answer → formula → animation → code walk → check → predict (animations described, not built).
- [x] Adds one piece of physics over rung 3 (hydrogen, S(α,β), lattices); analog transport.
- [x] Code walks generated by the tool where it resolves, hand-filled hops labelled.
- [x] Every number quoted from a record (file, date, commit) or computed here with settings.
- [x] Verification and validation named correctly; deliberate liberties listed.
- [x] History left as visible placeholders.
- [ ] Demo rung linked both ways; truthful speed; phone width — phase 2.
- [ ] Banner, review stamp, "doesn't tally" button — banner done; the rest at publication.
