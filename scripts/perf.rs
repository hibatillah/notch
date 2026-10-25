//! Performance check: starts the notch, moves the pointer to open and close it, and fails when memory or CPU use
//! exceeds the limits below. Run it through `scripts/perf.sh`, which builds the notch and passes the paths.
//!
//! Limits are what a healthy build stays well under; raise one only with a reason.

use std::{
    fs,
    os::windows::io::AsRawHandle,
    path::{Path, PathBuf},
    process::{Child, Command, ExitCode},
    thread::sleep,
    time::{Duration, Instant},
};

use windows::Win32::{
    Foundation::{FILETIME, HANDLE},
    System::{
        ProcessStatus::K32QueryWorkingSet,
        Threading::{GetProcessHandleCount, GetProcessTimes},
    },
    UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SetCursorPos, SetProcessDPIAware},
};

const IDLE_SECONDS: u64 = 20;
const CYCLES: u32 = 10;
const MAX_IDLE_CPU_PERCENT: f64 = 0.1;
const MAX_CPU_MS_PER_CYCLE: f64 = 400.0;
const MAX_MEMORY_MB: f64 = 30.0;
const MAX_MEMORY_GROWTH_MB: f64 = 3.0;
const MAX_HANDLE_GROWTH: f64 = 20.0;
const SETTLE_SECONDS: u64 = 20;
/// A point far from the notch, where the pointer rests while the notch should be collapsed.
const PARKED_POINTER: (i32, i32) = (50, 600);
const PAGE_SIZE_BYTES: usize = 4096;
const BYTES_PER_MEGABYTE: f64 = 1024.0 * 1024.0;

struct Measurement {
    private_working_set_mb: f64,
    handles: u32,
    cpu_ms: f64,
}

/// The notch process; it is stopped when this goes out of scope, even after a failed check.
struct RunningNotch(Child);

impl RunningNotch {
    fn start(executable: &Path, log: &Path, pages: Option<&str>) -> Self {
        park_pointer();
        let _ = fs::remove_file(log);
        let mut command = Command::new(executable);
        command.env("NOTCH_DEBUG", log);
        match pages {
            Some(pages) => command.env("NOTCH_PAGES", pages),
            None => command.env_remove("NOTCH_PAGES"),
        };
        let notch = Self(command.spawn().expect("the notch executable should start"));
        sleep(Duration::from_secs(3));
        notch
    }

    fn measure(&self) -> Measurement {
        let process = HANDLE(self.0.as_raw_handle());
        let (mut creation, mut exit, mut kernel, mut user) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let mut handles = 0;
        unsafe {
            let _ = GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user);
            let _ = GetProcessHandleCount(process, &mut handles);
        }
        // File times count 100 ns ticks.
        let cpu_ms = (ticks(kernel) + ticks(user)) as f64 / 10_000.0;
        Measurement { private_working_set_mb: private_working_set_mb(process), handles, cpu_ms }
    }

    /// Start-up and animations finish at their own pace; idle use only means something once the notch is quiet.
    fn wait_until_quiet(&self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(SETTLE_SECONDS);
        let mut last = self.measure().cpu_ms;
        while Instant::now() < deadline {
            sleep(Duration::from_secs(2));
            let now = self.measure().cpu_ms;
            if now - last < 1.0 {
                return true;
            }
            last = now;
        }
        false
    }

    /// CPU use, as a percentage of one core, while the pointer is away and nothing should be moving.
    fn idle_cpu_percent(&self) -> f64 {
        park_pointer();
        let before = self.measure().cpu_ms;
        sleep(Duration::from_secs(IDLE_SECONDS));
        let used = self.measure().cpu_ms - before;
        (100.0 * used / (IDLE_SECONDS as f64 * 1000.0) * 1000.0).round() / 1000.0
    }
}

impl Drop for RunningNotch {
    fn drop(&mut self) {
        park_pointer();
        let _ = self.0.kill();
    }
}

