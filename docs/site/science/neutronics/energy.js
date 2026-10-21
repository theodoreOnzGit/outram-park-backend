// Neutronics lessons, pages 8 and 9 (gh:#906 design comment, 2026-10-16):
// why the neutron's energy matters, and why nuclear levels are so
// complicated.
//
// - #resonate (page 8): page 4's box ("Only some states fit": standing
//   waves, energy ∝ n²) offered an incoming energy. The response is an
//   ILLUSTRATION, a sum of Lorentzians of one width around the box's levels
//   (the Breit-Wigner shape real resonances also have), not a model of any
//   nucleus.
// - #shells (page 9, supporting): three potentials (box, oscillator, a
//   Woods-Saxon shape) and two level ladders, the oscillator's and the
//   shell model's with spin-orbit coupling. The magic numbers on the
//   ladders are COMPUTED here from each orbital's 2j + 1 (shellModel), not
//   typed in; check-physics.js checks them. Level heights are schematic.
// - #xs (page 9, main): sigma_total(E) of He-4, Mn-55 and U-238 from
//   xs-data.js (our RECONR port, provenance in its header), log-log, on the
//   same axes.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame, buttons = L.buttons, toggle = L.toggle;

  // ─── The maths, exposed for check-physics.js ──────────────────────────
  var LEVELS = [1, 2, 3, 4, 5, 6];
  var GAMMA = 0.6; // width of every toy level, in units of the ground-state energy
  // Response of the box to an offered energy E (units of the ground
  // state's): the largest Lorentzian (Γ/2)² / ((E − n²)² + (Γ/2)²), so 1 on
  // a level and 1/2 at Γ/2 from it.
  function response(E) {
    var best = 0, n = 1;
    LEVELS.forEach(function (k) {
      var d = E - k * k, r = (GAMMA * GAMMA / 4) / (d * d + GAMMA * GAMMA / 4);
      if (r > best) { best = r; n = k; }
    });
    return { r: best, n: n };
  }
  // Harmonic oscillator shells: shell N holds (N + 1)(N + 2) nucleons of one
  // kind (spin included); the closures are the running sums.
  function oscillatorClosures(nShells) {
    var out = [], sum = 0;
    for (var N = 0; N < nShells; N++) { sum += (N + 1) * (N + 2); out.push(sum); }
    return out;
  }
  // The shell model's orbitals with spin-orbit coupling, in the textbook
  // order (Mayer 1949; Haxel, Jensen & Suess 1949), grouped into the shells
  // between the big gaps. Each orbital nl_j holds 2j + 1.
  var SHELLS = [
    ["1s1/2"],
    ["1p3/2", "1p1/2"],
    ["1d5/2", "2s1/2", "1d3/2"],
    ["1f7/2"],
    ["2p3/2", "1f5/2", "2p1/2", "1g9/2"],
    ["1g7/2", "2d5/2", "2d3/2", "3s1/2", "1h11/2"],
    ["1h9/2", "2f7/2", "2f5/2", "3p3/2", "3p1/2", "1i13/2"]
  ];
  function capacity(orb) { var j = orb.split("/")[0].replace(/^\d+[a-z]/, ""); return +j + 1; } // 2j + 1 with j = (2j)/2
  function shellModel() {
    var out = [], sum = 0;
    SHELLS.forEach(function (sh) { sh.forEach(function (o) { sum += capacity(o); }); out.push(sum); });
    return out;
  }
  L.energyMath = { response: response, GAMMA: GAMMA, oscillatorClosures: oscillatorClosures, shellModel: shellModel, capacity: capacity, SHELLS: SHELLS };

  // ─── Page 8: offer the box an energy ──────────────────────────────────
  function resonate(root) {
    var st = { E: 2.0, t: 0, amp: 0, anim: true };
    var frame = new Frame(root, {
      reset: function () { st.E = 2.0; st.amp = 0; },
      tick: function (dt) {
        st.t += dt;
        var target = response(st.E).r;
        st.amp += (target - st.amp) * Math.min(1, dt * 2.5);
        return st.anim;
      },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var res = response(st.E), Emax = 40;
        // Left: the ladder, with the offered energy as a marker.
        var lw = Math.min(150, w * 0.34), pad = 14, top = 64, bot = h - 92;
        var yE = function (E) { return bot - (bot - top) * Math.min(E, Emax) / Emax * zoom; };
        ctx.font = "13px system-ui, sans-serif";
        LEVELS.forEach(function (n) {
          var y = yE(n * n); if (y < top - 10) return;
          var on = res.n === n && res.r > 0.5;
          ctx.strokeStyle = on ? hot : mut; ctx.lineWidth = on ? 3 : 1.5;
          ctx.beginPath(); ctx.moveTo(pad, y); ctx.lineTo(lw, y); ctx.stroke();
          ctx.fillStyle = on ? hot : fg; ctx.fillText("n = " + n, pad, y - 4);
        });
        var ym = yE(st.E);
        ctx.strokeStyle = acc; ctx.lineWidth = 2; ctx.setLineDash([5, 4]);
        ctx.beginPath(); ctx.moveTo(pad - 6, ym); ctx.lineTo(lw + 8, ym); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = acc; ctx.beginPath(); ctx.moveTo(lw + 8, ym); ctx.lineTo(lw + 18, ym - 6); ctx.lineTo(lw + 18, ym + 6); ctx.fill();
        ctx.fillStyle = mut; ctx.fillText("energy ↑ (levels ∝ n²)", pad, bot + 18);
        // Right: the box and its wave, growing with the response.
        var bx = lw + 30, bw = w - bx - pad, my = top + (bot - top) * 0.45, amp = Math.min((bot - top) * 0.32, 110);
        ctx.strokeStyle = fg; ctx.lineWidth = 3;
        ctx.beginPath(); ctx.moveTo(bx, my - amp - 10); ctx.lineTo(bx, my + amp + 10);
        ctx.moveTo(bx + bw, my - amp - 10); ctx.lineTo(bx + bw, my + amp + 10); ctx.stroke();
        var c = Math.cos(2 * Math.PI * 0.4 * Math.sqrt(st.E) * st.t);
        ctx.strokeStyle = res.r > 0.5 ? hot : acc; ctx.lineWidth = 2.5; ctx.beginPath();
        for (var i = 0; i <= 200; i++) {
          var x = i / 200, y = my - amp * st.amp * c * Math.sin(res.n * Math.PI * x);
          if (i) ctx.lineTo(bx + x * bw, y); else ctx.moveTo(bx + x * bw, y);
        }
        ctx.stroke();
        // The response meter, and the response curve over all energies.
        var mx = bx, mw = bw, mtop = bot - 2;
        ctx.fillStyle = mut; ctx.fillText("response", mx, mtop - 6);
        ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(mx + 70, mtop - 18, mw - 70, 14);
        ctx.fillStyle = res.r > 0.5 ? hot : acc; ctx.fillRect(mx + 70, mtop - 18, (mw - 70) * st.amp, 14);
        var cy0 = h - 14, ch = 44;
        ctx.strokeStyle = mut; ctx.beginPath(); ctx.moveTo(pad, cy0); ctx.lineTo(w - pad, cy0); ctx.stroke();
        ctx.strokeStyle = acc; ctx.lineWidth = 1.5; ctx.beginPath();
        for (var k = 0; k <= 400; k++) {
          var E = Emax * k / 400, X = pad + (w - 2 * pad) * k / 400, Y = cy0 - ch * response(E).r;
          if (k) ctx.lineTo(X, Y); else ctx.moveTo(X, Y);
        }
        ctx.stroke();
        var xm = pad + (w - 2 * pad) * Math.min(st.E, Emax) / Emax;
        ctx.strokeStyle = hot; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(xm, cy0 + 4); ctx.lineTo(xm, cy0 - ch - 4); ctx.stroke();
        var near = res.n * res.n;
        return "Offered energy " + st.E.toFixed(2) + " × the ground state's. Nearest allowed level n = " + res.n + " (energy " + near + "): response " +
          Math.round(100 * res.r) + " % of the maximum." + (res.r > 0.5 ? " On a level: the box takes the energy and the wave builds up. A resonance." : " Between levels: little happens.");
      },
      panel: function (body, redraw, play) {
        toggle(body, "Animate", st.anim, function (v) { st.anim = v; if (v) play(); });
        body.append(el("p", { "class": "w-note" }, "An illustration: one particle in a box with hard walls, its levels at n² × the lowest, each given the same width. The curve along the bottom is the box's response at every energy. Not a model of any nucleus."));
      }
    });
    var input = root.parentNode.querySelector("input[data-energy]"), out = root.parentNode.querySelector("[data-energy-out]");
    if (input) {
      var set = function () { st.E = parseFloat(input.value); if (out) out.textContent = st.E.toFixed(2) + " × E₁"; frame.play(); frame.redraw(); };
      input.oninput = set; set();
    }
    return frame;
  }

  // ─── Page 9: potentials and level ladders (supporting) ─────────────────
  function shells(root) {
    var st = { view: "wells" };
    var frame = new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        ctx.font = "13px system-ui, sans-serif";
        var top = 74, bot = h - 26;
        if (st.view === "wells") {
          // Three potentials against distance from the centre; depth down.
          var names = ["box", "oscillator", "Woods–Saxon"], third = (w - 20) / 3;
          for (var p = 0; p < 3; p++) {
            var x0 = 10 + p * third, pw = third - 12, y0 = top + 10, yb = bot - 30;
            ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x0 + pw, y0); ctx.stroke();
            ctx.strokeStyle = acc; ctx.lineWidth = 2.5; ctx.beginPath();
            for (var i = 0; i <= 100; i++) {
              var r = 1.6 * i / 100, v; // r in units of the nuclear radius
              if (p === 0) v = r < 1 ? -1 : 0;
              else if (p === 1) v = Math.min(0, -1 + r * r);
              else v = -1 / (1 + Math.exp((r - 1) / 0.12));
              var X = x0 + pw * i / 100, Y = y0 + (yb - y0) * (-v) * zoom;
              if (i) ctx.lineTo(X, Math.min(Y, yb + 40)); else ctx.moveTo(X, Y);
            }
            ctx.stroke();
            ctx.fillStyle = fg; ctx.fillText(names[p], x0, bot - 6 > yb + 20 ? yb + 22 : bot - 6);
          }
          ctx.fillStyle = mut; ctx.fillText("energy ↓ against distance from the centre →", 10, top - 6);
          return "The potential a nucleon feels, three ways. A box has hard walls; the oscillator's walls are soft but keep rising; a Woods–Saxon shape (Woods & Saxon, 1954) is flat inside, the size of the nucleus, with a soft edge: closest to a real nucleus. Schematic.";
        }
        // Level ladders: oscillator shells, or the shell model with spin-orbit.
        var so = st.view === "so", closures = so ? shellModel() : oscillatorClosures(7), x0 = 14;
        // One row per shell: its orbitals as short stacked lines, their
        // names on one line, and the closure the full shell reaches.
        var rows = [];
        if (!so) for (var N = 0; N < 7; N++) rows.push({ lines: 1, label: "N = " + N + ": holds " + (N + 1) * (N + 2), close: closures[N] });
        else SHELLS.forEach(function (sh, s2) { rows.push({ lines: sh.length, label: sh.join("  "), close: closures[s2] }); });
        var Y = function (k) { return bot - 8 - (bot - top - 20) * k / (rows.length - 1) * zoom; };
        ctx.font = "12px system-ui, sans-serif";
        rows.forEach(function (rw, k) {
          var y = Y(k); if (y < top - 8) return;
          ctx.strokeStyle = fg; ctx.lineWidth = 2;
          for (var j = 0; j < rw.lines; j++) { var yy = y - 3 * (rw.lines - 1) / 2 + 3 * j; ctx.beginPath(); ctx.moveTo(x0, yy); ctx.lineTo(x0 + 34, yy); ctx.stroke(); }
          ctx.fillStyle = fg; ctx.fillText(rw.label, x0 + 42, y + 4);
          ctx.fillStyle = so ? hot : acc; ctx.font = "bold 14px system-ui, sans-serif";
          var c = String(rw.close); ctx.fillText(c, w - 14 - ctx.measureText(c).width, y + 5); ctx.font = "12px system-ui, sans-serif";
        });
        ctx.fillStyle = mut; ctx.fillText("energy ↑ (schematic)", x0, h - 8);
        var hd = so ? "closes at (magic)" : "closes at"; ctx.fillText(hd, w - 14 - ctx.measureText(hd).width, top - 8);
        return so ? "With spin–orbit coupling each level splits by j = l ± ½, holding 2j + 1 nucleons, and the highest-j piece drops into the shell below. The gaps now fall at " + closures.join(", ") + ": the magic numbers (Mayer, 1949; Haxel, Jensen & Suess, 1949)."
          : "Oscillator shells: shell N holds (N + 1)(N + 2) nucleons of one kind, so shells close at " + closures.slice(0, 5).join(", ") + " and so on. The first three are magic; 40 and 70 are not.";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "Choose the view with the buttons under the picture. Potentials and level heights are schematic; the closures are counted from each level's capacity, 2j + 1."));
      }
    });
    var card = root.parentNode;
    card.querySelectorAll("[data-view]").forEach(function (b) {
      b.onclick = function () {
        st.view = b.getAttribute("data-view");
        card.querySelectorAll("[data-view]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
        frame.redraw();
      };
    });
    return frame;
  }

  // ─── Page 9: real total cross sections ─────────────────────────────────
  var NUCS = ["He-4", "Mn-55", "U-238"];
  function xs(root) {
    var D = window.XS_DATA;
    var st = { nuc: "He-4" };
    var frame = new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        var R = D[st.nuc], x0 = 46, y0 = 76, pw = w - x0 - 12, ph = h - y0 - 40;
        var lx0 = -5, lx1 = Math.log10(2e7), ly0 = -2, ly1 = 5;
        var X = function (e) { return x0 + (Math.log10(e) - lx0) / (lx1 - lx0) * pw; };
        var Y = function (s) { return y0 + (ly1 - Math.log10(Math.max(s, 1e-3))) / (ly1 - ly0) * ph / zoom; };
        ctx.font = "12px system-ui, sans-serif"; ctx.fillStyle = mut; ctx.strokeStyle = "rgba(128,128,128,0.25)"; ctx.lineWidth = 1;
        for (var d = -4; d <= 7; d++) { var gx = X(Math.pow(10, d)); ctx.beginPath(); ctx.moveTo(gx, y0); ctx.lineTo(gx, y0 + ph); ctx.stroke(); }
        [["1 meV", -3], ["1 eV", 0], ["1 keV", 3], ["1 MeV", 6]].forEach(function (t) { var gx = X(Math.pow(10, t[1])); ctx.fillText(t[0], gx - ctx.measureText(t[0]).width / 2, y0 + ph + 16); });
        for (var b = ly0; b <= ly1; b++) { var gy = Y(Math.pow(10, b)); if (gy > y0 + ph + 1) continue; ctx.beginPath(); ctx.moveTo(x0, gy); ctx.lineTo(x0 + pw, gy); ctx.stroke(); ctx.fillText("10" + ["⁻²", "⁻¹", "⁰", "¹", "²", "³", "⁴", "⁵"][b + 2], 6, gy + 4); }
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        ctx.strokeStyle = acc; ctx.lineWidth = 1.4; ctx.beginPath();
        for (var i = 0; i < R.e.length; i++) { var xx = X(R.e[i]), yy = Y(R.s[i]); if (i) ctx.lineTo(xx, yy); else ctx.moveTo(xx, yy); }
        ctx.stroke(); ctx.restore();
        ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif";
        ctx.fillText("σ_total [barn] against neutron energy, " + st.nuc, x0, y0 - 8);
        return st.nuc + ", ENDF/B-VIII.0, reconstructed at 0 K by our RECONR port: " + R.n.toLocaleString("en") + " points, " + R.e.length +
          " drawn (the highest and lowest of each pixel step, so no peak is lost). Resonance parameters on the tape: " + R.mf2 + ".";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "Same axes for every nucleus, so the three can be compared. Thinned for a phone: 1e-5 eV to 20 MeV in 900 steps of log E, the highest and the lowest point of each step kept; nothing is smoothed."));
      }
    });
    var card = root.parentNode;
    card.querySelectorAll("[data-nuc]").forEach(function (b) {
      b.onclick = function () {
        st.nuc = b.getAttribute("data-nuc");
        card.querySelectorAll("[data-nuc]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
        frame.redraw();
      };
    });
    return frame;
  }

  L.widget("resonate", resonate);
  L.widget("shells", shells);
  L.widget("xs", xs);
})();
