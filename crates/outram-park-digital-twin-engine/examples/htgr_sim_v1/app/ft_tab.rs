//! **The Fault tree tab** (gh:#1720): the DRAFT HTR-10 SB-LOCA tree (or the
//! SCRAM reference sample) drawn by `bugis::fault_tree`, quantified by
//! `raffles::scram` through [`crate::fault_tree`], with the reader's
//! controls: a log-scale slider per basic-event probability, the SB-LOCA
//! break-size range, and the minimal cut sets and importance factors.
//!
//! **DRAFT: top-event definition pending Adolphus Lye (#1719).** Every
//! DUMMY number carries a DUMMY tag on the diagram and beside its slider.
//!
//! # State and recomputation
//!
//! [`FaultTreeTab`] owns the inputs; any change bumps a revision, and
//! [`FaultTreeTab::ensure`] rebuilds and re-quantifies only when the
//! revision moved. Rebuild plus quantification measured 0.13 ms in release
//! (`fault_tree::tests::quantify_is_well_inside_a_frame`), so it runs on the
//! UI thread when an input changes, never per frame (the no-lag rule allows
//! work well under a frame; a tree that took longer would move to the
//! worker).

use bugis::fault_tree::{self as ft, Diagram, Layout, Mark};
use bugis::web_demo::view::{apply_zoom, zoom_buttons, View};
use egui::{Rect, RichText, Ui};
use raffles::scram::fault_tree::FaultTreeModel;
use uom::si::f64::Length;
use uom::si::length::millimeter;

use crate::fault_tree::{
    diagram, event_node, format_probability, node_event, quantify, BreakSizeRange, DataKind, FaultTreeStudy, ModelChoice,
    Quantified,
};

/// Probability slider range: the smallest a slider reaches, and 1.
pub const P_MIN: f64 = 1e-9;
/// Break-diameter slider range, mm.
pub const D_RANGE_MM: (f64, f64) = (0.1, 1000.0);

/// One rebuild-and-quantify result.
pub struct Computed {
    pub model: FaultTreeModel,
    /// The model's own probabilities (cited or DUMMY), per basic event.
    pub defaults: Vec<f64>,
    /// What is quantified: the defaults with the reader's overrides.
    pub probabilities: Vec<f64>,
    pub q: Quantified,
    pub diagram: Diagram,
    pub layout: Layout,
}

/// What the tab asks the app to do this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FtAction {
    None,
    /// The reader selected basic event `.0`, which sits on CV `.1`: select
    /// that CV on the Control volumes tab.
    SelectedEvent(String, Option<String>),
    /// Switch to the Control volumes tab (the "Show its CV" button).
    ShowCv,
}

/// The tab's retained state.
pub struct FaultTreeTab {
    choice: ModelChoice,
    study: Result<FaultTreeStudy, String>,
    range: BreakSizeRange,
    count_sg_tube_rupture: bool,
    /// The reader's probabilities, by basic-event name.
    overrides: Vec<(String, f64)>,
    pub view: View,
    pub selected_event: Option<String>,
    pub highlighted_cut: Option<usize>,
    computed: Option<Result<Computed, String>>,
    revision: u64,
    computed_revision: u64,
}

impl Default for FaultTreeTab {
    fn default() -> Self {
        let choice = ModelChoice::default();
        Self {
            choice,
            study: FaultTreeStudy::load(choice),
            range: BreakSizeRange::default(),
            count_sg_tube_rupture: false,
            overrides: Vec::new(),
            view: View::new(4.0),
            selected_event: None,
            highlighted_cut: None,
            computed: None,
            revision: 1,
            computed_revision: 0,
        }
    }
}

impl FaultTreeTab {
    /// Switch model; clears the overrides and selection, which belong to the
    /// other model's events.
    pub fn set_choice(&mut self, choice: ModelChoice) {
        if choice != self.choice {
            self.choice = choice;
            self.study = FaultTreeStudy::load(choice);
            self.overrides.clear();
            self.selected_event = None;
            self.highlighted_cut = None;
            self.view.fitted = false;
            self.revision += 1;
        }
    }

