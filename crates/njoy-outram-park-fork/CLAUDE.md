# CLAUDE.md — njoy-outram-park-fork

Pure-Rust port (in progress) of **NJOY2016** nuclear-data processing. Produces
the ACE continuous-energy libraries that `outram-mc-libs` consumes — the data-prep
step upstream of an OpenMC run.

> Workspace member of the **OUTRAM PARK** backend. See the root `CLAUDE.md` for
> the shared dependency policy and design rules. Dep versions come from
> `[workspace.dependencies]` — do not pin locally.

## Maturity: DECLARED MATURE (2026-09-05)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer",
and the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

- **2026-09-05 — mature.** Bar: reconstructed and Doppler-broadened output
  agrees with **NJOY2016** to **7 significant figures** where a reference tape
  exists (group boundaries match the Fortran output bit-for-bit at that
  precision). Evidence class: **cross-code comparison** against the Fortran it
  is a port of, supported by unit tests over the SLBW and Reich-Moore
  resonance formalisms.

  Measured at declaration: RECONR + BROADR verified on U-238 (MAT 9237, AWR
  236.0058, 246 tape sections, 50 reactions, 961,073 grid points) broadened to
  293.6 K and 900 K, via `examples/endf_to_broadened_xs.rs`. **651 tests pass** (8 ignored).

  Caveat already recorded in this crate's docs: a faithful port does not
  reproduce a 2017 evaluation bit-for-bit in every path, so the 7-figure bar
  applies where a reference tape is available and not as a blanket claim.


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

## Oracle examples assert their comparison (2026-09-11)

This crate's `tests/` are already dense with `*_njoy_golden.rs` /
`*_njoy_oracle.rs` cases. Its `examples/` were not: they **printed** an oracle
comparison and exited 0 whatever the numbers were. Five now assert it, using the
gate helpers in [`vv`](src/vv.rs) — which live here, in the lower crate, and are
re-exported by `outram_mc_libs::vv`, so there is one implementation rather than
two that drift.

| example | oracle | what it now asserts |
|---|---|---|
| `seam_stage_probe` | the tape's own MF=3 | `thnmax` is at the resolved-resonance limit; **bounded** BROADR reproduces MF=3 above the seam to +0.000 %; **unbounded** BROADR still loses 46.7 % there; the (n,2n) threshold is exactly zero under the bounded kernel and leaks 1.0e-6 b under the unbounded one |
| `endf_to_broadened_xs` | analytic (convolution) | area under σ(E) is conserved 293.6 K → 900 K to **+0.000 %**, while the peak falls 37.1 % and the valley rises 253 % |
| `tutorial_resonance_to_groups` | published RI_∞ + analytic | RI_∞ matches the published 275.7 b; RI_∞ is flat in temperature; the self-shielded integrals **rise** — which is Doppler feedback |
| `temperature_thinning_study` | the evaluation itself | production (`LI=2`) interpolation is within **1.42 %** worst at 0.0253 eV; error grows with bracket width; log-space beats the stated law on 7/8 points |
| `graphite_sab_generation` | the official ENDF/B-VIII.0 tape | every stored MT=4 `S` is **bit-identical**; MT=2 Bragg edges to 1e-6 and `S(E)` to 1e-4 |

**`seam_stage_probe`'s unbounded-kernel assertions are counter-examples and are
meant to keep failing the old way.** They are what demonstrates that bounding
BROADR at `thnmax` fixes something. If `doppler_broaden` ever quietly acquires
the bound, the two entry points stop being distinct and that program has nothing
left to compare — the assertion says so in its failure message.

That defect is also the worked example behind the root `CLAUDE.md`'s "read
upstream first" rule: it was a missing **limit**, not a wrong formula. Formulas
get reviewed line-by-line during translation; control flow does not.

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
cargo check --release -p njoy-outram-park-fork --lib
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

## MF=6 LAW=1 carries its angular half (2026-09-16, `op-og56`)

`acer/energy/mf6.rs` used to read ENDF `NA` only to compute a row's stride,
keep the energy density `f₀`, and discard `f₁ … f_NA`; `LANG` was never read
off the TAB2 at all. Its own doc comment said so — *"the angular dependence
present in the ENDF data … is **not** carried here … (isotropic emission)"* —
and `outram-mc-libs` consequently emitted every MT=91 continuum and MT=16
(n,2n) neutron isotropically.

**What changed.** `Mf6Neutron` now carries `lang: Mf6AngularLaw` and
`angular: Vec<Mf6AngularTable>`, parallel to `law4.incident`. `f₀` is retained
**unnormalised** beside the coefficients, because `LANG = 1`'s Legendre terms
are on `f₀`'s own scale and `build_outgoing` renormalises the pdf it builds —
dividing by the stored pdf would be wrong.

**ACE output is unchanged.** Law 4 is an energy-only law by definition, so the
DLW serialisation does not see any of this and the golden ACE comparisons are
untouched. The angular half exists for a transport consumer, not for the ACE
writer; turning it into ACE Law 61/44 remains separate work.

