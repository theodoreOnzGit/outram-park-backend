//! **HTR-10 core k-eff against the RMC benchmark** — `bn:op-867c`, gh #214.
//!
//! The epic's acceptance criterion. Explicit TRISO, hybrid delta/surface
//! tracking, reflector from IAEA-TECDOC-1382 Table 4-3.
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_rmc_keff
//! OUTRAM_HTR10_HISTORIES=20000 OUTRAM_HTR10_RINGS=14 cargo run --release ...
//! ```
//!
//! # STATUS
//!
//! ~~**2026-09-17: THIS DOES NOT WORK YET.** First run (8 rings x 12 layers,
//! 1500 histories) returned `k_eff = 0.000000 +/- 0.000000` with 2,820,163,146
//! virtual collisions and zero entropy. Two findings: the majorant cost was as
//! predicted (~22 rejections per real collision) and NOT the failure; `k = 0`
//! was a separate, un-isolated bug, suspected to be the source box, the empty
//! helium material, or `material_at` returning `None`. **Do not treat this
//! example as a result** — it computes no eigenvalue.~~
//!
//! **CORRECTED 2026-09-18 — it works, and every hypothesis quoted above was
//! wrong.** `k = 0` was none of those three. The cause was a **port defect in
//! `HexLattice::distance`**: its axial branch compared a lattice-frame `z`
//! against a tile-local bound, returning NEGATIVE distances so neutrons stepped
//! backwards and oscillated until the event budget killed them. A
//! budget-exhausted history is scored as a *leak*, so the neutron balance
//! closed and `k` reported no error at all. The majorant was never implicated.
//!
//! Three silent geometry defects followed it, all costing fuel rather than
//! histories: the lattice axial centre, the ring count, and the bed cylinder
//! being circumscribed about the tiled hexagon instead of inscribed in it. The
//! largest single reactivity term turned out to be a missing **void** — 98.758
//! cm of helium core cavity above the bed that had been modelled as graphite.
//!
//! ~~**Current result**~~ **Result of 2026-09-18, superseded** (gh:#428: every
//! number in this STATUS section predates the explicit reflector, the 30P
//! graphite law and Şeker's bed; see the note at the end of the section)
//! (14 rings x 25 layers, 10000 histories x [40 inactive +
//! 120 active], surface tracking, ENDF/B-VIII.0):
//!
//! ```text
//! k_eff = 0.995200 +/- 0.001082     RMC 1.004288     -909 +/- 108 pcm
//! lost locate = 0   stuck events = 0   negative distances = 0
//! ```
//!
//! That is inside the 500-1000 pcm gate. **It is the gate being met, not a
//! validated model** — see the qualifications below, and note in particular
//! that this is a SINGLE SEED (`seed: 20260917`), as is every ablation in the
//! V&V record. Pooled multi-seed re-measurement is gh:#196 / `bn:op-awwi` and
//! has NOT been done, so quote this as one draw, not as a mean.
//!
//! # UPDATE 2026-09-18 — the conus is now modelled, and it OVERSHOOTS
//!
//! The 0.995200 above was measured with a **flat-bottomed** bed, omitting the
//! 36.946 cm conus of pebbles beneath it. Modelling the conus (`op-5n34`,
//! `HTR10_CONUS_HEIGHT_CM`) adds 14.8 % kernel volume and is worth
//! **+4578 +/- 158 pcm** (29 sigma), taking the same settings to
//!
//! ```text
//! k_eff = 1.040984 +/- 0.001148     RMC 1.004288     +3670 +/- 115 pcm
//! ```
//!
//! The predicted SIGN was confirmed — adding fuel raised `k`. The magnitude
//! **overshoots the 909 pcm it was meant to close by a factor of five.**
//!
//! ~~**So the -909 pcm above was agreeing for the wrong reason.** A model
//! missing 13.6 % of its fuel volume cannot be 909 pcm low by accident;
//! something else is over-reactive by a few thousand pcm.~~
//!
//! # CORRECTED 2026-09-18 (later) — the conus contents were wrong
//!
//! The overshoot was not a masked error elsewhere. **The conus was filled with
//! FUEL pebbles, and it holds only dummy ones.** Terry et al. (2005) §2, in
//! this repo's own derived geometry
//! (`kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md:256`):
//!
//! > *"the conus and discharge tube contained only **dummy** pebbles"*
//!
//! The conus is part of the bed hex lattice, and `bed_tile_levels` applied the
//! core's 57:43 fuel:dummy split to every level. (Since 2026-09-25 the
//! explicit-TRISO bed assigns fuel per BALL, ~~through `bed::TwoBallBed`~~
//! since 2026-10-01 through `bed::SekerBed` (gh:#472; the two-ball bed is
//! withdrawn), with the conus all-dummy by construction.) Extending the lattice to the
//! conus floor therefore filled it with fuel. The geometry was right; the
//! contents were not. Correcting it is worth **-5177 +/- 420 pcm (12 sigma)**.
//!
//! The same sentence covers the DISCHARGE TUBE, which was solid reflector
//! graphite (over-reflecting the conus tip) — ~~now pebble graphite at the
//! bed's 0.61 filling fraction, between that bound and the pure-helium one~~
//! then a 0.61 smear of pebble graphite; **since 2026-09-25 explicit whole
//! graphite balls**, rejected at the cone and tube (CORRECTED 2026-10-01,
//! gh:#428; the smear is only the `OUTRAM_HTR10_HOMOG_TUBE` ablation).
//!
//! **The "two offsetting errors" reading is withdrawn.** The flat-bottomed
//! model was not missing fuel; it was missing the conus's *dummy* pebbles and
//! had reflector graphite there instead, worth only about -680 pcm.
//!
//! ~~Current~~ The 2026-09-18 physical model, 3000 histories x [20 + 60]:
//!
//! ```text
//! single seed : k_eff = 0.991372 +/- 0.003002   ->  -1292 +/- 300 pcm
//! 8 seeds     : pooled dk = -1592 pcm, sem +/-63, seed-to-seed sd 179
//! ```
//!
//! **Quote the pooled number.** The single draw sits 1.7 sd off it. At
//! `sd = 179 pcm` one run of this case re-randomises by that much, so a
//! single-seed residual is not quotable to better than a few hundred pcm --
//! `OUTRAM_BENCH_SEEDS=n` runs the ensemble (gh:#196 / `bn:op-awwi`).
//!
//! Full ablation chain, methodology and results:
//! `crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`.
//!
//! **Note 2026-10-01 (gh:#428).** None of the numbers above describes the
//! current default. Since they were taken: the reflector became explicit
//! 3-D geometry with withdrawn rods (PR #327), graphite took the 30P law
//! (2026-09-27), the reference is matched on the paper's whole-ball height and
//! then by ball count (gh:#333, gh:#472), and the bed became Şeker & Çolak
//! (2003)'s 13-ball cell (gh:#472). The later records are in
//! `crates/outram-mc-libs/verification_and_validation/htr10_rmc/` (e.g.
//! `fast_ablation_2026_09_26.md`).
//!
//! # Read this before quoting any number it prints
//!
//! - ~~**ENDF/B-VIII.0**; RMC, MCNP, Serpent and HCP all used **VII.0**. On a
//!   graphite-moderated LEU system that difference alone is worth hundreds of
//!   pcm, so a disagreement CANNOT be attributed to transport.~~
//!   **MEASURED 2026-09-18 — it is worth `+1644 +/- 438 pcm` (3.75 sigma), and
//!   it was essentially the whole residual.** Run `OUTRAM_HTR10_ENDF7=1` for
//!   the reference's own library: `k = 1.004525 +/- 0.003096`, i.e.
//!   **`+385 +/- 310 pcm` from RMC height-matched, 1.24 sigma** — agreement,
//!   against `-1259 pcm` on VIII.0. The warning was right that a disagreement
//!   could not be attributed to transport; it understated the size by 5x.
//!   Single seed — pool before quoting.
//!   **CORRECTED 2026-10-01 (gh:#428) on who used VII.0:** Li, Yu & Wei
//!   (2014) state ENDF/B-7.0 for RMC (abstract and § III). Their MCNP columns
//!   are Şeker & Çolak (2003)'s results (`htr10_rmc` module docs), and Şeker
//!   p.265 used **ENDF/B-VI**, with TMCCS graphite. Serpent and HCP are not in
//!   Li's paper: Not re-checked, no source for their library was found.
//! - The reference quotes **no uncertainty** on any of its twelve values.
//! - ~~The reflector densities are **R-Z homogenised**; TECDOC says a 3-D model
//!   must correct them for the boring geometries. Unadjusted, they smear the
//!   control-rod and helium-flow channels uniformly.~~ **CORRECTED 2026-10-01
//!   (gh:#428):** since PR #327 the borings are explicit 3-D geometry and the
//!   zone densities carry TECDOC p. 242's corrections
//!   (`htr10_rmc::reflector_geometry`, `core_model::mat::for_zone_mc`).
//! - ~~**No control rods or absorber balls** are modelled.~~ **CORRECTED
//!   2026-10-01 (gh:#428):** the ten rods are explicit at their withdrawn
//!   position (B4C, steel, iron; `OUTRAM_HTR10_NO_WITHDRAWN_RODS=1` empties
//!   them). The absorber-ball (KLAK) and irradiation channels are empty
//!   (maintainer, gh:#330); no absorber balls are modelled.
//! - ~~The realised TRISO count is **8340**, not 8335 — unattainable, see
//!   `cubic_array_in_ball`.~~ **CORRECTED 2026-10-01 (gh:#430):** 8335, as
//!   stated (Şeker & Çolak 2003 p.266), through a generic lattice offset.

