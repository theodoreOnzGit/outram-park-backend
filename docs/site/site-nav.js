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
(function () {
  "use strict";
  var me = document.currentScript;
  if (!me) return;
  var root = new URL(".", me.src);
  var here = new URL(location.href);
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
