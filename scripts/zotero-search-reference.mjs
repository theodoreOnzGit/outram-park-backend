// zotero-search-reference.mjs — recorded HTTP responses and upstream Zotero's
// output for kovan-literature's port of Zotero's identifier lookup (search
// translators; GitHub #756, epic #747).
//
// TWO MODES
//
//   node scripts/zotero-search-reference.mjs --record
//       NETWORK. Run only by a human, on purpose. For every live case in
//       CASES below, runs upstream Zotero's search IN-PROCESS (the
//       translation-server's own code, see zotero-reference.mjs "IN-PROCESS
//       UPSTREAM") against the real services, and records every HTTP
//       exchange the translators make, raw, as a fixture:
//         crates/kovan-literature/tests/data/zotero/search/fixtures/<case>.json
//       One request at a time, at least DELAY_MS apart (arXiv asks for one
//       request every 3 s; the others ask for less), with a descriptive
//       User-Agent (USER_AGENT below; Crossref's polite pool wants a mailto,
//       which is taken from KOVAN_LOOKUP_MAILTO if the human running this
//       sets it, and is never hard-coded). Synthetic cases are not fetched.
//       Redaction: only the response's Content-Type is kept from the response
//       headers (no cookies); request headers are kept except User-Agent.
//
//   node scripts/zotero-search-reference.mjs
//       OFFLINE (the default). Replays the committed fixtures: upstream's
//       translators run in-process with every HTTP request answered from the
//       fixture (a request the fixture does not hold fails as a network
//       error), and what upstream returned is written as the reference:
//         crates/kovan-literature/tests/data/zotero/search/reference/<case>.json
//       plus reference/extract_identifiers.json (Zotero.Utilities.
//       extractIdentifiers on fixtures/extract_identifiers.json).
//
// HOW REQUESTS ARE INTERCEPTED. The translation-server's Zotero.HTTP.request
// (src/http.js) sends through the `request` npm module. Before http.js is
// loaded, `request` is replaced in Node's module cache by a stand-in that
// either passes through to the real module and tees what comes back
// (--record) or answers from the fixture (replay). Everything above it —
// status checks, charset decoding, JSON parsing, jsdom documents, the
// translators, the framework — is upstream's own code, unchanged.
//
// WHAT A REFERENCE HOLDS: the input, the translators upstream detected (in
// order) and the one whose items were kept, the requests it made (method,
// URL, body), and the items as the server's /search returns them
// (itemToAPIJSON), or the error. Normalisation, the only two: item keys and
// parentItem (random) are renumbered as zotero-reference.mjs does (normKey),
// and an accessDate the framework stamped during this run (Web _itemDone
// sets the clock time when an item has a URL) becomes "NOW".
//
// SYNTHETIC CASES (fixtures/synthetic_*.json) are kovan-authored responses
// for services that were deliberately not contacted: ADS (its API needs an
// access token from a bootstrap call; DATA_POLICY.md forbids seeking tokens),
// Câmara Brasileira do Livro (its search API is called with an api-key),
// Open WorldCat (scraped behind a session token), mEDRA and Lulu (disabled
// upstream). Upstream runs on them exactly as on recorded ones, so the
// comparison is still code-to-code; only the responses are not the
// services' own.

import fs from "node:fs";
import path from "node:path";
import Module, { createRequire } from "node:module";
import { EventEmitter } from "node:events";

const RECORD = process.argv.includes("--record");
const DELAY_MS = 3500;
const MAILTO = process.env.KOVAN_LOOKUP_MAILTO || "";
const USER_AGENT = "kovan-zotero-reference/0.1 (outram-park-backend kovan-literature; "
	+ "recording public bibliographic metadata fixtures for code-to-code tests; "
	+ "https://github.com/theodoreOnzGit/outram-park-backend"
	+ (MAILTO ? `; mailto:${MAILTO}` : "") + ")";

const REPO = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const VENDOR = process.env.ZOTERO_VENDOR
	|| [path.join(REPO, "vendor"), path.resolve(REPO, "../../../vendor")]
		.find((p) => fs.existsSync(path.join(p, "translators")));
const DATA = path.join(REPO, "crates/kovan-literature/tests/data/zotero/search");
const FIXTURES = path.join(DATA, "fixtures");
const OUT = path.join(DATA, "reference");

