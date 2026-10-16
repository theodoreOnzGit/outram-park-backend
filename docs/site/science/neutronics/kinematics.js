// Neutronics lessons: collision kinematics and thermal motion (gh:#906
// checkpoint §11–§12).
// Classical kinematics only: the billiard collision is intuition for how
// energy and momentum are shared, never a model of nuclear scattering.
(function () {
  "use strict";
  var L = window.Lesson;
  var H = L.H, HBAR = L.HBAR, EV = L.EV, MN = L.MN, C = L.C;
  var css = L.css, el = L.el, fmtE = L.fmtE, pct = L.pct, fmtLen = L.fmtLen, fmtSpeed = L.fmtSpeed;
  var Frame = L.Frame, buttons = L.buttons, slider = L.slider, toggle = L.toggle, arrow = L.arrow;

  // ─── Collision: two-body elastic kinematics, target at rest ────────────
  // Neutron mass 1, nucleus mass A (its AWR). With n the unit contact normal
  // (neutron centre to nucleus centre): v' = v − 2A/(1+A) (v·n) n,
  // V' = 2/(1+A) (v·n) n. Head-on, E'/E = ((A−1)/(A+1))².
  function collision(root) {
    var awr = { h1: +root.dataset.awrH1, c12: +root.dataset.awrC12, u238: +root.dataset.awrU238 };
    var name = { h1: "H-1", c12: "C-12", u238: "U-238" };
    var st = { nuc: "h1", e: 1e6, aim: 0, t: -1.6 };
    var RN = 0.3, SPEED = 3, T0 = -1.6, T1 = 1.8;
    function kin() {
      var A = awr[st.nuc], rN = 0.3 * Math.cbrt(A), D = RN + rN;
      var b = st.aim * D * 0.98;
      var nx = Math.sqrt(1 - (b / D) * (b / D)), ny = -b / D;
      var vx = SPEED, vy = 0, vn = vx * nx + vy * ny;
      var f = 2 * A / (1 + A), g = 2 / (1 + A);
      return {
        A: A, rN: rN, D: D, b: b, nx: nx, ny: ny,
        v1: [vx - f * vn * nx, vy - f * vn * ny], V1: [g * vn * nx, g * vn * ny],
        kept: ((vx - f * vn * nx) * (vx - f * vn * nx) + (vy - f * vn * ny) * (vy - f * vn * ny)) / (SPEED * SPEED)
      };
    }
    return new Frame(root, {
      reset: function () { st.t = T0; },
      onSeen: function () { st.t = T0; },
      tick: function (dt) { st.t += dt * 0.9; if (st.t >= T1) { st.t = T1; return false; } return true; },
      draw: function (ctx, w, h, zoom) {
        var k = kin(), s = Math.min(w / 12, h / 7) * zoom, cx = w / 2, cy = h / 2;
        var X = function (x) { return cx + x * s; }, Y = function (y) { return cy - y * s; };
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        var c0x = -k.D * k.nx, c0y = -k.D * k.ny, t = st.t;
        var n = t < 0 ? [c0x + SPEED * t, c0y] : [c0x + k.v1[0] * t, c0y + k.v1[1] * t];
        var N = t < 0 ? [0, 0] : [k.V1[0] * t, k.V1[1] * t];
        // The incoming line, faint.
        ctx.strokeStyle = mut; ctx.setLineDash([4, 6]); ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(X(-7), Y(c0y)); ctx.lineTo(X(c0x), Y(c0y)); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = "#b85c38";
        ctx.beginPath(); ctx.arc(X(N[0]), Y(N[1]), k.rN * s, 0, 2 * Math.PI); ctx.fill();
        ctx.fillStyle = acc;
        ctx.beginPath(); ctx.arc(X(n[0]), Y(n[1]), RN * s, 0, 2 * Math.PI); ctx.fill();
        var as = 0.45;
        if (t < 0) arrow(ctx, X(n[0]), Y(n[1]), X(n[0] + SPEED * as), Y(n[1]), fg);
        else {
          arrow(ctx, X(n[0]), Y(n[1]), X(n[0] + k.v1[0] * as), Y(n[1] + k.v1[1] * as), fg);
          arrow(ctx, X(N[0]), Y(N[1]), X(N[0] + k.V1[0] * as), Y(N[1] + k.V1[1] * as), "#b85c38");
        }
        ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif";
        ctx.fillText("neutron", X(n[0]) - 20, Y(n[1]) - RN * s - 6);
        ctx.fillText(name[st.nuc] + " nucleus (A = " + k.A + ")", X(N[0]) - 40, Y(N[1]) + k.rN * s + 16);
        var eAfter = st.e * k.kept, theta = Math.atan2(k.v1[1], k.v1[0]) * 180 / Math.PI;
        var alpha = Math.pow((k.A - 1) / (k.A + 1), 2);
        return "Before: " + fmtE(st.e) + ". After: neutron " + fmtE(eAfter) + " (" + pct(k.kept) + " kept), nucleus " +
          fmtE(st.e - eAfter) + ", " + (k.kept < 1e-6 ? "neutron all but stopped" : "neutron turned " + Math.abs(theta).toFixed(1) + "°") +
          ". Head-on, " + name[st.nuc] + " leaves the neutron at least " + pct(alpha) + ".";
      },
      panel: function (body, redraw, play) {
        buttons(body, "Target nucleus", [["h1", "H-1"], ["c12", "C-12"], ["u238", "U-238"]], st.nuc, function (v) { st.nuc = v; st.t = T0; play(); });
        buttons(body, "Neutron energy", [[0.0253, "0.0253 eV"], [1, "1 eV"], [1e3, "1 keV"], [1e6, "1 MeV"], [2e6, "2 MeV"]], st.e,
          function (v) { st.e = v; redraw(); });
        slider(body, "Aim", 0, 1, 0.01, st.aim, function (v) {
          st.aim = v; st.t = T0; redraw();
          return v < 0.02 ? "head-on" : v > 0.9 ? "glancing" : (100 * v).toFixed(0) + " % off-centre";
        });
        var b = el("button", { type: "button", "class": "w-opt wide" }, "▶ Collide");
        b.onclick = function () { st.t = T0; play(); };
        body.append(b);
        body.append(el("p", { "class": "w-note" }, "The energy only scales the numbers: the fractions depend on the mass ratio and the aim, not on the energy. At 0.0253 eV a real nucleus is not at rest; this picture does not hold there."));
      }
    });
  }
  // ─── Thermal motion and relative velocity (checkpoint §12) ─────────────
  // A free gas: each component of a nucleus's velocity is Gaussian with
  // σ = sqrt(kT/M), M = A m_n (A its AWR). The neutron moves along +x at
  // the speed of its energy. An encounter's energy is the energy the neutron
  // would have if the nucleus stood still, E_rel = ½ m_n |v − V|², the
  // convention of Doppler broadening; its mean is exactly E + (3/2) kT / A.
  // The histogram is over nuclei, not weighted by how often each pair meets
  // (a reaction rate weights each by |v − V|); the note on the page says so.
  // The same random numbers are scaled for every temperature, so the
  // slider moves the picture smoothly rather than reshuffling it.
  var KB = 8.617333262e-5; // eV/K, CODATA 2018 (exact)
  function rng(seed) {
    return function () {
      seed = (seed + 0x6D2B79F5) | 0;
      var t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  function normals(n, seed) {
    var r = rng(seed), out = new Float64Array(n);
    for (var i = 0; i < n; i += 2) {
      var u = 1 - r(), v = r(), m = Math.sqrt(-2 * Math.log(u));
      out[i] = m * Math.cos(2 * Math.PI * v);
      if (i + 1 < n) out[i + 1] = m * Math.sin(2 * Math.PI * v);
    }
    return out;
  }
  function thermal(root) {
    var awr = { h1: +root.dataset.awrH1, c12: +root.dataset.awrC12, u238: +root.dataset.awrU238 };
    var name = { h1: "H-1", c12: "C-12", u238: "U-238" };
    var st = { nuc: "c12", T: 300, e: 0.0253, pick: 0, clock: 0, anim: true };
    var NF = 30, NH = 20000;
    var zf = normals(3 * NF, 11), zh = normals(3 * NH, 12), pos = [];
    var r0 = rng(13);
    for (var i = 0; i < NF; i++) pos.push([r0(), r0()]);
    function phys() {
      var A = awr[st.nuc], M = A * MN;
      var sig = Math.sqrt(KB * st.T * EV / M);       // m/s, one component
      var vn = Math.sqrt(2 * st.e * EV / MN);        // m/s
      return { A: A, sig: sig, vn: vn };
    }
    function erel(P, z, i) {
      var dx = P.vn - P.sig * z[3 * i], dy = -P.sig * z[3 * i + 1], dz = -P.sig * z[3 * i + 2];
      return 0.5 * MN * (dx * dx + dy * dy + dz * dz) / EV;
    }
    return new Frame(root, {
      reset: function () { st.pick = 0; st.clock = 0; },
      tick: function (dt) {
        if (!st.anim) return false;
        st.clock += dt;
        if (st.clock > 2.5) { st.clock = 0; st.pick = (st.pick + 1) % NF; }
        var P = phys(), vmax = Math.max(P.vn, 3 * P.sig, 1e-9);
        for (var i = 0; i < NF; i++) {
          // Drift in proportion to the drawn arrows: a nucleus's arrow
          // length per 4 s, so a fast nucleus visibly outruns a slow one.
          pos[i][0] = (pos[i][0] + 0.06 * dt * P.sig * zf[3 * i] / vmax + 1) % 1;
          pos[i][1] = (pos[i][1] + 0.06 * dt * P.sig * zf[3 * i + 1] / vmax + 1) % 1;
        }
        return true;
      },
      draw: function (ctx, w, h, zoom) {
        var P = phys(), fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        // The frame's buttons sit across the top 52 px; draw below them.
        var nucCol = "#b85c38", y0 = 56, topH = h * 0.58, half = w * 0.45;
        var vmax = Math.max(P.vn, 3 * P.sig, 1e-9);
        ctx.font = "13px system-ui, sans-serif";
        // Left: the nuclei, each with its velocity (in the plane of the page).
        var fieldW = half - 12, fieldH = topH - y0 - 4, sc = 0.22 * Math.min(fieldW, fieldH) / vmax * zoom;
        ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(6, y0, fieldW, fieldH);
        ctx.fillStyle = mut; ctx.fillText(name[st.nuc] + ", " + st.T + " K", 10, y0 + 14);
        for (var i = 0; i < NF; i++) {
          var x = 6 + pos[i][0] * fieldW, y = y0 + pos[i][1] * fieldH, on = i === st.pick;
          ctx.fillStyle = on ? nucCol : "rgba(184, 92, 56, 0.45)";
          ctx.beginPath(); ctx.arc(x, y, on ? 6 : 4, 0, 2 * Math.PI); ctx.fill();
          arrow(ctx, x, y, x + sc * P.sig * zf[3 * i], y - sc * P.sig * zf[3 * i + 1], on ? nucCol : "rgba(184, 92, 56, 0.6)");
          if (on) { ctx.strokeStyle = fg; ctx.lineWidth = 1.5; ctx.beginPath(); ctx.arc(x, y, 10, 0, 2 * Math.PI); ctx.stroke(); }
        }
        // Right: the velocity triangle for the ringed nucleus.
        var k = st.pick, Vx = P.sig * zf[3 * k], Vy = P.sig * zf[3 * k + 1];
        var dsc = 0.8 * Math.min(w - half - 40, (topH - y0) / 2) / vmax * zoom;
        var ox = half + 16, oy = y0 + (topH - y0) / 2 + 10;
        ctx.fillStyle = mut; ctx.fillText("the ringed pair", half + 12, y0 + 14);
        arrow(ctx, ox, oy, ox + dsc * P.vn, oy, acc);
        arrow(ctx, ox, oy, ox + dsc * Vx, oy - dsc * Vy, nucCol);
        ctx.setLineDash([5, 4]);
        arrow(ctx, ox + dsc * Vx, oy - dsc * Vy, ox + dsc * P.vn, oy, fg);
        ctx.setLineDash([]);
        ctx.fillStyle = acc; ctx.fillText("neutron v", ox + dsc * P.vn - 60, oy + 18);
        ctx.fillStyle = nucCol; ctx.fillText("nucleus V", ox + dsc * Vx + 4, oy - dsc * Vy - 6);
        ctx.fillStyle = fg; ctx.fillText("v − V (relative)", ox + 4, oy - 30);
        // Bottom: the encounter energies over many nuclei.
        var top = topH + 18, hh = h - top - 22, bins = 50, cnt = new Array(bins).fill(0);
        var D = Math.sqrt(4 * st.e * KB * st.T / P.A);
        var lo = Math.max(0, st.e - 4 * D), hi = st.e + 4 * D + (D === 0 ? 0.1 * st.e : 0);
        if (D === 0) lo = 0.9 * st.e;
        var sum = 0, sum2 = 0;
        for (var j = 0; j < NH; j++) {
          var E = erel(P, zh, j); sum += E; sum2 += E * E;
          var b = Math.floor((E - lo) / (hi - lo) * bins);
          if (b >= 0 && b < bins) cnt[b]++;
        }
        var mx = Math.max.apply(null, cnt), bw = w / bins;
        ctx.fillStyle = "rgba(31, 111, 120, 0.45)";
        for (var b2 = 0; b2 < bins; b2++) {
          var bh = hh * cnt[b2] / mx;
          ctx.fillRect(b2 * bw + 1, top + hh - bh, bw - 2, bh);
        }
        var xe = (st.e - lo) / (hi - lo) * w;
        ctx.strokeStyle = fg; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(xe, top); ctx.lineTo(xe, top + hh); ctx.stroke();
        ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(0, top + hh); ctx.lineTo(w, top + hh); ctx.stroke();
        ctx.fillStyle = fg;
        ctx.fillText(lo > 0 ? fmtE(lo) : "0", 6, h - 6);
        var hiTxt = fmtE(hi);
        ctx.fillText(hiTxt, w - 6 - ctx.measureText(hiTxt).width, h - 6);
        var axis = "encounter energy ½mₙ|v − V|²";
        ctx.fillText(axis, (w - ctx.measureText(axis).width) / 2, h - 6);
        ctx.fillText("neutron's own energy", Math.min(xe + 6, w - 140), top + 12);
        var mean = sum / NH, sd = Math.sqrt(Math.max(0, sum2 / NH - mean * mean));
        return "Neutron " + fmtE(st.e) + ", " + fmtSpeed(P.vn) + ". " + name[st.nuc] + " at " + st.T + " K: each velocity component spread ±" +
          fmtSpeed(P.sig) + " (√(kT/M)). Encounter energies over " + NH.toLocaleString() + " nuclei: mean " + fmtE(mean) +
          " (exact: E + 3kT/2A = " + fmtE(st.e + 1.5 * KB * st.T / P.A) + "), spread ±" + fmtE(sd) + ".";
      },
      panel: function (body, redraw, play) {
        buttons(body, "Target nucleus", [["h1", "H-1"], ["c12", "C-12"], ["u238", "U-238"]], st.nuc, function (v) { st.nuc = v; redraw(); });
        slider(body, "Temperature", 0, 1200, 10, st.T, function (v) { st.T = v; redraw(); return v + " K"; });
        buttons(body, "Neutron energy", [[0.0253, "0.0253 eV"], [1, "1 eV"], [6.67, "6.67 eV"], [1e3, "1 keV"]], st.e,
          function (v) { st.e = v; redraw(); });
        var b = el("button", { type: "button", "class": "w-opt wide" }, "Pick another nucleus");
        b.onclick = function () { st.pick = (st.pick + 1) % NF; st.clock = 0; redraw(); };
        body.append(b);
        toggle(body, "Animate", st.anim, function (v) { st.anim = v; if (v) play(); });
        body.append(el("p", { "class": "w-note" }, "Arrows share one scale, set by the faster of the neutron and the nuclei; zoom to enlarge them. The arrows show two of the three velocity components; the encounter energies use all three. At 0 K every nucleus is still and every encounter has the neutron's own energy."));
      }
    });
  }

  L.widget("collision", collision);
  L.widget("thermal", thermal);
})();
