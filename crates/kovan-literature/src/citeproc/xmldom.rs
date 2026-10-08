// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/xmldom.js (nothing ported: see the module docs)
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! `src/xmldom.js` (`CSL.XmlDOM`) is **deliberately not ported**.
//!
//! `CSL.setupXml` (src/system.js, ported in `system.rs`) builds a
//! `CSL.XmlDOM` only for an argument that has a `getAttribute` method (a DOM
//! node), and a `CSL.XmlE4X` only for one with `toXMLString`. The engine is
//! given its style as a string (`Engine::new(sys, style, lang)`); a string
//! always goes through `CSL.parseXml` and `CSL.XmlJSON` (src/xmljson.js,
//! `xmljson.rs`), whether it holds XML or serialized JSON. The same holds for
//! every locale `sys.retrieveLocale` returns, which `localeConfigure` passes
//! through `setupXml`. So `XmlDOM` (the `DOMParser` shims, `importNode`,
//! the `getElementsByTagName` walkers) is unreachable and nothing in it is
//! ported.
//!
//! `CSL.XmlJSON` and the preprocessing passes both parsers share
//! (`addMissingNameNodes`, `addInstitutionNodes`, `insertPublisherAndPlace`,
//! `flagDateMacros`) are in `xmljson.rs`.
