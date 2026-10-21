// Physics worker for the nuclear data demo. It loads the SAME wasm module as
// the page; there is no `window` in a worker, so the module's `main` runs the
// engine instead of the GUI: it downloads the ENDF tapes, runs RECONR, BROADR,
// PURR, THERMR and the group collapse, and posts each result to the page. The
// page never blocks, so it stays responsive throughout.
import init from "./nuclear_data_web.js";
init();