    #[allow(dead_code)] // read by the tests
    pub fn choice(&self) -> ModelChoice {
        self.choice
    }

    /// Set the break-size range; the bounds are kept ordered.
    pub fn set_range(&mut self, range: BreakSizeRange) {
        let ordered = if range.min <= range.max { range } else { BreakSizeRange { min: range.max, max: range.min } };
        if ordered != self.range {
            self.range = ordered;
            self.highlighted_cut = None;
            self.revision += 1;
        }
    }

    #[allow(dead_code)] // read by the tests
    pub fn range(&self) -> BreakSizeRange {
        self.range
    }

    pub fn set_count_sg_tube_rupture(&mut self, on: bool) {
        if on != self.count_sg_tube_rupture {
            self.count_sg_tube_rupture = on;
            self.highlighted_cut = None;
            self.revision += 1;
        }
    }

    /// Override basic event `name`'s probability (clamped to `[P_MIN, 1]`).
    pub fn set_probability(&mut self, name: &str, p: f64) {
        let p = p.clamp(P_MIN, 1.0);
        match self.overrides.iter_mut().find(|(n, _)| n == name) {
            Some(entry) if entry.1 == p => return,
            Some(entry) => entry.1 = p,
            None => self.overrides.push((name.to_string(), p)),
        }
        self.revision += 1;
    }

    /// Back to the model's own value.
    pub fn reset_probability(&mut self, name: &str) {
        let before = self.overrides.len();
        self.overrides.retain(|(n, _)| n != name);
        if self.overrides.len() != before {
            self.revision += 1;
        }
    }

    /// Every input back to the model's own: probabilities, range, house
    /// event.
    pub fn reset_all(&mut self) {
        self.overrides.clear();
        self.range = BreakSizeRange::default();
        self.count_sg_tube_rupture = false;
        self.highlighted_cut = None;
        self.revision += 1;
    }

    /// Rebuild and re-quantify if an input changed since the last call.
    pub fn ensure(&mut self) {
        if self.computed_revision == self.revision && self.computed.is_some() {
            return;
        }
        self.computed_revision = self.revision;
        let study = match &self.study {
            Ok(s) => s,
            Err(e) => {
                self.computed = Some(Err(e.clone()));
                return;
            }
        };
        self.computed = Some((|| {
            let model = study.tree(&self.range, self.count_sg_tube_rupture)?;
            let defaults = model.probabilities().to_vec();
            let probabilities: Vec<f64> = model
                .basic_event_names()
                .iter()
                .zip(&defaults)
                .map(|(name, d)| self.overrides.iter().find(|(n, _)| n == name).map_or(*d, |(_, p)| *p))
                .collect();
            let q = quantify(&model, &probabilities)?;
            let diagram = diagram(study, &model, &probabilities, q.top_probability);
            let layout = ft::layout(&diagram);
            Ok(Computed { model, defaults, probabilities, q, diagram, layout })
        })());
    }

    /// The current result, after [`Self::ensure`].
    #[allow(dead_code)] // read by the tests
    pub fn computed(&self) -> Option<&Result<Computed, String>> {
        self.computed.as_ref()
    }

    /// The loaded model's documentation, if it loaded.
    pub fn study(&self) -> Option<&FaultTreeStudy> {
        self.study.as_ref().ok()
    }

    /// The `(component, cv)` the selected basic event sits on.
    pub fn selected_link(&self) -> Option<(Option<String>, Option<String>)> {
        let name = self.selected_event.as_deref()?;
        let e = self.study()?.event(name)?;
        Some((e.component.clone(), e.cv.clone()))
    }

    /// The basic events that sit on CV `cv`.
    pub fn events_on_cv(&self, cv: &str) -> Vec<String> {
        self.study()
            .map(|s| s.events.iter().filter(|e| e.cv.as_deref() == Some(cv)).map(|e| e.name.clone()).collect())
            .unwrap_or_default()
    }