use std::time::Instant;

use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;
use nee_soon::htr10_rmc::core_model::assemble_explicit_triso;
use nee_soon::htr10_rmc::data::{
    load_htr10_nuclides, CarbonTreatment, Coolant, Htr10DataConfig, Htr10DataError,
    Htr10NuclideLayout, NuclearDataLibrary, RodMetalTreatment, ThermalScatteringTreatment,
    TapeSource, U238Evaluation, Uo2Laws,
};
use nee_soon::htr10_rmc::reflector::zone_composition;
use nee_soon::htr10_rmc::materials::GraphiteLaw;
use outram_mc_libs::run_diagnostics::RunDiagnostics;
use outram_mc_libs::pebble_beds::htr10::BoronReading;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg_hybrid;

const TEMP_K: f64 = 300.15;
/// RMC's value at the **123.576 cm** loading height.
///
/// Kept as the historical comparison point, but **do not compare against it
/// blind** -- see [`rmc_at_height`]. ~~The bed this example builds is
/// `n_axial x 4.899` cm tall (`2 * bed_half_height`; not `lat_height *
/// n_axial`, which stopped being true on 2026-09-25 when the tile became the
/// two-ball 9.798 cm prism), which at the default layer count is NOT
/// 123.576 cm,~~ **CORRECTED 2026-10-01 (gh:#428):** the bed is Şeker's,
/// `9.798 N + 6` cm, so the default N = 12 IS 123.576 cm tall; but it holds
/// 1.2 % fewer balls than Şeker's model at that height, so the reference is
/// read at the equal-ball-count height (122.091 cm at N = 12, gh:#472). RMC's
/// own curve is steep enough (~270 pcm/cm near this point) that the mismatch
/// is a real systematic rather than a rounding detail.
const RMC_KEFF: f64 = 1.004288; // 123.576 cm loading height

