//! What one `fhr_sim_v2` frame actually costs, measured rather than guessed.
//!
//! # Why this exists
//!
//! "The simulator feels laggy" is not a diagnosis, and this simulator's lag has
//! already been misattributed once by reasoning instead of measuring (physics
//! was blamed before the thermal-hydraulics loop was found to be at 20 % duty).
//! The side panel's `GuiFrameMetrics` readout (from the engine scaffold,
//! `outram_park_digital_twin_engine::app_scaffold::gui_frame_metrics`) tells a
//! *human* whether a running instance is healthy, but it cannot say **which
//! part** of the render body costs what, and it cannot fail a build when a
//! future change makes the schematic ten times more expensive.
//!
//! This module closes that second gap: it drives the real render bodies --
//! [`FHRSimulatorApp::main_page`], [`FHRSimulatorApp::side_panel`] and
//! [`FHRSimulatorApp::reactor_power_page_graph`] -- in a headless `egui`
//! context, tessellates the result, and reports shapes, vertices, indices and
//! wall clock.
//!
//! # Relationship to the pebble-bed harness
//!
//! The engine library already contains a headless frame-cost harness, written
//! for the pebble-bed texture bake
//! (`src/components/pebble_bed_texture/tests.rs`). It is `#[cfg(test)]`-private
//! to the library and therefore cannot be imported by an example target, so the
//! shape of the measurement here -- `headless_context`, `FrameCost`, warm-up
//! then median-of-N -- is deliberately copied from it so the two sets of numbers
//! are directly comparable. That harness measures **one widget**; this one
//! measures **one application frame**, which is the question the lag report
//! actually asks.
//!
//! # Honest limits
//!
//! These measurements cover shape building plus `egui` tessellation. They do
//! **not** cover GPU upload, draw submission, present, or vsync -- so they bound
//! the CPU side of a frame from above and say nothing about the compositor. The
//! live-application figures below were taken separately, from the running
//! binary on a real display, precisely to close that gap.
//!
//! # Live-application verification (2026-08-12)
//!
//! **Methodology.** The headless numbers bound the `ui()` body but cannot see
//! the compositor, so the released binary was run on a real X11 display and its
//! own `GuiFrameMetrics` readout -- smoothed `update` CPU time, decaying peak
//! CPU, `stable_dt` frame interval, and the implied frame rate -- was sampled
//! every 60th frame to stderr through a temporary print (since removed; the
//! same four figures are permanently visible in the side panel). Three
//! conditions were run in `--release`: idle on the main schematic page, idle on
//! the power-graph page, and under synthetic interaction driven by `xdotool` --
//! a 60-step mouse sweep across the schematic (which repeatedly enters the
//! reactor vessel's hover region and raises its tooltip) followed by a 60-step
//! drag of the left control-rod slider. Display: 2560x1440 at **59.95 Hz**;
//! window 2560x1363; 32 CPU cores, so the three physics threads cannot starve
//! the GUI thread. Pass criterion: sustained frame rate at the display's refresh
//! rate with CPU time a small fraction of the frame interval, which is the
//! signature of a vsync-limited (healthy) renderer per the `gui_frame_metrics`
//! reading table.
//!
//! **Results.**
//!
//! | Condition | `update` CPU | Peak CPU | Frame interval | Frame rate |
//! |---|---|---|---|---|
//! | Idle, main schematic page | 0.14-0.55 ms | < 0.81 ms | 16.61-16.70 ms | 60.0 fps |
//! | Idle, power-graph page | 0.34-0.44 ms | < 0.52 ms | 16.51-16.73 ms | 60.0 fps |
//! | Mouse sweep + slider drag | 0.14-0.23 ms | < 0.41 ms | 16.61-16.72 ms | 60.0 fps |
//!
//! First frame 4.63 ms (font atlas plus the pebble-bed bake). Across roughly
//! 2 760 sampled frames, two intervals exceeded 18 ms -- about 0.07 % of frames,
//! i.e. two dropped frames, both isolated.
//!
//! **Interpretation.** The simulator renders at exactly the display's refresh
//! rate, and spends **1.4 % of each frame** in its own `ui()` body even while
//! being interacted with; the remaining 98.6 % is idle waiting for vsync. This
//! is the first row of the `gui_frame_metrics` reading table -- "healthy,
//! vsync-limited" -- and not the third ("the app's own render is the
//! bottleneck"). **No further render-side optimisation of this simulator can
//! raise its frame rate**, because deleting the entire render body would recover
//! 0.23 ms out of a 16.67 ms frame that is hardware-capped at 59.95 Hz. Any
//! remaining perception of lag is therefore not frame rate and must be looked
//! for elsewhere -- most plausibly in how quickly the *plant* responds to a
//! control input, which is a physics-timescale question, not a rendering one.
//!
//! These figures are machine- and display-specific and should be re-measured
//! rather than quoted; the shape counts asserted below are deterministic and are
//! what guards against a regression.

