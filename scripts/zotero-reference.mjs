// zotero-reference.mjs — record upstream Zotero's import/export output as the
// committed reference for kovan-literature's translator port (GitHub #752,
// #749, epic #747).
//
// Run through scripts/zotero-reference.sh, which checks the server first.
//
// WHAT IT DOES
//
// (#749: some fixtures go through the same server code loaded in-process
// instead, see IN-PROCESS UPSTREAM below.)
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
//   manifest.json            server, translate, utilities, translators and
//                             schema commits; time zone; date; normalisation
//   chain_inputs/<format>.json, chain/<format>.json  (#749, MODS and Endnote
//                             XML) each own import fixture's items exported
//                             in the same format and re-imported
//   notes/inputs.json, notes/<format>.json  (#749) the note probes exported
//                             by Note HTML and Note Markdown (in-process)
//
// The XML translators' tests are crates/kovan-literature/tests/
// zotero_xml_translators.rs.
//
// Fixtures:
//   * every import `testCases` entry of the translators in FORMATS
//     (vendor/translators; BibTeX.js, RIS.js, CSL JSON.js and, since #749,
//     the tagged-text, JSON and XML importers), input verbatim;
//   * fixtures/import/<format>/* for the XML translators (#749): a DSpace
//     document derived from DSpace's testCase, kovan-authored Citavi probes;
//   * crates/kovan-literature/tests/data/zotero/fixtures/import/* (one
//     upstream file, book_and_child_note.ris from zotero/test/tests/data, and
//     kovan-authored probes);
//   * export inputs: upstream's itemJSON test data (the copy in
//     crates/kovan-common/tests/data/zotero/itemJSON.json), one list per item
//     type plus all of them in one list; the items each import fixture
//     produced; and fixtures/export/kovan_probe_items.json (kovan-authored).
//     The XML import fixtures are not export sets (see FORMATS);
//   * fixtures/export/kovan_note_items.json (kovan-authored note probes,
//     #749), exported only by the note exporters.
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
import { createRequire } from "node:module";

const SERVER = process.env.ZOTERO_SERVER || "http://127.0.0.1:1969";
const REPO = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
// vendor/ is gitignored and lives in the main checkout; from a worktree under
// .claude/worktrees/<name>/ it is three levels up.
const VENDOR = process.env.ZOTERO_VENDOR
	|| [path.join(REPO, "vendor"), path.resolve(REPO, "../../../vendor")]
		.find((p) => fs.existsSync(path.join(p, "translators")));
const DATA = path.join(REPO, "crates/kovan-literature/tests/data/zotero");
const OUT = path.join(DATA, "reference");

