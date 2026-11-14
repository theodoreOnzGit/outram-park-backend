// The MSRE demo's worker starts here: it loads the SAME wasm module as the
// page. There is no `window` in a worker, so the module's `main` runs the
// plant (msre::session) instead of the GUI. The page never computes, so it
// never freezes.
import init from "./msre_web.js";
init({ module_or_path: "./msre_web_bg.wasm" });
