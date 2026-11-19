//! **Control-rod sliders and the rod-insertion drawing, for any number of
//! rods** (gh:#1654). The standard layout of every k_eff simulator (#1652)
//! puts the sliders in the left panel and the drawing in the right dock.
//!
//! A rod is an insertion fraction, 0 (withdrawn) to 1 (fully in); the model
//! maps the fraction to geometry. [`RodBank`] holds the reader's setting: a
//! master slider that moves every rod while synced, one slider per rod, the
//! selected rod (the one a side view cuts through). [`controls_ui`] draws
//! the sliders and [`draw_bank`] the rods as columns against the core.
//!
//! Moved out of the HTR-10 rung (`examples/monte_carlo_web/htr10/rods.rs`,
//! ten rods), which keeps its fixed-size `RodControls` and delegates here,
//! so its own tests still pin the behaviour. Research, education and V&V
//! only.

use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// What the reader has set, for `rods.len()` rods.
#[derive(Clone, Debug, PartialEq)]
pub struct RodBank {
    /// One master slider moves every rod.
    pub sync: bool,
    pub master: f64,
    /// Each rod's insertion, 0..=1.
    pub rods: Vec<f64>,
    /// The rod the side view cuts through.
    pub selected: usize,
}

impl RodBank {
    /// `n` rods, synced, all withdrawn, rod `selected` selected.
    pub fn new(n: usize, selected: usize) -> Self {
        Self { sync: true, master: 0.0, rods: vec![0.0; n], selected: selected.min(n.saturating_sub(1)) }
    }

    /// Sync on puts every rod at the master's position; off leaves them.
    pub fn set_sync(&mut self, on: bool) {
        self.sync = on;
        if on {
            self.rods.iter_mut().for_each(|r| *r = self.master);
        }
    }

    /// The master slider: moves every rod when synced.
    pub fn set_master(&mut self, f: f64) {
        self.master = f.clamp(0.0, 1.0);
        if self.sync {
            self.rods.iter_mut().for_each(|r| *r = self.master);
        }
    }

    /// Rod `i`'s slider: synced, it is the master (all follow); otherwise
    /// it moves rod `i` alone. Either way rod `i` becomes the selected rod.
    pub fn set_rod(&mut self, i: usize, f: f64) {
        self.selected = i.min(self.rods.len().saturating_sub(1));
        if self.sync {
            self.set_master(f);
        } else {
            self.rods[self.selected] = f.clamp(0.0, 1.0);
        }
    }

    /// Withdraw every rod (and the master).
    pub fn withdraw_all(&mut self) {
        self.master = 0.0;
        self.rods.iter_mut().for_each(|r| *r = 0.0);
    }

    /// The rods a page URL asks for: `?rods=` the insertions in %,
    /// comma-separated, or one value for all; `?sync=0` unsyncs; `?rod=`
    /// the selected rod, 1-based. Anything unreadable leaves the default
    /// ([`RodBank::new`]`(n, selected)`).
    pub fn from_query(n: usize, selected: usize, query: &[(String, String)]) -> Self {
        let get = |k: &str| query.iter().find(|(name, _)| name == k).map(|(_, v)| v.as_str());
        let mut c = Self::new(n, selected);
        if let Some(v) = get("rod").and_then(|v| v.parse::<usize>().ok()).filter(|v| (1..=n).contains(v)) {
            c.selected = v - 1;
        }
        let vals: Vec<f64> = get("rods").map_or(Vec::new(), |v| v.split(',').filter_map(|x| x.trim().parse::<f64>().ok()).map(|x| (x / 100.0).clamp(0.0, 1.0)).collect());
        if vals.len() == 1 {
            c.set_master(vals[0]);
        } else if vals.len() == n && n > 0 {
            c.sync = false;
            c.rods.copy_from_slice(&vals);
            c.master = vals[0];
        }
        if get("sync") == Some("0") {
            c.sync = false;
        }
        c
    }
}

/// `n` as a word for the sliders' notes ("all ten together"), digits past
/// twelve.
pub fn count_word(n: usize) -> String {
    const WORDS: [&str; 13] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];
    WORDS.get(n).map_or_else(|| n.to_string(), |w| (*w).to_string())
}

