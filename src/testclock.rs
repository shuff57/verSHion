//! Test-only stopwatch that counts the calling thread's CPU time instead of the wall clock.
//!
//! A timing assertion in a test ("this render takes under 0.5 s") is there to catch pathological
//! slowness (a lost work budget, a quadratic loop), not to measure how busy the machine is. The
//! full suite runs on all cores next to other cargo builds, so wall time on a busy machine is two to
//! five times the work the code did. `Stopwatch` reads the thread's own on-CPU time, so a descheduled
//! thread is not charged for the time it spent waiting, while a real slowdown (more work) still is.
//!
//! Use it where `Instant::now()` / `.elapsed()` was: `let t = Stopwatch::start(); ...; t.elapsed()`.
//! Only the calling thread is counted, so use it on code that runs on that thread (all the figure
//! renders do); code that hands work to other threads must stay on `Instant`.
//!
//! Linux reads `/proc/thread-self/schedstat` (nanoseconds on the CPU), else `/proc/thread-self/stat`
//! (utime + stime, 10 ms ticks); anywhere else, or if /proc cannot be read, it is the wall clock.

use std::time::{Duration, Instant};

/// CPU time the calling thread has used so far, or `None` where it cannot be read.
fn thread_cpu() -> Option<Duration> {
    #[cfg(target_os = "linux")]
    {
        if let Some(ns) = std::fs::read_to_string("/proc/thread-self/schedstat").ok().and_then(|s| s.split_whitespace().next()?.parse::<u64>().ok()) {
            return Some(Duration::from_nanos(ns));
        }
        let s = std::fs::read_to_string("/proc/thread-self/stat").ok()?;
        // (the fields after the command's closing bracket: utime and stime are the 12th and 13th)
        let f: Vec<&str> = s[s.rfind(')')? + 1..].split_whitespace().collect();
        let ticks = f.get(11)?.parse::<u64>().ok()? + f.get(12)?.parse::<u64>().ok()?;
        Some(Duration::from_millis(ticks * 10))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Started by [`Stopwatch::start`]; [`Stopwatch::elapsed`] is the calling thread's CPU time since then.
/// (`Debug` shows the wall-clock start too, so `format!("{t:?}")` still works as a unique cache-busting title.)
#[derive(Clone, Copy, Debug)]
pub struct Stopwatch {
    cpu: Option<Duration>,
    wall: Instant,
}

impl Stopwatch {
    pub fn start() -> Self {
        Stopwatch { cpu: thread_cpu(), wall: Instant::now() }
    }

    pub fn elapsed(&self) -> Duration {
        match (self.cpu, thread_cpu()) {
            (Some(a), Some(b)) => b.saturating_sub(a),
            _ => self.wall.elapsed(),
        }
    }
}

/// Runs `f` on its own thread and returns the CPU time it used, or `None` if it had not finished after `limit` of CPU
/// (a stuck or runaway computation: the thread is left running, as a wall-clock `recv_timeout` would). The limit is
/// CPU, not wall, so a busy machine cannot trip it; the thread being watched is read from /proc on Linux, and
/// elsewhere (or if /proc cannot be read) `limit` is wall time. A panic in `f` also gives `None`.
pub fn cpu_within<F: FnOnce() + Send + 'static>(limit: Duration, f: F) -> Option<Duration> {
    use std::sync::mpsc::{channel, RecvTimeoutError};
    let (tx, rx) = channel::<Result<Duration, String>>();
    std::thread::spawn(move || {
        // (first message: where to read this thread's CPU time; then the CPU time it used)
        let _ = tx.send(Err(std::fs::read_link("/proc/thread-self").map(|p| p.display().to_string()).unwrap_or_default()));
        let w = Stopwatch::start();
        f();
        let _ = tx.send(Ok(w.elapsed()));
    });
    let wall = Instant::now();
    let mut path = String::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(Ok(used)) => return Some(used),
            Ok(Err(p)) => path = p,
            Err(RecvTimeoutError::Disconnected) => return None,
            Err(RecvTimeoutError::Timeout) => {}
        }
        let used = std::fs::read_to_string(format!("/proc/{path}/schedstat")).ok().and_then(|s| s.split_whitespace().next()?.parse::<u64>().ok()).map(Duration::from_nanos);
        if used.unwrap_or_else(|| wall.elapsed()) > limit { return None; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_work_but_not_sleeping() {
        let w = Stopwatch::start();
        std::thread::sleep(Duration::from_millis(300));
        let slept = w.elapsed();
        if cfg!(target_os = "linux") {
            assert!(slept < Duration::from_millis(100), "sleeping was charged: {slept:?}");
        }
        // work moves it: spin until it reads 100 ms of CPU (the wall clock bounds the loop, so a stuck clock fails)
        let (w, wall) = (Stopwatch::start(), Instant::now());
        let mut x = 0u64;
        while w.elapsed() < Duration::from_millis(100) && wall.elapsed() < Duration::from_secs(60) {
            x = std::hint::black_box(x.wrapping_add(1));
        }
        assert!(w.elapsed() >= Duration::from_millis(100), "the clock did not advance with work (x = {x})");
    }

    #[test]
    fn cpu_within_lets_slow_wall_time_pass_and_stops_a_spinner() {
        // sleeping for far longer than the limit uses no CPU: finished, not stuck (on Linux; elsewhere the limit is wall time)
        assert!(!cfg!(target_os = "linux") || cpu_within(Duration::from_millis(200), || std::thread::sleep(Duration::from_millis(800))).is_some());
        // a spinner uses CPU past the limit: reported stuck (the thread ends itself after a while so it does not outlive the test run)
        let end = Instant::now() + Duration::from_secs(3);
        assert!(cpu_within(Duration::from_millis(200), move || while Instant::now() < end { std::hint::spin_loop(); }).is_none());
    }
}