**`acer::angular::legendre_cosine_law` is now public** so the continuum path
reuses the MF=4 linearisation rather than growing a second one that drifts from
it. Same argument the `vv` module makes about oracle values, applied to code.

**Measured on the evaluations** (`tests/mf6_continuum_angular_vs_endf.rs`,
which re-derives the table on every run):

| nuclide | MT | LANG | rows | rows with `NA>0` | peak `|⟨μ⟩|` |
|---|---|---|---|---|---|
| U-238 | 91 | Legendre | 8654 | 8652 | 0.5573 |
| U-235 | 91 | Legendre | 5296 | 5294 | 0.5751 |
| F-19 | 91 | Legendre | 175 | **0** | 0.0000 |
| O-16 | 91 | **Kalbach-Mann** | 1751 | 1751 | n/a |
| Al-27 | 91 | **Kalbach-Mann** | 642 | 642 | n/a |

`NA` runs as high as **26** on U-238's MT=91, so a closed-form inversion of the
`NA = 1` linear density is not sufficient — the series has to be linearised.
That was measured before choosing the implementation rather than assumed.

**`LANG = 2` (Kalbach-Mann) landed the same day.** O-16 and Al-27 use it on
MT=16 and MT=91, storing only `r` (`NA = 1`), so the slope `a` comes from the
Kalbach-86 systematics — **`groupr::kinematics::bach`, which this crate already
had** as a port of `groupr.f90:8812-8932` for the GROUPR path. Reused rather
than reimplemented; two copies of one systematics would drift.

The sampler inverts the Kalbach cumulative in **closed form and one variate**:
`sinh(a mu) + r cosh(a mu)` collapses to `sqrt(1-r^2) sinh(a mu + atanh r)`, so
`mu = [asinh(((2xi-1) sinh a + r cosh a)/sqrt(1-r^2)) - atanh r] / a`. One
variate matters — the usual two-draw branch-then-invert form would consume a
different number of draws from the isotropic fallback and shift the random
stream under an ablation. Verified against the density's own closed-form mean
`<mu> = r (coth a - 1/a)` over 25 `(r, a)` combinations.

**Note O-16's MT=91 threshold is ~10 MeV**, far above any fission spectrum, so
this changes nothing for the reactor cases in this workspace. It matters for
high-energy applications, and it closes the representation gap.

What is left unsampled is `LANG = 11…15` (tabulated cosines), which no
evaluation in `reference-data/endf/` uses on a neutron subsection.

**Reading a peak coefficient is not reading the physics.** The 0.557 above is
the largest `a₁` anywhere in the table and badly overstates what a neutron
experiences: weighted by each row's own `f₀`, U-238's MT=91 law is *exactly*
isotropic below 1.2 MeV and only reaches `+0.272` at 14 MeV. Anyone pricing
this law should weight by `f₀` first.

## MF=6 LAW=7 is parsed correctly now — and the sampler is still unported (2026-09-16)

`acer/energy/mf6.rs`'s `parse_law7_lab_angle_energy_body` had two defects. The
second was found only by writing a test for the first.

**1. The angular distribution was discarded at parse time.** LAW=7 stores, per
incident energy, a lab-cosine grid and — at each cosine — a tabulated outgoing
energy spectrum. The *relative* integrals of those per-cosine tables **are**
`f(mu)`; LAW=7 carries no separate angular record. Every table was pushed
through `normalize_pdf_cdf`, which renormalises to unit area, and the divisor
was dropped. A consumer would have sampled `mu` uniformly with nothing saying
so. `Law7MuTable::weight` retains it now, via `normalize_pdf_cdf_weighted`.
Same class as `op-og56`.

**2. It was reading the wrong record type, so it had never parsed anything.**
The reader followed `acefc.f90`'s `acelf6`, which takes the per-incident-energy
record as a `TAB1` with `INTMU = L1`, `NMU = L2`.

> **Two NJOY routines read LAW=7 and they disagree — correctly.** `acelf6` is
> right *for ACER*, which runs on NJOY's own intermediate File 6; `skip6a`'s
> header comment says so in as many words: *"Special version of skip6 for
> special version of File 6 used in ACER. Law=7 has a TAB1 containing the
> angular distribution instead of the normal TAB2 for each incident energy."*
> A genuine ENDF-6 tape has the normal `TAB2`, with `NMU` in `N2`.

This port reads evaluation tapes, so it needs `groupr.f90`'s `getmf6`
(`law.eq.7`, ~7876-7911), which it now follows. On Be-9 MT=16 the old code read
`L1 = L2 = 0`, built **zero** cosine tables, and mis-consumed the real data as
the TAB1's own pairs.

**The general lesson, worth more than the fix:** "read upstream first" means the
upstream routine that owns *this input format*, not the one whose name matches
the task. Matching on the name picked the ACER reader for ENDF input.

