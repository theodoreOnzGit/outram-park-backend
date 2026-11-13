// Physics worker for the double-pipe twin demo (gh:#1182). It
// loads the SAME wasm module as the page; there is no `window` in a worker,
// so the module's `main` serves the engine instead of the GUI: the TUAS 1-D
// double pipe's UA along one design slice per message. The page never
// blocks, so the view stays live. Versioned like the page (`__BUILD__`,
// written by build.sh).
import init from "./double_pipe_twin_web.js?v=__BUILD__";
init({ module_or_path: "./double_pipe_twin_web_bg.wasm?v=__BUILD__" });
