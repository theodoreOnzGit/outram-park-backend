//! Keep-warm rust-analyzer daemon for `kovan-cli def`/`sig`/`refs` (op-fdph,
//! GitHub issue #32's follow-up).
//!
//! # Why
//!
//! [`super::semq::run_def`]/`run_sig`/`run_refs`'s fallback path
//! (`super::semq::connect`) spawns a **fresh** `rust-analyzer` and waits out
//! its full index on *every* invocation, then shuts it down — correct for a
//! single one-shot query, wasteful across a session that asks several. This
//! module fixes that by keeping one indexed session alive in a small
//! background daemon process, so only the *first* query in a workspace pays
//! the indexing cost; every later one (from any `kovan-cli` invocation, i.e.
//! any process) answers immediately.
//!
//! # Shape
//!
//! One daemon process per workspace root, holding one
//! [`kopitiam_semantic::AsyncRustAnalyzerSession`] (the primitive
//! `kopitiam-semantic` already ships for exactly this: non-blocking readiness
//! polling, built for a long-lived host rather than a one-shot call — see its
//! own module doc). [`serve`] runs the daemon in the foreground (spawned
//! detached by [`query`]'s lazy-start path) and keeps running **until
//! explicitly stopped** (`kovan-cli lsp-daemon stop --root <root>`, wired to
//! [`stop`]) — no idle timeout, by maintainer direction (2026-08-22): once
//! warm, it stays warm for the rest of the session rather than risking a cold
//! restart mid-work. Precedent for the lazy-spawn half of this shape already
//! lives in this workspace: `kopi-beans`' own `bn daemon run`.
//!
//! Wire protocol: one [`Request`]/[`Response`] pair, as one line of JSON
//! each, per connection — a client opens a connection, writes one line,
//! reads one line, and closes. No framing beyond the newline, no
//! multiplexing: `kovan-cli` invocations are short-lived processes issuing
//! one query each, so a persistent multi-request connection buys nothing.
//!
//! # Platform scope
//!
//! Unix-domain-socket-based ([`std::os::unix::net`], no extra dependency),
//! so this only actually runs on `cfg(unix)` — which covers Linux, macOS,
//! *and* Android/Termux (Android's `target_family` is `"unix"`), matching
//! this crate's Android-clean rule with no extra gating needed. **Windows has
//! no daemon**: [`query`] unconditionally returns `None` there (checked at
//! compile time, not silently degraded), and every caller already treats
//! `None` as "fall back to `super::semq::connect`'s spawn-per-call path" —
//! so Windows keeps working, just without the warm-daemon speedup. A named
//! pipe implementation is future work, not attempted here (this workspace
//! has no Windows CI to validate it against).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// One request a client sends the daemon — the three queries `super::semq`
/// also implements directly (mirrored here so the daemon can answer them
/// from its own already-warm session), plus [`Request::Shutdown`], the only
/// way a daemon stops (see the module doc's "Shape" section).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(super) enum Request {
    Def { file: PathBuf, symbol: String },
    Sig { file: PathBuf, symbol: String },
    Refs { file: PathBuf, symbol: String },
    Shutdown,
}

/// A `file:line:character` coordinate, the wire form of `super::semq::SymPos`
/// plus which file it's in (a `Refs` response spans multiple files).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Coord {
    pub(super) file: String,
    pub(super) line: u32,
    pub(super) character: u32,
}

/// The daemon's answer to one [`Request`]. Only the fields relevant to the
/// request that produced it are `Some`; `super::semq::print_daemon_response`
/// prints whichever are present.
#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct Response {
    pub(super) ok: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) definition: Option<Coord>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) refs: Option<Vec<Coord>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) error: Option<String>,
}

impl Response {
    fn error(message: impl Into<String>) -> Self {
        Response {
            ok: false,
            error: Some(message.into()),
            ..Default::default()
        }
    }

    fn ack() -> Self {
        Response {
            ok: true,
            ..Default::default()
        }
    }
}

#[cfg(unix)]
mod unix_impl {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    use kopitiam_semantic::{AsyncRustAnalyzerSession, LspState};

    use super::{Coord, Request, Response};
    use crate::commands::semq;

    /// Where this workspace root's daemon listens — a temp-dir path keyed by
    /// a hash of the canonical root, so two different roots never collide
    /// and the same root always finds the same daemon. Hashed (rather than
    /// slugified) because a Unix socket path has a hard length limit
    /// (~100 bytes on Linux) that a deeply nested workspace path could
    /// easily exceed.
    fn socket_path(canonical_root: &Path) -> PathBuf {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        canonical_root.hash(&mut hasher);
        std::env::temp_dir().join(format!("kovan-lsp-{:016x}.sock", hasher.finish()))
    }

