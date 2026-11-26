// Meshing worker for Mesh Studio (gh:#1767). It loads the SAME wasm module as
// the page; there is no `window` in a worker, so the module's `main` serves
// the engine instead of the GUI: authoring the surface, cfMesh's staged
// tet -> dual -> layers pipeline, and the cut-plane sections. The page never
// blocks, so the view stays live while a mesh is built. Versioned like the
// page (`__BUILD__`, written by build.sh).
import init from "./mesh_studio.js?v=__BUILD__";
init({ module_or_path: "./mesh_studio_bg.wasm?v=__BUILD__" });
