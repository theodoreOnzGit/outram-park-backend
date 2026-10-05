# Coverage: every module, its lesson, its walk, its record

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response.

> **Review status: first draft, 2026-10-04, AI-assisted, not yet reviewed by a
> human.**

Every public module of `njoy-outram-park-fork` (the `pub mod` list of
[`src/lib.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/lib.rs),
checked 2026-10-04) has a row. A module with no lesson, no walk or no record
is a **visible gap**, not a silent omission. "Verified" here always means
**compared with NJOY2016 or OpenMC on the same input**: verification, not
validation.

**Status key:** **V** verified against another code, record named; **P**
partly verified (what is not is stated on the page); **U** ported, no
cross-code record; **N** `NotPorted` stub; **T** tooling, no physics.

| module | lesson section | code walk | V&V record | status |
|---|---|---|---|---|
| `endf` | [Rung 1](./endf.md) | `Tape::read_file` (rung 1) | indirect: every NJOY comparison reads through it; float-parser defect record in `endf/parse.rs` | V |
| `moder` | [Rung 1](./endf.md), supporting modules | `moder::select_materials` ([appendix](./call-trees.md)) | ~~none against NJOY; README says verified by hand~~ `moder_vs_njoy2016.md` (2026-10-05): selection and every `a11` field match NJOY2016's MODER; text, `i11` and blank fields do not ([#536](https://github.com/theodoreOnzGit/outram-park-backend/issues/536)) | ~~**U — gap**~~ **P** |
| `reference_data` | [Rung 1](./endf.md) | appendix | `tests/no_endf_inside_crates.rs` (layout) | T |
| `acquire` | [Rung 1](./endf.md) | — | none (cache substrate; download path behind `net-fetch`) | T |
| `reconr` | [Rung 2](./reconr.md) | `reconr::reconr` | `tests/pendf_stages_vs_njoy2016.rs`, `tests/reconr_lrf7_threshold_channels_vs_njoy2016.rs`, `reconr_sr88_lrf7_kbk_vs_njoy2016.md` | V |
| `samm` | [Rung 2](./reconr.md) (Fe-57 case study), [Rung 8](./errorr.md) (derivatives) | `rml::add_rml_range` (appendix) | `reconr_sr88_…`, `samm_lrf7_errorr_mf32_cl35_vs_njoy2016.md` | V |
| `broadr` | [Rung 3](./broadr.md) | `broaden_result_with_tolerance` | `tests/pendf_stages_vs_njoy2016.rs`, `tests/acer_broadening_vs_njoy2016.rs`, `examples/seam_stage_probe.rs` | V |
| `unresr` | [Rung 4](./purr.md) | `unresolved_cross_sections` (appendix) | `tests/reconr_urr_kernel_vs_njoy2016.rs`, `tests/purr_u235_urr.rs` | V (kernel); PENDF MT=152 output and `run()` not ported |
| `purr` | [Rung 4](./purr.md) | `UrrProbabilityTables::from_endf` | `unr_block_write_2026_09_26.md`, `tests/unr_block_write_vs_njoy2016.rs` | V; PENDF MT=153 writer not ported |
| `thermr` | [Rung 5](./thermr.md) | `IncoherentInelasticScattering::from_tape`, `calcem_inelastic_xs` | `tests/thermr_calcem_vs_njoy2016_golden.rs` | V; `nmix > 1` and the card driver not ported |
| `leapr` | [Rung 5](./thermr.md) | `leapr::run_deck` (appendix) | `groupr_gaminr_covr_leapr_vs_njoy2016.md` §4.3–4.4, `examples/graphite_sab_generation.rs` | V; `coldh` self-consistency only |
| `heatr` | [Rung 6](./heatr.md) | `Kerma::from_reconr` | `heatr_vs_njoy2016.md` | **P** — kinematic limit, plus a photon-only energy-balance correction on the ACE route (not compared with NJOY); full energy balance (H6) and most damage channels not ported; ~~MT=445 threshold discrepancy open~~ MT=445 threshold diagnosed 2026-10-05 (NJOY interpolates between 10 % nodes) |
| `gaspr` | [Rung 6](./heatr.md) | `GasProduction::from_reconr_and_tape` | `gaspr_light_nuclides_vs_njoy2016.md` | V |
| `photon` | [Consumer surface](./consumer-surface.md) | `PhotonProduction::from_endf` (appendix) | indirect: ACE photon blocks word-identical (rung 9); `tests/photon.rs` | P; MF=12 `LO = 2` not ported |
| `groupr` | [Rung 7](./groupr.md) | `self_shielded_group_xs` | `groupr_gaminr_covr_leapr_vs_njoy2016.md` §3, five `tests/groupr_*_golden.rs` | V |
| `gaminr` | [Rung 7](./groupr.md) | `gaminr::run_with_input` (appendix) | `gaminr_u_photoatomic_vs_njoy2016.md` | ~~**P — gap**: MF=26 coherent/incoherent matrices disagree with NJOY on real uranium~~ **V** (2026-10-05): all reactions incl. MF=26 matrices ≤ 3.7e-7 on real uranium once NJOY's deck uses separate units ([#534](https://github.com/theodoreOnzGit/outram-park-backend/issues/534)) |
| `errorr` | [Rung 8](./errorr.md) | `errorr::run_mf33` | `tests/errorr_mf33_golden.rs`, three `errorr_mf32_*` records | V (MF=33, MF=32 paths named); MF=31/34/35/40 not ported |
| `covr` | [Rung 8](./errorr.md) | `covr::run_library` | `tests/covr_boxer_golden.rs` | V (library option); plotting not ported |
| `acer` | [Rung 9](./acer.md) | `build_full_with_purr` | `ace_block_parity_2026_09_26.md`, `acer_*_vs_njoy2016.md` (9 records) | V |
| `wimsr` | [Output formats](./output-formats.md) | `wimsr::run_gendf` | `wimsr_u238_vs_njoy2016.md` | V; card-deck `run()` not ported |
| `dtfr` | [Output formats](./output-formats.md) | listed by hand | `tests/dtfr_u238_claw_*_golden.rs` (worst 4.9e-6, first recorded [2026-10-04](./remeasured.md)) | V |
| `resxsr` | [Output formats](./output-formats.md) | listed by hand | `tests/resxsr_h2_njoy_golden.rs` (byte-identical) | V |
| `mixr` | [Output formats](./output-formats.md) | listed by hand | `tests/mixr_h2_be9_njoy_golden.rs` (exact, first recorded [2026-10-04](./remeasured.md)) | V |
| `matxsr` | [Output formats](./output-formats.md) | — | — | **N** |
| `ccccr` | [Output formats](./output-formats.md) | — | — | **N** |
| `powr` | [Output formats](./output-formats.md) | — | — | **N** |
| `plotr` | [Output formats](./output-formats.md) | — | — | **N** |
| `viewr` | [Output formats](./output-formats.md) | — | — | **N** |
| `hdf5` | [OpenMC interchange](./openmc-interchange.md) | `write_nuclide` | `hdf5_nuclide_write/`, `nuclide_h5_fissile/`, `nuclide_h5_read/`, `depletion_chain_xml/` | P; fissile `k` comparison and read-side `k` comparison not yet run |
| `wmp` | [Windowed multipole](./wmp.md) | `XsProvider::micro` → `evaluate` | `wmp_h5_write_2026_09_24.md`, `tests/wmp_arbitrary_temperature.rs` | P; WMP vs direct BROADR not done; ~~pole-count discrepancy (602 vs 4 062) between two records, not re-checked~~ pole count re-checked 2026-10-05: **4 062** (602 was a KB size, [#536](https://github.com/theodoreOnzGit/outram-park-backend/issues/536)) |
| `gpu_wmp` | [Windowed multipole](./wmp.md) | `wmp_evaluate_batch_cpu` | per-machine report only (methodology template) | P |
| `gpu` | [Windowed multipole](./wmp.md) | — | `gpu::probe` unit tests | T |
| `nuclear_data` | [Rung 0](./where-sigma-comes-from.md), [Consumer surface](./consumer-surface.md) | `XsProvider::micro` | consumers' records (`outram-mc-libs` ICSBEP five-route) | P |
| `interface` | [Consumer surface](./consumer-surface.md) | `reconstruct`, `broaden` | none of its own (wraps verified stages) | U |
| `prelude`, `units`, `common`, `modules`, `error` | [Consumer surface](./consumer-surface.md) | — | — | T |
| `vv` | [Consumer surface](./consumer-surface.md) | — | is the gate mechanism | T |
| `perf_report`, `wasm_par` | [Consumer surface](./consumer-surface.md) | — | `tests/wasm_fallback.rs` | T |
| `bin/njoy-tui` | [Consumer surface](./consumer-surface.md) | — | `docs/njoy-tui.md` | T |

## Gaps this table makes visible

Each is tracked as a GitHub issue, filed 2026-10-04 by this track:

1. ~~**GAMINR's MF=26 coherent and incoherent scattering matrices disagree with
   NJOY2016** on the real ENDF/B-VIII.0 uranium photo-atomic evaluation;
   undiagnosed.~~ **Resolved 2026-10-05:** the NJOY deck shared one unit
   between the ENDF and PENDF inputs; with separate units the port agrees to
   3.7e-7 everywhere, unchanged. [#534](https://github.com/theodoreOnzGit/outram-park-backend/issues/534)
2. **HEATR's energy-balance method (H6) is not ported**, so the port's KERMA is
   the kinematic limit (with a photon-only correction on the ACE route); ~~plus
   the open MT=445 damage threshold discrepancy on Fe-58~~ the MT=445 threshold
   difference on Fe-58 is diagnosed (2026-10-05): it is `disbar`'s 10 %-node
   interpolation in NJOY, not a port defect. [#535](https://github.com/theodoreOnzGit/outram-park-backend/issues/535)
3. ~~**MODER has no cross-code comparison** of a tape it writes.~~ **Compared
   2026-10-05:** the selection and all 59 098 `a11` number fields match the
   tape NJOY2016's MODER writes; the writer is not byte-faithful (`i11`
   integers, blank fields and the MF=1/MT=451 text are not reproduced, and
   the text is lost). [#536](https://github.com/theodoreOnzGit/outram-park-backend/issues/536)
4. ~~**WMP: two records disagree on U-238's pole count** (602 against 4 062).~~
   **Resolved 2026-10-05:** `WmpLibrary::core().get("U238")` has **4 062 poles
   in 4 309 windows**; the 602 was U-238's size in KB in
   `docs/wmp-nuclide-manifest.md`, copied into the GPU benchmark's docs as a
   pole count. Corrected with strike-throughs there. [#536](https://github.com/theodoreOnzGit/outram-park-backend/issues/536)
5. **Not ported by design** (visible, no issue needed unless the maintainer
   wants them): MATXSR, CCCCR, POWR, PLOTR, VIEWR, every card-deck `run()`.
