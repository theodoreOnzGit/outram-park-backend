// Neutronics lessons, page 21 (gh:#906 design, 2026-10-16): what lattice
// scattering depends on; graphite's coherent elastic (Bragg) cross section.
// Data: bound-data.js (examples/lesson_bound.rs): the ENDF/B-VIII.0
// crystalline-graphite Bragg edges E_i and cumulative S(E_i, T) through
// OUTRAM PARK's THERMR port (thermr::coherent::s_of_e_at), so that
// sigma_coh(E) = S(E_i, T)/E for the last edge E_i <= E (ENDF-102 7.2; NJOY2016
// manual 7.1), zero below the first edge with S > 0. A Bragg edge E_i is where
// the neutron's wavelength equals twice a plane spacing, lambda = 2d.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame;
  var D = window.BOUND_DATA;

  function sigmaCoh(B, E) {
    var s = 0;
    for (var i = 0; i < B.e.length && B.e[i] <= E; i++) s = B.s[i];
    return s / E;
  }
  // The first Bragg edge whose cumulative S is positive: below it, nothing diffracts.
  function cutoff(B) { for (var i = 0; i < B.e.length; i++) if (B.s[i] > 0) return B.e[i]; return NaN; }
  // de Broglie wavelength [Angstrom] of a neutron of energy E [eV], and back.
  function lambdaA(E) { return L.H / Math.sqrt(2 * L.MN * E * L.EV) * 1e10; }
  function energyOf(lam) { var p = L.H / (lam * 1e-10); return p * p / (2 * L.MN) / L.EV; }
  L.braggMath = { sigmaCoh: sigmaCoh, cutoff: cutoff, lambdaA: lambdaA, energyOf: energyOf };

  function braggWidget(root) {
    var st = { t: "296", lam: 1.8 };
    var frame = new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var B = D.bragg[st.t], ec = cutoff(B), lc = lambdaA(ec), E = energyOf(st.lam);
        var x0 = 46, y0 = 84, pw = w - x0 - 12, ph = h - y0 - 46, lx0 = -4, lx1 = Math.log10(5), ly0 = -2, ly1 = 1.5;
        if (zoom > 1) { var c = Math.log10(E), half = (lx1 - lx0) / 2 / zoom; lx0 = c - half; lx1 = c + half; }
        var X = function (e) { return x0 + (Math.log10(e) - lx0) / (lx1 - lx0) * pw; };
        var Y = function (s) { return y0 + (ly1 - Math.log10(Math.max(s, 1e-9))) / (ly1 - ly0) * ph; };
        ctx.font = "12px system-ui, sans-serif"; ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        // Below the cutoff: the wavelength is too long to diffract.
        ctx.fillStyle = "rgba(128,128,128,0.12)"; ctx.fillRect(x0, y0, X(ec) - x0, ph);
        // The staircase: S/E falls as 1/E between edges and jumps at each.
        ctx.strokeStyle = acc; ctx.lineWidth = 1.6; ctx.beginPath();
        var first = true, N = 900;
        for (var k = 0; k <= N; k++) {
          var e = Math.pow(10, lx0 + (lx1 - lx0) * k / N), s = sigmaCoh(B, e);
          if (!(s > 0)) { first = true; continue; }
          if (first) { ctx.moveTo(X(e), Y(s)); first = false; } else ctx.lineTo(X(e), Y(s));
        }
        ctx.stroke();
        ctx.strokeStyle = hot; ctx.setLineDash([4, 3]); ctx.beginPath(); ctx.moveTo(X(E), y0); ctx.lineTo(X(E), y0 + ph); ctx.stroke(); ctx.setLineDash([]);
        ctx.restore();
        ctx.fillStyle = mut;
        [["0.1 meV", -4], ["1 meV", -3], ["10 meV", -2], ["0.1 eV", -1], ["1 eV", 0]].forEach(function (t) { var gx = X(Math.pow(10, t[1])); if (gx < x0 - 1 || gx > x0 + pw + 1) return; ctx.fillText(t[0], gx - ctx.measureText(t[0]).width / 2, y0 + ph + 16); });
        [[0.01, "0.01 b"], [0.1, "0.1 b"], [1, "1 b"], [10, "10 b"]].forEach(function (t) { ctx.fillText(t[1], 2, Y(t[0]) + 4); });
        ctx.fillStyle = fg; ctx.fillText("coherent elastic σ, " + st.t + " K; dashed: neutron", x0, y0 - 10);
        var s = sigmaCoh(B, E), dmax = lc / 2;
        return "λ = " + st.lam.toFixed(2) + " Å, so E = " + fmtE(E) + ". " + (st.lam > lc ? "That is longer than " + lc.toFixed(2) + " Å, twice the widest spacing of graphite planes that diffract (" + dmax.toFixed(2) + " Å): no set of planes can diffract it, and the coherent elastic cross section is zero."
          : "Shorter than " + lc.toFixed(2) + " Å, so some sets of planes diffract it: σ_coh = " + s.toPrecision(3) + " b. Each jump in the curve is one more set of planes joining in.") +
          " Data: ENDF/B-VIII.0 crystalline graphite, " + B.e.length + " Bragg edges, through OUTRAM PARK's THERMR port.";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "σ_coh(E) = S(E_i, T)/E, with S the evaluation's cumulative structure factor at the last Bragg edge E_i below E (ENDF MF=7/MT=2). The wavelength is the neutron's de Broglie wavelength (page 4)."));
      }
    });
    var card = root.parentNode;
    card.querySelectorAll("[data-t]").forEach(function (b) {
      b.onclick = function () { st.t = b.getAttribute("data-t"); card.querySelectorAll("[data-t]").forEach(function (o) { o.classList.toggle("ghost", o !== b); }); frame.redraw(); };
    });
    var input = card.querySelector("input[data-lam]"), out = card.querySelector("[data-lam-out]");
    if (input) { var go = function () { st.lam = parseFloat(input.value); if (out) out.textContent = st.lam.toFixed(2) + " Å"; frame.redraw(); }; input.oninput = go; go(); }
    return frame;
  }

  L.widget("bragg", braggWidget);
})();
