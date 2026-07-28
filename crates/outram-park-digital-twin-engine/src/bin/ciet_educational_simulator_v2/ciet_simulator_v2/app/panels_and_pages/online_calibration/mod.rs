//! On-the-fly recalibration of the heater discretisation, while the simulator
//! is running.
//!
//! ## Provenance
//!
//! Ported from the CIET Educational Simulator **v1**
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app/panels_and_pages/online_calibration/mod.rs`),
//! GPL-3.0, same licence.
//!
//! **What v2 changed here:** v1 defined its own `HeaterType` enum in this file
//! and mirrored the user's choice in a GUI-local field that the repaint
//! callback pushed into shared state every frame. v2 re-exports the library's
//! [`HeaterType`] instead (so the OPC-UA `CoarseHeaterMesh` switch and the GUI
//! name the same type) and writes the choice into shared state **only when the
//! user changes it**, so a remote client toggling the mesh is not immediately
//! overwritten by the GUI.

// The heater-type re-export below is needed by the physics thread, which runs
// headless on Android; only the `egui` page is desktop-only.
#[cfg(not(target_os = "android"))]
use std::ops::Deref;

#[cfg(not(target_os = "android"))]
use egui::Ui;

#[cfg(not(target_os = "android"))]
use crate::ciet_simulator_v2::CIETApp;

/// Which discretisation of the CIET heater the physics thread integrates.
///
/// Defined in the crate library so the GUI, the physics thread and the OPC-UA
/// `CoarseHeaterMesh` switch all agree; see
/// [`outram_park_digital_twin_engine::ciet_opcua::state::HeaterType`].
pub use outram_park_digital_twin_engine::ciet_opcua::state::HeaterType;

#[cfg(not(target_os = "android"))]
impl CIETApp {
    /// The "Online Calibration" page: pick the heater mesh while running.
    ///
    /// Reads the currently-integrated mesh out of shared state each frame, so
    /// the combo box also acts as a read-back of whatever an OPC-UA client set.
    /// A change is written straight back into shared state under a write lock.
    pub fn ciet_sim_online_calibration_page(&mut self, ui: &mut Ui) {
        ui.heading("Heater Calibration");

        // Read-back: whatever the physics thread is currently integrating,
        // whether the GUI or an OPC-UA client asked for it.
        let current_heater_type: HeaterType =
            self.ciet_state.read().unwrap().deref().current_heater_type;

        let mut user_desired_heater_type = current_heater_type;

        let heater_type_display: String = current_heater_type.to_string();
        egui::ComboBox::from_label("Choose Heater Type")
            .selected_text(heater_type_display)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut user_desired_heater_type,
                    HeaterType::InsulatedHeaterV1Fine15Mesh,
                    "Heater V1 Fine (15 Nodes)",
                );
                ui.selectable_value(
                    &mut user_desired_heater_type,
                    HeaterType::InsulatedHeaterV1Coarse8Mesh,
                    "Heater V1 Coarse (8 Nodes)",
                );
            });

        // Only write when the user actually changed the selection. v1 wrote
        // every repaint, which would fight an OPC-UA client.
        if user_desired_heater_type != current_heater_type {
            self.ciet_state.write().unwrap().current_heater_type = user_desired_heater_type;
        }

        // displays current heater type
        ui.label("current_heater_type:");
        ui.label(format!("{current_heater_type}"));
    }
}