// ---------------------------------------------------------------- the cases
// kind "identifier": the translation-server's /search on this text
// (searchEndpoint.js: extractIdentifiers, the PMID rule, setIdentifier, every
// detected translator in priority order, the next one tried when one returns
// nothing). kind "forced": one translator on one search item
// (translate.setSearch + setTranslator), for translators no identifier
// reaches or whose detection another translator pre-empts. Identifiers come
// from each translator's own testCases where it has them.
const CASES = [
	{ name: "doi_crossref", kind: "identifier", text: "10.1109/TPS.1987.4316723" },
	{ name: "doi_datacite", kind: "identifier", text: "10.48550/arXiv.1706.03762" },
	{ name: "doi_other_ra", kind: "identifier", text: "10.12763/ONA1045" },
	{ name: "arxiv_new", kind: "identifier", text: "arXiv:1706.03762" },
	{ name: "arxiv_old", kind: "identifier", text: "math/0211159" },
	{ name: "arxiv_with_doi", kind: "identifier", text: "1207.7214" },
	{ name: "isbn_loc", kind: "identifier", text: "9780521779241" },
	{ name: "pmid", kind: "identifier", text: "31978945" },
	{ name: "eidr", kind: "identifier", text: "10.5240/6F7E-EF59-329B-1F0A-8440-2" },
	{ name: "crossref_rest_article", kind: "forced", translator: "Crossref REST.js", search: { itemType: "journalArticle", DOI: "10.1103/PhysRevB.110.245108" } },
	{ name: "crossref_rest_book", kind: "forced", translator: "Crossref REST.js", search: { itemType: "journalArticle", DOI: "10.1145/1947940" } },
	{ name: "crossref_rest_preprint", kind: "forced", translator: "Crossref REST.js", search: { itemType: "journalArticle", DOI: "10.1101/2020.04.07.20057075" } },
	{ name: "bnf_isbn", kind: "forced", translator: "BnF ISBN.js", search: { itemType: "book", ISBN: "9781841692203" } },
	{ name: "k10plus_isbn", kind: "forced", translator: "K10plus ISBN.js", search: { itemType: "book", ISBN: "9783830931492" } },
	{ name: "nlp_isbn", kind: "forced", translator: "National Library of Poland ISBN.js", search: { itemType: "book", ISBN: "8301136545" } },
	{ name: "libris_isbn", kind: "forced", translator: "LIBRIS ISBN.js", search: { itemType: "book", ISBN: "978-91-977109-4-7" } },
	{ name: "who_isbn", kind: "forced", translator: "WHO.js", search: { itemType: "book", ISBN: "9789241506236" } },
	{ name: "eric_journal", kind: "forced", translator: "ERIC.js", search: { ericNumber: "EJ1125432" } },
	{ name: "eric_report", kind: "forced", translator: "ERIC.js", search: { ericNumber: "ED616685" } },
	{ name: "openalex", kind: "forced", translator: "OpenAlex.js", search: { openAlex: "W2741809807" } },
];

// Synthetic cases: every fixtures/synthetic_*.json, each naming its own
// input ({kind, text} or {kind, translator, search}).
function syntheticCases() {
	if (!fs.existsSync(FIXTURES)) return [];
	return fs.readdirSync(FIXTURES).filter((f) => f.startsWith("synthetic_") && f.endsWith(".json")).sort()
		.map((f) => {
			const fx = JSON.parse(fs.readFileSync(path.join(FIXTURES, f), "utf8"));
			return { name: f.replace(/\.json$/, ""), ...fx.input, synthetic: true };
		});
}

// ------------------------------------------------- the `request` stand-in
const TS = path.join(VENDOR, "translation-server");
const tsRequire = createRequire(path.join(TS, "src/http.js"));
const realRequest = tsRequire("request");

let mode = null; // { record: [] } or { replay: Map<key, [exchange]> }
let log = []; // requests made by the current case

