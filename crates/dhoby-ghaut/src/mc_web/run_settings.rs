//! **The k_eff run settings of the left panel** (gh:#1653): neutrons per
//! generation, inactive and active generations and the seed, held to the
//! demo caps, with "Defaults" and "Recorded settings" buttons. The standard
//! layout of every k_eff simulator (#1652) puts this in the left panel.
//!
//! Moved out of the HTR-10 rung's `pool_panel`
//! (`examples/monte_carlo_web/htr10/core/screen.rs`) so another simulator
//! does not copy it; the rung calls this. Research, education and V&V only.

use super::keff::{clamp_to_caps, settings_line, KeffConfig, MAX_GENERATIONS, MAX_PARTICLES, MIN_ACTIVE};

/// What the buttons under the fields set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunPresets {
    /// "Defaults".
    pub defaults: KeffConfig,
    /// "Recorded settings (...)": the settings a recorded result was run
    /// with, when the simulator has one.
    pub recorded: Option<KeffConfig>,
}

/// The fields' upper bounds: (inactive, active) given the other's value,
/// so the two together never exceed [`MAX_GENERATIONS`] and active keeps
/// [`MIN_ACTIVE`].
pub fn generation_bounds(cfg: &KeffConfig) -> (usize, usize) {
    let inactive_hi = MAX_GENERATIONS - cfg.n_active.max(MIN_ACTIVE);
    let active_hi = MAX_GENERATIONS - cfg.n_inactive.min(MAX_GENERATIONS - MIN_ACTIVE);
    (inactive_hi, active_hi)
}

/// The fields and the buttons. `id` keeps two panels on one page apart;
/// `min_inactive` is the simulator's floor on inactive generations;
/// `running` greys the buttons while a run is going (its settings are
/// fixed). After the fields, `cfg` is held to the caps
/// ([`clamp_to_caps`]). Returns whether `cfg` changed.
///
/// GUI code, not reached by a test (the workspace's GUI exception); its
/// bounds are [`generation_bounds`]' and the caps [`clamp_to_caps`]', which
/// are tested.
pub fn run_settings_ui(ui: &mut egui::Ui, id: &str, cfg: &mut KeffConfig, min_inactive: usize, presets: &RunPresets, running: bool) -> bool {
    let before = *cfg;
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        ui.label("neutrons per generation");
        ui.add(egui::DragValue::new(&mut cfg.n_particles).range(100..=MAX_PARTICLES).speed(100));
        ui.end_row();
        ui.label("inactive generations");
        let (hi, _) = generation_bounds(cfg);
        ui.add(egui::DragValue::new(&mut cfg.n_inactive).range(0..=hi));
        ui.end_row();
        ui.label("active generations");
        let (_, hi) = generation_bounds(cfg);
        ui.add(egui::DragValue::new(&mut cfg.n_active).range(MIN_ACTIVE..=hi));
        ui.end_row();
        ui.label("seed");
        ui.add(egui::DragValue::new(&mut cfg.seed));
        ui.end_row();
    });
    *cfg = clamp_to_caps(*cfg, min_inactive);
    if let Some(rec) = presets.recorded {
        if cfg.seed != rec.seed {
            ui.weak(format!("The record used seed {}.", rec.seed));
        }
    }
    ui.add_enabled_ui(!running, |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Defaults").clicked() {
                *cfg = presets.defaults;
            }
            if let Some(rec) = presets.recorded {
                if ui.button(format!("Recorded settings ({})", settings_line(&rec))).clicked() {
                    *cfg = rec;
                }
            }
        });
    });
    *cfg != before
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(n_inactive: usize, n_active: usize) -> KeffConfig {
        KeffConfig { n_particles: 1000, n_inactive, n_active, seed: 1, point_source: false, want_sites: false }
    }

    /// The bounds leave the other field room and never exceed the cap, and
    /// a set of fields at their bounds survives `clamp_to_caps` unchanged.
    #[test]
    fn the_bounds_share_the_generation_cap() {
        let (i_hi, a_hi) = generation_bounds(&cfg(40, 120));
        assert_eq!(i_hi, MAX_GENERATIONS - 120);
        assert_eq!(a_hi, MAX_GENERATIONS - 40);
        let (i_hi, _) = generation_bounds(&cfg(40, 0));
        assert_eq!(i_hi, MAX_GENERATIONS - MIN_ACTIVE, "active keeps its minimum");
        let (_, a_hi) = generation_bounds(&cfg(MAX_GENERATIONS, 2));
        assert_eq!(a_hi, MIN_ACTIVE, "inactive cannot take the active minimum");
        let at = cfg(40, a_hi.max(MAX_GENERATIONS - 40));
        assert_eq!(clamp_to_caps(at, 0), at);
    }
}
