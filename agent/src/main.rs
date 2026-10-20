mod config;
mod metrics;
mod network;
mod sender;

use std::time::Duration;
use sysinfo::{Networks, System};

#[tokio::main]
async fn main() {
    let cfg = match config::Config::load("config.toml") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Config error: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "Agent started: id={}, server={}, interval={}s",
        cfg.computer_id, cfg.server_url, cfg.interval_secs
    );

    let client = match sender::build_client() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("HTTP client error: {e}");
            std::process::exit(1);
        }
    };

    let mut sys = System::new();
    sys.refresh_cpu_usage();
    let mut networks = Networks::new_with_refreshed_list();

    loop {
        tokio::time::sleep(Duration::from_secs(cfg.interval_secs)).await;
        let sys_metrics = metrics::collect(&mut sys);
        let net_metrics = network::collect(&mut networks);

        let payload = serde_json::json!({
            "computer_id": cfg.computer_id,
            "system": sys_metrics,
            "network": net_metrics,
        });

        match sender::send(&client, &cfg, &payload).await {
            Ok(()) => println!("Metrics sent to {}", cfg.server_url),
            Err(e) => eprintln!("Could not send metrics: {e}"),
        }
    }
}
