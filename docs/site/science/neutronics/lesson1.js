// Neutronics Lesson 1, "Meet the neutron" (gh:#906, #910–#913): the page's
// widgets. Each follows the demo layout of
// docs/claude-md/mobile-first-tutorials-and-demos.md in miniature: the
// picture fills the widget, on-screen + / − / Reset (and full screen) buttons
// sit on it, and every control is in a panel that folds ("« Hide",
// "Controls »") and starts folded on a narrow screen. Every computation here
// takes well under a millisecond a frame, so nothing needs a worker; the one
// heavy demo (the random walk) is the Monte Carlo demo itself, in an iframe
// that loads only when asked.
//
// Physics guardrails (spec §14): lambda = h/p; the packet's envelope is
// drawn separately from its carrier and is not lambda; the Heisenberg strip
// relates the two spreads only; the collision is classical kinematics only.
(function () {
  "use strict";

  // CODATA 2018 (Tiesinga et al., 2021): h and e exact, m_n and c.
  var H = 6.62607015e-34, HBAR = H / (2 * Math.PI), EV = 1.602176634e-19;
  var MN = 1.67492749804e-27, C = 299792458;

  function css(name, fallback) {
    var v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
  }
  function el(tag, attrs, text) {
    var e = document.createElement(tag);
    for (var k in attrs || {}) e.setAttribute(k, attrs[k]);
    if (text !== undefined) e.textContent = text;
    return e;
  }
  function fmtE(ev) {
    // Choose the unit after rounding, so 999.99 keV reads 1.00 MeV.
    if (ev >= 0.9995e6) return (ev / 1e6).toPrecision(3) + " MeV";
    if (ev >= 0.9995e3) return (ev / 1e3).toPrecision(3) + " keV";
    if (ev >= 0.9995) return ev.toPrecision(3) + " eV";
    if (ev >= 1e-3) return (ev * 1e3).toPrecision(3) + " meV";
    return ev.toExponential(2) + " eV";
  }
  function pct(f) {
    var p = 100 * f;
    return (p >= 0.1 ? p.toFixed(1) : p.toPrecision(2)) + " %";
  }
  function fmtLen(m) {
    var a = Math.abs(m);
    if (a >= 1e-6) return (m * 1e6).toPrecision(3) + " µm";
    if (a >= 1e-9) return (m * 1e9).toPrecision(3) + " nm";
    if (a >= 1e-11) return (m * 1e10).toPrecision(3) + " Å";
    if (a >= 1e-13) return (m * 1e12).toPrecision(3) + " pm";
    return (m * 1e15).toPrecision(3) + " fm";
  }
  function fmtSpeed(v) {
    if (v >= 1e6) return (v / 1e3).toExponential(2) + " km/s";
    if (v >= 1e3) return (v / 1e3).toPrecision(3) + " km/s";
    return v.toPrecision(3) + " m/s";
  }

  // ─── The widget frame ──────────────────────────────────────────────────
  // spec: { root, draw(ctx, w, h, zoom), tick(dt) -> keep animating?, panel(div) }
  function Frame(root, spec) {
    var self = this;
    this.zoom = 1;
    this.spec = spec;
    var canvas = root.querySelector("canvas");
    this.canvas = canvas;
    var bar = el("div", { "class": "w-bar" });
    var show = el("button", { type: "button", "class": "w-btn w-show" }, "Controls »");
    var zin = el("button", { type: "button", "class": "w-btn", "aria-label": "Zoom in" }, "+");
    var zout = el("button", { type: "button", "class": "w-btn", "aria-label": "Zoom out" }, "−");
    var reset = el("button", { type: "button", "class": "w-btn" }, "Reset");
    var full = el("button", { type: "button", "class": "w-btn", "aria-label": "Full screen" }, "⛶");
    var right = el("span", { "class": "w-right" });
    right.append(zin, zout, reset, full);
    bar.append(show, right);
    var panel = el("div", { "class": "w-panel" });
    var head = el("div", { "class": "w-head" });
    var hide = el("button", { type: "button", "class": "w-btn" }, "« Hide");
    head.append(el("b", {}, "Controls"), hide);
    var body = el("div", { "class": "w-body" });
    panel.append(head, body);
    this.status = el("div", { "class": "w-status", "aria-live": "polite" });
    root.append(bar, panel, this.status);
    function fold(f) { root.classList.toggle("folded", f); show.hidden = !f; }
    fold(window.innerWidth < 700);
    show.onclick = function () { fold(false); };
    hide.onclick = function () { fold(true); };
    zin.onclick = function () { self.zoom = Math.min(self.zoom * 1.4, 40); self.redraw(); };
    zout.onclick = function () { self.zoom = Math.max(self.zoom / 1.4, 0.25); self.redraw(); };
    reset.onclick = function () { self.zoom = 1; if (spec.reset) spec.reset(); self.redraw(); };
    full.onclick = function () {
      var on = !root.classList.contains("full");
      root.classList.toggle("full", on);
      document.body.classList.toggle("w-noscroll", on);
      full.textContent = on ? "✕" : "⛶";
      full.setAttribute("aria-label", on ? "Leave full screen" : "Full screen");
      self.redraw();
    };
    spec.panel(body, function () { self.redraw(); }, function () { self.play(); });
    this.running = false;
    this.last = 0;
    new ResizeObserver(function () { self.redraw(); }).observe(canvas);
    this.redraw();
  }
  Frame.prototype.redraw = function () {
    var c = this.canvas, dpr = window.devicePixelRatio || 1;
    var w = c.clientWidth, h = c.clientHeight;
    if (!w || !h) return;
    if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
      c.width = Math.round(w * dpr); c.height = Math.round(h * dpr);
    }
    var ctx = c.getContext("2d");
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    var s = this.spec.draw(ctx, w, h, this.zoom);
    this.status.textContent = s || "";
  };
  Frame.prototype.play = function () {
    var self = this;
    if (this.running) return;
    this.running = true;
    this.last = performance.now();
    function step(now) {
      var dt = Math.min((now - self.last) / 1000, 0.05);
      self.last = now;
      var more = self.spec.tick(dt);
      self.redraw();
      if (more) requestAnimationFrame(step); else self.running = false;
    }
    requestAnimationFrame(step);
  };

  function buttons(parent, label, options, current, onPick) {
    var g = el("div", { "class": "w-group", role: "group", "aria-label": label });
    g.append(el("span", { "class": "w-label" }, label));
    var row = el("div", { "class": "w-row" });
    options.forEach(function (o) {
      var b = el("button", { type: "button", "class": "w-opt" }, o[1]);
      if (o[0] === current) b.classList.add("on");
      b.onclick = function () {
        row.querySelectorAll(".w-opt").forEach(function (x) { x.classList.remove("on"); });
        b.classList.add("on");
        onPick(o[0]);
      };
      row.append(b);
    });
    g.append(row);
    parent.append(g);
  }
  function slider(parent, label, min, max, stepv, value, onInput) {
    var g = el("label", { "class": "w-group" });
    var out = el("span", { "class": "w-val" });
    g.append(el("span", { "class": "w-label" }, label), out);
    var s = el("input", { type: "range", min: min, max: max, step: stepv });
    s.value = value;
    s.oninput = function () { out.textContent = onInput(parseFloat(s.value)); };
    out.textContent = onInput(parseFloat(value));
    g.append(s);
    parent.append(g);
    return s;
  }
  function toggle(parent, label, value, onChange) {
    var g = el("label", { "class": "w-check" });
    var c = el("input", { type: "checkbox" });
    c.checked = value;
    c.onchange = function () { onChange(c.checked); };
    g.append(c, document.createTextNode(" " + label));
    parent.append(g);
  }
  function arrow(ctx, x0, y0, x1, y1, col) {
    var a = Math.atan2(y1 - y0, x1 - x0), L = Math.hypot(x1 - x0, y1 - y0);
    if (L < 2) return;
    ctx.strokeStyle = col; ctx.fillStyle = col; ctx.lineWidth = 2;
    ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x1, y1); ctx.stroke();
    var hs = Math.min(10, L / 2);
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x1 - hs * Math.cos(a - 0.4), y1 - hs * Math.sin(a - 0.4));
    ctx.lineTo(x1 - hs * Math.cos(a + 0.4), y1 - hs * Math.sin(a + 0.4));
    ctx.closePath(); ctx.fill();
  }

  // ─── Collision: two-body elastic kinematics, target at rest ────────────
  // Neutron mass 1, nucleus mass A (its AWR). With n the unit contact normal
  // (neutron centre to nucleus centre): v' = v − 2A/(1+A) (v·n) n,
  // V' = 2/(1+A) (v·n) n. Head-on, E'/E = ((A−1)/(A+1))².
  function collision(root) {
    var awr = { h1: +root.dataset.awrH1, c12: +root.dataset.awrC12, u238: +root.dataset.awrU238 };
    var name = { h1: "H-1", c12: "C-12", u238: "U-238" };
    var st = { nuc: "h1", e: 1e6, aim: 0, t: -1.6 };
    var RN = 0.3, SPEED = 3, T0 = -1.6, T1 = 1.8;
    function kin() {
      var A = awr[st.nuc], rN = 0.3 * Math.cbrt(A), D = RN + rN;
      var b = st.aim * D * 0.98;
      var nx = Math.sqrt(1 - (b / D) * (b / D)), ny = -b / D;
      var vx = SPEED, vy = 0, vn = vx * nx + vy * ny;
      var f = 2 * A / (1 + A), g = 2 / (1 + A);
      return {
        A: A, rN: rN, D: D, b: b, nx: nx, ny: ny,
        v1: [vx - f * vn * nx, vy - f * vn * ny], V1: [g * vn * nx, g * vn * ny],
        kept: ((vx - f * vn * nx) * (vx - f * vn * nx) + (vy - f * vn * ny) * (vy - f * vn * ny)) / (SPEED * SPEED)
      };
    }
    return new Frame(root, {
      reset: function () { st.t = T0; },
      tick: function (dt) { st.t += dt * 0.9; if (st.t >= T1) { st.t = T1; return false; } return true; },
      draw: function (ctx, w, h, zoom) {
        var k = kin(), s = Math.min(w / 12, h / 7) * zoom, cx = w / 2, cy = h / 2;
        var X = function (x) { return cx + x * s; }, Y = function (y) { return cy - y * s; };
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        var c0x = -k.D * k.nx, c0y = -k.D * k.ny, t = st.t;
        var n = t < 0 ? [c0x + SPEED * t, c0y] : [c0x + k.v1[0] * t, c0y + k.v1[1] * t];
        var N = t < 0 ? [0, 0] : [k.V1[0] * t, k.V1[1] * t];
        // The incoming line, faint.
        ctx.strokeStyle = mut; ctx.setLineDash([4, 6]); ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(X(-7), Y(c0y)); ctx.lineTo(X(c0x), Y(c0y)); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = "#b85c38";
        ctx.beginPath(); ctx.arc(X(N[0]), Y(N[1]), k.rN * s, 0, 2 * Math.PI); ctx.fill();
        ctx.fillStyle = acc;
        ctx.beginPath(); ctx.arc(X(n[0]), Y(n[1]), RN * s, 0, 2 * Math.PI); ctx.fill();
        var as = 0.45;
        if (t < 0) arrow(ctx, X(n[0]), Y(n[1]), X(n[0] + SPEED * as), Y(n[1]), fg);
        else {
          arrow(ctx, X(n[0]), Y(n[1]), X(n[0] + k.v1[0] * as), Y(n[1] + k.v1[1] * as), fg);
          arrow(ctx, X(N[0]), Y(N[1]), X(N[0] + k.V1[0] * as), Y(N[1] + k.V1[1] * as), "#b85c38");
        }
        ctx.fillStyle = fg; ctx.font = "13px system-ui, sans-serif";
        ctx.fillText("neutron", X(n[0]) - 20, Y(n[1]) - RN * s - 6);
        ctx.fillText(name[st.nuc] + " nucleus (A = " + k.A + ")", X(N[0]) - 40, Y(N[1]) + k.rN * s + 16);
        var eAfter = st.e * k.kept, theta = Math.atan2(k.v1[1], k.v1[0]) * 180 / Math.PI;
        var alpha = Math.pow((k.A - 1) / (k.A + 1), 2);
        return "Before: " + fmtE(st.e) + ". After: neutron " + fmtE(eAfter) + " (" + pct(k.kept) + " kept), nucleus " +
          fmtE(st.e - eAfter) + ", " + (k.kept < 1e-6 ? "neutron all but stopped" : "neutron turned " + Math.abs(theta).toFixed(1) + "°") +
          ". Head-on, " + name[st.nuc] + " leaves the neutron at least " + pct(alpha) + ".";
      },
      panel: function (body, redraw, play) {
        buttons(body, "Target nucleus", [["h1", "H-1"], ["c12", "C-12"], ["u238", "U-238"]], st.nuc, function (v) { st.nuc = v; st.t = T0; play(); });
        buttons(body, "Neutron energy", [[0.0253, "0.0253 eV"], [1, "1 eV"], [1e3, "1 keV"], [1e6, "1 MeV"], [2e6, "2 MeV"]], st.e,
          function (v) { st.e = v; redraw(); });
        slider(body, "Aim", 0, 1, 0.01, st.aim, function (v) {
          st.aim = v; st.t = T0; redraw();
          return v < 0.02 ? "head-on" : v > 0.9 ? "glancing" : (100 * v).toFixed(0) + " % off-centre";
        });
        var b = el("button", { type: "button", "class": "w-opt wide" }, "▶ Collide");
        b.onclick = function () { st.t = T0; play(); };
        body.append(b);
        body.append(el("p", { "class": "w-note" }, "The energy only scales the numbers: the fractions depend on the mass ratio and the aim, not on the energy. At 0.0253 eV a real nucleus is not at rest; this picture does not hold there."));
      }
    });
  }

  // ─── The wave packet: a free Gaussian, exact ───────────────────────────
  // psi(x,t) ∝ q^(-1/2) exp(−(x − v t)² / (4 σ² q)) exp(i(k0 x − ω t)),
  // q = 1 + i ħ t / (2 m σ²), k0 = p/ħ, ω = ħ k0² / (2m) (so the crests move
  // at v/2 and the envelope at v). p is relativistic, so λ = h/p holds at
  // every energy on the slider; the motion is the non-relativistic packet.
  function packet(root) {
    var st = { lg: Math.log10(0.0253), sig: 0.4e-9, re: true, prob: true, heis: false, t: 0, anim: false };
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

  function start() {
    var c = document.getElementById("collision"); if (c) collision(c).play();
    var p = document.getElementById("packet"); if (p) packet(p);
    var l = document.getElementById("levels"); if (l) levels(l).play();
    // The random-walk demo loads only when asked: it downloads and processes
    // nuclear data, which a reader skimming the page should not pay for.
    document.querySelectorAll(".iframe-slot").forEach(function (slot) {
      var b = slot.querySelector(".start-demo");
      if (!b) return;
      b.onclick = function () {
        var f = el("iframe", { src: slot.getAttribute("data-src"), title: "Neutron random-walk demo", allow: "fullscreen", loading: "eager" });
        slot.innerHTML = "";
        slot.append(f);
        slot.classList.add("live");
      };
    });
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
