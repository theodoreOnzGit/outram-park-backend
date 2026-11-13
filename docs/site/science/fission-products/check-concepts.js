// The TRISO-ATOPS concept map and walks (gh:#1207 part 2), checked under node
// (no browser): `node docs/site/science/fission-products/check-concepts.js`.
// A check that cannot rot: every name the map and the walks print is looked
// up in the files it names, so a rename, a deleted test or a dropped rung
// fails here instead of leaving a dead card.
//
// What it checks, and against what:
//   1. each card: unique id, known stage; each port file exists and has
//      `fn <name>`; each test file exists and has `fn <name>`;
//   2. provenance: a ported card's first port file names its upstream Python
//      file in its header (first 40 lines); an extension's says NOT a port.
//      (The upstream clone is gitignored and absent on CI, so it is never
//      read here.)
//   3. each quoted code-to-code row is a table row of the record
//      (crates/boon-lay/docs/triso-atops-code-to-code.md) with that group,
//      case count and deviation;
//   4. each lesson exists (or is in PENDING_LESSONS), and a LESSON_TITLES
//      entry matches an existing page's <title>;
//   5. each demo link (cards, walks, map) names a rung in
//      examples/triso_atops_web/rungs.rs TABLE, only keys in app.rs's
//      DEEP_LINK_KEYS, and a `pick` that is `all`, a PICK_GROUP_WORDS word
//      or one of the 84 TRISO-ATOPS nuclides (atops-data.js); numbers parse;
//   6. the walks: every concept anchor is a card id, every code-review link
//      resolves to `fn <name>` in its file, every data-q number resolves to
//      a value from the recorded data (atops-walk-numbers.js), and each walk
//      links back to the map.
// Result, 2026-11-04: all checks pass (the count is printed).
"use strict";
const fs = require("fs"), path = require("path");
const HERE = __dirname, ROOT = path.resolve(HERE, "../../../..");
global.window = {};
for (const f of ["atops-concepts-data.js", "atops-data.js", "atops-walk-recorded.js", "atops-walk-numbers.js"]) eval(fs.readFileSync(path.join(HERE, f), "utf8"));
const D = window.ATOPS_CONCEPTS, A = window.ATOPS_DATA;
let failed = 0, passed = 0;
function ok(cond, what) { if (cond) passed++; else { failed++; console.log("FAIL " + what); } }
const read = (p) => { try { return fs.readFileSync(path.join(ROOT, p), "utf8"); } catch (e) { return null; } };
const hasFn = (src, name) => src !== null && new RegExp("\\bfn\\s+" + name.replace(/[^A-Za-z0-9_]/g, "") + "\\s*[<(]").test(src);

// The demo's rung names and query keys, from the Rust sources.
const rungsSrc = read("crates/dhoby-ghaut/examples/triso_atops_web/rungs.rs");
const RUNGS = [...rungsSrc.matchAll(/^\s*name:\s*"([^"]+)"/gm)].map((m) => m[1]);
const appSrc = read("crates/dhoby-ghaut/examples/triso_atops_web/app.rs");
const constList = (name) => {
  const m = appSrc.match(new RegExp("const " + name + ": \\[&str; \\d+\\] = \\[([^\\]]*)\\]"));
  return m ? [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]) : [];
};
const KEYS = constList("DEEP_LINK_KEYS"), GROUP_WORDS = constList("PICK_GROUP_WORDS");
ok(RUNGS.length === 8, `rungs.rs TABLE read (${RUNGS.length} names)`);
ok(KEYS.length > 1 && GROUP_WORDS.length === 5, `app.rs DEEP_LINK_KEYS (${KEYS}) and PICK_GROUP_WORDS (${GROUP_WORDS}) read`);
const norm = (s) => s.toLowerCase().replace(/-/g, "");
const NUCLIDES = new Set(A.nuclides.map((n) => norm(n.name)));

function checkDemo(q, where) {
  const pairs = q.replace(/^\?/, "").split("&").filter(Boolean).map((kv) => kv.split("="));
  const kv = Object.fromEntries(pairs);
  ok(pairs.every(([k]) => KEYS.includes(k)), `${where}: demo keys ${pairs.map((p) => p[0])} all in DEEP_LINK_KEYS`);
  ok(RUNGS.includes(kv.rung), `${where}: rung '${kv.rung}' is in rungs.rs TABLE`);
  if ("pick" in kv) {
    const p = decodeURIComponent(kv.pick).toLowerCase();
    ok(p === "all" || GROUP_WORDS.some((g) => p === g || p === g + "s") || NUCLIDES.has(norm(p)), `${where}: pick '${kv.pick}' is all, a group word or a TRISO-ATOPS nuclide`);
    ok(kv.rung === "release", `${where}: pick is read on the release rung only`);
  }
  for (const k of ["hold", "hours", "now"]) if (k in kv) ok(Number.isFinite(Number(kv[k])) && ["release", "failure"].includes(kv.rung), `${where}: ${k}=${kv[k]} is a number on the failure or release rung`);
  if ("leak" in kv) ok(["on", "off"].includes(kv.leak) && kv.rung === "release", `${where}: leak=${kv.leak} is on/off on the release rung`);
}

