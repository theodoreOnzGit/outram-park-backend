// The EBR-II demo's worker starts here: it loads the SAME wasm module as the
// page. There is no `window` in a worker, so the module's `main` runs the
// plant (ebr2::web's engine) instead of the GUI. The page never computes, so
// it never freezes.
import init from "./ebr2_web.js";
init({ module_or_path: "./ebr2_web_bg.wasm" });
