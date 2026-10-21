// Neutronics lessons, page 19 (gh:#906 design and amendments, 2026-10-16):
// PURR, made-up resonances that obey the statistics. Four stages of one
// widget, every sampled number from OUTRAM PARK's PURR port (purr-data.js,
// written by crates/njoy-outram-park-fork/examples/lesson_purr.rs):
//   1 ladder: one ladder's resonances appear, one at a time, coloured by spin
//     sequence;
//   2 sample: sigma_total at the random sample energies;
//   3 histogram: all of one ladder's samples binned (how likely each sigma is),
//     with each bin's mean capture;
//   4 many ladders: the running average over ladders against the infinitely
//     dilute value computed from the same parameters ("infd").
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame;
  var D = window.PURR_DATA;
  var SEQ = ["#1f6f78", "#4a9aa3", "#b85c38", "#d08a5c", "#7a5ea8"];

  // Bin values on log-spaced edges between lo and hi; returns counts and the
  // mean of `w` in each bin.
  function histogram(v, w, lo, hi, nb) {
    var n = new Array(nb).fill(0), s = new Array(nb).fill(0), l0 = Math.log(lo), dl = Math.log(hi / lo) / nb;
    for (var i = 0; i < v.length; i++) {
      var b = Math.floor((Math.log(v[i]) - l0) / dl);
      b = Math.max(0, Math.min(nb - 1, b));
      n[b]++; s[b] += w[i];
    }
    return { n: n, mean: s.map(function (x, k) { return n[k] ? x / n[k] : 0; }), lo: lo, hi: hi };
  }
  function running(m) { var out = [], s = 0; m.forEach(function (x, i) { s += x; out.push(s / (i + 1)); }); return out; }
  L.purrMath = { histogram: histogram, running: running };

  function purrWidget(root) {
    var st = { stage: "ladder", t: 0, nlad: 1 };
    var R = D.ladder, nres = R.seqs.reduce(function (a, s) { return a + s.r.length; }, 0);
    var all = []; R.seqs.forEach(function (s, j) { s.r.forEach(function (r) { all.push({ e: r[0], g: r[1], j: j }); }); });
    all.sort(function (a, b) { return a.e - b.e; });
    var S = D.samples, tmin = Math.min.apply(null, S.t), tmax = Math.max.apply(null, S.t);
    var H = histogram(S.t, S.c, tmin, tmax * 1.0001, 24);
    var run = running(D.means.t);
    var frame = new Frame(root, {
      reset: function () { st.t = 0; },
      onSeen: function () { st.t = 0; },
      tick: function (dt) { st.t += dt; return st.t < 4; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var x0 = 46, y0 = 84, pw = w - x0 - 12, ph = h - y0 - 46, f = Math.min(1, st.t / 3);
        ctx.font = "12px system-ui, sans-serif"; ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        if (st.stage === "ladder" || st.stage === "sample") {
          var lo = R.lo, hi = lo + (R.hi - lo) / zoom;
          var X = function (e) { return x0 + (e - lo) / (hi - lo) * pw; };
          // Resonances as ticks: height grows with the total width.
          var shown = st.stage === "ladder" ? Math.floor(f * all.length) : all.length;
          ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
          for (var i = 0; i < shown; i++) {
            var r = all[i], xx = X(r.e);
            ctx.strokeStyle = SEQ[r.j % SEQ.length]; ctx.lineWidth = 1.5;
            ctx.beginPath(); ctx.moveTo(xx, y0 + ph); ctx.lineTo(xx, y0 + ph - 16 - 10 * r.j); ctx.stroke();
          }
          if (st.stage === "sample") {
            var vmax = Math.max.apply(null, R.t) * 1.05, ns = Math.floor(f * R.e.length);
            var Y = function (v) { return y0 + ph - 70 - v / vmax * (ph - 80); };
            ctx.strokeStyle = "rgba(128,128,128,0.4)"; ctx.lineWidth = 1; ctx.beginPath();
            for (i = 0; i < ns; i++) { if (i) ctx.lineTo(X(R.e[i]), Y(R.t[i])); else ctx.moveTo(X(R.e[i]), Y(R.t[i])); }
            ctx.stroke();
            ctx.fillStyle = acc;
            for (i = 0; i < ns; i++) { ctx.beginPath(); ctx.arc(X(R.e[i]), Y(R.t[i]), 1.8, 0, 2 * Math.PI); ctx.fill(); }
            ctx.fillStyle = mut; ctx.fillText(vmax.toFixed(0) + " b", 4, Y(vmax) + 4); ctx.fillText("0 b", 14, Y(0) + 4);
          }
          ctx.restore();
          ctx.fillStyle = mut; ctx.fillText("+0 eV", x0, y0 + ph + 16); var tl = "+" + (hi - lo).toFixed(0) + " eV"; ctx.fillText(tl, x0 + pw - ctx.measureText(tl).width, y0 + ph + 16);
          ctx.fillStyle = fg; ctx.fillText(st.stage === "ladder" ? "made-up resonances; colour = spin sequence" : "σ_total at the random sample energies", x0, y0 - 10);
          if (st.stage === "ladder") return "A made-up ladder for U-238 at " + fmtE(D.e) + ": " + R.seqs.length + " spin sequences, each with its own mean spacing (" + R.seqs.map(function (s) { return s.d.toFixed(1); }).join(", ") +
            " eV). Each starts at a random place, steps up by Wigner-distributed spacings and draws chi-square widths. " + all.length + " resonances in this " + (R.hi - R.lo).toFixed(0) + " eV stretch. Energies are on the ladder's own scale.";
          return R.e.length + " of the ladder's " + D.nsamp + " random sample energies fall in this stretch; at each, PURR adds up every resonance's Doppler-broadened shape at " + D.t + " K. The peaks are made up; their statistics are U-238's.";
        }
        if (st.stage === "hist") {
          var nb = H.n.length, nmax = Math.max.apply(null, H.n), bw = pw / nb, shownN = Math.ceil(f * nb);
          for (var b = 0; b < shownN; b++) {
            var hh = H.n[b] / nmax * (ph - 20);
            ctx.fillStyle = "rgba(31,111,120,0.75)"; ctx.fillRect(x0 + b * bw + 1, y0 + ph - hh, bw - 2, hh);
          }
          ctx.fillStyle = mut; ctx.fillText(H.lo.toFixed(1) + " b", x0, y0 + ph + 16); var t2 = H.hi.toFixed(0) + " b (log bins)"; ctx.fillText(t2, x0 + pw - ctx.measureText(t2).width, y0 + ph + 16);
          ctx.fillStyle = fg; ctx.fillText("how often each σ_total occurs, one ladder", x0, y0 - 10);
          var peak = H.n.indexOf(nmax), lowFrac = S.t.filter(function (v) { return v < D.infd.total; }).length / S.t.length;
          return "All " + D.nsamp + " samples of one ladder, binned by σ_total. Most samples sit in the valleys between resonances, near " + (H.lo * Math.pow(H.hi / H.lo, (peak + 0.5) / nb)).toFixed(1) + " b; a few land on peaks up to " + H.hi.toFixed(0) + " b. " +
            (100 * lowFrac).toFixed(0) + " % of samples lie below the average " + D.infd.total.toFixed(2) + " b. For each bin PURR also keeps the mean capture of the samples that landed in it: a conditional capture, from " + Math.min.apply(null, H.mean.filter(function (x) { return x; })).toPrecision(2) + " b to " + Math.max.apply(null, H.mean).toPrecision(2) + " b.";
        }
        // Many ladders: running average against infd.
        var N = Math.max(1, Math.round(st.nlad)), M = D.means.t, lo2 = Math.min.apply(null, M), hi2 = Math.max.apply(null, M), pad = (hi2 - lo2) * 0.15;
        var Ym = function (v) { return y0 + (hi2 + pad - v) / (hi2 - lo2 + 2 * pad) * ph; }, Xm = function (k) { return x0 + (k + 0.5) / M.length * pw; };
        ctx.strokeStyle = hot; ctx.setLineDash([5, 4]); ctx.beginPath(); ctx.moveTo(x0, Ym(D.infd.total)); ctx.lineTo(x0 + pw, Ym(D.infd.total)); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = "rgba(128,128,128,0.6)";
        for (var k = 0; k < N; k++) { ctx.beginPath(); ctx.arc(Xm(k), Ym(M[k]), 2.2, 0, 2 * Math.PI); ctx.fill(); }
        ctx.strokeStyle = acc; ctx.lineWidth = 2; ctx.beginPath();
        for (k = 0; k < N; k++) { if (k) ctx.lineTo(Xm(k), Ym(run[k])); else ctx.moveTo(Xm(k), Ym(run[k])); }
        ctx.stroke();
        ctx.fillStyle = mut; ctx.fillText((hi2 + pad).toFixed(2) + " b", 2, y0 + 10); ctx.fillText((lo2 - pad).toFixed(2) + " b", 2, y0 + ph);
        ctx.fillText("ladder 1", x0, y0 + ph + 16); var t3 = "ladder " + M.length; ctx.fillText(t3, x0 + pw - ctx.measureText(t3).width, y0 + ph + 16);
        ctx.fillStyle = fg; ctx.fillText("mean σ_total: each ladder (dots), running average (line)", x0, y0 - 10);
        var dev = 100 * (run[N - 1] / D.infd.total - 1);
        return "After " + N + " ladder" + (N > 1 ? "s" : "") + " the running average of σ_total is " + run[N - 1].toFixed(3) + " b, " + (dev >= 0 ? "+" : "") + dev.toFixed(2) + " % from the dilute value " + D.infd.total.toFixed(3) +
          " b that the same parameters give directly (dashed). One ladder alone scatters by about " + D.conv.pct.toFixed(1) + " %: that is why PURR makes many.";
      },
      panel: function (body, redraw) {
        body.append(el("p", { "class": "w-note" }, "U-238, ENDF/B-VIII.0 unresolved parameters at " + fmtE(D.e) + ", " + D.t + " K. OUTRAM PARK's PURR port with NJOY's random-number generator and seed; " + D.nsamp + " samples per ladder, " + D.nladr + " ladders. No File-3 background added: σ is the resonance part plus potential scattering."));
      }
    });
    var card = root.parentNode;
    card.querySelectorAll("[data-stage]").forEach(function (b) {
      b.onclick = function () {
        st.stage = b.getAttribute("data-stage"); st.t = 0;
        card.querySelectorAll("[data-stage]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
        var row = card.querySelector("[data-lads]"); if (row) row.style.display = st.stage === "many" ? "" : "none";
        frame.play();
      };
    });
    card.querySelectorAll("[data-nlad]").forEach(function (b) {
      b.onclick = function () { st.nlad = +b.getAttribute("data-nlad"); frame.redraw(); };
    });
    var row = card.querySelector("[data-lads]"); if (row) row.style.display = "none";
    return frame;
  }

  L.widget("purr", purrWidget);
})();