fn ticks(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

fn move_pointer(x: i32, y: i32) {
    unsafe {
        let _ = SetCursorPos(x, y);
    }
}

fn park_pointer() {
    move_pointer(PARKED_POINTER.0, PARKED_POINTER.1);
}

/// Memory only this process uses: working-set pages shared with no other process.
fn private_working_set_mb(process: HANDLE) -> f64 {
    // The buffer starts with the entry count, followed by one word per page.
    let mut capacity = 8192usize;
    loop {
        let mut buffer = vec![0usize; capacity + 1];
        let size_bytes = (buffer.len() * size_of::<usize>()) as u32;
        let is_complete = unsafe { K32QueryWorkingSet(process, buffer.as_mut_ptr().cast(), size_bytes).as_bool() };
        let count = buffer[0];
        if is_complete {
            // Bit 8 of each word is set when the page can be shared with other processes.
            let private_pages = buffer[1..=count].iter().filter(|word| (*word >> 8) & 1 == 0).count();
            return (private_pages * PAGE_SIZE_BYTES) as f64 / BYTES_PER_MEGABYTE;
        }
        capacity = if count > capacity { count + 512 } else { capacity * 2 };
    }
}

struct Report {
    failures: Vec<String>,
}

impl Report {
    fn check(&mut self, name: &str, value: f64, limit: f64, unit: &str) {
        let is_ok = value <= limit;
        println!("  {name:<34} {value:>8} {unit:<4} (limit {limit} {unit})  {}", if is_ok { "ok" } else { "FAIL" });
        if !is_ok {
            self.failures.push(name.to_string());
        }
    }

    fn require(&mut self, name: &str, is_ok: bool, detail: &str) {
        println!("  {name:<34} {detail}  {}", if is_ok { "ok" } else { "FAIL" });
        if !is_ok {
            self.failures.push(name.to_string());
        }
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let value_of =
        |flag: &str| arguments.iter().position(|argument| argument == flag).and_then(|index| arguments.get(index + 1));
    let (Some(executable), Some(log)) = (value_of("--exe"), value_of("--log")) else {
        eprintln!("usage: perf --exe <notch.exe> --log <log file> [--with-media]");
        return ExitCode::from(2);
    };
    let (executable, log) = (PathBuf::from(executable), PathBuf::from(log));
    let with_media = arguments.iter().any(|argument| argument == "--with-media");

    unsafe {
        let _ = SetProcessDPIAware();
    }
    let center_x = unsafe { GetSystemMetrics(SM_CXSCREEN) } / 2;
    let mut report = Report { failures: Vec::new() };

    println!("Notch performance check: shelf page only, {IDLE_SECONDS} s idle, {CYCLES} open/close cycles");
    {
        let notch = RunningNotch::start(&executable, &log, Some("shelf"));
        let started = notch.measure();
        println!(
            "  after start: {:.1} MB private working set, {} handles",
            started.private_working_set_mb, started.handles
        );

        println!("Idle, collapsed:");
        report.require("settles after start-up", notch.wait_until_quiet(), "");
        report.check("CPU while idle", notch.idle_cpu_percent(), MAX_IDLE_CPU_PERCENT, "%");
        report.check("memory (private working set)", round(started.private_working_set_mb), MAX_MEMORY_MB, "MB");

        println!("Opening and closing {CYCLES} times:");
        let before = notch.measure();
        for _ in 0..CYCLES {
            move_pointer(center_x, 12);
            sleep(Duration::from_millis(1000));
            park_pointer();
            sleep(Duration::from_millis(1000));
        }
        let after = notch.measure();
        let cpu_per_cycle = ((after.cpu_ms - before.cpu_ms) / f64::from(CYCLES)).round();
        report.check("CPU per open+close cycle", cpu_per_cycle, MAX_CPU_MS_PER_CYCLE, "ms");
        let growth = round(after.private_working_set_mb - before.private_working_set_mb);
        report.check("memory growth over the cycles", growth, MAX_MEMORY_GROWTH_MB, "MB");
        let handle_growth = f64::from(after.handles) - f64::from(before.handles);
        report.check("handle growth over the cycles", handle_growth, MAX_HANDLE_GROWTH, "");

        let log_text = fs::read_to_string(&log).unwrap_or_default();
        let events = log_text.lines().filter(|line| line.contains("hovering=")).count();
        let expected = 2 * CYCLES as usize;
        let hover_ok = events == expected;
        report.require("hover events logged", hover_ok, &format!("{events:>8}      (expected {expected})"));
        if !hover_ok {
            println!("    log:\n{log_text}");
        }

        println!("Idle again after the animations (timers must have stopped):");
        report.require("settles after the animations", notch.wait_until_quiet(), "");
        report.check("CPU while idle after cycles", notch.idle_cpu_percent(), MAX_IDLE_CPU_PERCENT, "%");
    }

    if with_media {
        println!("All pages on (informational; play music first):");
        let notch = RunningNotch::start(&executable, &log, None);
        let measured = notch.measure();
        let idle = notch.idle_cpu_percent();
        println!(
            "  {:.1} MB private working set, {} handles; CPU {idle} % of one core over {IDLE_SECONDS} s",
            measured.private_working_set_mb, measured.handles
        );
    }
    let _ = fs::remove_file(&log);

    if report.failures.is_empty() {
        println!("All performance checks passed.");
        ExitCode::SUCCESS
    } else {
        println!("FAILED: {}", report.failures.join(", "));
        ExitCode::FAILURE
    }
}

fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
