//! Step 9's HTR-10 prefill: the multiphysics case at 100 % power.
//!
//! Geometry and power are read from `nee_soon::htr10_rmc::table1` (Li, Yu &
//! Wei 2014 Table 1), and the system pressure and inlet temperature from
//! `tampines::gas_phase::properties::htr10_design_point` (IAEA HTR-10
//! benchmark), so the case cannot drift from the models that export them.
//! Values neither exports are typed here and marked where they are; their
//! source is Gao & Shi (2002), "Thermal hydraulic calculation of the HTR-10
//! for the initial and equilibrium core", Nucl. Eng. Des. 218, 51–64
//! (`gao2002htr10th`, proprietary, in the maintainer's private library), as
//! transcribed with provenance in `docs/reactor-scoping/htr10-plant-data.md`
//! §7.4–7.6 (read 2026-10-05). No page is given where the transcription
//! gives a table or section instead.

use dhoby_ghaut::workbench::multiphysics::{
    CouplingSettings, FeedbackWeighting, FoamSide, MultiphysicsSetup, NeutronicsSide, PowerShape,
    StructuralSide, WallCondition,
};
use dhoby_ghaut::workbench::recipe::{Citation, ElementStatus, ModelElement};
use nee_soon::htr10_rmc::table1;
use tampines::gas_phase::properties::htr10_design_point;
use uom::si::pressure::megapascal;
use uom::si::thermodynamic_temperature::degree_celsius;

use crate::preset::{LI2014, TECDOC1382};

/// kovan citekey of Gao & Shi (2002), the HTR-10 thermal-hydraulic design
/// calculation (NED 218, 51–64).
pub const GAO2002: &str = "gao2002htr10th";
/// Chen et al. (2009): the isothermal temperature coefficient used by the
/// THERMIX post-test analysis, Table 1. **Not in the kovan corpus**; the
/// value is as transcribed in `outram-park-digital-twin-engine`'s
/// `htgr_sim_v1/physics/kinetics.rs` (`HTR10_TEMPERATURE_COEFFICIENT_PER_K`).
pub const CHEN2009: &str = "chen2009htr10";

/// Gao & Shi (2002) Table 2, 100 % load: total helium flow \[kg/s\].
/// Typed here: tampines' design point carries the benchmark's rounded 4.3.
pub const TOTAL_FLOW_KG_S: f64 = 4.32;
/// Gao & Shi (2002) Table 1: flow through the pebble bed and bottom
/// reflector \[kg/s\] (87.3 % of 4.32).
pub const CORE_FLOW_KG_S: f64 = 3.77;
/// Gao & Shi (2002) §4.2: equilibrium-core maximum power density \[W/cm³\].
pub const MAX_POWER_DENSITY_W_CM3: f64 = 2.57;
/// Gao & Shi (2002) Table 2: average power density at 100 % \[MW/m³\].
pub const MEAN_POWER_DENSITY_MW_M3: f64 = 2.0;
/// Chen et al. (2009) Table 1: isothermal coefficient \[1/K\].
pub const ISOTHERMAL_COEFFICIENT_PER_K: f64 = -1.4e-4;

/// Gao & Shi (2002) Table 2 at 100 % (equilibrium core), the published
/// values Step 10 compares with: (what, value °C or kPa, note).
pub const PUBLISHED: [(&str, f64, &str); 5] = [
    (
        "Reactor outlet helium [°C]",
        700.0,
        "Table 2 (design value; mixed outlet)",
    ),
    ("Max coolant temperature [°C]", 818.0, "Table 2"),
    (
        "Max fuel temperature [°C]",
        918.7,
        "Table 2; includes the §4.1 uncertainty factors",
    ),
    (
        "Max fuel surface temperature [°C]",
        876.7,
        "Table 2; includes the §4.1 uncertainty factors",
    ),
    (
        "Core pressure drop [kPa]",
        1.3,
        "Table 1/2: pebble bed AND bottom reflector",
    ),
];

