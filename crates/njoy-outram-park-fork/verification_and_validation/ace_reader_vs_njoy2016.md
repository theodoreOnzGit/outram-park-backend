# Reading ACE — Type 1 and Type 2 — vs NJOY2016

**Generated:** 2026-09-21, UTC.
**Crate / commit:** `njoy-outram-park-fork` 0.0.3, on
`claude/neutronics-runs-handoff-8xk972`.
**Reference:** NJOY2016 **2016.79** (`ac5adf5`), built from
`upstream_source/NJOY2016/` and run.
**Class:** verification — code-to-code against upstream. **Not validation.**
AI-assisted draft.

## What upstream reads

`acer.f90:485-533` handles `iopt = 7` and `iopt = 8`, with `itype = iopt - 6`:

| `iopt` | `itype` | container |
|---|---|---|
| 7 | 1 | Type 1, formatted ASCII |
| 8 | 2 | Type 2, Fortran unformatted sequential |

and dispatches on the **class letter** of the ZAID:

| letter | class | upstream routine |
|---|---|---|
| `c` | continuous-energy neutron | `acefix` |
| `h` `o` `r` `s` `a` | charged particle | `acefix` |
| `t` | thermal S(α,β) | `thrfix` |
| `p` | photoatomic | `phofix` |
| `u` | photonuclear | `phnfix` |
| `y` | dosimetry | `dosfix` |

## What this port read before: nothing

Until 2026-09-21 the crate could **write** ACE and could not read it. Five
files carried their own ad-hoc Type-1 parser — `examples/ace_vs_njoy2016.rs`,
`examples/thermal_ace_vs_njoy2016.rs`, `tests/acer.rs`, `tests/thermal_ace.rs`
and `tests/thermal_ace_zrh.rs` — and none of them validated anything about the
file they read.

`src/acer/read.rs` is now the single implementation: both containers, all six
classes, `NXS`/`JXS`/`XSS` plus the full header.

**One detail upstream is explicit about and the ad-hoc parsers were not:** the
ZAID is read as text first and only re-read as a *number* when the class is not
`t` (`acer.f90:505-508`), because a thermal ZAID such as `al27.00t` has no
numeric form. `AceHeader::zaid_num` is therefore `Option<f64>`.

## Verification: one reader, two containers, same table

NJOY2016 wrote the **same** Al-27 table twice from one ENDF tape (MAT 1325,
ENDF/B-VIII.0, 0 K), changing only `itype` on ACER card 2 — 5 442 964 bytes of
ASCII and 2 154 676 bytes of binary, sharing no bytes. Reading both and
requiring the same table is a real test of the Type-2 layout, which was taken
from the writer at `acefc.f90:13028-13053` (`ner = 512`, `nbw = 8`, at `:187`).

| quantity | result |
|---|---|
| class from ZAID | `c` on both |
| `NXS(1..16)` | **identical** |
| `JXS(1..32)` | **identical** |
| `XSS` length | **identical** (268 746) |

### The first version of this test was wrong about the format

It asserted the `XSS` values were bit-identical. **208 109 of 268 746 differ**,
and both mechanisms are Type 1's:

1. **Type 1 writes 12 significant figures**, so a value whose 13th digit is
   non-zero cannot survive. Index 0 is exactly this — the grid value is
   `1.00000000000009978e-11`, Type 2 stores it, Type 1 writes
   `1.00000000000E-11`. Relative size ~1e-13; **200 592 entries**, worst
   measured **1.003e-13**.
2. **`change` coerces the fields it treats as integers** (`acefc.f90:13088`,
   *"Change ACE data fields from integer to real or vice versa"*). Index 169615
   holds `5001.2` in Type 2 and `5001` in Type 1, with `16001.2`, `22001.2`,
   `28001.2` and `102291.1` beside it. Relative size 4e-5 — which is why the
   worst case was 400x larger than rounding alone explains. **7 517 entries**.

So **Type 1 is the lossy container, not the reader**. The gate now asserts what
the format actually guarantees: away from integer-coerced fields the two agree
to Type 1's text precision (`1e-10`, measured `1.003e-13`), and every coerced
entry is exactly the Type-2 value rounded to nearest. A Type-2 layout defect
still fails it loudly — a misaligned block produces differences of order 1, not
of order 1e-13.

**An earlier draft pinned the coercion count at "4" and was wrong by three
orders of magnitude.** It is now reported, not asserted, and the invariant is
asserted instead. Guessing a number the run could have told me is the failure
this workspace's "measure, never inherit" rule names.

Gate: `tests/ace_read_type1_vs_type2.rs`.

## What is NOT covered

- **The fixtures are not committed** (5 MB + 2 MB). The gate skips when they
  are absent and honours `OUTRAM_PARK_REQUIRE_REFERENCE_DATA`, so the skip
  cannot pass silently.
- ~~**Only the `c` and `t` classes have been read from real files.**~~
  **CORRECTED 2026-09-21, same day** — **four of the six** are now read from
  real NJOY2016 output, each decoding end to end with `NXS(1)` matching the
  actual `XSS` length:

  | ZAID | class | produced by | `XSS` |
  |---|---|---|---|
  | `13027.00c` | continuous-energy neutron | `acer iopt=1` | 268 746 |
  | `al27.00t` | thermal | `acer iopt=2` | 29 664 |
  | `92000.00p` | photoatomic | `acer iopt=4` | 71 807 |
  | `2004.00a` | charged particle (alpha) | `acer iopt=1`, alpha sublibrary | 674 |

  The two still uncovered are **`u` (photonuclear)** and **`y` (dosimetry)**:
  neither sublibrary is present in `reference-data/endf/`, so no such table can
  be produced to read back. That is a gap in the *fixtures*, not in the reader —
  the container is class-independent and all ten class letters are unit-tested.
  Gate: `every_obtainable_class_reads`.
- **mcnpx-format (13-character ZAID) is implemented but unexercised.** The
  width is inferred from the Type-2 record length; no mcnpx file was available
  to confirm it.
- **Reading is not editing.** Upstream's `iopt = 7/8` exist to print or edit a
  table and write it back out; this port reads only.
- **Nothing here is validation.** It says this port reads what NJOY2016 writes.
