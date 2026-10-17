//! **Line plots and map slices as PNG**, drawn on
//! [`ImageData`] and encoded by its own `to_png_bytes` (gh #941). No
//! plotting or image crate: lines are a DDA, text is outram-blender's
//! built-in 5×7 bitmap font ([`draw_text`]; the project's own GPL-3.0 work,
//! upper case only), ticks reuse its [`nice_step`] / [`fmt_tick`].
//!
//! **Style rule (workspace memory, 2026-10):** published curves are drawn
//! thick and solid, references dashed, our own calculations small and
//! dotted, and every series' legend names where it comes from.

use outram_blender::csg::plot::annotate::{draw_text, fmt_tick, nice_step, text_width};
use outram_blender::csg::plot::colour::Rgb;
use outram_blender::csg::plot::image::ImageData;

/// How a series is drawn, by where it comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    /// A published (literature) curve: thick solid.
    #[allow(dead_code)]
    Literature,
    /// A reference calculation: dashed.
    #[allow(dead_code)]
    Reference,
    /// Our own calculation: small dots.
    Ours,
}

/// One curve.
#[derive(Debug, Clone)]
pub(crate) struct Series {
    pub label: String,
    pub style: Style,
    pub colour: Rgb,
    pub points: Vec<(f64, f64)>,
}

/// A 2-D line plot.
#[derive(Debug, Clone)]
pub(crate) struct LinePlot {
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub x_log: bool,
    pub series: Vec<Series>,
}

/// Series colours (colour-blind-safe, Okabe & Ito 2008 order).
pub(crate) const COLOURS: [Rgb; 6] = [
    Rgb {
        r: 0,
        g: 114,
        b: 178,
    },
    Rgb {
        r: 213,
        g: 94,
        b: 0,
    },
    Rgb {
        r: 0,
        g: 158,
        b: 115,
    },
    Rgb {
        r: 204,
        g: 121,
        b: 167,
    },
    Rgb {
        r: 230,
        g: 159,
        b: 0,
    },
    Rgb {
        r: 86,
        g: 180,
        b: 233,
    },
];

const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
const GRID: Rgb = Rgb {
    r: 225,
    g: 225,
    b: 225,
};
const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};

fn put(img: &mut ImageData, x: i64, y: i64, c: Rgb) {
    if x >= 0 && y >= 0 && (x as usize) < img.width && (y as usize) < img.height {
        img.set(x as usize, y as usize, c);
    }
}

fn rect(img: &mut ImageData, x0: i64, y0: i64, w: i64, h: i64, c: Rgb) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            put(img, x, y, c);
        }
    }
}

/// A DDA line from `a` to `b`; `pattern(step)` decides whether pixel
/// `step` along the run is drawn (dots and dashes), and `thick` draws a
/// 2-pixel-wide line.
fn line(img: &mut ImageData, a: (f64, f64), b: (f64, f64), c: Rgb, style: Style, run: &mut usize) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let n = dx.abs().max(dy.abs()).ceil().max(1.0) as usize;
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let (x, y) = ((a.0 + t * dx).round() as i64, (a.1 + t * dy).round() as i64);
        *run += 1;
        match style {
            Style::Literature => {
                put(img, x, y, c);
                put(img, x + 1, y, c);
                put(img, x, y + 1, c);
            }
            Style::Reference => {
                if *run % 10 < 6 {
                    put(img, x, y, c);
                }
            }
            Style::Ours => {
                if *run % 3 == 0 {
                    put(img, x, y, c);
                }
            }
        }
    }
}

/// Axis range with "nice" ticks: `(lo, hi, ticks)`.
fn linear_axis(lo: f64, hi: f64) -> (f64, f64, Vec<f64>, f64) {
    let (lo, hi) = if hi > lo {
        (lo, hi)
    } else {
        (lo - 0.5, lo + 0.5)
    };
    let step = nice_step(hi - lo, 6);
    let a = (lo / step).floor() * step;
    let b = (hi / step).ceil() * step;
    let n = ((b - a) / step).round() as usize;
    (
        (a),
        (b),
        (0..=n).map(|i| a + i as f64 * step).collect(),
        step,
    )
}

