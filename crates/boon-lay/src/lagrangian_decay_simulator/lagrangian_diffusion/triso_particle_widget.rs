// Cargo.toml
// [package]
// name = "triso_egui_widget"
// version = "0.1.0"
// edition = "2021"
//
// [dependencies]
// eframe = "0.27"
// egui = "0.27"

use eframe::{egui, egui::{Color32, Pos2, Stroke, Widget}};


#[derive(Clone)]
struct _TrisoParticle {
    // Metadata: physical diameter (not used for scaling unless you decide to map mm→px)
    _diameter_mm: f32,

    // Fraction of the smallest UI dimension that the particle’s diameter should occupy.
    // 0.40 means the drawn diameter will be ~40% of the panel's min(width, height).
    ui_diameter_ratio: f32,

    // How many concentric rings to draw (uniform spacing from center to outer radius).
    num_rings: usize,

    // Line style for rings.
    stroke: Stroke,

    // Optional tint for the rings (stroke color), background is taken from the panel.
    color: Color32,
}

impl Default for _TrisoParticle {
    fn default() -> Self {
        Self {
            _diameter_mm: 1.0,          // 1 mm (metadata)
            ui_diameter_ratio: 0.40,   // occupy ~40% of the UI
            num_rings: 18,             // adjust to taste
            stroke: Stroke { width: 6.0, color: Color32::WHITE },
            color: Color32::WHITE,
        }
    }
}

// Implement the Widget trait so you can `ui.add(triso.clone())`
impl Widget for _TrisoParticle {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        // Reserve all available space in the current UI region.
        let desired = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());

        // Compute center and target radius based on the 40% diameter target.
        let min_dim = rect.width().min(rect.height());
        let outer_radius = 0.5 * self.ui_diameter_ratio * min_dim; // radius = 0.5 * (ratio * min_dim)
        let center: Pos2 = rect.center();

        // Painter for drawing.
        let painter = ui.painter_at(rect);

        // Stroke setup (allow color override separate from stroke.color if desired).
        let mut stroke = self.stroke;
        stroke.color = self.color;

        // Draw concentric rings from inner to outer radius.
        // If num_rings == 0, we draw just a single circle at outer_radius.
        if self.num_rings == 0 {
            if outer_radius > 0.0 {
                painter.circle_stroke(center, outer_radius, stroke);
            }
        } else {
            let spacing = outer_radius / (self.num_rings as f32);
            // Start slightly away from 0 to avoid a degenerate tiny center circle thicker than its radius.
            for i in 1..=self.num_rings {
                let r = spacing * (i as f32);
                if r > 0.0 {
                    painter.circle_stroke(center, r, stroke);
                }
            }
        }

        response
    }
}

#[derive(Default)]
struct _App {
    particle: _TrisoParticle,
}

impl eframe::App for _App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_gray(32)))
            .show_inside(ui, |ui| {
                // Optional controls
                ui.horizontal(|ui| {
                    ui.label("Rings:");
                    ui.add(egui::Slider::new(&mut self.particle.num_rings, 0..=30));
                    ui.label("Line width:");
                    ui.add(egui::Slider::new(&mut self.particle.stroke.width, 1.0..=12.0));
                    ui.label("Diameter ratio:");
                    ui.add(egui::Slider::new(&mut self.particle.ui_diameter_ratio, 0.1..=0.9));
                });
                ui.separator();

                // Draw the TRISO particle widget
                ui.add(self.particle.clone());
            });
        let _ = ctx;
    }
}
