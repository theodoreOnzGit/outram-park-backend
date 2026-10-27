// The HTGR demo's physics worker (gh:#1030).
//
// It loads the SAME wasm module as the page. A worker has no `window`, so
// the module's entry point runs the plant instead of the GUI — the trick the
// Fukushima demo uses, which keeps one build rather than two and guarantees
// the physics in here is the physics the page would otherwise have run.
//
// The page never computes: it draws the egui widgets and nothing else, so it
// stays responsive while the plant steps (the workspace's no-lag rule).
import init from "./htgr_sim_v1.js";
init({ module_or_path: "./htgr_sim_v1_bg.wasm" });
