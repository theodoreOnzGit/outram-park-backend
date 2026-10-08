// Confirms that citeproc-js 2.4.63's CSL.PublisherOutput.render() throws
// "this._purgeEmptyBlobs is not a function" (util_publishers.js assigns
// CSL.NameOutput.prototype._join to PublisherOutput, whose body calls a method
// PublisherOutput lacks). The Rust port reproduces the throw: see the test in
// src/citeproc/util_publishers.rs. Run:
//   CITEPROC_MODULE=<repo>/target/csl-reference/node_modules/citeproc/citeproc_commonjs.js node scripts/csl-units/publishers.cjs
'use strict';
const { freshCSL, root } = require('./common.cjs');
const fs = require('fs');
const path = require('path');
const CSL = freshCSL();
const style = `<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0"><info><id>x</id><title>x</title><updated>2020-01-01T00:00:00+00:00</updated></info><citation><layout><group subgroup-delimiter="; " delimiter=", "><text variable="publisher"/><text variable="publisher-place"/></group></layout></citation></style>`;
const sys = {
  retrieveItem: () => ({ id: 'a', type: 'book', publisher: 'A; B', 'publisher-place': 'X; Y' }),
  retrieveLocale: (l) => fs.readFileSync(path.join(root, 'vendor/citeproc-js/locale', 'locales-' + l + '.xml'), 'utf8'),
};
const e = new CSL.Engine(sys, style, 'en-US');
try {
  e.updateItems(['a']);
  console.log(e.makeCitationCluster([{ id: 'a' }]));
} catch (x) {
  console.log('THROWS:', x.message);
}
