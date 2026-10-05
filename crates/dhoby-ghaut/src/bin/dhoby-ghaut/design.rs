//! What the recipe asks the geometry builder for, and what it cannot build.
//!
//! Every model input of Steps 1–5 either reaches the assembled core through
//! [`Htr10CoreDesign`] (the pebble-type mix, the fuel pebble's fuelled zone,
//! TRISO radii and particle count, the control-rod insertion), or is listed
//! by [`plan`] as NOT built, in words the settings panel shows (gh:#566).
//! Nothing a recipe states is dropped silently: a value the `nee_soon`
//! builders cannot represent is named, with what the model uses instead.
//!
//! The comparison values for "cannot represent" are the model's own
//! constants (`nee_soon::htr10_rmc`), not the preset recipe, so a recipe
//! edited by hand is judged against what is actually built.

use dhoby_ghaut::workbench::recipe::{PebbleDesign, Recipe};
use nee_soon::htr10_rmc::control_rod::{
    B4C_DENSITY_G_PER_CM3, B4C_INNER_RADIUS_CM, B4C_OUTER_RADIUS_CM, N_CONTROL_RODS,
};
use nee_soon::htr10_rmc::core_design::Htr10CoreDesign;
use nee_soon::htr10_rmc::core_model::{
    HTR10_CORE_RADIUS_CM, HTR10_MODEL_HEIGHT_CM, HTR10_REFLECTOR_OUTER_CM,
};
use nee_soon::htr10_rmc::explicit_bed::PEBBLE_RADIUS_CM;

/// The design the builder is asked for, and every recipe value it cannot
/// represent.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildPlan {
    /// What the geometry is built to.
    pub design: Htr10CoreDesign,
    /// Recipe values NOT in the built model, each a sentence saying what is
    /// built instead. Empty when the recipe is fully represented.
    pub not_built: Vec<String>,
}

const TOL: f64 = 1e-9;

fn differs(a: f64, b: f64) -> bool {
    (a - b).abs() > TOL * b.abs().max(1.0)
}

/// Map `r` onto the builder's design, listing what it cannot build.
pub fn plan(r: &Recipe) -> BuildPlan {
    let mut d = Htr10CoreDesign::default();
    let mut nb: Vec<String> = Vec::new();

    // Step 1: the mix. Fuel is built as a fraction of all balls above the
    // bed floor; fertile and poison balls do not exist in the model.
    let mix = &r.pebble_bed.mix;
    let total = mix.total();
    if total > 0.0 {
        d.fuel_ball_fraction = (mix.fuel / total).clamp(0.0, 1.0);
        if differs(total, 1.0) {
            nb.push(format!(
                "The mix sums to {total:.3}; it is built normalised (fuel {:.3} of all balls).",
                d.fuel_ball_fraction
            ));
        }
    } else {
        nb.push("The mix is empty; the paper's 0.57 fuel fraction is built.".into());
    }
    for (name, f) in [("fertile", mix.fertile), ("poison", mix.poison)] {
        if f > 0.0 {
            nb.push(format!(
                "{name} pebbles ({:.1} % of the mix) are NOT in the model: the builder has fuel and \
                 graphite balls only, so they are built as graphite (moderator) balls.",
                100.0 * f / total.max(f)
            ));
        }
    }

    // Step 2: the designs.
    if r.pebble_design.interstitial.to_lowercase() != "helium" {
        nb.push(format!(
            "Interstitial \"{}\" is NOT built: the voids are helium.",
            r.pebble_design.interstitial
        ));
    }
    let mut saw_fuel = false;
    for p in &r.pebble_design.pebbles {
        match p.kind.as_str() {
            "fuel" => {
                saw_fuel = true;
                fuel_design(p, &mut d, &mut nb);
            }
            "moderator" => shell_only(p, &mut nb),
            other => nb.push(format!(
                "A \"{other}\" pebble design is NOT in the model (fuel and graphite balls only)."
            )),
        }
    }
    if !saw_fuel {
        nb.push("No fuel pebble design: the record's fuel pebble is built.".into());
    }

    // Steps 3 and 4: fixed hardware in the model.
    let rf = &r.reflector;
    for (what, got, built) in [
        ("core radius", rf.core_radius_cm, HTR10_CORE_RADIUS_CM),
        ("reflector outer radius", rf.reflector_outer_cm, HTR10_REFLECTOR_OUTER_CM),
        ("model height", rf.model_height_cm, HTR10_MODEL_HEIGHT_CM),
        ("B4C inner radius", r.inserts.b4c_inner_radius_cm, B4C_INNER_RADIUS_CM),
        ("B4C outer radius", r.inserts.b4c_outer_radius_cm, B4C_OUTER_RADIUS_CM),
    ] {
        if differs(got, built) {
            nb.push(format!("The {what} {got:.3} cm is NOT built: the model's is {built:.3} cm."));
        }
    }
    if differs(r.inserts.b4c_density_g_per_cm3, B4C_DENSITY_G_PER_CM3) {
        nb.push(format!(
            "B4C density {:.3} g/cm³ is NOT built: the model's is {B4C_DENSITY_G_PER_CM3} g/cm³.",
            r.inserts.b4c_density_g_per_cm3
        ));
    }
    if r.inserts.control_rods != N_CONTROL_RODS {
        nb.push(format!(
            "{} control rods are NOT built: the model has {N_CONTROL_RODS}.",
            r.inserts.control_rods
        ));
    }

    // Step 5: the rods move together.
    d.rod_insertion = r.monte_carlo.rod_insertion.clamp(0.0, 1.0);
    if differs(d.rod_insertion, r.monte_carlo.rod_insertion) {
        nb.push(format!(
            "Rod insertion {:.3} is outside [0, 1]; {:.3} is built.",
            r.monte_carlo.rod_insertion, d.rod_insertion
        ));
    }
    if let Err(e) = d.check() {
        nb.push(format!("The builder refuses this design ({e}); the record's design is built instead."));
        let rods = d.rod_insertion;
        d = Htr10CoreDesign {
            rod_insertion: rods,
            fuel_ball_fraction: d.fuel_ball_fraction,
            ..Htr10CoreDesign::default()
        };
    }
    BuildPlan { design: d, not_built: nb }
}

