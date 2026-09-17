//! Baked **luminance/alpha textures** for the packed pebble bed, tinted at
//! draw time.
//!
//! # Why this module exists
//!
//! The baked packing in [`crate::components::pebble_packing`] holds 523
//! pebbles, and each is drawn as a graphite body plus a TRISO speckle of
//! [`triso_dot_count`] dots. At the sizes the HTR-10 and FHR vessels are drawn
//! in their example simulators that is **19 874 and 9 880 filled circles**
//! respectively (measured 2026-08-12), *every repaint*, each one tessellated to
//! triangles by `egui` from scratch — 80 472 and 52 340 vertices per frame for
//! one widget. Both example simulators were reported laggy on the GUI thread
//! with physics exonerated; this artwork was the shared cost.
//!
//! The artwork is **static across frames** — the packing is a `const` table and
//! the speckle comes from [`triso_dot_offset`], which is deterministic by
//! construction precisely so the bed does not shimmer. Only the *colour*
//! changes, once per frame, with temperature.
//!
//! So: rasterise the shape once on the CPU, upload it, and let the colour be a
//! per-vertex tint.
//!
//! # What is baked, and what is not
//!
//! **No colour is baked.** The bake is three **luminance/alpha masks** — pure
//! coverage, no hue — and every colour arrives at draw time as a vertex tint:
//!
//! | Layer | What it covers | Tinted with |
//! |---|---|---|
//! | `backdrop` | every pixel any pebble covers | the bed backdrop colour |
//! | `matrix` | the graphite bodies, at their depth shade | the matrix colour |
//! | `kernel` | the TRISO speckle, at its depth shade | **the temperature colour** |
//!
//! Painted in that order, alpha-over-alpha, a pixel of a matrix body composites
//! to `matrix * shade + backdrop * (1 - shade)` and a speckle pixel to
//! `kernel * shade + backdrop * (1 - shade)` — algebraically identical to the
//! `blend_rgb(BED_BACKDROP, colour, shade)` the direct-circle path computes per
//! shape. Temperature therefore **never invalidates the cache**: it is a vertex
//! colour, not a texel.
//!
//! # Axial nodalisation is built in, not deferred
//!
//! The kernel layer is painted through [`BedTint`], which is either one colour
//! for the whole bed or a **vertical strip of quads with per-vertex colours
//! interpolated up the bed** ([`BedTint::Axial`]). A future axially nodalised
//! bed (15–25 zones) passes one colour per node and gets a smooth gradient with
//! no change here and no change to the bake. Nothing in this module assumes the
//! bed is one uniform colour.
//!
//! # Cache key: physical pixels
//!
//! [`BedBakeKey`] is keyed on the bed's size in **physical pixels**
//! (`points * pixels_per_point`), the packing scale, and the crop window — not
//! on any colour, and not on the widget's position. So panning is free, changing
//! temperature is free, and the bake is repeated only on a resize or a DPI /
//! monitor change.
//!
//! # The tradeoff, stated plainly
//!
//! A texture is raster where the circle path is vector. Consequences:
//!
//! - **During a resize drag the bed is re-baked every frame** at the new pixel
//!   size. The bake is CPU rasterisation of the whole bed — measured at 2.4 ms
//!   for a 232x353 px HTR-10 bed on 2026-08-12 — and is not free, so a resize
//!   drag is measurably less smooth than a static view. The direct path was
//!   paying 1.6 ms of shape-building and tessellation on *every* frame, resize
//!   or not, so a bake pays for itself after two frames at one size.
//! - **The quad is snapped to whole physical pixels** at draw time
//!   ([`snap_to_physical_pixels`]) so that in the steady state one texel maps to
//!   one pixel and the artwork is as crisp as the vector path. Sub-pixel
//!   scrolling of the vessel would resample it; the vessel widgets do not
//!   scroll.
//! - **Small beds keep the vector path.** Below the speckle threshold (a drawn
//!   pebble radius under [`crate::components::htr10_reactor_vessel`]'s
//!   single-dot cutoff) the bed is at most ~1000 circles, which was never the
//!   problem, and vector output stays crisp at thumbnail sizes where resampling
//!   would hurt most. See [`BedDrawStrategy`].
//! - **Memory.** Three RGBA8 images of the bed's pixel box. For the HTR-10
//!   vessel at its example size (a 190x400 pt box, giving a 232x353 px bed at 2x
//!   device pixel ratio) that is 983 kB in total, freed when the cache entry is
//!   replaced.
//!
//! # Accuracy against the direct-circle path
//!
//! Interiors are exact (see the composite algebra above). The one approximation
//! is the anti-aliased **outer fringe**: the direct path feathers the finished
//! colour `blend_rgb(backdrop, colour, shade)` against whatever is behind the
//! bed, while this path feathers the backdrop and the tint as two separate
//! alpha layers. The two differ by at most `backdrop * shade / 4`, i.e. under
//! 4/255 of one channel, on the single-pixel rim of each pebble. Verified in
//! [`tests::composite_matches_the_direct_blend_on_interiors`].