/// The paper's loading height for a bed built with `n_axial` ball layers:
/// whole-ball extent, `(n_axial - 1)` layer pitches of 4.899 cm plus one ball
/// diameter (gh:#333, see [`rmc_at_height`]).
///
/// **Since 2026-10-01 (gh:#472) a fallback only.** `main` uses it only when
/// the bed reports no ball count, and Şeker's bed (the only bed that can be
/// built) always reports one. (This doc block and [`rmc_at_height`]'s were
/// attached to the wrong functions until 2026-10-01, gh:#428.)
fn paper_height(bed_height_cm: f64) -> f64 {
    let pitch = nee_soon::htr10_rmc::table1::LAYER_HEIGHT_CM / 2.0;
    bed_height_cm - pitch + nee_soon::htr10_rmc::table1::BALL_DIAMETER_CM
}

/// RMC's `k_eff` interpolated to an arbitrary fuel-loading height \[cm\], from
/// the paper's own twelve-point curve.
///
/// # Why this exists
///
/// ~~The example compared every result against the single 123.576 cm point while
/// building a bed of `n_axial x 4.899` cm. At the default 25 layers that
/// bed is **122.474 cm**, and RMC's curve interpolates there to **1.000676**
/// rather than 1.004288 -- so **+361 pcm of the reported disagreement was the
/// comparison point, not the model**.~~
///
/// **CORRECTED 2026-09-27 (gh:#333) -- that "correction" was itself the
/// error.** Li, Yu & Wei (2014) step the loading by whole prism layers
/// (*"the step size of fuel addition is selected as the height of a layer
/// i.e. 9.798 cm in order to avoid fractional fuel or moderator balls"*) and
/// complete the top layer's balls (*"the top layer is formed by adding half
/// spheres to each ball present in this layer"*). Every tabulated height is
/// `9.798 N + 6.0` cm: `N` prisms carry `2N + 1` ball layers (faces and
/// mid-planes), whose whole-ball extent is `(2N) x 4.899 + 6.0` cm. The paper's
/// height is therefore **bottom of the lowest ball to top of the highest**.
///
/// Measured on the built two-ball bed (2026-09-27; that bed is withdrawn
/// since 2026-10-01, and `layers` now counts Şeker layers N): `n_axial` = 25 gives 25
/// whole fuelled ball layers whose extent is **123.576 cm** -- exactly the
/// paper's critical loading, so the right reference is the tabulated
/// **1.004288**, not an interpolation at the volume-equivalent 122.474 cm. In
/// general `n_axial` maps to `4.899 (n_axial - 1) + 6.0` cm ([`paper_height`]).
/// (The bed also holds a 0.55 cm cap of a 26th layer under the bed-top plane:
/// shell graphite plus ~0.05 cm of fuel zone. Not in the paper's model; noted,
/// not priced.) The curve rises ~270 pcm/cm here, so the mapping mattered:
/// -361 pcm at n = 25, -477 at n = 20, -165 at n = 41.
///
/// Returns `None` outside the tabulated range \[94.182, 201.960\] cm rather
/// than extrapolating: past the ends the curve flattens and a linear
/// extension would invent reactivity.
///
/// **Since 2026-10-01 (gh:#472)** `main` calls it at the height where Şeker's
/// model holds as many balls as the built bed
/// (`htr10_rmc::seker_height_for_balls`), not at the built height.
///
/// Since 2026-10-02 (gh:#501) a call to the shared
/// [`keff_curve_at_height`](nee_soon::htr10_rmc::keff_curve_at_height).
fn rmc_at_height(h_cm: f64) -> Option<f64> {
    nee_soon::htr10_rmc::keff_curve_at_height(nee_soon::htr10_rmc::RMC_KEFF_VS_HEIGHT, h_cm)
}
fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// Ablation knobs over the NUCLEAR DATA rather than the geometry, read into
/// the shared [`Htr10DataConfig`].
///
/// **Since 2026-10-01 the nuclide set is built by
/// `nee_soon::htr10_rmc::data`**, which this example and
/// `htr10_endf8_height_sweep`, `htr10_rod_metal_full` and
/// `htr10_rod_metal_simplified` all call. ~~`fn nuclides` and `fn
/// rod_metal_plan` here built it inline~~ (moved verbatim in logic; the
/// comments that explained each tape now live in that module). With no knob
/// set, the config is [`Htr10DataConfig::default`]: ENDF/B-VIII.0, natural
/// carbon (C-12 / C-13, gh:#425), helium coolant (gh:#426), real Ni and Fe in
/// the rod steel (gh:#329; **refused until gh:#339 is fixed**, so set
/// `OUTRAM_HTR10_NI_AS_FE=1 OUTRAM_HTR10_FE57_AS_FE56=1` for the simplified
/// case meanwhile), every bound thermal law.
///
/// ~~No VII.0 tape is available locally, so the library term cannot be
/// reproduced exactly; what CAN be done is to bound library sensitivity on the
/// nuclide that carries most of it.~~ **CORRECTED 2026-10-01 (gh:#428):** the
/// VII.0 tapes were downloaded on 2026-09-18 and `OUTRAM_HTR10_ENDF7=1` runs
/// the whole nuclide set from them. The JENDL knob below predates that and
/// remains a different-library bound.
///
/// - `OUTRAM_HTR10_ENDF7=1` runs every nuclide from ENDF/B-VII.0, the library
///   Li, Yu & Wei (2014) state for RMC. VII.0 carbon is elemental C-nat, which
///   is natural carbon (so ~~the VIII.0 arm does not carry the C-13 and the
///   difference is "not separable without a third arm"~~ **CORRECTED
///   2026-10-01, gh:#425:** both arms now carry natural carbon). No SiC law;
///   helium and the rod metals from VIII.0 (stated in the diagnostics).
/// - `OUTRAM_HTR10_GRAPHITE_TSL=crystalline|10P|30P` (VIII.0 only; default 30P).
/// - `OUTRAM_HTR10_U238_JENDL=1` swaps U-238 to the JENDL-3.3 evaluation.
///   **This is not the VII.0 offset** and must never be quoted as one.
/// - `OUTRAM_HTR10_NO_SAB=1` drops every S(alpha,beta) and leaves every
///   nuclide a free gas. Primarily a HARNESS check: in a graphite-moderated
///   system this must be worth a large, resolved amount.
/// - `OUTRAM_HTR10_CARBON_AS_C12=1` (VIII.0 only, new 2026-10-01): all carbon
///   as C-12, the pre-2026-10-01 model, to price the C-13 term.
/// - `OUTRAM_HTR10_VACUUM_COOLANT=1` (new 2026-10-01): the coolant regions
///   are exact vacuum, the pre-2026-10-01 model; Li reports both.
/// - `OUTRAM_HTR10_NI_AS_FE=1`, `OUTRAM_HTR10_FE57_AS_FE56=1` (both = the
///   SIMPLIFIED rod metal), `OUTRAM_HTR10_NO_WITHDRAWN_RODS=1`.
/// - `OUTRAM_HTR10_UO2_TAPE_DIR=<dir>` reads the UO2 laws from tabulated tapes.
fn data_config() -> Htr10DataConfig {
    let on = |k: &str| std::env::var(k).is_ok();
    let graphite_choice = std::env::var("OUTRAM_HTR10_GRAPHITE_TSL").ok();
    let graphite_law = match graphite_choice.as_deref() {
        None => GraphiteLaw::default(),
        Some(v) => GraphiteLaw::from_name(v).unwrap_or_else(|| {
            panic!("OUTRAM_HTR10_GRAPHITE_TSL must be crystalline, 10P or 30P, got {v}")
        }),
    };
    Htr10DataConfig {
        library: if on("OUTRAM_HTR10_ENDF7") {
            NuclearDataLibrary::EndfB7
        } else {
            NuclearDataLibrary::EndfB8
        },
        graphite_law,
        carbon: if on("OUTRAM_HTR10_CARBON_AS_C12") {
            CarbonTreatment::AllC12
        } else {
            CarbonTreatment::Natural
        },
        coolant: if on("OUTRAM_HTR10_VACUUM_COOLANT") {
            Coolant::Vacuum
        } else {
            Coolant::Helium
        },
        rod_metal: RodMetalTreatment::from_knobs(
            on("OUTRAM_HTR10_NI_AS_FE"),
            on("OUTRAM_HTR10_FE57_AS_FE56"),
            on("OUTRAM_HTR10_NO_WITHDRAWN_RODS"),
        ),
        thermal: if on("OUTRAM_HTR10_NO_SAB") {
            ThermalScatteringTreatment::FreeGas
        } else {
            ThermalScatteringTreatment::Bound
        },
        u238: if on("OUTRAM_HTR10_U238_JENDL") {
            U238Evaluation::Jendl33
        } else {
            U238Evaluation::Library
        },
        uo2_laws: match std::env::var("OUTRAM_HTR10_UO2_TAPE_DIR") {
            Ok(dir) => Uo2Laws::Tapes(dir.into()),
            Err(_) => Uo2Laws::GeneratedFromLeapr,
        },
        temperature: ThermodynamicTemperature::new::<kelvin>(TEMP_K),
        tapes: TapeSource::Workspace,
    }
}

