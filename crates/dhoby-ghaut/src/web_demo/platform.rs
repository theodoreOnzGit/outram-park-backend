//! What differs between the browser and a native window: the clock, the page
//! title, the URL query.
//!
//! `std::time::Instant` compiles on `wasm32-unknown-unknown` and then panics
//! at run time, so the clock goes through here.

/// Seconds since an arbitrary start, on both targets.
pub fn now_s() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() / 1000.0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        T0.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
    }
}

/// The page title in the browser, the window title natively. Cheap to call
/// every frame only if the caller skips unchanged titles.
pub fn set_title(ctx: &egui::Context, title: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        if let Some(d) = web_sys::window().and_then(|w| w.document()) {
            d.set_title(title);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.to_owned()));
}

/// The page's URL query as `(key, value)` pairs (`?rung=godiva&mode=run`);
/// natively, the command line's `--key value` pairs, its twin. A bare key
/// (`?autostart`) has an empty value.
pub fn query_pairs() -> Vec<(String, String)> {
    #[cfg(target_arch = "wasm32")]
    {
        let search = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
        search
            .trim_start_matches('?')
            .split('&')
            .filter(|kv| !kv.is_empty())
            .map(|kv| match kv.split_once('=') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (kv.to_string(), String::new()),
            })
            .collect()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < args.len() {
            if let Some(k) = args[i].strip_prefix("--") {
                match args.get(i + 1).filter(|v| !v.starts_with("--")) {
                    Some(v) => {
                        out.push((k.to_string(), v.clone()));
                        i += 1;
                    }
                    None => out.push((k.to_string(), String::new())),
                }
            }
            i += 1;
        }
        out
    }
}

/// The value of `key` in [`query_pairs`]-style pairs.
pub fn query_value<'a>(query: &'a [(String, String)], key: &str) -> Option<&'a str> {
    query.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Replace the page's URL query (no reload, no history entry), so a shared
/// link opens what the reader is looking at. Does nothing natively.
pub fn set_query(pairs: &[(&str, &str)]) {
    #[cfg(target_arch = "wasm32")]
    if let Some(h) = web_sys::window().and_then(|w| w.history().ok()) {
        let q: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let _ = h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&format!("?{}", q.join("&"))));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = pairs;
}

/// `?autostart` in the page URL (natively, `--autostart` or the
/// `DEMO_AUTOSTART` environment variable): start animating or computing as
/// soon as the data are ready, for unattended browser checks. Read it ONCE at
/// start-up: [`set_query`] rewrites the query later.
pub fn autostart() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("DEMO_AUTOSTART").is_some() {
        return true;
    }
    query_value(&query_pairs(), "autostart").is_some()
}
