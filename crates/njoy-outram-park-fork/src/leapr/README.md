# LEAPR — thermal scattering law S(α,β) generation

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §LEAPR); upstream Fortran: `leapr.f90` (~3.6k lines),
> git commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.

## Theory

LEAPR **generates** the thermal scattering law S(α, β) for bound moderators (in
ENDF-6 MF=7 form) — it is the *upstream* of THERMR, which only *reads* MF=7. It is
based on the British LEAP + ADDELT codes and handles the large α, β encountered at
high incident energy / low temperature that GASKET could not.

S(α, β) is built in the **incoherent / Gaussian** approximation from a phonon
frequency spectrum ρ(ω):

- **Solid-type (phonon expansion)** — S is a sum over phonon orders,
  `S = e^{−α λ} Σ_n (α λ)ⁿ/n! · T_n(β)`, with the Debye–Waller λ and the
  self-convolution functions T_n derived from ρ(ω). `T_1(β)` comes from the
  frequency integrals (`start`/`fsum`); `T_{n+1} = T_1 ∗ T_n` (`convol`).
- **Translational / diffusive** — a free-gas Gaussian (`c = 0`) or an
  Egelstaff–Schofield diffusion law (`c > 0`, using the modified Bessel `K₁`)
  convolved onto the solid law for liquids.
- **Discrete oscillators** — molecular vibrational modes (e.g. H₂O bending/
  stretching) convolved in as Bessel-weighted delta-function ladders.
- **Cold H₂/D₂** — Young–Koppel discrete rotational modes for ortho/para
  hydrogen and deuterium (helpers only — see below).
- **Coherent elastic** parameters (Bragg edges) for crystalline solids.

## What is ported (this directory)

The **physics kernels** are ported into function-grouped files (each with a
provenance header + inline `#[cfg(test)]` V&V tests):

| File | Fortran (`leapr.f90`) | Contents |
|---|---|---|
| `input.rs` | 122–372 | `LeaprInput` + option enums (typed card deck) |
| `deck.rs` | 216–372, 3096–3110 | `LeaprDeck::parse` — the **text** card-deck reader (`.leapr` file → typed deck → one `LeaprInput` per temperature) |
| `sct.rs` | 605–607, 1084–1085 | free-gas / short-collision-time Gaussian |
| `frequency.rs` | 647–724 (`start`), 726–764 (`fsum`) | ρ→P(β), Debye–Waller λ (`f0`), effective-T factor (`tbar`), `T_1(β)` |
| `continuous.rs` | 455–645 (`contin`), 766–790 (`terpt`), 792–842 (`convol`) | phonon-expansion sum, SCT tail fill, moment checks |
| `translation.rs` | 844–1007 (`trans`), 1009–1122 (`stable`), 1124–1162 (`terps`), 1164–1251 (`sbfill`), 1253–1318 (`besk1`) | translational (free-gas/diffusion) term |
| `discrete.rs` | 1320–1661 (`discre`), 1663–1796 (`bfact`), 1798–1832 (`bfill`), 1834–1865 (`exts`), 1867–1934 (`sint`) + `I₀`/`I₁` | discrete-oscillator convolution |
| `coldh.rs` | 2185–2209 (`bt`), 2211–2245 (`sumh`), 2247–2340 (`cn`), 2342–2442 (`sjbes`), 2444–2466 (`terpk`) | cold-H₂/D₂ helpers |
| `mod.rs` | — | `SabMatrix` type, module map, `run() → NotPorted` |

The `SabMatrix` type stores the asymmetric `S_s(α, −β)` with β as the fastest
index (NJOY `ssm(nbeta,nalpha)` layout).

## What is NOT ported (honest gaps)

- **`run()` — the NJOY unit plumbing and kernel orchestration.** Narrower than
  it used to be: the free-format **card reader is now ported** (`deck.rs`) and
  so is the `endout` MF=7 tape writer. What is absent is the Fortran
  unit/file plumbing (`nsysi`/`nout`) and the glue that composes
  `start` → `contin` → `coher` → `endout` into a `LeaprOutput`. That glue also
  owes two conversions nothing currently performs: `dwpix /= awr * T * bk`
  (`leapr.f90:3035`) and `tempf = tbar * T` (`leapr.f90:717`), both of which
  `endout` expects pre-converted. `run()` returns `NjoyError::NotPorted`; drive
  the module API directly.
- `copys` (2468–2487): scratch-tape plumbing for the mixed-moderator merge.
- **`coldh` orchestrator** (1936–2183): the Young–Koppel rotational
  convolution loop is ported (`coldh::add_cold_hydrogen`) but is only
  self-consistency tested, never reference-validated.
- **`skold`** (2816–2922): the Sköld pair-correlation correction. `deck.rs`
  *parses* cards 17–19 into `PairCorrelation`, and
  `LeaprDeck::unsupported_features()` flags such a deck, but nothing consumes
  the data.