**And a sharper one: the correct rule was already written in this file, thirty
lines above the defect.** `skip_mf6_subsection`'s doc comment spells out the
`skip6`/`skip6a` split at length, quotes `skip6a`'s header, states that LAW=7's
per-incident record is *"**one TAB2** whose `N2` is `NMU`"*, and even names
*"ENDF/B-VIII.0's Be-9 MF=6/MT=16 ... exercises exactly this"* as the case. The
skipper was right; the parser beside it was wrong. A documented rule does not
propagate itself to the next function that needs it — which is an argument for
gates over prose, and the reason the two cases in
`tests/mf6_law7_mu_weights.rs` exist rather than another paragraph.

**Verification uses the evaluation's own normalisation, not ours.** ENDF-102
normalises LAW=7 so the double integral of `f(mu, E')` over both variables is 1;
each retained weight is the inner integral, so the weights must integrate to 1
across the cosine grid. Measured on Be-9 MT=16 (the only LAW=7 neutron
subsection in `reference-data/endf/`) at all 24 incident energies:
**1.000000-1.000001**, i.e. within `1e-6`. Worst per-cosine weight spread
`(max-min)/mean = 3.67`, so what was being discarded was a strongly anisotropic
distribution. Gates: `tests/mf6_law7_mu_weights.rs`.

**LAW=7 sampling landed the same day.** The conversion reuses the existing
machinery rather than adding a law, exactly as LAW=6 did. LAW=7 tabulates the
joint `f(mu, E')`; the samplers want the marginal `f(E')` and the conditional
`P(mu|E')`, and both fall out of `f(mu_j, E') = w_j * p_j(E')` with `w_j` the
retained per-cosine weight. The one construction step is a **merged outgoing-
energy grid** — the union of every cosine's own knots — which is exact rather
than approximate, since evaluating a piecewise-linear density on a superset of
its own knots reproduces it identically. The angular half becomes
`ContinuumAngular::LabTabulated`, kept distinct from `Legendre` because it is a
different representation **and** laboratory-frame by construction (ENDF-102:
LAW=7 is in the lab regardless of `LCT`), so it must never acquire a CM->lab
transform. `INTMU = 1` (histogram over cosine) returns `Ok(None)` and keeps the
caller's fallback rather than silently applying the lin-lin rule; Be-9 uses
`INTMU = 2`.

Two measurements, both against oracles rather than assertions:

| check | result |
|---|---|
| converted `<E'>` and `<mu>` vs the raw LAW=7 tables, 24 incident energies | **7e-16 / 9e-16** worst relative |
| sampled `<mu>` through the transport kernel at 14 MeV vs the law's closed form | **+0.254104 +- 0.001210** against **+0.253844**, 0.21 sigma |
| isotropic-ablation arm (control) | **+0.001801 +- 0.001290**, identical final RNG seed |

So Be-9 (n,2n) was emitting isotropically a law whose laboratory `<mu>` is
`+0.25` to `+0.58`. Both moment integrals are done in **closed form on each
linear segment**, never by trapezoid: trapezoid is exact for `int f` but not for
`int x f`, the error that produced a false "+0.60 % bias" earlier in this port.

**A correction from that work, worth more than the result.** The transport test's
first oracle weighted `mubar` by `pdf[k]`, copying the older Legendre control,
and read **3.30 sigma** — close enough to pass its 4 sigma gate and wrong. The
sampler selects row `k` with probability `cdf[k+1] - cdf[k]`, because
`sample_ct_table_indexed` returns the lower edge of the CDF bin. Weighting it the
way the code behaves gives 0.21 sigma. The defect was in the oracle; a looser
gate would have buried the distinction rather than exposing it.

Be-9 is in none of this workspace's criticality cases, so none of this moves a
`k_eff`. What it closes is a silent fallback.

Gates: `njoy-outram-park-fork`'s `tests/mf6_law7_conversion.rs` and
`outram-mc-libs`'s `tests/law7_lab_angle_energy_transport.rs`.

## MF=4 + MF=5 emission wired in, and a V&V reference found wrong (2026-09-16)

### The gap, and how it was found

A coverage survey (`tests/continuum_law_coverage_survey.rs`) asked a question
nobody had asked directly: **where does the Weisskopf evaporation stand-in still
fire?** It classifies every `(tape, MT)` a transport run reaches into
"evaluated", "MF=4/5", "nothing anywhere" and "MF=6 present but no law".

Its first version counted 11 sections as benign — "the evaluation carries no
MF=6, so the stand-in is all there is". **That was wrong, and checking rather
than assuming showed it**: all 11 carry both MF=4 and MF=5.

| tape | MTs | MF=4 `LTT` | MF=5 `LF` |
|---|---|---|---|
| Li-7 ENDF/B-VIII.0 | 16 | 2 (tabulated) | 1 |
| C-12 ENDF/B-VIII.0 | 91 | 0 (isotropic) | 9 (evaporation) |
| Sr-88 ENDF/B-VIII.1 | 16, 17, 91 | 1 | 1 |
| U-238 JENDL-3.3 | 16, 17, 91 | 2 / 1 | 1 |
| Pu-239 JENDL-3.3 | 16, 17, 91 | 2 / 1 | 1 |

