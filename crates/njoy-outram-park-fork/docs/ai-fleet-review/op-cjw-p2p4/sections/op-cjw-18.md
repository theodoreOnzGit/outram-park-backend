# op-cjw.18 — MIXR / RESXSR / DTFR: file-level `run()` drivers + tape I/O

**Upstream oracle:** NJOY2016 `src/{mixr,resxsr,dtfr}.f90` @ git
`ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.
**Scope touched:** `src/mixr/`, `src/resxsr/`, `src/dtfr/` only.
**Provenance:** every new/edited file carries the GPLv3 + NJOY2016/ac5adf5
attribution header block.

**Honesty note.** There are **no golden files** from upstream NJOY, so nothing
here is byte-compared against a real NJOY run. The gates are **round-trip** and
**structural** correctness (record sequence, word counts, field order, and the
regression-lock of the already-tested kernels through the new drivers). Where a
piece is not faithfully finishable it stays an explicit `NjoyError::NotPorted`
with the Fortran routine + line range — no fabricated numbers, no fake-green.

**Final test result (2026-07-15, capped suite via `scripts/test.sh`):**

```
test result: ok. 327 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Build: `cargo build -p njoy-outram-park-fork --release` — **0 warnings**.
Per-module: MIXR 13 tests, RESXSR 13 tests, DTFR 18 tests (all pass).
**Test-count delta: +11** (MIXR +4, RESXSR +3, DTFR +4).

> Out-of-scope observation: `covr::boxer` briefly failed mid-session from a
> *different* agent's in-progress `src/covr/` work sharing this worktree; it was
> green by the final full run. Not my module; not touched.

---

## MIXR — `src/mixr/`

### Files changed
- **`src/mixr/driver.rs`** *(new, 170 lines)* — the file-level driver
  `run_mix<R: Read, W: Write>` (`Tape::read` → `mix` → `Tape::write`),
  mirroring `mixr.f90:144-389`.
- **`src/mixr/mix.rs`** *(edited)* — added `nsub_from_awi` and `header_seeds`
  (AWI/EMAX/NSUB fidelity, `mixr.f90:147-181`); `mix()` now seeds the output
  MF=1/451 header from the inputs instead of hard-coding AWI=1/NSUB=10.
- **`src/mixr/mod.rs`** *(edited)* — `pub mod driver;` + re-exports
  (`run_mix`, `nsub_from_awi`); `run()` re-documented as the registry shim.

### DONE
- **`run_mix` driver (DONE).** Reads each input tape from a byte source, mixes
  via the already-tested engine, writes the output PENDF tape. The one
  deliberate divergence from `mixr.f90:99-121` is that the six cards arrive as a
  typed `MixrInput` rather than a re-parsed free-format `nsysi` text deck (same
  shape as every other driver in this crate).
- **AWI/EMAX/NSUB header fidelity (DONE).** `header_seeds` reads the ENDF-6
  MF=1/451 third CONT row `[AWI, EMAX, LREL, 0, NSUB, NVER]` (`mixr.f90:222-227`)
  from the **first** contributing input for AWI/NSUB and raises EMAX to the
  largest input EMAX (`mixr.f90:171-180`); bare MF=3 PENDF inputs keep the MIXR
  defaults AWI=1 / EMAX=20 MeV / NSUB=10. `nsub_from_awi` reproduces the six
  charged-particle mass-ratio windows verbatim.

### PARTIAL / NotPorted (honest)
- **`mixr::run()` (no-arg registry shim)** still returns
  `NjoyError::NotPorted("mixr card-deck driver (functional driver:
  crate::mixr::driver::run_mix)")`. What remains unported is only the
  free-format `nsysi` **card-text** reader and the physical `nout` unit handle
  (`mixr.f90:84-121`). The functional path is `run_mix`.
- **MF=1/451 comment TEXT (NotPorted, documented).** The crate's `[f64; 6]`
  section-row model cannot store Hollerith characters, so the card-6 description
  becomes a single **blank** comment record (`mixr.f90:119-121`). AWI/EMAX/NSUB
  are numeric and *are* now faithful; only the comment prose is not
  representable. Called out in `mix.rs` module docs and `mod.rs`.

### Tests / properties (+4)
- `nsub_from_awi_windows_match_fortran` — each `mixr.f90:174-180` window +
  neutron default (awi=1 → 10).
- `header_seeds_read_awi_emax_nsub_from_first_input` — **task V&V gate**: output
  row2 = `[0.9986 (AWI from 1st input), 1e8 (EMAX raised to largest), _, _,
  10010 (NSUB from AWI), _]`.
- `header_seeds_default_for_bare_pendf` — no MF=1/451 → `[1, 2e7, _, _, 10, _]`.
- `driver_matches_direct_mix` — **task V&V gate**: `run_mix` over serialized
  input tapes reproduces the direct `mix()` MF=3 pairs (both taken through the
  same ENDF-ASCII `write`/`read` round-trip so the sub-1e-12 sigfig-bias digits
  truncate identically); physical mix = 20 and 30 barns.

---

## RESXSR — `src/resxsr/`

