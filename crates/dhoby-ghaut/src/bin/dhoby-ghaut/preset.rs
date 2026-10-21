//! The HTR-10 pebble-bed preset: Basic mode's starting point.
//!
//! Every number is read from `nee_soon::htr10_rmc` (the model of the HTR-10
//! RMC code-to-code record) or `outram_mc_libs::pebble_beds` where they export
//! it, so the recipe and the model the solver builds cannot drift. ~~Two
//! values they do not export (the 2.5 cm fuel zone and 8335 particles per
//! pebble) are typed here and marked where they are.~~ **CORRECTED
//! 2026-10-05:** both are exported (`explicit_bed::FUEL_ZONE_RADIUS_CM`, and
//! `core_design::PARTICLES_PER_PEBBLE` since gh:#566) and read from there.
//! Each value
//! carries the source that `nee_soon`'s own docs give for it. Page numbers
//! are given only where those docs state one; otherwise the table or section
//! is named in `what`, and `page` is left empty rather than guessed.
//!
//! The element lists (Steps 3 and 4) state what the model does and does not
//! contain, read from `nee_soon::htr10_rmc`'s module docs and
//! `reflector_geometry` on 2026-10-05.

use dhoby_ghaut::workbench::catalogue::Mode;
use dhoby_ghaut::workbench::recipe::{
    Citation, ElementStatus, InsertStep, ModelElement, MonteCarloStep, NuclearDataStep,
    PebbleBedStep, PebbleDesign, PebbleDesignStep, PebbleMix, Recipe, RecipeHeader, ReflectorStep,
    ReviewGate, Triso, FORMAT, VERSION,
};
use nee_soon::htr10_rmc::control_rod::{
    B4C_DENSITY_G_PER_CM3, B4C_INNER_RADIUS_CM, B4C_OUTER_RADIUS_CM, N_CONTROL_RODS,
};
use nee_soon::htr10_rmc::core_model::{
    HTR10_CORE_RADIUS_CM, HTR10_MODEL_HEIGHT_CM, HTR10_REFLECTOR_OUTER_CM,
};
use nee_soon::htr10_rmc::keff_vs_height::{SweepStatistics, RINGS, TEMPERATURE_K};
use nee_soon::htr10_rmc::table1;
use outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec;

/// kovan citekey of Li, Yu & Wei (2014), the RMC benchmark paper.
pub const LI2014: &str = "li2014htr";
/// IAEA-TECDOC-1382 (HTR-10 benchmark), part 2, which holds the reflector
/// zone map; named as the maintainer's kovan library files it.
pub const TECDOC1382: &str = "iaea-tecdoc-1382-part2";
/// Şeker & Çolak (2003), the bed model's ball inventory.
pub const SEKER2003: &str = "seker2003htr10";

/// The V&V status of the preset, quoted from `nee_soon::htr10_rmc`'s module
/// docs (2026-10-05). The pre-built card shows this, never "validated".
pub const VV_STATUS: &str = "TENTATIVE. Code-to-code against RMC (Li, Yu & Wei 2014), not \
    against experiment. Open residual: -2726 pcm on ENDF/B-VIII.0 at the critical loading \
    (explicit reflector, fast single-seed statistics, gh:#333), with a height-dependent drift \
    (gh:#218) still unexplained. AI-drafted model awaiting human review.";

/// Layers of the default bed: N = 12 is the 123.576 cm loading the geometry
/// images and the RMC critical loading use.
pub const DEFAULT_LAYERS: usize = 12;

fn cite(field: &str, citekey: &str, page: Option<u32>, what: &str) -> Citation {
    Citation {
        field: field.into(),
        citekey: citekey.into(),
        page,
        what: what.into(),
    }
}

fn el(name: &str, status: ElementStatus, note: &str) -> ModelElement {
    ModelElement {
        name: name.into(),
        status,
        note: note.into(),
    }
}

/// Worker threads for Monte Carlo: every core but two, at most 14 (the
/// maintainer keeps two cores free).
pub fn default_threads() -> usize {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get());
    n.saturating_sub(2).clamp(1, 14)
}