Ten of eleven are `LF=1`, one is `LF=9`; **all eleven are `LCT = 1`
(laboratory)**. Every `LF` and `LTT` involved was already ported. The gap was
the reading, not the representation — the same shape as LAW=6 and LAW=7.

Coverage now: **42 sections from MF=6, 11 from MF=4/5, 0 on the stand-in.**

### The frame question, settled upstream rather than argued

ENDF-102 puts MF=5 secondary energies in the laboratory system while MF=4 carries
its own `LCT`, so one frame flag looked unable to express the pair. Reading NJOY
settled it in one look: `acefc.f90:5825-5869` takes `lct` from **MF=4's** own
CONT record and sets the ACE `TY` sign from it — one flag per reaction, from
MF=4. `UncorrelatedEmission::from_endf` refuses `LCT >= 2` rather than shipping
an untested frame transform; no held evaluation exercises it.

### Reuse, not conversion — and why this one is the exception

Every other law here converts into `ChiTabular` to reuse the continuum sampler.
This one deliberately does not: `sample_chi` already samples every ported MF=5
`LF` (including the analytic ones) and `sample_mf4_mu_cm` already implements
OpenMC's statistical-neighbour convention on MF=4's own grid. Converting would
have replaced two exact samplers with one tabulated approximation and forced MF=4
onto MF=5's unrelated energy grid. `Nuclide::sample_inelastic_emission` is now
the single place the MF=6 / MF=4+5 / stand-in choice is made.

