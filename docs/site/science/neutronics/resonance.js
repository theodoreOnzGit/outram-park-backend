// Neutronics lessons, pages 12 to 15 (gh:#906 design and amendment,
// 2026-10-16): the shape of a resonance peak (SLBW), then one pair of levels
// seen through the multi-level formalisms (pages 13-15). The formulas are
// OUTRAM PARK's RECONR port line for line (njoy-outram-park-fork
// reconr/slbw.rs: eval_slbw_lstate, eval_mlbw_lstate,
// shift_and_penetrability, facphi; reconr/rm.rs: eval_rm_range, thrinv,
// frobns), itself NJOY2016 reconr.f90's csslbw, csmlbw and csrmat
// (ENDF-102 Appendix D). check-physics.js compares these functions
// with the Rust port's own numbers in resonance-data.js. All shapes are at
// 0 K: no Doppler broadening.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame;
  var D = window.RES_DATA;
  var HBAR_EV_S = 6.582119569e-16; // CODATA 2018 (Tiesinga et al., 2021), as page 4

  function facts(l, rho) {
    var r2 = rho * rho;
    if (l === 0) return [0, rho];
    if (l === 1) { var d = 1 + r2; return [-1 / d, r2 * rho / d]; }
    var r4 = r2 * r2, den = 3 * r2 + r4 + 9;
    return [-(18 + 3 * r2) / den, r4 * rho / den];
  }
  function facphi(l, rho) {
    var r2 = rho * rho, phi = l === 0 ? rho : l === 1 ? rho - Math.atan(rho) : rho - Math.atan(3 * rho / (3 - r2));
    return l >= 2 && phi / rho < 1e-6 ? 0 : phi;
  }
  // res: [[ER, AJ, GT, GN, GG, GF], ...]; returns {el, cap, fis} [b].
  function slbw(e, res, l, spi, ap, awri, ra) {
    if (e <= 0) return { el: 0, cap: 0, fis: 0 };
    var arat = awri / (awri + 1), k = D.waveK * arat * Math.sqrt(e), pifac = Math.PI / (k * k);
    var sp = facts(l, k * ra), se = sp[0], pe = sp[1], phi = facphi(l, k * ap);
    var cos2p = Math.cos(2 * phi), sin2p = Math.sin(2 * phi), sinsq = Math.pow(Math.sin(phi), 2);
    var el = 4 * (2 * l + 1) * pifac * sinsq, cap = 0, fis = 0, spifac = 1 / (2 * spi + 1);
    res.forEach(function (r) {
      var er = r[0], aj = r[1], gn = r[3], gg = r[4], gf = r[5];
      var kr = D.waveK * arat * Math.sqrt(Math.abs(er)), srp = facts(l, kr * ra), ser = srp[0], per = srp[1];
      var rper = Math.abs(per) > 1e-30 ? 1 / per : 0;
      var gne = gn * pe * rper, erp = er + gn * (ser - se) * rper / 2, edelt = e - erp, gx = gg + gf, gtt = gne + gx;
      var gj = (2 * aj + 1) * spifac / 2;
      var comfac = pifac * gj * gne / (edelt * edelt + gtt * gtt / 4);
      el += comfac * (gne * cos2p - 2 * gx * sinsq + 2 * edelt * sin2p);
      cap += comfac * gg; fis += comfac * gf;
    });
    return { el: el, cap: cap, fis: fis };
  }
  function mlbw(e, res, l, spi, ap, awri, ra) {
    if (e <= 0) return { el: 0, cap: 0, fis: 0 };
    var arat = awri / (awri + 1), k = D.waveK * arat * Math.sqrt(e), pifac = Math.PI / (k * k);
    var sp = facts(l, k * ra), se = sp[0], pe = sp[1], phi = facphi(l, k * ap);
    var cos2p = 1 - Math.cos(2 * phi), sin2p = Math.sin(2 * phi);
    var den = 4 * spi + 2, ajmin = Math.abs(Math.abs(spi - l) - 0.5), ajmax = spi + l + 0.5;
    var nj = Math.min(10, Math.round(ajmax - ajmin + 1)), gj = [], sum = 0, aj = ajmin;
    for (var i = 0; i < nj; i++) { gj.push((2 * aj + 1) / den); aj += 1; sum += gj[i]; }
    var diff = 2 * l + 1 - sum, s1 = new Array(nj).fill(0), s2 = new Array(nj).fill(0), cap = 0, fis = 0;
    res.forEach(function (r) {
      var j = Math.min(Math.max(Math.round(r[1] - ajmin + 1), 1), nj) - 1;
      var kr = D.waveK * arat * Math.sqrt(Math.abs(r[0])), srp = facts(l, kr * ra), ser = srp[0], per = srp[1];
      var rper = Math.abs(per) > 1e-30 ? 1 / per : 0;
      var erp = r[0] + r[3] * (ser - se) * rper / 2, edelt = e - erp, gne = r[3] * pe * rper, gx = r[4] + r[5], gtt = gne + gx;
      var x = 2 * edelt / gtt, comfac = 2 * gne / gtt / (1 + x * x);
      s1[j] += comfac; s2[j] += comfac * x;
      comfac = comfac * gj[j] / gtt;
      fis += comfac * r[5]; cap += comfac * r[4];
    });
    var el = 0;
    for (var m = 0; m < nj; m++) el += gj[m] * (Math.pow(cos2p - s1[m], 2) + Math.pow(sin2p + s2[m], 2));
    el += 2 * diff * cos2p;
    return { el: el * pifac, cap: cap * 2 * pifac, fis: fis * 2 * pifac };
  }
  // Reich–Moore: reconr/rm.rs eval_rm_range (NJOY2016 csrmat, with thrinv,
  // abcmat and frobns), for one l-state whose list carries no APL of its own
  // (so the radius for the penetrability is ra and for the phase ap).
  // res: [[ER, AJ, GN, GG, GFA, GFB], ...]; returns {el, cap, fis} [b].
  function thrinv(d) {
    var i, j, lr;
    for (j = 0; j < 3; j++) { for (i = 0; i <= j; i++) { d[i][j] = -d[i][j]; d[j][i] = d[i][j]; } d[j][j] = 1 + d[j][j]; }
    for (lr = 0; lr < 3; lr++) {
      var fooey = 1 - d[lr][lr];
      if (fooey === 0) return false;
      d[lr][lr] = 1 / fooey;
      var s = [0, 0, 0];
      for (j = 0; j < 3; j++) { s[j] = d[lr][j]; if (j !== lr) { d[j][lr] *= d[lr][lr]; d[lr][j] = d[j][lr]; } }
      for (j = 0; j < 3; j++) if (j !== lr) for (i = 0; i <= j; i++) if (i !== lr) { d[i][j] += d[i][lr] * s[j]; d[j][i] = d[i][j]; }
    }
    return true;
  }
  function mul3(a, b) {
    var c = [[0, 0, 0], [0, 0, 0], [0, 0, 0]];
    for (var i = 0; i < 3; i++) for (var j = 0; j < 3; j++) for (var k = 0; k < 3; k++) c[i][j] += a[i][k] * b[k][j];
    return c;
  }
  var copy3 = function (m) { return m.map(function (r) { return r.slice(); }); };
  var zero3 = function () { return [[0, 0, 0], [0, 0, 0], [0, 0, 0]]; };
  // (A + iB)^-1 = C + iD.
  function frobns(a, b) {
    var orig = copy3(a);
    if (!thrinv(a)) return [zero3(), zero3()];
    var q = mul3(a, b), t = mul3(b, q), c = copy3(orig);
    for (var i = 0; i < 3; i++) for (var j = 0; j < 3; j++) c[i][j] += t[i][j];
    if (!thrinv(c)) return [zero3(), zero3()];
    var d = mul3(q, c);
    for (i = 0; i < 3; i++) for (j = 0; j < 3; j++) d[i][j] = -d[i][j];
    return [c, d];
  }
  function kcode(kchanl, kp, kn, jj, jjl, numj) {
    var interior = jj > jjl && jj < numj;
    if (kchanl === 1) return kp > 0 ? (kn === 0 ? (interior ? 2 : 1) : 1) : (kn === 0 ? (interior ? 2 : 1) : 0);
    if (kchanl === 2) return kp > 0 ? (kn > 0 ? 1 : 0) : (kn > 0 ? (interior ? 2 : 1) : 0);
    return 0;
  }
  function rm(e, res, l, spi, ap, awri, ra) {
    if (e <= 0) return { el: 0, cap: 0, fis: 0 };
    var arat = awri / (awri + 1), k = D.waveK * arat * Math.sqrt(e), pifac = Math.PI / (k * k), gjd = 2 * (2 * spi + 1);
    var gf = false, sig = [0, 0, 0, 0];
    var pe = facts(l, k * ra)[1], phi = facphi(l, k * ap), p1 = Math.cos(2 * phi), p2 = Math.sin(2 * phi);
    var ajmin = Math.abs(Math.abs(spi - l) - 0.5), numj = Math.round(spi + l + 0.5 - ajmin + 1);
    var jjl = l !== 0 && l > spi - 0.5 && l <= spi ? 0 : 1, ajc = ajmin - 1;
    for (var jj = 1; jj <= numj; jj++) {
      ajc += 1;
      var gj = (2 * ajc + 1) / gjd;
      for (var kchanl = 1; kchanl <= 2; kchanl++) {
        var r = zero3(), s = zero3(), kp = 0, kn = 0;
        res.forEach(function (v) {
          var er = v[0], aj = v[1], gn = v[2], gg = v[3], gfa = v[4], gfb = v[5];
          if (Math.abs(Math.abs(aj) - ajc) > 0.25) return;
          if (aj < 0) kn++;
          if (aj > 0) kp++;
          if ((kchanl === 1 && aj < 0) || (kchanl === 2 && aj > 0)) return;
          var per = facts(l, D.waveK * arat * Math.sqrt(Math.abs(er)) * ra)[1];
          var a1 = Math.sqrt(gn * pe / per), a2 = gfa !== 0 ? Math.sign(gfa) * Math.sqrt(Math.abs(gfa)) : 0, a3 = gfb !== 0 ? Math.sign(gfb) * Math.sqrt(Math.abs(gfb)) : 0;
          var diff = er - e, den = diff * diff + 0.25 * gg * gg, de2 = 0.5 * diff / den, gg4 = 0.25 * gg / den;
          r[0][0] += gg4 * a1 * a1; s[0][0] -= de2 * a1 * a1;
          if (gfa !== 0 || gfb !== 0) {
            r[0][1] += gg4 * a1 * a2; s[0][1] -= de2 * a1 * a2; r[0][2] += gg4 * a1 * a3; s[0][2] -= de2 * a1 * a3;
            r[1][1] += gg4 * a2 * a2; s[1][1] -= de2 * a2 * a2; r[2][2] += gg4 * a3 * a3; s[2][2] -= de2 * a3 * a3;
            r[1][2] += gg4 * a2 * a3; s[1][2] -= de2 * a2 * a3;
            gf = true;
          }
        });
        var kkk = kcode(kchanl, kp, kn, jj, jjl, numj);
        if (kkk === 0) continue;
        var termt, termn, termf;
        if (gf) {
          r[0][0] += 1; r[1][1] += 1; r[2][2] += 1;
          r[1][0] = r[0][1]; s[1][0] = s[0][1]; r[2][0] = r[0][2]; s[2][0] = s[0][2]; r[2][1] = r[1][2]; s[2][1] = s[1][2];
          var inv = frobns(r, s), ri = inv[0], si = inv[1];
          termf = 4 * gj * (ri[0][1] * ri[0][1] + si[0][1] * si[0][1] + ri[0][2] * ri[0][2] + si[0][2] * si[0][2]);
          var u11r = p1 * (2 * ri[0][0] - 1) + 2 * p2 * si[0][0], u11i = p2 * (1 - 2 * ri[0][0]) + 2 * p1 * si[0][0];
          termt = 2 * gj * (1 - u11r); termn = gj * ((1 - u11r) * (1 - u11r) + u11i * u11i);
        } else {
          var dd = r[0][0], rr = 1 + dd, ss = s[0][0], amag = rr * rr + ss * ss, rri = rr / amag, ssi = -ss / amag;
          var uur = p1 * (2 * rri - 1) + 2 * p2 * ssi, uui = p2 * (1 - 2 * rri) + 2 * p1 * ssi;
          if (Math.abs(dd) < 3e-4 && Math.abs(phi) < 3e-4) {
            var xx = 2 * dd;
            xx += 2 * (dd * dd + ss * ss + phi * phi + p2 * ss);
            xx -= 2 * phi * phi * (dd * dd + ss * ss);
            xx /= amag;
            termt = 2 * gj * xx; termn = gj * (xx * xx + uui * uui);
          } else { termt = 2 * gj * (1 - uur); termn = gj * ((1 - uur) * (1 - uur) + uui * uui); }
          termf = 0;
        }
        if (kkk === 2) { termn += 2 * gj * (1 - p1); termt += 2 * gj * (1 - p1); }
        sig[1] += termn; sig[3] += termt - termf - termn; sig[2] += termf; sig[0] += termt;
      }
    }
    return { el: pifac * sig[1], cap: pifac * sig[3], fis: pifac * sig[2] };
  }
  L.resonanceMath = { slbw: slbw, mlbw: mlbw, rm: rm, HBAR_EV_S: HBAR_EV_S };

  function axes(ctx, x0, y0, pw, ph, mut) {
    ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
  }

  // ─── Page 12: one resonance (SLBW) ─────────────────────────────────────
  function slbwWidget(root) {
    var U = D.u238;
    var st = { gn: U.gn, gg: U.gg, gf: 0, er: U.er, ch: "cap" };
    var res = function () { return [[st.er, U.aj, st.gn + st.gg + st.gf, st.gn, st.gg, st.gf]]; };
    var frame = new Frame(root, {
      reset: function () {},
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var G = st.gn + st.gg + st.gf, half = Math.max(12 * G, 0.15) / zoom, e0 = Math.max(st.er - half, 0.01), e1 = st.er + half;
        var N = 600, xs = [], ys = [], peak = 0, peakE = st.er, lo = Infinity;
        for (var i = 0; i <= N; i++) {
          var e = e0 + (e1 - e0) * i / N, s = slbw(e, res(), 0, U.spi, U.ap, U.awri, U.ra), v = st.ch === "cap" ? s.cap : s.el;
          xs.push(e); ys.push(v); if (v > peak) { peak = v; peakE = e; } lo = Math.min(lo, v);
        }
        // Measured full width at half maximum: the grid brackets each crossing,
        // bisection finds it.
        var hm = peak / 2, a = peakE, b = peakE, f = function (e) { var t = slbw(e, res(), 0, U.spi, U.ap, U.awri, U.ra); return (st.ch === "cap" ? t.cap : t.el) - hm; };
        var cross = function (lo, hi) { for (var k = 0; k < 60; k++) { var mid = (lo + hi) / 2; if ((f(lo) < 0) === (f(mid) < 0)) lo = mid; else hi = mid; } return (lo + hi) / 2; };
        for (i = 1; i <= N; i++) if (ys[i] >= hm) { a = ys[i - 1] < hm ? cross(xs[i - 1], xs[i]) : xs[i]; break; }
        for (i = N - 1; i >= 0; i--) if (ys[i] >= hm) { b = ys[i + 1] < hm ? cross(xs[i], xs[i + 1]) : xs[i]; break; }
        var x0 = 50, y0 = 70, pw = w - x0 - 12, ph = h - y0 - 46;
        var ly1 = Math.ceil(Math.log10(peak * 1.5)), ly0 = ly1 - 5;
        var X = function (e) { return x0 + (e - e0) / (e1 - e0) * pw; }, Y = function (v) { return y0 + (ly1 - Math.log10(Math.max(v, 1e-9))) / (ly1 - ly0) * ph; };
        axes(ctx, x0, y0, pw, ph, mut);
        ctx.font = "12px system-ui, sans-serif"; ctx.fillStyle = mut;
        for (var d = ly0; d <= ly1; d++) ctx.fillText(d === 0 ? "1 b" : "10" + String(d).replace("-", "⁻").replace(/[0-9]/g, function (c) { return "⁰¹²³⁴⁵⁶⁷⁸⁹"[+c]; }), 6, Y(Math.pow(10, d)) + 4);
        ctx.fillText(fmtE(e0), x0, y0 + ph + 16); var t1 = fmtE(e1); ctx.fillText(t1, x0 + pw - ctx.measureText(t1).width, y0 + ph + 16);
        ctx.strokeStyle = st.ch === "cap" ? acc : hot; ctx.lineWidth = 2; ctx.beginPath();
        for (i = 0; i <= N; i++) { if (i) ctx.lineTo(X(xs[i]), Y(ys[i])); else ctx.moveTo(X(xs[i]), Y(ys[i])); }
        ctx.stroke();
        ctx.strokeStyle = mut; ctx.setLineDash([4, 3]); ctx.beginPath(); ctx.moveTo(X(a), Y(hm)); ctx.lineTo(X(b), Y(hm)); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif";
        ctx.fillText((st.ch === "cap" ? "capture" : "elastic") + " σ [b] near " + fmtE(st.er), x0, y0 - 10);
        var tau = HBAR_EV_S / G;
        return "Peak " + (peak >= 100 ? peak.toFixed(0) : peak.toPrecision(3)) + " b; full width at half maximum " + fmtE(b - a) + " (Γ = Γn + Γγ" + (st.gf ? " + Γf" : "") + " = " + fmtE(G) +
          "); lifetime τ = ħ/Γ = " + (tau * 1e15).toPrecision(3) + " fs." + (st.ch === "el" && lo < peak / 1e3 ? " The dip below the peak is the level interfering with potential scattering." : "") +
          " U-238's 6.67 eV level is evaluated with Reich–Moore parameters; drawn with the SLBW formula it is an approximation. 0 K: no Doppler broadening.";
      },
      panel: function (body, redraw) {
        L.slider(body, "Fission width Γf (meV)", 0, 50, 0.1, st.gf * 1e3, function (v) { st.gf = v / 1e3; redraw(); return v.toFixed(1); });
        body.append(el("p", { "class": "w-note" }, "A capture-only level unless you give it a fission width. The formula is NJOY2016's csslbw, through OUTRAM PARK's RECONR port."));
      }
    });
    var card = root.parentNode;
    var bind = function (attr, set, fmt) {
      var input = card.querySelector("[" + attr + "]"), out = card.querySelector("[" + attr + "-out]");
      if (!input) return;
      var go = function () { set(parseFloat(input.value)); if (out) out.textContent = fmt(parseFloat(input.value)); frame.redraw(); };
      input.oninput = go; go();
    };
    bind("data-gn", function (v) { st.gn = v / 1e3; }, function (v) { return v.toFixed(2) + " meV"; });
    bind("data-gg", function (v) { st.gg = v / 1e3; }, function (v) { return v.toFixed(1) + " meV"; });
    card.querySelectorAll("[data-ch]").forEach(function (b) {
      b.onclick = function () { st.ch = b.getAttribute("data-ch"); card.querySelectorAll("[data-ch]").forEach(function (o) { o.classList.toggle("ghost", o !== b); }); frame.redraw(); };
    });
    var rs = card.querySelector("[data-u238]");
    if (rs) rs.onclick = function () {
      st.gn = U.gn; st.gg = U.gg; st.gf = 0;
      var a = card.querySelector("[data-gn]"), g = card.querySelector("[data-gg]");
      if (a) { a.value = U.gn * 1e3; a.oninput(); } if (g) { g.value = U.gg * 1e3; g.oninput(); }
    };
    return frame;
  }

  // ─── Pages 13-15: one pair of levels, a formalism per page ─────────────
  // mode "rmatrix" (page 13): levels interfering (Reich-Moore, the closest
  //   formalism our port can evaluate) against each peak alone, summed (SLBW).
  // mode "reichmoore" (page 14): Reich-Moore in each channel, with an optional
  //   fission width, against the SLBW sum.
  // mode "mlbw" (page 15): Reich-Moore, MLBW and the SLBW sum together.
  function pairWidget(mode) {
    return function (root) {
      var P = D.ref.pair;
      var st = { sep: 2, same: true, ch: "el", gf: 0 };
      var j2 = function () { return st.same ? 1 : 0; };
      var bw = function () { return [[10, 1, 1.03 + st.gf, 1.0, 0.03, st.gf], [10 + st.sep, j2(), 1.03 + st.gf, 1.0, 0.03, st.gf]]; };
      var rmres = function () { return [[10, 1, 1.0, 0.03, st.gf, 0], [10 + st.sep, j2(), 1.0, 0.03, st.gf, 0]]; };
      var NAME = { el: "elastic", cap: "capture", fis: "fission" }, CAP = { el: "Elastic", cap: "Capture", fis: "Fission" };
      var frame = new Frame(root, {
        reset: function () {},
        tick: function () { return false; },
        draw: function (ctx, w, h, zoom) {
          var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
          if (st.ch === "fis" && !st.gf) st.ch = "el";
          var ch = st.ch, e0 = 6, e1 = 10 + st.sep + 4, N = 500, cur = { rm: [], ml: [], sl: [] }, hi = 0, lo = 0, dML = 0, dSL = 0, dMS = 0;
          for (var i = 0; i <= N; i++) {
            var e = e0 + (e1 - e0) * i / N;
            var r = rm(e, rmres(), 0, P.spi, P.ap, P.awri, P.ra)[ch], s = slbw(e, bw(), 0, P.spi, P.ap, P.awri, P.ra)[ch], m = mlbw(e, bw(), 0, P.spi, P.ap, P.awri, P.ra)[ch];
            cur.rm.push(r); cur.sl.push(s); cur.ml.push(m);
            hi = Math.max(hi, r, s, mode === "mlbw" ? m : 0); lo = Math.min(lo, r, s, mode === "mlbw" ? m : 0);
            dSL = Math.max(dSL, Math.abs(r - s)); dML = Math.max(dML, Math.abs(r - m)); dMS = Math.max(dMS, Math.abs(m - s));
          }
          hi *= 1.1 / zoom; lo = Math.min(lo * 1.1, -0.02 * hi);
          var nleg = mode === "mlbw" ? 3 : 2, ly = 66, x0 = 50, y0 = ly + 16 * nleg + 26, pw = w - x0 - 12, ph = h - y0 - 46;
          var X = function (e) { return x0 + (e - e0) / (e1 - e0) * pw; }, Y = function (v) { return y0 + (hi - v) / (hi - lo) * ph; };
          axes(ctx, x0, y0, pw, ph, mut);
          ctx.strokeStyle = mut; ctx.beginPath(); ctx.moveTo(x0, Y(0)); ctx.lineTo(x0 + pw, Y(0)); ctx.stroke();
          ctx.font = "12px system-ui, sans-serif"; ctx.fillStyle = mut; ctx.fillText("0 b", 14, Y(0) + 4); ctx.fillText(hi >= 1e4 ? Math.round(hi / 1e3) + " kb" : hi >= 100 ? hi.toFixed(0) + " b" : hi.toPrecision(3) + " b", 4, y0 + 10);
          ctx.fillText(fmtE(e0), x0, y0 + ph + 16); var t1 = fmtE(e1); ctx.fillText(t1, x0 + pw - ctx.measureText(t1).width, y0 + ph + 16);
          ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
          var line = function (arr, col, dash, wdt) {
            ctx.strokeStyle = col; ctx.lineWidth = wdt; ctx.setLineDash(dash); ctx.beginPath();
            for (var j = 0; j <= N; j++) { var xx = X(e0 + (e1 - e0) * j / N), yy = Y(arr[j]); if (j) ctx.lineTo(xx, yy); else ctx.moveTo(xx, yy); }
            ctx.stroke(); ctx.setLineDash([]);
          };
          line(cur.sl, mut, [6, 4], 2);
          if (mode === "mlbw") line(cur.ml, acc, [], 2);
          line(cur.rm, hot, [], mode === "mlbw" ? 1.5 : 2);
          ctx.restore();
          // Legend, one entry per line so it fits a phone.
          var leg = mode === "rmatrix" ? [["levels interfering", hot, []], ["each peak alone, summed", mut, [6, 4]]]
            : mode === "reichmoore" ? [["Reich–Moore", hot, []], ["each peak alone, summed (SLBW)", mut, [6, 4]]]
              : [["Reich–Moore", hot, []], ["MLBW", acc, []], ["SLBW sum", mut, [6, 4]]];
          ctx.font = "12px system-ui, sans-serif";
          leg.forEach(function (g, k) {
            var yy = ly + 16 * k;
            ctx.strokeStyle = g[1]; ctx.lineWidth = 2; ctx.setLineDash(g[2]); ctx.beginPath(); ctx.moveTo(x0, yy); ctx.lineTo(x0 + 22, yy); ctx.stroke(); ctx.setLineDash([]);
            ctx.fillStyle = fg; ctx.fillText(g[0], x0 + 28, yy + 4);
          });
          ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText(NAME[ch] + " σ [b]", x0, y0 - 8);
          var spin = st.same ? "the same spin J" : "different spins J";
          var tail = " Illustrative parameters, not an evaluation; 0 K.";
          if (mode === "rmatrix") return "Two levels " + st.sep.toFixed(2) + " eV apart, " + spin + (st.same ? ": their amplitudes add before squaring, so the result is not the sum of two peaks" : ": they cannot interfere, so the result is the sum of two peaks") +
            " (largest difference " + dSL.toPrecision(3) + " b)." + tail;
          if (mode === "reichmoore") return "Two levels " + st.sep.toFixed(2) + " eV apart, " + spin + (st.gf ? ", Γf " + (1e3 * st.gf).toFixed(0) + " meV each" : "") + ". " + CAP[ch] + ": Reich–Moore differs from the sum of single peaks by up to " + dSL.toPrecision(3) + " b." + tail;
          return "Two levels " + st.sep.toFixed(2) + " eV apart, " + spin + (st.gf ? ", Γf " + (1e3 * st.gf).toFixed(0) + " meV each" : "") + ": width over spacing Γ/D = " + ((1.03 + st.gf) / st.sep).toFixed(2) + ". " + CAP[ch] + ": MLBW differs from Reich–Moore by up to " + dML.toPrecision(3) + " b and from the SLBW sum by up to " + dMS.toPrecision(3) + " b" +
            (ch !== "el" ? " (MLBW's " + NAME[ch] + " is the SLBW sum by construction)" : "") + "." + tail;
        },
        panel: function (body, redraw) {
          if (mode !== "rmatrix") L.slider(body, "Fission width Γf on both levels (meV)", 0, 400, 10, 0, function (v) { st.gf = v / 1e3; syncCh(); redraw(); return v.toFixed(0); });
          body.append(el("p", { "class": "w-note" }, "Two s-wave levels, Γn = 1 eV and Γγ = 30 meV each, on a target of mass 100 and spin ½ (so J can be 0 or 1)" +
            (mode !== "rmatrix" ? "; with a fission width, one fission channel, the same sign on both levels" : "") + ". The formulas are NJOY2016's " +
            (mode === "mlbw" ? "csrmat, csmlbw and csslbw" : "csrmat and csslbw") + ", through OUTRAM PARK's RECONR port."));
        }
      });
      var card = root.parentNode, input = card.querySelector("[data-sep]"), out = card.querySelector("[data-sep-out]");
      if (input) { var go = function () { st.sep = parseFloat(input.value); if (out) out.textContent = st.sep.toFixed(2) + " eV"; frame.redraw(); }; input.oninput = go; go(); }
      card.querySelectorAll("[data-j]").forEach(function (b) {
        b.onclick = function () { st.same = b.getAttribute("data-j") === "same"; card.querySelectorAll("[data-j]").forEach(function (o) { o.classList.toggle("ghost", o !== b); }); frame.redraw(); };
      });
      // The fission button is live only with a fission width.
      function syncCh() {
        if (!card) return; // the panel's slider reports once while the frame is built
        card.querySelectorAll("[data-ch]").forEach(function (o) {
          var c = o.getAttribute("data-ch");
          if (c === "fis") o.disabled = !st.gf;
          if (c === "fis" && !st.gf && st.ch === "fis") st.ch = "el";
          o.classList.toggle("ghost", c !== st.ch);
        });
      }
      card.querySelectorAll("[data-ch]").forEach(function (b) {
        b.onclick = function () { st.ch = b.getAttribute("data-ch"); syncCh(); frame.redraw(); };
      });
      syncCh();
      return frame;
    };
  }

  L.widget("slbw", slbwWidget);
  L.widget("rmatrix", pairWidget("rmatrix"));
  L.widget("reichmoore", pairWidget("reichmoore"));
  L.widget("mlbw", pairWidget("mlbw"));
})();
