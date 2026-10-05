//! Physics off the UI thread (the no-lagging HARD RULE): a background thread
//! natively, a Web Worker in the browser, and one [`Link`] for the UI to talk
//! to either.
//!
//! The UI sends requests and drains events; it never waits. Natively, the
//! engine thread and the UI share a [`Mailbox`] in an `Arc<RwLock<_>>` (the
//! workspace's shared-state rule), held only to push or take messages; the UI
//! takes with `try_write`, so a frame never blocks on the engine. In the
//! browser, the page starts a module worker on the SAME wasm module; finding
//! no `window` there, the module's `main` calls [`worker_main`], and messages
//! cross as JS objects ([`Message`]).
//!
//! **The hello handshake.** A module worker sets its `onmessage` only after
//! the wasm has loaded, and the browser drops a message dispatched before
//! that. So the page queues what it sends until the worker posts
//! `{kind: "hello"}` ([`worker_main`] does, once it is listening), then sends
//! the queue and goes direct. (Without this, the Monte Carlo demo's first
//! "load the data" request was lost and it sat on "Downloading" forever.)
//!
//! **Keep each request short.** A worker handles one message at a time, so a
//! request that runs for minutes cannot be paused. Split long work into steps
//! the UI asks for one by one (a generation, a batch of histories), or yield
//! with async between steps (as the Monte Carlo demo's data loading does
//! between nuclides).

use std::sync::{Arc, RwLock};

/// What crosses between the UI and the engine. Natively anything `Send + Sync` is a
/// message; in the browser it must also turn into a JS object and back.
#[cfg(not(target_arch = "wasm32"))]
pub trait Message: Send + Sync + 'static {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + 'static> Message for T {}

/// What crosses between the page and the worker: a JS object each way. Use
/// [`js`] to build and read them; a `kind` string field is the convention.
#[cfg(target_arch = "wasm32")]
pub trait Message: Sized + 'static {
    fn to_js(&self) -> wasm_bindgen::JsValue;
    fn from_js(v: &wasm_bindgen::JsValue) -> Result<Self, String>;
}

/// The engine thread's and the UI's shared mail.
#[cfg(not(target_arch = "wasm32"))]
pub struct Mailbox<R, E> {
    pub events: Vec<E>,
    pub requests: Vec<R>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<R, E> Default for Mailbox<R, E> {
    fn default() -> Self {
        Self { events: Vec::new(), requests: Vec::new() }
    }
}

/// The UI's handle on the engine.
pub enum Link<R, E> {
    #[cfg(not(target_arch = "wasm32"))]
    Native(Arc<RwLock<Mailbox<R, E>>>),
    #[cfg(target_arch = "wasm32")]
    Web {
        worker: web_sys::Worker,
        inbox: Arc<RwLock<Vec<E>>>,
        /// Requests held until the worker says hello; `None` once it has.
        outbox: Arc<RwLock<Option<Vec<wasm_bindgen::JsValue>>>>,
        _r: std::marker::PhantomData<R>,
    },
}

impl<R: Message, E: Message> Link<R, E> {
    /// Send a request. Never blocks for long (natively it takes the mailbox
    /// lock only to push).
    pub fn send(&self, r: R) {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Link::Native(m) => {
                if let Ok(mut m) = m.write() {
                    m.requests.push(r);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Link::Web { worker, outbox, .. } => {
                let msg = r.to_js();
                if let Ok(mut o) = outbox.write() {
                    if let Some(queue) = o.as_mut() {
                        queue.push(msg);
                        return;
                    }
                }
                let _ = worker.post_message(&msg);
            }
        }
    }

    /// Every event that has arrived since the last call. Never blocks: if the
    /// engine thread is posting at this instant, the events wait a frame.
    pub fn drain(&self) -> Vec<E> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Link::Native(m) => m.try_write().map(|mut m| std::mem::take(&mut m.events)).unwrap_or_default(),
            #[cfg(target_arch = "wasm32")]
            Link::Web { inbox, .. } => inbox.write().map(|mut i| std::mem::take(&mut *i)).unwrap_or_default(),
        }
    }
}

// ─── Native: an engine thread ────────────────────────────────────────────────

/// The engine as a native thread runs it: one request at a time, in order,
/// posting events as it goes. `post` takes the mailbox lock only to push.
#[cfg(not(target_arch = "wasm32"))]
pub trait NativeEngine: Send + 'static {
    type Req: Message;
    type Ev: Message;
    fn handle(&mut self, req: Self::Req, post: &mut impl FnMut(Self::Ev));
}

