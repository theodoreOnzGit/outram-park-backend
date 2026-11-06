//! Verification and measurement for the baked pebble bed.
//!
//! Two things need checking, and they are different questions:
//!
//! 1. **Is it the same picture?** The baked path must composite to the colour
//!    the direct-circle path computes, and must stay temperature-responsive.
//! 2. **Is it actually faster?** The whole point of the change is a frame-cost
//!    reduction, so it is measured here rather than asserted.

use super::*;
use crate::components::fhr_reactor_vessel::FhrReactorVesselVisual;
use crate::components::htr10_reactor_vessel::{Htr10ReactorVesselVisual, NATIVE_ASPECT_RATIO};
use egui::{Rect, Sense, Vec2};
use std::time::Instant;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// Size the HTGR simulator actually allocates for its vessel
/// (`examples/htgr_sim_v1/app/schematic.rs`, `REACTOR_BOX`), so the measurements
/// below describe the real widget rather than an invented one.
const EXAMPLE_VESSEL_BOX: Vec2 = Vec2::new(190.0, 400.0);

/// Size the FHR simulator allocates for its vessel
/// (`examples/fhr_sim_v2/app/mod.rs`: `150 * 1.5` by `700 * 1.5` points).
const EXAMPLE_FHR_BOX: Vec2 = Vec2::new(225.0, 1050.0);

/// Which example simulator's vessel a benchmark frame draws.
///
/// An enum rather than a callback, per this workspace's Rust design rules: both
/// vessels share the pebble artwork this module optimises, and the bead reported
/// both simulators as laggy, so both are measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BenchVessel {
    /// The HTR-10 gas-cooled vessel, gravity-settled bed.
    Htr10,
    /// The FHR salt-cooled vessel, buoyant (inverted) bed.
    Fhr,
}

/// A headless `egui` context configured the way a desktop backend configures
/// one, so `load_texture` and `tessellate` behave as they do in the app.
fn headless_context() -> (egui::Context, egui::RawInput) {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 1200.0))),
        max_texture_side: Some(8192),
        ..Default::default()
    };
    // One warm-up frame so fonts exist before anything is tessellated.
    let _ = ctx.run_ui(input.clone(), |_| {});
    (ctx, input)
}

/// Tessellated cost of one frame: vertices, indices, shapes, and the wall clock
/// for building plus tessellating them.
#[derive(Clone, Copy, Debug, Default)]
struct FrameCost {
    vertices: usize,
    indices: usize,
    shapes: usize,
    milliseconds: f64,
}

/// Runs one frame that draws `vessel` at `widget_size` and returns its cost.
fn measure_vessel_frame(
    ctx: &egui::Context,
    input: &egui::RawInput,
    vessel: BenchVessel,
    widget_size: Vec2,
) -> FrameCost {
    let t = |k: f64| ThermodynamicTemperature::new::<kelvin>(k);
    let start = Instant::now();
    let output = ctx.run_ui(input.clone(), |ui| {
        let where_to = Rect::from_min_size(Pos2::new(20.0, 20.0), widget_size);
        match vessel {
            BenchVessel::Htr10 => {
                ui.put(
                    where_to,
                    Htr10ReactorVesselVisual::new(
                        widget_size,
                        t(523.0),
                        t(1273.0),
                        t(1100.0),
                        t(523.0),
                        t(973.0),
                        t(600.0),
                    ),
                );
            }
            BenchVessel::Fhr => {
                ui.put(
                    where_to,
                    FhrReactorVesselVisual::new(
                        widget_size,
                        t(723.0),
                        t(1023.0),
                        t(950.0),
                        t(900.0),
                        t(880.0),
                        t(920.0),
                        t(860.0),
                        t(960.0),
                        t(870.0),
                        t(865.0),
                        t(860.0),
                        t(870.0),
                        t(865.0),
                        t(860.0),
                    ),
                );
            }
        }
    });
    let shapes = output.shapes.len();
    let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
    let milliseconds = start.elapsed().as_secs_f64() * 1000.0;

    let mut cost = FrameCost {
        shapes,
        milliseconds,
        ..Default::default()
    };
    for primitive in &primitives {
        if let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive {
            cost.vertices += mesh.vertices.len();
            cost.indices += mesh.indices.len();
        }
    }
    cost
}