use egui::epaint::{Mesh, Vertex};
use egui::{Color32, ColorImage, Context, Id, Painter, Pos2, Rect, TextureHandle, TextureOptions};

use crate::components::htr10_reactor_vessel::{
    depth_shade, draw_packed_pebbles_direct, triso_dot_count, triso_dot_offset, triso_dot_radius,
    PackingTransform, PackingWindow, VerticalSense,
};
use crate::components::pebble_packing::{PackedPebble, PACKED_PEBBLES, SPHERE_RADIUS};

/// How the packed bed is put on screen this frame.
///
/// An enum rather than a trait object, per this workspace's Rust design rules:
/// the set of strategies is closed and known at compile time, so a `match` on
/// it is exhaustive and adding a third strategy would be a compile error at
/// every site rather than a runtime surprise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BedDrawStrategy {
    /// Rasterise once into [`BakedPebbleBed`] and paint tinted quads.
    ///
    /// Chosen when each pebble carries a real TRISO speckle, which is where the
    /// per-frame circle count runs into the tens of thousands.
    Baked,
    /// Paint every circle every frame — the reference path.
    ///
    /// Chosen when the bed is too small to speckle (at most one dot per pebble,
    /// so about 1000 circles), when the pixel box would exceed the backend's
    /// maximum texture side, or when the geometry is degenerate. Vector output
    /// also stays crisp at thumbnail sizes, where a resampled texture would look
    /// worst.
    Direct,
}

/// Picks the strategy for a bed whose pebbles draw at `radius_pt` points and
/// whose pixel box is `size_px`, given the backend's `max_texture_side`.
///
/// `radius_pt` is the drawn pebble radius in **points** (not pixels): the dot
/// count is a function of the point radius, so that is what decides whether a
/// speckle exists at all.
pub(crate) fn choose_strategy(
    radius_pt: f32,
    size_px: [usize; 2],
    max_texture_side: usize,
) -> BedDrawStrategy {
    let speckled = triso_dot_count(radius_pt) >= 2;
    let fits = size_px[0] > 0
        && size_px[1] > 0
        && size_px[0] <= max_texture_side
        && size_px[1] <= max_texture_side;
    if speckled && fits {
        BedDrawStrategy::Baked
    } else {
        BedDrawStrategy::Direct
    }
}

/// The colour the TRISO speckle is tinted with, uniform or graded up the bed.
///
/// **This is the temperature-response seam.** The kernel layer is the only part
/// of the bed that tracks temperature, and this is how the caller supplies it.
/// Nothing is baked: whichever variant is used, the colour is a vertex
/// attribute applied at draw time, so a temperature change costs no re-bake.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BedTint {
    /// One colour for the whole bed — the right thing for a lumped bed with a
    /// single fuel temperature, which is what both example simulators feed
    /// today.
    Uniform(Color32),
    /// One colour per axial node, interpolated up the bed.
    ///
    /// Colours run **from the bottom of the bed in the packing's own frame**
    /// (the dense settled base) to its top (the free surface), evenly spaced
    /// over the drawn bed's height. Which screen end that is depends on
    /// [`VerticalSense`] — a buoyant FHR bed is drawn inverted, and this type
    /// does not need the caller to know that.
    ///
    /// Intended for an axially nodalised bed of roughly 15–25 zones: pass one
    /// colour per node and the bed is painted as a vertical strip of quads whose
    /// per-vertex colours interpolate between adjacent nodes. An empty list or a
    /// single colour degrades to a uniform tint rather than failing.
    ///
    /// **No widget constructs this yet** — both vessels feed a single lumped
    /// fuel temperature today. It is built and tested now (see
    /// `tests::an_axial_tint_grades_up_the_bed_and_respects_buoyancy`) so that
    /// nodalising a bed later is a change at the *call site* only, with nothing
    /// to unpick here. Hence the `dead_code` allowance: the seam is deliberate,
    /// not leftover.
    #[allow(dead_code)]
    Axial(Vec<Color32>),
}

