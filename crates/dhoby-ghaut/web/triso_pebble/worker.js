// Physics worker for the TRISO pebble demo. It loads the SAME wasm module as
// the page; there is no `window` in a worker, so the module's `main` runs the
// physics engine instead of the GUI: it downloads and processes the ENDF
// tapes and transports the neutrons, and posts progress and finished tracks
// to the page. The page never blocks, so it stays responsive throughout.
import init from "./triso_pebble_web.js";
init();