/// The composite algebra of the three tinted layers must reproduce the
/// direct-circle path's `blend_rgb(backdrop, colour, shade)` on pebble
/// interiors.
///
/// **Methodology.** The baked path paints three alpha-over layers: a backdrop
/// mask at coverage `a`, a matrix mask at `m`, a kernel mask at `k`. For a pixel
/// wholly inside a graphite body, `(a, m, k) = (1, shade, 0)`; for one wholly
/// inside a TRISO kernel, `(1, 0, shade)`. Composite those two cases in
/// premultiplied-alpha arithmetic exactly as `epaint`'s blend function does
/// (`dst = src + dst * (1 - src.a)`), and compare against
/// `blend_rgb(BED_BACKDROP, colour, shade)` — the colour the direct path hands
/// to `circle_filled`. The comparison is run over shades spanning the depth
/// window's `MIN_DEPTH_SHADE`..1.0 and over hot, cold and neutral fuel colours,
/// against an arbitrary bed-fill colour behind, which must not affect an opaque
/// interior. Pass criterion: every channel within 1/255.
///
/// **Results (2026-08-12).** Maximum channel error 1/255 over 3 shades x 3 fuel
/// colours x 2 layers = 18 cases, i.e. pure byte rounding — the algebra is
/// exact. Interpretation: switching a bed from circles to tinted quads does not
/// change the colour of the artwork's interior, so the optimisation is not
/// paid for in fidelity. The one place the two paths genuinely differ is the
/// single-pixel anti-aliased rim, bounded at `backdrop * shade / 4` (under
/// 4/255) by the module docs and not exercised here.
#[test]
fn composite_matches_the_direct_blend_on_interiors() {
    /// `src` over `dst`, in premultiplied bytes, as `epaint` blends.
    fn over(src: Color32, dst: Color32) -> Color32 {
        let keep = 1.0 - src.a() as f32 / 255.0;
        let mix = |s: u8, d: u8| (s as f32 + d as f32 * keep).round().min(255.0) as u8;
        Color32::from_rgba_premultiplied(
            mix(src.r(), dst.r()),
            mix(src.g(), dst.g()),
            mix(src.b(), dst.b()),
            mix(src.a(), dst.a()),
        )
    }

    /// A luminance/alpha texel at coverage `a`, tinted by `tint`, as the GPU
    /// multiplies them.
    fn tinted_texel(coverage: f32, tint: Color32) -> Color32 {
        let a = coverage.clamp(0.0, 1.0);
        let mix = |c: u8| (c as f32 * a).round() as u8;
        Color32::from_rgba_premultiplied(mix(tint.r()), mix(tint.g()), mix(tint.b()), mix(tint.a()))
    }

    // The direct path's own blend, reproduced here so the test does not depend
    // on a private helper.
    fn direct_blend(backdrop: Color32, colour: Color32, shade: f32) -> Color32 {
        let t = shade.clamp(0.0, 1.0);
        let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        Color32::from_rgba_premultiplied(
            mix(backdrop.r(), colour.r()),
            mix(backdrop.g(), colour.g()),
            mix(backdrop.b(), colour.b()),
            backdrop.a(),
        )
    }

    let backdrop = Color32::from_rgb(16, 16, 20);
    let matrix = Color32::from_rgb(28, 28, 32);
    let bed_fill = Color32::from_rgb(200, 40, 30);

    let mut worst = 0i32;
    let mut cases = 0;
    for shade in [0.45f32, 0.72, 1.0] {
        for fuel in [
            Color32::from_rgb(0, 18, 97),
            Color32::from_rgb(236, 229, 224),
            Color32::from_rgb(89, 0, 8),
        ] {
            for (m, k, expected_colour) in [(shade, 0.0, matrix), (0.0, shade, fuel)] {
                // backdrop layer (coverage 1), then matrix layer, then kernel.
                let mut pixel = over(tinted_texel(1.0, backdrop), bed_fill);
                pixel = over(tinted_texel(m, matrix), pixel);
                pixel = over(tinted_texel(k, fuel), pixel);

                let expected = direct_blend(backdrop, expected_colour, shade);
                for (a, b) in [
                    (pixel.r(), expected.r()),
                    (pixel.g(), expected.g()),
                    (pixel.b(), expected.b()),
                ] {
                    worst = worst.max((a as i32 - b as i32).abs());
                }
                cases += 1;
            }
        }
    }
    println!("composite: {cases} interior cases, worst channel error {worst}/255");
    assert!(
        worst <= 1,
        "baked composite drifts from the direct blend by {worst}/255"
    );
}

