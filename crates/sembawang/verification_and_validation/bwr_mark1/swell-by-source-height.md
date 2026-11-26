# The swollen level by source height (gh:#1810, 2026-11-26)

> Research, education and V&V only. AI draft, unreviewed. Not validated.

## The defect

Reported by the maintainer on the Fukushima web demo (2026-11-26): when the
first node melts, "it suddenly turns the core blue". Peak cladding fell
from 2099 K to 1019 K between two screenshots, the level fell and the RPV
pressure rose.

**Reproduced headless** (`Plant::with_mechanisms(Mode::Historical,
Mechanisms::default())`, 1 s steps, printed every step around the first
relocation; a throw-away driver, not committed). At t = 13225 s one node in
36 relocates onto the core plate. Then:

| t (s) | RPV (MPa) | peak cladding (K) | relief (kg/s) | level the core sees (m above BAF) |
|---|---|---|---|---|
| 13225 | 8.12 | 2499 | 0 | 0.31 |
| 13226 | 8.15 | 2275 | 0 | 2.06 |
| 13227 | 9.93 | 2098 | 0 | 3.73 |
| 13228 | 11.47 | 1170 | 448 | 3.55 |
| 13229 | 12.16 | 701 | 522 | 3.37 |

The heat into the water reached 6.9 GW. The relief then vented about
570 kg/s for tens of seconds.

**Cause.** The plant fed the core the swollen level of `swollen_level`,
which is MELCOR CVH/FL RM §5.1.3 for **one** control volume. It divides
**all** the liquid below the collapsed level by `1 - alpha`, the lower
plenum included.

The relocated node's quench on the plate (about 120 MW, at the bottom of
active fuel, with the collapsed level 0.31 m above it) drove `alpha` to its
0.4 cap. The lower plenum's water was "swollen" by 1/0.6 into the core. The
uncovered core, at about 2500 K, was then counted as covered, and quenched.

The committed 30 h fixture is sampled every 600 s, so it never showed the
spike.

## The fix

Vapour raised at a height voids only the water above it
(`bwr_mark1::swollen_level_from_sources`).

- With `J(z)` the vapour flux through height `z` from every source at or
  below it, the local void is `alpha(z) = min(J(z)/v0, alpha_max)`.
- The swollen level `H` solves `∫0^H A (1 - alpha) dz = V_liquid`, on a
  1 cm grid.
- For uniform boiling this is MELCOR's mean `J/(2 v0)`. Stated difference:
  the cap applies locally, not to the mean.
- The core reports its pool sources by height (`CoreResult::q_pool_*`):
  - the lower plenum's debris, steel and wet head at the vessel bottom;
  - the core plate at the bottom of active fuel;
  - each covered row at its mid-height.
  - The core bottom's radiation boils the surface and voids no pool.
- `swollen_level` itself is unchanged. `tampines` ports it bit for bit, and
  `tests/swollen_level_vs_tampines.rs` pins that port.

This is a correction of the physics, not an ablation: no mechanism was
turned off.

**Why this form.** MELCOR's own BWR nodalisations put the lower plenum, the
core and the upper plenum in separate control volumes. Vapour raised in the
core does not pass through the lower plenum below it.

## Verification

**Unit (`the_swell_voids_only_the_water_above_each_source`).**

- No vapour: the collapsed level.
- The defect's case (plate 120 MW, collapsed level 0.31 m):
  - prediction: 0.31/0.6 = 0.517 m;
  - result: within 1 cm of it;
  - the whole-pool form gives 2.18 m for the same single step.
- Uniform boiling in six rows, mean void 0.1:
  - prediction: `L / (1 - J/(2 v0))`;
  - result: within 3 cm of it (the grid and the half-row source offset).
- Pass.

**Step-resolved plant (`the_first_relocation_does_not_quench_the_core`,
`long-tests`).**

- Criteria, written before the run and placed between the two behaviours:
  over the 60 s after the first relocation, peak cladding stays above
  2000 K and the RPV stays below 9 MPa.
- **Prediction** (written before the run): no core-wide quench, the RPV
  near 8 MPa, no relief-valve lift.

**Results (2026-11-26, release):**

| | before the fix | after the fix |
|---|---|---|
| first relocation | 13225 s | 13226 s |
| peak cladding, 60 s after | down to 701 K | **≥ 2310.3 K** (the hottest node fell; the next is the peak, and heats again) |
| RPV, 60 s after | up to 12.67 MPa | **≤ 8.310 MPa** |
| relief | ~570 kg/s for tens of s | one 2 s lift at ~370 kg/s |
| heat into the water, peak | 6.9 GW | **0.395 GW** |

