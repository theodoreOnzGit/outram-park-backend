// Site navigation for every page except the main menu: a breadcrumb back to
// Home and an "Up" link to the page one level up (maintainer request,
// 2026-10-05). `scripts/build-pages.sh` adds this script to every built HTML
// page, so books, demos and the API reference need no per-page edits.
//
// Hierarchy:
//   Home (index.html, opened at the page's track tab)
//   `- tutorials/<track>/ | deep-dives/<track>/ | demos/<demo>/ | api/<crate>/
//     `- any page inside it  (Up = that book's / demo's / crate's index)
//
// On pages with a <main> (mdBook, rustdoc) the bar is the first thing in
// <main>. On full-screen demos (a canvas, no <main>) it is a thin fixed bar
// at the top and the page below it is shortened, so it never covers the
// demo's own buttons (mobile-first rule, docs/claude-md/).
//
// Under the bar, deep dives, tutorials and API pages get the code map as a
// navigation strip (code-map-bar.js, loaded from here; see its header).
//
// Demo links open in a new tab (maintainer request, 2026-10-05): any link
// into `demos/<demo>/` opens in a new tab unless the page is already inside
// that demo, so a reader keeps the lesson they came from. It is done here
// rather than per link because mdBook's markdown links cannot carry a
// `target`, and the main menu loads this script for this part only.
(function () {
  "use strict";
  var me = document.currentScript;
  if (!me) return;
  var root = new URL(".", me.src);
  var here = new URL(location.href);
  var demos = new URL("demos/", root);

  // The demo a URL is inside ("monte-carlo" for demos/monte-carlo/...), or "".
  function demoOf(url) {
    if (url.origin !== demos.origin || url.pathname.indexOf(demos.pathname) !== 0) return "";
    return url.pathname.slice(demos.pathname.length).split("/")[0];
  }
  var hereDemo = demoOf(here);
  function newTabIfDemo(el) {
    if (!el || !el.href || el.hasAttribute("target")) return;
    var d = demoOf(new URL(el.href, location.href));
    if (!d || d === hereDemo) return;
    el.target = "_blank";
    el.rel = (el.rel ? el.rel + " " : "") + "noopener";
  }
  function markAll() {
    var links = document.querySelectorAll("a[href]");
    for (var i = 0; i < links.length; i++) newTabIfDemo(links[i]);
  }
  // Links already on the page, and (capture phase, before the browser
  // follows it) any link a page script added after load.
  document.addEventListener("click", function (ev) {
    var t = ev.target;
    while (t && t.nodeName !== "A") t = t.parentNode;
    if (t && t.hasAttribute && t.hasAttribute("href")) newTabIfDemo(t);
  }, true);
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", markAll);
  } else {
    markAll();
  }

  if (here.href.indexOf(root.href) !== 0) return;
  var parts = here.pathname.slice(root.pathname.length).split("/");
  if (parts.length < 2) return; // the main menu itself

  // Track of each book, demo and crate: the main menu's tab ids.
  var TRACK = {
    "monte-carlo": "monte-carlo", "triso-pebble": "monte-carlo",
    "outram_mc_libs": "monte-carlo",
    "nuclear-data": "nuclear-data", "njoy_outram_park_fork": "nuclear-data",
    "dispersion": "dispersion", "changi": "dispersion", "buangkok": "dispersion",
    "sembawang": "dispersion",
    "triso-atops": "triso-atops", "boon_lay": "triso-atops"
  };
  var TRACK_NAME = {
    "monte-carlo": "Monte Carlo", "nuclear-data": "Nuclear data",
    "dispersion": "Gaussian plume & puff", "triso-atops": "TRISO-ATOPS"
  };
  var KIND = { "tutorials": "tutorial", "deep-dives": "deep dive", "demos": "demo", "api": "API" };

  var kind = parts[0], name = parts[1] || "";
  if (!KIND[kind] || !name) return;
  var track = TRACK[name];
  var home = new URL("index.html" + (track ? "#" + track : ""), root);
  var sectionUrl = new URL(kind + "/" + name + "/", root);
  var sectionLabel = kind === "api"
    ? "API: " + name.replace(/_/g, "-")
    : (TRACK_NAME[track] || name) + " " + KIND[kind];

  // The section's own index is one level below Home; everything else is one
  // level below its section.
  var rest = parts.slice(2).join("/");
  var atSectionIndex = rest === "" || rest === "index.html";
  var upUrl = atSectionIndex ? home : sectionUrl;
  var upLabel = atSectionIndex ? "Home" : sectionLabel;

  function a(href, text) {
    var el = document.createElement("a");
    el.href = href;
    el.textContent = text;
    return el;
  }
  var bar = document.createElement("nav");
  bar.className = "op-site-nav";
  bar.setAttribute("aria-label", "Site");
  var crumbs = document.createElement("span");
  crumbs.className = "op-crumbs";
  crumbs.appendChild(a(home.href, "\u2302 Home"));
  crumbs.appendChild(document.createTextNode(" \u203a "));
  if (atSectionIndex) {
    var cur = document.createElement("span");
    cur.textContent = sectionLabel;
    crumbs.appendChild(cur);
  } else {
    crumbs.appendChild(a(sectionUrl.href, sectionLabel));
  }
  var up = a(upUrl.href, "\u2191 Up: " + upLabel);
  up.className = "op-up";
  bar.appendChild(crumbs);
  bar.appendChild(up);

  var css = document.createElement("style");
  css.textContent =
    ".op-site-nav{display:flex;flex-wrap:wrap;gap:6px 16px;align-items:center;" +
    "justify-content:space-between;font:14px/1.4 system-ui,-apple-system,'Segoe UI',sans-serif;" +
    "padding:6px 0;margin:0 0 12px;border-bottom:1px solid rgba(128,128,128,.35)}" +
    ".op-site-nav a{display:inline-block;min-height:36px;line-height:36px;padding:0 4px}" +
    ".op-site-nav .op-up{font-weight:600}" +
    ".op-site-nav.op-fixed{position:fixed;top:0;left:0;right:0;z-index:1000;margin:0;" +
    "padding:0 12px;height:40px;box-sizing:border-box;flex-wrap:nowrap;overflow:hidden;" +
    "background:#161a22;color:#d8dce4;border-bottom:1px solid #2a303c}" +
    ".op-site-nav.op-fixed a{color:#9cc0ff;white-space:nowrap}" +
    ".op-site-nav.op-fixed .op-crumbs{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}";
  document.head.appendChild(css);

  function place() {
    var main = document.querySelector("main");
    if (main) {
      main.insertBefore(bar, main.firstChild);
      // The code map as a navbar under the breadcrumb (maintainer,
      // 2026-10-06): lessons and API pages only, never a demo's screen.
      if (kind !== "demos") {
        var cm = document.createElement("script");
        cm.src = new URL("code-map-bar.js", root).href;
        document.head.appendChild(cm);
      }
      return;
    }
    // Full-screen demo or plain page: a fixed bar, and the page starts below it.
    bar.classList.add("op-fixed");
    document.body.insertBefore(bar, document.body.firstChild);
    // With border-box, a `height: 100%` body keeps its height and its content
    // box (where a demo's `height: 100%` canvas lives) shrinks by the bar.
    document.body.style.boxSizing = "border-box";
    document.body.style.paddingTop = "40px";
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", place);
  } else {
    place();
  }
})();
