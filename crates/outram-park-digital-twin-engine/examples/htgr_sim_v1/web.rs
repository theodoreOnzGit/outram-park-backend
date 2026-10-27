//! `htgr_sim_v1` in the browser (gh:#986).
//!
//! The same simulator, the same widgets and the same physics as the desktop
//! example — only the platform layer differs. This module is that layer, and
//! nothing else: it owns the canvas, the Web Worker and the messages between
//! them, and it adds no model.
//!
//! ## Threads, natively and in the browser
//!
//! | desktop | browser |
//! |---|---|
//! | GUI thread draws | the page draws (the same egui widgets, on a canvas) |
//! | `spawn_physics_thread_monitored` steps and paces to real time | a **Web Worker** steps; JavaScript paces it |
//! | plot-sampler thread | the same worker samples, beside the step |
//!
//! `wasm32` has no `std::thread`, `Instant` panics there, and the workspace's
//! **no-lag rule** forbids stepping a plant on the thread that draws. So the
//! physics goes to a worker. The page never computes, so it never freezes:
//! pan, zoom and the sliders stay live while the plant steps.
//!
//! This is the arrangement the CIET and Fukushima browser demos already use.
//! It is possible here without touching the model because `runtime.rs` was
//! written to own no thread, no clock and no pacing policy, and
//! [`crate::app::WebPhysics`] is the native physics closure with those three
//! removed.
//!
//! ## One module, three starts
//!
//! The page and the worker load the **same** wasm module. A worker has no
//! `window`, which is how [`start`] tells them apart — the Fukushima demo's
//! trick. So there is one build, not two, and the physics in the worker is
//! the physics the page would have run.
//!
//! ## What crosses the boundary
//!
//! The whole [`HtgrSnapshot`] each way, as JSON: the worker's outputs to the
//! page, the page's controls to the worker. That mirrors the native design,
//! where one shared snapshot has two writers — the GUI writes the control
//! fields and the physics writes the rest. JSON rather than a packed `f64`
//! wire format because the snapshot has over a hundred fields and a
//! hand-maintained field order would be wrong the first time someone added a
//! control; at ten messages a second the cost is irrelevant and the
//! correctness is not.
//!
//! Research and education only (`RESPONSIBLE_USE.md`), and an offline
//! demonstration: nothing here connects to a plant. The model is the demo
//! model, not validated.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::wasm_bindgen::prelude::*;
use eframe::wasm_bindgen::JsCast as _;

use crate::app::state::{HtgrPlotData, HtgrSnapshot};
use crate::app::{HtgrSimApp, WebPhysics, Wire};
use outram_park_digital_twin_engine::app_scaffold::SharedState;

/// The script each worker starts from; it loads this same module.
pub const WORKER_JS: &str = "./htgr_worker.js";

/// Milliseconds between physics ticks in the worker.
///
/// The desktop `PHYSICS_TICK` is 100 ms and this matches it, so the browser
/// paces the plant the same way the desktop does.
const TICK_MS: i32 = 100;

/// Milliseconds between the page's control messages to the worker.
///
/// Controls are not time-critical — natively they are picked up at the next
/// physics tick — so this need not be faster than the tick.
const CONTROL_MS: i32 = 100;

/// The entry point, on the page and in the worker alike.
pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    install_panic_hook();
    if js_sys::global()
        .dyn_ref::<web_sys::DedicatedWorkerGlobalScope>()
        .is_some()
    {
        worker_main();
    } else {
        page_main();
    }
}

// ─── In the worker: the plant ───────────────────────────────────────────────

