// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): CFF References.js (translatorID
//   99A6641F-A8C2-4923-9BBB-0DA87F1E5187, lastUpdated 2024-05-17 20:02:13).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The CFF References translator: export. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "99A6641F-A8C2-4923-9BBB-0DA87F1E5187",
    label: "CFF References",
    creator: "Sebastian Karcher, Dave Bunten",
    target: "cff",
    min_version: "5.0",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-05-17 20:02:13",
};

/// `doExport`. STUB.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
