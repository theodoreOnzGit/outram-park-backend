// Fission products, the diffusion arc (gh:#1209): the widgets of "The
// diffusion equation, three ways" (diffusion-three-ways.html) and "Is
// TRISO-ATOPS's analytical solution conservative?" (atops-conservative.html).
// Data: diffarc-data.js, RECORDED by crates/dhoby-ghaut/examples/
// lesson_diffusion_arc (boon-lay's ported Booth formulas and its multilayer
// finite volumes, NOT a port), and decaywalk-data.js for the walkers. No
// physics here: the widgets draw recorded curves and look numbers up.
(function () {
  "use strict";
  var L = window.Lesson, css = L.css, el = L.el;
  var LP = window.LessonPlot, sci = LP.sci, Box = LP.Box, linTicks = LP.linTicks, decTicks = LP.decTicks, legend = LP.legend;
  var DA = function () { return window.DIFFARC_DATA; };
  var COL = { crank: "#1d2125", fv: "#2f7fd8", wos: "#b85c38", atops: "#c0392b", bare: "#d4a017", intact: "#2a9d55", mixed: "#7a4cc2" };
  function fg() { return css("--fg", "#1d2125"); }
  function mut() { return css("--muted", "#5b636b"); }
  function pct(x) { return (100 * x).toPrecision(3) + " %"; }
  // Below the on-screen + / − / Reset buttons (36 px, top right).
  var TOP = 58;
  function f1(v) { return v.toFixed(1); }
  function f2(v) { return v.toFixed(2); }
  function caseOf(el, t) { return DA().b.cases.filter(function (c) { return c.el === el && c.t === t; })[0]; }

  // ─── One sphere: Crank, finite volumes, walkers ──────────────────────
  function mesh(root) {
    var st = { k: 3, walk: true };
    return new L.Frame(root, {
      reset: function () {}, tick: function () { return false; },
      draw: function (ctx, w, h) {
        var A = DA().a, fv = A.fv[st.k], W = window.DECAYWALK_DATA;
        var items = [["Crank's series", COL.crank], ["finite volumes, " + fv.cells + " cells", COL.fv]];
        if (st.walk && W) items.push(["10 000 walkers (Walk-on-Spheres)", COL.wos]);
        // Legend first, below the zoom buttons; the plot under it.
        var top = legend(ctx, items, 8, TOP, w - 16) + 6;
        var B = new Box(58, top, w - 12, h - 40, [0, 200], [0, 0.6], false, false);
        B.frame(ctx, "time at 1200 °C (h)", "fraction released", linTicks(0, 200, 50), linTicks(0, 0.6, 0.1, f1));
        B.line(ctx, A.hours, A.crank, COL.crank, 2);
        ctx.fillStyle = COL.fv;
        A.hours.forEach(function (x, i) { ctx.fillRect(B.X(x) - 2.5, B.Y(fv.frac[i]) - 2.5, 5, 5); });
        if (st.walk && W) {
          var R = W.walk.release[0];
          ctx.fillStyle = COL.wos;
          R.hours.forEach(function (x, i) { ctx.beginPath(); ctx.arc(B.X(x), B.Y(R.wos[i]), 2.5, 0, 2 * Math.PI); ctx.fill(); });
        }
        return "CRP-6 Case 1a: Cs-137 in a 212.5 µm kernel at 1200 °C. " + fv.cells + " cells: worst gap to Crank " + sci(fv.worst, 2) + ", at 200 h " + sci(fv.end, 2) + ".";
      },
      panel: function (body, redraw) {
        L.buttons(body, "Cells", DA().a.fv.map(function (f, k) { return [k, String(f.cells)]; }), 3, function (v) { st.k = v; redraw(); });
        L.toggle(body, "Walkers", true, function (on) { st.walk = on; redraw(); });
        body.append(el("p", { "class": "w-note" }, "Five cells already have the shape; each halving of the cell size cuts the error about four times. The walkers are the lesson Walking on spheres' recorded run of the same case."));
      }
    });
  }

  // ─── Inside the sphere: the profile ──────────────────────────────────
  function profile(root) {
    var st = { k: 0 };
    return new L.Frame(root, {
      reset: function () {}, tick: function () { return false; },
      draw: function (ctx, w, h) {
        var P = DA().a.profiles[st.k];
        var top = legend(ctx, [["exact series (Crank eq. 6.18)", COL.crank], ["finite volumes, 80 cells", COL.fv]], 8, TOP, w - 16) + 6;
        var B = new Box(58, top, w - 12, h - 40, [0, 1], [0, 1.05], false, false);
        B.frame(ctx, "r / a (centre to surface)", "c / c₀", linTicks(0, 1, 0.25, f2), linTicks(0, 1, 0.25, f2));
        B.line(ctx, P.x, P.exact, COL.crank, 2);
        ctx.fillStyle = COL.fv;
        P.x.forEach(function (x, i) { if (i % 2 === 0) ctx.fillRect(B.X(x) - 2, B.Y(P.fv[i]) - 2, 4, 4); });
        return "Dt/a² = " + P.tau + ": the concentration falls to zero at the surface and is still flat in the middle. Worst gap " + sci(P.worst, 2) + " of c₀.";
      },
      panel: function (body, redraw) {
        L.buttons(body, "Dt/a²", DA().a.profiles.map(function (p, k) { return [k, String(p.tau)]; }), 0, function (v) { st.k = v; redraw(); });
      }
    });
  }

  // ─── TRISO-ATOPS's formula against the whole particle ────────────────
  function compare(root) {
    var st = { el: "Cs", t: 1600 };
    return new L.Frame(root, {
      reset: function () {}, tick: function () { return false; },
      draw: function (ctx, w, h) {
        var C = caseOf(st.el, st.t), H = DA().b.hours, ag = st.el === "Ag";
        var items = ag
          ? [["TRISO-ATOPS breakthrough (every particle)", COL.atops], ["mesh: well mixed inside the SiC", COL.mixed, [6, 3]], ["mesh: intact particle, five layers", COL.intact]]
          : [["TRISO-ATOPS Booth, failed particle", COL.atops], ["mesh: bare kernel", COL.bare], ["mesh: intact particle (TRISO-ATOPS: 0)", COL.intact]];
        var top = legend(ctx, items, 8, TOP, w - 16) + 6;
        var B = new Box(58, top, w - 12, h - 40, [0, 500], [-8, 0], false, true);
        B.frame(ctx, "time at " + st.t + " °C (h)", "fraction released", linTicks(0, 500, 100), decTicks(-8, 0, 2));
        if (ag) {
          B.line(ctx, H, C.atops, COL.atops, 2.5);
          B.line(ctx, H, C.mixed, COL.mixed, 2, [6, 3]);
          B.line(ctx, H, C.intact, COL.intact, 2);
        } else {
          B.line(ctx, H, C.atops, COL.atops, 2.5);
          ctx.fillStyle = COL.bare;
          H.forEach(function (x, i) { if (C.bare[i] > 0) { ctx.fillRect(B.X(x) - 2, B.Y(C.bare[i]) - 2, 4, 4); } });
          B.line(ctx, H, C.intact, COL.intact, 2);
        }
        var end = C.intact[C.intact.length - 1];
        return st.el + " at " + st.t + " °C, 500 h: intact particle " + sci(end, 2) + (st.el === "Ag" ? "; TRISO-ATOPS " + sci(C.atops[C.atops.length - 1], 3) : "; TRISO-ATOPS for a failed particle " + sci(C.atops[C.atops.length - 1], 3) + ", for an intact one 0") + ". Values below 1e-8 are off the bottom of the plot.";
      },
      panel: function (body, redraw) {
        L.buttons(body, "Element", [["Cs", "Cs"], ["Sr", "Sr"], ["Ag", "Ag"]], "Cs", function (v) { st.el = v; redraw(); });
        L.buttons(body, "Hold", DA().b.holds.map(function (t) { return [t, t + " °C"]; }), 1600, function (v) { st.t = v; redraw(); });
        body.append(el("p", { "class": "w-note" }, "Same D in every layer on both sides; the difference is only the formula's assumptions. Whole inventory starts in the kernel; 500 h isothermal; no decay."));
      }
    });
  }

  // Numbers in the prose, bound by data-da="...".
  function fillDa() {
    var A = DA().a, B = DA().b, m = {};
    m.d1200 = sci(A.d, 4) + " m²/s";
    A.fv.forEach(function (f) { m["fvEnd" + f.cells] = sci(f.end, 2); m["fvWorst" + f.cells] = sci(f.worst, 2); });
    m.crank200 = A.crank[A.crank.length - 1].toFixed(4);
    A.profiles.forEach(function (p, k) { m["prof" + k] = sci(p.worst, 2); });
    var worstBare = 0, worstRefine = 0;
    B.cases.forEach(function (c) {
      var key = c.el + c.t, n = c.intact.length - 1;
      m["intact" + key] = sci(c.intact[n], 2);
      m["atops" + key] = sci(c.atops[n], 3);
      if (c.bare) { m["bare" + key] = sci(c.bare[n], 3); m["bareWorst" + key] = sci(c.bareWorst, 2); worstBare = Math.max(worstBare, c.bareWorst); }
      if (c.mixed) { m["mixed" + key] = sci(c.mixed[n], 3); m["ratio" + key] = (c.atops[n] / c.intact[n]).toFixed(2); }
      worstRefine = Math.max(worstRefine, c.refineEnd);
    });
    m.bareWorst = sci(worstBare, 2);
    m.refineEnd = sci(worstRefine, 2);
    m.csOver1e4 = Math.round(caseOf("Cs", 1600).intact[50] / 1e-4);
    var cs = caseOf("Cs", 1600).d, sr = caseOf("Sr", 1400).d;
    m.dCsSic1600 = sci(cs[3], 2) + " m²/s";
    m.dSrSic1400 = sci(sr[3], 2) + " m²/s";
    m.dSrK1400 = sci(sr[0], 2) + " m²/s";
    document.querySelectorAll("[data-da]").forEach(function (n) { var k = n.getAttribute("data-da"); if (k in m) n.textContent = m[k]; });
  }
  L.widget("da-numbers", fillDa);
  L.widget("da-mesh", mesh);
  L.widget("da-profile", profile);
  L.widget("da-compare", compare);
})();
