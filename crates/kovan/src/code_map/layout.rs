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
//!   topic gets a box, even an empty one.
//! - Inside a topic box, **fidelity columns** run from the highest level on
//!   the left to the lowest on the right. The columns are the levels a crate
//!   in the box sits at, plus both ends of every range. A column is as wide
//!   as the most crates any one row puts at that level (ties sit side by
//!   side, sorted by name, at most [`MAX_TIES`] abreast before wrapping to
//!   a lane below); a column only a range touches is half a card.
//! - A **range** crate spans from the left of its `hi` column to the right of
//!   its `lo` column, on its own lane below the single-level crates of its
//!   row (lanes assigned greedily, widest range first, so ranges that do not
//!   overlap share one). `raffles` `[0, 4]` therefore spans the whole Risk box.
//! - Rows line up across the boxes: a row's band is as tall as the most
//!   lanes any box needs in it.
//! - The **utilities base** (rows 1, then 0) spans the pyramid's width below
//!   the topic boxes, each row's crates centred.
//! - The **knowledge-management box** stands to the right, from the app band
//!   to the bottom of the base; each of its crates sits in its own row's band.

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
        let n = count.iter().filter(|((_, col), _)| *col == k).map(|(_, n)| *n).max().unwrap_or(0);
        let n = n.min(MAX_TIES);
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
        for c in points {
            let i = col_of(hi_col(c));
            let k = placed_in.entry(hi_col(c)).or_default();
            let lane = *k / MAX_TIES;
            while lanes.len() <= lane {
                lanes.push(Vec::new());
                occupied.push(Vec::new());
            }
            lanes[lane].push((c, cols[i].1 + (*k % MAX_TIES) as f64 * (CARD_W + GAP), CARD_W));
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

    // Row bands: topic rows (3, 2 and any other a topic crate uses), then
    // the base's rows (1, 0 and any other a utility uses), high to low.
    let mut topic_rows: BTreeSet<u8> = [3, 2].into();
    for b in &boxes {
        topic_rows.extend(b.rows.keys());
    }
    let mut base_rows: BTreeSet<u8> = [1, 0].into();
    base_rows.extend(utilities.iter().map(|c| c.row));
    base_rows.retain(|r| !topic_rows.contains(r));
    let lanes_in = |row: u8| -> usize {
        boxes.iter().filter_map(|b| b.rows.get(&row)).map(Vec::len).max().unwrap_or(0).max(1)
    };

    // Horizontal: topic boxes from x = 0.
    let mut frames = Vec::new();
    let mut cards: BTreeMap<String, Card> = BTreeMap::new();
    let mut labels = Vec::new();
    let mut box_x = Vec::new();
    let mut x = 0.0;
    for b in &boxes {
        box_x.push(x);
        x += b.inner_w + 2.0 * PAD + BOX_GAP;
    }
    let pyramid_w = x - BOX_GAP;
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
    let topic_top = app_top + app_box_h + 2.0 * BOX_GAP;
    let mut band_y: BTreeMap<u8, (f64, f64)> = BTreeMap::new();
    let mut y = topic_top + HEADER;
    for &r in topic_rows.iter().rev() {
        let h = lanes_in(r) as f64 * CARD_H + (lanes_in(r) - 1) as f64 * GAP;
        band_y.insert(r, (y, h));
        y += h + BAND_GAP;
    }
    let topic_bottom = y - BAND_GAP + PAD;
    let base_top = topic_bottom + 2.0 * BOX_GAP;
    let mut y = base_top + HEADER;
    for &r in base_rows.iter().rev() {
        band_y.insert(r, (y, CARD_H));
        y += CARD_H + BAND_GAP;
    }
    let base_bottom = y - BAND_GAP + PAD;
    band_y.entry(4).or_insert((app_top + HEADER, CARD_H));

    // Row labels in the gutter.
    for (&r, &(y, h)) in band_y.iter().rev() {
        labels.push(Label { x: -0.5 * GUTTER, y: y + 0.5 * h, text: format!("row {r}") });
    }

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

    // Topic boxes.
    for (b, &bx) in boxes.iter().zip(&box_x) {
        let rect = Rect { x: bx, y: topic_top, w: b.inner_w + 2.0 * PAD, h: topic_bottom - topic_top };
        frames.push(Frame { kind: FrameKind::Topic, topic: b.topic, title: b.topic.title().into(), rect });
        let f = frames.len() - 1;
        let left = bx + PAD;
        for (k, cx, w) in &b.cols {
            let text = if *k < 0 { "F?".to_string() } else { format!("F{k}") };
            labels.push(Label { x: left + cx + 0.5 * w, y: topic_top + 34.0, text });
        }
        for (row, lanes) in &b.rows {
            let (y0, _) = band_y[row];
            for (i, lane) in lanes.iter().enumerate() {
                for (c, cx, w) in lane {
                    let r = Rect { x: left + cx, y: y0 + i as f64 * (CARD_H + GAP), w: *w, h: CARD_H };
                    cards.insert(c.name.clone(), Card { name: c.name.clone(), rect: r, frame: f });
                }
            }
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
            let rect = Rect { x: cx, y: band_y[&r].0, w: CARD_W, h: CARD_H };
            cards.insert(c.name.clone(), Card { name: c.name.clone(), rect, frame: f });
            cx += CARD_W + GAP;
        }
    }

    // Knowledge management, to the right, full height.
    let km_rows: BTreeSet<u8> = km.iter().map(|c| c.row).collect();
    let most = km_rows.iter().map(|r| km.iter().filter(|c| c.row == *r).count()).max().unwrap_or(1).max(1);
    let km_w = most as f64 * CARD_W + (most - 1) as f64 * GAP + 2.0 * PAD;
    let km_x = base_w + 2.0 * BOX_GAP;
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
        let w = row.len() as f64 * CARD_W + (row.len() - 1) as f64 * GAP;
        let mut cx = km_x + 0.5 * km_w - 0.5 * w;
        // A row with no band of its own (row 4 is the app band) still has one.
        let y = band_y.get(&r).map(|b| b.0).unwrap_or(app_top + HEADER);
        for c in row {
            let rect = Rect { x: cx, y, w: CARD_W, h: CARD_H };
            cards.insert(c.name.clone(), Card { name: c.name.clone(), rect, frame: f });
            cx += CARD_W + GAP;
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
        assert!((raffles.x - (risk.x + PAD)).abs() < 1e-9);
        assert!((raffles.right() - (risk.right() - PAD)).abs() < 1e-9);
        let changi = l.card("changi").unwrap().rect;
        assert!(changi.w > CARD_W && changi.w < raffles.w);
        // changi [1, 3]: from the F3 column (pflotran, redhill) to F1 (buangkok).
        assert!((changi.x - l.card("pflotran").unwrap().rect.x).abs() < 1e-9);
        assert!((changi.right() - l.card("buangkok").unwrap().rect.right()).abs() < 1e-9);
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
    p
}
