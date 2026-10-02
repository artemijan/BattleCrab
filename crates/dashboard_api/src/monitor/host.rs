//! Host-level pressure, read by the dashboard itself (`docs/MONITORING.md`
//! §3): the box both servers share, so one reading per interval rather than
//! one per service.

use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostReading {
    /// 1/5/15-minute load averages.
    pub load: Option<[f64; 3]>,
    pub mem_total_bytes: Option<i64>,
    /// `MemAvailable`, not `MemFree`: free excludes reclaimable page cache, so
    /// on a healthy Linux box it trends to zero and reads as a false alarm.
    pub mem_available_bytes: Option<i64>,
    /// The filesystem holding `metrics.db` — which in every deployment is the
    /// one holding the game database and the logs too.
    pub disk_total_bytes: Option<i64>,
    pub disk_free_bytes: Option<i64>,
}

pub fn read(disk_path: &Path) -> HostReading {
    let (mem_total_bytes, mem_available_bytes) = memory();
    let (disk_total_bytes, disk_free_bytes) = disk(disk_path);
    HostReading {
        load: load(),
        mem_total_bytes,
        mem_available_bytes,
        disk_total_bytes,
        disk_free_bytes,
    }
}

/// `getloadavg(3)`: Linux and macOS both, no procfs needed.
fn load() -> Option<[f64; 3]> {
    let mut l = [0f64; 3];
    // SAFETY: `l` is a valid buffer of exactly the 3 elements requested.
    let n = unsafe { libc::getloadavg(l.as_mut_ptr(), 3) };
    (n == 3).then_some(l)
}

#[cfg(target_os = "linux")]
fn memory() -> (Option<i64>, Option<i64>) {
    match std::fs::read_to_string("/proc/meminfo") {
        Ok(text) => parse_meminfo(&text),
        Err(_) => (None, None),
    }
}

/// No procfs off Linux; local macOS dev shows memory as unavailable, as the
/// servers' RSS does.
#[cfg(not(target_os = "linux"))]
fn memory() -> (Option<i64>, Option<i64>) {
    (None, None)
}

/// `(MemTotal, MemAvailable)` in bytes. The file reports kB.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_meminfo(text: &str) -> (Option<i64>, Option<i64>) {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))
            .and_then(|rest| rest.split_whitespace().next()?.parse::<i64>().ok())
            .map(|kb| kb * 1024)
    };
    (field("MemTotal"), field("MemAvailable"))
}

/// `statvfs(3)` on `path`, or its parent if the file does not exist yet.
fn disk(path: &Path) -> (Option<i64>, Option<i64>) {
    let target = if path.exists() {
        path
    } else {
        match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        }
    };
    let Ok(c_path) = std::ffi::CString::new(target.as_os_str().as_encoded_bytes()) else {
        return (None, None);
    };
    let mut st = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `c_path` is NUL-terminated and `st` is a writable `statvfs`.
    if unsafe { libc::statvfs(c_path.as_ptr(), st.as_mut_ptr()) } != 0 {
        return (None, None);
    }
    // SAFETY: `statvfs` returned 0, so it filled the struct.
    let st = unsafe { st.assume_init() };
    // Field widths differ by platform (u32 vs u64), hence the casts.
    #[allow(clippy::unnecessary_cast)]
    let frsize = st.f_frsize as i64;
    #[allow(clippy::unnecessary_cast)]
    let (blocks, avail) = (st.f_blocks as i64, st.f_bavail as i64);
    // `f_bavail`, not `f_bfree`: the blocks an unprivileged process — the
    // servers — can actually use.
    (Some(blocks * frsize), Some(avail * frsize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo_is_read_in_bytes_and_by_exact_key() {
        let text = "MemTotal:        8000000 kB\nMemFree:          100000 kB\n\
                    MemAvailable:    6000000 kB\nMemAvailableX:  1 kB\n";
        assert_eq!(
            parse_meminfo(text),
            (Some(8_000_000 * 1024), Some(6_000_000 * 1024))
        );
        assert_eq!(parse_meminfo(""), (None, None));
    }

    #[test]
    fn load_and_disk_are_available_on_every_platform_we_run() {
        let r = read(Path::new("does-not-exist-yet.db"));
        assert!(r.load.is_some());
        let (total, free) = (r.disk_total_bytes.unwrap(), r.disk_free_bytes.unwrap());
        assert!(total > 0 && free >= 0 && free <= total);
    }
}