### Files changed
- **`src/resxsr/resxs.rs`** *(new, 517 lines)* — PENDF reader feeder
  `read_pendf_reactions` (the in-memory `findf`/`gety1` analogue,
  `resxsr.f90:296-347`) and the binary RESXS `ResxsFile::write` / `::read`
  (`resxsr.f90:435-501`).
- **`src/resxsr/driver.rs`** *(edited)* — added functional `run_resxs<W: Write>`
  (read PENDF → assemble → thin → write RESXS) alongside the existing
  `NotPorted` `run()`.
- **`src/resxsr/mod.rs`** *(edited)* — `pub mod resxs;` + re-exports.

### Actual RESXS record layout implemented (for human verify)

RESXS is a NJOY *unformatted* binary file whose Fortran record markers are
compiler/runtime specific → **not reproducible byte-for-byte without a golden
file**. This port defines an explicit, self-delimiting encoding that reproduces
the RESXS **record sequence, field order, and word counts** exactly (each
`nwds` matches the `src/resxsr/format.rs` formula), and says so plainly in
`resxs.rs` module docs.

**Storage word = 4 bytes** (`resxsr.f90:95-97`; `mult == 2` ⇒ an `a8` Hollerith
spans two words), little-endian:
- **real** word → `f32` (RESXS reals are single-precision `real(kr=4)`;
  energies/xs round-trip to ~7 significant figures);
- **integer** word → `i32`;
- **`a8` Hollerith** → 8 ASCII bytes (`mult`=2 words), space-padded / truncated.

Every record framed as `[u32 nwds LE][nwds × 4 payload bytes]`.

| # | Record | Fortran | Words (`mult=2`) | Fields in order |
|---|--------|---------|------------------|-----------------|
| 1 | File identification | `:437-445` | `3*mult+1 = 7` | `hname`(a8) `huse1`(a8) `huse2`(a8) `ivers`(i32) |
| 2 | File control | `:447-455` | `5` | `efirst`(f32) `elast`(f32) `nholl`(i32) `nmat`(i32) `nblok`(i32) |
| 3 | Set Hollerith | `:457-463` | `mult*nholl` | `nholl` blank a8 words |
| 4 | File data | `:465-473` | `(mult+2)*nmat` | `hmatn[nmat]`(a8) `ntemp[nmat]`(i32) `locm[nmat]`(i32) |
| 5a | Material control | `:399-410` | `mult+ntemp+3` | `hmat`(a8) `amass`(f32) `temp[ntemp]`(f32) `nreac`(i32) `nener`(i32) |
| 5b | XS block(s) | `:412-430` | `≤ nblok`, `(nblok/nn)*nn` | per point: `energy`(f32) + `nreac*ntemp` values(f32); `nn = 1+nreac*ntemp` words/point |

Reaction column order elastic(2), fission(18), capture(102) (`resxsr.f90:300`);
`nreac` = 3 fissionable / 2 otherwise.

### DONE
- **PENDF reader feeder (DONE).** `read_pendf_reactions` pulls MF=3 TAB1 for the
  resonance MTs into `PointwiseReaction`s; `is_fissionable` = MT-18-present.
- **RESXS writer + reader (DONE).** Full record sequence written and read back;
  word counts tied to `format.rs`.
- **`run_resxs` driver (DONE, single-temperature).** Wires
  `read_pendf_reactions` → `assemble_union_grid` → `thin_linear` →
  `ResxsFile::write` end to end; computes `locm` record offsets and `amass`
  (MF=3 HEAD `AWR`) / `temp` (MF=1/451 header) per material.

### PARTIAL / NotPorted (honest)
- **`resxsr::run()` and `driver::run(&input)`** remain
  `NotPorted("resxsr")` / `NotPorted("resxsr::run")` — they are the registry /
  deck-only shims; the functional entry is `run_resxs(input, tapes, out)`.
- **Multi-temperature loop NOT ported** (`resxsr.f90:267-352`): the Fortran
  grows `jx = nreac*ntemp` reaction columns across temperatures; `run_resxs`
  handles `ntemp == 1` (first temperature only). Documented on `run_resxs`.
- **`loada`/`finda` scratch buffering** replaced by in-memory `Vec`s (not a
  fidelity gap, an architecture change).
- **Byte-for-byte NJOY parity NOT claimed** (no golden file).

### Tests / properties (+3)
- `pendf_feeder_reads_resonance_mts` — canonical MT order `[2, 102]`, TAB1 pairs
  intact, `is_fissionable == false`.
- `resxs_file_roundtrip` — **task V&V gate**: full pipeline → `write` → `read`
  preserves the 3-point union grid `{4,100,200}` eV and both reaction columns to
  `< 1e-4` relative (single precision); recovered `nener=3, nreac=2,
  amass=236.006, ivers=1`.
- `run_resxs_roundtrips` — **task V&V gate**: functional driver output reads back
  as one material `u238`, `nreac=2`, grid within `[4,200]` eV, `nmat=1`.

---

## DTFR — `src/dtfr/`

