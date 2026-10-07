// zotero-reference.mjs — record upstream Zotero's import/export output as the
// committed reference for kovan-literature's translator port (GitHub #752,
// #749, epic #747).
//
// Run through scripts/zotero-reference.sh, which checks the server first.
//
// WHAT IT DOES
//
// Sends every fixture through a RUNNING Zotero translation-server
// (https://github.com/zotero/translation-server; default
// http://127.0.0.1:1969, override with ZOTERO_SERVER) and writes what upstream
// returned under crates/kovan-literature/tests/data/zotero/reference/. The
// Rust tests (crates/kovan-literature/tests/zotero_translators.rs) compare the
// port with these files, so CI needs neither Node nor the server.
//
//   import/<format>.json      every import fixture: input text, the
//                             translator the server detected, its items
//   export_inputs/<set>.json  named item lists posted to /export
//   export/<format>/<set>.json   upstream's export text for each list
//   roundtrip/<format>/<set>.json  the export text posted back to /import
//   utilities.json            Zotero.Utilities helpers (unescapeHTML,
//                             cleanAuthor, cleanDOI, ...) run directly from
//                             the server's utilities.js on fixed inputs
//
//   The RDF translators (#749) add, through the same server:
//   import/rdf.json, import/rdf_bibliontology.json
//                             RDF.js and Bibliontology RDF.js testCases and
//                             fixtures/import/*.rdf (*.bibo.rdf for
//                             Bibliontology), with the translator the server
//                             detected (it may be the other one)
//   export_inputs_rdf/<set>.json  the items those imports produced, and the
//                             items of fixtures/library/*.json; kept apart
//                             from export_inputs/ so the four earlier
//                             formats' references are unchanged
//   export/<rdf format>/<set>.json, roundtrip/<rdf format>/<set>.json
//                             every export_inputs/ and export_inputs_rdf/
//                             list through rdf_zotero, rdf_bibliontology and
//                             rdf_dc, and the text back through /import (for
//                             the export-only formats too: that round trip is
//                             the point of Zotero RDF)
//   and, run in-process from the server's own modules because its endpoints
//   cannot do it (scripts/zotero-reference-inproc.mjs, which documents it):
//   inproc/forced_import.json  RDF.js / Bibliontology imports with the
//                             translator forced, as Zotero's translator
//                             tests run testCases
//   inproc/library.json       the library round trip with collections
//   manifest.json            server, translate, utilities, translators and
//                             schema commits; time zone; date; normalisation
//
// Fixtures:
//   * every `testCases` entry of the four translators (vendor/translators,
//     BibTeX.js, BibLaTeX.js (none), RIS.js, CSL JSON.js), input verbatim;
//   * crates/kovan-literature/tests/data/zotero/fixtures/import/* (one
//     upstream file, book_and_child_note.ris from zotero/test/tests/data, and
//     kovan-authored probes);
//   * export inputs: upstream's itemJSON test data (the copy in
//     crates/kovan-common/tests/data/zotero/itemJSON.json), one list per item
//     type plus all of them in one list; the items each import fixture
//     produced; and fixtures/export/kovan_probe_items.json (kovan-authored).
//
// RDF exports are stored verbatim too. Their blank-node ids (rdf:nodeID="n42")
// come from a counter global to the server process, so they differ run to
// run; the Rust tests start the port's counter where the server's was, read
// from the first nodeID in the output.
//
// NORMALISATION — exactly one thing, and only on /import output: the server's
// itemToAPIJSON gives each item a RANDOM 8-character `key`
// (Zotero.Utilities.generateObjectKey) and child notes a `parentItem` naming
// it. Each distinct key is replaced, in order of first appearance, by the n-th
// key of a fixed sequence (normKey below; the Rust port's default key
// generator yields the same sequence), and `parentItem` is rewritten with the
// same map. Nothing else is touched. Export output is stored verbatim.
//
// Child notes are folded into their parent's `notes` array before export, as
// Zotero's own export does for a library item (itemToExportFormat); the Rust
// side folds the same way (zotero::framework::fold_child_notes).

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

const SERVER = process.env.ZOTERO_SERVER || "http://127.0.0.1:1969";
const REPO = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
// vendor/ is gitignored and lives in the main checkout; from a worktree under
// .claude/worktrees/<name>/ it is three levels up.
const VENDOR = process.env.ZOTERO_VENDOR
	|| [path.join(REPO, "vendor"), path.resolve(REPO, "../../../vendor")]
		.find((p) => fs.existsSync(path.join(p, "translators")));
