// Fission products, lesson 1: the widgets. #onefission animates one fission
// event sampled by fission-physics.js, stage by stage, with the real time of
// each stage and the factor it is slowed by; #fpcharts draws the same
// event's numbers (yield curve, N/Z, energy budget, spectra); #lattice is an
// illustrative picture of what each kind of particle does to a crystal.
// Uses ../neutronics/lesson-kit.js unchanged.
//
// Event sampling takes well under a millisecond (pairs are tabulated once
// per parent and energy, about 50 ms the first time); the 2000-event
// spectrum sample runs in chunks of 100 between frames, so the page never
// blocks.
(function () {
  "use strict";
  var L = window.Lesson, F = window.FissionPhysics;
  var css = L.css, el = L.el, Frame = L.Frame;
  var D = function () { return window.FISSION_DATA; };
  var PEND = function () { return window.FISSION_PENDING || {}; };
  var SYM = ["n","H","He","Li","Be","B","C","N","O","F","Ne","Na","Mg","Al","Si","P","S","Cl","Ar","K","Ca","Sc","Ti","V","Cr","Mn","Fe","Co","Ni","Cu","Zn","Ga","Ge","As","Se","Br","Kr","Rb","Sr","Y","Zr","Nb","Mo","Tc","Ru","Rh","Pd","Ag","Cd","In","Sn","Sb","Te","I","Xe","Cs","Ba","La","Ce","Pr","Nd","Pm","Sm","Eu","Gd","Tb","Dy","Ho","Er","Tm","Yb","Lu","Hf","Ta","W","Re","Os","Ir","Pt","Au","Hg","Tl","Pb","Bi","Po","At","Rn","Fr","Ra","Ac","Th","Pa","U","Np","Pu"];
  function nm(z, a) { return SYM[z] + "-" + a; }
  function sci(x, d) {
    if (x === null || x === undefined || !isFinite(x)) return "?";
    var e = Math.floor(Math.log10(Math.abs(x))), m = x / Math.pow(10, e);
    if (m >= 9.995) { m /= 10; e += 1; }
    return m.toFixed(d === undefined ? 1 : d) + "×10" + sup(e);
  }
  function sup(n) { return String(n).replace("-", "⁻").replace(/[0-9]/g, function (c) { return "⁰¹²³⁴⁵⁶⁷⁸⁹"[+c]; }); }
  function eLabel(e) { return e < 1 ? "thermal (0.0253 eV)" : e < 1e6 ? (e / 1e3) + " keV" : (e / 1e6) + " MeV"; }
  function param(k) { var m = new RegExp("[?&]" + k + "=([^&]*)").exec(location.search); return m ? decodeURIComponent(m[1]) : null; }

  // Shared state: the chosen parent, energy and the last event.
  var S = {
    parent: "U-235", ie: 0, speed: 1, ev: null, chains: null, seed: +(param("seed") || 1),
    forceTernary: param("ternary") === "1", listeners: [], spectrum: null, launches: 0
  };
  var TERNARY_EVERY = 10;
  // Defer the next slice of a long build to after the next frame.
  function SLICE_LATER(f) { setTimeout(f, 0); }
  function notify() { S.listeners.forEach(function (f) { f(); }); }
  function Pnow() { return D().parents[S.parent]; }

  // ── The stages of one event, with their real durations ────────────────
  var VIEW_FM = 40; // half-width of the scene at zoom 1 [fm]
  function stages(ev) {
    var P = Pnow(), T = F.timescales(S.parent, S.ie), pend = PEND();
    var R = F.R0_FM * Math.pow(P.a, 1 / 3);
    var vf = Math.max(Math.hypot.apply(null, ev.frags[0].v0), Math.hypot.apply(null, ev.frags[1].v0)) * F.C;
    var cn = nm(P.z, P.a + 1) + "*";
    return [
      { key: "approach", title: "The neutron reaches " + S.parent, real: 4 * R * 1e-15 / T.vn, disp: 3,
        how: "4R / v, the last 4 nuclear radii of its path (R = r₀A^⅓, v from its energy)" },
      { key: "cn", title: "The compound nucleus " + cn + " wobbles", real: T.tauCN, disp: 3,
        how: T.tauCN ? "τ = ħ/⟨Γ⟩, the mean total width of the resolved resonances" : "source pending: no resolved resonance widths at this energy" },
      { key: "neck", title: "It stretches, necks and splits", real: pend.saddleToScissionS, disp: 3, how: "saddle to scission: source pending" },
      { key: "accel", title: "The two fragments push apart", real: pend.fragmentAccelerationS, disp: 2, how: "acceleration time: source pending" },
      { key: "fly", title: "The fragments fly apart", real: 30e-15 / vf, disp: 3, illustrative: true, how: "the time the faster fragment takes for the 30 fm drawn: the picture's distance, so not on the stage timeline" },
      { key: "prompt", title: "Prompt neutrons boil off the fragments", real: null, bound: T.promptBound, disp: 4,
        how: "over 99 % of fission neutrons leave within about 10⁻¹³ s (an upper bound); not to scale" },
      { key: "gamma", title: "Prompt gamma rays", real: pend.promptGammaStartS, disp: 2, how: "start of the gamma cascade: source pending" },
      { key: "beta", title: "Beta decay, on a slow clock", real: null, disp: 14, log: [-1, 10], how: "half-lives from NuDat; log clock 0.1 s to 10¹⁰ s" }
    ];
  }

  // ── #onefission ────────────────────────────────────────────────────────
  function oneFission(root) {
    var A = { stage: -1, t: 0, playing: true, st: null };
    var readout = root.parentNode.querySelector(".fp-readout");
    function launch() {
      // Ternary fission is rare, and its rate is source pending
      // (fission-pending.js). So that a reader meets one, the demo makes
      // every TERNARY_EVERY-th launch ternary: the demo's choice, said so on
      // screen, never a rate. The panel's switch forces it.
      S.launches += 1;
      var tern = S.forceTernary || S.launches % TERNARY_EVERY === 0;
      var parent = S.parent, ie = S.ie;
      // The first event of a parent, energy and kind builds its pair table
      // (~30 ms): in slices between frames, so the page never blocks.
      F.preparePairs(parent, ie, tern, function () {
        if (parent !== S.parent || ie !== S.ie) return; // the reader moved on
        var ev = F.event(parent, ie, { seed: S.seed, ternary: tern });
        S.seed += 1;
        S.ev = ev;
        S.chains = ev.frags.map(function (f, i) { return F.betaChain(parent, f.z, f.a, ev.seed * 7 + i + 1, 1e10); });
        A.st = stages(ev); A.stage = 0; A.t = 0; A.playing = true;
        writeReadout();
        notify();
        frame.play();
      }, SLICE_LATER, 120);
    }
    function writeReadout() {
      var ev = S.ev; if (!ev || !readout) return;
      var P = Pnow(), cnA = P.a + 1;
      var f = ev.frags, pr = ev.pair;
      var lines = [];
      lines.push("<b>The split:</b> " + nm(P.z, cnA) + "* → " + nm(f[0].z, f[0].A) + " + " + nm(f[1].z, f[1].A) + (ev.ternary ? " + He-4 (ternary, forced)" : "") +
        "; after the prompt neutrons: " + nm(pr.z1, pr.a1) + " + " + nm(pr.z2, pr.a2) + " + " + pr.nu + " n. Charge " + pr.z1 + " + " + pr.z2 + (ev.ternary ? " + 2" : "") + " = " + P.z +
        "; mass number " + pr.a1 + " + " + pr.a2 + " + " + pr.nu + (ev.ternary ? " + 4" : "") + " = " + cnA + ".");
      lines.push("<b>Energy:</b> E* of " + nm(P.z, cnA) + " = S<sub>n</sub> + E<sub>cm</sub> = " + ev.I.Sn.toFixed(3) + " + " + (ev.I.Ex - ev.I.Sn).toExponential(2) + " = " + ev.I.Ex.toFixed(3) +
        " MeV. Q of this split = M(CN*) − M₁ − M₂ − ν m<sub>n</sub>" + (ev.ternary ? " − M(He-4)" : "") + " = " + ev.qSplit.toFixed(2) + " MeV. TKE = EFR (MT=458, evaluated average) = " + ev.tke.toFixed(2) +
        " MeV; excitation left for neutrons and gammas = " + (ev.qSplit - ev.tke).toFixed(2) + " MeV, shared in proportion to mass number (an assumption): " + f[0].ex.toFixed(2) + " and " + f[1].ex.toFixed(2) + " MeV.");
      if (ev.ternary) lines.push("<b>Ternary fission:</b> a light charged particle, here an alpha, left from the neck. Its energy " + ev.Talpha.toFixed(2) + " MeV" + (ev.alphaRule ? " is a placeholder rule (EFR × 4/A<sub>CN</sub>), <span class=\"pending\">source pending</span>" : "") + "; it leaves perpendicular to the fission axis. In nature ternary fission is rare; its rate is <span class=\"pending\">source pending</span>, so the demo makes one launch in " + TERNARY_EVERY + " ternary (or every launch, with the switch): <b>a demo choice, not the rate</b>.");
      var nz = f.map(function (x) {
        var zs = F.stableZ(x.A);
        return nm(x.z, x.A) + " N/Z = " + ((x.A - x.z) / x.z).toFixed(3) + " against " + ((x.A - zs) / zs).toFixed(3) + " for the stable isobar " + nm(zs, x.A);
      });
      lines.push("<b>Neutron-rich:</b> " + nz.join("; ") + ".");
      lines.push("<b>Prompt neutrons</b> (lab energies): " + (ev.neutrons.length ? ev.neutrons.map(function (n) { return n.T.toFixed(2); }).join(", ") + " MeV" : "none in this event") +
        "; <b>prompt gammas:</b> " + (ev.gammas.length ? ev.gammas.map(function (g) { return g.E.toFixed(2); }).join(", ") + " MeV" : "none") + ".");
      lines.push("<b>Fragment kinetic energies</b> at scission: " + f[0].T0.toFixed(2) + " and " + f[1].T0.toFixed(2) + " MeV (" + L.fmtSpeed(Math.hypot.apply(null, f[0].v0) * F.C) + ", " + L.fmtSpeed(Math.hypot.apply(null, f[1].v0) * F.C) + ").");
      lines.push("<b>Conservation</b> (all final particles against neutron + target): |Σp − p<sub>n</sub>| = " + ev.dP.toExponential(1) + " MeV/c (p<sub>fragment</sub> ≈ " + ev.pScale.toFixed(0) +
        " MeV/c); ΔE = " + ev.dE.toExponential(1) + " MeV out of " + ev.eScale.toExponential(4) + " MeV (relative " + (Math.abs(ev.dE) / ev.eScale).toExponential(1) + "): round-off.");
      readout.innerHTML = lines.map(function (l) { return "<p>" + l + "</p>"; }).join("");
    }
    // Scene drawing helpers.
    function circle(ctx, x, y, r, fill, stroke) { ctx.beginPath(); ctx.arc(x, y, Math.max(r, 1), 0, 2 * Math.PI); if (fill) { ctx.fillStyle = fill; ctx.fill(); } if (stroke) { ctx.strokeStyle = stroke; ctx.lineWidth = 1.5; ctx.stroke(); } }
    function label(ctx, t, x, y, col) { ctx.fillStyle = col; ctx.fillText(t, x, y); }
    var COL = { n: "#2f7fd8", frag1: "#d9822b", frag2: "#b5452f", gamma: "#8e5bd6", alpha: "#2a9d55", target: "#7a8691" };

    var frame = new Frame(root, {
      reset: function () {},
      onSeen: function () { if (!S.ev) launch(); },
      tick: function (dt) {
        if (!A.st || !A.playing) return false;
        A.t += dt * S.speed;
        var s = A.st[A.stage];
        if (A.t >= s.disp) {
          if (A.stage < A.st.length - 1) { A.stage += 1; A.t = 0; }
          else { A.t = s.disp; A.playing = false; return false; }
        }
        return true;
      },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b"), acc = css("--accent", "#1f6f78");
        ctx.font = "12px system-ui, sans-serif";
        var tlH = 58, sh = h - tlH, cx = w / 2, cy = 50 + (sh - 50) / 2;
        var scale = Math.min(w, sh - 50) / (2 * VIEW_FM) * zoom; // px per fm
        if (!S.ev || !A.st) { ctx.fillStyle = mut; ctx.fillText("Press “Launch neutron”.", 12, 60); drawTimeline(ctx, w, h, tlH, mut, fg, acc); return "No event yet."; }
        var ev = S.ev, P = Pnow(), s = A.st[A.stage], u = Math.min(A.t / s.disp, 1);
        var Rt = F.R0_FM * Math.pow(P.a, 1 / 3), Rc = F.R0_FM * Math.pow(P.a + 1, 1 / 3);
        var f = ev.frags, R1 = F.R0_FM * Math.pow(f[0].A, 1 / 3), R2 = F.R0_FM * Math.pow(f[1].A, 1 / 3);
        var ax = Math.atan2(f[0].v0[1], f[0].v0[0]);
        var X = function (fm) { return cx + fm * scale; }, Y = function (fm) { return cy - fm * scale; };
        var key = s.key;
        if (key === "approach") {
          circle(ctx, X(0), Y(0), Rt * scale, "rgba(122,134,145,0.35)", COL.target);
          label(ctx, S.parent, X(0) - 18, Y(0) + Rt * scale + 14, fg);
          var xn = -5 * Rt + 4 * Rt * u;
          circle(ctx, X(xn), Y(0), Math.max(0.9 * scale, 3), COL.n);
          label(ctx, "n", X(xn) - 3, Y(0) - 8, COL.n);
        } else if (key === "cn") {
          ctx.save(); ctx.translate(X(0), Y(0));
          var wob = 0.12 * Math.sin(2 * Math.PI * 2 * u);
          ctx.beginPath();
          for (var k = 0; k <= 64; k++) {
            var ph = 2 * Math.PI * k / 64, c2 = Math.cos(ph - ax), rr = Rc * (1 + wob * (1.5 * c2 * c2 - 0.5)) * scale;
            var px = rr * Math.cos(ph), py = -rr * Math.sin(ph);
            if (k) ctx.lineTo(px, py); else ctx.moveTo(px, py);
          }
          ctx.fillStyle = "rgba(217,130,43,0.35)"; ctx.fill(); ctx.strokeStyle = COL.frag1; ctx.lineWidth = 2; ctx.stroke(); ctx.restore();
          label(ctx, nm(P.z, P.a + 1) + "*, E* = " + ev.I.Ex.toFixed(2) + " MeV", X(0) - 60, Y(0) + Rc * scale * 1.3 + 14, fg);
        } else if (key === "neck" || key === "accel") {
          var sep = key === "neck" ? (0.3 + 0.75 * u) * (R1 + R2) : (1.05 + 0.6 * u * u) * (R1 + R2);
          var d1 = sep * f[1].A / (f[0].A + f[1].A), d2 = sep - d1;
          var x1 = d1 * Math.cos(ax), y1 = d1 * Math.sin(ax), x2 = -d2 * Math.cos(ax), y2 = -d2 * Math.sin(ax);
          if (key === "neck") {
            var nw = Math.max(0, 1 - u) * Math.min(R1, R2) * 0.9;
            ctx.save(); ctx.translate(X(0), Y(0)); ctx.rotate(-ax);
            ctx.fillStyle = "rgba(217,130,43,0.35)"; ctx.fillRect(-d2 * scale, -nw * scale, sep * scale, 2 * nw * scale); ctx.restore();
          }
          circle(ctx, X(x1), Y(y1), R1 * scale, "rgba(217,130,43,0.45)", COL.frag1);
          circle(ctx, X(x2), Y(y2), R2 * scale, "rgba(181,69,47,0.45)", COL.frag2);
          if (key === "accel" || u > 0.85) {
            label(ctx, nm(f[0].z, f[0].A), X(x1) - 20, Y(y1) - R1 * scale - 6, COL.frag1);
            label(ctx, nm(f[1].z, f[1].A), X(x2) - 20, Y(y2) - R2 * scale - 6, COL.frag2);
          }
          if (ev.ternary && key === "accel") {
            var al = (u * 8) * 1, aa = ax + Math.PI / 2;
            circle(ctx, X(al * Math.cos(aa)), Y(al * Math.sin(aa)), 1.6 * scale, COL.alpha);
            label(ctx, "α", X(al * Math.cos(aa)) + 6, Y(al * Math.sin(aa)), COL.alpha);
          }
        } else if (key === "fly") {
          var vmax = Math.max(Math.hypot.apply(null, f[0].v0), Math.hypot.apply(null, f[1].v0));
          var base = 1.65 * (R1 + R2);
          f.forEach(function (fr, i) {
            var v = fr.v0, d = (i ? -1 : 1) * base * (i ? f[0].A : f[1].A) / (f[0].A + f[1].A);
            var dist = 30 * u / vmax;
            var x = d * Math.cos(ax) + v[0] * dist, y = d * Math.sin(ax) + v[1] * dist;
            circle(ctx, X(x), Y(y), (i ? R2 : R1) * scale, i ? "rgba(181,69,47,0.45)" : "rgba(217,130,43,0.45)", i ? COL.frag2 : COL.frag1);
            label(ctx, nm(fr.z, fr.A) + " " + fr.T0.toFixed(0) + " MeV", X(x) - 30, Y(y) - (i ? R2 : R1) * scale - 6, i ? COL.frag2 : COL.frag1);
          });
          if (ev.alpha) {
            var va = F.velocity(ev.alpha), da = 30 * u / vmax;
            circle(ctx, X(va[0] * da + 8 * Math.cos(ax + Math.PI / 2)), Y(va[1] * da + 8 * Math.sin(ax + Math.PI / 2)), 1.6 * scale, COL.alpha);
          }
          ctx.fillStyle = mut; ctx.fillText("Speeds to scale: the faster fragment covers 30 fm in this stage.", 12, sh - 8);
        } else if (key === "prompt" || key === "gamma") {
          // Not to scale: by now the fragments are far apart.
          var px1 = VIEW_FM * 0.55, ends = [[px1 * Math.cos(ax), px1 * Math.sin(ax)], [-px1 * Math.cos(ax), -px1 * Math.sin(ax)]];
          var vmx = 0;
          ev.neutrons.forEach(function (n) { vmx = Math.max(vmx, Math.hypot(n.p[1], n.p[2]) / n.p[0]); });
          f.forEach(function (fr, i) {
            var e0 = ends[i];
            circle(ctx, X(e0[0]), Y(e0[1]), (i ? R2 : R1) * scale * 0.8, i ? "rgba(181,69,47,0.45)" : "rgba(217,130,43,0.45)", i ? COL.frag2 : COL.frag1);
            var k0 = key === "prompt" ? Math.min(Math.floor(u * (fr.nu + 1)), fr.nu) : fr.nu;
            var lbl = nm(fr.z, fr.A - k0);
            label(ctx, lbl, X(e0[0]) - 18, Y(e0[1]) + (i ? R2 : R1) * scale + 14, i ? COL.frag2 : COL.frag1);
            fr.emissions.forEach(function (em, k) {
              var t0 = (k + 1) / (fr.nu + 1);
              var uu = key === "prompt" ? (u - t0) : (1 - t0 + u);
              if (uu <= 0) return;
              var vv = [em.p[1] / em.p[0], em.p[2] / em.p[0]], len = 26 * uu / Math.max(vmx, 1e-9);
              var xn = e0[0] + vv[0] * len, yn = e0[1] + vv[1] * len;
              ctx.strokeStyle = "rgba(47,127,216,0.35)"; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(X(e0[0]), Y(e0[1])); ctx.lineTo(X(xn), Y(yn)); ctx.stroke();
              circle(ctx, X(xn), Y(yn), 3, COL.n);
              label(ctx, em.p[0] - F.MN > 0 ? (em.p[0] - F.MN).toFixed(2) + " MeV" : "", X(xn) + 5, Y(yn) - 4, COL.n);
            });
            if (key === "gamma" && fr.gamma) {
              var gv = [fr.gamma.p[1] / fr.gamma.E, fr.gamma.p[2] / fr.gamma.E], gl = 40 * u;
              ctx.strokeStyle = COL.gamma; ctx.lineWidth = 2; ctx.beginPath();
              for (var q = 0; q <= 40; q++) {
                var tt = gl * q / 40, wv = Math.sin(q * 1.2) * 1.2;
                var gx = e0[0] + gv[0] * tt - gv[1] * wv, gy = e0[1] + gv[1] * tt + gv[0] * wv;
                if (q) ctx.lineTo(X(gx), Y(gy)); else ctx.moveTo(X(gx), Y(gy));
              }
              ctx.stroke();
              label(ctx, "γ " + fr.gamma.E.toFixed(2) + " MeV", X(e0[0] + gv[0] * gl) + 4, Y(e0[1] + gv[1] * gl), COL.gamma);
            }
          });
          ctx.fillStyle = mut; ctx.fillText("Not to scale: the fragments are drawn close; directions and energies are the event's.", 12, sh - 8);
        } else if (key === "beta") {
          drawBeta(ctx, w, sh, u, fg, mut);
        }
        drawTimeline(ctx, w, h, tlH, mut, fg, acc);
        return status(s, u);
      },
      panel: function (body, redraw) {
        var parentRow = el("div");
        L.buttons(parentRow, "Parent nucleus", Object.keys(D().parents).map(function (k) { return [k, k + " + n"]; }), S.parent, function (v) { S.parent = v; S.ie = 0; buildEnergy(); launch(); });
        body.append(parentRow);
        var energyRow = el("div");
        body.append(energyRow);
        function buildEnergy() {
          energyRow.innerHTML = "";
          L.buttons(energyRow, "Incident neutron energy (the yields' tabulated energies)", Pnow().yields.map(function (y, i) { return [i, eLabel(y.e)]; }), S.ie, function (v) { S.ie = v; launch(); });
        }
        buildEnergy();
        var go = el("button", { type: "button", "class": "w-opt wide on" }, "Launch neutron");
        go.onclick = launch;
        var play = el("button", { type: "button", "class": "w-opt wide" }, "Play / pause");
        play.onclick = function () { A.playing = !A.playing; if (A.playing) frame.play(); };
        var step = el("button", { type: "button", "class": "w-opt wide" }, "Step to next stage");
        step.onclick = function () { if (!A.st) return; if (A.stage < A.st.length - 1) { A.stage += 1; A.t = 0; } else A.t = A.st[A.stage].disp; A.playing = false; redraw(); };
        body.append(go, play, step);
        L.slider(body, "Animation speed", -2, 2, 1, 0, function (v) { S.speed = Math.pow(2, v); return S.speed + "×"; });
        L.toggle(body, "Make every launch ternary (otherwise one in " + TERNARY_EVERY + ": a demo choice; the real rate is source pending)", S.forceTernary, function (v) { S.forceTernary = v; });
        body.append(el("p", { "class": "w-note" }, "Each launch samples a new event (seeded, so ?seed=N reproduces one). Every stage shows its real duration and how much it is slowed; a stage whose duration has no source yet says so."));
      }
    });
    function status(s, u) {
      var known = 0, pending = 0;
      for (var i = 0; i <= A.stage; i++) {
        var st = A.st[i], frac = i === A.stage ? u : 1;
        if (st.key === "beta" || st.key === "prompt" || st.illustrative) continue;
        if (st.real) known += st.real * frac; else pending += 1;
      }
      var line = (A.stage + 1) + "/" + A.st.length + " · " + s.title + ". ";
      if (s.real) {
        var slow = s.disp / S.speed / s.real;
        line += "This stage really lasts " + sci(s.real, 1) + " s; shown slowed 10" + sup(Math.round(Math.log10(slow))) + " ×. ";
      } else if (s.bound) {
        line += "Real time: within about " + sci(s.bound, 0) + " s of the fission (an upper bound); the slow-down is not stated. ";
      } else if (s.key === "beta") {
        var tb = Math.pow(10, s.log[0] + (s.log[1] - s.log[0]) * u);
        line += "Real time since fission ≈ " + sci(tb, 1) + " s (log clock: each shown second spans ×" + (Math.pow(10, (s.log[1] - s.log[0]) / (s.disp / S.speed))).toFixed(1) + "). ";
      } else {
        line += "Real duration: source pending, so the slow-down factor is unknown. ";
      }
      if (s.key !== "beta") line += "Clock: " + sci(known, 1) + " s known" + (pending ? " + " + pending + " stage" + (pending > 1 ? "s" : "") + " of pending duration" : "") + ". ";
      return line + "(" + s.how + ")";
    }
    function drawTimeline(ctx, w, h, tlH, mut, fg, acc) {
      var y0 = h - tlH + 6, x0 = 12, x1 = w - 12, lo = -22, hi = 10;
      var Xt = function (lg) { return x0 + (lg - lo) / (hi - lo) * (x1 - x0); };
      ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(x0, y0 + 18); ctx.lineTo(x1, y0 + 18); ctx.stroke();
      ctx.font = "10px system-ui, sans-serif"; ctx.fillStyle = mut;
      for (var d = lo; d <= hi; d += 4) { var tx = Xt(d); ctx.beginPath(); ctx.moveTo(tx, y0 + 15); ctx.lineTo(tx, y0 + 21); ctx.stroke(); ctx.fillText("10" + sup(d) + " s", tx - 12, y0 + 33); }
      ctx.fillText("duration of each stage (log time)", x0, y0 + 4);
      if (!A.st) return;
      var lastKnown = lo + 1;
      A.st.forEach(function (st, i) {
        if (st.illustrative) return;
        if (st.key === "beta") {
          // The beta stage is a span: the log clock's range.
          ctx.strokeStyle = i === A.stage ? acc : fg; ctx.lineWidth = 4;
          ctx.beginPath(); ctx.moveTo(Xt(st.log[0]), y0 + 18); ctx.lineTo(Xt(st.log[1]), y0 + 18); ctx.stroke(); ctx.lineWidth = 1;
          if (i === A.stage) { ctx.font = "bold 11px system-ui, sans-serif"; ctx.fillStyle = acc; ctx.fillText(String(i + 1), Xt(st.log[0]) - 3, y0 + 46); ctx.font = "10px system-ui, sans-serif"; }
          return;
        }
        var lg = st.real ? Math.log10(st.real) : st.bound ? Math.log10(st.bound) : null;
        var on = i === A.stage;
        if (lg === null) lg = lastKnown + 0.6; else lastKnown = lg;
        var tx = Xt(lg);
        ctx.fillStyle = on ? acc : (st.real || st.bound || st.key === "beta") ? fg : mut;
        ctx.beginPath(); ctx.arc(tx, y0 + 18, on ? 6 : 4, 0, 2 * Math.PI);
        if (st.real || st.bound || st.key === "beta") ctx.fill(); else { ctx.lineWidth = 1.5; ctx.strokeStyle = ctx.fillStyle; ctx.stroke(); }
        if (!st.real && !st.bound && st.key !== "beta") ctx.fillText("?", tx - 3, y0 + 12);
        if (on) { ctx.font = "bold 11px system-ui, sans-serif"; ctx.fillText(String(i + 1), tx - 3, y0 + 46); ctx.font = "10px system-ui, sans-serif"; }
      });
      ctx.font = "12px system-ui, sans-serif";
    }
    function drawBeta(ctx, w, sh, u, fg, mut) {
      var st = A.st[A.stage], tb = Math.pow(10, st.log[0] + (st.log[1] - st.log[0]) * u);
      var colW = w / 2;
      ctx.fillStyle = mut; ctx.fillText("Each fragment beta-decays (n → p + e⁻ + ν̄) towards the stable isobar. Real time ≈ " + sci(tb, 1) + " s.", 12, 64);
      S.chains.forEach(function (ch, i) {
        var x = 12 + i * colW, y = 88;
        ch.forEach(function (stp, k) {
          var reached = k === 0 || ch[k - 1].t <= tb;
          if (!reached) return;
          var cur = !(stp.to && stp.t <= tb);
          ctx.fillStyle = cur ? (i ? "#b5452f" : "#d9822b") : fg;
          ctx.font = (cur ? "bold " : "") + "13px system-ui, sans-serif";
          var tag = nm(stp.z, stp.a) + (stp.half ? "  t½ " + sci(stp.half, 1) + " s" : stp.stable ? "  stable" : "");
          ctx.fillText(tag, x, y);
          ctx.font = "11px system-ui, sans-serif";
          if (stp.to && stp.t <= tb) {
            ctx.fillStyle = mut;
            var t = "β⁻ at " + sci(stp.t, 1) + " s, Q " + stp.q.toFixed(1) + " MeV";
            if (stp.dn) t += " + delayed n " + (stp.dn.E / 1e6).toFixed(2) + " MeV (group " + (stp.dn.group + 1) + ")";
            ctx.fillText(t, x + 8, y + 14);
            if (stp.dn) { ctx.fillStyle = "#2f7fd8"; ctx.beginPath(); ctx.arc(x + colW - 24, y + 10, 5, 0, 2 * Math.PI); ctx.fill(); }
          }
          if (stp.pending && tb > stp.t) { ctx.fillStyle = mut; ctx.fillText("… decays later (t½ " + sci(stp.half, 1) + " s)", x + 8, y + 14); }
          y += 32;
        });
      });
      ctx.font = "12px system-ui, sans-serif";
    }
    S.listeners.push(function () { frame.redraw(); });
    return frame;
  }

  // ── #fpcharts: the same event's numbers ────────────────────────────────
  function charts(root) {
    var view = "yield", sampling = null;
    function axes(ctx, x0, y0, pw, ph, mut) { ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph); }
    var frame = new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b"), acc = css("--accent", "#1f6f78"), hot = "#b85c38";
        ctx.font = "12px system-ui, sans-serif";
        // The first ~56 px are under the widget's button bar: titles start below it.
        var P = Pnow(), Y = P.yields[S.ie], ev = S.ev, x0 = 52, y0 = 104, pw = w - x0 - 14, ph = h - y0 - 36;
        if (view === "yield") {
          var my = {}; for (var i = 0; i < Y.a.length; i++) my[Y.a[i]] = (my[Y.a[i]] || 0) + Y.y[i];
          var X = function (a) { return x0 + (a - 66) / (172 - 66) * pw; };
          var Yp = function (v) { return y0 + ph * (1 - (Math.log10(Math.max(v, 1e-9)) + 8) / 8 / zoom); };
          axes(ctx, x0, y0, pw, ph, mut);
          ctx.fillStyle = mut;
          [-8, -6, -4, -2, 0].forEach(function (d) { var yy = Yp(Math.pow(10, d)); if (yy >= y0) ctx.fillText("10" + sup(d), 8, yy + 4); });
          [70, 90, 110, 130, 150, 170].forEach(function (a) { ctx.fillText(a, X(a) - 8, y0 + ph + 16); });
          ctx.fillText("mass number A", x0 + pw - 90, y0 + ph + 30);
          ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
          ctx.strokeStyle = acc; ctx.lineWidth = 2; ctx.beginPath();
          for (var a = 66, first = true; a <= 172; a++) { if (!my[a]) continue; var yy2 = Yp(my[a] / 2); if (first) { ctx.moveTo(X(a), yy2); first = false; } else ctx.lineTo(X(a), yy2); }
          ctx.stroke();
          if (ev && ev.parent === S.parent) ev.frags.forEach(function (f, j) {
            ctx.strokeStyle = j ? "#b5452f" : "#d9822b"; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(X(f.a), y0); ctx.lineTo(X(f.a), y0 + ph); ctx.stroke();
            ctx.setLineDash([4, 3]); ctx.beginPath(); ctx.moveTo(X(f.A), y0); ctx.lineTo(X(f.A), y0 + ph); ctx.stroke(); ctx.setLineDash([]);
          });
          ctx.restore();
          ctx.fillStyle = fg; ctx.fillText("Independent yield Y(A)/2 per product, " + eLabel(Y.e), 8, 70);
          ctx.fillStyle = mut; ctx.fillText("Solid: this event's products; dashed: its fragments before their prompt neutrons.", 8, 88);
          return S.parent + " at " + eLabel(Y.e) + ": the two humps of the mass yield, from ENDF/B-VIII.0 (fission-yields-data). The yields are counted after the prompt neutrons leave and before any beta decay.";
        }
        if (view === "nz") {
          var Xa = function (a) { return x0 + (a - 66) / (172 - 66) * pw; };
          var Yn = function (r) { return y0 + ph * (1 - (r - 1.1) / (1.75 - 1.1)); };
          axes(ctx, x0, y0, pw, ph, mut);
          ctx.fillStyle = mut; [1.2, 1.4, 1.6].forEach(function (r) { ctx.fillText(r.toFixed(1), 18, Yn(r) + 4); });
          [70, 90, 110, 130, 150, 170].forEach(function (a) { ctx.fillText(a, Xa(a) - 8, y0 + ph + 16); });
          ctx.strokeStyle = mut; ctx.lineWidth = 2; ctx.beginPath();
          for (var a2 = 66; a2 <= 172; a2++) { var zs = F.stableZ(a2); var yy3 = Yn((a2 - zs) / zs); if (a2 === 66) ctx.moveTo(Xa(a2), yy3); else ctx.lineTo(Xa(a2), yy3); }
          ctx.stroke();
          var zc = P.z, ac = P.a + 1, rc = (ac - zc) / zc;
          ctx.strokeStyle = hot; ctx.setLineDash([5, 4]); ctx.beginPath(); ctx.moveTo(x0, Yn(rc)); ctx.lineTo(x0 + pw, Yn(rc)); ctx.stroke(); ctx.setLineDash([]);
          ctx.fillStyle = hot; ctx.fillText(nm(zc, ac) + " N/Z = " + rc.toFixed(3), x0 + 6, Yn(rc) - 6);
          ctx.fillStyle = mut; ctx.fillText("beta-stable isobar (lowest mass in NuDat)", x0 + 6, Yn(1.2) + 30);
          if (ev && ev.parent === S.parent) ev.frags.forEach(function (f, j) {
            ctx.fillStyle = j ? "#b5452f" : "#d9822b";
            ctx.beginPath(); ctx.arc(Xa(f.A), Yn((f.A - f.z) / f.z), 6, 0, 2 * Math.PI); ctx.fill();
            ctx.fillText(nm(f.z, f.A), Xa(f.A) - 20, Yn((f.A - f.z) / f.z) - 10);
          });
          ctx.fillStyle = fg; ctx.fillText("N/Z against mass number A", 8, 70);
          ctx.fillStyle = mut; ctx.fillText("Fragments keep the compound nucleus's N/Z; stable nuclei have less.", 8, 88);
          return "The fragments inherit the compound nucleus's N/Z (dashed). The stable isobars at A ≈ 90-150 need fewer neutrons per proton, so the fragments are neutron-rich: they shed neutrons at once, then beta-decay (n → p) for seconds to years.";
        }
        if (view === "budget") {
          var e = Y.e, comps = [[1, "fragments (EFR)", "#d9822b"], [2, "prompt n (ENP)", "#2f7fd8"], [4, "prompt γ (EGP)", "#8e5bd6"], [5, "betas (EB)", "#2a9d55"], [6, "delayed γ (EGD)", "#b07cd6"], [3, "delayed n (END)", "#7fb2e8"], [7, "neutrinos (ENU)", "#9aa3ab"]];
          var vals = comps.map(function (c) { return F.mt458(P, c[0], e); }), tot = vals.reduce(function (s, v) { return s + v; }, 0);
          var bx = 12, bw = w - 24, by = 96, bh = 30, x = bx;
          comps.forEach(function (c, k) {
            var ww = bw * vals[k] / tot; ctx.fillStyle = c[2]; ctx.fillRect(x, by, ww, bh); x += ww;
          });
          ctx.font = "12px system-ui, sans-serif";
          comps.forEach(function (c, k) {
            var yy = by + bh + 20 + k * 17;
            ctx.fillStyle = c[2]; ctx.fillRect(bx, yy - 10, 12, 12);
            ctx.fillStyle = fg; ctx.fillText(c[1] + ": " + vals[k].toFixed(3) + " MeV (" + (100 * vals[k] / tot).toFixed(1) + " %)", bx + 18, yy);
          });
          var ER = F.mt458(P, 8, e), ET = F.mt458(P, 9, e);
          ctx.fillStyle = mut; ctx.fillText("ER = " + ER.toFixed(2) + " MeV (all but the neutrinos),", bx, by + bh + 20 + comps.length * 17 + 2);
          ctx.fillText("ET = " + ET.toFixed(2) + " MeV (all); thermal point.", bx, by + bh + 20 + comps.length * 17 + 18);
          ctx.fillStyle = fg; ctx.fillText("Where one fission's energy goes (MT=458)", 8, 70);
          return "The bar sums the seven components at this energy (" + tot.toFixed(2) + " MeV). EFR, ENP, END and EGP are tabulated against energy in this evaluation (LFC = 1); EB, EGD and ENU are the thermal values.";
        }
        // Spectra: prompt MF=5/MT=18 at this energy, the six delayed groups, and the demo's sampled prompt neutrons.
        var Xe = function (E) { return x0 + Math.min(E / 1e6 / 8, 1) * pw; }, pmax = 0;
        var c = Y.chi; for (var i2 = 0; i2 < c.p.length; i2++) pmax = Math.max(pmax, c.p[i2]);
        P.groups.forEach(function (g) { var area = 0; for (var q = 0; q + 1 < g.e.length; q++) area += g.p[q] * (g.e[q + 1] - g.e[q]); g._n = area; for (q = 0; q < g.p.length; q++) pmax = Math.max(pmax, g.p[q] / area); });
        var Ys = function (v) { return y0 + ph * (1 - v / pmax * zoom); };
        axes(ctx, x0, y0, pw, ph, mut);
        ctx.fillStyle = mut; [0, 2, 4, 6, 8].forEach(function (E) { ctx.fillText(E + " MeV", Xe(E * 1e6) - 12, y0 + ph + 16); });
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        var gc = ["#c6dbef", "#9ecae1", "#6baed6", "#4292c6", "#2171b5", "#08519c"];
        P.groups.forEach(function (g, k) {
          ctx.strokeStyle = gc[k]; ctx.lineWidth = 1.5; ctx.beginPath();
          for (var q = 0; q + 1 < g.e.length; q++) { var yy = Ys(g.p[q] / g._n); ctx.lineTo(Xe(g.e[q]), yy); ctx.lineTo(Xe(g.e[q + 1]), yy); }
          ctx.stroke();
        });
        ctx.strokeStyle = hot; ctx.lineWidth = 2.5; ctx.beginPath();
        for (i2 = 0; i2 < c.e.length; i2++) { var yy4 = Ys(c.p[i2]); if (i2) ctx.lineTo(Xe(c.e[i2]), yy4); else ctx.moveTo(Xe(c.e[i2]), yy4); }
        ctx.stroke();
        if (S.spectrum && S.spectrum.key === S.parent + S.ie) {
          var H = S.spectrum, bwid = 0.25e6;
          ctx.fillStyle = "rgba(47,127,216,0.35)";
          for (var b = 0; b < H.bins.length; b++) { var dens = H.bins[b] / (H.n * bwid); ctx.fillRect(Xe(b * bwid), Ys(dens), Xe((b + 1) * bwid) - Xe(b * bwid) - 1, y0 + ph - Ys(dens)); }
        }
        ctx.restore();
        ctx.fillStyle = fg; ctx.fillText("Neutron spectra per MeV, " + S.parent + ", " + eLabel(Y.e), 8, 66);
        ctx.fillStyle = hot; ctx.fillText("prompt, MF=5/MT=18", 8, 82); ctx.fillStyle = "#2171b5"; ctx.fillText("delayed groups 1-6, MF=5/MT=455", 140, 82);
        ctx.fillStyle = "rgba(47,127,216,0.8)"; ctx.fillText("bars: this demo's prompt neutrons", 8, 97);
        var msg = "The delayed neutrons (six precursor groups, each its own histogram) come out much softer than the prompt ones. Prompt mean (MF=5): " + (c.mean / 1e6).toFixed(3) + " MeV.";
        if (S.spectrum && S.spectrum.key === S.parent + S.ie) msg += " Demo: " + S.spectrum.n + " neutrons from " + S.spectrum.events + " events, mean " + (S.spectrum.sum / S.spectrum.n).toFixed(3) + " MeV" + (S.spectrum.done ? "." : " (sampling…)");
        return msg;
      },
      panel: function (body, redraw) {
        L.buttons(body, "Show", [["yield", "Yield curve"], ["nz", "N/Z"], ["budget", "Energy budget"], ["spectra", "Neutron spectra"]], view, function (v) { view = v; redraw(); });
        var b = el("button", { type: "button", "class": "w-opt wide" }, "Sample 2000 events into the spectrum");
        b.onclick = function () {
          view = "spectra";
          var key = S.parent + S.ie, H = { key: key, bins: new Array(40).fill(0), n: 0, sum: 0, events: 0, done: false }, seed = 100000;
          S.spectrum = H;
          if (sampling) clearTimeout(sampling);
          (function chunk() {
            for (var k = 0; k < 100 && H.events < 2000; k++) {
              var ev = F.event(S.parent, S.ie, { seed: seed++ });
              ev.neutrons.forEach(function (n) { var bi = Math.floor(n.T / 0.25); if (bi < 40) H.bins[bi] += 1; H.n += 1; H.sum += n.T; });
              H.events += 1;
            }
            H.done = H.events >= 2000;
            redraw();
            if (!H.done && S.spectrum === H) sampling = setTimeout(chunk, 0);
          })();
        };
        body.append(b);
        body.append(el("p", { "class": "w-note" }, "Sampling runs in chunks of 100 events between frames, so the page stays responsive."));
      }
    });
    S.listeners.push(function () { frame.redraw(); });
    return frame;
  }

  // ── #lattice: what hits the lattice (illustrative) ─────────────────────
  function lattice(root) {
    var parts = [], atoms = [], foreign = [], NX = 18, NY = 11;
    function build() {
      atoms = [];
      for (var i = 0; i < NX; i++) for (var j = 0; j < NY; j++) atoms.push({ x: i + 0.5 + (j % 2) * 0.5, y: j + 0.5, dx: 0, dy: 0, hit: 0 });
      foreign = []; parts = [];
    }
    build();
    var KIND = {
      fragment: { col: "#d9822b", speed: 2.2, life: 6 },
      neutron: { col: "#2f7fd8", speed: 6, life: 99 },
      gamma: { col: "#8e5bd6", speed: 14, life: 99 },
      beta: { col: "#2a9d55", speed: 5, life: 3 },
      neutrino: { col: "#9aa3ab", speed: 14, life: 99 }
    };
    function send(kind, n) {
      for (var k = 0; k < n; k++) {
        var y = 1 + Math.random() * (NY - 2);
        parts.push({ kind: kind, x: -0.5 - Math.random() * 2, y: y, vx: 1, vy: (Math.random() - 0.5) * 0.3, s: 0, trail: [], sparks: [] });
      }
    }
    function sendEvent() {
      var ev = S.ev, nb = 0;
      if (S.chains) S.chains.forEach(function (c) { c.forEach(function (s) { if (s.to) nb += 1; }); });
      send("fragment", 2);
      send("neutron", ev ? ev.neutrons.length : 2);
      send("gamma", ev ? Math.max(ev.gammas.length, 1) : 2);
      send("beta", nb || 3);
      send("neutrino", nb || 3);
    }
    var frame = new Frame(root, {
      reset: function () { build(); },
      tick: function (dt) {
        parts.forEach(function (p) {
          var K = KIND[p.kind], step = K.speed * dt;
          var n = Math.hypot(p.vx, p.vy); p.vx /= n; p.vy /= n;
          p.x += p.vx * step; p.y += p.vy * step; p.s += step;
          if (p.trail.length === 0 || Math.hypot(p.x - p.trail[p.trail.length - 1][0], p.y - p.trail[p.trail.length - 1][1]) > 0.15) p.trail.push([p.x, p.y]);
          if (p.kind === "fragment" || p.kind === "neutron" || p.kind === "beta" || p.kind === "gamma") {
            atoms.forEach(function (a) {
              var d = Math.hypot(a.x + a.dx - p.x, a.y + a.dy - p.y);
              if (p.kind === "fragment" && d < 0.6 && !p.stopped) { a.dx += (Math.random() - 0.5) * 0.5; a.dy += (Math.random() - 0.5) * 0.5; a.hit = 1; }
              if (p.kind === "neutron" && d < 0.25 && p.cool !== a) { var th = Math.random() * 2 * Math.PI; p.vx = Math.cos(th); p.vy = Math.sin(th); a.hit = 1; a.dx += 0.08 * Math.cos(th + Math.PI); p.cool = a; }
              if (p.kind === "beta" && d < 0.45 && Math.random() < 0.2) { p.vx += (Math.random() - 0.5) * 0.8; p.vy += (Math.random() - 0.5) * 0.8; p.sparks.push([a.x, a.y, 1]); }
              if (p.kind === "gamma" && d < 0.3 && Math.random() < 0.02) p.sparks.push([a.x, a.y, 1]);
            });
          }
          if (p.kind === "fragment" && p.s > K.life && !p.stopped) { p.stopped = true; foreign.push([p.x, p.y]); }
          if (p.kind === "fragment" && p.stopped) { p.x -= p.vx * step; p.y -= p.vy * step; }
          if (p.kind === "beta" && p.s > K.life) p.dead = true;
          if (p.x > NX + 2 || p.x < -4 || p.y < -2 || p.y > NY + 2) p.dead = true;
          p.sparks.forEach(function (s) { s[2] -= dt; });
        });
        parts = parts.filter(function (p) { return !p.dead || p.kind === "fragment"; });
        atoms.forEach(function (a) { a.hit = Math.max(0, a.hit - dt); });
        return parts.some(function (p) { return !p.dead && !p.stopped; });
      },
      draw: function (ctx, w, h, zoom) {
        var mut = css("--muted", "#5b636b"), fg = css("--fg", "#1d2125");
        var sc = Math.min((w - 20) / (NX + 1), (h - 104) / (NY + 1)) * zoom, ox = 10, oy = 94;
        var X = function (x) { return ox + x * sc; }, Y = function (y) { return oy + y * sc; };
        atoms.forEach(function (a) {
          ctx.fillStyle = a.hit > 0 ? "#e04646" : (Math.hypot(a.dx, a.dy) > 0.15 ? "#c27c6a" : "rgba(128,128,128,0.55)");
          ctx.beginPath(); ctx.arc(X(a.x + a.dx), Y(a.y + a.dy), Math.max(sc * 0.18, 2), 0, 2 * Math.PI); ctx.fill();
        });
        foreign.forEach(function (f) { ctx.fillStyle = "#d9822b"; ctx.strokeStyle = fg; ctx.lineWidth = 1.5; ctx.beginPath(); ctx.arc(X(f[0]), Y(f[1]), Math.max(sc * 0.28, 3), 0, 2 * Math.PI); ctx.fill(); ctx.stroke(); });
        parts.forEach(function (p) {
          var K = KIND[p.kind];
          ctx.strokeStyle = K.col; ctx.lineWidth = p.kind === "fragment" ? 4 : 1.5;
          if (p.kind === "neutrino") ctx.setLineDash([4, 4]);
          ctx.beginPath(); p.trail.forEach(function (t, i) { if (i) ctx.lineTo(X(t[0]), Y(t[1])); else ctx.moveTo(X(t[0]), Y(t[1])); }); ctx.stroke(); ctx.setLineDash([]);
          if (!p.dead) { ctx.fillStyle = K.col; ctx.beginPath(); ctx.arc(X(p.x), Y(p.y), p.kind === "fragment" ? 6 : 3.5, 0, 2 * Math.PI); ctx.fill(); }
          p.sparks.forEach(function (s) { if (s[2] > 0) { ctx.fillStyle = "rgba(230,200,0," + s[2] + ")"; ctx.beginPath(); ctx.arc(X(s[0]) + 6, Y(s[1]) - 6, 2.5, 0, 2 * Math.PI); ctx.fill(); } });
        });
        ctx.font = "12px system-ui, sans-serif"; ctx.fillStyle = mut;
        ctx.fillText("Illustrative, not to scale: a grid of atoms.", 10, 66);
        var legend = [["fragment", "fragment"], ["neutron", "neutron"], ["gamma", "gamma"], ["beta", "beta"], ["neutrino", "neutrino"]], lx = 10;
        var ly = 76;
        legend.forEach(function (l) { var wl = ctx.measureText(l[1]).width + 28; if (lx + wl > w) { lx = 10; ly += 14; } ctx.fillStyle = KIND[l[0]].col; ctx.fillRect(lx, ly, 10, 10); ctx.fillStyle = fg; ctx.fillText(l[1], lx + 13, ly + 9); lx += wl; });
        return "Fragment: a thick track that disorders the atoms it passes, then stops and stays as a new, foreign atom (orange ring). Neutron: bounces off nuclei. Gamma: passes, now and then knocks out an electron (yellow). Beta: a short, wandering, ionising track. Neutrino: leaves untouched. What each does is cited in the table below the picture; the picture itself is illustrative.";
      },
      panel: function (body) {
        var row = el("div", { "class": "w-row" });
        [["Whole fission", function () { sendEvent(); }], ["Fragment", function () { send("fragment", 1); }], ["Neutron", function () { send("neutron", 1); }], ["Gamma", function () { send("gamma", 1); }], ["Beta", function () { send("beta", 1); }], ["Neutrino", function () { send("neutrino", 1); }]].forEach(function (b) {
          var x = el("button", { type: "button", "class": "w-opt" }, b[0]); x.onclick = function () { b[1](); frame.play(); }; row.append(x);
        });
        var clr = el("button", { type: "button", "class": "w-opt" }, "Clear"); clr.onclick = function () { build(); frame.redraw(); }; row.append(clr);
        body.append(row);
        body.append(el("p", { "class": "w-note" }, "“Whole fission” sends the particle counts of the last event in the demo above (betas and neutrinos: one each per beta decay in its chains)."));
      }
    });
    return frame;
  }

  // The "What hits the lattice" table's energy shares, from the data: MeV
  // and per cent of ET, U-235 at thermal energy.
  function fillShares() {
    var P = D().parents["U-235"], e = P.yields[0].e, ET = F.mt458(P, 9, e);
    document.querySelectorAll("[data-mt458]").forEach(function (n) {
      var ks = n.getAttribute("data-mt458").split("+"), v = 0;
      ks.forEach(function (k) { v += F.mt458(P, +k, e); });
      n.textContent = v.toFixed(2) + " MeV" + (n.hasAttribute("data-pct") ? " (" + (100 * v / ET).toFixed(1) + " % of ET)" : "");
    });
    // The other numbers the prose quotes, each computed here from the data
    // by the functions check-fission.js checks.
    var Y = P.yields[0], sa = 0, k;
    for (k = 0; k < Y.a.length; k++) sa += Y.y[k] * Y.a[k];
    var nuY = P.a + 1 - sa, nuD = F.delayedPerFission("U-235", 0);
    var Ifor = F.initial(P, e), Ytop = P.mt458.tab["1"];
    var vals = {
      nuYields: nuY.toFixed(3), nup: Y.nup.toFixed(3),
      nuDelayed: nuD.toFixed(4), nud: Y.nud.toFixed(4), nuDelayedMiss: (100 * (nuD / Y.nud - 1)).toFixed(0) + " %",
      nzCN: ((P.a + 1 - P.z) / P.z).toFixed(3), snTable: Ifor.Sn.toFixed(3) + " MeV", snEval: (P.sn / 1e6).toFixed(3) + " MeV",
      efrTop: (Ytop[1][Ytop[1].length - 1] / 1e6).toFixed(2) + " MeV", efrTopE: L.fmtE(Ytop[0][Ytop[0].length - 1]),
      efrThermal: F.mt458(P, 1, e).toFixed(2) + " MeV"
    };
    function fill() {
      document.querySelectorAll("[data-chk]").forEach(function (n) {
        var v = vals[n.getAttribute("data-chk")];
        if (v !== undefined) n.textContent = v;
      });
    }
    fill();
    // The pair construction's mean needs the pair table: built in slices.
    F.preparePairs("U-235", 0, false, function () {
      var nuP = F.pairs("U-235", 0, false).meanNu;
      vals.nuPairs = nuP.toFixed(2); vals.nuPairsMiss = (100 * (1 - nuP / Y.nup)).toFixed(0) + " %";
      fill();
    }, SLICE_LATER, 120);
  }

  L.widget("onefission", oneFission);
  L.widget("fpcharts", charts);
  L.widget("lattice", lattice);
  L.widget("fp-shares", function () { fillShares(); });
  window.OneFission = { state: S };
})();