    /// Client side: ask the daemon for `root` to answer `req`, spawning it
    /// (detached) if it isn't already running. Returns `None` — never an
    /// error — on anything that stops this from working, so every caller can
    /// treat `None` as "fall back to the spawn-per-call path" uniformly.
    pub(in crate::commands) fn query(root: &Path, req: &Request) -> Option<Response> {
        let root = std::fs::canonicalize(root).ok()?;
        let socket_path = socket_path(&root);

        if let Ok(stream) = UnixStream::connect(&socket_path) {
            return send(stream, req);
        }

        spawn_daemon(&root).ok()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(stream) = UnixStream::connect(&socket_path) {
                return send(stream, req);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// `kovan-cli lsp-daemon stop --root <root>` — asks a running daemon to
    /// shut down. Not an error if none is running for `root`; that's the
    /// already-stopped state, reported as such rather than as a failure.
    pub fn stop(root: PathBuf) -> Result<(), String> {
        let root = std::fs::canonicalize(&root).map_err(|e| format!("resolving root: {e}"))?;
        let path = socket_path(&root);
        let Ok(stream) = UnixStream::connect(&path) else {
            println!("no lsp-daemon running for {}", root.display());
            return Ok(());
        };
        match send(stream, &Request::Shutdown) {
            Some(resp) if resp.ok => println!("stopped lsp-daemon for {}", root.display()),
            Some(resp) => {
                return Err(resp
                    .error
                    .unwrap_or_else(|| "daemon reported failure stopping".to_string()))
            }
            None => return Err("daemon did not respond to shutdown request".to_string()),
        }
        Ok(())
    }

    fn spawn_daemon(root: &Path) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        std::process::Command::new(exe)
            .arg("lsp-daemon-serve")
            .arg("--root")
            .arg(root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawning kovan-cli lsp-daemon-serve: {e}"))?;
        Ok(())
    }

    fn send(mut stream: UnixStream, req: &Request) -> Option<Response> {
        let line = serde_json::to_string(req).ok()?;
        stream.write_all(line.as_bytes()).ok()?;
        stream.write_all(b"\n").ok()?;
        stream.flush().ok()?;
        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).ok()?;
        serde_json::from_str(&response_line).ok()
    }

    /// Server side: run the daemon for `root` in the foreground until a
    /// [`Request::Shutdown`] arrives (see [`stop`]) — no idle timeout, by
    /// maintainer direction: once warm, it stays warm. Called only from
    /// `kovan-cli lsp-daemon-serve`, which [`spawn_daemon`] launches detached
    /// — never meant to be typed by a human or agent directly.
    pub fn serve(root: PathBuf) -> Result<(), String> {
        let root =
            std::fs::canonicalize(&root).map_err(|e| format!("resolving root: {e}"))?;
        let path = socket_path(&root);

        let listener = match UnixListener::bind(&path) {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                // Either another daemon for this root already won the race,
                // or this is a stale socket file from one that crashed.
                if UnixStream::connect(&path).is_ok() {
                    return Ok(()); // someone else is already serving -- done
                }
                std::fs::remove_file(&path)
                    .map_err(|e| format!("removing stale socket {}: {e}", path.display()))?;
                UnixListener::bind(&path)
                    .map_err(|e| format!("binding {}: {e}", path.display()))?
            }
            Err(e) => return Err(format!("binding {}: {e}", path.display())),
        };

        let session = AsyncRustAnalyzerSession::spawn_async_with_args(
            "rust-analyzer",
            &[],
            &root,
            semq::ra_timeout(),
        );

        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            if handle_connection(stream, &session) {
                break; // Request::Shutdown was received and acknowledged
            }
        }
        drop(session);
        std::fs::remove_file(&path).ok();
        Ok(())
    }

    /// Handles one connection: reads its one request line, dispatches it,
    /// writes the response line back. Returns `true` if the request was
    /// [`Request::Shutdown`] — the caller's signal to stop accepting and
    /// exit, sent *after* the ack so the client sees a clean reply.
    fn handle_connection(stream: UnixStream, session: &AsyncRustAnalyzerSession) -> bool {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            return false;
        }
        let (response, should_stop) = match serde_json::from_str::<Request>(line.trim()) {
            Ok(Request::Shutdown) => (Response::ack(), true),
            Ok(req) => (handle_request(session, req), false),
            Err(e) => (Response::error(format!("bad request: {e}")), false),
        };
        let out = serde_json::to_string(&response)
            .unwrap_or_else(|_| r#"{"ok":false,"error":"internal serialization error"}"#.to_string());
        let stream = reader.get_mut();
        let _ = stream.write_all(out.as_bytes());
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
        should_stop
    }

    fn handle_request(session: &AsyncRustAnalyzerSession, req: Request) -> Response {
        if let Err(e) = wait_until_ready(session, semq::ra_timeout()) {
            return Response::error(e);
        }
        match req {
            Request::Shutdown => unreachable!("handled by the caller before dispatch"),
            Request::Def { file, symbol } => match semq::locate_declaration(&file, &symbol) {
                Ok(pos) => {
                    let hover = session.hover(&file, pos.line, pos.character).ok().flatten();
                    Response {
                        ok: true,
                        signature: hover.map(|h| semq::extract_signature(&h.contents)),
                        definition: Some(Coord {
                            file: file.display().to_string(),
                            line: pos.line,
                            character: pos.character,
                        }),
                        ..Default::default()
                    }
                }
                Err(e) => Response::error(e),
            },
            Request::Sig { file, symbol } => match semq::locate_declaration(&file, &symbol) {
                Ok(pos) => {
                    let hover = session.hover(&file, pos.line, pos.character).ok().flatten();
                    Response {
                        ok: true,
                        signature: hover.map(|h| semq::extract_signature(&h.contents)),
                        ..Default::default()
                    }
                }
                Err(e) => Response::error(e),
            },
            Request::Refs { file, symbol } => match semq::locate_declaration(&file, &symbol) {
                Ok(pos) => match session.references(&file, pos.line, pos.character, false) {
                    Ok(locations) => {
                        let mut coords: Vec<Coord> = locations
                            .iter()
                            .map(|l| Coord {
                                file: l.path.display().to_string(),
                                line: l.range.start.line,
                                character: l.range.start.character,
                            })
                            .collect();
                        coords.sort_by(|a, b| {
                            (&a.file, a.line, a.character).cmp(&(&b.file, b.line, b.character))
                        });
                        Response {
                            ok: true,
                            refs: Some(coords),
                            ..Default::default()
                        }
                    }
                    Err(e) => Response::error(e.to_string()),
                },
                Err(e) => Response::error(e),
            },
        }
    }

    /// Polls `session.state()` until [`LspState::Ready`], returns the
    /// recorded error on [`LspState::Failed`], or times out — `run()`
    /// (`AsyncRustAnalyzerSession`'s internals) fast-fails with `NotReady`
    /// unless the state is already `Ready`, so a request against a session
    /// that just started must wait here first rather than surfacing that as
    /// a client-visible error on the very first query.
    fn wait_until_ready(session: &AsyncRustAnalyzerSession, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            match session.state() {
                LspState::Ready => return Ok(()),
                LspState::Failed => {
                    return Err(session
                        .error()
                        .unwrap_or_else(|| "rust-analyzer failed to start".to_string()))
                }
                LspState::Connecting => {
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "rust-analyzer did not finish indexing within {}s",
                            timeout.as_secs()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }
}

#[cfg(not(unix))]
mod unix_impl {
    use std::path::{Path, PathBuf};

    use super::{Request, Response};

    /// No daemon on non-Unix targets (see the module doc's "Platform scope")
    /// — always tells the caller to fall back to the spawn-per-call path.
    pub(in crate::commands) fn query(_root: &Path, _req: &Request) -> Option<Response> {
        None
    }

    /// Unreachable in practice: [`query`] never spawns `lsp-daemon-serve` on
    /// this platform, so nothing ever invokes this. Kept so the CLI's
    /// `LspDaemonServe` variant compiles on every target.
    pub fn serve(_root: PathBuf) -> Result<(), String> {
        Err("the kovan-cli lsp-daemon is Unix-only (Linux/macOS/Android); \
             this platform always uses the spawn-per-call path instead"
            .to_string())
    }

    /// There is never a daemon to stop on this platform.
    pub fn stop(root: PathBuf) -> Result<(), String> {
        println!(
            "no lsp-daemon on this platform (Unix-only) for {}",
            root.display()
        );
        Ok(())
    }
}

pub(super) use unix_impl::query;
pub use unix_impl::{serve, stop};
