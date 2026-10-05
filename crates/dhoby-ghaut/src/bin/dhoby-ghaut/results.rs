//! Step 0's tape table and Step 5's results: nuclide processing, the
//! `openmc.run()`-style console, k by generation, the lethargy spectrum and
//! the saved runs.

use egui::{Color32, RichText};
use egui_plot::{Line, Plot, PlotPoints, Points};

use crate::app::App;
use crate::engine::KeffOutcome;

pub fn tape_table(app: &mut App, ui: &mut egui::Ui) {
    let Some((dir, tapes, needed)) = &app.scan else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Scanning the ENDF folder…");
        });
        return;
    };
    ui.heading(format!("{} tapes in {}", tapes.len(), dir.display()));
    let in_folder = needed.iter().filter(|n| n.in_folder).count();
    let elsewhere: Vec<_> = needed
        .iter()
        .filter(|n| !n.in_folder && n.at_default)
        .collect();
    let missing: Vec<_> = needed
        .iter()
        .filter(|n| !n.in_folder && !n.at_default)
        .collect();
    ui.label(format!(
        "The HTR-10 data plan reads {} tapes: {} in this folder, {} elsewhere, {} missing.",
        needed.len(),
        in_folder,
        elsewhere.len(),
        missing.len()
    ));
    // Group the tapes found elsewhere by the folder they are read from (the
    // ENDF folder and the ACE submodule's, for HTR-10).
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    for n in &elsewhere {
        let d = n
            .default_path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    for d in &dirs {
        let names: Vec<&str> = elsewhere
            .iter()
            .filter(|n| n.default_path.parent() == Some(d.as_path()))
            .map(|n| n.file.as_str())
            .collect();
        let shown = std::fs::canonicalize(d).unwrap_or_else(|_| d.clone());
        ui.label(
            RichText::new(format!("Read from {} ({}):", shown.display(), names.len()))
                .color(Color32::from_rgb(70, 130, 190)),
        );
        ui.label(names.join(", "));
    }
    for n in &missing {
        ui.colored_label(
            Color32::from_rgb(220, 60, 60),
            format!("MISSING: {}", n.file),
        );
        ui.small(format!("expected at {}", n.default_path.display()));
    }
    let default_dir = nee_soon::htr10_rmc::data::DataDir::Endf.path();
    let default_dir = std::fs::canonicalize(&default_dir).unwrap_or(default_dir);
    if std::fs::canonicalize(dir).ok() != std::fs::canonicalize(&default_dir).ok() {
        ui.colored_label(
            Color32::from_rgb(170, 90, 0),
            format!(
                "This folder is checked, but the HTR-10 loader still reads {} (gh:#581).",
                default_dir.display()
            ),
        );
    }
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("tapes")
            .striped(true)
            .num_columns(5)
            .show(ui, |ui| {
                for h in ["file", "MAT", "ZSYMAM", "sub-library", "needed"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for t in tapes {
                    ui.label(&t.file);
                    ui.label(t.mat.map_or("?".into(), |m| m.to_string()));
                    ui.label(&t.symbol);
                    ui.label(t.kind());
                    ui.label(if needed.iter().any(|n| n.file == t.file) {
                        "yes"
                    } else {
                        ""
                    });
                    ui.end_row();
                }
            });
    });
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    // Fixed widths for the two text columns; the plots take the rest. The
    // height is the panel's own, never the window's.
    let h = 430.0;
    let w = ui.available_width();
    let (wp, wc) = (340.0_f32.min(w * 0.25), 640.0_f32.min(w * 0.42));
    ui.horizontal_top(|ui| {
        ui.allocate_ui(egui::vec2(wp, h), |ui| {
            ui.set_width(wp);
            ui.vertical(|ui| progress(app, ui));
        });
        ui.separator();
        ui.allocate_ui(egui::vec2(wc, h), |ui| {
            ui.set_width(wc);
            ui.vertical(|ui| console(app, ui));
        });
        ui.separator();
        let rest = ui.available_width();
        ui.allocate_ui(egui::vec2(rest, h), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("plots")
                .show(ui, |ui| plots(app, ui));
        });
    });
}