const DATA = path.join(REPO, "crates/kovan-literature/tests/data/zotero");
// ZOTERO_REFERENCE_OUT writes elsewhere (to compare a regeneration with the
// committed references before replacing them).
const OUT = process.env.ZOTERO_REFERENCE_OUT || path.join(DATA, "reference");

const FORMATS = {
	bibtex: { file: "BibTeX.js", import: true },
	biblatex: { file: "BibLaTeX.js", import: false },
	ris: { file: "RIS.js", import: true },
	csljson: { file: "CSL JSON.js", import: true },
};

const KEY_CHARS = "23456789ABCDEFGHIJKLMNPQRSTUVWXYZ";
// The n-th (0-based) normalised key: "KVN" then n in base 33 over Zotero's
// allowed key characters, five digits ("KVN22222", "KVN22223", ...).
function normKey(n) {
	let s = "";
	for (let i = 0; i < 5; i++) {
		s = KEY_CHARS[n % 33] + s;
		n = Math.floor(n / 33);
	}
	return "KVN" + s;
}

function normalise(items) {
	const map = new Map();
	const get = (k) => {
		if (!map.has(k)) map.set(k, normKey(map.size));
		return map.get(k);
	};
	for (const it of items) {
		if (typeof it.key === "string") it.key = get(it.key);
	}
	for (const it of items) {
		if (typeof it.parentItem === "string") it.parentItem = get(it.parentItem);
	}
	return items;
}

function foldChildNotes(items) {
	const out = [];
	const byKey = new Map();
	for (const it of items) {
		if (it.itemType === "note" && it.parentItem && byKey.has(it.parentItem)) {
			const parent = byKey.get(it.parentItem);
			(parent.notes = parent.notes || []).push(it);
			continue;
		}
		const copy = JSON.parse(JSON.stringify(it));
		out.push(copy);
		if (copy.key) byKey.set(copy.key, copy);
	}
	return out;
}

function readHead(repoDir) {
	// The commit a checkout is at, read from its .git without running git
	// (a submodule's .git is a file pointing at its git dir).
	let gitPath = path.join(repoDir, ".git");
	if (!fs.existsSync(gitPath)) return null;
	if (fs.statSync(gitPath).isFile()) {
		const m = fs.readFileSync(gitPath, "utf8").match(/gitdir:\s*(.*)/);
		gitPath = path.resolve(repoDir, m[1].trim());
	}
	let head = fs.readFileSync(path.join(gitPath, "HEAD"), "utf8").trim();
	if (head.startsWith("ref:")) {
		const ref = head.slice(4).trim();
		const refFile = path.join(gitPath, ref);
		if (fs.existsSync(refFile)) return fs.readFileSync(refFile, "utf8").trim();
		const packed = path.join(gitPath, "packed-refs");
		if (fs.existsSync(packed)) {
			for (const line of fs.readFileSync(packed, "utf8").split("\n")) {
				const [sha, name] = line.split(" ");
				if (name === ref) return sha;
			}
		}
		return null;
	}
	return head;
}

function testCases(file) {
	const src = fs.readFileSync(path.join(VENDOR, "translators", file), "utf8");
	const i = src.indexOf("/** BEGIN TEST CASES **/");
	if (i < 0) return [];
	const j = src.indexOf("/** END TEST CASES **/");
	// The block is a JS literal (`var testCases = [...]`), not JSON.
	return new Function(src.slice(i + 24, j) + "\n;return testCases;")();
}

async function postImport(text) {
	const res = await fetch(SERVER + "/import", {
		method: "POST",
		headers: { "Content-Type": "text/plain" },
		body: text,
	});
	const body = await res.text();
	const rec = { status: res.status, translatorID: res.headers.get("zotero-translator-id") };
	if (res.status === 200) rec.items = normalise(JSON.parse(body));
	else rec.error = body;
	return rec;
}

async function postExport(format, items) {
	const res = await fetch(SERVER + "/export?format=" + format, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(items),
	});
	const body = await res.text();
	return res.status === 200 ? { status: 200, output: body } : { status: res.status, error: body };
}

function write(rel, value) {
	const p = path.join(OUT, rel);
	fs.mkdirSync(path.dirname(p), { recursive: true });
	fs.writeFileSync(p, JSON.stringify(value, null, "\t") + "\n");
}