impl BedTint {
    /// The colour at dimensionless height `f` up the bed, `0.0` at the packing
    /// frame's bottom and `1.0` at its top.
    ///
    /// Used by the direct-circle path so that both strategies agree, and by the
    /// degenerate cases of the baked path. Values outside `[0, 1]` clamp.
    pub(crate) fn sample(&self, f: f32) -> Color32 {
        match self {
            Self::Uniform(colour) => *colour,
            Self::Axial(colours) => match colours.len() {
                0 => Color32::WHITE,
                1 => colours[0],
                n => {
                    let position = f.clamp(0.0, 1.0) * (n - 1) as f32;
                    let lower = position.floor() as usize;
                    let upper = (lower + 1).min(n - 1);
                    lerp_colour(colours[lower], colours[upper], position - lower as f32)
                }
            },
        }
    }
}

/// Linear interpolation between two colours in premultiplied byte space,
/// including alpha.
fn lerp_colour(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

/// Axis-aligned bounding box of the drawn bed, in the packing's own normalised
/// vessel frame (barrel inner radius `R = 1`).
///
/// Kept in packing coordinates, not screen points, so that moving the widget
/// does not invalidate the bake — the screen rectangle is recomputed from the
/// transform every frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PackingBox {
    /// Least `x` any drawn circle reaches, in vessel radii.
    pub min_x: f32,
    /// Greatest `x` any drawn circle reaches, in vessel radii.
    pub max_x: f32,
    /// Least `y` any drawn circle reaches, in vessel radii.
    pub min_y: f32,
    /// Greatest `y` any drawn circle reaches, in vessel radii.
    pub max_y: f32,
}

impl PackingBox {
    /// Empty box, used as the fold seed and returned when a window selects no
    /// pebbles at all.
    fn empty() -> Self {
        Self {
            min_x: f32::INFINITY,
            max_x: f32::NEG_INFINITY,
            min_y: f32::INFINITY,
            max_y: f32::NEG_INFINITY,
        }
    }

    /// Whether the box contains any drawn circle.
    fn is_empty(&self) -> bool {
        !(self.max_x > self.min_x && self.max_y > self.min_y)
    }
}

/// Bounding box of every pebble `window` keeps, expanded by the drawn radius.
///
/// This is the extent the bake must cover: it is derived from the data rather
/// than from the window bounds, because a window can be wider than the pebbles
/// that survive it.
pub(crate) fn bed_bounding_box(window: &PackingWindow) -> PackingBox {
    let mut bbox = PackingBox::empty();
    for pebble in PACKED_PEBBLES {
        if !window.contains(pebble) {
            continue;
        }
        bbox.min_x = bbox.min_x.min(pebble.x - SPHERE_RADIUS);
        bbox.max_x = bbox.max_x.max(pebble.x + SPHERE_RADIUS);
        bbox.min_y = bbox.min_y.min(pebble.y - SPHERE_RADIUS);
        bbox.max_y = bbox.max_y.max(pebble.y + SPHERE_RADIUS);
    }
    bbox
}

/// The screen rectangle, in points, that `bbox` occupies under `transform`.
///
/// Recomputed every frame rather than stored, so that panning the vessel is free
/// — only the *size* is a cache key.
pub(crate) fn bed_screen_rect(transform: &PackingTransform, bbox: &PackingBox) -> Rect {
    let corner = |x: f32, y: f32| transform.centre(&PackedPebble::new(x, y, 0.0));
    let a = corner(bbox.min_x, bbox.min_y);
    let b = corner(bbox.max_x, bbox.max_y);
    Rect::from_min_max(
        Pos2::new(a.x.min(b.x), a.y.min(b.y)),
        Pos2::new(a.x.max(b.x), a.y.max(b.y)),
    )
}