/// The cache key must ignore colour and position, and respond to physical
/// pixel size.
///
/// **Methodology.** Build a key, then rebuild it after (a) translating the
/// transform, which is what panning a vessel does, and (b) changing the pixel
/// size, which is what resizing or a DPI change does. Colour never enters the
/// key's constructor at all, which the type system enforces — there is no colour
/// field to pass.
///
/// **Results (2026-08-12).** Translation left the key equal; a one-pixel size
/// change made it differ. Interpretation: temperature and panning are free, and
/// a re-bake happens only on resize or a DPI/monitor change, which is the
/// intended cache behaviour.
#[test]
fn the_cache_key_ignores_position_and_tracks_pixel_size() {
    let window = PackingWindow::whole_bed();
    let transform = PackingTransform {
        axis_x: 100.0,
        origin_y: 300.0,
        scale: 57.0,
        vertical: VerticalSense::GravityUp,
    };
    let moved = PackingTransform {
        axis_x: 900.0,
        origin_y: 40.0,
        ..transform
    };

    let key = BedBakeKey::new([228, 350], &transform, &window, 2.0);
    assert_eq!(
        key,
        BedBakeKey::new([228, 350], &moved, &window, 2.0),
        "panning the vessel must not force a re-bake"
    );
    assert_ne!(
        key,
        BedBakeKey::new([229, 350], &transform, &window, 2.0),
        "a change of physical pixel size must force a re-bake"
    );
    assert_ne!(
        key,
        BedBakeKey::new([228, 350], &transform, &window, 1.5),
        "a DPI change must force a re-bake"
    );
}

/// An axial tint must grade up the bed, and must flip with a buoyant bed.
///
/// **Methodology.** [`BedTint::sample`] is the shared definition both draw paths
/// use, so check it directly: with three node colours, height 0.0 must return
/// the first, 1.0 the last, and 0.5 the middle. Then paint a three-node axial
/// tint into a real `egui` frame under each [`VerticalSense`] and confirm the
/// resulting mesh carries the first colour at the screen bottom for a
/// gravity-settled bed and at the screen top for a buoyant (inverted) one.
///
/// **Results (2026-08-12).** `sample` returned the node colours exactly at
/// 0.0/0.5/1.0. The painted mesh put node 0 at screen y = rect.bottom() for
/// `GravityUp` and at rect.top() for `Buoyant`. Interpretation: the seam for an
/// axially nodalised bed (the maintainer's 15-25 zones) is real and exercised,
/// not a placeholder, and callers do not have to know which way their bed is
/// drawn.
#[test]
fn an_axial_tint_grades_up_the_bed_and_respects_buoyancy() {
    let low = Color32::from_rgb(10, 20, 30);
    let mid = Color32::from_rgb(100, 110, 120);
    let high = Color32::from_rgb(200, 210, 220);
    let tint = BedTint::Axial(vec![low, mid, high]);

    assert_eq!(tint.sample(0.0), low);
    assert_eq!(tint.sample(0.5), mid);
    assert_eq!(tint.sample(1.0), high);
    assert_eq!(tint.sample(-5.0), low, "below the bed must clamp");
    assert_eq!(tint.sample(5.0), high, "above the bed must clamp");

    let (ctx, input) = headless_context();
    let rect = Rect::from_min_size(Pos2::new(0.0, 100.0), Vec2::new(50.0, 200.0));

    for (sense, expected_y_of_node_zero) in [
        (VerticalSense::GravityUp, rect.bottom()),
        (VerticalSense::Buoyant, rect.top()),
    ] {
        let texture = ctx.load_texture(
            "axial_test",
            ColorImage::new([2, 2], vec![Color32::WHITE; 4]),
            TextureOptions::LINEAR,
        );
        let output = ctx.run_ui(input.clone(), |ui| {
            let (_, painter) = ui.allocate_painter(rect.size(), Sense::hover());
            paint_tinted_layer(&painter, &texture, rect, &tint, &sense);
        });
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        let mut found = None;
        for primitive in &primitives {
            if let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive {
                for vertex in &mesh.vertices {
                    if vertex.color == low {
                        found = Some(vertex.pos.y);
                    }
                }
            }
        }
        let y = found.expect("node-0 colour must appear in the painted mesh");
        assert!(
            (y - expected_y_of_node_zero).abs() < 0.5,
            "{sense:?}: node 0 landed at y={y}, expected {expected_y_of_node_zero}"
        );
    }
}

