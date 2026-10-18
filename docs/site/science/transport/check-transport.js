// The transport lessons' physics, checked under node (no browser):
// `node docs/site/science/transport/check-transport.js`, run by
// scripts/build-pages.sh beside the neutronics checks, so a regression fails
// the Pages build. Each check says what it compares against. Results on
// 2026-10-18 are in the comments and in the lines the script prints.
"use strict";
const fs = require("fs"), path = require("path");
global.window = {}; window.Lesson = { widget() {}, css() {}, el() {} };
eval(fs.readFileSync(path.join(__dirname, "collision-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "transport.js"), "utf8"));
const T = window.Lesson.transportMath, CD = window.COLLISION_DATA;
let failed = 0;
function check(name, value, limit) {
  const ok = value <= limit;
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}: ${value.toExponential(2)} (limit ${limit})`);
}
const bits = (x) => { const b = new DataView(new ArrayBuffer(8)); b.setFloat64(0, x); return b.getBigUint64(0); };

// 1. The generator against values PINNED in the Rust crate: petir::rng::lcg
//    tests::moved_stream_is_pinned (crates/petir/src/rng/lcg.rs:278-296):
//    the bit patterns of the first four draws from seed 1, the state after
//    them, future_seed(1_000_000, 42), and init_seed(7, 3, 12345).
//    2026-10-18: all equal, so the JavaScript is the Rust generator.
{
  let s = 1n; const got = [];
  for (let i = 0; i < 4; i++) { const p = T.prn(s); s = p.state; got.push(bits(p.x)); }
  const want = [0x3fe6bf040c43ddedn, 0x3fe8188b145e9089n, 0x3fd520ab6528f072n, 0x3fdde6638ab31967n];
  check("RNG: first four draws from seed 1 vs the Rust crate's pinned bit patterns (mismatches)", got.filter((g, i) => g !== want[i]).length, 0);
  check("RNG: the state after four draws vs the pinned 0x620355cd119357c5 (mismatch)", s === 0x620355cd119357c5n ? 0 : 1, 0);
  check("RNG: future_seed(1e6, 42) vs the pinned 0xe9e8eeadb5d2156a (mismatch)", T.futureSeed(1000000n, 42n) === 0xe9e8eeadb5d2156an ? 0 : 1, 0);
  check("RNG: init_seed(7, 3, 12345) = future_seed(7 * 152917, 12345 + 3) vs the pinned 0xda85a4b030a7a755 (mismatch)", T.futureSeed(7n * T.DEFAULT_STRIDE, 12345n + 3n) === 0xda85a4b030a7a755n ? 0 : 1, 0);
}

// 2. The jump-ahead is n steps of the recurrence, for several n and seeds.
{
  let bad = 0;
  for (const [n, seed] of [[0, 5n], [1, 5n], [17, 123456789n], [1000, 987654321012345n], [4097, 42n]]) {
    let s = seed; for (let i = 0; i < n; i++) s = T.step(s);
    if (s !== T.futureSeed(BigInt(n), seed)) bad++;
  }
  check("RNG: future_seed(n) equals n single steps (mismatches)", bad, 0);
}

// 3. Streams. A history's seed is one stride past the previous history's, in the
//    same generation (transport_csg.rs run_keff_csg_par_progress); and the
//    aliasing the page describes (gh:#700, open at 86bf1c420): an auxiliary
//    stream a multiple of the stride from a history's seed IS another history's
//    seed. This is arithmetic about the layout, not a statement about the Rust
//    crate's current state, which the page dates.
{
  const a = T.historySeed(1, 7, 41), b = T.historySeed(1, 7, 42);
  check("streams: history i+1 starts one stride after history i (mismatch)", b === T.futureSeed(T.DEFAULT_STRIDE, a) ? 0 : 1, 0);
  check("streams: an auxiliary stream 5 strides on from history i is history i+5's (gh:#700; mismatch)", T.futureSeed(5n * T.DEFAULT_STRIDE, a) === T.historySeed(1, 7, 46) ? 0 : 1, 0);
  check("streams: generations are 2^40 draws apart, far more than 1e6 histories' strides (ratio of 1e6 strides to the gap)", 1e6 * Number(T.DEFAULT_STRIDE) / Number(T.GEN_STRIDE), 0.2);
}

// 3b. The widget's claim: with a stream per history the result of every history is the same under any
//     number of workers (the value is a function of (seed, generation, history) only); with one shared
//     stream it is not. 2026-10-18: the second count is printed.
{
  const N = 400, own = T.ownStreams(N, 7, 3), again = T.ownStreams(N, 7, 3);
  check("streams: a stream per history gives the same result for every history on a rerun (histories that differ)", T.changed(own, again), 0);
  check("streams: ... and a history's result does not depend on the others: history 17 computed alone equals its entry",
    Math.abs(T.toyHistory(T.historySeed(7, 3, 17)) - own[17]), 0);
  const s1 = T.sharedStream(N, 7, 1), s4 = T.sharedStream(N, 7, 4);
  check("streams: a shared stream gives some histories different numbers when the workers change (0 if it does)", T.changed(s1, s4) > N / 2 ? 0 : 1, 0);
  console.log(`     shared stream: ${T.changed(s1, s4)} of ${N} histories change between 1 and 4 workers; own streams: 0`);
}

// 4. The uniforms: mean, variance and 100-bin chi-square over 1e6 draws.
{
  let s = 12345n, n = 1000000, m = 0, v = 0; const cnt = new Array(100).fill(0);
  for (let i = 0; i < n; i++) { const p = T.prn(s); s = p.state; m += p.x; v += p.x * p.x; cnt[Math.floor(p.x * 100)]++; }
  m /= n; v = v / n - m * m;
  check("RNG: mean of 1e6 draws vs 1/2, in standard errors", Math.abs(m - 0.5) / Math.sqrt(1 / 12 / n), 4);
  check("RNG: variance of 1e6 draws vs 1/12 (relative)", Math.abs(v * 12 - 1), 0.01);
  let c2 = 0; for (const c of cnt) c2 += (c - n / 100) ** 2 / (n / 100);
  check("RNG: 100-bin chi-square per degree of freedom", c2 / 99, 1.5);
}

// 5. The flight: d = -ln(xi)/Sigma has mean 1/Sigma and survival exp(-Sigma d).
{
  const r = T.rng(5), sigma = 0.8, n = 400000; let s = 0, s2 = 0, b1 = 0, b3 = 0;
  for (let i = 0; i < n; i++) { const d = T.flightDistance(r(), sigma); s += d; s2 += d * d; if (d > 1 / sigma) b1++; if (d > 3 / sigma) b3++; }
  const m = s / n, se = Math.sqrt((s2 / n - m * m) / n);
  check("flight: mean of -ln(xi)/Sigma vs 1/Sigma, in standard errors", Math.abs(m - 1 / sigma) / se, 4);
  check("flight: fraction beyond one mean free path vs e^-1, in standard errors", Math.abs(b1 / n - Math.exp(-1)) / Math.sqrt(Math.exp(-1) * (1 - Math.exp(-1)) / n), 4);
  check("flight: fraction beyond three mean free paths vs e^-3, in standard errors", Math.abs(b3 / n - Math.exp(-3)) / Math.sqrt(Math.exp(-3) * (1 - Math.exp(-3)) / n), 4);
  check("flight: a void gives an infinite flight (0 if so)", T.flightDistance(0.5, 0) === Infinity ? 0 : 1, 0);
}

// 6. Standing on a surface. A point stepped onto a unit sphere by the flight distance sits on it up to
//    rounding, so the sign of the surface equation is not evidence of the side: head on it is exactly zero
//    (read as "not inside" by f < 0), and elsewhere it is whatever rounding left. The recorded side
//    (sign of u . n) and the 1e-9 nudge along the normal are right every time. 2026-10-18, 20 000 rays per
//    impact parameter: the sign test finds the neutron inside 0.0 % of the time head on, 100 % at b = 0.5,
//    93 % near grazing (printed below). The check can fail: if the sign test never erred, the first
//    limit would trip.
{
  const r = T.rng(168); let worstWrong = 0, tokenBad = 0, nudgeBad = 0; const lines = [];
  for (const b of [0, 0.5, 0.99, 0.99999]) {
    let a = 0, t = 0, u = 0; const n = 20000;
    for (let i = 0; i < n; i++) { const o = T.surfaceStep(b, r); if (o.signInside) a++; if (o.tokenInside) t++; if (o.nudgedInside) u++; }
    worstWrong = Math.max(worstWrong, 1 - a / n); tokenBad += n - t; nudgeBad += n - u;
    lines.push(`b=${b}: sign says inside ${(100 * a / n).toFixed(1)} %`);
  }
  check("surfaces: the sign test is wrong somewhere (1 if it never errs by 5 % or more)", worstWrong >= 0.05 ? 0 : 1, 0);
  check("surfaces: the recorded side is right every time (wrong sides)", tokenBad, 0);
  check("surfaces: the 1e-9 nudge along the normal puts it inside every time (wrong sides)", nudgeBad, 0);
  console.log("     " + lines.join("; "));
  // The three figures the page quotes (0 % head on, 100 % at b = 0.5, about 93 % near grazing).
  const r2 = T.rng(168), frac = (b) => { let a = 0; for (let i = 0; i < 20000; i++) if (T.surfaceStep(b, r2).signInside) a++; return a / 20000; };
  const f0 = frac(0), f5 = frac(0.5), fg = frac(0.99999), html = fs.readFileSync(path.join(__dirname, "surfaces.html"), "utf8");
  check("page 3: the sign test finds the neutron inside 0 % head on, as the page says", f0, 0);
  check("page 3: ... 100 % at b = 0.5 (shortfall)", 1 - f5, 0);
  check("page 3: ... about 93 % near grazing (distance from 0.93)", Math.abs(fg - 0.93), 0.01);
  check("page 3: the page states those three figures (0 if all present)", html.includes("0&nbsp;% head on") && html.includes("100&nbsp;% at b&nbsp;=&nbsp;0.5") && html.includes("about 93&nbsp;% near grazing") ? 0 : 1, 0);
}

// 7. The two draws at a collision: the nuclide and the reaction follow the widths.
{
  const E = CD.energies["1000000"], keys = Object.keys(E.nuclides), tots = keys.map((k) => E.nuclides[k].n * E.nuclides[k].total), sum = tots.reduce((a, b) => a + b, 0);
  const r = T.rng(7), n = 400000, cnt = new Array(keys.length).fill(0);
  for (let i = 0; i < n; i++) cnt[T.nuclidePick(tots, r())]++;
  let worst = 0; keys.forEach((k, i) => { const p = tots[i] / sum; worst = Math.max(worst, Math.abs(cnt[i] / n - p) / Math.sqrt(p * (1 - p) / n)); });
  check("collision: nuclide draw frequencies vs N*sigma_t shares, worst in standard errors", worst, 4);
  const U = E.nuclides.U235, parts = { fission: U.fission, capture: U.capture, inelastic: U.inelastic, n2n: U.n2n, other: Math.max(0, U.other), elastic: U.elastic };
  const order = Object.keys(parts), tot = order.reduce((a, k) => a + parts[k], 0), c2 = {}; order.forEach((k) => c2[k] = 0);
  for (let i = 0; i < n; i++) c2[T.ladderPick(parts, r())]++;
  let w2 = 0; order.forEach((k) => { const p = parts[k] / tot; if (p > 0) w2 = Math.max(w2, Math.abs(c2[k] / n - p) / Math.sqrt(p * (1 - p) / n)); });
  check("collision: reaction ladder frequencies vs the partial cross sections (U-235, 1 MeV), worst in standard errors", w2, 4);
  // The data: the partial cross sections add up to the total, the macroscopic total adds up, shares are shares.
  let worstSum = 0, worstSig = 0;
  for (const ev of Object.keys(CD.energies)) {
    const e = CD.energies[ev]; let st = 0;
    for (const k of Object.keys(e.nuclides)) { const x = e.nuclides[k]; worstSum = Math.max(worstSum, Math.abs(x.fission + x.capture + x.inelastic + x.n2n + x.elastic + x.other - x.total) / x.total); st += x.n * x.total; }
    worstSig = Math.max(worstSig, Math.abs(st / e.sigma_t - 1));
  }
  check("collision data: fission + capture + inelastic + (n,2n) + elastic + other = total (relative)", worstSum, 1e-9);
  check("collision data: sum of N_i sigma_t,i = the material's Sigma_t (relative)", worstSig, 1e-9);
  console.log("     Godiva material at 1 MeV: " + keys.map((k, i) => k + " " + (100 * tots[i] / sum).toFixed(1) + " %").join(", ") + "; mean free path " + (1 / E.sigma_t).toFixed(2) + " cm");
}

// 8. Delta tracking against surface tracking, one dimension: both land on the closed forms.
{
  const sA = 0.7, sB = 2.5, a = 3, Len = 9, n = 300000, pA = 1 - Math.exp(-sA * a), pLeak = Math.exp(-sA * a - sB * (Len - a)), pB = 1 - pA - pLeak;
  const out = {};
  for (const [name, f, seed] of [["surface", T.surfaceTrack, 11], ["delta", T.deltaTrack, 12]]) {
    const r = T.rng(seed), c = { A: 0, B: 0, leak: 0 }; let virt = 0, flights = 0;
    for (let i = 0; i < n; i++) { const o = f(sA, sB, a, Len, r); c[o.where]++; virt += o.virtual; flights += o.flights; }
    out[name] = { c, virt, flights };
    let w = 0; for (const [k, p] of [["A", pA], ["B", pB], ["leak", pLeak]]) w = Math.max(w, Math.abs(c[k] / n - p) / Math.sqrt(p * (1 - p) / n));
    check("delta tracking: " + name + " tracking vs the closed-form fractions (A, B, leak), worst in standard errors", w, 4);
  }
  check("delta tracking: it makes virtual collisions where surface tracking makes none (0 if so)", out.delta.virt > 0 && out.surface.virt === 0 ? 0 : 1, 0);
  console.log(`     per neutron: surface ${(out.surface.flights / n).toFixed(2)} flights; delta ${(out.delta.flights / n).toFixed(2)} flights, ${(out.delta.virt / n).toFixed(2)} virtual`);
}

// 9. The fission bank (ports of physics/fission.rs).
{
  const r = T.rng(9), n = 400000; let s = 0, s2 = 0;
  for (let i = 0; i < n; i++) { const k = T.numNeutrons(2.43, 1.07, r()); s += k; s2 += k * k; }
  const m = s / n;
  check("bank: mean sites per fission vs nu/k, in standard errors", Math.abs(m - 2.43 / 1.07) / Math.sqrt((s2 / n - m * m) / n), 4);
  let outside = 0, sumBad = 0;
  for (const [total, want] of [[10, 8], [7, 13], [9, 9], [5, 12]]) for (let t = 0; t < 50; t++) {
    const mu = T.combMultiplicities(total, want, (t + 0.5) / 50), lo = Math.floor(want / total), hi = Math.ceil(want / total);
    mu.forEach((c) => { if (c < lo || c > hi) outside++; }); if (mu.reduce((x, y) => x + y, 0) !== want) sumBad++;
  }
  check("bank: the comb takes every site floor or ceil of n/total times (sites outside)", outside, 0);
  check("bank: the comb returns exactly n sites (cases that do not)", sumBad, 0);
  const rr = T.rng(3); let rep = 0; for (let t = 0; t < 50; t++) { const mu = T.replacementMultiplicities(10, 10, rr); if (mu.some((c) => c !== 1)) rep++; }
  check("bank: sampling with replacement does break that (0 if it does, in at least 40 of 50 trials)", rep >= 40 ? 0 : 1, 0);
}

// 10. Correlated generations: the quoted error that assumes independence is too small by about sqrt((1+rho)/(1-rho)).
{
  const rho = 0.5, n = 400, reps = 1500, r = T.rng(10); let naive = 0; const means = [];
  for (let i = 0; i < reps; i++) { const ms = T.meanStderr(T.ar1(n, rho, r)); means.push(ms.mean); naive += ms.se; }
  naive /= reps; const sd = T.meanStderr(means).se * Math.sqrt(reps), ratio = sd / naive, want = Math.sqrt((1 + rho) / (1 - rho));
  check("statistics: actual scatter / quoted error for lag-one correlation 0.5 vs sqrt((1+rho)/(1-rho)) (relative)", Math.abs(ratio / want - 1), 0.08);
  console.log(`     ratio ${ratio.toFixed(3)} against ${want.toFixed(3)}`);
}

// Page text against the data: every data-* number on the transport pages.
{
  const half = (t) => { const d = t.split(".")[1]; return 0.5 * Math.pow(10, -(d ? d.length : 0)) * (1 + 1e-9); };
  let bad = 0, n = 0;
  for (const f of fs.readdirSync(__dirname).filter((x) => x.endsWith(".html"))) {
    const h = fs.readFileSync(path.join(__dirname, f), "utf8");
    for (const m of h.matchAll(/data-mfp="([0-9]+)">([^<]+)</g)) { n++; const want = 1 / CD.energies[m[1]].sigma_t; if (Math.abs(+m[2] - want) > half(m[2])) { bad++; console.log("     mismatch", f, m[0], want); } }
    for (const m of h.matchAll(/data-share="([0-9]+),([A-Z0-9]+)">([^<]+)</g)) { const e = CD.energies[m[1]]; n++; const want = 100 * e.nuclides[m[2]].n * e.nuclides[m[2]].total / Object.keys(e.nuclides).reduce((a, k) => a + e.nuclides[k].n * e.nuclides[k].total, 0); if (Math.abs(+m[3] - want) > half(m[3])) { bad++; console.log("     mismatch", f, m[0], want); } }
    for (const m of h.matchAll(/data-part="([0-9]+),([A-Z0-9]+),([a-z0-9]+)">([^<]+)</g)) { n++; const want = CD.energies[m[1]].nuclides[m[2]][m[3]]; if (Math.abs(+m[4] - want) > half(m[4])) { bad++; console.log("     mismatch", f, m[0], want); } }
    for (const m of h.matchAll(/data-hex="([a-z]+)">([^<]+)</g)) {
      n++; let s = 1n; const g = []; for (let i = 0; i < 4; i++) { const p = T.prn(s); s = p.state; g.push(bits(p.x).toString(16)); }
      const want = { first: g[0], state: s.toString(16) }[m[1]]; if (m[2].replace(/^0x/, "").replace(/_/g, "") !== want) { bad++; console.log("     mismatch", f, m[0], want); }
    }
  }
  check("transport pages: " + n + " numbers in the prose vs the data and the RNG", bad, 0);
}

if (failed) { console.log(failed + " check(s) failed"); process.exit(1); }
console.log("all transport lesson checks passed");
