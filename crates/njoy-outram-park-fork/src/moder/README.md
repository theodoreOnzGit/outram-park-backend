# MODER — tape mode conversion and selection

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §MODER); upstream Fortran: `moder.f90` (1714 lines).

## Theory

MODER is the plumbing module of the NJOY pipeline. It:

- converts tapes between NJOY's **blocked-binary** mode and **formatted** (ASCII)
  mode, in either direction;
- copies data from one logical unit to another without change of mode;
- builds a new tape containing **selected materials** (by MAT number) drawn from
  one or more input ENDF, PENDF, or GENDF tapes.

It understands ENDF-4 through ENDF-6, plus the NJOY-specific GENDF formats emitted
by GROUPR and ERRORR. Binary mode exists purely to speed intermediate I/O between
modules; the physics content is identical to the ASCII form.

## How the port implements it

In Rust the "logical unit / tape mode" indirection collapses: [`crate::endf`]
already holds an in-memory `Tape` model with record parsing, so MODER becomes
largely a *selection + serialisation* concern rather than a byte-format
converter, exactly as planned:

- **Material selection** ([`crate::moder::select_materials`]) — ports the
  card-3 `(nin, matd)` loop in `moder.f90` (labels 130-205): for each
  `(input-tape-index, MAT)` request, gather every section on that tape with the
  requested MAT and append it to a new output `Tape`. Faithfully mirrors two
  Fortran behaviours: the **ascending-MAT-order** check (`moder.f90:135-136`,
  fatal `error` — ENDF/PENDF path only in Fortran, applied unconditionally here
  since this port doesn't distinguish tape "kind"), and **"material not
  found" is a warning, not a fatal error** (`moder.f90:174-177`'s `mess` call —
  ported to `log::warn!` per `docs/porting-plan.md` §5's `mess`→`log`
  convention, skipping the request rather than aborting the whole call).
- **ASCII serialisation (write)** — [`crate::endf::tape::Tape::write`], added
  as part of this port (MODER is the first module needing a tape *writer*;
  `Tape::read` already existed). Emits sections in file order followed by the
  SEND/FEND/MEND/TEND sentinels, using a new
  [`crate::endf::parse::format_endf_float`] — a faithful line-for-line port of
  `a11` (`endf.f90:882-981`), including its extended nine-significant-figure
  branch and the two post-hoc fallback rewrites, ~~verified by hand against the
  values already used in `endf::parse`'s pre-existing `parse_endf_float` tests
  (`" 2.004000+3"`, `" 9.991673-1"`)~~ **verified against NJOY2016 on
  2026-10-05** (GitHub #536): all 59 098 float fields of a two-material
  selection are character-identical to the tape NJOY's MODER writes (see
  *Testing*).
- **NJOY blocked-binary** — not ported. A fully in-memory Rust pipeline that
  passes typed `Tape` values between modules has no use for it (per
  `porting-plan.md` §5); port only if interchange with the upstream Fortran
  binary is ever needed.

## Testing

~~**TODO** (Opus verification pass). Gate (from `porting-plan.md` Phase 1): read
a reference ENDF tape, run it through `select_materials` and `Tape::write`, and
assert structural equality against the original (MAT/MF/MT sections and record
values within a tight float tolerance — not byte equality, since formatting
differs). No tests were written as part of this translation pass, per the
crate's model-division-of-labour rule (`CLAUDE.md`).~~

**Cross-code gate since 2026-10-05 (GitHub #536):**
`tests/moder_vs_njoy2016.rs`, record
`verification_and_validation/moder_vs_njoy2016.md`. NJOY2016 (`ac5adf5`) MODER
selects H-2 (MAT 128) and Li-6 (MAT 325) from two ENDF/B-VIII.0 tapes onto one
coded tape; the port does the same with `select_materials` + `Tape::write`, and
the two are compared line by line. Measured: same 11 793 lines and the same
MAT/MF/MT on every line; **59 098 / 59 098 `a11` float fields
character-identical**; every parsed value bit-identical; ~~**1 / 11 793 lines
byte-identical**, the rest differing exactly in the classes listed under
*Caveats* below. One of them is data loss, not formatting: **the MF=1/MT=451
descriptive text does not survive** (616 rows written as numbers).~~

~~The test pins those divergences, so fixing one fails it and asks for the
record to be updated.~~ **CHANGED 2026-10-05 (GitHub #553): all 11 793 lines
byte-identical**, the MF=1/MT=451 text included. `layout.rs` ports MODER's
record walkers (`file1` … `file40`, not `file32`), and `Tape::write` formats
each line by the record it belongs to. The test asserts whole-line identity.

## Caveats

- ~~**CONT-record integer fields are not distinguished from LIST-record float
  fields.** … the column layout does not byte-match genuine NJOY output for
  those four fields per CONT record.~~ **Fixed by #553:** `layout.rs` recovers
  each line's record kind, and CONT integers are written as `i11`.
- **Not byte-faithful for MF=32 and for GENDF/ERRORR materials**: `file32` is
  not ported, and group materials take MODER's separate path. Those sections,
  and any whose layout would blank a non-zero field, are written with every
  field as `a11`; no value is dropped.
- **Ascending-MAT-order check applied unconditionally.** Fortran only enforces
  it for the ENDF/PENDF path (`inout=1`); GENDF/covariance tapes do not require
  it. This port doesn't distinguish tape "kind", so the check always applies —
  a documented divergence for GENDF/covariance material selection.
- ~~**Sequence numbers (tape columns 76-80) are a simple monotonic counter**
  across the whole written tape, not NJOY's per-file `nsh`/`nsp`/`nsc` reset
  convention.~~ **Fixed by #553:** each section counts from 1, SEND carries
  99999, FEND/MEND/TEND carry 0.
- The blocked-binary format is an NJOY implementation detail; a fully in-memory
  Rust pipeline may never need it. Port it only when interchange with the Fortran
  oracle demands it.
- ~~ASCII round-trips are structural, not byte-identical~~ **byte-identical to
  NJOY's MODER on the H-2 + Li-6 selection since #553.**
- ~~**MF=1/MT=451 descriptive text is lost on write** (measured 2026-10-05,
  GitHub #536) …~~ **Fixed by #553:** `Tape::read` keeps MF=1/MT=451's raw
  text, `select_materials` carries it to the new tape, and `Tape::write`
  writes it verbatim; blank directory fields and blank sentinels are written
  blank. A tape built in memory (`Tape::from_sections`) has no text to keep,
  so its text lines are blank.

## References

- NJOY2016 manual §MODER (LA-UR-17-20093)
- `moder.f90` (NJOY2016 2016.79)
- `endf.f90` (`a11`, `contio`, `lineio` — the formatting/write-side routines
  ported alongside MODER since it is the first module needing a tape writer)
- ENDF-102 format manual
