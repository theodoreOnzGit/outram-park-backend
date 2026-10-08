//! Where everything on the code map goes, in world units (points at zoom 1,
//! y down). Pure and deterministic: the same [`CodeMap`] always gives the
//! same [`Layout`], to the last bit.
//!
//! # The rules
//!
//! - **Root** card centred at the top; under it one **app box** per row-4
//!   crate, side by side, centred over the pyramid.
//! - **Topic boxes** ([`Topic::COLUMNS`]) are columns through the
//!   topic rows (3 and 2, plus any other row a topic crate declares). Every
//!   topic gets a box, even an empty one. **Since 2026-10-06** (maintainer,
//!   #734) the eight boxes wrap onto **bands of [`BOXES_PER_BAND`]** (two
//!   rows of four) instead of one row of eight, which made the map about
//!   7:1 wide. Order is kept (left to right, then the next band). Each box
//!   labels its own rows in a strip at its left ([`ROW_TAG`]); within a
//!   band a row's cards line up across the boxes.
//! - Inside a topic box, **fidelity columns** run from the highest level on
//!   the left to the lowest on the right. The columns are the levels a crate
//!   in the box sits at, plus both ends of every range. A column is as wide
//!   as the most crates any one row puts at that level (ties sit side by
//!   side, sorted by name, at most [`MAX_TIES`] abreast before wrapping to
//!   a lane below; **row 2 stacks its ties one above another**,
//!   [`ROW2_TIES`], since 2026-10-08); a column only a range touches is half
//!   a card.
//! - A **range** crate spans from the left of its `hi` column to the right of
//!   its `lo` column, on its own lane below the single-level crates of its
//!   row (lanes assigned greedily, widest range first, so ranges that do not
//!   overlap share one). `raffles` `[0, 4]` therefore spans the whole Risk box.
//! - Rows line up across the boxes of a band: a row is as tall as the most
//!   lanes any box of that band needs in it.
//! - The **utilities base** (rows 1, then 0) spans the pyramid's width below
//!   the topic boxes, each row's crates centred.
//! - ~~The **knowledge-management box** stands to the right, from the app band
//!   to the bottom of the base; each of its crates sits in its own row's band.~~
//!   **CORRECTED 2026-10-08** (maintainer: "Knowledge management is a column
//!   on the left"): the knowledge-management box is **one card wide, left of
//!   the row gutter**, from the app band to the bottom of the base. Its
//!   crates stack one per lane, each in its own row's band; a band row (or a
//!   base row) is made tall enough for the knowledge-management crates it
//!   holds, so rows still line up across the map.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{CodeMap, CrateNode, Fidelity, Topic};

/// A card, world units.
pub const CARD_W: f64 = 230.0;
pub const CARD_H: f64 = 56.0;
/// The root card.
pub const ROOT_W: f64 = 300.0;
pub const ROOT_H: f64 = 72.0;
/// Space between cards, and between lanes.
pub const GAP: f64 = 14.0;
/// Inner padding of a box.
pub const PAD: f64 = 16.0;
/// Height of a box's title strip (title and fidelity labels).
pub const HEADER: f64 = 46.0;
/// Space between boxes.
pub const BOX_GAP: f64 = 30.0;
/// Space between bands of rows (row 3 to row 2 etc.).
pub const BAND_GAP: f64 = 26.0;
/// Width of the row-label gutter left of the pyramid.
pub const GUTTER: f64 = 70.0;
/// Most crates of one fidelity level side by side in one row; more wrap
/// onto a further lane below. A drawing choice (2026-10-06), not one of the
/// maintainer's rules: without it the six `outram-foam-*` crates at
/// fidelity 3 made the map about seven times wider than tall.
pub const MAX_TIES: usize = 3;
/// Crates of one fidelity level side by side in **row 2** (domain solvers):
/// one, so they stack (maintainer, 2026-10-08: "make row 2 stack on each
/// other so row 2 becomes thicker", to keep the map from being too wide).
/// Row 2 holds most crates (eight in thermal hydraulics, five of them at
/// F3), so it set the width of every topic box.
pub const ROW2_TIES: usize = 1;