Measured (`outram-mc-libs`'s `tests/mf45_uncorrelated_emission.rs`), JENDL-3.3
U-238 MT=91 at 13 MeV, 200 000 collisions:

| quantity | sampled | evaluation | |
|---|---|---|---|
| `<E'>` | 8.171914e6 eV ± 3.3e3 | 8.177120e6 eV | 1.58 sigma |
| `<mu>` | +0.339126 ± 0.001208 | +0.339994 | 0.72 sigma |

### The defect this uncovered: `mean_cosine` used the wrong quadrature

`EnergyAngular::mean_cosine` integrated `mu*f(mu)` by the **trapezoid rule**,
under a comment asserting that was exact because `f` is lin-lin. `f` linear makes
`mu*f(mu)` **quadratic**, and trapezoid is exact only for a linear integrand.

The truth needs no quadrature: for MF=4 `LTT=1` the mean cosine is **exactly
`a_1`**, the first normalised Legendre coefficient, straight off the tape.
U-235 MT=2:

| E (eV) | grid pts | `a_1` (exact) | closed form | trapezoid |
|---|---|---|---|---|
| 1.0e3 | 9 | +0.001195 | **+0.001195** | +0.001232 |
| 1.0e5 | 9 | +0.126123 | **+0.126091** | +0.130067 |
| 2.0e6 | 92 | +0.621682 | **+0.622143** | +0.622697 |

At 1.0e5 eV the trapezoid rule is **123 times** further from the truth.

**It had propagated into a V&V reference.** `elastic_mubar_vs_openmc.rs`'s
oracle is, by its own provenance note, *"the trapezoidal integral of
`mu*p(mu)`"* — and its committed `0.13007` reproduces **our trapezoid** to 3e-6
while the true value is `0.12612`. That test was passing on **two matching
errors**, the failure mode it exists to prevent. It surfaced only because fixing
the library made it fail.

Resolution, with nothing loosened:

- `mean_cosine` integrates in closed form.
- New exact gate, `tests/mf4_mean_cosine_vs_legendre_a1.rs`: 1857 Legendre rows
  across U-235 and U-238, worst **1.20e-3 below 6 MeV** (gate 2e-3, which the
  trapezoid's 3.9e-3 fails), 6.21e-3 over the full range to 30 MeV.
- Both OpenMC mu-bar tests reproduce the oracle's own quadrature locally, clearly
  labelled, so they compare like with like at their original tolerances —
  elastic back to **5.58e-4** (recorded 5.6e-4), inelastic to **3.13e-3**
  (recorded 3.1e-3). They still check the *parse* against an independent code;
  the *integral* is now checked far more sharply by `a_1`.

**A hypothesis checked and killed.** The residual above 6 MeV was first blamed on
`legendre_cosine_law`'s positivity clamp, which would legitimately move the mean
away from `a_1`. The test evaluates the raw series at every grid point itself:
**zero rows are clamped.** It is the lineariser's `ANGLE_TOL = 5e-3` (stated on
`f`, leaving a residual in a *moment* of `f`) — U-238 MT=59 at 13 MeV has 129
grid points and still differs by 6.2e-3, so it is not a coarse grid.

**Flagged, not fixed:** tightening `ANGLE_TOL` would reduce that at the cost of
larger tables. The whole effect sits above 6 MeV, and changing a lineariser
tolerance moves every angular table in the workspace — that deserves its own
paired measurement, not a drive-by.

### Scope

None of this workspace's criticality cases is affected by the MF=4/5 wiring:
Godiva and the thermal cases run on ENDF/B-VIII.0 evaluations that all carry
MF=6. The `mean_cosine` fix does reach the fast-tier MGXS `mu-bar` column and
anything reading `elastic_mubar_cm` / `inelastic_mubar_cm`.

## The photo-atomic ACE class landed, and two container facts came with it (2026-09-21)

`acer iopt = 4` is ported — `src/acer/photoatomic/` carries `acepho`, `iheat`,
`alax` and `phoout` from `acepa.f90`. The Type-1 output is **byte-identical**
to NJOY2016's own file on `reference-data/endf/photoat-synthetic-Z6.endf`
(9 950 bytes, gated by `tests/acer_photoatomic_vs_njoy2016.rs`); on U
ENDF/B-VIII.0, **71 781 of 71 807 words** sit at the file's own print
precision. Full record:
[`verification_and_validation/acer_photoatomic_vs_njoy2016.md`](verification_and_validation/acer_photoatomic_vs_njoy2016.md).

Three things are worth carrying forward.

### `ner = 512` is the fast path's number, not ACE's

The Type-2 (binary) XSS is blocked **one real per record** by every writer
except the fast/charged-particle one: `aceth.f90:571`, `acepa.f90:297`,
`acedo.f90:319`, `acepn.f90:1877` all declare `ner = 1`; only
`acefc.f90:187` says 512. This never affected *reading* — a Fortran
unformatted record carries its own length — but the Type-2 writer committed in
`42c361576` used 512 for every class, so it would have written a thermal or
photo-atomic file with the right values and the wrong bytes. That commit's
"container parity" claim was true for class `c` and overstated for the rest;
`xss_per_record(class)` now supplies the value.

**And the asymmetry goes further than the blocking:** `phoout` applies the
ESZG natural log *inside its `itype == 1` branch only*
(`acepa.f90:966-973`), so **NJOY's Type-2 photo-atomic file carries linear
energies and cross sections while its Type-1 file carries logs**. Measured,
not inferred: the Type-2 data starts at `1.0000000000001e-3` where the Type-1
file has `-6.90775527898`. A port that "fixed" that would stop reproducing
upstream's files, so `PhotoatomicAce::into_raw` takes the target container.

### `terp1` now uses upstream's grouping, and `gety1` is not `terpa`

Two corrections in `src/endf/`, both found by comparing *binary* output where
a 12-digit text comparison sees nothing:

- **`terp1`** evaluated lin-lin as `y1 + r·(y2−y1)` with `r` precomputed;
  `endf.f90:1431` writes `y1 + (x−x1)*(y2−y1)/(x2−x1)`, which is a different
  rounding. Laws 4 and 5 used `powf` where upstream uses `exp(… ln …)`. The
  degenerate-interval test was a tolerance (`|x2−x1| < EPSILON`) where
  upstream tests exact equality — that one would return `y1` on a
  legitimately narrow panel.
- **`gety1`** (`src/endf/gety1.rs`) is a *new* module, not an alias for
  `terpa`. The two disagree at every boundary: `gety1` skips leading
  zero-valued points ("zero extension as in mf13"), shades its first retained
  abscissa down by `0.999999`, and holds the last value out to `1e12` where
  `terpa` drops to zero just past `shade`. Reaching for `terpa` where upstream
  used `gety1` gives an energy grid that is wrong at its first point and its
  last — invisible in a shared-grid comparison. Same lesson as the
  `skip6`/`skip6a` split above: **read the upstream routine that owns the
  input format, not the one whose name matches.**

### A residual that is upstream's convergence, not the port's arithmetic

The heating column disagrees with NJOY by up to 8.7e-8 at 26 of 11 942
energies, all above 14 MeV, and it attributes entirely to `iheat`. Three
hypotheses were tested and killed (round-off amplification in the attribution;
conditioning — 1.4e-16 under a 1-ulp input change; the `1 − unow`
cancellation — worth 0.0). The cause is the fourth: **upstream's `pnow/2`
panel rule stops converging above ~300 MeV**, carrying 4.9e-5 of its own
truncation at 1.5 GeV and 9.9e-4 at 10 GeV, measured by refining the panel
limit (`iheat_refined`). `iheat` keeps upstream's rule; the refined path
exists as a control, not as the default. Above ~300 MeV *neither* code
supplies photon heating to better than 1e-4, and that is the number to quote.

## Dosimetry (`acer iopt = 3`) landed too, and NJOY does not test it (2026-09-21)

`src/acer/dosimetry.rs` ports `acedos` and `dosout`. Output is
**byte-identical** to NJOY2016 on two cases — H-1 (2 reactions, 2 532 words,
52 130 bytes) and Mn-55 (119 reactions, 70 440 words, 1 427 267 bytes) — with
both codes reading the same PENDF so the comparison isolates `acedos`. Record:
[`verification_and_validation/acer_dosimetry_vs_njoy2016.md`](verification_and_validation/acer_dosimetry_vs_njoy2016.md).

**All 40 cases in `upstream_source/NJOY2016/tests` were checked and none runs
`iopt = 3`.** This gate covers a path with no regression cover upstream. Worth
remembering the next time a discrepancy in a little-used module looks
surprising: "NJOY does it this way" is only evidence that the code says so,
not that anyone has ever run it.

Two behaviours to know:

- **MF=3 and MF=10 store an interpolation table differently** — the first as
  all `NBT` then all `INT` (`acedo.f90:150-153`), the second as `(NBT, INT)`
  pairs (`:220-223`) — and `dosout` writes both as `2·NR` undifferentiated
  words, so nothing downstream can tell which convention a reaction used. The
  port reproduces both rather than picking one.
- **A 0 K dosimetry table is refused here, and that is a deliberate
  divergence.** `acedos`'s temperature search runs only while the temperature
  does *not* match, so at `tempd = 0` (and at any `tempd < 1.0102 K`) the loop
  body never executes and `za`/`awr` are never read. NJOY really does then
  write a table whose ZAID is `0.00y` with `AWR = 0` — measured, 269 494 bytes
  of it. This port errors instead, and
  `zero_kelvin_is_refused_rather_than_written_with_no_zaid` pins the refusal.

## All three thermal IFENG forms are written now (2026-09-22)

`aceth.f90:674-676` maps ACER card 9's `iwt` onto `NXS(7)`: `iwt = 1` is the
equiprobable `IFENG = 0`, `iwt = 0` the skewed `IFENG = 1`, `iwt = 2` the
continuous `IFENG = 2`. The port wrote the first only; it writes all three,
selected by `ThermalAceOptions::form`. Record:
[`verification_and_validation/acer_thermal_ifeng_vs_njoy2016.md`](verification_and_validation/acer_thermal_ifeng_vs_njoy2016.md).

**`IFENG = 1` was nearly free and was nearly wrong.** Its bins come from
`BinWeights::Variable`, which `acesix.rs` already had — the only missing piece
was the flag. Writing `NXS(7) = 0` while using the variable weights would have
produced a table that lies about its own contents, which is worse than not
supporting the form: a sampler would draw the bins uniformly when the whole
point of the `1 4 10 … 10 4 1` pattern is that they are not. `ifeng1_is_skewed_
and_declares_itself` pins that 28 832 of 29 969 `ITXE` words actually move.

**`IFENG = 2` reproduces NJOY's point counts exactly, and that is the result.**
The continuous form abandons the fixed bin count: each incident energy keeps
its own number of `(E', pdf, cdf, mu...)` points, decided by a panel-merging
threshold (`:392-395`, absorb a panel contributing less than `eps/10 = 1e-6`).
That count is not an input, so reproducing all 106 of Al-27's (48…212 points,
10 518 total) and all 106 of graphite's (239…549, 36 092 total) is a statement
about the algorithm rather than about the data. It also makes a **positional**
comparison legitimate, and taken that way the disagreement is **exactly one
point per incident energy** — the last one, where the two codes' outgoing
ranges end. The 10 412 interior points agree to **4.2e-7** in `E'` and 5e-5 in
cosine; the CDF agrees to 6.5e-6 everywhere.

