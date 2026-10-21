// Neutronics lessons: how to store a continuous shape (gh:#906 checkpoint
// §16-§20, angular addendum §2-§7). The target is page 5's angular
// distribution (U-238, 2 MeV, the impenetrable sphere of qscatter.js), so
// the reader stores a curve they have already met. Two finite
// representations: values at points joined by straight lines, and a sum of
// scaled cosines (a Fourier cosine series on 0 ≤ θ ≤ π). Guardrails: no
// ENDF interpolation laws yet; Bessel functions are not "the 2D Fourier
// transform"; spherical harmonics are shapes on a sphere.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, Frame = L.Frame, buttons = L.buttons, toggle = L.toggle;

  // The target: dσ/dΩ against θ on [0, π], scaled to a peak of 1.
  function target() {
    var S = L.qscatterMath.solve(236.0058, 9.48, 2e6), f = new Float64Array(S.NM);
    for (var i = 0; i < S.NM; i++) f[i] = S.f[i] / S.fmax;
    return f;
  }
  // Store n values at equally spaced angles, join them with straight lines.
  function byPoints(f, n) {
    var NM = f.length, out = new Float64Array(NM), xs = [], ys = [];
    for (var k = 0; k < n; k++) {
      var t = k / (n - 1) * (NM - 1), i0 = Math.floor(t), u = t - i0;
      xs.push(t); ys.push(i0 + 1 < NM ? f[i0] + u * (f[i0 + 1] - f[i0]) : f[i0]);
    }
    for (var i = 0, s = 0; i < NM; i++) {
      while (s < n - 2 && xs[s + 1] < i) s++;
      var w = (i - xs[s]) / (xs[s + 1] - xs[s]);
      out[i] = ys[s] + w * (ys[s + 1] - ys[s]);
    }
    return { y: out, px: xs, py: ys };
  }
  // The first n terms of the cosine series f(θ) ≈ c_0 + Σ c_m cos(mθ),
  // c_0 = (1/π)∫f dθ, c_m = (2/π)∫f cos(mθ) dθ, by the trapezoid rule.
  function bySeries(f, n) {
    var NM = f.length, h = Math.PI / (NM - 1), c = [], out = new Float64Array(NM);
    for (var m = 0; m < n; m++) {
      var s = 0;
      for (var i = 0; i < NM; i++) s += (i === 0 || i === NM - 1 ? 0.5 : 1) * f[i] * Math.cos(m * i * h);
      c.push(s * h * (m === 0 ? 1 : 2) / Math.PI);
    }
    for (var j = 0; j < NM; j++) {
      var y = 0;
      for (var k = 0; k < n; k++) y += c[k] * Math.cos(k * j * h);
      out[j] = y;
    }
    return { y: out, c: c };
  }
  function worst(f, g) {
    var e = 0;
    for (var i = 0; i < f.length; i++) e = Math.max(e, Math.abs(f[i] - g[i]));
    return e;
  }

  function store(root) {
    var f = target(), NM = f.length;
    var st = { mode: "points", np: 8, ns: 4, terms: true };
    var countsRow = null, redrawRef = null;
    function counts(body) {
      if (countsRow) countsRow.remove();
      var holder = el("div");
      if (st.mode === "points") {
        buttons(holder, "Stored points", [[4, "4"], [8, "8"], [16, "16"], [32, "32"], [64, "64"]], st.np, function (v) { st.np = v; redrawRef(); });
      } else {
        buttons(holder, "Shape functions", [[1, "1"], [2, "2"], [3, "3"], [4, "4"], [8, "8"], [16, "16"]], st.ns, function (v) { st.ns = v; redrawRef(); });
      }
      countsRow = holder;
      body.insertBefore(holder, body.children[1] || null);
    }
    return new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        var y0 = 58, pad = 30, x0 = 8, plotW = w - 16, plotH = h - y0 - pad;
        var lo = st.mode === "points" ? -0.05 : -0.45, hi = 1.08;
        var mid = 0.5 * (lo + hi), half = 0.5 * (hi - lo) / zoom;
        lo = mid - half; hi = mid + half;
        var X = function (i) { return x0 + i / (NM - 1) * plotW; };
        var Y = function (v) { return y0 + (hi - v) / (hi - lo) * plotH; };
        ctx.font = "13px system-ui, sans-serif";
        ctx.strokeStyle = mut; ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(x0, Y(0)); ctx.lineTo(x0 + plotW, Y(0)); ctx.stroke();
        function curve(arr, col, width, dash) {
          ctx.strokeStyle = col; ctx.lineWidth = width; ctx.setLineDash(dash || []);
          ctx.beginPath();
          for (var i = 0; i < NM; i++) { if (i) ctx.lineTo(X(i), Y(arr[i])); else ctx.moveTo(X(i), Y(arr[i])); }
          ctx.stroke(); ctx.setLineDash([]);
        }
        var r, n, msg;
        if (st.mode === "points") {
          n = st.np; r = byPoints(f, n);
          curve(f, "rgba(120, 120, 120, 0.55)", 5);
          curve(r.y, acc, 2);
          ctx.fillStyle = acc;
          for (var k = 0; k < r.px.length; k++) { ctx.beginPath(); ctx.arc(X(r.px[k]), Y(r.py[k]), 4, 0, 2 * Math.PI); ctx.fill(); }
          msg = n + " numbers stored (the curve's height at " + n + " angles), joined by straight lines.";
        } else {
          n = st.ns; r = bySeries(f, n);
          curve(f, "rgba(120, 120, 120, 0.55)", 5);
          if (st.terms) {
            for (var m = 0; m < n; m++) {
              var t = new Float64Array(NM);
              for (var i = 0; i < NM; i++) t[i] = r.c[m] * Math.cos(m * i * Math.PI / (NM - 1));
              curve(t, "rgba(184, 92, 56, 0.55)", 1);
            }
          }
          curve(r.y, acc, 2);
          msg = n + " numbers stored (how much of each of " + n + " cosine shapes to add).";
        }
        ctx.fillStyle = mut;
        ctx.fillText("0° forward", x0, h - 8);
        var e1 = "180° backward"; ctx.fillText(e1, x0 + plotW - ctx.measureText(e1).width, h - 8);
        var e2 = "scattering angle θ"; ctx.fillText(e2, x0 + (plotW - ctx.measureText(e2).width) / 2, h - 8);
        ctx.fillStyle = fg;
        ctx.fillText("grey: the true curve · teal: rebuilt", x0 + 4, y0 + 4);
        return msg + " Worst error: " + (100 * worst(f, r.y)).toFixed(1) + " % of the peak.";
      },
      panel: function (body, redraw) {
        redrawRef = redraw;
        buttons(body, "Store it as", [["points", "Points"], ["shapes", "Shape functions"]], st.mode, function (v) { st.mode = v; counts(body); redraw(); });
        counts(body);
        toggle(body, "Show each scaled shape (shape functions)", st.terms, function (v) { st.terms = v; redraw(); });
        body.append(el("p", { "class": "w-note" }, "The curve is page 5's: U-238 at 2 MeV. Points are equally spaced in angle. The shapes are cosines, cos(mθ) for m = 0, 1, 2, …, each scaled by one stored number."));
      }
    });
  }

  // ─── Shape families (illustration) ─────────────────────────────────────
  // Bessel J_n by its integral form, J_n(x) = (1/π)∫₀^π cos(nτ − x sin τ) dτ
  // (Simpson, 200 panels: ample for x ≤ 16). The spherical harmonics are
  // drawn in a plane through their axis as r = |Y_lm(θ)|, coloured by sign:
  // Y_l0 ∝ P_l(cos θ); for m > 0 the azimuthal factor cos(mφ) is ±1 there.
  function besselJ(n, x) {
    var N = 200, h = Math.PI / N, s = 0;
    for (var i = 0; i <= N; i++) {
      var t = i * h, w = i === 0 || i === N ? 1 : i % 2 ? 4 : 2;
      s += w * Math.cos(n * t - x * Math.sin(t));
    }
    return s * h / 3 / Math.PI;
  }
  var HARMONICS = [
    ["l=0, m=0", function () { return 1; }],
    ["l=1, m=0", function (c) { return c; }],
    ["l=1, m=1", function (c, s) { return s; }],
    ["l=2, m=0", function (c) { return 0.5 * (3 * c * c - 1); }],
    ["l=2, m=1", function (c, s) { return 3 * s * c; }],
    ["l=2, m=2", function (c, s) { return 3 * s * s; }]
  ];
  function families(root) {
    var st = { fam: "trig" };
    return new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), fg = css("--fg", "#1d2125"), brown = "#b85c38";
        var y0 = 56, cols = st.fam === "sph" ? 3 : 2, rows = st.fam === "sph" ? 2 : 2, n = st.fam === "sph" ? 6 : 4;
        var cw = (w - 12) / cols, ch = (h - y0 - 6) / rows;
        ctx.font = "12px system-ui, sans-serif";
        for (var k = 0; k < n; k++) {
          var cx0 = 6 + (k % cols) * cw, cy0 = y0 + Math.floor(k / cols) * ch, mid = cy0 + ch / 2, amp = ch * 0.36 * zoom;
          ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(cx0 + 2, cy0 + 2, cw - 4, ch - 4);
          ctx.fillStyle = fg;
          if (st.fam === "sph") {
            var lab = HARMONICS[k][0], fn = HARMONICS[k][1], R = Math.min(cw, ch) * 0.36 * zoom, ox = cx0 + cw / 2, oy = mid + 6, mx = 0, i;
            for (i = 0; i <= 360; i++) { var th = i * Math.PI / 180; mx = Math.max(mx, Math.abs(fn(Math.cos(th), Math.abs(Math.sin(th))))); }
            for (var sign = 1; sign >= -1; sign -= 2) {
              ctx.fillStyle = sign > 0 ? "rgba(31, 111, 120, 0.55)" : "rgba(184, 92, 56, 0.55)";
              ctx.beginPath(); ctx.moveTo(ox, oy);
              for (i = 0; i <= 360; i++) {
                var t2 = i * Math.PI / 180, c = Math.cos(t2), s = Math.sin(t2), v = fn(c, Math.abs(s));
                // m odd: the far half (s < 0, φ = π) carries cos(mφ) = −1.
                if (s < 0 && (k === 2 || k === 4)) v = -v;
                var r = sign * v > 0 ? Math.abs(v) / mx * R : 0;
                ctx.lineTo(ox + r * s, oy - r * c);
              }
              ctx.closePath(); ctx.fill();
            }
            ctx.fillStyle = fg; ctx.fillText(lab, cx0 + 8, cy0 + 16);
            continue;
          }
          ctx.strokeStyle = mut; ctx.beginPath(); ctx.moveTo(cx0 + 6, mid); ctx.lineTo(cx0 + cw - 6, mid); ctx.stroke();
          ctx.strokeStyle = k % 2 ? brown : acc; ctx.lineWidth = 2; ctx.beginPath();
          for (var j = 0; j <= 200; j++) {
            var u = j / 200, x, y;
            if (st.fam === "trig") { x = 2 * Math.PI * u; y = k < 2 ? Math.sin((k + 1) * x) : Math.cos((k - 1) * x); }
            else { x = 16 * u; y = besselJ(k, x); }
            var X = cx0 + 6 + u * (cw - 12), Y = mid - y * amp;
            if (j) ctx.lineTo(X, Y); else ctx.moveTo(X, Y);
          }
          ctx.stroke();
          ctx.fillStyle = fg;
          ctx.fillText(st.fam === "trig" ? (k < 2 ? (k ? "sin 2x" : "sin x") : (k === 2 ? "cos x" : "cos 2x")) : "J" + k + "(r), from the axis outwards", cx0 + 8, cy0 + 16);
        }
        return st.fam === "trig" ? "Sines and cosines: the natural shapes of waves along a line (a Fourier series adds them)."
          : st.fam === "bessel" ? "Bessel functions: the natural radial shapes around an axis, as in a drum skin or a cylindrical fuel rod. They are not simply a two-dimensional Fourier series."
          : "Spherical harmonics: shapes on a sphere, drawn here in a slice through their axis (teal positive, brown negative). Those with m = 0 are Legendre polynomials in cos θ; m > 0 adds a sine/cosine around the axis.";
      },
      panel: function (body, redraw) {
        buttons(body, "Family", [["trig", "Sines & cosines"], ["bessel", "Bessel"], ["sph", "Spherical harmonics"]], st.fam, function (v) { st.fam = v; redraw(); });
      }
    });
  }

  L.widget("store", store);
  L.widget("families", families);
  L.shapesMath = { target: target, byPoints: byPoints, bySeries: bySeries, worst: worst, besselJ: besselJ };
})();
