// csl-verify-citeproc-build.cjs: check that vendor/citeproc-js (juris-m/citeproc-js
// at 73bc1b44, the commit that bumped the package to 2.4.63) builds the same
// engine as npm citeproc 2.4.63 (GitHub #791).
//
// Method: concatenate src/<name>.js in the order of csl-citeproc-sources.cjs
// (citeproc-test-runner's Bundle(): each file + "\n"), prefix the licence block
// ("/*\n" + LICENSE trimmed + "\n*/\n") and append "\nmodule.exports = CSL", as
// citeproc-test-runner's Bundle() does, then compare byte for byte with
//   (1) the citeproc_commonjs.js committed in vendor/citeproc-js, and
//   (2) npm citeproc 2.4.63's citeproc_commonjs.js ($CITEPROC_MODULE).
// Exits non-zero unless all three are identical.
'use strict';
const fs = require('fs');
const path = require('path');
const sources = require('./csl-citeproc-sources.cjs');

const root = path.join(__dirname, '..', 'vendor', 'citeproc-js');
let body = '';
for (const name of sources) {
  const p = path.join(root, 'src', name + '.js');
  if (!fs.existsSync(p)) {
    console.error('csl-verify-citeproc-build: missing src/' + name + '.js');
    process.exit(1);
  }
  body += fs.readFileSync(p).toString() + '\n';
}
const licence = '/*\n' + fs.readFileSync(path.join(root, 'LICENSE')).toString().trim() + '\n*/\n';
const built = Buffer.from(licence + body + '\nmodule.exports = CSL');
const committed = fs.readFileSync(path.join(root, 'citeproc_commonjs.js'));
const npm = fs.readFileSync(process.env.CITEPROC_MODULE);
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const ok = built.equals(committed) && committed.equals(npm) && version === '2.4.63';
console.log(
  'csl-verify-citeproc-build: package.json ' + version + '; src build ' + built.length + ' bytes; ' +
    'equals committed bundle: ' + built.equals(committed) + '; equals npm bundle: ' + built.equals(npm)
);
process.exit(ok ? 0 : 1);