const FORMATS = {
	bibtex: { file: "BibTeX.js", import: true },
	biblatex: { file: "BibLaTeX.js", import: false },
	ris: { file: "RIS.js", import: true },
	csljson: { file: "CSL JSON.js", import: true },
	// Tagged-text, JSON and simple export translators (#749). `inProcess`:
	// recorded through IN-PROCESS upstream (below). `ext`: the extension of
	// their files in fixtures/import/ (never ".json", which is CSL JSON's).
	// `server`: the translation-server's /export format name (formats.js),
	// when it has one. `export: false`: import only.
	refer: { file: "ReferBibIX.js", import: true, inProcess: true, ext: ".refer", server: "refer" },
	refworks_tagged: { file: "RefWorks Tagged.js", import: true, inProcess: true, ext: ".refworks", server: "refworks_tagged" },
	bookmarks: { file: "Bookmarks.js", import: true, inProcess: true, ext: ".bookmarks", server: "bookmarks" },
	medline_nbib: { file: "MEDLINEnbib.js", import: true, export: false, inProcess: true, ext: ".nbib" },
	ovid_tagged: { file: "OVID Tagged.js", import: true, export: false, inProcess: true, ext: ".ovid" },
	wos_tagged: { file: "Web of Science Tagged.js", import: true, export: false, inProcess: true, ext: ".wos" },
	mab2: { file: "MAB2.js", import: true, export: false, inProcess: true, ext: ".mab2" },
	datacite_json: { file: "Datacite JSON.js", import: true, export: false, inProcess: true, ext: ".datacite" },
	openalex_json: { file: "OpenAlex JSON.js", import: true, export: false, inProcess: true, ext: ".openalex" },
	csv: { file: "CSV.js", import: false, inProcess: true, server: "csv" },
	coins: { file: "COinS.js", import: false, inProcess: true, server: "coins" },
	wikipedia: { file: "Wikipedia Citation Templates.js", import: false, inProcess: true, server: "wikipedia" },
	wikidata_quickstatements: { file: "Wikidata QuickStatements.js", import: false, inProcess: true },
	cff: { file: "CFF.js", import: false, inProcess: true },
	cff_references: { file: "CFF References.js", import: false, inProcess: true },
	evernote: { file: "Evernote.js", import: false, inProcess: true, server: "evernote" },
	// XML translators (#749). The key is the translation-server's /export
	// format name where it has one (mods, endnote_xml, tei: formats.js);
	// import-only translators have none and are named here. `export: false`:
	// no export. `chain`: the import fixtures' items are exported and
	// re-imported in this format only (written to chain_inputs/ and chain/,
	// not export_inputs/, so the other formats' export sets stay as they are).
	mods: { file: "MODS.js", import: true, chain: true },
	endnote_xml: { file: "Endnote XML.js", import: true, chain: true },
	tei: { file: "TEI.js", import: false },
	crossref_unixref_xml: { file: "Crossref Unixref XML.js", import: true, export: false },
	marcxml: { file: "MARCXML.js", import: true, export: false },
	marc: { file: "MARC.js", import: true, export: false },
	pubmed_xml: { file: "PubMed XML.js", import: true, export: false },
	mets: { file: "METS.js", import: true, export: false },
	primo_normalized_xml: { file: "Primo Normalized XML.js", import: true, export: false },
	dspace_intermediate_metadata: { file: "DSpace Intermediate Metadata.js", import: true, export: false },
	citavi5_xml: { file: "Citavi 5 XML.js", import: true, export: false },
	xml_contextobject: { file: "XML ContextObject.js", import: true, export: false },
	// Note exporters (#749, need the XML/DOM layer): no server format name,
	// so exported in-process.
	note_html: { file: "Note HTML.js", import: false, inProcess: true },
	note_markdown: { file: "Note Markdown.js", import: false, inProcess: true },
};

// The translatorID in a translator file's header.
function translatorId(file) {
	const src = fs.readFileSync(path.join(VENDOR, "translators", file), "utf8");
	return src.match(/"translatorID"\s*:\s*"([^"]+)"/)[1];
}

// ---------------------------------------------------------------------------
// IN-PROCESS UPSTREAM (#749). The translation-server's /import always picks a
// translator by detection, and its /export only knows the formats in
// formats.js. To run a given import translator, or an export translator
// with no format name, this script loads the SAME translation-server code
// (vendor/translation-server: src/zotero.js, translators.js,
// translation/translate.js, exportEndpoint.js, with the same submodules and
// the same translators directory) into this Node process and calls it. The
// running server is not touched. Every result the server can also produce
// is produced both ways and must agree (crossChecks in the manifest); a
// disagreement stops the script.
// ---------------------------------------------------------------------------
let inProc = null;
async function inProcess() {
	if (inProc) return inProc;
	const ts = path.join(VENDOR, "translation-server");
	process.env.NODE_CONFIG_DIR = path.join(ts, "config");
	process.env.NODE_CONFIG = JSON.stringify({ translatorsDirectory: path.join(VENDOR, "translators") });
	const req = createRequire(path.join(ts, "src/server.js"));
	const saved = globalThis.Zotero;
	req("./zotero.js");
	req("./debug").init(0);
	await req("./translators").init();
	inProc = {
		Translate: req("./translation/translate"),
		ExportEndpoint: req("./exportEndpoint"),
		formats: req("./formats"),
		Zotero: globalThis.Zotero,
	};
	globalThis.Zotero = saved;
	return inProc;
}

