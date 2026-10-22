// Fission products, chapter 4: the Web Worker for lesson 32 (no-lag rule).
// The layered walks run here and stream back, one temperature at a time.
// Messages in:
//   { id, cmd: "sweep", radii, temps: [tC...], D: [[5 D]...], n, seed, maxHops }
//       -> { id, cmd: "point", tC, mean, se, capped, hops } ... { id, cmd: "done" }
//   { id, cmd: "tracks", radii, D: [5 D], k, seed, maxHops }
//       -> { id, cmd: "tracks", tracks: [[[t, r]...]...], out: [bool...] }
//   { id, cmd: "stop" } abandons a running sweep.
importScripts("barrier-physics.js");
var B = self.BarrierPhysics, current = 0;
self.onmessage = function (e) {
  var m = e.data;
  if (m.cmd === "stop") { current = 0; return; }
  if (m.cmd === "tracks") {
    var S = B.shells(m.radii, m.D, B.CELLS), r = B.rng(m.seed), tracks = [], out = [];
    for (var i = 0; i < m.k; i++) {
      // Sample a point every few hops, keeping each track to ~4000 points.
      var w = B.walkAtom(S, B.CELLS[0], r, m.maxHops, 1);
      var tr = w.track, step = Math.max(1, Math.floor(tr.length / 4000)), thin = [];
      for (var j = 0; j < tr.length; j += step) thin.push(tr[j]);
      if (thin[thin.length - 1] !== tr[tr.length - 1]) thin.push(tr[tr.length - 1]);
      tracks.push(thin); out.push(w.out);
    }
    self.postMessage({ id: m.id, cmd: "tracks", tracks: tracks, out: out });
    return;
  }
  if (m.cmd === "sweep") {
    current = m.id;
    var k = 0;
    // One temperature per timer tick, so a stop gets through between them.
    (function next() {
      if (current !== m.id) return;
      if (k >= m.temps.length) { self.postMessage({ id: m.id, cmd: "done" }); return; }
      var res = B.latticeWalk(m.radii, m.D[k], m.n, m.seed + k, m.maxHops);
      var mu = B.mean(res.times), v = 0; res.times.forEach(function (t) { v += (t - mu) * (t - mu); });
      self.postMessage({ id: m.id, cmd: "point", tC: m.temps[k], mean: mu, se: Math.sqrt(v / Math.max(1, res.times.length - 1) / res.times.length), capped: res.capped, hops: res.hops });
      k++;
      setTimeout(next, 0);
    })();
  }
};
