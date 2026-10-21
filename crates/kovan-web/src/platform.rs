//! What differs between the browser and a native window: the clock, the
//! URL hash (deep links) and opening a new tab.
//!
//! `now_ms` follows `dhoby_ghaut::web_demo::platform::now_s`
//! (`std::time::Instant` compiles on wasm32 and panics at run time). Not
//! reused from there: dhoby-ghaut depends on `kovan`, and desktop kovan will
//! embed this crate, so a dependency on dhoby-ghaut would be a cycle.

/// Milliseconds since an arbitrary start, on both targets.
pub fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        T0.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() * 1e3
    }
}

/// The page's `#…` hash (`""` when none). Natively, the `--open <hash>`
/// argument, read once.
pub fn hash() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().and_then(|w| w.location().hash().ok()).unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static ARG: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        ARG.get_or_init(|| {
            let a: Vec<String> = std::env::args().collect();
            a.iter().position(|x| x == "--open").and_then(|i| a.get(i + 1)).cloned().unwrap_or_default()
        })
        .clone()
    }
}

/// Replace the page's hash without a reload or a new history entry, so the
/// address bar is always a link to what is on screen. Natively nothing.
pub fn set_hash(hash: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(w) = web_sys::window() {
        let base = w.location().pathname().unwrap_or_default() + &w.location().search().unwrap_or_default();
        if let Ok(h) = w.history() {
            let _ = h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&format!("{base}{hash}")));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = hash;
}

/// The page's URL with `hash` in place of its own: the pop-out link.
pub fn url_with_hash(hash: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(w) = web_sys::window() else { return hash.to_string() };
        let l = w.location();
        format!(
            "{}{}{}{hash}",
            l.origin().unwrap_or_default(),
            l.pathname().unwrap_or_default(),
            l.search().unwrap_or_default()
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        hash.to_string()
    }
}