// importEndpoint.js:31-47 with `translate.setTranslator(id)` in place of
// detection.
async function inProcessImport(id, text) {
	const p = await inProcess();
	const saved = globalThis.Zotero;
	globalThis.Zotero = p.Zotero;
	try {
		const translate = new p.Translate.Import();
		translate.setString(text || "");
		translate.setTranslator(id);
		const items = await translate.translate({ libraryID: 1 });
		const out = [];
		items.forEach((item) => out.push(...p.Zotero.Utilities.Item.itemToAPIJSON(item)));
		// What a client parses from the server's JSON.stringify(newItems).
		return { status: 200, translatorID: id, items: normalise(JSON.parse(JSON.stringify(out))) };
	}
	catch (e) {
		return { status: 500, translatorID: id, error: String((e && e.message) || e) };
	}
	finally {
		globalThis.Zotero = saved;
	}
}

// exportEndpoint.js run as it is, with the format added to its FORMATS table
// (in this process only) when the server has no name for it.
async function inProcessExport(format, id, items) {
	const p = await inProcess();
	const saved = globalThis.Zotero;
	globalThis.Zotero = p.Zotero;
	if (!p.formats.FORMATS[format]) {
		p.formats.FORMATS[format] = id;
		p.formats.CONTENT_TYPES[format] = "text/plain";
	}
	const ctx = {
		request: { body: items, query: { format } },
		response: {},
		is: () => true,
		set() {},
		assert(c, status) {
			if (!c) {
				const e = new Error("assert");
				e.status = status;
				throw e;
			}
		},
		throw(status, msg) {
			const e = new Error(msg);
			e.status = status;
			throw e;
		},
	};
	try {
		await p.ExportEndpoint.handle(ctx);
		return { status: 200, output: ctx.response.body };
	}
	catch (e) {
		return { status: e.status || 500, error: e.message };
	}
	finally {
		globalThis.Zotero = saved;
	}
}

