// Neutronics lessons, page 18 (gh:#906 design and amendments, 2026-10-16):
// the unresolved resonance range and its smeared, infinitely dilute average.
// Data: unresolved-data.js, written by
// crates/njoy-outram-park-fork/examples/lesson_unresolved.rs: capture from
// OUTRAM PARK's RECONR port at 0 K (in the unresolved range, the infinitely
// dilute average: U-238's own File 3, LSSF = 1; computed from the average
// parameters for U-234, LSSF = 0), its running average over E/(1+W)..E(1+W),
// and each spin sequence's tabulated mean spacing D(E).
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame;
  var D = window.UNRESOLVED_DATA;

  // D(E) of one spin sequence, interpolated with its own ENDF law (INT 2
  // lin-lin, INT 5 log-log).
  function spacing(seq, E) {
    var d = seq.d;
    for (var i = 1; i < d.length; i++) if (d[i][0] >= E) {
      var x0 = d[i - 1][0], y0 = d[i - 1][1], x1 = d[i][0], y1 = d[i][1];
      if (seq.int === 5) return Math.exp(Math.log(y0) + Math.log(E / x0) / Math.log(x1 / x0) * Math.log(y1 / y0));
      return y0 + (y1 - y0) * (E - x0) / (x1 - x0);
    }
    return d[d.length - 1][1];
  }
  // The running average nearest to E (on the example's log grid).
  function avgAt(N, E) {
    var a = N.avg, best = 0;
    for (var i = 1; i < a.e.length; i++) if (Math.abs(Math.log(a.e[i] / E)) < Math.abs(Math.log(a.e[best] / E))) best = i;
    return { e: a.e[best], s: a.s[best] };
  }
  L.unresolvedMath = { spacing: spacing, avgAt: avgAt };

  function smearWidget(root) {
    var st = { nuc: "U-238", stage: "forest" };
    var frame = new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var N = D[st.nuc], x0 = 46, y0 = 84, pw = w - x0 - 12, ph = h - y0 - 56;
        var lx0 = 2, lx1 = Math.log10(2e5), ly1 = 4, ly0 = -2;
        var mid = (Math.log10(N.el) + lx0) / 2, span = (lx1 - lx0) / zoom;
        if (zoom > 1) { lx0 = Math.max(2, Math.log10(N.el) - span / 2); lx1 = lx0 + span; }
        var X = function (e) { return x0 + (Math.log10(e) - lx0) / (lx1 - lx0) * pw; };
        var Y = function (s) { return y0 + (ly1 - Math.log10(Math.max(s, 1e-9))) / (ly1 - ly0) * ph; };
        ctx.font = "12px system-ui, sans-serif"; ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        // The unresolved range.
        var xu0 = X(N.el), xu1 = X(N.eh);
        if (st.stage === "urr") { ctx.fillStyle = "rgba(184,92,56,0.10)"; ctx.fillRect(xu0, y0, xu1 - xu0, ph); }
        ctx.strokeStyle = "rgba(128,128,128,0.22)";
        for (var d = 2; d <= 5; d++) { var gx = X(Math.pow(10, d)); ctx.beginPath(); ctx.moveTo(gx, y0); ctx.lineTo(gx, y0 + ph); ctx.stroke(); }
        // The resolved forest; from the "urr" stage on, also the dilute average above it.
        var C = N.cap;
        ctx.strokeStyle = acc; ctx.lineWidth = 1; ctx.globalAlpha = st.stage === "forest" ? 0.9 : 0.35; ctx.beginPath();
        for (var i = 0, first = true; i < C.e.length; i++) {
          var e = C.e[i];
          if (e < 100 || (e > N.el && st.stage !== "urr")) continue;
          if (first) { ctx.moveTo(X(e), Y(C.s[i])); first = false; } else ctx.lineTo(X(e), Y(C.s[i]));
        }
        ctx.stroke(); ctx.globalAlpha = 1;
        if (st.stage === "urr") {
          ctx.strokeStyle = hot; ctx.lineWidth = 2.5; ctx.beginPath(); first = true;
          for (i = 0; i < C.e.length; i++) { e = C.e[i]; if (e < N.el || e > N.eh) continue; if (first) { ctx.moveTo(X(e), Y(C.s[i])); first = false; } else ctx.lineTo(X(e), Y(C.s[i])); }
          ctx.stroke();
        }
        // The running average of the forest.
        if (st.stage !== "forest") {
          var A = N.avg; ctx.strokeStyle = fg; ctx.lineWidth = 2; ctx.beginPath(); first = true;
          for (i = 0; i < A.e.length; i++) {
            if (A.e[i] * 1.05 > N.el) break;
            if (first) { ctx.moveTo(X(A.e[i]), Y(A.s[i])); first = false; } else ctx.lineTo(X(A.e[i]), Y(A.s[i]));
          }
          ctx.stroke();
        }
        ctx.strokeStyle = hot; ctx.setLineDash([4, 3]); ctx.beginPath(); ctx.moveTo(xu0, y0); ctx.lineTo(xu0, y0 + ph); ctx.stroke(); ctx.setLineDash([]);
        ctx.restore();
        ctx.fillStyle = mut;
        [["100 eV", 2], ["1 keV", 3], ["10 keV", 4], ["100 keV", 5]].forEach(function (t) { var gx = X(Math.pow(10, t[1])); if (gx < x0 - 1 || gx > x0 + pw + 1) return; ctx.fillText(t[0], gx - ctx.measureText(t[0]).width / 2, y0 + ph + 16); });
        for (var b = ly0; b <= ly1; b += 2) ctx.fillText("10" + String(b).replace("-", "⁻").replace(/[0-9]/g, function (c) { return "⁰¹²³⁴⁵⁶⁷⁸⁹"[+c]; }), 6, Y(Math.pow(10, b)) + 4);
        ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif";
        ctx.fillText(st.nuc + " capture σ [b]", x0, y0 - 26);
        ctx.font = "12px system-ui, sans-serif"; ctx.fillStyle = hot;
        ctx.fillText(st.stage === "urr" ? "resolved | unresolved, " + fmtE(N.el) + "–" + fmtE(N.eh) : "resolved range ends at " + fmtE(N.el), x0, y0 - 8);
        var lo = avgAt(N, N.el / 1.06), hi = avgAt(N, N.el * 1.08);
        if (st.stage === "forest") return st.nuc + "'s capture cross section through its resolved range, reconstructed from the evaluation's resonance parameters by OUTRAM PARK's RECONR port at 0 K. Every spike is one resonance. Press Smear it.";
        if (st.stage === "smear") return "Black: the plain average of the forest over a window ±5 % wide in energy at each point. At " + fmtE(lo.e) + " it is " + lo.s.toPrecision(3) + " b. The individual peaks are gone; the average is all that is left. Where the window holds only a few resonances (below about 1 keV for U-238) the average still jumps; higher up it holds dozens and runs smooth.";
        return "Shaded: the unresolved range. The evaluation lists no individual resonances there, only averages, and the brown curve is the infinitely dilute average: " +
          (N.lssf === 1 ? "for " + st.nuc + " (LSSF = 1) the evaluator's own File 3, taken from measurements." : "for " + st.nuc + " (LSSF = 0) computed by RECONR from the average parameters.") +
          " Just below the boundary the smeared forest gives " + lo.s.toPrecision(3) + " b at " + fmtE(lo.e) + "; just above, the dilute average over the same window is " + hi.s.toPrecision(3) + " b at " + fmtE(hi.e) + ".";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "Capture (MT 102), ENDF/B-VIII.0, at 0 K. The forest is thinned for a phone (the highest and lowest point of each of 900 log-E steps); the averages are taken on every reconstructed point."));
      }
    });
    var card = root.parentNode;
    var pick = function (attr, set) {
      card.querySelectorAll("[" + attr + "]").forEach(function (b) {
        b.onclick = function () {
          set(b.getAttribute(attr));
          card.querySelectorAll("[" + attr + "]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
          frame.redraw();
        };
      });
    };
    pick("data-nuc", function (v) { st.nuc = v; });
    pick("data-stage", function (v) { st.stage = v; });
    return frame;
  }

  L.widget("smear", smearWidget);
})();
