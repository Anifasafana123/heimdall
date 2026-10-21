use serde::Serialize;
use sysinfo::{Disks, System};

#[derive(Debug, Serialize)]
pub struct SystemMetrics {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub uptime_secs: u64,
    pub cpu_usage_percent: f32,
    pub cpu_cores: usize,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
}

pub fn collect(sys: &mut System) -> SystemMetrics {
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let disks = Disks::new_with_refreshed_list();
    let disk_total: u64 = disks.iter().map(|d| d.total_space()).sum();
    let disk_free: u64 = disks.iter().map(|d| d.available_space()).sum();

    SystemMetrics {
        hostname: System::host_name().unwrap_or_default(),
        os_name: System::name().unwrap_or_default(),
        os_version: System::os_version().unwrap_or_default(),
        uptime_secs: System::uptime(),
        cpu_usage_percent: sys.global_cpu_usage(),
        cpu_cores: sys.cpus().len(),
        memory_total_bytes: sys.total_memory(),
        memory_used_bytes: sys.used_memory(),
        disk_total_bytes: disk_total,
        disk_used_bytes: disk_total.saturating_sub(disk_free),
    }
}
