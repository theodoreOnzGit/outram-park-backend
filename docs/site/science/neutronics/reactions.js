// Neutronics lessons, pages 10 and 11 (gh:#906 design comment and amendment,
// 2026-10-16): what can happen when a neutron meets a nucleus, and fission.
// Every number comes from reaction-data.js, read from the ENDF/B-VIII.0
// tapes by crates/njoy-outram-park-fork/examples/lesson_reactions.rs.
//
// - #channels (page 10): the compound nucleus's excitation
//   E* = S_n + E_cm (S_n = the capture Q, MT 102 QM; E_cm = E · A/(A + 1))
//   against each exit channel's threshold, lit when energetically open: a
//   channel with Q-value QI opens at a lab energy −QI · (A + 1)/A (zero if
//   QI ≥ 0). Fission is lit from the data, not from its Q (positive for both
//   uranium isotopes): for U-238 from the energy where its σ_f stays above
//   10 % of its 2 MeV value.
// - #fission (page 11): σ_f(E) of U-235 and U-238 on the same log-log axes.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame;

  function q(R, mt) { for (var i = 0; i < R.channels.length; i++) if (R.channels[i].mt === mt) return R.channels[i]; return null; }
  // Lab threshold of a channel with Q-value qi [eV] on a target of mass ratio A.
  function threshold(qi, A) { return qi >= 0 ? 0 : -qi * (A + 1) / A; }
  // The exit channels and whether each is open at lab energy E [eV].
  function channels(R, E) {
    var A = R.awr, out = [];
    var add = function (mt, label, open, note) { if (q(R, mt)) out.push({ mt: mt, label: label, open: open, note: note, ch: q(R, mt) }); };
    add(2, "(n,n) elastic", true, "always open");
    var c51 = q(R, 51);
    if (c51) add(51, "(n,n′) inelastic", E >= threshold(c51.qi, A), "from " + fmtE(threshold(c51.qi, A)) + " (first level)");
    var c102 = q(R, 102);
    if (c102) add(102, "(n,γ) capture", true, "Q +" + fmtE(c102.qm) + ": always open");
    [[103, "(n,p)"], [107, "(n,α)"], [16, "(n,2n)"]].forEach(function (p) {
      var c = q(R, p[0]); if (!c) return;
      var t = threshold(c.qi, A);
      add(p[0], p[1], E >= t, (c.qi >= 0 ? "Q +" + fmtE(c.qi) + ": any energy" : "Q −" + fmtE(-c.qi) + ": from " + fmtE(t)));
    });
    if (R.sf) {
      var fissile = R.sf.thermal > 1;
      add(18, "fission", fissile || E >= R.sf.onset, fissile ? "fissile: any energy" : "steeply from ≈ " + fmtE(R.sf.onset) + " (page 11)");
    }
    return out;
  }
  L.reactionsMath = { threshold: threshold, channels: channels };

  // ─── Page 10: the compound nucleus and its exits ───────────────────────
  function channelsWidget(root) {
    var D = window.REACTION_DATA;
    var st = { nuc: "U-238", lg: Math.log10(2e6) };
    var frame = new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var R = D[st.nuc], A = R.awr, E = Math.pow(10, st.lg), Ecm = E * A / (A + 1), Sn = q(R, 102).qm, Ex = Sn + Ecm;
        // Left column: the energy diagram, MeV above the compound nucleus's ground state.
        var top = 64, colw = Math.min(150, w * 0.36), x0 = 12, bot = h - 20, emax = Math.max(Ex, Sn) * 1.25 / zoom;
        var Y = function (e) { return bot - (bot - top) * e / emax; };
        ctx.font = "12px system-ui, sans-serif";
        ctx.strokeStyle = fg; ctx.lineWidth = 2;
        ctx.beginPath(); ctx.moveTo(x0, Y(0)); ctx.lineTo(x0 + colw, Y(0)); ctx.stroke();
        ctx.fillStyle = fg; ctx.fillText("compound ground", x0, Y(0) - 4);
        ctx.strokeStyle = mut; ctx.setLineDash([4, 3]);
        ctx.beginPath(); ctx.moveTo(x0, Y(Sn)); ctx.lineTo(x0 + colw, Y(Sn)); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = mut; ctx.fillText("target + n at rest", x0, Y(Sn) + 14);
        ctx.strokeStyle = hot; ctx.lineWidth = 3;
        ctx.beginPath(); ctx.moveTo(x0, Y(Ex)); ctx.lineTo(x0 + colw, Y(Ex)); ctx.stroke();
        ctx.fillStyle = hot; ctx.fillText("E* = " + (Ex / 1e6).toFixed(3) + " MeV", x0, Y(Ex) - 5);
        ctx.strokeStyle = mut; ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(x0 + colw / 2, Y(0)); ctx.lineTo(x0 + colw / 2, Y(Sn)); ctx.stroke();
        ctx.fillStyle = mut; ctx.fillText("S_n " + (Sn / 1e6).toFixed(3), x0 + colw / 2 + 4, (Y(0) + Y(Sn)) / 2);
        // Right column: the exits, lit when open.
        var cx = x0 + colw + 18, rows = channels(R, E), rh = Math.min(46, (bot - top) / rows.length);
        rows.forEach(function (c, i) {
          var y = top + 10 + i * rh;
          ctx.fillStyle = c.open ? hot : "rgba(128,128,128,0.35)";
          ctx.beginPath(); ctx.arc(cx + 8, y, 7, 0, 2 * Math.PI); ctx.fill();
          ctx.fillStyle = c.open ? fg : mut; ctx.font = (c.open ? "bold " : "") + "13px system-ui, sans-serif";
          ctx.fillText(c.label + "  MT " + (c.mt === 51 ? "51-91" : c.mt), cx + 22, y + 4);
          ctx.font = "11px system-ui, sans-serif"; ctx.fillStyle = mut;
          ctx.fillText(c.note, cx + 22, y + 18);
        });
        var open = rows.filter(function (c) { return c.open; }).map(function (c) { return c.label; });
        return st.nuc + " + a " + fmtE(E) + " neutron: the compound nucleus is left " + (Ex / 1e6).toFixed(3) + " MeV above its ground state (binding " +
          (Sn / 1e6).toFixed(3) + " MeV + " + fmtE(Ecm) + " of kinetic energy in the centre of mass). Open: " + open.join(", ") + ".";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "Open means energetically allowed. How likely each exit is, is its cross section, which nuclear data stores for each MT number. Charged particles must also get out past the nucleus's electric repulsion, so (n,p) and (n,α) stay rare just above their thresholds."));
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
    var input = card.querySelector("input[data-loge]"), out = card.querySelector("[data-loge-out]");
    if (input) {
      var set = function () { st.lg = parseFloat(input.value); if (out) out.textContent = fmtE(Math.pow(10, st.lg)); frame.redraw(); };
      input.oninput = set; set();
    }
    return frame;
  }

  // ─── Page 11: fissile against fissionable ──────────────────────────────
  function fissionWidget(root) {
    var D = window.REACTION_DATA;
    return new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var x0 = 46, y0 = 76, pw = w - x0 - 12, ph = h - y0 - 40, lx0 = -5, lx1 = Math.log10(2e7), ly0 = -6, ly1 = 4;
        var X = function (e) { return x0 + (Math.log10(e) - lx0) / (lx1 - lx0) * pw; };
        var Y = function (s) { return y0 + (ly1 - Math.log10(Math.max(s, 1e-7))) / (ly1 - ly0) * ph / zoom; };
        ctx.font = "12px system-ui, sans-serif"; ctx.fillStyle = mut; ctx.strokeStyle = "rgba(128,128,128,0.25)"; ctx.lineWidth = 1;
        for (var d = -4; d <= 7; d++) { var gx = X(Math.pow(10, d)); ctx.beginPath(); ctx.moveTo(gx, y0); ctx.lineTo(gx, y0 + ph); ctx.stroke(); }
        [["1 meV", -3], ["1 eV", 0], ["1 keV", 3], ["1 MeV", 6]].forEach(function (t) { var gx = X(Math.pow(10, t[1])); ctx.fillText(t[0], gx - ctx.measureText(t[0]).width / 2, y0 + ph + 16); });
        for (var b = ly0; b <= ly1; b += 2) { var gy = Y(Math.pow(10, b)); if (gy > y0 + ph + 1) continue; ctx.beginPath(); ctx.moveTo(x0, gy); ctx.lineTo(x0 + pw, gy); ctx.stroke(); ctx.fillText("10" + String(b).replace("-", "⁻").replace(/[0-9]/g, function (c) { return "⁰¹²³⁴⁵⁶⁷⁸⁹"[+c]; }), 6, gy + 4); }
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        [["U-235", acc], ["U-238", hot]].forEach(function (p) {
          var S = D[p[0]].sf; ctx.strokeStyle = p[1]; ctx.lineWidth = 1.4; ctx.beginPath();
          for (var i = 0; i < S.e.length; i++) { var xx = X(S.e[i]), yy = Y(S.s[i]); if (i) ctx.lineTo(xx, yy); else ctx.moveTo(xx, yy); }
          ctx.stroke();
        });
        var on = D["U-238"].sf.onset, xo = X(on);
        ctx.strokeStyle = hot; ctx.setLineDash([4, 3]); ctx.beginPath(); ctx.moveTo(xo, y0); ctx.lineTo(xo, y0 + ph); ctx.stroke(); ctx.setLineDash([]);
        ctx.restore();
        ctx.font = "bold 13px system-ui, sans-serif"; ctx.fillStyle = acc; ctx.fillText("U-235", x0 + 6, y0 - 8);
        ctx.fillStyle = hot; ctx.fillText("U-238", x0 + 64, y0 - 8);
        ctx.fillStyle = mut; ctx.font = "12px system-ui, sans-serif"; ctx.fillText("σ_f [barn]", x0 + 124, y0 - 8);
        var a = D["U-235"].sf, c = D["U-238"].sf;
        return "Fission cross sections, ENDF/B-VIII.0, our RECONR port at 0 K. At 0.0253 eV (thermal): U-235 " + a.thermal.toFixed(0) + " b, U-238 " + c.thermal.toExponential(1) +
          " b. At 2 MeV: U-235 " + a.at2MeV.toFixed(2) + " b, U-238 " + c.at2MeV.toFixed(2) + " b. U-238's stays above 10 % of its 2 MeV value only from " + fmtE(c.onset) + " (dashed).";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "Same axes for both. Thinned for a phone as on page 9: the highest and lowest point of each of 900 log-E steps; nothing smoothed."));
      }
    });
  }

  L.widget("channels", channelsWidget);
  L.widget("fission", fissionWidget);
})();
