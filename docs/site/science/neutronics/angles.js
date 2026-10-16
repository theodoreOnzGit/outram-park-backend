// Neutronics lessons: how nuclear data stores scattering angles (gh:#906
// checkpoint §21-§23, angular addendum §8-§13). Real data only: the
// ENDF/B-VIII.0 Legendre coefficients of C-12 and U-238 elastic scattering
// and the table ACER writes from them (angular-data.js, with provenance).
//
//   ENDF (MF=4, LTT=1 or 3; ENDF-6 manual §4.2.1):
//       f(μ) = Σ_{l=0}^{NL} (2l+1)/2 · a_l · P_l(μ),  a_0 = 1
//   ACE: the (μ, pdf, cdf) table ptleg2 makes of it, sampled lin-lin by
//       sampleTabular, a port of outram-mc-libs material/nuclide.rs
//       sample_tabular_mu (itself OpenMC's Tabular::sample, lin-lin).
//
// Guardrails: the Legendre coefficients are not what ACE stores; the
// distributions are in the centre-of-mass frame (LCT = 2).
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame, buttons = L.buttons, toggle = L.toggle, arrow = L.arrow;

  function sub(n) { return String(n).replace(/[0-9]/g, function (d) { return "₀₁₂₃₄₅₆₇₈₉"[+d]; }); }
  function legendre(n, mu, p) {
    p[0] = 1; if (n > 0) p[1] = mu;
    for (var l = 1; l < n; l++) p[l + 1] = ((2 * l + 1) * mu * p[l] - l * p[l - 1]) / (l + 1);
    return p;
  }
  // The ENDF density with the first `upto` coefficients (upto = 0: P0 only).
  function density(a, upto, mu) {
    var p = legendre(upto, mu, new Float64Array(upto + 1)), y = 0.5;
    for (var l = 1; l <= upto; l++) y += 0.5 * (2 * l + 1) * a[l - 1] * p[l];
    return y;
  }
  // Lin-lin inversion of a tabulated cosine cdf: outram-mc-libs
  // sample_tabular_mu, line for line.
  function sampleTabular(mu, pdf, cdf, xi) {
    var n = mu.length;
    if (n === 1) return Math.max(-1, Math.min(1, mu[0]));
    var k = 0;
    while (k + 1 < n && cdf[k + 1] <= xi) k++;
    k = Math.min(k, n - 2);
    var xi0 = mu[k], x1 = mu[k + 1], p0 = pdf[k], p1 = pdf[k + 1], c0 = cdf[k], dx = x1 - xi0;
    if (dx <= 0) return Math.max(-1, Math.min(1, xi0));
    var m = (p1 - p0) / dx, x;
    if (m === 0) x = p0 > 0 ? xi0 + (xi - c0) / p0 : xi0;
    else x = xi0 + (Math.sqrt(Math.max(0, p0 * p0 + 2 * m * (xi - c0))) - p0) / m;
    return Math.max(-1, Math.min(1, x));
  }

  // ─── The Legendre shapes (supporting) ──────────────────────────────────
  function pshapes(root) {
    var st = { pick: -1 };
    var COLS = ["#1f6f78", "#b85c38", "#7a5cc0", "#2f8f3a", "#c0902a", "#c03a7a"];
    return new Frame(root, {
      reset: function () { st.pick = -1; },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var mut = css("--muted", "#5b636b"), fg = css("--fg", "#1d2125");
        var y0 = 56, x0 = 10, pw = w - 20, ph = h - y0 - 28, mid = y0 + ph / 2, amp = ph / 2 * 0.9 * zoom;
        ctx.strokeStyle = mut; ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(x0, mid); ctx.lineTo(x0 + pw, mid); ctx.moveTo(x0 + pw / 2, y0); ctx.lineTo(x0 + pw / 2, y0 + ph); ctx.stroke();
        var p = new Float64Array(6);
        for (var l = 0; l <= 5; l++) {
          var on = st.pick < 0 || st.pick === l;
          ctx.strokeStyle = COLS[l]; ctx.globalAlpha = on ? 1 : 0.18; ctx.lineWidth = st.pick === l ? 3 : 2;
          ctx.beginPath();
          for (var i = 0; i <= 200; i++) {
            var mu = -1 + 2 * i / 200;
            legendre(5, mu, p);
            var X = x0 + (mu + 1) / 2 * pw, Y = mid - p[l] * amp;
            if (i) ctx.lineTo(X, Y); else ctx.moveTo(X, Y);
          }
          ctx.stroke();
        }
        ctx.globalAlpha = 1;
        ctx.font = "13px system-ui, sans-serif"; ctx.fillStyle = mut;
        ctx.fillText("μ = −1 (back)", x0, h - 8);
        var t = "μ = +1 (forward)"; ctx.fillText(t, x0 + pw - ctx.measureText(t).width, h - 8);
        ctx.fillStyle = fg;
        return st.pick < 0 ? "P₀ … P₅ together. Each higher one wiggles once more between backward and forward; each equals 1 at μ = +1."
          : "P" + sub(st.pick) + "(μ): " + ["flat: no preference for any direction", "a straight tilt: more forward than back (or the reverse, if its coefficient is negative)", "one bend: favours forward and backward over sideways", "two bends", "three bends", "four bends"][st.pick] + ".";
      },
      panel: function (body, redraw) {
        buttons(body, "Show", [[-1, "All"], [0, "P₀"], [1, "P₁"], [2, "P₂"], [3, "P₃"], [4, "P₄"], [5, "P₅"]], st.pick, function (v) { st.pick = v; redraw(); });
      }
    });
  }

  // ─── Rebuild a real distribution, see what ACE stores, sample it ───────
  function rebuild(root) {
    var D = window.ANGULAR_DATA;
    var st = { nuc: "U-238", ei: 2, upto: 1, terms: true, ace: false, samples: [], rnd: 2463 };
    var termsLabel = null;
    function rec() { return D[st.nuc][st.ei]; }
    function rand() {
      st.rnd ^= st.rnd << 13; st.rnd ^= st.rnd >>> 17; st.rnd ^= st.rnd << 5;
      return (st.rnd >>> 0) / 4294967296;
    }
    function setTerms() {
      if (termsLabel) termsLabel.textContent = "Legendre terms: P₀ … P" + sub(st.upto) + " (the evaluation has P₀ … P" + sub(rec().a.length) + ")";
    }
    return new Frame(root, {
      reset: function () { st.samples = []; },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), brown = "#b85c38";
        var R = rec(), NL = R.a.length, N = 241, full = new Float64Array(N), part = new Float64Array(N);
        var lo = 0, hi = 0, i, mu;
        for (i = 0; i < N; i++) {
          mu = -1 + 2 * i / (N - 1);
          full[i] = density(R.a, NL, mu); part[i] = density(R.a, st.upto, mu);
          lo = Math.min(lo, part[i], full[i]); hi = Math.max(hi, part[i], full[i]);
        }
        if (st.ace) for (i = 0; i < R.pdf.length; i++) hi = Math.max(hi, R.pdf[i]);
        hi *= 1.08 / zoom; lo = lo < 0 ? lo * 1.1 / zoom : 0;
        var y0 = 58, x0 = 10, pw = w - 20, ph = h - y0 - 30;
        var X = function (m) { return x0 + (m + 1) / 2 * pw; }, Y = function (v) { return y0 + (hi - v) / (hi - lo) * ph; };
        ctx.save(); ctx.beginPath(); ctx.rect(0, y0 - 4, w, ph + 8); ctx.clip();
        ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(x0, Y(0)); ctx.lineTo(x0 + pw, Y(0)); ctx.stroke();
        function curve(arr, col, width, dash) {
          ctx.strokeStyle = col; ctx.lineWidth = width; ctx.setLineDash(dash || []);
          ctx.beginPath();
          for (var j = 0; j < N; j++) { var xx = X(-1 + 2 * j / (N - 1)), yy = Y(arr[j]); if (j) ctx.lineTo(xx, yy); else ctx.moveTo(xx, yy); }
          ctx.stroke(); ctx.setLineDash([]);
        }
        if (st.terms) {
          var p = new Float64Array(st.upto + 1);
          for (var l = 0; l <= st.upto; l++) {
            var t = new Float64Array(N), c = l === 0 ? 0.5 : 0.5 * (2 * l + 1) * R.a[l - 1];
            for (i = 0; i < N; i++) { legendre(st.upto, -1 + 2 * i / (N - 1), p); t[i] = c * p[l]; }
            curve(t, "rgba(184, 92, 56, 0.45)", 1);
          }
        }
        curve(full, "rgba(120, 120, 120, 0.6)", 5);
        curve(part, acc, 2);
        if (st.ace) {
          ctx.strokeStyle = fg; ctx.lineWidth = 1; ctx.beginPath();
          for (i = 0; i < R.mu.length; i++) { if (i) ctx.lineTo(X(R.mu[i]), Y(R.pdf[i])); else ctx.moveTo(X(R.mu[i]), Y(R.pdf[i])); }
          ctx.stroke();
          ctx.fillStyle = fg;
          for (i = 0; i < R.mu.length; i++) { ctx.beginPath(); ctx.arc(X(R.mu[i]), Y(R.pdf[i]), 2.5, 0, 2 * Math.PI); ctx.fill(); }
        }
        // Sampled cosines: a rug along the axis, the last one as a line.
        for (i = 0; i < st.samples.length; i++) {
          var last = i === st.samples.length - 1, xs = X(st.samples[i]);
          ctx.strokeStyle = last ? brown : "rgba(184, 92, 56, 0.4)"; ctx.lineWidth = last ? 2.5 : 1;
          ctx.beginPath(); ctx.moveTo(xs, last ? y0 : Y(0) - 8); ctx.lineTo(xs, Y(0) + 8); ctx.stroke();
        }
        ctx.restore();
        // The sampled direction, as the neutron leaves (top right inset).
        if (st.samples.length) {
          var ms = st.samples[st.samples.length - 1], th = Math.acos(ms), r = Math.min(54, w * 0.12), cx = w - r - 16, cy = y0 + r + 4;
          arrow(ctx, cx - r, cy, cx - 6, cy, mut);
          ctx.fillStyle = brown; ctx.beginPath(); ctx.arc(cx, cy, 5, 0, 2 * Math.PI); ctx.fill();
          arrow(ctx, cx, cy, cx + r * Math.cos(th), cy - r * Math.sin(th), brown);
        }
        ctx.font = "13px system-ui, sans-serif"; ctx.fillStyle = mut;
        ctx.fillText("μ = −1 back", x0, h - 8);
        var e1 = "forward μ = +1"; ctx.fillText(e1, x0 + pw - ctx.measureText(e1).width, h - 8);
        var e2 = "0 sideways"; ctx.fillText(e2, x0 + (pw - ctx.measureText(e2).width) / 2, h - 8);
        var worst = 0, pk = 0;
        for (i = 0; i < N; i++) { worst = Math.max(worst, Math.abs(full[i] - part[i])); pk = Math.max(pk, full[i]); }
        var s = st.nuc + " elastic at " + fmtE(R.e) + ": the evaluation stores " + NL + " coefficients. With P₀ … P" + sub(st.upto) +
          ", worst error " + (100 * worst / pk).toFixed(1) + " % of the peak" + (lo < 0 ? "; this partial sum goes negative, which no probability can" : "") + ".";
        if (st.ace) s += " ACE stores " + R.mu.length + " points (μ, density, cumulative), not the coefficients.";
        if (st.samples.length) {
          var ml = st.samples[st.samples.length - 1], mean = 0;
          for (i = 0; i < st.samples.length; i++) mean += st.samples[i];
          mean /= st.samples.length;
          s += " Last sample μ = " + ml.toFixed(3) + " (θ = " + (Math.acos(ml) * 180 / Math.PI).toFixed(1) + "°). Mean of " + st.samples.length +
            " samples " + mean.toFixed(3) + "; the exact mean cosine is a₁ = " + R.a[0].toFixed(3) + ".";
        }
        return s;
      },
      panel: function (body, redraw) {
        var reset = function () { st.samples = []; st.upto = Math.min(st.upto, rec().a.length); setTerms(); redraw(); };
        buttons(body, "Nucleus", [["C-12", "C-12"], ["U-238", "U-238"]], st.nuc, function (v) { st.nuc = v; reset(); });
        buttons(body, "Neutron energy", D["U-238"].map(function (r, k) { return [k, ["0.1", "0.5", "1", "2", "5"][k] + " MeV"]; }), st.ei, function (v) { st.ei = v; reset(); });
        termsLabel = el("span", { "class": "w-label" });
        var g = el("div", { "class": "w-group" }), row = el("div", { "class": "w-row" });
        var minus = el("button", { type: "button", "class": "w-opt" }, "− term"), plus = el("button", { type: "button", "class": "w-opt" }, "+ term"), all = el("button", { type: "button", "class": "w-opt" }, "All");
        minus.onclick = function () { st.upto = Math.max(0, st.upto - 1); setTerms(); redraw(); };
        plus.onclick = function () { st.upto = Math.min(rec().a.length, st.upto + 1); setTerms(); redraw(); };
        all.onclick = function () { st.upto = rec().a.length; setTerms(); redraw(); };
        row.append(minus, plus, all); g.append(termsLabel, row); body.append(g); setTerms();
        toggle(body, "Show each scaled Legendre shape", st.terms, function (v) { st.terms = v; redraw(); });
        toggle(body, "Show what ACE stores (the ACER table)", st.ace, function (v) { st.ace = v; redraw(); });
        var b = el("button", { type: "button", "class": "w-opt wide" }, "Sample scattering angle");
        b.onclick = function () { st.samples.push(sampleTabular(rec().mu, rec().pdf, rec().cdf, rand())); if (st.samples.length > 400) st.samples.shift(); redraw(); };
        var b10 = el("button", { type: "button", "class": "w-opt wide" }, "Sample 100 more");
        b10.onclick = function () { for (var k = 0; k < 100; k++) st.samples.push(sampleTabular(rec().mu, rec().pdf, rec().cdf, rand())); while (st.samples.length > 400) st.samples.shift(); redraw(); };
        body.append(b, b10);
        body.append(el("p", { "class": "w-note" }, "Grey: the evaluation's full Legendre series. Teal: the sum of the terms so far. Brown, faint: each scaled shape. Black dots: ACER's table, which the sampling uses. Centre-of-mass frame."));
      }
    });
  }

  L.widget("pshapes", pshapes);
  L.widget("rebuild", rebuild);
  L.anglesMath = { legendre: legendre, density: density, sampleTabular: sampleTabular };
})();
