# `bwr_mark1`: the lower-head wall (gh:#1034, plan item F3), measured

> Research, education and V&V only. AI draft, not reviewed. Uncalibrated:
> nothing was tuned toward the BSAF band.

## What changed

Before, the lower head failed when dry debris in the plenum exceeded
`LOWER_HEAD_FAILURE_K` = 1500 K \[A\], a criterion on the **debris** temperature.

Now it fails on the **wall**. This follows the public description of MELCOR's
COR lower-head model (`crates/kovan-literature/derived/melcor-2.2-manuals-digest.md`
§F3; MELCOR 2.2 Vol. 2 §6.1 and §6.3; Vol. 1 COR_LHF and COR_VP). No MELCOR
source was used.

- **Wall:** one 1-D column of 304L steel (Kim, ANL-75-55, via
  `tuas_boussinesq_solver`), 0.22 m thick (MELCOR's generic BWR default, \[A\]),
  5 nodes, explicit conduction.
- **Inner face:** the debris through 1000 W/m²K (MELCOR's default, \[A\]),
  over the wall area that a spherical-cap debris bed touches (porosity 0.4,
  \[A\]). While the plenum is wet, the face boils against the water instead.
- **Outer face:** adiabatic.
- **Failure,** whichever comes first:
  - the inner node reaches 1273.15 K (MELCOR's TPFAIL default, \[A\]);
  - Larson-Miller creep strain reaches 0.18, with SA533B1 constants \[P\]
    (NUREG/CR-5582 via the manual) and stress from the vessel-to-drywell
    pressure difference plus the debris head.
- **Ablation:** `Mechanisms::without_lower_head_wall`, which is the old
  criterion. The model has it in both property paths.

## Prediction (written before the run) and result

**Predicted:** lower-head failure later by 0.3–1.5 h. The estimate was about
45 t of head steel heated from 560 to 1273 K (about 19 GJ) on about 7 MW of
debris decay power.

**Measured** (2026-10-29, x86_64, `--release`):

| | Before (`without_lower_head_wall`) | After |
|---|---|---|
| Lower-head failure | 5.399 h | **5.543 h** |
| In-vessel H2 at 25 h | 120.73 kg | 119.95 kg |
| Drywell peak, 0–25 h | 0.4746 MPa | 0.4753 MPa |

The ablation reproduces the "before" column exactly (pinned in
`compare::tests::the_uncalibrated_baseline_is_pinned`).

**The prediction missed on magnitude:** 0.14 h, not 0.3–1.5 h. The sign was
right.

**Why it missed** (from a step-by-step trace of the run):

1. The plenum stays wet until about 17 000 s, and the debris is quenched to
   about 568 K.
2. Once the plenum is dry, the debris heats at about 0.3 K/s.
3. The criterion is the **inner node**, a half-cell of 2.75 cm of steel. It
   tracks the debris about 80 K behind, long before the bulk of the wall
   warms. The wall's conduction time over 0.22 m is about an hour.
4. So the inner node reaches 1273 K about 500 s after the old criterion's
   1500 K debris temperature.

The estimate heated the whole wall as one lump, which is not what an
inner-node criterion measures. The model is doing what its description says;
the estimate was wrong. Creep did not decide: strain stayed below 1e-10,
because by the time the wall was hot the vessel was close to the drywell
pressure.

## Mesh study (wall nodes)

The inner-node criterion depends on the through-wall mesh, so the result was
measured at four meshes. 17 is the explicit-stability limit while the face
boils against water.

| Nodes | Lower-head failure |
|---|---|
| 3 | 5.558 h |
| 5 (default) | 5.543 h |
| 9 | 5.531 h |
| 17 | 5.537 h |

The spread is 0.027 h (about 100 s), and 5 nodes is within 0.007 h of 17.
The default stays at 5. The mesh error is negligible next to the 1.9 h miss
against the band.

## Interpretation

The wall alone does not close the lower-head miss. 5.54 h is still about 1.9 h
before the BSAF band opens (7.45 h, Tier B). In this run the time to failure is
set by **when the plenum dries and how fast the debris heats**, not by the
wall. That points at the items the digest ranks next:

- debris held in the core until the support fails, instead of the 2100 K
  whole-node slump (F4, #1035);
- the lower-plenum quench and its dryout limit;
- in-core radiation (F6).

### Checked and rejected for now: the lower-plenum dryout cap (2026-10-29)

After the rod-collapse change (gh:#1035), three changes in a row moved
lower-head failure with plenum dryout. The digest's next candidate was
MELCOR's falling-debris quench plus the Lipinski 0-D dryout cap (Vol. 2
§2.3.8, Eqs 2-112 to 2-115; Vol. 1 COR_LP). It was evaluated by hand before
building it. MELCOR has no default particle diameter (DHYPD = −1.1 means "copy
the reference cell"), so d = 5 mm was used, the value in V1's example deck,
with porosity 0.4.

| Conditions | Lipinski dryout flux | Over the 18 m² plenum |
|---|---|---|
| 7 MPa (ρl 740 kg/m³, ρv 36 kg/m³, h_lv 1.5 MJ/kg) | about 6 MW/m² | about 110 MW |
| 0.1 MPa | about 2.2 MW/m² | about 40 MW |

The debris's decay power is about 6 MW, so the cap cannot bind in this run.
MELCOR's quench coefficient (100 W/m²K over the particle area 6V/d, about
0.5 MW/K for this debris) quenches on the same seconds-to-minutes scale as the
model's 120 s. **Porting it would change almost nothing here, so it is not
built yet.** What sets dryout is boil-off of the plenum water. The remaining
in-vessel levers are:

- debris held in the core until the support fails. This needs the core
  plate's dimensions (plan L4);
- in-core radiation and heat structures (F6).

Stated simplifications:

- One column stands for the whole wetted wall.
- The outer face is adiabatic. MELCOR's dry-cavity default, 10 W/(m² K),
  would carry about 4 % of the debris power, which would delay failure
  slightly.
- No penetrations.
- The wall stays at its initial temperature until debris arrives.
- No wall melting.

## Tests

- `core::tests::the_lower_head_wall_conserves_energy_and_creeps_by_larson_miller`:
  - wall energy conserved to 1e-9;
  - Larson-Miller t_R at 10 MPa and 1273 K within 10 % of 6.0e4 s, the
    hand value;
  - the debris cap volume inverts.
- The pinned baseline and its ablations.
