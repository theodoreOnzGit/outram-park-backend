//! **A bottom tab bar over a main view** (gh:#1717): a strip of
//! finger-sized, selectable tabs along the bottom edge that switches what the
//! main view shows, as the MSRE simulator's bar does
//! (`outram-park-digital-twin-engine/examples/msre_web/app.rs`, `Tab`).
//!
//! # Why it lives here
//!
//! The MSRE simulator hand-rolled its bar: a fixed 460 px strip of 84 px
//! buttons centred on the main view. At a 390 px phone width that strip is
//! wider than the screen, so its outer tabs fall off both edges. The HTR-10
//! simulator needed the same bar (gh:#1716), so it is written once here, with
//! a layout that **always fits**: the tabs share the width that is there,
//! up to a comfortable maximum, and never overflow. `msre_web` can adopt it
//! by replacing its `bar` rect and button loop with [`layout`] and
//! [`tab_bar`]; that change is left to its own issue.
//!
//! It is not a `web_demo::dock::Dock`-style panel: a dock folds a
//! companion view away, while a tab bar replaces the main view's content.
//!
//! # Use
//!
//! ```
//! # use egui::{Pos2, Rect, Vec2};
//! let full = Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0));
//! let bar = bugis::tab_bar::layout(full, 3);
//! assert_eq!(bar.tabs.len(), 3);
//! assert!(bar.main.bottom() <= bar.bar.top());
//! // Draw the selected view in `bar.main`, then:
//! // if let Some(i) = bugis::tab_bar::tab_bar(ui, &bar, &["Plant", "CVs", "Fault tree"], selected) { .. }
//! ```
//!
//! Presentation only: no physics (the crate rule).

use alloc::vec::Vec;
use egui::{Pos2, Rect, RichText, Vec2};

/// Height of a tab, px: above the 36 px touch-target floor of the
/// mobile-first rule (`docs/claude-md/mobile-first-tutorials-and-demos.md`,
/// item 5) and close to `web_demo::dock`'s 44 px header.
pub const TAB_H: f32 = 40.0;
/// Margin around the tabs inside the bar, px.
pub const MARGIN_PX: f32 = 4.0;
/// Gap between neighbouring tabs, px.
pub const GAP_PX: f32 = 4.0;
/// Widest a tab grows on a wide screen, px: a bar of three tabs stays a
/// compact strip in the middle of a desktop window rather than a full-width
/// band.
pub const MAX_TAB_W: f32 = 150.0;
/// Height of the whole bar strip, px: one tab plus its margins.
pub const BAR_H: f32 = TAB_H + 2.0 * MARGIN_PX;

/// Where [`layout`] put the bar and its tabs.
#[derive(Clone, Debug, PartialEq)]
pub struct TabBarLayout {
    /// What is left for the main view, above the bar.
    pub main: Rect,
    /// The bar strip along the bottom of the full rect.
    pub bar: Rect,
    /// One rect per tab, left to right, inside [`Self::bar`].
    pub tabs: Vec<Rect>,
}

/// Split `full` into the main view and a bottom bar of `n` tabs.
///
/// The tabs share the bar's width minus its margins and gaps, each at most
/// [`MAX_TAB_W`], and the row is centred. They shrink to fit rather than
/// overflow, so the bar never runs off a narrow screen; `n = 0` gives an
/// empty bar of the same height (the main view does not jump when a caller
/// has no tabs to show).
pub fn layout(full: Rect, n: usize) -> TabBarLayout {
    let bar_h = BAR_H.min(full.height().max(0.0));
    let bar = Rect::from_min_max(Pos2::new(full.left(), full.bottom() - bar_h), full.max);
    let main = Rect::from_min_max(full.min, Pos2::new(full.right(), bar.top()));
    let mut tabs = Vec::with_capacity(n);
    if n > 0 {
        let gaps = GAP_PX * (n - 1) as f32;
        let room = (bar.width() - 2.0 * MARGIN_PX - gaps).max(0.0);
        let w = (room / n as f32).min(MAX_TAB_W);
        let row = w * n as f32 + gaps;
        let mut x = bar.center().x - 0.5 * row;
        let top = bar.top() + MARGIN_PX;
        let h = (bar_h - 2.0 * MARGIN_PX).max(0.0);
        for _ in 0..n {
            tabs.push(Rect::from_min_size(Pos2::new(x, top), Vec2::new(w, h)));
            x += w + GAP_PX;
        }
    }
    TabBarLayout { main, bar, tabs }
}