/// Physical pixels of margin around the baked artwork.
///
/// Two, not one: half a pixel for the anti-aliased fringe of a tangent pebble,
/// half for the pixel-grid snap, and half again so the outermost texel row is
/// genuinely empty rather than carrying a partial fringe that would sample
/// against the quad's edge.
const BED_QUAD_MARGIN_PX: f32 = 2.0;

/// The screen rectangle the baked quad occupies: the artwork's extent, grown by
/// [`BED_QUAD_MARGIN_PX`] physical pixels all round, snapped to whole physical
/// pixels.
///
/// The margin is not slack. A filled circle's anti-aliased fringe reaches half a
/// pixel past its geometric edge, the packing's outermost pebbles are tangent to
/// [`bed_bounding_box`] by construction, and snapping the rectangle to the pixel
/// grid can move it another half pixel. Without the margin the bed's rim would
/// be cut off at the image border. Bake and paint both go through this one
/// function, so the texel grid and the quad can never disagree.
pub(crate) fn bed_quad_rect(
    transform: &PackingTransform,
    bbox: &PackingBox,
    pixels_per_point: f32,
) -> Rect {
    let margin = if pixels_per_point > 0.0 {
        BED_QUAD_MARGIN_PX / pixels_per_point
    } else {
        BED_QUAD_MARGIN_PX
    };
    snap_to_physical_pixels(
        bed_screen_rect(transform, bbox).expand(margin),
        pixels_per_point,
    )
}

/// The size in whole physical pixels of `rect`.
pub(crate) fn pixel_size_of(rect: Rect, pixels_per_point: f32) -> [usize; 2] {
    [
        (rect.width() * pixels_per_point).round().max(0.0) as usize,
        (rect.height() * pixels_per_point).round().max(0.0) as usize,
    ]
}

/// Snaps `rect` so its corners land on whole **physical pixels**.
///
/// One texel then maps to one pixel and the baked artwork is as crisp as the
/// vector path it replaces. Without this, a bed whose left edge sits at a
/// fractional pixel would be resampled every frame even though nothing moved.
pub(crate) fn snap_to_physical_pixels(rect: Rect, pixels_per_point: f32) -> Rect {
    if !(pixels_per_point > 0.0) || !rect.is_finite() {
        return rect;
    }
    let snap = |v: f32| (v * pixels_per_point).round() / pixels_per_point;
    Rect::from_min_max(
        Pos2::new(snap(rect.min.x), snap(rect.min.y)),
        Pos2::new(snap(rect.max.x), snap(rect.max.y)),
    )
}

/// Everything a re-bake depends on — and nothing else.
///
/// Deliberately excludes **every colour** (temperature is a vertex tint) and the
/// widget's **position** (the screen rectangle is recomputed each frame). What
/// remains is the physical pixel size, the packing scale (which sets the dot
/// count and dot radius), the crop window, and which way up the bed is drawn.
///
/// Floating-point fields are quantised to integers before being compared, so a
/// hair of layout jitter does not force a re-bake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BedBakeKey {
    /// Baked image width in physical pixels.
    pub width_px: usize,
    /// Baked image height in physical pixels.
    pub height_px: usize,
    /// Points per vessel radius, quantised to 1e-3 pt.
    pub scale_milli: i64,
    /// Device pixel ratio, quantised to 1e-3.
    pub pixels_per_point_milli: i64,
    /// The crop window's three bounds, quantised to 1e-4 vessel radii.
    pub window_quantised: [i64; 3],
    /// Whether the bed is drawn inverted (a buoyant FHR bed).
    pub buoyant: bool,
}

