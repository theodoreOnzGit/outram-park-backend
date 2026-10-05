# MODER material selection and ASCII write vs NJOY2016's MODER

**Generated:** 2026-10-05T00:10Z
**Crate version / commit:** branch `claude/nuclear-data-gaps` from develop
`f39501b8b` (the record and its test are new in this change; no library code
changed)
**Issue:** GitHub #536 (item 1)
**Gate:** `tests/moder_vs_njoy2016.rs`

## Why this record exists

`src/moder/README.md` said the ASCII writer (`Tape::write`, `format_endf_float`
= NJOY's `a11`) was "verified by hand" against two values, and its *Testing*
section was a TODO. No tape written by this port had ever been compared with
one NJOY writes. The nuclear-data lesson book's coverage table listed MODER as
the one supporting module with no cross-code record.

## Methodology

**Oracle.** Upstream NJOY2016 at `ac5adf5` (the port's documented sync point),
built on 2026-10-05 from `upstream_source/NJOY2016` with gfortran 13.3.0 and
`cmake -DCMAKE_BUILD_TYPE=Release` (target `njoy_executable`). MODER in
material-selection mode (`nin = 1`, ENDF/PENDF), coded (ASCII) output:

```text
moder
 1 30/
 'moder selection: H-2 (128) + Li-6 (325), ENDF/B-VIII.0'/
 20 128/
 21 325/
 0/
stop
```

`tape20` = `reference-data/endf/n-001_H_002-ENDF8.0.endf` (MAT 128),
`tape21` = `reference-data/endf/n-003_Li_006-ENDF8.0.endf` (MAT 325). NJOY's
`tape30` is committed as `reference-data/moder/h2-li6-select.moder.tape`
(955 233 bytes, 11 793 lines, SHA-256 `0a15e866…91b1b6`), with the deck beside
it. Two input tapes and two materials, so the selection loop
(`moder.f90` labels 130-205) runs across a unit change, and the ascending-MAT
check is exercised.

**Port.** `Tape::read_file` on the same two tapes,
`moder::select_materials(&inputs, tpid, [(0, 128), (1, 325)])`, then
`Tape::write`. The TPID record is passed already padded to NJOY's layout
(66 columns, then `MAT=1 MF=0 MT=0`, sequence 0) because `select_materials`
writes the string it is given verbatim.

**Comparison.** The two files are paired line by line. Each NJOY line is put in
one class by what NJOY wrote: the TPID line; sentinels (SEND, FEND, MEND,
TEND); the MF=1/MT=451 descriptive text (the `NWD` rows after the fourth
CONT, counted from NJOY's own row 4, reset per material); and everything else
("numeric"). On numeric lines each 11-column field is classed by NJOY's
representation: `a11` float (contains a decimal point), `i11` integer, or
blank. The test also parses every field of both files with this crate's
`parse_line` and compares the values bit for bit.

**Pass criterion (asserted).** Same line count; identical MAT/MF/MT columns on
every line; every `a11` float field character-identical; every parsed value on
a numeric line bit-identical. The documented divergences are **pinned** too:
the test asserts that no `i11` field matches and no Hollerith row survives, so
fixing either fails the test and asks for this record to be updated.

**Prediction, written before the first run.** From the caveats already
documented in `Tape::write` and `src/moder/README.md`: `a11` fields identical;
`i11` fields different (written as `a11` floats); Hollerith text lost (the
`[f64; 6]` row model reads text as 0.0); sentinels and sequence numbers
different; values identical outside the text.

## Results after GitHub #553 (2026-10-05, later the same day)

**Change.** `moder.f90`'s record walkers (`file1` … `file40`, all but
`file32`) are ported as `src/moder/layout.rs`. It walks each section's rows
with the same `contio` / `listio` / `tab1io` / `tab2io` / `hdatio` / `dictio`
sequence MODER uses and records each line's kind. `Tape::write` formats each
line with `endf.f90`'s descriptor for that kind:
- `2a11,4i11` for a CONT;
- `a11` with blank fields past a count for data;
- `i11` pairs for interpolation tables;
- verbatim text for MF=1/MT=451, which `Tape` now keeps;
- `22x,4i11` for the directory.

It writes NJOY's blank sentinels and per-section sequence numbers. A section
the walker cannot account for, a GENDF or ERRORR material, or a layout that
would blank a non-zero field keeps the old all-`a11` form.

**Prediction.** Every class byte-identical on this tape, since nothing in it
is MF=32 or a group material.

| class (by NJOY's line) | lines | columns 1-75 identical | columns 76-80 identical |
|---|---:|---:|---:|
| TPID | 1 | 1 | 1 |
| sentinel | 120 | **120** | **120** |
| MF=1/MT=451 text | 616 | **616** | **616** |
| numeric | 11 056 | **11 056** | **11 056** |

| field class on numeric lines | fields | identical |
|---|---:|---:|
| `a11` float | 59 098 | 59 098 |
| `i11` integer | 4 998 | **4 998** |
| blank | 2 240 | **2 240** |

**Whole lines byte-identical: 11 793 of 11 793.** The tape is NJOY's, byte
for byte, and the evaluation's description survives. The test now asserts
whole-line identity; the #536 pins below failed, as designed, on the first
run after the change and were replaced.

**A regression caught, and the guard it left.** The first run broke
`covr_boxer_golden`'s Li-6 tier-2 case: an ERRORR output tape (MF=1/MT=451
`N1 = -11`) has group-wise LISTs, which the walker read as ENDF structure,
blanking a field that held 5.981. MODER sends such materials down a separate
path (`moder.f90:161`, `:271-273`), and so does the port now
(`moder::layout::is_group_material`). The writer also refuses any layout
that would blank a non-zero field, so a misread structure can no longer drop
a value. With both, `covr_boxer_golden`, `errorr_mf33_golden`,
`mixr_h2_be9_njoy_golden` and `resxsr_h2_njoy_golden` pass unchanged.

**Not covered:** MF=32 (`file32` not ported) and GENDF/ERRORR materials are
still written with every field as `a11`.

## Results before #553 (2026-10-05), superseded


| class (by NJOY's line) | lines | columns 1-75 identical | columns 76-80 (sequence) identical |
|---|---:|---:|---:|
| TPID | 1 | 1 | 1 |
| sentinel (SEND/FEND/MEND/TEND) | 120 | 0 | 0 |
| MF=1/MT=451 text | 616 | 0 | 216 |
| numeric | 11 056 | **9 264** | 21 |
| **total** | **11 793** | | |

| field class on numeric lines | fields | identical |
|---|---:|---:|
| `a11` float | 59 098 | **59 098** |
| `i11` integer (CONT L1/L2/N1/N2, directory) | 4 998 | 0 |
| blank (MF=1/MT=451 directory rows) | 2 240 | 0 |

- **Structure: identical.** 11 793 lines each, and the MAT/MF/MT columns agree
  on every line, so `select_materials` takes the same sections in the same
  order as NJOY and `Tape::write` emits SEND/FEND/MEND/TEND at the same places.
- **`a11`: identical in every one of 59 098 float fields**, including the
  nine-significant-figure branch. This is the check the README's "verified by
  hand against two values" stood in for.
- **Values: bit-identical** on every numeric line.
- **Whole lines byte-identical: 1 of 11 793** (the TPID line, which the test
  formatted). The prediction held in every class; no unpredicted difference
  was found.

### The differences, each traced to its cause

1. **CONT integer fields.** NJOY's `contio` writes L1/L2/N1/N2 with `i11`
   (`contio`, `endf.f90:105`, format `2a11,4i11`); the port writes every field through `a11`, because
   `Section` stores rows as `[f64; 6]` with no record type. Same value, other
   characters (`          6` against ` 6.000000+0`). 4 998 fields.
2. **Blank fields.** The MF=1/MT=451 directory rows (`dictio`, `endf.f90:701`, format `22x,4i11`) leave the
   first two fields blank; the port writes ` 0.000000+0`. 2 240 fields.
3. **MF=1/MT=451 descriptive text is lost.** `parse_endf_float` maps Hollerith
   to 0.0 (documented, matching Fortran `e11.0`), so the port's tape carries
   zeros, or a number where a text field happens to parse as one, in place of
   the evaluation's description. **This is data loss, not formatting**: a tape
   written by this port no longer says what evaluation it holds. Example:
   Li-6 line 1720, NJOY ` NDS 148, 1 (2018)    DIST-FEB18   20170207`; the
   port parsed the date field as 20 170 207 and wrote it back as
   `2.017021+7` (it reads back as 20 170 210). 616 rows.
4. **Sentinels.** NJOY's `asend`/`afend`/`amend`/`atend` (`endf.f90:1280-1435`) write blank columns
   1-66 and sequence 99999 on a SEND (0 on FEND/MEND/TEND); the port writes six
   `a11` zeros and a running count. 120 lines.
5. **Sequence numbers.** NJOY numbers each section from 1; the port counts
   across the tape. The input tapes carry no sequence numbers at all (columns
   76-80 blank), so NJOY generates them. Cosmetic: every reader in this crate
   ignores the column. 21 numeric lines and 216 text lines match by
   coincidence.
6. **TPID.** `select_materials` writes its `tpid` argument verbatim, so the
   caller must supply NJOY's padded `MAT=1 MF=0 MT=0` record to match
   (`moder.f90:124` writes card 2's text through `tpidio`, `endf.f90:511`).

### Interpretation

What MODER is for in this crate (pulling materials onto one tape and reading
them back) is verified: the selection is NJOY's, and every number survives the
round trip unchanged. **The ASCII writer is not a byte-faithful MODER**, and
the cause is the tape model, not `a11`: a row of six floats cannot say which
fields are integers, which are blank or which are text. Item 3 is the one that
matters to a user, because it silently drops an evaluation's provenance from
any tape this crate writes. Byte parity needs per-record type information
(CONT/LIST/TAB/TEXT) ~~in `Section`; filed as a follow-up rather than patched
here, because changing `Section` touches every module that builds one~~.
**RESOLVED by #553 (above):** the record type is recovered at write time by
porting MODER's own walkers, so `Section` did not have to change.

## Reference

```bibtex
@techreport{njoy2016manual,
  author      = {MacFarlane, R. E. and Muir, D. W. and Boicourt, R. M. and
                 Kahler, A. C. and Conlin, J. L.},
  title       = {The {NJOY} Nuclear Data Processing System, Version 2016},
  institution = {Los Alamos National Laboratory},
  number      = {LA-UR-17-20093},
  year        = {2016},
  note        = {Section on MODER; source moder.f90 and endf.f90 at ac5adf5}
}
```

Evaluations: ENDF/B-VIII.0 H-2 and Li-6 (Brown et al., Nucl. Data Sheets 148,
1 (2018)), provenance in `reference-data/endf/README.md`.
