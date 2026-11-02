//! **Files the user supplies, and files the page gives back** (GitHub #863):
//! a file picker and a download, for demos whose input is the reader's own
//! data (an ENDF tape, an ACE file) rather than data the page ships.
//!
//! - In the browser, [`pick_file`] opens the browser's own file chooser (an
//!   `<input type="file">`) and reads the chosen file into memory; nothing is
//!   uploaded anywhere, the bytes stay in the tab. [`save_file`] hands bytes to
//!   the browser as a download.
//! - Natively a demo uses `egui-file-dialog` (the picker the desktop apps
//!   use), reads the path with [`FileSlot::fill_from_path`], and writes with
//!   [`save_file`] to the path the dialog chose.
//!
//! The picker is asynchronous in the browser, so the result lands in a
//! [`FileSlot`] the UI polls with [`FileSlot::take`] each frame.
//!
//! Browser glue: the file chooser and the download are browser APIs with no
//! headless test; [`FileSlot`]'s own logic is tested natively.

use std::sync::{Arc, RwLock};

/// A file the user chose: its name and its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickedFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Where a picked file (or the reason picking failed) waits for the UI.
#[derive(Clone, Default)]
pub struct FileSlot(Arc<RwLock<Option<Result<PickedFile, String>>>>);

impl FileSlot {
    /// The picked file, once, when it has arrived.
    pub fn take(&self) -> Option<Result<PickedFile, String>> {
        self.0.write().ok().and_then(|mut g| g.take())
    }

    fn put(&self, r: Result<PickedFile, String>) {
        if let Ok(mut g) = self.0.write() {
            *g = Some(r);
        }
    }

    /// Read the file at `path` into the slot (native).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn fill_from_path(&self, path: &std::path::Path) {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        self.put(std::fs::read(path).map(|bytes| PickedFile { name, bytes }).map_err(|e| format!("{}: {e}", path.display())));
    }
}

/// Open the browser's file chooser; the chosen file arrives in `slot`.
/// Choosing nothing leaves the slot empty.
#[cfg(target_arch = "wasm32")]
pub fn pick_file(slot: &FileSlot) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast as _;
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        slot.put(Err("no document to open a file chooser in".into()));
        return;
    };
    let input: web_sys::HtmlInputElement = match doc.create_element("input").map(|e| e.dyn_into()) {
        Ok(Ok(i)) => i,
        _ => {
            slot.put(Err("could not create a file chooser".into()));
            return;
        }
    };
    input.set_type("file");
    let slot = slot.clone();
    let inp = input.clone();
    let on_change = Closure::once(move || {
        let Some(file) = inp.files().and_then(|l| l.get(0)) else { return };
        let name = file.name();
        wasm_bindgen_futures::spawn_local(async move {
            let r = match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                Ok(buf) => Ok(PickedFile { name, bytes: js_sys::Uint8Array::new(&buf).to_vec() }),
                Err(e) => Err(format!("{name}: could not be read ({e:?})")),
            };
            slot.put(r);
        });
    });
    input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
    on_change.forget();
    input.click();
}

/// Give `bytes` to the browser as a download named `name`.
#[cfg(target_arch = "wasm32")]
pub fn save_file(name: &str, bytes: &[u8]) -> Result<(), String> {
    use wasm_bindgen::JsCast as _;
    let doc = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/octet-stream");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(|e| format!("{e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("{e:?}"))?;
    let a: web_sys::HtmlAnchorElement = doc.create_element("a").map_err(|e| format!("{e:?}"))?.dyn_into().map_err(|_| "not an anchor")?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

/// Write `bytes` to `path` (native: the path the save dialog chose).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// A file read into a slot is taken once; a missing file arrives as an
    /// error naming it; saving writes the bytes back.
    #[test]
    fn a_slot_hands_over_its_file_once_and_errors_name_the_file() {
        let dir = std::env::temp_dir().join(format!("dhoby-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.ace");
        save_file(&p, b"hello").unwrap();
        let s = FileSlot::default();
        assert!(s.take().is_none());
        s.fill_from_path(&p);
        assert_eq!(s.take(), Some(Ok(PickedFile { name: "a.ace".into(), bytes: b"hello".to_vec() })));
        assert!(s.take().is_none(), "taken once");
        s.fill_from_path(&dir.join("missing.ace"));
        assert!(matches!(s.take(), Some(Err(e)) if e.contains("missing.ace")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