impl BedBakeKey {
    /// Build the key for a bed of `size_px` physical pixels drawn under
    /// `transform` and cropped to `window` at `pixels_per_point`.
    pub(crate) fn new(
        size_px: [usize; 2],
        transform: &PackingTransform,
        window: &PackingWindow,
        pixels_per_point: f32,
    ) -> Self {
        let quantise = |v: f32, unit: f32| (v as f64 * unit as f64).round() as i64;
        Self {
            width_px: size_px[0],
            height_px: size_px[1],
            scale_milli: quantise(transform.scale, 1.0e3),
            pixels_per_point_milli: quantise(pixels_per_point, 1.0e3),
            window_quantised: [
                quantise(window.max_abs_x, 1.0e4),
                quantise(window.min_y, 1.0e4),
                quantise(window.max_y, 1.0e4),
            ],
            buoyant: matches!(transform.vertical, VerticalSense::Buoyant),
        }
    }
}

/// A rasterised pebble bed held on the GPU as three luminance/alpha masks.
///
/// Cheap to clone (each handle is an `Arc` into `egui`'s texture manager), which
/// is what lets it live in `egui`'s per-context temp store. Dropping the last
/// clone frees the textures.
#[derive(Clone)]
pub(crate) struct BakedPebbleBed {
    /// What this bake was made for; a mismatch forces a re-bake.
    key: BedBakeKey,
    /// Coverage of every pixel any pebble touches.
    backdrop: TextureHandle,
    /// Coverage of the graphite bodies, already multiplied by depth shade.
    matrix: TextureHandle,
    /// Coverage of the TRISO speckle, already multiplied by depth shade.
    kernel: TextureHandle,
    /// Extent of the artwork in packing coordinates.
    bbox: PackingBox,
    /// How many pebbles the window kept — reported back to the caller so a
    /// silently empty bed is still detectable.
    pebbles: usize,
}

impl BakedPebbleBed {
    /// How many pebbles this bake contains.
    pub(crate) fn pebbles(&self) -> usize {
        self.pebbles
    }
}

/// Which of the three masks a rasterised disc writes into.
///
/// Enum dispatch again: the rasteriser matches on it exhaustively rather than
/// taking a callback or a trait object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaskLayer {
    /// A graphite pebble body.
    Matrix,
    /// One TRISO kernel dot.
    Kernel,
}

/// Three parallel coverage buffers, one per layer, in `[0, 1]`.
///
/// Held as `f32` during the bake so repeated over-compositing does not
/// accumulate byte-rounding error; converted to 8-bit images once at the end.
struct BedMasks {
    width: usize,
    height: usize,
    backdrop: Vec<f32>,
    matrix: Vec<f32>,
    kernel: Vec<f32>,
}

impl BedMasks {
    /// All-zero (fully transparent) masks of `width` x `height` pixels.
    fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Self {
            width,
            height,
            backdrop: vec![0.0; n],
            matrix: vec![0.0; n],
            kernel: vec![0.0; n],
        }
    }

    /// Composites one anti-aliased filled disc over the masks.
    ///
    /// `centre` and `radius` are in **pixels**, `shade` is the depth-shading
    /// weight in `[0, 1]`. The disc is opaque, so it replaces what is underneath
    /// in proportion to its coverage — which is exactly the painter's-algorithm
    /// occlusion the direct path gets from drawing back to front.
    ///
    /// Coverage is the standard one-pixel distance ramp,
    /// `clamp(radius + 0.5 - d, 0, 1)`, matching `egui`'s own feathering width.
    fn fill_disc(&mut self, centre: (f32, f32), radius: f32, layer: MaskLayer, shade: f32) {
        if !radius.is_finite() || radius <= 0.0 || !centre.0.is_finite() || !centre.1.is_finite() {
            return;
        }
        let reach = radius + 1.0;
        let x0 = ((centre.0 - reach).floor().max(0.0)) as usize;
        let y0 = ((centre.1 - reach).floor().max(0.0)) as usize;
        let x1 = ((centre.0 + reach).ceil().max(0.0) as usize).min(self.width);
        let y1 = ((centre.1 + reach).ceil().max(0.0) as usize).min(self.height);

        for y in y0..y1 {
            let dy = y as f32 + 0.5 - centre.1;
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - centre.0;
                let coverage = (radius + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
                if coverage <= 0.0 {
                    continue;
                }
                let i = y * self.width + x;
                let keep = 1.0 - coverage;
                self.backdrop[i] = self.backdrop[i] * keep + coverage;
                match layer {
                    MaskLayer::Matrix => {
                        self.matrix[i] = self.matrix[i] * keep + coverage * shade;
                        self.kernel[i] *= keep;
                    }
                    MaskLayer::Kernel => {
                        self.kernel[i] = self.kernel[i] * keep + coverage * shade;
                        self.matrix[i] *= keep;
                    }
                }
            }
        }
    }

    /// One mask as a premultiplied white [`ColorImage`], ready to be tinted.
    ///
    /// A texel is white at alpha `a`, i.e. `(255a, 255a, 255a, 255a)`. Multiplied
    /// by a premultiplied vertex tint `C` and composited over `dst` this gives
    /// `C * a + dst * (1 - a)` — the alpha-over the tinting scheme depends on.
    fn to_image(mask: &[f32], width: usize, height: usize) -> ColorImage {
        let pixels = mask
            .iter()
            .map(|a| {
                let v = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
                Color32::from_rgba_premultiplied(v, v, v, v)
            })
            .collect();
        ColorImage::new([width, height], pixels)
    }
}