async function main() {
	fs.rmSync(OUT, { recursive: true, force: true });
	const exportInputs = {};

	// 1. Import fixtures.
	const fixtureDir = path.join(DATA, "fixtures/import");
	const fixtureFiles = fs.readdirSync(fixtureDir).sort();
	for (const [format, def] of Object.entries(FORMATS)) {
		if (!def.import) continue;
		const cases = [];
		testCases(def.file).forEach((tc, n) => {
			if (tc.type !== "import") return;
			cases.push({
				name: `testCase${String(n).padStart(2, "0")}`,
				source: `translators/${def.file} testCases[${n}]`,
				input: tc.input,
				upstreamTestItems: tc.items,
			});
		});
		const ext = { bibtex: ".bib", ris: ".ris", csljson: ".json" }[format];
		for (const f of fixtureFiles.filter((f) => f.endsWith(ext))) {
			cases.push({
				name: f.replace(/\.[^.]+$/, ""),
				source: `fixtures/import/${f}`,
				input: fs.readFileSync(path.join(fixtureDir, f), "utf8"),
			});
		}
		const inputs = {};
		for (const c of cases) {
			Object.assign(c, await postImport(c.input));
			if (c.items && c.items.length) inputs[c.name] = foldChildNotes(c.items);
		}
		write(`import/${format}.json`, cases);
		exportInputs[`import-${format}`] = inputs;
	}

	// 2. Export inputs from upstream's item JSON and the kovan probes.
	const itemJSON = JSON.parse(
		fs.readFileSync(path.join(REPO, "crates/kovan-common/tests/data/zotero/itemJSON.json"), "utf8")
	);
	const ij = {};
	for (const [type, item] of Object.entries(itemJSON)) ij[type] = [item];
	ij.all = Object.values(itemJSON);
	exportInputs.itemJSON = ij;
	const probes = JSON.parse(
		fs.readFileSync(path.join(DATA, "fixtures/export/kovan_probe_items.json"), "utf8")
	);
	const kp = { all: probes };
	for (const it of probes) kp[it.key] = [it];
	exportInputs.kovanProbes = kp;

	for (const [set, lists] of Object.entries(exportInputs)) write(`export_inputs/${set}.json`, lists);

	// 3. Export every list in every format; re-import what can be imported.
	for (const [format, def] of Object.entries(FORMATS)) {
		for (const [set, lists] of Object.entries(exportInputs)) {
			const exp = {};
			const rt = {};
			for (const [name, items] of Object.entries(lists)) {
				exp[name] = await postExport(format, JSON.parse(JSON.stringify(items)));
				if (def.import && exp[name].status === 200) {
					rt[name] = await postImport(exp[name].output);
				}
			}
			write(`export/${format}/${set}.json`, exp);
			if (def.import) write(`roundtrip/${format}/${set}.json`, rt);
		}
	}

	// 3b. RDF translators (#749): imports, exports and round trips through the
	// server, then the in-process references.
	await rdfReferences(exportInputs);
	execFileSync(process.execPath, [path.join(path.dirname(new URL(import.meta.url).pathname), "zotero-reference-inproc.mjs"), VENDOR, DATA, OUT], { stdio: "inherit" });

	// 4. Zotero.Utilities helpers the translators call, run directly from the
	// server's own utilities.js (the code the server runs, with its jsdom), on
	// the inputs in fixtures/utilities.json plus every string field value of
	// every import fixture's items (unescapeHTML only).
	const ts = path.join(VENDOR, "translation-server");
	globalThis.Zotero = { isNode: true, debug() {} };
	const ZU = (await import(path.join(ts, "modules/utilities/utilities.js"))).default;
	const util = JSON.parse(fs.readFileSync(path.join(DATA, "fixtures/utilities.json"), "utf8"));
	const html = new Set(util.unescapeHTML);
	for (const format of ["bibtex", "ris", "csljson"]) {
		const cases = JSON.parse(fs.readFileSync(path.join(OUT, `import/${format}.json`), "utf8"));
		for (const c of cases) {
			for (const line of c.input.split(/\r?\n/)) if (/[<&]/.test(line)) html.add(line);
		}
	}
	write("utilities.json", {
		unescapeHTML: [...html].map((s) => [s, ZU.unescapeHTML(s)]),
		cleanAuthor: util.cleanAuthor.map(([s, type, useComma]) => [s, type, useComma, ZU.cleanAuthor(s, type, useComma)]),
		cleanDOI: util.cleanDOI.map((s) => [s, ZU.cleanDOI(s)]),
		text2html: util.text2html.map(([s, p]) => [s, p, ZU.text2html(s, p)]),
		trimInternal: util.trimInternal.map((s) => [s, ZU.trimInternal(s)]),
		removeDiacritics: util.removeDiacritics.map(([s, lc]) => [s, lc, ZU.removeDiacritics(s, lc)]),
	});

	// 5. Manifest.
	write("manifest.json", {
		generatedBy: "scripts/zotero-reference.sh",
		generatedOn: new Date().toISOString(),
		server: SERVER,
		node: process.version,
		utcOffsetMinutes: -new Date().getTimezoneOffset(),
		commits: {
			"translation-server": readHead(ts),
			"translation-server/modules/translate": readHead(path.join(ts, "modules/translate")),
			"translation-server/modules/utilities": readHead(path.join(ts, "modules/utilities")),
			"translation-server/modules/zotero-schema": readHead(path.join(ts, "modules/zotero-schema")),
			translators: readHead(path.join(VENDOR, "translators")),
		},
		notes: [
			"The server was started with translatorsDirectory = vendor/translators, so the translators are the commit above, not the server's own submodule.",
			"Type/field validity on the server comes from modules/utilities/resource/zoteroTypeSchemaData.js; CSL mappings from modules/zotero-schema/schema.json.",
			"Normalisation: /import output keys and parentItem only (see scripts/zotero-reference.mjs). Export output is verbatim.",
			"utcOffsetMinutes is this script's zone; the server ran on the same machine.",
			"RDF exports (#749) are verbatim; their rdf:nodeID numbers come from a counter global to the server process (see the RDF section above).",
			"inproc/ was produced in-process from the server's own modules (scripts/zotero-reference-inproc.mjs): the saved items minus their random `id`, collections written as {name, children}.",
		],
	});
}