/// Step the plant forever, paced by `setTimeout`, posting each snapshot.
fn worker_main() {
    let scope: web_sys::DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let physics = Rc::new(RefCell::new(WebPhysics::new()));

    // The page's control inputs.
    {
        let physics = physics.clone();
        let on_msg = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |ev: web_sys::MessageEvent| {
                let bytes = to_bytes(&ev.data());
                if bytes.is_empty() {
                    return;
                }
                // A malformed message must not kill the worker and freeze
                // the demo with no diagnosis; log and carry on with the
                // controls already held.
                if let Err(e) = physics.borrow_mut().apply_page_state(&bytes) {
                    log::warn!("htgr worker: ignoring a control message: {e}");
                }
            },
        );
        scope.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();
    }

    // The tick. `setTimeout` rather than `setInterval` so a slow tick delays
    // the next one instead of queueing up behind it.
    let last = Rc::new(RefCell::new(0.0_f64));
    let tick: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let tick_clone = tick.clone();
    let scope_for_tick = scope.clone();
    *tick.borrow_mut() = Some(Closure::<dyn FnMut()>::new(move || {
        let now = scope_for_tick
            .performance()
            .map_or(0.0, |p| p.now());
        let wall_ms = {
            let mut last = last.borrow_mut();
            let d = if *last > 0.0 { now - *last } else { 0.0 };
            *last = now;
            d
        };

        {
            let mut p = physics.borrow_mut();
            p.tick(wall_ms);
            let bytes = p.to_wire();
            let _ = scope_for_tick.post_message(&js_sys::Uint8Array::from(&bytes[..]));
        }

        if let Some(cb) = tick_clone.borrow().as_ref() {
            let _ = scope_for_tick.set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                TICK_MS,
            );
        }
    }));
    if let Some(cb) = tick.borrow().as_ref() {
        let _ = scope.set_timeout_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            TICK_MS,
        );
    }
    // The closures outlive this function by design: the worker's whole life
    // is these callbacks.
    std::mem::forget(tick);
}

// ─── On the page: the widgets ───────────────────────────────────────────────

/// Start the egui app on the canvas and the worker behind it.
fn page_main() {
    let web_options = eframe::WebOptions::default();
    // Filled in by the app creator below, then read by the worker's message
    // handler. `Rc<RefCell<..>>` and not a channel because the page is
    // single-threaded and everything here runs on it.
    let handles: Rc<RefCell<Option<(SharedState<HtgrSnapshot>, SharedState<HtgrPlotData>)>>> =
        Rc::new(RefCell::new(None));
    let creator_handles = handles.clone();

    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("htgr_canvas")
            .expect("no #htgr_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#htgr_canvas is not a canvas");

        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| {
                    egui_extras::install_image_loaders(&cc.egui_ctx);
                    let app = HtgrSimApp::new(cc);
                    *creator_handles.borrow_mut() =
                        Some((app.physics_handle(), app.plots_handle()));
                    Ok(Box::new(app))
                }),
            )
            .await;

        match started {
            Ok(()) => {
                if let Some(el) = document.get_element_by_id("loading") {
                    el.remove();
                }
                if let Err(e) = wire_worker(handles) {
                    log::error!("htgr: the physics worker did not start: {e:?}");
                }
            }
            Err(e) => {
                log::error!("htgr: eframe did not start: {e:?}");
                if let Some(el) = document.get_element_by_id("loading") {
                    el.set_text_content(Some(
                        "This demo needs WebGPU or WebGL. Try a current Chrome, Edge or Firefox.",
                    ));
                }
            }
        }
    });
}