/// A bed too small to speckle must keep the crisp direct-circle path; a
/// speckled one must take the baked path.
///
/// **Methodology.** [`choose_strategy`] is the whole decision, so drive it
/// directly across the three TRISO regimes (no dots, one dot, a full speckle),
/// and separately with a pixel box larger than the backend's maximum texture
/// side.
///
/// **Results (2026-08-12).** Radii 1.0 and 2.0 pt (0 and 1 dots) chose `Direct`;
/// 4.3 pt — the radius the HTGR example actually draws at — chose `Baked`; a
/// 9000 px box against a 8192 px limit chose `Direct`. Interpretation: the
/// texture is used exactly where the circle count is pathological, and a
/// thumbnail-sized bed keeps vector crispness where a resampled texture would
/// look worst.
#[test]
fn the_strategy_switches_on_speckle_and_texture_limits() {
    assert_eq!(
        choose_strategy(1.0, [200, 300], 8192),
        BedDrawStrategy::Direct
    );
    assert_eq!(
        choose_strategy(2.0, [200, 300], 8192),
        BedDrawStrategy::Direct
    );
    assert_eq!(
        choose_strategy(4.3, [200, 300], 8192),
        BedDrawStrategy::Baked
    );
    assert_eq!(
        choose_strategy(4.3, [9000, 300], 8192),
        BedDrawStrategy::Direct,
        "a bed larger than the maximum texture side must not be baked"
    );
    assert_eq!(
        choose_strategy(4.3, [0, 300], 8192),
        BedDrawStrategy::Direct
    );
}

/// The rasteriser must reproduce the packing: every pebble the window keeps,
/// covered, occluded back to front, and confined to the baked box.
///
/// **Methodology.** Rasterise the HTR-10 whole-bed window at the example's own
/// drawn scale and check four properties of the masks that a wrong rasteriser
/// would break: all 523 pebbles are visited; a substantial fraction of the box
/// is covered (an empty or mistransformed bake would not be); the kernel mask is
/// non-empty (the speckle survived); and no coverage lands on the outermost
/// pixel ring, which would mean the bounding box is too tight and pebbles are
/// being clipped.
///
/// **Results (2026-08-12).** 232x353 px at 2x DPI: 523 pebbles rasterised;
/// backdrop coverage 79.3 % of the box; kernel coverage 24.9 %; maximum coverage
/// on the outer pixel ring 0.0000. Interpretation: the bake covers the same
/// artwork as the direct path, the speckle is present at roughly the density
/// `TRISO_TARGET_FILL` (0.55 of each fuelled zone) implies, and the two-pixel
/// margin keeps the outermost pebbles' anti-aliased rims inside the image.
#[test]
fn the_rasteriser_covers_the_whole_packing_without_clipping() {
    let window = PackingWindow::whole_bed();
    let bbox = bed_bounding_box(&window);
    let transform = PackingTransform {
        axis_x: 0.0,
        origin_y: 0.0,
        scale: 57.0,
        vertical: VerticalSense::GravityUp,
    };
    let pixels_per_point = 2.0;
    let rect = bed_quad_rect(&transform, &bbox, pixels_per_point);
    let size_px = pixel_size_of(rect, pixels_per_point);

    let (masks, pebbles) = rasterise_bed(&transform, &window, rect.min, size_px, pixels_per_point);
    assert_eq!(pebbles, 523, "the whole-bed window must keep every pebble");

    let total = (size_px[0] * size_px[1]) as f32;
    let covered: f32 = masks.backdrop.iter().sum();
    let speckled: f32 = masks.kernel.iter().sum();
    println!(
        "rasterised {pebbles} pebbles into {}x{} px: backdrop {:.1} %, kernel {:.1} %",
        size_px[0],
        size_px[1],
        100.0 * covered / total,
        100.0 * speckled / total
    );
    assert!(
        covered / total > 0.5,
        "the bed covers only {:.1} % of its own box",
        100.0 * covered / total
    );
    assert!(speckled / total > 0.05, "the TRISO speckle is missing");

    let mut edge = 0.0f32;
    for x in 0..size_px[0] {
        edge = edge.max(masks.backdrop[x]);
        edge = edge.max(masks.backdrop[(size_px[1] - 1) * size_px[0] + x]);
    }
    for y in 0..size_px[1] {
        edge = edge.max(masks.backdrop[y * size_px[0]]);
        edge = edge.max(masks.backdrop[y * size_px[0] + size_px[0] - 1]);
    }
    println!("maximum coverage on the outer pixel ring: {edge:.4}");
    assert!(
        edge < 0.02,
        "coverage {edge} reaches the outer pixel ring — the baked box clips the bed"
    );
}

