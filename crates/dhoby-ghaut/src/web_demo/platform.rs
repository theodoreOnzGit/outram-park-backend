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

/// Keep every canvas's backing store at its CSS size times
/// `devicePixelRatio`, the invariant eframe's web runner relies on (gh:#556).
///
/// eframe sizes the backing store from a `ResizeObserver`'s
/// `devicePixelContentBoxSize`, but converts it to egui points by dividing by
/// `window.devicePixelRatio` (`canvas_size_in_points`). When the browser's
/// two answers disagree, egui lays out on a screen of the wrong size: with
/// the backing store at CSS size and `devicePixelRatio = 2`, the page is
/// 195 points wide on a 390 px phone and everything is drawn twice too big,
/// with the zoom buttons over "Controls »". Measured 2026-10-05 in headless
/// Chromium (Playwright, `/opt/pw-browsers/chromium-1194`): under DPR
/// emulation (`deviceScaleFactor` 2 or 3, which Chrome's device toolbar also
/// uses) `devicePixelContentBoxSize` reports CSS pixels while
/// `devicePixelRatio` is 2 or 3; with a real `--force-device-scale-factor=2`
/// it is the other way round (780 device pixels, `devicePixelRatio` 1).
/// eframe's own web demo uses the same sizing path. On a browser whose two
/// answers agree (the expected case on a real phone, not checked here) the
/// backing store is already within a pixel of the target and this does
/// nothing.
///
/// [`Panel::show`](super::panel::Panel::show) calls it every frame, so every
/// demo on this framework gets it. A mismatch of more than 2 device pixels is
/// corrected (device-pixel snapping can differ from `round(css × dpr)` by
/// one) and a repaint is requested, so the next frame is laid out right.
/// Returns whether this frame was laid out at the wrong size (it corrected
/// the canvas, or egui has not yet seen the correction). Natively it does nothing and returns `false`.
pub fn keep_canvas_at_device_pixels(ctx: &egui::Context) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let mut fixed = false;
        use wasm_bindgen::JsCast as _;
        let Some(w) = web_sys::window() else { return false };
        let Some(d) = w.document() else { return false };
        let Ok(list) = d.query_selector_all("canvas") else { return false };
        let dpr = w.device_pixel_ratio();
        if !(dpr.is_finite() && dpr > 0.0) {
            return false;
        }
        for i in 0..list.length() {
            let Some(c) = list.item(i).and_then(|n| n.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else { continue };
            let r = c.get_bounding_client_rect();
            let (want_w, want_h) = ((r.width() * dpr).round(), (r.height() * dpr).round());
            if want_w < 1.0 || want_h < 1.0 {
                continue; // hidden
            }
            if (c.width() as f64 - want_w).abs() > 2.0 || (c.height() as f64 - want_h).abs() > 2.0 {
                c.set_width(want_w as u32);
                c.set_height(want_h as u32);
                ctx.request_repaint();
                fixed = true;
            }
            // egui's screen lags the canvas by a frame: this frame is still
            // wrong until its width in device pixels matches too.
            let laid_out = ctx.content_rect().width() as f64 * ctx.pixels_per_point() as f64;
            if (laid_out - want_w).abs() > 2.0 {
                ctx.request_repaint();
                fixed = true;
            }
        }
        fixed
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ctx;
        false
    }
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