/// Draw the bar laid out by [`layout`] with one label per tab, the
/// `selected` one highlighted, and return the index of a tab clicked this
/// frame.
///
/// GUI drawing code, exempt from a reaching test (egui paint and clicks
/// cannot be driven from a unit test here); its geometry is [`layout`]'s,
/// which is tested. Labels beyond `layout.tabs.len()` are ignored.
pub fn tab_bar(ui: &mut egui::Ui, layout: &TabBarLayout, labels: &[&str], selected: usize) -> Option<usize> {
    // The strip in the theme's panel colour, so the main view's drawing does
    // not show through between the tabs.
    ui.painter().rect_filled(layout.bar, 0.0, ui.visuals().panel_fill);
    let mut clicked = None;
    for (i, (rect, label)) in layout.tabs.iter().zip(labels.iter()).enumerate() {
        let button = egui::Button::selectable(i == selected, RichText::new(*label).size(16.0));
        if ui.put(*rect, button).clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))
    }

    /// At phone width every tab is inside the screen, finger-sized, and the
    /// tabs do not overlap; the main view and the bar tile the full rect.
    #[test]
    fn three_tabs_fit_a_phone_and_tile_the_screen() {
        let full = rect(390.0, 844.0);
        let l = layout(full, 3);
        assert_eq!(l.tabs.len(), 3);
        assert_eq!(l.main.min, full.min);
        assert_eq!(l.main.bottom(), l.bar.top());
        assert_eq!(l.bar.max, full.max);
        assert_eq!(l.bar.height(), BAR_H);
        for t in &l.tabs {
            assert!(full.contains_rect(*t), "{t:?} is off the screen");
            assert!(t.height() >= 36.0, "touch target {t:?}");
        }
        for w in l.tabs.windows(2) {
            assert!(w[0].right() + GAP_PX <= w[1].left() + 1e-3, "tabs overlap: {w:?}");
        }
    }

    /// Five tabs (the MSRE bar's count) still fit 390 px, which its fixed
    /// 460 px strip did not; on a wide screen a tab stops growing at
    /// [`MAX_TAB_W`] and the row is centred.
    #[test]
    fn five_tabs_shrink_to_fit_and_wide_tabs_are_capped_and_centred() {
        let phone = layout(rect(390.0, 844.0), 5);
        assert!(phone.tabs.iter().all(|t| t.left() >= 0.0 && t.right() <= 390.0));
        assert!(phone.tabs[0].width() > 60.0, "a phone tab is still a usable width");

        let wide = layout(rect(1600.0, 900.0), 3);
        assert!(wide.tabs.iter().all(|t| t.width() == MAX_TAB_W));
        let left = wide.tabs[0].left();
        let right = 1600.0 - wide.tabs[2].right();
        assert!((left - right).abs() < 1e-3, "centred: {left} vs {right}");
    }

    /// No tabs: an empty bar of the usual height. A rect shorter than the
    /// bar gives the bar all of it and the main view none, never a negative
    /// size.
    #[test]
    fn degenerate_inputs_stay_well_formed() {
        let l = layout(rect(390.0, 844.0), 0);
        assert!(l.tabs.is_empty());
        assert_eq!(l.bar.height(), BAR_H);
        let tiny = layout(rect(390.0, 20.0), 3);
        assert_eq!(tiny.main.height(), 0.0);
        assert!(tiny.tabs.iter().all(|t| t.height() >= 0.0 && t.width() >= 0.0));
    }
}
