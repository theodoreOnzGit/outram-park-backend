// zotero-reference-inproc.mjs — the part of the Zotero reference outputs that
// the translation-server's HTTP endpoints cannot produce (GitHub #749, #752,
// epic #747). Run by scripts/zotero-reference.mjs in a child process; not
// meant to be run by hand.
//
// WHAT IT DOES
//
// Loads the translation-server's own modules (src/zotero.js, its translate
// and utilities submodules, the translators in vendor/translators) into this
// Node process, exactly as the server's src/server.js does, and runs
// translations through them directly. It does not start or contact a server
// and installs nothing; it reads the server checkout's node_modules.
//
// Two things need this:
//
//   inproc/forced_import.json  each import fixture of the RDF translators
//       run with THAT translator (Translate.Import#setTranslator), as Zotero's
//       own translator tests run a testCase. /import picks a translator by
//       detection instead (RDF.js testCases with BIBO classes go to
//       Bibliontology RDF), so the server references alone never exercise
//       RDF.js on them. Recorded per case: the saved items as the translator
//       framework hands them to the item saver (after _itemDone), the
//       collections it completed, and those items through itemToAPIJSON.
//
//   inproc/library.json  the library round trip: each
//       fixtures/library/*.json ({items, collections}) exported with Zotero
//       RDF the way the server's /export does it (exportEndpoint.js's item
//       preparation, copied below) but with the collections handed out by
//       nextCollection(), as Zotero desktop does (the server's ItemGetter
//       returns none); then that text imported with RDF.js as above.
//
// NORMALISATION (both outputs):
//   * items keep everything but the random `id` _itemDone gives each saved
//     item (Zotero.Utilities.randomString(); itemToAPIJSON drops it too);
//   * itemToAPIJSON keys are renumbered exactly as zotero-reference.mjs does;
//   * collections are written as {name, children: [{type: "item", id} |
//     {type: "collection", name, children}]}: RDF.js builds nested
//     collections as arrays carrying properties, which JSON.stringify would
//     write as [];
//   * firstBlankNodeId records the RDF library's global blank-node counter
//     (rdf/term.js Term.NextId) when each translation started, so the port can
//     number its blank nodes the same way.

import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const [VENDOR, DATA, OUT] = process.argv.slice(2);
const TS = path.join(VENDOR, "translation-server");
process.env.NODE_CONFIG_DIR = path.join(TS, "config");
process.env.TRANSLATORS_DIR = path.join(VENDOR, "translators");
const require = createRequire(path.join(TS, "src/server.js"));
require(path.join(TS, "src/zotero.js"));
require(path.join(TS, "src/debug.js")).init(0);
await require(path.join(TS, "src/translators.js")).init();
const Translate = require(path.join(TS, "src/translation/translate.js"));
const Term = require(path.join(TS, "modules/translate/src/rdf/term.js"));

const ID = {
	rdf: "5e3ad958-ac79-463d-812b-a86a9235c28f",
	rdf_bibliontology: "14763d25-8ba0-45df-8f52-b8d1108e7ac9",
	rdf_zotero: "14763d24-8ba0-45df-8f52-b8d1108e7ac9",
};

const KEY_CHARS = "23456789ABCDEFGHIJKLMNPQRSTUVWXYZ";
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
	for (const it of items) if (typeof it.key === "string") it.key = get(it.key);
	for (const it of items) if (typeof it.parentItem === "string") it.parentItem = get(it.parentItem);
	return items;
}

function serCollection(c) {
	return {
		name: c.name === undefined ? "" : String(c.name),
		children: (c.children || []).map((ch) =>
			ch.type === "item" ? { type: "item", id: String(ch.id) } : { type: "collection", ...serCollection(ch) }
		),
	};
}

async function importWith(translatorID, text) {
	const t = new Translate.Import();
	t.setString(text);
	t.setTranslator(translatorID);
	const firstBlankNodeId = Term.NextId;
	try {
		const items = await t.translate({ libraryID: 1 });
		const saved = JSON.parse(JSON.stringify(items));
		for (const it of saved) delete it.id;
		const api = [];
		for (const it of JSON.parse(JSON.stringify(items))) {
			api.push(...Zotero.Utilities.Item.itemToAPIJSON(it));
		}
		return {
			firstBlankNodeId,
			items: saved,
			collections: (t.newCollections || []).map(serCollection),
			apiItems: normalise(JSON.parse(JSON.stringify(api))),
		};
	} catch (e) {
		return { firstBlankNodeId, error: String(e && e.message ? e.message : e) };
	}
}

