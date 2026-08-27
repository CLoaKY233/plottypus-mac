#[must_use]
pub fn bytes_short(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    let n = bytes as f64;
    if n >= TIB {
        format!("{:.1}T", n / TIB)
    } else if n >= GIB {
        format!("{:.1}G", n / GIB)
    } else if n >= MIB {
        format!("{:.0}M", n / MIB)
    } else if n >= KIB {
        format!("{:.0}K", n / KIB)
    } else {
        format!("{bytes}B")
    }
}

#[must_use]
pub fn percent_display(ratio: f32) -> String {
    format!("{:.0}%", (ratio.clamp(0.0, 1.0) * 100.0).round())
}

#[must_use]
pub fn bits_per_sec(bps: u64) -> String {
    const K: f64 = 1000.0;
    const M: f64 = K * 1000.0;
    const G: f64 = M * 1000.0;
    let n = bps as f64;
    if n >= G {
        format!("{:.1}Gb", n / G)
    } else if n >= M {
        format!("{:.1}Mb", n / M)
    } else if n >= K {
        format!("{:.0}Kb", n / K)
    } else {
        format!("{bps}b")
    }
}

#[must_use]
pub fn bytes_per_sec(bps: u64) -> String {
    format!("{}/s", bytes_short(bps))
}

#[must_use]
pub fn watts_display(watts: f32) -> String {
    if watts >= 10.0 {
        format!("{watts:.0}W")
    } else {
        format!("{watts:.1}W")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_scale() {
        assert_eq!(bytes_short(512), "512B");
        assert_eq!(bytes_short(2048), "2K");
        assert_eq!(bytes_short(12 * 1024 * 1024), "12M");
        assert_eq!(bytes_short(36 * 1024 * 1024 * 1024), "36.0G");
    }

    #[test]
    fn percent_rounds() {
        assert_eq!(percent_display(0.184), "18%");
        assert_eq!(percent_display(1.2), "100%");
        assert_eq!(percent_display(-0.1), "0%");
    }

    #[test]
    fn watts_precision() {
        assert_eq!(watts_display(8.24), "8.2W");
        assert_eq!(watts_display(18.7), "19W");
    }

    #[test]
    fn throughput_scale() {
        assert_eq!(bits_per_sec(800), "800b");
        assert_eq!(bits_per_sec(12_400_000), "12.4Mb");
        assert_eq!(bytes_per_sec(800), "800B/s");
        assert_eq!(bytes_per_sec(12 * 1024 * 1024), "12M/s");
    }
}
