# Cross-code summary — `outram-park-fork-liggghts` vs upstream LIGGGHTS-PUBLIC

> **Status: verification, not validation.** Everything below establishes that
> this crate computes what LIGGGHTS computes. None of it compares either code
> against an experiment. Per
> [`VERIFICATION_AND_VALIDATION.md`](../../../VERIFICATION_AND_VALIDATION.md),
> verification asks "implemented correctly?" and validation asks "represents
> physical reality well enough?" — only the first is answered here. Two codes
> agreeing can be two codes wrong together; see "What this does NOT cover".
>
> Numbers are measured, not estimated. Each row names the test that produces
> it and the upstream file it is judged against. Narrative, defect histories
> and the reasoning behind each case live in
> [`verification-and-validation.md`](./verification-and-validation.md); this
> file is the one place that carries every case in a single table.

## Method

The upstream code was **cloned, compiled and executed** — not quoted from its
documentation — and its output committed under
[`reference-data/liggghts/`](../../../reference-data/liggghts/) together with
the input decks and the two conversion scripts, so the comparison regenerates
rather than being trusted. Same standard `petir` is held to against GSL 2.8.

| | |
|---|---|
| Upstream | LIGGGHTS-PUBLIC, `github.com/CFDEMproject/LIGGGHTS-PUBLIC` |
| Commit | `3d5c00f20519e6bb6eb6756f51f1ad36564e649d` ("Remove logos", 2024-06-07) |
| Internal version | `LAMMPS_VERSION "23 Nov 2013"` (its LAMMPS base) |
| Build | `make stubs` then `make serial`; g++ 15.2.1, `-O2 -fPIC`, MPI stubs, no VTK |
| Host | x86-64 Linux, Intel Xeon @ 2.80 GHz, 4 cores |
| Dates run | 2026-09-15 (deterministic + bulk), 2026-09-17 (HTR-10 at `E = 5e8`) |

**The precision patch is load-bearing.** Stock LIGGGHTS writes `dump custom`
fields with `"%g "` — six significant figures — at which a comparison saturates
at ~`1e-6` relative and cannot distinguish "this port agrees with LIGGGHTS"
from "this port agrees with LIGGGHTS' `printf`". Two lines in
`src/dump_custom.cpp` print full `double` instead. A third change was forced by
the first: `format_default` is allocated `3*size_one+1` bytes, which the wider
format strings overrun, and the run segfaulted until it was enlarged to
`8*size_one+1`. All three are recorded in the reference-data README.

**"Bit-identical"** means the IEEE-754 bit patterns match exactly, on **every
sampled frame of the trajectory**, not merely at the endpoint. Where they do
not match, the worst absolute difference is reported with the ulp it
corresponds to.

### Why whole trajectories rather than final states

A settled endpoint is a weak discriminator: two different contact models can
reach the same resting configuration, and a comparison of final states would
pass against a port that had silently substituted one. The trajectory is what
identifies the model actually implemented. The same reasoning is why the two
head-on cases are kept separate below — Hertz and Hooke land on *different*
realised restitutions (`0.900007` vs `0.899972`) from the same requested
`e = 0.9`, and each code reproduces its own model's value, so the agreement
cannot be an artefact of both converging on the requested number.

## Deterministic cases — full-trajectory comparison

| Case | Upstream model file | Frames | `max abs dx` [m] | `max abs dv` [m/s] | `max abs domega` [rad/s] | Test |
|---|---|---|---|---|---|---|
| head-on, Hertz | `normal_model_hertz.h` | 251 | `0` | `0` | `0` | `liggghts_cross_code.rs` |
| head-on, Hooke | `normal_model_hooke.h` | 251 | `0` | `0` | `0` | `liggghts_cross_code.rs` |
| wall bounce + gravity, 400 000 steps | `fix_wall_gran.cpp`, `fix_wall_gran_base.h` | 2001 | `0` | `0` | — | `liggghts_cross_code.rs` |
| rolling, counter-spinning pair | `rolling_model_cdt.h` | 201 | `0` | `0` | `0` | `liggghts_cross_code.rs` |
| oblique + friction | `tangential_model_history.h`, `surface_model_default.h` | 251 | `0` | `1.11e-16` | `5.68e-14` | `liggghts_cross_code.rs` |
| oblique, no-history (stateless path) | `tangential_model_no_history.h` | 251 | `0` | `1.11e-16` | `1.42e-14` | `legacy_path_cross_code.rs` |

**Four of the six are bit-identical over the whole trajectory.** The two
oblique cases agree to round-off: `1.11e-16 m/s` is one ulp at `|v| ~ 1 m/s`,
and `5.68e-14 rad/s` is about 3 ulp at the final `omega_z = -83.1815 rad/s`.
That residual is the expected consequence of summing identical terms in a
different association order, not a modelling difference.

The last row verifies a **second engine**: the crate's stateless
`contact` + `simulation` path, against the upstream model it actually
implements (`no_history`) rather than the one the history-spring tests use.

## Bulk cases — statistical comparison