/// How many crates of one fidelity level sit side by side in `row`. Row 3
/// stacks too (maintainer, 2026-10-08: "Neutronics can be stacked"; its
/// three row-3 crates were the only row-3 ties).
pub fn ties_in_row(row: u8) -> usize {
    if row == 2 || row == 3 { ROW2_TIES } else { MAX_TIES }
}
/// Topic boxes per band (maintainer, 2026-10-06: two rows of four).
pub const BOXES_PER_BAND: usize = 4;
/// Width of the strip inside a topic box's left edge that labels its rows.
pub const ROW_TAG: f64 = 30.0;

/// What a row means (hover text of the row labels; the tag documentation in
/// `crates/kovan/tests/code_map_tags.rs`).
pub fn row_meaning(row: u8) -> &'static str {
    match row {
        4 => "Row 4: integrated GUI apps",
        3 => "Row 3: coupled multiphysics",
        2 => "Row 2: domain solvers",
        1 => "Row 1: utilities built on another crate",
        0 => "Row 0: standalone utilities",
        _ => "Row: unknown",
    }
}

/// What a fidelity column means (hover text of the F labels).
pub fn fidelity_meaning(level: i8) -> &'static str {
    match level {
        4 => "F4: brute force (Monte Carlo, first-principles data)",
        3 => "F3: three resolved dimensions (CFD, diffusion)",
        2 => "F2: two (subchannel)",
        1 => "F1: one (system code)",
        0 => "F0: lumped",
        _ => "F?: no fidelity tag",
    }
}

/// An axis-aligned rectangle: top-left corner and size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn centre(&self) -> (f64, f64) {
        (self.x + 0.5 * self.w, self.y + 0.5 * self.h)
    }
    /// Whether the two overlap with positive area.
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
    /// Whether `o` lies inside this rectangle.
    pub fn contains(&self, o: &Rect) -> bool {
        o.x >= self.x - 1e-9
            && o.y >= self.y - 1e-9
            && o.right() <= self.right() + 1e-9
            && o.bottom() <= self.bottom() + 1e-9
    }
    fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect { x, y, w: self.right().max(o.right()) - x, h: self.bottom().max(o.bottom()) - y }
    }
}

/// What kind of box a [`Frame`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameKind {
    App,
    Topic,
    Utilities,
    KnowledgeManagement,
}

/// A box on the map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub kind: FrameKind,
    pub topic: Topic,
    pub title: String,
    pub rect: Rect,
}

/// A crate's card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub name: String,
    pub rect: Rect,
    /// Index into [`Layout::frames`] of the box the card is in.
    pub frame: usize,
}

/// A text label: a row number in the gutter, or a fidelity column's level
/// in a topic box's title strip. `(x, y)` is the text's centre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub x: f64,
    pub y: f64,
    pub text: String,
    /// What the label means, shown on hover ([`row_meaning`],
    /// [`fidelity_meaning`]).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tip: String,
}

/// Everything the map draws, world units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub root: Rect,
    pub frames: Vec<Frame>,
    /// One per crate, in [`CodeMap::crates`] order.
    pub cards: Vec<Card>,
    pub labels: Vec<Label>,
    /// The extent of everything above.
    pub bounds: Rect,
}

impl Layout {
    /// The card of the crate called `name`.
    pub fn card(&self, name: &str) -> Option<&Card> {
        self.cards.iter().find(|c| c.name == name)
    }
}

/// Column key: a fidelity level, or -1 for a crate with none.
type Col = i8;

fn hi_col(c: &CrateNode) -> Col {
    c.fidelity.map(|f| f.hi() as Col).unwrap_or(-1)
}
fn lo_col(c: &CrateNode) -> Col {
    c.fidelity.map(|f| f.lo() as Col).unwrap_or(-1)
}
fn is_range(c: &CrateNode) -> bool {
    matches!(c.fidelity, Some(Fidelity::Range(..)))
}

/// One topic box, laid out at the origin (x from 0, rows not yet placed).
struct TopicBox<'a> {
    topic: Topic,
    /// Columns high to low: (key, x offset from the inner left, width).
    cols: Vec<(Col, f64, f64)>,
    inner_w: f64,
    /// Per row: lanes, each a list of (crate, x offset, width).
    rows: BTreeMap<u8, Vec<Vec<(&'a CrateNode, f64, f64)>>>,
}

