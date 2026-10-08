//! The code map as a standalone SVG, for the static site (GitHub #734).
//!
//! Plain string building from [`super::layout::Layout`]: no GUI library and
//! no new dependency. Every number is printed with one decimal, nothing
//! depends on time, locale or iteration order, so the same [`CodeMap`] gives
//! byte-identical output (tested).
//!
//! What it draws, back to front: the boxes and their titles, the row and
//! fidelity labels, the required dependency edges (faint curves, the same
//! edge-to-edge Bézier as the mind map's connectors,
//! [`crate::mindmap_view::connector_sized`]), then the root and one card per
//! crate. A card shows the crate's name, its maturity (a numbered badge and
//! its label, `"2 · parts at 3"` when modules are rated higher) and its
//! fidelity; maturity-0 crates are greyed and dashed. Each card is a
//! `<g class="card" data-crate="…">` with a `<title>` tooltip (row, topic,
//! fidelity, maturity, the higher-rated modules with their reasons, the
//! description), and each edge a `<path class="edge" data-from data-to>`, so
//! a page can highlight a crate's edges with a few lines of script.
//!
//! Colours are CSS classes in the SVG's own `<style>`, with a dark variant
//! under `prefers-color-scheme: dark`, so the file reads in either theme
//! whether it is inlined or shown as an `<img>`.

use std::fmt::Write;

use super::layout::{FrameKind, Layout, Rect};
use super::{maturity_label, CodeMap, CrateNode, Fidelity, Topic};
use crate::geometry::Point;

/// Escape text for XML content and attribute values.
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c => o.push(c),
        }
    }
    o
}

/// The topic's colour (the card's left strip and its box's border), as hex.
pub fn topic_colour(t: Topic) -> &'static str {
    match t {
        Topic::App => "#475569",
        Topic::Neutronics => "#3b6fd4",
        Topic::ThermalHydraulics => "#d4553b",
        Topic::FuelPerformance => "#c08a1a",
        Topic::StructuralMechanics => "#6b7280",
        Topic::Chemistry => "#1f9d8f",
        Topic::FuelCycle => "#8e5bd1",
        Topic::GranularDem => "#a0522d",
        Topic::Risk => "#c2185b",
        Topic::Utility => "#5f7d8c",
        Topic::KnowledgeManagement => "#38803c",
    }
}

/// The maturity badge's colour, 0 (grey) to 4 (deep green).
pub fn maturity_colour(level: u8) -> &'static str {
    match level {
        0 => "#9e9e9e",
        1 => "#e0802a",
        2 => "#c9a400",
        3 => "#7cb342",
        _ => "#2e7d32",
    }
}

/// The card's second line: `"M2 AI V&V · F3"`.
pub fn card_subtitle(c: &CrateNode) -> String {
    let m = if c.maturity_modules.is_empty() {
        format!("M{} {}", c.maturity, maturity_label(c.maturity))
    } else {
        format!("M{}", c.maturity_summary())
    };
    match c.fidelity {
        Some(f) => format!("{m} \u{b7} {}", f.label()),
        None => m,
    }
}

/// The hover text: row, topic, fidelity, maturity, higher-rated modules and
/// the description, one per line.
pub fn tooltip(map: &CodeMap, c: &CrateNode) -> String {
    let mut t = format!("{}\nrow {} \u{b7} topic {}", c.name, c.row, c.topic.as_str());
    match c.fidelity {
        Some(Fidelity::Level(l)) => {
            let _ = write!(t, "\nfidelity {l} ({})", Fidelity::level_meaning(l));
        }
        Some(Fidelity::Range(lo, hi)) => {
            let _ = write!(
                t,
                "\nfidelity {lo}\u{2013}{hi} ({} to {})",
                Fidelity::level_meaning(lo),
                Fidelity::level_meaning(hi)
            );
        }
        None => {}
    }
    let _ = write!(t, "\nmaturity {} ({})", c.maturity, maturity_label(c.maturity));
    if !c.maturity_modules.is_empty() {
        t.push_str("\nparts rated higher:");
        for m in &c.maturity_modules {
            let _ = write!(t, "\n  {} at {} ({}): {}", m.module, m.level, maturity_label(m.level), m.why);
        }
    }
    let deps = map.dependencies(&c.name);
    if !deps.is_empty() {
        let _ = write!(t, "\ndepends on: {}", deps.join(", "));
    }
    if let Some(d) = &c.description {
        let _ = write!(t, "\n{d}");
    }
    t
}

fn n(v: f64) -> String {
    // One decimal, and never "-0.0".
    let s = format!("{v:.1}");
    if s == "-0.0" {
        "0.0".into()
    } else {
        s
    }
}

