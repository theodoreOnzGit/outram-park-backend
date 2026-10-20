// Fission products, chapter 2 "A fragment's story" (lessons 4-9): the
// widgets. Each registers with Lesson.widget(id, make) and runs only on the
// page that has its element:
//   #flight   (fragment-flight.html)  two fragments' flight, a cartoon at the
//             recorded RustBCA ranges until the tracks land (gh:#979)
//   #cascade  (lattice-cascade.html)  a knock-on cascade and recombination,
//             a labelled cartoon (BCA cannot show recombination; MD, gh:#977)
//   #chain    (decay-again.html)      a stopped fragment walking down its
//             decay chain, boon-lay's sampled histories, with each decay's
//             particles
//   #race     (decay-or-move.html)    how far a fission product diffuses in
//             one half-life, from boon-lay's correlations
//   #hop      (atom-hop.html)         one atom rattling and hopping; a cloud
//             and an Arrhenius plot from the kinetic Monte Carlo (worker)
//   #swap     (atoms-swap.html)       the mechanisms: vacancy, interstitial,
//             push-out, exchange and ring; traps and irradiation (worker)
//   #real-d   (atoms-swap.html)       the cited correlations in boon-lay
//   #turnbull (atoms-swap.html)       Turnbull's three terms (SCIANTIX)
// Data: cascade-data.js (generated), cascade-recorded.js (cited, hand-kept),
// hop-physics.js. Heavy work (clouds, sweeps) runs in hop-worker.js; the UI
// thread only draws and steps one atom. Uses ../neutronics/lesson-kit.js.
(function () {
  "use strict";
  var L = window.Lesson, css = L.css, el = L.el;
  var D = function () { return window.CASCADE_DATA; };
  var R = function () { return window.CASCADE_RECORDED; };
  var HP = function () { return window.HopPhysics; };
  var COL = { light: "#2f7fd8", heavy: "#b85c38", beta: "#7a4cc2", gamma: "#2a9d55", recoil: "#c0392b", neutron: "#333", alpha: "#d4a017" };

  function fmtT(s) {
    if (s === null || !isFinite(s)) return "stable";
    var u = [[3.15576e7, "y"], [86400, "d"], [3600, "h"], [60, "min"], [1, "s"], [1e-3, "ms"], [1e-6, "µs"], [1e-9, "ns"], [1e-12, "ps"], [1e-15, "fs"]];
    for (var i = 0; i < u.length; i++) if (s >= u[i][0]) return (s / u[i][0]).toPrecision(3) + " " + u[i][1];
    return s.toExponential(2) + " s";
  }
  // Draws `text` wrapped to maxW from (x, y); returns the y below it.
  function wrap(ctx, text, x, y, maxW, lh) {
    var words = text.split(" "), line = "";
    words.forEach(function (wd) {
      var t = line ? line + " " + wd : wd;
      if (ctx.measureText(t).width > maxW && line) { ctx.fillText(line, x, y); y += lh; line = wd; } else line = t;
    });
    if (line) { ctx.fillText(line, x, y); y += lh; }
    return y;
  }

  // One shared worker for the hop pages, with request ids.
  var worker = null, wid = 0, handlers = {};
  function ask(msg, onMsg) {
    if (!worker) {
      worker = new Worker("hop-worker.js");
      worker.onmessage = function (e) { var h = handlers[e.data.id]; if (h) h(e.data); };
    }
    msg.id = ++wid;
    handlers[msg.id] = onMsg;
    worker.postMessage(msg);
    return msg.id;
  }

  // ─── A: the flight (cartoon at the recorded ranges) ───────────────────
  function flight(root) {
    var rec = R().rustbca, st = { t: 0, playing: true, show: true };
    var frame = new L.Frame(root, {
      reset: function () { st.t = 0; },
      tick: function (dt) { if (st.playing) st.t = Math.min(st.t + dt / 3, 1); return st.playing && st.t < 1; },
      onSeen: function () { if (st.t >= 1) st.t = 0; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b");
        var cx = w / 2, cy = 60 + (h - 60) / 2, span = Math.max(rec.light.rangeUm, rec.heavy.rangeUm);
        var s = zoom * Math.min((w - 40) / (2 * span * 1.15), (h - 120) / (span * 0.8));
        // The lattice, as dots, only where they can be told apart.
        ctx.fillStyle = mut;
        var stepUm = 0.5, px = stepUm * s;
        if (px > 6) for (var gx = cx % px; gx < w; gx += px) for (var gy = 60 + (cy - 60) % px; gy < h; gy += px) ctx.fillRect(gx - 0.5, gy - 0.5, 1, 1);
        // Each fragment: straight while fast (electronic), a zig-zag at the
        // end (nuclear). The split is a cartoon, not a measured length.
        [["light", -1], ["heavy", 1]].forEach(function (f) {
          var r = rec[f[0]], dir = f[1], len = r.rangeUm * s, col = COL[f[0]];
          var reach = Math.min(st.t * 1.25, 1) * len, kink = 0.82 * len;
          ctx.lineWidth = 3; ctx.strokeStyle = col; ctx.beginPath(); ctx.moveTo(cx, cy);
          ctx.lineTo(cx + dir * Math.min(reach, kink), cy); ctx.stroke();
          if (reach > kink) {
            ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(cx + dir * kink, cy);
            var n = 7;
            for (var k = 1; k <= n; k++) {
              var d = kink + (len - kink) * k / n;
              if (d > reach) break;
              ctx.lineTo(cx + dir * d, cy + (k % 2 ? -1 : 1) * Math.min(10, (len - kink) / 3));
            }
            ctx.stroke();
          }
          if (st.t >= 0.8) { ctx.fillStyle = col; ctx.beginPath(); ctx.arc(cx + dir * len, cy, 6, 0, 2 * Math.PI); ctx.fill(); }
          ctx.fillStyle = col; ctx.font = "11px system-ui, sans-serif";
          ctx.textAlign = dir < 0 ? "right" : "left";
          ctx.fillText("fast", cx + dir * 8, cy - 10);
          if (st.t > 0.85) ctx.fillText("slow", cx + dir * len, cy + 22);
          ctx.textAlign = "left";
        });
        ctx.fillStyle = "#000"; ctx.beginPath(); ctx.arc(cx, cy, 4, 0, 2 * Math.PI); ctx.fill();
        // Scale bar.
        var bar = 1;
        while (bar * s < 40) bar *= 2;
        while (bar * s > 160) bar /= 2;
        ctx.strokeStyle = fg; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(12, h - 16); ctx.lineTo(12 + bar * s, h - 16); ctx.stroke();
        ctx.fillStyle = fg; ctx.fillText(bar + " µm", 12, h - 22);
        // Legend: the rest points, and what "fast" and "slow" mean.
        ctx.font = "12px system-ui, sans-serif";
        var ly = cy + 50;
        [["light", rec.light], ["heavy", rec.heavy]].forEach(function (f) {
          ctx.fillStyle = COL[f[0]]; ctx.fillRect(12, ly - 9, 10, 10);
          ctx.fillStyle = fg; ctx.fillText(f[1].name + " at rest after " + f[1].rangeUm + " ± " + f[1].semUm + " µm", 28, ly); ly += 16;
        });
        ctx.fillStyle = mut;
        ly = wrap(ctx, "fast: mostly ionisation and heat (straight); slow: collisions with nuclei (zig-zag), then rest.", 12, ly + 4, w - 24, 15);
        if (st.show) {
          ctx.font = "12px system-ui, sans-serif";
          ctx.fillStyle = "rgba(184,92,56,0.12)"; ctx.fillRect(8, 62, w - 16, 40);
          ctx.fillStyle = fg;
          wrap(ctx, "CARTOON at the recorded mean ranges; RustBCA tracks pending (gh:#979). Illustrative: ranges not yet checked against measured data.", 14, 78, w - 28, 15);
        }
        return "Two fragments from one U-235 fission, back to back, in UO2. Ranges: boon-lay's RustBCA record of " + rec.date + " (" + rec.light.name + " " + rec.light.rangeUm + " µm, " + rec.heavy.name + " " + rec.heavy.rangeUm + " µm; illustrative). The straight-then-zig-zag shape is drawn by hand.";
      },
      panel: function (body, redraw, play) {
        var b = el("button", { type: "button", "class": "w-opt" }, "Fly again");
        b.onclick = function () { st.t = 0; st.playing = true; play(); };
        body.append(b);
        L.toggle(body, "Show the 'cartoon' notice", st.show, function (v) { st.show = v; redraw(); });
        body.append(el("p", { "class": "w-note" }, "The real tracks (every collision, the energy lost to electrons and to nuclei along the way) come from boon-lay's RustBCA once gh:#979 lets a generator run it; they will replace this drawing."));
      }
    });
    return frame;
  }

  // ─── B: the cascade (cartoon) ──────────────────────────────────────────
  function cascade(root) {
    var st = { stage: 0, seed: 3, tree: null };
    function build() {
      // A random branching tree: each moving atom knocks on 0-2 others,
      // shorter each generation. Drawn by hand, not a simulation.
      var r = HP().rng(st.seed), nodes = [{ x: 0, y: 0, a: -0.3, len: 120, g: 0 }], segs = [];
      for (var i = 0; i < nodes.length && nodes.length < 60; i++) {
        var n = nodes[i];
        if (n.len < 14) continue;
        var ex = n.x + n.len * Math.cos(n.a), ey = n.y + n.len * Math.sin(n.a);
        segs.push({ x0: n.x, y0: n.y, x1: ex, y1: ey, g: n.g });
        var kids = r() < 0.85 ? (r() < 0.6 ? 2 : 1) : 0;
        for (var k = 0; k < kids; k++) nodes.push({ x: n.x + (ex - n.x) * (0.3 + 0.6 * r()), y: n.y + (ey - n.y) * (0.3 + 0.6 * r()), a: n.a + (r() - 0.5) * 2.4, len: n.len * (0.45 + 0.25 * r()), g: n.g + 1 });
      }
      var pairs = segs.filter(function (s) { return s.g > 0; }).map(function (s) { return { vx: s.x0, vy: s.y0, ix: s.x1, iy: s.y1, keep: r() < 0.2 }; });
      st.tree = { segs: segs, pairs: pairs };
    }
    build();
    var frame = new L.Frame(root, {
      reset: function () { st.stage = 0; },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b");
        var s = zoom * Math.min(w, h - 60) / 330, ox = w * 0.3, oy = 60 + (h - 60) * 0.55;
        var X = function (x) { return ox + x * s; }, Y = function (y) { return oy + y * s; };
        var maxG = st.stage === 0 ? 0 : st.stage === 1 ? 99 : 99;
        st.tree.segs.forEach(function (sg) {
          if (sg.g > maxG) return;
          ctx.strokeStyle = sg.g === 0 ? COL.heavy : "rgba(47,127,216," + Math.max(0.35, 1 - sg.g * 0.15) + ")";
          ctx.lineWidth = sg.g === 0 ? 3 : 1.5;
          ctx.beginPath(); ctx.moveTo(X(sg.x0), Y(sg.y0)); ctx.lineTo(X(sg.x1), Y(sg.y1)); ctx.stroke();
        });
        if (st.stage >= 1) {
          st.tree.pairs.forEach(function (p) {
            if (st.stage === 3 && !p.keep) return;
            ctx.strokeStyle = fg; ctx.lineWidth = 1.2; ctx.beginPath(); ctx.arc(X(p.vx), Y(p.vy), 4, 0, 2 * Math.PI); ctx.stroke();
            ctx.fillStyle = COL.recoil; ctx.beginPath(); ctx.arc(X(p.ix), Y(p.iy), 3.5, 0, 2 * Math.PI); ctx.fill();
          });
        }
        if (st.stage === 2) {
          var g = ctx.createRadialGradient(X(60), Y(10), 10, X(60), Y(10), 160 * s);
          g.addColorStop(0, "rgba(255,140,0,0.45)"); g.addColorStop(1, "rgba(255,140,0,0)");
          ctx.fillStyle = g; ctx.fillRect(0, 60, w, h - 60);
        }
        ctx.fillStyle = fg; ctx.font = "12px system-ui, sans-serif";
        var lines = [
          ["The primary knock-on (thick) starts moving.", ""],
          ["It knocks on others: a branching cascade. ○ vacancy, ● interstitial (a Frenkel pair each).", ""],
          ["Thermal spike: the region is briefly hot and the atoms jostle.", ""],
          ["Most pairs recombine; a few survive. (How many: source pending.)", ""]
        ][st.stage];
        wrap(ctx, lines[0], 10, 76, w - 20, 15);
        ctx.fillStyle = mut;
        wrap(ctx, "CARTOON drawn by hand, not a simulation. Recombination needs molecular dynamics (gh:#977, crate bugis).", 10, h - 40, w - 20, 15);
        return ["1 of 4: the primary knock-on", "2 of 4: the cascade", "3 of 4: the thermal spike", "4 of 4: after recombination"][st.stage] + ". The binary-collision code (RustBCA) can follow stage 2; it cannot show stages 3 and 4.";
      },
      panel: function (body, redraw) {
        var row = el("div", { "class": "w-row" });
        var back = el("button", { type: "button", "class": "w-opt" }, "← Back"), next = el("button", { type: "button", "class": "w-opt" }, "Next →");
        back.onclick = function () { st.stage = Math.max(0, st.stage - 1); redraw(); };
        next.onclick = function () { st.stage = Math.min(3, st.stage + 1); redraw(); };
        row.append(back, next); body.append(el("span", { "class": "w-label" }, "Step"), row);
        var nb = el("button", { type: "button", "class": "w-opt" }, "Another cascade");
        nb.onclick = function () { st.seed++; build(); redraw(); };
        body.append(nb);
        body.append(el("p", { "class": "w-note" }, "The shape is invented to show the idea. The real recoil tree (RustBCA, precomputed) waits for gh:#979."));
      }
    });
    return frame;
  }

  // ─── C: then it decays ────────────────────────────────────────────────
  function chain(root) {
    var Dd = D(), rec = R().rustbca;
    var st = { start: Dd.chains[0].start, hist: 0, cursor: -13, playing: true, bursts: [] };
    var T0 = -13, T1 = 10; // log10 seconds on the axis
    function C() { return Dd.chains.filter(function (c) { return c.start === st.start; })[0]; }
    function H() { return Dd.histories[st.start][st.hist]; }
    function stepOf(parent) { return C().steps.filter(function (s) { return s.parent === parent; })[0]; }
    function frag() { var a = C().a; return Dd.fragments.pair.filter(function (f) { return f.a === a; })[0]; }
    function tMin() { var r = frag().a === rec.light.a ? rec.light : rec.heavy; return r.rangeUm * 1e-6 / frag().v; }
    var frame = new L.Frame(root, {
      reset: function () { st.cursor = T0; st.bursts = []; st.playing = true; },
      onSeen: function () { if (st.cursor >= T1) { st.cursor = T0; st.bursts = []; } },
      tick: function (dt) {
        if (!st.playing) return false;
        var before = st.cursor;
        st.cursor = Math.min(st.cursor + dt * 2.2, T1);
        H().forEach(function (x, i) {
          if (x[1] === null) return;
          var lt = Math.log10(x[1]);
          if (lt > before && lt <= st.cursor) st.bursts.push({ i: i, parent: x[0], age: 0 });
        });
        st.bursts.forEach(function (b) { b.age += dt; });
        return st.cursor < T1 || st.bursts.some(function (b) { return b.age < 1.5; });
      },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b"), acc = css("--accent", "#1f6f78");
        ctx.font = "12px system-ui, sans-serif";
        // The timeline.
        var x0 = 16, x1 = w - 16, ty = h - 50;
        var tx = function (lt) { return x0 + (lt - T0) / (T1 - T0) * (x1 - x0); };
        ctx.strokeStyle = mut; ctx.beginPath(); ctx.moveTo(x0, ty); ctx.lineTo(x1, ty); ctx.stroke();
        ctx.fillStyle = mut;
        for (var e3 = -12; e3 <= 9; e3 += (w < 600 ? 6 : 3)) { ctx.fillRect(tx(e3), ty - 4, 1, 8); var lab = fmtT(Math.pow(10, e3)).match(/^([\d.]+) (.*)$/); ctx.fillText(Math.round(+lab[1]) + " " + lab[2], tx(e3) - 12, ty + 16); }
        ctx.fillText("time since fission (log)", x1 - 140, ty + 30);
        // The flight: a lower bound, range / birth speed.
        var tf = Math.log10(tMin());
        ctx.fillStyle = COL.light; ctx.fillRect(x0, ty - 10, tx(tf) - x0, 6);
        ctx.fillText("flight ≥ " + fmtT(tMin()), x0, ty - 14);
        // Half-lives on the main path.
        C().steps.forEach(function (s, i) {
          var x = tx(Math.log10(s.t12));
          ctx.fillStyle = acc; ctx.beginPath(); ctx.moveTo(x, ty - 2); ctx.lineTo(x - 4, ty - 10); ctx.lineTo(x + 4, ty - 10); ctx.fill();
          if (zoom > 1.3 || i % 2 === 0) ctx.fillText(s.parent + " t½", x - 20, ty - 22 - (i % 2) * 12);
        });
        // The cursor.
        ctx.strokeStyle = fg; ctx.beginPath(); ctx.moveTo(tx(st.cursor), ty - 40); ctx.lineTo(tx(st.cursor), ty + 6); ctx.stroke();
        // The atom, at rest, and what it is now.
        var cx = w / 2, cy = 60 + (ty - 100) / 2 + 10, sc = zoom;
        var now = H()[0][0];
        H().forEach(function (x, i) { if (i && H()[i - 1][1] !== null && Math.log10(H()[i - 1][1]) <= st.cursor) now = x[0]; });
        ctx.fillStyle = COL.heavy; ctx.beginPath(); ctx.arc(cx, cy, 12 * sc, 0, 2 * Math.PI); ctx.fill();
        ctx.fillStyle = fg; ctx.font = "14px system-ui, sans-serif"; ctx.fillText(now, cx - 22, cy - 18 * sc);
        ctx.font = "12px system-ui, sans-serif";
        // Each decay's burst: beta (wiggle), gamma (dashed), recoil (arrow).
        st.bursts.forEach(function (b, k) {
          if (b.age > 1.5) return;
          var s = stepOf(b.parent); if (!s) return;
          var f = Math.min(b.age / 0.8, 1), a = k * 2.1 + 0.6, Lb = 140 * sc * f, Lr = 6 * sc * Math.log10(1 + s.betaRecoilMax[0]) * 3 * f;
          ctx.strokeStyle = COL.beta; ctx.lineWidth = 1.5; ctx.beginPath(); ctx.moveTo(cx, cy);
          for (var j = 1; j <= 20; j++) { var d = Lb * j / 20; ctx.lineTo(cx + d * Math.cos(a) + 6 * Math.sin(j * 1.7) * Math.cos(a + 1.57), cy + d * Math.sin(a) + 6 * Math.sin(j * 1.7) * Math.sin(a + 1.57)); }
          ctx.stroke();
          ctx.setLineDash([6, 5]); ctx.strokeStyle = COL.gamma; ctx.beginPath(); ctx.moveTo(cx, cy); ctx.lineTo(cx + 1.6 * Lb * Math.cos(a + 2.3), cy + 1.6 * Lb * Math.sin(a + 2.3)); ctx.stroke(); ctx.setLineDash([]);
          L.arrow(ctx, cx, cy, cx + Lr * Math.cos(a + Math.PI), cy + Lr * Math.sin(a + Math.PI), COL.recoil);
          ctx.fillStyle = fg; ctx.fillText(s.parent + " → " + s.daughter + ": recoil ≤ " + s.betaRecoilMax[0].toFixed(0) + " eV", cx + 20, cy + 40 + 14 * (k % 3));
        });
        ctx.fillStyle = mut;
        wrap(ctx, "β (wiggly) and γ (dashed) tracks are ILLUSTRATIVE: their transport is PUNGGOL's (gh:#975, #976). Red arrow: the daughter's recoil (longer for a bigger kick, log scale).", 10, 76, w - 20, 15);
        var last = H()[H().length - 1][0];
        return st.start + ", sampled history " + (st.hist + 1) + " of " + Dd.histories[st.start].length + " (boon-lay's decay Monte Carlo): " + H().map(function (x) { return x[0] + (x[1] === null ? "" : " (decays at " + fmtT(x[1]) + ")"); }).join(" → ") + ". Ends as stable " + last + ".";
      },
      panel: function (body, redraw, play) {
        L.buttons(body, "Fragment", Dd.chains.map(function (c) { return [c.start, c.start + (c.a < 120 ? " (light)" : " (heavy)")]; }), st.start, function (v) { st.start = v; st.hist = 0; st.cursor = T0; st.bursts = []; st.playing = true; play(); });
        L.slider(body, "Sampled history", 0, Dd.histories[st.start].length - 1, 1, 0, function (v) { st.hist = v; st.cursor = T0; st.bursts = []; st.playing = true; play(); return (v + 1) + ""; });
        var b = el("button", { type: "button", "class": "w-opt" }, "Replay");
        b.onclick = function () { st.cursor = T0; st.bursts = []; st.playing = true; play(); };
        body.append(b);
        body.append(el("p", { "class": "w-note" }, "The cursor runs over 23 decades of time. The flight bar is a lower bound: the recorded range divided by the birth speed (the fragment slows, so the real flight is longer; the RustBCA flight time waits for gh:#979)."));
      }
    });
    return frame;
  }

  // The tables under the chain demo.
  function chainTables(node) {
    var Dd = D(), body = node.querySelector("tbody");
    Dd.chains.forEach(function (c) {
      c.steps.forEach(function (s) {
        var tr = el("tr");
        tr.append(el("td", { "data-label": "Decay" }, s.parent + " → " + s.daughter + " (" + s.mode + ")"),
          el("td", { "data-label": "Half-life" }, fmtT(s.t12)),
          el("td", { "data-label": "Q (from masses)" }, (s.qKeV[0] / 1e3).toFixed(1) + " MeV (" + (s.qKeV[1] / 1e3).toFixed(1) + "–" + (s.qKeV[2] / 1e3).toFixed(1) + ")"),
          el("td", { "data-label": "Largest β recoil" }, s.betaRecoilMax[0].toFixed(0) + " eV (" + s.betaRecoilMax[1].toFixed(0) + "–" + s.betaRecoilMax[2].toFixed(0) + ")"),
          el("td", { "data-label": "Largest γ recoil" }, s.gammaRecoilMax[0].toFixed(0) + " eV"));
        body.append(tr);
      });
    });
  }
  function kickTable(node) {
    var Dd = D(), body = node.querySelector("tbody");
    Dd.kicks.forEach(function (k) {
      var tr = el("tr");
      tr.append(el("td", { "data-label": "Decay" }, k.parent + " → " + k.daughter + " (" + (k.kind === "alpha" ? "α" : "β⁻ then n") + ", branch " + (100 * k.br).toPrecision(3) + " %)"),
        el("td", { "data-label": "Q" }, (k.qKeV[0] / 1e3).toFixed(2) + " MeV"),
        el("td", { "data-label": "Recoil" }, k.kind === "alpha" ? (k.recoil[0] / 1e3).toFixed(1) + " keV (" + (k.recoil[1] / 1e3).toFixed(1) + "–" + (k.recoil[2] / 1e3).toFixed(1) + ")" : "≤ " + (k.recoil[0] / 1e3).toFixed(1) + " keV"));
      body.append(tr);
    });
  }
  // Numbers in the prose, bound by data-cd="...".
  function fillNumbers() {
    var Dd = D(), rec = R(), ed = {};
    Dd.ed.forEach(function (e) { ed[e[0]] = e[2]; });
    var maxBeta = 0, minBeta = Infinity;
    Dd.chains.forEach(function (c) { c.steps.forEach(function (s) { maxBeta = Math.max(maxBeta, s.betaRecoilMax[0]); minBeta = Math.min(minBeta, s.betaRecoilMax[0]); }); });
    var alpha = Dd.kicks.filter(function (k) { return k.kind === "alpha"; }), neut = Dd.kicks.filter(function (k) { return k.kind === "neutron"; });
    var lf = Dd.fragments.pair[0], hf = Dd.fragments.pair[1], inf = Dd.inflight;
    var tmin = rec.rustbca.light.rangeUm * 1e-6 / lf.v;
    var map = {
      efr: (Dd.fragments.efr / 1e6).toFixed(1) + " MeV",
      eLight: (lf.e / 1e6).toFixed(1) + " MeV", eHeavy: (hf.e / 1e6).toFixed(1) + " MeV",
      vLight: (lf.v / 1e3).toPrecision(3) + " km/s", vLightC: (lf.v / 299792458 * 100).toFixed(1) + " %",
      rangeLight: rec.rustbca.light.rangeUm + " ± " + rec.rustbca.light.semUm + " µm", rangeHeavy: rec.rustbca.heavy.rangeUm + " ± " + rec.rustbca.heavy.semUm + " µm",
      recDate: rec.rustbca.date, recoilsLight: rec.rustbca.light.recoils.toLocaleString("en"), recoilsHeavy: rec.rustbca.heavy.recoils.toLocaleString("en"),
      tFlightMin: fmtT(tmin),
      akev: lf.a + " keV", akevFrac: (lf.a * 1e3 / lf.e).toExponential(1),
      cost: rec.rustbca.light.secondsPerCascade.toFixed(2) + " s (light) to " + rec.rustbca.heavy.secondsPerCascade.toFixed(2) + " s (heavy)",
      t12First: fmtT(Math.min(Dd.chains[0].steps[0].t12, Dd.chains[1].steps[0].t12)),
      pPerPs: inf.map(function (f) { return f.name + " " + f.pPerPs.toExponential(1); }).join(", "),
      inflightAngle: inf.map(function (f) { return f.name + " " + (f.angleRad * 1e3).toFixed(1) + " mrad"; }).join(", "),
      inflightDE: inf.map(function (f) { return f.name + " " + f.dEKeV.toFixed(0) + " keV"; }).join(", "),
      betaMax: maxBeta.toFixed(0) + " eV", betaMin: minBeta.toFixed(0) + " eV",
      edU: ed.U + " eV", edO: ed.O + " eV", edC: ed.C + " eV", edSi: ed.Si + " eV",
      doeEd: rec.doe.displacementEv[0] + " eV (" + rec.doe.displacementEv[0] + " to " + rec.doe.displacementEv[1] + " eV for most metals)",
      alphaRecoil: alpha.map(function (k) { return k.parent + " " + (k.recoil[0] / 1e3).toFixed(0) + " keV"; }).join(", "),
      alphaE: alpha.map(function (k) { return k.parent + " " + (k.eAlpha[0] / 1e6).toFixed(2) + " MeV"; }).join(", "),
      neutronRecoil: neut.map(function (k) { return k.parent + " ≤ " + (k.recoil[0] / 1e3).toFixed(0) + " keV"; }).join(", "),
      kernelUm: (Dd.kernelRadiusM * 1e6).toFixed(0) + " µm",
      hopRatio: HP() ? (HP().boltzmann(1, 1500) / HP().boltzmann(1, 1000)).toFixed(0) + " times" : "", kT1000: (HP() ? HP().kT(1000) : 0).toFixed(4) + " eV", kT300: (HP() ? HP().kT(300) : 0).toFixed(4) + " eV", kT1500: (HP() ? HP().kT(1500) : 0).toFixed(4) + " eV"
    };
    document.querySelectorAll("[data-cd]").forEach(function (n) {
      var k = n.getAttribute("data-cd");
      if (k in map) n.textContent = map[k];
    });
  }

  // ─── D: decay or move? ────────────────────────────────────────────────
  function race(root) {
    var Dd = D(), st = { i: 40 };
    function seriesFor(z) { return Dd.diffusion.series.filter(function (s) { return s.z === z && s.material === "kernel"; })[0]; }
    var frame = new L.Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b"), acc = css("--accent", "#1f6f78");
        ctx.font = "12px system-ui, sans-serif";
        var T = Dd.diffusion.tC[st.i], rows = Dd.race, x0 = 90, x1 = w - 20, y0 = 70;
        // Log axis of length, 1 nm to 1 m, zoomable about the kernel radius.
        var lo = -9, hi = 0, mid = Math.log10(Dd.kernelRadiusM), half = (hi - lo) / 2 / zoom;
        var a = mid - half, b = mid + half;
        var X = function (m) { return x0 + (Math.log10(m) - a) / (b - a) * (x1 - x0); };
        var bh = Math.min(34, (h - y0 - 60) / rows.length - 6);
        rows.forEach(function (r, k) {
          var s = seriesFor(r.z), Dv = s.d[st.i], len = Math.sqrt(6 * Dv * r.t12), y = y0 + k * (bh + 6);
          ctx.fillStyle = len > Dd.kernelRadiusM ? "#b85c38" : "#2f7fd8";
          ctx.fillRect(x0, y, Math.max(0, Math.min(X(len), x1) - x0), bh);
          ctx.fillStyle = fg; ctx.fillText(r.name, 8, y + bh / 2 + 4);
          ctx.fillText(len.toExponential(1) + " m (t½ " + fmtT(r.t12) + ")" + (s.flatUpToC !== null && T <= s.flatUpToC ? " clamped" : ""), Math.min(X(len), x1 - 170) + 4, y + bh / 2 + 4);
        });
        var kx = X(Dd.kernelRadiusM);
        ctx.strokeStyle = acc; ctx.setLineDash([5, 4]); ctx.beginPath(); ctx.moveTo(kx, y0 - 8); ctx.lineTo(kx, h - 40); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = acc; ctx.fillText("kernel radius", kx + 4, h - 44);
        ctx.fillStyle = mut;
        for (var e = Math.ceil(a); e <= Math.floor(b); e++) { ctx.fillRect(X(Math.pow(10, e)), h - 34, 1, 6); ctx.fillText("1e" + e, X(Math.pow(10, e)) - 10, h - 18); }
        ctx.fillText("distance diffused in one half-life, √(6 D t½), m (log)", x0, h - 4);
        return "At " + T + " °C in the UO2 kernel (TRISO-ATOPS's NP-MHTGR correlations in boon-lay; Kr and Xe share one). Blue: decays long before it gets out of a kernel; brown: moves farther than a kernel radius in one half-life.";
      },
      panel: function (body, redraw) {
        L.slider(body, "Temperature (°C)", 0, Dd.diffusion.tC.length - 1, 1, st.i, function (v) { st.i = v; redraw(); return Dd.diffusion.tC[v] + " °C"; });
        body.append(el("p", { "class": "w-note" }, "'clamped': below a species' lower limit the correlation holds D at its value there (Cs and Sr in the kernel below 700 °C); Kr and Xe are extrapolated instead. Kr's switch temperature is disputed (gh:#974)."));
      }
    });
    return frame;
  }

  // ─── E: one atom hops ─────────────────────────────────────────────────
  function hop(root) {
    var H = HP(), st = { T: 1500, Em: 1.0, view: "atom", x: 0, y: 0, px: 0, py: 0, hopT: 1, attempts: 0, hops: 0, ff: false, cloud: null, cloudT: 0, sweep: [], fit: null, busy: false };
    var R0 = H.rng(42);
    // Attempts shown per second of screen time: fixed, so the temperature
    // slider changes how often the atom hops on screen. Chosen so that
    // E_m = 1 eV at 2000 K hops a few times a second.
    var ATTEMPTS_PER_S = 1.5 / (4 * H.boltzmann(1.0, 2000));
    function rate() { return 4 * H.boltzmann(st.Em, st.T); }
    function runCloud() {
      st.busy = true;
      var t = 400 / rate();
      ask({ cmd: "cloud", p: { Em: st.Em, T: st.T, trapFrac: 0, Eb: 0, seed: 1 }, n: 600, t: t, seed: 7 }, function (m) { st.cloud = m; st.cloudT = t; st.busy = false; frame.redraw(); });
    }
    function runSweep() {
      st.busy = true; st.sweep = []; st.fit = null;
      var Ts = []; for (var i = 0; i < 8; i++) Ts.push(900 + 150 * i);
      ask({ cmd: "sweep", kind: "walk", p: { Em: st.Em }, Ts: Ts, size: 800, hops: 400, seed: 21 }, function (m) {
        if (m.cmd === "point") st.sweep.push(m.pt);
        else { st.fit = m.fit; st.busy = false; }
        frame.redraw();
      });
    }
    var frame = new L.Frame(root, {
      reset: function () { st.x = st.y = st.px = st.py = 0; st.hops = 0; st.attempts = 0; },
      tick: function (dt) {
        if (st.view !== "atom") return false;
        var n = ATTEMPTS_PER_S * dt * (st.ff ? 1000 : 1), p = H.boltzmann(st.Em, st.T) * 4;
        st.attempts += n;
        // Hops in this frame: Poisson with mean n p; at most one is drawn.
        if (R0() < 1 - Math.exp(-n * p)) {
          var k = (R0() * 4) | 0;
          st.px = st.x; st.py = st.y; st.x += [1, -1, 0, 0][k]; st.y += [0, 0, 1, -1][k]; st.hopT = 0; st.hops++;
        }
        st.hopT = Math.min(st.hopT + dt * 4, 1);
        return true;
      },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b"), acc = css("--accent", "#1f6f78");
        ctx.font = "12px system-ui, sans-serif";
        var kt = H.kT(st.T);
        if (st.view === "atom") {
          // The lattice of wells, centred on the atom.
          var top = 64, landH = 110, gh = h - top - landH - 10, sp = zoom * Math.min(w, gh) / 7;
          var cx = w / 2, cy = top + gh / 2;
          var ax = st.px + (st.x - st.px) * st.hopT, ay = st.py + (st.y - st.py) * st.hopT;
          ctx.fillStyle = mut;
          for (var i = -6; i <= 6; i++) for (var j = -6; j <= 6; j++) {
            var gx = cx + (i - ax) * sp, gy = cy + (j - ay) * sp;
            if (gx < -sp || gx > w + sp || gy < top - sp || gy > top + gh) continue;
            ctx.beginPath(); ctx.arc(gx, gy, Math.max(2, sp * 0.08), 0, 2 * Math.PI); ctx.fill();
          }
          // The atom rattles: amplitude grows with sqrt(T) (cartoon scale).
          var amp = 0.06 * sp * Math.sqrt(st.T / 1500);
          var jx = (R0() - 0.5) * 2 * amp, jy = (R0() - 0.5) * 2 * amp;
          ctx.fillStyle = COL.heavy; ctx.beginPath(); ctx.arc(cx + jx, cy + jy, Math.max(6, sp * 0.22), 0, 2 * Math.PI); ctx.fill();
          // The landscape: E(x) between two sites, with E_m and kT.
          var ly = h - 20, lh = landH - 30, lx0 = 20, lx1 = w - 20, emax = Math.max(st.Em * 1.25, 0.3);
          var Y = function (e) { return ly - e / emax * lh; };
          ctx.strokeStyle = fg; ctx.lineWidth = 2; ctx.beginPath();
          for (var q = 0; q <= 100; q++) { var u = q / 100; var e = st.Em * 0.5 * (1 - Math.cos(2 * Math.PI * 2 * u)); var xx = lx0 + u * (lx1 - lx0); if (q) ctx.lineTo(xx, Y(e)); else ctx.moveTo(xx, Y(e)); }
          ctx.stroke();
          ctx.strokeStyle = acc; ctx.setLineDash([4, 4]); ctx.beginPath(); ctx.moveTo(lx0, Y(st.Em)); ctx.lineTo(lx1, Y(st.Em)); ctx.stroke();
          ctx.strokeStyle = COL.heavy; ctx.beginPath(); ctx.moveTo(lx0, Y(kt)); ctx.lineTo(lx1, Y(kt)); ctx.stroke(); ctx.setLineDash([]);
          ctx.fillStyle = acc; ctx.fillText("barrier E_m = " + st.Em.toFixed(2) + " eV (illustrative)", lx0, Y(st.Em) - 4);
          ctx.fillStyle = COL.heavy; ctx.fillText("kT = " + kt.toFixed(4) + " eV", lx1 - 110, Y(kt) - 4);
          var p1 = H.boltzmann(st.Em, st.T);
          return "T = " + st.T + " K: a rattle clears the barrier with chance exp(−E_m/kT) = 1 in " + (1 / p1).toPrecision(3) + ". Hops so far: " + st.hops + " in " + st.attempts.toExponential(2) + " rattles" + (st.ff ? " (fast-forward ×1000)" : "") + ".";
        }
        if (st.view === "cloud") {
          var c = st.cloud, ox = w / 2, oy = 60 + (h - 60) / 2;
          if (!c) { ctx.fillStyle = mut; ctx.fillText(st.busy ? "computing in the worker…" : "Press 'Release a cloud'.", 20, 90); return "A cloud of atoms released at one site."; }
          // Fit 3 sigma of the cloud into the view at zoom 1.
          var sc = zoom * Math.min(w, h - 70) / 2 / (3 * Math.sqrt(c.msd / 2) + 1);
          ctx.fillStyle = "rgba(184,92,56,0.45)";
          for (var k2 = 0; k2 < c.xs.length; k2++) ctx.fillRect(ox + c.xs[k2] * sc - 1.5, oy + c.ys[k2] * sc - 1.5, 3, 3);
          // The Gaussian's 1-sigma circle: sigma^2 = 2 D t per axis.
          var sig = Math.sqrt(c.msd / 2);
          ctx.strokeStyle = fg; ctx.beginPath(); ctx.arc(ox, oy, sig * sc, 0, 2 * Math.PI); ctx.stroke();
          ctx.beginPath(); ctx.arc(ox, oy, 2 * sig * sc, 0, 2 * Math.PI); ctx.stroke();
          ctx.fillStyle = fg; wrap(ctx, "circles: 1σ and 2σ of a Gaussian with the cloud's own spread", 10, 76, w - 20, 15);
          return c.xs.length + " atoms after " + st.cloudT.toExponential(2) + " rattles (~" + c.hops.toFixed(0) + " hops each) at " + st.T + " K: mean squared distance " + c.msd.toFixed(0) + " spacings², so the cloud's width grows as √(time).";
        }
        // The Arrhenius plot built from simulated hops.
        var x0 = 60, x1 = w - 20, y0 = 70, y1 = h - 40, pts = st.sweep;
        if (!pts.length) { ctx.fillStyle = mut; ctx.fillText(st.busy ? "computing in the worker…" : "Press 'Build the Arrhenius plot'.", 20, 90); return "ln D against 1/T, from the simulated hops."; }
        var ix = pts.map(function (p) { return p.invT; }), ly2 = pts.map(function (p) { return p.lnD; });
        var xa = Math.min.apply(null, ix), xb = Math.max.apply(null, ix), ya = Math.min.apply(null, ly2), yb = Math.max.apply(null, ly2);
        var PX = function (v) { return x0 + (v - xa) / (xb - xa || 1) * (x1 - x0); }, PY = function (v) { return y1 - (v - ya) / (yb - ya || 1) * (y1 - y0); };
        ctx.strokeStyle = mut; ctx.strokeRect(x0, y0, x1 - x0, y1 - y0);
        ctx.fillStyle = COL.light;
        pts.forEach(function (p) { ctx.beginPath(); ctx.arc(PX(p.invT), PY(p.lnD), 5, 0, 2 * Math.PI); ctx.fill(); });
        if (st.fit) {
          ctx.strokeStyle = COL.heavy; ctx.beginPath(); ctx.moveTo(PX(xa), PY(st.fit.intercept + st.fit.slope * xa)); ctx.lineTo(PX(xb), PY(st.fit.intercept + st.fit.slope * xb)); ctx.stroke();
        }
        ctx.fillStyle = mut; ctx.fillText("1/T (1/K) →", x1 - 80, y1 + 16); ctx.fillText("ln D", 8, y0 + 10);
        return st.fit ? "Straight line through the simulated points: slope −E/k_B gives E = " + st.fit.E.toFixed(3) + " eV; the barrier put in was " + st.Em.toFixed(2) + " eV." : "Points arrive from the worker one temperature at a time…";
      },
      panel: function (body, redraw, play) {
        L.buttons(body, "View", [["atom", "One atom"], ["cloud", "A cloud"], ["arrhenius", "Arrhenius plot"]], st.view, function (v) { st.view = v; if (v === "atom") play(); redraw(); });
        L.slider(body, "Temperature (K)", 600, 2000, 50, st.T, function (v) { st.T = v; redraw(); return v + " K, kT = " + H.kT(v).toFixed(4) + " eV"; });
        L.slider(body, "Barrier E_m (illustrative)", 0.5, 1.5, 0.05, st.Em, function (v) { st.Em = v; redraw(); return v.toFixed(2) + " eV"; });
        L.toggle(body, "Fast-forward ×1000", st.ff, function (v) { st.ff = v; });
        var b1 = el("button", { type: "button", "class": "w-opt" }, "Release a cloud");
        b1.onclick = function () { st.view = "cloud"; runCloud(); redraw(); };
        var b2 = el("button", { type: "button", "class": "w-opt" }, "Build the Arrhenius plot");
        b2.onclick = function () { st.view = "arrhenius"; runSweep(); redraw(); };
        body.append(b1, b2, el("p", { "class": "w-note" }, "Time is counted in rattles (attempts), so no vibration frequency is assumed. The cloud and the plot run in a Web Worker."));
      }
    });
    return frame;
  }

  // ─── F: atoms swapping ────────────────────────────────────────────────
  function swap(root) {
    var H = HP(), N = 9;
    var st = { mech: "vacancy", T: 1500, traps: false, irr: false, grid: null, vac: 0, tracer: 0, tPos: 0, inter: { x: 4, y: 4, px: 4, py: 4 }, f: 1, R: H.rng(5), acc: 0, sweeps: {}, busy: false, showPlot: false };
    function reset() {
      st.grid = []; for (var i = 0; i < N * N; i++) st.grid.push(i);
      st.vac = 4 * N + 3; st.grid[st.vac] = -1; st.tracer = 4 * N + 4; st.tPos = st.tracer;
      st.inter = { x: 4.5, y: 4.5, px: 4.5, py: 4.5 }; st.f = 1; st.anim = null;
    }
    reset();
    function stepVacancy() {
      var v = st.vac, x = v % N, y = (v / N) | 0, k = (st.R() * 4) | 0;
      var nx = Math.min(N - 1, Math.max(0, x + [1, -1, 0, 0][k])), ny = Math.min(N - 1, Math.max(0, y + [0, 0, 1, -1][k])), ns = ny * N + nx;
      if (ns === v) return;
      var a = st.grid[ns];
      st.anim = { atom: a, from: ns, to: v, t: 0 };
      st.grid[v] = a; st.grid[ns] = -1; st.vac = ns;
      if (a === st.tracer) st.tPos = v;
    }
    function stepInter() {
      var I = st.inter, k = (st.R() * 4) | 0;
      I.px = I.x; I.py = I.y;
      I.x = Math.min(N - 1.5, Math.max(0.5, I.x + [1, -1, 0, 0][k])); I.y = Math.min(N - 1.5, Math.max(0.5, I.y + [0, 0, 1, -1][k]));
      st.f = 0;
    }
    function runSweeps() {
      st.busy = true; st.sweeps = {}; st.showPlot = true;
      var Ts = []; for (var i = 0; i < 6; i++) Ts.push(800 + 200 * i);
      var cases = [
        ["thermal vacancies", "vacancy", { Ef: 0.6, Em: 0.8, cIrr: 0 }, 100, 100000],
        ["+ irradiation", "vacancy", { Ef: 0.6, Em: 0.8, cIrr: 1e-2 }, 100, 100000],
        ["interstitial", "walk", { Em: 0.8 }, 600, 300],
        ["interstitial + traps", "walk", { Em: 0.8, Eb: 0.5, trapFrac: 0.05 }, 600, 300]
      ];
      cases.forEach(function (c) {
        st.sweeps[c[0]] = { pts: [], fit: null };
        ask({ cmd: "sweep", kind: c[1], p: c[2], Ts: Ts, size: c[3], hops: c[4], seed: 9 }, function (m) {
          if (m.cmd === "point") st.sweeps[c[0]].pts.push(m.pt); else st.sweeps[c[0]].fit = m.fit;
          st.busy = Object.keys(st.sweeps).some(function (k) { return !st.sweeps[k].fit; });
          frame.redraw();
        });
      });
    }
    var SW = { "thermal vacancies": "#2f7fd8", "+ irradiation": "#7a4cc2", "interstitial": "#2a9d55", "interstitial + traps": "#b85c38" };
    var frame = new L.Frame(root, {
      reset: reset,
      tick: function (dt) {
        if (st.showPlot) return false;
        st.acc += dt;
        if (st.anim) { st.anim.t += dt * 3; if (st.anim.t >= 1) st.anim = null; }
        st.f = Math.min(1, st.f + dt * 3);
        if (st.acc > 0.45) {
          st.acc = 0;
          if (st.mech === "vacancy") stepVacancy();
          else if (st.mech === "interstitial") stepInter();
          else st.cartoon = ((st.cartoon || 0) + 1) % 40;
        }
        return true;
      },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b");
        ctx.font = "12px system-ui, sans-serif";
        if (st.showPlot) {
          var x0 = 60, x1 = w - 20, y0 = 70, y1 = h - 40, all = [];
          Object.keys(st.sweeps).forEach(function (k) { all = all.concat(st.sweeps[k].pts); });
          if (!all.length) { ctx.fillStyle = mut; ctx.fillText("computing in the worker…", 20, 90); return "Arrhenius plots of four mechanisms."; }
          var xa = Math.min.apply(null, all.map(function (p) { return p.invT; })), xb = Math.max.apply(null, all.map(function (p) { return p.invT; }));
          var ya = Math.min.apply(null, all.map(function (p) { return p.lnD; })), yb = Math.max.apply(null, all.map(function (p) { return p.lnD; }));
          var PX = function (v) { return x0 + (v - xa) / (xb - xa || 1) * (x1 - x0); }, PY = function (v) { return y1 - (v - ya) / (yb - ya || 1) * (y1 - y0); };
          ctx.strokeStyle = mut; ctx.strokeRect(x0, y0, x1 - x0, y1 - y0);
          var txt = [];
          Object.keys(st.sweeps).forEach(function (k, i) {
            var s = st.sweeps[k];
            ctx.fillStyle = SW[k]; ctx.strokeStyle = SW[k];
            s.pts.forEach(function (p) { ctx.beginPath(); ctx.arc(PX(p.invT), PY(p.lnD), 4, 0, 2 * Math.PI); ctx.fill(); });
            ctx.beginPath(); s.pts.forEach(function (p, j) { if (j) ctx.lineTo(PX(p.invT), PY(p.lnD)); else ctx.moveTo(PX(p.invT), PY(p.lnD)); }); ctx.stroke();
            ctx.fillText(k + (s.fit ? ": slope gives " + s.fit.E.toFixed(2) + " eV" : ""), x0 + 8, y0 + 16 + 15 * i);
            if (s.fit) txt.push(k + " " + s.fit.E.toFixed(2) + " eV");
          });
          ctx.fillStyle = mut; ctx.fillText("1/T →  (800-1800 K)", x1 - 130, y1 + 16); ctx.fillText("ln D", 8, y0 + 10);
          return "Illustrative barriers: E_m 0.8 eV, vacancy formation E_f 0.6 eV, trap binding E_b 0.5 eV (5 % of sites), irradiation vacancy fraction 1e-2. Slopes over 800-1800 K: " + txt.join("; ") + ".";
        }
        var top = 64, sp = zoom * Math.min(w - 20, h - top - 20) / N, ox = (w - sp * (N - 1)) / 2, oy = top + 10 + sp / 2;
        var P = function (i) { return [ox + (i % N) * sp, oy + ((i / N) | 0) * sp]; };
        var rad = Math.max(5, sp * 0.3);
        if (st.mech === "vacancy") {
          for (var s = 0; s < N * N; s++) {
            var a = st.grid[s], p = P(s);
            if (a < 0) { ctx.strokeStyle = fg; ctx.setLineDash([3, 3]); ctx.beginPath(); ctx.arc(p[0], p[1], rad, 0, 2 * Math.PI); ctx.stroke(); ctx.setLineDash([]); continue; }
            if (st.anim && st.anim.atom === a) { var f = P(st.anim.from), t = P(st.anim.to), u = st.anim.t; p = [f[0] + (t[0] - f[0]) * u, f[1] + (t[1] - f[1]) * u]; }
            ctx.fillStyle = a === st.tracer ? COL.heavy : "#9aa7b0"; ctx.beginPath(); ctx.arc(p[0], p[1], rad, 0, 2 * Math.PI); ctx.fill();
          }
          return "Vacancy mechanism: the empty site (dashed) wanders by trading places with a neighbour; an atom moves only when the vacancy comes by. The brown atom is one we follow (the animation's pace is not to scale).";
        }
        for (var s2 = 0; s2 < N * N; s2++) { var q = P(s2); ctx.fillStyle = "#9aa7b0"; ctx.beginPath(); ctx.arc(q[0], q[1], rad, 0, 2 * Math.PI); ctx.fill(); }
        if (st.mech === "interstitial") {
          var I = st.inter, ix = I.px + (I.x - I.px) * st.f, iy = I.py + (I.y - I.py) * st.f;
          ctx.fillStyle = COL.recoil; ctx.beginPath(); ctx.arc(ox + ix * sp, oy + iy * sp, rad * 0.55, 0, 2 * Math.PI); ctx.fill();
          return "Interstitial mechanism: a small atom slips between the lattice sites from gap to gap; no vacancy is needed, so it usually moves much faster.";
        }
        // Cartoons: push-out and exchange/ring, on the middle of the lattice.
        var c = (st.cartoon || 0) / 40, mid = 4 * N + 4;
        if (st.mech === "pushout") {
          var pm = P(mid), u2 = Math.min(1, c * 2), u3 = Math.max(0, c * 2 - 1), g = sp / 2;
          // The interstitial (red) comes in from the gap up-left; the site's
          // atom (brown) is pushed into the gap down-right.
          ctx.strokeStyle = mut; ctx.setLineDash([3, 3]); ctx.beginPath(); ctx.arc(pm[0] - g, pm[1] - g, rad * 0.7, 0, 2 * Math.PI); ctx.arc(pm[0] + g, pm[1] + g, rad * 0.7, 0, 2 * Math.PI); ctx.stroke(); ctx.setLineDash([]);
          ctx.fillStyle = COL.heavy; ctx.beginPath(); ctx.arc(pm[0] + u3 * g, pm[1] + u3 * g, rad, 0, 2 * Math.PI); ctx.fill();
          ctx.fillStyle = COL.recoil; ctx.beginPath(); ctx.arc(pm[0] - g + u2 * g, pm[1] - g + u2 * g, rad * 0.8, 0, 2 * Math.PI); ctx.fill();
          ctx.fillStyle = fg; ctx.fillText("CARTOON", 10, h - 10);
          return "Push-out (interstitialcy): an interstitial (red) pushes a lattice atom (brown) off its site into the next gap and takes the site. A cartoon.";
        }
        var ring = [mid, mid + 1, mid + 1 + N, mid + N], ang = c * Math.PI / 2;
        var cc = [(P(mid)[0] + P(mid + 1)[0]) / 2, (P(mid)[1] + P(mid + N)[1]) / 2];
        ring.forEach(function (sidx, k) {
          var p0 = P(sidx), a0 = Math.atan2(p0[1] - cc[1], p0[0] - cc[0]), r0 = Math.hypot(p0[0] - cc[0], p0[1] - cc[1]);
          ctx.fillStyle = ["#2f7fd8", "#b85c38", "#7a4cc2", "#2a9d55"][k]; ctx.beginPath(); ctx.arc(cc[0] + r0 * Math.cos(a0 + ang), cc[1] + r0 * Math.sin(a0 + ang), rad, 0, 2 * Math.PI); ctx.fill();
        });
        ctx.fillStyle = fg; ctx.fillText("CARTOON", 10, h - 10);
        return "Ring rotation: four neighbours turn together, each taking the next one's site (two swapping is direct exchange). How rare these are in crystals: source pending.";
      },
      panel: function (body, redraw, play) {
        L.buttons(body, "Mechanism", [["vacancy", "Vacancy"], ["interstitial", "Interstitial"], ["pushout", "Push-out"], ["ring", "Exchange / ring"]], st.mech, function (v) { st.mech = v; st.showPlot = false; reset(); play(); redraw(); });
        var b = el("button", { type: "button", "class": "w-opt" }, "Arrhenius plots: vacancies, irradiation, traps");
        b.onclick = function () { runSweeps(); redraw(); };
        var back = el("button", { type: "button", "class": "w-opt" }, "Back to the lattice");
        back.onclick = function () { st.showPlot = false; play(); redraw(); };
        body.append(b, back, el("p", { "class": "w-note" }, "The plots run the kinetic Monte Carlo of the previous lesson in a Web Worker, with illustrative barriers, and fit the slope of each."));
      }
    });
    return frame;
  }

  // ─── F: the cited correlations (boon-lay) ─────────────────────────────
  function realD(root) {
    var Dd = D(), st = { on: { "Xe in kernel": true, "Cs in kernel": true, "Sr in kernel": true, "Ag in SiC": true } };
    var PAL = ["#2f7fd8", "#b85c38", "#7a4cc2", "#2a9d55", "#d4a017", "#c0392b", "#555", "#1f6f78", "#8e44ad", "#16a085", "#7f8c8d"];
    var frame = new L.Frame(root, {
      reset: function () {}, tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b");
        ctx.font = "12px system-ui, sans-serif";
        var x0 = 64, x1 = w - 16, y0 = 66, y1 = h - 44, tC = Dd.diffusion.tC;
        var inv = tC.map(function (c) { return 1e4 / (c + 273.15); });
        var shown = Dd.diffusion.series.filter(function (s) { return st.on[s.label]; });
        var lo = Infinity, hi = -Infinity;
        shown.forEach(function (s) { s.d.forEach(function (v) { lo = Math.min(lo, Math.log10(v)); hi = Math.max(hi, Math.log10(v)); }); });
        if (!shown.length) return "Pick a correlation in the panel.";
        var mid = (lo + hi) / 2, half = (hi - lo) / 2 / zoom + 0.5;
        var PX = function (v) { return x0 + (v - inv[inv.length - 1]) / (inv[0] - inv[inv.length - 1]) * (x1 - x0); };
        var PY = function (v) { return y1 - (Math.log10(v) - (mid - half)) / (2 * half) * (y1 - y0); };
        ctx.strokeStyle = mut; ctx.strokeRect(x0, y0, x1 - x0, y1 - y0);
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, x1 - x0, y1 - y0); ctx.clip();
        Dd.diffusion.series.forEach(function (s, k) {
          if (!st.on[s.label]) return;
          ctx.strokeStyle = PAL[k % PAL.length]; ctx.lineWidth = 2;
          for (var i = 1; i < tC.length; i++) {
            ctx.setLineDash(s.flatUpToC !== null && tC[i] <= s.flatUpToC ? [4, 4] : []);
            ctx.beginPath(); ctx.moveTo(PX(inv[i - 1]), PY(s.d[i - 1])); ctx.lineTo(PX(inv[i]), PY(s.d[i])); ctx.stroke();
          }
          ctx.setLineDash([]);
        });
        ctx.restore();
        var li = 0;
        Dd.diffusion.series.forEach(function (s, k) { if (st.on[s.label]) { ctx.fillStyle = PAL[k % PAL.length]; ctx.fillText(s.label, x0 + 6, y0 + 14 + 14 * li++); } });
        ctx.fillStyle = mut;
        for (var e = Math.ceil(mid - half); e <= Math.floor(mid + half); e++) ctx.fillText("1e" + e, 6, PY(Math.pow(10, e)) + 4);
        [400, 800, 1200, 1600, 2000].forEach(function (c) { var x = PX(1e4 / (c + 273.15)); ctx.fillRect(x, y1, 1, 5); ctx.fillText(c + " °C", x - 14, y1 + 18); });
        ctx.fillText("D (m²/s), log, against 1/T: hotter to the left. Dashed: clamped (held constant below the correlation's limit).", x0, h - 6);
        return "TRISO-ATOPS's NP-MHTGR correlations as boon-lay ports them (diffusion_coefficient, diffusion_coefficient_sic_ag), 400-2000 °C. A straight line on this plot is one Arrhenius term; a bend is a second term or a switch.";
      },
      panel: function (body, redraw) {
        D().diffusion.series.forEach(function (s) { L.toggle(body, s.label, !!st.on[s.label], function (v) { st.on[s.label] = v; redraw(); }); });
        body.append(el("p", { "class": "w-note" }, "Kr, Xe, I, Te and Se share one correlation, with a switch at 1500 °C (whether Kr's switch belongs there is disputed, gh:#974). Kr and Xe are extrapolated, not clamped, outside the range."));
      }
    });
    return frame;
  }

  // ─── F advanced: Turnbull's three terms (SCIANTIX) ────────────────────
  function turnbullW(root) {
    var H = HP(), st = { logF: 19 };
    var frame = new L.Frame(root, {
      reset: function () {}, tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b");
        ctx.font = "12px system-ui, sans-serif";
        var F = Math.pow(10, st.logF), x0 = 64, x1 = w - 16, y0 = 66, y1 = h - 40, Ts = [];
        for (var T = 800; T <= 2400; T += 20) Ts.push(T);
        var rows = Ts.map(function (T) { return H.turnbull(T, F); });
        var hi = -12, lo = hi - 12 / zoom;
        var PX = function (T) { return x0 + (1 / T - 1 / 2400) / (1 / 800 - 1 / 2400) * (x1 - x0); };
        var PY = function (v) { return y1 - (Math.log10(Math.max(v, 1e-40)) - lo) / (hi - lo) * (y1 - y0); };
        ctx.strokeStyle = mut; ctx.strokeRect(x0, y0, x1 - x0, y1 - y0);
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, x1 - x0, y1 - y0); ctx.clip();
        [["d1", "#2f7fd8"], ["d2", "#7a4cc2"], ["d3", "#2a9d55"], ["total", fg]].forEach(function (k) {
          ctx.strokeStyle = k[1]; ctx.lineWidth = k[0] === "total" ? 2.5 : 1.5; ctx.beginPath();
          rows.forEach(function (r, i) { if (i) ctx.lineTo(PX(Ts[i]), PY(r[k[0]])); else ctx.moveTo(PX(Ts[i]), PY(r[k[0]])); });
          ctx.stroke();
        });
        ctx.restore();
        ctx.fillStyle = "#2f7fd8"; ctx.fillText("d1: thermal, needs its own vacancies", x0 + 6, y0 + 14);
        ctx.fillStyle = "#7a4cc2"; ctx.fillText("d2: vacancies from the fissions, ∝ √F", x0 + 6, y0 + 28);
        ctx.fillStyle = "#2a9d55"; ctx.fillText("d3: no temperature at all, ∝ F", x0 + 6, y0 + 42);
        ctx.fillStyle = mut;
        for (var e = Math.ceil(lo); e <= hi; e += 2) ctx.fillText("1e" + e, 6, PY(Math.pow(10, e)) + 4);
        [800, 1200, 1600, 2000, 2400].forEach(function (T) { ctx.fillRect(PX(T), y1, 1, 5); ctx.fillText(T + " K", PX(T) - 14, y1 + 18); });
        return "Turnbull et al. (1988) as SCIANTIX codes it, Xe in UO2, fission rate F = 1e" + st.logF.toFixed(1) + " fissions/m³/s (choose it in the panel; no value here is a typical one). Hotter to the left.";
      },
      panel: function (body, redraw) {
        L.slider(body, "log10 fission rate F (per m³ per s)", 16, 20, 0.5, st.logF, function (v) { st.logF = v; redraw(); return "1e" + v.toFixed(1); });
      }
    });
    return frame;
  }

  L.widget("flight", flight);
  L.widget("cascade", cascade);
  L.widget("chain", chain);
  L.widget("chain-table", chainTables);
  L.widget("kick-table", kickTable);
  L.widget("race", race);
  L.widget("hop", hop);
  L.widget("swap", swap);
  L.widget("real-d", realD);
  L.widget("turnbull", turnbullW);
  L.widget("cd-numbers", fillNumbers);
})();