function requestKey(method, url, body) {
	return `${method} ${url}\n${body == null ? "" : body}`;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let lastRequestAt = 0;

function stubRequest(opts) {
	const em = new EventEmitter();
	em.abort = () => {};
	const method = opts.method || "GET";
	const url = typeof opts.uri === "string" ? opts.uri : opts.uri.href;
	const body = opts.body == null ? null : String(opts.body);
	const headers = { ...opts.headers };
	delete headers["User-Agent"];
	log.push({ method, url, headers, body });
	if (mode.replay) {
		const q = mode.replay.get(requestKey(method, url, body));
		setImmediate(() => {
			if (!q || !q.length) {
				em.emit("error", new Error(`kovan replay: no recorded response for ${method} ${url}`));
				return;
			}
			const ex = q.length > 1 ? q.shift() : q[0];
			if (ex.response.error) {
				em.emit("error", new Error(ex.response.error));
				return;
			}
			const buf = ex.response.bodyEncoding === "base64"
				? Buffer.from(ex.response.body, "base64")
				: Buffer.from(ex.response.body, "utf8");
			const res = {
				statusCode: ex.response.status,
				headers: ex.response.headers,
				body: buf,
				request: { uri: { href: ex.response.url } },
			};
			em.emit("response", res);
			em.emit("data", buf);
			em.emit("end");
		});
		return em;
	}
	// --record: real network, one request at a time, spaced.
	(async () => {
		const wait = lastRequestAt + DELAY_MS - Date.now();
		if (wait > 0) await sleep(wait);
		lastRequestAt = Date.now();
		console.error(`zotero-search-reference: ${method} ${url}`);
		const rec = { request: { method, url, headers, body }, response: null };
		mode.record.push(rec);
		const chunks = [];
		const real = realRequest({ ...opts, headers: { ...opts.headers, "User-Agent": USER_AGENT } });
		em.abort = () => real.abort();
		real.on("error", (e) => {
			rec.response = rec.response || { error: String(e && e.message || e) };
			em.emit("error", e);
		});
		real.on("response", (res) => {
			rec.response = {
				status: res.statusCode,
				url: res.request.uri.href,
				headers: res.headers["content-type"] ? { "content-type": res.headers["content-type"] } : {},
			};
			em.emit("response", res);
		});
		real.on("data", (c) => {
			chunks.push(c);
			em.emit("data", c);
		});
		real.on("end", () => {
			const buf = Buffer.concat(chunks);
			const text = buf.toString("utf8");
			if (Buffer.from(text, "utf8").equals(buf)) {
				rec.response.body = text;
				rec.response.bodyEncoding = "utf8";
			}
			else {
				rec.response.body = buf.toString("base64");
				rec.response.bodyEncoding = "base64";
			}
			em.emit("end");
		});
	})();
	return em;
}
stubRequest.jar = () => realRequest.jar();

// Install the stand-in where http.js will find it.
{
	const p = tsRequire.resolve("request");
	const m = new Module(p);
	m.filename = p;
	m.loaded = true;
	m.exports = stubRequest;
	Module._cache[p] = m;
}

// ------------------------------------------------------ in-process upstream
let up = null;
async function upstream() {
	if (up) return up;
	process.env.NODE_CONFIG_DIR = path.join(TS, "config");
	process.env.NODE_CONFIG = JSON.stringify({ translatorsDirectory: path.join(VENDOR, "translators") });
	process.env.USER_AGENT = USER_AGENT;
	const req = createRequire(path.join(TS, "src/server.js"));
	req("./zotero.js");
	req("./debug").init(0);
	req("./http");
	await req("./translators").init();
	up = { Translate: req("./translation/translate"), Zotero: globalThis.Zotero };
	return up;
}

const KEY_CHARS = "23456789ABCDEFGHIJKLMNPQRSTUVWXYZ";
function normKey(n) {
	let s = "";
	for (let i = 0; i < 5; i++) {
		s = KEY_CHARS[n % 33] + s;
		n = Math.floor(n / 33);
	}
	return "KVN" + s;
}

function normalise(items, runStart) {
	const map = new Map();
	const get = (k) => {
		if (!map.has(k)) map.set(k, normKey(map.size));
		return map.get(k);
	};
	for (const it of items) if (typeof it.key === "string") it.key = get(it.key);
	for (const it of items) if (typeof it.parentItem === "string") it.parentItem = get(it.parentItem);
	for (const it of items) {
		if (typeof it.accessDate === "string") {
			const t = Date.parse(it.accessDate);
			if (!Number.isNaN(t) && t >= runStart - 1000 && t <= Date.now() + 1000) it.accessDate = "NOW";
		}
	}
	return items;
}

function translatorFile(file) {
	const src = fs.readFileSync(path.join(VENDOR, "translators", file), "utf8");
	return src.match(/"translatorID"\s*:\s*"([^"]+)"/)[1];
}