const STYLE: &str = r#"
.bg{fill:#ffffff}
.frame{fill:#f6f7f9;stroke-width:1.6}
.frame-title{font-size:15px;font-weight:700;letter-spacing:.06em;fill:#334155}
.label{font-size:12px;fill:#64748b}
.row-label{font-size:13px;font-weight:600;fill:#64748b}
.edge{fill:none;stroke:#64748b;stroke-opacity:.22;stroke-width:1.2}
.card-bg{fill:#ffffff;stroke-width:1.2}
.name{font-weight:700;fill:#0f172a}
.sub{fill:#475569}
.badge-text{font-size:12px;font-weight:700;fill:#ffffff}
.m0{opacity:.5}
.m0 .card-bg{stroke-dasharray:5 4}
.root-bg{fill:#1e293b}
.root-text{font-size:22px;font-weight:700;fill:#f8fafc}
@media (prefers-color-scheme: dark){
.bg{fill:#0f141b}
.frame{fill:#161d27}
.frame-title{fill:#cbd5e1}
.label,.row-label{fill:#94a3b8}
.edge{stroke:#94a3b8;stroke-opacity:.25}
.card-bg{fill:#1c2430}
.name{fill:#f1f5f9}
.sub{fill:#cbd5e1}
.root-bg{fill:#e2e8f0}
.root-text{fill:#0f172a}
}
"#;

fn card(out: &mut String, map: &CodeMap, c: &CrateNode, r: &Rect) {
    let colour = topic_colour(c.topic);
    let _ = write!(
        out,
        "<g class=\"card m{m}\" data-crate=\"{name}\"><title>{title}</title>\
         <rect class=\"card-bg\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"6.0\" stroke=\"{colour}\"/>\
         <rect x=\"{x}\" y=\"{y}\" width=\"6.0\" height=\"{h}\" rx=\"3.0\" fill=\"{colour}\"/>",
        m = c.maturity,
        name = escape(&c.name),
        title = escape(&tooltip(map, c)),
        x = n(r.x),
        y = n(r.y),
        w = n(r.w),
        h = n(r.h),
    );
    let badge = 20.0;
    let (name, size) = super::fit_label(&c.name, r.w - 22.0, 14.0, 11.0);
    let _ = write!(
        out,
        "<text class=\"name\" x=\"{}\" y=\"{}\" font-size=\"{}\">{}</text>",
        n(r.x + 14.0),
        n(r.y + 23.0),
        n(size),
        escape(&name)
    );
    let (sub, size) = super::fit_label(&card_subtitle(c), r.w - 22.0 - badge - 8.0, 11.5, 10.0);
    let _ = write!(
        out,
        "<text class=\"sub\" x=\"{}\" y=\"{}\" font-size=\"{}\">{}</text>",
        n(r.x + 14.0),
        n(r.y + 44.0),
        n(size),
        escape(&sub)
    );
    let (bx, by) = (r.right() - badge - 7.0, r.bottom() - badge - 6.0);
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"10.0\" fill=\"{}\"/>\
         <text class=\"badge-text\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text></g>\n",
        n(bx),
        n(by),
        n(badge),
        n(badge),
        maturity_colour(c.maturity),
        n(bx + 0.5 * badge),
        n(by + 14.5),
        c.maturity
    );
}

/// The SVG document for `map` laid out as `layout`.
pub fn render(map: &CodeMap, layout: &Layout) -> String {
    let b = layout.bounds;
    let m = 24.0;
    let (vx, vy, vw, vh) = (b.x - m, b.y - m, b.w + 2.0 * m, b.h + 2.0 * m);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" width=\"{}\" height=\"{}\" \
         font-family=\"system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif\" role=\"img\" \
         aria-label=\"Code map of {}: every crate by row, topic and fidelity, with its maturity and required dependencies\">",
        n(vx),
        n(vy),
        n(vw),
        n(vh),
        n(vw),
        n(vh),
        escape(&map.root)
    );
    let _ = writeln!(out, "<style>{STYLE}</style>");
    let _ = writeln!(
        out,
        "<rect class=\"bg\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
        n(vx),
        n(vy),
        n(vw),
        n(vh)
    );

    out.push_str("<g class=\"frames\">\n");
    for f in &layout.frames {
        let r = f.rect;
        let colour = topic_colour(f.topic);
        let _ = writeln!(
            out,
            "<g class=\"frame-{}\"><rect class=\"frame\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"10.0\" stroke=\"{colour}\"/>\
             <text class=\"frame-title\" x=\"{}\" y=\"{}\"{}>{}</text></g>",
            match f.kind {
                FrameKind::App => "app",
                FrameKind::Topic => "topic",
                FrameKind::Utilities => "utilities",
                FrameKind::KnowledgeManagement => "km",
            },
            n(r.x),
            n(r.y),
            n(r.w),
            n(r.h),
            n(r.x + 0.5 * r.w),
            n(r.y + 20.0),
            " text-anchor=\"middle\"",
            escape(&f.title)
        );
    }
    out.push_str("</g>\n<g class=\"labels\">\n");
    for l in &layout.labels {
        let class = if l.text.starts_with('F') { "label" } else { "row-label" };
        let title = if l.tip.is_empty() { String::new() } else { format!("<title>{}</title>", escape(&l.tip)) };
        let _ = writeln!(
            out,
            "<text class=\"{class}\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"middle\">{title}{}</text>",
            n(l.x),
            n(l.y),
            escape(&l.text)
        );
    }

    out.push_str("</g>\n<g class=\"edges\">\n");
    for e in &map.edges {
        let (Some(a), Some(z)) = (layout.card(&e.from), layout.card(&e.to)) else {
            continue;
        };
        let (ax, ay) = a.rect.centre();
        let (zx, zy) = z.rect.centre();
        let Some(c) = crate::mindmap_view::connector_sized(
            Point::new(ax, ay),
            (a.rect.w, a.rect.h),
            Point::new(zx, zy),
            (z.rect.w, z.rect.h),
        ) else {
            continue;
        };
        let _ = writeln!(
            out,
            "<path class=\"edge\" data-from=\"{}\" data-to=\"{}\" d=\"M{} {} C{} {} {} {} {} {}\"/>",
            escape(&e.from),
            escape(&e.to),
            n(c[0].x),
            n(c[0].y),
            n(c[1].x),
            n(c[1].y),
            n(c[2].x),
            n(c[2].y),
            n(c[3].x),
            n(c[3].y)
        );
    }

    out.push_str("</g>\n<g class=\"cards\">\n");
    let r = layout.root;
    let _ = writeln!(
        out,
        "<g class=\"root\"><rect class=\"root-bg\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"10.0\"/>\
         <text class=\"root-text\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text></g>",
        n(r.x),
        n(r.y),
        n(r.w),
        n(r.h),
        n(r.x + 0.5 * r.w),
        n(r.y + 0.5 * r.h),
        escape(&map.root)
    );
    for k in &layout.cards {
        if let Some(c) = map.get(&k.name) {
            card(&mut out, map, c, &k.rect);
        }
    }
    out.push_str("</g>\n</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::super::{fixture, layout::layout, CodeMap};
    use super::*;

    #[test]
    fn the_same_map_gives_byte_identical_svg() {
        let a = {
            let m = CodeMap::from_cargo_metadata(&fixture::json()).unwrap();
            render(&m, &layout(&m))
        };
        let b = {
            let m = CodeMap::from_cargo_metadata(&fixture::json()).unwrap();
            render(&m, &layout(&m))
        };
        assert_eq!(a, b);
    }

    #[test]
    fn every_crate_and_edge_is_drawn_with_its_tooltip() {
        let m = CodeMap::from_cargo_metadata(&fixture::json()).unwrap();
        let s = render(&m, &layout(&m));
        for c in &m.crates {
            assert!(s.contains(&format!("data-crate=\"{}\"", c.name)), "{}", c.name);
        }
        assert_eq!(s.matches("class=\"edge\"").count(), m.edges.len());
        assert!(s.contains("class=\"card m0\" data-crate=\"redhill\""));
        assert!(s.contains("region_1 at 3 (human reviewed): reviewed"));
        assert!(s.contains("M2 \u{b7} parts at 3 \u{b7} F0"));
    }

    /// The fixture's SVG, pinned by length and an FNV-1a hash (re-pinned
    /// 2026-10-06 for the two-band layout and the label tooltips, #734;
    /// re-pinned 2026-10-08 for row 2 stacking its ties, which moves the
    /// fixture's redhill under pflotran, and again the same day for row 3
    /// stacking and knowledge management as a column on the left, each
    /// looked at in the real map): a
    /// change here is a change to the drawing, to be looked at, not only
    /// re-pinned.
    #[test]
    fn the_fixture_svg_is_pinned() {
        let m = CodeMap::from_cargo_metadata(&fixture::json()).unwrap();
        let s = render(&m, &layout(&m));
        let h = s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
        assert!(s.contains("<title>Row 3: coupled multiphysics</title>3</text>"));
        assert_eq!((s.len(), format!("{h:016x}")), (PINNED_LEN, PINNED_HASH.to_string()), "SVG changed");
    }

    const PINNED_LEN: usize = 20041;
    const PINNED_HASH: &str = "4ac0fef1bae71c48";

    #[test]
    fn text_is_escaped() {
        assert_eq!(escape("a<b & \"c\""), "a&lt;b &amp; &quot;c&quot;");
    }
}
