// The content-hashed file cache (gh:#923), two jobs, both run by
// scripts/build-pages.sh:
//
//   node docs/site/check-sw.js                   run sw.js's cache logic under node
//   node docs/site/check-sw.js --manifest <site> write <site>/asset-manifest.json
//
// The checks drive `makeCache` (docs/site/sw.js) with an in-memory Cache API
// and a fake server across two deploys, and assert what a reader's browser
// should see: an unchanged file is not fetched again after a deploy, the same
// bytes at two paths are fetched once, a body whose hash is not the
// manifest's is served but never stored, dropped hashes are deleted, pages
// are always fetched and open offline from their last copy, and a site with
// no manifest is left alone. Any failure exits 1 and fails the build.
"use strict";
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");

const sha = (buf) => crypto.createHash("sha256").update(Buffer.from(buf)).digest("hex");

if (process.argv[2] === "--manifest") {
  const site = process.argv[3];
  if (!site) { console.error("check-sw: --manifest needs the site directory"); process.exit(1); }
  const files = {};
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) { walk(p); continue; }
      const rel = path.relative(site, p).split(path.sep).join("/");
      if (rel.endsWith(".html") || rel === "asset-manifest.json" || rel === "sw.js") continue;
      files[rel] = sha(fs.readFileSync(p));
    }
  };
  walk(site);
  fs.writeFileSync(path.join(site, "asset-manifest.json"), JSON.stringify({ files }));
  const n = Object.keys(files).length;
  const distinct = new Set(Object.values(files)).size;
  console.log(`check-sw: asset-manifest.json lists ${n} files (${distinct} distinct contents)`);
  process.exit(0);
}

const { makeCache } = require(path.join(__dirname, "sw.js"));
const ROOT = "https://example.test/site/";

// An in-memory Cache API: what a real one does that matters here (a match is
// a fresh Response each time; keys are requests with a url).
function fakeCaches() {
  const stores = new Map();
  return {
    stores,
    open: async (name) => {
      if (!stores.has(name)) stores.set(name, new Map());
      const s = stores.get(name);
      return {
        match: async (k) => {
          const e = s.get(typeof k === "string" ? k : k.url);
          return e ? new Response(e.buf.slice(0), { status: e.status, headers: e.headers }) : undefined;
        },
        put: async (k, r) => {
          const buf = await r.arrayBuffer();
          s.set(typeof k === "string" ? k : k.url, { buf, status: r.status, headers: Object.fromEntries(r.headers) });
        },
        keys: async () => [...s.keys()].map((url) => ({ url })),
        delete: async (k) => s.delete(k.url),
      };
    },
  };
}

// A server: path -> bytes, a log of what was fetched, and an off switch.
function fakeServer() {
  const srv = { files: {}, log: [], offline: false, lie: {} };
  srv.fetch = async (u, _opts) => {
    const url = typeof u === "string" ? u : u.url;
    if (srv.offline) throw new TypeError("network down");
    srv.log.push(url.slice(ROOT.length));
    const p = url.slice(ROOT.length);
    const body = srv.lie[p] !== undefined ? srv.lie[p] : srv.files[p];
    if (body === undefined) return new Response("missing", { status: 404 });
    const type = p.endsWith(".wasm") ? "application/wasm" : p.endsWith(".json") ? "application/json" : "text/html";
    return new Response(Buffer.from(body), { headers: { "content-type": type } });
  };
  srv.deploy = (files) => {
    srv.files = { ...files };
    const m = {};
    for (const [p, b] of Object.entries(files)) if (!p.endsWith(".html")) m[p] = sha(Buffer.from(b));
    srv.files["asset-manifest.json"] = JSON.stringify({ files: m });
  };
  return srv;
}

const env = (srv, caches) => ({
  caches,
  fetch: srv.fetch,
  Response,
  root: ROOT,
  digestHex: async (buf) => sha(buf),
});
const get = (p, mode) => ({ url: ROOT + p, method: "GET", mode: mode || "cors" });
const text = async (r) => Buffer.from(await r.arrayBuffer()).toString();

let bad = 0;
const ok = (cond, what) => { if (!cond) { console.error("check-sw: FAIL " + what); bad++; } };