fn topic_box<'a>(topic: Topic, crates: &[&'a CrateNode]) -> TopicBox<'a> {
    // Columns: every point level, both ends of every range.
    let mut keys: BTreeSet<Col> = BTreeSet::new();
    for c in crates {
        keys.insert(hi_col(c));
        keys.insert(lo_col(c));
    }
    // Width of each column, in cards: the most single-level crates one row
    // puts there.
    let mut count: BTreeMap<(u8, Col), usize> = BTreeMap::new();
    for c in crates.iter().filter(|c| !is_range(c)) {
        *count.entry((c.row, hi_col(c))).or_default() += 1;
    }
    let mut cols = Vec::new();
    let mut x = 0.0;
    for &k in keys.iter().rev() {
        let n = count
            .iter()
            .filter(|((_, col), _)| *col == k)
            .map(|((row, _), n)| (*n).min(ties_in_row(*row)))
            .max()
            .unwrap_or(0);
        let w = if n == 0 { 0.5 * CARD_W } else { n as f64 * CARD_W + (n - 1) as f64 * GAP };
        if !cols.is_empty() {
            x += GAP;
        }
        cols.push((k, x, w));
        x += w;
    }
    let inner_w = if cols.is_empty() { CARD_W } else { x.max(CARD_W) };
    let col_of = |k: Col| cols.iter().position(|(c, _, _)| *c == k).unwrap();

    let mut rows: BTreeMap<u8, Vec<Vec<(&CrateNode, f64, f64)>>> = BTreeMap::new();
    let row_set: BTreeSet<u8> = crates.iter().map(|c| c.row).collect();
    for row in row_set {
        let mut points: Vec<&CrateNode> =
            crates.iter().copied().filter(|c| c.row == row && !is_range(c)).collect();
        points.sort_by(|a, b| hi_col(b).cmp(&hi_col(a)).then(a.name.cmp(&b.name)));
        let mut lanes: Vec<Vec<(&CrateNode, f64, f64)>> = vec![Vec::new()];
        // Occupied column-index intervals per lane.
        let mut occupied: Vec<Vec<(usize, usize)>> = vec![Vec::new()];
        let mut placed_in: BTreeMap<Col, usize> = BTreeMap::new();
        let ties = ties_in_row(row);
        for c in points {
            let i = col_of(hi_col(c));
            let k = placed_in.entry(hi_col(c)).or_default();
            let lane = *k / ties;
            while lanes.len() <= lane {
                lanes.push(Vec::new());
                occupied.push(Vec::new());
            }
            lanes[lane].push((c, cols[i].1 + (*k % ties) as f64 * (CARD_W + GAP), CARD_W));
            *k += 1;
            occupied[lane].push((i, i));
        }
        let mut ranges: Vec<&CrateNode> =
            crates.iter().copied().filter(|c| c.row == row && is_range(c)).collect();
        // Widest first, then highest, then by name.
        ranges.sort_by(|a, b| {
            let wa = hi_col(a) - lo_col(a);
            let wb = hi_col(b) - lo_col(b);
            wb.cmp(&wa).then(hi_col(b).cmp(&hi_col(a))).then(a.name.cmp(&b.name))
        });
        for c in ranges {
            let (s, e) = (col_of(hi_col(c)), col_of(lo_col(c)));
            let free = |lane: &Vec<(usize, usize)>| lane.iter().all(|(a, b)| e < *a || s > *b);
            let lane = match occupied.iter().position(free) {
                Some(l) => l,
                None => {
                    occupied.push(Vec::new());
                    lanes.push(Vec::new());
                    occupied.len() - 1
                }
            };
            occupied[lane].push((s, e));
            let x0 = cols[s].1;
            let x1 = cols[e].1 + cols[e].2;
            lanes[lane].push((c, x0, x1 - x0));
        }
        // Lane 0 can be empty when a row holds only ranges; drop empty lanes.
        lanes.retain(|l| !l.is_empty());
        rows.insert(row, lanes);
    }
    TopicBox { topic, cols, inner_w, rows }
}

