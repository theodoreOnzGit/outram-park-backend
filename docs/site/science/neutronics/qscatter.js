// Neutronics lessons: quantum scattering of a neutron wave (gh:#906
// checkpoint §14). Exact partial-wave scattering from an impenetrable
// sphere whose radius is the evaluation's scattering radius AP (ENDF MF=2),
// in the centre-of-mass frame:
//
//   tan δ_l = j_l(ka) / y_l(ka)            (the hard-sphere phase shift; it
//                                           is −φ_l of NJOY's facphi,
//                                           reconr.f90:4441, ported in
//                                           njoy-outram-park-fork reconr/slbw.rs)
//   f(θ)    = (1/k) Σ (2l+1) e^{iδ_l} sin δ_l P_l(cos θ)
//   dσ/dΩ   = |f(θ)|²,   σ = (4π/k²) Σ (2l+1) sin² δ_l
//   ψ_sc    = Σ i^{l+1} (2l+1) e^{iδ_l} sin δ_l h_l⁽¹⁾(kr) P_l(cos θ),  r ≥ a
//
// with k = cwaven · A/(A+1) · √E (NJOY's cwaven, 2.196807689e-4 fm⁻¹/√eV)
// and the wave inside the sphere zero. The total wave drawn is
// e^{ikz} + ψ_sc, exact everywhere outside the sphere (not only far away).
//
// Guardrails (checkpoint §14): this is real quantum scattering from the
// simplest potential, not a model of a real nucleus: a real nucleus can be
// entered, absorbs and resonates, and its measured angular distributions
// come from evaluations (page 7). Classical scattering is not taught as
// isotropic; isotropy is named as a simplifying model.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame, buttons = L.buttons, slider = L.slider, toggle = L.toggle;
  var CWAVEN = 2.196807689091228e-4; // fm⁻¹ / √eV (NJOY phys.f90's cwaven, in fm)

  // Spherical Bessel functions j_l(x), y_l(x) for l = 0..n. y by upward
  // recurrence (stable); j by upward recurrence where x > n, otherwise by
  // Miller's downward recurrence normalised to j_0 (or j_1 near a zero of
  // sin x).
  function sphJY(n, x, j, y) {
    var s = Math.sin(x), c = Math.cos(x);
    y[0] = -c / x;
    if (n > 0) y[1] = -c / (x * x) - s / x;
    for (var l = 1; l < n; l++) y[l + 1] = (2 * l + 1) / x * y[l] - y[l - 1];
    if (x > n) {
      j[0] = s / x;
      if (n > 0) j[1] = s / (x * x) - c / x;
      for (var m = 1; m < n; m++) j[m + 1] = (2 * m + 1) / x * j[m] - j[m - 1];
      return;
    }
    var top = n + 16 + Math.ceil(x), jp = 0, jc = 1e-300, k;
    for (k = top; k > 0; k--) {
      var jm = (2 * k + 1) / x * jc - jp;
      jp = jc; jc = jm;
      if (k - 1 <= n) j[k - 1] = jc;
      if (Math.abs(jc) > 1e250) { // rescale to stay finite
        for (var q = k - 1; q <= n; q++) j[q] *= 1e-250;
        jc *= 1e-250; jp *= 1e-250;
      }
    }
    var j0 = s / x, j1 = s / (x * x) - c / x;
    var scale = Math.abs(j0) > Math.abs(j1) ? j0 / j[0] : j1 / j[1];
    for (k = 0; k <= n; k++) j[k] *= scale;
  }
  function legendre(n, mu, p) {
    p[0] = 1; if (n > 0) p[1] = mu;
    for (var l = 1; l < n; l++) p[l + 1] = ((2 * l + 1) * mu * p[l] - l * p[l - 1]) / (l + 1);
  }

  // The scattering problem at one energy: phase shifts, the angular
  // distribution on a μ grid, and the total cross section.
  function solve(A, apFm, eEv) {
    var k = CWAVEN * A / (A + 1) * Math.sqrt(eEv), ka = k * apFm;
    var n = Math.ceil(ka) + 14, j = new Float64Array(n + 1), y = new Float64Array(n + 1);
    sphJY(n, ka, j, y);
    var sd = new Float64Array(n + 1), cd = new Float64Array(n + 1), lmax = 0, sig = 0;
    for (var l = 0; l <= n; l++) {
      var d = Math.atan(j[l] / y[l]);
      sd[l] = Math.sin(d); cd[l] = Math.cos(d);
      sig += (2 * l + 1) * sd[l] * sd[l];
      if (Math.abs(sd[l]) > 1e-3) lmax = l;
    }
    sig *= 4 * Math.PI / (k * k); // fm²
    var NM = 361, mu = new Float64Array(NM), f = new Float64Array(NM), p = new Float64Array(n + 1), fmax = 0;
    for (var i = 0; i < NM; i++) {
      var th = Math.PI * i / (NM - 1);
      mu[i] = Math.cos(th);
      legendre(n, mu[i], p);
      var re = 0, im = 0;
      for (var m = 0; m <= n; m++) {
        var w = (2 * m + 1) * sd[m] * p[m];
        re += w * cd[m]; im += w * sd[m];
      }
      f[i] = (re * re + im * im) / (k * k); // fm²/sr
      if (f[i] > fmax) fmax = f[i];
    }
    return { k: k, ka: ka, n: n, sd: sd, cd: cd, lmax: lmax, sig: sig, f: f, fmax: fmax, NM: NM };
  }

  // The coefficients c_l = i^{l+1} (2l+1) e^{iδ_l} sin δ_l of ψ_sc.
  function coefficients(S) {
    var n = S.n, cr = new Float64Array(n + 1), ci = new Float64Array(n + 1);
    for (var l = 0; l <= n; l++) {
      var ar = (2 * l + 1) * S.sd[l] * S.cd[l], ai = (2 * l + 1) * S.sd[l] * S.sd[l];
      var ph = (l + 1) % 4; // multiply (ar + i ai) by i^{l+1}
      cr[l] = ph === 0 ? ar : ph === 1 ? -ai : ph === 2 ? -ar : ai;
      ci[l] = ph === 0 ? ai : ph === 1 ? ar : ph === 2 ? -ai : -ar;
    }
    return { cr: cr, ci: ci };
  }
  // ψ_sc at distance r ≥ a and cosine mu from the beam: [Re, Im]. The work
  // arrays j, y, p have length S.n + 1.
  function scattered(S, cf, r, mu, j, y, p) {
    sphJY(S.n, S.k * r, j, y); legendre(S.n, mu, p);
    var re = 0, im = 0;
    for (var m = 0; m <= S.n; m++) {
      var hr = j[m] * p[m], hi = y[m] * p[m]; // h_m P_m, h = j + i y
      re += cf.cr[m] * hr - cf.ci[m] * hi; im += cf.cr[m] * hi + cf.ci[m] * hr;
    }
    return [re, im];
  }

  // The wave on a grid: Re and Im of the total wave e^{ikz} + ψ_sc and of
  // ψ_sc alone, so a frame only rotates the phase.
  function field(S, apFm, half, NX, NZ) {
    var n = S.n, j = new Float64Array(n + 1), y = new Float64Array(n + 1), p = new Float64Array(n + 1), cf = coefficients(S);
    var tr = new Float32Array(NX * NZ), ti = new Float32Array(NX * NZ), sr = new Float32Array(NX * NZ), si = new Float32Array(NX * NZ);
    var inside = new Uint8Array(NX * NZ);
    for (var iz = 0; iz < NZ; iz++) {
      var x = (iz + 0.5) / NZ * 2 * half - half; // transverse
      for (var ix = 0; ix < NX; ix++) {
        var z = (ix + 0.5) / NX * 2 * half * NX / NZ - half * NX / NZ; // along the beam
        var r = Math.hypot(x, z), q = iz * NX + ix;
        if (r < apFm) { inside[q] = 1; continue; }
        var w = scattered(S, cf, r, z / r, j, y, p);
        sr[q] = w[0]; si[q] = w[1];
        tr[q] = Math.cos(S.k * z) + w[0]; ti[q] = Math.sin(S.k * z) + w[1];
      }
    }
    return { tr: tr, ti: ti, sr: sr, si: si, inside: inside };
  }

  function qscatter(root) {
    var nuc = {
      c12: { name: "C-12", A: +root.dataset.awrC12, ap: 10 * +root.dataset.apC12 },
      o16: { name: "O-16", A: +root.dataset.awrO16, ap: 10 * +root.dataset.apO16 },
      u238: { name: "U-238", A: +root.dataset.awrU238, ap: 10 * +root.dataset.apU238 }
    };
    var st = { nuc: "u238", lg: Math.log10(2e6), show: "total", phase: 0, anim: true, samples: [], rnd: 12345 };
    var S = null, F = null, key = "", NX = 0, NZ = 0, half = 0, off = document.createElement("canvas"), img = null;
    function rand() { // xorshift, deterministic so a reload shows the same samples
      st.rnd ^= st.rnd << 13; st.rnd ^= st.rnd >>> 17; st.rnd ^= st.rnd << 5;
      return (st.rnd >>> 0) / 4294967296;
    }
    function update(aspect) {
      var N = nuc[st.nuc], E = Math.pow(10, st.lg), k2 = st.nuc + st.lg.toFixed(3) + aspect.toFixed(2);
      if (k2 === key) return;
      key = k2;
      S = solve(N.A, N.ap, E);
      half = Math.max(4 * N.ap, 1.25 * 2 * Math.PI / S.k);
      NZ = 90; NX = Math.max(60, Math.min(200, Math.round(NZ * aspect)));
      F = field(S, N.ap, half, NX, NZ);
      off.width = NX; off.height = NZ;
      img = off.getContext("2d").createImageData(NX, NZ);
    }
    // Sample μ from dσ/dΩ (per unit solid angle, so μ itself is sampled
    // uniformly against it) by rejection; return θ.
    function sample() {
      for (;;) {
        var mu = 2 * rand() - 1, th = Math.acos(mu), i = th / Math.PI * (S.NM - 1);
        var i0 = Math.floor(i), t = i - i0, fv = S.f[i0] + (i0 + 1 < S.NM ? t * (S.f[i0 + 1] - S.f[i0]) : 0);
        if (rand() * S.fmax <= fv) return th;
      }
    }
    return new Frame(root, {
      reset: function () { st.samples = []; },
      tick: function (dt) { if (!st.anim) return false; st.phase += dt * 2.2; return true; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        var wide = w >= 1.25 * h, y0 = 56;
        var fx = 6, fy = y0, fw = wide ? w * 0.56 : w - 12, fh = wide ? h - y0 - 8 : (h - y0) * 0.5;
        var px = wide ? fx + fw + 10 : 6, py = wide ? y0 : fy + fh + 8, pw = wide ? w - px - 6 : w - 12, ph = wide ? h - y0 - 8 : h - py - 6;
        update(fw / fh);
        var N = nuc[st.nuc];
        // The wave: Re[ψ e^{-iφ}], teal positive, brown negative.
        var re = st.show === "total" ? F.tr : F.sr, im = st.show === "total" ? F.ti : F.si;
        var c = Math.cos(st.phase), s = Math.sin(st.phase), d = img.data, amax = st.show === "total" ? 2 : 1.2;
        for (var q = 0; q < NX * NZ; q++) {
          var v = (re[q] * c + im[q] * s) / amax, o = 4 * q;
          if (F.inside[q]) { d[o] = 60; d[o + 1] = 60; d[o + 2] = 60; d[o + 3] = 255; continue; }
          v = Math.max(-1, Math.min(1, v));
          if (v >= 0) { d[o] = 31; d[o + 1] = 111; d[o + 2] = 120; } else { d[o] = 184; d[o + 1] = 92; d[o + 2] = 56; }
          d[o + 3] = Math.round(255 * Math.abs(v));
        }
        off.getContext("2d").putImageData(img, 0, 0);
        ctx.save();
        ctx.beginPath(); ctx.rect(fx, fy, fw, fh); ctx.clip();
        var zw = fw * zoom, zh = fh * zoom;
        ctx.imageSmoothingEnabled = true;
        ctx.drawImage(off, fx + (fw - zw) / 2, fy + (fh - zh) / 2, zw, zh);
        // The sphere, drawn crisp over the grid.
        var pxPerFm = zh / (2 * half), cx = fx + fw / 2, cy = fy + fh / 2;
        ctx.fillStyle = "#3c3c3c"; ctx.beginPath(); ctx.arc(cx, cy, Math.max(2, N.ap * pxPerFm), 0, 2 * Math.PI); ctx.fill();
        // The last sampled direction, launched from the nucleus.
        if (st.samples.length) {
          var th = st.samples[st.samples.length - 1], sgn = st.samples.length % 2 ? 1 : -1, len = Math.min(fw, fh) * 0.45;
          L.arrow(ctx, cx, cy, cx + len * Math.cos(th), cy - sgn * len * Math.sin(th), fg);
        }
        ctx.restore();
        ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(fx, fy, fw, fh);
        ctx.font = "13px system-ui, sans-serif"; ctx.fillStyle = fg;
        ctx.fillText("neutron wave →", fx + 6, fy + 16);
        var lamFm = 2 * Math.PI / S.k, bar = lamFm * pxPerFm;
        if (bar > 8 && bar < fw * 0.6) {
          ctx.strokeStyle = fg; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(fx + 8, fy + fh - 10); ctx.lineTo(fx + 8 + bar, fy + fh - 10); ctx.stroke();
          ctx.fillText("λ = " + lamFm.toPrecision(3) + " fm", fx + 14 + bar, fy + fh - 6);
        }
        // The angular distribution, polar: forward (θ = 0) to the right.
        var pcx = px + pw / 2, pcy = py + ph / 2 + 8, R = Math.min(pw / 2, (ph - 20) / 2) * 0.92;
        ctx.strokeStyle = mut; ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(pcx - R, pcy); ctx.lineTo(pcx + R, pcy); ctx.moveTo(pcx, pcy - R); ctx.lineTo(pcx, pcy + R); ctx.stroke();
        // Isotropic with the same σ, for comparison: dσ/dΩ = σ/4π.
        var iso = S.sig / (4 * Math.PI) / S.fmax * R;
        ctx.setLineDash([4, 4]); ctx.beginPath(); ctx.arc(pcx, pcy, iso, 0, 2 * Math.PI); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = "rgba(31, 111, 120, 0.25)"; ctx.strokeStyle = acc; ctx.lineWidth = 2;
        ctx.beginPath();
        for (var sgn2 = 1; sgn2 >= -1; sgn2 -= 2) {
          for (var i = 0; i < S.NM; i++) {
            var ii = sgn2 === 1 ? i : S.NM - 1 - i, t2 = Math.PI * ii / (S.NM - 1), r2 = S.f[ii] / S.fmax * R;
            var X = pcx + r2 * Math.cos(t2), Y = pcy - sgn2 * r2 * Math.sin(t2);
            if (i === 0 && sgn2 === 1) ctx.moveTo(X, Y); else ctx.lineTo(X, Y);
          }
        }
        ctx.closePath(); ctx.fill(); ctx.stroke();
        for (var k = 0; k < st.samples.length; k++) {
          var tk = st.samples[k], sk = k % 2 ? -1 : 1, last = k === st.samples.length - 1;
          ctx.strokeStyle = last ? fg : "rgba(120, 120, 120, 0.35)"; ctx.lineWidth = last ? 2 : 1;
          ctx.beginPath(); ctx.moveTo(pcx, pcy); ctx.lineTo(pcx + R * Math.cos(tk), pcy - sk * R * Math.sin(tk)); ctx.stroke();
        }
        ctx.fillStyle = fg;
        ctx.fillText("chance of each direction", px + 4, py + 14);
        ctx.fillStyle = mut;
        ctx.fillText("forward", pcx + R - 50, pcy + 16);
        ctx.fillText("back", pcx - R + 2, pcy + 16);
        var fb = S.f[0] / S.f[S.NM - 1];
        return N.name + ", E = " + fmtE(Math.pow(10, st.lg)) + " (lab). Sphere radius a = " + N.ap.toPrecision(4) + " fm (the evaluation's AP), λ = " +
          lamFm.toPrecision(3) + " fm, ka = " + S.ka.toPrecision(3) + ": partial waves l = 0…" + S.lmax + " matter. σ = " + (S.sig / 100).toPrecision(4) +
          " b (4πa² = " + (4 * Math.PI * N.ap * N.ap / 100).toPrecision(4) + " b). Forward/backward = " + fb.toPrecision(3) + "." +
          (st.samples.length ? " Sampled " + st.samples.length + ": last θ = " + (st.samples[st.samples.length - 1] * 180 / Math.PI).toFixed(1) + "°." : "");
      },
      panel: function (body, redraw, play) {
        buttons(body, "Target nucleus", [["c12", "C-12"], ["o16", "O-16"], ["u238", "U-238"]], st.nuc, function (v) { st.nuc = v; st.samples = []; redraw(); });
        slider(body, "Neutron energy", 4, Math.log10(2e7), 0.01, st.lg, function (v) {
          st.lg = v; st.samples = []; redraw(); return fmtE(Math.pow(10, v));
        });
        buttons(body, "Show", [["total", "Whole wave"], ["sc", "Scattered wave only"]], st.show, function (v) { st.show = v; redraw(); });
        var b = el("button", { type: "button", "class": "w-opt wide" }, "Sample one scattering angle");
        b.onclick = function () { st.samples.push(sample()); if (st.samples.length > 60) st.samples.shift(); redraw(); };
        body.append(b);
        toggle(body, "Animate", st.anim, function (v) { st.anim = v; if (v) play(); });
        body.append(el("p", { "class": "w-note" }, "The dashed circle is isotropic scattering with the same total: every direction equally likely. The energy slider is spaced by powers of ten, 10 keV to 20 MeV."));
      }
    });
  }
  L.widget("qscatter", qscatter);
  // For the tests (tests/qscatter.test.js under node): the pure functions.
  L.qscatterMath = { sphJY: sphJY, legendre: legendre, solve: solve, coefficients: coefficients, scattered: scattered, field: field, CWAVEN: CWAVEN };
})();
