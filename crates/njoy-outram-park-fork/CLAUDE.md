# CLAUDE.md — njoy-outram-park-fork

Pure-Rust port (in progress) of **NJOY2016** nuclear-data processing. Produces
the ACE continuous-energy libraries that `outram-mc-libs` consumes — the data-prep
step upstream of an OpenMC run.

> Workspace member of the **OUTRAM PARK** backend. See the root `CLAUDE.md` for
> the shared dependency policy and design rules. Dep versions come from
> `[workspace.dependencies]` — do not pin locally.

## Standing goal: openmc-notebooks data notebooks as verification tests (MANDATORY)

Part of the workspace-wide direction that **every notebook in
https://github.com/openmc-dev/openmc-notebooks becomes a verification test** as
`outram-mc-libs` grows an OpenMC-like API. **This crate owns the data notebooks:**
`nuclear-data`, `nuclear-data-resonance-covariance`, `search`, and the
cross-section-generation side of `mgxs-part-i/ii/iii` + `mdgxs-part-i/ii`
(the transport/geometry/tally notebooks belong to `outram-mc-libs`). Build a
notebook→test→required-API mapping for this subset, scaffold the tests
(tractable ones live, the rest `#[ignore]` with a documented "requires API X"
reason + a per-notebook bead), cite notebook provenance (source + commit), and
document V&V methodology **and** measured results. Tracked under beads epic
**op-6tz** (this crate's slice: **op-6tz.6**).

## License compliance (MANDATORY — do not break)

This crate is a **derivative work** of NJOY2016, which is under a *modified BSD
3-Clause* license (LANL/DOE variant). That license is GPL-compatible, so the
crate as a whole is `GPL-3.0-only`. To stay compliant, you MUST:

- **Keep `LICENSE.njoy` and `NOTICE`** at the crate root, verbatim. Never delete
  or alter the upstream copyright notice/disclaimer.
- **Mark this as a modified, non-LANL version.** Do not remove the "not the LANL
  version / not endorsed" language from `NOTICE`, `README.md`, or the crate-level
  `//!` doc in `src/lib.rs`.
- **No endorsement (BSD-3 cond. 3).** Never use "Los Alamos", "LANL", "U.S.
  Government", or NJOY contributor names to endorse or promote this crate.
- If you publish to crates.io, flip `publish = false` off only after adding
  `include` so `LICENSE.njoy` + `NOTICE` ship in the tarball.

## Design rules (see also root CLAUDE.md)

- **Enum dispatch, not trait objects.** Module selection uses the `NjoyModule`
  enum (`src/modules/mod.rs`), not `Box<dyn _>`. The module set is closed.
- **No `Box<T>`, no lifetime parameters, no `dyn`.** Own data by value or share
  read-only tables with `Arc<T>`. Fortran `common` blocks become owned structs,
  not globals.
- **`uom` at physics boundaries.** Energies, temperatures, and cross sections in
  public signatures carry dimensioned types; spell out units in doc comments.
- **Errors via `Result<_, NjoyError>`**, never a process-aborting `error()` call
  the way upstream Fortran does.
- **File size cap: 1000 lines, 1500 only if truly necessary (mandatory,
  2026-07-07 onward).** Split a ported module by function/responsibility into
  a `module_name/` directory (`mod.rs` = module doc + `pub use` re-exports;
  siblings named for their functional group) rather than growing one flat
  file. See `src/samm/coulomb/` for the pattern. Applies to every module
  ported from this date forward. Existing over-length files are tracked in
  `docs/porting-plan.md` §5 — split opportunistically, don't grow them
  further without splitting first.

## Dependency posture — "lean" is now target-qualified

The root `CLAUDE.md` calls this crate "lean (`thiserror`, `uom`; no BLAS) so
data consumers stay light". That still holds **on Android and for the default
data path** — nothing pulls a BLAS/LAPACK or C/Fortran toolchain, and the
offline WMP + MGXS path needs no network. One honest qualification since
2026-07-17:

- **Optional GPU compute (`src/gpu.rs`)** adds `wgpu` (Vulkan/Metal/DX12/GL) as
  a **target-gated** dependency — declared only under
  `[target.'cfg(not(target_os = "android"))'.dependencies]`. So **Android stays
  lean and pure-CPU** (no `wgpu`, no GPU stack), while **desktop** carries `wgpu`
  behind the target gate for the optional GPU acceleration of njoy's
  embarrassingly-parallel kernels. At runtime `gpu::probe()` returns `None`
  whenever no GPU adapter is present, so the CPU path is always the fallback and
  the CPU path stays the trusted/deterministic reference (GPU `f32` is
  acceleration only). The `wgpu` version comes from
  `[workspace.dependencies]` (matches the egui/eframe 0.34 stack — no duplicate
  `wgpu` in the tree). This does **not** re-introduce a BLAS/Fortran build
  burden, and it does **not** change the Android or default-path leanness.

## HARD RULE — no raw ENDF tape inside any crate directory

**`.endf` tapes live at the repo root in `reference-data/endf/`, never under
`crates/`.** `cargo package` builds its tarball by walking the crate root, so a
tape placed anywhere under a crate is a candidate for publication, and crates.io
caps a package at 10 MB. This workspace's eleven reference tapes total ~89 MB —
U-235 alone is 35 MB.

- **Read them through [`reference_data`](src/reference_data.rs)**:
  `reference_endf("<file>")` → `Option<PathBuf>`, or
  `reference_endf_or_skip("<file>", "<label>")` to print a skip note. Both
  honour the `OUTRAM_PARK_ENDF_DIR` override. Do **not** hand-roll
  `CARGO_MANIFEST_DIR`-relative paths at each call site.
- **Data-gated tests must skip, not fail**, when a tape is absent — a crates.io
  consumer has no repository around the crate.
- **`tests/no_endf_inside_crates.rs` enforces this** and fails with the offending
  paths if any `.endf` reappears under `crates/`.
- Record every new tape's provenance in `reference-data/endf/README.md`
  (library, MAT, size, source URL, date accessed), per `DATA_POLICY.md`.
- Until 2026-08-17 the tapes sat in `tests/resources/` and were kept out of the
  tarball only by `Cargo.toml`'s `include` allowlist. That worked, but one
  careless `"tests/**"` entry would have attempted an 89 MB publish. The layout
  now enforces it instead of a rule.

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check -p njoy-outram-park-fork --lib
cargo test  -p njoy-outram-park-fork --lib --release
```

### HARD RULE — cap unit-test memory at ~12 GB

Unit tests for this crate **must** run under a hard ~12 GB address-space cap.
The ACER / thermal S(α,β) import paths build large per-incident-energy emission
tables; a malformed ENDF record (a cursor that fails to advance, an unbounded
energy grid) becomes runaway allocation that can freeze the whole machine
instead of failing a test.

Run tests through the wrapper, which sets `ulimit -v` before invoking cargo:

```bash
crates/njoy-outram-park-fork/scripts/test.sh              # full suite, capped
crates/njoy-outram-park-fork/scripts/test.sh thermal      # subset by substring
```

Do **not** invoke a bare `cargo test -p njoy-outram-park-fork` for interactive
runs — that has no cap. If a test legitimately needs more than 12 GB, that is a
design smell (stream/chunk the data); raise it with a human before lifting the
cap in `scripts/test.sh`.

## Porting plan & C-source map (read on demand)

The full module list, the Fortran-source → Rust-module map with line counts, the
phased porting order (OpenMC ACE path first), the Fortran→Rust translation
conventions, and the golden-file verification strategy against upstream NJOY all
live in **`docs/porting-plan.md`**. The reference Fortran source is at
`upstream_source/NJOY2016`.

## Model division of labour (MANDATORY for this port)

The NJOY Fortran→Rust port runs a two-model workflow to control cost:

- **Sonnet ports, module by module, WITHOUT tests.** Sonnet does the faithful,
  line-for-line translation of a module's Fortran into Rust only. It does **not**
  write or run verification tests, and it does **not** "improve" the algorithm
  during translation (see `docs/porting-plan.md` §5). Where a piece is not yet
  done, leave an explicit `NjoyError::NotPorted` / `TODO` marker — **never** paper
  over a gap with a plausible-looking value.
- **Opus debugs, verifies, and tests.** A separate Opus pass validates each
  translated module against the NJOY golden oracle (`upstream_source/NJOY2016`), writes
  the V&V tests (methodology **and** results, per the root `CLAUDE.md` V&V rule),
  and localises/fixes discrepancies. Opus does not redo the translation.

Keep every port **line-traceable to the Fortran** so the Opus verification pass
can localise a discrepancy to a specific subroutine. Per-module theory,
implementation notes, testing status, and caveats live in each module's
`README.md` (co-located with its Rust source under `src/`).
