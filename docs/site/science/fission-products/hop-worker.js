// Fission products, lessons 8-9: the Web Worker that runs the kinetic Monte
// Carlo of hop-physics.js off the UI thread (no-lag rule,
// docs/claude-md/mobile-first-tutorials-and-demos.md). Messages in:
//   { id, cmd: "cloud", p, n, t, seed }       -> { id, cmd, xs, ys, msd, D, hops }
//   { id, cmd: "sweep", kind, p, Ts, size, hops, seed }
//        -> { id, cmd: "point", i, pt } per temperature, then { id, cmd: "sweep", fit, points }
// Each temperature is one short computation, posted as soon as it is done.
importScripts("hop-physics.js");
var H = self.HopPhysics;
self.onmessage = function (e) {
  var m = e.data;
  if (m.cmd === "cloud") {
    var c = H.cloud(m.p, m.n, m.t, m.seed, 2e5);
    self.postMessage({ id: m.id, cmd: "cloud", xs: c.xs, ys: c.ys, msd: c.msd, D: c.D, hops: c.hops });
  } else if (m.cmd === "sweep") {
    var r = H.sweep(m.kind, m.p, m.Ts, m.size, m.hops, m.seed, function (i, pt) {
      self.postMessage({ id: m.id, cmd: "point", i: i, pt: pt });
    });
    self.postMessage({ id: m.id, cmd: "sweep", fit: r.fit, points: r.points });
  }
};
