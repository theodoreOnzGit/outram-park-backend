//! web-kovan in eframe: in the browser (`web/build.sh`, the page at
//! `code-review/` on Pages) and natively, reading the same data folder:
//!
//! ```text
//! cargo run -p kovan-web --example web --release -- --data <dir> [--workspace .] [--open '<hash>']
//! ```
//!
//! `<dir>` is laid out as the site's `code-review/data/` (see
//! `kovan_web::data`); `--open` takes a deep link such as
//! `crate=boon-lay` or a function id.

#[cfg(target_os = "android")]
fn main() {
    eprintln!("web-kovan is a windowed app; it does not run on Android/Termux.");
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> eframe::Result<()> {
    use kovan_web::data::DataSource;
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let data = arg("--data").unwrap_or_else(|| "target/pages/code-review/data".into());
    let workspace = arg("--workspace").unwrap_or_else(|| ".".into());
    let source = DataSource::Dir { data: data.into(), workspace: workspace.into() };
    let mut app = kovan_web::ui::CodeReview::new(kovan_web::Mode::Web, source);
    eframe::run_ui_native("web-kovan", eframe::NativeOptions::default(), move |ui, _frame| app.show(ui))
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    let web_options = eframe::WebOptions { renderer: eframe::Renderer::Glow, ..Default::default() };
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("kw_canvas")
            .expect("no #kw_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#kw_canvas is not a canvas");
        let source = kovan_web::data::DataSource::Http { data: "data/".into(), api: "../api/".into() };
        let started = eframe::WebRunner::new()
            .start(canvas, web_options, Box::new(move |_cc| Ok(Box::new(App(kovan_web::ui::CodeReview::new(kovan_web::Mode::Web, source))))))
            .await;
        if let Some(el) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>Failed to start: {e:?}</p>")),
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
struct App(kovan_web::ui::CodeReview);

#[cfg(target_arch = "wasm32")]
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.show(ui);
    }
}
