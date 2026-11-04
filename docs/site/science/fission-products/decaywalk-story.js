// Fission products, chapter 2: the widgets of "An atom's clock: sampling a
// decay" (decay-clock.html) and "Walking on spheres"
// (walk-on-spheres.html). Each registers with Lesson.widget(id, make).
// Data: decaywalk-data.js, RECORDED by crates/dhoby-ghaut/examples/
// lesson_decay_walk from boon-lay (decay Monte Carlo, CRAM48, Gaussian
// step, Walk-on-Spheres, CRP-6 Case 1). No physics here: the widgets look
// up which species an atom is at a time from its recorded clock, play
// recorded hops, and draw recorded curves.
(function () {
  "use strict";
  var L = window.Lesson, css = L.css, el = L.el;
  var LP = window.LessonPlot, sci = LP.sci, Box = LP.Box, linTicks = LP.linTicks, legend = LP.legend, fmtT = LP.fmtT;
  var DW = function () { return window.DECAYWALK_DATA; };
  var SCOL = ["#b85c38", "#2f7fd8", "#2a9d55", "#7a4cc2", "#d4a017", "#7f8c8d", "#c0392b"];
  var RCOL = ["#d4a017", "#9fb7d9", "#5b6b7a", "#2a9d55", "#3d4a57", "#c0392b"];
  var RNAME = ["kernel", "buffer", "IPyC", "SiC", "OPyC", "outside"];
  function fg() { return css("--fg", "#1d2125"); }
  function mut() { return css("--muted", "#5b636b"); }
  function lerp(x, xs, ys) { if (x <= xs[0]) return ys[0]; for (var i = 1; i < xs.length; i++) if (x <= xs[i]) { var u = (x - xs[i - 1]) / (xs[i] - xs[i - 1]); return ys[i - 1] + u * (ys[i] - ys[i - 1]); } return ys[ys.length - 1]; }
  // An atom's species at t (in parent half-lives): its recorded stages and
  // the times they end, [k0, end0, k1, end1, ..., kLast].
  function stageAt(a, t) { for (var i = 0; i + 1 < a.length; i += 2) if (t < a[i + 1]) return a[i]; return a[a.length - 1]; }

  // ─── An atom's clock: a population decaying ──────────────────────────
  function clock(root) {
    var st = { p: 0, t: 0, play: true };
    var frame = new L.Frame(root, {
      reset: function () { st.t = 0; st.play = true; },
      tick: function (dt) { if (!st.play) return false; st.t += dt * 0.5; if (st.t > 5) st.t = 0; return true; },
      onSeen: function () {},
      draw: function (ctx, w, h, zoom) {
        var P = DW().decay.parents[st.p], X = DW().decay.xs, top = 50, wide = w >= 620;
        var gw = wide ? Math.min(w * 0.42, h - top - 10) : w - 16, gh = wide ? gw : Math.min((h - top) * 0.42, w * 0.6);
        var cols = 20, rows = Math.ceil(P.atoms.length / cols), cell = Math.min(gw / cols, gh / rows) * Math.min(zoom, 1.5);
        var counts = P.species.map(function () { return 0; });
        P.atoms.forEach(function (a, i) {
          var k = stageAt(a, st.t); counts[k]++;
          var x = 8 + (i % cols) * cell + cell / 2, y = top + Math.floor(i / cols) * cell + cell / 2;
          ctx.fillStyle = SCOL[k % SCOL.length]; ctx.beginPath(); ctx.arc(x, y, Math.max(1.5, cell * 0.38), 0, 2 * Math.PI); ctx.fill();
        });
        // Fractions against t / T½.
        var bx0 = wide ? 8 + gw + 56 : 52, by0 = wide ? top : top + rows * cell + 34;
        var B = new Box(bx0, by0, w - 12, h - 44, [0, 5], [0, 1], false, false);
        B.frame(ctx, "time / parent half-life", "fraction of atoms", linTicks(0, 5, 1), linTicks(0, 1, 0.25));
        B.line(ctx, X, DW().decay.law, fg(), 1.5, [5, 4]);
        P.species.forEach(function (s, k) {
          if (Math.max.apply(null, P.cram[k]) < 0.01) return;
          B.line(ctx, X, P.cram[k], SCOL[k % SCOL.length], 1, [2, 3]);
          B.line(ctx, X, P.mc[k], SCOL[k % SCOL.length], 2);
        });
        ctx.strokeStyle = mut(); ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(B.X(st.t), B.y0); ctx.lineTo(B.X(st.t), B.y1); ctx.stroke();
        ctx.fillStyle = SCOL[0]; ctx.beginPath(); ctx.arc(B.X(st.t), B.Y(counts[0] / P.atoms.length), 4, 0, 2 * Math.PI); ctx.fill();
        var items = P.species.map(function (s, k) { return [s + " " + counts[k], SCOL[k % SCOL.length]]; }).filter(function (x, k) { return k === 0 || counts[k] > 0 || Math.max.apply(null, P.cram[k]) >= 0.01; });
        items.push(["2^(−t/T½)", fg(), [5, 4]]);
        legend(ctx, items, 8, 42 - 12, w - 16);
        return P.name + " (T½ = " + fmtT(P.t12) + "), t = " + st.t.toFixed(2) + " T½: " + counts[0] + " of " + P.atoms.length + " drawn atoms are still " + P.name + " (law: " + (P.atoms.length * Math.pow(2, -st.t)).toFixed(0) + "). Solid lines: " + DW().decay.nBig + " atoms counted; dotted: CRAM48; dashed: the decay law.";
      },
      panel: function (body, redraw) {
        L.buttons(body, "Parent", DW().decay.parents.map(function (p, k) { return [k, p.name]; }), 0, function (v) { st.p = v; st.t = 0; redraw(); frame.play(); });
        L.toggle(body, "Play", true, function (on) { st.play = on; if (on) frame.play(); });
        var s = L.slider(body, "Time (half-lives)", 0, 5, 0.05, 0, function (v) { st.t = v; redraw(); return v.toFixed(2); });
        s.addEventListener("input", function () { st.play = false; });
        body.append(el("p", { "class": "w-note" }, "Each dot is one atom, coloured by what it is now. Its whole future (branch and lifetimes) was sampled at birth by boon-lay; the page only reads its clock."));
      }
    });
    return frame;
  }

  // ─── Which branch: the bar of ratios ─────────────────────────────────
  function branchBar(root) {
    var frame = new L.Frame(root, {
      reset: function () {}, tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var D = DW().decay, top = 52, rowH = Math.min(64, (h - top - 10) / D.parents.length) * Math.min(zoom, 1.3), x0 = 70, x1 = w - 12;
        ctx.font = "11px system-ui, sans-serif";
        D.parents.forEach(function (P, r) {
          var y = top + r * rowH;
          ctx.fillStyle = fg(); ctx.font = "bold 12px system-ui, sans-serif"; ctx.fillText(P.name, 6, y + 16); ctx.font = "11px system-ui, sans-serif";
          var x = x0, bs = P.branches.filter(function (b) { return b.to !== "none"; });
          [["data", bs.map(function (b) { return b.ratio; })], ["sampled", P.firstBranch]].forEach(function (row, j) {
            var yy = y + 4 + j * (rowH * 0.36), hh = rowH * 0.3; x = x0;
            row[1].forEach(function (f, k) {
              var ww = f * (x1 - x0);
              ctx.fillStyle = SCOL[(k + 1) % SCOL.length]; ctx.globalAlpha = j ? 0.55 : 0.9; ctx.fillRect(x, yy, ww, hh); ctx.globalAlpha = 1;
              ctx.fillStyle = "#fff"; var lab = bs[k].to + " " + (100 * f).toFixed(2) + " %";
              if (ctx.measureText(lab).width < ww - 6) ctx.fillText(lab, x + 4, yy + hh - 4);
              x += ww;
            });
            ctx.fillStyle = mut(); ctx.fillText(row[0], 6, yy + hh - 3 + (j ? 0 : 14));
          });
        });
        return "Top bar of each: the library's branching ratios (ENDF/B-VIII.0). Lower bar: where " + D.nBig + " sampled atoms actually went first. A uniform random number lands on the bar; its slot is the branch.";
      },
      panel: function (body) { body.append(el("p", { "class": "w-note" }, "Recorded: the ratios are read from boon-lay's DecayLibrary, the sampled shares from its StochasticDecayChain.")); }
    });
    return frame;
  }

  // ─── One Gaussian step from the buffer ───────────────────────────────
  function rings(ctx, cx, cy, s, R) {
    for (var k = 4; k >= 0; k--) { ctx.fillStyle = RCOL[k]; ctx.globalAlpha = 0.35; ctx.beginPath(); ctx.arc(cx, cy, R[k] * s, 0, 2 * Math.PI); ctx.fill(); }
    ctx.globalAlpha = 1;
    for (k = 0; k < 5; k++) { ctx.strokeStyle = mut(); ctx.lineWidth = 1; ctx.beginPath(); ctx.arc(cx, cy, R[k] * s, 0, 2 * Math.PI); ctx.stroke(); }
  }
  function gaussStep(root) {
    var st = { k: 2 };
    var frame = new L.Frame(root, {
      reset: function () { st.k = 2; }, tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var W = DW().walk, S = W.steps[st.k], R = W.radii, top = 44, wide = w >= 620;
        var size = wide ? Math.min(w * 0.6, h - top - 8) : Math.min(w - 16, (h - top) * 0.7);
        var cx = 8 + size / 2, cy = top + size / 2, s = size / 2 / (R[4] * 1.35) * zoom;
        ctx.save(); ctx.beginPath(); ctx.rect(0, top, wide ? size + 16 : w, size); ctx.clip();
        rings(ctx, cx, cy, s, R);
        S.pts.forEach(function (p) {
          ctx.strokeStyle = mut(); ctx.globalAlpha = 0.35; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(cx + p[0] * s, cy - p[1] * s); ctx.lineTo(cx + p[2] * s, cy - p[3] * s); ctx.stroke(); ctx.globalAlpha = 1;
          ctx.fillStyle = RCOL[p[4]]; ctx.strokeStyle = fg(); ctx.lineWidth = 0.5; ctx.beginPath(); ctx.arc(cx + p[2] * s, cy - p[3] * s, 3, 0, 2 * Math.PI); ctx.fill(); ctx.stroke();
        });
        ctx.restore();
        // Where the steps landed (20 000 of them).
        var bx = wide ? size + 30 : 70, by = wide ? top + 10 : top + size + 10, bw = (wide ? w - bx - 50 : w - bx - 50), bh = Math.max(12, Math.min(22, ((wide ? h - top : h - by) - 20) / 6 - 4));
        ctx.font = "11px system-ui, sans-serif";
        S.frac.forEach(function (f, r) {
          var y = by + r * (bh + 4);
          ctx.fillStyle = fg(); ctx.fillText(RNAME[r], bx - 60 < 4 ? 4 : bx - 60, y + bh - 4);
          ctx.fillStyle = RCOL[r]; ctx.fillRect(bx, y, Math.max(1, f * bw), bh);
          ctx.fillStyle = fg(); ctx.fillText((100 * f).toFixed(1) + " %", bx + f * bw + 4, y + bh - 4);
        });
        var past = S.frac[3] + S.frac[4] + S.frac[5];
        return "σ = " + S.sigmaUm.toFixed(0) + " µm per component (Δt = " + sci(S.dt) + " s at boon-lay's buffer D): " + (100 * past).toFixed(1) + " % of steps from the buffer land in the SiC or beyond, jumping the whole 40 µm IPyC. Lines: 300 steps (start to landing), drawn on the slice; colour: the layer the landing point is really in.";
      },
      panel: function (body, redraw) {
        L.buttons(body, "Step size σ", DW().walk.steps.map(function (s, k) { return [k, s.sigmaUm.toFixed(0) + " µm"]; }), 2, function (v) { st.k = v; redraw(); });
        body.append(el("p", { "class": "w-note" }, "One Gaussian step per atom, by boon-lay's sample_dimensioned_gaussian_vector; the layer at the landing point by the assembled TrisoCell (CRP-6 radii)."));
      }
    });
    return frame;
  }

  // ─── Walkers hopping on spheres in a bare kernel ─────────────────────
  function wosHop(root) {
    var st = { hop: 0, acc: 0, one: false, play: true };
    function maxHops() { return Math.max.apply(null, DW().walk.paths.map(function (p) { return p.length - 1; })); }
    var frame = new L.Frame(root, {
      reset: function () { st.hop = 0; st.acc = 0; st.play = true; },
      tick: function (dt) { if (!st.play) return false; st.acc += dt; if (st.acc > 0.45) { st.acc = 0; st.hop++; if (st.hop > maxHops() + 3) st.hop = 0; } return true; },
      draw: function (ctx, w, h, zoom) {
        var W = DW().walk, R = W.radii[0], top = 44, size = Math.min(w - 16, h - top - 8);
        var cx = w / 2, cy = top + size / 2, s = size / 2 / (R * 1.08) * zoom;
        ctx.fillStyle = RCOL[0]; ctx.globalAlpha = 0.3; ctx.beginPath(); ctx.arc(cx, cy, R * s, 0, 2 * Math.PI); ctx.fill(); ctx.globalAlpha = 1;
        ctx.strokeStyle = "#c0392b"; ctx.lineWidth = 2; ctx.beginPath(); ctx.arc(cx, cy, R * s, 0, 2 * Math.PI); ctx.stroke();
        var paths = st.one ? W.paths.slice(0, 1) : W.paths, tmax = 0, out = 0;
        paths.forEach(function (p, j) {
          var col = SCOL[(j + 1) % SCOL.length], n = Math.min(st.hop, p.length - 1);
          ctx.strokeStyle = col; ctx.lineWidth = 1.5; ctx.beginPath(); ctx.moveTo(cx + p[0][0] * s, cy - p[0][1] * s);
          for (var i = 1; i <= n; i++) ctx.lineTo(cx + p[i][0] * s, cy - p[i][1] * s);
          ctx.stroke();
          if (n < p.length - 1) {
            // The next sphere: centred on the walker, touching the surface.
            var c = p[n], r = p[n + 1][3];
            ctx.setLineDash([4, 3]); ctx.beginPath(); ctx.arc(cx + c[0] * s, cy - c[1] * s, r * s, 0, 2 * Math.PI); ctx.stroke(); ctx.setLineDash([]);
          } else out++;
          ctx.fillStyle = col; ctx.beginPath(); ctx.arc(cx + p[n][0] * s, cy - p[n][1] * s, 4, 0, 2 * Math.PI); ctx.fill();
          tmax = Math.max(tmax, p[n][4]);
        });
        ctx.strokeStyle = mut(); ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(12, h - 14); ctx.lineTo(12 + 50 * s, h - 14); ctx.stroke(); ctx.fillStyle = mut(); ctx.font = "10px system-ui, sans-serif"; ctx.fillText("50 µm", 12, h - 18);
        return "Hop " + st.hop + ": " + out + " of " + paths.length + " walkers captured at the kernel surface (red). Dashed: the sphere each will hop on next, the largest that stays in the kernel. Each walker's clock: up to " + fmtT(tmax) + ". Projected onto a plane; recorded from boon-lay's WoSWalker::hop at 1200 °C.";
      },
      panel: function (body, redraw) {
        L.toggle(body, "Play", true, function (on) { st.play = on; if (on) frame.play(); });
        L.toggle(body, "Only one walker", false, function (on) { st.one = on; redraw(); });
        var b = el("button", { type: "button", "class": "w-opt" }, "Next hop");
        b.onclick = function () { st.play = false; st.hop++; redraw(); };
        body.append(b);
      }
    });
    return frame;
  }

  // ─── The release fraction against Crank ──────────────────────────────
  function release(root) {
    var st = { k: 0, gauss: true };
    var frame = new L.Frame(root, {
      reset: function () { st.k = 0; }, tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var Rl = DW().walk.release[st.k], top = 60, H = Rl.hours, tmax = H[H.length - 1];
        var B = new Box(54, top, w - 12, h - 44, [0, tmax / Math.max(zoom, 1)], [0, 1.0], false, false);
        B.frame(ctx, "time (h)", "fraction released", linTicks(0, tmax, tmax / 4), linTicks(0, 1, 0.25));
        B.line(ctx, H, Rl.crank, fg(), 2);
        var items = [["Crank (exact)", fg()]];
        if (st.gauss && Rl.gauss) Rl.gauss.forEach(function (g, j) {
          var col = ["#c0392b", "#d4a017", "#7a4cc2"][j]; B.line(ctx, H, g.frac, col, 1.5, [5, 3]);
          items.push(["Gaussian step σ = R/" + Math.round(1 / g.sOverR), col, [5, 3]]);
        });
        ctx.fillStyle = "#2f7fd8";
        H.forEach(function (x, i) { if (x <= B.xr[1]) { ctx.beginPath(); ctx.arc(B.X(x), B.Y(Rl.wos[i]), 3, 0, 2 * Math.PI); ctx.fill(); } });
        items.push(["Walk-on-Spheres, 10 000 walkers", "#2f7fd8"]);
        legend(ctx, items, 8, 44 - 12, w - 16);
        return "CRP-6 Case 1" + (Rl.temp === 1200 ? "a" : "b") + ", " + Rl.temp + " °C, D = " + sci(Rl.d) + " m²/s. Walk-on-Spheres within " + Rl.worst[0].toFixed(1) + " σ of Crank at every point. The record's check at 200 h (40 000 walkers): " + Rl.case1.toFixed(4) + " against " + Rl.case1Crank.toFixed(4) + ".";
      },
      panel: function (body, redraw) {
        L.buttons(body, "Case", [[0, "1a, 1200 °C"], [1, "1b, 1600 °C"]], 0, function (v) { st.k = v; redraw(); });
        L.toggle(body, "Fixed-step Gaussian walks (1a)", true, function (on) { st.gauss = on; redraw(); });
      }
    });
    return frame;
  }

  // Numbers in the prose, bound by data-dw="...".
  function fillDw() {
    var D = DW().decay, W = DW().walk, byName = {};
    D.parents.forEach(function (p) { byName[p.name] = p; });
    var bi = byName["Bi-212"], cs = byName["Cs-137"];
    var br = function (P, to) { var k = P.branches.filter(function (b) { return b.to !== "none"; }).map(function (b) { return b.to; }).indexOf(to); return k; };
    var worstAll = 0; D.parents.forEach(function (p) { p.worst.forEach(function (x) { worstAll = Math.max(worstAll, x[0]); }); });
    var survAll = 0; D.parents.forEach(function (p) { survAll = Math.max(survAll, p.surviveWorst[0]); });
    var r1a = W.release[0], r1b = W.release[1];
    var map = {
      nBig: D.nBig.toLocaleString("en"), nShow: String(D.nShow),
      biTl: (100 * bi.branches[br(bi, "Tl-208")].ratio).toFixed(2) + " %", biTlMc: (100 * bi.firstBranch[br(bi, "Tl-208")]).toFixed(2) + " %",
      biPo: (100 * bi.branches[br(bi, "Po-212")].ratio).toFixed(2) + " %",
      csM: (100 * cs.branches[br(cs, "Ba-137m")].ratio).toFixed(1) + " %", csMMc: (100 * cs.firstBranch[br(cs, "Ba-137m")]).toFixed(1) + " %",
      worstAll: worstAll.toFixed(1), survAll: survAll.toFixed(1),
      bufD: sci(W.bufferD), sig141dt: sci(W.steps[2].dt) + " s", sig447dt: sci(W.steps[3].dt) + " s",
      past141: (100 * (W.steps[2].frac[3] + W.steps[2].frac[4] + W.steps[2].frac[5])).toFixed(0) + " %",
      past447: (100 * (W.steps[3].frac[3] + W.steps[3].frac[4] + W.steps[3].frac[5])).toFixed(0) + " %",
      past14: (100 * (W.steps[0].frac[3] + W.steps[0].frac[4] + W.steps[0].frac[5])).toFixed(2) + " %",
      out447: (100 * W.steps[3].frac[5]).toFixed(0) + " %",
      hopsMean: W.hopsMean.toFixed(1), hopsMax: String(W.hopsMax), theta: W.theta[0].toFixed(4) + " ± " + W.theta[1].toFixed(4),
      d1a: sci(r1a.d) + " m²/s", d1b: sci(r1b.d) + " m²/s", c1a: r1a.case1.toFixed(4), c1aCrank: r1a.case1Crank.toFixed(4), c1b: r1b.case1.toFixed(4),
      w1a: r1a.worst[0].toFixed(1), w1b: r1b.worst[0].toFixed(1),
      g10: (r1a.crank[50] - r1a.gauss[0].frac[50]).toFixed(3), g30: (r1a.crank[50] - r1a.gauss[1].frac[50]).toFixed(3), g100: (r1a.crank[50] - r1a.gauss[2].frac[50]).toFixed(3)
    };
    document.querySelectorAll("[data-dw]").forEach(function (n) { var k = n.getAttribute("data-dw"); if (k in map) n.textContent = map[k]; });
  }
  L.widget("dw-numbers", fillDw);
  L.widget("dw-clock", clock);
  L.widget("dw-branch", branchBar);
  L.widget("dw-gauss", gaussStep);
  L.widget("dw-hop", wosHop);
  L.widget("dw-release", release);
})();