/// Lay out `map`. See the module docs for the rules.
pub fn layout(map: &CodeMap) -> Layout {
    let by_topic = |t: Topic| -> Vec<&CrateNode> { map.crates.iter().filter(|c| c.topic == t).collect() };
    let apps = by_topic(Topic::App);
    let utilities = by_topic(Topic::Utility);
    let km = by_topic(Topic::KnowledgeManagement);
    let boxes: Vec<TopicBox> = Topic::COLUMNS.iter().map(|&t| topic_box(t, &by_topic(t))).collect();

    // Topic boxes wrap onto bands of BOXES_PER_BAND; each band has its own
    // rows (3, 2 and any other a crate of that band uses), high to low.
    let bands: Vec<&[TopicBox]> = boxes.chunks(BOXES_PER_BAND).collect();
    let band_rows: Vec<BTreeSet<u8>> = bands
        .iter()
        .map(|band| {
            let mut r: BTreeSet<u8> = [3, 2].into();
            for b in band.iter() {
                r.extend(b.rows.keys());
            }
            r
        })
        .collect();
    let topic_rows: BTreeSet<u8> = band_rows.iter().flatten().copied().collect();
    let mut base_rows: BTreeSet<u8> = [1, 0].into();
    base_rows.extend(utilities.iter().map(|c| c.row));
    base_rows.retain(|r| !topic_rows.contains(r));
    let lanes_in = |band: &[TopicBox], row: u8| -> usize {
        band.iter().filter_map(|b| b.rows.get(&row)).map(Vec::len).max().unwrap_or(0).max(1)
    };
    // Knowledge-management crates stack one per lane in their row's band.
    let km_in = |row: u8| -> usize { km.iter().filter(|c| c.row == row).count() };
    let box_w = |b: &TopicBox| ROW_TAG + b.inner_w + 2.0 * PAD;
    let band_w: Vec<f64> =
        bands.iter().map(|band| band.iter().map(box_w).sum::<f64>() + (band.len() - 1) as f64 * BOX_GAP).collect();

    let mut frames = Vec::new();
    let mut cards: BTreeMap<String, Card> = BTreeMap::new();
    let mut labels = Vec::new();
    let pyramid_w = band_w.iter().copied().fold(0.0, f64::max);
    let base_need = base_rows
        .iter()
        .map(|r| {
            let n = utilities.iter().filter(|c| c.row == *r).count();
            n as f64 * CARD_W + n.saturating_sub(1) as f64 * GAP + 2.0 * PAD
        })
        .fold(0.0, f64::max);
    let base_w = pyramid_w.max(base_need);

    // Vertical.
    let root = Rect { x: 0.5 * base_w - 0.5 * ROOT_W, y: 0.0, w: ROOT_W, h: ROOT_H };
    let app_top = root.bottom() + 2.0 * BOX_GAP;
    let app_box_h = HEADER + CARD_H + PAD;
    // (band, row) -> (top y, height); the first band's rows also serve the
    // knowledge-management box.
    let mut band_y: BTreeMap<(usize, u8), (f64, f64)> = BTreeMap::new();
    let mut band_top = app_top + app_box_h + 2.0 * BOX_GAP;
    let mut band_span = Vec::new();
    for (i, band) in bands.iter().enumerate() {
        let mut y = band_top + HEADER;
        for &r in band_rows[i].iter().rev() {
            let n = if i == 0 { lanes_in(band, r).max(km_in(r)) } else { lanes_in(band, r) };
            let h = n as f64 * CARD_H + (n - 1) as f64 * GAP;
            band_y.insert((i, r), (y, h));
            y += h + BAND_GAP;
        }
        let bottom = y - BAND_GAP + PAD;
        band_span.push((band_top, bottom));
        band_top = bottom + 2.0 * BOX_GAP;
    }
    let topic_bottom = band_span.last().map(|s| s.1).unwrap_or(band_top);
    let base_top = topic_bottom + 2.0 * BOX_GAP;
    let mut y = base_top + HEADER;
    let mut base_y: BTreeMap<u8, f64> = BTreeMap::new();
    for &r in base_rows.iter().rev() {
        base_y.insert(r, y);
        labels.push(Label { x: -0.5 * GUTTER, y: y + 0.5 * CARD_H, text: format!("row {r}"), tip: row_meaning(r).into() });
        let n = km_in(r).max(1);
        y += n as f64 * CARD_H + (n - 1) as f64 * GAP + BAND_GAP;
    }
    let base_bottom = y - BAND_GAP + PAD;
    labels.push(Label { x: -0.5 * GUTTER, y: app_top + HEADER + 0.5 * CARD_H, text: "row 4".into(), tip: row_meaning(4).into() });

    // App boxes, centred over the pyramid.
    let app_w = CARD_W + 2.0 * PAD;
    let total = apps.len() as f64 * app_w + apps.len().saturating_sub(1) as f64 * BOX_GAP;
    let mut ax = 0.5 * base_w - 0.5 * total;
    for c in &apps {
        let rect = Rect { x: ax, y: app_top, w: app_w, h: app_box_h };
        frames.push(Frame { kind: FrameKind::App, topic: Topic::App, title: Topic::App.title().into(), rect });
        let card = Rect { x: ax + PAD, y: app_top + HEADER, w: CARD_W, h: CARD_H };
        cards.insert(c.name.clone(), Card { name: c.name.clone(), rect: card, frame: frames.len() - 1 });
        ax += app_w + BOX_GAP;
    }

    // Topic boxes, band by band, each band centred.
    for (i, band) in bands.iter().enumerate() {
        let (top, bottom) = band_span[i];
        let mut bx = 0.5 * base_w - 0.5 * band_w[i];
        for b in band.iter() {
            let rect = Rect { x: bx, y: top, w: box_w(b), h: bottom - top };
            frames.push(Frame { kind: FrameKind::Topic, topic: b.topic, title: b.topic.title().into(), rect });
            let f = frames.len() - 1;
            for &r in &band_rows[i] {
                let (y0, h) = band_y[&(i, r)];
                labels.push(Label { x: bx + PAD + 0.5 * ROW_TAG - 4.0, y: y0 + 0.5 * h, text: format!("{r}"), tip: row_meaning(r).into() });
            }
            let left = bx + PAD + ROW_TAG;
            for (k, cx, w) in &b.cols {
                let text = if *k < 0 { "F?".to_string() } else { format!("F{k}") };
                labels.push(Label { x: left + cx + 0.5 * w, y: top + 34.0, text, tip: fidelity_meaning(*k).into() });
            }
            for (row, lanes) in &b.rows {
                let (y0, _) = band_y[&(i, *row)];
                for (j, lane) in lanes.iter().enumerate() {
                    for (c, cx, w) in lane {
                        let r = Rect { x: left + cx, y: y0 + j as f64 * (CARD_H + GAP), w: *w, h: CARD_H };
                        cards.insert(c.name.clone(), Card { name: c.name.clone(), rect: r, frame: f });
                    }
                }
            }
            bx += box_w(b) + BOX_GAP;
        }
    }

    // Utilities base.
    let base = Rect { x: 0.0, y: base_top, w: base_w, h: base_bottom - base_top };
    frames.push(Frame { kind: FrameKind::Utilities, topic: Topic::Utility, title: Topic::Utility.title().into(), rect: base });
    let f = frames.len() - 1;
    for &r in &base_rows {
        let row: Vec<&&CrateNode> = utilities.iter().filter(|c| c.row == r).collect();
        let w = row.len() as f64 * CARD_W + row.len().saturating_sub(1) as f64 * GAP;
        let mut cx = 0.5 * base_w - 0.5 * w;
        for c in row {
            let rect = Rect { x: cx, y: base_y[&r], w: CARD_W, h: CARD_H };
            cards.insert(c.name.clone(), Card { name: c.name.clone(), rect, frame: f });
            cx += CARD_W + GAP;
        }
    }

    // Knowledge management: one card wide, left of the row gutter, full height.
    let km_rows: BTreeSet<u8> = km.iter().map(|c| c.row).collect();
    let km_w = CARD_W + 2.0 * PAD;
    let km_x = -GUTTER - BOX_GAP - km_w;
    let km_rect = Rect { x: km_x, y: app_top, w: km_w, h: base_bottom - app_top };
    frames.push(Frame {
        kind: FrameKind::KnowledgeManagement,
        topic: Topic::KnowledgeManagement,
        title: Topic::KnowledgeManagement.title().into(),
        rect: km_rect,
    });
    let f = frames.len() - 1;
    for &r in &km_rows {
        let row: Vec<&&CrateNode> = km.iter().filter(|c| c.row == r).collect();
        // A row with no band of its own (row 4 is the app band) still has one.
        let mut y = band_y.get(&(0, r)).map(|b| b.0).or_else(|| base_y.get(&r).copied()).unwrap_or(app_top + HEADER);
        for c in row {
            let rect = Rect { x: km_x + PAD, y, w: CARD_W, h: CARD_H };
            cards.insert(c.name.clone(), Card { name: c.name.clone(), rect, frame: f });
            y += CARD_H + GAP;
        }
    }

    let cards: Vec<Card> = map.crates.iter().filter_map(|c| cards.remove(&c.name)).collect();
    let gutter = Rect { x: -GUTTER, y: 0.0, w: GUTTER, h: 1.0 };
    let bounds = frames.iter().map(|f| f.rect).fold(root.union(&gutter), |a, b| a.union(&b));
    Layout { root, frames, cards, labels, bounds }
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;

    fn map() -> CodeMap {
        CodeMap::from_cargo_metadata(&fixture::json()).unwrap()
    }

    /// The checks every layout must pass, shared with the real-workspace
    /// test in `tests/code_map_tags.rs` through [`check`].
    #[test]
    fn the_fixture_layout_passes_every_check() {
        let m = map();
        let l = layout(&m);
        let problems = check(&m, &l);
        assert!(problems.is_empty(), "{problems:#?}");
    }

    #[test]
    fn raffles_spans_the_whole_risk_box_and_changi_part_of_it() {
        let m = map();
        let l = layout(&m);
        let risk = l.frames.iter().find(|f| f.topic == Topic::Risk).unwrap().rect;
        let raffles = l.card("raffles").unwrap().rect;
        assert!((raffles.x - (risk.x + PAD + ROW_TAG)).abs() < 1e-9);
        assert!((raffles.right() - (risk.right() - PAD)).abs() < 1e-9);
        let changi = l.card("changi").unwrap().rect;
        assert!(changi.w > CARD_W && changi.w < raffles.w);
        // changi [1, 3]: from the F3 column (pflotran, redhill) to F1 (buangkok).
        assert!((changi.x - l.card("pflotran").unwrap().rect.x).abs() < 1e-9);
        assert!((changi.right() - l.card("buangkok").unwrap().rect.right()).abs() < 1e-9);
    }

    /// Methodology: lay out the fixture (two topics with crates, six
    /// empty) and check the wrap: two bands of four boxes, the second band
    /// below the first, rows aligned within a band, every row and fidelity
    /// label carrying its meaning.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn topic_boxes_wrap_onto_two_bands_with_rows_aligned() {
        let m = map();
        let l = layout(&m);
        let topics: Vec<&Frame> = l.frames.iter().filter(|f| f.kind == FrameKind::Topic).collect();
        assert_eq!(topics.len(), 8);
        let tops: BTreeSet<i64> = topics.iter().map(|f| f.rect.y as i64).collect();
        assert_eq!(tops.len(), 2, "two bands");
        for (i, f) in topics.iter().enumerate() {
            assert_eq!(f.topic, Topic::COLUMNS[i], "order kept");
        }
        assert!(topics[4].rect.y > topics[0].rect.bottom());
        assert!(l.labels.iter().all(|x| !x.tip.is_empty()));
        assert!(l.labels.iter().any(|x| x.tip == "Row 3: coupled multiphysics"));
        assert!(l.labels.iter().any(|x| x.tip == "F4: brute force (Monte Carlo, first-principles data)"));
        assert!(check(&m, &l).is_empty(), "{:#?}", check(&m, &l));
    }

    /// Row 2 stacks crates of one fidelity level (gh: maintainer,
    /// 2026-10-08): the fixture's two row-2 F3 crates in the Risk box,
    /// pflotran and redhill, share a column and sit one above the other.
    #[test]
    fn row_2_ties_stack_one_above_another() {
        let m = map();
        let l = layout(&m);
        let (a, b) = (l.card("pflotran").unwrap().rect, l.card("redhill").unwrap().rect);
        assert!((a.x - b.x).abs() < 1e-9, "same column");
        assert!((a.y - b.y).abs() >= CARD_H, "stacked");
        assert_eq!(ties_in_row(2), 1);
        assert_eq!(ties_in_row(4), MAX_TIES);
        assert!(check(&m, &l).is_empty(), "{:#?}", check(&m, &l));
    }

    /// Knowledge management is one column left of the row gutter
    /// (maintainer, 2026-10-08): every one of its cards shares one x, the
    /// box ends left of every other box, and the checks still pass.
    #[test]
    fn knowledge_management_is_one_column_on_the_left() {
        let m = map();
        let l = layout(&m);
        let km = l.frames.iter().position(|f| f.kind == FrameKind::KnowledgeManagement).unwrap();
        let xs: BTreeSet<i64> = l.cards.iter().filter(|c| c.frame == km).map(|c| c.rect.x as i64).collect();
        assert!(xs.len() <= 1, "one column: {xs:?}");
        let r = l.frames[km].rect;
        assert!(r.right() <= -GUTTER);
        assert!(l.frames.iter().filter(|f| f.kind != FrameKind::KnowledgeManagement).all(|f| f.rect.x >= 0.0));
        assert_eq!(ties_in_row(3), 1);
        assert!(check(&m, &l).is_empty(), "{:#?}", check(&m, &l));
    }

    #[test]
    fn the_layout_is_deterministic() {
        let a = serde_json::to_string(&layout(&map())).unwrap();
        let b = serde_json::to_string(&layout(&map())).unwrap();
        assert_eq!(a, b);
    }
}

