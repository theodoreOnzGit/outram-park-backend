// The site search's tokeniser (docs/site/search.js, between the
// tokenise-begin / tokenise-end markers) against the fixture the Rust index
// builder is also tested on (crates/kovan/src/site_search.rs): the two must
// split every case identically, or a query would miss words the index holds
// (gh:#876 item 8). Run by scripts/build-pages.sh; a mismatch fails the build.
"use strict";
const fs = require("fs");
const path = require("path");
const src = fs.readFileSync(path.join(__dirname, "search.js"), "utf8");
const m = src.match(/\/\/ tokenise-begin[^\n]*\n([\s\S]*?)\/\/ tokenise-end/);
if (!m) { console.error("check-search: no tokenise block in search.js"); process.exit(1); }
const tokenise = new Function(m[1] + "\nreturn tokenise;")();
const cases = JSON.parse(fs.readFileSync(path.join(__dirname, "search-tokenise-fixture.json"), "utf8"));
let bad = 0;
for (const [input, want] of cases) {
  const got = tokenise(input);
  if (JSON.stringify(got) !== JSON.stringify(want)) { console.error(`check-search: ${JSON.stringify(input)} -> ${JSON.stringify(got)}, want ${JSON.stringify(want)}`); bad++; }
}
if (bad) process.exit(1);
console.log(`check-search: tokeniser agrees with the fixture on ${cases.length} cases`);