/// Two frames at the same size must reuse one bake; a resize must replace it.
///
/// **Methodology.** Draw the vessel twice at one size through the real widget
/// and count `egui`'s allocated textures each time (the font atlas plus three
/// bed masks); then draw at a different size and count again. A cache that
/// missed would leave more than three bed textures alive, and a cache that never
/// invalidated would not re-bake on the resize.
///
/// **Results (2026-08-12).** After two frames at 190x400 pt: 4 textures (font
/// atlas + 3 masks). After a third frame at 260x548 pt: still 4, the old three
/// having been freed when the cache entry was replaced. Interpretation: the
/// cache holds exactly one bake per vessel, re-bakes on resize, and does not
/// leak textures across resizes.
#[test]
fn the_cache_holds_one_bake_per_vessel_and_re_bakes_on_resize() {
    let (ctx, input) = headless_context();

    let _ = measure_vessel_frame(&ctx, &input, BenchVessel::Htr10, EXAMPLE_VESSEL_BOX);
    let _ = measure_vessel_frame(&ctx, &input, BenchVessel::Htr10, EXAMPLE_VESSEL_BOX);
    let after_two_frames = ctx.tex_manager().read().num_allocated();

    let _ = measure_vessel_frame(&ctx, &input, BenchVessel::Htr10, EXAMPLE_VESSEL_BOX * 1.37);
    let after_resize = ctx.tex_manager().read().num_allocated();

    println!(
        "textures allocated: {after_two_frames} after two frames, {after_resize} after a resize"
    );
    assert_eq!(
        after_two_frames, 4,
        "expected the font atlas plus three bed masks, got {after_two_frames}"
    );
    assert_eq!(
        after_resize, 4,
        "a resize must replace the bake, not accumulate one, got {after_resize}"
    );
}

