// Fission products, chapter 3 "TRISO kernel": the Web Worker for the heavy
// demos (no-lag rule): the Potts coarsening (streams a snapshot every few
// sweeps) and the random walk out of a sphere. Messages in:
//   { id, cmd: "potts", L, q, kT, sweeps, every, seed } -> { id, cmd: "frame", g, sweep, bf } ..., { id, cmd: "done" }
//   { id, cmd: "walk", n, x, steps, seed, keep }        -> { id, cmd: "walk", f, paths }
//   { id, cmd: "stop" } stops a running Potts job.
importScripts("kernel-physics.js");
var K = self.KernelPhysics, current = 0;
self.onmessage = function (e) {
  var m = e.data;
  if (m.cmd === "stop") { current = 0; return; }
  if (m.cmd === "walk") {
    var w = K.walkSphere(m.n, m.x, m.steps, m.seed, m.keep);
    self.postMessage({ id: m.id, cmd: "walk", f: w.f, paths: w.paths });
    return;
  }
  if (m.cmd === "potts") {
    current = m.id;
    var s = K.pottsInit(m.L, m.q, m.seed), k = 0;
    // One slice of sweeps per timer tick, so a stop message gets through.
    (function slice() {
      if (current !== m.id) return;
      for (var j = 0; j < m.every && k < m.sweeps; j++, k++) K.pottsSweep(s, m.kT);
      self.postMessage({ id: m.id, cmd: "frame", g: s.g.slice(), sweep: k, bf: K.boundaryFraction(s) });
      if (k < m.sweeps) setTimeout(slice, 0); else self.postMessage({ id: m.id, cmd: "done" });
    })();
  }
};
