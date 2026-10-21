# MG anisotropic scattering — the prediction, recorded BEFORE the run

GitHub **#265**. Written 2026-09-22, **before** `examples/mg_scatter_anisotropy_ablation.rs`
was executed for the first time. Nothing below is back-filled; the measured
results live in `ablation_2026_09_22.md` and are compared against this file as
written.

Per the workspace rule — *"state its predicted sign and magnitude before you
measure"* — and per #265's own acceptance criterion.

## The case

The 2-group macroscopic set from `tests/openmc_notebooks/mg_mode_part_i.rs`
(the `mg-mode-part-i` notebook's illustrative constants), cm⁻¹:

| | group 0 (fast) | group 1 (thermal) |
|---|---|---|
| Σ_t | 0.080 | 0.180 |
| Σ_a | 0.010 | 0.080 |
| νΣ_f | 0.008 | 0.100 |
| χ | 1.0 | 0.0 |
| Σ_s,g→0 | 0.050 | 0.000 |
| Σ_s,g→1 | 0.020 | 0.100 |

**ANISO arm.** Every transfer carries the P3 kernel `a = [1, 0.3, 0.1, 0.03]`,
i.e. declared `⟨μ⟩ = 0.3`.

That kernel is **positive everywhere on [−1, 1]**, which matters: its
derivative `0.2925 + 0.75 μ + 0.7875 μ²` has discriminant
`0.5625 − 4(0.7875)(0.2925) < 0`, so it is monotone increasing, and
`f(−1) = 0.195 > 0`. So there is **no negativity truncation** and the sampled
`⟨μ⟩` equals the declared 0.3 exactly (see `physics::scattdata` for why that is
not automatic).

**P0 arm.** The same set through `Mgxs::without_scatter_anisotropy()` —
isotropic in the lab, `⟨μ⟩ = 0`.

Both arms are otherwise bit-identical: same geometry, same seeds, same
constants.

## One-group collapse (the basis of the number)

Infinite-medium spectrum: `(Σ_a0 + Σ_s,0→1) φ0 = S`, `Σ_a1 φ1 = Σ_s,0→1 φ0`, so

    φ1 / φ0 = 0.020 / 0.080 = 0.25,   φ0 : φ1 = 0.8 : 0.2.

Flux-weighted collapse over `(1, 0.25)`:

| | value |
|---|---|
| Σ_t | (0.080 + 0.180·0.25)/1.25 = **0.100** |
| Σ_a | (0.010 + 0.080·0.25)/1.25 = **0.024** |
| Σ_s (row sums 0.070, 0.100) | (0.070 + 0.100·0.25)/1.25 = **0.076** |
| νΣ_f | (0.008 + 0.100·0.25)/1.25 = **0.0264** |

**Check on the collapse itself:** `k∞ = νΣ_f / Σ_a = 0.0264 / 0.024 = 1.10`,
which reproduces the set's closed-form 2-group `k∞ = 1.10` exactly. The
collapse is therefore not a fudge — it is exact for the eigenvalue.

## The prediction

Transport cross section `Σ_tr = Σ_t − μ̄ Σ_s`; `D = 1/(3 Σ_tr)`;
`L² = D/Σ_a`; bare cube of side `S = 2a` with extrapolation
`z₀ = 0.7104 λ_tr = 2.1312 D`, so `S_e = 2a + 2 z₀` and `B² = 3 (π/S_e)²`;
`k_eff = k∞ / (1 + L² B²)`.

| | P0 (μ̄ = 0) | P3 (μ̄ = 0.3) |
|---|---|---|
| Σ_tr | 0.100 | 0.100 − 0.3(0.076) = 0.0772 |
| D | 3.3333 cm | 4.3178 cm |
| L² | 138.89 cm² | 179.91 cm² |

**Cube half-width a = 35 cm** (side 70 cm), vacuum on all six faces:

| | S_e | B² | L²B² | predicted k |
|---|---|---|---|---|
| P0 | 84.208 cm | 4.1754e−3 | 0.57992 | **0.69624** |
| P3 | 88.404 cm | 3.78921e−3 | 0.68173 | **0.65409** |

### Stated before measuring

1. **Sign: NEGATIVE.** Turning the anisotropy on LOWERS `k_eff` in the bare
   cube. Forward-peaked scattering lengthens the transport mean free path,
   which raises leakage out of a finite system. This is the same sign as
   `op-tm9f` on the continuous-energy side, for the same reason.
2. **Magnitude: ≈ −4200 pcm.** Diffusion theory on a 70 cm cube with a
   ~14–18 cm extrapolation distance is not exact, so the honest band is
   **−3000 to −5500 pcm** (±30 %). If the measurement lands outside that band
   the *diagnosis* is in question, not just the arithmetic.
3. **Control — the reflective cube must NOT move.** With zero leakage there is
   no transport-correction term to collect: `k∞ = 1.10` by construction in both
   arms, and the measured difference must be consistent with zero. This is the
   check that distinguishes "a leakage effect was added" from "two errors now
   cancel"; `CLAUDE.md` records the same control being decisive for `op-tm9f`.
4. **The sampled group mean cosine must come out at 0.3**, not merely close —
   this kernel does not go negative, so `Mgxs::group_mean_cosine` must return
   0.3 to quadrature precision in the P3 arm and exactly 0 in the P0 arm.

A result that matches (1) and (3) but misses (2) badly means the diffusion
estimate is the wrong instrument for a cube this leaky, and should be reported
as such rather than quietly dropped.
