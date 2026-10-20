// Fission products, lesson 2: the widget. #splits draws fission after
// fission from the evaluated yields (split-physics.js) into a histogram of
// mass number, with the exact yield curve as an optional overlay, and the
// last split drawn above it. Uses ../neutronics/lesson-kit.js unchanged.
//
// No lag: a batch of fissions is added a few dozen per frame (each draw is a
// binary search), so even 1000 at once animates and the page stays
// responsive. Changing the energy rebuilds the sampler once (about a
// thousand interpolated yields).
(function () {
  "use strict";
  var L = window.Lesson, S = window.SplitPhysics, FE = window.FPEnergy;
  var css = L.css, el = L.el;
  var D = function () { return window.YIELDS_DATA; };
  var SYM = ["n","H","He","Li","Be","B","C","N","O","F","Ne","Na","Mg","Al","Si","P","S","Cl","Ar","K","Ca","Sc","Ti","V","Cr","Mn","Fe","Co","Ni","Cu","Zn","Ga","Ge","As","Se","Br","Kr","Rb","Sr","Y","Zr","Nb","Mo","Tc","Ru","Rh","Pd","Ag","Cd","In","Sn","Sb","Te","I","Xe","Cs","Ba","La","Ce","Pr","Nd","Pm","Sm","Eu","Gd","Tb","Dy","Ho","Er","Tm","Yb","Lu","Hf","Ta","W","Re","Os","Ir","Pt","Au","Hg","Tl","Pb","Bi","Po","At","Rn","Fr","Ra","Ac","Th","Pa","U","Np","Pu"];
  function nm(z, a) { return SYM[z] + "-" + a; }
  var HI = 1.4e7; // the highest tabulated energy of every parent [eV]

  function splits(root) {
    var st = {
      parent: "U-235", e: 0.0253, hist: {}, n: 0, pending: 0, last: null, overlay: true, logy: false,
      smp: null, r: S.rng(20261020), lastAge: 0
    };
    function lo() { return S.energies(st.parent)[0]; }
    st.e = FE.clamp(FE.initial(0.0253), lo(), HI);
    function rebuild() { st.smp = S.sampler(st.parent, st.e); }
    function reset() { st.hist = {}; st.n = 0; st.pending = 0; st.last = null; }
    rebuild();

    var frame = new L.Frame(root, {
      reset: function () {},
      tick: function (dt) {
        st.lastAge += dt;
        if (st.pending > 0) {
          var k = Math.min(st.pending, Math.max(1, Math.ceil(st.pending / 20)), 60);
          for (var i = 0; i < k; i++) {
            var p = S.samplePair(st.smp, st.r);
            if (!p) continue;
            st.hist[p.a1] = (st.hist[p.a1] || 0) + 1;
            st.hist[p.a2] = (st.hist[p.a2] || 0) + 1;
            st.n += 1; st.last = p; st.lastAge = 0;
          }
          st.pending -= k;
        }
        return st.pending > 0 || st.lastAge < 1.2;
      },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), mut = css("--muted", "#5b636b"), acc = css("--accent", "#1f6f78");
        var P = D()[st.parent], acn = P.a + 1;
        ctx.font = "12px system-ui, sans-serif";
        // The last split, as a strip across the top.
        // The first ~56 px are under the button bar: the strip starts below it.
        var top = 124;
        if (st.last) {
          var p = st.last, cx = w / 2, cy = 92, sep = Math.min(w * 0.32, 220) * Math.min(1, 0.35 + st.lastAge * 1.5);
          var r1 = 7 + 6 * Math.cbrt(p.a1 / 100), r2 = 7 + 6 * Math.cbrt(p.a2 / 100);
          ctx.fillStyle = "#d9822b"; ctx.beginPath(); ctx.arc(cx - sep / 2, cy, r1, 0, 2 * Math.PI); ctx.fill();
          ctx.fillStyle = "#b5452f"; ctx.beginPath(); ctx.arc(cx + sep / 2, cy, r2, 0, 2 * Math.PI); ctx.fill();
          ctx.fillStyle = fg; ctx.textAlign = "center";
          ctx.fillText(nm(p.z1, p.a1), cx - sep / 2, cy + r1 + 13);
          ctx.fillText(nm(p.z2, p.a2), cx + sep / 2, cy + r2 + 13);
          ctx.fillStyle = mut;
          ctx.fillText(p.z1 + " + " + p.z2 + " = " + P.z + " protons; " + p.a1 + " + " + p.a2 + " + " + p.nu + " n = " + acn, cx, 70);
          ctx.textAlign = "left";
        } else {
          ctx.fillStyle = mut; ctx.fillText("Press a fission button (Controls):", 10, 76); ctx.fillText("each fission adds its two products below.", 10, 92);
        }
        // The histogram of A, with the exact yield curve (expected counts N·Y(A)).
        var x0 = 46, y0 = top + 8, pw = w - x0 - 12, ph = h - y0 - 34;
        var aLo = 66, aHi = 176, X = function (a) { return x0 + (a - aLo) / (aHi - aLo + 1) * pw; };
        var zy = st.smp.zy, m = S.massYields(zy), ymax = 0, k;
        for (k = 0; k < m.y.length; k++) ymax = Math.max(ymax, m.y[k]);
        var cmax = 0; for (var a in st.hist) cmax = Math.max(cmax, st.hist[a]);
        var top_ = Math.max(st.n * ymax * 1.15, cmax * 1.1, 1) / zoom;
        var Y = st.logy
          ? function (c) { var lmin = Math.log10(0.5), lmax = Math.log10(Math.max(top_, 10)); return y0 + ph * (1 - (Math.log10(Math.max(c, 0.5)) - lmin) / (lmax - lmin)); }
          : function (c) { return y0 + ph * (1 - c / top_); };
        ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        var bw = pw / (aHi - aLo + 1);
        ctx.fillStyle = "rgba(47,127,216,0.55)";
        for (a in st.hist) { var c = st.hist[a]; ctx.fillRect(X(+a), Y(c), Math.max(bw - 1, 1), y0 + ph - Y(c)); }
        if (st.overlay && st.n > 0) {
          ctx.strokeStyle = "#b85c38"; ctx.lineWidth = 2; ctx.beginPath();
          var first = true;
          for (k = 0; k < m.y.length; k++) {
            var aa = m.a0 + k; if (aa < aLo || aa > aHi) continue;
            var yy = Y(st.n * m.y[k]);
            if (first) { ctx.moveTo(X(aa) + bw / 2, yy); first = false; } else ctx.lineTo(X(aa) + bw / 2, yy);
          }
          ctx.stroke();
        }
        ctx.restore();
        ctx.fillStyle = mut;
        [70, 90, 110, 130, 150, 170].forEach(function (t) { ctx.fillText(t, X(t) - 8, y0 + ph + 14); });
        ctx.fillText("mass number A", x0 + pw - 90, y0 + ph + 28);
        ctx.save(); ctx.translate(12, y0 + ph / 2 + 30); ctx.rotate(-Math.PI / 2); ctx.fillText(st.logy ? "count (log)" : "count", 0, 0); ctx.restore();
        if (st.overlay) { ctx.fillStyle = "#b85c38"; ctx.fillText("— exact: N × Y(A)", x0 + 6, y0 + 14); }
        var pv = S.peakValley(m, acn / 2);
        return st.parent + " at " + FE.label(st.e) + ": " + st.n + " fissions, " + 2 * st.n + " products. The evaluated curve here: peaks at A = " + pv.light[0] + " and " + pv.heavy[0] +
          ", valley at A = " + pv.valley[0] + ", peak-to-valley " + (pv.ratio >= 100 ? pv.ratio.toFixed(0) : pv.ratio.toFixed(1)) + ".";
      },
      panel: function (body, redraw, play) {
        L.buttons(body, "Parent nucleus", Object.keys(D()).map(function (k) { return [k, k + " + n"]; }), st.parent, function (v) {
          st.parent = v; st.e = FE.clamp(st.e, lo(), HI); syncSlider(); rebuild(); reset(); redraw();
        });
        var out = el("span", { "class": "w-val" });
        var g = el("label", { "class": "w-group" });
        g.append(el("span", { "class": "w-label" }, "Incident neutron energy"), out);
        var s = el("input", { type: "range", min: 0, max: FE.STEPS, step: 1 });
        g.append(s); body.append(g);
        var marks = el("div", { "class": "w-row" });
        body.append(el("span", { "class": "w-label" }, "Tabulated energies (the evaluation's own):"), marks);
        function syncSlider() {
          s.value = FE.toPos(st.e, lo(), HI);
          out.textContent = FE.label(st.e);
          marks.innerHTML = "";
          S.energies(st.parent).forEach(function (e) {
            var b = el("button", { type: "button", "class": "w-opt" }, FE.label(e));
            if (Math.abs(e / st.e - 1) < 1e-9) b.classList.add("on");
            b.onclick = function () { setE(e); };
            marks.append(b);
          });
          link.href = "prompt-spectrum.html?e=" + st.e;
        }
        function setE(e) { st.e = e; FE.remember(e); syncSlider(); rebuild(); reset(); redraw(); }
        s.oninput = function () { setE(FE.toEnergy(+s.value, lo(), HI)); };
        var row = el("div", { "class": "w-row" });
        [1, 10, 100, 1000].forEach(function (n) {
          var b = el("button", { type: "button", "class": "w-opt" }, n === 1 ? "1 fission" : n + " fissions");
          b.onclick = function () { st.pending += n; play(); };
          row.append(b);
        });
        var rs = el("button", { type: "button", "class": "w-opt" }, "Reset");
        rs.onclick = function () { reset(); redraw(); };
        row.append(rs);
        body.append(el("span", { "class": "w-label" }, "Fissions"), row);
        L.toggle(body, "Overlay the exact yield curve", st.overlay, function (v) { st.overlay = v; redraw(); });
        L.toggle(body, "Logarithmic count axis", st.logy, function (v) { st.logy = v; redraw(); });
        var link = el("a", { href: "prompt-spectrum.html" }, "The prompt neutrons at this energy (lesson 3) →");
        body.append(el("p", { "class": "w-note" }, "Between the tabulated energies the yields are interpolated linearly in energy, as fission-yields-data does: interpolation, not physics."), link);
        syncSlider();
      }
    });
    return frame;
  }

  // The peak-to-valley table, filled from the generated data.
  function pvTable(node) {
    var body = node.querySelector("tbody"), Dd = D();
    Object.keys(Dd).forEach(function (name) {
      Dd[name].tables.forEach(function (t) {
        var tr = el("tr");
        tr.append(el("td", { "data-label": "Parent" }, name), el("td", { "data-label": "Neutron energy" }, FE.label(t.e)),
          el("td", { "data-label": "Light peak" }, "A = " + t.pv.light[0]), el("td", { "data-label": "Heavy peak" }, "A = " + t.pv.heavy[0]),
          el("td", { "data-label": "Valley" }, "A = " + t.pv.valley[0]), el("td", { "data-label": "Peak-to-valley" }, t.pv.ratio >= 100 ? t.pv.ratio.toFixed(0) : t.pv.ratio.toFixed(1)));
        body.append(tr);
      });
    });
  }

  // The advanced section's two measures, computed here from the data.
  function fillChecks() {
    var a = document.querySelector('[data-chk="constructionDev"]'), b = document.querySelector('[data-chk="chargeAsym"]');
    if (a) a.textContent = (100 * S.constructionDeviation("U-235", 0.0253)).toFixed(1) + " %";
    if (b) b.textContent = S.chargeAsymmetry("U-235", 0.0253).toFixed(3);
  }

  L.widget("splits", splits);
  L.widget("pv-table", function (node) { pvTable(node); fillChecks(); });
})();