    /// Per-occurrence marks for the diagram: the highlighted cut set if one
    /// is chosen; else the selected event (and the gates above it); else the
    /// events on `selected_cv`; else every event outside the break-size
    /// range dimmed.
    pub fn marks(&self, selected_cv: Option<&str>) -> Vec<Mark> {
        let Some(Ok(c)) = &self.computed else { return Vec::new() };
        let nodes_of = |events: &[usize]| -> Vec<usize> { events.iter().map(|&e| event_node(&c.model, e)).collect() };
        if let Some(cut) = self.highlighted_cut.and_then(|k| c.q.cut_sets.get(k)) {
            return ft::highlight(&c.layout, &nodes_of(&cut.members));
        }
        if let Some(e) = self.selected_event.as_deref().and_then(|n| c.model.basic_event_index(n)) {
            let node = event_node(&c.model, e);
            let mut m = ft::highlight(&c.layout, &[node]);
            for i in c.layout.occurrences_of(node) {
                m[i] = Mark::Selected;
            }
            return m;
        }
        if let Some(cv) = selected_cv {
            let on: Vec<usize> = self
                .events_on_cv(cv)
                .iter()
                .filter_map(|n| c.model.basic_event_index(n))
                .collect();
            if !on.is_empty() {
                return ft::highlight(&c.layout, &nodes_of(&on));
            }
        }
        let mut m = vec![Mark::Normal; c.layout.placed.len()];
        for (i, p) in c.layout.placed.iter().enumerate() {
            if node_event(&c.model, p.node).is_some_and(|e| !c.q.included[e]) {
                m[i] = Mark::Dim;
            }
        }
        m
    }