const RDF_EXPORTS = {
	rdf_zotero: "Zotero RDF.js",
	rdf_bibliontology: "Bibliontology RDF.js",
	rdf_dc: "Unqualified Dublin Core RDF.js",
};
const RDF_IMPORTS = {
	rdf: ["RDF.js", (f) => f.endsWith(".rdf") && !f.endsWith(".bibo.rdf")],
	rdf_bibliontology: ["Bibliontology RDF.js", (f) => f.endsWith(".bibo.rdf")],
};

async function rdfReferences(exportInputs) {
	const fixtureDir = path.join(DATA, "fixtures/import");
	const fixtureFiles = fs.readdirSync(fixtureDir).sort();
	const rdfInputs = {};
	for (const [format, [file, pick]] of Object.entries(RDF_IMPORTS)) {
		const cases = [];
		testCases(file).forEach((tc, n) => {
			if (tc.type !== "import") return;
			cases.push({
				name: `testCase${String(n).padStart(2, "0")}`,
				source: `translators/${file} testCases[${n}]`,
				input: tc.input,
				upstreamTestItems: tc.items,
			});
		});
		for (const f of fixtureFiles.filter(pick)) {
			cases.push({
				name: f.replace(/\.[^.]+$/, ""),
				source: `fixtures/import/${f}`,
				input: fs.readFileSync(path.join(fixtureDir, f), "utf8"),
			});
		}
		const inputs = {};
		for (const c of cases) {
			Object.assign(c, await postImport(c.input));
			if (c.items && c.items.length) inputs[c.name] = foldChildNotes(c.items);
		}
		write(`import/${format}.json`, cases);
		rdfInputs[`import-${format}`] = inputs;
	}
	const libDir = path.join(DATA, "fixtures/library");
	const lib = {};
	for (const f of fs.readdirSync(libDir).filter((f) => f.endsWith(".json")).sort()) {
		lib[f.replace(/\.json$/, "")] = JSON.parse(fs.readFileSync(path.join(libDir, f), "utf8")).items;
	}
	rdfInputs.kovanLibrary = lib;
	for (const [set, lists] of Object.entries(rdfInputs)) write(`export_inputs_rdf/${set}.json`, lists);

	const allSets = { ...exportInputs, ...rdfInputs };
	for (const format of Object.keys(RDF_EXPORTS)) {
		for (const [set, lists] of Object.entries(allSets)) {
			const exp = {};
			const rt = {};
			for (const [name, items] of Object.entries(lists)) {
				exp[name] = await postExport(format, JSON.parse(JSON.stringify(items)));
				if (exp[name].status === 200) rt[name] = await postImport(exp[name].output);
			}
			write(`export/${format}/${set}.json`, exp);
			write(`roundtrip/${format}/${set}.json`, rt);
		}
	}
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
