# REVIEW MANIFEST — GPU-vs-CPU benchmarks (Godiva + HIGH-fidelity TRISO)

> # ⚠️ UNTRUSTED AI DRAFT — REQUIRES HUMAN REVIEW BEFORE IT IS TRUSTED ⚠️
>
> Every file listed below was **AI-drafted** in one agent-fleet session on
> **2026-07-17** and is **untrusted draft material** per `RESPONSIBLE_USE.md` and
> `AI_USAGE.md`. The numbers were produced by actually running the code on real
> hardware (they are not fabricated), but the code, the geometry/material
> choices, the tolerances, and the interpretation all still need human
> inspection, license-provenance review, and V&V sign-off before this work is
> promoted past the "Unit Tested" V&V stage. **Do not cite these numbers in a
> paper or describe this path as validated until a human clears the checklist
> below.** Not for nuclear facility operation, reactor control, safety-critical,
> or licensing decisions.

## Session scope

- **Beads:** op-nx0 (Godiva k_eff benchmark), op-6tz.37 (second TRISO tutorial),
  op-u6s.4 (GPU-XS-into-transport wire-in — partial progress).
- **Scope guard honored:** only `crates/outram-mc-libs/` was touched. No edits to
  `njoy-outram-park-fork` or any other crate. (The `net-fetch` run downloaded
  ENDF data via njoy but edited no njoy source.) No `main` branch changes.
- **Machine:** NVIDIA GeForce RTX 3050, Vulkan backend, `wgpu` 29.0.3 — GPU runs
  were **real, not skipped**.

## What was ACTUALLY measured on the RTX 3050 vs what is PENDING

**Measured (real hardware, 2026-07-17):**
- CPU k_eff / k∞ baselines (wall-clock, throughput, k ± σ) for both cases.
- Isolated **cross-section-interpolation kernel** GPU-vs-CPU throughput and
  f32-vs-f64 agreement, on the real macroscopic Σ_t of each case.

**PENDING (explicitly NOT done — no fabricated number exists for it):**
- **End-to-end GPU-accelerated k_eff / k∞.** The GPU XS kernel is not yet wired
  into the transport collision sweep (op-u6s.4). No GPU eigenvalue speedup is
  claimed. `union_grid.rs` is a **dense-log resample** of Σ_t, not a native-ENDF-
  breakpoint union, and is not yet consumed inside the transport loop.

## Godiva numbers (op-nx0) — LOW tier, offline, reproducible

- CPU **k_eff = 1.01024 ± 0.00171** (+1024 pcm vs ICSBEP 1.0000 ± 0.0010);
  wall 1.224 s; 653 725 histories/s; deterministic.
- XS-kernel GPU speedup **2.49× → 7.39×** (batch 2^16 → 2^20); f32-vs-f64
  max \|Δ\| 1.854e-5 cm⁻¹.

## TRISO numbers (op-6tz.37) — HIGH-fidelity net-fetched ENDF/B-VII.1

- **Net-fetch USED:** all four nuclides (U-234/-235/-238 + H-1 moderator)
  reconstructed from ENDF/B-VII.1 via `Nuclide::from_endf` (RECONR+BROADR),
  total 42.1 s. Accessed 2026-07-17 (IAEA NDS; U tapes were cached).
- **Moderator fidelity limitation:** carbon/graphite is not in the port's
  `well_known_mat` table, so the moderator fell back to **H-1** (still HIGH-tier
  ENDF, but a lighter scatterer than graphite). Documented, not hidden.
- CPU **k∞ = 1.86062 ± 0.00268** (fissile HEU infinite medium, k∞ ≫ 1 expected);
  transport 15.3 s; 1.311e4 histories/s.
- XS-kernel GPU speedup **1.88× → 6.01×**; f32-vs-f64 max \|Δ\| 2.082e-5 cm⁻¹.

## Files in this session (all AI-drafted, all need review)

| File | Kind | Notes |
|---|---|---|
| `src/gpu/union_grid.rs` | NEW source | `UnionTotalXs`: tabulate + batched CPU/GPU Σ_t lookup (op-u6s.4 partial). 2 tests incl. real-GPU agreement. |
| `src/gpu/mod.rs` | edit | added `pub mod union_grid;` + one doc line. |
| `examples/godiva_gpu_benchmark.rs` | NEW example | op-nx0 CPU baseline + XS-kernel GPU throughput; writes 2 CSVs. |
| `examples/triso_gpu_benchmark.rs` | NEW example | op-6tz.37 HIGH-fidelity net-fetch TRISO; writes 2 CSVs; `net-fetch`-gated with no-feature stub. |
| `verification_and_validation/gpu_cpu_benchmarks.md` | NEW V&V doc (committed) | methodology + measured results for both cases. |
| `verification_and_validation/gpu_benchmarks/*.csv` | NEW data (committed) | 4 plottable CSVs. Committed at maintainer request; excluded from the packaged crate via `Cargo.toml`. |
| `Cargo.toml` | edit | broadened `exclude` to keep `verification_and_validation/**/*.csv` out of the published package. |

## Human-verify checklist (for the maintainer)

- [ ] **No fabricated numbers.** Spot-check by re-running both examples and
      confirming the CSVs / doc numbers regenerate to within run-to-run noise.
- [ ] **Godiva LOW-tier bias** (+1024 pcm) is the accepted known offset, not a
      transport regression.
- [ ] **TRISO moderator = H-1 (not graphite)** is acceptable for a HIGH-fidelity
      *path* demonstration; decide whether to add carbon to `well_known_mat`
      (candidate follow-up bead) before this is called a TRISO physics benchmark.
- [ ] **GPU speedup framing.** The CPU reference is single-thread scalar f64;
      confirm the "large-batch win vs single-thread baseline" caveat is
      acceptable, or request a vectorised/multithread CPU baseline.
- [ ] **f32 tolerance** on `union_grid` (`3e-3` rel) is appropriate for real
      resonance Σ_t (revisit for 1e3–1e5 barn peaks, per op-u6s.4).
- [ ] **op-u6s.4 still open** — the native-breakpoint union + in-transport GPU
      sweep are not done; confirm the bead stays open.
- [ ] **License/provenance:** OpenMC (MIT) mirror citations present; ENDF/B-VII.1
      + openmc `triso.ipynb`@cf1e5db provenance recorded.
- [ ] Flip the crate README "V&V — human-reviewed" axis only after this review.

## Gate status at hand-off (all green)

- `cargo test -p outram-mc-libs --lib --release` — 78 passed, 0 failed (incl.
  both `union_grid` tests; GPU test ran on the RTX 3050, not skipped).
- `cargo build -p outram-mc-libs --release --example godiva_gpu_benchmark
  --example triso_gpu_benchmark` — clean; `triso_gpu_benchmark` also builds both
  with and without `--features net-fetch`.
- `cargo check -p outram-mc-libs --lib --target aarch64-linux-android` — clean
  (GPU path cfg-gated out).
