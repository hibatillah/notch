//! How the stats page writes numbers. Pure, so it is tested without a window.

const KILO: f64 = 1000.0;
const GIGA: f64 = 1_000_000_000.0;
const GIBI: f64 = 1024.0 * 1024.0 * 1024.0;

/// "12%".
pub fn percent_text(percent: f32) -> String {
    format!("{}%", percent.round().clamp(0.0, 100.0) as u32)
}

/// A number with one decimal below 100 and none from 100 up: "2.4", "120".
fn short_number(value: f64) -> String {
    if value >= 99.95 { format!("{}", value.round() as u64) } else { format!("{value:.1}") }
}

/// A transfer rate: "0 B/s", "120 KB/s", "2.4 MB/s".
pub fn rate_text(bytes_per_second: f64) -> String {
    let rate = bytes_per_second.max(0.0);
    if rate < KILO {
        format!("{} B/s", rate.round() as u64)
    } else if rate < KILO * KILO {
        format!("{} KB/s", short_number(rate / KILO))
    } else if rate < KILO * KILO * KILO {
        format!("{} MB/s", short_number(rate / (KILO * KILO)))
    } else {
        format!("{} GB/s", short_number(rate / (KILO * KILO * KILO)))
    }
}

/// A disk size in decimal gigabytes, the way Windows reports drives: "200 GB".
pub fn disk_text(bytes: u64) -> String {
    format!("{} GB", short_number(bytes as f64 / GIGA))
}

/// "8.1 of 16 GB": the memory in use and the total, which Windows reports in binary gigabytes.
pub fn memory_pair_text(used: u64, total: u64) -> String {
    format!("{} of {} GB", short_number(used as f64 / GIBI), (total as f64 / GIBI).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentages_are_whole_numbers_inside_the_range() {
        assert_eq!(percent_text(11.6), "12%");
        assert_eq!(percent_text(-3.0), "0%");
        assert_eq!(percent_text(140.0), "100%");
    }

    #[test]
    fn rates_pick_a_unit_and_keep_a_short_number() {
        assert_eq!(rate_text(0.0), "0 B/s");
        assert_eq!(rate_text(999.0), "999 B/s");
        assert_eq!(rate_text(120_000.0), "120 KB/s");
        assert_eq!(rate_text(2_400_000.0), "2.4 MB/s");
        assert_eq!(rate_text(56_000.0), "56.0 KB/s");
        assert_eq!(rate_text(1_500_000_000.0), "1.5 GB/s");
    }

    #[test]
    fn a_negative_rate_reads_as_zero() {
        assert_eq!(rate_text(-5.0), "0 B/s");
    }

    #[test]
    fn the_memory_pair_rounds_the_total_to_whole_gigabytes() {
        assert_eq!(memory_pair_text(8_697_308_160, 17_179_869_184), "8.1 of 16 GB");
    }

    #[test]
    fn disks_are_in_decimal_gigabytes() {
        assert_eq!(disk_text(200_000_000_000), "200 GB");
        assert_eq!(disk_text(34_000_000_000), "34.0 GB");
    }
}
