# AGENTS.md — punggol

**PUNGGOL — Particle transport, UNcertainty, Geometry, Gamma transport,
Optimization Library.** Photons, electrons and positrons: where the gammas and
betas go and where they deposit their energy.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate.

## Status (2026-11-20)

Decay-photon source spectra (`src/decay_spectrum.rs`, gh:#1391) and the
first port of OpenMC's photon samplers (`src/openmc_photon/`, gh:#1683, not
differentially tested) are the code. The EGSnrc port (gh:#976) is **barely begun** (`src/egsnrc/`: one module
per upstream Mortran file; `ANNIH_AT_REST`, `ANNIH`, `COMPT`, `RELAX`, `PAIR`, `PHOTO`, `BHABHA`, `MOLLER`, `BREMS`, `PHOTON`, `SHOWER`, `ELECTR`, `mscat`, `sscat`, `spin_rejection`, `UPHI` (IENTRY 2 and 3), Russian
roulette and the stack and azimuth macros are a first port, not differentially tested, gh:#1680; the
remaining files are placeholders with no stub functions (`rad_compton1` and the deferred `EEMF`/`egs_parallel` files; `get_inputs`, `get_media_inputs`, `pegs4_routines` and `pegs4` are first ports) (`docs/egsnrc_port_remaining.md` lists what is ported and what each remaining item needs); RANMAR/RANLUX are in PETIR). ~~The OpenMC photon port (gh:#975) is not begun.~~ **CORRECTED 2026-11-20** — samplers, TTB and reaction sampling exist (`src/openmc_photon/`). ~~There is no transport loop~~ **CORRECTED 2026-11-21:** `photon`, `electr` and `shower` are transport loops, but they run behind compile-time `Geometry`/`ElectronGeometry` seams and this crate supplies no geometry (tests use a slab): do not describe
this crate as tracking particles through a real geometry.

## The particle split (maintainer, 2026-10-16 and 2026-10-20)

- **Neutrons stay in `outram-mc-libs`.** "outramMC will take care of
  neutrons, PUNGGOL takes care of gammas and other things."
- **PUNGGOL depends on `outram-mc-libs`, never the reverse.** If a change here
  seems to need an edit there, it is neutron-agnostic data access (as the
  chain codec re-export is) or it belongs elsewhere. Never make
  `outram-mc-libs` depend on this crate.
- Coupled neutron-photon transport: `outram-mc-libs` records where secondary
  photons are born in its collision loop; PUNGGOL transports them.
- Ions (fission fragments, alphas, recoil knock-ons) stay in `boon-lay`.
- **General numerics go to `petir`**: alias tables, RANMAR and RANLUX,
  interpolation. Do not grow a second copy here.

## Where each port goes: an `egsnrc` module in every crate it touches (maintainer, 2026-11-20)

Both PUNGGOL ports live in this crate: the OpenMC photon port (gh:#975) and
the EGSnrc port (gh:#976). **Code ported from EGSnrc always lives in a module
named `egsnrc`**, in whichever crate owns that kind of code, so a reader can
tell its upstream from the path:

| EGSnrc part | Goes to |
|---|---|
| Physics: the `HEN_HOUSE/src` core and PEGS4 | `punggol::egsnrc` (scaffolded) |
| egs++ geometry (`HEN_HOUSE/egs++/geometry`) | `outram_blender::egsnrc`, the workspace's geometry crate (see `crates/outram-blender/AGENTS.md`) |
| General numerics (RANMAR, RANLUX, alias table, interpolator) | `petir`. These are **already ported** and pinned against compiled EGSnrc, but they live at `petir::rng::{ranmar, ranlux, alias_table}` and `petir::interp_egs`. Give them the `egsnrc` label by adding a `petir::egsnrc` module that re-exports them, not by moving them: moving the files would edit possibly-reviewed code. Never port them a second time. |
| Any other part, "and such" | an `egsnrc` module in the crate that owns that kind of code |

- **Keep the OpenMC photon port out of `punggol::egsnrc`.** It goes in its
  own module named for OpenMC.
- **Ports are done in three phases** (gh:#1680-#1685):
  1. a Sonnet first port;
  2. differential testing against the compiled upstream;
  3. one Opus pass that reads only what the differential tests did not reach.

## Differential testing (phase 2 of a port; maintainer, 2026-11-21)

Full recipe, results and numbers: `docs/egsnrc_phase2_differential.md`. Rules
and commands an agent needs:

- **Reference = the real upstream, built from source in scratch, never
  committed.** Scratch clones go under `/tmp/claude-0/up/`, builds under
  `/tmp/claude-0/<name>/`. Neither upstream source nor its data (AGPL EGSnrc
  data files, ENDF evaluations, HDF5 libraries) enters this repository; only
  our tests, drivers (`verification_and_validation/<topic>/…_inputs/`) and
  recorded numbers do. Check `df -h /tmp` first (the per-session disk is
  small; `rm -rf target/release` frees about 19 GB).
- **EGSnrc** (`bd49a47a`, sparse clone of `HEN_HOUSE/{src,pegs4,mortran3,
  pieces,scripts,specs,makefiles,cutils,data,user_codes/tutor1}`; `git
  sparse-checkout add` more paths as needed, and never `git grep` a blobless
  clone: it fetches everything). Build Mortran3 with `gfortran -std=legacy`
  plus a stub for `egs_fdate`, make `mortran3.dat`, write a `machine.macros`
  for gfortran (`$RECL-FACTOR 4`, `$BYTE_ORDER '1234'`, `$CALL_EXIT`,
  `$FLUSH_UNIT`, ...), compile a user code with `mortran3.exe -d mortran3.dat
  -f egsnrc.macros machine.macros ranmar.macros <user>.mortran ranmar.mortran
  nrcaux.mortran machine.mortran egs_utilities.mortran egsnrc.mortran`, then
  `gfortran -x f77-cpp-input -std=legacy -O1 -DGIT_HASH=... -DCOMPILE_TIME=...`
  with stubs for the `egs_*` system routines. Do not use
  `-ffixed-line-length-132` (it corrupts string literals across continuation
  lines). `HEN_HOUSE=<clone>/HEN_HOUSE/ EGS_HOME=<scratch>/home/`, and the run
  directory needs an (empty) `<code>.egsinp`. `tutor1` is the working example;
  a copy that dumps tables (`difftab`) or logs discarded particles (`difftut`)
  is how the tests got their references.
- **Compare like with like on precision.** Mortran3 emits single-precision
  constants, so a stock gfortran build rounds them through `f32`; build every
  reference **twice**, stock and with `-fdefault-real-8 -fdefault-double-8`.
  Decision (gh:#1687): first validate a single-precision-constants mode of the
  port against the stock build, then `f64` constants are the standard,
  validated against the promoted build. State which build each number is from.
- **Same RNG, same seeds.** `petir` RANMAR/RANLUX and
  `outram_mc_libs::rng::lcg` are already bit-identical; seed the reference
  explicitly (EGSnrc: `ixx=1802; jxx=9373; call init_ranmar`).
- **Tests are env-gated and report, they do not tune.** Each differential test
  skips unless its `*_REFERENCE_*` variable is set (see the doc for the list),
  prints worst relative differences per table or per history, and asserts only
  structure. Never change an input, a tolerance or a constant to make a
  comparison pass; find the cause by reading upstream (root `CLAUDE.md`, "Get
  the PROCESS right first"). Report residuals above 1e-9 to the maintainer.
- **Compare in order:** data readers on the real files, then derived tables
  (HATCH photon, electron, spin, Rayleigh, Compton), then a full transport run
  (`shower`) history by history. A table that disagrees makes the history
  comparison meaningless.
- **OpenMC** (`8f79d9d0`): `cmake -S <clone> -B <build> -DOPENMC_BUILD_TESTS=OFF
  -DOPENMC_USE_OPENMP=OFF`, `cmake --build <build> --target libopenmc -j 2`
  (apt `libhdf5-dev libpugixml-dev libfmt-dev`; the vendored libraries are
  found or fetched). Photon data: NJOY2016 (clone `njoy/NJOY2016`, cmake,
  `njoy < tests/48/input` with `tape20 = photoat-…endf`, `tape21 =
  atrelax-…endf`, both in `tests/resources`) writes a photoatomic ACE; OpenMC's
  Python `openmc.data.IncidentPhoton.from_ace` turns it into HDF5 (use
  `/usr/bin/python3.12`: the apt `numpy`/`h5py`/`scipy`/`pandas`/`lxml`/
  `uncertainties` match it; `PYTHONPATH` the OpenMC clone, the `endf` package
  and a fake `openmc-<ver>.dist-info/METADATA` directory, since OpenMC asks
  `importlib.metadata` for its own version). The `endf` package is
  `github.com/paulromano/endf-python` (maintainer approved the clone,
  2026-11-21): clone it into `/tmp/claude-0/up/`, build its pybind11
  extension by hand (`g++ -O2 -shared -fPIC -std=c++17 $(python3.12-config
  --includes) -I/usr/include/pybind11 src/endf/_records.cpp -o
  src/endf/_records$(python3.12-config --extension-suffix)`; apt
  `pybind11-dev python3.12-dev`) and put `src/` on `PYTHONPATH`. Use
  `IncidentPhoton.from_endf(photoat, atrelax)`, not `from_ace` (OpenMC's
  `from_ace` rejects NJOY's older photoatomic ACE: no subshell data). Ubuntu's
  pandas 2.1 lacks `future.no_silent_downcasting` and refuses the integer
  replacement in `AtomicRelaxation.to_hdf5`; a small driver that patches both
  (numbers unchanged) is in the phase 2 doc. The comparison links `libopenmc` in a small C++ harness that calls
  `PhotonInteraction::{compton_scatter, rayleigh_scatter, pair_production,
  calculate_xs}` with the same seeds as `punggol::openmc_photon`.
- **Permissions:** downloading and running third-party binaries or cloning
  repositories the maintainer did not name is refused by the session's
  permission check; ask the maintainer rather than working around it.

## Rules

- **Search before building.** The chain `<source>` codec is
  `outram_mc_libs::depletion::DepletionChainXml` and the univariate samplers are
  `outram_mc_libs::source::distribution`. Reuse them.
- **Porting rule.** Every routine is ported from the upstream (OpenMC under
  `crates/outram-mc-libs/upstream_source/OpenMC/`, EGSnrc from
  <https://github.com/nrc-cnrc/EGSnrc>), cites `file:line`, and keeps the file's
  attribution header block (upstream project, file, commit, copyright, licence).
  EGSnrc is AGPL-3.0 and is **never vendored or committed**; clone it into a
  scratch directory to read it.
- **Code-to-code to bit identity** where OpenMC is the oracle; round-off only
  below 1e-9 relative, with the cause explained (root `CLAUDE.md`). Never
  loosen a gate.
- **V&V records carry methodology and results**, and the OpenMC drivers that
  made the reference are committed under `verification_and_validation/<topic>/openmc_inputs/`.
- **`no_std` + `alloc`** without the default `std` feature. Use `core::` /
  `alloc::` paths and `crate::nostd_prelude::*` in `no_std` builds; transcendental
  functions through `petir::mathf::RealMath`.
- **Geometry is drawn.** If this crate ever builds or changes a geometry (a
  shielding model, a detector), it follows the geometry-drawing rule in
  `crates/outram-mc-libs/AGENTS.md`: render from the assembled geometry and show
  the pictures to the maintainer.
- **No macros** (workspace rule). Mortran macros are expanded by hand into
  functions, enums and plain `match` arms. No `dyn`, no `Box`, no lifetime
  parameters (`docs/claude-md/rust-design-rules.md`).
- **Every function written or changed is reached by a test.**
- **Not a licensing or dose tool.** Research, education and V&V only.

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