/// **The measurement this whole change exists for**: what one HTR-10 vessel
/// costs per frame, before and after.
///
/// **Methodology.** Both paths are measured in one process on one machine, so
/// the comparison is not across builds. A headless `egui::Context` runs the real
/// [`Htr10ReactorVesselVisual`] widget at the size the HTGR simulator allocates
/// (`REACTOR_BOX`, 190x400 pt), at 1x device pixel ratio. Each frame is built
/// and then tessellated with `Context::tessellate`, and the reported cost is the
/// wall clock for **both** steps — that is precisely the CPU work this change
/// removes, and it is what the bead identified as the suspect (a small `update`
/// time inside a long frame interval means the cost is shape-building and
/// tessellation, not the `ui` body).
///
/// The direct-circle path is forced for the "before" run with
/// [`force_direct_path`], so the same binary measures both. Twenty frames are
/// timed per path after a warm-up frame, and the median is reported; the vertex
/// and index counts are exact, not timed, so they are the primary evidence and
/// the timing is corroboration.
///
/// **What it does not measure:** GPU upload, draw submission, present, or vsync.
/// It bounds the CPU side only. It also measures a *whole vessel*, so the bed's
/// share is the difference between the two runs, not the totals themselves.
///
/// **Results (2026-08-12, release build, this machine).** Printed by this test:
///
/// | HTR-10 vessel, 190x400 pt | Direct circles | Baked textures |
/// |---|---|---|
/// | Shapes emitted by the widget | 19 910 | 39 |
/// | Tessellated vertices | 80 788 | 1 304 |
/// | Tessellated indices | 124 242 | 5 016 |
/// | Build + tessellate, median of 20 | 1.73 ms | 0.014 ms |
///
/// | FHR vessel, 225x1050 pt | Direct circles | Baked textures |
/// |---|---|---|
/// | Shapes emitted by the widget | 9 909 | 32 |
/// | Tessellated vertices | 52 340 | 1 052 |
/// | Tessellated indices | 118 680 | 5 268 |
/// | Build + tessellate, median of 20 | 0.53 ms | 0.013 ms |
///
/// The bed's own share is the difference: HTR-10 goes from 19 874 circles
/// (523 pebbles x 38 shapes each) and ~79 500 vertices to **3 quads and 12
/// vertices**; the FHR, whose narrower crop window keeps fewer pebbles, from
/// 9 880 circles to the same 3 quads. Whole-widget totals fall 62x (HTR-10) and
/// 50x (FHR) in tessellated vertices, and 120x and 40x in CPU time. (The
/// HTR-10's baked total rose from 988 to 1 304 vertices on 2026-08-12 when the
/// side-reflector control rods were drawn and animated; that is new artwork, not
/// a regression in the bake.)
///
/// The two vessels differ because they draw at different scales: the HTR-10 bed
/// is smaller on screen, so its pebbles are 3.4 pt and carry 37 dots each, while
/// the FHR's are larger and its window keeps fewer of them.
///
/// Interpretation: the bead's diagnosis is confirmed — the pebble artwork was
/// the dominant per-frame shape cost in both simulators, and baking removes
/// essentially all of it. Absolute times are machine-dependent and should be
/// re-measured rather than quoted; the count reduction is deterministic and is
/// what the assertions below pin.
#[test]
fn baking_the_bed_cuts_the_frame_cost() {
    let (ctx, input) = headless_context();

    let run = |vessel: BenchVessel, size: Vec2, direct: bool| -> FrameCost {
        force_direct_path(&ctx, direct);
        // Warm up: builds the bake on the cached path, and fills egui's caches
        // on both, so the timed frames measure steady state.
        let _ = measure_vessel_frame(&ctx, &input, vessel, size);
        let mut samples = Vec::new();
        let mut cost = FrameCost::default();
        for _ in 0..20 {
            cost = measure_vessel_frame(&ctx, &input, vessel, size);
            samples.push(cost.milliseconds);
        }
        samples.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));
        cost.milliseconds = samples[samples.len() / 2];
        cost
    };

    for (vessel, size) in [
        (BenchVessel::Htr10, EXAMPLE_VESSEL_BOX),
        (BenchVessel::Fhr, EXAMPLE_FHR_BOX),
    ] {
        let direct = run(vessel, size, true);
        let baked = run(vessel, size, false);
        force_direct_path(&ctx, false);

        println!(
            "{vessel:?} vessel at {}x{} pt, build + tessellate:\n\
             \x20 direct circles : {:>6} shapes, {:>7} vertices, {:>7} indices, {:.3} ms\n\
             \x20 baked textures : {:>6} shapes, {:>7} vertices, {:>7} indices, {:.3} ms\n\
             \x20 reduction      : {:.1}x vertices, {:.0}x time",
            size.x,
            size.y,
            direct.shapes,
            direct.vertices,
            direct.indices,
            direct.milliseconds,
            baked.shapes,
            baked.vertices,
            baked.indices,
            baked.milliseconds,
            direct.vertices as f64 / baked.vertices.max(1) as f64,
            direct.milliseconds / baked.milliseconds.max(1.0e-9),
        );

        assert!(
            direct.vertices > 50_000,
            "{vessel:?}: the direct path should be emitting a very large mesh; got {} vertices \
             — has the artwork changed?",
            direct.vertices
        );
        assert!(
            baked.vertices * 20 < direct.vertices,
            "{vessel:?}: baking must cut tessellated vertices by at least 20x: {} -> {}",
            direct.vertices,
            baked.vertices
        );
    }
}

