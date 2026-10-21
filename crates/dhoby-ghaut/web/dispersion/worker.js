// Calculation worker for the dispersion demo. It loads the SAME wasm module as
// the page; there is no `window` in a worker, so the module's `main` serves
// the engine instead of the GUI: plume and deposition maps, sigma curves, the
// puff train and the dose curves, each a short request answered in
// milliseconds. The page never blocks, so the map stays live throughout.
import init from "./dispersion_web.js";
init();
