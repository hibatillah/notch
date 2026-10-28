//! Load and transfer rates require differences between consecutive Windows counter readings.

use std::time::Instant;

use windows::{
    Win32::{
        Foundation::FILETIME,
        NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2},
        Storage::FileSystem::GetDiskFreeSpaceExW,
        System::{
            SystemInformation::{GetSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO},
            Threading::GetSystemTimes,
        },
    },
    core::HSTRING,
};

const IF_TYPE_ETHERNET: u32 = 6;
const IF_TYPE_WIFI: u32 = 71;
const OPER_STATUS_UP: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkKind {
    Wifi,
    Ethernet,
    Other,
}

impl NetworkKind {
    pub fn label(self) -> &'static str {
        match self {
            NetworkKind::Wifi => "Wi\u{2011}Fi",
            NetworkKind::Ethernet => "Ethernet",
            NetworkKind::Other => "Network",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    /// Busy share of all processors since the previous reading; `None` on the first reading.
    pub cpu_percent: Option<f32>,
    pub logical_processors: u32,
    pub memory_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    /// `None` on the first reading.
    pub download_bytes_per_second: Option<f64>,
    pub upload_bytes_per_second: Option<f64>,
    pub network_kind: NetworkKind,
    pub disk_percent: f32,
    pub disk_free_bytes: u64,
}

pub fn cpu_percent(previous: (u64, u64), now: (u64, u64)) -> Option<f32> {
    let total = now.1.checked_sub(previous.1).filter(|total| *total > 0)?;
    let idle = now.0.saturating_sub(previous.0).min(total);
    Some(100.0 * (1.0 - idle as f32 / total as f32))
}

/// Bytes per second between two counter readings; a counter that went backwards (a reset) counts as no traffic.
pub fn bytes_per_second(previous: u64, now: u64, seconds: f64) -> f64 {
    if seconds <= 0.0 { 0.0 } else { now.saturating_sub(previous) as f64 / seconds }
}

pub fn used_percent(total: u64, free: u64) -> f32 {
    if total == 0 { 0.0 } else { 100.0 * (1.0 - free.min(total) as f32 / total as f32) }
}

pub struct Sampler {
    previous_cpu: Option<(u64, u64)>,
    previous_network: Option<(u64, u64, Instant)>,
    drive: HSTRING,
    drive_name: String,
}

impl Sampler {
    pub fn new() -> Self {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
        Self {
            previous_cpu: None,
            previous_network: None,
            drive: HSTRING::from(format!("{drive}\\")),
            drive_name: drive,
        }
    }

    pub fn disk_label(&self) -> String {
        format!("Disk {}", self.drive_name)
    }

    pub fn reset(&mut self) {
        self.previous_cpu = None;
        self.previous_network = None;
    }

    pub fn sample(&mut self) -> Sample {
        let cpu_now = cpu_times();
        let cpu = self.previous_cpu.and_then(|previous| cpu_percent(previous, cpu_now));
        self.previous_cpu = Some(cpu_now);

        let (received, sent, network_kind) = network_counters();
        let now = Instant::now();
        let (download, upload) = match self.previous_network {
            Some((previous_received, previous_sent, then)) => {
                let seconds = now.duration_since(then).as_secs_f64();
                (
                    Some(bytes_per_second(previous_received, received, seconds)),
                    Some(bytes_per_second(previous_sent, sent, seconds)),
                )
            }
            None => (None, None),
        };
        self.previous_network = Some((received, sent, now));

        let mut memory = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        let _ = unsafe { GlobalMemoryStatusEx(&mut memory) };
        let (mut free_to_user, mut disk_total, mut disk_free) = (0u64, 0u64, 0u64);
        let _ = unsafe {
            GetDiskFreeSpaceExW(&self.drive, Some(&mut free_to_user), Some(&mut disk_total), Some(&mut disk_free))
        };
        let mut info = SYSTEM_INFO::default();
        unsafe { GetSystemInfo(&mut info) };

        Sample {
            cpu_percent: cpu,
            logical_processors: info.dwNumberOfProcessors,
            memory_percent: memory.dwMemoryLoad as f32,
            memory_used_bytes: memory.ullTotalPhys.saturating_sub(memory.ullAvailPhys),
            memory_total_bytes: memory.ullTotalPhys,
            download_bytes_per_second: download,
            upload_bytes_per_second: upload,
            network_kind,
            disk_percent: used_percent(disk_total, disk_free),
            disk_free_bytes: disk_free,
        }
    }
}

fn file_time_ticks(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

fn cpu_times() -> (u64, u64) {
    let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
    let _ = unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) };
    // The kernel time already includes the idle time.
    (file_time_ticks(idle), file_time_ticks(kernel) + file_time_ticks(user))
}