**A stored density may be negative, and upstream's is.** The gate bounds the
magnitude, not the sign, because **NJOY's own Al-27 table carries 7 negative
densities** out of 10 518 — the first `-8.47e-16` at a final point whose CDF
has already reached 1. An `assert!(pdf >= 0.0)` would have failed against a
file NJOY wrote. Check what upstream actually produces before deciding what
"obviously" must hold.

## mcnpx format, and why a read-then-write round trip is worth asserting (2026-09-22)

The mcnpx variant (negative `iopt`, a 13-character ZAID) was "implemented and
unexercised" in three V&V records. It took one NJOY run each to fix that, and
the output is now **byte-identical** on both classes that have a reference —
photo-atomic (9 953 bytes) and dosimetry (52 133 bytes). Asking what it would
take to gate an unexercised path is usually cheaper than it looks; leaving the
note in place was the expensive option.

Three findings worth keeping:

- **A file carries no width flag.** Upstream never needs one: `acer.f90:490-494`
  takes the ZAID width from the sign the *user* typed on `iopt`. A reader given
  only the file has to decide, and the decision is available in the bytes —
  columns 11-13 hold the mcnpx class suffix (`"pp "`, `"ny "`, `"nt "`,
  `"nc "`) in that variant and the first three columns of the `f12.6` AWR
  otherwise, and an `f12.6` field cannot contain a letter.
- **Upstream cannot read back most of the mcnpx files it writes.**
  `acer.f90:510` reads the suffix `a3` and dispatches on `ht(1:1)`: `'p'` for
  photo-atomic works, but dosimetry's `"ny "` and thermal's `"nt "` both give
  `'n'`, which matches none of its branches. This port takes the class from the
  **last** letter, which is the same character under both conventions. A
  faithful port is not a bug-for-bug one; the divergence is recorded rather
  than inherited.