// 1-5: the cards.
const record = read(D.RECORD.file);
ok(record !== null, `record ${D.RECORD.file} exists`);
const recordRows = (record || "").split("\n").filter((l) => l.startsWith("|")).map((l) => l.replace(/[`*]/g, "").split("|").map((c) => c.trim()));
const STAGES = D.STAGES.map((s) => s[0]);
const ids = new Set();
const lessonOk = (f) => fs.existsSync(path.join(HERE, f)) || D.PENDING_LESSONS.includes(f);
for (const c of D.CARDS) {
  const w = `card ${c.id}`;
  ok(!ids.has(c.id), `${w}: unique id`); ids.add(c.id);
  ok(STAGES.includes(c.stage), `${w}: stage '${c.stage}' known`);
  for (const p of c.port) ok(hasFn(read(p.file), p.fn), `${w}: fn ${p.fn} in ${p.file}`);
  for (const t of c.tests) ok(hasFn(read(t.file), t.fn), `${w}: test fn ${t.fn} in ${t.file}`);
  ok(c.tests.length > 0, `${w}: names a test`);
  const head = (read(c.port[0].file) || "").split("\n").slice(0, 40).join("\n");
  if (c.upstream) ok(head.includes(c.upstream.file), `${w}: ${c.port[0].file} header names upstream ${c.upstream.file}`);
  else ok(/NOT a port/.test(head) && c.stage === "extension", `${w}: extension, header of ${c.port[0].file} says NOT a port`);
  if (c.stage === "extension") ok(c.upstream === null, `${w}: an extension names no upstream`);
  for (const r of c.c2c) ok(recordRows.some((cells) => cells[1] === r.group && cells[2] === r.cases && cells[3] === r.dev), `${w}: c2c row '${r.group} | ${r.cases} | ${r.dev}' is in the record`);
  ok(c.lessons.length > 0, `${w}: names a lesson`);
  for (const f of c.lessons) ok(lessonOk(f), `${w}: lesson ${f} exists or is pending`);
  if (c.demo) checkDemo(c.demo, w); else ok(typeof c.demoNote === "string" && c.demoNote.length > 0, `${w}: no demo, and says why`);
}
for (const [f, t] of Object.entries(D.LESSON_TITLES)) {
  const p = path.join(HERE, f);
  if (fs.existsSync(p) && !D.PENDING_LESSONS.includes(f)) {
    const m = fs.readFileSync(p, "utf8").match(/<title>([^<]*)<\/title>/);
    ok(m && m[1].replace(/&amp;/g, "&") === t, `LESSON_TITLES ${f}: '${t}' matches the page's <title> '${m && m[1]}'`);
  } else ok(D.PENDING_LESSONS.includes(f), `LESSON_TITLES ${f}: page exists or is pending`);
}
for (const c of D.CARDS) for (const f of c.lessons) ok(f in D.LESSON_TITLES, `card ${c.id}: lesson ${f} has a title`);

// 6: the walks and the map page.
const N = window.ATOPS_WALK_NUMBERS.numbers(A, window.ATOPS_WALK_RECORDED);
const WALKS = ["atops-walk-cs137.html", "atops-walk-accident.html", "atops-walk-silver.html"];
for (const f of ["atops-concepts.html", ...WALKS]) {
  const src = fs.readFileSync(path.join(HERE, f), "utf8");
  for (const m of src.matchAll(/demos\/triso-atops\/\?([^"#\s]*)/g)) checkDemo(m[1].replace(/&amp;/g, "&"), `${f} href`);
  for (const m of src.matchAll(/data-demo="([^"]*)"/g)) checkDemo(m[1].replace(/&amp;/g, "&"), `${f} data-demo`);
  for (const m of src.matchAll(/(?:atops-concepts\.html#|data-concept=")([a-z0-9-]+)/g)) ok(ids.has(m[1]), `${f}: concept '${m[1]}' is a card`);
  for (const m of src.matchAll(/data-lesson="([^"]+)"/g)) ok(lessonOk(m[1]) && m[1] in D.LESSON_TITLES, `${f}: lesson ${m[1]} exists or is pending, and has a title`);
  for (const m of src.matchAll(/code-review\/#([^"]+?)::([A-Za-z0-9_]+)"/g)) ok(hasFn(read(m[1]), m[2]), `${f}: code-review link ${m[1]}::${m[2]} resolves`);
  for (const m of src.matchAll(/data-q="([^"]+)"/g)) ok(m[1] in N && N[m[1]] !== "" && !/NaN|undefined/.test(N[m[1]]), `${f}: number '${m[1]}' resolves (${N[m[1]]})`);
  for (const m of src.matchAll(/href="([a-z0-9-]+\.html)(?:#[^"]*)?"/g)) ok(fs.existsSync(path.join(HERE, m[1])) || D.PENDING_LESSONS.includes(m[1]), `${f}: link ${m[1]} exists`);
  // A number quoted from a Rust constant: data-rs="<file>:<CONST>" with the
  // constant's value as the span's text.
  for (const m of src.matchAll(/data-rs="([^":]+):([A-Z0-9_]+)">([^<]*)</g)) {
    const c = (read(m[1]) || "").match(new RegExp("const " + m[2] + ": f64 = ([^;]+);"));
    ok(c && Number(c[1].replace(/_/g, "")) === Number(m[3]), `${f}: quoted ${m[2]} = ${m[3]} matches ${m[1]} (${c && c[1]})`);
  }
  if (WALKS.includes(f)) {
    ok(/href="atops-concepts\.html"/.test(src), `${f}: links back to the map`);
    ok(/data-concept=/.test(src), `${f}: has steps`);
  }
}
// The walk recording is the demo's own headless output, with its header.
const R = window.ATOPS_WALK_RECORDED;
ok(R && /--headless release/.test(R.command) && R.csv.split("\n")[0].startsWith("nuclide,group,"), "atops-walk-recorded.js holds the release rung's headless CSV");

// The entry point: index.html's chapter 5 block links the map and the walks.
const index = fs.readFileSync(path.join(HERE, "../index.html"), "utf8");
for (const f of ["atops-concepts.html", ...WALKS]) ok(index.includes(`href="fission-products/${f}"`), `index.html links ${f}`);

console.log(`${failed ? "FAILED" : "ok"}: ${passed} checks passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
