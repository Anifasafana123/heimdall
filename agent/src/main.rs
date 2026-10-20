mod metrics;

use std::time::Duration;
use sysinfo::System;

#[tokio::main]
async fn main() {
    let mut sys = System::new();
    sys.refresh_cpu_usage();

    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let m = metrics::collect(&mut sys);
        println!("{}", serde_json::to_string_pretty(&m).unwrap());
    }
}