// searchEndpoint.js handle + handleIdentifier (kind "identifier"), or
// handleIdentifier's body with the translator forced (kind "forced").
async function runCase(c) {
	const { Translate, Zotero } = await upstream();
	const runStart = Date.now();
	log = [];
	const out = { name: c.name, kind: c.kind };
	if (c.synthetic) out.synthetic = true;
	let translate;
	try {
		translate = new Translate.Search();
		if (c.kind === "identifier") {
			out.text = c.text;
			let identifiers = Zotero.Utilities.extractIdentifiers(c.text);
			if (identifiers.length && identifiers[0].PMID
					&& identifiers[0].PMID !== c.text.replace(/^\s*(?:pmid:)?([0-9]+)\s*$/, "$1")) {
				identifiers = [];
			}
			out.identifiers = identifiers;
			if (!identifiers.length) {
				out.status = 0;
				out.error = "no identifier (the server would run a text search)";
				return out;
			}
			translate.setIdentifier(identifiers[0]);
			const translators = await translate.getTranslators();
			out.detected = translators.map((t) => t.translatorID);
			if (!translators.length) {
				out.status = 501;
				out.error = "No translators available";
				return out;
			}
			translate.setTranslator(translators);
		}
		else {
			out.translator = translatorFile(c.translator);
			out.search = c.search;
			translate.setSearch(c.search);
			translate.setTranslator(out.translator);
		}
		const items = await translate.translate({ libraryID: false });
		out.status = 200;
		out.translatorUsed = translate.translator[0].translatorID;
		const newItems = [];
		items.forEach((item) => newItems.push(...Zotero.Utilities.Item.itemToAPIJSON(item)));
		out.items = normalise(JSON.parse(JSON.stringify(newItems)), runStart);
	}
	catch (e) {
		out.status = e == (translate && translate.ERROR_NO_RESULTS) ? 501 : 500;
		out.error = String((e && e.message) || e);
	}
	finally {
		out.requests = log.map(({ method, url, body }) => ({ method, url, body }));
	}
	return out;
}

function write(dir, name, value) {
	fs.mkdirSync(dir, { recursive: true });
	fs.writeFileSync(path.join(dir, name), JSON.stringify(value, null, "\t") + "\n");
}

async function main() {
	if (RECORD) {
		console.error(`zotero-search-reference: RECORDING (network) as "${USER_AGENT}"`);
		for (const c of CASES) {
			mode = { record: [] };
			const r = await runCase(c);
			write(FIXTURES, `${c.name}.json`, {
				case: c.name,
				input: c.kind === "identifier" ? { kind: c.kind, text: c.text } : { kind: c.kind, translator: c.translator, search: c.search },
				recordedOn: new Date().toISOString(),
				userAgent: USER_AGENT.replace(/; mailto:[^)]*/, "; mailto:<redacted>"),
				exchanges: mode.record,
			});
			console.error(`zotero-search-reference: ${c.name}: status ${r.status}, ${mode.record.length} request(s)`);
		}
	}
	// Replay every fixture (recorded and synthetic) and write references.
	// `--only <case>`: replay that one case and leave the other references
	// (and the manifest) as they are.
	const only = process.argv.includes("--only") ? process.argv[process.argv.indexOf("--only") + 1] : null;
	if (!only) fs.rmSync(OUT, { recursive: true, force: true });
	const cases = [...CASES, ...syntheticCases()].filter((c) => !only || c.name === only);
	if (only && !cases.length) throw new Error(`no case ${only}`);
	for (const c of cases) {
		const fxPath = path.join(FIXTURES, `${c.name}.json`);
		if (!fs.existsSync(fxPath)) {
			console.error(`zotero-search-reference: no fixture for ${c.name}; skipped`);
			continue;
		}
		const fx = JSON.parse(fs.readFileSync(fxPath, "utf8"));
		const replay = new Map();
		for (const ex of fx.exchanges) {
			const k = requestKey(ex.request.method, ex.request.url, ex.request.body);
			if (!replay.has(k)) replay.set(k, []);
			replay.get(k).push(ex);
		}
		mode = { replay };
		const r = await runCase(c);
		write(OUT, `${c.name}.json`, r);
		console.error(`zotero-search-reference: replayed ${c.name}: status ${r.status}`);
	}
	if (only) process.exit(0);
	// extractIdentifiers on fixed inputs.
	const { Zotero } = await upstream();
	const inputs = JSON.parse(fs.readFileSync(path.join(DATA, "extract_identifiers_inputs.json"), "utf8"));
	write(OUT, "extract_identifiers.json", inputs.map((s) => [s, Zotero.Utilities.extractIdentifiers(s)]));
	write(OUT, "manifest.json", {
		generatedBy: "scripts/zotero-search-reference.mjs",
		generatedOn: new Date().toISOString(),
		node: process.version,
		utcOffsetMinutes: -new Date().getTimezoneOffset(),
		translators: "vendor/translators (see the main reference manifest for commits)",
		notes: [
			"References are upstream run in-process on the committed fixtures (no network).",
			"Normalisation: item keys/parentItem renumbered (normKey); an accessDate stamped during the run becomes NOW.",
		],
	});
	process.exit(0);
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
