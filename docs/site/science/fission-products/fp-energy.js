// The incident-energy slider shared by Fission products lessons 2 and 3
// (maintainer design 2026-10-20: "tie lesson 3 to lesson 2's energy
// slider"). One log scale, one remembered value: the page reads ?e=<eV>
// first, then the value the other lesson last set (localStorage, per
// viewer, optional), and writes the value back as the slider moves, so
// moving from one lesson to the other keeps the energy. Pure helpers, no
// drawing; check-fission.js loads it under node.
(function () {
  "use strict";
  var W = typeof window !== "undefined" ? window : global;
  var KEY = "fp-energy-ev";
  var STEPS = 400; // slider positions over the whole log range

  // Slider position (0..STEPS) <-> energy [eV], log scale on [lo, hi].
  function toEnergy(pos, lo, hi) { return lo * Math.pow(hi / lo, pos / STEPS); }
  function toPos(e, lo, hi) { return Math.round(STEPS * Math.log(e / lo) / Math.log(hi / lo)); }
  function clamp(e, lo, hi) { return Math.min(Math.max(e, lo), hi); }

  // The energy a page should start at: ?e=, else the remembered one, else `fallback`.
  function initial(fallback) {
    try {
      var m = /[?&]e=([^&]*)/.exec(W.location ? W.location.search : "");
      if (m && isFinite(+decodeURIComponent(m[1]))) return +decodeURIComponent(m[1]);
    } catch (e) { /* no location under node */ }
    try {
      var v = W.localStorage && W.localStorage.getItem(KEY);
      if (v !== null && v !== undefined && isFinite(+v) && +v > 0) return +v;
    } catch (e) { /* storage blocked: start at the fallback */ }
    return fallback;
  }
  function remember(e) {
    try { if (W.localStorage) W.localStorage.setItem(KEY, String(e)); } catch (err) { /* optional */ }
  }

  // "0.0253 eV", "500 keV", "14 MeV": three significant figures.
  function label(e) {
    var u = e >= 0.9995e6 ? [1e6, " MeV"] : e >= 0.9995e3 ? [1e3, " keV"] : e >= 1e-3 ? [1, " eV"] : [1e-6, " µeV"];
    return String(+(e / u[0]).toPrecision(3)) + u[1];
  }

  // The tabulated energies that bracket e, and the linear weight of the
  // upper one: the interpolation fission-yields-data applies to the yields
  // (and outram-mc-libs to the spectrum grid), linear in E.
  function bracket(grid, e) {
    var n = grid.length;
    if (n < 2 || e <= grid[0]) return [0, 0, 0];
    if (e >= grid[n - 1]) return [n - 2, n - 1, 1];
    var i = 0;
    while (grid[i + 1] <= e) i++;
    return [i, i + 1, (e - grid[i]) / (grid[i + 1] - grid[i])];
  }

  // The lowest incident energy at which a lesson may show a parent
  // FISSIONING (gh:#1307). U-235, U-233 and Pu-239 are fissile: a thermal
  // neutron splits them, so they start at the slider's bottom. U-238 is
  // fissionable but not fissile: a slow neutron is captured (U-238 + n ->
  // U-239), and it splits only on a fast neutron. Its fission cross section in
  // ENDF/B-VIII.0 (MF=3/MT=18, read from reference-data/endf/n-092_U_238.endf,
  // 2026-11-06, lin-lin) is 0 b at 0.0253 eV in MF=3, 2.8e-4 b at 500 keV,
  // 0.015 b at 1 MeV, 0.36 b at 1.5 MeV and 0.54 b at 2 MeV: the threshold
  // lies between 1 and 1.5 MeV. U-238 starts at 500 keV, the lowest energy
  // its evaluated yields cover, as in lessons 1 and 2. The evaluation still
  // tabulates U-238's MF=5 spectrum down to 1e-5 eV; that table is
  // formal, and the lessons do not show it.
  var LOWEST = { "U-238": 5e5 };
  function lowest(name) { return LOWEST[name] || 0; }

  W.FPEnergy = { STEPS: STEPS, toEnergy: toEnergy, toPos: toPos, clamp: clamp, initial: initial, remember: remember, label: label, bracket: bracket, lowest: lowest };
})();