use crate::app::graph_data::PagePlotData;
use crate::{FHRSimulatorApp, FHRState};
use egui::{Pos2, Rect, Vec2};
use std::hint::black_box;
use std::time::Instant;

/// Logical screen the benchmark frames are laid out in \[points\].
///
/// Matches the `with_inner_size` the simulator asks for in `main.rs`, so the
/// panels get the space they really get.
const SCREEN: Vec2 = Vec2::new(1920.0, 1080.0);

/// A headless `egui` context configured the way a desktop backend configures
/// one, so texture loading and tessellation behave as they do in the app.
///
/// Runs one warm-up frame so the font atlas exists before anything is measured.
fn headless_context() -> (egui::Context, egui::RawInput) {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        max_texture_side: Some(8192),
        ..Default::default()
    };
    let _ = ctx.run_ui(input.clone(), |_| {});
    (ctx, input)
}

/// Tessellated cost of one frame: vertices, indices, shapes, and the wall clock
/// for building plus tessellating them \[ms\].
#[derive(Clone, Copy, Debug, Default)]
struct FrameCost {
    vertices: usize,
    indices: usize,
    shapes: usize,
    milliseconds: f64,
}

/// Which part of the render body a benchmark frame exercises.
///
/// An enum rather than a callback, per this workspace's Rust design rules: the
/// set of panels the simulator can show is closed and known at compile time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    /// The schematic alone: reactor vessel, ~40 pipe widgets, the turbine.
    MainPage,
    /// The right-hand control and diagnostics panel alone.
    SidePanel,
    /// The "Power Diagnostics" plot page alone.
    PowerGraphPage,
    /// Everything the default view draws: side panel plus schematic, nested in
    /// the same panels and scroll areas the real `ui()` body uses.
    WholeMainView,
}

