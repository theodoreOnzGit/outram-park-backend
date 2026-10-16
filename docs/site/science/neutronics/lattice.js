// Neutronics lessons, page 23 (gh:#906 designs and amendments, 2026-10-16):
// what is a phonon? Two toy widgets on a square 2D lattice of masses joined
// by springs (no damping: the ideal, harmonic modes do not decay).
//
// #lattice (stages 1-3): a numerical spring-mass lattice. Each bond has a
//   stiffness kL for stretching along it and kT for sliding across it
//   (a toy that gives the lattice both longitudinal and transverse motion),
//   fixed walls around the edge, velocity-Verlet steps. Push an atom and the
//   wave spreads; start a normal mode; count phonons in one mode.
// #phonon (stages 4-5): one travelling phonon, drawn from the toy dispersion
//   omega_b(q)^2 = (4 k_b / m)(sin^2(q_x a/2) + sin^2(q_y a/2)), b = L or T,
//   with its wavevector q and its polarisation e drawn as separate arrows; and
//   a neutron kick Q with an energy loss: a phonon is made only if
//   q = Q - G (G the nearest reciprocal-lattice vector) and the energy loss
//   equals hbar omega_b(q), with strength (Q^.e)^2 (Squires 2012, ch. 3).
// Energies are scaled so the toy's highest phonon is 152 meV, the top of the
// alpha-quartz spectrum in its LEAPR deck (page 20); everything else is toy.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, Frame = L.Frame;
  var KL = 1, KT = 0.35, M = 1, TOP_MEV = 152;

  function omega2(qx, qy, k, m) { var sx = Math.sin(qx / 2), sy = Math.sin(qy / 2); return 4 * k / m * (sx * sx + sy * sy); }
  var OMAX = Math.sqrt(omega2(Math.PI, Math.PI, KL, M));
  function mev(w) { return w / OMAX * TOP_MEV; }
  // Fold a kick into the first zone: q = Q - G, G = 2 pi (n_x, n_y).
  function fold(Qx, Qy) {
    var gx = 2 * Math.PI * Math.round(Qx / (2 * Math.PI)), gy = 2 * Math.PI * Math.round(Qy / (2 * Math.PI));
    return { qx: Qx - gx, qy: Qy - gy, gx: gx, gy: gy };
  }
  // The phonon a kick (Q, energy loss dE meV) makes, if any: per branch, the
  // mismatch in energy and the strength (Q^.e)^2.
  function match(Qx, Qy, dE) {
    var f = fold(Qx, Qy), qn = Math.hypot(f.qx, f.qy), Qn = Math.hypot(Qx, Qy);
    var eL = qn > 1e-9 ? [f.qx / qn, f.qy / qn] : [1, 0], eT = [-eL[1], eL[0]];
    var out = {};
    [["L", KL, eL], ["T", KT, eT]].forEach(function (b) {
      var w = mev(Math.sqrt(omega2(f.qx, f.qy, b[1], M))), c = Qn > 1e-9 ? (Qx * b[2][0] + Qy * b[2][1]) / Qn : 0;
      out[b[0]] = { e: b[2], mev: w, strength: c * c, hit: Math.abs(dE - w) < 0.04 * TOP_MEV };
    });
    out.fold = f;
    return out;
  }
  L.latticeMath = { omega2: omega2, mev: mev, fold: fold, match: match, KL: KL, KT: KT, OMAX: OMAX };

  // ─── #lattice: push, modes, phonons ───────────────────────────────────
  function latticeWidget(root) {
    var N = 9, st = { stage: "push", two: false, mode: "long", n: 0, has: false, t: 0, shot: -1, lastE: null };
    var ux, uy, vx, vy, m;
    function clear() {
      var k = N * N; ux = new Float64Array(k); uy = new Float64Array(k); vx = new Float64Array(k); vy = new Float64Array(k); m = new Float64Array(k);
      for (var i = 0; i < N; i++) for (var j = 0; j < N; j++) m[i * N + j] = st.two && (i + j) % 2 ? 3 : 1;
    }
    // Neighbour displacement, with the fixed walls (displacement 0) past the edge.
    function at(a, i, j) { return i < 0 || j < 0 || i >= N || j >= N ? 0 : a[i * N + j]; }
    function forces() {
      var fx = new Float64Array(N * N), fy = new Float64Array(N * N);
      for (var i = 0; i < N; i++) for (var j = 0; j < N; j++) {
        var k = i * N + j, x = ux[k], y = uy[k];
        // x-direction bonds: stretch kL on ux, slide kT on uy; y-direction the other way round.
        fx[k] = KL * (at(ux, i - 1, j) + at(ux, i + 1, j) - 2 * x) + KT * (at(ux, i, j - 1) + at(ux, i, j + 1) - 2 * x);
        fy[k] = KT * (at(uy, i - 1, j) + at(uy, i + 1, j) - 2 * y) + KL * (at(uy, i, j - 1) + at(uy, i, j + 1) - 2 * y);
      }
      return [fx, fy];
    }
    function step(dt) {
      var f = forces(), k;
      for (k = 0; k < N * N; k++) { vx[k] += 0.5 * dt * f[0][k] / m[k]; vy[k] += 0.5 * dt * f[1][k] / m[k]; ux[k] += dt * vx[k]; uy[k] += dt * vy[k]; }
      f = forces();
      for (k = 0; k < N * N; k++) { vx[k] += 0.5 * dt * f[0][k] / m[k]; vy[k] += 0.5 * dt * f[1][k] / m[k]; }
    }
    // A standing mode of the walled lattice, polarised along x: u = A sin(p pi (i+1)/(N+1)) sin(r pi (j+1)/(N+1)).
    function modeOf(p, r) {
      var kx = p * Math.PI / (N + 1), ky = r * Math.PI / (N + 1);
      return { p: p, r: r, w: Math.sqrt(4 * KL / M * Math.pow(Math.sin(kx / 2), 2) + 4 * KT / M * Math.pow(Math.sin(ky / 2), 2)) };
    }
    function startMode() {
      clear();
      var p = st.mode === "long" ? 1 : 4, r = 1;
      for (var i = 0; i < N; i++) for (var j = 0; j < N; j++) {
        var k = i * N + j;
        if (st.mode === "optical") ux[k] = 0.25 * ((i + j) % 2 ? -1 : 1) / m[k] * Math.sin(Math.PI * (j + 1) / (N + 1));
        else ux[k] = 0.3 * Math.sin(p * Math.PI * (i + 1) / (N + 1)) * Math.sin(r * Math.PI * (j + 1) / (N + 1));
      }
    }
    clear();
    var MODE = modeOf(1, 1);
    var frame = new Frame(root, {
      reset: function () { st.t = 0; st.n = 0; st.shot = -1; st.lastE = null; if (st.stage === "mode") startMode(); else clear(); },
      tick: function (dt) {
        st.t += dt;
        if (st.stage !== "phonon") for (var s = 0; s < 6; s++) step(Math.min(dt, 0.05) * 1.2);
        if (st.shot >= 0) { st.shot += dt; if (st.shot > 2.2) st.shot = -1; }
        return true;
      },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var side = Math.min(w - (st.stage === "phonon" ? 140 : 30), h - 150), a = side / (N + 1) * zoom, x0 = 18 + a, y0 = 84 + a;
        var amp = st.stage === "phonon" ? 0.45 * Math.sqrt(st.n) * Math.cos(st.t * 3) : 0;
        var pos = function (i, j) {
          var k = i * N + j;
          if (st.stage === "phonon") return [x0 + i * a + a * amp * Math.sin(Math.PI * (i + 1) / (N + 1)) * Math.sin(Math.PI * (j + 1) / (N + 1)), y0 + j * a];
          return [x0 + (i + ux[k]) * a, y0 + (j + uy[k]) * a];
        };
        ctx.strokeStyle = "rgba(128,128,128,0.55)"; ctx.lineWidth = 1;
        for (var i = 0; i < N; i++) for (var j = 0; j < N; j++) {
          var p = pos(i, j);
          if (i + 1 < N) { var q = pos(i + 1, j); ctx.beginPath(); ctx.moveTo(p[0], p[1]); ctx.lineTo(q[0], q[1]); ctx.stroke(); }
          if (j + 1 < N) { var r = pos(i, j + 1); ctx.beginPath(); ctx.moveTo(p[0], p[1]); ctx.lineTo(r[0], r[1]); ctx.stroke(); }
        }
        for (i = 0; i < N; i++) for (j = 0; j < N; j++) {
          var pp = pos(i, j), heavy = st.two && st.stage !== "phonon" && (i + j) % 2;
          ctx.fillStyle = heavy ? "#7d5a4f" : acc; ctx.beginPath(); ctx.arc(pp[0], pp[1], a * (heavy ? 0.2 : 0.14), 0, 2 * Math.PI); ctx.fill();
        }
        ctx.strokeStyle = mut; ctx.strokeRect(x0 - a, y0 - a, (N + 1) * a, (N + 1) * a);
        ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif";
        if (st.stage === "push") { ctx.fillText("tap an atom to push it", 12, 76); return "Each atom is held to its neighbours by springs. Push one and its neighbours follow, so the disturbance travels through the lattice as a wave. Nothing damps it: in the ideal lattice the waves keep going (Reset to calm it)."; }
        if (st.stage === "mode") {
          ctx.fillText(st.mode === "optical" ? "neighbours against each other" : st.mode === "long" ? "the longest wave" : "a shorter wave", 12, 76);
          return st.mode === "optical" ? "With two kinds of atom, neighbours can also move against each other: a higher-frequency (optical) pattern. Drawn by starting the lattice in that pattern; near the walls it is not exactly a mode, so it slowly changes."
            : "A normal mode: every atom moves in step, at one frequency, like a guitar string's harmonic. The longest waves are sound in the solid (acoustic). Shorter waves vibrate faster.";
        }
        // Phonons: the ladder of the longest mode, with the count.
        var lx = w - 92, ly0 = y0 + (N - 1) * a, stp = Math.min(34, (N - 1) * a / 5);
        ctx.strokeStyle = fg; ctx.lineWidth = 1.5;
        for (var k2 = 0; k2 < 6; k2++) { ctx.beginPath(); ctx.moveTo(lx, ly0 - k2 * stp); ctx.lineTo(lx + 40, ly0 - k2 * stp); ctx.stroke(); }
        ctx.fillStyle = hot; ctx.beginPath(); ctx.arc(lx + 20, ly0 - Math.min(st.n, 5) * stp, 6, 0, 2 * Math.PI); ctx.fill();
        ctx.fillStyle = fg; ctx.font = "bold 15px system-ui, sans-serif"; ctx.fillText("n = " + st.n, lx - 2, ly0 - 5.6 * stp);
        ctx.font = "13px system-ui, sans-serif"; ctx.fillText("phonons in the longest mode", 12, 76);
        // The neutron's pass, when one was sent.
        if (st.shot >= 0) {
          var t = st.shot, cx = x0 + 4 * a, cy = y0 + 4 * a, nx = t < 1 ? x0 - a + t * (cx - x0 + a) : cx + (t - 1) * a * 3 * (st.lastE && st.lastE.gain ? 1.3 : 0.8), ny = t < 1 ? cy : cy - (t - 1) * a * 2;
          ctx.fillStyle = "#3a6ea5"; ctx.beginPath(); ctx.arc(nx, ny, 5, 0, 2 * Math.PI); ctx.fill();
        }
        var hw = mev(MODE.w);
        var s = "One mode's energy comes in equal packets of ħω = " + hw.toFixed(1) + " meV (toy scale). Each packet is one phonon; this mode now holds " + st.n + ".";
        if (st.lastE) s += st.lastE.gain ? " A " + st.lastE.e.toFixed(1) + " meV neutron absorbed one: it left with " + st.lastE.e2.toFixed(1) + " meV, so E′ − E = ħω." : " A " + st.lastE.e.toFixed(1) + " meV neutron made one: it left with " + st.lastE.e2.toFixed(1) + " meV, so E − E′ = ħω.";
        return s + " The neutron never hits the phonon itself: it kicks a nucleus, and the springs pass the kick into the vibration.";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "A toy: a 2D lattice of 9 × 9 masses joined by springs, stiffer for stretching (" + KL + ") than for sliding (" + KT + "), fixed walls around it, no damping. Real crystals are 3D, and real bonds are spring-like only for small displacements (the harmonic approximation). Phonon energies are scaled so the toy's highest is " + TOP_MEV + " meV, the top of α-quartz's spectrum (page 20)."));
      }
    });
    var card = root.parentNode;
    function show(sel, on) { card.querySelectorAll(sel).forEach(function (r) { r.style.display = on ? "" : "none"; }); }
    function sync() { show("[data-for=mode]", st.stage === "mode"); show("[data-for=phonon]", st.stage === "phonon"); }
    card.querySelectorAll("[data-stage]").forEach(function (b) {
      b.onclick = function () {
        st.stage = b.getAttribute("data-stage"); st.t = 0; st.n = 0; st.lastE = null;
        card.querySelectorAll("[data-stage]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
        if (st.stage === "mode") startMode(); else clear();
        sync(); frame.play();
      };
    });
    card.querySelectorAll("[data-mode]").forEach(function (b) {
      b.onclick = function () {
        st.mode = b.getAttribute("data-mode"); st.two = st.mode === "optical";
        card.querySelectorAll("[data-mode]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
        startMode(); frame.play();
      };
    });
    var on = function (sel, f) { var b = card.querySelector(sel); if (b) b.onclick = f; };
    on("[data-add]", function () { st.n++; st.lastE = null; frame.play(); });
    on("[data-remove]", function () { if (st.n > 0) st.n--; st.lastE = null; frame.play(); });
    on("[data-neutron]", function () {
      var E = 25.3, hw = mev(MODE.w), gain = st.has && st.n > 0;
      if (gain) st.n--; else st.n++;
      st.lastE = { e: E, e2: gain ? E + hw : E - hw, gain: gain }; st.shot = 0; frame.play();
    });
    var hasBox = card.querySelector("input[data-has]");
    if (hasBox) hasBox.onchange = function () { st.has = hasBox.checked; if (st.has && st.n === 0) st.n = 1; frame.play(); };
    // Tap to push the nearest atom.
    frame.canvas.addEventListener("pointerdown", function (ev) {
      if (st.stage !== "push") return;
      var rect = frame.canvas.getBoundingClientRect(), x = ev.clientX - rect.left, y = ev.clientY - rect.top;
      var side = Math.min(rect.width - 30, rect.height - 150), a = side / (N + 1) * frame.zoom, x0 = 18 + a, y0 = 84 + a;
      var i = Math.round((x - x0) / a), j = Math.round((y - y0) / a);
      if (i >= 0 && j >= 0 && i < N && j < N) { vx[i * N + j] += 1.2; frame.play(); }
    });
    sync();
    return frame;
  }

  // ─── #phonon: one phonon's direction and style; the neutron's kick ─────
  function phononWidget(root) {
    var st = { theta: 0, lam: 6, style: "L", t: 0, kick: false, qang: 0, qmag: 1.2, dE: 60 };
    var frame = new Frame(root, {
      reset: function () { st.t = 0; },
      tick: function (dt) { st.t += dt; return true; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var qx, qy, e, wv, strength = 1, msg, mm = null;
        if (!st.kick) {
          var qn = 2 * Math.PI / st.lam; qx = qn * Math.cos(st.theta); qy = qn * Math.sin(st.theta);
          e = st.style === "L" ? [Math.cos(st.theta), Math.sin(st.theta)] : [-Math.sin(st.theta), Math.cos(st.theta)];
          wv = Math.sqrt(omega2(qx, qy, st.style === "L" ? KL : KT, M));
        } else {
          var Qx = st.qmag * Math.PI * Math.cos(st.qang), Qy = st.qmag * Math.PI * Math.sin(st.qang);
          mm = match(Qx, Qy, st.dE);
          var b = mm.L.hit && mm.L.strength >= mm.T.strength ? "L" : mm.T.hit ? "T" : mm.L.hit ? "L" : null;
          qx = mm.fold.qx; qy = mm.fold.qy;
          e = b ? mm[b].e : [1, 0]; strength = b ? mm[b].strength : 0; wv = b ? Math.sqrt(omega2(qx, qy, b === "L" ? KL : KT, M)) : 0;
          st.style = b || st.style;
        }
        var N = 11, side = Math.min(w - 24, h - 170), a = side / (N - 1) * zoom, x0 = (w - side) / 2, y0 = 92;
        var A = 0.28 * a * Math.sqrt(strength);
        ctx.fillStyle = acc;
        for (var i = 0; i < N; i++) for (var j = 0; j < N; j++) {
          var ph = qx * i + qy * j - wv * st.t * 2.2, d = A * Math.cos(ph);
          ctx.beginPath(); ctx.arc(x0 + i * a + d * e[0], y0 + j * a + d * e[1], a * 0.12, 0, 2 * Math.PI); ctx.fill();
        }
        // Two separate arrows: where the wave travels (q), how the atoms move (e).
        var cx = w - 84, cy = y0 + side + 40, qn2 = Math.hypot(qx, qy) || 1;
        L.arrow(ctx, cx - 30, cy, cx - 30 + 26 * qx / qn2, cy + 26 * qy / qn2, hot);
        if (!st.kick || strength > 0) L.arrow(ctx, cx + 20, cy, cx + 20 + 22 * e[0], cy + 22 * e[1], acc);
        ctx.font = "11px system-ui, sans-serif"; ctx.fillStyle = hot; ctx.fillText("travels", cx - 52, cy + 30); ctx.fillStyle = acc; ctx.fillText("atoms move", cx, cy + 30);
        if (st.kick) {
          var kx = x0 + side / 2, ky = y0 + side / 2, Qn = Math.hypot(Math.cos(st.qang), Math.sin(st.qang));
          L.arrow(ctx, kx, ky, kx + 50 * Math.cos(st.qang) / Qn, ky + 50 * Math.sin(st.qang) / Qn, "#3a6ea5");
          // Dispersion inset along the kick's direction: hbar omega against |q|.
          var ix = 14, iy = y0 + side + 22, iw = 130, ih = 56;
          ctx.strokeStyle = mut; ctx.strokeRect(ix, iy, iw, ih);
          // Curves along the phonon's own wavevector direction, out to |q| = pi sqrt(2).
          var qfn = Math.hypot(mm.fold.qx, mm.fold.qy), ux2 = qfn > 1e-9 ? mm.fold.qx / qfn : 1, uy2 = qfn > 1e-9 ? mm.fold.qy / qfn : 0, QM = Math.PI * Math.SQRT2;
          [["L", KL, hot], ["T", KT, acc]].forEach(function (br) {
            ctx.strokeStyle = br[2]; ctx.beginPath();
            for (var s = 0; s <= 40; s++) { var qq = s / 40 * QM, wq = mev(Math.sqrt(omega2(qq * ux2, qq * uy2, br[1], M))); var X = ix + s / 40 * iw, Y = iy + ih - wq / TOP_MEV * ih; if (s) ctx.lineTo(X, Y); else ctx.moveTo(X, Y); }
            ctx.stroke();
          });
          var qf = qfn / QM;
          ctx.fillStyle = "#3a6ea5"; ctx.beginPath(); ctx.arc(ix + qf * iw, iy + ih - st.dE / TOP_MEV * ih, 4, 0, 2 * Math.PI); ctx.fill();
          ctx.fillStyle = mut; ctx.fillText("ħω vs wavevector", ix, iy - 3);
          var res = (mm.L.hit || mm.T.hit) && strength < 0.02 ? "The energy fits a " + (st.style === "L" ? "longitudinal" : "transverse") + " phonon (" + mm[st.style].mev.toFixed(0) + " meV), but its atoms move across the kick: (Q̂·e)² = " + strength.toFixed(2) + ", so almost nothing is made."
            : (mm.L.hit || mm.T.hit) ? "A match: the kick makes a " + (st.style === "L" ? "longitudinal" : "transverse") + " phonon of " + mm[st.style].mev.toFixed(0) + " meV, with strength (Q̂·e)² = " + strength.toFixed(2) + "."
            : "No match: a " + st.dE.toFixed(0) + " meV energy loss with this kick lands on no allowed phonon (longitudinal would need " + mm.L.mev.toFixed(0) + " meV, transverse " + mm.T.mev.toFixed(0) + " meV), so no phonon is made.";
          msg = res + (mm.fold.gx || mm.fold.gy ? " The kick is longer than the lattice allows a wave to be short, so the phonon's wavevector is the kick minus a lattice vector: its atom motion can now lie across the kick." : " Here the phonon's wavevector equals the kick, so only the longitudinal style (atoms moving along the kick) is pushed.");
          ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText("the neutron's kick (blue) and the phonon it makes", 12, 80);
          return msg;
        }
        ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText("one phonon", 12, 80);
        return "One phonon: a wave " + st.lam.toFixed(1) + " spacings long, travelling at " + Math.round(st.theta * 180 / Math.PI) + "°, " + (st.style === "L" ? "longitudinal: the atoms move along the travel direction, like sound." : "transverse: the atoms move across the travel direction, like a wave on a rope. In 3D there are two such sideways styles.") +
          " ħω = " + mev(wv).toFixed(0) + " meV (toy scale).";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "A toy dispersion on a square lattice: ω² = (4k/m)(sin²(q_x a/2) + sin²(q_y a/2)), stiffer for longitudinal (k = " + KL + ") than transverse (" + KT + ") motion, energies scaled so the highest phonon is " + TOP_MEV + " meV."));
      }
    });
    var card = root.parentNode;
    var bindRange = function (attr, set, fmt) { var i = card.querySelector("[" + attr + "]"), o = card.querySelector("[" + attr + "-out]"); if (!i) return; var go = function () { set(parseFloat(i.value)); if (o) o.textContent = fmt(parseFloat(i.value)); frame.redraw(); }; i.oninput = go; go(); };
    bindRange("data-theta", function (v) { st.theta = v * Math.PI / 180; }, function (v) { return v.toFixed(0) + "°"; });
    bindRange("data-lam", function (v) { st.lam = v; }, function (v) { return v.toFixed(1) + " spacings"; });
    bindRange("data-qang", function (v) { st.qang = v * Math.PI / 180; }, function (v) { return v.toFixed(0) + "°"; });
    bindRange("data-qmag", function (v) { st.qmag = v; }, function (v) { return v.toFixed(2) + " π/a"; });
    bindRange("data-de", function (v) { st.dE = v; }, function (v) { return v.toFixed(0) + " meV"; });
    card.querySelectorAll("[data-style]").forEach(function (b) {
      b.onclick = function () { st.style = b.getAttribute("data-style"); card.querySelectorAll("[data-style]").forEach(function (o) { o.classList.toggle("ghost", o !== b); }); frame.redraw(); };
    });
    function show(sel, on2) { card.querySelectorAll(sel).forEach(function (r) { r.style.display = on2 ? "" : "none"; }); }
    card.querySelectorAll("[data-pstage]").forEach(function (b) {
      b.onclick = function () {
        st.kick = b.getAttribute("data-pstage") === "kick";
        card.querySelectorAll("[data-pstage]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
        show("[data-for=one]", !st.kick); show("[data-for=kick]", st.kick); frame.redraw();
      };
    });
    show("[data-for=kick]", false);
    return frame;
  }

  L.widget("lattice", latticeWidget);
  L.widget("phonon", phononWidget);
})();