These start from LIGGGHTS' own post-insertion state, so both codes integrate
the identical configuration; reproducing LIGGGHTS' `fix insert/pack` RNG stream
is not attempted and is not the point. Packing fraction is measured over a slab
excluding `4 r` at the floor and at the free surface, by exact sphere-cap
integration.

~~The comparison is statistical.~~ **CORRECTED 2026-09-17** — that framing was
written on the assumption that settling is chaotic enough to make a
per-particle comparison meaningless. For the HTR-10 case it is not: see
"Per-particle agreement" below, where the median pebble lands 61 micrometres
from where LIGGGHTS puts it. The bulk quantities below are still the *asserted*
gate, because a single-number gate is what a test can act on, but "statistical"
undersold what the codes actually agree on.

| Case | Geometry | N | Quantity | Ours | LIGGGHTS | Agreement | Test |
|---|---|---|---|---|---|---|---|
| bulk bed settling | cylinder, `D/d = 6` | 354 | solid fraction | 0.5571 | 0.5582 | 0.20 % | `pebble_bed_bulk.rs` |
| angle of repose, lifting cylinder | STL mesh wall, `R = 0.050 m` | 656 | repose angle | 12.78 deg | 15.43 deg | 2.65 deg | `angle_of_repose.rs` |
| HTR-10 full core, `E = 1e8` | cylinder, `D/d = 30` | 27 558 | solid fraction | 0.5811 | 0.5810 | 0.02 % | `htr10_pebble_bed.rs` |
| HTR-10 full core, `E = 3e8` | cylinder, `D/d = 30` | 27 558 | solid fraction | 0.5754 | 0.5754 | 4 decimals | `htr10_pebble_bed.rs` |
| **HTR-10 conus slump**, `E = 5e8` | **STL mesh** conus + tube + valve, `D/d = 30` | 27 554 | solid fraction | 0.5769 | 0.5769 | **0.01 %** | `htr10_conus_cross_code.rs` |
| HTR-10 full core, `E = 5e8` | cylinder, `D/d = 30` | 27 554 | solid fraction | **0.5732** | **0.5732** | 4 decimals | `htr10_pebble_bed.rs` |

The angle-of-repose case is the weakest agreement on this page and is reported
as such rather than being tuned: a heap angle is an emergent property of the
whole settling history, and the two codes' heaps differ by 2.65 degrees. See
`verification-and-validation.md` section 4.6.

### Per-particle agreement — stronger than the bulk numbers suggest

Both codes' settled HTR-10 beds are committed
(`htr10_settled.csv` from LIGGGHTS, `htr10_settled_ours.csv` from this port),
so they can be compared pebble by pebble rather than only in aggregate. Each of
the 27 554 pebbles was integrated **50 000 steps independently** by the two
codes from the same initial state. Displacement between the two final
positions, measured 2026-09-17 at `E = 5e8`:

| statistic | value | as a fraction of a 60 mm pebble |
|---|---|---|
| median | `6.09e-05 m` | 0.10 % |
| mean | `7.12e-05 m` | 0.12 % |
| p90 | `1.24e-04 m` | 0.21 % |
| p99 | `1.59e-04 m` | 0.27 % |
| p99.9 | `6.99e-04 m` | 1.2 % |
| max | `2.91e-02 m` | 49 % (one outlier) |
| under 1 mm | **27 535 of 27 554 (99.93 %)** | |

The median pebble ends **61 micrometres** from where LIGGGHTS puts it. That is
a much stronger statement than the packing fraction agreeing to four decimals,
which a wrong model could reach by luck; agreeing on where 27 554 individual
pebbles come to rest, after 50 000 independent steps each, is not something a
different contact model does.

**It is still not bit-identical, and the residual is expected.** A granular bed
is a chaotic system with tens of thousands of contacts making and breaking;
round-off differences of the kind the oblique cases show (1-3 ulp) are
amplified by contact-order sensitivity. The 0.49-diameter outlier is one pebble
that resolved a contact differently and settled into a neighbouring void — the
kind of event a bulk measure is deliberately insensitive to, and the reason the
asserted gate is the bulk number rather than a per-particle tolerance.

### Packing fraction is not stiffness-independent

The HTR-10 rows above were run at three moduli, and `phi` falls monotonically
as the pebbles stiffen: **0.5811 -> 0.5754 -> 0.5732**, about 1.4 % in total,
and both codes track each other at every point of that sweep.
Softer pebbles interpenetrate, and interpenetration reads as extra solid in a
centre-based cap integration.

This contradicts the strict form of the usual justification for DEM stiffness
softening ("packing is set by geometry and friction, not by stiffness"), and it
was visible only because both codes were run at three stiffnesses rather than
one. It also means the number is **not converged in `E`** — at graphite's true
~9 GPa it would presumably fall a little further. The cross-code agreement was
unaffected throughout, which is the point worth keeping separate: what the soft
modulus threatened was never whether the port is correct, only whether the bed
is a fair stand-in for a real one.

## The conus slump — the mesh-wall path, verified where it matters

The angle-of-repose row above is the weakest in this table, and it is a **mesh
wall** case. Since the HTR-10 recirculation study runs on a mesh conus, that
gap sat directly under the pebble-bed work, so the conus was given its own
comparison rather than assumed to inherit the flat-floor result.

