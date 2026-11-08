//! Human-readable formatting helpers.

/// `1536` → `1.5 KB` (binary units, labelled KB/MB/GB as most users expect).
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v >= 100.0 { format!("{v:.0} {}", UNITS[i]) } else { format!("{v:.1} {}", UNITS[i]) }
}

pub fn speed(bytes_per_sec: f64) -> String {
    if !bytes_per_sec.is_finite() || bytes_per_sec <= 0.0 {
        return "—".into();
    }
    format!("{}/s", bytes(bytes_per_sec as u64))
}

/// `3725` → `1:02:05`, `65` → `1:05`.
pub fn duration(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "—".into();
    }
    let s = secs.round() as u64;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// ETA like `2m 05s`, `1h 03m`, `12s`.
pub fn eta(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1023), "1023 B");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bytes(10 * 1024 * 1024), "10.0 MB");
        assert_eq!(bytes(250 * 1024 * 1024), "250 MB");
    }

    #[test]
    fn formats_duration() {
        assert_eq!(duration(65.0), "1:05");
        assert_eq!(duration(3725.0), "1:02:05");
        assert_eq!(duration(-1.0), "—");
    }

    #[test]
    fn formats_eta_and_speed() {
        assert_eq!(eta(12), "12s");
        assert_eq!(eta(125), "2m 05s");
        assert_eq!(eta(3780), "1h 03m");
        assert_eq!(speed(2048.0), "2.0 KB/s");
        assert_eq!(speed(0.0), "—");
    }
}