async function exportZoteroRdf(items, collections) {
	const translatorID = ID.rdf_zotero;
	const translator = Zotero.Translators.get(translatorID);
	const legacy = Zotero.Utilities.semverCompare("4.0.27", translator.metadata.minVersion) > 0;
	// exportEndpoint.js:59-77, verbatim in effect.
	for (const item of items) {
		if (!item.uri) {
			item.uri = item.key;
			delete item.key;
		}
		if (legacy) {
			if (item.dateAdded) item.dateAdded = Zotero.Date.isoToSQL(item.dateAdded);
			if (item.dateModified) item.dateAdded = Zotero.Date.isoToSQL(item.dateModified);
			if (item.accessDate) item.accessDate = Zotero.Date.isoToSQL(item.accessDate);
		}
	}
	const queue = collections.slice();
	const proto = Translate.ItemGetter.prototype;
	const original = proto.nextCollection;
	proto.nextCollection = function () {
		return queue.length ? queue.shift() : false;
	};
	const t = new Translate.Export();
	t.setTranslator(translatorID);
	t.setItems(items);
	const firstBlankNodeId = Term.NextId;
	try {
		const output = await new Promise((resolve, reject) => {
			t.setHandler("done", (obj, status) => (status ? resolve(t.string) : reject(new Error("export failed"))));
			t.translate();
		});
		return { firstBlankNodeId, output };
	} catch (e) {
		return { firstBlankNodeId, error: String(e && e.message ? e.message : e) };
	} finally {
		proto.nextCollection = original;
	}
}

function testCases(file) {
	const src = fs.readFileSync(path.join(VENDOR, "translators", file), "utf8");
	const i = src.indexOf("/** BEGIN TEST CASES **/");
	if (i < 0) return [];
	const j = src.indexOf("/** END TEST CASES **/");
	return new Function(src.slice(i + 24, j) + "\n;return testCases;")();
}

function write(rel, value) {
	const p = path.join(OUT, rel);
	fs.mkdirSync(path.dirname(p), { recursive: true });
	fs.writeFileSync(p, JSON.stringify(value, null, "\t") + "\n");
}

// 1. Forced-translator imports.
const fixtureDir = path.join(DATA, "fixtures/import");
const fixtures = fs.readdirSync(fixtureDir).sort();
const forced = {};
for (const [format, file, pick] of [
	["rdf", "RDF.js", (f) => f.endsWith(".rdf") && !f.endsWith(".bibo.rdf")],
	["rdf_bibliontology", "Bibliontology RDF.js", (f) => f.endsWith(".rdf")],
]) {
	const cases = [];
	testCases(file).forEach((tc, n) => {
		if (tc.type !== "import") return;
		cases.push({ name: `testCase${String(n).padStart(2, "0")}`, source: `translators/${file} testCases[${n}]`, input: tc.input });
	});
	for (const f of fixtures.filter(pick)) {
		cases.push({ name: f.replace(/\.[^.]+$/, ""), source: `fixtures/import/${f}`, input: fs.readFileSync(path.join(fixtureDir, f), "utf8") });
	}
	for (const c of cases) Object.assign(c, await importWith(ID[format], c.input));
	forced[format] = cases;
}
write("inproc/forced_import.json", forced);

// 2. Library round trip.
const libDir = path.join(DATA, "fixtures/library");
const library = {};
for (const f of fs.readdirSync(libDir).filter((f) => f.endsWith(".json")).sort()) {
	const lib = JSON.parse(fs.readFileSync(path.join(libDir, f), "utf8"));
	const exp = await exportZoteroRdf(JSON.parse(JSON.stringify(lib.items)), JSON.parse(JSON.stringify(lib.collections)));
	const rec = { source: `fixtures/library/${f}`, export: exp };
	if (exp.output !== undefined) rec.import = await importWith(ID.rdf, exp.output);
	library[f.replace(/\.json$/, "")] = rec;
}
write("inproc/library.json", library);