(async () => {
  const srv = fakeServer();
  const caches = fakeCaches();
  srv.deploy({
    "demos/a/index.html": "<p>a v1</p>",
    "demos/a/app_bg.wasm": "WASM-A-1",
    "demos/a/data/u238.zz": "TAPE-U238",
    "demos/b/data/u238.zz": "TAPE-U238", // the same tape under a second demo
    "demos/b/app_bg.wasm": "WASM-B-1",
  });
  let c = makeCache(env(srv, caches));

  // A page load: the page from the network, then its files once each.
  let r = await c.handle(get("demos/a/index.html", "navigate"));
  ok((await text(r)) === "<p>a v1</p>", "first page load serves the page");
  srv.log = [];
  r = await c.handle(get("demos/a/app_bg.wasm"));
  ok((await text(r)) === "WASM-A-1", "the wasm is served");
  ok(r.headers.get("content-type") === "application/wasm", "the wasm keeps its content type (instantiateStreaming needs it)");
  await c.handle(get("demos/a/data/u238.zz"));
  ok(srv.log.join() === "demos/a/app_bg.wasm,demos/a/data/u238.zz", "first visit fetches each file: " + srv.log.join());

  // Again: nothing from the network.
  srv.log = [];
  r = await c.handle(get("demos/a/app_bg.wasm"));
  ok((await text(r)) === "WASM-A-1" && srv.log.length === 0, "a held file is not fetched again");

  // The same tape under the other demo: held already, by its hash.
  r = await c.handle(get("demos/b/data/u238.zz"));
  ok((await text(r)) === "TAPE-U238" && srv.log.length === 0, "the same bytes at another path are not fetched again: " + srv.log.join());

  // A deploy that changes demo a's wasm only.
  srv.deploy({
    "demos/a/index.html": "<p>a v2</p>",
    "demos/a/app_bg.wasm": "WASM-A-2",
    "demos/a/data/u238.zz": "TAPE-U238",
    "demos/b/data/u238.zz": "TAPE-U238",
    "demos/b/app_bg.wasm": "WASM-B-1",
  });
  r = await c.handle(get("demos/a/index.html", "navigate"));
  ok((await text(r)) === "<p>a v2</p>", "after a deploy the page is the new one");
  srv.log = [];
  r = await c.handle(get("demos/a/app_bg.wasm"));
  ok((await text(r)) === "WASM-A-2" && srv.log.join() === "demos/a/app_bg.wasm", "a changed file is fetched");
  r = await c.handle(get("demos/a/data/u238.zz"));
  ok((await text(r)) === "TAPE-U238" && srv.log.length === 1, "an unchanged file survives the deploy");
  const held = [...caches.stores.get("outram-files-v1").keys()];
  ok(!held.includes(ROOT + "__sha256/" + sha("WASM-A-1")), "the old wasm's hash was deleted");

  // A worker restarted by the browser: the kept manifest, the held files.
  c = makeCache(env(srv, caches));
  srv.log = [];
  r = await c.handle(get("demos/a/app_bg.wasm"));
  ok((await text(r)) === "WASM-A-2" && srv.log.length === 0, "a restarted worker still serves held files: " + srv.log.join());

  // A body that is not what the manifest says (a stale copy): served, not stored.
  srv.lie["demos/b/app_bg.wasm"] = "STALE";
  r = await c.handle(get("demos/b/app_bg.wasm"));
  ok((await text(r)) === "STALE", "a mismatched body is still served");
  ok(!caches.stores.get("outram-files-v1").has(ROOT + "__sha256/" + sha("WASM-B-1")), "a mismatched body is not stored under the manifest's hash");
  delete srv.lie["demos/b/app_bg.wasm"];
  r = await c.handle(get("demos/b/app_bg.wasm"));
  ok((await text(r)) === "WASM-B-1", "the right body, next time, is served and stored");

  // Offline: a visited page and its files still open.
  srv.offline = true;
  r = await c.handle(get("demos/a/index.html", "navigate"));
  ok((await text(r)) === "<p>a v2</p>", "a visited page opens offline");
  r = await c.handle(get("demos/a/app_bg.wasm"));
  ok((await text(r)) === "WASM-A-2", "its wasm opens offline");
  srv.offline = false;

  // Left to the browser: other origins, queries, POSTs, the manifest itself.
  ok(c.handle({ url: "https://elsewhere.test/x.js", method: "GET", mode: "cors" }) === null, "another origin is not handled");
  ok(c.handle(get("demos/a/app_bg.wasm?x=1")) === null, "a URL with a query is not handled");
  ok(c.handle({ url: ROOT + "demos/a/app_bg.wasm", method: "POST", mode: "cors" }) === null, "a POST is not handled");
  ok(c.handle(get("asset-manifest.json")) === null, "the manifest is not cached by itself");

  // A site with no manifest: every request passes through.
  const bare = fakeServer();
  bare.files = { "x.wasm": "X" };
  const c2 = makeCache(env(bare, fakeCaches()));
  r = await c2.handle(get("x.wasm"));
  ok((await text(r)) === "X", "no manifest: the file is served");
  bare.log = [];
  await c2.handle(get("x.wasm"));
  ok(bare.log.join() === "x.wasm", "no manifest: nothing is cached");

  if (bad) process.exit(1);
  console.log("check-sw: the content-hashed cache passes all checks");
})().catch((e) => { console.error("check-sw:", e); process.exit(1); });