/// Start `engine` on its own thread. `repaint` is called after every event
/// (pass `move || ctx.request_repaint()`).
#[cfg(not(target_arch = "wasm32"))]
pub fn start_native<N: NativeEngine>(mut engine: N, repaint: impl Fn() + Send + 'static) -> Link<N::Req, N::Ev> {
    let mailbox: Arc<RwLock<Mailbox<N::Req, N::Ev>>> = Arc::default();
    let m = mailbox.clone();
    std::thread::spawn(move || {
        let mut post = |e: N::Ev| {
            if let Ok(mut m) = m.write() {
                m.events.push(e);
            }
            repaint();
        };
        loop {
            let requests = m.write().map(|mut m| std::mem::take(&mut m.requests)).unwrap_or_default();
            if requests.is_empty() {
                std::thread::sleep(std::time::Duration::from_millis(3));
                continue;
            }
            for r in requests {
                engine.handle(r, &mut post);
            }
        }
    });
    Link::Native(mailbox)
}

#[cfg(not(target_arch = "wasm32"))]
impl<R, E> Default for Link<R, E> {
    /// A link to nothing (no engine thread): sends go nowhere, nothing
    /// arrives. For tests and for a failed start.
    fn default() -> Self {
        Link::Native(Arc::default())
    }
}

// ─── Browser: a Web Worker running the same module ───────────────────────────

#[cfg(target_arch = "wasm32")]
pub use web::{fetch_bytes, start_web, worker_main, Poster, WorkerEngine};

/// Building and reading the JS objects messages travel as.
#[cfg(target_arch = "wasm32")]
pub mod js {
    use js_sys::{Float64Array, Object, Reflect};
    use wasm_bindgen::{JsCast as _, JsValue};

