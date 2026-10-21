//! The network behind identifier lookup (GitHub #756): the ONE place kovan
//! sends the requests `kovan_literature::zotero::search` asks for.
//!
//! **Network is opt-in.** Nothing here runs unless the user asks for a
//! lookup (`kovan-cli zotero lookup`, `kovan-cli lit import --lookup`).
//! The library stays network-free and wasm32-clean: it hands this module an
//! [`HttpRequest`] and gets back an [`HttpResponse`] or a [`FetchError`].
//!
//! ```text
//! LookupSession::run_with(|req| backend.fetch(req))
//!     HttpBackend::Native  -- ureq (rustls, no OpenSSL; connect 10 s,
//!                             read/write 30 s, 10 redirects, 10 MB cap)
//!     HttpBackend::Disabled -- every request -> FetchError::Unsupported
//!                             (a build without the `lookup-net` feature)
//! ```
//!
//! A browser (web-kovan, wasm32) does not use this module: it drives
//! `LookupSession::step` / `answer` with `fetch()` itself (follow-up issue).
//!
//! Every failure is a value; nothing here panics or waits without a timeout.

use kovan_literature::zotero::search::{FetchError, HttpRequest, HttpResponse};

/// The `User-Agent` kovan sends: it names the tool, as DATA_POLICY.md and
/// the services' guidelines ask. With a user-supplied email (never a
/// hard-coded one) it carries `mailto:` for Crossref's polite pool.
pub fn user_agent(mailto: Option<&str>) -> String {
    let mut ua = format!(
        "kovan/{} (Zotero identifier-lookup port; +https://github.com/theodoreOnzGit/outram-park-backend",
        env!("CARGO_PKG_VERSION")
    );
    if let Some(m) = mailto.filter(|m| !m.trim().is_empty()) {
        ua.push_str("; mailto:");
        ua.push_str(m.trim());
    }
    ua.push(')');
    ua
}

/// Largest response accepted (the translation-server's
/// `httpMaxResponseSize` is 10 MB).
pub const MAX_RESPONSE_BYTES: u64 = 10 * 1024 * 1024;

/// How requests are sent. An enum, not a trait object: one variant per
/// backend.
pub enum HttpBackend {
    /// Native HTTP(S) through `ureq` with rustls.
    #[cfg(all(feature = "lookup-net", not(target_os = "android")))]
    Native(NativeHttp),
    /// No network backend in this build: every request fails with
    /// [`FetchError::Unsupported`] carrying this reason.
    Disabled(String),
}

impl HttpBackend {
    /// The backend this build has: native when the `lookup-net` feature is
    /// on, else disabled.
    pub fn for_this_build(mailto: Option<&str>) -> HttpBackend {
        #[cfg(all(feature = "lookup-net", not(target_os = "android")))]
        {
            HttpBackend::Native(NativeHttp::new(&user_agent(mailto)))
        }
        #[cfg(not(all(feature = "lookup-net", not(target_os = "android"))))]
        {
            let _ = mailto;
            HttpBackend::Disabled(if cfg!(target_os = "android") {
                "network lookup is not built for Android yet (rustls's ring needs the NDK C compiler)"
                    .to_owned()
            } else {
                "this kovan was built without the `lookup-net` feature".to_owned()
            })
        }
    }

    /// Send `req`.
    pub fn fetch(&self, req: &HttpRequest) -> Result<HttpResponse, FetchError> {
        match self {
            #[cfg(all(feature = "lookup-net", not(target_os = "android")))]
            HttpBackend::Native(n) => n.fetch(req),
            HttpBackend::Disabled(why) => {
                let _ = req;
                Err(FetchError::Unsupported(why.clone()))
            }
        }
    }
}

/// Native HTTP(S).
#[cfg(all(feature = "lookup-net", not(target_os = "android")))]
pub struct NativeHttp {
    agent: ureq::Agent,
    user_agent: String,
}

