//! A thread natively, inline on `wasm32` (gh:#924).
//!
//! The CIET loop runs its flow solves beside the heat transfer on threads of
//! their own (v1's design). `wasm32-unknown-unknown` has no threads, so there
//! the same closure runs where it is forked. Every closure forked in this
//! simulator reads its own start-of-step copies and writes only its own
//! result, so running it inline gives the same numbers as running it on a
//! thread; `stepper`'s tests check that bit for bit.

/// A forked computation: running on a thread, or already done.
pub enum Forked<T> {
    /// Natively: running on its own thread.
    #[cfg(not(target_arch = "wasm32"))]
    Thread(std::thread::JoinHandle<T>),
    /// On `wasm32`, or when forced inline: already run.
    Done(T),
}

/// Start `f` on a thread natively, or run it now on `wasm32`.
pub fn fork<T: Send + 'static, F: FnOnce() -> T + Send + 'static>(f: F) -> Forked<T> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !inline() {
            return Forked::Thread(std::thread::spawn(f));
        }
    }
    Forked::Done(f())
}

impl<T> Forked<T> {
    /// Its result, waiting for the thread if there is one.
    ///
    /// # Panics
    ///
    /// If the forked closure panicked (a solver bug, as `JoinHandle::join`
    /// then `unwrap` did in v1).
    pub fn join(self) -> T {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Forked::Thread(handle) => handle.join().expect("a forked CIET solve panicked"),
            Forked::Done(value) => value,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static INLINE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether forks on this thread run inline (natively, for tests that compare
/// the two orders).
#[cfg(not(target_arch = "wasm32"))]
fn inline() -> bool {
    INLINE.with(|c| c.get())
}

/// Run every fork made on this thread inline, as `wasm32` does, from now on
/// (`true`) or not (`false`). Natively only; for tests.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_inline(on: bool) {
    INLINE.with(|c| c.set(on));
}
