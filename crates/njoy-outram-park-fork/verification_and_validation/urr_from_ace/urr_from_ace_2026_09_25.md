# Decoding the ACE UNR block, so both data routes carry the same physics

GitHub **#307**. Implemented and measured 2026-09-25.

## The defect

`outram_mc_libs::Nuclide::from_ace` ended with `urr: None, dbrc: None`, so a
nuclide built from ACE carried **no unresolved-resonance self-shielding**, while
one built from ENDF via `from_endf`/`from_tape` carried it **ON by default**
since the 2026-09-20 "correct physics is the DEFAULT SETTING" change
(`f8dbb4951`).

Two routes through one workspace with different physics. Worse than a
default-off flag, in two ways:

- **Structurally absent**, so the ablation machinery could not express it —
  `without_urr_probability_tables()` had no `with` arm to remove.
- **`tests/correct_physics_is_default.rs` covered only the ENDF path.** The test
  written to stop exactly this drift went on passing while it happened. A rule
  pinned on one construction path is pinned on none.

The data was never missing: ACE carries the tables in the **UNR block** at
`JXS(23)`. It was simply not decoded.

## What was implemented

`UrrProbabilityTables::from_ace`, ported from
`openmc/data/urr.py::ProbabilityTables.from_ace` at OpenMC `afa7a14`, plus the
`jxs::LUNR` locator (`JXS(23)`, 0-based 22) which the `jxs` module lacked.

Block layout, word for word:

| words | meaning |
|---|---|
| 0 | `N`, number of incident energies |
| 1 | `M`, number of probability bands |
| 2 | interpolation: 2 lin-lin, 5 log-log |
| 3 | inelastic competition flag |
| 4 | other-absorption flag |
| 5 | `IFF`: 1 ⇒ the values multiply the smooth cross section |
| 6..6+N | incident energies \[MeV\] |
| then | `N × 6 × M`, C-order `(energy, column, band)` |

Columns are `[cumulative probability, total, elastic, fission, capture,
heating]`. **Only heating is in MeV**; upstream scales exactly `table[:, 5, :]`
and so does this, which is why the other four are left untouched.

`IFF` is ACE's `LSSF`: `IFF = 1` means the values multiply the smooth cross
section, i.e. `UrrSample::SelfShieldingFactors`. Mapping it the wrong way round
multiplies barns by barns and is invisible to any shape check, so it is
asserted rather than trusted.

## Results, 2026-09-25

`tests/urr_from_ace_unr_block.rs` — **5 passed, 0 failed, no skips.**

**U-238's block decodes to the evaluation's own range:**

```text
U238 ACE UNR: lssf=1 range 2.0000e4..1.4901e5 eV, 83 energies, T=293.6 K
```

20 keV – 149.01 keV is ENDF/B-VIII.0's U-238 unresolved range. U-235 gives a
different range (2.25 keV – 25 keV, 19 energies), so the decoder reads each
table's own header rather than something shared.

### Two independent routes to one quantity

The strong check. The ACE side **deserialises** the tables NJOY2016's PURR
wrote; the ENDF side runs **this crate's own PURR port** on the same evaluation
to **generate** its own. A decode error here and a PURR bug there would have to
agree to pass.

| | LSSF | range \[eV\] | energies |
|---|---|---|---|
| NJOY's, via ACE | 1 | 2.00000e4 – 1.49009e5 | 83 |
| ours, via PURR | 1 | 2.00000e4 – 1.49009e5 | 83 |

Six significant figures on both bounds, identical energy count, same
convention.

### The band spread is the physics, and the mean is nearly vacuous

**A methodological correction to this study's own first instrument.** The
comparison initially used the probability-weighted **mean** factor:

```text
NJOY's [1.00005, 1.00005, 1.00000, 0.99998]  vs  ours [0.99995, 0.99995, 1.00000, 1.00013]
```

That agreement is almost meaningless. Self-shielding factors are **normalised
so their probability-weighted mean is 1** — that is what makes the mean cross
section equal the infinitely-dilute one. So the mean compares 1.0 with 1.0 and
would pass for a table whose every factor is exactly 1, i.e. one that shields
nothing.

