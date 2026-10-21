// Neutronics lessons, page 24 (gh:#906 design, 2026-10-16): can we have more
// than one phonon? Random kicks build up the density of states.
// Each kick has a random direction; in the averaged (incoherent / polycrystal
// / liquid) picture any phonon of the toy lattice (page 23) can be made, and a
// phonon of wavevector q and style b is made with weight (Q^.e_b)^2: so the
// kick draws q uniformly over the zone and the style with that weight.
// Averaged over kick directions each style gets 1/2 (2D), so the energies made
// build up the toy's density of states rho: every mode counted once, by energy
// (check-physics.js). Real spectra from bound-data.js (the alpha-quartz and
// H in H2O LEAPR decks).
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, Frame = L.Frame;
  var LM = L.latticeMath, D = window.BOUND_DATA;
  var NB = 38, TOP = 152;

  function rng(seed) { return function () { seed |= 0; seed = seed + 0x6D2B79F5 | 0; var t = Math.imul(seed ^ seed >>> 15, 1 | seed); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }
  // One random kick: the phonon it makes (energy in meV, style).
  function kick(r) {
    var th = r() * 2 * Math.PI, qx = (2 * r() - 1) * Math.PI, qy = (2 * r() - 1) * Math.PI, qn = Math.hypot(qx, qy) || 1;
    var c = (Math.cos(th) * qx + Math.sin(th) * qy) / qn, style = r() < c * c ? "L" : "T";
    return { mev: LM.mev(Math.sqrt(LM.omega2(qx, qy, style === "L" ? LM.KL : LM.KT, 1))), style: style };
  }
  // The toy's density of states: every mode on a fine q grid, both styles, counted once.
  function rhoToy(nb) {
    var h = new Array(nb).fill(0), n = 160, tot = 0;
    for (var i = 0; i < n; i++) for (var j = 0; j < n; j++) {
      var qx = -Math.PI + (i + 0.5) * 2 * Math.PI / n, qy = -Math.PI + (j + 0.5) * 2 * Math.PI / n;
      [LM.KL, LM.KT].forEach(function (k) { var w = LM.mev(Math.sqrt(LM.omega2(qx, qy, k, 1))); h[Math.min(nb - 1, Math.floor(w / TOP * nb))]++; tot++; });
    }
    return h.map(function (x) { return x / tot; });
  }
  L.manyMath = { kick: kick, rhoToy: rhoToy, rng: rng, NB: NB };

  function manyWidget(root) {
    var RHO = rhoToy(NB), st = { tab: "toy", h: new Array(NB).fill(0), n: 0, pending: 0, last: null, r: rng(906) };
    var frame = new Frame(root, {
      reset: function () { st.h = new Array(NB).fill(0); st.n = 0; st.pending = 0; st.last = null; st.r = rng(906); },
      tick: function () {
        if (!st.pending) return false;
        var k = Math.min(st.pending, Math.max(1, Math.ceil(st.pending / 20)));
        for (var i = 0; i < k; i++) { var p = kick(st.r); st.h[Math.min(NB - 1, Math.floor(p.mev / TOP * NB))]++; st.n++; st.last = p; }
        st.pending -= k;
        return st.pending > 0;
      },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var x0 = 40, y0 = 84, pw = w - x0 - 12, ph = h - y0 - 46;
        ctx.font = "12px system-ui, sans-serif"; ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        if (st.tab === "toy") {
          var hmax = Math.max.apply(null, RHO) * 1.15 / zoom, bw = pw / NB;
          for (var b = 0; b < NB; b++) {
            var f = st.n ? st.h[b] / st.n : 0, hh = Math.min(f / hmax, 1) * ph;
            ctx.fillStyle = "rgba(31,111,120,0.7)"; ctx.fillRect(x0 + b * bw + 1, y0 + ph - hh, bw - 2, hh);
          }
          if (st.n >= 200) {
            ctx.strokeStyle = hot; ctx.lineWidth = 2; ctx.beginPath();
            for (b = 0; b < NB; b++) { var yy = y0 + ph - Math.min(RHO[b] / hmax, 1) * ph; if (b) ctx.lineTo(x0 + (b + 0.5) * bw, yy); else ctx.moveTo(x0 + (b + 0.5) * bw, yy); }
            ctx.stroke();
          }
          ctx.fillStyle = mut; ctx.fillText("0", x0, y0 + ph + 16); var t1 = TOP + " meV"; ctx.fillText(t1, x0 + pw - ctx.measureText(t1).width, y0 + ph + 16);
          ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText("phonons made, by energy" + (st.n >= 200 ? "; brown: ρ" : ""), x0, y0 - 10);
          var s = st.n + " kick" + (st.n === 1 ? "" : "s") + ", each in a random direction, each making one phonon of the toy lattice (page 23).";
          if (st.last) s += " The last made a " + (st.last.style === "L" ? "longitudinal" : "transverse") + " phonon of " + st.last.mev.toFixed(0) + " meV.";
          if (st.n >= 200) s += " Brown: the toy's density of states, every mode counted once by its energy. The more kicks, the closer the bars come to it.";
          return s;
        }
        // Real spectra: alpha-quartz (a crystal) and H in water (a liquid, with two sharp molecular lines).
        var Q = D.decks["Si in alpha-quartz"], W = D.decks["H in H2O"], emax = 0.45;
        var X = function (e) { return x0 + e / emax * pw; };
        var curve = function (d, col, scale) {
          ctx.strokeStyle = col; ctx.lineWidth = 2; ctx.beginPath();
          var m = Math.max.apply(null, d.rho);
          d.rho.forEach(function (v, i) { var xx = X(i * d.delta), yy = y0 + ph - v / m * ph * scale; if (i) ctx.lineTo(xx, yy); else ctx.moveTo(xx, yy); });
          ctx.stroke();
        };
        curve(Q, acc, 0.9); curve(W, hot, 0.6);
        W.osc.forEach(function (o) { var xx = X(o[0]); ctx.strokeStyle = hot; ctx.lineWidth = 3; ctx.beginPath(); ctx.moveTo(xx, y0 + ph); ctx.lineTo(xx, y0 + ph - ph * 0.95); ctx.stroke(); });
        ctx.fillStyle = mut; ctx.fillText("0", x0, y0 + ph + 16); ctx.fillText("0.2 eV", X(0.2) - 16, y0 + ph + 16); ctx.fillText("0.4 eV", X(0.4) - 16, y0 + ph + 16);
        ctx.font = "13px system-ui, sans-serif"; ctx.fillStyle = acc; ctx.fillText("Si in α-quartz", x0 + 6, y0 - 10); ctx.fillStyle = hot; ctx.fillText("H in water", x0 + 120, y0 - 10);
        return "Real spectra, from the LEAPR input decks behind the ENDF/B-VIII.0 evaluations: Si in α-quartz, a crystal (teal, its bands up to " + ((Q.rho.length - 1) * Q.delta).toFixed(3) + " eV), and H in water, a liquid (brown: a broad part from the molecules' rotation and drift, plus two sharp lines, the molecule's own vibrations at " + W.osc[0][0] + " and " + W.osc[1][0] + " eV). Each curve scaled to its own height.";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "Toy kicks: each picks a random direction; the phonon it makes has a random wavevector, and is longitudinal with chance (Q̂·e)², the share of the push along the atoms' motion. Averaged over directions that is ½ for each style, so every mode ends up counted equally."));
      }
    });
    var card = root.parentNode;
    card.querySelectorAll("[data-kicks]").forEach(function (b) { b.onclick = function () { st.pending += +b.getAttribute("data-kicks"); frame.play(); }; });
    card.querySelectorAll("[data-tab]").forEach(function (b) {
      b.onclick = function () { st.tab = b.getAttribute("data-tab"); card.querySelectorAll("[data-tab]").forEach(function (o) { o.classList.toggle("ghost", o !== b); }); frame.redraw(); };
    });
    return frame;
  }

  L.widget("many", manyWidget);
})();
