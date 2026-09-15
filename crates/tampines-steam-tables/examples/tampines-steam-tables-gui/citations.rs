//! The Citations tab: every literature/attribution source this GUI relies
//! on, gathered in one place (issue #26, 2026-08-21: "ensure to include the
//! citation within the gui app" / "display it under separate a citations
//! tab").
//!
//! This module does not hold its own copy of any reference text — it reads
//! [`LayerId::provenance`] for the reference-data layers (so a citation
//! cannot drift between the sidebar tooltip, the CSV export and this tab)
//! and the Gruvbox attribution already documented in [`crate::theme`].

use eframe::egui;

use crate::data::LayerKind;
use crate::layers::LayerId;

/// One citation entry: the reference text, and which layer(s)/feature it
/// backs, so a reader can tell what in the app it is provenance for.
struct CitationEntry {
    reference: &'static str,
    used_for: Vec<&'static str>,
}

/// Every literature citation behind a reference-data layer, deduplicated by
/// reference text (the two Wagner/Kretzschmar layers cite the same source,
/// so they collapse into one entry with both labels listed under it) and
/// grouped in [`LayerId::ALL`] order.
fn reference_layer_citations() -> Vec<CitationEntry> {
    let mut entries: Vec<CitationEntry> = Vec::new();
    for layer in LayerId::ALL {
        if layer.kind() != LayerKind::ReferencePoints {
            continue;
        }
        let reference = layer.provenance();
        if let Some(entry) = entries.iter_mut().find(|e| e.reference == reference) {
            entry.used_for.push(layer.label());
        } else {
            entries.push(CitationEntry {
                reference,
                used_for: vec![layer.label()],
            });
        }
    }
    entries
}

/// Draws the Citations tab: one entry per literature/attribution source,
/// each showing the full reference text and which layer(s) or GUI feature it
/// backs.
pub fn ui(ui: &mut egui::Ui) {
    ui.heading("Citations");
    ui.label(
        "Every reference-data layer and every borrowed visual asset this GUI uses, with the \
         source it comes from. Nothing plotted or styled here is uncited.",
    );
    ui.separator();

    ui.heading("Reference / validation data");
    for entry in reference_layer_citations() {
        ui.group(|ui| {
            ui.label(egui::RichText::new(entry.used_for.join(", ")).strong());
            ui.label(entry.reference);
        });
        ui.add_space(4.0);
    }

    ui.separator();
    ui.heading("Tabulated single-phase and saturation tables");
    ui.group(|ui| {
        ui.label(egui::RichText::new("Tabulated data layers (isobar/isotherm crosses, single-phase points, saturation-table points)").strong());
        ui.label(crate::layers::WAGNER_KRETZSCHMAR_CITATION);
    });

    ui.separator();
    ui.heading("Visual assets");
    ui.group(|ui| {
        ui.label(egui::RichText::new("Gruvbox Dark / Gruvbox Light theme palettes").strong());
        ui.label(
            "Based on morhetz/gruvbox (https://github.com/morhetz/gruvbox), licensed under the \
             MIT License. Only the published hex colour values are reproduced here; no source \
             code from that project is used.",
        );
    });
}

/// Checks every reference-data [`LayerId`] contributes a non-empty citation,
/// and that layers sharing one provenance string (the two Wagner/Kretzschmar
/// layers) collapse into a single [`CitationEntry`] rather than appearing
/// twice.
///
/// # Result (measured 2026-08-21)
///
/// Holds: 7 reference-data layers collapse to fewer than 7 entries (the two
/// Wagner/Kretzschmar layers share one), and every entry's reference text is
/// non-empty.
#[cfg(test)]
#[test]
fn reference_layer_citations_are_nonempty_and_deduplicated() {
    let entries = reference_layer_citations();
    let reference_layer_count = LayerId::ALL
        .iter()
        .filter(|l| l.kind() == LayerKind::ReferencePoints)
        .count();
    assert!(
        entries.len() < reference_layer_count,
        "expected at least one shared citation (Wagner/Kretzschmar) to collapse two layers \
         into one entry"
    );
    for entry in &entries {
        assert!(
            !entry.reference.is_empty(),
            "citation text must not be empty"
        );
        assert!(
            !entry.used_for.is_empty(),
            "citation must name at least one layer"
        );
    }
}
