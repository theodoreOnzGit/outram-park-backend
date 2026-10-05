// Calculation worker for the TRISO-ATOPS demo. It loads the SAME wasm module
// as the page; there is no `window` in a worker, so the module's `main` serves
// the engine instead of the GUI: the region slice, decay chains, the
// Walk-on-Spheres ensemble, fuel failure, chemistry and the coolant pools,
// each a short request. The page never blocks, so the view stays live.
import init from "./triso_atops_web.js";
init();