const crossChecks = { import: 0, export: 0 };
const sameJSON = (a, b) => JSON.stringify(a) === JSON.stringify(b);

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
	// Decoded keeping a leading U+FEFF: res.text() would strip it (WHATWG
	// UTF-8 decode), and CSV.js writes one (#749).
	const body = new TextDecoder("utf-8", { ignoreBOM: true }).decode(await res.arrayBuffer());
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
	const chainInputs = {};
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
		const ext = { bibtex: ".bib", ris: ".ris", csljson: ".json" }[format] || def.ext;
		for (const f of fixtureFiles.filter((f) => ext && f.endsWith(ext))) {
			cases.push({
				name: f.replace(/\.[^.]+$/, ""),
				source: `fixtures/import/${f}`,
				input: fs.readFileSync(path.join(fixtureDir, f), "utf8"),
			});
		}
		// XML translators (#749): fixtures/import/<format>/* (every file).
		const sub = path.join(fixtureDir, format);
		if (fs.existsSync(sub) && fs.statSync(sub).isDirectory()) {
			for (const f of fs.readdirSync(sub).sort()) {
				cases.push({
					name: f.replace(/\.[^.]+$/, ""),
					source: `fixtures/import/${format}/${f}`,
					input: fs.readFileSync(path.join(sub, f), "utf8"),
				});
			}
		}
		const inputs = {};
		for (const c of cases) {
			if (def.inProcess) {
				// #749: the translator forced, in-process. What the server's
				// detection picks is recorded beside it; when it picks this
				// translator, the server's output must be the same.
				const id = translatorId(def.file);
				const server = await postImport(c.input);
				Object.assign(c, await inProcessImport(id, c.input));
				c.via = "in-process, translator forced (zotero-reference.mjs inProcessImport)";
				c.serverTranslatorID = server.translatorID;
				if (server.translatorID === id) {
					if (!sameJSON(server.items || server.error, c.items || c.error)) {
						throw new Error(`in-process import differs from the server: ${format} ${c.name}`);
					}
					crossChecks.import++;
				}
			}
			else {
				Object.assign(c, await postImport(c.input));
			}
			if (c.items && c.items.length) inputs[c.name] = foldChildNotes(c.items);
		}
		write(`import/${format}.json`, cases);
		// The XML translators' fixtures (#749) are not export sets for every
		// format (see FORMATS); only the original three are.
		if (def.chain) chainInputs[format] = inputs;
		else if (ext) exportInputs[`import-${format}`] = inputs;
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
		if (def.export === false) continue;
		for (const [set, lists] of Object.entries(exportInputs)) {
			const exp = {};
			const rt = {};
			for (const [name, items] of Object.entries(lists)) {
				if (def.inProcess && !def.server) {
					// #749: no server format name: exportEndpoint.js in-process.
					exp[name] = await inProcessExport(format, translatorId(def.file), JSON.parse(JSON.stringify(items)));
					exp[name].via = "in-process (zotero-reference.mjs inProcessExport)";
				}
				else {
					exp[name] = await postExport(def.server || format, JSON.parse(JSON.stringify(items)));
					if (def.inProcess) {
						const ip = await inProcessExport(def.server, translatorId(def.file), JSON.parse(JSON.stringify(items)));
						if (ip.status !== exp[name].status || ip.output !== exp[name].output) {
							throw new Error(`in-process export differs from the server: ${format} ${set}/${name}\n${JSON.stringify(ip)}\n${JSON.stringify(exp[name])}`);
						}
						crossChecks.export++;
					}
				}
				if (def.import && exp[name].status === 200) {
					rt[name] = await postImport(exp[name].output);
				}
			}
			write(`export/${format}/${set}.json`, exp);
			if (def.import) write(`roundtrip/${format}/${set}.json`, rt);
		}
	}

	// 3b. XML translators (#749): each import fixture's items exported in
	// the same format and re-imported (the import -> export -> import chain).
	for (const [format, inputs] of Object.entries(chainInputs)) {
		write(`chain_inputs/${format}.json`, inputs);
		const out = {};
		for (const [name, items] of Object.entries(inputs)) {
			const exp = await postExport(format, JSON.parse(JSON.stringify(items)));
			out[name] = { export: exp };
			if (exp.status === 200) out[name].reimport = await postImport(exp.output);
		}
		write(`chain/${format}.json`, out);
	}

	// 3c. Note exporters (#749): no export list above holds a top-level note
	// (child notes are folded into their parents), so the kovan-authored
	// note probes (fixtures/export/kovan_note_items.json: one list per item
	// and all together) are exported by Note HTML and Note Markdown,
	// in-process (no server format name).
	{
		const probes = JSON.parse(
			fs.readFileSync(path.join(DATA, "fixtures/export/kovan_note_items.json"), "utf8")
		);
		const lists = { all: probes };
		for (const it of probes) lists[it.key] = [it];
		write("notes/inputs.json", lists);
		for (const format of ["note_html", "note_markdown"]) {
			const def = FORMATS[format];
			const out = {};
			for (const [name, items] of Object.entries(lists)) {
				out[name] = await inProcessExport(format, translatorId(def.file), JSON.parse(JSON.stringify(items)));
				out[name].via = "in-process (zotero-reference.mjs inProcessExport)";
			}
			write(`notes/${format}.json`, out);
		}
	}

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
		// #749.
		capitalizeTitle: (util.capitalizeTitle || []).map((s) => [s, ZU.capitalizeTitle(s, true)]),
		cleanISBN: (util.cleanISBN || []).map((s) => [s, ZU.cleanISBN(s)]),
		cleanISSN: (util.cleanISSN || []).map((s) => [s, ZU.cleanISSN(s)]),
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
			"#749 formats (every FORMATS entry with inProcess): imports run in-process with the translator forced (the server's own detection is recorded as serverTranslatorID); exports run through the server when it has a format name, else exportEndpoint.js in-process. crossChecks counts the results produced both ways; all were identical (the script stops otherwise).",
		],
		crossChecks,
	});
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
