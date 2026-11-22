# Predictions, written BEFORE any comparison run (gh:#1426, 2026-11-13)

Committed ahead of the first run so it can fail. Upstream: `lcpp-org/RustBCA`,
tag **v3.0.0 = `673bd3f2b907b14517a6b3cafa1c5372a5eb4ae2`**, cloned over git into
scratch, never modified, built outside the workspace with its own target directory.

**Provenance finding, made while reading (not a run).** `src/rustbca/mod.rs` and
`AGENTS.md` say "v3.0.0, `a356280d`". `a356280d` (2026-10-02, "Merge PR #345 fix-direction-scaling") is a **post-tag commit on `main`**:
tag v3.0.0 is its ancestor, 17 commits earlier (`git log v3.0.0..a356280d`). The copy is therefore
v3.0.0 **plus** those commits, not the tag. `git diff v3.0.0 a356280d -- src` touches only `consts.rs`
(adds `MAX_DENSITY`), `geometry.rs`/`sphere.rs`/`parry.rs` (the `MAX_DENSITY` asserts) and `input.rs`
(removes the `length_unit` multiplication of sampled direction cosines, the bug of #345).
**`bca.rs`, `interactions.rs`, `material.rs`, `particle.rs`, `physics.rs`, `math.rs`, `structs.rs`,
`enums.rs` are byte-identical** between the two commits. So the kernel comparison below is valid against either; the copy is of `a356280d`, not of the tag.

## What is compared (one shared harness, `shared.rs`, drives both codes)

1. **Layer 1, kernel, no RNG.** `bca::calculate_binary_collision` for H/He/Ar on W/Si/C,
   5 potentials (KR_C, ZBL, MOLIERE, LENZ_JENSEN, TRIDYN) x 4 scattering integrals (MW,
   GM10, GL, MAGIC) x 4 energies (100 eV to 100 keV) x 6 impact parameters = 4320 rows x 6 outputs.
2. **Layer 2, functions.** `screening_length`, `interaction_potential`, `phi`, `dphi`,
   Lindhard-Scharff and Bethe-Bloch cross sections.
3. **Layer 3, seeded cascades.** 90 cases x 8 ions, `ChaCha8Rng::seed_from_u64(20261113)`
   then `set_stream(case*1000+ion)` (upstream's own per-particle pattern); every output
   particle and the incident ion's collision-by-collision (E, x, y, z). The issue proposed a
   shim RNG; unnecessary, because boon-lay's `petir::rng::ChaCha8Rng` is a draw-for-draw port
   (gh:#995) and a seeded stream is therefore identical on both sides. (The issue text saying
   "the random stream differs from upstream by design" is out of date since #995: struck through
   in the gap survey.)

## Predictions

Mechanism: the only intended source of difference is `libm` (Rust `libm`, musl-derived, via
`petir::real::Real`) versus the platform glibc in upstream's `f64::exp/sin/cos/atan2/powf/ln/cbrt`.
Both are accurate to < 1 ulp; the port doc says they differ in the last ulp on "a few per cent of calls".

- **P1 (functions, layer 2):** outputs depending on one transcendental call are bit-identical
  in >= 95 % of rows; worst relative difference <= 4.5e-16 (2 ulp) per call, <= 1e-14 for
  compound expressions (`interaction_potential`, Bethe-Bloch with `ln`). Any larger difference is a
  port defect, not libm.
- **P2 (kernel, layer 1):** >= 70 % of the 4320 x 6 numbers bit-identical (each kernel evaluation
  chains tens of transcendental calls and a Newton root finder, so more rows drift than in layer 2);
  worst relative difference <= 1e-12 for `theta`, `psi`, `psi_recoil`, `recoil_energy`; `ERR`/`PANIC`
  status identical on every row. Near-grazing rows (large impact parameter, theta ~ 1e-8) may
  amplify cancellation: worst relative difference there <= 1e-9.
- **P3 (cascades, layer 3):** because the RNG stream is identical, divergence needs a *branch flip*
  (a comparison against a cutoff moved by an ulp). Prediction: >= 50 % of ions are bit-identical in
  every output particle; among the rest the incident ion's collision count differs in <= 15 % of
  all ions; the particle count (`N` line) is equal in >= 90 % of ions. Where an ion's collision
  sequence is identical, relative difference of final energy <= 1e-10. I expect the longest-cascade cases
  (Ar 10 keV on W) to be the least likely to stay bit-identical.
- **P4 (no defect):** no kernel or cascade difference that is not explained by a libm 1-ulp first
  divergence. If the first divergent operation of a mismatch is NOT a transcendental call, it is a
  port defect, to be fixed by translating upstream.

Pass criterion for the pin test: counts of bit-identical rows (per layer) equal the recorded counts,
and the worst relative difference stays under the recorded bound. The bounds are set from the
measured values (stated as such), never loosened afterwards.