#[cfg(all(feature = "lookup-net", not(target_os = "android")))]
impl NativeHttp {
    /// An agent with connect/read/write timeouts and a redirect limit.
    pub fn new(user_agent: &str) -> NativeHttp {
        use std::time::Duration;
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .redirects(10)
            .build();
        NativeHttp {
            agent,
            user_agent: user_agent.to_owned(),
        }
    }

    /// Send `req`; a non-2xx status is a response, not an error (the
    /// library applies Zotero's success-code rules).
    pub fn fetch(&self, req: &HttpRequest) -> Result<HttpResponse, FetchError> {
        let mut r = self
            .agent
            .request(&req.method, &req.url)
            .set("User-Agent", &self.user_agent);
        for (k, v) in &req.headers {
            r = r.set(k, v);
        }
        let result = match &req.body {
            Some(b) => r.send_string(b),
            None => r.call(),
        };
        let resp = match result {
            Ok(resp) => resp,
            Err(ureq::Error::Status(_, resp)) => resp,
            Err(ureq::Error::Transport(t)) => return Err(transport_error(&t)),
        };
        let status = resp.status();
        let url = resp.get_url().to_owned();
        let content_type = resp.header("content-type").map(str::to_owned);
        let retry_after = resp.header("retry-after").map(str::to_owned);
        let mut body = Vec::new();
        use std::io::Read;
        resp.into_reader()
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|e| io_error(&e))?;
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(FetchError::TooLarge);
        }
        Ok(HttpResponse {
            status,
            url,
            content_type,
            retry_after,
            body,
        })
    }
}

#[cfg(all(feature = "lookup-net", not(target_os = "android")))]
fn io_error(e: &std::io::Error) -> FetchError {
    use std::io::ErrorKind as K;
    match e.kind() {
        K::TimedOut | K::WouldBlock => FetchError::Timeout,
        K::NetworkUnreachable | K::NetworkDown | K::HostUnreachable => FetchError::Offline,
        K::ConnectionRefused | K::ConnectionReset | K::ConnectionAborted | K::NotConnected => {
            FetchError::Connect(e.to_string())
        }
        _ => FetchError::Other(e.to_string()),
    }
}

#[cfg(all(feature = "lookup-net", not(target_os = "android")))]
fn transport_error(t: &ureq::Transport) -> FetchError {
    use std::error::Error;
    // An io::Error underneath tells timeouts and unreachable networks apart.
    let mut src: Option<&(dyn Error + 'static)> = t.source();
    while let Some(e) = src {
        if let Some(io) = e.downcast_ref::<std::io::Error>() {
            return io_error(io);
        }
        src = e.source();
    }
    match t.kind() {
        ureq::ErrorKind::Dns => FetchError::Connect(format!("DNS: {t}")),
        ureq::ErrorKind::ConnectionFailed => FetchError::Connect(t.to_string()),
        _ => FetchError::Other(t.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_names_the_tool_and_only_a_given_email() {
        let ua = user_agent(None);
        assert!(ua.starts_with("kovan/"));
        assert!(!ua.contains("mailto"));
        assert!(user_agent(Some("a@b.org")).contains("mailto:a@b.org"));
    }

    #[test]
    fn disabled_backend_is_a_typed_error() {
        let b = HttpBackend::Disabled("no network here".into());
        let req = HttpRequest {
            method: "GET".into(),
            url: "https://example.org/".into(),
            headers: vec![],
            body: None,
        };
        assert_eq!(
            b.fetch(&req),
            Err(FetchError::Unsupported("no network here".into()))
        );
    }

    /// Nothing listening on a local port: a connect error, quickly, not a
    /// hang or a panic. (Loopback only; no network.)
    #[cfg(all(feature = "lookup-net", not(target_os = "android")))]
    #[test]
    fn refused_connection_is_a_typed_error() {
        let n = NativeHttp::new(&user_agent(None));
        let req = HttpRequest {
            method: "GET".into(),
            url: "http://127.0.0.1:9/".into(),
            headers: vec![],
            body: None,
        };
        assert!(matches!(
            n.fetch(&req),
            Err(FetchError::Connect(_)) | Err(FetchError::Other(_)) | Err(FetchError::Offline)
        ));
    }
}
