//! # The MC page (gh:#1740, #1761; on the standard shell since gh:#1758)
//!
//! **In the browser the page runs the full HTR-10 Monte Carlo** (maintainer,
//! 2026-11-25: "I want the mc in htgrsimv1 to be a full mc simulation built
//! in, not a link. Like a copy exposing the htr10 mc simulation with Dem
//! pour"): the Monte Carlo demo's HTR-10 core, with its DEM pour, the bed,
//! the rod bank, the worker pool and the k_eff and entropy plots, runs in a
//! frame placed exactly over the shell's central view
//! ([`place_core_frame`]), on the same site (`../monte-carlo/`, the same
//! origin, so its processed-data cache is the one the Monte Carlo demo
//! fills). It is that demo's simulation itself, not a second copy: one
//! implementation, `dhoby-ghaut/examples/monte_carlo_web/htr10/`. The
//! coefficient sets and the picker move to the left panel
//! ([`draw_sets`]).
//!
//! **Natively** there is no browser to host it: the central view shows the
//! coefficient sets, and says the full core runs in the browser build (or
//! `cargo run --release -p dhoby-ghaut --example monte_carlo_web`).
//!
//! Moving the rung's code into a library so this page draws it itself is
//! the longer route (#1761's plan); it is not done.
//!
//! GUI drawing code: exempt from a reaching test (it needs a live egui
//! frame or a browser). The rows are `CoefficientSet::values`, tested against
//! what the plant runs on (`physics::coefficient_set`).
//!
//! Research, education and V&V only. NOT VALIDATED.

use egui::{RichText, Ui};
use outram_park_digital_twin_engine::app_scaffold::SharedState;

use crate::app::panels::coefficient_picker;
use crate::app::state::HtgrSnapshot;
use crate::physics::coefficient_set::CoefficientSet;

/// The Monte Carlo demo's HTR-10 core, relative to this page
/// (`demos/htgr/` beside `demos/monte-carlo/`).
pub const CORE_URL: &str = "../monte-carlo/?rung=htr10&view=core";

/// The coefficient sets beside each other, and the picker.
pub fn draw_sets(ui: &mut Ui, physics: &SharedState<HtgrSnapshot>, snapshot: &HtgrSnapshot) {
    ui.label(RichText::new("Coefficient sets").strong());
    ui.label(
        RichText::new(
            "The plant's feedback uses a fuel (kernel Doppler) and a moderator (graphite) temperature \
             coefficient, and the ten-rod bank a total worth. Each set gives all three.",
        )
        .small(),
    );
    for set in CoefficientSet::ALL {
        let in_use = set == snapshot.coefficient_set;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let title = if in_use { format!("{}  (in use)", set.label()) } else { set.label().to_string() };
            ui.label(RichText::new(title).strong());
            match set.values() {
                Some((fuel, moderator, worth)) => {
                    egui::Grid::new(("mc_set", set.label())).num_columns(2).show(ui, |ui| {
                        ui.label("Fuel coefficient");
                        ui.label(format!("{:.2} pcm/K", fuel * 1e5));
                        ui.end_row();
                        ui.label("Moderator coefficient");
                        ui.label(format!("{:.2} pcm/K", moderator * 1e5));
                        ui.end_row();
                        ui.label("Rod-bank worth");
                        ui.label(format!("{:.2} % Δk/k", worth * 100.0));
                        ui.end_row();
                    });
                }
                None => {
                    ui.label(format!("Pending: the committed Monte Carlo record of gh:#{} is not in yet.", crate::physics::coefficient_set::POURED_BED_ISSUE));
                }
            }
            ui.small(set.source());
        });
        ui.add_space(4.0);
    }
    coefficient_picker(ui, physics, snapshot);
    ui.small("A switch keeps the plant's state and reference temperatures, so a hot plant steps in reactivity when the set changes.");
}

/// The central view natively: the sets, and where the full core runs.
pub fn draw(ui: &mut Ui, physics: &SharedState<HtgrSnapshot>, snapshot: &HtgrSnapshot) {
    let inner = ui.max_rect().shrink(8.0);
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut ui, |ui| {
        ui.heading("Monte Carlo: HTR-10");
        ui.label(
            "The full HTR-10 core (the DEM pour, the bed, the rods, k_eff on a pool of workers) runs on \
             this page in the browser build. Natively, run it with \
             `cargo run --release -p dhoby-ghaut --example monte_carlo_web` and pick the HTR-10 rung.",
        );
        ui.separator();
        draw_sets(ui, physics, snapshot);
        ui.small("Research, education and V&V only. Not validated.");
    });
}

/// Place the Monte Carlo demo's HTR-10 core over `rect` (the shell's
/// central view, in egui points), or hide it with `None`; the frame is made
/// on first use. `last` holds the style last set, so the page is touched
/// only when the rect moves. Browser only.
#[cfg(target_arch = "wasm32")]
pub fn place_core_frame(ctx: &egui::Context, rect: Option<egui::Rect>, last: &mut String) {
    const ID: &str = "htr10-mc-core";
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let frame = match doc.get_element_by_id(ID) {
        Some(e) => e,
        None => {
            if rect.is_none() {
                return;
            }
            let Ok(e) = doc.create_element("iframe") else { return };
            let _ = e.set_attribute("id", ID);
            let _ = e.set_attribute("src", CORE_URL);
            let _ = e.set_attribute("title", "HTR-10 whole-core Monte Carlo with DEM pour");
            let Some(body) = doc.body() else { return };
            if body.append_child(&e).is_err() {
                return;
            }
            e
        }
    };
    let style = match rect {
        None => "display:none".to_string(),
        Some(r) => {
            // egui points to CSS px, from the canvas's own place on the page.
            let (x0, y0) = doc.get_element_by_id("htgr_canvas").map(|c| {
                let b = c.get_bounding_client_rect();
                (b.left(), b.top())
            }).unwrap_or((0.0, 0.0));
            let z = ctx.zoom_factor() as f64;
            format!(
                "position:fixed;left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;border:0;z-index:5;background:#1d2021",
                x0 + r.left() as f64 * z,
                y0 + r.top() as f64 * z,
                r.width() as f64 * z,
                r.height() as f64 * z
            )
        }
    };
    if *last != style {
        let _ = frame.set_attribute("style", &style);
        *last = style;
    }
}
