// Fission products, chapter 5: the widget of "Breathing: how the release
// leaves the vessel" (atops-breathing.html). Data: breathing-data.js,
// RECORDED by crates/dhoby-ghaut/examples/lesson_breathing (the TRISO-ATOPS
// port's release and vented fraction, and a well-mixed gas stepped by
// boon-lay's live_pools). No physics here: recorded curves and look-ups.
(function () {
  "use strict";
  var L = window.Lesson, css = L.css, el = L.el;
  var LP = window.LessonPlot, sci = LP.sci, Box = LP.Box, linTicks = LP.linTicks, decTicks = LP.decTicks, legend = LP.legend;
  var BD = function () { return window.BREATHING_DATA; };
  var COL = { t: "#7f8c8d", r: "#1d2125", atops: "#c0392b", mixed: "#2a9d55", nodecay: "#2f7fd8", vent: "#d4a017" };
  var TOP = 58;
  function find(name, peak) { return BD().cases.filter(function (c) { return c.name === name && c.peak === peak; })[0]; }
  function i100(c) { return c.hours.indexOf(100); }

  function breathe(root) {
    var st = { n: "Cs-137", p: 1600, log: true };
    return new L.Frame(root, {
      reset: function () {}, tick: function () { return false; },
      draw: function (ctx, w, h) {
        var c = find(st.n, st.p);
        var items = [["released from the fuel, R(t)", COL.r], ["TRISO-ATOPS: frac(t) × R(t)", COL.atops], ["well-mixed gas", COL.mixed], ["well-mixed gas, no decay", COL.nodecay, [5, 3]], ["gas pushed out (ideal gas)", COL.vent, [2, 3]]];
        var top = legend(ctx, items, 8, TOP, w - 16) + 6;
        var lo = st.log ? -6 : 0, B = new Box(58, top, w - 12, h - 40, [0, 300], st.log ? [lo, 0] : [0, 1], false, st.log);
        B.frame(ctx, "time (h): heat-up, hold, cool-down", "fraction of the kernel inventory", linTicks(0, 300, 50), st.log ? decTicks(lo, 0, 1) : linTicks(0, 1, 0.25, function (v) { return v.toFixed(2); }));
        // The hold, shaded.
        ctx.fillStyle = "rgba(127,140,141,0.12)"; ctx.fillRect(B.X(50), B.y0, B.X(100) - B.X(50), B.y1 - B.y0);
        B.line(ctx, c.hours, c.release, COL.r, 1.5);
        B.line(ctx, c.hours, c.ventExact, COL.vent, 1.5, [2, 3]);
        B.line(ctx, c.hours, c.atops, COL.atops, 2.5);
        B.line(ctx, c.hours, c.mixedNoDecay, COL.nodecay, 1.5, [5, 3]);
        B.line(ctx, c.hours, c.mixed, COL.mixed, 2.5);
        var k = i100(c);
        return st.n + ", peak " + st.p + " °C. By the end of the hold (100 h, shaded 50-100 h) the fuel has released " + sci(c.release[k], 3) + "; TRISO-ATOPS carries out " + sci(c.atops[k], 3) + ", a well-mixed gas " + sci(c.mixed[k], 3) + " (" + (c.mixed[k] / c.atops[k]).toFixed(2) + " of it). Nothing leaves while the core cools.";
      },
      panel: function (body, redraw) {
        L.buttons(body, "Nuclide", ["Cs-137", "Sr-90", "I-131", "Kr-88"].map(function (n) { return [n, n]; }), "Cs-137", function (v) { st.n = v; redraw(); });
        L.buttons(body, "Peak", BD().peaks.map(function (p) { return [p, p + " °C"]; }), 1600, function (v) { st.p = v; redraw(); });
        L.toggle(body, "Log scale", true, function (on) { st.log = on; redraw(); });
        body.append(el("p", { "class": "w-note" }, "The same release R(t) and the same temperatures on both sides; only how the gas carries it out differs. An illustrative heat-up on one MHTGR Case A node, not a plant calculation."));
      }
    });
  }

  function fillBr() {
    var m = {};
    BD().cases.forEach(function (c) {
      var k = i100(c), key = c.name.replace("-", "") + c.peak;
      m["R" + key] = sci(c.release[k], 3);
      m["A" + key] = sci(c.atops[k], 3);
      m["M" + key] = sci(c.mixed[k], 3);
      m["x" + key] = (c.mixed[k] / c.atops[k]).toFixed(2);
      m["d" + key] = (c.mixed[k] / c.mixedNoDecay[k]).toFixed(2);
    });
    var cs = find("Cs-137", 1600);
    m.vent5 = cs.vent[1].toFixed(4);
    m.vent5exact = cs.ventExact[1].toFixed(4);
    m.vent100 = cs.vent[i100(cs)].toFixed(3);
    m.vent100exact = cs.ventExact[i100(cs)].toFixed(3);
    var r = BD().cases.map(function (c) { return c.atops[i100(c)] / c.mixed[i100(c)]; });
    m.foldLo = Math.min.apply(null, r).toFixed(1);
    m.foldHi = Math.round(Math.max.apply(null, r));
    document.querySelectorAll("[data-br]").forEach(function (n) { var k = n.getAttribute("data-br"); if (k in m) n.textContent = m[k]; });
  }
  L.widget("br-numbers", fillBr);
  L.widget("br-breathe", breathe);
})();