fn main() {
    let histories = env_usize("OUTRAM_HTR10_HISTORIES", 2000);
    let rings = env_usize("OUTRAM_HTR10_RINGS", 8);
    // Şeker layers N since 2026-10-01 (gh:#472): the bed is 9.798 N + 6 cm, a
    // row of Li's table for N = 9..20. ~~Half-layers of 4.899 cm~~ (the
    // two-ball / one-ball `2 N + 1`). The default 12 is the critical row.
    let layers = env_usize("OUTRAM_HTR10_LAYERS", 12);

    println!("HTR-10 core k-eff vs Li, Yu & Wei (2014), RMC {RMC_KEFF}");
    println!("=========================================================");
    // The library actually in use, not a hardcoded one. Until 2026-09-24 this
    // line printed "ENDF/B-VIII.0" unconditionally, so an `OUTRAM_HTR10_ENDF7=1`
    // run announced itself as VIII.0 at the top of its own transcript -- the
    // one place a reader looks to find out which arm a saved log came from.
    if std::env::var("OUTRAM_HTR10_ENDF7").is_ok() {
        // CORRECTED 2026-10-01 (gh:#428): "the library RMC, MCNP, Serpent and
        // HCP used" -- Li's MCNP columns are Şeker's ENDF/B-VI runs.
        println!("  ENDF/B-VII.0 (the library Li, Yu & Wei state for RMC)");
    } else {
        println!("  ENDF/B-VIII.0 (RMC used VII.0 -- offset NOT corrected)");
    }
    println!("  explicit TRISO, hybrid delta/surface tracking, TECDOC reflector\n");

    eprintln!("Reconstructing cross sections:");
    // The run's own diagnostic record. Nuclear-data processing and transport
    // are timed SEPARATELY: they scale with completely different things --
    // data prep with the nuclide count and the thermal laws asked for,
    // transport with histories x cycles -- and one combined number makes a
    // run impossible to reason about.
    let mut diag = RunDiagnostics::new("htr10-rmc-keff");
    diag.note(format!(
        "{histories} histories x [{} inactive + {} active], {rings} rings x {layers} layers",
        env_usize("OUTRAM_HTR10_INACTIVE", 30),
        env_usize("OUTRAM_HTR10_ACTIVE", 70)
    ));
    let data_cfg = data_config();
    let layout = match Htr10NuclideLayout::plan(&data_cfg) {
        Ok(l) => l,
        Err(e) => {
            println!("REFUSED: {e}");
            return;
        }
    };
    println!("  rod metal: {}", data_cfg.rod_metal.label());
    let nucs = match load_htr10_nuclides(&data_cfg, &layout, &mut diag) {
        Ok(v) => v,
        Err(e @ Htr10DataError::BlockedByGh339) => {
            println!("REFUSED: {e}");
            return;
        }
        Err(e) => {
            println!("SKIP: {e}");
            return;
        }
    };

    // Pebble materials, slots 0..5 in DhUniverse::pebble order.
    // OUTRAM_HTR10_NOBORON=1 drops the Table 2 boron impurity rows. Not
    // physical -- real nuclear graphite carries it -- but it bounds how much of
    // the k deficit the boron treatment could possibly account for.
    let boron = if std::env::var("OUTRAM_HTR10_NOBORON").is_ok() {
        BoronReading::None
    } else {
        BoronReading::Natural
    };
    // The material set now lives in `htr10_rmc::materials` so that this
    // example and `htr10_geometry_export` cannot drift apart -- the exporter
    // writes the manuscript's material table and must describe the model this
    // eigenvalue is actually computed with. The ENVIRONMENT KNOBS stay here;
    // the module takes an explicit config.
    let zone_id = std::env::var("OUTRAM_HTR10_REFL_ZONE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(22usize);
    let refl_scale: f64 = std::env::var("OUTRAM_HTR10_REFL_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let z_report = zone_composition(zone_id).expect("zone is listed");
    println!(
        "  reflector zone: {zone_id} (C {:.4e} x{refl_scale:.3}, natural B {:.4e})",
        z_report.carbon, z_report.natural_boron
    );
    let mut mats = nee_soon::htr10_rmc::materials::htr10_material_set(
        &layout,
        nee_soon::htr10_rmc::materials::Htr10MaterialConfig {
            temperature_k: TEMP_K,
            boron,
            reflector_zone: zone_id,
            reflector_carbon_scale: refl_scale,
        },
    );

    // OUTRAM_HTR10_SURFACE=1 runs the SAME geometry with surface tracking only.
    let surface_only = std::env::var("OUTRAM_HTR10_SURFACE").is_ok();
    // ~~OUTRAM_HTR10_HOMOG=1 drops the nested TRISO lattice for a homogenised
    // fuel zone (the one-ball `core_model::assemble`).~~ WITHDRAWN 2026-10-01
    // (maintainer): that model cuts pebbles at its tile faces, which is wrong
    // physics, and it is never to be run, not even as a diagnostic.
    assert!(
        std::env::var("OUTRAM_HTR10_HOMOG").is_err(),
        "OUTRAM_HTR10_HOMOG is withdrawn: the one-ball model cuts pebbles (wrong physics)"
    );
    let maj_idx = if surface_only { usize::MAX } else { 0 };
    let core = assemble_explicit_triso(rings, layers, maj_idx);
    // The no-withdrawn-rods modelling assumption (`RodMetalTreatment::
    // NotModelled`): the rod metals were not loaded (they are the last slots),
    // so drop their components, and PROVE that no cell is filled with a
    // material that lost them.
    if !layout.rod_metal.loads_rod_metal() {
        let mut stripped = Vec::new();
        for (i, m) in mats.iter_mut().enumerate() {
            let before = m.components.len();
            m.components.retain(|c| c.nuclide_idx < nucs.len());
            if m.components.len() != before {
                stripped.push(i);
            }
        }
        for c in &core.geometry.cells {
            if let outram_mc_libs::geometry::cell::CellFill::Material(m) = c.fill {
                assert!(
                    !stripped.contains(&m),
                    "cell {} is filled with material {m} ({}) whose rod-metal nuclides \
                     were not loaded -- the no-rods assumption does not hold",
                    c.id,
                    mats[m].name
                );
            }
        }
        println!(
            "  ASSUMPTION: no withdrawn rods; materials {stripped:?} stripped, used by no cell"
        );
    }
    // Every component must name a loaded nuclide (guards the Ni-as-Fe slot
    // remap and the no-rods strip alike).
    for m in &mats {
        for c in &m.components {
            assert!(
                c.nuclide_idx < nucs.len(),
                "material {} names nuclide slot {} but only {} are loaded",
                m.name,
                c.nuclide_idx,
                nucs.len()
            );
        }
    }
    println!("  fuel zone: explicit TRISO lattice");
    println!(
        "  tracking: {}",
        if surface_only {
            "SURFACE ONLY"
        } else {
            "hybrid (delta bed)"
        }
    );
    println!(
        "  geometry: {} tiles, {} cells, {} universes",
        core.tiles, core.cells, core.universes
    );

    // Region-local majorant: the BED's materials only. The reflector is
    // surface-tracked, so it must NOT raise the bed's tracking cost.
    //
    // `OUTRAM_HTR10_MAJORANT_WITHOUT_BREAKPOINTS=1` is the ABLATION back to
    // the pre-#589 construction (log grid only; under-bounds Sigma_t 14x at
    // 661 eV in the UO2 kernel). It exists as a control: run on today's code,
    // it separates the majorant's effect on k from every other code change
    // since an old record was taken. Never the default.
    let maj = if std::env::var_os("OUTRAM_HTR10_MAJORANT_WITHOUT_BREAKPOINTS").is_some() {
        println!("  majorant: ABLATION, log grid only (pre-#589, under-bound)");
        let bed_mats: Vec<usize> = (0..=nee_soon::htr10_rmc::core_model::mat::HELIUM).collect();
        outram_mc_libs::pebble_beds::delta_tracking::Majorant::over_indices_without_breakpoints(
            &mats,
            &bed_mats,
            &nucs,
            &nee_soon::htr10_rmc::keff_vs_height::majorant_energy_grid(),
            0.3,
        )
    } else {
        nee_soon::htr10_rmc::keff_vs_height::bed_majorant(&mats, &nucs)
    };

    // `OUTRAM_MAJORANT_AUDIT=1`: audit the bed majorant this run would use on
    // the bed's own materials, then stop (GitHub #589).
    if std::env::var_os("OUTRAM_MAJORANT_AUDIT").is_some() {
        let bed = &mats[..=nee_soon::htr10_rmc::core_model::mat::HELIUM];
        let a = maj.audit(bed, &nucs, 1.0e-5, 2.0e7, 2_000_000);
        println!(
            "MAJORANT-AUDIT htr10_rmc_keff bed_majorant | worst {:.4} at {:.5e} eV in '{}' | \
             {} nodes | {} energies",
            a.worst_ratio,
            a.energy_ev,
            bed[a.material].name,
            maj.len(),
            a.energies_checked
        );
        return;
    }

    let settings = KeffSettings {
        n_particles: histories,
        // Tunable so source convergence can be MEASURED rather than assumed.
        // A loosely-coupled 1.8 m pebble core is exactly where a thin inactive
        // stage biases k, and the reference paper's own 5 inactive cycles are
        // not a model to copy.
        n_inactive: env_usize("OUTRAM_HTR10_INACTIVE", 30),
        n_active: env_usize("OUTRAM_HTR10_ACTIVE", 70),
        temperature_k: TEMP_K,
        // `OUTRAM_HTR10_SEED=n` replaces the default seed, so a multi-seed
        // study can run one seed per process (resumable, one log per seed).
        seed: std::env::var("OUTRAM_HTR10_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20260917),
        // `OUTRAM_HTR10_THREADS=n` pins the thread count; default stays Auto.
        //
        // This exists because thread-count independence is **tested for
        // `run_keff` and merely assumed for `run_keff_csg_hybrid`**, which is
        // the driver this case actually uses.
        // `physics::keff::tests::cpu_multi_is_reproducible` asserts bit-identity
        // between 1 and 4 threads -- on the simple sphere path, through
        // `run_keff`. Nothing covers the hybrid CSG path, and with `Auto` the
        // thread count follows machine load, so two runs of this example on a
        // busy box need not use the same count. If the hybrid driver is
        // order-dependent, every single-seed number in the HTR-10 V&V record is
        // irreproducible and the ablation chain built from their differences is
        // unsound. Pinning the count is what makes that testable.
        compute: ComputeType::CpuMultiThread(
            match std::env::var("OUTRAM_HTR10_THREADS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
            {
                Some(n) => ThreadCount::Fixed(n),
                None => ThreadCount::Auto,
            },
        ),
        ..KeffSettings::default()
    };
    let r = core.tiles as f64;
    let _ = r;
    // The source box and entropy mesh must span the WHOLE fissile region.
    //
    // Both were [-50,50]^3 / [-60,60]^3, fixed numbers that predate the conus
    // and the corrected bed extent. The bed now runs from `conus_floor`
    // (-98.2 cm at 25 layers) to `+bed_half_height`, so the old box missed the
    // entire conus and the top of the bed. A starting source that misses fuel
    // is recoverable given enough inactive generations; an entropy mesh that
    // is blind to part of the core is NOT -- it reports convergence of the
    // region it can see, which is exactly the diagnostic one must not trust.
    // Both are `nee_soon::htr10_rmc::keff_vs_height`'s since 2026-10-02
    // (gh:#501), shared with every HTR-10 example.
    let src = nee_soon::htr10_rmc::keff_vs_height::fissile_source_box(&core);
    let entropy_mesh = nee_soon::htr10_rmc::keff_vs_height::fissile_entropy_mesh(&core);

    println!(
        "  {histories} histories x [{} inactive + {} active], seed {}\n",
        settings.n_inactive, settings.n_active, settings.seed
    );
    println!(
        "  nuclear data processed in {:.1} s ({} items)",
        diag.data_seconds(),
        diag.data_item_count()
    );
    let t = Instant::now();
    let res = diag.time_phase("transport (k-eigenvalue)", || {
        run_keff_csg_hybrid(
            &core.geometry,
            &mats,
            &nucs,
            if surface_only {
                &[]
            } else {
                std::slice::from_ref(&maj)
            },
            Some(&entropy_mesh),
            src,
            &settings,
            None,
        )
    });
    let secs = t.elapsed().as_secs_f64();

    // SEED ENSEMBLE (OUTRAM_BENCH_SEEDS, default 1 -- single-seed behaviour and
    // runtime unchanged unless asked for).
    //
    // This case scatters seed-to-seed by far more than most of the effects
    // being argued about, so a single pair CANNOT resolve anything much below
    // ~300 pcm. Anything smaller must be quoted as a pooled mean with its sem,
    // or not quoted at all. `run_keff_csg_hybrid` is already internally
    // multi-threaded, so seeds run sequentially and each uses every core.
    let n_seeds = outram_mc_libs::vv::bench_seeds();
    if n_seeds > 1 {
        let mut ens: Vec<f64> = vec![(res.k_mean - RMC_KEFF) * 1.0e5];
        for seed in 2..=n_seeds as u64 {
            let sset = KeffSettings {
                seed: settings.seed + seed,
                ..settings.clone()
            };
            let r2 = run_keff_csg_hybrid(
                &core.geometry,
                &mats,
                &nucs,
                if surface_only {
                    &[]
                } else {
                    std::slice::from_ref(&maj)
                },
                Some(&entropy_mesh),
                src,
                &sset,
                None,
            );
            eprintln!("    seed {seed}: k = {:.6} +/- {:.6}", r2.k_mean, r2.k_std);
            ens.push((r2.k_mean - RMC_KEFF) * 1.0e5);
        }
        let (mean, sd, sem) = outram_mc_libs::vv::pooled(&ens);
        println!("\n  ENSEMBLE HTR-10 vs RMC: {n_seeds} seeds");
        println!("    pooled dk    = {mean:+.0} pcm");
        println!("    seed-to-seed sd  = {sd:.0} pcm   (what ONE run scatters by)");
        println!("    uncertainty  sem = +/-{sem:.0} pcm   (on the pooled mean)");
    }

    // Height-matched comparison. The bed is `2 * bed_half_height` = `n_axial x
    // 4.899` cm tall (not `lat_height * n_axial` since the two-ball tile); RMC's
    // curve is sampled at ITS heights, so comparing against a point the model
    // does not occupy imports a systematic worth ~270 pcm per cm of mismatch.
    // CORRECTED 2026-09-27 (gh:#333): matched on the paper's whole-ball
    // extent, not the volume-equivalent height.
    //
    // Since 2026-10-01 (gh:#472) Şeker's bed IS built on the paper's height
    // axis (`9.798 N + 6` cm, every ball whole), so no mapping is applied; ~~the
    // two-ball and one-ball beds keep the gh:#333 mapping~~ (CORRECTED
    // 2026-10-01, gh:#428: both are withdrawn and panic, so `paper_height`
    // below is a fallback the default path never reaches).
    let volume_height_cm = core.bed_half_height * 2.0;
    //
    // ~~Şeker's bed is compared at its built height~~ **CHANGED 2026-10-01
    // (gh:#472, maintainer: "match by ball count"):** our whole-ball bed holds
    // 1.2 % fewer balls than Şeker's model at the same height (Şeker kept balls
    // crossing the wall by up to 0.21 cm). So the reference is read at the
    // height where Şeker's model holds OUR inventory,
    // `htr10_rmc::seker_height_for_balls`.
    let bed_height_cm = match core.bed.as_ref().and_then(|b| b.core_balls()) {
        Some(balls) => {
            let h = nee_soon::htr10_rmc::seker_height_for_balls(balls);
            println!(
                "\n  BALL-COUNT MATCH (gh:#472): {balls} balls in a {volume_height_cm:.3} cm bed; \
                 Şeker's model holds that many at {h:.3} cm"
            );
            h
        }
        None => paper_height(volume_height_cm),
    };
    let rmc_here = rmc_at_height(bed_height_cm);
    match rmc_here {
        Some(k) => println!(
            // CORRECTED 2026-10-01 (gh:#428): this printed "(volume) = ... ball
            // extent (paper's convention, gh:#333)", which is not what
            // `bed_height_cm` is for Şeker's bed. `htr10_pooled_study` parses
            // the `HEIGHT-MATCHED` and `RMC(interp) =` tokens; keep both.
            "\n  HEIGHT-MATCHED: bed {volume_height_cm:.3} cm built; reference read at \
             {bed_height_cm:.3} cm (equal ball count, gh:#472) -> RMC(interp) = {k:.6}\n  \
             (the {RMC_KEFF:.6} headline is RMC at 123.576 cm; difference {:+.0} pcm \
             is comparison point, NOT model)",
            (RMC_KEFF - k) * 1.0e5
        ),
        None => println!(
            "\n  HEIGHT-MATCHED: bed is {bed_height_cm:.3} cm -- OUTSIDE the tabulated \
             RMC range [94.182, 201.960] cm, so no height-matched reference exists \
             and the {RMC_KEFF:.6} comparison below is NOT like-for-like."
        ),
    }

    // A rough gauge only (maintainer direction 2026-09-27): the paper's MCNP
    // columns, from an independently built model. RMC stays the reference.
    let at = |c: &[(f64, f64)]| nee_soon::htr10_rmc::keff_curve_at_height(c, bed_height_cm);
    if let (Some(m3), Some(m4)) = (
        at(nee_soon::htr10_rmc::MCNP_TABLE3_KEFF_VS_HEIGHT),
        at(nee_soon::htr10_rmc::MCNP_TABLE4_KEFF_VS_HEIGHT),
    ) {
        println!(
            "  GAUGE (not a reference): MCNP at this height, Table 3 {m3:.6} ({:+.0} pcm), \
             Table 4 {m4:.6} ({:+.0} pcm)",
            (res.k_mean - m3) * 1.0e5,
            (res.k_mean - m4) * 1.0e5
        );
    }

    let pcm = (res.k_mean - RMC_KEFF) * 1.0e5;
    if let Some(k) = rmc_here {
        println!(
            "  dk height-matched = {:+.0} pcm   (against {:.6}, not {RMC_KEFF:.6})",
            (res.k_mean - k) * 1.0e5,
            k
        );
    }
    let sigma = res.k_std * 1.0e5;
    println!("  k_eff        = {:.6} +/- {:.6}", res.k_mean, res.k_std);
    println!("  RMC          = {RMC_KEFF:.6}");
    println!("  difference   = {pcm:+.0} pcm   (our sigma {sigma:.0} pcm)");
    println!("  virtual coll = {}", res.virtual_collisions);
    // Histories transported = n_particles x every generation, active or not.
    let n_hist = res.histories.max(1) as f64;
    println!(
        "  histories    = {} (planned {})",
        res.histories,
        settings.n_particles * (settings.n_inactive + settings.n_active)
    );
    println!(
        "  collisions   = {} ({:.2} per history)",
        res.collisions,
        res.collisions as f64 / n_hist
    );
    println!(
        "  lost locate  = {} ({:.3} %)",
        res.lost_locate,
        100.0 * res.lost_locate as f64 / n_hist
    );
    println!(
        "  stuck events = {} ({:.3} %)",
        res.stuck_events,
        100.0 * res.stuck_events as f64 / n_hist
    );
    if res.stuck_events > 0 {
        println!(
            "  stuck path   = {:.4} cm mean, last E = {:.4e} eV",
            res.stuck_path_cm / res.stuck_events as f64,
            res.stuck_last_e
        );
    }
    println!(
        "  neg distance = {} (worst {:.4e} cm, level {})",
        res.neg_dist, res.neg_worst, res.neg_level
    );
    println!(
        "      from lattice = {}, from surface = {}",
        res.neg_from_lattice, res.neg_from_surface
    );
    println!(
        "  leak vacuum  = {} ({:.3} %)",
        res.leak_vacuum,
        100.0 * res.leak_vacuum as f64 / n_hist
    );
    println!(
        "  leak infinity= {} ({:.3} %)",
        res.leak_infinity,
        100.0 * res.leak_infinity as f64 / n_hist
    );
    println!("  wall clock   = {secs:.1} s");
    println!("  generations reported: {}", res.k_by_generation.len());
    let nz = res.k_by_generation.iter().filter(|k| **k > 0.0).count();
    println!("  generations with k > 0: {nz}");
    for (i, k) in res.k_by_generation.iter().take(5).enumerate() {
        println!("    gen {i}: k = {k:.6}");
    }
    // Full entropy trace: a still-rising trace means the source has NOT
    // converged and every active generation before it is biased.
    if !res.entropy.is_empty() {
        let step = (res.entropy.len() / 12).max(1);
        print!("  entropy trace:");
        for (i, h) in res.entropy.iter().enumerate() {
            if i % step == 0 || i + 1 == res.entropy.len() {
                print!(" {h:.3}");
            }
        }
        println!();
    }
    if let (Some(first), Some(last)) = (res.entropy.first(), res.entropy.last()) {
        println!(
            "  entropy      = {first:.4} -> {last:.4} bits (ceiling {:.4})",
            (entropy_mesh.n_bins() as f64).log2()
        );
    }
    // CORRECTED 2026-09-27: this used to say every run was "a REDUCED core,
    // not the 123.576 cm loading". Both halves were false: the bed radius is
    // always the physical 90 cm (`rings` is only a floor, see
    // `core_model::assemble`), and ~~`layers = 25` IS the 123.576 cm loading
    // in the paper's whole-ball-extent convention (gh:#333)~~ CORRECTED
    // 2026-10-01 (gh:#428): `layers` is Şeker layers N since gh:#472, and
    // N = 12 is the 123.576 cm loading. The printout said "{layers} layers =
    // {bed_height_cm} cm", but `bed_height_cm` is the equal-ball-count
    // reference height, not the bed's.
    println!(
        "\n  Gate is 500-1000 pcm. Full-radius bed, {layers} Seker layers = {volume_height_cm:.3} cm \
         built, reference read at {bed_height_cm:.3} cm; VIII.0 runs carry the VIII.0-vs-VII.0 offset."
    );

    // Data-processing time and transport time, reported separately, and the
    // full provenance record written to a file. If any data item failed to
    // load, `print_summary` says so loudly -- a run that silently fell back
    // to free gas must not be read as if it had not.
    diag.note(format!("k_eff = {:.6} +/- {:.6}", res.k_mean, res.k_std));
    diag.print_summary();
    diag.write_and_report();
}