Pass. One part of the prediction missed: there is a brief relief lift at
8.31 MPa, as the plate's quench steam passes up through the hot core. The
exit gas reaches ~1700 K for a few seconds, and that steam oxidises some
cladding. Both are physical.

## What it moved (recorded, not tuned)

**Lumped 30 h fixture (`headless_30h_lumped.csv`, re-blessed).**

| | before | after |
|---|---|---|
| H2 at 30 h | 470.774 kg | 338.039 kg |
| lower-head failure (600 s samples) | 19800 s | 20400 s |
| peak drywell | 0.812239 MPa | 0.774368 MPa |
| peak cladding | 2490.51 K | 2493.80 K |
| first relocation (600 s samples) | 13800 s | 13800 s |
| peak RPV | 8.287379 MPa | 8.287379 MPa |

The network fixture (`headless_30h.csv`) is not a regression pin
(maintainer, 2026-11-02, gh:#1110) and was left as it was.

**BSAF Unit 1, lumped plant to 26 h (`tests/bwr_mark1_vs_bsaf_figures.rs`,
re-pinned).**

| | before | after |
|---|---|---|
| RPV vs measured (2 points), mean / max | 3.4388 / 6.5826 MPa | **0.9590 / 1.5196 MPa** |
| drywell vs measured (33 points), mean / max | 0.0670 / 0.2573 MPa | 0.1581 / 0.3567 MPa |
| inside the nine-code envelope: RPV | 0.6154 | 0.1648 |
| drywell | 0.6703 | 0.3846 |
| **H2** | **0.9885** | **0.1034** |
| Xe | 0.8125 | 0.8750 |
| I | 0.8542 | 0.9167 |

**Interpretation.** The measured RPV agreement improved sharply. The drywell
and the code envelopes moved away, H2 most of all.

The H2 envelope agreement rested on the defect. The false quench drove steam
through a hot core, which oxidised.

With the fix, the model's H2 history is:

| t (h) | 4 | 5 | 6 | 8 | 12 to 24 |
|---|---|---|---|---|---|
| model (kg) | 218 | 337.5 | 337.9 | 337.9 | 337.9 to 338.0 |
| codes, min to max (kg) | 8.8 to 553 | 62 to 677 | 361 to 688 | 398 to 1250 | 398 to 1315 |

So the model is below all nine codes from 6 h. Oxidation stops by 5 h, once
most of the core has relocated.

**A second behaviour that rested on the defect: the melt ejection.** Before
the fix, the RPV depressurised at 4.958 h. That appears to have followed the
false quench's 12.6 MPa spike at 3.7 h. Now nothing depressurises Unit 1
before the lower head fails. The head fails at pressure, at 5.565 h
(`depressurised_h == lower_head_h`), and the melt is ejected at high
pressure (`fdi_hpme`, which `the_debris_falls_through_the_pedestal_water`
had asserted false). The debris through the pedestal water falls from 8.07
to 0.449 kg, and its steam from 74.75 to 48.37 kg.

Unit 1 is generally understood to have depressurised before the head
failed, so the depressurisation path (the main steam line's creep, the
SRVs' gaskets) is the other item for gh:#1810.

**The pins (2026-11-26).** Maintainer: "Ignore sembawang pins for now,
nothing to pin against". The root `CLAUDE.md` now says "No regression pins
until there is concrete V&V at unit-test level" (refined the same day:
"No pins until there are concrete AI v&v being done at unit test level").

- **Ignored:** the lumped fixture test and
  `compare::the_uncalibrated_baseline_is_pinned`. The baseline moved: H2 at
  25 h 470.75 → 338.04 kg; drywell peak 0.81294607 → 0.77562553 MPa; lower
  head 5.4839 → 5.5647 h; depressurised 4.9578 → 5.5647 h.
- **Printed, not asserted:** the BSAF numbers and the value pins inside
  six other tests. Their old values are kept in the code.
- **Still asserted:** conservation (UO2 to 1e-9, noble gas to 1e-12), the
  B4C cap, the hand-off identities and the first relocation's bounds.

Maintainer, 2026-11-26: "right physics before right answers". The fix
stands, and the H2 shortfall is the next item to investigate under gh:#1810.
Candidates:

- the steam reaching the relocating and relocated material after the first
  fall;
- the debris oxidation's steam supply in a dry plenum;
- whether MELCOR's later-phase H2 comes from the lower plenum, the core
  plate or the remaining standing rods.
