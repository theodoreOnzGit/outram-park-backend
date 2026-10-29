// Every worker of the Fukushima demo starts here: it loads the SAME wasm
// module as the page. There is no `window` in a worker, so the module's
// `main` runs physics instead of the GUI; the worker's name tells it which
// part: unnamed = the plant worker (vessel, operators, pacing), "core" and
// "containment" = the two solver workers the plant worker starts, which step
// from a start-of-step snapshot at the same time (the CIET browser demo's
// pattern). The page never computes, so it never freezes.
import init from "./fukushima_web.js";
init({ module_or_path: "./fukushima_web_bg.wasm" });
