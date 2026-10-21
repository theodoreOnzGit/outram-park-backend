// Neutronics lessons: matter waves (gh:#906 checkpoint §13).
// Physics guardrails: lambda = h/p; the packet's envelope is drawn
// separately from its carrier and is not lambda; the Heisenberg strip
// relates the two spreads only, never the mean speed.
(function () {
  "use strict";
  var L = window.Lesson;
  var H = L.H, HBAR = L.HBAR, EV = L.EV, MN = L.MN, C = L.C;
  var css = L.css, el = L.el, fmtE = L.fmtE, pct = L.pct, fmtLen = L.fmtLen, fmtSpeed = L.fmtSpeed;
  var Frame = L.Frame, buttons = L.buttons, slider = L.slider, toggle = L.toggle, arrow = L.arrow;

  // ─── The wave packet: a free Gaussian, exact ───────────────────────────
  // psi(x,t) ∝ q^(-1/2) exp(−(x − v t)² / (4 σ² q)) exp(i(k0 x − ω t)),
  // q = 1 + i ħ t / (2 m σ²), k0 = p/ħ, ω = ħ k0² / (2m) (so the crests move
  // at v/2 and the envelope at v). p is relativistic, so λ = h/p holds at
  // every energy on the slider; the motion is the non-relativistic packet.
  function packet(root) {
    var st = { lg: Math.log10(0.0253), sig: 0.4e-9, re: true, prob: true, heis: false, t: 0, anim: true };
    function phys() {
      var T = Math.pow(10, st.lg) * EV, mc2 = MN * C * C;
      var p = Math.sqrt(T * T + 2 * T * mc2) / C;
      var v = p * C * C / (T + mc2);
      return { E: Math.pow(10, st.lg), p: p, v: v, lam: H / p, k0: p / HBAR };
    }
    return new Frame(root, {
      reset: function () { st.t = 0; },
      tick: function (dt) {
        if (!st.anim) return false;
        var P = phys(), T = 4 * st.sig / P.v;
        st.t += dt * T / 3;
        if (st.t > T) st.t = -T;
        return true;
      },
      draw: function (ctx, w, h, zoom) {
        var P = phys(), fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        var hTop = st.heis ? h * 0.62 : h, half = 6 * st.sig / zoom, x0 = 0;
        var mid = hTop * 0.55, amp = hTop * 0.38;
        var X = function (x) { return (x - x0 + half) / (2 * half) * w; };
        ctx.strokeStyle = mut; ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(0, mid); ctx.lineTo(w, mid); ctx.stroke();
        var n = Math.max(400, Math.round(w * 2)), dx = 2 * half / n;
        var tau = HBAR * st.t / (2 * MN * st.sig * st.sig), omega = HBAR * P.k0 * P.k0 / (2 * MN);
        // 1/q = (1 − iτ)/(1 + τ²); q^(-1/2) has modulus (1+τ²)^(-1/4).
        var qq = 1 + tau * tau, mod = Math.pow(qq, -0.25), argq = -0.5 * Math.atan(tau);
        var drawCarrier = P.lam / (2 * half) * w >= 4;
        var reP = [], envP = [], probP = [];
        for (var i = 0; i <= n; i++) {
          var x = x0 - half + i * dx, u = x - P.v * st.t;
          var a = -(u * u) / (4 * st.sig * st.sig * qq);           // real part of the exponent
          var b = (u * u) * tau / (4 * st.sig * st.sig * qq);       // imaginary part
          var envelope = mod * Math.exp(a);
          var phase = b + argq + P.k0 * x - omega * st.t;
          reP.push(envelope * Math.cos(phase)); envP.push(envelope); probP.push(envelope * envelope);
        }
        function line(arr, sign, col, width, dash) {
          ctx.strokeStyle = col; ctx.lineWidth = width; ctx.setLineDash(dash || []);
          ctx.beginPath();
          for (var j = 0; j <= n; j++) {
            var px = j / n * w, py = mid - sign * arr[j] * amp;
            if (j) ctx.lineTo(px, py); else ctx.moveTo(px, py);
          }
          ctx.stroke(); ctx.setLineDash([]);
        }
        if (st.prob) {
          ctx.fillStyle = "rgba(184, 92, 56, 0.25)";
          ctx.beginPath(); ctx.moveTo(0, mid);
          for (var j = 0; j <= n; j++) ctx.lineTo(j / n * w, mid - probP[j] * amp);
          ctx.lineTo(w, mid); ctx.closePath(); ctx.fill();
        }
        if (st.re) {
          line(envP, 1, mut, 1, [5, 4]); line(envP, -1, mut, 1, [5, 4]);
          if (drawCarrier) line(reP, 1, acc, 1.5);
        }
        // A one-wavelength scale bar, where it can be drawn.
        ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif";
        var lamPx = P.lam / (2 * half) * w;
        if (st.re && drawCarrier && lamPx < w / 2) {
          var y = hTop - 14;
          ctx.strokeStyle = fg; ctx.lineWidth = 2;
          ctx.beginPath(); ctx.moveTo(12, y); ctx.lineTo(12 + lamPx, y); ctx.stroke();
          ctx.fillText("λ", 12 + lamPx + 6, y + 4);
        }
        ctx.fillText("window " + fmtLen(2 * half) + " wide", w - 150, hTop - 10);
        if (st.heis) {
          var sv = HBAR / (2 * MN * st.sig), top = hTop + 10, hh = h - top - 24;
          ctx.strokeStyle = mut; ctx.beginPath(); ctx.moveTo(0, top + hh); ctx.lineTo(w, top + hh); ctx.stroke();
          ctx.fillStyle = "rgba(31, 111, 120, 0.3)"; ctx.beginPath(); ctx.moveTo(0, top + hh);
          for (var k = 0; k <= 200; k++) {
            var z = -4 + 8 * k / 200;
            ctx.lineTo(k / 200 * w, top + hh - hh * Math.exp(-z * z / 2));
          }
          ctx.lineTo(w, top + hh); ctx.closePath(); ctx.fill();
          ctx.fillStyle = fg;
          ctx.fillText("speed spread: " + fmtSpeed(P.v) + " ± " + fmtSpeed(sv) + " (axis ±4σ)", 8, top + 14);
        }
        var s = "E = " + fmtE(P.E) + ", v = " + fmtSpeed(P.v) + ", λ = h/p = " + fmtLen(P.lam) +
          ". Envelope width σx = " + fmtLen(st.sig) + " (set separately).";
        if (st.re && !drawCarrier) s += " The carrier is too fine to draw at this zoom: about " +
          Math.round(2 * st.sig / P.lam).toLocaleString() + " wavelengths fit across 2σx. Zoom in to see it.";
        if (st.anim) s += " t = " + (st.t * 1e12).toPrecision(3) + " ps.";
        return s;
      },
      panel: function (body, redraw, play) {
        slider(body, "Neutron energy", -3, 7.3, 0.01, st.lg, function (v) {
          st.lg = v; st.t = 0; redraw(); return fmtE(Math.pow(10, v));
        });
        slider(body, "Localisation σx", -10, Math.log10(3e-9), 0.01, Math.log10(st.sig), function (v) {
          st.sig = Math.pow(10, v); st.t = 0; redraw(); return fmtLen(st.sig);
        });
        toggle(body, "Show wavefunction (carrier and envelope)", st.re, function (v) { st.re = v; redraw(); });
        toggle(body, "Show probability density", st.prob, function (v) { st.prob = v; redraw(); });
        toggle(body, "Animate", st.anim, function (v) { st.anim = v; if (v) play(); else { st.t = 0; redraw(); } });
        toggle(body, "Heisenberg: show the spread of speeds", st.heis, function (v) { st.heis = v; redraw(); });
        body.append(el("p", { "class": "w-note" }, "The energy slider changes the carrier's wavelength; the localisation slider changes the envelope. They are independent. The slider is spaced by powers of ten so it can reach from 1 meV to 20 MeV."));
      }
    });
  }

  // ─── Standing waves and a ladder of levels (illustration) ──────────────
  function levels(root) {
    var st = { n: 1, t: 0, anim: true };
    return new Frame(root, {
      reset: function () { st.n = 1; },
      tick: function (dt) { st.t += dt; return st.anim; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        var lw = Math.min(140, w * 0.32), pad = 16, top = 72, bot = h - 24, E = function (n) { return n * n; };
        var yE = function (n) { return bot - (bot - top) * E(n) / E(4) * zoom; };
        ctx.font = "13px system-ui, sans-serif";
        for (var n = 1; n <= 4; n++) {
          var y = yE(n);
          if (y < 0) continue;
          ctx.strokeStyle = n === st.n ? acc : mut; ctx.lineWidth = n === st.n ? 3 : 1.5;
          ctx.beginPath(); ctx.moveTo(pad, y); ctx.lineTo(lw, y); ctx.stroke();
          ctx.fillStyle = fg; ctx.fillText(n === 1 ? "n = 1, ground state" : "n = " + n, pad, y - 5);
        }
        ctx.fillStyle = mut; ctx.fillText("energy ↑ (∝ n²)", pad, h - 6);
        var bx = lw + 24, bw = w - bx - pad, my = h / 2, amp = Math.min(h * 0.35, 120);
        ctx.strokeStyle = fg; ctx.lineWidth = 3;
        ctx.beginPath(); ctx.moveTo(bx, my - amp - 10); ctx.lineTo(bx, my + amp + 10);
        ctx.moveTo(bx + bw, my - amp - 10); ctx.lineTo(bx + bw, my + amp + 10); ctx.stroke();
        ctx.strokeStyle = acc; ctx.lineWidth = 2; ctx.beginPath();
        var c = Math.cos(2 * Math.PI * 0.25 * E(st.n) * st.t);
        for (var i = 0; i <= 200; i++) {
          var x = i / 200, y = my - amp * c * Math.sin(st.n * Math.PI * x);
          if (i) ctx.lineTo(bx + x * bw, y); else ctx.moveTo(bx + x * bw, y);
        }
        ctx.stroke();
        return "Level n = " + st.n + ": " + st.n + " half-wavelength" + (st.n > 1 ? "s" : "") + " fit in the box, energy " + E(st.n) + " × the ground state's.";
      },
      panel: function (body, redraw, play) {
        buttons(body, "Allowed state", [[1, "n = 1"], [2, "n = 2"], [3, "n = 3"], [4, "n = 4"]], st.n, function (v) { st.n = v; redraw(); });
        toggle(body, "Animate", st.anim, function (v) { st.anim = v; if (v) play(); });
        body.append(el("p", { "class": "w-note" }, "A wave in a box can only fit whole half-wavelengths, so only these patterns, and these energies, exist. Nothing in between is allowed."));
      }
    });
  }
  L.widget("packet", packet);
  L.widget("levels", levels);
})();