impl LinePlot {
    /// Draw it at `w` × `h` pixels.
    pub(crate) fn render(&self, w: usize, h: usize) -> ImageData {
        let mut img = ImageData::filled(w, h, WHITE);
        let (left, right, top, bottom) = (120i64, 24i64, 56i64, 96i64);
        let (pw, ph) = (w as i64 - left - right, h as i64 - top - bottom);
        draw_text(&mut img, left, 8, &self.title, BLACK);
        let pts = self
            .series
            .iter()
            .flat_map(|s| s.points.iter().copied())
            .filter(|(x, y)| x.is_finite() && y.is_finite() && (!self.x_log || *x > 0.0));
        let (mut x0, mut x1, mut y0, mut y1) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for (x, y) in pts {
            let x = if self.x_log { x.log10() } else { x };
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        if !x0.is_finite() {
            draw_text(&mut img, left, top + ph / 2, "NO DATA", BLACK);
            return img;
        }
        // x axis: decades on a log axis, nice steps on a linear one.
        let (xa, xb, xticks, xstep) = if self.x_log {
            let (a, b) = (x0.floor(), x1.ceil().max(x0.floor() + 1.0));
            let every = (((b - a) / 8.0).ceil()).max(1.0);
            let mut t = Vec::new();
            let mut v = a;
            while v <= b + 1e-9 {
                t.push(v);
                v += every;
            }
            (a, b, t, 1.0)
        } else {
            linear_axis(x0, x1)
        };
        // Start the y axis at 0 unless the data sit far above it (a k trace).
        let y_from = if y0 > 0.0 && y0 > 0.5 * y1 {
            y0
        } else {
            y0.min(0.0)
        };
        let (ya, yb, yticks, ystep) = linear_axis(y_from, y1);
        let px = |x: f64| left as f64 + (x - xa) / (xb - xa) * pw as f64;
        let py = |y: f64| (top + ph) as f64 - (y - ya) / (yb - ya) * ph as f64;
        for &t in &yticks {
            let y = py(t).round() as i64;
            rect(&mut img, left, y, pw, 1, GRID);
            rect(&mut img, left - 6, y, 6, 1, BLACK);
            let s = fmt_tick(t, ystep);
            draw_text(
                &mut img,
                left - 10 - text_width(&s) as i64,
                y - 7,
                &s,
                BLACK,
            );
        }
        for &t in &xticks {
            let x = px(t).round() as i64;
            rect(&mut img, x, top, 1, ph, GRID);
            rect(&mut img, x, top + ph, 1, 6, BLACK);
            let s = if self.x_log {
                format!("1E{}", t.round() as i64)
            } else {
                fmt_tick(t, xstep)
            };
            draw_text(
                &mut img,
                x - text_width(&s) as i64 / 2,
                top + ph + 10,
                &s,
                BLACK,
            );
        }
        rect(&mut img, left, top, pw, 1, BLACK);
        rect(&mut img, left, top + ph, pw + 1, 1, BLACK);
        rect(&mut img, left, top, 1, ph, BLACK);
        rect(&mut img, left + pw, top, 1, ph, BLACK);
        draw_text(
            &mut img,
            left + pw / 2 - text_width(&self.x_label) as i64 / 2,
            top + ph + 36,
            &self.x_label,
            BLACK,
        );
        draw_text(&mut img, 8, top - 22, &self.y_label, BLACK);
        for s in &self.series {
            let mut run = 0usize;
            let mut last: Option<(f64, f64)> = None;
            for &(x, y) in &s.points {
                if !(x.is_finite() && y.is_finite()) || (self.x_log && x <= 0.0) {
                    last = None;
                    continue;
                }
                let p = (px(if self.x_log { x.log10() } else { x }), py(y));
                match last {
                    Some(q) => line(&mut img, q, p, s.colour, s.style, &mut run),
                    None => line(&mut img, p, p, s.colour, s.style, &mut run),
                }
                last = Some(p);
            }
        }
        // Legend: top right inside the frame.
        let lw = self
            .series
            .iter()
            .map(|s| text_width(&s.label))
            .max()
            .unwrap_or(0) as i64
            + 48;
        let mut ly = top + 10;
        for s in &self.series {
            let lx = left + pw - lw - 8;
            rect(&mut img, lx - 4, ly - 4, lw + 8, 22, WHITE);
            let mut run = 0;
            line(
                &mut img,
                (lx as f64, (ly + 7) as f64),
                ((lx + 30) as f64, (ly + 7) as f64),
                s.colour,
                s.style,
                &mut run,
            );
            draw_text(&mut img, lx + 38, ly, &s.label, s.colour);
            ly += 22;
        }
        img
    }
}

/// The viridis colour map at five anchors (matplotlib's viridis, CC0),
/// linearly interpolated: `t` in `[0, 1]`.
pub(crate) fn viridis(t: f64) -> Rgb {
    const A: [(f64, f64, f64); 5] = [
        (68.0, 1.0, 84.0),
        (59.0, 82.0, 139.0),
        (33.0, 145.0, 140.0),
        (94.0, 201.0, 98.0),
        (253.0, 231.0, 37.0),
    ];
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    } * 4.0;
    let i = (t.floor() as usize).min(3);
    let f = t - i as f64;
    let (a, b) = (A[i], A[i + 1]);
    let m = |x: f64, y: f64| (x + f * (y - x)).round() as u8;
    Rgb::new(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// **A map slice**: `values[j][i]` (`i` along the horizontal axis, `j`
/// along the vertical, `j = 0` at the bottom) over `extent_cm =
/// [h_lo, h_hi, v_lo, v_hi]`, coloured by viridis from 0 to the maximum,
/// with a colour bar, cm ticks and labels. Cells with value 0 are drawn
/// light grey ("nothing scored").
pub(crate) fn heatmap(
    values: &[Vec<f64>],
    extent_cm: [f64; 4],
    title: &str,
    axes: (&str, &str),
    unit: &str,
) -> ImageData {
    let ny = values.len().max(1);
    let nx = values.first().map_or(1, Vec::len).max(1);
    let cell = (480 / nx.max(ny)).max(4) as i64;
    let (left, top) = (110i64, 56i64);
    let (pw, ph) = (cell * nx as i64, cell * ny as i64);
    let w = (left + pw + 190) as usize;
    let h = (top + ph + 90) as usize;
    let mut img = ImageData::filled(w, h, WHITE);
    draw_text(&mut img, left, 14, title, BLACK);
    let vmax = values
        .iter()
        .flatten()
        .copied()
        .filter(|v| v.is_finite())
        .fold(0.0_f64, f64::max);
    for (j, row) in values.iter().enumerate() {
        for (i, &v) in row.iter().enumerate() {
            let c = if v > 0.0 && vmax > 0.0 {
                viridis(v / vmax)
            } else {
                Rgb::new(235, 235, 235)
            };
            rect(
                &mut img,
                left + i as i64 * cell,
                top + ph - (j as i64 + 1) * cell,
                cell,
                cell,
                c,
            );
        }
    }
    rect(&mut img, left, top, pw, 1, BLACK);
    rect(&mut img, left, top + ph, pw + 1, 1, BLACK);
    rect(&mut img, left, top, 1, ph, BLACK);
    rect(&mut img, left + pw, top, 1, ph, BLACK);
    let (_, _, ht, hs) = linear_axis(extent_cm[0], extent_cm[1]);
    for t in ht
        .into_iter()
        .filter(|t| *t >= extent_cm[0] - 1e-9 && *t <= extent_cm[1] + 1e-9)
    {
        let x =
            left + ((t - extent_cm[0]) / (extent_cm[1] - extent_cm[0]) * pw as f64).round() as i64;
        rect(&mut img, x, top + ph, 1, 6, BLACK);
        let s = fmt_tick(t, hs);
        draw_text(
            &mut img,
            x - text_width(&s) as i64 / 2,
            top + ph + 10,
            &s,
            BLACK,
        );
    }
    let (_, _, vt, vs) = linear_axis(extent_cm[2], extent_cm[3]);
    for t in vt
        .into_iter()
        .filter(|t| *t >= extent_cm[2] - 1e-9 && *t <= extent_cm[3] + 1e-9)
    {
        let y = top + ph
            - ((t - extent_cm[2]) / (extent_cm[3] - extent_cm[2]) * ph as f64).round() as i64;
        rect(&mut img, left - 6, y, 6, 1, BLACK);
        let s = fmt_tick(t, vs);
        draw_text(
            &mut img,
            left - 10 - text_width(&s) as i64,
            y - 7,
            &s,
            BLACK,
        );
    }
    let xl = format!("{} [CM]", axes.0);
    draw_text(
        &mut img,
        left + pw / 2 - text_width(&xl) as i64 / 2,
        top + ph + 34,
        &xl,
        BLACK,
    );
    draw_text(&mut img, 8, top - 24, &format!("{} [CM]", axes.1), BLACK);
    // Colour bar.
    let bx = left + pw + 30;
    for k in 0..ph {
        let t = 1.0 - k as f64 / ph as f64;
        rect(&mut img, bx, top + k, 20, 1, viridis(t));
    }
    draw_text(&mut img, bx + 26, top, &format!("{vmax:.3E}"), BLACK);
    draw_text(&mut img, bx + 26, top + ph - 14, "0", BLACK);
    draw_text(&mut img, bx - 10, top + ph + 34, unit, BLACK);
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plot draws its series inside the frame, in its colour, and both
    /// renderers give a PNG that decodes back to the same pixels.
    #[test]
    fn plots_render_and_round_trip() {
        let p = LinePlot {
            title: "TEST".into(),
            x_label: "ENERGY [EV]".into(),
            y_label: "FLUX".into(),
            x_log: true,
            series: vec![Series {
                label: "OURS".into(),
                style: Style::Ours,
                colour: COLOURS[0],
                points: (0..100)
                    .map(|i| (10f64.powf(i as f64 / 10.0 - 3.0), (i as f64).sqrt()))
                    .collect(),
            }],
        };
        let img = p.render(900, 600);
        assert!(img.pixels.iter().filter(|&&c| c == COLOURS[0]).count() > 50);
        let back = outram_blender::csg::plot::image::decode_png(&img.to_png_bytes()).unwrap();
        assert_eq!(back, img);
        let lin = LinePlot {
            x_log: false,
            ..p.clone()
        }
        .render(400, 300);
        assert_eq!(lin.width, 400);
        let empty = LinePlot {
            series: vec![],
            ..p
        }
        .render(300, 200);
        assert_eq!(empty.height, 200);
        let mut r = 0;
        let mut im = ImageData::filled(20, 20, WHITE);
        for s in [Style::Literature, Style::Reference] {
            line(&mut im, (0.0, 0.0), (19.0, 19.0), BLACK, s, &mut r);
        }
        assert!(im.pixels.iter().any(|&c| c == BLACK));
        let hm = heatmap(
            &[vec![0.0, 1.0], vec![2.0, 3.0]],
            [-1.0, 1.0, -1.0, 1.0],
            "MAP",
            ("X", "Y"),
            "U",
        );
        // 2 x 2 cells of 240 px: the top-left cell is values[1][0] = 2, the
        // bottom-left values[0][0] = 0 (grey, nothing scored).
        assert_eq!(hm.get(112, 58), viridis(2.0 / 3.0));
        assert_eq!(hm.get(112, 56 + 480 - 2), Rgb::new(235, 235, 235));
        assert_eq!(viridis(0.0), Rgb::new(68, 1, 84));
        assert_eq!(viridis(1.0), Rgb::new(253, 231, 37));
    }
}
