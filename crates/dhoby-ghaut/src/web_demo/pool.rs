//! **A pool of engines**: several Web Workers in the browser (or threads
//! natively) for work that splits, sized to the device (gh:#786).
//!
//! GitHub Pages serves no COOP/COEP headers, so a page there cannot use
//! `SharedArrayBuffer` or wasm threads: every worker is a separate wasm
//! instance with **its own copy of the data**, and they share only messages.
//! So a pool costs memory per worker, and how many to start is a decision:
//!
//! - **cores:** one worker per logical core the browser reports
//!   (`navigator.hardwareConcurrency`), less one for the page itself;
//! - **memory:** at most as many as fit in a budget, at the per-worker memory
//!   the caller **measured** (`wasm_memory_mb` reports it from inside a
//!   worker): half the device memory where the browser reports it
//!   (`navigator.deviceMemory`, Chromium only, capped at 8 GB by the browser
//!   itself), a third on a phone, and 2 GB (1 GB on a phone) where it does
//!   not;
//! - **phones get fewer:** at most 2 workers on a narrow screen;
//! - a cap of 8, and an explicit override (`?workers=N`) that bypasses all of
//!   it, for measurements.
//!
//! [`plan`] is that rule, a pure function, pinned by its tests; [`Device::probe`]
//! reads the device. Starting the workers is the caller's (each is an
//! ordinary [`super::link::Link`]).

/// What the device reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Device {
    /// Logical cores (`navigator.hardwareConcurrency`; natively
    /// `available_parallelism`).
    pub logical_cores: usize,
    /// Device memory in GB, where known (`navigator.deviceMemory`, which only
    /// Chromium reports, rounded and capped at 8 by the browser).
    pub memory_gb: Option<f64>,
    /// A phone-sized screen (narrower than 700 CSS px, the panel's own fold
    /// width).
    pub phone: bool,
}

impl Device {
    /// Read the device. In the browser: the navigator and the window width;
    /// natively: the core count, unknown memory, not a phone.
    pub fn probe() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            use js_sys::Reflect;
            let g = js_sys::global();
            let nav = Reflect::get(&g, &"navigator".into()).ok();
            let num = |k: &str| {
                nav.as_ref()
                    .and_then(|n| Reflect::get(n, &k.into()).ok())
                    .and_then(|v| v.as_f64())
            };
            let width = Reflect::get(&g, &"innerWidth".into())
                .ok()
                .and_then(|v| v.as_f64());
            Self {
                logical_cores: num("hardwareConcurrency").map_or(2, |c| c.max(1.0) as usize),
                memory_gb: num("deviceMemory"),
                phone: width.is_some_and(|w| w < 700.0),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self {
                logical_cores: std::thread::available_parallelism().map_or(2, |n| n.get()),
                memory_gb: None,
                phone: false,
            }
        }
    }
}

/// How many workers to start, and why (for the panel).
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub workers: usize,
    pub reason: String,
}

/// Most workers any device gets.
pub const MAX_WORKERS: usize = 8;
/// Most workers a phone gets.
pub const MAX_PHONE_WORKERS: usize = 2;

/// The sizing rule (module docs). `per_worker_mb` is the memory one worker
/// needs once its data are loaded, as measured; `forced` is an explicit
/// count (`?workers=N`), clamped to 1..=16.
pub fn plan(device: Device, per_worker_mb: f64, forced: Option<usize>) -> Plan {
    if let Some(n) = forced {
        let n = n.clamp(1, 16);
        return Plan {
            workers: n,
            reason: format!("{n} asked for in the URL (?workers=)"),
        };
    }
    let by_cores = device.logical_cores.saturating_sub(1).max(1);
    let budget_mb = match device.memory_gb {
        Some(gb) => gb * 1024.0 * if device.phone { 1.0 / 3.0 } else { 0.5 },
        None if device.phone => 1024.0,
        None => 2048.0,
    };
    let by_memory = ((budget_mb / per_worker_mb.max(1.0)).floor() as usize).max(1);
    let cap = if device.phone {
        MAX_PHONE_WORKERS
    } else {
        MAX_WORKERS
    };
    let workers = by_cores.min(by_memory).min(cap);
    let limit = if workers == cap && cap < by_cores.min(by_memory) {
        if device.phone {
            "phone-sized screen"
        } else {
            "the cap"
        }
    } else if by_memory < by_cores {
        "memory"
    } else {
        "cores"
    };
    Plan {
        workers,
        reason: format!(
            "{workers}: {} logical cores less one for the page; {:.0} MB memory budget at {:.0} MB per worker{}; limited by {limit}",
            device.logical_cores,
            budget_mb,
            per_worker_mb,
            if device.phone { "; phone" } else { "" },
        ),
    }
}

/// This wasm instance's linear memory, MB: inside a worker, the memory that
/// worker holds (the data dominate it). `None` natively, where threads share
/// one process.
pub fn wasm_memory_mb() -> Option<f64> {
    #[cfg(target_arch = "wasm32")]
    {
        let mem = wasm_bindgen::memory();
        let buf = js_sys::Reflect::get(&mem, &"buffer".into()).ok()?;
        let len = js_sys::Reflect::get(&buf, &"byteLength".into())
            .ok()?
            .as_f64()?;
        Some(len / (1024.0 * 1024.0))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(cores: usize, gb: Option<f64>, phone: bool) -> Device {
        Device {
            logical_cores: cores,
            memory_gb: gb,
            phone,
        }
    }

    /// Cores less one, within the memory budget and the caps; phones get at
    /// most two; an override wins.
    #[test]
    fn the_pool_is_sized_by_cores_memory_and_screen() {
        // A desktop with plenty of memory: cores less one, then the cap.
        assert_eq!(plan(dev(4, Some(8.0), false), 500.0, None).workers, 3);
        assert_eq!(plan(dev(32, Some(8.0), false), 400.0, None).workers, 8);
        // Memory binds: 8 GB × 0.5 = 4096 MB at 1500 MB per worker is 2.
        let p = plan(dev(16, Some(8.0), false), 1500.0, None);
        assert_eq!(p.workers, 2);
        assert!(p.reason.contains("memory"), "{}", p.reason);
        // Unknown memory: a 2 GB budget.
        assert_eq!(plan(dev(16, None, false), 600.0, None).workers, 3);
        // A phone: at most two, and a third of its memory.
        assert_eq!(plan(dev(8, Some(8.0), true), 400.0, None).workers, 2);
        assert_eq!(plan(dev(8, Some(2.0), true), 500.0, None).workers, 1);
        assert_eq!(plan(dev(8, None, true), 600.0, None).workers, 1);
        // Never zero.
        assert_eq!(plan(dev(1, Some(0.5), true), 5000.0, None).workers, 1);
        // The override.
        assert_eq!(plan(dev(2, Some(1.0), true), 900.0, Some(4)).workers, 4);
        assert_eq!(plan(dev(2, None, false), 900.0, Some(0)).workers, 1);
        // The probe answers something sane natively.
        let d = Device::probe();
        assert!(d.logical_cores >= 1 && !d.phone);
        assert!(wasm_memory_mb().is_none());
    }
}