fn progress(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new("Nuclear data").strong());
    egui::ScrollArea::vertical()
        .id_salt("nuc")
        .max_height(380.0)
        .show(ui, |ui| {
            let n_done = app.mc.items.iter().filter(|i| i.1.is_some()).count();
            if !app.mc.items.is_empty() {
                ui.add(
                    egui::ProgressBar::new(n_done as f32 / app.mc.items.len() as f32)
                        .text(format!("{n_done} of {}", app.mc.items.len())),
                );
            }
            for (item, secs) in &app.mc.items {
                let running = app.mc.current.as_deref() == Some(item.as_str());
                ui.horizontal(|ui| {
                    match secs {
                        Some(s) => ui.label(format!("done  {item}  {s:.1} s")),
                        None if running => {
                            ui.spinner();
                            ui.label(format!("{item}  processing…"))
                        }
                        None => ui.colored_label(Color32::GRAY, item.as_str()),
                    };
                });
            }
        });
    if app.mc.running && !app.mc.loading_data && app.mc.started_at > 0.0 {
        let t = app.now() - app.mc.started_at;
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(format!(
                "Transport: {t:.0} s, {} histories planned",
                app.mc.planned_histories
            ));
        });
        let done = app.mc.live.read().map_or(0, |l| l.len());
        let total = app.recipe.monte_carlo.inactive + app.recipe.monte_carlo.active;
        ui.add(
            egui::ProgressBar::new(done as f32 / total.max(1) as f32)
                .text(format!("generation {done} of {total}")),
        );
    }
}

fn console(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Console").strong());
        let n = app.mc.outcomes.len();
        for i in 0..n {
            if ui
                .selectable_label(app.mc.shown == i, &app.mc.outcomes[i].label)
                .clicked()
            {
                app.mc.shown = i;
            }
        }
    });
    // While a run is going, its generations stream in live; afterwards the
    // finished run (or the one picked above) is shown.
    let live: Vec<(f64, Option<f64>)> = if app.mc.running {
        app.mc
            .live
            .read()
            .map(|l| l.iter().map(|g| (g.k, g.entropy)).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let inactive = |shown: usize| {
        let mc = &app.recipe.monte_carlo;
        mc.runs.get(shown).map_or(mc.inactive, |r| r.inactive)
    };
    let mut lines = vec![
        " Bat./Gen.      k       Entropy         Average k".to_string(),
        " =========   ========   ========   ====================".to_string(),
    ];
    if app.mc.running {
        if live.is_empty() {
            lines.push(if app.mc.loading_data {
                " (processing nuclear data…)".into()
            } else {
                " (first generation running…)".into()
            });
        }
        let ks: Vec<f64> = live.iter().map(|x| x.0).collect();
        let hs: Vec<f64> = live.iter().filter_map(|x| x.1).collect();
        lines.extend(generation_lines(&ks, &hs, app.recipe.monte_carlo.inactive));
    } else if let Some(o) = app.mc.outcomes.get(app.mc.shown) {
        lines.extend(console_lines(o, inactive(app.mc.shown)));
        lines.push(String::new());
        lines.push(format!(" k-effective = {:.5} +/- {:.5}", o.k, o.sigma));
        lines.push(format!(
            " histories {}, lost (locate) {}, transport {:.1} s",
            o.histories, o.lost_locate, o.transport_s
        ));
        for n in &o.notes {
            lines.push(format!(" note: {n}"));
        }
    } else {
        ui.label("No run yet.");
        return;
    }
    egui::ScrollArea::both()
        .id_salt("console")
        .max_height(380.0)
        .max_width(ui.available_width())
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(lines.join("\n"))
                        .monospace()
                        .size(crate::app::fs(11.0)),
                )
                .extend(),
            );
        });
}

/// One console line per generation, as `openmc.run()` prints them: the
/// running mean and its standard error over the active generations so far.
pub fn console_lines(o: &KeffOutcome, inactive: usize) -> Vec<String> {
    generation_lines(&o.k_by_generation, &o.entropy, inactive)
}