/// Start the physics worker and wire the two directions of messages.
fn wire_worker(
    handles: Rc<RefCell<Option<(SharedState<HtgrSnapshot>, SharedState<HtgrPlotData>)>>>,
) -> Result<(), JsValue> {
    let opts = web_sys::WorkerOptions::new();
    opts.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options(WORKER_JS, &opts)?;

    // Worker -> page: the plant snapshot, straight into the shared state the
    // widgets already read.
    {
        let handles = handles.clone();
        let on_msg =
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
                let bytes = to_bytes(&ev.data());
                if bytes.is_empty() {
                    return;
                }
                let Ok(wire) = postcard::from_bytes::<Wire>(&bytes) else {
                    log::warn!("htgr page: a snapshot did not parse");
                    return;
                };
                let mut incoming = wire.snap;
                // The two fields that could not travel inside the snapshot.
                incoming.stability_class = pasquill_class(&wire.stability_class);
                for (slot, name) in incoming.release.iter_mut().zip(&wire.nuclide_names) {
                    slot.name = intern_nuclide_name(name);
                }
                if let Some((physics, plots)) = handles.borrow().as_ref() {
                    // Keep the page's own control fields: the operator may
                    // have moved a slider since this snapshot was computed,
                    // and the worker will have it at the next control
                    // message. Everything else is the worker's to publish.
                    physics.update(|s| {
                        let controls = crate::app::plant_commands_from(s);
                        let rod = s.control_rod_insertion_fraction;
                        let flow = s.helium_flow_setpoint_kg_per_s;
                        let tripped = s.circulator_tripped;
                        let ingress = s.water_ingress_triggered;
                        let rps = s.rps_enabled;
                        let ff = s.fast_forward_enabled;
                        let manual = s.feedwater_manual;
                        *s = incoming.clone();
                        let _ = controls;
                        s.control_rod_insertion_fraction = rod;
                        s.helium_flow_setpoint_kg_per_s = flow;
                        s.circulator_tripped = tripped;
                        s.water_ingress_triggered = ingress;
                        s.rps_enabled = rps;
                        s.fast_forward_enabled = ff;
                        s.feedwater_manual = manual;
                    });
                    plots.update(|p| p.push_sample(&incoming));
                }
            });
        worker.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();
    }

    // Page -> worker: the controls, on a timer.
    {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let worker_for_controls = worker.clone();
        let send = Closure::<dyn FnMut()>::new(move || {
            if let Some((physics, _)) = handles.borrow().as_ref() {
                let wire = Wire::from_snapshot(&physics.snapshot());
                let bytes = postcard::to_allocvec(&wire).unwrap_or_default();
                let _ = worker_for_controls
                    .post_message(&js_sys::Uint8Array::from(&bytes[..]));
            }
        });
        window.set_interval_with_callback_and_timeout_and_arguments_0(
            send.as_ref().unchecked_ref(),
            CONTROL_MS,
        )?;
        send.forget();
    }

    std::mem::forget(worker);
    Ok(())
}

/// A `Uint8Array` (or anything array-like) from `postMessage`, as bytes.
///
/// Returns empty for anything else, which the callers treat as "ignore this
/// message" — a stray or malformed post must not take the demo down.
fn to_bytes(data: &JsValue) -> Vec<u8> {
    if let Some(arr) = data.dyn_ref::<js_sys::Uint8Array>() {
        arr.to_vec()
    } else {
        Vec::new()
    }
}

/// Map a Pasquill stability letter back to the `&'static str` the snapshot
/// field needs. Six letters and the empty "none yet", so a match does it
/// and nothing is leaked.
fn pasquill_class(s: &str) -> &'static str {
    match s {
        "A" => "A",
        "B" => "B",
        "C" => "C",
        "D" => "D",
        "E" => "E",
        "F" => "F",
        _ => "",
    }
}

/// Leak a nuclide name once, so later snapshots reuse the same `&'static str`.
///
/// Called ten times a second with the same handful of names, so the cache is
/// what makes this bounded rather than a leak that grows for as long as the
/// page is open.
fn intern_nuclide_name(name: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};

    static CACHE: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    if name.is_empty() {
        return "";
    }
    let cache = CACHE.get_or_init(|| Mutex::new(HashSet::new()));
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = cache.get(name) {
        return found;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cache.insert(leaked);
    leaked
}

/// Report a panic to the console with its message and location.
///
/// Without a hook, a wasm panic surfaces in the browser as a bare
/// `RuntimeError: unreachable` with no message, no payload and no location —
/// which is exactly as much use as it sounds. This is the cheap half of what
/// `console_error_panic_hook` does, without taking the dependency: the
/// payload and the `Location` are both in `PanicHookInfo`, and `log` is
/// already wired to the console by `WebLogger`.
fn install_panic_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic with a non-string payload".to_owned());
            match info.location() {
                Some(l) => log::error!("PANIC at {}:{}:{} — {msg}", l.file(), l.line(), l.column()),
                None => log::error!("PANIC — {msg}"),
            }
        }));
    });
}