/// The bake itself must be cheap enough that a resize drag stays usable.
///
/// **Methodology.** The honest cost of this change is paid on resize, where the
/// bed is rasterised again at the new pixel size. Time
/// [`rasterise_bed`] alone (no upload) for the HTR-10 whole bed at the example's
/// drawn scale and at 2x device pixel ratio — the worst case a typical desktop
/// reaches. Report the median of five runs. There is no pass/fail threshold on
/// wall clock, since that is machine-dependent; the assertion is only that the
/// bake completes and covers the bed.
///
/// **Results (2026-08-12, release build, this machine).** 232x353 px at 2x DPI,
/// 523 pebbles and their speckle: median 2.4 ms (2.4-3.8 ms across repeated
/// runs). Interpretation: a resize drag costs roughly one 60 Hz frame per size
/// change, on top of the frame's own work, so resizing is measurably less smooth
/// than a static view. The direct path was paying 1.6 ms of shape-building and
/// tessellation on **every** frame, resize or not, so a bake amortises after two
/// frames at one size. Documented rather than hidden; see the module docs.
#[test]
fn the_bake_cost_is_paid_only_on_resize() {
    let window = PackingWindow::whole_bed();
    let bbox = bed_bounding_box(&window);
    let transform = PackingTransform {
        axis_x: 0.0,
        origin_y: 0.0,
        scale: 57.0,
        vertical: VerticalSense::GravityUp,
    };
    let pixels_per_point = 2.0;
    let rect = bed_quad_rect(&transform, &bbox, pixels_per_point);
    let size_px = pixel_size_of(rect, pixels_per_point);

    let mut samples = Vec::new();
    let mut pebbles = 0;
    for _ in 0..5 {
        let start = Instant::now();
        let (_, drawn) = rasterise_bed(&transform, &window, rect.min, size_px, pixels_per_point);
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        pebbles = drawn;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));
    println!(
        "bake of {pebbles} pebbles into {}x{} px: median {:.2} ms",
        size_px[0],
        size_px[1],
        samples[samples.len() / 2]
    );
    assert_eq!(pebbles, 523);
}

/// The vessel's own aspect fit must not be disturbed by the change — a
/// regression guard on the geometry the bake is derived from.
///
/// **Methodology.** The bed's bounding box is computed from the packing, not
/// from the widget box, so confirm it matches the packing's published
/// `BED_BOUNDS` for the whole-bed window (the window that keeps everything).
///
/// **Results (2026-08-12).** Bounding box `[-1.00090, 0.99984, -0.89900,
/// 2.16819]`, equal to `BED_BOUNDS` to within 1e-4 vessel radii. Interpretation:
/// the baked image spans exactly the artwork the direct path draws, so the two
/// paths place the bed identically.
#[test]
fn the_baked_box_matches_the_packings_published_bounds() {
    use crate::components::pebble_packing::BED_BOUNDS;

    let bbox = bed_bounding_box(&PackingWindow::whole_bed());
    for (got, want) in [
        (bbox.min_x, BED_BOUNDS[0]),
        (bbox.max_x, BED_BOUNDS[1]),
        (bbox.min_y, BED_BOUNDS[2]),
        (bbox.max_y, BED_BOUNDS[3]),
    ] {
        assert!(
            (got - want).abs() < 1.0e-4,
            "baked box {got} does not match the published bound {want}"
        );
    }
    // Sanity: the widget's native aspect is untouched by any of this.
    assert!(NATIVE_ASPECT_RATIO > 0.0);
}
