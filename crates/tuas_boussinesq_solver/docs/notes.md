# tuas_boussinesq_solver — workspace & migration notes

Reference material: OUTRAM PARK workspace integration, the 2026-06 dependency
migration log, and the v0.1.1 CustomSolid/CustomLiquid integration-hang bug
writeup. Consulted on demand, not per turn. Mandatory rules and the crate
architecture live in CLAUDE.md.

## OUTRAM PARK workspace notes

> This crate is now a member of the **OUTRAM PARK** workspace
> (`crates/tuas_boussinesq_solver`). See the workspace root `CLAUDE.md` for the
> shared dependency policy and full migration history. Dependencies are inherited
> from the root `[workspace.dependencies]` — **do not** pin versions in this
> crate's `Cargo.toml` (`uom.workspace = true`, etc.).

The "Update dependencies" instructions above (`cargo upgrade` / `cargo update`)
no longer apply per-crate: change shared versions in the **root** `Cargo.toml`.

### v0.1.1 patch (2026-06)

**Bug fix — `CustomSolid` / `CustomLiquid` enthalpy integration hang**

**Symptom:** Any code constructing a component with `SolidMaterial::CustomSolid`
or `LiquidMaterial::CustomLiquid` would silently freeze at startup — no panic,
no error, just an infinite hang. Built-in materials (`SteelSS304L`, `Copper`,
`FLiBe`, etc.) were unaffected because they use pre-computed analytical splines
for enthalpy, bypassing numerical integration entirely.

**Call chain where the hang occurs:**

```
InsulatedFluidComponent::new_insulated_pipe  (or any component constructor)
  └─ SolidColumn::new_cylindrical_shell  (shell material = CustomSolid)
       └─ try_get_h(CustomSolid, T)
            └─ solid_specific_enthalpy
                 └─ get_custom_solid_enthalpy
                      └─ peroxide::integrate(cp_fn, (T_low, T), G20K41(1e-9, 100))
                           ↑ HANGS HERE (same path for CustomLiquid)
```

**Root cause:** `peroxide` 0.37 → 0.41 made the `G20K41` Gauss-Kronrod variant
**fully adaptive**: it keeps subdividing the integration interval until the
absolute error drops below the specified tolerance. The code used
`Integral::G20K41(1e-9, 100)` — absolute tolerance 1 nJ/kg. For a cp function
integrated over a few hundred kelvin, the result is on the order of MJ/kg;
achieving 1 nJ/kg absolute accuracy is numerically impossible, so the integrator
subdivided exponentially and never terminated.

**Fix:** Switch to a fixed non-adaptive Gauss-Legendre quadrature:

```rust
// before (hangs with peroxide ≥ 0.41 — adaptive absolute tolerance impossible to satisfy)
let abs_tolerance = 1e-9;
let integration_method = Integral::G20K41(abs_tolerance, max_iterations);

// after
let integration_method = Integral::GaussLegendre(20);
```

`GaussLegendre(20)` is non-adaptive — evaluates at exactly 20 fixed points and
returns immediately. No convergence loop, no subdivision. For smooth cp
functions (polynomial/rational in T) this gives machine-precision accuracy.

`G20K41R` (relative tolerance) was tried first but caused a severe performance
regression: `tutorial_6` (3000 timesteps × many CV nodes × bisection root-
finding iterations) went from 0.15 s (original) to >3 min. Even though each
G20K41R call converges quickly, the adaptive machinery overhead compounds
across millions of calls. `GaussLegendre(20)` runs `tutorial_6` in **0.06 s**
— faster than the peroxide 0.37 baseline.

**Affected files:**
- `src/lib/boussinesq_thermophysical_properties/solid_database/custom_solid_material/mod.rs`
- `src/lib/boussinesq_thermophysical_properties/liquid_database/custom_liquid_material/mod.rs`

**Test to watch:** `tutorial_6` in
`pre_built_components/insulated_pipes_and_fluid_components/tutorials/` directly
constructs a `CustomSolid` graphite pipe and would have hung before this fix.
The `gfhr_pipe_tests` reference graphite only in comments and use built-in
materials — they were not affected.

---

### Migration notes (2026-06)

- Moved into the workspace; standalone git history dropped; dev-deps (`chem-eng…`,
  egui stack) now resolve to in-tree path crates rather than crates.io.
- Bumped to latest stable: `uom` 0.36→0.38, `ndarray` 0.15→0.17,
  `ndarray-linalg` 0.16→0.18, `peroxide` 0.37→0.41, `thiserror` 1→2,
  `csv` 1.3→1.4, egui/eframe 0.29→0.34, `egui_plot`→0.35. The **library and all
  test suites compile cleanly** on these versions with no source changes — the
  ~150 earlier test errors were purely a duplicate-`uom` artifact from the old
  crates.io `chem-eng…` (fixed by unifying `uom` across the workspace).
- ✅ **`examples/ciet_educational_simulator` migrated to egui 0.34** (builds &
  links). `app.rs`: `eframe::App::update` → `ui(&mut self, ui, frame)` with
  `let ctx = ui.ctx();`. The per-page plot files under
  `app/panels_and_pages/` (`ctah_page`, `ctah_pump_page`, `dhx_page`,
  `heater_page`, `tchx_page`): `egui_plot::Line::new(points).name(s)` →
  `Line::new(s, points)` (20 sites). Two HTC plots had no `.name()` and were
  labelled `"CTAH HTC"` / `"TCHX HTC"`. Deprecated `Panel::show` /
  `egui::menu::bar` warnings were left as-is (non-blocking).