The quantity that produces self-shielding is the **spread** across bands:

| route | total | elastic | fission | capture |
|---|---|---|---|---|
| NJOY's, via ACE | 0.19703 | 0.20408 | 0.00000 | 0.20106 |
| ours, via PURR | 0.13631 | 0.14088 | 0.00000 | 0.18379 |

Both routes independently find real structure, ~14–20 %, agreeing best (9 %) on
**capture** — the channel U-238 self-shielding actually matters for. Fission is
**exactly flat on both sides**, correctly: U-238 is sub-threshold across the
whole unresolved range.

The spreads are **not** expected to match closely and the gate does not demand
it — PURR samples random ladders with its own seed and bin count. The asserted
bound is an order of magnitude, which catches the failure that matters (one
route finding structure and the other finding none) without failing for
ladder-sampling scatter. Nothing here characterises that scatter, so no tighter
threshold is quoted.

### It reaches the cross sections

Attaching tables that nothing reads would be indistinguishable from not having
them, so this is asserted end to end in
`outram-mc-libs/tests/correct_physics_is_default.rs`:

```text
U238 from ACE at 8.450e4 eV: infinitely dilute total 12.20224 b,
                             band-0.05 total 9.11916 b
```

A **25 % reduction** on a low-probability band — self-shielding, applied.

### Corrupt blocks are refused

- A locator of 0 gives `Ok(None)`, tested by zeroing it on a table that has
  one. `None` now means only "the evaluation has no unresolved range", never
  "this path does not read the block" — the distinction the old code destroyed.
- A declared extent past the end of `XSS` errors naming the block and what it
  ran past, rather than reading adjacent blocks as probabilities.
- A zero band count errors rather than being silently treated as absent, which
  would hide a corrupt block behind the same answer as a legitimate absence.

## DBRC is still absent, and that is a property of the table

`dbrc: None` remains on the ACE route, for a reason that is not an oversight:
DBRC needs **0 K elastic data** to sample the target velocity, and a table
broadened to its own temperature carries none — ACE's ESZ elastic column is
already at `kT`. A 0 K table would carry it, so this is a property of the file
rather than of the reader. `correct_physics_is_default` deliberately does **not**
require DBRC on the ACE path, because requiring it would fail a correct
implementation.

## What this does and does not change

- **Closes** #307's scope items 1, 2 and 4: the UNR decoder, wiring it in by
  default, and extending the default-physics test to cover the ACE path.
- ~~**Does not** re-measure the ACE-vs-ENDF `k` comparison.~~ **DONE 2026-09-25,
  and the prediction in the next sentence was WRONG.** It said the asymmetry "is
  gone for URR, so re-running `lct008_ace_roundtrip` should move the ACE arm".
  It is not gone: the UNR block is now *read*, but this workspace's ACE **writer
  never emits one** (GitHub #325), so a round-tripped table still carries no URR
  and the ACE arm cannot move for that reason. *(Writer gap closed 2026-09-26 by
  `acer::build_full_with_purr` — see `../unr_block_write/`.)* The comparison was instead redone
  with URR+DBRC **ablated off the ENDF arm**, over eight seeds:
  `ACE − ENDF = +23.9 ± 125.0 pcm` and `ACE − ENDF(ablated) = −14.5 ± 131.2 pcm`
  — see
  [`ace_route_physics/route_parity_8seed_2026_09_25.md`](../../../outram-mc-libs/verification_and_validation/ace_route_physics/route_parity_8seed_2026_09_25.md).
  The original text is struck through rather than deleted because it carried a
  prediction, and the prediction failing is the informative part. Note the `+269.3 pcm` gap it reported was itself only
  0.78 sigma, i.e. never established — see
  `outram-mc-libs/verification_and_validation/ace_route_physics/urr_dbrc_worth_2026_09_25.md`,
  which bounds the joint URR+DBRC worth on that homogenised case at
  **< 154 pcm at 2 sigma**.
- **Does not** settle DBRC, item 3 above.
