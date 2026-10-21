// Neutronics lessons, page 20 (gh:#906 design, 2026-10-16): "But the atoms
// are bound!". One nucleus, free or held in a toy 2D lattice by springs,
// struck by a neutron of energy E. The comparison that decides whether the
// binding matters: the most energy a collision can hand a free atom of mass
// ratio A, 4A/(A+1)^2 E (page 2's kinematics), against the energies of the
// bound atoms' vibrations, read from the alpha-quartz LEAPR deck in
// bound-data.js (its spectrum rho runs to delta*(n-1)). The springs and the
// equally spaced ladder are illustrations, not a model of quartz.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame;
  var D = window.BOUND_DATA;

  // The most energy a head-on elastic collision gives a free atom at rest.
  function maxTransfer(E, A) { return 4 * A / ((A + 1) * (A + 1)) * E; }
  // The top of a deck's continuous vibration spectrum [eV], and the energy
  // at which its rho peaks.
  function spectrum(deck) {
    var n = deck.rho.length, peak = 0;
    for (var i = 1; i < n; i++) if (deck.rho[i] > deck.rho[peak]) peak = i;
    return { top: deck.delta * (n - 1), peak: deck.delta * peak };
  }
  L.boundMath = { maxTransfer: maxTransfer, spectrum: spectrum };

  function boundWidget(root) {
    var Q = D.decks["Si in alpha-quartz"], A = Q.awr, sp = spectrum(Q);
    var st = { bound: true, lg: Math.log10(2e6), t: -1.2 };
    var frame = new Frame(root, {
      reset: function () { st.t = -1.2; },
      onSeen: function () { st.t = -1.2; },
      tick: function (dt) { st.t += dt; if (st.t > 3) { st.t = 3; return false; } return true; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var E = Math.pow(10, st.lg), T = maxTransfer(E, A), ratio = T / sp.top, freeLike = !st.bound || ratio > 30;
        var cx = w * 0.42, cy = 84 + (h - 140) / 2, a = Math.min(w * 0.13, (h - 160) / 4) * zoom, t = st.t;
        // Recoil: a free (or effectively free) atom flies off; a bound one wobbles.
        var off = t > 0 ? (freeLike ? Math.min(t, 2.2) * a * 0.9 : Math.sin(t * 9) * a * 0.12) : 0;
        if (st.bound) {
          ctx.strokeStyle = mut; ctx.lineWidth = 1;
          for (var i = -2; i <= 2; i++) for (var j = -2; j <= 2; j++) {
            var x = cx + i * a, y = cy + j * a;
            if (i === 0 && j === 0) continue;
            if (i < 2) spring(ctx, x, y, x + a, y, i === -1 && j === 0 ? off : 0, freeLike && t > 0.3 && i === -1 && j === 0);
            if (j < 2) spring(ctx, x, y, x, y + a, 0, false);
          }
          // Springs on the struck atom, stretched by its displacement.
          spring(ctx, cx + off, cy, cx + a, cy, 0, freeLike && t > 0.3);
          spring(ctx, cx, cy - a, cx + off, cy, 0, freeLike && t > 0.3);
          spring(ctx, cx + off, cy, cx, cy + a, 0, freeLike && t > 0.3);
          ctx.fillStyle = "#9aa5ad";
          for (i = -2; i <= 2; i++) for (j = -2; j <= 2; j++) if (i || j) { ctx.beginPath(); ctx.arc(cx + i * a, cy + j * a, a * 0.16, 0, 2 * Math.PI); ctx.fill(); }
        }
        ctx.fillStyle = hot; ctx.beginPath(); ctx.arc(cx + off, cy, a * 0.2, 0, 2 * Math.PI); ctx.fill();
        // The neutron: in from the left, out after the collision.
        var nx = t < 0 ? cx - a * 0.4 + t * a * 2 : cx - a * 0.4 - t * a * 1.4, ny = t < 0 ? cy : cy - t * a * 0.9;
        ctx.fillStyle = acc; ctx.beginPath(); ctx.arc(nx, ny, a * 0.09, 0, 2 * Math.PI); ctx.fill();
        // The ladder: one vibration mode's equally spaced steps (illustration).
        if (st.bound && !freeLike) {
          var lx = w - 70, ly0 = cy + a * 1.6, step = a * 0.6;
          ctx.strokeStyle = fg; ctx.lineWidth = 1.5;
          for (var k = 0; k < 5; k++) { ctx.beginPath(); ctx.moveTo(lx, ly0 - k * step); ctx.lineTo(lx + 44, ly0 - k * step); ctx.stroke(); }
          var lit = t > 0 ? 1 : 0;
          ctx.fillStyle = hot; ctx.beginPath(); ctx.arc(lx + 22, ly0 - lit * step, 5, 0, 2 * Math.PI); ctx.fill();
          ctx.fillStyle = mut; ctx.font = "11px system-ui, sans-serif"; ctx.fillText("one mode's", lx - 6, ly0 - 4.6 * step); ctx.fillText("levels", lx - 6, ly0 - 4.6 * step + 13);
        }
        ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif";
        ctx.fillText(st.bound ? "bound: held by springs (an illustration)" : "free: a lone atom", 12, 76);
        var verdict = !st.bound ? "A free atom recoils with whatever the collision gives it (page 2)."
          : ratio > 30 ? "That is far more than any vibration energy of the lattice, so the springs barely matter: the struck atom recoils as if it were free, and free-gas scattering (page 3) works."
            : ratio > 1 ? "That is only a few times the lattice's vibration energies, so the binding already matters: the atom cannot simply recoil on its own."
              : "That is less than the lattice's vibration energies: the neutron can only exchange energy with the whole lattice, in vibration-sized steps, and it can gain energy as well as lose it.";
        return "A " + fmtE(E) + " neutron can give a free Si atom (A = " + A.toFixed(2) + ") at most " + fmtE(T) + ". The vibrations of Si bound in α-quartz reach up to " + fmtE(sp.top) + " (LEAPR deck). " + verdict;
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "Vibration energies from the α-quartz LEAPR deck (reference-data/leapr/), read by OUTRAM PARK's LEAPR deck parser. The springs, the 2D lattice and the equally spaced ladder are illustrations: real crystals are 3D and have a spread of vibration energies (pages 21 and 23). The widget draws the atom as flying off once the most a collision can give it exceeds 30 times the top vibration energy: a rough line for the picture, not a sharp physical threshold."));
      }
    });
    function spring(ctx, x0, y0, x1, y1, dx, broken) {
      ctx.strokeStyle = broken ? "rgba(184,92,56,0.5)" : css("--muted", "#5b636b");
      ctx.setLineDash(broken ? [3, 4] : []);
      var n = 8, len = Math.hypot(x1 - x0, y1 - y0), ux = (x1 - x0) / len, uy = (y1 - y0) / len, amp = 4;
      ctx.beginPath(); ctx.moveTo(x0, y0);
      for (var k = 1; k < n; k++) { var s = k / n, px = x0 + (x1 - x0) * s, py = y0 + (y1 - y0) * s, sg = k % 2 ? 1 : -1; ctx.lineTo(px - uy * amp * sg, py + ux * amp * sg); }
      ctx.lineTo(x1, y1); ctx.stroke(); ctx.setLineDash([]);
    }
    var card = root.parentNode;
    card.querySelectorAll("[data-bound]").forEach(function (b) {
      b.onclick = function () {
        st.bound = b.getAttribute("data-bound") === "1"; st.t = -1.2;
        card.querySelectorAll("[data-bound]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
        frame.play();
      };
    });
    var input = card.querySelector("input[data-loge]"), out = card.querySelector("[data-loge-out]");
    if (input) {
      var go = function () { st.lg = parseFloat(input.value); if (out) out.textContent = fmtE(Math.pow(10, st.lg)); frame.redraw(); };
      input.oninput = go; go();
    }
    var hit = card.querySelector("[data-hit]"); if (hit) hit.onclick = function () { st.t = -1.2; frame.play(); };
    return frame;
  }

  L.widget("bound", boundWidget);
})();
