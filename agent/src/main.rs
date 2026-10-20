mod metrics;
mod network;

use std::time::Duration;
use sysinfo::{Networks, System};

#[tokio::main]
async fn main() {
    let mut sys = System::new();
    sys.refresh_cpu_usage();
    let mut networks = Networks::new_with_refreshed_list();

    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let sys_metrics = metrics::collect(&mut sys);
        let net_metrics = network::collect(&mut networks);

        let payload = serde_json::json!({
            "system": sys_metrics,
            "network": net_metrics,
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    }
}