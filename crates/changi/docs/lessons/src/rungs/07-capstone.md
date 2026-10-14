# Rung 7: capstone, HTR-10 air ingress from release to dose

> **Research, education and V&V only.** This is a **bounding** model case,
> chosen by the maintainer to be pessimistic, not a source term or a dose for
> HTR-10 or any facility. Its doses are properties of the model, never doses
> to real people, and nothing here is for emergency planning or response,
> licensing or any safety decision (`RESPONSIBLE_USE.md`).

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).
> [It doesn't tally](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Dispersion%20rung%207%20doesn%27t%20tally).

> **Demo:** [open this rung in the dispersion demo](../../../demos/dispersion/?rung=capstone): this rung's recorded results with their provenance; the chain itself is too heavy for a phone, so nothing is computed live. Every calculation runs in a background worker, so the page stays live; the demo's "What's happening here?" link opens this page.

## The problem

Rungs 1–6 built the chain one link at a time, each from a unit release. Now
put a real-shaped source term at the front of it. The case is
`sembawang`'s `htr10_air_ingress_kora_bound`, the maintainer's **bounding air
ingress** for HTR-10 (2026-09-30, gh:#434, #435, #438): the whole core held at
1400 °C for 140 h, the fuel's graphite matrix burned to bare silicon carbide,
the measured KORA oxidation failure added to the modelled fuel failure.
**What does the chain predict, from fuel to dose against distance, and which
link carries the answer?**

```text
 Liu & Cao Table 1 inventory          (changi::activity::inventory)
   + failure fractions                (HTR-10 design values, boon-lay PANAMA increment, KORA test)
   + a flat 1400 C hold               (sembawang::scenario, prescribed)
 -> TRISO-ATOPS release               (boon-lay, via sembawang::accident::release)   [other track]
 -> single-plume chi/Q, worst class   (buangkok::pydoseia::dispersion, rung 1)
 -> submersion + inhalation + groundshine  (buangkok, FGR-15 / FGR-11, rung 6)
```

---

## Step 1. Where does the release come from?

**Short answer.** From the fuel, by diffusion out of the particles that have
failed or will fail, computed by `boon-lay`'s port of INL's TRISO-ATOPS [(Ong, 2026)](#ref-ong2026boonlay). That
physics is the [TRISO-ATOPS track](../../triso-atops/index.html) (a side path
from here); this rung uses it as a black box with stated inputs.

**The inputs, each named in the example with its source:**

| Input | Value | Source |
|---|---|---|
| Inventory | 22 nuclides, kept where TRISO-ATOPS has the nuclide | Liu & Cao (2002) Table 1 |
| Hold | 1400 °C for 140 h, uniform | the KORA / JAERI test temperature (bounding choice) |
| `f_hm` (free uranium) | 3e-4 | Liu & Cao §2.1, HTR-10 design |
| `f_inc` (irradiation failure) | 5e-4 | Liu & Cao §2.1, HTR-10 design |
| SiC-only fractions | 1e-4, 3.6e-5 | **stand-ins**, the NP-MHTGR reference; no HTR-10 value exists |
| accident increment | `boon-lay` PANAMA increment + **1.2e-3** | KORA AVR 92/22 (an irradiated sphere in air at 1400 °C for 140 h, about 20 of 16 400 particles failed) |
| irradiation temperature | 776 °C | **stand-in** (gh:#297); the PANAMA increment depends on it |

**In the code.** From the example to the release driver:

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/sembawang/examples/htr10_air_ingress_kora_bound.rs::main) and follow its callees (▸) to [`accident_release_with_venting`](../../../code-review/#crates/sembawang/src/accident/release.rs::accident_release_with_venting). Each card opens the function's source and shows its review stamp.

**Predict.** A uniform hold never heats. TRISO-ATOPS moves activity out of the
core by **venting** as the coolant heats up. How much does a flat hold vent?

---

## Step 2. How the release leaves the core: venting, and a defect found by reading upstream

**Short answer.** For a uniform constant hold, everything that leaves the fuel
leaves the core: upstream TRISO-ATOPS has an explicit branch for it.

**The story, because it is the workspace's "read upstream first" rule in
action.** The first run of this example (2026-09-30) released **exactly
0 Bq**. It was first read as a limitation of TRISO-ATOPS (a heat-up venting
model has nothing to vent on a flat hold) and filed as gh:#446. Reading
upstream `trisoatops.py::accident_case` (commit `de374c8`) showed

```text
if not np.all(accident_temp == accident_temp[0,0,0]): ... coolant_release ...
else: frac = np.ones(np.size(times))
```

so a uniform constant hold vents **fully** at every sample. The port had
always called `coolant_release`. The branch was restored as
`Venting::Upstream`; the conservative `Venting::FullFlowThrough` must agree
with it on this case, and the example prints the two totals side by side
(it does not assert it; the comparison is read from the printout below).

**The forks** (`sembawang::accident::release::Venting`): `Upstream` (heat-up
venting with the uniform-hold branch), `FullFlowThrough` (all released
activity leaves), `Prescribed` (a caller's exchanged fraction, for instance
HTR-10's cavity ventilation, `Venting::gao_shi_htr10_cavity_ventilation`). See
[sembawang beyond the capstone](../ext/sembawang.md).

---

## Step 3. Which dispersion, and why not the puff train?

**Short answer.** `buangkok`'s steady single plume (rung 1), at the **largest**
`chi/Q` over the six classes at each distance, for a ground-level release, a
1 m/s wind at 10 m, the receptor on the centreline, the whole release blowing
over one point.

That is a **bounding** dispersion, chosen on purpose: no direction change, no
plume rise, no building wake, no depletion, no decay in transit. `changi`'s
puff train (rungs 4 and 5) would let the wind turn and decay act in flight,
both of which lower the dose; using it here would turn a bound into an
estimate. `sembawang::chain::pad_for_dispersion` is the hand-off for that
other use (`examples/npmhtgr_chain.rs`).

**In the code.**

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/sembawang/examples/htr10_air_ingress_kora_bound.rs::main) and follow its callees (▸) to [`master_equation_single_plume`](../../../code-review/#crates/buangkok/src/pydoseia/dispersion.rs::master_equation_single_plume). Each card opens the function's source and shows its review stamp.

---

## Step 4. Dose against distance

**Short answer.** Three pathways from rung 6, adult, FGR coefficients:
submersion (FGR-15), inhalation (FGR-11, breathing 3.33e-4 m³/s), and
groundshine (FGR-15) from activity deposited at once at pyDOSEIA's SRS-19
velocities and decaying over the 96 h dose window. Ingestion and plume shine
are **not** computed (rung 6: no transfer factors or photon data ship).

**In the code.**

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/sembawang/examples/htr10_air_ingress_kora_bound.rs::main) and follow its callees (▸) to [`submersion_dose`](../../../code-review/#crates/buangkok/src/pydoseia/dose.rs::submersion_dose). Each card opens the function's source and shows its review stamp.

**The numbers to quote.** Re-measured for this lesson, because the record in
the example's doc comment (2026-09-30) predates later changes to the release
chain on the TRISO-ATOPS track:

**Re-run for this lesson** (`develop` `d4428668be`, one core (core 12, `-j 1`), 2026-10-04; the example ran in 0.34 s after a 14.5 min
single-core build). **Every number reproduced the 2026-09-30 record** at its
printed precision; none is superseded, and the re-measurement is recorded in
the example's doc comment.

Release over the 96 h window (TRISO-ATOPS plus Liu & Cao's circulating
activity): **1.689e13 Bq**; `Upstream` and `FullFlowThrough` venting both
1.6888e13 Bq. Released fraction of the core: noble gases and iodine about
1.9e-4, Cs-137 1.02e-3, Cs-134 1.49e-3, strontium 1.9e-5, Ag-110m 2.49e-2.
Full-failure fraction 2.0004e-3, 60 % of it the KORA term.

Maximum dose at 400 m, adult, the largest `chi/Q` (class F,
2.857e-3 s/m³):

| Pathway | Dose | Largest contributors |
|---|---|---|
| Submersion (FGR-15) | 1.35 mSv | I-135 0.70, I-133 0.32 |
| Inhalation (FGR-11) | 41.4 mSv | I-131 15.8, I-133 6.06, Cs-137 5.81, Cs-134 5.52, Sr-90 3.45 |
| Groundshine, 96 h (FGR-15) | 23.4 mSv | I-133 5.90, Cs-134 5.22, I-131 4.38, I-135 3.65 |
| **Total** | **66.1 mSv** | |

Against distance (class F gives the largest `chi/Q` at every distance here):

| Distance | 400 m | 600 m | 800 m | 1 km | 1.5 km | 2 km | 3 km | 5 km | 10 km |
|---|---|---|---|---|---|---|---|---|---|
| Dose (mSv) | 66.1 | 33.5 | 20.8 | 14.3 | 7.38 | 4.78 | 2.67 | 1.33 | 0.54 |

The cavity-ventilation arm (`Venting::gao_shi_htr10_cavity_ventilation`)
releases 0.950 of the full-venting amount and gives 62.9 mSv at 400 m.

**Read these as properties of a deliberately pessimistic model**, a whole
core at 1400 °C, the worst stability class for four days in one direction,
no rise, no wake, no depletion, no decay in transit. They are not an estimate
of what HTR-10 would release or of any dose to anyone.

**Which link carries the answer?** Close in, inhalation (iodine, caesium,
strontium) and groundshine dominate; submersion by the noble gases is small.
The dose falls with distance roughly as `chi/Q` does, because every pathway
here is linear in the time-integrated concentration.

---

## Step 5. The check

**There is no pass criterion.** This is a bounding estimate, not a
prediction of anything measured, and the example says so. What can fail:

- **An ordering** (gh:#438): this release should sit **below** gh:#435's
  hand-coded 100 %-release bound, except silver, where both take 100 %.
- **An internal agreement**: `Venting::Upstream` and `FullFlowThrough` must
  give the same release on a uniform hold (step 2); printed, not asserted.
- **Context only, not validation:** Liu & Cao's Table 9 whole-body doses
  (0.077 mSv depressurisation, 0.20 mSv water ingress, at 0.25 km) are for
  **different** accidents, with other codes and measured site meteorology.
  They are not a comparator for this case.

**Under-counted, stated:** ingestion; plume shine; the nuclides screened out
for half-lives under 4 % of the transient; H-3, Xe-135m and Rb-88, which
TRISO-ATOPS's table lacks. **Conservative, stated:** class F-like worst class
at 1 m/s for 96 h with no direction change; ground release, no wake, no
depletion, no decay in transit; a whole-core 1400 °C hold.

## Use, modify, create

```bash
cargo run --release -p sembawang --example htr10_air_ingress_kora_bound
```

- **Modify.** Replace `Venting::FullFlowThrough`'s role by the cavity
  ventilation (`Venting::gao_shi_htr10_cavity_ventilation`) and predict the
  change first: the example's own record says three air changes already
  exchange 95 % of the core gas.
- **Create.** Feed the release through `sembawang::chain::pad_for_dispersion`
  and `changi::activity::dilution_factors` (rungs 4–5) with a wind that turns
  after a day, and compare the 400 m dose with the bounding plume's.

## Deliberate liberties

| Liberty | Why | What it may cost |
|---|---|---|
| Whole core at 1400 °C for 140 h | bounding choice | over-states the release by a factor not measured here |
| SiC-only fractions and irradiation temperature are stand-ins | no HTR-10 value exists | not measured |
| One node, uniform | a uniform hold | none for a uniform hold |
| Worst class at 1 m/s, one direction for 96 h | bounding | over-states dose; size shown by rung 2's spread between classes |
| FGR-11 (ICRP 26/30) with FGR-15 (ICRP 103) | the freely usable coefficients | mixed weighting schemes, stated |
| `f_hm` at the HTR-10 design value, no qualification band shown | the stored Kugeler (2017) band is unchecked on the TRISO-ATOPS track (gh:#383) | release is linear in `f_hm` for noble gases and halogens; the band's effect is not shown here |

## History

> **[HISTORY: needs a source]** The KORA heating tests and the German
> air-ingress programme are the history here. The example cites IAEA-TECDOC-978
> Table 5-7 and Kugeler (2017) Table 9 for the KORA result; neither is in the
> open corpus, so no history is told until the maintainer supplies a source.

## Call-tree appendix

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/sembawang/examples/htr10_air_ingress_kora_bound.rs::main). Each card opens the function's source and shows its review stamp.

**Back to the start:** the [dispersion demo](../../../demos/dispersion/?rung=plume) runs the same chain,
rung by rung. **Further:** [sembawang beyond the capstone](../ext/sembawang.md)
compares this case with an equal-power LWR.

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-ong2026boonlay" style="padding-left: 2em; text-indent: -2em;">Ong, T. K. C. (2026). <i>boon-lay: BOmbardment Open source Nuclide simulation Laboratory Algorithm for Yields (BOON LAY)</i>. GitHub. https://github.com/theodoreOnzGit/boon-lay</p>

<!-- references:end -->