- **"Byte-exact is impossible here" was too pessimistic, and a value gate hid
  two real defects.** The earlier record argued a Type-1 round trip could only
  be pinned through the value domain, because the integer/real split is not
  stored. It is not stored — but for photo-atomic, dosimetry and thermal it is
  *derivable*, because `NXS`/`JXS` fully describe the writer's walk
  (`RawAceTable::derive_xss_is_int`). Asserting bytes then exposed two header
  bugs a value comparison cannot see: the date was being whitespace-split
  rather than sliced as an `a10` field, losing NJOY's `'  '//dater()` padding,
  and `f11.0` was written without its trailing decimal point while `1pE11.4` of
  zero came out as `0.0000` instead of ` 0.0000E+00`. Five files now read back
  and rewrite byte for byte. **CE and charged-particle tables still use the
  heuristic** — `change` decides word by word over far more blocks.

## One ACE serialiser, one set of edit descriptors (2026-09-22)

The crate had **two** independent Type-1 ACE writers — `AceTable`'s own in
`src/acer/write.rs` and `RawAceTable::to_type1_string` in `src/acer/read.rs` —
each with its own copies of `fortran_e`, `fortran_f`, `fortran_f0` and the
text padding. `AceTable` now converts to `RawAceTable` and the serialisation
happens once, on the shared descriptors in `src/acer/fortran_fmt.rs`.

**They had already drifted, and only a byte comparison found it.** The
read-side copies wrote `1pE11.4` of zero as `0.0000` instead of
`" 0.0000E+00"`, dropped `f11.0`'s trailing decimal point, and used Rust's
`{:E}` exponent (`E4`) where Fortran writes a signed two-digit one (`E+04`).
All three are **header** fields, so every byte after them shifted — and the
only test of that path compared *values*, which cannot see a header at all.
The rule this leaves: **two implementations of one format is one
implementation and one latent bug.**

Merging meant picking one mantissa formulation, and it was picked by
measurement. `write.rs` took the mantissa from Rust's `{:E}` (correctly
rounded); `read.rs` divided by `10^floor(log10 x)`. They disagree on **17 of
every 400 000** random values by one unit in the 12th digit, and the division
is the wrong one — it rounds twice. The shared helper uses `{:E}`, and all
1.5 MB of byte-exact NJOY comparisons (photo-atomic Type 1 and Type 2,
dosimetry H-1 and Mn-55, both mcnpx variants) still reproduce exactly.
`mantissa_is_correctly_rounded_not_divided` pins the measured case.

**Two capabilities fell out of the merge**, which is the usual sign that the
duplication was load-bearing: `AceTable::write_type2` and
`NuclearDataLibrary::write_ace_type2` exist now. The old `AceTable`-specific
writer could emit Type 1 alone, so nothing this crate built had a binary form,
and nobody had noticed because the CE tests only ever wrote text.

**Where the ad-hoc readers went.** `acer::read` is the single reader:
`tests/acer.rs`, `tests/thermal_ace.rs`, `tests/thermal_ace_zrh.rs`,
`examples/ace_vs_njoy2016.rs` and `examples/thermal_ace_vs_njoy2016.rs` keep
thin adapters that delegate to it and nothing else, and no crate outside
`njoy-outram-park-fork` parses or writes ACE at all (checked across all 43).
Turning the test round trips into *write-then-read-with-the-production-reader*
is what makes them real: a table this crate writes but its own reader cannot
read now fails.

## Photo-nuclear: NJOY rebuilt, a tape synthesised, class `u` closed (2026-09-22)

NJOY2016 was rebuilt from `upstream_source/NJOY2016` on this machine — a clean
100 % `cmake` build with gfortran 13.3.0 into `build2/` — so the oracle is
reproducible rather than inherited.

**The blocker was data, not code.** `acer iopt = 5` needs a photo-nuclear
(NSUB=0) ENDF tape; `reference-data/endf/` has none, and the IAEA NDS host
`acquire` downloads from is **blocked by the execution environment's network
policy** (the agent proxy answers `403` to CONNECT on
`www-nds.iaea.org:443`). The environment's rule is to report a blocked host,
not route around it. So the tape was **synthesised** —
`photonuc-synthetic-Z6.endf` plus its committed generator — which is the
device this repository already uses twice over. It is fed to both codes, so it
verifies that they agree on identical input; it says nothing about physics and
nothing is claimed from it.

**What that immediately bought.** NJOY turned it into a 27 453-word `6012.00u`
table on the first run, and porting `phnout`'s layout walk
(`acepn.f90:2504-2780`, now `src/acer/photonuclear/layout.rs`) made that table
**read back and rewrite byte for byte**. With it, **every class letter
upstream dispatches on has now been read from a file NJOY wrote** — `u` was
the last one outstanding, and it had been sitting in the V&V record as "a gap
in the fixtures" since the reader was written.

