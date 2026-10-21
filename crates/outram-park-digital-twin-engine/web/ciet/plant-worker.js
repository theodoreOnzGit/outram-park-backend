// The plant worker of the CIET demo (gh:#924): the desktop simulator's physics
// thread, in a Web Worker. It steps the CIET loop (the v2 stepper, compiled
// to wasm), paces it to real time (or faster), and streams the plant to the
// page, which only draws. In three-worker mode each step goes as v2's does
// with its threads: the snapshot of the start-of-step state goes to the DRACS
// and primary flow workers, this worker does the step's heat transfer
// meanwhile, then it takes their flows and finishes the step. In one-worker
// mode the same solves run inline in this worker.
import init, { CietWeb } from "./ciet_web.js";

let plant = null;
let fields = [];
let mode = "three";
let ports = { dracs: null, primary: null };
let speed = 1; // simulated seconds per wall second; 0 = as fast as possible
let paused = false;
let running = false;
let stopped = false;
const queue = []; // controls, applied between steps
let anchor = { wall: 0, sim: 0 };
let nextId = 1;
const pending = new Map();

// Per-window performance: compute time per step, split by part.
let perf = newPerf();
function newPerf() {
  return { steps: 0, stepMs: 0, maxStepMs: 0, heatMs: 0, dracsMs: 0, primaryMs: 0, wall0: performance.now(), sim0: plant ? plant.simulation_time_s() : 0 };
}

function setAnchor() {
  anchor = { wall: performance.now(), sim: plant.simulation_time_s() };
}

function ask(which, snap) {
  return new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    ports[which].postMessage({ id, snap }, [snap.buffer]);
  });
}

function onReply(e) {
  const r = e.data;
  const p = pending.get(r.id);
  if (!p) return;
  pending.delete(r.id);
  if (r.error) p.reject(new Error(r.error));
  else p.resolve(r);
}

async function threeWorkerStep() {
  const dracsSnap = plant.snapshot_dracs();
  const primarySnap = plant.snapshot_primary();
  const d = ask("dracs", dracsSnap);
  const p = ask("primary", primarySnap);
  const h0 = performance.now();
  plant.begin_step();
  perf.heatMs += performance.now() - h0;
  const [rd, rp] = await Promise.all([d, p]);
  plant.finish(rd.result, rp.result[0], rp.result[1], rp.result[2]);
  perf.dracsMs += rd.ms;
  perf.primaryMs += rp.ms;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let lastFrame = 0;

function postFrame(force) {
  const now = performance.now();
  if (!force && now - lastFrame < 50) return;
  lastFrame = now;
  const n = Math.max(perf.steps, 1);
  const simNow = plant.simulation_time_s();
  const stats = {
    mode,
    speed,
    paused,
    steps: perf.steps,
    stepMs: perf.stepMs / n,
    maxStepMs: perf.maxStepMs,
    heatMs: perf.heatMs / n,
    dracsMs: perf.dracsMs / n,
    primaryMs: perf.primaryMs / n,
    // simulated seconds per wall second since the speed was last set (a
    // window of a few seconds holds only tens of 0.1 s steps, too coarse)
    pace: now > anchor.wall ? (simNow - anchor.sim) / ((now - anchor.wall) / 1000) : 0,
    // how much faster than real time this worker could step, from its own
    // compute time per step
    capacity: perf.steps ? (plant.timestep_s() * 1000) / (perf.stepMs / n) : 0,
    behind: speed > 0 ? simTarget() - simNow : 0,
  };
  const readout = plant.readout();
  self.postMessage({ type: "frame", readout, stats }, [readout.buffer]);
  if (now - perf.wall0 > 3000) perf = newPerf();
}

function simTarget() {
  if (speed === 0) return Infinity;
  return anchor.sim + (speed * (performance.now() - anchor.wall)) / 1000;
}

function applyQueued() {
  while (queue.length) {
    const m = queue.shift();
    if (m.type === "set") plant.set(m.name, m.value);
    else if (m.type === "speed") { speed = m.value; setAnchor(); perf = newPerf(); }
    else if (m.type === "pause") { paused = m.on; if (!paused) setAnchor(); perf = newPerf(); }
    else if (m.type === "mode") { mode = m.mode; perf = newPerf(); }
  }
}

async function run() {
  if (running || stopped) return;
  running = true;
  try {
    let lastYield = performance.now();
    for (;;) {
      applyQueued();
      if (paused) { postFrame(true); break; }
      const target = simTarget();
      const sim = plant.simulation_time_s();
      if (sim >= target) {
        // Ahead of the clock: wait until the clock reaches the simulation
        // (then the next step is due, as the desktop loop sleeps).
        postFrame(false);
        const dueWall = anchor.wall + ((sim - anchor.sim) / speed) * 1000;
        await sleep(Math.max(1, dueWall - performance.now()));
        lastYield = performance.now();
        continue;
      }
      const t0 = performance.now();
      if (mode === "three") await threeWorkerStep();
      else {
        plant.advance(1);
        perf.heatMs += performance.now() - t0;
      }
      const dt = performance.now() - t0;
      perf.stepMs += dt;
      perf.maxStepMs = Math.max(perf.maxStepMs, dt);
      perf.steps += 1;
      postFrame(false);
      // Let control messages in now and then when behind or fast-forwarding
      // (three-worker steps already yield while they wait for the flows).
      if (performance.now() - lastYield > 30) {
        await sleep(0);
        lastYield = performance.now();
      }
    }
  } catch (err) {
    stopped = true;
    self.postMessage({ type: "error", message: String(err && err.message ? err.message : err) });
  } finally {
    running = false;
  }
}

self.onmessage = async (ev) => {
  const m = ev.data;
  if (m.type === "init") {
    await init();
    plant = new CietWeb();
    fields = CietWeb.fields().split(",");
    mode = m.mode || "three";
    ports.dracs = m.dracsPort;
    ports.primary = m.primaryPort;
    ports.dracs.onmessage = onReply;
    ports.primary.onmessage = onReply;
    for (const c of m.controls || []) plant.set(c.name, c.value);
    speed = m.speed ?? 1;
    setAnchor();
    perf = newPerf();
    self.postMessage({ type: "ready", fields });
    run();
    return;
  }
  if (!plant) return;
  queue.push(m);
  if (m.type === "pause" && !m.on) setTimeout(run, 0);
  if (!running) setTimeout(run, 0);
};