/// The sliders: sync, the master, one slider per rod (labelled by
/// `labels`, one per rod; tapping a name selects that rod), "Withdraw all".
/// `intro` is the simulator's line about its rods; `note`, when given, sits
/// under the per-rod sliders. Returns whether the setting changed.
///
/// GUI code, not reached by a test (the workspace's GUI exception); its
/// decisions are [`RodBank`]'s, which are tested.
pub fn controls_ui(ui: &mut egui::Ui, id: &str, bank: &mut RodBank, labels: &[String], intro: &str, note: Option<&str>) -> bool {
    let before = bank.clone();
    let all = count_word(bank.rods.len());
    ui.label(intro);
    let mut sync = bank.sync;
    if ui.checkbox(&mut sync, "Sync all rods").changed() {
        bank.set_sync(sync);
    }
    ui.weak(if bank.sync {
        format!("Synced: the master slider moves all {all} together and every rod slider follows. Moving any rod slider moves them all.")
    } else {
        "Not synced: each slider moves its own rod alone. Turning sync on puts every rod at the master slider's position.".to_string()
    });
    let mut m = bank.master;
    ui.add_enabled_ui(bank.sync, |ui| {
        if ui.add(egui::Slider::new(&mut m, 0.0..=1.0).text("all rods").custom_formatter(|v, _| format!("{:.0} %", 100.0 * v))).changed() {
            bank.set_master(m);
        }
    });
    egui::CollapsingHeader::new("Each rod").id_salt(id).default_open(!bank.sync).show(ui, |ui| {
        egui::Grid::new((id, "rod-sliders")).num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
            for i in 0..bank.rods.len() {
                let sel = bank.selected == i;
                let name = labels.get(i).cloned().unwrap_or_else(|| format!("rod {}", i + 1));
                if ui.selectable_label(sel, name).clicked() {
                    bank.selected = i;
                }
                let mut f = bank.rods[i];
                if ui.add(egui::Slider::new(&mut f, 0.0..=1.0).show_value(true).custom_formatter(|v, _| format!("{:.0} %", 100.0 * v))).changed() {
                    bank.set_rod(i, f);
                }
                ui.end_row();
            }
        });
        if let Some(n) = note {
            ui.weak(n);
        }
    });
    if ui.button("Withdraw all").clicked() {
        bank.withdraw_all();
    }
    *bank != before
}

/// How [`draw_bank`] draws: a depth axis running down the rect, the core's
/// band on it, and the colours.
#[derive(Clone, Debug, PartialEq)]
pub struct BankStyle {
    /// The caption at the top-left.
    pub title: String,
    /// Depth at the top and the bottom of the drawing, in the simulator's
    /// axial coordinate (e.g. HTR-10's `z_T` in cm, which grows downwards).
    pub depth_top: f64,
    pub depth_bottom: f64,
    /// The core's band (top, bottom) on that axis, and its label.
    pub core: (f64, f64),
    pub core_label: String,
    pub absorber: Color32,
    pub core_colour: Color32,
}