/// The HTR-10 Basic-mode recipe.
pub fn htr10() -> Recipe {
    let spec = TrisoSpec::HTR10_LI2014;
    let q = SweepStatistics::QUICK;
    let raw = nee_soon::htr10_rmc::data::DataDir::Endf.path();
    let endf_dir = std::fs::canonicalize(&raw).unwrap_or(raw);
    Recipe {
        header: RecipeHeader {
            format: FORMAT.into(),
            version: VERSION,
            title: "HTR-10 pebble bed".into(),
            reactor: "htgr".into(),
            mode: Mode::Basic,
            preset: Some("htr10".into()),
            edited: false,
            vv_status: VV_STATUS.into(),
        },
        nuclear_data: NuclearDataStep {
            endf_dir: endf_dir.display().to_string(),
            library: "ENDF/B-VIII.0".into(),
            temperature_k: TEMPERATURE_K,
            cites: vec![cite(
                "temperature_k",
                LI2014,
                None,
                "the benchmark is at 27 °C (300.15 K) throughout; Şeker & Çolak (2003) state the same",
            )],
        },
        pebble_bed: PebbleBedStep {
            packing: "Şeker 13-ball hexagonal prism cell; whole balls, rejected at the wall, cone and tube".into(),
            rings: RINGS,
            layers: DEFAULT_LAYERS,
            filling_fraction: table1::BALL_FILLING_FRACTION,
            source: dhoby_ghaut::workbench::recipe::BedSource::Lattice,
            dem: Some(dhoby_ghaut::workbench::recipe::DemPour {
                // The full core the published 0.61 is quoted for (Table 1).
                n_pebbles: table1::FUEL_ELEMENTS as usize,
                friction: 0.1,
                rolling_friction: 0.0,
                youngs_modulus_pa: 5.0e8,
                seed: 0x5EED_0010,
                settled_steps: None,
                phi_whole_core: None,
            }),
            mix: PebbleMix {
                fuel: table1::FUEL_BALL_FRACTION,
                moderator: table1::MODERATOR_BALL_FRACTION,
                fertile: 0.0,
                poison: 0.0,
            },
            cites: vec![
                cite("mix", LI2014, None, "Table 1: fuel-to-moderator ball ratio 0.57/0.43"),
                cite("filling_fraction", LI2014, None, "body text: ball filling fraction 0.61 in the core"),
                cite("packing", SEKER2003, None, "Table 3: 1346 N + 733 balls for N layers; the bed is built to this inventory"),
            ],
        },
        pebble_design: PebbleDesignStep {
            interstitial: "helium".into(),
            pebbles: vec![
                PebbleDesign {
                    kind: "fuel".into(),
                    outer_radius_cm: 0.5 * table1::BALL_DIAMETER_CM,
                    // ~~Not exported as constants by the model~~ CORRECTED
                    // 2026-10-05: the builder's own constants (Li 2014
                    // Table 2), so the recipe cannot drift from the model.
                    fuel_zone_radius_cm: Some(nee_soon::htr10_rmc::explicit_bed::FUEL_ZONE_RADIUS_CM),
                    matrix: "graphite".into(),
                    triso: Some(Triso {
                        kernel: "UO2".into(),
                        enrichment: outram_mc_libs::pebble_beds::htr10::ENRICHMENT_WT,
                        kernel_radius_cm: spec.kernel,
                        layers: vec![
                            ("buffer PyC".into(), spec.buffer),
                            ("IPyC".into(), spec.ipyc),
                            ("SiC".into(), spec.sic),
                            ("OPyC".into(), spec.opyc),
                        ],
                        particles_per_pebble: nee_soon::htr10_rmc::core_design::PARTICLES_PER_PEBBLE,
                    }),
                },
                PebbleDesign {
                    kind: "moderator".into(),
                    outer_radius_cm: 0.5 * table1::BALL_DIAMETER_CM,
                    fuel_zone_radius_cm: None,
                    matrix: "graphite".into(),
                    triso: None,
                },
            ],
            cites: vec![
                cite("pebble.outer_radius_cm", LI2014, None, "Table 2: 6 cm ball diameter, fuel and moderator"),
                cite("pebble.fuel.triso", LI2014, None, "Table 2: 8335 particles per ball, 250 µm kernel, UO2 at 10.4 g/cm³, 17 wt% U-235"),
                cite("pebble.fuel.fuel_zone_radius_cm", LI2014, None, "Table 2: 2.5 cm fuelled zone"),
            ],
        },
        reflector: ReflectorStep {
            core_radius_cm: HTR10_CORE_RADIUS_CM,
            reflector_outer_cm: HTR10_REFLECTOR_OUTER_CM,
            model_height_cm: HTR10_MODEL_HEIGHT_CM,
            elements: vec![
                el("graphite reflector zones (TECDOC Fig. 4.10 zone map)", ElementStatus::InModel, "each zone with its own Table 4-3 composition"),
                el("boronated carbon bricks", ElementStatus::InModel, "own material (mat::BORONATED)"),
                el("control-rod channels (10)", ElementStatus::InModel, "explicit borings; 18° azimuth convention (gh:#330)"),
                el("small-absorber-sphere (KLAK) channels (7)", ElementStatus::InModel, "explicit borings, including the slot section"),
                el("irradiation channels (3)", ElementStatus::InModel, "explicit borings"),
                el("cold gas risers (20 coolant channels)", ElementStatus::InModel, "explicit borings in the side reflector"),
                el("hot gas duct", ElementStatus::InModel, "explicit"),
                el("defuelling chute (discharge tube)", ElementStatus::InModel, "explicit whole graphite balls, Li (2014)'s rejection rule"),
                el("cold helium chamber (above the core)", ElementStatus::Simplified, "TECDOC zone 3 composition; not an explicit plenum"),
                el("hot gas plenum (below the core)", ElementStatus::Simplified, "represented by its TECDOC zone composition; check against the zone map"),
                el("refuelling chute", ElementStatus::NotInModel, "not in the benchmark model the record follows; gh:#570"),
            ],
            cites: vec![
                cite("elements", TECDOC1382, None, "Fig. 4.10 zone map and Table 4-3 zone compositions; p. 242 (printed) for the corrections once borings are explicit"),
                cite("core_radius_cm", LI2014, None, "Table 1: core diameter 180 cm"),
            ],
        },
        inserts: InsertStep {
            control_rods: N_CONTROL_RODS,
            b4c_inner_radius_cm: B4C_INNER_RADIUS_CM,
            b4c_outer_radius_cm: B4C_OUTER_RADIUS_CM,
            b4c_density_g_per_cm3: B4C_DENSITY_G_PER_CM3,
            elements: vec![
                el("control rods (B4C, steel sleeves, iron joints)", ElementStatus::InModel, "explicit; withdrawn (the benchmark's state) unless Step 5 inserts them, all ten together (gh:#580)"),
                el("small absorber spheres", ElementStatus::NotInModel, "KLAK channels are empty (maintainer, gh:#330); gh:#570"),
                el("pebbles in the irradiation channels", ElementStatus::NotInModel, "irradiation channels are empty (gh:#330); gh:#570"),
                el("pressure vessel and core barrel steel", ElementStatus::NotInModel, "the model ends at the reflector's outer radius, 190 cm"),
            ],
            cites: vec![cite("control_rods", TECDOC1382, None, "control-rod geometry and B4C composition")],
        },
        review: ReviewGate::default(),
        monte_carlo: MonteCarloStep {
            particles: q.particles,
            inactive: q.inactive,
            active: q.active,
            seed: q.seed,
            threads: default_threads(),
            rod_insertion: 0.0,
            spectrum_bins_per_decade: 10,
            ablations: Vec::new(),
            runs: Vec::new(),
            cites: vec![cite(
                "particles",
                LI2014,
                None,
                "the paper ran 10 000 × [5 + 135]; the prefill is the workspace's QUICK record statistics, 2000 × [30 + 70]",
            )],
        },
        // Step 6: multiphysics by default; the map's planner spans 300-1200 K
        // (a workbench default, not a value from a source).
        branch: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The preset round-trips through the recipe format, and claims nothing
    /// it should not: no "validated", and every NOT-in-model element names
    /// an issue.
    #[test]
    fn the_htr10_preset_round_trips_and_states_its_gaps() {
        let r = htr10();
        let md = r.to_markdown("2026-10-05T00:00:00Z").expect("write");
        assert_eq!(Recipe::from_markdown(&md).expect("read"), r);
        assert!(r.header.vv_status.starts_with("TENTATIVE"));
        assert!(!r.header.vv_status.to_lowercase().contains("validated"));
        for e in r.reflector.elements.iter().chain(&r.inserts.elements) {
            if e.status == ElementStatus::NotInModel && !e.name.contains("vessel") {
                assert!(e.note.contains("gh:#"), "{} has no issue", e.name);
            }
        }
        assert!((r.pebble_bed.mix.total() - 1.0).abs() < 1e-12);
    }
}