/// Rasterises the bed into three coverage masks at `pixels_per_point`.
///
/// Pure CPU, deterministic, and independent of `egui`'s context — which is what
/// makes it testable without a GPU, and consistent with this workspace's
/// offline-deterministic ethos (no render-to-texture callback, no GPU readback).
///
/// Geometry is taken in **points** exactly as the direct path computes it (the
/// dot count and dot radius are functions of the point radius, so baking at
/// pixel scale would change the artwork) and only then multiplied by
/// `pixels_per_point`.
///
/// `origin` is the top-left of the quad the masks will be painted into, in
/// screen points — always [`bed_quad_rect`]'s `min`, so that texel (0, 0) is the
/// quad's top-left pixel.
///
/// Returns the masks and the number of pebbles drawn.
fn rasterise_bed(
    transform: &PackingTransform,
    window: &PackingWindow,
    origin: Pos2,
    size_px: [usize; 2],
    pixels_per_point: f32,
) -> (BedMasks, usize) {
    let mut masks = BedMasks::new(size_px[0], size_px[1]);
    let mut pebbles = 0usize;

    // The baked table is sorted FARTHEST FIRST, so compositing straight through
    // it reproduces the painter's-algorithm occlusion of the direct path.
    for (index, pebble) in PACKED_PEBBLES.iter().enumerate() {
        if !window.contains(pebble) {
            continue;
        }
        pebbles += 1;

        let centre_pt = transform.centre(pebble);
        let radius_pt = transform.radius(pebble);
        let shade = depth_shade(pebble.depth());

        let to_px = |p: Pos2| {
            (
                (p.x - origin.x) * pixels_per_point,
                (p.y - origin.y) * pixels_per_point,
            )
        };

        masks.fill_disc(
            to_px(centre_pt),
            radius_pt * pixels_per_point,
            MaskLayer::Matrix,
            shade,
        );

        let dots = triso_dot_count(radius_pt);
        let dot_radius_px = triso_dot_radius(radius_pt) * pixels_per_point;
        for k in 0..dots {
            let dot_centre = centre_pt + triso_dot_offset(index as i32, k, radius_pt);
            masks.fill_disc(to_px(dot_centre), dot_radius_px, MaskLayer::Kernel, shade);
        }
    }

    (masks, pebbles)
}

/// Rasterises and uploads the bed, returning a cacheable handle.
///
/// The three textures are uploaded with linear filtering, so that a bed drawn at
/// a size other than the one it was baked at degrades smoothly rather than
/// blockily — see the module docs on the raster/vector tradeoff.
fn bake(
    ctx: &Context,
    transform: &PackingTransform,
    window: &PackingWindow,
    bbox: &PackingBox,
    key: BedBakeKey,
    pixels_per_point: f32,
) -> BakedPebbleBed {
    let size_px = [key.width_px, key.height_px];
    let origin = bed_quad_rect(transform, bbox, pixels_per_point).min;
    let (masks, pebbles) = rasterise_bed(transform, window, origin, size_px, pixels_per_point);

    let upload = |name: &str, mask: &[f32]| {
        ctx.load_texture(
            name,
            BedMasks::to_image(mask, masks.width, masks.height),
            TextureOptions::LINEAR,
        )
    };

    BakedPebbleBed {
        key,
        backdrop: upload("outram_park_bed_backdrop", &masks.backdrop),
        matrix: upload("outram_park_bed_matrix", &masks.matrix),
        kernel: upload("outram_park_bed_kernel", &masks.kernel),
        bbox: *bbox,
        pebbles,
    }
}

