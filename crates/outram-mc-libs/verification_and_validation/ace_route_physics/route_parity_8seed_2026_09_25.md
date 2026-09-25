# Restating ACE-vs-ENDF route parity with the URR+DBRC asymmetry named

GitHub **#307 item 5**. Measured 2026-09-25.

## What was being asked, and why it needed asking

`examples/lct008_ace_roundtrip.rs` recorded, on 2026-09-23:

```text
ENDF route : k_eff = 0.84980 +/- 0.00232
ACE  route : k_eff = 0.85250 +/- 0.00258
difference : +269.3 pcm  (combined sigma 346.6 pcm, 0.78 sigma)  -> AGREE
```

Two problems with citing that as route parity, and #307 named the first:

1. **The arms carried different physics.** The ENDF route applies URR
   self-shielding and DBRC by default; the ACE route carried neither. So the
   comparison was not "one library, two formats" — it was two libraries and two
   physics models.
2. **One seed is not a difference.** `+269.3 pcm` was a single realisation of a
   quantity whose seed-to-seed spread turns out to be ~250 pcm.

## Methodology

`lct008_ace_roundtrip.rs` gained two things: a **third arm** and a **seed loop**.

- **Arm A** — ENDF route, URR + DBRC **on** (the default since `f8dbb4951`).
- **Arm B** — the same tape out through this workspace's ACER and back in.
- **Arm A'** — arm A's nuclides with `without_urr_probability_tables()` and
  `without_dbrc()` applied, i.e. **arm B's omissions imposed on the ENDF route**.
  It asserts it carries neither term, so it cannot quietly stop being a control.
  `via_endf` is *consumed* into it rather than cloned — arm A's transport is done
  by then and a third copy of U-238's 284 415-point grid is hundreds of MB.

**Why the control ablates A rather than enriching B.** It would be better to give
arm B the same physics. It cannot have it: this workspace's **ACE writer emits no
UNR block at all** (no `JXS(23)` anywhere in `acer::build` — GitHub #325), so a
table written here cannot carry URR even though the reader has decoded one since
2026-09-25; and a table broadened to 293.6 K holds no 0 K elastic for DBRC. That
is a writer gap, and until it is closed, ablating A is the only symmetric
comparison available.

**The seed loop repeats only the transport**, over 8 consecutive seeds, reusing
the nuclides: the ACE build is ~265 s and the library 607 MB, so paying it once is
what makes eight seeds affordable at all. Each arm reports the mean over seeds and
the **standard error of that mean from the seed-to-seed scatter** — not an average
of the per-run internal estimates, which understate the uncertainty on a mean over
independent power iterations.

**Pairing is absent and is stated as absent.** URR and DBRC change how many
variates a history draws, so arm A' 's random stream diverges from arm A's from the
first collision that consults a probability table. These are independent runs; the
differences below carry √2 × the per-arm sigma and nothing cancels. The same
finding is recorded in
[`urr_dbrc_worth_2026_09_25.md`](urr_dbrc_worth_2026_09_25.md).

Geometry: the homogenised 30 % fuel / 70 % borated water sphere, `r = 40 cm`,
3000 histories × [30 inactive + 80 active] per seed. **Not** the LCT-008 benchmark
— homogenising destroys the lumping the benchmark exists for.

## Results (2026-09-25, 8 seeds)

| arm | `k` | sem over seeds | per-seed sd |
|---|---|---|---|
| A — ENDF, URR + DBRC on | 0.84956 | 0.00090 | 255 pcm |
| B — ACE round trip | 0.84980 | 0.00087 | 245 pcm |
| A' — ENDF, both ablated | 0.84994 | 0.00098 | 279 pcm |

| difference | value | sigma |
|---|---|---|
| **B − A** (the parity number) | **+23.9 ± 125.0 pcm** | 0.19 |
| **A' − A** (the worth of URR + DBRC here) | **+38.4 ± 133.4 pcm** | 0.29 |
| **B − A'** (the routes, same physics) | **−14.5 ± 131.2 pcm** | 0.11 |
| change in gap (`|B−A'| − |B−A|`) | −9.4 ± 181.2 pcm | 0.05 |

Per-seed `B − A`, pcm: `+269.3, +93.4, −214.4, +204.3, −59.4, −666.0, −45.5,
+609.5`.

## What this settles

**`+269.3 pcm` was one seed's fluctuation.** Over eight seeds the route
difference is `+23.9 pcm` against a per-seed spread of ~250 pcm, so the original
number sat about one standard deviation from zero and was never evidence of a
difference. The 2026-09-23 record is struck through in the example's own docs
rather than quietly replaced, because it was quoted as a measurement.

**The asymmetry's effect here is not measurable.** Imposing arm B's omissions on
arm A changes the comparison by `−9.4 ± 181.2 pcm`. So the asymmetry was real and
worth removing as a matter of correctness, and it was **not** what the 0.78 sigma
was hiding.

**All three differences are consistent with zero, so these are bounds.** The two
data routes agree to within roughly **± 250 pcm at 2 sigma** on this geometry, and
the worth of URR + DBRC is below the same bound — consistent with the sharper
twelve-seed measurement in `urr_dbrc_worth_2026_09_25.md` (`+63.5 ± 77 pcm`, whose
own conclusion is `|worth| < 154 pcm at 2 sigma`). That remains the number to
quote for the worth; this run adds eight seeds of agreement with it, not a better
estimate.

## What this does **not** claim

- **Not a measurement of URR's worth in general.** This geometry *homogenises*
  the fuel, which destroys the resonance self-shielding that makes the unresolved
  range matter. The lumped `lct008_keff.rs` is where URR should be priced.
- **Not route equivalence at any interesting precision.** Resolving the remaining
  `+23.9 pcm` at 3 sigma needs `sem <= 8 pcm`, about **246×** these statistics —
  roughly 2000 seeds at ~15 min each. The right instrument for route parity is
  `tests/nuclide_from_ace_vs_endf.rs`, which compares **cross sections** and
  resolves agreement to **0.03 %**; `k` on a 3000-history sphere cannot compete
  with that and this record should not be read as trying to.
- **The timing ratio moved and the cause is an attribution, not a measurement.**
  The ACE arm transports 694 s/seed against the ENDF arm's 91 s — **7.6×**, where
  2026-09-23 recorded 4.6× (603.56 s against 131.34 s). The ratio widened because
  the **ENDF arm got 1.4× faster**, not because the ACE arm slowed (694 s against
  604 s is within run-to-run variation on a shared machine). That is consistent
  with `develop`'s `total_at_energy` change (`1a83fad7c`) making cross-section
  lookup cheaper, which helps the route that does less of it — but nothing here
  isolates that, and it is recorded as a hypothesis.

## Follow-ups filed

- **#325** — the ACE writer emits no UNR block, which is why arm B cannot carry
  URR and why this control had to ablate arm A.
