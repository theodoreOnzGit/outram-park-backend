// A flow worker of the CIET demo (gh:#924). The desktop simulator (v2) solves
// the DRACS loop flow and the primary loop flows on threads of their own
// beside the heat transfer; here each runs on a Web Worker of its own. This
// worker holds a CIET loop built as the plant worker's is, and for each
// snapshot the plant worker sends (the fluid temperatures of the components
// its solve reads, plus the pump and valve settings) it sets them and solves.
// That this gives the same flows as solving in the step, bit for bit, is
// tested natively (examples/ciet_web, `the_three_worker_step_is_the_one_worker_step`).
import init, { CietFlow } from "./ciet_web.js";

let kind = null; // "dracs" or "primary"
let solver = null;
let port = null;

const ready = init();

self.onmessage = async (ev) => {
  const m = ev.data;
  if (m.type !== "init") return;
  kind = m.kind;
  port = m.port;
  await ready;
  solver = new CietFlow();
  port.onmessage = (e) => {
    const q = e.data;
    if (q.type === "reset") {
      solver = new CietFlow();
      return;
    }
    const t0 = performance.now();
    try {
      const result = kind === "dracs" ? solver.dracs(q.snap) : Array.from(solver.primary(q.snap));
      port.postMessage({ id: q.id, result, ms: performance.now() - t0 });
    } catch (err) {
      port.postMessage({ id: q.id, error: String(err), ms: performance.now() - t0 });
    }
  };
  self.postMessage({ type: "ready", kind });
};