/// Paints one mask as a vertical strip of textured quads carrying `tint`.
///
/// A [`BedTint::Uniform`] tint is one quad (4 vertices); an axial tint of `n`
/// colours is `n - 1` quads whose per-vertex colours interpolate between
/// adjacent nodes, so the bed grades smoothly up its height. All of them go into
/// a single [`Mesh`], hence a single draw.
///
/// `sense` decides which screen end colour index 0 belongs at: a gravity-settled
/// bed puts it at the bottom, a buoyant (inverted) bed at the top.
fn paint_tinted_layer(
    painter: &Painter,
    texture: &TextureHandle,
    rect: Rect,
    tint: &BedTint,
    sense: &VerticalSense,
) {
    let colours: Vec<Color32> = match tint {
        BedTint::Uniform(colour) => vec![*colour, *colour],
        BedTint::Axial(colours) => match colours.len() {
            0 => vec![Color32::WHITE, Color32::WHITE],
            1 => vec![colours[0], colours[0]],
            _ => colours.clone(),
        },
    };

    // Screen y that dimensionless bed height 0.0 and 1.0 map to.
    let (y_at_bottom_of_bed, y_at_top_of_bed) = match sense {
        VerticalSense::GravityUp => (rect.bottom(), rect.top()),
        VerticalSense::Buoyant => (rect.top(), rect.bottom()),
    };

    let bands = colours.len() - 1;
    let mut mesh = Mesh::with_texture(texture.id());
    mesh.reserve_triangles(bands * 2);
    mesh.reserve_vertices(bands * 4);

    let height = rect.height();
    let v_of = |y: f32| {
        if height > 0.0 {
            (y - rect.top()) / height
        } else {
            0.0
        }
    };

    for band in 0..bands {
        let f0 = band as f32 / bands as f32;
        let f1 = (band + 1) as f32 / bands as f32;
        let ya = y_at_bottom_of_bed + (y_at_top_of_bed - y_at_bottom_of_bed) * f0;
        let yb = y_at_bottom_of_bed + (y_at_top_of_bed - y_at_bottom_of_bed) * f1;

        // Order the band's edges top-first so the quad is wound consistently.
        let ((y_top, colour_top), (y_bottom, colour_bottom)) = if ya <= yb {
            ((ya, colours[band]), (yb, colours[band + 1]))
        } else {
            ((yb, colours[band + 1]), (ya, colours[band]))
        };

        let base = mesh.vertices.len() as u32;
        let (v_top, v_bottom) = (v_of(y_top), v_of(y_bottom));
        for (y, v, colour) in [
            (y_top, v_top, colour_top),
            (y_bottom, v_bottom, colour_bottom),
        ] {
            mesh.vertices.push(Vertex {
                pos: Pos2::new(rect.left(), y),
                uv: Pos2::new(0.0, v),
                color: colour,
            });
            mesh.vertices.push(Vertex {
                pos: Pos2::new(rect.right(), y),
                uv: Pos2::new(1.0, v),
                color: colour,
            });
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 2, base + 1, base + 3);
    }

    painter.add(egui::Shape::mesh(mesh));
}

/// Paints a baked bed: backdrop, then graphite bodies, then the temperature-tinted
/// speckle.
///
/// The order is the alpha-over chain the module docs derive; changing it changes
/// the picture. `painter` may already carry a clip rectangle — a fill-level
/// indicator composes as a clip at draw time and is deliberately **not** part of
/// [`BedBakeKey`], so raising or lowering the fill line never re-bakes anything.
pub(crate) fn paint_baked_bed(
    painter: &Painter,
    baked: &BakedPebbleBed,
    transform: &PackingTransform,
    backdrop: Color32,
    matrix: Color32,
    kernel: &BedTint,
    pixels_per_point: f32,
) {
    let rect = bed_quad_rect(transform, &baked.bbox, pixels_per_point);
    if !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let uniform = |c: Color32| BedTint::Uniform(c);
    paint_tinted_layer(
        painter,
        &baked.backdrop,
        rect,
        &uniform(backdrop),
        &transform.vertical,
    );
    paint_tinted_layer(
        painter,
        &baked.matrix,
        rect,
        &uniform(matrix),
        &transform.vertical,
    );
    paint_tinted_layer(painter, &baked.kernel, rect, kernel, &transform.vertical);
}