### Files changed
- **`src/dtfr/gendf.rs`** *(new, 434 lines)* — minimal, **self-contained** GENDF
  reader (depends only on `endf::records::SectionCursor`, **not** on the
  concurrently-edited `src/groupr/`) + `build_neutron_table`.
- **`src/dtfr/mod.rs`** *(edited)* — `pub mod gendf;` + re-exports.

### Actual GENDF record layout implemented (for human verify)

Read over the in-memory `Tape` via `SectionCursor`:

**Material header — MF=1/451** (`dtfr.f90:199-237`): HEAD CONT with
`nsigz = L2` (`a(4)`), `ntw = N2` (`a(6)`); then a LIST with head
`temp = C1` (`a(7)`), `ngn = L1` (`a(9)`), `ngg = L2` (`a(10)`). LIST data block =
`[ntw title][nsigz sigma-zeros][ngn+1 neutron bounds][ngg+1 gamma bounds]`.

**Reaction section — MF=3/6/23/26** (`dtfr.f90:295-323`): HEAD CONT with
`nl = L1` (`a(3)`), `nz = L2` (`a(4)`); then one LIST per initial group with head
`ng2 = L1` (`a(3)`), `ig2lo = L2` (`a(4)`), `ig = N2` (`a(6)`). Data value for
Legendre `il`, sigma-zero `jz`, secondary `k` at zero-based offset
`(il-1) + nl*((jz-1) + nz*(k-1))` (`dtfr.f90:343,356,418`); `k=1` = group flux,
`k=2` = group cross section (MF=3) / first transfer (MF=6).

`build_neutron_table` (P0, `il=1`) assembles two channels into the ported
`DtfTable`:
- **MF=3 MT=1 total** → `sig(iptotl, jg)`; seeds absorption `sig(iptotl-2,jg) +=
  total` (`dtfr.f90:340-349`), DTF order `jg = ng - ig + 1`.
- **MF=6 MT=2 elastic transfer** → `DtfTable::add_scatter_record` (unchanged
  triangular packing + `absorption = total − scatter`, `dtfr.f90:409-427`).

### DONE
- **Minimal GENDF reader (DONE):** `read_header`, `read_section_records`,
  `GendfGroupRecord::value` / `cross_section`.
- **GENDF → DTF assembly (DONE, P0 total + elastic scatter):**
  `build_neutron_table` feeds the ported `dtf_group` / `add` / `add_scatter_record`
  and checks `ngn == ng`.

### PARTIAL / NotPorted (honest)
- **`dtfr::run()` / `driver::run(&input)`** remain `NotPorted("dtfr")` /
  `NotPorted("dtfr::run")`. The reader feeds the kernels directly; the full
  orchestration shell is still the documented skeleton.
- **NOT assembled** (beyond the minimal subset, `dtfr.f90:430-553`): fission
  `nu*sigf`/`chi` (MT18/19/455), edit cross sections, thermal corrections
  (`mti`/`mtc`), photon production (MF16/17), higher Legendre orders, and the
  sigma-zero self-shielding factors. Listed on `build_neutron_table`.
- **Plotting** (`ploted`/`plotnn`/`plotnp`, `dtfr.f90:948-1507`) — **permanently
  out of scope** (viewr/PostScript).
- **`contio`/`moreio` scratch-tape staging** replaced by in-memory `Tape`.

### Tests / properties (+4)
- `header_roundtrips` — recovers `temp=293.6, ngn=3, nsigz=1, sigz, egn`.
- `group_record_value_indexing` — offset math; `cross_section` = k=2 slot.
- `gendf_to_dtf_table_roundtrip` — **task V&V gate**: DTF-order totals
  `sig(46,1)=5, sig(46,2)=3, sig(46,3)=2`; in-group elastic `sig(47,1)=4`;
  absorption `sig(44,1) = 5 − 4 = 1`. Triangular packing unchanged.
- `gendf_table_formats_to_dtf` — closes GENDF→DTF-IV format: `iptotl` column
  `[5,3,2]` → one `format0_body` line containing `5.0000E+00`/`2.0000E+00`.

---

## Still-NotPorted list (single view)

| Module | Item | Fortran | Status |
|--------|------|---------|--------|
| MIXR | `run()` free-format `nsysi` card-text reader | `mixr.f90:84-121` | shim; use `run_mix` |
| MIXR | MF=1/451 comment **text** (Hollerith) | `mixr.f90:119-121` | not representable in `[f64;6]` model |
| RESXSR | `run()` / `run(&input)` deck-only shims | `resxsr.f90:10-248` | shim; use `run_resxs` |
| RESXSR | multi-temperature column growth | `resxsr.f90:267-352` | single-temperature only |
| RESXSR | byte-exact NJOY unformatted parity | `resxsr.f90:435-501` | no golden file — record structure matched, not bytes |
| DTFR | `run()` full orchestration shell | `dtfr.f90:52-588` | reader feeds kernels; shell is skeleton |
| DTFR | fission nu*sigf/chi, edits, thermal, photons, P>0, self-shielding | `dtfr.f90:430-553` | minimal reader covers P0 total + elastic only |
| DTFR | plotting `ploted`/`plotnn`/`plotnp` | `dtfr.f90:948-1507` | permanently out of scope |
