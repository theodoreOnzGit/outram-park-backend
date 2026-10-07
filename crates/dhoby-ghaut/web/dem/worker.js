// Physics worker for the DEM pour demo (gh:#787). It loads the SAME wasm
// module as the page; there is no `window` in a worker, so the module's
// `main` serves the engine instead of the GUI: the LIGGGHTS port's HTR-10
// pour, one short chunk of steps per message. The page never blocks, so the
// view stays live. Versioned like the page (`__BUILD__`, written by build.sh).
import init from "./dem_web.js?v=__BUILD__";
init({ module_or_path: "./dem_web_bg.wasm?v=__BUILD__" });
