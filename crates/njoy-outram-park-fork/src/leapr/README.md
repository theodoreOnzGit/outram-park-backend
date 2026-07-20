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

- **`run()` / card-input driver + `endout` MF=7 tape writer** (2972–3623) and
  `copys` (2468–2487): the ENDF-6 File-7 output plumbing. `run()` returns
  `NjoyError::NotPorted`; use the typed module API directly.
- **`coldh` orchestrator** (1936–2183): the Young–Koppel rotational
  convolution loop. Its self-contained helpers (`bt`, `sumh`, `cn`, `sjbes`,
  `terpk`) **are** ported and tested; `coldh::coldh()` returns `NotPorted`.
- **Coherent elastic** `coher`/`formf`/`tausq`/`taufcc`/`taubcc`
  (2489–2814, 2924–2970): the Bragg-edge calculation. The **consuming** side
  already exists in `crate::thermr::coherent`, so this is deferred with a
  pointer rather than duplicated.
- **`skold`** (2816–2922): the Sköld pair-correlation correction.

## Testing — methodology and results (2026-07-15)

Ran under the 12 GB cap via `scripts/test.sh leapr::`. **20 tests, 20 passed,
0 failed.** All builds/tests in `--release`; `cargo check --lib --tests` is
clean (0 warnings).

Closed-form / self-consistency V&V checks with **measured numbers**:

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

### What a human must still verify (untrusted AI draft)

These are **verification** (self-consistency) checks, not **validation** against
a reference. Before trusting the port:

1. **End-to-end validation** — regenerate S(α,β) for a standard moderator (H in
   H₂O, graphite) from its published phonon spectrum and compare against an
   upstream LEAPR MF=7 tape (and downstream THERMR cross sections) within
   tolerance. No such golden comparison has been run yet.
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

- **Not currently needed for the ACE path** — ENDF/B ships MF=7 thermal
  evaluations directly, so THERMR is fed without LEAPR. Port on demand
  (new/custom moderators).
- The incoherent-Gaussian approximation has known limits for strongly coherent
  inelastic scatterers.

## References

- NJOY2016 manual §LEAPR (LA-UR-17-20093)
- `leapr.f90` (NJOY2016, git commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`)
- LEAP + ADDELT (UK); GASKET (General Atomics); ENDF-102 File 7
