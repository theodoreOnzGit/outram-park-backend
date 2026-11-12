# ICSBEP k-eff re-measure after the OpenMC-faithful transport defaults (gh:#1386)

Written 2026-11-12 ~00:35 UTC, **before reading any new ENSEMBLE line**.
Code: `49148da11` (worktree branch `worktree-agent-acefa2821d57a2cf6`).
The 128-seed Jemima run was already in progress (started 23:59 UTC); only
its per-seed lines up to seed 79 had scrolled past, and no pooled value was
computed from them.

Defaults that changed on this branch: OpenMC-faithful collision kernels;
OpenMC reaction order with a single-cutoff level pick; OpenMC fission-neutron
order; per-collision fission banking; direction-norm guard (gh:#1497).
Godiva already re-measured: +4 ± 21 pcm (64 seeds) against −6 ± 5 (1024).

## Old values (read to the end of each correction history)

All Δk are against k = 1.0000 (what each example prints), pcm, ± sem (n seeds).

| case | example doc comment's own pooled value | latest like-for-like record (the baseline used) |
|---|---|---|
| Jemima `jemima_keff.rs` | −253 ± 34 (32), 2026-09-18 — **stale**: pre URR/DBRC default; `icsbep/README.md` records −395 (URR on, no sem) | **−376 ± 17 (128)**, route 4 of `icsbep_five_route_keff`, 2026-10-18 (gh:#931, `route4_process_931/README.md`): same 3 nuclides via `from_endf_file_with_speed`, same 5000 × [40 + 120] |
| HST-009 `hst009_keff.rs` | −38 ± 36 (32), 2026-09-18 — **stale**: pre URR/DBRC, pre U-236 | **+76 ± 14 (96)**, route 4, 2026-10-18 (k 1.00076 ± 0.00014), same 5000 × [40 + 120], U-236 in. Caveat: route 4 takes H-in-H2O from the embedded LEAPR deck since 2026-10-14; the example reads `tsl-HinH2O.endf` via `ThermalScattering::from_endf_file`. Not strictly like-for-like. |
| LCT-008 `lct008_keff.rs` case 1 | +165 ± 25 (32), 2026-09-18 — **stale**: pre URR/DBRC, older nuclide set; root `CLAUDE.md` records +139 after DBRC+URR (no sem) | **No like-for-like record exists** at the example's 4000 × [120 + 250] with 35 nuclides. Nearest: route 4 `lct008-full` +135 ± 23 (32), 2026-10-18, at 10 000 × [40 + 120], LEAPR H-in-H2O. Used as the comparison with that caveat. |

No `RECORDED_*` constant exists in any of the three examples
(`assert_reproduces_keff(.., None)` / `recorded_pcm = None`).

## Prediction (written before the run)

The changed defaults are draw-order and kernel translations that OpenMC
itself uses, verified bit-for-bit at kernel level, and Godiva moved +10 ± 22
(0.5σ). Prediction for each case: **new − baseline within ~2σ of zero**
(σ = quadrature of the two sems), sign not predicted; no case beyond 3σ.
For LCT-008 and HST-009 a 2–3σ move would not by itself implicate the
transport defaults, because the baseline differs in run size / S(α,β) source.

## Hardware

Intel(R) Xeon(R) Processor @ 2.80GHz, 4 logical cores shared with other
agents' builds, 15 GiB RAM, Linux 6.18.44, CPU only. Toolchain 1.99.1
(rust-toolchain.toml), `--release`.
