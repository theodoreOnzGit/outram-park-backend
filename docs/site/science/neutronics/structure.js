// Neutronics lessons, page 25 (gh:#906 design, 2026-10-16): how structure
// shapes the many phonons. Three stages of one widget:
//   molecule: a three-atom molecule's three internal vibrations, beside the
//     H in H2O spectrum from its LEAPR deck (bound-data.js);
//   crystal: the toy density of states of page 23's lattice with one mass or
//     two (a checkerboard of masses m1, m2), from the toy's dynamical matrix;
//   layers: stiff springs along x, weak ones along y, and how a phonon's
//     frequency changes with its direction.
// Toy lattice: each polarisation decouples; for x-polarised motion the bonds
// along x pull with stretch stiffness ax and those along y with slide
// stiffness by (and the other way round for y-polarised). With two masses on
// a checkerboard every neighbour of one kind is of the other, so for each
// polarisation omega^2 = (d1 + d2)/2 +- sqrt(((d1 - d2)/2)^2 + f^2), with
// d_i = 2(a + b)/m_i and f = 2(a cos q_x + b cos q_y)/sqrt(m1 m2).
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, Frame = L.Frame;
  var D = window.BOUND_DATA, LM = L.latticeMath;
  var NB = 40;

  // The squared frequencies of the toy at q, for stiffnesses {xs, xt, ys, yt}
  // (stretch/slide along x and along y bonds) and masses m1, m2.
  function modes(qx, qy, k, m1, m2) {
    var out = [];
    [[k.xs, k.yt], [k.xt, k.ys]].forEach(function (ab) {
      var a = ab[0], b = ab[1], d1 = 2 * (a + b) / m1, d2 = 2 * (a + b) / m2, f = 2 * (a * Math.cos(qx) + b * Math.cos(qy)) / Math.sqrt(m1 * m2);
      var c = (d1 + d2) / 2, r = Math.sqrt((d1 - d2) * (d1 - d2) / 4 + f * f);
      out.push(Math.max(c - r, 0), c + r);
    });
    return out;
  }
  // Density of states: every mode on a q grid, binned by frequency up to wmax.
  function dos(k, m1, m2, wmax, nb) {
    var h = new Array(nb).fill(0), n = 120, tot = 0;
    for (var i = 0; i < n; i++) for (var j = 0; j < n; j++) {
      var qx = -Math.PI + (i + 0.5) * 2 * Math.PI / n, qy = -Math.PI + (j + 0.5) * 2 * Math.PI / n;
      modes(qx, qy, k, m1, m2).forEach(function (w2) { var w = Math.sqrt(w2); h[Math.min(nb - 1, Math.floor(w / wmax * nb))]++; tot++; });
    }
    return h.map(function (x) { return x / tot; });
  }
  // The highest frequency of a one-mass toy phonon of wavevector length q
  // travelling at angle theta: the larger of its two eigenfrequencies.
  function alongDir(theta, q, k) {
    var qx = q * Math.cos(theta), qy = q * Math.sin(theta), sx = Math.sin(qx / 2), sy = Math.sin(qy / 2);
    var wx = 4 * (k.xs * sx * sx + k.yt * sy * sy), wy = 4 * (k.xt * sx * sx + k.ys * sy * sy);
    return Math.sqrt(Math.max(wx, wy));
  }
  var ISO = { xs: LM.KL, xt: LM.KT, ys: LM.KL, yt: LM.KT };
  var LAYER = { xs: LM.KL, xt: LM.KT, ys: 0.12, yt: 0.04 };
  L.structureMath = { modes: modes, dos: dos, alongDir: alongDir, ISO: ISO, LAYER: LAYER };

  function structureWidget(root) {
    var st = { stage: "molecule", two: false, theta: 0, t: 0, mode: 0 };
    var WMAX = Math.sqrt(Math.max.apply(null, modes(Math.PI, Math.PI, ISO, 1, 1).concat(modes(0, 0, ISO, 1, 3)))) * 1.02;
    var cache = {};
    function rho(two) { var key = two ? "2" : "1"; if (!cache[key]) cache[key] = dos(ISO, 1, two ? 3 : 1, WMAX, NB); return cache[key]; }
    var frame = new Frame(root, {
      reset: function () { st.t = 0; },
      tick: function (dt) { st.t += dt; return st.stage === "molecule" || st.stage === "layers"; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        ctx.font = "12px system-ui, sans-serif";
        if (st.stage === "molecule") {
          // The three internal vibrations of a bent three-atom molecule (toy drawing).
          var cx = w / 2, cy = 150, b = 46, ang = 52 * Math.PI / 180, s = Math.sin(st.t * 6) * 0.18;
          var hL = [-Math.sin(ang), Math.cos(ang)], hR = [Math.sin(ang), Math.cos(ang)];
          var dL, dR, dO;
          if (st.mode === 0) { dL = [hL[0] * s, hL[1] * s]; dR = [hR[0] * s, hR[1] * s]; dO = [0, -s * 0.1]; }
          else if (st.mode === 1) { dL = [hL[0] * s, hL[1] * s]; dR = [-hR[0] * s, -hR[1] * s]; dO = [0, 0]; }
          else { dL = [hL[1] * s, -hL[0] * s]; dR = [-hR[1] * s, hR[0] * s]; dO = [0, -s * 0.1]; }
          var O = [cx + dO[0] * b, cy + dO[1] * b], HL = [cx + (hL[0] + dL[0]) * b, cy + (hL[1] + dL[1]) * b], HR = [cx + (hR[0] + dR[0]) * b, cy + (hR[1] + dR[1]) * b];
          ctx.strokeStyle = mut; ctx.lineWidth = 3; ctx.beginPath(); ctx.moveTo(HL[0], HL[1]); ctx.lineTo(O[0], O[1]); ctx.lineTo(HR[0], HR[1]); ctx.stroke();
          ctx.fillStyle = hot; ctx.beginPath(); ctx.arc(O[0], O[1], 16, 0, 2 * Math.PI); ctx.fill();
          ctx.fillStyle = acc; [HL, HR].forEach(function (p) { ctx.beginPath(); ctx.arc(p[0], p[1], 9, 0, 2 * Math.PI); ctx.fill(); });
          ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText(["both bonds stretch together", "one stretches as the other shortens", "the angle opens and closes"][st.mode], 12, 80);
          // The real spectrum: H in H2O, continuous part plus the two oscillators.
          var W = D.decks["H in H2O"], x0 = 40, y0 = 230, pw = w - x0 - 12, ph = h - y0 - 46, emax = 0.45, m = Math.max.apply(null, W.rho);
          var X = function (e) { return x0 + e / emax * pw; };
          ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
          ctx.strokeStyle = hot; ctx.lineWidth = 2; ctx.beginPath();
          W.rho.forEach(function (v, i) { var xx = X(i * W.delta), yy = y0 + ph - v / m * ph * 0.6; if (i) ctx.lineTo(xx, yy); else ctx.moveTo(xx, yy); });
          ctx.stroke();
          W.osc.forEach(function (o) { ctx.lineWidth = 3; ctx.beginPath(); ctx.moveTo(X(o[0]), y0 + ph); ctx.lineTo(X(o[0]), y0 + 6); ctx.stroke(); });
          ctx.fillStyle = mut; ctx.fillText("0", x0, y0 + ph + 16); ctx.fillText("0.2 eV", X(0.2) - 16, y0 + ph + 16); ctx.fillText("0.4 eV", X(0.4) - 16, y0 + ph + 16);
          ctx.fillStyle = fg; ctx.fillText("H in liquid water: its vibration spectrum (LEAPR deck)", x0, y0 - 8);
          return "A non-linear molecule of N atoms has 3N − 6 internal vibrations, so water has 3: two stretches and a bend (drawn as a toy). Each is a ladder of equal steps, so it shows up as a sharp line. In the liquid, each molecule also rotates and drifts, which gives the broad part. The evaluation's model uses two oscillators, at " + W.osc[0][0] + " and " + W.osc[1][0] + " eV, on top of a spectrum from molecular dynamics.";
        }
        var x1 = 40, y1 = 84, pw1 = w - x1 - 12, ph1 = h - y1 - 46;
        ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x1, y1, pw1, ph1);
        if (st.stage === "crystal") {
          var r1 = rho(false), r2 = rho(true), hm = Math.max.apply(null, r1.concat(r2)) * 1.1 / zoom, bw = pw1 / NB, R = st.two ? r2 : r1;
          for (var i2 = 0; i2 < NB; i2++) { var hh = Math.min(R[i2] / hm, 1) * ph1; ctx.fillStyle = st.two ? "rgba(184,92,56,0.7)" : "rgba(31,111,120,0.7)"; ctx.fillRect(x1 + i2 * bw + 1, y1 + ph1 - hh, bw - 2, hh); }
          ctx.fillStyle = mut; ctx.fillText("0", x1, y1 + ph1 + 16); var tw = "highest frequency"; ctx.fillText(tw, x1 + pw1 - ctx.measureText(tw).width, y1 + ph1 + 16);
          ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText("toy density of states, " + (st.two ? "two masses (1 and 3)" : "one mass"), x1, y1 - 10);
          var gapLo = -1, gapHi = -1; for (i2 = 1; i2 < NB - 1; i2++) if (R[i2] === 0 && gapLo < 0 && R[i2 - 1] > 0) gapLo = i2; for (i2 = NB - 2; i2 > 0; i2--) if (R[i2] === 0 && R[i2 + 1] > 0) { gapHi = i2; break; }
          return st.two ? "Two masses: the vibrations split into a low band (neighbours moving together, acoustic) and a high band (neighbours moving against each other, optical). The shape of ρ has changed completely, with the same springs." + (gapLo > 0 && gapHi >= gapLo ? " Between the bands is a gap: no vibration of the crystal has a frequency there." : "")
            : "One kind of atom: the lattice's vibrations spread over one band, up to a highest frequency. Change the structure and the bands change: masses, bond stiffnesses and arrangement all shape ρ.";
        }
        // Layers: frequency against direction, isotropic vs layered.
        var cx2 = w / 2, cy2 = y1 + ph1 / 2, rad = Math.min(pw1, ph1) / 2 - 20, q = 0.6 * Math.PI;
        var wmx = alongDir(0, q, ISO);
        [[ISO, acc, "same springs both ways"], [LAYER, hot, "layered: weak between layers"]].forEach(function (c, k) {
          ctx.strokeStyle = c[1]; ctx.lineWidth = 2; ctx.beginPath();
          for (var s2 = 0; s2 <= 120; s2++) { var th = s2 / 120 * 2 * Math.PI, rr = alongDir(th, q, c[0]) / wmx * rad; var xx = cx2 + rr * Math.cos(th), yy = cy2 - rr * Math.sin(th); if (s2) ctx.lineTo(xx, yy); else ctx.moveTo(xx, yy); }
          ctx.stroke();
          ctx.fillStyle = c[1]; ctx.fillText(c[2], x1 + 6, y1 + 16 + k * 16);
        });
        var wi = alongDir(st.theta, q, ISO), wl = alongDir(st.theta, q, LAYER);
        L.arrow(ctx, cx2, cy2, cx2 + wl / wmx * rad * Math.cos(st.theta), cy2 - wl / wmx * rad * Math.sin(st.theta), fg);
        ctx.fillStyle = mut; ctx.fillText("along the layers →", x1 + pw1 - 120, cy2 - 6);
        return "Distance from the centre: the highest frequency of a phonon of the same wavelength travelling in that direction. With the same springs both ways (teal) it is the same along both axes, but not quite on the diagonals: even this simple square crystal is a little direction-dependent. With weak springs between layers (brown), a phonon along the layers vibrates " + (alongDir(0, q, LAYER) / alongDir(Math.PI / 2, q, LAYER)).toFixed(1) + " times faster than one across them. At " + Math.round(st.theta * 180 / Math.PI) + "°: layered " + (wl / wi).toFixed(2) + " of the even lattice's frequency. A toy: graphite's real stiffnesses are not these.";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "Toys built on page 23's lattice. The molecule's motions are drawings; the crystal and layer stages are computed from the toy's springs and masses. The water spectrum is the H in H2O LEAPR deck behind ENDF/B-VIII.0."));
      }
    });
    var card = root.parentNode;
    function show(sel, on) { card.querySelectorAll(sel).forEach(function (r) { r.style.display = on ? "" : "none"; }); }
    function sync() { show("[data-for=molecule]", st.stage === "molecule"); show("[data-for=crystal]", st.stage === "crystal"); show("[data-for=layers]", st.stage === "layers"); }
    var pick = function (attr, set) { card.querySelectorAll("[" + attr + "]").forEach(function (b) { b.onclick = function () { set(b.getAttribute(attr)); card.querySelectorAll("[" + attr + "]").forEach(function (o) { o.classList.toggle("ghost", o !== b); }); sync(); frame.play(); frame.redraw(); }; }); };
    pick("data-sstage", function (v) { st.stage = v; });
    pick("data-mmode", function (v) { st.mode = +v; });
    pick("data-mass", function (v) { st.two = v === "2"; });
    var th = card.querySelector("input[data-dir]"), out = card.querySelector("[data-dir-out]");
    if (th) { var go = function () { st.theta = parseFloat(th.value) * Math.PI / 180; if (out) out.textContent = th.value + "°"; frame.redraw(); }; th.oninput = go; go(); }
    sync();
    return frame;
  }

  L.widget("structure", structureWidget);
})();
