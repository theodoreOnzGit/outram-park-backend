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
