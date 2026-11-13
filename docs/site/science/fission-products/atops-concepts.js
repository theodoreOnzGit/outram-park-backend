// Renders the TRISO-ATOPS concept-to-code map (atops-concepts.html) from
// atops-concepts-data.js, and the step lists of the three concept walks
// (atops-walk-*.html). No physics: the map shows text from the data file,
// and the walks show recorded numbers looked up by atops-walk-numbers.js.
// GUI drawing code (the test rule's exception): check-concepts.js checks
// the data it draws.
(function () {
  "use strict";
  var D = window.ATOPS_CONCEPTS;
  var DEMO = "../../demos/triso-atops/?";
  var REVIEW = "../../code-review/#";
  var REPO = "https://github.com/theodoreOnzGit/outram-park-backend/blob/develop/";

  function el(tag, attrs, kids) {
    var n = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { n.setAttribute(k, attrs[k]); });
    (kids || []).forEach(function (k) { n.append(k); });
    return n;
  }
  function code(t) { return el("code", null, [t]); }
  function link(href, kids) { return el("a", { href: href }, kids); }
  function stageName(s) {
    for (var i = 0; i < D.STAGES.length; i++) if (D.STAGES[i][0] === s) return D.STAGES[i][1];
    return s;
  }
  function lessonLink(f) {
    var t = D.LESSON_TITLES[f] || f;
    var a = link(f, [t]);
    if (D.PENDING_LESSONS.indexOf(f) >= 0) return el("span", null, [a, " ", el("span", { "class": "pending" }, ["being written"])]);
    return a;
  }
  function row(label, kids) {
    return el("div", { "class": "cc-row" }, [el("div", { "class": "cc-label" }, [label]), el("div", { "class": "cc-val" }, kids)]);
  }
  function list(items) { var u = el("ul"); items.forEach(function (i) { u.append(el("li", null, i)); }); return u; }

  function card(c) {
    var art = el("article", { "class": "cc-card", id: c.id, "data-stage": c.stage });
    var head = el("h3", null, [link("#" + c.id, [c.title])]);
    art.append(el("p", { "class": "cc-stage cc-" + c.stage }, [stageName(c.stage)]), head, el("p", null, [c.idea]));
    art.append(el("pre", { "class": "cc-eq", "aria-label": "Equation as the code computes it" }, [c.eq]));
    art.append(row("Upstream (de374c8)", c.upstream
      ? [code(c.upstream.file), " :: ", code(c.upstream.fn)]
      : [el("b", null, ["NOT a port"]), ": boon-lay's own extension; upstream has no counterpart"]));
    art.append(row("Port", [list(c.port.map(function (p) {
      return [link(REVIEW + p.file + "::" + p.fn, [code(p.fn)]), " in ", code(p.file.replace("crates/boon-lay/src/", ""))];
    }))]));
    art.append(row("Tests", [list(c.tests.map(function (t) {
      return [link(REPO + t.file, [code(t.fn)]), " in ", code(t.file.replace("crates/boon-lay/", ""))];
    }))]));
    var rec = [];
    if (c.c2c.length) {
      rec.push(list(c.c2c.map(function (r) {
        return [code(r.group), ": " + r.cases + " cases, max relative deviation ", el("b", null, [r.dev])];
      })));
      rec.push(el("p", { "class": "note" }, ["From ", link(REPO + D.RECORD.file, [code("triso-atops-code-to-code.md")]), ", " + D.RECORD.passes + "."]));
    }
    if (c.c2cNote) rec.push(el("p", { "class": "cc-small" }, [c.c2cNote]));
    art.append(row(c.c2c.length ? "Code to code" : "Recorded check", rec));
    art.append(row("Lessons", [list(c.lessons.map(function (f) { return [lessonLink(f)]; }))]));
    art.append(row("Demo", c.demo
      ? [link(DEMO + c.demo, [code("?" + c.demo)]), " — " + c.demoNote]
      : [el("span", { "class": "cc-small" }, [c.demoNote])]));
    return art;
  }

  function map(root) {
    var bar = el("div", { "class": "chips cc-filter", role: "group", "aria-label": "Filter by stage" });
    var all = [["all", "All stages"]].concat(D.STAGES);
    var cards = el("div", { "class": "cc-cards" });
    D.CARDS.forEach(function (c) { cards.append(card(c)); });
    var count = el("p", { "class": "status", "aria-live": "polite" });
    function show(s) {
      var n = 0;
      cards.querySelectorAll(".cc-card").forEach(function (a) {
        var on = s === "all" || a.getAttribute("data-stage") === s;
        a.hidden = !on; if (on) n++;
      });
      bar.querySelectorAll("button").forEach(function (b) { b.setAttribute("aria-pressed", String(b.getAttribute("data-s") === s)); });
      count.textContent = n + " of " + D.CARDS.length + " concepts";
    }
    all.forEach(function (s) {
      var b = el("button", { type: "button", "class": "chip", "data-s": s[0] }, [s[0] === "all" ? s[1] : s[1].split(":")[0]]);
      b.addEventListener("click", function () { show(s[0]); });
      bar.append(b);
    });
    root.append(bar, count, cards);
    // A #id deep link opens on that card, whatever the filter.
    show("all");
    var h = location.hash.slice(1);
    if (h && document.getElementById(h)) document.getElementById(h).scrollIntoView();
  }

  // A walk page: each <li data-concept data-lesson data-demo> gets its
  // links appended from the data, so the walk cannot name a link the map
  // does not have.
  function walk() {
    var N = window.ATOPS_WALK_NUMBERS ? window.ATOPS_WALK_NUMBERS.numbers(window.ATOPS_DATA, window.ATOPS_WALK_RECORDED) : {};
    document.querySelectorAll("[data-q]").forEach(function (s) { var k = s.getAttribute("data-q"); if (k in N) s.textContent = N[k]; });
    document.querySelectorAll("li[data-concept]").forEach(function (li) {
      var c = li.getAttribute("data-concept"), ls = li.getAttribute("data-lesson"), dm = li.getAttribute("data-demo");
      var card = D.CARDS.filter(function (x) { return x.id === c; })[0];
      var p = el("p", { "class": "cc-links" }, [link("atops-concepts.html#" + c, ["Concept card: " + (card ? card.title : c)])]);
      if (ls) p.append(" · ", lessonLink(ls));
      if (dm) p.append(" · ", link(DEMO + dm, ["Open the demo here"]));
      li.append(p);
    });
  }

  function start() {
    var root = document.getElementById("cc-map");
    if (root) map(root); else walk();
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start); else start();
})();
