// Physics worker for Foam Studio (gh:#1155). It loads the SAME wasm module as
// the page; there is no `window` in a worker, so the module's `main` serves
// the engine instead of the GUI: meshing (the O-grid / box blockMesh) and
// OUTRAM PARK's pimpleFoam port, one short chunk of time steps per message.
// The page never blocks, so the view stays live. Versioned like the page
// (`__BUILD__`, written by build.sh).
import init from "./foam_studio_web.js?v=__BUILD__";
init({ module_or_path: "./foam_studio_web_bg.wasm?v=__BUILD__" });