fn network_counters() -> (u64, u64, NetworkKind) {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    if unsafe { GetIfTable2(&mut table) }.is_err() || table.is_null() {
        return (0, 0, NetworkKind::Other);
    }
    let (mut received, mut sent) = (0u64, 0u64);
    let mut busiest = (0u64, NetworkKind::Other);
    // SAFETY: GetIfTable2 returned a table of `NumEntries` rows, valid until `FreeMibTable`.
    let rows = unsafe { std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize) };
    for row in rows {
        let is_hardware = row.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
        if !is_hardware || row.OperStatus.0 != OPER_STATUS_UP {
            continue;
        }
        received += row.InOctets;
        sent += row.OutOctets;
        let traffic = row.InOctets + row.OutOctets;
        if traffic >= busiest.0 {
            let kind = match row.Type {
                IF_TYPE_WIFI => NetworkKind::Wifi,
                IF_TYPE_ETHERNET => NetworkKind::Ethernet,
                _ => NetworkKind::Other,
            };
            busiest = (traffic, kind);
        }
    }
    unsafe { FreeMibTable(table.cast()) };
    (received, sent, busiest.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cpu_is_busy_for_the_share_of_time_it_was_not_idle() {
        assert_eq!(cpu_percent((100, 1000), (400, 2000)), Some(70.0));
        assert_eq!(cpu_percent((0, 0), (500, 500)), Some(0.0));
    }

    #[test]
    fn two_readings_with_no_time_between_them_say_nothing() {
        assert_eq!(cpu_percent((100, 1000), (100, 1000)), None);
        assert_eq!(cpu_percent((100, 1000), (50, 900)), None);
    }

    #[test]
    fn idle_time_cannot_exceed_the_time_that_passed() {
        assert_eq!(cpu_percent((0, 0), (900, 500)), Some(0.0));
    }

    #[test]
    fn a_rate_is_the_difference_over_the_time() {
        assert_eq!(bytes_per_second(1000, 5000, 2.0), 2000.0);
        assert_eq!(bytes_per_second(1000, 5000, 0.0), 0.0);
    }

    #[test]
    fn a_counter_that_went_backwards_counts_as_no_traffic() {
        assert_eq!(bytes_per_second(5000, 100, 2.0), 0.0);
    }

    #[test]
    fn the_used_share_of_a_disk_follows_its_free_space() {
        assert_eq!(used_percent(1000, 250), 75.0);
        assert_eq!(used_percent(0, 0), 0.0);
        assert_eq!(used_percent(1000, 5000), 0.0);
    }

    #[test]
    fn a_real_reading_makes_sense() {
        let mut sampler = Sampler::new();
        let first = sampler.sample();
        assert!(first.cpu_percent.is_none() && first.download_bytes_per_second.is_none());
        assert!(first.memory_total_bytes > 0 && first.memory_percent <= 100.0);
        assert!(first.logical_processors > 0);
        assert!(first.disk_percent >= 0.0 && first.disk_percent <= 100.0);
        let second = sampler.sample();
        assert!(
            second.cpu_percent.is_some_and(|percent| (0.0..=100.0).contains(&percent)) || second.cpu_percent.is_none()
        );
        assert!(second.download_bytes_per_second.is_some());
    }
}