/// Every rule a layout must obey, as a list of problems (empty when all
/// hold): each crate placed exactly once, inside its box; no two cards
/// overlap; nothing overlaps the root; within a topic box, a crate of
/// higher fidelity never starts to the right of one of lower fidelity; and
/// a crate of row r sits no lower on the page than one of row r - 1 in the
/// same box. Public so the real-workspace test can run it too.
pub fn check(map: &CodeMap, l: &Layout) -> Vec<String> {
    let mut p = Vec::new();
    if l.cards.len() != map.crates.len() {
        p.push(format!("{} cards for {} crates", l.cards.len(), map.crates.len()));
    }
    for c in &map.crates {
        let n = l.cards.iter().filter(|k| k.name == c.name).count();
        if n != 1 {
            p.push(format!("{} placed {n} times", c.name));
        }
    }
    for (i, a) in l.cards.iter().enumerate() {
        if a.rect.overlaps(&l.root) {
            p.push(format!("{} overlaps the root", a.name));
        }
        if !l.frames[a.frame].rect.contains(&a.rect) {
            p.push(format!("{} lies outside its box", a.name));
        }
        for b in &l.cards[i + 1..] {
            if a.rect.overlaps(&b.rect) {
                p.push(format!("{} overlaps {}", a.name, b.name));
            }
        }
    }
    for (i, f) in l.frames.iter().enumerate() {
        for g in &l.frames[i + 1..] {
            let nested = f.rect.contains(&g.rect) || g.rect.contains(&f.rect);
            if f.rect.overlaps(&g.rect) && !nested {
                p.push(format!("box {} overlaps box {}", f.title, g.title));
            }
        }
    }
    for a in &l.cards {
        for b in &l.cards {
            if a.frame != b.frame || l.frames[a.frame].kind != FrameKind::Topic {
                continue;
            }
            let (ca, cb) = (map.get(&a.name).unwrap(), map.get(&b.name).unwrap());
            if hi_col(ca) > hi_col(cb) && a.rect.x > b.rect.x + 1e-9 {
                p.push(format!("{} (higher fidelity) starts right of {}", a.name, b.name));
            }
            if ca.row > cb.row && a.rect.y > b.rect.y + 1e-9 {
                p.push(format!("{} (row {}) drawn below {} (row {})", a.name, ca.row, b.name, cb.row));
            }
        }
    }
    // Rows line up across the topic boxes of a band: the top card of a row
    // sits at the same height in every box of the band that has that row.
    let mut row_top: BTreeMap<(i64, u8), Vec<(String, f64)>> = BTreeMap::new();
    for a in &l.cards {
        let f = &l.frames[a.frame];
        if f.kind != FrameKind::Topic {
            continue;
        }
        let row = map.get(&a.name).map(|c| c.row).unwrap_or(0);
        row_top.entry((f.rect.y as i64, row)).or_default().push((f.title.clone(), a.rect.y));
    }
    for ((_, row), v) in &row_top {
        let mut by_box: BTreeMap<&str, f64> = BTreeMap::new();
        for (t, y) in v {
            let e = by_box.entry(t.as_str()).or_insert(*y);
            *e = e.min(*y);
        }
        let ys: Vec<f64> = by_box.values().copied().collect();
        if ys.iter().any(|y| (y - ys[0]).abs() > 1e-9) {
            p.push(format!("row {row} is not aligned across a band: {by_box:?}"));
        }
    }
    p
}