**Why the walk is worth more than a mask.** A photo-nuclear table's
integer/real split is stored nowhere, so reproducing NJOY's bytes requires
walking the *whole* per-emitted-particle structure: `PXS`, `PHN`, `MTRP`,
`TYRP`, `LSIGP`/`SIGP` with its MF=12/13 branch, `LANDP`/`ANDP` with its
equiprobable-versus-tabulated sub-branch, and `LDLWP`/`DLWP` with law bodies
4, 44, 61, 7/9 and 33 — law 61 alone needs two levels of locator chasing. One
wrong count anywhere in 27 453 words changes a byte, so the round trip is a
sharp test of the whole structure, and the same walk serves as a structural
validator and as the builder's skeleton.

**The builder (`acephn`, 1 826 lines) is not done**, and the V&V record says
so rather than implying otherwise. Also untested, and stated there: whether a
production photo-nuclear evaluation exercises branches the synthetic tape does
not — MF=4-only angular data, MT=18 with its nubar, discrete MT=600-849
levels, and the `ielas = 1` elastic path.

## `acer iopt = 5` reaches byte parity, and how three defects were found (2026-09-22)

`src/acer/photonuclear/build.rs` ports `acephn`'s LANL-style path and
reproduces NJOY2016's own photo-nuclear ACE **byte for byte** — 27 453 words,
556 781 bytes, worst value difference **1.0e-13**. Both codes read the same
synthetic NSUB=0 tape. Full record:
[`verification_and_validation/acer_photonuclear_vs_njoy2016.md`](verification_and_validation/acer_photonuclear_vs_njoy2016.md).

**Every one of the three defects was in this port, not upstream, and two would
have survived a tolerance.**

1. **`gety1`'s initialisation returns an `xnext` that is not `xlast`.** The
   zero-extension scan shades the returned break down by `0.999999` when the
   *next* point is the first non-zero one (`endf.f90:1511`), so a grid walk
   starts just **below** the reaction threshold. This port returned `xlast`,
   giving a 37-point grid where NJOY makes 38. A real `gety1` contract bug,
   fixed in `src/endf/gety1.rs`; it changes nothing for the photo-atomic or
   dosimetry paths, which discard that value.
2. **`ip > 1` excludes the photon, not just the neutron.** `acepn.f90:1809`
   folds production heating into the table total only for `ip > 1`, and `ip`
   is the **ZAP** — so ZAP 1 (neutron) *and* ZAP 0 (photon) are both out.
   Reading it as "everything but the neutron" double-counted the photon and
   left the heating column 39 % high.
3. **`avll` is updated inside the `ig /= 1` guard** (`:1690`), so the second
   outgoing point's trapezoid uses `0`, not `E'(1)`. Hoisting it out adds
   ~**4e-8 MeV** to every law's mean outgoing energy — invisible in the value,
   and exactly enough to flip `sigfig(., 7, 0)` on the heating block. **A 1e-7
   tolerance would have passed this.** Bytes found it.

**The builder does not track its own integer/real flags.** It derives them
from `layout::walk`, the same port of `phnout` the reader uses and already
byte-verified, so the builder cannot disagree with the reader about where a
locator is — and a table the walk cannot traverse is reported as a *builder*
defect instead of being written out. Worth copying: when a format's structure
is recoverable from the data, describe it once and let both directions use it.

**`ptleg2` was checked and rejected for reuse.** `acer::angular::legendre_cosine_law`
looks like the same conversion and is not: `ptleg2` uses tolerances
`2e-4`/`2e-3`, a 24-deep stack, a `1e-10` floor and a negative-lobe repair,
against `ANGLE_TOL = 5e-3` and a different bisection. Reusing it would have
produced a different grid and lost parity. Only the isotropic case is
implemented, pinned against NJOY's own output.

**Everything unported refuses by name** (`NjoyError::NotPorted`): `ielas = 1`,
`LANG = 2`, `NA > 0`, `ND > 0`, `LCT /= 1`, recoil subsections, MF=4/MF=5,
`LAW = 2`/`LAW = 4`, and MT=18's nubar substitution. A photo-nuclear table
that silently omits an emitted particle still reads and still looks plausible.

**The pair is archived, and the `hk` comment is the provenance.** Both tables
— NJOY2016's and this port's — live in the `reference-data/ace` submodule as
`{reference,outram-park}-njoy/synthetic/0K/Z6-photonuclear.ace.gz`, each
stamped inside its own 70-character `hk` field with generator, version, commit
and date. They differ in that one line and agree on the other 556 781 bytes, so
the pair *is* the evidence and a `diff` re-checks it without a Rust toolchain.
The fixture this crate tests against is the same bytes as the reference copy
(SHA-256 `7b64749a…a251186a`, in the submodule's `MANIFEST.tsv`). Because the
gate is byte-identity, the stamp is asserted like any other line: regenerate
the fixture under a different comment and the test fails at byte 47 rather than
accepting it.
