//! Resource measurements of the running program. Ignored by default (the
//! numbers only mean something for a release build); CI runs it with
//! `cargo test --release --test perf -- --ignored --nocapture`.
//!
//! Every measurement is printed as a `PERF key = value` line.

mod common;

use std::fmt::Display;
use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use common::Session;

/// Linux reports CPU time in clock ticks of 1/100 s (USER_HZ).
const TICKS_PER_SEC: f64 = 100.0;

fn report(key: &str, value: impl Display) {
    println!("PERF {key} = {value}");
}

fn status_kb(pid: u32, key: &str) -> u64 {
    fs::read_to_string(format!("/proc/{pid}/status"))
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix(key))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// utime + stime of the process, in clock ticks.
fn cpu_ticks(pid: u32) -> u64 {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    let rest = stat.rsplit(") ").next().unwrap_or("");
    let mut f = rest.split_whitespace();
    let utime: u64 = f.nth(11).and_then(|v| v.parse().ok()).unwrap_or(0);
    let stime: u64 = f.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    utime + stime
}

/// Resident memory of the direct children (the pane shells), in kB.
fn children_rss_kb(parent: u32) -> u64 {
    let mut total = 0;
    for entry in fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        if status_kb(pid, "PPid:") == u64::from(parent) {
            total += status_kb(pid, "VmRSS:");
        }
    }
    total
}

fn first_line_with(path: &str, key: &str) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split(':').nth(1))
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn idle_cpu_percent(pid: u32, seconds: u64) -> f64 {
    let before = cpu_ticks(pid);
    thread::sleep(Duration::from_secs(seconds));
    let used = cpu_ticks(pid) - before;
    used as f64 / TICKS_PER_SEC / seconds as f64 * 100.0
}

fn snapshot(s: &Session, panes: usize) {
    let pid = s.pid();
    thread::sleep(Duration::from_secs(1));
    report(&format!("panes_{panes}_rss_kb"), status_kb(pid, "VmRSS:"));
    report(
        &format!("panes_{panes}_peak_rss_kb"),
        status_kb(pid, "VmHWM:"),
    );
    report(
        &format!("panes_{panes}_shells_rss_kb"),
        children_rss_kb(pid),
    );
    report(
        &format!("panes_{panes}_idle_cpu_percent_5s"),
        format!("{:.2}", idle_cpu_percent(pid, 5)),
    );
}

#[test]
#[ignore = "measurement; run in release mode"]
fn measure_memory_and_cpu() {
    report("cpu_model", first_line_with("/proc/cpuinfo", "model name"));
    report(
        "cpu_threads",
        thread::available_parallelism().map_or(0, |n| n.get()),
    );
    report(
        "mem_total_kb",
        first_line_with("/proc/meminfo", "MemTotal").replace(" kB", ""),
    );
    report(
        "kernel",
        fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap_or_default()
            .trim(),
    );
    report(
        "binary_bytes",
        fs::metadata(common::bin_path()).map_or(0, |m| m.len()),
    );

    // A large terminal so nine panes each get a real size.
    let mut s = Session::start(50, 200);
    snapshot(&s, 1);

    for n in 2..=4 {
        s.send("\x02c");
        s.wait_for(&format!("pane {n}/{n}"));
    }
    snapshot(&s, 4);

    for n in 5..=9 {
        s.send("\x02c");
        s.wait_for(&format!("pane {n}/{n}"));
    }
    snapshot(&s, 9);

    // Heavy output goes to pane 1, the largest: the ninth pane is only about
    // a dozen columns wide, which would wrap the marker below.
    s.window_mode();
    s.send("1");
    s.send("i");
    s.wait_for("TERMINAL");

    // Heavy output into the focused pane.
    let pid = s.pid();
    let ticks_before = cpu_ticks(pid);
    let start = Instant::now();
    // The marker is computed by the shell, so it appears only once the
    // output has finished (the typed command line contains `$((6*7))`).
    s.send("seq 1 300000; echo flood_done_$((6*7))\r");
    s.wait_for("flood_done_42");
    let secs = start.elapsed().as_secs_f64();
    let used = (cpu_ticks(pid) - ticks_before) as f64 / TICKS_PER_SEC;
    report("flood_300000_lines_wall_seconds", format!("{secs:.2}"));
    report("flood_300000_lines_cpu_seconds", format!("{used:.2}"));
    report("flood_peak_rss_kb", status_kb(pid, "VmHWM:"));

    // Loose guards that only catch something badly wrong.
    assert!(status_kb(pid, "VmHWM:") < 300_000, "peak RSS above 300 MB");
    s.quit();
    assert!(s.exited_ok());
}
