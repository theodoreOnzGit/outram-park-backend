# Loading nuclear data from a folder, a zip or ACE files, natively and in a browser (gh #935, #936)

> Research, education and V&V only. AI-assisted (Claude Opus 5.5 in Claude
> Code), not yet reviewed by the maintainer. Not for reactor operation,
> licensing or safety decisions.

**Class: verification.** Does the new loading code give the same data, and the
same `k`, whichever way the data comes in, and does it give the same numbers in
a browser as natively? Nothing here is compared with a benchmark experiment.

## What was built

```text
 folder (native) ─┐                      names ─► LibraryLayout (which files to open)
 zip as bytes     ├─► EndfLibraryIndex ─┤
 files as bytes  ─┘                      each file's first 4 KB ─► EndfHeader (ZA, LISO, NSUB, MAT, HSUB library)
                          │
                          └─► main_library() ─► NuclearData::{from_folder, from_zip_bytes, from_files} ─► process ─► Nuclide

 ACE files / zip ─► AceSet (ZAID, class, kT) ─► NuclearData::from_ace_files ─► Nuclide::from_ace (nothing processed)
 EndfLibraryIndex ─► generate_ace (+ 0 K companion), generate_thermal_ace (compiled-in LEAPR deck) ─► write_zip ─► download
```

- `njoy_outram_park_fork::archive`: ZIP read (ZIP64, stored and DEFLATE,
  CRC-checked) and write, over `miniz_oxide`; no new dependency.
- `njoy_outram_park_fork::endf_library`: the index, the layout enum, header
  identity, the picker's fuzzy match, ACE generation.
- `njoy_outram_park_fork::acer::ace_set`: ACE files indexed by header.
- `outram_mc_libs::model::NuclearData`: the beginner handle gains
  `from_folder`, `from_zip_bytes`, `from_files`, `from_ace_files` and
  `with_thermal_table`.

## Methodology

1. **Bit-identical Godiva, three handles** (`tests/nuclear_data_loading.rs`):
   `workspace_reference()`, `from_folder(reference-data/endf)` and
   `from_zip_bytes` of a zip of that folder (all 89 files, made in memory by
   `write_zip`). Godiva (`vv::godiva`, 293.6 K) through `model::Model`,
   `MCSettings::Custom` 400 × [3 + 6], seed 11. Pass: every cycle's `k` equal
   to the bit, and each nuclide resolved to the same tape.
2. **ACE way in** (same test file): Godiva from NJOY2016's ENDF/B-VIII.0 ACE
   tables (`reference-data/ace/reference-njoy/endf-b-viii.0/`, 293.6 K U-234,
   U-235, U-238 and the 0 K U-235). Pass: runs; the run's `data_library` lists
   each table with its file and temperature; DBRC on for U-235 (0 K companion
   found by header), off with a reason for U-234 and U-238; a 600 K model is
   refused.
3. **Native against browser** (`tests/nd_wasm_probe_native.rs` against
   `probe.html` in headless Chromium, both calling
   `examples/nd_wasm_probe/probe.rs`). Input: F-19, U-234, U-235 and U-238
   ENDF/B-VIII.0 tapes named as NNDC's `ENDF-B-VIII.0_neutrons.zip` names them,
   plus a changelog; NJOY2016's U-234 ACE table. Tasks: (1) zip and index;
   (2) generate F-19's ACE at 293.6 K plus its 0 K companion from the zip and
   read both back by header; (3) Godiva from the zip, 200 × [2 + 3], seed 5;
   (4) the ACE file indexed and built into a nuclide. FNV-1a fingerprints over
   the zip bytes, the index (path, ZA, LISO, MAT, NSUB, AWR bits, library),
   the generated ACE text (header lines skipped: they carry the git commit),
   every cycle's `k` bits, and the ACE table's XSS bits. Pass: the browser's
   five fingerprints equal the native ones pinned in the test.
   Browser: `run.sh` builds the probe for `wasm32-unknown-unknown`, runs
   `wasm-bindgen --target web`, serves the repository on 127.0.0.1 with
   `python3 -m http.server`, and drives Chromium (`--headless=new`, fresh
   profile) over the DevTools protocol (`run_headless.mjs`). The page reads
   the tapes from the local server; nothing leaves the machine.
4. **Malformed input never panics**: `archive::tests` truncates a zip at every
   byte and flips every byte; junk files, broken zips and non-ACE files are
   skipped with a reason in `endf_library::tests` and `acer::ace_set::tests`.

Hardware: Intel Core i9-13900K, 16 logical CPUs visible, 3 used (`taskset -c
0-2`, `RAYON_NUM_THREADS=3`), 62 GiB RAM, Linux 7.2.9, CPU only; the machine
was shared with other agents' builds. Release builds. Date 2026-10-17.

## Results

| check | result |
|---|---|
| 1. Godiva, `workspace_reference` / `from_folder` / `from_zip_bytes` | **identical `k` in all 9 cycles**, k = 0.996506 ± 0.018180 each (tiny run, not a benchmark number); same tapes |
| 1. the mixed reference folder | read as ENDF/B-VIII.0 (its `HSUB` majority): 52 incident-neutron tapes used, 18 of other libraries left out, 8 thermal, 6 files skipped |
| 1. zip of the folder | 89 files, 373.7 MB to 106.4 MB in 15.5 s (`write_zip`, level 6) |
| 2. ACE Godiva | runs; 4 tables indexed in 3.5 s (~580 MB of text after gunzip); `data_library` names all four; DBRC on U-235 only, as expected; 600 K refused naming 293.6 K; k = 1.0208 ± 0.0192 (tiny run; different physics from 1, no agreement claimed) |
| 3. headless Chromium 154 against native | **all five fingerprints equal**: zip `0xdbbc098a42d96f62`, index `0x0e80455b35bda55c`, generated ACE `0xd2477939bbaba20a`, Godiva `k` trace `0xcedab1315589be07` (k = 0.996492226932384 both), ACE XSS `0x9a9dbebd27900700` |
| 3. browser timings | zip + index 5.2 s; F-19 ACE + 0 K companion 1.4 s; Godiva (processing three uranium tapes, single thread, main thread) 172.6 s; ACE read 0.17 s; wasm memory at the end 688 MB |
| 4. malformed input | no panic on any truncation or flipped byte; corrupt members fail their CRC or inflate |

**What this shows.** The folder, the zip and the upload paths feed the same
bytes to the same processing, and the browser runs the same code to the same
bits, including RECONR, BROADR, PURR and transport. **What it does not show:**
the page is a harness, not the GUI (#936's picker and drag-and-drop page is not
built); the browser run used the main thread (a demo must use a Web Worker,
per the no-lag rule); a phone was not tried; the full 310 MB NNDC archive was
not loaded in the browser (the maintainer's local copy was used for nothing
here).

## Re-running

```text
cargo test --release -p outram-mc-libs --test nuclear_data_loading --test nd_wasm_probe_native -- --nocapture
crates/outram-mc-libs/verification_and_validation/nuclear_data_loading_web/run.sh   # ~6 min; --skip-godiva for ~1 min
```