/// A fuel pebble's design onto the builder's: fuelled zone, TRISO radii and
/// count are built; the ball radius, matrix, kernel and enrichment are not.
fn fuel_design(p: &PebbleDesign, d: &mut Htr10CoreDesign, nb: &mut Vec<String>) {
    shell_only(p, nb);
    match p.fuel_zone_radius_cm {
        Some(z) => d.fuel_zone_radius_cm = z,
        None => nb.push("The fuel pebble names no fuelled zone: the record's 2.5 cm is built.".into()),
    }
    let Some(t) = &p.triso else {
        nb.push("The fuel pebble has no TRISO design: the record's particle is built.".into());
        return;
    };
    if t.kernel.to_uppercase() != "UO2" {
        nb.push(format!("Kernel \"{}\" is NOT built: the kernel material is UO2.", t.kernel));
    }
    let enrichment = outram_mc_libs::pebble_beds::htr10::ENRICHMENT_WT;
    if differs(t.enrichment, enrichment) {
        nb.push(format!(
            "Enrichment {:.2} wt% is NOT built: the kernel material is fixed at {:.2} wt% U-235 \
             (materials are not edited by the workbench).",
            100.0 * t.enrichment,
            100.0 * enrichment
        ));
    }
    if t.layers.len() == 4 {
        let mut radii = [t.kernel_radius_cm; 5];
        for (i, (_, r)) in t.layers.iter().enumerate() {
            radii[i + 1] = *r;
        }
        d.triso_radii_cm = radii;
    } else {
        nb.push(format!(
            "A TRISO with {} coating layers is NOT built: the model's particle has four \
             (buffer, IPyC, SiC, OPyC); the record's radii are built.",
            t.layers.len()
        ));
    }
    d.particles_per_pebble = t.particles_per_pebble;
}

/// What any ball's design states that the builder cannot change.
fn shell_only(p: &PebbleDesign, nb: &mut Vec<String>) {
    if differs(p.outer_radius_cm, PEBBLE_RADIUS_CM) {
        nb.push(format!(
            "The {} pebble's outer radius {:.3} cm is NOT built: the bed lattice is built for \
             {PEBBLE_RADIUS_CM} cm balls.",
            p.kind, p.outer_radius_cm
        ));
    }
    if p.matrix.to_lowercase() != "graphite" {
        nb.push(format!(
            "The {} pebble's matrix \"{}\" is NOT built: it is graphite.",
            p.kind, p.matrix
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The preset is built exactly as the record's model, with nothing
    /// listed as missing.
    #[test]
    fn the_preset_is_fully_represented_by_the_default_design() {
        let p = plan(&crate::preset::htr10());
        assert_eq!(p.not_built, Vec::<String>::new());
        assert_eq!(p.design, Htr10CoreDesign::default());
    }

    /// Edits the builder can make change the design; edits it cannot are
    /// listed, never dropped (gh:#566, gh:#580).
    #[test]
    fn edits_reach_the_design_or_are_listed_as_not_built() {
        let mut r = crate::preset::htr10();
        r.pebble_bed.mix.fuel = 0.40;
        r.pebble_bed.mix.moderator = 0.50;
        r.pebble_bed.mix.poison = 0.10;
        r.monte_carlo.rod_insertion = 0.5;
        let fuel = r.pebble_design.pebbles.iter_mut().find(|p| p.kind == "fuel").expect("fuel");
        fuel.fuel_zone_radius_cm = Some(2.4);
        fuel.outer_radius_cm = 3.1;
        let t = fuel.triso.as_mut().expect("triso");
        t.particles_per_pebble = 9000;
        t.kernel_radius_cm = 0.03;
        t.enrichment = 0.20;
        let p = plan(&r);
        assert!((p.design.fuel_ball_fraction - 0.40).abs() < 1e-12);
        assert!((p.design.rod_insertion - 0.5).abs() < 1e-12);
        assert!((p.design.fuel_zone_radius_cm - 2.4).abs() < 1e-12);
        assert!((p.design.triso_radii_cm[0] - 0.03).abs() < 1e-12);
        assert_eq!(p.design.particles_per_pebble, 9000);
        let text = p.not_built.join("\n");
        for needle in ["poison", "outer radius 3.100", "Enrichment 20.00"] {
            assert!(text.contains(needle), "{needle} not listed in:\n{text}");
        }
        assert_eq!(p.not_built.len(), 3, "{text}");
    }

    /// A design the builder would refuse is listed, and the record's is
    /// built rather than a geometry that cannot exist.
    #[test]
    fn a_refused_design_falls_back_to_the_record_and_says_so() {
        let mut r = crate::preset::htr10();
        let fuel = r.pebble_design.pebbles.iter_mut().find(|p| p.kind == "fuel").expect("fuel");
        fuel.fuel_zone_radius_cm = Some(3.5);
        let p = plan(&r);
        assert!(p.not_built.iter().any(|s| s.contains("refuses")), "{:?}", p.not_built);
        assert_eq!(p.design.fuel_zone_radius_cm, Htr10CoreDesign::default().fuel_zone_radius_cm);
    }
}