/// Gao & Shi (2002) for the **initial** core (13 500 fuel elements, graphite
/// balls half the bed; §4.2, §4.3, §5), as transcribed in
/// `docs/reactor-scoping/htr10-plant-data.md` §7.6: (what, value, note). The
/// solved Step 10 run on the preset's initial-core recipe compares with
/// these, the equilibrium-core [`PUBLISHED`] being a different core.
pub const PUBLISHED_INITIAL: [(&str, f64, &str); 4] = [
    (
        "Max power density [W/cm³]",
        2.84,
        "§4.2: at R = 0, Z = 90 cm (equilibrium core 2.57)",
    ),
    (
        "Mean fuel temperature [°C]",
        605.7,
        "§4.3 average",
    ),
    (
        "Max fuel centre [°C]",
        995.0,
        "§4.3 'about 995'; §5 says 1049 (unresolved, see the transcription)",
    ),
    (
        "Reactor outlet helium [°C]",
        700.0,
        "Table 2 design value",
    ),
];

fn cite(field: &str, citekey: &str, what: &str) -> Citation {
    Citation {
        field: field.into(),
        citekey: citekey.into(),
        page: None,
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

/// What Step 10's run models, simplifies and leaves out, with the issue
/// that tracks each gap.
pub fn elements() -> Vec<ModelElement> {
    use ElementStatus::{InModel, NotInModel, Simplified};
    vec![
        el("Helium energy balance", InModel, "Exact enthalpy march per ring with the coolprop-port helium EOS ((p, h) flash)."),
        el("Pebble-to-helium heat transfer", InModel, "Wakao-Funazkri (1978) on local Re, Pr and k (tampines::pebble_bed::cht)."),
        el("Pebble and TRISO conduction", InModel, "Two-zone pebble + hottest TRISO at the pebble centre (tampines::pebble_bed::Pebble::htr10), fluence from Step 9."),
        el("Bed friction and flow split", InModel, "KTA 3102.3 per node; rings share one plenum-to-plenum pressure drop (the coupling loop)."),
        el("Thermal-hydraulic mesh", Simplified, "Equal-area rings x axial nodes (r-z multi-channel). Step 7's tet-dual TH mesh carries power and temperature between the neutronics mesh and the rings but is not solved on (gh:#592)."),
        el("OUTRAM-Foam porous solver", NotInModel, "outram-foam-appbuilder-lib's OnePhaseSolver has constant properties and one-cell tests only; not used (gh:#592)."),
        el("Radial conduction and radiation between rings", NotInModel, "ZBS effective conductivity (tampines::pebble_bed::zbs) is not applied across rings (gh:#592)."),
        el("Side wall", Simplified, "Adiabatic: no heat to the side reflector or the RCCS (gh:#592)."),
        el("Reflector, plenums, bypass channels", Simplified, "Not modelled thermally; the bypass (total minus core flow) is mixed at the inlet temperature."),
        el("Power shape and k", InModel, "Solved (gh:#591): GeN-Foam port multigroup diffusion k-eigenvalue on Step 7's neutronics mesh, Step 8's constants at each cell's TFuel (ln T, extrapolated linearly outside the state points as upstream), Marshak vacuum boundary, Picard-coupled to the march through Step 7's maps with relaxed power. Ablation: --prescribed-power (J0 x cosine with lumped feedback)."),
        el("Neutronics data", Simplified, "2 groups by default, P0 scattering, D = 1/(3 Sigma_t), no discontinuity factors, no delayed neutrons (steady state only), Monte Carlo statistics of a short run (gh:#595). KNOWN DEFECT: Step 8's pebble-bed constants are about 1/0.61 too large (its collision-estimator flux misses the helium voids of the delta-tracked bed, gh:#598); diffusion k is then ~+25 000 pcm above Step 8's Monte Carlo and the shape too peaked (V&V record)."),
        el("Neutronics regions", Simplified, "Cells take their region by centroid on the 30 cm neutronics mesh: region boundaries are stair-stepped, borings not explicit (gh:#594). Fission power landing outside the TH bed is reported and the bed power rescaled to the thermal power."),
        el("Temperature feedback", Simplified, "One temperature per cell drives every material's constants: the fuel-pebble volume average in the bed (state points are isothermal, gh:#595); no separate moderator / coolant-density feedback."),
        el("Reflector, conus, tube temperatures (for the cross sections)", Simplified, "Held at the inlet helium temperature: no reflector heat balance (gh:#592)."),
        el("Neutronics -> ring grid transfer", Simplified, "Power: Step 7's volume-weighted map to the TH mesh, then each bed cell's power shared over the ring nodes its nearest-cell samples fall in (conservative). Temperature: TH cell takes its centroid's node, then Step 7's map to the neutronics mesh."),
        el("Structural (farrer-park)", NotInModel, "farrer-park is FEM mechanics only (no heat conduction) and has no mapping from the TH mesh yet (gh:#593); not run."),
    ]
}

/// With the solved power shape, the coupled case is **the reactor Steps 1-8
/// built**: the march's bed takes Step 7's bed (radius, and height = the
/// recipe's loading) and the recipe's fuel-pebble share and filling
/// fraction, so the thermal-hydraulics and the neutronics describe one core.
/// The Step 9 prefill describes Gao & Shi's 197 cm all-fuel equilibrium core;
/// the HTR-10 preset recipe is the 57 % fuel initial core loaded to first
/// criticality, so the two differ and every line that moves is returned.
pub fn on_built_core(
    mut s: MultiphysicsSetup,
    d: &dhoby_ghaut::workbench::meshes::RzDomain,
    recipe: &dhoby_ghaut::workbench::recipe::Recipe,
) -> (MultiphysicsSetup, Vec<String>) {
    let mut notes = Vec::new();
    let mut set = |what: &str, v: &mut f64, new: f64, unit: &str| {
        if (*v - new).abs() > 1e-6 * new.abs().max(1.0) {
            notes.push(format!(
                "{what}: {:.4} -> {:.4} {unit} (the reactor Steps 1-8 built)",
                *v, new
            ));
            *v = new;
        }
    };
    let f = &mut s.foam;
    set("bed radius", &mut f.core_radius_cm, d.core_radius, "cm");
    set("bed height", &mut f.core_height_cm, d.bed_top - d.conus_top, "cm");
    let mix = &recipe.pebble_bed.mix;
    if mix.total() > 0.0 {
        set(
            "fuel-pebble share",
            &mut f.fuel_pebble_fraction,
            mix.fuel / mix.total(),
            "",
        );
    }
    set(
        "filling fraction",
        &mut f.filling_fraction,
        recipe.pebble_bed.filling_fraction,
        "",
    );
    (s, notes)
}

/// The prescribed-shape ablation of [`htr10`] (`--prescribed-power`): the
/// J0 x cosine shape fixed by Gao & Shi's peak/mean, with the lumped
/// isothermal-coefficient feedback. The element list says so.
pub fn prescribed(mut s: MultiphysicsSetup) -> MultiphysicsSetup {
    s.neutronics.shape = PowerShape::J0Cosine {
        peak_to_mean: MAX_POWER_DENSITY_W_CM3 / MEAN_POWER_DENSITY_MW_M3,
    };
    s.coupling.max_iterations = 60;
    s.elements = elements_for(false);
    s
}

/// The element list for a solved (`true`) or prescribed power shape.
pub fn elements_for(solved: bool) -> Vec<ModelElement> {
    let mut v = elements();
    if !solved {
        for e in &mut v {
            if e.name == "Power shape and k" {
                e.status = ElementStatus::Simplified;
                e.note = "ABLATION (--prescribed-power): PRESCRIBED shape (J0 x cosine with its one length fixed by the published peak/mean, or uniform); k is LUMPED, rho = alpha_iso (T_bed - T_ref), relative to a cold-critical reference, not an eigenvalue. The default solves it by diffusion (gh:#591).".into();
            }
        }
        v.retain(|e| {
            !matches!(
                e.name.as_str(),
                "Neutronics data"
                    | "Neutronics regions"
                    | "Temperature feedback"
                    | "Reflector, conus, tube temperatures (for the cross sections)"
                    | "Neutronics -> ring grid transfer"
            )
        });
    }
    v
}

/// The HTR-10 Step 9 case. `reference_temperature_k` is Step 5's data
/// temperature, the cold state the feedback is measured from.
pub fn htr10(reference_temperature_k: f64) -> MultiphysicsSetup {
    MultiphysicsSetup {
        foam: FoamSide {
            core_radius_cm: 0.5 * table1::CORE_DIAMETER_CM,
            core_height_cm: table1::CORE_HEIGHT_CM,
            filling_fraction: table1::BALL_FILLING_FRACTION,
            pebble_diameter_cm: table1::BALL_DIAMETER_CM,
            // Typed: the equilibrium core has no graphite balls (Gao & Shi
            // §4.1, "fuel element heat factor 1.0").
            fuel_pebble_fraction: 1.0,
            inlet_temperature_c: htr10_design_point::core_inlet_temperature().get::<degree_celsius>(),
            total_mass_flow_kg_s: TOTAL_FLOW_KG_S,
            core_mass_flow_kg_s: CORE_FLOW_KG_S,
            outlet_pressure_mpa: htr10_design_point::pressure().get::<megapascal>(),
            wall: WallCondition::Adiabatic,
            flow_downward: true,
            fast_fluence_1e25_per_m2: 0.0,
            friction_model: "KTA 3102.3 packed-bed friction".into(),
            heat_transfer_model: "Wakao-Funazkri (1978) particle-to-fluid Nusselt".into(),
            pebble_model: "two-zone conduction + hottest TRISO (tampines Pebble::htr10)".into(),
            helium_properties: "outram-park-fork-coolprop helium EOS, Arp-McCarty-Friend viscosity, Hands-Arp conductivity".into(),
            // Mesh study 2026-10-05 (V&V record): with no inter-ring
            // conduction the peaks rise as rings resolve the centreline;
            // 5x40 reads 931.5 °C peak fuel, 20x160 955.1, 40x200 958.7,
            // 80x400 960.9. 40x200 is within 2.2 K of 80x400 and runs in
            // about 10 s; the coarse mesh is NOT kept for reading closer to
            // the published value.
            radial_rings: 40,
            axial_nodes: 200,
        },
        neutronics: NeutronicsSide {
            thermal_power_mw: table1::THERMAL_POWER_MW,
            // Solved by default (gh:#591); `prescribed()` is the ablation.
            shape: PowerShape::Diffusion,
            isothermal_coefficient_per_k: ISOTHERMAL_COEFFICIENT_PER_K,
            reference_temperature_k,
            weighting: FeedbackWeighting::Power,
        },
        coupling: CouplingSettings {
            max_iterations: 150,
            pressure_tolerance: 1e-4,
            temperature_tolerance_k: 0.01,
            flow_relaxation: 0.8,
            power_relaxation: 0.5,
            power_tolerance: 1e-4,
            k_tolerance: 1e-6,
        },
        structural: StructuralSide {
            enabled: false,
            element: "Tet4 (never polyhedral for stress)".into(),
            material_model: "linear elastic, small strain".into(),
            load: "thermal expansion from the TH temperatures (needs the Step 7 mapping)".into(),
        },
        elements: elements(),
        cites: vec![
            cite("foam.core_radius_cm", LI2014, "Table 1: core diameter 180 cm"),
            cite("foam.core_height_cm", LI2014, "Table 1: core height 197 cm"),
            cite("foam.filling_fraction", LI2014, "body text: ball filling fraction 0.61"),
            cite("foam.pebble_diameter_cm", TECDOC1382, "Chapter 4: ball diameter 6.0 cm"),
            cite("foam.inlet_temperature_c", GAO2002, "Table 2: helium at reactor inlet 250 °C at 100 % load"),
            cite("foam.outlet_pressure_mpa", GAO2002, "Table 2: coolant pressure 3 MPa"),
            cite("foam.total_mass_flow_kg_s", GAO2002, "Table 2: coolant mass flow 4.32 kg/s"),
            cite("foam.core_mass_flow_kg_s", GAO2002, "Table 1: 3.77 kg/s through the pebble bed and bottom reflector (87.3 %); §1 asks at least 86 %"),
            cite("foam.flow_downward", GAO2002, "§2: cold helium from the top plenum flows downward through the bed"),
            cite("foam.fuel_pebble_fraction", GAO2002, "§4.1: equilibrium core, fuel element heat factor 1.0 (no graphite balls)"),
            cite("neutronics.thermal_power_mw", LI2014, "Table 1: thermal power 10 MW"),
            cite("neutronics.shape", GAO2002, "§4.2: max power density 2.57 W/cm³ (equilibrium core) against Table 2's 2 MW/m³ mean (the prescribed-shape ablation's input; the comparison for the solved shape)"),
            cite("neutronics.isothermal_coefficient_per_k", CHEN2009, "Table 1: -1.4e-4 dk/k per °C (as transcribed in htgr_sim_v1 kinetics.rs; not in the kovan corpus)"),
        ],
    }
}