/// The rods as columns: each rod's absorber span `spans[i] = (top,
/// bottom)` on the style's depth axis against the core's band, the selected
/// rod outlined and labelled by number.
///
/// GUI drawing, not reached by a test (the workspace's GUI exception); its
/// positions are [`depth_to_y`]'s, which is tested.
pub fn draw_bank(painter: &egui::Painter, rect: Rect, style: &BankStyle, spans: &[(f64, f64)], selected: usize, text: f32) {
    let ink = bugis::Ink::of_ctx(painter.ctx());
    painter.rect_filled(rect, 4.0, ink.card);
    let f = egui::FontId::proportional((text * 0.8).max(8.0));
    let top = rect.top() + text + 6.0;
    let bottom = rect.bottom() - text - 4.0;
    let y = |d: f64| depth_to_y(d, style.depth_top, style.depth_bottom, top, bottom);
    painter.text(rect.left_top() + Vec2::new(6.0, 3.0), egui::Align2::LEFT_TOP, &style.title, f.clone(), ink.muted);
    let left = rect.left() + 34.0;
    let n = spans.len().max(1);
    let w = (rect.right() - 6.0 - left) / n as f32;
    let core = Rect::from_min_max(Pos2::new(left, y(style.core.0)), Pos2::new(rect.right() - 6.0, y(style.core.1)));
    painter.rect_filled(core, 0.0, style.core_colour);
    painter.text(Pos2::new(rect.left() + 4.0, core.top()), egui::Align2::LEFT_CENTER, &style.core_label, f.clone(), ink.muted);
    for (i, &(hi, lo)) in spans.iter().enumerate() {
        let x0 = left + w * i as f32 + w * 0.25;
        let x1 = left + w * (i as f32 + 0.75);
        let r = Rect::from_min_max(Pos2::new(x0, y(hi.max(style.depth_top))), Pos2::new(x1, y(lo)));
        painter.rect_filled(r, 1.0, style.absorber);
        if i == selected {
            painter.rect_stroke(r.expand(2.0), 2.0, Stroke::new(1.5, ink.text), egui::StrokeKind::Outside);
        }
        painter.text(Pos2::new(0.5 * (x0 + x1), bottom + 2.0), egui::Align2::CENTER_TOP, format!("{}", i + 1), f.clone(), if i == selected { ink.text } else { ink.muted });
    }
}

/// Screen y of depth `d` on an axis from `depth_top` (at `y_top`) to
/// `depth_bottom` (at `y_bottom`), linear.
pub fn depth_to_y(d: f64, depth_top: f64, depth_bottom: f64, y_top: f32, y_bottom: f32) -> f32 {
    y_top + (y_bottom - y_top) * ((d - depth_top) / (depth_bottom - depth_top)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(a, b)| ((*a).into(), (*b).into())).collect()
    }

    #[test]
    fn sliders_sync_unsync_and_withdraw() {
        let mut b = RodBank::new(3, 1);
        b.set_master(0.4);
        assert_eq!(b.rods, vec![0.4; 3]);
        b.set_rod(2, 0.7);
        assert_eq!((b.rods.clone(), b.selected), (vec![0.7; 3], 2), "synced: any rod moves them all");
        b.set_sync(false);
        b.set_rod(0, 0.1);
        assert_eq!(b.rods, vec![0.1, 0.7, 0.7]);
        b.set_rod(9, 2.0);
        assert_eq!((b.selected, b.rods[2]), (2, 1.0), "index and value clamped");
        b.set_sync(true);
        assert_eq!(b.rods, vec![0.7; 3], "sync puts every rod at the master");
        b.withdraw_all();
        assert_eq!((b.master, b.rods.clone()), (0.0, vec![0.0; 3]));
    }

    #[test]
    fn rods_from_the_url() {
        let b = RodBank::from_query(3, 0, &q(&[("rods", "50"), ("rod", "3")]));
        assert_eq!((b.rods.clone(), b.selected, b.sync), (vec![0.5; 3], 2, true));
        let b = RodBank::from_query(3, 0, &q(&[("rods", "10, 20,30")]));
        assert_eq!((b.rods.clone(), b.sync, b.master), (vec![0.1, 0.2, 0.3], false, 0.1));
        let b = RodBank::from_query(3, 1, &q(&[("rods", "1,2"), ("rod", "7"), ("sync", "0")]));
        assert_eq!(b, RodBank { sync: false, ..RodBank::new(3, 1) }, "wrong count and rod ignored");
    }

    #[test]
    fn depth_axis_and_count_words() {
        assert_eq!(depth_to_y(0.0, 0.0, 450.0, 10.0, 460.0), 10.0);
        assert_eq!(depth_to_y(450.0, 0.0, 450.0, 10.0, 460.0), 460.0);
        assert_eq!(depth_to_y(225.0, 0.0, 450.0, 10.0, 460.0), 235.0);
        assert_eq!(count_word(10), "ten");
        assert_eq!(count_word(3), "three");
        assert_eq!(count_word(13), "13");
    }
}
