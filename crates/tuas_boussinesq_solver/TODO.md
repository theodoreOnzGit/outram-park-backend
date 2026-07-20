# tuas_boussinesq_solver — TODO

## Android portability (assessed 2026-07-04)

TUAS is the thermal-hydraulics backend behind the `fhr_sim_v2` simulator in
`tampines-steam-tables`, whose Android port was found feasible. The key enabling
fact lives here:

- **TUAS has no native BLAS dependency.** It uses `peroxide` (pure-Rust, default
  features — no `O3`/LAPACK) + `ndarray`, *not* `ndarray-linalg`/OpenBLAS. This
  is what makes the whole `tampines → tuas` chain cross-compile cleanly for
  `aarch64-linux-android`. Do **not** add `ndarray-linalg` or enable peroxide's
  `O3` feature to this crate without accounting for the Android target — it
  would drag in a system BLAS that has no Android build here.

- [ ] If the `ciet_educational_simulator` eframe example is ever targeted at
  Android, it needs the same example→`cdylib` + `android_main` restructuring and
  the `egui_extras` `all_loaders → ["image"]` change described in
  `../tampines-steam-tables/TODO.md`. The BLAS-free story above already holds, so
  no numerics work is required.

See `../tampines-steam-tables/TODO.md` and the workspace-root `TODO.md` for the
full findings and work-item list.
