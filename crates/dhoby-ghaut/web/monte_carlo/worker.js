// Physics worker for the Monte Carlo demo. It loads the SAME wasm module as
// the page; there is no `window` in a worker, so the module's `main` runs the
// physics engine instead of the GUI: it downloads and processes the ENDF
// tapes, transports the neutrons and runs the power iteration, and posts
// progress, finished tracks and generations to the page. The page never
// blocks, so it stays responsive throughout.
import init from "./monte_carlo_web.js";
init();