/// [`console_lines`] from the raw per-generation k and entropy.
pub fn generation_lines(ks: &[f64], entropy: &[f64], inactive: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut active: Vec<f64> = Vec::new();
    for (i, k) in ks.iter().enumerate() {
        let h = entropy
            .get(i)
            .map_or("        ".to_string(), |h| format!("{h:8.5}"));
        let mut s = format!("{:>8}/1    {k:7.5}   {h}", i + 1);
        if i >= inactive {
            active.push(*k);
            let n = active.len() as f64;
            if active.len() > 1 {
                let m = active.iter().sum::<f64>() / n;
                let var = active.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
                s.push_str(&format!("   {m:7.5} +/- {:7.5}", (var / n).sqrt()));
            }
        }
        out.push(s);
    }
    out
}

fn plots(app: &mut App, ui: &mut egui::Ui) {
    let Some(o) = app.mc.outcomes.get(app.mc.shown) else {
        runs_table(app, ui);
        return;
    };
    ui.label(RichText::new("k by generation").strong());
    let pts: Vec<[f64; 2]> = o
        .k_by_generation
        .iter()
        .enumerate()
        .map(|(i, k)| [(i + 1) as f64, *k])
        .collect();
    Plot::new("kgen").height(130.0).show(ui, |p| {
        p.points(Points::new("k per generation", PlotPoints::from(pts)).radius(2.0));
        let n = o.k_by_generation.len() as f64;
        p.line(Line::new(
            "k mean",
            PlotPoints::from(vec![[1.0, o.k], [n, o.k]]),
        ));
    });
    ui.label(RichText::new("Neutron spectrum, φ per unit lethargy (normalised)").strong());
    let spec: Vec<[f64; 2]> = o
        .phi_per_lethargy
        .iter()
        .enumerate()
        .filter(|(_, v)| v.0 > 0.0)
        .map(|(i, v)| [(o.edges[i] * o.edges[i + 1]).sqrt().log10(), v.0])
        .collect();
    Plot::new("spectrum")
        .height(170.0)
        .x_axis_label("log10 E [eV]")
        .show(ui, |p| p.line(Line::new("φ(u)", PlotPoints::from(spec))));
    runs_table(app, ui);
}

fn runs_table(app: &App, ui: &mut egui::Ui) {
    if app.recipe.monte_carlo.runs.is_empty() {
        return;
    }
    ui.label(RichText::new("Runs").strong());
    egui::Grid::new("runs").striped(true).show(ui, |ui| {
        for h in [
            "run",
            "T [K]",
            "rods",
            "statistics",
            "k ± σ",
            "Δρ vs run 1 [pcm]",
        ] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        let first = app.recipe.monte_carlo.runs.first().map(|r| (r.k, r.sigma));
        for r in &app.recipe.monte_carlo.runs {
            ui.label(&r.label);
            ui.label(format!("{:.2}", r.temperature_k));
            ui.label(if r.rod_insertion == 0.0 {
                "out".to_string()
            } else {
                format!("{:.0} %", 100.0 * r.rod_insertion)
            });
            ui.label(format!("{} × [{} + {}]", r.particles, r.inactive, r.active));
            ui.label(format!("{:.5} ± {:.5}", r.k, r.sigma));
            match first {
                Some((k1, s1)) => {
                    let d = (1.0 / k1 - 1.0 / r.k) * 1.0e5;
                    let e =
                        ((s1 / (k1 * k1)).powi(2) + (r.sigma / (r.k * r.k)).powi(2)).sqrt() * 1.0e5;
                    ui.label(format!("{d:+.0} ± {e:.0}"))
                }
                None => ui.label(""),
            };
            ui.end_row();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_console_averages_only_active_generations() {
        let o = KeffOutcome {
            label: "t".into(),
            k: 1.0,
            sigma: 0.0,
            k_by_generation: vec![0.5, 0.5, 1.0, 1.2],
            entropy: vec![],
            histories: 0,
            lost_locate: 0,
            transport_s: 0.0,
            edges: vec![],
            phi_per_lethargy: vec![],
            notes: vec![],
        };
        let l = console_lines(&o, 2);
        assert_eq!(l.len(), 4);
        assert!(
            !l[2].contains("+/-"),
            "one active generation has no error bar"
        );
        assert!(l[3].contains("1.10000 +/- 0.10000"), "{}", l[3]);
    }
}