## Testing — methodology and results

Ran under the 12 GB cap via `scripts/test.sh leapr`. **Re-measured 2026-08-13:
35 in-module tests, 35 passed, 0 failed**, plus the 4 integration tests of
`tests/leapr_graphite_deck_parity.rs` (see the validation section above). All
builds/tests in `--release`; `cargo check -p njoy-outram-park-fork
--all-targets` is clean (0 warnings).

Closed-form / self-consistency V&V checks with **measured numbers**
(2026-07-15 unless noted):

- **SCT / free-gas detailed balance** (`sct.rs`): `S(α,−β) = e^{−β} S(α,β)`
  exact for `tbar = 1` — relative error `< 1e-13` across sampled (α,β).
- **`besk1`** (`translation.rs`): `K₁(0.5) = 1.656441`, `K₁(1) = 0.601907`,
  `e²·K₁(2) = 1.03339` — all match to `< 2e-4`.
- **`I₀`/`I₁`** (`discrete.rs`): `I₀(1) = 1.2660658`, `I₁(1) = 0.5651591`
  to `< 5e-7`; large-argument branch `I₀(5)`, `I₁(5)` to `< 1e-4` relative.
- **`sjbes`** (`coldh.rs`): `j₀(1) = 0.8414710`, `j₁(1) = 0.3011687` to `< 1e-5`;
  `cn(0,0,0) = 1`, odd-parity `cn` vanishes; even-parity `bt` weights sum to
  `0.5` (`< 1e-12`).
- **`start` frequency integrals** (`frequency.rs`): for a Debye `ρ ∼ E²` at
  ~293 K, `deltab = 0.079050`, Debye–Waller `λ = f0 = 0.29726`, effective-T
  factor `tbar = 2.31659`; the normalized first moment recovers `tbeta` to
  `< 1e-12`.
- **Phonon expansion** (`continuous.rs`): same Debye model, `nphon = 100`. At
  `α = 1.0` the normalization check `sum0 = 0.99289` and the sum-rule check
  `sum1 = 0.98867` (both within ~1.2% of the ideal 1.0); the ~1% deficit is the
  expected finite-β-grid + SCT-tail truncation error. All S entries finite and
  non-negative.
- **`add_translation` / `add_discrete_oscillators`**: smoke tests confirm S
  stays finite and non-negative, the effective temperature moves toward the
  physical T, and the Debye–Waller λ grows when oscillators are added.

## Validation against ENDF/B-VIII.0 — graphite (2026-08-13)

**The incoherent-inelastic path is now validated against a reference LEAPR
tape.** `tests/leapr_graphite_deck_parity.rs` reads the 12,444-byte
`tsl-crystalline-graphite.leapr` deck distributed with ENDF/B-VIII.0, regenerates
S(α,β) over the deck's own 150 × 400 α/β grid at `nphon = 100`, applies
`endout`'s symmetric-S conversion `S = ssm · e^{−β/2}`, and compares against
MF=7/MT=4 of the 8,730,804-byte `tsl-crystalline-graphite.endf` tape (MAT 30).

**Measured agreement** (relative deviation over points where the tape value
exceeds 1e-30):

| Temperature | points | max | RMS | zero-pattern mismatches |
|---|---|---|---|---|
| 296 K | 48,941 | 4.917e-6 | 6.390e-7 | 0 (6,645 zeros each side) |
| 1000 K | 52,378 | 4.838e-6 | 5.817e-7 | 0 (6,328 zeros each side) |

The residual is the **tape's own printed precision**, demonstrably so: `endout`
stores values ≥ 1e-9 with 7 significant figures and smaller ones with 6
(`leapr.f90:3341-3345`), capping relative error at 5e-7 and 5e-6 respectively.
Split at that boundary the 296 K deviations are max 4.948e-7 (37,950 points,
7-figure band) and max 4.917e-6 (10,991 points, 6-figure band) — each at 99 % of
its own ceiling and neither above it.

**Boltzmann-constant caveat.** Those figures use `bk = 8.617385e-5 eV/K`, the
value NJOY2016 carried until 23 Oct 2017 and therefore the one this EVAL-SEP17
evaluation was produced with. With the crate's CODATA2018
`8.617333262e-5` the agreement is max 3.711e-4 / RMS 6.842e-5 — still good, but
100× looser, and the entire difference is `k_B`. `S(α,β)` depends on `k_B`
through `tev`, so byte-level reproduction of a pre-2018 evaluation needs the
era's constant as an input.

**Regeneration cost** (release, 12-core workstation shared with other agents, so
contention-limited rather than CPU-limited): **1.75–1.82 s per temperature** at
load average ~7 (17.9 s for all ten, single-threaded), rising to 3.3–10.7 s at
load average ~19.6 (65.7 s for all ten). Since contention can only inflate a
timing, ~1.75 s is an **upper bound** on the uncontended per-temperature cost;
a quiet machine would be needed to sharpen it. Memory is negligible. Either way
this is a build-time or first-use-cached cost, not a per-query one.

