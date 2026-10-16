// The site's content-hashed file cache (gh:#923, maintainer 2026-10-16: "Do
// content hash lah"). Registered from site-nav.js, which every page loads.
//
// Why a service worker and not renamed files: GitHub Pages sets each file's
// ETag and Last-Modified to the deploy time and `max-age=600`, so after every
// push an unchanged 18 MB wasm is downloaded again whatever it is called.
//
// How it works:
// - scripts/build-pages.sh writes asset-manifest.json: every non-HTML file of
//   the site, by path, with the SHA-256 of its bytes.
// - A GET of a listed path (no query string) is answered from the Cache API
//   entry for its hash, the network only when that hash is not held. Keying by
//   hash alone means a file that did not change between deploys is never
//   fetched again, and the same bytes published at two paths (an ENDF tape in
//   two demos) are fetched once.
// - A downloaded body is stored only when its SHA-256 is the manifest's. A
//   stale copy (an HTTP cache, a CDN node behind the deploy) is still served,
//   never stored under a hash it does not have.
// - Pages (navigations) go to the network, revalidated, so each page is the
//   current build; a copy of each is kept for when the network is down, so a
//   demo already visited opens offline.
// - The manifest is re-read, revalidated, on every navigation. Hashes it no
//   longer lists are deleted.
// - No manifest (a local `python3 -m http.server` of docs/site): everything
//   goes straight to the network.
//
// The logic is in `makeCache`, which takes the platform (caches, fetch,
// digest) as arguments so docs/site/check-sw.js runs it under node.
"use strict";

/** The cache logic. `env`: { caches, fetch, digestHex(ArrayBuffer) -> Promise<hex>,
 *  Response, root: the site root URL ending in "/" }. */
function makeCache(env) {
  var FILES = "outram-files-v1"; // key: root + "__sha256/" + hash
  var PAGES = "outram-pages-v1"; // key: the page URL; offline copies
  var META = "outram-meta-v1"; // key: the manifest URL; the last one read
  var manifestUrl = env.root + "asset-manifest.json";
  var manifest = null; // { files: { path: hash } }, or false when the site has none

  function hashKey(h) { return env.root + "__sha256/" + h; }

  /** Read the manifest from the network (revalidated), keep it, prune what it
   *  no longer lists. Falls back to the last one kept. */
  function refresh() {
    return env.fetch(manifestUrl, { cache: "no-cache" }).then(function (r) {
      if (!r.ok) throw new Error("manifest " + r.status);
      return r.clone().json().then(function (m) {
        manifest = m;
        return env.caches.open(META).then(function (c) { return c.put(manifestUrl, r); })
          .then(function () { return prune(m); })
          .then(function () { return m; });
      });
    }).catch(function () {
      return env.caches.open(META).then(function (c) { return c.match(manifestUrl); }).then(function (r) {
        if (!r) { manifest = manifest || false; return manifest; }
        return r.json().then(function (m) { manifest = m; return m; });
      });
    });
  }

  /** The manifest in memory, or the kept one (a restarted worker), or the network's. */
  function current() {
    if (manifest !== null) return Promise.resolve(manifest);
    return env.caches.open(META).then(function (c) { return c.match(manifestUrl); }).then(function (r) {
      if (!r) return refresh();
      return r.json().then(function (m) { manifest = m; return m; });
    });
  }

  /** Delete every held hash the manifest `m` does not list. */
  function prune(m) {
    var keep = {};
    Object.keys(m.files).forEach(function (p) { keep[hashKey(m.files[p])] = true; });
    return env.caches.open(FILES).then(function (c) {
      return c.keys().then(function (reqs) {
        return Promise.all(reqs.filter(function (q) { return !keep[q.url]; }).map(function (q) { return c.delete(q); }));
      });
    });
  }

  /** A listed file: from the hash's entry, else the network (stored when it checks). */
  function file(url, hash) {
    return env.caches.open(FILES).then(function (c) {
      return c.match(hashKey(hash)).then(function (hit) {
        if (hit) return hit;
        return env.fetch(url, { cache: "no-cache" }).then(function (r) {
          if (!r.ok) return r;
          var type = r.headers.get("content-type") || "application/octet-stream";
          return r.arrayBuffer().then(function (buf) {
            return env.digestHex(buf).then(function (got) {
              var make = function () { return new env.Response(buf, { headers: { "content-type": type } }); };
              if (got !== hash) return make(); // served, never stored under the wrong hash
              return c.put(hashKey(hash), make()).then(make);
            });
          });
        });
      });
    });
  }

  /** A page: the network (revalidated), its offline copy when that fails. */
  function page(request) {
    var refreshed = refresh();
    return env.fetch(request.url, { cache: "no-cache" }).then(function (r) {
      if (!r.ok) return r;
      var copy = r.clone();
      return env.caches.open(PAGES).then(function (c) { return c.put(request.url, copy); })
        .then(function () { return refreshed; }).then(function () { return r; });
    }).catch(function (e) {
      return env.caches.open(PAGES).then(function (c) { return c.match(request.url); }).then(function (hit) {
        if (hit) return hit;
        throw e;
      });
    });
  }

  /** What to answer `request` with, or null to leave it to the browser. */
  function handle(request) {
    if (request.method !== "GET") return null;
    var url = new URL(request.url);
    if (url.href.indexOf(env.root) !== 0) return null;
    if (request.mode === "navigate") return page(request);
    if (url.search) return null;
    var path = url.pathname.slice(new URL(env.root).pathname.length);
    if (path === "asset-manifest.json" || path === "sw.js") return null;
    return current().then(function (m) {
      var h = m && m.files[path];
      return h ? file(url.href, h) : env.fetch(request);
    });
  }

  return { handle: handle, refresh: refresh };
}

if (typeof module !== "undefined") module.exports = { makeCache: makeCache };

if (typeof ServiceWorkerGlobalScope !== "undefined" && self instanceof ServiceWorkerGlobalScope) {
  var cache = makeCache({
    caches: caches,
    fetch: function (u, o) { return fetch(u, o); },
    Response: Response,
    root: new URL(".", self.location.href).href,
    digestHex: function (buf) {
      return crypto.subtle.digest("SHA-256", buf).then(function (d) {
        return Array.from(new Uint8Array(d)).map(function (b) { return b.toString(16).padStart(2, "0"); }).join("");
      });
    },
  });
  self.addEventListener("install", function () { self.skipWaiting(); });
  self.addEventListener("activate", function (ev) {
    ev.waitUntil(cache.refresh().then(function () { return self.clients.claim(); }));
  });
  self.addEventListener("fetch", function (ev) {
    var answer = cache.handle(ev.request);
    if (answer) ev.respondWith(answer);
  });
}
