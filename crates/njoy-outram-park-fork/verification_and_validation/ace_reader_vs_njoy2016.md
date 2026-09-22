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

> ~~`ner = 512`, `nbw = 8` — the Type-2 XSS record blocking.~~
> **CORRECTED 2026-09-21.** Those are the values for the **fast /
> charged-particle** path *only*. Every other ACE writer blocks the Type-2 XSS
> **one real per record**: thermal (`aceth.f90:571` and `:2302`),
> photo-atomic (`acepa.f90:297`, `:931`), dosimetry (`acedo.f90:319`, `:495`)
> and photonuclear (`acepn.f90:1877`, `:2481`) all declare `ner = 1`,
> `nbw = 1`. Measured the same day on NJOY's own Type-2 photo-atomic output
> for the synthetic Z = 6 tape: a 500-byte header record followed by **449
> records of 8 bytes each**, 7 692 bytes total.
>
> This did **not** affect reading — a Fortran unformatted record carries its
> own length, so `read_type2` recovers `xss` whatever the blocking is, and the
> Al-27 result below stands. It *did* affect writing: the `to_type2_bytes`
> committed in `42c361576` used 512 for every class, so it would have written
> a thermal or photo-atomic Type-2 file with the right values and the wrong
> bytes. The commit message's claim of "container parity" was therefore true
> for class `c` and overstated for the rest. `xss_per_record(class)` now
> supplies the value and `tests/acer_photoatomic_vs_njoy2016.rs` gates it
> against NJOY's own file.

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
- ~~**mcnpx-format (13-character ZAID) is implemented but unexercised.**~~
  **CORRECTED 2026-09-22** — references were produced (`acer` with a negative
  `iopt`) and it is gated. Two things came out of doing it:

  1. **A Type-1 file records no width flag**, so the reader decides from the
     bytes: columns 11-13 hold the mcnpx class suffix (`"pp "`, `"ny "`,
     `"nt "`, `"nc "`) in that variant and the first three columns of the
     `f12.6` AWR otherwise, and an `f12.6` field cannot contain a letter.
     Upstream never has to decide — `acer.f90:490-494` takes the width from
     the user's sign on `iopt`.
  2. **Upstream cannot read back the mcnpx files it writes, for most
     classes.** `acer.f90:510` reads the 13-character ZAID as `f10.0, a3` and
     then dispatches on `ht(1:1)`. For photo-atomic that is `'p'` and works;
     for dosimetry (`"ny "`) and thermal (`"nt "`) it is `'n'`, which matches
     none of its branches. This port takes the class from the **last** letter
     of the field instead — the same character in both conventions, `'p'`,
     `'y'`, `'t'`, `'c'` — and reads all of them.

  Gate: `type1_files_read_back_and_rewrite_byte_exactly`, five files across
  both widths and two classes, each read and written back **byte for byte**.
- ~~**Reading is not editing.**~~ **CORRECTED 2026-09-22** — the **edit** half
  of `iopt = 7/8` is implemented and gated: `RawAceTable::apply_edits` is
  `phofix`'s three steps (`acepa.f90:344-368`) — re-suffix the ZAID, replace
  the comment, keep or clear the IZ/AW pairs — and reproduces NJOY's own
  `acer iopt=7 ... .30` output **byte for byte**. A thermal ZAID is a name and
  is left alone by a suffix, which is pinned separately. Gate:
  `iopt7_resuffix_and_recomment_matches_njoy2016`.

  **The `print` half is still not implemented** — `phoprt`, `dosprt`,
  `thrprt` and `acefc`'s printer produce a human listing, not data, and
  nothing here consumes one.
- **`p` is now produced as well as read.** The photo-atomic path
  (`acer iopt = 4`) is ported; see `acer_photoatomic_vs_njoy2016.md`, where
  the Type-1 output is byte-identical to NJOY's.
- **Nothing here is validation.** It says this port reads what NJOY2016 writes.


## Read-then-write is byte-exact for three classes now (2026-09-22)

The 2026-09-21 record said a Type-1 round trip was pinned "through the value
domain" and deliberately **not** asserted byte-exact, because reproducing a
formatted text container needs every edit descriptor to match *including which
words the writer emits as integers*, which a file does not record.

That is true in general and was too pessimistic for three of the classes. A
file does not record the split, but for photo-atomic, dosimetry and thermal it
does not have to: the writer walks a layout that `NXS` and `JXS` fully
describe, so the split can be **derived**. `RawAceTable::derive_xss_is_int`
does that, and five files now read back and rewrite byte for byte:

| file | class | ZAID width | words |
|---|---|---|---|
| `z6_photoatomic_njoy2016.ace` | `p` | 10 | 449 |
| `z6_photoatomic_mcnpx_njoy2016.ace` | `p` | 13 | 449 |
| `h1_293k_dosimetry_njoy2016.ace` | `y` | 10 | 2 532 |
| `h1_293k_dosimetry_mcnpx_njoy2016.ace` | `y` | 13 | 2 532 |
| `mn55_293k_dosimetry_njoy2016.ace` | `y` | 10 | 70 440 |

Two corrections were needed to get there, both invisible to a value
comparison:

- **The date is an `a10` field, not a free token.** It was being read by
  whitespace-splitting the first line, which lost NJOY's `'  '//dater()`
  padding, so a rewrite left-justified it. It is now sliced at a fixed offset
  — 25 columns past the ZAID, whatever the ZAID's width.
- **`f11.0` keeps its decimal point** and `1pE11.4` of zero is
  `" 0.0000E+00"`, not `"0.0000"`. Both were wrong in the Type-1 writer and
  both sit in the header, so every byte after them shifted.

**The continuous-energy and charged-particle classes still use the
heuristic** and are still only pinned through the value domain: `change`
(`acefc.f90:13066-13200`) decides word by word over a much larger set of
blocks, and deriving that is separate work.


## And one writer, as of the same day

The byte comparisons above were only possible because the writers were merged.
Until 2026-09-22 this crate had **two** Type-1 serialisers — `AceTable`'s own
and `RawAceTable::to_type1_string` — with two copies of the Fortran edit
descriptors between them. They had drifted, in the direction the section above
records: the read-side copies were the wrong ones.

`AceTable` now converts to `RawAceTable`, the descriptors live once in
`src/acer/fortran_fmt.rs`, and the choice between the two mantissa
formulations was made by measurement rather than by seniority — Rust's `{:E}`
(correctly rounded) against `x / 10^floor(log10 x)` (rounds twice), which
disagree on **17 of every 400 000** random values by one unit in the 12th
digit. Every byte-exact comparison in this record was re-run afterwards and
still holds.

Two capabilities came with it, which is usually how one finds out the
duplication mattered: `AceTable::write_type2` and
`NuclearDataLibrary::write_ace_type2`. The old `AceTable` serialiser could
emit Type 1 only, so no continuous-energy or thermal table this crate built
had a binary form. Gate: `one_writer_and_both_containers_for_a_built_table`
in `tests/acer.rs`, which asserts that `write_to` *is* the shared serialiser
byte for byte, and that a built table survives a Type-2 file round trip with
**zero** differing values (a binary container stores raw doubles, unlike
Type 1's 12 printed digits).