/// Fetches the bed for `cache_id`, re-baking only if `key` does not match what
/// is stored.
///
/// The cache lives in `egui`'s per-context temp store, keyed by an [`Id`] the
/// caller owns, so two vessels on screen at once keep separate bakes instead of
/// thrashing one entry.
fn cached_bake(
    ctx: &Context,
    cache_id: Id,
    transform: &PackingTransform,
    window: &PackingWindow,
    bbox: &PackingBox,
    key: BedBakeKey,
    pixels_per_point: f32,
) -> BakedPebbleBed {
    let existing: Option<BakedPebbleBed> = ctx.data_mut(|d| d.get_temp(cache_id));
    if let Some(bed) = existing {
        if bed.key == key {
            return bed;
        }
    }
    let fresh = bake(ctx, transform, window, bbox, key, pixels_per_point);
    ctx.data_mut(|d| d.insert_temp(cache_id, fresh.clone()));
    fresh
}

/// Whether the direct-circle path has been forced for this context.
///
/// Test-only: the benchmark in this module measures both paths in one process,
/// which needs a way to ask the widget for its pre-optimisation behaviour. There
/// is no such switch in a release build.
#[cfg(test)]
fn direct_path_forced(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(force_direct_id()).unwrap_or(false))
}

#[cfg(not(test))]
fn direct_path_forced(_ctx: &Context) -> bool {
    false
}

/// Id of the test-only direct-path override.
#[cfg(test)]
fn force_direct_id() -> Id {
    Id::new("outram_park_bed_force_direct")
}

/// Forces (or releases) the direct-circle path for `ctx`. Test-only.
#[cfg(test)]
pub(crate) fn force_direct_path(ctx: &Context, force: bool) {
    ctx.data_mut(|d| d.insert_temp(force_direct_id(), force));
}

/// Draws the settled pebble bed, baking it to textures when that is cheaper.
///
/// This is the entry point both vessel widgets use. It picks a
/// [`BedDrawStrategy`], and on the baked path returns without touching a single
/// circle: three tinted quads (12 vertices for a uniform tint) replace the
/// 19 874 circles the HTR-10 bed used to emit per frame.
///
/// `cache_id` must be stable for a given vessel across frames — the widgets pass
/// an id derived from their allocated response — and distinct between vessels.
///
/// Returns how many pebbles were drawn, matching the direct path, so a caller
/// checking that a degenerate size did not silently empty the bed can still do
/// so.
pub(crate) fn draw_bed(
    painter: &Painter,
    cache_id: Id,
    transform: &PackingTransform,
    window: &PackingWindow,
    backdrop: Color32,
    matrix: Color32,
    kernel: &BedTint,
) -> usize {
    let ctx = painter.ctx();
    let bbox = bed_bounding_box(window);
    if bbox.is_empty() {
        return 0;
    }

    let pixels_per_point = ctx.pixels_per_point();
    let size_px = pixel_size_of(
        bed_quad_rect(transform, &bbox, pixels_per_point),
        pixels_per_point,
    );
    let max_texture_side = ctx.input(|i| i.max_texture_side);
    let radius_pt = SPHERE_RADIUS * transform.scale;

    if direct_path_forced(ctx)
        || choose_strategy(radius_pt, size_px, max_texture_side) == BedDrawStrategy::Direct
    {
        return draw_packed_pebbles_direct(painter, transform, window, backdrop, matrix, kernel);
    }

    let key = BedBakeKey::new(size_px, transform, window, pixels_per_point);
    let baked = cached_bake(
        ctx,
        cache_id,
        transform,
        window,
        &bbox,
        key,
        pixels_per_point,
    );
    paint_baked_bed(
        painter,
        &baked,
        transform,
        backdrop,
        matrix,
        kernel,
        pixels_per_point,
    );
    baked.pebbles()
}

#[cfg(test)]
mod tests;