    pub fn object() -> Object {
        Object::new()
    }
    pub fn set(o: &Object, k: &str, v: impl Into<JsValue>) {
        let _ = Reflect::set(o, &k.into(), &v.into());
    }
    pub fn get_f64(o: &JsValue, k: &str) -> Option<f64> {
        Reflect::get(o, &k.into()).ok().and_then(|v| v.as_f64())
    }
    pub fn get_bool(o: &JsValue, k: &str) -> Option<bool> {
        Reflect::get(o, &k.into()).ok().and_then(|v| v.as_bool())
    }
    pub fn get_str(o: &JsValue, k: &str) -> String {
        Reflect::get(o, &k.into()).ok().and_then(|v| v.as_string()).unwrap_or_default()
    }
    /// A `Float64Array` field as a `Vec<f64>` (bulk numbers cross this way).
    pub fn get_f64s(o: &JsValue, k: &str) -> Vec<f64> {
        Reflect::get(o, &k.into())
            .ok()
            .and_then(|d| d.dyn_into::<Float64Array>().ok())
            .map(|a| a.to_vec())
            .unwrap_or_default()
    }
    pub fn f64s(v: &[f64]) -> JsValue {
        Float64Array::from(v).into()
    }
    /// Bytes as a `Uint8Array` (an image or a material map: 8 times smaller
    /// than the same numbers as `f64`s).
    pub fn u8s(v: &[u8]) -> JsValue {
        js_sys::Uint8Array::from(v).into()
    }
    /// A `Uint8Array` field as a `Vec<u8>`.
    pub fn get_u8s(o: &JsValue, k: &str) -> Vec<u8> {
        Reflect::get(o, &k.into())
            .ok()
            .and_then(|d| d.dyn_into::<js_sys::Uint8Array>().ok())
            .map(|a| a.to_vec())
            .unwrap_or_default()
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::{js, Link, Message};
    use std::sync::{Arc, RwLock};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;

    /// Page side: start the module worker at `worker_js` (e.g.
    /// `"./worker.js"`, which only imports and inits the same module) and
    /// route its messages into the inbox. `on_error` turns a worker error
    /// into one of your events.
    pub fn start_web<R: Message, E: Message>(
        ctx: egui::Context,
        worker_js: &str,
        on_error: fn(String) -> E,
    ) -> Result<Link<R, E>, String> {
        let opts = web_sys::WorkerOptions::new();
        opts.set_type(web_sys::WorkerType::Module);
        let worker = web_sys::Worker::new_with_options(worker_js, &opts).map_err(|e| format!("{e:?}"))?;
        let inbox: Arc<RwLock<Vec<E>>> = Arc::default();
        let outbox: Arc<RwLock<Option<Vec<JsValue>>>> = Arc::new(RwLock::new(Some(Vec::new())));
        let (ib, ob, w, c) = (inbox.clone(), outbox.clone(), worker.clone(), ctx.clone());
        // `Closure<dyn FnMut>` is wasm-bindgen's only callback type: a framework
        // boundary, not a design choice (the workspace otherwise avoids `dyn`).
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
            let data = ev.data();
            if js::get_str(&data, "kind") == "hello" {
                let queued = ob.write().ok().and_then(|mut o| o.take()).unwrap_or_default();
                for m in queued {
                    let _ = w.post_message(&m);
                }
                return;
            }
            let e = E::from_js(&data).unwrap_or_else(|m| on_error(format!("bad message from the worker: {m}")));
            if let Ok(mut i) = ib.write() {
                i.push(e);
            }
            c.request_repaint();
        });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        let (ib, c) = (inbox.clone(), ctx);
        let on_err = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |ev: web_sys::ErrorEvent| {
            if let Ok(mut i) = ib.write() {
                i.push(on_error(format!("physics worker: {}", ev.message())));
            }
            c.request_repaint();
        });
        worker.set_onerror(Some(on_err.as_ref().unchecked_ref()));
        on_err.forget();
        Ok(Link::Web { worker, inbox, outbox, _r: std::marker::PhantomData })
    }

    fn scope() -> web_sys::DedicatedWorkerGlobalScope {
        js_sys::global().unchecked_into()
    }

    /// Posts events from the worker to the page. `Copy`: hand it to async
    /// tasks freely.
    pub struct Poster<E>(std::marker::PhantomData<fn(E)>);
    impl<E> Clone for Poster<E> {
        fn clone(&self) -> Self {
            *self
        }
    }
    impl<E> Copy for Poster<E> {}
    impl<E: Message> Poster<E> {
        pub fn post(&self, e: E) {
            let _ = scope().post_message(&e.to_js());
        }
    }

    /// The engine as the browser worker runs it. `handle` gets the shared
    /// state and returns at once; long work (downloads) goes into
    /// `wasm_bindgen_futures::spawn_local` tasks that post as they go.
    pub trait WorkerEngine: Default + 'static {
        type Req: Message;
        type Ev: Message;
        fn handle(state: &Arc<RwLock<Self>>, req: Self::Req, post: Poster<Self::Ev>);
        /// Turn a panic or a bad message into an event the page shows.
        fn error(message: String) -> Self::Ev;
    }

    /// Worker side: call from `main` when the module finds itself in a
    /// worker (`js_sys::global().dyn_ref::<web_sys::DedicatedWorkerGlobalScope>()`).
    /// Installs a panic hook that reports to the page, listens, and says hello.
    pub fn worker_main<W: WorkerEngine>() {
        std::panic::set_hook(Box::new(|info| {
            let m = info.to_string();
            web_sys::console::error_1(&m.as_str().into());
            Poster::<W::Ev>(std::marker::PhantomData).post(W::error(format!("physics worker panicked: {m}")));
        }));
        let state: Arc<RwLock<W>> = Arc::default();
        let post = Poster::<W::Ev>(std::marker::PhantomData);
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
            match W::Req::from_js(&ev.data()) {
                Ok(r) => W::handle(&state, r, post),
                Err(e) => post.post(W::error(e)),
            }
        });
        scope().set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        let hello = js::object();
        js::set(&hello, "kind", "hello");
        let _ = scope().post_message(&hello);
    }

    /// Fetch a URL relative to the worker (or page) as bytes.
    pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
        let p = match js_sys::global().dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
            Some(w) => w.fetch_with_str(url),
            None => web_sys::window().ok_or("no window")?.fetch_with_str(url),
        };
        fetch_promise(p, url).await
    }

    /// The bytes of an already-issued `fetch` promise (issue every request
    /// first, then await each, so downloads overlap the processing).
    pub async fn fetch_promise(p: js_sys::Promise, url: &str) -> Result<Vec<u8>, String> {
        let err = |e: JsValue| format!("{url}: {e:?}");
        let resp: web_sys::Response = JsFuture::from(p).await.map_err(err)?.dyn_into().map_err(err)?;
        if !resp.ok() {
            return Err(format!("{url}: HTTP {}", resp.status()));
        }
        let buf = JsFuture::from(resp.array_buffer().map_err(err)?).await.map_err(err)?;
        Ok(js_sys::Uint8Array::new(&buf).to_vec())
    }

    /// Issue a fetch from the worker without awaiting it.
    pub fn fetch_start(url: &str) -> js_sys::Promise {
        scope().fetch_with_str(url)
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::{fetch_promise, fetch_start};
