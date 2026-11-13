// Physics worker for the helical-coil steam generator demo (gh:#1157). It
// loads the SAME wasm module as the page; there is no `window` in a worker,
// so the module's `main` serves the engine instead of the GUI: tampines'
// helical-coil OTSG march, one shooting march per message. The page never
// blocks, so the view stays live. Versioned like the page (`__BUILD__`,
// written by build.sh).
import init from "./helical_sg_web.js?v=__BUILD__";
init({ module_or_path: "./helical_sg_web_bg.wasm?v=__BUILD__" });
