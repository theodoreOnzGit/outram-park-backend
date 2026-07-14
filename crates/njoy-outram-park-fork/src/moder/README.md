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
  branch and the two post-hoc fallback rewrites, verified by hand against the
  values already used in `endf::parse`'s pre-existing `parse_endf_float` tests
  (`" 2.004000+3"`, `" 9.991673-1"`).
- **NJOY blocked-binary** — not ported. A fully in-memory Rust pipeline that
  passes typed `Tape` values between modules has no use for it (per
  `porting-plan.md` §5); port only if interchange with the upstream Fortran
  binary is ever needed.

## Testing

**TODO** (Opus verification pass). Gate (from `porting-plan.md` Phase 1): read
a reference ENDF tape, run it through `select_materials` and `Tape::write`, and
assert structural equality against the original (MAT/MF/MT sections and record
values within a tight float tolerance — not byte equality, since formatting
differs). No tests were written as part of this translation pass, per the
crate's model-division-of-labour rule (`CLAUDE.md`).

## Caveats

- **CONT-record integer fields are not distinguished from LIST-record float
  fields.** `crate::endf::tape::Section` stores every row as a uniform
  `[f64; 6]` with no per-row tag, so `format_line`/`Tape::write` writes *every*
  field (including a CONT record's L1/L2/N1/N2 integer control fields) in
  exponential `a11` form, not upstream's plain right-justified `i11` integers.
  The written **value** round-trips exactly through this crate's own reader
  (e.g. an integer `5` becomes `5.000000+0`, which parses back to exactly
  `5.0`), but the column layout does not byte-match genuine NJOY output for
  those four fields per CONT record.
- **Ascending-MAT-order check applied unconditionally.** Fortran only enforces
  it for the ENDF/PENDF path (`inout=1`); GENDF/covariance tapes do not require
  it. This port doesn't distinguish tape "kind", so the check always applies —
  a documented divergence for GENDF/covariance material selection.
- **Sequence numbers (tape columns 76-80) are a simple monotonic counter**
  across the whole written tape, not NJOY's per-file `nsh`/`nsp`/`nsc` reset
  convention. This column is documented as cosmetic/ignored on read by every
  reader in this crate (see `parse_line`), so it carries no information either
  way.
- The blocked-binary format is an NJOY implementation detail; a fully in-memory
  Rust pipeline may never need it. Port it only when interchange with the Fortran
  oracle demands it.
- ASCII round-trips are structural, not byte-identical (field formatting differs,
  and see the CONT-field caveat above).

## References

- NJOY2016 manual §MODER (LA-UR-17-20093)
- `moder.f90` (NJOY2016 2016.79)
- `endf.f90` (`a11`, `contio`, `lineio` — the formatting/write-side routines
  ported alongside MODER since it is the first module needing a tape writer)
- ENDF-102 format manual
