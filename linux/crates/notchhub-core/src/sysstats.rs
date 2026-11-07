//! CPU, memory, network, battery, disk and the busiest processes.
use sysinfo::{Disks, Networks, ProcessesToUpdate, System};
use std::collections::VecDeque;
use std::time::Instant;

pub const HISTORY: usize = 60;

#[derive(Clone, Debug, Default)]
pub struct Battery {
    pub percent: f64,
    pub charging: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub cpu: f64,
    pub mem: f64,
    pub mem_used_gb: f64,
    pub mem_total_gb: f64,
    pub down: f64,
    pub up: f64,
    pub disk_free_gb: f64,
    pub disk_total_gb: f64,
    pub battery: Option<Battery>,
    pub top: Vec<(String, f64)>,
}

pub struct Monitor {
    sys: System,
    nets: Networks,
    disks: Disks,
    last_net: Option<(Instant, u64, u64)>,
    pub cpu_history: VecDeque<f64>,
    pub net_history: VecDeque<f64>,
    pub snap: Snapshot,
}

fn read_battery() -> Option<Battery> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for e in dir.flatten() {
        let p = e.path();
        if std::fs::read_to_string(p.join("type")).ok()?.trim() != "Battery" {
            continue;
        }
        let percent: f64 = std::fs::read_to_string(p.join("capacity")).ok()?.trim().parse().ok()?;
        let status = std::fs::read_to_string(p.join("status")).unwrap_or_default();
        return Some(Battery { percent, charging: matches!(status.trim(), "Charging" | "Full") });
    }
    None
}

impl Monitor {
    pub fn new() -> Monitor {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Monitor {
            sys,
            nets: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            last_net: None,
            cpu_history: VecDeque::new(),
            net_history: VecDeque::new(),
            snap: Snapshot::default(),
        }
    }

    /// `with_processes`: the busiest-process list costs more, so it is only built while the tab is visible.
    pub fn refresh(&mut self, with_processes: bool) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.nets.refresh(true);
        let (rx, tx) = self.nets.iter().fold((0u64, 0u64), |a, (_, n)| (a.0 + n.total_received(), a.1 + n.total_transmitted()));
        let now = Instant::now();
        let (down, up) = match self.last_net {
            Some((t, r, x)) => {
                let dt = now.duration_since(t).as_secs_f64().max(0.001);
                (rx.saturating_sub(r) as f64 / dt, tx.saturating_sub(x) as f64 / dt)
            }
            None => (0.0, 0.0),
        };
        self.last_net = Some((now, rx, tx));
        let gb = 1024.0 * 1024.0 * 1024.0;
        let total = self.sys.total_memory() as f64;
        let used = self.sys.used_memory() as f64;
        self.disks.refresh(true);
        let root = self.disks.iter().find(|d| d.mount_point() == std::path::Path::new("/")).or_else(|| self.disks.iter().next());
        let mut top = std::mem::take(&mut self.snap.top);
        if with_processes {
            self.sys.refresh_processes(ProcessesToUpdate::All, true);
            let ncpu = self.sys.cpus().len().max(1) as f64;
            let mut v: Vec<(String, f64)> =
                self.sys.processes().values().map(|p| (p.name().to_string_lossy().to_string(), p.cpu_usage() as f64 / ncpu)).collect();
            v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            v.truncate(3);
            top = v;
        }
        self.snap = Snapshot {
            cpu: self.sys.global_cpu_usage() as f64,
            mem: if total > 0.0 { used / total * 100.0 } else { 0.0 },
            mem_used_gb: used / gb,
            mem_total_gb: total / gb,
            down,
            up,
            disk_free_gb: root.map_or(0.0, |d| d.available_space() as f64 / gb),
            disk_total_gb: root.map_or(0.0, |d| d.total_space() as f64 / gb),
            battery: read_battery(),
            top,
        };
        push(&mut self.cpu_history, self.snap.cpu);
        push(&mut self.net_history, down + up);
    }
}

fn push(q: &mut VecDeque<f64>, v: f64) {
    q.push_back(v);
    while q.len() > HISTORY {
        q.pop_front();
    }
}

pub fn human_rate(bytes_per_s: f64) -> String {
    if bytes_per_s >= 1024.0 * 1024.0 {
        format!("{:.1} MB/s", bytes_per_s / 1024.0 / 1024.0)
    } else {
        format!("{:.0} KB/s", bytes_per_s / 1024.0)
    }
}

/// Raises each alert once, then waits `cooldown` seconds before repeating it.
#[derive(Default)]
pub struct AlertGate {
    last: std::collections::HashMap<&'static str, f64>,
}

impl AlertGate {
    pub fn check(&mut self, key: &'static str, now_s: f64, cooldown: f64) -> bool {
        match self.last.get(key) {
            Some(t) if now_s - t < cooldown => false,
            _ => {
                self.last.insert(key, now_s);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_are_readable() {
        assert_eq!(human_rate(512.0 * 1024.0), "512 KB/s");
        assert_eq!(human_rate(3.0 * 1024.0 * 1024.0), "3.0 MB/s");
    }

    #[test]
    fn alert_gate_waits_for_cooldown() {
        let mut g = AlertGate::default();
        assert!(g.check("cpu", 0.0, 300.0));
        assert!(!g.check("cpu", 100.0, 300.0));
        assert!(g.check("mem", 100.0, 300.0));
        assert!(g.check("cpu", 301.0, 300.0));
    }

    #[test]
    fn monitor_reads_this_machine() {
        let mut m = Monitor::new();
        m.refresh(true);
        std::thread::sleep(std::time::Duration::from_millis(250));
        m.refresh(true);
        assert!(m.snap.mem_total_gb > 0.0 && m.snap.mem > 0.0);
        assert!(m.cpu_history.len() == 2);
    }
}