**Scope.** MT=4 only, one moderator, `twt = c = 0`, `nd = 0`. The translational,
diffusive, discrete-oscillator, cold-hydrogen and MT=2 elastic paths are **not**
validated by this.

### End-to-end through `endout`: bit-identical (2026-08-13)

The figures above compare the **unrounded kernel output** against the tape. Run
the whole path instead — deck → kernels → `endout` → ENDF text — and the
residual disappears, because `endout` applies the same `sigfig(x, 7, 0)` /
`sigfig(x, 6, 0)` rounding NJOY applies before storing a value:

| Quantity, MF=7/MT=4 at 296 K | Measured |
|---|---|
| Stored `S` values identical to the official tape | **60,000 / 60,000** |
| max relative deviation (points above 1e-30) | **0.000e0** over 48,941 points |

So for the inelastic channel the 12 KB deck does not approximate the 8.7 MB
tape — it reproduces the published section exactly. Reproduce with
`examples/graphite_sab_generation.rs`; the full V&V record, including the
licence finding that keeps the decks out of the crate, is
`docs/leapr-deck-provenance.md`.

### Regeneration is now the default source

`leapr::generate::thermal_scattering_law` is the consumer surface: ask for a
material at a temperature and get an MF=7 law, regenerated from the deck unless
a tape is named explicitly. Results are cached through the crate's one caching
layer (`acquire::EndfCache`), keyed by a hash of the whole recipe — deck bytes,
temperature, constant set, channels, generator revision. Measured on the same
machine and day: **2.0–2.7 s** cold, **0.009 s** from the disk cache, sub-ms
from the in-process memo.

**MT=2 is validated too, as of the same day, and it is also exact.**
`tests/leapr_graphite_coherent_elastic_parity.rs` closes the elastic channel:
all **221 of 221** thinned Bragg edges retained. With the deck's own vintage
constants the agreement is max **1.001e-13** on both the edge energies and
`S(E, T)` across all ten temperatures — float round-trip noise on a 7-digit ENDF
field. Through `leapr::generate` at 296 K the stored values match to
**0.000e0**.

The vintage matters *differently* for the two channels: MT=4 depends on `bk`
(`tev = bk*T`), MT=2 on `ev`/`amu`/`hbar`/`amassn` (`econ`, the Bragg energy
scale). Correcting only `bk` leaves MT=2 9.986e-7 off as a uniform
multiplicative offset; the full `PhysicalConstants::Njoy2016Legacy` set closes
it. `coher_with_constants` is the entry point that takes the set;
`coher` keeps the old signature and the crate-default constants.

`SabRequest::validation` reports the standing per channel **and per material** —
the 10P/30P porous grades run the identical code with a different deck and have
**not** been measured, so they report unvalidated.

### What a human must still verify (untrusted AI draft)

Outside the graphite MT=4 case above, these remain **verification**
(self-consistency) checks, not **validation** against a reference:

1. **The other moderator classes** — H in H₂O (discrete oscillators +
   translation), liquid H₂/D₂ (cold), and any diffusive case have no golden
   comparison. The graphite result says nothing about them: it exercises
   `contin` alone.
2. **`discre` fidelity** — the discrete-oscillator orchestrator faithfully
   reproduces two NJOY quirks that a reader should confirm against intent: the
   running effective-temperature ratio `tbart` **accumulates across the α loop**,
   and the delta-function placement shares its `idone` flag with the inner
   nearest-β search so at most one delta is placed per α. These are ported
   as-is for line-traceability; verify they match the oracle's numeric output.
3. **`trans` convolution grid** — `sbfill`'s underflow guard (`delta *= 10`) and
   the Simpson weights should be diffed against `leapr.f90` on a real diffusion
   case.

## Caveats

- **No longer merely optional for the ACE path.** ENDF/B does ship MF=7
  evaluations directly, so THERMR can be fed without LEAPR — but the
  ENDF/B-VIII.0 sublibrary also ships the LEAPR *deck* beside every tape, and
  for graphite that deck is 12,444 bytes against the tape's 8,730,804 (a 701×
  saving) and regenerates it to the tape's printed precision in ~1.8 s per
  temperature. It also removes the temperature-grid constraint: S(α,β) can be
  generated *at* an operating point rather than interpolated toward it. The
  physics caveat is that ρ(E) is then reused as if temperature-independent —
  see `LeaprDeck::input_at_temperature`.
- The incoherent-Gaussian approximation has known limits for strongly coherent
  inelastic scatterers.

## References

- NJOY2016 manual §LEAPR (LA-UR-17-20093)
- `leapr.f90` (NJOY2016, git commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`)
- LEAP + ADDELT (UK); GASKET (General Atomics); ENDF-102 File 7