/// Runs one frame that draws `part` and returns its cost.
fn measure(
    ctx: &egui::Context,
    input: &egui::RawInput,
    app: &mut FHRSimulatorApp,
    part: Part,
) -> FrameCost {
    let start = Instant::now();
    let output = ctx.run_ui(input.clone(), |ui| match part {
        Part::MainPage => app.main_page(ui),
        Part::SidePanel => app.side_panel(ui),
        Part::PowerGraphPage => app.reactor_power_page_graph(ui),
        Part::WholeMainView => {
            egui::Panel::right("Supplementary Info").show(ui, |ui| {
                app.side_panel(ui);
            });
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::both().show(ui, |ui| app.main_page(ui));
            });
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

/// Warms up, then reports the median of 30 timed frames.
///
/// The warm-up matters: the first frame that draws the vessel pays the
/// pebble-bed texture bake, which is a one-off (see
/// [`the_first_frame_pays_the_pebble_bed_bake_and_no_later_frame_does`]).
fn steady_state(
    ctx: &egui::Context,
    input: &egui::RawInput,
    app: &mut FHRSimulatorApp,
    part: Part,
) -> FrameCost {
    for _ in 0..3 {
        let _ = measure(ctx, input, app, part);
    }
    let mut samples = Vec::new();
    let mut cost = FrameCost::default();
    for _ in 0..30 {
        cost = measure(ctx, input, app, part);
        samples.push(cost.milliseconds);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));
    cost.milliseconds = samples[samples.len() / 2];
    cost
}

/// Shared state carrying node vectors of the length the *running* physics
/// publishes, rather than the one-element placeholders [`FHRState::default`]
/// starts from.
///
/// This matters only for the clone-cost question: a frame that reads a
/// 1-element vector and a frame that reads a 10-element one draw the same
/// picture (each pipe widget renders the vector's mean), but they do not
/// allocate the same amount. Lengths follow the thermal-hydraulics loop's own
/// nodalisation -- 2 nodes for a pipe or pump, 10 for a heat-exchanger side.
fn state_with_realistic_node_counts() -> FHRState {
    let mut state = FHRState::default();
    let profile = |n: usize| (0..n).map(|i| 500.0 + i as f64).collect::<Vec<f64>>();

    state.pipe_4_temperature_vector_degc = profile(2);
    state.pipe_5_temperature_vector_degc = profile(2);
    state.ihx_shell_6_temperature_vector_degc = profile(10);
    state.pipe_7_temperature_vector_degc = profile(2);
    state.pipe_8_temperature_vector_degc = profile(2);
    state.pri_pump_9_temperature_vector_degc = profile(2);
    state.pipe_10_temperature_vector_degc = profile(2);
    state.pipe_11_temperature_vector_degc = profile(2);
    state.ihx_tube_6_temperature_vector_degc = profile(10);
    state.pipe_12_temperature_vector_degc = profile(2);
    state.pipe_13_temperature_vector_degc = profile(2);
    state.sg_shell_14_temperature_vector_degc = profile(10);
    state.pipe_15_temperature_vector_degc = profile(2);
    state.intrmd_pump_16_temperature_vector_degc = profile(2);
    state.pipe_17_temperature_vector_degc = profile(2);
    state
}

/// An application ready to render, with realistic shared state and **no physics
/// threads**.
///
/// [`FHRSimulatorApp::default`] is used rather than
/// [`FHRSimulatorApp::new`] deliberately: `new` spawns the PRKE,
/// thermal-hydraulics and plot-updater threads, which would make every reading
/// here depend on whatever those threads happened to be doing. The render body
/// is what is under measurement.
fn app_ready_to_render() -> FHRSimulatorApp {
    let app = FHRSimulatorApp::default();
    *app.fhr_state.lock().unwrap() = state_with_realistic_node_counts();
    app
}

/// **The measurement the lag report exists for**: the whole render body costs a
/// small fraction of one frame, so the GUI is vsync-limited rather than
/// render-bound.
///
/// **Methodology.** A headless `egui::Context` at 1920x1080 points and 1x device
/// pixel ratio drives the simulator's real render bodies. Each part is built and
/// then tessellated with `Context::tessellate`, and the reported cost is the
/// wall clock for **both** steps -- that is the whole CPU contribution of the
/// application's own `ui()` body, which is what `GuiFrameMetrics`'
/// `update_cpu_time_ms` reports at runtime and what the pebble-bed bake
/// reduced. Three warm-up frames precede 30 timed frames; the median is
/// reported. Vertex, index and shape counts are exact rather than timed, so they
/// are the primary evidence and the timing corroborates.
/// Pass criterion: the whole default view must stay under 2 ms (about an eighth
/// of a 60 Hz frame) and must not regress by an order of magnitude in shape
/// count.
///
/// **Results (2026-08-12, release build, this machine).** Printed by this test;
/// counts are exact and reproduced identically on every run, timings are the
/// range of medians over four consecutive runs:
///
/// | Part | Shapes | Vertices | Indices | Build + tessellate |
/// |---|---|---|---|---|
/// | Side panel | 123 | 6 256 | 15 024 | 0.084-0.102 ms |
/// | Main page (schematic) | 189 | 2 236 | 9 708 | 0.038-0.043 ms |
/// | Whole main view | 313 | 7 952 | 23 172 | 0.119-0.129 ms |
/// | Power-graph page | 59 | 65 408 | 291 390 | 0.714-0.759 ms |
///
/// **Interpretation.** The whole default view costs **about 0.13 ms against a
/// 16.67 ms frame budget -- 0.8 % duty.** The schematic, which is where a
/// "laggy simulator" report would naturally point, is the *cheapest* part of the
/// frame at 0.041 ms; the side panel costs twice as much as the entire
/// schematic, because ~15 temperature-legend buttons and ~40 text labels
/// tessellate more glyph geometry than 40 single-line pipes and one baked
/// vessel do. The pebble bed no longer appears in these numbers at all: it was
/// baked to three textured quads on 2026-08-12, taking the FHR vessel from
/// 9 909 shapes / 0.53 ms to 32 shapes / 0.013 ms.
///
/// The plot page is the most expensive part measured, at about 0.74 ms, and it
/// is still only 4.4 % of a frame. Its cost is 8 000 plotted points
/// (`NUM_DATA_PTS_IN_PLOTS` = 4 000, two series), which is genuinely dynamic
/// data and correctly not cached.
///
/// **This test therefore indicts nothing.** There is no static artwork left to
/// bake and no per-frame work worth caching: an optimisation that removed the
/// *entire* render body could not raise the frame rate, because 99.2 % of each
/// frame is already spent idle waiting for vsync. Confirmed against the running
/// binary -- see the module-level hand-off and
/// [`cloning_the_whole_fhr_state_is_not_the_bottleneck`].
#[test]
fn the_whole_render_body_costs_a_small_fraction_of_a_frame() {
    let (ctx, input) = headless_context();
    let mut app = app_ready_to_render();

    let mut whole_main_view = FrameCost::default();
    for part in [
        Part::SidePanel,
        Part::MainPage,
        Part::WholeMainView,
        Part::PowerGraphPage,
    ] {
        let cost = steady_state(&ctx, &input, &mut app, part);
        println!(
            "{part:?}: {:>6} shapes, {:>7} vertices, {:>7} indices, {:.3} ms",
            cost.shapes, cost.vertices, cost.indices, cost.milliseconds
        );
        if part == Part::WholeMainView {
            whole_main_view = cost;
        }
    }

    // A 60 Hz frame is 16.67 ms. Two milliseconds is roughly an eighth of that
    // and leaves a very wide margin over the 0.128 ms measured above, so this
    // fails only on a real regression, not on a slow CI machine.
    assert!(
        whole_main_view.milliseconds < 2.0,
        "the default view now costs {:.3} ms of a 16.67 ms frame",
        whole_main_view.milliseconds
    );

    // The count guard is the deterministic one. The pebble bed alone used to
    // emit 9 880 circles here; anything approaching that means a bake has been
    // lost or a new pathological widget has been added.
    assert!(
        whole_main_view.shapes < 2_000,
        "the default view now emits {} shapes -- has a texture bake been lost?",
        whole_main_view.shapes
    );
}

/// Cloning the whole [`FHRState`] twice per frame is real waste, and it is
/// **not** the cause of any perceptible lag.
///
/// **Methodology.** `op-szmi.9` records that both `app/mod.rs` and
/// `side_panel.rs` clone the entire `FHRState` every repaint to read a handful
/// of scalars, and that the struct carries 15 `Vec<f64>` node-temperature
/// vectors -- roughly 30 heap allocations plus memcpy per frame, about 1 800
/// allocations per second at 60 fps. The bead explicitly says to measure before
/// assuming this is the cause. So: clone the state 10 000 times with realistic
/// node counts (2 nodes per pipe/pump, 10 per heat-exchanger side -- see
/// [`state_with_realistic_node_counts`]), take the mean, and express it as a
/// fraction of the 16.67 ms frame budget. `std::hint::black_box` guards both the
/// input and the result so the optimiser cannot delete the clone; without it the
/// same loop reports an implausible 0.001 us, which is the measurement being
/// elided rather than the work being fast. [`PagePlotData`] -- 4 000 points,
/// cloned once per frame by the plot page -- is measured the same way.
/// Pass criterion: report the numbers, and fail only if a clone costs more than
/// 1 % of a frame, which is the threshold at which it would be worth fixing.
///
/// **Results (2026-08-12, release build, this machine).** One `FHRState` clone
/// costs **0.142-0.190 us** over four runs. The GUI performs two per frame, so
/// the total per-frame cost of the waste `op-szmi.9` describes is **0.28-0.38 us
/// out of 16 670 us -- 0.0017-0.0023 % of a frame.** One `PagePlotData` clone
/// (4 000 tuples of three `uom` quantities, 96 kB) costs 1.6-2.5 us, paid only
/// while the plot page is open. Without `black_box` the same two loops report
/// 0.107 us and 0.001 us respectively -- the second is three orders of magnitude
/// too fast and is the optimiser deleting the clone, which is why the guards are
/// there.
///
/// **Interpretation.** `op-szmi.9` is correctly identified as waste and is
/// wrongly identified as a lag suspect. Removing it entirely would recover two
/// ten-thousandths of one percent of the frame, which is unmeasurable on the
/// side panel's own readout: the smoothed CPU figure fluctuates by ±0.1 ms
/// between frames, roughly five hundred times the size of the effect. The
/// recommendation is therefore to close `op-szmi.9` as measured-and-declined
/// rather than to change working code for no observable gain -- and this test
/// exists so that recommendation rests on a number instead of an argument.
/// (Deciding to close it is the maintainer's, not this test's.)
#[test]
fn cloning_the_whole_fhr_state_is_not_the_bottleneck() {
    /// One 60 Hz frame \[us\].
    const FRAME_BUDGET_US: f64 = 1.0e6 / 60.0;

    let app = app_ready_to_render();
    let state = app.fhr_state.lock().unwrap().clone();

    let rounds = 10_000;
    let start = Instant::now();
    for _ in 0..rounds {
        let clone = black_box(&state).clone();
        black_box(clone);
    }
    let per_clone_us = start.elapsed().as_secs_f64() * 1.0e6 / rounds as f64;

    // The GUI clones it twice per repaint: once in `main_page`, once in
    // `side_panel`.
    let per_frame_us = 2.0 * per_clone_us;
    let percent_of_frame = 100.0 * per_frame_us / FRAME_BUDGET_US;
    println!(
        "FHRState clone: {per_clone_us:.3} us each, {per_frame_us:.3} us per frame \
         ({percent_of_frame:.4} % of a 60 Hz frame)"
    );

    let plot = PagePlotData::default();
    let rounds = 2_000;
    let start = Instant::now();
    for _ in 0..rounds {
        let clone = black_box(&plot).clone();
        black_box(clone);
    }
    let per_plot_clone_us = start.elapsed().as_secs_f64() * 1.0e6 / rounds as f64;
    println!(
        "PagePlotData clone ({} points): {per_plot_clone_us:.3} us each",
        plot.reactor_power_plot_data.len()
    );

    assert!(
        percent_of_frame < 1.0,
        "the per-frame state clone now costs {percent_of_frame:.3} % of a frame -- \
         it has become worth fixing"
    );
}

/// The pebble-bed bake is paid once, on the first frame, and never again.
///
/// **Methodology.** The honest cost of the texture bake that removed the pebble
/// artwork is a slow *first* frame (and a slow frame after each resize, since
/// the cache key tracks physical pixel size). Measure the very first frame that
/// draws the schematic against the median of the 30 that follow it, at one
/// fixed size. There is no wall-clock pass criterion -- that is
/// machine-dependent -- only that the steady state is much cheaper than the
/// first frame, which is the property being claimed.
///
/// **Results (2026-08-12, release build, this machine).** First schematic frame
/// 1.78-1.91 ms; steady-state median 0.042-0.065 ms, a 29-44x drop, over four
/// runs. The running binary shows the same shape: its very first frame measured
/// 4.63 ms on the side panel's own readout (a larger figure because that frame
/// also builds the font atlas and every other panel), and every subsequent frame
/// sat between 0.14 and 0.55 ms.
///
/// **Interpretation.** The bake costs roughly one dropped frame at start-up and
/// one per resize, and buys back the 0.5 ms per frame the direct-circle path
/// used to cost, so it amortises within about four frames (0.07 s) and is repaid
/// many thousand times over in a session. This is recorded rather than hidden: a
/// user dragging the window edge continuously will see the schematic re-bake,
/// and that is a real, accepted tradeoff.
#[test]
fn the_first_frame_pays_the_pebble_bed_bake_and_no_later_frame_does() {
    let (ctx, input) = headless_context();
    let mut app = app_ready_to_render();

    let first = measure(&ctx, &input, &mut app, Part::MainPage);
    let steady = steady_state(&ctx, &input, &mut app, Part::MainPage);

    println!(
        "schematic: first frame {:.3} ms, steady-state median {:.3} ms ({:.0}x)",
        first.milliseconds,
        steady.milliseconds,
        first.milliseconds / steady.milliseconds.max(1.0e-9)
    );

    assert!(
        steady.milliseconds < first.milliseconds,
        "the first frame ({:.3} ms) should be the expensive one, not the steady state ({:.3} ms)",
        first.milliseconds,
        steady.milliseconds
    );
    // The bake must actually be cached: a steady-state frame that still cost
    // milliseconds would mean the cache key is missing every frame.
    assert!(
        steady.milliseconds < 1.0,
        "steady-state schematic costs {:.3} ms -- is the bed cache missing every frame?",
        steady.milliseconds
    );
}