    /// Draw the main view into `rect` (GUI drawing code; exempt from a
    /// reaching test, egui paint and input cannot be driven here; its state
    /// is the tested methods above).
    pub fn draw(&mut self, ui: &mut Ui, rect: Rect, selected_cv: Option<&str>) -> FtAction {
        self.ensure();
        let mut action = FtAction::None;
        let resp = ui.interact(rect, ui.id().with("ft_tab_view"), egui::Sense::click_and_drag());
        let marks = self.marks(selected_cv);
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        let mut header: Vec<String> = vec![self.choice.banner().to_string(), crate::app::cv_tab::RESEARCH_BANNER.to_string()];
        match &self.computed {
            Some(Ok(c)) => {
                let centre = c.layout.centre();
                self.view.home = centre;
                self.view.half_extent = c.layout.half_extent();
                self.view.handle_input(ui, &resp);
                let view = self.view;
                let hits = ft::draw::draw(
                    &painter,
                    &c.diagram,
                    &c.layout,
                    |p| view.to_screen(rect, p[0], p[1]),
                    view.scale as f32,
                    &marks,
                    ft::draw::Palette::from_visuals(&visuals),
                );
                header.push(format!("Top event P = {} (exact, BDD)", format_probability(c.q.top_probability)));
                if resp.clicked() {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let hit = hits.iter().position(|r| r.contains(p));
                        match hit.and_then(|i| node_event(&c.model, c.layout.placed[i].node)) {
                            Some(e) => {
                                let name = c.model.basic_event_names()[e].clone();
                                let cv = self.study().and_then(|s| s.event(&name)).and_then(|i| i.cv.clone());
                                self.selected_event = Some(name.clone());
                                self.highlighted_cut = None;
                                action = FtAction::SelectedEvent(name, cv);
                            }
                            None => {
                                self.selected_event = None;
                                self.highlighted_cut = None;
                            }
                        }
                    }
                }
            }
            Some(Err(e)) => header.push(format!("The model could not be read: {e}")),
            None => {}
        }
        let lines: Vec<&str> = header.iter().map(String::as_str).collect();
        crate::app::cv_tab::banner(&painter, rect, &lines, &visuals);
        if let Some(z) = zoom_buttons(ui, rect) {
            apply_zoom(&mut self.view, rect, z);
        }
        action
    }

    /// The side-panel section: model choice, break-size range, one
    /// log-scale slider per basic event with its tag and a reset, the cut
    /// sets (click to highlight) and the importance factors (GUI drawing
    /// code; exempt, the state it edits is the tested methods above).
    pub fn side_panel(&mut self, ui: &mut Ui) -> FtAction {
        self.ensure();
        let mut action = FtAction::None;
        ui.heading("Fault tree");
        ui.label(RichText::new(self.choice.banner()).strong().color(ui.visuals().warn_fg_color));
        ui.small(crate::app::cv_tab::RESEARCH_BANNER);
        let mut choice = self.choice;
        egui::ComboBox::from_label("Model")
            .selected_text(choice.label())
            .show_ui(ui, |ui| {
                for c in ModelChoice::ALL {
                    ui.selectable_value(&mut choice, c, c.label());
                }
            });
        self.set_choice(choice);
        if ui.button("Reset all inputs").clicked() {
            self.reset_all();
        }

        if self.choice == ModelChoice::SblocaDraft {
            ui.separator();
            ui.label(RichText::new("Break-size range (DRAFT)").strong());
            ui.small("Equivalent break diameter. An event counts when its size band overlaps this range (house events IN_SIZE_*). Default 1-65 mm: up to the largest stated HTR-10 RPV penetration.");
            let mut lo = self.range.min.get::<millimeter>();
            let mut hi = self.range.max.get::<millimeter>();
            ui.add(egui::Slider::new(&mut lo, D_RANGE_MM.0..=D_RANGE_MM.1).logarithmic(true).suffix(" mm").text("min"));
            ui.add(egui::Slider::new(&mut hi, D_RANGE_MM.0..=D_RANGE_MM.1).logarithmic(true).suffix(" mm").text("max"));
            self.set_range(BreakSizeRange { min: Length::new::<millimeter>(lo), max: Length::new::<millimeter>(hi) });
            let mut sgtr = self.count_sg_tube_rupture;
            ui.checkbox(&mut sgtr, "Count SG tube rupture (water ingress; DRAFT off)");
            self.set_count_sg_tube_rupture(sgtr);
            if let Some(s) = self.study() {
                let houses = s.house_values(&self.range, self.count_sg_tube_rupture);
                egui::CollapsingHeader::new("Size bands").show(ui, |ui| {
                    for (name, on) in &houses {
                        ui.monospace(format!("{name} = {on}"));
                    }
                    for b in &s.bands {
                        ui.small(format!(
                            "{}: ({:.2}, {:.2}] mm. {}",
                            b.house,
                            b.d_min.get::<millimeter>(),
                            b.d_max.get::<millimeter>(),
                            b.source
                        ));
                    }
                });
            }
        }
        self.ensure();

        let Some(Ok(c)) = &self.computed else { return action };
        ui.separator();
        ui.label(RichText::new(format!("Top event P = {}", format_probability(c.q.top_probability))).strong().size(18.0));
        ui.small("Exact, from the BDD (raffles::scram::bdd). Probability within the one-year mission time.");

        // Probability sliders.
        ui.separator();
        ui.label(RichText::new("Basic events").strong());
        let names = c.model.basic_event_names().to_vec();
        let defaults = c.defaults.clone();
        let mut probs = c.probabilities.clone();
        let included = c.q.included.clone();
        let mut resets = Vec::new();
        let mut selected = None;
        for (i, name) in names.iter().enumerate() {
            let info = self.study().and_then(|s| s.event(name)).cloned();
            let tag = info.as_ref().map_or(DataKind::Unlabelled, |e| e.data);
            ui.horizontal_wrapped(|ui| {
                let mut text = RichText::new(name).monospace();
                if !included[i] {
                    text = text.weak();
                }
                if ui.selectable_label(self.selected_event.as_deref() == Some(name.as_str()), text).clicked() {
                    selected = Some((name.clone(), info.as_ref().and_then(|e| e.cv.clone())));
                }
                if !tag.tag().is_empty() {
                    let colour = if tag == DataKind::Dummy { ui.visuals().error_fg_color } else { ui.visuals().warn_fg_color };
                    ui.label(RichText::new(tag.tag()).strong().color(colour));
                }
                if !included[i] {
                    ui.small("(not counted at this range)");
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut probs[i], P_MIN..=1.0).logarithmic(true).custom_formatter(|v, _| format_probability(v)));
                if ui.add_enabled(probs[i] != defaults[i], egui::Button::new("Reset")).clicked() {
                    resets.push(name.clone());
                }
            });
            if let Some(e) = &info {
                ui.small(format!("{} · on {} · {}", e.component.as_deref().unwrap_or("-"), e.cv.as_deref().unwrap_or("-"), e.source));
            }
        }
        let changed: Vec<(String, f64)> =
            names.iter().zip(&probs).zip(&c.probabilities).filter(|((_, a), b)| a != b).map(|((n, a), _)| (n.clone(), *a)).collect();

        // Cut sets.
        ui.separator();
        ui.label(RichText::new(format!("Minimal cut sets ({})", c.q.cut_sets.len())).strong());
        ui.small("raffles::scram::zbdd; most probable first. Click one to highlight it on the tree.");
        let mut cut_click = None;
        for (k, cut) in c.q.cut_sets.iter().enumerate() {
            let members: Vec<&str> = cut.members.iter().map(|&m| names[m].as_str()).collect();
            let text = format!("{}  {}", format_probability(cut.probability), members.join(" + "));
            if ui.selectable_label(self.highlighted_cut == Some(k), RichText::new(text).monospace()).clicked() {
                cut_click = Some(k);
            }
        }

        // Importance.
        ui.separator();
        ui.label(RichText::new("Importance factors").strong());
        ui.small("raffles::scram::importance_factors_from_bdd. MIF is Birnbaum's; DIF is SCRAM's Fussell-Vesely diagnosis factor (p x RAW), not the cut-set fraction other codes call F-V.");
        egui::Grid::new("ft_importance").striped(true).show(ui, |ui| {
            for h in ["event", "MIF", "CIF", "DIF", "RAW", "RRW"] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for (i, name) in names.iter().enumerate() {
                ui.monospace(name);
                match c.q.importance[i] {
                    Some(f) => {
                        for v in [f.mif, f.cif, f.dif, f.raw, f.rrw] {
                            ui.monospace(format_probability(v));
                        }
                    }
                    None => {
                        ui.label("-");
                    }
                }
                ui.end_row();
            }
        });

        // Apply after the borrow of `computed` ends.
        for (n, p) in changed {
            self.set_probability(&n, p);
        }
        for n in resets {
            self.reset_probability(&n);
        }
        if let Some(k) = cut_click {
            self.highlighted_cut = if self.highlighted_cut == Some(k) { None } else { Some(k) };
            self.selected_event = None;
        }
        if let Some((name, cv)) = selected {
            self.selected_event = Some(name.clone());
            self.highlighted_cut = None;
            action = FtAction::SelectedEvent(name, cv);
        }
        if let Some((component, cv)) = self.selected_link() {
            ui.separator();
            ui.label(RichText::new("Selected event").strong());
            ui.label(format!("component: {}", component.unwrap_or_else(|| "not stated".into())));
            ui.label(format!("control volume: {}", cv.unwrap_or_else(|| "not stated".into())));
            if ui.button("Show its CV").clicked() {
                action = FtAction::ShowCv;
            }
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mm(x: f64) -> Length {
        Length::new::<millimeter>(x)
    }

    /// Inputs re-quantify only when they change: a slider moves the top
    /// probability, Reset brings it back, an unchanged value does not bump
    /// the revision.
    #[test]
    fn probability_changes_requantify_and_reset() {
        let mut tab = FaultTreeTab::default();
        tab.ensure();
        let base = tab.computed().unwrap().as_ref().unwrap().q.top_probability;
        let rev = tab.revision;
        tab.ensure();
        assert_eq!(tab.computed_revision, rev, "nothing changed, nothing recomputed");

        tab.set_probability("INSTR_LINE_BREAK", 0.3);
        tab.ensure();
        let raised = tab.computed().unwrap().as_ref().unwrap().q.top_probability;
        assert!(raised > base);
        let rev = tab.revision;
        tab.set_probability("INSTR_LINE_BREAK", 0.3);
        assert_eq!(tab.revision, rev, "the same value is not a change");
        tab.set_probability("INSTR_LINE_BREAK", 50.0);
        tab.ensure();
        let c = tab.computed().unwrap().as_ref().unwrap();
        let i = c.model.basic_event_index("INSTR_LINE_BREAK").unwrap();
        assert_eq!(c.probabilities[i], 1.0, "clamped");

        tab.reset_probability("INSTR_LINE_BREAK");
        tab.ensure();
        assert_eq!(tab.computed().unwrap().as_ref().unwrap().q.top_probability, base);
        let rev = tab.revision;
        tab.reset_probability("INSTR_LINE_BREAK");
        assert_eq!(tab.revision, rev);
    }

    /// The range slider drives inclusion, keeps its bounds ordered, and
    /// Reset all restores the DRAFT defaults; the SG-tube house event adds
    /// its event.
    #[test]
    fn range_and_house_event_controls_change_the_included_events() {
        let mut tab = FaultTreeTab::default();
        tab.ensure();
        let count = |t: &FaultTreeTab| t.computed().unwrap().as_ref().unwrap().q.included.iter().filter(|x| **x).count();
        assert_eq!(count(&tab), 12);
        tab.set_range(BreakSizeRange { min: mm(65.0), max: mm(20.0) });
        assert_eq!(tab.range(), BreakSizeRange { min: mm(20.0), max: mm(65.0) }, "ordered");
        tab.ensure();
        assert_eq!(count(&tab), 7);
        tab.set_count_sg_tube_rupture(true);
        tab.ensure();
        assert_eq!(count(&tab), 7, "a 14 mm tube band is below 20 mm");
        tab.set_range(BreakSizeRange::default());
        tab.ensure();
        assert_eq!(count(&tab), 13);
        tab.reset_all();
        tab.ensure();
        assert_eq!(count(&tab), 12);
        assert_eq!(tab.range(), BreakSizeRange::default());
    }

    /// Selecting an event links it to its CV and component; the CV side
    /// lists its events; marks follow the cut set, then the selection, then
    /// the CV, then dim what is out of range. Switching model clears it.
    #[test]
    fn selection_links_events_and_cvs_and_drives_the_marks() {
        let mut tab = FaultTreeTab::default();
        tab.ensure();
        let dims = tab.marks(None).iter().filter(|m| **m == Mark::Dim).count();
        assert_eq!(dims, 1, "only SG tube rupture (house event off) is out at the default");
        tab.selected_event = Some("FLT_BREAK".into());
        assert_eq!(
            tab.selected_link(),
            Some((Some("rpv.fuel_loading_tube".into()), Some("primary.rpv_annuli".into())))
        );
        let m = tab.marks(None);
        assert!(m.contains(&Mark::Selected) && m.contains(&Mark::Dim));
        tab.highlighted_cut = Some(0);
        assert!(!tab.marks(None).contains(&Mark::Selected), "a cut set overrides");
        tab.highlighted_cut = None;
        tab.selected_event = None;
        let on = tab.events_on_cv("primary.cold_duct");
        assert_eq!(on, ["HGD_TUBE_LEAK", "SG_VESSEL_LEAK", "CIRC_HOUSING_LEAK"]);
        assert!(tab.marks(Some("primary.cold_duct")).contains(&Mark::Highlight));
        tab.set_range(BreakSizeRange { min: mm(20.0), max: mm(65.0) });
        tab.ensure();
        assert!(tab.marks(None).contains(&Mark::Dim), "out-of-range events are dimmed");

        tab.set_choice(ModelChoice::TheatreSample);
        assert_eq!(tab.choice(), ModelChoice::TheatreSample);
        tab.ensure();
        assert!(tab.selected_event.is_none());
        assert!(tab.events_on_cv("primary.cold_duct").is_empty());
        let c = tab.computed().unwrap().as_ref().unwrap();
        assert_eq!(c.model.basic_event_names().len(), 3);
        assert!(tab.study().is_some());
        assert_eq!(FtAction::None, FtAction::None);
    }
}
