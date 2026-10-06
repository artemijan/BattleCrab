//! Process-level CPU and memory readings — `docs/MONITORING.md` §4.
//!
//! Both are cheap enough to read every sample: one syscall for CPU, one small
//! procfs read for RSS. No `sysinfo`: it is a large dependency tree for two
//! numbers.

/// User + system CPU time this process has consumed since it started, in
/// microseconds. `getrusage(RUSAGE_SELF)` sums every thread on both Linux and
/// macOS, so the game thread, the tokio workers and the writers all count.
pub fn cpu_micros() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` is a valid, writable `rusage`; RUSAGE_SELF is always a
    // valid `who`, so the call cannot fail on the arguments we pass.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc != 0 {
        return 0;
    }
    // SAFETY: `getrusage` returned 0, so it filled the struct.
    let usage = unsafe { usage.assume_init() };
    let micros = |t: libc::timeval| t.tv_sec as u64 * 1_000_000 + t.tv_usec as u64;
    micros(usage.ru_utime) + micros(usage.ru_stime)
}

/// Resident set size in bytes: `/proc/self/statm`'s second field (resident
/// pages) times the page size.
#[cfg(target_os = "linux")]
pub fn rss_bytes() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    // SAFETY: `sysconf` has no memory-safety preconditions.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (page_size > 0).then(|| pages * page_size as u64)
}

/// No procfs off Linux. `None` is stored as SQL `NULL`, so local macOS dev
/// shows memory as unavailable while prod (Linux) reports it.
#[cfg(not(target_os = "linux"))]
pub fn rss_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_time_only_moves_forward() {
        let before = cpu_micros();
        // Burn a little CPU so the reading has something to count.
        let mut x = 0u64;
        for i in 0..2_000_000u64 {
            x = x.wrapping_mul(31).wrapping_add(i);
        }
        std::hint::black_box(x);
        assert!(cpu_micros() >= before);
        assert!(before > 0, "a running test process has used some CPU");
    }

    #[test]
    fn rss_is_either_unavailable_or_plausible() {
        // Passes on both macOS (None) and Linux (a real number).
        if let Some(bytes) = rss_bytes() {
            assert!(
                bytes > 1024 * 1024,
                "RSS of {bytes} bytes is implausibly small"
            );
        }
    }
}