Both codes start from LIGGGHTS' own settled bed (converted by `csv2data.sh`),
read the **same STL**, and integrate 20 000 steps independently.

| step | `φ` ours vs LIGGGHTS | Δ surface height | per-particle median | within 1 mm |
|---|---|---|---|---|
| 2 000 | −0.00 % | +0.0 mm | **11 µm** | **27 554 / 27 554** |
| 6 000 | −0.00 % | +0.1 mm | 1.10 mm | 12 755 / 27 554 |
| 12 000 | +0.00 % | +2.7 mm | 3.55 mm | 739 / 27 554 |
| 20 000 | **−0.01 %** | **+2.8 mm** | 3.82 mm | 592 / 27 554 |

**Read the first row and the last row as two different statements.** The first
verifies the mesh-wall contact path: every one of 27 554 pebbles within 1 mm,
median 11 µm. The last records that a bed draining 17 cm into a funnel is a
**chaotic** rearrangement, so per-particle trajectories must diverge — while
the bulk quantity the comparison actually gates on stays at four decimals
throughout.

This is why the HTR-10 settling case (§ per-particle agreement below) and this
one use different instruments: settling is a small perturbation about an
already-settled bed, and draining is not.

## Defects this comparison found

All three were in code that compiled, passed its own tests, and carried doc
comments asserting the opposite. None would have been caught without running
upstream.

| Defect | Symptom measured | Fixed in |
|---|---|---|
| `Particle::integrate` was not symplectic, though its docs said it was | `E/E0 = 2.13e43` at `dt = 0.1/omega`; restitution 1.0031 on a nominally elastic collision at `dt = 1 us` | `src/integrator.rs` (velocity-Verlet), call site removed |
| `contact.rs` used lever arm `r` where upstream uses the contact radius `r - delta/2` | wrong contact torque, so wrong spin-up | `src/contact.rs` |
| `rolling.rs` CDT diverged three ways, chiefly damped `abs(F_n)` where upstream uses the elastic `k_n * delta_n` | 21 % apart on rolling torque | `src/rolling.rs` |

**One claim of mine was wrong and is retracted, not quietly dropped.** I
reported a tangential-damping defect in `contact.rs`; it came from comparing
that module against upstream's *history* model when it implements
`no_history`. Against the right upstream file it was already correct. Recorded
in `verification-and-validation.md` section 4.2.

## What this does NOT cover

- **No experimental comparison.** Nothing here is validation. The settled
  voidage both codes agree on at `D/d = 6` (`eps ~ 0.442`) sits **2.2
  percentage points above** the Dixon (1988) correlation for that ratio — a
  disagreement with the literature that this cross-code work neither explains
  nor resolves.
- **No published-benchmark agreement.** The HTR-10 bulk fraction (~0.573) sits
  about 6 % below the published filling fraction of 0.61, and the two are not
  the same quantity: 0.61 is a whole-core design closure including the
  near-wall region and the bed surface, while these are bulk-only slab
  measurements. Closing that gap would mean calibrating `mu` and `mu_r`
  downward, which would convert the benchmark into an input and destroy the
  check.
- **Serial only.** Upstream was built with MPI stubs and run on one rank. No
  domain-decomposition or parallel-consistency claim is made for either code.
- **`units si` only.** The port drops LIGGGHTS' `force->nktv2p` and
  `force->ftm2v` conversions, which are exactly 1.0 for SI and are **not** 1.0
  for `units real` (1.6021765e6) or `units metal` (2.94210108e13). A deck in
  those unit systems is out of scope and would be silently wrong.
- **Not every upstream model.** Ported and compared: Hertz, Hooke, tangential
  history and no-history, CDT rolling, the default surface model, primitive and
  mesh walls, `fix nve/sphere`, `fix check/timestep/gran`. Everything else in
  LIGGGHTS' granular package — cohesion models, liquid bridges, superquadrics,
  fibres, CFD-DEM coupling — is neither ported nor compared.
- **Bookkeeping axes unsigned.** Both axes of the crate README's
  `## Bookkeeping status` block remain **not yet manually checked**. Nothing in
  this document flips them; only the maintainer does.

## Reproducing

```bash
# the deterministic cases (seconds)
cargo test --release -p outram-park-fork-liggghts --test liggghts_cross_code
cargo test --release -p outram-park-fork-liggghts --test legacy_path_cross_code

# the bulk cases, gated behind the default-on `long-tests` feature
cargo test --release -p outram-park-fork-liggghts --test pebble_bed_bulk    # 317 s
cargo test --release -p outram-park-fork-liggghts --test angle_of_repose    # 2313 s
cargo test --release -p outram-park-fork-liggghts --test htr10_pebble_bed  # 2714 s
```

Runtimes measured on the host named above, 2026-09-16 and 2026-09-17. They are
machine-dependent -- re-measure rather than trusting them.

Regenerating the upstream side from source — the clone, the precision patch,
the build, and the two dump-to-CSV converters — is described in
[`reference-data/liggghts/README.md`](../../../reference-data/liggghts/README.md).
