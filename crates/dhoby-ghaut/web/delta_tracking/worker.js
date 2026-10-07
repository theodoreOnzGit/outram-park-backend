// Physics worker for the delta-vs-surface tracking demo. It loads the SAME
// wasm module as the page; there is no `window` in a worker, so the module's
// `main` runs the physics engine instead of the GUI: it downloads and
// processes the ENDF tapes, builds the majorant, traces each neutron both
// ways and runs the generations of "Run many", posting results to the page.
// The page never blocks, so it stays responsive throughout.
import init from "./delta_tracking_web.js";
init();
