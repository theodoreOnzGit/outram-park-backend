// Fission products, lessons 8-9 (how an atom hops; atoms swapping): the
// kinetic Monte Carlo behind the demos, shared by hop-worker.js (the pages
// run it there, never on the UI thread), the pages' small calculations and
// check-fission.js (node). Pure functions, no DOM.
//
// Units: energies in eV, temperatures in K, lengths in lattice spacings
// (lambda = 1) and times in vibration periods (1/nu): the attempt frequency
// nu is a source-pending number (cascade-recorded.js), so the demos count
// time in attempts and never need it. Every barrier the demos use is
// ILLUSTRATIVE and labelled so on the page.
//
// The model: an atom on a square lattice (2D, z = 4 neighbours) tries to hop
// to each neighbour; one try succeeds with probability exp(-E/kT), so the
// hop rate per direction is Gamma = nu exp(-E/kT). Time advances by the
// residence-time (BKL) rule: the wait to the next hop is exponential with
// the total rate. With Gamma per direction, the walk's D = Gamma lambda^2 on
// this lattice (<R^2> = 4 D t in 2D).
(function () {
  "use strict";
  var root = typeof window !== "undefined" ? window : self;
  // Boltzmann constant in eV/K: k_B [J/K] / e [C], both exact in the SI
  // since 2019 (CODATA 2018, Tiesinga et al. 2021).
  var K_B_EV = 1.380649e-23 / 1.602176634e-19;
  var K_B_J = 1.380649e-23;

  function kT(T) { return K_B_EV * T; }
  // The chance that one attempt clears a barrier E at temperature T.
  function boltzmann(E, T) { return Math.exp(-E / kT(T)); }

  // A small seedable generator (mulberry32), so a run can be repeated.
  function rng(seed) {
    var a = seed >>> 0;
    return function () {
      a = (a + 0x6D2B79F5) >>> 0;
      var t = a;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  var DX = [1, -1, 0, 0], DY = [0, 0, 1, -1];

  // Is lattice site (x, y) a trap? A fixed pseudo-random field (hash of the
  // site and seed), so an endless lattice needs no storage.
  function isTrap(x, y, frac, seed) {
    if (!(frac > 0)) return false;
    var h = Math.imul(x | 0, 374761393) ^ Math.imul(y | 0, 668265263) ^ Math.imul(seed | 0, 2246822519);
    h = Math.imul(h ^ (h >>> 13), 1274126177);
    h ^= h >>> 16;
    return ((h >>> 0) / 4294967296) < frac;
  }

  // One walker's hops up to time t (in 1/nu). Barrier Em everywhere, plus Eb
  // to climb out of a trap site. Returns {x, y, hops}. maxHops guards the
  // loop.
  function walk(r, p, t, maxHops) {
    var x = 0, y = 0, time = 0, hops = 0;
    var g = boltzmann(p.Em, p.T), gt = p.trapFrac > 0 ? boltzmann(p.Em + p.Eb, p.T) : g;
    for (;;) {
      var rate = 4 * (isTrap(x, y, p.trapFrac, p.seed) ? gt : g);
      time += -Math.log(1 - r()) / rate;
      if (time > t || hops >= maxHops) break;
      var k = (r() * 4) | 0;
      x += DX[k]; y += DY[k]; hops++;
    }
    return { x: x, y: y, hops: hops };
  }

  // A cloud of n walkers from the origin to time t: positions and the mean
  // squared displacement. D = <R^2>/(4t) on the square lattice.
  function cloud(p, n, t, seed, maxHops) {
    var r = rng(seed), xs = new Array(n), ys = new Array(n), s = 0, hops = 0;
    for (var i = 0; i < n; i++) {
      var w = walk(r, p, t, maxHops || 1e7);
      xs[i] = w.x; ys[i] = w.y; s += w.x * w.x + w.y * w.y; hops += w.hops;
    }
    return { xs: xs, ys: ys, msd: s / n, D: s / n / (4 * t), hops: hops / n };
  }

  // The vacancy mechanism on an L x L periodic lattice: every site holds an
  // atom except nv vacancies; a vacancy swaps with a neighbouring atom at
  // Gamma = nu exp(-Em/kT) per direction, and that atom moves one spacing
  // (an atom moves only when a vacancy comes by). Runs `jumps` vacancy
  // jumps; returns the atoms' D at the simulated vacancy fraction nv/L^2.
  // (A vacancy that picks a vacant neighbour stays: two vacancies trading
  // places changes nothing.)
  function vacancyRun(L, nv, Em, T, jumps, seed) {
    var r = rng(seed), N = L * L, occ = new Int32Array(N), dx = new Int32Array(N), dy = new Int32Array(N);
    var i, vac = [];
    for (i = 0; i < N; i++) occ[i] = i;
    // Place the vacancies at distinct random sites.
    while (vac.length < nv) {
      var s = (r() * N) | 0;
      if (occ[s] >= 0) { occ[s] = -1; vac.push(s); }
    }
    var g = boltzmann(Em, T), time = 0, sum = 0;
    for (var j = 0; j < jumps; j++) {
      time += -Math.log(1 - r()) / (nv * 4 * g);
      var v = (r() * nv) | 0, k = (r() * 4) | 0, site = vac[v];
      var x = site % L, y = (site / L) | 0;
      var nx = (x + DX[k] + L) % L, ny = (y + DY[k] + L) % L, ns = ny * L + nx;
      var a = occ[ns];
      if (a < 0) continue;
      // The atom moves the opposite way to the vacancy.
      var ox = dx[a], oy = dy[a];
      dx[a] = ox - DX[k]; dy[a] = oy - DY[k];
      sum += dx[a] * dx[a] + dy[a] * dy[a] - ox * ox - oy * oy;
      occ[site] = a; occ[ns] = -1; vac[v] = ns;
    }
    var atoms = N - nv;
    return { D: sum / atoms / (4 * time), time: time, atoms: atoms };
  }

  // Vacancy fraction: a thermal part exp(-Ef/kT) and an irradiation part
  // cIrr held fixed by the cascades (both illustrative).
  function vacancyFraction(Ef, T, cIrr) { return boltzmann(Ef, T) + (cIrr || 0); }

  // The vacancy mechanism at temperature T: simulate round(L^2 c_v) (at least
  // one) vacancies, then scale D to the exact fraction c_v (D is proportional
  // to the number of vacancies while they rarely meet).
  function vacancyD(p, T, L, jumps, seed) {
    var c = vacancyFraction(p.Ef, T, p.cIrr), nv = Math.max(1, Math.round(L * L * c));
    var run = vacancyRun(L, nv, p.Em, T, jumps, seed);
    return { D: run.D * c * L * L / nv, c: c, nv: nv };
  }

  // Least-squares line y = a + b x, with r^2.
  function fitLine(xs, ys) {
    var n = xs.length, mx = 0, my = 0, i;
    for (i = 0; i < n; i++) { mx += xs[i]; my += ys[i]; }
    mx /= n; my /= n;
    var sxx = 0, sxy = 0, syy = 0;
    for (i = 0; i < n; i++) {
      sxx += (xs[i] - mx) * (xs[i] - mx); sxy += (xs[i] - mx) * (ys[i] - my); syy += (ys[i] - my) * (ys[i] - my);
    }
    var b = sxy / sxx;
    return { slope: b, intercept: my - b * mx, r2: syy > 0 ? sxy * sxy / (sxx * syy) : 1 };
  }

  // The Arrhenius sweep: D at each temperature from the simulated hops, and
  // the slope of ln D against 1/T read back as an energy, E = -slope k_B.
  // kind "walk": the cloud (interstitial, with traps if p.trapFrac > 0),
  // `size` walkers, the time chosen so a free walker makes ~hops hops; kind
  // "vacancy": an L = size lattice and `hops` vacancy jumps. onPoint(i, pt)
  // is called after each temperature (the worker streams it).
  function sweep(kind, p, Ts, size, hops, seed, onPoint) {
    var pts = [];
    for (var i = 0; i < Ts.length; i++) {
      var T = Ts[i], D;
      if (kind === "vacancy") D = vacancyD(p, T, size, hops, seed + i).D;
      else {
        // With traps the walker waits in them, so run longer by the mean
        // slow-down 1 + x (e^{Eb/kT} - 1) to still see ~hops hops. This only
        // sets how long the simulation runs; D is measured from the walk.
        var slow = p.trapFrac > 0 ? 1 + p.trapFrac * (Math.exp(p.Eb / kT(T)) - 1) : 1;
        D = cloud({ Em: p.Em, Eb: p.Eb || 0, trapFrac: p.trapFrac || 0, T: T, seed: seed }, size, slow * hops / (4 * boltzmann(p.Em, T)), seed + 1000 + i, 50 * hops).D;
      }
      var pt = { T: T, D: D, invT: 1 / T, lnD: Math.log(D) };
      pts.push(pt);
      if (onPoint) onPoint(i, pt);
    }
    return { points: pts, fit: fitOf(pts) };
  }
  function fitOf(pts) {
    var f = fitLine(pts.map(function (q) { return q.invT; }), pts.map(function (q) { return q.lnD; }));
    return { slope: f.slope, intercept: f.intercept, r2: f.r2, E: -f.slope * K_B_EV };
  }

  // Turnbull et al. (1988)'s three-term single-atom diffusivity of Xe in UO2
  // [m^2/s], as SCIANTIX implements it (System.C, setFissionGasDiffusivity,
  // iFissionGasDiffusivity = 1, commit 2344575, MIT): T in K, F in
  // fissions/m^3/s. Read from that source on 2026-10-20.
  function turnbull(T, F) {
    var d1 = 7.6e-10 * Math.exp(-4.86e-19 / (K_B_J * T));
    var d2 = 4.0 * 1.41e-25 * Math.sqrt(F) * Math.exp(-1.91e-19 / (K_B_J * T));
    var d3 = 8.0e-40 * F;
    return { d1: d1, d2: d2, d3: d3, total: d1 + d2 + d3 };
  }

  root.HopPhysics = {
    K_B_EV: K_B_EV, K_B_J: K_B_J, kT: kT, boltzmann: boltzmann, rng: rng, isTrap: isTrap,
    walk: walk, cloud: cloud, vacancyRun: vacancyRun, vacancyFraction: vacancyFraction, vacancyD: vacancyD,
    fitLine: fitLine, sweep: sweep, fitOf: fitOf, turnbull: turnbull
  };
})();
