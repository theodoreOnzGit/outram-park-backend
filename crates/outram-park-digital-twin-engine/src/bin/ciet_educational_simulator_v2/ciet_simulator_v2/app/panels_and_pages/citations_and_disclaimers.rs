//! The disclaimer, copyright, artwork credits and citation block.
//!
//! ## Provenance
//!
//! Ported from the CIET Educational Simulator **v1**
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app/panels_and_pages/citations_and_disclaimers.rs`),
//! GPL-3.0, same licence.
//!
//! **What v2 changed here:** two lines added to the disclaimer stating that v2 is
//! a port of v1 and that the port equivalence has not been verified. Nothing was
//! removed — in particular the DWSIM artwork credit and both citations are
//! reproduced verbatim, because the DWSIM process-object images are used under
//! GPLv3 and the attribution is a licence obligation, not decoration.

use egui::Ui;

use crate::ciet_simulator_v2::CIETApp;

impl CIETApp {
    /// The citations / disclaimer / acknowledgements block, shown on the main
    /// page's side panel.
    ///
    /// Do not remove the DWSIM artwork credit: the heater, cooler and
    /// heat-exchanger images are DWSIM's, used under GPLv3, and the attribution
    /// is required by that licence.
    pub fn citation_disclaimer_and_acknowledgements(&mut self, ui: &mut Ui) {
        ui.heading("DISCLAIMER");

        ui.label(" ");

        ui.label("This is an educational simulator under testing and development");
        ui.label("Limited Validation has been done on transient forced circulation");
        ui.label("and steady statenatural circulation");
        ui.label("Validation is still work in progress");
        ui.label("This is given under GPLv3 without ANY WARRANY");
        ui.label("Results are not guaranteed to be physically accurate");
        ui.label("USE AT YOUR OWN RISK");

        ui.label(" ");
        ui.label("v2 note: this is a port of the CIET Educational Simulator v1.");
        ui.label("The physics is v1's, unchanged; the validation above refers to v1.");
        ui.label("Equivalence of the v2 port to v1 has NOT yet been verified.");

        ui.label(" ");
        ui.label(" ");

        ui.heading("COPYRIGHT");

        ui.label(" ");

        ui.label("Theodore Kay Chen Ong, SiCong Xiao, SNRSI, and Per F. Peterson");

        ui.label(" ");
        ui.label(" ");
        ui.heading("CREDITS");

        ui.label(" ");

        ui.label("Heater, cooler and heat exchanger artwork from DWSIM released under GPLv3");

        ui.label(" ");
        ui.label(" ");

        ui.heading("Citations appreciated:");
        ui.label(" ");

        ui.label("@phdthesis{ong2024digital,");
        ui.label("title={Digital Twins as Testbeds for Iterative Simulated Neutronics Feedback Controller Development},");
        ui.label("author={Ong, Theodore Kay Chen},");
        ui.label("year={2024},");
        ui.label("school={UC Berkeley}");
        ui.label("}");

        ui.label(" ");

        ui.label("@article{ong2024open,");
        ui.label("title={An open-source Thermo-hydraulic Uniphase Advection and Convection Solver for Salt Flows (TUAS)},");
        ui.label("author={Ong, Theodore Kay Chen and Xiao, Sicong and Peterson, Per F},");
        ui.label(
            "journal={International Journal of Advanced Nuclear Reactor Design and Technology},",
        );
        ui.label("volume={6},");
        ui.label("number={4},");
        ui.label("pages={281--301},");
        ui.label("year={2024},");
        ui.label("publisher={Elsevier}");
        ui.label("}");
    }
}
